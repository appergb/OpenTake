//! Caption-group style sync — pure, immutable operators for batch-applying a
//! single [`TextStyle`] to every clip in a caption group. Part of the
//! ADVANCED-FEATURES.md subtitle slice (see #29). Zero IO: every function takes
//! a `&Timeline` and returns owned values; nothing is mutated in place.
//!
//! Caption-group semantics are the ones established by `subtitle_export.rs`: a
//! clip belongs to a caption group iff its `caption_group_id` is `Some(group)`.
//! Membership is purely the `caption_group_id` match — `text_content` presence
//! is irrelevant here (we are restyling, not exporting), so a caption clip whose
//! text is still empty is still restyled.
//!
//! Conventions carried over from the rest of this crate:
//! - Frames are `i32`; the timeline span is half-open `[start_frame, end_frame)`
//!   (not used directly here, but membership/order never depends on timing).
//! - Values are immutable: [`sync_caption_group_style`] returns a brand-new
//!   `Timeline` (deep clone) with the targeted clips' `text_style` replaced.
//! - Old projects that predate the `text_style` / `caption_group_id` keys decode
//!   with those fields `None` (`#[serde(default)]` on `Clip`); such clips simply
//!   never match a group id, so sync is a safe no-op for them.

use crate::text::TextStyle;
use crate::timeline::Timeline;

/// Every distinct `caption_group_id` present in the timeline, in first-seen
/// track-then-clip order, de-duplicated. Read-only; does not allocate per clip
/// beyond the returned ids.
///
/// Order is deterministic: tracks are visited in storage order, clips within a
/// track in storage order, and each group id is emitted the first time it is
/// seen. This mirrors how the rest of the crate treats track/clip storage order
/// as the canonical traversal.
pub fn caption_group_ids(timeline: &Timeline) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for track in &timeline.tracks {
        for clip in &track.clips {
            if let Some(group) = clip.caption_group_id.as_ref() {
                if !seen.iter().any(|g| g == group) {
                    seen.push(group.clone());
                }
            }
        }
    }
    seen
}

/// Borrowed references to every clip whose `caption_group_id` equals `group_id`,
/// across all tracks, in track-then-clip storage order. Read-only.
///
/// Returns an empty vec when no clip carries that group id (including the case
/// of an empty timeline or `group_id` matching nothing).
pub fn clips_in_group<'a>(timeline: &'a Timeline, group_id: &str) -> Vec<&'a crate::clip::Clip> {
    let mut out: Vec<&crate::clip::Clip> = Vec::new();
    for track in &timeline.tracks {
        for clip in &track.clips {
            if clip.caption_group_id.as_deref() == Some(group_id) {
                out.push(clip);
            }
        }
    }
    out
}

