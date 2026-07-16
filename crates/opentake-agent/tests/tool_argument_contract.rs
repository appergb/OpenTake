use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use opentake_agent::mcp::core_handle::CoreHandle;
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_agent::tools::descriptions::input_schema;
use opentake_agent::tools::names::ToolName;
use opentake_domain::{MediaManifest, Timeline};
use opentake_ops::{EditCommand, EditResult};

struct ReadOnlyHandle {
    apply_calls: Arc<AtomicUsize>,
}

impl CoreHandle for ReadOnlyHandle {
    fn timeline(&self) -> Timeline {
        Timeline::new()
    }

    fn media(&self) -> MediaManifest {
        MediaManifest::new()
    }

    fn apply(&self, _cmd: EditCommand) -> anyhow::Result<EditResult> {
        self.apply_calls.fetch_add(1, Ordering::AcqRel);
        anyhow::bail!("read-only contract fixture")
    }

    fn project_dir(&self) -> Option<PathBuf> {
        None
    }
}

fn dispatcher() -> (Dispatcher, Arc<AtomicUsize>) {
    let apply_calls = Arc::new(AtomicUsize::new(0));
    (
        Dispatcher::new(
            Arc::new(ReadOnlyHandle {
                apply_calls: apply_calls.clone(),
            }),
            Arc::new(RwLock::new(PluginRegistry::new())),
        ),
        apply_calls,
    )
}

