//! Media import command surface.
//!
//! These are the commands the media panel calls to bring local files into the
//! project. They sit on top of two managed-state handles:
//!
//! - [`opentake_core::AppCore`] — the authoritative session; importing appends a
//!   [`MediaManifestEntry`](opentake_domain::MediaManifestEntry) to its manifest
//!   and emits `MediaChanged` (forwarded to the WebView by
//!   [`crate::forward_event`]).
//! - [`MediaState`] — a thin wrapper over an [`opentake_media::MediaEngine`],
//!   used here only to **probe** each file (duration / dimensions / fps / audio).
//!
//! The split mirrors upstream `addMediaAsset(from:)` → `finalizeImportedAsset`:
//! the manifest entry is created from the file path immediately (an *external*
//! reference — the file is not copied into the bundle), then the probe fills in
//! the metadata. Probing is best-effort: if ffprobe is unavailable or the file
//! is unreadable, the asset still imports with zero/empty metadata rather than
//! failing the whole batch (a missing/offline file is a recoverable state the
//! editor already models).
//!
//! Thumbnails are exposed as local cache file paths when they already exist.
//! Import and list commands never decode frames; the WebView asks for thumbnails
//! lazily through `generate_thumbnail`.

use std::collections::{BTreeSet, HashSet};
use std::ffi::{OsStr, OsString};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cap_fs_ext::{ambient_authority, DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use image::ImageEncoder;
use same_file::Handle as FileIdentity;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use opentake_core::{
    importable_clip_type, AppCore, CommittedMediaImport, CoreError, DeferredCoreEvents,
    DerivedStemProvenance, PreparedMediaFolderRef, PreparedMediaImportOp, ProbedMedia,
};
use opentake_domain::{
    AudioDenoise, Clip, ClipType, DenoiseMode, GenerationInput, GenerationJobStatus,
    LoudnessNormalization, MediaManifest, MediaManifestEntry, MediaProxy, MediaSource,
    StabilizationTrack, Timeline,
};
use opentake_media::library::{FavoriteRequest, LibraryStore, PreparedFavorite};
#[cfg(test)]
use opentake_media::MediaCancelToken;
use opentake_media::{
    analysis::{
        analyze_loudness_with_progress, analyze_stabilization as build_stabilization,
        denoise_interleaved, separate_stems, track_translation_motion, LoudnessNormalizationConfig,
        StabilizationConfig, StemExecution, StemSeparationRequest,
    },
    cache_key::visual_file_identity_key,
    create_proxy, decode_frame_at, decode_frame_at_cancellable, decode_frames_at,
    decode_frames_at_cancellable, extract_pcm_cancellable_with_progress,
    thumbnail::{
        encode_sprite, representative_thumbnail_times, save_sprite, sprite::grid_geometry,
        video_thumbnail_times, EncodedSpriteArtifact, ThumbnailCacheMeta, VideoThumb,
        MAX_VIDEO_THUMBNAILS, THUMB_MAX_SIZE, THUMB_TOLERANCE_SECS,
    },
    waveform::store::CACHE_SUBDIR,
    FrameRequest, MediaEngine, MediaError, PcmFormat, PcmSpec, ProxyProgressCallback, ProxyRequest,
    RgbaFrame,
};
use opentake_ops::{ClipEntry, EditCommand};
use opentake_project::ProjectRoot;

use crate::library::LibraryState;

pub mod prewarm;

/// Managed-state wrapper over the media engine. The engine is read-only here
/// (probe only) and shared across commands; `Send + Sync` so it lives in Tauri
/// state.
pub struct MediaState {
    engine: MediaEngine,
    admission: crate::updater::InstallAdmissionGate,
}

/// Single-flight cooperative cancellation for an Inspector stabilization run.
#[derive(Default)]
pub struct StabilizationAnalysisState {
    active: Mutex<Option<ActiveDeferredAnalysis>>,
    admission: crate::updater::InstallAdmissionGate,
}

/// Single-flight cooperative cancellation for an Inspector loudness run.
#[derive(Default)]
pub struct LoudnessAnalysisState {
    active: Mutex<Option<ActiveDeferredAnalysis>>,
    admission: crate::updater::InstallAdmissionGate,
}

/// Single-flight cooperative cancellation for an Inspector denoise validation.
#[derive(Default)]
pub struct DenoiseAnalysisState {
    active: Mutex<Option<ActiveDeferredAnalysis>>,
    admission: crate::updater::InstallAdmissionGate,
}

struct ActiveDeferredAnalysis {
    cancel: opentake_media::MediaCancelToken,
    _admission: crate::updater::ActivityLease,
}

/// Single-flight cooperative cancellation for a two-stem separation job.
#[derive(Default)]
pub struct StemSeparationState {
    active: Mutex<Option<ActiveProjectMutation>>,
    admission: crate::updater::InstallAdmissionGate,
}

/// Single-flight proxy transcode plus the app-level playback preference. The
/// preference is mirrored from localStorage at startup; it changes playback
/// source selection only and never changes export resolution.
#[derive(Default)]
pub struct MediaProxyState {
    active: Mutex<Option<ActiveProjectMutation>>,
    admission: crate::updater::InstallAdmissionGate,
    enabled: AtomicBool,
}

struct ActiveProjectMutation {
    cancel: opentake_media::MediaCancelToken,
    _admission: crate::updater::ActivityLease,
}

impl MediaProxyState {
    pub(crate) fn new(admission: crate::updater::InstallAdmissionGate) -> Self {
        Self {
            active: Mutex::new(None),
            admission,
            enabled: AtomicBool::new(false),
        }
    }

    fn begin(&self) -> Result<opentake_media::MediaCancelToken, String> {
        let admission = self.admission.begin_activity()?;
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.is_some() {
            return Err("media_proxy_busy".to_string());
        }
        let token = opentake_media::MediaCancelToken::new();
        *active = Some(ActiveProjectMutation {
            cancel: token.clone(),
            _admission: admission,
        });
        Ok(token)
    }

    fn finish(&self, token: &opentake_media::MediaCancelToken) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active
            .as_ref()
            .is_some_and(|current| current.cancel.same_instance(token))
        {
            *active = None;
        }
    }

    pub(crate) fn cancel(&self) -> bool {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.as_ref().is_some_and(|operation| {
            operation.cancel.cancel();
            true
        })
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }
}

impl StemSeparationState {
    pub(crate) fn new(admission: crate::updater::InstallAdmissionGate) -> Self {
        Self {
            active: Mutex::new(None),
            admission,
        }
    }

    fn begin(&self) -> Result<opentake_media::MediaCancelToken, String> {
        let admission = self.admission.begin_activity()?;
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.is_some() {
            return Err("stem_separation_busy".to_string());
        }
        let token = opentake_media::MediaCancelToken::new();
        *active = Some(ActiveProjectMutation {
            cancel: token.clone(),
            _admission: admission,
        });
        Ok(token)
    }

    fn finish(&self, token: &opentake_media::MediaCancelToken) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active
            .as_ref()
            .is_some_and(|current| current.cancel.same_instance(token))
        {
            *active = None;
        }
    }

    fn cancel(&self) -> bool {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.as_ref().is_some_and(|operation| {
            operation.cancel.cancel();
            true
        })
    }
}

fn begin_direct_media_project_write(
    admission: &crate::updater::InstallAdmissionGate,
) -> Result<crate::updater::ActivityLease, String> {
    crate::updater::begin_mutating_activity(admission)
}

impl DenoiseAnalysisState {
    pub(crate) fn new(admission: crate::updater::InstallAdmissionGate) -> Self {
        Self {
            active: Mutex::new(None),
            admission,
        }
    }

    fn begin(&self) -> Result<opentake_media::MediaCancelToken, String> {
        let admission = self.admission.begin_activity()?;
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.is_some() {
            return Err("denoise_analysis_busy".to_string());
        }
        let token = opentake_media::MediaCancelToken::new();
        *active = Some(ActiveDeferredAnalysis {
            cancel: token.clone(),
            _admission: admission,
        });
        Ok(token)
    }

    fn finish(&self, token: &opentake_media::MediaCancelToken) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active
            .as_ref()
            .is_some_and(|current| current.cancel.same_instance(token))
        {
            *active = None;
        }
    }

    fn cancel(&self) -> bool {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.as_ref().is_some_and(|analysis| {
            analysis.cancel.cancel();
            true
        })
    }
}

impl LoudnessAnalysisState {
    pub(crate) fn new(admission: crate::updater::InstallAdmissionGate) -> Self {
        Self {
            active: Mutex::new(None),
            admission,
        }
    }

    fn begin(&self) -> Result<opentake_media::MediaCancelToken, String> {
        let admission = self.admission.begin_activity()?;
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.is_some() {
            return Err("loudness_analysis_busy".to_string());
        }
        let token = opentake_media::MediaCancelToken::new();
        *active = Some(ActiveDeferredAnalysis {
            cancel: token.clone(),
            _admission: admission,
        });
        Ok(token)
    }

    fn finish(&self, token: &opentake_media::MediaCancelToken) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active
            .as_ref()
            .is_some_and(|current| current.cancel.same_instance(token))
        {
            *active = None;
        }
    }

    fn cancel(&self) -> bool {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.as_ref().is_some_and(|analysis| {
            analysis.cancel.cancel();
            true
        })
    }
}

impl StabilizationAnalysisState {
    pub(crate) fn new(admission: crate::updater::InstallAdmissionGate) -> Self {
        Self {
            active: Mutex::new(None),
            admission,
        }
    }

    fn begin(&self) -> Result<opentake_media::MediaCancelToken, String> {
        let admission = self.admission.begin_activity()?;
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.is_some() {
            return Err("a stabilization analysis is already running".to_string());
        }
        let token = opentake_media::MediaCancelToken::new();
        *active = Some(ActiveDeferredAnalysis {
            cancel: token.clone(),
            _admission: admission,
        });
        Ok(token)
    }

    fn finish(&self, token: &opentake_media::MediaCancelToken) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active
            .as_ref()
            .is_some_and(|current| current.cancel.same_instance(token))
        {
            *active = None;
        }
    }

    fn cancel(&self) -> bool {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(analysis) = active.as_ref() {
            analysis.cancel.cancel();
            true
        } else {
            false
        }
    }
}

/// Project replacement invalidates every Inspector analysis result. Cancelling
/// all three workers at the identity-transition boundary avoids wasting decode
/// work; the front end additionally submits any late result with its original
/// optimistic authority token, so it cannot be committed to the replacement
/// project even if cancellation races with completion.
pub(crate) fn cancel_project_bound_analyses(
    stabilization: &StabilizationAnalysisState,
    loudness: &LoudnessAnalysisState,
    denoise: &DenoiseAnalysisState,
) -> bool {
    let stabilization_cancelled = stabilization.cancel();
    let loudness_cancelled = loudness.cancel();
    let denoise_cancelled = denoise.cancel();
    stabilization_cancelled || loudness_cancelled || denoise_cancelled
}

impl MediaState {
    /// Wrap an engine for managed state.
    pub fn new(engine: MediaEngine) -> Self {
        Self::new_with_admission(engine, crate::updater::InstallAdmissionGate::default())
    }

    pub(crate) fn new_with_admission(
        engine: MediaEngine,
        admission: crate::updater::InstallAdmissionGate,
    ) -> Self {
        MediaState { engine, admission }
    }

    /// The wrapped engine.
    pub fn engine(&self) -> &MediaEngine {
        &self.engine
    }

    fn begin_cache_write(&self) -> Result<crate::updater::ActivityLease, String> {
        crate::updater::begin_mutating_activity(&self.admission)
    }
}

/// One media item for the panel. camelCase to match the existing DTO surface
/// (`core-SPEC.md` §6). `duration` is in seconds; `thumbnail` is the on-disk
/// first-frame thumbnail path when one exists. `path` is the resolvable absolute
/// source path.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaItemDto {
    /// Asset id (the clip layer's `media_ref`).
    pub id: String,
    /// Display name (file stem unless renamed).
    pub name: String,
    /// Media kind: `"video" | "audio" | "image" | ...` (lowercase, per `ClipType`).
    #[serde(rename = "type")]
    pub kind: ClipType,
    /// Duration in seconds (0 for stills).
    pub duration: f64,
    /// Source width in pixels, when known.
    pub width: Option<i32>,
    /// Source height in pixels, when known.
    pub height: Option<i32>,
    /// Probed source frame rate, used by the first-video settings decision.
    pub source_fps: Option<f64>,
    /// Whether the asset carries audio.
    pub has_audio: bool,
    /// Original source color signalling retained for HDR-aware UI.
    pub color: Option<opentake_domain::MediaColorMetadata>,
    /// True for PQ/HLG sources. The current compositor delivers SDR BT.709.
    pub is_hdr: bool,
    /// Absolute path to the source file, when resolvable. Project-relative
    /// derived assets are resolved against the open project bundle.
    pub path: Option<String>,
    /// Absolute project-local playback proxy path when one is materialized.
    pub proxy_path: Option<String>,
    pub proxy_width: Option<u32>,
    pub proxy_height: Option<u32>,
    /// On-disk thumbnail path, or `None` to render a type placeholder.
    pub thumbnail: Option<String>,
    /// Library folder this asset lives in (`None` = root), for the folder view.
    pub folder_id: Option<String>,
    /// Source file size in bytes, when the file resolves on disk. Surfaced for
    /// the Inspector's Source → File section "Size" row (upstream
    /// `InspectorView.fileSize(for:)`, which reads `FileManager` attributes).
    /// `None` for missing/unresolvable sources.
    pub file_size: Option<u64>,
    /// Generation snapshot for an AI-generated asset (`None` for imported /
    /// user assets). 1:1 with upstream `MediaAsset.generationInput`; drives the
    /// Inspector's Source → Generated / Prompt / References sections. Today no
    /// generation flow populates it (generate_* is still stubbed), so it is
    /// always `None` in practice — the Inspector renders those sections only
    /// when it is present, matching upstream's `if let gen = asset.generationInput`.
    pub generation_input: Option<GenerationInput>,
    /// Durable async lifecycle projected from `generation_input`.
    pub generation_status: String,
    pub generation_progress: Option<f64>,
    pub generation_error_code: Option<String>,
    /// `true` when the asset's source file is not on disk (moved / deleted /
    /// offline). Derived from file existence on every read (mirrors upstream
    /// `MediaResolver.isMissing`), so it clears automatically once a `relink_media`
    /// points the asset at a real file again. The panel/timeline render an
    /// "offline" affordance for missing assets.
    pub missing: bool,
    /// `true` when the user has favorited this asset (#91). Backs the media
    /// panel's "mine" tab. Persisted per-project in the manifest's favorites set
    /// (not browser localStorage), so favorites travel with the project.
    pub favorite: bool,
}

impl MediaItemDto {
    /// Project a manifest entry onto the panel DTO. `project_dir` resolves
    /// [`MediaSource::Project`] relative paths for the `missing` existence check.
    fn from_entry(
        entry: &MediaManifestEntry,
        project_dir: Option<&Path>,
        cache_root: Option<&Path>,
        favorite: bool,
    ) -> Self {
        let resolved = resolve_source_path(entry, project_dir);
        let path = resolved
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned());
        let resolved_proxy = entry
            .proxy
            .as_ref()
            .and_then(|proxy| trusted_project_proxy_path(project_dir?, &proxy.relative_path))
            .filter(|path| crate::fs_availability::is_materialized_regular_file(path));
        let generation_input = entry.generation_input.as_ref();
        let generation_status = match generation_input.and_then(|input| input.status) {
            Some(GenerationJobStatus::Queued | GenerationJobStatus::Generating) => "generating",
            Some(GenerationJobStatus::Downloading | GenerationJobStatus::Finalizing) => {
                "downloading"
            }
            Some(GenerationJobStatus::Failed) => "failed",
            Some(GenerationJobStatus::Cancelled) => "cancelled",
            Some(GenerationJobStatus::Ready) | None => "none",
        }
        .to_string();
        let generation_pending = matches!(
            generation_input.and_then(|input| input.status),
            Some(
                GenerationJobStatus::Queued
                    | GenerationJobStatus::Generating
                    | GenerationJobStatus::Downloading
                    | GenerationJobStatus::Finalizing
            )
        );
        // Missing = a finalized source can resolve and is absent or still a
        // cloud-only placeholder. Pending generation placeholders intentionally
        // have no file yet and are not "offline".
        // An unresolvable (e.g. remote-only) source is not flagged missing.
        let missing = !generation_pending
            && resolved
                .as_ref()
                .map(|path| !crate::fs_availability::is_materialized_regular_file(path))
                .unwrap_or(false);
        let thumbnail = if missing {
            None
        } else {
            resolved
                .as_deref()
                .and_then(|path| {
                    cache_root.and_then(|root| cached_thumbnail_path_for_entry(root, entry, path))
                })
                .filter(|path| {
                    crate::fs_availability::is_materialized_regular_file(Path::new(path))
                })
        };
        // File size from the resolved source when it exists (upstream reads
        // FileManager attributes lazily). Skipped for missing/unresolvable sources.
        let file_size = if missing {
            None
        } else {
            resolved
                .as_deref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| m.len())
        };
        MediaItemDto {
            id: entry.id.clone(),
            name: entry.name.clone(),
            kind: entry.kind,
            duration: entry.duration,
            width: entry.source_width,
            height: entry.source_height,
            source_fps: entry.source_fps,
            has_audio: entry.has_audio.unwrap_or(false),
            color: entry.color.clone(),
            is_hdr: entry.color.as_ref().is_some_and(|color| color.is_hdr()),
            path,
            proxy_path: resolved_proxy
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            proxy_width: entry.proxy.as_ref().map(|proxy| proxy.width),
            proxy_height: entry.proxy.as_ref().map(|proxy| proxy.height),
            thumbnail,
            folder_id: entry.folder_id.clone(),
            file_size,
            generation_input: entry.generation_input.clone(),
            generation_status,
            generation_progress: generation_input.and_then(|input| input.progress),
            generation_error_code: generation_input.and_then(|input| input.error_code.clone()),
            missing,
            favorite,
        }
    }
}

/// Resolve a manifest entry's source to a local path, when it has one:
/// external assets are absolute; project-relative assets join the bundle base.
fn resolve_source_path(entry: &MediaManifestEntry, project_dir: Option<&Path>) -> Option<PathBuf> {
    match &entry.source {
        MediaSource::External { absolute_path } => Some(PathBuf::from(absolute_path)),
        MediaSource::Project { relative_path } => project_dir.map(|base| base.join(relative_path)),
    }
}

fn source_path_for_entry(
    entry: &MediaManifestEntry,
    project_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    match &entry.source {
        MediaSource::External { absolute_path } => Ok(PathBuf::from(absolute_path)),
        MediaSource::Project { relative_path } => project_dir
            .map(|base| base.join(relative_path))
            .ok_or_else(|| "project not saved; cannot resolve media path".into()),
    }
}

/// A media-library folder for the panel's folder tree (mirror of
/// [`opentake_domain::MediaFolder`]).
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaFolderDto {
    pub id: String,
    pub name: String,
    pub parent_folder_id: Option<String>,
}

/// The media panel's catalog: every manifest entry as a [`MediaItemDto`].
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaListDto {
    /// All media items, in manifest order.
    pub items: Vec<MediaItemDto>,
    /// All library folders (flat list; nest via `parentFolderId`).
    pub folders: Vec<MediaFolderDto>,
    /// File names that were dropped during this import because their type is not
    /// importable (mirrors upstream `addMediaAsset` → `mediaPanelToast`). Always
    /// empty for pure listing/relink; only import commands populate it so the
    /// front end can toast "skipped N unsupported files" instead of dropping them
    /// silently. Serialized as `skipped`.
    #[serde(default)]
    pub skipped: Vec<String>,
    /// Admission decisions for best-effort import poster prewarm. Import stays
    /// successful even when the bounded queue is busy, while callers can still
    /// observe whether each poster was queued, coalesced, cached, or rejected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prewarm: Vec<ImportPrewarmDto>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteSyncFailureDto {
    pub asset_id: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteSyncDto {
    pub media: MediaListDto,
    pub migrated_legacy_asset_ids: Vec<String>,
    pub failures: Vec<FavoriteSyncFailureDto>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportPrewarmDto {
    pub media_ref: String,
    pub result: prewarm::PrewarmResult,
}

impl MediaListDto {
    /// Build the list from the core's current manifest snapshot, with no skipped
    /// files (listing / relink / non-import surfaces). `pub(crate)` so sibling
    /// command modules (e.g. capture-to-media in `render.rs`) can return the
    /// current catalog after mutating it.
    pub(crate) fn from_core(core: &AppCore, cache_root: Option<&Path>) -> Self {
        Self::from_core_with_import_results(core, cache_root, Vec::new(), Vec::new())
    }

    fn from_core_with_import_results(
        core: &AppCore,
        cache_root: Option<&Path>,
        skipped: Vec<String>,
        prewarm: Vec<ImportPrewarmDto>,
    ) -> Self {
        let snapshot = core.runtime_snapshot();
        let manifest = snapshot.media;
        let project_dir = snapshot.project_dir;
        MediaListDto {
            items: manifest
                .entries
                .iter()
                .map(|e| {
                    MediaItemDto::from_entry(
                        e,
                        project_dir.as_deref(),
                        cache_root,
                        manifest.is_favorite(&e.id),
                    )
                })
                .collect(),
            folders: manifest
                .folders
                .iter()
                .map(|f| MediaFolderDto {
                    id: f.id.clone(),
                    name: f.name.clone(),
                    parent_folder_id: f.parent_folder_id.clone(),
                })
                .collect(),
            skipped,
            prewarm,
        }
    }
}

/// Cached thumbnail/sprite metadata returned to the WebView. Paths are plain
/// local file paths; the front end converts them through Tauri's asset protocol.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailDto {
    /// Asset id this thumbnail belongs to.
    pub media_ref: String,
    /// Media kind (`type` in JSON).
    #[serde(rename = "type")]
    pub kind: ClipType,
    /// Single-frame thumbnail path (PNG), suitable for media cards.
    pub thumbnail_path: Option<String>,
    /// Video sprite path (JPEG), suitable for timeline filmstrips.
    pub sprite_path: Option<String>,
    /// Sprite/source tile width in pixels.
    pub tile_width: Option<u32>,
    /// Sprite/source tile height in pixels.
    pub tile_height: Option<u32>,
    /// Number of columns in the video sprite grid.
    pub columns: Option<u32>,
    /// Source times represented by the sprite tiles, in seconds.
    pub times: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSpriteDto {
    pub status: prewarm::TimelineSpriteStatus,
    pub thumbnail: Option<ThumbnailDto>,
}

fn empty_thumbnail_dto(entry: &MediaManifestEntry) -> ThumbnailDto {
    ThumbnailDto {
        media_ref: entry.id.clone(),
        kind: entry.kind,
        thumbnail_path: None,
        sprite_path: None,
        tile_width: None,
        tile_height: None,
        columns: None,
        times: Vec::new(),
    }
}

fn cache_key_for(path: &Path) -> Result<String, String> {
    visual_file_identity_key(path)
        .ok_or_else(|| format!("could not build thumbnail cache key for {}", path.display()))
}

fn visual_cache_dir(cache_root: &Path) -> PathBuf {
    cache_root.join(CACHE_SUBDIR)
}

fn sprite_path_for(cache_root: &Path, key: &str) -> PathBuf {
    visual_cache_dir(cache_root).join(format!("{key}.thumbs.jpg"))
}

fn partial_sprite_path_for(cache_root: &Path, key: &str) -> PathBuf {
    visual_cache_dir(cache_root).join(format!("{key}.thumbs.partial.jpg"))
}

fn poster_path_for(cache_root: &Path, key: &str) -> PathBuf {
    visual_cache_dir(cache_root).join(format!("{key}.thumb.png"))
}

/// Hi-res preview-poster box: the first-frame still shown instantly behind the
/// `<video>` in the single-media preview. Much larger than the 120×68 grid
/// thumbnail ([`THUMB_MAX_SIZE`]) so the preview isn't blurry; the asset
/// protocol streams the real video progressively once metadata loads, so this is
/// purely the instant placeholder. Downscale-only (never enlarged).
const PREVIEW_POSTER_MAX_SIZE: (u32, u32) = (1920, 1080);

/// Cache path for a hi-res preview poster. Keyed separately (`.preview…`) from
/// the small grid poster (`.thumb…`) so the two sizes never clobber each other.
fn preview_poster_path_for(cache_root: &Path, key: &str, time_secs: f64) -> PathBuf {
    if time_secs <= 0.0 {
        return visual_cache_dir(cache_root).join(format!("{key}.preview.png"));
    }
    let millis = (time_secs * 1000.0).round().max(0.0) as u64;
    visual_cache_dir(cache_root).join(format!("{key}.preview.{millis}.png"))
}

fn timed_poster_path_for(cache_root: &Path, key: &str, time_secs: f64) -> PathBuf {
    if time_secs <= 0.0 {
        return poster_path_for(cache_root, key);
    }
    let millis = (time_secs * 1000.0).round().max(0.0) as u64;
    visual_cache_dir(cache_root).join(format!("{key}.thumb.{millis}.png"))
}

fn write_png(path: &Path, frame: &RgbaFrame) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = encode_png(frame)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

fn encode_png(frame: &RgbaFrame) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            &frame.rgba,
            frame.width,
            frame.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("png encode: {e}"))?;
    Ok(bytes)
}

fn cached_thumbnail_path_for_entry(
    cache_root: &Path,
    entry: &MediaManifestEntry,
    path: &Path,
) -> Option<String> {
    if !matches!(entry.kind, ClipType::Video | ClipType::Image) {
        return None;
    }
    let key = cache_key_for(path).ok()?;
    let poster_path = poster_path_for(cache_root, &key);
    poster_path
        .is_file()
        .then(|| poster_path.to_string_lossy().into_owned())
}

fn poster_target_time(time_secs: Option<f64>) -> f64 {
    time_secs
        .filter(|t| t.is_finite() && *t > 0.0)
        .unwrap_or(0.0)
}

fn read_cached_poster(
    poster_path: &Path,
    target: f64,
) -> Option<Result<(PathBuf, u32, u32, f64), String>> {
    if !poster_path.exists() {
        return None;
    }
    Some(
        image::image_dimensions(poster_path)
            .map(|(width, height)| (poster_path.to_path_buf(), width, height, target))
            .map_err(|error| format!("thumbnail dimensions: {error}")),
    )
}

/// Decode (or read from cache) a single poster frame for `path` at `target`,
/// scaled to fit `max_size`, written to `poster_path`. Shared by the small grid
/// poster ([`video_poster`]) and the hi-res preview poster
/// ([`video_preview_poster`]); the two pass different `max_size` + `poster_path`
/// so their caches never clash. Returns `(path, width, height, actual_time)`.
fn decode_poster_to(
    path: &Path,
    poster_path: PathBuf,
    target: f64,
    max_size: (u32, u32),
) -> Result<(PathBuf, u32, u32, f64), String> {
    if let Some(cached) = read_cached_poster(&poster_path, target) {
        return cached;
    }

    let req = FrameRequest {
        time_secs: target,
        max_size,
        tolerance_secs: THUMB_TOLERANCE_SECS,
        apply_rotation: true,
    };
    let (actual, frame) = decode_frame_at(path, &req).map_err(|e| e.to_string())?;
    write_png(&poster_path, &frame)?;
    Ok((poster_path, frame.width, frame.height, actual))
}

fn video_poster(
    engine: &MediaEngine,
    path: &Path,
    key: &str,
    time_secs: Option<f64>,
) -> Result<(PathBuf, u32, u32, f64), String> {
    let target = poster_target_time(time_secs);
    let poster_path = timed_poster_path_for(engine.cache_root(), key, target);
    decode_poster_to(path, poster_path, target, THUMB_MAX_SIZE)
}

/// Hi-res first-frame poster for the single-media preview (see
/// [`PREVIEW_POSTER_MAX_SIZE`]). Cached separately from the grid poster.
fn video_preview_poster(
    engine: &MediaEngine,
    path: &Path,
    key: &str,
    time_secs: Option<f64>,
) -> Result<(PathBuf, u32, u32, f64), String> {
    let target = poster_target_time(time_secs);
    let poster_path = preview_poster_path_for(engine.cache_root(), key, target);
    decode_poster_to(path, poster_path, target, PREVIEW_POSTER_MAX_SIZE)
}

fn sprite_meta_path_for(cache_root: &Path, key: &str) -> PathBuf {
    visual_cache_dir(cache_root).join(format!("{key}.thumbs.json"))
}

fn partial_sprite_meta_path_for(cache_root: &Path, key: &str) -> PathBuf {
    visual_cache_dir(cache_root).join(format!("{key}.thumbs.partial.json"))
}

fn read_sprite_meta_at(sprite_path: &Path, meta_path: &Path) -> Option<ThumbnailCacheMeta> {
    if !sprite_path.is_file() || !meta_path.is_file() {
        return None;
    }
    let bytes = std::fs::read(meta_path).ok()?;
    let meta: ThumbnailCacheMeta = serde_json::from_slice(&bytes).ok()?;
    if meta.tile_width == 0
        || meta.tile_height == 0
        || meta.columns == 0
        || meta.times.is_empty()
        || meta.times.len() > MAX_VIDEO_THUMBNAILS
    {
        return None;
    }
    Some(meta)
}

fn read_cached_sprite_meta(cache_root: &Path, key: &str) -> Option<ThumbnailCacheMeta> {
    let sprite_path = sprite_path_for(cache_root, key);
    let meta_path = sprite_meta_path_for(cache_root, key);
    read_sprite_meta_at(&sprite_path, &meta_path)
}

fn thumbnail_dto_for_sprite(
    entry: &MediaManifestEntry,
    cache_root: &Path,
    poster_key: &str,
    sprite_path: &Path,
    meta: ThumbnailCacheMeta,
) -> ThumbnailDto {
    let poster = poster_path_for(cache_root, poster_key);
    ThumbnailDto {
        media_ref: entry.id.clone(),
        kind: entry.kind,
        thumbnail_path: poster
            .is_file()
            .then(|| poster.to_string_lossy().into_owned()),
        sprite_path: Some(sprite_path.to_string_lossy().into_owned()),
        tile_width: Some(meta.tile_width),
        tile_height: Some(meta.tile_height),
        columns: Some(meta.columns),
        times: meta.times,
    }
}

fn commit_sprite_artifact(
    context: &prewarm::JobContext,
    sprite_path: &Path,
    meta_path: &Path,
    artifact: &EncodedSpriteArtifact,
) -> Result<bool, String> {
    if !context.commit_staged_bytes(sprite_path, &artifact.jpeg)? {
        return Ok(false);
    }
    context.commit_staged_bytes(meta_path, &artifact.json)
}

fn sprite_frame_limit(max_frames: Option<usize>) -> usize {
    max_frames
        .unwrap_or(MAX_VIDEO_THUMBNAILS)
        .clamp(1, MAX_VIDEO_THUMBNAILS)
}

fn video_sprite(
    engine: &MediaEngine,
    entry: &MediaManifestEntry,
    path: &Path,
    key: &str,
    max_frames: Option<usize>,
) -> Result<Option<ThumbnailCacheMeta>, String> {
    let limit = sprite_frame_limit(max_frames);
    if let Some(mut meta) = read_cached_sprite_meta(engine.cache_root(), key) {
        meta.times.truncate(limit);
        return Ok(Some(meta));
    }

    let times: Vec<f64> = video_thumbnail_times(entry.duration)
        .into_iter()
        .take(limit)
        .collect();
    if times.is_empty() {
        return Ok(None);
    }

    let req = FrameRequest {
        time_secs: 0.0,
        max_size: THUMB_MAX_SIZE,
        tolerance_secs: THUMB_TOLERANCE_SECS,
        apply_rotation: true,
    };
    let mut thumbs = Vec::with_capacity(times.len());
    for result in decode_frames_at(path, &times, &req) {
        let (actual, frame) = result.map_err(|e| e.to_string())?;
        thumbs.push(VideoThumb {
            time_secs: actual,
            image: frame,
        });
    }
    if thumbs.is_empty() {
        return Ok(None);
    }
    save_sprite(engine.cache_root(), key, &thumbs).map_err(|e| e.to_string())?;
    let (columns, _) = grid_geometry(thumbs.len());
    Ok(Some(ThumbnailCacheMeta {
        tile_width: thumbs[0].image.width,
        tile_height: thumbs[0].image.height,
        columns,
        times: thumbs.iter().map(|t| t.time_secs).collect(),
    }))
}

