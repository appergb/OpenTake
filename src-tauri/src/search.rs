//! Visual + spoken semantic search command surface.
//!
//! Wires the built-but-previously-unreachable SigLIP2 visual-search engine
//! (`opentake_media::search`) to the app, alongside the already-wired spoken
//! (transcript keyword) search. Upstream is `Search/SearchIndexCoordinator.swift`
//! (per-project indexing queue + query) and `MediaTab+Search.swift` (the three
//! result groups: Moments / Spoken / Files). OpenTake substitutes ONNX Runtime
//! for CoreML, so the SigLIP2 model is two explicit `.onnx` files the user
//! downloads once (mirroring the whisper flow in `transcribe.rs`).
//!
//! Commands (all camelCase DTOs, `web/src/lib/types.ts` contract — the repo's #1
//! bug class — with serde round-trip tests):
//! - [`search_model_status`] — is the SigLIP2 model installed? (+ label / size).
//! - [`download_search_model`] — async download with `search://progress` events,
//!   SHA-256 verified exactly as `search::model_download::install` provides.
//! - [`search_index_status`] — how much of the project's visual media is indexed.
//! - [`search_index_start`] — index every not-yet-indexed video/image asset
//!   (sampled frames → SigLIP2 embeddings → `PALMEMB1` store), emitting
//!   `search://index` progress events. Idempotent (already-current assets skip).
//! - [`search_query`] — run the three-group query: Moments (visual), Spoken
//!   (transcript), Files (name match). Matches upstream's groups, caps, and order.
//!
//! The visual index / query path needs the ONNX Runtime backend (feature
//! `ort-backend`, ON for the shipped app). When the model isn't installed, the
//! visual groups degrade to empty and `search_query` still returns Spoken + Files
//! — so plain filename filtering keeps working with zero setup, exactly like the
//! upstream Files group.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use opentake_core::AppCore;
use opentake_domain::{ClipType, MediaResolver};
use opentake_media::search::config as search_config;
use opentake_media::search::config::{RELATIVE_CUTOFF, SEARCH_LIMIT, VISUAL_MATCH_COSINE_FLOOR};
use opentake_media::MediaEngine;

use crate::media::MediaState;

/// One visual ("Moments") hit projected to the front end. `frame` is the shot's
/// start in **source frames** (upstream drags `shotStart…shotEnd`; the panel
/// thumbnails at `shotStart`). `startSec`/`endSec` carry the full source-second
/// range so the UI can drag it onto the timeline as a trimmed clip. camelCase.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MomentHitDto {
    /// Asset id (the clip layer's `media_ref`).
    pub media_id: String,
    /// Shot-start frame in source-media frames (thumb + preview anchor).
    pub frame: i64,
    /// Shot-start in source seconds (drag range lower bound).
    pub start_sec: f64,
    /// Shot-end in source seconds (drag range upper bound). Equals `start_sec`
    /// for stills (zero-length shot).
    pub end_sec: f64,
    /// Uncalibrated similarity score (ordering only — upstream note).
    pub score: f32,
    /// True for still images (no time range → drag as a plain asset).
    pub is_image: bool,
}

/// One spoken ("Spoken") hit: an asset's transcript segment matching every query
/// term. Keyword hits are unranked upstream; `score` is a fixed `1.0` so the DTO
/// shape is uniform (ordering within the group follows transcript order).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpokenHitDto {
    pub media_id: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub text: String,
    pub score: f32,
}

/// One filename ("Files") match. `score` is a fixed `1.0` (name matches are
/// unranked; upstream sorts by the panel's sort mode, default insertion order).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileHitDto {
    pub media_id: String,
    pub score: f32,
}

/// The full three-group query result, mirroring upstream's Moments / Spoken /
/// Files sections (`MediaTab+Search.swift:12-33`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultsDto {
    pub moments: Vec<MomentHitDto>,
    pub spoken: Vec<SpokenHitDto>,
    pub files: Vec<FileHitDto>,
}

/// Whether the SigLIP2 model is installed, plus enough to prompt a download.
/// Mirrors `transcribe.rs`'s `ModelStatusDto`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchModelStatusDto {
    /// True when both encoder files + tokenizer are present on disk.
    pub installed: bool,
    /// Human label for the model (`"siglip2-base-patch16-256"`).
    pub model: String,
    /// Approximate combined download size in bytes (image + text encoder +
    /// tokenizer), for the prompt. `0` until the ONNX assets are hosted.
    pub bytes: i64,
}

/// Visual-index coverage for the project's indexable (video/image) assets.
/// Drives the panel's indexing affordance (upstream `SearchIndexCoordinator`'s
/// `batchTotal`/`batchCompleted`, surfaced here as a snapshot the UI polls or
/// receives via `search://index` events).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexStatusDto {
    /// The model must be installed before anything can be indexed.
    pub model_installed: bool,
    /// Count of video/image assets in the project (upstream `indexableAssets`).
    pub indexable: usize,
    /// How many of those already have a current on-disk embedding index.
    pub indexed: usize,
}

/// Progress payload for the `search://progress` (model download) event.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    fraction: f64,
}

/// Progress payload for the `search://index` event: `completed`/`total` assets
/// plus the current asset's fraction (mirrors the coordinator's progress ring
/// math in `MediaTab+IndexStatus.swift`).
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct IndexProgress {
    completed: usize,
    total: usize,
    fraction: f64,
}

