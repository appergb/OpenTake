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

use std::sync::{Arc, Mutex, RwLock};

use opentake_domain::{AnimPair, Crop, Interpolation, Keyframe, KeyframeTrack};
use opentake_domain::{
    ChromaKey, ColorGrade, Effect, LiftGammaGain, Mask, MaskShape, MediaManifest, Point2, Rgb,
    Rgba, TextStyle, Timeline, Transform, VideoType,
};
use opentake_ops::{
    ClipEntry, ClipMove, ClipProperties, EditCommand, FrameRange, KeyframePayload,
    KeyframeProperty, RenameEntry, TextEntry,
};
use serde_json::Value;

use crate::mcp::core_handle::CoreHandle;
use crate::mcp::gen_catalog;
use crate::plugin::registry::PluginRegistry;
use crate::signal::engine;
use crate::signal::rules::OpContext;
use crate::tools::args::{self, *};
use crate::tools::encode_timeline::encode_timeline;
use crate::tools::errors::{decode_tool_args, ToolError};
use crate::tools::names::ToolName;
use crate::tools::result::ToolResult;
use crate::tools::short_id;

/// The in-process tool dispatcher. Holds the [`CoreHandle`] boundary, the plugin
/// registry (read-locked for the active plugin), and a per-dispatcher agent-undo
/// stack so `undo` only reverts edits this session made.
pub struct Dispatcher {
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    /// Action names of agent edits applied through this dispatcher, newest last.
    /// Guards `undo`: we only revert when this session has pushed an edit.
    agent_undo: Mutex<Vec<String>>,
}

impl Dispatcher {
    /// New dispatcher over a core handle + plugin registry.
    pub fn new(handle: Arc<dyn CoreHandle>, registry: Arc<RwLock<PluginRegistry>>) -> Self {
        Dispatcher {
            handle,
            registry,
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
            ToolName::RippleDeleteRanges => self.ripple_delete_ranges(args, op),
            ToolName::AddTexts => self.add_texts(args),
            ToolName::CreateFolder => self.create_folder(args),
            ToolName::MoveToFolder => self.move_to_folder(args),
            ToolName::SetClipProperties => self.set_clip_properties(args),
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

            // --- Not yet implementable in this phase (honest stubs) ---
            // Media reads (inspect/transcript/search) + import need the media
            // backend via a widened CoreHandle; generation/upscale need the async
            // GenClient + BYOK auth; inspect_timeline needs the render+text path;
            // motion graphics is #34. Each is wired in a later batch.
            ToolName::InspectMedia
            | ToolName::GetTranscript
            | ToolName::InspectTimeline
            | ToolName::SearchMedia
            | ToolName::GenerateVideo
            | ToolName::GenerateImage
            | ToolName::GenerateAudio
            | ToolName::UpscaleMedia
            | ToolName::ImportMedia
            | ToolName::AddCaptions
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
        for (i, raw) in a.entries.iter().enumerate() {
            let e: AddClipEntry = decode_tool_args(raw, &format!("entries[{i}]"))?;
            let (media_type, has_audio) = resolve_media_kind(manifest, &e.media_ref);
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
                add_linked_audio: false,
            });
        }
        op.added_media_refs = media_refs;
        op.track_index = entries.first().map(|e| e.track_index);
        let res = self.apply(EditCommand::AddClips { entries })?;
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
                add_linked_audio: false,
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

    fn ripple_delete_ranges(
        &self,
        args: &Value,
        op: &mut OpContext,
    ) -> Result<ToolResult, ToolError> {
        let a: RippleDeleteRangesArgs = decode_tool_args(args, "")?;
        let track_index = a.track_index.unwrap_or(0);
        op.track_index = Some(track_index);
        let ranges: Vec<FrameRange> = a
            .ranges
            .iter()
            .map(|r| {
                let start = r.first().copied().unwrap_or(0.0).round() as i32;
                let end = r.get(1).copied().unwrap_or(0.0).round() as i32;
                FrameRange::new(start, end)
            })
            .collect();
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

    fn set_clip_properties(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let a: SetClipPropertiesArgs = decode_tool_args(args, "")?;
        let clip_ids = a.clip_ids.clone();
        let properties = ClipProperties {
            duration_frames: a.duration_frames,
            trim_start_frame: a.trim_start_frame,
            trim_end_frame: a.trim_end_frame,
            speed: a.speed,
            volume: a.volume,
            opacity: a.opacity,
            transform: a.transform.map(transform_from_arg),
            text_content: a.content.clone(),
        };
        let res = self.apply(EditCommand::SetClipProperties {
            clip_ids,
            properties,
        })?;
        Ok(ToolResult::ok(res.summary))
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
}
