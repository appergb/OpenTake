//! Video encoding back end for `opentake-render`'s export path. The wgpu
//! compositor produces RGBA frames; this encoder pipes them to the system ffmpeg
//! CLI and muxes them (with an optional audio track) into a container.
//!
//! `opentake-render` decides the (even) frame size, applies BT.709 instructions,
//! and resolves keyframe ramps; this crate only encodes already-composited
//! frames (SPEC §2.4 / §8.2). The arg builder ([`encode_args`]) is pure and
//! unit-tested; the encode itself requires ffmpeg.

pub mod preset;

pub use preset::{even_dimension, ExportPreset, ExportResolution, VideoCodec};

use std::io::Write;
use std::path::Path;

use crate::decode::pcm::PcmBuffer;
use crate::error::{MediaError, Result};
use crate::frame::RgbaFrame;

/// Build the ffmpeg arg list for encoding a raw-RGBA frame stream (read from
/// stdin) to `out` with `preset`. Pure so the CLI contract is testable.
///
/// Layout: `-f rawvideo -pix_fmt rgba -s {w}x{h} -r {fps} -i -` for video,
/// followed by codec/pixfmt/color args, then `out`.
fn encode_args(out: &Path, w: u32, h: u32, fps: i32, preset: &ExportPreset) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    args.push("-y".into()); // overwrite
                            // Raw video input from stdin.
    args.push("-f".into());
    args.push("rawvideo".into());
    args.push("-pix_fmt".into());
    args.push("rgba".into());
    args.push("-s".into());
    args.push(format!("{w}x{h}"));
    args.push("-r".into());
    args.push(fps.to_string());
    args.push("-i".into());
    args.push("-".into());

    // Video codec + pixel format.
    args.push("-c:v".into());
    args.push(preset.vcodec_arg().into());
    args.push("-pix_fmt".into());
    args.push(preset.pix_fmt_arg().into());
    args.extend(preset.color_args());

    args.push(out.to_string_lossy().into_owned());
    args
}

/// A streaming RGBA → video encoder. Push frames in order, then `finish`.
///
/// Audio muxing for a pre-rendered mix is intentionally limited here: the export
/// pipeline composites/mixes audio in `opentake-render`; a follow-up wires the
/// mixed PCM as a second ffmpeg input. For now [`push_audio`] records the PCM so
/// the render layer can supply it, and the video-only path is fully functional.
pub struct VideoEncoder {
    child: ffmpeg_sidecar::child::FfmpegChild,
    stdin: Option<std::process::ChildStdin>,
    expected_frame_bytes: usize,
    pending_audio: Option<PcmBuffer>,
}

impl VideoEncoder {
    /// Start an encoder writing to `out`. `w`/`h` must already be even.
    pub fn new(out: &Path, w: u32, h: u32, fps: i32, preset: &ExportPreset) -> Result<Self> {
        let mut child = crate::ff::ffmpeg()
            .args(encode_args(out, w, h, fps, preset))
            .spawn()
            .map_err(|e| MediaError::Encode(format!("spawn: {e}")))?;
        let stdin = child.take_stdin();
        Ok(VideoEncoder {
            child,
            stdin,
            expected_frame_bytes: w as usize * h as usize * 4,
            pending_audio: None,
        })
    }

    /// Push one composited frame. The frame's byte length must match the
    /// encoder's configured dimensions.
    pub fn push_frame(&mut self, rgba: &RgbaFrame) -> Result<()> {
        if rgba.rgba.len() != self.expected_frame_bytes {
            return Err(MediaError::Encode(format!(
                "frame size mismatch: got {} bytes, expected {}",
                rgba.rgba.len(),
                self.expected_frame_bytes
            )));
        }
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| MediaError::Encode("encoder stdin closed".into()))?;
        stdin
            .write_all(&rgba.rgba)
            .map_err(|e| MediaError::Encode(format!("write frame: {e}")))?;
        Ok(())
    }

    /// Record the mixed audio PCM to mux. (Muxing is completed by the render
    /// export pipeline; see the type docs.)
    pub fn push_audio(&mut self, pcm: PcmBuffer) {
        self.pending_audio = Some(pcm);
    }

    /// Finish encoding: close stdin and wait for ffmpeg to flush the container.
    pub fn finish(mut self) -> Result<()> {
        // Drop stdin to signal EOF to ffmpeg.
        self.stdin.take();
        let status = self.child.wait().map_err(MediaError::Io)?;
        if !status.success() {
            return Err(MediaError::Encode(format!("ffmpeg exited {status}")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_args_declare_rawvideo_stdin_input() {
        let preset = ExportPreset::new(VideoCodec::H264, ExportResolution::P1080);
        let args = encode_args(Path::new("/out.mp4"), 1920, 1080, 30, &preset);
        // input is rawvideo rgba from stdin at the right size/fps.
        assert!(args.windows(2).any(|w| w == ["-f", "rawvideo"]));
        assert!(args.windows(2).any(|w| w == ["-pix_fmt", "rgba"]));
        assert!(args.windows(2).any(|w| w == ["-s", "1920x1080"]));
        assert!(args.windows(2).any(|w| w == ["-r", "30"]));
        assert!(args.windows(2).any(|w| w == ["-i", "-"]));
        assert_eq!(args.last().unwrap(), "/out.mp4");
    }

    #[test]
    fn encode_args_use_preset_codec_and_color() {
        let preset = ExportPreset::new(VideoCodec::H265, ExportResolution::P720);
        let args = encode_args(Path::new("/o.mp4"), 1280, 720, 24, &preset);
        assert!(args.windows(2).any(|w| w == ["-c:v", "libx265"]));
        assert!(args.windows(2).any(|w| w == ["-pix_fmt", "yuv420p"]));
        assert!(args.windows(2).any(|w| w == ["-colorspace", "bt709"]));
    }

    #[test]
    fn encode_args_prores_pixfmt_and_no_color_tag() {
        let preset = ExportPreset::new(VideoCodec::ProRes422, ExportResolution::P2160);
        let args = encode_args(Path::new("/o.mov"), 3840, 2160, 30, &preset);
        assert!(args.windows(2).any(|w| w == ["-c:v", "prores_ks"]));
        assert!(args.windows(2).any(|w| w == ["-pix_fmt", "yuv422p10le"]));
        // ProRes path does not add BT.709 color tags here.
        assert!(!args.windows(2).any(|w| w == ["-colorspace", "bt709"]));
    }
}