// MARK: - Pure helpers (testable without the ONNX backend)

/// The combined model download size (image + text encoder + tokenizer).
fn model_bytes() -> i64 {
    let m = search_config::manifest();
    m.image_encoder.bytes + m.text_encoder.bytes + m.tokenizer.bytes
}

/// Convert source seconds to a source frame with upstream's **truncating**
/// `secondsToFrame` (`Int(s*fps)`, not rounding). `fps <= 0` falls back to 30.
fn seconds_to_frame(seconds: f64, fps: i32) -> i64 {
    let fps = if fps > 0 { fps as f64 } else { 30.0 };
    (seconds.max(0.0) * fps) as i64
}

/// Name-substring match for the Files group — case-insensitive `contains`, the
/// zero-setup fallback (upstream `passesFilters`' `localizedCaseInsensitiveContains`).
/// Returns matches in manifest (insertion) order to mirror the default
/// `.dateAdded` sort. Never mutates the input.
fn file_matches(entries: &[(String, String)], query: &str) -> Vec<FileHitDto> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    entries
        .iter()
        .filter(|(_, name)| name.to_lowercase().contains(&q))
        .map(|(id, _)| FileHitDto {
            media_id: id.clone(),
            score: 1.0,
        })
        .collect()
}

/// Project a spoken (transcript keyword) hit into its DTO. Score is a fixed
/// `1.0` (keyword matches are unranked; ordering is transcript order).
fn spoken_dto(h: &opentake_media::SpokenHit) -> SpokenHitDto {
    SpokenHitDto {
        media_id: h.asset_id.clone(),
        start_sec: h.start,
        end_sec: h.end,
        text: h.text.clone(),
        score: 1.0,
    }
}

/// Project a visual rank `Hit` into its Moments DTO. `is_image` is true when the
/// shot is zero-length (`shot_start == shot_end`, upstream's still-image row).
fn moment_dto(h: &opentake_media::Hit, fps: i32) -> MomentHitDto {
    let is_image = h.shot_end <= h.shot_start;
    MomentHitDto {
        media_id: h.asset_id.clone(),
        frame: seconds_to_frame(h.shot_start, fps),
        start_sec: h.shot_start,
        end_sec: h.shot_end,
        score: h.score,
        is_image,
    }
}

/// One indexable/searchable asset resolved from the live manifest: id, absolute
/// source path, and kind (drives visual vs. spoken candidacy).
#[derive(Clone)]
struct ResolvedAsset {
    id: String,
    name: String,
    path: PathBuf,
    kind: ClipType,
    has_audio: bool,
}

/// Resolve every manifest asset to `(id, name, path, kind)`, dropping any whose
/// path can't be resolved. Offline (missing) files are kept — indexing/search
/// skip them at read time, matching upstream (a missing file simply yields no
/// index rather than dropping the asset).
fn resolve_assets(core: &AppCore) -> Vec<ResolvedAsset> {
    let snapshot = core.runtime_snapshot();
    resolve_assets_from_snapshot(&snapshot.media, snapshot.project_dir.as_deref())
}

fn resolve_assets_from_snapshot(
    manifest: &opentake_domain::MediaManifest,
    project_dir: Option<&std::path::Path>,
) -> Vec<ResolvedAsset> {
    let resolver = MediaResolver::new(manifest, project_dir);
    manifest
        .entries
        .iter()
        .filter_map(|e| {
            let path = resolver.expected_path(&e.id)?;
            Some(ResolvedAsset {
                id: e.id.clone(),
                name: e.name.clone(),
                path,
                kind: e.kind,
                has_audio: e.has_audio.unwrap_or(false),
            })
        })
        .collect()
}

/// A visual asset is a video or image (upstream `type == .video || .image`).
fn is_visual(kind: ClipType) -> bool {
    matches!(kind, ClipType::Video | ClipType::Image)
}

/// A spoken-searchable asset is a video or audio (upstream candidate filter in
/// `scheduleMomentSearch` / `spokenResults`).
fn is_spoken(kind: ClipType) -> bool {
    matches!(kind, ClipType::Video | ClipType::Audio)
}

// MARK: - Commands

/// `search_model_status`: report whether the SigLIP2 ONNX model is installed.
/// Never downloads. The panel calls this to decide whether to show the
/// "Smart search" download affordance (upstream `MediaTab+IndexStatus.swift`).
#[tauri::command]
pub fn search_model_status(media: State<'_, MediaState>) -> SearchModelStatusDto {
    let models_dir = media.engine().models_dir();
    let manifest = search_config::manifest();
    SearchModelStatusDto {
        installed: opentake_media::search::model_download::installed(models_dir, &manifest)
            .is_some(),
        model: manifest.model.clone(),
        bytes: model_bytes(),
    }
}

