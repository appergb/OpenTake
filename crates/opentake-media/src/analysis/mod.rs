//! Lightweight offline media analysis primitives.

pub mod autocrop;
pub mod beat;
pub mod denoise;
pub mod loudness;
pub mod matting;
pub mod silence;
pub mod stabilization;
pub mod stems;

pub use autocrop::{
    detect_autocrop, AutocropConfig, AutocropPlan, CropRect, CropTransform, FrameBuffer,
    PixelFormat,
};
pub use beat::{detect_beats, BeatDetectionConfig, BeatOnset};
pub use denoise::{denoise_interleaved, DenoiseError, DenoiseProgressCallback};
pub use loudness::{
    analyze_loudness, analyze_loudness_with_progress, apply_loudness_gain, LoudnessAnalysis,
    LoudnessError, LoudnessNormalizationConfig, LoudnessProgressCallback,
};
#[cfg(feature = "ort-backend")]
pub use matting::RvmMattingSession;
#[cfg(feature = "model-download")]
pub use matting::{download_rvm_model, MattingDownloadProgress};
pub use matting::{
    matting_model_path, verify_rvm_model, AlphaMatteFrame, InstalledMattingModel, RVM_MODEL_BYTES,
    RVM_MODEL_FILE, RVM_MODEL_ID, RVM_MODEL_SHA256, RVM_MODEL_URL,
};
pub use silence::{detect_silences, SilenceDetectionConfig, SilenceRange};
pub use stabilization::{
    analyze_stabilization, track_region_motion, track_translation_motion, NormalizedMotionRegion,
    RegionMotionTrack, StabilizationConfig, StabilizationMotionSample,
};
pub use stems::{
    ensure_local_stem_model, separate_stems, verify_local_stem_model, InstalledStemModel,
    StemExecution, StemMetrics, StemOutput, StemProgressCallback, StemProvenance,
    StemSeparationRequest, StemSeparationResult,
};
