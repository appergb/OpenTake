//! Global asset library — a cross-project store of "favorited" media that lives
//! outside any single OpenTake project (issue #54, part of #37 "全局可复用素材库").
//!
//! Layout under the library root (resolved cross-platform via [`dirs`], e.g.
//! `~/Library/Application Support/OpenTake/Library/` on macOS):
//! ```text
//! <root>/
//!   library.json          manifest: { version, entries: [LibraryEntry, …] }
//!   files/<hash><ext>      copy-on-favorite content, content-addressed
//!   library.json.tmp       transient; atomically renamed over library.json
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

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{MediaError, Result};

/// Manifest filename under the library root.
pub const MANIFEST_NAME: &str = "library.json";
/// Subdirectory holding the content-addressed copies.
pub const FILES_SUBDIR: &str = "files";
/// On-disk manifest schema version (bumped on incompatible changes).
pub const MANIFEST_VERSION: u32 = 1;
/// Application directory name under the platform data dir.
const APP_DIR: &str = "OpenTake";
/// Library directory name under the application directory.
const LIBRARY_DIR: &str = "Library";

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

/// The global library store, rooted at a directory. Cloneable handles are not
/// provided; share one instance behind an `Arc` if multiple owners are needed.
pub struct LibraryStore {
    root: PathBuf,
    /// Serializes manifest read-modify-write across in-process threads.
    write_lock: Mutex<()>,
}

/// Cross-platform default library directory:
/// `<platform data dir>/OpenTake/Library`. Returns `None` only if the platform
/// data directory cannot be resolved (handled as an error by callers).
pub fn default_library_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(APP_DIR).join(LIBRARY_DIR))
}

impl LibraryStore {
    /// Open (or lazily create) a store rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        LibraryStore {
            root: root.into(),
            write_lock: Mutex::new(()),
        }
    }

    /// Open a store at the platform-default library directory.
    pub fn open_default() -> Result<Self> {
        let root = default_library_dir().ok_or_else(|| {
            MediaError::Other(anyhow::anyhow!("could not resolve platform data directory"))
        })?;
        Ok(LibraryStore::new(root))
    }

    /// The library root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join(MANIFEST_NAME)
    }

    fn files_dir(&self) -> PathBuf {
        self.root.join(FILES_SUBDIR)
    }

    /// Read the manifest, returning an empty one if it does not exist yet.
    fn load_manifest(&self) -> Result<Manifest> {
        let path = self.manifest_path();
        match std::fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Manifest {
                version: MANIFEST_VERSION,
                entries: Vec::new(),
            }),
            Err(e) => Err(MediaError::Io(e)),
        }
    }

    /// Atomically persist the manifest: write a temp file, then rename over the
    /// real path. The rename is atomic on the same filesystem.
    fn store_manifest(&self, manifest: &Manifest) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let bytes = serde_json::to_vec_pretty(manifest)?;
        let final_path = self.manifest_path();
        let tmp_path = self.root.join(format!("{MANIFEST_NAME}.tmp"));
        std::fs::write(&tmp_path, &bytes)?;
        std::fs::rename(&tmp_path, &final_path)?;
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

    /// Favorite a file: copy its bytes into the library (dedup by content hash)
    /// and record an entry. If the same content is already favorited, the
    /// existing entry is returned unchanged and no duplicate file is written.
    ///
    /// The whole read-modify-write runs under the in-process write lock so two
    /// concurrent favorites cannot clobber each other's manifest update.
    pub fn favorite(&self, req: &FavoriteRequest<'_>) -> Result<LibraryEntry> {
        let bytes = std::fs::read(req.source)?;
        let id = hash_hex(&bytes);

        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let mut manifest = self.load_manifest()?;
        manifest.version = MANIFEST_VERSION;

        if let Some(existing) = manifest.entries.iter().find(|e| e.id == id) {
            return Ok(existing.clone());
        }

        // Copy the content into the library under its hashed name. The extension
        // is preserved for readability/tooling; identity is the hash, not the ext.
        let files_dir = self.files_dir();
        std::fs::create_dir_all(&files_dir)?;
        let ext = req
            .source
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| format!(".{s}"))
            .unwrap_or_default();
        let stored = files_dir.join(format!("{id}{ext}"));
        if !stored.exists() {
            let tmp = files_dir.join(format!("{id}{ext}.tmp"));
            std::fs::write(&tmp, &bytes)?;
            std::fs::rename(&tmp, &stored)?;
        }

        let entry = LibraryEntry {
            id,
            kind: req.kind.to_string(),
            category: req.category.clone(),
            favorited_at: req.favorited_at,
            source: req.source.to_str().map(|s| s.to_string()),
            thumb: req.thumb.clone(),
        };
        manifest.entries.push(entry.clone());
        self.store_manifest(&manifest)?;
        Ok(entry)
    }

    /// Absolute path to the stored copy for an entry id, if present on disk.
    pub fn stored_path(&self, id: &str) -> Result<Option<PathBuf>> {
        let dir = self.files_dir();
        let read = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(MediaError::Io(e)),
        };
        for entry in read {
            let path = entry?.path();
            if path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|stem| stem == id)
            {
                return Ok(Some(path));
            }
        }
        Ok(None)
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
        if let Some(path) = self.stored_path(id)? {
            let _ = std::fs::remove_file(path);
        }
        self.store_manifest(&manifest)?;
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
        let count = std::fs::read_dir(lib.join(FILES_SUBDIR)).unwrap().count();
        assert_eq!(count, 1);
        // The kept entry is the first favorite (source a).
        assert_eq!(second.source.as_deref(), a.to_str());
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
