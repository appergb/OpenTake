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

use super::audio::{build_clock_paused_cancellable, AudioPlayback, AudioPrepareWorker};
use super::engine::{
    BoundedReaper, FrameSink, PlaybackClock, PlaybackEngine, PlayheadEmitter, ReapPermit,
};
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
    server: Option<Arc<PreviewServer>>,
    reap: ReapPermit,
}

type PreparedAudio = Result<(Arc<dyn PlaybackClock>, Option<AudioPlayback>), String>;

struct PlaybackResources {
    engine: PlaybackEngine,
    audio: Option<AudioPlayback>,
    publication: PublicationGate,
    server: Arc<PreviewServer>,
}

impl RunningPlayback {
    fn close_publication(&self) {
        self.publication.close();
    }

    fn shutdown(self) -> Result<(), PlaybackCommandError> {
        self.publication.close();
        if let Some(server) = self.server.as_ref() {
            server.clear_session(&self.identity);
        }
        if let Some(audio) = self.audio.as_ref() {
            audio.mute();
        }
        let mut handles = Vec::with_capacity(2);
        if let Some(handle) = self.audio.and_then(|audio| audio.request_stop()) {
            handles.push(handle);
        }
        if let Some(handle) = self.engine.request_stop() {
            handles.push(handle);
        }
        self.reap
            .enqueue(handles)
            .map_err(PlaybackCommandError::engine)
    }
}

#[derive(Default)]
struct PlaybackSlot {
    sessions: SessionRegistry,
    running: Option<RunningPlayback>,
    prepare_cancel: Option<opentake_media::MediaCancelToken>,
}

