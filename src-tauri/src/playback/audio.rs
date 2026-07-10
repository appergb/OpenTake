//! Audio master clock + cpal output for streaming playback (#63 / #160).
//!
//! The acceptance is "audio drives the playhead; video follows (dropping frames
//! to stay in sync)". [`build_clock`] realises that: when the timeline carries
//! sound it pre-mixes the whole timeline to one **interleaved stereo** buffer at
//! the cpal device sample rate, plays it through a dedicated cpal output thread,
//! and exposes the device's frame position as [`AudioClock`] — the master clock
//! the render loop reads to pick its target video frame. A silent timeline falls
//! back to the wall-clock [`InstantClock`] PR1 ships.
//!
//! ## Why preload-mix (not chunked streaming)
//! The cpal callback must never block or allocate. Pre-mixing to an immutable
//! buffer makes the callback a lock-free copy from `buffer[pos..]` (advancing one
//! `AtomicU64`), which is the simplest correct master clock — no live decode race
//! in the real-time audio thread. The cost is an up-front decode (off the IPC
//! thread, see `commands.rs`) + memory for the mix; chunked / background-filled
//! streaming for very long timelines is the remaining half of #160.
//!
//! Stereo is mixed once and mapped to the device's channel count in the callback
//! (mono downmix / >2 zero-fill). The mixing math mirrors the proven export
//! mixdown (`export.rs`), parameterised by the device rate and done per channel.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

use opentake_domain::{Clip, ClipType, Timeline};
use opentake_media::{
    decode_pcm_interleaved_cancellable, MediaCancelToken, MediaError, PcmFormat, PcmSpec,
};

use super::engine::{InstantClock, PlaybackClock};
use super::project::MediaInfo;

/// Default device sample rate when cpal can't report one (no device queried yet).
const FALLBACK_SAMPLE_RATE: u32 = 48_000;

/// The mix is always interleaved stereo; the callback maps it to the device's
/// channel count.
const MIX_CHANNELS: usize = 2;
const MAX_SESSION_PREMIX_BYTES: usize = 256 * 1024 * 1024;
const MIX_CANCEL_CHUNK_FRAMES: usize = 4 * 1024;

struct AudioPrepareJob<T> {
    build: Box<dyn FnOnce() -> T + Send + 'static>,
    result: tokio::sync::oneshot::Sender<T>,
}

/// One persistent blocking worker with exactly one admitted audio-prepare job.
pub struct AudioPrepareWorker<T: Send + 'static> {
    sender: SyncSender<AudioPrepareJob<T>>,
    occupied: Arc<AtomicBool>,
}

impl<T: Send + 'static> AudioPrepareWorker<T> {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel::<AudioPrepareJob<T>>(1);
        let occupied = Arc::new(AtomicBool::new(false));
        let worker_occupied = Arc::clone(&occupied);
        let _ = thread::Builder::new()
            .name("opentake-audio-prepare".to_string())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let value = (job.build)();
                    let _ = job.result.send(value);
                    worker_occupied.store(false, Ordering::Release);
                }
            });
        Self { sender, occupied }
    }

    pub fn try_submit(
        &self,
        build: impl FnOnce() -> T + Send + 'static,
    ) -> Result<tokio::sync::oneshot::Receiver<T>, String> {
        self.occupied
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "audio_prepare_busy".to_string())?;
        let (result, receiver) = tokio::sync::oneshot::channel();
        let job = AudioPrepareJob {
            build: Box::new(build),
            result,
        };
        match self.sender.try_send(job) {
            Ok(()) => Ok(receiver),
            Err(TrySendError::Full(_)) => {
                self.occupied.store(false, Ordering::Release);
                Err("audio_prepare_busy".to_string())
            }
            Err(TrySendError::Disconnected(_)) => {
                self.occupied.store(false, Ordering::Release);
                Err("audio_prepare_worker_stopped".to_string())
            }
        }
    }

    pub fn is_occupied(&self) -> bool {
        self.occupied.load(Ordering::Acquire)
    }
}

impl<T: Send + 'static> Default for AudioPrepareWorker<T> {
    fn default() -> Self {
        Self::new()
    }
}

fn audio_buffer_too_large(detail: impl std::fmt::Display) -> MediaError {
    MediaError::Decode(format!("audio_buffer_too_large: {detail}"))
}

fn audio_allocation_failed(detail: impl std::fmt::Display) -> MediaError {
    MediaError::Decode(format!("audio_allocation_failed: {detail}"))
}

fn checked_audio_bytes(frames: usize) -> Result<usize, MediaError> {
    frames
        .checked_mul(MIX_CHANNELS)
        .and_then(|samples| samples.checked_mul(std::mem::size_of::<f32>()))
        .ok_or_else(|| audio_buffer_too_large("stereo f32 byte count overflow"))
}

