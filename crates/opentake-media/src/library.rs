//! Global asset library — a cross-project store of "favorited" media that lives
//! outside any single OpenTake project (issue #54, part of #37 "全局可复用素材库").
//!
//! Layout under the library root (resolved cross-platform via [`dirs`], e.g.
//! `~/Library/Application Support/OpenTake/Library/` on macOS):
//! ```text
//! <root>/
//!   library.json          manifest: { version, entries: [LibraryEntry, …] }
//!   files/<hash><ext>      copy-on-favorite content, content-addressed
//!   files/.staging/*       unpublished content, reclaimed on startup
//!   .library.json.*.tmp    per-transaction temp, atomically committed/cleaned
//! ```
//!
//! Design choices:
//! - **copy-on-favorite**: favoriting copies the source file *into* the library
//!   so it survives the original being moved/deleted.
//! - **hash dedup**: the in-library filename is the SHA-256 of the file content,
//!   so favoriting the same bytes twice stores one copy and reuses it.
//! - **atomic manifest**: the manifest is written to a temp file and renamed,
//!   so a crash mid-write never leaves a truncated `library.json`. An in-process
//!   `Mutex` serializes read-modify-write so concurrent favorites from worker
//!   threads do not lose entries.
//!
//! The store takes its root as an explicit path so it stays testable; the Tauri
//! command layer (#55) constructs it from `app_data_dir`. [`default_library_dir`]
//! provides the `dirs`-based production default.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cap_fs_ext::{ambient_authority, DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{MediaError, Result};

/// Manifest filename under the library root.
pub const MANIFEST_NAME: &str = "library.json";
/// Subdirectory holding the content-addressed copies.
pub const FILES_SUBDIR: &str = "files";
/// Hidden directory for favorite content that is not yet visible in the
/// library manifest.
const STAGING_SUBDIR: &str = ".staging";
/// On-disk manifest schema version (bumped on incompatible changes).
pub const MANIFEST_VERSION: u32 = 1;
/// Application directory name under the platform data dir.
const APP_DIR: &str = "OpenTake";
/// Library directory name under the application directory.
const LIBRARY_DIR: &str = "Library";

#[cfg(test)]
std::thread_local! {
    static STORED_INDEX_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// One favorited asset in the global library.
///
/// JSON is camelCase (`favoritedAt`) to match the frontend DTO (#37-B/#37-C).
/// Every field carries `#[serde(default)]` so older/partial manifests still load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    /// Content hash (SHA-256 hex) of the stored file — the library-internal id.
    #[serde(default)]
    pub id: String,
    /// Asset kind, e.g. `"video"`, `"audio"`, `"image"`. `type` in JSON.
    #[serde(default, rename = "type")]
    pub kind: String,
    /// Optional user category/tag for filtering; `None` when uncategorized.
    #[serde(default)]
    pub category: Option<String>,
    /// Unix epoch seconds when the asset was favorited.
    #[serde(default)]
    pub favorited_at: f64,
    /// Original source path the file was copied from (for display/back-ref).
    #[serde(default)]
    pub source: Option<String>,
    /// Optional thumbnail reference (path or data URI), filled by upper layers.
    #[serde(default)]
    pub thumb: Option<String>,
}

/// The persisted manifest: a version tag plus the entry list.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    entries: Vec<LibraryEntry>,
}

/// Describes a file to favorite into the library. The id/stored path are derived
/// from the file content, so the caller only supplies the source and metadata.
#[derive(Clone, Debug)]
pub struct FavoriteRequest<'a> {
    /// Path to the source file to copy in.
    pub source: &'a Path,
    /// Asset kind (`"video"` / `"audio"` / `"image"` / …).
    pub kind: &'a str,
    /// Optional category/tag.
    pub category: Option<String>,
    /// Unix epoch seconds to record; the command layer passes the real clock.
    pub favorited_at: f64,
    /// Optional thumbnail reference.
    pub thumb: Option<String>,
}

/// Result of one favorite transaction. `created` reports whether publication
/// appended the entry to the manifest; callers must not use it as ownership
/// proof for a later compensating delete.
#[derive(Clone, Debug, PartialEq)]
pub struct FavoriteOutcome {
    /// The entry selected by the actual source bytes read by this transaction.
    pub entry: LibraryEntry,
    /// `true` only when this transaction appended `entry` to the manifest.
    pub created: bool,
}

/// A favorite prepared from one immutable source-byte snapshot. A new entry's
/// content stays under the hidden staging directory until [`LibraryStore`] is
/// explicitly asked to publish it. Dropping an unpublished preparation removes
/// its private stage best-effort; even if cleanup fails, it is never in Mine.
pub struct PreparedFavorite {
    entry: LibraryEntry,
    capabilities: Arc<LibraryCapabilities>,
    active_stages: Arc<Mutex<HashSet<OsString>>>,
    staged_name: Option<OsString>,
    stored_name: Option<OsString>,
    owns_stored_path: bool,
}

impl PreparedFavorite {
    /// Entry identity derived from the exact source bytes read during prepare.
    pub fn entry(&self) -> &LibraryEntry {
        &self.entry
    }

    /// Whether publication is still required after the project mapping saves.
    pub fn needs_publish(&self) -> bool {
        self.staged_name.is_some()
    }

    fn release_stage(&mut self) {
        let Some(name) = self.staged_name.take() else {
            return;
        };
        let _ = self.capabilities.staging.remove_file_or_symlink(&name);
        self.active_stages
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&name);
    }
}

impl Drop for PreparedFavorite {
    fn drop(&mut self) {
        self.release_stage();
        if self.owns_stored_path {
            if let Some(name) = self.stored_name.take() {
                let _ = self.capabilities.files.remove_file_or_symlink(name);
            }
            self.owns_stored_path = false;
        }
    }
}

struct LibraryCapabilities {
    root: Dir,
    files: Dir,
    staging: Dir,
}

/// The global library store, rooted at a directory. Cloneable handles are not
/// provided; share one instance behind an `Arc` if multiple owners are needed.
pub struct LibraryStore {
    root: PathBuf,
    capabilities: std::result::Result<Arc<LibraryCapabilities>, String>,
    active_stages: Arc<Mutex<HashSet<OsString>>>,
    /// Serializes manifest read-modify-write across in-process threads.
    write_lock: Mutex<()>,
}

/// Cross-platform default library directory:
/// `<platform data dir>/OpenTake/Library`. Returns `None` only if the platform
/// data directory cannot be resolved (handled as an error by callers).
pub fn default_library_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(APP_DIR).join(LIBRARY_DIR))
}

fn open_capabilities(root_path: &Path) -> std::io::Result<LibraryCapabilities> {
    let parent_path = root_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "library root must have a parent directory",
        )
    })?;
    let root_name = root_path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "library root must have a final component",
        )
    })?;
    Dir::create_ambient_dir_all(parent_path, ambient_authority())?;
    let parent = Dir::open_ambient_dir(parent_path, ambient_authority())?;
    ensure_child_dir(&parent, root_name)?;
    let root = parent.open_dir_nofollow(root_name)?;
    ensure_child_dir(&root, FILES_SUBDIR)?;
    let files = root.open_dir_nofollow(FILES_SUBDIR)?;
    ensure_child_dir(&files, STAGING_SUBDIR)?;
    let staging = files.open_dir_nofollow(STAGING_SUBDIR)?;
    Ok(LibraryCapabilities {
        root,
        files,
        staging,
    })
}

