//! Self-contained bundle archiver. Port of `PalmierProjectExporter.export`
//! (`Export/PalmierProjectExporter.swift`).
//!
//! Writes a `.opentake` bundle in which every resolvable media reference is
//! copied into the new bundle's `media/` directory and its manifest source
//! rewritten to a bundle-relative `media/<file>` path. Dangling references
//! (source file not found) are kept as-is and reported as missing.
//!
//! Behavior matched 1:1 with upstream:
//! - Sources are deduplicated by their *lexically* standardized path — the same
//!   purely textual normalization upstream gets from
//!   `srcURL.standardizedFileURL.path` (collapse `.`/`..` and repeated
//!   separators, no filesystem access). Two paths that reach the same physical
//!   file through different symlinks are therefore NOT merged: like upstream,
//!   each is copied separately. See [`standardize`].
//! - Internal (`.project`) sources keep their existing file name; external
//!   sources become `import-<id[..8]>.<ext>`.
//! - Name collisions in `media/` get `-1`, `-2`, … appended (extension
//!   preserved).
//! - `collected` lists the ids of entries that were `.external` and are now
//!   bundled; `copied_internal` counts `.project` files copied; `total_bytes`
//!   is the bytes copied into the new bundle.
//! - After writing `project.json` / `media.json` / `generation-log.json`, the
//!   source bundle's `thumbnail.jpg` and `chat-sessions/` are carried across
//!   when present.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use opentake_domain::{MediaManifest, MediaSource, Timeline};

use crate::error::{ProjectError, Result};
use crate::gen_log::GenerationLog;
use crate::layout;

/// Outcome of an [`archive`] run. 1:1 with upstream `PalmierProjectExporter.Report`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ArchiveReport {
    /// Ids of entries that were `.external` and are now bundled.
    pub collected: Vec<String>,
    /// Count of already-internal (`.project`) media files copied across.
    pub copied_internal: usize,
    /// Entries whose source file could not be found (kept as dangling refs).
    pub missing: Vec<MissingMedia>,
    /// Total bytes copied into the new bundle's `media/` directory.
    pub total_bytes: u64,
}

/// A media entry whose source file was not found during archiving.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MissingMedia {
    /// The manifest entry id.
    pub id: String,
    /// The manifest entry display name.
    pub name: String,
}

/// Write a self-contained `.opentake` bundle to `dest_bundle`.
///
/// `source_bundle` is the original bundle directory used to resolve `.project`
/// relative media paths and to carry across the thumbnail / chat sessions; pass
/// `None` when the project has never been saved (only `.external` media can
/// then be resolved).
///
/// `dest_bundle` is created fresh: if it already exists it is removed first
/// (matching upstream's atomic replace).
pub fn archive(
    timeline: &Timeline,
    manifest: &MediaManifest,
    generation_log: &GenerationLog,
    source_bundle: Option<&Path>,
    dest_bundle: &Path,
) -> Result<ArchiveReport> {
    // Match upstream's "remove then land" semantics (Swift exporter:
    // `if fm.fileExists(atPath: destURL.path) { try fm.removeItem(at: destURL) }`
    // before moving the freshly staged bundle into place). Without this, re-
    // archiving over an existing bundle would leak stale `media/` files, an old
    // `thumbnail.jpg`, etc. Deleting first yields a pure bundle and honors this
    // function's doc contract.
    if dest_bundle.exists() {
        fs::remove_dir_all(dest_bundle).map_err(|e| ProjectError::io(dest_bundle, e))?;
    }

    let media_dir = layout::media_dir(dest_bundle);
    create_dir_all(&media_dir)?;

    let mut report = ArchiveReport::default();
    let mut new_entries = Vec::with_capacity(manifest.entries.len());
    // Dedup: standardized absolute source path -> "media/<file>".
    let mut relative_by_source: HashMap<PathBuf, String> = HashMap::new();

    for entry in &manifest.entries {
        let src = resolve_source(&entry.source, source_bundle);
        let exists = src.as_ref().map(|p| p.is_file()).unwrap_or(false);
        let Some(src) = src.filter(|_| exists) else {
            report.missing.push(MissingMedia {
                id: entry.id.clone(),
                name: entry.name.clone(),
            });
            new_entries.push(entry.clone()); // keep the dangling reference as-is
            continue;
        };

        let key = standardize(&src);
        let relative_path = if let Some(existing) = relative_by_source.get(&key) {
            existing.clone()
        } else {
            let preferred = filename_for(entry.id.as_str(), &entry.source, &src);
            let dest = unique_path(&media_dir, &preferred);
            copy_file(&src, &dest)?;
            let rel = format!("{}/{}", layout::MEDIA_DIR, file_name_str(&dest));
            relative_by_source.insert(key, rel.clone());
            report.total_bytes += file_size(&dest);
            if matches!(entry.source, MediaSource::Project { .. }) {
                report.copied_internal += 1;
            }
            rel
        };

        if matches!(entry.source, MediaSource::External { .. }) {
            report.collected.push(entry.id.clone());
        }
        let mut rewritten = entry.clone();
        rewritten.source = MediaSource::Project { relative_path };
        new_entries.push(rewritten);
    }

    let new_manifest = MediaManifest {
        version: manifest.version,
        entries: new_entries,
        folders: manifest.folders.clone(),
    };

    write_json(
        &layout::timeline_path(dest_bundle),
        layout::TIMELINE_FILE,
        timeline,
    )?;
    write_json(
        &layout::manifest_path(dest_bundle),
        layout::MANIFEST_FILE,
        &new_manifest,
    )?;
    write_json(
        &layout::generation_log_path(dest_bundle),
        layout::GENERATION_LOG_FILE,
        generation_log,
    )?;

    if let Some(source_bundle) = source_bundle {
        copy_if_present(
            &layout::thumbnail_path(source_bundle),
            &layout::thumbnail_path(dest_bundle),
        )?;
        copy_dir_if_present(
            &layout::chat_sessions_dir(source_bundle),
            &layout::chat_sessions_dir(dest_bundle),
        )?;
    }

    Ok(report)
}

