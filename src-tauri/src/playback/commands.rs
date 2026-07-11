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

use super::audio::{
    build_clock_paused_cancellable, AudioPlayback, AudioPreparePermit, AudioPrepareWorker,
};
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

type PreparedAudio =
    Result<(Arc<dyn PlaybackClock>, Option<AudioPlayback>), opentake_media::MediaError>;

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
    prepare: Option<PendingPrepare>,
}

struct PendingPrepare {
    identity: PlaybackIdentity,
    cancel: opentake_media::MediaCancelToken,
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

    pub fn ensure_project_transition_available(&self) -> Result<(), PlaybackCommandError> {
        self.slot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sessions
            .ensure_project_transition_available()
    }

    fn coordinate_start(
        &self,
        identity: PlaybackIdentity,
        authoritative: opentake_core::ProjectRevision,
        frame: i32,
        cancel: opentake_media::MediaCancelToken,
    ) -> Result<
        Option<(StartTicket, ReapPermit, AudioPreparePermit<PreparedAudio>)>,
        PlaybackCommandError,
    > {
        let (decision, old, pending_reap, audio_admission) = {
            let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
            if slot.sessions.start_would_resume(&identity, authoritative)? {
                let decision = slot.sessions.begin_start(identity.clone(), authoritative)?;
                debug_assert!(matches!(decision, StartDecision::Resume));
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
            let audio_admission = self
                .audio_prepare
                .try_reserve()
                .map_err(PlaybackCommandError::busy)?;
            let pending_reap = self
                .reaper
                .try_reserve()
                .map_err(PlaybackCommandError::busy)?;
            let decision = slot.sessions.begin_start(identity.clone(), authoritative)?;
            let StartDecision::Build(_) = decision else {
                unreachable!("resume handled before reaper admission")
            };
            if let Some(incumbent) = slot.prepare.replace(PendingPrepare {
                identity: identity.clone(),
                cancel,
            }) {
                incumbent.cancel.cancel();
            }
            let old = slot.running.take();
            (decision, old, pending_reap, audio_admission)
        };
        if let Some(running) = old {
            running.shutdown()?;
        }
        let StartDecision::Build(ticket) = decision else {
            unreachable!("resume returned above")
        };
        Ok(Some((ticket, pending_reap, audio_admission)))
    }

    #[cfg(test)]
    fn prepare_start(
        &self,
        identity: PlaybackIdentity,
        authoritative: opentake_core::ProjectRevision,
        frame: i32,
    ) -> Result<
        Option<(StartTicket, ReapPermit, AudioPreparePermit<PreparedAudio>)>,
        PlaybackCommandError,
    > {
        self.coordinate_start(
            identity,
            authoritative,
            frame,
            opentake_media::MediaCancelToken::new(),
        )
    }

    fn finish_prepare(&self, cancel: &opentake_media::MediaCancelToken) {
        let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
        Self::finish_prepare_locked(&mut slot, cancel);
    }

    fn finish_prepare_locked(slot: &mut PlaybackSlot, cancel: &opentake_media::MediaCancelToken) {
        if slot
            .prepare
            .as_ref()
            .is_some_and(|pending| pending.cancel.same_instance(cancel))
        {
            slot.prepare = None;
        }
    }

    fn cancel_prepare(slot: &mut PlaybackSlot) {
        if let Some(pending) = slot.prepare.take() {
            pending.cancel.cancel();
        }
    }

    #[cfg(test)]
    fn pending_prepare_identity(&self) -> Option<PlaybackIdentity> {
        self.slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .prepare
            .as_ref()
            .map(|pending| pending.identity.clone())
    }

    fn install_if_current(
        &self,
        ticket: StartTicket,
        cleanup: ReapPermit,
        authoritative: opentake_core::ProjectRevision,
        resources: PlaybackResources,
        frame: i32,
        prepare_cancel: &opentake_media::MediaCancelToken,
    ) -> Result<(), PlaybackCommandError> {
        let identity = ticket.identity().clone();
        let PlaybackResources {
            engine,
            audio,
            publication,
            server,
        } = resources;
        let mut slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
        Self::finish_prepare_locked(&mut slot, prepare_cancel);
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
        if control == SessionControl::Stop
            && slot
                .prepare
                .as_ref()
                .is_some_and(|pending| pending.identity == identity)
        {
            slot.sessions.stop_all();
            Self::cancel_prepare(&mut slot);
            let running = slot.running.take();
            drop(slot);
            if let Some(running) = running {
                running.shutdown()?;
            }
            return Ok(());
        }
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
        Self::cancel_prepare(&mut slot);
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
                Self::cancel_prepare(&mut slot);
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
            let pending_invalid = slot.prepare.as_ref().is_some_and(|pending| {
                pending.identity.project_epoch == project_epoch
                    && pending.identity.timeline_version != version
            });
            let running_invalid = slot
                .sessions
                .invalidate_for_timeline_change(project_epoch, version);
            if pending_invalid {
                slot.sessions.stop_all();
                Self::cancel_prepare(&mut slot);
            }
            if running_invalid {
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

fn audio_prepare_error(error: opentake_media::MediaError) -> PlaybackCommandError {
    let message = error.to_string();
    match error {
        opentake_media::MediaError::Cancelled => PlaybackCommandError::cancelled(message),
        _ => PlaybackCommandError::engine(message),
    }
}

fn cleanup_audio_after_engine_ready_failure(
    identity: PlaybackIdentity,
    audio: Option<AudioPlayback>,
    publication: PublicationGate,
    server: Arc<PreviewServer>,
    cleanup: ReapPermit,
) -> Result<(), PlaybackCommandError> {
    publication.close();
    server.clear_session(&identity);
    if let Some(audio) = audio.as_ref() {
        audio.mute();
    }
    let handles = audio
        .and_then(AudioPlayback::request_stop)
        .into_iter()
        .collect();
    cleanup
        .enqueue(handles)
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
    let cancel = opentake_media::MediaCancelToken::new();
    let Some((ticket, cleanup, audio_admission)) = app.state::<PlaybackState>().coordinate_start(
        identity.clone(),
        authoritative,
        start_at,
        cancel.clone(),
    )?
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
    let receiver = {
        let timeline = timeline.clone();
        let media = media.clone();
        let prepare_cancel = cancel.clone();
        match audio_admission.submit(move || {
            build_clock_paused_cancellable(&timeline, &media, fps, start_at, &prepare_cancel)
        }) {
            Ok(receiver) => receiver,
            Err(error) => {
                app.state::<PlaybackState>().finish_prepare(&cancel);
                return Err(PlaybackCommandError::busy(error));
            }
        }
    };
    let prepared = match receiver.await {
        Ok(prepared) => prepared,
        Err(_) => {
            app.state::<PlaybackState>().finish_prepare(&cancel);
            return Err(PlaybackCommandError::engine("audio prepare worker stopped"));
        }
    };
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            app.state::<PlaybackState>().finish_prepare(&cancel);
            return Err(PlaybackCommandError::engine(error));
        }
    };
    let (clock, audio) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            app.state::<PlaybackState>().finish_prepare(&cancel);
            return Err(audio_prepare_error(error));
        }
    };

