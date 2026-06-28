//! Tauri command surface + managed state for streaming playback (#53 / PR2).
//!
//! `playback_start` snapshots the live session, builds the render engine
//! ([`PlaybackEngine`]) with a clock (cpal master clock when the timeline has
//! audio, else the wall-clock [`InstantClock`]), an [`MjpegSink`] feeding the
//! loopback transport, and a Tauri playhead emitter, then keeps the running
//! engine in [`PlaybackState`] so `playback_pause` / `playback_seek` /
//! `playback_stop` can drive it.
//!
//! The front end points an `<img>` at [`get_preview_endpoint`] during PLAY and
//! moves its playhead from the `playback_frame` events; scrub / pause stay on the
//! existing `<video>` + `composite_frame` path (wired in PR3).

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager, State};

use opentake_core::AppCore;
use opentake_render::{even, RenderSize};

use super::audio::build_clock;
use super::engine::{FrameSink, PlaybackEngine, PlayheadEmitter};
use super::project::{project_media, project_text};
use super::transport::{PreviewServer, TauriPlayheadEmitter};

/// Preview downscale cap (longest side, px) for streaming playback — matches the
/// single-frame preview so PLAY and scrub/pause look identical.
const PLAYBACK_PREVIEW_CAP: u32 = 1280;

/// A live playback session: the render engine plus the audio device handle.
/// The audio handle is kept alive for the session (dropping it stops the cpal
/// stream); `_audio` is `None` for a silent timeline (wall-clock driven).
struct RunningPlayback {
    engine: PlaybackEngine,
    _audio: Option<super::audio::AudioPlayback>,
}

/// Holds the currently-running playback session (if any). A new `playback_start`
/// stops the previous one first, so at most one render thread + audio stream run.
#[derive(Default)]
pub struct PlaybackState {
    running: Mutex<Option<RunningPlayback>>,
}

impl PlaybackState {
    pub fn new() -> Self {
        PlaybackState::default()
    }

    /// Replace the running session, stopping (and joining) any previous one
    /// AFTER releasing the lock — a slow render-thread join must not block the
    /// other playback commands (which all take this same lock).
    fn install(&self, engine: PlaybackEngine, audio: Option<super::audio::AudioPlayback>) {
        let old = {
            let mut guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
            let old = guard.take();
            *guard = Some(RunningPlayback {
                engine,
                _audio: audio,
            });
            old
        };
        if let Some(session) = old {
            session.engine.stop();
        }
    }

    /// Stop and drop the running session (render thread joined, audio dropped).
    fn shutdown(&self) {
        let taken = {
            let mut guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
            guard.take()
        };
        if let Some(session) = taken {
            session.engine.stop();
        }
    }

    /// Forward a seek to the running engine, if any.
    fn forward_seek(&self, frame: i32) {
        let guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(session) = guard.as_ref() {
            session.engine.seek(frame);
        }
    }
}

/// Even-ized, cap-limited playback render size (uniform scale preserves the
/// plan's affine math). Mirrors `render::preview_render_size`.
fn playback_render_size(canvas_w: i32, canvas_h: i32, cap: u32) -> RenderSize {
    let cw = (canvas_w.max(2)) as f64;
    let ch = (canvas_h.max(2)) as f64;
    if cap == 0 {
        return RenderSize::new(even(cw), even(ch));
    }
    let long = cw.max(ch);
    let scale = if long > cap as f64 {
        cap as f64 / long
    } else {
        1.0
    };
    RenderSize::new(even(cw * scale), even(ch * scale))
}

/// Start (or restart) continuous playback from `from_frame`.
///
/// `from_frame` is the current playhead (the front end owns playhead state). The
/// engine renders forward from there, streaming JPEG frames over the MJPEG
/// transport and emitting `playback_frame` events. Returns `Err` only on engine
/// spawn failure; a GPU-less host fails fast here.
#[tauri::command]
pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), String> {
    // Snapshot the session synchronously — no managed-state guard is held across
    // the await below (Tauri async commands require a Send future).
    let (timeline, sizes, media, text, render_size, fps, sink, emitter) = {
        let core = app.state::<AppCore>();
        let timeline = core.get_timeline().timeline;
        let manifest = core.media();
        let project_dir = core.project_dir();
        let (sizes, media) = project_media(&manifest, &project_dir);
        let text = project_text(&timeline);
        let render_size =
            playback_render_size(timeline.width, timeline.height, PLAYBACK_PREVIEW_CAP);
        let fps = timeline.fps;
        let sink: Arc<dyn FrameSink> = Arc::new(app.state::<Arc<PreviewServer>>().sink());
        let emitter: Arc<dyn PlayheadEmitter> = Arc::new(TauriPlayheadEmitter::new(app.clone()));
        (
            timeline,
            sizes,
            media,
            text,
            render_size,
            fps,
            sink,
            emitter,
        )
    };
    let start_at = from_frame.max(0);

    // Decoding + mixing the whole timeline's audio (ffmpeg per clip) can take
    // seconds on a long project; run it (and cpal setup) off the IPC thread so
    // the command never freezes the UI.
    let (clock, audio) = {
        let timeline = timeline.clone();
        let media = media.clone();
        tokio::task::spawn_blocking(move || build_clock(&timeline, &media, fps, start_at))
            .await
            .map_err(|e| format!("audio prepare task failed: {e}"))?
    };

    let engine = PlaybackEngine::spawn(
        timeline,
        media,
        text,
        sizes,
        render_size,
        clock,
        sink,
        emitter,
    )?;
    app.state::<PlaybackState>().install(engine, audio);
    Ok(())
}

/// Pause playback: stop the render thread (the front end freezes the `<video>`
/// at the last `playback_frame`). Same teardown as stop in this cut.
#[tauri::command]
pub fn playback_pause(playback: State<'_, PlaybackState>) -> Result<(), String> {
    playback.shutdown();
    Ok(())
}

/// Stop playback and tear down the engine.
#[tauri::command]
pub fn playback_stop(playback: State<'_, PlaybackState>) -> Result<(), String> {
    playback.shutdown();
    Ok(())
}

/// Seek the running engine to `frame` (no-op when not playing).
#[tauri::command]
pub fn playback_seek(playback: State<'_, PlaybackState>, frame: i32) -> Result<(), String> {
    playback.forward_seek(frame.max(0));
    Ok(())
}

/// The MJPEG stream URL the front end points a playback `<img>` at.
#[tauri::command]
pub fn get_preview_endpoint(server: State<'_, Arc<PreviewServer>>) -> String {
    server.endpoint()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_size_caps_long_side_keeping_aspect() {
        assert_eq!(
            playback_render_size(1920, 1080, 1280),
            RenderSize::new(1280, 720)
        );
    }

    #[test]
    fn render_size_never_upscales_under_cap() {
        assert_eq!(
            playback_render_size(640, 480, 1280),
            RenderSize::new(640, 480)
        );
    }

    #[test]
    fn render_size_floors_degenerate_canvas() {
        assert_eq!(playback_render_size(0, 0, 1280), RenderSize::new(2, 2));
    }
}
