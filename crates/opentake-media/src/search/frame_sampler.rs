//! Visual-dedup frame sampling for indexing. Port of
//! `Search/Indexing/FrameSampler.swift` + `LumaGrid`.
//!
//! Luma scene changes start new shots; a coverage floor keeps long static shots
//! represented. The luma fingerprint and mean-diff are pure
//! ([`luma_grid`]/[`luma_mean_diff`]); the candidate-time formula
//! ([`candidate_times`]) and the keep/shot decision ([`ShotDetector`]) are pure
//! state machines, all unit-tested. The ffmpeg decode is glued in
//! `sample_frames`.
//!
//! Subtle rule (SPEC §5.4): the luma fingerprint updates on **every** decoded
//! frame, but `last_kept_time` advances only when a frame is **kept**.

use std::path::Path;

use crate::decode::frame::{decode_frames_at, FrameRequest};
use crate::error::Result;
use crate::frame::RgbaFrame;

/// Sampler format version (bumped if the sampling algorithm changes).
pub const SAMPLER_VERSION: i32 = 1;
/// Luma grid is `LUMA_CELLS × LUMA_CELLS`.
pub const LUMA_CELLS: usize = 8;

/// Sampler tuning, port of `FrameSampler.Options`.
#[derive(Clone, Debug)]
pub struct SamplerOptions {
    pub candidate_interval: f64,
    pub coverage_floor: f64,
    pub promote_diff: f32,
    pub max_size: (u32, u32),
    pub high_res_edge: u32,
}

impl Default for SamplerOptions {
    fn default() -> Self {
        SamplerOptions {
            candidate_interval: 2.0,
            coverage_floor: 8.0,
            promote_diff: 12.0,
            max_size: (512, 512),
            high_res_edge: 3000,
        }
    }
}

/// A sampled frame kept for indexing.
pub struct SampledFrame {
    pub time_secs: f64,
    pub image: RgbaFrame,
    pub is_new_shot: bool,
}

/// Candidate timestamps for sampling, port of the `stride` logic
/// (`FrameSampler.swift:62-64`): `stride(from: interval/2, to: duration, by:
/// interval)` (strictly `< duration`); if empty, `[duration/2]`. Returns empty
/// only when `duration <= 0`.
pub fn candidate_times(duration: f64, interval: f64) -> Vec<f64> {
    if duration <= 0.0 || interval <= 0.0 {
        return Vec::new();
    }
    let mut times = Vec::new();
    let mut t = interval / 2.0;
    while t < duration {
        times.push(t);
        t += interval;
    }
    if times.is_empty() {
        times.push(duration / 2.0);
    }
    times
}

/// The effective sampling interval: doubled for high-resolution sources
/// (`max(|w|,|h|) >= high_res_edge`), per `FrameSampler.swift:48-52`.
pub fn effective_interval(opts: &SamplerOptions, width: u32, height: u32) -> f64 {
    if width.max(height) >= opts.high_res_edge {
        opts.candidate_interval * 2.0
    } else {
        opts.candidate_interval
    }
}

/// 8×8 mean-luma fingerprint. Downsamples `frame` to `LUMA_CELLS²` cells and
/// computes per-cell Rec.601 luma `0.299R + 0.587G + 0.114B`. Port of
/// `LumaGrid.compute` (`:94-110`).
pub fn luma_grid(frame: &RgbaFrame) -> [f32; LUMA_CELLS * LUMA_CELLS] {
    let n = LUMA_CELLS;
    let mut out = [0.0f32; LUMA_CELLS * LUMA_CELLS];
    if frame.width == 0 || frame.height == 0 || frame.rgba.is_empty() {
        return out;
    }
    // Average-pool each source region into one of the n×n cells.
    let w = frame.width as usize;
    let h = frame.height as usize;
    for cy in 0..n {
        for cx in 0..n {
            let x0 = cx * w / n;
            let x1 = ((cx + 1) * w / n).max(x0 + 1).min(w);
            let y0 = cy * h / n;
            let y1 = ((cy + 1) * h / n).max(y0 + 1).min(h);
            let mut sum = 0.0f32;
            let mut count = 0.0f32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let base = (y * w + x) * 4;
                    if base + 2 < frame.rgba.len() {
                        let r = frame.rgba[base] as f32;
                        let g = frame.rgba[base + 1] as f32;
                        let b = frame.rgba[base + 2] as f32;
                        sum += r * 0.299 + g * 0.587 + b * 0.114;
                        count += 1.0;
                    }
                }
            }
            out[cy * n + cx] = if count > 0.0 { sum / count } else { 0.0 };
        }
    }
    out
}

