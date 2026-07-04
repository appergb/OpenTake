//! Timeline export as OpenTimelineIO JSON (`.otio`) — the industry-standard
//! interchange format `otioview`, DaVinci Resolve, and Blender's VSE read.
//!
//! ## Why hand-written JSON
//!
//! There is no maintained, GPL-compatible OpenTimelineIO crate on crates.io, so
//! this module emits the OTIO JSON schema directly via `serde_json` (the project
//! convention: prefer std/serde over heavy deps). The shape is validated against
//! the reference samples in the OpenTimelineIO repo
//! (`tests/sample_data/simple_cut.otio`, `clip_example.otio`): each node carries
//! its exact `"OTIO_SCHEMA"` tag and field set, so a real OTIO reader round-trips
//! it. Keys serialize alphabetically (serde_json's default `Map` ordering) — OTIO
//! readers key by name, not position, so this is conformant and deterministic.
//!
//! ## Structure (OTIO 0.x schema)
//!
//! ```text
//! Timeline.1
//! └── tracks: Stack.1
//!     └── children: [ Track.1 (kind "Video"/"Audio")
//!         └── children: [ Clip.1 | Gap.1 ] ] ]
//! ```
//!
//! - **RationalTime.1** `{ value, rate }` — `value` is an integer frame count,
//!   `rate` is the timeline fps.
//! - **TimeRange.1** `{ start_time: RationalTime, duration: RationalTime }`.
//! - **Clip.1** carries `source_range` (the trimmed source window, in source
//!   frames) and a `media_reference` (**ExternalReference.1** with a `file://`
//!   `target_url` and an `available_range` = the whole source).
//! - **Gap.1** fills the space between clips (and any lead-in before the first
//!   clip) so a track's children tile its timeline span contiguously, which is
//!   how OTIO represents empty time.
//!
//! ## What this preserves vs. drops
//!
//! Preserves: track order and kind (Video/Audio), per-clip timeline placement
//! (via gaps + ordering), the trimmed **source range** (trim + speed-consumed
//! frames), gaps between clips, and a per-clip external media reference with its
//! available range.
//!
//! Drops: transforms / scale / rotation / crop / opacity / volume / keyframes
//! (OTIO models these as `effects`, which this first cut leaves empty), fades,
//! linked-A/V grouping, and text-clip styling. Speed is folded into the source
//! range length (the clip still references the right source window) but no
//! `LinearTimeWarp` effect is emitted.
//!
//! ## Frame fidelity
//!
//! All `RationalTime` values are integer frames at the timeline fps. The clip
//! source window is `[trim_start, trim_start + source_frames_consumed)`; OTIO
//! `start_time` is the source-in frame and `duration` is the consumed length, so
//! the record-side length on the timeline equals the clip's `duration_frames`
//! when speed is 1 (matching OTIO's "trimmed source, placed in order" model).

use std::path::Path;

use opentake_domain::{Clip, ClipType, MediaManifest, MediaResolver, Timeline, Track};
use serde_json::{json, Value};

/// Export a [`Timeline`] as an OpenTimelineIO JSON string (pretty-printed).
/// Pure function: takes the timeline, media manifest, and the project base dir
/// (for resolving `Project`-relative media into `file://` URLs).
pub fn export_otio(
    timeline: &Timeline,
    manifest: &MediaManifest,
    project_base: Option<&Path>,
) -> String {
    let resolver = MediaResolver::new(manifest, project_base);
    let doc = Builder {
        timeline,
        resolver: &resolver,
    }
    .build();
    // Pretty-print: OTIO files are conventionally human-readable & 2-space.
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string())
}

struct Builder<'a> {
    timeline: &'a Timeline,
    resolver: &'a MediaResolver<'a>,
}

