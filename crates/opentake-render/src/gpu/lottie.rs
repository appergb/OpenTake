//! Lottie rasterization interface (Issue #65). Upstream bakes Lottie
//! (Bodymovin JSON) animations into an intermediate video via the
//! CoreAnimationTool; OpenTake rasterizes each Lottie clip to a sequence of
//! premultiplied-RGBA textures that composite like any other layer.
//!
//! This module defines the trait boundary + a null implementation that returns
//! `None` (never `todo!()` / `unimplemented!()`), so the compositor can route
//! Lottie clips and tests never trip an unimplemented panic. A real backend
//! (e.g. a `rlottie`/`vello` wrapper, or the `opentake-motion` crate's
//! `MotionClipSource` exposed as a `FrameProvider`) implements this trait and is
//! injected into the production `TextureResolver`.
//!
//! ## Integration points
//!
//! - The plan builder routes `ClipType::Lottie` → [`TextureSource::Lottie`]
//!   (see `plan/build.rs::texture_source_for`).
//! - `source_frame_index` maps the timeline frame to a Lottie internal frame
//!   (modulo `lottie_frame_count`, see `plan/build.rs`).
//! - The production resolver (`src-tauri/render.rs::MediaResolver`) calls the
//!   injected `LottieRasterizer` for `TextureSource::Lottie`, uploading the
//!   returned [`DecodedFrame`] as a texture (same path as image/text).
//!
//! ## Why a separate trait (not `FrameProvider::lottie_frame`)?
//!
//! `FrameProvider` is the contract for already-decoded sources (video/image
//! frames living in a codec). Lottie baking is a *rasterization* step (JSON →
//! pixels), mirroring `TextRasterizer` (style → pixels): both turn non-pixel
//! clip data into a texture on demand. Keeping the trait separate also lets the
//! resolver cache by `(media_ref, frame)` without entangling codec frame
//! providers, exactly as it caches text by `clip_id`.

use crate::source::DecodedFrame;

/// Inputs needed to rasterize one Lottie frame.
///
/// `canvas` is the compositor's preview render size; a rasterizer may downscale
/// the Lottie's intrinsic composition to fit it (matching how text/image layers
/// respect the preview cap). The returned [`DecodedFrame`] carries its own
/// width/height so the uploader is size-agnostic.
#[derive(Clone, PartialEq, Debug)]
pub struct LottieRasterRequest<'a> {
    /// The Lottie asset ref (resolves to a `.json`/`.lottie` file in the
    /// caller's media manifest).
    pub media_ref: &'a str,
    /// Lottie internal frame index (already wrapped to `[0, frame_count)` by
    /// `source_frame_index` when `lottie_frame_count` is known).
    pub frame: i64,
    /// Compositor canvas size — the rasterizer may cap the output to this size
    /// to bound CPU/RAM (same rationale as the preview cap for video).
    pub canvas: (u32, u32),
}

/// Rasterizes one Lottie frame to a premultiplied-RGBA [`DecodedFrame`].
///
/// Implementations MUST be deterministic for cacheability: the same
/// `(media_ref, frame, canvas)` yields the same pixels (mirrors the motion
/// crate's content-hash contract).
pub trait LottieRasterizer {
    /// Render the request, or `None` if Lottie baking is unavailable in this
    /// build (the null backend), the asset is missing/corrupt, or the frame
    /// index is out of range. Returning `None` makes the compositor skip the
    /// layer (same graceful degradation as a failed video decode or an
    /// un-rasterizable text clip).
    fn rasterize(&self, request: &LottieRasterRequest<'_>) -> Option<DecodedFrame>;

    /// The Lottie composition's internal frame count, if known. Used by the
    /// plan builder to wrap the source-frame index modulo this count (SPEC
    /// §4.3). `None` when unknown — the plan then clamps at 0 instead of
    /// wrapping. Default `None` so a stub backend compiles without
    /// introspecting the JSON.
    fn frame_count(&self, _media_ref: &str) -> Option<i64> {
        None
    }
}

/// Placeholder backend: produces no texture and reports no frame count. Lets
/// the pipeline compile, route Lottie clips, and run end-to-end without a
/// Lottie engine. Replaced by a real backend (rlottie / vello / motion crate)
/// in a later phase — inject it into `MediaResolver` when available.
///
/// This is the Lottie analogue of [`crate::gpu::NullTextRasterizer`].
#[derive(Clone, Copy, Debug, Default)]
pub struct NullLottieRasterizer;

impl LottieRasterizer for NullLottieRasterizer {
    fn rasterize(&self, _request: &LottieRasterRequest<'_>) -> Option<DecodedFrame> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_rasterizer_returns_none_without_panicking() {
        let r = NullLottieRasterizer;
        let req = LottieRasterRequest {
            media_ref: "anim.json",
            frame: 0,
            canvas: (1920, 1080),
        };
        assert!(r.rasterize(&req).is_none());
    }

    #[test]
    fn null_rasterizer_reports_no_frame_count() {
        let r = NullLottieRasterizer;
        assert_eq!(r.frame_count("anim.json"), None);
    }

    #[test]
    fn request_carries_media_ref_and_frame() {
        let req = LottieRasterRequest {
            media_ref: "intro.json",
            frame: 12,
            canvas: (1280, 720),
        };
        assert_eq!(req.media_ref, "intro.json");
        assert_eq!(req.frame, 12);
        assert_eq!(req.canvas, (1280, 720));
    }
}
