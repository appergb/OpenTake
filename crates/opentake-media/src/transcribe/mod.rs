//! Transcription data model + backend trait. The data types, `offsetting`,
//! locale matching, cache filtering, and keyword search are a 1:1 port of
//! `Transcription/{Transcription,TranscriptCache,TranscriptSearch}.swift`; only
//! the ASR backend changes (macOS Speech → whisper.cpp behind a feature).
//!
//! Time unit is **seconds (f64)** at every boundary (SPEC §0.1). JSON field
//! names match upstream so `<key>.json` transcript caches are interchangeable.

pub mod cache;
pub mod locale;
pub mod search;

#[cfg(feature = "whisper-backend")]
pub mod whisper;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::decode::pcm::{extract_pcm, PcmBuffer, PcmFormat, PcmSpec};
use crate::error::Result;

/// One token/word with optional timing. `start`/`end` may be `None` when the
/// backend cannot localize a token (upstream `audioTimeRange` is optional too).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionWord {
    pub text: String,
    pub start: Option<f64>,
    pub end: Option<f64>,
}

/// One endpointed utterance (pause/sentence boundary). `text` carries the
/// backend's punctuation and casing.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionSegment {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

/// Full transcription result.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: Option<String>,
    pub words: Vec<TranscriptionWord>,
    pub segments: Vec<TranscriptionSegment>,
}

impl TranscriptionResult {
    /// Shift every timestamp by `offset` seconds, back into source time after
    /// transcribing an extracted range. `offset == 0` is the identity. `None`
    /// word timings stay `None`. Verbatim port of `offsetting(by:)`
    /// (`Transcription.swift:26-38`).
    pub fn offsetting(&self, offset: f64) -> TranscriptionResult {
        if offset == 0.0 {
            return self.clone();
        }
        TranscriptionResult {
            text: self.text.clone(),
            language: self.language.clone(),
            words: self
                .words
                .iter()
                .map(|w| TranscriptionWord {
                    text: w.text.clone(),
                    start: w.start.map(|s| s + offset),
                    end: w.end.map(|e| e + offset),
                })
                .collect(),
            segments: self
                .segments
                .iter()
                .map(|s| TranscriptionSegment {
                    text: s.text.clone(),
                    start: s.start + offset,
                    end: s.end + offset,
                })
                .collect(),
        }
    }
}

/// Backend-tuning knobs (port of the `transcribe*` parameters).
#[derive(Clone, Debug, Default)]
pub struct TranscribeOptions {
    /// Upstream `etiquetteReplacements`. whisper has no built-in equivalent; the
    /// whisper backend applies an optional profanity word-list post-pass when
    /// set (off by default).
    pub censor_profanity: bool,
    /// BCP-47 / ISO-639 language hint passed to the backend.
    pub preferred_language: Option<String>,
    /// Absolute-seconds range to transcribe; the audio is extracted for this
    /// window and timestamps are shifted back via `offsetting(lower)`.
    pub source_range: Option<(f64, f64)>,
}

/// Pluggable ASR backend. Implementations consume 16 kHz mono f32 PCM and return
/// segment/word timestamps. Real backend (whisper) is feature-gated; tests use a
/// mock.
pub trait Transcriber: Send + Sync {
    fn transcribe_pcm(
        &self,
        pcm: &PcmBuffer,
        opts: &TranscribeOptions,
    ) -> Result<TranscriptionResult>;
}

/// whisper consumes 16 kHz mono f32 — the canonical PCM spec for transcription.
pub fn whisper_pcm_spec() -> PcmSpec {
    PcmSpec {
        sample_rate: 16_000,
        channels: 1,
        format: PcmFormat::F32,
    }
}

/// Transcribe a file (audio or video) via `t`. Extracts PCM for the requested
/// range (if any), runs the backend, and shifts timestamps back to source time.
/// Port of `Transcription.transcribe`/`transcribeVideoAudio`.
pub fn transcribe_file(
    path: &Path,
    t: &dyn Transcriber,
    opts: &TranscribeOptions,
) -> Result<TranscriptionResult> {
    let pcm = extract_pcm(path, &whisper_pcm_spec(), opts.source_range)?;
    let result = t.transcribe_pcm(&pcm, opts)?;
    let offset = opts.source_range.map(|(lo, _)| lo).unwrap_or(0.0);
    Ok(result.offsetting(offset))
}

#[cfg(test)]
pub(crate) mod test_support {
    //! A deterministic mock transcriber for offline tests across the crate.
    use super::*;

    /// Returns a fixed two-segment result regardless of input (timestamps in the
    /// 0..N range, so `offsetting` is observable).
    pub struct MockTranscriber {
        pub language: Option<String>,
    }

    impl Default for MockTranscriber {
        fn default() -> Self {
            MockTranscriber {
                language: Some("en".to_string()),
            }
        }
    }

    impl Transcriber for MockTranscriber {
        fn transcribe_pcm(
            &self,
            _pcm: &PcmBuffer,
            _opts: &TranscribeOptions,
        ) -> Result<TranscriptionResult> {
            Ok(TranscriptionResult {
                text: "hello world".to_string(),
                language: self.language.clone(),
                words: vec![
                    TranscriptionWord {
                        text: "hello".into(),
                        start: Some(0.0),
                        end: Some(0.5),
                    },
                    TranscriptionWord {
                        text: "world".into(),
                        start: Some(0.5),
                        end: Some(1.0),
                    },
                ],
                segments: vec![TranscriptionSegment {
                    text: "hello world".into(),
                    start: 0.0,
                    end: 1.0,
                }],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TranscriptionResult {
        TranscriptionResult {
            text: "a b".into(),
            language: Some("en".into()),
            words: vec![
                TranscriptionWord {
                    text: "a".into(),
                    start: Some(1.0),
                    end: Some(2.0),
                },
                TranscriptionWord {
                    text: "b".into(),
                    start: None,
                    end: None,
                },
            ],
            segments: vec![TranscriptionSegment {
                text: "a b".into(),
                start: 1.0,
                end: 3.0,
            }],
        }
    }

    #[test]
    fn offsetting_zero_is_identity() {
        let r = sample();
        assert_eq!(r.offsetting(0.0), r);
    }

    #[test]
    fn offsetting_shifts_all_timecodes() {
        let r = sample().offsetting(10.0);
        assert_eq!(r.words[0].start, Some(11.0));
        assert_eq!(r.words[0].end, Some(12.0));
        assert_eq!(r.segments[0].start, 11.0);
        assert_eq!(r.segments[0].end, 13.0);
    }

    #[test]
    fn offsetting_preserves_none_word_timings() {
        let r = sample().offsetting(10.0);
        assert_eq!(r.words[1].start, None);
        assert_eq!(r.words[1].end, None);
        assert_eq!(r.words[1].text, "b");
    }

    #[test]
    fn offsetting_does_not_touch_text_or_language() {
        let r = sample().offsetting(5.0);
        assert_eq!(r.text, "a b");
        assert_eq!(r.language.as_deref(), Some("en"));
    }

    #[test]
    fn json_field_names_match_upstream() {
        let r = sample();
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"text\":"));
        assert!(json.contains("\"language\":"));
        assert!(json.contains("\"words\":"));
        assert!(json.contains("\"segments\":"));
        assert!(json.contains("\"start\":"));
        assert!(json.contains("\"end\":"));
        // round-trips
        let back: TranscriptionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn whisper_spec_is_16k_mono_f32() {
        let s = whisper_pcm_spec();
        assert_eq!(s.sample_rate, 16_000);
        assert_eq!(s.channels, 1);
        assert_eq!(s.format, PcmFormat::F32);
    }
}
