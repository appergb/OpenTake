//! Visual semantic search (SigLIP2 dual-encoder). Faithful port of the upstream
//! `Search/` subtree:
//! - [`embedder`]: `Embedder` trait + spec + squash-resize preprocessing.
//! - [`tokenizer`]: SigLIP fixed-length right-padded tokenize.
//! - [`frame_sampler`]: luma scene-change frame sampling.
//! - [`indexer`]: per-asset frame → embedding → store (idempotent).
//! - [`embed_store`]: `PALMEMB1` binary format (f16 disk / f32 memory).
//! - [`ranker`]: matrix·vector scoring + best-per-shot + cutoff.
//! - [`model_download`]: weight download/verify/install.
//! - [`config`]: thresholds + model manifest.
//!
//! The ONNX backend ([`OrtEmbedder`]) is behind feature `ort-backend`; tests use
//! a mock embedder.

pub mod config;
pub mod embed_store;
pub mod embedder;
pub mod frame_sampler;
pub mod indexer;
pub mod model_download;
pub mod ranker;
pub mod tokenizer;

#[cfg(feature = "ort-backend")]
pub mod ort_embedder;

pub use embed_store::{AssetIndex, Header, Row};
pub use embedder::{Embedder, EmbedderSpec};
pub use frame_sampler::{SampledFrame, SamplerOptions};
pub use indexer::{index_image, index_video, needs_index, CancelToken};
pub use ranker::{search as rank, Hit};
pub use tokenizer::SiglipTokenizer;

#[cfg(feature = "ort-backend")]
pub use ort_embedder::{IoNames, OrtEmbedder};

#[cfg(test)]
mod integration_tests {
    //! End-to-end (mock) index → rank flow, fully offline.
    use super::*;
    use crate::frame::RgbaFrame;
    use embedder::test_support::MockEmbedder;

    #[test]
    fn index_then_rank_finds_brightest_match() {
        let dir = tempfile::tempdir().unwrap();
        let embedder = MockEmbedder::small();

        // Two "image assets": a bright (white-ish) one and a dark one.
        let bright_file = make_media(&dir, "bright.png");
        let dark_file = make_media(&dir, "dark.png");
        let bright = RgbaFrame::new(8, 8, vec![240; 8 * 8 * 4]);
        let dark = RgbaFrame::new(8, 8, {
            let mut v = vec![0u8; 8 * 8 * 4];
            for px in v.chunks_exact_mut(4) {
                px[3] = 255;
            }
            v
        });
        let cancel = CancelToken::new();
        index_image(dir.path(), &bright_file, &bright, &embedder, &cancel).unwrap();
        index_image(dir.path(), &dark_file, &dark, &embedder, &cancel).unwrap();

        // Load both indexes.
        let bk = embed_store::key(&bright_file).unwrap();
        let dk = embed_store::key(&dark_file).unwrap();
        let indexes = vec![
            (
                "bright".to_string(),
                embed_store::load(dir.path(), &bk).unwrap(),
            ),
            (
                "dark".to_string(),
                embed_store::load(dir.path(), &dk).unwrap(),
            ),
        ];

        // Query with the bright image's own embedding → it must rank first.
        let q = embedder.encode_image(&bright).unwrap();
        let hits = rank(&q, &indexes, 20, 0.0, None);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].asset_id, "bright");
    }

    fn make_media(dir: &tempfile::TempDir, name: &str) -> std::path::PathBuf {
        let p = dir.path().join(name);
        std::fs::write(&p, b"fake image bytes").unwrap();
        p
    }
}
