use serde::{Deserialize, Serialize};

/// Visual transition applied at the cut from one clip to its exact adjacent
/// successor. V1 intentionally starts with the lossless baseline required by
/// the product plan; additional shader-backed kinds can extend this enum later.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransitionKind {
    #[default]
    CrossDissolve,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    /// Both sides are persisted so a transition cannot silently rebind when a
    /// project is reordered. Empty is accepted only for legacy project files;
    /// the next validated edit normalizes it to the containing clip id.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub from_clip_id: String,
    pub to_clip_id: String,
    pub kind: TransitionKind,
    pub duration_frames: i32,
}
