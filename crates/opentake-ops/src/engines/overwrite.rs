//! Overwrite engine — pure functions for clearing a region of a track by
//! removing, trimming, or splitting the clips that overlap it.
//!
//! 1:1 port of upstream `OverwriteEngine.swift`. The only deviation: upstream's
//! `.split` action carries a freshly minted `rightId: UUID().uuidString`, but
//! `clearRegion` ignores it (it re-runs `splitClip`, which mints its own id).
//! To keep this engine pure and id-free, [`OverwriteAction::Split`] omits the
//! id; the caller's split path mints ids. All numeric outputs are identical.
//!
//! `round()` is half-away-from-zero (`f64::round` == Swift `.rounded()`).

use opentake_domain::Clip;

/// One mutation needed to clear part of a region on a track.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum OverwriteAction {
    /// Clip lies entirely inside the region — drop it.
    Remove { clip_id: String },

    /// Clip overlaps the region's left edge — trim its right edge.
    TrimEnd { clip_id: String, new_duration: i32 },

    /// Clip overlaps the region's right edge — trim its left edge.
    TrimStart {
        clip_id: String,
        new_start_frame: i32,
        new_trim_start: i32,
        new_duration: i32,
    },

    /// Clip spans the whole region — split it; the middle (the region) is later
    /// removed by the caller. Carries everything the caller needs to build the
    /// right half (its id is minted by the caller's split path).
    Split {
        clip_id: String,
        left_duration: i32,
        right_start_frame: i32,
        right_trim_start: i32,
        right_duration: i32,
    },
}

/// Pure functions for overwrite editing.
pub struct OverwriteEngine;

