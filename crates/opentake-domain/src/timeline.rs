//! Timeline / Track containers. 1:1 port of `Timeline` and `Track` from
//! upstream `Timeline.swift`.
//!
//! Note on `id` tolerance: upstream synthesizes a fresh UUID when a Track/Clip
//! `id` is missing on decode. To keep this crate a zero-business-dependency leaf
//! (no `uuid`), a missing `id` decodes to an empty string here; the project layer
//! (which owns `uuid`) is responsible for backfilling empty ids after load. All
//! other missing-key fallbacks match upstream exactly.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::clip::Clip;
use crate::clip_type::ClipType;

/// Clip location inside track storage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClipLocation {
    pub track_index: usize,
    pub clip_index: usize,
}

impl ClipLocation {
    pub fn new(track_index: usize, clip_index: usize) -> Self {
        ClipLocation {
            track_index,
            clip_index,
        }
    }
}

fn default_fps() -> i32 {
    30
}
fn default_width() -> i32 {
    1920
}
fn default_height() -> i32 {
    1080
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    #[serde(default = "default_fps")]
    pub fps: i32,
    #[serde(default = "default_width")]
    pub width: i32,
    #[serde(default = "default_height")]
    pub height: i32,
    #[serde(default)]
    pub settings_configured: bool,
    #[serde(default)]
    pub tracks: Vec<Track>,
}

impl Default for Timeline {
    fn default() -> Self {
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: false,
            tracks: Vec::new(),
        }
    }
}

impl Timeline {
    pub fn new() -> Self {
        Timeline::default()
    }

    /// Largest `end_frame` across all tracks (0 when empty).
    pub fn total_frames(&self) -> i32 {
        self.tracks.iter().map(|t| t.end_frame()).max().unwrap_or(0)
    }
}

fn default_sync_locked() -> bool {
    true
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ClipType,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default = "default_sync_locked")]
    pub sync_locked: bool,
    #[serde(default)]
    pub clips: Vec<Clip>,
}

impl Track {
    /// New empty track of `kind`. `id` is caller-supplied (the project layer owns
    /// UUID generation); use `String::new()` for a placeholder if needed.
    pub fn new(id: impl Into<String>, kind: ClipType) -> Self {
        Track {
            id: id.into(),
            kind,
            muted: false,
            hidden: false,
            sync_locked: true,
            clips: Vec::new(),
        }
    }

    /// Largest `end_frame` across this track's clips (0 when empty).
    pub fn end_frame(&self) -> i32 {
        self.clips.iter().map(|c| c.end_frame()).max().unwrap_or(0)
    }

