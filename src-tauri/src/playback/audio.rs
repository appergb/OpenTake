//! Audio master clock + cpal output for streaming playback (#63 / #160).
//!
//! The acceptance is "audio drives the playhead; video follows (dropping frames
//! to stay in sync)". [`try_build_clock`] realises that: when the timeline carries
//! sound it schedules bounded **interleaved stereo** windows at the cpal device
//! sample rate, plays them through a dedicated cpal output thread,
//! and exposes the device's frame position as [`AudioClock`] — the master clock
//! the render loop reads to pick its target video frame. A silent timeline falls
//! back to the wall-clock [`InstantClock`] PR1 ships.
//!
//! The cpal callback never blocks or allocates. A single producer decodes and
//! mixes fixed windows into a bounded channel; the callback uses only atomics
//! and non-blocking `try_recv`, emitting silence on underrun. Seek advances a
//! generation, cancels the old decode, and makes both producer and consumer
//! discard stale windows before audible output resumes.
//!
//! Stereo is mixed once and mapped to the device's channel count in the callback
//! (mono downmix / >2 zero-fill). The mixing math mirrors the proven export
//! mixdown (`export.rs`), parameterised by the device rate and done per channel.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use crossbeam_channel::{bounded, Receiver as ChunkReceiver, Sender as ChunkSender};

use opentake_domain::{AudioDenoise, Clip, ClipType, Timeline};
use opentake_media::{
    decode_pcm_interleaved_cancellable, encode::mix::apply_true_peak_ceiling, MediaCancelToken,
    MediaError, PcmFormat, PcmSpec,
};

use super::engine::{InstantClock, PlaybackClock};
use super::project::MediaInfo;

/// Default device sample rate when cpal can't report one (no device queried yet).
const FALLBACK_SAMPLE_RATE: u32 = 48_000;

/// The mix is always interleaved stereo; the callback maps it to the device's
/// channel count.
const MIX_CHANNELS: usize = 2;
const MIX_CANCEL_CHUNK_FRAMES: usize = 4 * 1024;
const STREAM_WINDOW_SECONDS: usize = 2;
const STREAM_WINDOW_CAPACITY: usize = 4;
const STREAM_SEND_POLL: Duration = Duration::from_millis(5);
const CALLBACK_START_TIMEOUT: Duration = Duration::from_secs(1);
const CALLBACK_POLL_INTERVAL: Duration = Duration::from_millis(5);
const AUDIO_CLOCK_STALL_TIMEOUT: Duration = Duration::from_millis(150);
const CALLBACKS_REQUIRED_FOR_LIVENESS: u64 = 2;
pub(super) const AUDIO_PREPARE_BUSY: &str = "audio_prepare_busy";

type AudioRateReply = SyncSender<Option<u32>>;

/// Serializes CPAL device discovery on one process-lifetime thread.
///
/// CPAL 0.15's WASAPI backend caches its `IMMDeviceEnumerator` process-wide,
/// while COM initialization is thread-local. Rust's test harness (and Tokio in
/// production) may invoke playback setup from successive short-lived threads;
/// allowing the thread which first created the enumerator to exit can leave the
/// cached COM object with no live originating apartment and the next query can
/// terminate the process with `STATUS_ACCESS_VIOLATION`. Keeping discovery on a
/// dedicated thread both preserves that COM lifetime and prevents concurrent
/// default-device queries from racing.
static AUDIO_RATE_PROBE: OnceLock<Option<SyncSender<AudioRateReply>>> = OnceLock::new();

struct AudioPrepareJob<T> {
    build: Box<dyn FnOnce() -> T + Send + 'static>,
    result: tokio::sync::oneshot::Sender<Result<T, String>>,
}

struct AudioPrepareOccupancy {
    occupied: AtomicBool,
    idle: tokio::sync::Notify,
}

impl AudioPrepareOccupancy {
    fn new() -> Self {
        Self {
            occupied: AtomicBool::new(false),
            idle: tokio::sync::Notify::new(),
        }
    }

    fn release(&self) {
        self.occupied.store(false, Ordering::Release);
        self.idle.notify_waiters();
    }
}

struct AudioPrepareOccupancyGuard(Arc<AudioPrepareOccupancy>);

impl Drop for AudioPrepareOccupancyGuard {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// One persistent blocking worker with exactly one admitted audio-prepare job.
pub struct AudioPrepareWorker<T: Send + 'static> {
    sender: SyncSender<AudioPrepareJob<T>>,
    occupancy: Arc<AudioPrepareOccupancy>,
}

/// Owning admission for the single audio-prepare worker slot. Dropping an
/// unsubmitted permit releases the reservation; after submit, the worker owns
/// it until the build closure has fully returned.
#[must_use]
pub struct AudioPreparePermit<T: Send + 'static> {
    sender: SyncSender<AudioPrepareJob<T>>,
    occupancy: Arc<AudioPrepareOccupancy>,
    reserved: bool,
}

impl<T: Send + 'static> AudioPreparePermit<T> {
    pub fn submit(
        mut self,
        build: impl FnOnce() -> T + Send + 'static,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<T, String>>, String> {
        let (result, receiver) = tokio::sync::oneshot::channel();
        let job = AudioPrepareJob {
            build: Box::new(build),
            result,
        };
        match self.sender.try_send(job) {
            Ok(()) => {
                self.reserved = false;
                Ok(receiver)
            }
            Err(TrySendError::Full(_)) => Err(AUDIO_PREPARE_BUSY.to_string()),
            Err(TrySendError::Disconnected(_)) => Err("audio_prepare_worker_stopped".to_string()),
        }
    }
}

impl<T: Send + 'static> Drop for AudioPreparePermit<T> {
    fn drop(&mut self) {
        if self.reserved {
            self.occupancy.release();
        }
    }
}

impl<T: Send + 'static> AudioPrepareWorker<T> {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel::<AudioPrepareJob<T>>(1);
        let occupancy = Arc::new(AudioPrepareOccupancy::new());
        let worker_occupancy = Arc::clone(&occupancy);
        let _ = thread::Builder::new()
            .name("opentake-audio-prepare".to_string())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let occupancy = AudioPrepareOccupancyGuard(Arc::clone(&worker_occupancy));
                    let value = catch_unwind(AssertUnwindSafe(job.build))
                        .map_err(|_| "audio_prepare_job_panicked".to_string());
                    drop(occupancy);
                    let _ = job.result.send(value);
                }
            });
        Self { sender, occupancy }
    }

    pub fn try_reserve(&self) -> Result<AudioPreparePermit<T>, String> {
        self.occupancy
            .occupied
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| AUDIO_PREPARE_BUSY.to_string())?;
        Ok(AudioPreparePermit {
            sender: self.sender.clone(),
            occupancy: Arc::clone(&self.occupancy),
            reserved: true,
        })
    }

    pub fn try_submit(
        &self,
        build: impl FnOnce() -> T + Send + 'static,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<T, String>>, String> {
        self.try_reserve()?.submit(build)
    }

    pub fn is_occupied(&self) -> bool {
        self.occupancy.occupied.load(Ordering::Acquire)
    }

    /// Wait for the admitted closure to return without polling a worker thread.
    pub async fn wait_until_idle(&self, timeout: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let idle = self.occupancy.idle.notified();
            if !self.is_occupied() {
                return true;
            }
            if tokio::time::timeout_at(deadline, idle).await.is_err() {
                return !self.is_occupied();
            }
        }
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

