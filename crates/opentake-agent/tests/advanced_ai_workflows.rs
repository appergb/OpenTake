use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use opentake_agent::mcp::advanced::{
    AdvancedWorkflowBridge, AdvancedWorkflowCommit, AdvancedWorkflowError, AdvancedWorkflowRequest,
};
use opentake_agent::mcp::core_handle::CoreHandle;
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_agent::tools::names::ToolName;
use opentake_domain::{MediaManifest, Timeline};
use opentake_ops::{EditCommand, EditResult};
use serde_json::{json, Value};

struct ReadOnlyHandle;

impl CoreHandle for ReadOnlyHandle {
    fn timeline(&self) -> Timeline {
        Timeline::new()
    }

    fn media(&self) -> MediaManifest {
        MediaManifest::new()
    }

    fn apply(&self, _cmd: EditCommand) -> anyhow::Result<EditResult> {
        anyhow::bail!("advanced workflow fixture is read-only")
    }

    fn project_dir(&self) -> Option<PathBuf> {
        None
    }
}

struct DeterministicAdvancedBridge;

impl AdvancedWorkflowBridge for DeterministicAdvancedBridge {
    fn supported_tools(&self) -> Vec<ToolName> {
        ToolName::ADVANCED_AI.to_vec()
    }

    fn execute(
        &self,
        request: AdvancedWorkflowRequest,
        _cancel: &opentake_media::MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        Ok(AdvancedWorkflowCommit {
            result: json!({"tool": request.tool().as_str(), "status": "completed"}),
            action_name: None,
        })
    }
}

struct MisleadingAdvancedBridge;

impl AdvancedWorkflowBridge for MisleadingAdvancedBridge {
    fn supported_tools(&self) -> Vec<ToolName> {
        vec![ToolName::TrackMotion]
    }

    fn execute(
        &self,
        _request: AdvancedWorkflowRequest,
        _cancel: &opentake_media::MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        Ok(AdvancedWorkflowCommit {
            result: json!({"status":"completed-without-edit"}),
            action_name: Some("Track Motion".into()),
        })
    }
}

struct LeakyAdvancedBridge;

impl AdvancedWorkflowBridge for LeakyAdvancedBridge {
    fn supported_tools(&self) -> Vec<ToolName> {
        ToolName::ADVANCED_AI.to_vec()
    }

    fn execute(
        &self,
        request: AdvancedWorkflowRequest,
        _cancel: &opentake_media::MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        Ok(AdvancedWorkflowCommit {
            result: json!({
                "tool": request.tool().as_str(),
                "status": "completed",
                "clipId": "clip-safe",
                "assetId": "asset-safe",
                "previewPath": "/Users/private/advanced-output.mov",
                "signedUrl": "https://provider.invalid/output?token=SIGNED_WORKFLOW_SECRET",
                "providerRequestId": "provider-secret-request-id",
                "prompt": "PRIVATE_WORKFLOW_PROMPT",
                "errors": [{"message": "raw provider error with sk-secret-provider-key"}],
            }),
            action_name: None,
        })
    }
}

fn dispatcher(advanced: bool) -> Dispatcher {
    Dispatcher::with_all_capability_bridges(
        Arc::new(ReadOnlyHandle),
        Arc::new(RwLock::new(PluginRegistry::new())),
        None,
        None,
        None,
        advanced.then(|| Arc::new(DeterministicAdvancedBridge) as Arc<dyn AdvancedWorkflowBridge>),
    )
}

fn cases() -> [(ToolName, Value); 9] {
    [
        (
            ToolName::TrackMotion,
            json!({"clipId":"clip","region":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}}),
        ),
        (ToolName::GenerateMatte, json!({"clipId":"clip"})),
        (
            ToolName::RemoveObject,
            json!({"clipId":"clip","maskId":"mask"}),
        ),
        (
            ToolName::MatchColor,
            json!({"clipId":"clip","referenceMediaRef":"asset"}),
        ),
        (ToolName::SeparateStems, json!({"mediaRef":"asset"})),
        (
            ToolName::TranslateCaptions,
            json!({"captionClipIds":["caption"],"targetLocale":"zh-CN"}),
        ),
        (
            ToolName::ScriptToVideo,
            json!({"segments":[{"script":"intro","mediaRef":"asset","durationFrames":30}]}),
        ),
        (
            ToolName::GenerateAvatar,
            json!({"portraitMediaRef":"portrait","audioMediaRef":"audio","consentId":"consent","costAuthorized":true}),
        ),
        (
            ToolName::CloneVoice,
            json!({"action":"revoke","consentId":"consent","voiceId":"voice"}),
        ),
    ]
}

