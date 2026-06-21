//! Internal editing operations — the building blocks [`crate::command`] composes
//! into transactions. Each is a direct port of an `EditorViewModel` method,
//! stripped of AppKit/undo glue: they mutate the `Timeline` (or `MediaManifest`)
//! in place, and the command layer snapshots/commits around them.

pub mod clear_region;
pub mod folders;
pub mod linking;
pub mod move_clips;
pub mod place;
pub mod ripple;
pub mod split;
pub mod tracks;
pub mod trim;

pub use clear_region::clear_region;
pub use folders::{create_folder, move_to_folder};
pub use linking::{
    expand_to_link_group, link_index, linked_partner_ids, partner_moves,
    timing_propagation_partners,
};
pub use move_clips::{move_clips, ClipMove};
pub use place::{place_clip, sort_clips, PlaceSpec};
pub use ripple::{
    apply_shifts, ripple_delete, ripple_delete_ranges_on_track, ripple_insert, validate_shifts,
    RippleOutcome, RippleRangesReport,
};
pub use split::{split_clip, split_single_clip};
pub use tracks::{
    available_audio_track_index, insert_track, prune_empty_tracks, remove_tracks,
    resolve_or_create_audio_track, zones, ZoneLayout,
};
pub use trim::{trim_clip_internal, trim_clips, trim_values, TrimEdge};
