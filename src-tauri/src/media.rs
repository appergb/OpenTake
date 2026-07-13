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

use std::path::{Path, PathBuf};

use image::ImageEncoder;
use serde::Serialize;
use tauri::{AppHandle, State};

use opentake_core::{importable_clip_type, AppCore, CoreError, EditCommand, ProbedMedia};
use opentake_domain::{
    ClipType, GenerationInput, MediaManifest, MediaManifestEntry, MediaSource, Timeline,
};
use opentake_media::{
    cache_key::{file_identity_key, KEY_HEX_LEN},
    decode_frame_at, decode_frames_at,
    thumbnail::{
        save_sprite, sprite::grid_geometry, video_thumbnail_times, ThumbnailCacheMeta, VideoThumb,
        MAX_VIDEO_THUMBNAILS, THUMB_MAX_SIZE, THUMB_TOLERANCE_SECS,
    },
    waveform::store::CACHE_SUBDIR,
    FrameRequest, MediaEngine, RgbaFrame,
};

pub mod prewarm;

/// Managed-state wrapper over the media engine. The engine is read-only here
/// (probe only) and shared across commands; `Send + Sync` so it lives in Tauri
/// state.
pub struct MediaState {
    engine: MediaEngine,
}

impl MediaState {
    /// Wrap an engine for managed state.
    pub fn new(engine: MediaEngine) -> Self {
        MediaState { engine }
    }

