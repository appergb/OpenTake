//! The playback render loop + its dedicated thread (#53).
//!
//! A single thread owns a wgpu device and drives the whole "read clock → build
//! the frame plan → pull/decode each clip's frame → composite → hand the frame
//! to a sink → broadcast the playhead" cycle. Keeping it on one thread is a hard
//! requirement: the compositor's textures are `Rc` (not `Send`), and wgpu's
//! device/queue must be touched from one thread. The thread creates its **own**
//! [`RenderDevice`] and never touches the preview's `RenderState`, so playback and
//! the paused-frame `composite_frame` path never contend.
//!
//! The clock, frame sink, and playhead emitter are traits so the loop logic is
//! decoupled from cpal / MJPEG / Tauri: PR1 ships an [`InstantClock`] and lets a
//! gated integration test supply in-memory sink/emitter; PR2 swaps in the cpal
//! master clock, the MJPEG sink, and the Tauri event emitter without touching the
//! loop.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use opentake_domain::Timeline;
use opentake_media::MediaCancelToken;
use opentake_render::{
    try_build_render_plan, Compositor, DecodedFrame, RenderDevice, RenderPlan, RenderSize,
};

use super::project::{ManifestMetrics, MediaInfo, TextInfo};
use super::resolver::{PlaybackResolverState, StreamingResolver};

const REAPER_CAPACITY: usize = 2;

struct ReapJob(Vec<JoinHandle<()>>);

struct ReaperInner {
    sender: mpsc::SyncSender<ReapJob>,
    outstanding: Arc<AtomicUsize>,
}

/// One persistent join worker with at most two outstanding teardown jobs.
#[derive(Clone)]
pub struct BoundedReaper {
    inner: Arc<ReaperInner>,
}

pub struct ReapPermit {
    reaper: BoundedReaper,
    active: bool,
}

impl BoundedReaper {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel::<ReapJob>(REAPER_CAPACITY);
        let inner = Arc::new(ReaperInner {
            sender,
            outstanding: Arc::new(AtomicUsize::new(0)),
        });
        let worker_outstanding = Arc::clone(&inner.outstanding);
        let _ = thread::Builder::new()
            .name("opentake-playback-reaper".to_string())
            .spawn(move || {
                while let Ok(ReapJob(handles)) = receiver.recv() {
                    for handle in handles {
                        let _ = handle.join();
                    }
                    worker_outstanding.fetch_sub(1, Ordering::AcqRel);
                }
            });
        Self { inner }
    }

    pub fn try_reserve(&self) -> Result<ReapPermit, String> {
        self.inner
            .outstanding
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |outstanding| {
                (outstanding < REAPER_CAPACITY).then_some(outstanding + 1)
            })
            .map_err(|_| "playback_teardown_busy".to_string())?;
        Ok(ReapPermit {
            reaper: self.clone(),
            active: true,
        })
    }

    #[cfg(test)]
    pub(crate) fn wait_until_idle(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while self.inner.outstanding.load(Ordering::Acquire) != 0 && Instant::now() < deadline {
            thread::yield_now();
        }
        assert_eq!(self.inner.outstanding.load(Ordering::Acquire), 0);
    }

    #[cfg(test)]
    pub(crate) fn outstanding_count(&self) -> usize {
        self.inner.outstanding.load(Ordering::Acquire)
    }
}

impl Default for BoundedReaper {
    fn default() -> Self {
        Self::new()
    }
}

impl ReapPermit {
    pub fn enqueue(mut self, handles: Vec<JoinHandle<()>>) -> Result<(), String> {
        match self.reaper.inner.sender.try_send(ReapJob(handles)) {
            Ok(()) => {
                self.active = false;
                Ok(())
            }
            Err(error) => Err(format!("playback reaper enqueue failed: {error}")),
        }
    }
}

