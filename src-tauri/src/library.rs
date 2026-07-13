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

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use cap_fs_ext::{ambient_authority, DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use same_file::Handle;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::State;

use opentake_core::{
    importable_clip_type, AppCore, CoreError, DeferredCoreEvents, ImportCommitWarning, ProbedMedia,
};
use opentake_domain::ClipType;
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
    /// Present only when the import committed but a postcondition rollback
    /// could not be published; callers must treat the returned asset as live.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<LibraryImportWarningDto>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LibraryImportWarningDto {
    PostconditionRollbackFailed {
        postcondition: String,
        rollback: String,
    },
}

impl From<ImportCommitWarning> for LibraryImportWarningDto {
    fn from(warning: ImportCommitWarning) -> Self {
        match warning {
            ImportCommitWarning::PostconditionRollbackFailed {
                postcondition,
                rollback,
            } => Self::PostconditionRollbackFailed {
                postcondition,
                rollback,
            },
        }
    }
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
    let workflow = library.lock_workflow();
    let mut events = DeferredCoreEvents::default();
    let result = {
        let _project_identity = core.lock_project_identity_workflow();
        library_import_to_project_with_events(core, media, library, id, &mut events)
    };
    drop(workflow);
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
    library_import_to_project_with_hook(core, media, library, id, events, |_, _| {})
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImportHookPhase {
    BeforeProbe,
    AfterProbe,
    BeforeManifestWrite,
    AfterManifestWrite,
    AfterProjectCommit,
}

fn library_import_to_project_with_hook(
    core: &AppCore,
    media: &MediaState,
    library: &LibraryState,
    id: &str,
    events: &mut DeferredCoreEvents,
    mut hook: impl FnMut(ImportHookPhase, &Path),
) -> Result<LibraryImportDto, String> {
    core.ensure_project_mutable()
        .map_err(|error| error.to_string())?;
    let project = core.runtime_snapshot();
    let project_dir = project
        .project_dir
        .clone()
        .ok_or_else(|| "save the project before importing a global favorite".to_string())?;
    let store = library.store()?;
    let library_entry = store
        .entries()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| format!("unknown library entry: {id}"))?;
    let source = library_entry
        .source
        .as_deref()
        .ok_or_else(|| format!("library entry has no trusted source metadata: {id}"))?;
    let source_path = Path::new(source);
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .ok_or_else(|| format!("library entry has no safe source extension: {id}"))?
        .to_ascii_lowercase();
    let expected_kind = importable_clip_type(source_path)
        .ok_or_else(|| format!("library source metadata is not importable: {source}"))?;
    if crate::media::clip_type_name(expected_kind) != library_entry.kind {
        return Err(format!(
            "library source metadata type does not match manifest kind: {}",
            library_entry.kind
        ));
    }
    let project_media =
        ProjectMediaCapability::open_verified(core, project.project_epoch, &project_dir, true)?;
    if let Some(existing) = existing_project_import(
        &project_media,
        &project.media,
        id,
        expected_kind,
        &extension,
    )? {
        return Ok(existing);
    }
    static IMPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = IMPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let imported_name = format!("library-{id}-{}-{sequence}.{extension}", std::process::id());
    let imported_handle = project_media.create_leaf(Path::new(&imported_name))?;
    let mut imported = ProjectImportGuard {
        path: project_media.absolute_path(Path::new(&imported_name)),
        name: imported_name.into(),
        media: project_media
            .media
            .try_clone()
            .map_err(|error| error.to_string())?,
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
    if !project_media.matches_leaf(&imported)? {
        return Err("project import leaf identity changed before probe".to_string());
    }

    hook(ImportHookPhase::BeforeProbe, &imported.path);
    let probe = probe_or_default_file(media.engine(), imported.handle.as_file());
    hook(ImportHookPhase::AfterProbe, &imported.path);
    if !project_media.matches_leaf(&imported)? {
        return Err("project import leaf identity changed during probe".to_string());
    }
    let name = display_name(source_path);
    let hook = std::cell::RefCell::new(hook);
    let commit = core
        .import_library_media_for_project_deferred_with_manifest_writer(
            project.project_epoch,
            &project_dir,
            &imported.path,
            name.clone(),
            &probe,
            id,
            events,
            |manifest| {
                (hook.borrow_mut())(ImportHookPhase::BeforeManifestWrite, &imported.path);
                let result = project_media
                    .write_manifest(manifest)
                    .map_err(CoreError::Media);
                (hook.borrow_mut())(ImportHookPhase::AfterManifestWrite, &imported.path);
                result
            },
            || {
                (hook.borrow_mut())(ImportHookPhase::AfterProjectCommit, &imported.path);
                match project_media.matches_leaf(&imported) {
                    Ok(true) => Ok(()),
                    Ok(false) => Err(CoreError::Media(
                        "project import leaf identity changed during commit".to_string(),
                    )),
                    Err(error) => Err(CoreError::Media(format!(
                        "project import leaf identity changed during commit: {error}"
                    ))),
                }
            },
        )
        .map_err(|e| e.to_string())?;
    imported.committed = true;
    let warning = commit.warning.map(LibraryImportWarningDto::from);
    let entry = commit.entry;

    Ok(LibraryImportDto {
        id: entry.id,
        name: entry.name,
        path: imported.path.to_string_lossy().into_owned(),
        warning,
    })
}

struct ProjectMediaCapability {
    parent: Dir,
    root: Dir,
    media: Dir,
    parent_path: PathBuf,
    root_name: std::ffi::OsString,
    parent_identity: Handle,
    root_identity: Handle,
    media_identity: Handle,
    project_dir: PathBuf,
}

impl ProjectMediaCapability {
    fn open_verified(
        core: &AppCore,
        project_epoch: u64,
        project_dir: &Path,
        create_media: bool,
    ) -> Result<Self, String> {
        Self::open_with_root_gate(project_dir, create_media, |root_identity| {
            core.ensure_project_root_identity_for_project(project_epoch, project_dir, root_identity)
                .map_err(|error| error.to_string())
        })
    }

