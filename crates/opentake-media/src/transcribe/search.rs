//! Keyword search over cached transcripts ("spoken search"). Pure logic, port of
//! `Transcription/TranscriptSearch.swift`.
//!
//! Matching is case- and diacritic-insensitive AND-of-substrings: a segment hits
//! when every query term appears as a substring of its text. Diacritic folding
//! uses Unicode NFD with combining marks stripped (SPEC §6.4).

use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

use super::cache;

/// A spoken-search hit: an asset's segment whose text matched all query terms.
#[derive(Clone, PartialEq, Debug)]
pub struct SpokenHit {
    pub asset_id: String,
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Split a query into terms: whitespace-separated, with leading/trailing
/// punctuation stripped (`"budget," → "budget"`), empties removed. Port of
/// `terms(in:)` (`TranscriptSearch.swift:27-32`).
pub fn terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| {
                c.is_ascii_punctuation() || (!c.is_alphanumeric() && !c.is_whitespace())
            })
            .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Case/diacritic-insensitive fold: NFD-decompose, drop combining marks, lowercase.
fn fold(s: &str) -> String {
    s.nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

/// Unicode combining marks (general categories Mn/Mc/Me) live in these ranges;
/// for diacritic folding the common `0x300..=0x36F` block plus a few others
/// cover Latin/Greek/Cyrillic accents. We use the canonical combining-mark test.
fn is_combining_mark(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F   // combining diacritical marks
        | 0x1AB0..=0x1AFF // combining diacritical marks extended
        | 0x1DC0..=0x1DFF // combining diacritical marks supplement
        | 0x20D0..=0x20FF // combining diacritical marks for symbols
        | 0xFE20..=0xFE2F // combining half marks
    )
}

/// True when `text` contains every term (case/diacritic-insensitive).
/// Port of `matches(_:terms:)` (`TranscriptSearch.swift:34-36`).
pub fn matches(text: &str, terms: &[String]) -> bool {
    if terms.is_empty() {
        return false;
    }
    let folded_text = fold(text);
    terms.iter().all(|t| folded_text.contains(&fold(t)))
}

/// Search cached transcripts (disk only) for `query` across `assets`
/// (`(asset_id, path)`). Returns up to `limit` hits, stopping early once the
/// limit is reached. Port of `TranscriptSearch.search` (`:12-25`).
pub fn search(
    cache_root: &Path,
    query: &str,
    assets: &[(String, PathBuf)],
    limit: usize,
) -> Vec<SpokenHit> {
    let terms = terms(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (asset_id, path) in assets {
        let Some(transcript) = cache::cached_on_disk(cache_root, path) else {
            continue;
        };
        for seg in &transcript.segments {
            if matches(&seg.text, &terms) {
                hits.push(SpokenHit {
                    asset_id: asset_id.clone(),
                    start: seg.start,
                    end: seg.end,
                    text: seg.text.clone(),
                });
                if hits.len() >= limit {
                    return hits;
                }
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcribe::{TranscriptionResult, TranscriptionSegment};

    #[test]
    fn terms_splits_and_strips_edge_punctuation() {
        assert_eq!(terms("budget, plan"), vec!["budget", "plan"]);
        assert_eq!(terms("  hello!  world?? "), vec!["hello", "world"]);
        assert_eq!(terms("a-b"), vec!["a-b"]); // interior punctuation preserved
    }

    #[test]
    fn terms_empty_query_is_empty() {
        assert!(terms("").is_empty());
        assert!(terms("   ,, !! ").is_empty());
    }

    #[test]
    fn matches_requires_all_terms_and_substring() {
        assert!(matches("the quarterly budget plan", &terms("budget plan")));
        assert!(!matches("the budget", &terms("budget plan"))); // missing "plan"
        assert!(matches("budgeting", &terms("budget"))); // substring
    }

    #[test]
    fn matches_is_case_insensitive() {
        assert!(matches("BUDGET", &terms("budget")));
        assert!(matches("Budget Plan", &terms("BUDGET plan")));
    }

    #[test]
    fn matches_is_diacritic_insensitive() {
        assert!(matches("café au lait", &terms("cafe")));
        assert!(matches("naïve", &terms("naive")));
        assert!(matches("résumé", &terms("resume")));
    }

    #[test]
    fn matches_empty_terms_is_false() {
        assert!(!matches("anything", &[]));
    }

    #[test]
    fn search_collects_and_respects_limit() {
        let dir = tempfile::tempdir().unwrap();
        // Write two transcripts via the cache module's disk format.
        let f1 = make_file(&dir, "one.wav");
        let f2 = make_file(&dir, "two.wav");
        write_transcript(dir.path(), &f1, &["alpha budget", "beta"]);
        write_transcript(dir.path(), &f2, &["budget gamma", "delta budget"]);

        let assets = vec![
            ("a1".to_string(), f1.clone()),
            ("a2".to_string(), f2.clone()),
        ];
        let hits = search(dir.path(), "budget", &assets, 10);
        // 1 from f1 + 2 from f2 = 3 hits.
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].asset_id, "a1");

        // limit truncates.
        let limited = search(dir.path(), "budget", &assets, 2);
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(search(dir.path(), "", &[], 10).is_empty());
    }

    // --- helpers: write a transcript using the real cache disk layout ---

    fn make_file(dir: &tempfile::TempDir, name: &str) -> PathBuf {
        let p = dir.path().join(name);
        std::fs::write(&p, b"dummy audio bytes").unwrap();
        p
    }

    fn write_transcript(cache_root: &Path, media_path: &Path, segment_texts: &[&str]) {
        let segments = segment_texts
            .iter()
            .enumerate()
            .map(|(i, t)| TranscriptionSegment {
                text: t.to_string(),
                start: i as f64,
                end: i as f64 + 1.0,
            })
            .collect();
        let result = TranscriptionResult {
            text: segment_texts.join(" "),
            language: Some("en".into()),
            words: vec![],
            segments,
        };
        cache::write_disk_for_test(cache_root, media_path, &result);
    }
}