fn frames_at_rate(timeline_frames: i32, fps: i32, rate: u32) -> Result<usize, MediaError> {
    if timeline_frames <= 0 || fps <= 0 || rate == 0 {
        return Ok(0);
    }
    let frames = (f64::from(timeline_frames) / f64::from(fps) * f64::from(rate)).ceil();
    if !frames.is_finite() || frames > usize::MAX as f64 {
        return Err(audio_buffer_too_large("audio frame count exceeds usize"));
    }
    Ok(frames as usize)
}

fn projected_session_premix_bytes(timeline: &Timeline, rate: u32) -> Result<usize, MediaError> {
    let output_frames = frames_at_rate(timeline.total_frames(), timeline.fps, rate)?;
    let mut bytes = checked_audio_bytes(output_frames)?;
    for track in &timeline.tracks {
        if track.muted {
            continue;
        }
        for clip in &track.clips {
            if clip.media_type != ClipType::Audio && clip.media_type != ClipType::Video {
                continue;
            }
            let source_frames = clip.source_frames_consumed().max(0);
            let decoded_frames = frames_at_rate(source_frames, timeline.fps, rate)?;
            bytes = bytes
                .checked_add(checked_audio_bytes(decoded_frames)?)
                .ok_or_else(|| audio_buffer_too_large("session pre-mix byte count overflow"))?;
        }
    }
    Ok(bytes)
}

/// Audio master clock: the playhead derives from the device frame position
/// (`pos`, in output audio frames), which the cpal callback advances in lock-step
/// with the sound the user hears — so video genuinely follows audio.
pub struct AudioClock {
    /// Output audio frames played so far (shared with the cpal callback).
    pos: Arc<AtomicU64>,
    /// Output device sample rate (Hz = frames/sec).
    rate: u32,
    /// Project fps (for `seek`, which has no fps argument).
    fps: i32,
}

impl PlaybackClock for AudioClock {
    fn frame(&self, fps: i32) -> i32 {
        let fps = if fps > 0 { fps } else { self.fps.max(1) };
        let pos = self.pos.load(Ordering::Relaxed);
        // Truncate (secondsToFrame = Int(secs*fps)).
        ((pos as f64 / self.rate.max(1) as f64) * fps as f64) as i32
    }

    fn seek(&self, frame: i32) {
        let fps = self.fps.max(1);
        // Round (consistent with the clip placement in project_clip_audio_stereo)
        // so a seek round-trips back to the same frame even when the device rate
        // isn't a multiple of fps (e.g. 44100 Hz @ 24 fps) — plain truncation would
        // land a half-sample short and frame() would report frame-1.
        let pos = ((frame.max(0) as f64 / fps as f64) * self.rate as f64).round() as u64;
        // Release pairs with the callback's AcqRel fetch_add so it observes the seek.
        self.pos.store(pos, Ordering::Release);
    }
}

/// Owns the cpal output thread for a playback session. The cpal `Stream` is
/// `!Send` on macOS, so it lives entirely on that thread; this handle drives a
/// cooperative stop. Dropping it stops audio and joins the thread.
pub struct AudioPlayback {
    control_tx: Sender<AudioCmd>,
    paused: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

enum AudioCmd {
    Pause(Sender<Result<(), String>>),
    Resume(Sender<Result<(), String>>),
    Stop,
}

impl AudioPlayback {
    /// Start playing `buffer` (interleaved stereo, at the device rate) from `pos`.
    /// Returns `Err` if the device/stream can't be set up (caller falls back to
    /// the wall clock). Blocks until the stream is built so failures surface
    /// synchronously.
    fn start(
        buffer: Arc<Vec<f32>>,
        pos: Arc<AtomicU64>,
        paused: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let (control_tx, control_rx) = mpsc::channel::<AudioCmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let thread_paused = paused.clone();
        let handle = thread::Builder::new()
            .name("opentake-audio".to_string())
            .spawn(move || audio_thread(buffer, pos, thread_paused, control_rx, ready_tx))
            .map_err(|e| format!("spawn audio thread: {e}"))?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(AudioPlayback {
                control_tx,
                paused,
                handle: Some(handle),
            }),
            Ok(Err(e)) => {
                let _ = handle.join();
                Err(e)
            }
            Err(_) => Err("audio thread exited before init".to_string()),
        }
    }

    pub fn pause(&self) -> Result<(), String> {
        self.paused.store(true, Ordering::Release);
        self.control(AudioCmd::Pause)
    }

