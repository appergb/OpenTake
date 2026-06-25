//! Per-asset visual indexing: sampled frames → embeddings → `EmbeddingStore`.
//! Idempotent per `(file, model, sampler)`. Port of
//! `Search/Indexing/VisualIndexer.swift`.
//!
//! The shot-accumulation math ([`accumulate_rows`]) is pure and unit-tested: the
//! first shot's start is forced to `0` regardless of the first frame's actual
//! time, each row's `shot_end` is the next shot's start (or `duration` for the
//! last shot).

use std::path::Path;

use crate::error::{MediaError, Result};
use crate::frame::RgbaFrame;
use crate::search::embed_store::{self, Header, Row};
use crate::search::embedder::{Embedder, EmbedderSpec};
use crate::search::frame_sampler::{sample_frames, SamplerOptions, SAMPLER_VERSION};

/// A cooperative cancellation token.
#[derive(Clone, Default)]
pub struct CancelToken(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        CancelToken::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(MediaError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// True when `path` needs (re)indexing for `spec` (no current on-disk index).
pub fn needs_index(cache_root: &Path, path: &Path, spec: &EmbedderSpec) -> bool {
    match embed_store::key(path) {
        Some(key) => {
            !embed_store::is_current(cache_root, &key, &spec.model, spec.version, SAMPLER_VERSION)
        }
        None => false,
    }
}

/// One sampled frame's contribution to the index.
pub struct FrameInput<'a> {
    pub time: f64,
    pub is_new_shot: bool,
    pub image: &'a RgbaFrame,
}

/// Pure shot/row accumulation. Given the per-frame `(time, is_new_shot)` flags
/// and the total `duration`, produce the `Row` list. Verbatim port of the loop
/// in `VisualIndexer.index` (`:34-49`):
/// - on a new shot, push `if shots.is_empty() { 0.0 } else { time }`
/// - each row's `shot_start` = its shot's start
/// - `shot_end` = next shot's start, or `duration` for the final shot
pub fn accumulate_rows(frames: &[(f64, bool)], duration: f64) -> Vec<Row> {
    let mut shot_starts: Vec<f64> = Vec::new();
    let mut shot_index_per_frame: Vec<usize> = Vec::with_capacity(frames.len());

    for &(time, is_new_shot) in frames {
        if is_new_shot {
            shot_starts.push(if shot_starts.is_empty() { 0.0 } else { time });
        }
        // Frames before any new-shot flag (shouldn't happen: first is always new)
        // map to shot 0 defensively.
        let shot = shot_starts.len().saturating_sub(1);
        shot_index_per_frame.push(shot);
    }

    frames
        .iter()
        .zip(shot_index_per_frame.iter())
        .map(|(&(time, _), &shot)| {
            let shot_start = shot_starts.get(shot).copied().unwrap_or(0.0);
            let shot_end = shot_starts.get(shot + 1).copied().unwrap_or(duration);
            Row {
                time,
                shot_start,
                shot_end,
            }
        })
        .collect()
}

/// Index a video: sample frames, embed each, accumulate shot rows, and save.
/// No-op (returns `Ok`) when already current. Honors `cancel`.
#[allow(clippy::too_many_arguments)]
pub fn index_video(
    cache_root: &Path,
    path: &Path,
    duration_secs: f64,
    width: u32,
    height: u32,
    embedder: &dyn Embedder,
    opts: &SamplerOptions,
    cancel: &CancelToken,
    on_progress: Option<&dyn Fn(f64)>,
) -> Result<()> {
    let Some(key) = embed_store::key(path) else {
        return Ok(());
    };
    let spec = embedder.spec().clone();
    if !needs_index(cache_root, path, &spec) {
        return Ok(());
    }

    let frames = sample_frames(path, duration_secs, width, height, opts)?;

    let mut flags: Vec<(f64, bool)> = Vec::with_capacity(frames.len());
    let mut vectors: Vec<f32> = Vec::with_capacity(frames.len() * spec.embedding_dim);
    for frame in &frames {
        cancel.check()?;
        let v = embedder.encode_image(&frame.image)?;
        vectors.extend_from_slice(&v);
        flags.push((frame.time_secs, frame.is_new_shot));
        if duration_secs > 0.0 {
            if let Some(cb) = on_progress {
                cb((frame.time_secs / duration_secs).min(1.0));
            }
        }
    }
    cancel.check()?;

    let rows = accumulate_rows(&flags, duration_secs);
    save_index(cache_root, &key, &spec, &rows, &vectors)
}

/// Index a still image: one embedding, zero-length shot range
/// (`Row{time:0, shot_start:0, shot_end:0}`). Port of `indexImage`.
pub fn index_image(
    cache_root: &Path,
    path: &Path,
    image: &RgbaFrame,
    embedder: &dyn Embedder,
    cancel: &CancelToken,
) -> Result<()> {
    let Some(key) = embed_store::key(path) else {
        return Ok(());
    };
    let spec = embedder.spec().clone();
    if !needs_index(cache_root, path, &spec) {
        return Ok(());
    }
    cancel.check()?;
    let vectors = embedder.encode_image(image)?;
    let rows = vec![Row {
        time: 0.0,
        shot_start: 0.0,
        shot_end: 0.0,
    }];
    cancel.check()?;
    save_index(cache_root, &key, &spec, &rows, &vectors)
}

fn save_index(
    cache_root: &Path,
    key: &str,
    spec: &EmbedderSpec,
    rows: &[Row],
    vectors: &[f32],
) -> Result<()> {
    let header = Header {
        model: spec.model.clone(),
        model_version: spec.version,
        sampler_version: SAMPLER_VERSION,
        dim: spec.embedding_dim,
        count: rows.len(),
    };
    embed_store::save(cache_root, key, &header, rows, vectors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::embedder::test_support::MockEmbedder;

    // --- accumulate_rows: pure shot math ---

    #[test]
    fn first_shot_start_forced_to_zero() {
        // First frame at t=5 marked new shot → shot_start must be 0, not 5.
        let frames = vec![(5.0, true), (7.0, false)];
        let rows = accumulate_rows(&frames, 20.0);
        assert_eq!(rows[0].shot_start, 0.0);
        assert_eq!(rows[1].shot_start, 0.0);
    }

    #[test]
    fn shot_end_is_next_shot_start() {
        // shots: [0..10), [10..duration). frames: (1,new),(5,no),(10,new),(15,no)
        let frames = vec![(1.0, true), (5.0, false), (10.0, true), (15.0, false)];
        let rows = accumulate_rows(&frames, 30.0);
        // shot 0 rows end at 10 (next shot start)
        assert_eq!(rows[0].shot_start, 0.0);
        assert_eq!(rows[0].shot_end, 10.0);
        assert_eq!(rows[1].shot_end, 10.0);
        // shot 1 rows end at duration
        assert_eq!(rows[2].shot_start, 10.0);
        assert_eq!(rows[2].shot_end, 30.0);
        assert_eq!(rows[3].shot_end, 30.0);
    }

    #[test]
    fn single_shot_ends_at_duration() {
        let frames = vec![(2.0, true), (4.0, false), (6.0, false)];
        let rows = accumulate_rows(&frames, 12.0);
        assert!(rows
            .iter()
            .all(|r| r.shot_start == 0.0 && r.shot_end == 12.0));
    }

    #[test]
    fn three_shots_chain_correctly() {
        let frames = vec![(1.0, true), (4.0, true), (9.0, true)];
        let rows = accumulate_rows(&frames, 15.0);
        assert_eq!(rows[0].shot_start, 0.0);
        assert_eq!(rows[0].shot_end, 4.0);
        assert_eq!(rows[1].shot_start, 4.0);
        assert_eq!(rows[1].shot_end, 9.0);
        assert_eq!(rows[2].shot_start, 9.0);
        assert_eq!(rows[2].shot_end, 15.0);
    }

    #[test]
    fn empty_frames_yields_empty_rows() {
        assert!(accumulate_rows(&[], 10.0).is_empty());
    }

    // --- index_image + needs_index idempotency (no ffmpeg needed) ---

    #[test]
    fn index_image_writes_zero_length_shot() {
        let dir = tempfile::tempdir().unwrap();
        let mut media = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut media, b"img").unwrap();
        std::io::Write::flush(&mut media).unwrap();

        let embedder = MockEmbedder::small();
        let frame = RgbaFrame::new(8, 8, vec![200; 8 * 8 * 4]);
        let cancel = CancelToken::new();

        assert!(needs_index(dir.path(), media.path(), embedder.spec()));
        index_image(dir.path(), media.path(), &frame, &embedder, &cancel).unwrap();

        // Now current → no longer needs index.
        assert!(!needs_index(dir.path(), media.path(), embedder.spec()));

        // Loaded row is the zero-length shot.
        let key = embed_store::key(media.path()).unwrap();
        let idx = embed_store::load(dir.path(), &key).unwrap();
        assert_eq!(idx.rows.len(), 1);
        assert_eq!(idx.rows[0].shot_start, 0.0);
        assert_eq!(idx.rows[0].shot_end, 0.0);
        assert_eq!(idx.header.dim, embedder.spec().embedding_dim);
    }

    #[test]
    fn index_image_respects_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        let mut media = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut media, b"img").unwrap();
        std::io::Write::flush(&mut media).unwrap();

        let embedder = MockEmbedder::small();
        let frame = RgbaFrame::black(8, 8);
        let cancel = CancelToken::new();
        cancel.cancel();
        let err = index_image(dir.path(), media.path(), &frame, &embedder, &cancel);
        assert!(matches!(err, Err(MediaError::Cancelled)));
    }
}
