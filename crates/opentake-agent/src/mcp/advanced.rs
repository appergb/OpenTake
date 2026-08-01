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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvancedWorkflowErrorKind {
    InvalidArguments,
    ResourceNotFound,
    CapabilityUnavailable,
    ConsentRequired,
    CostAuthorizationRequired,
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
