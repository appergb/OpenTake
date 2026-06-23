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
//! Zero IO, pure logic, fully unit-testable. The only dependency is `serde`.

pub mod clip;
pub mod clip_type;
pub mod grade;
pub mod keyframe;
pub mod media;
pub mod signal;
pub mod split;
pub mod subtitle_export;
pub mod text;
pub mod timeline;
pub mod transform;

// Flat re-export of the public domain API for ergonomic downstream use.
pub use clip::{Clip, FadeEdge, VolumeScale};
pub use clip_type::ClipType;
pub use grade::{
    chroma_cb_cr, luma709, smoothstep01, ChromaKey, ColorGrade, Effect, LiftGammaGain, Mask,
    MaskShape, Point2, Rgb,
};
pub use keyframe::{
    smoothstep, split_keyframe_track, AnimPair, AnimatableProperty, Interpolation, Keyframe,
    KeyframeInterpolatable, KeyframeTrack,
};
pub use media::{
    GenerationInput, GenerationStatus, MediaAsset, MediaFolder, MediaManifest, MediaManifestEntry,
    MediaResolver, MediaSource,
};
pub use signal::{
    ContextSignal, EditingSkeleton, EditingStage, StageGuidance, TrackHint, TrackRole,
    TrackRoleAssignment, VideoType,
};
pub use split::split_clip;
pub use subtitle_export::{collect_caption_cues, export_srt, export_vtt, SubtitleCue};
pub use text::{Fill, Rgba, Shadow, TextAlignment, TextLayout, TextStyle};
pub use timeline::{ClipLocation, Timeline, Track};
pub use transform::{Crop, CropAspectLock, Point, Transform};