fn generate_thumbnail_for_entry(
    engine: &MediaEngine,
    entry: &MediaManifestEntry,
    path: &Path,
    time_secs: Option<f64>,
    max_frames: Option<usize>,
    include_sprite: bool,
) -> Result<ThumbnailDto, String> {
    if !path.is_file() {
        return Err(format!("source file not found: {}", path.display()));
    }

    let key = cache_key_for(path)?;
    match entry.kind {
        ClipType::Video => {
            let (poster_path, poster_w, poster_h, poster_time) =
                video_poster(engine, path, &key, time_secs)?;
            let sprite_meta = if include_sprite {
                video_sprite(engine, entry, path, &key, max_frames)?
            } else {
                None
            };
            let sprite_path = sprite_path_for(engine.cache_root(), &key);
            Ok(ThumbnailDto {
                media_ref: entry.id.clone(),
                kind: entry.kind,
                thumbnail_path: Some(poster_path.to_string_lossy().into_owned()),
                sprite_path: if include_sprite && sprite_path.is_file() {
                    Some(sprite_path.to_string_lossy().into_owned())
                } else {
                    None
                },
                tile_width: sprite_meta
                    .as_ref()
                    .map(|m| m.tile_width)
                    .or(Some(poster_w)),
                tile_height: sprite_meta
                    .as_ref()
                    .map(|m| m.tile_height)
                    .or(Some(poster_h)),
                columns: sprite_meta.as_ref().map(|m| m.columns).or(Some(1)),
                times: sprite_meta
                    .map(|m| m.times)
                    .unwrap_or_else(|| vec![poster_time]),
            })
        }
        ClipType::Image => {
            let poster_path = poster_path_for(engine.cache_root(), &key);
            if !poster_path.exists() {
                let frame = engine.image_thumbnail(path).map_err(|e| e.to_string())?;
                write_png(&poster_path, &frame)?;
            }
            let (tile_width, tile_height) = image::image_dimensions(&poster_path)
                .map(|(w, h)| (Some(w), Some(h)))
                .unwrap_or((None, None));
            Ok(ThumbnailDto {
                media_ref: entry.id.clone(),
                kind: entry.kind,
                thumbnail_path: Some(poster_path.to_string_lossy().into_owned()),
                sprite_path: None,
                tile_width,
                tile_height,
                columns: Some(1),
                times: vec![0.0],
            })
        }
        _ => Ok(empty_thumbnail_dto(entry)),
    }
}

fn cached_thumbnail_for_entry(
    engine: &MediaEngine,
    entry: &MediaManifestEntry,
    path: &Path,
    time_secs: Option<f64>,
    max_frames: Option<usize>,
    include_sprite: bool,
) -> Option<Result<ThumbnailDto, String>> {
    if !path.is_file() {
        return Some(Err(format!("source file not found: {}", path.display())));
    }
    let key = match cache_key_for(path) {
        Ok(key) => key,
        Err(error) => return Some(Err(error)),
    };
    match entry.kind {
        ClipType::Video => {
            let target = poster_target_time(time_secs);
            let poster_path = timed_poster_path_for(engine.cache_root(), &key, target);
            let (poster_path, poster_w, poster_h, poster_time) =
                match read_cached_poster(&poster_path, target)? {
                    Ok(cached) => cached,
                    Err(error) => return Some(Err(error)),
                };
            let sprite_meta = if include_sprite {
                let limit = sprite_frame_limit(max_frames);
                match read_cached_sprite_meta(engine.cache_root(), &key) {
                    Some(mut meta) => {
                        meta.times.truncate(limit);
                        Some(meta)
                    }
                    None if video_thumbnail_times(entry.duration)
                        .into_iter()
                        .take(limit)
                        .next()
                        .is_none() =>
                    {
                        None
                    }
                    None => return None,
                }
            } else {
                None
            };
            let sprite_path = sprite_path_for(engine.cache_root(), &key);
            Some(Ok(ThumbnailDto {
                media_ref: entry.id.clone(),
                kind: entry.kind,
                thumbnail_path: Some(poster_path.to_string_lossy().into_owned()),
                sprite_path: if include_sprite && sprite_path.is_file() {
                    Some(sprite_path.to_string_lossy().into_owned())
                } else {
                    None
                },
                tile_width: sprite_meta
                    .as_ref()
                    .map(|meta| meta.tile_width)
                    .or(Some(poster_w)),
                tile_height: sprite_meta
                    .as_ref()
                    .map(|meta| meta.tile_height)
                    .or(Some(poster_h)),
                columns: sprite_meta.as_ref().map(|meta| meta.columns).or(Some(1)),
                times: sprite_meta
                    .map(|meta| meta.times)
                    .unwrap_or_else(|| vec![poster_time]),
            }))
        }
        ClipType::Image => {
            let poster_path = poster_path_for(engine.cache_root(), &key);
            if !poster_path.exists() {
                return None;
            }
            let (tile_width, tile_height) = image::image_dimensions(&poster_path)
                .map(|(width, height)| (Some(width), Some(height)))
                .unwrap_or((None, None));
            Some(Ok(ThumbnailDto {
                media_ref: entry.id.clone(),
                kind: entry.kind,
                thumbnail_path: Some(poster_path.to_string_lossy().into_owned()),
                sprite_path: None,
                tile_width,
                tile_height,
                columns: Some(1),
                times: vec![0.0],
            }))
        }
        _ => Some(Ok(empty_thumbnail_dto(entry))),
    }
}

/// Probe `path` via the engine, mapping ffprobe facts to [`ProbedMedia`]. Probe
/// failures (no ffprobe, unreadable file) degrade to defaults so a single bad
/// file never sinks a batch import.
pub(crate) fn probe_media(engine: &MediaEngine, path: &Path) -> ProbedMedia {
    engine
        .probe(path)
        .map(media_probe_to_core)
        .unwrap_or_default()
}

fn media_probe_to_core(probe: opentake_media::MediaProbe) -> ProbedMedia {
    ProbedMedia {
        duration_secs: probe.duration_secs,
        width: probe.width.map(|width| width as i32),
        height: probe.height.map(|height| height as i32),
        fps: probe.fps,
        has_audio: probe.has_audio,
        color: probe.color,
    }
}

/// Trusted facts emitted by the producer that wrote a reserved save-as output.
/// Generated media never needs to be reopened by pathname just to rediscover
/// facts the encoder/WAV writer already knows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SavedMediaMetadata {
    Video(crate::export::ExportSummary),
    Wav {
        sample_count: usize,
        sample_rate: u32,
    },
}

impl SavedMediaMetadata {
    fn to_probe(&self) -> Result<ProbedMedia, String> {
        match self {
            Self::Video(summary) => {
                if summary.width == 0
                    || summary.height == 0
                    || summary.fps <= 0
                    || summary.frame_count <= 0
                {
                    return Err("invalid completed video metadata".to_string());
                }
                let width = i32::try_from(summary.width)
                    .map_err(|_| "completed video width exceeds manifest limits")?;
                let height = i32::try_from(summary.height)
                    .map_err(|_| "completed video height exceeds manifest limits")?;
                Ok(ProbedMedia {
                    duration_secs: f64::from(summary.frame_count) / f64::from(summary.fps),
                    width: Some(width),
                    height: Some(height),
                    fps: Some(f64::from(summary.fps)),
                    has_audio: summary.has_audio,
                    color: None,
                })
            }
            Self::Wav {
                sample_count,
                sample_rate,
            } => {
                if *sample_rate == 0 {
                    return Err("invalid completed WAV sample rate".to_string());
                }
                Ok(ProbedMedia {
                    duration_secs: *sample_count as f64 / f64::from(*sample_rate),
                    width: None,
                    height: None,
                    fps: None,
                    has_audio: true,
                    color: None,
                })
            }
        }
    }
}

/// Display name for an imported file: its stem, or the full file name when there
/// is no stem (mirrors upstream `url.deletingPathExtension().lastPathComponent`).
pub(crate) fn display_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The full file name (with extension) for a skipped-file report — what the user
/// sees in a picker (mirrors upstream `url.lastPathComponent` in the toast).
fn display_file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Map a MIME type to the file extension the imported asset is written with.
/// 1:1 port of upstream `ToolExecutor+Import.fileExtension(forMime:)` — the
/// accepted set the agent's `import_media` (bytes / url override) validates
/// against. `json`/Lottie is intentionally excluded from the import white-list
/// downstream, but the mapping is kept for parity with upstream's table.
pub(crate) fn file_extension_for_mime(mime: &str) -> Option<&'static str> {
    match mime.to_ascii_lowercase().as_str() {
        "video/mp4" | "video/mpeg4" => Some("mp4"),
        "video/quicktime" => Some("mov"),
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/aac" => Some("aac"),
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => Some("m4a"),
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/tiff" => Some("tiff"),
        "image/heic" | "image/heif" => Some("heic"),
        _ => None,
    }
}

/// The accepted-MIME error line upstream raises for an unsupported `mimeType`
/// (`ToolExecutor+Import`). Centralized so bytes / url imports share the wording.
pub(crate) const IMPORT_ACCEPTED_MIMES: &str =
    "Accepted: video/mp4, video/quicktime, audio/mpeg, audio/wav, audio/aac, audio/mp4, image/png, image/jpeg, image/tiff, image/heic.";

/// Import one file into the core, probing it first. Returns the created entry, or
/// `None` when the extension is not importable (the file is skipped, not an
/// error — matches upstream's per-file tolerance during folder/batch import).
///
pub(crate) fn import_one(
    core: &AppCore,
    engine: &MediaEngine,
    path: &Path,
) -> Result<Option<MediaManifestEntry>, CoreError> {
    if importable_clip_type(path).is_none() {
        return Ok(None);
    }
    let probe = probe_media(engine, path);
    // `import_media_file` re-validates the extension; the type check above only
    // lets us skip probing unsupported files.
    let entry = core.import_media_file(path, display_name(path), &probe)?;
    Ok(Some(entry))
}

fn import_cancel_checkpoint(
    cancel: Option<&opentake_media::MediaCancelToken>,
) -> Result<(), CoreError> {
    if cancel.is_some_and(opentake_media::MediaCancelToken::checkpoint) {
        Err(CoreError::Media("media import was cancelled".to_string()))
    } else {
        Ok(())
    }
}

/// Admit an imported asset's small grid poster to the project-scoped scheduler.
/// The post-import snapshot proves the entry still belongs to the epoch being
/// scheduled; if a project replacement won the race, old content is rejected.
fn schedule_import_poster(
    core: &AppCore,
    engine: &MediaEngine,
    scheduler: &prewarm::PrewarmScheduler,
    entry: &MediaManifestEntry,
    path: &Path,
) -> ImportPrewarmDto {
    let snapshot = core.runtime_snapshot();
    let result = if !snapshot.media.entries.iter().any(|candidate| {
        candidate.id == entry.id && candidate.kind == entry.kind && candidate.source == entry.source
    }) {
        prewarm::PrewarmResult::StaleProject
    } else if let Ok(key) = cache_key_for(path) {
        let target = poster_path_for(engine.cache_root(), &key);
        scheduler.schedule_grid_poster(
            snapshot.project_epoch,
            entry.kind,
            key,
            path.to_path_buf(),
            target,
        )
    } else {
        prewarm::PrewarmResult::Cached
    };
    ImportPrewarmDto {
        media_ref: entry.id.clone(),
        result,
    }
}

#[cfg(test)]
pub(crate) fn import_saved_media(
    core: &AppCore,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    expected_project_epoch: u64,
    expected_project_dir: &Path,
    path: &Path,
) -> Result<MediaListDto, String> {
    let probe = probe_media(engine, path);
    import_saved_media_with_hooks(
        SavedMediaImportContext {
            core,
            engine,
            prewarm,
            expected_project_epoch,
            expected_project_dir,
            path,
            probe: &probe,
        },
        (|| {}, || Ok(())),
    )
}

#[cfg(test)]
fn import_saved_media_with_before_transaction(
    core: &AppCore,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    expected_project_epoch: u64,
    expected_project_dir: &Path,
    path: &Path,
    before_transaction: impl FnOnce(),
) -> Result<MediaListDto, String> {
    let probe = probe_media(engine, path);
    import_saved_media_with_hooks(
        SavedMediaImportContext {
            core,
            engine,
            prewarm,
            expected_project_epoch,
            expected_project_dir,
            path,
            probe: &probe,
        },
        (before_transaction, || Ok(())),
    )
}

struct SavedMediaImportContext<'a> {
    core: &'a AppCore,
    engine: &'a MediaEngine,
    prewarm: &'a prewarm::PrewarmScheduler,
    expected_project_epoch: u64,
    expected_project_dir: &'a Path,
    path: &'a Path,
    // Save-as callers supply metadata from their encoder/WAV writer. This
    // transaction must never re-probe the exchangeable final path.
    probe: &'a ProbedMedia,
}

fn import_saved_media_with_hooks(
    context: SavedMediaImportContext<'_>,
    hooks: (impl FnOnce(), impl FnOnce() -> Result<(), String>),
) -> Result<MediaListDto, String> {
    let SavedMediaImportContext {
        core,
        engine,
        prewarm,
        expected_project_epoch,
        expected_project_dir,
        path,
        probe,
    } = context;
    path.strip_prefix(expected_project_dir)
        .ok()
        .filter(|relative| {
            let mut components = relative.components();
            components
                .next()
                .is_some_and(|component| component.as_os_str() == "media")
                && components.all(|component| matches!(component, std::path::Component::Normal(_)))
        })
        .ok_or("saved media output must be inside the expected project media directory")?;
    importable_clip_type(path).ok_or("failed to import saved media")?;
    let (before_transaction, postcondition) = hooks;
    before_transaction();
    let entry = core
        .import_media_file_for_project_checked(
            expected_project_epoch,
            expected_project_dir,
            path,
            display_name(path),
            probe,
            || postcondition().map_err(CoreError::Media),
        )
        .map_err(|error| error.to_string())?;
    let mut result = schedule_import_poster(core, engine, prewarm, &entry, path);
    if result.result == prewarm::PrewarmResult::Busy {
        // One bounded re-attempt: a single saved-media import racing a
        // saturated prewarm queue may catch a slot the workers just freed.
        result = schedule_import_poster(core, engine, prewarm, &entry, path);
    }
    Ok(MediaListDto::from_core_with_import_results(
        core,
        Some(engine.cache_root()),
        Vec::new(),
        vec![result],
    ))
}

pub(crate) struct SavedMediaFinalizationContext<'a> {
    pub(crate) core: &'a AppCore,
    pub(crate) engine: &'a MediaEngine,
    pub(crate) prewarm: &'a prewarm::PrewarmScheduler,
    pub(crate) expected_project_epoch: u64,
    pub(crate) expected_project_dir: &'a Path,
    pub(crate) metadata: SavedMediaMetadata,
    pub(crate) on_progress: &'a dyn Fn(i32, i32),
}

pub(crate) fn finalize_saved_media(
    context: SavedMediaFinalizationContext<'_>,
    output: crate::export::ProjectMediaOutput,
    guard: &mut crate::export::ExportGuard<'_>,
) -> Result<MediaListDto, String> {
    finalize_saved_media_with_hooks(context, output, guard, (|| {}, || {}, || {}, || {}))
}

fn finalize_saved_media_with_hooks(
    context: SavedMediaFinalizationContext<'_>,
    output: crate::export::ProjectMediaOutput,
    guard: &mut crate::export::ExportGuard<'_>,
    hooks: (impl FnOnce(), impl FnOnce(), impl FnOnce(), impl FnOnce()),
) -> Result<MediaListDto, String> {
    let SavedMediaFinalizationContext {
        core,
        engine,
        prewarm,
        expected_project_epoch,
        expected_project_dir,
        metadata,
        on_progress,
    } = context;
    let (after_sync, after_metadata, before_transaction, before_commit) = hooks;
    output.prepare_commit_cancellable(guard, after_sync)?;
    let probe = metadata.to_probe()?;
    after_metadata();
    guard.checkpoint()?;
    let path = output.path().to_path_buf();
    let result = import_saved_media_with_hooks(
        SavedMediaImportContext {
            core,
            engine,
            prewarm,
            expected_project_epoch,
            expected_project_dir,
            path: &path,
            probe: &probe,
        },
        (before_transaction, || {
            output.verify_identity()?;
            before_commit();
            guard.commit()
        }),
    )?;
    let _committed_path = output.mark_kept();
    on_progress(
        crate::export::AUDIO_PROGRESS_TOTAL,
        crate::export::AUDIO_PROGRESS_TOTAL,
    );
    Ok(result)
}

/// `import_folder`: bring a local directory into the library.
///
/// - `recursive = false` (default): flat — import the top-level media files into
///   the library root (no folders), as before.
/// - `recursive = true`: **mirror the directory tree** (剪映-style, #49) — create
///   a library folder for the selected directory and each nested subdirectory,
///   and import each file into the folder mirroring its on-disk location. Empty
///   directories still create their folder. Files are visited in
///   case-insensitive name order so ids mint deterministically.
#[tauri::command]
pub fn import_folder(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    prewarm: State<'_, prewarm::PrewarmScheduler>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    path: String,
    recursive: Option<bool>,
) -> Result<MediaListDto, String> {
    let _activity = begin_direct_media_project_write(&admission)?;
    import_folder_impl(&core, media.engine(), &prewarm, path, recursive)
}

fn import_folder_impl(
    core: &AppCore,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    path: String,
    recursive: Option<bool>,
) -> Result<MediaListDto, String> {
    core.ensure_project_mutable().map_err(|e| e.to_string())?;
    let project = core.runtime_snapshot();
    let project_dir = project
        .project_dir
        .as_ref()
        .ok_or_else(|| "no project open".to_string())?;
    let root = PathBuf::from(&path);
    let recursive = recursive.unwrap_or(false);
    let mut planning_checkpoint = |_| {};
    let prepared = prepare_directory_import(
        engine,
        &root,
        recursive,
        None,
        None,
        DIRECTORY_IMPORT_LIMITS,
        &mut planning_checkpoint,
    )
    .map_err(|error| CoreError::from(error).to_string())?;
    let PreparedDirectoryImport {
        root,
        snapshot,
        plan,
        skipped,
        recursive,
        limits,
    } = prepared;
    let committed = if plan.is_empty() {
        Vec::new()
    } else {
        core.import_media_batch_for_project_persisted_checked(
            project.project_epoch,
            project_dir,
            plan,
            || {
                root.verify_snapshot(&snapshot, recursive, None, limits)
                    .map_err(CoreError::from)
            },
        )
        .map_err(|e| e.to_string())?
    };
    let prewarm_results = schedule_committed_posters(core, engine, prewarm, &committed);
    Ok(MediaListDto::from_core_with_import_results(
        core,
        Some(engine.cache_root()),
        skipped,
        prewarm_results,
    ))
}

/// Recursively mirror `dir` into the library: create a folder for `dir` (nested
/// under `parent_folder_id`), import its direct media files into that folder, and
/// recurse into subdirectories. Hidden entries (dot-prefixed) are skipped. Names
/// of non-importable visible files are appended to `skipped` so the caller can
/// toast them.
#[allow(dead_code)]
pub(crate) fn mirror_dir(
    core: &AppCore,
    engine: &MediaEngine,
    dir: &Path,
    parent_folder_id: Option<String>,
    skipped: &mut Vec<String>,
) -> Result<(), CoreError> {
    mirror_dir_cancellable(
        core,
        engine,
        dir,
        parent_folder_id,
        skipped,
        &opentake_media::MediaCancelToken::new(),
    )
}

pub(crate) fn mirror_dir_cancellable(
    core: &AppCore,
    engine: &MediaEngine,
    dir: &Path,
    parent_folder_id: Option<String>,
    skipped: &mut Vec<String>,
    cancel: &opentake_media::MediaCancelToken,
) -> Result<(), CoreError> {
    mirror_dir_cancellable_with_hook(core, engine, dir, parent_folder_id, skipped, cancel, || {})
}

fn mirror_dir_cancellable_with_hook(
    core: &AppCore,
    engine: &MediaEngine,
    dir: &Path,
    parent_folder_id: Option<String>,
    skipped: &mut Vec<String>,
    cancel: &opentake_media::MediaCancelToken,
    before_commit: impl FnOnce(),
) -> Result<(), CoreError> {
    let mut planning_checkpoint = |_| {};
    mirror_dir_cancellable_with_hooks(
        core,
        engine,
        dir,
        parent_folder_id,
        skipped,
        cancel,
        DIRECTORY_IMPORT_LIMITS,
        &mut planning_checkpoint,
        before_commit,
    )
}

#[allow(clippy::too_many_arguments)]
fn mirror_dir_cancellable_with_hooks(
    core: &AppCore,
    engine: &MediaEngine,
    dir: &Path,
    parent_folder_id: Option<String>,
    skipped: &mut Vec<String>,
    cancel: &opentake_media::MediaCancelToken,
    limits: DirectoryImportLimits,
    planning_checkpoint: &mut dyn FnMut(usize),
    before_commit: impl FnOnce(),
) -> Result<(), CoreError> {
    import_cancel_checkpoint(Some(cancel))?;
    core.ensure_project_mutable()?;
    let project = core.runtime_snapshot();
    let project_dir = project.project_dir.ok_or(CoreError::NoProjectOpen)?;
    let prepared = prepare_directory_import(
        engine,
        dir,
        true,
        parent_folder_id.map(PreparedMediaFolderRef::Existing),
        Some(cancel),
        limits,
        planning_checkpoint,
    )
    .map_err(CoreError::from)?;
    let PreparedDirectoryImport {
        root,
        snapshot,
        plan,
        skipped: prepared_skipped,
        recursive,
        limits,
    } = prepared;
    before_commit();
    import_cancel_checkpoint(Some(cancel))?;
    core.import_media_batch_for_project_persisted_checked(
        project.project_epoch,
        &project_dir,
        plan,
        || {
            import_cancel_checkpoint(Some(cancel))?;
            root.verify_snapshot(&snapshot, recursive, Some(cancel), limits)
                .map_err(CoreError::from)
        },
    )?;
    skipped.extend(prepared_skipped);
    Ok(())
}

/// Beta bounds for one selected directory import. They cap both scanner work
/// and the all-at-once manifest transaction before any project state changes.
pub(crate) const DIRECTORY_IMPORT_MAX_DEPTH: usize = 32;
pub(crate) const DIRECTORY_IMPORT_MAX_ENTRIES: usize = 10_000;
pub(crate) const DIRECTORY_IMPORT_MAX_FILES: usize = 5_000;
pub(crate) const DIRECTORY_IMPORT_MAX_PLAN_OPERATIONS: usize = 7_500;
pub(crate) const DIRECTORY_IMPORT_MAX_AGGREGATE_BYTES: u64 = 100 * 1024 * 1024 * 1024;
const DIRECTORY_IMPORT_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug)]
struct DirectoryImportLimits {
    max_depth: usize,
    max_entries: usize,
    max_files: usize,
    max_plan_operations: usize,
    max_aggregate_bytes: u64,
}

const DIRECTORY_IMPORT_LIMITS: DirectoryImportLimits = DirectoryImportLimits {
    max_depth: DIRECTORY_IMPORT_MAX_DEPTH,
    max_entries: DIRECTORY_IMPORT_MAX_ENTRIES,
    max_files: DIRECTORY_IMPORT_MAX_FILES,
    max_plan_operations: DIRECTORY_IMPORT_MAX_PLAN_OPERATIONS,
    max_aggregate_bytes: DIRECTORY_IMPORT_MAX_AGGREGATE_BYTES,
};

#[derive(Debug)]
enum DirectoryImportError {
    Cancelled,
    RootNotDirectory(PathBuf),
    SymlinkOrReparse(PathBuf),
    UnsupportedEntryType(PathBuf),
    Cycle(PathBuf),
    LimitExceeded {
        resource: &'static str,
        limit: u64,
    },
    NamespaceChanged,
    Io {
        action: &'static str,
        path: PathBuf,
        reason: String,
    },
}

impl std::fmt::Display for DirectoryImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("media import was cancelled"),
            Self::RootNotDirectory(path) => write!(
                formatter,
                "directory_import_root_not_directory: {}",
                path.display()
            ),
            Self::SymlinkOrReparse(path) => write!(
                formatter,
                "directory_import_symlink_or_reparse_rejected: {}",
                path.display()
            ),
            Self::UnsupportedEntryType(path) => write!(
                formatter,
                "directory_import_unsupported_entry_type: {}",
                path.display()
            ),
            Self::Cycle(path) => write!(
                formatter,
                "directory_import_cycle_detected: {}",
                path.display()
            ),
            Self::LimitExceeded { resource, limit } => write!(
                formatter,
                "directory_import_{resource}_limit_exceeded: maximum {limit}"
            ),
            Self::NamespaceChanged => {
                formatter.write_str("directory_import_namespace_changed_before_commit")
            }
            Self::Io {
                action,
                path,
                reason,
            } => write!(
                formatter,
                "directory_import_{action}_failed: {}: {reason}",
                path.display()
            ),
        }
    }
}

