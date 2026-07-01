/**
 * Pure keyframe snap resolver for the Inspector KeyframesPanel (SPEC §6.4).
 * Mirrors upstream `KeyframesLaneRow.applySnap` (Inspector/Keyframes/KeyframesLane.swift)
 * at the "which target wins" level: nearest target within threshold wins, no
 * per-kind threshold weighting (unlike the main-timeline `lib/snap.ts` port,
 * which gives the playhead a larger threshold multiplier — upstream's keyframe
 * lane treats playhead/clip-edge/cross-property targets uniformly, see
 * KeyframesLane.swift:177-216 where all `SnapTarget`s share one
 * `snapThresholdPixels` call).
 */

export interface KeyframeSnapResult {
  /** The candidate frame, replaced by the nearest in-threshold target if any. */
  frame: number;
  /** The target frame snapped to, or null if the candidate was left unchanged. */
  snappedTo: number | null;
}

/**
 * Snap `candidate` to the nearest value in `targets` that is within
 * `threshold` frames. Ties resolve to whichever target is encountered first
 * in `targets` (deterministic, order-dependent — callers control tie-break
 * priority via target ordering). Returns the unchanged candidate with
 * `snappedTo: null` when no target is within threshold (including an empty
 * target list or a non-positive threshold).
 */
export function snapFrame(
  candidate: number,
  targets: number[],
  threshold: number,
): KeyframeSnapResult {
  let best: number | null = null;
  let bestDist = Number.POSITIVE_INFINITY;
  for (const target of targets) {
    const dist = Math.abs(candidate - target);
    if (dist <= threshold && dist < bestDist) {
      bestDist = dist;
      best = target;
    }
  }
  return best === null ? { frame: candidate, snappedTo: null } : { frame: best, snappedTo: best };
}