/// `download_search_model`: fetch the SigLIP2 ONNX assets (idempotent), emit
/// `search://progress` events as bytes arrive, and SHA-256-verify each file
/// before installing — exactly the machinery `search::model_download::install`
/// provides. Async (network-bound) so it never blocks the UI. Returns the
/// installed status on success.
#[tauri::command]
pub async fn download_search_model(
    app: AppHandle,
    media: State<'_, MediaState>,
) -> Result<SearchModelStatusDto, String> {
    let models_dir = media.engine().models_dir().to_path_buf();
    let manifest = search_config::manifest();
    let base_url = search_config::MODEL_DOWNLOAD_BASE_URL;
    let on_progress = |fraction: f64| {
        let _ = app.emit("search://progress", DownloadProgress { fraction });
    };
    opentake_media::search::model_download::install(&models_dir, &manifest, base_url, on_progress)
        .await
        .map_err(|e| e.to_string())?;
    Ok(SearchModelStatusDto {
        installed: true,
        model: manifest.model.clone(),
        bytes: model_bytes(),
    })
}

/// `search_index_status`: snapshot how much of the project's indexable (video/
/// image) media already has a current on-disk embedding index. Never indexes.
/// The panel uses it to decide whether to offer "index now" and to show the
/// progress ring's denominator.
#[tauri::command]
pub fn search_index_status(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
) -> SearchIndexStatusDto {
    let engine = media.engine();
    let models_dir = engine.models_dir();
    let manifest = search_config::manifest();
    let model_installed =
        opentake_media::search::model_download::installed(models_dir, &manifest).is_some();
    let spec = search_config::embedder_spec();
    let assets = resolve_assets(&core);
    let visual: Vec<&ResolvedAsset> = assets.iter().filter(|a| is_visual(a.kind)).collect();
    let indexed = visual
        .iter()
        .filter(|a| !opentake_media::search::needs_index(engine.cache_root(), &a.path, &spec))
        .count();
    SearchIndexStatusDto {
        model_installed,
        indexable: visual.len(),
        indexed,
    }
}

/// `search_index_start`: index every not-yet-current video/image asset in the
/// project (sampled frames → SigLIP2 embeddings → `PALMEMB1` store), emitting
/// `search://index` progress as each asset completes. Idempotent — already-current
/// assets are skipped by the indexer. Errors if the model isn't installed
/// (guiding the UI to `download_search_model`). Runs the CPU/GPU-bound inference
/// on Tauri's worker thread (the command is sync, so Tauri dispatches it off the
/// UI thread, matching `transcribe_media`). The ONNX backend is always enabled in
/// the shipped app (`opentake-media`'s `ort-backend` feature), so this calls the
/// SigLIP2 embedder directly, mirroring how `transcribe.rs` calls whisper.
#[tauri::command]
pub fn search_index_start(
    app: AppHandle,
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
) -> Result<SearchIndexStatusDto, String> {
    let engine = media.engine();
    let assets = resolve_assets(&core);
    let cache_root = engine.cache_root().to_path_buf();
    let models_dir = engine.models_dir().to_path_buf();
    let pressure = engine.export_pause();
    let worker = production_index_worker(pressure.clone());
    let spec = search_config::embedder_spec();
    let source_identity = assets
        .iter()
        .map(|asset| {
            opentake_media::cache_key::file_identity_key(&asset.path)
                .unwrap_or_else(|| format!("missing:{}", asset.id))
        })
        .collect::<Vec<_>>()
        .join("|");
    let request = opentake_media::ort_worker::JobRequest::new(
        opentake_media::ort_worker::JobKind::Index,
        format!("{}@{}", spec.model, spec.version),
        format!(
            "{}:{}@{}:{source_identity}",
            cache_root.display(),
            spec.model,
            spec.version
        ),
        opentake_media::ort_worker::JobPriority::Background,
    );
    let handle = worker
        .submit(request, move |models, cancel| {
            let job_engine = MediaEngine::new(cache_root, models_dir.clone());
            let model_key = format!("{}@{}:{}", spec.model, spec.version, models_dir.display());
            let embedder = models.get_or_try_init(&model_key, || {
                load_embedder(&job_engine).map_err(opentake_media::ort_worker::WorkerError::Model)
            })?;
            index_assets(
                app,
                &job_engine,
                &assets,
                embedder.as_ref(),
                cancel,
                &pressure,
            )
            .map_err(opentake_media::ort_worker::WorkerError::Job)?;
            Ok(index_status_snapshot(&job_engine, &assets))
        })
        .map_err(|error| error.to_string())?;
    handle.wait().map_err(|error| error.to_string())
}

/// One process-wide production worker. `MediaState` itself is process-wide, so
/// the first command supplies the same shared playback/export pressure counter
/// observed by every later request.
fn production_index_worker(
    pressure: opentake_media::ExportPause,
) -> &'static opentake_media::ort_worker::OrtWorker {
    static WORKER: std::sync::OnceLock<opentake_media::ort_worker::OrtWorker> =
        std::sync::OnceLock::new();
    WORKER.get_or_init(|| opentake_media::ort_worker::OrtWorker::spawn(pressure, 8))
}