    pub fn resume(&self) -> Result<(), String> {
        self.control(AudioCmd::Resume)?;
        self.paused.store(false, Ordering::Release);
        Ok(())
    }

    fn control(
        &self,
        command: impl FnOnce(Sender<Result<(), String>>) -> AudioCmd,
    ) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.control_tx
            .send(command(reply_tx))
            .map_err(|_| "audio thread exited before transport control".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "audio thread exited during transport control".to_string())?
    }

    pub fn mute(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn request_stop(mut self) -> Option<JoinHandle<()>> {
        self.mute();
        let _ = self.control_tx.send(AudioCmd::Stop);
        self.handle.take()
    }

    #[cfg(test)]
    pub(crate) fn test_stub() -> (Self, Arc<AtomicBool>, Receiver<()>) {
        let (control_tx, control_rx) = mpsc::channel();
        let (stopped_tx, stopped_rx) = mpsc::channel();
        let paused = Arc::new(AtomicBool::new(false));
        let handle = thread::spawn(move || {
            while let Ok(command) = control_rx.recv() {
                match command {
                    AudioCmd::Pause(reply) | AudioCmd::Resume(reply) => {
                        let _ = reply.send(Ok(()));
                    }
                    AudioCmd::Stop => {
                        let _ = stopped_tx.send(());
                        break;
                    }
                }
            }
        });
        (
            Self {
                control_tx,
                paused: Arc::clone(&paused),
                handle: Some(handle),
            },
            paused,
            stopped_rx,
        )
    }
}

impl Drop for AudioPlayback {
    fn drop(&mut self) {
        let _ = self.control_tx.send(AudioCmd::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The audio thread: build + play the output stream, report the result, then park
/// (holding the `!Send` stream alive) until a stop is requested.
fn audio_thread(
    buffer: Arc<Vec<f32>>,
    pos: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    control_rx: Receiver<AudioCmd>,
    ready_tx: Sender<Result<(), String>>,
) {
    match build_and_play(&buffer, &pos, &paused) {
        Ok(stream) => {
            let _ = ready_tx.send(Ok(()));
            while let Ok(command) = control_rx.recv() {
                match command {
                    AudioCmd::Pause(reply) => {
                        let _ =
                            reply.send(stream.pause().map_err(|e| format!("stream pause: {e}")));
                    }
                    AudioCmd::Resume(reply) => {
                        let _ =
                            reply.send(stream.play().map_err(|e| format!("stream resume: {e}")));
                    }
                    AudioCmd::Stop => break,
                }
            }
            drop(stream);
        }
        Err(e) => {
            let _ = ready_tx.send(Err(e));
        }
    }
}

/// Acquire the default output device + config, build the typed output stream, and
/// start it. The returned `Stream` must stay alive on the calling thread.
fn build_and_play(
    buffer: &Arc<Vec<f32>>,
    pos: &Arc<AtomicU64>,
    paused: &Arc<AtomicBool>,
) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no default audio output device".to_string())?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("default output config: {e}"))?;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let stream = build_stream(
        sample_format,
        &device,
        &config,
        buffer.clone(),
        pos.clone(),
        paused.clone(),
    )?;
    if !paused.load(Ordering::Acquire) {
        stream.play().map_err(|e| format!("stream play: {e}"))?;
    }
    Ok(stream)
}

/// Dispatch on the device sample format to the typed stream builder.
fn build_stream(
    format: cpal::SampleFormat,
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Vec<f32>>,
    pos: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
) -> Result<cpal::Stream, String> {
    // Cover every fixed-size cpal format (all satisfy SizedSample + FromSample<f32>)
    // so a non-F32 default device (I32 is common on Linux/Windows) still gets audio
    // instead of silently falling back to the wall clock.
    match format {
        cpal::SampleFormat::F32 => out_stream::<f32>(device, config, buffer, pos, paused),
        cpal::SampleFormat::F64 => out_stream::<f64>(device, config, buffer, pos, paused),
        cpal::SampleFormat::I8 => out_stream::<i8>(device, config, buffer, pos, paused),
        cpal::SampleFormat::I16 => out_stream::<i16>(device, config, buffer, pos, paused),
        cpal::SampleFormat::I32 => out_stream::<i32>(device, config, buffer, pos, paused),
        cpal::SampleFormat::I64 => out_stream::<i64>(device, config, buffer, pos, paused),
        cpal::SampleFormat::U8 => out_stream::<u8>(device, config, buffer, pos, paused),
        cpal::SampleFormat::U16 => out_stream::<u16>(device, config, buffer, pos, paused),
        cpal::SampleFormat::U32 => out_stream::<u32>(device, config, buffer, pos, paused),
        cpal::SampleFormat::U64 => out_stream::<u64>(device, config, buffer, pos, paused),
        other => Err(format!("unsupported cpal sample format: {other}")),
    }
}

/// Write one interleaved stereo `(left, right)` sample to a device output frame,
/// mapping to its channel count: mono = average, stereo = L/R, >2 = L/R then
/// silence. Pure (no I/O) so the mapping is unit-tested.
fn write_frame<T: cpal::Sample + FromSample<f32>>(frame: &mut [T], left: f32, right: f32) {
    match frame.len() {
        0 => {}
        1 => frame[0] = T::from_sample((left + right) * 0.5),
        _ => {
            frame[0] = T::from_sample(left);
            frame[1] = T::from_sample(right);
            for sample in frame[2..].iter_mut() {
                *sample = T::from_sample(0.0f32);
            }
        }
    }
}

/// Build an output stream whose callback maps the interleaved stereo mix to the
/// device channels and advances `pos` by the frames written — the lock-free
/// master-clock tick.
fn out_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Vec<f32>>,
    pos: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = (config.channels as usize).max(1);
    let err_fn = |e| eprintln!("[audio] stream error: {e}");
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let out_frames = data.len() / channels;
                // Atomically claim this block's start frame and advance the master
                // clock. A concurrent `seek` (store) is honored on the next
                // callback; within a block we play from the claimed start.
                if paused.load(Ordering::Acquire) {
                    for sample in data.iter_mut() {
                        *sample = T::from_sample(0.0f32);
                    }
                    return;
                }
                let start = pos.fetch_add(out_frames as u64, Ordering::AcqRel) as usize;
                for (i, frame) in data.chunks_mut(channels).enumerate() {
                    let base = (start + i) * MIX_CHANNELS;
                    let (left, right) = if base + 1 < buffer.len() {
                        (buffer[base], buffer[base + 1])
                    } else {
                        (0.0, 0.0) // past the mix end → silence (video may outlast audio)
                    };
                    write_frame(frame, left, right);
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("build output stream: {e}"))
}

