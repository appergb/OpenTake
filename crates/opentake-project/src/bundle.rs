//! The `.opentake` directory bundle: in-memory [`Project`] plus
//! [`Project::open`] / [`Project::save`].
//!
//! Port of `VideoProject`'s persistence (`Project/VideoProject.swift`), minus
//! the AppKit `NSDocument` / `FileWrapper` machinery. A bundle is a plain
//! directory; we read and write its files by path.
//!
//! Read semantics match upstream `read(from:)`:
//! - `project.json` is mandatory; absence is [`ProjectError::MissingTimeline`]
//!   (upstream throws `fileReadCorruptFile`).
//! - `media.json`, if present, is parsed strictly; a parse failure is an error
//!   (upstream throws `fileReadCorruptFile`).
//! - `generation-log.json`, if present, is parsed leniently; a parse failure is
//!   swallowed and the log becomes `None` (upstream `try?`).
//!
//! Write semantics follow the architecture note "assemble an in-memory
//! snapshot, then write atomically": each JSON component is written to a
//! sibling temp file and renamed into place, so a crash never leaves a
//! half-written `project.json`. `save` owns only the JSON components (and the
//! thumbnail when held); it never creates or deletes `media/` or
//! `chat-sessions/`, which the media and agent layers manage out-of-band.

use std::fs;
use std::path::{Path, PathBuf};

use opentake_domain::{MediaManifest, Timeline};
use serde::Serialize;

use crate::error::{ProjectError, Result};
use crate::gen_log::GenerationLog;
use crate::layout;

/// An opened `.opentake` project: the bundle path plus its decoded components.
///
/// Media files referenced by `manifest` live under the bundle's `media/`
/// directory (`.project` sources) or at absolute paths (`.external`); they are
/// not loaded into this struct. Chat sessions and the thumbnail are likewise
/// left on disk, except for an optional in-memory `thumbnail` that `save` will
/// persist when set.
#[derive(Clone, Debug)]
pub struct Project {
    /// Absolute path to the bundle directory (`…/Name.opentake`).
    pub bundle_path: PathBuf,
    /// The timeline (`project.json`).
    pub timeline: Timeline,
    /// The media manifest (`media.json`). Defaults to empty when the file was
    /// absent.
    pub manifest: MediaManifest,
    /// The generation log (`generation-log.json`). `None` when the file was
    /// absent or failed to parse.
    pub generation_log: Option<GenerationLog>,
    /// JPEG thumbnail bytes to write on the next `save`. `None` leaves any
    /// existing `thumbnail.jpg` on disk untouched.
    pub thumbnail: Option<Vec<u8>>,
}

impl Project {
    /// Create a fresh, empty project rooted at `bundle_path` (not yet written).
    pub fn new(bundle_path: impl Into<PathBuf>) -> Self {
        Project {
            bundle_path: bundle_path.into(),
            timeline: Timeline::new(),
            manifest: MediaManifest::new(),
            generation_log: None,
            thumbnail: None,
        }
    }

    /// Open the `.opentake` bundle at `path`.
    ///
    /// Returns [`ProjectError::NotABundle`] if `path` is not a directory,
    /// [`ProjectError::MissingTimeline`] if `project.json` is absent, and
    /// [`ProjectError::Json`] if `project.json` or `media.json` fails to parse.
    /// A malformed `generation-log.json` is ignored.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let bundle = path.as_ref();
        if !bundle.is_dir() {
            return Err(ProjectError::NotABundle(bundle.to_path_buf()));
        }

        let timeline_path = layout::timeline_path(bundle);
        if !timeline_path.is_file() {
            return Err(ProjectError::MissingTimeline {
                file: layout::TIMELINE_FILE,
                bundle: bundle.to_path_buf(),
            });
        }
        let timeline_bytes = read_file(&timeline_path)?;
        let timeline: Timeline = serde_json::from_slice(&timeline_bytes)
            .map_err(|e| ProjectError::json(layout::TIMELINE_FILE, e))?;

        // media.json: strict when present, empty default when absent.
        let manifest_path = layout::manifest_path(bundle);
        let manifest = if manifest_path.is_file() {
            let bytes = read_file(&manifest_path)?;
            serde_json::from_slice(&bytes)
                .map_err(|e| ProjectError::json(layout::MANIFEST_FILE, e))?
        } else {
            MediaManifest::new()
        };

        // generation-log.json: lenient — a parse error degrades to None.
        let gen_log_path = layout::generation_log_path(bundle);
        let generation_log = if gen_log_path.is_file() {
            match read_file(&gen_log_path) {
                Ok(bytes) => serde_json::from_slice::<GenerationLog>(&bytes).ok(),
                Err(_) => None,
            }
        } else {
            None
        };

