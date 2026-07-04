//! Pure editing engines: Overwrite (region clearing), Ripple (shift math), and
//! Snap (drag snapping). All side-effect-free; callers apply the results.

pub mod overwrite;
pub mod ripple;
pub mod snap;

pub use overwrite::{OverwriteAction, OverwriteEngine};
pub use ripple::{ClipShift, FrameRange, GapSelection, RippleEngine};
pub use snap::{SnapEngine, SnapKind, SnapResult, SnapState, SnapTarget};