    #[cfg(test)]
    fn open(project_dir: &Path, create_media: bool) -> Result<Self, String> {
        Self::open_with_root_gate(project_dir, create_media, |_| Ok(()))
    }

    fn open_with_root_gate(
        project_dir: &Path,
        create_media: bool,
        gate: impl FnOnce(&Handle) -> Result<(), String>,
    ) -> Result<Self, String> {
        let parent_path = project_dir
            .parent()
            .ok_or_else(|| "project bundle has no parent directory".to_string())?;
        let root_name = project_dir
            .file_name()
            .ok_or_else(|| "project bundle has no final component".to_string())?
            .to_owned();
        let parent = Dir::open_ambient_dir(parent_path, ambient_authority())
            .map_err(|error| error.to_string())?;
        let root = parent
            .open_dir_nofollow(&root_name)
            .map_err(|error| format!("project bundle is not a trusted directory: {error}"))?;
        let parent_identity = Handle::from_file(
            parent
                .try_clone()
                .map_err(|error| error.to_string())?
                .into_std_file(),
        )
        .map_err(|error| error.to_string())?;
        let root_identity = Handle::from_file(
            root.try_clone()
                .map_err(|error| error.to_string())?
                .into_std_file(),
        )
        .map_err(|error| error.to_string())?;
        // This is the write boundary: no media directory or leaf is created
        // until the opened root matches the handle retained by AppCore.
        gate(&root_identity)?;
        if create_media {
            match root.create_dir("media") {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        let media = root
            .open_dir_nofollow("media")
            .map_err(|error| format!("project media directory is unavailable: {error}"))?;
        let media_identity = Handle::from_file(
            media
                .try_clone()
                .map_err(|error| error.to_string())?
                .into_std_file(),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            parent,
            root,
            media,
            parent_path: parent_path.to_owned(),
            root_name,
            parent_identity,
            root_identity,
            media_identity,
            project_dir: project_dir.to_owned(),
        })
    }

    fn create_leaf(&self, name: &Path) -> Result<Handle, String> {
        if !matches!(
            name.components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        ) {
            return Err("project import target must be one relative leaf".to_string());
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;
            use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
            use windows_sys::Win32::Storage::FileSystem::{
                DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };
            options
                .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
        }
        let file = self
            .media
            .open_with(name, &options)
            .map_err(|error| error.to_string())?;
        Handle::from_file(file.into_std()).map_err(|error| error.to_string())
    }

    fn open_leaf(&self, name: &std::ffi::OsStr) -> Result<Handle, String> {
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.share_mode(0x1 | 0x2 | 0x4);
        }
        let file = self
            .media
            .open_with(name, &options)
            .map_err(|error| format!("project library media is unavailable: {error}"))?;
        if !file
            .metadata()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            return Err("project library mapping is not a regular file".to_string());
        }
        Handle::from_file(file.into_std()).map_err(|error| error.to_string())
    }

