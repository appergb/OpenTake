//! Resource budget shared by real media workflows in each test executable.

use std::sync::{Condvar, Mutex};
use std::time::Duration;

// Successful-workflow tests must leave room under the eight-probe product
// limit for version checks and probes nested in the high-level media APIs.
// Deliberate overload tests use isolated processes instead of this budget.
const MAX_CONCURRENT_FIXTURES: usize = 4;
static ACTIVE_FIXTURES: Mutex<usize> = Mutex::new(0);
static FIXTURE_AVAILABLE: Condvar = Condvar::new();

pub(crate) struct FfmpegFixturePermit;

impl FfmpegFixturePermit {
    pub(crate) fn acquire() -> Self {
        let active = ACTIVE_FIXTURES.lock().expect("lock fixture budget");
        let (mut active, _) = FIXTURE_AVAILABLE
            .wait_timeout_while(active, Duration::from_secs(60), |active| {
                *active >= MAX_CONCURRENT_FIXTURES
            })
            .expect("wait for fixture capacity");
        if *active >= MAX_CONCURRENT_FIXTURES {
            drop(active);
            panic!("FFmpeg fixture capacity did not become available within 60 seconds");
        }
        *active += 1;
        Self
    }
}

impl Drop for FfmpegFixturePermit {
    fn drop(&mut self) {
        *ACTIVE_FIXTURES.lock().expect("release fixture budget") -= 1;
        FIXTURE_AVAILABLE.notify_one();
    }
}