struct AudioStreamControl {
    generation: AtomicU64,
    requested_start: AtomicU64,
    stopped: AtomicBool,
    underruns: AtomicU64,
    active_decode: Mutex<Option<MediaCancelToken>>,
}

impl AudioStreamControl {
    fn new(start_frame: u64) -> Self {
        Self {
            generation: AtomicU64::new(0),
            requested_start: AtomicU64::new(start_frame),
            stopped: AtomicBool::new(false),
            underruns: AtomicU64::new(0),
            active_decode: Mutex::new(None),
        }
    }

    fn request_seek(&self, start_frame: u64) {
        self.requested_start.store(start_frame, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        if let Some(cancel) = self
            .active_decode
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
        {
            cancel.cancel();
        }
    }

    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(cancel) = self
            .active_decode
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
        {
            cancel.cancel();
        }
    }
}

#[derive(Debug)]
struct AudioStreamChunk {
    generation: u64,
    start_frame: u64,
    samples: Vec<f32>,
}

struct AudioStreamConsumer {
    receiver: ChunkReceiver<Result<AudioStreamChunk, MediaError>>,
    control: Arc<AudioStreamControl>,
    current: Option<AudioStreamChunk>,
}

impl AudioStreamConsumer {
    fn discard_stale(&mut self) {
        let generation = self.control.generation.load(Ordering::Acquire);
        if self
            .current
            .as_ref()
            .is_some_and(|chunk| chunk.generation != generation)
        {
            self.current = None;
        }
        if self.current.is_some() {
            return;
        }
        while let Ok(item) = self.receiver.try_recv() {
            match item {
                Ok(chunk) if chunk.generation == generation => {
                    self.current = Some(chunk);
                    break;
                }
                Ok(_) => {}
                Err(error) => {
                    eprintln!("[audio] streaming decode failed: {error}");
                }
            }
        }
    }

    fn sample_frame(&mut self, frame: u64) -> (f32, f32) {
        let generation = self.control.generation.load(Ordering::Acquire);
        if self.current.as_ref().is_none_or(|chunk| {
            let frames = chunk.samples.len() / MIX_CHANNELS;
            chunk.generation != generation
                || frame < chunk.start_frame
                || frame >= chunk.start_frame.saturating_add(frames as u64)
        }) {
            self.current = None;
            self.discard_stale();
        }
        if let Some(chunk) = self.current.as_ref() {
            let offset = frame.saturating_sub(chunk.start_frame) as usize * MIX_CHANNELS;
            if offset + 1 < chunk.samples.len() {
                return (chunk.samples[offset], chunk.samples[offset + 1]);
            }
        }
        self.control.underruns.fetch_add(1, Ordering::Relaxed);
        (0.0, 0.0)
    }
}

enum PlaybackSamples {
    Buffered(Arc<Vec<f32>>),
    Streaming(AudioStreamConsumer),
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
    stream: Option<Arc<AudioStreamControl>>,
    progress: Mutex<AudioClockProgress>,
}

struct AudioClockProgress {
    observed_pos: u64,
    observed_at: Instant,
    fallback: Option<(Instant, i32)>,
    last_frame: i32,
}

impl AudioClock {
    fn new(
        pos: Arc<AtomicU64>,
        rate: u32,
        fps: i32,
        stream: Option<Arc<AudioStreamControl>>,
    ) -> Self {
        let observed_pos = pos.load(Ordering::Acquire);
        let initial_frame = audio_position_frame(observed_pos, rate, fps);
        Self {
            pos,
            rate,
            fps,
            stream,
            progress: Mutex::new(AudioClockProgress {
                observed_pos,
                observed_at: Instant::now(),
                fallback: None,
                last_frame: initial_frame,
            }),
        }
    }
}

fn audio_position_frame(pos: u64, rate: u32, fps: i32) -> i32 {
    let fps = fps.max(1);
    ((pos as f64 / rate.max(1) as f64) * fps as f64) as i32
}

impl PlaybackClock for AudioClock {
    fn frame(&self, fps: i32) -> i32 {
        let fps = if fps > 0 { fps } else { self.fps.max(1) };
        let pos = self.pos.load(Ordering::Acquire);
        let audio_frame = audio_position_frame(pos, self.rate, fps);
        let now = Instant::now();
        let mut progress = self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if pos != progress.observed_pos {
            progress.observed_pos = pos;
            progress.observed_at = now;
        } else if progress.fallback.is_none()
            && now.saturating_duration_since(progress.observed_at) >= AUDIO_CLOCK_STALL_TIMEOUT
        {
            // The callback was proven live at startup/resume, but devices can be
            // interrupted later. Continue from the last monotonic frame on wall
            // time instead of rendering the same timeline frame forever.
            progress.fallback = Some((progress.observed_at, progress.last_frame.max(audio_frame)));
        }

        let candidate = if let Some((origin, base_frame)) = progress.fallback {
            let elapsed_frames =
                (now.saturating_duration_since(origin).as_secs_f64() * fps as f64) as i32;
            let wall_frame = base_frame.saturating_add(elapsed_frames.max(0));
            if audio_frame >= wall_frame {
                progress.fallback = None;
                audio_frame
            } else {
                wall_frame
            }
        } else {
            audio_frame
        };
        progress.last_frame = progress.last_frame.max(candidate);
        progress.last_frame
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
        let mut progress = self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *progress = AudioClockProgress {
            observed_pos: pos,
            observed_at: Instant::now(),
            fallback: None,
            last_frame: frame.max(0),
        };
        drop(progress);
        if let Some(stream) = &self.stream {
            stream.request_seek(pos);
        }
    }
}

