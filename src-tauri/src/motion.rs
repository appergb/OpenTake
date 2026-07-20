//! Motion-graphics rendering command surface (Issue #34).
//!
//! Wires `opentake-motion`'s render pipeline into the Tauri command layer. The
//! front end calls [`render_motion_clip`] with a `MotionSource` (inline code or
//! template + params) and render parameters; the command builds a
//! [`MotionRenderRequest`], checks the content-hash cache, and renders the frame
//! sequence to disk. The return value is a `motion://<hash>` media_ref the
//! timeline can assign to a clip's `media_ref` — the compositor recognizes the
//! prefix (see `opentake_domain::is_motion_ref`) and routes it to a
//! `MotionClipSource` instead of the file decoder.
//!
//! ## Renderer dispatch
//!
//! - **Default (no `chromium` feature):** [`StubRenderer`] — deterministic,
//!   browser-free, always works. Produces solid-color frames derived from the
//!   content hash so the full pipeline (cache → frame files → compositor) is
//!   exercised end-to-end without a browser.
//! - **With `chromium` feature + Chrome binary:** [`HeadlessChromiumRenderer`]
//!   is attempted first. Until the CDP client is implemented (Issue #14 TODO),
//!   it returns `RendererUnavailable`; the command then falls back to
//!   [`StubRenderer`] so a missing/unimplemented browser never hard-fails the
//!   render.
//!
//! ## Cache
//!
//! The content-hash cache ([`MotionCache`]) means an identical request reuses
//! already-rendered frames without re-rendering — the command returns the hash
//! immediately on a cache hit.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::State;

use opentake_domain::motion_ref_for_hash;
use opentake_motion::{
    content_hash, MotionCache, MotionRenderRequest, MotionRenderer, MotionSource, SandboxPolicy,
    StubRenderer,
};
// `HeadlessChromiumRenderer` is only used on the `chromium` feature path; keep
// the import conditional so the default build stays warning-free.
#[cfg(feature = "chromium")]
use opentake_motion::HeadlessChromiumRenderer;

/// Managed state: the motion frame cache root + sandbox policy. Mirrors
/// `MediaState` / `RenderState`. `Send + Sync` (all fields are).
pub struct MotionState {
    cache: MotionCache,
    policy: SandboxPolicy,
}

impl MotionState {
    /// Build motion state rooted at `cache_root` (typically
    /// `<app_cache_dir>/motion-cache`).
    pub fn new(cache_root: PathBuf) -> Self {
        MotionState {
            cache: MotionCache::new(cache_root),
            policy: SandboxPolicy::default(),
        }
    }

    /// The frame cache.
    pub fn cache(&self) -> &MotionCache {
        &self.cache
    }
}

/// Render parameters mirroring the non-source fields of
/// [`MotionRenderRequest`]. camelCase to match the existing DTO surface.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionRenderParamsDto {
    /// Timeline frames per second.
    pub fps: u32,
    /// Number of frames to produce.
    pub duration_frames: u32,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Whether to capture a straight-alpha overlay (transparent body). Defaults
    /// to `false` here (the caller sends an explicit flag); the motion crate's
    /// `MotionRenderRequest::new` defaults to `true`, but the DTO round-trips
    /// the caller's explicit choice.
    #[serde(default)]
    pub transparent: bool,
}

/// The result of [`render_motion_clip`]: the `motion://<hash>` media_ref plus
/// rendered-clip metadata. camelCase serde for the WebView.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionRenderResultDto {
    /// The `motion://<content_hash>` media_ref to assign to a clip's
    /// `media_ref`. The compositor recognizes the prefix and routes the clip to
    /// a `MotionClipSource`.
    pub media_ref: String,
    /// The content hash (SHA-256 hex) naming the cache directory.
    pub content_hash: String,
    /// Number of frames rendered (== `duration_frames`).
    pub frame_count: usize,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Whether the frames carry straight alpha.
    pub transparent: bool,
    /// Which renderer produced the frames: `"cache"` (cache hit), `"stub"`, or
    /// `"chromium"`. Lets the caller/UI surface whether real rendering occurred.
    pub renderer: String,
}

