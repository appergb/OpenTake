//! Deterministic camera-motion smoothing for editable stabilization tracks.

use opentake_domain::{StabilizationKeyframe, StabilizationTrack};

use crate::{MediaCancelToken, MediaError, Result, RgbaFrame};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StabilizationMotionSample {
    pub frame: i32,
    /// Observed camera translation in normalized output-canvas coordinates.
    pub translation_x: f64,
    pub translation_y: f64,
    pub rotation_degrees: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedMotionRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegionMotionTrack {
    pub samples: Vec<StabilizationMotionSample>,
    pub minimum_confidence: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StabilizationConfig {
    /// Half-width of the centered moving-average window.
    pub smoothing_radius: usize,
}

impl Default for StabilizationConfig {
    fn default() -> Self {
        Self {
            smoothing_radius: 2,
        }
    }
}

/// Convert tracked camera motion into a non-destructive compensation track.
/// The analyzer never reads or writes the source media; callers own motion
/// extraction and persist the returned track through the edit command layer.
pub fn analyze_stabilization(
    samples: &[StabilizationMotionSample],
    source_identity: impl Into<String>,
    config: StabilizationConfig,
    cancel: &MediaCancelToken,
) -> Result<StabilizationTrack> {
    if cancel.checkpoint() {
        return Err(MediaError::Cancelled);
    }
    if samples.len() < 2 {
        return Err(MediaError::Decode(
            "stabilization requires at least two motion samples".to_string(),
        ));
    }
    let source_identity = source_identity.into();
    if source_identity.trim().is_empty() {
        return Err(MediaError::Decode(
            "stabilization source identity is required".to_string(),
        ));
    }
    for (index, sample) in samples.iter().enumerate() {
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        if index > 0 && sample.frame <= samples[index - 1].frame {
            return Err(MediaError::Decode(
                "stabilization motion frames must be strictly increasing".to_string(),
            ));
        }
        if !sample.translation_x.is_finite()
            || !sample.translation_y.is_finite()
            || !sample.rotation_degrees.is_finite()
        {
            return Err(MediaError::Decode(
                "stabilization motion samples must be finite".to_string(),
            ));
        }
    }

    let radius = config.smoothing_radius.min(samples.len() - 1);
    let keyframes = samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let start = index.saturating_sub(radius);
            let end = (index + radius + 1).min(samples.len());
            let count = (end - start) as f64;
            let smoothed_x = samples[start..end]
                .iter()
                .map(|entry| entry.translation_x)
                .sum::<f64>()
                / count;
            let smoothed_y = samples[start..end]
                .iter()
                .map(|entry| entry.translation_y)
                .sum::<f64>()
                / count;
            let smoothed_rotation = samples[start..end]
                .iter()
                .map(|entry| entry.rotation_degrees)
                .sum::<f64>()
                / count;
            StabilizationKeyframe {
                frame: sample.frame,
                translation_x: smoothed_x - sample.translation_x,
                translation_y: smoothed_y - sample.translation_y,
                rotation_degrees: smoothed_rotation - sample.rotation_degrees,
            }
        })
        .collect();

    Ok(StabilizationTrack {
        model: "opentake.motion-smoothing".to_string(),
        model_version: 1,
        source_identity,
        strength: 1.0,
        crop_margin: 0.0,
        keyframes,
    })
}

/// Track a dominant translation path from decoded frames using deterministic
/// luma block matching. Each frame is paired with its clip-relative timeline
/// frame so the returned motion can be turned directly into a persisted track.
pub fn track_translation_motion(
    frames: &[(i32, RgbaFrame)],
    cancel: &MediaCancelToken,
) -> Result<Vec<StabilizationMotionSample>> {
    if frames.len() < 2 {
        return Err(MediaError::Decode(
            "stabilization requires at least two decoded frames".to_string(),
        ));
    }
    let width = frames[0].1.width;
    let height = frames[0].1.height;
    if width < 24 || height < 24 {
        return Err(MediaError::Decode(
            "stabilization frames are too small for motion tracking".to_string(),
        ));
    }
    if frames
        .iter()
        .any(|(_, frame)| frame.width != width || frame.height != height)
    {
        return Err(MediaError::Decode(
            "stabilization frames must have consistent dimensions".to_string(),
        ));
    }

    let mut x = 0.0;
    let mut y = 0.0;
    let mut samples = vec![StabilizationMotionSample {
        frame: frames[0].0,
        translation_x: x,
        translation_y: y,
        rotation_degrees: 0.0,
    }];
    for pair in frames.windows(2) {
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        let (dx, dy) = estimate_translation(&pair[0].1, &pair[1].1, cancel)?;
        x += dx as f64 / width as f64;
        y += dy as f64 / height as f64;
        samples.push(StabilizationMotionSample {
            frame: pair[1].0,
            translation_x: x,
            translation_y: y,
            rotation_degrees: 0.0,
        });
    }
    Ok(samples)
}

