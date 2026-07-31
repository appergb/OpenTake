//! Timeline / Track containers. 1:1 port of `Timeline` and `Track` from
//! upstream `Timeline.swift`.
//!
//! Note on `id` tolerance: domain decoding is deterministic and maps a missing
//! or malformed Track/Clip `id` to an empty placeholder. The project persistence
//! boundary owns UUID repair because it retains the raw JSON needed to
//! distinguish that placeholder from an explicitly encoded empty string.

use std::collections::{HashMap, HashSet};

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
    /// Editable child timelines referenced by clips through
    /// `Clip::nested_sequence_id`. The registry lives on the root timeline so
    /// every reference has one stable identity and graph cycles are detectable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nested_sequences: Vec<NestedSequence>,
    #[serde(default)]
    pub tracks: Vec<Track>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NestedSequence {
    pub id: String,
    pub name: String,
    pub timeline: Timeline,
}

impl NestedSequence {
    pub const WIRE_FIELDS: &'static [&'static str] = &["id", "name", "timeline"];
    pub const ID_WIRE_FIELD: &'static str = "id";
    pub const TIMELINE_WIRE_FIELD: &'static str = "timeline";

    pub fn new(id: impl Into<String>, name: impl Into<String>, timeline: Timeline) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            timeline,
        }
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: false,
            nested_sequences: Vec::new(),
            tracks: Vec::new(),
        }
    }
}

impl Timeline {
    pub const TRACKS_WIRE_FIELD: &'static str = "tracks";
    pub const NESTED_SEQUENCES_WIRE_FIELD: &'static str = "nestedSequences";

    pub fn new() -> Self {
        Timeline::default()
    }

    /// Largest `end_frame` across all tracks (0 when empty).
    pub fn total_frames(&self) -> i32 {
        self.tracks.iter().map(|t| t.end_frame()).max().unwrap_or(0)
    }

    /// Validate unique sequence identities, every reference, and graph cycles.
    /// This is a pure preflight used before edits are committed or plans built.
    pub fn validate_nested_sequences(&self) -> Result<(), String> {
        let mut registry = HashMap::new();
        for sequence in &self.nested_sequences {
            if sequence.id.is_empty() {
                return Err("nested sequence id must not be empty".to_string());
            }
            if registry.insert(sequence.id.as_str(), sequence).is_some() {
                return Err(format!("duplicate nested sequence id: {}", sequence.id));
            }
            if !sequence.timeline.nested_sequences.is_empty() {
                return Err(format!(
                    "nested sequence {} contains a nestedSequences registry; child references must use the root registry",
                    sequence.id
                ));
            }
        }

        // Several cross-cutting consumers (text resolution, selection, and
        // edit commands) address clips by id without a sequence namespace.
        // Once a project has nested timelines, ids therefore must be unique
        // across the entire stored graph rather than only inside one track.
        let uses_nested_graph = !self.nested_sequences.is_empty()
            || self
                .tracks
                .iter()
                .flat_map(|track| &track.clips)
                .any(|clip| clip.nested_sequence_id.is_some());
        if uses_nested_graph {
            let mut clip_ids = HashSet::new();
            for timeline in std::iter::once(self).chain(
                self.nested_sequences
                    .iter()
                    .map(|sequence| &sequence.timeline),
            ) {
                for track in &timeline.tracks {
                    for clip in &track.clips {
                        if clip.id.is_empty() {
                            return Err(
                                "clip id must not be empty in a nested timeline graph".to_string()
                            );
                        }
                        if !clip_ids.insert(clip.id.as_str()) {
                            return Err(format!(
                                "duplicate clip id in nested timeline graph: {}",
                                clip.id
                            ));
                        }
                        if clip.nested_sequence_id.is_some()
                            && (track.kind == ClipType::Audio
                                || clip.media_type != ClipType::Video
                                || !clip.media_ref.is_empty()
                                || clip.start_frame < 0
                                || clip.duration_frames < 1
                                || clip.trim_start_frame < 0)
                        {
                            return Err(format!(
                                "invalid compound clip representation: {}",
                                clip.id
                            ));
                        }
                    }
                }
            }
        }

        fn references(timeline: &Timeline) -> impl Iterator<Item = &str> {
            timeline.tracks.iter().flat_map(|track| {
                track
                    .clips
                    .iter()
                    .filter_map(|clip| clip.nested_sequence_id.as_deref())
            })
        }

        for reference in references(self) {
            if !registry.contains_key(reference) {
                return Err(format!("missing nested sequence reference: {reference}"));
            }
        }

        fn visit<'a>(
            id: &'a str,
            registry: &HashMap<&'a str, &'a NestedSequence>,
            visiting: &mut Vec<&'a str>,
            complete: &mut HashSet<&'a str>,
        ) -> Result<(), String> {
            if complete.contains(id) {
                return Ok(());
            }
            if let Some(index) = visiting.iter().position(|candidate| *candidate == id) {
                let mut cycle = visiting[index..].to_vec();
                cycle.push(id);
                return Err(format!("nested sequence cycle: {}", cycle.join(" -> ")));
            }
            let sequence = registry
                .get(id)
                .ok_or_else(|| format!("missing nested sequence reference: {id}"))?;
            visiting.push(id);
            for reference in references(&sequence.timeline) {
                visit(reference, registry, visiting, complete)?;
            }
            visiting.pop();
            complete.insert(id);
            Ok(())
        }

