//! Ripple engine — pure functions for computing how clips shift after
//! insertions or deletions. 1:1 port of upstream `RippleEngine.swift`.
//!
//! No side effects: every function returns proposed shifts / merged ranges that
//! the caller applies. Frame ranges are half-open `[start, end)`.

use std::collections::HashSet;

use opentake_domain::Clip;

/// A proposed new start frame for a single clip, produced by the ripple engine
/// and applied by the caller. 1:1 port of `ClipShift`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClipShift {
    pub clip_id: String,
    pub new_start_frame: i32,
}

impl ClipShift {
    pub fn new(clip_id: impl Into<String>, new_start_frame: i32) -> Self {
        ClipShift {
            clip_id: clip_id.into(),
            new_start_frame,
        }
    }
}

/// A half-open `[start, end)` frame interval on a single track. 1:1 port of
/// `FrameRange`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FrameRange {
    pub start: i32,
    pub end: i32,
}

impl FrameRange {
    pub fn new(start: i32, end: i32) -> Self {
        FrameRange { start, end }
    }

    /// `end - start`.
    pub fn length(&self) -> i32 {
        self.end - self.start
    }
}

/// A user-selected empty gap on a single track. 1:1 port of `GapSelection`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GapSelection {
    pub track_index: usize,
    pub range: FrameRange,
}

/// Pure functions for ripple editing.
pub struct RippleEngine;

impl RippleEngine {
    /// After removing clips from a track, compute new start frames for remaining
    /// clips that should shift backward to close the gap.
    pub fn compute_ripple_shifts(clips: &[Clip], removed_ids: &HashSet<String>) -> Vec<ClipShift> {
        let removed_ranges: Vec<FrameRange> = clips
            .iter()
            .filter(|c| removed_ids.contains(&c.id))
            .map(|c| FrameRange::new(c.start_frame, c.end_frame()))
            .collect();
        let remaining: Vec<Clip> = clips
            .iter()
            .filter(|c| !removed_ids.contains(&c.id))
            .cloned()
            .collect();
        Self::compute_ripple_shifts_for_ranges(&remaining, &removed_ranges)
    }

    /// Shift clips leftward to close the gaps defined by `removed_ranges`. Used
    /// when ranges come from a different track (sync-locked ripple).
    pub fn compute_ripple_shifts_for_ranges(
        clips: &[Clip],
        removed_ranges: &[FrameRange],
    ) -> Vec<ClipShift> {
        let merged = Self::merge_ranges(removed_ranges);
        if merged.is_empty() {
            return Vec::new();
        }

        let mut sorted: Vec<&Clip> = clips.iter().collect();
        sorted.sort_by_key(|c| c.start_frame);

        let mut shifts = Vec::new();
        for clip in sorted {
            let shift: i32 = merged
                .iter()
                .filter(|r| r.end <= clip.start_frame)
                .map(|r| r.length())
                .sum();
            if shift > 0 {
                shifts.push(ClipShift::new(clip.id.clone(), clip.start_frame - shift));
            }
        }
        shifts
    }

    /// Push all clips at or after `insert_frame` forward by `push_amount` frames.
    pub fn compute_ripple_push(
        clips: &[Clip],
        insert_frame: i32,
        push_amount: i32,
        exclude_ids: &HashSet<String>,
    ) -> Vec<ClipShift> {
        clips
            .iter()
            .filter(|c| !exclude_ids.contains(&c.id) && c.start_frame >= insert_frame)
            .map(|c| ClipShift::new(c.id.clone(), c.start_frame + push_amount))
            .collect()
    }

