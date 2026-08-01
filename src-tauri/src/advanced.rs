//! Production desktop implementations for capability-gated advanced workflows.

use std::collections::VecDeque;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use opentake_agent::mcp::advanced::{
    AdvancedWorkflowBridge, AdvancedWorkflowCommit, AdvancedWorkflowError,
    AdvancedWorkflowErrorKind, AdvancedWorkflowRequest,
};
use opentake_agent::tools::args::{GenerateMatteArgs, RemoveObjectArgs, TrackMotionArgs};
use opentake_agent::tools::names::ToolName;
use opentake_core::{AppCore, MotionPlacement, ProbedMedia, ProjectRevision};
use opentake_domain::{
    AnimPair, GenerationInput, GenerationJobStatus, Interpolation, Keyframe, KeyframeTrack, Mask,
};
use opentake_media::analysis::{
    track_region_motion, verify_rvm_model, NormalizedMotionRegion, RegionMotionTrack,
    RvmMattingSession,
};
use opentake_media::decode::spawn_video_stream;
use opentake_media::{
    decode_frames_at_cancellable, extract_pcm_cancellable, file_sha256, probe, ExportPreset,
    ExportResolution, FrameRequest, MediaCancelToken, MediaError, PcmFormat, PcmSpec, RgbaFrame,
    StreamVideoFrame, VideoCodec, VideoEncoder, VideoStream, VideoStreamRequest,
};
use opentake_ops::{EditCommand, KeyframePayload, KeyframeProperty};
use same_file::Handle;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};

use crate::media::MediaState;

pub struct TauriAdvancedWorkflowBridge {
    core: AppCore,
    cache_root: PathBuf,
    models_dir: PathBuf,
}

pub struct AdvancedWorkflowCommandState {
    bridge: Arc<TauriAdvancedWorkflowBridge>,
    active: Mutex<Option<MediaCancelToken>>,
}

impl AdvancedWorkflowCommandState {
    pub fn new(bridge: Arc<TauriAdvancedWorkflowBridge>) -> Self {
        Self {
            bridge,
            active: Mutex::new(None),
        }
    }

    fn begin(&self) -> Result<MediaCancelToken, String> {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.is_some() {
            return Err("advanced_workflow_busy".to_string());
        }
        let token = MediaCancelToken::new();
        *active = Some(token.clone());
        Ok(token)
    }

    fn finish(&self, token: &MediaCancelToken) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active
            .as_ref()
            .is_some_and(|current| current.same_instance(token))
        {
            *active = None;
        }
    }

    pub fn cancel_active(&self) -> bool {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.as_ref().is_some_and(|token| {
            token.cancel();
            true
        })
    }
}

#[derive(Default)]
pub struct MattingModelInstallState {
    active: Mutex<Option<MediaCancelToken>>,
}

impl MattingModelInstallState {
    fn begin(&self) -> Result<MediaCancelToken, String> {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.is_some() {
            return Err("matting_model_download_busy".to_string());
        }
        let token = MediaCancelToken::new();
        *active = Some(token.clone());
        Ok(token)
    }

    fn finish(&self, token: &MediaCancelToken) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active
            .as_ref()
            .is_some_and(|current| current.same_instance(token))
        {
            *active = None;
        }
    }

    fn cancel(&self) -> bool {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.as_ref().is_some_and(|token| {
            token.cancel();
            true
        })
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MattingModelStatusDto {
    pub installed: bool,
    pub model: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct MattingProgressDto {
    fraction: f64,
    downloaded_bytes: u64,
    total_bytes: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GenerateMatteResultDto {
    pub result: serde_json::Value,
    pub action_name: Option<String>,
}

#[tauri::command]
pub fn matting_model_status(media: State<'_, MediaState>) -> MattingModelStatusDto {
    let installed = verify_rvm_model(media.engine().models_dir()).is_ok();
    MattingModelStatusDto {
        installed,
        model: opentake_media::analysis::RVM_MODEL_ID.to_string(),
        bytes: opentake_media::analysis::RVM_MODEL_BYTES,
        sha256: opentake_media::analysis::RVM_MODEL_SHA256.to_string(),
    }
}

#[tauri::command]
pub async fn download_matting_model(
    app: AppHandle,
    media: State<'_, MediaState>,
    state: State<'_, MattingModelInstallState>,
) -> Result<MattingModelStatusDto, String> {
    let token = state.begin()?;
    let models_dir = media.engine().models_dir().to_path_buf();
    let progress_app = app.clone();
    let progress = Arc::new(move |downloaded_bytes: u64, total_bytes: u64| {
        let fraction = if total_bytes == 0 {
            0.0
        } else {
            downloaded_bytes as f64 / total_bytes as f64
        };
        let _ = progress_app.emit(
            "matting://progress",
            MattingProgressDto {
                fraction: fraction.clamp(0.0, 1.0),
                downloaded_bytes,
                total_bytes,
            },
        );
    });
    let result = opentake_media::analysis::download_rvm_model(&models_dir, &token, Some(progress))
        .await
        .map_err(|error| error.to_string());
    state.finish(&token);
    result?;
    Ok(matting_model_status(media))
}

#[tauri::command]
pub fn cancel_matting_model_download(state: State<'_, MattingModelInstallState>) -> bool {
    state.cancel()
}

#[tauri::command]
pub async fn advanced_generate_matte(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: GenerateMatteArgs,
) -> Result<GenerateMatteResultDto, String> {
    let token = state.begin()?;
    let bridge = Arc::clone(&state.bridge);
    let worker_token = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(
            AdvancedWorkflowRequest::GenerateMatte(request),
            &worker_token,
        )
    })
    .await
    .map_err(|error| format!("advanced workflow worker failed: {error}"))
    .and_then(|result| result.map_err(|error| error.message));
    state.finish(&token);
    let commit = result?;
    Ok(GenerateMatteResultDto {
        result: commit.result,
        action_name: commit.action_name,
    })
}

#[tauri::command]
pub async fn advanced_remove_object(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: RemoveObjectArgs,
) -> Result<GenerateMatteResultDto, String> {
    let token = state.begin()?;
    let bridge = Arc::clone(&state.bridge);
    let worker_token = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(
            AdvancedWorkflowRequest::RemoveObject(request),
            &worker_token,
        )
    })
    .await
    .map_err(|error| format!("advanced workflow worker failed: {error}"))
    .and_then(|result| result.map_err(|error| error.message));
    state.finish(&token);
    let commit = result?;
    Ok(GenerateMatteResultDto {
        result: commit.result,
        action_name: commit.action_name,
    })
}

#[tauri::command]
pub fn cancel_advanced_workflow(state: State<'_, AdvancedWorkflowCommandState>) -> bool {
    state.cancel_active()
}

impl TauriAdvancedWorkflowBridge {
    pub fn new(core: AppCore, cache_root: PathBuf, models_dir: PathBuf) -> Self {
        Self {
            core,
            cache_root,
            models_dir,
        }
    }

