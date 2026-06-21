//! Pure-function RenderPlan tests (SPEC §2.7). No GPU. Each assertion traces to
//! an upstream `CompositionBuilder` formula or the domain `*_at` semantics.

use opentake_domain::{
    AnimPair, Clip, ClipType, Crop, Interpolation, Keyframe, KeyframeTrack, Point, Timeline, Track,
    Transform,
};

use super::affine::affine_transform;
use super::build::{build_render_plan, source_frame_index};
use super::types::{RenderSize, TextureSource};
use crate::source::SourceMetrics;

/// Test metrics: every media ref reports a fixed natural size, identity
/// orientation, no alpha. Lottie frame count configurable per ref.
struct TestMetrics {
    nat: (u32, u32),
    premultiply: bool,
    pt: [f64; 6],
    lottie_frames: Option<i64>,
}

impl Default for TestMetrics {
    fn default() -> Self {
        TestMetrics {
            nat: (1920, 1080),
            premultiply: false,
            pt: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            lottie_frames: None,
        }
    }
}

impl SourceMetrics for TestMetrics {
    fn natural_size(&self, _r: &str) -> Option<(u32, u32)> {
        Some(self.nat)
    }
    fn preferred_transform(&self, _r: &str) -> [f64; 6] {
        self.pt
    }
    fn needs_premultiply(&self, _r: &str) -> bool {
        self.premultiply
    }
    fn lottie_frame_count(&self, _r: &str) -> Option<i64> {
        self.lottie_frames
    }
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn approx_affine(a: [f64; 6], b: [f64; 6]) {
    for i in 0..6 {
        assert!((a[i] - b[i]).abs() < 1e-9, "elem {i}: {} != {}", a[i], b[i]);
    }
}

fn video_clip(id: &str, start: i32, dur: i32) -> Clip {
    Clip::new(id, "asset", start, dur)
}

fn single_video_timeline(clip: Clip) -> Timeline {
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 1920;
    tl.height = 1080;
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(clip);
    tl.tracks.push(track);
    tl
}

const RS: RenderSize = RenderSize {
    width: 1920,
    height: 1080,
};

// --- Single clip, no transform: full-canvas identity-ish affine ---

#[test]
fn single_clip_full_canvas_no_transform() {
    // nat == render, default transform -> affine = [render/nat, 0, 0, ..., 0, 0]
    // = [1, 0, 0, 1, 0, 0].
    let tl = single_video_timeline(video_clip("c0", 0, 30));
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    assert_eq!(plan.clip_plans.len(), 1);
    assert_eq!(plan.fps, 30);
    assert_eq!(plan.total_frames, 30);

    let fp = plan.frame(&tl, 10);
    assert_eq!(fp.clear_rgba, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(fp.draws.len(), 1);
    let d = &fp.draws[0];
    approx_affine(d.affine, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    approx(d.opacity, 1.0);
    assert_eq!(d.crop_uv, (0.0, 0.0, 1.0, 1.0));
    assert_eq!(d.clip_id, "c0");
    assert!(matches!(d.source, TextureSource::Decoded { .. }));
}

#[test]
fn nat_smaller_than_render_scales_up() {
    let tl = single_video_timeline(video_clip("c0", 0, 30));
    let m = TestMetrics {
        nat: (960, 540),
        ..Default::default()
    };
    let plan = build_render_plan(&tl, RS, &m);
    let fp = plan.frame(&tl, 0);
    // sx = 1920/960 = 2, sy = 1080/540 = 2.
    approx_affine(fp.draws[0].affine, [2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
}

// --- Flip ---

#[test]
fn flip_horizontal_affine() {
    let mut clip = video_clip("c0", 0, 30);
    clip.transform = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    clip.transform.flip_horizontal = true;
    let tl = single_video_timeline(clip);
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    let fp = plan.frame(&tl, 0);
    let d = &fp.draws[0];
    // sx negative; tx = (0 + 1) * 1920.
    approx(d.affine[0], -1.0);
    approx(d.affine[4], 1920.0);
}

// --- Rotation matches direct affine_transform ---

#[test]
fn rotation_90_at_center_matches_affine_helper() {
    let mut clip = video_clip("c0", 0, 30);
    clip.transform = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    clip.transform.rotation = 90.0;
    let tl = single_video_timeline(clip.clone());
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    let fp = plan.frame(&tl, 0);

    let expected = affine_transform(&clip.transform_at(0), (1920.0, 1080.0), RS);
    approx_affine(fp.draws[0].affine, expected);
}

// --- Transform keyframes: per-frame eval == transform_at(mid) ---

#[test]
fn transform_keyframe_midframe_equals_transform_at() {
    let mut clip = video_clip("c0", 0, 20);
    clip.position_track = Some(KeyframeTrack::from_keyframes(vec![
        Keyframe::with_interpolation(0, AnimPair::new(0.0, 0.0), Interpolation::Linear),
        Keyframe::new(20, AnimPair::new(0.2, 0.4)),
    ]));
    clip.scale_track = Some(KeyframeTrack::from_keyframes(vec![
        Keyframe::with_interpolation(0, AnimPair::new(1.0, 1.0), Interpolation::Linear),
        Keyframe::new(20, AnimPair::new(1.0, 1.0)),
    ]));
    let tl = single_video_timeline(clip.clone());
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());

    // At frame 10 (midpoint) the per-frame value must equal the affine computed
    // from the domain's `transform_at(10)` — i.e. our per-frame eval == upstream
    // ramp endpoint sampled at the same frame.
    let fp = plan.frame(&tl, 10);
    let expected = affine_transform(&clip.transform_at(10), (1920.0, 1080.0), RS);
    approx_affine(fp.draws[0].affine, expected);

    // Sanity: the matrix at frame 10 differs from frame 0 (animation is live).
    let fp0 = plan.frame(&tl, 0);
    assert!(
        (fp.draws[0].affine[4] - fp0.draws[0].affine[4]).abs() > 1e-6,
        "tx should change across the animation"
    );
}

// --- Crop keyframes -> crop_uv changes per frame ---

#[test]
fn crop_keyframe_uv_tracks_frame() {
    let mut clip = video_clip("c0", 0, 20);
    clip.crop_track = Some(KeyframeTrack::from_keyframes(vec![
        Keyframe::with_interpolation(
            0,
            Crop {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
            Interpolation::Linear,
        ),
        Keyframe::new(
            20,
            Crop {
                left: 0.4,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
        ),
    ]));
    let tl = single_video_timeline(clip);
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());

    // frame 0: no crop -> u0 = 0.
    approx(plan.frame(&tl, 0).draws[0].crop_uv.0, 0.0);
    // frame 10 (midpoint, linear): left = 0.2 -> u0 = 0.2.
    approx(plan.frame(&tl, 10).draws[0].crop_uv.0, 0.2);
    // frame 19 (near end): left close to 0.38.
    let near_end = plan.frame(&tl, 19).draws[0].crop_uv.0;
    assert!(near_end > 0.2 && near_end < 0.4, "got {near_end}");
}

// --- Opacity fade-in ---

#[test]
fn opacity_fade_in_linear() {
    let mut clip = video_clip("c0", 0, 30);
    clip.media_type = ClipType::Video;
    clip.opacity = 1.0;
    clip.fade_in_frames = 10;
    let tl = single_video_timeline(clip);
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());

    // rel=0 -> fade 0 -> draw skipped (opacity <= 0).
    assert_eq!(plan.frame(&tl, 0).draws.len(), 0);
    // rel=5 -> linear 0.5.
    approx(plan.frame(&tl, 5).draws[0].opacity, 0.5);
    // rel=10 -> full.
    approx(plan.frame(&tl, 10).draws[0].opacity, 1.0);
}

#[test]
fn opacity_fade_in_smooth_uses_smoothstep() {
    let mut clip = video_clip("c0", 0, 30);
    clip.fade_in_frames = 10;
    clip.fade_in_interpolation = Interpolation::Smooth;
    let tl = single_video_timeline(clip);
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    // rel=2 -> t=0.2 -> smoothstep(0.2).
    approx(
        plan.frame(&tl, 2).draws[0].opacity,
        opentake_domain::smoothstep(0.2),
    );
}

// --- Clip-span hit test ---

#[test]
fn clip_outside_span_does_not_draw() {
    let tl = single_video_timeline(video_clip("c0", 10, 20)); // [10, 30)
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    assert_eq!(plan.frame(&tl, 9).draws.len(), 0); // before start
    assert_eq!(plan.frame(&tl, 10).draws.len(), 1); // at start
    assert_eq!(plan.frame(&tl, 29).draws.len(), 1); // last visible
    assert_eq!(plan.frame(&tl, 30).draws.len(), 0); // exclusive end
}

// --- Same-track overlap de-dup (upstream L152/L424) ---

#[test]
fn same_track_overlap_drops_later_clip() {
    let mut tl = Timeline::new();
    tl.fps = 30;
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(video_clip("a", 0, 30)); // [0, 30)
    track.clips.push(video_clip("b", 20, 30)); // [20, 50) overlaps a -> dropped
    track.clips.push(video_clip("c", 30, 30)); // [30, 60) abuts a -> kept
    tl.tracks.push(track);

    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    let ids: Vec<&str> = plan.clip_plans.iter().map(|p| p.clip_id.as_str()).collect();
    assert!(ids.contains(&"a"));
    assert!(!ids.contains(&"b"));
    assert!(ids.contains(&"c"));
}

#[test]
fn zero_duration_clip_skipped() {
    let mut tl = Timeline::new();
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(video_clip("z", 0, 0)); // duration 0
    tl.tracks.push(track);
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    assert!(plan.clip_plans.is_empty());
}

// --- Multi-track blend order: track 0 below track 1 ---

#[test]
fn multitrack_blend_order_track0_below_track1() {
    let mut tl = Timeline::new();
    tl.fps = 30;
    let mut t0 = Track::new("t0", ClipType::Video);
    t0.clips.push(video_clip("bottom", 0, 30));
    let mut t1 = Track::new("t1", ClipType::Video);
    t1.clips.push(video_clip("top", 0, 30));
    tl.tracks.push(t0);
    tl.tracks.push(t1);

    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    // clip_plans ordered by (track_index, start): bottom first, top second.
    assert_eq!(plan.clip_plans[0].clip_id, "bottom");
    assert_eq!(plan.clip_plans[1].clip_id, "top");

    let fp = plan.frame(&tl, 0);
    // draws preserve plan order: index 0 bottom, last on top.
    assert_eq!(fp.draws[0].clip_id, "bottom");
    assert_eq!(fp.draws[1].clip_id, "top");
}

// --- Hidden track excluded ---

#[test]
fn hidden_track_excluded() {
    let mut tl = Timeline::new();
    let mut t0 = Track::new("t0", ClipType::Video);
    t0.hidden = true;
    t0.clips.push(video_clip("hidden", 0, 30));
    let mut t1 = Track::new("t1", ClipType::Video);
    t1.clips.push(video_clip("shown", 0, 30));
    tl.tracks.push(t0);
    tl.tracks.push(t1);

    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    let ids: Vec<&str> = plan.clip_plans.iter().map(|p| p.clip_id.as_str()).collect();
    assert_eq!(ids, vec!["shown"]);
}

// --- Text clips go to text_plans, composite on top, not deduped ---

#[test]
fn text_clips_separate_and_on_top() {
    let mut tl = Timeline::new();
    tl.fps = 30;
    // Video on track 0.
    let mut t0 = Track::new("t0", ClipType::Video);
    t0.clips.push(video_clip("vid", 0, 30));
    tl.tracks.push(t0);
    // Two overlapping text clips on a text track — both must survive (no dedup).
    let mut t1 = Track::new("t1", ClipType::Text);
    let mut txt_a = Clip::new("txt_a", "", 0, 30);
    txt_a.media_type = ClipType::Text;
    let mut txt_b = Clip::new("txt_b", "", 10, 30); // overlaps txt_a
    txt_b.media_type = ClipType::Text;
    t1.clips.push(txt_a);
    t1.clips.push(txt_b);
    tl.tracks.push(t1);

    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    assert_eq!(plan.clip_plans.len(), 1); // only the video
    assert_eq!(plan.text_plans.len(), 2); // both text clips, overlap allowed

    // At frame 15 all three are visible; draws = video first, then text on top.
    let fp = plan.frame(&tl, 15);
    assert_eq!(fp.draws.len(), 3);
    assert_eq!(fp.draws[0].clip_id, "vid");
    assert!(matches!(fp.draws[1].source, TextureSource::Text { .. }));
    assert!(matches!(fp.draws[2].source, TextureSource::Text { .. }));
}

// --- Audio track contributes no video draw ---

#[test]
fn audio_track_no_video_plan() {
    let mut tl = Timeline::new();
    let mut t0 = Track::new("t0", ClipType::Audio);
    let mut a = Clip::new("a", "asset", 0, 30);
    a.media_type = ClipType::Audio;
    t0.clips.push(a);
    tl.tracks.push(t0);
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    assert!(plan.clip_plans.is_empty());
    assert!(plan.text_plans.is_empty());
    assert_eq!(plan.frame(&tl, 0).draws.len(), 0);
}

// --- Black background clear color always present ---

#[test]
fn clear_color_is_opaque_black() {
    let tl = Timeline::new(); // empty
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    let fp = plan.frame(&tl, 0);
    assert_eq!(fp.clear_rgba, [0.0, 0.0, 0.0, 1.0]);
    assert!(fp.draws.is_empty());
}

// --- Source frame index (SPEC §2.5, upstream insertClip L301-343) ---

#[test]
fn source_frame_video_with_trim_and_speed() {
    let mut clip = video_clip("c0", 100, 30);
    clip.trim_start_frame = 5;
    clip.speed = 2.0;
    let tl = single_video_timeline(clip);
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    let cp = &plan.clip_plans[0];
    // rel=0 -> trim + round(0*2) = 5.
    assert_eq!(source_frame_index(cp, 100), 5);
    // rel=10 -> 5 + round(10*2) = 25.
    assert_eq!(source_frame_index(cp, 110), 25);
}

#[test]
fn source_frame_round_half_away_from_zero() {
    let mut clip = video_clip("c0", 0, 30);
    clip.speed = 0.5;
    let tl = single_video_timeline(clip);
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    let cp = &plan.clip_plans[0];
    // rel=1 -> round(0.5) = 1 (away from zero).
    assert_eq!(source_frame_index(cp, 1), 1);
    // rel=3 -> round(1.5) = 2.
    assert_eq!(source_frame_index(cp, 3), 2);
}

#[test]
fn source_frame_image_is_zero_and_trim_floored() {
    let mut clip = Clip::new("img", "asset", 0, 30);
    clip.media_type = ClipType::Image;
    clip.trim_start_frame = -5; // image floors trim at max(0, ...)
    let mut tl = Timeline::new();
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(clip);
    tl.tracks.push(track);
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    let cp = &plan.clip_plans[0];
    // Image always references frame 0 regardless of rel.
    assert_eq!(source_frame_index(cp, 0), 0);
    assert_eq!(source_frame_index(cp, 20), 0);
}

#[test]
fn source_frame_lottie_wraps_modulo() {
    let mut clip = Clip::new("lot", "asset", 0, 100);
    clip.media_type = ClipType::Lottie;
    let mut tl = Timeline::new();
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(clip);
    tl.tracks.push(track);
    let m = TestMetrics {
        lottie_frames: Some(24),
        ..Default::default()
    };
    let plan = build_render_plan(&tl, RS, &m);
    let cp = &plan.clip_plans[0];
    assert_eq!(cp.lottie_frame_count, Some(24));
    // rel=0 -> 0.
    assert_eq!(source_frame_index(cp, 0), 0);
    // rel=25 -> 25 % 24 = 1.
    assert_eq!(source_frame_index(cp, 25), 1);
    // rel=48 -> 48 % 24 = 0.
    assert_eq!(source_frame_index(cp, 48), 0);
}

// --- Box normalization with a 90-degree display matrix (upstream L170-172) ---

#[test]
fn preferred_transform_box_normalization_rotates_nat_size() {
    // A 90-degree rotation display matrix on a 1920x1080 source should yield a
    // natural size of 1080x1920 (axes swapped), with the box re-origined to 0,0.
    // CG rotate(+90): [0, 1, -1, 0, 0, 0].
    let m = TestMetrics {
        nat: (1920, 1080),
        pt: [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
        ..Default::default()
    };
    let tl = single_video_timeline(video_clip("c0", 0, 30));
    let plan = build_render_plan(&tl, RS, &m);
    let cp = &plan.clip_plans[0];
    approx(cp.nat_size.0, 1080.0);
    approx(cp.nat_size.1, 1920.0);
}

// --- needs_premultiply flows from metrics for video, false for image ---

#[test]
fn premultiply_flag_from_metrics() {
    let tl = single_video_timeline(video_clip("c0", 0, 30));
    let m = TestMetrics {
        premultiply: true,
        ..Default::default()
    };
    let plan = build_render_plan(&tl, RS, &m);
    assert!(plan.clip_plans[0].needs_premultiply);
    assert!(plan.frame(&tl, 0).draws[0].needs_premultiply);
}

#[test]
fn image_never_premultiplied_even_if_metrics_say_so() {
    let mut clip = Clip::new("img", "asset", 0, 30);
    clip.media_type = ClipType::Image;
    let mut tl = Timeline::new();
    let mut track = Track::new("t0", ClipType::Video);
    track.clips.push(clip);
    tl.tracks.push(track);
    let m = TestMetrics {
        premultiply: true,
        ..Default::default()
    };
    let plan = build_render_plan(&tl, RS, &m);
    // Image is authored premultiplied -> flag stays false (SPEC §4.1).
    assert!(!plan.clip_plans[0].needs_premultiply);
}
