//! Lightweight offline media analysis primitives.

pub mod autocrop;
pub mod beat;
pub mod silence;

pub use autocrop::{
    detect_autocrop, AutocropConfig, AutocropPlan, CropRect, CropTransform, FrameBuffer,
    PixelFormat,
};
pub use beat::{detect_beats, BeatDetectionConfig, BeatOnset};
pub use silence::{detect_silences, SilenceDetectionConfig, SilenceRange};