/// Track the selected subject rectangle rather than the dominant full-frame
/// camera motion. The rectangle is normalized to the decoded frame and follows
/// the previous match, so a static background cannot overpower a small moving
/// subject. Translation samples are normalized to the full frame and can be
/// applied directly as position-keyframe deltas.
pub fn track_region_motion(
    frames: &[(i32, RgbaFrame)],
    region: NormalizedMotionRegion,
    cancel: &MediaCancelToken,
) -> Result<RegionMotionTrack> {
    if frames.len() < 2 {
        return Err(MediaError::Decode(
            "motion tracking requires at least two decoded frames".to_string(),
        ));
    }
    if ![region.x, region.y, region.width, region.height]
        .into_iter()
        .all(f64::is_finite)
        || region.x < 0.0
        || region.y < 0.0
        || region.width <= 0.0
        || region.height <= 0.0
        || region.x + region.width > 1.0
        || region.y + region.height > 1.0
    {
        return Err(MediaError::Decode(
            "motion tracking region must be a positive normalized rectangle inside the frame"
                .to_string(),
        ));
    }
    let width = frames[0].1.width;
    let height = frames[0].1.height;
    if frames
        .iter()
        .any(|(_, frame)| frame.width != width || frame.height != height)
    {
        return Err(MediaError::Decode(
            "motion tracking frames must have consistent dimensions".to_string(),
        ));
    }
    let region_width = (region.width * width as f64).round() as i32;
    let region_height = (region.height * height as f64).round() as i32;
    if region_width < 8 || region_height < 8 {
        return Err(MediaError::Decode(
            "motion tracking region is too small".to_string(),
        ));
    }
    let mut origin_x =
        ((region.x * width as f64).round() as i32).clamp(0, width as i32 - region_width);
    let mut origin_y =
        ((region.y * height as f64).round() as i32).clamp(0, height as i32 - region_height);
    let mut total_x = 0_i32;
    let mut total_y = 0_i32;
    let mut minimum_confidence = 1.0_f64;
    let mut samples = vec![StabilizationMotionSample {
        frame: frames[0].0,
        translation_x: 0.0,
        translation_y: 0.0,
        rotation_degrees: 0.0,
    }];
    for pair in frames.windows(2) {
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        let (dx, dy, confidence) = estimate_region_translation(
            &pair[0].1,
            &pair[1].1,
            origin_x,
            origin_y,
            region_width,
            region_height,
            cancel,
        )?;
        origin_x += dx;
        origin_y += dy;
        total_x += dx;
        total_y += dy;
        minimum_confidence = minimum_confidence.min(confidence);
        samples.push(StabilizationMotionSample {
            frame: pair[1].0,
            translation_x: total_x as f64 / width as f64,
            translation_y: total_y as f64 / height as f64,
            rotation_degrees: 0.0,
        });
    }
    Ok(RegionMotionTrack {
        samples,
        minimum_confidence,
    })
}

fn estimate_region_translation(
    previous: &RgbaFrame,
    current: &RgbaFrame,
    origin_x: i32,
    origin_y: i32,
    region_width: i32,
    region_height: i32,
    cancel: &MediaCancelToken,
) -> Result<(i32, i32, f64)> {
    const SEARCH: i32 = 8;
    const STEP: usize = 2;
    let mut min_luma = u16::MAX;
    let mut max_luma = 0_u16;
    for y in (0..region_height).step_by(STEP) {
        for x in (0..region_width).step_by(STEP) {
            let value = luma(previous, (origin_x + x) as u32, (origin_y + y) as u32);
            min_luma = min_luma.min(value);
            max_luma = max_luma.max(value);
        }
    }
    let texture = (max_luma.saturating_sub(min_luma) as f64 / 64.0).clamp(0.0, 1.0);
    let mut best = (u64::MAX, i32::MAX, i32::MAX, i32::MAX, 0, 0);
    for dy in -SEARCH..=SEARCH {
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        for dx in -SEARCH..=SEARCH {
            let candidate_x = origin_x + dx;
            let candidate_y = origin_y + dy;
            if candidate_x < 0
                || candidate_y < 0
                || candidate_x + region_width > current.width as i32
                || candidate_y + region_height > current.height as i32
            {
                continue;
            }
            let mut error = 0_u64;
            let mut count = 0_u64;
            for y in (0..region_height).step_by(STEP) {
                for x in (0..region_width).step_by(STEP) {
                    let a = luma(previous, (origin_x + x) as u32, (origin_y + y) as u32);
                    let b = luma(current, (candidate_x + x) as u32, (candidate_y + y) as u32);
                    error += a.abs_diff(b) as u64;
                    count += 1;
                }
            }
            let normalized = error.checked_div(count).unwrap_or(u64::MAX);
            let candidate = (normalized, dx.abs() + dy.abs(), dy.abs(), dx.abs(), dx, dy);
            if candidate < best {
                best = candidate;
            }
        }
    }
    if best.0 == u64::MAX {
        return Err(MediaError::Decode(
            "motion tracking region left the frame".to_string(),
        ));
    }
    let match_quality = (1.0 - best.0 as f64 / 255.0).clamp(0.0, 1.0);
    Ok((best.4, best.5, match_quality * texture))
}