    fn track_motion(
        &self,
        args: TrackMotionArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        let snapshot = self.core.runtime_snapshot();
        let clip = snapshot
            .timeline
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == args.clip_id)
            .cloned()
            .ok_or_else(|| advanced_resource(format!("clip not found: {}", args.clip_id)))?;
        if clip.media_type != opentake_domain::ClipType::Video || clip.nested_sequence_id.is_some()
        {
            return Err(advanced_invalid(
                "motion tracking requires an ordinary video clip",
            ));
        }
        let region: opentake_agent::tools::args::MotionRegionArg =
            serde_json::from_value(args.region)
                .map_err(|error| advanced_invalid(error.to_string()))?;
        let region = NormalizedMotionRegion {
            x: region.x,
            y: region.y,
            width: region.width,
            height: region.height,
        };
        let clip_end = clip.start_frame + clip.duration_frames;
        let start = args.start_frame.unwrap_or(clip.start_frame);
        let end = args.end_frame.unwrap_or(clip_end);
        if start < clip.start_frame || end > clip_end || end - start < 2 {
            return Err(advanced_invalid(
                "startFrame/endFrame must be a non-empty range inside the clip",
            ));
        }
        let first_relative = start - clip.start_frame;
        let last_relative = end - clip.start_frame - 1;
        let sample_count = (last_relative - first_relative + 1).clamp(2, 48) as usize;
        let relative_frames = (0..sample_count)
            .map(|index| {
                (first_relative as f64
                    + index as f64 * (last_relative - first_relative) as f64
                        / (sample_count - 1) as f64)
                    .round() as i32
            })
            .collect::<Vec<_>>();
        let fps = snapshot.timeline.fps.max(1) as f64;
        let source_start = clip.trim_start_frame as f64 / fps;
        let times = relative_frames
            .iter()
            .map(|frame| source_start + *frame as f64 * clip.speed.max(0.0001) / fps)
            .collect::<Vec<_>>();
        let (path, is_video) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &clip.media_ref)
                .map_err(advanced_resource)?;
        if !is_video {
            return Err(advanced_invalid("motion tracking source is not a video"));
        }
        let request = FrameRequest {
            max_size: (640, 360),
            tolerance_secs: 0.05,
            ..FrameRequest::default()
        };
        let decoded = decode_frames_at_cancellable(&path, &times, &request, cancel);
        let mut frames = decoded
            .into_iter()
            .filter_map(Result::ok)
            .map(|(actual, frame)| {
                let relative =
                    ((actual - source_start) * fps / clip.speed.max(0.0001)).round() as i32;
                (relative.clamp(first_relative, last_relative), frame)
            })
            .collect::<Vec<_>>();
        if cancel.is_cancelled() {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::Cancelled,
                "motion tracking cancelled",
            ));
        }
        frames.sort_by_key(|(frame, _)| *frame);
        frames.dedup_by_key(|(frame, _)| *frame);
        let tracked = track_region_motion(&frames, region, cancel)
            .map_err(|error| advanced_execution(error.to_string()))?;
        if tracked.minimum_confidence < 0.25 {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::AnalysisLowConfidence,
                format!(
                    "motion tracking confidence {:.3} is below the 0.25 threshold",
                    tracked.minimum_confidence
                ),
            ));
        }
        let keyframes = position_keyframes(&clip, &tracked);
        let apply_requested = args.apply.unwrap_or(false);
        let (applied, action_name) = if apply_requested {
            let result = self
                .core
                .apply_at_revision(
                    ProjectRevision {
                        project_epoch: snapshot.project_epoch,
                        version: snapshot.version,
                    },
                    EditCommand::SetKeyframes {
                        clip_id: clip.id.clone(),
                        property: KeyframeProperty::Position,
                        payload: KeyframePayload::Pair(KeyframeTrack::from_keyframes(
                            keyframes.clone(),
                        )),
                    },
                )
                .map_err(|error| advanced_execution(error.to_string()))?;
            (result.changed, result.changed.then_some(result.action_name))
        } else {
            (false, None)
        };
        let response_keyframes = keyframes
            .iter()
            .map(|keyframe| {
                json!({
                    "frame": keyframe.frame,
                    "position": {"x": keyframe.value.a, "y": keyframe.value.b},
                    "interpolation": "linear"
                })
            })
            .collect::<Vec<_>>();
        Ok(AdvancedWorkflowCommit {
            result: json!({
                "clipId": clip.id,
                "applied": applied,
                "algorithm": "opentake.region-block-match",
                "algorithmVersion": 1,
                "minimumConfidence": tracked.minimum_confidence,
                "region": {"x": region.x, "y": region.y, "width": region.width, "height": region.height},
                "keyframes": response_keyframes
            }),
            action_name,
        })
    }

    fn generate_matte(
        &self,
        args: GenerateMatteArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("advanced video workflow cancelled"));
        }
        let snapshot = self.core.runtime_snapshot();
        let clip = snapshot
            .timeline
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == args.clip_id)
            .cloned()
            .ok_or_else(|| advanced_resource(format!("clip not found: {}", args.clip_id)))?;
        if clip.media_type != opentake_domain::ClipType::Video
            || clip.nested_sequence_id.is_some()
            || clip.reversed
            || (clip.speed - 1.0).abs() > f64::EPSILON
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                "matting currently requires an ordinary forward 1x video clip",
            ));
        }
        let installed = verify_rvm_model(&self.models_dir).map_err(|error| {
            AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                error.to_string(),
            )
        })?;
        if args
            .model
            .as_deref()
            .is_some_and(|model| model != installed.id)
        {
            return Err(advanced_invalid(format!(
                "unsupported matting model; installed model is {}",
                installed.id
            )));
        }
        let clip_end = clip.start_frame + clip.duration_frames;
        let start = args.start_frame.unwrap_or(clip.start_frame);
        let end = args.end_frame.unwrap_or(clip_end);
        if start < clip.start_frame || end > clip_end || start >= end {
            return Err(advanced_invalid(
                "startFrame/endFrame must be a non-empty range inside the clip",
            ));
        }
        let (source_path, is_video) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &clip.media_ref)
                .map_err(advanced_resource)?;
        if !is_video {
            return Err(advanced_invalid("matting source is not a video"));
        }
        let source_sha256 =
            file_sha256(&source_path).map_err(|error| advanced_execution(error.to_string()))?;
        let key_seed = format!(
            "{}|{}|{}|{}|{}|{}|{}",
            source_sha256,
            installed.sha256,
            clip.trim_start_frame,
            clip.duration_frames,
            snapshot.timeline.fps,
            start,
            end
        );
        let cache_key = format!("{:x}", Sha256::digest(key_seed.as_bytes()));
        let cache_dir = self.cache_root.join("matting");
        std::fs::create_dir_all(&cache_dir)
            .map_err(|error| advanced_execution(error.to_string()))?;
        let cache_path = cache_dir.join(format!("{cache_key}.mov"));
        if !cache_path.exists() {
            materialize_matte_video(
                &source_path,
                &cache_path,
                &clip,
                snapshot.timeline.fps,
                start,
                end,
                &self.models_dir,
                cancel,
            )?;
        }
        let mut cached_file = open_verified_regular_file(&cache_path)?;
        let output_probe = opentake_media::probe::probe_file(&cached_file)
            .map_err(|error| advanced_execution(error.to_string()))?;
        let expected_duration = clip.duration_frames as f64 / snapshot.timeline.fps.max(1) as f64;
        if !output_probe.has_video
            || output_probe.width.is_none()
            || output_probe.height.is_none()
            || output_probe
                .fps
                .is_none_or(|fps| (fps - snapshot.timeline.fps as f64).abs() > 0.01)
            || (output_probe.duration_secs - expected_duration).abs()
                > 1.5 / snapshot.timeline.fps.max(1) as f64
        {
            return Err(advanced_execution("matting output probe failed"));
        }
        let apply_requested = args.apply.unwrap_or(false);
        let mut asset_id = None;
        let mut action_name = None;
        let applied = if apply_requested {
            let project_dir = snapshot.project_dir.clone().ok_or_else(|| {
                advanced_invalid("save the project before applying a generated matte")
            })?;
            let project_media = crate::library::ProjectMediaCapability::open_verified(
                &self.core,
                snapshot.project_epoch,
                &project_dir,
                true,
            )
            .map_err(advanced_execution)?;
            let leaf_name = format!("matte-{}.mov", uuid::Uuid::new_v4());
            let mut published = project_media
                .create_import(Path::new(&leaf_name))
                .map_err(advanced_execution)?;
            cached_file
                .seek(SeekFrom::Start(0))
                .map_err(|error| advanced_execution(error.to_string()))?;
            std::io::copy(&mut cached_file, published.file_mut())
                .and_then(|_| published.file_mut().flush())
                .and_then(|_| published.file().sync_all())
                .map_err(|error| advanced_execution(error.to_string()))?;
            if !project_media
                .matches_leaf(&published)
                .map_err(advanced_execution)?
            {
                return Err(advanced_execution(
                    "matting output identity changed before project commit",
                ));
            }
            let provenance = GenerationInput {
                prompt: json!({"kind":"aiMatte","startFrame":start,"endFrame":end}).to_string(),
                model: installed.id.clone(),
                duration: clip.duration_frames,
                aspect_ratio: format!(
                    "{}:{}",
                    output_probe.width.unwrap_or(0),
                    output_probe.height.unwrap_or(0)
                ),
                provider: Some("opentake-matting".into()),
                status: Some(GenerationJobStatus::Ready),
                source_asset_id: Some(clip.media_ref.clone()),
                source_clip_id: Some(clip.id.clone()),
                source_start_frame: Some(start),
                source_end_frame: Some(end),
                ..GenerationInput::default()
            };
            let committed = self.core.commit_motion_media_for_project(
                snapshot.project_epoch,
                snapshot.version,
                &project_dir,
                published.path(),
                "AI Matte",
                &ProbedMedia {
                    duration_secs: output_probe.duration_secs,
                    width: output_probe
                        .width
                        .and_then(|value| i32::try_from(value).ok()),
                    height: output_probe
                        .height
                        .and_then(|value| i32::try_from(value).ok()),
                    fps: output_probe.fps,
                    has_audio: output_probe.has_audio,
                    color: output_probe.color,
                },
                provenance,
                MotionPlacement::Replace {
                    clip_id: clip.id.clone(),
                },
            );
            let committed = match committed {
                Ok(committed) => committed,
                Err(error) => return Err(advanced_execution(error.to_string())),
            };
            published.commit();
            asset_id = Some(committed.media.id);
            action_name = Some(committed.edit.action_name);
            true
        } else {
            false
        };
        Ok(AdvancedWorkflowCommit {
            result: json!({
                "clipId": clip.id,
                "sourceMediaRef": clip.media_ref,
                "assetId": asset_id,
                "applied": applied,
                "cacheKey": cache_key,
                "previewPath": cache_path,
                "frameCount": clip.duration_frames,
                "width": output_probe.width,
                "height": output_probe.height,
                "fps": output_probe.fps,
                "model": installed.id,
                "modelSha256": installed.sha256,
                "sourceSha256": source_sha256,
                "startFrame": start,
                "endFrame": end
            }),
            action_name,
        })
    }

    fn remove_object(
        &self,
        args: RemoveObjectArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("object removal cancelled"));
        }
        const PROVIDER: &str = "opentake-local";
        const MODEL: &str = "opentake-boundary-fill-v1";
        if args
            .provider
            .as_deref()
            .is_some_and(|provider| provider != PROVIDER && provider != "local")
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                "this build supports only the on-device opentake-local object-removal provider",
            ));
        }
        if args.model.as_deref().is_some_and(|model| model != MODEL) {
            return Err(advanced_invalid(format!(
                "unsupported object-removal model; available model is {MODEL}"
            )));
        }

        let snapshot = self.core.runtime_snapshot();
        let clip = snapshot
            .timeline
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == args.clip_id)
            .cloned()
            .ok_or_else(|| advanced_resource(format!("clip not found: {}", args.clip_id)))?;
        if clip.media_type != opentake_domain::ClipType::Video
            || clip.nested_sequence_id.is_some()
            || clip.reversed
            || (clip.speed - 1.0).abs() > f64::EPSILON
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                "object removal currently requires an ordinary forward 1x video clip",
            ));
        }
        let mask_index = parse_mask_index(&args.mask_id)?;
        let mask = clip.masks.get(mask_index).cloned().ok_or_else(|| {
            advanced_resource(format!(
                "mask not found: {} (clip has {} masks)",
                args.mask_id,
                clip.masks.len()
            ))
        })?;
        let clip_end = clip.start_frame + clip.duration_frames;
        let start = args.start_frame.unwrap_or(clip.start_frame);
        let end = args.end_frame.unwrap_or(clip_end);
        if start < clip.start_frame || end > clip_end || start >= end {
            return Err(advanced_invalid(
                "startFrame/endFrame must be a non-empty range inside the clip",
            ));
        }
        let (source_path, is_video) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &clip.media_ref)
                .map_err(advanced_resource)?;
        if !is_video {
            return Err(advanced_invalid("object-removal source is not a video"));
        }
        let source_sha256 =
            file_sha256(&source_path).map_err(|error| advanced_execution(error.to_string()))?;
        let mask_json =
            serde_json::to_string(&mask).map_err(|error| advanced_execution(error.to_string()))?;
        let key_seed = format!(
            "{source_sha256}|{MODEL}|{}|{}|{}|{start}|{end}|{mask_json}",
            clip.trim_start_frame, clip.duration_frames, snapshot.timeline.fps
        );
        let cache_key = format!("{:x}", Sha256::digest(key_seed.as_bytes()));
        let cache_dir = self.cache_root.join("object-removal");
        std::fs::create_dir_all(&cache_dir)
            .map_err(|error| advanced_execution(error.to_string()))?;
        let cache_path = cache_dir.join(format!("{cache_key}.mov"));
        if !cache_path.exists() {
            materialize_object_removal_video(
                &source_path,
                &cache_path,
                &clip,
                snapshot.timeline.fps,
                start,
                end,
                &mask,
                cancel,
            )?;
        }
        if cancel.checkpoint() {
            return Err(cancelled_workflow("object removal cancelled"));
        }
        let mut cached_file = open_verified_regular_file(&cache_path)?;
        let output_probe = opentake_media::probe::probe_file(&cached_file)
            .map_err(|error| advanced_execution(error.to_string()))?;
        let expected_duration = clip.duration_frames as f64 / snapshot.timeline.fps.max(1) as f64;
        if !output_probe.has_video
            || output_probe.width.is_none()
            || output_probe.height.is_none()
            || output_probe
                .fps
                .is_none_or(|fps| (fps - snapshot.timeline.fps as f64).abs() > 0.01)
            || (output_probe.duration_secs - expected_duration).abs()
                > 1.5 / snapshot.timeline.fps.max(1) as f64
        {
            return Err(advanced_execution("object-removal output probe failed"));
        }

        let mut asset_id = None;
        let mut action_name = None;
        let applied = if args.apply.unwrap_or(false) {
            let project_dir = snapshot.project_dir.clone().ok_or_else(|| {
                advanced_invalid("save the project before applying object removal")
            })?;
            let project_media = crate::library::ProjectMediaCapability::open_verified(
                &self.core,
                snapshot.project_epoch,
                &project_dir,
                true,
            )
            .map_err(advanced_execution)?;
            let leaf_name = format!("object-removed-{}.mov", uuid::Uuid::new_v4());
            let mut published = project_media
                .create_import(Path::new(&leaf_name))
                .map_err(advanced_execution)?;
            cached_file
                .seek(SeekFrom::Start(0))
                .map_err(|error| advanced_execution(error.to_string()))?;
            std::io::copy(&mut cached_file, published.file_mut())
                .and_then(|_| published.file_mut().flush())
                .and_then(|_| published.file().sync_all())
                .map_err(|error| advanced_execution(error.to_string()))?;
            if !project_media
                .matches_leaf(&published)
                .map_err(advanced_execution)?
            {
                return Err(advanced_execution(
                    "object-removal output identity changed before project commit",
                ));
            }
            let provenance = GenerationInput {
                prompt: json!({
                    "kind":"objectRemoval",
                    "maskIndex":mask_index,
                    "mask":mask,
                    "startFrame":start,
                    "endFrame":end
                })
                .to_string(),
                model: MODEL.into(),
                duration: clip.duration_frames,
                aspect_ratio: format!(
                    "{}:{}",
                    output_probe.width.unwrap_or(0),
                    output_probe.height.unwrap_or(0)
                ),
                provider: Some(PROVIDER.into()),
                status: Some(GenerationJobStatus::Ready),
                source_asset_id: Some(clip.media_ref.clone()),
                source_clip_id: Some(clip.id.clone()),
                source_start_frame: Some(start),
                source_end_frame: Some(end),
                ..GenerationInput::default()
            };
            let committed = self
                .core
                .commit_motion_media_for_project(
                    snapshot.project_epoch,
                    snapshot.version,
                    &project_dir,
                    published.path(),
                    "Object Removed",
                    &ProbedMedia {
                        duration_secs: output_probe.duration_secs,
                        width: output_probe
                            .width
                            .and_then(|value| i32::try_from(value).ok()),
                        height: output_probe
                            .height
                            .and_then(|value| i32::try_from(value).ok()),
                        fps: output_probe.fps,
                        has_audio: output_probe.has_audio,
                        color: output_probe.color,
                    },
                    provenance,
                    MotionPlacement::ReplaceAndClearMasks {
                        clip_id: clip.id.clone(),
                    },
                )
                .map_err(|error| advanced_execution(error.to_string()))?;
            published.commit();
            asset_id = Some(committed.media.id);
            action_name = Some(committed.edit.action_name);
            true
        } else {
            false
        };

        Ok(AdvancedWorkflowCommit {
            result: json!({
                "clipId": clip.id,
                "sourceMediaRef": clip.media_ref,
                "assetId": asset_id,
                "applied": applied,
                "cacheKey": cache_key,
                "previewPath": cache_path,
                "frameCount": clip.duration_frames,
                "width": output_probe.width,
                "height": output_probe.height,
                "fps": output_probe.fps,
                "provider": PROVIDER,
                "model": MODEL,
                "sourceSha256": source_sha256,
                "maskIndex": mask_index,
                "startFrame": start,
                "endFrame": end
            }),
            action_name,
        })
    }
}