fn ensure_child_dir(parent: &Dir, name: impl AsRef<Path>) -> std::io::Result<()> {
    match parent.create_dir(name.as_ref()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

fn read_nofollow(dir: &Dir, name: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir.open_with(name, &options)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn write_new_nofollow(dir: &Dir, name: impl AsRef<Path>, bytes: &[u8]) -> std::io::Result<()> {
    let name = name.as_ref();
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let mut file = dir.open_with(name, &options)?;
    let result = file.write_all(bytes).and_then(|_| file.sync_all());
    if result.is_err() {
        drop(file);
        let _ = dir.remove_file_or_symlink(name);
    }
    result
}

fn copy_new_nofollow(
    source_dir: &Dir,
    source_name: impl AsRef<Path>,
    target_dir: &Dir,
    target_name: impl AsRef<Path>,
) -> std::io::Result<()> {
    let source_name = source_name.as_ref();
    let target_name = target_name.as_ref();
    let mut read_options = OpenOptions::new();
    read_options.read(true).follow(FollowSymlinks::No);
    let mut source = source_dir.open_with(source_name, &read_options)?;
    let mut write_options = OpenOptions::new();
    write_options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let mut target = target_dir.open_with(target_name, &write_options)?;
    let result = std::io::copy(&mut source, &mut target).and_then(|_| target.sync_all());
    if result.is_err() {
        drop(target);
        let _ = target_dir.remove_file_or_symlink(target_name);
    }
    result
}

fn unique_manifest_artifact(suffix: &str) -> OsString {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    OsString::from(format!(
        ".{MANIFEST_NAME}.{}.{sequence:020}.{suffix}",
        std::process::id()
    ))
}

struct CapabilityArtifactGuard<'a> {
    dir: &'a Dir,
    name: OsString,
}

impl Drop for CapabilityArtifactGuard<'_> {
    fn drop(&mut self) {
        let _ = self.dir.remove_file_or_symlink(&self.name);
    }
}

fn is_manifest_artifact(name: &OsStr) -> bool {
    let text = name.to_string_lossy();
    text == format!("{MANIFEST_NAME}.tmp")
        || (text.starts_with(&format!(".{MANIFEST_NAME}."))
            && (text.ends_with(".tmp") || text.ends_with(".backup")))
}

fn is_manifest_backup(name: &OsStr) -> bool {
    let text = name.to_string_lossy();
    text.starts_with(&format!(".{MANIFEST_NAME}.")) && text.ends_with(".backup")
}

#[cfg(not(windows))]
fn commit_manifest_file(root: &Dir, tmp_name: &OsStr) -> std::io::Result<()> {
    // Unix renameat replaces the destination atomically within the retained
    // root directory capability.
    root.rename(tmp_name, root, MANIFEST_NAME)
}

#[cfg(windows)]
fn commit_manifest_file(root: &Dir, tmp_name: &OsStr) -> std::io::Result<()> {
    // std/cap-std rename does not replace an existing destination on Windows.
    // Preserve crash atomicity with a retained-root two-phase protocol: a crash
    // with the canonical name absent leaves an immutable backup that startup
    // reconciliation restores before accepting writes.
    match root.symlink_metadata(MANIFEST_NAME) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return root.rename(tmp_name, root, MANIFEST_NAME);
        }
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    let backup_name = unique_manifest_artifact("backup");
    root.rename(MANIFEST_NAME, root, &backup_name)?;
    if let Err(error) = root.rename(tmp_name, root, MANIFEST_NAME) {
        let restore = root.rename(&backup_name, root, MANIFEST_NAME);
        return match restore {
            Ok(()) => Err(error),
            Err(restore_error) => Err(std::io::Error::new(
                restore_error.kind(),
                format!(
                    "manifest commit failed ({error}); backup restore also failed ({restore_error})"
                ),
            )),
        };
    }
    let _ = root.remove_file_or_symlink(backup_name);
    Ok(())
}