// --- Source resolution & naming (ports of the Swift helpers) ---

/// Resolve a media source to an on-disk path. `.external` → its absolute path;
/// `.project` → joined onto `source_bundle` (or `None` without one).
fn resolve_source(source: &MediaSource, source_bundle: Option<&Path>) -> Option<PathBuf> {
    match source {
        MediaSource::External { absolute_path } => Some(PathBuf::from(absolute_path)),
        MediaSource::Project { relative_path } => {
            source_bundle.map(|base| base.join(relative_path))
        }
    }
}

/// Name for the copied file. `.project` keeps the original last component;
/// `.external` becomes `import-<id[..8]>` plus the source extension.
///
/// The extension is taken with [`path_extension`] (Swift `URL.pathExtension`
/// semantics) rather than [`Path::extension`], so anomalous source names agree
/// with upstream: `foo. mp4` (space in the extension) and `..mp4` (all leading
/// dots) both yield *no* extension, e.g. `import-abcdef01` not
/// `import-abcdef01. mp4` / `import-abcdef01.mp4`.
fn filename_for(id: &str, source: &MediaSource, src: &Path) -> String {
    match source {
        MediaSource::Project { .. } => file_name_str(src),
        MediaSource::External { .. } => {
            let prefix: String = id.chars().take(8).collect();
            let base = format!("import-{prefix}");
            match path_extension(&file_name_str(src)) {
                Some(ext) => format!("{base}.{ext}"),
                None => base,
            }
        }
    }
}