/// `search_query`: run the three-group content query — Moments (visual, when the
/// model is installed), Spoken (transcript keyword), Files (name match). Matches
/// upstream's groups, caps, and order (`MediaTab+Search.swift`). Visual is
/// best-effort: with no installed model (or an all-unindexed project) `moments`
/// is empty and Spoken + Files still return — so plain filename filtering is the
/// zero-setup fallback. Never errors on a missing model (an empty query returns
/// empty groups).
#[tauri::command]
pub fn search_query(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    query: String,
) -> SearchResultsDto {
    let trimmed = query.trim().to_string();
    if trimmed.is_empty() {
        return SearchResultsDto::default();
    }
    let engine = media.engine();
    let snapshot = core.runtime_snapshot();
    let assets = resolve_assets_from_snapshot(&snapshot.media, snapshot.project_dir.as_deref());
    let fps = snapshot.timeline.fps;

    // Files: name-substring over every asset (the zero-setup fallback).
    let name_entries: Vec<(String, String)> = assets
        .iter()
        .map(|a| (a.id.clone(), a.name.clone()))
        .collect();
    let files = file_matches(&name_entries, &trimmed);

    // Spoken: keyword over cached transcripts of video/audio assets.
    let spoken_candidates: Vec<(String, PathBuf)> = assets
        .iter()
        .filter(|a| is_spoken(a.kind))
        .map(|a| (a.id.clone(), a.path.clone()))
        .collect();
    let spoken: Vec<SpokenHitDto> = engine
        .search_spoken(&trimmed, &spoken_candidates, SEARCH_LIMIT)
        .iter()
        .map(spoken_dto)
        .collect();

    // Moments: visual rank over on-disk embedding indexes (needs the model).
    let moments = search_visual(engine, &assets, &trimmed, fps);

    SearchResultsDto {
        moments,
        spoken,
        files,
    }
}

/// Visual query for the panel: rank the project's visual assets, capped at the
/// panel default [`SEARCH_LIMIT`]. Delegates to [`visual_hits_by_id`].
fn search_visual(
    engine: &MediaEngine,
    assets: &[ResolvedAsset],
    query: &str,
    fps: i32,
) -> Vec<MomentHitDto> {
    let id_paths: Vec<(String, PathBuf)> = assets
        .iter()
        .filter(|a| is_visual(a.kind))
        .map(|a| (a.id.clone(), a.path.clone()))
        .collect();
    visual_hits_by_id(engine, &id_paths, query, fps, SEARCH_LIMIT)
}

/// Rank `query` against the on-disk embedding indexes of the given visual assets
/// (`(id, path)` pairs), returning up to `limit` Moments hits. Loads the
/// installed model, encodes the text query, loads each asset's `.embed` index
/// (skipping missing/stale), and ranks best-per-shot with the `min_score` floor
/// then `limit` + relative cutoff — the exact upstream order. Empty when the
/// model isn't installed, the query can't encode, or nothing is indexed. Shared
/// by the panel query and the `search_media` MCP bridge so both rank identically.
pub(crate) fn visual_hits_by_id(
    engine: &MediaEngine,
    id_paths: &[(String, PathBuf)],
    query: &str,
    fps: i32,
    limit: usize,
) -> Vec<MomentHitDto> {
    use opentake_media::search::embed_store;

    let Ok(embedder) = load_embedder(engine) else {
        return Vec::new();
    };
    // Encode the text query once; a failure yields no visual hits.
    let Ok(vector) = opentake_media::search::Embedder::encode_text(&embedder, query) else {
        return Vec::new();
    };

    // Load each asset's current index (skip missing/stale silently).
    let mut indexes: Vec<(String, embed_store::AssetIndex)> = Vec::new();
    for (id, path) in id_paths {
        let Some(key) = embed_store::key(path) else {
            continue;
        };
        if let Ok(index) = embed_store::load(engine.cache_root(), &key) {
            indexes.push((id.clone(), index));
        }
    }
    if indexes.is_empty() {
        return Vec::new();
    }

    opentake_media::search_visual_ranked(
        &vector,
        &indexes,
        limit,
        RELATIVE_CUTOFF,
        Some(VISUAL_MATCH_COSINE_FLOOR),
    )
    .iter()
    .map(|h| moment_dto(h, fps))
    .collect()
}

/// Compute the visual-index coverage for a set of visual asset `(id, path)`
/// pairs: `(indexable, indexed)`. Shared by the `search_media` bridge for its
/// `indexableAssets`/`indexedAssets` fields. `indexed` counts assets whose
/// on-disk embedding index is current for the configured model.
pub(crate) fn visual_coverage(
    engine: &MediaEngine,
    id_paths: &[(String, PathBuf)],
) -> (usize, usize) {
    let spec = search_config::embedder_spec();
    let indexed = id_paths
        .iter()
        .filter(|(_, path)| !opentake_media::search::needs_index(engine.cache_root(), path, &spec))
        .count();
    (id_paths.len(), indexed)
}

/// True when the SigLIP2 model is installed. Shared with the `search_media`
/// bridge to pick its `status` string.
pub(crate) fn model_installed(engine: &MediaEngine) -> bool {
    let manifest = search_config::manifest();
    opentake_media::search::model_download::installed(engine.models_dir(), &manifest).is_some()
}

// MARK: - ort-backed indexing internals

/// Load the installed SigLIP2 embedder, or a structured "model not installed"
/// error the UI turns into a download prompt. Mirrors `transcribe.rs`'s
/// `load_backend`.
fn load_embedder(engine: &MediaEngine) -> Result<opentake_media::search::OrtEmbedder, String> {
    let models_dir = engine.models_dir();
    let manifest = search_config::manifest();
    let installed = opentake_media::search::model_download::installed(models_dir, &manifest)
        .ok_or_else(|| {
            format!(
                "visual search model not installed — download '{}' first",
                manifest.model
            )
        })?;
    let tokenizer_json = installed.tokenizer_folder.join("tokenizer.json");
    opentake_media::search::OrtEmbedder::new(
        &installed.image_encoder,
        &installed.text_encoder,
        &tokenizer_json,
        installed.spec,
    )
    .map_err(|e| e.to_string())
}

