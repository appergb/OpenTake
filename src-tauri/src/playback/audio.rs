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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

use opentake_domain::{Clip, ClipType, Timeline};
use opentake_media::{decode_pcm_interleaved, PcmFormat, PcmSpec};

use super::engine::{InstantClock, PlaybackClock};
use super::project::MediaInfo;

/// Default device sample rate when cpal can't report one (no device queried yet).
const FALLBACK_SAMPLE_RATE: u32 = 48_000;

/// The mix is always interleaved stereo; the callback maps it to the device's
/// channel count.
const MIX_CHANNELS: usize = 2;

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
        // Match frame()'s float path so a seek round-trips exactly even when the
        // device rate isn't a multiple of fps (e.g. 44100 Hz @ 24 fps).
        let pos = (frame.max(0) as f64 / fps as f64 * self.rate as f64) as u64;
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
    /// Start playing `buffer` (interleaved stereo, at the device rate) from `pos`.
    /// Returns `Err` if the device/stream can't be set up (caller falls back to
    /// the wall clock). Blocks until the stream is built so failures surface
    /// synchronously.
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
    // Cover every fixed-size cpal format (all satisfy SizedSample + FromSample<f32>)
    // so a non-F32 default device (I32 is common on Linux/Windows) still gets audio
    // instead of silently falling back to the wall clock.
    match format {
        cpal::SampleFormat::F32 => out_stream::<f32>(device, config, buffer, pos),
        cpal::SampleFormat::F64 => out_stream::<f64>(device, config, buffer, pos),
        cpal::SampleFormat::I8 => out_stream::<i8>(device, config, buffer, pos),
        cpal::SampleFormat::I16 => out_stream::<i16>(device, config, buffer, pos),
        cpal::SampleFormat::I32 => out_stream::<i32>(device, config, buffer, pos),
        cpal::SampleFormat::I64 => out_stream::<i64>(device, config, buffer, pos),
        cpal::SampleFormat::U8 => out_stream::<u8>(device, config, buffer, pos),
        cpal::SampleFormat::U16 => out_stream::<u16>(device, config, buffer, pos),
        cpal::SampleFormat::U32 => out_stream::<u32>(device, config, buffer, pos),
        cpal::SampleFormat::U64 => out_stream::<u64>(device, config, buffer, pos),
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
                let start = pos.fetch_add(out_frames as u64, Ordering::Relaxed) as usize;
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
) -> Option<StereoClip> {
    if clip.duration_frames <= 0 || timeline_fps <= 0 || rate == 0 {
        return None;
    }
    let info = media.get(&clip.media_ref)?;
    let (lo, hi) = clip_source_window_secs(clip, timeline_fps)?;

    let spec = PcmSpec {
        sample_rate: rate,
        channels: MIX_CHANNELS as u16,
        format: PcmFormat::F32,
    };
    let interleaved = decode_pcm_interleaved(&info.path, &spec, Some((lo, hi))).ok()?;
    let frames = interleaved.len() / MIX_CHANNELS;
    if frames == 0 {
        return None;
    }

    let start_frame =
        ((clip.start_frame.max(0) as f64) / timeline_fps as f64 * rate as f64).round() as usize;
    let frames_per_tl_frame = rate as f64 / timeline_fps as f64;
    let mut gains = Vec::with_capacity(frames);
    let mut all_unity = true;
    for k in 0..frames {
        let tl_frame = clip.start_frame + (k as f64 / frames_per_tl_frame).floor() as i32;
        let g = clip.volume_at(tl_frame) as f32;
        if (g - 1.0).abs() > f32::EPSILON {
            all_unity = false;
        }
        gains.push(g);
    }

    Some(StereoClip {
        start_frame,
        interleaved,
        gains: if all_unity { Vec::new() } else { gains },
    })
}

/// Sum placed stereo clips into one interleaved buffer, applying per-frame gains
/// and hard-limiting to [-1, 1] (mirrors the export mixdown, per channel).
fn mix_stereo(clips: &[StereoClip]) -> Vec<f32> {
    let total_frames = clips
        .iter()
        .map(|c| c.start_frame + c.interleaved.len() / MIX_CHANNELS)
        .max()
        .unwrap_or(0);
    let mut out = vec![0.0f32; total_frames * MIX_CHANNELS];
    for c in clips {
        let frames = c.interleaved.len() / MIX_CHANNELS;
        for k in 0..frames {
            let g = if c.gains.is_empty() { 1.0 } else { c.gains[k] };
            let o = (c.start_frame + k) * MIX_CHANNELS;
            out[o] += c.interleaved[k * MIX_CHANNELS] * g;
            out[o + 1] += c.interleaved[k * MIX_CHANNELS + 1] * g;
        }
    }
    for v in &mut out {
        *v = v.clamp(-1.0, 1.0);
    }
    out
}

/// Pre-mix every audio-bearing clip into one interleaved stereo buffer at `rate`.
/// Empty when the timeline has no audio (→ caller uses the wall clock).
fn mix_timeline_stereo(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    rate: u32,
) -> Vec<f32> {
    if timeline.fps <= 0 || rate == 0 {
        return Vec::new();
    }
    let mut clips: Vec<StereoClip> = Vec::new();
    for track in &timeline.tracks {
        if track.muted {
            continue;
        }
        for clip in &track.clips {
            if clip.media_type != ClipType::Audio && clip.media_type != ClipType::Video {
                continue;
            }
            if let Some(sc) = project_clip_audio_stereo(clip, media, timeline.fps, rate) {
                clips.push(sc);
            }
        }
    }
    if clips.is_empty() {
        return Vec::new();
    }
    mix_stereo(&clips)
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
    let mixed = mix_timeline_stereo(timeline, media, rate);
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
        assert!(project_clip_audio_stereo(&clip, &media, 30, 48_000).is_none());
    }

    #[test]
    fn mix_timeline_stereo_empty_when_no_audio_clips() {
        let timeline = Timeline::new();
        let media: HashMap<String, MediaInfo> = HashMap::new();
        assert!(mix_timeline_stereo(&timeline, &media, 48_000).is_empty());
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
        let out = mix_stereo(&[a, b]);
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
        let out = mix_stereo(&[c]);
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
