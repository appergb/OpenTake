//! Background index/transcribe scheduling primitives. Port of the schedulable
//! core of `Search/SearchIndexCoordinator.swift` — UI/`@Observable` state lives
//! in `opentake-core`/the frontend (SPEC §7.3); this crate provides the
//! export-pause signal and the schedule-eligibility logic as testable pieces.
//!
//! Scope boundary (SPEC §7.3): this module ships only the portable, testable
//! scheduling *kernel* — which assets need work ([`work_needed`]), how the
//! progress bar splits ([`visual_share`]), and the cross-window export-pause
//! ref-count ([`ExportPause`]). The runtime pieces of §7.3 are **deliberately
//! deferred to `opentake-core`**, which owns the tokio runtime and event loop:
//!   - the single-worker tokio queue (enqueue / dequeue-skip-stale / `index_one`
//!     running transcribe + visual concurrently, plus the `failed` set);
//!   - `search_visual`'s off-thread index snapshot;
//!   - the 2 s `wait_while_active` poll referenced by §5.5 / §7.3 — built there
//!     on [`ExportPause::is_active`] against that crate's runtime, so this crate
//!     stays runtime-agnostic and needs no async/tokio dependency.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use opentake_domain::media::MediaAsset;
use opentake_domain::ClipType;

use crate::search::embedder::EmbedderSpec;
use crate::search::indexer::needs_index;
use crate::transcribe::cache::has_cached_on_disk;

/// Cross-window reference-counted export-active flag. Background work yields
/// while the count is non-zero. Port of `ExportPauseCounter`
/// (`SearchIndexCoordinator.swift:37-47`).
#[derive(Default)]
struct ExportPauseInner {
    active: AtomicUsize,
    changed: Condvar,
    gate: Mutex<()>,
}

#[derive(Clone, Default)]
pub struct ExportPause(Arc<ExportPauseInner>);

/// Balanced playback/export pressure holder. Dropping the final guard wakes
/// every blocked background worker, including early-return and unwind paths.
pub struct ExportPauseGuard {
    pause: ExportPause,
}

impl ExportPause {
    pub fn new() -> Self {
        ExportPause::default()
    }
    /// Mark an export as begun (increment).
    pub fn begin(&self) {
        self.0.active.fetch_add(1, Ordering::SeqCst);
    }
    /// Mark an export as ended (decrement; saturating at 0).
    pub fn end(&self) {
        let previous = self
            .0
            .active
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                Some(v.saturating_sub(1))
            });
        if matches!(previous, Ok(1)) {
            self.0.changed.notify_all();
        }
    }
    /// True while any export is active.
    pub fn is_active(&self) -> bool {
        self.0.active.load(Ordering::SeqCst) > 0
    }

    /// Begin pressure and return an unwind-safe, automatically balanced guard.
    pub fn guard(&self) -> ExportPauseGuard {
        self.begin();
        ExportPauseGuard {
            pause: self.clone(),
        }
    }

    /// Block until playback/export pressure clears. The cancellation predicate
    /// is checked at short intervals so shutdown and job cancellation cannot be
    /// stranded behind a missing `end`. Returns `false` when cancelled.
    pub fn wait_while_active(&self, cancelled: impl Fn() -> bool) -> bool {
        let mut gate = self.0.gate.lock().unwrap_or_else(|e| e.into_inner());
        while self.is_active() {
            if cancelled() {
                return false;
            }
            let (next, _) = self
                .0
                .changed
                .wait_timeout(gate, Duration::from_millis(20))
                .unwrap_or_else(|e| e.into_inner());
            gate = next;
        }
        !cancelled()
    }
}

impl Drop for ExportPauseGuard {
    fn drop(&mut self) {
        self.pause.end();
    }
}

/// What kinds of work an asset needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WorkNeeded {
    pub visual: bool,
    pub transcript: bool,
}

impl WorkNeeded {
    pub fn any(self) -> bool {
        self.visual || self.transcript
    }
}

/// Decide whether `asset` should be scheduled, and for what. Mirrors the
/// `schedule`/`needsVisual`/`needsTranscript` conditions
/// (`SearchIndexCoordinator.swift:107-124`):
/// - skip while the asset is still generating;
/// - **visual**: a video or image whose embedding index is not current;
/// - **transcript**: audio, or a video with audio, that has no disk transcript.
pub fn work_needed(cache_root: &Path, asset: &MediaAsset, spec: &EmbedderSpec) -> WorkNeeded {
    if asset.is_generating() {
        return WorkNeeded::default();
    }
    let path = asset.url.as_path();

    let visual = matches!(asset.kind, ClipType::Video | ClipType::Image)
        && needs_index(cache_root, path, spec);

    let needs_transcript_kind = match asset.kind {
        ClipType::Audio => true,
        ClipType::Video => asset.has_audio,
        _ => false,
    };
    let transcript = needs_transcript_kind && !has_cached_on_disk(cache_root, path);

    WorkNeeded { visual, transcript }
}