pub struct PlaybackState {
    slot: Mutex<PlaybackSlot>,
    audio_prepare: AudioPrepareWorker<PreparedAudio>,
    reaper: BoundedReaper,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            slot: Mutex::new(PlaybackSlot::default()),
            audio_prepare: AudioPrepareWorker::new(),
            reaper: BoundedReaper::new(),
        }
    }
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
    ) -> Result<Option<(StartTicket, ReapPermit)>, PlaybackCommandError> {
        let (decision, old, pending_reap) = {
            let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(cancel) = slot.prepare_cancel.take() {
                cancel.cancel();
            }
            let pending_reap = self
                .reaper
                .try_reserve()
                .map_err(PlaybackCommandError::busy)?;
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
                drop(pending_reap);
                return Ok(None);
            }
            let old = slot.running.take();
            (decision, old, pending_reap)
        };
        if let Some(running) = old {
            running.shutdown()?;
        }
        let StartDecision::Build(ticket) = decision else {
            unreachable!("resume returned above")
        };
        Ok(Some((ticket, pending_reap)))
    }

    fn install_if_current(
        &self,
        ticket: StartTicket,
        cleanup: ReapPermit,
        authoritative: opentake_core::ProjectRevision,
        resources: PlaybackResources,
        frame: i32,
    ) -> Result<(), PlaybackCommandError> {
        let identity = ticket.identity().clone();
        let PlaybackResources {
            engine,
            audio,
            publication,
            server,
        } = resources;
        let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(error) = slot.sessions.install_if_current(ticket, authoritative) {
            drop(slot);
            publication.close();
            let running = RunningPlayback {
                identity,
                engine,
                audio,
                publication,
                server: Some(server),
                reap: cleanup,
            };
            running.shutdown()?;
            return Err(error);
        }
        if let Err(error) = engine.resume(frame) {
            slot.sessions.control(&identity, SessionControl::Stop);
            drop(slot);
            let running = RunningPlayback {
                identity,
                engine,
                audio,
                publication,
                server: Some(server),
                reap: cleanup,
            };
            running.shutdown()?;
            return Err(PlaybackCommandError::engine(error));
        }
        if let Some(audio_playback) = audio.as_ref() {
            if let Err(error) = audio_playback.resume() {
                slot.sessions.control(&identity, SessionControl::Stop);
                drop(slot);
                let running = RunningPlayback {
                    identity,
                    engine,
                    audio,
                    publication,
                    server: Some(server),
                    reap: cleanup,
                };
                running.shutdown()?;
                return Err(PlaybackCommandError::engine(error));
            }
        }
        slot.running = Some(RunningPlayback {
            identity,
            engine,
            audio,
            publication,
            server: Some(server),
            reap: cleanup,
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
                    running.shutdown()?;
                }
            }
        }
        Ok(())
    }

    pub fn begin_project_transition(&self) -> Result<ProjectTransition, PlaybackCommandError> {
        let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
        let transition = slot.sessions.begin_project_transition()?;
        if let Some(running) = slot.running.as_ref() {
            running.close_publication();
            if let Some(server) = running.server.as_ref() {
                server.clear_session(&running.identity);
            }
        }
        Ok(transition)
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
            let _ = running.shutdown();
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
            let _ = running.shutdown();
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
            let _ = running.shutdown();
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

fn audio_prepare_error(message: String) -> PlaybackCommandError {
    if message.contains("audio_buffer_too_large") {
        PlaybackCommandError::audio_buffer_too_large(message)
    } else if message.contains("audio_allocation_failed") {
        PlaybackCommandError::allocation(message)
    } else {
        PlaybackCommandError::engine(message)
    }
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
    let Some((ticket, cleanup)) =
        app.state::<PlaybackState>()
            .prepare_start(identity.clone(), authoritative, start_at)?
    else {
        return Ok(());
    };

    // Snapshot the session synchronously — no managed-state guard is held across
    // the await below (Tauri async commands require a Send future).
    let (timeline, sizes, media, text, render_size, fps, sink, emitter, publication, server) = {
        let timeline = snapshot.timeline;
        let manifest = snapshot.media;
        let project_dir = snapshot.project_dir;
        let (sizes, media) = project_media(&manifest, &project_dir);
        let text = project_text(&timeline);
        let render_size =
            playback_render_size(timeline.width, timeline.height, PLAYBACK_PREVIEW_CAP);
        let fps = timeline.fps;
        let server = app.state::<Arc<PreviewServer>>().inner().clone();
        let publication = PublicationGate::open();
        let concrete_sink = server.sink(identity.clone(), publication.clone());
        let emitter: Arc<dyn PlayheadEmitter> = Arc::new(TauriPlayheadEmitter::new(
            app.clone(),
            &concrete_sink,
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
            server,
        )
    };

    // Decoding + mixing the whole timeline's audio (ffmpeg per clip) can take
    // seconds on a long project; run it (and cpal setup) off the IPC thread so
    // the command never freezes the UI.
    let cancel = opentake_media::MediaCancelToken::new();
    {
        let state = app.state::<PlaybackState>();
        state
            .slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .prepare_cancel = Some(cancel.clone());
    }
    let receiver = {
        let timeline = timeline.clone();
        let media = media.clone();
        let prepare_cancel = cancel.clone();
        app.state::<PlaybackState>()
            .audio_prepare
            .try_submit(move || {
                build_clock_paused_cancellable(&timeline, &media, fps, start_at, &prepare_cancel)
            })
            .map_err(PlaybackCommandError::busy)?
    };
    let (clock, audio) = receiver
        .await
        .map_err(|_| PlaybackCommandError::engine("audio prepare worker stopped"))?
        .map_err(audio_prepare_error)?;

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
        cleanup,
        current,
        PlaybackResources {
            engine,
            audio,
            publication,
            server,
        },
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
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

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
                server: None,
                reap: state
                    .reaper
                    .try_reserve()
                    .expect("reserve test session reap"),
            });
        }
        (state, gate)
    }

    fn install_server_on_running(state: &PlaybackState, server: Arc<PreviewServer>) {
        state
            .slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .running
            .as_mut()
            .expect("running playback")
            .server = Some(server);
    }

    fn publish_test_frame(
        server: &PreviewServer,
        identity: PlaybackIdentity,
        gate: PublicationGate,
        frame: i32,
    ) {
        let sink = server.sink(identity, gate);
        let publication = sink.publication();
        sink.push_frame(&opentake_render::DecodedFrame::new(
            1,
            1,
            vec![255, 0, 0, 255],
            false,
        ));
        assert!(publication.commit(frame, frame).is_some());
    }

    fn frame_status(server: &PreviewServer, identity: &PlaybackIdentity, frame: i32) -> u16 {
        let endpoint = server.endpoint_frame();
        let address = endpoint
            .strip_prefix("http://")
            .and_then(|rest| rest.strip_suffix("/frame"))
            .expect("loopback frame endpoint");
        let mut stream = TcpStream::connect(address).expect("connect preview server");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set preview read timeout");
        write!(
            stream,
            "GET /frame?projectEpoch={}&timelineVersion={}&sessionId={}&frame={frame}&sequence=1 HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n",
            identity.project_epoch, identity.timeline_version, identity.session_id
        )
        .expect("request exact frame");
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .expect("read exact frame response");
        String::from_utf8_lossy(&response)
            .split_whitespace()
            .nth(1)
            .expect("HTTP status")
            .parse()
            .expect("numeric HTTP status")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_closes_publication_and_mutes_audio_before_any_reap() {
        let identity = identity(4, 2, "shutdown-order");
        let server = PreviewServer::start().await.expect("start preview server");
        let (state, gate) = state_with_running(identity.clone());
        install_server_on_running(&state, Arc::clone(&server));
        publish_test_frame(&server, identity.clone(), gate.clone(), 0);
        assert_eq!(frame_status(&server, &identity, 0), 200);
        let (audio, muted, audio_stop) = super::super::audio::AudioPlayback::test_stub();
        let (engine, engine_stop) = PlaybackEngine::test_stub();
        {
            let mut slot = state
                .slot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let running = slot.running.as_mut().expect("running playback");
            running.audio = Some(audio);
            running.engine = engine;
        }

        state
            .control(identity.clone(), SessionControl::Stop, 0)
            .expect("stop queues teardown");

        assert!(!gate.is_open());
        assert!(muted.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(frame_status(&server, &identity, 0), 204);
        audio_stop
            .recv_timeout(Duration::from_secs(2))
            .expect("audio stop requested before command returns");
        engine_stop
            .recv_timeout(Duration::from_secs(2))
            .expect("render stop requested before command returns");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn late_inflight_render_cannot_publish_after_project_boundary_returns() {
        let identity = identity(5, 7, "late-render");
        let server = PreviewServer::start().await.expect("start preview server");
        let (state, gate) = state_with_running(identity.clone());
        install_server_on_running(&state, Arc::clone(&server));
        publish_test_frame(&server, identity.clone(), gate.clone(), 0);
        assert_eq!(frame_status(&server, &identity, 0), 200);

        let _transition = state
            .begin_project_transition()
            .expect("begin project boundary");
        let sink = server.sink(identity.clone(), gate);
        let publication = sink.publication();
        let late = std::thread::spawn(move || {
            sink.push_frame(&opentake_render::DecodedFrame::new(
                1,
                1,
                vec![0, 255, 0, 255],
                false,
            ));
            publication.commit(1, 1)
        });

        assert!(late.join().expect("join late render").is_none());
        assert_eq!(frame_status(&server, &identity, 0), 204);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn clearing_old_session_latest_cannot_clear_replacement_session_frame() {
        let old = identity(8, 3, "old-session");
        let replacement = identity(8, 3, "replacement-session");
        let server = PreviewServer::start().await.expect("start preview server");
        publish_test_frame(&server, old.clone(), PublicationGate::open(), 0);
        publish_test_frame(&server, replacement.clone(), PublicationGate::open(), 0);

        server.clear_session(&old);

        assert_eq!(frame_status(&server, &old, 0), 204);
        assert_eq!(frame_status(&server, &replacement, 0), 200);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn external_project_event_invalidates_while_reaper_capacity_is_full() {
        let old = identity(12, 4, "external-full-old");
        let replacement = identity(13, 0, "external-full-new");
        let server = PreviewServer::start().await.expect("start preview server");
        let (state, gate) = state_with_running(old.clone());
        install_server_on_running(&state, Arc::clone(&server));
        let sink = server.sink(old.clone(), gate.clone());
        let publication = sink.publication();

        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let backlog = state
            .reaper
            .try_reserve()
            .expect("second and final reaper slot");
        backlog
            .enqueue(vec![std::thread::spawn(move || {
                release_rx.recv().expect("release teardown backlog");
            })])
            .expect("enqueue teardown backlog");

        let invalidated = state.activate_project_event(13);

        assert_eq!(invalidated, Some(old.clone()));
        assert!(!gate.is_open());
        assert!(publication.commit(1, 1).is_none());
        let error = state
            .control(old, SessionControl::Seek, 1)
            .expect_err("external boundary invalidates old identity synchronously");
        assert_eq!(
            error.code,
            super::super::session::PlaybackErrorCode::Superseded
        );
        let busy = match state.prepare_start(replacement.clone(), replacement.revision(), 0) {
            Err(error) => error,
            Ok(_) => panic!("new start stays busy until teardown capacity recovers"),
        };
        assert_eq!(busy.code, super::super::session::PlaybackErrorCode::Busy);

        release_tx.send(()).expect("release teardown backlog");
        state.reaper.wait_until_idle(Duration::from_secs(2));
        let pending = state
            .prepare_start(replacement.clone(), replacement.revision(), 0)
            .expect("capacity recovered")
            .expect("replacement build admitted");
        drop(pending);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn external_timeline_invalidation_uses_reserved_reap_slot_when_backlog_is_full() {
        let old = identity(21, 6, "timeline-full-old");
        let replacement = identity(21, 7, "timeline-full-new");
        let server = PreviewServer::start().await.expect("start preview server");
        let (state, gate) = state_with_running(old.clone());
        install_server_on_running(&state, Arc::clone(&server));
        let sink = server.sink(old.clone(), gate.clone());
        let publication = sink.publication();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        state
            .reaper
            .try_reserve()
            .expect("second and final reaper slot")
            .enqueue(vec![std::thread::spawn(move || {
                release_rx.recv().expect("release teardown backlog");
            })])
            .expect("enqueue teardown backlog");

        let invalidated = state.invalidate_timeline(21, 7);

        assert_eq!(invalidated, Some(old.clone()));
        assert!(!gate.is_open());
        assert!(publication.commit(1, 1).is_none());
        let error = state
            .control(old, SessionControl::Pause, 1)
            .expect_err("timeline invalidation supersedes old control immediately");
        assert_eq!(
            error.code,
            super::super::session::PlaybackErrorCode::Superseded
        );
        let busy = match state.prepare_start(replacement.clone(), replacement.revision(), 0) {
            Err(error) => error,
            Ok(_) => panic!("new start stays busy until timeline teardown capacity recovers"),
        };
        assert_eq!(busy.code, super::super::session::PlaybackErrorCode::Busy);

        release_tx.send(()).expect("release teardown backlog");
        state.reaper.wait_until_idle(Duration::from_secs(2));
        let pending = state
            .prepare_start(replacement.clone(), replacement.revision(), 0)
            .expect("capacity recovered")
            .expect("replacement build admitted");
        drop(pending);
    }

    #[test]
    fn overlapping_project_transition_is_busy_and_preserves_owner_resources() {
        let current = identity(8, 3, "current-session");
        let (state, publication) = state_with_running(current.clone());
        let first = state
            .begin_project_transition()
            .expect("begin owning project transition");
        let second = state
            .begin_project_transition()
            .expect_err("overlapping project transition must report busy");

        assert_eq!(second.code, super::super::session::PlaybackErrorCode::Busy);

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
    fn overlapping_project_open_is_rejected_before_core_or_playback_mutation() {
        let core = AppCore::new();
        let before = core.project_revision();
        let current = identity(before.project_epoch, before.version, "overlap-session");
        let (playback, publication) = state_with_running(current.clone());
        let first = playback
            .begin_project_transition()
            .expect("begin owning project transition");
        let missing = std::env::temp_dir()
            .join(format!(
                "opentake-overlap-must-not-open-{}.opentake",
                std::process::id()
            ))
            .to_string_lossy()
            .into_owned();

        let error = crate::commands::project_open_with_playback(&core, missing, &playback)
            .expect_err("a second project boundary must report busy");
        let error = serde_json::to_value(error).expect("serialize command error");

        assert_eq!(
            error.get("code").and_then(serde_json::Value::as_str),
            Some("busy")
        );
        assert_eq!(core.project_revision(), before);
        assert_eq!(playback.active_identity(), Some(current.clone()));
        assert!(
            !publication.is_open(),
            "the first transition must retain publication ownership"
        );

        playback.cancel_project_transition(first);
        assert!(publication.is_open());
        playback
            .control(current, SessionControl::Stop, 0)
            .expect("stop retained test session");
    }

    #[test]
    fn project_event_interleave_keeps_core_event_and_playback_on_one_boundary() {
        use opentake_core::CoreEvent;

        let core = AppCore::new();
        let before = core.project_revision();
        let current = identity(before.project_epoch, before.version, "interleave-session");
        let (playback, publication) = state_with_running(current.clone());
        let playback = Arc::new(playback);
        let observations = Arc::new(Mutex::new(Vec::new()));
        let callback_core = core.clone();
        let callback_playback = Arc::clone(&playback);
        let callback_observations = Arc::clone(&observations);
        core.subscribe(move |event| {
            let CoreEvent::ProjectOpened { project_epoch, .. } = event else {
                return;
            };
            let invalidated = callback_playback.activate_project_event(*project_epoch);
            let new_error =
                crate::commands::project_new_with_playback(&callback_core, &callback_playback)
                    .expect_err("nested project_new must report busy");
            let open_error = crate::commands::project_open_with_playback(
                &callback_core,
                "must-not-reach-core.opentake".to_owned(),
                &callback_playback,
            )
            .expect_err("nested project_open must report busy");
            callback_observations
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push((
                    *project_epoch,
                    invalidated,
                    new_error.code,
                    open_error.code,
                    callback_core.project_revision(),
                ));
        });

        let snapshot = crate::commands::project_new_with_playback(&core, &playback)
            .expect("owning project boundary succeeds");
        let observed = observations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].0, snapshot.project_epoch);
        assert_eq!(observed[0].1, None);
        assert_eq!(
            observed[0].2,
            super::super::session::PlaybackErrorCode::Busy
        );
        assert_eq!(
            observed[0].3,
            super::super::session::PlaybackErrorCode::Busy
        );
        assert_eq!(observed[0].4, core.project_revision());
        assert_eq!(snapshot.project_epoch, before.project_epoch + 1);
        assert_eq!(
            core.project_revision().project_epoch,
            snapshot.project_epoch
        );
        assert_eq!(playback.active_identity(), None);
        assert!(
            !publication.is_open(),
            "the old session cannot publish after the committed boundary"
        );
        let error = playback
            .control(current, SessionControl::Seek, 1)
            .expect_err("the old playback identity must be superseded");
        assert_eq!(
            error.code,
            super::super::session::PlaybackErrorCode::Superseded
        );
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