#[test]
fn advanced_ai_workflows_are_hidden_without_a_live_host() {
    let dispatcher = dispatcher(false);
    for (tool, args) in cases() {
        assert!(!dispatcher.advertised_tools().contains(&tool));
        let result = dispatcher.dispatch(tool.as_str(), args);
        assert!(result.is_error);
        assert!(result.text_joined().contains("not advertised"));
    }
}

#[test]
fn advanced_ai_workflows_route_through_exact_tool_contracts() {
    let dispatcher = dispatcher(true);
    for (tool, args) in cases() {
        assert!(ToolName::KNOWN.contains(&tool));
        assert!(dispatcher.advertised_tools().contains(&tool));
        let result = dispatcher.dispatch(tool.as_str(), args);
        assert!(
            !result.is_error,
            "{}: {}",
            tool.as_str(),
            result.text_joined()
        );
        assert!(result.text_joined().contains(tool.as_str()));
        assert!(result.text_joined().contains("completed"));
    }

    for tool in [ToolName::TightenSilences, ToolName::RemoveFillerWords] {
        assert!(ToolName::ALL.contains(&tool));
    }
}

#[test]
fn advanced_dispatch_results_never_forward_private_host_payload_fields() {
    let dispatcher = Dispatcher::with_all_capability_bridges(
        Arc::new(ReadOnlyHandle),
        Arc::new(RwLock::new(PluginRegistry::new())),
        None,
        None,
        None,
        Some(Arc::new(LeakyAdvancedBridge)),
    );
    for (tool, args) in cases() {
        let result = dispatcher.dispatch(tool.as_str(), args);
        assert!(
            !result.is_error,
            "{}: {}",
            tool.as_str(),
            result.text_joined()
        );
        let text = result.text_joined();
        assert!(text.contains(tool.as_str()), "{text}");
        for private in [
            "/Users/private/advanced-output.mov",
            "SIGNED_WORKFLOW_SECRET",
            "provider-secret-request-id",
            "PRIVATE_WORKFLOW_PROMPT",
            "sk-secret-provider-key",
        ] {
            assert!(
                !text.contains(private),
                "{} leaked {private}: {text}",
                tool.as_str()
            );
        }
    }
}

#[test]
fn advanced_nested_contracts_reject_unknown_fields_before_host_execution() {
    let dispatcher = dispatcher(true);
    let result = dispatcher.dispatch(
        "track_motion",
        json!({"clipId":"clip","region":{"x":0.0,"y":0.0,"width":1.0,"height":1.0,"secret":true}}),
    );
    assert!(result.is_error);
    assert!(
        result.text_joined().contains("region:") && result.text_joined().contains("'secret'"),
        "{}",
        result.text_joined()
    );

    let result = dispatcher.dispatch(
        "script_to_video",
        json!({"segments":[{"script":"x","mediaRef":"asset","durationFrames":30,"secret":true}]}),
    );
    assert!(result.is_error);
    assert!(
        result.text_joined().contains("segments[0]:") && result.text_joined().contains("'secret'"),
        "{}",
        result.text_joined()
    );
}

#[test]
fn action_name_without_a_real_undo_transaction_grants_no_undo_authority() {
    let dispatcher = Dispatcher::with_all_capability_bridges(
        Arc::new(ReadOnlyHandle),
        Arc::new(RwLock::new(PluginRegistry::new())),
        None,
        None,
        None,
        Some(Arc::new(MisleadingAdvancedBridge)),
    );
    let result = dispatcher.dispatch(
        "track_motion",
        json!({"clipId":"clip","region":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}}),
    );
    assert!(!result.is_error, "{}", result.text_joined());

    let undo = dispatcher.dispatch("undo", json!({}));
    assert!(undo.is_error);
    assert!(undo.text_joined().contains("No assistant edit"));
}
