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
//! Thumbnails are intentionally left as a placeholder (`thumbnail: None`) in
//! this phase: the panel renders from `id` / `name` / `type` / `duration` and
//! the resolvable `path`; persisting + serving thumbnail images to the WebView
//! is a separate concern wired in a later phase.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use opentake_core::{importable_clip_type, AppCore, EditCommand, ProbedMedia};
use opentake_domain::{ClipType, MediaManifestEntry, MediaSource};
use opentake_media::MediaEngine;

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
/// thumbnail path when one exists (always `None` in this phase — the panel falls
/// back to a type placeholder). `path` is the resolvable absolute source path.
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
    /// `true` when the asset's source file is not on disk (moved / deleted /
    /// offline). Derived from file existence on every read (mirrors upstream
    /// `MediaResolver.isMissing`), so it clears automatically once a `relink_media`
    /// points the asset at a real file again. The panel/timeline render an
    /// "offline" affordance for missing assets.
    pub missing: bool,
}

impl MediaItemDto {
    /// Project a manifest entry onto the panel DTO. `project_dir` resolves
    /// [`MediaSource::Project`] relative paths for the `missing` existence check.
    fn from_entry(entry: &MediaManifestEntry, project_dir: Option<&Path>) -> Self {
        let resolved = resolve_source_path(entry, project_dir);
        let path = match &entry.source {
            MediaSource::External { absolute_path } => Some(absolute_path.clone()),
            // Project-relative assets need the bundle base to resolve; not
            // produced by importing (always external) but handled for safety.
            MediaSource::Project { .. } => None,
        };
        // Missing = we can resolve a local source path and it doesn't exist.
        // An unresolvable (e.g. remote-only) source is not flagged missing.
        let missing = resolved.map(|p| !p.exists()).unwrap_or(false);
        MediaItemDto {
            id: entry.id.clone(),
            name: entry.name.clone(),
            kind: entry.kind,
            duration: entry.duration,
            width: entry.source_width,
            height: entry.source_height,
            has_audio: entry.has_audio.unwrap_or(false),
            path,
            thumbnail: None,
            folder_id: entry.folder_id.clone(),
            missing,
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
}

impl MediaListDto {
    /// Build the list from the core's current manifest snapshot.
    fn from_core(core: &AppCore) -> Self {
        let manifest = core.media();
        let project_dir = core.project_dir();
        MediaListDto {
            items: manifest
                .entries
                .iter()
                .map(|e| MediaItemDto::from_entry(e, project_dir.as_deref()))
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
        }
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

/// Import one file into the core, probing it first. Returns the created entry, or
/// `None` when the extension is not importable (the file is skipped, not an
/// error — matches upstream's per-file tolerance during folder/batch import).
fn import_one(core: &AppCore, engine: &MediaEngine, path: &Path) -> Option<MediaManifestEntry> {
    importable_clip_type(path)?;
    let probe = probe_media(engine, path);
    // `import_media_file` re-validates the extension; the type check above only
    // lets us skip probing unsupported files.
    core.import_media_file(path, display_name(path), &probe)
        .ok()
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
    path: String,
    recursive: Option<bool>,
) -> Result<MediaListDto, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let engine = media.engine();

    if recursive.unwrap_or(false) {
        mirror_dir(&core, engine, &root, None);
    } else {
        for file in &collect_media_files(&root, false) {
            let _ = import_one(&core, engine, file);
        }
    }
    Ok(MediaListDto::from_core(&core))
}

/// Recursively mirror `dir` into the library: create a folder for `dir` (nested
/// under `parent_folder_id`), import its direct media files into that folder, and
/// recurse into subdirectories. Hidden entries (dot-prefixed) are skipped.
fn mirror_dir(core: &AppCore, engine: &MediaEngine, dir: &Path, parent_folder_id: Option<String>) {
    let folder_id = create_folder(core, &dir_name(dir), parent_folder_id);

    // Partition this directory's visible entries into media files + subdirs,
    // both in case-insensitive name order.
    let (files, subdirs) = list_dir(dir);

    let mut imported_ids = Vec::new();
    for file in &files {
        if let Some(entry) = import_one(core, engine, file) {
            imported_ids.push(entry.id);
        }
    }
    if let Some(fid) = &folder_id {
        if !imported_ids.is_empty() {
            let _ = core.apply(EditCommand::MoveToFolder {
                asset_ids: imported_ids,
                folder_id: Some(fid.clone()),
            });
        }
    }

    for sub in subdirs {
        mirror_dir(core, engine, &sub, folder_id.clone());
    }
}

/// Create a library folder, returning its new id (or `None` if the core rejected
/// it — e.g. an empty name, which `dir_name` avoids).
fn create_folder(core: &AppCore, name: &str, parent_folder_id: Option<String>) -> Option<String> {
    core.apply(EditCommand::CreateFolder {
        name: name.to_string(),
        parent_folder_id,
    })
    .ok()
    .and_then(|res| res.affected_clip_ids.into_iter().next())
}

/// Directory display name (its last path component), falling back to "folder".
fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "folder".to_string())
}

/// One directory's visible media files + subdirectories, each sorted by
/// case-insensitive name (skipping dot-prefixed entries).
fn list_dir(dir: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (files, subdirs);
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
        }
    }
    let by_name = |a: &PathBuf, b: &PathBuf| {
        let an = a.file_name().map(|s| s.to_string_lossy().to_lowercase());
        let bn = b.file_name().map(|s| s.to_string_lossy().to_lowercase());
        an.cmp(&bn)
    };
    files.sort_by(by_name);
    subdirs.sort_by(by_name);
    (files, subdirs)
}