impl OverwriteEngine {
    /// Given a region `[region_start, region_end)` on a track, returns the actions
    /// needed to clear that region so a new clip can be placed there.
    ///
    /// Clips are inspected in the given order; the returned action order matches
    /// upstream so downstream id minting / undo grouping stays deterministic.
    pub fn compute_overwrite(
        clips: &[Clip],
        region_start: i32,
        region_end: i32,
    ) -> Vec<OverwriteAction> {
        if region_end <= region_start {
            return Vec::new();
        }
        let mut actions = Vec::new();

        for clip in clips {
            let cs = clip.start_frame;
            let ce = clip.end_frame();

            // Entirely outside the region.
            if ce <= region_start || cs >= region_end {
                continue;
            }

            if cs >= region_start && ce <= region_end {
                // Entirely inside — remove.
                actions.push(OverwriteAction::Remove {
                    clip_id: clip.id.clone(),
                });
            } else if cs < region_start && ce > region_end {
                // Spans the whole region — split.
                let left_duration = region_start - cs;
                let right_start_frame = region_end;
                let right_trim_start =
                    clip.trim_start_frame + ((region_end - cs) as f64 * clip.speed).round() as i32;
                let right_duration = ce - region_end;
                actions.push(OverwriteAction::Split {
                    clip_id: clip.id.clone(),
                    left_duration,
                    right_start_frame,
                    right_trim_start,
                    right_duration,
                });
            } else if cs < region_start {
                // Overlaps left side — trim right edge.
                let new_duration = region_start - cs;
                actions.push(OverwriteAction::TrimEnd {
                    clip_id: clip.id.clone(),
                    new_duration,
                });
            } else {
                // Overlaps right side — trim left edge.
                let trim_amount = region_end - cs;
                let new_start_frame = region_end;
                let new_trim_start =
                    clip.trim_start_frame + (trim_amount as f64 * clip.speed).round() as i32;
                let new_duration = ce - region_end;
                actions.push(OverwriteAction::TrimStart {
                    clip_id: clip.id.clone(),
                    new_start_frame,
                    new_trim_start,
                    new_duration,
                });
            }
        }

        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::Clip;

    fn clip(id: &str, start: i32, dur: i32) -> Clip {
        Clip::new(id, "asset", start, dur)
    }

    #[test]
    fn empty_region_yields_no_actions() {
        let clips = vec![clip("a", 0, 100)];
        assert!(OverwriteEngine::compute_overwrite(&clips, 50, 50).is_empty());
        assert!(OverwriteEngine::compute_overwrite(&clips, 60, 50).is_empty());
    }

    #[test]
    fn clip_outside_region_is_skipped() {
        // Region [100,200); clip ends at 100 (touching, exclusive) and one starts at 200.
        let clips = vec![clip("before", 0, 100), clip("after", 200, 50)];
        assert!(OverwriteEngine::compute_overwrite(&clips, 100, 200).is_empty());
    }

    #[test]
    fn clip_inside_region_is_removed() {
        let clips = vec![clip("inner", 110, 30)]; // [110,140) within [100,200)
        let actions = OverwriteEngine::compute_overwrite(&clips, 100, 200);
        assert_eq!(
            actions,
            vec![OverwriteAction::Remove {
                clip_id: "inner".into()
            }]
        );
    }

    #[test]
    fn left_overlap_trims_end() {
        // clip [50,150), region [100,200): keep [50,100) -> duration 50.
        let clips = vec![clip("c", 50, 100)];
        let actions = OverwriteEngine::compute_overwrite(&clips, 100, 200);
        assert_eq!(
            actions,
            vec![OverwriteAction::TrimEnd {
                clip_id: "c".into(),
                new_duration: 50
            }]
        );
    }

    #[test]
    fn right_overlap_trims_start_with_speed_rounding() {
        // clip [150,250) speed 2.0, region [100,200): keep [200,250).
        // trimAmount = end-cs = 200-150 = 50; new_trim_start = 0 + round(50*2)=100.
        let mut c = clip("c", 150, 100);
        c.speed = 2.0;
        let actions = OverwriteEngine::compute_overwrite(&[c], 100, 200);
        assert_eq!(
            actions,
            vec![OverwriteAction::TrimStart {
                clip_id: "c".into(),
                new_start_frame: 200,
                new_trim_start: 100,
                new_duration: 50,
            }]
        );
    }

    #[test]
    fn spanning_clip_is_split_with_speed_rounding() {
        // clip [0,300) speed 1.0, trimStart 10, region [100,200).
        // left_duration = 100-0 = 100; right_start = 200; right_trim_start = 10 + round((200-0)*1) = 210;
        // right_duration = 300-200 = 100.
        let mut c = clip("c", 0, 300);
        c.trim_start_frame = 10;
        let actions = OverwriteEngine::compute_overwrite(&[c], 100, 200);
        assert_eq!(
            actions,
            vec![OverwriteAction::Split {
                clip_id: "c".into(),
                left_duration: 100,
                right_start_frame: 200,
                right_trim_start: 210,
                right_duration: 100,
            }]
        );
    }

    #[test]
    fn half_away_from_zero_rounding_in_trim_start() {
        // clip [150,250) speed 0.25 region [100,200): trimAmount=50, round(50*0.25)=round(12.5)=13.
        let mut c = clip("c", 150, 100);
        c.speed = 0.25;
        c.trim_start_frame = 0;
        let actions = OverwriteEngine::compute_overwrite(&[c], 100, 200);
        match &actions[0] {
            OverwriteAction::TrimStart { new_trim_start, .. } => assert_eq!(*new_trim_start, 13),
            other => panic!("expected TrimStart, got {other:?}"),
        }
    }

    #[test]
    fn multiple_clips_keep_input_order() {
        let clips = vec![
            clip("inside", 110, 20),  // remove
            clip("left", 50, 60),     // [50,110) -> trim end? region [100,200): overlaps left
            clip("outside", 300, 10), // skip
        ];
        let actions = OverwriteEngine::compute_overwrite(&clips, 100, 200);
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], OverwriteAction::Remove { .. }));
        assert!(matches!(actions[1], OverwriteAction::TrimEnd { .. }));
    }
}
