/**
 * Keyboard clip nudge — pure move planner. OpenTake EXTENSION (no upstream
 * grounding): shift selected clips by a whole-frame delta along the timeline,
 * preserving each clip's track. The group floors as one unit so the earliest
 * clip lands at frame 0 and the selection keeps its relative spacing (same
 * `max(delta, -minStart)` rule the drag-move commit uses), rather than clamping
 * each clip independently. Returns the `ClipMove`-shaped list `moveClips` wants;
 * empty when nothing moves (no ids, or the delta floors to zero).
 */

import type { ClipMoveReq, Timeline } from "./types";

interface NudgeClip {
  id: string;
  trackIndex: number;
  startFrame: number;
}

/** Resolve the selected ids to their current {id, trackIndex, startFrame}. */
function selectedClips(timeline: Timeline, ids: Set<string>): NudgeClip[] {
  const out: NudgeClip[] = [];
  for (let ti = 0; ti < timeline.tracks.length; ti++) {
    for (const clip of timeline.tracks[ti].clips) {
      if (ids.has(clip.id)) out.push({ id: clip.id, trackIndex: ti, startFrame: clip.startFrame });
    }
  }
  return out;
}

/**
 * Build the moves to nudge `selectedIds` by `deltaFrames` (may be negative).
 * `selectedIds` should already include linked partners (the caller expands the
 * link group so partners travel together). No-ops to `[]` when the set is empty
 * or the floored delta is zero (e.g. the group is already at frame 0 and delta
 * is negative). Track is preserved (`toTrack === trackIndex`).
 */
export function planNudge(
  timeline: Timeline,
  selectedIds: Set<string>,
  deltaFrames: number,
): ClipMoveReq[] {
  if (selectedIds.size === 0 || deltaFrames === 0) return [];
  const clips = selectedClips(timeline, selectedIds);
  if (clips.length === 0) return [];

  const minStart = Math.min(...clips.map((c) => c.startFrame));
  // Floor the whole group at frame 0 (never push the earliest clip negative).
  const applied = Math.max(deltaFrames, -minStart);
  if (applied === 0) return [];

  return clips.map((c) => ({
    clipId: c.id,
    toTrack: c.trackIndex,
    toFrame: c.startFrame + applied,
  }));
}
