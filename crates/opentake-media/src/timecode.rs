//! Source start timecode — the ffprobe equivalent of upstream
//! `XMLExporter.Builder.readStartTimecodeFrame` (`Export/XMLExporter.swift:245-262`),
//! which reads a media file's start timecode so the XMEML `<file><timecode>` node
//! carries the real source offset instead of a `00:00:00:00` dummy.
//!
//! ## Why a string parser (upstream reads a raw tmcd frame count)
//! Upstream pulls the QuickTime `tmcd` timecode track via `AVAssetReader` and
//! reads the leading **big-endian UInt32 frame count** straight out of the sample
//! buffer — it never parses a `"HH:MM:SS:FF"` string. That AVFoundation path has
//! no cross-platform equivalent. ffprobe instead surfaces the same information as
//! a formatted string tag (`tags.timecode`), so this module reads that string and
//! converts it back to a start **frame** at the file's frame rate. The two routes
//! yield the same integer for a file whose tmcd track holds `HH:MM:SS:FF`.
//!
//! The string→frame conversion ([`parse_smpte_timecode`]):
//! - **NDF** (`HH:MM:SS:FF`): `frame = ((hh*60+mm)*60+ss)*fps + ff` — the exact
//!   inverse of upstream `formatTimecode`'s non-drop path (`:265-275`).
//! - **DF** (`HH:MM:SS;FF`, 29.97/59.94): *subtracts* the standard SMPTE drop
//!   count (`drop * (total_minutes − total_minutes/10)`,
//!   `drop = round(fps*0.066666)`) from the naive wall-clock frame count. This is
//!   the canonical inverse of the **valid** drop-frame strings ffprobe emits.
//!   (Upstream reads a raw tmcd frame count and never parses a DF *string*, and
//!   its own `formatTimecode` linear-offsets rather than skipping the `;00`/`;01`
//!   boundary frames — so there is no upstream string→frame DF reference to
//!   mirror; canonical SMPTE is the correct target for real ffprobe input.)
//!
//! The parser is pure and unit-tested; the ffprobe read ([`read_start_timecode_frame`])
//! follows the invocation pattern in [`crate::probe`] / [`crate::ff`] and needs a
//! real file, so it is exercised only when a media fixture is available.

use std::path::Path;

use crate::ff;

/// Parse an SMPTE timecode string (`"HH:MM:SS:FF"` non-drop, or `"HH:MM:SS;FF"`
/// drop-frame) to a start **frame** at `fps`, or `None` for malformed input.
///
/// The separator before the frames field selects the mode: `;` (or `.`, the
/// alternate drop-frame separator some tools emit) means drop-frame; `:` means
/// non-drop. Drop-frame math is only applied when `fps` actually rounds to a
/// drop-frame rate (30 or 60) — a `;` on a 25 fps file is treated as non-drop, so
/// a mis-tagged separator never corrupts the frame count.
///
/// `fps` is the integer timebase (upstream `rateTags` timebase: 30 for 29.97,
/// 60 for 59.94, …); `<= 0` yields `None`.
pub fn parse_smpte_timecode(input: &str, fps: i32) -> Option<i32> {
    if fps <= 0 {
        return None;
    }
    let s = input.trim();
    if s.is_empty() {
        return None;
    }

    // The frames field is separated from seconds by ':' (NDF) or ';' / '.' (DF).
    // Split off the final field first, remembering which separator was used, then
    // split the remaining HH:MM:SS on ':'.
    let drop_sep_pos = s.rfind([';', '.']);
    let (head, frames_str, drop_frame) = match drop_sep_pos {
        Some(pos) => (&s[..pos], &s[pos + 1..], true),
        None => {
            let pos = s.rfind(':')?;
            (&s[..pos], &s[pos + 1..], false)
        }
    };

    let mut parts = head.split(':');
    let hh = parse_field(parts.next()?)?;
    let mm = parse_field(parts.next()?)?;
    let ss = parse_field(parts.next()?)?;
    if parts.next().is_some() {
        return None; // too many ':' groups
    }
    let ff = parse_field(frames_str)?;

    // Range sanity: minutes/seconds must be clock-valid; frames below the
    // timebase. (Upstream never validates because the tmcd frame count is already
    // canonical; here the string is external input, so reject nonsense.)
    if mm >= 60 || ss >= 60 || ff >= fps {
        return None;
    }

    let total_seconds = (hh * 60 + mm) * 60 + ss;
    let naive = total_seconds * fps + ff;

    // Only compensate when the timebase is genuinely a drop-frame rate; a stray
    // ';' on 24/25 fps is honored as plain non-drop.
    if drop_frame && is_drop_frame_rate(fps) {
        let drop = drop_frames_per_minute(fps);
        let total_minutes = hh * 60 + mm;
        let dropped = drop * (total_minutes - total_minutes / 10);
        Some(naive - dropped)
    } else {
        Some(naive)
    }
}