/// Index every not-yet-current video/image asset, emitting a `search://index`
/// event as each completes. The single-worker sequential loop mirrors the
/// coordinator's `ensureWorker` queue (`SearchIndexCoordinator.swift:139-160`) —
/// one asset at a time, in manifest order — kept simple here (Tauri already runs
/// the command off the UI thread; a background queue is a later refinement).
fn index_assets(
    app: AppHandle,
    engine: &MediaEngine,
    assets: &[ResolvedAsset],
    embedder: &opentake_media::search::OrtEmbedder,
    cancel: &opentake_media::search::CancelToken,
    pressure: &opentake_media::ExportPause,
) -> Result<(), String> {
    use opentake_media::search::Embedder;
    use opentake_media::search::{index_image, index_video, needs_index, SamplerOptions};

    let spec = Embedder::spec(embedder).clone();
    let cache_root = engine.cache_root();
    let opts = SamplerOptions::default();

    // Only assets that actually need visual or transcript work (idempotent),
    // preserving manifest order. File identity plus model/sampler versions are
    // encoded in the two stores, so changed media and model upgrades invalidate
    // themselves and an interrupted run resumes only missing work.
    let pending: Vec<&ResolvedAsset> = assets
        .iter()
        .filter(|a| {
            let visual = is_visual(a.kind) && needs_index(cache_root, &a.path, &spec);
            let transcript = (a.kind == ClipType::Audio
                || (a.kind == ClipType::Video && a.has_audio))
                && !opentake_media::transcribe::cache::has_cached_on_disk(cache_root, &a.path);
            visual || transcript
        })
        .collect();
    let total = pending.len();
    if total == 0 {
        let _ = app.emit(
            "search://index",
            IndexProgress {
                completed: 0,
                total: 0,
                fraction: 1.0,
            },
        );
        return Ok(());
    }

    for (i, a) in pending.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err("indexing cancelled".into());
        }
        if !pressure.wait_while_active(|| cancel.is_cancelled()) {
            return Err("indexing cancelled while yielding to playback/export".into());
        }

        // Per-asset progress: forward the sampler's fraction into the batch.
        let base = i;
        let on_progress = |frac: f64| {
            let _ = app.emit(
                "search://index",
                IndexProgress {
                    completed: base,
                    total,
                    fraction: (base as f64 + frac.clamp(0.0, 1.0)) / total as f64,
                },
            );
        };

        // A per-asset failure (offline file, decode error) is skipped — one bad
        // clip must not abort the batch (upstream `failedIds.insert` + continue).
        let visual_result = match a.kind {
            ClipType::Image => match engine.image_thumbnail(&a.path) {
                // Reuse the decoded thumbnail as the still's frame; a full-res
                // decode is unnecessary for a single squash-resized embedding.
                Ok(frame) => index_image(cache_root, &a.path, &frame, embedder, cancel),
                Err(e) => Err(e),
            },
            ClipType::Video => {
                // Probe the source for its true duration/dimensions so the sampler
                // walks the whole clip (the manifest duration may be stale).
                let (duration, width, height) = match engine.probe(&a.path) {
                    Ok(p) => (p.duration_secs, p.width.unwrap_or(0), p.height.unwrap_or(0)),
                    Err(e) => {
                        eprintln!("[search] probe failed {}: {e}", a.path.display());
                        emit_completed(&app, i + 1, total);
                        continue;
                    }
                };
                index_video(
                    cache_root,
                    &a.path,
                    duration,
                    width,
                    height,
                    embedder,
                    &opts,
                    cancel,
                    Some(&on_progress),
                )
            }
            _ => Ok(()),
        };
        if let Err(e) = visual_result {
            if cancel.is_cancelled() {
                return Err("indexing cancelled".into());
            }
            eprintln!("[search] index failed {}: {e}", a.path.display());
        }

        // Automatic spoken indexing shares the same one-worker boundary. A
        // missing whisper model or bad audio marks only this asset failed; the
        // visual store and remaining assets still converge on restart.
        let needs_transcript = (a.kind == ClipType::Audio
            || (a.kind == ClipType::Video && a.has_audio))
            && !opentake_media::transcribe::cache::has_cached_on_disk(cache_root, &a.path);
        if needs_transcript {
            if cancel.is_cancelled() {
                return Err("transcription cancelled".into());
            }
            if let Err(error) = crate::transcribe::transcribe_with_cache(
                engine,
                &a.path,
                a.kind == ClipType::Video,
                None,
            ) {
                eprintln!(
                    "[search] transcription failed {}: {error}",
                    a.path.display()
                );
            }
        }
        emit_completed(&app, i + 1, total);
    }
    Ok(())
}

/// Emit a batch-completed progress tick (`completed`/`total`, fraction settled).
fn emit_completed(app: &AppHandle, completed: usize, total: usize) {
    let fraction = if total > 0 {
        completed as f64 / total as f64
    } else {
        1.0
    };
    let _ = app.emit(
        "search://index",
        IndexProgress {
            completed,
            total,
            fraction,
        },
    );
}

