//! opentake-domain — value-type domain model.
//!
//! A faithful 1:1 port of PalmierPro's `Models/` layer to Rust: Timeline / Track
//! / Clip / Keyframe / Transform / Crop / TextStyle / media manifest types, plus
//! all of their pure derived functions (`end_frame`, `source_frames_consumed`,
//! `*_at` sampling, `fade_multiplier`, keyframe `sample`, dB <-> linear). Also
//! defines the Phase A agent context-signal types (see
//! `docs/AGENT-CONTEXT-SIGNAL.md`).
//!
//! Design rules carried over from upstream and the port map:
//! - Frames are `i32`; the timeline span is half-open `[start, end)`.
//! - `round()` is half-away-from-zero (Rust `f64::round` == Swift `.rounded()`).
//! - JSON keys match Swift's default `JSONEncoder` output (property names
//!   verbatim, camelCase with abbreviation casing preserved) so projects round-
//!   trip with the upstream app.
//! - Decoding is missing-key tolerant (`#[serde(default)]` + `Option`), including
//!   the legacy `Transform` `x`/`y` -> center migration and the `MediaManifest`
//!   `version` fallback to 1.
//!
//! Zero IO, pure logic, fully unit-testable. The only runtime dependency is
//! `serde`; persistence-side UUID repair belongs to `opentake-project`.

pub mod audio;
pub mod caption_sync;
pub mod clip;
pub mod clip_type;
mod clip_wire;
pub mod grade;
pub mod keyframe;
pub mod lut;
pub mod media;
pub mod signal;
pub mod split;
pub mod stabilization;
pub mod subtitle_export;
pub mod text;
mod text_wire;
pub mod timeline;
pub mod transform;
pub mod transition;

// Flat re-export of the public domain API for ergonomic downstream use.
pub use audio::{AudioDenoise, DenoiseMode, LoudnessNormalization};
pub use caption_sync::{caption_group_ids, clips_in_group, sync_caption_group_style};
pub use clip::{Clip, FadeEdge, KeyframeTrackWireField, KeyframeValueWireShape, VolumeScale};
pub use clip_type::ClipType;
pub use grade::{
    chroma_cb_cr, effect_registry, luma709, smoothstep01, validate_effect_chain, ChromaKey,
    ColorGrade, ColorGradeValidationError, ColorMatchInput, Effect, EffectDescriptor,
    EffectParameterDescriptor, EffectValidationError, HslSecondary, LiftGammaGain, Mask, MaskShape,
    MaskTransform, Point2, Rgb, MAX_EFFECTS_PER_CLIP, MAX_MASKS_PER_CLIP, MAX_POLYGON_MASK_POINTS,
};
pub use keyframe::{
    smoothstep, split_keyframe_track, AnimPair, AnimatableProperty, Interpolation, Keyframe,
    KeyframeInterpolatable, KeyframeTrack,
};
pub use lut::{CubeLut, CubeLutError, LutReference, LutReferenceValidationError};
pub use media::{
    GenerationInput, GenerationJobStatus, GenerationStatus, MediaAsset, MediaColorMetadata,
    MediaFolder, MediaManifest, MediaManifestEntry, MediaProxy, MediaResolver, MediaSource,
};
pub use signal::{
    ContextSignal, EditingSkeleton, EditingStage, StageGuidance, TrackHint, TrackRole,
    TrackRoleAssignment, VideoType,
};
pub use split::split_clip;
pub use stabilization::{StabilizationKeyframe, StabilizationTrack, StabilizationTransform};
pub use subtitle_export::{collect_caption_cues, export_srt, export_vtt, SubtitleCue};
pub use text::{Fill, Rgba, Shadow, TextAlignment, TextLayout, TextStyle};
pub use timeline::{ClipLocation, NestedSequence, Timeline, Track};
pub use transform::{Crop, CropAspectLock, Point, Transform};
pub use transition::{Transition, TransitionKind};