/// `render_motion_clip`: render a motion-graphic source to a frame sequence.
///
/// Takes a [`MotionSource`] (inline code or template + params) + render params,
/// builds a [`MotionRenderRequest`], checks the content-hash cache, and renders
/// via [`StubRenderer`] (the deterministic offline backend). Returns a
/// `motion://<hash>` media_ref the timeline can assign to a clip.
///
/// When the `chromium` feature is compiled in AND a Chrome binary is available,
/// the CDP backend ([`HeadlessChromiumRenderer`]) is attempted first; on
/// `RendererUnavailable` it falls back to [`StubRenderer`] so the command never
/// hard-fails just because Chrome is absent or the CDP path is unimplemented.
#[tauri::command]
pub fn render_motion_clip(
    state: State<'_, MotionState>,
    source: MotionSource,
    params: MotionRenderParamsDto,
) -> Result<MotionRenderResultDto, String> {
    let req = MotionRenderRequest::new(
        source,
        params.fps,
        params.duration_frames,
        params.width,
        params.height,
    )
    .with_transparent(params.transparent);
    render_motion(state.cache(), state.policy.clone(), &req).map_err(|e| {
        // Log server-side so a render failure is visible even if the frontend
        // swallows the error (mirrors `get_waveform` error logging).
        eprintln!(
            "render_motion_clip failed: hash={} error={e}",
            content_hash(&req)
        );
        e.to_string()
    })
}

/// Core render logic, extracted from the Tauri command shim for direct testing
/// (mirrors `import_one` / `probe_media` in `media.rs`). The command is a thin
/// wrapper over this; tests drive it with a temp-dir cache.
///
/// 1. Validate the request (ranges + source).
/// 2. Compute the content hash.
/// 3. Cache hit → return immediately with `renderer: "cache"`.
/// 4. Cache miss → render (chromium if available, else stub), return the clip.
fn render_motion(
    cache: &MotionCache,
    policy: SandboxPolicy,
    req: &MotionRenderRequest,
) -> Result<MotionRenderResultDto, String> {
    req.validate().map_err(|e| e.to_string())?;
    let hash = content_hash(req);

    // Cache hit: reuse existing frames without re-rendering.
    if cache.is_cached(req) {
        return Ok(MotionRenderResultDto {
            media_ref: motion_ref_for_hash(&hash),
            content_hash: hash,
            frame_count: req.duration_frames as usize,
            width: req.width,
            height: req.height,
            transparent: req.transparent,
            renderer: "cache".to_string(),
        });
    }

    // Cache miss: render. Try the CDP backend when the feature is compiled in
    // AND a Chrome binary is available; fall back to the deterministic stub on
    // any failure so the command always succeeds offline.
    //
    // TODO(#14): once the CDP client is implemented under the `chromium` feature,
    // this path will produce real headless-Chromium frames. Until then
    // `HeadlessChromiumRenderer::render` returns `RendererUnavailable` and the
    // stub fallback handles production rendering.
    //
    // `policy` is only consumed by the chromium backend below; silence the
    // unused-variable warning when the feature is off (the stub applies its own
    // default policy internally).
    #[cfg(not(feature = "chromium"))]
    let _ = &policy;
    #[cfg(feature = "chromium")]
    {
        if HeadlessChromiumRenderer::chrome_available() {
            let chromium = HeadlessChromiumRenderer::new(cache.clone(), policy.clone());
            match chromium.render(req) {
                Ok(clip) => {
                    return Ok(to_dto(clip, "chromium"));
                }
                Err(e) => {
                    eprintln!(
                        "render_motion: chromium backend failed ({e}); \
                         falling back to StubRenderer"
                    );
                }
            }
        } else {
            eprintln!(
                "render_motion: chromium feature enabled but no Chrome binary found; \
                 falling back to StubRenderer"
            );
        }
    }

    // Default / fallback: the deterministic stub renderer (always works).
    let stub = StubRenderer::new(cache.clone());
    let clip = stub.render(req).map_err(|e| e.to_string())?;
    Ok(to_dto(clip, "stub"))
}