        Ok(Project {
            bundle_path: bundle.to_path_buf(),
            timeline,
            manifest,
            generation_log,
            thumbnail: None,
        })
    }

    /// Write this project's JSON components into [`Self::bundle_path`].
    ///
    /// Creates the bundle directory if needed. Always (re)writes `project.json`
    /// and `media.json`; writes `generation-log.json` when a log is held and
    /// `thumbnail.jpg` when [`Self::thumbnail`] is set. Each file is written
    /// atomically (temp file + rename). Existing `media/` and `chat-sessions/`
    /// directories are left untouched.
    pub fn save(&self) -> Result<()> {
        self.save_to(&self.bundle_path)
    }

    /// Like [`Self::save`] but targets an explicit `bundle` directory (used by
    /// the archiver to stage a self-contained copy). Does not mutate `self`.
    pub fn save_to(&self, bundle: impl AsRef<Path>) -> Result<()> {
        let bundle = bundle.as_ref();
        create_dir_all(bundle)?;

        write_json_atomic(bundle, layout::TIMELINE_FILE, &self.timeline)?;
        write_json_atomic(bundle, layout::MANIFEST_FILE, &self.manifest)?;
        if let Some(log) = &self.generation_log {
            write_json_atomic(bundle, layout::GENERATION_LOG_FILE, log)?;
        }
        if let Some(bytes) = &self.thumbnail {
            write_bytes_atomic(&layout::thumbnail_path(bundle), bytes)?;
        }
        Ok(())
    }
}

/// Copy a source bundle's `media/` directory into `dest_bundle`, recursively,
/// preserving the relative layout — the port of upstream `mediaDirWrapper`
/// (`Project/VideoProject.swift:112-117`), which folds the whole `media/`
/// directory into the saved package on every save/save-as. Save-as builds the
/// new bundle at a fresh path; without this, project-internal media
/// ([`MediaSource::Project`](opentake_domain::MediaSource) relative paths — AI
/// output, pasted, captured stills) is left behind and every reference silently
/// dangles.
///
/// Contract:
/// - **Missing source `media/`** → no-op `Ok(())` (upstream returns `nil` from
///   `mediaDirWrapper` when the dir doesn't exist; nothing to carry).
/// - **Same-path save** (source and dest bundle are the same directory) → no-op,
///   so autosave never copies `media/` onto itself.
/// - **Partial-copy failure** → the destination `media/` is never left
///   half-populated: the tree is staged into a sibling temp directory and
///   atomically renamed into place only after a fully successful copy; any error
///   removes the temp staging and propagates, matching the atomic-replace
///   philosophy [`archive`](crate::archive) uses.
pub fn copy_media_dir(source_bundle: &Path, dest_bundle: &Path) -> Result<()> {
    // Same bundle (autosave / plain save): nothing to copy. Compare with
    // `standardize`-free canonical-ish equality via the same-path check the
    // caller already knows; here we guard the source==dest media dir case so a
    // direct call is self-protecting too.
    if source_bundle == dest_bundle {
        return Ok(());
    }

    let src_media = layout::media_dir(source_bundle);
    if !src_media.is_dir() {
        return Ok(()); // upstream: no media/ dir -> no wrapper -> nothing written
    }

    let dest_media = layout::media_dir(dest_bundle);
    create_dir_all(dest_bundle)?;

    // Stage into a sibling temp dir, then atomically swap into `media/` so a
    // failure mid-copy never leaves a partially populated `media/`.
    let staging = temp_sibling(&dest_media);
    // A stale staging dir from a crashed prior run would break create_dir_all's
    // freshness; clear it first (best-effort).
    let _ = fs::remove_dir_all(&staging);
    if let Err(e) = copy_dir_recursive(&src_media, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }

    // Replace any existing dest `media/` with the freshly staged tree. `rename`
    // onto an existing directory fails on most platforms, so remove first; the
    // window between remove and rename is the same one `write_bytes_atomic`
    // accepts for JSON components.
    if dest_media.exists() {
        if let Err(e) = fs::remove_dir_all(&dest_media) {
            let _ = fs::remove_dir_all(&staging);
            return Err(ProjectError::io(&dest_media, e));
        }
    }
    match fs::rename(&staging, &dest_media) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            Err(ProjectError::io(&dest_media, e))
        }
    }
}

