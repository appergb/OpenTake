//! `EditorSession` — the in-memory document: the authoritative
//! [`opentake_ops::EditorState`] (timeline + manifest + undo/redo + version)
//! plus the bundle path and generation log that live outside `EditorState` but
//! are needed to round-trip a `.opentake` project.
//!
//! This is the data half of the assembly layer; [`crate::core::AppCore`] wraps
//! it in a lock + event bus to form the concurrent, observable façade.
//!
//! ## What lives where (and why this isn't a second EditorState)
//!
//! `EditorState` already owns the editable truth (timeline, manifest) and the
//! whole undo/version transaction machinery (Batch 1). This session **does not
//! duplicate any of that** — it holds `EditorState` by value and delegates every
//! edit to [`opentake_ops::command::apply`]. It only adds the two pieces of
//! project state `EditorState` deliberately omits (it is persistence-agnostic):
//!
//! - `project_dir`: the `.opentake` bundle path, so a no-arg save knows where to
//!   write (upstream `EditorViewModel.projectURL`).
//! - `generation_log`: the append-only AI audit log, persisted as
//!   `generation-log.json` (upstream `EditorViewModel.generationLog`; the type
//!   lives in `opentake-project`, not `opentake-domain`).
//!
//! ## Open assembly order (`core-SPEC.md` §5.4, upstream `makeWindowControllers`)
//!
//! 1. decode `timeline` → `EditorState` at version 0,
//! 2. record `project_dir`,
//! 3. decode `manifest` into `EditorState`,
//! 4. decode `generation_log` (lenient); when no valid log exists, seed one
//!    deterministically from manifest generation provenance.
//!
//! Asset materialization / thumbnails / waveforms (step 3's tail in the spec)
//! are a media-layer concern injected via [`crate::deps`] and are not performed
//! here.

use std::fs;
use std::path::{Path, PathBuf};

use opentake_domain::{
    ClipType, GenerationInput, GenerationJobStatus, MediaAsset, MediaColorMetadata, MediaManifest,
    MediaManifestEntry, MediaProxy, MediaSource, Timeline,
};
use opentake_ops::command::{self, EditCommand, EditResult};
use opentake_ops::{EditorState, IdGen};
use opentake_project::{
    GenerationLog, GenerationLogEntry, Project, ProjectCompatibility, ProjectRoot,
    ProjectRootIdentity, ThumbnailUpdate,
};
use same_file::Handle;

use crate::error::{CoreError, Result};

/// The subset of probed media facts the session needs to materialize an asset.
///
/// `opentake-core` deliberately does not depend on `opentake-media` (the
/// assembly layer stays decoupled from the heavy ffmpeg/ML stack — see
/// [`crate::deps`]). The caller that owns the media engine (`src-tauri`) probes
/// the file and hands these plain values in, so [`EditorSession::import_media_file`]
/// stays unit-testable without invoking ffprobe. Mirrors the facts
/// `MediaAsset.loadMetadata` reads upstream (duration / dimensions / fps /
/// audio presence).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProbedMedia {
    /// Duration in seconds (0 for stills).
    pub duration_secs: f64,
    /// Rotation-corrected pixel width, when known.
    pub width: Option<i32>,
    /// Rotation-corrected pixel height, when known.
    pub height: Option<i32>,
    /// Frames per second for video, when known.
    pub fps: Option<f64>,
    /// Whether the file carries an audio track.
    pub has_audio: bool,
    /// Source color signalling for HDR-aware decode and durable project state.
    pub color: Option<MediaColorMetadata>,
}

/// Non-secret provenance attached when a separated audio stem re-enters the
/// ordinary media manifest. Content/model hashes make the derivation auditable
/// without persisting provider credentials or result URLs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedStemProvenance {
    pub source_asset_id: String,
    pub source_sha256: String,
    pub execution: String,
    pub model_sha256: Option<String>,
    pub stem: String,
}

/// Validated provider-neutral generation job prepared by the Agent/Tauri host.
/// Credentials, signed URLs, and provider diagnostics are deliberately absent.
#[derive(Clone, Debug)]
pub struct PreparedGenerationJob {
    pub name: String,
    pub kind: ClipType,
    pub folder_id: Option<String>,
    pub provider: String,
    pub input: GenerationInput,
    pub output_count: usize,
    pub source_asset_id: Option<String>,
    pub source_clip_id: Option<String>,
    pub estimated_cost_credits: Option<i64>,
    pub created_at: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationJobCommit {
    pub job_id: String,
    pub placeholder_asset_ids: Vec<String>,
}

/// One durable lifecycle update. Provider messages are reduced to a fixed
/// application-owned `error_code` before reaching this boundary.
#[derive(Clone, Debug)]
pub struct GenerationStateUpdate {
    pub status: GenerationJobStatus,
    pub progress: Option<f64>,
    pub error_code: Option<String>,
    pub provider_job_id: Option<String>,
    pub cost_credits: Option<i64>,
    pub created_at: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct PreparedGenerationOutput {
    pub asset_id: String,
    pub relative_path: String,
    pub probe: ProbedMedia,
    pub created_at: Option<f64>,
}

#[derive(Clone)]
pub(crate) struct GenerationStateCheckpoint {
    manifest: MediaManifest,
    log: GenerationLog,
    component_present: bool,
}

/// File extensions the importer accepts, grouped by the [`ClipType`] they map to.
///
/// Upstream's picker (`MediaTab.swift:754` — `allowedContentTypes = [.movie,
/// .image, .audio, .json]`) surfaces *anything* AVFoundation recognizes for those
/// UTTypes, far more than upstream's own bare-extension `ClipType(fileExtension:)`
/// list. OpenTake's importer routes every decode through the system `ffmpeg`,
/// which handles a much wider set of containers/codecs cross-platform, so the
/// white-list is widened to the formats ffmpeg reads well rather than mirroring
/// upstream's narrow macOS-native list. Lottie JSON and `.lottie` containers
/// are admitted by extension and validated by the Tauri render/materializer
/// boundary before they are shown as usable media.
pub const SUPPORTED_VIDEO_EXTENSIONS: [&str; 14] = [
    "mov", "mp4", "m4v", "mkv", "webm", "avi", "mts", "m2ts", "mpg", "mpeg", "3gp", "wmv", "flv",
    "ts",
];
/// Accepted audio extensions.
pub const SUPPORTED_AUDIO_EXTENSIONS: [&str; 11] = [
    "mp3", "wav", "aac", "m4a", "flac", "ogg", "opus", "aiff", "aif", "wma", "caf",
];
/// Accepted image extensions.
pub const SUPPORTED_IMAGE_EXTENSIONS: [&str; 9] = [
    "png", "jpg", "jpeg", "tiff", "heic", "webp", "bmp", "gif", "avif",
];
/// Accepted Lottie document extensions. Validation happens in the desktop
/// renderer because the zero-dependency core must not link the Velato parser.
pub const SUPPORTED_LOTTIE_EXTENSIONS: [&str; 2] = ["json", "lottie"];

/// The [`ClipType`] for `path` if its (lowercased) extension is on the import
/// white-list, else `None`. Lottie files map to [`ClipType::Lottie`] and are
/// validated by the host before registration.
pub fn importable_clip_type(path: &Path) -> Option<ClipType> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if SUPPORTED_VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Some(ClipType::Video)
    } else if SUPPORTED_AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Some(ClipType::Audio)
    } else if SUPPORTED_IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        Some(ClipType::Image)
    } else if SUPPORTED_LOTTIE_EXTENSIONS.contains(&ext.as_str()) {
        Some(ClipType::Lottie)
    } else {
        None
    }
}

fn safe_provider_prefix(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn safe_generation_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn validate_generation_update(update: &GenerationStateUpdate) -> Result<()> {
    if update.cost_credits.is_some_and(|credits| credits < 0) {
        return Err(CoreError::Media(
            "generation cost must not be negative".to_string(),
        ));
    }
    if let Some(progress) = update.progress {
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err(CoreError::Media(
                "generation progress must be finite and between 0 and 1".to_string(),
            ));
        }
    }
    if let Some(error_code) = update.error_code.as_deref() {
        if !safe_generation_error_code(error_code) {
            return Err(CoreError::Media(
                "generation failure code is invalid".to_string(),
            ));
        }
    }
    if update.status == GenerationJobStatus::Failed && update.error_code.is_none() {
        return Err(CoreError::Media(
            "failed generation status requires an error code".to_string(),
        ));
    }
    if update.status != GenerationJobStatus::Failed && update.error_code.is_some() {
        return Err(CoreError::Media(
            "generation error code is only valid for failed status".to_string(),
        ));
    }
    if let Some(provider_job_id) = update.provider_job_id.as_deref() {
        if provider_job_id.is_empty()
            || provider_job_id.len() > 512
            || provider_job_id.chars().any(char::is_control)
            || provider_job_id.contains("://")
        {
            return Err(CoreError::Media(
                "provider job identity is invalid".to_string(),
            ));
        }
    }
    Ok(())
}

