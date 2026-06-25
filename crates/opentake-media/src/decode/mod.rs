//! Decode facade: frame seek/decode and audio PCM extraction. Both back ends
//! shell out to the system ffmpeg CLI (see `crate::ff`).

pub mod frame;
pub mod pcm;

pub use frame::{decode_frame_at, decode_frames_at, fit_within, FrameRequest};
pub use pcm::{extract_pcm, PcmBuffer, PcmFormat, PcmSpec};