        let mut complete = HashSet::new();
        for sequence in &self.nested_sequences {
            visit(&sequence.id, &registry, &mut Vec::new(), &mut complete)?;
        }
        Ok(())
    }
}

fn default_sync_locked() -> bool {
    true
}

/// Match Track's upstream `(try? decode(...)) ?? default` behavior for its
/// tolerant fields, including present values of the wrong JSON type.
fn deserialize_default_on_error<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    deserialize_or(deserializer, T::default())
}

fn deserialize_or<'de, D, T>(deserializer: D, fallback: T) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    #[allow(dead_code)]
    enum Tolerant<T> {
        Value(T),
        Invalid(serde::de::IgnoredAny),
    }

    Ok(match Tolerant::deserialize(deserializer)? {
        Tolerant::Value(value) => value,
        Tolerant::Invalid(_) => fallback,
    })
}

fn deserialize_true_on_error<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, true)
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    #[serde(default, deserialize_with = "deserialize_default_on_error")]
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ClipType,
    #[serde(default, deserialize_with = "deserialize_default_on_error")]
    pub muted: bool,
    #[serde(default, deserialize_with = "deserialize_default_on_error")]
    pub hidden: bool,
    #[serde(
        default = "default_sync_locked",
        deserialize_with = "deserialize_true_on_error"
    )]
    pub sync_locked: bool,
    #[serde(default)]
    pub clips: Vec<Clip>,
}