fn estimate_translation(
    previous: &RgbaFrame,
    current: &RgbaFrame,
    cancel: &MediaCancelToken,
) -> Result<(i32, i32)> {
    const SEARCH: i32 = 8;
    const STEP: usize = 8;
    let width = current.width as i32;
    let height = current.height as i32;
    let mut best = (u64::MAX, i32::MAX, i32::MAX, i32::MAX, 0, 0);
    for dy in -SEARCH..=SEARCH {
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        for dx in -SEARCH..=SEARCH {
            let mut error = 0_u64;
            let mut count = 0_u64;
            for y in ((SEARCH + 1) as usize..(height - SEARCH - 1) as usize).step_by(STEP) {
                for x in ((SEARCH + 1) as usize..(width - SEARCH - 1) as usize).step_by(STEP) {
                    let previous_x = x as i32 - dx;
                    let previous_y = y as i32 - dy;
                    let a = luma(previous, previous_x as u32, previous_y as u32);
                    let b = luma(current, x as u32, y as u32);
                    error += a.abs_diff(b) as u64;
                    count += 1;
                }
            }
            let normalized = error.checked_div(count).unwrap_or(u64::MAX);
            let candidate = (normalized, dx.abs() + dy.abs(), dy.abs(), dx.abs(), dx, dy);
            if candidate < best {
                best = candidate;
            }
        }
    }
    Ok((best.4, best.5))
}

fn luma(frame: &RgbaFrame, x: u32, y: u32) -> u16 {
    let offset = ((y * frame.width + x) * 4) as usize;
    let r = frame.rgba[offset] as u16;
    let g = frame.rgba[offset + 1] as u16;
    let b = frame.rgba[offset + 2] as u16;
    (54 * r + 183 * g + 19 * b) >> 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_pre_cancelled_analysis_before_work() {
        let cancel = MediaCancelToken::new();
        cancel.cancel();
        let result = analyze_stabilization(
            &[
                StabilizationMotionSample {
                    frame: 0,
                    translation_x: 0.0,
                    translation_y: 0.0,
                    rotation_degrees: 0.0,
                },
                StabilizationMotionSample {
                    frame: 1,
                    translation_x: 0.1,
                    translation_y: 0.0,
                    rotation_degrees: 0.0,
                },
            ],
            "asset",
            StabilizationConfig::default(),
            &cancel,
        );
        assert!(matches!(result, Err(MediaError::Cancelled)));
    }

    #[test]
    fn block_match_tracks_known_translation() {
        let make_frame = |offset: u32| {
            let mut frame = RgbaFrame::black(64, 48);
            for y in 8..40 {
                for x in offset..offset + 24 {
                    let index = ((y * frame.width + x) * 4) as usize;
                    let value = ((x - offset) * 37 + y * 19 + (x - offset) * y * 3) as u8;
                    frame.rgba[index..index + 3].copy_from_slice(&[value, value, value]);
                }
            }
            frame
        };
        let samples = track_translation_motion(
            &[
                (0, make_frame(16)),
                (1, make_frame(20)),
                (2, make_frame(24)),
            ],
            &MediaCancelToken::new(),
        )
        .expect("track translated fixture");
        assert!(samples[1].translation_x > 0.0);
        assert!(samples[2].translation_x > samples[1].translation_x);
        assert_eq!(samples[2].translation_y, 0.0);
    }

    #[test]
    fn region_tracker_keeps_known_subject_center_within_five_pixels() {
        let make_frame = |offset_x: u32, offset_y: u32| {
            let mut frame = RgbaFrame::black(96, 72);
            for y in offset_y..offset_y + 20 {
                for x in offset_x..offset_x + 24 {
                    let index = ((y * frame.width + x) * 4) as usize;
                    let local_x = x - offset_x;
                    let local_y = y - offset_y;
                    frame.rgba[index..index + 3].copy_from_slice(&[
                        (local_x * 9 + local_y * 3) as u8,
                        (local_x * 2 + local_y * 11) as u8,
                        (local_x * 7 + local_y * 5) as u8,
                    ]);
                }
            }
            frame
        };
        let tracked = track_region_motion(
            &[
                (0, make_frame(20, 24)),
                (1, make_frame(24, 26)),
                (2, make_frame(28, 28)),
            ],
            NormalizedMotionRegion {
                x: 20.0 / 96.0,
                y: 24.0 / 72.0,
                width: 24.0 / 96.0,
                height: 20.0 / 72.0,
            },
            &MediaCancelToken::new(),
        )
        .expect("track selected subject");

        assert!(tracked.minimum_confidence >= 0.25);
        let final_sample = tracked.samples.last().expect("final sample");
        assert!((final_sample.translation_x * 96.0 - 8.0).abs() <= 5.0);
        assert!((final_sample.translation_y * 72.0 - 4.0).abs() <= 5.0);
    }
}
