//! whisper.cpp backend (feature `whisper-backend`). Produces segment + word
//! timestamps from 16 kHz mono f32 PCM, mapped onto [`TranscriptionResult`].
//!
//! This compiles native whisper.cpp and links nothing the default build needs;
//! it is excluded unless the feature is on. Token timestamps are enabled and
//! whisper's centisecond segment times are converted to seconds, mirroring
//! upstream `decodeResults` (`Transcription.swift:284-322`): one
//! `TranscriptionSegment` per endpointed segment, one `TranscriptionWord` per
//! non-blank token, `text` = trimmed concatenation of segment texts.

use std::path::Path;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::{
    TranscribeOptions, Transcriber, TranscriptionResult, TranscriptionSegment, TranscriptionWord,
};
use crate::decode::pcm::PcmBuffer;
use crate::error::{MediaError, Result};

/// A loaded whisper model. Thread-safe; one model can back many transcriptions.
pub struct WhisperTranscriber {
    ctx: WhisperContext,
    n_threads: i32,
}

impl WhisperTranscriber {
    /// Load a ggml/gguf whisper model from disk.
    pub fn from_model_path(path: &Path) -> Result<Self> {
        let ctx = WhisperContext::new_with_params(
            &path.to_string_lossy(),
            WhisperContextParameters::default(),
        )
        .map_err(|e| MediaError::ModelInstall(format!("whisper load: {e}")))?;
        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        Ok(WhisperTranscriber { ctx, n_threads })
    }

    /// Override the inference thread count.
    pub fn with_threads(mut self, threads: i32) -> Self {
        self.n_threads = threads.max(1);
        self
    }
}

/// whisper segment times are in centiseconds (1/100 s).
fn cs_to_secs(cs: i64) -> f64 {
    cs as f64 / 100.0
}

// whisper.cpp's experimental token timestamps can spread the first words of
// an utterance backwards across a long leading pause (the segment timestamp
// token commonly starts at the pause itself).  That is dangerous for edit
// tools: deleting a filler word would then delete silence, or an earlier word,
// instead of the spoken filler.  Tighten only *silent segment edges* using the
// same PCM that Whisper consumed, then preserve token order by mapping the
// original token positions into the audible interval.  Internal pauses are
// intentionally untouched.
fn align_segment_to_speech(
    pcm: &PcmBuffer,
    segment: &mut TranscriptionSegment,
    words: &mut [TranscriptionWord],
) {
    const WINDOW_SECS: f64 = 0.020;
    const HOP_SECS: f64 = 0.010;
    const ABSOLUTE_RMS_FLOOR: f64 = 0.001; // -60 dBFS
    const RELATIVE_TO_PEAK: f64 = 0.02; // -34 dB from this segment's peak
    const EDGE_PADDING_SECS: f64 = 0.040;
    const MIN_EDGE_TRIM_SECS: f64 = 0.080;

    let sample_rate = pcm.spec.sample_rate as usize;
    let old_start = segment.start.max(0.0);
    let old_end = segment.end.min(pcm.duration_secs());
    let old_duration = old_end - old_start;
    if sample_rate == 0 || old_duration <= 0.0 || words.is_empty() {
        return;
    }

    let range_start = (old_start * sample_rate as f64).floor() as usize;
    let range_end = ((old_end * sample_rate as f64).ceil() as usize).min(pcm.samples_f32.len());
    if range_end <= range_start {
        return;
    }

    let window = ((WINDOW_SECS * sample_rate as f64).round() as usize).max(1);
    let hop = ((HOP_SECS * sample_rate as f64).round() as usize).max(1);
    let mut energy_windows = Vec::new();
    let mut cursor = range_start;
    while cursor < range_end {
        let end = (cursor + window).min(range_end);
        let samples = &pcm.samples_f32[cursor..end];
        let square_sum = samples
            .iter()
            .map(|sample| f64::from(*sample) * f64::from(*sample))
            .sum::<f64>();
        let rms = (square_sum / samples.len() as f64).sqrt();
        energy_windows.push((cursor, end, rms));
        cursor = cursor.saturating_add(hop);
    }

    let peak_rms = energy_windows
        .iter()
        .map(|(_, _, rms)| *rms)
        .fold(0.0_f64, f64::max);
    if peak_rms <= ABSOLUTE_RMS_FLOOR {
        return;
    }
    let threshold = ABSOLUTE_RMS_FLOOR.max(peak_rms * RELATIVE_TO_PEAK);
    let Some(first_audible) = energy_windows
        .iter()
        .position(|(_, _, rms)| *rms > threshold)
    else {
        return;
    };
    let last_audible = energy_windows
        .iter()
        .rposition(|(_, _, rms)| *rms > threshold)
        .unwrap_or(first_audible);

    let detected_start = energy_windows[first_audible].0 as f64 / sample_rate as f64;
    let detected_end = energy_windows[last_audible].1 as f64 / sample_rate as f64;
    let mut new_start = (detected_start - EDGE_PADDING_SECS).max(old_start);
    let mut new_end = (detected_end + EDGE_PADDING_SECS).min(old_end);
    if new_start - old_start < MIN_EDGE_TRIM_SECS {
        new_start = old_start;
    }
    if old_end - new_end < MIN_EDGE_TRIM_SECS {
        new_end = old_end;
    }
    if new_end <= new_start || (new_start == old_start && new_end == old_end) {
        return;
    }

    let new_duration = new_end - new_start;
    let remap = |time: f64| {
        let progress = ((time - old_start) / old_duration).clamp(0.0, 1.0);
        new_start + progress * new_duration
    };
    for word in words {
        if let Some(start) = word.start {
            word.start = Some(remap(start));
        }
        if let Some(end) = word.end {
            word.end = Some(remap(end));
        }
        if let (Some(start), Some(end)) = (word.start, word.end) {
            if end < start {
                word.end = Some(start);
            }
        }
    }
    segment.start = new_start;
    segment.end = new_end;
}

