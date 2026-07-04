//! Transcript cache: in-memory LRU (max 4, cleared wholesale when full) + disk
//! JSON, with half-open window filtering. Port of
//! `Transcription/TranscriptCache.swift`.
//!
//! Only **full-file** transcripts are cached; windowed requests are served by
//! [`filter`]-ing a cached full transcript. Disk files are
//! `<cache_root>/Transcripts/<key>.json` and interchangeable with upstream.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{TranscribeOptions, Transcriber, TranscriptionResult};
use crate::cache_key::{file_identity_key, KEY_HEX_LEN};

/// Cache subdirectory name (kept identical to upstream).
pub const CACHE_SUBDIR: &str = "Transcripts";
/// In-memory cache capacity before a wholesale clear (`memoryMax`).
pub const MEMORY_MAX: usize = 4;

fn disk_path(cache_root: &Path, key: &str) -> PathBuf {
    cache_root.join(CACHE_SUBDIR).join(format!("{key}.json"))
}

/// Filter a full transcript to a half-open `[lower, upper)` window. Segments hit
/// when `end > lower && start < upper`; words additionally require non-`None`
/// timings. The result's `text` is the surviving segment texts joined by a
/// space. Verbatim port of `TranscriptCache.filter` (`:29-39`).
pub fn filter(r: &TranscriptionResult, range: (f64, f64)) -> TranscriptionResult {
    let (lower, upper) = range;
    let segments: Vec<_> = r
        .segments
        .iter()
        .filter(|s| s.end > lower && s.start < upper)
        .cloned()
        .collect();
    let words: Vec<_> = r
        .words
        .iter()
        .filter(|w| match (w.start, w.end) {
            (Some(s), Some(e)) => e > lower && s < upper,
            _ => false,
        })
        .cloned()
        .collect();
    let text = segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    TranscriptionResult {
        text,
        language: r.language.clone(),
        words,
        segments,
    }
}

/// Disk-only existence check (`hasCachedOnDisk`).
pub fn has_cached_on_disk(cache_root: &Path, path: &Path) -> bool {
    match file_identity_key(path, KEY_HEX_LEN) {
        Some(key) => disk_path(cache_root, &key).exists(),
        None => false,
    }
}

/// Disk-only read (`cachedOnDisk`). Returns `None` on missing/unparsable file.
pub fn cached_on_disk(cache_root: &Path, path: &Path) -> Option<TranscriptionResult> {
    let key = file_identity_key(path, KEY_HEX_LEN)?;
    let data = std::fs::read(disk_path(cache_root, &key)).ok()?;
    serde_json::from_slice(&data).ok()
}

/// In-memory + disk transcript cache. Thread-safe.
pub struct TranscriptCache {
    cache_root: PathBuf,
    memory: Mutex<HashMap<String, TranscriptionResult>>,
}

impl TranscriptCache {
    pub fn new(cache_root: impl Into<PathBuf>) -> Self {
        TranscriptCache {
            cache_root: cache_root.into(),
            memory: Mutex::new(HashMap::new()),
        }
    }

    /// Resolve a transcript for `path`, transcribing via `t` on a miss. With a
    /// `range`: a cached full transcript is filtered; otherwise the range is
    /// transcribed directly (not cached). Without a range: the full transcript is
    /// transcribed and cached. Port of `TranscriptCache.transcript` (`:12-27`).
    pub fn transcript(
        &self,
        path: &Path,
        is_video: bool,
        range: Option<(f64, f64)>,
        t: &dyn Transcriber,
    ) -> crate::error::Result<TranscriptionResult> {
        let _ = is_video; // backend reads the track type from the file itself.
        let key = file_identity_key(path, KEY_HEX_LEN);

        if let Some(ref key) = key {
            if let Some(full) = self.cached(key) {
                return Ok(match range {
                    Some(r) => filter(&full, r),
                    None => full,
                });
            }
        }

        if let Some(r) = range {
            // Window request with no cached full transcript: transcribe just the
            // window (not cached), timestamps shifted back in `transcribe_file`.
            let opts = TranscribeOptions {
                source_range: Some(r),
                ..Default::default()
            };
            return super::transcribe_file(path, t, &opts);
        }

        let full = super::transcribe_file(path, t, &TranscribeOptions::default())?;
        if let Some(key) = key {
            self.store(&key, &full);
        }
        Ok(full)
    }

    /// Memory-then-disk read, promoting a disk hit into memory.
    fn cached(&self, key: &str) -> Option<TranscriptionResult> {
        if let Some(r) = self.memory.lock().unwrap().get(key).cloned() {
            return Some(r);
        }
        let data = std::fs::read(disk_path(&self.cache_root, key)).ok()?;
        let r: TranscriptionResult = serde_json::from_slice(&data).ok()?;
        self.remember(key, r.clone());
        Some(r)
    }

    fn store(&self, key: &str, result: &TranscriptionResult) {
        self.remember(key, result.clone());
        let dir = self.cache_root.join(CACHE_SUBDIR);
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(json) = serde_json::to_vec(result) {
            let _ = std::fs::write(dir.join(format!("{key}.json")), json);
        }
    }

    fn remember(&self, key: &str, result: TranscriptionResult) {
        let mut mem = self.memory.lock().unwrap();
        if mem.len() >= MEMORY_MAX {
            mem.clear(); // wholesale clear, verbatim upstream behavior.
        }
        mem.insert(key.to_string(), result);
    }

    #[cfg(test)]
    pub(crate) fn memory_len(&self) -> usize {
        self.memory.lock().unwrap().len()
    }
}