/// Map a [`opentake_motion::RenderedClip`] + renderer name to the DTO.
fn to_dto(
    clip: opentake_motion::RenderedClip,
    renderer: &str,
) -> MotionRenderResultDto {
    // Compute `frame_count` before `content_hash` is moved out of `clip`
    // (frame_count borrows self; the String move below would make that a
    // partial-move borrow error).
    let frame_count = clip.frame_count();
    MotionRenderResultDto {
        media_ref: motion_ref_for_hash(&clip.content_hash),
        content_hash: clip.content_hash,
        frame_count,
        width: clip.width,
        height: clip.height,
        transparent: clip.transparent,
        renderer: renderer.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_motion::MotionSource;
    use tempfile::tempdir;

    /// Build a MotionState rooted at a temp dir (tests must not touch the real
    /// app cache).
    fn state_in_tmp() -> (MotionState, tempfile::TempDir) {
        let tmp = tempdir().unwrap();
        let state = MotionState::new(tmp.path().to_path_buf());
        (state, tmp)
    }

    /// A minimal valid request: 30 fps, 5 frames, 16×8, transparent.
    fn sample_req() -> MotionRenderRequest {
        MotionRenderRequest::new(MotionSource::code("<div>hi</div>"), 30, 5, 16, 8)
    }

    #[test]
    fn render_motion_stub_produces_frames_and_motion_ref() {
        let (state, _tmp) = state_in_tmp();
        let req = sample_req();
        let result = render_motion(state.cache(), SandboxPolicy::default(), &req).unwrap();

        // The media_ref is motion://<hash>.
        assert!(result.media_ref.starts_with("motion://"));
        assert_eq!(
            result.media_ref.strip_prefix("motion://"),
            Some(result.content_hash.as_str())
        );
        // The hash is the content-hash of the request.
        assert_eq!(result.content_hash, content_hash(&req));
        // Stub renderer produced the expected frame count.
        assert_eq!(result.frame_count, 5);
        assert_eq!(result.width, 16);
        assert_eq!(result.height, 8);
        assert!(result.transparent); // default is transparent
        assert_eq!(result.renderer, "stub");
    }

    #[test]
    fn render_motion_cache_hit_returns_without_rerendering() {
        let (state, _tmp) = state_in_tmp();
        let req = sample_req();

        // First render: miss → stub renders.
        let first = render_motion(state.cache(), SandboxPolicy::default(), &req).unwrap();
        assert_eq!(first.renderer, "stub");

        // Second render: hit → returns "cache" without re-rendering.
        let second = render_motion(state.cache(), SandboxPolicy::default(), &req).unwrap();
        assert_eq!(second.renderer, "cache");
        // Same hash, same metadata.
        assert_eq!(second.content_hash, first.content_hash);
        assert_eq!(second.media_ref, first.media_ref);
        assert_eq!(second.frame_count, first.frame_count);
    }

    #[test]
    fn render_motion_rejects_invalid_request() {
        let (state, _tmp) = state_in_tmp();
        // fps = 0 is invalid.
        let bad = MotionRenderRequest::new(MotionSource::code("x"), 0, 5, 16, 8);
        let err = render_motion(state.cache(), SandboxPolicy::default(), &bad);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("fps"));
    }

    #[test]
    fn render_motion_rejects_empty_code() {
        let (state, _tmp) = state_in_tmp();
        let bad = MotionRenderRequest::new(MotionSource::code("   "), 30, 5, 16, 8);
        let err = render_motion(state.cache(), SandboxPolicy::default(), &bad);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("empty"));
    }

    #[test]
    fn render_motion_different_sources_get_different_hashes() {
        let (state, _tmp) = state_in_tmp();
        let a = MotionRenderRequest::new(MotionSource::code("<a/>"), 30, 5, 16, 8);
        let b = MotionRenderRequest::new(MotionSource::code("<b/>"), 30, 5, 16, 8);
        let ra = render_motion(state.cache(), SandboxPolicy::default(), &a).unwrap();
        let rb = render_motion(state.cache(), SandboxPolicy::default(), &b).unwrap();
        assert_ne!(ra.content_hash, rb.content_hash);
        assert_ne!(ra.media_ref, rb.media_ref);
    }

    #[test]
    fn render_motion_template_source_works() {
        let (state, _tmp) = state_in_tmp();
        let req = MotionRenderRequest::new(MotionSource::template("lower-third.glass"), 30, 3, 64, 32);
        let result = render_motion(state.cache(), SandboxPolicy::default(), &req).unwrap();
        assert_eq!(result.frame_count, 3);
        assert_eq!(result.width, 64);
        assert_eq!(result.height, 32);
        assert!(result.media_ref.starts_with("motion://"));
    }
}