    /// The wrapped engine.
    pub fn engine(&self) -> &MediaEngine {
        &self.engine
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
    /// Whether the asset carries audio.
    pub has_audio: bool,
    /// Absolute path to the source file, when resolvable (external assets only
    /// in this phase, which is all importing produces).
    pub path: Option<String>,
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
        let path = match &entry.source {
            MediaSource::External { absolute_path } => Some(absolute_path.clone()),
            // Project-relative assets need the bundle base to resolve; not
            // produced by importing (always external) but handled for safety.
            MediaSource::Project { .. } => None,
        };
        // Missing = we can resolve a local source path and it doesn't exist.
        // An unresolvable (e.g. remote-only) source is not flagged missing.
        let missing = resolved.as_ref().map(|p| !p.exists()).unwrap_or(false);
        let thumbnail = if missing {
            None
        } else {
            resolved.as_deref().and_then(|path| {
                cache_root.and_then(|root| cached_thumbnail_path_for_entry(root, entry, path))
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
            has_audio: entry.has_audio.unwrap_or(false),
            path,
            thumbnail,
            folder_id: entry.folder_id.clone(),
            file_size,
            generation_input: entry.generation_input.clone(),
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

fn source_path_for_entry(core: &AppCore, entry: &MediaManifestEntry) -> Result<PathBuf, String> {
    match &entry.source {
        MediaSource::External { absolute_path } => Ok(PathBuf::from(absolute_path)),
        MediaSource::Project { relative_path } => core
            .project_dir()
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
        let manifest = core.media();
        let project_dir = core.project_dir();
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
    file_identity_key(path, KEY_HEX_LEN)
        .ok_or_else(|| format!("could not build thumbnail cache key for {}", path.display()))
}

fn visual_cache_dir(cache_root: &Path) -> PathBuf {
    cache_root.join(CACHE_SUBDIR)
}

fn sprite_path_for(cache_root: &Path, key: &str) -> PathBuf {
    visual_cache_dir(cache_root).join(format!("{key}.thumbs.jpg"))
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
    if poster_path.exists() {
        let (width, height) = image::image_dimensions(&poster_path)
            .map_err(|e| format!("thumbnail dimensions: {e}"))?;
        return Ok((poster_path, width, height, target));
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

fn read_cached_sprite_meta(cache_root: &Path, key: &str) -> Option<ThumbnailCacheMeta> {
    let sprite_path = sprite_path_for(cache_root, key);
    let meta_path = sprite_meta_path_for(cache_root, key);
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

/// Probe `path` via the engine, mapping ffprobe facts to [`ProbedMedia`]. Probe
/// failures (no ffprobe, unreadable file) degrade to defaults so a single bad
/// file never sinks a batch import.
fn probe_media(engine: &MediaEngine, path: &Path) -> ProbedMedia {
    match engine.probe(path) {
        Ok(p) => ProbedMedia {
            duration_secs: p.duration_secs,
            width: p.width.map(|w| w as i32),
            height: p.height.map(|h| h as i32),
            fps: p.fps,
            has_audio: p.has_audio,
        },
        Err(_) => ProbedMedia::default(),
    }
}

/// Display name for an imported file: its stem, or the full file name when there
/// is no stem (mirrors upstream `url.deletingPathExtension().lastPathComponent`).
fn display_name(path: &Path) -> String {
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

pub(crate) fn import_saved_media(
    core: &AppCore,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    expected_project_epoch: u64,
    expected_project_dir: &Path,
    path: &Path,
) -> Result<MediaListDto, String> {
    let current = core.runtime_snapshot();
    if current.project_epoch != expected_project_epoch
        || current.project_dir.as_deref() != Some(expected_project_dir)
    {
        return Err("project changed while saving media".to_string());
    }
    let entry = import_one(core, engine, path)
        .map_err(|error| error.to_string())?
        .ok_or("failed to import saved media")?;
    let project_dir = core
        .project_dir()
        .ok_or("project closed while importing saved media")?;
    match &entry.source {
        MediaSource::Project { relative_path }
            if project_dir.join(relative_path) == path
                && Path::new(relative_path)
                    .components()
                    .next()
                    .is_some_and(|component| component.as_os_str() == "media") => {}
        _ => return Err("saved media must be imported as a project-relative source".to_string()),
    }
    let result = schedule_import_poster(core, engine, prewarm, &entry, path);
    Ok(MediaListDto::from_core_with_import_results(
        core,
        Some(engine.cache_root()),
        Vec::new(),
        vec![result],
    ))
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
    path: String,
    recursive: Option<bool>,
) -> Result<MediaListDto, String> {
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
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let mut skipped = Vec::new();
    let mut prewarm_results = Vec::new();
    if recursive.unwrap_or(false) {
        mirror_dir_scheduled(
            core,
            engine,
            prewarm,
            &root,
            None,
            &mut skipped,
            &mut prewarm_results,
        )
        .map_err(|e| e.to_string())?;
    } else {
        let (files, skipped_files) = list_top_level(&root);
        for file in &files {
            if let Some(entry) = import_one(core, engine, file).map_err(|e| e.to_string())? {
                prewarm_results.push(schedule_import_poster(core, engine, prewarm, &entry, file));
            }
        }
        skipped = skipped_files;
    }
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
pub(crate) fn mirror_dir(
    core: &AppCore,
    engine: &MediaEngine,
    dir: &Path,
    parent_folder_id: Option<String>,
    skipped: &mut Vec<String>,
) -> Result<(), CoreError> {
    let mut unused_results = Vec::new();
    mirror_dir_impl(
        core,
        engine,
        None,
        dir,
        parent_folder_id,
        skipped,
        &mut unused_results,
    )
}

fn mirror_dir_scheduled(
    core: &AppCore,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    dir: &Path,
    parent_folder_id: Option<String>,
    skipped: &mut Vec<String>,
    prewarm_results: &mut Vec<ImportPrewarmDto>,
) -> Result<(), CoreError> {
    mirror_dir_impl(
        core,
        engine,
        Some(prewarm),
        dir,
        parent_folder_id,
        skipped,
        prewarm_results,
    )
}

fn mirror_dir_impl(
    core: &AppCore,
    engine: &MediaEngine,
    prewarm: Option<&prewarm::PrewarmScheduler>,
    dir: &Path,
    parent_folder_id: Option<String>,
    skipped: &mut Vec<String>,
    prewarm_results: &mut Vec<ImportPrewarmDto>,
) -> Result<(), CoreError> {
    let folder_id = create_folder(core, &dir_name(dir), parent_folder_id)?;

    // Partition this directory's visible entries into media files + subdirs
    // (both case-insensitive name order) plus the names of unsupported files.
    let (files, subdirs, mut dir_skipped) = list_dir(dir);
    skipped.append(&mut dir_skipped);

    let mut imported_ids = Vec::new();
    for file in &files {
        if let Some(entry) = import_one(core, engine, file)? {
            if let Some(prewarm) = prewarm {
                prewarm_results.push(schedule_import_poster(core, engine, prewarm, &entry, file));
            }
            imported_ids.push(entry.id);
        }
    }
    if !imported_ids.is_empty() {
        core.apply(EditCommand::MoveToFolder {
            asset_ids: imported_ids,
            folder_id: Some(folder_id.clone()),
        })?;
    }

    for sub in subdirs {
        mirror_dir_impl(
            core,
            engine,
            prewarm,
            &sub,
            Some(folder_id.clone()),
            skipped,
            prewarm_results,
        )?;
    }
    Ok(())
}

/// Create a library folder, returning its new id or propagating the rejection.
fn create_folder(
    core: &AppCore,
    name: &str,
    parent_folder_id: Option<String>,
) -> Result<String, CoreError> {
    core.apply(EditCommand::CreateFolder {
        name: name.to_string(),
        parent_folder_id,
    })?
    .affected_clip_ids
    .into_iter()
    .next()
    .ok_or_else(|| CoreError::Media("folder creation returned no id".into()))
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
fn list_top_level(dir: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let (files, _subdirs, skipped) = list_dir(dir);
    (files, skipped)
}

/// `import_media`: import an explicit list of file paths, returning the updated
/// catalog. Unsupported or unreadable paths are skipped (not fatal); the returned
/// list reflects whatever imported successfully and carries the names of skipped
/// unsupported files in `skipped` so the front end can toast them (upstream
/// `mediaPanelToast`) instead of dropping them silently.
#[tauri::command]
pub fn import_media(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    prewarm: State<'_, prewarm::PrewarmScheduler>,
    paths: Vec<String>,
) -> Result<MediaListDto, String> {
    import_media_impl(&core, media.engine(), &prewarm, paths)
}

fn import_media_impl(
    core: &AppCore,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    paths: Vec<String>,
) -> Result<MediaListDto, String> {
    core.ensure_project_mutable().map_err(|e| e.to_string())?;
    let mut skipped = Vec::new();
    let mut prewarm_results = Vec::new();
    for p in &paths {
        let path = PathBuf::from(p);
        if !path.is_file() {
            continue;
        }
        // Only an unsupported *type* is a user-visible "skip"; a supported file
        // that fails to import (unreadable etc.) is not reported here (matches the
        // pre-existing best-effort behavior and upstream, which only toasts the
        // unsupported-type case).
        if importable_clip_type(&path).is_none() {
            skipped.push(display_file_name(&path));
            continue;
        }
        if let Some(entry) = import_one(core, engine, &path).map_err(|e| e.to_string())? {
            prewarm_results.push(schedule_import_poster(core, engine, prewarm, &entry, &path));
        }
    }
    Ok(MediaListDto::from_core_with_import_results(
        core,
        Some(engine.cache_root()),
        skipped,
        prewarm_results,
    ))
}

/// `get_media`: the current media catalog for the panel. Infallible.
#[tauri::command]
pub fn get_media(core: State<'_, AppCore>, media: State<'_, MediaState>) -> MediaListDto {
    MediaListDto::from_core(&core, Some(media.engine().cache_root()))
}

/// `toggle_favorite`: add or remove `asset_ids` from the per-project favorites
/// set (#91), returning the refreshed catalog so the panel's "mine" tab and the
/// per-card favorite affordance update. Favorites persist in the project manifest
/// (not browser storage); unknown ids are ignored by the core.
#[tauri::command]
pub fn toggle_favorite(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    asset_ids: Vec<String>,
    favorite: bool,
) -> Result<MediaListDto, String> {
    toggle_favorite_impl(&core, media.engine().cache_root(), asset_ids, favorite)
        .map_err(|e| e.to_string())
}

fn toggle_favorite_impl(
    core: &AppCore,
    cache_root: &Path,
    asset_ids: Vec<String>,
    favorite: bool,
) -> Result<MediaListDto, CoreError> {
    core.set_media_favorite(&asset_ids, favorite)?;
    Ok(MediaListDto::from_core(core, Some(cache_root)))
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
) -> Result<MediaListDto, String> {
    save_clip_as_media_impl(&core, || {
        save_clip_as_media_workflow(&app, &core, &control, media.engine(), &prewarm, &clip_id)
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
    app: &AppHandle,
    core: &AppCore,
    control: &crate::export::ExportControl,
    engine: &MediaEngine,
    prewarm: &prewarm::PrewarmScheduler,
    clip_id: &str,
) -> Result<MediaListDto, String> {
    let snapshot = core.runtime_snapshot();
    let project_dir = snapshot
        .project_dir
        .clone()
        .ok_or("save your project before saving a clip as media")?;
    let (single_timeline, subset, media_type) =
        build_single_clip_export(&snapshot.timeline, &snapshot.media, clip_id)?;
    let ext = save_clip_extension(media_type)?;
    let _guard = control.try_begin()?;
    let out_path =
        crate::export::unique_project_media_path(&project_dir, &format!("clip_{clip_id}"), ext)?;
    let project_dir_option = Some(project_dir.clone());

    let on_progress = |done: i32, total: i32| {
        crate::export::emit_export_progress(app, done, total);
    };

    let export_result = match ext {
        "mp4" => {
            let req = crate::export::ExportRequest {
                out_path: out_path.to_string_lossy().into_owned(),
                codec: crate::export::ExportCodec::H264,
                quality: crate::export::ExportQuality::P1080,
            };
            crate::export::run_export_with_control(
                &single_timeline,
                &subset,
                &project_dir_option,
                &req,
                Some(control),
                Some(&on_progress),
                None,
            )
            .map(|_| ())
        }
        "wav" => crate::export::mix_timeline_audio_for_manifest(
            &single_timeline,
            &subset,
            &project_dir_option,
        )
        .and_then(|pcm| pcm.ok_or_else(|| "audio clip contains no decodable audio".to_string()))
        .and_then(|pcm| {
            if control.is_cancelled() {
                return Err(crate::export::CANCELLED_SENTINEL.to_string());
            }
            crate::export::write_wav_s16le(&pcm.samples_f32, pcm.spec.sample_rate, &out_path)?;
            if control.is_cancelled() {
                return Err(crate::export::CANCELLED_SENTINEL.to_string());
            }
            crate::export::emit_export_progress(app, 1, 1);
            Ok(())
        }),
        _ => unreachable!("save clip extension is fixed by clip type"),
    };

    crate::export::cleanup_partial_output(
        &out_path,
        export_result.and_then(|_| {
            import_saved_media(
                core,
                engine,
                prewarm,
                snapshot.project_epoch,
                &project_dir,
                &out_path,
            )
        }),
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
    media_id: String,
    out_path: String,
) -> Result<String, String> {
    // Path boundary check first (review #4): fail fast on a bad output path
    // before touching the manifest or spawning ffmpeg.
    let output = validate_extract_output(&out_path)?;
    let manifest = core.media();
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.id == media_id)
        .ok_or_else(|| format!("unknown media id: {media_id}"))?;
    let input = match &entry.source {
        MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
        MediaSource::Project { relative_path } => match core.project_dir() {
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
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    media_ref: String,
    new_path: String,
) -> Result<MediaListDto, String> {
    let new = PathBuf::from(&new_path);
    if !new.is_file() {
        return Err(format!("file not found: {new_path}"));
    }
    // Validate the target type matches before touching the catalog (upstream
    // rejects relinking across types). `relink_media_file` re-checks, but doing
    // it here yields a precise message and avoids a needless probe.
    let manifest = core.media();
    let entry = manifest
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
    core.relink_media_file(&media_ref, &new, &probe)
        .map_err(|e| e.to_string())?;
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
    let manifest = core.media();
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.id == media_ref)
        .ok_or_else(|| format!("media not found: {media_ref}"))?;
    let path = source_path_for_entry(&core, entry)?;
    generate_thumbnail_for_entry(
        media.engine(),
        entry,
        &path,
        time_secs,
        max_frames,
        include_sprite.unwrap_or(false),
    )
    .map_err(|e| {
        eprintln!(
            "generate_thumbnail failed: media_ref={media_ref} path={} error={e}",
            path.display()
        );
        e
    })
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
    let manifest = core.media();
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.id == media_ref)
        .ok_or_else(|| format!("media not found: {media_ref}"))?;
    if entry.kind != ClipType::Video {
        return Ok(None);
    }
    let path = source_path_for_entry(&core, entry)?;
    if !path.is_file() {
        return Err(format!("source file not found: {}", path.display()));
    }
    let key = cache_key_for(&path)?;
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
    let manifest = core.media();
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.id == media_ref)
        .ok_or_else(|| format!("media not found: {media_ref}"))?;
    let path = match &entry.source {
        MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
        MediaSource::Project { relative_path } => match core.project_dir() {
            Some(base) => base.join(relative_path),
            None => return Err("project not saved; cannot resolve media path".into()),
        },
    };
    media.engine().waveform(&path, entry.duration).map_err(|e| {
        // Log server-side too (the frontend swallows the error into "no
        // waveform"); without this a decode failure is invisible.
        eprintln!(
            "get_waveform failed: media_ref={media_ref} path={} error={e}",
            path.display()
        );
        e.to_string()
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

        let error = toggle_favorite_impl(
            &core,
            &tmp.path().join("cache"),
            vec!["asset-1".into()],
            true,
        )
        .expect_err("favorite must be rejected");

        assert_eq!(error.code(), "validation");
        assert_eq!(core.media(), before);
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
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        };
        let dto = MediaItemDto::from_entry(&entry, None, None, false);
        assert!(!dto.missing);
        assert_eq!(dto.file_size, Some(10));
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
            has_audio: false,
            path: Some("/p.png".into()),
            thumbnail: None,
            folder_id: None,
            file_size: Some(2048),
            generation_input: None,
            missing: false,
            favorite: true,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"hasAudio\""));
        assert!(json.contains("\"type\":\"image\""));
        assert!(json.contains("\"thumbnail\":null"));
        assert!(json.contains("\"folderId\":null"));
        assert!(json.contains("\"fileSize\":2048"));
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
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.project_revision().project_epoch);
        let mut skipped = Vec::new();
        let mut prewarm_results = Vec::new();
        mirror_dir_scheduled(
            &core,
            &engine,
            &scheduler,
            &root,
            None,
            &mut skipped,
            &mut prewarm_results,
        )
        .unwrap();

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
    fn media_list_dto_projects_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Lib");
        fs::create_dir(&root).unwrap();
        touch(&root.join("x.png"));
        let core = AppCore::new();
        let engine = engine_for(tmp.path());
        let scheduler = prewarm::PrewarmScheduler::new(core.project_revision().project_epoch);
        let mut skipped = Vec::new();
        let mut prewarm_results = Vec::new();
        mirror_dir_scheduled(
            &core,
            &engine,
            &scheduler,
            &root,
            None,
            &mut skipped,
            &mut prewarm_results,
        )
        .unwrap();

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
    fn toggle_favorite_marks_item_and_ignores_unknown_ids() {
        // #91: favoriting flows through the core into the DTO's `favorite` flag —
        // the media panel's "mine" tab reads this, not browser storage.
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
            let p = validate_extract_output(&format!("/tmp/out.{ext}"))
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
        let err = validate_extract_output("/tmp/out.mp4").unwrap_err();
        assert!(
            err.contains("unsupported audio extension"),
            "video extension must be rejected: got {err}"
        );
    }

    #[test]
    fn validate_extract_output_rejects_missing_extension() {
        let err = validate_extract_output("/tmp/out").unwrap_err();
        assert!(
            err.contains("no extension"),
            "extensionless path must be rejected: got {err}"
        );
    }
}
