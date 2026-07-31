//! Persisted, editable video-stabilization solution.
//!
//! The track is deliberately separate from the user's authored position/scale/
//! rotation keyframes. Renderers compose both tracks, so applying or resetting
//! stabilization never destroys manual animation or source media identity.

use serde::{Deserialize, Serialize};

fn default_strength() -> f64 {
    1.0
}

fn default_model_version() -> u32 {
    1
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StabilizationTransform {
    pub translation_x: f64,
    pub translation_y: f64,
    pub rotation_degrees: f64,
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StabilizationKeyframe {
    pub frame: i32,
    pub translation_x: f64,
    pub translation_y: f64,
    pub rotation_degrees: f64,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StabilizationTrack {
    pub model: String,
    #[serde(default = "default_model_version")]
    pub model_version: u32,
    pub source_identity: String,
    #[serde(default = "default_strength")]
    pub strength: f64,
    #[serde(default)]
    pub crop_margin: f64,
    #[serde(default)]
    pub keyframes: Vec<StabilizationKeyframe>,
}

impl StabilizationTrack {
    /// Linearly sample the correction track at one clip-relative frame.
    pub fn sample(&self, frame: i32) -> StabilizationTransform {
        let Some(first) = self.keyframes.first() else {
            return StabilizationTransform::default();
        };
        let strength = self.strength.clamp(0.0, 1.0);
        let raw = if frame <= first.frame {
            keyframe_transform(*first)
        } else if let Some(last) = self.keyframes.last().filter(|last| frame >= last.frame) {
            keyframe_transform(*last)
        } else {
            let pair = self
                .keyframes
                .windows(2)
                .find(|pair| frame >= pair[0].frame && frame <= pair[1].frame)
                .expect("a sorted stabilization track covers an interior sample");
            let span = (pair[1].frame - pair[0].frame).max(1) as f64;
            let t = (frame - pair[0].frame) as f64 / span;
            StabilizationTransform {
                translation_x: lerp(pair[0].translation_x, pair[1].translation_x, t),
                translation_y: lerp(pair[0].translation_y, pair[1].translation_y, t),
                rotation_degrees: lerp(pair[0].rotation_degrees, pair[1].rotation_degrees, t),
            }
        };
        StabilizationTransform {
            translation_x: raw.translation_x * strength,
            translation_y: raw.translation_y * strength,
            rotation_degrees: raw.rotation_degrees * strength,
        }
    }

    /// Conservative uniform zoom needed to keep every output corner covered.
    /// `aspect_ratio` is output width / height.
    pub fn crop_scale(&self, aspect_ratio: f64) -> f64 {
        let aspect = aspect_ratio.max(1e-6);
        let required = self
            .keyframes
            .iter()
            .map(|keyframe| {
                let correction = self.sample(keyframe.frame);
                coverage_scale(correction, aspect)
            })
            .fold(1.0_f64, f64::max);
        required + self.crop_margin.max(0.0) * 2.0
    }

    pub fn guarantees_coverage(&self, aspect_ratio: f64) -> bool {
        let scale = self.crop_scale(aspect_ratio);
        self.keyframes.iter().all(|keyframe| {
            scale + 1e-12 >= coverage_scale(self.sample(keyframe.frame), aspect_ratio.max(1e-6))
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.model.trim().is_empty() || self.model_version == 0 {
            return Err("stabilization model and version are required".to_string());
        }
        if self.source_identity.trim().is_empty() {
            return Err("stabilization source identity is required".to_string());
        }
        if !(0.0..=1.0).contains(&self.strength) || !self.strength.is_finite() {
            return Err("stabilization strength must be finite and within 0..=1".to_string());
        }
        if !(0.0..=0.5).contains(&self.crop_margin) || !self.crop_margin.is_finite() {
            return Err("stabilization crop margin must be finite and within 0..=0.5".to_string());
        }
        if self.keyframes.len() < 2 {
            return Err("stabilization requires at least two keyframes".to_string());
        }
        let mut previous = None;
        for keyframe in &self.keyframes {
            if previous.is_some_and(|frame| keyframe.frame <= frame) {
                return Err("stabilization keyframes must be strictly increasing".to_string());
            }
            if !keyframe.translation_x.is_finite()
                || !keyframe.translation_y.is_finite()
                || !keyframe.rotation_degrees.is_finite()
            {
                return Err("stabilization keyframes must be finite".to_string());
            }
            previous = Some(keyframe.frame);
        }
        Ok(())
    }
}

fn keyframe_transform(keyframe: StabilizationKeyframe) -> StabilizationTransform {
    StabilizationTransform {
        translation_x: keyframe.translation_x,
        translation_y: keyframe.translation_y,
        rotation_degrees: keyframe.rotation_degrees,
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn coverage_scale(correction: StabilizationTransform, aspect: f64) -> f64 {
    let radians = correction.rotation_degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let sin = sin.abs();
    let cos = cos.abs();
    let translation_x = correction.translation_x.abs();
    let translation_y = correction.translation_y.abs();
    let cover_width = cos + sin / aspect + 2.0 * (translation_x + translation_y / aspect);
    let cover_height = cos + sin * aspect + 2.0 * (translation_y + translation_x * aspect);
    cover_width.max(cover_height).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampling_scales_correction_by_editable_strength() {
        let track = StabilizationTrack {
            model: "test".into(),
            model_version: 1,
            source_identity: "asset".into(),
            strength: 0.5,
            crop_margin: 0.0,
            keyframes: vec![
                StabilizationKeyframe::default(),
                StabilizationKeyframe {
                    frame: 10,
                    translation_x: 0.2,
                    translation_y: -0.1,
                    rotation_degrees: 4.0,
                },
            ],
        };
        let sample = track.sample(5);
        assert!((sample.translation_x - 0.05).abs() < 1e-12);
        assert!((sample.translation_y + 0.025).abs() < 1e-12);
        assert!((sample.rotation_degrees - 1.0).abs() < 1e-12);
        assert!(track.guarantees_coverage(16.0 / 9.0));
    }
}
