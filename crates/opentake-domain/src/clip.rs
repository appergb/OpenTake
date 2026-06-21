//! Clip value type and all derived sampling logic. 1:1 port of `Clip` and
//! `VolumeScale` from upstream `Timeline.swift` / `InspectorView.swift`.
//!
//! Conventions preserved exactly from upstream:
//! - Frames are `i32`. Timeline span is half-open `[start_frame, end_frame)`.
//! - `round()` is half-away-from-zero (Rust `f64::round` == Swift `.rounded()`).
//! - `source_frames_consumed = round(duration * speed)`.
//! - Keyframe storage is clip-relative; sampling clamps at endpoints.
//! - `volume_at` = static volume * dB keyframe gain * fade envelope.
//! - Speed has a hard floor of `0.0001` in `timeline_frame`.

use serde::{Deserialize, Serialize};

use crate::clip_type::ClipType;
use crate::keyframe::{AnimPair, Interpolation, KeyframeTrack};
use crate::text::TextStyle;
use crate::transform::{Crop, Point, Transform};

/// Linear amplitude <-> dB mapping for the volume slider. 1:1 port of
/// upstream `VolumeScale`. Below the floor we snap to true 0 (hard mute).
pub struct VolumeScale;

impl VolumeScale {
    pub const FLOOR_DB: f64 = -60.0;
    pub const CEILING_DB: f64 = 15.0;

    pub fn db_from_linear(linear: f64) -> f64 {
        if linear > 0.0 {
            Self::CEILING_DB.min(Self::FLOOR_DB.max(20.0 * linear.log10()))
        } else {
            Self::FLOOR_DB
        }
    }

    pub fn linear_from_db(db: f64) -> f64 {
        if db > Self::FLOOR_DB {
            10f64.powf(db.min(Self::CEILING_DB) / 20.0)
        } else {
            0.0
        }
    }
}

/// Which edge a fade applies to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FadeEdge {
    Left,
    Right,
}

fn default_speed() -> f64 {
    1.0
}
fn default_volume() -> f64 {
    1.0
}
fn default_opacity() -> f64 {
    1.0
}
fn default_linear() -> Interpolation {
    Interpolation::Linear
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub media_ref: String,
    #[serde(default)]
    pub media_type: ClipType,
    /// Original media type for derived clips; used for color-coding.
    #[serde(default)]
    pub source_clip_type: ClipType,
    pub start_frame: i32,
    pub duration_frames: i32,
    #[serde(default)]
    pub trim_start_frame: i32,
    #[serde(default)]
    pub trim_end_frame: i32,
    #[serde(default = "default_speed")]
    pub speed: f64,
    #[serde(default = "default_volume")]
    pub volume: f64,
    #[serde(default)]
    pub fade_in_frames: i32,
    #[serde(default)]
    pub fade_out_frames: i32,
    #[serde(default = "default_linear")]
    pub fade_in_interpolation: Interpolation,
    #[serde(default = "default_linear")]
    pub fade_out_interpolation: Interpolation,
    #[serde(default = "default_opacity")]
    pub opacity: f64,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default)]
    pub crop: Crop,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_group_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_group_id: Option<String>,

    // Text clips only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_style: Option<TextStyle>,

    // Keyframe tracks for each animatable property. None when no animation exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity_track: Option<KeyframeTrack<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_track: Option<KeyframeTrack<AnimPair>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_track: Option<KeyframeTrack<AnimPair>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_track: Option<KeyframeTrack<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_track: Option<KeyframeTrack<Crop>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_track: Option<KeyframeTrack<f64>>,
}

impl Clip {
    /// Minimal constructor mirroring the upstream defaulted memberwise init.
    pub fn new(
        id: impl Into<String>,
        media_ref: impl Into<String>,
        start_frame: i32,
        duration_frames: i32,
    ) -> Self {
        Clip {
            id: id.into(),
            media_ref: media_ref.into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame,
            duration_frames,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            opacity: 1.0,
            transform: Transform::default(),
            crop: Crop::default(),
            link_group_id: None,
            caption_group_id: None,
            text_content: None,
            text_style: None,
            opacity_track: None,
            position_track: None,
            scale_track: None,
            rotation_track: None,
            crop_track: None,
            volume_track: None,
        }
    }

    /// Frame where this clip ends on the timeline (exclusive end).
    pub fn end_frame(&self) -> i32 {
        self.start_frame + self.duration_frames
    }