    fn matches_namespace(&self) -> Result<bool, String> {
        let ambient_parent = Dir::open_ambient_dir(&self.parent_path, ambient_authority())
            .map_err(|error| error.to_string())?;
        let ambient_parent =
            Handle::from_file(ambient_parent.into_std_file()).map_err(|error| error.to_string())?;
        if ambient_parent != self.parent_identity {
            return Ok(false);
        }
        let current_root = self
            .parent
            .open_dir_nofollow(&self.root_name)
            .map_err(|error| error.to_string())?;
        let current_root =
            Handle::from_file(current_root.into_std_file()).map_err(|error| error.to_string())?;
        if current_root != self.root_identity {
            return Ok(false);
        }
        let current_media = self
            .root
            .open_dir_nofollow("media")
            .map_err(|error| error.to_string())?;
        let current_media =
            Handle::from_file(current_media.into_std_file()).map_err(|error| error.to_string())?;
        Ok(current_media == self.media_identity)
    }

    fn matches_handle(&self, name: &std::ffi::OsStr, expected: &Handle) -> Result<bool, String> {
        if !self.matches_namespace()? {
            return Ok(false);
        }
        let current = self.open_leaf(name)?;
        Ok(&current == expected)
    }

    fn matches_leaf(&self, leaf: &ProjectImportGuard) -> Result<bool, String> {
        self.matches_handle(&leaf.name, &leaf.handle)
    }

    fn absolute_path(&self, name: impl AsRef<Path>) -> PathBuf {
        self.project_dir.join("media").join(name)
    }

    fn write_manifest(&self, manifest: &opentake_domain::MediaManifest) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
        opentake_media::library::write_atomic_capability_file(
            &self.root,
            opentake_project::layout::MANIFEST_FILE,
            &bytes,
        )
        .map_err(|error| error.to_string())
    }
}

fn existing_project_import(
    project_media: &ProjectMediaCapability,
    manifest: &opentake_domain::MediaManifest,
    library_id: &str,
    expected_kind: ClipType,
    expected_extension: &str,
) -> Result<Option<LibraryImportDto>, String> {
    let mut matches = manifest
        .favorite_library_ids
        .iter()
        .filter(|(_, mapped_id)| mapped_id.as_str() == library_id);
    let Some((asset_id, _)) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(format!(
            "project contains multiple assets for library entry: {library_id}"
        ));
    }
    let entry = manifest
        .entries
        .iter()
        .find(|entry| entry.id == *asset_id)
        .ok_or_else(|| format!("project library mapping has no media entry: {asset_id}"))?;
    if entry.kind != expected_kind {
        return Err(format!(
            "project library mapping has an unexpected media type: {asset_id}"
        ));
    }
    let relative_path = match &entry.source {
        opentake_domain::MediaSource::Project { relative_path } => Path::new(relative_path),
        opentake_domain::MediaSource::External { .. } => {
            return Err(format!(
                "project library mapping is not backed by project-owned media: {asset_id}"
            ))
        }
    };
    let mut components = relative_path.components();
    let valid_media = matches!(components.next(), Some(Component::Normal(name)) if name == "media");
    let leaf_name = match components.next() {
        Some(Component::Normal(name)) => name,
        _ => {
            return Err(format!(
                "project library mapping has an invalid relative path: {asset_id}"
            ))
        }
    };
    if !valid_media || components.next().is_some() {
        return Err(format!(
            "project library mapping escapes the media directory: {asset_id}"
        ));
    }
    let leaf_extension = Path::new(leaf_name)
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("project library mapping has no media extension: {asset_id}"))?;
    if !leaf_extension.eq_ignore_ascii_case(expected_extension) {
        return Err(format!(
            "project library mapping has an unexpected media extension: {asset_id}"
        ));
    }
    let mut handle = project_media.open_leaf(leaf_name)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = handle
            .as_file_mut()
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != library_id {
        return Err(format!(
            "project library media content does not match mapping: {asset_id}"
        ));
    }
    if !project_media.matches_handle(leaf_name, &handle)? {
        return Err(format!(
            "project library media identity changed during validation: {asset_id}"
        ));
    }
    Ok(Some(LibraryImportDto {
        id: entry.id.clone(),
        name: entry.name.clone(),
        path: project_media
            .absolute_path(leaf_name)
            .to_string_lossy()
            .into_owned(),
        warning: None,
    }))
}