fn valid_generation_transition(
    current: Option<GenerationJobStatus>,
    next: GenerationJobStatus,
) -> bool {
    use GenerationJobStatus as Status;
    match (current, next) {
        (None, Status::Queued) => true,
        (Some(current), next) if current == next => true,
        (Some(Status::Queued), Status::Generating | Status::Failed | Status::Cancelled) => true,
        (Some(Status::Generating), Status::Downloading | Status::Failed | Status::Cancelled) => {
            true
        }
        (
            Some(Status::Downloading),
            Status::Finalizing | Status::Ready | Status::Failed | Status::Cancelled,
        ) => true,
        (Some(Status::Finalizing), Status::Ready | Status::Failed | Status::Cancelled) => true,
        (Some(Status::Failed | Status::Cancelled), Status::Queued) => true,
        (Some(Status::Ready), Status::Ready) => true,
        _ => false,
    }
}

fn validate_project_media_relative_path(value: &str) -> Result<()> {
    let path = Path::new(value);
    let mut components = path.components();
    if components.next() != Some(std::path::Component::Normal("media".as_ref()))
        || components.clone().next().is_none()
        || path.is_absolute()
        || components.any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(CoreError::Media(
            "generated output path must be a safe media-relative path".to_string(),
        ));
    }
    Ok(())
}

/// The open document plus its project-level metadata.
pub struct EditorSession {
    /// Authoritative editable state: timeline, manifest, undo/redo, version.
    /// Edits go through [`opentake_ops::command::apply`]; the session never
    /// reimplements the transaction.
    state: EditorState,

    /// Absolute path to the `.opentake` bundle, or `None` for an unsaved project.
    project_dir: Option<PathBuf>,

    /// Retained no-follow authority for the concrete opened/saved bundle.
    /// Same-project reads and writes never re-resolve [`Self::project_dir`].
    project_root: Option<ProjectRoot>,

    /// Append-only AI generation audit log (persisted as `generation-log.json`).
    generation_log: GenerationLog,

    /// Whether a valid optional log component existed when the session opened.
    /// This preserves an intentionally empty log across Save-As without making
    /// every new empty project create the optional component.
    generation_log_component_present: bool,

    /// Persisted fields this build cannot safely write back.
    compatibility: ProjectCompatibility,
}

impl Default for EditorSession {
    fn default() -> Self {
        EditorSession::new_project()
    }
}

impl EditorSession {
    /// A fresh, unsaved project: an empty timeline + manifest at version 0, no
    /// bundle path, an empty generation log. Mirrors creating a new document
    /// before any save.
    pub fn new_project() -> Self {
        EditorSession {
            state: EditorState::default(),
            project_dir: None,
            project_root: None,
            generation_log: GenerationLog::new(),
            generation_log_component_present: false,
            compatibility: ProjectCompatibility::default(),
        }
    }

    /// Open the `.opentake` bundle at `path` into a fresh session, following the
    /// upstream assembly order. The document starts at version 0; the caller is
    /// expected to fetch the first snapshot itself (open does not emit a change
    /// event).
    ///
    /// Propagates [`opentake_project::ProjectError`] (missing/corrupt
    /// `project.json`, etc.) as [`CoreError::Project`].
    pub fn open_project(path: impl AsRef<Path>) -> Result<Self> {
        let project_root = ProjectRoot::open(path)?;
        let project = Project::open_from_root(&project_root)?;
        let compatibility = project.compatibility().clone();
        let generation_log_component_present = project.generation_log.is_some();
        let generation_log = match project.generation_log.clone() {
            Some(log) => log,
            None => project.seed_generation_log_from_assets()?,
        };
        // EditorState::new wraps timeline + manifest with empty history at
        // version 0 — exactly the post-open state we want.
        let state = EditorState::new(project.timeline, project.manifest);
        Ok(EditorSession {
            state,
            project_dir: Some(project.bundle_path),
            project_root: Some(project_root),
            generation_log,
            generation_log_component_present,
            compatibility,
        })
    }

    /// Write the current document to disk.
    ///
    /// With `path = None` it saves back to [`Self::project_dir`] (autosave);
    /// `Some(path)` is a save-as that also adopts the new directory as the
    /// session's project dir. Returns the bundle path that was written.
    ///
    /// Assembles a fresh [`Project`] from clones of the live timeline/manifest
    /// (so saving never mutates the document) plus the generation log, and lets
    /// `opentake-project` write the bundle atomically.
    ///
    /// **Save-as also copies the source bundle's `media/` directory** into the
    /// new bundle (upstream `mediaDirWrapper`, `Project/VideoProject.swift:112-117`):
    /// a project holding internal media
    /// ([`MediaSource::Project`](opentake_domain::MediaSource) relative paths —
    /// AI-generated, pasted, captured stills) would otherwise have every one of
    /// those references silently dangle after Save-As, since `bundle.rs::save`
    /// "never creates or deletes `media/`". A plain save (target equals the
    /// current dir) copies nothing; a missing source `media/` is a no-op; a
    /// partial-copy failure propagates as a real error. Both source and
    /// destination traversal use the retained [`ProjectRoot`] authorities.
    ///
    /// Errors with [`CoreError::NoProjectOpen`] when neither a path nor a
    /// remembered project dir is available.
    pub fn save_project(&mut self, path: Option<PathBuf>) -> Result<PathBuf> {
        self.save_project_with_thumbnail(path, None)
    }

    /// Persist the current media manifest as the sole durable component commit.
    /// Used by library workflows, which do not mutate any other project state.
    pub fn save_media_manifest(&mut self) -> Result<PathBuf> {
        self.ensure_mutable()?;
        let target = self.project_dir.clone().ok_or(CoreError::NoProjectOpen)?;
        let root = self.project_root.as_ref().ok_or(CoreError::NoProjectOpen)?;
        let mut project =
            Project::new_with_compatibility(target.clone(), self.compatibility.clone());
        project.manifest = self.state.manifest.clone();
        project.save_manifest_to_root(root)?;
        Ok(target)
    }

    /// Like [`Self::save_project`] but also writes a cover `thumbnail.jpg` when
    /// `thumbnail` carries JPEG bytes. The caller (which owns the media engine /
    /// GPU) captures the representative frame — see
    /// [`opentake_media::capture_project_thumbnail`], the port of upstream
    /// `captureThumbnail` — and hands the bytes in, so `opentake-core` stays free
    /// of the ffmpeg/GPU stack (`crate::deps`). `None` leaves any existing
    /// `thumbnail.jpg` untouched (`bundle.rs::save` only writes the thumbnail when
    /// [`Project::thumbnail`] is set), matching upstream's best-effort capture
    /// that simply omits the cover on failure.
    pub fn save_project_with_thumbnail(
        &mut self,
        path: Option<PathBuf>,
        thumbnail: Option<Vec<u8>>,
    ) -> Result<PathBuf> {
        self.save_project_with_thumbnail_update(
            path,
            thumbnail.map_or(ThumbnailUpdate::Preserve, ThumbnailUpdate::Replace),
        )
    }

    /// Persist with an explicit authoritative cover mutation.
    pub fn save_project_with_thumbnail_update(
        &mut self,
        path: Option<PathBuf>,
        thumbnail: ThumbnailUpdate,
    ) -> Result<PathBuf> {
        self.ensure_mutable()?;
        // Remember the currently-open bundle before we adopt any new target, so
        // a save-as knows the source `media/` to carry across.
        let previous_dir = self.project_dir.clone();
        let requested_target = match path.or_else(|| previous_dir.clone()) {
            Some(p) => p,
            None => return Err(CoreError::NoProjectOpen),
        };
        let same_target = if previous_dir.as_deref() == Some(requested_target.as_path()) {
            true
        } else if let Some(root) = &self.project_root {
            root.matches_path(&requested_target)?
        } else {
            false
        };
        let target = if same_target {
            previous_dir.clone().unwrap_or(requested_target)
        } else {
            requested_target
        };

        let mut project =
            Project::new_with_compatibility(target.clone(), self.compatibility.clone());
        project.timeline = self.state.timeline.clone();
        project.manifest = self.state.manifest.clone();
        // Preserve an existing valid-but-empty optional component across
        // Save-As; otherwise only create the log once there are rows.
        if self.generation_log_component_present || !self.generation_log.entries.is_empty() {
            project.generation_log = Some(self.generation_log.clone());
        }
        let new_root = if same_target {
            let root = self.project_root.as_ref().ok_or(CoreError::NoProjectOpen)?;
            project.save_to_root_with_thumbnail_update(root, thumbnail)?;
            None
        } else {
            Some(project.publish_complete_to_with_thumbnail_update(
                &target,
                self.project_root.as_ref(),
                thumbnail,
            )?)
        };

        self.project_dir = Some(target.clone());
        if let Some(root) = new_root {
            self.project_root = Some(root);
        }
        if project.generation_log.is_some() {
            self.generation_log_component_present = true;
        }
        Ok(target)
    }

