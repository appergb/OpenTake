//! Shared test utilities: a dependency-free temporary directory that cleans
//! itself up on drop. Avoids pulling `tempfile` (and its rustix/getrandom
//! chain) into the build just for tests.
//!
//! This module is compiled independently into each integration-test binary, so
//! helpers used by one test file look "dead" to another; `#[allow(dead_code)]`
//! on the rarely-used accessors keeps the shared API coherent without per-binary
//! warnings.

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