impl AdvancedWorkflowBridge for TauriAdvancedWorkflowBridge {
    fn supported_tools(&self) -> Vec<ToolName> {
        let mut tools = vec![ToolName::TrackMotion, ToolName::RemoveObject];
        if verify_rvm_model(&self.models_dir).is_ok() {
            tools.push(ToolName::GenerateMatte);
        }
        tools
    }

    fn execute(
        &self,
        request: AdvancedWorkflowRequest,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        match request {
            AdvancedWorkflowRequest::TrackMotion(args) => self.track_motion(args, cancel),
            AdvancedWorkflowRequest::GenerateMatte(args) => self.generate_matte(args, cancel),
            AdvancedWorkflowRequest::RemoveObject(args) => self.remove_object(args, cancel),
            _ => Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                "advanced workflow is not supported by this desktop host",
            )),
        }
    }
}

fn position_keyframes(
    clip: &opentake_domain::Clip,
    tracked: &RegionMotionTrack,
) -> Vec<Keyframe<AnimPair>> {
    tracked
        .samples
        .iter()
        .map(|sample| {
            let absolute_frame = clip.start_frame + sample.frame;
            let base = clip.transform_at(absolute_frame);
            Keyframe::with_interpolation(
                sample.frame,
                AnimPair::new(
                    base.center_x + sample.translation_x,
                    base.center_y + sample.translation_y,
                ),
                Interpolation::Linear,
            )
        })
        .collect()
}

