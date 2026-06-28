//! Audio master clock + cpal output for streaming playback (#63).
//!
//! The acceptance is "audio drives the playhead; video follows (dropping frames
//! to stay in sync)". [`build_clock`] realises that: when the timeline carries
//! sound it pre-mixes the whole timeline to one mono buffer at the cpal device
//! sample rate, plays it through a dedicated cpal output thread, and exposes the
//! device's sample position as [`AudioClock`] — the master clock the render loop
//! reads to pick its target frame. A silent timeline falls back to the wall-clock
//! [`InstantClock`] PR1 ships.
//!
//! ## Why preload-mix (not chunked streaming)
//! The cpal callback must never block or allocate. Pre-mixing to an immutable
//! buffer makes the callback a lock-free copy from `buffer[pos..]` (advancing one
//! `AtomicU64`), which is the simplest correct master clock — no live decode race
//! in the real-time audio thread. The cost is an up-front decode + memory for the
//! mix; chunked/stereo streaming for very long timelines is a follow-up.
//!
//! Mono preview audio is intentional for this cut: the clock only needs the
//! sample *count*, and `extract_pcm` already returns a mono mixdown. Stereo
//! panning is a follow-up. The mixing math mirrors the proven export mixdown
//! (`export.rs`), parameterised by the device rate.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

use opentake_domain::{Clip, ClipType, Timeline};
use opentake_media::encode::{mix, ClipAudio};
use opentake_media::{extract_pcm, PcmFormat, PcmSpec};

use super::engine::{InstantClock, PlaybackClock};
use super::project::MediaInfo;

/// Default device sample rate when cpal can't report one (no device queried yet).
const FALLBACK_SAMPLE_RATE: u32 = 48_000;

/// Audio master clock: the playhead derives from the device sample position
/// (`pos`, in mono output samples), which the cpal callback advances in lock-step
/// with the sound the user hears — so video genuinely follows audio.
pub struct AudioClock {
    /// Mono output samples played so far (shared with the cpal callback).
    pos: Arc<AtomicU64>,
    /// Output device sample rate (Hz).
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
        let pos = (frame.max(0) as u64 * self.rate as u64) / fps as u64;
        self.pos.store(pos, Ordering::Relaxed);
    }
}

/// Owns the cpal output thread for a playback session. The cpal `Stream` is
/// `!Send` on macOS, so it lives entirely on that thread; this handle drives a
/// cooperative stop. Dropping it stops audio and joins the thread.
pub struct AudioPlayback {
    stop_tx: Sender<()>,
    handle: Option<JoinHandle<()>>,
}

impl AudioPlayback {
    /// Start playing `buffer` (mono, at the device rate) from `pos`. Returns
    /// `Err` if the device/stream can't be set up (caller falls back to the wall
    /// clock). Blocks until the stream is built so failures surface synchronously.
    fn start(buffer: Arc<Vec<f32>>, pos: Arc<AtomicU64>) -> Result<Self, String> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let handle = thread::Builder::new()
            .name("opentake-audio".to_string())
            .spawn(move || audio_thread(buffer, pos, stop_rx, ready_tx))
            .map_err(|e| format!("spawn audio thread: {e}"))?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(AudioPlayback {
                stop_tx,
                handle: Some(handle),
            }),
            Ok(Err(e)) => {
                let _ = handle.join();
                Err(e)
            }
            Err(_) => Err("audio thread exited before init".to_string()),
        }
    }
}

impl Drop for AudioPlayback {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
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
    stop_rx: Receiver<()>,
    ready_tx: Sender<Result<(), String>>,
) {
    match build_and_play(&buffer, &pos) {
        Ok(stream) => {
            let _ = ready_tx.send(Ok(()));
            // Park until stop (or the handle is dropped); then drop the stream.
            let _ = stop_rx.recv();
            drop(stream);
        }
        Err(e) => {
            let _ = ready_tx.send(Err(e));
        }
    }
}

/// Acquire the default output device + config, build the typed output stream, and
/// start it. The returned `Stream` must stay alive on the calling thread.
fn build_and_play(buffer: &Arc<Vec<f32>>, pos: &Arc<AtomicU64>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no default audio output device".to_string())?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("default output config: {e}"))?;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let stream = build_stream(sample_format, &device, &config, buffer.clone(), pos.clone())?;
    stream.play().map_err(|e| format!("stream play: {e}"))?;
    Ok(stream)
}

/// Dispatch on the device sample format to the typed stream builder.
fn build_stream(
    format: cpal::SampleFormat,
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Vec<f32>>,
    pos: Arc<AtomicU64>,
) -> Result<cpal::Stream, String> {
    match format {
        cpal::SampleFormat::F32 => out_stream::<f32>(device, config, buffer, pos),
        cpal::SampleFormat::I16 => out_stream::<i16>(device, config, buffer, pos),
        cpal::SampleFormat::U16 => out_stream::<u16>(device, config, buffer, pos),
        other => Err(format!("unsupported audio sample format: {other}")),
    }
}

