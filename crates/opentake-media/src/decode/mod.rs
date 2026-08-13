//! Decode facade: frame seek/decode and audio PCM extraction. Both back ends
//! shell out to the system ffmpeg CLI (see `crate::ff`).

pub mod audio_stream;
pub mod frame;
pub mod pcm;
pub mod stream;

pub use audio_stream::{decode_pcm_interleaved, decode_pcm_interleaved_cancellable};
pub use frame::{
    convert_frame_rate, decode_frame_at, decode_frame_at_cancellable,
    decode_frame_file_at_cancellable, decode_frames_at, decode_frames_at_cancellable, fit_within,
    interpolate_frame_pair, FrameInterpolationFallback, FrameInterpolationMode,
    FrameInterpolationResult, FrameRateSample, FrameRequest,
};
pub use pcm::{
    extract_pcm, extract_pcm_cancellable, extract_pcm_cancellable_with_progress, PcmBuffer,
    PcmFormat, PcmProgressCallback, PcmSpec,
};
pub use stream::{
    spawn_video_stream, StreamDecodeControl, StreamVideoFrame, VideoStream, VideoStreamRequest,
    DEFAULT_VIDEO_STREAM_QUEUE_CAPACITY,
};
