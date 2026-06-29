//! Timeline export as a CMX3600 EDL (`.edl`) — the classic edit decision list
//! Premiere / DaVinci Resolve / Avid / 剪映 import.
//!
//! ## Format
//!
//! A CMX3600 EDL is a fixed-column ASCII text file:
//!
//! ```text
//! TITLE: My Timeline
//! FCM: NON-DROP FRAME
//!
//! 001  AX       V     C        00:00:00:00 00:00:02:00 00:00:00:00 00:00:02:00
//! * FROM CLIP NAME: shot.mp4
//! ```
//!
//! - `TITLE:` line, then `FCM:` (frame-code mode — `DROP FRAME` for 29.97/59.94,
//!   else `NON-DROP FRAME`).
//! - One **event** per clip: a 3-digit number, the reel name, the channel
//!   (`V` / `A` / `AA` / `B` for video+audio), the transition (`C` cut,
//!   `D` dissolve), then the four timecodes: source-in, source-out, record-in,
//!   record-out (`HH:MM:SS:FF` at the timeline fps).
//! - A `* FROM CLIP NAME:` comment naming the clip's media.
//!
//! ## What this preserves vs. drops
//!
//! Preserves: clip ordering, source in/out (trim), record placement, fade →
//! dissolve transition, the clip's media name, and the reel name. Source and
//! record timecodes are at the **timeline** fps; drop-frame is signalled per the
//! fps (29.97 / 59.94 → `DROP FRAME`).
//!
//! Drops (intrinsic to the EDL format — documented in a `* ` comment in the
//! output, mirroring how real NLEs emit EDLs):
//! - **Audio tracks / clips.** CMX3600 describes a *single* video track plus its
//!   linked audio channels; it has no representation for OpenTake's ordered,
//!   typed multi-track model. We export the topmost video track only (the same
//!   limitation DaVinci's "EDL" export carries). Use XMEML / OTIO / FCPXML for
//!   audio + multi-track.
//! - Transforms, scale, rotation, crop, opacity, volume, keyframes, speed
//!   (an EDL has no fields for them — `M2` speed lines are intentionally omitted
//!   to keep the file maximally portable).
//! - Text overlays.
//!
//! ## Frame fidelity
//!
//! All timing is integer frames. `HH:MM:SS:FF` is computed at the timeline fps
//! exactly like the XMEML exporter's `format_timecode` (drop-frame uses `;` and
//! skips dropped frame numbers). Source timecodes start at frame 0 (OpenTake has
//! no cross-platform tape/source-timecode reader — see `fcpxml.rs`); the source
//! window is `[trim_start, trim_start + source_frames_consumed)`.

use opentake_domain::{Clip, MediaManifest, MediaResolver, Timeline, Track};

/// Reel name for every event. Real source-tape names need a tape-timecode
/// reader OpenTake lacks; `AX` ("auxiliary") is the CMX3600 convention for
/// file-based / unnamed sources and is what DaVinci emits for the same case.
const REEL: &str = "AX";

/// Export a [`Timeline`] as a CMX3600 EDL string. Pure function: takes the
/// timeline + media manifest, returns the full EDL text (video track only — see
/// the module docs).
pub fn export_edl(timeline: &Timeline, manifest: &MediaManifest) -> String {
    let resolver = MediaResolver::new(manifest, None);
    Builder {
        timeline,
        resolver: &resolver,
    }
    .build()
}

struct Builder<'a> {
    timeline: &'a Timeline,
    resolver: &'a MediaResolver<'a>,
}

