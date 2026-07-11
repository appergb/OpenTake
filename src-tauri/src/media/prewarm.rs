//! Bounded, project-scoped media prewarming.
//!
//! One scheduler owns three persistent workers and a 24-job bounded queue.
//! Reservations cover queued and running work, so duplicate UI hints coalesce
//! before they consume queue capacity. Project transitions cancel the old token
//! and make admission fail closed; cache publication is checked under the same
//! state lock immediately before the same-filesystem rename.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, Weak};
use std::thread;

use opentake_media::MediaCancelToken;
use serde::Serialize;

const PREWARM_QUEUE_CAPACITY: usize = 24;
const PREWARM_WORKERS: usize = 3;
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum PrewarmKind {
    GridPoster,
    PreviewPoster,
    TimelineVisuals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PrewarmResult {
    Queued,
    Duplicate,
    Cached,
    Busy,
    StaleProject,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ReservationKey {
    kind: PrewarmKind,
    cache_key: String,
}

struct SchedulerState {
    active_epoch: u64,
    transitioning: bool,
    cancel: MediaCancelToken,
    in_flight: HashSet<ReservationKey>,
}

struct SchedulerInner {
    sender: SyncSender<PrewarmJob>,
    state: Mutex<SchedulerState>,
}

type PrewarmWork = Box<dyn FnOnce(JobContext) + Send + 'static>;

struct PrewarmJob {
    epoch: u64,
    token: MediaCancelToken,
    reservation: ReservationKey,
    work: PrewarmWork,
}

pub struct PrewarmScheduler {
    inner: Arc<SchedulerInner>,
}

pub struct JobContext {
    inner: Weak<SchedulerInner>,
    epoch: u64,
    token: MediaCancelToken,
}

struct ReservationGuard {
    inner: Weak<SchedulerInner>,
    reservation: Option<ReservationKey>,
}

impl PrewarmScheduler {
    pub fn new(active_epoch: u64) -> Self {
        let (sender, receiver) = mpsc::sync_channel(PREWARM_QUEUE_CAPACITY);
        let inner = Arc::new(SchedulerInner {
            sender,
            state: Mutex::new(SchedulerState {
                active_epoch,
                transitioning: false,
                cancel: MediaCancelToken::new(),
                in_flight: HashSet::new(),
            }),
        });
        let receiver = Arc::new(Mutex::new(receiver));
        for index in 0..PREWARM_WORKERS {
            let worker_inner = Arc::downgrade(&inner);
            let worker_receiver = Arc::clone(&receiver);
            thread::Builder::new()
                .name(format!("opentake-media-prewarm-{index}"))
                .spawn(move || prewarm_worker(worker_inner, worker_receiver))
                .expect("spawn persistent media prewarm worker");
        }
        Self { inner }
    }

    pub fn begin_project_transition(&self) -> Result<(), String> {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.transitioning {
            return Err("media prewarm project transition is already in progress".to_string());
        }
        state.transitioning = true;
        state.cancel.cancel();
        Ok(())
    }

    pub fn activate_project(&self, epoch: u64) {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        if !state.transitioning && state.active_epoch == epoch {
            return;
        }
        state.cancel.cancel();
        state.active_epoch = epoch;
        state.transitioning = false;
        state.cancel = MediaCancelToken::new();
    }

    pub fn schedule<F, K>(
        &self,
        epoch: u64,
        kind: PrewarmKind,
        cache_key: K,
        cached: bool,
        work: F,
    ) -> PrewarmResult
    where
        K: Into<String>,
        F: FnOnce(JobContext) + Send + 'static,
    {
        let reservation = ReservationKey {
            kind,
            cache_key: cache_key.into(),
        };
        let token = {
            let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.transitioning || state.active_epoch != epoch {
                return PrewarmResult::StaleProject;
            }
            if cached {
                return PrewarmResult::Cached;
            }
            if !state.in_flight.insert(reservation.clone()) {
                return PrewarmResult::Duplicate;
            }
            state.cancel.clone()
        };
        let job = PrewarmJob {
            epoch,
            token,
            reservation: reservation.clone(),
            work: Box::new(work),
        };
        match self.inner.sender.try_send(job) {
            Ok(()) => PrewarmResult::Queued,
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.remove_reservation(&reservation);
                PrewarmResult::Busy
            }
        }
    }

    fn remove_reservation(&self, reservation: &ReservationKey) {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .in_flight
            .remove(reservation);
    }

    #[cfg(test)]
    fn in_flight_count(&self) -> usize {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .in_flight
            .len()
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn project_state(&self) -> (u64, bool) {
        let state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        (state.active_epoch, state.transitioning)
    }
}

impl JobContext {
    pub fn cancel_token(&self) -> MediaCancelToken {
        self.token.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        !self.is_current()
    }

    fn is_current(&self) -> bool {
        let Some(inner) = self.inner.upgrade() else {
            return false;
        };
        let state = inner.state.lock().unwrap_or_else(|p| p.into_inner());
        !state.transitioning
            && state.active_epoch == self.epoch
            && state.cancel.same_instance(&self.token)
            && !self.token.is_cancelled()
    }

    /// Write to a unique sibling staging file and publish only while this job's
    /// epoch/token still owns the scheduler. Holding the state lock across the
    /// rename makes transition cancellation and cache publication ordered.
    pub fn commit_staged_bytes(&self, target: &Path, bytes: &[u8]) -> Result<bool, String> {
        let parent = target
            .parent()
            .ok_or_else(|| format!("prewarm cache target has no parent: {}", target.display()))?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let file_name = target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| {
                format!(
                    "prewarm cache target has no file name: {}",
                    target.display()
                )
            })?;
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging = parent.join(format!(
            ".{file_name}.prewarm-{}-{sequence}.tmp",
            std::process::id()
        ));
        std::fs::write(&staging, bytes).map_err(|error| error.to_string())?;

        let Some(inner) = self.inner.upgrade() else {
            let _ = std::fs::remove_file(&staging);
            return Ok(false);
        };
        let state = inner.state.lock().unwrap_or_else(|p| p.into_inner());
        let current = !state.transitioning
            && state.active_epoch == self.epoch
            && state.cancel.same_instance(&self.token)
            && !self.token.is_cancelled();
        if !current {
            drop(state);
            let _ = std::fs::remove_file(&staging);
            return Ok(false);
        }
        let rename = std::fs::rename(&staging, target);
        drop(state);
        match rename {
            Ok(()) => Ok(true),
            Err(error) => {
                let _ = std::fs::remove_file(&staging);
                Err(error.to_string())
            }
        }
    }
}

impl Drop for ReservationGuard {
    fn drop(&mut self) {
        let Some(reservation) = self.reservation.take() else {
            return;
        };
        if let Some(inner) = self.inner.upgrade() {
            inner
                .state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .in_flight
                .remove(&reservation);
        }
    }
}

fn prewarm_worker(inner: Weak<SchedulerInner>, receiver: Arc<Mutex<Receiver<PrewarmJob>>>) {
    loop {
        let job = {
            let receiver = receiver.lock().unwrap_or_else(|p| p.into_inner());
            receiver.recv()
        };
        let Ok(job) = job else {
            return;
        };
        let guard = ReservationGuard {
            inner: inner.clone(),
            reservation: Some(job.reservation),
        };
        let context = JobContext {
            inner: inner.clone(),
            epoch: job.epoch,
            token: job.token,
        };
        if !context.is_cancelled() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (job.work)(context);
            }));
        }
        drop(guard);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_media::{decode_frame_at_cancellable, FrameRequest, MediaError};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) {
        let deadline = Instant::now() + timeout;
        while !predicate() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(predicate(), "condition did not become true before timeout");
    }

    #[test]
    fn queue_capacity_is_bounded_and_excess_job_returns_busy() {
        let scheduler = PrewarmScheduler::new(1);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = Arc::new(Mutex::new(release_rx));
        for index in 0..PREWARM_WORKERS {
            let entered = entered_tx.clone();
            let release = Arc::clone(&release_rx);
            assert_eq!(
                scheduler.schedule(
                    1,
                    PrewarmKind::PreviewPoster,
                    format!("run-{index}"),
                    false,
                    move |_| {
                        entered.send(()).expect("announce running job");
                        release
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .recv()
                            .expect("release running job");
                    }
                ),
                PrewarmResult::Queued
            );
        }
        for _ in 0..PREWARM_WORKERS {
            entered_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("three workers enter");
        }
        for index in 0..PREWARM_QUEUE_CAPACITY {
            assert_eq!(
                scheduler.schedule(
                    1,
                    PrewarmKind::PreviewPoster,
                    format!("queued-{index}"),
                    false,
                    |_| {}
                ),
                PrewarmResult::Queued
            );
        }
        assert_eq!(
            scheduler.schedule(1, PrewarmKind::PreviewPoster, "overflow", false, |_| {}),
            PrewarmResult::Busy
        );
        for _ in 0..PREWARM_WORKERS {
            release_tx.send(()).expect("release worker");
        }
    }

    #[test]
    fn same_epoch_kind_and_cache_key_is_coalesced() {
        let scheduler = PrewarmScheduler::new(7);
        assert_eq!(
            scheduler.schedule(7, PrewarmKind::GridPoster, "already", true, |_| {}),
            PrewarmResult::Cached
        );
        assert_eq!(
            serde_json::to_string(&PrewarmResult::StaleProject).expect("serialize result"),
            "\"staleProject\""
        );
        let (release_tx, release_rx) = mpsc::channel();
        assert_eq!(
            scheduler.schedule(7, PrewarmKind::GridPoster, "same", false, move |_| {
                release_rx.recv().expect("release first job");
            }),
            PrewarmResult::Queued
        );
        assert_eq!(
            scheduler.schedule(7, PrewarmKind::GridPoster, "same", false, |_| {}),
            PrewarmResult::Duplicate
        );
        release_tx.send(()).expect("release first job");
    }

    #[test]
    fn project_epoch_rotation_cancels_queued_jobs() {
        let scheduler = PrewarmScheduler::new(1);
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = Arc::new(Mutex::new(release_rx));
        for index in 0..PREWARM_WORKERS {
            let release = Arc::clone(&release_rx);
            assert_eq!(
                scheduler.schedule(
                    1,
                    PrewarmKind::GridPoster,
                    format!("block-{index}"),
                    false,
                    move |_| {
                        release
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .recv()
                            .expect("release blocker");
                    }
                ),
                PrewarmResult::Queued
            );
        }
        let ran = Arc::new(AtomicUsize::new(0));
        let queued_ran = Arc::clone(&ran);
        assert_eq!(
            scheduler.schedule(1, PrewarmKind::GridPoster, "queued", false, move |_| {
                queued_ran.fetch_add(1, Ordering::SeqCst);
            }),
            PrewarmResult::Queued
        );
        scheduler
            .begin_project_transition()
            .expect("begin transition");
        assert_eq!(
            scheduler.schedule(1, PrewarmKind::GridPoster, "cached-stale", true, |_| {}),
            PrewarmResult::StaleProject
        );
        scheduler.activate_project(2);
        for _ in 0..PREWARM_WORKERS {
            release_tx.send(()).expect("release blocker");
        }
        wait_until(Duration::from_secs(2), || scheduler.in_flight_count() == 0);
        assert_eq!(ran.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn project_epoch_rotation_cancels_running_decoder() {
        let scheduler = PrewarmScheduler::new(4);
        let temp = tempfile::tempdir().expect("create decoder fixture directory");
        let fifo = temp.path().join("blocking-prewarm-video");
        let status = Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo must start");
        assert!(status.success(), "mkfifo must create decoder fixture");
        let (token_tx, token_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        assert_eq!(
            scheduler.schedule(
                4,
                PrewarmKind::TimelineVisuals,
                "decoder",
                false,
                move |context| {
                    let token = context.cancel_token();
                    token_tx.send(token.clone()).expect("publish decoder token");
                    let result =
                        decode_frame_at_cancellable(&fifo, &FrameRequest::default(), &token);
                    result_tx.send(result).expect("publish decoder result");
                }
            ),
            PrewarmResult::Queued
        );
        let token = token_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("decoder starts");
        wait_until(Duration::from_secs(2), || token.spawned_child_count() == 1);
        scheduler
            .begin_project_transition()
            .expect("begin transition");
        assert!(token.is_cancelled());
        let error = result_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("running decoder stops")
            .expect_err("cancelled decoder must fail");
        assert!(matches!(error, MediaError::Cancelled));
        assert_eq!(token.active_reader_count(), 0);
    }

    #[test]
    fn stale_epoch_job_never_commits_cache_file() {
        let scheduler = PrewarmScheduler::new(10);
        let dir = tempfile::tempdir().expect("cache tempdir");
        let target = dir.path().join("cache.bin");
        let target_for_job = target.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (continue_tx, continue_rx) = mpsc::channel();
        let (committed_tx, committed_rx) = mpsc::channel();
        assert_eq!(
            scheduler.schedule(
                10,
                PrewarmKind::PreviewPoster,
                "cache",
                false,
                move |context| {
                    entered_tx.send(()).expect("announce staging");
                    continue_rx.recv().expect("continue stale job");
                    committed_tx
                        .send(
                            context
                                .commit_staged_bytes(&target_for_job, b"stale")
                                .expect("staged commit"),
                        )
                        .expect("publish commit result");
                }
            ),
            PrewarmResult::Queued
        );
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("job entered");
        scheduler
            .begin_project_transition()
            .expect("begin transition");
        scheduler.activate_project(11);
        continue_tx.send(()).expect("continue stale job");
        assert!(!committed_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("commit result"));
        assert!(!target.exists());
        assert_eq!(
            std::fs::read_dir(dir.path())
                .expect("read cache dir")
                .count(),
            0
        );
    }

    #[test]
    fn worker_concurrency_never_exceeds_three() {
        let scheduler = PrewarmScheduler::new(1);
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = Arc::new(Mutex::new(release_rx));
        for index in 0..12 {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            let entered = entered_tx.clone();
            let release = Arc::clone(&release_rx);
            assert_eq!(
                scheduler.schedule(
                    1,
                    PrewarmKind::GridPoster,
                    format!("job-{index}"),
                    false,
                    move |_| {
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        entered.send(()).expect("announce worker entry");
                        release
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .recv()
                            .expect("release worker");
                        active.fetch_sub(1, Ordering::SeqCst);
                    }
                ),
                PrewarmResult::Queued
            );
        }
        for _ in 0..PREWARM_WORKERS {
            entered_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("three workers enter");
        }
        assert_eq!(peak.load(Ordering::SeqCst), PREWARM_WORKERS);
        for _ in 0..12 {
            release_tx.send(()).expect("release scheduled work");
        }
        wait_until(Duration::from_secs(2), || scheduler.in_flight_count() == 0);
        assert_eq!(peak.load(Ordering::SeqCst), PREWARM_WORKERS);
    }

    #[test]
    fn new_epoch_can_schedule_same_cache_key_after_old_reservation_drops() {
        let scheduler = PrewarmScheduler::new(1);
        let (release_tx, release_rx) = mpsc::channel();
        assert_eq!(
            scheduler.schedule(1, PrewarmKind::PreviewPoster, "shared", false, move |_| {
                release_rx.recv().expect("release old epoch");
            }),
            PrewarmResult::Queued
        );
        scheduler
            .begin_project_transition()
            .expect("begin transition");
        scheduler.activate_project(2);
        assert_eq!(
            scheduler.schedule(2, PrewarmKind::PreviewPoster, "shared", false, |_| {}),
            PrewarmResult::Duplicate
        );
        release_tx.send(()).expect("release old reservation");
        wait_until(Duration::from_secs(2), || scheduler.in_flight_count() == 0);
        assert_eq!(
            scheduler.schedule(2, PrewarmKind::PreviewPoster, "shared", false, |_| {}),
            PrewarmResult::Queued
        );
    }
}