    let ready_cancel = cancel.clone();
    let engine = match spawn_ready_off_executor(move || {
        PlaybackEngine::spawn_ready_cancellable(
            timeline,
            media,
            text,
            sizes,
            render_size,
            clock,
            sink,
            emitter,
            start_at,
            ready_cancel,
        )
    })
    .await
    {
        Ok(engine) => engine,
        Err(mut error) => {
            if cancel.is_cancelled() {
                error = PlaybackCommandError::cancelled(error.message);
            }
            app.state::<PlaybackState>().finish_prepare(&cancel);
            cleanup_audio_after_engine_ready_failure(
                identity,
                audio,
                publication,
                server,
                cleanup,
            )?;
            return Err(error);
        }
    };
    let current = app.state::<AppCore>().project_revision();
    let result = app.state::<PlaybackState>().install_if_current(
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
        &cancel,
    );
    result
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

    #[test]
    fn rejected_start_does_not_cancel_incumbent_prepare() {
        let state = PlaybackState::new();
        let incumbent_identity = identity(1, 4, "incumbent-prepare");
        let incumbent_cancel = opentake_media::MediaCancelToken::new();
        let (incumbent_ticket, incumbent_cleanup, incumbent_admission) = state
            .coordinate_start(
                incumbent_identity.clone(),
                incumbent_identity.revision(),
                0,
                incumbent_cancel.clone(),
            )
            .expect("incumbent prepare adopted")
            .expect("incumbent builds");
        drop(incumbent_admission);

        let rejected_cancel = opentake_media::MediaCancelToken::new();
        let error = match state.coordinate_start(
            identity(1, 3, "stale-prepare"),
            incumbent_identity.revision(),
            0,
            rejected_cancel.clone(),
        ) {
            Err(error) => error,
            Ok(_) => panic!("stale start must be rejected"),
        };

        assert_eq!(
            error.code,
            super::super::session::PlaybackErrorCode::Superseded
        );
        assert!(!incumbent_cancel.is_cancelled());
        assert!(!rejected_cancel.is_cancelled());
        assert_eq!(state.pending_prepare_identity(), Some(incumbent_identity));
        drop((incumbent_ticket, incumbent_cleanup));
    }