    /// Merge overlapping/touching ranges. Sorted ascending by start; a range
    /// merges into the previous when `range.start <= last.end`.
    pub fn merge_ranges(ranges: &[FrameRange]) -> Vec<FrameRange> {
        let mut sorted: Vec<FrameRange> = ranges.to_vec();
        sorted.sort_by_key(|r| r.start);
        let mut merged: Vec<FrameRange> = Vec::new();
        for range in sorted {
            if let Some(last) = merged.last_mut() {
                if range.start <= last.end {
                    last.end = last.end.max(range.end);
                    continue;
                }
            }
            merged.push(range);
        }
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::Clip;

    fn clip(id: &str, start: i32, dur: i32) -> Clip {
        Clip::new(id, "asset", start, dur)
    }

    fn ids(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn merge_ranges_combines_overlapping_and_touching() {
        let r = RippleEngine::merge_ranges(&[
            FrameRange::new(0, 10),
            FrameRange::new(10, 20), // touching -> merges
            FrameRange::new(25, 30), // gap -> separate
            FrameRange::new(5, 8),   // inside first -> merged
        ]);
        assert_eq!(r, vec![FrameRange::new(0, 20), FrameRange::new(25, 30)]);
    }

    #[test]
    fn merge_ranges_empty() {
        assert!(RippleEngine::merge_ranges(&[]).is_empty());
    }

    #[test]
    fn compute_ripple_shifts_closes_gap_after_removal() {
        // Track: [0,30) a, [30,30) b at 30, [60,30) c. Remove a -> b,c shift left 30.
        let clips = vec![clip("a", 0, 30), clip("b", 30, 30), clip("c", 60, 30)];
        let shifts = RippleEngine::compute_ripple_shifts(&clips, &ids(&["a"]));
        assert_eq!(
            shifts,
            vec![ClipShift::new("b", 0), ClipShift::new("c", 30)]
        );
    }

    #[test]
    fn compute_ripple_shifts_no_shift_for_clips_before_removal() {
        // Remove c (last). a,b are before it -> no shift.
        let clips = vec![clip("a", 0, 30), clip("b", 30, 30), clip("c", 60, 30)];
        let shifts = RippleEngine::compute_ripple_shifts(&clips, &ids(&["c"]));
        assert!(shifts.is_empty());
    }

    #[test]
    fn compute_ripple_shifts_for_ranges_sums_preceding_lengths() {
        // Clip at 100; two removed ranges before it [0,10),[20,40) -> shift 10+20=30.
        let clips = vec![clip("x", 100, 10)];
        let shifts = RippleEngine::compute_ripple_shifts_for_ranges(
            &clips,
            &[FrameRange::new(0, 10), FrameRange::new(20, 40)],
        );
        assert_eq!(shifts, vec![ClipShift::new("x", 70)]);
    }

    #[test]
    fn compute_ripple_shifts_for_ranges_ignores_ranges_after_clip() {
        // Range [200,300) is after clip start 100 (end<=start fails) -> no shift.
        let clips = vec![clip("x", 100, 10)];
        let shifts =
            RippleEngine::compute_ripple_shifts_for_ranges(&clips, &[FrameRange::new(200, 300)]);
        assert!(shifts.is_empty());
    }

    #[test]
    fn compute_ripple_shifts_for_ranges_boundary_end_equals_start_counts() {
        // Range end exactly == clip.start_frame (100) -> counts (end <= start).
        let clips = vec![clip("x", 100, 10)];
        let shifts =
            RippleEngine::compute_ripple_shifts_for_ranges(&clips, &[FrameRange::new(50, 100)]);
        assert_eq!(shifts, vec![ClipShift::new("x", 50)]);
    }

    #[test]
    fn compute_ripple_push_moves_clips_at_or_after_insert() {
        let clips = vec![clip("a", 0, 30), clip("b", 50, 30), clip("c", 100, 30)];
        // insert at 50, push 20 -> b (50>=50) and c move; a stays.
        let shifts = RippleEngine::compute_ripple_push(&clips, 50, 20, &HashSet::new());
        assert_eq!(
            shifts,
            vec![ClipShift::new("b", 70), ClipShift::new("c", 120)]
        );
    }

    #[test]
    fn compute_ripple_push_respects_exclude() {
        let clips = vec![clip("a", 50, 30), clip("b", 60, 30)];
        let shifts = RippleEngine::compute_ripple_push(&clips, 0, 10, &ids(&["a"]));
        assert_eq!(shifts, vec![ClipShift::new("b", 70)]);
    }

    #[test]
    fn frame_range_length() {
        assert_eq!(FrameRange::new(10, 35).length(), 25);
    }
}
