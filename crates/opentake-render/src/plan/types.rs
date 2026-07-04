//! RenderPlan data structures (SPEC §2.2).
//!
//! Two layers, mirroring upstream's static `trackMappings` + dynamic
//! `buildVisuals` split (VideoEngine L169/L195):
//!
//! - [`RenderPlan`] — frame-independent: which tracks, which clips (deduped /
//!   sorted), each clip's source type + natSize + preferredTransform + blend
//!   order + canvas size + fps + total frames. Parsed once from a `Timeline`.
//! - [`FramePlan`] (via [`RenderPlan::frame`](crate::plan::RenderPlan::frame)) —
//!   instantaneous: an ordered `Vec<LayerDraw>` for one frame, each carrying the
//!   affine matrix / crop UV / opacity sampled by the domain `*_at` methods.
//!
//! The black background is NOT a clip here — it is the compositor clear color
//! `(0,0,0,1)` (SPEC §3.5).

use opentake_domain::{ChromaKey, ClipType, ColorGrade, Effect, Mask};

/// Canvas pixel size (already even-ized; see [`crate::size`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RenderSize {
    pub width: u32,
    pub height: u32,
}

impl RenderSize {
    pub fn new(width: u32, height: u32) -> Self {
        RenderSize { width, height }
    }

    pub fn width_f(&self) -> f64 {
        self.width as f64
    }

    pub fn height_f(&self) -> f64 {
        self.height as f64
    }
}

/// Where a clip's source texture comes from. Materialization strategy: SPEC §4.
///
/// Upstream burns images / Lottie / text into intermediate videos because
/// `AVPlayer` can't play them directly; our compositor consumes textures
/// natively, so those hacks disappear (ARCHITECTURE §6).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TextureSource {
    /// Video (audio carries no video texture; the render side ignores it):
    /// decode by `media_ref` + source-frame index.
    Decoded { media_ref: String },
    /// Image: one static texture (content-hash cached). Upstream's 30-minute
    /// still-video hack is gone.
    Image { media_ref: String },
    /// Lottie: a texture per "Lottie internal frame" (content-hash cached).
    Lottie { media_ref: String },
    /// Text: rasterized for this clip at the canvas size (content-hash cached,
    /// key = style + content + canvas).
    Text { clip_id: String },
}

/// Static (frame-independent) render description for one clip.
#[derive(Clone, PartialEq, Debug)]
pub struct ClipPlan {
    pub clip_id: String,
    /// Index of the timeline track this clip belongs to (blend order; SPEC §1.5).
    pub track_index: usize,
    /// Index of the clip inside `timeline.tracks[track_index].clips`, so
    /// [`RenderPlan::frame`](crate::plan::RenderPlan::frame) can fetch the `&Clip`
    /// without a string search (SPEC §2.4 note).
    pub clip_index: usize,
    pub source: TextureSource,
    pub start_frame: i32,
    /// Half-open end (`start_frame + duration_frames`).
    pub end_frame: i32,
    /// Source display size = upstream `clipNaturalSizes[clip.id]`
    /// (CompositionBuilder L166-172):
    /// `nat_size = |CGRect(origin: .zero, size: natSize0).applying(preferredTransform)|.size`.
    pub nat_size: (f64, f64),
    /// Source-track orientation fix-up (upstream `preferredTransform`, already
    /// translated so its bounding box origin is at (0,0), L172). Row-major
    /// `[a, b, c, d, tx, ty]`. Identity = `[1, 0, 0, 1, 0, 0]`.
    pub preferred_transform: [f64; 6],
    /// Whether a straight-alpha source needs premultiplying (SPEC §4.1). Image /
    /// text / Lottie are already premultiplied.
    pub needs_premultiply: bool,
    /// Playback speed (source-frame index conversion; SPEC §2.5).
    pub speed: f64,
    pub trim_start_frame: i32,
    pub media_type: ClipType,
    /// For [`TextureSource::Lottie`], the source's internal frame count (modulo
    /// wrap; SPEC §4.3). `None` for non-Lottie or unknown.
    pub lottie_frame_count: Option<i64>,

