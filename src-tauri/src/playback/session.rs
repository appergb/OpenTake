use serde::{Deserialize, Serialize};

pub use opentake_core::ProjectRevision;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackIdentity {
    pub project_epoch: u64,
    pub timeline_version: u64,
    pub session_id: String,
}

impl PlaybackIdentity {
    pub fn new(
        project_epoch: u64,
        timeline_version: u64,
        session_id: impl Into<String>,
    ) -> Result<Self, PlaybackCommandError> {
        let session_id = session_id.into();
        if !valid_session_id(&session_id) {
            return Err(PlaybackCommandError::cancelled(
                "invalid playback session id",
            ));
        }
        Ok(Self {
            project_epoch,
            timeline_version,
            session_id,
        })
    }

    pub fn validate(self) -> Result<Self, PlaybackCommandError> {
        Self::new(self.project_epoch, self.timeline_version, self.session_id)
    }

    pub fn revision(&self) -> ProjectRevision {
        ProjectRevision {
            project_epoch: self.project_epoch,
            version: self.timeline_version,
        }
    }
}

fn valid_session_id(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackErrorCode {
    Superseded,
    Cancelled,
    Busy,
    Engine,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlaybackCommandError {
    pub code: PlaybackErrorCode,
    pub message: String,
}

impl PlaybackCommandError {
    pub fn superseded(message: impl Into<String>) -> Self {
        Self {
            code: PlaybackErrorCode::Superseded,
            message: message.into(),
        }
    }

    pub fn cancelled(message: impl Into<String>) -> Self {
        Self {
            code: PlaybackErrorCode::Cancelled,
            message: message.into(),
        }
    }

    pub fn busy(message: impl Into<String>) -> Self {
        Self {
            code: PlaybackErrorCode::Busy,
            message: message.into(),
        }
    }

    pub fn engine(message: impl Into<String>) -> Self {
        Self {
            code: PlaybackErrorCode::Engine,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PlaybackCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionPhase {
    Running,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionControl {
    Pause,
    Seek,
    Stop,
}

#[derive(Clone, Debug)]
struct ActiveSession {
    identity: PlaybackIdentity,
    phase: SessionPhase,
    publication_open: bool,
}

#[derive(Clone, Debug)]
pub struct StartTicket {
    generation: u64,
    identity: PlaybackIdentity,
}

impl StartTicket {
    pub fn identity(&self) -> &PlaybackIdentity {
        &self.identity
    }
}

#[derive(Clone, Debug)]
pub enum StartDecision {
    Resume,
    Build(StartTicket),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectTransition(u64);

#[derive(Default)]
pub struct SessionRegistry {
    generation: u64,
    project_epoch: u64,
    transition: Option<ProjectTransition>,
    active: Option<ActiveSession>,
}

impl SessionRegistry {
    pub fn begin_start(
        &mut self,
        requested: PlaybackIdentity,
        authoritative: ProjectRevision,
    ) -> Result<StartDecision, PlaybackCommandError> {
        let requested = requested.validate()?;
        if self.transition.is_some() {
            return Err(PlaybackCommandError::cancelled(
                "project transition is in progress",
            ));
        }
        if requested.revision() != authoritative {
            return Err(PlaybackCommandError::superseded(
                "requested playback revision is stale",
            ));
        }
        self.generation = self.generation.wrapping_add(1);
        if let Some(active) = self.active.as_mut() {
            if active.identity == requested && active.phase == SessionPhase::Paused {
                active.phase = SessionPhase::Running;
                active.publication_open = true;
                return Ok(StartDecision::Resume);
            }
        }
        self.active = None;
        Ok(StartDecision::Build(StartTicket {
            generation: self.generation,
            identity: requested,
        }))
    }

    pub fn install_if_current(
        &mut self,
        ticket: StartTicket,
        authoritative: ProjectRevision,
    ) -> Result<(), PlaybackCommandError> {
        if self.transition.is_some()
            || ticket.generation != self.generation
            || ticket.identity.revision() != authoritative
        {
            return Err(PlaybackCommandError::superseded(
                "playback start was superseded",
            ));
        }
        self.project_epoch = authoritative.project_epoch;
        self.active = Some(ActiveSession {
            identity: ticket.identity,
            phase: SessionPhase::Running,
            publication_open: true,
        });
        Ok(())
    }

    pub fn control(&mut self, identity: &PlaybackIdentity, control: SessionControl) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        if &active.identity != identity {
            return false;
        }
        match control {
            SessionControl::Pause => active.phase = SessionPhase::Paused,
            SessionControl::Seek => {}
            SessionControl::Stop => {
                self.active = None;
                self.generation = self.generation.wrapping_add(1);
            }
        }
        true
    }

    pub fn stop_all(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.active = None;
    }

    pub fn begin_project_transition(&mut self) -> ProjectTransition {
        self.generation = self.generation.wrapping_add(1);
        let transition = ProjectTransition(self.generation);
        self.transition = Some(transition);
        if let Some(active) = self.active.as_mut() {
            active.publication_open = false;
        }
        transition
    }

    pub fn cancel_project_transition(&mut self, transition: ProjectTransition) {
        if self.transition != Some(transition) {
            return;
        }
        self.transition = None;
        if let Some(active) = self.active.as_mut() {
            active.publication_open = true;
        }
    }

    pub fn activate_project(&mut self, transition: ProjectTransition, project_epoch: u64) {
        if self.transition != Some(transition) {
            return;
        }
        self.project_epoch = project_epoch;
        self.transition = None;
        self.active = None;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn activate_project_event(&mut self, project_epoch: u64) {
        self.project_epoch = project_epoch;
        self.transition = None;
        self.active = None;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn invalidate_for_timeline_change(&mut self, project_epoch: u64, version: u64) -> bool {
        let should_invalidate = self.active.as_ref().is_some_and(|active| {
            active.identity.project_epoch == project_epoch
                && active.identity.timeline_version != version
        });
        if should_invalidate {
            self.stop_all();
        }
        should_invalidate
    }

    pub fn active_identity(&self) -> Option<&PlaybackIdentity> {
        self.active.as_ref().map(|active| &active.identity)
    }

    pub fn phase(&self) -> Option<SessionPhase> {
        self.active.as_ref().map(|active| active.phase)
    }

    pub fn publication_is_open(&self, identity: &PlaybackIdentity) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| &active.identity == identity && active.publication_open)
    }

    pub fn project_epoch(&self) -> u64 {
        self.project_epoch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_core::{AppCore, CoreEvent};
    use std::sync::{Arc, Mutex};

    fn identity(epoch: u64, version: u64, id: &str) -> PlaybackIdentity {
        PlaybackIdentity::new(epoch, version, id).expect("valid identity")
    }

    fn install(registry: &mut SessionRegistry, identity: PlaybackIdentity) {
        let StartDecision::Build(ticket) = registry
            .begin_start(identity.clone(), identity.revision())
            .expect("start accepted")
        else {
            panic!("fresh identity must build");
        };
        registry
            .install_if_current(ticket, identity.revision())
            .expect("install accepted");
    }

    #[test]
    fn same_version_different_epoch_never_resumes_paused_playback() {
        let mut registry = SessionRegistry::default();
        let old = identity(4, 0, "old-session");
        install(&mut registry, old.clone());
        assert!(registry.control(&old, SessionControl::Pause));

        let replacement = identity(5, 0, "replacement");
        let decision = registry
            .begin_start(replacement.clone(), replacement.revision())
            .expect("new epoch may start");

        assert!(matches!(decision, StartDecision::Build(_)));
        assert_ne!(registry.active_identity(), Some(&old));
    }

    #[test]
    fn project_transition_rejects_start_between_invalidation_and_commit() {
        let mut registry = SessionRegistry::default();
        let current = identity(3, 8, "current");
        install(&mut registry, current.clone());
        let transition = registry.begin_project_transition();

        let error = registry
            .begin_start(
                identity(4, 0, "too-early"),
                ProjectRevision {
                    project_epoch: 4,
                    version: 0,
                },
            )
            .expect_err("transition must reject starts");

        assert_eq!(error.code, PlaybackErrorCode::Cancelled);
        assert!(!registry.publication_is_open(&current));
        registry.cancel_project_transition(transition);
        assert!(registry.publication_is_open(&current));
    }

    #[test]
    fn project_swap_happens_only_after_old_publication_is_closed() {
        let mut registry = SessionRegistry::default();
        let old = identity(9, 2, "old");
        install(&mut registry, old.clone());

        let transition = registry.begin_project_transition();
        assert!(!registry.publication_is_open(&old));
        registry.activate_project(transition, 10);

        assert_eq!(registry.project_epoch(), 10);
        assert!(registry.active_identity().is_none());
    }

    #[test]
    fn project_open_failure_does_not_advance_epoch_or_stop_playback() {
        let core = AppCore::new();
        let before = core.project_revision();
        let current = identity(before.project_epoch, before.version, "current");
        let mut registry = SessionRegistry::default();
        install(&mut registry, current.clone());
        let events = Arc::new(Mutex::new(Vec::<CoreEvent>::new()));
        let sink = Arc::clone(&events);
        core.subscribe(move |event| sink.lock().expect("events").push(event.clone()));

        let transition = registry.begin_project_transition();
        let result = core.open_project(std::env::temp_dir().join(format!(
            "opentake-missing-project-{}-5-2.opentake",
            std::process::id()
        )));
        assert!(result.is_err());
        registry.cancel_project_transition(transition);

        assert_eq!(core.project_revision(), before);
        assert_eq!(registry.active_identity(), Some(&current));
        assert_eq!(registry.phase(), Some(SessionPhase::Running));
        assert!(registry.publication_is_open(&current));
        assert!(events
            .lock()
            .expect("events")
            .iter()
            .all(|event| !matches!(event, CoreEvent::ProjectOpened { .. })));
    }

    #[test]
    fn successful_project_transition_advances_epoch_exactly_once() {
        let core = AppCore::new();
        let before = core.project_revision();
        let mut registry = SessionRegistry::default();
        let transition = registry.begin_project_transition();

        let snapshot = core.new_project();
        registry.activate_project(transition, snapshot.project_epoch);

        assert_eq!(snapshot.project_epoch, before.project_epoch + 1);
        assert_eq!(
            core.project_revision().project_epoch,
            before.project_epoch + 1
        );
        assert_eq!(registry.project_epoch(), before.project_epoch + 1);
    }

    #[test]
    fn stale_requested_revision_is_rejected_without_displacing_current_session() {
        let mut registry = SessionRegistry::default();
        let current = identity(2, 11, "current");
        install(&mut registry, current.clone());

        let error = registry
            .begin_start(identity(2, 10, "stale"), current.revision())
            .expect_err("stale revision rejected");

        assert_eq!(error.code, PlaybackErrorCode::Superseded);
        assert_eq!(registry.active_identity(), Some(&current));
        assert!(registry.publication_is_open(&current));
    }

    #[test]
    fn timeline_change_during_start_rejects_pending_install() {
        let mut registry = SessionRegistry::default();
        let requested = identity(7, 14, "pending");
        let StartDecision::Build(ticket) = registry
            .begin_start(requested.clone(), requested.revision())
            .expect("start accepted")
        else {
            panic!("new start must build");
        };

        let error = registry
            .install_if_current(
                ticket,
                ProjectRevision {
                    project_epoch: 7,
                    version: 15,
                },
            )
            .expect_err("changed timeline rejects install");
        assert_eq!(error.code, PlaybackErrorCode::Superseded);
        assert!(registry.active_identity().is_none());
    }

    #[test]
    fn project_change_during_start_rejects_pending_install_even_when_version_is_zero() {
        let mut registry = SessionRegistry::default();
        let requested = identity(1, 0, "pending");
        let StartDecision::Build(ticket) = registry
            .begin_start(requested.clone(), requested.revision())
            .expect("start accepted")
        else {
            panic!("new start must build");
        };

        let error = registry
            .install_if_current(
                ticket,
                ProjectRevision {
                    project_epoch: 2,
                    version: 0,
                },
            )
            .expect_err("changed project rejects install");
        assert_eq!(error.code, PlaybackErrorCode::Superseded);
    }

    #[test]
    fn stale_session_control_cannot_pause_seek_or_stop_replacement() {
        let mut registry = SessionRegistry::default();
        let old = identity(6, 3, "old");
        install(&mut registry, old.clone());
        let replacement = identity(6, 3, "replacement");
        install(&mut registry, replacement.clone());

        assert!(!registry.control(&old, SessionControl::Pause));
        assert!(!registry.control(&old, SessionControl::Seek));
        assert!(!registry.control(&old, SessionControl::Stop));
        assert_eq!(registry.active_identity(), Some(&replacement));
        assert_eq!(registry.phase(), Some(SessionPhase::Running));
    }

    #[test]
    fn invalid_session_ids_are_rejected_at_the_boundary() {
        for invalid in ["", "white space", "slash/id", "underscores_are_out"] {
            let error = PlaybackIdentity::new(1, 2, invalid).expect_err("invalid id");
            assert_eq!(error.code, PlaybackErrorCode::Cancelled);
        }
        assert!(PlaybackIdentity::new(1, 2, "a".repeat(129)).is_err());
        assert!(PlaybackIdentity::new(1, 2, "A-z-09").is_ok());
    }
}