impl From<DirectoryImportError> for CoreError {
    fn from(error: DirectoryImportError) -> Self {
        CoreError::Media(error.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DirectorySnapshotKind {
    RegularFile,
    Directory,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DirectorySnapshotEntry {
    relative_path: PathBuf,
    kind: DirectorySnapshotKind,
    identity: u64,
    bytes: u64,
}

struct DirectoryImportRoot {
    path: PathBuf,
    namespace_parent: Dir,
    name: OsString,
    dir: Dir,
    identity: u64,
}

impl DirectoryImportRoot {
    fn open(path: &Path) -> Result<Self, DirectoryImportError> {
        let (parent_path, name) = match path.file_name() {
            Some(name) => (
                path.parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new(".")),
                name.to_owned(),
            ),
            None if path.has_root() => (path, OsString::from(".")),
            None => return Err(DirectoryImportError::RootNotDirectory(path.to_path_buf())),
        };
        let namespace_parent =
            Dir::open_ambient_dir(parent_path, ambient_authority()).map_err(|error| {
                DirectoryImportError::Io {
                    action: "root_parent_open",
                    path: parent_path.to_path_buf(),
                    reason: error.to_string(),
                }
            })?;
        let metadata =
            namespace_parent
                .symlink_metadata(&name)
                .map_err(|error| DirectoryImportError::Io {
                    action: "root_metadata",
                    path: path.to_path_buf(),
                    reason: error.to_string(),
                })?;
        if capability_metadata_is_symlink_or_reparse(&metadata) {
            return Err(DirectoryImportError::SymlinkOrReparse(path.to_path_buf()));
        }
        if !metadata.is_dir() {
            return Err(DirectoryImportError::RootNotDirectory(path.to_path_buf()));
        }
        let dir = namespace_parent.open_dir_nofollow(&name).map_err(|error| {
            DirectoryImportError::Io {
                action: "root_open_nofollow",
                path: path.to_path_buf(),
                reason: error.to_string(),
            }
        })?;
        let retained_metadata = dir
            .dir_metadata()
            .map_err(|error| DirectoryImportError::Io {
                action: "root_retained_metadata",
                path: path.to_path_buf(),
                reason: error.to_string(),
            })?;
        if capability_metadata_is_symlink_or_reparse(&retained_metadata)
            || !retained_metadata.is_dir()
        {
            return Err(DirectoryImportError::SymlinkOrReparse(path.to_path_buf()));
        }
        let identity = directory_identity(&dir, path)?;
        Ok(Self {
            path: path.to_path_buf(),
            namespace_parent,
            name,
            dir,
            identity,
        })
    }

    fn reopen_current(&self) -> Result<Dir, DirectoryImportError> {
        let metadata = self
            .namespace_parent
            .symlink_metadata(&self.name)
            .map_err(|error| DirectoryImportError::Io {
                action: "root_revalidate_metadata",
                path: self.path.clone(),
                reason: error.to_string(),
            })?;
        if capability_metadata_is_symlink_or_reparse(&metadata) {
            return Err(DirectoryImportError::SymlinkOrReparse(self.path.clone()));
        }
        if !metadata.is_dir() {
            return Err(DirectoryImportError::NamespaceChanged);
        }
        let current = self
            .namespace_parent
            .open_dir_nofollow(&self.name)
            .map_err(|_| DirectoryImportError::NamespaceChanged)?;
        if directory_identity(&current, &self.path)? != self.identity {
            return Err(DirectoryImportError::NamespaceChanged);
        }
        Ok(current)
    }

    fn verify_snapshot(
        &self,
        expected: &[DirectorySnapshotEntry],
        recursive: bool,
        cancel: Option<&opentake_media::MediaCancelToken>,
        limits: DirectoryImportLimits,
    ) -> Result<(), DirectoryImportError> {
        import_cancel_checkpoint(cancel).map_err(|_| DirectoryImportError::Cancelled)?;
        let current = self.reopen_current()?;
        let mut noop = |_| {};
        let mut planner = DirectoryImportPlanner::new(None, cancel, limits, false, &mut noop);
        planner.visited.insert(self.identity);
        planner.scan_dir(&current, &self.path, Path::new(""), None, 0, recursive)?;
        planner.snapshot.sort();
        if planner.snapshot != expected {
            return Err(DirectoryImportError::NamespaceChanged);
        }
        Ok(())
    }
}

struct PreparedDirectoryImport {
    root: DirectoryImportRoot,
    snapshot: Vec<DirectorySnapshotEntry>,
    plan: Vec<PreparedMediaImportOp>,
    skipped: Vec<String>,
    recursive: bool,
    limits: DirectoryImportLimits,
}

struct PendingDirectoryEntry {
    name: OsString,
    kind: DirectorySnapshotKind,
}

struct DirectoryImportPlanner<'a> {
    engine: Option<&'a MediaEngine>,
    cancel: Option<&'a opentake_media::MediaCancelToken>,
    limits: DirectoryImportLimits,
    build_plan: bool,
    on_checkpoint: &'a mut dyn FnMut(usize),
    checkpoints: usize,
    entries: usize,
    files: usize,
    aggregate_bytes: u64,
    operations: usize,
    next_folder_key: u64,
    visited: HashSet<u64>,
    snapshot: Vec<DirectorySnapshotEntry>,
    plan: Vec<PreparedMediaImportOp>,
    skipped: Vec<String>,
}

impl<'a> DirectoryImportPlanner<'a> {
    fn new(
        engine: Option<&'a MediaEngine>,
        cancel: Option<&'a opentake_media::MediaCancelToken>,
        limits: DirectoryImportLimits,
        build_plan: bool,
        on_checkpoint: &'a mut dyn FnMut(usize),
    ) -> Self {
        Self {
            engine,
            cancel,
            limits,
            build_plan,
            on_checkpoint,
            checkpoints: 0,
            entries: 0,
            files: 0,
            aggregate_bytes: 0,
            operations: 0,
            next_folder_key: 0,
            visited: HashSet::new(),
            snapshot: Vec::new(),
            plan: Vec::new(),
            skipped: Vec::new(),
        }
    }

    fn checkpoint(&mut self) -> Result<(), DirectoryImportError> {
        self.checkpoints = self.checkpoints.saturating_add(1);
        (self.on_checkpoint)(self.checkpoints);
        import_cancel_checkpoint(self.cancel).map_err(|_| DirectoryImportError::Cancelled)
    }

    fn bump_limit(
        value: &mut usize,
        limit: usize,
        resource: &'static str,
    ) -> Result<(), DirectoryImportError> {
        *value = value.saturating_add(1);
        if *value > limit {
            return Err(DirectoryImportError::LimitExceeded {
                resource,
                limit: limit as u64,
            });
        }
        Ok(())
    }

    fn bump_operation(&mut self) -> Result<(), DirectoryImportError> {
        Self::bump_limit(
            &mut self.operations,
            self.limits.max_plan_operations,
            "planned_operations",
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_dir(
        &mut self,
        dir: &Dir,
        absolute_path: &Path,
        relative_path: &Path,
        parent: Option<PreparedMediaFolderRef>,
        depth: usize,
        recursive: bool,
    ) -> Result<(), DirectoryImportError> {
        self.checkpoint()?;
        if depth > self.limits.max_depth {
            return Err(DirectoryImportError::LimitExceeded {
                resource: "depth",
                limit: self.limits.max_depth as u64,
            });
        }

        let folder = if recursive {
            self.bump_operation()?;
            let key = self.next_folder_key;
            self.next_folder_key = self.next_folder_key.saturating_add(1);
            if self.build_plan {
                self.plan.push(PreparedMediaImportOp::CreateFolder {
                    key,
                    name: dir_name(absolute_path),
                    parent,
                });
            }
            Some(PreparedMediaFolderRef::Planned(key))
        } else {
            None
        };

        let entries = dir.entries().map_err(|error| DirectoryImportError::Io {
            action: "read_directory",
            path: absolute_path.to_path_buf(),
            reason: error.to_string(),
        })?;
        let mut files = Vec::new();
        let mut directories = Vec::new();
        for entry in entries {
            self.checkpoint()?;
            let entry = entry.map_err(|error| DirectoryImportError::Io {
                action: "read_directory_entry",
                path: absolute_path.to_path_buf(),
                reason: error.to_string(),
            })?;
            let name = entry.file_name();
            if !is_single_normal_component(&name) {
                return Err(DirectoryImportError::UnsupportedEntryType(
                    absolute_path.join(&name),
                ));
            }
            let child_path = absolute_path.join(&name);
            let metadata =
                dir.symlink_metadata(&name)
                    .map_err(|error| DirectoryImportError::Io {
                        action: "entry_metadata_nofollow",
                        path: child_path.clone(),
                        reason: error.to_string(),
                    })?;
            if capability_metadata_is_symlink_or_reparse(&metadata) {
                return Err(DirectoryImportError::SymlinkOrReparse(child_path));
            }
            let kind = if metadata.is_file() {
                DirectorySnapshotKind::RegularFile
            } else if metadata.is_dir() {
                DirectorySnapshotKind::Directory
            } else {
                return Err(DirectoryImportError::UnsupportedEntryType(child_path));
            };
            Self::bump_limit(&mut self.entries, self.limits.max_entries, "entries")?;
            let pending = PendingDirectoryEntry { name, kind };
            match kind {
                DirectorySnapshotKind::RegularFile => files.push(pending),
                DirectorySnapshotKind::Directory => directories.push(pending),
            }
        }
        files.sort_by(|left, right| directory_entry_name_cmp(&left.name, &right.name));
        directories.sort_by(|left, right| directory_entry_name_cmp(&left.name, &right.name));

        for entry in files {
            self.checkpoint()?;
            let child_path = absolute_path.join(&entry.name);
            let relative_child = relative_path.join(&entry.name);
            let (file, bytes) = open_regular_file_nofollow(dir, &entry.name, &child_path)?;
            Self::bump_limit(&mut self.files, self.limits.max_files, "files")?;
            self.aggregate_bytes = self.aggregate_bytes.checked_add(bytes).ok_or(
                DirectoryImportError::LimitExceeded {
                    resource: "aggregate_bytes",
                    limit: self.limits.max_aggregate_bytes,
                },
            )?;
            if self.aggregate_bytes > self.limits.max_aggregate_bytes {
                return Err(DirectoryImportError::LimitExceeded {
                    resource: "aggregate_bytes",
                    limit: self.limits.max_aggregate_bytes,
                });
            }
            self.snapshot.push(DirectorySnapshotEntry {
                relative_path: relative_child,
                kind: entry.kind,
                identity: file_identity(&file, &child_path)?,
                bytes,
            });

            if is_hidden_name(&entry.name) {
                continue;
            }
            if importable_clip_type(&child_path).is_some() {
                self.bump_operation()?;
                if self.build_plan {
                    self.checkpoint()?;
                    let engine = self.engine.ok_or_else(|| DirectoryImportError::Io {
                        action: "planner_invariant",
                        path: child_path.clone(),
                        reason: "missing media engine".to_string(),
                    })?;
                    let probe = probe_media_file(engine, &file, self.cancel);
                    self.checkpoint()?;
                    self.plan.push(PreparedMediaImportOp::ImportFile {
                        path: child_path.clone(),
                        name: display_name(&child_path),
                        probe,
                        folder: folder.clone(),
                    });
                }
            } else if self.build_plan {
                self.skipped.push(display_file_name(&child_path));
            }
        }

        for entry in directories {
            self.checkpoint()?;
            let child_path = absolute_path.join(&entry.name);
            let relative_child = relative_path.join(&entry.name);
            let child = open_directory_nofollow(dir, &entry.name, &child_path)?;
            let identity = directory_identity(&child, &child_path)?;
            self.snapshot.push(DirectorySnapshotEntry {
                relative_path: relative_child.clone(),
                kind: entry.kind,
                identity,
                bytes: 0,
            });
            if recursive && !is_hidden_name(&entry.name) {
                if !self.visited.insert(identity) {
                    return Err(DirectoryImportError::Cycle(child_path));
                }
                self.scan_dir(
                    &child,
                    &child_path,
                    &relative_child,
                    folder.clone(),
                    depth.saturating_add(1),
                    true,
                )?;
            }
        }
        Ok(())
    }
}

fn prepare_directory_import(
    engine: &MediaEngine,
    path: &Path,
    recursive: bool,
    parent: Option<PreparedMediaFolderRef>,
    cancel: Option<&opentake_media::MediaCancelToken>,
    limits: DirectoryImportLimits,
    planning_checkpoint: &mut dyn FnMut(usize),
) -> Result<PreparedDirectoryImport, DirectoryImportError> {
    let root = DirectoryImportRoot::open(path)?;
    let mut planner =
        DirectoryImportPlanner::new(Some(engine), cancel, limits, true, planning_checkpoint);
    planner.visited.insert(root.identity);
    planner.scan_dir(&root.dir, path, Path::new(""), parent, 0, recursive)?;
    planner.snapshot.sort();
    Ok(PreparedDirectoryImport {
        root,
        snapshot: planner.snapshot,
        plan: planner.plan,
        skipped: planner.skipped,
        recursive,
        limits,
    })
}

pub(crate) fn capability_metadata_is_symlink_or_reparse(metadata: &cap_std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        windows_file_attributes_are_reparse(metadata.file_attributes())
    }
    #[cfg(not(windows))]
    false
}

#[cfg(windows)]
pub(crate) fn windows_file_attributes_are_reparse(attributes: u32) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn directory_identity(dir: &Dir, path: &Path) -> Result<u64, DirectoryImportError> {
    let file = dir
        .try_clone()
        .map_err(|error| DirectoryImportError::Io {
            action: "directory_identity_clone",
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?
        .into_std_file();
    file_identity(&file, path)
}

fn file_identity(file: &std::fs::File, path: &Path) -> Result<u64, DirectoryImportError> {
    let retained = file.try_clone().map_err(|error| DirectoryImportError::Io {
        action: "entry_identity_clone",
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    let identity = FileIdentity::from_file(retained).map_err(|error| DirectoryImportError::Io {
        action: "entry_identity",
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    // `same-file` hashes the stable device/inode (Unix) or volume/file-index
    // key (Windows). A theoretical hash collision only causes a fail-closed
    // false cycle/snapshot mismatch; it cannot admit a repeated identity.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    identity.hash(&mut hasher);
    Ok(hasher.finish())
}

fn open_directory_nofollow(
    parent: &Dir,
    name: &OsStr,
    path: &Path,
) -> Result<Dir, DirectoryImportError> {
    let metadata = parent
        .symlink_metadata(name)
        .map_err(|error| DirectoryImportError::Io {
            action: "directory_metadata_nofollow",
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    if capability_metadata_is_symlink_or_reparse(&metadata) {
        return Err(DirectoryImportError::SymlinkOrReparse(path.to_path_buf()));
    }
    if !metadata.is_dir() {
        return Err(DirectoryImportError::NamespaceChanged);
    }
    let child = parent
        .open_dir_nofollow(name)
        .map_err(|error| DirectoryImportError::Io {
            action: "directory_open_nofollow",
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    let retained = child
        .dir_metadata()
        .map_err(|error| DirectoryImportError::Io {
            action: "directory_retained_metadata",
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    if capability_metadata_is_symlink_or_reparse(&retained) || !retained.is_dir() {
        return Err(DirectoryImportError::NamespaceChanged);
    }
    Ok(child)
}

fn open_regular_file_nofollow(
    parent: &Dir,
    name: &OsStr,
    path: &Path,
) -> Result<(std::fs::File, u64), DirectoryImportError> {
    let metadata = parent
        .symlink_metadata(name)
        .map_err(|error| DirectoryImportError::Io {
            action: "file_metadata_nofollow",
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    if capability_metadata_is_symlink_or_reparse(&metadata) {
        return Err(DirectoryImportError::SymlinkOrReparse(path.to_path_buf()));
    }
    if !metadata.is_file() {
        return Err(DirectoryImportError::UnsupportedEntryType(
            path.to_path_buf(),
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }
    let file = parent
        .open_with(name, &options)
        .map_err(|error| DirectoryImportError::Io {
            action: "file_open_nofollow",
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    let retained = file.metadata().map_err(|error| DirectoryImportError::Io {
        action: "file_retained_metadata",
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    if capability_metadata_is_symlink_or_reparse(&retained) {
        return Err(DirectoryImportError::SymlinkOrReparse(path.to_path_buf()));
    }
    if !retained.is_file() {
        return Err(DirectoryImportError::UnsupportedEntryType(
            path.to_path_buf(),
        ));
    }
    let bytes = retained.len();
    Ok((file.into_std(), bytes))
}

fn probe_media_file(
    engine: &MediaEngine,
    file: &std::fs::File,
    cancel: Option<&opentake_media::MediaCancelToken>,
) -> ProbedMedia {
    let local_cancel = opentake_media::MediaCancelToken::new();
    engine
        .probe_file_cancellable(
            file,
            cancel.unwrap_or(&local_cancel),
            DIRECTORY_IMPORT_PROBE_TIMEOUT,
        )
        .map(media_probe_to_core)
        .unwrap_or_default()
}

fn is_single_normal_component(name: &OsStr) -> bool {
    matches!(
        Path::new(name).components().collect::<Vec<_>>().as_slice(),
        [std::path::Component::Normal(_)]
    )
}

fn is_hidden_name(name: &OsStr) -> bool {
    name.to_str()
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

fn directory_entry_name_cmp(left: &OsStr, right: &OsStr) -> std::cmp::Ordering {
    left.to_string_lossy()
        .to_lowercase()
        .cmp(&right.to_string_lossy().to_lowercase())
        .then_with(|| left.to_string_lossy().cmp(&right.to_string_lossy()))
}

/// Schedule the grid poster for every committed import, re-attempting the ones
/// the bounded prewarm queue rejected mid-batch until they fit. The three
/// workers drain the queue while a large folder import commits, so a tail poster
/// that lost the queue race fits on a later attempt — without this a 50+ file
/// import permanently drops its last posters until a card scroll happens to
/// request them lazily. The drain wait is bounded so a saturated queue can never
/// stall the import command.
fn schedule_committed_posters(
    core: &AppCore,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    committed: &[CommittedMediaImport],
) -> Vec<ImportPrewarmDto> {
    let mut results: Vec<ImportPrewarmDto> = committed
        .iter()
        .map(|imported| {
            schedule_import_poster(core, engine, prewarm, &imported.entry, &imported.path)
        })
        .collect();
    let mut busy: Vec<usize> = results
        .iter()
        .enumerate()
        .filter(|(_, dto)| dto.result == prewarm::PrewarmResult::Busy)
        .map(|(index, _)| index)
        .collect();
    if !busy.is_empty() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !busy.is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
            busy.retain(|&index| {
                let imported = &committed[index];
                let dto =
                    schedule_import_poster(core, engine, prewarm, &imported.entry, &imported.path);
                if dto.result == prewarm::PrewarmResult::Busy {
                    true
                } else {
                    results[index].result = dto.result;
                    false
                }
            });
        }
    }
    results
}

/// Directory display name (its last path component), falling back to "folder".
fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "folder".to_string())
}

/// One directory's visible media files + subdirectories (each sorted by
/// case-insensitive name), plus the names of visible non-importable files.
/// Dot-prefixed (hidden) entries are ignored entirely — an unsupported *type* is
/// a skip the user should hear about; a hidden dotfile is not.
#[cfg(test)]
fn list_dir(dir: &Path) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<String>) {
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    let mut skipped = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (files, subdirs, skipped);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.starts_with('.'))
            .unwrap_or(false);
        if hidden {
            continue;
        }
        if path.is_dir() {
            subdirs.push(path);
        } else if importable_clip_type(&path).is_some() {
            files.push(path);
        } else {
            skipped.push(display_file_name(&path));
        }
    }
    let by_name = |a: &PathBuf, b: &PathBuf| {
        let an = a.file_name().map(|s| s.to_string_lossy().to_lowercase());
        let bn = b.file_name().map(|s| s.to_string_lossy().to_lowercase());
        an.cmp(&bn)
    };
    files.sort_by(by_name);
    subdirs.sort_by(by_name);
    skipped.sort_by_key(|s| s.to_lowercase());
    (files, subdirs, skipped)
}

/// The top-level importable media files + the names of unsupported files in
/// `dir`, for a flat (non-recursive) folder import. Subdirectories are ignored
/// (as before); their contents are neither imported nor reported skipped.
#[cfg(test)]
fn list_top_level(dir: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let (files, _subdirs, skipped) = list_dir(dir);
    (files, skipped)
}

/// `import_media`: import an explicit list of file paths, returning the updated
/// catalog. Unsupported or unreadable paths are skipped (not fatal); the returned
/// list reflects whatever imported successfully and carries the names of skipped
/// unsupported files in `skipped` so the front end can toast them (upstream
/// `mediaPanelToast`) instead of dropping them silently.
pub(crate) const EXPLICIT_IMPORT_MAX_FILES: usize = 5_000;
pub(crate) const EXPLICIT_IMPORT_MAX_AGGREGATE_BYTES: u64 = 100 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
struct ExplicitImportLimits {
    max_files: usize,
    max_aggregate_bytes: u64,
}

const EXPLICIT_IMPORT_LIMITS: ExplicitImportLimits = ExplicitImportLimits {
    max_files: EXPLICIT_IMPORT_MAX_FILES,
    max_aggregate_bytes: EXPLICIT_IMPORT_MAX_AGGREGATE_BYTES,
};

struct RetainedExplicitImportSource {
    requested_path: PathBuf,
    final_path: PathBuf,
    identity: FileIdentity,
    admitted_bytes: u64,
}

impl RetainedExplicitImportSource {
    fn open(path: &Path) -> std::io::Result<Self> {
        // Reuse the local-asset boundary: Unix opens are non-blocking/no-follow;
        // Windows opens use OPEN_NO_RECALL. The returned handle, rather than a
        // later pathname reopen, supplies both metadata and ffprobe bytes.
        let (file, final_path) = crate::safe_asset_protocol::open_retained_regular_file(path)?;
        let admitted_bytes = file.metadata()?.len();
        let identity = FileIdentity::from_file(file)?;
        Ok(Self {
            requested_path: path.to_path_buf(),
            final_path,
            identity,
            admitted_bytes,
        })
    }

    fn verify_current_path(&self) -> Result<u64, CoreError> {
        let current = crate::safe_asset_protocol::open_retained_regular_file(&self.requested_path)
            .and_then(|(file, _)| {
                let bytes = file.metadata()?.len();
                let identity = FileIdentity::from_file(file)?;
                Ok((identity, bytes))
            });
        match current {
            Ok((current, bytes)) if current == self.identity && bytes == self.admitted_bytes => {
                Ok(bytes)
            }
            _ => Err(CoreError::Media(format!(
                "explicit_import_source_changed_before_commit: {}",
                self.requested_path.display()
            ))),
        }
    }
}

struct PreparedExplicitImportBatch {
    plan: Vec<PreparedMediaImportOp>,
    sources: Vec<RetainedExplicitImportSource>,
    skipped: Vec<String>,
}

fn prepare_explicit_import_batch(
    engine: &MediaEngine,
    paths: &[String],
    limits: ExplicitImportLimits,
) -> Result<PreparedExplicitImportBatch, String> {
    if paths.len() > limits.max_files {
        return Err(format!(
            "explicit_import_files_limit_exceeded: limit={}",
            limits.max_files
        ));
    }
    let mut aggregate_bytes = 0_u64;
    let mut plan = Vec::new();
    let mut sources = Vec::new();
    let mut skipped = Vec::new();
    for path_text in paths {
        let requested_path = PathBuf::from(path_text);
        let Ok(source) = RetainedExplicitImportSource::open(&requested_path) else {
            continue;
        };
        aggregate_bytes = aggregate_bytes
            .checked_add(source.admitted_bytes)
            .ok_or_else(|| "explicit_import_aggregate_bytes_limit_exceeded".to_string())?;
        if aggregate_bytes > limits.max_aggregate_bytes {
            return Err(format!(
                "explicit_import_aggregate_bytes_limit_exceeded: limit={}",
                limits.max_aggregate_bytes
            ));
        }
        if importable_clip_type(&source.final_path).is_none() {
            skipped.push(display_file_name(&source.final_path));
            continue;
        }
        let probe = probe_media_file(engine, source.identity.as_file(), None);
        plan.push(PreparedMediaImportOp::ImportFile {
            path: source.final_path.clone(),
            name: display_name(&source.final_path),
            probe,
            folder: None,
        });
        sources.push(source);
    }
    Ok(PreparedExplicitImportBatch {
        plan,
        sources,
        skipped,
    })
}

#[tauri::command]
pub fn import_media(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    prewarm: State<'_, prewarm::PrewarmScheduler>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    paths: Vec<String>,
) -> Result<MediaListDto, String> {
    let _activity = begin_direct_media_project_write(&admission)?;
    import_media_impl(&core, media.engine(), &prewarm, paths)
}

fn import_media_impl(
    core: &AppCore,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    paths: Vec<String>,
) -> Result<MediaListDto, String> {
    import_media_impl_with_options(core, engine, prewarm, paths, EXPLICIT_IMPORT_LIMITS, || {})
}

fn import_media_impl_with_options(
    core: &AppCore,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    paths: Vec<String>,
    limits: ExplicitImportLimits,
    before_commit: impl FnOnce(),
) -> Result<MediaListDto, String> {
    core.ensure_project_mutable().map_err(|e| e.to_string())?;
    let project = core.runtime_snapshot();
    let project_dir = project
        .project_dir
        .as_ref()
        .ok_or_else(|| "no project open".to_string())?;
    let PreparedExplicitImportBatch {
        plan,
        sources,
        skipped,
    } = prepare_explicit_import_batch(engine, &paths, limits)?;
    before_commit();
    let committed = if plan.is_empty() {
        Vec::new()
    } else {
        core.import_media_batch_for_project_persisted_checked(
            project.project_epoch,
            project_dir,
            plan,
            || {
                let mut verified_aggregate_bytes = 0_u64;
                for source in &sources {
                    verified_aggregate_bytes = verified_aggregate_bytes
                        .checked_add(source.verify_current_path()?)
                        .ok_or_else(|| {
                            CoreError::Media(
                                "explicit_import_aggregate_bytes_limit_exceeded".to_string(),
                            )
                        })?;
                    if verified_aggregate_bytes > limits.max_aggregate_bytes {
                        return Err(CoreError::Media(format!(
                            "explicit_import_aggregate_bytes_limit_exceeded: limit={}",
                            limits.max_aggregate_bytes
                        )));
                    }
                }
                Ok(())
            },
        )
        .map_err(|e| e.to_string())?
    };
    let prewarm_results = schedule_committed_posters(core, engine, prewarm, &committed);
    Ok(MediaListDto::from_core_with_import_results(
        core,
        Some(engine.cache_root()),
        skipped,
        prewarm_results,
    ))
}

/// `get_media`: the current media catalog for the panel. Infallible.
#[tauri::command]
pub fn get_media<R: Runtime>(
    app: AppHandle<R>,
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
) -> MediaListDto {
    let mut catalog = MediaListDto::from_core(&core, Some(media.engine().cache_root()));
    if catalog.items.iter().any(|item| item.proxy_path.is_some()) {
        if let Ok(_activity) = media.begin_cache_write() {
            grant_catalog_proxy_asset_scope(&app, &mut catalog);
        } else {
            for item in &mut catalog.items {
                item.proxy_path = None;
            }
        }
    }
    catalog
}

/// Persist one project asset in the content-addressed global library and mirror
/// that identity in the current project manifest.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri injects project/media/library/update state
pub fn toggle_favorite(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    library: State<'_, LibraryState>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    asset_id: String,
    favorite: bool,
    expected_project_epoch: u64,
    expected_project_path: String,
) -> Result<MediaListDto, String> {
    let _activity = begin_direct_media_project_write(&admission)?;
    let _workflow = library.lock_workflow();
    toggle_favorite_impl_for_project(
        &core,
        media.engine().cache_root(),
        library.store()?,
        &asset_id,
        favorite,
        expected_project_epoch,
        Path::new(&expected_project_path),
    )
}

#[cfg(test)]
fn toggle_favorite_impl(
    core: &AppCore,
    cache_root: &Path,
    store: &LibraryStore,
    asset_id: &str,
    favorite: bool,
) -> Result<MediaListDto, String> {
    core.ensure_project_mutable()
        .map_err(|error| error.to_string())?;
    let project = core.runtime_snapshot();
    let project_dir = project
        .project_dir
        .ok_or_else(|| "save the project before changing global favorites".to_string())?;
    toggle_favorite_impl_for_project(
        core,
        cache_root,
        store,
        asset_id,
        favorite,
        project.project_epoch,
        &project_dir,
    )
}

#[derive(Clone, Copy)]
struct ExpectedFavoriteProject<'a> {
    epoch: u64,
    dir: &'a Path,
}

fn toggle_favorite_impl_for_project(
    core: &AppCore,
    cache_root: &Path,
    store: &LibraryStore,
    asset_id: &str,
    favorite: bool,
    expected_project_epoch: u64,
    expected_project_dir: &Path,
) -> Result<MediaListDto, String> {
    let mut events = DeferredCoreEvents::default();
    let result = {
        let _project_identity = core.lock_project_identity_workflow();
        toggle_favorite_impl_with(
            core,
            cache_root,
            store,
            asset_id,
            favorite,
            ExpectedFavoriteProject {
                epoch: expected_project_epoch,
                dir: expected_project_dir,
            },
            &mut events,
            |request| {
                store
                    .prepare_favorite(request)
                    .map_err(|error| error.to_string())
            },
        )
    };
    core.emit_deferred(events);
    result
}

#[allow(clippy::too_many_arguments)]
fn toggle_favorite_impl_with<F>(
    core: &AppCore,
    cache_root: &Path,
    store: &LibraryStore,
    asset_id: &str,
    favorite: bool,
    expected_project: ExpectedFavoriteProject<'_>,
    events: &mut DeferredCoreEvents,
    favorite_file: F,
) -> Result<MediaListDto, String>
where
    F: for<'a> FnOnce(&FavoriteRequest<'a>) -> Result<PreparedFavorite, String>,
{
    let project = core
        .mutable_runtime_snapshot_for_project(expected_project.epoch, expected_project.dir)
        .map_err(|error| error.to_string())?;
    let project_dir = project
        .project_dir
        .clone()
        .ok_or_else(|| "save the project before changing global favorites".to_string())?;
    let before = project.media;
    let entry = before
        .entries
        .iter()
        .find(|entry| entry.id == asset_id)
        .cloned()
        .ok_or_else(|| format!("media asset not found: {asset_id}"))?;

    if favorite {
        let source = resolve_source_path(&entry, Some(&project_dir))
            .ok_or_else(|| "media source could not be resolved".to_string())?;
        if !source.is_file() {
            return Err("media source is offline; relink before favoriting".to_string());
        }
        let kind = clip_type_name(entry.kind);
        let request = FavoriteRequest {
            source: &source,
            kind,
            category: None,
            favorited_at: now_epoch_secs(),
            thumb: None,
        };
        let prepared = favorite_file(&request)?;
        let library_id = prepared.entry().id.clone();
        let needs_publish = prepared.needs_publish();
        let changed = match core.set_media_global_favorite_for_project_deferred(
            project.project_epoch,
            &project_dir,
            asset_id,
            Some(library_id),
            events,
        ) {
            Ok(changed) => changed,
            Err(error) => return Err(error.to_string()),
        };
        if changed || needs_publish {
            if let Err(error) = core.save_media_manifest_for_project_deferred(
                project.project_epoch,
                &project_dir,
                events,
            ) {
                restore_project_favorites(
                    core,
                    project.project_epoch,
                    &project_dir,
                    &before,
                    events,
                );
                return Err(format!(
                    "global favorite mapping could not be saved: {error}"
                ));
            }
        }
        if let Err(error) = store.publish_favorite(prepared) {
            restore_project_favorites(core, project.project_epoch, &project_dir, &before, events);
            core.save_media_manifest_for_project_deferred(
                project.project_epoch,
                &project_dir,
                events,
            )
            .map_err(|rollback| {
                format!(
                    "global favorite could not be published: {error}; project mapping rollback could not be saved: {rollback}"
                )
            })?;
            return Err(format!("global favorite could not be published: {error}"));
        }
    } else {
        let library_id = match before.library_favorite_id(asset_id) {
            Some(id) => id.to_string(),
            None if before.is_favorite(asset_id) => {
                let source = resolve_source_path(&entry, Some(&project_dir))
                    .ok_or_else(|| "media source could not be resolved".to_string())?;
                if !source.is_file() {
                    return Err("favorite migration needs the source to be relinked".to_string());
                }
                store
                    .content_id(source)
                    .map_err(|error| error.to_string())?
            }
            None => return Ok(MediaListDto::from_core(core, Some(cache_root))),
        };
        store
            .remove(&library_id)
            .map_err(|error| format!("global favorite could not be removed: {error}"))?;
        let cleared = core
            .clear_media_global_favorite_id_for_project_deferred(
                project.project_epoch,
                &project_dir,
                &library_id,
                events,
            )
            .map_err(|error| error.to_string())?;
        let legacy_cleared = core
            .set_media_global_favorite_for_project_deferred(
                project.project_epoch,
                &project_dir,
                asset_id,
                None,
                events,
            )
            .map_err(|error| error.to_string())?;
        if cleared > 0 || legacy_cleared {
            if let Err(error) = core.save_media_manifest_for_project_deferred(
                project.project_epoch,
                &project_dir,
                events,
            ) {
                restore_project_favorites(
                    core,
                    project.project_epoch,
                    &project_dir,
                    &before,
                    events,
                );
                return Err(format!(
                    "project favorite mirror could not be saved: {error}"
                ));
            }
        }
    }
    Ok(MediaListDto::from_core(core, Some(cache_root)))
}

#[tauri::command]
pub fn sync_project_favorites(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    library: State<'_, LibraryState>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    legacy_asset_ids: Vec<String>,
    expected_project_epoch: u64,
    expected_project_path: String,
) -> Result<FavoriteSyncDto, String> {
    let _activity = begin_direct_media_project_write(&admission)?;
    let _workflow = library.lock_workflow();
    sync_project_favorites_impl_for_project(
        &core,
        media.engine().cache_root(),
        library.store()?,
        legacy_asset_ids,
        expected_project_epoch,
        Path::new(&expected_project_path),
    )
}

#[cfg(test)]
fn sync_project_favorites_impl(
    core: &AppCore,
    cache_root: &Path,
    store: &LibraryStore,
    legacy_asset_ids: Vec<String>,
) -> Result<FavoriteSyncDto, String> {
    let project = core.runtime_snapshot();
    let project_dir = project
        .project_dir
        .ok_or_else(|| "save the project before synchronizing global favorites".to_string())?;
    sync_project_favorites_impl_for_project(
        core,
        cache_root,
        store,
        legacy_asset_ids,
        project.project_epoch,
        &project_dir,
    )
}

fn sync_project_favorites_impl_for_project(
    core: &AppCore,
    cache_root: &Path,
    store: &LibraryStore,
    legacy_asset_ids: Vec<String>,
    expected_project_epoch: u64,
    expected_project_dir: &Path,
) -> Result<FavoriteSyncDto, String> {
    let mut events = DeferredCoreEvents::default();
    let result = {
        let _project_identity = core.lock_project_identity_workflow();
        sync_project_favorites_impl_with_events(
            core,
            cache_root,
            store,
            legacy_asset_ids,
            expected_project_epoch,
            expected_project_dir,
            &mut events,
        )
    };
    core.emit_deferred(events);
    result
}

fn sync_project_favorites_impl_with_events(
    core: &AppCore,
    cache_root: &Path,
    store: &LibraryStore,
    legacy_asset_ids: Vec<String>,
    expected_project_epoch: u64,
    expected_project_dir: &Path,
    events: &mut DeferredCoreEvents,
) -> Result<FavoriteSyncDto, String> {
    let project = core
        .mutable_runtime_snapshot_for_project(expected_project_epoch, expected_project_dir)
        .map_err(|error| error.to_string())?;
    let project_dir = project
        .project_dir
        .clone()
        .ok_or_else(|| "save the project before synchronizing global favorites".to_string())?;
    let before = project.media;
    let project_ids: HashSet<&str> = before
        .entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    let legacy_inputs: BTreeSet<String> = legacy_asset_ids
        .into_iter()
        .filter(|id| project_ids.contains(id.as_str()))
        .collect();
    let library_ids: HashSet<String> = store
        .entries()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|entry| entry.id)
        .collect();
    let stored_ids = store
        .stored_ids_verified()
        .map_err(|error| error.to_string())?;
    let mapped_at_start: HashSet<String> = before.favorite_library_ids.keys().cloned().collect();
    let mut migrated = BTreeSet::new();
    let mut failures = Vec::new();
    let mut changed = false;
    let mut pending_publications: Vec<(String, bool, PreparedFavorite)> = Vec::new();

    let stale_ids: BTreeSet<String> = before
        .favorite_library_ids
        .values()
        .filter(|id| !library_ids.contains(*id))
        .cloned()
        .collect();
    for library_id in stale_ids {
        let cleared = match core.clear_media_global_favorite_id_for_project_deferred(
            project.project_epoch,
            &project_dir,
            &library_id,
            events,
        ) {
            Ok(cleared) => cleared,
            Err(error) => {
                restore_project_favorites(
                    core,
                    project.project_epoch,
                    &project_dir,
                    &before,
                    events,
                );
                return Err(error.to_string());
            }
        };
        changed |= cleared > 0;
    }
    for (asset_id, library_id) in &before.favorite_library_ids {
        if !library_ids.contains(library_id) {
            if legacy_inputs.contains(asset_id) {
                migrated.insert(asset_id.clone());
            }
            continue;
        }
        let stored_exists = stored_ids.contains(library_id);
        let repair_result = if stored_exists {
            Ok(())
        } else {
            let entry = before
                .entries
                .iter()
                .find(|entry| entry.id == asset_id.as_str())
                .ok_or_else(|| format!("media asset not found: {asset_id}"));
            entry.and_then(|entry| {
                let source = resolve_source_path(entry, Some(&project_dir))
                    .ok_or_else(|| "media source could not be resolved".to_string())?;
                if !source.is_file() {
                    return Err("media source is offline; relink before favoriting".to_string());
                }
                store
                    .repair_stored_copy(library_id, &source)
                    .map_err(|error| error.to_string())
            })
        };
        match repair_result {
            Ok(()) if legacy_inputs.contains(asset_id) => {
                migrated.insert(asset_id.clone());
            }
            Ok(()) => {}
            Err(message) => failures.push(FavoriteSyncFailureDto {
                asset_id: asset_id.clone(),
                message,
            }),
        }
    }

    let mut candidates: BTreeSet<String> = before
        .favorites
        .iter()
        .filter(|id| !mapped_at_start.contains(*id))
        .cloned()
        .collect();
    candidates.extend(
        legacy_inputs
            .iter()
            .filter(|id| !mapped_at_start.contains(*id))
            .cloned(),
    );
    for asset_id in candidates {
        let Some(entry) = before.entries.iter().find(|entry| entry.id == asset_id) else {
            continue;
        };
        let result = (|| -> Result<PreparedFavorite, String> {
            let source = resolve_source_path(entry, Some(&project_dir))
                .ok_or_else(|| "media source could not be resolved".to_string())?;
            if !source.is_file() {
                return Err("media source is offline; relink before favoriting".to_string());
            }
            let kind = clip_type_name(entry.kind);
            let request = FavoriteRequest {
                source: &source,
                kind,
                category: None,
                favorited_at: now_epoch_secs(),
                thumb: None,
            };
            let prepared = store
                .prepare_favorite(&request)
                .map_err(|error| error.to_string())?;
            changed |= core
                .set_media_global_favorite_for_project_deferred(
                    project.project_epoch,
                    &project_dir,
                    &asset_id,
                    Some(prepared.entry().id.clone()),
                    events,
                )
                .map_err(|error| error.to_string())?;
            Ok(prepared)
        })();
        match result {
            Ok(prepared) => pending_publications.push((
                asset_id.clone(),
                legacy_inputs.contains(&asset_id),
                prepared,
            )),
            Err(message) => failures.push(FavoriteSyncFailureDto { asset_id, message }),
        }
    }

    if changed
        || pending_publications
            .iter()
            .any(|(_, _, item)| item.needs_publish())
    {
        if let Err(error) = core.save_media_manifest_for_project_deferred(
            project.project_epoch,
            &project_dir,
            events,
        ) {
            restore_project_favorites(core, project.project_epoch, &project_dir, &before, events);
            return Err(format!(
                "favorite synchronization could not be saved: {error}"
            ));
        }
    }
    let mut publish_rollbacks = Vec::new();
    for (asset_id, is_legacy, prepared) in pending_publications {
        match store.publish_favorite(prepared) {
            Ok(_) if is_legacy => {
                migrated.insert(asset_id);
            }
            Ok(_) => {}
            Err(error) => {
                failures.push(FavoriteSyncFailureDto {
                    asset_id: asset_id.clone(),
                    message: format!("global favorite could not be published: {error}"),
                });
                publish_rollbacks.push(asset_id);
            }
        }
    }
    if !publish_rollbacks.is_empty() {
        for asset_id in &publish_rollbacks {
            let mapping = before.library_favorite_id(asset_id).map(str::to_string);
            core.set_media_global_favorite_for_project_deferred(
                project.project_epoch,
                &project_dir,
                asset_id,
                mapping.clone(),
                events,
            )
            .map_err(|error| error.to_string())?;
            if mapping.is_none() && before.is_favorite(asset_id) {
                core.set_media_favorite_for_project_deferred(
                    project.project_epoch,
                    &project_dir,
                    std::slice::from_ref(asset_id),
                    true,
                    events,
                )
                .map_err(|error| error.to_string())?;
            }
        }
        core.save_media_manifest_for_project_deferred(project.project_epoch, &project_dir, events)
            .map_err(|error| {
                format!("failed favorite mappings could not be rolled back: {error}")
            })?;
    }
    Ok(FavoriteSyncDto {
        media: MediaListDto::from_core(core, Some(cache_root)),
        migrated_legacy_asset_ids: migrated.into_iter().collect(),
        failures,
    })
}

pub(crate) fn restore_project_favorites(
    core: &AppCore,
    project_epoch: u64,
    project_dir: &Path,
    before: &MediaManifest,
    events: &mut DeferredCoreEvents,
) {
    events.clear();
    for entry in &before.entries {
        let mapping = before.library_favorite_id(&entry.id).map(str::to_string);
        let _ = core.set_media_global_favorite_for_project_deferred(
            project_epoch,
            project_dir,
            &entry.id,
            mapping,
            events,
        );
        if before.is_favorite(&entry.id) && before.library_favorite_id(&entry.id).is_none() {
            let _ = core.set_media_favorite_for_project_deferred(
                project_epoch,
                project_dir,
                std::slice::from_ref(&entry.id),
                true,
                events,
            );
        }
    }
    events.clear();
}

pub(crate) fn clip_type_name(kind: ClipType) -> &'static str {
    match kind {
        ClipType::Video => "video",
        ClipType::Audio => "audio",
        ClipType::Image => "image",
        ClipType::Text => "text",
        ClipType::Lottie => "lottie",
    }
}

fn now_epoch_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

/// Build the render inputs for "save clip as media" (#91 §3.5): a single-clip
/// timeline (the clip re-based to frame 0 on a visible, unmuted track) plus a
/// manifest subset carrying only that clip's source entry. Pure — the caller
/// drives the GPU render/encode — so the framing is unit-testable. Also returns
/// the clip's media type (only video renders today). Errors if the clip id is
/// not on the timeline, or its source is missing from the manifest.
fn build_single_clip_export(
    timeline: &Timeline,
    manifest: &MediaManifest,
    clip_id: &str,
) -> Result<(Timeline, MediaManifest, ClipType), String> {
    let track = timeline
        .tracks
        .iter()
        .find(|t| t.clips.iter().any(|c| c.id == clip_id))
        .ok_or_else(|| format!("clip not found: {clip_id}"))?;
    let clip = track
        .clips
        .iter()
        .find(|c| c.id == clip_id)
        .expect("clip is present in the matched track");
    if clip.duration_frames <= 0 {
        return Err("clip duration must be greater than zero".to_string());
    }

    // One track — the clip's own, cloned to keep its type/props — holding only
    // this clip re-based to frame 0; forced visible + unmuted so export renders
    // it even if the source track was hidden/muted.
    let mut solo_track = track.clone();
    let mut solo_clip = clip.clone();
    solo_clip.start_frame = 0;
    solo_track.clips = vec![solo_clip];
    solo_track.hidden = false;
    solo_track.muted = false;

    // Clone-then-replace keeps every other Timeline field (fps/size/version…).
    let mut single_timeline = timeline.clone();
    single_timeline.tracks = vec![solo_track];

    // Manifest subset: only the clip's source (render metrics + decode need it,
    // nothing else does). Clone-then-retain preserves the manifest version.
    let mut subset = manifest.clone();
    subset.entries.retain(|e| e.id == clip.media_ref);
    subset.folders.clear();
    subset.favorites.clear();
    if subset.entries.is_empty() {
        return Err(format!("media not found for clip: {}", clip.media_ref));
    }

    Ok((single_timeline, subset, clip.media_type))
}

/// `save_clip_as_media` (#91 §3.5 / 另存为媒体): render one timeline clip — with
/// its trims, speed, effects, color and text baked in — to a new `.mp4` in the
/// project's `media/` dir, then import it as a fresh asset so it shows up in the
/// panel. Reuses the export pipeline via a single-clip timeline plus the normal
/// import path. Returns the refreshed catalog.
///
/// Video clips only for now (audio/image save-as is a follow-up; basic audio
/// extraction already exists via `extract_audio`). Requires a saved project —
/// there must be a bundle `media/` dir to write into.
#[tauri::command]
pub fn save_clip_as_media(
    app: AppHandle,
    core: State<'_, AppCore>,
    control: State<'_, crate::export::ExportControl>,
    media: State<'_, MediaState>,
    prewarm: State<'_, prewarm::PrewarmScheduler>,
    clip_id: String,
    operation_id: String,
) -> Result<MediaListDto, String> {
    let progress_app = app.clone();
    let progress_operation_id = operation_id.clone();
    let on_progress: crate::export::AudioExportProgress = Arc::new(move |done, total| {
        crate::export::emit_export_progress(&progress_app, &progress_operation_id, done, total);
    });
    save_clip_as_media_impl(&core, || {
        save_clip_as_media_workflow(
            &core,
            &control,
            media.engine(),
            &prewarm,
            &clip_id,
            &operation_id,
            on_progress,
        )
    })
}

fn save_clip_as_media_impl(
    core: &AppCore,
    workflow: impl FnOnce() -> Result<MediaListDto, String>,
) -> Result<MediaListDto, String> {
    core.ensure_project_mutable().map_err(|e| e.to_string())?;
    workflow()
}

fn save_clip_as_media_workflow(
    core: &AppCore,
    control: &crate::export::ExportControl,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    clip_id: &str,
    operation_id: &str,
    on_progress: crate::export::AudioExportProgress,
) -> Result<MediaListDto, String> {
    let snapshot = core.runtime_snapshot();
    let project_dir = snapshot
        .project_dir
        .clone()
        .ok_or("save your project before saving a clip as media")?;
    let (single_timeline, subset, media_type) =
        build_single_clip_export(&snapshot.timeline, &snapshot.media, clip_id)?;
    let ext = save_clip_extension(media_type)?;
    let mut guard = control.try_begin(operation_id)?;
    let output =
        crate::export::reserve_project_media_output(&project_dir, &format!("clip_{clip_id}"), ext)?;
    let out_path = output.path().to_path_buf();
    let project_dir_option = Some(project_dir.clone());

    let metadata = match ext {
        "mp4" => {
            let req = crate::export::ExportRequest {
                out_path: out_path.to_string_lossy().into_owned(),
                codec: crate::export::ExportCodec::H264,
                quality: crate::export::ExportQuality::P1080,
            };
            let summary = crate::export::run_export_with_control(
                &single_timeline,
                &subset,
                &project_dir_option,
                &req,
                crate::export::ExportRunOptions {
                    control: Some(control),
                    on_progress: Some(Arc::clone(&on_progress)),
                    output_file: Some(output.writer()?),
                    defer_completion: true,
                    ..crate::export::ExportRunOptions::default()
                },
            )?;
            SavedMediaMetadata::Video(summary)
        }
        "wav" => {
            let mut writer = output.writer()?;
            let sample_count = crate::export::write_timeline_audio_wav_for_manifest_with_control(
                &single_timeline,
                &subset,
                &project_dir_option,
                &mut writer,
                control,
                Some(Arc::clone(&on_progress)),
            )?
            .ok_or_else(|| "audio clip contains no decodable audio".to_string())?;
            SavedMediaMetadata::Wav {
                sample_count,
                sample_rate: opentake_media::encode::MIX_SAMPLE_RATE,
            }
        }
        _ => unreachable!("save clip extension is fixed by clip type"),
    };

    finalize_saved_media(
        SavedMediaFinalizationContext {
            core,
            engine,
            prewarm,
            expected_project_epoch: snapshot.project_epoch,
            expected_project_dir: &project_dir,
            metadata,
            on_progress: on_progress.as_ref(),
        },
        output,
        &mut guard,
    )
}

fn save_clip_extension(media_type: ClipType) -> Result<&'static str, String> {
    match media_type {
        ClipType::Video => Ok("mp4"),
        ClipType::Audio => Ok("wav"),
        _ => Err("only video and audio clips can be saved as media".to_string()),
    }
}

/// Validate the user-chosen output path for [`extract_audio`] (Issue #39
/// review #4 — "out_path 无后端路径边界校验").
///
/// Enforces a path-safety boundary so an `out_path` arriving from the WebView
/// cannot:
/// - smuggle null bytes (`\0`) which some OS APIs silently truncate, leaving
///   the written file at an unexpected location;
/// - be relative (the native save dialog always returns absolute, but the
///   command is also callable directly via the Tauri API);
/// - use an extension ffmpeg would otherwise fall back on an arbitrary codec
///   for — only `.m4a` / `.m4r` / `.aac` / `.mp3` / `.wav` are allowed,
///   matching the codec table in
///   [`opentake_media::MediaEngine::extract_audio`] and the save-dialog
///   filters in `MediaPanel.tsx`.
///
/// Returns the parsed absolute [`PathBuf`] on success.
fn validate_extract_output(out_path: &str) -> Result<PathBuf, String> {
    if out_path.contains('\0') {
        return Err("output path contains null byte".into());
    }
    let output = PathBuf::from(out_path);
    if !output.is_absolute() {
        return Err(format!(
            "output path must be absolute: {}",
            output.display()
        ));
    }
    match output.extension().and_then(|e| e.to_str()) {
        Some("m4a") | Some("m4r") | Some("aac") | Some("mp3") | Some("wav") => Ok(output),
        Some(ext) => Err(format!(
            "unsupported audio extension: .{ext} (use .m4a, .mp3, or .wav)"
        )),
        None => Err("output path has no extension (use .m4a, .mp3, or .wav)".into()),
    }
}

/// `extract_audio`: extract the audio track from a media asset into a
/// self-contained audio file (`.m4a` / `.mp3` / `.wav`). The output path is
/// chosen by the caller via a native save dialog; the codec falls out of the
/// extension. Used by the media panel's per-card "extract audio" action
/// (Issue #39).
///
/// The `out_path` is first run through [`validate_extract_output`] to enforce
/// path-safety boundaries (review #4). Returns the output path on success.
/// Errors when the asset is unknown, the source path cannot be resolved or
/// found, the output path is invalid, or ffmpeg fails (missing binary,
/// non-zero exit, unsupported extension).
#[tauri::command]
pub fn extract_audio(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    media_id: String,
    out_path: String,
) -> Result<String, String> {
    // The output is user-selected rather than project state, but it is a long
    // durable write that must not be cut off by the updater's process exit.
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    // Path boundary check first (review #4): fail fast on a bad output path
    // before touching the manifest or spawning ffmpeg.
    let output = validate_extract_output(&out_path)?;
    let snapshot = core.runtime_snapshot();
    let manifest = snapshot.media;
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.id == media_id)
        .ok_or_else(|| format!("unknown media id: {media_id}"))?;
    let input = match &entry.source {
        MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
        MediaSource::Project { relative_path } => match snapshot.project_dir {
            Some(base) => base.join(relative_path),
            None => return Err("project not saved; cannot resolve media path".into()),
        },
    };
    if !input.is_file() {
        return Err(format!("source file not found: {}", input.display()));
    }
    media
        .engine()
        .extract_audio(&input, &output)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

/// `relink_media`: point a missing/offline asset at a newly chosen file, KEEPING
/// the same asset id so every clip that references it recovers in place. This is
/// the fix for "lost media stays red after re-selecting the path": the old flow
/// only had `import_media`, which mints a NEW id and leaves existing clips
/// stranded on the missing entry forever. Mirrors upstream
/// `EditorViewModel.relinkAsset(id:to:)` — the new file's type must match the
/// original (rejected otherwise), and the freshly probed metadata refreshes the
/// entry. Returns the updated catalog (with `missing` recomputed → now `false`).
#[tauri::command]
pub fn relink_media(
    app: AppHandle,
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    media_ref: String,
    new_path: String,
) -> Result<MediaListDto, String> {
    let _activity = begin_direct_media_project_write(&admission)?;
    let new = PathBuf::from(&new_path);
    if !new.is_file() {
        return Err(format!("file not found: {new_path}"));
    }
    let _identity = core.lock_project_identity_workflow();
    // Validate the target type matches before touching the catalog (upstream
    // rejects relinking across types). `relink_media_file` re-checks, but doing
    // it here yields a precise message and avoids a needless probe.
    let snapshot = core.runtime_snapshot();
    let entry = snapshot
        .media
        .entries
        .iter()
        .find(|e| e.id == media_ref)
        .ok_or_else(|| format!("media not found: {media_ref}"))?;
    let new_kind =
        importable_clip_type(&new).ok_or_else(|| format!("unsupported file: {new_path}"))?;
    if new_kind != entry.kind {
        return Err(format!(
            "cannot relink a {:?} asset to a {:?} file",
            entry.kind, new_kind
        ));
    }

    let probe = probe_media(media.engine(), &new);
    let old_proxy = entry.proxy.clone();
    if old_proxy.is_some() {
        let project_dir = snapshot
            .project_dir
            .as_deref()
            .ok_or_else(|| "media_proxy_project_must_be_saved".to_string())?;
        let project_root = ProjectRoot::open(project_dir).map_err(|error| error.to_string())?;
        core.ensure_project_root_identity_for_project(
            snapshot.project_epoch,
            project_dir,
            project_root.identity(),
        )
        .map_err(|error| error.to_string())?;
    }
    core.relink_media_file(&media_ref, &new, &probe)
        .map_err(|e| e.to_string())?;
    if let (Some(project_dir), Some(proxy)) = (snapshot.project_dir, old_proxy) {
        if let Some(path) = trusted_project_proxy_path(&project_dir, &proxy.relative_path) {
            let _ = std::fs::remove_file(&path);
            revoke_proxy_asset_file(&app, &path);
        }
    }
    Ok(MediaListDto::from_core(
        &core,
        Some(media.engine().cache_root()),
    ))
}

/// `generate_thumbnail`: generate (and disk-cache) a media asset thumbnail.
/// Video requests decode one poster frame by default. The JPEG sprite grid used
/// by timeline filmstrips is generated only when `include_sprite` is true, and
/// is capped so long sources cannot enqueue thousands of decoded frames.
#[tauri::command]
pub fn generate_thumbnail(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    media_ref: String,
    time_secs: Option<f64>,
    max_frames: Option<usize>,
    include_sprite: Option<bool>,
) -> Result<ThumbnailDto, String> {
    let snapshot = core.runtime_snapshot();
    let manifest = snapshot.media;
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.id == media_ref)
        .ok_or_else(|| format!("media not found: {media_ref}"))?;
    let path = source_path_for_entry(entry, snapshot.project_dir.as_deref())?;
    let include_sprite = include_sprite.unwrap_or(false);
    if let Some(cached) = cached_thumbnail_for_entry(
        media.engine(),
        entry,
        &path,
        time_secs,
        max_frames,
        include_sprite,
    ) {
        return cached;
    }
    let _activity = media.begin_cache_write()?;
    generate_thumbnail_for_entry(
        media.engine(),
        entry,
        &path,
        time_secs,
        max_frames,
        include_sprite,
    )
    .map_err(|e| {
        eprintln!(
            "generate_thumbnail failed: media_ref={media_ref} path={} error={e}",
            path.display()
        );
        e
    })
}

/// Cache-first timeline sprite request. A miss only admits work to the single
/// low-priority worker; the command never performs multi-point FFmpeg seeks on
/// the Tauri command thread. Partial sprites are published every six decoded
/// samples and the final JSON sidecar remains the completeness marker.
#[tauri::command]
pub fn request_timeline_sprite(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    prewarm: State<'_, prewarm::PrewarmScheduler>,
    media_ref: String,
    max_frames: Option<usize>,
) -> Result<TimelineSpriteDto, String> {
    const PARTIAL_STRIDE: usize = 6;

    let snapshot = core.runtime_snapshot();
    let entry = snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == media_ref)
        .ok_or_else(|| format!("media not found: {media_ref}"))?;
    if entry.kind != ClipType::Video {
        return Ok(TimelineSpriteDto {
            status: prewarm::TimelineSpriteStatus::Cached,
            thumbnail: None,
        });
    }
    let path = source_path_for_entry(entry, snapshot.project_dir.as_deref())?;
    if !path.is_file() {
        return Ok(TimelineSpriteDto {
            status: prewarm::TimelineSpriteStatus::Failed,
            thumbnail: None,
        });
    }
    let cache_root = media.engine().cache_root().to_path_buf();
    let key = cache_key_for(&path)?;
    let limit = sprite_frame_limit(max_frames);
    let sprite_cache_key = format!("{key}.timeline-v2-{limit}");
    let final_sprite = sprite_path_for(&cache_root, &sprite_cache_key);
    if let Some(meta) = read_cached_sprite_meta(&cache_root, &sprite_cache_key) {
        return Ok(TimelineSpriteDto {
            status: prewarm::TimelineSpriteStatus::Cached,
            thumbnail: Some(thumbnail_dto_for_sprite(
                entry,
                &cache_root,
                &key,
                &final_sprite,
                meta,
            )),
        });
    }

    let partial_sprite = partial_sprite_path_for(&cache_root, &sprite_cache_key);
    let partial_meta_path = partial_sprite_meta_path_for(&cache_root, &sprite_cache_key);
    let partial_meta = read_sprite_meta_at(&partial_sprite, &partial_meta_path);
    let partial_thumbnail = partial_meta
        .map(|meta| thumbnail_dto_for_sprite(entry, &cache_root, &key, &partial_sprite, meta));
    let epoch = snapshot.project_epoch;
    let duration = entry.duration;
    let job_key = sprite_cache_key.clone();
    let status_key = sprite_cache_key.clone();
    let job_final_sprite = final_sprite.clone();
    let job_final_meta = sprite_meta_path_for(&cache_root, &sprite_cache_key);
    let job_partial_sprite = partial_sprite.clone();
    let job_partial_meta = partial_meta_path.clone();
    let admission = prewarm.schedule(
        epoch,
        prewarm::PrewarmKind::TimelineSprite,
        job_key,
        false,
        move |context| {
            let times = representative_thumbnail_times(duration, limit);
            let request = FrameRequest {
                time_secs: 0.0,
                max_size: THUMB_MAX_SIZE,
                tolerance_secs: THUMB_TOLERANCE_SECS,
                apply_rotation: true,
            };
            let cancel = context.cancel_token();
            let mut thumbs = Vec::with_capacity(times.len());
            let mut last_time = f64::NEG_INFINITY;
            for (index, time_secs) in times.iter().copied().enumerate() {
                if context.is_cancelled() {
                    context.set_timeline_sprite_status(prewarm::TimelineSpriteStatus::Cancelled);
                    return;
                }
                let frame_request = FrameRequest {
                    time_secs,
                    ..request.clone()
                };
                match decode_frame_at_cancellable(&path, &frame_request, &cancel) {
                    Ok((actual, frame)) if actual > last_time => {
                        last_time = actual;
                        thumbs.push(VideoThumb {
                            time_secs: actual,
                            image: frame,
                        });
                    }
                    Ok(_) | Err(MediaError::Decode(_)) => continue,
                    Err(MediaError::Cancelled) => {
                        context
                            .set_timeline_sprite_status(prewarm::TimelineSpriteStatus::Cancelled);
                        return;
                    }
                    Err(_) => {
                        context.set_timeline_sprite_status(prewarm::TimelineSpriteStatus::Failed);
                        return;
                    }
                }
                if !thumbs.is_empty()
                    && thumbs.len() % PARTIAL_STRIDE == 0
                    && index + 1 < times.len()
                {
                    let Ok(Some(artifact)) = encode_sprite(&thumbs) else {
                        context.set_timeline_sprite_status(prewarm::TimelineSpriteStatus::Failed);
                        return;
                    };
                    match commit_sprite_artifact(
                        &context,
                        &job_partial_sprite,
                        &job_partial_meta,
                        &artifact,
                    ) {
                        Ok(true) => context
                            .set_timeline_sprite_status(prewarm::TimelineSpriteStatus::Partial),
                        Ok(false) => return,
                        Err(_) => {
                            context
                                .set_timeline_sprite_status(prewarm::TimelineSpriteStatus::Failed);
                            return;
                        }
                    }
                }
            }
            let Ok(Some(artifact)) = encode_sprite(&thumbs) else {
                context.set_timeline_sprite_status(prewarm::TimelineSpriteStatus::Failed);
                return;
            };
            match commit_sprite_artifact(&context, &job_final_sprite, &job_final_meta, &artifact) {
                Ok(true) => {
                    context.set_timeline_sprite_status(prewarm::TimelineSpriteStatus::Cached)
                }
                Ok(false) => {}
                Err(_) => context.set_timeline_sprite_status(prewarm::TimelineSpriteStatus::Failed),
            }
        },
    );

    let status = match admission {
        prewarm::PrewarmResult::Cached => prewarm::TimelineSpriteStatus::Cached,
        prewarm::PrewarmResult::Busy => prewarm::TimelineSpriteStatus::Busy,
        prewarm::PrewarmResult::StaleProject => prewarm::TimelineSpriteStatus::StaleProject,
        prewarm::PrewarmResult::Cancelled => prewarm::TimelineSpriteStatus::Cancelled,
        prewarm::PrewarmResult::Queued | prewarm::PrewarmResult::Duplicate => prewarm
            .timeline_sprite_status(&status_key)
            .unwrap_or(prewarm::TimelineSpriteStatus::Queued),
    };
    Ok(TimelineSpriteDto {
        status,
        thumbnail: partial_thumbnail,
    })
}

#[tauri::command]
pub fn set_timeline_sprite_interactive(
    prewarm: State<'_, prewarm::PrewarmScheduler>,
    active: bool,
) {
    prewarm.set_interactive(active);
}

/// `preview_poster`: decode (and disk-cache) a HI-RES first-frame still for the
/// single-media preview, returning its on-disk path. This is the instant
/// placeholder painted behind the `<video>` so a cold preview shows its first
/// frame immediately (no blank/spinner) and is sharp — the asset protocol then
/// streams the real video progressively (it honors HTTP Range, so `<video>` does
/// not download the whole file). Larger than the 120×68 grid thumbnail and
/// cached separately, so the two never clobber. Returns `None` for non-video
/// assets (images render straight from disk; audio has no frame). Errors only
/// when the asset is unknown or its path can't be resolved.
#[tauri::command]
pub fn preview_poster(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    media_ref: String,
    time_secs: Option<f64>,
) -> Result<Option<String>, String> {
    let snapshot = core.runtime_snapshot();
    let manifest = snapshot.media;
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.id == media_ref)
        .ok_or_else(|| format!("media not found: {media_ref}"))?;
    if entry.kind != ClipType::Video {
        return Ok(None);
    }
    let path = source_path_for_entry(entry, snapshot.project_dir.as_deref())?;
    if !path.is_file() {
        return Err(format!("source file not found: {}", path.display()));
    }
    let key = cache_key_for(&path)?;
    let target = poster_target_time(time_secs);
    let cached_path = preview_poster_path_for(media.engine().cache_root(), &key, target);
    if let Some(cached) = read_cached_poster(&cached_path, target) {
        return cached
            .map(|(poster_path, _, _, _)| Some(poster_path.to_string_lossy().into_owned()));
    }
    let _activity = media.begin_cache_write()?;
    let (poster_path, _, _, _) = video_preview_poster(media.engine(), &path, &key, time_secs)
        .map_err(|e| {
            eprintln!(
                "preview_poster failed: media_ref={media_ref} path={} error={e}",
                path.display()
            );
            e
        })?;
    Ok(Some(poster_path.to_string_lossy().into_owned()))
}

/// `get_waveform`: normalized waveform buckets (`0 = loud, 1 = silence`) for the
/// media asset `media_ref`, computed (and disk-cached) by the media engine. The
/// returned array spans the WHOLE source; the timeline maps each clip's trimmed
/// sub-range into it (mirrors upstream `MediaVisualCache.waveform`). Errors when
/// the asset is unknown, has no resolvable path, or carries no audio track.
#[tauri::command]
pub fn get_waveform(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    media_ref: String,
) -> Result<Vec<f32>, String> {
    let snapshot = core.runtime_snapshot();
    let manifest = snapshot.media;
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.id == media_ref)
        .ok_or_else(|| format!("media not found: {media_ref}"))?;
    let path = match &entry.source {
        MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
        MediaSource::Project { relative_path } => match snapshot.project_dir {
            Some(base) => base.join(relative_path),
            None => return Err("project not saved; cannot resolve media path".into()),
        },
    };
    let result = if let Some(key) = visual_file_identity_key(&path) {
        if let Some(cached) =
            opentake_media::waveform::store::load_waveform(media.engine().cache_root(), &key)
        {
            return Ok(cached);
        }
        let _activity = media.begin_cache_write()?;
        media.engine().waveform(&path, entry.duration)
    } else {
        opentake_media::waveform::waveform(&path, entry.duration)
    };
    result.map_err(|e| {
        // Log server-side too (the frontend swallows the error into "no
        // waveform"); without this a decode failure is invisible.
        eprintln!(
            "get_waveform failed: media_ref={media_ref} path={} error={e}",
            path.display()
        );
        e.to_string()
    })
}

/// Analyze one video clip into a source-bound editable stabilization track.
/// Decoding is bounded to at most 48 downscaled frames; the source file is
/// strictly read-only and the caller applies the returned solution separately
/// through `edit_apply`.
#[tauri::command]
pub async fn analyze_stabilization(
    app: AppHandle,
    clip_id: String,
) -> Result<StabilizationTrack, String> {
    let cancel = app.state::<StabilizationAnalysisState>().begin()?;
    let prepared = (|| {
        let snapshot = app.state::<AppCore>().runtime_snapshot();
        let clip = find_runtime_clip(&snapshot.timeline, &clip_id)
            .ok_or_else(|| format!("clip not found: {clip_id}"))?;
        if clip.media_type != ClipType::Video || clip.nested_sequence_id.is_some() {
            return Err("stabilization requires an ordinary video clip".to_string());
        }
        let (path, is_video) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &clip.media_ref)?;
        if !is_video {
            return Err("stabilization source is not a video".to_string());
        }
        let fps = snapshot.timeline.fps.max(1) as f64;
        let sample_count = (clip.duration_frames.max(2) as usize).min(48);
        let last_relative_frame = (clip.duration_frames - 1).max(1);
        let relative_frames = (0..sample_count)
            .map(|index| {
                ((index as f64 * last_relative_frame as f64 / (sample_count - 1) as f64).round()
                    as i32)
                    .clamp(0, last_relative_frame)
            })
            .collect::<Vec<_>>();
        let source_start = clip.trim_start_frame as f64 / fps;
        let times = relative_frames
            .iter()
            .map(|frame| source_start + *frame as f64 * clip.speed.max(0.0001) / fps)
            .collect::<Vec<_>>();
        Ok((
            path,
            times,
            relative_frames,
            source_start,
            fps,
            clip.speed,
            last_relative_frame,
            clip.media_ref.clone(),
        ))
    })();

    let result = match prepared {
        Err(error) => Err(error),
        Ok((
            path,
            times,
            relative_frames,
            source_start,
            fps,
            speed,
            last_relative_frame,
            source_identity,
        )) => {
            let worker_cancel = cancel.clone();
            match tauri::async_runtime::spawn_blocking(move || {
                let request = FrameRequest {
                    max_size: (320, 180),
                    tolerance_secs: 0.1,
                    ..FrameRequest::default()
                };
                let decoded = decode_frames_at_cancellable(&path, &times, &request, &worker_cancel);
                let mut frames = decoded
                    .into_iter()
                    .filter_map(|result| result.ok())
                    .map(|(actual, frame)| {
                        let relative =
                            ((actual - source_start) * fps / speed.max(0.0001)).round() as i32;
                        (relative.clamp(0, last_relative_frame), frame)
                    })
                    .collect::<Vec<_>>();
                if frames.len() < relative_frames.len() && worker_cancel.is_cancelled() {
                    return Err(MediaError::Cancelled.to_string());
                }
                frames.sort_by_key(|(frame, _)| *frame);
                frames.dedup_by_key(|(frame, _)| *frame);
                let motion = track_translation_motion(&frames, &worker_cancel)
                    .map_err(|error| error.to_string())?;
                build_stabilization(
                    &motion,
                    source_identity,
                    StabilizationConfig::default(),
                    &worker_cancel,
                )
                .map_err(|error| error.to_string())
            })
            .await
            {
                Ok(result) => result,
                Err(error) => Err(format!("stabilization analysis task failed: {error}")),
            }
        }
    };
    app.state::<StabilizationAnalysisState>().finish(&cancel);
    result
}

#[tauri::command]
pub fn cancel_stabilization_analysis(analysis: State<'_, StabilizationAnalysisState>) -> bool {
    analysis.cancel()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoudnessProgressEvent {
    clip_id: String,
    done: usize,
    total: usize,
}

/// Analyze the selected clip's exact visible source window. The returned value
/// is applied in a separate `edit_apply` call so analysis failures never mutate
/// project history and a successful apply remains one undoable transaction.
#[tauri::command]
pub async fn analyze_loudness(
    app: AppHandle,
    clip_id: String,
    target_lufs: f64,
    true_peak_ceiling_dbtp: f64,
) -> Result<LoudnessNormalization, String> {
    let cancel = app.state::<LoudnessAnalysisState>().begin()?;
    let prepared = (|| {
        let snapshot = app.state::<AppCore>().runtime_snapshot();
        let clip = find_runtime_clip(&snapshot.timeline, &clip_id)
            .ok_or_else(|| format!("loudness_clip_not_found: {clip_id}"))?;
        if !matches!(clip.media_type, ClipType::Audio | ClipType::Video)
            || clip.nested_sequence_id.is_some()
        {
            return Err(
                "loudness_unreadable_audio: requires an ordinary audio-bearing clip".to_string(),
            );
        }
        let (path, _) = crate::transcribe::resolve_asset_from_snapshot(&snapshot, &clip.media_ref)
            .map_err(|error| format!("loudness_unreadable_audio: {error}"))?;
        let fps = snapshot.timeline.fps.max(1) as f64;
        let start = clip.trim_start_frame.max(0) as f64 / fps;
        let duration = clip.source_frames_consumed().max(0) as f64 / fps;
        if duration <= 0.0 {
            return Err("loudness_unreadable_audio: clip has no visible duration".to_string());
        }
        if duration > 600.0 {
            return Err(
                "loudness_audio_too_long: analysis is limited to 10 minutes per clip".to_string(),
            );
        }
        Ok((path, (start, start + duration)))
    })();

    let result = match prepared {
        Err(error) => Err(error),
        Ok((path, range)) => {
            let worker_cancel = cancel.clone();
            let worker_app = app.clone();
            let worker_clip_id = clip_id.clone();
            match tauri::async_runtime::spawn_blocking(move || {
                const ANALYSIS_SAMPLE_RATE: u32 = 48_000;
                let decode_app = worker_app.clone();
                let decode_clip_id = worker_clip_id.clone();
                let decode_progress = Arc::new(move |done: usize, total: usize| {
                    let mapped = done.min(total.max(1)).saturating_mul(60) / total.max(1);
                    let _ = decode_app.emit(
                        "loudness://progress",
                        LoudnessProgressEvent {
                            clip_id: decode_clip_id.clone(),
                            done: mapped,
                            total: 100,
                        },
                    );
                });
                let pcm = extract_pcm_cancellable_with_progress(
                    &path,
                    &PcmSpec {
                        sample_rate: ANALYSIS_SAMPLE_RATE,
                        channels: 1,
                        format: PcmFormat::F32,
                    },
                    Some(range),
                    &worker_cancel,
                    Some(decode_progress),
                )
                .map_err(|error| match error {
                    MediaError::Cancelled => "loudness_cancelled".to_string(),
                    other => format!("loudness_unreadable_audio: {other}"),
                })?;
                let analysis_app = worker_app.clone();
                let analysis_clip_id = worker_clip_id.clone();
                let analysis_progress = Arc::new(move |done: usize, total: usize| {
                    let mapped = 60 + done.min(total.max(1)).saturating_mul(40) / total.max(1);
                    let _ = analysis_app.emit(
                        "loudness://progress",
                        LoudnessProgressEvent {
                            clip_id: analysis_clip_id.clone(),
                            done: mapped,
                            total: 100,
                        },
                    );
                });
                let measured = analyze_loudness_with_progress(
                    &pcm.samples_f32,
                    ANALYSIS_SAMPLE_RATE,
                    LoudnessNormalizationConfig {
                        target_lufs,
                        true_peak_ceiling_dbtp,
                    },
                    &worker_cancel,
                    Some(analysis_progress),
                )
                .map_err(|error| error.to_string())?;
                Ok(LoudnessNormalization {
                    target_lufs: measured.target_lufs,
                    true_peak_ceiling_dbtp: measured.true_peak_ceiling_dbtp,
                    input_integrated_lufs: measured.input_integrated_lufs,
                    input_true_peak_dbtp: measured.input_true_peak_dbtp,
                    gain_db: measured.gain_db,
                    output_integrated_lufs: measured.output_integrated_lufs,
                    output_true_peak_dbtp: measured.output_true_peak_dbtp,
                })
            })
            .await
            {
                Ok(result) => result,
                Err(error) => Err(format!("loudness_analysis_task_failed: {error}")),
            }
        }
    };
    app.state::<LoudnessAnalysisState>().finish(&cancel);
    result
}

#[tauri::command]
pub fn cancel_loudness_analysis(analysis: State<'_, LoudnessAnalysisState>) -> bool {
    analysis.cancel()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DenoiseProgressEvent {
    clip_id: String,
    done: usize,
    total: usize,
}

/// Decode and process the exact visible source window before an Inspector apply.
/// The processed copy is discarded: success proves the configured operation is
/// runnable, while the source stays immutable and the separate edit command is
/// the only history mutation.
#[tauri::command]
pub async fn prepare_denoise(
    app: AppHandle,
    clip_id: String,
    mode: DenoiseMode,
    strength: f64,
    preview_enabled: bool,
) -> Result<AudioDenoise, String> {
    let cancel = app.state::<DenoiseAnalysisState>().begin()?;
    let config = AudioDenoise {
        mode,
        strength,
        preview_enabled,
    };
    let prepared = (|| {
        config
            .validate()
            .map_err(|error| format!("denoise_invalid_config: {error}"))?;
        let snapshot = app.state::<AppCore>().runtime_snapshot();
        let clip = find_runtime_clip(&snapshot.timeline, &clip_id)
            .ok_or_else(|| format!("denoise_clip_not_found: {clip_id}"))?;
        if !matches!(clip.media_type, ClipType::Audio | ClipType::Video)
            || clip.nested_sequence_id.is_some()
        {
            return Err(
                "denoise_unreadable_audio: requires an ordinary audio-bearing clip".to_string(),
            );
        }
        let (path, _) = crate::transcribe::resolve_asset_from_snapshot(&snapshot, &clip.media_ref)
            .map_err(|error| format!("denoise_unreadable_audio: {error}"))?;
        let fps = snapshot.timeline.fps.max(1) as f64;
        let start = clip.trim_start_frame.max(0) as f64 / fps;
        let duration = clip.source_frames_consumed().max(0) as f64 / fps;
        if duration <= 0.0 {
            return Err("denoise_unreadable_audio: clip has no visible duration".to_string());
        }
        if duration > 600.0 {
            return Err(
                "denoise_audio_too_long: validation is limited to 10 minutes per clip".to_string(),
            );
        }
        Ok((path, (start, start + duration)))
    })();

    let result = match prepared {
        Err(error) => Err(error),
        Ok((path, range)) => {
            let worker_cancel = cancel.clone();
            let worker_app = app.clone();
            let worker_clip_id = clip_id.clone();
            match tauri::async_runtime::spawn_blocking(move || {
                const SAMPLE_RATE: u32 = 48_000;
                let decode_app = worker_app.clone();
                let decode_clip_id = worker_clip_id.clone();
                let decode_progress = Arc::new(move |done: usize, total: usize| {
                    let mapped = done.min(total.max(1)).saturating_mul(60) / total.max(1);
                    let _ = decode_app.emit(
                        "denoise://progress",
                        DenoiseProgressEvent {
                            clip_id: decode_clip_id.clone(),
                            done: mapped,
                            total: 100,
                        },
                    );
                });
                let pcm = extract_pcm_cancellable_with_progress(
                    &path,
                    &PcmSpec {
                        sample_rate: SAMPLE_RATE,
                        channels: 1,
                        format: PcmFormat::F32,
                    },
                    Some(range),
                    &worker_cancel,
                    Some(decode_progress),
                )
                .map_err(|error| match error {
                    MediaError::Cancelled => "denoise_cancelled".to_string(),
                    other => format!("denoise_unreadable_audio: {other}"),
                })?;
                let process_app = worker_app.clone();
                let process_clip_id = worker_clip_id.clone();
                let process_progress = Arc::new(move |done: usize, total: usize| {
                    let mapped = 60 + done.min(total.max(1)).saturating_mul(40) / total.max(1);
                    let _ = process_app.emit(
                        "denoise://progress",
                        DenoiseProgressEvent {
                            clip_id: process_clip_id.clone(),
                            done: mapped,
                            total: 100,
                        },
                    );
                });
                let _processed = denoise_interleaved(
                    &pcm.samples_f32,
                    1,
                    SAMPLE_RATE,
                    config,
                    &worker_cancel,
                    Some(process_progress),
                )
                .map_err(|error| error.to_string())?;
                Ok(config)
            })
            .await
            {
                Ok(result) => result,
                Err(error) => Err(format!("denoise_analysis_task_failed: {error}")),
            }
        }
    };
    app.state::<DenoiseAnalysisState>().finish(&cancel);
    result
}

#[tauri::command]
pub fn cancel_denoise_analysis(analysis: State<'_, DenoiseAnalysisState>) -> bool {
    analysis.cancel()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StemProgressEvent {
    source_asset_id: String,
    done: usize,
    total: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StemSeparationDto {
    pub vocals_asset_id: String,
    pub accompaniment_asset_id: String,
    pub source_sha256: String,
    pub execution: String,
    pub model_sha256: Option<String>,
    pub vocal_sdr_improvement_db: f64,
}

/// Run local stem separation off the UI thread, import both outputs atomically,
/// and persist their source/model provenance. Hosted mode is fail-closed until
/// a concrete provider adapter is configured; no upload occurs in this command.
#[tauri::command]
pub async fn separate_audio_stems(
    app: AppHandle,
    source_asset_id: String,
    execution: String,
    provider: Option<String>,
    model: Option<String>,
    upload_confirmed: bool,
) -> Result<StemSeparationDto, String> {
    if execution != "local" {
        if provider.as_deref().is_none_or(str::is_empty)
            || model.as_deref().is_none_or(str::is_empty)
        {
            return Err("stem_hosted_provider_and_model_required".to_string());
        }
        if !upload_confirmed {
            return Err("stem_hosted_privacy_confirmation_required".to_string());
        }
        return Err("stem_hosted_provider_not_configured".to_string());
    }

    let cancel = app.state::<StemSeparationState>().begin()?;
    let result = async {
        let core = app.state::<AppCore>();
        core.ensure_project_mutable()
            .map_err(|error| error.to_string())?;
        let snapshot = core.runtime_snapshot();
        let project_dir = snapshot
            .project_dir
            .clone()
            .ok_or_else(|| "stem_project_must_be_saved".to_string())?;
        let source_entry = snapshot
            .media
            .entries
            .iter()
            .find(|entry| entry.id == source_asset_id)
            .cloned()
            .ok_or_else(|| format!("stem_source_not_found:{source_asset_id}"))?;
        if !matches!(source_entry.kind, ClipType::Audio | ClipType::Video)
            || !source_entry
                .has_audio
                .unwrap_or(source_entry.kind == ClipType::Audio)
        {
            return Err("stem_source_has_no_audio".to_string());
        }
        let source_path = source_path_for_entry(&source_entry, Some(&project_dir))?;
        if !source_path.is_file() {
            return Err("stem_source_unreadable".to_string());
        }
        let output_dir = project_dir
            .join("media")
            .join(format!("stems-{}", uuid::Uuid::new_v4()));
        let model_dir = app
            .state::<MediaState>()
            .engine()
            .models_dir()
            .to_path_buf();
        let worker_app = app.clone();
        let worker_asset_id = source_asset_id.clone();
        let worker_cancel = cancel.clone();
        let worker_output_dir = output_dir.clone();
        let separated = match tauri::async_runtime::spawn_blocking(move || {
            let progress_app = worker_app.clone();
            let progress_asset_id = worker_asset_id.clone();
            let progress = Arc::new(move |done: usize, total: usize| {
                let _ = progress_app.emit(
                    "stems://progress",
                    StemProgressEvent {
                        source_asset_id: progress_asset_id.clone(),
                        done,
                        total,
                    },
                );
            });
            separate_stems(
                StemSeparationRequest {
                    source: &source_path,
                    output_dir: &worker_output_dir,
                    execution: StemExecution::Local {
                        model_dir: &model_dir,
                    },
                },
                &worker_cancel,
                Some(progress),
            )
        })
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(MediaError::Cancelled)) => {
                let _ = std::fs::remove_dir_all(&output_dir);
                return Err("stem_separation_cancelled".to_string());
            }
            Ok(Err(error)) => {
                let _ = std::fs::remove_dir_all(&output_dir);
                return Err(format!("stem_separation_failed:{error}"));
            }
            Err(error) => {
                let _ = std::fs::remove_dir_all(&output_dir);
                return Err(format!("stem_separation_task_failed:{error}"));
            }
        };
        if cancel.is_cancelled() {
            let _ = std::fs::remove_dir_all(&output_dir);
            return Err("stem_separation_cancelled".to_string());
        }
        let media = app.state::<MediaState>();
        let vocals_probe = probe_media(media.engine(), &separated.vocals.path);
        let accompaniment_probe = probe_media(media.engine(), &separated.accompaniment.path);
        if !vocals_probe.has_audio || !accompaniment_probe.has_audio {
            let _ = std::fs::remove_dir_all(&output_dir);
            return Err("stem_output_probe_failed".to_string());
        }
        let common = |stem: &str| DerivedStemProvenance {
            source_asset_id: source_asset_id.clone(),
            source_sha256: separated.provenance.source_sha256.clone(),
            execution: separated.provenance.execution.clone(),
            model_sha256: separated.provenance.model_sha256.clone(),
            stem: stem.to_string(),
        };
        let committed = core
            .import_media_batch_for_project_persisted(
                snapshot.project_epoch,
                &project_dir,
                vec![
                    PreparedMediaImportOp::ImportDerivedStem {
                        path: separated.vocals.path.clone(),
                        name: separated.vocals.name.clone(),
                        probe: vocals_probe,
                        provenance: common("vocals"),
                    },
                    PreparedMediaImportOp::ImportDerivedStem {
                        path: separated.accompaniment.path.clone(),
                        name: separated.accompaniment.name.clone(),
                        probe: accompaniment_probe,
                        provenance: common("accompaniment"),
                    },
                ],
            )
            .map_err(|error| {
                let _ = std::fs::remove_dir_all(&output_dir);
                format!("stem_import_failed:{error}")
            })?;
        if committed.len() != 2 {
            return Err("stem_import_incomplete".to_string());
        }
        Ok(StemSeparationDto {
            vocals_asset_id: committed[0].entry.id.clone(),
            accompaniment_asset_id: committed[1].entry.id.clone(),
            source_sha256: separated.provenance.source_sha256,
            execution: separated.provenance.execution,
            model_sha256: separated.provenance.model_sha256,
            vocal_sdr_improvement_db: separated.metrics.vocal_sdr_improvement_db,
        })
    }
    .await;
    app.state::<StemSeparationState>().finish(&cancel);
    result
}

#[tauri::command]
pub fn cancel_stem_separation(state: State<'_, StemSeparationState>) -> bool {
    state.cancel()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStemsToTracksDto {
    pub clip_ids: Vec<String>,
    pub action_name: String,
}

/// Place an already reviewed aligned stem pair on separate fresh audio tracks
/// as one undoable edit. The derived media remain reusable in the catalog when
/// the placement is undone.
#[tauri::command]
pub fn import_stems_to_tracks(
    core: State<'_, AppCore>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    vocals_asset_id: String,
    accompaniment_asset_id: String,
    start_frame: i32,
) -> Result<ImportStemsToTracksDto, String> {
    let _activity = begin_direct_media_project_write(&admission)?;
    import_stems_to_tracks_core(&core, vocals_asset_id, accompaniment_asset_id, start_frame)
}

fn import_stems_to_tracks_core(
    core: &AppCore,
    vocals_asset_id: String,
    accompaniment_asset_id: String,
    start_frame: i32,
) -> Result<ImportStemsToTracksDto, String> {
    if start_frame < 0 || vocals_asset_id == accompaniment_asset_id {
        return Err("stem_track_import_invalid_arguments".to_string());
    }
    let snapshot = core.runtime_snapshot();
    let find = |id: &str, expected_stem: &str| {
        let entry = snapshot
            .media
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| format!("stem_track_import_asset_not_found:{id}"))?;
        if entry.kind != ClipType::Audio
            || entry
                .generation_input
                .as_ref()
                .is_none_or(|input| input.prompt != format!("stem:{expected_stem}"))
        {
            return Err(format!("stem_track_import_asset_invalid:{id}"));
        }
        Ok(entry)
    };
    let vocals = find(&vocals_asset_id, "vocals")?;
    let accompaniment = find(&accompaniment_asset_id, "accompaniment")?;
    let fps = snapshot.timeline.fps.max(1) as f64;
    let vocals_duration = (vocals.duration * fps).round().max(1.0) as i32;
    let accompaniment_duration = (accompaniment.duration * fps).round().max(1.0) as i32;
    if (vocals_duration - accompaniment_duration).abs() > 1 {
        return Err("stem_track_import_duration_mismatch".to_string());
    }
    let duration_frames = vocals_duration.max(accompaniment_duration);
    let entry = |media_ref: String| ClipEntry {
        media_ref,
        media_type: ClipType::Audio,
        source_clip_type: ClipType::Audio,
        track_index: 0,
        start_frame,
        duration_frames,
        trim_start_frame: None,
        trim_end_frame: None,
        has_audio: true,
        add_linked_audio: false,
        transform: None,
    };
    let result = core
        .apply_at_revision(
            opentake_core::ProjectRevision {
                project_epoch: snapshot.project_epoch,
                version: snapshot.version,
            },
            EditCommand::AddClipsToSeparateAutoTracks {
                entries: vec![entry(vocals_asset_id), entry(accompaniment_asset_id)],
            },
        )
        .map_err(|error| error.to_string())?;
    Ok(ImportStemsToTracksDto {
        clip_ids: result.affected_clip_ids,
        action_name: result.action_name,
    })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaProxyProgressEvent {
    asset_id: String,
    done: usize,
    total: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaProxyDto {
    pub asset_id: String,
    pub path: String,
    pub source_sha256: String,
    pub width: u32,
    pub height: u32,
}

fn resolved_project_proxy_path(project_dir: &Path, relative_path: &str) -> Option<PathBuf> {
    let relative = Path::new(relative_path);
    if relative.is_absolute() {
        return None;
    }
    let components: Vec<_> = relative.components().collect();
    if components.len() != 3
        || components[0] != std::path::Component::Normal(std::ffi::OsStr::new("media"))
        || components[1] != std::path::Component::Normal(std::ffi::OsStr::new("proxies"))
        || !matches!(components[2], std::path::Component::Normal(_))
        || relative
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("mp4")
    {
        return None;
    }
    Some(project_dir.join(relative))
}

fn metadata_is_symlink_or_reparse(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

fn project_proxy_directory(project_dir: &Path, create: bool) -> Result<PathBuf, String> {
    let project_metadata = std::fs::symlink_metadata(project_dir)
        .map_err(|error| format!("media_proxy_project_metadata_failed:{error}"))?;
    if !project_metadata.is_dir() || metadata_is_symlink_or_reparse(&project_metadata) {
        return Err("media_proxy_project_directory_required".to_string());
    }
    let project_root = project_dir
        .canonicalize()
        .map_err(|error| format!("media_proxy_project_resolve_failed:{error}"))?;

    let media_dir = project_dir.join("media");
    match std::fs::symlink_metadata(&media_dir) {
        Ok(metadata) if metadata.is_dir() && !metadata_is_symlink_or_reparse(&metadata) => {}
        Ok(_) => return Err("media_proxy_media_directory_required".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
            std::fs::create_dir(&media_dir)
                .map_err(|error| format!("media_proxy_media_create_failed:{error}"))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("media_proxy_media_directory_missing".to_string());
        }
        Err(error) => return Err(format!("media_proxy_media_metadata_failed:{error}")),
    }
    let media_metadata = std::fs::symlink_metadata(&media_dir)
        .map_err(|error| format!("media_proxy_media_metadata_failed:{error}"))?;
    if !media_metadata.is_dir() || metadata_is_symlink_or_reparse(&media_metadata) {
        return Err("media_proxy_media_directory_required".to_string());
    }
    let resolved_media = media_dir
        .canonicalize()
        .map_err(|error| format!("media_proxy_media_resolve_failed:{error}"))?;
    if resolved_media.parent() != Some(project_root.as_path()) {
        return Err("media_proxy_media_directory_escape".to_string());
    }

    let proxy_dir = media_dir.join("proxies");
    match std::fs::symlink_metadata(&proxy_dir) {
        Ok(metadata) if metadata.is_dir() && !metadata_is_symlink_or_reparse(&metadata) => {}
        Ok(_) => return Err("media_proxy_directory_required".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
            std::fs::create_dir(&proxy_dir)
                .map_err(|error| format!("media_proxy_directory_create_failed:{error}"))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("media_proxy_directory_missing".to_string());
        }
        Err(error) => return Err(format!("media_proxy_directory_metadata_failed:{error}")),
    }
    let proxy_metadata = std::fs::symlink_metadata(&proxy_dir)
        .map_err(|error| format!("media_proxy_directory_metadata_failed:{error}"))?;
    if !proxy_metadata.is_dir() || metadata_is_symlink_or_reparse(&proxy_metadata) {
        return Err("media_proxy_directory_required".to_string());
    }
    let resolved_proxy = proxy_dir
        .canonicalize()
        .map_err(|error| format!("media_proxy_directory_resolve_failed:{error}"))?;
    if resolved_proxy.parent() != Some(resolved_media.as_path()) {
        return Err("media_proxy_directory_escape".to_string());
    }
    Ok(proxy_dir)
}

pub(crate) fn trusted_project_proxy_path(
    project_dir: &Path,
    relative_path: &str,
) -> Option<PathBuf> {
    let candidate = resolved_project_proxy_path(project_dir, relative_path)?;
    let proxy_dir = project_proxy_directory(project_dir, false).ok()?;
    if candidate.parent() != Some(proxy_dir.as_path()) {
        return None;
    }
    let metadata = std::fs::symlink_metadata(&candidate).ok()?;
    if !metadata.is_file() || metadata_is_symlink_or_reparse(&metadata) {
        return None;
    }
    Some(candidate)
}

fn grant_proxy_asset_file<R: Runtime>(app: &AppHandle<R>, path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("media_proxy_scope_metadata_failed:{error}"))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err("media_proxy_scope_regular_file_required".to_string());
    }
    if !crate::fs_availability::is_materialized_regular_file(path) {
        return Err("media_proxy_scope_materialized_file_required".to_string());
    }
    app.asset_protocol_scope()
        .allow_file(path)
        .map_err(|error| format!("media_proxy_scope_grant_failed:{error}"))
}

fn revoke_proxy_asset_file<R: Runtime>(app: &AppHandle<R>, path: &Path) {
    let scope = app.asset_protocol_scope();
    // persisted-scope writes on PathAllowed events. Add the exact-file deny
    // first, then re-emit the exact allow so both patterns are durably saved;
    // deny precedence keeps the removed path inaccessible after restart.
    if scope.forbid_file(path).is_ok() {
        let _ = scope.allow_file(path);
    }
}

fn grant_catalog_proxy_asset_scope<R: Runtime>(app: &AppHandle<R>, catalog: &mut MediaListDto) {
    for item in &mut catalog.items {
        let Some(path) = item.proxy_path.as_deref().map(Path::new) else {
            continue;
        };
        if grant_proxy_asset_file(app, path).is_err() {
            item.proxy_path = None;
        }
    }
}

/// Create a bounded project-local H.264 proxy off the UI thread, then persist
/// its path + source digest in one manifest commit. The original source remains
/// untouched and is still the only input used by export.
fn create_media_proxy_blocking(
    app: AppHandle,
    asset_id: String,
    max_width: Option<u32>,
    max_height: Option<u32>,
    cancel: opentake_media::MediaCancelToken,
) -> Result<MediaProxyDto, String> {
    let core = app.state::<AppCore>();
    let _identity = core.lock_project_identity_workflow();
    core.ensure_project_mutable()
        .map_err(|error| error.to_string())?;
    let snapshot = core.runtime_snapshot();
    let project_dir = snapshot
        .project_dir
        .clone()
        .ok_or_else(|| "media_proxy_project_must_be_saved".to_string())?;
    let entry = snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == asset_id)
        .cloned()
        .ok_or_else(|| format!("media_proxy_source_not_found:{asset_id}"))?;
    if entry.kind != ClipType::Video {
        return Err("media_proxy_video_required".to_string());
    }
    let source = source_path_for_entry(&entry, Some(&project_dir))?;
    if !source.is_file() {
        return Err("media_proxy_source_unreadable".to_string());
    }

    let project_root = ProjectRoot::open(&project_dir).map_err(|error| error.to_string())?;
    core.ensure_project_root_identity_for_project(
        snapshot.project_epoch,
        &project_dir,
        project_root.identity(),
    )
    .map_err(|error| error.to_string())?;
    let proxy_dir = project_proxy_directory(&project_dir, true)?;
    let leaf = format!("{}.mp4", uuid::Uuid::new_v4());
    let relative_path = format!("media/proxies/{leaf}");
    let output = proxy_dir.join(leaf);
    let progress_app = app.clone();
    let progress_asset_id = asset_id.clone();
    let progress: ProxyProgressCallback = Arc::new(move |done, total| {
        let _ = progress_app.emit(
            "proxy://progress",
            MediaProxyProgressEvent {
                asset_id: progress_asset_id.clone(),
                done,
                total,
            },
        );
    });
    let created = match create_proxy(
        ProxyRequest {
            source: &source,
            output: &output,
            max_size: (max_width.unwrap_or(1280), max_height.unwrap_or(720)),
        },
        &cancel,
        Some(progress),
    ) {
        Ok(created) => created,
        Err(MediaError::Cancelled) => return Err("media_proxy_cancelled".to_string()),
        Err(error) => return Err(format!("media_proxy_failed:{error}")),
    };
    let proxy = MediaProxy {
        relative_path: relative_path.clone(),
        source_sha256: created.source_sha256.clone(),
        width: created.width,
        height: created.height,
    };
    if let Err(error) = core.ensure_project_root_identity_for_project(
        snapshot.project_epoch,
        &project_dir,
        project_root.identity(),
    ) {
        let _ = std::fs::remove_file(&output);
        return Err(error.to_string());
    }
    if let Err(error) = core.set_media_proxy_for_project(
        snapshot.project_epoch,
        &project_dir,
        &asset_id,
        Some(proxy),
    ) {
        let _ = std::fs::remove_file(&output);
        return Err(format!("media_proxy_persist_failed:{error}"));
    }
    if let Err(error) = grant_proxy_asset_file(&app, &output) {
        let rollback = core.set_media_proxy_for_project(
            snapshot.project_epoch,
            &project_dir,
            &asset_id,
            entry.proxy.clone(),
        );
        let _ = std::fs::remove_file(&output);
        return match rollback {
            Ok(_) => Err(error),
            Err(rollback_error) => Err(format!(
                "{error};media_proxy_scope_rollback_failed:{rollback_error}"
            )),
        };
    }
    if let Some(old) = entry
        .proxy
        .as_ref()
        .and_then(|proxy| trusted_project_proxy_path(&project_dir, &proxy.relative_path))
    {
        if old != output {
            let _ = std::fs::remove_file(&old);
            revoke_proxy_asset_file(&app, &old);
        }
    }
    Ok(MediaProxyDto {
        asset_id,
        path: output.to_string_lossy().into_owned(),
        source_sha256: created.source_sha256,
        width: created.width,
        height: created.height,
    })
}

#[tauri::command]
pub async fn create_media_proxy(
    app: AppHandle,
    asset_id: String,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<MediaProxyDto, String> {
    let cancel = app.state::<MediaProxyState>().begin()?;
    let worker_app = app.clone();
    let worker_cancel = cancel.clone();
    let result = match tauri::async_runtime::spawn_blocking(move || {
        create_media_proxy_blocking(worker_app, asset_id, max_width, max_height, worker_cancel)
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(format!("media_proxy_task_failed:{error}")),
    };
    app.state::<MediaProxyState>().finish(&cancel);
    result
}

#[tauri::command]
pub fn cancel_media_proxy(state: State<'_, MediaProxyState>) -> bool {
    state.cancel()
}

#[tauri::command]
pub fn set_proxy_playback_enabled(state: State<'_, MediaProxyState>, enabled: bool) -> bool {
    state.set_enabled(enabled);
    state.enabled()
}

#[tauri::command]
pub fn get_proxy_playback_enabled(state: State<'_, MediaProxyState>) -> bool {
    state.enabled()
}

fn remove_media_proxy_impl(
    core: &AppCore,
    asset_id: &str,
    cleanup: impl FnOnce(&Path),
) -> Result<bool, String> {
    let _identity = core.lock_project_identity_workflow();
    core.ensure_project_mutable()
        .map_err(|error| error.to_string())?;
    let snapshot = core.runtime_snapshot();
    let project_dir = snapshot
        .project_dir
        .clone()
        .ok_or_else(|| "media_proxy_project_must_be_saved".to_string())?;
    let old = snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == asset_id)
        .ok_or_else(|| format!("media_proxy_source_not_found:{asset_id}"))?
        .proxy
        .clone();
    if old.is_none() {
        return Ok(false);
    }
    let project_root = ProjectRoot::open(&project_dir).map_err(|error| error.to_string())?;
    core.ensure_project_root_identity_for_project(
        snapshot.project_epoch,
        &project_dir,
        project_root.identity(),
    )
    .map_err(|error| error.to_string())?;
    core.set_media_proxy_for_project(snapshot.project_epoch, &project_dir, asset_id, None)
        .map_err(|error| error.to_string())?;
    if let Some(path) = old
        .as_ref()
        .and_then(|proxy| trusted_project_proxy_path(&project_dir, &proxy.relative_path))
    {
        cleanup(&path);
    }
    Ok(true)
}

#[tauri::command]
pub fn remove_media_proxy(
    app: AppHandle,
    core: State<'_, AppCore>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    asset_id: String,
) -> Result<bool, String> {
    let _activity = begin_direct_media_project_write(&admission)?;
    remove_media_proxy_impl(&core, &asset_id, |path| {
        let _ = std::fs::remove_file(path);
        revoke_proxy_asset_file(&app, path);
    })
}

fn find_runtime_clip<'a>(timeline: &'a Timeline, clip_id: &str) -> Option<&'a Clip> {
    timeline
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .find(|clip| clip.id == clip_id)
        .or_else(|| {
            timeline
                .nested_sequences
                .iter()
                .flat_map(|sequence| &sequence.timeline.tracks)
                .flat_map(|track| &track.clips)
                .find(|clip| clip.id == clip_id)
        })
}

/// `preload_media`: enqueue the smallest cache that makes the selected media
/// immediately useful — a hi-res first-frame poster for video or a waveform for
/// audio. The bounded project scheduler keeps this fire-and-forget work off the
/// command thread and returns an explicit admission result.
///
/// Deliberately does not warm the 240-frame filmstrip sprite. Video playback is
/// streamed progressively; audio has no progressive visual fallback, so its
/// bounded waveform job is the useful equivalent of the video poster.
#[tauri::command]
pub fn preload_media(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    prewarm: State<'_, prewarm::PrewarmScheduler>,
    media_ref: String,
) -> Result<prewarm::PrewarmResult, String> {
    let snapshot = core.runtime_snapshot();
    let Some(entry) = snapshot.media.entries.iter().find(|e| e.id == media_ref) else {
        return Ok(prewarm::PrewarmResult::Cached);
    };
    let Some(path) = resolve_source_path(entry, snapshot.project_dir.as_deref()) else {
        return Ok(prewarm::PrewarmResult::Cached);
    };
    if !path.is_file() {
        return Ok(prewarm::PrewarmResult::Cached);
    }
    let key = cache_key_for(&path)?;
    let epoch = snapshot.project_epoch;
    match entry.kind {
        ClipType::Video => {
            let target = preview_poster_path_for(media.engine().cache_root(), &key, 0.0);
            let cached = image::image_dimensions(&target).is_ok();
            Ok(prewarm.schedule(
                epoch,
                prewarm::PrewarmKind::PreviewPoster,
                key,
                cached,
                move |context| {
                    let request = FrameRequest {
                        time_secs: 0.0,
                        max_size: PREVIEW_POSTER_MAX_SIZE,
                        tolerance_secs: THUMB_TOLERANCE_SECS,
                        apply_rotation: true,
                    };
                    let cancel = context.cancel_token();
                    let Ok((_, bytes)) =
                        opentake_media::decode::frame::decode_frame_png_cancellable(
                            &path, &request, &cancel,
                        )
                    else {
                        return;
                    };
                    let _ = context.commit_staged_bytes(&target, &bytes);
                },
            ))
        }
        ClipType::Audio => {
            let target =
                visual_cache_dir(media.engine().cache_root()).join(format!("{key}.waveform"));
            let cached =
                opentake_media::waveform::store::load_waveform(media.engine().cache_root(), &key)
                    .is_some();
            let duration = entry.duration;
            Ok(prewarm.schedule(
                epoch,
                prewarm::PrewarmKind::TimelineVisuals,
                key,
                cached,
                move |context| {
                    let cancel = context.cancel_token();
                    let Ok(bytes) = opentake_media::waveform::waveform_cache_bytes_cancellable(
                        &path, duration, &cancel,
                    ) else {
                        return;
                    };
                    let _ = context.commit_staged_bytes(&target, &bytes);
                },
            ))
        }
        _ => Ok(prewarm::PrewarmResult::Cached),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn engine_for(tmp: &Path) -> MediaEngine {
        MediaEngine::new(tmp.join("cache"), tmp.join("models"))
    }

    fn touch(path: &Path) {
        fs::write(path, b"x").unwrap();
    }

    fn unknown_core(root: &Path) -> AppCore {
        let bundle = root.join("Unknown.opentake");
        let source = root.join("source.mp4");
        touch(&source);
        let mut project = opentake_project::Project::new(&bundle);
        project.manifest.entries.push(MediaManifestEntry {
            id: "asset-1".into(),
            name: "source".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: source.to_string_lossy().into_owned(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: Some(320),
            source_height: Some(240),
            source_fps: Some(30.0),
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        project.save().expect("save known fixture");
        let path = bundle.join("project.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read timeline fixture"))
                .expect("decode timeline fixture");
        value["futureTimeline"] = serde_json::json!(true);
        fs::write(
            &path,
            serde_json::to_vec_pretty(&value).expect("encode unknown fixture"),
        )
        .expect("write unknown fixture");
        let core = AppCore::new();
        core.open_project(bundle).expect("unknown project opens");
        core
    }

    fn saved_core_with_media(root: &Path) -> (AppCore, PathBuf, PathBuf, String) {
        let bundle = root.join("Favorite.opentake");
        let source = root.join("source.mp4");
        fs::write(&source, b"favorite bytes").expect("write media fixture");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save initial project");
        let entry = core
            .import_media_file(&source, "source", &ProbedMedia::default())
            .expect("import fixture");
        core.save_project(None).expect("persist imported media");
        (core, bundle, source, entry.id)
    }

    fn recursive_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
            if !dir.exists() {
                return;
            }
            let mut paths = fs::read_dir(dir)
                .expect("read tree")
                .map(|entry| entry.expect("read tree entry").path())
                .collect::<Vec<_>>();
            paths.sort();
            for path in paths {
                let relative = path
                    .strip_prefix(root)
                    .expect("tree path under root")
                    .into();
                if path.is_dir() {
                    out.push((relative, b"<dir>".to_vec()));
                    walk(root, &path, out);
                } else {
                    out.push((relative, fs::read(&path).expect("read tree file")));
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out
    }

    #[test]
    fn favorite_command_refuses_unknown_project_without_manifest_change() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let core = unknown_core(tmp.path());
        let before = core.media();
        let store = LibraryStore::new(tmp.path().join("library"));

        let error = toggle_favorite_impl(&core, &tmp.path().join("cache"), &store, "asset-1", true)
            .expect_err("favorite must be rejected");

        assert!(error.contains("compatibility read-only"), "{error}");
        assert_eq!(core.media(), before);
    }

    #[test]
    fn stale_project_identity_cannot_mutate_replacement_project_or_global_library() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let project_a_root = tmp.path().join("project-a");
        let project_b_root = tmp.path().join("project-b");
        fs::create_dir(&project_a_root).unwrap();
        fs::create_dir(&project_b_root).unwrap();
        let (core, _bundle_a, _source_a, asset_a) = saved_core_with_media(&project_a_root);
        let project_a = core.runtime_snapshot();
        let project_a_dir = project_a.project_dir.clone().unwrap();
        let (_other, bundle_b, _source_b, _asset_b) = saved_core_with_media(&project_b_root);
        core.open_project(bundle_b).expect("replace A with B");
        let project_b_before = core.media();
        let library_root = tmp.path().join("library");
        let store = LibraryStore::new(&library_root);
        let library_before = recursive_tree(&library_root);

        let toggle_error = toggle_favorite_impl_for_project(
            &core,
            tmp.path(),
            &store,
            &asset_a,
            true,
            project_a.project_epoch,
            &project_a_dir,
        )
        .expect_err("stale A toggle must be rejected before library I/O");
        let sync_error = sync_project_favorites_impl_for_project(
            &core,
            tmp.path(),
            &store,
            vec![asset_a],
            project_a.project_epoch,
            &project_a_dir,
        )
        .expect_err("stale A sync must be rejected before library I/O");

        assert!(toggle_error.contains("project changed"), "{toggle_error}");
        assert!(sync_error.contains("project changed"), "{sync_error}");
        assert_eq!(core.media(), project_b_before);
        assert_eq!(store.entries().unwrap(), Vec::new());
        assert_eq!(recursive_tree(&library_root), library_before);
    }

    #[test]
    fn global_favorite_is_copied_mapped_and_durable_on_reopen() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, _source, asset_id) = saved_core_with_media(tmp.path());
        let store = LibraryStore::new(tmp.path().join("library"));

        toggle_favorite_impl(&core, tmp.path(), &store, &asset_id, true)
            .expect("favorite globally");

        let library_id = core
            .media()
            .library_favorite_id(&asset_id)
            .expect("project mapping")
            .to_string();
        assert!(store.contains(&library_id).unwrap());
        assert!(store.stored_path(&library_id).unwrap().unwrap().is_file());
        let reopened = AppCore::new();
        reopened.open_project(bundle).expect("reopen project");
        assert_eq!(
            reopened.media().library_favorite_id(&asset_id),
            Some(library_id.as_str())
        );
    }

    #[test]
    fn global_library_failure_preserves_the_legacy_project_marker() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, _source, asset_id) = saved_core_with_media(tmp.path());
        core.set_media_favorite(std::slice::from_ref(&asset_id), true)
            .expect("seed legacy marker");
        core.save_project(None).expect("persist legacy marker");
        let library_root = tmp.path().join("library");
        fs::create_dir_all(&library_root).unwrap();
        fs::write(library_root.join("library.json"), b"not json").unwrap();
        let store = LibraryStore::new(library_root);

        toggle_favorite_impl(&core, tmp.path(), &store, &asset_id, true)
            .expect_err("corrupt library must reject favorite");

        assert!(core.media().is_favorite(&asset_id));
        assert_eq!(core.media().library_favorite_id(&asset_id), None);
        let reopened = AppCore::new();
        reopened.open_project(bundle).expect("reopen project");
        assert!(reopened.media().is_favorite(&asset_id));
        assert_eq!(reopened.media().library_favorite_id(&asset_id), None);
    }

    #[test]
    fn failed_project_commit_never_removes_a_preexisting_returned_library_id() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, _project_source, asset_id) = saved_core_with_media(tmp.path());
        let store = LibraryStore::new(tmp.path().join("library"));
        let other_source = tmp.path().join("other-project.mp4");
        fs::write(&other_source, b"other project bytes").unwrap();
        let existing = store
            .favorite(&FavoriteRequest {
                source: &other_source,
                kind: "video",
                category: None,
                favorited_at: 1.0,
                thumb: None,
            })
            .unwrap();
        let prepared = store
            .prepare_favorite(&FavoriteRequest {
                source: &other_source,
                kind: "video",
                category: None,
                favorited_at: 2.0,
                thumb: None,
            })
            .unwrap();
        assert!(!prepared.needs_publish());
        let before = core.media();
        let manifest_path = bundle.join(opentake_project::layout::MANIFEST_FILE);
        let manifest_before = fs::read(&manifest_path).unwrap();
        fs::remove_file(&manifest_path).unwrap();
        fs::create_dir(&manifest_path).unwrap();
        let project = core.runtime_snapshot();
        let project_dir = project.project_dir.clone().unwrap();
        let mut events = DeferredCoreEvents::default();

        let error = toggle_favorite_impl_with(
            &core,
            tmp.path(),
            &store,
            &asset_id,
            true,
            ExpectedFavoriteProject {
                epoch: project.project_epoch,
                dir: &project_dir,
            },
            &mut events,
            |_request| Ok(prepared),
        )
        .expect_err("project manifest publish must fail");

        assert!(
            error.contains("global favorite mapping could not be saved"),
            "{error}"
        );
        assert!(store.contains(&existing.id).unwrap());
        assert_eq!(store.entries().unwrap(), vec![existing.clone()]);
        assert!(store.stored_path(&existing.id).unwrap().unwrap().is_file());
        assert_eq!(core.media(), before);
        fs::remove_dir(&manifest_path).unwrap();
        fs::write(&manifest_path, manifest_before).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(reopened.media(), before);
    }

    #[test]
    fn failed_project_commit_keeps_new_favorite_invisible_when_library_manifest_is_blocked() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, _source, asset_id) = saved_core_with_media(tmp.path());
        let store = LibraryStore::new(tmp.path().join("library"));
        let before = core.media();
        let manifest_path = bundle.join(opentake_project::layout::MANIFEST_FILE);
        let manifest_before = fs::read(&manifest_path).unwrap();
        fs::remove_file(&manifest_path).unwrap();
        fs::create_dir(&manifest_path).unwrap();

        toggle_favorite_impl(&core, tmp.path(), &store, &asset_id, true)
            .expect_err("project manifest publish must fail");

        assert!(store.entries().unwrap().is_empty());
        assert_eq!(core.media(), before);
        fs::remove_dir(&manifest_path).unwrap();
        fs::write(&manifest_path, manifest_before).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(reopened.media(), before);
    }

    #[test]
    fn failed_global_publish_retries_to_a_complete_durable_favorite() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, _source, asset_id) = saved_core_with_media(tmp.path());
        let store = LibraryStore::new(tmp.path().join("library"));
        let project = core.runtime_snapshot();
        let project_dir = project.project_dir.clone().unwrap();
        let mut events = DeferredCoreEvents::default();

        let error = toggle_favorite_impl_with(
            &core,
            tmp.path(),
            &store,
            &asset_id,
            true,
            ExpectedFavoriteProject {
                epoch: project.project_epoch,
                dir: &project_dir,
            },
            &mut events,
            |request| {
                let prepared = store
                    .prepare_favorite(request)
                    .map_err(|error| error.to_string())?;
                fs::create_dir(store.root().join("library.json"))
                    .map_err(|error| error.to_string())?;
                Ok(prepared)
            },
        )
        .expect_err("blocked library manifest publish must fail");

        assert!(
            error.contains("global favorite could not be published"),
            "{error}"
        );
        fs::remove_dir(store.root().join("library.json")).unwrap();
        assert!(store.entries().unwrap().is_empty());
        assert!(store.stored_paths().unwrap().is_empty());
        assert_eq!(core.media().library_favorite_id(&asset_id), None);
        let reopened_stale = AppCore::new();
        reopened_stale.open_project(&bundle).unwrap();
        assert_eq!(reopened_stale.media().library_favorite_id(&asset_id), None);

        let retry = sync_project_favorites_impl(
            &reopened_stale,
            tmp.path(),
            &store,
            vec![asset_id.clone()],
        )
        .expect("sync retries unpublished favorite");

        assert!(retry.failures.is_empty());
        assert_eq!(retry.migrated_legacy_asset_ids, vec![asset_id.clone()]);
        let retried_library_id = reopened_stale
            .media()
            .library_favorite_id(&asset_id)
            .expect("retry persists project mapping")
            .to_string();
        assert_eq!(store.entries().unwrap().len(), 1);
        assert!(store.contains(&retried_library_id).unwrap());
        assert!(store
            .stored_path(&retried_library_id)
            .unwrap()
            .expect("retry publishes stored copy")
            .is_file());
        let reopened_converged = AppCore::new();
        reopened_converged.open_project(bundle).unwrap();
        assert_eq!(
            reopened_converged.media().library_favorite_id(&asset_id),
            Some(retried_library_id.as_str())
        );
    }