    /// IDs of clips forming a contiguous chain starting at `from_end`, excluding
    /// `exclude_id`. Walks clips sorted by `start_frame`; a clip joins the chain
    /// only when its `start_frame` equals the running chain end.
    pub fn contiguous_clip_ids(&self, from_end: i32, exclude_id: &str) -> HashSet<String> {
        let mut ids = HashSet::new();
        let mut chain_end = from_end;
        let mut sorted: Vec<&Clip> = self.clips.iter().collect();
        sorted.sort_by_key(|c| c.start_frame);
        for c in sorted {
            if c.id == exclude_id || c.start_frame < from_end {
                continue;
            }
            if c.start_frame != chain_end {
                break;
            }
            chain_end = c.end_frame();
            ids.insert(c.id.clone());
        }
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(id: &str, start: i32, dur: i32) -> Clip {
        Clip::new(id, "asset", start, dur)
    }

    #[test]
    fn timeline_defaults() {
        let t = Timeline::default();
        assert_eq!(t.fps, 30);
        assert_eq!(t.width, 1920);
        assert_eq!(t.height, 1080);
        assert!(!t.settings_configured);
        assert!(t.tracks.is_empty());
    }

    #[test]
    fn timeline_total_frames_is_max_track_end() {
        let mut tl = Timeline::new();
        let mut t1 = Track::new("t1", ClipType::Video);
        t1.clips.push(clip("a", 0, 50));
        let mut t2 = Track::new("t2", ClipType::Audio);
        t2.clips.push(clip("b", 10, 120)); // ends at 130
        tl.tracks.push(t1);
        tl.tracks.push(t2);
        assert_eq!(tl.total_frames(), 130);
    }

    #[test]
    fn timeline_total_frames_empty_is_zero() {
        assert_eq!(Timeline::new().total_frames(), 0);
    }

    #[test]
    fn track_end_frame_is_max_clip_end() {
        let mut t = Track::new("t", ClipType::Video);
        assert_eq!(t.end_frame(), 0);
        t.clips.push(clip("a", 0, 30));
        t.clips.push(clip("b", 100, 30)); // ends at 130
        assert_eq!(t.end_frame(), 130);
    }

    #[test]
    fn contiguous_clip_ids_walks_adjacent_chain() {
        let mut t = Track::new("t", ClipType::Video);
        // chain from 0: [0,30) [30,60) then gap, [70,100)
        t.clips.push(clip("a", 0, 30));
        t.clips.push(clip("b", 30, 30));
        t.clips.push(clip("c", 70, 30));
        let ids = t.contiguous_clip_ids(0, "zzz");
        assert!(ids.contains("a"));
        assert!(ids.contains("b"));
        assert!(!ids.contains("c")); // gap breaks the chain
    }

    #[test]
    fn contiguous_clip_ids_excludes_self() {
        let mut t = Track::new("t", ClipType::Video);
        t.clips.push(clip("a", 0, 30));
        t.clips.push(clip("b", 30, 30));
        // exclude "a"; chain starts at 30 -> picks up "b"
        let ids = t.contiguous_clip_ids(30, "a");
        assert!(ids.contains("b"));
        assert!(!ids.contains("a"));
    }

    #[test]
    fn contiguous_clip_ids_breaks_on_first_gap() {
        let mut t = Track::new("t", ClipType::Video);
        // from_end=0 but first clip starts at 5 -> immediate break, empty set
        t.clips.push(clip("a", 5, 30));
        let ids = t.contiguous_clip_ids(0, "zzz");
        assert!(ids.is_empty());
    }

    #[test]
    fn track_decode_defaults_missing_fields() {
        // Only `type` present; id->"", muted/hidden->false, sync_locked->true.
        let json = r#"{"type":"audio"}"#;
        let t: Track = serde_json::from_str(json).unwrap();
        assert_eq!(t.kind, ClipType::Audio);
        assert_eq!(t.id, "");
        assert!(!t.muted);
        assert!(!t.hidden);
        assert!(t.sync_locked);
        assert!(t.clips.is_empty());
    }

    #[test]
    fn track_serializes_type_key() {
        let t = Track::new("t1", ClipType::Video);
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"type\":\"video\""));
        assert!(json.contains("\"syncLocked\":true"));
    }

    #[test]
    fn timeline_serializes_camel_case_settings_key() {
        let mut tl = Timeline::new();
        tl.settings_configured = true;
        let json = serde_json::to_string(&tl).unwrap();
        assert!(json.contains("\"settingsConfigured\":true"));
        // and decodes back from the camelCase key
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert!(back.settings_configured);
    }

    #[test]
    fn timeline_decode_defaults() {
        let tl: Timeline = serde_json::from_str("{}").unwrap();
        assert_eq!(tl.fps, 30);
        assert_eq!(tl.width, 1920);
        assert_eq!(tl.height, 1080);
    }

    #[test]
    fn timeline_roundtrip_json() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        tl.settings_configured = true;
        let mut t = Track::new("t1", ClipType::Video);
        t.clips.push(clip("a", 0, 30));
        tl.tracks.push(t);
        let json = serde_json::to_string(&tl).unwrap();
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(tl, back);
    }

    #[test]
    fn clip_location_fields() {
        let loc = ClipLocation::new(2, 5);
        assert_eq!(loc.track_index, 2);
        assert_eq!(loc.clip_index, 5);
    }
}