    /// Source frames consumed by the visible portion: `round(duration * speed)`.
    pub fn source_frames_consumed(&self) -> i32 {
        (self.duration_frames as f64 * self.speed).round() as i32
    }

    /// Total source frames the clip references, including both trims.
    pub fn source_duration_frames(&self) -> i32 {
        self.source_frames_consumed() + self.trim_start_frame + self.trim_end_frame
    }

    /// Half-open membership test: `start <= frame < end`.
    pub fn contains(&self, frame: i32) -> bool {
        frame >= self.start_frame && frame < self.end_frame()
    }

    /// Absolute timeline frame -> clip-relative offset used by track storage.
    fn keyframe_offset(&self, frame: i32) -> i32 {
        frame - self.start_frame
    }

    pub fn opacity_at(&self, frame: i32) -> f64 {
        let base = self.raw_opacity_at(frame);
        if self.media_type == ClipType::Audio
            || (self.fade_in_frames == 0 && self.fade_out_frames == 0)
        {
            return base;
        }
        base * self.fade_multiplier(frame)
    }

    /// Authored opacity without the fade envelope.
    pub fn raw_opacity_at(&self, frame: i32) -> f64 {
        match &self.opacity_track {
            Some(t) => t.sample(self.keyframe_offset(frame), self.opacity),
            None => self.opacity,
        }
    }

    pub fn rotation_at(&self, frame: i32) -> f64 {
        match &self.rotation_track {
            Some(t) => t.sample(self.keyframe_offset(frame), self.transform.rotation),
            None => self.transform.rotation,
        }
    }

    /// Sampled top-left (normalized canvas space) at `frame`.
    pub fn top_left_at(&self, frame: i32) -> Point {
        if let Some(track) = &self.position_track {
            if track.is_active() {
                let p = track.sample(self.keyframe_offset(frame), AnimPair::new(0.0, 0.0));
                return Point { x: p.a, y: p.b };
            }
        }
        let c = self.transform.center();
        let sz = self.size_at(frame);
        Point {
            x: c.x - sz.0 / 2.0,
            y: c.y - sz.1 / 2.0,
        }
    }

    /// Sampled `(width, height)` at `frame`.
    pub fn size_at(&self, frame: i32) -> (f64, f64) {
        let fallback = AnimPair::new(self.transform.width, self.transform.height);
        let s = match &self.scale_track {
            Some(t) => t.sample(self.keyframe_offset(frame), fallback),
            None => fallback,
        };
        (s.a, s.b)
    }

    /// Resolve the full Transform at `frame`.
    pub fn transform_at(&self, frame: i32) -> Transform {
        let tl = self.top_left_at(frame);
        let sz = self.size_at(frame);
        let mut t = Transform::from_top_left(tl, sz.0, sz.1);
        t.rotation = self.rotation_at(frame);
        t
    }

    pub fn has_transform_animation(&self) -> bool {
        self.position_track.as_ref().is_some_and(|t| t.is_active())
            || self.scale_track.as_ref().is_some_and(|t| t.is_active())
            || self.rotation_track.as_ref().is_some_and(|t| t.is_active())
    }

    pub fn crop_at(&self, frame: i32) -> Crop {
        match &self.crop_track {
            Some(t) => t.sample(self.keyframe_offset(frame), self.crop),
            None => self.crop,
        }
    }

    /// Live volume keyframe value in dB, or `None` when the frame is outside the
    /// clip or no active volume track exists. Note: upstream uses the raw
    /// `frame - start_frame` offset here (same as `keyframe_offset`).
    pub fn live_volume_kf_db(&self, frame: i32) -> Option<f64> {
        if !self.contains(frame) {
            return None;
        }
        let track = self.volume_track.as_ref()?;
        if !track.is_active() {
            return None;
        }
        Some(track.sample(frame - self.start_frame, 0.0))
    }

    /// Effective linear volume: keyframe envelope (dB) first, fade ramp on top,
    /// static volume as outer gain.
    pub fn volume_at(&self, frame: i32) -> f64 {
        let kf_gain = match &self.volume_track {
            Some(t) if t.is_active() => {
                VolumeScale::linear_from_db(t.sample(self.keyframe_offset(frame), 0.0))
            }
            _ => 1.0,
        };
        self.volume * kf_gain * self.fade_multiplier(frame)
    }

