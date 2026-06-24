/**
 * Snap engine — 1:1 port of upstream `Timeline/SnapEngine.swift` (SPEC §5.7).
 * Collects clip-edge + optional playhead targets and finds the nearest snap in
 * frame space, honoring the sticky multiplier and playhead priority multiplier.
 */

import { SNAP } from "./theme";
import { endFrame } from "./geometry";
import type { Timeline } from "./types";

export type SnapKind = "clipEdge" | "playhead";

export interface SnapTarget {
  frame: number;
  kind: SnapKind;
}

export interface SnapResult {
  /** Snapped frame. */
  frame: number;
  kind: SnapKind;
}

/** Collect snap targets: every clip start/end (excluding dragged clips) plus an
 *  optional playhead (SnapEngine.swift:31-48). `includePlayhead` defaults false
 *  to match upstream: only callers that explicitly opt in get playhead snap. */
export function collectTargets(
  timeline: Timeline,
  excludeClipIds: Set<string>,
  playheadFrame: number | null,
  includePlayhead = false,
): SnapTarget[] {
  const targets: SnapTarget[] = [];
  for (const track of timeline.tracks) {
    for (const clip of track.clips) {
      if (excludeClipIds.has(clip.id)) continue;
      targets.push({ frame: clip.startFrame, kind: "clipEdge" });
      targets.push({ frame: endFrame(clip), kind: "clipEdge" });
    }
  }
  if (playheadFrame !== null && includePlayhead) {
    targets.push({ frame: playheadFrame, kind: "playhead" });
  }
  return targets;
}

/**
 * Find the nearest snap for a probe frame. `currentlySnapped` carries the
 * previously snapped frame so the sticky band (1.5x) keeps it engaged until the
 * probe moves out (SnapEngine.swift:64-93). Returns null when nothing snaps.
 */
export function findSnap(
  probeFrame: number,
  targets: SnapTarget[],
  pixelsPerFrame: number,
  currentlySnapped: number | null,
): SnapResult | null {
  const baseThresholdFrames = SNAP.thresholdPixels / pixelsPerFrame;

  // Sticky: stay snapped until we exceed the sticky band of the held target.
  if (currentlySnapped !== null) {
    const stickyBand = baseThresholdFrames * SNAP.stickyMultiplier;
    if (Math.abs(probeFrame - currentlySnapped) <= stickyBand) {
      return { frame: currentlySnapped, kind: "clipEdge" };
    }
  }

  let best: SnapResult | null = null;
  let bestDist = Number.POSITIVE_INFINITY;
  for (const t of targets) {
    const threshold =
      t.kind === "playhead"
        ? baseThresholdFrames * SNAP.playheadMultiplier
        : baseThresholdFrames;
    const dist = Math.abs(probeFrame - t.frame);
    if (dist > threshold) continue;
    // Playhead wins ties via its larger threshold; otherwise nearest wins.
    if (dist < bestDist) {
      bestDist = dist;
      best = { frame: t.frame, kind: t.kind };
    }
  }
  return best;
}

/**
 * Multi-probe snap (SPEC §5.8 `findSnap probeOffsets`): for a set of probe
 * offsets (e.g. start + end edges of all selected clips), find the snap that
 * yields the smallest correction, returning the delta to apply.
 */
export function findSnapDelta(
  probeFrames: number[],
  targets: SnapTarget[],
  pixelsPerFrame: number,
): { delta: number; snappedFrame: number } | null {
  let best: { delta: number; snappedFrame: number } | null = null;
  let bestDist = Number.POSITIVE_INFINITY;
  for (const probe of probeFrames) {
    const res = findSnap(probe, targets, pixelsPerFrame, null);
    if (!res) continue;
    const dist = Math.abs(res.frame - probe);
    if (dist < bestDist) {
      bestDist = dist;
      best = { delta: res.frame - probe, snappedFrame: res.frame };
    }
  }
  return best;
}
