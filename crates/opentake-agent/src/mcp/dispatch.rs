//! The uniform tool-dispatch shell (`agent-SPEC.md` §8.2; port of upstream
//! `ToolExecutor.execute`).
//!
//! ONE pipeline wraps EVERY tool:
//! 1. resolve the name to a [`ToolName`] (unknown → error result),
//! 2. snapshot `before = timeline` + `manifest = media`,
//! 3. expand inbound short-id prefixes in the args,
//! 4. decode the typed args (precise-path errors → error result),
//! 5. run the tool body (editing tools build an [`EditCommand`] and apply it;
//!    read tools serialize state),
//! 6. attach a `context_signal` block via [`signal::engine::attach`],
//! 7. shorten outbound ids in the result,
//! 8. return the [`ToolResult`].
//!
//! Sync throughout: every wired (EXISTS-mapped) tool is synchronous. The async
//! generation / media tools are stubs in this phase and return an honest
//! "not yet implemented" so the tool table is complete.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

use opentake_domain::{AnimPair, Crop, Interpolation, Keyframe, KeyframeTrack};
use opentake_domain::{
    ChromaKey, ColorGrade, Effect, LiftGammaGain, Mask, MaskShape, MediaManifest, Point2, Rgb,
    Rgba, TextStyle, Timeline, Transform, VideoType,
};
use opentake_media::analysis::{
    detect_beats, detect_silences, BeatDetectionConfig, SilenceDetectionConfig,
};
use opentake_media::{PcmFormat, PcmSpec};
use opentake_ops::{
    ClipEntry, ClipMove, ClipProperties, EditCommand, FrameRange, KeyframePayload,
    KeyframeProperty, RenameEntry, TextEntry,
};
use serde_json::Value;

use crate::mcp::core_handle::CoreHandle;
use crate::mcp::gen_catalog;
use crate::mcp::media_bridge::{
    frame_to_block, ImportSource, InspectResult, MediaBridge, SearchCandidate, TranscriptSource,
};
use crate::plugin::registry::PluginRegistry;
use crate::signal::engine;
use crate::signal::rules::OpContext;
use crate::tools::args::{self, *};
use crate::tools::encode_timeline::encode_timeline;
use crate::tools::errors::{decode_tool_args, ToolError};
use crate::tools::names::ToolName;
use crate::tools::result::{Block, ToolResult};
use crate::tools::short_id;

/// `inspect_timeline` frame-sampling + downscale constants, 1:1 with upstream
/// `ToolExecutor+InspectTimeline`: default 6 sampled frames, hard cap 12, longest
/// render edge 512px (the JPEG quality lives with the encoder in the bridge).
const INSPECT_TIMELINE_DEFAULT_FRAMES: i32 = 6;
const INSPECT_TIMELINE_MAX_FRAMES: i32 = 12;
const INSPECT_TIMELINE_MAX_DIMENSION: u32 = 512;

/// The in-process tool dispatcher. Holds the [`CoreHandle`] boundary, the plugin
/// registry (read-locked for the active plugin), and a per-dispatcher agent-undo
/// stack so `undo` only reverts edits this session made.
pub struct Dispatcher {
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    /// The render + import side-door (`inspect_timeline` / `import_media`), or
    /// `None` in a non-Tauri build / tests. See [`MediaBridge`]. Kept separate from
    /// [`CoreHandle`] because those two capabilities reach into crates the agent
    /// layer does not link (`opentake-render`, the src-tauri import path).
    bridge: Option<Arc<dyn MediaBridge>>,
    /// Action names of agent edits applied through this dispatcher, newest last.
    /// Guards `undo`: we only revert when this session has pushed an edit.
    agent_undo: Mutex<Vec<String>>,
}

impl Dispatcher {
    /// New dispatcher over a core handle + plugin registry, with no media bridge
    /// (the two render/import tools then report "not available"). Used by tests and
    /// any non-Tauri host.
    pub fn new(handle: Arc<dyn CoreHandle>, registry: Arc<RwLock<PluginRegistry>>) -> Self {
        Self::with_bridge(handle, registry, None)
    }

    /// New dispatcher with an optional [`MediaBridge`] wired in. The Tauri shell
    /// (`src-tauri/src/mcp.rs`) passes `Some(bridge)` so `inspect_timeline` /
    /// `import_media` reach the real GPU + import paths.
    pub fn with_bridge(
        handle: Arc<dyn CoreHandle>,
        registry: Arc<RwLock<PluginRegistry>>,
        bridge: Option<Arc<dyn MediaBridge>>,
    ) -> Self {
        Dispatcher {
            handle,
            registry,
            bridge,
            agent_undo: Mutex::new(Vec::new()),
        }
    }

    /// Run one tool through the full pipeline and return its neutral result.
    pub fn dispatch(&self, name: &str, args: Value) -> ToolResult {
        // 1. Resolve the tool name.
        let Ok(tool) = name.parse::<ToolName>() else {
            return ToolResult::error(format!("Unknown tool: {name}"));
        };

        // 2. Snapshot the pre-run state.
        let before = self.handle.timeline();
        let manifest = self.handle.media();

        // 3. Expand inbound short-id prefixes against the pre-run id universe.
        let universe = short_id::current_id_universe(&before, &manifest);
        let args = match short_id::expand_id_prefixes(&args, &universe) {
            Ok(v) => v,
            Err(e) => return ToolResult::error(e.message),
        };

        // 4 + 5. Decode typed args and run the body. `op` collects what the body
        // did for the rule layer; `result` is the body's neutral output.
        let mut op = OpContext::default();
        let result = match self.run_body(tool, &args, &before, &manifest, &mut op) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(e.message),
        };

        // 6. Attach the context signal against the post-run timeline.
        let after = self.handle.timeline();
        let plugin_guard = self.registry.read().ok();
        let plugin = plugin_guard.as_ref().and_then(|g| g.active());
        let manual_video_type: Option<VideoType> = None;
        let result = engine::attach(tool, result, &after, plugin, manual_video_type, &op);
        drop(plugin_guard);

