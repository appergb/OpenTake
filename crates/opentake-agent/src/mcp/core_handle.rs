//! `CoreHandle` — the testable boundary between the dispatch shell and
//! `opentake-core` (`agent-SPEC.md` §8.1).
//!
//! The dispatcher never touches [`opentake_core::AppCore`] directly; it talks to
//! this trait. Production wiring passes an [`AppCoreHandle`] (a thin delegating
//! wrapper); tests can pass a fake in-memory handle. Keeping the surface this
//! narrow (read timeline, read media, apply one command, ask for the project dir)
//! means the whole tool-dispatch pipeline is unit-testable without a UI or a
//! transport.

use std::path::PathBuf;

use opentake_core::AppCore;
use opentake_domain::{MediaManifest, MediaResolver, Timeline};
use opentake_media::{extract_pcm, PcmBuffer, PcmSpec};
use opentake_ops::command::{EditCommand, EditResult};

/// The narrow document surface the dispatch shell needs. `Send + Sync` so a
/// `Dispatcher` holding `Arc<dyn CoreHandle>` stays shareable across threads
/// (matching [`AppCore`]'s cross-client design).
pub trait CoreHandle: Send + Sync {
    /// The current timeline snapshot (the `get_timeline` source + before/after
    /// snapshots the shell takes around every tool).
    fn timeline(&self) -> Timeline;

    /// The current media manifest (the `get_media` / `list_folders` source + the
    /// id universe for short-id expansion/shortening).
    fn media(&self) -> MediaManifest;

    /// Apply one editing command, mapping the core error into `anyhow` so the
    /// shell can turn any failure into a single `ToolResult::error`.
    fn apply(&self, cmd: EditCommand) -> anyhow::Result<EditResult>;

    /// The open project's bundle directory, or `None` for an unsaved project.
    fn project_dir(&self) -> Option<PathBuf>;

    /// Resolve an asset id to the local file path that media analysis can read.
    /// The default implementation mirrors `MediaResolver.expected_path`.
    fn media_path(&self, media_ref: &str) -> Option<PathBuf> {
        let manifest = self.media();
        let project_dir = self.project_dir();
        MediaResolver::new(&manifest, project_dir.as_deref()).expected_path(media_ref)
    }

    /// Decode a media asset's first audio track into the PCM format requested by
    /// an analysis tool. Test handles can override this to inject synthetic PCM
    /// without invoking ffmpeg.
    fn extract_analysis_pcm(
        &self,
        media_ref: &str,
        spec: PcmSpec,
        range: Option<(f64, f64)>,
    ) -> anyhow::Result<PcmBuffer> {
        let path = self
            .media_path(media_ref)
            .ok_or_else(|| anyhow::anyhow!("media path not found for mediaRef: {media_ref}"))?;
        extract_pcm(&path, &spec, range).map_err(|e| anyhow::anyhow!("{e}"))
    }
}

/// Production [`CoreHandle`] over the authoritative [`AppCore`]. A clone of the
/// `AppCore` points at the same session, so this can be constructed per request
/// without copying any document state.
pub struct AppCoreHandle(pub AppCore);

impl AppCoreHandle {
    /// Wrap an [`AppCore`] handle.
    pub fn new(core: AppCore) -> Self {
        AppCoreHandle(core)
    }
}

impl CoreHandle for AppCoreHandle {
    fn timeline(&self) -> Timeline {
        self.0.get_timeline().timeline
    }

    fn media(&self) -> MediaManifest {
        self.0.media()
    }

    fn apply(&self, cmd: EditCommand) -> anyhow::Result<EditResult> {
        self.0.apply(cmd).map_err(|e| anyhow::anyhow!("{e}"))
    }

    fn project_dir(&self) -> Option<PathBuf> {
        self.0.project_dir()
    }
}
