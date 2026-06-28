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
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use opentake_domain::Timeline;
use opentake_render::{
    build_render_plan, Compositor, DecodedFrame, RenderDevice, RenderPlan, RenderSize,
};

use super::project::{ManifestMetrics, MediaInfo, TextInfo};
use super::resolver::{PlaybackResolverState, StreamingResolver};

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
    /// Jump the clock + restart streams at this frame.
    Seek(i32),
    /// Stop the loop and tear down (streams stop cooperatively).
    Stop,
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
        let dev = RenderDevice::try_new().map_err(|e| format!("no GPU device: {e}"))?;
        let compositor = Compositor::new(&dev.device);
        let metrics = ManifestMetrics { sizes };
        let plan = build_render_plan(&timeline, render_size, &metrics);
        let state = PlaybackResolverState::new(
            media,
            text,
            plan.fps,
            (render_size.width, render_size.height),
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
        resolver.sync_active(&frame_plan);
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
    handle: Option<JoinHandle<()>>,
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
        let (tx, rx) = mpsc::channel();
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
                );
            })
            .map_err(|e| format!("spawn playback thread: {e}"))?;
        Ok(PlaybackEngine {
            control_tx: tx,
            handle: Some(handle),
        })
    }

    /// Seek the running engine to `frame`.
    pub fn seek(&self, frame: i32) {
        let _ = self.control_tx.send(PlaybackCmd::Seek(frame));
    }

    /// Stop the engine and join the render thread.
    pub fn stop(mut self) {
        let _ = self.control_tx.send(PlaybackCmd::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        // Best-effort cooperative stop if the caller didn't `stop()` explicitly.
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
) {
    let mut render_loop = match RenderLoop::new(timeline, media, text, sizes, render_size) {
        Ok(rl) => rl,
        Err(e) => {
            eprintln!("[playback] {e}");
            return;
        }
    };
    let total = render_loop.total_frames();
    let fps = render_loop.fps();
    if total <= 0 {
        return;
    }
    let frame_dur = Duration::from_secs_f64(1.0 / fps.max(1) as f64);

    loop {
        // Drain pending control messages first.
        loop {
            match rx.try_recv() {
                Ok(PlaybackCmd::Seek(f)) => {
                    clock.seek(f);
                    render_loop.seek();
                }
                Ok(PlaybackCmd::Stop) => return,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        let (clamped, done) = loop_step(clock.frame(fps), total);
        match render_loop.render_frame(clamped) {
            Ok(frame) => {
                sink.push_frame(&frame);
                emitter.emit(clamped);
            }
            Err(e) => eprintln!("[playback] {e}"),
        }

        // Auto-stop once the clock reaches the final frame (#53: end → stop).
        if done {
            return;
        }
        thread::sleep(frame_dur);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