impl Builder<'_> {
    fn build(&self) -> String {
        let fps = self.timeline.fps.max(1);
        let drop_frame = is_drop_frame(fps);

        let mut out = String::new();
        out.push_str("TITLE: Timeline Export\n");
        out.push_str(if drop_frame {
            "FCM: DROP FRAME\n"
        } else {
            "FCM: NON-DROP FRAME\n"
        });
        // Document the format's structural limitations, the way NLEs annotate EDLs.
        out.push_str("* CMX3600 EDL — video track only; audio, effects, transforms, and\n");
        out.push_str("* multi-track layering are not representable. Use XMEML / OTIO / FCPXML\n");
        out.push_str("* for full fidelity.\n");

        let clips = self.top_video_clips();
        if clips.is_empty() {
            return out;
        }

        for (idx, clip) in clips.iter().enumerate() {
            self.push_event(&mut out, idx as u32 + 1, clip, fps, drop_frame);
        }
        out
    }

    /// Clips of the topmost video track, sorted by start frame. CMX3600 holds a
    /// single video track, so we pick the first visual track in timeline order.
    fn top_video_clips(&self) -> Vec<Clip> {
        let track: Option<&Track> = self.timeline.tracks.iter().find(|t| t.kind.is_visual());
        let Some(track) = track else {
            return Vec::new();
        };
        let mut clips: Vec<Clip> = track.clips.clone();
        clips.sort_by_key(|c| c.start_frame);
        clips
    }

    /// Emit one numbered event line + its `FROM CLIP NAME` comment.
    fn push_event(&self, out: &mut String, event: u32, clip: &Clip, fps: i32, drop: bool) {
        // Source window: trim offset for `source_frames_consumed` frames. Source
        // timecode origin is 0 (no tape-timecode reader). Record window is the
        // clip's timeline placement.
        let src_in = clip.trim_start_frame.max(0);
        let src_out = src_in + clip.source_frames_consumed().max(0);
        let rec_in = clip.start_frame.max(0);
        let rec_out = clip.end_frame().max(rec_in);

        // Fade-in OR fade-out makes this a dissolve; CMX3600 encodes the
        // dissolve length (frames) in the transition-duration column.
        let fade = clip.fade_in_frames.max(clip.fade_out_frames);
        let (transition, dur_col) = if fade > 0 {
            ("D".to_string(), format!("{fade:03}"))
        } else {
            ("C".to_string(), "   ".to_string())
        };

        // Fixed-column CMX3600 event line. Channel is always `V` (video-only).
        out.push_str(&format!(
            "{event:03}  {reel:<8} {chan:<4} {trans:<4} {dur} {si} {so} {ri} {ro}\n",
            reel = REEL,
            chan = "V",
            trans = transition,
            dur = dur_col,
            si = format_timecode(src_in, fps, drop),
            so = format_timecode(src_out, fps, drop),
            ri = format_timecode(rec_in, fps, drop),
            ro = format_timecode(rec_out, fps, drop),
        ));
        out.push_str(&format!(
            "* FROM CLIP NAME: {}\n",
            self.resolver.display_name(&clip.media_ref)
        ));
    }
}

/// 29.97 / 59.94 (NTSC rates whose nominal fps is a multiple of 30) use
/// drop-frame timecode. 23.976 / 24 / 25 / 30 / 50 / 60 are non-drop.
fn is_drop_frame(fps: i32) -> bool {
    fps == 30 || fps == 60
}