impl Builder<'_> {
    fn build(&self) -> Value {
        let fps = self.timeline.fps.max(1);
        let track_nodes: Vec<Value> = self
            .timeline
            .tracks
            .iter()
            .map(|t| self.track_node(t, fps))
            .collect();

        json!({
            "OTIO_SCHEMA": "Timeline.1",
            "metadata": {},
            "name": "Timeline Export",
            "global_start_time": Value::Null,
            "tracks": {
                "OTIO_SCHEMA": "Stack.1",
                "children": track_nodes,
                "effects": [],
                "markers": [],
                "metadata": {},
                "name": "tracks",
                "source_range": Value::Null,
            },
        })
    }

    /// One `Track.1`. Its `children` tile the track span: a lead-in gap (if the
    /// first clip starts after 0), each clip, and a gap between consecutive
    /// clips. Overlapping clips on the same track (which OTIO's single-lane track
    /// cannot represent) are placed back-to-back with no negative gap.
    fn track_node(&self, track: &Track, fps: i32) -> Value {
        let kind = if track.kind == ClipType::Audio {
            "Audio"
        } else {
            "Video"
        };

        let mut clips: Vec<&Clip> = track.clips.iter().collect();
        clips.sort_by_key(|c| c.start_frame);

        let mut children: Vec<Value> = Vec::new();
        let mut cursor = 0i32; // next free timeline frame on this lane
        for clip in clips {
            let start = clip.start_frame.max(0);
            if start > cursor {
                children.push(gap_node(start - cursor, fps));
            }
            children.push(self.clip_node(clip, fps));
            // Advance by the clip's timeline length; never go backwards.
            cursor = cursor.max(clip.end_frame());
        }

        json!({
            "OTIO_SCHEMA": "Track.1",
            "children": children,
            "effects": [],
            "kind": kind,
            "markers": [],
            "metadata": {},
            "name": track.id_or_default(kind),
            "source_range": Value::Null,
        })
    }

    /// One `Clip.1` with a trimmed `source_range` and an `ExternalReference.1`
    /// media reference. The source window is `[trim_start, trim_start +
    /// source_frames_consumed)`; `available_range` spans the whole source.
    fn clip_node(&self, clip: &Clip, fps: i32) -> Value {
        let source_in = clip.trim_start_frame.max(0);
        let consumed = clip.source_frames_consumed().max(0);

        let entry = self.resolver.entry(&clip.media_ref);
        let media_name = self.resolver.display_name(&clip.media_ref);
        // Whole-source length in frames (for available_range). Fall back to the
        // clip's own source span when the manifest has no duration.
        let source_total = entry
            .map(|e| seconds_to_frame(e.duration, fps).max(consumed))
            .unwrap_or_else(|| clip.source_duration_frames().max(consumed));

        json!({
            "OTIO_SCHEMA": "Clip.1",
            "effects": [],
            "enabled": true,
            "markers": [],
            "media_reference": self.media_reference(clip, media_name.as_str(), source_total, fps),
            "metadata": {},
            "name": media_name,
            "source_range": time_range(source_in, consumed, fps),
        })
    }

    /// The clip's `ExternalReference.1`: a `file://` URL to the resolved media
    /// path (or a relative `media/<id>` URL when unresolved) plus the whole
    /// source's `available_range`.
    fn media_reference(&self, clip: &Clip, media_name: &str, source_total: i32, fps: i32) -> Value {
        let target_url = self
            .resolver
            .expected_path(&clip.media_ref)
            .map(|p| path_to_file_url(&p))
            .unwrap_or_else(|| format!("media/{}", clip.media_ref));

        json!({
            "OTIO_SCHEMA": "ExternalReference.1",
            "available_range": time_range(0, source_total, fps),
            "metadata": {},
            "name": media_name,
            "target_url": target_url,
        })
    }
}

/// A `Gap.1` of `frames` length at the timeline fps. Used to space clips out so a
/// track's children tile its span (OTIO has no implicit gaps).
fn gap_node(frames: i32, fps: i32) -> Value {
    json!({
        "OTIO_SCHEMA": "Gap.1",
        "effects": [],
        "enabled": true,
        "markers": [],
        "metadata": {},
        "name": "Gap",
        "source_range": time_range(0, frames.max(0), fps),
    })
}

/// `TimeRange.1 { start_time, duration }` from integer frame counts.
fn time_range(start_frame: i32, duration_frames: i32, fps: i32) -> Value {
    json!({
        "OTIO_SCHEMA": "TimeRange.1",
        "duration": rational_time(duration_frames, fps),
        "start_time": rational_time(start_frame, fps),
    })
}

/// `RationalTime.1 { value, rate }` — integer frame `value` at `rate` fps.
fn rational_time(frame: i32, fps: i32) -> Value {
    json!({
        "OTIO_SCHEMA": "RationalTime.1",
        "rate": fps,
        "value": frame,
    })
}

/// `seconds * fps`, truncated (matches the rest of the export layer / upstream
/// `secondsToFrame`).
fn seconds_to_frame(seconds: f64, fps: i32) -> i32 {
    (seconds * fps as f64) as i32
}

/// Absolute path → an OTIO `file://` URL. OTIO recommends the `file://` scheme
/// for local references; we emit `file://<abs-path>` (POSIX paths already start
/// with `/`, giving the canonical `file:///path`).
fn path_to_file_url(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        // Non-absolute (e.g. Windows or relative): still prefix the scheme.
        format!("file:///{s}")
    }
}

/// Helper to give a track a stable, non-empty name. Domain `Track.id` may be the
/// empty placeholder string (see `timeline.rs`); fall back to the kind.
trait TrackName {
    fn id_or_default(&self, kind: &str) -> String;
}

