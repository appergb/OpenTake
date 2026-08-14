//! Host boundary for deterministic motion-graphic rendering and placement.
//!
//! The Agent crate owns schemas and discovery, while the desktop host owns the
//! browser/renderer, project filesystem authority, media import, and atomic
//! timeline transaction. Keeping those capabilities behind this trait lets the
//! tool contract run against deterministic fakes without advertising a stub in
//! hosts that do not provide the production bridge.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum MotionSourceRequest {
    Code(String),
    Template {
        template_id: String,
        params: Map<String, Value>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AddMotionRequest {
    pub source: MotionSourceRequest,
    pub start_frame: i32,
    pub duration_frames: i32,
    pub transparent: bool,
    pub track_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditMotionRequest {
    pub clip_id: String,
    pub code: Option<String>,
    pub params: Option<Map<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionCommit {
    pub clip_id: String,
    pub asset_id: String,
    pub content_hash: String,
    pub action_name: String,
    pub output: MotionOutputMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_document: Option<MotionDocumentReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionDocumentReference {
    pub document_id: String,
    pub revision_hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelSafeMotionCommit<'a> {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    clip_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    asset_id: Option<&'a str>,
    duration_frames: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fps: Option<f64>,
    dimensions: ModelSafeMotionDimensions,
}

#[derive(Debug, Serialize)]
struct ModelSafeMotionDimensions {
    width: u32,
    height: u32,
}

/// Rebuild the renderer-owned commit as the minimal model-facing contract.
/// Provenance hashes, renderer versions, filesystem names, and action text stay
/// behind the host boundary.
pub(crate) fn model_safe_commit(commit: &MotionCommit) -> Value {
    let finite_positive = |value: f64| value.is_finite().then_some(value).filter(|v| *v >= 0.0);
    let dto = ModelSafeMotionCommit {
        status: "completed",
        clip_id: safe_motion_id(&commit.clip_id),
        asset_id: safe_motion_id(&commit.asset_id),
        duration_frames: commit.output.duration_frames.max(0),
        duration_seconds: finite_positive(commit.output.duration_seconds),
        fps: finite_positive(commit.output.fps),
        dimensions: ModelSafeMotionDimensions {
            width: commit.output.width,
            height: commit.output.height,
        },
    };
    serde_json::to_value(dto).unwrap_or_else(|_| serde_json::json!({"status": "completed"}))
}

fn safe_motion_id(value: &str) -> Option<&str> {
    (!value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')))
    .then_some(value)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionOutputMetadata {
    pub renderer: String,
    pub renderer_version: String,
    pub output_file: String,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub duration_frames: i32,
    pub duration_seconds: f64,
    pub content_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionBridgeErrorKind {
    InvalidArguments,
    ResourceNotFound,
    CapabilityUnavailable,
    Cancelled,
    RenderFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionBridgeError {
    pub kind: MotionBridgeErrorKind,
    pub message: String,
}

impl MotionBridgeError {
    pub fn new(kind: MotionBridgeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub trait MotionBridge: Send + Sync {
    /// True only when the host has a production renderer and project commit
    /// path. Discovery omits both motion tools when this returns false.
    fn can_render_motion(&self) -> bool;

    fn add(
        &self,
        request: AddMotionRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionCommit, MotionBridgeError>;

    fn edit(
        &self,
        request: EditMotionRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<MotionCommit, MotionBridgeError>;
}
