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
