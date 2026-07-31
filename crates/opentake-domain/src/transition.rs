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
    /// Pair identity prevents a transition from silently binding to a different
    /// neighbor after timeline edits.
    pub to_clip_id: String,
    pub kind: TransitionKind,
    pub duration_frames: i32,
}
