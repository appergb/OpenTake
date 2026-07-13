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

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStderr, ChildStdout, ExitStatus};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::cancel::MediaCancelToken;
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

const ENCODE_POLL_INTERVAL: Duration = Duration::from_millis(5);
const OUTPUT_COPY_CHUNK: usize = 64 * 1024;
const ENCODE_PROGRESS_TOTAL: usize = 1_000;
const FIRST_PASS_END: usize = 100;
const PCM_WRITE_END: usize = 700;
const MUX_WAIT_START: usize = 800;
const MUX_COPY_START: usize = 900;

pub type EncodeProgressCallback = dyn Fn(usize, usize);

/// A streaming RGBA → video encoder. FFmpeg writes only inside a private
/// owner-only workspace; completed bytes are copied into the caller's retained
/// output file, so FFmpeg never reopens the final pathname.
pub struct VideoEncoder {
    child: ffmpeg_sidecar::child::FfmpegChild,
    stdin: Option<std::process::ChildStdin>,
    output_pump: Option<JoinHandle<Result<()>>>,
    stderr_pump: Option<JoinHandle<Result<()>>>,
    expected_frame_bytes: usize,
    workspace: tempfile::TempDir,
    first_pass: PathBuf,
    output: File,
    acodec: &'static str,
    pending_audio: Option<PcmBuffer>,
    child_reaped: bool,
}

impl VideoEncoder {
    /// Start an encoder writing to `out`. `w`/`h` must already be even.
    pub fn new(out: &Path, w: u32, h: u32, fps: i32, preset: &ExportPreset) -> Result<Self> {
        reject_link_output(out)?;
        let output = open_output_nofollow(out)?;
        Self::new_with_file(out, output, w, h, fps, preset)
    }