/// Keep edit-facing word rows lexical and give Whisper's zero-duration lexical
/// tokens a real, non-overlapping interval.  Whisper commonly emits `You`
/// `[t,t]` followed by `know` `[t,t+n]`; dropping the first span makes the
/// multi-word filler impossible to review or remove.  Punctuation-only tokens
/// are not independently editable words, so their time is available to the
/// neighboring lexical token.
fn normalize_word_timings(segment: &TranscriptionSegment, words: &mut Vec<TranscriptionWord>) {
    const EPSILON: f64 = 1e-9;

    words.retain(|word| word.text.chars().any(char::is_alphanumeric));
    let mut index = 0;
    while index < words.len() {
        let (Some(start), Some(end)) = (words[index].start, words[index].end) else {
            index += 1;
            continue;
        };
        if end > start + EPSILON {
            index += 1;
            continue;
        }

        // A same-start run with a later positive interval (for example
        // `You [t,t]`, `know [t,t+n]`) represents a single Whisper interval
        // shared by multiple lexical tokens. Split it by character weight.
        let mut run_end = index + 1;
        let mut shared_end = start;
        while run_end < words.len() {
            let (Some(next_start), Some(next_end)) = (words[run_end].start, words[run_end].end)
            else {
                break;
            };
            if (next_start - start).abs() > EPSILON {
                break;
            }
            shared_end = shared_end.max(next_end);
            run_end += 1;
        }
        if shared_end > start + EPSILON {
            let total_weight = words[index..run_end]
                .iter()
                .map(|word| {
                    word.text
                        .chars()
                        .filter(|c| c.is_alphanumeric())
                        .count()
                        .max(1)
                })
                .sum::<usize>() as f64;
            let mut cursor = start;
            for word in &mut words[index..run_end] {
                let weight = word
                    .text
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .count()
                    .max(1) as f64;
                let next = if (cursor - shared_end).abs() <= EPSILON {
                    shared_end
                } else {
                    (cursor + (shared_end - start) * weight / total_weight).min(shared_end)
                };
                word.start = Some(cursor);
                word.end = Some(next);
                cursor = next;
            }
            if let Some(last) = words.get_mut(run_end - 1) {
                last.end = Some(shared_end);
            }
            index = run_end;
            continue;
        }

        // A lone zero-duration token owns the gap to the next lexical token.
        let next_start = words[index + 1..]
            .iter()
            .filter_map(|word| word.start)
            .find(|next| *next > start + EPSILON)
            .unwrap_or(segment.end);
        if next_start > start + EPSILON {
            words[index].end = Some(next_start.min(segment.end));
        } else if index > 0 {
            let previous_end = words[index - 1].end.unwrap_or(segment.start);
            let fallback_start = (segment.end - 0.08).max(previous_end);
            if segment.end > fallback_start + EPSILON {
                words[index].start = Some(fallback_start);
                words[index].end = Some(segment.end);
            }
        }
        index += 1;
    }
}