/// Return a new [`Timeline`] in which every clip belonging to caption group
/// `group_id` has its `text_style` set to a clone of `style`. All other clips —
/// and every other field of every clip — are preserved verbatim.
///
/// This is an immutable operator: the input `timeline` is never mutated; the
/// result is a deep clone with only the targeted `text_style` fields rewritten.
///
/// No-op semantics: if `group_id` matches no clip (unknown group, empty
/// timeline, or a legacy project whose clips have no `caption_group_id`), the
/// returned timeline is value-equal to the input. Clips in *other* groups are
/// never touched, so cross-group styles do not bleed.
pub fn sync_caption_group_style(
    timeline: &Timeline,
    group_id: &str,
    style: &TextStyle,
) -> Timeline {
    let mut next = timeline.clone();
    for track in &mut next.tracks {
        for clip in &mut track.clips {
            if clip.caption_group_id.as_deref() == Some(group_id) {
                clip.text_style = Some(style.clone());
            }
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::Clip;
    use crate::clip_type::ClipType;
    use crate::text::{Rgba, TextAlignment, TextStyle};
    use crate::timeline::{Timeline, Track};

    /// Build a caption clip in `group` with a starting `text_style` of `None`.
    fn caption(id: &str, group: &str, start: i32, dur: i32) -> Clip {
        let mut c = Clip::new(id, "caption", start, dur);
        c.media_type = ClipType::Text;
        c.caption_group_id = Some(group.to_string());
        c.text_content = Some(format!("text-{id}"));
        c
    }

    /// A non-default style distinguishable from `TextStyle::default()`.
    fn red_big_style() -> TextStyle {
        TextStyle {
            font_name: "Georgia".to_string(),
            font_size: 42.0,
            color: Rgba::new(1.0, 0.0, 0.0, 1.0),
            alignment: TextAlignment::Left,
            ..Default::default()
        }
    }

    fn one_track_timeline(clips: Vec<Clip>) -> Timeline {
        let mut tl = Timeline::new();
        let mut t = Track::new("t-cap", ClipType::Text);
        t.clips = clips;
        tl.tracks.push(t);
        tl
    }

    // --- empty group / empty timeline (no-op) ---

    #[test]
    fn empty_timeline_is_noop() {
        let tl = Timeline::new();
        let out = sync_caption_group_style(&tl, "g1", &red_big_style());
        assert_eq!(out, tl);
        assert!(caption_group_ids(&tl).is_empty());
        assert!(clips_in_group(&tl, "g1").is_empty());
    }

    #[test]
    fn unknown_group_is_noop() {
        let tl = one_track_timeline(vec![caption("c1", "g1", 0, 30)]);
        let out = sync_caption_group_style(&tl, "does-not-exist", &red_big_style());
        // Value-equal to input: nothing changed.
        assert_eq!(out, tl);
        // And the original clip's style is still None.
        assert!(out.tracks[0].clips[0].text_style.is_none());
    }

    // --- single clip ---

    #[test]
    fn single_clip_gets_style() {
        let tl = one_track_timeline(vec![caption("c1", "g1", 0, 30)]);
        let style = red_big_style();
        let out = sync_caption_group_style(&tl, "g1", &style);
        assert_eq!(out.tracks[0].clips[0].text_style.as_ref(), Some(&style));
        // Input untouched (immutability).
        assert!(tl.tracks[0].clips[0].text_style.is_none());
    }

    #[test]
    fn single_clip_overwrites_preexisting_style() {
        let mut clip = caption("c1", "g1", 0, 30);
        clip.text_style = Some(TextStyle::default());
        let tl = one_track_timeline(vec![clip]);
        let style = red_big_style();
        let out = sync_caption_group_style(&tl, "g1", &style);
        assert_eq!(out.tracks[0].clips[0].text_style.as_ref(), Some(&style));
    }

    // --- multi-track, multiple clips, same group ---

    #[test]
    fn multi_track_same_group_all_restyled() {
        let mut tl = Timeline::new();
        let mut t1 = Track::new("t1", ClipType::Text);
        t1.clips = vec![caption("a", "g1", 0, 30), caption("b", "g1", 30, 30)];
        let mut t2 = Track::new("t2", ClipType::Text);
        t2.clips = vec![caption("c", "g1", 60, 30)];
        tl.tracks.push(t1);
        tl.tracks.push(t2);

        let style = red_big_style();
        let out = sync_caption_group_style(&tl, "g1", &style);

        for track in &out.tracks {
            for clip in &track.clips {
                assert_eq!(clip.text_style.as_ref(), Some(&style));
            }
        }
        assert_eq!(clips_in_group(&out, "g1").len(), 3);
    }

    // --- cross-group isolation (no friendly fire) ---

    #[test]
    fn other_groups_are_not_touched() {
        let tl = one_track_timeline(vec![
            caption("c1", "g1", 0, 30),
            caption("c2", "g2", 30, 30),
            caption("c3", "g1", 60, 30),
        ]);
        let style = red_big_style();
        let out = sync_caption_group_style(&tl, "g1", &style);

        // g1 clips restyled.
        assert_eq!(out.tracks[0].clips[0].text_style.as_ref(), Some(&style));
        assert_eq!(out.tracks[0].clips[2].text_style.as_ref(), Some(&style));
        // g2 clip untouched.
        assert!(out.tracks[0].clips[1].text_style.is_none());
    }

    #[test]
    fn non_caption_clips_are_not_touched() {
        // A plain (non-caption) clip has caption_group_id == None and must never
        // match any group.
        let mut plain = Clip::new("p1", "asset", 0, 30);
        plain.media_type = ClipType::Video;
        let tl = one_track_timeline(vec![plain, caption("c1", "g1", 30, 30)]);
        let style = red_big_style();
        let out = sync_caption_group_style(&tl, "g1", &style);

        assert!(out.tracks[0].clips[0].text_style.is_none());
        assert_eq!(out.tracks[0].clips[1].text_style.as_ref(), Some(&style));
    }

    // --- legacy project safety (fields absent -> None) ---

    #[test]
    fn legacy_clip_without_caption_fields_is_safe_noop() {
        // Simulate a project decoded from old JSON: no caption_group_id, no
        // text_style. Clip::new() already leaves both None.
        let legacy = Clip::new("old", "video.mp4", 0, 120);
        assert!(legacy.caption_group_id.is_none());
        assert!(legacy.text_style.is_none());
        let tl = one_track_timeline(vec![legacy]);
        let out = sync_caption_group_style(&tl, "g1", &red_big_style());
        assert_eq!(out, tl);
        assert!(out.tracks[0].clips[0].text_style.is_none());
    }

    // --- read-only helpers ---

    #[test]
    fn caption_group_ids_are_distinct_and_first_seen_ordered() {
        let mut tl = Timeline::new();
        let mut t1 = Track::new("t1", ClipType::Text);
        t1.clips = vec![
            caption("a", "g2", 0, 30),
            caption("b", "g1", 30, 30),
            caption("c", "g2", 60, 30),
        ];
        let mut t2 = Track::new("t2", ClipType::Text);
        t2.clips = vec![caption("d", "g3", 0, 30), caption("e", "g1", 30, 30)];
        tl.tracks.push(t1);
        tl.tracks.push(t2);

        // First-seen order across tracks then clips: g2, g1, g3 (de-duplicated).
        assert_eq!(caption_group_ids(&tl), vec!["g2", "g1", "g3"]);
    }

    #[test]
    fn clips_in_group_returns_matching_in_storage_order() {
        let tl = one_track_timeline(vec![
            caption("c1", "g1", 0, 30),
            caption("c2", "g2", 30, 30),
            caption("c3", "g1", 60, 30),
        ]);
        let g1 = clips_in_group(&tl, "g1");
        assert_eq!(g1.len(), 2);
        assert_eq!(g1[0].id, "c1");
        assert_eq!(g1[1].id, "c3");
        assert!(clips_in_group(&tl, "g2").len() == 1);
        assert!(clips_in_group(&tl, "nope").is_empty());
    }

    // --- serialization round-trip (project compatibility) ---

    #[test]
    fn synced_timeline_round_trips_through_json() {
        let tl = one_track_timeline(vec![
            caption("c1", "g1", 0, 30),
            caption("c2", "g1", 30, 30),
        ]);
        let style = red_big_style();
        let out = sync_caption_group_style(&tl, "g1", &style);

        let json = serde_json::to_string(&out).expect("serialize");
        let back: Timeline = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, out);
        // Style survived the round trip on both clips.
        assert_eq!(back.tracks[0].clips[0].text_style.as_ref(), Some(&style));
        assert_eq!(back.tracks[0].clips[1].text_style.as_ref(), Some(&style));
    }

    #[test]
    fn legacy_json_without_text_style_decodes_then_syncs() {
        // A minimal project JSON predating text_style/caption keys. It must
        // decode (missing-key tolerant), and our sync must then restyle the
        // caption clip we add into a group.
        let legacy_json = r#"{
            "fps": 30,
            "width": 1920,
            "height": 1080,
            "tracks": [
                { "type": "text", "clips": [
                    { "id": "c1", "mediaRef": "caption", "startFrame": 0,
                      "durationFrames": 30, "captionGroupId": "g1" }
                ] }
            ]
        }"#;
        let tl: Timeline = serde_json::from_str(legacy_json).expect("legacy decode");
        assert!(tl.tracks[0].clips[0].text_style.is_none());
        let style = red_big_style();
        let out = sync_caption_group_style(&tl, "g1", &style);
        assert_eq!(out.tracks[0].clips[0].text_style.as_ref(), Some(&style));
    }
}