/// Frame count → `HH:MM:SS:FF`. Drop-frame (30/60) uses `;` and skips the
/// dropped frame numbers. 1:1 with the XMEML exporter's `format_timecode` so the
/// two formats agree on the same timeline.
fn format_timecode(frame: i32, fps: i32, drop_frame: bool) -> String {
    let mut f = frame.max(0);
    if drop_frame {
        let drop = (fps as f64 * 0.066_666).round() as i32; // 30 → 2, 60 → 4
        let d = f / (fps * 600);
        let m = f % (fps * 600);
        f += drop * 9 * d
            + if m > drop {
                drop * ((m - drop) / (fps * 60))
            } else {
                0
            };
    }
    let sep = if drop_frame { ";" } else { ":" };
    let ff = f % fps;
    let ss = (f / fps) % 60;
    let mm = (f / (fps * 60)) % 60;
    let hh = f / (fps * 3600);
    format!("{hh:02}:{mm:02}:{ss:02}{sep}{ff:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{ClipType, MediaManifestEntry, MediaSource};

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
            source_fps: Some(30.0),
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

    // --- timecode ---

    #[test]
    fn timecode_non_drop_basic() {
        assert_eq!(format_timecode(0, 30, false), "00:00:00:00");
        assert_eq!(format_timecode(30, 30, false), "00:00:01:00");
        assert_eq!(format_timecode(90, 30, false), "00:00:03:00");
        // 1h 1m 1s 1f at 30fps.
        let f = 30 * 3600 + 30 * 60 + 30 + 1;
        assert_eq!(format_timecode(f, 30, false), "01:01:01:01");
    }

    #[test]
    fn timecode_drop_frame_uses_semicolon() {
        // Drop-frame separates with `;`.
        let tc = format_timecode(0, 30, true);
        assert_eq!(tc, "00:00:00;00");
        assert!(format_timecode(45, 30, true).contains(';'));
    }

    #[test]
    fn drop_frame_classification() {
        assert!(is_drop_frame(30));
        assert!(is_drop_frame(60));
        assert!(!is_drop_frame(24));
        assert!(!is_drop_frame(25));
    }

    // --- header ---

    #[test]
    fn header_has_title_and_fcm() {
        let tl = Timeline::new(); // 30fps → drop frame
        let edl = export_edl(&tl, &manifest(vec![]));
        assert!(edl.starts_with("TITLE: Timeline Export\n"));
        assert!(edl.contains("FCM: DROP FRAME\n"));
        // The video-only limitation must be documented in a comment.
        assert!(edl.contains("* CMX3600 EDL — video track only"));
    }

    #[test]
    fn non_drop_header_for_24fps() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let edl = export_edl(&tl, &manifest(vec![]));
        assert!(edl.contains("FCM: NON-DROP FRAME\n"));
    }

    // --- events ---

    #[test]
    fn single_clip_emits_numbered_event_and_clip_name() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        // start 0, dur 48 (2s @24). trim 0, speed 1 → src [0,48), rec [0,48).
        vt.clips.push(Clip::new("c1", "v1", 0, 48));
        tl.tracks.push(vt);
        let edl = export_edl(
            &tl,
            &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        );

        // 001  AX  V  C  <blank>  00:00:00:00 00:00:02:00 00:00:00:00 00:00:02:00
        assert!(edl.contains("001  AX"));
        assert!(edl.contains("V "));
        assert!(edl.contains(" C  "));
        assert!(edl.contains("00:00:00:00 00:00:02:00 00:00:00:00 00:00:02:00"));
        assert!(edl.contains("* FROM CLIP NAME: shot.mp4"));
    }

    #[test]
    fn trim_offsets_source_in_out() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        let mut clip = Clip::new("c1", "v1", 24, 24); // rec [24,48) = [1s,2s)
        clip.trim_start_frame = 24; // src starts at 1s
        vt.clips.push(clip);
        tl.tracks.push(vt);
        let edl = export_edl(
            &tl,
            &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        );
        // src in 1s, src out 2s; rec in 1s, rec out 2s.
        assert!(edl.contains("00:00:01:00 00:00:02:00 00:00:01:00 00:00:02:00"));
    }

    #[test]
    fn multiple_clips_get_sequential_event_numbers_in_start_order() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        // Push out of order; exporter sorts by start frame.
        vt.clips.push(Clip::new("c2", "v2", 48, 24));
        vt.clips.push(Clip::new("c1", "v1", 0, 48));
        tl.tracks.push(vt);
        let edl = export_edl(
            &tl,
            &manifest(vec![
                entry("v1", "first.mp4", ClipType::Video, 4.0),
                entry("v2", "second.mp4", ClipType::Video, 4.0),
            ]),
        );
        let first = edl.find("first.mp4").unwrap();
        let second = edl.find("second.mp4").unwrap();
        assert!(first < second, "events must be in start order");
        assert!(edl.contains("001  AX"));
        assert!(edl.contains("002  AX"));
    }

    #[test]
    fn fade_emits_dissolve_with_duration() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        let mut clip = Clip::new("c1", "v1", 0, 48);
        clip.fade_in_frames = 12;
        vt.clips.push(clip);
        tl.tracks.push(vt);
        let edl = export_edl(
            &tl,
            &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        );
        // Dissolve transition `D` with a 012 duration column.
        assert!(edl.contains(" D  "));
        assert!(edl.contains("012"));
    }

    #[test]
    fn audio_track_is_dropped_video_only() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut at = Track::new("a", ClipType::Audio);
        let mut aclip = Clip::new("ca", "a1", 0, 48);
        aclip.media_type = ClipType::Audio;
        at.clips.push(aclip);
        let mut vt = Track::new("v", ClipType::Video);
        vt.clips.push(Clip::new("cv", "v1", 0, 48));
        tl.tracks.push(at);
        tl.tracks.push(vt);
        let edl = export_edl(
            &tl,
            &manifest(vec![
                entry("a1", "song.mp3", ClipType::Audio, 10.0),
                entry("v1", "shot.mp4", ClipType::Video, 4.0),
            ]),
        );
        // Only the video clip appears; the audio clip is dropped.
        assert!(edl.contains("shot.mp4"));
        assert!(!edl.contains("song.mp3"));
        // Exactly one event.
        assert!(edl.contains("001  AX"));
        assert!(!edl.contains("002  AX"));
    }

    #[test]
    fn empty_timeline_is_header_only() {
        let tl = Timeline::new();
        let edl = export_edl(&tl, &manifest(vec![]));
        assert!(edl.contains("TITLE:"));
        assert!(!edl.contains("001"));
    }

    #[test]
    fn unknown_media_falls_back_to_offline_name() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        vt.clips.push(Clip::new("c1", "ghost", 0, 24));
        tl.tracks.push(vt);
        let edl = export_edl(&tl, &manifest(vec![]));
        // MediaResolver yields "Offline" for an unknown asset id.
        assert!(edl.contains("* FROM CLIP NAME: Offline"));
    }

    #[test]
    fn speed_clip_uses_consumed_source_frames() {
        let mut tl = Timeline::new();
        tl.fps = 24;
        let mut vt = Track::new("v", ClipType::Video);
        let mut clip = Clip::new("c1", "v1", 0, 24); // 1s on timeline
        clip.speed = 2.0; // consumes 48 source frames (2s)
        vt.clips.push(clip);
        tl.tracks.push(vt);
        let edl = export_edl(
            &tl,
            &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        );
        // src window 2s, record window 1s.
        assert!(edl.contains("00:00:00:00 00:00:02:00 00:00:00:00 00:00:01:00"));
    }
}
