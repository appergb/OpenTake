//! Post-edit **timeline** transcript assembly — the pure word→project-frame
//! mapping + paging behind the `get_transcript` MCP tool. Verbatim port of
//! `Agent/Tools/ToolExecutor+Timeline.swift`'s `getTranscript` body +
//! `spanFrames` (`:548-651`).
//!
//! `get_transcript` walks every audio/video clip on the timeline, maps each
//! transcript word through the clip's trim/speed/position into PROJECT frames,
//! and concatenates in timeline order. Every unit here is pure and unit-tested
//! (trim/speed/window/paging edge cases); the actual transcription (whisper +
//! cache) is injected by the caller as a resolved `TranscriptionResult` per
//! source, so this module never touches ffmpeg or a model.
//!
//! **Time / rounding contract** (SPEC §0.1 + 移植铁律): source seconds → source
//! frames uses `seconds * fps`; source frame → timeline frame is
//! `round(startFrame + (sourceFrame - visStart) / max(speed, 0.0001))`, where
//! `round` is `f64::round` (Swift `.rounded()` = round-half-away-from-zero,
//! identical to Rust's). Frames here are non-negative, so the tie direction is
//! moot in practice, but the formula is kept exact for a 1:1 cache/behavior match.

use opentake_domain::Clip;

use super::TranscriptionResult;

/// Total-word cap across all clips in one `get_transcript` response (upstream
/// `inspectMaxWords`). Rows past the cap are dropped and the caller pages with
/// `startFrame`/`endFrame` using the returned `next_start_frame`.
pub const TIMELINE_MAX_WORDS: usize = 10_000;

/// One `[text, startFrame, endFrame]` row: a single word mapped to project
/// frames. `text` carries the backend's casing/punctuation for that token.
#[derive(Clone, Debug, PartialEq)]
pub struct WordRow {
    /// The word/token text.
    pub text: String,
    /// Project frame the word starts on (inclusive).
    pub start_frame: i32,
    /// Project frame the word ends on (`>= start_frame`).
    pub end_frame: i32,
}

/// One clip's contribution to the timeline transcript: its identity + the word
/// rows that fell inside its visible span (already mapped to project frames,
/// sorted, and window-filtered). Mirrors one entry of upstream's `clips` array.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipTranscript {
    /// The clip id (pass straight to `ripple_delete_ranges`).
    pub clip_id: String,
    /// 0-based track index the clip lives on.
    pub track_index: usize,
    /// Clip start on the timeline, in project frames.
    pub start_frame: i32,
    /// Clip end on the timeline (`start_frame + duration_frames`).
    pub end_frame: i32,
    /// The word rows attributed to this clip, in `(start, end)` order. Truncated
    /// to keep the whole response at [`TIMELINE_MAX_WORDS`].
    pub words: Vec<WordRow>,
}

/// One clip + its resolved source transcript, the input to [`timeline_transcript`].
/// The caller (the MCP bridge) has already: filtered to caption-eligible clips,
/// resolved each clip's source, transcribed it (cached), and located its track.
pub struct ClipFragment<'a> {
    /// The clip id.
    pub clip_id: String,
    /// 0-based track index the clip lives on.
    pub track_index: usize,
    /// The clip geometry (start/trim/duration/speed) driving the frame mapping.
    pub clip: &'a Clip,
    /// The source asset's full transcript (source-seconds timings). `None` when
    /// that source failed to transcribe — the clip is skipped, not an error.
    pub transcript: Option<&'a TranscriptionResult>,
}

/// The assembled timeline transcript: the per-clip rows plus paging state.
/// Serialization (the `{fps, timing, wordFormat, clips, …}` envelope) lives with
/// the caller so this stays a pure value type.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineTranscript {
    /// Clips (in timeline order) that contributed at least one word.
    pub clips: Vec<ClipTranscript>,
    /// Total words that matched across ALL clips *before* the cap — echoed as
    /// `totalWords` only when it exceeds [`TIMELINE_MAX_WORDS`].
    pub total_words: usize,
    /// The next page's `startFrame` (the last emitted word's end frame) when the
    /// response was truncated by the cap; `None` when everything fit.
    pub next_start_frame: Option<i32>,
}