/// `import_media`: import an explicit list of file paths, returning the updated
/// catalog. Unsupported or unreadable paths are skipped (not fatal); the
/// returned list reflects whatever imported successfully.
#[tauri::command]
pub fn import_media(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    paths: Vec<String>,
) -> Result<MediaListDto, String> {
    let engine = media.engine();
    for p in &paths {
        let path = PathBuf::from(p);
        if path.is_file() {
            let _ = import_one(&core, engine, &path);
        }
    }
    Ok(MediaListDto::from_core(&core))
}

/// `get_media`: the current media catalog for the panel. Infallible.
#[tauri::command]
pub fn get_media(core: State<'_, AppCore>) -> MediaListDto {
    MediaListDto::from_core(&core)
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
    Ok(MediaListDto::from_core(&core))
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

/// Collect importable media files under `root`. Top-level only unless
/// `recursive`. Sorted by case-insensitive file name so a folder import mints
/// asset ids in a stable order. Hidden entries (dot-prefixed) are skipped, as
/// upstream does (`.skipsHiddenFiles`).
fn collect_media_files(root: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_into(root, recursive, &mut out);
    out.sort_by(|a, b| {
        let an = a.file_name().map(|s| s.to_string_lossy().to_lowercase());
        let bn = b.file_name().map(|s| s.to_string_lossy().to_lowercase());
        an.cmp(&bn)
    });
    out
}

fn collect_into(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_hidden = path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.starts_with('.'))
            .unwrap_or(false);
        if is_hidden {
            continue;
        }
        if path.is_dir() {
            if recursive {
                collect_into(&path, recursive, out);
            }
        } else if importable_clip_type(&path).is_some() {
            out.push(path);
        }
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
        let dto = MediaItemDto::from_entry(&entry, None);
        assert_eq!(dto.id, "a");
        assert_eq!(dto.kind, ClipType::Video);
        assert_eq!(dto.duration, 3.0);
        assert_eq!(dto.width, Some(640));
        assert!(dto.has_audio);
        assert_eq!(dto.path.as_deref(), Some("/abs/clip.mp4"));
        assert_eq!(dto.thumbnail, None);
        // /abs/clip.mp4 doesn't exist → missing is true (existence-derived).
        assert!(dto.missing);
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
            missing: false,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"hasAudio\""));
        assert!(json.contains("\"type\":\"image\""));
        assert!(json.contains("\"thumbnail\":null"));
        assert!(json.contains("\"folderId\":null"));
        assert!(json.contains("\"missing\":false"));
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
        mirror_dir(&core, &engine, &root, None);

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
    }

    #[test]
    fn media_list_dto_projects_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Lib");
        fs::create_dir(&root).unwrap();
        touch(&root.join("x.png"));
        let core = AppCore::new();
        let engine = engine_for(tmp.path());
        mirror_dir(&core, &engine, &root, None);

        let dto = MediaListDto::from_core(&core);
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
    fn collect_top_level_only_skips_subdirs_and_hidden_and_unsupported() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("a.mp4"));
        touch(&root.join("b.png"));
        touch(&root.join("c.txt")); // unsupported
        touch(&root.join(".hidden.mp4")); // hidden
        fs::create_dir(root.join("sub")).unwrap();
        touch(&root.join("sub").join("d.mov"));

        let files = collect_media_files(root, false);
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.mp4", "b.png"]);
    }

    #[test]
    fn collect_recursive_includes_subdirs_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("z.mp4"));
        fs::create_dir(root.join("sub")).unwrap();
        touch(&root.join("sub").join("a.mov"));

        let files = collect_media_files(root, true);
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        // Sorted case-insensitively by file name: a.mov before z.mp4.
        assert_eq!(names, vec!["a.mov", "z.mp4"]);
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

        let list = MediaListDto::from_core(&core);
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].kind, ClipType::Video);
        assert_eq!(list.items[0].name, "clip");
        assert_eq!(list.items[0].path.as_deref(), Some(good.to_str().unwrap()));
    }

    #[test]
    fn get_media_reflects_imported_items() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let core = AppCore::new();
        let engine = engine_for(root);
        let f = root.join("a.png");
        touch(&f);
        import_one(&core, &engine, &f);

        let list = MediaListDto::from_core(&core);
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
        let id = import_one(&core, &engine, &orig).unwrap().id;

        // Source goes missing → the panel reads it as offline.
        fs::remove_file(&orig).unwrap();
        let list = MediaListDto::from_core(&core);
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

        let list = MediaListDto::from_core(&core);
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
        let id = import_one(&core, &engine, &orig).unwrap().id;

        // Relinking a video asset to an audio file is rejected (upstream parity).
        let wrong = root.join("song.mp3");
        touch(&wrong);
        let probe = probe_media(&engine, &wrong);
        assert!(core.relink_media_file(&id, &wrong, &probe).is_err());
        let list = MediaListDto::from_core(&core);
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
}
