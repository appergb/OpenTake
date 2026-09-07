//! Search index configuration constants and the model manifest — port of
//! `Search/SearchIndexConfig.swift`. The model is the ONNX build of
//! `siglip2-base-patch16-256` (dim 768, image 256, context 64). The
//! ONNX Community FP32 export is pinned by revision, byte size and SHA-256.

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
// v2 invalidates embeddings made with the old uncalibrated export contract.
pub const MODEL_VERSION: i32 = 2;
pub const EMBEDDING_DIM: usize = 768;
pub const IMAGE_SIZE: u32 = 256;
pub const CONTEXT_LENGTH: usize = 64;

/// Public ONNX Community export of Google's model, pinned to an immutable commit.
/// Provenance and verified hashes: docs/knowledge/2026-09-06-semantic-search-model.md.
pub const MODEL_DOWNLOAD_BASE_URL: &str =
    "https://huggingface.co/onnx-community/siglip2-base-patch16-256-ONNX/resolve/d1114256522a37ffa257a0a58017348ab0058db2";

/// The separate encoder graphs return unnormalized pooled features.
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

/// FP32 encoder assets (no external-data sidecars) and the matching tokenizer.
pub fn manifest() -> Manifest {
    Manifest {
        model: MODEL_NAME.to_string(),
        version: MODEL_VERSION,
        embedding_dim: EMBEDDING_DIM,
        image_size: IMAGE_SIZE,
        context_length: CONTEXT_LENGTH,
        image_encoder: ManifestFile {
            name: "onnx/vision_model.onnx".to_string(),
            sha256: "f5cb16728a704703f05516ded628397e11dbca4de2eb5db04b0c0bcee988aa7a".into(),
            bytes: 371_992_072,
        },
        text_encoder: ManifestFile {
            name: "onnx/text_model.onnx".to_string(),
            sha256: "d3de4a6bbbfcb429b6615ac496790353cf4a4fc0f19fbbe7179e523ae60daaef".into(),
            bytes: 1_129_469_657,
        },
        tokenizer: ManifestFile {
            name: "tokenizer.json".to_string(),
            sha256: "cb9140fae3ac5122c972d37adf83e1248471a38147ad76f8215c8872c6fd8322".into(),
            bytes: 34_363_039,
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
    #[test]
    fn public_manifest_has_pinned_revision_and_complete_checksums() {
        let revision = MODEL_DOWNLOAD_BASE_URL.rsplit('/').next().unwrap();
        assert_eq!(revision.len(), 40);
        assert!(revision.bytes().all(|b| b.is_ascii_hexdigit()));
        let m = manifest();
        let files = [&m.image_encoder, &m.text_encoder, &m.tokenizer];
        for file in files {
            assert!(file.bytes > 0);
            assert_eq!(file.sha256.len(), 64);
            assert!(file.sha256.bytes().all(|b| b.is_ascii_hexdigit()));
        }
        assert!(files.iter().map(|f| f.bytes).sum::<i64>() < 2_000_000_000);
        assert_eq!(m.spec(), embedder_spec());
    }
}