fn advanced_invalid(message: impl Into<String>) -> AdvancedWorkflowError {
    AdvancedWorkflowError::new(AdvancedWorkflowErrorKind::InvalidArguments, message)
}

fn advanced_resource(message: impl Into<String>) -> AdvancedWorkflowError {
    AdvancedWorkflowError::new(AdvancedWorkflowErrorKind::ResourceNotFound, message)
}

fn advanced_execution(message: impl Into<String>) -> AdvancedWorkflowError {
    AdvancedWorkflowError::new(AdvancedWorkflowErrorKind::ExecutionFailed, message)
}

fn cancelled_workflow(message: impl Into<String>) -> AdvancedWorkflowError {
    AdvancedWorkflowError::new(AdvancedWorkflowErrorKind::Cancelled, message)
}

fn media_workflow_error(error: MediaError) -> AdvancedWorkflowError {
    let kind = if matches!(error, MediaError::Cancelled) {
        AdvancedWorkflowErrorKind::Cancelled
    } else {
        AdvancedWorkflowErrorKind::ExecutionFailed
    };
    AdvancedWorkflowError::new(kind, error.to_string())
}

fn parse_mask_index(mask_id: &str) -> Result<usize, AdvancedWorkflowError> {
    if mask_id == "primary" {
        return Ok(0);
    }
    let numeric = mask_id.strip_prefix("mask-").unwrap_or(mask_id);
    numeric
        .parse::<usize>()
        .map_err(|_| advanced_invalid("maskId must be primary, a zero-based index, or mask-N"))
}

#[allow(clippy::too_many_arguments)]
fn materialize_object_removal_video(
    source: &Path,
    cache_path: &Path,
    clip: &opentake_domain::Clip,
    timeline_fps: i32,
    removal_start: i32,
    removal_end: i32,
    mask: &Mask,
    cancel: &MediaCancelToken,
) -> Result<(), AdvancedWorkflowError> {
    if cancel.checkpoint() {
        return Err(cancelled_workflow("object removal cancelled"));
    }
    let cache_dir = cache_path
        .parent()
        .ok_or_else(|| advanced_execution("object-removal cache path has no parent"))?;
    let metadata = std::fs::symlink_metadata(cache_dir)
        .map_err(|error| advanced_execution(error.to_string()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(advanced_execution(
            "object-removal cache directory must be a regular directory",
        ));
    }
    let source_probe = probe(source).map_err(media_workflow_error)?;
    let source_start = i64::from(clip.trim_start_frame);
    let source_end = source_start + i64::from(clip.duration_frames);
    let stream = spawn_video_stream(VideoStreamRequest {
        path: source.to_path_buf(),
        start_frame: source_start,
        end_frame: Some(source_end),
        timeline_fps,
        max_size: (0, 0),
        queue_capacity: 8,
        apply_rotation: true,
    })
    .map_err(media_workflow_error)?;
    let first = match receive_video_frame(&stream, cancel) {
        Ok(frame) => frame,
        Err(error) => {
            let _ = stream.join();
            return Err(error);
        }
    };
    let width = first.frame.width;
    let height = first.frame.height;
    validate_removal_mask(mask, width, height)?;
    let partial_path = cache_dir.join(format!(
        ".object-removal-{}.partial.mov",
        uuid::Uuid::new_v4()
    ));
    let preset = ExportPreset::new(VideoCodec::ProRes422, ExportResolution::P1080);
    let mut encoder = match VideoEncoder::new(&partial_path, width, height, timeline_fps, &preset) {
        Ok(encoder) => encoder,
        Err(error) => {
            let _ = stream.join();
            let _ = std::fs::remove_file(&partial_path);
            return Err(media_workflow_error(error));
        }
    };
    let mut encoded_frames = 0_i32;
    let encode_result = (|| {
        encode_object_removal_frame(
            first.frame,
            encoded_frames,
            clip,
            removal_start,
            removal_end,
            mask,
            &mut encoder,
            cancel,
        )?;
        encoded_frames += 1;
        while encoded_frames < clip.duration_frames {
            if cancel.checkpoint() {
                return Err(cancelled_workflow("object removal cancelled"));
            }
            let next = receive_video_frame(&stream, cancel)?;
            if next.frame.width != width || next.frame.height != height {
                return Err(advanced_execution(
                    "object-removal source dimensions changed during decode",
                ));
            }
            encode_object_removal_frame(
                next.frame,
                encoded_frames,
                clip,
                removal_start,
                removal_end,
                mask,
                &mut encoder,
                cancel,
            )?;
            encoded_frames += 1;
        }
        if source_probe.has_audio {
            let fps = timeline_fps.max(1) as f64;
            let audio = extract_pcm_cancellable(
                source,
                &PcmSpec {
                    sample_rate: 48_000,
                    channels: 1,
                    format: PcmFormat::F32,
                },
                Some((source_start as f64 / fps, source_end as f64 / fps)),
                cancel,
            )
            .map_err(media_workflow_error)?;
            encoder.push_audio(audio).map_err(media_workflow_error)?;
        }
        Ok(())
    })();
    stream.request_stop();
    let _ = stream.join();
    if let Err(error) = encode_result {
        encoder.abort();
        let _ = std::fs::remove_file(&partial_path);
        return Err(error);
    }
    if let Err(error) = encoder.finish_cancellable(cancel, None) {
        let _ = std::fs::remove_file(&partial_path);
        return Err(media_workflow_error(error));
    }
    match std::fs::hard_link(&partial_path, cache_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            let _ = std::fs::remove_file(&partial_path);
            return Err(advanced_execution(error.to_string()));
        }
    }
    let _ = std::fs::remove_file(&partial_path);
    let published = std::fs::symlink_metadata(cache_path)
        .map_err(|error| advanced_execution(error.to_string()))?;
    if !published.is_file() || published.file_type().is_symlink() {
        return Err(advanced_execution(
            "object-removal cache output must be a regular file",
        ));
    }
    Ok(())
}