/// Query the default output device's sample rate (Hz), or `None` if unavailable.
fn default_output_rate() -> Option<u32> {
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let config = device.default_output_config().ok()?;
    Some(config.sample_rate().0)
}

/// Source-media window `[lo, hi)` seconds a clip consumes (trim + speed). Mirrors
/// `export::clip_source_window_secs`.
fn clip_source_window_secs(clip: &Clip, timeline_fps: i32) -> Option<(f64, f64)> {
    if clip.duration_frames <= 0 || timeline_fps <= 0 {
        return None;
    }
    let fps = timeline_fps as f64;
    let lo = clip.trim_start_frame.max(0) as f64 / fps;
    let consumed = clip.source_frames_consumed().max(0);
    if consumed == 0 {
        return None;
    }
    Some((lo, lo + consumed as f64 / fps))
}

/// One clip's decoded audio, placed on the output timeline as interleaved stereo
/// at the device rate, with its per-output-frame `volume_at` gain envelope.
struct StereoClip {
    /// Output audio-frame offset on the timeline (sample index = ×2).
    start_frame: usize,
    /// Interleaved stereo samples (length = 2 × frames).
    interleaved: Vec<f32>,
    /// Per-output-frame gain (length = frames; empty = unity throughout).
    gains: Vec<f32>,
}

/// Decode one clip's visible audio window into a placed [`StereoClip`] at `rate`
/// (interleaved stereo). `None` when the clip contributes no audio.
fn project_clip_audio_stereo(
    clip: &Clip,
    media: &HashMap<String, MediaInfo>,
    timeline_fps: i32,
    rate: u32,
    cancel: &MediaCancelToken,
) -> Result<Option<StereoClip>, MediaError> {
    if clip.duration_frames <= 0 || timeline_fps <= 0 || rate == 0 {
        return Ok(None);
    }
    let Some(info) = media.get(&clip.media_ref) else {
        return Ok(None);
    };
    let Some((lo, hi)) = clip_source_window_secs(clip, timeline_fps) else {
        return Ok(None);
    };

    let spec = PcmSpec {
        sample_rate: rate,
        channels: MIX_CHANNELS as u16,
        format: PcmFormat::F32,
    };
    let interleaved =
        decode_pcm_interleaved_cancellable(&info.path, &spec, Some((lo, hi)), cancel)?;
    let frames = interleaved.len() / MIX_CHANNELS;
    if frames == 0 {
        return Ok(None);
    }

    let start_frame =
        ((clip.start_frame.max(0) as f64) / timeline_fps as f64 * rate as f64).round() as usize;
    let frames_per_tl_frame = rate as f64 / timeline_fps as f64;
    let mut gains = Vec::new();
    gains
        .try_reserve_exact(frames)
        .map_err(|error| audio_allocation_failed(format!("gain reserve {frames}: {error}")))?;
    let mut all_unity = true;
    for k in 0..frames {
        let tl_frame = clip.start_frame + (k as f64 / frames_per_tl_frame).floor() as i32;
        let g = clip.volume_at(tl_frame) as f32;
        if (g - 1.0).abs() > f32::EPSILON {
            all_unity = false;
        }
        gains.push(g);
    }

    Ok(Some(StereoClip {
        start_frame,
        interleaved,
        gains: if all_unity { Vec::new() } else { gains },
    }))
}