/// Mean absolute per-cell difference of two luma grids. Port of
/// `LumaGrid.meanDiff` (`:112-116`).
pub fn luma_mean_diff(
    a: &[f32; LUMA_CELLS * LUMA_CELLS],
    b: &[f32; LUMA_CELLS * LUMA_CELLS],
) -> f32 {
    let mut diff = 0.0f32;
    for i in 0..a.len() {
        diff += (a[i] - b[i]).abs();
    }
    diff / a.len() as f32
}

/// Decision outcome for one decoded frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    pub keep: bool,
    pub is_new_shot: bool,
}

/// Pure shot/keep state machine. Feed decoded frames in time order; it decides
/// whether each starts a new shot and whether it should be kept, applying the
/// "luma updates always, last_kept only on keep" rule.
pub struct ShotDetector {
    promote_diff: f32,
    coverage_floor: f64,
    last_grid: Option<[f32; LUMA_CELLS * LUMA_CELLS]>,
    last_kept_time: f64,
    last_time: f64,
}

impl ShotDetector {
    pub fn new(promote_diff: f32, coverage_floor: f64) -> Self {
        ShotDetector {
            promote_diff,
            coverage_floor,
            last_grid: None,
            last_kept_time: f64::NEG_INFINITY,
            last_time: f64::NEG_INFINITY,
        }
    }

    /// Process a decoded frame at `time` with luma `grid`. Returns `None` if the
    /// frame is a non-advancing duplicate (`time <= last_time`).
    pub fn observe(&mut self, time: f64, grid: [f32; LUMA_CELLS * LUMA_CELLS]) -> Option<Decision> {
        if time <= self.last_time {
            return None; // dedupe frames that don't advance
        }
        self.last_time = time;

        let is_new_shot = match self.last_grid {
            Some(last) => luma_mean_diff(&grid, &last) > self.promote_diff,
            None => true, // first frame always starts a shot
        };
        self.last_grid = Some(grid); // update on EVERY decoded frame

        let keep = is_new_shot || (time - self.last_kept_time >= self.coverage_floor);
        if keep {
            self.last_kept_time = time; // advance only on keep
        }
        Some(Decision { keep, is_new_shot })
    }
}

