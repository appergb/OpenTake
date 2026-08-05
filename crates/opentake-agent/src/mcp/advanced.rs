//! Host boundary for capability-gated advanced editing workflows.
//!
//! The agent owns stable, strict tool contracts. The desktop host owns model
//! availability, provider authorization, rendering, imports, and atomic edits.
//! A tool is discoverable only when the injected host bridge explicitly lists
//! it as supported; this prevents schema-only placeholders from reaching users.

use serde_json::Value;

use crate::tools::args::{
    CloneVoiceArgs, GenerateAvatarArgs, GenerateMatteArgs, MatchColorArgs, RemoveObjectArgs,
    ScriptToVideoArgs, SeparateStemsArgs, TrackMotionArgs, TranslateCaptionsArgs,
};
use crate::tools::names::ToolName;

#[derive(Debug, Clone, PartialEq)]
pub enum AdvancedWorkflowRequest {
    TrackMotion(TrackMotionArgs),
    GenerateMatte(GenerateMatteArgs),
    RemoveObject(RemoveObjectArgs),
    MatchColor(MatchColorArgs),
    SeparateStems(SeparateStemsArgs),
    TranslateCaptions(TranslateCaptionsArgs),
    ScriptToVideo(ScriptToVideoArgs),
    GenerateAvatar(GenerateAvatarArgs),
    CloneVoice(CloneVoiceArgs),
}

impl AdvancedWorkflowRequest {
    pub fn tool(&self) -> ToolName {
        match self {
            Self::TrackMotion(_) => ToolName::TrackMotion,
            Self::GenerateMatte(_) => ToolName::GenerateMatte,
            Self::RemoveObject(_) => ToolName::RemoveObject,
            Self::MatchColor(_) => ToolName::MatchColor,
            Self::SeparateStems(_) => ToolName::SeparateStems,
            Self::TranslateCaptions(_) => ToolName::TranslateCaptions,
            Self::ScriptToVideo(_) => ToolName::ScriptToVideo,
            Self::GenerateAvatar(_) => ToolName::GenerateAvatar,
            Self::CloneVoice(_) => ToolName::CloneVoice,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdvancedWorkflowCommit {
    /// Structured result returned to the agent after a successful operation.
    pub result: Value,
    /// Present only when the host committed an undoable project mutation.
    pub action_name: Option<String>,
}

/// Rebuild a host-owned advanced-workflow payload as a model-facing typed
/// allowlist. The desktop host may carry paths, provider request ids, hashes,
/// or raw provider diagnostics in its internal result; none are copied unless a
/// field is explicitly reconstructed below with its expected primitive shape.
pub(crate) fn model_safe_result(tool: ToolName, result: &Value) -> Value {
    let source = result.as_object();
    let mut out = serde_json::Map::new();
    out.insert("tool".into(), Value::String(tool.as_str().to_string()));

    if let Some(status) = source
        .and_then(|object| object.get("status"))
        .and_then(Value::as_str)
        .filter(|status| matches!(*status, "completed" | "completed-without-edit"))
    {
        out.insert("status".into(), Value::String(status.to_string()));
    }

    match tool {
        ToolName::TrackMotion => {
            copy_id(source, &mut out, "clipId");
            copy_bool(source, &mut out, "applied");
            copy_number(source, &mut out, "minimumConfidence");
            copy_region(source, &mut out);
            copy_tracking_keyframes(source, &mut out);
        }
        ToolName::GenerateMatte | ToolName::RemoveObject => {
            for key in ["clipId", "sourceMediaRef", "assetId"] {
                copy_id(source, &mut out, key);
            }
            copy_bool(source, &mut out, "applied");
            for key in ["frameCount", "width", "height", "startFrame", "endFrame"] {
                copy_integer(source, &mut out, key);
            }
            copy_number(source, &mut out, "fps");
        }
        ToolName::MatchColor => {
            for key in ["clipId", "referenceMediaRef"] {
                copy_id(source, &mut out, key);
            }
            for key in ["referenceFrame", "targetFrame"] {
                copy_integer(source, &mut out, key);
            }
            for key in [
                "targetMeanLinear",
                "referenceMeanLinear",
                "matchedMeanLinear",
                "deltaEBefore",
                "deltaEAfter",
                "targetLumaBefore",
                "targetLumaAfter",
            ] {
                copy_number(source, &mut out, key);
            }
            copy_bool(source, &mut out, "applied");
        }
        ToolName::SeparateStems => {
            for key in ["sourceMediaRef", "vocalsAssetId", "accompanimentAssetId"] {
                copy_id(source, &mut out, key);
            }
            copy_id_array(source, &mut out, "clipIds", 64);
            copy_bool(source, &mut out, "importedToTracks");
            copy_number(source, &mut out, "vocalSdrImprovementDb");
        }
        ToolName::TranslateCaptions => {
            for key in ["projectEpoch", "version", "captionCount", "translatedCount"] {
                copy_integer(source, &mut out, key);
            }
            for key in ["sourceLocale", "targetLocale"] {
                copy_token(source, &mut out, key, 32);
            }
            copy_bool(source, &mut out, "applied");
            copy_translation_review(source, &mut out);
            let failure_count = source
                .and_then(|object| object.get("errors"))
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            out.insert("failureCount".into(), Value::from(failure_count));
        }
        ToolName::ScriptToVideo => {
            for key in ["projectEpoch", "version", "startFrame", "endFrame"] {
                copy_integer(source, &mut out, key);
            }
            copy_id(source, &mut out, "planId");
            copy_bool(source, &mut out, "applied");
            copy_script_segments(source, &mut out);
        }
        ToolName::GenerateAvatar => {
            for key in ["assetId", "portraitMediaRef", "audioMediaRef"] {
                copy_id(source, &mut out, key);
            }
            copy_id_array(source, &mut out, "clipIds", 64);
            copy_integer(source, &mut out, "durationFrames");
            copy_bool(source, &mut out, "imported");
        }
        ToolName::CloneVoice => {
            copy_enum(
                source,
                &mut out,
                "action",
                &["enroll", "generate", "revoke"],
            );
            for key in ["voiceId", "assetId", "sourceAudioMediaRef"] {
                copy_id(source, &mut out, key);
            }
            copy_id_array(source, &mut out, "clipIds", 64);
            copy_text(source, &mut out, "voiceName", 256);
            copy_integer(source, &mut out, "durationFrames");
            copy_bool(source, &mut out, "imported");
            copy_bool(source, &mut out, "revoked");
        }
        _ => {}
    }
    Value::Object(out)
}

fn safe_token(value: &str, max_chars: usize) -> bool {
    !value.is_empty()
        && value.chars().count() <= max_chars
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn copy_id(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
    key: &str,
) {
    copy_token(source, out, key, 128);
}

fn copy_token(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
    key: &str,
    max_chars: usize,
) {
    if let Some(value) = source
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .filter(|value| safe_token(value, max_chars))
    {
        out.insert(key.into(), Value::String(value.to_string()));
    }
}

fn copy_text(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
    key: &str,
    max_chars: usize,
) {
    if let Some(value) = source
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .filter(|value| value.chars().count() <= max_chars)
    {
        out.insert(key.into(), Value::String(value.to_string()));
    }
}

fn copy_enum(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
    key: &str,
    allowed: &[&str],
) {
    if let Some(value) = source
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .filter(|value| allowed.contains(value))
    {
        out.insert(key.into(), Value::String(value.to_string()));
    }
}

fn copy_bool(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(value) = source
        .and_then(|object| object.get(key))
        .and_then(Value::as_bool)
    {
        out.insert(key.into(), Value::Bool(value));
    }
}

fn copy_integer(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(value) = source.and_then(|object| object.get(key)).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|v| i64::try_from(v).ok()))
    }) {
        out.insert(key.into(), Value::from(value));
    }
}

