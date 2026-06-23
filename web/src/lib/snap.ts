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
 *  optional playhead (SnapEngine.swift:31-48). */
export function collectTargets(
  timeline: Timeline,
  excludeClipIds: Set<string>,
  playheadFrame: number | null,
): SnapTarget[] {
  const targets: SnapTarget[] = [];
  for (const track of timeline.tracks) {
    for (const clip of track.clips) {
      if (excludeClipIds.has(clip.id)) continue;
      targets.push({ frame: clip.startFrame, kind: "clipEdge" });
      targets.push({ frame: endFrame(clip), kind: "clipEdge" });
    }
  }
  if (playheadFrame !== null) {
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
 *
 * `currentlySnapped` carries the previously snapped `{frame, probeOffset}` so
 * the sticky band (1.5x) keeps the same probe engaged across pointer events
 * (SnapEngine.swift:64-93) — without it, the snap would toggle off/on near the
 * threshold edge and the clip would jitter. `probeOffsets` is a parallel array
 * of stable per-probe identifiers (e.g. the frame offset from the lead clip's
 * start); when omitted the probe index is used. The snapped `probeOffset` is
 * returned so the caller can feed it back in on the next move.
 */
export function findSnapDelta(
  probeFrames: number[],
  targets: SnapTarget[],
  pixelsPerFrame: number,
  currentlySnapped: { frame: number; probeOffset: number } | null = null,
  probeOffsets?: number[],
): { delta: number; snappedFrame: number; probeOffset: number } | null {
  if (probeFrames.length === 0) return null;
  const offsets = probeOffsets ?? probeFrames.map((_, i) => i);
  const baseThresholdFrames = SNAP.thresholdPixels / pixelsPerFrame;
  const stickyBand = baseThresholdFrames * SNAP.stickyMultiplier;

  // Sticky: keep the held target engaged while its owning probe stays within
  // the sticky band (1.5x). This mirrors findSnap's sticky branch but tracks
  // WHICH probe was snapped via probeOffset.
  if (currentlySnapped !== null) {
    const idx = offsets.indexOf(currentlySnapped.probeOffset);
    if (idx >= 0) {
      const probe = probeFrames[idx];
      if (Math.abs(probe - currentlySnapped.frame) <= stickyBand) {
        return {
          delta: currentlySnapped.frame - probe,
          snappedFrame: currentlySnapped.frame,
          probeOffset: currentlySnapped.probeOffset,
        };
      }
    }
  }

  let best: { delta: number; snappedFrame: number; probeOffset: number } | null = null;
  let bestDist = Number.POSITIVE_INFINITY;
  for (let i = 0; i < probeFrames.length; i++) {
    const probe = probeFrames[i];
    const res = findSnap(probe, targets, pixelsPerFrame, null);
    if (!res) continue;
    const dist = Math.abs(res.frame - probe);
    if (dist < bestDist) {
      bestDist = dist;
      best = { delta: res.frame - probe, snappedFrame: res.frame, probeOffset: offsets[i] };
    }
  }
  return best;
}
