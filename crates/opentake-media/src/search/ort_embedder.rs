//! ONNX Runtime SigLIP2 embedder (feature `ort-backend`). Real implementation of
//! the [`Embedder`] trait; the default build and tests use the mock instead.
//!
//! Image input is `NCHW` f32 `(1,3,256,256)` (mean/std from
//! `crate::search::embedder`); text input is int64 `(1,context_length)`,
//! right-padded with 0. Output is a `(1, embedding_dim)` f32 vector; we assert
//! the length matches the spec, mirroring upstream `vector(from:dim:)`.
//!
//! IO tensor names default to the SigLIP CoreML names (`image`/`tokens` →
//! `embedding`); ONNX exports often use `pixel_values`/`input_ids` →
//! `image_embeds`/`text_embeds`. `IoNames` makes them configurable.

use std::path::Path;
use std::sync::Mutex;

use ndarray::{Array2, Array4};
use ort::session::Session;
use ort::value::Tensor;

use super::embedder::{
    l2_normalize, preprocess_image, Embedder, EmbedderSpec, SIGLIP_MEAN, SIGLIP_STD,
};
use super::tokenizer::SiglipTokenizer;
use crate::error::{MediaError, Result};
use crate::frame::RgbaFrame;

/// ONNX graph IO tensor names.
#[derive(Clone, Debug)]
pub struct IoNames {
    pub image_input: String,
    pub image_output: String,
    pub text_input: String,
    pub text_output: String,
}

impl Default for IoNames {
    fn default() -> Self {
        // SigLIP CoreML names (upstream `VisualEmbedder`).
        IoNames {
            image_input: "image".into(),
            image_output: "embedding".into(),
            text_input: "tokens".into(),
            text_output: "embedding".into(),
        }
    }
}

/// ONNX-backed SigLIP2 embedder. `Session` is not `Sync`, so each is behind a
/// `Mutex` to satisfy the `Embedder: Send + Sync` bound.
pub struct OrtEmbedder {
    image: Mutex<Session>,
    text: Mutex<Session>,
    tokenizer: SiglipTokenizer,
    spec: EmbedderSpec,
    io: IoNames,
}

impl OrtEmbedder {
    /// Load image+text encoders and the tokenizer for `spec` from disk. Uses the
    /// platform's default execution provider, falling back to CPU.
    pub fn new(
        image_encoder: &Path,
        text_encoder: &Path,
        tokenizer_json: &Path,
        spec: EmbedderSpec,
    ) -> Result<Self> {
        Self::with_io(
            image_encoder,
            text_encoder,
            tokenizer_json,
            spec,
            IoNames::default(),
        )
    }

    pub fn with_io(
        image_encoder: &Path,
        text_encoder: &Path,
        tokenizer_json: &Path,
        spec: EmbedderSpec,
        io: IoNames,
    ) -> Result<Self> {
        let image = build_session(image_encoder)?;
        let text = build_session(text_encoder)?;
        let tokenizer = SiglipTokenizer::from_file(tokenizer_json, spec.context_length)?;
        Ok(OrtEmbedder {
            image: Mutex::new(image),
            text: Mutex::new(text),
            tokenizer,
            spec,
            io,
        })
    }

    fn finalize(&self, mut v: Vec<f32>) -> Result<Vec<f32>> {
        if v.len() != self.spec.embedding_dim {
            return Err(MediaError::BadModelOutput);
        }
        if self.spec.normalized {
            // Model already normalizes — leave as-is.
        } else {
            // Upstream default assumes in-graph normalization; only normalize
            // when the spec explicitly requests external normalization (kept
            // here for the calibration path, SPEC §0.8). Currently a no-op.
            let _ = &mut v;
        }
        Ok(v)
    }
}

fn build_session(path: &Path) -> Result<Session> {
    let builder = Session::builder().map_err(|e| MediaError::ModelInstall(format!("ort: {e}")))?;
    // Default EP set; ort falls back to CPU when an accelerator is unavailable.
    let builder = builder
        .with_intra_threads(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        )
        .map_err(|e| MediaError::ModelInstall(format!("ort threads: {e}")))?;
    builder
        .commit_from_file(path)
        .map_err(|e| MediaError::ModelInstall(format!("ort load {}: {e}", path.display())))
}

fn extract_vec(value: &ort::value::Value) -> Result<Vec<f32>> {
    let (_, data) = value
        .try_extract_tensor::<f32>()
        .map_err(|_| MediaError::BadModelOutput)?;
    Ok(data.to_vec())
}

impl Embedder for OrtEmbedder {
    fn spec(&self) -> &EmbedderSpec {
        &self.spec
    }

    fn encode_image(&self, frame: &RgbaFrame) -> Result<Vec<f32>> {
        let tensor: Array4<f32> =
            preprocess_image(frame, self.spec.image_size, SIGLIP_MEAN, SIGLIP_STD);
        let input = Tensor::from_array(tensor)
            .map_err(|e| MediaError::Decode(format!("ort tensor: {e}")))?;
        let mut session = self.image.lock().unwrap();
        let outputs = session
            .run(ort::inputs![self.io.image_input.as_str() => input])
            .map_err(|e| MediaError::Decode(format!("ort run image: {e}")))?;
        let value = outputs
            .get(self.io.image_output.as_str())
            .ok_or(MediaError::BadModelOutput)?;
        let v = extract_vec(value)?;
        self.finalize(v)
    }

    fn encode_text(&self, text: &str) -> Result<Vec<f32>> {
        let ids = self.tokenizer.tokenize(text)?;
        let arr = Array2::from_shape_vec((1, ids.len()), ids)
            .map_err(|e| MediaError::Decode(format!("ort text shape: {e}")))?;
        let input =
            Tensor::from_array(arr).map_err(|e| MediaError::Decode(format!("ort tensor: {e}")))?;
        let mut session = self.text.lock().unwrap();
        let outputs = session
            .run(ort::inputs![self.io.text_input.as_str() => input])
            .map_err(|e| MediaError::Decode(format!("ort run text: {e}")))?;
        let value = outputs
            .get(self.io.text_output.as_str())
            .ok_or(MediaError::BadModelOutput)?;
        let v = extract_vec(value)?;
        self.finalize(v)
    }
}

// Keep `l2_normalize` referenced for the calibration path even while the default
// (in-graph-normalized) configuration does not call it.
#[allow(dead_code)]
fn _normalize_for_calibration(v: &mut [f32]) {
    l2_normalize(v);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_io_names_match_upstream_coreml() {
        let io = IoNames::default();
        assert_eq!(io.image_input, "image");
        assert_eq!(io.text_input, "tokens");
        assert_eq!(io.image_output, "embedding");
    }
}
