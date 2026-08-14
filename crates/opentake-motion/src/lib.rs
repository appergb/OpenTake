//! opentake-motion — native motion fallback primitives (Issue #34,
//! docs/MOTION-GRAPHICS-PLUGIN.md).
//!
//! The v1 motion/AI-video plan uses a forked Motion Canvas plugin to render a
//! materialized video file that OpenTake imports as ordinary media. This crate is
//! kept as the native fallback layer for later work: RGBA frame sequences,
//! transparent alpha overlays, content-hash caches, and an optional HTML/CSS
//! renderer.
//!
//! ## Fallback pipeline
//!
//! ```text
//! MotionSource (Code | Template+params)
//!   └─ MotionRenderRequest (fps, duration_frames, w, h, transparent)  [validated]
//!        └─ content_hash ──▶ MotionCache  (hit → reuse frames)
//!             └─ MotionRenderer::render   (miss → render)
//!                  ├─ StubRenderer            (deterministic, browser-free; tests)
//!                  └─ HeadlessChromiumRenderer (CDP virtual-time; behind `chromium`)
//!                       └─ RenderedClip (on-disk RGBA PNG frames)
//!                            └─ MotionClipSource: impl SourceMetrics + FrameProvider
//!                                 └─ opentake-render compositor (future texture layer)
//! ```
//!
//! ## Determinism & caching
//!
//! Renderers MUST be reproducible (preview == export). The cache key
//! ([`cache::content_hash`]) is a SHA-256 over the source, params, fps, size, and
//! transparency, so identical inputs reuse frames and any change invalidates them.
//!
//! ## Security
//!
//! Untrusted native fallback code runs under a [`sandbox::SandboxPolicy`]:
//! network denied by default (explicit allowlist only), a render timeout fuse,
//! and a document size ceiling. See [`sandbox`].
//!
//! ## Module map
//! - [`source`]   — value types: [`MotionSource`], [`MotionRenderRequest`], [`RenderedClip`].
//! - [`manifest`] — the template `plugin.json` model: [`MotionPlugin`].
//! - [`renderer`] — the [`MotionRenderer`] trait + [`StubRenderer`] + [`HeadlessChromiumRenderer`].
//! - [`cache`]    — [`content_hash`](cache::content_hash) + [`MotionCache`].
//! - [`sandbox`]  — [`SandboxPolicy`] and its pure checks.
//! - [`integration`] — [`MotionClipSource`]: the `opentake-render` bridge.
//! - [`error`]    — [`MotionError`] / [`MotionResult`].

pub mod cache;
pub mod error;
pub mod integration;
pub mod manifest;
pub mod renderer;
pub mod sandbox;
pub mod source;

// Flat re-export of the public API for ergonomic downstream use.
pub use cache::{content_hash, MotionCache};
pub use error::{MotionError, MotionResult};
pub use integration::{
    read_single_preview_png, FrameDecoder, MotionClipSource, MAX_PREVIEW_PNG_BYTES,
};
pub use manifest::{
    DurationMode, DurationSpec, FpsPolicy, MotionPlugin, MotionPluginAuthor, ParamSpec,
};
pub use renderer::{
    deterministic_clock_script, HeadlessChromiumRenderer, MotionCancellationToken, MotionRenderer,
    StubRenderer,
};
pub use sandbox::{AllowedOrigin, SandboxPolicy, OFFLINE_DOCUMENT_CSP};
pub use source::{
    limits, MotionDocumentSource, MotionRenderRequest, MotionSource, MotionSourceDiagnostic,
    ParamValue, RenderedClip,
};
