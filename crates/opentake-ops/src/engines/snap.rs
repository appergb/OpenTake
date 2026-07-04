//! Snap engine — pure snapping math for timeline drags. 1:1 port of upstream
//! `SnapEngine.swift`, minus the platform-bound bits.
//!
//! Stripped from upstream (rebuilt elsewhere): `NSHapticFeedbackManager`
//! alignment feedback fires on a fresh snap — the render/UI layer triggers
//! platform haptics off [`SnapResult`]; this crate stays IO-free. Everything
//! numeric (target collection, sticky hysteresis, playhead priority, multi-probe
//! nearest) is preserved exactly.
//!
//! Constants from upstream `Snap` (`Utilities/Constants.swift`):
//! `threshold_pixels = 8.0`, `sticky_multiplier = 1.5`, `playhead_multiplier =
//! 1.5`. (The "2.5x" wording in an upstream comment is stale; the constant is
//! 1.5.) `base_threshold` is passed in pixels by the caller.

use std::collections::HashSet;

use opentake_domain::Track;

/// Upstream `Snap` constants.
pub mod consts {
    pub const THRESHOLD_PIXELS: f64 = 8.0;
    pub const STICKY_MULTIPLIER: f64 = 1.5;
    pub const PLAYHEAD_MULTIPLIER: f64 = 1.5;
}

/// What a snap target represents.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapKind {
    Playhead,
    ClipEdge,
}

/// A candidate frame a drag can snap to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SnapTarget {
    pub frame: i32,
    pub kind: SnapKind,
}

/// The result of a successful snap.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SnapResult {
    pub frame: i32,
    /// Which probe snapped (`0` = start, `duration` = end).
    pub probe_offset: i32,
    /// Snap-indicator pixel position (`frame * pixels_per_frame`).
    pub x: f64,
}

/// Mutable state persisted across drag events for sticky snap behavior. 1:1 port
/// of `SnapState`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SnapState {
    pub currently_snapped_to: Option<i32>,
    /// Which probe is sticky.
    pub current_probe_offset: i32,
}

impl SnapState {
    pub fn new() -> Self {
        SnapState::default()
    }
}

/// Pure snapping functions.
pub struct SnapEngine;

impl SnapEngine {
    /// Collects all clip edges, and optionally the playhead, as snap targets.
    /// Pass `exclude_clip_ids` to skip clips being dragged. Pass
    /// `include_playhead = true` when the playhead itself is NOT what's moving.
    pub fn collect_targets(
        tracks: &[Track],
        playhead_frame: i32,
        exclude_clip_ids: &HashSet<String>,
        include_playhead: bool,
    ) -> Vec<SnapTarget> {
        let mut targets = Vec::new();
        if include_playhead {
            targets.push(SnapTarget {
                frame: playhead_frame,
                kind: SnapKind::Playhead,
            });
        }
        for track in tracks {
            for clip in &track.clips {
                if exclude_clip_ids.contains(&clip.id) {
                    continue;
                }
                targets.push(SnapTarget {
                    frame: clip.start_frame,
                    kind: SnapKind::ClipEdge,
                });
                targets.push(SnapTarget {
                    frame: clip.end_frame(),
                    kind: SnapKind::ClipEdge,
                });
            }
        }
        targets
    }