impl Track {
    /// Persisted keys owned by Track's wire schema. Compatibility scanning
    /// consumes this metadata instead of duplicating field names downstream.
    pub const WIRE_FIELDS: &'static [&'static str] =
        &["id", "type", "muted", "hidden", "syncLocked", "clips"];
    pub const TOLERANT_SCALAR_WIRE_FIELDS: &'static [&'static str] =
        &["id", "muted", "hidden", "syncLocked"];
    pub const ID_WIRE_FIELD: &'static str = "id";
    pub const CLIPS_WIRE_FIELD: &'static str = "clips";

    /// New empty track of `kind`. The caller owns identity for new edits; the
    /// project persistence boundary repairs missing/malformed legacy IDs.
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
        // Only `type` present; id deterministically defaults to an empty
        // placeholder, muted/hidden->false, sync_locked->true.
        let json = r#"{"type":"audio"}"#;
        let t: Track = serde_json::from_str(json).unwrap();
        assert_eq!(t.kind, ClipType::Audio);
        assert_eq!(t.id, "");
        assert!(!t.muted);
        assert!(!t.hidden);
        assert!(t.sync_locked);
        assert!(t.clips.is_empty());

        for id in ["null", "7", "false", "{}", "[]"] {
            let malformed: Track =
                serde_json::from_str(&format!(r#"{{"id":{id},"type":"audio"}}"#)).unwrap();
            assert_eq!(malformed.id, "", "id shape {id}");
        }
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
    fn nested_sequence_roundtrip_and_legacy_omission_are_stable() {
        let legacy: Timeline = serde_json::from_str(r#"{"fps":24,"tracks":[]}"#).unwrap();
        assert!(legacy.nested_sequences.is_empty());
        assert!(!serde_json::to_string(&legacy)
            .unwrap()
            .contains("nestedSequences"));

        let mut child = Timeline::new();
        child
            .tracks
            .push(Track::new("child-track", ClipType::Video));
        let mut root = Timeline::new();
        root.nested_sequences
            .push(NestedSequence::new("sequence-a", "Scene A", child));
        let encoded = serde_json::to_string(&root).unwrap();
        assert!(encoded.contains("\"nestedSequences\""));
        assert_eq!(serde_json::from_str::<Timeline>(&encoded).unwrap(), root);
    }

    #[test]
    fn nested_sequence_validation_is_deterministic_and_fail_closed() {
        let mut root = Timeline::new();
        root.nested_sequences
            .push(NestedSequence::new("", "Empty", Timeline::new()));
        assert_eq!(
            root.validate_nested_sequences().unwrap_err(),
            "nested sequence id must not be empty"
        );

        root.nested_sequences = vec![
            NestedSequence::new("duplicate", "A", Timeline::new()),
            NestedSequence::new("duplicate", "B", Timeline::new()),
        ];
        assert_eq!(
            root.validate_nested_sequences().unwrap_err(),
            "duplicate nested sequence id: duplicate"
        );

        let mut missing_track = Track::new("root-track", ClipType::Video);
        missing_track
            .clips
            .push(Clip::new_nested("compound", "missing", 0, 10));
        root.nested_sequences.clear();
        root.tracks = vec![missing_track];
        assert_eq!(
            root.validate_nested_sequences().unwrap_err(),
            "missing nested sequence reference: missing"
        );

        let mut a = Timeline::new();
        let mut a_track = Track::new("a-track", ClipType::Video);
        a_track.clips.push(Clip::new_nested("a-to-b", "b", 0, 10));
        a.tracks.push(a_track);
        let mut b = Timeline::new();
        let mut b_track = Track::new("b-track", ClipType::Video);
        b_track.clips.push(Clip::new_nested("b-to-a", "a", 0, 10));
        b.tracks.push(b_track);
        root.tracks.clear();
        root.nested_sequences = vec![
            NestedSequence::new("a", "A", a),
            NestedSequence::new("b", "B", b),
        ];
        assert_eq!(
            root.validate_nested_sequences().unwrap_err(),
            "nested sequence cycle: a -> b -> a"
        );
    }

    #[test]
    fn nested_sequence_validation_rejects_graph_wide_clip_id_collisions() {
        let mut child = Timeline::new();
        let mut child_track = Track::new("child-track", ClipType::Video);
        child_track.clips.push(clip("shared-id", 0, 10));
        child.tracks.push(child_track);

        let mut root = Timeline::new();
        let mut root_track = Track::new("root-track", ClipType::Video);
        root_track.clips.push(clip("shared-id", 0, 10));
        root_track
            .clips
            .push(Clip::new_nested("compound", "sequence", 10, 10));
        root.tracks.push(root_track);
        root.nested_sequences
            .push(NestedSequence::new("sequence", "Scene", child));

        assert_eq!(
            root.validate_nested_sequences().unwrap_err(),
            "duplicate clip id in nested timeline graph: shared-id"
        );
    }

    #[test]
    fn nested_sequence_validation_rejects_compound_on_audio_track() {
        let mut root = Timeline::new();
        let mut track = Track::new("audio-track", ClipType::Audio);
        track
            .clips
            .push(Clip::new_nested("compound", "sequence", 0, 10));
        root.tracks.push(track);
        root.nested_sequences
            .push(NestedSequence::new("sequence", "Scene", Timeline::new()));

        assert_eq!(
            root.validate_nested_sequences().unwrap_err(),
            "invalid compound clip representation: compound"
        );
    }

    #[test]
    fn clip_location_fields() {
        let loc = ClipLocation::new(2, 5);
        assert_eq!(loc.track_index, 2);
        assert_eq!(loc.clip_index, 5);
    }
}
