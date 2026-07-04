//! Track-role auto-detection + per-role advice (`agent-SPEC.md` §6.4,
//! `AGENT-CONTEXT-SIGNAL.md` §3). Pure structural detection from the `Timeline`;
//! semantically-heavy signals (long speech / spectral richness) are deferred to
//! `opentake-media` and approximated structurally here (MVP).
//!
//! Uses the real `opentake_domain::TrackRole` enum (`MainCamera`, `BRoll`,
//! `Voice`, `Bgm`, `Sfx`, `Text`, `Caption`).

use opentake_domain::{ClipType, Timeline, TrackHint, TrackRole, TrackRoleAssignment};

/// Frames/seconds thresholds from the doc's rule table.
const LONG_CLIP_SECONDS: f64 = 10.0;
const SHORT_CLIP_SECONDS: f64 = 5.0;

/// Detect a role for every track from structural features alone
/// (`AGENT-CONTEXT-SIGNAL.md` §3.1):
/// - video: long contiguous clips → MainCamera; short clips above a MainCamera
///   → BRoll; all-text → Text; all-caption → Caption; else GenericVideo
///   (mapped to MainCamera as the closest real role for video).
/// - audio: structural-only here → Voice by default (long speech needs media
///   signals), short non-speech → Sfx, else Bgm.
pub fn detect_track_roles(timeline: &Timeline) -> Vec<TrackRoleAssignment> {
    let fps = timeline.fps.max(1) as f64;
    // First, find the index of the longest-clip video track (the MainCamera
    // candidate) so "above MainCamera" can be判定 for BRoll.
    let main_camera_index = video_main_camera_index(timeline, fps);

    timeline
        .tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let role = match track.kind {
                ClipType::Video | ClipType::Image | ClipType::Lottie => {
                    detect_visual_role(track, i, main_camera_index, fps)
                }
                ClipType::Text => TrackRole::Text,
                ClipType::Audio => detect_audio_role(track, fps),
            };
            TrackRoleAssignment {
                track_index: i,
                role,
            }
        })
        .collect()
}

fn all_clips_are_text(track: &opentake_domain::Track) -> bool {
    !track.clips.is_empty() && track.clips.iter().all(|c| c.media_type == ClipType::Text)
}

fn all_clips_are_caption(track: &opentake_domain::Track) -> bool {
    !track.clips.is_empty() && track.clips.iter().all(|c| c.caption_group_id.is_some())
}

/// Longest *average* clip among video tracks, returned as the MainCamera index.
fn video_main_camera_index(timeline: &Timeline, fps: f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, t) in timeline.tracks.iter().enumerate() {
        if t.kind != ClipType::Video || t.clips.is_empty() {
            continue;
        }
        if all_clips_are_text(t) || all_clips_are_caption(t) {
            continue;
        }
        let avg = t
            .clips
            .iter()
            .map(|c| c.duration_frames as f64 / fps)
            .sum::<f64>()
            / t.clips.len() as f64;
        if avg >= LONG_CLIP_SECONDS && best.map(|(_, b)| avg > b).unwrap_or(true) {
            best = Some((i, avg));
        }
    }
    best.map(|(i, _)| i)
}

fn detect_visual_role(
    track: &opentake_domain::Track,
    index: usize,
    main_camera_index: Option<usize>,
    fps: f64,
) -> TrackRole {
    if all_clips_are_caption(track) {
        return TrackRole::Caption;
    }
    if all_clips_are_text(track) {
        return TrackRole::Text;
    }
    if track.clips.is_empty() {
        return TrackRole::MainCamera;
    }
    let avg = track
        .clips
        .iter()
        .map(|c| c.duration_frames as f64 / fps)
        .sum::<f64>()
        / track.clips.len() as f64;
    if Some(index) == main_camera_index {
        return TrackRole::MainCamera;
    }
    // Short clips layered above the MainCamera → B-roll overlay.
    if let Some(mc) = main_camera_index {
        if index > mc && avg < SHORT_CLIP_SECONDS {
            return TrackRole::BRoll;
        }
    }
    if avg >= LONG_CLIP_SECONDS {
        TrackRole::MainCamera
    } else {
        TrackRole::BRoll
    }
}

fn detect_audio_role(track: &opentake_domain::Track, fps: f64) -> TrackRole {
    if track.clips.is_empty() {
        return TrackRole::Voice;
    }
    let avg = track
        .clips
        .iter()
        .map(|c| c.duration_frames as f64 / fps)
        .sum::<f64>()
        / track.clips.len() as f64;
    // Short, many-clip audio tracks read as SFX; long continuous as Voice;
    // medium-long as BGM. (Spectral detection is a media-layer upgrade.)
    if avg < 2.0 {
        TrackRole::Sfx
    } else if avg >= LONG_CLIP_SECONDS {
        TrackRole::Voice
    } else {
        TrackRole::Bgm
    }
}

