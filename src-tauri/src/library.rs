//! Global asset library command surface (#55, part of #37 "全局可复用素材库").
//!
//! These commands sit on top of [`opentake_media::library::LibraryStore`] (#54),
//! a cross-project, copy-on-favorite store rooted at `<data dir>/OpenTake/Library`.
//! The store owns its persistence primitives (atomic manifest,
//! content-addressed files, in-process write lock). Commands that span the
//! global store and current project additionally take a workflow lock, then map
//! boundary `MediaError`s to `String` so the WebView gets a plain rejected
//! Promise (`AGENTS.md`: "边界层转 Tauri 的 `Err(String)`").
//!
//! `library_import_to_project` bridges the global library back into the *current*
//! project: it resolves the stored copy for an entry id, probes it via the media
//! engine, and appends it to the [`AppCore`] manifest with a fresh project asset
//! id (so the same favorite can be imported into many projects). Cross-store and
//! project mutations hold [`LibraryState::workflow_lock`]; the store's own lock
//! still owns each atomic library-manifest operation. It reuses the
//! [`crate::media::MediaState`] engine for probing rather than re-opening one.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use same_file::Handle;
use serde::Serialize;
use tauri::State;

use opentake_core::{importable_clip_type, AppCore, DeferredCoreEvents, ProbedMedia};
use opentake_media::library::{FavoriteRequest, LibraryEntry, LibraryStore};

use crate::media::MediaState;

/// Managed-state wrapper over the global [`LibraryStore`]. The store is shared
/// across commands behind an `Arc` (it has no `Clone`); `Send + Sync` so it
/// lives in Tauri managed state.
pub struct LibraryState {
    store: Option<Arc<LibraryStore>>,
    init_error: Option<String>,
    workflow_lock: Mutex<()>,
}

impl LibraryState {
    /// Wrap a store for managed state.
    pub fn new(store: LibraryStore) -> Self {
        if let Err(error) = store.reconcile_storage() {
            return LibraryState::unavailable(format!(
                "global library unavailable: storage reconciliation failed: {error}"
            ));
        }
        LibraryState {
            store: Some(Arc::new(store)),
            init_error: None,
            workflow_lock: Mutex::new(()),
        }
    }

    /// Keep ordinary editing available when the platform data directory cannot
    /// be resolved, while making every library command fail explicitly.
    pub fn unavailable(error: impl Into<String>) -> Self {
        LibraryState {
            store: None,
            init_error: Some(error.into()),
            workflow_lock: Mutex::new(()),
        }
    }

    /// The shared store handle.
    pub fn store(&self) -> Result<&LibraryStore, String> {
        self.store.as_deref().ok_or_else(|| {
            self.init_error
                .clone()
                .unwrap_or_else(|| "global library is unavailable".to_string())
        })
    }

    /// Serialize workflows that span the global manifest and current project.
    pub fn lock_workflow(&self) -> MutexGuard<'_, ()> {
        self.workflow_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// One library entry for the front end. A direct, serde-stable mirror of
/// [`LibraryEntry`] (camelCase, `type` key, `favoritedAt`) so the command surface
/// owns its wire shape independently of the storage type. Every field is
/// optional/defaulted on the store side; here they are always populated.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntryDto {
    /// Content hash (SHA-256 hex) — the library-internal id.
    pub id: String,
    /// Asset kind: `"video" | "audio" | "image" | ...`. `type` in JSON.
    #[serde(rename = "type")]
    pub kind: String,
    /// User category/tag; `None` when uncategorized.
    pub category: Option<String>,
    /// Unix epoch seconds when favorited.
    pub favorited_at: f64,
    /// Original source path the file was copied from.
    pub source: Option<String>,
    /// Optional thumbnail reference (path or data URI).
    pub thumb: Option<String>,
    /// Reserved compatibility field. Library ambient paths are never exposed as
    /// media authority; previews use only content URLs supplied in `thumb`.
    pub stored_path: Option<String>,
}