impl Drop for ReapPermit {
    fn drop(&mut self) {
        if self.active {
            self.reaper.inner.outstanding.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// Drives the playback playhead. The audio master clock (cpal) implements this in
/// PR2; PR1 uses [`InstantClock`] (wall-clock) and the no-audio fallback.
pub trait PlaybackClock: Send + Sync {
    /// The target timeline frame *now*, given the project fps.
    fn frame(&self, fps: i32) -> i32;
    /// Reset the clock so `frame()` resumes counting from `frame`.
    fn seek(&self, frame: i32);
}

/// Receives each composited frame. PR1: an in-memory collector (tests). PR2: the
/// MJPEG sink (JPEG-encode + broadcast).
pub trait FrameSink: Send + Sync {
    fn push_frame(&self, frame: &DecodedFrame);
}

/// Broadcasts the current playhead frame so the front end can move its playhead /
/// timecode while the pixels arrive over a separate channel. PR1: a collector;
/// PR2: a Tauri event emitter.
pub trait PlayheadEmitter: Send + Sync {
    fn emit(&self, frame: i32);
}

/// Control messages to the render thread.
pub enum PlaybackCmd {
    /// Freeze at `frame` while retaining GPU and decoder state.
    Pause(i32, mpsc::Sender<()>),
    /// Resume a retained session from `frame`.
    Resume(i32, mpsc::Sender<()>),
    /// Wake the render thread to consume the newest coalesced seek.
    Seek,
    /// Stop the loop and tear down (streams stop cooperatively).
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SeekRequest {
    frame: i32,
    generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SeekSubmission {
    should_wake: bool,
}

#[derive(Default)]
struct SeekMailboxState {
    generation: u64,
    pending: Option<SeekRequest>,
    wake_queued: bool,
}

/// One-slot newest-wins mailbox. Twenty rapid seeks overwrite one pending
/// request and enqueue at most one control wake; the generation also lets the
/// render thread discard pixels completed after a newer seek arrived.
#[derive(Default)]
struct SeekMailbox(Mutex<SeekMailboxState>);

impl SeekMailbox {
    fn submit(&self, frame: i32) -> SeekSubmission {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.generation = state.generation.wrapping_add(1);
        state.pending = Some(SeekRequest {
            frame,
            generation: state.generation,
        });
        let should_wake = !state.wake_queued;
        state.wake_queued = true;
        SeekSubmission { should_wake }
    }

    fn take(&self) -> Option<SeekRequest> {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let request = state.pending.take();
        state.wake_queued = false;
        request
    }

    fn generation(&self) -> u64 {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .generation
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation() == generation
    }
}

/// Integer target frame from a base frame plus elapsed time. Truncates (matching
/// the `secondsToFrame = Int(secs*fps)` port rule), never rounds. `fps <= 0`
/// falls back to 30 (the project default) to stay defined.
fn frame_at_elapsed(base_frame: i32, elapsed_secs: f64, fps: i32) -> i32 {
    let fps = if fps > 0 { fps } else { 30 };
    base_frame + (elapsed_secs.max(0.0) * fps as f64) as i32
}

/// Clamp the clock's frame to the drawable range and decide whether playback has
/// reached the end. Returns `(target, done)`: `target` is the frame to render,
/// `done` is true once the clock hits the last frame (→ auto-stop). Pure so the
/// loop's termination boundary is unit-tested.
fn loop_step(clock_frame: i32, total: i32) -> (i32, bool) {
    let last = total.max(1) - 1;
    (clock_frame.clamp(0, last), clock_frame >= last)
}

/// Wall-clock playback clock: the PR1 driver and the no-audio fallback. Advances
/// the playhead by real elapsed time from the last `seek` (or construction).
pub struct InstantClock {
    /// `(origin, base_frame)`: `frame()` = `base_frame + elapsed_since(origin)`.
    inner: Mutex<(Instant, i32)>,
}

impl InstantClock {
    pub fn new(start_frame: i32) -> Self {
        InstantClock {
            inner: Mutex::new((Instant::now(), start_frame)),
        }
    }
}

impl PlaybackClock for InstantClock {
    fn frame(&self, fps: i32) -> i32 {
        // Recover from a poisoned lock rather than panicking on the render thread.
        let guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let (origin, base) = *guard;
        frame_at_elapsed(base, origin.elapsed().as_secs_f64(), fps)
    }

    fn seek(&self, frame: i32) {
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        *guard = (Instant::now(), frame);
    }
}

/// The GPU-backed render loop: owns the device, the (frame-independent)
/// [`RenderPlan`], and the streaming resolver state. One instance lives for a
/// whole playback session on the render thread. Exposed (with `render_frame`) so
/// a GPU+ffmpeg integration test can drive it deterministically without the
/// thread/clock.
pub struct RenderLoop {
    device: opentake_render::wgpu::Device,
    queue: opentake_render::wgpu::Queue,
    compositor: Compositor,
    timeline: Timeline,
    plan: RenderPlan,
    render_size: RenderSize,
    state: PlaybackResolverState,
}

impl RenderLoop {
    /// Build the render loop: acquire a GPU device, build the render plan from the
    /// timeline (same `build_render_plan` the preview/export use), and prime the
    /// resolver state. Returns `Err` (never panics) when no GPU is available.
    pub fn new(
        timeline: Timeline,
        media: HashMap<String, MediaInfo>,
        text: HashMap<String, TextInfo>,
        sizes: HashMap<String, (u32, u32)>,
        render_size: RenderSize,
    ) -> Result<Self, String> {
        Self::new_with_cancel(
            timeline,
            media,
            text,
            sizes,
            render_size,
            MediaCancelToken::new(),
        )
    }

    fn new_with_cancel(
        timeline: Timeline,
        media: HashMap<String, MediaInfo>,
        text: HashMap<String, TextInfo>,
        sizes: HashMap<String, (u32, u32)>,
        render_size: RenderSize,
        cancel: MediaCancelToken,
    ) -> Result<Self, String> {
        let dev = RenderDevice::try_new().map_err(|e| format!("no GPU device: {e}"))?;
        let compositor = Compositor::new(&dev.device);
        let metrics = ManifestMetrics { sizes };
        let plan = try_build_render_plan(&timeline, render_size, &metrics)
            .map_err(|error| format!("invalid timeline graph: {error}"))?;
        let state = PlaybackResolverState::new(
            media,
            text,
            plan.fps,
            (render_size.width, render_size.height),
            cancel,
        );
        Ok(RenderLoop {
            device: dev.device,
            queue: dev.queue,
            compositor,
            timeline,
            plan,
            render_size,
            state,
        })
    }

    pub fn total_frames(&self) -> i32 {
        self.plan.total_frames
    }

    pub fn fps(&self) -> i32 {
        self.plan.fps
    }

    /// Composite a single frame at `target`: reconcile the streams to this frame,
    /// then run the same compositor pixel path as the preview/export.
    pub fn render_frame(&mut self, target: i32) -> Result<DecodedFrame, String> {
        let frame_plan = self.plan.frame(&self.timeline, target);
        let mut resolver = StreamingResolver::new(&self.device, &self.queue, &mut self.state);
        resolver.sync_active(&frame_plan)?;
        self.compositor
            .render_to_rgba(
                &self.device,
                &self.queue,
                self.render_size,
                &frame_plan,
                &mut resolver,
            )
            .map_err(|e| format!("composite render failed at frame {target}: {e}"))
    }

    /// Restart all decode streams (used on seek): the next `render_frame` re-spawns
    /// each visible clip's stream at its new target source frame.
    pub fn seek(&mut self) {
        self.state.clear_streams();
    }
}

/// Owns the playback render thread and a control channel to it. Dropping (or
/// `stop`) requests a cooperative shutdown.
pub struct PlaybackEngine {
    control_tx: mpsc::Sender<PlaybackCmd>,
    seek_mailbox: Arc<SeekMailbox>,
    handle: Option<JoinHandle<()>>,
    cancel: MediaCancelToken,
}

impl PlaybackEngine {
    /// Spawn the render thread. The GPU device is created **inside** the thread
    /// (so nothing non-`Send` crosses the boundary); on GPU-acquire failure the
    /// thread logs and exits, leaving this handle inert.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        timeline: Timeline,
        media: HashMap<String, MediaInfo>,
        text: HashMap<String, TextInfo>,
        sizes: HashMap<String, (u32, u32)>,
        render_size: RenderSize,
        clock: Arc<dyn PlaybackClock>,
        sink: Arc<dyn FrameSink>,
        emitter: Arc<dyn PlayheadEmitter>,
    ) -> Result<Self, String> {
        Self::spawn_internal(
            timeline,
            media,
            text,
            sizes,
            render_size,
            clock,
            sink,
            emitter,
            None,
            None,
            MediaCancelToken::new(),
        )
    }

    /// Spawn the GPU thread, render and buffer its first complete frame, then
    /// return a paused handle. The caller installs the authoritative session
    /// before `resume` makes that buffered frame observable. Waiting for the
    /// render-thread handshake is synchronous, so async command callers must run
    /// this constructor on a blocking worker.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_ready(
        timeline: Timeline,
        media: HashMap<String, MediaInfo>,
        text: HashMap<String, TextInfo>,
        sizes: HashMap<String, (u32, u32)>,
        render_size: RenderSize,
        clock: Arc<dyn PlaybackClock>,
        sink: Arc<dyn FrameSink>,
        emitter: Arc<dyn PlayheadEmitter>,
        start_frame: i32,
    ) -> Result<Self, String> {
        Self::spawn_ready_cancellable(
            timeline,
            media,
            text,
            sizes,
            render_size,
            clock,
            sink,
            emitter,
            start_frame,
            MediaCancelToken::new(),
        )
    }

    /// Prepare the first exact frame with a caller-owned session token. The
    /// playback coordinator keeps this token reachable until installation, so
    /// project/timeline invalidation can cancel a blocked initial bootstrap
    /// before a [`PlaybackEngine`] handle exists.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_ready_cancellable(
        timeline: Timeline,
        media: HashMap<String, MediaInfo>,
        text: HashMap<String, TextInfo>,
        sizes: HashMap<String, (u32, u32)>,
        render_size: RenderSize,
        clock: Arc<dyn PlaybackClock>,
        sink: Arc<dyn FrameSink>,
        emitter: Arc<dyn PlayheadEmitter>,
        start_frame: i32,
        cancel: MediaCancelToken,
    ) -> Result<Self, String> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let engine = Self::spawn_internal(
            timeline,
            media,
            text,
            sizes,
            render_size,
            clock,
            sink,
            emitter,
            Some(start_frame.max(0)),
            Some(ready_tx),
            cancel,
        )?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(engine),
            Ok(Err(error)) => {
                engine.stop();
                Err(error)
            }
            Err(_) => {
                engine.stop();
                Err("playback thread exited before the first frame".to_string())
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_internal(
        timeline: Timeline,
        media: HashMap<String, MediaInfo>,
        text: HashMap<String, TextInfo>,
        sizes: HashMap<String, (u32, u32)>,
        render_size: RenderSize,
        clock: Arc<dyn PlaybackClock>,
        sink: Arc<dyn FrameSink>,
        emitter: Arc<dyn PlayheadEmitter>,
        initial_frame: Option<i32>,
        startup: Option<mpsc::Sender<Result<(), String>>>,
        cancel: MediaCancelToken,
    ) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let seek_mailbox = Arc::new(SeekMailbox::default());
        let render_seek_mailbox = Arc::clone(&seek_mailbox);
        let render_cancel = cancel.clone();
        let handle = thread::Builder::new()
            .name("opentake-playback-render".to_string())
            .spawn(move || {
                run_render_thread(
                    timeline,
                    media,
                    text,
                    sizes,
                    render_size,
                    clock,
                    sink,
                    emitter,
                    rx,
                    render_seek_mailbox,
                    initial_frame,
                    startup,
                    render_cancel,
                );
            })
            .map_err(|e| format!("spawn playback thread: {e}"))?;
        Ok(PlaybackEngine {
            control_tx: tx,
            seek_mailbox,
            handle: Some(handle),
            cancel,
        })
    }

    /// Seek the running engine to `frame`.
    pub fn seek(&self, frame: i32) {
        if self.seek_mailbox.submit(frame).should_wake {
            let _ = self.control_tx.send(PlaybackCmd::Seek);
        }
    }

    pub fn pause(&self, frame: i32) -> Result<(), String> {
        self.barrier(|reply| PlaybackCmd::Pause(frame, reply))
    }

    pub fn resume(&self, frame: i32) -> Result<(), String> {
        self.barrier(|reply| PlaybackCmd::Resume(frame, reply))
    }

    fn barrier(&self, command: impl FnOnce(mpsc::Sender<()>) -> PlaybackCmd) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.control_tx
            .send(command(reply_tx))
            .map_err(|_| "playback render thread exited before control".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "playback render thread exited during control".to_string())
    }

    /// Stop the engine and join the render thread.
    pub fn stop(mut self) {
        self.cancel.cancel();
        let _ = self.control_tx.send(PlaybackCmd::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    pub fn request_stop(mut self) -> Option<JoinHandle<()>> {
        self.cancel.cancel();
        let _ = self.control_tx.send(PlaybackCmd::Stop);
        self.handle.take()
    }

    #[cfg(test)]
    pub(crate) fn test_stub() -> (Self, mpsc::Receiver<()>) {
        let (control_tx, control_rx) = mpsc::channel();
        let (stopped_tx, stopped_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            while let Ok(command) = control_rx.recv() {
                match command {
                    PlaybackCmd::Stop => {
                        let _ = stopped_tx.send(());
                        break;
                    }
                    PlaybackCmd::Pause(_, reply) | PlaybackCmd::Resume(_, reply) => {
                        let _ = reply.send(());
                    }
                    PlaybackCmd::Seek => {}
                }
            }
        });
        (
            Self {
                control_tx,
                seek_mailbox: Arc::new(SeekMailbox::default()),
                handle: Some(handle),
                cancel: MediaCancelToken::new(),
            },
            stopped_rx,
        )
    }

    #[cfg(test)]
    pub(crate) fn test_resume_observer(
        audio_paused: Arc<std::sync::atomic::AtomicBool>,
    ) -> (Self, mpsc::Receiver<bool>, mpsc::Receiver<()>) {
        let (control_tx, control_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let (stopped_tx, stopped_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            while let Ok(command) = control_rx.recv() {
                match command {
                    PlaybackCmd::Stop => {
                        let _ = stopped_tx.send(());
                        break;
                    }
                    PlaybackCmd::Pause(_, reply) => {
                        let _ = reply.send(());
                    }
                    PlaybackCmd::Resume(_, reply) => {
                        let paused = audio_paused.load(std::sync::atomic::Ordering::Acquire);
                        let _ = resume_tx.send(paused);
                        let _ = reply.send(());
                    }
                    PlaybackCmd::Seek => {}
                }
            }
        });
        (
            Self {
                control_tx,
                seek_mailbox: Arc::new(SeekMailbox::default()),
                handle: Some(handle),
                cancel: MediaCancelToken::new(),
            },
            resume_rx,
            stopped_rx,
        )
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        // Best-effort cooperative stop if the caller didn't `stop()` explicitly.
        self.cancel.cancel();
        let _ = self.control_tx.send(PlaybackCmd::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The render thread body: build the loop, then render frames paced at the
/// project fps until the clock reaches the end or a `Stop` arrives.
#[allow(clippy::too_many_arguments)]
fn run_render_thread(
    timeline: Timeline,
    media: HashMap<String, MediaInfo>,
    text: HashMap<String, TextInfo>,
    sizes: HashMap<String, (u32, u32)>,
    render_size: RenderSize,
    clock: Arc<dyn PlaybackClock>,
    sink: Arc<dyn FrameSink>,
    emitter: Arc<dyn PlayheadEmitter>,
    rx: mpsc::Receiver<PlaybackCmd>,
    seek_mailbox: Arc<SeekMailbox>,
    initial_frame: Option<i32>,
    mut startup: Option<mpsc::Sender<Result<(), String>>>,
    cancel: MediaCancelToken,
) {
    let mut render_loop =
        match RenderLoop::new_with_cancel(timeline, media, text, sizes, render_size, cancel) {
            Ok(rl) => rl,
            Err(e) => {
                if let Some(tx) = startup.take() {
                    let _ = tx.send(Err(e.clone()));
                }
                eprintln!("[playback] {e}");
                return;
            }
        };
    let total = render_loop.total_frames();
    let fps = render_loop.fps();
    if total <= 0 {
        if let Some(tx) = startup.take() {
            let _ = tx.send(Err("playback timeline has no drawable frames".to_string()));
        }
        return;
    }
    if let Some(frame) = initial_frame {
        clock.seek(frame);
    }
    let frame_dur = Duration::from_secs_f64(1.0 / fps.max(1) as f64);
    let mut paused = false;
    let mut buffered_first: Option<(i32, DecodedFrame)> = None;

    loop {
        if paused {
            match rx.recv() {
                Ok(PlaybackCmd::Pause(frame, reply)) => {
                    clock.seek(frame);
                    let _ = reply.send(());
                }
                Ok(PlaybackCmd::Resume(frame, reply)) => {
                    clock.seek(frame);
                    render_loop.seek();
                    if let Some((buffered_frame, image)) = buffered_first.take() {
                        sink.push_frame(&image);
                        emitter.emit(buffered_frame);
                    }
                    paused = false;
                    let _ = reply.send(());
                }
                Ok(PlaybackCmd::Seek) => {
                    if let Some(request) = seek_mailbox.take() {
                        clock.seek(request.frame);
                        render_loop.seek();
                        buffered_first = None;
                    }
                }
                Ok(PlaybackCmd::Stop) | Err(_) => return,
            }
            continue;
        }
        let tick = Instant::now();

        // Drain pending control messages first.
        loop {
            match rx.try_recv() {
                Ok(PlaybackCmd::Pause(frame, reply)) => {
                    clock.seek(frame);
                    paused = true;
                    let _ = reply.send(());
                    break;
                }
                Ok(PlaybackCmd::Resume(frame, reply)) => {
                    clock.seek(frame);
                    render_loop.seek();
                    let _ = reply.send(());
                }
                Ok(PlaybackCmd::Seek) => {
                    if let Some(request) = seek_mailbox.take() {
                        clock.seek(request.frame);
                        render_loop.seek();
                    }
                }
                Ok(PlaybackCmd::Stop) => return,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        if paused {
            continue;
        }

        let (clamped, done) = loop_step(clock.frame(fps), total);
        let render_generation = seek_mailbox.generation();
        let rendered = render_loop.render_frame(clamped);
        if !seek_mailbox.is_current(render_generation) {
            continue;
        }
        match rendered {
            Ok(frame) => {
                if let Some(tx) = startup.take() {
                    if tx.send(Ok(())).is_err() {
                        return;
                    }
                    buffered_first = Some((clamped, frame));
                    paused = true;
                } else {
                    sink.push_frame(&frame);
                    emitter.emit(clamped);
                }
            }
            Err(e) => {
                if let Some(tx) = startup.take() {
                    let _ = tx.send(Err(e.clone()));
                    return;
                }
                eprintln!("[playback] {e}");
            }
        }

        // Auto-stop once the clock reaches the final frame (#53: end → stop).
        if done || paused {
            paused = true;
            continue;
        }

        // Sleep only the remainder of the frame budget (#192): the target
        // frame comes from the audio-master clock (absolute time), so when a
        // render overruns `frame_dur` we don't sleep at all and `loop_step`
        // catches up on the next iteration. Sleeping the full `frame_dur`
        // unconditionally here previously stacked render time on top of the
        // frame period and capped playback at ~22fps regardless of target fps.
        let elapsed = tick.elapsed();
        if elapsed < frame_dur {
            thread::sleep(frame_dur - elapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_reaper_rejects_new_start_when_teardown_backlog_is_full() {
        let reaper = BoundedReaper::new();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = Arc::new(Mutex::new(release_rx));

        for _ in 0..2 {
            let permit = reaper
                .try_reserve()
                .expect("two teardown jobs fit the bounded reaper");
            let job_release = Arc::clone(&release_rx);
            let handle = thread::spawn(move || {
                job_release
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .recv()
                    .expect("release teardown handle");
            });
            permit
                .enqueue(vec![handle])
                .expect("enqueue teardown handles");
        }

        assert!(
            reaper.try_reserve().is_err(),
            "a third start must be rejected while two teardowns are outstanding"
        );
        release_tx.send(()).expect("release first teardown");
        release_tx.send(()).expect("release second teardown");
        reaper.wait_until_idle(Duration::from_secs(2));
    }

    #[test]
    fn frame_at_elapsed_truncates_not_rounds() {
        // 0.999 frames of elapsed time is still frame 0 (truncate toward zero).
        assert_eq!(frame_at_elapsed(0, 0.999 / 30.0, 30), 0);
        // Exactly one frame's worth advances by one.
        assert_eq!(frame_at_elapsed(0, 1.0 / 30.0, 30), 1);
        // 2.5 frames -> 2 (no rounding up).
        assert_eq!(frame_at_elapsed(0, 2.5 / 30.0, 30), 2);
    }

    #[test]
    fn frame_at_elapsed_applies_base_offset() {
        assert_eq!(frame_at_elapsed(100, 1.0, 30), 130);
    }

    #[test]
    fn loop_step_clamps_and_flags_end() {
        assert_eq!(loop_step(5, 100), (5, false));
        assert_eq!(loop_step(99, 100), (99, true)); // last frame → done
        assert_eq!(loop_step(150, 100), (99, true)); // past end → clamp + done
        assert_eq!(loop_step(-5, 100), (0, false)); // negative → clamp to 0
        assert_eq!(loop_step(0, 1), (0, true)); // single-frame timeline
    }

    #[test]
    fn frame_at_elapsed_clamps_negative_elapsed_and_bad_fps() {
        assert_eq!(frame_at_elapsed(10, -5.0, 30), 10);
        // fps <= 0 falls back to 30, so one second is 30 frames.
        assert_eq!(frame_at_elapsed(0, 1.0, 0), 30);
    }

    #[test]
    fn instant_clock_seek_resets_base_frame() {
        let clock = InstantClock::new(0);
        clock.seek(500);
        // Immediately after a seek, ~no time has elapsed, so we're at the base.
        let f = clock.frame(30);
        assert!(
            (500..=501).contains(&f),
            "expected ~500 right after seek, got {f}"
        );
    }

    #[test]
    fn rapid_seek_mailbox_keeps_only_the_latest_frame_and_one_wake() {
        let mailbox = SeekMailbox::default();
        let wake_count = (0..20)
            .filter(|frame| mailbox.submit(*frame).should_wake)
            .count();

        assert_eq!(wake_count, 1);
        assert_eq!(
            mailbox.take(),
            Some(SeekRequest {
                frame: 19,
                generation: 20
            })
        );
        assert_eq!(mailbox.take(), None);
    }

    #[test]
    fn a_new_seek_generation_supersedes_an_inflight_render() {
        let mailbox = SeekMailbox::default();
        mailbox.submit(10);
        let rendering = mailbox.take().expect("first seek");
        mailbox.submit(99);

        assert!(!mailbox.is_current(rendering.generation));
        assert_eq!(
            mailbox.take(),
            Some(SeekRequest {
                frame: 99,
                generation: 2
            })
        );
    }
}
