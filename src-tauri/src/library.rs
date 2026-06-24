//! Global asset library command surface (#55, part of #37 "全局可复用素材库").
//!
//! These commands sit on top of [`opentake_media::library::LibraryStore`] (#54),
//! a cross-project, copy-on-favorite store rooted at `<data dir>/OpenTake/Library`.
//! The store owns all persistence (atomic manifest, content-addressed files,
//! in-process write lock); each command here is a thin shim that locks nothing of
//! its own, calls a store method, and maps the boundary `MediaError` to a
//! `String` so the WebView gets a plain rejected Promise (`AGENTS.md`: "边界层转
//! Tauri 的 `Err(String)`").
//!
//! `library_import_to_project` bridges the global library back into the *current*
//! project: it resolves the stored copy for an entry id, probes it via the media
//! engine, and appends it to the [`AppCore`] manifest with a fresh project asset
//! id (so the same favorite can be imported into many projects). It reuses the
//! [`crate::media::MediaState`] engine for probing rather than re-opening one.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use opentake_core::{importable_clip_type, AppCore, ProbedMedia};
use opentake_media::library::{FavoriteRequest, LibraryEntry, LibraryStore};

use crate::media::MediaState;

/// Managed-state wrapper over the global [`LibraryStore`]. The store is shared
/// across commands behind an `Arc` (it has no `Clone`); `Send + Sync` so it
/// lives in Tauri managed state.
pub struct LibraryState {
    store: Arc<LibraryStore>,
}

impl LibraryState {
    /// Wrap a store for managed state.
    pub fn new(store: LibraryStore) -> Self {
        LibraryState {
            store: Arc::new(store),
        }
    }

    /// The shared store handle.
    pub fn store(&self) -> &LibraryStore {
        &self.store
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
        }
    }
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
    let store = library.store();
    let entries = match category.as_deref() {
        None | Some("") => store.entries(),
        Some(c) => store.entries_in_category(Some(c)),
    }
    .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(LibraryEntryDto::from).collect())
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
    library
        .store()
        .favorite(&req)
        .map(LibraryEntryDto::from)
        .map_err(|e| e.to_string())
}

/// `library_unfavorite`: remove an entry (and its stored copy) by id. Returns
/// `true` if an entry was removed, `false` if the id was unknown (idempotent).
#[tauri::command]
pub fn library_unfavorite(library: State<'_, LibraryState>, id: String) -> Result<bool, String> {
    library.store().remove(&id).map_err(|e| e.to_string())
}

/// `library_categorize`: set (or clear, with `category = None`) the category of
/// one entry. Returns the updated entry, or an error if the id is unknown.
#[tauri::command]
pub fn library_categorize(
    library: State<'_, LibraryState>,
    id: String,
    category: Option<String>,
) -> Result<LibraryEntryDto, String> {
    library
        .store()
        .set_category(&id, category)
        .map_err(|e| e.to_string())?
        .map(LibraryEntryDto::from)
        .ok_or_else(|| format!("unknown library entry: {id}"))
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
    library
        .store()
        .rename_category(&from, to)
        .map_err(|e| e.to_string())
}

/// `library_delete`: alias of `library_unfavorite` for the front end's "delete
/// from library" affordance. Removes the entry and its stored copy by id;
/// returns `true` if something was removed.
#[tauri::command]
pub fn library_delete(library: State<'_, LibraryState>, id: String) -> Result<bool, String> {
    library.store().remove(&id).map_err(|e| e.to_string())
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
    let stored = library
        .store()
        .stored_path(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("library entry has no stored file: {id}"))?;
    if !stored.is_file() {
        return Err(format!(
            "library file missing on disk: {}",
            stored.display()
        ));
    }
    if importable_clip_type(&stored).is_none() {
        return Err(format!(
            "library file is not an importable media type: {}",
            stored.display()
        ));
    }

    let probe = probe_or_default(media.engine(), &stored);
    let name = display_name(&stored);
    let entry = core
        .import_media_file(&stored, name.clone(), &probe)
        .map_err(|e| e.to_string())?;

    Ok(LibraryImportDto {
        id: entry.id,
        name: entry.name,
        path: stored.to_string_lossy().into_owned(),
    })
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
