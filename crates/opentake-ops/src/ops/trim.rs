//! Clip trimming. 1:1 port of `trimClipInternal` and the `commitTrim` /
//! `trimValues` helpers from `EditorViewModel+Ripple.swift` / `+Linking.swift`.
//!
//! Overwrite-style: a clip resizes in place — no adjacent-clip shift on the same
//! track, no sync-lock push to other tracks. Incoming trim values are *source*
//! frames; their deltas are translated to timeline frames via `round(delta /
//! speed)` before touching `start_frame` / `duration_frames`.

use opentake_domain::{ClipType, Timeline};

/// Which edge a trim drag grabs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrimEdge {
    Left,
    Right,
}

/// Apply a single clip's trim to new *source-frame* `trim_start` / `trim_end`.
/// No-op if the clip isn't found. 1:1 port of `trimClipInternal`.
pub fn trim_clip_internal(
    timeline: &mut Timeline,
    clip_id: &str,
    trim_start_frame: i32,
    trim_end_frame: i32,
) {
    let Some((ti, ci)) = find(timeline, clip_id) else {
        return;
    };
    let clip = &timeline.tracks[ti].clips[ci];
    let prev_start = clip.trim_start_frame;
    let prev_end = clip.trim_end_frame;
    let prev_duration = clip.duration_frames;
    let speed = clip.speed;

    let delta_start_source = trim_start_frame - prev_start;
    let delta_end_source = trim_end_frame - prev_end;
    let delta_start_timeline = (delta_start_source as f64 / speed).round() as i32;
    let delta_end_timeline = (delta_end_source as f64 / speed).round() as i32;
    let new_duration = prev_duration - delta_start_timeline - delta_end_timeline;
    let new_start_frame = clip.start_frame + delta_start_timeline;

    let c = &mut timeline.tracks[ti].clips[ci];
    c.trim_start_frame = trim_start_frame;
    c.trim_end_frame = trim_end_frame;
    c.start_frame = new_start_frame;
    c.set_duration(new_duration);

    sort_track(timeline, ti);
}

/// A `(clip_id, trim_start, trim_end)` edit, in source frames.
pub type TrimEdit = (String, i32, i32);

/// Apply a batch of trim edits (one undo group upstream; here just sequential).
/// 1:1 port of `trimClips(_:)`.
pub fn trim_clips(timeline: &mut Timeline, edits: &[TrimEdit]) {
    for (id, ts, te) in edits {
        trim_clip_internal(timeline, id, *ts, *te);
    }
}

/// Compute the new source-frame `(trim_start, trim_end)` for an edge drag of
/// `delta` timeline frames. Image/Text clips are unbounded (trims may go
/// negative); video/audio clamp the moved edge at 0. 1:1 port of `trimValues`.
pub fn trim_values(
    media_type: ClipType,
    speed: f64,
    cur_trim_start: i32,
    cur_trim_end: i32,
    edge: TrimEdge,
    delta: i32,
) -> (i32, i32) {
    let source_delta = (delta as f64 * speed).round() as i32;
    let unbounded = media_type == ClipType::Image || media_type == ClipType::Text;
    match edge {
        TrimEdge::Left => {
            let new_start = cur_trim_start + source_delta;
            (
                if unbounded {
                    new_start
                } else {
                    new_start.max(0)
                },
                cur_trim_end,
            )
        }
        TrimEdge::Right => {
            let new_end = cur_trim_end - source_delta;
            (
                cur_trim_start,
                if unbounded { new_end } else { new_end.max(0) },
            )
        }
    }
}

fn find(timeline: &Timeline, clip_id: &str) -> Option<(usize, usize)> {
    for (ti, t) in timeline.tracks.iter().enumerate() {
        if let Some(ci) = t.clips.iter().position(|c| c.id == clip_id) {
            return Some((ti, ci));
        }
    }
    None
}

fn sort_track(timeline: &mut Timeline, ti: usize) {
    timeline.tracks[ti].clips.sort_by_key(|c| c.start_frame);
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, Track};

    fn tl(c: Clip) -> Timeline {
        let mut tl = Timeline::new();
        let mut t = Track::new("v", ClipType::Video);
        t.clips.push(c);
        tl.tracks.push(t);
        tl
    }

    #[test]
    fn trim_start_advances_start_and_shrinks_duration() {
        // clip [100,200) speed 1.0. set trimStart 0->20 -> deltaTimeline 20.
        let mut tl = tl(Clip::new("c", "a", 100, 100));
        trim_clip_internal(&mut tl, "c", 20, 0);
        let c = &tl.tracks[0].clips[0];
        assert_eq!(c.trim_start_frame, 20);
        assert_eq!(c.start_frame, 120);
        assert_eq!(c.duration_frames, 80);
    }

    #[test]
    fn trim_end_shrinks_duration_only() {
        let mut tl = tl(Clip::new("c", "a", 100, 100));
        trim_clip_internal(&mut tl, "c", 0, 30);
        let c = &tl.tracks[0].clips[0];
        assert_eq!(c.trim_end_frame, 30);
        assert_eq!(c.start_frame, 100);
        assert_eq!(c.duration_frames, 70);
    }

    #[test]
    fn trim_translates_source_delta_through_speed() {
        // speed 2.0: source delta 40 -> timeline delta round(40/2)=20.
        let mut c = Clip::new("c", "a", 100, 100);
        c.speed = 2.0;
        let mut tl = tl(c);
        trim_clip_internal(&mut tl, "c", 40, 0);
        let c = &tl.tracks[0].clips[0];
        assert_eq!(c.start_frame, 120);
        assert_eq!(c.duration_frames, 80);
    }

    #[test]
    fn trim_values_left_clamps_for_video() {
        // video, speed 1.0, cur trimStart 5, drag left delta -10 -> -5 clamps to 0.
        let (ts, te) = trim_values(ClipType::Video, 1.0, 5, 0, TrimEdge::Left, -10);
        assert_eq!((ts, te), (0, 0));
    }

    #[test]
    fn trim_values_left_unbounded_for_text() {
        let (ts, _te) = trim_values(ClipType::Text, 1.0, 5, 0, TrimEdge::Left, -10);
        assert_eq!(ts, -5); // text has no source bound
    }

    #[test]
    fn trim_values_right_subtracts_source_delta() {
        // right edge: newEnd = cur_trim_end - round(delta*speed).
        let (ts, te) = trim_values(ClipType::Video, 1.0, 0, 50, TrimEdge::Right, 10);
        assert_eq!((ts, te), (0, 40));
    }
}