struct ProjectImportGuard {
    path: PathBuf,
    name: std::ffi::OsString,
    media: Dir,
    handle: Handle,
    committed: bool,
}

impl ProjectImportGuard {
    fn owns_name(&self) -> bool {
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };
            options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
        }
        self.media
            .open_with(&self.name, &options)
            .ok()
            .and_then(|file| Handle::from_file(file.into_std()).ok())
            .is_some_and(|current| current == self.handle)
    }
}

impl Drop for ProjectImportGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        // The retained handle still denotes the uncommitted candidate even if
        // its original name was moved or replaced. Scrub those candidate bytes
        // first, then only unlink when the capability-relative name still maps
        // to that exact handle. A replacement leaf is never truncated/deleted.
        let _ = self.handle.as_file().set_len(0);
        let _ = self.handle.as_file().sync_all();
        if !self.owns_name() {
            return;
        }
        #[cfg(windows)]
        if delete_project_import_by_handle(self).is_ok() {
            return;
        }
        if self.owns_name() {
            let _ = self.media.remove_file(&self.name);
        }
    }
}

#[cfg(windows)]
fn delete_project_import_by_handle(guard: &ProjectImportGuard) -> std::io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: the synchronous call receives a valid DELETE-capable retained
    // file handle and a correctly sized FILE_DISPOSITION_INFO value.
    let deleted = unsafe {
        SetFileInformationByHandle(
            guard.handle.as_file().as_raw_handle(),
            FileDispositionInfo,
            std::ptr::addr_of!(disposition).cast(),
            u32::try_from(std::mem::size_of::<FILE_DISPOSITION_INFO>())
                .expect("FILE_DISPOSITION_INFO size fits u32"),
        )
    };
    if deleted == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Probe a stored library file, degrading to defaults on any probe failure (no
/// ffprobe / unreadable) so importing never fails on metadata alone — mirrors the
/// best-effort import path in [`crate::media`].
fn probe_or_default_file(
    engine: &opentake_media::MediaEngine,
    file: &std::fs::File,
) -> ProbedMedia {
    match engine.probe_file(file) {
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

    fn favorite_video(library: &LibraryState, source: &Path) -> LibraryEntry {
        library
            .store()
            .unwrap()
            .favorite(&FavoriteRequest {
                source,
                kind: "video",
                category: None,
                favorited_at: 1.0,
                thumb: None,
            })
            .unwrap()
    }

    fn generate_video(path: &Path, size: &str) -> bool {
        if !opentake_media::ffmpeg_status::ffmpeg_available() {
            return false;
        }
        let ffmpeg = std::env::var_os("OPENTAKE_FFMPEG")
            .unwrap_or_else(|| std::ffi::OsString::from("ffmpeg"));
        std::process::Command::new(ffmpeg)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &format!("color=c=black:s={size}:r=1"),
                "-t",
                "1",
                "-c:v",
                "mpeg4",
                "-y",
            ])
            .arg(path)
            .status()
            .is_ok_and(|status| status.success())
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
            std::fs::read(files.join(".staging/crashed.pending")).unwrap(),
            b"stage"
        );
        assert_eq!(
            std::fs::read(files.join(format!("{}.mp4", "0".repeat(64)))).unwrap(),
            b"final orphan"
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

        assert_eq!(imported.warning, None);
        assert!(serde_json::to_value(&imported)
            .unwrap()
            .get("warning")
            .is_none());
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

    #[test]
    fn deferred_import_events_allow_core_and_library_reentry() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"library bytes").unwrap();
        let library = Arc::new(LibraryState::new(LibraryStore::new(
            tmp.path().join("library"),
        )));
        let entry = favorite_video(&library, &source);
        let core = Arc::new(AppCore::new());
        core.save_project(Some(tmp.path().join("Reentrant.opentake")))
            .unwrap();
        let callback_core = Arc::clone(&core);
        let callback_library = Arc::clone(&library);
        let callbacks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let callback_count = Arc::clone(&callbacks);
        core.subscribe(move |_| {
            let _snapshot = callback_core.runtime_snapshot();
            let _workflow = callback_library.lock_workflow();
            callback_count.fetch_add(1, Ordering::SeqCst);
        });

        library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
            .expect("deferred event subscribers may re-enter after workflow locks are released");

        assert_eq!(callbacks.load(Ordering::SeqCst), 2);
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_a_project_rebound_before_capability_acquisition_without_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"trusted library bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = favorite_video(&library, &source);
        let projects = tmp.path().join("projects");
        let retained_projects = tmp.path().join("projects-retained");
        let bundle = projects.join("Rebound.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let retained_manifest = std::fs::read(bundle.join("media.json")).unwrap();

        std::fs::rename(&projects, &retained_projects).unwrap();
        let replacement_bundle = projects.join("Rebound.opentake");
        std::fs::create_dir_all(&replacement_bundle).unwrap();

        let error =
            library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
                .expect_err("pre-acquisition project replacement must fail closed");

        assert!(error.contains("identity no longer matches"), "{error}");
        assert!(std::fs::read_dir(&replacement_bundle)
            .unwrap()
            .next()
            .is_none());
        assert_eq!(
            std::fs::read(
                retained_projects
                    .join("Rebound.opentake")
                    .join("media.json")
            )
            .unwrap(),
            retained_manifest
        );
        assert!(core.media().entries.is_empty());
    }

    #[test]
    fn failed_capability_manifest_publish_preserves_disk_and_live_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"trusted library bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = favorite_video(&library, &source);
        let bundle = tmp.path().join("FailedPublish.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let before = core.media();
        let manifest_path = bundle.join("media.json");
        let bytes_before = std::fs::read(&manifest_path).unwrap();

        opentake_media::library::fail_next_atomic_capability_replace_for_test();
        library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
            .expect_err("injected manifest publication failure");

        assert_eq!(core.media(), before);
        assert_eq!(std::fs::read(manifest_path).unwrap(), bytes_before);
        let media_dir = opentake_project::layout::media_dir(&bundle);
        let leaked_candidate = std::fs::read_dir(media_dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .any(|item| item.file_name().to_string_lossy().starts_with("library-"));
        assert!(
            !leaked_candidate,
            "a precommit manifest failure must remove its zero-byte import candidate"
        );
    }

    #[test]
    fn postcommit_stored_cleanup_failure_still_persists_project_mapping_removal() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"trusted library bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = favorite_video(&library, &source);
        let bundle = tmp.path().join("CleanupFailure.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let imported =
            library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
                .unwrap();

        opentake_media::library::fail_next_removed_stored_cleanup_for_test();
        assert!(remove_from_library_and_project(&core, &library, &entry.id).unwrap());

        assert!(library.store().unwrap().entries().unwrap().is_empty());
        assert_eq!(core.media().library_favorite_id(&imported.id), None);
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        assert_eq!(reopened.media().library_favorite_id(&imported.id), None);
        assert!(!remove_from_library_and_project(&reopened, &library, &entry.id).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn retained_file_probe_ignores_a_temporary_ambient_project_rebind() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("trusted.mp4");
        let replacement_video = tmp.path().join("replacement.mp4");
        if !generate_video(&source, "32x18") || !generate_video(&replacement_video, "64x36") {
            return;
        }
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = favorite_video(&library, &source);
        let projects = tmp.path().join("projects");
        let retained_projects = tmp.path().join("projects-retained");
        let replacement_projects = tmp.path().join("projects-replacement");
        let bundle = projects.join("ProbeAba.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle)).unwrap();
        let mut events = DeferredCoreEvents::default();

        let imported = library_import_to_project_with_hook(
            &core,
            &engine_for(tmp.path()),
            &library,
            &entry.id,
            &mut events,
            |phase, path| match phase {
                ImportHookPhase::BeforeProbe => {
                    std::fs::rename(&projects, &retained_projects).unwrap();
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::copy(&replacement_video, path).unwrap();
                }
                ImportHookPhase::AfterProbe => {
                    std::fs::rename(&projects, &replacement_projects).unwrap();
                    std::fs::rename(&retained_projects, &projects).unwrap();
                }
                _ => {}
            },
        )
        .expect("temporary ambient rebind cannot redirect retained-file probe");
        let imported_entry = core
            .media()
            .entries
            .into_iter()
            .find(|candidate| candidate.id == imported.id)
            .unwrap();

        assert_eq!(imported_entry.source_width, Some(32));
        assert_eq!(imported_entry.source_height, Some(18));
        assert!(
            std::fs::metadata(replacement_projects.join("ProbeAba.opentake/media"))
                .unwrap()
                .is_dir()
        );
    }

    #[cfg(unix)]
    #[test]
    fn capability_manifest_publish_ignores_a_temporary_ambient_project_rebind() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"trusted library bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = favorite_video(&library, &source);
        let projects = tmp.path().join("projects");
        let retained_projects = tmp.path().join("projects-retained");
        let replacement_projects = tmp.path().join("projects-replacement");
        let bundle = projects.join("ManifestAba.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let mut events = DeferredCoreEvents::default();

        let imported = library_import_to_project_with_hook(
            &core,
            &engine_for(tmp.path()),
            &library,
            &entry.id,
            &mut events,
            |phase, _| match phase {
                ImportHookPhase::BeforeManifestWrite => {
                    std::fs::rename(&projects, &retained_projects).unwrap();
                    let replacement = projects.join("ManifestAba.opentake");
                    std::fs::create_dir_all(&replacement).unwrap();
                    std::fs::write(replacement.join("media.json"), b"replacement marker").unwrap();
                }
                ImportHookPhase::AfterManifestWrite => {
                    std::fs::rename(&projects, &replacement_projects).unwrap();
                    std::fs::rename(&retained_projects, &projects).unwrap();
                }
                _ => {}
            },
        )
        .expect("retained project root must be the only manifest authority");

        assert_eq!(
            std::fs::read(
                replacement_projects
                    .join("ManifestAba.opentake")
                    .join("media.json")
            )
            .unwrap(),
            b"replacement marker"
        );
        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(
            reopened.media().library_favorite_id(&imported.id),
            Some(entry.id.as_str())
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
            |phase, path| {
                if phase != ImportHookPhase::AfterProjectCommit {
                    return;
                }
                let moved = path.with_extension("moved");
                std::fs::rename(path, &moved).unwrap();
                std::fs::write(path, b"replacement bytes").unwrap();
                *moved_path.lock().unwrap() = Some(moved);
                let replacement_core = replacement_core.clone();
                let done_tx = done_tx.clone();
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

    #[cfg(unix)]
    #[test]
    fn postcondition_rollback_does_not_erase_an_intervening_unrelated_import() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        let unrelated = tmp.path().join("unrelated.mp4");
        std::fs::write(&source, b"trusted library bytes").unwrap();
        std::fs::write(&unrelated, b"unrelated media bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = favorite_video(&library, &source);
        let bundle = tmp.path().join("ConcurrentRollback.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let worker_core = core.clone();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker = std::sync::Mutex::new(None);
        let mut events = DeferredCoreEvents::default();

        library_import_to_project_with_hook(
            &core,
            &engine_for(tmp.path()),
            &library,
            &entry.id,
            &mut events,
            |phase, path| {
                if phase != ImportHookPhase::AfterProjectCommit {
                    return;
                }
                let moved = path.with_extension("moved");
                std::fs::rename(path, moved).unwrap();
                std::fs::write(path, b"replacement bytes").unwrap();
                let unrelated = unrelated.clone();
                let done_tx = done_tx.clone();
                let worker_core = worker_core.clone();
                *worker.lock().unwrap() = Some(std::thread::spawn(move || {
                    let imported = worker_core
                        .import_media_file(&unrelated, "unrelated", &ProbedMedia::default())
                        .unwrap();
                    worker_core.save_project(None).unwrap();
                    done_tx.send(imported.id).unwrap();
                }));
                assert!(done_rx
                    .recv_timeout(std::time::Duration::from_millis(50))
                    .is_err());
            },
        )
        .expect_err("leaf replacement must roll back the library import");

        let unrelated_id = done_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("ordinary import proceeds after the core transaction releases");
        worker.lock().unwrap().take().unwrap().join().unwrap();
        assert!(core
            .media()
            .entries
            .iter()
            .any(|candidate| candidate.id == unrelated_id));
        assert!(core
            .media()
            .favorite_library_ids
            .values()
            .all(|library_id| library_id != &entry.id));
        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert!(reopened
            .media()
            .entries
            .iter()
            .any(|candidate| candidate.id == unrelated_id));
    }

    #[cfg(unix)]
    #[test]
    fn failed_postcommit_rollback_reports_the_durable_import_as_committed() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"trusted library bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = favorite_video(&library, &source);
        let projects = tmp.path().join("projects");
        let retained_projects = tmp.path().join("projects-retained");
        let replacement_projects = tmp.path().join("projects-replacement");
        let bundle = projects.join("RollbackFailure.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let mut events = DeferredCoreEvents::default();

        let committed = library_import_to_project_with_hook(
            &core,
            &engine_for(tmp.path()),
            &library,
            &entry.id,
            &mut events,
            |phase, _| {
                if phase == ImportHookPhase::AfterProjectCommit {
                    std::fs::rename(&projects, &retained_projects).unwrap();
                    std::fs::create_dir_all(projects.join("RollbackFailure.opentake")).unwrap();
                    opentake_media::library::fail_next_atomic_capability_replace_for_test();
                }
            },
        )
        .expect("failed rollback leaves the first commit authoritative");

        let warning = committed
            .warning
            .as_ref()
            .expect("rollback failure must be surfaced to the caller");
        let LibraryImportWarningDto::PostconditionRollbackFailed {
            postcondition,
            rollback,
        } = warning;
        assert!(
            postcondition.contains("identity changed"),
            "{postcondition}"
        );
        assert!(rollback.contains("injected"), "{rollback}");
        let warning_json = serde_json::to_value(warning).unwrap();
        assert_eq!(warning_json["kind"], "postconditionRollbackFailed");
        std::fs::rename(&projects, &replacement_projects).unwrap();
        std::fs::rename(&retained_projects, &projects).unwrap();
        let imported = core
            .media()
            .entries
            .iter()
            .find(|candidate| candidate.id == committed.id)
            .expect("live import remains committed")
            .clone();
        let relative = match &imported.source {
            opentake_domain::MediaSource::Project { relative_path } => relative_path,
            other => panic!("expected project media, got {other:?}"),
        };
        assert!(std::fs::metadata(bundle.join(relative)).unwrap().len() > 0);
        let reopened = AppCore::new();
        reopened.open_project(bundle).unwrap();
        assert_eq!(reopened.media(), core.media());
        assert_eq!(
            reopened.media().library_favorite_id(&imported.id),
            Some(entry.id.as_str())
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_media_capability_rejects_a_symlink_to_the_original_leaf() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let bundle = tmp.path().join("SymlinkLeaf.opentake");
        std::fs::create_dir(&bundle).unwrap();
        let capability = ProjectMediaCapability::open(&bundle, true).unwrap();
        let mut handle = capability.create_leaf(Path::new("leaf.mp4")).unwrap();
        handle.as_file_mut().write_all(b"trusted leaf").unwrap();
        let path = capability.absolute_path("leaf.mp4");
        let moved = capability.absolute_path("moved.mp4");
        std::fs::rename(&path, &moved).unwrap();
        symlink(&moved, &path).unwrap();

        assert!(!matches!(
            capability.matches_handle(std::ffi::OsStr::new("leaf.mp4"), &handle),
            Ok(true)
        ));
        assert_eq!(std::fs::read(moved).unwrap(), b"trusted leaf");
    }

    #[cfg(unix)]
    #[test]
    fn project_media_capability_rejects_an_ambient_parent_replacement() {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("projects");
        let retained_projects = tmp.path().join("projects-retained");
        let bundle = projects.join("ParentSwap.opentake");
        std::fs::create_dir_all(&bundle).unwrap();
        let capability = ProjectMediaCapability::open(&bundle, true).unwrap();

        std::fs::rename(&projects, &retained_projects).unwrap();
        std::fs::create_dir_all(projects.join("ParentSwap.opentake/media")).unwrap();

        assert!(!capability.matches_namespace().unwrap());
        assert!(retained_projects.join("ParentSwap.opentake/media").is_dir());
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
    fn library_import_type_comes_from_manifest_metadata_not_stored_leaf_name() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"video bytes with renamed leaf").unwrap();
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
        let stored = library
            .store()
            .unwrap()
            .stored_path(&entry.id)
            .unwrap()
            .unwrap();
        let renamed = stored.with_extension("wav");
        std::fs::rename(stored, &renamed).unwrap();
        let bundle = tmp.path().join("RenamedLeaf.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle)).unwrap();

        let imported =
            library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
                .expect("stored leaf name is not type authority");
        let manifest_entry = core
            .media()
            .entries
            .into_iter()
            .find(|item| item.id == imported.id)
            .unwrap();

        assert_eq!(manifest_entry.kind, opentake_domain::ClipType::Video);
        match manifest_entry.source {
            opentake_domain::MediaSource::Project { relative_path } => {
                assert!(relative_path.ends_with(".mp4"), "{relative_path}");
            }
            other => panic!("expected project source, got {other:?}"),
        }
        assert_eq!(
            std::fs::read(imported.path).unwrap(),
            b"video bytes with renamed leaf"
        );
    }

    #[test]
    fn idempotent_import_rejects_a_wrong_typed_existing_mapping() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"trusted video bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = favorite_video(&library, &source);
        let bundle = tmp.path().join("WrongKind.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let imported =
            library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
                .unwrap();
        let mut tampered = opentake_project::Project::open(&bundle).unwrap();
        tampered
            .manifest
            .entries
            .iter_mut()
            .find(|candidate| candidate.id == imported.id)
            .unwrap()
            .kind = ClipType::Audio;
        tampered.save_manifest().unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();

        let error =
            library_import_to_project_impl(&reopened, &engine_for(tmp.path()), &library, &entry.id)
                .expect_err("wrong-typed idempotent mapping must fail closed");

        assert!(error.contains("unexpected media type"), "{error}");
        assert_eq!(reopened.media().entries.len(), 1);
    }

    #[test]
    fn idempotent_import_rejects_a_wrong_extension_existing_mapping() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"trusted video bytes").unwrap();
        let library = LibraryState::new(LibraryStore::new(tmp.path().join("library")));
        let entry = favorite_video(&library, &source);
        let bundle = tmp.path().join("WrongExtension.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let imported =
            library_import_to_project_impl(&core, &engine_for(tmp.path()), &library, &entry.id)
                .unwrap();
        let mut tampered = opentake_project::Project::open(&bundle).unwrap();
        let manifest_entry = tampered
            .manifest
            .entries
            .iter_mut()
            .find(|candidate| candidate.id == imported.id)
            .unwrap();
        let relative = match &manifest_entry.source {
            opentake_domain::MediaSource::Project { relative_path } => PathBuf::from(relative_path),
            other => panic!("expected project media, got {other:?}"),
        };
        let wrong_relative = relative.with_extension("wav");
        std::fs::rename(bundle.join(&relative), bundle.join(&wrong_relative)).unwrap();
        manifest_entry.source = opentake_domain::MediaSource::Project {
            relative_path: wrong_relative.to_string_lossy().into_owned(),
        };
        tampered.save_manifest().unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();

        let error =
            library_import_to_project_impl(&reopened, &engine_for(tmp.path()), &library, &entry.id)
                .expect_err("wrong-extension idempotent mapping must fail closed");

        assert!(error.contains("unexpected media extension"), "{error}");
        assert_eq!(reopened.media().entries.len(), 1);
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
