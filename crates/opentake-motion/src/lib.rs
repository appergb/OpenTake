//! opentake-motion — the Web motion-graphics module (Issue #14,
//! docs/MOTION-GRAPHICS-PLUGIN.md).
//!
//! The agent (or a community template) authors an animation in HTML/CSS/JS; this
//! crate renders it **deterministically** to a sequence of RGBA frames (with
//! alpha), content-hash caches the result, and exposes it as an ordinary clip
//! source so the `opentake-render` wgpu compositor blends it like any other
//! texture layer.
//!
//! ## Pipeline
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
//!                                 └─ opentake-render compositor (one texture layer)
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
//! Untrusted motion code runs under a [`sandbox::SandboxPolicy`]: network denied
//! by default (explicit allowlist only), a render timeout fuse, and a document
//! size ceiling. See [`sandbox`].
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
pub use integration::{FrameDecoder, MotionClipSource};
pub use manifest::{
    DurationMode, DurationSpec, FpsPolicy, MotionPlugin, MotionPluginAuthor, ParamSpec,
};
pub use renderer::{
    deterministic_clock_script, HeadlessChromiumRenderer, MotionRenderer, StubRenderer,
};
pub use sandbox::{AllowedOrigin, SandboxPolicy};
pub use source::{limits, MotionRenderRequest, MotionSource, ParamValue, RenderedClip};
