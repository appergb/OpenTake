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

pub mod analysis;
pub mod cache_key;
pub mod cancel;
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
pub mod timecode;
pub mod transcribe;
pub mod waveform;

use std::path::{Path, PathBuf};

/// Materialize an exact visible source range as an uploadable MP4. This is used
/// by generation/upscale when `sourceClipId` is supplied, so provider uploads
/// receive the clip's trimmed source window instead of the complete asset.
pub fn trim_video_range(
    source: &Path,
    destination: &Path,
    start_seconds: f64,
    end_seconds: f64,
    cancel: &MediaCancelToken,
) -> Result<()> {
    if !start_seconds.is_finite()
        || !end_seconds.is_finite()
        || start_seconds < 0.0
        || end_seconds <= start_seconds
    {
        return Err(MediaError::Ffmpeg(
            "invalid trimmed generation source range".to_string(),
        ));
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let duration = end_seconds - start_seconds;
    let mut child = std::process::Command::new(ff::ffmpeg_path())
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .arg("-ss")
        .arg(format!("{start_seconds:.6}"))
        .arg("-i")
        .arg(source)
        .arg("-t")
        .arg(format!("{duration:.6}"))
        .args([
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "18",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
        ])
        .arg(destination)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| MediaError::Ffmpeg(format!("trim source spawn: {error}")))?;
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(destination);
            return Err(MediaError::Cancelled);
        }
        if let Some(status) = child.try_wait()? {
            if !status.success() || !destination.is_file() {
                let _ = std::fs::remove_file(destination);
                return Err(MediaError::Ffmpeg(
                    "trimmed generation source could not be materialized".to_string(),
                ));
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

// --- flat re-exports of the public API ---

pub use cancel::MediaCancelToken;
pub use error::{MediaError, Result};
pub use frame::RgbaFrame;

pub use probe::{probe, MediaProbe};

pub use decode::{
    decode_frame_at, decode_frame_at_cancellable, decode_frames_at, decode_frames_at_cancellable,
    decode_pcm_interleaved, decode_pcm_interleaved_cancellable, extract_pcm,
    extract_pcm_cancellable, extract_pcm_cancellable_with_progress, FrameRequest, PcmBuffer,
    PcmFormat, PcmProgressCallback, PcmSpec, StreamDecodeControl, StreamVideoFrame, VideoStream,
    VideoStreamRequest, DEFAULT_VIDEO_STREAM_QUEUE_CAPACITY,
};

pub use encode::{ExportPreset, ExportResolution, VideoCodec, VideoEncoder};

pub use thumbnail::{
    capture_project_thumbnail, image_thumbnail, pick_thumbnail_source, video_thumbnail_times,
    video_thumbnails, PartialThumbCallback, ThumbnailCacheMeta, ThumbnailKind, ThumbnailSource,
    VideoThumb,
};

pub use timecode::{parse_smpte_timecode, read_start_timecode_frame};

pub use waveform::{
    waveform, waveform_cached, waveform_cached_cancellable, waveform_cancellable,
    waveform_sample_count,
};

pub use transcribe::{
    cache::TranscriptCache,
    captions::{
        caption_specs, dominant_speech_track, CaptionCase, CaptionClipSpec, CaptionTarget, Phrase,
        MIN_DISPLAY_DURATION_SECS,
    },
    languages::{match_language, WHISPER_LANGUAGES},
    model::{self as whisper_model, WhisperModel, DEFAULT_MODEL as DEFAULT_WHISPER_MODEL},
    search::{search as search_spoken, SpokenHit},
    timeline::{
        span_frames, timeline_transcript, ClipFragment, ClipTranscript, TimelineTranscript,
        WordRow, TIMELINE_MAX_WORDS,
    },
    TranscribeOptions, Transcriber, TranscriptionResult, TranscriptionSegment, TranscriptionWord,
};

#[cfg(feature = "whisper-backend")]
pub use transcribe::whisper::WhisperTranscriber;

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

    /// Probe through an already-open regular-file handle.
    pub fn probe_file(&self, file: &std::fs::File) -> Result<MediaProbe> {
        probe::probe_file(file)
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

    /// Extract the audio track from `input` into `output` as a self-contained
    /// audio file. The container/codec is picked from the output extension:
    /// `.m4a` → AAC in MP4, `.mp3` → libmp3lame, `.wav` → PCM s16le. Video is
    /// dropped (`-vn`). Streams the mux directly (input file → output file),
    /// never holding the full audio in memory — suitable for long sources.
    ///
    /// Returns the output path on success. Errors bubble up as `MediaError::Ffmpeg`
    /// when ffmpeg is missing, exits non-zero, or the extension is unsupported.
    pub fn extract_audio(&self, input: &Path, output: &Path) -> Result<std::path::PathBuf> {
        extract_audio_file(input, output).map(|_| output.to_path_buf())
    }
}

/// Pick the ffmpeg codec args for an audio output extension (Issue #39).
///
/// Returns `None` for unsupported extensions so the caller can surface a
/// friendly error before spawning ffmpeg. The table matches the save-dialog
/// filters in `MediaPanel.tsx` (`.m4a` / `.mp3` / `.wav`) plus the closely
/// related `.m4r` (ringtone) and `.aac` (raw AAC) containers, all of which
/// the AAC encoder can mux.
///
/// Extracted as a pure function so the codec selection can be unit-tested
/// without ffmpeg on PATH (review #3 — "缺 issue #39 验收测试").
fn audio_codec_args(ext: &str) -> Option<Vec<&'static str>> {
    match ext {
        "m4a" | "m4r" | "aac" => Some(vec!["-c:a", "aac", "-b:a", "192k"]),
        "mp3" => Some(vec!["-c:a", "libmp3lame", "-b:a", "192k"]),
        "wav" => Some(vec!["-c:a", "pcm_s16le"]),
        _ => None,
    }
}

/// Run `ffmpeg -y -i <input> -vn <codec args> <output>` to mux the audio track
/// into a standalone file. Codec is selected by `output`'s extension so the
/// caller just picks a save-path filter in the native dialog and the right
/// encoder falls out. `-y` overwrites (the save dialog already confirmed).
fn extract_audio_file(input: &Path, output: &Path) -> Result<()> {
    let ext = output.extension().and_then(|e| e.to_str()).ok_or_else(|| {
        MediaError::Ffmpeg("output path has no extension (use .m4a, .mp3, or .wav)".into())
    })?;
    let codec_args = audio_codec_args(ext).ok_or_else(|| {
        MediaError::Ffmpeg(format!(
            "unsupported audio extension: .{ext} (use m4a, mp3, or wav)"
        ))
    })?;

    let mut cmd = std::process::Command::new(ff::ffmpeg_path());
    cmd.arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-vn")
        .args(&codec_args)
        .arg(output);

    let out = cmd
        .output()
        .map_err(|e| MediaError::Ffmpeg(format!("ffmpeg spawn: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(MediaError::Ffmpeg(format!(
            "ffmpeg exited {}{}",
            out.status,
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", stderr.trim())
            }
        )));
    }
    Ok(())
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

    // --- extract_audio codec selection (Issue #39 review #3) ---
    //
    // `audio_codec_args` is a pure function: no ffmpeg spawn, no filesystem
    // access. These tests run on every platform without any binary on PATH.

    #[test]
    fn audio_codec_args_picks_aac_for_m4a_family() {
        for ext in ["m4a", "m4r", "aac"] {
            let args = audio_codec_args(ext).unwrap_or_else(|| panic!(".{ext}"));
            assert_eq!(args, ["-c:a", "aac", "-b:a", "192k"], "mismatch for .{ext}");
        }
    }

    #[test]
    fn audio_codec_args_picks_lame_for_mp3() {
        assert_eq!(
            audio_codec_args("mp3").unwrap(),
            ["-c:a", "libmp3lame", "-b:a", "192k"]
        );
    }

    #[test]
    fn audio_codec_args_picks_pcm_for_wav() {
        assert_eq!(audio_codec_args("wav").unwrap(), ["-c:a", "pcm_s16le"]);
    }

    #[test]
    fn audio_codec_args_rejects_unknown_extensions() {
        // Video containers + empty + uppercase (extension matching is
        // case-sensitive by design — the save dialog emits lowercase).
        for ext in ["mp4", "mov", "", "M4A"] {
            assert!(
                audio_codec_args(ext).is_none(),
                ".{ext:?} should not map to a codec"
            );
        }
    }

    /// End-to-end verification of Issue #39 acceptance criteria:
    /// 1. the output file exists after extraction;
    /// 2. its duration matches the input (within 0.5s);
    /// 3. it contains no video stream.
    ///
    /// Requires `ffmpeg` + `ffprobe` on PATH; auto-skips when either is
    /// unavailable (Windows local dev has neither, CI Linux has both — see
    /// `.github/workflows/ci.yml` `Install system deps`). Run explicitly with
    /// `cargo test -p opentake-media --ignored extract_audio`.
    #[test]
    #[ignore = "requires ffmpeg + ffprobe on PATH; run with --ignored"]
    fn extract_audio_file_produces_audio_only_output_matching_input_duration() {
        use std::process::Command;
        // Skip when ffmpeg/ffprobe unavailable.
        if Command::new(ff::ffmpeg_path())
            .arg("-version")
            .output()
            .is_err()
        {
            eprintln!("skipping: ffmpeg unavailable");
            return;
        }
        if Command::new("ffprobe").arg("-version").output().is_err() {
            eprintln!("skipping: ffprobe unavailable");
            return;
        }

        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("src.mp4");
        let output = tmp.path().join("out.m4a");

        // Generate a 2s fixture: 320x240 black video + 440Hz sine audio.
        // `-shortest` trims to the shorter stream so both are exactly 2s.
        let gen = Command::new(ff::ffmpeg_path())
            .arg("-y")
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=2"])
            .args(["-f", "lavfi", "-i", "color=size=320x240:rate=24:duration=2"])
            .args(["-c:a", "aac"])
            .args(["-c:v", "libx264"])
            .arg("-shortest")
            .arg(&input)
            .output()
            .expect("ffmpeg fixture gen spawn failed");
        assert!(
            gen.status.success(),
            "fixture gen failed: {}",
            String::from_utf8_lossy(&gen.stderr)
        );

        // Run the extraction under test.
        extract_audio_file(&input, &output).expect("extract_audio_file failed");

        // #1: output exists.
        assert!(
            output.is_file(),
            "output file not created: {}",
            output.display()
        );

        // Helper: probe duration (seconds) of a file via ffprobe.
        let probe_duration = |path: &Path| -> f64 {
            let out = Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-show_entries",
                    "format=duration",
                    "-of",
                    "csv=p=0",
                ])
                .arg(path)
                .output()
                .expect("ffprobe spawn failed");
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse::<f64>()
                .expect("ffprobe returned non-numeric duration")
        };

        // #2: duration matches input (within 0.5s — muxing overhead can shift
        // by a few hundred ms).
        let dur_in = probe_duration(&input);
        let dur_out = probe_duration(&output);
        assert!(
            (dur_in - dur_out).abs() < 0.5,
            "duration mismatch: in={dur_in} out={dur_out}"
        );

        // #3: no video stream in output (`-vn` must have dropped it).
        let v_streams = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v",
                "-show_entries",
                "stream=index",
                "-of",
                "csv=p=0",
            ])
            .arg(&output)
            .output()
            .expect("ffprobe spawn failed");
        let v_streams = String::from_utf8_lossy(&v_streams.stdout);
        assert!(
            v_streams.trim().is_empty(),
            "output has video stream(s): {}",
            v_streams.trim()
        );
    }

    #[test]
    fn trim_video_range_materializes_only_visible_window_and_honors_cancel() {
        use std::process::Command;
        if !ff::ffmpeg_available() || !ff::ffprobe_available() {
            eprintln!("skipping: ffmpeg/ffprobe unavailable");
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.mp4");
        let trimmed = temp.path().join("trimmed.mp4");
        let generated = Command::new(ff::ffmpeg_path())
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(["-f", "lavfi", "-i", "color=size=64x48:rate=24:duration=3"])
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&source)
            .output()
            .unwrap();
        assert!(generated.status.success());
        let source_before = std::fs::read(&source).unwrap();

        trim_video_range(&source, &trimmed, 0.75, 1.75, &MediaCancelToken::new()).unwrap();
        let trimmed_probe = probe(&trimmed).unwrap();
        assert!((trimmed_probe.duration_secs - 1.0).abs() < 0.2);
        assert_eq!(std::fs::read(&source).unwrap(), source_before);

        let cancelled_path = temp.path().join("cancelled.mp4");
        let cancel = MediaCancelToken::new();
        cancel.cancel();
        assert!(matches!(
            trim_video_range(&source, &cancelled_path, 0.0, 2.0, &cancel),
            Err(MediaError::Cancelled)
        ));
        assert!(!cancelled_path.exists());
    }
}
