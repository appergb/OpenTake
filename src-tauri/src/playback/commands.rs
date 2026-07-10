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

use super::audio::build_clock_paused;
use super::engine::{FrameSink, PlaybackEngine, PlayheadEmitter};
use super::project::{project_media, project_text};
use super::session::{
    PlaybackCommandError, PlaybackIdentity, ProjectTransition, SessionControl, SessionRegistry,
    StartDecision, StartTicket,
};
use super::transport::{PreviewServer, PublicationGate, TauriPlayheadEmitter};

/// Preview downscale cap (longest side, px) for streaming playback — matches the
/// single-frame preview so PLAY and scrub/pause look identical.
const PLAYBACK_PREVIEW_CAP: u32 = 1280;

/// A live playback session: the render engine plus the audio device handle.
/// The audio handle is kept alive for the session (dropping it stops the cpal
/// stream); `_audio` is `None` for a silent timeline (wall-clock driven).
struct RunningPlayback {
    identity: PlaybackIdentity,
    engine: PlaybackEngine,
    audio: Option<super::audio::AudioPlayback>,
    publication: PublicationGate,
}

impl RunningPlayback {
    fn close_publication(&self) {
        self.publication.close();
    }

    fn stop(self) {
        self.publication.close();
        drop(self.audio);
        self.engine.stop();
    }
}

#[derive(Default)]
struct PlaybackSlot {
    sessions: SessionRegistry,
    running: Option<RunningPlayback>,
}

#[derive(Default)]
pub struct PlaybackState {
    slot: Mutex<PlaybackSlot>,
}

impl PlaybackState {
    pub fn new() -> Self {
        PlaybackState::default()
    }

    fn prepare_start(
        &self,
        identity: PlaybackIdentity,
        authoritative: opentake_core::ProjectRevision,
        frame: i32,
    ) -> Result<Option<StartTicket>, PlaybackCommandError> {
        let (decision, old) = {
            let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
            let decision = slot.sessions.begin_start(identity.clone(), authoritative)?;
            if matches!(decision, StartDecision::Resume) {
                let Some(running) = slot.running.as_ref() else {
                    return Err(PlaybackCommandError::superseded(
                        "retained playback resources are no longer installed",
                    ));
                };
                if running.identity != identity {
                    return Err(PlaybackCommandError::superseded(
                        "retained playback identity changed",
                    ));
                }
                running
                    .engine
                    .resume(frame)
                    .map_err(PlaybackCommandError::engine)?;
                if let Some(audio) = running.audio.as_ref() {
                    audio.resume().map_err(PlaybackCommandError::engine)?;
                }
                return Ok(None);
            }
            let old = slot.running.take();
            (decision, old)
        };
        if let Some(running) = old {
            running.stop();
        }
        let StartDecision::Build(ticket) = decision else {
            unreachable!("resume returned above")
        };
        Ok(Some(ticket))
    }

    fn install_if_current(
        &self,
        ticket: StartTicket,
        authoritative: opentake_core::ProjectRevision,
        engine: PlaybackEngine,
        audio: Option<super::audio::AudioPlayback>,
        publication: PublicationGate,
        frame: i32,
    ) -> Result<(), PlaybackCommandError> {
        let identity = ticket.identity().clone();
        let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(error) = slot.sessions.install_if_current(ticket, authoritative) {
            drop(slot);
            publication.close();
            drop(audio);
            engine.stop();
            return Err(error);
        }
        if let Err(error) = engine.resume(frame) {
            slot.sessions.control(&identity, SessionControl::Stop);
            drop(slot);
            publication.close();
            drop(audio);
            engine.stop();
            return Err(PlaybackCommandError::engine(error));
        }
        if let Some(audio_playback) = audio.as_ref() {
            if let Err(error) = audio_playback.resume() {
                slot.sessions.control(&identity, SessionControl::Stop);
                drop(slot);
                publication.close();
                drop(audio);
                engine.stop();
                return Err(PlaybackCommandError::engine(error));
            }
        }
        slot.running = Some(RunningPlayback {
            identity,
            engine,
            audio,
            publication,
        });
        Ok(())
    }

