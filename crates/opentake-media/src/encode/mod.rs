//! Video encoding back end for `opentake-render`'s export path. The wgpu
//! compositor produces RGBA frames; this encoder pipes them to the system ffmpeg
//! CLI and muxes them (with an optional audio track) into a container.
//!
//! `opentake-render` decides the (even) frame size, applies BT.709 instructions,
//! and resolves keyframe ramps; this crate only encodes already-composited
//! frames (SPEC §2.4 / §8.2). The arg builder ([`encode_args`]) is pure and
//! unit-tested; the encode itself requires ffmpeg.

pub mod mix;
pub mod preset;

pub use mix::{mix_clips, mono_f32_to_s16le, ClipAudio, MIX_SAMPLE_RATE};
pub use preset::{even_dimension, ExportPreset, ExportResolution, VideoCodec};

use std::io::Write;
use std::path::{Path, PathBuf};

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

/// Build the ffmpeg arg list for the second mux pass: take the already-encoded
/// (audio-less) video at `video_in` and a raw mono `s16le` PCM stream at
/// `pcm_in`, copy the video stream untouched, encode the audio with `acodec`,
/// and write the muxed container to `out`. Pure so the CLI contract is testable.
///
/// `-shortest` trims the muxed output to the shorter of the two streams, so a
/// trailing audio tail past the last video frame doesn't extend the video.
fn mux_args(
    video_in: &Path,
    pcm_in: &Path,
    out: &Path,
    sample_rate: u32,
    acodec: &str,
) -> Vec<String> {
    vec![
        "-y".into(),
        // Input 0: the encoded video (audio-less).
        "-i".into(),
        video_in.to_string_lossy().into_owned(),
        // Input 1: raw mono s16le PCM (the mixed audio).
        "-f".into(),
        "s16le".into(),
        "-ar".into(),
        sample_rate.to_string(),
        "-ac".into(),
        "1".into(),
        "-i".into(),
        pcm_in.to_string_lossy().into_owned(),
        // Copy the video stream verbatim; (re-)encode the audio.
        "-c:v".into(),
        "copy".into(),
        "-c:a".into(),
        acodec.into(),
        "-shortest".into(),
        out.to_string_lossy().into_owned(),
    ]
}

/// A streaming RGBA → video encoder. Push frames in order, then `finish`.
///
/// When [`push_audio`] has supplied a mixed PCM buffer, `finish` runs a second
/// ffmpeg pass that mux's the audio into the encoded container (`-c:v copy` +
/// `-c:a aac`/`pcm_s16le`). Without audio the video-only first pass *is* the
/// final file. The mux-args builder ([`mux_args`]) is pure and unit-tested; the
/// mux itself requires ffmpeg.
pub struct VideoEncoder {
    child: ffmpeg_sidecar::child::FfmpegChild,
    stdin: Option<std::process::ChildStdin>,
    expected_frame_bytes: usize,
    /// Final output path (the video first pass writes here; the mux pass, when
    /// audio is present, rewrites it from a temp video + the PCM).
    out_path: PathBuf,
    /// ffmpeg `-c:a` token for the mux pass (from the preset).
    acodec: &'static str,
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
            out_path: out.to_path_buf(),
            acodec: preset.acodec_arg(),
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

    /// Record the mixed-down mono audio buffer to mux on `finish`. The buffer's
    /// `spec.sample_rate` is the rate ffmpeg is told to read the muxed PCM at
    /// (the orchestrator decodes/mixes at [`MIX_SAMPLE_RATE`]). An empty buffer
    /// is ignored — `finish` then keeps the video-only output.
    pub fn push_audio(&mut self, pcm: PcmBuffer) {
        if pcm.samples_f32.is_empty() {
            self.pending_audio = None;
        } else {
            self.pending_audio = Some(pcm);
        }
    }

    /// Finish encoding: close stdin, wait for the video pass, then — when a
    /// mixed audio buffer was supplied — run a second ffmpeg pass to mux it in.
    ///
    /// The video first pass writes `out_path` directly. To mux, the encoded
    /// video is moved aside to a sibling temp file, the mixed PCM is written to
    /// another temp file, and ffmpeg copies the video stream while encoding the
    /// audio back into `out_path`. Both temp files are removed afterward (best
    /// effort). Without audio this is exactly the old video-only `finish`.
    pub fn finish(mut self) -> Result<()> {
        // Drop stdin to signal EOF to ffmpeg, then wait for the video pass.
        self.stdin.take();
        let status = self.child.wait().map_err(MediaError::Io)?;
        if !status.success() {
            return Err(MediaError::Encode(format!("ffmpeg exited {status}")));
        }

        let Some(pcm) = self.pending_audio.take() else {
            return Ok(()); // video-only: the first pass is the final file.
        };

        self.mux_audio(&pcm)
    }