    #[test]
    fn stop_cancels_matching_pending_readiness_before_install() {
        let state = PlaybackState::new();
        let pending_identity = identity(1, 5, "pending-readiness");
        let cancel = opentake_media::MediaCancelToken::new();
        let (_ticket, _cleanup, admission) = state
            .coordinate_start(
                pending_identity.clone(),
                pending_identity.revision(),
                0,
                cancel.clone(),
            )
            .expect("pending readiness is coordinated")
            .expect("fresh session builds");
        drop(admission);

        state
            .control(pending_identity, SessionControl::Stop, 0)
            .expect("matching stop cancels pending readiness");

        assert!(cancel.is_cancelled());
        assert_eq!(state.pending_prepare_identity(), None);
    }

    #[test]
    fn stop_shuts_down_running_if_pending_and_installed_ownership_overlap() {
        let current = identity(1, 6, "readiness-handoff");
        let (state, publication) = state_with_running(current.clone());
        let cancel = opentake_media::MediaCancelToken::new();
        {
            let mut slot = state
                .slot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            slot.prepare = Some(PendingPrepare {
                identity: current.clone(),
                cancel: cancel.clone(),
            });
        }

        state
            .control(current, SessionControl::Stop, 0)
            .expect("matching stop closes every handoff owner");

        assert!(cancel.is_cancelled());
        assert!(!publication.is_open());
        let slot = state
            .slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(slot.prepare.is_none());
        assert!(slot.running.is_none());
    }

    #[test]
    fn reaper_busy_start_does_not_cancel_incumbent_prepare() {
        let state = PlaybackState::new();
        let incumbent_identity = identity(2, 1, "incumbent-busy");
        let incumbent_cancel = opentake_media::MediaCancelToken::new();
        let (incumbent_ticket, incumbent_cleanup, incumbent_admission) = state
            .coordinate_start(
                incumbent_identity.clone(),
                incumbent_identity.revision(),
                0,
                incumbent_cancel.clone(),
            )
            .expect("incumbent prepare adopted")
            .expect("incumbent builds");
        drop(incumbent_admission);
        let backlog = state
            .reaper
            .try_reserve()
            .expect("occupy final reaper slot");

        let error = match state.coordinate_start(
            identity(2, 1, "rejected-busy"),
            incumbent_identity.revision(),
            0,
            opentake_media::MediaCancelToken::new(),
        ) {
            Err(error) => error,
            Ok(_) => panic!("reaper-saturated start must be Busy"),
        };

        assert_eq!(error.code, super::super::session::PlaybackErrorCode::Busy);
        assert!(!incumbent_cancel.is_cancelled());
        assert_eq!(state.pending_prepare_identity(), Some(incumbent_identity));
        drop(backlog);
        drop((incumbent_ticket, incumbent_cleanup));
    }