impl From<LibraryEntry> for LibraryEntryDto {
    fn from(e: LibraryEntry) -> Self {
        LibraryEntryDto {
            id: e.id,
            kind: e.kind,
            category: e.category,
            favorited_at: e.favorited_at,
            source: e.source,
            thumb: e.thumb,
            stored_path: None,
        }
    }
}

fn entry_dto(entry: LibraryEntry) -> LibraryEntryDto {
    LibraryEntryDto::from(entry)
}

/// The asset minted in the current project by `library_import_to_project`. The
/// front end re-fetches the full catalog via `get_media` after a successful
/// import; this is the just-created project-side asset for an optimistic update.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryImportDto {
    /// New project asset id (the clip layer's `media_ref`).
    pub id: String,
    /// Display name (derived from the original source file name).
    pub name: String,
    /// Absolute path of the imported (library-stored) source file.
    pub path: String,
}

/// `library_list`: every favorited entry, or only those in `category` when
/// supplied. `category = Some("")` and an omitted `category` both list **all**
/// entries; pass a non-empty string to filter, or use the dedicated
/// uncategorized view by sending the sentinel the front end agrees on. To keep
/// the contract simple, `None`/empty = all, non-empty = that category.
#[tauri::command]
pub fn library_list(
    library: State<'_, LibraryState>,
    category: Option<String>,
) -> Result<Vec<LibraryEntryDto>, String> {
    let store = library.store()?;
    let entries = match category.as_deref() {
        None | Some("") => store.entries(),
        Some(c) => store.entries_in_category(Some(c)),
    }
    .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(entry_dto).collect())
}

/// `library_favorite`: copy a local file into the global library (dedup by
/// content hash) and record an entry. `favorited_at` is recorded server-side
/// from the wall clock so the front end never has to supply it. Returns the
/// created (or pre-existing, on dedup) entry.
#[tauri::command]
pub fn library_favorite(
    library: State<'_, LibraryState>,
    source: String,
    kind: String,
    category: Option<String>,
    thumb: Option<String>,
) -> Result<LibraryEntryDto, String> {
    let _workflow = library.lock_workflow();
    let store = library.store()?;
    let source_path = PathBuf::from(&source);
    if !source_path.is_file() {
        return Err(format!("source file not found: {source}"));
    }
    let req = FavoriteRequest {
        source: &source_path,
        kind: &kind,
        category,
        favorited_at: now_epoch_secs(),
        thumb,
    };
    let entry = store.favorite(&req).map_err(|e| e.to_string())?;
    Ok(entry_dto(entry))
}

/// `library_unfavorite`: remove an entry (and its stored copy) by id. Returns
/// `true` if an entry was removed, `false` if the id was unknown (idempotent).
#[tauri::command]
pub fn library_unfavorite(
    core: State<'_, AppCore>,
    library: State<'_, LibraryState>,
    id: String,
) -> Result<bool, String> {
    remove_from_library_and_project(&core, &library, &id)
}

/// `library_categorize`: set (or clear, with `category = None`) the category of
/// one entry. Returns the updated entry, or an error if the id is unknown.
#[tauri::command]
pub fn library_categorize(
    library: State<'_, LibraryState>,
    id: String,
    category: Option<String>,
) -> Result<LibraryEntryDto, String> {
    let _workflow = library.lock_workflow();
    let store = library.store()?;
    let entry = store
        .set_category(&id, category)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("unknown library entry: {id}"))?;
    Ok(entry_dto(entry))
}

/// `library_rename`: rename a category — move every entry whose category equals
/// `from` to `to` (`to = None` un-categorizes them). Returns the number of
/// entries changed (0 when no entry was in `from`).
#[tauri::command]
pub fn library_rename(
    library: State<'_, LibraryState>,
    from: String,
    to: Option<String>,
) -> Result<usize, String> {
    let _workflow = library.lock_workflow();
    library
        .store()?
        .rename_category(&from, to)
        .map_err(|e| e.to_string())
}

/// `library_delete`: alias of `library_unfavorite` for the front end's "delete
/// from library" affordance. Removes the entry and its stored copy by id;
/// returns `true` if something was removed.
#[tauri::command]
pub fn library_delete(
    core: State<'_, AppCore>,
    library: State<'_, LibraryState>,
    id: String,
) -> Result<bool, String> {
    remove_from_library_and_project(&core, &library, &id)
}