    // Advanced pixel-effect inputs (A-tier; `docs/ADVANCED-FEATURES.md`). Copied
    // from the source `Clip` at plan-build time (not keyframed in this round, so
    // they are frame-independent). The compositor applies them in the fragment
    // shader; the pure pixel math lives in `opentake_domain::grade`.
    /// Linear-light color grade, or `None` when the clip has no grade.
    pub color_grade: Option<ColorGrade>,
    /// Chroma key, or `None` when the clip has no keying.
    pub chroma_key: Option<ChromaKey>,
    /// Vector masks (intersected coverage). Empty = no masking.
    pub masks: Vec<Mask>,
    /// Generic named-effect chain. Empty = no effects. Carried through the plan
    /// for downstream passes; the current compositor renders the color/chroma/mask
    /// stages and ignores unknown effect names (see render TODO).
    pub effects: Vec<Effect>,
}

/// The static plan for a whole timeline.
#[derive(Clone, PartialEq, Debug)]
pub struct RenderPlan {
    pub fps: i32,
    pub render_size: RenderSize,
    pub total_frames: i32,
    /// Blend order: larger index = on top (SPEC §1.5). Sorted by
    /// `(track_index, start_frame)`; within a track no clips overlap. The black
    /// background is the clear color, not an entry here.
    pub clip_plans: Vec<ClipPlan>,
    /// Text clips, kept separate so they always composite ABOVE all video
    /// (upstream bakes text over the video composition via the CoreAnimationTool,
    /// ExportService L237-248; SPEC §4.2). Ordered by appearance, NOT deduped
    /// per track (upstream collects text clips without the per-track skip rule).
    pub text_plans: Vec<ClipPlan>,
}

/// One draw after evaluating a single frame (instantaneous).
#[derive(Clone, PartialEq, Debug)]
pub struct LayerDraw<'a> {
    pub source: &'a TextureSource,
    /// Source-frame index referenced at this frame (Decoded / Lottie; Image /
    /// Text are always 0). SPEC §2.5.
    pub source_frame: i64,
    /// Final affine: normalized canvas (0–1) -> pixels, =
    /// `preferred ∘ affine_transform(transform_at(f))` (SPEC §1.3). Row-major
    /// `[a, b, c, d, tx, ty]`; coordinate frame: origin bottom-left, y up, unit =
    /// pixels (SPEC §1.3 projection convention).
    pub affine: [f64; 6],
    /// Source natural size (pixels) the `affine` was built against — the shader
    /// scales its `[0,1]` quad by THIS to recover source-pixel space before
    /// applying `affine`. It MUST be the value passed to `affine_transform`
    /// (`ClipPlan.nat_size`), NOT the decoded texture's resolution: the preview
    /// decodes at a downscaled `max_size`, so a texture-size proxy mismatches the
    /// affine and renders the layer shrunk into a corner (and jittering as the
    /// texture size varies). SPEC §1.3 / §3.3.
    pub nat_size: (f64, f64),
    /// Source-texture UV sub-rect (folded from `crop_at(f)`), `(u0, v0, u1, v1)`
    /// in `[0, 1]`. SPEC §3.4.
    pub crop_uv: (f64, f64, f64, f64),
    /// Premultiplied-alpha global multiplier = `clip.opacity_at(f)` in `[0, 1]`.
    pub opacity: f64,
    pub needs_premultiply: bool,
    pub clip_id: &'a str,
    /// Color grade applied in-shader (linear-light chain), borrowed from the
    /// [`ClipPlan`]. `None` = no grade.
    pub color_grade: Option<&'a ColorGrade>,
    /// Chroma key applied in-shader, borrowed from the [`ClipPlan`]. `None` = none.
    pub chroma_key: Option<&'a ChromaKey>,
    /// Masks applied in-shader (intersected coverage), borrowed from the
    /// [`ClipPlan`]. Empty = no masking.
    pub masks: &'a [Mask],
    /// Effect chain, borrowed from the [`ClipPlan`]. Carried for downstream
    /// passes. Empty = none.
    pub effects: &'a [Effect],
}

/// A single frame's ordered draw list + clear color.
#[derive(Clone, PartialEq, Debug)]
pub struct FramePlan<'a> {
    /// Always `[0, 0, 0, 1]` opaque black (SPEC §3.5).
    pub clear_rgba: [f64; 4],
    /// Already in blend order (video first, then text on top); composite by
    /// straight-forward sequential alpha-over.
    pub draws: Vec<LayerDraw<'a>>,
}