    /// Linear volume without the fade envelope.
    pub fn raw_volume_at(&self, frame: i32) -> f64 {
        let kf_gain = match &self.volume_track {
            Some(t) if t.is_active() => {
                VolumeScale::linear_from_db(t.sample(self.keyframe_offset(frame), 0.0))
            }
            _ => 1.0,
        };
        self.volume * kf_gain
    }

    /// 0..=1 envelope from the fade head/tail ramps. `min(in, out)`. Returns 0
    /// outside `[0, duration_frames]` (closed interval, as upstream).
    pub fn fade_multiplier(&self, frame: i32) -> f64 {
        let rel = frame - self.start_frame;
        if rel < 0 || rel > self.duration_frames {
            return 0.0;
        }
        let in_mul = if self.fade_in_frames > 0 {
            let t = (rel as f64 / self.fade_in_frames as f64).min(1.0);
            if self.fade_in_interpolation == Interpolation::Smooth {
                crate::keyframe::smoothstep(t)
            } else {
                t
            }
        } else {
            1.0
        };
        let out_rem = self.duration_frames - rel;
        let out_mul = if self.fade_out_frames > 0 {
            let t = (out_rem as f64 / self.fade_out_frames as f64).min(1.0);
            if self.fade_out_interpolation == Interpolation::Smooth {
                crate::keyframe::smoothstep(t)
            } else {
                t
            }
        } else {
            1.0
        };
        in_mul.min(out_mul)
    }

    /// Source-seconds -> project-timeline-frame through this clip's placement,
    /// trim, and speed. Returns `None` when the result falls outside the clip.
    /// Speed is floored at `0.0001`.
    pub fn timeline_frame(&self, source_seconds: f64, fps: i32) -> Option<i32> {
        let source_frame = source_seconds * fps as f64;
        let offset_from_trim = source_frame - self.trim_start_frame as f64;
        if offset_from_trim < 0.0 {
            return None;
        }
        let frame =
            (self.start_frame as f64 + offset_from_trim / self.speed.max(0.0001)).round() as i32;
        if frame >= self.start_frame && frame < self.end_frame() {
            Some(frame)
        } else {
            None
        }
    }

    // MARK: - Mutation: fades

    /// Clamp fade ramps so head + tail can't exceed the clip's duration.
    pub fn clamp_fades_to_duration(&mut self) {
        self.fade_in_frames = self.fade_in_frames.max(0).min(self.duration_frames);
        self.fade_out_frames = self
            .fade_out_frames
            .max(0)
            .min(self.duration_frames - self.fade_in_frames);
    }

    /// Set the fade length for one edge and clamp to fit.
    pub fn set_fade(&mut self, edge: FadeEdge, frames: i32) {
        let v = frames.max(0);
        match edge {
            FadeEdge::Left => self.fade_in_frames = v,
            FadeEdge::Right => self.fade_out_frames = v,
        }
        self.clamp_fades_to_duration();
    }

    pub fn set_fade_interpolation(&mut self, edge: FadeEdge, interpolation: Interpolation) {
        match edge {
            FadeEdge::Left => self.fade_in_interpolation = interpolation,
            FadeEdge::Right => self.fade_out_interpolation = interpolation,
        }
    }

    pub fn fade_frames(&self, edge: FadeEdge) -> i32 {
        match edge {
            FadeEdge::Left => self.fade_in_frames,
            FadeEdge::Right => self.fade_out_frames,
        }
    }

    pub fn fade_interpolation(&self, edge: FadeEdge) -> Interpolation {
        match edge {
            FadeEdge::Left => self.fade_in_interpolation,
            FadeEdge::Right => self.fade_out_interpolation,
        }
    }

    pub fn set_duration(&mut self, new_duration: i32) {
        self.duration_frames = new_duration;
        self.clamp_keyframes_to_duration();
        self.clamp_fades_to_duration();
    }

    // MARK: - Mutation: keyframe tracks

    /// Drops volume keyframes outside `[0, duration_frames]`.
    pub fn clamp_volume_kfs_to_duration(&mut self) {
        self.volume_track = clamped_track(self.volume_track.take(), self.duration_frames);
    }