/// Parse one non-negative integer field, rejecting signs / non-digits so that a
/// stray `-` or letter fails the whole timecode rather than silently truncating.
fn parse_field(s: &str) -> Option<i32> {
    let s = s.trim();
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse::<i32>().ok()
}

/// A drop-frame rate is one whose integer timebase is a multiple of 30 (30 →
/// 29.97, 60 → 59.94). Mirrors upstream's `dropFrame = ntsc && timebase % 30 == 0`
/// gate, minus the ntsc flag (which the string tag doesn't carry).
fn is_drop_frame_rate(fps: i32) -> bool {
    fps % 30 == 0
}

/// Frames dropped per minute (except every 10th): `round(fps * 0.066666)` — 2 at
/// 30, 4 at 60. Verbatim from upstream `formatTimecode`'s `drop` computation.
fn drop_frames_per_minute(fps: i32) -> i32 {
    (fps as f64 * 0.066666).round() as i32
}

/// Read a media file's start timecode as a frame count at `fps`, or `None` when
/// the file carries no timecode tag / the tag is unparseable / ffprobe is
/// unavailable. ffprobe exposes the QuickTime `tmcd` timecode (and container
/// timecode metadata) as a `tags.timecode` string on a stream or on the format;
/// this reads whichever is present and converts it via [`parse_smpte_timecode`].
///
/// `fps` is the export timebase the caller already computed for the file (upstream
/// `rateTags` timebase), so the frames-per-second used for parsing matches the
/// `<rate>` written alongside the `<timecode>` node.
pub fn read_start_timecode_frame(path: &Path, fps: i32) -> Option<i32> {
    let json = ff::ffprobe_json(path).ok()?;
    let tag = timecode_tag(&json)?;
    parse_smpte_timecode(&tag, fps)
}

/// Extract the first `tags.timecode` string from ffprobe JSON, preferring a
/// stream tag (the `tmcd` track / video stream) and falling back to the format
/// (container) tag. Pure over the JSON so the lookup precedence is unit-testable
/// without invoking ffprobe.
pub fn timecode_tag(json: &serde_json::Value) -> Option<String> {
    // Stream tags first: a dedicated timecode stream or a video stream that
    // carries the tmcd metadata.
    if let Some(streams) = json.get("streams").and_then(|v| v.as_array()) {
        for s in streams {
            if let Some(tc) = tags_timecode(s) {
                return Some(tc);
            }
        }
    }
    // Container-level fallback (`format.tags.timecode`).
    tags_timecode(json.get("format")?)
}

