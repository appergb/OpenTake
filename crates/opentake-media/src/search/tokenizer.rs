//! SigLIP text tokenizer wrapper. Port of `Search/Models/TextTokenizer.swift`.
//!
//! SigLIP was trained on `max_length`-padded sequences with **no attention
//! mask**, so we must reproduce the Python reference exactly: encode, truncate to
//! `context_length`, then right-pad with `pad_token = 0` to the fixed length. We
//! disable the tokenizer's own padding/truncation and do it manually so the
//! behavior is deterministic and unit-testable.

use std::path::Path;

use tokenizers::Tokenizer;

use crate::error::{MediaError, Result};

/// SigLIP pad token id (right-padding).
pub const PAD_TOKEN: i64 = 0;

/// Loaded SigLIP tokenizer with a fixed context length.
pub struct SiglipTokenizer {
    inner: Tokenizer,
    context_length: usize,
}

impl SiglipTokenizer {
    /// Load from a `tokenizer.json` file.
    pub fn from_file(path: &Path, context_length: usize) -> Result<Self> {
        let mut inner = Tokenizer::from_file(path)
            .map_err(|e| MediaError::ModelInstall(format!("tokenizer load: {e}")))?;
        // We pad/truncate manually; turn off the tokenizer's own behavior.
        inner.with_padding(None);
        let _ = inner.with_truncation(None);
        Ok(SiglipTokenizer {
            inner,
            context_length,
        })
    }

    pub fn context_length(&self) -> usize {
        self.context_length
    }

    /// Tokenize `text`: encode (with special tokens), truncate to
    /// `context_length`, right-pad with `PAD_TOKEN`. Output length is always
    /// exactly `context_length`. Verbatim port of `TextTokenizer.tokenize`.
    pub fn tokenize(&self, text: &str) -> Result<Vec<i64>> {
        let encoding = self
            .inner
            .encode(text, true)
            .map_err(|e| MediaError::ModelInstall(format!("tokenize: {e}")))?;
        Ok(pad_or_truncate(encoding.get_ids(), self.context_length))
    }
}

/// Pure: truncate `ids` to `len`, then right-pad with [`PAD_TOKEN`] to exactly
/// `len`. Split out so the SigLIP padding rule is unit-testable without a model.
pub fn pad_or_truncate(ids: &[u32], len: usize) -> Vec<i64> {
    let mut out: Vec<i64> = ids.iter().take(len).map(|&i| i as i64).collect();
    if out.len() < len {
        out.resize(len, PAD_TOKEN);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_short_sequence_to_context_length() {
        let out = pad_or_truncate(&[5, 6, 7], 8);
        assert_eq!(out, vec![5, 6, 7, 0, 0, 0, 0, 0]);
        assert_eq!(out.len(), 8);
    }

    #[test]
    fn truncates_long_sequence() {
        let ids: Vec<u32> = (1..=100).collect();
        let out = pad_or_truncate(&ids, 64);
        assert_eq!(out.len(), 64);
        assert_eq!(out[0], 1);
        assert_eq!(out[63], 64);
    }

    #[test]
    fn exact_length_unchanged() {
        let out = pad_or_truncate(&[1, 2, 3, 4], 4);
        assert_eq!(out, vec![1, 2, 3, 4]);
    }

    #[test]
    fn empty_input_is_all_pad() {
        let out = pad_or_truncate(&[], 5);
        assert_eq!(out, vec![0; 5]);
    }

    #[test]
    fn pad_token_is_zero() {
        assert_eq!(PAD_TOKEN, 0);
    }
}