    pub fn new_with_file(
        out_hint: &Path,
        mut output: File,
        w: u32,
        h: u32,
        fps: i32,
        preset: &ExportPreset,
    ) -> Result<Self> {
        output.set_len(0).map_err(MediaError::Io)?;
        output.seek(SeekFrom::Start(0)).map_err(MediaError::Io)?;
        let workspace = tempfile::Builder::new()
            .prefix("opentake-encode-")
            .tempdir()
            .map_err(MediaError::Io)?;
        #[cfg(unix)]
        std::fs::set_permissions(
            workspace.path(),
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )
        .map_err(MediaError::Io)?;
        let extension = if preset.codec == VideoCodec::ProRes422 {
            "mov"
        } else {
            "mp4"
        };
        let first_pass = workspace.path().join(format!("video.{extension}"));
        let mut child = crate::ff::ffmpeg()
            .args(encode_args(&first_pass, w, h, fps, preset))
            .spawn()
            .map_err(|e| MediaError::Encode(format!("spawn: {e}")))?;
        let stdin = child.take_stdin();
        let stdout = child.take_stdout().ok_or_else(|| {
            terminate_child(&mut child);
            MediaError::Encode("encoder stdout pipe missing".to_string())
        })?;
        let stderr = child.take_stderr().ok_or_else(|| {
            terminate_child(&mut child);
            MediaError::Encode("encoder stderr pipe missing".to_string())
        })?;
        let output_pump = match thread::Builder::new()
            .name("opentake-encoder-stdout".to_string())
            .spawn(move || drain_stdout(stdout))
        {
            Ok(pump) => pump,
            Err(error) => {
                terminate_child(&mut child);
                return Err(MediaError::Encode(format!(
                    "spawn encoder output pump for {}: {error}",
                    out_hint.display()
                )));
            }
        };
        let stderr_pump = match thread::Builder::new()
            .name("opentake-encoder-stderr".to_string())
            .spawn(move || drain_stderr(stderr))
        {
            Ok(pump) => pump,
            Err(error) => {
                terminate_child(&mut child);
                let _ = join_named_pump(output_pump, "encoder output");
                return Err(MediaError::Encode(format!(
                    "spawn encoder stderr pump: {error}"
                )));
            }
        };
        Ok(VideoEncoder {
            child,
            stdin,
            output_pump: Some(output_pump),
            stderr_pump: Some(stderr_pump),
            expected_frame_bytes: w as usize * h as usize * 4,
            workspace,
            first_pass,
            output,
            acodec: preset.acodec_arg(),
            pending_audio: None,
            child_reaped: false,
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

    /// Abort a mid-stream encode (e.g. a user cancel): kill the ffmpeg child and
    /// wait for it to exit, so the caller can safely remove the (now-closed)
    /// partial output file. `std::process::Child`'s own `Drop` does **not** kill
    /// or wait — a plain `drop(encoder)` would orphan the ffmpeg process, which
    /// could still be writing `out_path` at the moment the caller deletes it.
    /// Best-effort: the child may have already exited on its own.
    pub fn abort(mut self) {
        self.reap_child();
    }

    pub fn finish(mut self) -> Result<()> {
        self.finish_cancellable_inner(&MediaCancelToken::new(), None, None)
    }

    pub fn finish_cancellable(
        mut self,
        cancel: &MediaCancelToken,
        progress: Option<&EncodeProgressCallback>,
    ) -> Result<()> {
        self.finish_cancellable_inner(cancel, progress, None)
    }

    fn finish_cancellable_inner(
        &mut self,
        cancel: &MediaCancelToken,
        progress: Option<&EncodeProgressCallback>,
        mux_wait_hook: Option<&dyn Fn()>,
    ) -> Result<()> {
        let status = self.wait_for_child(cancel, progress)?;
        if !status.success() {
            return Err(MediaError::Encode(format!("ffmpeg exited {status}")));
        }
        report_progress(progress, FIRST_PASS_END);

        match self.pending_audio.take() {
            Some(pcm) => self.mux_audio(&pcm, cancel, progress, mux_wait_hook)?,
            None => self.copy_video_only(cancel, progress)?,
        };
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        self.output.flush().map_err(MediaError::Io)?;
        self.output.sync_all().map_err(MediaError::Io)?;
        report_progress(progress, ENCODE_PROGRESS_TOTAL);
        Ok(())
    }

    fn wait_for_child(
        &mut self,
        cancel: &MediaCancelToken,
        progress: Option<&EncodeProgressCallback>,
    ) -> Result<ExitStatus> {
        self.stdin.take();
        let mut polls = 0_usize;
        loop {
            if cancel.checkpoint() {
                terminate_child(&mut self.child);
                self.child_reaped = true;
                let _ = self.join_output_pump();
                return Err(MediaError::Cancelled);
            }
            match self.child.as_inner_mut().try_wait() {
                Ok(Some(status)) => {
                    self.child_reaped = true;
                    self.join_output_pump()?;
                    return Ok(status);
                }
                Ok(None) => {
                    polls = polls.saturating_add(1);
                    if polls.is_multiple_of(20) {
                        report_progress(progress, (polls / 20).min(FIRST_PASS_END - 1));
                    }
                    thread::sleep(ENCODE_POLL_INTERVAL);
                }
                Err(error) => {
                    self.reap_child();
                    return Err(MediaError::Io(error));
                }
            }
        }
    }

    fn join_output_pump(&mut self) -> Result<()> {
        let output = self
            .output_pump
            .take()
            .map(|pump| join_named_pump(pump, "encoder output"))
            .unwrap_or(Ok(()));
        let stderr = self
            .stderr_pump
            .take()
            .map(|pump| join_named_pump(pump, "encoder stderr"))
            .unwrap_or(Ok(()));
        output.and(stderr)
    }

    fn reap_child(&mut self) {
        self.stdin.take();
        if !self.child_reaped {
            terminate_child(&mut self.child);
            self.child_reaped = true;
        }
        let _ = self.join_output_pump();
    }

    #[cfg(test)]
    fn child_id(&mut self) -> u32 {
        self.child.as_inner_mut().id()
    }

    fn copy_video_only(
        &mut self,
        cancel: &MediaCancelToken,
        progress: Option<&EncodeProgressCallback>,
    ) -> Result<()> {
        self.copy_file_to_output(&self.first_pass.clone(), cancel, progress, FIRST_PASS_END)
    }

    fn copy_file_to_output(
        &mut self,
        source_path: &Path,
        cancel: &MediaCancelToken,
        progress: Option<&EncodeProgressCallback>,
        progress_start: usize,
    ) -> Result<()> {
        let mut source = File::open(source_path).map_err(MediaError::Io)?;
        source.seek(SeekFrom::Start(0)).map_err(MediaError::Io)?;
        self.output.set_len(0).map_err(MediaError::Io)?;
        self.output
            .seek(SeekFrom::Start(0))
            .map_err(MediaError::Io)?;
        let total = source.metadata().map_err(MediaError::Io)?.len().max(1);
        let mut copied = 0_u64;
        let mut chunk = [0_u8; OUTPUT_COPY_CHUNK];
        loop {
            if cancel.checkpoint() {
                return Err(MediaError::Cancelled);
            }
            let read = source.read(&mut chunk).map_err(MediaError::Io)?;
            if read == 0 {
                break;
            }
            self.output
                .write_all(&chunk[..read])
                .map_err(MediaError::Io)?;
            copied = copied.saturating_add(read as u64);
            let mapped = progress_start
                + ((copied.min(total) * (ENCODE_PROGRESS_TOTAL - progress_start) as u64) / total)
                    as usize;
            report_progress(progress, mapped.min(ENCODE_PROGRESS_TOTAL - 1));
        }
        Ok(())
    }

    fn mux_audio(
        &mut self,
        pcm: &PcmBuffer,
        cancel: &MediaCancelToken,
        progress: Option<&EncodeProgressCallback>,
        mux_wait_hook: Option<&dyn Fn()>,
    ) -> Result<()> {
        let pcm_path = self.workspace.path().join("audio.pcm");
        let mut pcm_tmp = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&pcm_path)
            .map_err(MediaError::Io)?;
        write_pcm_s16le_cancellable(&pcm.samples_f32, &mut pcm_tmp, cancel, progress, None)?;
        pcm_tmp.flush().map_err(MediaError::Io)?;
        report_progress(progress, PCM_WRITE_END);

        let mux_path = self.workspace.path().join(
            if self
                .first_pass
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mov"))
            {
                "muxed.mov"
            } else {
                "muxed.mp4"
            },
        );
        let args = mux_args(
            &self.first_pass,
            &pcm_path,
            &mux_path,
            pcm.spec.sample_rate,
            self.acodec,
        );
        let mut child = crate::ff::ffmpeg()
            .args(args)
            .spawn()
            .map_err(|e| MediaError::Encode(format!("mux spawn: {e}")))?;
        let stdout = child.take_stdout().ok_or_else(|| {
            terminate_child(&mut child);
            MediaError::Encode("mux stdout pipe missing".to_string())
        })?;
        let stderr = child.take_stderr().ok_or_else(|| {
            terminate_child(&mut child);
            MediaError::Encode("mux stderr pipe missing".to_string())
        })?;
        let pump = match thread::Builder::new()
            .name("opentake-mux-stdout".to_string())
            .spawn(move || drain_stdout(stdout))
        {
            Ok(pump) => pump,
            Err(error) => {
                terminate_child(&mut child);
                return Err(MediaError::Encode(format!(
                    "spawn mux output pump: {error}"
                )));
            }
        };
        let stderr_pump = match thread::Builder::new()
            .name("opentake-mux-stderr".to_string())
            .spawn(move || drain_stderr(stderr))
        {
            Ok(pump) => pump,
            Err(error) => {
                terminate_child(&mut child);
                let _ = join_named_pump(pump, "mux output");
                return Err(MediaError::Encode(format!(
                    "spawn mux stderr pump: {error}"
                )));
            }
        };
        report_progress(progress, MUX_WAIT_START);
        let status = wait_external_child(
            &mut child,
            pump,
            stderr_pump,
            cancel,
            progress,
            mux_wait_hook,
        )?;
        if !status.success() {
            return Err(MediaError::Encode(format!("ffmpeg mux exited {status}")));
        }
        self.copy_file_to_output(&mux_path, cancel, progress, MUX_COPY_START)
    }
}

fn reject_link_output(out: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(out) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(MediaError::Io(error)),
    };
    #[cfg(not(windows))]
    let is_link = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let is_link = {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    };
    if is_link {
        return Err(MediaError::Encode(
            "refusing to encode through a symlink or reparse point".to_string(),
        ));
    }
    if !metadata.is_file() {
        return Err(MediaError::Encode(
            "encoder output must be a regular file".to_string(),
        ));
    }
    Ok(())
}

fn open_output_nofollow(out: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(out).map_err(MediaError::Io)?;
    let metadata = file.metadata().map_err(MediaError::Io)?;
    #[cfg(not(windows))]
    let is_link = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let is_link = {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    };
    if is_link || !metadata.is_file() {
        return Err(MediaError::Encode(
            "encoder output must be a regular non-link file".to_string(),
        ));
    }
    Ok(file)
}

impl Drop for VideoEncoder {
    fn drop(&mut self) {
        self.reap_child();
    }
}

fn drain_stdout(mut stdout: ChildStdout) -> Result<()> {
    let mut bytes = [0_u8; OUTPUT_COPY_CHUNK];
    loop {
        let read = stdout.read(&mut bytes).map_err(MediaError::Io)?;
        if read == 0 {
            return Ok(());
        }
    }
}

fn drain_stderr(mut stderr: ChildStderr) -> Result<()> {
    let mut bytes = [0_u8; 8 * 1024];
    loop {
        let read = stderr.read(&mut bytes).map_err(MediaError::Io)?;
        if read == 0 {
            return Ok(());
        }
    }
}

fn write_pcm_s16le_cancellable(
    samples: &[f32],
    destination: &mut File,
    cancel: &MediaCancelToken,
    progress: Option<&EncodeProgressCallback>,
    checkpoint_hook: Option<&dyn Fn(usize)>,
) -> Result<()> {
    destination.set_len(0).map_err(MediaError::Io)?;
    destination
        .seek(SeekFrom::Start(0))
        .map_err(MediaError::Io)?;
    for (chunk_index, chunk) in samples.chunks(OUTPUT_COPY_CHUNK / 2).enumerate() {
        let done = chunk_index.saturating_mul(OUTPUT_COPY_CHUNK / 2);
        if let Some(hook) = checkpoint_hook {
            hook(done);
        }
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        let bytes = mix::mono_f32_to_s16le(chunk);
        destination.write_all(&bytes).map_err(MediaError::Io)?;
        let completed = (done + chunk.len()).min(samples.len());
        let span = PCM_WRITE_END - FIRST_PASS_END;
        let mapped = FIRST_PASS_END + completed.saturating_mul(span) / samples.len().max(1);
        report_progress(progress, mapped);
    }
    if cancel.checkpoint() {
        return Err(MediaError::Cancelled);
    }
    Ok(())
}

fn wait_external_child(
    child: &mut ffmpeg_sidecar::child::FfmpegChild,
    output_pump: JoinHandle<Result<()>>,
    stderr_pump: JoinHandle<Result<()>>,
    cancel: &MediaCancelToken,
    progress: Option<&EncodeProgressCallback>,
    wait_hook: Option<&dyn Fn()>,
) -> Result<ExitStatus> {
    if let Some(hook) = wait_hook {
        hook();
    }
    let mut polls = 0_usize;
    loop {
        if cancel.checkpoint() {
            terminate_child(child);
            let _ = join_named_pump(output_pump, "mux output");
            let _ = join_named_pump(stderr_pump, "mux stderr");
            return Err(MediaError::Cancelled);
        }
        match child.as_inner_mut().try_wait() {
            Ok(Some(status)) => {
                let output = join_named_pump(output_pump, "mux output");
                let stderr = join_named_pump(stderr_pump, "mux stderr");
                output.and(stderr)?;
                return Ok(status);
            }
            Ok(None) => {
                polls = polls.saturating_add(1);
                if polls.is_multiple_of(20) {
                    report_progress(
                        progress,
                        MUX_WAIT_START + (polls / 20).min(MUX_COPY_START - MUX_WAIT_START - 1),
                    );
                }
                thread::sleep(ENCODE_POLL_INTERVAL);
            }
            Err(error) => {
                terminate_child(child);
                let _ = join_named_pump(output_pump, "mux output");
                let _ = join_named_pump(stderr_pump, "mux stderr");
                return Err(MediaError::Io(error));
            }
        }
    }
}

fn join_named_pump(pump: JoinHandle<Result<()>>, name: &str) -> Result<()> {
    pump.join()
        .map_err(|_| MediaError::Encode(format!("{name} pump panicked")))?
}

fn terminate_child(child: &mut ffmpeg_sidecar::child::FfmpegChild) {
    let _ = child.kill();
    let _ = child.wait();
}

fn report_progress(progress: Option<&EncodeProgressCallback>, done: usize) {
    if let Some(report) = progress {
        report(done.min(ENCODE_PROGRESS_TOTAL), ENCODE_PROGRESS_TOTAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[cfg(unix)]
    fn process_is_running(pid: u32) -> bool {
        std::process::Command::new("ps")
            .args(["-p", &pid.to_string()])
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(unix)]
    #[test]
    fn dropping_live_encoder_reaps_child_and_releases_output() {
        assert!(
            crate::ff::ffmpeg_available(),
            "encoder lifecycle test requires FFmpeg"
        );
        let temp = tempfile::tempdir().expect("encoder temp dir");
        let output = temp.path().join("live.mp4");
        let preset = ExportPreset::new(VideoCodec::H264, ExportResolution::P720);
        let mut encoder = VideoEncoder::new(&output, 16, 16, 30, &preset).expect("start encoder");
        let pid = encoder.child_id();

        assert!(process_is_running(pid), "encoder child must be live");
        drop(encoder);

        assert!(
            !process_is_running(pid),
            "drop must kill and wait for FFmpeg"
        );
        if output.exists() {
            std::fs::remove_file(&output).expect("reaped encoder releases output file");
        }
    }

    #[cfg(unix)]
    #[test]
    fn encoder_rejects_preexisting_output_symlink() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("encoder temp dir");
        let outside = temp.path().join("outside.mp4");
        std::fs::write(&outside, b"keep").expect("outside fixture");
        let output = temp.path().join("linked.mp4");
        symlink(&outside, &output).expect("output symlink");
        let preset = ExportPreset::new(VideoCodec::H264, ExportResolution::P720);

        let error = VideoEncoder::new(&output, 16, 16, 30, &preset)
            .err()
            .expect("encoder must reject output links");

        assert!(error.to_string().contains("symlink"));
        assert_eq!(std::fs::read(&outside).expect("outside readable"), b"keep");
    }

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
    fn cancellation_inside_mux_pcm_write_stops_the_actual_loop() {
        let temp = tempfile::tempdir().expect("PCM temp dir");
        let output = temp.path().join("audio.pcm");
        let samples = vec![0.25_f32; (OUTPUT_COPY_CHUNK / 2) * 4];
        let expected_full_len = samples.len() * 2;
        let cancel = MediaCancelToken::new();
        let worker_cancel = cancel.clone();
        let worker_output = output.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut destination = File::create(&worker_output).expect("create PCM destination");
            let hook = move |done: usize| {
                if done == OUTPUT_COPY_CHUNK / 2 {
                    entered_tx.send(()).expect("PCM checkpoint entered");
                    release_rx.recv().expect("release PCM checkpoint");
                }
            };
            write_pcm_s16le_cancellable(
                &samples,
                &mut destination,
                &worker_cancel,
                None,
                Some(&hook),
            )
        });

        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("actual PCM write loop reached its second chunk");
        cancel.cancel();
        release_tx.send(()).expect("release PCM loop");
        let result = worker.join().expect("PCM writer joins");

        assert!(matches!(result, Err(MediaError::Cancelled)));
        let partial_len = std::fs::metadata(&output)
            .expect("partial PCM exists")
            .len() as usize;
        assert!(partial_len > 0);
        assert!(partial_len < expected_full_len);
    }