fn remove_from_library_and_project(
    core: &AppCore,
    library: &LibraryState,
    id: &str,
) -> Result<bool, String> {
    let _workflow = library.lock_workflow();
    let store = library.store()?;
    let project = core.runtime_snapshot();
    if project.project_dir.is_some() {
        core.ensure_project_mutable()
            .map_err(|error| error.to_string())?;
    }
    let removed = store
        .remove(id)
        .map_err(|error| format!("global favorite could not be removed: {error}"))?;
    let mut events = DeferredCoreEvents::default();
    if let Some(project_dir) = project.project_dir.as_deref() {
        let cleared = core
            .clear_media_global_favorite_id_for_project_deferred(
                project.project_epoch,
                project_dir,
                id,
                &mut events,
            )
            .map_err(|e| e.to_string())?;
        if cleared > 0 {
            if let Err(error) = core.save_media_manifest_for_project_deferred(
                project.project_epoch,
                project_dir,
                &mut events,
            ) {
                crate::media::restore_project_favorites(
                    core,
                    project.project_epoch,
                    project_dir,
                    &project.media,
                    &mut events,
                );
                core.emit_deferred(events);
                return Err(format!(
                    "project favorite mirror could not be saved: {error}"
                ));
            }
        }
    }
    core.emit_deferred(events);
    Ok(removed)
}

/// `library_import_to_project`: bring a library entry into the *current* project.
/// Resolves the entry's stored copy, probes it for metadata, and appends it to
/// the core manifest with a fresh project asset id (so one favorite can seed many
/// projects). Errors when the id is unknown, the stored file is missing, the
/// kind is not importable, or the import is rejected by the core.
#[tauri::command]
pub fn library_import_to_project(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    library: State<'_, LibraryState>,
    id: String,
) -> Result<LibraryImportDto, String> {
    library_import_to_project_impl(&core, &media, &library, &id)
}

fn library_import_to_project_impl(
    core: &AppCore,
    media: &MediaState,
    library: &LibraryState,
    id: &str,
) -> Result<LibraryImportDto, String> {
    let _workflow = library.lock_workflow();
    let mut events = DeferredCoreEvents::default();
    let result = {
        let _project_identity = core.lock_project_identity_workflow();
        library_import_to_project_with_events(core, media, library, id, &mut events)
    };
    core.emit_deferred(events);
    result
}

fn library_import_to_project_with_events(
    core: &AppCore,
    media: &MediaState,
    library: &LibraryState,
    id: &str,
    events: &mut DeferredCoreEvents,
) -> Result<LibraryImportDto, String> {
    library_import_to_project_with_hook(core, media, library, id, events, |_| {})
}

