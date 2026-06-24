//! opentake-media — the media read & offline-analysis layer.
//!
//! Ports PalmierPro's AVFoundation / DSWaveformImage / macOS-Speech / CoreML
//! media stack to cross-platform Rust:
//! - **probe / decode / encode**: system ffmpeg CLI via [`ff`] (no libav* link).
//! - **thumbnails**: seek-decode + JPEG sprite-grid disk cache.
//! - **waveform**: Symphonia PCM decode → RMS downsample → normalized buckets.
//! - **transcribe**: `Transcriber` trait (+ data model, locale, cache, search);
//!   real backend is whisper.cpp behind feature `whisper-backend`.
//! - **search**: SigLIP2 visual semantic search (`Embedder` trait + indexer +
//!   `PALMEMB1` store + ranker); real backend is ONNX Runtime behind feature
//!   `ort-backend`.
//! - **ort_worker**: generic ONNX inference surface for advanced AI features.
//!
//! Design rules (see `docs/specs/media-SPEC.md`): seconds (f64) at every IO
//! boundary; zero hardcoded thresholds (all named constants/`Options`); cache
//! keys & on-disk formats byte-compatible with upstream; heavy ML behind
//! features so the default build/test is fully offline and links no native ML.
//!
//! ## Why ffmpeg over the CLI
//! The local toolchain is ffmpeg 8.1 (libavcodec 62), which the C-binding crates
//! (`ffmpeg-next` / `ffmpeg-the-third`) do not support, and `pkg-config` is
//! absent. `ffmpeg-sidecar` drives the binaries on `PATH` — zero native linkage
//! and a clean cross-platform build — so it is the chosen backend (SPEC §1.2,
//! "若 ffmpeg-next 不支持 8.x … 改用 ffmpeg-sidecar").

mod ff;

pub mod cache_key;
pub mod decode;
pub mod encode;
pub mod error;
pub mod frame;
pub mod index_coordinator;
pub mod library;
pub mod ort_worker;
pub mod probe;
pub mod search;
pub mod thumbnail;
pub mod transcribe;
pub mod waveform;

use std::path::{Path, PathBuf};

// --- flat re-exports of the public API ---

pub use error::{MediaError, Result};
pub use frame::RgbaFrame;

pub use probe::{probe, MediaProbe};

pub use decode::{
    decode_frame_at, decode_frames_at, extract_pcm, FrameRequest, PcmBuffer, PcmFormat, PcmSpec,
};

pub use encode::{ExportPreset, ExportResolution, VideoCodec, VideoEncoder};

pub use thumbnail::{
    image_thumbnail, video_thumbnail_times, video_thumbnails, PartialThumbCallback,
    ThumbnailCacheMeta, VideoThumb,
};

pub use waveform::{waveform, waveform_cached, waveform_sample_count};

pub use transcribe::{
    cache::TranscriptCache,
    search::{search as search_spoken, SpokenHit},
    TranscribeOptions, Transcriber, TranscriptionResult, TranscriptionSegment, TranscriptionWord,
};

pub use search::{
    rank as search_visual_ranked, AssetIndex, CancelToken, Embedder, EmbedderSpec, Hit,
    SamplerOptions,
};

pub use index_coordinator::{work_needed, ExportPause, IndexProgress, WorkNeeded};

pub use ort_worker::ExecutionProvider;

/// ffmpeg/ffprobe availability probes (re-exported for integration tests and
/// host-capability checks).
pub mod ffmpeg_status {
    pub use crate::ff::{ffmpeg_available, ffprobe_available};
}

/// Facade bundling the media engine's roots for `opentake-core` (SPEC §8.4).
/// Holds the cache and model directories and exposes the high-level operations
/// over plain value types (`RgbaFrame` / `PcmBuffer` / domain types). Heavy
/// ML-backed methods accept the backend the caller constructed (a feature-gated
/// implementation or a mock), keeping this struct backend-free.
pub struct MediaEngine {
    cache_root: PathBuf,
    models_dir: PathBuf,
    export_pause: ExportPause,
}

impl MediaEngine {
    /// Construct with the cache root (Tauri `app_cache_dir`) and model directory
    /// (Tauri `app_data_dir`).
    pub fn new(cache_root: impl Into<PathBuf>, models_dir: impl Into<PathBuf>) -> Self {
        MediaEngine {
            cache_root: cache_root.into(),
            models_dir: models_dir.into(),
            export_pause: ExportPause::new(),
        }
    }

    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }
    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// Probe a media file's metadata.
    pub fn probe(&self, path: &Path) -> Result<MediaProbe> {
        probe::probe(path)
    }

    /// Generate (and cache) a video thumbnail sequence.
    pub fn video_thumbnails(
        &self,
        path: &Path,
        duration_secs: f64,
        on_partial: Option<PartialThumbCallback<'_>>,
    ) -> Result<Vec<VideoThumb>> {
        thumbnail::video_thumbnails(&self.cache_root, path, duration_secs, on_partial)
    }

    /// Decode a single image thumbnail (long edge ≤ 120 px).
    pub fn image_thumbnail(&self, path: &Path) -> Result<RgbaFrame> {
        thumbnail::image_thumbnail(path, thumbnail::IMAGE_THUMB_MAX_PIXEL)
    }

    /// Generate (and cache) a normalized waveform.
    pub fn waveform(&self, path: &Path, duration_secs: f64) -> Result<Vec<f32>> {
        waveform::waveform_cached(&self.cache_root, path, duration_secs)
    }

    /// Transcribe a file via the provided backend, caching the full transcript.
    pub fn transcribe(
        &self,
        path: &Path,
        is_video: bool,
        range: Option<(f64, f64)>,
        transcriber: &dyn Transcriber,
        cache: &TranscriptCache,
    ) -> Result<TranscriptionResult> {
        cache.transcript(path, is_video, range, transcriber)
    }

    /// Keyword search over cached transcripts.
    pub fn search_spoken(
        &self,
        query: &str,
        assets: &[(String, PathBuf)],
        limit: usize,
    ) -> Vec<SpokenHit> {
        transcribe::search::search(&self.cache_root, query, assets, limit)
    }

    /// The shared export-pause signal; `opentake-render` calls `begin`/`end`
    /// around exports so background indexing yields.
    pub fn export_pause(&self) -> ExportPause {
        self.export_pause.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_engine_exposes_roots() {
        let e = MediaEngine::new("/cache", "/models");
        assert_eq!(e.cache_root(), Path::new("/cache"));
        assert_eq!(e.models_dir(), Path::new("/models"));
    }

    #[test]
    fn export_pause_is_shared_from_engine() {
        let e = MediaEngine::new("/c", "/m");
        let p = e.export_pause();
        p.begin();
        assert!(e.export_pause().is_active());
        p.end();
        assert!(!e.export_pause().is_active());
    }

    #[test]
    fn spoken_search_over_engine_uses_cache_root() {
        let dir = tempfile::tempdir().unwrap();
        let e = MediaEngine::new(dir.path(), dir.path());
        // No transcripts on disk → empty.
        assert!(e.search_spoken("anything", &[], 10).is_empty());
    }

    #[test]
    fn crate_public_types_are_reachable() {
        // Smoke: the flat re-exports compile and name the right types.
        let _: Option<MediaProbe> = None;
        let _: Option<RgbaFrame> = None;
        let _: Option<TranscriptionResult> = None;
        let _: Option<Hit> = None;
        let _ = PcmFormat::F32;
        let _ = VideoCodec::H264;
    }
}
