use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Default)]
struct CancelState {
    cancelled: AtomicBool,
    spawned_children: AtomicUsize,
    active_readers: AtomicUsize,
    checkpoints: AtomicUsize,
}

/// Cloneable cooperative cancellation shared by media workers and FFmpeg jobs.
#[derive(Clone, Default)]
pub struct MediaCancelToken {
    state: Arc<CancelState>,
}

impl MediaCancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.state.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
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
}