/// Recursively copy directory `src` into `dest`, creating `dest` and mirroring
/// the subtree. Shared by [`copy_media_dir`]; kept here (rather than reused from
/// [`crate::archive`], whose copy helper is private and coupled to its report
/// bookkeeping) so bundle save stays self-contained.
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    create_dir_all(dest)?;
    let entries = fs::read_dir(src).map_err(|e| ProjectError::io(src, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| ProjectError::io(src, e))?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        let file_type = entry.file_type().map_err(|e| ProjectError::io(&from, e))?;
        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .map(|_| ())
                .map_err(|e| ProjectError::io(&to, e))?;
        }
    }
    Ok(())
}

// --- IO helpers (each tags the failing path) ---

fn read_file(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|e| ProjectError::io(path, e))
}

fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| ProjectError::io(path, e))
}

/// Serialize `value` to pretty JSON and write it atomically into
/// `dir/file_name`.
fn write_json_atomic<T: Serialize>(dir: &Path, file_name: &str, value: &T) -> Result<()> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| ProjectError::json(file_name, e))?;
    write_bytes_atomic(&dir.join(file_name), &json)
}

/// Write `bytes` to `dest` via a sibling temp file + rename, so a partial write
/// never clobbers an existing good file.
fn write_bytes_atomic(dest: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = temp_sibling(dest);
    fs::write(&tmp, bytes).map_err(|e| ProjectError::io(&tmp, e))?;
    match fs::rename(&tmp, dest) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(ProjectError::io(dest, e))
        }
    }
}

/// A temp path next to `dest` (same directory, so `rename` is atomic on the
/// same filesystem). Uniqueness comes from the pid plus a process-global
/// counter — enough to avoid collisions between concurrent writers in one
/// process without pulling in an RNG dependency.
fn temp_sibling(dest: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = dest
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bundle".to_string());
    let tmp_name = format!(".{}.{}.{}.tmp", name, std::process::id(), n);
    match dest.parent() {
        Some(parent) => parent.join(tmp_name),
        None => PathBuf::from(tmp_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A per-call-unique scratch dir under the system temp dir, removed on drop.
    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir()
                .join(format!("opentake-bundle-{tag}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn copy_media_dir_mirrors_nested_layout() {
        let tmp = TmpDir::new("nested");
        let src = tmp.path().join("Src.opentake");
        let dst = tmp.path().join("Dst.opentake");
        let src_media = layout::media_dir(&src);
        fs::create_dir_all(src_media.join("sub")).unwrap();
        fs::write(src_media.join("a.png"), b"AAA").unwrap();
        fs::write(src_media.join("sub").join("b.mov"), b"BBBB").unwrap();

        copy_media_dir(&src, &dst).unwrap();

        assert_eq!(fs::read(dst.join("media").join("a.png")).unwrap(), b"AAA");
        assert_eq!(
            fs::read(dst.join("media").join("sub").join("b.mov")).unwrap(),
            b"BBBB"
        );
    }

    #[test]
    fn copy_media_dir_missing_source_is_noop() {
        let tmp = TmpDir::new("missing");
        let src = tmp.path().join("Src.opentake"); // no media/ under it
        let dst = tmp.path().join("Dst.opentake");
        fs::create_dir_all(&src).unwrap();

        copy_media_dir(&src, &dst).unwrap();
        assert!(!dst.join("media").exists());
    }

    #[test]
    fn copy_media_dir_same_path_is_noop() {
        let tmp = TmpDir::new("same");
        let bundle = tmp.path().join("Same.opentake");
        let media = layout::media_dir(&bundle);
        fs::create_dir_all(&media).unwrap();
        fs::write(media.join("keep.png"), b"KEEP").unwrap();

        // Source == dest: must not touch (delete/replace) the existing media/.
        copy_media_dir(&bundle, &bundle).unwrap();
        assert_eq!(fs::read(media.join("keep.png")).unwrap(), b"KEEP");
    }

    #[test]
    fn copy_media_dir_replaces_existing_dest_media() {
        let tmp = TmpDir::new("replace");
        let src = tmp.path().join("Src.opentake");
        let dst = tmp.path().join("Dst.opentake");
        fs::create_dir_all(layout::media_dir(&src)).unwrap();
        fs::write(layout::media_dir(&src).join("new.png"), b"NEW").unwrap();
        // Pre-existing stale file in the destination media/ that is NOT in the
        // source; a full swap must not leave it behind.
        fs::create_dir_all(layout::media_dir(&dst)).unwrap();
        fs::write(layout::media_dir(&dst).join("stale.png"), b"OLD").unwrap();

        copy_media_dir(&src, &dst).unwrap();

        assert_eq!(fs::read(dst.join("media").join("new.png")).unwrap(), b"NEW");
        assert!(
            !dst.join("media").join("stale.png").exists(),
            "stale dest media should be replaced, not merged"
        );
    }
}
