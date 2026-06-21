//! opentake-ops — editing algorithms + the command transaction model.
//!
//! Three layers, all depending only on `opentake-domain`:
//!
//! - **Pure engines** ([`engines`]): [`OverwriteEngine`] (region clearing),
//!   [`RippleEngine`] (shift math), [`SnapEngine`] (drag snapping). Side-effect
//!   free; 1:1 ports of the upstream Swift engines.
//! - **Internal ops** ([`ops`]): place / split / trim / move / ripple / link /
//!   tracks / folders — direct ports of `EditorViewModel` methods, stripped of
//!   AppKit & undo glue, mutating a `Timeline` / `MediaManifest` in place.
//! - **Command layer** ([`command`]): the single editing entry point
//!   [`EditCommand`] + [`apply`], implementing the upstream `withTimelineSwap`
//!   transaction (snapshot -> mutate -> commit-if-changed -> version++) over an
//!   [`EditorState`] with an integral-tree undo/redo stack.
//!
//! Ids for new entities are injected via [`IdGen`] (this leaf crate avoids a
//! `uuid` dependency); tests use the deterministic [`SeqIdGen`].

pub mod command;
pub mod editor_state;
pub mod engines;
pub mod id;
pub mod ops;

// --- Pure engines ---
pub use engines::{
    ClipShift, FrameRange, GapSelection, OverwriteAction, OverwriteEngine, RippleEngine,
    SnapEngine, SnapKind, SnapResult, SnapState, SnapTarget,
};

// --- Command layer ---
pub use command::{
    apply, ClipEntry, ClipProperties, EditCommand, EditError, EditResult, KeyframePayload,
    KeyframeProperty, TextEntry,
};
pub use editor_state::{DocSnapshot, EditorState};
pub use id::{IdGen, SeqIdGen};

// --- Internal ops (re-exported for the core/command-router layer) ---
pub use ops::move_clips::ClipMove;
pub use ops::place::PlaceSpec;
pub use ops::ripple::{RippleOutcome, RippleRangesReport};
pub use ops::trim::{TrimEdge, TrimEdit};
pub use ops::ZoneLayout;
