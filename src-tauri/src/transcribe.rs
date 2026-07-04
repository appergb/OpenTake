//! Transcription command surface + shared backend helpers.
//!
//! Wires the built-but-previously-unreachable whisper.cpp transcription engine
//! (`opentake_media::transcribe`) to the app. Upstream
//! (`Transcription/Transcription.swift`) uses Apple's on-device
//! `SpeechTranscriber` + `AssetInventory` auto-install; OpenTake substitutes
//! whisper.cpp, so the model is an explicit ggml file the user downloads once.
//!
//! Commands:
//! - [`transcribe_model_status`] — is the whisper model installed? (+ its label
//!   / size, for a download prompt).
//! - [`download_transcribe_model`] — async download with `transcribe://progress`
//!   events (mirrors `export_video`'s progress pattern), SHA-1 verified.
//! - [`transcribe_media`] — transcribe one asset → transcript DTO (segments +
//!   words with times); cached so a re-transcribe is instant. Runs the blocking
//!   whisper inference on a worker thread (the command itself is sync, so Tauri
//!   already dispatches it off the UI thread).
//! - [`transcript_get`] — return a cached transcript if present, without
//!   transcribing (for the UI to check state).
//!
//! DTOs are camelCase (`web/src/lib/types.ts` contract; the repo's #1 bug class),
//! with serde round-trip tests.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use opentake_core::AppCore;
use opentake_domain::{ClipType, MediaResolver};
use opentake_media::{
    whisper_model, MediaEngine, TranscribeOptions, TranscriptCache, TranscriptionResult,
    WhisperTranscriber, DEFAULT_WHISPER_MODEL,
};

use crate::media::MediaState;

/// One word/token with optional source-seconds timing. camelCase DTO.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptWordDto {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
}

/// One endpointed segment (sentence/pause boundary) with source-seconds times.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegmentDto {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

/// A full transcript in source seconds, projected to the front end. Mirrors
/// [`TranscriptionResult`] with camelCase fields.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDto {
    /// The asset id this transcript is for.
    pub media_id: String,
    /// Full concatenated text.
    pub text: String,
    /// BCP-47 / ISO-639 language, when the backend reports one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Sentence-level segments (source seconds).
    pub segments: Vec<TranscriptSegmentDto>,
    /// Per-word timings (source seconds); may be empty.
    pub words: Vec<TranscriptWordDto>,
}

impl TranscriptDto {
    fn from_result(media_id: &str, r: TranscriptionResult) -> Self {
        TranscriptDto {
            media_id: media_id.to_string(),
            text: r.text,
            language: r.language,
            segments: r
                .segments
                .into_iter()
                .map(|s| TranscriptSegmentDto {
                    text: s.text,
                    start: s.start,
                    end: s.end,
                })
                .collect(),
            words: r
                .words
                .into_iter()
                .map(|w| TranscriptWordDto {
                    text: w.text,
                    start: w.start,
                    end: w.end,
                })
                .collect(),
        }
    }
}

/// Whether the whisper model is installed, plus enough info to prompt a download.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatusDto {
    /// True when the ggml model file is present on disk.
    pub installed: bool,
    /// Human label for the model (`"base (multilingual)"`).
    pub model: String,
    /// Approximate download size in bytes (for the prompt).
    pub bytes: u64,
}

/// Progress payload for the `transcribe://progress` event during a model
/// download: `fraction` in `0.0..=1.0`. Mirrors `export_video`'s throttled event
/// shape (here the reqwest stream drives it directly).
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    fraction: f64,
}

/// `transcribe_model_status`: report whether the whisper ggml model is installed.
/// Never downloads. The UI calls this before transcribing to decide whether to
/// prompt for a one-time download.
#[tauri::command]
pub fn transcribe_model_status(media: State<'_, MediaState>) -> ModelStatusDto {
    let models_dir = media.engine().models_dir();
    ModelStatusDto {
        installed: whisper_model::installed(models_dir, &DEFAULT_WHISPER_MODEL).is_some(),
        model: DEFAULT_WHISPER_MODEL.label.to_string(),
        bytes: DEFAULT_WHISPER_MODEL.bytes,
    }
}