impl Transcriber for WhisperTranscriber {
    fn transcribe_pcm(
        &self,
        pcm: &PcmBuffer,
        opts: &TranscribeOptions,
    ) -> Result<TranscriptionResult> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(self.n_threads);
        params.set_token_timestamps(true);
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        if let Some(lang) = opts.preferred_language.as_deref() {
            params.set_language(Some(lang));
        }

        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| MediaError::Transcribe(format!("create state: {e}")))?;
        state
            .full(params, &pcm.samples_f32)
            .map_err(|e| MediaError::Transcribe(format!("full: {e}")))?;

        let n_segments = state
            .full_n_segments()
            .map_err(|e| MediaError::Transcribe(format!("n_segments: {e}")))?;

        let mut segments = Vec::new();
        let mut words = Vec::new();
        let mut full_text = String::new();

        for i in 0..n_segments {
            let seg_text = state
                .full_get_segment_text(i)
                .map_err(|e| MediaError::Transcribe(format!("segment text: {e}")))?;

            let t0 = state.full_get_segment_t0(i).unwrap_or(0);
            let t1 = state.full_get_segment_t1(i).unwrap_or(0);
            let trimmed = seg_text.trim();
            // Skip a segment that is (once trimmed) nothing but a non-speech
            // marker whisper learned from its training captions — e.g.
            // "[BLANK_AUDIO]" over a silent gap (#198). These are ordinary
            // decoded text, not the internal special tokens filtered below, so
            // they only ever show up reconstructed at the segment level.
            // Excluded from `full_text` too, so the plain-text summary stays
            // consistent with `segments`.
            let keep_segment = !trimmed.is_empty() && !super::is_non_speech_marker(trimmed);
            if keep_segment {
                full_text.push_str(&seg_text);
            }

            let n_tokens = state.full_n_tokens(i).unwrap_or(0);
            let mut segment_words = Vec::new();
            for j in 0..n_tokens {
                let tok_text = match state.full_get_token_text(i, j) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let trimmed_tok = tok_text.trim();
                // Skip special tokens (whisper wraps them in [..] / <|..|>),
                // blanks, and a token that is itself a whole non-speech marker
                // (a short marker like "[MUSIC]" can decode as one token).
                if trimmed_tok.is_empty()
                    || (trimmed_tok.starts_with("[_"))
                    || (trimmed_tok.starts_with("<|") && trimmed_tok.ends_with("|>"))
                    || super::is_non_speech_marker(trimmed_tok)
                {
                    continue;
                }
                let data = state.full_get_token_data(i, j).ok();
                let (start, end) = match data {
                    Some(d) => (Some(cs_to_secs(d.t0)), Some(cs_to_secs(d.t1))),
                    None => (None, None),
                };
                segment_words.push(TranscriptionWord {
                    text: trimmed_tok.to_string(),
                    start,
                    end,
                });
            }
            if keep_segment {
                let mut segment = TranscriptionSegment {
                    text: trimmed.to_string(),
                    start: cs_to_secs(t0),
                    end: cs_to_secs(t1),
                };
                align_segment_to_speech(pcm, &mut segment, &mut segment_words);
                normalize_word_timings(&segment, &mut segment_words);
                segments.push(segment);
                words.extend(segment_words);
            }
        }

        let language = opts
            .preferred_language
            .clone()
            .or_else(|| Some("auto".to_string()));

        Ok(TranscriptionResult {
            text: full_text.trim().to_string(),
            language,
            words,
            segments,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::pcm::{PcmFormat, PcmSpec};

    #[test]
    fn centiseconds_convert_to_seconds() {
        assert!((cs_to_secs(150) - 1.5).abs() < 1e-9);
        assert_eq!(cs_to_secs(0), 0.0);
    }

    fn pcm(samples: Vec<f32>, sample_rate: u32) -> PcmBuffer {
        PcmBuffer {
            spec: PcmSpec {
                sample_rate,
                channels: 1,
                format: PcmFormat::F32,
            },
            samples_f32: samples,
        }
    }

    fn word(text: &str, start: f64, end: f64) -> TranscriptionWord {
        TranscriptionWord {
            text: text.into(),
            start: Some(start),
            end: Some(end),
        }
    }

    #[test]
    fn silent_segment_edges_are_trimmed_and_words_remapped() {
        let sample_rate = 1_000;
        let mut samples = vec![0.0; 2 * sample_rate as usize];
        samples.extend(vec![0.5; 6 * sample_rate as usize]);
        samples.extend(vec![0.0; 2 * sample_rate as usize]);
        let pcm = pcm(samples, sample_rate);
        let mut segment = TranscriptionSegment {
            text: "one two three".into(),
            start: 0.0,
            end: 10.0,
        };
        let mut words = vec![
            word("one", 1.0, 2.0),
            word("two", 4.0, 5.0),
            word("three", 8.0, 9.0),
        ];

        align_segment_to_speech(&pcm, &mut segment, &mut words);

        assert!((segment.start - 1.96).abs() < 0.02, "{:?}", segment);
        assert!((segment.end - 8.05).abs() < 0.02, "{:?}", segment);
        assert!(words[0].start.unwrap() >= segment.start);
        assert!(words[2].end.unwrap() <= segment.end);
        assert!(words.windows(2).all(|pair| {
            pair[0].start.unwrap() <= pair[1].start.unwrap()
                && pair[0].end.unwrap() <= pair[1].end.unwrap()
        }));
    }

    #[test]
    fn fully_audible_segment_keeps_original_timestamps() {
        let pcm = pcm(vec![0.5; 2_000], 1_000);
        let mut segment = TranscriptionSegment {
            text: "hello".into(),
            start: 0.0,
            end: 2.0,
        };
        let mut words = vec![word("hello", 0.2, 1.8)];

        align_segment_to_speech(&pcm, &mut segment, &mut words);

        assert_eq!(segment.start, 0.0);
        assert_eq!(segment.end, 2.0);
        assert_eq!(words[0].start, Some(0.2));
        assert_eq!(words[0].end, Some(1.8));
    }

    #[test]
    fn zero_duration_words_receive_reviewable_non_overlapping_spans() {
        let segment = TranscriptionSegment {
            text: "Um, today. You know.".into(),
            start: 4.8,
            end: 12.5,
        };
        let mut words = vec![
            word("Um", 5.0, 5.0),
            word(",", 5.0, 5.2),
            word("today", 5.2, 5.7),
            word("You", 11.8, 11.8),
            word("know", 11.8, 12.3),
            word(".", 12.3, 12.4),
        ];

        normalize_word_timings(&segment, &mut words);

        assert_eq!(
            words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Um", "today", "You", "know"]
        );
        assert_eq!(words[0].start, Some(5.0));
        assert_eq!(words[0].end, Some(5.2));
        assert_eq!(words[2].start, Some(11.8));
        assert!(words[2].end.unwrap() > 11.8);
        assert_eq!(words[2].end, words[3].start);
        assert_eq!(words[3].end, Some(12.3));
        assert!(words.windows(2).all(|pair| {
            pair[0].end.unwrap() <= pair[1].start.unwrap()
                && pair[0].end.unwrap() > pair[0].start.unwrap()
        }));
        assert!(words.last().unwrap().end.unwrap() > words.last().unwrap().start.unwrap());
    }
}