impl TrackName for Track {
    fn id_or_default(&self, kind: &str) -> String {
        if self.id.is_empty() {
            kind.to_string()
        } else {
            self.id.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{MediaManifestEntry, MediaSource};
    use serde_json::Value;

    fn entry(id: &str, name: &str, kind: ClipType, duration: f64) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.into(),
            name: name.into(),
            kind,
            source: MediaSource::External {
                absolute_path: format!("/media/{name}"),
            },
            duration,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(24.0),
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        }
    }

    fn manifest(entries: Vec<MediaManifestEntry>) -> MediaManifest {
        let mut m = MediaManifest::new();
        m.entries = entries;
        m
    }

    /// Parse the exported string back into JSON so tests assert on structure, not
    /// whitespace.
    fn export_value(tl: &Timeline, m: &MediaManifest) -> Value {
        let s = export_otio(tl, m, None);
        serde_json::from_str(&s).expect("exported OTIO must be valid JSON")
    }

    // --- top-level shape ---

    #[test]
    fn top_level_is_timeline_schema_with_stack_tracks() {
        let tl = Timeline::new();
        let v = export_value(&tl, &manifest(vec![]));
        assert_eq!(v["OTIO_SCHEMA"], "Timeline.1");
        assert_eq!(v["tracks"]["OTIO_SCHEMA"], "Stack.1");
        assert!(v["tracks"]["children"].is_array());
        // global_start_time present and null (matches reference samples).
        assert!(v["global_start_time"].is_null());
    }

    #[test]
    fn empty_timeline_has_no_tracks() {
        let tl = Timeline::new();
        let v = export_value(&tl, &manifest(vec![]));
        assert_eq!(v["tracks"]["children"].as_array().unwrap().len(), 0);
    }

    // --- track kind ---

    #[test]
    fn video_and_audio_tracks_get_correct_kind() {
        let mut tl = Timeline::new();
        tl.tracks.push(Track::new("v", ClipType::Video));
        tl.tracks.push(Track::new("a", ClipType::Audio));
        let v = export_value(&tl, &manifest(vec![]));
        let tracks = v["tracks"]["children"].as_array().unwrap();
        assert_eq!(tracks[0]["OTIO_SCHEMA"], "Track.1");
        assert_eq!(tracks[0]["kind"], "Video");
        assert_eq!(tracks[1]["kind"], "Audio");
    }

    #[test]
    fn image_and_text_tracks_are_video_kind() {
        let mut tl = Timeline::new();
        tl.tracks.push(Track::new("i", ClipType::Image));
        tl.tracks.push(Track::new("t", ClipType::Text));
        let v = export_value(&tl, &manifest(vec![]));
        let tracks = v["tracks"]["children"].as_array().unwrap();
        assert_eq!(tracks[0]["kind"], "Video");
        assert_eq!(tracks[1]["kind"], "Video");
    }

    // --- clip / source range ---