/// Stream visually distinct frames from `path` for indexing. Decodes candidate
/// times via ffmpeg, runs the [`ShotDetector`], and returns the kept frames.
pub fn sample_frames(
    path: &Path,
    duration_secs: f64,
    width: u32,
    height: u32,
    opts: &SamplerOptions,
) -> Result<Vec<SampledFrame>> {
    let interval = effective_interval(opts, width, height);
    let times = candidate_times(duration_secs, interval);
    if times.is_empty() {
        return Ok(Vec::new());
    }
    let tolerance = (interval / 2.0).max(1.0);
    let req = FrameRequest {
        time_secs: 0.0,
        max_size: opts.max_size,
        tolerance_secs: tolerance,
        apply_rotation: true,
    };

    let mut detector = ShotDetector::new(opts.promote_diff, opts.coverage_floor);
    let mut kept = Vec::new();
    for result in decode_frames_at(path, &times, &req) {
        let (actual, frame) = result?;
        let grid = luma_grid(&frame);
        if let Some(decision) = detector.observe(actual, grid) {
            if decision.keep {
                kept.push(SampledFrame {
                    time_secs: actual,
                    image: frame,
                    is_new_shot: decision.is_new_shot,
                });
            }
        }
    }
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- candidate_times ---

    #[test]
    fn candidate_times_strides_from_half_interval() {
        // duration 10, interval 2 → 1,3,5,7,9
        let t = candidate_times(10.0, 2.0);
        assert_eq!(t, vec![1.0, 3.0, 5.0, 7.0, 9.0]);
    }

    #[test]
    fn candidate_times_strictly_less_than_duration() {
        // interval/2 = 1, then 3 (<4), 5 would be >=4 stop → but duration 4
        let t = candidate_times(4.0, 2.0);
        assert_eq!(t, vec![1.0, 3.0]);
    }

    #[test]
    fn candidate_times_short_clip_falls_back_to_midpoint() {
        // duration 1, interval 2 → half-interval 1.0 not < 1.0 → empty → [0.5]
        let t = candidate_times(1.0, 2.0);
        assert_eq!(t, vec![0.5]);
    }

    #[test]
    fn candidate_times_zero_duration_empty() {
        assert!(candidate_times(0.0, 2.0).is_empty());
        assert!(candidate_times(-1.0, 2.0).is_empty());
    }

    // --- effective_interval ---

    #[test]
    fn high_res_doubles_interval() {
        let o = SamplerOptions::default();
        assert_eq!(effective_interval(&o, 4000, 2000), 4.0); // >=3000
        assert_eq!(effective_interval(&o, 1920, 1080), 2.0); // normal
        assert_eq!(effective_interval(&o, 2000, 3000), 4.0); // height triggers
    }

    // --- luma grid ---

    #[test]
    fn luma_grid_black_is_zero() {
        let g = luma_grid(&RgbaFrame::black(16, 16));
        assert!(g.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn luma_grid_white_is_255() {
        let f = RgbaFrame::new(16, 16, vec![255; 16 * 16 * 4]);
        let g = luma_grid(&f);
        for v in g {
            assert!((v - 255.0).abs() < 0.5, "got {v}");
        }
    }

    #[test]
    fn luma_grid_uses_rec601_coefficients() {
        // Pure red (255,0,0) → luma 0.299*255 ≈ 76.245.
        let f = RgbaFrame::new(8, 8, {
            let mut v = vec![0u8; 8 * 8 * 4];
            for px in v.chunks_exact_mut(4) {
                px[0] = 255;
                px[3] = 255;
            }
            v
        });
        let g = luma_grid(&f);
        assert!((g[0] - 76.245).abs() < 1.0, "got {}", g[0]);
    }

    #[test]
    fn luma_mean_diff_identical_is_zero() {
        let g = luma_grid(&RgbaFrame::new(8, 8, vec![128; 8 * 8 * 4]));
        assert_eq!(luma_mean_diff(&g, &g), 0.0);
    }

    #[test]
    fn luma_mean_diff_black_vs_white_is_255() {
        let black = luma_grid(&RgbaFrame::black(8, 8));
        let white = luma_grid(&RgbaFrame::new(8, 8, vec![255; 8 * 8 * 4]));
        assert!((luma_mean_diff(&black, &white) - 255.0).abs() < 1.0);
    }

    // --- ShotDetector ---

    fn flat_grid(v: f32) -> [f32; 64] {
        [v; 64]
    }

    #[test]
    fn detector_first_frame_is_new_shot_and_kept() {
        let mut d = ShotDetector::new(12.0, 8.0);
        let dec = d.observe(1.0, flat_grid(100.0)).unwrap();
        assert!(dec.is_new_shot);
        assert!(dec.keep);
    }

    #[test]
    fn detector_dedupes_non_advancing_time() {
        let mut d = ShotDetector::new(12.0, 8.0);
        d.observe(5.0, flat_grid(100.0)).unwrap();
        assert!(d.observe(5.0, flat_grid(100.0)).is_none());
        assert!(d.observe(4.0, flat_grid(100.0)).is_none());
    }

    #[test]
    fn detector_big_luma_change_starts_new_shot() {
        let mut d = ShotDetector::new(12.0, 8.0);
        d.observe(1.0, flat_grid(0.0)).unwrap(); // shot A
        let dec = d.observe(3.0, flat_grid(100.0)).unwrap(); // big jump
        assert!(dec.is_new_shot);
        assert!(dec.keep);
    }

    #[test]
    fn detector_small_change_within_coverage_floor_is_dropped() {
        let mut d = ShotDetector::new(12.0, 8.0);
        d.observe(1.0, flat_grid(50.0)).unwrap(); // kept (first), last_kept=1
                                                  // tiny change, only 2s later (< coverage 8) → not new shot, not kept.
        let dec = d.observe(3.0, flat_grid(51.0)).unwrap();
        assert!(!dec.is_new_shot);
        assert!(!dec.keep);
    }

    #[test]
    fn detector_coverage_floor_keeps_static_shot_periodically() {
        let mut d = ShotDetector::new(12.0, 8.0);
        d.observe(1.0, flat_grid(50.0)).unwrap(); // kept, last_kept=1
        d.observe(3.0, flat_grid(50.0)).unwrap(); // dropped (3-1<8)
                                                  // 10s after first keep → coverage floor triggers keep even with no change.
        let dec = d.observe(10.0, flat_grid(50.0)).unwrap();
        assert!(!dec.is_new_shot);
        assert!(dec.keep);
    }

    #[test]
    fn detector_luma_updates_even_on_dropped_frames() {
        // Frame B is dropped but updates the luma baseline; a later frame is
        // compared against B, not against the kept frame A.
        let mut d = ShotDetector::new(12.0, 8.0);
        d.observe(1.0, flat_grid(0.0)).unwrap(); // A kept, grid=0
                                                 // B at 2s: diff vs A = (small)? Use 5 → diff 5 < 12 not new; 2-1<8 dropped.
        let b = d.observe(2.0, flat_grid(5.0)).unwrap();
        assert!(!b.keep);
        // C at 3s: grid 10 → diff vs B(5) = 5 < 12 (NOT new). If it compared to
        // A(0) the diff would be 10, still <12, so use a sharper check:
        // C grid 16 → diff vs B(5)=11 (<12, not new) but vs A(0)=16 (>12 new).
        let c = d.observe(3.0, flat_grid(16.0)).unwrap();
        assert!(
            !c.is_new_shot,
            "must compare against last decoded frame B, not kept A"
        );
    }
}
