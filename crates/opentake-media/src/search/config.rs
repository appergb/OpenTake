//! Search index configuration constants and the model manifest — port of
//! `Search/SearchIndexConfig.swift`. The model is the ONNX build of
//! `siglip2-base-patch16-256` (dim 768, image 256, context 64); upstream's
//! CoreML hashes/bytes are replaced when the ONNX assets are hosted (SPEC T8.0).

use crate::search::embedder::EmbedderSpec;
use crate::search::model_download::{Manifest, ManifestFile};

/// Absolute cosine floor for a visual match (upstream `visualMatchCosineFloor`).
pub const VISUAL_MATCH_COSINE_FLOOR: f32 = 0.05;
/// Relative score cutoff vs. the top hit (`VisualSearch.search` default).
pub const RELATIVE_CUTOFF: f32 = 0.85;
/// Default result limit.
pub const SEARCH_LIMIT: usize = 20;

/// SigLIP2 model identity.
pub const MODEL_NAME: &str = "siglip2-base-patch16-256";
pub const MODEL_VERSION: i32 = 1;
pub const EMBEDDING_DIM: usize = 768;
pub const IMAGE_SIZE: u32 = 256;
pub const CONTEXT_LENGTH: usize = 64;

/// The [`EmbedderSpec`] for the configured SigLIP2 model. `normalized` defaults
/// to `false` to match upstream's assumption that the exported model L2-
/// normalizes internally (SPEC §0.8); flip it only if calibration proves the
/// embeddings need external normalization.
pub fn embedder_spec() -> EmbedderSpec {
    EmbedderSpec {
        model: MODEL_NAME.to_string(),
        version: MODEL_VERSION,
        embedding_dim: EMBEDDING_DIM,
        image_size: IMAGE_SIZE,
        context_length: CONTEXT_LENGTH,
        normalized: false,
    }
}

/// The download manifest for the ONNX model assets. The `sha256`/`bytes` are
/// placeholders until the ONNX build is hosted (SPEC T8.0); they are validated
/// at download time and must be filled before enabling real downloads.
pub fn manifest() -> Manifest {
    Manifest {
        model: MODEL_NAME.to_string(),
        version: MODEL_VERSION,
        embedding_dim: EMBEDDING_DIM,
        image_size: IMAGE_SIZE,
        context_length: CONTEXT_LENGTH,
        image_encoder: ManifestFile {
            name: "image_encoder.onnx".to_string(),
            sha256: String::new(),
            bytes: 0,
        },
        text_encoder: ManifestFile {
            name: "text_encoder.onnx".to_string(),
            sha256: String::new(),
            bytes: 0,
        },
        tokenizer: ManifestFile {
            name: "tokenizer.zip".to_string(),
            sha256: String::new(),
            bytes: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_upstream() {
        assert_eq!(VISUAL_MATCH_COSINE_FLOOR, 0.05);
        assert_eq!(RELATIVE_CUTOFF, 0.85);
        assert_eq!(SEARCH_LIMIT, 20);
        assert_eq!(EMBEDDING_DIM, 768);
        assert_eq!(IMAGE_SIZE, 256);
        assert_eq!(CONTEXT_LENGTH, 64);
    }

    #[test]
    fn embedder_spec_is_consistent() {
        let s = embedder_spec();
        assert_eq!(s.model, MODEL_NAME);
        assert_eq!(s.embedding_dim, EMBEDDING_DIM);
        assert_eq!(s.image_size, IMAGE_SIZE);
        assert_eq!(s.context_length, CONTEXT_LENGTH);
        assert!(!s.normalized);
    }

    #[test]
    fn manifest_carries_model_identity() {
        let m = manifest();
        assert_eq!(m.model, MODEL_NAME);
        assert_eq!(m.version, MODEL_VERSION);
        assert_eq!(m.embedding_dim, EMBEDDING_DIM);
    }
}