    /// Route one [`EditCommand`] through the single editing entry point,
    /// delegating the whole snapshot/commit/version transaction to
    /// `opentake-ops`. `Undo`/`Redo` are ordinary commands here (the ops layer
    /// models them as such), so the session needs no separate undo plumbing.
    pub fn apply(&mut self, command: EditCommand, ids: &dyn IdGen) -> Result<EditResult> {
        self.ensure_mutable()?;
        Ok(command::apply(&mut self.state, command, ids)?)
    }

    /// Import a local media file as an external reference and append it to the
    /// manifest. Returns the freshly created [`MediaManifestEntry`].
    ///
    /// Mirrors upstream `addMediaAsset(from:)` + `importMediaAsset` +
    /// `finalizeImportedAsset`: build a [`MediaAsset`] from the file
    /// ([`MediaSource::External`] — the file is referenced in place, not copied
    /// into the bundle), fold in the probed metadata, then derive its persisted
    /// entry and push it onto [`MediaManifest::entries`]. The clip layer only
    /// ever stores the asset id (`media_ref`); the manifest is the bridge from id
    /// to file.
    ///
    /// `id` is the caller-minted asset id, `name` its display name (upstream uses
    /// the file stem). Errors with [`CoreError::Unsupported`]`("media")` when the
    /// extension is not on the import white-list — a recoverable value the
    /// command layer maps to a clear message, never a panic.
    ///
    /// Manifest mutation here is intentionally *outside* the undo transaction:
    /// upstream appends imports to the manifest directly (only folder moves, which
    /// go through [`Self::apply`], are undoable). Importing does not bump the
    /// timeline version.
    pub fn import_media_file(
        &mut self,
        path: impl AsRef<Path>,
        id: impl Into<String>,
        name: impl Into<String>,
        probe: &ProbedMedia,
    ) -> Result<MediaManifestEntry> {
        self.import_media_file_checked(path, id, name, probe, || Ok(()))
    }

    /// Import a ready vocals/accompaniment file through the shared media path
    /// and attach durable provenance. The original asset remains immutable.
    pub fn import_derived_stem_file(
        &mut self,
        path: impl AsRef<Path>,
        id: impl Into<String>,
        name: impl Into<String>,
        probe: &ProbedMedia,
        provenance: DerivedStemProvenance,
    ) -> Result<MediaManifestEntry> {
        self.ensure_mutable()?;
        let source = self
            .state
            .manifest
            .entries
            .iter()
            .find(|entry| entry.id == provenance.source_asset_id)
            .ok_or_else(|| {
                CoreError::Media(format!(
                    "stem source asset does not exist: {}",
                    provenance.source_asset_id
                ))
            })?;
        if !matches!(source.kind, ClipType::Audio | ClipType::Video)
            || !source.has_audio.unwrap_or(source.kind == ClipType::Audio)
        {
            return Err(CoreError::Media(
                "stem source asset has no audio".to_string(),
            ));
        }
        if !valid_sha256(&provenance.source_sha256)
            || provenance
                .model_sha256
                .as_deref()
                .is_some_and(|digest| !valid_sha256(digest))
        {
            return Err(CoreError::Media(
                "stem provenance checksum is invalid".to_string(),
            ));
        }
        let (provider, model) = provenance.execution.split_once(':').ok_or_else(|| {
            CoreError::Media("stem execution must be '<provider>:<model>'".to_string())
        })?;
        if !safe_provider_prefix(provider) || model.trim().is_empty() {
            return Err(CoreError::Media(
                "stem execution provider or model is invalid".to_string(),
            ));
        }
        let output_index = match provenance.stem.as_str() {
            "vocals" => 0,
            "accompaniment" => 1,
            _ => {
                return Err(CoreError::Media(
                    "stem kind must be vocals or accompaniment".to_string(),
                ))
            }
        };

        let before = self.state.manifest.clone();
        let result = (|| {
            let entry = self.import_media_file(path, id, name, probe)?;
            let target = self
                .state
                .manifest
                .entries
                .iter_mut()
                .find(|candidate| candidate.id == entry.id)
                .ok_or_else(|| CoreError::Media("imported stem disappeared".to_string()))?;
            target.generation_input = Some(GenerationInput {
                prompt: format!("stem:{}", provenance.stem),
                model: model.to_string(),
                duration: probe.duration_secs.max(0.0).round() as i32,
                aspect_ratio: "audio".to_string(),
                quality: provenance
                    .model_sha256
                    .map(|digest| format!("model-sha256:{digest}")),
                reference_audio_urls: Some(vec![format!("sha256:{}", provenance.source_sha256)]),
                provider: Some(provider.to_string()),
                status: Some(GenerationJobStatus::Ready),
                progress: Some(1.0),
                output_index: Some(output_index),
                source_asset_id: Some(provenance.source_asset_id),
                ..GenerationInput::default()
            });
            Ok(target.clone())
        })();
        if result.is_err() {
            self.state.manifest = before;
        }
        result
    }

    /// Import one file and roll the manifest back if `postcondition` fails.
    /// Save-as-media uses this to bind its final retained-file identity check to
    /// the live manifest mutation: an attacker-triggered swap can never leave a
    /// dangling entry behind after the command reports failure.
    pub fn import_media_file_checked(
        &mut self,
        path: impl AsRef<Path>,
        id: impl Into<String>,
        name: impl Into<String>,
        probe: &ProbedMedia,
        postcondition: impl FnOnce() -> Result<()>,
    ) -> Result<MediaManifestEntry> {
        self.ensure_mutable()?;
        let manifest_before = self.state.manifest.clone();
        let path = path.as_ref();
        let entry = self.prepare_media_file_entry(path, id, name, probe)?;
        // Dedup (#91 "素材重复出现"): importing a file that is already in the
        // manifest reuses the existing entry — keeping its id so any clip that
        // references it stays valid — instead of appending a second entry for the
        // same source. `source` is the resolved path (external abs or project
        // relative), identical for the same file under the same project.
        let entry = if let Some(existing) = self
            .state
            .manifest
            .entries
            .iter()
            .find(|e| e.source == entry.source)
        {
            existing.clone()
        } else {
            self.state.manifest.entries.push(entry.clone());
            entry
        };
        if let Err(error) = postcondition() {
            self.state.manifest = manifest_before;
            return Err(error);
        }
        Ok(entry)
    }

    /// Build the manifest representation of a validated local file without
    /// mutating the manifest, history, or version. Deferred render workflows
    /// use this to prepare a command that will later register the entry and
    /// edit the timeline in one revision-bound transaction.
    pub fn prepare_media_file_entry(
        &self,
        path: impl AsRef<Path>,
        id: impl Into<String>,
        name: impl Into<String>,
        probe: &ProbedMedia,
    ) -> Result<MediaManifestEntry> {
        self.ensure_mutable()?;
        let path = path.as_ref();
        let kind = importable_clip_type(path).ok_or(CoreError::Unsupported("media"))?;

        let mut asset = MediaAsset::new(id, path, kind, name, probe.duration_secs);
        asset.source_width = probe.width;
        asset.source_height = probe.height;
        asset.source_fps = probe.fps;
        asset.color = probe.color.clone();
        // Video defaults to having audio (MediaAsset::new); refine from the probe.
        // Non-video never carries a video-track-linked audio flag upstream.
        asset.has_audio = match kind {
            ClipType::Audio => true,
            ClipType::Video => probe.has_audio,
            _ => false,
        };

        // `now = 0`: a freshly prepared local file has no cached remote URL, so
        // the freshness clock is irrelevant to the produced entry.
        Ok(asset.to_manifest_entry(self.project_dir.as_deref(), 0.0))
    }

    /// Relink an existing asset to a new on-disk file, **keeping the same id** so
    /// every clip that references it recovers in place (mirrors upstream
    /// `EditorViewModel+Relink.applyRelink`: same asset, swapped url + refreshed
    /// metadata). The new file's type must match the original's `kind`
    /// (`CoreError::Media` on mismatch — upstream rejects a type change), and the
    /// id must exist. Re-importing instead would mint a NEW id, orphaning the old
    /// clips on the missing entry forever — which is the bug this fixes.
    pub fn relink_media_file(
        &mut self,
        asset_id: &str,
        path: impl AsRef<Path>,
        probe: &ProbedMedia,
    ) -> Result<MediaManifestEntry> {
        self.ensure_mutable()?;
        let path = path.as_ref();
        let kind = importable_clip_type(path).ok_or(CoreError::Unsupported("media"))?;
        let entry = self
            .state
            .manifest
            .entries
            .iter_mut()
            .find(|e| e.id == asset_id)
            .ok_or_else(|| CoreError::Media(format!("unknown media asset: {asset_id}")))?;
        if entry.kind != kind {
            return Err(CoreError::Media(format!(
                "cannot relink a {:?} asset to a {:?} file",
                entry.kind, kind
            )));
        }
        // Same id; only the source path + probed metadata change. The `missing`
        // state the panel derives from file existence clears automatically once
        // the source points at a real file again.
        entry.source = opentake_domain::MediaSource::External {
            absolute_path: path.to_string_lossy().into_owned(),
        };
        entry.duration = probe.duration_secs;
        entry.source_width = probe.width;
        entry.source_height = probe.height;
        entry.source_fps = probe.fps;
        entry.color = probe.color.clone();
        entry.proxy = None;
        entry.has_audio = Some(match kind {
            ClipType::Audio => true,
            ClipType::Video => probe.has_audio,
            _ => false,
        });
        Ok(entry.clone())
    }