/// A post-index status snapshot (model installed + indexable/indexed counts).
fn index_status_snapshot(engine: &MediaEngine, assets: &[ResolvedAsset]) -> SearchIndexStatusDto {
    let manifest = search_config::manifest();
    let model_installed =
        opentake_media::search::model_download::installed(engine.models_dir(), &manifest).is_some();
    let spec = search_config::embedder_spec();
    let visual: Vec<&ResolvedAsset> = assets.iter().filter(|a| is_visual(a.kind)).collect();
    let indexed = visual
        .iter()
        .filter(|a| !opentake_media::search::needs_index(engine.cache_root(), &a.path, &spec))
        .count();
    SearchIndexStatusDto {
        model_installed,
        indexable: visual.len(),
        indexed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_single_worker_cancels_skips_stale_and_yields_to_playback_export() {
        use opentake_media::ort_worker::{
            JobKind, JobPriority, JobRequest, JobState, OrtWorker, WorkerError,
        };
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Mutex};
        use std::time::Duration;

        let pause = opentake_media::ExportPause::new();
        let worker = OrtWorker::spawn(pause.clone(), 4);
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let order = Arc::new(Mutex::new(Vec::new()));

        let submit = |key: &str, priority: JobPriority, marker: usize| {
            let active = active.clone();
            let max_active = max_active.clone();
            let order = order.clone();
            worker.submit(
                JobRequest::new(JobKind::Index, "siglip2@1", key, priority),
                move |_models, cancel| {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(now, Ordering::SeqCst);
                    for _ in 0..20 {
                        if cancel.is_cancelled() {
                            active.fetch_sub(1, Ordering::SeqCst);
                            return Err(WorkerError::Cancelled);
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    order.lock().unwrap().push(marker);
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(marker)
                },
            )
        };

        // Playback/export pressure prevents any queued inference from starting.
        let pressure = pause.guard();
        let low = submit("asset-low", JobPriority::Background, 1).unwrap();
        let high_a = submit("asset-high-a", JobPriority::Interactive, 2).unwrap();
        let high_b = submit("asset-high-b", JobPriority::Interactive, 3).unwrap();
        let duplicate = submit("asset-high-a", JobPriority::Interactive, 99).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(low.state(), JobState::Queued);
        assert!(order.lock().unwrap().is_empty());
        drop(pressure);

        assert_eq!(high_a.wait().unwrap(), 2);
        assert_eq!(duplicate.wait().unwrap(), 2); // one persisted result, no duplicate work
        assert_eq!(high_b.wait().unwrap(), 3);
        assert_eq!(low.wait().unwrap(), 1);
        assert_eq!(*order.lock().unwrap(), vec![2, 3, 1]); // FIFO within priority
        assert_eq!(max_active.load(Ordering::SeqCst), 1);

        // A queued cancellation never runs; a running cancellation cooperates
        // at a batch boundary and the next model error/panic cannot kill worker.
        let blocker = submit("blocker", JobPriority::Interactive, 4).unwrap();
        let queued = submit("queued-cancel", JobPriority::Background, 5).unwrap();
        queued.cancel();
        assert_eq!(blocker.wait().unwrap(), 4);
        assert_eq!(queued.wait(), Err(WorkerError::Cancelled));
        assert_eq!(queued.state(), JobState::Cancelled);

        let running = submit("running-cancel", JobPriority::Interactive, 6).unwrap();
        running.wait_until_running(Duration::from_secs(1)).unwrap();
        running.cancel();
        assert_eq!(running.wait(), Err(WorkerError::Cancelled));

        let model_failure = worker
            .submit::<usize, _>(
                JobRequest::new(
                    JobKind::Transcribe,
                    "whisper@1",
                    "model-failure",
                    JobPriority::Background,
                ),
                |_models, _cancel| Err(WorkerError::Model("fixture failure".into())),
            )
            .unwrap();
        assert!(matches!(model_failure.wait(), Err(WorkerError::Model(_))));
        let retried = submit("model-failure", JobPriority::Background, 12).unwrap();
        assert_eq!(retried.wait().unwrap(), 12);
        let panicked = worker
            .submit::<usize, _>(
                JobRequest::new(
                    JobKind::Index,
                    "siglip2@1",
                    "panic",
                    JobPriority::Background,
                ),
                |_models, _cancel| panic!("fixture panic"),
            )
            .unwrap();
        assert_eq!(panicked.wait(), Err(WorkerError::Panicked));
        let recovered = submit("recovered", JobPriority::Background, 7).unwrap();
        assert_eq!(recovered.wait().unwrap(), 7);

        let model_loads = Arc::new(AtomicUsize::new(0));
        for key in ["registry-a", "registry-b"] {
            let model_loads = model_loads.clone();
            let handle = worker
                .submit(
                    JobRequest::new(
                        JobKind::Search,
                        "shared-model@1",
                        key,
                        JobPriority::Interactive,
                    ),
                    move |models, _cancel| {
                        let model = models.get_or_try_init("shared-model@1", || {
                            model_loads.fetch_add(1, Ordering::SeqCst);
                            Ok::<_, WorkerError>(42usize)
                        })?;
                        Ok::<_, WorkerError>(*model)
                    },
                )
                .unwrap();
            assert_eq!(handle.wait().unwrap(), 42);
        }
        assert_eq!(model_loads.load(Ordering::SeqCst), 1);

        // Changing either source fingerprint or model version forms a new key;
        // a restarted worker can resume only the missing key and converge on the
        // same final result.
        let v1 = submit("media-fingerprint-a@siglip2-1", JobPriority::Background, 8).unwrap();
        assert_eq!(v1.wait().unwrap(), 8);
        let changed = submit("media-fingerprint-b@siglip2-1", JobPriority::Background, 9).unwrap();
        let upgraded =
            submit("media-fingerprint-b@siglip2-2", JobPriority::Background, 10).unwrap();
        assert_eq!(changed.wait().unwrap(), 9);
        assert_eq!(upgraded.wait().unwrap(), 10);

        worker.shutdown().unwrap();
        assert_eq!(worker.active_jobs(), 0);
        assert!(matches!(
            submit("after-shutdown", JobPriority::Background, 11),
            Err(WorkerError::Shutdown)
        ));

        // Admission is truly bounded even while pressure holds the worker.
        let bounded_pause = opentake_media::ExportPause::new();
        let bounded_guard = bounded_pause.guard();
        let bounded = OrtWorker::spawn(bounded_pause, 2);
        let first = bounded
            .submit(
                JobRequest::new(JobKind::Index, "m@1", "first", JobPriority::Background),
                |_models, _cancel| Ok::<_, WorkerError>(1usize),
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(25));
        let second = bounded
            .submit(
                JobRequest::new(JobKind::Index, "m@1", "second", JobPriority::Background),
                |_models, _cancel| Ok::<_, WorkerError>(2usize),
            )
            .unwrap();
        assert_eq!(bounded.queued_jobs(), 2);
        assert!(matches!(
            bounded.submit(
                JobRequest::new(JobKind::Index, "m@1", "third", JobPriority::Background),
                |_models, _cancel| Ok::<_, WorkerError>(3usize),
            ),
            Err(WorkerError::QueueFull)
        ));
        drop(bounded_guard);
        assert_eq!(first.wait().unwrap(), 1);
        assert_eq!(second.wait().unwrap(), 2);
        bounded.shutdown().unwrap();

        // Four interactive jobs is the explicit starvation bound.
        let fairness_pause = opentake_media::ExportPause::new();
        let fairness_guard = fairness_pause.guard();
        let fairness = OrtWorker::spawn(fairness_pause, 8);
        let fairness_order = Arc::new(Mutex::new(Vec::new()));
        let enqueue = |key: &'static str, priority, marker| {
            let fairness_order = fairness_order.clone();
            fairness
                .submit(
                    JobRequest::new(JobKind::Index, "m@1", key, priority),
                    move |_models, _cancel| {
                        fairness_order.lock().unwrap().push(marker);
                        Ok::<_, WorkerError>(marker)
                    },
                )
                .unwrap()
        };
        let low = enqueue("fair-low", JobPriority::Background, 0usize);
        let highs = [1usize, 2, 3, 4, 5]
            .into_iter()
            .map(|marker| {
                let key = match marker {
                    1 => "fair-high-1",
                    2 => "fair-high-2",
                    3 => "fair-high-3",
                    4 => "fair-high-4",
                    _ => "fair-high-5",
                };
                enqueue(key, JobPriority::Interactive, marker)
            })
            .collect::<Vec<_>>();
        drop(fairness_guard);
        for high in &highs {
            let _ = high.wait().unwrap();
        }
        assert_eq!(low.wait().unwrap(), 0);
        assert_eq!(*fairness_order.lock().unwrap(), vec![1, 2, 3, 4, 0, 5]);
        fairness.shutdown().unwrap();

        let restarted = OrtWorker::spawn(pause, 1);
        let resumed = restarted
            .submit(
                JobRequest::new(
                    JobKind::Index,
                    "siglip2@2",
                    "media-fingerprint-b@siglip2-2",
                    JobPriority::Background,
                ),
                |_models, _cancel| Ok::<_, WorkerError>(10usize),
            )
            .unwrap();
        assert_eq!(resumed.wait().unwrap(), 10);
        restarted.shutdown().unwrap();
        assert_eq!(restarted.active_jobs(), 0);
    }

    // --- pure DTO / merge / cap logic (no ort, no ffmpeg) ---

    #[test]
    fn seconds_to_frame_truncates_like_upstream() {
        // Int(s*fps) truncation, not rounding: 1.99s @ 30fps → 59, not 60.
        assert_eq!(seconds_to_frame(1.99, 30), 59);
        assert_eq!(seconds_to_frame(2.0, 30), 60);
        assert_eq!(seconds_to_frame(0.0, 30), 0);
        // Non-positive fps falls back to 30.
        assert_eq!(seconds_to_frame(1.0, 0), 30);
        assert_eq!(seconds_to_frame(-5.0, 30), 0); // negative clamps to 0
    }

    #[test]
    fn file_matches_is_case_insensitive_substring_in_order() {
        let entries = vec![
            ("a".into(), "Sunset Beach.mp4".into()),
            ("b".into(), "harbor.mov".into()),
            ("c".into(), "SUNSET timelapse.mp4".into()),
        ];
        let hits = file_matches(&entries, "sunset");
        // Both "Sunset" assets match, in manifest order; "harbor" doesn't.
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].media_id, "a");
        assert_eq!(hits[1].media_id, "c");
        assert!(hits.iter().all(|h| h.score == 1.0));
    }

    #[test]
    fn file_matches_empty_query_is_empty() {
        let entries = vec![("a".into(), "x.mp4".into())];
        assert!(file_matches(&entries, "").is_empty());
        assert!(file_matches(&entries, "   ").is_empty());
    }

    #[test]
    fn spoken_dto_projects_fields_with_fixed_score() {
        let h = opentake_media::SpokenHit {
            asset_id: "a1".into(),
            start: 1.5,
            end: 2.5,
            text: "the budget plan".into(),
        };
        let dto = spoken_dto(&h);
        assert_eq!(dto.media_id, "a1");
        assert_eq!(dto.start_sec, 1.5);
        assert_eq!(dto.end_sec, 2.5);
        assert_eq!(dto.text, "the budget plan");
        assert_eq!(dto.score, 1.0);
    }

    #[test]
    fn moment_dto_marks_zero_length_shot_as_image() {
        // A still: shot_start == shot_end → is_image true, no meaningful range.
        let still = opentake_media::Hit {
            asset_id: "img".into(),
            time: 0.0,
            shot_start: 0.0,
            shot_end: 0.0,
            score: 0.9,
        };
        let d = moment_dto(&still, 30);
        assert!(d.is_image);
        assert_eq!(d.frame, 0);

        // A video shot: range present, frame = trunc(shot_start*fps).
        let vid = opentake_media::Hit {
            asset_id: "vid".into(),
            time: 3.2,
            shot_start: 3.0,
            shot_end: 6.0,
            score: 0.8,
        };
        let d = moment_dto(&vid, 30);
        assert!(!d.is_image);
        assert_eq!(d.frame, 90);
        assert_eq!(d.start_sec, 3.0);
        assert_eq!(d.end_sec, 6.0);
    }

    #[test]
    fn is_visual_and_is_spoken_partition_kinds_like_upstream() {
        assert!(is_visual(ClipType::Video));
        assert!(is_visual(ClipType::Image));
        assert!(!is_visual(ClipType::Audio));
        assert!(is_spoken(ClipType::Video));
        assert!(is_spoken(ClipType::Audio));
        assert!(!is_spoken(ClipType::Image)); // images have nothing spoken
    }

    // --- DTO serde round-trips (camelCase wire contract) ---

    #[test]
    fn moment_hit_dto_is_camel_case_and_round_trips() {
        let dto = MomentHitDto {
            media_id: "m1".into(),
            frame: 90,
            start_sec: 3.0,
            end_sec: 6.0,
            score: 0.8,
            is_image: false,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"mediaId\":\"m1\""));
        assert!(json.contains("\"startSec\":3.0"));
        assert!(json.contains("\"endSec\":6.0"));
        assert!(json.contains("\"isImage\":false"));
        let back: MomentHitDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn spoken_hit_dto_camel_case_round_trips() {
        let dto = SpokenHitDto {
            media_id: "m1".into(),
            start_sec: 1.0,
            end_sec: 2.0,
            text: "hello".into(),
            score: 1.0,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"mediaId\":\"m1\""));
        assert!(json.contains("\"startSec\":1.0"));
        let back: SpokenHitDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn search_results_dto_round_trips_all_groups() {
        let dto = SearchResultsDto {
            moments: vec![MomentHitDto {
                media_id: "v".into(),
                frame: 0,
                start_sec: 0.0,
                end_sec: 1.0,
                score: 0.7,
                is_image: false,
            }],
            spoken: vec![SpokenHitDto {
                media_id: "a".into(),
                start_sec: 0.0,
                end_sec: 1.0,
                text: "x".into(),
                score: 1.0,
            }],
            files: vec![FileHitDto {
                media_id: "f".into(),
                score: 1.0,
            }],
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"moments\":"));
        assert!(json.contains("\"spoken\":"));
        assert!(json.contains("\"files\":"));
        let back: SearchResultsDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn search_model_status_dto_camel_case() {
        let dto = SearchModelStatusDto {
            installed: false,
            model: "siglip2-base-patch16-256".into(),
            bytes: 0,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"installed\":false"));
        assert!(json.contains("\"model\":\"siglip2-base-patch16-256\""));
        let back: SearchModelStatusDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn search_index_status_dto_camel_case_round_trips() {
        let dto = SearchIndexStatusDto {
            model_installed: true,
            indexable: 5,
            indexed: 2,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"modelInstalled\":true"));
        assert!(json.contains("\"indexable\":5"));
        assert!(json.contains("\"indexed\":2"));
        let back: SearchIndexStatusDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn download_and_index_progress_are_camel_case() {
        let d = DownloadProgress { fraction: 0.5 };
        assert_eq!(serde_json::to_string(&d).unwrap(), "{\"fraction\":0.5}");
        let ip = IndexProgress {
            completed: 1,
            total: 4,
            fraction: 0.25,
        };
        let json = serde_json::to_string(&ip).unwrap();
        assert!(json.contains("\"completed\":1"));
        assert!(json.contains("\"total\":4"));
        assert!(json.contains("\"fraction\":0.25"));
    }
}
