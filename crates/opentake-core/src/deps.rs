//! `CoreDeps` — injected handles to the capability layers the core orchestrates
//! but does not implement (preview, export, media import, generation).
//!
//! `core-SPEC.md` §5.2 makes these *injected trait objects* rather than hard
//! `use`s of concrete functions, so the assembly layer stays decoupled from the
//! still-unfinished `opentake-render` / `opentake-media` / `opentake-gen`
//! crates and remains unit-testable with stubs.
//!
//! ## Placeholder discipline (no `todo!()` on a reachable path)
//!
//! These traits are the seams where later phases plug in. Until those crates
//! land, the core ships [`UnsupportedBackends`], whose methods return
//! [`CoreError::Unsupported`] — a real, recoverable error value, never a panic.
//! That keeps the whole crate compiling and every code path exercisable: a test
//! (or the front end) that calls `seek` before the render backend exists gets a
//! clean `Unsupported("preview")` error instead of a crash. The real backends
//! will implement these same traits in their own phases without touching the
//! core.

use std::sync::Arc;

use crate::error::{CoreError, Result};

/// Drives preview/scrub playback. The core's `seek` clamps a frame and forwards
/// it here; the backend decodes + composites and (out of band) signals readiness
/// (`core-SPEC.md` §3.4). Implemented by `opentake-render` in a later phase.
pub trait PreviewBackend: Send + Sync {
    /// Request that frame `frame` be composited. `interactive` marks a scrub
    /// (throttled, draft quality) vs an exact seek. Returns `Ok(())` once the
    /// request is accepted; the resulting pixels are delivered out of band.
    fn request_frame(&self, frame: i32, interactive: bool) -> Result<()>;
}

/// Starts background export jobs and streams progress out of band
/// (`core-SPEC.md` §5.3, §6.2). Implemented by `opentake-render` in a later
/// phase. The opaque `spec_json` carries the export options until the concrete
/// `ExportOptions` type lands with the backend.
pub trait ExportBackend: Send + Sync {
    /// Begin an export described by `spec_json`; returns an opaque job id.
    fn start_export(&self, spec_json: &str) -> Result<String>;
}

/// Imports media (local path / URL / bytes), materializes a runtime asset, and
/// kicks off thumbnail/waveform generation (`core-SPEC.md` §5.3). Implemented by
/// `opentake-media` in a later phase. The opaque `source_json` carries the
/// import source until the concrete `ImportSource` type lands with the backend.
pub trait MediaImporter: Send + Sync {
    /// Import the media described by `source_json`; returns the new asset id.
    fn import(&self, source_json: &str) -> Result<String>;
}

/// Runs AI generation jobs (BYOK / managed) and streams status out of band
/// (`core-SPEC.md` §5.2; `opentake-gen`, latest phase). Optional — absent in
/// early builds.
pub trait GenBackend: Send + Sync {
    /// Begin a generation described by `request_json`; returns an opaque job id.
    fn start_generation(&self, request_json: &str) -> Result<String>;
}

/// The bundle of capability handles the core holds. Cloning is cheap (every
/// field is an `Arc`). `gen` is optional and `None` until generation is wired.
#[derive(Clone)]
pub struct CoreDeps {
    /// Preview/scrub backend.
    pub preview: Arc<dyn PreviewBackend>,
    /// Export backend.
    pub export: Arc<dyn ExportBackend>,
    /// Media import backend.
    pub media: Arc<dyn MediaImporter>,
    /// Generation backend (optional; `None` in early builds).
    pub gen: Option<Arc<dyn GenBackend>>,
}

impl Default for CoreDeps {
    /// The placeholder wiring: every capability reports
    /// [`CoreError::Unsupported`]. This is the default the core runs with until
    /// the render/media/gen crates land, and what tests use to exercise the
    /// assembly layer in isolation.
    fn default() -> Self {
        let stub = Arc::new(UnsupportedBackends);
        CoreDeps {
            preview: stub.clone(),
            export: stub.clone(),
            media: stub,
            gen: None,
        }
    }
}

/// A unit struct implementing every capability trait with a recoverable
/// [`CoreError::Unsupported`]. Used as the default backend set; replaced
/// per-capability as the real crates come online.
pub struct UnsupportedBackends;

impl PreviewBackend for UnsupportedBackends {
    fn request_frame(&self, _frame: i32, _interactive: bool) -> Result<()> {
        Err(CoreError::Unsupported("preview"))
    }
}

impl ExportBackend for UnsupportedBackends {
    fn start_export(&self, _spec_json: &str) -> Result<String> {
        Err(CoreError::Unsupported("export"))
    }
}

impl MediaImporter for UnsupportedBackends {
    fn import(&self, _source_json: &str) -> Result<String> {
        Err(CoreError::Unsupported("media"))
    }
}

impl GenBackend for UnsupportedBackends {
    fn start_generation(&self, _request_json: &str) -> Result<String> {
        Err(CoreError::Unsupported("gen"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_deps_report_unsupported_not_panic() {
        let deps = CoreDeps::default();
        assert!(matches!(
            deps.preview.request_frame(0, false),
            Err(CoreError::Unsupported("preview"))
        ));
        assert!(matches!(
            deps.export.start_export("{}"),
            Err(CoreError::Unsupported("export"))
        ));
        assert!(matches!(
            deps.media.import("{}"),
            Err(CoreError::Unsupported("media"))
        ));
        assert!(deps.gen.is_none());
    }
}
