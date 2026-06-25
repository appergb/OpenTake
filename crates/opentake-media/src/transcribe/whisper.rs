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
            full_text.push_str(&seg_text);

            let t0 = state.full_get_segment_t0(i).unwrap_or(0);
            let t1 = state.full_get_segment_t1(i).unwrap_or(0);
            let trimmed = seg_text.trim();
            if !trimmed.is_empty() {
                segments.push(TranscriptionSegment {
                    text: trimmed.to_string(),
                    start: cs_to_secs(t0),
                    end: cs_to_secs(t1),
                });
            }

            let n_tokens = state.full_n_tokens(i).unwrap_or(0);
            for j in 0..n_tokens {
                let tok_text = match state.full_get_token_text(i, j) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let trimmed_tok = tok_text.trim();
                // Skip special tokens (whisper wraps them in [..] / <|..|>) and blanks.
                if trimmed_tok.is_empty()
                    || (trimmed_tok.starts_with("[_"))
                    || (trimmed_tok.starts_with("<|") && trimmed_tok.ends_with("|>"))
                {
                    continue;
                }
                let data = state.full_get_token_data(i, j).ok();
                let (start, end) = match data {
                    Some(d) => (Some(cs_to_secs(d.t0)), Some(cs_to_secs(d.t1))),
                    None => (None, None),
                };
                words.push(TranscriptionWord {
                    text: trimmed_tok.to_string(),
                    start,
                    end,
                });
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

    #[test]
    fn centiseconds_convert_to_seconds() {
        assert!((cs_to_secs(150) - 1.5).abs() < 1e-9);
        assert_eq!(cs_to_secs(0), 0.0);
    }
}