/// Owns the cpal output thread for a playback session. The cpal `Stream` is
/// `!Send` on macOS, so it lives entirely on that thread; this handle drives a
/// cooperative stop. Dropping it stops audio and joins the thread.
pub struct AudioPlayback {
    control_tx: Sender<AudioCmd>,
    paused: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    stream_control: Option<Arc<AudioStreamControl>>,
    stream_producer: Option<JoinHandle<()>>,
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
        let callback_epoch = Arc::new(AtomicU64::new(0));
        let thread_callback_epoch = Arc::clone(&callback_epoch);
        let handle = thread::Builder::new()
            .name("opentake-audio".to_string())
            .spawn(move || {
                audio_thread(
                    PlaybackSamples::Buffered(buffer),
                    pos,
                    thread_paused,
                    thread_callback_epoch,
                    control_rx,
                    ready_tx,
                )
            })
            .map_err(|e| format!("spawn audio thread: {e}"))?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(AudioPlayback {
                control_tx,
                paused,
                handle: Some(handle),
                stream_control: None,
                stream_producer: None,
            }),
            Ok(Err(e)) => {
                let _ = handle.join();
                Err(e)
            }
            Err(_) => Err("audio thread exited before init".to_string()),
        }
    }

    fn start_stream(
        consumer: AudioStreamConsumer,
        stream_control: Arc<AudioStreamControl>,
        stream_producer: JoinHandle<()>,
        pos: Arc<AtomicU64>,
        paused: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let (control_tx, control_rx) = mpsc::channel::<AudioCmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let thread_paused = Arc::clone(&paused);
        let callback_epoch = Arc::new(AtomicU64::new(0));
        let thread_callback_epoch = Arc::clone(&callback_epoch);
        let handle = match thread::Builder::new()
            .name("opentake-audio".to_string())
            .spawn(move || {
                audio_thread(
                    PlaybackSamples::Streaming(consumer),
                    pos,
                    thread_paused,
                    thread_callback_epoch,
                    control_rx,
                    ready_tx,
                )
            }) {
            Ok(handle) => handle,
            Err(error) => {
                stream_control.stop();
                let _ = stream_producer.join();
                return Err(format!("spawn audio thread: {error}"));
            }
        };
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                control_tx,
                paused,
                handle: Some(handle),
                stream_control: Some(stream_control),
                stream_producer: Some(stream_producer),
            }),
            Ok(Err(error)) => {
                stream_control.stop();
                let _ = handle.join();
                let _ = stream_producer.join();
                Err(error)
            }
            Err(_) => {
                stream_control.stop();
                let _ = stream_producer.join();
                Err("audio thread exited before init".to_string())
            }
        }
    }

    pub fn pause(&self) -> Result<(), String> {
        self.paused.store(true, Ordering::Release);
        self.control(AudioCmd::Pause)
    }

    /// Prove that callbacks continue while output remains logically muted. The
    /// hardware stream stays running for the whole retained session, avoiding
    /// asynchronous backend play/pause acknowledgement races. The caller can
    /// then seek/resume the video clock before committing audible output.
    pub fn prepare_resume(&self) -> Result<(), String> {
        self.paused.store(true, Ordering::Release);
        self.control(AudioCmd::Resume)
    }

    /// Commit a successfully prepared resume after the render clock has been
    /// positioned. The already-running callback begins consuming at `pos` on
    /// its next block.
    pub fn commit_resume(&self) {
        self.paused.store(false, Ordering::Release);
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
        if let Some(control) = &self.stream_control {
            control.stop();
        }
        let _ = self.control_tx.send(AudioCmd::Stop);
        let audio = self.handle.take();
        let producer = self.stream_producer.take();
        match (audio, producer) {
            (Some(audio), Some(producer)) => {
                let joins = Arc::new(Mutex::new(Some((audio, producer))));
                let worker_joins = Arc::clone(&joins);
                match thread::Builder::new()
                    .name("opentake-audio-stop".to_string())
                    .spawn(move || {
                        if let Some((audio, producer)) = worker_joins
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .take()
                        {
                            let _ = audio.join();
                            let _ = producer.join();
                        }
                    }) {
                    Ok(handle) => Some(handle),
                    Err(_) => {
                        if let Some((audio, producer)) = joins
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .take()
                        {
                            let _ = audio.join();
                            let _ = producer.join();
                        }
                        None
                    }
                }
            }
            (Some(audio), None) => Some(audio),
            (None, Some(producer)) => Some(producer),
            (None, None) => None,
        }
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
                stream_control: None,
                stream_producer: None,
            },
            paused,
            stopped_rx,
        )
    }

    #[cfg(test)]
    pub(crate) fn test_failing_resume() -> (Self, Arc<AtomicBool>) {
        let (control_tx, control_rx) = mpsc::channel();
        let paused = Arc::new(AtomicBool::new(true));
        let handle = thread::spawn(move || {
            while let Ok(command) = control_rx.recv() {
                match command {
                    AudioCmd::Pause(reply) => {
                        let _ = reply.send(Ok(()));
                    }
                    AudioCmd::Resume(reply) => {
                        let _ = reply.send(Err("test audio callback unavailable".to_string()));
                    }
                    AudioCmd::Stop => break,
                }
            }
        });
        (
            Self {
                control_tx,
                paused: Arc::clone(&paused),
                handle: Some(handle),
                stream_control: None,
                stream_producer: None,
            },
            paused,
        )
    }

    #[cfg(test)]
    pub(crate) fn test_blocking_stop() -> (Self, Arc<AtomicBool>, Receiver<()>, Sender<()>) {
        let (control_tx, control_rx) = mpsc::channel();
        let (stopped_tx, stopped_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let paused = Arc::new(AtomicBool::new(false));
        let handle = thread::spawn(move || {
            while let Ok(command) = control_rx.recv() {
                match command {
                    AudioCmd::Pause(reply) | AudioCmd::Resume(reply) => {
                        let _ = reply.send(Ok(()));
                    }
                    AudioCmd::Stop => {
                        let _ = stopped_tx.send(());
                        let _ = release_rx.recv();
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
                stream_control: None,
                stream_producer: None,
            },
            paused,
            stopped_rx,
            release_tx,
        )
    }
}

impl Drop for AudioPlayback {
    fn drop(&mut self) {
        if let Some(control) = &self.stream_control {
            control.stop();
        }
        let _ = self.control_tx.send(AudioCmd::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.stream_producer.take() {
            let _ = handle.join();
        }
    }
}

/// The audio thread: build + play the output stream, report the result, then park
/// (holding the `!Send` stream alive) until a stop is requested.
fn audio_thread(
    samples: PlaybackSamples,
    pos: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    callback_epoch: Arc<AtomicU64>,
    control_rx: Receiver<AudioCmd>,
    ready_tx: Sender<Result<(), String>>,
) {
    match build_and_play(samples, &pos, &paused, &callback_epoch) {
        Ok(stream) => {
            let _ = ready_tx.send(Ok(()));
            while let Ok(command) = control_rx.recv() {
                match command {
                    AudioCmd::Pause(reply) => {
                        // Logical pause is established by `paused=true` before
                        // this barrier. Keep the hardware stream running muted
                        // so a later resume can prove current callback liveness
                        // without trusting an asynchronous backend play ack.
                        let _ = reply.send(Ok(()));
                    }
                    AudioCmd::Resume(reply) => {
                        let before = callback_epoch.load(Ordering::Acquire);
                        let result =
                            require_callback_after(&callback_epoch, before, CALLBACK_START_TIMEOUT);
                        let _ = reply.send(result);
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
    samples: PlaybackSamples,
    pos: &Arc<AtomicU64>,
    paused: &Arc<AtomicBool>,
    callback_epoch: &Arc<AtomicU64>,
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
        samples,
        pos.clone(),
        paused.clone(),
        Arc::clone(callback_epoch),
    )?;
    // A successful backend `play()` request does not guarantee that the output
    // callback is live. Installing an AudioClock before sustained callbacks can
    // freeze the playhead at frame zero forever. Prepared sessions start muted
    // for this handshake and keep the hardware stream running silently, so
    // resume never depends on an asynchronous backend play/pause transition.
    let before = callback_epoch.load(Ordering::Acquire);
    stream.play().map_err(|e| format!("stream play: {e}"))?;
    require_callback_after(callback_epoch, before, CALLBACK_START_TIMEOUT)?;
    Ok(stream)
}

fn require_callback_after(epoch: &AtomicU64, before: u64, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while epoch.load(Ordering::Acquire).wrapping_sub(before) < CALLBACKS_REQUIRED_FOR_LIVENESS {
        if Instant::now() >= deadline {
            return Err(format!(
                "audio output callback did not remain live within {} ms",
                timeout.as_millis()
            ));
        }
        thread::sleep(CALLBACK_POLL_INTERVAL.min(timeout));
    }
    Ok(())
}

/// Dispatch on the device sample format to the typed stream builder.
fn build_stream(
    format: cpal::SampleFormat,
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: PlaybackSamples,
    pos: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    callback_epoch: Arc<AtomicU64>,
) -> Result<cpal::Stream, String> {
    // Cover every fixed-size cpal format (all satisfy SizedSample + FromSample<f32>)
    // so a non-F32 default device (I32 is common on Linux/Windows) still gets audio
    // instead of silently falling back to the wall clock.
    match format {
        cpal::SampleFormat::F32 => {
            out_stream::<f32>(device, config, samples, pos, paused, callback_epoch)
        }
        cpal::SampleFormat::F64 => {
            out_stream::<f64>(device, config, samples, pos, paused, callback_epoch)
        }
        cpal::SampleFormat::I8 => {
            out_stream::<i8>(device, config, samples, pos, paused, callback_epoch)
        }
        cpal::SampleFormat::I16 => {
            out_stream::<i16>(device, config, samples, pos, paused, callback_epoch)
        }
        cpal::SampleFormat::I32 => {
            out_stream::<i32>(device, config, samples, pos, paused, callback_epoch)
        }
        cpal::SampleFormat::I64 => {
            out_stream::<i64>(device, config, samples, pos, paused, callback_epoch)
        }
        cpal::SampleFormat::U8 => {
            out_stream::<u8>(device, config, samples, pos, paused, callback_epoch)
        }
        cpal::SampleFormat::U16 => {
            out_stream::<u16>(device, config, samples, pos, paused, callback_epoch)
        }
        cpal::SampleFormat::U32 => {
            out_stream::<u32>(device, config, samples, pos, paused, callback_epoch)
        }
        cpal::SampleFormat::U64 => {
            out_stream::<u64>(device, config, samples, pos, paused, callback_epoch)
        }
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
    mut samples: PlaybackSamples,
    pos: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    callback_epoch: Arc<AtomicU64>,
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
                callback_epoch.fetch_add(1, Ordering::Release);
                let out_frames = data.len() / channels;
                // Atomically claim this block's start frame and advance the master
                // clock. A concurrent `seek` (store) is honored on the next
                // callback; within a block we play from the claimed start.
                if paused.load(Ordering::Acquire) {
                    if let PlaybackSamples::Streaming(consumer) = &mut samples {
                        consumer.discard_stale();
                    }
                    for sample in data.iter_mut() {
                        *sample = T::from_sample(0.0f32);
                    }
                    return;
                }
                let start = pos.fetch_add(out_frames as u64, Ordering::AcqRel);
                for (i, frame) in data.chunks_mut(channels).enumerate() {
                    let audio_frame = start.saturating_add(i as u64);
                    let (left, right) = match &mut samples {
                        PlaybackSamples::Buffered(buffer) => {
                            let base = usize::try_from(audio_frame)
                                .ok()
                                .and_then(|frame| frame.checked_mul(MIX_CHANNELS));
                            match base.filter(|base| base + 1 < buffer.len()) {
                                Some(base) => (buffer[base], buffer[base + 1]),
                                None => (0.0, 0.0),
                            }
                        }
                        PlaybackSamples::Streaming(consumer) => consumer.sample_frame(audio_frame),
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
    let probe = AUDIO_RATE_PROBE
        .get_or_init(|| {
            let (request_tx, request_rx) = mpsc::sync_channel::<AudioRateReply>(1);
            thread::Builder::new()
                .name("opentake-audio-device".to_string())
                .spawn(move || run_audio_rate_probe(request_rx, query_default_output_rate))
                .ok()
                .map(|_| request_tx)
        })
        .as_ref()?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    probe.send(reply_tx).ok()?;
    reply_rx.recv().ok().flatten()
}

fn run_audio_rate_probe(
    request_rx: Receiver<AudioRateReply>,
    mut query: impl FnMut() -> Option<u32>,
) {
    while let Ok(reply) = request_rx.recv() {
        let rate = catch_unwind(AssertUnwindSafe(&mut query)).unwrap_or(None);
        let _ = reply.send(rate);
    }
}

fn query_default_output_rate() -> Option<u32> {
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
    /// User true-peak ceiling. The mixer keeps the same codec reconstruction
    /// safety margin as export so native preview does not audition hotter peaks.
    true_peak_ceiling_dbtp: Option<f64>,
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
    let interleaved =
        apply_preview_denoise(&interleaved, MIX_CHANNELS, rate, clip.audio_denoise, cancel)?;
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
        true_peak_ceiling_dbtp: clip
            .loudness_normalization
            .map(|normalization| normalization.true_peak_ceiling_dbtp),
    }))
}

fn apply_preview_denoise(
    samples: &[f32],
    channels: usize,
    sample_rate: u32,
    config: Option<AudioDenoise>,
    cancel: &MediaCancelToken,
) -> Result<Vec<f32>, MediaError> {
    let Some(config) = config.filter(|config| config.preview_enabled) else {
        return Ok(samples.to_vec());
    };
    opentake_media::analysis::denoise_interleaved(
        samples,
        channels,
        sample_rate,
        config,
        cancel,
        None,
    )
    .map_err(|error| match error {
        opentake_media::analysis::DenoiseError::Cancelled => MediaError::Cancelled,
        other => MediaError::Decode(other.to_string()),
    })
}

/// Visit a placed stereo mix in bounded windows. Only one `window_frames`
/// scratch buffer is live at a time regardless of the total timeline extent.
fn mix_stereo_windows(
    clips: &[StereoClip],
    window_frames: usize,
    cancel: &MediaCancelToken,
    mut emit: impl FnMut(usize, &[f32]) -> Result<(), MediaError>,
) -> Result<(), MediaError> {
    if window_frames == 0 {
        return Err(MediaError::Decode(
            "audio mix window must contain at least one frame".to_string(),
        ));
    }
    let true_peak_ceiling_dbtp = clips
        .iter()
        .filter_map(|clip| clip.true_peak_ceiling_dbtp)
        .min_by(f64::total_cmp);
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
    for window_start in (0..total_frames).step_by(window_frames) {
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        let window_end = window_start.saturating_add(window_frames).min(total_frames);
        let window_samples = window_end
            .saturating_sub(window_start)
            .checked_mul(MIX_CHANNELS)
            .ok_or_else(|| audio_buffer_too_large("mix window sample count overflow"))?;
        let mut out = Vec::new();
        out.try_reserve_exact(window_samples).map_err(|error| {
            audio_allocation_failed(format!("mix window reserve {window_samples}: {error}"))
        })?;
        out.resize(window_samples, 0.0);
        for clip in clips {
            let clip_frames = clip.interleaved.len() / MIX_CHANNELS;
            let clip_end = clip.start_frame.saturating_add(clip_frames);
            let overlap_start = window_start.max(clip.start_frame);
            let overlap_end = window_end.min(clip_end);
            if overlap_start >= overlap_end {
                continue;
            }
            for chunk_start in (overlap_start..overlap_end).step_by(MIX_CANCEL_CHUNK_FRAMES) {
                if cancel.checkpoint() {
                    return Err(MediaError::Cancelled);
                }
                let chunk_end = chunk_start
                    .saturating_add(MIX_CANCEL_CHUNK_FRAMES)
                    .min(overlap_end);
                for timeline_frame in chunk_start..chunk_end {
                    let clip_frame = timeline_frame - clip.start_frame;
                    let gain = if clip.gains.is_empty() {
                        1.0
                    } else {
                        clip.gains[clip_frame]
                    };
                    let output = (timeline_frame - window_start) * MIX_CHANNELS;
                    let input = clip_frame * MIX_CHANNELS;
                    out[output] += clip.interleaved[input] * gain;
                    out[output + 1] += clip.interleaved[input + 1] * gain;
                }
            }
        }
        for value in &mut out {
            *value = value.clamp(-1.0, 1.0);
        }
        apply_true_peak_ceiling(&mut out, true_peak_ceiling_dbtp);
        emit(window_start, &out)?;
    }
    Ok(())
}

/// Sum placed stereo clips into one interleaved buffer, applying per-frame gains
/// and hard-limiting to [-1, 1] (mirrors the export mixdown, per channel).
fn mix_stereo(clips: &[StereoClip], cancel: &MediaCancelToken) -> Result<Vec<f32>, MediaError> {
    let mut out = Vec::new();
    mix_stereo_windows(
        clips,
        MIX_CANCEL_CHUNK_FRAMES,
        cancel,
        |_start_frame, samples| {
            out.try_reserve(samples.len()).map_err(|error| {
                audio_allocation_failed(format!("mix output reserve {}: {error}", samples.len()))
            })?;
            out.extend_from_slice(samples);
            Ok(())
        },
    )?;
    Ok(out)
}

fn retime_interleaved_stereo(samples: &[f32], target_frames: usize) -> Vec<f32> {
    let source_frames = samples.len() / MIX_CHANNELS;
    if source_frames == 0 || target_frames == 0 {
        return Vec::new();
    }
    if source_frames == target_frames {
        return samples[..source_frames * MIX_CHANNELS].to_vec();
    }
    let mut output = Vec::with_capacity(target_frames * MIX_CHANNELS);
    let source_span = source_frames.saturating_sub(1) as f64;
    let target_span = target_frames.saturating_sub(1).max(1) as f64;
    for frame in 0..target_frames {
        let source = if target_frames == 1 {
            0.0
        } else {
            frame as f64 * source_span / target_span
        };
        let lo = source.floor() as usize;
        let hi = source.ceil() as usize;
        let fraction = (source - lo as f64) as f32;
        for channel in 0..MIX_CHANNELS {
            let a = samples[lo * MIX_CHANNELS + channel];
            let b = samples[hi * MIX_CHANNELS + channel];
            output.push(a + (b - a) * fraction);
        }
    }
    output
}

fn timeline_audio_frames(timeline: &Timeline, rate: u32) -> Result<u64, MediaError> {
    if timeline.fps <= 0 || rate == 0 {
        return Ok(0);
    }
    let frames = timeline.total_frames().max(0) as u64;
    let numerator = frames
        .checked_mul(rate as u64)
        .ok_or_else(|| audio_buffer_too_large("streaming timeline frame extent overflow"))?;
    numerator
        .checked_add(timeline.fps as u64 / 2)
        .map(|rounded| rounded / timeline.fps as u64)
        .ok_or_else(|| audio_buffer_too_large("streaming timeline frame rounding overflow"))
}

fn mix_timeline_window(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    rate: u32,
    window_start: u64,
    window_frames: usize,
    cancel: &MediaCancelToken,
) -> Result<Vec<f32>, MediaError> {
    let sample_count = window_frames
        .checked_mul(MIX_CHANNELS)
        .ok_or_else(|| audio_buffer_too_large("streaming window sample count overflow"))?;
    let mut mixed = vec![0.0_f32; sample_count];
    let mut true_peak_ceiling_dbtp: Option<f64> = None;
    let window_end = window_start.saturating_add(window_frames as u64);
    for track in &timeline.tracks {
        if track.muted {
            continue;
        }
        for clip in &track.clips {
            if cancel.checkpoint() {
                return Err(MediaError::Cancelled);
            }
            if !matches!(clip.media_type, ClipType::Audio | ClipType::Video)
                || clip.duration_frames <= 0
            {
                continue;
            }
            let Some(info) = media.get(&clip.media_ref) else {
                continue;
            };
            let clip_start = ((clip.start_frame.max(0) as f64 / timeline.fps as f64) * rate as f64)
                .round() as u64;
            let clip_frames = ((clip.duration_frames as f64 / timeline.fps as f64) * rate as f64)
                .round()
                .max(0.0) as u64;
            let clip_end = clip_start.saturating_add(clip_frames);
            let overlap_start = window_start.max(clip_start);
            let overlap_end = window_end.min(clip_end);
            if overlap_start >= overlap_end || clip_frames == 0 {
                continue;
            }
            let Some((source_lo, source_hi)) = clip_source_window_secs(clip, timeline.fps) else {
                continue;
            };
            let source_span = source_hi - source_lo;
            let relative_start = (overlap_start - clip_start) as f64 / clip_frames as f64;
            let relative_end = (overlap_end - clip_start) as f64 / clip_frames as f64;
            let range = (
                source_lo + source_span * relative_start,
                source_lo + source_span * relative_end,
            );
            let spec = PcmSpec {
                sample_rate: rate,
                channels: MIX_CHANNELS as u16,
                format: PcmFormat::F32,
            };
            let decoded =
                match decode_pcm_interleaved_cancellable(&info.path, &spec, Some(range), cancel) {
                    Ok(decoded) => decoded,
                    Err(MediaError::NoTrack(_, _)) => continue,
                    Err(error) => return Err(error),
                };
            let target_frames = (overlap_end - overlap_start) as usize;
            let retimed = retime_interleaved_stereo(&decoded, target_frames);
            let retimed =
                apply_preview_denoise(&retimed, MIX_CHANNELS, rate, clip.audio_denoise, cancel)?;
            let frames_per_timeline_frame = rate as f64 / timeline.fps as f64;
            for frame in 0..target_frames.min(retimed.len() / MIX_CHANNELS) {
                let timeline_sample = overlap_start.saturating_add(frame as u64);
                let timeline_frame =
                    (timeline_sample as f64 / frames_per_timeline_frame).floor() as i32;
                let gain = clip.volume_at(timeline_frame) as f32;
                let output = (overlap_start - window_start) as usize + frame;
                let output = output * MIX_CHANNELS;
                mixed[output] += retimed[frame * MIX_CHANNELS] * gain;
                mixed[output + 1] += retimed[frame * MIX_CHANNELS + 1] * gain;
            }
            if let Some(ceiling) = clip
                .loudness_normalization
                .map(|normalization| normalization.true_peak_ceiling_dbtp)
            {
                true_peak_ceiling_dbtp =
                    Some(true_peak_ceiling_dbtp.map_or(ceiling, |current| current.min(ceiling)));
            }
        }
    }
    for sample in &mut mixed {
        *sample = sample.clamp(-1.0, 1.0);
    }
    apply_true_peak_ceiling(&mut mixed, true_peak_ceiling_dbtp);
    Ok(mixed)
}

struct PreparedTimelineAudio {
    consumer: AudioStreamConsumer,
    control: Arc<AudioStreamControl>,
    producer: JoinHandle<()>,
}

fn send_stream_chunk(
    sender: &ChunkSender<Result<AudioStreamChunk, MediaError>>,
    mut chunk: Result<AudioStreamChunk, MediaError>,
    control: &AudioStreamControl,
    generation: u64,
) -> bool {
    loop {
        if control.stopped.load(Ordering::Acquire)
            || control.generation.load(Ordering::Acquire) != generation
        {
            return false;
        }
        match sender.try_send(chunk) {
            Ok(()) => return true,
            Err(crossbeam_channel::TrySendError::Full(returned)) => {
                chunk = returned;
                thread::sleep(STREAM_SEND_POLL);
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => return false,
        }
    }
}

/// Prepare bounded timeline-audio scheduling at `rate`. The initial window is
/// mixed synchronously so decode failures surface before playback ownership is
/// published; all subsequent windows are produced on one bounded worker.
fn mix_timeline_stereo(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    rate: u32,
    start_frame: u64,
    cancel: &MediaCancelToken,
) -> Result<Option<PreparedTimelineAudio>, MediaError> {
    if timeline.fps <= 0 || rate == 0 {
        return Ok(None);
    }
    let has_candidates = timeline
        .tracks
        .iter()
        .filter(|track| !track.muted)
        .any(|track| {
            track.clips.iter().any(|clip| {
                matches!(clip.media_type, ClipType::Audio | ClipType::Video)
                    && clip.duration_frames > 0
                    && media.contains_key(&clip.media_ref)
            })
        });
    if !has_candidates {
        return Ok(None);
    }
    let total_frames = timeline_audio_frames(timeline, rate)?;
    if start_frame >= total_frames {
        return Ok(None);
    }
    let window_frames = (rate as usize)
        .checked_mul(STREAM_WINDOW_SECONDS)
        .ok_or_else(|| audio_buffer_too_large("streaming window frame overflow"))?;
    let first_len = (total_frames - start_frame).min(window_frames as u64) as usize;
    let first_samples = mix_timeline_window(timeline, media, rate, start_frame, first_len, cancel)?;
    let (sender, receiver) = bounded(STREAM_WINDOW_CAPACITY);
    sender
        .send(Ok(AudioStreamChunk {
            generation: 0,
            start_frame,
            samples: first_samples,
        }))
        .map_err(|_| MediaError::Decode("audio stream queue closed during prefill".to_string()))?;
    let control = Arc::new(AudioStreamControl::new(start_frame));
    let producer_control = Arc::clone(&control);
    let producer_timeline = timeline.clone();
    let producer_media = media.clone();
    let producer = thread::Builder::new()
        .name("opentake-audio-fill".to_string())
        .spawn(move || {
            let mut generation = 0_u64;
            let mut next_frame = start_frame.saturating_add(first_len as u64);
            loop {
                if producer_control.stopped.load(Ordering::Acquire) {
                    break;
                }
                let observed = producer_control.generation.load(Ordering::Acquire);
                if observed != generation {
                    generation = observed;
                    next_frame = producer_control.requested_start.load(Ordering::Acquire);
                }
                if next_frame >= total_frames {
                    thread::sleep(STREAM_SEND_POLL);
                    continue;
                }
                let len = (total_frames - next_frame).min(window_frames as u64) as usize;
                let window_cancel = MediaCancelToken::new();
                *producer_control
                    .active_decode
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(window_cancel.clone());
                if producer_control.generation.load(Ordering::Acquire) != generation {
                    window_cancel.cancel();
                    producer_control
                        .active_decode
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .take();
                    continue;
                }
                let result = mix_timeline_window(
                    &producer_timeline,
                    &producer_media,
                    rate,
                    next_frame,
                    len,
                    &window_cancel,
                );
                producer_control
                    .active_decode
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take();
                if producer_control.generation.load(Ordering::Acquire) != generation {
                    continue;
                }
                match result {
                    Ok(samples) => {
                        if !send_stream_chunk(
                            &sender,
                            Ok(AudioStreamChunk {
                                generation,
                                start_frame: next_frame,
                                samples,
                            }),
                            &producer_control,
                            generation,
                        ) {
                            continue;
                        }
                        next_frame = next_frame.saturating_add(len as u64);
                    }
                    Err(MediaError::Cancelled) => continue,
                    Err(error) => {
                        let _ =
                            send_stream_chunk(&sender, Err(error), &producer_control, generation);
                        break;
                    }
                }
            }
        })
        .map_err(|error| MediaError::Decode(format!("spawn audio fill worker: {error}")))?;
    Ok(Some(PreparedTimelineAudio {
        consumer: AudioStreamConsumer {
            receiver,
            control: Arc::clone(&control),
            current: None,
        },
        control,
        producer,
    }))
}

/// Production-facing clock construction with explicit media/allocation errors.
pub fn try_build_clock(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    fps: i32,
    start_frame: i32,
) -> Result<(Arc<dyn PlaybackClock>, Option<AudioPlayback>), MediaError> {
    build_clock_with_state(
        timeline,
        media,
        fps,
        start_frame,
        false,
        &MediaCancelToken::new(),
    )
}

/// Prepare audio without starting the device clock. The retained playback
/// session resumes audio only after its first composited frame is buffered.
pub fn build_clock_paused(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    fps: i32,
    start_frame: i32,
) -> Result<(Arc<dyn PlaybackClock>, Option<AudioPlayback>), MediaError> {
    build_clock_paused_cancellable(timeline, media, fps, start_frame, &MediaCancelToken::new())
}

pub fn build_clock_paused_cancellable(
    timeline: &Timeline,
    media: &HashMap<String, MediaInfo>,
    fps: i32,
    start_frame: i32,
    cancel: &MediaCancelToken,
) -> Result<(Arc<dyn PlaybackClock>, Option<AudioPlayback>), MediaError> {
    build_clock_with_state(timeline, media, fps, start_frame, true, cancel)
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
    let start_audio_frame =
        ((start_frame.max(0) as f64 / fps.max(1) as f64) * rate as f64).round() as u64;
    let Some(prepared) = mix_timeline_stereo(timeline, media, rate, start_audio_frame, cancel)?
    else {
        return Ok((Arc::new(InstantClock::new(start_frame)), None));
    };
    let pos = Arc::new(AtomicU64::new(start_audio_frame));
    let paused = Arc::new(AtomicBool::new(start_paused));
    let clock = AudioClock::new(
        Arc::clone(&pos),
        rate,
        fps,
        Some(Arc::clone(&prepared.control)),
    );
    match AudioPlayback::start_stream(
        prepared.consumer,
        prepared.control,
        prepared.producer,
        pos,
        paused,
    ) {
        Ok(audio) => Ok((Arc::new(clock), Some(audio))),
        Err(error) => {
            eprintln!("[audio] {error}; falling back to wall clock");
            Ok((Arc::new(InstantClock::new(start_frame)), None))
        }
    }
}

fn clock_from_mixed<F>(
    mixed: Vec<f32>,
    rate: u32,
    fps: i32,
    start_frame: i32,
    start_paused: bool,
    start: F,
) -> (Arc<dyn PlaybackClock>, Option<AudioPlayback>)
where
    F: FnOnce(Arc<Vec<f32>>, Arc<AtomicU64>, Arc<AtomicBool>) -> Result<AudioPlayback, String>,
{
    let buffer = Arc::new(mixed);
    let pos = Arc::new(AtomicU64::new(0));
    let paused = Arc::new(AtomicBool::new(start_paused));
    let clock = AudioClock::new(pos.clone(), rate, fps, None);
    clock.seek(start_frame); // begin playback at the current playhead

    match start(buffer, pos, paused) {
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
            (
                "m1".to_string(),
                MediaInfo {
                    path: first,
                    straight_alpha: false,
                },
            ),
            (
                "m2".to_string(),
                MediaInfo {
                    path: second,
                    straight_alpha: false,
                },
            ),
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
                    0,
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
        let error = match done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("cancelled audio prepare must return")
        {
            Err(error) => error,
            Ok(_) => panic!("cancelled audio prepare must fail"),
        };
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
            true_peak_ceiling_dbtp: None,
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
    fn long_timeline_mix_has_constant_peak_allocation_and_matches_short_reference() {
        let near = StereoClip {
            start_frame: 0,
            interleaved: vec![0.6, -0.6, 0.5, 0.5],
            gains: Vec::new(),
            true_peak_ceiling_dbtp: None,
        };
        let far = StereoClip {
            start_frame: 48_000 * 60 * 60,
            interleaved: vec![0.25, -0.25],
            gains: Vec::new(),
            true_peak_ceiling_dbtp: None,
        };
        let reference = mix_stereo(
            &[StereoClip {
                start_frame: near.start_frame,
                interleaved: near.interleaved.clone(),
                gains: near.gains.clone(),
                true_peak_ceiling_dbtp: near.true_peak_ceiling_dbtp,
            }],
            &MediaCancelToken::new(),
        )
        .unwrap();
        let mut first_window = Vec::new();
        let mut peak_samples = 0;

        mix_stereo_windows(
            &[near, far],
            1024,
            &MediaCancelToken::new(),
            |start_frame, samples| {
                peak_samples = peak_samples.max(samples.len());
                if start_frame == 0 {
                    first_window.extend_from_slice(samples);
                }
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(&first_window[..reference.len()], reference);
        assert!(
            peak_samples <= 1024 * MIX_CHANNELS,
            "one-hour timeline must retain only one bounded mix window"
        );
    }

    #[test]
    fn streaming_consumer_discards_pre_seek_chunks_and_reports_underrun_as_silence() {
        let control = Arc::new(AudioStreamControl::new(0));
        let (sender, receiver) = bounded(4);
        sender
            .send(Ok(AudioStreamChunk {
                generation: 0,
                start_frame: 0,
                samples: vec![0.25, -0.25],
            }))
            .unwrap();
        let mut consumer = AudioStreamConsumer {
            receiver,
            control: Arc::clone(&control),
            current: None,
        };
        assert_eq!(consumer.sample_frame(0), (0.25, -0.25));

        control.request_seek(10);
        sender
            .send(Ok(AudioStreamChunk {
                generation: 0,
                start_frame: 1,
                samples: vec![0.5, 0.5],
            }))
            .unwrap();
        sender
            .send(Ok(AudioStreamChunk {
                generation: 1,
                start_frame: 10,
                samples: vec![0.75, -0.75],
            }))
            .unwrap();
        assert_eq!(consumer.sample_frame(10), (0.75, -0.75));
        assert_eq!(consumer.sample_frame(99), (0.0, 0.0));
        assert_eq!(control.underruns.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn paused_stream_drain_retains_the_next_current_generation_chunk() {
        let control = Arc::new(AudioStreamControl::new(0));
        let (sender, receiver) = bounded(4);
        for (start_frame, sample) in [(0, 0.25), (1, 0.75)] {
            sender
                .send(Ok(AudioStreamChunk {
                    generation: 0,
                    start_frame,
                    samples: vec![sample, -sample],
                }))
                .unwrap();
        }
        let mut consumer = AudioStreamConsumer {
            receiver,
            control,
            current: None,
        };

        consumer.discard_stale();
        consumer.discard_stale();

        assert_eq!(consumer.sample_frame(0), (0.25, -0.25));
        assert_eq!(consumer.sample_frame(1), (0.75, -0.75));
    }

    #[test]
    fn audio_decode_failure_is_not_silently_treated_as_silent_timeline() {
        let timeline = audio_timeline(vec![audio_clip("broken", "missing", 0, 30)]);
        let media = HashMap::from([(
            "missing".to_string(),
            MediaInfo {
                path: PathBuf::from("/definitely/missing/audio.wav"),
                straight_alpha: false,
            },
        )]);

        let error =
            match mix_timeline_stereo(&timeline, &media, 48_000, 0, &MediaCancelToken::new()) {
                Err(error) => error,
                Ok(_) => panic!("decode failure must propagate instead of producing an empty mix"),
            };

        assert!(!matches!(error, MediaError::Cancelled));
    }

    #[test]
    fn try_build_clock_propagates_missing_audio_error_without_panic() {
        let timeline = audio_timeline(vec![audio_clip("broken", "missing", 0, 30)]);
        let media = HashMap::from([(
            "missing".to_string(),
            MediaInfo {
                path: PathBuf::from("/definitely/missing/try-build-clock.wav"),
                straight_alpha: false,
            },
        )]);

        let error = match try_build_clock(&timeline, &media, 30, 0) {
            Err(error) => error,
            Ok(_) => panic!("production clock entry must propagate media errors"),
        };

        assert!(!matches!(error, MediaError::Cancelled));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn default_output_rate_survives_sequential_short_lived_callers() {
        for _ in 0..8 {
            std::thread::spawn(default_output_rate)
                .join()
                .expect("WASAPI rate probing must survive caller thread teardown");
        }
    }

    #[test]
    fn audio_rate_probe_survives_a_query_panic() {
        let (request_tx, request_rx) = mpsc::sync_channel::<AudioRateReply>(1);
        let mut attempts = 0;
        let worker = std::thread::spawn(move || {
            run_audio_rate_probe(request_rx, move || {
                attempts += 1;
                if attempts == 1 {
                    panic!("simulated CPAL query panic");
                }
                Some(44_100)
            });
        });

        let request = |sender: &SyncSender<AudioRateReply>| {
            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
            sender.send(reply_tx).expect("submit rate query");
            reply_rx.recv().expect("receive rate query result")
        };
        assert_eq!(request(&request_tx), None);
        assert_eq!(request(&request_tx), Some(44_100));

        drop(request_tx);
        worker.join().expect("join audio rate probe");
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
        assert_eq!(
            first
                .blocking_recv()
                .expect("first result channel")
                .expect("first job succeeds"),
            1
        );
        assert_eq!(max_active.load(Ordering::Acquire), 1);
    }

    #[test]
    fn panicking_prepare_reports_error_releases_capacity_and_worker_survives() {
        let worker = AudioPrepareWorker::<usize>::new();
        let first = worker
            .try_reserve()
            .expect("reserve first prepare")
            .submit(|| panic!("deterministic prepare panic"))
            .expect("submit first prepare");

        let error = first
            .blocking_recv()
            .expect("worker must publish a panic result")
            .expect_err("panicking closure must be an explicit job error");
        assert_eq!(error, "audio_prepare_job_panicked");
        assert!(!worker.is_occupied());

        let second = worker
            .try_reserve()
            .expect("capacity recovered after unwind")
            .submit(|| 7)
            .expect("persistent worker accepts next job");
        assert_eq!(
            second
                .blocking_recv()
                .expect("second result channel")
                .expect("second job succeeds"),
            7
        );
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
        first
            .blocking_recv()
            .expect("cancelled worker result channel")
            .expect("cancelled worker exits");
        let deadline = Instant::now() + Duration::from_secs(2);
        while worker.is_occupied() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(!worker.is_occupied());
        worker
            .try_submit(|| ())
            .expect("capacity releases only after exit")
            .blocking_recv()
            .expect("replacement result channel")
            .expect("replacement prepare completes");
    }

    #[test]
    fn audio_clock_frame_and_seek_round_trip() {
        let clock = AudioClock::new(Arc::new(AtomicU64::new(0)), 48_000, 30, None);
        // seek(30) → 30 frames = 1s = 48000 output frames → frame()==30.
        clock.seek(30);
        assert_eq!(clock.pos.load(Ordering::Relaxed), 48_000);
        assert_eq!(clock.frame(30), 30);

        // Half a second of frames → frame 15.
        let half_second = AudioClock::new(Arc::new(AtomicU64::new(24_000)), 48_000, 30, None);
        assert_eq!(half_second.frame(30), 15);
    }

    #[test]
    fn audio_clock_falls_back_to_wall_time_when_callbacks_stall_mid_playback() {
        let clock = AudioClock::new(Arc::new(AtomicU64::new(0)), 48_000, 100, None);

        assert_eq!(clock.frame(100), 0);
        std::thread::sleep(AUDIO_CLOCK_STALL_TIMEOUT + Duration::from_millis(20));

        assert!(
            clock.frame(100) >= 1,
            "a dead audio callback must not freeze the timeline forever"
        );
    }

    #[test]
    fn recovered_audio_callbacks_and_explicit_seeks_never_rewind_accidentally() {
        let pos = Arc::new(AtomicU64::new(0));
        let clock = AudioClock::new(Arc::clone(&pos), 48_000, 100, None);
        let _ = clock.frame(100);
        std::thread::sleep(AUDIO_CLOCK_STALL_TIMEOUT + Duration::from_millis(20));
        let fallback_frame = clock.frame(100);
        assert!(fallback_frame >= 1);

        // A recovering device may initially report a position behind the wall
        // fallback. It must catch up without pulling the timeline backwards.
        pos.store(4_800, Ordering::Release);
        assert!(clock.frame(100) >= fallback_frame);
        pos.store(48_000, Ordering::Release);
        assert!(clock.frame(100) >= 100);

        // A user/transport seek is authoritative and intentionally may move back.
        clock.seek(7);
        assert_eq!(clock.frame(100), 7);
    }

    #[test]
    fn callback_liveness_detects_epoch_advance() {
        let epoch = Arc::new(AtomicU64::new(4));
        let worker_epoch = Arc::clone(&epoch);
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            worker_epoch.fetch_add(1, Ordering::Release);
            std::thread::sleep(Duration::from_millis(10));
            worker_epoch.fetch_add(1, Ordering::Release);
        });

        require_callback_after(&epoch, 4, Duration::from_millis(250))
            .expect("callback epoch should advance");
        worker.join().expect("callback worker");
    }

    #[test]
    fn callback_liveness_times_out_without_callback() {
        let epoch = AtomicU64::new(0);
        let error = require_callback_after(&epoch, 0, Duration::from_millis(15))
            .expect_err("missing callback must fail readiness");
        assert!(error.contains("did not remain live"));
    }

    #[test]
    fn callback_liveness_rejects_one_trailing_callback() {
        let epoch = Arc::new(AtomicU64::new(7));
        let trailing_epoch = Arc::clone(&epoch);
        let trailing = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            trailing_epoch.fetch_add(1, Ordering::Release);
        });

        require_callback_after(&epoch, 7, Duration::from_millis(25))
            .expect_err("one trailing callback must not prove resumed liveness");
        trailing.join().expect("trailing callback worker");
    }

    #[test]
    fn failed_audio_start_installs_advancing_wall_clock() {
        let (clock, audio) = clock_from_mixed(
            vec![0.0, 0.0],
            48_000,
            100,
            0,
            false,
            |_buffer, _pos, _paused| Err("callback readiness timeout".to_string()),
        );

        assert!(audio.is_none());
        std::thread::sleep(Duration::from_millis(25));
        assert!(
            clock.frame(100) >= 1,
            "fallback clock must keep playback live"
        );
    }

    #[test]
    fn successful_audio_start_retains_device_clock() {
        let (clock, audio) = clock_from_mixed(
            vec![0.0, 0.0],
            48_000,
            30,
            0,
            false,
            |_buffer, pos, _paused| {
                pos.store(48_000, Ordering::Release);
                Ok(AudioPlayback::test_stub().0)
            },
        );

        assert!(audio.is_some());
        assert_eq!(clock.frame(30), 30);
    }

    #[test]
    fn audio_clock_truncates_partial_frames() {
        let clock = AudioClock::new(Arc::new(AtomicU64::new(0)), 48_000, 30, None);
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
        let clock = AudioClock::new(Arc::new(AtomicU64::new(0)), 44_100, 24, None);
        for f in [1, 7, 23, 100, 511] {
            clock.seek(f);
            assert_eq!(clock.frame(24), f, "seek({f}) must round-trip");
        }
    }

    #[test]
    fn streaming_timeline_extent_rounds_like_the_audio_clock_seek() {
        let mut timeline = Timeline::new();
        timeline.fps = 24;
        let mut track = Track::new("v1", ClipType::Video);
        track.clips.push(audio_clip("odd", "source", 0, 7));
        timeline.tracks.push(track);

        assert_eq!(timeline_audio_frames(&timeline, 44_100).unwrap(), 12_863);
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
            mix_timeline_stereo(&timeline, &media, 48_000, 0, &MediaCancelToken::new())
                .expect("empty timeline")
                .is_none()
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
            true_peak_ceiling_dbtp: None,
        };
        let b = StereoClip {
            start_frame: 1,
            interleaved: vec![0.6, 0.6],
            gains: Vec::new(),
            true_peak_ceiling_dbtp: None,
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
            true_peak_ceiling_dbtp: None,
        };
        let out = mix_stereo(&[c], &MediaCancelToken::new()).expect("mix");
        assert_eq!(out, vec![0.5, 0.5, 0.25, 0.25]);
    }

    #[test]
    fn mix_stereo_enforces_normalized_true_peak_with_codec_margin() {
        let clip = StereoClip {
            start_frame: 0,
            interleaved: vec![1.0, -1.0],
            gains: Vec::new(),
            true_peak_ceiling_dbtp: Some(-1.0),
        };
        let out = mix_stereo(&[clip], &MediaCancelToken::new()).expect("mix");
        let expected = 10.0_f32.powf(-3.0 / 20.0);
        assert!((out[0] - expected).abs() < 1e-6);
        assert!((out[1] + expected).abs() < 1e-6);
    }

    #[test]
    fn denoise_preview_uses_shared_processing_owner() {
        let config = opentake_domain::AudioDenoise {
            mode: opentake_domain::DenoiseMode::Adaptive,
            strength: 0.75,
            preview_enabled: true,
        };
        let input = vec![0.2, -0.1, 0.15, -0.05, 0.1, 0.0, 0.05, 0.05];
        let cancel = MediaCancelToken::new();
        let preview = apply_preview_denoise(&input, 2, 48_000, Some(config), &cancel)
            .expect("preview denoise");
        let shared =
            opentake_media::analysis::denoise_interleaved(&input, 2, 48_000, config, &cancel, None)
                .expect("shared denoise");
        assert_eq!(preview, shared);
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