fn library_import_to_project_with_hook(
    core: &AppCore,
    media: &MediaState,
    library: &LibraryState,
    id: &str,
    events: &mut DeferredCoreEvents,
    after_project_commit: impl FnOnce(&Path),
) -> Result<LibraryImportDto, String> {
    core.ensure_project_mutable()
        .map_err(|error| error.to_string())?;
    let project = core.runtime_snapshot();
    let project_dir = project
        .project_dir
        .clone()
        .ok_or_else(|| "save the project before importing a global favorite".to_string())?;
    let store = library.store()?;
    let stored_name = store
        .stored_file_name(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("library entry has no stored file: {id}"))?;
    if importable_clip_type(Path::new(&stored_name)).is_none() {
        return Err(format!(
            "library file is not an importable media type: {}",
            Path::new(&stored_name).display()
        ));
    }
    let extension = Path::new(&stored_name)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .unwrap_or("bin");
    let media_dir = opentake_project::layout::media_dir(&project_dir);
    std::fs::create_dir_all(&media_dir).map_err(|error| error.to_string())?;
    static IMPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = IMPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let imported_path = media_dir.join(format!(
        "library-{id}-{}-{sequence}.{extension}",
        std::process::id()
    ));
    let mut imported_options = std::fs::OpenOptions::new();
    imported_options.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        imported_options.share_mode(0x1 | 0x2);
    }
    let imported_file = imported_options
        .open(&imported_path)
        .map_err(|error| error.to_string())?;
    let imported_handle = Handle::from_file(imported_file).map_err(|error| error.to_string())?;
    let mut imported = ProjectImportGuard {
        path: imported_path,
        handle: imported_handle,
        committed: false,
    };
    store
        .copy_stored_verified(id, imported.handle.as_file_mut())
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("library entry has no stored file: {id}"))?;
    imported
        .handle
        .as_file_mut()
        .flush()
        .map_err(|error| error.to_string())?;
    imported
        .handle
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    if !imported.matches_path().map_err(|error| error.to_string())? {
        return Err("project import leaf identity changed before probe".to_string());
    }

    let probe = probe_or_default(media.engine(), &imported.path);
    if !imported.matches_path().map_err(|error| error.to_string())? {
        return Err("project import leaf identity changed during probe".to_string());
    }
    let name = display_name(Path::new(&stored_name));
    let entry = core
        .import_library_media_for_project_deferred(
            project.project_epoch,
            &project_dir,
            &imported.path,
            name.clone(),
            &probe,
            id,
            events,
        )
        .map_err(|e| e.to_string())?;
    after_project_commit(&imported.path);
    if !imported.matches_path().map_err(|error| error.to_string())? {
        core.restore_media_manifest_for_project_deferred(
            project.project_epoch,
            &project_dir,
            project.media,
            events,
        )
        .map_err(|error| error.to_string())?;
        return Err("project import leaf identity changed during commit".to_string());
    }
    imported.committed = true;

    Ok(LibraryImportDto {
        id: entry.id,
        name: entry.name,
        path: imported.path.to_string_lossy().into_owned(),
    })
}

struct ProjectImportGuard {
    path: PathBuf,
    handle: Handle,
    committed: bool,
}

impl ProjectImportGuard {
    fn matches_path(&self) -> std::io::Result<bool> {
        Ok(self.handle == Handle::from_path(&self.path)?)
    }
}

impl Drop for ProjectImportGuard {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.handle.as_file().set_len(0);
            let _ = self.handle.as_file().sync_all();
        }
    }
}

