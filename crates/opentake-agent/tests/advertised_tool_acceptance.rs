use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use opentake_agent::mcp::core_handle::CoreHandle;
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_agent::tools::names::ToolName;
use opentake_domain::{MediaManifest, Timeline};
use opentake_ops::{EditCommand, EditResult};

struct ReadOnlyHandle;

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
    let dispatcher = Dispatcher::new(
        Arc::new(ReadOnlyHandle),
        Arc::new(RwLock::new(PluginRegistry::new())),
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

    for (tool, args) in cases {
        if !ToolName::ALL.contains(&tool) {
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
    }
}