fn copy_number(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(value) = source
        .and_then(|object| object.get(key))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .and_then(serde_json::Number::from_f64)
    {
        out.insert(key.into(), Value::Number(value));
    }
}

fn copy_id_array(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
    key: &str,
    max_items: usize,
) {
    let Some(values) = source
        .and_then(|object| object.get(key))
        .and_then(Value::as_array)
    else {
        return;
    };
    let values = values
        .iter()
        .take(max_items)
        .filter_map(Value::as_str)
        .filter(|value| safe_token(value, 128))
        .map(|value| Value::String(value.to_string()))
        .collect();
    out.insert(key.into(), Value::Array(values));
}

fn copy_region(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
) {
    let Some(region) = source
        .and_then(|object| object.get("region"))
        .and_then(Value::as_object)
    else {
        return;
    };
    let mut safe = serde_json::Map::new();
    for key in ["x", "y", "width", "height"] {
        copy_number(Some(region), &mut safe, key);
    }
    if safe.len() == 4 {
        out.insert("region".into(), Value::Object(safe));
    }
}

fn copy_tracking_keyframes(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
) {
    let Some(values) = source
        .and_then(|object| object.get("keyframes"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let rows = values
        .iter()
        .take(10_000)
        .filter_map(Value::as_object)
        .filter_map(|row| {
            let frame = row.get("frame")?.as_i64()?;
            let position = row.get("position")?.as_object()?;
            let x = position
                .get("x")?
                .as_f64()
                .filter(|value| value.is_finite())?;
            let y = position
                .get("y")?
                .as_f64()
                .filter(|value| value.is_finite())?;
            Some(serde_json::json!({
                "frame": frame,
                "position": {"x": x, "y": y},
                "interpolation": "linear"
            }))
        })
        .collect();
    out.insert("keyframes".into(), Value::Array(rows));
}

fn copy_translation_review(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
) {
    let Some(values) = source
        .and_then(|object| object.get("review"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let rows = values
        .iter()
        .take(500)
        .filter_map(Value::as_object)
        .filter_map(|row| {
            let id = row
                .get("id")?
                .as_str()
                .filter(|value| safe_token(value, 128))?;
            let source_text = row
                .get("sourceText")?
                .as_str()
                .filter(|value| value.chars().count() <= 20_000)?;
            let translated_text = row
                .get("translatedText")?
                .as_str()
                .filter(|value| value.chars().count() <= 20_000)?;
            Some(serde_json::json!({
                "id": id,
                "sourceText": source_text,
                "translatedText": translated_text,
            }))
        })
        .collect();
    out.insert("review".into(), Value::Array(rows));
}

fn copy_script_segments(
    source: Option<&serde_json::Map<String, Value>>,
    out: &mut serde_json::Map<String, Value>,
) {
    let Some(values) = source
        .and_then(|object| object.get("segments"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let rows = values
        .iter()
        .take(100)
        .filter_map(Value::as_object)
        .filter_map(|row| {
            let script = row
                .get("script")?
                .as_str()
                .filter(|value| value.chars().count() <= 20_000)?;
            let media_ref = row
                .get("mediaRef")?
                .as_str()
                .filter(|value| safe_token(value, 128))?;
            let start_frame = row.get("startFrame")?.as_i64()?;
            let duration_frames = row.get("durationFrames")?.as_i64()?;
            let mut safe = serde_json::json!({
                "script": script,
                "mediaRef": media_ref,
                "startFrame": start_frame,
                "durationFrames": duration_frames,
            });
            if let Some(narration) = row
                .get("narrationMediaRef")
                .and_then(Value::as_str)
                .filter(|value| safe_token(value, 128))
            {
                safe["narrationMediaRef"] = Value::String(narration.to_string());
            }
            Some(safe)
        })
        .collect();
    out.insert("segments".into(), Value::Array(rows));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvancedWorkflowErrorKind {
    InvalidArguments,
    ResourceNotFound,
    CapabilityUnavailable,
    ConsentRequired,
    CostAuthorizationRequired,
    AnalysisLowConfidence,
    Cancelled,
    ExecutionFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvancedWorkflowError {
    pub kind: AdvancedWorkflowErrorKind,
    pub message: String,
}

impl AdvancedWorkflowError {
    pub fn new(kind: AdvancedWorkflowErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub trait AdvancedWorkflowBridge: Send + Sync {
    /// Exact advanced tools backed by a production implementation right now.
    /// The dispatcher ignores names outside [`ToolName::ADVANCED_AI`].
    fn supported_tools(&self) -> Vec<ToolName>;

    fn execute(
        &self,
        request: AdvancedWorkflowRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_safe_results_drop_host_paths_urls_provider_ids_and_diagnostics() {
        const PRIVATE_PATH: &str = "/Users/private/render/avatar.mov";
        const SIGNED_URL: &str = "https://provider.invalid/output.mov?token=SIGNED_ADVANCED_SECRET";
        const PROVIDER_ID: &str = "provider-request-PRIVATE-123";
        const RAW_ERROR: &str = "ffmpeg failed opening /Users/private/source.mov";
        const API_KEY: &str = "sk-private-advanced-key";
        const PRIVATE_PROMPT: &str = "PRIVATE_AVATAR_PROMPT";

        let result = serde_json::json!({
            "status": "completed",
            "clipId": "clip-safe",
            "sourceMediaRef": "source-safe",
            "assetId": "asset-safe",
            "portraitMediaRef": "portrait-safe",
            "audioMediaRef": "audio-safe",
            "clipIds": ["clip-safe"],
            "applied": true,
            "imported": true,
            "previewPath": PRIVATE_PATH,
            "signedUrl": SIGNED_URL,
            "providerRequestId": PROVIDER_ID,
            "prompt": PRIVATE_PROMPT,
            "errors": [{"message": RAW_ERROR, "apiKey": API_KEY}],
            "provider": {"apiKey": API_KEY, "url": SIGNED_URL},
            "unknown": {"path": PRIVATE_PATH}
        });

        for tool in ToolName::ADVANCED_AI {
            let safe = model_safe_result(tool, &result);
            let encoded = safe.to_string();
            assert_eq!(safe["tool"], tool.as_str());
            assert_eq!(safe["status"], "completed");
            for private in [
                PRIVATE_PATH,
                SIGNED_URL,
                PROVIDER_ID,
                RAW_ERROR,
                API_KEY,
                PRIVATE_PROMPT,
            ] {
                assert!(
                    !encoded.contains(private),
                    "{} leaked {private}: {encoded}",
                    tool.as_str()
                );
            }
            for forbidden_key in [
                "previewPath",
                "signedUrl",
                "providerRequestId",
                "prompt",
                "errors",
                "provider",
                "unknown",
            ] {
                assert!(safe.get(forbidden_key).is_none(), "{tool:?}: {safe}");
            }
        }
    }
}