    fn control(
        &self,
        identity: PlaybackIdentity,
        control: SessionControl,
        frame: i32,
    ) -> Result<(), PlaybackCommandError> {
        let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
        if !slot.sessions.control(&identity, control) {
            return Err(PlaybackCommandError::superseded(
                "playback control targeted a stale session",
            ));
        }
        match control {
            SessionControl::Pause => {
                let running = slot.running.as_ref().ok_or_else(|| {
                    PlaybackCommandError::superseded("playback session is no longer installed")
                })?;
                if let Some(audio) = running.audio.as_ref() {
                    audio.pause().map_err(PlaybackCommandError::engine)?;
                }
                running
                    .engine
                    .pause(frame)
                    .map_err(PlaybackCommandError::engine)?;
            }
            SessionControl::Seek => {
                let running = slot.running.as_ref().ok_or_else(|| {
                    PlaybackCommandError::superseded("playback session is no longer installed")
                })?;
                running.engine.seek(frame);
            }
            SessionControl::Stop => {
                let running = slot.running.take();
                drop(slot);
                if let Some(running) = running {
                    running.stop();
                }
            }
        }
        Ok(())
    }

    pub fn begin_project_transition(&self) -> ProjectTransition {
        let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
        let transition = slot.sessions.begin_project_transition();
        if let Some(running) = slot.running.as_ref() {
            running.close_publication();
        }
        transition
    }

    pub fn cancel_project_transition(&self, transition: ProjectTransition) {
        let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
        if slot.sessions.cancel_project_transition(transition) {
            if let Some(running) = slot.running.as_ref() {
                running.publication.reopen();
            }
        }
    }

    pub fn activate_project(&self, transition: ProjectTransition, project_epoch: u64) {
        let running = {
            let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
            if slot.sessions.activate_project(transition, project_epoch) {
                slot.running.take()
            } else {
                None
            }
        };
        if let Some(running) = running {
            running.stop();
        }
    }

    pub fn activate_project_event(&self, project_epoch: u64) -> Option<PlaybackIdentity> {
        let running = {
            let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
            if slot.sessions.activate_project_event(project_epoch) {
                slot.running.take()
            } else {
                None
            }
        };
        let identity = running.as_ref().map(|running| running.identity.clone());
        if let Some(running) = running {
            running.stop();
        }
        identity
    }

    pub fn invalidate_timeline(
        &self,
        project_epoch: u64,
        version: u64,
    ) -> Option<PlaybackIdentity> {
        let running = {
            let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
            if slot
                .sessions
                .invalidate_for_timeline_change(project_epoch, version)
            {
                slot.running.take()
            } else {
                None
            }
        };
        let identity = running.as_ref().map(|running| running.identity.clone());
        if let Some(running) = running {
            running.stop();
        }
        identity
    }