    /// Toggle favorite state for `asset_ids` (#91). Like import, this is a media
    /// manifest mutation *outside* the undo transaction — favoriting is a library
    /// action, not a timeline edit, so it never enters undo and leaves the
    /// timeline version untouched. Unknown ids are ignored. Returns the number of
    /// ids whose favorite state actually changed.
    pub fn set_media_favorite(&mut self, asset_ids: &[String], favorite: bool) -> Result<usize> {
        self.ensure_mutable()?;
        Ok(self.state.manifest.set_favorites(asset_ids, favorite))
    }

    /// Attach or clear a project-local playback proxy without changing the
    /// authoritative source used by export. The proxy path is deliberately
    /// constrained to `media/proxies/` and the source digest is fixed-width so
    /// corrupt or externally-authored manifests cannot redirect playback.
    pub fn set_media_proxy(
        &mut self,
        asset_id: &str,
        proxy: Option<MediaProxy>,
    ) -> Result<MediaManifestEntry> {
        self.ensure_mutable()?;
        if let Some(proxy) = proxy.as_ref() {
            let path = Path::new(&proxy.relative_path);
            let components: Vec<_> = path.components().collect();
            if path.is_absolute()
                || components.len() != 3
                || components[0] != std::path::Component::Normal("media".as_ref())
                || components[1] != std::path::Component::Normal("proxies".as_ref())
                || !matches!(components[2], std::path::Component::Normal(_))
                || path.extension().and_then(|extension| extension.to_str()) != Some("mp4")
                || proxy.width == 0
                || proxy.height == 0
                || proxy.source_sha256.len() != 64
                || !proxy
                    .source_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(CoreError::Media("invalid media proxy metadata".to_string()));
            }
        }
        let entry = self
            .state
            .manifest
            .entries
            .iter_mut()
            .find(|entry| entry.id == asset_id)
            .ok_or_else(|| CoreError::Media(format!("unknown media asset: {asset_id}")))?;
        entry.proxy = proxy;
        Ok(entry.clone())
    }

    /// Set or clear one asset's content-addressed global favorite id. This is a
    /// manifest mutation outside undo, matching [`Self::set_media_favorite`].
    pub fn set_media_global_favorite(
        &mut self,
        asset_id: &str,
        library_id: Option<String>,
    ) -> Result<bool> {
        self.ensure_mutable()?;
        Ok(self
            .state
            .manifest
            .set_global_favorite(asset_id, library_id))
    }

    /// Clear every current-project mirror of a removed global-library entry.
    pub fn clear_media_global_favorite_id(&mut self, library_id: &str) -> Result<usize> {
        self.ensure_mutable()?;
        Ok(self.state.manifest.clear_global_favorite_id(library_id))
    }

    /// A clone of the current media manifest (read-only mirror for the media
    /// panel). The manifest is the persisted id→file catalog.
    pub fn media(&self) -> MediaManifest {
        self.state.manifest.clone()
    }

    /// Restore a previously captured manifest after an application-layer
    /// transaction fails. Kept crate-private so ordinary callers cannot bypass
    /// the command/session invariants.
    pub(crate) fn restore_media(&mut self, manifest: MediaManifest) {
        self.state.manifest = manifest;
    }

    /// Capture the complete undoable document state before an application-layer
    /// batch transaction. The matching restore is intentionally crate-private:
    /// only [`crate::AppCore`] may use it while holding the authoritative
    /// session lock.
    pub(crate) fn checkpoint_editor_state(&self) -> EditorState {
        self.state.clone()
    }

    /// Restore a failed application-layer batch exactly, including manifest,
    /// undo/redo history, and version.
    pub(crate) fn restore_editor_state(&mut self, state: EditorState) {
        self.state = state;
    }

    /// The manifest entry for `asset_id`, if present (lookup without cloning the
    /// whole manifest).
    pub fn media_entry(&self, asset_id: &str) -> Option<&MediaManifestEntry> {
        self.state
            .manifest
            .entries
            .iter()
            .find(|e| e.id == asset_id)
    }

    /// The current monotonic document version (sourced from `EditorState`, not a
    /// duplicate counter): bumps on every committing edit and every undo/redo.
    pub fn version(&self) -> u64 {
        self.state.version()
    }

    /// A clone of the current timeline (for read-only mirror snapshots).
    pub fn timeline(&self) -> Timeline {
        self.state.timeline.clone()
    }

    /// Whether an undo is available.
    pub fn can_undo(&self) -> bool {
        self.state.can_undo()
    }

    /// Label of the current top-level undo transaction.
    pub fn undo_action_name(&self) -> Option<&str> {
        self.state.undo_action_name()
    }

    /// Stable version identity of the undo transaction at the top of history.
    pub fn undo_transaction_version(&self) -> Option<u64> {
        self.state.undo_transaction_version()
    }

    /// Whether a redo is available.
    pub fn can_redo(&self) -> bool {
        self.state.can_redo()
    }

    /// The current bundle path, if the project has one.
    pub fn project_dir(&self) -> Option<&Path> {
        self.project_dir.as_deref()
    }

    /// Open a project-local asset through the retained no-follow bundle
    /// authority. Reads never re-resolve [`Self::project_dir`], so an ambient
    /// rename or replacement of the bundle pathname cannot redirect the open
    /// to a rebound bundle. The returned file is read-only and opened with
    /// no-follow semantics on every directory component and the final leaf.
    pub fn open_asset_file(&self, relative: &Path) -> Result<fs::File> {
        let root = self.project_root.as_ref().ok_or(CoreError::NoProjectOpen)?;
        Ok(root.open_asset_file(relative)?)
    }

    /// Compare a caller's retained no-follow bundle handle with the handle
    /// retained when this exact session opened or saved the project.
    pub(crate) fn matches_project_root_identity(&self, current: &Handle) -> Result<bool> {
        self.ensure_mutable()?;
        let expected = self.project_root.as_ref().ok_or(CoreError::NoProjectOpen)?;
        Ok(expected.matches_identity(current))
    }

    pub(crate) fn project_root_identity(&self) -> Option<ProjectRootIdentity> {
        self.project_root.as_ref().map(ProjectRoot::stable_identity)
    }

    pub(crate) fn project_root_is_current_namespace(&self) -> Result<bool> {
        self.project_root
            .as_ref()
            .ok_or(CoreError::NoProjectOpen)?
            .is_current_namespace()
            .map_err(CoreError::from)
    }

    /// Read-only access to the generation log.
    pub fn generation_log(&self) -> &GenerationLog {
        &self.generation_log
    }

    /// Create durable manifest placeholders and append the corresponding
    /// queued audit events. The caller persists the returned in-memory state in
    /// the same critical section before exposing the placeholder ids.
    pub(crate) fn begin_generation_job(
        &mut self,
        mut plan: PreparedGenerationJob,
        ids: &dyn IdGen,
    ) -> Result<GenerationJobCommit> {
        self.ensure_mutable()?;
        if self.project_dir.is_none() {
            return Err(CoreError::NoProjectOpen);
        }
        if !(1..=4).contains(&plan.output_count) {
            return Err(CoreError::Media(
                "generation output count must be between 1 and 4".to_string(),
            ));
        }
        if plan.input.model.trim().is_empty() || plan.provider.trim().is_empty() {
            return Err(CoreError::Media(
                "generation model and provider are required".to_string(),
            ));
        }
        if !safe_provider_prefix(&plan.provider) {
            return Err(CoreError::Media(
                "generation provider prefix is invalid".to_string(),
            ));
        }
        if let Some(folder_id) = plan.folder_id.as_deref() {
            if !self
                .state
                .manifest
                .folders
                .iter()
                .any(|folder| folder.id == folder_id)
            {
                return Err(CoreError::Media(format!(
                    "generation folder does not exist: {folder_id}"
                )));
            }
        }
        if let Some(source_asset_id) = plan.source_asset_id.as_deref() {
            if !self
                .state
                .manifest
                .entries
                .iter()
                .any(|entry| entry.id == source_asset_id)
            {
                return Err(CoreError::Media(format!(
                    "generation source asset does not exist: {source_asset_id}"
                )));
            }
        }

        let job_id = ids.next_id();
        plan.input.job_id = Some(job_id.clone());
        plan.input.provider = Some(plan.provider.clone());
        plan.input.provider_job_id = None;
        plan.input.status = Some(GenerationJobStatus::Queued);
        plan.input.progress = Some(0.0);
        plan.input.error_code = None;
        plan.input.source_asset_id = plan.source_asset_id.clone();
        plan.input.source_clip_id = plan.source_clip_id.clone();
        plan.input.estimated_cost_credits = plan.estimated_cost_credits;
        plan.input.created_at = plan.created_at;

        let mut placeholder_asset_ids = Vec::with_capacity(plan.output_count);
        for output_index in 0..plan.output_count {
            let asset_id = ids.next_id();
            let mut input = plan.input.clone();
            input.output_index = Some(output_index);
            let display_name = if plan.output_count == 1 {
                plan.name.clone()
            } else {
                format!("{} {}", plan.name, output_index + 1)
            };
            self.state.manifest.entries.push(MediaManifestEntry {
                id: asset_id.clone(),
                name: display_name,
                kind: plan.kind,
                source: MediaSource::Project {
                    relative_path: format!("media/{asset_id}.pending"),
                },
                duration: (input.duration.max(0)) as f64,
                generation_input: Some(input.clone()),
                source_width: None,
                source_height: None,
                source_fps: None,
                has_audio: Some(
                    plan.kind == ClipType::Audio
                        || (plan.kind == ClipType::Video && input.generate_audio.unwrap_or(true)),
                ),
                color: None,
                proxy: None,
                folder_id: plan.folder_id.clone(),
                cached_remote_url: None,
                cached_remote_url_expires_at: None,
            });
            self.generation_log
                .entries
                .push(GenerationLogEntry::job_event(
                    ids.next_id(),
                    job_id.clone(),
                    input.model.clone(),
                    None,
                    plan.provider.clone(),
                    None,
                    asset_id.clone(),
                    GenerationJobStatus::Queued,
                    Some(0.0),
                    None,
                    plan.created_at,
                    plan.source_asset_id.clone(),
                    plan.source_clip_id.clone(),
                ));
            placeholder_asset_ids.push(asset_id);
        }
        self.generation_log_component_present = true;
        Ok(GenerationJobCommit {
            job_id,
            placeholder_asset_ids,
        })
    }

    pub(crate) fn update_generation_job(
        &mut self,
        job_id: &str,
        update: GenerationStateUpdate,
        ids: &dyn IdGen,
    ) -> Result<usize> {
        self.ensure_mutable()?;
        validate_generation_update(&update)?;
        let mut events = Vec::new();
        for entry in &mut self.state.manifest.entries {
            let Some(input) = entry.generation_input.as_mut() else {
                continue;
            };
            if input.job_id.as_deref() != Some(job_id) {
                continue;
            }
            if !valid_generation_transition(input.status, update.status) {
                return Err(CoreError::Media(format!(
                    "invalid generation transition from {:?} to {:?}",
                    input.status, update.status
                )));
            }
            input.status = Some(update.status);
            input.progress = update.progress;
            input.error_code = update.error_code.clone();
            if update.provider_job_id.is_some() {
                input.provider_job_id = update.provider_job_id.clone();
            }
            events.push((entry.id.clone(), input.clone()));
        }
        if events.is_empty() {
            return Err(CoreError::Media(format!(
                "generation job does not exist: {job_id}"
            )));
        }
        for (event_index, (asset_id, input)) in events.iter().enumerate() {
            self.append_generation_event(
                ids,
                asset_id,
                input,
                update.status,
                update.progress,
                update.error_code.clone(),
                (event_index == 0).then_some(update.cost_credits).flatten(),
                update.created_at,
            );
        }
        Ok(events.len())
    }

    pub(crate) fn finalize_generation_output(
        &mut self,
        output: PreparedGenerationOutput,
        ids: &dyn IdGen,
    ) -> Result<()> {
        self.ensure_mutable()?;
        validate_project_media_relative_path(&output.relative_path)?;
        let entry = self
            .state
            .manifest
            .entries
            .iter_mut()
            .find(|entry| entry.id == output.asset_id)
            .ok_or_else(|| {
                CoreError::Media(format!(
                    "generation placeholder does not exist: {}",
                    output.asset_id
                ))
            })?;
        let input = entry.generation_input.as_mut().ok_or_else(|| {
            CoreError::Media("generation placeholder has no provenance".to_string())
        })?;
        if !valid_generation_transition(input.status, GenerationJobStatus::Ready) {
            return Err(CoreError::Media(format!(
                "generation placeholder cannot finalize from {:?}",
                input.status
            )));
        }
        entry.source = MediaSource::Project {
            relative_path: output.relative_path,
        };
        entry.duration = output.probe.duration_secs;
        entry.source_width = output.probe.width;
        entry.source_height = output.probe.height;
        entry.source_fps = output.probe.fps;
        entry.has_audio = Some(output.probe.has_audio);
        entry.color = output.probe.color.clone();
        entry.proxy = None;
        input.status = Some(GenerationJobStatus::Ready);
        input.progress = Some(1.0);
        input.error_code = None;
        let asset_id = entry.id.clone();
        let input = input.clone();
        self.append_generation_event(
            ids,
            &asset_id,
            &input,
            GenerationJobStatus::Ready,
            Some(1.0),
            None,
            None,
            output.created_at,
        );
        Ok(())
    }

    pub(crate) fn fail_generation_output(
        &mut self,
        asset_id: &str,
        error_code: &str,
        created_at: Option<f64>,
        ids: &dyn IdGen,
    ) -> Result<()> {
        if !safe_generation_error_code(error_code) {
            return Err(CoreError::Media(
                "generation failure code is invalid".to_string(),
            ));
        }
        let entry = self
            .state
            .manifest
            .entries
            .iter_mut()
            .find(|entry| entry.id == asset_id)
            .ok_or_else(|| {
                CoreError::Media(format!("generation placeholder does not exist: {asset_id}"))
            })?;
        let input = entry.generation_input.as_mut().ok_or_else(|| {
            CoreError::Media("generation placeholder has no provenance".to_string())
        })?;
        if matches!(input.status, Some(GenerationJobStatus::Ready)) {
            return Ok(());
        }
        input.status = Some(GenerationJobStatus::Failed);
        input.progress = None;
        input.error_code = Some(error_code.to_string());
        let input = input.clone();
        self.append_generation_event(
            ids,
            asset_id,
            &input,
            GenerationJobStatus::Failed,
            None,
            Some(error_code.to_string()),
            None,
            created_at,
        );
        Ok(())
    }

    pub(crate) fn cancel_generation_output(
        &mut self,
        asset_id: &str,
        created_at: Option<f64>,
        ids: &dyn IdGen,
    ) -> Result<()> {
        let entry = self
            .state
            .manifest
            .entries
            .iter_mut()
            .find(|entry| entry.id == asset_id)
            .ok_or_else(|| {
                CoreError::Media(format!("generation placeholder does not exist: {asset_id}"))
            })?;
        let input = entry.generation_input.as_mut().ok_or_else(|| {
            CoreError::Media("generation placeholder has no provenance".to_string())
        })?;
        if matches!(
            input.status,
            Some(
                GenerationJobStatus::Ready
                    | GenerationJobStatus::Failed
                    | GenerationJobStatus::Cancelled
            )
        ) {
            return Ok(());
        }
        input.status = Some(GenerationJobStatus::Cancelled);
        input.progress = None;
        input.error_code = None;
        let input = input.clone();
        self.append_generation_event(
            ids,
            asset_id,
            &input,
            GenerationJobStatus::Cancelled,
            None,
            None,
            None,
            created_at,
        );
        Ok(())
    }

    pub(crate) fn save_generation_state(&mut self) -> Result<PathBuf> {
        self.ensure_mutable()?;
        let target = self.project_dir.clone().ok_or(CoreError::NoProjectOpen)?;
        let mut project =
            Project::new_with_compatibility(target.clone(), self.compatibility.clone());
        project.timeline = self.state.timeline.clone();
        project.manifest = self.state.manifest.clone();
        project.generation_log = Some(self.generation_log.clone());
        // Generation spans media.json + generation-log.json. Publish a complete
        // sibling bundle so both become visible at one rename commit point.
        // The source root carries media/chat/thumbnail into the fresh stage.
        let source_root = self.project_root.take().ok_or(CoreError::NoProjectOpen)?;
        let new_root = match project.publish_complete_replacing_root(&target, source_root) {
            Ok(root) => root,
            Err(error) => {
                // A pre-commit failure restores the original target; recover
                // retained authority when possible while preserving the exact
                // publication error for the caller. Post-commit ambiguity stays
                // fail-closed if the target cannot be reopened.
                self.project_root = ProjectRoot::open(&target).ok();
                return Err(error.into());
            }
        };
        self.project_root = Some(new_root);
        self.generation_log_component_present = true;
        Ok(target)
    }

    pub(crate) fn save_generation_state_with_media(
        &mut self,
        media_leaf: &str,
        media_byte_size: u64,
        media: &mut dyn std::io::Read,
    ) -> Result<PathBuf> {
        self.ensure_mutable()?;
        let target = self.project_dir.clone().ok_or(CoreError::NoProjectOpen)?;
        let mut project =
            Project::new_with_compatibility(target.clone(), self.compatibility.clone());
        project.timeline = self.state.timeline.clone();
        project.manifest = self.state.manifest.clone();
        project.generation_log = Some(self.generation_log.clone());
        let source_root = self.project_root.take().ok_or(CoreError::NoProjectOpen)?;
        let new_root = match project.publish_complete_replacing_root_with_media(
            &target,
            source_root,
            media_leaf,
            media_byte_size,
            media,
        ) {
            Ok(root) => root,
            Err(error) => {
                self.project_root = ProjectRoot::open(&target).ok();
                return Err(error.into());
            }
        };
        self.project_root = Some(new_root);
        self.generation_log_component_present = true;
        Ok(target)
    }

    pub(crate) fn checkpoint_generation_state(&self) -> GenerationStateCheckpoint {
        GenerationStateCheckpoint {
            manifest: self.state.manifest.clone(),
            log: self.generation_log.clone(),
            component_present: self.generation_log_component_present,
        }
    }

    pub(crate) fn restore_generation_state(&mut self, checkpoint: GenerationStateCheckpoint) {
        self.state.manifest = checkpoint.manifest;
        self.generation_log = checkpoint.log;
        self.generation_log_component_present = checkpoint.component_present;
    }

    #[allow(clippy::too_many_arguments)]
    fn append_generation_event(
        &mut self,
        ids: &dyn IdGen,
        asset_id: &str,
        input: &GenerationInput,
        status: GenerationJobStatus,
        progress: Option<f64>,
        error_code: Option<String>,
        cost_credits: Option<i64>,
        created_at: Option<f64>,
    ) {
        self.generation_log
            .entries
            .push(GenerationLogEntry::job_event(
                ids.next_id(),
                input.job_id.clone().unwrap_or_default(),
                input.model.clone(),
                cost_credits,
                input.provider.clone().unwrap_or_default(),
                input.provider_job_id.clone(),
                asset_id.to_string(),
                status,
                progress,
                error_code,
                created_at,
                input.source_asset_id.clone(),
                input.source_clip_id.clone(),
            ));
        self.generation_log_component_present = true;
    }

    /// Compatibility state inherited from the opened project.
    pub fn compatibility(&self) -> &ProjectCompatibility {
        &self.compatibility
    }

    pub(crate) fn ensure_mutable(&self) -> Result<()> {
        self.compatibility.ensure_writable()?;
        Ok(())
    }

    /// Test-only seam: reseat the editable state from a prebuilt timeline (empty
    /// manifest, fresh history at version 0). Lets tests stand up a session over
    /// a hand-built timeline without going through disk, while keeping all
    /// production state mutation funneled through [`Self::apply`] /
    /// [`Self::open_project`].
    #[cfg(test)]
    pub(crate) fn seed_from_timeline(&mut self, timeline: Timeline) {
        self.state = EditorState::from_timeline(timeline);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::ClipType;
    use opentake_ops::command::ClipEntry;
    use opentake_ops::SeqIdGen;

    fn one_video_track() -> Timeline {
        use opentake_domain::Track;
        let mut tl = Timeline::new();
        tl.tracks.push(Track::new("t1", ClipType::Video));
        tl
    }

    fn add_one_clip_cmd() -> EditCommand {
        EditCommand::AddClips {
            entries: vec![ClipEntry {
                media_ref: "asset-1".into(),
                media_type: ClipType::Video,
                source_clip_type: ClipType::Video,
                track_index: 0,
                start_frame: 0,
                duration_frames: 30,
                trim_start_frame: None,
                trim_end_frame: None,
                has_audio: false,
                add_linked_audio: false,
                transform: None,
            }],
        }
    }

    #[test]
    fn new_project_starts_empty_at_version_zero() {
        let s = EditorSession::new_project();
        assert_eq!(s.version(), 0);
        assert!(!s.can_undo());
        assert!(!s.can_redo());
        assert!(s.project_dir().is_none());
        assert!(s.timeline().tracks.is_empty());
    }

    #[test]
    fn save_without_path_or_dir_errors() {
        let mut s = EditorSession::new_project();
        assert!(matches!(
            s.save_project(None),
            Err(CoreError::NoProjectOpen)
        ));
    }

    #[test]
    fn new_save_open_roundtrip_preserves_timeline() {
        let dir = std::env::temp_dir().join(format!(
            "opentake-core-session-{}-{}.opentake",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        // New project with one edit applied.
        let mut s = EditorSession::new_project();
        s.state = EditorState::from_timeline(one_video_track());
        let ids = SeqIdGen::new("c-");
        let res = s.apply(add_one_clip_cmd(), &ids).unwrap();
        assert!(res.changed);
        let saved_timeline = s.timeline();

        // Save-as to a new dir, then open it back.
        let written = s.save_project(Some(dir.clone())).unwrap();
        assert_eq!(written, dir);
        assert_eq!(s.project_dir(), Some(dir.as_path()));

        let reopened = EditorSession::open_project(&dir).unwrap();
        assert_eq!(reopened.timeline(), saved_timeline);
        // A freshly opened project starts at version 0 with empty history.
        assert_eq!(reopened.version(), 0);
        assert!(!reopened.can_undo());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn same_project_save_uses_the_root_retained_when_the_session_opened() {
        let root = std::env::temp_dir().join(format!(
            "opentake-core-session-retained-save-{}-{}",
            std::process::id(),
            line!()
        ));
        let projects = root.join("projects");
        let retained = root.join("projects-retained");
        let bundle = projects.join("A.opentake");
        let _ = std::fs::remove_dir_all(&root);
        let mut project = Project::new(&bundle);
        project.timeline = one_video_track();
        project.save().unwrap();
        let mut session = EditorSession::open_project(&bundle).unwrap();

        std::fs::rename(&projects, &retained).unwrap();
        Project::new(&bundle).save().unwrap();
        session
            .apply(add_one_clip_cmd(), &SeqIdGen::new("retained-"))
            .unwrap();
        session.save_project(None).unwrap();

        assert!(Project::open(&bundle).unwrap().timeline.tracks.is_empty());
        assert_eq!(
            Project::open(retained.join("A.opentake"))
                .unwrap()
                .timeline
                .tracks[0]
                .clips
                .len(),
            1
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_then_undo_redo_through_session() {
        let mut s = EditorSession::new_project();
        s.state = EditorState::from_timeline(one_video_track());
        let ids = SeqIdGen::new("c-");

        let added = s.apply(add_one_clip_cmd(), &ids).unwrap();
        assert!(added.changed);
        assert_eq!(s.version(), 1);
        assert_eq!(s.timeline().tracks[0].clips.len(), 1);

        let undo = s.apply(EditCommand::Undo, &ids).unwrap();
        assert!(undo.changed);
        assert_eq!(s.version(), 2);
        assert_eq!(s.timeline().tracks[0].clips.len(), 0);

        let redo = s.apply(EditCommand::Redo, &ids).unwrap();
        assert!(redo.changed);
        assert_eq!(s.version(), 3);
        assert_eq!(s.timeline().tracks[0].clips.len(), 1);
    }

    // --- Media import ---

    #[test]
    fn importable_clip_type_covers_whitelist_and_rejects_others() {
        assert_eq!(
            importable_clip_type(Path::new("/x/a.MP4")),
            Some(ClipType::Video)
        );
        assert_eq!(
            importable_clip_type(Path::new("/x/song.m4a")),
            Some(ClipType::Audio)
        );
        assert_eq!(
            importable_clip_type(Path::new("/x/pic.JPG")),
            Some(ClipType::Image)
        );
        assert_eq!(
            importable_clip_type(Path::new("/x/anim.json")),
            Some(ClipType::Lottie)
        );
        assert_eq!(
            importable_clip_type(Path::new("/x/anim.lottie")),
            Some(ClipType::Lottie)
        );
        assert_eq!(importable_clip_type(Path::new("/x/notes.txt")), None);
        assert_eq!(importable_clip_type(Path::new("/x/noext")), None);
    }

    #[test]
    fn importable_clip_type_maps_every_whitelisted_extension() {
        // Each list must map to exactly its ClipType, case-insensitively, so a
        // new extension can never silently fall through to `None`.
        for ext in SUPPORTED_VIDEO_EXTENSIONS {
            let p = format!("/x/clip.{ext}");
            assert_eq!(
                importable_clip_type(Path::new(&p)),
                Some(ClipType::Video),
                "video ext .{ext} should import as Video"
            );
            // Same extension upper-cased still maps (extension is lowercased).
            let up = format!("/x/clip.{}", ext.to_ascii_uppercase());
            assert_eq!(importable_clip_type(Path::new(&up)), Some(ClipType::Video));
        }
        for ext in SUPPORTED_AUDIO_EXTENSIONS {
            let p = format!("/x/song.{ext}");
            assert_eq!(
                importable_clip_type(Path::new(&p)),
                Some(ClipType::Audio),
                "audio ext .{ext} should import as Audio"
            );
        }
        for ext in SUPPORTED_IMAGE_EXTENSIONS {
            let p = format!("/x/pic.{ext}");
            assert_eq!(
                importable_clip_type(Path::new(&p)),
                Some(ClipType::Image),
                "image ext .{ext} should import as Image"
            );
        }
    }

    #[test]
    fn importable_clip_type_covers_newly_added_extensions() {
        // Spot-check a representative newcomer from each widened list plus junk.
        assert_eq!(
            importable_clip_type(Path::new("/x/a.mkv")),
            Some(ClipType::Video)
        );
        assert_eq!(
            importable_clip_type(Path::new("/x/a.webm")),
            Some(ClipType::Video)
        );
        assert_eq!(
            importable_clip_type(Path::new("/x/s.flac")),
            Some(ClipType::Audio)
        );
        assert_eq!(
            importable_clip_type(Path::new("/x/s.opus")),
            Some(ClipType::Audio)
        );
        assert_eq!(
            importable_clip_type(Path::new("/x/p.gif")),
            Some(ClipType::Image)
        );
        assert_eq!(
            importable_clip_type(Path::new("/x/p.avif")),
            Some(ClipType::Image)
        );
        // Junk / documents still rejected.
        assert_eq!(importable_clip_type(Path::new("/x/a.pdf")), None);
        assert_eq!(importable_clip_type(Path::new("/x/a.exe")), None);
        assert_eq!(importable_clip_type(Path::new("/x/a.doc")), None);
    }

    #[test]
    fn import_video_builds_external_entry_with_probe_metadata() {
        let mut s = EditorSession::new_project();
        let probe = ProbedMedia {
            duration_secs: 12.5,
            width: Some(1920),
            height: Some(1080),
            fps: Some(30.0),
            has_audio: true,
            color: None,
        };
        let entry = s
            .import_media_file("/abs/clip.mp4", "asset-1", "clip", &probe)
            .unwrap();

        assert_eq!(entry.id, "asset-1");
        assert_eq!(entry.name, "clip");
        assert_eq!(entry.kind, ClipType::Video);
        assert_eq!(entry.duration, 12.5);
        assert_eq!(entry.source_width, Some(1920));
        assert_eq!(entry.source_height, Some(1080));
        assert_eq!(entry.source_fps, Some(30.0));
        assert_eq!(entry.has_audio, Some(true));
        // Unsaved project + absolute path outside any bundle -> External ref.
        assert_eq!(
            entry.source,
            opentake_domain::MediaSource::External {
                absolute_path: "/abs/clip.mp4".into()
            }
        );

        // Appended to the manifest, queryable by id; importing leaves the
        // timeline version untouched.
        assert_eq!(s.media().entries.len(), 1);
        assert_eq!(
            s.media_entry("asset-1").map(|e| e.id.as_str()),
            Some("asset-1")
        );
        assert_eq!(s.version(), 0);
    }

    #[test]
    fn derived_stem_import_reuses_media_path_and_persists_provenance() {
        let mut session = EditorSession::new_project();
        let source_probe = ProbedMedia {
            duration_secs: 5.0,
            has_audio: true,
            ..ProbedMedia::default()
        };
        session
            .import_media_file("/abs/source.wav", "source-asset", "Source", &source_probe)
            .unwrap();
        let stem = session
            .import_derived_stem_file(
                "/abs/source-vocals.wav",
                "stem-asset",
                "Source Vocals",
                &source_probe,
                DerivedStemProvenance {
                    source_asset_id: "source-asset".into(),
                    source_sha256: "a".repeat(64),
                    execution: "local:opentake-center-v1".into(),
                    model_sha256: Some("b".repeat(64)),
                    stem: "vocals".into(),
                },
            )
            .unwrap();
        let provenance = stem.generation_input.expect("derived provenance");
        assert_eq!(provenance.prompt, "stem:vocals");
        assert_eq!(provenance.provider.as_deref(), Some("local"));
        assert_eq!(provenance.model, "opentake-center-v1");
        assert_eq!(provenance.source_asset_id.as_deref(), Some("source-asset"));
        assert_eq!(
            provenance.reference_audio_urls,
            Some(vec![format!("sha256:{}", "a".repeat(64))])
        );
        assert_eq!(
            provenance.quality,
            Some(format!("model-sha256:{}", "b".repeat(64)))
        );
        assert_eq!(provenance.status, Some(GenerationJobStatus::Ready));
    }

    #[test]
    fn reimporting_the_same_file_reuses_the_entry_instead_of_duplicating() {
        // #91: importing a file already in the manifest must not append a second
        // entry (the "素材重复出现" bug). The existing entry — id and all — is
        // reused, so clips that reference it stay valid and the panel shows it once.
        let mut s = EditorSession::new_project();
        let probe = ProbedMedia {
            duration_secs: 5.0,
            width: Some(640),
            height: Some(480),
            fps: Some(24.0),
            has_audio: true,
            color: None,
        };
        let first = s
            .import_media_file("/abs/clip.mp4", "asset-1", "clip", &probe)
            .unwrap();
        // Same on-disk file, different caller-minted id + display name.
        let second = s
            .import_media_file("/abs/clip.mp4", "asset-2", "clip-again", &probe)
            .unwrap();

        assert_eq!(s.media().entries.len(), 1, "re-import must not duplicate");
        // The reused entry keeps the ORIGINAL id (not the second caller's id).
        assert_eq!(second.id, "asset-1");
        assert_eq!(second.source, first.source);
    }

    #[test]
    fn media_proxy_metadata_is_confined_and_never_replaces_source() {
        let mut session = EditorSession::new_project();
        let source = "/abs/source.mp4";
        let entry = session
            .import_media_file(source, "asset", "source", &ProbedMedia::default())
            .unwrap();
        let original_source = entry.source;
        let proxy = MediaProxy {
            relative_path: "media/proxies/asset.mp4".into(),
            source_sha256: "a".repeat(64),
            width: 640,
            height: 360,
        };
        let updated = session
            .set_media_proxy("asset", Some(proxy.clone()))
            .unwrap();
        assert_eq!(updated.source, original_source);
        assert_eq!(updated.proxy, Some(proxy));

        assert!(session
            .set_media_proxy(
                "asset",
                Some(MediaProxy {
                    relative_path: "../outside.mp4".into(),
                    source_sha256: "a".repeat(64),
                    width: 640,
                    height: 360,
                }),
            )
            .is_err());
        assert!(session
            .set_media_proxy(
                "asset",
                Some(MediaProxy {
                    relative_path: "media/proxies/nested/asset.mp4".into(),
                    source_sha256: "a".repeat(64),
                    width: 640,
                    height: 360,
                }),
            )
            .is_err());
        assert!(session
            .set_media_proxy("asset", None)
            .unwrap()
            .proxy
            .is_none());
    }

    #[test]
    fn import_image_has_no_audio_regardless_of_probe() {
        let mut s = EditorSession::new_project();
        let probe = ProbedMedia {
            duration_secs: 0.0,
            width: Some(800),
            height: Some(600),
            fps: None,
            has_audio: true, // probe lies; an image never has audio
            color: None,
        };
        let entry = s
            .import_media_file("/abs/pic.png", "img-1", "pic", &probe)
            .unwrap();
        assert_eq!(entry.kind, ClipType::Image);
        assert_eq!(entry.has_audio, Some(false));
    }

    #[test]
    fn import_audio_marks_has_audio_true() {
        let mut s = EditorSession::new_project();
        let entry = s
            .import_media_file("/abs/song.mp3", "aud-1", "song", &ProbedMedia::default())
            .unwrap();
        assert_eq!(entry.kind, ClipType::Audio);
        assert_eq!(entry.has_audio, Some(true));
    }

    #[test]
    fn import_unsupported_extension_errors_without_touching_manifest() {
        let mut s = EditorSession::new_project();
        let err = s.import_media_file("/abs/doc.txt", "x", "doc", &ProbedMedia::default());
        assert!(matches!(err, Err(CoreError::Unsupported("media"))));
        assert!(s.media().entries.is_empty());
    }

    #[test]
    fn global_favorite_interfaces_keep_project_mirrors_in_sync() {
        let mut session = EditorSession::new_project();
        session
            .import_media_file("/abs/clip.mp4", "asset-1", "clip", &ProbedMedia::default())
            .unwrap();

        assert!(session
            .set_media_global_favorite("asset-1", Some("content-hash".into()))
            .unwrap());
        assert_eq!(
            session.media().library_favorite_id("asset-1"),
            Some("content-hash")
        );
        assert_eq!(
            session
                .clear_media_global_favorite_id("content-hash")
                .unwrap(),
            1
        );
        assert!(!session.media().is_favorite("asset-1"));
    }

    // --- Save-as copies the project-internal media/ directory (Item 1) ---

    /// A per-call-unique scratch dir under the system temp dir, removed on drop.
    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir()
                .join(format!("opentake-saveas-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A project-internal (`.project`) manifest entry pointing at
    /// `media/<file>`, plus the actual file written under the source bundle's
    /// `media/` dir — the setup a project with internal media has on disk.
    fn seed_bundle_with_internal_media(
        bundle: &Path,
        file_name: &str,
        bytes: &[u8],
    ) -> EditorSession {
        use opentake_domain::{MediaManifestEntry, MediaSource};
        let media_dir = bundle.join("media");
        std::fs::create_dir_all(&media_dir).unwrap();
        std::fs::write(media_dir.join(file_name), bytes).unwrap();

        let mut project = Project::new(bundle.to_path_buf());
        project.manifest.entries.push(MediaManifestEntry {
            id: "asset-1".into(),
            name: file_name.into(),
            kind: ClipType::Image,
            source: MediaSource::Project {
                relative_path: format!("media/{file_name}"),
            },
            duration: 0.0,
            generation_input: None,
            source_width: Some(2),
            source_height: Some(2),
            source_fps: None,
            has_audio: None,
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        project.save().unwrap();

        EditorSession::open_project(bundle).unwrap()
    }

    #[test]
    fn save_as_copies_internal_media_to_new_bundle_and_manifest_resolves() {
        use opentake_domain::{MediaResolver, MediaSource};
        let tmp = TmpDir::new("copy");
        let src = tmp.path().join("Source.opentake");
        let dst = tmp.path().join("Dest.opentake");

        let payload = b"PNGDATA";
        let mut s = seed_bundle_with_internal_media(&src, "still.png", payload);
        // Sanity: the session opened against the source bundle.
        assert_eq!(s.project_dir(), Some(src.as_path()));

        // Save-as to a brand-new directory.
        let written = s.save_project(Some(dst.clone())).unwrap();
        assert_eq!(written, dst);
        assert_eq!(s.project_dir(), Some(dst.as_path()));

        // The media file now exists at the SAME relative path inside the new
        // bundle (media/still.png), with identical bytes.
        let copied = dst.join("media").join("still.png");
        assert!(copied.is_file(), "media file missing at {copied:?}");
        assert_eq!(std::fs::read(&copied).unwrap(), payload);

        // The reopened manifest still resolves the entry to the on-disk file in
        // the new bundle (the reference did not dangle).
        let reopened = EditorSession::open_project(&dst).unwrap();
        let manifest = reopened.media();
        let entry = &manifest.entries[0];
        assert!(matches!(
            &entry.source,
            MediaSource::Project { relative_path } if relative_path == "media/still.png"
        ));
        let resolver = MediaResolver::new(&manifest, Some(dst.as_path()));
        let resolved = resolver.expected_path("asset-1").unwrap();
        assert!(
            resolved.is_file(),
            "resolved path not on disk: {resolved:?}"
        );
        assert_eq!(std::fs::read(&resolved).unwrap(), payload);
    }

    #[test]
    fn plain_save_same_path_does_not_touch_media_dir() {
        let tmp = TmpDir::new("samepath");
        let src = tmp.path().join("Same.opentake");
        let mut s = seed_bundle_with_internal_media(&src, "clip.png", b"x");

        // A no-arg save writes back to the same bundle. It must not recurse into
        // or rewrite media/ (bundle.rs::save "never creates or deletes media/",
        // and copy_media_dir short-circuits on source == dest). We assert the
        // existing media file is left exactly as-is.
        let media_file = src.join("media").join("clip.png");
        let before = std::fs::metadata(&media_file).unwrap();
        let written = s.save_project(None).unwrap();
        assert_eq!(written, src);
        // File still present, same length; the dir was not replaced/emptied.
        let after = std::fs::metadata(&media_file).unwrap();
        assert_eq!(before.len(), after.len());
        assert!(media_file.is_file());
    }

    #[test]
    fn path_alias_to_the_retained_root_is_a_same_project_save() {
        let tmp = TmpDir::new("same-root-alias");
        let bundle = tmp.path().join("Same.opentake");
        let mut session = seed_bundle_with_internal_media(&bundle, "clip.png", b"media bytes");
        std::fs::create_dir_all(bundle.join("chat-sessions")).unwrap();
        std::fs::write(bundle.join("chat-sessions/thread.json"), b"chat bytes").unwrap();
        std::fs::write(bundle.join("thumbnail.jpg"), b"cover bytes").unwrap();
        let alias_hop = tmp.path().join("alias-hop");
        std::fs::create_dir_all(&alias_hop).unwrap();
        let alias = alias_hop.join("..").join("Same.opentake");

        let written = session.save_project(Some(alias)).unwrap();

        assert_eq!(written, bundle);
        assert_eq!(session.project_dir(), Some(bundle.as_path()));
        assert_eq!(
            std::fs::read(bundle.join("media/clip.png")).unwrap(),
            b"media bytes"
        );
        assert_eq!(
            std::fs::read(bundle.join("chat-sessions/thread.json")).unwrap(),
            b"chat bytes"
        );
        assert_eq!(
            std::fs::read(bundle.join("thumbnail.jpg")).unwrap(),
            b"cover bytes"
        );
        assert!(!tmp.path().join(".Same.opentake.opentake-backup").exists());
    }

    #[test]
    fn save_with_thumbnail_bytes_writes_thumbnail_jpg() {
        let tmp = TmpDir::new("thumb");
        let dir = tmp.path().join("Cover.opentake");
        let mut s = EditorSession::new_project();
        s.state = EditorState::from_timeline(one_video_track());

        let jpeg = vec![0xFF, 0xD8, 1, 2, 3, 0xFF, 0xD9]; // stand-in JPEG bytes
        let written = s
            .save_project_with_thumbnail(Some(dir.clone()), Some(jpeg.clone()))
            .unwrap();
        assert_eq!(written, dir);
        let thumb = dir.join("thumbnail.jpg");
        assert!(thumb.is_file(), "thumbnail.jpg not written");
        assert_eq!(std::fs::read(&thumb).unwrap(), jpeg);
    }

    #[test]
    fn save_without_thumbnail_leaves_existing_cover_untouched() {
        let tmp = TmpDir::new("thumb-keep");
        let dir = tmp.path().join("Keep.opentake");
        let mut s = EditorSession::new_project();
        s.state = EditorState::from_timeline(one_video_track());

        // First save writes a cover.
        let jpeg = vec![0xFF, 0xD8, 9, 9, 0xFF, 0xD9];
        s.save_project_with_thumbnail(Some(dir.clone()), Some(jpeg.clone()))
            .unwrap();

        // A subsequent save with no thumbnail bytes must not delete/overwrite the
        // existing thumbnail.jpg (bundle.save only writes it when Some).
        s.save_project_with_thumbnail(None, None).unwrap();
        assert_eq!(std::fs::read(dir.join("thumbnail.jpg")).unwrap(), jpeg);
    }

    #[test]
    fn explicit_thumbnail_remove_is_distinct_from_capture_failure_preserve() {
        let tmp = TmpDir::new("thumb-remove");
        let dir = tmp.path().join("Remove.opentake");
        let mut session = EditorSession::new_project();
        session.state = EditorState::from_timeline(one_video_track());
        let jpeg = vec![0xFF, 0xD8, 4, 2, 0xFF, 0xD9];
        session
            .save_project_with_thumbnail(Some(dir.clone()), Some(jpeg))
            .unwrap();

        session
            .save_project_with_thumbnail_update(None, ThumbnailUpdate::Remove)
            .unwrap();

        assert!(!dir.join("thumbnail.jpg").exists());
    }

    #[test]
    fn save_as_with_no_source_media_dir_is_ok() {
        let tmp = TmpDir::new("nomedia");
        let src = tmp.path().join("NoMedia.opentake");
        let dst = tmp.path().join("Out.opentake");

        // Source bundle saved WITHOUT any media/ dir (external-only / empty
        // project). Save-as must succeed and simply not create a media/ dir.
        let mut project = Project::new(src.clone());
        project.timeline = one_video_track();
        project.save().unwrap();
        let mut s = EditorSession::open_project(&src).unwrap();

        let written = s.save_project(Some(dst.clone())).unwrap();
        assert_eq!(written, dst);
        assert!(dst.join("project.json").is_file());
        assert!(
            !dst.join("media").exists(),
            "no source media/ -> none should be created"
        );
    }
}