/// Append `-1`, `-2`, … (extension preserved) until the name is free in `dir`.
fn unique_path(dir: &Path, preferred_name: &str) -> PathBuf {
    let candidate = dir.join(preferred_name);
    if !candidate.exists() {
        return candidate;
    }
    let (base, ext) = split_extension(preferred_name);
    let mut n = 1u32;
    loop {
        let name = match &ext {
            Some(ext) => format!("{base}-{n}.{ext}"),
            None => format!("{base}-{n}"),
        };
        let path = dir.join(&name);
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

/// Extract a file name's extension with the exact semantics of Swift's
/// `URL.pathExtension` / `NSString.pathExtension`, so archive output matches
/// upstream byte-for-byte on anomalous names where `Path::extension` differs.
///
/// Returns the substring after the final `.` only when **all** hold (verified
/// against Foundation):
/// 1. a `.` is present;
/// 2. the segment after the final `.` is non-empty;
/// 3. that segment contains no ASCII space `' '` — Foundation rejects a space
///    but, deliberately, *not* tabs/newlines (`foo.m\tp` keeps ext `m\tp`);
/// 4. the prefix before the final `.` contains at least one non-`.` character
///    (so `..mp4`, `...mp4`, `.hidden` have no extension, while `a..mp4`,
///    ` .mp4` do).
///
/// Otherwise there is no extension. Examples: `foo.mp4` → `mp4`,
/// `my.clip.mov` → `mov`, `foo. mp4` → `None`, `..mp4` → `None`,
/// `trailing.` → `None`, `.hidden` → `None`.
fn path_extension(name: &str) -> Option<&str> {
    let pos = name.rfind('.')?;
    let ext = &name[pos + 1..];
    let prefix = &name[..pos];
    let valid = !ext.is_empty() && !ext.contains(' ') && prefix.chars().any(|c| c != '.');
    valid.then_some(ext)
}

/// Split a file name into `(base, extension)` for collision renaming, using the
/// same Swift-equivalent extension rule as [`path_extension`] (this is what
/// upstream's `uniqueURL` relies on via `deletingPathExtension`/`pathExtension`).
/// `"a.b.ext"` → (`"a.b"`, `Some("ext")`); names without a recognized extension
/// (e.g. `".hidden"`, `"..mp4"`, `"trailing."`) → (`name`, `None`).
fn split_extension(name: &str) -> (String, Option<String>) {
    match path_extension(name) {
        Some(ext) => {
            let base_len = name.len() - ext.len() - 1; // drop ".<ext>"
            (name[..base_len].to_string(), Some(ext.to_string()))
        }
        None => (name.to_string(), None),
    }
}

// --- IO helpers ---

fn file_name_str(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Purely lexical path normalization for dedup keying, matching the semantics
/// of Swift's `URL.standardizedFileURL.path`: collapse `.` components,
/// resolve `..` textually, and fold repeated separators — all **without**
/// touching the filesystem and **without** resolving symlinks. Two distinct
/// symlink paths pointing at one physical file thus produce different keys and
/// are copied separately, exactly as upstream's exporter does.
///
/// `..` is resolved textually, never against the real filesystem:
/// - after a *normal* segment, it pops that segment;
/// - right after the root (or a Windows prefix root), it is absorbed — you
///   cannot go above `/`, so `/../x` standardizes to `/x`, matching
///   `URL.standardizedFileURL`;
/// - in a *relative* path with no segment to pop, a leading `..` is kept
///   verbatim (`../x` stays `../x`).
///
/// This intentionally does **not** stat anything, because the dedup key must be
/// byte-for-byte equivalent to the upstream key regardless of what the disk
/// currently contains.
fn standardize(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut out = PathBuf::new();
    let mut has_root = false;
    for component in path.components() {
        match component {
            // Drop "." segments and any folded-away repeated separators.
            Component::CurDir => {}
            Component::ParentDir => {
                match out.components().next_back() {
                    // Pop a preceding normal segment.
                    Some(Component::Normal(_)) => {
                        out.pop();
                    }
                    // Absorb ".." that sits directly on the root ("/.." -> "/").
                    _ if has_root => {}
                    // Relative path with nothing to pop: keep the leading "..".
                    _ => out.push(component.as_os_str()),
                }
            }
            // Root, prefix (Windows), and normal segments are kept as-is.
            other => {
                if matches!(other, Component::RootDir | Component::Prefix(_)) {
                    has_root = true;
                }
                out.push(other.as_os_str());
            }
        }
    }
    if out.as_os_str().is_empty() {
        // An all-"." relative path standardizes to ".", like the upstream URL.
        out.push(Component::CurDir.as_os_str());
    }
    out
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| ProjectError::io(path, e))
}

fn copy_file(src: &Path, dest: &Path) -> Result<()> {
    fs::copy(src, dest)
        .map(|_| ())
        .map_err(|e| ProjectError::io(dest, e))
}

fn write_json<T: serde::Serialize>(dest: &Path, file_name: &str, value: &T) -> Result<()> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| ProjectError::json(file_name, e))?;
    fs::write(dest, json).map_err(|e| ProjectError::io(dest, e))
}

/// Copy a single file only when the source exists. Missing source is a no-op.
fn copy_if_present(src: &Path, dest: &Path) -> Result<()> {
    if !src.is_file() {
        return Ok(());
    }
    fs::copy(src, dest)
        .map(|_| ())
        .map_err(|e| ProjectError::io(dest, e))
}

