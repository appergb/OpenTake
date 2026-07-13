use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::error::SecureFilesystemReason;

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum UnixProbeSample {
    Linux {
        magic: i64,
        fsid: u64,
        device: u64,
        ext_flags: std::result::Result<i64, SecureFilesystemReason>,
    },
    MacOs {
        mount_flags: u32,
        type_name: [u8; 16],
        fsid: u64,
        device: u64,
        case_sensitive: i64,
    },
    Failure(SecureFilesystemReason),
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
static UNIX_PROBE: OnceLock<Mutex<Option<UnixProbeSample>>> = OnceLock::new();

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) struct UnixProbeGuard;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn install_unix_probe(sample: UnixProbeSample) -> UnixProbeGuard {
    let mut slot = UNIX_PROBE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("Unix probe mutex poisoned");
    assert!(
        slot.is_none(),
        "safe_fs probe tests require --test-threads=1"
    );
    *slot = Some(sample);
    UnixProbeGuard
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl UnixProbeGuard {
    pub(super) fn replace(&self, sample: UnixProbeSample) {
        let mut slot = UNIX_PROBE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("Unix probe mutex poisoned");
        assert!(slot.is_some(), "Unix probe guard is not installed");
        *slot = Some(sample);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn unix_probe_sample() -> Option<UnixProbeSample> {
    UNIX_PROBE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("Unix probe mutex poisoned")
        .clone()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for UnixProbeGuard {
    fn drop(&mut self) {
        *UNIX_PROBE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("Unix probe mutex poisoned") = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CreateFailurePoint {
    Metadata,
    FilesystemProbe,
    CaseProof,
    ParentDuplicate,
}
static CREATE_FAILURE: OnceLock<Mutex<Option<CreateFailurePoint>>> = OnceLock::new();
pub(super) struct CreateFailureGuard;
pub(super) fn install_create_failure(point: CreateFailurePoint) -> CreateFailureGuard {
    let mut slot = CREATE_FAILURE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("create failure mutex poisoned");
    assert!(
        slot.is_none(),
        "safe_fs create-failure tests require --test-threads=1"
    );
    *slot = Some(point);
    CreateFailureGuard
}
pub(super) fn take_create_failure(point: CreateFailurePoint) -> bool {
    let mut slot = CREATE_FAILURE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("create failure mutex poisoned");
    if *slot == Some(point) {
        *slot = None;
        true
    } else {
        false
    }
}
impl Drop for CreateFailureGuard {
    fn drop(&mut self) {
        *CREATE_FAILURE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("create failure mutex poisoned") = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RollbackFailurePoint {
    RetainedIdentity,
    QuarantineMove,
    Delete,
}
static ROLLBACK_FAILURE: OnceLock<Mutex<Option<RollbackFailurePoint>>> = OnceLock::new();
pub(super) struct RollbackFailureGuard;
pub(super) fn install_rollback_failure(point: RollbackFailurePoint) -> RollbackFailureGuard {
    let mut slot = ROLLBACK_FAILURE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("rollback failure mutex poisoned");
    assert!(
        slot.is_none(),
        "safe_fs rollback-failure tests require --test-threads=1"
    );
    *slot = Some(point);
    RollbackFailureGuard
}
pub(super) fn take_rollback_failure(point: RollbackFailurePoint) -> bool {
    let mut slot = ROLLBACK_FAILURE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("rollback failure mutex poisoned");
    if *slot == Some(point) {
        *slot = None;
        true
    } else {
        false
    }
}
impl Drop for RollbackFailureGuard {
    fn drop(&mut self) {
        *ROLLBACK_FAILURE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("rollback failure mutex poisoned") = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HookPoint {
    BeforeQuarantineRename,
    BeforeQuarantineRestore,
    AfterFinalIdentityReadBeforeNameSyscall,
    BeforeMappingRewalk,
    BeforeCreatedRollbackInitialNameCheck,
    BeforeCreatedRollbackQuarantine,
    AfterCreatedRollbackVerifyBeforeDelete,
}
type Hook = Arc<dyn Fn(HookPoint) + Send + Sync>;
static HOOK: OnceLock<Mutex<Option<Hook>>> = OnceLock::new();
pub(super) struct HookGuard;
pub(super) fn install(hook: Hook) -> HookGuard {
    let mut slot = HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("hook mutex poisoned");
    assert!(
        slot.is_none(),
        "safe_fs race tests require --test-threads=1"
    );
    *slot = Some(hook);
    HookGuard
}
pub(super) fn hit(point: HookPoint) {
    let hook = HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("hook mutex poisoned")
        .clone();
    if let Some(hook) = hook {
        hook(point);
    }
}
impl Drop for HookGuard {
    fn drop(&mut self) {
        *HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("hook mutex poisoned") = None;
    }
}

pub(super) struct RaceGate {
    reached: (Mutex<bool>, Condvar),
    released: (Mutex<bool>, Condvar),
}
impl RaceGate {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            reached: (Mutex::new(false), Condvar::new()),
            released: (Mutex::new(false), Condvar::new()),
        })
    }
    pub(super) fn hook(self: &Arc<Self>, expected: HookPoint) -> Hook {
        let gate = Arc::clone(self);
        Arc::new(move |actual| {
            if actual != expected {
                return;
            }
            *gate.reached.0.lock().expect("race reached mutex poisoned") = true;
            gate.reached.1.notify_all();
            let released = gate.released.0.lock().expect("race release mutex poisoned");
            let (released, timeout) = gate
                .released
                .1
                .wait_timeout_while(released, Duration::from_secs(5), |value| !*value)
                .expect("race release mutex poisoned");
            assert!(*released && !timeout.timed_out(), "race release timed out");
        })
    }
    pub(super) fn wait_reached(&self) {
        let reached = self.reached.0.lock().expect("race reached mutex poisoned");
        let (reached, timeout) = self
            .reached
            .1
            .wait_timeout_while(reached, Duration::from_secs(5), |value| !*value)
            .expect("race reached mutex poisoned");
        assert!(
            *reached && !timeout.timed_out(),
            "race hook was not reached"
        );
    }
    pub(super) fn release(&self) {
        *self.released.0.lock().expect("race release mutex poisoned") = true;
        self.released.1.notify_all();
    }
}