    /// Drops keyframes past `[0, duration_frames]` on every track. Call after any
    /// mutation that shrinks the clip.
    pub fn clamp_keyframes_to_duration(&mut self) {
        let d = self.duration_frames;
        self.opacity_track = clamped_track(self.opacity_track.take(), d);
        self.position_track = clamped_track(self.position_track.take(), d);
        self.scale_track = clamped_track(self.scale_track.take(), d);
        self.rotation_track = clamped_track(self.rotation_track.take(), d);
        self.crop_track = clamped_track(self.crop_track.take(), d);
        self.volume_track = clamped_track(self.volume_track.take(), d);
    }

    /// Rescale every keyframe's frame by `scale` (`round(frame * scale)`). A
    /// non-finite or non-positive scale leaves tracks untouched.
    pub fn rescale_keyframes(&mut self, scale: f64) {
        self.opacity_track = rescaled_track(self.opacity_track.take(), scale);
        self.position_track = rescaled_track(self.position_track.take(), scale);
        self.scale_track = rescaled_track(self.scale_track.take(), scale);
        self.rotation_track = rescaled_track(self.rotation_track.take(), scale);
        self.crop_track = rescaled_track(self.crop_track.take(), scale);
        self.volume_track = rescaled_track(self.volume_track.take(), scale);
    }
}

