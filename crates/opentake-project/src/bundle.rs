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
