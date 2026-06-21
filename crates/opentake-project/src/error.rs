//! Error type for project bundle IO and (de)serialization.

use std::path::PathBuf;

/// Failures from reading, writing, or archiving an `.opentake` bundle.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// The bundle is missing the mandatory `project.json`. Mirrors upstream's
    /// `fileReadCorruptFile` when `project.json` is absent.
    #[error("missing required {file} in bundle at {bundle}")]
    MissingTimeline {
        /// The expected file name (`project.json`).
        file: &'static str,
        /// The bundle directory that was inspected.
        bundle: PathBuf,
    },

    /// The given path is not a directory (an `.opentake` bundle is a directory).
    #[error("not a project bundle directory: {0}")]
    NotABundle(PathBuf),

    /// A filesystem operation failed. `path` records what we were touching.
    #[error("io error at {path}: {source}")]
    Io {
        /// The path involved in the failed operation.
        path: PathBuf,
        /// The underlying IO error.
        source: std::io::Error,
    },

    /// JSON (de)serialization of a bundle component failed. `file` records
    /// which component (e.g. `project.json`).
    #[error("failed to parse {file}: {source}")]
    Json {
        /// The bundle file whose JSON failed.
        file: String,
        /// The underlying serde error.
        source: serde_json::Error,
    },
}

impl ProjectError {
    /// Wrap an [`std::io::Error`] with the path it occurred at.
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        ProjectError::Io {
            path: path.into(),
            source,
        }
    }

    /// Wrap a [`serde_json::Error`] with the bundle file it came from.
    pub(crate) fn json(file: impl Into<String>, source: serde_json::Error) -> Self {
        ProjectError::Json {
            file: file.into(),
            source,
        }
    }
}

/// Convenience alias for results in this crate.
pub type Result<T> = std::result::Result<T, ProjectError>;