/// Probe a stored library file, degrading to defaults on any probe failure (no
/// ffprobe / unreadable) so importing never fails on metadata alone — mirrors the
/// best-effort import path in [`crate::media`].
fn probe_or_default(engine: &opentake_media::MediaEngine, path: &std::path::Path) -> ProbedMedia {
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
/// is no stem.
fn display_name(path: &std::path::Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Current Unix epoch seconds as `f64`. Falls back to `0.0` if the system clock
/// is set before the epoch (not expected on a real machine).
fn now_epoch_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved_core_with_mapping(
        root: &std::path::Path,
        library_id: &str,
    ) -> (AppCore, PathBuf, String) {
        let source = root.join("source.mp4");
        std::fs::write(&source, b"project media").unwrap();
        let bundle = root.join("Mapped.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let entry = core
            .import_media_file(&source, "source", &ProbedMedia::default())
            .unwrap();
        core.set_media_global_favorite(&entry.id, Some(library_id.to_string()))
            .unwrap();
        core.save_project(None).unwrap();
        (core, bundle, entry.id)
    }

    fn engine_for(root: &std::path::Path) -> MediaState {
        MediaState::new(opentake_media::MediaEngine::new(
            root.join("cache"),
            root.join("models"),
        ))
    }

    #[test]
    fn library_list_dto_never_exposes_an_ambient_library_path() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"library bytes").unwrap();
        let store = LibraryStore::new(tmp.path().join("library"));
        let entry = store
            .favorite(&FavoriteRequest {
                source: &source,
                kind: "video",
                category: None,
                favorited_at: 1.0,
                thumb: None,
            })
            .unwrap();

        let dto = entry_dto(entry);

        assert_eq!(dto.stored_path, None);
    }

    #[test]
    fn library_state_startup_reconciles_crash_window_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("library");
        let files = root.join(opentake_media::library::FILES_SUBDIR);
        std::fs::create_dir_all(files.join(".staging")).unwrap();
        std::fs::write(files.join(".staging/crashed.pending"), b"stage").unwrap();
        std::fs::write(
            files.join(format!("{}.mp4", "0".repeat(64))),
            b"final orphan",
        )
        .unwrap();

        let library = LibraryState::new(LibraryStore::new(root));

        assert!(library.store().is_ok());
        assert_eq!(
            std::fs::read_dir(files.join(".staging"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.metadata().is_ok_and(|metadata| metadata.len() > 0))
                .count(),
            0
        );
        assert_eq!(
            std::fs::metadata(files.join(format!("{}.mp4", "0".repeat(64))))
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn library_import_marks_and_persists_the_new_project_asset() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"library bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = library
            .store()
            .unwrap()
            .favorite(&FavoriteRequest {
                source: &source,
                kind: "video",
                category: None,
                favorited_at: 1.0,
                thumb: None,
            })
            .unwrap();
        let bundle = tmp.path().join("Import.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();

        let imported =
            library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
                .expect("import library item");

        assert_eq!(
            core.media().library_favorite_id(&imported.id),
            Some(entry.id.as_str())
        );
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        assert_eq!(
            reopened.media().library_favorite_id(&imported.id),
            Some(entry.id.as_str())
        );

        assert!(remove_from_library_and_project(&core, &library, &entry.id).unwrap());
        let reopened_after_remove = AppCore::new();
        reopened_after_remove.open_project(bundle).unwrap();
        assert_eq!(
            reopened_after_remove
                .media()
                .library_favorite_id(&imported.id),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn import_leaf_replacement_rolls_back_while_identity_lease_blocks_replacement() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"trusted library bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = library
            .store()
            .unwrap()
            .favorite(&FavoriteRequest {
                source: &source,
                kind: "video",
                category: None,
                favorited_at: 1.0,
                thumb: None,
            })
            .unwrap();
        let bundle = tmp.path().join("ImportSwap.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle)).unwrap();
        let before = core.media();
        let replacement_core = core.clone();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker = std::sync::Mutex::new(None);
        let moved_path = std::sync::Mutex::new(None);
        let mut events = DeferredCoreEvents::default();
        let lease = core.lock_project_identity_workflow();

        let error = library_import_to_project_with_hook(
            &core,
            &engine_for(tmp.path()),
            &library,
            &entry.id,
            &mut events,
            |path| {
                let moved = path.with_extension("moved");
                std::fs::rename(path, &moved).unwrap();
                std::fs::write(path, b"replacement bytes").unwrap();
                *moved_path.lock().unwrap() = Some(moved);
                *worker.lock().unwrap() = Some(std::thread::spawn(move || {
                    replacement_core.new_project();
                    done_tx.send(()).unwrap();
                }));
                assert!(done_rx
                    .recv_timeout(std::time::Duration::from_millis(50))
                    .is_err());
            },
        )
        .expect_err("leaf replacement must fail closed");

        assert!(
            error.contains("leaf identity changed during commit"),
            "{error}"
        );
        assert_eq!(core.media(), before);
        let moved = moved_path.lock().unwrap().clone().unwrap();
        assert_eq!(std::fs::metadata(moved).unwrap().len(), 0);
        let replacement = std::fs::read_dir(opentake_project::layout::media_dir(
            core.runtime_snapshot().project_dir.as_ref().unwrap(),
        ))
        .unwrap()
        .find_map(|item| {
            let path = item.ok()?.path();
            (std::fs::read(&path).ok()?.as_slice() == b"replacement bytes").then_some(path)
        });
        assert!(replacement.is_some(), "replacement leaf was modified");
        drop(lease);
        done_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("project replacement proceeds after import lease releases");
        worker.lock().unwrap().take().unwrap().join().unwrap();
    }

    #[test]
    fn unavailable_library_returns_the_initialization_error() {
        let library = LibraryState::unavailable("app data missing");
        assert!(matches!(library.store(), Err(message) if message == "app data missing"));
    }

    #[test]
    fn unavailable_remove_leaves_live_and_reopened_project_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let (core, bundle, asset_id) = saved_core_with_mapping(tmp.path(), "persisted-library-id");
        let before = core.media();
        let library = LibraryState::unavailable("app data missing");

        let error = remove_from_library_and_project(&core, &library, "persisted-library-id")
            .expect_err("unavailable library must reject before project mutation");

        assert_eq!(error, "app data missing");
        assert_eq!(core.media(), before);
        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(reopened.media(), before);
        assert_eq!(
            reopened.media().library_favorite_id(&asset_id),
            Some("persisted-library-id")
        );
    }

    #[test]
    fn already_missing_global_entry_still_clears_and_persists_stale_mirror() {
        let tmp = tempfile::tempdir().unwrap();
        let (core, bundle, asset_id) = saved_core_with_mapping(tmp.path(), "missing-library-id");
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));

        assert!(!remove_from_library_and_project(&core, &library, "missing-library-id").unwrap());

        assert_eq!(core.media().library_favorite_id(&asset_id), None);
        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(reopened.media().library_favorite_id(&asset_id), None);
    }

    #[test]
    fn failed_library_import_restores_manifest_and_retry_does_not_duplicate() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"library bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = library
            .store()
            .unwrap()
            .favorite(&FavoriteRequest {
                source: &source,
                kind: "video",
                category: None,
                favorited_at: 1.0,
                thumb: None,
            })
            .unwrap();
        let bundle = tmp.path().join("ImportFailure.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let before = core.media();
        let manifest_path = bundle.join(opentake_project::layout::MANIFEST_FILE);
        let manifest_before = std::fs::read(&manifest_path).unwrap();
        std::fs::remove_file(&manifest_path).unwrap();
        std::fs::create_dir(&manifest_path).unwrap();

        library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
            .expect_err("atomic media manifest publish failure must roll back import");

        assert_eq!(core.media(), before);
        std::fs::remove_dir(&manifest_path).unwrap();
        std::fs::write(&manifest_path, manifest_before).unwrap();
        let reopened_after_failure = AppCore::new();
        reopened_after_failure.open_project(&bundle).unwrap();
        assert_eq!(reopened_after_failure.media(), before);

        let first =
            library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
                .expect("retry import");
        let second =
            library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
                .expect("idempotent retry");

        assert_eq!(second.id, first.id);
        assert_eq!(core.media().entries.len(), 1);
        assert_eq!(
            core.media().library_favorite_id(&first.id),
            Some(entry.id.as_str())
        );
        let reopened_after_retry = AppCore::new();
        reopened_after_retry.open_project(bundle).unwrap();
        assert_eq!(reopened_after_retry.media(), core.media());
    }

    #[test]
    fn library_import_does_not_rewrite_an_unrelated_generation_log() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"library bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = library
            .store()
            .unwrap()
            .favorite(&FavoriteRequest {
                source: &source,
                kind: "video",
                category: None,
                favorited_at: 1.0,
                thumb: None,
            })
            .unwrap();
        let bundle = tmp.path().join("GenerationLog.opentake");
        let mut project = opentake_project::Project::new(&bundle);
        let mut log = opentake_project::GenerationLog::new();
        log.entries.push(opentake_project::GenerationLogEntry::new(
            "log-1",
            "model",
            Some(1),
            Some(2.0),
        ));
        project.generation_log = Some(log);
        project.save().unwrap();
        let core = AppCore::new();
        core.open_project(&bundle).unwrap();
        let generation_log_path = bundle.join(opentake_project::layout::GENERATION_LOG_FILE);
        let generation_log_before = std::fs::read(&generation_log_path).unwrap();
        std::fs::remove_file(&generation_log_path).unwrap();
        std::fs::create_dir(&generation_log_path).unwrap();

        let imported =
            library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
                .expect("manifest-only import must not touch generation log");

        assert!(generation_log_path.is_dir());
        std::fs::remove_dir(&generation_log_path).unwrap();
        std::fs::write(&generation_log_path, generation_log_before).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(
            reopened.media().library_favorite_id(&imported.id),
            Some(entry.id.as_str())
        );
        assert_eq!(reopened.media(), core.media());
    }
}
