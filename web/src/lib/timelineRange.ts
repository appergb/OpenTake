/**
 * Marked in/out timeline range — pure helpers, 1:1 port of upstream
 * `Timeline/TimelineRangeSelection.swift`. The range is a project-frame span
 * `[startFrame, endFrame)`; `startFrame` may exceed `endFrame` while the user is
 * still marking (I set before O, or an inverted drag), so `normalize` swaps them
 * and `isValid` requires a non-empty span. See SPEC §5 / upstream
 * `validSelectedTimelineRange`.
 */

export interface TimelineRange {
  startFrame: number;
  endFrame: number;
}

/** Swap endpoints so `startFrame <= endFrame` (upstream `normalized`). */
export function normalizeRange(range: TimelineRange): TimelineRange {
  return range.startFrame <= range.endFrame
    ? range
    : { startFrame: range.endFrame, endFrame: range.startFrame };
}

/** A range is valid only when, once normalized, it spans at least one frame
 *  (upstream `isValid`: `endFrame > startFrame`). A single-endpoint range
 *  (start == end) is not yet valid. */
export function isValidRange(range: TimelineRange): boolean {
  const n = normalizeRange(range);
  return n.endFrame > n.startFrame;
}

/** The normalized range if valid, else null — the gate every consumer uses
 *  (upstream `validSelectedTimelineRange`). */
export function validRange(range: TimelineRange | null): TimelineRange | null {
  if (!range) return null;
  const n = normalizeRange(range);
  return isValidRange(n) ? n : null;
}

/** Whether `frame` falls in `[startFrame, endFrame)` of the normalized range
 *  (upstream `contains(frame:)`). */
export function rangeContains(range: TimelineRange, frame: number): boolean {
  const n = normalizeRange(range);
  return frame >= n.startFrame && frame < n.endFrame;
}

/** Set the range start at `frame` (clamped `>= 0`), keeping the existing end or
 *  collapsing to a point when there is none (upstream `markTimelineRangeStart`). */
export function withRangeStart(
  existing: TimelineRange | null,
  frame: number,
): TimelineRange {
  const start = Math.max(0, frame);
  return { startFrame: start, endFrame: existing?.endFrame ?? start };
}

/** Set the range end at `frame` (clamped `>= 0`), keeping the existing start or
 *  collapsing to a point when there is none (upstream `markTimelineRangeEnd`). */
export function withRangeEnd(
  existing: TimelineRange | null,
  frame: number,
): TimelineRange {
  const end = Math.max(0, frame);
  return { startFrame: existing?.startFrame ?? end, endFrame: end };
}
