use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

struct CancelState {
    /// 0 = active, 1 = cancelled, 2 = committed. A committed operation no
    /// longer accepts cancellation, which gives callers a final linearization
    /// point after all output validation has passed.
    status: AtomicU8,
    spawned_children: AtomicUsize,
    active_readers: AtomicUsize,
    checkpoints: AtomicUsize,
}

impl Default for CancelState {
    fn default() -> Self {
        Self {
            status: AtomicU8::new(0),
            spawned_children: AtomicUsize::new(0),
            active_readers: AtomicUsize::new(0),
            checkpoints: AtomicUsize::new(0),
        }
    }
}

struct CancellationCoordinator {
    parent: Arc<CancelState>,
    active_phase: Mutex<Option<Weak<CancelState>>>,
}

/// Cloneable cooperative cancellation shared by media workers and FFmpeg jobs.
#[derive(Clone)]
pub struct MediaCancelToken {
    state: Arc<CancelState>,
    coordinator: Arc<CancellationCoordinator>,
    is_phase: bool,
}

impl Default for MediaCancelToken {
    fn default() -> Self {
        let state = Arc::new(CancelState::default());
        Self {
            state: Arc::clone(&state),
            coordinator: Arc::new(CancellationCoordinator {
                parent: state,
                active_phase: Mutex::new(None),
            }),
            is_phase: false,
        }
    }
}

impl MediaCancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a cancellation scope whose cancellation inherits from this
    /// token, while a final commit on the child does not commit the parent
    /// workflow. Long-running workflows use this for independently committed
    /// phases such as timeline preprocessing followed by upload.
    pub fn child(&self) -> Self {
        let state = Arc::new(CancelState::default());
        {
            let mut active_phase = self
                .coordinator
                .active_phase
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match self.coordinator.parent.status.load(Ordering::Acquire) {
                0 => *active_phase = Some(Arc::downgrade(&state)),
                1 => state.status.store(1, Ordering::Release),
                2 => state.status.store(2, Ordering::Release),
                _ => unreachable!("cancel status is a three-state contract"),
            }
        }
        Self {
            state,
            coordinator: Arc::clone(&self.coordinator),
            is_phase: true,
        }
    }

    pub fn cancel(&self) {
        let mut active_phase = self
            .coordinator
            .active_phase
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.is_phase {
            let _ = self
                .state
                .status
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
            if active_phase
                .as_ref()
                .and_then(Weak::upgrade)
                .is_some_and(|phase| Arc::ptr_eq(&phase, &self.state))
            {
                *active_phase = None;
            }
            return;
        }
        let parent = &self.coordinator.parent;
        let _ = parent
            .status
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
        if let Some(phase) = active_phase.take().and_then(|phase| phase.upgrade()) {
            let _ = phase
                .status
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
        }
    }

    /// Atomically claim the final success boundary. Returns false when
    /// cancellation won the race; later `cancel()` calls cannot invalidate a
    /// committed operation.
    pub fn try_commit(&self) -> bool {
        let mut active_phase = self
            .coordinator
            .active_phase
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.is_phase && self.coordinator.parent.status.load(Ordering::Acquire) == 1 {
            self.state.status.store(1, Ordering::Release);
            *active_phase = None;
            return false;
        }
        let committed = self
            .state
            .status
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
        if committed && self.is_phase {
            *active_phase = None;
        }
        committed
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.status.load(Ordering::Acquire) == 1
            || (self.is_phase
                && self.state.status.load(Ordering::Acquire) == 0
                && self.coordinator.parent.status.load(Ordering::Acquire) == 1)
    }

    pub(crate) fn child_spawned(&self) {
        self.state.spawned_children.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn reader_started(&self) {
        self.state.active_readers.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn reader_finished(&self) {
        self.state.active_readers.fetch_sub(1, Ordering::AcqRel);
    }

    #[doc(hidden)]
    pub fn checkpoint(&self) -> bool {
        self.state.checkpoints.fetch_add(1, Ordering::AcqRel);
        self.is_cancelled()
    }

    #[doc(hidden)]
    pub fn spawned_child_count(&self) -> usize {
        self.state.spawned_children.load(Ordering::Acquire)
    }

    #[doc(hidden)]
    pub fn active_reader_count(&self) -> usize {
        self.state.active_readers.load(Ordering::Acquire)
    }

    #[doc(hidden)]
    pub fn checkpoint_count(&self) -> usize {
        self.state.checkpoints.load(Ordering::Acquire)
    }

    #[doc(hidden)]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }
}

#[cfg(test)]
mod tests {
    use super::MediaCancelToken;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn commit_blocks_later_cancellation() {
        let token = MediaCancelToken::new();
        assert!(token.try_commit());
        token.cancel();
        assert!(!token.is_cancelled());
        assert!(!token.try_commit());
    }

    #[test]
    fn cancellation_blocks_commit() {
        let token = MediaCancelToken::new();
        token.cancel();
        assert!(!token.try_commit());
        assert!(token.is_cancelled());
    }

    #[test]
    fn child_commit_does_not_commit_parent_workflow() {
        let parent = MediaCancelToken::new();
        let child = parent.child();
        assert!(child.try_commit());
        assert!(!parent.is_cancelled());
        assert!(parent.try_commit());
    }

    #[test]
    fn parent_cancel_and_child_commit_have_one_linearized_winner() {
        for _ in 0..32 {
            let parent = Arc::new(MediaCancelToken::new());
            let child = parent.child();
            let child_observer = child.clone();
            let barrier = Arc::new(Barrier::new(3));
            let commit_barrier = Arc::clone(&barrier);
            let cancel_barrier = Arc::clone(&barrier);
            let commit = thread::spawn(move || {
                commit_barrier.wait();
                child.try_commit()
            });
            let cancel_parent = Arc::clone(&parent);
            let cancel = thread::spawn(move || {
                cancel_barrier.wait();
                cancel_parent.cancel();
            });
            barrier.wait();
            let committed = commit.join().expect("child commit thread");
            cancel.join().expect("parent cancel thread");
            assert_eq!(committed, !child_observer.is_cancelled());
        }
    }

    #[test]
    fn commit_and_cancel_have_one_linearized_winner() {
        for _ in 0..32 {
            let token = Arc::new(MediaCancelToken::new());
            let barrier = Arc::new(Barrier::new(3));
            let commit_token = Arc::clone(&token);
            let cancel_token = Arc::clone(&token);
            let commit_barrier = Arc::clone(&barrier);
            let cancel_barrier = Arc::clone(&barrier);
            let commit = thread::spawn(move || {
                commit_barrier.wait();
                commit_token.try_commit()
            });
            let cancel = thread::spawn(move || {
                cancel_barrier.wait();
                cancel_token.cancel();
            });
            barrier.wait();
            let committed = commit.join().expect("commit thread");
            cancel.join().expect("cancel thread");
            assert_eq!(committed, !token.is_cancelled());
        }
    }
}