    #[test]
    fn clip_has_source_range_and_external_reference() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        vt.clips.push(Clip::new("c1", "v1", 0, 48));
        tl.tracks.push(vt);
        let v = export_value(
            &tl,
            &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        );
        let clip = &v["tracks"]["children"][0]["children"][0];
        assert_eq!(clip["OTIO_SCHEMA"], "Clip.1");
        assert_eq!(clip["name"], "shot.mp4");
        // source_range: TimeRange with RationalTime start/duration.
        let sr = &clip["source_range"];
        assert_eq!(sr["OTIO_SCHEMA"], "TimeRange.1");
        assert_eq!(sr["start_time"]["OTIO_SCHEMA"], "RationalTime.1");
        assert_eq!(sr["start_time"]["value"], 0);
        assert_eq!(sr["start_time"]["rate"], 24);
        assert_eq!(sr["duration"]["value"], 48);
        // media_reference: ExternalReference with file:// url + available_range.
        let mr = &clip["media_reference"];
        assert_eq!(mr["OTIO_SCHEMA"], "ExternalReference.1");
        assert_eq!(mr["target_url"], "file:///media/shot.mp4");
        assert_eq!(mr["available_range"]["OTIO_SCHEMA"], "TimeRange.1");
        // available range duration = 4s * 24 = 96.
        assert_eq!(mr["available_range"]["duration"]["value"], 96);
    }

    #[test]
    fn trim_offsets_source_start_time() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        let mut clip = Clip::new("c1", "v1", 0, 24);
        clip.trim_start_frame = 12;
        vt.clips.push(clip);
        tl.tracks.push(vt);
        let v = export_value(
            &tl,
            &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        );
        let sr = &v["tracks"]["children"][0]["children"][0]["source_range"];
        assert_eq!(sr["start_time"]["value"], 12);
        assert_eq!(sr["duration"]["value"], 24);
    }

    #[test]
    fn speed_folds_into_consumed_source_duration() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        let mut clip = Clip::new("c1", "v1", 0, 24); // 1s timeline
        clip.speed = 2.0; // consumes 48 source frames
        vt.clips.push(clip);
        tl.tracks.push(vt);
        let v = export_value(
            &tl,
            &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        );
        let sr = &v["tracks"]["children"][0]["children"][0]["source_range"];
        assert_eq!(sr["duration"]["value"], 48);
    }

    // --- gaps ---

    #[test]
    fn lead_in_gap_before_first_clip() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        vt.clips.push(Clip::new("c1", "v1", 24, 24)); // starts at frame 24
        tl.tracks.push(vt);
        let v = export_value(
            &tl,
            &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        );
        let children = v["tracks"]["children"][0]["children"].as_array().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["OTIO_SCHEMA"], "Gap.1");
        assert_eq!(children[0]["source_range"]["duration"]["value"], 24);
        assert_eq!(children[1]["OTIO_SCHEMA"], "Clip.1");
    }

    #[test]
    fn gap_between_clips() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        vt.clips.push(Clip::new("c1", "v1", 0, 24)); // [0,24)
        vt.clips.push(Clip::new("c2", "v2", 48, 24)); // gap [24,48), then clip
        tl.tracks.push(vt);
        let v = export_value(
            &tl,
            &manifest(vec![
                entry("v1", "a.mp4", ClipType::Video, 4.0),
                entry("v2", "b.mp4", ClipType::Video, 4.0),
            ]),
        );
        let children = v["tracks"]["children"][0]["children"].as_array().unwrap();
        // clip, gap(24), clip
        assert_eq!(children.len(), 3);
        assert_eq!(children[0]["OTIO_SCHEMA"], "Clip.1");
        assert_eq!(children[1]["OTIO_SCHEMA"], "Gap.1");
        assert_eq!(children[1]["source_range"]["duration"]["value"], 24);
        assert_eq!(children[2]["OTIO_SCHEMA"], "Clip.1");
    }

    #[test]
    fn adjacent_clips_have_no_gap() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        vt.clips.push(Clip::new("c1", "v1", 0, 24));
        vt.clips.push(Clip::new("c2", "v2", 24, 24)); // contiguous
        tl.tracks.push(vt);
        let v = export_value(
            &tl,
            &manifest(vec![
                entry("v1", "a.mp4", ClipType::Video, 4.0),
                entry("v2", "b.mp4", ClipType::Video, 4.0),
            ]),
        );
        let children = v["tracks"]["children"][0]["children"].as_array().unwrap();
        assert_eq!(children.len(), 2);
        assert!(children.iter().all(|c| c["OTIO_SCHEMA"] == "Clip.1"));
    }

    #[test]
    fn overlapping_clips_placed_back_to_back_no_negative_gap() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        vt.clips.push(Clip::new("c1", "v1", 0, 48)); // [0,48)
        vt.clips.push(Clip::new("c2", "v2", 24, 48)); // overlaps; starts at 24
        tl.tracks.push(vt);
        let v = export_value(
            &tl,
            &manifest(vec![
                entry("v1", "a.mp4", ClipType::Video, 4.0),
                entry("v2", "b.mp4", ClipType::Video, 4.0),
            ]),
        );
        let children = v["tracks"]["children"][0]["children"].as_array().unwrap();
        // No gap inserted (would be negative); both clips present.
        assert_eq!(children.len(), 2);
        assert!(children.iter().all(|c| c["OTIO_SCHEMA"] == "Clip.1"));
    }

    // --- unresolved media ---

    #[test]
    fn unresolved_media_uses_relative_target_url() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        vt.clips.push(Clip::new("c1", "ghost", 0, 24));
        tl.tracks.push(vt);
        let v = export_value(&tl, &manifest(vec![]));
        let mr = &v["tracks"]["children"][0]["children"][0]["media_reference"];
        assert_eq!(mr["target_url"], "media/ghost");
        // name falls back to "Offline".
        assert_eq!(v["tracks"]["children"][0]["children"][0]["name"], "Offline");
    }

    // --- file url ---

    #[test]
    fn file_url_for_absolute_path() {
        assert_eq!(
            path_to_file_url(Path::new("/abs/clip.mov")),
            "file:///abs/clip.mov"
        );
    }

    // --- rational time / time range ---

    #[test]
    fn rational_time_carries_value_and_rate() {
        let rt = rational_time(42, 30);
        assert_eq!(rt["OTIO_SCHEMA"], "RationalTime.1");
        assert_eq!(rt["value"], 42);
        assert_eq!(rt["rate"], 30);
    }

    #[test]
    fn fps_zero_floored_to_one() {
        let mut tl = Timeline::new();
        tl.fps = 0;
        tl.tracks.push(Track::new("v", ClipType::Video));
        let v = export_value(&tl, &manifest(vec![]));
        // No panic; track present.
        assert_eq!(v["tracks"]["children"].as_array().unwrap().len(), 1);
    }
}