/// Per-role advice (`AGENT-CONTEXT-SIGNAL.md` §3.2; text VERBATIM from the doc).
pub fn role_advice(role: TrackRole) -> &'static str {
    match role {
        TrackRole::MainCamera => {
            "这是口播/讲解的主画面(A-roll)。不要在这条轨上做大幅缩放；硬切处用放大+位移遮蔽或贴 B-roll。主干时间轴，删 clip 会影响整体结构。"
        }
        TrackRole::BRoll => {
            "补充画面层。B-roll 遵循五注意：对齐口播时长 / 成组添加 / 遮蔽硬切 / 不重复 / 整轨静音。不够长就换素材，不要漏字。"
        }
        TrackRole::Text => {
            "文字层。文字安全区在画布中央 80%。避免压在人物脸上。竖屏项目注意上下留白。"
        }
        TrackRole::Voice => {
            "主声音轨。气口按三规则处理（保留/扩充/叠化）；切点选在句界或重音；有 BGM 时做侧链让位。不可整轨静音。"
        }
        TrackRole::Bgm => {
            "背景音乐。检测节拍作为镜头切换参考点；口播段压低让位人声(侧链/手动)；段落间做 J/L-cut 过渡。"
        }
        TrackRole::Sfx => {
            "音效轨。上升音效(Rise)用于段落过渡前；低频轰鸣(Sub Boom)用于重点落点；环境音提前画面 2-3 秒渐入。"
        }
        TrackRole::Caption => {
            "字幕层。气口按三规则处理；切点选在句界。文字安全区在画布中央 80%。"
        }
    }
}

/// Build the per-track hints from role assignments.
pub fn track_hints(assignments: &[TrackRoleAssignment]) -> Vec<TrackHint> {
    assignments
        .iter()
        .map(|a| TrackHint {
            track_index: a.track_index,
            role: a.role,
            advice: role_advice(a.role).to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, ClipType, Track};

    fn clip(id: &str, start: i32, dur: i32) -> Clip {
        Clip::new(id, "asset", start, dur)
    }

    #[test]
    fn long_video_track_is_main_camera() {
        let mut tl = Timeline::new(); // fps 30
        let mut v = Track::new("v1", ClipType::Video);
        v.clips.push(clip("a", 0, 30 * 15)); // 15s clip
        tl.tracks.push(v);
        let roles = detect_track_roles(&tl);
        assert_eq!(roles[0].role, TrackRole::MainCamera);
    }

    #[test]
    fn short_clips_above_main_camera_are_broll() {
        let mut tl = Timeline::new();
        let mut main = Track::new("v1", ClipType::Video);
        main.clips.push(clip("m", 0, 30 * 15)); // 15s -> MainCamera
        let mut overlay = Track::new("v2", ClipType::Video);
        overlay.clips.push(clip("b", 0, 30 * 2)); // 2s, above main
        tl.tracks.push(main);
        tl.tracks.push(overlay);
        let roles = detect_track_roles(&tl);
        assert_eq!(roles[0].role, TrackRole::MainCamera);
        assert_eq!(roles[1].role, TrackRole::BRoll);
    }

    #[test]
    fn all_text_track_is_text_role() {
        let mut tl = Timeline::new();
        let mut t = Track::new("v1", ClipType::Video);
        let mut c = clip("t", 0, 60);
        c.media_type = ClipType::Text;
        t.clips.push(c);
        tl.tracks.push(t);
        let roles = detect_track_roles(&tl);
        assert_eq!(roles[0].role, TrackRole::Text);
    }

    #[test]
    fn caption_track_is_caption_role() {
        let mut tl = Timeline::new();
        let mut t = Track::new("v1", ClipType::Video);
        let mut c = clip("cap", 0, 30);
        c.media_type = ClipType::Text;
        c.caption_group_id = Some("g".into());
        t.clips.push(c);
        tl.tracks.push(t);
        let roles = detect_track_roles(&tl);
        assert_eq!(roles[0].role, TrackRole::Caption);
    }

    #[test]
    fn long_audio_is_voice_short_is_sfx() {
        let mut tl = Timeline::new();
        let mut voice = Track::new("a1", ClipType::Audio);
        voice.clips.push(clip("v", 0, 30 * 20)); // 20s -> Voice
        let mut sfx = Track::new("a2", ClipType::Audio);
        sfx.clips.push(clip("s", 0, 30)); // 1s -> Sfx
        tl.tracks.push(voice);
        tl.tracks.push(sfx);
        let roles = detect_track_roles(&tl);
        assert_eq!(roles[0].role, TrackRole::Voice);
        assert_eq!(roles[1].role, TrackRole::Sfx);
    }

    #[test]
    fn advice_text_is_verbatim_for_main_camera() {
        assert!(role_advice(TrackRole::MainCamera).contains("A-roll"));
        assert!(role_advice(TrackRole::BRoll).contains("五注意"));
        assert!(role_advice(TrackRole::Voice).contains("气口按三规则"));
    }

    #[test]
    fn hints_carry_index_role_advice() {
        let a = vec![TrackRoleAssignment {
            track_index: 2,
            role: TrackRole::Bgm,
        }];
        let h = track_hints(&a);
        assert_eq!(h[0].track_index, 2);
        assert_eq!(h[0].role, TrackRole::Bgm);
        assert!(h[0].advice.contains("背景音乐"));
    }
}