/// Build an output stream whose callback copies the mono mix to every channel and
/// advances `pos` by the frames written — the lock-free master-clock tick.
fn out_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Vec<f32>>,
    pos: Arc<AtomicU64>,
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
                let frames = data.len() / channels;
                // Atomically claim this block's mono start index and advance the
                // master clock. A concurrent `seek` (store) is honored on the next
                // callback; within a block we play from the claimed start.
                let start = pos.fetch_add(frames as u64, Ordering::Relaxed) as usize;
                for (i, frame) in data.chunks_mut(channels).enumerate() {
                    let s = buffer.get(start + i).copied().unwrap_or(0.0);
                    let value = T::from_sample(s);
                    for sample in frame.iter_mut() {
                        *sample = value;
                    }
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

/// Decode one clip's visible audio window into a placed [`ClipAudio`] at `rate`
/// (mono), with its per-sample `volume_at` gain envelope. Mirrors
/// `export::project_clip_audio`, parameterised by the device sample rate.
fn project_clip_audio(
    clip: &Clip,
    media: &HashMap<String, MediaInfo>,
    timeline_fps: i32,
    rate: u32,
) -> Option<ClipAudio> {
    if clip.duration_frames <= 0 || timeline_fps <= 0 || rate == 0 {
        return None;
    }
    let info = media.get(&clip.media_ref)?;
    let (lo, hi) = clip_source_window_secs(clip, timeline_fps)?;

    let spec = PcmSpec {
        sample_rate: rate,
        channels: 1,
        format: PcmFormat::F32,
    };
    let pcm = extract_pcm(&info.path, &spec, Some((lo, hi))).ok()?;
    if pcm.samples_f32.is_empty() {
        return None;
    }

    let start_sample =
        ((clip.start_frame.max(0) as f64) / timeline_fps as f64 * rate as f64).round() as usize;
    let samples_per_frame = rate as f64 / timeline_fps as f64;
    let mut gains = Vec::with_capacity(pcm.samples_f32.len());
    let mut all_unity = true;
    for k in 0..pcm.samples_f32.len() {
        let tl_frame = clip.start_frame + (k as f64 / samples_per_frame).floor() as i32;
        let g = clip.volume_at(tl_frame) as f32;
        if (g - 1.0).abs() > f32::EPSILON {
            all_unity = false;
        }
        gains.push(g);
    }

    Some(ClipAudio {
        start_sample,
        samples: pcm.samples_f32,
        gains: if all_unity { Vec::new() } else { gains },
    })
}

/// Pre-mix every audio-bearing clip into one mono buffer at `rate`. Empty when the
/// timeline has no audio (→ caller uses the wall clock).
fn mix_timeline_mono(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    rate: u32,
) -> Vec<f32> {
    if timeline.fps <= 0 || rate == 0 {
        return Vec::new();
    }
    let mut clips_audio: Vec<ClipAudio> = Vec::new();
    for track in &timeline.tracks {
        if track.muted {
            continue;
        }
        for clip in &track.clips {
            if clip.media_type != ClipType::Audio && clip.media_type != ClipType::Video {
                continue;
            }
            if let Some(ca) = project_clip_audio(clip, media, timeline.fps, rate) {
                clips_audio.push(ca);
            }
        }
    }
    if clips_audio.is_empty() {
        return Vec::new();
    }
    mix::mix_clips(&clips_audio).unwrap_or_default()
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
    let rate = default_output_rate().unwrap_or(FALLBACK_SAMPLE_RATE);
    let mixed = mix_timeline_mono(timeline, media, rate);
    if mixed.is_empty() {
        return (Arc::new(InstantClock::new(start_frame)), None);
    }

    let buffer = Arc::new(mixed);
    let pos = Arc::new(AtomicU64::new(0));
    let clock = AudioClock {
        pos: pos.clone(),
        rate,
        fps,
    };
    clock.seek(start_frame); // begin playback at the current playhead

    match AudioPlayback::start(buffer, pos) {
        Ok(audio) => (Arc::new(clock), Some(audio)),
        Err(e) => {
            eprintln!("[audio] {e}; falling back to wall clock");
            (Arc::new(InstantClock::new(start_frame)), None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::Clip;

    #[test]
    fn audio_clock_frame_and_seek_round_trip() {
        let clock = AudioClock {
            pos: Arc::new(AtomicU64::new(0)),
            rate: 48_000,
            fps: 30,
        };
        // seek(30) → 30 frames = 1s = 48000 mono samples → frame()==30.
        clock.seek(30);
        assert_eq!(clock.pos.load(Ordering::Relaxed), 48_000);
        assert_eq!(clock.frame(30), 30);

        // Half a second of samples → frame 15.
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
        // 1599 samples @ 48k, 30fps = 0.999 frame → truncates to 0.
        clock.pos.store(1_599, Ordering::Relaxed);
        assert_eq!(clock.frame(30), 0);
        // 1600 samples = exactly one frame.
        clock.pos.store(1_600, Ordering::Relaxed);
        assert_eq!(clock.frame(30), 1);
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
    fn project_clip_audio_skips_clip_without_media_entry() {
        let clip = Clip::new("c1", "missing", 0, 30);
        let media: HashMap<String, MediaInfo> = HashMap::new();
        assert!(project_clip_audio(&clip, &media, 30, 48_000).is_none());
    }

    #[test]
    fn mix_timeline_mono_empty_when_no_audio_clips() {
        let timeline = Timeline::new();
        let media: HashMap<String, MediaInfo> = HashMap::new();
        assert!(mix_timeline_mono(&timeline, &media, 48_000).is_empty());
    }
}