#[test]
fn all_tool_schemas_reject_unknown_missing_wrong_type() {
    let (dispatcher, apply_calls) = dispatcher();

    for tool in ToolName::ALL {
        let unknown = dispatcher.dispatch(tool.as_str(), serde_json::json!({"__unknown": true}));
        assert!(
            unknown.is_error,
            "{} accepted an unknown field",
            tool.as_str()
        );
        assert!(
            unknown.text_joined().contains("unknown field"),
            "{} bypassed strict validation: {}",
            tool.as_str(),
            unknown.text_joined()
        );

        let wrong_root = dispatcher.dispatch(tool.as_str(), serde_json::json!([]));
        assert!(
            wrong_root.is_error,
            "{} accepted a non-object argument root",
            tool.as_str()
        );
        assert!(
            !wrong_root.text_joined().contains("not yet implemented"),
            "{} reached its stub before validating argument types: {}",
            tool.as_str(),
            wrong_root.text_joined()
        );
    }

    let required = [
        ToolName::InspectMedia,
        ToolName::SearchMedia,
        ToolName::AddClips,
        ToolName::InsertClips,
        ToolName::RemoveClips,
        ToolName::RemoveTracks,
        ToolName::MoveClips,
        ToolName::SetClipProperties,
        ToolName::SetKeyframes,
        ToolName::SplitClip,
        ToolName::RippleDeleteRanges,
        ToolName::AddTexts,
        ToolName::SmartReframe,
        ToolName::GenerateVideo,
        ToolName::GenerateImage,
        ToolName::UpscaleMedia,
        ToolName::ImportMedia,
        ToolName::DeleteMedia,
        ToolName::DeleteFolder,
        ToolName::ActivateWorkflow,
        ToolName::SetColorGrade,
        ToolName::ChromaKey,
        ToolName::SetMask,
        ToolName::ApplyEffect,
        ToolName::AddMotionGraphic,
        ToolName::EditMotionGraphic,
    ];
    for tool in required {
        let missing = dispatcher.dispatch(tool.as_str(), serde_json::json!({}));
        assert!(
            missing.is_error,
            "{} accepted missing required args",
            tool.as_str()
        );
        assert!(
            !missing.text_joined().contains("not yet implemented"),
            "{} reached its stub before checking required args: {}",
            tool.as_str(),
            missing.text_joined()
        );
    }

    let nested_unknowns = [
        (
            ToolName::SetClipProperties,
            serde_json::json!({"clipIds": [], "transform": {"bogus": 1}}),
            "transform",
        ),
        (
            ToolName::AddTexts,
            serde_json::json!({"entries": [{"startFrame": 0, "durationFrames": 1, "content": "x", "transform": {"bogus": 1}}]}),
            "entries[0].transform",
        ),
        (
            ToolName::SetColorGrade,
            serde_json::json!({"clipIds": [], "lift": {"bogus": 1}}),
            "lift",
        ),
        (
            ToolName::SetMask,
            serde_json::json!({"clipIds": [], "masks": [{"kind": "circle", "center": {"bogus": 1}}]}),
            "masks[0].center",
        ),
        (
            ToolName::ImportMedia,
            serde_json::json!({"source": {"path": "/x.mp4", "bogus": 1}}),
            "source",
        ),
        (
            ToolName::AddMotionGraphic,
            serde_json::json!({"source": {"code": "x", "bogus": 1}, "startFrame": 0, "durationFrames": 1}),
            "source",
        ),
    ];
    for (tool, args, expected_path) in nested_unknowns {
        let calls_before = apply_calls.load(Ordering::Acquire);
        let result = dispatcher.dispatch(tool.as_str(), args);
        assert!(result.is_error, "{} accepted nested unknown", tool.as_str());
        assert!(
            result.text_joined().contains(expected_path)
                && result.text_joined().contains("unknown field"),
            "{} did not report the nested path: {}",
            tool.as_str(),
            result.text_joined()
        );
        assert_eq!(
            apply_calls.load(Ordering::Acquire),
            calls_before,
            "{} dispatched after nested validation failed",
            tool.as_str()
        );
    }

    let valid_entry = serde_json::json!({
        "mediaRef": "asset",
        "startFrame": 0,
        "durationFrames": 1
    });
    let exact_cases = [
        (
            "missing",
            serde_json::json!({
                "entries": [
                    valid_entry.clone(),
                    valid_entry.clone(),
                    valid_entry.clone(),
                    {"mediaRef": "asset", "durationFrames": 1}
                ]
            }),
            "entries[3].startFrame: missing required field 'startFrame'",
        ),
        (
            "type",
            serde_json::json!({
                "entries": [
                    valid_entry.clone(),
                    valid_entry.clone(),
                    valid_entry.clone(),
                    {"mediaRef": "asset", "startFrame": "soon", "durationFrames": 1}
                ]
            }),
            "entries[3].startFrame: expected i32, got something else",
        ),
        (
            "unknown",
            serde_json::json!({
                "entries": [
                    valid_entry.clone(),
                    valid_entry.clone(),
                    valid_entry,
                    {"mediaRef": "asset", "startFrame": 0, "durationFrames": 1, "startFrames": 0}
                ]
            }),
            "entries[3]: unknown field(s) 'startFrames'. Allowed: durationFrames, mediaRef, startFrame, trackIndex, trimEndFrame, trimStartFrame.",
        ),
    ];
    for (case, args, expected) in exact_cases {
        let calls_before = apply_calls.load(Ordering::Acquire);
        let result = dispatcher.dispatch(ToolName::AddClips.as_str(), args);
        assert!(result.is_error, "{case} case unexpectedly succeeded");
        assert_eq!(result.text_joined(), expected, "{case} message drifted");
        assert_eq!(
            apply_calls.load(Ordering::Acquire),
            calls_before,
            "{case} case reached the edit boundary"
        );
    }

    let add_texts_schema = input_schema(ToolName::AddTexts);
    let text_transform_schema = add_texts_schema
        .pointer("/properties/entries/items/properties/transform")
        .expect("add_texts transform schema");
    assert_eq!(
        text_transform_schema.get("additionalProperties"),
        Some(&serde_json::Value::Bool(false))
    );
    for key in ["flipHorizontal", "flipVertical"] {
        assert!(
            text_transform_schema
                .pointer(&format!("/properties/{key}"))
                .is_some(),
            "add_texts schema omitted runtime transform key {key}"
        );
    }
    let flipped_text = dispatcher.dispatch(
        ToolName::AddTexts.as_str(),
        serde_json::json!({
            "entries": [{
                "startFrame": 0,
                "durationFrames": 1,
                "content": "flip",
                "transform": {"flipHorizontal": true, "flipVertical": false}
            }]
        }),
    );
    assert!(
        !flipped_text.text_joined().contains("unknown field"),
        "runtime rejected schema-declared transform keys: {}",
        flipped_text.text_joined()
    );

    let transcript_type = dispatcher.dispatch(
        ToolName::GetTranscript.as_str(),
        serde_json::json!({"wordTimestamps": "yes"}),
    );
    assert!(transcript_type.is_error);
    assert!(
        transcript_type.text_joined().contains("wordTimestamps")
            && transcript_type.text_joined().contains("expected"),
        "accepted key must still enforce its declared type: {}",
        transcript_type.text_joined()
    );

    // Dynamic maps are deliberate key exceptions; their values still follow
    // the published per-tool schema.
    let effect = dispatcher.dispatch(
        ToolName::ApplyEffect.as_str(),
        serde_json::json!({
            "clipIds": [],
            "effects": [{"name": "custom", "params": {"vendorRadius": 2.0}}]
        }),
    );
    assert!(
        !effect.text_joined().contains("unknown field"),
        "dynamic effect params were incorrectly closed: {}",
        effect.text_joined()
    );
    let motion = dispatcher.dispatch(
        ToolName::AddMotionGraphic.as_str(),
        serde_json::json!({
            "source": {"templateId": "x", "params": {"title": "Hello", "count": 2, "visible": true}},
            "startFrame": 0,
            "durationFrames": 1
        }),
    );
    assert!(
        motion.text_joined().contains("not yet implemented"),
        "dynamic motion params must remain open: {}",
        motion.text_joined()
    );

    let invalid_motion_values = [
        (
            ToolName::AddMotionGraphic,
            serde_json::json!({
                "source": {"templateId": "x", "params": {"vendor": {"nested": true}}},
                "startFrame": 0,
                "durationFrames": 1
            }),
            "source.params.vendor: expected string, number, or bool, got something else",
        ),
        (
            ToolName::EditMotionGraphic,
            serde_json::json!({"clipId": "c1", "params": {"vendor": [1, 2]}}),
            "params.vendor: expected string, number, or bool, got something else",
        ),
    ];
    for (tool, args, expected) in invalid_motion_values {
        let result = dispatcher.dispatch(tool.as_str(), args);
        assert!(result.is_error);
        assert_eq!(result.text_joined(), expected);
        assert!(!result.text_joined().contains("not yet implemented"));
    }

    for (args, expected) in [
        (
            serde_json::json!({
                "source": {},
                "startFrame": 0,
                "durationFrames": 1
            }),
            "source: exactly one of 'code' or 'templateId' is required",
        ),
        (
            serde_json::json!({
                "source": {"code": "x", "templateId": "y"},
                "startFrame": 0,
                "durationFrames": 1
            }),
            "source: exactly one of 'code' or 'templateId' is required",
        ),
        (
            serde_json::json!({
                "source": {"code": "x", "params": {"title": "invalid with code"}},
                "startFrame": 0,
                "durationFrames": 1
            }),
            "source.params: only valid with 'templateId'",
        ),
    ] {
        let result = dispatcher.dispatch(ToolName::AddMotionGraphic.as_str(), args);
        assert!(result.is_error);
        assert_eq!(result.text_joined(), expected);
    }

    let empty_edit = dispatcher.dispatch(
        ToolName::EditMotionGraphic.as_str(),
        serde_json::json!({"clipId": "c1"}),
    );
    assert!(empty_edit.is_error);
    assert_eq!(
        empty_edit.text_joined(),
        "arguments: at least one of 'code' or 'params' is required"
    );
}
