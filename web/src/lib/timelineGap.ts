/**
 * Empty-region (gap) selection between clips on one track — pure helpers, 1:1
 * port of upstream `TimelineInputController.hitTestGap`. A gap is the empty span
 * `[previousClipEnd, nextClipStart)` on a track, bounded on the RIGHT by a clip
 * (an open-ended tail past the last clip is not a selectable gap upstream). See
 * SPEC §5 / upstream `GapSelection`.
 */

import type { Timeline } from "./types";

export interface GapSelection {
  trackIndex: number;
  startFrame: number;
  endFrame: number;
}

interface GapClip {
  startFrame: number;
  durationFrames: number;
}

function endOf(clip: GapClip): number {
  return clip.startFrame + clip.durationFrames;
}

/**
 * The gap on `trackIndex` containing project `frame`, or null. Returns null when
 * `frame` lands inside any clip, or when there is no clip to the right (upstream
 * requires a `nextStart`, so the open tail past the last clip is not a gap). The
 * gap's left edge is the max end of clips ending at/before `frame` (0 if none).
 *
 * Pure mirror of upstream `hitTestGap` MINUS the y-band check (the caller has
 * already resolved the track from the pointer's y, exactly as upstream does).
 */
export function gapAtFrame(
  timeline: Timeline,
  trackIndex: number,
  frame: number,
): GapSelection | null {
  const track = timeline.tracks[trackIndex];
  if (!track) return null;
  const clips = track.clips as GapClip[];

  // Inside a clip → not a gap.
  if (clips.some((c) => frame >= c.startFrame && frame < endOf(c))) return null;

  // Must be bounded on the right by a clip.
  let nextStart = Number.POSITIVE_INFINITY;
  for (const c of clips) {
    if (c.startFrame > frame && c.startFrame < nextStart) nextStart = c.startFrame;
  }
  if (!Number.isFinite(nextStart)) return null;

  // Left edge: latest end at/before `frame`, else 0.
  let prevEnd = 0;
  for (const c of clips) {
    const e = endOf(c);
    if (e <= frame && e > prevEnd) prevEnd = e;
  }

  return { trackIndex, startFrame: prevEnd, endFrame: nextStart };
}