    pub fn active_identity(&self) -> Option<PlaybackIdentity> {
        self.slot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sessions
            .active_identity()
            .cloned()
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

async fn spawn_ready_off_executor<T, F>(build: F) -> Result<T, PlaybackCommandError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(build)
        .await
        .map_err(|error| {
            PlaybackCommandError::engine(format!("playback ready task failed: {error}"))
        })?
        .map_err(PlaybackCommandError::engine)
}

/// Start (or restart) continuous playback from `from_frame`.
///
/// `from_frame` is the current playhead (the front end owns playhead state). The
/// engine renders forward from there, streaming JPEG frames over the MJPEG
/// transport and emitting `playback_frame` events. Returns `Err` only on engine
/// spawn failure; a GPU-less host fails fast here.
#[tauri::command]
pub async fn playback_start(
    app: AppHandle,
    from_frame: i32,
    identity: PlaybackIdentity,
) -> Result<(), PlaybackCommandError> {
    let identity = identity.validate()?;
    let snapshot = app.state::<AppCore>().runtime_snapshot();
    let authoritative = opentake_core::ProjectRevision {
        project_epoch: snapshot.project_epoch,
        version: snapshot.version,
    };
    let start_at = from_frame.max(0);
    let Some(ticket) =
        app.state::<PlaybackState>()
            .prepare_start(identity.clone(), authoritative, start_at)?
    else {
        return Ok(());
    };

    // Snapshot the session synchronously — no managed-state guard is held across
    // the await below (Tauri async commands require a Send future).
    let (timeline, sizes, media, text, render_size, fps, sink, emitter, publication) = {
        let timeline = snapshot.timeline;
        let manifest = snapshot.media;
        let project_dir = snapshot.project_dir;
        let (sizes, media) = project_media(&manifest, &project_dir);
        let text = project_text(&timeline);
        let render_size =
            playback_render_size(timeline.width, timeline.height, PLAYBACK_PREVIEW_CAP);
        let fps = timeline.fps;
        let server = app.state::<Arc<PreviewServer>>();
        let publication = PublicationGate::open();
        let concrete_sink = server.sink(identity.clone(), publication.clone());
        let emitter: Arc<dyn PlayheadEmitter> = Arc::new(TauriPlayheadEmitter::new(
            app.clone(),
            server.inner().as_ref(),
            &concrete_sink,
            identity.clone(),
            publication.clone(),
            timeline.total_frames().max(1) - 1,
        ));
        let sink: Arc<dyn FrameSink> = Arc::new(concrete_sink);
        (
            timeline,
            sizes,
            media,
            text,
            render_size,
            fps,
            sink,
            emitter,
            publication,
        )
    };

    // Decoding + mixing the whole timeline's audio (ffmpeg per clip) can take
    // seconds on a long project; run it (and cpal setup) off the IPC thread so
    // the command never freezes the UI.
    let (clock, audio) = {
        let timeline = timeline.clone();
        let media = media.clone();
        tokio::task::spawn_blocking(move || build_clock_paused(&timeline, &media, fps, start_at))
            .await
            .map_err(|e| PlaybackCommandError::engine(format!("audio prepare task failed: {e}")))?
    };

    let engine = spawn_ready_off_executor(move || {
        PlaybackEngine::spawn_ready(
            timeline,
            media,
            text,
            sizes,
            render_size,
            clock,
            sink,
            emitter,
            start_at,
        )
    })
    .await?;
    let current = app.state::<AppCore>().project_revision();
    app.state::<PlaybackState>().install_if_current(
        ticket,
        current,
        engine,
        audio,
        publication,
        start_at,
    )
}

/// Pause the matching session while retaining its render and audio resources.
#[tauri::command]
pub fn playback_pause(
    playback: State<'_, PlaybackState>,
    identity: PlaybackIdentity,
    frame: i32,
) -> Result<(), PlaybackCommandError> {
    playback.control(identity.validate()?, SessionControl::Pause, frame.max(0))
}

/// Stop playback and tear down the engine.
#[tauri::command]
pub fn playback_stop(
    playback: State<'_, PlaybackState>,
    identity: PlaybackIdentity,
) -> Result<(), PlaybackCommandError> {
    playback.control(identity.validate()?, SessionControl::Stop, 0)
}

/// Seek the running engine to `frame` (no-op when not playing).
#[tauri::command]
pub fn playback_seek(
    playback: State<'_, PlaybackState>,
    identity: PlaybackIdentity,
    frame: i32,
) -> Result<(), PlaybackCommandError> {
    playback.control(identity.validate()?, SessionControl::Seek, frame.max(0))
}

/// The session-scoped `/frame` endpoint used for exact JPEG requests.
#[tauri::command]
pub fn get_preview_endpoint(server: State<'_, Arc<PreviewServer>>) -> String {
    server.endpoint_frame()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use opentake_domain::Timeline;

    use super::super::engine::{InstantClock, PlaybackClock};

    struct NoopSink;

    impl FrameSink for NoopSink {
        fn push_frame(&self, _frame: &opentake_render::DecodedFrame) {}
    }

    struct NoopEmitter;

    impl PlayheadEmitter for NoopEmitter {
        fn emit(&self, _frame: i32) {}
    }

    fn identity(epoch: u64, version: u64, session_id: &str) -> PlaybackIdentity {
        PlaybackIdentity::new(epoch, version, session_id).expect("valid playback identity")
    }

    fn state_with_running(identity: PlaybackIdentity) -> (PlaybackState, PublicationGate) {
        let state = PlaybackState::new();
        let gate = PublicationGate::open();
        let engine = PlaybackEngine::spawn(
            Timeline::default(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            RenderSize::new(2, 2),
            Arc::new(InstantClock::new(0)) as Arc<dyn PlaybackClock>,
            Arc::new(NoopSink),
            Arc::new(NoopEmitter),
        )
        .expect("spawn inert playback resource");
        {
            let mut slot = state
                .slot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let StartDecision::Build(ticket) = slot
                .sessions
                .begin_start(identity.clone(), identity.revision())
                .expect("begin playback session")
            else {
                panic!("fresh session must build");
            };
            slot.sessions
                .install_if_current(ticket, identity.revision())
                .expect("install playback session");
            slot.running = Some(RunningPlayback {
                identity,
                engine,
                audio: None,
                publication: gate.clone(),
            });
        }
        (state, gate)
    }

    #[test]
    fn stale_project_transition_cannot_reopen_or_take_current_resources() {
        let current = identity(8, 3, "current-session");
        let (state, publication) = state_with_running(current.clone());
        let first = state.begin_project_transition();
        let second = state.begin_project_transition();

        assert!(
            state.activate_project_event(9).is_none(),
            "a project event cannot take resources owned by a live transition"
        );
        assert!(
            !publication.is_open(),
            "project event must keep the transition publication closed"
        );
        state
            .control(current.clone(), SessionControl::Seek, 3)
            .expect("project event must retain transition-owned resources");

        state.cancel_project_transition(first);
        assert!(
            !publication.is_open(),
            "stale cancel must not reopen the transition owner's publication"
        );

        state.activate_project(first, 9);
        state
            .control(current.clone(), SessionControl::Seek, 4)
            .expect("stale activate must not take current playback resources");

        state.cancel_project_transition(second);
        assert!(
            publication.is_open(),
            "owning cancel restores the retained publication"
        );
        state
            .control(current, SessionControl::Stop, 0)
            .expect("stop retained test session");
    }

    #[test]
    fn external_project_event_without_local_transition_still_invalidates_playback() {
        let current = identity(8, 3, "external-boundary");
        let (state, _publication) = state_with_running(current.clone());

        assert_eq!(state.activate_project_event(9), Some(current.clone()));
        assert_eq!(state.active_identity(), None);
        let error = state
            .control(current, SessionControl::Seek, 4)
            .expect_err("external boundary removes the old playback session");
        assert_eq!(
            error.code,
            super::super::session::PlaybackErrorCode::Superseded
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn spawn_ready_wait_does_not_block_async_executor() {
        use std::sync::mpsc;
        use std::time::Duration;

        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (heartbeat_tx, heartbeat_rx) = mpsc::channel();
        tokio::spawn(async move {
            tokio::task::yield_now().await;
            let _ = heartbeat_tx.send(());
        });
        let watchdog = std::thread::spawn(move || {
            entered_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("blocking ready work started");
            let executor_remained_live = heartbeat_rx
                .recv_timeout(Duration::from_millis(250))
                .is_ok();
            release_tx.send(()).expect("release blocking ready work");
            executor_remained_live
        });

        let value = spawn_ready_off_executor(move || {
            entered_tx.send(()).expect("announce blocking ready work");
            release_rx.recv().expect("wait until liveness was observed");
            Ok::<_, String>(42)
        })
        .await
        .expect("blocking start result");

        assert_eq!(value, 42);
        assert!(
            watchdog.join().expect("join liveness watchdog"),
            "the async executor must schedule a heartbeat while spawn_ready waits"
        );
    }

    #[test]
    fn project_open_failure_does_not_advance_epoch_or_stop_playback() {
        use opentake_core::CoreEvent;

        let core = AppCore::new();
        let before = core.project_revision();
        let current = identity(before.project_epoch, before.version, "current-session");
        let (playback, publication) = state_with_running(current.clone());
        let events = Arc::new(Mutex::new(Vec::<CoreEvent>::new()));
        let received = Arc::clone(&events);
        core.subscribe(move |event| {
            received
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(event.clone());
        });
        let missing = std::env::temp_dir()
            .join(format!(
                "opentake-missing-review-fix-{}.opentake",
                std::process::id()
            ))
            .to_string_lossy()
            .into_owned();

        let result = crate::commands::project_open_with_playback(&core, missing, &playback);

        assert!(result.is_err());
        assert_eq!(core.project_revision(), before);
        assert_eq!(playback.active_identity(), Some(current.clone()));
        assert!(
            publication.is_open(),
            "failed open must restore publication"
        );
        playback
            .control(current, SessionControl::Seek, 4)
            .expect("failed open must retain playback resources");
        assert!(events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .all(|event| !matches!(event, CoreEvent::ProjectOpened { .. })));
    }

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
