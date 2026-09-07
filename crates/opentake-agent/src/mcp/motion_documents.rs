//! Typed, capability-confined Agent access to project Motion Studio documents.
//!
//! Admission captures the exact host project while its lifecycle lease is
//! held; execution happens only after that lease is released so the desktop
//! bridge can take publication locks and run Chromium/FFmpeg without reversing
//! the project lock order.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tools::errors::{decode_tool_args, ToolArgs, ToolError};
use crate::tools::names::ToolName;
use crate::tools::result::{Block, PublicErrorKind, ToolResult};

pub const MAX_MOTION_DOCUMENTS: usize = 128;
pub const MAX_MOTION_DOCUMENT_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_MOTION_DOCUMENT_EDITS: usize = 256;
pub const MAX_MOTION_DOCUMENT_TITLE_CHARS: usize = 120;
pub const MAX_MOTION_PREVIEW_DIMENSION: u32 = 4096;
// Keep this capability boundary aligned with the production Chromium renderer
// (`opentake_motion::limits::MAX_FRAMES`) without adding a renderer dependency
// to the Agent crate.
pub const MAX_MOTION_PREVIEW_FRAMES: u32 = 3_600;
pub const MAX_MOTION_PREVIEW_PNG_BASE64: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionDocumentTool {
    List,
    Read,
    Create,
    Patch,
    Preview,
    Publish,
}

impl MotionDocumentTool {
    pub const ALL: [Self; 6] = [
        Self::List,
        Self::Read,
        Self::Create,
        Self::Patch,
        Self::Preview,
        Self::Publish,
    ];

