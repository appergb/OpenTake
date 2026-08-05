//! Host boundary for capability-gated vision analysis (subject-aware
//! reframing).
//!
//! The agent owns the `smart_reframe` schema and discovery contract; the
//! desktop host would own frame sampling and saliency/subject analysis. No
//! host implements this backend yet, so `smart_reframe` stays schema-known but
//! is never advertised: discovery remains fail-closed until a production
//! backend exists. A future host attaches a [`VisionBridge`] through
//! `Dispatcher::with_vision_bridge` (the setter keeps every existing bridge
//! constructor source-compatible).

/// Host capability seam for subject-aware reframing. Discovery appends the
/// vision tool set only while an injected bridge reports a usable backend;
/// hosts without one get a fail-closed "vision analysis backend is not
/// available" result instead of a placeholder advertisement.
pub trait VisionBridge: Send + Sync {
    /// True only when the host can sample frames and run subject/saliency
    /// analysis today. `smart_reframe` is discovered only while this holds.
    fn can_reframe(&self) -> bool;
}
