//! Production desktop implementations for capability-gated advanced workflows.

use opentake_agent::mcp::advanced::{
    AdvancedWorkflowBridge, AdvancedWorkflowCommit, AdvancedWorkflowError,
    AdvancedWorkflowErrorKind, AdvancedWorkflowRequest,
};
use opentake_agent::tools::args::TrackMotionArgs;
use opentake_agent::tools::names::ToolName;
use opentake_core::{AppCore, ProjectRevision};
use opentake_domain::{AnimPair, Interpolation, Keyframe, KeyframeTrack};
use opentake_media::analysis::{track_region_motion, NormalizedMotionRegion, RegionMotionTrack};
use opentake_media::{decode_frames_at_cancellable, FrameRequest, MediaCancelToken};
use opentake_ops::{EditCommand, KeyframePayload, KeyframeProperty};
use serde_json::json;

pub struct TauriAdvancedWorkflowBridge {
    core: AppCore,
}

impl TauriAdvancedWorkflowBridge {
    pub fn new(core: AppCore) -> Self {
        Self { core }
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
}

impl AdvancedWorkflowBridge for TauriAdvancedWorkflowBridge {
    fn supported_tools(&self) -> Vec<ToolName> {
        vec![ToolName::TrackMotion]
    }

    fn execute(
        &self,
        request: AdvancedWorkflowRequest,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        match request {
            AdvancedWorkflowRequest::TrackMotion(args) => self.track_motion(args, cancel),
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

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_core::{PreparedMediaImportOp, ProbedMedia};
    use opentake_domain::Clip;
    use opentake_domain::ClipType;
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
        let bridge = TauriAdvancedWorkflowBridge::new(core.clone());
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
}