/// `<obj>.tags.timecode` as a non-empty trimmed string.
fn tags_timecode(obj: &serde_json::Value) -> Option<String> {
    let tc = obj
        .get("tags")?
        .get("timecode")?
        .as_str()?
        .trim()
        .to_string();
    if tc.is_empty() {
        None
    } else {
        Some(tc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- NDF: plain inverse of upstream formatTimecode's non-drop path ---

    #[test]
    fn ndf_zero_is_frame_zero() {
        assert_eq!(parse_smpte_timecode("00:00:00:00", 30), Some(0));
    }

    #[test]
    fn ndf_counts_frames_seconds_minutes_hours() {
        // 1h 2m 3s 4f @ 30 = ((1*60+2)*60+3)*30 + 4 = (3723)*30 + 4 = 111694.
        assert_eq!(parse_smpte_timecode("01:02:03:04", 30), Some(111_694));
    }

    #[test]
    fn ndf_one_second_is_fps_frames() {
        assert_eq!(parse_smpte_timecode("00:00:01:00", 24), Some(24));
        assert_eq!(parse_smpte_timecode("00:00:01:00", 25), Some(25));
    }

    #[test]
    fn ndf_frames_field_adds_directly() {
        assert_eq!(parse_smpte_timecode("00:00:00:29", 30), Some(29));
    }

    #[test]
    fn ndf_is_exact_inverse_of_upstream_format_timecode_non_drop() {
        // Reproduce upstream formatTimecode(non-drop) for several frames and
        // confirm parse() round-trips back to the original frame index.
        let fps = 25;
        for &frame in &[0, 1, 24, 25, 26, 1000, 90_061] {
            let ff = frame % fps;
            let ss = (frame / fps) % 60;
            let mm = (frame / (fps * 60)) % 60;
            let hh = frame / (fps * 3600);
            let s = format!("{hh:02}:{mm:02}:{ss:02}:{ff:02}");
            assert_eq!(parse_smpte_timecode(&s, fps), Some(frame), "frame {frame}");
        }
    }

    // --- DF: inverse of upstream's drop-frame path (subtract SMPTE drop count) ---

    #[test]
    fn df_first_minute_matches_naive_count() {
        // Within the first minute nothing is dropped: 00:00:10;15 @ 30 = 10*30+15.
        assert_eq!(parse_smpte_timecode("00:00:10;15", 30), Some(315));
    }

    #[test]
    fn df_drops_two_frames_at_first_whole_minute() {
        // Drop-frame 29.97: at 00:01:00;02 the two frames 00:01:00;00 and ;01 do
        // not exist, so it is the frame immediately after 00:00:59;29.
        // naive(00:01:00;02) = 60*30 + 2 = 1802; dropped = 2*(1 - 0) = 2 → 1800.
        // naive(00:00:59;29) = 59*30 + 29 = 1799 → +1 = 1800. They meet at 1800.
        assert_eq!(parse_smpte_timecode("00:00:59;29", 30), Some(1799));
        assert_eq!(parse_smpte_timecode("00:01:00;02", 30), Some(1800));
    }

    #[test]
    fn df_no_drop_on_tenth_minute() {
        // Every 10th minute keeps its frames: 00:10:00;00 drops 2*(10 - 1) = 18.
        // naive = 10*60*30 = 18000 → 18000 - 18 = 17982.
        assert_eq!(parse_smpte_timecode("00:10:00;00", 30), Some(17_982));
    }

    #[test]
    fn df_is_inverse_of_standard_drop_frame_encoding() {
        // ffprobe's `tags.timecode` for a 29.97 file is a *valid* SMPTE
        // drop-frame string (the two frames `;00`/`;01` are skipped at each
        // minute except every tenth). Encode a frame index to that canonical
        // string, then confirm parse() reads back the same integer.
        //
        // NOTE: upstream `formatTimecode` does NOT skip those boundary frames —
        // it linearly offsets and can emit the invalid `00:01:00;00`. Upstream
        // never *parses* timecode (it reads a raw tmcd frame count), so it
        // provides no string→frame DF reference; the canonical SMPTE encoding
        // that real ffprobe output uses is the correct inverse target here.
        let fps = 30;
        let drop = (fps as f64 * 0.066666).round() as i32; // 2
                                                           // Canonical SMPTE drop-frame: frame index -> valid HH:MM:SS;FF.
        let forward = |frame: i32| -> String {
            let frames_per_10min = fps * 600 - drop * 9;
            let frames_per_min_df = fps * 60 - drop;
            let d = frame / frames_per_10min;
            let m = frame % frames_per_10min;
            let f = if m > drop {
                frame + drop * 9 * d + drop * ((m - drop) / frames_per_min_df)
            } else {
                frame + drop * 9 * d
            };
            let ff = f % fps;
            let ss = (f / fps) % 60;
            let mm = (f / (fps * 60)) % 60;
            let hh = f / (fps * 3600);
            format!("{hh:02}:{mm:02}:{ss:02};{ff:02}")
        };
        for &frame in &[0, 1, 1799, 1800, 1801, 17_982, 20_000, 107_892] {
            let s = forward(frame);
            assert_eq!(
                parse_smpte_timecode(&s, fps),
                Some(frame),
                "frame {frame} via {s}"
            );
        }
    }

    #[test]
    fn df_separator_on_non_drop_rate_is_treated_as_non_drop() {
        // A ';' on 25 fps must NOT apply drop math (25 is not a drop-frame rate).
        assert_eq!(parse_smpte_timecode("00:10:00;00", 25), Some(15_000));
    }

    #[test]
    fn dot_separator_is_drop_frame() {
        // Some tools emit '.' as the drop-frame separator; treat it like ';'.
        assert_eq!(
            parse_smpte_timecode("00:10:00.00", 30),
            parse_smpte_timecode("00:10:00;00", 30)
        );
    }

    // --- Malformed input → None ---

    #[test]
    fn rejects_bad_shapes() {
        assert_eq!(parse_smpte_timecode("", 30), None);
        assert_eq!(parse_smpte_timecode("garbage", 30), None);
        assert_eq!(parse_smpte_timecode("00:00:00", 30), None); // only 3 fields
        assert_eq!(parse_smpte_timecode("00:00:00:00:00", 30), None); // 5 fields
        assert_eq!(parse_smpte_timecode("aa:bb:cc:dd", 30), None);
        assert_eq!(parse_smpte_timecode("00:00:00:-1", 30), None); // signed field
        assert_eq!(parse_smpte_timecode("01:02:03:04", 0), None); // bad fps
    }

    #[test]
    fn rejects_out_of_range_fields() {
        assert_eq!(parse_smpte_timecode("00:60:00:00", 30), None); // minutes
        assert_eq!(parse_smpte_timecode("00:00:60:00", 30), None); // seconds
        assert_eq!(parse_smpte_timecode("00:00:00:30", 30), None); // frame >= fps
        assert_eq!(parse_smpte_timecode("00:00:00:24", 24), None); // frame == fps
    }

    // --- timecode_tag JSON lookup precedence ---

    #[test]
    fn timecode_tag_prefers_stream_over_format() {
        let j = json!({
            "streams": [
                {"codec_type": "video", "tags": {"timecode": "01:00:00:00"}}
            ],
            "format": {"tags": {"timecode": "02:00:00:00"}}
        });
        assert_eq!(timecode_tag(&j).as_deref(), Some("01:00:00:00"));
    }

    #[test]
    fn timecode_tag_falls_back_to_format() {
        let j = json!({
            "streams": [{"codec_type": "video"}],
            "format": {"tags": {"timecode": "00:00:10:00"}}
        });
        assert_eq!(timecode_tag(&j).as_deref(), Some("00:00:10:00"));
    }

    #[test]
    fn timecode_tag_absent_is_none() {
        let j = json!({"streams": [{"codec_type": "video"}], "format": {}});
        assert_eq!(timecode_tag(&j), None);
    }

    #[test]
    fn timecode_tag_empty_string_is_none() {
        let j = json!({"streams": [], "format": {"tags": {"timecode": "   "}}});
        assert_eq!(timecode_tag(&j), None);
    }

    #[test]
    fn timecode_tag_scans_multiple_streams() {
        // Audio stream first with no tc; the tmcd/video stream carries it.
        let j = json!({
            "streams": [
                {"codec_type": "audio"},
                {"codec_type": "data", "tags": {"timecode": "00:05:00:00"}}
            ],
            "format": {}
        });
        assert_eq!(timecode_tag(&j).as_deref(), Some("00:05:00:00"));
    }
}