/// Test-only: write a transcript to the disk cache for `media_path` using the
/// real on-disk layout (so `search`/`cached_on_disk` tests can seed fixtures).
#[cfg(test)]
pub(crate) fn write_disk_for_test(
    cache_root: &Path,
    media_path: &Path,
    result: &TranscriptionResult,
) {
    let key = file_identity_key(media_path, KEY_HEX_LEN).expect("media file must exist");
    let dir = cache_root.join(CACHE_SUBDIR);
    std::fs::create_dir_all(&dir).unwrap();
    let json = serde_json::to_vec(result).unwrap();
    std::fs::write(dir.join(format!("{key}.json")), json).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcribe::test_support::MockTranscriber;
    use crate::transcribe::{TranscriptionSegment, TranscriptionWord};
    use std::io::Write;

    fn res() -> TranscriptionResult {
        TranscriptionResult {
            text: "one two three".into(),
            language: Some("en".into()),
            words: vec![
                TranscriptionWord {
                    text: "one".into(),
                    start: Some(0.0),
                    end: Some(1.0),
                },
                TranscriptionWord {
                    text: "two".into(),
                    start: Some(5.0),
                    end: Some(6.0),
                },
                TranscriptionWord {
                    text: "three".into(),
                    start: None,
                    end: None,
                },
            ],
            segments: vec![
                TranscriptionSegment {
                    text: "one".into(),
                    start: 0.0,
                    end: 2.0,
                },
                TranscriptionSegment {
                    text: "two".into(),
                    start: 5.0,
                    end: 7.0,
                },
            ],
        }
    }

    #[test]
    fn filter_half_open_overlap_segments() {
        let r = res();
        // window [1,6): seg0 [0,2) overlaps, seg1 [5,7) overlaps.
        let f = filter(&r, (1.0, 6.0));
        assert_eq!(f.segments.len(), 2);
        // window [2,5): seg0 ends at 2 (end>lower? 2>2 false), seg1 starts at 5 (start<upper? 5<5 false) → none.
        let f2 = filter(&r, (2.0, 5.0));
        assert_eq!(f2.segments.len(), 0);
    }

    #[test]
    fn filter_drops_words_without_timing() {
        let r = res();
        let f = filter(&r, (0.0, 100.0));
        // "three" has no timing → excluded; one+two kept.
        assert_eq!(f.words.len(), 2);
        assert!(f.words.iter().all(|w| w.text != "three"));
    }

    #[test]
    fn filter_text_is_space_joined_segments() {
        let r = res();
        let f = filter(&r, (0.0, 100.0));
        assert_eq!(f.text, "one two");
    }

    #[test]
    fn filter_word_window_boundaries() {
        let r = res();
        // window [5.5, 5.9): word "two" [5,6) overlaps (6>5.5 and 5<5.9).
        let f = filter(&r, (5.5, 5.9));
        assert_eq!(f.words.len(), 1);
        assert_eq!(f.words[0].text, "two");
    }

    #[test]
    fn memory_lru_clears_wholesale_at_capacity() {
        let dir = tempfile::tempdir().unwrap();
        let cache = TranscriptCache::new(dir.path());
        // Insert MEMORY_MAX entries directly.
        for i in 0..MEMORY_MAX {
            cache.remember(&format!("k{i}"), res());
        }
        assert_eq!(cache.memory_len(), MEMORY_MAX);
        // One more triggers a wholesale clear, then insert → len 1.
        cache.remember("overflow", res());
        assert_eq!(cache.memory_len(), 1);
    }

    // NOTE: the cache *miss* path runs `extract_pcm` → real ffmpeg, so it is
    // covered by the ffmpeg integration tests (offline-gated). Here we exercise
    // the cache *hit* and *windowed-filter* logic by seeding the disk cache
    // directly — fully offline, no media decode.

    #[test]
    fn transcript_hit_returns_cached_full() {
        let dir = tempfile::tempdir().unwrap();
        let mut media = tempfile::NamedTempFile::new().unwrap();
        media.write_all(b"audio").unwrap();
        media.flush().unwrap();

        // Seed the disk cache so `transcript(None)` hits it without extracting.
        let seeded = res();
        write_disk_for_test(dir.path(), media.path(), &seeded);

        let cache = TranscriptCache::new(dir.path());
        let t = MockTranscriber::default();
        let got = cache.transcript(media.path(), false, None, &t).unwrap();
        assert_eq!(got, seeded); // returned the cached transcript, not the mock
        assert!(has_cached_on_disk(dir.path(), media.path()));
    }

    #[test]
    fn transcript_window_filters_cached_full() {
        let dir = tempfile::tempdir().unwrap();
        let mut media = tempfile::NamedTempFile::new().unwrap();
        media.write_all(b"audio").unwrap();
        media.flush().unwrap();

        // Seed a full transcript on disk (segments at [0,2) and [5,7)).
        write_disk_for_test(dir.path(), media.path(), &res());

        let cache = TranscriptCache::new(dir.path());
        let t = MockTranscriber::default();
        // Window [1,6) hits the cache then filters: both segments overlap.
        let win = cache
            .transcript(media.path(), false, Some((1.0, 6.0)), &t)
            .unwrap();
        assert_eq!(win.segments.len(), 2);
        assert_eq!(win.text, "one two");
        // Window [2,5) overlaps neither segment (half-open boundaries).
        let empty = cache
            .transcript(media.path(), false, Some((2.0, 5.0)), &t)
            .unwrap();
        assert_eq!(empty.segments.len(), 0);
    }

    #[test]
    fn has_cached_missing_file_is_false() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_cached_on_disk(dir.path(), Path::new("/no/such.wav")));
    }
}