/// `download_transcribe_model`: fetch the whisper ggml model (idempotent), emit
/// `transcribe://progress` events as bytes arrive, and SHA-1-verify before
/// installing. Async so it runs on Tauri's runtime without blocking the UI (the
/// download is network-bound). Returns the installed status on success.
#[tauri::command]
pub async fn download_transcribe_model(
    app: AppHandle,
    media: State<'_, MediaState>,
) -> Result<ModelStatusDto, String> {
    let models_dir = media.engine().models_dir().to_path_buf();
    let on_progress = |fraction: f64| {
        let _ = app.emit("transcribe://progress", DownloadProgress { fraction });
    };
    whisper_model::download(&models_dir, &DEFAULT_WHISPER_MODEL, on_progress)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ModelStatusDto {
        installed: true,
        model: DEFAULT_WHISPER_MODEL.label.to_string(),
        bytes: DEFAULT_WHISPER_MODEL.bytes,
    })
}

/// `transcribe_media`: transcribe one asset and return its transcript. Cached via
/// [`TranscriptCache`], so a repeat call (or a prior `get_transcript`) is instant.
/// `language` is an optional BCP-47/ISO-639 hint; omit for auto-detect. Errors if
/// the model isn't installed (guiding the UI to `download_transcribe_model`) or
/// the asset can't be resolved/decoded.
#[tauri::command]
pub fn transcribe_media(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    media_id: String,
    language: Option<String>,
) -> Result<TranscriptDto, String> {
    let (path, is_video) = resolve_asset(&core, &media_id)?;
    let result = transcribe_with_cache(media.engine(), &path, is_video, language.as_deref())?;
    Ok(TranscriptDto::from_result(&media_id, result))
}

/// `transcript_get`: return the cached transcript for an asset if one exists on
/// disk, else `null`. Never transcribes — the UI uses it to show existing state
/// without triggering work.
#[tauri::command]
pub fn transcript_get(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    media_id: String,
) -> Result<Option<TranscriptDto>, String> {
    let (path, _is_video) = resolve_asset(&core, &media_id)?;
    let cache_root = media.engine().cache_root();
    Ok(
        opentake_media::transcribe::cache::cached_on_disk(cache_root, &path)
            .map(|r| TranscriptDto::from_result(&media_id, r)),
    )
}

/// Resolve an asset id to `(source_path, is_video)` from the live manifest.
/// `is_video` drives audio extraction (video → extract audio track first).
pub(crate) fn resolve_asset(core: &AppCore, media_id: &str) -> Result<(PathBuf, bool), String> {
    let manifest = core.media();
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.id == media_id)
        .ok_or_else(|| format!("media not found: {media_id}"))?;
    let is_video = entry.kind == ClipType::Video;
    let project_dir = core.project_dir();
    let path = MediaResolver::new(&manifest, project_dir.as_deref())
        .expected_path(media_id)
        .ok_or_else(|| format!("could not resolve a file path for media: {media_id}"))?;
    if !path.exists() {
        return Err(format!(
            "media file is offline (relink required): {}",
            path.display()
        ));
    }
    Ok((path, is_video))
}

/// Load the whisper backend from the installed default model, or a structured
/// "model not installed" error the UI can turn into a download prompt.
pub(crate) fn load_backend(engine: &MediaEngine) -> Result<WhisperTranscriber, String> {
    let models_dir = engine.models_dir();
    let model_path =
        whisper_model::installed(models_dir, &DEFAULT_WHISPER_MODEL).ok_or_else(|| {
            format!(
                "transcription model not installed — download '{}' first",
                DEFAULT_WHISPER_MODEL.label
            )
        })?;
    WhisperTranscriber::from_model_path(&model_path).map_err(|e| e.to_string())
}