    pub fn from_tool_name(tool: ToolName) -> Option<Self> {
        match tool {
            ToolName::ListMotionDocuments => Some(Self::List),
            ToolName::ReadMotionDocument => Some(Self::Read),
            ToolName::CreateMotionDocument => Some(Self::Create),
            ToolName::PatchMotionDocument => Some(Self::Patch),
            ToolName::PreviewMotionDocument => Some(Self::Preview),
            ToolName::PublishMotionDocument => Some(Self::Publish),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionDocumentSummary {
    pub document_id: String,
    pub title: String,
    pub revision_hash: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionDocument {
    pub summary: MotionDocumentSummary,
    pub html: String,
    pub css: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionTextReplacement {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionDocumentPatchRequest {
    pub document_id: String,
    pub file: String,
    pub baseline_hash: String,
    pub edits: Vec<MotionTextReplacement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionDocumentPreviewRequest {
    pub document_id: String,
    pub revision_hash: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_frames: u32,
    pub frame: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionDocumentPublishRequest {
    pub document_id: String,
    pub revision_hash: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_frames: i32,
    pub start_frame: Option<i32>,
    pub track_index: Option<usize>,
    /// Transparent output is supported for a new published clip. Existing
    /// Motion clip replacement remains opaque until alpha-preserving edit is
    /// implemented end to end.
    pub transparent: bool,
    pub clip_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotionDocumentRequest {
    List,
    Read { document_id: String },
    Create { title: Option<String> },
    Patch(MotionDocumentPatchRequest),
    Preview(MotionDocumentPreviewRequest),
    Publish(MotionDocumentPublishRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionPreviewDiagnostic {
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionDocumentPreview {
    pub revision_hash: String,
    pub frame: u32,
    pub png_base64: String,
    pub diagnostics: Vec<MotionPreviewDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionDocumentPublish {
    pub clip_id: String,
    pub asset_id: String,
    pub duration_frames: i32,
    pub duration_seconds: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub source_document: MotionDocumentReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionDocumentReference {
    pub document_id: String,
    pub revision_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MotionDocumentResponse {
    Documents(Vec<MotionDocumentSummary>),
    Document(MotionDocument),
    Preview(MotionDocumentPreview),
    Published(MotionDocumentPublish),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionDocumentBridgeErrorKind {
    InvalidArguments,
    ResourceNotFound,
    Conflict,
    CapabilityUnavailable,
    Cancelled,
    RenderFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionDocumentBridgeError {
    pub kind: MotionDocumentBridgeErrorKind,
    pub message: String,
    pub current_revision_hash: Option<String>,
}

impl MotionDocumentBridgeError {
    pub fn new(kind: MotionDocumentBridgeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            current_revision_hash: None,
        }
    }

    pub fn conflict(current_revision_hash: Option<String>) -> Self {
        Self {
            kind: MotionDocumentBridgeErrorKind::Conflict,
            message: "Motion Studio document revision conflict".into(),
            current_revision_hash,
        }
    }
}

pub trait AdmittedMotionDocumentOperation: Send {
    fn execute(
        self: Box<Self>,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionDocumentResponse, MotionDocumentBridgeError>;
}

pub trait MotionDocumentBridge: Send + Sync {
    fn can_edit_motion_documents(&self) -> bool;

    fn admit(
        &self,
        request: MotionDocumentRequest,
    ) -> Result<Box<dyn AdmittedMotionDocumentOperation>, MotionDocumentBridgeError>;
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadArgs {
    document_id: String,
}
impl ToolArgs for ReadArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["documentId"];
}

#[derive(Debug, Default, Deserialize)]
struct EmptyArgs {}
impl ToolArgs for EmptyArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[];
}

#[derive(Debug, Default, Deserialize)]
struct CreateArgs {
    title: Option<String>,
}
impl ToolArgs for CreateArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["title"];
}

#[derive(Debug, Default, Deserialize)]
struct ReplacementArgs {
    start: usize,
    end: usize,
    replacement: String,
}
impl ToolArgs for ReplacementArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["start", "end", "replacement"];
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PatchArgs {
    document_id: String,
    file: String,
    baseline_hash: String,
    edits: Vec<Value>,
}
impl ToolArgs for PatchArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &["documentId", "file", "baselineHash", "edits"];
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewArgs {
    document_id: String,
    revision_hash: String,
    width: u32,
    height: u32,
    fps: u32,
    duration_frames: u32,
    frame: u32,
}
impl ToolArgs for PreviewArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "documentId",
        "revisionHash",
        "width",
        "height",
        "fps",
        "durationFrames",
        "frame",
    ];
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishArgs {
    document_id: String,
    revision_hash: String,
    width: u32,
    height: u32,
    fps: u32,
    duration_frames: i32,
    start_frame: Option<i32>,
    track_index: Option<usize>,
    #[serde(default)]
    transparent: bool,
    clip_id: Option<String>,
}
impl ToolArgs for PublishArgs {
    const ALLOWED_KEYS: &'static [&'static str] = &[
        "documentId",
        "revisionHash",
        "width",
        "height",
        "fps",
        "durationFrames",
        "startFrame",
        "trackIndex",
        "transparent",
        "clipId",
    ];
}

pub(crate) fn decode_request(
    tool: ToolName,
    args: &Value,
) -> Result<MotionDocumentRequest, ToolError> {
    match MotionDocumentTool::from_tool_name(tool) {
        Some(MotionDocumentTool::List) => {
            let _: EmptyArgs = decode_tool_args(args, "")?;
            Ok(MotionDocumentRequest::List)
        }
        Some(MotionDocumentTool::Read) => {
            let args: ReadArgs = decode_tool_args(args, "")?;
            validate_document_id(&args.document_id)?;
            Ok(MotionDocumentRequest::Read {
                document_id: args.document_id,
            })
        }
        Some(MotionDocumentTool::Create) => {
            let args: CreateArgs = decode_tool_args(args, "")?;
            if let Some(title) = args.title.as_deref() {
                validate_title(title)?;
            }
            Ok(MotionDocumentRequest::Create { title: args.title })
        }
        Some(MotionDocumentTool::Patch) => {
            let args: PatchArgs = decode_tool_args(args, "")?;
            validate_document_id(&args.document_id)?;
            validate_hash(&args.baseline_hash, "baselineHash")?;
            if !matches!(args.file.as_str(), "index.html" | "styles.css") {
                return Err(ToolError::new(
                    "file must be exactly 'index.html' or 'styles.css'",
                ));
            }
            if args.edits.is_empty() || args.edits.len() > MAX_MOTION_DOCUMENT_EDITS {
                return Err(ToolError::new(
                    "edits must contain between 1 and 256 replacements",
                ));
            }
            let mut inserted_bytes = 0usize;
            let edits = args
                .edits
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let edit: ReplacementArgs =
                        decode_tool_args(value, &format!("edits[{index}]"))?;
                    if edit.start > edit.end {
                        return Err(ToolError::new(format!(
                            "edits[{index}]: start must not exceed end"
                        )));
                    }
                    inserted_bytes = inserted_bytes
                        .checked_add(edit.replacement.len())
                        .ok_or_else(|| ToolError::new("edits exceed their byte limit"))?;
                    if inserted_bytes > MAX_MOTION_DOCUMENT_SOURCE_BYTES {
                        return Err(ToolError::new("edits exceed their byte limit"));
                    }
                    Ok(MotionTextReplacement {
                        start: edit.start,
                        end: edit.end,
                        replacement: edit.replacement,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(MotionDocumentRequest::Patch(MotionDocumentPatchRequest {
                document_id: args.document_id,
                file: args.file,
                baseline_hash: args.baseline_hash,
                edits,
            }))
        }
        Some(MotionDocumentTool::Preview) => {
            let args: PreviewArgs = decode_tool_args(args, "")?;
            validate_document_id(&args.document_id)?;
            validate_hash(&args.revision_hash, "revisionHash")?;
            validate_render_bounds(args.width, args.height, args.fps, args.duration_frames)?;
            if args.frame >= args.duration_frames {
                return Err(ToolError::new("frame must be inside durationFrames"));
            }
            Ok(MotionDocumentRequest::Preview(
                MotionDocumentPreviewRequest {
                    document_id: args.document_id,
                    revision_hash: args.revision_hash,
                    width: args.width,
                    height: args.height,
                    fps: args.fps,
                    duration_frames: args.duration_frames,
                    frame: args.frame,
                },
            ))
        }
        Some(MotionDocumentTool::Publish) => {
            let args: PublishArgs = decode_tool_args(args, "")?;
            validate_document_id(&args.document_id)?;
            validate_hash(&args.revision_hash, "revisionHash")?;
            let frames = u32::try_from(args.duration_frames)
                .map_err(|_| ToolError::new("durationFrames must be positive"))?;
            validate_render_bounds(args.width, args.height, args.fps, frames)?;
            if !args.width.is_multiple_of(2) || !args.height.is_multiple_of(2) {
                return Err(ToolError::new(
                    "publish width and height must be even numbers",
                ));
            }
            match (&args.clip_id, args.start_frame) {
                (None, Some(start)) if start >= 0 => {}
                (Some(clip_id), None) => validate_safe_id(clip_id, "clipId")?,
                (None, _) => {
                    return Err(ToolError::new(
                        "startFrame is required when clipId is omitted",
                    ))
                }
                (Some(_), Some(_)) => {
                    return Err(ToolError::new(
                        "startFrame must be omitted when clipId is provided",
                    ))
                }
            }
            if args.clip_id.is_some() && args.track_index.is_some() {
                return Err(ToolError::new(
                    "trackIndex must be omitted when clipId is provided",
                ));
            }
            if args.clip_id.is_some() && args.transparent {
                return Err(ToolError::new(
                    "transparent is supported only when publishing a new Motion clip",
                ));
            }
            Ok(MotionDocumentRequest::Publish(
                MotionDocumentPublishRequest {
                    document_id: args.document_id,
                    revision_hash: args.revision_hash,
                    width: args.width,
                    height: args.height,
                    fps: args.fps,
                    duration_frames: args.duration_frames,
                    start_frame: args.start_frame,
                    track_index: args.track_index,
                    transparent: args.transparent,
                    clip_id: args.clip_id,
                },
            ))
        }
        None => Err(ToolError::new("not a Motion Studio document tool")),
    }
}

pub(crate) fn result_from_operation(
    tool: ToolName,
    operation: Box<dyn AdmittedMotionDocumentOperation>,
    cancel: &opentake_media::MediaCancelToken,
) -> ToolResult {
    if cancel.is_cancelled() {
        return ToolResult::error("Cancelled");
    }
    match operation.execute(cancel) {
        Ok(response) => model_safe_response(response),
        Err(error) => result_from_error(tool, error),
    }
}

pub(crate) fn result_from_error(tool: ToolName, error: MotionDocumentBridgeError) -> ToolResult {
    if error.kind == MotionDocumentBridgeErrorKind::Conflict {
        let current_revision_hash = error
            .current_revision_hash
            .filter(|hash| validate_hash_value(hash).is_ok());
        return ToolResult::ok(
            serde_json::json!({
                "status": "conflict",
                "currentRevisionHash": current_revision_hash,
                "remediation": "Read the document again and reapply the intended patch explicitly."
            })
            .to_string(),
        );
    }
    let (kind, detail) = match error.kind {
        MotionDocumentBridgeErrorKind::InvalidArguments => (
            PublicErrorKind::InvalidArguments(tool),
            "Motion Studio document arguments are invalid.",
        ),
        MotionDocumentBridgeErrorKind::ResourceNotFound => (
            PublicErrorKind::ResourceNotFound(tool),
            "The Motion Studio document or clip was not found.",
        ),
        MotionDocumentBridgeErrorKind::CapabilityUnavailable => (
            PublicErrorKind::CapabilityUnavailable(tool),
            "Motion Studio rendering is unavailable.",
        ),
        MotionDocumentBridgeErrorKind::Cancelled | MotionDocumentBridgeErrorKind::RenderFailed => {
            return ToolResult::error(match error.kind {
                MotionDocumentBridgeErrorKind::Cancelled => {
                    "Motion Studio operation was cancelled."
                }
                _ => "Motion Studio rendering failed.",
            })
        }
        MotionDocumentBridgeErrorKind::Conflict => unreachable!(),
    };
    ToolResult::public_error(kind, detail)
}

fn model_safe_response(response: MotionDocumentResponse) -> ToolResult {
    match response {
        MotionDocumentResponse::Documents(documents) => {
            if documents.len() > MAX_MOTION_DOCUMENTS
                || documents.iter().any(|item| validate_summary(item).is_err())
            {
                return ToolResult::error("Motion Studio document response exceeded its bounds");
            }
            ToolResult::ok(serde_json::json!({"documents": documents}).to_string())
        }
        MotionDocumentResponse::Document(document) => {
            if validate_document(&document).is_err() {
                return ToolResult::error("Motion Studio document response exceeded its bounds");
            }
            ToolResult::ok(
                serde_json::to_string(&document)
                    .unwrap_or_else(|_| "{\"status\":\"unavailable\"}".into()),
            )
        }
        MotionDocumentResponse::Preview(preview) => {
            if validate_hash_value(&preview.revision_hash).is_err()
                || preview.png_base64.is_empty()
                || preview.png_base64.len() > MAX_MOTION_PREVIEW_PNG_BASE64
                || preview.diagnostics.len() > 32
                || preview.diagnostics.iter().any(|diagnostic| {
                    !matches!(diagnostic.severity.as_str(), "error" | "warning" | "info")
                        || diagnostic.message.is_empty()
                        || diagnostic.message.len() > 512
                        || diagnostic.message.chars().any(char::is_control)
                })
            {
                return ToolResult::error("Motion Studio preview response exceeded its bounds");
            }
            ToolResult::blocks(vec![
                Block::text(
                    serde_json::json!({
                        "status": "previewed",
                        "revisionHash": preview.revision_hash,
                        "frame": preview.frame,
                        "diagnostics": preview.diagnostics,
                    })
                    .to_string(),
                ),
                Block::image(preview.png_base64, "image/png"),
            ])
        }
        MotionDocumentResponse::Published(published) => {
            if validate_safe_id(&published.clip_id, "clipId").is_err()
                || validate_safe_id(&published.asset_id, "assetId").is_err()
                || validate_document_id(&published.source_document.document_id).is_err()
                || validate_hash_value(&published.source_document.revision_hash).is_err()
                || !published.duration_seconds.is_finite()
                || !published.fps.is_finite()
            {
                return ToolResult::error("Motion Studio publish response was invalid");
            }
            ToolResult::ok(
                serde_json::json!({
                    "status": "published",
                    "clipId": published.clip_id,
                    "assetId": published.asset_id,
                    "durationFrames": published.duration_frames,
                    "durationSeconds": published.duration_seconds,
                    "fps": published.fps,
                    "width": published.width,
                    "height": published.height,
                    "sourceDocument": published.source_document,
                })
                .to_string(),
            )
        }
    }
}

fn validate_render_bounds(
    width: u32,
    height: u32,
    fps: u32,
    duration_frames: u32,
) -> Result<(), ToolError> {
    if width < 2
        || height < 2
        || width > MAX_MOTION_PREVIEW_DIMENSION
        || height > MAX_MOTION_PREVIEW_DIMENSION
    {
        return Err(ToolError::new("width and height are outside their bounds"));
    }
    if !(1..=120).contains(&fps) {
        return Err(ToolError::new("fps must be between 1 and 120"));
    }
    if !(1..=MAX_MOTION_PREVIEW_FRAMES).contains(&duration_frames) {
        return Err(ToolError::new("durationFrames is outside its bounds"));
    }
    Ok(())
}

fn validate_document(document: &MotionDocument) -> Result<(), ToolError> {
    validate_summary(&document.summary)?;
    if document.html.len() > MAX_MOTION_DOCUMENT_SOURCE_BYTES
        || document.css.len() > MAX_MOTION_DOCUMENT_SOURCE_BYTES
    {
        return Err(ToolError::new("document source exceeds its byte limit"));
    }
    let parameter_bytes = serde_json::to_vec(&document.parameters)
        .map_err(|_| ToolError::new("document parameters are invalid"))?;
    if parameter_bytes.len() > MAX_MOTION_DOCUMENT_SOURCE_BYTES {
        return Err(ToolError::new(
            "document parameters exceed their byte limit",
        ));
    }
    Ok(())
}

fn validate_summary(summary: &MotionDocumentSummary) -> Result<(), ToolError> {
    validate_document_id(&summary.document_id)?;
    validate_title(&summary.title)?;
    validate_hash(&summary.revision_hash, "revisionHash")?;
    if summary.updated_at == 0 {
        return Err(ToolError::new("updatedAt must be positive"));
    }
    Ok(())
}

fn validate_document_id(value: &str) -> Result<(), ToolError> {
    let bytes = value.as_bytes();
    let canonical = bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase(),
        });
    if !canonical {
        return Err(ToolError::new("documentId must be a canonical UUID"));
    }
    Ok(())
}

fn validate_title(value: &str) -> Result<(), ToolError> {
    if value.trim().is_empty()
        || value.chars().count() > MAX_MOTION_DOCUMENT_TITLE_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(ToolError::new("title is invalid"));
    }
    Ok(())
}

fn validate_hash(value: &str, field: &str) -> Result<(), ToolError> {
    validate_hash_value(value).map_err(|_| ToolError::new(format!("{field} is invalid")))
}

fn validate_hash_value(value: &str) -> Result<(), ()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(())
    }
}

fn validate_safe_id(value: &str, field: &str) -> Result<(), ToolError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ToolError::new(format!("{field} is invalid")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "2b9c865b-cd8d-4d8f-b3bb-455cf3bf5c55";
    const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn motion_document_tools_are_typed_and_bounded() {
        const { assert!(MAX_MOTION_DOCUMENT_SOURCE_BYTES <= 1024 * 1024) };
        assert_eq!(MotionDocumentTool::ALL.len(), 6);
    }

    #[test]
    fn patch_requires_hash_and_rejects_path_like_file_names() {
        let missing = decode_request(
            ToolName::PatchMotionDocument,
            &serde_json::json!({
                "documentId": ID,
                "file": "index.html",
                "edits": [{"start": 0, "end": 0, "replacement": "x"}]
            }),
        );
        assert!(missing.is_err());
        for file in ["../index.html", "/tmp/index.html", "link/styles.css"] {
            let result = decode_request(
                ToolName::PatchMotionDocument,
                &serde_json::json!({
                    "documentId": ID,
                    "file": file,
                    "baselineHash": HASH,
                    "edits": [{"start": 0, "end": 0, "replacement": "x"}]
                }),
            );
            assert!(result.is_err(), "accepted {file}");
        }
    }

    #[test]
    fn preview_and_publish_are_strictly_bounded() {
        for (width, height) in [(1, 1080), (1920, 1)] {
            let undersized = decode_request(
                ToolName::PreviewMotionDocument,
                &serde_json::json!({
                    "documentId": ID, "revisionHash": HASH,
                    "width": width, "height": height, "fps": 60,
                    "durationFrames": 120, "frame": 0
                }),
            );
            assert!(undersized.is_err());
        }
        let preview = decode_request(
            ToolName::PreviewMotionDocument,
            &serde_json::json!({
                "documentId": ID, "revisionHash": HASH,
                "width": 8192, "height": 1080, "fps": 60,
                "durationFrames": 120, "frame": 0
            }),
        );
        assert!(preview.is_err());
        let excessive_duration = decode_request(
            ToolName::PreviewMotionDocument,
            &serde_json::json!({
                "documentId": ID, "revisionHash": HASH,
                "width": 1920, "height": 1080, "fps": 60,
                "durationFrames": 3601, "frame": 0
            }),
        );
        assert!(excessive_duration.is_err());
        let publish = decode_request(
            ToolName::PublishMotionDocument,
            &serde_json::json!({
                "documentId": ID, "revisionHash": HASH,
                "width": 1920, "height": 1080, "fps": 60,
                "durationFrames": 120, "startFrame": 0, "clipId": "both"
            }),
        );
        assert!(publish.is_err());

        let transparent = decode_request(
            ToolName::PublishMotionDocument,
            &serde_json::json!({
                "documentId": ID, "revisionHash": HASH,
                "width": 1920, "height": 1080, "fps": 60,
                "durationFrames": 120, "startFrame": 0, "transparent": true
            }),
        )
        .expect("transparent Motion document add is part of the publish contract");
        let MotionDocumentRequest::Publish(transparent) = transparent else {
            panic!("expected publish request");
        };
        assert!(transparent.transparent);

        let transparent_edit = decode_request(
            ToolName::PublishMotionDocument,
            &serde_json::json!({
                "documentId": ID, "revisionHash": HASH,
                "width": 1920, "height": 1080, "fps": 60,
                "durationFrames": 120, "clipId": "clip", "transparent": true
            }),
        );
        assert!(transparent_edit
            .expect_err("alpha edit is not implemented yet")
            .to_string()
            .contains("new Motion clip"));
    }

    #[test]
    fn conflict_is_structured_and_non_error() {
        struct Conflict;
        impl AdmittedMotionDocumentOperation for Conflict {
            fn execute(
                self: Box<Self>,
                _cancel: &opentake_media::MediaCancelToken,
            ) -> Result<MotionDocumentResponse, MotionDocumentBridgeError> {
                Err(MotionDocumentBridgeError::conflict(Some(HASH.into())))
            }
        }
        let result = result_from_operation(
            ToolName::PatchMotionDocument,
            Box::new(Conflict),
            &opentake_media::MediaCancelToken::new(),
        );
        assert!(!result.is_error);
        assert!(result.text_joined().contains("\"status\":\"conflict\""));
        assert!(result.text_joined().contains(HASH));
    }

    #[test]
    fn model_results_have_no_filesystem_path_surface() {
        let response = MotionDocumentResponse::Document(MotionDocument {
            summary: MotionDocumentSummary {
                document_id: ID.into(),
                title: "Title".into(),
                revision_hash: HASH.into(),
                updated_at: 1,
            },
            html: "<main>真实字符</main>".into(),
            css: "main { color: white; }".into(),
            parameters: BTreeMap::new(),
        });
        let text = model_safe_response(response).text_joined();
        assert!(text.contains("\"documentId\""));
        assert!(!text.contains("/tmp"));
        assert!(!text.contains("projectPath"));
        assert!(!text.contains("directory"));
    }
}