    #[test]
    fn deferred_favorite_events_allow_project_reentry_without_deadlock() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, _source, asset_id) = saved_core_with_media(tmp.path());
        let store = Arc::new(LibraryStore::new(tmp.path().join("library")));
        let media_reentered = Arc::new(AtomicBool::new(false));
        let saved_reentered = Arc::new(AtomicBool::new(false));
        let callback_core = core.clone();
        let callback_bundle = bundle.clone();
        let media_gate = Arc::clone(&media_reentered);
        let saved_gate = Arc::clone(&saved_reentered);
        core.subscribe(move |event| match event {
            opentake_core::CoreEvent::MediaChanged { .. }
                if !media_gate.swap(true, Ordering::SeqCst) =>
            {
                callback_core.new_project();
                callback_core.open_project(&callback_bundle).unwrap();
                callback_core.save_project(None).unwrap();
            }
            opentake_core::CoreEvent::ProjectSaved { .. }
                if !saved_gate.swap(true, Ordering::SeqCst) =>
            {
                callback_core.new_project();
                callback_core.open_project(&callback_bundle).unwrap();
                callback_core.save_project(None).unwrap();
            }
            _ => {}
        });
        let worker_core = core.clone();
        let worker_store = Arc::clone(&store);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result =
                toggle_favorite_impl(&worker_core, tmp.path(), &worker_store, &asset_id, true);
            done_tx.send(result).unwrap();
        });

        done_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("favorite event callback deadlocked")
            .expect("favorite succeeds");
        worker.join().unwrap();
        assert!(media_reentered.load(Ordering::SeqCst));
        assert!(saved_reentered.load(Ordering::SeqCst));
    }

    #[test]
    fn offline_unfavorite_uses_persisted_id_and_is_durable() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, source, asset_id) = saved_core_with_media(tmp.path());
        let store = LibraryStore::new(tmp.path().join("library"));
        toggle_favorite_impl(&core, tmp.path(), &store, &asset_id, true)
            .expect("favorite globally");
        fs::remove_file(source).expect("take source offline");

        toggle_favorite_impl(&core, tmp.path(), &store, &asset_id, false)
            .expect("unfavorite through persisted id");

        assert!(store.entries().unwrap().is_empty());
        let reopened = AppCore::new();
        reopened.open_project(bundle).expect("reopen project");
        assert!(!reopened.media().is_favorite(&asset_id));
        assert_eq!(reopened.media().library_favorite_id(&asset_id), None);
    }

    #[test]
    fn failed_global_remove_restores_and_persists_project_favorite() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, source, asset_id) = saved_core_with_media(tmp.path());
        let library_root = tmp.path().join("library");
        let store = LibraryStore::new(&library_root);
        toggle_favorite_impl(&core, tmp.path(), &store, &asset_id, true)
            .expect("favorite globally");
        let library_id = core
            .media()
            .library_favorite_id(&asset_id)
            .expect("project mapping")
            .to_string();
        fs::remove_file(source).expect("take original source offline");
        let manifest_path = library_root.join("library.json");
        let manifest_before = fs::read(&manifest_path).unwrap();
        fs::remove_file(&manifest_path).unwrap();
        fs::create_dir(&manifest_path).expect("block manifest read");

        let error = toggle_favorite_impl(&core, tmp.path(), &store, &asset_id, false)
            .expect_err("global remove must fail");

        assert!(
            error.contains("global favorite could not be removed"),
            "{error}"
        );
        fs::remove_dir(&manifest_path).unwrap();
        fs::write(&manifest_path, manifest_before).unwrap();
        assert!(store.contains(&library_id).unwrap());
        assert!(store.stored_path(&library_id).unwrap().unwrap().is_file());
        assert_eq!(
            core.media().library_favorite_id(&asset_id),
            Some(library_id.as_str())
        );
        let reopened = AppCore::new();
        reopened.open_project(bundle).expect("reopen project");
        assert_eq!(
            reopened.media().library_favorite_id(&asset_id),
            Some(library_id.as_str())
        );
    }

    #[test]
    fn reopened_stale_mapping_converges_without_resurrecting_removed_global_favorite() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, _source, asset_id) = saved_core_with_media(tmp.path());
        let store = LibraryStore::new(tmp.path().join("library"));
        toggle_favorite_impl(&core, tmp.path(), &store, &asset_id, true)
            .expect("favorite globally");
        let library_id = core
            .media()
            .library_favorite_id(&asset_id)
            .unwrap()
            .to_string();
        store
            .remove(&library_id)
            .expect("remove from another project");
        let reopened_after_global_delete = AppCore::new();
        reopened_after_global_delete
            .open_project(&bundle)
            .expect("reopen crash-window project");
        assert_eq!(
            reopened_after_global_delete
                .media()
                .library_favorite_id(&asset_id),
            Some(library_id.as_str())
        );

        let synced = sync_project_favorites_impl(
            &reopened_after_global_delete,
            tmp.path(),
            &store,
            vec![asset_id.clone()],
        )
        .expect("reconcile stale mapping");

        assert_eq!(synced.migrated_legacy_asset_ids, vec![asset_id.clone()]);
        assert!(store.entries().unwrap().is_empty());
        assert_eq!(
            reopened_after_global_delete
                .media()
                .library_favorite_id(&asset_id),
            None
        );
        let reopened = AppCore::new();
        reopened.open_project(bundle).expect("reopen project");
        assert!(!reopened.media().is_favorite(&asset_id));
    }

    #[test]
    fn sync_commits_stale_cleanup_when_another_copy_lookup_fails() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, _source_a, asset_a) = saved_core_with_media(tmp.path());
        let source_b = tmp.path().join("source-b.mp4");
        fs::write(&source_b, b"favorite B bytes").expect("write second media fixture");
        let asset_b = core
            .import_media_file(&source_b, "source-b", &ProbedMedia::default())
            .expect("import second fixture")
            .id;
        let store = LibraryStore::new(tmp.path().join("library"));
        let global_b = store
            .favorite(&FavoriteRequest {
                source: &source_b,
                kind: "video",
                category: None,
                favorited_at: 1.0,
                thumb: None,
            })
            .expect("favorite second fixture");
        core.set_media_global_favorite(&asset_a, Some("f".repeat(64)))
            .expect("seed stale mapping A");
        core.set_media_global_favorite(&asset_b, Some(global_b.id.clone()))
            .expect("seed valid mapping B");
        core.save_project(None).expect("persist both mappings");
        fs::remove_file(
            store
                .stored_path(&global_b.id)
                .expect("lookup second stored copy")
                .expect("second stored copy exists"),
        )
        .expect("remove second stored copy");
        opentake_media::library::fail_next_repair_stored_copy_for_test();

        let synced = sync_project_favorites_impl(&core, tmp.path(), &store, vec![])
            .expect("copy lookup errors are per-asset failures");

        assert_eq!(synced.failures.len(), 1);
        assert_eq!(synced.failures[0].asset_id, asset_b);
        assert!(synced.failures[0].message.starts_with("io:"));
        assert_eq!(core.media().library_favorite_id(&asset_a), None);
        assert_eq!(
            core.media()
                .library_favorite_id(&synced.failures[0].asset_id),
            Some(global_b.id.as_str())
        );
        let reopened = AppCore::new();
        reopened.open_project(bundle).expect("reopen project");
        assert_eq!(reopened.media().library_favorite_id(&asset_a), None);
        assert_eq!(
            reopened
                .media()
                .library_favorite_id(&synced.failures[0].asset_id),
            Some(global_b.id.as_str())
        );
    }

    #[test]
    fn sync_rejects_changed_source_when_repairing_expected_library_id() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, source, asset_id) = saved_core_with_media(tmp.path());
        let store = LibraryStore::new(tmp.path().join("library"));
        toggle_favorite_impl(&core, tmp.path(), &store, &asset_id, true)
            .expect("favorite globally");
        let library_id = core
            .media()
            .library_favorite_id(&asset_id)
            .unwrap()
            .to_string();
        let stored = store.stored_path(&library_id).unwrap().unwrap();
        fs::remove_file(stored).expect("remove durable copy");
        fs::write(&source, b"changed in place").expect("change source bytes");

        let synced = sync_project_favorites_impl(&core, tmp.path(), &store, vec![asset_id.clone()])
            .expect("sync returns per-asset failure");

        assert!(synced.migrated_legacy_asset_ids.is_empty());
        assert_eq!(synced.failures.len(), 1);
        assert_eq!(synced.failures[0].asset_id, asset_id);
        assert!(synced.failures[0]
            .message
            .contains("source content changed"));
        assert_eq!(store.entries().unwrap().len(), 1);
        assert_eq!(store.entries().unwrap()[0].id, library_id);
        assert!(store.stored_path(&library_id).unwrap().is_none());
        assert_eq!(
            core.media()
                .library_favorite_id(&synced.failures[0].asset_id),
            Some(library_id.as_str())
        );
        let reopened = AppCore::new();
        reopened.open_project(bundle).expect("reopen project");
        assert_eq!(
            reopened
                .media()
                .library_favorite_id(&synced.failures[0].asset_id),
            Some(library_id.as_str())
        );
    }

    #[test]
    fn sync_migrates_a_legacy_manifest_favorite_once_and_persists_it() {
        let tmp = tempfile::tempdir().expect("temp root");
        let (core, bundle, _source, asset_id) = saved_core_with_media(tmp.path());
        let store = LibraryStore::new(tmp.path().join("library"));
        core.set_media_favorite(std::slice::from_ref(&asset_id), true)
            .expect("seed legacy favorite");
        core.save_project(None).expect("persist legacy favorite");

        let first = sync_project_favorites_impl(&core, tmp.path(), &store, vec![asset_id.clone()])
            .expect("migrate legacy favorite");
        let second = sync_project_favorites_impl(&core, tmp.path(), &store, vec![asset_id.clone()])
            .expect("repeat migration");

        assert_eq!(first.migrated_legacy_asset_ids, vec![asset_id.clone()]);
        assert_eq!(second.migrated_legacy_asset_ids, vec![asset_id.clone()]);
        assert_eq!(store.entries().unwrap().len(), 1);
        let reopened = AppCore::new();
        reopened.open_project(bundle).expect("reopen project");
        assert!(reopened.media().library_favorite_id(&asset_id).is_some());
    }

    #[test]
    fn import_commands_refuse_unknown_project_without_manifest_or_folder_change() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let root = tmp.path();
        let core = unknown_core(root);
        let engine = engine_for(root);
        let explicit = root.join("explicit.mp4");
        touch(&explicit);
        let flat = root.join("flat");
        fs::create_dir(&flat).expect("create flat fixture");
        touch(&flat.join("flat.mp4"));
        let recursive = root.join("recursive");
        fs::create_dir_all(recursive.join("nested")).expect("create recursive fixture");
        touch(&recursive.join("nested/recursive.mp4"));
        let empty = root.join("empty");
        fs::create_dir(&empty).expect("create empty fixture");
        let before = core.media();
        let scheduler = prewarm::PrewarmScheduler::new(core.project_revision().project_epoch);

        let explicit_error = import_media_impl(
            &core,
            &engine,
            &scheduler,
            vec![explicit.to_string_lossy().into_owned()],
        )
        .expect_err("explicit import must be rejected");
        assert!(
            explicit_error.contains("compatibility read-only"),
            "{explicit_error}"
        );
        assert_eq!(core.media(), before);
        let flat_error = import_folder_impl(
            &core,
            &engine,
            &scheduler,
            flat.to_string_lossy().into_owned(),
            Some(false),
        )
        .expect_err("flat import must be rejected");
        assert!(
            flat_error.contains("compatibility read-only"),
            "{flat_error}"
        );
        assert_eq!(core.media(), before);
        let recursive_error = import_folder_impl(
            &core,
            &engine,
            &scheduler,
            recursive.to_string_lossy().into_owned(),
            Some(true),
        )
        .expect_err("recursive import must be rejected");
        assert!(
            recursive_error.contains("compatibility read-only"),
            "{recursive_error}"
        );
        assert_eq!(core.media(), before);
        let empty_error = import_folder_impl(
            &core,
            &engine,
            &scheduler,
            empty.to_string_lossy().into_owned(),
            Some(true),
        )
        .expect_err("empty import must be rejected");
        assert!(
            empty_error.contains("compatibility read-only"),
            "{empty_error}"
        );
        assert_eq!(core.media(), before);
    }

    #[test]
    fn explicit_import_persists_manifest_before_command_returns() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("ImportPersisted.opentake");
        let source = tmp.path().join("still.png");
        image::RgbaImage::from_pixel(32, 24, image::Rgba([10, 20, 30, 255]))
            .save(&source)
            .expect("write import fixture");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save empty project");
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.runtime_snapshot().project_epoch);

        let imported = import_media_impl(
            &core,
            &engine,
            &scheduler,
            vec![source.to_string_lossy().into_owned()],
        )
        .expect("import media");

        assert_eq!(imported.items.len(), 1);
        let reopened = AppCore::new();
        reopened
            .open_project(bundle)
            .expect("reopen imported project");
        assert_eq!(reopened.media().entries.len(), 1);
        assert_eq!(reopened.media().entries[0].name, "still");
    }

    #[test]
    fn explicit_import_rejects_file_count_and_aggregate_bytes_before_mutation() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("ImportLimits.opentake");
        let first = tmp.path().join("first.mp4");
        let second = tmp.path().join("second.mp4");
        fs::write(&first, b"12345").unwrap();
        fs::write(&second, b"67890").unwrap();
        let paths = vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ];
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let before = core.media();
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.runtime_snapshot().project_epoch);

        let file_error = import_media_impl_with_options(
            &core,
            &engine,
            &scheduler,
            paths.clone(),
            ExplicitImportLimits {
                max_files: 1,
                max_aggregate_bytes: 100,
            },
            || {},
        )
        .expect_err("file-count admission must fail");
        assert!(file_error.contains("files_limit_exceeded"), "{file_error}");
        assert_eq!(core.media(), before);

        let byte_error = import_media_impl_with_options(
            &core,
            &engine,
            &scheduler,
            paths,
            ExplicitImportLimits {
                max_files: 2,
                max_aggregate_bytes: 9,
            },
            || {},
        )
        .expect_err("aggregate-byte admission must fail");
        assert!(
            byte_error.contains("aggregate_bytes_limit_exceeded"),
            "{byte_error}"
        );
        assert_eq!(core.media(), before);

        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(reopened.media(), before);
    }

    #[test]
    fn explicit_import_rejects_a_path_replacement_before_atomic_commit() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("ImportRace.opentake");
        let source = tmp.path().join("still.png");
        let retained_old = tmp.path().join("retained-old.png");
        image::RgbaImage::from_pixel(32, 24, image::Rgba([10, 20, 30, 255]))
            .save(&source)
            .unwrap();
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let before = core.media();
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.runtime_snapshot().project_epoch);

        let error = import_media_impl_with_options(
            &core,
            &engine,
            &scheduler,
            vec![source.to_string_lossy().into_owned()],
            EXPLICIT_IMPORT_LIMITS,
            || {
                fs::rename(&source, &retained_old).unwrap();
                image::RgbaImage::from_pixel(32, 24, image::Rgba([200, 100, 50, 255]))
                    .save(&source)
                    .unwrap();
            },
        )
        .expect_err("a replaced selected path must fail before manifest mutation");
        assert!(
            error.contains("explicit_import_source_changed_before_commit"),
            "{error}"
        );
        assert_eq!(core.media(), before);

        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(reopened.media(), before);
    }

    #[test]
    fn explicit_import_rejects_in_place_growth_before_atomic_commit() {
        use std::io::Write;

        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("ImportGrowthRace.opentake");
        let source = tmp.path().join("still.png");
        image::RgbaImage::from_pixel(32, 24, image::Rgba([10, 20, 30, 255]))
            .save(&source)
            .unwrap();
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let before = core.media();
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.runtime_snapshot().project_epoch);

        let error = import_media_impl_with_options(
            &core,
            &engine,
            &scheduler,
            vec![source.to_string_lossy().into_owned()],
            EXPLICIT_IMPORT_LIMITS,
            || {
                fs::OpenOptions::new()
                    .append(true)
                    .open(&source)
                    .unwrap()
                    .write_all(b"grown-after-planning")
                    .unwrap();
            },
        )
        .expect_err("in-place growth must invalidate retained admission metadata");
        assert!(
            error.contains("explicit_import_source_changed_before_commit"),
            "{error}"
        );
        assert_eq!(core.media(), before);

        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(reopened.media(), before);
    }

    #[cfg(unix)]
    #[test]
    fn explicit_import_rejects_a_fifo_without_waiting_for_a_writer() {
        use std::os::unix::ffi::OsStrExt;

        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("ImportFifo.opentake");
        let fifo = tmp.path().join("blocked.mp4");
        let fifo_name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: `fifo_name` is a valid, NUL-terminated path in our temp dir.
        assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);
        let core = AppCore::new();
        core.save_project(Some(bundle)).unwrap();
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.runtime_snapshot().project_epoch);

        let started = std::time::Instant::now();
        let imported = import_media_impl_with_options(
            &core,
            &engine,
            &scheduler,
            vec![fifo.to_string_lossy().into_owned()],
            EXPLICIT_IMPORT_LIMITS,
            || {},
        )
        .expect("unsupported special file must be skipped");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "non-blocking/no-recall admission must not wait for a FIFO writer"
        );
        assert!(imported.items.is_empty());
        assert!(core.media().entries.is_empty());
    }

    #[test]
    fn flat_folder_import_persists_manifest_before_command_returns() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("FolderImportPersisted.opentake");
        let source_dir = tmp.path().join("source");
        fs::create_dir(&source_dir).expect("create source folder");
        image::RgbaImage::from_pixel(32, 24, image::Rgba([40, 50, 60, 255]))
            .save(source_dir.join("still.png"))
            .expect("write import fixture");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save empty project");
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.runtime_snapshot().project_epoch);

        let imported = import_folder_impl(
            &core,
            &engine,
            &scheduler,
            source_dir.to_string_lossy().into_owned(),
            Some(false),
        )
        .expect("import folder");

        assert_eq!(imported.items.len(), 1);
        let reopened = AppCore::new();
        reopened
            .open_project(bundle)
            .expect("reopen imported project");
        assert_eq!(reopened.media().entries.len(), 1);
    }

    #[test]
    fn recursive_folder_import_persists_folders_and_media_before_command_returns() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let bundle = tmp.path().join("RecursiveImportPersisted.opentake");
        let source_dir = tmp.path().join("source");
        let nested_dir = source_dir.join("nested");
        fs::create_dir_all(&nested_dir).expect("create nested source folder");
        image::RgbaImage::from_pixel(32, 24, image::Rgba([70, 80, 90, 255]))
            .save(nested_dir.join("still.png"))
            .expect("write import fixture");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save empty project");
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.runtime_snapshot().project_epoch);

        let imported = import_folder_impl(
            &core,
            &engine,
            &scheduler,
            source_dir.to_string_lossy().into_owned(),
            Some(true),
        )
        .expect("import recursive folder");

        assert_eq!(imported.items.len(), 1);
        assert_eq!(imported.folders.len(), 2);
        let reopened = AppCore::new();
        reopened
            .open_project(bundle)
            .expect("reopen imported project");
        assert_eq!(reopened.media().entries.len(), 1);
        assert_eq!(reopened.media().folders.len(), 2);
        assert!(reopened.media().entries[0].folder_id.is_some());
    }

    #[test]
    fn save_clip_as_media_refuses_before_media_output_creation() {
        let tmp = tempfile::tempdir().expect("create temp root");
        let core = unknown_core(tmp.path());
        let media_tree = core.project_dir().expect("opened project").join("media");
        fs::create_dir_all(media_tree.join("existing")).expect("create media tree fixture");
        fs::write(media_tree.join("existing/keep.bin"), b"before")
            .expect("write media tree fixture");
        let before = recursive_tree(&media_tree);
        let called = std::cell::Cell::new(false);
        let sentinel = media_tree.join("workflow-ran-before-guard.bin");

        let error = save_clip_as_media_impl(&core, || {
            called.set(true);
            fs::write(&sentinel, b"bad ordering").expect("write workflow sentinel");
            Err("workflow should not run".into())
        })
        .expect_err("save clip must be rejected");

        assert!(error.contains("compatibility read-only"), "{error}");
        assert!(!called.get());
        assert!(!sentinel.exists());
        assert_eq!(recursive_tree(&media_tree), before);
    }

    #[test]
    fn single_clip_save_accepts_video_and_audio_but_rejects_image() {
        assert_eq!(save_clip_extension(ClipType::Video).unwrap(), "mp4");
        assert_eq!(save_clip_extension(ClipType::Audio).unwrap(), "wav");
        assert_eq!(
            save_clip_extension(ClipType::Image).unwrap_err(),
            "only video and audio clips can be saved as media"
        );
    }

    #[test]
    fn completed_video_metadata_uses_encoder_facts_without_probe_defaults() {
        let metadata = SavedMediaMetadata::Video(crate::export::ExportSummary {
            out_path: "/unused/generated.mp4".to_string(),
            width: 1920,
            height: 1080,
            fps: 30,
            frame_count: 75,
            has_audio: true,
        });

        assert_eq!(
            metadata.to_probe().expect("valid producer metadata"),
            ProbedMedia {
                duration_secs: 2.5,
                width: Some(1920),
                height: Some(1080),
                fps: Some(30.0),
                has_audio: true,
                color: None,
            }
        );
    }

    #[test]
    fn completed_video_metadata_rejects_nonpositive_frame_counts() {
        for frame_count in [0, -1] {
            let metadata = SavedMediaMetadata::Video(crate::export::ExportSummary {
                out_path: "/unused/generated.mp4".to_string(),
                width: 1920,
                height: 1080,
                fps: 30,
                frame_count,
                has_audio: false,
            });

            assert_eq!(
                metadata
                    .to_probe()
                    .expect_err("saved video needs at least one encoded frame"),
                "invalid completed video metadata"
            );
        }
    }

    #[test]
    fn zero_duration_video_save_is_rejected_without_output_or_manifest_progress() {
        use std::sync::Mutex;

        use opentake_domain::{Clip, Track};

        let tmp = tempfile::tempdir().expect("temp root");
        let bundle = tmp.path().join("ZeroDurationSave.opentake");
        let source = tmp.path().join("source.mp4");
        touch(&source);
        let mut project = opentake_project::Project::new(&bundle);
        let mut track = Track::new("video", ClipType::Video);
        track.clips.push(Clip::new("zero", "source", 0, 0));
        project.timeline.tracks.push(track);
        project.manifest.entries.push(MediaManifestEntry {
            id: "source".into(),
            name: "source".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: source.to_string_lossy().into_owned(),
            },
            duration: 0.0,
            generation_input: None,
            source_width: Some(320),
            source_height: Some(240),
            source_fps: Some(30.0),
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        project.save().expect("save zero-duration fixture");

        let core = AppCore::new();
        core.open_project(bundle.clone()).expect("open fixture");
        let before_live = core.media();
        let manifest_path = bundle.join("media.json");
        let before_disk = fs::read(&manifest_path).expect("read persisted manifest");
        let media_dir = bundle.join("media");
        let before_outputs = recursive_tree(&media_dir);
        let progress = Arc::new(Mutex::new(Vec::<(i32, i32)>::new()));
        let observed = Arc::clone(&progress);
        let on_progress: crate::export::AudioExportProgress = Arc::new(move |done, total| {
            observed
                .lock()
                .expect("record save progress")
                .push((done, total));
        });
        let control = crate::export::ExportControl::default();
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.runtime_snapshot().project_epoch);

        let error = save_clip_as_media_workflow(
            &core,
            &control,
            &engine,
            &scheduler,
            "zero",
            "save-as:zero-duration",
            on_progress,
        )
        .expect_err("zero-duration video save must fail");

        assert_eq!(error, "clip duration must be greater than zero");
        assert_eq!(recursive_tree(&media_dir), before_outputs);
        assert_eq!(core.media(), before_live, "live manifest must not change");
        assert_eq!(
            fs::read(&manifest_path).expect("reread persisted manifest"),
            before_disk,
            "persisted manifest must not change"
        );
        assert!(
            progress
                .lock()
                .expect("inspect save progress")
                .iter()
                .all(|(done, total)| done != total),
            "failed save must not emit terminal progress"
        );
        let reopened = AppCore::new();
        reopened.open_project(bundle).expect("reopen fixture");
        assert_eq!(reopened.media(), before_live);
        assert!(
            control.try_begin("save-as:after-zero-duration").is_ok(),
            "failed preflight must not retain the export operation"
        );
    }

    #[test]
    fn completed_wav_metadata_uses_mono_sample_count_and_rate() {
        let metadata = SavedMediaMetadata::Wav {
            sample_count: 24_000,
            sample_rate: 48_000,
        };

        assert_eq!(
            metadata.to_probe().expect("valid producer metadata"),
            ProbedMedia {
                duration_secs: 0.5,
                width: None,
                height: None,
                fps: None,
                has_audio: true,
                color: None,
            }
        );
    }

    #[test]
    fn audio_clip_save_writes_wav_and_imports_project_relative_source() {
        let tmp = tempfile::tempdir().expect("temp root");
        let bundle = tmp.path().join("AudioSave.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save project");
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.runtime_snapshot().project_epoch);
        let output = crate::export::unique_project_media_path(&bundle, "clip_audio", "wav")
            .expect("project output");
        crate::export::write_wav_s16le(&[0.0; 480], 48_000, &output).expect("write audio output");

        import_saved_media(
            &core,
            &engine,
            &scheduler,
            core.runtime_snapshot().project_epoch,
            &bundle,
            &output,
        )
        .expect("import saved audio");

        let entry = core
            .media()
            .entries
            .into_iter()
            .find(|candidate| candidate.name.starts_with("clip_audio"))
            .expect("imported entry");
        let relative_path = match entry.source {
            MediaSource::Project { relative_path } => relative_path,
            source => panic!("saved audio was not project-relative: {source:?}"),
        };
        assert_eq!(bundle.join(&relative_path), output);
        assert_eq!(
            output.extension().and_then(|value| value.to_str()),
            Some("wav")
        );

        let _ = fs::remove_dir_all(engine.cache_root());
        assert!(bundle.join(relative_path).is_file());
    }

    #[test]
    fn project_switch_before_saved_media_transaction_leaves_replacement_unchanged() {
        let tmp = tempfile::tempdir().expect("temp root");
        let project_a = tmp.path().join("A.opentake");
        let project_b = tmp.path().join("B.opentake");
        let core = AppCore::new();
        core.save_project(Some(project_a.clone()))
            .expect("save project A");
        let expected_epoch = core.runtime_snapshot().project_epoch;
        let output = crate::export::unique_project_media_path(&project_a, "clip_audio", "wav")
            .expect("project output");
        crate::export::write_wav_s16le(&[0.0; 480], 48_000, &output)
            .expect("write rendered output");
        opentake_project::Project::new(&project_b)
            .save()
            .expect("save project B");
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(expected_epoch);
        let before_transaction = || {
            core.open_project(project_b.clone())
                .expect("switch to project B after render");
        };

        let result = crate::export::cleanup_partial_output(
            &output,
            import_saved_media_with_before_transaction(
                &core,
                &engine,
                &scheduler,
                expected_epoch,
                &project_a,
                &output,
                before_transaction,
            ),
        );
        let before = serde_json::to_vec(&opentake_domain::MediaManifest::default())
            .expect("serialize expected B manifest");

        assert_eq!(
            result.expect_err("stale import must fail"),
            "project changed while saving media"
        );
        assert!(!output.exists(), "failed save must clean project A output");
        assert_eq!(
            serde_json::to_vec(&core.media()).expect("serialize actual B manifest"),
            before,
            "project B manifest must remain byte-for-byte unchanged"
        );
    }

    #[cfg(unix)]
    #[test]
    fn final_identity_swap_rolls_back_live_and_persisted_saved_media_import() {
        let tmp = tempfile::tempdir().expect("temp root");
        let bundle = tmp.path().join("IdentityRollback.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save project");
        let snapshot = core.runtime_snapshot();
        let before_live = snapshot.media.clone();
        let manifest_path = bundle.join("media.json");
        let before_disk = fs::read(&manifest_path).expect("read persisted manifest");
        let output =
            crate::export::reserve_project_media_output(&bundle, "post_import_swap", "wav")
                .expect("reserve saved-media output");
        let output_path = output.path().to_path_buf();
        let moved_original = output_path.with_extension("moved");
        let mut writer = output.writer().expect("clone saved-media writer");
        crate::export::write_wav_s16le_cancellable_to_file(
            &[0.0; 480],
            48_000,
            &mut writer,
            &opentake_media::MediaCancelToken::new(),
            None,
            None,
        )
        .expect("write rendered WAV");
        drop(writer);
        output.prepare_commit().expect("pre-import identity valid");
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(snapshot.project_epoch);
        let probe = SavedMediaMetadata::Wav {
            sample_count: 480,
            sample_rate: 48_000,
        }
        .to_probe()
        .expect("construct trusted saved-media metadata");

        let result = import_saved_media_with_hooks(
            SavedMediaImportContext {
                core: &core,
                engine: &engine,
                prewarm: &scheduler,
                expected_project_epoch: snapshot.project_epoch,
                expected_project_dir: &bundle,
                path: &output_path,
                probe: &probe,
            },
            (
                || {},
                || {
                    fs::rename(&output_path, &moved_original)
                        .map_err(|error| format!("swap reserved output: {error}"))?;
                    fs::write(&output_path, b"replacement")
                        .map_err(|error| format!("install replacement: {error}"))?;
                    output.verify_identity()
                },
            ),
        );

        let error = result.expect_err("post-import identity swap must abort transaction");
        assert!(error.contains("output changed"), "{error}");
        assert_eq!(
            core.media(),
            before_live,
            "failed postcondition must restore the live manifest"
        );
        assert_eq!(
            fs::read(&manifest_path).expect("reread persisted manifest"),
            before_disk,
            "failed postcondition must not alter persisted media.json"
        );
        let reopened = AppCore::new();
        reopened
            .open_project(bundle.clone())
            .expect("reopen project after rollback");
        assert_eq!(
            reopened.media(),
            before_live,
            "reopened project must not contain a dangling saved-media entry"
        );
        assert_eq!(
            fs::read(&output_path).expect("replacement remains untouched"),
            b"replacement"
        );
        drop(output);
        assert_eq!(
            fs::read(&output_path).expect("replacement remains after cleanup"),
            b"replacement"
        );
        if moved_original.exists() {
            assert_eq!(
                fs::metadata(&moved_original)
                    .expect("inspect moved retained output")
                    .len(),
                0,
                "failed output payload must be destroyed through its retained handle"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn transient_final_path_swap_cannot_change_trusted_saved_media_metadata() {
        let tmp = tempfile::tempdir().expect("temp root");
        let bundle = tmp.path().join("TrustedMetadataIdentity.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save project");
        let snapshot = core.runtime_snapshot();
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(snapshot.project_epoch);
        let output =
            crate::export::reserve_project_media_output(&bundle, "trusted_swap_restore", "wav")
                .expect("reserve output");
        let output_path = output.path().to_path_buf();
        let moved_output = output_path.with_extension("retained");
        let replacement = tmp.path().join("replacement.wav");
        let mut writer = output.writer().expect("clone retained output");
        crate::export::write_wav_s16le_cancellable_to_file(
            &[0.0; 480],
            48_000,
            &mut writer,
            &MediaCancelToken::new(),
            None,
            None,
        )
        .expect("write 10ms retained WAV");
        drop(writer);
        fs::File::create(&replacement).expect("create replacement WAV");
        crate::export::write_wav_s16le(&[0.0; 4_800], 48_000, &replacement)
            .expect("write 100ms replacement WAV");
        let retained_duration = engine
            .probe(&output_path)
            .expect("ffprobe retained WAV")
            .duration_secs;
        let replacement_duration = engine
            .probe(&replacement)
            .expect("ffprobe replacement WAV")
            .duration_secs;
        assert!(replacement_duration > retained_duration * 5.0);

        let control = crate::export::ExportControl::default();
        let mut guard = control
            .try_begin("trusted-metadata-test")
            .expect("begin save generation");
        let result = finalize_saved_media_with_hooks(
            SavedMediaFinalizationContext {
                core: &core,
                engine: &engine,
                prewarm: &scheduler,
                expected_project_epoch: snapshot.project_epoch,
                expected_project_dir: &bundle,
                metadata: SavedMediaMetadata::Wav {
                    sample_count: 480,
                    sample_rate: 48_000,
                },
                on_progress: &|_, _| {},
            },
            output,
            &mut guard,
            (
                || {},
                || {
                    fs::rename(&output_path, &moved_output).expect("move retained final name");
                    fs::copy(&replacement, &output_path).expect("install transient replacement");
                    fs::remove_file(&output_path).expect("remove transient replacement");
                    fs::rename(&moved_output, &output_path).expect("restore retained final name");
                },
                || {},
                || {},
            ),
        )
        .expect("trusted producer metadata must commit");

        let imported = result
            .items
            .iter()
            .find(|item| item.name.starts_with("trusted_swap_restore"))
            .expect("saved media imported");
        assert!((imported.duration - retained_duration).abs() < 0.001);
        assert!((imported.duration - replacement_duration).abs() > 0.05);
        core.save_project(None).expect("persist imported manifest");
        let reopened = AppCore::new();
        reopened
            .open_project(bundle)
            .expect("reopen project after trusted metadata import");
        let reopened_entry = reopened
            .media()
            .entries
            .into_iter()
            .find(|entry| entry.name.starts_with("trusted_swap_restore"))
            .expect("reopened saved media entry");
        assert!((reopened_entry.duration - retained_duration).abs() < 0.001);
        assert!(output_path.is_file());
        assert!(!moved_output.exists());
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum FinalizationCancelStage {
        Sync,
        Metadata,
        Import,
        Commit,
    }

    fn assert_finalization_cancelled_at(stage: FinalizationCancelStage) {
        let tmp = tempfile::tempdir().expect("temp root");
        let bundle = tmp.path().join("CancelledFinalization.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save project");
        let snapshot = core.runtime_snapshot();
        let before_live = snapshot.media.clone();
        let manifest_path = bundle.join("media.json");
        let before_disk = fs::read(&manifest_path).expect("read persisted manifest");
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(snapshot.project_epoch);
        let output =
            crate::export::reserve_project_media_output(&bundle, "cancelled_finalization", "wav")
                .expect("reserve output");
        let output_path = output.path().to_path_buf();
        let mut writer = output.writer().expect("clone output");
        crate::export::write_wav_s16le_cancellable_to_file(
            &[0.0; 480],
            48_000,
            &mut writer,
            &MediaCancelToken::new(),
            None,
            None,
        )
        .expect("write output");
        drop(writer);

        let control = crate::export::ExportControl::default();
        let mut guard = control
            .try_begin("cancel-finalization-test")
            .expect("begin save generation");
        let cancel = guard.cancel_token().clone();
        let sync_cancel = cancel.clone();
        let metadata_cancel = cancel.clone();
        let import_cancel = cancel.clone();
        let commit_cancel = cancel;
        let terminal_progress = std::cell::Cell::new(false);
        let result = finalize_saved_media_with_hooks(
            SavedMediaFinalizationContext {
                core: &core,
                engine: &engine,
                prewarm: &scheduler,
                expected_project_epoch: snapshot.project_epoch,
                expected_project_dir: &bundle,
                metadata: SavedMediaMetadata::Wav {
                    sample_count: 480,
                    sample_rate: 48_000,
                },
                on_progress: &|done, total| {
                    if done == total {
                        terminal_progress.set(true);
                    }
                },
            },
            output,
            &mut guard,
            (
                move || {
                    if stage == FinalizationCancelStage::Sync {
                        sync_cancel.cancel();
                    }
                },
                move || {
                    if stage == FinalizationCancelStage::Metadata {
                        metadata_cancel.cancel();
                    }
                },
                move || {
                    if stage == FinalizationCancelStage::Import {
                        import_cancel.cancel();
                    }
                },
                move || {
                    if stage == FinalizationCancelStage::Commit {
                        commit_cancel.cancel();
                    }
                },
            ),
        );

        assert_eq!(
            result.expect_err("cancellation must abort finalization"),
            crate::export::CANCELLED_SENTINEL
        );
        assert!(
            !terminal_progress.get(),
            "cancelled save must not emit 100%"
        );
        assert!(!output_path.exists(), "cancelled output must be removed");
        assert_eq!(core.media(), before_live, "live manifest must roll back");
        assert_eq!(
            fs::read(&manifest_path).expect("reread persisted manifest"),
            before_disk,
            "persisted manifest must remain unchanged"
        );
        let reopened = AppCore::new();
        reopened
            .open_project(bundle)
            .expect("reopen project after cancellation");
        assert_eq!(reopened.media(), before_live);
    }

    #[test]
    fn cancellation_during_sync_aborts_finalization_without_terminal_progress() {
        assert_finalization_cancelled_at(FinalizationCancelStage::Sync);
    }

    #[test]
    fn cancellation_after_metadata_aborts_finalization_without_terminal_progress() {
        assert_finalization_cancelled_at(FinalizationCancelStage::Metadata);
    }

    #[test]
    fn cancellation_before_import_rolls_back_without_terminal_progress() {
        assert_finalization_cancelled_at(FinalizationCancelStage::Import);
    }

    #[test]
    fn cancellation_inside_core_commit_rolls_back_without_terminal_progress() {
        assert_finalization_cancelled_at(FinalizationCancelStage::Commit);
    }

    #[test]
    fn range_saved_media_survives_export_cache_deletion() {
        let tmp = tempfile::tempdir().expect("temp root");
        let bundle = tmp.path().join("RangeSave.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save project");
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.runtime_snapshot().project_epoch);
        let output = crate::export::unique_project_media_path(&bundle, "range_10_20", "mp4")
            .expect("project output");
        fs::write(&output, b"rendered range").expect("write range output");

        import_saved_media(
            &core,
            &engine,
            &scheduler,
            core.runtime_snapshot().project_epoch,
            &bundle,
            &output,
        )
        .expect("import saved range");
        let entry = core
            .media()
            .entries
            .into_iter()
            .find(|candidate| candidate.name.starts_with("range_10_20"))
            .expect("imported range");
        let relative_path = match entry.source {
            MediaSource::Project { relative_path } => relative_path,
            source => panic!("saved range was not project-relative: {source:?}"),
        };

        let _ = fs::remove_dir_all(engine.cache_root());
        assert_eq!(bundle.join(&relative_path), output);
        assert!(bundle.join(relative_path).is_file());
    }

    #[test]
    fn dto_projects_external_entry_with_path() {
        let entry = MediaManifestEntry {
            id: "a".into(),
            name: "clip".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/abs/clip.mp4".into(),
            },
            duration: 3.0,
            generation_input: None,
            source_width: Some(640),
            source_height: Some(480),
            source_fps: Some(24.0),
            has_audio: Some(true),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        };
        let dto = MediaItemDto::from_entry(&entry, None, None, false);
        assert_eq!(dto.id, "a");
        assert_eq!(dto.kind, ClipType::Video);
        assert_eq!(dto.duration, 3.0);
        assert_eq!(dto.width, Some(640));
        assert!(dto.has_audio);
        assert_eq!(dto.path.as_deref(), Some("/abs/clip.mp4"));
        assert_eq!(dto.thumbnail, None);
        // /abs/clip.mp4 doesn't exist → missing is true (existence-derived), and
        // a missing source has no readable size or generation snapshot.
        assert!(dto.missing);
        assert_eq!(dto.file_size, None);
        assert_eq!(dto.generation_input, None);
    }

    #[test]
    fn dto_projects_project_relative_entry_with_resolved_path() {
        let bundle = tempfile::tempdir().unwrap();
        let relative_path = PathBuf::from("media/stems/job/vocals.wav");
        let source = bundle.path().join(&relative_path);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"derived-audio").unwrap();
        let entry = MediaManifestEntry {
            id: "stem-vocals".into(),
            name: "Mix Vocals".into(),
            kind: ClipType::Audio,
            source: MediaSource::Project {
                relative_path: relative_path.to_string_lossy().into_owned(),
            },
            duration: 5.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(true),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        };

        let dto = MediaItemDto::from_entry(&entry, Some(bundle.path()), None, false);

        assert_eq!(dto.path.as_deref(), Some(source.to_string_lossy().as_ref()));
        assert!(!dto.missing);
        assert_eq!(dto.file_size, Some(13));
    }

    #[test]
    fn dto_reports_file_size_for_present_source() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        fs::write(&source, b"0123456789").unwrap(); // 10 bytes
        let entry = MediaManifestEntry {
            id: "a".into(),
            name: "clip".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: source.to_string_lossy().into_owned(),
            },
            duration: 3.0,
            generation_input: None,
            source_width: Some(640),
            source_height: Some(480),
            source_fps: Some(24.0),
            has_audio: Some(true),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        };
        let dto = MediaItemDto::from_entry(&entry, None, None, false);
        assert!(!dto.missing);
        assert_eq!(dto.file_size, Some(10));
    }

    #[cfg(unix)]
    #[test]
    fn dto_treats_a_fifo_source_as_offline_without_opening_it() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("blocking-source.mov");
        let source_c = CString::new(source.as_os_str().as_bytes()).unwrap();
        // SAFETY: `source_c` is a valid, NUL-terminated path owned for the call.
        assert_eq!(unsafe { libc::mkfifo(source_c.as_ptr(), 0o600) }, 0);
        let entry = MediaManifestEntry {
            id: "fifo".into(),
            name: "blocking-source".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: source.to_string_lossy().into_owned(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(false),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        };

        let dto = MediaItemDto::from_entry(&entry, None, None, false);

        assert!(dto.missing);
        assert_eq!(dto.file_size, None);
        assert_eq!(dto.thumbnail, None);
    }

    #[test]
    fn media_item_dto_serializes_camel_case() {
        let dto = MediaItemDto {
            id: "a".into(),
            name: "n".into(),
            kind: ClipType::Image,
            duration: 0.0,
            width: Some(10),
            height: Some(20),
            source_fps: Some(24.0),
            has_audio: false,
            color: None,
            is_hdr: false,
            path: Some("/p.png".into()),
            proxy_path: None,
            proxy_width: None,
            proxy_height: None,
            thumbnail: None,
            folder_id: None,
            file_size: Some(2048),
            generation_input: None,
            generation_status: "none".into(),
            generation_progress: None,
            generation_error_code: None,
            missing: false,
            favorite: true,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"hasAudio\""));
        assert!(json.contains("\"type\":\"image\""));
        assert!(json.contains("\"thumbnail\":null"));
        assert!(json.contains("\"folderId\":null"));
        assert!(json.contains("\"fileSize\":2048"));
        assert!(json.contains("\"sourceFps\":24.0"));
        assert!(json.contains("\"generationInput\":null"));
        assert!(json.contains("\"missing\":false"));
        assert!(json.contains("\"favorite\":true"));
    }

    #[test]
    fn media_item_uses_existing_cached_thumbnail_without_decoding() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        touch(&source);
        let cache_root = tmp.path().join("cache");
        let key = cache_key_for(&source).unwrap();
        let poster = poster_path_for(&cache_root, &key);
        fs::create_dir_all(poster.parent().unwrap()).unwrap();
        fs::write(&poster, b"cached").unwrap();
        let entry = MediaManifestEntry {
            id: "a".into(),
            name: "clip".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: source.to_string_lossy().into_owned(),
            },
            duration: 60.0 * 60.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        };

        let dto = MediaItemDto::from_entry(&entry, None, Some(&cache_root), false);

        assert!(!dto.missing);
        let poster_string = poster.to_string_lossy().into_owned();
        assert_eq!(dto.thumbnail.as_deref(), Some(poster_string.as_str()));
    }

    #[test]
    fn preview_poster_path_is_distinct_from_grid_poster() {
        // The hi-res preview poster and the small grid poster must never share a
        // cache file, or one size would clobber the other.
        let root = Path::new("/cache");
        let key = "abc123";
        assert_ne!(
            preview_poster_path_for(root, key, 0.0),
            poster_path_for(root, key),
            "preview poster must not collide with the grid poster"
        );
        assert!(preview_poster_path_for(root, key, 0.0)
            .to_string_lossy()
            .ends_with("abc123.preview.png"));
    }

    #[test]
    fn preview_poster_path_encodes_nonzero_time() {
        let root = Path::new("/cache");
        let key = "k";
        // t=0 → base name; t>0 → millisecond-suffixed, and distinct per time.
        assert!(preview_poster_path_for(root, key, 0.0)
            .to_string_lossy()
            .ends_with("k.preview.png"));
        assert!(preview_poster_path_for(root, key, 1.5)
            .to_string_lossy()
            .ends_with("k.preview.1500.png"));
        assert_ne!(
            preview_poster_path_for(root, key, 1.0),
            preview_poster_path_for(root, key, 2.0)
        );
    }

    #[test]
    fn imported_grid_poster_is_coalesced_and_stale_queue_never_publishes() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("still.png");
        image::RgbaImage::from_pixel(32, 24, image::Rgba([10, 20, 30, 255]))
            .save(&source)
            .unwrap();
        let core = AppCore::new();
        let engine = engine_for(tmp.path());
        let epoch = core.project_revision().project_epoch;
        let scheduler = prewarm::PrewarmScheduler::new(epoch);

        // Occupy all three persistent workers so the production import poster
        // remains queued while ownership rotates.
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let release_rx = std::sync::Arc::new(std::sync::Mutex::new(release_rx));
        for index in 0..3 {
            let entered = entered_tx.clone();
            let release = std::sync::Arc::clone(&release_rx);
            assert_eq!(
                scheduler.schedule(
                    epoch,
                    prewarm::PrewarmKind::PreviewPoster,
                    format!("block-import-{index}"),
                    false,
                    move |_| {
                        entered.send(()).unwrap();
                        release.lock().unwrap().recv().unwrap();
                    },
                ),
                prewarm::PrewarmResult::Queued
            );
        }
        for _ in 0..3 {
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap();
        }

        let entry = import_one(&core, &engine, &source).unwrap().unwrap();
        let target = poster_path_for(engine.cache_root(), &cache_key_for(&source).unwrap());
        let first = schedule_import_poster(&core, &engine, &scheduler, &entry, &source);
        let duplicate = schedule_import_poster(&core, &engine, &scheduler, &entry, &source);
        assert_eq!(first.result, prewarm::PrewarmResult::Queued);
        assert_eq!(duplicate.result, prewarm::PrewarmResult::Duplicate);
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::json!({"mediaRef": entry.id, "result": "queued"})
        );

        scheduler.begin_project_transition().unwrap();
        scheduler.activate_project(epoch + 1);
        for _ in 0..3 {
            release_tx.send(()).unwrap();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while scheduler.in_flight_count() != 0 && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(scheduler.in_flight_count(), 0);
        assert!(!target.exists(), "stale queued import poster was published");
    }

    #[test]
    fn large_folder_import_schedules_a_poster_for_every_committed_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("batch");
        fs::create_dir(&root).unwrap();
        // More files than the bounded prewarm queue (64) so the tail of the
        // batch exercises the Busy re-pass; tiny PNGs keep the import and the
        // poster decode fast.
        let file_count = 72;
        let mut files = Vec::with_capacity(file_count);
        for index in 0..file_count {
            let path = root.join(format!("still-{index:03}.png"));
            image::RgbaImage::from_pixel(32, 24, image::Rgba([index as u8, 20, 30, 255]))
                .save(&path)
                .unwrap();
            files.push(path);
        }
        let core = AppCore::new();
        let bundle = tmp.path().join("batch.opentake");
        core.save_project(Some(bundle)).unwrap();
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.project_revision().project_epoch);

        let dto = import_folder_impl(
            &core,
            &engine,
            &scheduler,
            root.to_string_lossy().into_owned(),
            Some(false),
        )
        .expect("folder import succeeds");
        assert_eq!(dto.items.len(), file_count);
        assert!(
            dto.prewarm
                .iter()
                .all(|r| r.result != prewarm::PrewarmResult::Busy),
            "the bounded queue must not permanently drop batch posters: {:?}",
            dto.prewarm
        );

        // Every poster must land on disk; the re-pass + drain wait converge
        // well inside the deadline for these tiny images.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        for path in &files {
            let target = poster_path_for(engine.cache_root(), &cache_key_for(path).unwrap());
            while !target.is_file() && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            assert!(target.is_file(), "poster missing for {}", path.display());
        }
    }

    #[test]
    fn completed_project_swap_rejects_same_id_from_old_source() {
        let tmp = tempfile::tempdir().unwrap();
        let old_source = tmp.path().join("old.png");
        let new_source = tmp.path().join("new.png");
        image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]))
            .save(&old_source)
            .unwrap();
        image::RgbaImage::from_pixel(16, 16, image::Rgba([0, 0, 255, 255]))
            .save(&new_source)
            .unwrap();
        let engine = engine_for(tmp.path());

        let core = AppCore::new();
        let old_entry = import_one(&core, &engine, &old_source).unwrap().unwrap();
        let old_epoch = core.project_revision().project_epoch;
        let scheduler = prewarm::PrewarmScheduler::new(old_epoch);

        // A separate project has its own id generator, so its first persisted
        // asset legitimately reuses the old project's id with another source.
        let replacement = AppCore::new();
        let new_entry = import_one(&replacement, &engine, &new_source)
            .unwrap()
            .unwrap();
        assert_eq!(new_entry.id, old_entry.id);
        assert_ne!(new_entry.source, old_entry.source);
        let bundle = tmp.path().join("replacement.opentake");
        replacement.save_project(Some(bundle.clone())).unwrap();

        scheduler.begin_project_transition().unwrap();
        let snapshot = core.open_project(bundle).unwrap();
        scheduler.activate_project(snapshot.project_epoch);
        let old_target = poster_path_for(engine.cache_root(), &cache_key_for(&old_source).unwrap());

        let admission = schedule_import_poster(&core, &engine, &scheduler, &old_entry, &old_source);
        assert_eq!(admission.result, prewarm::PrewarmResult::StaleProject);
        assert!(!old_target.exists());
    }

    #[test]
    fn thumbnail_dto_serializes_camel_case() {
        let dto = ThumbnailDto {
            media_ref: "m".into(),
            kind: ClipType::Video,
            thumbnail_path: Some("/cache/poster.png".into()),
            sprite_path: Some("/cache/sprite.jpg".into()),
            tile_width: Some(120),
            tile_height: Some(68),
            columns: Some(3),
            times: vec![0.0, 1.0],
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"mediaRef\":\"m\""));
        assert!(json.contains("\"type\":\"video\""));
        assert!(json.contains("\"thumbnailPath\":\"/cache/poster.png\""));
        assert!(json.contains("\"spritePath\":\"/cache/sprite.jpg\""));
        assert!(json.contains("\"tileWidth\":120"));
        assert!(json.contains("\"tileHeight\":68"));
    }

    #[test]
    fn import_folder_recursive_mirrors_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Trip");
        fs::create_dir(&root).unwrap();
        touch(&root.join("a.mp4"));
        let day1 = root.join("Day1");
        fs::create_dir(&day1).unwrap();
        touch(&day1.join("b.mov"));
        touch(&day1.join("note.txt")); // unsupported → skipped
        fs::create_dir(root.join("Empty")).unwrap(); // empty subfolder still mirrors

        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("MirrorTree.opentake")))
            .expect("save project");
        let engine = engine_for(tmp.path());
        let mut skipped = Vec::new();
        mirror_dir(&core, &engine, &root, None, &mut skipped).unwrap();

        let m = core.media();
        // Folders: Trip (root) + Day1 + Empty, nested under Trip.
        assert_eq!(m.folders.len(), 3, "{:?}", m.folders);
        let trip = m.folders.iter().find(|f| f.name == "Trip").unwrap();
        let day1f = m.folders.iter().find(|f| f.name == "Day1").unwrap();
        let empty = m.folders.iter().find(|f| f.name == "Empty").unwrap();
        assert!(trip.parent_folder_id.is_none());
        assert_eq!(day1f.parent_folder_id.as_deref(), Some(trip.id.as_str()));
        assert_eq!(empty.parent_folder_id.as_deref(), Some(trip.id.as_str()));

        // Entries: a.mp4 in Trip, b.mov in Day1; the .txt was skipped.
        assert_eq!(m.entries.len(), 2, "{:?}", m.entries);
        let a = m.entries.iter().find(|e| e.name == "a").unwrap();
        let b = m.entries.iter().find(|e| e.name == "b").unwrap();
        assert_eq!(a.folder_id.as_deref(), Some(trip.id.as_str()));
        assert_eq!(b.folder_id.as_deref(), Some(day1f.id.as_str()));
        // The unsupported note.txt is reported skipped, not dropped silently.
        assert_eq!(skipped, vec!["note.txt"]);
    }

    #[test]
    fn cancelled_recursive_mirror_before_commit_changes_neither_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("CancelledTrip");
        fs::create_dir(&root).unwrap();
        touch(&root.join("a.mp4"));
        let nested = root.join("Day1");
        fs::create_dir(&nested).unwrap();
        touch(&nested.join("b.mov"));

        let bundle = tmp.path().join("CancelledMirror.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone()))
            .expect("save project");
        let engine = engine_for(tmp.path());
        let before_live = core.media();
        let manifest_path = bundle.join("media.json");
        let before_disk = fs::read(&manifest_path).expect("read persisted manifest");
        let cancel = opentake_media::MediaCancelToken::new();
        let mut skipped = Vec::new();

        let result = mirror_dir_cancellable_with_hook(
            &core,
            &engine,
            &root,
            None,
            &mut skipped,
            &cancel,
            || cancel.cancel(),
        );

        let error = result.expect_err("cancelled mirror must not commit its prepared batch");
        assert!(error.to_string().contains("cancel"), "{error}");
        assert_eq!(core.media(), before_live, "live manifest changed");
        assert_eq!(
            fs::read(&manifest_path).expect("reread persisted manifest"),
            before_disk,
            "persisted media.json changed"
        );
        let reopened = AppCore::new();
        reopened.open_project(bundle).expect("reopen project");
        assert_eq!(reopened.media(), before_live, "reopened manifest changed");
    }

    #[test]
    fn directory_import_beta_limits_are_fixed() {
        assert_eq!(DIRECTORY_IMPORT_MAX_DEPTH, 32);
        assert_eq!(DIRECTORY_IMPORT_MAX_ENTRIES, 10_000);
        assert_eq!(DIRECTORY_IMPORT_MAX_FILES, 5_000);
        assert_eq!(DIRECTORY_IMPORT_MAX_PLAN_OPERATIONS, 7_500);
        assert_eq!(
            DIRECTORY_IMPORT_MAX_AGGREGATE_BYTES,
            100 * 1024 * 1024 * 1024
        );
    }

    #[cfg(unix)]
    #[test]
    fn recursive_mirror_rejects_self_loop_symlink_without_publication() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("LoopRoot");
        fs::create_dir(&root).unwrap();
        symlink(".", root.join("self")).expect("create self-loop symlink");
        let bundle = tmp.path().join("LoopMirror.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let before = core.media();
        let engine = engine_for(tmp.path());
        let mut skipped = Vec::new();

        let error = mirror_dir(&core, &engine, &root, None, &mut skipped)
            .expect_err("self-loop symlink must fail closed");

        assert!(error.to_string().contains("symlink_or_reparse"), "{error}");
        assert_eq!(core.media(), before);
        assert!(skipped.is_empty());
        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(reopened.media(), before);
    }

    #[cfg(unix)]
    #[test]
    fn recursive_mirror_rejects_external_directory_symlink_and_preserves_canary() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Selected");
        let outside = tmp.path().join("Outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let canary = outside.join("canary.mp4");
        fs::write(&canary, b"outside-canary").unwrap();
        symlink(&outside, root.join("escape")).expect("create external directory symlink");
        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("EscapeMirror.opentake")))
            .unwrap();
        let before = core.media();
        let engine = engine_for(tmp.path());
        let mut skipped = Vec::new();

        let error = mirror_dir(&core, &engine, &root, None, &mut skipped)
            .expect_err("external symlink must fail closed");

        assert!(error.to_string().contains("symlink_or_reparse"), "{error}");
        assert_eq!(fs::read(canary).unwrap(), b"outside-canary");
        assert_eq!(core.media(), before);
        assert!(skipped.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn recursive_mirror_rejects_fifo_without_blocking_or_publication() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("SpecialRoot");
        fs::create_dir(&root).unwrap();
        let fifo = root.join("named-pipe.mp4");
        let fifo_name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: `fifo_name` is a valid NUL-terminated pathname and mode is ordinary rw-------.
        let status = unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) };
        assert_eq!(
            status,
            0,
            "mkfifo failed: {}",
            std::io::Error::last_os_error()
        );
        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("SpecialMirror.opentake")))
            .unwrap();
        let before = core.media();
        let engine = engine_for(tmp.path());
        let mut skipped = Vec::new();

        let error = mirror_dir(&core, &engine, &root, None, &mut skipped)
            .expect_err("FIFO must be rejected from directory import");

        assert!(
            error.to_string().contains("unsupported_entry_type"),
            "{error}"
        );
        assert_eq!(core.media(), before);
        assert!(skipped.is_empty());
    }

    #[test]
    fn recursive_mirror_rejects_tree_deeper_than_beta_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("DeepRoot");
        fs::create_dir(&root).unwrap();
        let mut cursor = root.clone();
        for depth in 0..=DIRECTORY_IMPORT_MAX_DEPTH {
            cursor = cursor.join(format!("d{depth}"));
            fs::create_dir(&cursor).unwrap();
        }
        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("DeepMirror.opentake")))
            .unwrap();
        let before = core.media();
        let engine = engine_for(tmp.path());
        let mut skipped = Vec::new();

        let error = mirror_dir(&core, &engine, &root, None, &mut skipped)
            .expect_err("over-depth tree must fail closed");

        assert!(
            error.to_string().contains("depth_limit_exceeded"),
            "{error}"
        );
        assert_eq!(core.media(), before);
        assert!(skipped.is_empty());
    }

    #[test]
    fn recursive_mirror_enforces_entry_file_operation_and_aggregate_byte_limits() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = engine_for(tmp.path());

        let entry_root = tmp.path().join("EntryLimit");
        fs::create_dir(&entry_root).unwrap();
        for name in ["a.txt", "b.txt", "c.txt"] {
            touch(&entry_root.join(name));
        }
        let entry_core = AppCore::new();
        entry_core
            .save_project(Some(tmp.path().join("EntryLimit.opentake")))
            .unwrap();
        let entry_before = entry_core.media();
        let entry_cancel = opentake_media::MediaCancelToken::new();
        let mut entry_skipped = Vec::new();
        let mut noop = |_| {};
        let error = mirror_dir_cancellable_with_hooks(
            &entry_core,
            &engine,
            &entry_root,
            None,
            &mut entry_skipped,
            &entry_cancel,
            DirectoryImportLimits {
                max_entries: 2,
                ..DIRECTORY_IMPORT_LIMITS
            },
            &mut noop,
            || {},
        )
        .expect_err("entry limit must fail closed");
        assert!(
            error.to_string().contains("entries_limit_exceeded"),
            "{error}"
        );
        assert_eq!(entry_core.media(), entry_before);
        assert!(entry_skipped.is_empty());

        let file_root = tmp.path().join("FileLimit");
        fs::create_dir(&file_root).unwrap();
        touch(&file_root.join("a.txt"));
        touch(&file_root.join("b.txt"));
        let file_core = AppCore::new();
        file_core
            .save_project(Some(tmp.path().join("FileLimit.opentake")))
            .unwrap();
        let file_before = file_core.media();
        let file_cancel = opentake_media::MediaCancelToken::new();
        let mut file_skipped = Vec::new();
        let mut noop = |_| {};
        let error = mirror_dir_cancellable_with_hooks(
            &file_core,
            &engine,
            &file_root,
            None,
            &mut file_skipped,
            &file_cancel,
            DirectoryImportLimits {
                max_files: 1,
                ..DIRECTORY_IMPORT_LIMITS
            },
            &mut noop,
            || {},
        )
        .expect_err("file limit must fail closed");
        assert!(
            error.to_string().contains("files_limit_exceeded"),
            "{error}"
        );
        assert_eq!(file_core.media(), file_before);
        assert!(file_skipped.is_empty());

        let operation_root = tmp.path().join("OperationLimit");
        fs::create_dir(&operation_root).unwrap();
        touch(&operation_root.join("supported.mp4"));
        let operation_core = AppCore::new();
        operation_core
            .save_project(Some(tmp.path().join("OperationLimit.opentake")))
            .unwrap();
        let operation_before = operation_core.media();
        let operation_cancel = opentake_media::MediaCancelToken::new();
        let mut operation_skipped = Vec::new();
        let mut noop = |_| {};
        let error = mirror_dir_cancellable_with_hooks(
            &operation_core,
            &engine,
            &operation_root,
            None,
            &mut operation_skipped,
            &operation_cancel,
            DirectoryImportLimits {
                max_plan_operations: 1,
                ..DIRECTORY_IMPORT_LIMITS
            },
            &mut noop,
            || {},
        )
        .expect_err("planned-operation limit must fail closed");
        assert!(
            error
                .to_string()
                .contains("planned_operations_limit_exceeded"),
            "{error}"
        );
        assert_eq!(operation_core.media(), operation_before);
        assert!(operation_skipped.is_empty());

        let byte_root = tmp.path().join("ByteLimit");
        fs::create_dir(&byte_root).unwrap();
        fs::write(byte_root.join("large.mp4"), b"123456789").unwrap();
        let byte_core = AppCore::new();
        byte_core
            .save_project(Some(tmp.path().join("ByteLimit.opentake")))
            .unwrap();
        let byte_before = byte_core.media();
        let byte_cancel = opentake_media::MediaCancelToken::new();
        let mut byte_skipped = Vec::new();
        let mut noop = |_| {};
        let error = mirror_dir_cancellable_with_hooks(
            &byte_core,
            &engine,
            &byte_root,
            None,
            &mut byte_skipped,
            &byte_cancel,
            DirectoryImportLimits {
                max_aggregate_bytes: 8,
                ..DIRECTORY_IMPORT_LIMITS
            },
            &mut noop,
            || {},
        )
        .expect_err("aggregate byte limit must fail closed");
        assert!(
            error.to_string().contains("aggregate_bytes_limit_exceeded"),
            "{error}"
        );
        assert_eq!(byte_core.media(), byte_before);
        assert!(byte_skipped.is_empty());
    }

    #[test]
    fn cancellation_mid_directory_plan_publishes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("MidPlanCancel");
        fs::create_dir(&root).unwrap();
        touch(&root.join("a.mp4"));
        touch(&root.join("b.mp4"));
        let bundle = tmp.path().join("MidPlanCancel.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let before_live = core.media();
        let manifest_path = bundle.join("media.json");
        let before_disk = fs::read(&manifest_path).unwrap();
        let engine = engine_for(tmp.path());
        let cancel = opentake_media::MediaCancelToken::new();
        let mut skipped = Vec::new();
        let mut reached = 0;
        let mut cancel_during_plan = |checkpoint| {
            reached = checkpoint;
            if checkpoint == 4 {
                cancel.cancel();
            }
        };

        let error = mirror_dir_cancellable_with_hooks(
            &core,
            &engine,
            &root,
            None,
            &mut skipped,
            &cancel,
            DIRECTORY_IMPORT_LIMITS,
            &mut cancel_during_plan,
            || {},
        )
        .expect_err("mid-plan cancellation must fail closed");

        assert!(error.to_string().contains("cancel"), "{error}");
        assert!(reached >= 4);
        assert!(skipped.is_empty());
        assert_eq!(core.media(), before_live);
        assert_eq!(fs::read(manifest_path).unwrap(), before_disk);
    }

    #[cfg(unix)]
    #[test]
    fn recursive_mirror_root_swap_before_commit_is_rejected_atomically() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("SwapRoot");
        let moved = tmp.path().join("SwapRootMoved");
        let outside = tmp.path().join("SwapOutside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        touch(&root.join("inside.mp4"));
        let canary = outside.join("canary.mp4");
        fs::write(&canary, b"outside-swap-canary").unwrap();
        let bundle = tmp.path().join("SwapRoot.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let before_live = core.media();
        let before_disk = fs::read(bundle.join("media.json")).unwrap();
        let engine = engine_for(tmp.path());
        let cancel = opentake_media::MediaCancelToken::new();
        let mut skipped = Vec::new();

        let error = mirror_dir_cancellable_with_hook(
            &core,
            &engine,
            &root,
            None,
            &mut skipped,
            &cancel,
            || {
                fs::rename(&root, &moved).unwrap();
                symlink(&outside, &root).unwrap();
            },
        )
        .expect_err("root namespace swap must fail before commit");

        assert!(
            error.to_string().contains("symlink_or_reparse")
                || error.to_string().contains("namespace_changed"),
            "{error}"
        );
        assert_eq!(core.media(), before_live);
        assert_eq!(fs::read(bundle.join("media.json")).unwrap(), before_disk);
        assert_eq!(fs::read(canary).unwrap(), b"outside-swap-canary");
        assert!(skipped.is_empty());
    }

    #[test]
    fn recursive_mirror_normal_nested_tree_preserves_exact_sources_and_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("ExactTree");
        let nested = root.join("Nested");
        fs::create_dir_all(&nested).unwrap();
        let first = root.join("first.mp4");
        let second = nested.join("second.wav");
        fs::write(&first, b"first-exact-bytes").unwrap();
        fs::write(&second, b"second-exact-bytes").unwrap();
        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("ExactTree.opentake")))
            .unwrap();
        let engine = engine_for(tmp.path());
        let mut skipped = Vec::new();

        mirror_dir(&core, &engine, &root, None, &mut skipped).unwrap();

        let manifest = core.media();
        assert_eq!(manifest.entries.len(), 2);
        let imported_paths = manifest
            .entries
            .iter()
            .map(|entry| match &entry.source {
                MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
                MediaSource::Project { .. } => panic!("directory import must retain exact sources"),
            })
            .collect::<HashSet<_>>();
        assert_eq!(
            imported_paths,
            HashSet::from([first.clone(), second.clone()])
        );
        assert_eq!(fs::read(first).unwrap(), b"first-exact-bytes");
        assert_eq!(fs::read(second).unwrap(), b"second-exact-bytes");
        assert!(skipped.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn recursive_mirror_rejects_windows_directory_junction() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Selected");
        let outside = tmp.path().join("Outside");
        let junction = root.join("junction");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let canary = outside.join("canary.mp4");
        fs::write(&canary, b"junction-canary").unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .status()
            .expect("create junction");
        assert!(
            status.success(),
            "mklink /J must create the junction fixture"
        );
        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("JunctionMirror.opentake")))
            .unwrap();
        let before = core.media();
        let engine = engine_for(tmp.path());
        let mut skipped = Vec::new();

        let error = mirror_dir(&core, &engine, &root, None, &mut skipped)
            .expect_err("junction must fail closed");

        assert!(error.to_string().contains("symlink_or_reparse"), "{error}");
        assert_eq!(fs::read(canary).unwrap(), b"junction-canary");
        assert_eq!(core.media(), before);
        assert!(skipped.is_empty());
    }

    #[test]
    fn media_list_dto_projects_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Lib");
        fs::create_dir(&root).unwrap();
        touch(&root.join("x.png"));
        let core = AppCore::new();
        core.save_project(Some(tmp.path().join("MirrorDto.opentake")))
            .expect("save project");
        let engine = engine_for(tmp.path());
        let mut skipped = Vec::new();
        mirror_dir(&core, &engine, &root, None, &mut skipped).unwrap();

        let dto = MediaListDto::from_core(&core, None);
        assert_eq!(dto.folders.len(), 1);
        assert_eq!(dto.folders[0].name, "Lib");
        assert_eq!(dto.items.len(), 1);
        assert_eq!(
            dto.items[0].folder_id.as_deref(),
            Some(dto.folders[0].id.as_str())
        );
    }

    #[test]
    fn display_name_uses_stem() {
        assert_eq!(display_name(Path::new("/a/b/My Clip.mp4")), "My Clip");
        assert_eq!(display_name(Path::new("/a/b/noext")), "noext");
    }

    #[test]
    fn list_top_level_keeps_media_reports_unsupported_and_ignores_subdirs_and_hidden() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("a.mp4"));
        touch(&root.join("b.png"));
        touch(&root.join("c.txt")); // unsupported → reported skipped
        touch(&root.join("readme.md")); // unsupported → reported skipped
        touch(&root.join(".hidden.mp4")); // hidden → ignored entirely (not skipped)
        fs::create_dir(root.join("sub")).unwrap();
        touch(&root.join("sub").join("d.mov")); // subdir contents ignored in flat mode

        let (files, skipped) = list_top_level(root);
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.mp4", "b.png"]);
        // Unsupported top-level files are reported (sorted, case-insensitive);
        // the hidden dotfile and the subdir file are NOT reported.
        assert_eq!(skipped, vec!["c.txt", "readme.md"]);
    }

    #[test]
    fn list_dir_partitions_files_subdirs_and_skipped_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("z.mp4"));
        touch(&root.join("A.mov"));
        touch(&root.join("junk.bin")); // unsupported
        fs::create_dir(root.join("sub")).unwrap();

        let (files, subdirs, skipped) = list_dir(root);
        let fnames: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        // Files sorted case-insensitively: A.mov before z.mp4.
        assert_eq!(fnames, vec!["A.mov", "z.mp4"]);
        assert_eq!(subdirs.len(), 1);
        assert_eq!(skipped, vec!["junk.bin"]);
    }

    #[test]
    fn import_media_imports_supported_and_skips_others() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let good = root.join("clip.mp4");
        let bad = root.join("doc.txt");
        touch(&good);
        touch(&bad);

        let core = AppCore::new();
        let media = MediaState::new(engine_for(root));

        // Drive the import logic directly (the #[tauri::command] wrapper only
        // adds State extraction). Probing a non-media file yields defaults.
        for p in [&good, &bad] {
            if p.is_file() {
                let _ = import_one(&core, media.engine(), p);
            }
        }

        let list = MediaListDto::from_core(&core, None);
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].kind, ClipType::Video);
        assert_eq!(list.items[0].name, "clip");
        assert_eq!(list.items[0].path.as_deref(), Some(good.to_str().unwrap()));
    }

    #[test]
    fn compatibility_favorite_marker_projects_into_the_media_dto() {
        // The per-project marker remains a compatibility mirror for old project
        // files and card state. The Mine grid itself reads the global library.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let clip = root.join("clip.mp4");
        touch(&clip);
        let core = AppCore::new();
        let media = MediaState::new(engine_for(root));
        let entry = import_one(&core, media.engine(), &clip).unwrap().unwrap();

        // A freshly imported asset is not favorited.
        let before = MediaListDto::from_core(&core, None);
        assert_eq!(before.items.len(), 1);
        assert!(!before.items[0].favorite);

        // Favoriting it surfaces in the DTO.
        assert_eq!(
            core.set_media_favorite(std::slice::from_ref(&entry.id), true)
                .unwrap(),
            1
        );
        assert!(MediaListDto::from_core(&core, None).items[0].favorite);

        // Unknown ids never create phantom favorites.
        assert_eq!(core.set_media_favorite(&["ghost".into()], true).unwrap(), 0);

        // Unfavoriting flips it back.
        assert_eq!(
            core.set_media_favorite(std::slice::from_ref(&entry.id), false)
                .unwrap(),
            1
        );
        assert!(!MediaListDto::from_core(&core, None).items[0].favorite);
    }

    #[test]
    fn single_clip_export_rebases_clip_and_subsets_manifest() {
        use opentake_domain::{Clip, Track};

        fn entry_for(id: &str) -> MediaManifestEntry {
            MediaManifestEntry {
                id: id.into(),
                name: id.into(),
                kind: ClipType::Video,
                source: MediaSource::External {
                    absolute_path: format!("/abs/{id}.mp4"),
                },
                duration: 2.0,
                generation_input: None,
                source_width: Some(640),
                source_height: Some(480),
                source_fps: Some(30.0),
                has_audio: Some(true),
                color: None,
                proxy: None,
                folder_id: None,
                cached_remote_url: None,
                cached_remote_url_expires_at: None,
            }
        }

        // Multi-track, multi-clip timeline; save clip "c2" off a hidden track.
        let mut tl = Timeline::new();
        let mut t0 = Track::new("t0", ClipType::Video);
        t0.clips.push(Clip::new("c1", "mediaA", 0, 30));
        let mut t1 = Track::new("t1", ClipType::Video);
        t1.hidden = true;
        t1.clips.push(Clip::new("c2", "mediaB", 120, 45));
        tl.tracks.push(t0);
        tl.tracks.push(t1);

        let mut manifest = MediaManifest::new();
        manifest.entries.push(entry_for("mediaA"));
        manifest.entries.push(entry_for("mediaB"));

        let (single, subset, kind) = build_single_clip_export(&tl, &manifest, "c2").unwrap();

        assert_eq!(kind, ClipType::Video);
        // One track, one clip, re-based to frame 0, forced visible + unmuted.
        assert_eq!(single.tracks.len(), 1);
        assert_eq!(single.tracks[0].clips.len(), 1);
        assert_eq!(single.tracks[0].clips[0].id, "c2");
        assert_eq!(single.tracks[0].clips[0].start_frame, 0);
        assert_eq!(single.tracks[0].clips[0].duration_frames, 45); // preserved
        assert!(!single.tracks[0].hidden);
        assert!(!single.tracks[0].muted);
        // Timeline-level fields are preserved by clone-then-replace.
        assert_eq!(single.fps, tl.fps);
        assert_eq!(single.width, tl.width);
        // Manifest subset carries only the clip's source.
        assert_eq!(subset.entries.len(), 1);
        assert_eq!(subset.entries[0].id, "mediaB");
        assert!(subset.favorites.is_empty());

        // Unknown clip id is an error, not a panic.
        assert!(build_single_clip_export(&tl, &manifest, "nope").is_err());
    }

    #[test]
    fn media_list_dto_serializes_skipped_camel_case() {
        // Listing surfaces carry an empty `skipped`; the field name stays
        // `skipped` in JSON (single word, so camelCase == snake_case here) and is
        // always present so the front end can read it unconditionally.
        let empty = MediaListDto {
            items: vec![],
            folders: vec![],
            skipped: vec![],
            prewarm: vec![],
        };
        let json = serde_json::to_string(&empty).unwrap();
        assert!(json.contains("\"skipped\":[]"));

        let with_skips = MediaListDto {
            items: vec![],
            folders: vec![],
            skipped: vec!["a.txt".into(), "b.pdf".into()],
            prewarm: vec![],
        };
        let json = serde_json::to_string(&with_skips).unwrap();
        assert!(json.contains("\"skipped\":[\"a.txt\",\"b.pdf\"]"));
    }

    #[test]
    fn from_core_default_skipped_is_empty_and_with_skipped_carries_names() {
        let core = AppCore::new();
        // Non-import surfaces report no skips.
        assert!(MediaListDto::from_core(&core, None).skipped.is_empty());
        // Import surfaces thread the skipped file names through unchanged.
        let dto = MediaListDto::from_core_with_import_results(
            &core,
            None,
            vec!["note.txt".into(), "archive.zip".into()],
            Vec::new(),
        );
        assert_eq!(dto.skipped, vec!["note.txt", "archive.zip"]);
    }

    #[test]
    fn proxy_asset_scope_grants_only_regular_file_and_revoke_denies_it() {
        let app = tauri::test::mock_app();
        let handle = app.handle();
        let temp = tempfile::tempdir().unwrap();
        let proxy = temp.path().join("proxy.mp4");
        fs::write(&proxy, b"proxy").unwrap();

        assert!(!handle.asset_protocol_scope().is_allowed(&proxy));
        grant_proxy_asset_file(handle, &proxy).expect("grant exact proxy file");
        assert!(handle.asset_protocol_scope().is_allowed(&proxy));
        revoke_proxy_asset_file(handle, &proxy);
        assert!(!handle.asset_protocol_scope().is_allowed(&proxy));
    }

    #[test]
    fn update_install_rejects_synchronous_cache_writers() {
        let temp = tempfile::tempdir().unwrap();
        let (core, _bundle, _source, asset_id) = saved_core_with_media(temp.path());
        let admission = crate::updater::InstallAdmissionGate::default();
        let app = tauri::test::mock_app();
        app.manage(core);
        app.manage(MediaState::new_with_admission(
            engine_for(temp.path()),
            admission.clone(),
        ));
        let install = admission.begin_install().expect("install starts");
        let expected = "app update installation is in progress";

        assert_eq!(
            generate_thumbnail(
                app.state::<AppCore>(),
                app.state::<MediaState>(),
                asset_id.clone(),
                None,
                None,
                None,
            )
            .expect_err("thumbnail cache writer must fail closed"),
            expected
        );
        assert_eq!(
            preview_poster(
                app.state::<AppCore>(),
                app.state::<MediaState>(),
                asset_id.clone(),
                None,
            )
            .expect_err("preview-poster cache writer must fail closed"),
            expected
        );
        assert_eq!(
            get_waveform(app.state::<AppCore>(), app.state::<MediaState>(), asset_id,)
                .expect_err("waveform cache writer must fail closed"),
            expected
        );
        drop(install);
    }

    #[test]
    fn update_install_allows_cache_hits_and_nonwriting_thumbnail_requests() {
        let temp = tempfile::tempdir().unwrap();
        let (core, _bundle, source, video_id) = saved_core_with_media(temp.path());
        let audio_source = temp.path().join("audio.wav");
        fs::write(&audio_source, b"audio-placeholder").unwrap();
        let audio_id = core
            .import_media_file(&audio_source, "audio", &ProbedMedia::default())
            .expect("import audio fixture")
            .id;
        core.save_project(None).expect("persist audio fixture");

        let engine = engine_for(temp.path());
        let key = cache_key_for(&source).expect("video cache key");
        let thumbnail_path = timed_poster_path_for(engine.cache_root(), &key, 0.0);
        let preview_path = preview_poster_path_for(engine.cache_root(), &key, 0.0);
        write_png(&thumbnail_path, &RgbaFrame::black(2, 2)).expect("seed thumbnail cache");
        write_png(&preview_path, &RgbaFrame::black(4, 4)).expect("seed preview cache");
        let cached_waveform = vec![0.25, 0.75];
        opentake_media::waveform::store::save_waveform(engine.cache_root(), &key, &cached_waveform)
            .expect("seed waveform cache");

        let admission = crate::updater::InstallAdmissionGate::default();
        let app = tauri::test::mock_app();
        app.manage(core);
        app.manage(MediaState::new_with_admission(engine, admission.clone()));
        let install = admission.begin_install().expect("install starts");

        let thumbnail = generate_thumbnail(
            app.state::<AppCore>(),
            app.state::<MediaState>(),
            video_id.clone(),
            None,
            None,
            Some(false),
        )
        .expect("cached thumbnail is read-only");
        assert_eq!(
            thumbnail.thumbnail_path.as_deref(),
            Some(thumbnail_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            preview_poster(
                app.state::<AppCore>(),
                app.state::<MediaState>(),
                video_id.clone(),
                None,
            )
            .expect("cached preview is read-only")
            .as_deref(),
            Some(preview_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            get_waveform(app.state::<AppCore>(), app.state::<MediaState>(), video_id,)
                .expect("cached waveform is read-only"),
            cached_waveform
        );
        let audio_thumbnail = generate_thumbnail(
            app.state::<AppCore>(),
            app.state::<MediaState>(),
            audio_id,
            None,
            None,
            Some(false),
        )
        .expect("audio has no thumbnail cache write");
        assert_eq!(audio_thumbnail.kind, ClipType::Audio);
        assert_eq!(audio_thumbnail.thumbnail_path, None);
        drop(install);
    }

    #[test]
    fn get_media_does_not_persist_proxy_scope_during_update_install() {
        let temp = tempfile::tempdir().unwrap();
        let (core, _bundle, _source, asset_id) = saved_core_with_media(temp.path());
        let snapshot = core.runtime_snapshot();
        let project_dir = snapshot.project_dir.clone().unwrap();
        let proxy = project_dir.join("media/proxies/proxy.mp4");
        fs::create_dir_all(proxy.parent().unwrap()).unwrap();
        fs::write(&proxy, b"proxy").unwrap();
        core.set_media_proxy_for_project(
            snapshot.project_epoch,
            &project_dir,
            &asset_id,
            Some(MediaProxy {
                relative_path: "media/proxies/proxy.mp4".into(),
                source_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .into(),
                width: 1280,
                height: 720,
            }),
        )
        .unwrap();
        let admission = crate::updater::InstallAdmissionGate::default();
        let app = tauri::test::mock_app();
        app.manage(core);
        app.manage(MediaState::new_with_admission(
            engine_for(temp.path()),
            admission.clone(),
        ));
        let install = admission.begin_install().expect("install starts");

        let catalog = get_media(
            app.handle().clone(),
            app.state::<AppCore>(),
            app.state::<MediaState>(),
        );

        assert_eq!(catalog.items.len(), 1);
        assert_eq!(catalog.items[0].proxy_path, None);
        assert!(!app.handle().asset_protocol_scope().is_allowed(&proxy));
        drop(install);
    }

    #[cfg(unix)]
    #[test]
    fn proxy_asset_scope_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let app = tauri::test::mock_app();
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("outside.mp4");
        let proxy = temp.path().join("proxy.mp4");
        fs::write(&target, b"outside").unwrap();
        symlink(&target, &proxy).unwrap();

        assert_eq!(
            grant_proxy_asset_file(app.handle(), &proxy).unwrap_err(),
            "media_proxy_scope_regular_file_required"
        );
        assert!(!app.handle().asset_protocol_scope().is_allowed(&target));
    }

    #[cfg(unix)]
    #[test]
    fn project_proxy_path_rejects_symlinked_ancestor_directory() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("ProxyPath.opentake");
        let outside = temp.path().join("outside");
        fs::create_dir_all(bundle.join("media")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("proxy.mp4"), b"outside").unwrap();
        symlink(&outside, bundle.join("media/proxies")).unwrap();

        assert!(trusted_project_proxy_path(&bundle, "media/proxies/proxy.mp4").is_none());
    }

    #[test]
    fn remove_proxy_holds_project_identity_through_file_cleanup() {
        use std::sync::mpsc;
        use std::time::Duration;

        let temp = tempfile::tempdir().unwrap();
        let project_a_root = temp.path().join("project-a");
        let project_b_root = temp.path().join("project-b");
        fs::create_dir_all(&project_a_root).unwrap();
        fs::create_dir_all(&project_b_root).unwrap();
        let (core, _bundle_a, _source_a, asset_id) = saved_core_with_media(&project_a_root);
        let (_other, bundle_b, _source_b, _asset_b) = saved_core_with_media(&project_b_root);
        let core = Arc::new(core);
        let snapshot = core.runtime_snapshot();
        let project_dir = snapshot.project_dir.clone().unwrap();
        let proxy_path = project_dir.join("media/proxies/proxy.mp4");
        fs::create_dir_all(proxy_path.parent().unwrap()).unwrap();
        fs::write(&proxy_path, b"proxy").unwrap();
        core.set_media_proxy_for_project(
            snapshot.project_epoch,
            &project_dir,
            &asset_id,
            Some(MediaProxy {
                relative_path: "media/proxies/proxy.mp4".into(),
                source_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .into(),
                width: 1280,
                height: 720,
            }),
        )
        .unwrap();

        let (start_tx, start_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let replacement_core = Arc::clone(&core);
        let replacement = std::thread::spawn(move || {
            start_rx.recv().unwrap();
            replacement_core.open_project(bundle_b).unwrap();
            done_tx.send(()).unwrap();
        });

        remove_media_proxy_impl(&core, &asset_id, |path| {
            fs::remove_file(path).unwrap();
            start_tx.send(()).unwrap();
            assert!(
                done_rx.recv_timeout(Duration::from_millis(100)).is_err(),
                "project replacement must stay blocked until proxy cleanup returns"
            );
        })
        .unwrap();

        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        replacement.join().unwrap();
        assert!(!proxy_path.exists());
    }

    #[test]
    fn get_media_reflects_imported_items() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let core = AppCore::new();
        let engine = engine_for(root);
        let f = root.join("a.png");
        touch(&f);
        import_one(&core, &engine, &f).unwrap();

        let list = MediaListDto::from_core(&core, None);
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].kind, ClipType::Image);
        // The touched file exists → not missing.
        assert!(!list.items[0].missing);
    }

    #[test]
    fn relink_keeps_same_id_and_clears_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let core = AppCore::new();
        let engine = engine_for(root);
        let orig = root.join("clip.mp4");
        touch(&orig);
        let id = import_one(&core, &engine, &orig).unwrap().unwrap().id;

        // Source goes missing → the panel reads it as offline.
        fs::remove_file(&orig).unwrap();
        let list = MediaListDto::from_core(&core, None);
        assert_eq!(list.items.len(), 1);
        assert!(
            list.items[0].missing,
            "a deleted source must read as missing"
        );

        // Relink to a new file of the SAME type — keeps the id, heals in place.
        let moved = root.join("clip-moved.mp4");
        touch(&moved);
        let probe = probe_media(&engine, &moved);
        core.relink_media_file(&id, &moved, &probe).unwrap();

        let list = MediaListDto::from_core(&core, None);
        assert_eq!(list.items.len(), 1, "relink must not mint a new entry");
        assert_eq!(list.items[0].id, id, "same id so existing clips recover");
        assert!(
            !list.items[0].missing,
            "relinked source exists → not missing"
        );
        assert_eq!(list.items[0].path.as_deref(), moved.to_str());
    }

    #[test]
    fn relink_rejects_type_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let core = AppCore::new();
        let engine = engine_for(root);
        let orig = root.join("clip.mp4");
        touch(&orig);
        let id = import_one(&core, &engine, &orig).unwrap().unwrap().id;

        // Relinking a video asset to an audio file is rejected (upstream parity).
        let wrong = root.join("song.mp3");
        touch(&wrong);
        let probe = probe_media(&engine, &wrong);
        assert!(core.relink_media_file(&id, &wrong, &probe).is_err());
        let list = MediaListDto::from_core(&core, None);
        assert_eq!(list.items[0].kind, ClipType::Video, "catalog unchanged");
    }

    #[test]
    fn relink_unknown_id_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        let f = tmp.path().join("x.mp4");
        touch(&f);
        let probe = probe_media(&engine_for(tmp.path()), &f);
        assert!(core.relink_media_file("nope", &f, &probe).is_err());
    }

    // --- extract_audio output-path validation (Issue #39 review #4) ---
    //
    // The command is callable from the WebView with an arbitrary string; these
    // tests lock down the boundary that `validate_extract_output` enforces
    // before any ffmpeg work begins. They run without ffmpeg on PATH.

    #[test]
    fn validate_extract_output_accepts_whitelisted_extensions() {
        // All five extensions accepted by the codec table + the native save
        // dialog filters should parse to an absolute PathBuf.
        for ext in ["m4a", "m4r", "aac", "mp3", "wav"] {
            let output = std::env::temp_dir().join(format!("out.{ext}"));
            let p = validate_extract_output(&output.to_string_lossy())
                .unwrap_or_else(|e| panic!(".{ext}: {e}"));
            assert_eq!(p.extension().unwrap().to_str().unwrap(), ext);
            assert!(p.is_absolute());
        }
    }

    #[test]
    fn validate_extract_output_rejects_relative_path() {
        let err = validate_extract_output("out.m4a").unwrap_err();
        assert!(
            err.contains("absolute"),
            "relative path must be rejected: got {err}"
        );
    }

    #[test]
    fn validate_extract_output_rejects_null_byte() {
        // A null byte would be silently truncated by some OS path APIs,
        // writing the file at an unexpected location.
        let err = validate_extract_output("/tmp/out\0.m4a").unwrap_err();
        assert!(
            err.contains("null"),
            "null byte must be rejected: got {err}"
        );
    }

    #[test]
    fn validate_extract_output_rejects_unknown_extension() {
        let output = std::env::temp_dir().join("out.mp4");
        let err = validate_extract_output(&output.to_string_lossy()).unwrap_err();
        assert!(
            err.contains("unsupported audio extension"),
            "video extension must be rejected: got {err}"
        );
    }

    #[test]
    fn validate_extract_output_rejects_missing_extension() {
        let output = std::env::temp_dir().join("out");
        let err = validate_extract_output(&output.to_string_lossy()).unwrap_err();
        assert!(
            err.contains("no extension"),
            "extensionless path must be rejected: got {err}"
        );
    }

    #[test]
    fn stabilization_analysis_state_cancels_and_releases_single_flight_slot() {
        let state = StabilizationAnalysisState::default();
        let first = state.begin().expect("first analysis reserves the slot");
        assert!(state.begin().is_err(), "a concurrent analysis is rejected");

        assert!(state.cancel(), "the active analysis is cancellable");
        assert!(first.is_cancelled());
        state.finish(&first);
        assert!(!state.cancel(), "finishing clears the active token");

        let second = state.begin().expect("the slot can be reused after finish");
        assert!(!second.is_cancelled());
        state.finish(&second);
    }

    #[test]
    fn denoise_analysis_state_cancels_and_releases_single_flight_slot() {
        let state = DenoiseAnalysisState::default();
        let first = state.begin().expect("first analysis reserves the slot");
        let concurrent = match state.begin() {
            Ok(_) => panic!("a concurrent analysis must be rejected"),
            Err(error) => error,
        };
        assert_eq!(concurrent, "denoise_analysis_busy");

        assert!(state.cancel(), "the active analysis is cancellable");
        assert!(first.is_cancelled());
        state.finish(&first);
        assert!(!state.cancel(), "finishing clears the active token");

        let second = state.begin().expect("the slot can be reused after finish");
        assert!(!second.is_cancelled());
        state.finish(&second);
    }

    #[test]
    fn deferred_inspector_analyses_share_update_install_admission() {
        let admission = crate::updater::InstallAdmissionGate::default();
        let stabilization = StabilizationAnalysisState::new(admission.clone());
        let loudness = LoudnessAnalysisState::new(admission.clone());
        let denoise = DenoiseAnalysisState::new(admission.clone());

        let stabilization_token = stabilization.begin().unwrap();
        let loudness_token = loudness.begin().unwrap();
        let denoise_token = denoise.begin().unwrap();
        assert!(admission.begin_install().is_err());

        stabilization.finish(&stabilization_token);
        loudness.finish(&loudness_token);
        denoise.finish(&denoise_token);
        let install = admission.begin_install().unwrap();
        assert!(stabilization.begin().is_err());
        assert!(loudness.begin().is_err());
        assert!(denoise.begin().is_err());
        drop(install);
    }

    #[test]
    fn project_identity_transition_cancels_every_inspector_analysis() {
        let stabilization = StabilizationAnalysisState::default();
        let loudness = LoudnessAnalysisState::default();
        let denoise = DenoiseAnalysisState::default();
        let stabilization_token = stabilization.begin().expect("stabilization token");
        let loudness_token = loudness.begin().expect("loudness token");
        let denoise_token = denoise.begin().expect("denoise token");

        assert!(cancel_project_bound_analyses(
            &stabilization,
            &loudness,
            &denoise,
        ));
        assert!(stabilization_token.is_cancelled());
        assert!(loudness_token.is_cancelled());
        assert!(denoise_token.is_cancelled());
    }

    #[test]
    fn stem_separation_state_cancels_and_releases_single_flight_slot() {
        let state = StemSeparationState::default();
        let first = state.begin().expect("first separation reserves the slot");
        let concurrent = match state.begin() {
            Ok(_) => panic!("a concurrent separation must be rejected"),
            Err(error) => error,
        };
        assert_eq!(concurrent, "stem_separation_busy");
        assert!(state.cancel());
        assert!(first.is_cancelled());
        state.finish(&first);
        assert!(!state.cancel());
        let second = state.begin().expect("slot is reusable");
        state.finish(&second);
    }

    #[test]
    fn stem_separation_holds_update_admission_until_finish() {
        let admission = crate::updater::InstallAdmissionGate::default();
        let state = StemSeparationState::new(admission.clone());

        let token = state.begin().expect("stem separation starts");
        assert!(
            admission.begin_install().is_err(),
            "install must wait through the final persisted stem import"
        );
        state.finish(&token);

        let install = admission
            .begin_install()
            .expect("finished job releases gate");
        assert!(
            state.begin().is_err(),
            "an installer that wins admission rejects new stem work"
        );
        drop(install);
    }

    #[test]
    fn media_proxy_holds_update_admission_until_finish() {
        let admission = crate::updater::InstallAdmissionGate::default();
        let state = MediaProxyState::new(admission.clone());

        let token = state.begin().expect("proxy transcode starts");
        assert!(
            admission.begin_install().is_err(),
            "install must wait through the final proxy manifest commit"
        );
        state.finish(&token);

        let install = admission
            .begin_install()
            .expect("finished job releases gate");
        assert!(
            state.begin().is_err(),
            "an installer that wins admission rejects new proxy work"
        );
        drop(install);
    }

    #[test]
    fn direct_media_project_writer_is_mutually_exclusive_with_update_install() {
        let admission = crate::updater::InstallAdmissionGate::default();

        let write = begin_direct_media_project_write(&admission).expect("writer starts");
        assert!(
            admission.begin_install().is_err(),
            "a direct writer that starts first must block install"
        );
        drop(write);

        let install = admission.begin_install().expect("install starts");
        assert!(
            begin_direct_media_project_write(&admission).is_err(),
            "install that starts first must reject a stale writer IPC"
        );
        drop(install);
    }
}