        // 7. Shorten outbound ids against the post-run id universe (so newly
        //    created ids in summaries shorten too).
        let post_manifest = self.handle.media();
        let post_universe = short_id::current_id_universe(&after, &post_manifest);
        short_id::shorten_ids(result, &post_universe)
    }

    /// Decode args + execute one tool, returning its neutral result or a tool
    /// error. The `op` is filled in for the rule layer. Editing tools build an
    /// [`EditCommand`] and apply it through the handle; read tools serialize state.
    fn run_body(
        &self,
        tool: ToolName,
        args: &Value,
        before: &Timeline,
        manifest: &MediaManifest,
        op: &mut OpContext,
    ) -> Result<ToolResult, ToolError> {
        match tool {
            // --- Reads ---
            ToolName::GetTimeline => {
                let a: GetTimelineArgs = decode_tool_args(args, "")?;
                let tl = self.handle.timeline();
                // canGenerate is gated by the (not-yet-wired) generation backend;
                // false until that lands so the model never proposes generation.
                let json = encode_timeline(&tl, a.start_frame, a.end_frame, false);
                Ok(ToolResult::ok(json.to_string()))
            }
            ToolName::GetMedia => {
                let manifest = self.handle.media();
                let json = serde_json::to_value(&manifest)
                    .map(round_floats_3dp)
                    .map_err(|e| ToolError::new(format!("get_media: {e}")))?;
                Ok(ToolResult::ok(json.to_string()))
            }
            ToolName::ListFolders => {
                let manifest = self.handle.media();
                let json = serde_json::to_value(&manifest.folders)
                    .map_err(|e| ToolError::new(format!("list_folders: {e}")))?;
                Ok(ToolResult::ok(json.to_string()))
            }
            ToolName::ListModels => self.list_models_catalog(args),

            // --- Editing (wired to EditCommand) ---
            ToolName::AddClips => self.add_clips(args, manifest, op),
            ToolName::InsertClips => self.insert_clips(args, manifest),
            ToolName::MoveClips => self.move_clips(args, before),
            ToolName::RemoveClips => self.remove_clips(args, before, op),
            ToolName::RemoveTracks => self.remove_tracks(args),
            ToolName::SplitClip => self.split_clip(args, before, op),
            ToolName::SetKeyframes => self.set_keyframes(args),
            ToolName::RippleDeleteRanges => self.ripple_delete_ranges(args, before, op),
            ToolName::AddTexts => self.add_texts(args),
            ToolName::CreateFolder => self.create_folder(args),
            ToolName::MoveToFolder => self.move_to_folder(args),
            ToolName::SetClipProperties => self.set_clip_properties(args, before, manifest),
            ToolName::SetColorGrade => self.set_color_grade(args),
            ToolName::ChromaKey => self.chroma_key(args),
            ToolName::SetMask => self.set_mask(args),
            ToolName::ApplyEffect => self.apply_effect(args),
            ToolName::RenameMedia => self.rename_media(args),
            ToolName::RenameFolder => self.rename_folder(args),
            ToolName::DeleteMedia => self.delete_media(args),
            ToolName::DeleteFolder => self.delete_folder(args),
            ToolName::Undo => self.undo(),

            // --- Workflow plugins / Skills (OpenTake addition; backed by the
            //     PluginRegistry the dispatcher holds) ---
            ToolName::ListWorkflows => self.list_workflows(),
            ToolName::ActivateWorkflow => self.activate_workflow(args),
            ToolName::DeactivateWorkflow => self.deactivate_workflow(),

            // --- Analysis-driven edit surface ---
            ToolName::DetectBeats => self.detect_beats(args, before),
            ToolName::AutoCutToBeats => self.auto_cut_to_beats(args, before),
            ToolName::SmartReframe => self.smart_reframe(args),
            ToolName::TightenSilences => self.tighten_silences(args, before),

            // --- Render + import + transcript + search (wired to the injected MediaBridge) ---
            ToolName::InspectTimeline => self.inspect_timeline(args, before),
            ToolName::ImportMedia => self.import_media(args, manifest),
            ToolName::GetTranscript => self.get_transcript(args, before, manifest),
            ToolName::AddCaptions => self.add_captions(args, before, manifest),
            ToolName::SearchMedia => self.search_media(args, manifest),

            // --- Not yet implementable in this phase (honest stubs) ---
            // inspect_media still needs the analysis backend; generation/upscale
            // need the async GenClient + BYOK auth. Motion graphics (#34) now
            // routes through the planned Motion Canvas plugin: render mp4 ->
            // import media -> place clip.
            ToolName::InspectMedia
            | ToolName::GenerateVideo
            | ToolName::GenerateImage
            | ToolName::GenerateAudio
            | ToolName::UpscaleMedia
            | ToolName::AddMotionGraphic
            | ToolName::EditMotionGraphic => Ok(ToolResult::error(format!(
                "{}: not yet implemented",
                tool.as_str()
            ))),
        }
    }

    // MARK: - Generative read bodies

    /// `list_models`: project the built-in static catalog from `opentake-gen`
    /// into the `{ models, loaded }` payload, optionally filtered by `?type=`.
    /// Fully local — no network, no BYOK key — so it runs synchronously here and
    /// gives `get_timeline`'s `canGenerate` gate a real "catalog is listable"
    /// signal to build on.
    fn list_models_catalog(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: ListModelsArgs = decode_tool_args(args, "")?;
        let kind = gen_catalog::parse_kind(a.kind.as_deref())?;
        let payload = gen_catalog::list_models_payload(kind);
        Ok(ToolResult::ok(payload.to_string()))
    }

    // MARK: - Render + import tool bodies (backed by the MediaBridge)

    /// `inspect_timeline`: composite one project frame, or `maxFrames` frames
    /// evenly sampled across `[startFrame, endFrame)`, downscaled for tokens.
    /// 1:1 port of upstream `ToolExecutor+InspectTimeline.inspectTimeline`
    /// (frame-range validation + even sampling here; the GPU composite + JPEG
    /// encode behind the [`MediaBridge`]). Returns MCP image content per frame plus
    /// a trailing meta text block (`fps`/`width`/`height`/`totalFrames`/
    /// `frameNumbers`).
    fn inspect_timeline(&self, args: &Value, before: &Timeline) -> Result<ToolResult, ToolError> {
        let a: InspectTimelineArgs = decode_tool_args(args, "")?;

        let total_frames = before.total_frames();
        if total_frames <= 0 {
            return Ok(ToolResult::error("Timeline is empty — nothing to render."));
        }

        let start_frame = a.start_frame.unwrap_or(0);
        if start_frame < 0 || start_frame >= total_frames {
            return Ok(ToolResult::error(format!(
                "startFrame {start_frame} out of range [0, {total_frames})."
            )));
        }

        // Single frame, or evenly-sampled frames across [startFrame, endFrame).
        // Mirrors upstream exactly: count = clamp(maxFrames|default, ≤max, ≤span),
        // frame_i = startFrame + floor(span * (i + 0.5) / count).
        let sampled: Vec<i32> = if let Some(raw_end) = a.end_frame {
            let end_frame = raw_end.min(total_frames);
            if end_frame <= start_frame {
                return Ok(ToolResult::error(format!(
                    "endFrame must be greater than startFrame ({start_frame})."
                )));
            }
            let span = end_frame - start_frame;
            let count = a
                .max_frames
                .unwrap_or(INSPECT_TIMELINE_DEFAULT_FRAMES)
                .min(INSPECT_TIMELINE_MAX_FRAMES)
                .min(span)
                .max(1);
            (0..count)
                .map(|i| {
                    let offset = (span as f64 * (i as f64 + 0.5) / count as f64).floor() as i32;
                    start_frame + offset
                })
                .collect()
        } else {
            vec![start_frame]
        };

        let Some(bridge) = self.bridge.as_ref() else {
            return Ok(ToolResult::error(
                "inspect_timeline: rendering is not available in this build",
            ));
        };

        let InspectResult {
            frames,
            width,
            height,
        } = bridge
            .inspect_timeline(&sampled, INSPECT_TIMELINE_MAX_DIMENSION)
            .map_err(|e| ToolError::new(e.message))?;

        if frames.is_empty() {
            return Ok(ToolResult::error("Failed to render timeline frames."));
        }

        // Image blocks first, then the meta text block — upstream's
        // `imageBlocks + [metaJSON]` order.
        let mut blocks: Vec<Block> = frames.iter().map(frame_to_block).collect();
        let rendered_frames: Vec<i32> = frames.iter().map(|f| f.frame).collect();
        let meta = serde_json::json!({
            "fps": before.fps,
            "width": width,
            "height": height,
            "totalFrames": total_frames,
            "frameNumbers": rendered_frames,
        });
        blocks.push(Block::text(meta.to_string()));
        Ok(ToolResult::blocks(blocks))
    }

    /// `import_media`: import external media (url / path / bytes) through the SAME
    /// path as the user-facing import, via the [`MediaBridge`]. 1:1 port of
    /// upstream `ToolExecutor+Import.importMedia` — exactly-one-of-source
    /// validation + folderId existence check here; the IO (download / recursive
    /// path import / bytes write + poster/manifest/event) behind the bridge.
    fn import_media(
        &self,
        args: &Value,
        manifest: &MediaManifest,
    ) -> Result<ToolResult, ToolError> {
        let a: ImportMediaArgs = decode_tool_args(args, "")?;
        // Validate the nested `source` object's own keys (upstream
        // `validateUnknownKeys(source, path: "source")`). The top-level decode
        // sees `source` as an opaque object, so an unknown key inside it would be
        // silently dropped without this explicit second decode.
        let source = match args.get("source") {
            Some(raw) => decode_tool_args::<ImportSourceArg>(raw, "source")?,
            None => {
                return Ok(ToolResult::error("Missing required 'source' object"));
            }
        };

        // Exactly one of url / path / bytes.
        let set_count = [&source.url, &source.path, &source.bytes]
            .iter()
            .filter(|v| v.is_some())
            .count();
        if set_count != 1 {
            return Ok(ToolResult::error(format!(
                "source must set exactly one of 'url', 'path', or 'bytes' (got {set_count})"
            )));
        }

        // folderId, when provided, must name an existing folder (upstream
        // `resolveFolderId`). There is no reference fallback for a tool call.
        if let Some(folder_id) = a.folder_id.as_deref() {
            if !manifest.folders.iter().any(|f| f.id == folder_id) {
                return Ok(ToolResult::error(format!(
                    "folderId not found: {folder_id}"
                )));
            }
        }

        let import_source = if let Some(path) = source.path.clone() {
            ImportSource::Path(path)
        } else if let Some(base64) = source.bytes.clone() {
            let Some(mime_type) = source.mime_type.clone() else {
                return Ok(ToolResult::error(
                    "source.mimeType is required when source.bytes is set",
                ));
            };
            ImportSource::Bytes { base64, mime_type }
        } else if let Some(url) = source.url.clone() {
            ImportSource::Url {
                url,
                mime_type: source.mime_type.clone(),
            }
        } else {
            // Unreachable: set_count == 1 guaranteed one branch above.
            return Ok(ToolResult::error("import_media: no source set"));
        };

        let Some(bridge) = self.bridge.as_ref() else {
            return Ok(ToolResult::error(
                "import_media: importing is not available in this build",
            ));
        };

        let outcome = bridge
            .import_media(import_source, a.name.clone(), a.folder_id.clone())
            .map_err(|e| ToolError::new(e.message))?;
        Ok(ToolResult::ok(outcome.message))
    }

    /// `search_media`: content search over the library — visual (SigLIP2
    /// semantic) + spoken (transcript keyword), ranked independently and never
    /// blended. 1:1 port of `ToolExecutor+Search.searchMedia`
    /// (`ToolExecutor+Search.swift:6-32`): validate `query`/`scope`/`limit` +
    /// optional `mediaRef` restrict here, resolve the candidate set from the
    /// manifest, run both searches behind the [`MediaBridge`], and shape the
    /// upstream JSON envelope (`status`/`indexableAssets`/`indexedAssets`/
    /// `moments`/`spoken`). Scores are uncalibrated (ordering only). When the
    /// visual index isn't ready, `moments` may be empty — the model is told via
    /// `status` and the `indexableAssets`/`indexedAssets` counts, and Spoken +
    /// Files-style name lookups still work.
    fn search_media(
        &self,
        args: &Value,
        manifest: &MediaManifest,
    ) -> Result<ToolResult, ToolError> {
        use serde_json::json;
        let a: SearchMediaArgs = decode_tool_args(args, "")?;
        let query = a.query.trim().to_string();
        if query.is_empty() {
            return Ok(ToolResult::error("search_media: query is empty"));
        }
        // scope ∈ {visual, spoken, both}, default both (upstream).
        let scope = a.scope.as_deref().unwrap_or("both");
        if !matches!(scope, "visual" | "spoken" | "both") {
            return Ok(ToolResult::error(format!(
                "search_media: scope must be visual, spoken, or both (got '{scope}')"
            )));
        }
        // limit default 10, clamped to 1..=50 (upstream `min(max(limit,1),50)`).
        let limit = a.limit.unwrap_or(10).clamp(1, 50) as usize;

        // Optional `mediaRef` restricts the search to one existing asset.
        let restrict: Option<String> = match a.media_ref.as_deref() {
            Some(ref_id) => {
                let entry = manifest.entries.iter().find(|e| e.id == ref_id);
                match entry {
                    Some(e) => Some(e.id.clone()),
                    None => {
                        return Ok(ToolResult::error(format!(
                            "search_media: media not found: {ref_id}"
                        )));
                    }
                }
            }
            None => None,
        };

        // Build the candidate set from the manifest (kind → visual/spoken).
        use opentake_domain::ClipType;
        let candidates: Vec<SearchCandidate> = manifest
            .entries
            .iter()
            .filter(|e| restrict.as_deref().is_none_or(|r| r == e.id))
            .map(|e| SearchCandidate {
                media_ref: e.id.clone(),
                is_visual: matches!(e.kind, ClipType::Video | ClipType::Image),
                is_spoken: matches!(e.kind, ClipType::Video | ClipType::Audio),
            })
            .collect();

        let Some(bridge) = self.bridge.as_ref() else {
            return Ok(ToolResult::error(
                "search_media: search is not available in this build",
            ));
        };
        let result = bridge
            .search_media(&candidates, &query, scope, limit)
            .map_err(|e| ToolError::new(e.message))?;

        // Shape the upstream JSON. `name` per hit is looked up from the manifest.
        let name_of = |media_ref: &str| -> String {
            manifest
                .entries
                .iter()
                .find(|e| e.id == media_ref)
                .map(|e| e.name.clone())
                .unwrap_or_default()
        };

        let mut payload = serde_json::Map::new();
        if scope != "spoken" {
            // Visual group: status + counts always present; moments when ready.
            payload.insert("status".into(), json!(result.status.as_str()));
            payload.insert("indexableAssets".into(), json!(result.indexable_assets));
            if let Some(indexed) = result.indexed_assets {
                payload.insert("indexedAssets".into(), json!(indexed));
            }
            let moments: Vec<Value> = result
                .moments
                .iter()
                .map(|h| {
                    let mut m = serde_json::Map::new();
                    m.insert("mediaRef".into(), json!(h.media_ref));
                    m.insert("name".into(), json!(name_of(&h.media_ref)));
                    m.insert("score".into(), json!(h.score as f64));
                    if h.is_image {
                        m.insert("type".into(), json!("image"));
                    } else {
                        m.insert("startSeconds".into(), json!(h.start_seconds));
                        m.insert("endSeconds".into(), json!(h.end_seconds));
                    }
                    Value::Object(m)
                })
                .collect();
            payload.insert("moments".into(), json!(moments));
        }
        if scope != "visual" {
            let spoken: Vec<Value> = result
                .spoken
                .iter()
                .map(|h| {
                    json!({
                        "mediaRef": h.media_ref,
                        "name": name_of(&h.media_ref),
                        "startSeconds": h.start_seconds,
                        "endSeconds": h.end_seconds,
                        "text": h.text,
                    })
                })
                .collect();
            payload.insert("spoken".into(), json!(spoken));
        }

        let out = round_floats_3dp(Value::Object(payload));
        Ok(ToolResult::ok(out.to_string()))
    }

    /// `get_transcript`: the live timeline transcript in project frames. Walks
    /// every caption-eligible audio/video clip, transcribes each unique source
    /// once (cached, via the [`MediaBridge`]), maps each word through the clip's
    /// trim/speed/position into project frames, and emits compact
    /// `[text, startFrame, endFrame]` rows per clip with paging + optional
    /// `clipId` scoping. 1:1 port of `ToolExecutor+Timeline.getTranscript`
    /// (`:548-628`): the frag selection + window validation + JSON envelope here;
    /// the pure word→frame mapping in `opentake_media::timeline_transcript`; the
    /// transcription (whisper + cache) behind the bridge.
    fn get_transcript(
        &self,
        args: &Value,
        before: &Timeline,
        manifest: &MediaManifest,
    ) -> Result<ToolResult, ToolError> {
        let a: GetTranscriptArgs = decode_tool_args(args, "")?;
        let fps = before.fps;

        // Window validation (upstream: startFrame must be < endFrame).
        if let (Some(s), Some(e)) = (a.start_frame, a.end_frame) {
            if s >= e {
                return Ok(ToolResult::error(format!(
                    "startFrame ({s}) must be less than endFrame ({e})"
                )));
            }
        }

        // Caption-eligible fragments in timeline order (mirrors `captionTargets`).
        let frags = caption_target_fragments(before, manifest, a.clip_id.as_deref());
        if a.clip_id.is_some() && frags.is_empty() {
            return Ok(ToolResult::error(format!(
                "Clip {} not found, or it has no audio/video to transcribe.",
                a.clip_id.as_deref().unwrap_or("")
            )));
        }
        if frags.is_empty() {
            // No audio/video on the timeline — an empty transcript, not an error
            // (upstream returns an empty `clips` array).
            let out = serde_json::json!({
                "fps": fps,
                "timing": "projectFrames",
                "wordFormat": ["text", "start", "end"],
                "clips": [],
            });
            return Ok(ToolResult::ok(out.to_string()));
        }

        // Transcribe each UNIQUE source once (cached), via the bridge. Skip —
        // don't fail — on per-source errors, collecting `{file, reason}`.
        let unique_sources = unique_transcript_sources(&frags);
        let Some(bridge) = self.bridge.as_ref() else {
            return Ok(ToolResult::error(
                "get_transcript: transcription is not available in this build",
            ));
        };
        let source_results = bridge
            .transcribe_sources(&unique_sources)
            .map_err(|e| ToolError::new(e.message))?;

        // Index transcripts + collect skips by media_ref.
        let mut transcripts: BTreeMap<String, opentake_media::TranscriptionResult> =
            BTreeMap::new();
        let mut skipped: Vec<serde_json::Value> = Vec::new();
        for r in source_results {
            if let Some(t) = r.transcript {
                transcripts.insert(r.media_ref, t);
            } else if let Some(reason) = r.error {
                let file = manifest
                    .entries
                    .iter()
                    .find(|e| e.id == r.media_ref)
                    .map(|e| e.name.clone())
                    .unwrap_or_else(|| r.media_ref.clone());
                skipped.push(serde_json::json!({ "file": file, "reason": reason }));
            }
        }

        // Assemble via the pure mapper: attach each frag's transcript by media_ref.
        let mapper_frags: Vec<opentake_media::ClipFragment<'_>> = frags
            .iter()
            .map(|f| opentake_media::ClipFragment {
                clip_id: f.clip.id.clone(),
                track_index: f.track_index,
                clip: f.clip,
                transcript: transcripts.get(&f.clip.media_ref),
            })
            .collect();
        let assembled =
            opentake_media::timeline_transcript(mapper_frags, fps, a.start_frame, a.end_frame);

        // Serialize the upstream envelope: clips with nested compact word rows.
        let clips_json: Vec<serde_json::Value> = assembled
            .clips
            .iter()
            .map(|c| {
                let words: Vec<serde_json::Value> = c
                    .words
                    .iter()
                    .map(|w| serde_json::json!([w.text, w.start_frame, w.end_frame]))
                    .collect();
                serde_json::json!({
                    "clipId": c.clip_id,
                    "trackIndex": c.track_index,
                    "startFrame": c.start_frame,
                    "endFrame": c.end_frame,
                    "words": words,
                })
            })
            .collect();

        let mut out = serde_json::json!({
            "fps": fps,
            "timing": "projectFrames",
            "wordFormat": ["text", "start", "end"],
            "clips": clips_json,
        });
        if assembled.total_words > opentake_media::TIMELINE_MAX_WORDS {
            out["totalWords"] = serde_json::json!(assembled.total_words);
            if let Some(next) = assembled.next_start_frame {
                out["nextStartFrame"] = serde_json::json!(next);
                out["wordsNote"] = serde_json::json!(format!(
                    "First {} of {} words. Continue with startFrame = nextStartFrame.",
                    opentake_media::TIMELINE_MAX_WORDS,
                    assembled.total_words
                ));
            }
        }
        if !skipped.is_empty() {
            out["skipped"] = serde_json::json!(skipped);
        }
        Ok(ToolResult::ok(out.to_string()))
    }

    /// `add_captions`: transcribe spoken audio on-device and place styled caption
    /// clips on a fresh top track — the SAME pipeline as the Captions tab, driven
    /// through the [`MediaBridge`]. 1:1 port of `ToolExecutor+Captions.addCaptions`
    /// (`:9-53`) composed with `EditorViewModel.generateCaptions`
    /// (`EditorViewModel+Captions.swift:97-117`):
    ///
    ///   * resolve caption-eligible clips (all, or just `clipIds`); auto-pick the
    ///     dominant spoken track when `clipIds` is omitted,
    ///   * transcribe each unique source once (cached; language hint bypasses the
    ///     cache) via the bridge, skip-don't-fail per source,
    ///   * build caption clip specs with the pure `opentake_media::caption_specs`
    ///     (packing / timing / overlap all in that tested module), using the
    ///     style + placement from the args and this timeline's canvas for the
    ///     text-fit predicate and per-line transform,
    ///   * place them atomically via [`EditCommand::AddCaptions`] (one new track,
    ///     one undo step, each clip carrying the shared `captionGroupId`).
    ///
    /// `censorProfanity` is accepted for parity but is a no-op with the whisper
    /// backend (Apple's `.etiquetteReplacements` has no whisper equivalent yet);
    /// the value is threaded into transcription so it takes effect if/when the
    /// backend gains masking, matching upstream's boundary. `fontName`/`color`/
    /// `centerX`/`centerY`/`fontSize`/`textCase` map onto the caption style/placement.
    fn add_captions(
        &self,
        args: &Value,
        before: &Timeline,
        manifest: &MediaManifest,
    ) -> Result<ToolResult, ToolError> {
        let a: AddCaptionsArgs = decode_tool_args(args, "")?;

        // Style from args (defaults: Helvetica-Bold @ AppTheme.Caption.defaultFontSize=48,
        // white). Reuses the same builder as add_texts; caption font size default
        // differs from the generic text default (96), so seed it explicitly.
        let mut style = TextStyle {
            font_size: CAPTION_DEFAULT_FONT_SIZE,
            ..TextStyle::default()
        };
        if let Some(n) = a.font_name.clone() {
            style.font_name = n;
        }
        if let Some(s) = a.font_size {
            style.font_size = s;
        }
        if let Some(hex) = a.color.as_deref() {
            let c = Rgba::from_hex(hex).ok_or_else(|| {
                ToolError::new(format!(
                    "add_captions: invalid color '{hex}' (want #RRGGBB)"
                ))
            })?;
            style.color = c;
        }

        // Placement center (AppTheme.Caption.defaultCenter = (0.5, 0.9)).
        let center_x = a.center_x.unwrap_or(CAPTION_DEFAULT_CENTER_X);
        let center_y = a.center_y.unwrap_or(CAPTION_DEFAULT_CENTER_Y);

        // Letter case (default auto).
        let case = match a.text_case.as_deref() {
            None => opentake_media::CaptionCase::Auto,
            Some(raw) => opentake_media::CaptionCase::parse(raw).ok_or_else(|| {
                ToolError::new(format!(
                    "add_captions: textCase must be auto, upper, or lower (got {raw})"
                ))
            })?,
        };

        // Resolve the requested language against the backend's supported set
        // (upstream validates via matchLocale and errors on an unsupported one).
        let language = match a.language.as_deref() {
            None => None,
            Some(lang) => Some(opentake_media::match_language(lang).ok_or_else(|| {
                ToolError::new(format!(
                    "add_captions: on-device transcription does not support language '{lang}'."
                ))
            })?),
        };

        // Caption-eligible clips (all, or restricted to clipIds). Reuses the same
        // eligibility as get_transcript (`captionTargets`), plus each clip's track id.
        let clip_ids = a.clip_ids.clone().unwrap_or_default();
        let auto_detect = clip_ids.is_empty();
        let frags = if auto_detect {
            caption_target_fragments(before, manifest, None)
        } else {
            // Restrict to the requested clips (each filtered individually so an
            // ineligible id simply contributes nothing, as upstream).
            let wanted: std::collections::BTreeSet<&str> =
                clip_ids.iter().map(String::as_str).collect();
            caption_target_fragments(before, manifest, None)
                .into_iter()
                .filter(|f| wanted.contains(f.clip.id.as_str()))
                .collect()
        };
        if frags.is_empty() {
            return Ok(ToolResult::error(
                "add_captions: no audio/video clips to caption.",
            ));
        }

        // Transcribe each unique source (cached; language bypasses the cache).
        let sources = caption_transcript_sources(&frags, language.as_deref());
        let Some(bridge) = self.bridge.as_ref() else {
            return Ok(ToolResult::error(
                "add_captions: transcription is not available in this build",
            ));
        };
        let source_results = bridge
            .transcribe_sources(&sources)
            .map_err(|e| ToolError::new(e.message))?;
        let mut transcripts: BTreeMap<String, opentake_media::TranscriptionResult> =
            BTreeMap::new();
        for r in source_results {
            if let Some(t) = r.transcript {
                transcripts.insert(r.media_ref, t);
            }
        }

        // Build caption targets (clip + track id + resolved transcript).
        let track_id_of = |ti: usize| before.tracks[ti].id.clone();
        let targets: Vec<opentake_media::CaptionTarget<'_>> = frags
            .iter()
            .map(|f| opentake_media::CaptionTarget {
                clip_id: f.clip.id.clone(),
                track_id: track_id_of(f.track_index),
                clip: f.clip,
                transcript: transcripts.get(&f.clip.media_ref),
            })
            .collect();

        // Auto-detect: keep only the dominant spoken track (upstream `generateCaptions`).
        let targets: Vec<opentake_media::CaptionTarget<'_>> = if auto_detect {
            match opentake_media::dominant_speech_track(&targets, before.fps) {
                Some(winner) => targets
                    .into_iter()
                    .filter(|t| t.track_id == winner)
                    .collect(),
                None => return Ok(ToolResult::error("No speech detected to caption.")),
            }
        } else {
            targets
        };

        // Build specs with the pure caption builder. `fits` and the per-line
        // transform use this timeline's canvas (upstream `captionLineFits` /
        // `captionTransform`), approximated by the platform-free TextLayout.
        // One fresh group id per Generate (upstream `UUID().uuidString`).
        let group_id = new_caption_group_id();
        let canvas_w = before.width.max(1) as f64;
        let canvas_h = before.height.max(1) as f64;
        let max_text_w = canvas_w * CAPTION_MAX_TEXT_WIDTH_RATIO;
        let fits = |line: &str| {
            let (w, _) = opentake_domain::TextLayout::natural_size(
                line,
                &style,
                f64::MAX, // measure natural width, then compare to the ratio budget
                canvas_h,
            );
            w <= max_text_w
        };
        let specs = opentake_media::caption_specs(&targets, before.fps, case, &group_id, &fits);
        if specs.is_empty() {
            return Ok(ToolResult::error("No speech detected to caption."));
        }

        // Map each spec to a CaptionEntry with a per-line auto-fit transform
        // centered at (center_x, center_y) (upstream `captionTransform`).
        let entries: Vec<opentake_ops::CaptionEntry> = specs
            .into_iter()
            .map(|s| {
                let (w, h) = opentake_domain::TextLayout::natural_size(
                    &s.content, &style, max_text_w, canvas_h,
                );
                let transform = Transform {
                    center_x,
                    center_y,
                    width: w / canvas_w,
                    height: h / canvas_h,
                    ..Transform::default()
                };
                opentake_ops::CaptionEntry {
                    start_frame: s.start_frame,
                    duration_frames: s.duration_frames,
                    content: s.content,
                    text_style: style.clone(),
                    transform,
                    caption_group_id: s.caption_group_id,
                }
            })
            .collect();

        let count = entries.len();
        let res = self.apply(EditCommand::AddCaptions { entries })?;
        if !res.changed {
            return Ok(ToolResult::error("No speech detected to caption."));
        }
        Ok(ToolResult::ok(format!(
            "Added {count} caption{}.",
            if count == 1 { "" } else { "s" }
        )))
    }

    // MARK: - Editing tool bodies

    fn add_clips(
        &self,
        args: &Value,
        manifest: &MediaManifest,
        op: &mut OpContext,
    ) -> Result<ToolResult, ToolError> {
        let a: AddClipsArgs = decode_tool_args(args, "")?;
        let mut entries = Vec::with_capacity(a.entries.len());
        let mut media_refs = Vec::new();
        let mut omitted_count = 0usize;
        let mut explicit_count = 0usize;
        for (i, raw) in a.entries.iter().enumerate() {
            let e: AddClipEntry = decode_tool_args(raw, &format!("entries[{i}]"))?;
            let (media_type, has_audio) = resolve_media_kind(manifest, &e.media_ref);
            if e.track_index.is_some() {
                explicit_count += 1;
            } else {
                omitted_count += 1;
            }
            media_refs.push(e.media_ref.clone());
            entries.push(ClipEntry {
                media_ref: e.media_ref,
                media_type,
                source_clip_type: media_type,
                track_index: e.track_index.unwrap_or(0),
                start_frame: e.start_frame,
                duration_frames: e.duration_frames,
                trim_start_frame: e.trim_start_frame,
                trim_end_frame: e.trim_end_frame,
                has_audio,
                // `place_clip` only actually links when the target is a video
                // track and the source is a video-with-audio asset (see
                // `opentake_ops::ops::place::place_clip`), so this is safe to
                // request unconditionally.
                add_linked_audio: true,
                transform: None,
            });
        }
        if omitted_count > 0 && explicit_count > 0 {
            return Ok(ToolResult::error(
                "add_clips: mixing entries with trackIndex and entries without trackIndex is rejected; split into separate calls",
            ));
        }
        op.added_media_refs = media_refs;
        let command = if omitted_count > 0 {
            op.track_index = None;
            EditCommand::AddClipsAutoTrack { entries }
        } else {
            op.track_index = entries.first().map(|e| e.track_index);
            EditCommand::AddClips { entries }
        };
        let res = self.apply(command)?;
        Ok(ToolResult::ok(res.summary))
    }

    fn insert_clips(
        &self,
        args: &Value,
        manifest: &MediaManifest,
    ) -> Result<ToolResult, ToolError> {
        let a: InsertClipsArgs = decode_tool_args(args, "")?;
        let mut entries = Vec::with_capacity(a.entries.len());
        for (i, raw) in a.entries.iter().enumerate() {
            let e: InsertClipEntry = decode_tool_args(raw, &format!("entries[{i}]"))?;
            let (media_type, has_audio) = resolve_media_kind(manifest, &e.media_ref);
            entries.push(ClipEntry {
                media_ref: e.media_ref,
                media_type,
                source_clip_type: media_type,
                track_index: a.track_index,
                start_frame: a.at_frame,
                duration_frames: e.duration_frames.unwrap_or(0),
                trim_start_frame: e.trim_start_frame,
                trim_end_frame: e.trim_end_frame,
                has_audio,
                // See the matching note in `add_clips`: `place_clip` gates the
                // actual link on target-track/source-type/has_audio, so this is
                // safe to request unconditionally.
                add_linked_audio: true,
                transform: None,
            });
        }
        let res = self.apply(EditCommand::InsertClips {
            track_index: a.track_index,
            at_frame: a.at_frame,
            entries,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn move_clips(&self, args: &Value, before: &Timeline) -> Result<ToolResult, ToolError> {
        let a: MoveClipsArgs = decode_tool_args(args, "")?;
        let mut moves = Vec::with_capacity(a.moves.len());
        for (i, raw) in a.moves.iter().enumerate() {
            let m: MoveEntry = decode_tool_args(raw, &format!("moves[{i}]"))?;
            // Optional to_track / to_frame default to the clip's current location.
            let (cur_track, cur_frame) = clip_location(before, &m.clip_id);
            moves.push(ClipMove {
                clip_id: m.clip_id,
                to_track: m.to_track.or(cur_track).unwrap_or(0),
                to_frame: m.to_frame.or(cur_frame).unwrap_or(0),
            });
        }
        let res = self.apply(EditCommand::MoveClips { moves })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn remove_clips(
        &self,
        args: &Value,
        before: &Timeline,
        op: &mut OpContext,
    ) -> Result<ToolResult, ToolError> {
        let a: RemoveClipsArgs = decode_tool_args(args, "")?;
        op.clip_ids = a.clip_ids.clone();
        op.track_index = a
            .clip_ids
            .first()
            .and_then(|id| clip_location(before, id).0);
        let res = self.apply(EditCommand::RemoveClips {
            clip_ids: a.clip_ids,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn remove_tracks(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: RemoveTracksArgs = decode_tool_args(args, "")?;
        let res = self.apply(EditCommand::RemoveTracks {
            track_indexes: a.track_indexes,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn split_clip(
        &self,
        args: &Value,
        before: &Timeline,
        op: &mut OpContext,
    ) -> Result<ToolResult, ToolError> {
        let a: SplitClipArgs = decode_tool_args(args, "")?;
        op.track_index = clip_location(before, &a.clip_id).0;
        op.clip_ids = vec![a.clip_id.clone()];
        let res = self.apply(EditCommand::SplitClip {
            clip_id: a.clip_id,
            at_frame: a.at_frame,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn set_keyframes(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: SetKeyframesArgs = decode_tool_args(args, "")?;
        let (property, payload) = build_keyframe_payload(&a)?;
        let res = self.apply(EditCommand::SetKeyframes {
            clip_id: a.clip_id,
            property,
            payload,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn detect_beats(&self, args: &Value, before: &Timeline) -> Result<ToolResult, ToolError> {
        let a: DetectBeatsArgs = decode_tool_args(args, "")?;
        let beats = self.detect_beat_hints(
            before,
            BeatAnalysisRequest {
                clip_id: a.clip_id.as_deref(),
                media_ref: a.media_ref.as_deref(),
                start_frame: a.start_frame,
                end_frame: a.end_frame,
                sensitivity: a.sensitivity,
                tool_name: "detect_beats",
            },
        )?;
        let payload = serde_json::json!({
            "applied": false,
            "beats": beats.iter().map(|beat| serde_json::json!({
                "frame": beat.frame,
                "strength": beat.strength,
            })).collect::<Vec<_>>(),
            "count": beats.len(),
        });
        Ok(ToolResult::ok(round_floats_3dp(payload).to_string()))
    }

    fn auto_cut_to_beats(&self, args: &Value, before: &Timeline) -> Result<ToolResult, ToolError> {
        let a: AutoCutToBeatsArgs = decode_tool_args(args, "")?;
        let beats = self.detect_beat_hints(
            before,
            BeatAnalysisRequest {
                clip_id: a.beat_clip_id.as_deref(),
                media_ref: a.beat_media_ref.as_deref(),
                start_frame: a.start_frame,
                end_frame: a.end_frame,
                sensitivity: None,
                tool_name: "auto_cut_to_beats",
            },
        )?;
        let min_gap = a.min_clip_frames.unwrap_or(1).max(1);
        let max_gap = a.max_clip_frames.unwrap_or(i32::MAX).max(min_gap);
        let mut cut_frames = Vec::new();
        let mut last = None;
        for beat in &beats {
            if let Some(prev) = last {
                let gap = beat.frame - prev;
                if gap < min_gap {
                    continue;
                }
                if gap > max_gap {
                    cut_frames.push(prev + max_gap);
                }
            }
            cut_frames.push(beat.frame);
            last = Some(beat.frame);
        }
        cut_frames.sort_unstable();
        cut_frames.dedup();

        let placements = a
            .clip_ids
            .unwrap_or_default()
            .into_iter()
            .zip(cut_frames.iter().copied())
            .map(|(clip_id, to_frame)| {
                serde_json::json!({
                    "clipId": clip_id,
                    "toFrame": to_frame,
                })
            })
            .collect::<Vec<_>>();

        let payload = serde_json::json!({
            "applied": false,
            "alignCuts": a.align_cuts.unwrap_or(false),
            "beats": beats.iter().map(|beat| serde_json::json!({
                "frame": beat.frame,
                "strength": beat.strength,
            })).collect::<Vec<_>>(),
            "cutFrames": cut_frames,
            "placements": placements,
            "note": "Preview only. Apply returned frames through split_clip/move_clips/ripple_delete_ranges as needed.",
        });
        Ok(ToolResult::ok(round_floats_3dp(payload).to_string()))
    }

    fn smart_reframe(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let _: SmartReframeArgs = decode_tool_args(args, "")?;
        Ok(ToolResult::error(
            "smart_reframe: needs vision analysis backend; CoreHandle does not expose sampled frames or saliency/subject analysis yet",
        ))
    }

    fn tighten_silences(&self, args: &Value, before: &Timeline) -> Result<ToolResult, ToolError> {
        let a: TightenSilencesArgs = decode_tool_args(args, "")?;
        let targets = silence_targets(before, &a)?;
        let spec = analysis_pcm_spec();
        let fps = timeline_fps(before);
        let mut config = SilenceDetectionConfig::with_window(
            spec.sample_rate,
            fps,
            analysis_window_samples(spec.sample_rate),
        );
        config.rms_threshold = threshold_db_to_rms(a.threshold_db.unwrap_or(-40.0));
        config.min_silence_frames = a.min_silence_frames.unwrap_or(12).max(1) as u64;
        let padding = a.padding_frames.unwrap_or(3).max(0);

        let mut by_track: BTreeMap<usize, Vec<(i32, i32)>> = BTreeMap::new();
        let mut clip_payloads = Vec::new();
        let mut warnings = Vec::new();
        for target in targets {
            let source_range = visible_source_range_secs(target.clip, fps);
            let pcm = match self.handle.extract_analysis_pcm(
                &target.clip.media_ref,
                spec,
                Some(source_range),
            ) {
                Ok(pcm) => pcm,
                Err(e) => {
                    warnings.push(format!("{}: {e}", target.clip.id));
                    continue;
                }
            };
            config.sample_rate = pcm.spec.sample_rate;
            config.window_size_samples = analysis_window_samples(pcm.spec.sample_rate);
            config.hop_size_samples = (config.window_size_samples / 2).max(1);
            let ranges = detect_silences(&pcm.samples_f32, config);
            let mut clip_ranges = Vec::new();
            for range in ranges {
                let start_seconds = source_range.0 + range.start_frame as f64 / fps;
                let end_seconds = source_range.0 + range.end_frame as f64 / fps;
                let start = source_seconds_to_timeline_frame_clamped(
                    target.clip,
                    start_seconds,
                    before.fps,
                ) + padding;
                let end =
                    source_seconds_to_timeline_frame_clamped(target.clip, end_seconds, before.fps)
                        - padding;
                if end <= start {
                    continue;
                }
                by_track
                    .entry(target.track_index)
                    .or_default()
                    .push((start, end));
                clip_ranges.push(serde_json::json!([start, end]));
            }
            clip_payloads.push(serde_json::json!({
                "clipId": target.clip.id,
                "trackIndex": target.track_index,
                "ranges": clip_ranges,
            }));
        }

        for ranges in by_track.values_mut() {
            ranges.sort_unstable();
            ranges.dedup();
        }
        let commands = by_track
            .iter()
            .filter(|(_, ranges)| !ranges.is_empty())
            .map(|(track_index, ranges)| {
                serde_json::json!({
                    "tool": "ripple_delete_ranges",
                    "args": {
                        "trackIndex": track_index,
                        "units": "frames",
                        "ranges": ranges.iter().map(|(start, end)| {
                            serde_json::json!([start, end])
                        }).collect::<Vec<_>>(),
                    }
                })
            })
            .collect::<Vec<_>>();

        let payload = serde_json::json!({
            "applied": false,
            "clips": clip_payloads,
            "commands": commands,
            "warnings": warnings,
            "note": "Preview only. Run each returned ripple_delete_ranges command to apply.",
        });
        Ok(ToolResult::ok(round_floats_3dp(payload).to_string()))
    }

    fn detect_beat_hints(
        &self,
        timeline: &Timeline,
        request: BeatAnalysisRequest<'_>,
    ) -> Result<Vec<BeatHint>, ToolError> {
        let target = analysis_target(
            timeline,
            &self.handle.media(),
            request.clip_id,
            request.media_ref,
            request.start_frame,
            request.end_frame,
            request.tool_name,
        )?;
        let spec = analysis_pcm_spec();
        let pcm = self
            .handle
            .extract_analysis_pcm(&target.media_ref, spec, target.source_range)
            .map_err(|e| ToolError::new(format!("{}: {e}", request.tool_name)))?;
        let fps = timeline_fps(timeline);
        let mut config = BeatDetectionConfig::with_window(
            pcm.spec.sample_rate,
            fps,
            analysis_window_samples(pcm.spec.sample_rate),
        );
        config.min_onset_strength = sensitivity_to_onset_threshold(request.sensitivity);
        let beats = detect_beats(&pcm.samples_f32, config)
            .into_iter()
            .map(|beat| BeatHint {
                frame: target.map_relative_frame(beat.frame as i32, timeline.fps),
                strength: beat.strength,
            })
            .collect();
        Ok(beats)
    }

    fn ripple_delete_ranges(
        &self,
        args: &Value,
        before: &Timeline,
        op: &mut OpContext,
    ) -> Result<ToolResult, ToolError> {
        let a: RippleDeleteRangesArgs = decode_tool_args(args, "")?;
        let units = parse_range_units(a.units.as_deref())?;
        let track_index = match (a.track_index, a.clip_id.as_deref()) {
            (Some(track_index), None) => {
                if units == RangeUnits::Seconds {
                    return Ok(ToolResult::error(
                        "ripple_delete_ranges: units='seconds' is only valid with clipId; trackIndex mode requires units='frames'",
                    ));
                }
                track_index
            }
            (None, Some(clip_id)) => {
                let (track_index, _) = clip_location(before, clip_id);
                track_index.ok_or_else(|| {
                    ToolError::new(format!("ripple_delete_ranges: clip not found: {clip_id}"))
                })?
            }
            (Some(_), Some(_)) => {
                return Ok(ToolResult::error(
                    "ripple_delete_ranges: pass exactly one of trackIndex or clipId",
                ));
            }
            (None, None) => {
                return Ok(ToolResult::error(
                    "ripple_delete_ranges: missing trackIndex or clipId",
                ));
            }
        };
        op.track_index = Some(track_index);
        if let Some(clip_id) = a.clip_id.as_ref() {
            op.clip_ids = vec![clip_id.clone()];
        }
        let ranges = build_ripple_ranges(before, &a, units)?;
        let res = self.apply(EditCommand::RippleDeleteRanges {
            track_index,
            ranges,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn add_texts(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: AddTextsArgs = decode_tool_args(args, "")?;
        let mut entries = Vec::with_capacity(a.entries.len());
        for (i, raw) in a.entries.iter().enumerate() {
            let e: AddTextEntry = decode_tool_args(raw, &format!("entries[{i}]"))?;
            entries.push(TextEntry {
                track_index: e.track_index.unwrap_or(0),
                start_frame: e.start_frame,
                duration_frames: e.duration_frames,
                content: e.content,
                text_style: build_text_style(
                    e.font_name,
                    e.font_size,
                    e.color.as_deref(),
                    e.alignment.as_deref(),
                ),
                transform: build_transform(e.transform),
            });
        }
        let res = self.apply(EditCommand::AddTexts { entries })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn create_folder(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: CreateFolderArgs = decode_tool_args(args, "")?;
        // Single form (name / parentFolderId) only; the batch `entries` form is
        // not yet wired (one CreateFolder command per call).
        if a.entries.is_some() {
            return Ok(ToolResult::error(
                "create_folder: batch 'entries' form not yet implemented; pass name/parentFolderId",
            ));
        }
        let Some(name) = a.name else {
            return Err(ToolError::new("arguments: missing required field 'name'"));
        };
        let res = self.apply(EditCommand::CreateFolder {
            name,
            parent_folder_id: a.parent_folder_id,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn move_to_folder(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: MoveToFolderArgs = decode_tool_args(args, "")?;
        if a.entries.is_some() {
            return Ok(ToolResult::error(
                "move_to_folder: batch 'entries' form not yet implemented; pass assetIds/folderId",
            ));
        }
        let Some(asset_ids) = a.asset_ids else {
            return Err(ToolError::new(
                "arguments: missing required field 'assetIds'",
            ));
        };
        let res = self.apply(EditCommand::MoveToFolder {
            asset_ids,
            folder_id: a.folder_id,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn set_clip_properties(
        &self,
        args: &Value,
        before: &Timeline,
        manifest: &MediaManifest,
    ) -> Result<ToolResult, ToolError> {
        let a: SetClipPropertiesArgs = decode_tool_args(args, "")?;
        let clip_ids = a.clip_ids.clone();
        let properties = ClipProperties {
            duration_frames: a.duration_frames,
            trim_start_frame: a.trim_start_frame,
            trim_end_frame: a.trim_end_frame,
            speed: a.speed,
            volume: a.volume,
            opacity: a.opacity,
            transform: None,
            text_content: a.content.clone(),
            ..Default::default()
        };
        let Some(transform_patch) = a.transform else {
            let res = self.apply(EditCommand::SetClipProperties {
                clip_ids,
                properties: Box::new(properties),
            })?;
            return Ok(ToolResult::ok(res.summary));
        };

        let mut per_clip = Vec::new();
        for clip_id in &clip_ids {
            let clip = find_clip(before, clip_id).ok_or_else(|| {
                ToolError::new(format!("set_clip_properties: clip not found: {clip_id}"))
            })?;
            let aspect = media_canvas_aspect(before, manifest, clip)
                .or_else(|| current_transform_aspect(clip.transform));
            let mut clip_properties = properties.clone();
            clip_properties.transform = Some(merge_transform_arg(
                clip.transform,
                transform_patch.clone(),
                aspect,
            ));
            per_clip.push((clip_id.clone(), clip_properties));
        }

        let mut summaries = Vec::new();
        for (clip_id, clip_properties) in per_clip {
            let res = self.apply(EditCommand::SetClipProperties {
                clip_ids: vec![clip_id],
                properties: Box::new(clip_properties),
            })?;
            summaries.push(res.summary);
        }
        Ok(ToolResult::ok(summaries.join("; ")))
    }

    fn set_color_grade(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: SetColorGradeArgs = decode_tool_args(args, "")?;
        let grade = if a.clear == Some(true) {
            None
        } else {
            Some(color_grade_from_args(&a))
        };
        let res = self.apply(EditCommand::SetColorGrade {
            clip_ids: a.clip_ids,
            grade,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn chroma_key(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: ChromaKeyArgs = decode_tool_args(args, "")?;
        let chroma_key = if a.clear == Some(true) {
            None
        } else {
            Some(chroma_key_from_args(&a))
        };
        let res = self.apply(EditCommand::SetChromaKey {
            clip_ids: a.clip_ids,
            chroma_key,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn set_mask(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: SetMaskArgs = decode_tool_args(args, "")?;
        let mut masks = Vec::with_capacity(a.masks.len());
        for (i, raw) in a.masks.iter().enumerate() {
            let m: MaskArg = decode_tool_args(raw, &format!("masks[{i}]"))?;
            masks.push(mask_from_arg(&m, &format!("masks[{i}]"))?);
        }
        let res = self.apply(EditCommand::SetMasks {
            clip_ids: a.clip_ids,
            masks,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn apply_effect(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: ApplyEffectArgs = decode_tool_args(args, "")?;
        let mut effects = Vec::with_capacity(a.effects.len());
        for (i, raw) in a.effects.iter().enumerate() {
            let e: EffectArg = decode_tool_args(raw, &format!("effects[{i}]"))?;
            effects.push(Effect {
                name: e.name,
                params: e.params.unwrap_or_default(),
                enabled: e.enabled.unwrap_or(true),
            });
        }
        let res = self.apply(EditCommand::SetEffects {
            clip_ids: a.clip_ids,
            effects,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn rename_media(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: RenameMediaArgs = decode_tool_args(args, "")?;
        let entries = if let Some(raw) = a.entries {
            let mut out = Vec::with_capacity(raw.len());
            for (i, v) in raw.iter().enumerate() {
                let e: RenameMediaEntry = decode_tool_args(v, &format!("entries[{i}]"))?;
                out.push(RenameEntry {
                    id: e.media_ref,
                    name: e.name,
                });
            }
            out
        } else {
            let id = a
                .media_ref
                .ok_or_else(|| ToolError::new("arguments: missing required field 'mediaRef'"))?;
            let name = a
                .name
                .ok_or_else(|| ToolError::new("arguments: missing required field 'name'"))?;
            vec![RenameEntry { id, name }]
        };
        if entries.is_empty() {
            return Err(ToolError::new("rename_media: nothing to rename"));
        }
        let res = self.apply(EditCommand::RenameMedia { entries })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn rename_folder(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: RenameFolderArgs = decode_tool_args(args, "")?;
        let entries = if let Some(raw) = a.entries {
            let mut out = Vec::with_capacity(raw.len());
            for (i, v) in raw.iter().enumerate() {
                let e: RenameFolderEntry = decode_tool_args(v, &format!("entries[{i}]"))?;
                out.push(RenameEntry {
                    id: e.folder_id,
                    name: e.name,
                });
            }
            out
        } else {
            let id = a
                .folder_id
                .ok_or_else(|| ToolError::new("arguments: missing required field 'folderId'"))?;
            let name = a
                .name
                .ok_or_else(|| ToolError::new("arguments: missing required field 'name'"))?;
            vec![RenameEntry { id, name }]
        };
        if entries.is_empty() {
            return Err(ToolError::new("rename_folder: nothing to rename"));
        }
        let res = self.apply(EditCommand::RenameFolder { entries })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn delete_media(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: DeleteMediaArgs = decode_tool_args(args, "")?;
        if a.asset_ids.is_empty() {
            return Err(ToolError::new("arguments: 'assetIds' must not be empty"));
        }
        let res = self.apply(EditCommand::DeleteMedia {
            asset_ids: a.asset_ids,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    fn delete_folder(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: DeleteFolderArgs = decode_tool_args(args, "")?;
        if a.folder_ids.is_empty() {
            return Err(ToolError::new("arguments: 'folderIds' must not be empty"));
        }
        let res = self.apply(EditCommand::DeleteFolder {
            folder_ids: a.folder_ids,
        })?;
        Ok(ToolResult::ok(res.summary))
    }

    // MARK: - Workflow plugin (Skills) tools

    /// `list_workflows`: the installed plugins as `{id, name, description,
    /// videoType, active}` (per the tool's declared output shape).
    fn list_workflows(&self) -> Result<ToolResult, ToolError> {
        let guard = self
            .registry
            .read()
            .map_err(|_| ToolError::new("workflow registry lock poisoned"))?;
        let active = guard.active().map(|p| p.id().to_string());
        let arr: Vec<Value> = guard
            .installed()
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.manifest.id,
                    "name": p.manifest.name,
                    "description": p.manifest.description,
                    "videoType": p.manifest.video_type.primary,
                    "active": active.as_deref() == Some(p.id()),
                })
            })
            .collect();
        Ok(ToolResult::ok(Value::Array(arr).to_string()))
    }

    /// `activate_workflow`: activate a plugin by id. Returns a confirmation plus
    /// the plugin's `instructions.md`, so the agent immediately receives the
    /// skill's guidance; subsequent tool results also carry its rules/overrides
    /// via the context signal.
    fn activate_workflow(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: ActivateWorkflowArgs = decode_tool_args(args, "")?;
        let (name, instructions) = {
            let mut guard = self
                .registry
                .write()
                .map_err(|_| ToolError::new("workflow registry lock poisoned"))?;
            let plugin = guard
                .activate(&a.workflow_id)
                .map_err(|e| ToolError::new(e.to_string()))?;
            (plugin.name().to_string(), plugin.instructions_md.clone())
        };
        let mut text = format!("Activated workflow '{name}'.");
        if !instructions.trim().is_empty() {
            text.push_str("\n\n");
            text.push_str(instructions.trim());
        }
        Ok(ToolResult::ok(text))
    }

    /// `deactivate_workflow`: clear the active plugin (no-op if none active).
    fn deactivate_workflow(&self) -> Result<ToolResult, ToolError> {
        let mut guard = self
            .registry
            .write()
            .map_err(|_| ToolError::new("workflow registry lock poisoned"))?;
        let had = guard.active().is_some();
        guard.deactivate();
        drop(guard);
        Ok(ToolResult::ok(if had {
            "Deactivated the active workflow."
        } else {
            "No active workflow to deactivate."
        }))
    }

    fn undo(&self) -> Result<ToolResult, ToolError> {
        // Only revert when this dispatch session has actually pushed an edit.
        let mut stack = self.agent_undo.lock().expect("agent-undo mutex");
        if stack.pop().is_none() {
            return Ok(ToolResult::error("undo: no agent edits to revert"));
        }
        drop(stack);
        let res = self.apply_raw(EditCommand::Undo)?;
        Ok(ToolResult::ok(res.summary))
    }

    // MARK: - Apply helpers

    /// Apply an editing command through the handle, recording its action name on
    /// the agent-undo stack (so a later `undo` knows this session edited). Maps
    /// any core failure to a tool error.
    fn apply(&self, cmd: EditCommand) -> Result<opentake_ops::command::EditResult, ToolError> {
        let res = self.apply_raw(cmd)?;
        if res.changed {
            self.agent_undo
                .lock()
                .expect("agent-undo mutex")
                .push(res.action_name.clone());
        }
        Ok(res)
    }

    /// Apply without touching the agent-undo stack (used by `undo` itself).
    fn apply_raw(&self, cmd: EditCommand) -> Result<opentake_ops::command::EditResult, ToolError> {
        self.handle
            .apply(cmd)
            .map_err(|e| ToolError::new(e.to_string()))
    }
}

// MARK: - Free conversion helpers

/// Resolve a clip's media type + has-audio from the manifest entry by id.
/// Unknown refs fall back to video / no-audio; the ops layer then validates the
/// id against the track and rejects an incompatible / missing asset.
/// One caption-eligible clip located on the timeline: a borrowed [`Clip`] plus
/// its track index and whether its source is video (drives audio extraction).
/// The `get_transcript` body maps these through the pure timeline transcript
/// assembler.
struct TranscriptFrag<'a> {
    clip: &'a opentake_domain::Clip,
    track_index: usize,
    is_video: bool,
}

/// Whether a clip can be transcribed, mirroring upstream `captionCanTranscribe`:
/// its media type must be video/audio, and (when the referenced asset is known)
/// the asset must be audio, or video WITH an audio track. An unknown asset is
/// permissively eligible (upstream returns `true` when the asset is absent).
fn caption_can_transcribe(clip: &opentake_domain::Clip, manifest: &MediaManifest) -> bool {
    use opentake_domain::ClipType;
    if !matches!(clip.media_type, ClipType::Video | ClipType::Audio) {
        return false;
    }
    match manifest.entries.iter().find(|e| e.id == clip.media_ref) {
        None => true,
        Some(entry) => {
            entry.kind == ClipType::Audio
                || (entry.kind == ClipType::Video && entry.has_audio.unwrap_or(false))
        }
    }
}

/// Select the timeline's caption-eligible clips in `start_frame` order, mirroring
/// upstream `captionTargets(in:)`: keep audio/video clips that can be transcribed,
/// but drop a **video** clip whose `linkGroupId` also has a linked **audio** clip
/// (the audio partner is transcribed instead, so the video isn't double-counted).
/// When `clip_filter` is set, restrict to that single clip id. Pure over the
/// snapshot — unit-tested below.
fn caption_target_fragments<'a>(
    timeline: &'a Timeline,
    manifest: &MediaManifest,
    clip_filter: Option<&str>,
) -> Vec<TranscriptFrag<'a>> {
    use opentake_domain::ClipType;

    // Link groups that contain at least one audio clip anywhere on the timeline.
    let audio_link_groups: std::collections::BTreeSet<&str> = timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .filter(|c| c.media_type == ClipType::Audio)
        .filter_map(|c| c.link_group_id.as_deref())
        .collect();

    let mut frags: Vec<TranscriptFrag<'a>> = Vec::new();
    for (track_index, track) in timeline.tracks.iter().enumerate() {
        for clip in &track.clips {
            if let Some(filter) = clip_filter {
                if clip.id != filter {
                    continue;
                }
            }
            if !caption_can_transcribe(clip, manifest) {
                continue;
            }
            // Drop a video clip whose link group also has audio (transcribe the
            // audio partner instead).
            if clip.media_type == ClipType::Video {
                if let Some(gid) = clip.link_group_id.as_deref() {
                    if audio_link_groups.contains(gid) {
                        continue;
                    }
                }
            }
            let is_video = match manifest.entries.iter().find(|e| e.id == clip.media_ref) {
                Some(entry) => entry.kind == ClipType::Video,
                // No asset entry: fall back to the clip's own media type (upstream
                // `captionUsesVideoAudioExtraction` treats an unknown asset as
                // video when the clip's mediaType is video).
                None => clip.media_type == ClipType::Video,
            };
            frags.push(TranscriptFrag {
                clip,
                track_index,
                is_video,
            });
        }
    }
    frags.sort_by_key(|f| f.clip.start_frame);
    frags
}

/// Caption style/placement defaults, 1:1 with upstream `AppTheme.Caption`
/// (`UI/AppTheme.swift:239-249`): a 48-pt caption centered near the bottom
/// `(0.5, 0.9)`, wrapping at 90% of canvas width.
const CAPTION_DEFAULT_FONT_SIZE: f64 = 48.0;
const CAPTION_DEFAULT_CENTER_X: f64 = 0.5;
const CAPTION_DEFAULT_CENTER_Y: f64 = 0.9;
const CAPTION_MAX_TEXT_WIDTH_RATIO: f64 = 0.9;

/// Mint a fresh caption-group id (upstream `UUID().uuidString`). A process-wide
/// counter plus a nanosecond timestamp keeps it unique across Generates within a
/// session without pulling in a uuid dependency; the value is opaque (only used
/// for group membership: subtitle export + caption-group style sync).
fn new_caption_group_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("cap-{nanos:x}-{n:x}")
}

/// Distinct transcript sources for the caption fragments, tagging each with the
/// resolved `language` hint (so a foreign-language caption run transcribes with
/// the hint and bypasses the auto-detect cache). Like [`unique_transcript_sources`]
/// but carries the language for the `add_captions` path.
fn caption_transcript_sources(
    frags: &[TranscriptFrag<'_>],
    language: Option<&str>,
) -> Vec<TranscriptSource> {
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for f in frags {
        if seen.insert(f.clip.media_ref.as_str()) {
            out.push(TranscriptSource {
                media_ref: f.clip.media_ref.clone(),
                is_video: f.is_video,
                language: language.map(str::to_string),
            });
        }
    }
    out
}

/// Dedup fragments down to their distinct source assets for transcription
/// (upstream `Set(frags.map(\.url))`). First-seen `is_video` wins per media_ref.
fn unique_transcript_sources(frags: &[TranscriptFrag<'_>]) -> Vec<TranscriptSource> {
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for f in frags {
        if seen.insert(f.clip.media_ref.as_str()) {
            out.push(TranscriptSource {
                media_ref: f.clip.media_ref.clone(),
                is_video: f.is_video,
                // get_transcript reads whatever the auto-detect cache holds.
                language: None,
            });
        }
    }
    out
}

fn resolve_media_kind(
    manifest: &MediaManifest,
    media_ref: &str,
) -> (opentake_domain::ClipType, bool) {
    manifest
        .entries
        .iter()
        .find(|e| e.id == media_ref)
        .map(|e| (e.kind, e.has_audio.unwrap_or(false)))
        .unwrap_or((opentake_domain::ClipType::Video, false))
}

/// Current `(track_index, start_frame)` of a clip on the timeline, or `(None,
/// None)` if absent. Used to fill optional `move_clips` fields.
fn clip_location(timeline: &Timeline, clip_id: &str) -> (Option<usize>, Option<i32>) {
    for (ti, track) in timeline.tracks.iter().enumerate() {
        if let Some(clip) = track.clips.iter().find(|c| c.id == clip_id) {
            return (Some(ti), Some(clip.start_frame));
        }
    }
    (None, None)
}

#[derive(Clone, Debug)]
struct BeatHint {
    frame: i32,
    strength: f32,
}

struct BeatAnalysisRequest<'a> {
    clip_id: Option<&'a str>,
    media_ref: Option<&'a str>,
    start_frame: Option<i32>,
    end_frame: Option<i32>,
    sensitivity: Option<f64>,
    tool_name: &'a str,
}

struct AnalysisTarget<'a> {
    media_ref: String,
    clip: Option<&'a opentake_domain::Clip>,
    source_range: Option<(f64, f64)>,
    source_start_seconds: f64,
    project_start_frame: i32,
}

impl AnalysisTarget<'_> {
    fn map_relative_frame(&self, frame: i32, timeline_fps: i32) -> i32 {
        match self.clip {
            Some(clip) => {
                let fps = timeline_fps.max(1) as f64;
                let seconds = self.source_start_seconds + frame as f64 / fps;
                source_seconds_to_timeline_frame_clamped(clip, seconds, timeline_fps)
            }
            None => self.project_start_frame + frame,
        }
    }
}

struct SilenceTarget<'a> {
    track_index: usize,
    clip: &'a opentake_domain::Clip,
}

fn analysis_pcm_spec() -> PcmSpec {
    PcmSpec {
        sample_rate: 16_000,
        channels: 1,
        format: PcmFormat::F32,
    }
}

fn timeline_fps(timeline: &Timeline) -> f64 {
    timeline.fps.max(1) as f64
}

fn analysis_window_samples(sample_rate: u32) -> usize {
    ((sample_rate.max(1) as f64) * 0.05).round().max(1.0) as usize
}

fn sensitivity_to_onset_threshold(sensitivity: Option<f64>) -> f32 {
    let sensitivity = sensitivity.unwrap_or(0.5).clamp(0.0, 1.0);
    (0.16 - sensitivity * 0.12).clamp(0.02, 0.20) as f32
}

fn threshold_db_to_rms(db: f64) -> f32 {
    let db = db.clamp(-90.0, 0.0);
    10f64.powf(db / 20.0) as f32
}

fn analysis_target<'a>(
    timeline: &'a Timeline,
    manifest: &MediaManifest,
    clip_id: Option<&str>,
    media_ref: Option<&str>,
    start_frame: Option<i32>,
    end_frame: Option<i32>,
    tool_name: &str,
) -> Result<AnalysisTarget<'a>, ToolError> {
    match (clip_id, media_ref) {
        (Some(_), Some(_)) => Err(ToolError::new(format!(
            "{tool_name}: pass exactly one of clipId or mediaRef"
        ))),
        (None, None) => Err(ToolError::new(format!(
            "{tool_name}: missing clipId or mediaRef"
        ))),
        (Some(clip_id), None) => {
            let clip = find_clip(timeline, clip_id)
                .ok_or_else(|| ToolError::new(format!("{tool_name}: clip not found: {clip_id}")))?;
            let project_start = start_frame
                .unwrap_or(clip.start_frame)
                .clamp(clip.start_frame, clip.end_frame());
            let project_end = end_frame
                .unwrap_or(clip.end_frame())
                .clamp(clip.start_frame, clip.end_frame());
            if project_end <= project_start {
                return Err(ToolError::new(format!(
                    "{tool_name}: analysis range is empty"
                )));
            }
            let fps = timeline_fps(timeline);
            let speed = normalized_speed(clip);
            let source_start_frame =
                clip.trim_start_frame as f64 + (project_start - clip.start_frame) as f64 * speed;
            let source_end_frame =
                clip.trim_start_frame as f64 + (project_end - clip.start_frame) as f64 * speed;
            let source_range = (source_start_frame / fps, source_end_frame / fps);
            Ok(AnalysisTarget {
                media_ref: clip.media_ref.clone(),
                clip: Some(clip),
                source_range: Some(source_range),
                source_start_seconds: source_range.0,
                project_start_frame: project_start,
            })
        }
        (None, Some(media_ref)) => {
            let fps = timeline_fps(timeline);
            let start = start_frame.unwrap_or(0).max(0);
            let entry = manifest.entries.iter().find(|entry| entry.id == media_ref);
            let default_end = entry
                .and_then(|entry| (entry.duration > 0.0).then_some((entry.duration * fps) as i32));
            let source_range = match (start_frame, end_frame.or(default_end)) {
                (None, None) => None,
                (_, Some(end)) if end > start => Some((start as f64 / fps, end as f64 / fps)),
                _ => {
                    return Err(ToolError::new(format!(
                        "{tool_name}: mediaRef analysis range is empty or missing endFrame"
                    )));
                }
            };
            Ok(AnalysisTarget {
                media_ref: media_ref.to_string(),
                clip: None,
                source_range,
                source_start_seconds: source_range.map(|range| range.0).unwrap_or(0.0),
                project_start_frame: start,
            })
        }
    }
}

fn silence_targets<'a>(
    timeline: &'a Timeline,
    args: &TightenSilencesArgs,
) -> Result<Vec<SilenceTarget<'a>>, ToolError> {
    match (&args.clip_ids, args.track_index) {
        (Some(_), Some(_)) => Err(ToolError::new(
            "tighten_silences: pass clipIds or trackIndex, not both",
        )),
        (Some(ids), None) => {
            if ids.is_empty() {
                return Err(ToolError::new("tighten_silences: clipIds is empty"));
            }
            let mut out = Vec::new();
            for id in ids {
                let (track_index, clip) = find_clip_with_track(timeline, id).ok_or_else(|| {
                    ToolError::new(format!("tighten_silences: clip not found: {id}"))
                })?;
                out.push(SilenceTarget { track_index, clip });
            }
            Ok(out)
        }
        (None, Some(track_index)) => {
            let track = timeline.tracks.get(track_index).ok_or_else(|| {
                ToolError::new(format!("tighten_silences: track not found: {track_index}"))
            })?;
            Ok(track
                .clips
                .iter()
                .map(|clip| SilenceTarget { track_index, clip })
                .collect())
        }
        (None, None) => timeline
            .tracks
            .iter()
            .enumerate()
            .find(|(_, track)| track.kind == opentake_domain::ClipType::Audio)
            .map(|(track_index, track)| {
                track
                    .clips
                    .iter()
                    .map(|clip| SilenceTarget { track_index, clip })
                    .collect()
            })
            .ok_or_else(|| {
                ToolError::new("tighten_silences: missing clipIds/trackIndex and no audio track")
            }),
    }
}

fn find_clip_with_track<'a>(
    timeline: &'a Timeline,
    clip_id: &str,
) -> Option<(usize, &'a opentake_domain::Clip)> {
    timeline
        .tracks
        .iter()
        .enumerate()
        .find_map(|(track_index, track)| {
            track
                .clips
                .iter()
                .find(|clip| clip.id == clip_id)
                .map(|clip| (track_index, clip))
        })
}

fn visible_source_range_secs(clip: &opentake_domain::Clip, fps: f64) -> (f64, f64) {
    let speed = normalized_speed(clip);
    let start = clip.trim_start_frame as f64 / fps;
    let end = (clip.trim_start_frame as f64 + clip.duration_frames as f64 * speed) / fps;
    (start.max(0.0), end.max(start))
}

fn normalized_speed(clip: &opentake_domain::Clip) -> f64 {
    if clip.speed.is_finite() && clip.speed > 0.0 {
        clip.speed
    } else {
        1.0
    }
}

fn source_seconds_to_timeline_frame_clamped(
    clip: &opentake_domain::Clip,
    source_seconds: f64,
    timeline_fps: i32,
) -> i32 {
    let fps = timeline_fps.max(1) as f64;
    let source_frame = source_seconds * fps;
    let relative_source = source_frame - clip.trim_start_frame as f64;
    let frame = clip.start_frame as f64 + relative_source / normalized_speed(clip);
    (frame.round() as i32).clamp(clip.start_frame, clip.end_frame())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RangeUnits {
    Frames,
    Seconds,
}

fn parse_range_units(units: Option<&str>) -> Result<RangeUnits, ToolError> {
    match units.unwrap_or("frames") {
        "frames" => Ok(RangeUnits::Frames),
        "seconds" => Ok(RangeUnits::Seconds),
        other => Err(ToolError::new(format!(
            "units: unknown '{other}'. Allowed: frames, seconds."
        ))),
    }
}

fn build_ripple_ranges(
    timeline: &Timeline,
    args: &RippleDeleteRangesArgs,
    units: RangeUnits,
) -> Result<Vec<FrameRange>, ToolError> {
    let clip = args
        .clip_id
        .as_deref()
        .and_then(|clip_id| find_clip(timeline, clip_id));
    let mut ranges = Vec::with_capacity(args.ranges.len());
    for (i, row) in args.ranges.iter().enumerate() {
        if row.len() < 2 {
            return Err(ToolError::new(format!(
                "ranges[{i}]: expected [start, end]"
            )));
        }
        let (mut start, mut end) = match units {
            RangeUnits::Frames => (row[0] as i32, row[1] as i32),
            RangeUnits::Seconds => {
                if let Some(clip) = clip {
                    (
                        source_seconds_to_timeline_frame_clamped(clip, row[0], timeline.fps),
                        source_seconds_to_timeline_frame_clamped(clip, row[1], timeline.fps),
                    )
                } else {
                    let fps = timeline.fps.max(1) as f64;
                    ((row[0] * fps).round() as i32, (row[1] * fps).round() as i32)
                }
            }
        };
        if let Some(clip) = clip {
            start = start.clamp(clip.start_frame, clip.end_frame());
            end = end.clamp(clip.start_frame, clip.end_frame());
        }
        ranges.push(FrameRange::new(start, end));
    }
    Ok(ranges)
}

/// Build a domain [`Transform`] from the optional partial `TransformArg`, leaving
/// unspecified fields at their identity defaults.
fn build_transform(arg: Option<args::TransformArg>) -> Transform {
    match arg {
        Some(t) => transform_from_arg(t),
        None => Transform::default(),
    }
}

fn transform_from_arg(t: args::TransformArg) -> Transform {
    let base = Transform::default();
    Transform {
        center_x: t.center_x.unwrap_or(base.center_x),
        center_y: t.center_y.unwrap_or(base.center_y),
        width: t.width.unwrap_or(base.width),
        height: t.height.unwrap_or(base.height),
        rotation: base.rotation,
        flip_horizontal: t.flip_horizontal.unwrap_or(base.flip_horizontal),
        flip_vertical: t.flip_vertical.unwrap_or(base.flip_vertical),
    }
}

fn merge_transform_arg(
    base: Transform,
    patch: args::TransformArg,
    media_canvas_aspect: Option<f64>,
) -> Transform {
    let aspect = media_canvas_aspect
        .filter(|a| a.is_finite() && *a > 0.0)
        .unwrap_or_else(|| current_transform_aspect(base).unwrap_or(1.0));
    let (width, height) = match (patch.width, patch.height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, w / aspect),
        (None, Some(h)) => (h * aspect, h),
        (None, None) => (base.width, base.height),
    };
    Transform {
        center_x: patch.center_x.unwrap_or(base.center_x),
        center_y: patch.center_y.unwrap_or(base.center_y),
        width,
        height,
        rotation: base.rotation,
        flip_horizontal: patch.flip_horizontal.unwrap_or(base.flip_horizontal),
        flip_vertical: patch.flip_vertical.unwrap_or(base.flip_vertical),
    }
}

fn current_transform_aspect(t: Transform) -> Option<f64> {
    if t.width.is_finite() && t.height.is_finite() && t.width > 0.0 && t.height > 0.0 {
        Some(t.width / t.height)
    } else {
        None
    }
}

fn find_clip<'a>(timeline: &'a Timeline, clip_id: &str) -> Option<&'a opentake_domain::Clip> {
    timeline
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .find(|clip| clip.id == clip_id)
}

fn media_canvas_aspect(
    timeline: &Timeline,
    manifest: &MediaManifest,
    clip: &opentake_domain::Clip,
) -> Option<f64> {
    let entry = manifest
        .entries
        .iter()
        .find(|entry| entry.id == clip.media_ref)?;
    let sw = entry.source_width?;
    let sh = entry.source_height?;
    if sw <= 0 || sh <= 0 || timeline.width <= 0 || timeline.height <= 0 {
        return None;
    }
    let source_aspect = sw as f64 / sh as f64;
    let canvas_aspect = timeline.width as f64 / timeline.height as f64;
    Some(source_aspect / canvas_aspect)
}

/// Build a [`TextStyle`] from `add_texts` scalar fields, leaving unspecified
/// fields at their defaults. Color accepts `#RGB`/`#RRGGBB`/`#RRGGBBAA`.
fn build_text_style(
    font_name: Option<String>,
    font_size: Option<f64>,
    color: Option<&str>,
    alignment: Option<&str>,
) -> TextStyle {
    let mut style = TextStyle::default();
    if let Some(n) = font_name {
        style.font_name = n;
    }
    if let Some(s) = font_size {
        style.font_size = s;
    }
    if let Some(c) = color.and_then(Rgba::from_hex) {
        style.color = c;
    }
    if let Some(a) = alignment.and_then(parse_alignment) {
        style.alignment = a;
    }
    style
}

fn parse_alignment(s: &str) -> Option<opentake_domain::TextAlignment> {
    match s.to_ascii_lowercase().as_str() {
        "left" => Some(opentake_domain::TextAlignment::Left),
        "center" => Some(opentake_domain::TextAlignment::Center),
        "right" => Some(opentake_domain::TextAlignment::Right),
        _ => None,
    }
}

/// An [`Rgb`] from a partial `RgbArg`, defaulting missing channels to `default`.
fn rgb_from_arg(arg: Option<RgbArg>, default: Rgb) -> Rgb {
    match arg {
        Some(a) => Rgb {
            r: a.r.unwrap_or(default.r),
            g: a.g.unwrap_or(default.g),
            b: a.b.unwrap_or(default.b),
        },
        None => default,
    }
}

/// Build a [`ColorGrade`] from the flat `set_color_grade` args, mapping the flat
/// lift/gamma/gain triples onto the domain's nested [`LiftGammaGain`].
fn color_grade_from_args(a: &SetColorGradeArgs) -> ColorGrade {
    let base = ColorGrade::default();
    ColorGrade {
        exposure: a.exposure.unwrap_or(base.exposure),
        temperature: a.temperature.unwrap_or(base.temperature),
        tint: a.tint.unwrap_or(base.tint),
        lift_gamma_gain: LiftGammaGain {
            lift: rgb_from_arg(a.lift, Rgb::zero()),
            gamma: rgb_from_arg(a.gamma, Rgb::default()),
            gain: rgb_from_arg(a.gain, Rgb::default()),
        },
        contrast: a.contrast.unwrap_or(base.contrast),
        saturation: a.saturation.unwrap_or(base.saturation),
    }
}

/// Build a [`ChromaKey`] from the `chroma_key` args. `keyColor` accepts a hex
/// string; absent fields keep the domain defaults.
fn chroma_key_from_args(a: &ChromaKeyArgs) -> ChromaKey {
    let base = ChromaKey::default();
    let key_color = a
        .key_color
        .as_deref()
        .and_then(rgb_from_hex)
        .unwrap_or(base.key_color);
    ChromaKey {
        key_color,
        similarity: a.similarity.unwrap_or(base.similarity),
        smoothness: a.smoothness.unwrap_or(base.smoothness),
        spill: a.spill.unwrap_or(base.spill),
    }
}

/// Parse a hex color into an [`Rgb`] (alpha dropped). Reuses [`Rgba::from_hex`].
fn rgb_from_hex(hex: &str) -> Option<Rgb> {
    Rgba::from_hex(hex).map(|c| Rgb::new(c.r, c.g, c.b))
}

fn point2(p: Option<args::Point2Arg>) -> Point2 {
    match p {
        Some(p) => Point2::new(p.x.unwrap_or(0.0), p.y.unwrap_or(0.0)),
        None => Point2::new(0.0, 0.0),
    }
}

/// Build a domain [`Mask`] from a decoded `MaskArg`, choosing the shape by its
/// `kind` discriminant. An unknown kind is a tool error with a precise path.
fn mask_from_arg(m: &MaskArg, path: &str) -> Result<Mask, ToolError> {
    let shape = match m.kind.to_ascii_lowercase().as_str() {
        "linear" => MaskShape::Linear {
            point: point2(m.point),
            normal: point2(m.normal),
        },
        "circle" => MaskShape::Circle {
            center: point2(m.center),
            radius: point2(m.radius),
        },
        "poly" => {
            let points = m
                .points
                .as_ref()
                .map(|ps| {
                    ps.iter()
                        .map(|p| Point2::new(p.x.unwrap_or(0.0), p.y.unwrap_or(0.0)))
                        .collect()
                })
                .unwrap_or_default();
            MaskShape::Poly { points }
        }
        other => {
            return Err(ToolError::new(format!(
                "{path}.kind: unknown mask kind '{other}'. Allowed: linear, circle, poly."
            )))
        }
    };
    Ok(Mask {
        shape,
        feather: m.feather.unwrap_or(0.0),
        invert: m.invert.unwrap_or(false),
    })
}

/// Build the typed [`KeyframeProperty`] + [`KeyframePayload`] from the raw
/// `set_keyframes` rows. Rows are `[frame, ...values, interp?]`; the value arity
/// is decided by the property (scalar / pair / crop). 1:1 with upstream's
/// per-property row decoding.
fn build_keyframe_payload(
    a: &SetKeyframesArgs,
) -> Result<(KeyframeProperty, KeyframePayload), ToolError> {
    let property = parse_keyframe_property(&a.property)?;
    let payload = match property {
        KeyframeProperty::Opacity | KeyframeProperty::Volume | KeyframeProperty::Rotation => {
            let mut kfs = Vec::with_capacity(a.keyframes.len());
            for (i, row) in a.keyframes.iter().enumerate() {
                let (frame, vals, interp) = parse_kf_row(row, &format!("keyframes[{i}]"))?;
                let value = *vals
                    .first()
                    .ok_or_else(|| ToolError::new(format!("keyframes[{i}]: missing value")))?;
                kfs.push(make_keyframe(frame, value, interp));
            }
            KeyframePayload::Scalar(KeyframeTrack::from_keyframes(kfs))
        }
        KeyframeProperty::Position | KeyframeProperty::Scale => {
            let mut kfs = Vec::with_capacity(a.keyframes.len());
            for (i, row) in a.keyframes.iter().enumerate() {
                let (frame, vals, interp) = parse_kf_row(row, &format!("keyframes[{i}]"))?;
                if vals.len() < 2 {
                    return Err(ToolError::new(format!(
                        "keyframes[{i}]: {} needs [frame, a, b]",
                        a.property
                    )));
                }
                kfs.push(make_keyframe(
                    frame,
                    AnimPair::new(vals[0], vals[1]),
                    interp,
                ));
            }
            KeyframePayload::Pair(KeyframeTrack::from_keyframes(kfs))
        }
        KeyframeProperty::Crop => {
            let mut kfs = Vec::with_capacity(a.keyframes.len());
            for (i, row) in a.keyframes.iter().enumerate() {
                let (frame, vals, interp) = parse_kf_row(row, &format!("keyframes[{i}]"))?;
                if vals.len() < 4 {
                    return Err(ToolError::new(format!(
                        "keyframes[{i}]: crop needs [frame, left, top, right, bottom]"
                    )));
                }
                let crop = Crop {
                    left: vals[0],
                    top: vals[1],
                    right: vals[2],
                    bottom: vals[3],
                };
                kfs.push(make_keyframe(frame, crop, interp));
            }
            KeyframePayload::Crop(KeyframeTrack::from_keyframes(kfs))
        }
    };
    Ok((property, payload))
}

fn make_keyframe<V>(frame: i32, value: V, interp: Option<Interpolation>) -> Keyframe<V> {
    match interp {
        Some(i) => Keyframe::with_interpolation(frame, value, i),
        None => Keyframe::new(frame, value),
    }
}

fn parse_keyframe_property(s: &str) -> Result<KeyframeProperty, ToolError> {
    match s.to_ascii_lowercase().as_str() {
        "opacity" => Ok(KeyframeProperty::Opacity),
        "volume" => Ok(KeyframeProperty::Volume),
        "rotation" => Ok(KeyframeProperty::Rotation),
        "position" => Ok(KeyframeProperty::Position),
        "scale" => Ok(KeyframeProperty::Scale),
        "crop" => Ok(KeyframeProperty::Crop),
        other => Err(ToolError::new(format!(
            "property: unknown '{other}'. Allowed: opacity, volume, rotation, position, scale, crop."
        ))),
    }
}

/// Parse one keyframe row `[frame, ...values, interp?]`. The optional trailing
/// string element is the interpolation; numeric elements after `frame` are the
/// values.
fn parse_kf_row(
    row: &Value,
    path: &str,
) -> Result<(i32, Vec<f64>, Option<Interpolation>), ToolError> {
    let Some(arr) = row.as_array() else {
        return Err(ToolError::new(format!("{path}: expected an array row")));
    };
    if arr.is_empty() {
        return Err(ToolError::new(format!("{path}: empty row")));
    }
    let frame = arr[0]
        .as_f64()
        .ok_or_else(|| ToolError::new(format!("{path}[0]: frame must be a number")))?
        .round() as i32;
    let mut values = Vec::new();
    let mut interp = None;
    for el in &arr[1..] {
        match el {
            Value::Number(n) => values.push(n.as_f64().unwrap_or(0.0)),
            Value::String(s) => interp = parse_interpolation(s),
            _ => {}
        }
    }
    Ok((frame, values, interp))
}

fn parse_interpolation(s: &str) -> Option<Interpolation> {
    match s.to_ascii_lowercase().as_str() {
        "linear" => Some(Interpolation::Linear),
        "hold" => Some(Interpolation::Hold),
        "smooth" => Some(Interpolation::Smooth),
        _ => None,
    }
}

/// Round every float in a JSON tree to 3 decimal places (mirrors the encoder's
/// `round3`), so `get_media` floats match the rest of the agent surface.
fn round_floats_3dp(value: Value) -> Value {
    match value {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() != 0.0 => {
                serde_json::Number::from_f64((f * 1000.0).round() / 1000.0)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
            _ => Value::Number(n),
        },
        Value::Array(arr) => Value::Array(arr.into_iter().map(round_floats_3dp).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, round_floats_3dp(v)))
                .collect(),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_core::AppCore;
    use opentake_domain::{ClipType, MediaManifestEntry, MediaSource, Track};
    use opentake_ops::command::EditResult;
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::mcp::core_handle::CoreHandle;
    use crate::mcp::media_bridge::{
        SearchIndexState, SearchMediaResult, SearchSpokenHit, SearchVisualHit,
    };

    /// A faithful [`CoreHandle`] over a real in-memory [`AppCore`], seeded with a
    /// video track and one media asset so `add_clips` can run end to end.
    struct TestHandle {
        core: AppCore,
    }

    impl TestHandle {
        fn new() -> Self {
            let core = AppCore::new();
            // Seed a video track via the editing entry point.
            core.apply(EditCommand::InsertTrack {
                kind: ClipType::Video,
                at: None,
            })
            .unwrap();
            TestHandle { core }
        }

        /// Register a media asset directly on the manifest by applying through the
        /// session is not exposed; instead we rely on `resolve_media_kind`'s
        /// fallback (video) for unknown refs, which is what an un-imported ref
        /// hits. For a known-asset path we inject via a manifest helper below.
        fn with_asset(self, id: &str) -> Self {
            // The public AppCore surface imports via probe; for a unit test we
            // only need the manifest to contain the id so resolution succeeds.
            // AppCore has no direct manifest setter, so we accept the video
            // fallback (add_clips on a video track works regardless).
            let _ = id;
            self
        }
    }

    impl CoreHandle for TestHandle {
        fn timeline(&self) -> Timeline {
            self.core.get_timeline().timeline
        }
        fn media(&self) -> MediaManifest {
            self.core.media()
        }
        fn apply(&self, cmd: EditCommand) -> anyhow::Result<EditResult> {
            self.core.apply(cmd).map_err(|e| anyhow::anyhow!("{e}"))
        }
        fn project_dir(&self) -> Option<PathBuf> {
            self.core.project_dir()
        }
    }

    fn dispatcher_with(handle: Arc<dyn CoreHandle>) -> Dispatcher {
        Dispatcher::new(handle, Arc::new(RwLock::new(PluginRegistry::new())))
    }

    #[test]
    fn unknown_tool_is_error() {
        let d = dispatcher_with(Arc::new(TestHandle::new()));
        let r = d.dispatch("not_a_tool", serde_json::json!({}));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("Unknown tool: not_a_tool"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn add_clips_then_get_timeline_reflects_clip() {
        let d = dispatcher_with(Arc::new(TestHandle::new().with_asset("asset-1")));
        // Track 0 is the seeded video track.
        let add = d.dispatch(
            "add_clips",
            serde_json::json!({
                "entries": [{
                    "mediaRef": "asset-1",
                    "trackIndex": 0,
                    "startFrame": 0,
                    "durationFrames": 30
                }]
            }),
        );
        assert!(!add.is_error, "{}", add.text_joined());

        let tl = d.dispatch("get_timeline", serde_json::json!({}));
        assert!(!tl.is_error, "{}", tl.text_joined());
        // The first block is the compact timeline JSON; later blocks carry the
        // context_signal. Parse the first text block only.
        let first = match &tl.content[0] {
            crate::tools::result::Block::Text { text } => text.clone(),
            _ => panic!("expected text block"),
        };
        let v: Value = serde_json::from_str(&first).unwrap();
        let clips = v["tracks"][0]["clips"].as_array().unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0]["durationFrames"], serde_json::json!(30));
    }

    #[test]
    fn precise_path_arg_error_mentions_field() {
        let d = dispatcher_with(Arc::new(TestHandle::new()));
        // add_clips entry missing the required startFrame.
        let r = d.dispatch(
            "add_clips",
            serde_json::json!({"entries": [{"mediaRef": "asset-1", "durationFrames": 30}]}),
        );
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("entries[0].startFrame"),
            "{}",
            r.text_joined()
        );
        assert!(
            r.text_joined().contains("startFrame"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn short_id_round_trip_shortens_outbound_id() {
        // A handle whose timeline carries a full-UUID clip id so the outbound
        // get_timeline shortens it to its 8-char floor prefix.
        struct UuidHandle {
            timeline: Timeline,
        }
        impl CoreHandle for UuidHandle {
            fn timeline(&self) -> Timeline {
                self.timeline.clone()
            }
            fn media(&self) -> MediaManifest {
                MediaManifest::new()
            }
            fn apply(&self, _cmd: EditCommand) -> anyhow::Result<EditResult> {
                anyhow::bail!("read-only test handle")
            }
            fn project_dir(&self) -> Option<PathBuf> {
                None
            }
        }
        const FULL: &str = "abcdef12-3456-7890-abcd-ef1234567890";
        let mut tl = Timeline::new();
        let mut t = Track::new("track-uuid-aaaa-bbbb-cccc", ClipType::Video);
        t.clips
            .push(opentake_domain::Clip::new(FULL, "media-x", 0, 30));
        tl.tracks.push(t);
        let d = dispatcher_with(Arc::new(UuidHandle { timeline: tl }));
        let r = d.dispatch("get_timeline", serde_json::json!({}));
        let text = r.text_joined();
        // The full id is replaced by its 8-char prefix; the full form is gone.
        assert!(text.contains(&FULL[..8]), "{text}");
        assert!(!text.contains(FULL), "full id should be shortened: {text}");
    }

    #[test]
    fn undo_with_empty_stack_errors() {
        let d = dispatcher_with(Arc::new(TestHandle::new()));
        let r = d.dispatch("undo", serde_json::json!({}));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("no agent edits to revert"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn stub_tool_reports_not_implemented() {
        let d = dispatcher_with(Arc::new(TestHandle::new()));
        let r = d.dispatch("generate_video", serde_json::json!({"prompt": "x"}));
        assert!(r.is_error);
        assert!(
            r.text_joined()
                .contains("generate_video: not yet implemented"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn get_media_returns_json_object() {
        let d = dispatcher_with(Arc::new(TestHandle::new()));
        let r = d.dispatch("get_media", serde_json::json!({}));
        assert!(!r.is_error, "{}", r.text_joined());
        let v: Value = serde_json::from_str(&r.text_joined()).unwrap();
        assert!(v.get("entries").is_some());
        assert!(v.get("folders").is_some());
    }

    #[test]
    fn list_models_returns_builtin_catalog() {
        let d = dispatcher_with(Arc::new(TestHandle::new()));
        let r = d.dispatch("list_models", serde_json::json!({}));
        assert!(!r.is_error, "{}", r.text_joined());
        let v: Value = serde_json::from_str(&r.text_joined()).unwrap();
        assert_eq!(v["loaded"], serde_json::json!(true));
        let models = v["models"].as_array().expect("models array");
        // The static catalog is non-empty and carries the upstream wire shape.
        assert!(!models.is_empty());
        assert!(models
            .iter()
            .all(|m| m.get("id").is_some() && m.get("uiCapabilities").is_some()));
    }

    #[test]
    fn list_models_filters_by_kind() {
        let d = dispatcher_with(Arc::new(TestHandle::new()));
        let r = d.dispatch("list_models", serde_json::json!({ "type": "image" }));
        assert!(!r.is_error, "{}", r.text_joined());
        let v: Value = serde_json::from_str(&r.text_joined()).unwrap();
        let models = v["models"].as_array().expect("models array");
        assert!(!models.is_empty(), "catalog must have image models");
        assert!(models
            .iter()
            .all(|m| m["kind"] == serde_json::json!("image")));
    }

    #[test]
    fn list_models_unknown_kind_errors() {
        let d = dispatcher_with(Arc::new(TestHandle::new()));
        let r = d.dispatch("list_models", serde_json::json!({ "type": "gif" }));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("type: unknown value 'gif'"),
            "{}",
            r.text_joined()
        );
    }

    // MARK: - Manifest-backed fixtures (rename / delete / workflow tools)

    use opentake_domain::Clip;
    use opentake_ops::{apply as ops_apply, EditorState, SeqIdGen};
    use std::sync::Mutex;

    /// A [`CoreHandle`] over a directly-built [`EditorState`], so tests can seed
    /// manifest entries/folders the public AppCore surface can't inject.
    struct StateHandle {
        state: Mutex<EditorState>,
    }

    impl StateHandle {
        fn new(timeline: Timeline, manifest: MediaManifest) -> Self {
            StateHandle {
                state: Mutex::new(EditorState::new(timeline, manifest)),
            }
        }
    }

    struct AnalysisHandle {
        timeline: Timeline,
        manifest: MediaManifest,
        pcm: opentake_media::PcmBuffer,
    }

    impl CoreHandle for AnalysisHandle {
        fn timeline(&self) -> Timeline {
            self.timeline.clone()
        }
        fn media(&self) -> MediaManifest {
            self.manifest.clone()
        }
        fn apply(&self, _cmd: EditCommand) -> anyhow::Result<EditResult> {
            anyhow::bail!("read-only analysis test handle")
        }
        fn project_dir(&self) -> Option<PathBuf> {
            None
        }
        fn extract_analysis_pcm(
            &self,
            _media_ref: &str,
            _spec: opentake_media::PcmSpec,
            _range: Option<(f64, f64)>,
        ) -> anyhow::Result<opentake_media::PcmBuffer> {
            Ok(self.pcm.clone())
        }
    }

    fn pcm(samples: Vec<f32>, sample_rate: u32) -> opentake_media::PcmBuffer {
        opentake_media::PcmBuffer {
            spec: opentake_media::PcmSpec {
                sample_rate,
                channels: 1,
                format: opentake_media::PcmFormat::F32,
            },
            samples_f32: samples,
        }
    }

    fn first_json(result: &ToolResult) -> Value {
        let first = match &result.content[0] {
            crate::tools::result::Block::Text { text } => text,
            _ => panic!("expected text block"),
        };
        serde_json::from_str(first).unwrap()
    }

    impl CoreHandle for StateHandle {
        fn timeline(&self) -> Timeline {
            self.state.lock().unwrap().timeline.clone()
        }
        fn media(&self) -> MediaManifest {
            self.state.lock().unwrap().manifest.clone()
        }
        fn apply(&self, cmd: EditCommand) -> anyhow::Result<EditResult> {
            let ids = SeqIdGen::new("t-");
            let mut st = self.state.lock().unwrap();
            ops_apply(&mut st, cmd, &ids).map_err(|e| anyhow::anyhow!("{e}"))
        }
        fn project_dir(&self) -> Option<PathBuf> {
            None
        }
    }

    fn entry(id: &str, name: &str) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.into(),
            name: name.into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: format!("/{id}.mp4"),
            },
            duration: 1.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(false),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        }
    }

    fn audio_entry(id: &str, name: &str) -> MediaManifestEntry {
        let mut e = entry(id, name);
        e.kind = ClipType::Audio;
        e.has_audio = Some(true);
        e.source = MediaSource::External {
            absolute_path: format!("/{id}.mp3"),
        };
        e
    }

    /// A video asset whose source carries an audio track (`hasAudio: true`) —
    /// the case `add_clips`/`insert_clips` should auto-create a linked audio
    /// partner for.
    fn video_with_audio_entry(id: &str, name: &str) -> MediaManifestEntry {
        let mut e = entry(id, name);
        e.has_audio = Some(true);
        e
    }

    fn entry_with_size(id: &str, name: &str, width: i32, height: i32) -> MediaManifestEntry {
        let mut e = entry(id, name);
        e.source_width = Some(width);
        e.source_height = Some(height);
        e
    }

    /// One video track with `clip-1` referencing `asset-1`, and `asset-1` in the
    /// manifest named "Old Name".
    fn seeded_handle() -> Arc<StateHandle> {
        let mut tl = Timeline::new();
        let mut t = Track::new("track-1", ClipType::Video);
        t.clips.push(Clip::new("clip-1", "asset-1", 0, 30));
        tl.tracks.push(t);
        let mut m = MediaManifest::new();
        m.entries.push(entry("asset-1", "Old Name"));
        Arc::new(StateHandle::new(tl, m))
    }

    fn seeded_transform_handle(
        transform: Transform,
        media_size: Option<(i32, i32)>,
    ) -> Arc<StateHandle> {
        let mut tl = Timeline::new();
        let mut t = Track::new("track-1", ClipType::Video);
        let mut clip = Clip::new("clip-1", "asset-1", 0, 30);
        clip.transform = transform;
        t.clips.push(clip);
        tl.tracks.push(t);
        let mut m = MediaManifest::new();
        m.entries.push(match media_size {
            Some((w, h)) => entry_with_size("asset-1", "Hero", w, h),
            None => entry("asset-1", "Hero"),
        });
        Arc::new(StateHandle::new(tl, m))
    }

    fn empty_manifest_handle(entries: Vec<MediaManifestEntry>) -> Arc<StateHandle> {
        let mut m = MediaManifest::new();
        m.entries = entries;
        Arc::new(StateHandle::new(Timeline::new(), m))
    }

    fn two_track_ripple_handle() -> Arc<StateHandle> {
        let mut tl = Timeline::new();
        tl.fps = 30;
        let mut first = Track::new("track-1", ClipType::Video);
        first.clips.push(Clip::new("clip-a", "asset-1", 0, 90));
        let mut second = Track::new("track-2", ClipType::Video);
        second.clips.push(Clip::new("clip-b", "asset-2", 100, 30));
        tl.tracks.push(first);
        tl.tracks.push(second);

        let mut m = MediaManifest::new();
        m.entries.push(entry("asset-1", "A"));
        m.entries.push(entry("asset-2", "B"));
        Arc::new(StateHandle::new(tl, m))
    }

    #[test]
    fn add_clips_omitted_track_index_creates_shared_video_track() {
        let h = empty_manifest_handle(vec![entry("asset-1", "A"), entry("asset-2", "B")]);
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "add_clips",
            serde_json::json!({
                "entries": [
                    {"mediaRef": "asset-1", "startFrame": 0, "durationFrames": 30},
                    {"mediaRef": "asset-2", "startFrame": 40, "durationFrames": 20}
                ]
            }),
        );

        assert!(!r.is_error, "{}", r.text_joined());
        let tl = h.timeline();
        assert_eq!(tl.tracks.len(), 1);
        assert_eq!(tl.tracks[0].kind, ClipType::Video);
        assert_eq!(tl.tracks[0].clips.len(), 2);
        assert_eq!(tl.tracks[0].clips[0].media_ref, "asset-1");
        assert_eq!(tl.tracks[0].clips[1].media_ref, "asset-2");
    }

    #[test]
    fn add_clips_omitted_track_index_creates_shared_audio_track() {
        let h = empty_manifest_handle(vec![
            audio_entry("asset-1", "A"),
            audio_entry("asset-2", "B"),
        ]);
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "add_clips",
            serde_json::json!({
                "entries": [
                    {"mediaRef": "asset-1", "startFrame": 0, "durationFrames": 30},
                    {"mediaRef": "asset-2", "startFrame": 40, "durationFrames": 20}
                ]
            }),
        );

        assert!(!r.is_error, "{}", r.text_joined());
        let tl = h.timeline();
        assert_eq!(tl.tracks.len(), 1);
        assert_eq!(tl.tracks[0].kind, ClipType::Audio);
        assert_eq!(tl.tracks[0].clips.len(), 2);
    }

    #[test]
    fn add_clips_omitted_track_index_is_one_undo_step() {
        let h = empty_manifest_handle(vec![entry("asset-1", "A"), entry("asset-2", "B")]);
        let d = dispatcher_with(h.clone());

        let add = d.dispatch(
            "add_clips",
            serde_json::json!({
                "entries": [
                    {"mediaRef": "asset-1", "startFrame": 0, "durationFrames": 30},
                    {"mediaRef": "asset-2", "startFrame": 40, "durationFrames": 20}
                ]
            }),
        );
        assert!(!add.is_error, "{}", add.text_joined());
        assert_eq!(h.timeline().tracks.len(), 1);

        let undo = d.dispatch("undo", serde_json::json!({}));
        assert!(!undo.is_error, "{}", undo.text_joined());
        assert!(h.timeline().tracks.is_empty());
    }

    #[test]
    fn add_clips_mixed_track_index_presence_is_rejected() {
        let h = empty_manifest_handle(vec![entry("asset-1", "A"), entry("asset-2", "B")]);
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "add_clips",
            serde_json::json!({
                "entries": [
                    {"mediaRef": "asset-1", "trackIndex": 0, "startFrame": 0, "durationFrames": 30},
                    {"mediaRef": "asset-2", "startFrame": 40, "durationFrames": 20}
                ]
            }),
        );

        assert!(r.is_error);
        assert!(
            r.text_joined().contains("trackIndex"),
            "{}",
            r.text_joined()
        );
        assert!(h.timeline().tracks.is_empty());
    }

    #[test]
    fn add_clips_omitted_track_index_invalid_entry_does_not_create_track() {
        let h = empty_manifest_handle(vec![entry("asset-1", "A")]);
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "add_clips",
            serde_json::json!({
                "entries": [
                    {"mediaRef": "asset-1", "startFrame": 0, "durationFrames": 0}
                ]
            }),
        );

        assert!(r.is_error);
        assert!(
            r.text_joined().contains("durationFrames"),
            "{}",
            r.text_joined()
        );
        assert!(h.timeline().tracks.is_empty());
    }

    // MARK: - #196 add_clips/insert_clips linked-audio fixtures.

    /// One empty video track plus the given manifest entries — for `add_clips`/
    /// `insert_clips` calls that pass an explicit `trackIndex`.
    fn one_video_track_handle(entries: Vec<MediaManifestEntry>) -> Arc<StateHandle> {
        let mut tl = Timeline::new();
        tl.tracks.push(Track::new("track-1", ClipType::Video));
        let mut m = MediaManifest::new();
        m.entries = entries;
        Arc::new(StateHandle::new(tl, m))
    }

    #[test]
    fn add_clips_explicit_track_index_links_audio_for_video_with_audio() {
        let h = one_video_track_handle(vec![video_with_audio_entry("asset-1", "A")]);
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "add_clips",
            serde_json::json!({
                "entries": [
                    {"mediaRef": "asset-1", "trackIndex": 0, "startFrame": 0, "durationFrames": 30}
                ]
            }),
        );

        assert!(!r.is_error, "{}", r.text_joined());
        let tl = h.timeline();
        assert_eq!(tl.tracks.len(), 2, "expected a fresh linked audio track");
        assert_eq!(tl.tracks[0].clips.len(), 1);
        assert_eq!(tl.tracks[1].kind, ClipType::Audio);
        assert_eq!(tl.tracks[1].clips.len(), 1);
        let video_clip = &tl.tracks[0].clips[0];
        let audio_clip = &tl.tracks[1].clips[0];
        assert!(video_clip.link_group_id.is_some());
        assert_eq!(video_clip.link_group_id, audio_clip.link_group_id);
        assert_eq!(audio_clip.media_type, ClipType::Audio);
    }

    #[test]
    fn add_clips_omitted_track_index_links_audio_for_video_with_audio() {
        let h = empty_manifest_handle(vec![video_with_audio_entry("asset-1", "A")]);
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "add_clips",
            serde_json::json!({
                "entries": [
                    {"mediaRef": "asset-1", "startFrame": 0, "durationFrames": 30}
                ]
            }),
        );

        assert!(!r.is_error, "{}", r.text_joined());
        let tl = h.timeline();
        // The shared visual track (auto-created) plus a fresh linked audio track.
        assert_eq!(tl.tracks.len(), 2, "expected a fresh linked audio track");
        assert_eq!(tl.tracks[0].kind, ClipType::Video);
        assert_eq!(tl.tracks[1].kind, ClipType::Audio);
        let video_clip = &tl.tracks[0].clips[0];
        let audio_clip = &tl.tracks[1].clips[0];
        assert!(video_clip.link_group_id.is_some());
        assert_eq!(video_clip.link_group_id, audio_clip.link_group_id);
    }

    #[test]
    fn add_clips_does_not_link_audio_when_source_has_no_audio() {
        let h = one_video_track_handle(vec![entry("asset-1", "A")]); // has_audio: false
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "add_clips",
            serde_json::json!({
                "entries": [
                    {"mediaRef": "asset-1", "trackIndex": 0, "startFrame": 0, "durationFrames": 30}
                ]
            }),
        );

        assert!(!r.is_error, "{}", r.text_joined());
        let tl = h.timeline();
        assert_eq!(
            tl.tracks.len(),
            1,
            "no linked audio track should be created"
        );
        assert!(tl.tracks[0].clips[0].link_group_id.is_none());
    }

    #[test]
    fn insert_clips_links_audio_for_video_with_audio() {
        let h = one_video_track_handle(vec![video_with_audio_entry("asset-1", "A")]);
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "insert_clips",
            serde_json::json!({
                "trackIndex": 0,
                "atFrame": 0,
                "entries": [
                    {"mediaRef": "asset-1", "durationFrames": 30}
                ]
            }),
        );

        assert!(!r.is_error, "{}", r.text_joined());
        let tl = h.timeline();
        assert_eq!(tl.tracks.len(), 2, "expected a fresh linked audio track");
        let video_clip = &tl.tracks[0].clips[0];
        let audio_clip = tl.tracks[1]
            .clips
            .iter()
            .find(|c| c.media_type == ClipType::Audio)
            .expect("linked audio clip");
        assert!(video_clip.link_group_id.is_some());
        assert_eq!(video_clip.link_group_id, audio_clip.link_group_id);
    }

    #[test]
    fn insert_clips_does_not_link_audio_when_source_has_no_audio() {
        let h = one_video_track_handle(vec![entry("asset-1", "A")]); // has_audio: false
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "insert_clips",
            serde_json::json!({
                "trackIndex": 0,
                "atFrame": 0,
                "entries": [
                    {"mediaRef": "asset-1", "durationFrames": 30}
                ]
            }),
        );

        assert!(!r.is_error, "{}", r.text_joined());
        let tl = h.timeline();
        assert_eq!(
            tl.tracks.len(),
            1,
            "no linked audio track should be created"
        );
        assert!(tl.tracks[0].clips[0].link_group_id.is_none());
    }

    #[test]
    fn ripple_delete_ranges_clip_id_seconds_uses_clip_track_and_timeline_fps() {
        let h = two_track_ripple_handle();
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "ripple_delete_ranges",
            serde_json::json!({
                "clipId": "clip-b",
                "units": "seconds",
                "ranges": [[0.2, 0.5]]
            }),
        );

        assert!(!r.is_error, "{}", r.text_joined());
        let tl = h.timeline();
        assert_eq!(tl.tracks[0].clips[0].duration_frames, 90);
        let spans: Vec<(i32, i32)> = tl.tracks[1]
            .clips
            .iter()
            .map(|clip| (clip.start_frame, clip.duration_frames))
            .collect();
        assert_eq!(spans, vec![(100, 6), (106, 15)]);
    }

    #[test]
    fn ripple_delete_ranges_clip_id_seconds_rounds_after_speed_mapping() {
        let mut tl = Timeline::new();
        tl.fps = 30;
        let mut track = Track::new("track-1", ClipType::Video);
        let mut clip = Clip::new("clip-b", "asset-2", 100, 30);
        clip.speed = 2.0;
        track.clips.push(clip);
        tl.tracks.push(track);
        let mut manifest = MediaManifest::new();
        manifest.entries.push(entry("asset-2", "B"));
        let h = Arc::new(StateHandle::new(tl, manifest));
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "ripple_delete_ranges",
            serde_json::json!({
                "clipId": "clip-b",
                "units": "seconds",
                "ranges": [[0.24, 0.50]]
            }),
        );

        assert!(!r.is_error, "{}", r.text_joined());
        let spans: Vec<(i32, i32)> = h.timeline().tracks[0]
            .clips
            .iter()
            .map(|clip| (clip.start_frame, clip.duration_frames))
            .collect();
        assert_eq!(spans, vec![(100, 4), (104, 22)]);
    }

    #[test]
    fn ripple_delete_ranges_frames_are_used_without_rounding() {
        let h = two_track_ripple_handle();
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "ripple_delete_ranges",
            serde_json::json!({
                "trackIndex": 1,
                "units": "frames",
                "ranges": [[105.9, 110.9]]
            }),
        );

        assert!(!r.is_error, "{}", r.text_joined());
        let tl = h.timeline();
        let spans: Vec<(i32, i32)> = tl.tracks[1]
            .clips
            .iter()
            .map(|clip| (clip.start_frame, clip.duration_frames))
            .collect();
        assert_eq!(spans, vec![(100, 5), (105, 20)]);
    }

    #[test]
    fn ripple_delete_ranges_rejects_track_index_with_seconds() {
        let h = two_track_ripple_handle();
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "ripple_delete_ranges",
            serde_json::json!({
                "trackIndex": 1,
                "units": "seconds",
                "ranges": [[3.5, 3.8]]
            }),
        );

        assert!(r.is_error);
        assert!(r.text_joined().contains("seconds"), "{}", r.text_joined());
        assert_eq!(h.timeline(), two_track_ripple_handle().timeline());
    }

    #[test]
    fn detect_beats_returns_pcm_frame_hints() {
        let mut manifest = MediaManifest::new();
        manifest.entries.push(audio_entry("music-1", "Music"));
        let mut samples = vec![0.0f32; 1_000];
        for sample in &mut samples[500..530] {
            *sample = 1.0;
        }
        let mut timeline = Timeline::new();
        timeline.fps = 10;
        let h = Arc::new(AnalysisHandle {
            timeline,
            manifest,
            pcm: pcm(samples, 1_000),
        });
        let d = dispatcher_with(h);

        let beats = d.dispatch(
            "detect_beats",
            serde_json::json!({"mediaRef": "music-1", "sensitivity": 1.0}),
        );
        assert!(!beats.is_error, "{}", beats.text_joined());
        let json = first_json(&beats);
        let frames: Vec<i64> = json["beats"]
            .as_array()
            .unwrap()
            .iter()
            .map(|beat| beat["frame"].as_i64().unwrap())
            .collect();
        assert!(
            frames.iter().any(|frame| (4..=5).contains(frame)),
            "{frames:?}"
        );
    }

    #[test]
    fn smart_reframe_reports_needs_vision_backend() {
        let d = dispatcher_with(empty_manifest_handle(vec![]));
        let reframe = d.dispatch(
            "smart_reframe",
            serde_json::json!({"clipIds": ["clip-a"], "aspectRatio": "9:16"}),
        );
        assert!(reframe.is_error);
        assert!(
            reframe
                .text_joined()
                .contains("needs vision analysis backend")
                || reframe.text_joined().contains("needs vision backend")
                || reframe.text_joined().contains("needs vision"),
            "{}",
            reframe.text_joined()
        );
    }

    #[test]
    fn tighten_silences_returns_ripple_delete_preview() {
        let mut timeline = Timeline::new();
        timeline.fps = 10;
        let mut track = Track::new("audio-track", ClipType::Audio);
        track.clips.push(Clip::new("clip-a", "asset-1", 0, 10));
        timeline.tracks.push(track);
        let mut manifest = MediaManifest::new();
        manifest.entries.push(audio_entry("asset-1", "Voice"));
        let mut samples = vec![0.5f32; 300];
        samples.extend(std::iter::repeat_n(0.0f32, 400));
        samples.extend(std::iter::repeat_n(0.5f32, 300));
        let h = Arc::new(AnalysisHandle {
            timeline,
            manifest,
            pcm: pcm(samples, 1_000),
        });
        let d = dispatcher_with(h);

        let result = d.dispatch(
            "tighten_silences",
            serde_json::json!({
                "clipIds": ["clip-a"],
                "thresholdDb": -40.0,
                "minSilenceFrames": 2,
                "paddingFrames": 0
            }),
        );

        assert!(!result.is_error, "{}", result.text_joined());
        let json = first_json(&result);
        let ranges = json["commands"][0]["args"]["ranges"].as_array().unwrap();
        assert!(!ranges.is_empty(), "{json}");
        let first = ranges[0].as_array().unwrap();
        let start = first[0].as_i64().unwrap();
        let end = first[1].as_i64().unwrap();
        assert!(start <= 3, "{json}");
        assert!(end >= 6, "{json}");
        assert_eq!(json["applied"], serde_json::json!(false));
    }

    #[test]
    fn analysis_tools_reject_unknown_args_before_unsupported_error() {
        let d = dispatcher_with(empty_manifest_handle(vec![]));
        let r = d.dispatch(
            "tighten_silences",
            serde_json::json!({"clipIds": ["clip-a"], "bogus": true}),
        );
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("unknown field"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn remove_filler_words_stays_disabled_until_transcript_is_wired() {
        let d = dispatcher_with(empty_manifest_handle(vec![]));
        let r = d.dispatch("remove_filler_words", serde_json::json!({}));
        assert!(r.is_error);
        assert!(
            r.text_joined()
                .contains("Unknown tool: remove_filler_words"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn rename_media_updates_manifest_name() {
        let h = seeded_handle();
        let d = dispatcher_with(h.clone());
        let r = d.dispatch(
            "rename_media",
            serde_json::json!({"mediaRef": "asset-1", "name": "Hero Shot"}),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        assert!(r.text_joined().contains("Hero Shot"), "{}", r.text_joined());
        assert_eq!(h.media().entries[0].name, "Hero Shot");
    }

    #[test]
    fn delete_media_cascades_referencing_clip() {
        let h = seeded_handle();
        let d = dispatcher_with(h.clone());
        let r = d.dispatch("delete_media", serde_json::json!({"assetIds": ["asset-1"]}));
        assert!(!r.is_error, "{}", r.text_joined());
        assert!(
            r.text_joined().contains("Deleted 1 asset"),
            "{}",
            r.text_joined()
        );
        assert!(h.media().entries.is_empty());
        // The only clip referenced the deleted asset → removed, track pruned.
        assert!(h.timeline().tracks.is_empty());
    }

    #[test]
    fn delete_media_unknown_id_errors() {
        let d = dispatcher_with(seeded_handle());
        let r = d.dispatch("delete_media", serde_json::json!({"assetIds": ["ghost"]}));
        assert!(r.is_error);
        assert!(r.text_joined().contains("not found"), "{}", r.text_joined());
    }

    #[test]
    fn set_clip_properties_partial_transform_width_preserves_media_aspect() {
        let h = seeded_transform_handle(Transform::default(), Some((3840, 2160)));
        let d = dispatcher_with(h.clone());
        let r = d.dispatch(
            "set_clip_properties",
            serde_json::json!({
                "clipIds": ["clip-1"],
                "transform": { "width": 0.5 }
            }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let c = &h.timeline().tracks[0].clips[0];
        assert!((c.transform.width - 0.5).abs() < 1e-9);
        assert!((c.transform.height - 0.5).abs() < 1e-9);
        assert!((c.transform.center_x - 0.5).abs() < 1e-9);
    }

    #[test]
    fn set_clip_properties_partial_transform_center_keeps_size() {
        let h = seeded_transform_handle(
            Transform {
                center_x: 0.3,
                center_y: 0.4,
                width: 0.25,
                height: 0.5,
                ..Transform::default()
            },
            Some((1080, 1920)),
        );
        let d = dispatcher_with(h.clone());
        let r = d.dispatch(
            "set_clip_properties",
            serde_json::json!({
                "clipIds": ["clip-1"],
                "transform": { "centerY": 0.6 }
            }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let c = &h.timeline().tracks[0].clips[0];
        assert!((c.transform.center_x - 0.3).abs() < 1e-9);
        assert!((c.transform.center_y - 0.6).abs() < 1e-9);
        assert!((c.transform.width - 0.25).abs() < 1e-9);
        assert!((c.transform.height - 0.5).abs() < 1e-9);
    }

    #[test]
    fn set_clip_properties_partial_transform_uses_current_aspect_without_media_size() {
        let h = seeded_transform_handle(
            Transform {
                width: 0.4,
                height: 0.2,
                ..Transform::default()
            },
            None,
        );
        let d = dispatcher_with(h.clone());
        let r = d.dispatch(
            "set_clip_properties",
            serde_json::json!({
                "clipIds": ["clip-1"],
                "transform": { "height": 0.1 }
            }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let c = &h.timeline().tracks[0].clips[0];
        assert!((c.transform.width - 0.2).abs() < 1e-9);
        assert!((c.transform.height - 0.1).abs() < 1e-9);
    }

    #[test]
    fn set_clip_properties_partial_transform_missing_clip_is_rejected_without_mutation() {
        let h = seeded_transform_handle(
            Transform {
                width: 0.4,
                height: 0.2,
                ..Transform::default()
            },
            None,
        );
        let d = dispatcher_with(h.clone());

        let r = d.dispatch(
            "set_clip_properties",
            serde_json::json!({
                "clipIds": ["clip-1", "ghost"],
                "transform": { "height": 0.1 }
            }),
        );

        assert!(r.is_error);
        assert!(
            r.text_joined().contains("clip not found"),
            "{}",
            r.text_joined()
        );
        let c = &h.timeline().tracks[0].clips[0];
        assert!((c.transform.width - 0.4).abs() < 1e-9);
        assert!((c.transform.height - 0.2).abs() < 1e-9);
    }

    // MARK: - Workflow plugin (Skills) tools

    fn manifest_json(id: &str, name: &str, desc: &str, vtype: &str) -> String {
        format!(
            r#"{{"schema_version":"1.0","id":"{id}","name":"{name}","description":"{desc}","video_type":{{"primary":"{vtype}"}},"workflow":{{"stages":[{{"id":"s0","name":"S0","order":0}}]}}}}"#
        )
    }

    fn dispatcher_with_plugins() -> Dispatcher {
        let mut reg = PluginRegistry::new();
        reg.register(
            PluginRegistry::load_from_strings(
                &manifest_json("audio-first", "Audio-First", "Lay audio first.", "vlog"),
                "# Audio-First\nLay your audio bed before cutting picture.",
                ".",
            )
            .unwrap(),
        );
        reg.register(
            PluginRegistry::load_from_strings(
                &manifest_json(
                    "talking-head",
                    "Talking Head",
                    "TH workflow",
                    "talking_head",
                ),
                "",
                ".",
            )
            .unwrap(),
        );
        Dispatcher::new(Arc::new(TestHandle::new()), Arc::new(RwLock::new(reg)))
    }

    /// The first text block (the tool's own JSON, before any context_signal).
    fn first_text(r: &ToolResult) -> String {
        match &r.content[0] {
            crate::tools::result::Block::Text { text } => text.clone(),
            _ => panic!("expected a text block"),
        }
    }

    #[test]
    fn list_workflows_reports_installed_and_active_flag() {
        let d = dispatcher_with_plugins();
        let r = d.dispatch("list_workflows", serde_json::json!({}));
        assert!(!r.is_error, "{}", r.text_joined());
        let v: Value = serde_json::from_str(&first_text(&r)).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let af = arr.iter().find(|p| p["id"] == "audio-first").unwrap();
        assert_eq!(af["name"], "Audio-First");
        assert_eq!(af["description"], "Lay audio first.");
        assert_eq!(af["videoType"], "vlog");
        assert_eq!(af["active"], serde_json::json!(false));
    }

    #[test]
    fn activate_workflow_returns_instructions_and_marks_active() {
        let d = dispatcher_with_plugins();
        let r = d.dispatch(
            "activate_workflow",
            serde_json::json!({"workflowId": "audio-first"}),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let t = r.text_joined();
        assert!(t.contains("Activated workflow 'Audio-First'"), "{t}");
        assert!(t.contains("Lay your audio bed"), "{t}");

        let l = d.dispatch("list_workflows", serde_json::json!({}));
        let v: Value = serde_json::from_str(&first_text(&l)).unwrap();
        let af = v
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "audio-first")
            .unwrap()
            .clone();
        assert_eq!(af["active"], serde_json::json!(true));
    }

    #[test]
    fn activate_unknown_workflow_errors() {
        let d = dispatcher_with_plugins();
        let r = d.dispatch(
            "activate_workflow",
            serde_json::json!({"workflowId": "ghost"}),
        );
        assert!(r.is_error);
    }

    #[test]
    fn deactivate_workflow_clears_active() {
        let d = dispatcher_with_plugins();
        d.dispatch(
            "activate_workflow",
            serde_json::json!({"workflowId": "audio-first"}),
        );
        let r = d.dispatch("deactivate_workflow", serde_json::json!({}));
        assert!(!r.is_error, "{}", r.text_joined());
        assert!(r.text_joined().contains("Deactivated"));
    }

    // MARK: - MediaBridge tools (inspect_timeline / import_media)

    use crate::mcp::media_bridge::{
        BridgeError, ImportOutcome, ImportSource, InspectResult, InspectedFrame, MediaBridge,
        TranscriptSource, TranscriptSourceResult,
    };
    use crate::tools::result::Block;
    use opentake_media::{TranscriptionResult, TranscriptionSegment, TranscriptionWord};

    /// One recorded `import_media` forward: a `kind:detail` tag plus the name /
    /// folder the dispatcher passed through.
    struct ImportCall {
        tag: String,
        name: Option<String>,
        folder_id: Option<String>,
    }

    /// A recording fake bridge: captures the last inspect/import call so tests can
    /// assert the dispatcher forwarded validated args, and returns canned output.
    #[derive(Default)]
    struct FakeBridge {
        inspect_calls: Mutex<Vec<(Vec<i32>, u32)>>,
        import_calls: Mutex<Vec<ImportCall>>,
        /// Canned transcripts keyed by media_ref (source-seconds timings).
        transcripts: Mutex<std::collections::HashMap<String, TranscriptionResult>>,
        /// media_refs the bridge should report as skipped `{reason}`.
        transcribe_errors: Mutex<std::collections::HashMap<String, String>>,
        /// When set, `transcribe_sources` returns this hard error (e.g. model
        /// not installed), mirroring the real bridge's backend-load failure.
        transcribe_hard_error: Mutex<Option<String>>,
        /// Records the media_refs passed to the last `transcribe_sources` call,
        /// so tests can assert dedup.
        transcribe_calls: Mutex<Vec<Vec<String>>>,
        /// Canned `search_media` result; when `None` the trait default (disabled)
        /// runs. Records the `(query, scope, limit, candidate ids)` of each call.
        search_result: Mutex<Option<SearchMediaResult>>,
        search_calls: Mutex<Vec<SearchCall>>,
    }

    /// One recorded `search_media` call: `(query, scope, limit, candidate ids)`.
    type SearchCall = (String, String, usize, Vec<String>);

    impl FakeBridge {
        fn with_transcript(self, media_ref: &str, t: TranscriptionResult) -> Self {
            self.transcripts
                .lock()
                .unwrap()
                .insert(media_ref.to_string(), t);
            self
        }
    }

    impl MediaBridge for FakeBridge {
        fn transcribe_sources(
            &self,
            sources: &[TranscriptSource],
        ) -> Result<Vec<TranscriptSourceResult>, BridgeError> {
            self.transcribe_calls
                .lock()
                .unwrap()
                .push(sources.iter().map(|s| s.media_ref.clone()).collect());
            if let Some(err) = self.transcribe_hard_error.lock().unwrap().clone() {
                return Err(BridgeError::new(err));
            }
            let transcripts = self.transcripts.lock().unwrap();
            let errors = self.transcribe_errors.lock().unwrap();
            Ok(sources
                .iter()
                .map(|s| {
                    if let Some(reason) = errors.get(&s.media_ref) {
                        TranscriptSourceResult {
                            media_ref: s.media_ref.clone(),
                            transcript: None,
                            error: Some(reason.clone()),
                        }
                    } else {
                        TranscriptSourceResult {
                            media_ref: s.media_ref.clone(),
                            transcript: transcripts.get(&s.media_ref).cloned(),
                            error: None,
                        }
                    }
                })
                .collect())
        }

        fn inspect_timeline(
            &self,
            frames: &[i32],
            max_longest_edge: u32,
        ) -> Result<InspectResult, BridgeError> {
            self.inspect_calls
                .lock()
                .unwrap()
                .push((frames.to_vec(), max_longest_edge));
            Ok(InspectResult {
                frames: frames
                    .iter()
                    .map(|&frame| InspectedFrame {
                        frame,
                        bytes: vec![0xff, 0xd8, 0xff, 0xe0], // JPEG SOI/APP0 stub
                        media_type: "image/jpeg".into(),
                    })
                    .collect(),
                width: 512,
                height: 288,
            })
        }

        fn import_media(
            &self,
            source: ImportSource,
            name: Option<String>,
            folder_id: Option<String>,
        ) -> Result<ImportOutcome, BridgeError> {
            let tag = match &source {
                ImportSource::Path(p) => format!("path:{p}"),
                ImportSource::Bytes { mime_type, .. } => format!("bytes:{mime_type}"),
                ImportSource::Url { url, .. } => format!("url:{url}"),
            };
            self.import_calls.lock().unwrap().push(ImportCall {
                tag: tag.clone(),
                name,
                folder_id,
            });
            Ok(ImportOutcome {
                message: format!("Imported via {tag}."),
            })
        }

        fn search_media(
            &self,
            candidates: &[SearchCandidate],
            query: &str,
            scope: &str,
            limit: usize,
        ) -> Result<SearchMediaResult, BridgeError> {
            self.search_calls.lock().unwrap().push((
                query.to_string(),
                scope.to_string(),
                limit,
                candidates.iter().map(|c| c.media_ref.clone()).collect(),
            ));
            if let Some(result) = self.search_result.lock().unwrap().clone() {
                return Ok(result);
            }
            Ok(SearchMediaResult {
                status: SearchIndexState::Disabled,
                indexable_assets: 0,
                indexed_assets: None,
                moments: Vec::new(),
                spoken: Vec::new(),
            })
        }
    }

    /// A dispatcher whose timeline has a single 60-frame clip and a `FakeBridge`
    /// wired in. Returns both so tests can inspect the recorded bridge calls.
    fn dispatcher_with_fake_bridge() -> (Dispatcher, Arc<FakeBridge>) {
        let mut tl = Timeline::new();
        tl.fps = 30;
        let mut track = opentake_domain::Track::new("track-1", ClipType::Video);
        track.clips.push(Clip::new("clip-1", "asset-1", 0, 60));
        tl.tracks.push(track);
        let mut m = MediaManifest::new();
        m.entries.push(entry("asset-1", "Hero"));
        let handle = Arc::new(StateHandle::new(tl, m));
        let bridge = Arc::new(FakeBridge::default());
        let d = Dispatcher::with_bridge(
            handle,
            Arc::new(RwLock::new(PluginRegistry::new())),
            Some(bridge.clone() as Arc<dyn MediaBridge>),
        );
        (d, bridge)
    }

    #[test]
    fn inspect_timeline_without_bridge_reports_unavailable() {
        // The seeded TestHandle timeline is empty, so first assert the empty guard,
        // then a non-empty timeline with no bridge reports "not available".
        let d = dispatcher_with(seeded_handle());
        let r = d.dispatch("inspect_timeline", serde_json::json!({ "startFrame": 0 }));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("not available"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn inspect_timeline_empty_timeline_errors() {
        let (d, _b) = {
            // A bridge is present but the timeline is empty → the empty guard fires
            // before the bridge is ever consulted.
            let handle = Arc::new(StateHandle::new(Timeline::new(), MediaManifest::new()));
            let bridge = Arc::new(FakeBridge::default());
            let d = Dispatcher::with_bridge(
                handle,
                Arc::new(RwLock::new(PluginRegistry::new())),
                Some(bridge.clone() as Arc<dyn MediaBridge>),
            );
            (d, bridge)
        };
        let r = d.dispatch("inspect_timeline", serde_json::json!({}));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("Timeline is empty"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn inspect_timeline_start_frame_out_of_range_errors() {
        let (d, _b) = dispatcher_with_fake_bridge();
        // total_frames is 60; startFrame 60 is out of range [0, 60).
        let r = d.dispatch("inspect_timeline", serde_json::json!({ "startFrame": 60 }));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("out of range"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn inspect_timeline_end_before_start_errors() {
        let (d, _b) = dispatcher_with_fake_bridge();
        let r = d.dispatch(
            "inspect_timeline",
            serde_json::json!({ "startFrame": 30, "endFrame": 20 }),
        );
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("greater than startFrame"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn inspect_timeline_single_frame_returns_one_image_and_meta() {
        let (d, bridge) = dispatcher_with_fake_bridge();
        let r = d.dispatch("inspect_timeline", serde_json::json!({ "startFrame": 5 }));
        assert!(!r.is_error, "{}", r.text_joined());

        // The bridge was asked for exactly [5] at the 512px cap.
        let calls = bridge.inspect_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], (vec![5], 512));

        // One image block, then a meta text block (upstream order).
        let images = r
            .content
            .iter()
            .filter(|b| matches!(b, Block::Image { .. }))
            .count();
        assert_eq!(images, 1, "one composited frame");
        // The last text block is the meta JSON with the sampled frame numbers.
        let meta_text = match &r.content[1] {
            Block::Text { text } => text.clone(),
            _ => panic!("expected meta text block after the image"),
        };
        let meta: Value = serde_json::from_str(&meta_text).unwrap();
        assert_eq!(meta["frameNumbers"], serde_json::json!([5]));
        assert_eq!(meta["totalFrames"], serde_json::json!(60));
        assert_eq!(meta["width"], serde_json::json!(512));
        assert_eq!(meta["fps"], serde_json::json!(30));
    }

    #[test]
    fn inspect_timeline_range_samples_frames_evenly_capped() {
        let (d, bridge) = dispatcher_with_fake_bridge();
        // [0, 60) with default 6 frames → floor(60*(i+0.5)/6): 5,15,25,35,45,55.
        let r = d.dispatch(
            "inspect_timeline",
            serde_json::json!({ "startFrame": 0, "endFrame": 60 }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let calls = bridge.inspect_calls.lock().unwrap();
        assert_eq!(calls[0].0, vec![5, 15, 25, 35, 45, 55]);
    }

    #[test]
    fn inspect_timeline_max_frames_is_capped_at_12() {
        let (d, bridge) = dispatcher_with_fake_bridge();
        // maxFrames 100 is clamped to 12 (and to the span, which is 60 here).
        let r = d.dispatch(
            "inspect_timeline",
            serde_json::json!({ "startFrame": 0, "endFrame": 60, "maxFrames": 100 }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        assert_eq!(bridge.inspect_calls.lock().unwrap()[0].0.len(), 12);
    }

    #[test]
    fn inspect_timeline_rejects_unknown_arg() {
        let (d, _b) = dispatcher_with_fake_bridge();
        let r = d.dispatch("inspect_timeline", serde_json::json!({ "bogus": 1 }));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("unknown field"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn import_media_without_bridge_reports_unavailable() {
        let d = dispatcher_with(seeded_handle());
        let r = d.dispatch(
            "import_media",
            serde_json::json!({ "source": { "path": "/x.mp4" } }),
        );
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("not available"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn import_media_requires_exactly_one_source() {
        let (d, _b) = dispatcher_with_fake_bridge();
        // Zero of url/path/bytes.
        let none = d.dispatch("import_media", serde_json::json!({ "source": {} }));
        assert!(none.is_error);
        assert!(
            none.text_joined().contains("exactly one"),
            "{}",
            none.text_joined()
        );
        // Two of them.
        let two = d.dispatch(
            "import_media",
            serde_json::json!({ "source": { "path": "/a.mp4", "url": "https://x/a.mp4" } }),
        );
        assert!(two.is_error);
        assert!(
            two.text_joined().contains("exactly one"),
            "{}",
            two.text_joined()
        );
    }

    #[test]
    fn import_media_bytes_requires_mime_type() {
        let (d, _b) = dispatcher_with_fake_bridge();
        let r = d.dispatch(
            "import_media",
            serde_json::json!({ "source": { "bytes": "AAAA" } }),
        );
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("mimeType is required"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn import_media_unknown_folder_id_errors() {
        let (d, _b) = dispatcher_with_fake_bridge();
        let r = d.dispatch(
            "import_media",
            serde_json::json!({ "source": { "path": "/a.mp4" }, "folderId": "ghost" }),
        );
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("folderId not found"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn import_media_rejects_unknown_nested_source_key() {
        let (d, _b) = dispatcher_with_fake_bridge();
        let r = d.dispatch(
            "import_media",
            serde_json::json!({ "source": { "url": "https://x/a.mp4", "bogus": 1 } }),
        );
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("source: unknown field"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn import_media_path_forwards_to_bridge_and_returns_message() {
        let (d, bridge) = dispatcher_with_fake_bridge();
        let r = d.dispatch(
            "import_media",
            serde_json::json!({ "source": { "path": "/clip.mp4" }, "name": "Clip" }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        assert!(
            r.text_joined().contains("Imported via path:/clip.mp4"),
            "{}",
            r.text_joined()
        );
        let calls = bridge.import_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tag, "path:/clip.mp4");
        assert_eq!(calls[0].name.as_deref(), Some("Clip"));
        assert_eq!(calls[0].folder_id, None);
    }

    #[test]
    fn import_media_bytes_forwards_mime_to_bridge() {
        let (d, bridge) = dispatcher_with_fake_bridge();
        let r = d.dispatch(
            "import_media",
            serde_json::json!({ "source": { "bytes": "AAAA", "mimeType": "image/png" } }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        assert_eq!(
            bridge.import_calls.lock().unwrap()[0].tag,
            "bytes:image/png"
        );
    }

    // MARK: - search_media (visual + spoken content search via the MediaBridge)

    fn image_entry(id: &str, name: &str) -> MediaManifestEntry {
        let mut e = entry(id, name);
        e.kind = ClipType::Image;
        e
    }

    /// A dispatcher over a manifest with a video (`v`), audio (`a`), and image
    /// (`i`) asset, plus a `FakeBridge` seeded with `result`. Returns both so
    /// tests can assert the recorded call + JSON shape.
    fn search_dispatcher(result: SearchMediaResult) -> (Dispatcher, Arc<FakeBridge>) {
        let tl = Timeline::new();
        let mut m = MediaManifest::new();
        m.entries.push(entry("v", "Harbor Sunset"));
        m.entries.push(audio_entry("a", "Interview"));
        m.entries.push(image_entry("i", "Poster"));
        let handle = Arc::new(StateHandle::new(tl, m));
        let bridge = Arc::new(FakeBridge::default());
        *bridge.search_result.lock().unwrap() = Some(result);
        let d = Dispatcher::with_bridge(
            handle,
            Arc::new(RwLock::new(PluginRegistry::new())),
            Some(bridge.clone() as Arc<dyn MediaBridge>),
        );
        (d, bridge)
    }

    fn sample_search_result() -> SearchMediaResult {
        SearchMediaResult {
            status: SearchIndexState::Ready,
            indexable_assets: 2,
            indexed_assets: Some(2),
            moments: vec![
                SearchVisualHit {
                    media_ref: "v".into(),
                    start_seconds: 3.0,
                    end_seconds: 6.0,
                    score: 0.82,
                    is_image: false,
                },
                SearchVisualHit {
                    media_ref: "i".into(),
                    start_seconds: 0.0,
                    end_seconds: 0.0,
                    score: 0.5,
                    is_image: true,
                },
            ],
            spoken: vec![SearchSpokenHit {
                media_ref: "a".into(),
                start_seconds: 12.0,
                end_seconds: 14.0,
                text: "the budget plan".into(),
            }],
        }
    }

    #[test]
    fn search_media_shapes_upstream_json_with_both_groups() {
        let (d, bridge) = search_dispatcher(sample_search_result());
        let r = d.dispatch(
            "search_media",
            serde_json::json!({ "query": "sunset harbor" }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let v: serde_json::Value = serde_json::from_str(&first_text(&r)).unwrap();

        // Visual group: status + counts + moments.
        assert_eq!(v["status"], "ready");
        assert_eq!(v["indexableAssets"], 2);
        assert_eq!(v["indexedAssets"], 2);
        let moments = v["moments"].as_array().unwrap();
        assert_eq!(moments.len(), 2);
        // Video hit carries a source-second range + name; no `type`.
        assert_eq!(moments[0]["mediaRef"], "v");
        assert_eq!(moments[0]["name"], "Harbor Sunset");
        assert_eq!(moments[0]["startSeconds"], 3.0);
        assert_eq!(moments[0]["endSeconds"], 6.0);
        assert!(moments[0].get("type").is_none());
        // Image hit is `type: image`, no range.
        assert_eq!(moments[1]["mediaRef"], "i");
        assert_eq!(moments[1]["type"], "image");
        assert!(moments[1].get("startSeconds").is_none());

        // Spoken group: mediaRef/name/range/text.
        let spoken = v["spoken"].as_array().unwrap();
        assert_eq!(spoken.len(), 1);
        assert_eq!(spoken[0]["mediaRef"], "a");
        assert_eq!(spoken[0]["name"], "Interview");
        assert_eq!(spoken[0]["text"], "the budget plan");

        // Default scope=both, limit=10 forwarded; all three ids are candidates.
        let calls = bridge.search_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "both");
        assert_eq!(calls[0].2, 10);
        assert_eq!(calls[0].3.len(), 3);
    }

    #[test]
    fn search_media_scope_visual_omits_spoken() {
        let (d, _bridge) = search_dispatcher(sample_search_result());
        let r = d.dispatch(
            "search_media",
            serde_json::json!({ "query": "harbor", "scope": "visual" }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let v: serde_json::Value = serde_json::from_str(&first_text(&r)).unwrap();
        assert!(v.get("moments").is_some());
        assert!(v.get("spoken").is_none()); // upstream: visual scope omits spoken
        assert!(v.get("status").is_some());
    }

    #[test]
    fn search_media_scope_spoken_omits_visual_status() {
        let (d, _bridge) = search_dispatcher(sample_search_result());
        let r = d.dispatch(
            "search_media",
            serde_json::json!({ "query": "budget", "scope": "spoken" }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let v: serde_json::Value = serde_json::from_str(&first_text(&r)).unwrap();
        assert!(v.get("spoken").is_some());
        // Spoken-only skips the visual group entirely (no status/moments).
        assert!(v.get("status").is_none());
        assert!(v.get("moments").is_none());
    }

    #[test]
    fn search_media_limit_is_clamped_1_to_50() {
        let (d, bridge) = search_dispatcher(sample_search_result());
        // Over-max clamps to 50.
        let _ = d.dispatch(
            "search_media",
            serde_json::json!({ "query": "x", "limit": 999 }),
        );
        // Under-min clamps to 1.
        let _ = d.dispatch(
            "search_media",
            serde_json::json!({ "query": "x", "limit": 0 }),
        );
        let calls = bridge.search_calls.lock().unwrap();
        assert_eq!(calls[0].2, 50);
        assert_eq!(calls[1].2, 1);
    }

    #[test]
    fn search_media_media_ref_restricts_candidates() {
        let (d, bridge) = search_dispatcher(sample_search_result());
        let r = d.dispatch(
            "search_media",
            serde_json::json!({ "query": "x", "mediaRef": "v" }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let calls = bridge.search_calls.lock().unwrap();
        // Only the one restricted asset is a candidate.
        assert_eq!(calls[0].3, vec!["v".to_string()]);
    }

    #[test]
    fn search_media_unknown_media_ref_errors() {
        let (d, _bridge) = search_dispatcher(sample_search_result());
        let r = d.dispatch(
            "search_media",
            serde_json::json!({ "query": "x", "mediaRef": "nope" }),
        );
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("media not found"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn search_media_empty_query_errors() {
        let (d, _bridge) = search_dispatcher(sample_search_result());
        let r = d.dispatch("search_media", serde_json::json!({ "query": "   " }));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("query is empty"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn search_media_invalid_scope_errors() {
        let (d, _bridge) = search_dispatcher(sample_search_result());
        let r = d.dispatch(
            "search_media",
            serde_json::json!({ "query": "x", "scope": "sideways" }),
        );
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("scope must be"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn search_media_without_bridge_reports_unavailable() {
        let mut m = MediaManifest::new();
        m.entries.push(entry("v", "Clip"));
        let handle = Arc::new(StateHandle::new(Timeline::new(), m));
        let d = Dispatcher::new(handle, Arc::new(RwLock::new(PluginRegistry::new())));
        let r = d.dispatch("search_media", serde_json::json!({ "query": "x" }));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("not available in this build"),
            "{}",
            r.text_joined()
        );
    }

    // MARK: - get_transcript (timeline transcript via the MediaBridge)

    fn word(text: &str, start: f64, end: f64) -> TranscriptionWord {
        TranscriptionWord {
            text: text.into(),
            start: Some(start),
            end: Some(end),
        }
    }

    fn transcript(words: Vec<TranscriptionWord>) -> TranscriptionResult {
        TranscriptionResult {
            text: String::new(),
            language: Some("en".into()),
            words,
            segments: vec![],
        }
    }

    /// A dispatcher whose timeline has one audio clip (media `aud`, at frame 0,
    /// duration 60, identity) on an audio track, plus a `FakeBridge` seeded with
    /// `aud`'s transcript. Returns both. `has_audio` audio entry makes the clip
    /// caption-eligible.
    fn transcript_dispatcher(t: TranscriptionResult) -> (Dispatcher, Arc<FakeBridge>) {
        let mut tl = Timeline::new();
        tl.fps = 30;
        let mut track = opentake_domain::Track::new("track-a", ClipType::Audio);
        let mut clip = Clip::new("clip-a", "aud", 0, 60);
        clip.media_type = ClipType::Audio;
        track.clips.push(clip);
        tl.tracks.push(track);
        let mut m = MediaManifest::new();
        m.entries.push(audio_entry("aud", "Voice"));
        let handle = Arc::new(StateHandle::new(tl, m));
        let bridge = Arc::new(FakeBridge::default().with_transcript("aud", t));
        let d = Dispatcher::with_bridge(
            handle,
            Arc::new(RwLock::new(PluginRegistry::new())),
            Some(bridge.clone() as Arc<dyn MediaBridge>),
        );
        (d, bridge)
    }

    #[test]
    fn get_transcript_maps_words_to_project_frames() {
        let (d, _b) = transcript_dispatcher(transcript(vec![
            word("hello", 0.0, 0.5),
            word("world", 0.5, 1.0),
        ]));
        let r = d.dispatch("get_transcript", serde_json::json!({}));
        assert!(!r.is_error, "{}", r.text_joined());
        let v = first_json(&r);
        assert_eq!(v["fps"], 30);
        assert_eq!(v["timing"], "projectFrames");
        assert_eq!(v["wordFormat"], serde_json::json!(["text", "start", "end"]));
        let clips = v["clips"].as_array().unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0]["clipId"], "clip-a");
        assert_eq!(clips[0]["trackIndex"], 0);
        assert_eq!(clips[0]["startFrame"], 0);
        assert_eq!(clips[0]["endFrame"], 60);
        // hello 0..0.5s → 0..15, world 0.5..1.0s → 15..30 (30 fps, identity clip).
        assert_eq!(
            clips[0]["words"],
            serde_json::json!([["hello", 0, 15], ["world", 15, 30]])
        );
    }

    #[test]
    fn get_transcript_without_bridge_reports_unavailable() {
        // Same audio timeline but no bridge wired → honest "not available".
        let mut tl = Timeline::new();
        tl.fps = 30;
        let mut track = opentake_domain::Track::new("track-a", ClipType::Audio);
        let mut clip = Clip::new("clip-a", "aud", 0, 60);
        clip.media_type = ClipType::Audio;
        track.clips.push(clip);
        tl.tracks.push(track);
        let mut m = MediaManifest::new();
        m.entries.push(audio_entry("aud", "Voice"));
        let d = dispatcher_with(Arc::new(StateHandle::new(tl, m)));
        let r = d.dispatch("get_transcript", serde_json::json!({}));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("not available"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn get_transcript_empty_timeline_returns_empty_clips_not_error() {
        let d = dispatcher_with_fake_bridge(); // video-only, has_audio=false
        let (d, _b) = d;
        let r = d.dispatch("get_transcript", serde_json::json!({}));
        assert!(!r.is_error, "{}", r.text_joined());
        let v = first_json(&r);
        assert_eq!(v["clips"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn get_transcript_clip_filter_unknown_errors() {
        let (d, _b) = transcript_dispatcher(transcript(vec![word("hi", 0.0, 0.5)]));
        let r = d.dispatch("get_transcript", serde_json::json!({ "clipId": "ghost" }));
        assert!(r.is_error);
        assert!(r.text_joined().contains("not found"), "{}", r.text_joined());
    }

    #[test]
    fn get_transcript_clip_filter_scopes_to_one_clip() {
        let (d, _b) = transcript_dispatcher(transcript(vec![word("hi", 0.0, 0.5)]));
        let r = d.dispatch("get_transcript", serde_json::json!({ "clipId": "clip-a" }));
        assert!(!r.is_error, "{}", r.text_joined());
        let v = first_json(&r);
        assert_eq!(v["clips"].as_array().unwrap()[0]["clipId"], "clip-a");
    }

    #[test]
    fn get_transcript_window_paging_filters_words() {
        // words at 0..0.5s→0..15, 1..1.5s→30..45, 2..2.5s→60..75.
        let (d, _b) = transcript_dispatcher(transcript(vec![
            word("a", 0.0, 0.5),
            word("b", 1.0, 1.5),
            word("c", 2.0, 2.5),
        ]));
        // Need a long-enough clip for word c to be visible; extend the clip.
        // (The default clip is 60 frames = 2.0s at 30fps, so c's midpoint 2.25s
        // would be out; use a window that keeps b only.)
        let r = d.dispatch(
            "get_transcript",
            serde_json::json!({ "startFrame": 30, "endFrame": 60 }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let v = first_json(&r);
        let words = v["clips"].as_array().unwrap()[0]["words"]
            .as_array()
            .unwrap();
        assert_eq!(words.len(), 1);
        assert_eq!(words[0][0], "b");
    }

    #[test]
    fn get_transcript_window_start_ge_end_errors() {
        let (d, _b) = transcript_dispatcher(transcript(vec![word("a", 0.0, 0.5)]));
        let r = d.dispatch(
            "get_transcript",
            serde_json::json!({ "startFrame": 50, "endFrame": 20 }),
        );
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("must be less than"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn get_transcript_skipped_source_reported_not_fatal() {
        let (d, bridge) = transcript_dispatcher(transcript(vec![word("a", 0.0, 0.5)]));
        // Force the source to be skipped with a reason.
        bridge
            .transcribe_errors
            .lock()
            .unwrap()
            .insert("aud".into(), "decode failed".into());
        let r = d.dispatch("get_transcript", serde_json::json!({}));
        assert!(!r.is_error, "{}", r.text_joined());
        let v = first_json(&r);
        assert_eq!(v["clips"].as_array().unwrap().len(), 0);
        let skipped = v["skipped"].as_array().unwrap();
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0]["file"], "Voice"); // asset display name
        assert_eq!(skipped[0]["reason"], "decode failed");
    }

    #[test]
    fn get_transcript_hard_error_surfaces_as_tool_error() {
        let (d, bridge) = transcript_dispatcher(transcript(vec![word("a", 0.0, 0.5)]));
        *bridge.transcribe_hard_error.lock().unwrap() =
            Some("transcription model not installed".into());
        let r = d.dispatch("get_transcript", serde_json::json!({}));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("model not installed"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn get_transcript_rejects_unknown_arg() {
        let (d, _b) = transcript_dispatcher(transcript(vec![word("a", 0.0, 0.5)]));
        let r = d.dispatch("get_transcript", serde_json::json!({ "bogus": 1 }));
        assert!(r.is_error);
    }

    // MARK: - caption target selection (pure)

    #[test]
    fn caption_targets_include_audio_and_video_with_audio() {
        let mut tl = Timeline::new();
        let mut vt = opentake_domain::Track::new("v", ClipType::Video);
        vt.clips.push(Clip::new("v-with-audio", "vid_a", 0, 60));
        vt.clips.push(Clip::new("v-silent", "vid_silent", 60, 60));
        tl.tracks.push(vt);
        let mut at = opentake_domain::Track::new("a", ClipType::Audio);
        let mut ac = Clip::new("a1", "aud", 0, 60);
        ac.media_type = ClipType::Audio;
        at.clips.push(ac);
        tl.tracks.push(at);

        let mut m = MediaManifest::new();
        let mut v_with = entry("vid_a", "V");
        v_with.has_audio = Some(true);
        m.entries.push(v_with);
        m.entries.push(entry("vid_silent", "Silent")); // has_audio=false
        m.entries.push(audio_entry("aud", "A"));

        let frags = caption_target_fragments(&tl, &m, None);
        let ids: Vec<&str> = frags.iter().map(|f| f.clip.id.as_str()).collect();
        assert!(ids.contains(&"v-with-audio"));
        assert!(ids.contains(&"a1"));
        assert!(!ids.contains(&"v-silent")); // no audio track → not eligible
    }

    #[test]
    fn caption_targets_drop_video_when_linked_audio_present() {
        // A video clip and an audio clip share a link group → the video is
        // dropped (its audio partner is transcribed instead).
        let mut tl = Timeline::new();
        let mut vt = opentake_domain::Track::new("v", ClipType::Video);
        let mut vc = Clip::new("v1", "vid_a", 0, 60);
        vc.link_group_id = Some("grp".into());
        vt.clips.push(vc);
        tl.tracks.push(vt);
        let mut at = opentake_domain::Track::new("a", ClipType::Audio);
        let mut ac = Clip::new("a1", "aud", 0, 60);
        ac.media_type = ClipType::Audio;
        ac.link_group_id = Some("grp".into());
        at.clips.push(ac);
        tl.tracks.push(at);

        let mut m = MediaManifest::new();
        let mut v_with = entry("vid_a", "V");
        v_with.has_audio = Some(true);
        m.entries.push(v_with);
        m.entries.push(audio_entry("aud", "A"));

        let frags = caption_target_fragments(&tl, &m, None);
        let ids: Vec<&str> = frags.iter().map(|f| f.clip.id.as_str()).collect();
        assert!(!ids.contains(&"v1"), "linked video should be dropped");
        assert!(ids.contains(&"a1"));
    }

    #[test]
    fn unique_sources_dedup_by_media_ref() {
        // Two clips referencing the same audio asset dedup to one source.
        let mut tl = Timeline::new();
        let mut at = opentake_domain::Track::new("a", ClipType::Audio);
        for (i, start) in [(0, 0), (1, 60)] {
            let mut c = Clip::new(format!("a{i}"), "aud", start, 60);
            c.media_type = ClipType::Audio;
            at.clips.push(c);
        }
        tl.tracks.push(at);
        let mut m = MediaManifest::new();
        m.entries.push(audio_entry("aud", "A"));
        let frags = caption_target_fragments(&tl, &m, None);
        assert_eq!(frags.len(), 2);
        let sources = unique_transcript_sources(&frags);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].media_ref, "aud");
        assert!(!sources[0].is_video);
    }

    // MARK: - add_captions (transcribe + place, via the MediaBridge)

    fn segment(text: &str, start: f64, end: f64) -> TranscriptionSegment {
        TranscriptionSegment {
            text: text.into(),
            start,
            end,
        }
    }

    /// A caption transcript: words drive dominant-track selection; segments drive
    /// the caption-line packing (`caption_specs` iterates segments).
    fn caption_transcript(
        words: Vec<TranscriptionWord>,
        segments: Vec<TranscriptionSegment>,
    ) -> TranscriptionResult {
        TranscriptionResult {
            text: String::new(),
            language: Some("en".into()),
            words,
            segments,
        }
    }

    /// Dispatcher with one audio clip (media `aud`, frame 0, dur 300 @ 30fps) on
    /// an audio track and a `FakeBridge` seeded with `aud`'s caption transcript.
    fn caption_dispatcher(t: TranscriptionResult) -> (Dispatcher, Arc<FakeBridge>) {
        let mut tl = Timeline::new();
        tl.fps = 30;
        tl.width = 1920;
        tl.height = 1080;
        let mut track = opentake_domain::Track::new("track-a", ClipType::Audio);
        let mut clip = Clip::new("clip-a", "aud", 0, 300);
        clip.media_type = ClipType::Audio;
        track.clips.push(clip);
        tl.tracks.push(track);
        let mut m = MediaManifest::new();
        m.entries.push(audio_entry("aud", "Voice"));
        let handle = Arc::new(StateHandle::new(tl, m));
        let bridge = Arc::new(FakeBridge::default().with_transcript("aud", t));
        let d = Dispatcher::with_bridge(
            handle,
            Arc::new(RwLock::new(PluginRegistry::new())),
            Some(bridge.clone() as Arc<dyn MediaBridge>),
        );
        (d, bridge)
    }

    #[test]
    fn add_captions_places_caption_track_and_reports_count() {
        let (d, _b) = caption_dispatcher(caption_transcript(
            vec![word("hello", 0.0, 0.5), word("world", 0.5, 1.0)],
            vec![segment("Hello world.", 0.0, 1.0)],
        ));
        let r = d.dispatch("add_captions", serde_json::json!({}));
        assert!(!r.is_error, "{}", r.text_joined());
        assert!(r.text_joined().contains("caption"), "{}", r.text_joined());
        // A fresh video track was inserted at index 0 holding the caption clip.
        let tl = d.handle.timeline();
        assert_eq!(tl.tracks[0].kind, ClipType::Video);
        assert_eq!(tl.tracks[0].clips.len(), 1);
        let cap = &tl.tracks[0].clips[0];
        assert_eq!(cap.media_type, ClipType::Text);
        assert!(cap.caption_group_id.is_some());
        assert_eq!(cap.text_content.as_deref(), Some("Hello world."));
        // Placement near the bottom (default center Y 0.9).
        assert!((cap.transform.center_y - 0.9).abs() < 1e-9);
    }

    #[test]
    fn add_captions_applies_text_case_and_style() {
        let (d, _b) = caption_dispatcher(caption_transcript(
            vec![word("hi", 0.0, 0.5)],
            vec![segment("hi there", 0.0, 1.0)],
        ));
        let r = d.dispatch(
            "add_captions",
            serde_json::json!({ "textCase": "upper", "fontSize": 72, "color": "#FF0000" }),
        );
        assert!(!r.is_error, "{}", r.text_joined());
        let tl = d.handle.timeline();
        let cap = &tl.tracks[0].clips[0];
        assert_eq!(cap.text_content.as_deref(), Some("HI THERE"));
        let style = cap.text_style.as_ref().unwrap();
        assert_eq!(style.font_size, 72.0);
        assert!((style.color.r - 1.0).abs() < 1e-9 && style.color.g < 1e-9);
    }

    #[test]
    fn add_captions_is_one_undo_step() {
        let (d, _b) = caption_dispatcher(caption_transcript(
            vec![word("a", 0.0, 0.5)],
            vec![segment("A.", 0.0, 1.0)],
        ));
        assert!(!d.dispatch("add_captions", serde_json::json!({})).is_error);
        // The dispatcher tracks agent edits; one undo removes the whole track.
        let before = d.handle.timeline().tracks.len();
        let u = d.dispatch("undo", serde_json::json!({}));
        assert!(!u.is_error, "{}", u.text_joined());
        assert_eq!(d.handle.timeline().tracks.len(), before - 1);
    }

    #[test]
    fn add_captions_no_speech_detected_errors() {
        // Transcript with no segments → no caption lines → "No speech detected".
        let (d, _b) = caption_dispatcher(caption_transcript(vec![], vec![]));
        let r = d.dispatch("add_captions", serde_json::json!({}));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("No speech detected"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn add_captions_unsupported_language_errors() {
        let (d, _b) = caption_dispatcher(caption_transcript(
            vec![word("a", 0.0, 0.5)],
            vec![segment("A.", 0.0, 1.0)],
        ));
        let r = d.dispatch("add_captions", serde_json::json!({ "language": "klingon" }));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("does not support"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn add_captions_invalid_color_errors() {
        let (d, _b) = caption_dispatcher(caption_transcript(
            vec![word("a", 0.0, 0.5)],
            vec![segment("A.", 0.0, 1.0)],
        ));
        let r = d.dispatch("add_captions", serde_json::json!({ "color": "notacolor" }));
        assert!(r.is_error);
        assert!(r.text_joined().contains("color"), "{}", r.text_joined());
    }

    #[test]
    fn add_captions_no_audio_clips_errors() {
        // Video-only timeline with has_audio=false → nothing to caption.
        let (d, _b) = dispatcher_with_fake_bridge();
        let r = d.dispatch("add_captions", serde_json::json!({}));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("no audio/video"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn add_captions_without_bridge_reports_unavailable() {
        let mut tl = Timeline::new();
        tl.fps = 30;
        tl.width = 1920;
        tl.height = 1080;
        let mut track = opentake_domain::Track::new("track-a", ClipType::Audio);
        let mut clip = Clip::new("clip-a", "aud", 0, 300);
        clip.media_type = ClipType::Audio;
        track.clips.push(clip);
        tl.tracks.push(track);
        let mut m = MediaManifest::new();
        m.entries.push(audio_entry("aud", "Voice"));
        let d = dispatcher_with(Arc::new(StateHandle::new(tl, m)));
        let r = d.dispatch("add_captions", serde_json::json!({}));
        assert!(r.is_error);
        assert!(
            r.text_joined().contains("not available"),
            "{}",
            r.text_joined()
        );
    }

    #[test]
    fn add_captions_rejects_unknown_arg() {
        let (d, _b) = caption_dispatcher(caption_transcript(
            vec![word("a", 0.0, 0.5)],
            vec![segment("A.", 0.0, 1.0)],
        ));
        let r = d.dispatch("add_captions", serde_json::json!({ "bogus": 1 }));
        assert!(r.is_error);
    }
}