/// The visual share of progress for an asset: `0.5` when it also needs a
/// transcript (the two run concurrently and split the bar), else `1.0`. Port of
/// `visualShare` (`SearchIndexCoordinator.swift:181-185`).
pub fn visual_share(work: WorkNeeded) -> f64 {
    if work.transcript && work.visual {
        0.5
    } else {
        1.0
    }
}

/// Lightweight progress snapshot (mirrors the consumer-facing `IndexProgress`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IndexProgress {
    pub batch_total: usize,
    pub batch_completed: usize,
    pub current_fraction: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::media::GenerationStatus;
    use std::io::Write;

    fn spec() -> EmbedderSpec {
        crate::search::config::embedder_spec()
    }

    #[test]
    fn export_pause_ref_counts() {
        let p = ExportPause::new();
        assert!(!p.is_active());
        p.begin();
        assert!(p.is_active());
        p.begin();
        p.end();
        assert!(p.is_active()); // still 1 outstanding
        p.end();
        assert!(!p.is_active());
        // saturating: extra end stays at 0.
        p.end();
        assert!(!p.is_active());

        // A guard makes every early-return path balanced, and a blocked worker
        // wakes promptly once the final nested holder leaves.
        let outer = p.guard();
        let inner = p.guard();
        let waiter_pause = p.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            tx.send(waiter_pause.wait_while_active(|| false)).unwrap();
        });
        assert!(rx
            .recv_timeout(std::time::Duration::from_millis(25))
            .is_err());
        drop(inner);
        assert!(rx
            .recv_timeout(std::time::Duration::from_millis(25))
            .is_err());
        drop(outer);
        assert!(rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap());
        waiter.join().unwrap();

        p.begin();
        assert!(!p.wait_while_active(|| true));
        p.end();
    }

    #[test]
    fn export_pause_is_shared_across_clones() {
        let p = ExportPause::new();
        let q = p.clone();
        p.begin();
        assert!(q.is_active()); // clone observes the same counter
        q.end();
        assert!(!p.is_active());
    }

    #[test]
    fn generating_asset_needs_no_work() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = MediaAsset::new("a", "/x.mp4", ClipType::Video, "X", 5.0);
        a.generation_status = GenerationStatus::Generating;
        let w = work_needed(dir.path(), &a, &spec());
        assert!(!w.any());
    }

    #[test]
    fn video_with_audio_needs_visual_and_transcript() {
        let dir = tempfile::tempdir().unwrap();
        // Real file so needs_index() can compute a key (and finds no index).
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"video").unwrap();
        f.flush().unwrap();
        let mut a = MediaAsset::new("a", f.path(), ClipType::Video, "X", 5.0);
        a.has_audio = true;
        let w = work_needed(dir.path(), &a, &spec());
        assert!(w.visual);
        assert!(w.transcript);
        assert!(w.any());
    }

    #[test]
    fn silent_video_needs_only_visual() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"video").unwrap();
        f.flush().unwrap();
        let mut a = MediaAsset::new("a", f.path(), ClipType::Video, "X", 5.0);
        a.has_audio = false;
        let w = work_needed(dir.path(), &a, &spec());
        assert!(w.visual);
        assert!(!w.transcript);
    }

    #[test]
    fn audio_asset_needs_only_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"audio").unwrap();
        f.flush().unwrap();
        let a = MediaAsset::new("a", f.path(), ClipType::Audio, "X", 5.0);
        let w = work_needed(dir.path(), &a, &spec());
        assert!(!w.visual); // audio is not a visual type
        assert!(w.transcript);
    }

    #[test]
    fn image_asset_needs_only_visual() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"img").unwrap();
        f.flush().unwrap();
        let a = MediaAsset::new("a", f.path(), ClipType::Image, "X", 5.0);
        let w = work_needed(dir.path(), &a, &spec());
        assert!(w.visual);
        assert!(!w.transcript); // images have no audio to transcribe
    }

    #[test]
    fn transcript_skipped_when_already_cached() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"audio").unwrap();
        f.flush().unwrap();
        // Seed a disk transcript.
        crate::transcribe::cache::write_disk_for_test(
            dir.path(),
            f.path(),
            &crate::transcribe::TranscriptionResult {
                text: "x".into(),
                language: None,
                words: vec![],
                segments: vec![],
            },
        );
        let a = MediaAsset::new("a", f.path(), ClipType::Audio, "X", 5.0);
        let w = work_needed(dir.path(), &a, &spec());
        assert!(!w.transcript);
    }

    #[test]
    fn visual_share_splits_when_both() {
        assert_eq!(
            visual_share(WorkNeeded {
                visual: true,
                transcript: true
            }),
            0.5
        );
        assert_eq!(
            visual_share(WorkNeeded {
                visual: true,
                transcript: false
            }),
            1.0
        );
        assert_eq!(
            visual_share(WorkNeeded {
                visual: false,
                transcript: true
            }),
            1.0
        );
    }
}