/// The clip's visible source-frame window `[vis_start, vis_end)`:
/// `vis_start = trim_start_frame`, `vis_end = vis_start + duration * max(speed, ε)`.
/// Kept public for the caller's per-clip midpoint pre-filter (upstream inlines
/// the same two lines before calling `spanFrames`).
fn visible_source_span(clip: &Clip) -> (f64, f64) {
    let vis_start = clip.trim_start_frame as f64;
    let vis_end = vis_start + clip.duration_frames as f64 * clip.speed.max(SPEED_FLOOR);
    (vis_start, vis_end)
}

/// Lower bound on `speed` in the frame math, matching upstream `max(speed, 0.0001)`
/// — guards divide-by-zero for a (degenerate) zero-speed clip.
const SPEED_FLOOR: f64 = 0.0001;

/// Map one word's source-seconds span `[start, end]` to project frames for `clip`,
/// clamped to the clip's visible window first so a boundary-straddler yields its
/// real sliver, not a fabricated full-clip span. `None` when the word is not
/// visible in this clip. Verbatim port of `spanFrames` (`:643-651`).
pub fn span_frames(start: f64, end: f64, clip: &Clip, fps: i32) -> Option<(i32, i32)> {
    let fps_d = fps as f64;
    let (vis_start, vis_end) = visible_source_span(clip);
    let s = (start * fps_d).max(vis_start);
    let e = (end * fps_d).min(vis_end);
    if e <= s {
        return None;
    }
    let to_timeline = |source_frame: f64| -> i32 {
        (clip.start_frame as f64 + (source_frame - vis_start) / clip.speed.max(SPEED_FLOOR)).round()
            as i32
    };
    let a = to_timeline(s);
    Some((a, a.max(to_timeline(e))))
}