/// Sum placed stereo clips into one interleaved buffer, applying per-frame gains
/// and hard-limiting to [-1, 1] (mirrors the export mixdown, per channel).
fn mix_stereo(clips: &[StereoClip], cancel: &MediaCancelToken) -> Result<Vec<f32>, MediaError> {
    let total_frames = clips
        .iter()
        .map(|c| {
            c.start_frame
                .checked_add(c.interleaved.len() / MIX_CHANNELS)
                .ok_or_else(|| audio_buffer_too_large("mix frame extent overflow"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .unwrap_or(0);
    let total_samples = total_frames
        .checked_mul(MIX_CHANNELS)
        .ok_or_else(|| audio_buffer_too_large("mix sample count overflow"))?;
    let mut out = Vec::new();
    out.try_reserve_exact(total_samples).map_err(|error| {
        audio_allocation_failed(format!("mix reserve {total_samples}: {error}"))
    })?;
    out.resize(total_samples, 0.0);
    for c in clips {
        let frames = c.interleaved.len() / MIX_CHANNELS;
        for chunk_start in (0..frames).step_by(MIX_CANCEL_CHUNK_FRAMES) {
            if cancel.checkpoint() {
                return Err(MediaError::Cancelled);
            }
            let chunk_end = (chunk_start + MIX_CANCEL_CHUNK_FRAMES).min(frames);
            for k in chunk_start..chunk_end {
                let g = if c.gains.is_empty() { 1.0 } else { c.gains[k] };
                let o = (c.start_frame + k) * MIX_CHANNELS;
                out[o] += c.interleaved[k * MIX_CHANNELS] * g;
                out[o + 1] += c.interleaved[k * MIX_CHANNELS + 1] * g;
            }
        }
    }
    for chunk in out.chunks_mut(MIX_CANCEL_CHUNK_FRAMES * MIX_CHANNELS) {
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        for value in chunk {
            *value = value.clamp(-1.0, 1.0);
        }
    }
    Ok(out)
}

/// Pre-mix every audio-bearing clip into one interleaved stereo buffer at `rate`.
/// Empty when the timeline has no audio (→ caller uses the wall clock).
fn mix_timeline_stereo(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    rate: u32,
    cancel: &MediaCancelToken,
) -> Result<Vec<f32>, MediaError> {
    if timeline.fps <= 0 || rate == 0 {
        return Ok(Vec::new());
    }
    let projected_bytes = projected_session_premix_bytes(timeline, rate)?;
    if projected_bytes > MAX_SESSION_PREMIX_BYTES {
        return Err(audio_buffer_too_large(format!(
            "projected {projected_bytes} bytes exceeds {MAX_SESSION_PREMIX_BYTES}"
        )));
    }
    let mut clips: Vec<StereoClip> = Vec::new();
    for track in &timeline.tracks {
        if track.muted {
            continue;
        }
        for clip in &track.clips {
            if cancel.checkpoint() {
                return Err(MediaError::Cancelled);
            }
            if clip.media_type != ClipType::Audio && clip.media_type != ClipType::Video {
                continue;
            }
            if let Some(sc) = project_clip_audio_stereo(clip, media, timeline.fps, rate, cancel)? {
                clips.push(sc);
            }
        }
    }
    if clips.is_empty() {
        return Ok(Vec::new());
    }
    mix_stereo(&clips, cancel)
}

/// Build the playback clock for a session starting at `start_frame`.
///
/// Pre-mixes the timeline audio; if there's sound, plays it through cpal and
/// returns an [`AudioClock`] (audio is master) + the live [`AudioPlayback`].
/// Otherwise — or if the audio device can't be opened — returns the wall-clock
/// [`InstantClock`], so a silent project (or a headless host) still plays video.
pub fn build_clock(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    fps: i32,
    start_frame: i32,
) -> (Arc<dyn PlaybackClock>, Option<AudioPlayback>) {
    build_clock_with_state(
        timeline,
        media,
        fps,
        start_frame,
        false,
        &MediaCancelToken::new(),
    )
    .unwrap_or_else(|error| panic!("playback audio preparation failed: {error}"))
}

/// Prepare audio without starting the device clock. The retained playback
/// session resumes audio only after its first composited frame is buffered.
pub fn build_clock_paused(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    fps: i32,
    start_frame: i32,
) -> Result<(Arc<dyn PlaybackClock>, Option<AudioPlayback>), String> {
    build_clock_paused_cancellable(timeline, media, fps, start_frame, &MediaCancelToken::new())
}

pub fn build_clock_paused_cancellable(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    fps: i32,
    start_frame: i32,
    cancel: &MediaCancelToken,
) -> Result<(Arc<dyn PlaybackClock>, Option<AudioPlayback>), String> {
    build_clock_with_state(timeline, media, fps, start_frame, true, cancel)
        .map_err(|error| error.to_string())
}

fn build_clock_with_state(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    fps: i32,
    start_frame: i32,
    start_paused: bool,
    cancel: &MediaCancelToken,
) -> Result<(Arc<dyn PlaybackClock>, Option<AudioPlayback>), MediaError> {
    let rate = default_output_rate().unwrap_or(FALLBACK_SAMPLE_RATE);
    let mixed = mix_timeline_stereo(timeline, media, rate, cancel)?;
    if mixed.is_empty() {
        return Ok((Arc::new(InstantClock::new(start_frame)), None));
    }

    let buffer = Arc::new(mixed);
    let pos = Arc::new(AtomicU64::new(0));
    let paused = Arc::new(AtomicBool::new(start_paused));
    let clock = AudioClock {
        pos: pos.clone(),
        rate,
        fps,
    };
    clock.seek(start_frame); // begin playback at the current playhead

    match AudioPlayback::start(buffer, pos, paused) {
        Ok(audio) => Ok((Arc::new(clock), Some(audio))),
        Err(e) => {
            eprintln!("[audio] {e}; falling back to wall clock");
            Ok((Arc::new(InstantClock::new(start_frame)), None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use opentake_domain::{Clip, Track};
    use opentake_media::{MediaCancelToken, MediaError};

    fn audio_timeline(clips: Vec<Clip>) -> Timeline {
        let mut timeline = Timeline::new();
        timeline.fps = 30;
        let mut track = Track::new("a1", ClipType::Audio);
        track.clips = clips;
        timeline.tracks.push(track);
        timeline
    }

    fn audio_clip(id: &str, media_ref: &str, start_frame: i32, duration_frames: i32) -> Clip {
        let mut clip = Clip::new(id, media_ref, start_frame, duration_frames);
        clip.media_type = ClipType::Audio;
        clip.source_clip_type = ClipType::Audio;
        clip
    }

    #[test]
    fn audio_prepare_cancel_stops_before_decoding_next_clip() {
        assert!(
            opentake_media::ffmpeg_status::ffmpeg_available(),
            "required cancellation test needs a runnable FFmpeg"
        );
        let temp = tempfile::tempdir().expect("create audio cancellation fixtures");
        let first = temp.path().join("first.wav");
        let second = temp.path().join("second.wav");
        for fifo in [&first, &second] {
            let status = std::process::Command::new("mkfifo")
                .arg(fifo)
                .status()
                .expect("spawn mkfifo");
            assert!(status.success());
        }
        let timeline = audio_timeline(vec![
            audio_clip("c1", "m1", 0, 900),
            audio_clip("c2", "m2", 900, 900),
        ]);
        let media = HashMap::from([
            ("m1".to_string(), MediaInfo { path: first }),
            ("m2".to_string(), MediaInfo { path: second }),
        ]);
        let cancel = MediaCancelToken::new();
        let worker_cancel = cancel.clone();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            done_tx
                .send(mix_timeline_stereo(
                    &timeline,
                    &media,
                    48_000,
                    &worker_cancel,
                ))
                .expect("publish audio prepare result");
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while cancel.spawned_child_count() == 0 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(cancel.spawned_child_count(), 1);
        cancel.cancel();
        let error = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("cancelled audio prepare must return")
            .expect_err("cancelled audio prepare must fail");
        assert!(matches!(error, MediaError::Cancelled));
        worker.join().expect("join audio prepare worker");
        assert_eq!(
            cancel.spawned_child_count(),
            1,
            "second clip must not spawn"
        );
    }

    #[test]
    fn large_mix_observes_cancellation_between_chunks() {
        let cancel = MediaCancelToken::new();
        let worker_cancel = cancel.clone();
        let clip = StereoClip {
            start_frame: 0,
            interleaved: vec![0.25; 12_000_000],
            gains: Vec::new(),
        };
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            done_tx
                .send(mix_stereo(&[clip], &worker_cancel))
                .expect("publish mix result");
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while cancel.checkpoint_count() == 0 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(
            cancel.checkpoint_count() > 0,
            "mix must enter a production chunk"
        );
        cancel.cancel();
        let result = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("chunked mix must observe cancellation");
        assert!(matches!(result, Err(MediaError::Cancelled)));
        worker.join().expect("join mix worker");
    }

    #[test]
    fn audio_prepare_rejects_projected_mix_over_256_mib_without_allocation() {
        let timeline = audio_timeline(vec![audio_clip("huge", "missing", 0, 30 * 60 * 12)]);
        let media = HashMap::from([(
            "missing".to_string(),
            MediaInfo {
                path: PathBuf::from("/must/not/be/decoded.wav"),
            },
        )]);
        let cancel = MediaCancelToken::new();

        let error = mix_timeline_stereo(&timeline, &media, 48_000, &cancel)
            .expect_err("a 12-minute stereo f32 pre-mix exceeds 256 MiB");

        assert!(error.to_string().contains("audio_buffer_too_large"));
        assert_eq!(cancel.spawned_child_count(), 0);
    }

    #[test]
    fn audio_decode_failure_is_not_silently_treated_as_silent_timeline() {
        let timeline = audio_timeline(vec![audio_clip("broken", "missing", 0, 30)]);
        let media = HashMap::from([(
            "missing".to_string(),
            MediaInfo {
                path: PathBuf::from("/definitely/missing/audio.wav"),
            },
        )]);

        let error = mix_timeline_stereo(&timeline, &media, 48_000, &MediaCancelToken::new())
            .expect_err("decode failure must propagate instead of producing an empty mix");

        assert!(!matches!(error, MediaError::Cancelled));
    }

    #[test]
    fn rapid_superseding_starts_never_exceed_one_audio_prepare_job() {
        let worker = AudioPrepareWorker::<usize>::new();
        let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let job_active = Arc::clone(&active);
        let job_max = Arc::clone(&max_active);
        let first = worker
            .try_submit(move || {
                let now = job_active.fetch_add(1, Ordering::AcqRel) + 1;
                job_max.fetch_max(now, Ordering::AcqRel);
                entered_tx.send(()).expect("announce first prepare");
                release_rx.recv().expect("release first prepare");
                job_active.fetch_sub(1, Ordering::AcqRel);
                1
            })
            .expect("first prepare admitted");
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("first prepare starts");

        for _ in 0..16 {
            assert!(
                worker.try_submit(|| 2).is_err(),
                "superseding starts must be busy while one prepare owns capacity"
            );
        }
        release_tx.send(()).expect("release first prepare");
        assert_eq!(first.blocking_recv().expect("first result"), 1);
        assert_eq!(max_active.load(Ordering::Acquire), 1);
    }

    #[test]
    fn cancelled_prepare_releases_capacity_only_after_worker_exits() {
        let worker = AudioPrepareWorker::<()>::new();
        let cancel = MediaCancelToken::new();
        let job_cancel = cancel.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let first = worker
            .try_submit(move || {
                entered_tx.send(()).expect("announce prepare");
                while !job_cancel.is_cancelled() {
                    std::thread::yield_now();
                }
                release_rx
                    .recv()
                    .expect("hold cancelled worker before exit");
            })
            .expect("first prepare admitted");
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("prepare starts");
        cancel.cancel();

        assert!(worker.try_submit(|| ()).is_err());
        release_tx.send(()).expect("allow cancelled worker exit");
        first.blocking_recv().expect("cancelled worker exits");
        let deadline = Instant::now() + Duration::from_secs(2);
        while worker.is_occupied() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(!worker.is_occupied());
        worker
            .try_submit(|| ())
            .expect("capacity releases only after exit")
            .blocking_recv()
            .expect("replacement prepare completes");
    }

    #[test]
    fn audio_clock_frame_and_seek_round_trip() {
        let clock = AudioClock {
            pos: Arc::new(AtomicU64::new(0)),
            rate: 48_000,
            fps: 30,
        };
        // seek(30) → 30 frames = 1s = 48000 output frames → frame()==30.
        clock.seek(30);
        assert_eq!(clock.pos.load(Ordering::Relaxed), 48_000);
        assert_eq!(clock.frame(30), 30);

        // Half a second of frames → frame 15.
        clock.pos.store(24_000, Ordering::Relaxed);
        assert_eq!(clock.frame(30), 15);
    }

    #[test]
    fn audio_clock_truncates_partial_frames() {
        let clock = AudioClock {
            pos: Arc::new(AtomicU64::new(0)),
            rate: 48_000,
            fps: 30,
        };
        // 1599 frames @ 48k, 30fps = 0.999 video frame → truncates to 0.
        clock.pos.store(1_599, Ordering::Relaxed);
        assert_eq!(clock.frame(30), 0);
        // 1600 frames = exactly one video frame.
        clock.pos.store(1_600, Ordering::Relaxed);
        assert_eq!(clock.frame(30), 1);
    }

    #[test]
    fn audio_clock_seek_round_trips_at_non_divisible_rate() {
        // 44100 Hz @ 24 fps: rate/fps = 1837.5 (not integer). seek (round) +
        // frame (truncate) must still land back on the same frame — a regression
        // guard for the truncate-only seek that reported frame-1 here.
        let clock = AudioClock {
            pos: Arc::new(AtomicU64::new(0)),
            rate: 44_100,
            fps: 24,
        };
        for f in [1, 7, 23, 100, 511] {
            clock.seek(f);
            assert_eq!(clock.frame(24), f, "seek({f}) must round-trip");
        }
    }

    #[test]
    fn clip_source_window_uses_timeline_fps() {
        let mut clip = Clip::new("c1", "asset-1", 0, 60);
        clip.trim_start_frame = 15;
        clip.speed = 1.0;
        let (lo, hi) = clip_source_window_secs(&clip, 30).expect("window");
        assert!((lo - 0.5).abs() < 1e-6);
        assert!((hi - 2.5).abs() < 1e-6);
    }

    #[test]
    fn project_clip_audio_stereo_skips_clip_without_media_entry() {
        let clip = Clip::new("c1", "missing", 0, 30);
        let media: HashMap<String, MediaInfo> = HashMap::new();
        assert!(
            project_clip_audio_stereo(&clip, &media, 30, 48_000, &MediaCancelToken::new())
                .expect("missing media is silent")
                .is_none()
        );
    }

    #[test]
    fn mix_timeline_stereo_empty_when_no_audio_clips() {
        let timeline = Timeline::new();
        let media: HashMap<String, MediaInfo> = HashMap::new();
        assert!(
            mix_timeline_stereo(&timeline, &media, 48_000, &MediaCancelToken::new())
                .expect("empty timeline")
                .is_empty()
        );
    }

    #[test]
    fn mix_stereo_sums_placed_clips_and_clamps() {
        // Clip A at frame 0: 2 stereo frames [(0.6,-0.6),(0.5,0.5)].
        // Clip B at frame 1: 1 stereo frame (0.6,0.6) → overlaps A's frame 1.
        let a = StereoClip {
            start_frame: 0,
            interleaved: vec![0.6, -0.6, 0.5, 0.5],
            gains: Vec::new(),
        };
        let b = StereoClip {
            start_frame: 1,
            interleaved: vec![0.6, 0.6],
            gains: Vec::new(),
        };
        let out = mix_stereo(&[a, b], &MediaCancelToken::new()).expect("mix");
        assert_eq!(out.len(), 4); // 2 frames × 2 channels
                                  // frame 0 = A only.
        assert!((out[0] - 0.6).abs() < 1e-6);
        assert!((out[1] + 0.6).abs() < 1e-6);
        // frame 1 = A(0.5,0.5) + B(0.6,0.6) = (1.1,1.1) → clamped to (1.0,1.0).
        assert!((out[2] - 1.0).abs() < 1e-6);
        assert!((out[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn mix_stereo_applies_per_frame_gain() {
        let c = StereoClip {
            start_frame: 0,
            interleaved: vec![1.0, 1.0, 1.0, 1.0],
            gains: vec![0.5, 0.25],
        };
        let out = mix_stereo(&[c], &MediaCancelToken::new()).expect("mix");
        assert_eq!(out, vec![0.5, 0.5, 0.25, 0.25]);
    }

    #[test]
    fn write_frame_maps_to_device_channels() {
        // Mono device: average L+R.
        let mut mono = [0.0f32; 1];
        write_frame(&mut mono, 1.0, -1.0);
        assert!((mono[0] - 0.0).abs() < 1e-6);

        // Stereo device: L/R passthrough.
        let mut stereo = [0.0f32; 2];
        write_frame(&mut stereo, 0.3, -0.4);
        assert_eq!(stereo, [0.3, -0.4]);

        // Surround device: L, R, then silence on the extra channels.
        let mut surround = [9.0f32; 4];
        write_frame(&mut surround, 0.3, -0.4);
        assert_eq!(surround, [0.3, -0.4, 0.0, 0.0]);
    }
}