impl LibraryStore {
    /// Open (or lazily create) a store rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let capabilities = open_capabilities(&root)
            .map(Arc::new)
            .map_err(|error| format!("could not secure global library root: {error}"));
        LibraryStore {
            root,
            capabilities,
            active_stages: Arc::new(Mutex::new(HashSet::new())),
            write_lock: Mutex::new(()),
        }
    }

    /// Open a store at the platform-default library directory.
    pub fn open_default() -> Result<Self> {
        let root = default_library_dir().ok_or_else(|| {
            MediaError::Other(anyhow::anyhow!("could not resolve platform data directory"))
        })?;
        let store = LibraryStore::new(root);
        store.reconcile_storage()?;
        Ok(store)
    }

    /// The library root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn capabilities(&self) -> Result<&Arc<LibraryCapabilities>> {
        self.capabilities
            .as_ref()
            .map_err(|message| MediaError::Other(anyhow::anyhow!(message.clone())))
    }

    fn files_dir(&self) -> PathBuf {
        self.root.join(FILES_SUBDIR)
    }

    fn manifest_backups(&self) -> Result<Vec<OsString>> {
        let root = &self.capabilities()?.root;
        let mut backups = Vec::new();
        for entry in root.entries()? {
            let entry = entry?;
            let name = entry.file_name();
            if !is_manifest_backup(&name) {
                continue;
            }
            let file_type = entry.file_type()?;
            if !file_type.is_file() || file_type.is_symlink() {
                return Err(MediaError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "library manifest backup is not a nofollow regular file",
                )));
            }
            backups.push(name);
        }
        backups.sort();
        Ok(backups)
    }

    fn latest_valid_manifest_backup(&self) -> Result<Option<(OsString, Manifest)>> {
        let root = &self.capabilities()?.root;
        let mut latest_error = None;
        for backup in self.manifest_backups()?.into_iter().rev() {
            let bytes = read_nofollow(root, &backup)?;
            match serde_json::from_slice(&bytes) {
                Ok(manifest) => return Ok(Some((backup, manifest))),
                Err(error) if latest_error.is_none() => latest_error = Some(error),
                Err(_) => {}
            }
        }
        match latest_error {
            Some(error) => Err(MediaError::Json(error)),
            None => Ok(None),
        }
    }

    fn reconcile_manifest_artifacts(&self) -> Result<()> {
        let root = &self.capabilities()?.root;
        match root.symlink_metadata(MANIFEST_NAME) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                // A regular canonical leaf can still be corrupt. Validate it
                // before removing any crash-recovery backup.
                let bytes = read_nofollow(root, MANIFEST_NAME)?;
                let _: Manifest = serde_json::from_slice(&bytes)?;
            }
            Ok(_) => {
                return Err(MediaError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "library manifest is not a nofollow regular file",
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Some((backup, _manifest)) = self.latest_valid_manifest_backup()? {
                    root.rename(&backup, root, MANIFEST_NAME)?;
                }
            }
            Err(error) => return Err(MediaError::Io(error)),
        }
        for entry in root.entries()? {
            let entry = entry?;
            let name = entry.file_name();
            if !is_manifest_artifact(&name) {
                continue;
            }
            let file_type = entry.file_type()?;
            if file_type.is_dir() && !file_type.is_symlink() {
                return Err(MediaError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "library manifest artifact is an unexpected directory",
                )));
            }
            root.remove_file_or_symlink(name)?;
        }
        Ok(())
    }

    fn stored_name(id: &str, source: &Path) -> OsString {
        let safe_extension = source.extension().and_then(OsStr::to_str).filter(|value| {
            !value.is_empty()
                && value.len() <= 16
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });
        match safe_extension {
            Some(extension) => OsString::from(format!("{id}.{extension}")),
            None => OsString::from(id),
        }
    }

    fn staging_name(id: &str, source: &Path) -> OsString {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let file_name = Self::stored_name(id, source).to_string_lossy().into_owned();
        OsString::from(format!(
            "{file_name}.{}.{}.pending",
            std::process::id(),
            sequence
        ))
    }

    fn stored_index(&self) -> Result<HashMap<String, OsString>> {
        #[cfg(test)]
        STORED_INDEX_SCANS.with(|count| count.set(count.get() + 1));
        let capabilities = self.capabilities()?;
        let mut index = HashMap::new();
        for entry in capabilities.files.entries()? {
            let entry = entry?;
            if entry.file_name() == STAGING_SUBDIR {
                continue;
            }
            let file_type = entry.file_type()?;
            if !file_type.is_file() || file_type.is_symlink() {
                return Err(MediaError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "library content path is not a nofollow regular file",
                )));
            }
            let name = entry.file_name();
            let path = Path::new(&name);
            let Some(id) = path.file_stem().and_then(OsStr::to_str).map(str::to_owned) else {
                continue;
            };
            if index.insert(id.clone(), name).is_some() {
                return Err(MediaError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("multiple stored copies claim library id {id}"),
                )));
            }
        }
        Ok(index)
    }

    /// Remove content left without manifest ownership by a prior crash. A
    /// readable manifest is required before cleanup, and files whose content id
    /// is still referenced are never removed.
    pub fn reconcile_storage(&self) -> Result<()> {
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.reconcile_manifest_artifacts()?;
        let manifest = self.load_manifest()?;
        let valid_ids: HashSet<&str> = manifest
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        let capabilities = self.capabilities()?;
        let active_stages = self
            .active_stages
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        for entry in capabilities.staging.entries()? {
            let entry = entry?;
            let name = entry.file_name();
            if active_stages.contains(&name) {
                continue;
            }
            let file_type = entry.file_type()?;
            if file_type.is_dir() && !file_type.is_symlink() {
                return Err(MediaError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "library staging contains an unexpected directory",
                )));
            }
            capabilities.staging.remove_file_or_symlink(&name)?;
        }
        let mut seen_owned_ids = HashSet::new();
        for entry in capabilities.files.entries()? {
            let entry = entry?;
            if entry.file_name() == STAGING_SUBDIR {
                continue;
            }
            let file_type = entry.file_type()?;
            if !file_type.is_file() || file_type.is_symlink() {
                return Err(MediaError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "library content path is not a nofollow regular file",
                )));
            }
            let name = entry.file_name();
            let content_id = Path::new(&name).file_stem().and_then(OsStr::to_str);
            let is_manifest_owned = content_id.is_some_and(|id| valid_ids.contains(id));
            if let Some(id) = content_id.filter(|id| valid_ids.contains(*id)) {
                if !seen_owned_ids.insert(id.to_string()) {
                    return Err(MediaError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("multiple stored copies claim library id {id}"),
                    )));
                }
            }
            if !is_manifest_owned {
                entry.remove_file()?;
            }
        }
        Ok(())
    }

    /// Read the manifest, returning an empty one if it does not exist yet.
    fn load_manifest(&self) -> Result<Manifest> {
        let root = &self.capabilities()?.root;
        match read_nofollow(root, MANIFEST_NAME) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Some((_backup, manifest)) = self.latest_valid_manifest_backup()? {
                    Ok(manifest)
                } else {
                    Ok(Manifest {
                        version: MANIFEST_VERSION,
                        entries: Vec::new(),
                    })
                }
            }
            Err(e) => Err(MediaError::Io(e)),
        }
    }

    /// Atomically persist the manifest: write a temp file, then rename over the
    /// real path. The rename is atomic on the same filesystem.
    fn store_manifest(&self, manifest: &Manifest) -> Result<()> {
        let root = &self.capabilities()?.root;
        let bytes = serde_json::to_vec_pretty(manifest)?;
        let tmp_name = unique_manifest_artifact("tmp");
        let _tmp_cleanup = CapabilityArtifactGuard {
            dir: root,
            name: tmp_name.clone(),
        };
        write_new_nofollow(root, &tmp_name, &bytes)?;
        commit_manifest_file(root, &tmp_name)?;
        // The canonical manifest is committed at this point. Artifact cleanup
        // is maintenance only; surfacing its failure would make RAII delete a
        // final copy that the committed manifest now owns.
        let _ = self.reconcile_manifest_artifacts();
        Ok(())
    }

    /// All entries currently in the library (manifest order).
    pub fn entries(&self) -> Result<Vec<LibraryEntry>> {
        Ok(self.load_manifest()?.entries)
    }

    /// Entries filtered by `category`. `Some(c)` keeps entries whose category
    /// equals `c`; `None` keeps only uncategorized entries.
    pub fn entries_in_category(&self, category: Option<&str>) -> Result<Vec<LibraryEntry>> {
        let want = category.map(|c| c.to_string());
        Ok(self
            .load_manifest()?
            .entries
            .into_iter()
            .filter(|e| e.category == want)
            .collect())
    }

    /// Whether an entry with this content id already exists.
    pub fn contains(&self, id: &str) -> Result<bool> {
        Ok(self.load_manifest()?.entries.iter().any(|e| e.id == id))
    }

    /// Compute the content-addressed id without changing the library. Used to
    /// migrate legacy project favorites that predate persisted library ids.
    pub fn content_id(&self, path: impl AsRef<Path>) -> Result<String> {
        Ok(hash_hex(&std::fs::read(path)?))
    }

    fn store_copy(&self, id: &str, source: &Path, bytes: &[u8]) -> Result<()> {
        let capabilities = self.capabilities()?;
        let stored = Self::stored_name(id, source);
        if self.stored_index()?.contains_key(id) {
            return Ok(());
        }
        match write_new_nofollow(&capabilities.files, &stored, bytes) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let index = self.stored_index()?;
                if !index.contains_key(id) {
                    return Err(MediaError::Io(error));
                }
            }
            Err(error) => return Err(MediaError::Io(error)),
        }
        Ok(())
    }

    /// Favorite a file: copy its bytes into the library (dedup by content hash)
    /// and record an entry. If the same content is already favorited, the
    /// existing entry is returned unchanged and no duplicate file is written.
    ///
    /// Preparation and publication each run under the in-process write lock;
    /// publication re-reads the manifest so concurrent favorites cannot clobber
    /// each other or create duplicate entries.
    pub fn favorite(&self, req: &FavoriteRequest<'_>) -> Result<LibraryEntry> {
        Ok(self.favorite_with_outcome(req)?.entry)
    }

    /// Favorite a file and report whether this transaction created its manifest
    /// entry. The source is read exactly once; ownership is never inferred from
    /// a separate preflight hash that could observe different bytes.
    pub fn favorite_with_outcome(&self, req: &FavoriteRequest<'_>) -> Result<FavoriteOutcome> {
        let prepared = self.prepare_favorite(req)?;
        self.publish_favorite(prepared)
    }

    /// Read/hash one source snapshot and prepare a favorite without publishing a
    /// new manifest entry. Existing entries are returned without staging; new
    /// content is durable but hidden until [`Self::publish_favorite`].
    pub fn prepare_favorite(&self, req: &FavoriteRequest<'_>) -> Result<PreparedFavorite> {
        let bytes = std::fs::read(req.source)?;
        let id = hash_hex(&bytes);

        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let manifest = self.load_manifest()?;

        if let Some(existing) = manifest.entries.iter().find(|e| e.id == id).cloned() {
            if self.stored_path(&id)?.is_none() {
                self.store_copy(&id, req.source, &bytes)?;
            }
            return Ok(PreparedFavorite {
                entry: existing,
                capabilities: Arc::clone(self.capabilities()?),
                active_stages: Arc::clone(&self.active_stages),
                staged_name: None,
                stored_name: None,
                owns_stored_path: false,
            });
        }

        let staged_name = Self::staging_name(&id, req.source);
        let stored_name = Self::stored_name(&id, req.source);
        let entry = LibraryEntry {
            id,
            kind: req.kind.to_string(),
            category: req.category.clone(),
            favorited_at: req.favorited_at,
            source: req.source.to_str().map(|s| s.to_string()),
            thumb: req.thumb.clone(),
        };
        let capabilities = Arc::clone(self.capabilities()?);
        self.active_stages
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(staged_name.clone());
        if let Err(error) = write_new_nofollow(&capabilities.staging, &staged_name, &bytes) {
            self.active_stages
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&staged_name);
            let _ = capabilities.staging.remove_file_or_symlink(&staged_name);
            return Err(MediaError::Io(error));
        }
        Ok(PreparedFavorite {
            entry,
            capabilities,
            active_stages: Arc::clone(&self.active_stages),
            staged_name: Some(staged_name),
            stored_name: Some(stored_name),
            owns_stored_path: false,
        })
    }

    /// Publish a prepared favorite after its project mapping is durable. The
    /// manifest is re-read under the write lock so concurrent prepares dedup at
    /// publication time. RAII owns the staged or final copy until the manifest
    /// commit succeeds; startup reconciliation removes crash-window leftovers.
    pub fn publish_favorite(&self, mut prepared: PreparedFavorite) -> Result<FavoriteOutcome> {
        if !Arc::ptr_eq(self.capabilities()?, &prepared.capabilities) {
            return Err(MediaError::Other(anyhow::anyhow!(
                "favorite preparation belongs to a different library capability"
            )));
        }
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut manifest = self.load_manifest()?;
        manifest.version = MANIFEST_VERSION;
        if let Some(existing) = manifest
            .entries
            .iter()
            .find(|entry| entry.id == prepared.entry.id)
            .cloned()
        {
            if self.stored_path(&existing.id)?.is_none() {
                let staged_name = prepared.staged_name.as_ref().ok_or_else(|| {
                    MediaError::Other(anyhow::anyhow!(
                        "existing library entry has no durable copy"
                    ))
                })?;
                let stored_name = prepared.stored_name.as_ref().ok_or_else(|| {
                    MediaError::Other(anyhow::anyhow!("favorite preparation has no target"))
                })?;
                match copy_new_nofollow(
                    &prepared.capabilities.staging,
                    staged_name,
                    &prepared.capabilities.files,
                    stored_name,
                ) {
                    Ok(()) => {
                        prepared.release_stage();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(MediaError::Io(error)),
                }
            }
            return Ok(FavoriteOutcome {
                entry: existing,
                created: false,
            });
        }

        let staged_name = prepared.staged_name.as_ref().ok_or_else(|| {
            MediaError::Other(anyhow::anyhow!(
                "favorite preparation was already published"
            ))
        })?;
        let stored_name = prepared.stored_name.as_ref().ok_or_else(|| {
            MediaError::Other(anyhow::anyhow!("favorite preparation has no target"))
        })?;
        copy_new_nofollow(
            &prepared.capabilities.staging,
            staged_name,
            &prepared.capabilities.files,
            stored_name,
        )?;
        prepared.owns_stored_path = true;
        prepared.release_stage();

        manifest.entries.push(prepared.entry.clone());
        self.store_manifest(&manifest)?;
        prepared.owns_stored_path = false;
        prepared.stored_name = None;
        Ok(FavoriteOutcome {
            entry: prepared.entry.clone(),
            created: true,
        })
    }

    /// Restore a missing durable copy for an existing content id.
    ///
    /// The source is accepted only when its current bytes still hash to
    /// `expected_id`. Unlike [`Self::favorite`], this never creates a manifest
    /// entry, so a source that changed in place cannot create an orphan under a
    /// new id while leaving the project's original mapping unresolved.
    pub fn repair_stored_copy(&self, expected_id: &str, source: impl AsRef<Path>) -> Result<()> {
        let source = source.as_ref();
        let bytes = std::fs::read(source)?;
        let actual_id = hash_hex(&bytes);
        if actual_id != expected_id {
            return Err(MediaError::Other(anyhow::anyhow!(
                "source content changed: expected {expected_id}, got {actual_id}"
            )));
        }

        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !self
            .load_manifest()?
            .entries
            .iter()
            .any(|entry| entry.id == expected_id)
        {
            return Err(MediaError::Other(anyhow::anyhow!(
                "unknown library entry: {expected_id}"
            )));
        }
        if self.stored_path(expected_id)?.is_none() {
            self.store_copy(expected_id, source, &bytes)?;
        }
        Ok(())
    }

    /// Absolute path to the stored copy for an entry id, if present on disk.
    pub fn stored_path(&self, id: &str) -> Result<Option<PathBuf>> {
        Ok(self
            .stored_index()?
            .remove(id)
            .map(|name| self.files_dir().join(name)))
    }

    /// Validate and enumerate stored copies once, returning absolute display
    /// paths keyed by content id. Callers rendering or reconciling many entries
    /// should use this batch form instead of rescanning for each id.
    pub fn stored_paths(&self) -> Result<HashMap<String, PathBuf>> {
        Ok(self
            .stored_index()?
            .into_iter()
            .map(|(id, name)| (id, self.files_dir().join(name)))
            .collect())
    }

    /// Remove an entry from the manifest and delete its stored copy. Returns
    /// `true` if an entry was removed. Runs under the write lock.
    pub fn remove(&self, id: &str) -> Result<bool> {
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let mut manifest = self.load_manifest()?;
        let before = manifest.entries.len();
        manifest.entries.retain(|e| e.id != id);
        if manifest.entries.len() == before {
            return Ok(false);
        }
        manifest.version = MANIFEST_VERSION;
        let stored_name = self.stored_index()?.remove(id);
        self.store_manifest(&manifest)?;
        // Commit the manifest first. A failed manifest write must not leave an
        // entry that still exists on disk pointing at a copy we already deleted.
        // A failed best-effort cleanup after the commit only leaves an orphaned
        // content-addressed file, which is safe and can be reclaimed later.
        if let Some(name) = stored_name {
            let _ = self.capabilities()?.files.remove_file_or_symlink(name);
        }
        Ok(true)
    }

    /// Set (or clear, with `None`) the category of the entry with `id`. Returns
    /// the updated entry, or `None` if no entry has that id. Runs under the write
    /// lock so it cannot race a concurrent favorite/remove. Used by the command
    /// layer's `library_categorize` (#55).
    pub fn set_category(&self, id: &str, category: Option<String>) -> Result<Option<LibraryEntry>> {
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let mut manifest = self.load_manifest()?;
        let Some(entry) = manifest.entries.iter_mut().find(|e| e.id == id) else {
            return Ok(None);
        };
        entry.category = category;
        let updated = entry.clone();
        manifest.version = MANIFEST_VERSION;
        self.store_manifest(&manifest)?;
        Ok(Some(updated))
    }

    /// Rename a category: move every entry whose category equals `from` to `to`
    /// (`None` un-categorizes them). Returns the number of entries changed. Runs
    /// under the write lock. Used by the command layer's `library_rename` (#55).
    pub fn rename_category(&self, from: &str, to: Option<String>) -> Result<usize> {
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let mut manifest = self.load_manifest()?;
        let mut changed = 0usize;
        for entry in manifest.entries.iter_mut() {
            if entry.category.as_deref() == Some(from) {
                entry.category = to.clone();
                changed += 1;
            }
        }
        if changed == 0 {
            return Ok(0);
        }
        manifest.version = MANIFEST_VERSION;
        self.store_manifest(&manifest)?;
        Ok(changed)
    }
}