/// Recursively copy a directory only when it exists. Missing source is a no-op.
fn copy_dir_if_present(src: &Path, dest: &Path) -> Result<()> {
    if !src.is_dir() {
        return Ok(());
    }
    copy_dir_recursive(src, dest)
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{ClipType, MediaManifestEntry};

    /// Minimal `External` manifest entry for archive tests.
    fn external_entry(id: &str, absolute_path: &str) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.into(),
            name: id.into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: absolute_path.into(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        }
    }

    /// `standardize` is purely lexical: it folds `.`, `..`, and repeated
    /// separators with no filesystem access (matching `standardizedFileURL`).
    #[test]
    fn standardize_is_purely_lexical() {
        assert_eq!(
            standardize(Path::new("/a/./b/../c//d")),
            PathBuf::from("/a/c/d")
        );
        // Trailing "." and redundant separators collapse.
        assert_eq!(standardize(Path::new("/a/b/.")), PathBuf::from("/a/b"));
        // ".." past the root is kept (no real-FS resolution); root preserved.
        assert_eq!(standardize(Path::new("/../x")), PathBuf::from("/x"));
        // A leading ".." in a relative path is preserved verbatim.
        assert_eq!(standardize(Path::new("../x/./y")), PathBuf::from("../x/y"));
        // Identical strings normalize identically (dedup still works).
        assert_eq!(
            standardize(Path::new("/m/n.mp4")),
            standardize(Path::new("/m/./n.mp4"))
        );
    }

    /// Two *different* symlink paths to the same physical file standardize to
    /// two *different* keys — so dedup does NOT merge them, matching upstream's
    /// `srcURL.standardizedFileURL.path` (which never resolves symlinks).
    #[cfg(unix)]
    #[test]
    fn standardize_does_not_resolve_symlinks() {
        let dir = TestDir::new("standardize_symlink");
        let real = dir.path().join("real.mp4");
        fs::write(&real, b"x").unwrap();
        let link_a = dir.path().join("link-a.mp4");
        let link_b = dir.path().join("link-b.mp4");
        std::os::unix::fs::symlink(&real, &link_a).unwrap();
        std::os::unix::fs::symlink(&real, &link_b).unwrap();

        // Both symlinks point at one inode, but the lexical keys differ.
        assert_ne!(standardize(&link_a), standardize(&link_b));
        // And each key is the symlink's own path, not the resolved target.
        assert_eq!(standardize(&link_a), link_a);
        assert_eq!(standardize(&link_b), link_b);
    }

    /// End-to-end对拍 against upstream: when two manifest entries reference the
    /// same physical file through two distinct symlinks, upstream copies the
    /// file twice (it dedups by `standardizedFileURL.path`, which differs per
    /// symlink). The Rust archiver must do the same: two `media/` files, both
    /// ids `collected`, and `total_bytes` counting both copies.
    #[cfg(unix)]
    #[test]
    fn two_symlinks_to_one_file_are_not_deduped() {
        let dir = TestDir::new("archive_symlink_dedup");
        let real = dir.path().join("real.mp4");
        let payload = b"hello-media";
        fs::write(&real, payload).unwrap();
        let link_a = dir.path().join("link-a.mp4");
        let link_b = dir.path().join("link-b.mp4");
        std::os::unix::fs::symlink(&real, &link_a).unwrap();
        std::os::unix::fs::symlink(&real, &link_b).unwrap();

        let timeline = Timeline::new();
        let manifest = MediaManifest {
            version: 2,
            entries: vec![
                external_entry("aaaaaaaa-id", link_a.to_str().unwrap()),
                external_entry("bbbbbbbb-id", link_b.to_str().unwrap()),
            ],
            folders: Vec::new(),
        };
        let log = GenerationLog::new();
        let dest = dir.path().join("Out.opentake");

        let report = archive(&timeline, &manifest, &log, None, &dest).unwrap();

        // No dedup: both entries collected, two physical copies, both counted.
        assert_eq!(report.collected.len(), 2);
        assert!(report.missing.is_empty());
        assert_eq!(report.total_bytes, (payload.len() as u64) * 2);

        let copied: Vec<_> = fs::read_dir(layout::media_dir(&dest))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(copied.len(), 2, "expected two copies, got {copied:?}");
    }

    /// A scratch directory under the system temp dir, removed on drop.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir()
                .join(format!("opentake-archive-{tag}-{}-{n}", std::process::id()));
            fs::create_dir_all(&p).unwrap();
            TestDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn split_extension_handles_dots() {
        assert_eq!(
            split_extension("clip.mov"),
            ("clip".into(), Some("mov".into()))
        );
        assert_eq!(
            split_extension("my.clip.mov"),
            ("my.clip".into(), Some("mov".into()))
        );
        assert_eq!(split_extension("noext"), ("noext".into(), None));
        assert_eq!(split_extension(".hidden"), (".hidden".into(), None));
        assert_eq!(split_extension("trailing."), ("trailing.".into(), None));
        // All-leading-dot names have no extension under the Swift rule, so the
        // collision-rename base is the whole name (upstream: `..mp4` -> `..mp4-1`,
        // NOT Rust-rfind's `.-1.mp4`).
        assert_eq!(split_extension("..mp4"), ("..mp4".into(), None));
        assert_eq!(split_extension("...mp4"), ("...mp4".into(), None));
        // A non-dot char anywhere in the prefix makes the extension valid.
        assert_eq!(split_extension("a..mp4"), ("a.".into(), Some("mp4".into())));
    }

    /// Direct对拍 of [`path_extension`] against the Foundation values captured
    /// from `swift` (`URL.pathExtension` / `NSString.pathExtension`): only a
    /// non-empty, space-free segment whose prefix has a non-`.` char counts.
    #[test]
    fn path_extension_matches_foundation() {
        // Normal extensions agree with both Swift and `Path::extension`.
        assert_eq!(path_extension("foo.mp4"), Some("mp4"));
        assert_eq!(path_extension("my.clip.mov"), Some("mov"));
        assert_eq!(path_extension("a.b.ext"), Some("ext"));
        // Space inside the extension segment -> no extension (Foundation rule).
        assert_eq!(path_extension("foo. mp4"), None);
        assert_eq!(path_extension("foo.mp "), None);
        assert_eq!(path_extension("foo.m p"), None);
        // All-leading-dot names -> no extension (prefix is all `.`).
        assert_eq!(path_extension("..mp4"), None);
        assert_eq!(path_extension("...mp4"), None);
        assert_eq!(path_extension(".hidden"), None);
        // Empty extension segment / no dot.
        assert_eq!(path_extension("trailing."), None);
        assert_eq!(path_extension("noext"), None);
        // A non-`.` char (even a space) in the prefix validates the extension.
        assert_eq!(path_extension(" .mp4"), Some("mp4"));
        assert_eq!(path_extension("a..mp4"), Some("mp4"));
        // Tabs/newlines are NOT rejected by Foundation, only ASCII space.
        assert_eq!(path_extension("foo.m\tp"), Some("m\tp"));
    }

    #[test]
    fn filename_for_external_uses_id_prefix() {
        let src = MediaSource::External {
            absolute_path: "/abs/whatever.mp4".into(),
        };
        let name = filename_for("abcdef0123456789", &src, Path::new("/abs/whatever.mp4"));
        assert_eq!(name, "import-abcdef01.mp4");
        // No extension on source -> no extension on name.
        let name2 = filename_for("abcdef0123456789", &src, Path::new("/abs/whatever"));
        assert_eq!(name2, "import-abcdef01");

        // Anomalous source names follow Swift `URL.pathExtension`, not
        // `Path::extension`: a space in the extension drops it entirely.
        // (`Path::extension` would have produced `import-abcdef01. mp4`.)
        let space_ext = MediaSource::External {
            absolute_path: "/abs/foo. mp4".into(),
        };
        assert_eq!(
            filename_for("abcdef0123456789", &space_ext, Path::new("/abs/foo. mp4")),
            "import-abcdef01"
        );
        // All-leading-dot source name -> no extension.
        // (`Path::extension` would have produced `import-abcdef01.mp4`.)
        let dots = MediaSource::External {
            absolute_path: "/abs/..mp4".into(),
        };
        assert_eq!(
            filename_for("abcdef0123456789", &dots, Path::new("/abs/..mp4")),
            "import-abcdef01"
        );
    }

    #[test]
    fn filename_for_project_keeps_original() {
        let src = MediaSource::Project {
            relative_path: "media/keep.mov".into(),
        };
        let name = filename_for("id", &src, Path::new("/proj/media/keep.mov"));
        assert_eq!(name, "keep.mov");
    }

    #[test]
    fn resolve_source_external_and_project() {
        let ext = MediaSource::External {
            absolute_path: "/x/a.mp4".into(),
        };
        assert_eq!(
            resolve_source(&ext, Some(Path::new("/proj"))),
            Some(PathBuf::from("/x/a.mp4"))
        );
        let proj = MediaSource::Project {
            relative_path: "media/b.mov".into(),
        };
        assert_eq!(
            resolve_source(&proj, Some(Path::new("/proj"))),
            Some(PathBuf::from("/proj/media/b.mov"))
        );
        assert_eq!(resolve_source(&proj, None), None);
    }
}