/// Keep only keyframes within the closed interval `[0, duration]`; collapse an
/// emptied track to `None`. Re-sorts via `upsert` exactly like upstream.
fn clamped_track<V: Clone + PartialEq + std::fmt::Debug>(
    track: Option<KeyframeTrack<V>>,
    duration: i32,
) -> Option<KeyframeTrack<V>> {
    let track = track?;
    let mut normalized = KeyframeTrack::<V>::new();
    for kf in track.keyframes.into_iter() {
        if kf.frame >= 0 && kf.frame <= duration {
            normalized.upsert(kf);
        }
    }
    if normalized.keyframes.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

/// Rescale frames by `scale` with half-away-from-zero rounding. Invalid scale
/// (`<= 0` or non-finite) returns the track unchanged.
fn rescaled_track<V: Clone + PartialEq + std::fmt::Debug>(
    track: Option<KeyframeTrack<V>>,
    scale: f64,
) -> Option<KeyframeTrack<V>> {
    let existing = track?;
    if !scale.is_finite() || scale <= 0.0 {
        return Some(existing);
    }
    let mut normalized = KeyframeTrack::<V>::new();
    for mut kf in existing.keyframes.into_iter() {
        kf.frame = (kf.frame as f64 * scale).round() as i32;
        normalized.upsert(kf);
    }
    if normalized.keyframes.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyframe::Keyframe;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    fn base_clip() -> Clip {
        Clip::new("c1", "asset1", 100, 30)
    }

    // --- Derived geometry/frames ---

    #[test]
    fn end_frame_is_start_plus_duration() {
        assert_eq!(base_clip().end_frame(), 130);
    }

    #[test]
    fn source_frames_consumed_rounds_half_away_from_zero() {
        let mut c = base_clip();
        c.duration_frames = 30;
        c.speed = 1.5;
        assert_eq!(c.source_frames_consumed(), 45);
        // 10 * 0.25 = 2.5 -> rounds to 3 (away from zero)
        c.duration_frames = 10;
        c.speed = 0.25;
        assert_eq!(c.source_frames_consumed(), 3);
    }

    #[test]
    fn source_duration_includes_both_trims() {
        let mut c = base_clip();
        c.duration_frames = 30;
        c.speed = 1.0;
        c.trim_start_frame = 5;
        c.trim_end_frame = 7;
        assert_eq!(c.source_duration_frames(), 30 + 5 + 7);
    }

    #[test]
    fn contains_is_half_open() {
        let c = base_clip(); // [100, 130)
        assert!(!c.contains(99));
        assert!(c.contains(100));
        assert!(c.contains(129));
        assert!(!c.contains(130));
    }

    // --- Opacity ---

    #[test]
    fn opacity_at_without_fade_returns_static() {
        let mut c = base_clip();
        c.opacity = 0.8;
        approx(c.opacity_at(110), 0.8);
    }

    #[test]
    fn opacity_at_audio_ignores_fade() {
        let mut c = base_clip();
        c.media_type = ClipType::Audio;
        c.opacity = 0.5;
        c.fade_in_frames = 10;
        // audio short-circuits before fade
        approx(c.opacity_at(100), 0.5);
    }

    #[test]
    fn opacity_at_applies_fade_for_visual() {
        let mut c = base_clip();
        c.media_type = ClipType::Video;
        c.opacity = 1.0;
        c.fade_in_frames = 10;
        // rel=0 -> fade 0
        approx(c.opacity_at(100), 0.0);
        // rel=5, linear -> 0.5
        approx(c.opacity_at(105), 0.5);
        // rel=10 -> full
        approx(c.opacity_at(110), 1.0);
    }

    #[test]
    fn raw_opacity_at_samples_track() {
        let mut c = base_clip();
        c.opacity = 1.0;
        c.opacity_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::with_interpolation(0, 0.0, Interpolation::Linear),
            Keyframe::new(10, 1.0),
        ]));
        // offset = 105-100 = 5 -> 0.5
        approx(c.raw_opacity_at(105), 0.5);
    }

    // --- Fade multiplier ---

    #[test]
    fn fade_multiplier_zero_outside_clip() {
        let mut c = base_clip();
        c.fade_in_frames = 5;
        approx(c.fade_multiplier(99), 0.0); // rel=-1
        approx(c.fade_multiplier(131), 0.0); // rel=31 > duration 30
    }

    #[test]
    fn fade_multiplier_takes_min_of_in_out() {
        let mut c = base_clip(); // duration 30, [100,130)
        c.fade_in_frames = 10;
        c.fade_out_frames = 10;
        // rel=5 -> in=0.5, out=(30-5)/10 clamped 1.0 -> min 0.5
        approx(c.fade_multiplier(105), 0.5);
        // rel=27 -> in=1.0, out=(30-27)/10=0.3 -> min 0.3
        approx(c.fade_multiplier(127), 0.3);
    }

    #[test]
    fn fade_multiplier_smooth_uses_smoothstep() {
        let mut c = base_clip();
        c.fade_in_frames = 10;
        c.fade_in_interpolation = Interpolation::Smooth;
        // rel=2 -> t=0.2 -> smoothstep(0.2)
        approx(c.fade_multiplier(102), crate::keyframe::smoothstep(0.2));
    }

    #[test]
    fn fade_multiplier_closed_interval_at_duration() {
        let mut c = base_clip(); // duration 30
        c.fade_out_frames = 10;
        // rel = duration = 30 is allowed (closed); out_rem=0 -> 0.0
        approx(c.fade_multiplier(130), 0.0);
    }

    // --- Volume / dB ---

    #[test]
    fn volume_scale_db_from_linear() {
        approx(VolumeScale::db_from_linear(1.0), 0.0);
        // linear 0 -> floor
        approx(VolumeScale::db_from_linear(0.0), VolumeScale::FLOOR_DB);
        // negative -> floor
        approx(VolumeScale::db_from_linear(-1.0), VolumeScale::FLOOR_DB);
        // huge -> ceiling clamp
        approx(VolumeScale::db_from_linear(1000.0), VolumeScale::CEILING_DB);
    }

    #[test]
    fn volume_scale_linear_from_db() {
        approx(VolumeScale::linear_from_db(0.0), 1.0);
        // at/below floor -> hard 0
        approx(VolumeScale::linear_from_db(VolumeScale::FLOOR_DB), 0.0);
        approx(VolumeScale::linear_from_db(-100.0), 0.0);
        // ceiling clamp: linear_from_db(20) == linear_from_db(ceiling 15)
        approx(
            VolumeScale::linear_from_db(20.0),
            10f64.powf(VolumeScale::CEILING_DB / 20.0),
        );
        // -6 dB ~ 0.5012
        approx(VolumeScale::linear_from_db(-6.0), 10f64.powf(-6.0 / 20.0));
    }

    #[test]
    fn volume_scale_roundtrip_within_range() {
        for &db in &[-30.0, -6.0, 0.0, 6.0] {
            let lin = VolumeScale::linear_from_db(db);
            approx(VolumeScale::db_from_linear(lin), db);
        }
    }

    #[test]
    fn volume_at_static_only() {
        let mut c = base_clip();
        c.volume = 0.5;
        approx(c.volume_at(110), 0.5);
    }

    #[test]
    fn volume_at_combines_kf_db_and_fade() {
        let mut c = base_clip();
        c.volume = 1.0;
        // constant 0 dB keyframes -> gain 1.0
        c.volume_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::new(0, 0.0),
            Keyframe::new(30, 0.0),
        ]));
        c.fade_in_frames = 10;
        // rel=5 -> fade 0.5, gain 1.0 -> 0.5
        approx(c.volume_at(105), 0.5);
    }

    #[test]
    fn raw_volume_at_excludes_fade() {
        let mut c = base_clip();
        c.volume = 1.0;
        c.fade_in_frames = 10;
        c.volume_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::new(0, 0.0),
            Keyframe::new(30, 0.0),
        ]));
        // fade ignored -> 1.0 * gain(1.0)
        approx(c.raw_volume_at(105), 1.0);
    }

    #[test]
    fn live_volume_kf_db_requires_active_track_and_membership() {
        let mut c = base_clip();
        assert_eq!(c.live_volume_kf_db(110), None); // no track
        c.volume_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::new(0, -6.0),
            Keyframe::new(30, -6.0),
        ]));
        assert_eq!(c.live_volume_kf_db(99), None); // outside clip
        approx(c.live_volume_kf_db(110).unwrap(), -6.0);
    }

    // --- Transform / crop sampling ---

    #[test]
    fn transform_at_static_centers() {
        let mut c = base_clip();
        c.transform = Transform::from_center(Point { x: 0.5, y: 0.5 }, 0.4, 0.6);
        let t = c.transform_at(110);
        approx(t.width, 0.4);
        approx(t.height, 0.6);
        approx(t.center_x, 0.5);
    }

    #[test]
    fn transform_at_uses_position_and_scale_tracks() {
        let mut c = base_clip();
        c.position_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::with_interpolation(0, AnimPair::new(0.0, 0.0), Interpolation::Linear),
            Keyframe::new(10, AnimPair::new(0.2, 0.4)),
        ]));
        c.scale_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::with_interpolation(0, AnimPair::new(0.5, 0.5), Interpolation::Linear),
            Keyframe::new(10, AnimPair::new(1.0, 1.0)),
        ]));
        // offset 5: pos top-left (0.1,0.2), size (0.75,0.75)
        let t = c.transform_at(105);
        let tl = t.top_left();
        approx(tl.x, 0.1);
        approx(tl.y, 0.2);
        approx(t.width, 0.75);
    }

    #[test]
    fn rotation_at_falls_back_to_transform() {
        let mut c = base_clip();
        c.transform.rotation = 30.0;
        approx(c.rotation_at(110), 30.0);
    }

    #[test]
    fn crop_at_samples_or_falls_back() {
        let mut c = base_clip();
        c.crop = Crop {
            left: 0.1,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        };
        approx(c.crop_at(110).left, 0.1);
    }

    #[test]
    fn has_transform_animation_flag() {
        let mut c = base_clip();
        assert!(!c.has_transform_animation());
        c.rotation_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(0, 10.0)]));
        assert!(c.has_transform_animation());
    }

    // --- timeline_frame ---

    #[test]
    fn timeline_frame_basic() {
        let mut c = base_clip(); // start 100, dur 30, [100,130)
        c.speed = 1.0;
        c.trim_start_frame = 0;
        // source 1.0s * 30 fps = frame 30; offset 30; start+30=130 -> out of [100,130)
        assert_eq!(c.timeline_frame(1.0, 30), None);
        // 0.5s -> 15 -> 115 in range
        assert_eq!(c.timeline_frame(0.5, 30), Some(115));
    }

    #[test]
    fn timeline_frame_respects_trim_and_returns_none_before_trim() {
        let mut c = base_clip();
        c.speed = 1.0;
        c.trim_start_frame = 10;
        // source 0.1s*30=3 frames < trim 10 -> negative offset -> None
        assert_eq!(c.timeline_frame(0.1, 30), None);
        // source 0.5s*30=15; offset 15-10=5; 100+5=105
        assert_eq!(c.timeline_frame(0.5, 30), Some(105));
    }

    #[test]
    fn timeline_frame_speed_floor_no_div_by_zero() {
        let mut c = base_clip();
        c.speed = 0.0; // floored to 0.0001 internally
                       // offset 30/0.0001 = 300000 -> way past end -> None, but must not panic/inf
        assert_eq!(c.timeline_frame(1.0, 30), None);
    }

    #[test]
    fn timeline_frame_with_speed_two() {
        let mut c = base_clip();
        c.speed = 2.0;
        c.trim_start_frame = 0;
        // source 1.0s*30=30; offset/speed = 30/2 = 15; 100+15=115
        assert_eq!(c.timeline_frame(1.0, 30), Some(115));
    }

    // --- clamp / rescale ---

    #[test]
    fn clamp_keyframes_drops_out_of_range_and_nils_empty() {
        let mut c = base_clip(); // duration 30
        c.opacity_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::new(-5, 0.0),
            Keyframe::new(0, 0.1),
            Keyframe::new(30, 0.9),
            Keyframe::new(40, 1.0),
        ]));
        // a track that is entirely out of range -> becomes None
        c.rotation_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(100, 5.0)]));
        c.clamp_keyframes_to_duration();
        let op = c.opacity_track.as_ref().unwrap();
        assert_eq!(
            op.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            [0, 30]
        );
        assert!(c.rotation_track.is_none());
    }

    #[test]
    fn rescale_keyframes_rounds_frames() {
        let mut c = base_clip();
        c.opacity_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::new(0, 0.0),
            Keyframe::new(3, 0.5),
            Keyframe::new(10, 1.0),
        ]));
        c.rescale_keyframes(1.5);
        // 0->0, 3*1.5=4.5->5 (away from zero), 10*1.5=15
        let op = c.opacity_track.as_ref().unwrap();
        assert_eq!(
            op.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            [0, 5, 15]
        );
    }

    #[test]
    fn rescale_keyframes_invalid_scale_is_noop() {
        let mut c = base_clip();
        c.opacity_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(4, 0.0)]));
        c.rescale_keyframes(0.0);
        assert_eq!(c.opacity_track.as_ref().unwrap().keyframes[0].frame, 4);
        c.rescale_keyframes(f64::NAN);
        assert_eq!(c.opacity_track.as_ref().unwrap().keyframes[0].frame, 4);
    }

    // --- fades mutation ---

    #[test]
    fn clamp_fades_caps_head_then_tail() {
        let mut c = base_clip(); // duration 30
        c.fade_in_frames = 40;
        c.fade_out_frames = 40;
        c.clamp_fades_to_duration();
        assert_eq!(c.fade_in_frames, 30);
        assert_eq!(c.fade_out_frames, 0); // 30 - 30
    }

    #[test]
    fn set_fade_clamps() {
        let mut c = base_clip();
        c.set_fade(FadeEdge::Left, 100);
        assert_eq!(c.fade_in_frames, 30);
        c.set_fade(FadeEdge::Left, -5);
        assert_eq!(c.fade_in_frames, 0);
    }

    #[test]
    fn set_duration_reclamps_keyframes_and_fades() {
        let mut c = base_clip();
        c.fade_in_frames = 20;
        c.opacity_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::new(0, 0.0),
            Keyframe::new(25, 1.0),
        ]));
        c.set_duration(10);
        // kf at 25 now out of [0,10] -> dropped
        let op = c.opacity_track.as_ref().unwrap();
        assert_eq!(
            op.keyframes.iter().map(|k| k.frame).collect::<Vec<_>>(),
            [0]
        );
        assert_eq!(c.fade_in_frames, 10);
    }

    // --- serde ---

    #[test]
    fn clip_roundtrip_json_camel_case() {
        let mut c = base_clip();
        c.media_type = ClipType::Audio;
        c.trim_start_frame = 3;
        c.volume = 0.7;
        c.volume_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(0, -6.0)]));
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"mediaRef\":\"asset1\""));
        assert!(json.contains("\"trimStartFrame\":3"));
        assert!(json.contains("\"volumeTrack\""));
        let back: Clip = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn clip_decodes_with_missing_optional_fields() {
        // Only the required keys present; everything else falls back to defaults.
        let json = r#"{"id":"x","mediaRef":"m","startFrame":0,"durationFrames":12}"#;
        let c: Clip = serde_json::from_str(json).unwrap();
        assert_eq!(c.media_type, ClipType::Video);
        approx(c.speed, 1.0);
        approx(c.volume, 1.0);
        approx(c.opacity, 1.0);
        assert_eq!(c.fade_in_interpolation, Interpolation::Linear);
        assert_eq!(c.transform, Transform::default());
        assert!(c.opacity_track.is_none());
    }

    #[test]
    fn clip_does_not_emit_none_tracks() {
        let c = base_clip();
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("opacityTrack"));
        assert!(!json.contains("linkGroupId"));
    }
}
