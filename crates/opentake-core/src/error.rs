//! `CoreError` — the unified error surface of the assembly layer.
//!
//! `opentake-core` orchestrates three lower layers, each with its own error
//! type: editing ([`opentake_ops::EditError`]) and persistence
//! ([`opentake_project::ProjectError`]). This enum folds them into one type the
//! Tauri command surface (and the in-app agent) can map uniformly, and adds the
//! handful of conditions that only exist at the assembly level (no project open,
//! a backend that is not wired yet).
//!
//! The split between [`CoreError::Edit`] (a *validation* failure — the caller's
//! input was rejected, the document is untouched) and [`CoreError::Internal`] /
//! [`CoreError::Project`] (an IO / decode failure) mirrors the `code:
//! "validation"` vs `code: "internal"` distinction in `core-SPEC.md` §6.3.

use opentake_ops::EditError;
use opentake_project::ProjectError;

/// Anything that can go wrong driving the assembly layer.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// A command was rejected by the editing layer (bad index, missing clip,
    /// ripple refusal, ...). The document is unchanged and the version did not
    /// move. Maps to the `validation` error class.
    #[error("{0}")]
    Edit(#[from] EditError),

    /// A project bundle read/write failed. Maps to the `internal` error class.
    #[error("{0}")]
    Project(#[from] ProjectError),

    /// An operation needed an open project but none is loaded
    /// (e.g. `save_project` before `open_project`/`new_project`, or a save with
    /// no path and no remembered project directory).
    #[error("no project is open")]
    NoProjectOpen,

    /// A capability backend (preview / export / media import / generation) was
    /// invoked but is not wired in this build. Carries the backend name so the
    /// caller can surface a precise message. This is how the unfinished
    /// render/media/agent/gen modules are kept decoupled without `todo!()`.
    #[error("capability not available in this build: {0}")]
    Unsupported(&'static str),

    /// A media-library operation was rejected by input validation — e.g. a
    /// relink target whose asset id is unknown or whose type does not match the
    /// original (mirrors upstream's relink type-mismatch refusal). The catalog is
    /// unchanged. Maps to the `validation` error class.
    #[error("{0}")]
    Media(String),
}

/// Convenience alias for fallible assembly-layer operations.
pub type Result<T> = std::result::Result<T, CoreError>;

impl CoreError {
    /// Machine-readable error class for the Tauri boundary (`core-SPEC.md` §6.3):
    /// `"validation"` for rejected input, `"internal"` for everything else.
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::Edit(_) | CoreError::Media(_) => "validation",
            CoreError::Project(_) | CoreError::NoProjectOpen | CoreError::Unsupported(_) => {
                "internal"
            }
        }
    }
}
