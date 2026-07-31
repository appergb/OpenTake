//! Lightweight offline media analysis primitives.

pub mod autocrop;
pub mod beat;
pub mod loudness;
pub mod silence;
pub mod stabilization;

pub use autocrop::{
    detect_autocrop, AutocropConfig, AutocropPlan, CropRect, CropTransform, FrameBuffer,
    PixelFormat,
};
pub use beat::{detect_beats, BeatDetectionConfig, BeatOnset};
pub use loudness::{
    analyze_loudness, analyze_loudness_with_progress, apply_loudness_gain, LoudnessAnalysis,
    LoudnessError, LoudnessNormalizationConfig, LoudnessProgressCallback,
};
pub use silence::{detect_silences, SilenceDetectionConfig, SilenceRange};
pub use stabilization::{
    analyze_stabilization, track_translation_motion, StabilizationConfig, StabilizationMotionSample,
};
