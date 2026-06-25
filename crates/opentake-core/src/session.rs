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
//! 4. decode `generation_log` (lenient; `opentake-project` already degrades a
//!    malformed log to `None`).
//!
//! Asset materialization / thumbnails / waveforms (step 3's tail in the spec)
//! are a media-layer concern injected via [`crate::deps`] and are not performed
//! here.

use std::path::{Path, PathBuf};

use opentake_domain::{ClipType, MediaAsset, MediaManifest, MediaManifestEntry, Timeline};
use opentake_ops::command::{self, EditCommand, EditResult};
use opentake_ops::{EditorState, IdGen};
use opentake_project::{GenerationLog, Project};

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
}

/// File extensions the importer accepts, grouped by the [`ClipType`] they map to.
/// Mirrors upstream `ClipType(fileExtension:)` minus the Lottie special-case
/// (Lottie needs a content sniff the bare extension can't provide, so JSON files
/// are not auto-imported here).
pub const SUPPORTED_VIDEO_EXTENSIONS: [&str; 3] = ["mov", "mp4", "m4v"];
/// Accepted audio extensions.
pub const SUPPORTED_AUDIO_EXTENSIONS: [&str; 4] = ["mp3", "wav", "aac", "m4a"];
/// Accepted image extensions.
pub const SUPPORTED_IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "tiff", "heic", "webp"];

/// The [`ClipType`] for `path` if its (lowercased) extension is on the import
/// white-list, else `None`. JSON/Lottie are intentionally excluded (see
/// [`SUPPORTED_VIDEO_EXTENSIONS`]).
pub fn importable_clip_type(path: &Path) -> Option<ClipType> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if SUPPORTED_VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Some(ClipType::Video)
    } else if SUPPORTED_AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Some(ClipType::Audio)
    } else if SUPPORTED_IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        Some(ClipType::Image)
    } else {
        None
    }
}

/// The open document plus its project-level metadata.
pub struct EditorSession {
    /// Authoritative editable state: timeline, manifest, undo/redo, version.
    /// Edits go through [`opentake_ops::command::apply`]; the session never
    /// reimplements the transaction.
    state: EditorState,

    /// Absolute path to the `.opentake` bundle, or `None` for an unsaved project.
    project_dir: Option<PathBuf>,

    /// Append-only AI generation audit log (persisted as `generation-log.json`).
    generation_log: GenerationLog,
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
            generation_log: GenerationLog::new(),
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
        let project = Project::open(path)?;
        // EditorState::new wraps timeline + manifest with empty history at
        // version 0 — exactly the post-open state we want.
        let state = EditorState::new(project.timeline, project.manifest);
        Ok(EditorSession {
            state,
            project_dir: Some(project.bundle_path),
            generation_log: project.generation_log.unwrap_or_default(),
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
    /// Errors with [`CoreError::NoProjectOpen`] when neither a path nor a
    /// remembered project dir is available.
    pub fn save_project(&mut self, path: Option<PathBuf>) -> Result<PathBuf> {
        let target = match path.or_else(|| self.project_dir.clone()) {
            Some(p) => p,
            None => return Err(CoreError::NoProjectOpen),
        };

        let mut project = Project::new(target.clone());
        project.timeline = self.state.timeline.clone();
        project.manifest = self.state.manifest.clone();
        // Only persist a generation log once it has rows (mirrors the upstream
        // "write the log component when present" tolerance).
        if !self.generation_log.entries.is_empty() {
            project.generation_log = Some(self.generation_log.clone());
        }
        project.save()?;

        self.project_dir = Some(target.clone());
        Ok(target)
    }

    /// Route one [`EditCommand`] through the single editing entry point,
    /// delegating the whole snapshot/commit/version transaction to
    /// `opentake-ops`. `Undo`/`Redo` are ordinary commands here (the ops layer
    /// models them as such), so the session needs no separate undo plumbing.
    pub fn apply(&mut self, command: EditCommand, ids: &dyn IdGen) -> Result<EditResult> {
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
        let path = path.as_ref();
        let kind = importable_clip_type(path).ok_or(CoreError::Unsupported("media"))?;

        let mut asset = MediaAsset::new(id, path, kind, name, probe.duration_secs);
        asset.source_width = probe.width;
        asset.source_height = probe.height;
        asset.source_fps = probe.fps;
        // Video defaults to having audio (MediaAsset::new); refine from the probe.
        // Non-video never carries a video-track-linked audio flag upstream.
        asset.has_audio = match kind {
            ClipType::Audio => true,
            ClipType::Video => probe.has_audio,
            _ => false,
        };

        // `now = 0`: a freshly imported local file has no cached remote URL, so
        // the freshness clock is irrelevant to the produced entry.
        let entry = asset.to_manifest_entry(self.project_dir.as_deref(), 0.0);
        self.state.manifest.entries.push(entry.clone());
        Ok(entry)
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
        entry.has_audio = Some(match kind {
            ClipType::Audio => true,
            ClipType::Video => probe.has_audio,
            _ => false,
        });
        Ok(entry.clone())
    }

    /// A clone of the current media manifest (read-only mirror for the media
    /// panel). The manifest is the persisted id→file catalog.
    pub fn media(&self) -> MediaManifest {
        self.state.manifest.clone()
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

    /// Whether a redo is available.
    pub fn can_redo(&self) -> bool {
        self.state.can_redo()
    }

    /// The current bundle path, if the project has one.
    pub fn project_dir(&self) -> Option<&Path> {
        self.project_dir.as_deref()
    }

    /// Read-only access to the generation log.
    pub fn generation_log(&self) -> &GenerationLog {
        &self.generation_log
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
        // JSON/Lottie is intentionally not auto-importable here.
        assert_eq!(importable_clip_type(Path::new("/x/anim.json")), None);
        assert_eq!(importable_clip_type(Path::new("/x/notes.txt")), None);
        assert_eq!(importable_clip_type(Path::new("/x/noext")), None);
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
    fn import_image_has_no_audio_regardless_of_probe() {
        let mut s = EditorSession::new_project();
        let probe = ProbedMedia {
            duration_secs: 0.0,
            width: Some(800),
            height: Some(600),
            fps: None,
            has_audio: true, // probe lies; an image never has audio
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
}