/// Transcribe `path` with the whisper backend, caching the full transcript.
/// `is_video` selects audio extraction; `language` is an optional hint. The
/// whisper inference is CPU-bound and blocking; the Tauri command wrapper is sync
/// so Tauri already runs it on a worker thread (no UI stall).
///
/// A cached full transcript short-circuits before the backend loads (so re-reads
/// don't even need the model installed). On a miss, the backend transcribes the
/// full file — with the language hint threaded through [`TranscribeOptions`] —
/// and the result is persisted into the on-disk cache layout so subsequent
/// `transcribe_media` / `get_transcript` / spoken-search calls hit instantly.
pub(crate) fn transcribe_with_cache(
    engine: &MediaEngine,
    path: &Path,
    is_video: bool,
    language: Option<&str>,
) -> Result<TranscriptionResult, String> {
    let cache_root = engine.cache_root();
    // Fast path: a cached full transcript needs no backend.
    if let Some(cached) = opentake_media::transcribe::cache::cached_on_disk(cache_root, path) {
        return Ok(cached);
    }
    let backend = load_backend(engine)?;

    // No language hint → the cache convenience (default options) transcribes and
    // persists in one step.
    let Some(lang) = language else {
        return TranscriptCache::new(cache_root)
            .transcript(path, is_video, None, &backend)
            .map_err(|e| e.to_string());
    };

    // With a hint, transcribe the full file directly (so the hint reaches the
    // backend), then persist into the same on-disk cache layout the convenience
    // uses, so later reads hit.
    let opts = TranscribeOptions {
        preferred_language: Some(lang.to_string()),
        ..Default::default()
    };
    let result = opentake_media::transcribe::transcribe_file(path, &backend, &opts)
        .map_err(|e| e.to_string())?;
    persist_full_transcript(cache_root, path, &result);
    Ok(result)
}

/// Write a full transcript into the on-disk cache (`<cache_root>/Transcripts/<key>.json`)
/// using the same file-identity key the cache reads, so a hinted transcription is
/// served from cache on the next call. Best-effort: a write failure is non-fatal.
fn persist_full_transcript(cache_root: &Path, path: &Path, result: &TranscriptionResult) {
    let Some(key) =
        opentake_media::cache_key::file_identity_key(path, opentake_media::cache_key::KEY_HEX_LEN)
    else {
        return;
    };
    let dir = cache_root.join(opentake_media::transcribe::cache::CACHE_SUBDIR);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_vec(result) {
        let _ = std::fs::write(dir.join(format!("{key}.json")), json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_result() -> TranscriptionResult {
        TranscriptionResult {
            text: "hello world".into(),
            language: Some("en".into()),
            words: vec![opentake_media::TranscriptionWord {
                text: "hello".into(),
                start: Some(0.0),
                end: Some(0.5),
            }],
            segments: vec![opentake_media::TranscriptionSegment {
                text: "hello world".into(),
                start: 0.0,
                end: 1.0,
            }],
        }
    }

    #[test]
    fn transcript_dto_is_camel_case_and_round_trips() {
        let dto = TranscriptDto::from_result("m1", sample_result());
        let json = serde_json::to_string(&dto).unwrap();
        // camelCase field on the wire (mediaId, not media_id).
        assert!(json.contains("\"mediaId\":\"m1\""));
        assert!(json.contains("\"segments\":"));
        assert!(json.contains("\"words\":"));
        let back: TranscriptDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn transcript_dto_omits_none_language_and_word_times() {
        let mut r = sample_result();
        r.language = None;
        r.segments = vec![]; // isolate the word object (segments always carry times)
        r.words = vec![opentake_media::TranscriptionWord {
            text: "x".into(),
            start: None,
            end: None,
        }];
        let dto = TranscriptDto::from_result("m1", r);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(!json.contains("language"));
        // The word object is present but its start/end are omitted (untimed token).
        assert_eq!(dto.words[0].start, None);
        assert!(json.contains("\"words\":[{\"text\":\"x\"}]"));
        assert!(!json.contains("\"start\":"));
    }

    #[test]
    fn model_status_dto_camel_case() {
        let dto = ModelStatusDto {
            installed: false,
            model: "base (multilingual)".into(),
            bytes: 147_951_465,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"installed\":false"));
        assert!(json.contains("\"model\":\"base (multilingual)\""));
        let back: ModelStatusDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn download_progress_camel_case() {
        let p = DownloadProgress { fraction: 0.5 };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, "{\"fraction\":0.5}");
    }
}