    fn assert_cancelling_mux_wait_reaps_child() {
        assert!(
            crate::ff::ffmpeg_available(),
            "mux cancellation test requires FFmpeg"
        );
        let mut child = crate::ff::ffmpeg()
            .args([
                "-re",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=48000:cl=mono",
                "-t",
                "30",
                "-f",
                "null",
                "-",
            ])
            .spawn()
            .expect("spawn blocking mux-like FFmpeg");
        let stdout = child.take_stdout().expect("FFmpeg stdout");
        let stderr = child.take_stderr().expect("FFmpeg stderr");
        let output_pump = std::thread::spawn(move || drain_stdout(stdout));
        let stderr_pump = std::thread::spawn(move || drain_stderr(stderr));
        let cancel = MediaCancelToken::new();
        let worker_cancel = cancel.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let hook = move || {
                entered_tx.send(()).expect("mux wait entered");
                release_rx.recv().expect("release mux wait");
            };
            let result = wait_external_child(
                &mut child,
                output_pump,
                stderr_pump,
                &worker_cancel,
                None,
                Some(&hook),
            );
            let reaped = child
                .as_inner_mut()
                .try_wait()
                .expect("inspect mux child")
                .is_some();
            (result, reaped)
        });

        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("mux wait reached cancellation barrier");
        cancel.cancel();
        release_tx.send(()).expect("release mux wait");
        let (result, reaped) = worker.join().expect("mux wait worker joins");

        assert!(matches!(result, Err(MediaError::Cancelled)));
        assert!(reaped, "cancelled mux child must be killed and waited");
    }

    #[cfg(not(windows))]
    #[test]
    fn cancelling_mux_wait_reaps_child() {
        assert_cancelling_mux_wait_reaps_child();
    }

    #[cfg(windows)]
    #[test]
    fn windows_cancelling_mux_wait_reaps_child() {
        assert_cancelling_mux_wait_reaps_child();
    }
}