/// Assemble the live timeline transcript from per-clip fragments. Clips are
/// processed in `clip.start_frame` order; each word is assigned to the clip whose
/// visible span contains its **midpoint** (so a word split across a seam is
/// emitted once), mapped via [`span_frames`], filtered to the optional
/// `[window_start, window_end)` project-frame window, sorted, and truncated at
/// [`TIMELINE_MAX_WORDS`] across the whole response. Verbatim port of the
/// `getTranscript` body (`:583-616`).
///
/// `window_start` drops words ending at/before it; `window_end` drops words
/// starting at/after it (both project frames). Paging: when `total_words`
/// exceeds the cap, `next_start_frame` is the last emitted word's end frame.
pub fn timeline_transcript(
    mut frags: Vec<ClipFragment<'_>>,
    fps: i32,
    window_start: Option<i32>,
    window_end: Option<i32>,
) -> TimelineTranscript {
    // Timeline order (upstream `frags.sorted(by: startFrame)`).
    frags.sort_by_key(|f| f.clip.start_frame);

    let mut clips_out: Vec<ClipTranscript> = Vec::new();
    let mut total_words = 0usize;
    let mut remaining = TIMELINE_MAX_WORDS;
    let mut last_end: Option<i32> = None;

    for frag in &frags {
        let Some(transcript) = frag.transcript else {
            continue;
        };
        let (vis_start, vis_end) = visible_source_span(frag.clip);

        let mut rows: Vec<WordRow> = Vec::new();
        for w in &transcript.words {
            let (Some(s), Some(e)) = (w.start, w.end) else {
                continue;
            };
            // Assign a word to the clip whose visible range contains its midpoint.
            let mid_frame = (s + e) / 2.0 * fps as f64;
            if mid_frame < vis_start || mid_frame >= vis_end {
                continue;
            }
            let Some((fs, fe)) = span_frames(s, e, frag.clip, fps) else {
                continue;
            };
            if window_start.is_some_and(|ws| fe <= ws) {
                continue;
            }
            if window_end.is_some_and(|we| fs >= we) {
                continue;
            }
            rows.push(WordRow {
                text: w.text.clone(),
                start_frame: fs,
                end_frame: fe,
            });
        }
        rows.sort_by_key(|r| (r.start_frame, r.end_frame));
        if rows.is_empty() {
            continue;
        }
        total_words += rows.len();
        if remaining == 0 {
            continue;
        }
        let take = remaining.min(rows.len());
        let slice: Vec<WordRow> = rows.into_iter().take(take).collect();
        remaining -= take;
        if let Some(last) = slice.last() {
            last_end = Some(last.end_frame);
        }
        clips_out.push(ClipTranscript {
            clip_id: frag.clip_id.clone(),
            track_index: frag.track_index,
            start_frame: frag.clip.start_frame,
            end_frame: frag.clip.end_frame(),
            words: slice,
        });
    }

    let next_start_frame = if total_words > TIMELINE_MAX_WORDS {
        last_end
    } else {
        None
    };
    TimelineTranscript {
        clips: clips_out,
        total_words,
        next_start_frame,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcribe::{TranscriptionResult, TranscriptionWord};

    /// Build a video clip at `start`, `duration` frames, given `trim_start` and
    /// `speed`. Other fields are defaults (`Clip::new` defaults to video).
    fn clip(id: &str, start: i32, duration: i32, trim_start: i32, speed: f64) -> Clip {
        let mut c = Clip::new(id, "media", start, duration);
        c.trim_start_frame = trim_start;
        c.speed = speed;
        c
    }

    fn word(text: &str, start: f64, end: f64) -> TranscriptionWord {
        TranscriptionWord {
            text: text.into(),
            start: Some(start),
            end: Some(end),
        }
    }

    fn result(words: Vec<TranscriptionWord>) -> TranscriptionResult {
        TranscriptionResult {
            text: String::new(),
            language: Some("en".into()),
            words,
            segments: vec![],
        }
    }

    // --- span_frames -------------------------------------------------------

    #[test]
    fn span_frames_identity_clip_maps_seconds_to_frames() {
        // clip at frame 0, no trim, speed 1, 30 fps. Word 1.0..2.0s → 30..60.
        let c = clip("c", 0, 300, 0, 1.0);
        assert_eq!(span_frames(1.0, 2.0, &c, 30), Some((30, 60)));
    }

    #[test]
    fn span_frames_offsets_by_clip_start_and_trim() {
        // clip starts at timeline frame 100, trims first 30 source frames.
        // Word at source 1.0..1.5s = 30..45 source frames; visible from 30.
        // timeline = 100 + (30 - 30)/1 = 100 .. 100 + (45 - 30)/1 = 115.
        let c = clip("c", 100, 300, 30, 1.0);
        assert_eq!(span_frames(1.0, 1.5, &c, 30), Some((100, 115)));
    }

    #[test]
    fn span_frames_speed_compresses_timeline_span() {
        // speed 2 → source advances twice as fast, so a 1s (30-frame) source
        // span occupies 15 timeline frames. clip at 0, no trim.
        // s=30,e=60 source frames; timeline = 0 + (30-0)/2 = 15 .. (60-0)/2 = 30.
        let c = clip("c", 0, 300, 0, 2.0);
        assert_eq!(span_frames(1.0, 2.0, &c, 30), Some((15, 30)));
    }

    #[test]
    fn span_frames_clamps_straddler_to_visible_sliver() {
        // visible window is source frames [0, 30) (duration 30, speed 1).
        // A word 0.5..2.0s = 15..60 source frames straddles the end; it is
        // clamped to [15, 30) → timeline 15..30, not a fabricated 15..60.
        let c = clip("c", 0, 30, 0, 1.0);
        assert_eq!(span_frames(0.5, 2.0, &c, 30), Some((15, 30)));
    }

    #[test]
    fn span_frames_word_entirely_before_visible_is_none() {
        // trim 30 → visible source starts at frame 30 (=1.0s). A word at
        // 0.0..0.5s (0..15 source frames) is entirely trimmed away.
        let c = clip("c", 0, 300, 30, 1.0);
        assert_eq!(span_frames(0.0, 0.5, &c, 30), None);
    }

    #[test]
    fn span_frames_zero_length_after_clamp_is_none() {
        // Word exactly at the visible end (30 source frames) collapses to e<=s.
        let c = clip("c", 0, 30, 0, 1.0);
        assert_eq!(span_frames(1.0, 1.5, &c, 30), None); // s=30 == vis_end
    }

    #[test]
    fn span_frames_end_never_precedes_start() {
        // Rounding must never invert the interval (upstream `max(a, toTimeline(e))`).
        let c = clip("c", 0, 300, 0, 1.0);
        let (a, b) = span_frames(0.001, 0.002, &c, 30).unwrap();
        assert!(b >= a);
    }

    // --- timeline_transcript ----------------------------------------------

    #[test]
    fn assigns_words_and_maps_to_project_frames() {
        let c = clip("c1", 100, 300, 0, 1.0);
        let t = result(vec![word("hello", 0.0, 0.5), word("world", 0.5, 1.0)]);
        let frags = vec![ClipFragment {
            clip_id: "c1".into(),
            track_index: 2,
            clip: &c,
            transcript: Some(&t),
        }];
        let out = timeline_transcript(frags, 30, None, None);
        assert_eq!(out.clips.len(), 1);
        let cl = &out.clips[0];
        assert_eq!(cl.clip_id, "c1");
        assert_eq!(cl.track_index, 2);
        assert_eq!(cl.start_frame, 100);
        assert_eq!(cl.end_frame, 400);
        // hello 0..0.5s → 100..115; world 0.5..1.0s → 115..130.
        assert_eq!(
            cl.words,
            vec![
                WordRow {
                    text: "hello".into(),
                    start_frame: 100,
                    end_frame: 115
                },
                WordRow {
                    text: "world".into(),
                    start_frame: 115,
                    end_frame: 130
                },
            ]
        );
        assert_eq!(out.total_words, 2);
        assert_eq!(out.next_start_frame, None);
    }

    #[test]
    fn word_without_timing_is_skipped() {
        let c = clip("c1", 0, 300, 0, 1.0);
        let t = result(vec![
            TranscriptionWord {
                text: "notimed".into(),
                start: None,
                end: None,
            },
            word("ok", 0.0, 0.5),
        ]);
        let frags = vec![ClipFragment {
            clip_id: "c1".into(),
            track_index: 0,
            clip: &c,
            transcript: Some(&t),
        }];
        let out = timeline_transcript(frags, 30, None, None);
        assert_eq!(out.clips[0].words.len(), 1);
        assert_eq!(out.clips[0].words[0].text, "ok");
    }

    #[test]
    fn midpoint_outside_visible_span_is_dropped() {
        // trim 30 (1.0s): visible source [30, 330). A word at 0.0..0.4s has
        // midpoint 0.2s = 6 source frames < 30 → dropped even though it has
        // timing. A word at 1.0..1.4s midpoint 36 frames is kept.
        let c = clip("c1", 0, 300, 30, 1.0);
        let t = result(vec![word("before", 0.0, 0.4), word("inside", 1.0, 1.4)]);
        let frags = vec![ClipFragment {
            clip_id: "c1".into(),
            track_index: 0,
            clip: &c,
            transcript: Some(&t),
        }];
        let out = timeline_transcript(frags, 30, None, None);
        assert_eq!(out.clips[0].words.len(), 1);
        assert_eq!(out.clips[0].words[0].text, "inside");
    }

    #[test]
    fn seam_word_attributed_to_one_clip_by_midpoint() {
        // Two clips from the SAME source, split at source frame 30 (1.0s):
        //   clipA: trim 0,  duration 30  → visible source [0, 30)
        //   clipB: trim 30, duration 30  → visible source [30, 60)
        // A word at 0.9..1.1s has midpoint 1.0s = 30 source frames. By half-open
        // membership (`>= vis_start && < vis_end`) it lands in clipB only.
        let a = clip("A", 0, 30, 0, 1.0);
        let b = clip("B", 30, 30, 30, 1.0);
        let ta = result(vec![word("seam", 0.9, 1.1)]);
        let tb = result(vec![word("seam", 0.9, 1.1)]);
        let frags = vec![
            ClipFragment {
                clip_id: "A".into(),
                track_index: 0,
                clip: &a,
                transcript: Some(&ta),
            },
            ClipFragment {
                clip_id: "B".into(),
                track_index: 0,
                clip: &b,
                transcript: Some(&tb),
            },
        ];
        let out = timeline_transcript(frags, 30, None, None);
        // Only clipB contributes; the seam word is emitted exactly once.
        assert_eq!(out.clips.len(), 1);
        assert_eq!(out.clips[0].clip_id, "B");
        assert_eq!(out.total_words, 1);
    }

    #[test]
    fn clips_processed_in_timeline_start_order() {
        let later = clip("late", 200, 100, 0, 1.0);
        let early = clip("early", 0, 100, 0, 1.0);
        let tl = result(vec![word("x", 0.0, 0.5)]);
        let te = result(vec![word("y", 0.0, 0.5)]);
        // Pass out of order; expect sorted by start_frame.
        let frags = vec![
            ClipFragment {
                clip_id: "late".into(),
                track_index: 0,
                clip: &later,
                transcript: Some(&tl),
            },
            ClipFragment {
                clip_id: "early".into(),
                track_index: 0,
                clip: &early,
                transcript: Some(&te),
            },
        ];
        let out = timeline_transcript(frags, 30, None, None);
        assert_eq!(out.clips[0].clip_id, "early");
        assert_eq!(out.clips[1].clip_id, "late");
    }

    #[test]
    fn window_filters_words_by_project_frame() {
        // clip at 0, identity. words: a 0..0.5s→0..15, b 1..1.5s→30..45,
        // c 2..2.5s→60..75. window [30, 60): a ends at 15 (<=30? 15<=30 → drop),
        // b 30..45 kept, c starts at 60 (>=60 → drop).
        let c = clip("c1", 0, 300, 0, 1.0);
        let t = result(vec![
            word("a", 0.0, 0.5),
            word("b", 1.0, 1.5),
            word("c", 2.0, 2.5),
        ]);
        let frags = vec![ClipFragment {
            clip_id: "c1".into(),
            track_index: 0,
            clip: &c,
            transcript: Some(&t),
        }];
        let out = timeline_transcript(frags, 30, Some(30), Some(60));
        assert_eq!(out.clips.len(), 1);
        assert_eq!(out.clips[0].words.len(), 1);
        assert_eq!(out.clips[0].words[0].text, "b");
    }

    #[test]
    fn skipped_transcript_source_is_ignored_not_fatal() {
        let c = clip("c1", 0, 300, 0, 1.0);
        let frags = vec![ClipFragment {
            clip_id: "c1".into(),
            track_index: 0,
            clip: &c,
            transcript: None, // source failed to transcribe
        }];
        let out = timeline_transcript(frags, 30, None, None);
        assert!(out.clips.is_empty());
        assert_eq!(out.total_words, 0);
    }

    #[test]
    fn cap_truncates_and_sets_next_start_frame() {
        // Build one clip whose source has TIMELINE_MAX_WORDS + 5 timed words,
        // each 0.1s apart so they map to distinct increasing frames.
        let c = clip("c1", 0, 10_000_000, 0, 1.0);
        let mut words = Vec::new();
        let n = TIMELINE_MAX_WORDS + 5;
        for i in 0..n {
            let s = i as f64 * 0.1;
            words.push(word("w", s, s + 0.05));
        }
        let t = result(words);
        let frags = vec![ClipFragment {
            clip_id: "c1".into(),
            track_index: 0,
            clip: &c,
            transcript: Some(&t),
        }];
        let out = timeline_transcript(frags, 30, None, None);
        // Emitted exactly the cap; total_words reflects the true count.
        assert_eq!(out.clips[0].words.len(), TIMELINE_MAX_WORDS);
        assert_eq!(out.total_words, n);
        // next_start_frame is the last emitted word's end frame.
        let last = out.clips[0].words.last().unwrap();
        assert_eq!(out.next_start_frame, Some(last.end_frame));
    }

    #[test]
    fn under_cap_has_no_next_start_frame() {
        let c = clip("c1", 0, 300, 0, 1.0);
        let t = result(vec![word("a", 0.0, 0.5)]);
        let frags = vec![ClipFragment {
            clip_id: "c1".into(),
            track_index: 0,
            clip: &c,
            transcript: Some(&t),
        }];
        let out = timeline_transcript(frags, 30, None, None);
        assert_eq!(out.next_start_frame, None);
    }
}