    /// Snap position(s) to nearest target, with sticky behavior and playhead
    /// priority. Tests one or more probe positions (e.g. clip start and end)
    /// against all targets. Returns `None` when nothing is within threshold.
    ///
    /// `state` is updated in place to remember the sticky snap. A fresh snap (a
    /// `Some` result whose target differs from the prior sticky one) is where the
    /// UI layer should fire haptic feedback.
    pub fn find_snap(
        position: i32,
        probe_offsets: &[i32],
        targets: &[SnapTarget],
        state: &mut SnapState,
        base_threshold: f64,
        pixels_per_frame: f64,
    ) -> Option<SnapResult> {
        let base_frame_threshold = base_threshold / pixels_per_frame;

        // Sticky: stay snapped until moved past stickyMultiplier * threshold.
        if let Some(snapped) = state.currently_snapped_to {
            let hold_threshold = base_frame_threshold * consts::STICKY_MULTIPLIER;
            let probe_pos = position + state.current_probe_offset;
            if ((probe_pos - snapped) as f64).abs() <= hold_threshold
                && targets.iter().any(|t| t.frame == snapped)
            {
                return Some(SnapResult {
                    frame: snapped,
                    probe_offset: state.current_probe_offset,
                    x: snapped as f64 * pixels_per_frame,
                });
            }
            state.currently_snapped_to = None;
            state.current_probe_offset = 0;
        }

        // Find closest (probe, target) pair within per-kind thresholds.
        let mut best: Option<(i32, SnapTarget, f64)> = None;
        for &probe_offset in probe_offsets {
            let probe_pos = position + probe_offset;
            for target in targets {
                let threshold = match target.kind {
                    SnapKind::Playhead => base_frame_threshold * consts::PLAYHEAD_MULTIPLIER,
                    SnapKind::ClipEdge => base_frame_threshold,
                };
                let dist = ((probe_pos - target.frame) as f64).abs();
                if dist <= threshold && dist < best.map(|b| b.2).unwrap_or(f64::INFINITY) {
                    best = Some((probe_offset, *target, dist));
                }
            }
        }

        let (probe_offset, target, _dist) = best?;
        state.currently_snapped_to = Some(target.frame);
        state.current_probe_offset = probe_offset;
        Some(SnapResult {
            frame: target.frame,
            probe_offset,
            x: target.frame as f64 * pixels_per_frame,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, ClipType};

    fn track_with(clips: Vec<Clip>) -> Track {
        let mut t = Track::new("t", ClipType::Video);
        t.clips = clips;
        t
    }

    fn clip(id: &str, start: i32, dur: i32) -> Clip {
        Clip::new(id, "asset", start, dur)
    }

    #[test]
    fn collect_targets_gathers_edges_and_optional_playhead() {
        let tracks = vec![track_with(vec![clip("a", 0, 30), clip("b", 100, 30)])];
        let targets = SnapEngine::collect_targets(&tracks, 50, &HashSet::new(), true);
        // playhead(50) + a(0,30) + b(100,130)
        let frames: Vec<i32> = targets.iter().map(|t| t.frame).collect();
        assert_eq!(frames, vec![50, 0, 30, 100, 130]);
        assert_eq!(targets[0].kind, SnapKind::Playhead);
        assert_eq!(targets[1].kind, SnapKind::ClipEdge);
    }

    #[test]
    fn collect_targets_excludes_dragged_clip() {
        let tracks = vec![track_with(vec![clip("a", 0, 30), clip("b", 100, 30)])];
        let excl: HashSet<String> = ["a".to_string()].into_iter().collect();
        let targets = SnapEngine::collect_targets(&tracks, 0, &excl, false);
        let frames: Vec<i32> = targets.iter().map(|t| t.frame).collect();
        assert_eq!(frames, vec![100, 130]); // only b's edges
    }

    #[test]
    fn find_snap_picks_nearest_clip_edge() {
        // 1 px/frame, threshold 8 frames. Target at 100, probe at 105 -> dist 5 <= 8 -> snap.
        let targets = vec![SnapTarget {
            frame: 100,
            kind: SnapKind::ClipEdge,
        }];
        let mut state = SnapState::new();
        let r = SnapEngine::find_snap(105, &[0], &targets, &mut state, 8.0, 1.0).unwrap();
        assert_eq!(r.frame, 100);
        assert_eq!(r.probe_offset, 0);
        assert_eq!(r.x, 100.0);
        assert_eq!(state.currently_snapped_to, Some(100));
    }

    #[test]
    fn find_snap_returns_none_when_out_of_threshold() {
        let targets = vec![SnapTarget {
            frame: 100,
            kind: SnapKind::ClipEdge,
        }];
        let mut state = SnapState::new();
        // probe 120 -> dist 20 > 8 -> none.
        assert!(SnapEngine::find_snap(120, &[0], &targets, &mut state, 8.0, 1.0).is_none());
        assert_eq!(state.currently_snapped_to, None);
    }

    #[test]
    fn find_snap_sticky_holds_within_1_5x() {
        let targets = vec![SnapTarget {
            frame: 100,
            kind: SnapKind::ClipEdge,
        }];
        let mut state = SnapState {
            currently_snapped_to: Some(100),
            current_probe_offset: 0,
        };
        // hold threshold = 8 * 1.5 = 12 frames. probe 111 -> dist 11 <= 12 -> stays snapped.
        let r = SnapEngine::find_snap(111, &[0], &targets, &mut state, 8.0, 1.0).unwrap();
        assert_eq!(r.frame, 100);
        // probe 113 -> dist 13 > 12 -> releases; 113 also out of base 8 -> none.
        let r2 = SnapEngine::find_snap(113, &[0], &targets, &mut state, 8.0, 1.0);
        assert!(r2.is_none());
        assert_eq!(state.currently_snapped_to, None);
    }

    #[test]
    fn find_snap_playhead_has_wider_threshold() {
        // playhead at 100 with 1.5x threshold (12). probe 110 dist 10 -> snaps to playhead.
        let targets = vec![SnapTarget {
            frame: 100,
            kind: SnapKind::Playhead,
        }];
        let mut state = SnapState::new();
        let r = SnapEngine::find_snap(110, &[0], &targets, &mut state, 8.0, 1.0).unwrap();
        assert_eq!(r.frame, 100);
    }

    #[test]
    fn find_snap_multi_probe_uses_end_edge() {
        // Dragging a clip of duration 30: probes [0,30]. Target at 130, position 105.
        // probe 0 -> 105 vs 130 dist 25 (too far); probe 30 -> 135 vs 130 dist 5 -> snap.
        let targets = vec![SnapTarget {
            frame: 130,
            kind: SnapKind::ClipEdge,
        }];
        let mut state = SnapState::new();
        let r = SnapEngine::find_snap(105, &[0, 30], &targets, &mut state, 8.0, 1.0).unwrap();
        assert_eq!(r.frame, 130);
        assert_eq!(r.probe_offset, 30);
    }

    #[test]
    fn find_snap_threshold_scales_with_zoom() {
        // 2 px/frame -> base_frame_threshold = 8/2 = 4 frames. probe 103 dist 3 <= 4 -> snap;
        // x = frame * pixels_per_frame.
        let targets = vec![SnapTarget {
            frame: 100,
            kind: SnapKind::ClipEdge,
        }];
        let mut state = SnapState::new();
        let r = SnapEngine::find_snap(103, &[0], &targets, &mut state, 8.0, 2.0).unwrap();
        assert_eq!(r.frame, 100);
        assert_eq!(r.x, 200.0);
        // probe 105 dist 5 > 4 -> none.
        let mut s2 = SnapState::new();
        assert!(SnapEngine::find_snap(105, &[0], &targets, &mut s2, 8.0, 2.0).is_none());
    }
}
