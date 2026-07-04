//! Decode facade: frame seek/decode and audio PCM extraction. Both back ends
//! shell out to the system ffmpeg CLI (see `crate::ff`).

pub mod audio_stream;
pub mod frame;
pub mod pcm;
pub mod stream;

pub use audio_stream::decode_pcm_interleaved;
pub use frame::{decode_frame_at, decode_frames_at, fit_within, FrameRequest};
pub use pcm::{extract_pcm, PcmBuffer, PcmFormat, PcmSpec};
pub use stream::{
    spawn_video_stream, StreamDecodeControl, StreamVideoFrame, VideoStream, VideoStreamRequest,
    DEFAULT_VIDEO_STREAM_QUEUE_CAPACITY,
};