    /// Second ffmpeg pass: mux `pcm` (mono f32, written as s16le) into the
    /// already-encoded video at `self.out_path`, in place.
    fn mux_audio(&self, pcm: &PcmBuffer) -> Result<()> {
        let out = &self.out_path;
        // Sibling temp paths next to the output (same dir → cheap rename, same
        // filesystem). Suffixes keep them distinct from the final artifact.
        let video_tmp = sibling_temp(out, "video");
        let pcm_tmp = sibling_temp(out, "pcm");

        // Move the encoded video aside so ffmpeg can rewrite `out` from it.
        std::fs::rename(out, &video_tmp).map_err(MediaError::Io)?;

        // Run the mux, cleaning up temps regardless of outcome.
        let result = (|| {
            let bytes = mix::mono_f32_to_s16le(&pcm.samples_f32);
            std::fs::write(&pcm_tmp, &bytes).map_err(MediaError::Io)?;

            let args = mux_args(&video_tmp, &pcm_tmp, out, pcm.spec.sample_rate, self.acodec);
            let mut child = crate::ff::ffmpeg()
                .args(args)
                .spawn()
                .map_err(|e| MediaError::Encode(format!("mux spawn: {e}")))?;
            let status = child.wait().map_err(MediaError::Io)?;
            if !status.success() {
                return Err(MediaError::Encode(format!("ffmpeg mux exited {status}")));
            }
            Ok(())
        })();

        // Best-effort cleanup. If the mux failed, restore the video-only file so
        // the caller still has a valid (audio-less) export rather than nothing.
        let _ = std::fs::remove_file(&pcm_tmp);
        if result.is_err() {
            let _ = std::fs::rename(&video_tmp, out);
        } else {
            let _ = std::fs::remove_file(&video_tmp);
        }
        result
    }
}

/// Build a sibling temp path next to `out`: `<out>.<tag>.tmp`. Stays on the same
/// filesystem so the rename in `mux_audio` is atomic and cheap.
fn sibling_temp(out: &Path, tag: &str) -> PathBuf {
    let mut name = out
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(format!(".{tag}.tmp"));
    out.with_file_name(name)
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

    #[test]
    fn mux_args_copy_video_and_encode_audio() {
        let args = mux_args(
            Path::new("/v.mp4"),
            Path::new("/a.pcm"),
            Path::new("/out.mp4"),
            48_000,
            "aac",
        );
        // video input first, then the raw s16le PCM input declared with rate/ch.
        assert!(args.windows(2).any(|w| w == ["-i", "/v.mp4"]));
        assert!(args.windows(2).any(|w| w == ["-f", "s16le"]));
        assert!(args.windows(2).any(|w| w == ["-ar", "48000"]));
        assert!(args.windows(2).any(|w| w == ["-ac", "1"]));
        assert!(args.windows(2).any(|w| w == ["-i", "/a.pcm"]));
        // copy the video stream, encode audio with the preset codec.
        assert!(args.windows(2).any(|w| w == ["-c:v", "copy"]));
        assert!(args.windows(2).any(|w| w == ["-c:a", "aac"]));
        assert!(args.iter().any(|a| a == "-shortest"));
        assert_eq!(args.last().unwrap(), "/out.mp4");
    }

    #[test]
    fn mux_args_threads_prores_lpcm_codec() {
        let args = mux_args(
            Path::new("/v.mov"),
            Path::new("/a.pcm"),
            Path::new("/out.mov"),
            48_000,
            "pcm_s16le",
        );
        assert!(args.windows(2).any(|w| w == ["-c:a", "pcm_s16le"]));
    }

    #[test]
    fn sibling_temp_keeps_directory_and_tags_name() {
        let t = sibling_temp(Path::new("/tmp/clip/out.mp4"), "video");
        assert_eq!(t, PathBuf::from("/tmp/clip/out.mp4.video.tmp"));
    }
}
