use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use opentake_agent::mcp::core_handle::CoreHandle;
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::mcp::motion::{
    AddMotionRequest, EditMotionRequest, MotionBridge, MotionBridgeError, MotionCommit,
    MotionOutputMetadata,
};
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_agent::tools::names::ToolName;
use opentake_domain::{MediaManifest, Timeline};
use opentake_ops::{EditCommand, EditResult};

struct ReadOnlyHandle;

struct DeterministicMotionBridge;

fn output_metadata(content_hash: &str) -> MotionOutputMetadata {
    MotionOutputMetadata {
        renderer: "fixture".into(),
        renderer_version: "1".into(),
        output_file: "output.mp4".into(),
        fps: 30.0,
        width: 64,
        height: 36,
        duration_frames: 30,
        duration_seconds: 1.0,
        content_hash: content_hash.into(),
    }
}

impl MotionBridge for DeterministicMotionBridge {
    fn can_render_motion(&self) -> bool {
        true
    }

    fn add(
        &self,
        _request: AddMotionRequest,
        _cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionCommit, MotionBridgeError> {
        Ok(MotionCommit {
            clip_id: "motion-clip".into(),
            asset_id: "motion-asset".into(),
            content_hash: "add-hash".into(),
            action_name: "Add Motion Graphic".into(),
            output: output_metadata("add-hash"),
        })
    }

    fn edit(
        &self,
        request: EditMotionRequest,
        _cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionCommit, MotionBridgeError> {
        Ok(MotionCommit {
            clip_id: request.clip_id,
            asset_id: "edited-motion-asset".into(),
            content_hash: "edit-hash".into(),
            action_name: "Edit Motion Graphic".into(),
            output: output_metadata("edit-hash"),
        })
    }
}

impl CoreHandle for ReadOnlyHandle {
    fn timeline(&self) -> Timeline {
        Timeline::new()
    }

    fn media(&self) -> MediaManifest {
        MediaManifest::new()
    }

    fn apply(&self, _cmd: EditCommand) -> anyhow::Result<EditResult> {
        anyhow::bail!("advertised-tool fixture is read-only")
    }

    fn project_dir(&self) -> Option<PathBuf> {
        None
    }
}

#[test]
fn every_advertised_tool_is_live_or_absent() {
    let dispatcher = Dispatcher::with_capability_bridges(
        Arc::new(ReadOnlyHandle),
        Arc::new(RwLock::new(PluginRegistry::new())),
        None,
        None,
        Some(Arc::new(DeterministicMotionBridge)),
    );
    let cases = [
        (
            ToolName::InspectMedia,
            serde_json::json!({"mediaRef": "asset"}),
        ),
        (
            ToolName::GenerateVideo,
            serde_json::json!({"prompt": "clip"}),
        ),
        (
            ToolName::GenerateImage,
            serde_json::json!({"prompt": "still"}),
        ),
        (
            ToolName::GenerateAudio,
            serde_json::json!({"prompt": "music"}),
        ),
        (
            ToolName::UpscaleMedia,
            serde_json::json!({"mediaRef": "asset"}),
        ),
        (
            ToolName::AddMotionGraphic,
            serde_json::json!({
                "source": {"code": "export default {}"},
                "startFrame": 0,
                "durationFrames": 30
            }),
        ),
        (
            ToolName::EditMotionGraphic,
            serde_json::json!({"clipId": "clip", "code": "export default {}"}),
        ),
    ];
    let advertised = dispatcher.advertised_tools();

    for (tool, args) in cases {
        if !advertised.contains(&tool) {
            let result = dispatcher.dispatch(tool.as_str(), args);
            assert!(
                result.text_joined().contains("not advertised"),
                "{} is hidden from discovery but direct dispatch did not fail closed: {}",
                tool.as_str(),
                result.text_joined()
            );
            continue;
        }
        let result = dispatcher.dispatch(tool.as_str(), args);
        assert!(
            !result.text_joined().contains("not yet implemented"),
            "{} is advertised but still reaches a placeholder stub: {}",
            tool.as_str(),
            result.text_joined()
        );
        assert!(
            !result.text_joined().contains("not advertised"),
            "{} was advertised but dispatch rejected it: {}",
            tool.as_str(),
            result.text_joined()
        );
    }
}

#[test]
fn motion_tools_are_absent_without_a_live_host_bridge() {
    let dispatcher = Dispatcher::new(
        Arc::new(ReadOnlyHandle),
        Arc::new(RwLock::new(PluginRegistry::new())),
    );

    for tool in ToolName::MOTION {
        assert!(!dispatcher.advertised_tools().contains(&tool));
    }
}