fn validate_removal_mask(
    mask: &Mask,
    width: u32,
    height: u32,
) -> Result<(), AdvancedWorkflowError> {
    let mut selected = 0_usize;
    for y in 0..height {
        for x in 0..width {
            let coverage = mask.coverage(
                (f64::from(x) + 0.5) / f64::from(width),
                (f64::from(y) + 0.5) / f64::from(height),
            );
            selected += usize::from(coverage > 0.001);
        }
    }
    let pixels = width as usize * height as usize;
    if selected == 0 {
        return Err(advanced_invalid(
            "the selected mask does not cover any source pixels",
        ));
    }
    if selected == pixels {
        return Err(advanced_invalid(
            "the selected mask covers the entire frame; object removal needs surrounding pixels",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn encode_object_removal_frame(
    mut frame: RgbaFrame,
    output_index: i32,
    clip: &opentake_domain::Clip,
    removal_start: i32,
    removal_end: i32,
    mask: &Mask,
    encoder: &mut VideoEncoder,
    cancel: &MediaCancelToken,
) -> Result<(), AdvancedWorkflowError> {
    let project_frame = clip.start_frame + output_index;
    if (removal_start..removal_end).contains(&project_frame) {
        inpaint_masked_region(&mut frame, mask, cancel)?;
    }
    encoder.push_frame(&frame).map_err(media_workflow_error)
}

fn inpaint_masked_region(
    frame: &mut RgbaFrame,
    mask: &Mask,
    cancel: &MediaCancelToken,
) -> Result<(), AdvancedWorkflowError> {
    validate_removal_mask(mask, frame.width, frame.height)?;
    let width = frame.width as usize;
    let height = frame.height as usize;
    let pixels = width * height;
    let mut coverage = Vec::with_capacity(pixels);
    let mut known = Vec::with_capacity(pixels);
    for y in 0..height {
        if y % 32 == 0 && cancel.checkpoint() {
            return Err(cancelled_workflow("object removal cancelled"));
        }
        for x in 0..width {
            let value = mask.coverage(
                (x as f64 + 0.5) / width as f64,
                (y as f64 + 0.5) / height as f64,
            );
            coverage.push(value);
            known.push(value <= 0.001);
        }
    }
    let original = frame.rgba.clone();
    let mut filled = frame.rgba.clone();
    let mut queued = vec![false; pixels];
    let mut queue = VecDeque::new();
    for index in 0..pixels {
        if !known[index] && has_known_neighbour(index, width, height, &known) {
            queue.push_back(index);
            queued[index] = true;
        }
    }
    let mut processed = 0_usize;
    while let Some(index) = queue.pop_front() {
        if processed.is_multiple_of(4096) && cancel.checkpoint() {
            return Err(cancelled_workflow("object removal cancelled"));
        }
        let mut sums = [0_u32; 3];
        let mut count = 0_u32;
        for neighbour in neighbours(index, width, height).into_iter().flatten() {
            if known[neighbour] {
                let offset = neighbour * 4;
                sums[0] += u32::from(filled[offset]);
                sums[1] += u32::from(filled[offset + 1]);
                sums[2] += u32::from(filled[offset + 2]);
                count += 1;
            }
        }
        if count == 0 {
            continue;
        }
        let offset = index * 4;
        for channel in 0..3 {
            filled[offset + channel] = (sums[channel] / count) as u8;
        }
        known[index] = true;
        processed += 1;
        for neighbour in neighbours(index, width, height).into_iter().flatten() {
            if !known[neighbour] && !queued[neighbour] {
                queue.push_back(neighbour);
                queued[neighbour] = true;
            }
        }
    }
    if known.iter().any(|value| !value) {
        return Err(advanced_execution(
            "object-removal mask could not be filled from its boundary",
        ));
    }
    for (index, alpha) in coverage.into_iter().enumerate() {
        if alpha <= 0.0 {
            continue;
        }
        let offset = index * 4;
        for channel in 0..3 {
            frame.rgba[offset + channel] = (f64::from(original[offset + channel]) * (1.0 - alpha)
                + f64::from(filled[offset + channel]) * alpha)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
    Ok(())
}

fn has_known_neighbour(index: usize, width: usize, height: usize, known: &[bool]) -> bool {
    neighbours(index, width, height)
        .into_iter()
        .flatten()
        .any(|neighbour| known[neighbour])
}

fn neighbours(index: usize, width: usize, height: usize) -> [Option<usize>; 8] {
    let x = index % width;
    let y = index / width;
    let mut result = [None; 8];
    let mut cursor = 0;
    for dy in -1_i32..=1 {
        for dx in -1_i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && ny >= 0 && nx < width as i32 && ny < height as i32 {
                result[cursor] = Some(ny as usize * width + nx as usize);
                cursor += 1;
            }
        }
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn materialize_matte_video(
    source: &Path,
    cache_path: &Path,
    clip: &opentake_domain::Clip,
    timeline_fps: i32,
    matte_start: i32,
    matte_end: i32,
    models_dir: &Path,
    cancel: &MediaCancelToken,
) -> Result<(), AdvancedWorkflowError> {
    if cancel.checkpoint() {
        return Err(AdvancedWorkflowError::new(
            AdvancedWorkflowErrorKind::Cancelled,
            "matting cancelled",
        ));
    }
    let cache_dir = cache_path
        .parent()
        .ok_or_else(|| advanced_execution("matting cache path has no parent"))?;
    let metadata = std::fs::symlink_metadata(cache_dir)
        .map_err(|error| advanced_execution(error.to_string()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(advanced_execution(
            "matting cache directory must be a regular directory",
        ));
    }
    let source_probe = probe(source).map_err(media_workflow_error)?;
    let source_start = i64::from(clip.trim_start_frame);
    let source_end = source_start + i64::from(clip.duration_frames);
    let stream = spawn_video_stream(VideoStreamRequest {
        path: source.to_path_buf(),
        start_frame: source_start,
        end_frame: Some(source_end),
        timeline_fps,
        max_size: (0, 0),
        queue_capacity: 8,
        apply_rotation: true,
    })
    .map_err(media_workflow_error)?;
    let first = match receive_video_frame(&stream, cancel) {
        Ok(frame) => frame,
        Err(error) => {
            let _ = stream.join();
            return Err(error);
        }
    };
    let width = first.frame.width;
    let height = first.frame.height;
    let partial_path = cache_dir.join(format!(".matte-{}.partial.mov", uuid::Uuid::new_v4()));
    let preset = ExportPreset::new(VideoCodec::ProRes4444, ExportResolution::P1080);
    let mut encoder = match VideoEncoder::new(&partial_path, width, height, timeline_fps, &preset) {
        Ok(encoder) => encoder,
        Err(error) => {
            let _ = stream.join();
            let _ = std::fs::remove_file(&partial_path);
            return Err(media_workflow_error(error));
        }
    };
    let mut session = match RvmMattingSession::load(models_dir) {
        Ok(session) => session,
        Err(error) => {
            encoder.abort();
            let _ = stream.join();
            let _ = std::fs::remove_file(&partial_path);
            return Err(media_workflow_error(error));
        }
    };
    let mut encoded_frames = 0_i32;
    let encode_result = (|| {
        encode_matte_frame(
            first.frame,
            encoded_frames,
            clip,
            matte_start,
            matte_end,
            &mut session,
            &mut encoder,
            cancel,
        )?;
        encoded_frames += 1;
        while encoded_frames < clip.duration_frames {
            if cancel.checkpoint() {
                return Err(AdvancedWorkflowError::new(
                    AdvancedWorkflowErrorKind::Cancelled,
                    "matting cancelled",
                ));
            }
            let next = receive_video_frame(&stream, cancel)?;
            if next.frame.width != width || next.frame.height != height {
                return Err(advanced_execution(
                    "matting source dimensions changed during decode",
                ));
            }
            encode_matte_frame(
                next.frame,
                encoded_frames,
                clip,
                matte_start,
                matte_end,
                &mut session,
                &mut encoder,
                cancel,
            )?;
            encoded_frames += 1;
        }
        if source_probe.has_audio {
            let fps = timeline_fps.max(1) as f64;
            let audio = extract_pcm_cancellable(
                source,
                &PcmSpec {
                    sample_rate: 48_000,
                    channels: 1,
                    format: PcmFormat::F32,
                },
                Some((source_start as f64 / fps, source_end as f64 / fps)),
                cancel,
            )
            .map_err(media_workflow_error)?;
            encoder.push_audio(audio).map_err(media_workflow_error)?;
        }
        Ok(())
    })();
    stream.request_stop();
    let _ = stream.join();
    if let Err(error) = encode_result {
        encoder.abort();
        let _ = std::fs::remove_file(&partial_path);
        return Err(error);
    }
    if let Err(error) = encoder.finish_cancellable(cancel, None) {
        let _ = std::fs::remove_file(&partial_path);
        return Err(media_workflow_error(error));
    }
    match std::fs::hard_link(&partial_path, cache_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            let _ = std::fs::remove_file(&partial_path);
            return Err(advanced_execution(error.to_string()));
        }
    }
    let _ = std::fs::remove_file(&partial_path);
    let published = std::fs::symlink_metadata(cache_path)
        .map_err(|error| advanced_execution(error.to_string()))?;
    if !published.is_file() || published.file_type().is_symlink() {
        return Err(advanced_execution(
            "matting cache output must be a regular file",
        ));
    }
    Ok(())
}

fn receive_video_frame(
    stream: &VideoStream,
    cancel: &MediaCancelToken,
) -> Result<StreamVideoFrame, AdvancedWorkflowError> {
    loop {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("advanced video workflow cancelled"));
        }
        match stream
            .receiver()
            .recv_timeout(std::time::Duration::from_millis(50))
        {
            Ok(frame) => return frame.map_err(media_workflow_error),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(advanced_execution(
                    "advanced video source ended before the clip",
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_matte_frame(
    mut frame: RgbaFrame,
    output_index: i32,
    clip: &opentake_domain::Clip,
    matte_start: i32,
    matte_end: i32,
    session: &mut RvmMattingSession,
    encoder: &mut VideoEncoder,
    cancel: &MediaCancelToken,
) -> Result<(), AdvancedWorkflowError> {
    let project_frame = clip.start_frame + output_index;
    if (matte_start..matte_end).contains(&project_frame) {
        let matte = session
            .infer(&frame, cancel)
            .map_err(media_workflow_error)?;
        let mut rgba = Vec::with_capacity(matte.alpha.len() * 4);
        for (rgb, alpha) in matte.foreground_rgb.chunks_exact(3).zip(matte.alpha) {
            rgba.extend_from_slice(rgb);
            rgba.push(alpha);
        }
        frame = RgbaFrame::new(frame.width, frame.height, rgba);
    } else {
        for pixel in frame.rgba.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
    }
    encoder.push_frame(&frame).map_err(media_workflow_error)
}

fn open_verified_regular_file(path: &Path) -> Result<std::fs::File, AdvancedWorkflowError> {
    let before =
        std::fs::symlink_metadata(path).map_err(|error| advanced_execution(error.to_string()))?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(advanced_execution(
            "advanced workflow cache must be a regular file",
        ));
    }
    let file = std::fs::File::open(path).map_err(|error| advanced_execution(error.to_string()))?;
    let after =
        std::fs::symlink_metadata(path).map_err(|error| advanced_execution(error.to_string()))?;
    if !after.is_file() || after.file_type().is_symlink() {
        return Err(advanced_execution(
            "advanced workflow cache identity changed",
        ));
    }
    let opened = Handle::from_file(
        file.try_clone()
            .map_err(|error| advanced_execution(error.to_string()))?,
    )
    .map_err(|error| advanced_execution(error.to_string()))?;
    let current = Handle::from_path(path).map_err(|error| advanced_execution(error.to_string()))?;
    if opened != current {
        return Err(advanced_execution(
            "advanced workflow cache identity changed",
        ));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_core::{PreparedMediaImportOp, ProbedMedia};
    use opentake_domain::{Clip, ClipType, MaskShape, Point2};
    use opentake_media::analysis::StabilizationMotionSample;
    use opentake_ops::ClipEntry;
    use std::process::Command;

    #[test]
    fn tracked_motion_becomes_editable_linear_position_keyframes() {
        let mut clip = Clip::new("clip", "asset", 100, 30);
        clip.transform.center_x = 0.4;
        clip.transform.center_y = 0.6;
        let keyframes = position_keyframes(
            &clip,
            &RegionMotionTrack {
                samples: vec![
                    StabilizationMotionSample {
                        frame: 0,
                        translation_x: 0.0,
                        translation_y: 0.0,
                        rotation_degrees: 0.0,
                    },
                    StabilizationMotionSample {
                        frame: 10,
                        translation_x: 0.1,
                        translation_y: -0.05,
                        rotation_degrees: 0.0,
                    },
                ],
                minimum_confidence: 0.9,
            },
        );
        assert_eq!(keyframes.len(), 2);
        assert_eq!(keyframes[1].frame, 10);
        assert!((keyframes[1].value.a - 0.5).abs() < 1e-9);
        assert!((keyframes[1].value.b - 0.55).abs() < 1e-9);
        assert_eq!(keyframes[1].interpolation_out, Interpolation::Linear);
    }

    #[test]
    fn boundary_fill_removes_masked_pixels_and_preserves_unmasked_pixels() {
        let mut rgba = Vec::new();
        for y in 0..12_u8 {
            for x in 0..16_u8 {
                if (5..11).contains(&x) && (4..8).contains(&y) {
                    rgba.extend_from_slice(&[240, 10, 10, 255]);
                } else {
                    rgba.extend_from_slice(&[30, 90, 140, 255]);
                }
            }
        }
        let mut frame = RgbaFrame::new(16, 12, rgba);
        let before = frame.rgba.clone();
        let mask = Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.5, 0.5),
                radius: Point2::new(0.22, 0.22),
            },
            ..Mask::default()
        };

        inpaint_masked_region(&mut frame, &mask, &MediaCancelToken::new()).unwrap();

        let center = (6 * 16 + 8) * 4;
        assert!(frame.rgba[center] < 80);
        assert!(frame.rgba[center + 1] > 70);
        assert!(frame.rgba[center + 2] > 110);
        assert_eq!(&frame.rgba[0..4], &before[0..4]);
        assert_eq!(frame.rgba[center + 3], 255);
    }

    #[test]
    fn real_video_tracking_preview_apply_and_undo() {
        if !opentake_media::ffmpeg_status::ffmpeg_available() {
            eprintln!("SKIP: ffmpeg unavailable");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let frames_dir = root.path().join("frames");
        std::fs::create_dir(&frames_dir).unwrap();
        for frame_index in 0..12_u32 {
            let mut image = image::RgbaImage::from_pixel(96, 72, image::Rgba([8, 8, 8, 255]));
            let offset_x = 20 + frame_index;
            let offset_y = 24 + frame_index / 2;
            for y in offset_y..offset_y + 20 {
                for x in offset_x..offset_x + 24 {
                    let local_x = x - offset_x;
                    let local_y = y - offset_y;
                    image.put_pixel(
                        x,
                        y,
                        image::Rgba([
                            (local_x * 9 + local_y * 3) as u8,
                            (local_x * 2 + local_y * 11) as u8,
                            (local_x * 7 + local_y * 5) as u8,
                            255,
                        ]),
                    );
                }
            }
            image
                .save(frames_dir.join(format!("frame-{frame_index:03}.png")))
                .unwrap();
        }
        let source = root.path().join("moving-subject.mp4");
        let status = Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-framerate",
                "10",
            ])
            .arg("-i")
            .arg(frames_dir.join("frame-%03d.png"))
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());

        let core = AppCore::new();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 10,
            width: 96,
            height: 72,
        })
        .unwrap();
        let bundle = root.path().join("Tracking.opentake");
        core.save_project(Some(bundle.clone())).unwrap();
        let runtime = core.runtime_snapshot();
        let imported = core
            .import_media_batch_for_project_persisted(
                runtime.project_epoch,
                &bundle,
                vec![PreparedMediaImportOp::ImportFile {
                    path: source,
                    name: "moving-subject.mp4".into(),
                    probe: ProbedMedia {
                        duration_secs: 1.2,
                        width: Some(96),
                        height: Some(72),
                        fps: Some(10.0),
                        has_audio: false,
                        color: None,
                    },
                    folder: None,
                }],
            )
            .unwrap();
        let asset_id = imported[0].entry.id.clone();
        let placed = core
            .apply(EditCommand::AddClipsAutoTrack {
                entries: vec![ClipEntry {
                    media_ref: asset_id,
                    media_type: ClipType::Video,
                    source_clip_type: ClipType::Video,
                    track_index: 0,
                    start_frame: 0,
                    duration_frames: 12,
                    trim_start_frame: None,
                    trim_end_frame: None,
                    has_audio: false,
                    add_linked_audio: false,
                    transform: None,
                }],
            })
            .unwrap();
        let clip_id = placed.affected_clip_ids[0].clone();
        let bridge = TauriAdvancedWorkflowBridge::new(
            core.clone(),
            root.path().join("cache"),
            root.path().join("models"),
        );
        let request = TrackMotionArgs {
            clip_id: clip_id.clone(),
            region: json!({
                "x": 20.0 / 96.0,
                "y": 24.0 / 72.0,
                "width": 24.0 / 96.0,
                "height": 20.0 / 72.0
            }),
            start_frame: None,
            end_frame: None,
            apply: Some(false),
        };
        let preview = bridge
            .track_motion(request.clone(), &MediaCancelToken::new())
            .unwrap();
        assert!(preview.action_name.is_none());
        assert_eq!(preview.result["applied"], false);
        assert!(preview.result["keyframes"].as_array().unwrap().len() >= 2);
        assert!(core.runtime_snapshot().timeline.tracks[0].clips[0]
            .position_track
            .is_none());

        let applied = bridge
            .track_motion(
                TrackMotionArgs {
                    apply: Some(true),
                    ..request.clone()
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(applied.result["applied"], true);
        assert_eq!(applied.action_name.as_deref(), Some("Set Keyframes"));
        let after = core.runtime_snapshot();
        let clip = after.timeline.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.id == clip_id)
            .unwrap();
        assert!(clip.position_track.as_ref().unwrap().keyframes.len() >= 2);
        core.undo().unwrap();
        let undone = core.runtime_snapshot();
        assert!(undone.timeline.tracks[0].clips[0].position_track.is_none());

        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        let error = bridge
            .track_motion(
                TrackMotionArgs {
                    apply: Some(true),
                    ..request
                },
                &cancelled,
            )
            .expect_err("pre-cancelled analysis must fail");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::Cancelled);
        assert!(core.runtime_snapshot().timeline.tracks[0].clips[0]
            .position_track
            .is_none());
    }

    #[test]
    fn official_matting_preview_apply_undo_and_reopen() {
        let Some(model_source) = std::env::var_os("OPENTAKE_TEST_RVM_MODEL") else {
            eprintln!("SKIP: OPENTAKE_TEST_RVM_MODEL is not set");
            return;
        };
        if !opentake_media::ffmpeg_status::ffmpeg_available() {
            eprintln!("SKIP: ffmpeg unavailable");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let models_dir = root.path().join("models");
        let model_destination = opentake_media::analysis::matting_model_path(&models_dir);
        std::fs::create_dir_all(model_destination.parent().unwrap()).unwrap();
        std::fs::copy(model_source, model_destination).unwrap();

        let source = root.path().join("matting-source.mp4");
        let status = Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=64x64:rate=5:duration=0.4",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=0.4",
                "-shortest",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());

        let core = AppCore::new();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 5,
            width: 64,
            height: 64,
        })
        .unwrap();
        let bundle = root.path().join("Matting.opentake");
        core.save_project(Some(bundle.clone())).unwrap();
        let runtime = core.runtime_snapshot();
        let imported = core
            .import_media_batch_for_project_persisted(
                runtime.project_epoch,
                &bundle,
                vec![PreparedMediaImportOp::ImportFile {
                    path: source,
                    name: "matting-source.mp4".into(),
                    probe: ProbedMedia {
                        duration_secs: 0.4,
                        width: Some(64),
                        height: Some(64),
                        fps: Some(5.0),
                        has_audio: true,
                        color: None,
                    },
                    folder: None,
                }],
            )
            .unwrap();
        let source_asset_id = imported[0].entry.id.clone();
        let placed = core
            .apply(EditCommand::AddClipsAutoTrack {
                entries: vec![ClipEntry {
                    media_ref: source_asset_id.clone(),
                    media_type: ClipType::Video,
                    source_clip_type: ClipType::Video,
                    track_index: 0,
                    start_frame: 0,
                    duration_frames: 2,
                    trim_start_frame: None,
                    trim_end_frame: None,
                    has_audio: true,
                    add_linked_audio: false,
                    transform: None,
                }],
            })
            .unwrap();
        let clip_id = placed.affected_clip_ids[0].clone();
        let bridge =
            TauriAdvancedWorkflowBridge::new(core.clone(), root.path().join("cache"), models_dir);
        assert!(bridge.supported_tools().contains(&ToolName::GenerateMatte));
        let request = GenerateMatteArgs {
            clip_id: clip_id.clone(),
            model: None,
            start_frame: None,
            end_frame: None,
            apply: Some(false),
        };
        let preview = bridge
            .generate_matte(request.clone(), &MediaCancelToken::new())
            .unwrap();
        assert_eq!(preview.result["applied"], false);
        assert_eq!(core.media().entries.len(), 1);
        assert_eq!(
            core.runtime_snapshot().timeline.tracks[0].clips[0].media_ref,
            source_asset_id
        );

        let applied = bridge
            .generate_matte(
                GenerateMatteArgs {
                    apply: Some(true),
                    ..request.clone()
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(applied.result["applied"], true);
        assert_eq!(applied.action_name.as_deref(), Some("Edit Motion Graphic"));
        let generated_asset_id = applied.result["assetId"].as_str().unwrap().to_string();
        let generated = core
            .media()
            .entries
            .into_iter()
            .find(|entry| entry.id == generated_asset_id)
            .unwrap();
        assert!(generated.carries_straight_alpha());
        assert_eq!(generated.has_audio, Some(true));
        assert_eq!(
            generated
                .generation_input
                .as_ref()
                .unwrap()
                .source_asset_id
                .as_deref(),
            Some(source_asset_id.as_str())
        );
        assert_eq!(
            core.runtime_snapshot().timeline.tracks[0].clips[0].media_ref,
            generated_asset_id
        );

        core.undo().unwrap();
        assert_eq!(
            core.runtime_snapshot().timeline.tracks[0].clips[0].media_ref,
            source_asset_id
        );
        core.redo().unwrap();
        core.save_project(None).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        let reopened_clip = &reopened.runtime_snapshot().timeline.tracks[0].clips[0];
        assert_eq!(reopened_clip.id, clip_id);
        assert_eq!(reopened_clip.media_ref, generated_asset_id);
        assert!(reopened
            .media()
            .entries
            .iter()
            .any(|entry| entry.id == source_asset_id));

        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        let error = bridge
            .generate_matte(request, &cancelled)
            .expect_err("pre-cancelled matting must fail even when cached");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::Cancelled);
    }

    #[test]
    fn object_removal_preview_range_apply_undo_reopen_and_failure_atomicity() {
        if !opentake_media::ffmpeg_status::ffmpeg_available() {
            eprintln!("SKIP: ffmpeg unavailable");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let frames_dir = root.path().join("frames");
        std::fs::create_dir(&frames_dir).unwrap();
        for frame_index in 0..8_u32 {
            let mut image = image::RgbaImage::from_pixel(
                64,
                48,
                image::Rgba([30 + frame_index as u8, 90, 140, 255]),
            );
            for y in 17..31 {
                for x in 24..40 {
                    image.put_pixel(x, y, image::Rgba([240, 12, 12, 255]));
                }
            }
            image
                .save(frames_dir.join(format!("frame-{frame_index:03}.png")))
                .unwrap();
        }
        let source = root.path().join("object-removal-source.mp4");
        let status = Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-framerate",
                "4",
            ])
            .arg("-i")
            .arg(frames_dir.join("frame-%03d.png"))
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=2",
                "-shortest",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());

        let core = AppCore::new();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 4,
            width: 64,
            height: 48,
        })
        .unwrap();
        let bundle = root.path().join("ObjectRemoval.opentake");
        core.save_project(Some(bundle.clone())).unwrap();
        let runtime = core.runtime_snapshot();
        let imported = core
            .import_media_batch_for_project_persisted(
                runtime.project_epoch,
                &bundle,
                vec![PreparedMediaImportOp::ImportFile {
                    path: source,
                    name: "object-removal-source.mp4".into(),
                    probe: ProbedMedia {
                        duration_secs: 2.0,
                        width: Some(64),
                        height: Some(48),
                        fps: Some(4.0),
                        has_audio: true,
                        color: None,
                    },
                    folder: None,
                }],
            )
            .unwrap();
        let source_asset_id = imported[0].entry.id.clone();
        let placed = core
            .apply(EditCommand::AddClipsAutoTrack {
                entries: vec![ClipEntry {
                    media_ref: source_asset_id.clone(),
                    media_type: ClipType::Video,
                    source_clip_type: ClipType::Video,
                    track_index: 0,
                    start_frame: 0,
                    duration_frames: 8,
                    trim_start_frame: None,
                    trim_end_frame: None,
                    has_audio: true,
                    add_linked_audio: false,
                    transform: None,
                }],
            })
            .unwrap();
        let clip_id = placed.affected_clip_ids[0].clone();
        let mask = Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.5, 0.5),
                radius: Point2::new(0.16, 0.22),
            },
            feather: 0.01,
            ..Mask::default()
        };
        core.apply(EditCommand::SetMasks {
            clip_ids: vec![clip_id.clone()],
            masks: vec![mask.clone()],
        })
        .unwrap();
        core.save_project(None).unwrap();

        let bridge = TauriAdvancedWorkflowBridge::new(
            core.clone(),
            root.path().join("cache"),
            root.path().join("models"),
        );
        assert!(bridge.supported_tools().contains(&ToolName::RemoveObject));
        let request = RemoveObjectArgs {
            clip_id: clip_id.clone(),
            mask_id: "primary".into(),
            start_frame: Some(2),
            end_frame: Some(6),
            provider: Some("local".into()),
            model: Some("opentake-boundary-fill-v1".into()),
            cost_authorized: None,
            apply: Some(false),
        };
        let before_preview = core.runtime_snapshot();
        let preview = bridge
            .remove_object(request.clone(), &MediaCancelToken::new())
            .unwrap();
        assert_eq!(preview.result["applied"], false);
        assert_eq!(core.runtime_snapshot().timeline, before_preview.timeline);
        assert_eq!(core.runtime_snapshot().media, before_preview.media);
        let preview_path = PathBuf::from(preview.result["previewPath"].as_str().unwrap());
        let decoded = decode_frames_at_cancellable(
            &preview_path,
            &[0.25, 0.75],
            &FrameRequest {
                max_size: (64, 48),
                tolerance_secs: 0.1,
                ..FrameRequest::default()
            },
            &MediaCancelToken::new(),
        )
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
        let center = (24 * 64 + 32) * 4;
        assert!(
            decoded[0].1.rgba[center] > 180,
            "outside-range frame changed"
        );
        assert!(decoded[1].1.rgba[center] < 120, "masked object remained");

        let before_failure = core.runtime_snapshot();
        let error = bridge
            .remove_object(
                RemoveObjectArgs {
                    mask_id: "mask-99".into(),
                    ..request.clone()
                },
                &MediaCancelToken::new(),
            )
            .expect_err("missing mask must fail");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::ResourceNotFound);
        let after_failure = core.runtime_snapshot();
        assert_eq!(after_failure.timeline, before_failure.timeline);
        assert_eq!(after_failure.media, before_failure.media);
        assert_eq!(after_failure.version, before_failure.version);

        let applied = bridge
            .remove_object(
                RemoveObjectArgs {
                    apply: Some(true),
                    ..request.clone()
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(applied.action_name.as_deref(), Some("Remove Masked Object"));
        let generated_asset_id = applied.result["assetId"].as_str().unwrap().to_string();
        let after_apply = core.runtime_snapshot();
        assert_eq!(after_apply.timeline.tracks[0].clips[0].id, clip_id);
        assert_eq!(
            after_apply.timeline.tracks[0].clips[0].media_ref,
            generated_asset_id
        );
        assert!(after_apply.timeline.tracks[0].clips[0].masks.is_empty());
        let generated = core
            .media()
            .entries
            .into_iter()
            .find(|entry| entry.id == generated_asset_id)
            .unwrap();
        let (applied_path, applied_is_video) =
            crate::transcribe::resolve_asset_from_snapshot(&after_apply, &generated_asset_id)
                .unwrap();
        assert!(applied_is_video);
        assert_eq!(
            file_sha256(&applied_path).unwrap(),
            file_sha256(&preview_path).unwrap(),
            "Apply must publish the exact reviewed derivative consumed by playback/export"
        );
        assert_eq!(generated.has_audio, Some(true));
        let provenance = generated.generation_input.unwrap();
        assert_eq!(provenance.provider.as_deref(), Some("opentake-local"));
        assert_eq!(provenance.model, "opentake-boundary-fill-v1");
        assert_eq!(
            provenance.source_asset_id.as_deref(),
            Some(source_asset_id.as_str())
        );

        core.undo().unwrap();
        let undone = core.runtime_snapshot();
        assert_eq!(
            undone.timeline.tracks[0].clips[0].media_ref,
            source_asset_id
        );
        assert_eq!(undone.timeline.tracks[0].clips[0].masks, vec![mask]);
        core.redo().unwrap();
        core.save_project(None).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        let reopened_clip = &reopened.runtime_snapshot().timeline.tracks[0].clips[0];
        assert_eq!(reopened_clip.id, clip_id);
        assert_eq!(reopened_clip.media_ref, generated_asset_id);
        assert!(reopened_clip.masks.is_empty());
        assert!(reopened
            .media()
            .entries
            .iter()
            .any(|entry| entry.id == source_asset_id));

        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        let error = bridge
            .remove_object(request, &cancelled)
            .expect_err("pre-cancelled cached preview must fail");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::Cancelled);
    }
}
