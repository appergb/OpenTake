//! Shared test utilities: a dependency-free temporary directory that cleans
//! itself up on drop. Avoids pulling `tempfile` (and its rustix/getrandom
//! chain) into the build just for tests.
//!
//! This module is compiled independently into each integration-test binary, so
//! helpers used by one test file look "dead" to another; `#[allow(dead_code)]`
//! on the rarely-used accessors keeps the shared API coherent without per-binary
//! warnings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A throwaway directory under the OS temp dir, removed when dropped.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Create a fresh unique directory. Uniqueness: pid + a process-global
    /// counter (test binaries are single-process; this is collision-free here).
    pub fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("opentake-test-{}-{}-{}", tag, std::process::id(), n);
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempDir { path }
    }

    /// The directory path.
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A child path inside this temp dir (not created).
    pub fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Write a file, creating parent directories as needed.
pub fn write_file(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(path, contents).expect("write file");
}

/// A nofollow receipt entry for proving that an operation left a complete
/// directory tree unchanged, including unknown files and symlinks.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub enum TreeReceiptEntry {
    Directory,
    File(Vec<u8>),
    Symlink(PathBuf),
    Other,
}

/// Record every entry below `root` without following symlinks.
#[allow(dead_code)]
pub fn tree_receipt(root: &Path) -> BTreeMap<PathBuf, TreeReceiptEntry> {
    fn collect(root: &Path, directory: &Path, receipt: &mut BTreeMap<PathBuf, TreeReceiptEntry>) {
        let mut entries = std::fs::read_dir(directory)
            .expect("read receipt directory")
            .map(|entry| entry.expect("read receipt entry"))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("receipt entry stays below root")
                .to_path_buf();
            let file_type = entry.file_type().expect("read receipt entry type");
            if file_type.is_dir() {
                receipt.insert(relative, TreeReceiptEntry::Directory);
                collect(root, &path, receipt);
            } else if file_type.is_symlink() {
                receipt.insert(
                    relative,
                    TreeReceiptEntry::Symlink(
                        std::fs::read_link(&path).expect("read receipt symlink"),
                    ),
                );
            } else if file_type.is_file() {
                receipt.insert(
                    relative,
                    TreeReceiptEntry::File(std::fs::read(&path).expect("read receipt file")),
                );
            } else {
                receipt.insert(relative, TreeReceiptEntry::Other);
            }
        }
    }

    let metadata = std::fs::symlink_metadata(root).expect("read receipt root metadata");
    let mut receipt = BTreeMap::new();
    let root_key = PathBuf::from(".");
    if metadata.file_type().is_symlink() {
        receipt.insert(
            root_key,
            TreeReceiptEntry::Symlink(std::fs::read_link(root).expect("read receipt root symlink")),
        );
    } else if metadata.is_dir() {
        receipt.insert(root_key, TreeReceiptEntry::Directory);
        collect(root, root, &mut receipt);
    } else if metadata.is_file() {
        receipt.insert(
            root_key,
            TreeReceiptEntry::File(std::fs::read(root).expect("read receipt root file")),
        );
    } else {
        receipt.insert(root_key, TreeReceiptEntry::Other);
    }
    receipt
}