    #[test]
    fn audio_worker_busy_is_decided_before_incumbent_adoption_or_cancellation() {
        let state = PlaybackState::new();
        let incumbent_identity = identity(2, 2, "incumbent-worker");
        let incumbent_cancel = opentake_media::MediaCancelToken::new();
        let (_ticket, _cleanup, admission) = state
            .coordinate_start(
                incumbent_identity.clone(),
                incumbent_identity.revision(),
                0,
                incumbent_cancel.clone(),
            )
            .expect("incumbent coordinator succeeds")
            .expect("incumbent builds");
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let result = admission
            .submit(move || {
                entered_tx.send(()).expect("announce incumbent entry");
                release_rx.recv().expect("release incumbent closure");
                Ok((
                    Arc::new(InstantClock::new(0)) as Arc<dyn PlaybackClock>,
                    None,
                ))
            })
            .expect("submit through reserved production admission");
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("incumbent closure entered");

        let rejected_cancel = opentake_media::MediaCancelToken::new();
        let error = match state.coordinate_start(
            identity(2, 2, "worker-busy-request"),
            incumbent_identity.revision(),
            0,
            rejected_cancel.clone(),
        ) {
            Err(error) => error,
            Ok(_) => panic!("occupied production worker must reject B as Busy"),
        };

        assert_eq!(error.code, super::super::session::PlaybackErrorCode::Busy);
        assert!(!incumbent_cancel.is_cancelled());
        assert!(!rejected_cancel.is_cancelled());
        assert_eq!(state.pending_prepare_identity(), Some(incumbent_identity));

        release_tx.send(()).expect("release incumbent closure");
        result
            .blocking_recv()
            .expect("incumbent worker result channel")
            .expect("incumbent job succeeds")
            .expect("incumbent preparation succeeds");
    }

    #[test]
    fn typed_media_cancellation_maps_to_cancelled_command_error() {
        let error = audio_prepare_error(opentake_media::MediaError::Cancelled);
        let serialized = serde_json::to_value(&error).expect("serialize cancellation error");

        assert_eq!(
            error.code,
            super::super::session::PlaybackErrorCode::Cancelled
        );
        assert_eq!(error.message, "cancelled");
        assert_eq!(
            serialized.get("code").and_then(serde_json::Value::as_str),
            Some("cancelled")
        );
    }

    #[test]
    fn bounded_audio_errors_keep_sentinels_under_the_four_code_contract() {
        for sentinel in [
            "audio_buffer_too_large: projected peak",
            "audio_allocation_failed: interleaved reserve",
        ] {
            let error =
                audio_prepare_error(opentake_media::MediaError::Decode(sentinel.to_string()));
            let serialized = serde_json::to_value(&error).expect("serialize command error");

            assert_eq!(error.code, super::super::session::PlaybackErrorCode::Engine);
            assert!(error.message.contains(sentinel));
            assert_eq!(
                serialized.get("code").and_then(serde_json::Value::as_str),
                Some("engine")
            );
            assert_eq!(
                serialized
                    .get("message")
                    .and_then(serde_json::Value::as_str),
                Some(error.message.as_str())
            );
        }
    }

    #[test]
    fn newer_adopted_start_owns_pending_token_without_late_overwrite() {
        let state = PlaybackState::new();
        let old_identity = identity(3, 2, "older-caller");
        let old_cancel = opentake_media::MediaCancelToken::new();
        let old = state
            .coordinate_start(
                old_identity.clone(),
                old_identity.revision(),
                0,
                old_cancel.clone(),
            )
            .expect("old start adopted")
            .expect("old start builds");
        drop(old);

        let new_identity = identity(3, 2, "newer-caller");
        let new_cancel = opentake_media::MediaCancelToken::new();
        let new = state
            .coordinate_start(
                new_identity.clone(),
                new_identity.revision(),
                0,
                new_cancel.clone(),
            )
            .expect("new start adopted")
            .expect("new start builds");

        assert!(old_cancel.is_cancelled());
        assert!(!new_cancel.is_cancelled());
        assert_eq!(state.pending_prepare_identity(), Some(new_identity));
        drop(new);
    }

    #[test]
    fn retained_paused_resume_succeeds_when_reaper_slots_are_occupied() {
        let current = identity(4, 8, "retained-resume");
        let (state, _gate) = state_with_running(current.clone());
        let (engine, _stopped) = PlaybackEngine::test_stub();
        {
            let mut slot = state
                .slot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            assert!(slot.sessions.control(&current, SessionControl::Pause));
            slot.running.as_mut().expect("running playback").engine = engine;
        }
        let backlog = state
            .reaper
            .try_reserve()
            .expect("occupy second reaper slot");

        let result = state
            .coordinate_start(
                current.clone(),
                current.revision(),
                17,
                opentake_media::MediaCancelToken::new(),
            )
            .expect("retained resume bypasses reaper admission");

        assert!(result.is_none());
        assert_eq!(state.active_identity(), Some(current));
        drop(backlog);
    }

