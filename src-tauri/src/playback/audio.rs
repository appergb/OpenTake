//! Audio master clock + cpal output for streaming playback (#63).
//!
//! The acceptance criterion is "audio drives the playhead; video follows
//! (dropping frames to stay in sync)". [`build_clock`] decides the clock from the
//! timeline: when the project has sound, an audio master clock (cpal) advances
//! the playhead from the device's sample position; a silent project uses the
//! wall-clock [`InstantClock`] PR1 already ships.
//!
//! This slice wires the *decision* + the no-audio fallback so the MJPEG video
//! path is end-to-end first (wall-clock timed). The cpal output device + the
//! preload mixdown that backs the audio master clock land in the audio
//! follow-up; until then a timeline with audio also plays video-only on the
//! wall clock (no regression — there is no prior Rust playback audio).

use std::collections::HashMap;
use std::sync::Arc;

use opentake_domain::Timeline;

use super::engine::{InstantClock, PlaybackClock};
use super::project::MediaInfo;

/// Owns the audio output device + decode state for one playback session.
/// Dropping it stops the cpal stream. (The cpal output path lands in the audio
/// follow-up; this handle lets the engine/transport wiring own the audio device
/// lifetime now without a second refactor later.)
pub struct AudioPlayback {
    // cpal `Stream` (kept on its own thread, `!Send` on macOS) + the shared
    // sample-position atomic backing the master clock land here.
}

/// Build the playback clock for a session starting at `start_frame`.
///
/// Returns the wall-clock [`InstantClock`] for now (video-only timing). The cpal
/// master clock — where a timeline with audio returns an `AudioClock` plus a live
/// [`AudioPlayback`] — is added in the audio follow-up; the signature is final so
/// the command layer does not change when it lands.
pub fn build_clock(
    _timeline: &Timeline,
    _media: &HashMap<String, MediaInfo>,
    _fps: i32,
    start_frame: i32,
) -> (Arc<dyn PlaybackClock>, Option<AudioPlayback>) {
    (Arc::new(InstantClock::new(start_frame)), None)
}