/// SHA-256 of `bytes` as lowercase hex (the content id).
fn hash_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest.iter() {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Barrier};

    fn src_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(content).unwrap();
        p
    }

    fn req<'a>(source: &'a Path, kind: &'a str, category: Option<&str>) -> FavoriteRequest<'a> {
        FavoriteRequest {
            source,
            kind,
            category: category.map(|c| c.to_string()),
            favorited_at: 1_700_000_000.0,
            thumb: None,
        }
    }

    #[test]
    fn favorite_copies_file_and_writes_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let src = src_file(tmp.path(), "clip.mp4", b"hello world");

        let store = LibraryStore::new(&lib);
        let entry = store.favorite(&req(&src, "video", None)).unwrap();

        // File copied into the library under its content hash.
        let stored = store.stored_path(&entry.id).unwrap().unwrap();
        assert!(stored.exists());
        assert_eq!(std::fs::read(&stored).unwrap(), b"hello world");
        // Manifest persisted and reloads to the same single entry.
        assert!(lib.join(MANIFEST_NAME).exists());
        let entries = store.entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], entry);
        assert_eq!(entries[0].kind, "video");
    }

    #[test]
    fn dedup_same_content_does_not_duplicate() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        // Two different source paths, identical bytes.
        let a = src_file(tmp.path(), "a.mp4", b"same bytes");
        let b = src_file(tmp.path(), "b.mp4", b"same bytes");

        let store = LibraryStore::new(&lib);
        let first = store.favorite(&req(&a, "video", None)).unwrap();
        let second = store.favorite(&req(&b, "video", None)).unwrap();

        assert_eq!(first.id, second.id);
        // Only one manifest entry and one stored file.
        assert_eq!(store.entries().unwrap().len(), 1);
        assert_eq!(store.stored_paths().unwrap().len(), 1);
        // The kept entry is the first favorite (source a).
        assert_eq!(second.source.as_deref(), a.to_str());
    }

    #[test]
    fn favorite_outcome_ownership_uses_the_returned_content_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let first_source = src_file(tmp.path(), "first.mp4", b"shared bytes");
        let returned_source = src_file(tmp.path(), "returned.mp4", b"shared bytes");
        let store = LibraryStore::new(tmp.path().join("lib"));

        let created = store
            .favorite_with_outcome(&req(&first_source, "video", None))
            .unwrap();
        let reused = store
            .favorite_with_outcome(&req(&returned_source, "video", None))
            .unwrap();

        assert!(created.created);
        assert!(!reused.created);
        assert_eq!(reused.entry, created.entry);
        assert_eq!(store.entries().unwrap(), vec![created.entry]);
    }

    #[test]
    fn content_id_matches_favorite_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"same identity");
        let store = LibraryStore::new(tmp.path().join("lib"));

        let id = store.content_id(&source).unwrap();
        let entry = store.favorite(&req(&source, "video", None)).unwrap();

        assert_eq!(id, entry.id);
    }

    #[test]
    fn favorite_repairs_missing_stored_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"repair me");
        let store = LibraryStore::new(tmp.path().join("lib"));
        let first = store.favorite(&req(&source, "video", None)).unwrap();
        let stored = store.stored_path(&first.id).unwrap().unwrap();
        std::fs::remove_file(&stored).unwrap();

        let repaired = store.favorite(&req(&source, "video", None)).unwrap();

        assert_eq!(repaired, first);
        let repaired_path = store.stored_path(&first.id).unwrap().unwrap();
        assert_eq!(std::fs::read(repaired_path).unwrap(), b"repair me");
        assert_eq!(store.entries().unwrap(), vec![first]);
    }

    #[test]
    fn prepared_favorite_stays_hidden_until_publish_and_drop_cleans_stage() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"prepared bytes");
        let store = LibraryStore::new(tmp.path().join("lib"));

        let prepared = store
            .prepare_favorite(&req(&source, "video", None))
            .unwrap();
        let id = prepared.entry().id.clone();
        assert!(prepared.needs_publish());
        assert!(store.entries().unwrap().is_empty());
        assert!(store.stored_path(&id).unwrap().is_none());
        drop(prepared);
        assert!(store.entries().unwrap().is_empty());
        assert!(store.stored_path(&id).unwrap().is_none());
        assert_eq!(
            std::fs::read_dir(store.files_dir().join(STAGING_SUBDIR))
                .unwrap()
                .count(),
            0
        );

        let prepared = store
            .prepare_favorite(&req(&source, "video", None))
            .unwrap();
        let outcome = store.publish_favorite(prepared).unwrap();
        assert!(outcome.created);
        assert_eq!(outcome.entry.id, id);
        assert_eq!(store.entries().unwrap(), vec![outcome.entry]);
        assert_eq!(
            std::fs::read(store.stored_path(&id).unwrap().unwrap()).unwrap(),
            b"prepared bytes"
        );
    }

    #[test]
    fn failed_manifest_publish_cleans_both_stage_and_unowned_target() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"failed publish bytes");
        let store = LibraryStore::new(tmp.path().join("lib"));
        let prepared = store
            .prepare_favorite(&req(&source, "video", None))
            .unwrap();
        let id = prepared.entry().id.clone();
        std::fs::create_dir(store.root.join(MANIFEST_NAME)).unwrap();

        store
            .publish_favorite(prepared)
            .expect_err("blocked manifest commit must fail");

        std::fs::remove_dir(store.root.join(MANIFEST_NAME)).unwrap();
        assert!(store.entries().unwrap().is_empty());
        assert!(store.stored_path(&id).unwrap().is_none());
        assert!(store.stored_paths().unwrap().is_empty());
        assert_eq!(
            std::fs::read_dir(store.files_dir().join(STAGING_SUBDIR))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn restart_reconciliation_removes_stale_stage_and_unmanifested_content() {
        let tmp = tempfile::tempdir().unwrap();
        let library_root = tmp.path().join("lib");
        let source = src_file(tmp.path(), "kept.mp4", b"kept bytes");
        let store = LibraryStore::new(&library_root);
        let kept = store.favorite(&req(&source, "video", None)).unwrap();
        let kept_path = store.stored_path(&kept.id).unwrap().unwrap();
        let staging_dir = store.files_dir().join(STAGING_SUBDIR);
        std::fs::create_dir_all(&staging_dir).unwrap();
        std::fs::write(staging_dir.join("crashed.pending"), b"staged orphan").unwrap();
        let orphan_path = store.files_dir().join(format!("{}.mov", "0".repeat(64)));
        std::fs::write(&orphan_path, b"published before crash").unwrap();

        let reopened = LibraryStore::new(&library_root);
        reopened.reconcile_storage().unwrap();

        assert!(kept_path.is_file());
        assert_eq!(std::fs::read_dir(&staging_dir).unwrap().count(), 0);
        assert!(!orphan_path.exists());
        assert_eq!(reopened.entries().unwrap(), vec![kept]);
        assert_eq!(reopened.stored_paths().unwrap().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn reconciliation_rejects_a_symlinked_files_root_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let library_root = tmp.path().join("lib");
        let external = tmp.path().join("external");
        std::fs::create_dir_all(&library_root).unwrap();
        std::fs::create_dir(&external).unwrap();
        let sentinel = external.join("sentinel.mp4");
        std::fs::write(&sentinel, b"must survive").unwrap();
        symlink(&external, library_root.join(FILES_SUBDIR)).unwrap();
        let store = LibraryStore::new(library_root);

        let error = store
            .reconcile_storage()
            .expect_err("symlinked files root must be rejected");

        assert!(error.to_string().contains("secure global library root"));
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"must survive");
        assert_eq!(std::fs::read_dir(&external).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn retained_files_capability_defeats_a_barrier_coordinated_root_swap() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let library_root = tmp.path().join("lib");
        let external = tmp.path().join("external");
        std::fs::create_dir(&external).unwrap();
        let sentinel = external.join("sentinel.mp4");
        std::fs::write(&sentinel, b"must survive").unwrap();
        let store = Arc::new(LibraryStore::new(&library_root));
        let retained_files = library_root.join("files.retained");
        std::fs::write(store.files_dir().join("orphan.mp4"), b"orphan").unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let worker = {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.reconcile_storage()
            })
        };

        std::fs::rename(store.files_dir(), &retained_files).unwrap();
        symlink(&external, store.files_dir()).unwrap();
        barrier.wait();
        worker.join().unwrap().unwrap();

        assert_eq!(std::fs::read(&sentinel).unwrap(), b"must survive");
        assert_eq!(std::fs::read_dir(&external).unwrap().count(), 1);
        assert!(!retained_files.join("orphan.mp4").exists());
    }

    #[cfg(unix)]
    #[test]
    fn retained_root_capability_defeats_a_barrier_coordinated_namespace_swap() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"retained root bytes");
        let library_root = tmp.path().join("lib");
        let retained_root = tmp.path().join("lib.retained");
        let external = tmp.path().join("external");
        std::fs::create_dir(&external).unwrap();
        let sentinel = external.join("sentinel.json");
        std::fs::write(&sentinel, b"must survive").unwrap();
        let store = Arc::new(LibraryStore::new(&library_root));
        let barrier = Arc::new(Barrier::new(2));
        let worker = {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.favorite(&req(&source, "video", None))
            })
        };

        std::fs::rename(&library_root, &retained_root).unwrap();
        symlink(&external, &library_root).unwrap();
        barrier.wait();
        let entry = worker.join().unwrap().unwrap();

        assert_eq!(std::fs::read(&sentinel).unwrap(), b"must survive");
        assert_eq!(std::fs::read_dir(&external).unwrap().count(), 1);
        assert!(retained_root.join(MANIFEST_NAME).is_file());
        assert!(std::fs::read_dir(retained_root.join(FILES_SUBDIR))
            .unwrap()
            .any(|item| item
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(&entry.id)));
    }

    #[cfg(unix)]
    #[test]
    fn retained_staging_capability_defeats_a_barrier_coordinated_namespace_swap() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"retained staging bytes");
        let store = Arc::new(LibraryStore::new(tmp.path().join("lib")));
        let staging_path = store.files_dir().join(STAGING_SUBDIR);
        let retained_staging = store.files_dir().join(".staging.retained");
        let external = tmp.path().join("external-stage");
        std::fs::create_dir(&external).unwrap();
        let sentinel = external.join("sentinel.pending");
        std::fs::write(&sentinel, b"must survive").unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let worker = {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                drop(
                    store
                        .prepare_favorite(&req(&source, "video", None))
                        .unwrap(),
                );
            })
        };

        std::fs::rename(&staging_path, &retained_staging).unwrap();
        symlink(&external, &staging_path).unwrap();
        barrier.wait();
        worker.join().unwrap();

        assert_eq!(std::fs::read(&sentinel).unwrap(), b"must survive");
        assert_eq!(std::fs::read_dir(&external).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(&retained_staging).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn manifest_reads_never_follow_a_swapped_leaf_symlink() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let library_root = tmp.path().join("lib");
        let external_manifest = tmp.path().join("external.json");
        std::fs::write(&external_manifest, br#"{"version":1,"entries":[]}"#).unwrap();
        let store = LibraryStore::new(&library_root);
        symlink(&external_manifest, library_root.join(MANIFEST_NAME)).unwrap();

        store
            .entries()
            .expect_err("manifest leaf symlink must fail closed");

        assert_eq!(
            std::fs::read(&external_manifest).unwrap(),
            br#"{"version":1,"entries":[]}"#
        );
    }

    #[cfg(windows)]
    #[test]
    fn reconciliation_rejects_a_junctioned_files_root_without_touching_its_target() {
        let tmp = tempfile::tempdir().unwrap();
        let library_root = tmp.path().join("lib");
        let external = tmp.path().join("external");
        std::fs::create_dir_all(&library_root).unwrap();
        std::fs::create_dir(&external).unwrap();
        let sentinel = external.join("sentinel.mp4");
        std::fs::write(&sentinel, b"must survive").unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(library_root.join(FILES_SUBDIR))
            .arg(&external)
            .status()
            .unwrap();
        assert!(status.success(), "create junction fixture");
        let store = LibraryStore::new(library_root);

        store
            .reconcile_storage()
            .expect_err("junctioned files root must be rejected");

        assert_eq!(std::fs::read(&sentinel).unwrap(), b"must survive");
        assert_eq!(std::fs::read_dir(&external).unwrap().count(), 1);
    }

    #[test]
    fn reconciliation_does_not_sweep_a_live_prepared_favorite() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"live stage");
        let store = LibraryStore::new(tmp.path().join("lib"));
        let prepared = store
            .prepare_favorite(&req(&source, "video", None))
            .unwrap();

        store.reconcile_storage().unwrap();
        let outcome = store.publish_favorite(prepared).unwrap();

        assert!(outcome.created);
        assert!(store.stored_path(&outcome.entry.id).unwrap().is_some());
    }

    #[test]
    fn stored_lookup_rejects_a_directory_claiming_a_manifest_id() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"directory collision");
        let store = LibraryStore::new(tmp.path().join("lib"));
        let entry = store.favorite(&req(&source, "video", None)).unwrap();
        std::fs::remove_file(store.stored_path(&entry.id).unwrap().unwrap()).unwrap();
        std::fs::create_dir(store.files_dir().join(format!("{}.mp4", entry.id))).unwrap();

        let error = store
            .stored_path(&entry.id)
            .expect_err("directory must not be accepted as durable content");

        assert!(error.to_string().contains("nofollow regular file"));
        assert!(store.reconcile_storage().is_err());
    }

    #[test]
    fn stored_lookup_rejects_duplicate_extensions_for_one_content_id() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"duplicate collision");
        let store = LibraryStore::new(tmp.path().join("lib"));
        let entry = store.favorite(&req(&source, "video", None)).unwrap();
        std::fs::write(
            store.files_dir().join(format!("{}.mov", entry.id)),
            b"duplicate",
        )
        .unwrap();

        let error = store
            .stored_paths()
            .expect_err("duplicate candidates must fail closed");

        assert!(error.to_string().contains("multiple stored copies"));
        assert!(store.reconcile_storage().is_err());
    }

    #[test]
    fn batch_stored_path_index_enumerates_the_directory_once() {
        let tmp = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(tmp.path().join("lib"));
        for index in 0..32 {
            let source = src_file(
                tmp.path(),
                &format!("clip-{index}.mp4"),
                format!("bytes-{index}").as_bytes(),
            );
            store.favorite(&req(&source, "video", None)).unwrap();
        }
        STORED_INDEX_SCANS.with(|count| count.set(0));

        let paths = store.stored_paths().unwrap();

        assert_eq!(paths.len(), 32);
        STORED_INDEX_SCANS.with(|count| assert_eq!(count.get(), 1));
    }

    #[test]
    fn concurrent_prepared_favorites_dedup_when_published() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"shared prepared bytes");
        let store = Arc::new(LibraryStore::new(tmp.path().join("lib")));
        let first = store
            .prepare_favorite(&req(&source, "video", None))
            .unwrap();
        let second = store
            .prepare_favorite(&req(&source, "video", None))
            .unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = [first, second]
            .into_iter()
            .map(|prepared| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    store.publish_favorite(prepared).unwrap()
                })
            })
            .collect();
        let outcomes: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();

        assert_eq!(outcomes.iter().filter(|outcome| outcome.created).count(), 1);
        assert_eq!(store.entries().unwrap().len(), 1);
        assert_eq!(outcomes[0].entry.id, outcomes[1].entry.id);
        assert_eq!(
            std::fs::read_dir(store.files_dir().join(STAGING_SUBDIR))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn expected_id_repair_rejects_changed_source_without_creating_an_orphan() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"original bytes");
        let store = LibraryStore::new(tmp.path().join("lib"));
        let entry = store.favorite(&req(&source, "video", None)).unwrap();
        let stored = store.stored_path(&entry.id).unwrap().unwrap();
        std::fs::remove_file(stored).unwrap();
        std::fs::write(&source, b"changed bytes").unwrap();

        let error = store
            .repair_stored_copy(&entry.id, &source)
            .expect_err("changed source must not repair a different content id");

        assert!(error.to_string().contains("source content changed"));
        assert_eq!(store.entries().unwrap(), vec![entry.clone()]);
        assert!(store.stored_path(&entry.id).unwrap().is_none());
        assert!(store.stored_paths().unwrap().is_empty());
    }

    #[test]
    fn expected_id_repair_restores_unchanged_source() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"stable bytes");
        let store = LibraryStore::new(tmp.path().join("lib"));
        let entry = store.favorite(&req(&source, "video", None)).unwrap();
        std::fs::remove_file(store.stored_path(&entry.id).unwrap().unwrap()).unwrap();

        store.repair_stored_copy(&entry.id, &source).unwrap();

        assert_eq!(store.entries().unwrap(), vec![entry.clone()]);
        assert_eq!(
            std::fs::read(store.stored_path(&entry.id).unwrap().unwrap()).unwrap(),
            b"stable bytes"
        );
    }

    #[test]
    fn concurrent_favorites_do_not_lose_entries() {
        const THREADS: usize = 16;
        let tmp = tempfile::tempdir().unwrap();
        let store = Arc::new(LibraryStore::new(tmp.path().join("lib")));
        let barrier = Arc::new(Barrier::new(THREADS));
        let sources: Vec<_> = (0..THREADS)
            .map(|index| {
                src_file(
                    tmp.path(),
                    &format!("clip-{index}.mp4"),
                    format!("content-{index}").as_bytes(),
                )
            })
            .collect();

        let handles: Vec<_> = sources
            .into_iter()
            .map(|source| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    store.favorite(&req(&source, "video", None)).unwrap()
                })
            })
            .collect();
        let entries: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();

        let stored = store.entries().unwrap();
        assert_eq!(stored.len(), THREADS);
        assert!(entries
            .iter()
            .all(|entry| stored.iter().any(|stored| stored.id == entry.id)));
    }

    #[test]
    fn distinct_content_yields_distinct_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let a = src_file(tmp.path(), "a.mp4", b"alpha");
        let b = src_file(tmp.path(), "b.mp4", b"beta");

        let store = LibraryStore::new(&lib);
        store.favorite(&req(&a, "video", None)).unwrap();
        store.favorite(&req(&b, "audio", None)).unwrap();

        assert_eq!(store.entries().unwrap().len(), 2);
    }

    #[test]
    fn category_filter_partitions_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let a = src_file(tmp.path(), "a.mp4", b"a");
        let b = src_file(tmp.path(), "b.mp4", b"b");
        let c = src_file(tmp.path(), "c.mp4", b"c");

        let store = LibraryStore::new(&lib);
        store.favorite(&req(&a, "video", Some("broll"))).unwrap();
        store.favorite(&req(&b, "video", Some("broll"))).unwrap();
        store.favorite(&req(&c, "video", None)).unwrap();

        assert_eq!(store.entries_in_category(Some("broll")).unwrap().len(), 2);
        assert_eq!(store.entries_in_category(Some("music")).unwrap().len(), 0);
        // None keeps only uncategorized.
        let uncat = store.entries_in_category(None).unwrap();
        assert_eq!(uncat.len(), 1);
        assert_eq!(uncat[0].category, None);
    }

    #[test]
    fn missing_manifest_reads_as_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(tmp.path().join("never_created"));
        assert!(store.entries().unwrap().is_empty());
        assert!(!store.contains("anything").unwrap());
    }

    #[test]
    fn contains_reflects_favorited_id() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let a = src_file(tmp.path(), "a.mp4", b"payload");
        let store = LibraryStore::new(&lib);
        let e = store.favorite(&req(&a, "video", None)).unwrap();
        assert!(store.contains(&e.id).unwrap());
    }

    #[test]
    fn remove_deletes_entry_and_file() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let a = src_file(tmp.path(), "a.mp4", b"removable");
        let store = LibraryStore::new(&lib);
        let e = store.favorite(&req(&a, "video", None)).unwrap();

        assert!(store.remove(&e.id).unwrap());
        assert!(store.entries().unwrap().is_empty());
        assert!(store.stored_path(&e.id).unwrap().is_none());
        // Removing again is a no-op.
        assert!(!store.remove(&e.id).unwrap());
    }

    #[test]
    fn failed_remove_manifest_commit_preserves_entry_and_stored_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let source = src_file(tmp.path(), "a.mp4", b"must survive failed removal");
        let store = LibraryStore::new(&lib);
        let entry = store.favorite(&req(&source, "video", None)).unwrap();
        let stored = store.stored_path(&entry.id).unwrap().unwrap();
        let manifest_path = lib.join(MANIFEST_NAME);
        let manifest_before = std::fs::read(&manifest_path).unwrap();
        std::fs::remove_file(&manifest_path).unwrap();
        std::fs::create_dir(&manifest_path).unwrap();

        assert!(store.remove(&entry.id).is_err());
        std::fs::remove_dir(&manifest_path).unwrap();
        std::fs::write(&manifest_path, manifest_before).unwrap();
        assert!(store.contains(&entry.id).unwrap());
        assert_eq!(
            store.stored_path(&entry.id).unwrap().as_deref(),
            Some(stored.as_path())
        );
        assert!(stored.is_file());
    }

    #[test]
    fn stale_transaction_temp_does_not_block_a_later_manifest_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"fresh transaction");
        let store = LibraryStore::new(tmp.path().join("lib"));
        let legacy_tmp = store.root.join(format!("{MANIFEST_NAME}.tmp"));
        let unique_tmp = store.root.join(format!(".{MANIFEST_NAME}.0.{:020}.tmp", 7));
        std::fs::write(&legacy_tmp, b"crashed").unwrap();
        std::fs::write(&unique_tmp, b"crashed").unwrap();

        let entry = store.favorite(&req(&source, "video", None)).unwrap();

        assert_eq!(store.entries().unwrap(), vec![entry]);
        assert!(!legacy_tmp.exists());
        assert!(!unique_tmp.exists());
    }

    #[test]
    fn startup_restores_a_retained_manifest_backup_when_canonical_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let source = src_file(tmp.path(), "clip.mp4", b"recover backup");
        let library_root = tmp.path().join("lib");
        let store = LibraryStore::new(&library_root);
        let entry = store.favorite(&req(&source, "video", None)).unwrap();
        let backup = library_root.join(format!(
            ".{MANIFEST_NAME}.{}.{:020}.backup",
            std::process::id(),
            42
        ));
        std::fs::rename(library_root.join(MANIFEST_NAME), &backup).unwrap();
        let reopened = LibraryStore::new(&library_root);

        reopened.reconcile_storage().unwrap();

        assert_eq!(reopened.entries().unwrap(), vec![entry]);
        assert!(library_root.join(MANIFEST_NAME).is_file());
        assert!(!backup.exists());
    }

    #[test]
    fn invalid_canonical_manifest_preserves_a_recoverable_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let library_root = tmp.path().join("lib");
        let store = LibraryStore::new(&library_root);
        let backup = library_root.join(format!(
            ".{MANIFEST_NAME}.{}.{:020}.backup",
            std::process::id(),
            43
        ));
        std::fs::write(&backup, br#"{"version":1,"entries":[]}"#).unwrap();
        std::fs::write(library_root.join(MANIFEST_NAME), b"not json").unwrap();

        store
            .reconcile_storage()
            .expect_err("invalid canonical manifest must fail closed");

        assert!(backup.is_file());
        assert_eq!(
            std::fs::read(library_root.join(MANIFEST_NAME)).unwrap(),
            b"not json"
        );
    }

    #[test]
    fn manifest_roundtrips_all_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let a = src_file(tmp.path(), "a.png", b"img");
        let store = LibraryStore::new(&lib);
        let r = FavoriteRequest {
            source: &a,
            kind: "image",
            category: Some("logo".to_string()),
            favorited_at: 1_718_900_000.5,
            thumb: Some("data:thumb".to_string()),
        };
        let e = store.favorite(&r).unwrap();
        // Reload from a fresh store instance to exercise full serde roundtrip.
        let reopened = LibraryStore::new(&lib);
        let got = reopened.entries().unwrap();
        assert_eq!(got, vec![e]);
        assert_eq!(got[0].thumb.as_deref(), Some("data:thumb"));
        assert_eq!(got[0].favorited_at, 1_718_900_000.5);
    }

    #[test]
    fn entry_json_uses_camelcase_and_type_key() {
        let e = LibraryEntry {
            id: "abc".to_string(),
            kind: "video".to_string(),
            category: None,
            favorited_at: 1.0,
            source: None,
            thumb: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"type\":\"video\""));
        assert!(json.contains("\"favoritedAt\":1.0"));
    }

    #[test]
    fn set_category_updates_one_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let a = src_file(tmp.path(), "a.mp4", b"cat");
        let store = LibraryStore::new(&lib);
        let e = store.favorite(&req(&a, "video", None)).unwrap();

        let updated = store
            .set_category(&e.id, Some("broll".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(updated.category.as_deref(), Some("broll"));
        // Persisted across a fresh store instance.
        let got = LibraryStore::new(&lib).entries().unwrap();
        assert_eq!(got[0].category.as_deref(), Some("broll"));
        // Unknown id yields None.
        assert!(store.set_category("nope", None).unwrap().is_none());
        // Clearing works.
        store.set_category(&e.id, None).unwrap();
        assert_eq!(store.entries().unwrap()[0].category, None);
    }

    #[test]
    fn rename_category_moves_matching_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let a = src_file(tmp.path(), "a.mp4", b"x");
        let b = src_file(tmp.path(), "b.mp4", b"y");
        let c = src_file(tmp.path(), "c.mp4", b"z");
        let store = LibraryStore::new(&lib);
        store.favorite(&req(&a, "video", Some("old"))).unwrap();
        store.favorite(&req(&b, "video", Some("old"))).unwrap();
        store.favorite(&req(&c, "video", Some("keep"))).unwrap();

        let changed = store
            .rename_category("old", Some("new".to_string()))
            .unwrap();
        assert_eq!(changed, 2);
        assert_eq!(store.entries_in_category(Some("new")).unwrap().len(), 2);
        assert_eq!(store.entries_in_category(Some("keep")).unwrap().len(), 1);
        // No match is a no-op.
        assert_eq!(store.rename_category("missing", None).unwrap(), 0);
    }

    #[test]
    fn default_library_dir_ends_with_app_and_library() {
        if let Some(dir) = default_library_dir() {
            assert!(dir.ends_with(Path::new(APP_DIR).join(LIBRARY_DIR)));
        }
    }
}