    #[test]
    fn project_and_timeline_boundaries_cancel_inflight_prepare_atomically() {
        let cases = ["transition", "project-event", "timeline-change"];
        for (index, boundary) in cases.into_iter().enumerate() {
            let state = PlaybackState::new();
            let current = identity(10 + index as u64, 5, &format!("pending-{boundary}"));
            let cancel = opentake_media::MediaCancelToken::new();
            let pending = state
                .coordinate_start(current.clone(), current.revision(), 0, cancel.clone())
                .expect("pending prepare adopted")
                .expect("pending prepare builds");

            match boundary {
                "transition" => {
                    let _ = state
                        .begin_project_transition()
                        .expect("begin project transition");
                }
                "project-event" => {
                    assert_eq!(
                        state.activate_project_event(current.project_epoch + 1),
                        None
                    );
                }
                "timeline-change" => {
                    assert_eq!(
                        state.invalidate_timeline(
                            current.project_epoch,
                            current.timeline_version + 1
                        ),
                        None
                    );
                }
                _ => unreachable!(),
            }

            assert!(
                cancel.is_cancelled(),
                "{boundary} must cancel pending prepare"
            );
            assert_eq!(state.pending_prepare_identity(), None);
            drop(pending);
        }
    }

    #[test]
    fn boundary_cancel_releases_worker_capacity_only_after_prepare_exits() {
        let state = PlaybackState::new();
        let current = identity(30, 2, "boundary-worker");
        let cancel = opentake_media::MediaCancelToken::new();
        let (_ticket, _cleanup, admission) = state
            .coordinate_start(current.clone(), current.revision(), 0, cancel.clone())
            .expect("pending prepare adopted")
            .expect("pending prepare builds");
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker_cancel = cancel.clone();
        let result = admission
            .submit(move || {
                entered_tx.send(()).expect("announce prepare entry");
                while !worker_cancel.is_cancelled() {
                    std::thread::yield_now();
                }
                release_rx.recv().expect("release cancelled prepare");
                Err(opentake_media::MediaError::Cancelled)
            })
            .expect("submit production audio prepare");
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("prepare worker entered");

        let _ = state
            .begin_project_transition()
            .expect("project boundary cancels prepare");

        assert!(cancel.is_cancelled());
        assert!(state.audio_prepare.is_occupied());
        assert!(state
            .audio_prepare
            .try_submit(|| Ok((
                Arc::new(InstantClock::new(0)) as Arc<dyn PlaybackClock>,
                None,
            )))
            .is_err());
        release_tx.send(()).expect("release cancelled prepare");
        assert!(matches!(
            result
                .blocking_recv()
                .expect("prepare result channel")
                .expect("prepare job returns"),
            Err(opentake_media::MediaError::Cancelled)
        ));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while state.audio_prepare.is_occupied() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(!state.audio_prepare.is_occupied());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn engine_ready_failure_enqueues_blocking_audio_teardown_without_joining_command() {
        let state = PlaybackState::new();
        let identity = identity(40, 1, "engine-ready-failure");
        let server = PreviewServer::start().await.expect("start preview server");
        let gate = PublicationGate::open();
        let cleanup = state
            .reaper
            .try_reserve()
            .expect("reserve failed-start reap");
        let (audio, muted, stop_seen, release_tx) =
            super::super::audio::AudioPlayback::test_blocking_stop();

        cleanup_audio_after_engine_ready_failure(
            identity.clone(),
            Some(audio),
            gate.clone(),
            Arc::clone(&server),
            cleanup,
        )
        .expect("failed engine audio queued for bounded reap");

        assert!(!gate.is_open());
        assert!(muted.load(std::sync::atomic::Ordering::Acquire));
        stop_seen
            .recv_timeout(Duration::from_secs(2))
            .expect("audio stop requested before command-path helper returns");
        assert_eq!(state.reaper.outstanding_count(), 1);
        release_tx
            .send(())
            .expect("release blocking audio teardown");
        state.reaper.wait_until_idle(Duration::from_secs(2));
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
