/**
 * Timeline geometry — pure functions, 1:1 port of upstream
 * `Timeline/TimelineGeometry.swift` (see SPEC §5.2). All pixel<->frame
 * conversion lives in the front end (AGENTS.md). `headerWidth` is 0 inside the
 * canvas because the track-header column is a separate fixed column.
 */

import { LAYOUT, TRACK_SIZE } from "./theme";
import type { Timeline, Track, Clip } from "./types";

export interface ClipRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** displayHeight resolver: UI-only per-track height (default 50, clamped). */
export function trackDisplayHeight(
  track: Track,
  heights: Record<string, number>,
): number {
  const h = heights[track.id] ?? TRACK_SIZE.defaultHeight;
  return Math.max(TRACK_SIZE.minHeight, Math.min(TRACK_SIZE.maxHeight, h));
}

/** Top Y of track i. First track top = rulerHeight + dropZoneHeight, then
 *  cumulative displayHeight (TimelineGeometry.swift:39-55). */
export function trackY(
  timeline: Timeline,
  i: number,
  heights: Record<string, number>,
): number {
  let y = LAYOUT.rulerHeight + LAYOUT.dropZoneHeight;
  for (let k = 0; k < i && k < timeline.tracks.length; k++) {
    y += trackDisplayHeight(timeline.tracks[k], heights);
  }
  return y;
}

/** Total content height: max(visible, last track bottom + dropZone). */
export function contentHeight(
  timeline: Timeline,
  visibleHeight: number,
  heights: Record<string, number>,
): number {
  const n = timeline.tracks.length;
  if (n === 0) return Math.max(visibleHeight, LAYOUT.rulerHeight + LAYOUT.dropZoneHeight);
  const lastBottom =
    trackY(timeline, n - 1, heights) +
    trackDisplayHeight(timeline.tracks[n - 1], heights);
  return Math.max(visibleHeight, lastBottom + LAYOUT.dropZoneHeight);
}

/** Content width = zoom * totalFrames + visibleWidth*0.5 (TimelineView:116-129). */
export function contentWidth(
  totalFrames: number,
  pixelsPerFrame: number,
  visibleWidth: number,
): number {
  return pixelsPerFrame * totalFrames + visibleWidth * 0.5;
}

/** Largest endFrame across tracks (Timeline.total_frames). */
export function totalFrames(timeline: Timeline): number {
  let max = 0;
  for (const t of timeline.tracks) {
    for (const c of t.clips) {
      const end = c.startFrame + c.durationFrames;
      if (end > max) max = end;
    }
  }
  return max;
}

/** Clip rect (TimelineGeometry.swift:62-69): 2px inset top/bottom. */
export function clipRect(
  timeline: Timeline,
  trackIndex: number,
  clip: Clip,
  pixelsPerFrame: number,
  heights: Record<string, number>,
): ClipRect {
  const y = trackY(timeline, trackIndex, heights) + 2;
  return {
    x: clip.startFrame * pixelsPerFrame,
    y,
    width: clip.durationFrames * pixelsPerFrame,
    height: trackDisplayHeight(timeline.tracks[trackIndex], heights) - 4,
  };
}

/** frameAt(x): truncating, clamped at 0 (TimelineGeometry.swift:71-73). */
export function frameAt(x: number, pixelsPerFrame: number): number {
  return Math.max(0, Math.trunc(x / pixelsPerFrame));
}

/** xForFrame: headerWidth(0) + frame*ppf (TimelineGeometry.swift:138-140). */
export function xForFrame(frame: number, pixelsPerFrame: number): number {
  return frame * pixelsPerFrame;
}

/** trackAt(y): first track whose cumulative bottom exceeds y, else null
 *  (TimelineGeometry.swift:75-80). */
export function trackAt(
  timeline: Timeline,
  y: number,
  heights: Record<string, number>,
): number | null {
  let acc = LAYOUT.rulerHeight + LAYOUT.dropZoneHeight;
  for (let i = 0; i < timeline.tracks.length; i++) {
    acc += trackDisplayHeight(timeline.tracks[i], heights);
    if (y < acc) return i;
  }
  return null;
}

/** end frame (exclusive). */
export function endFrame(clip: Clip): number {
  return clip.startFrame + clip.durationFrames;
}

/**
 * Timecode formatter for the ruler / clip labels. Mirrors upstream
 * `formatTimecode`: HH:MM:SS:FF when >= 1h else MM:SS:FF.
 */
export function formatTimecode(frame: number, fps: number): string {
  const f = Math.max(0, Math.trunc(frame));
  const safeFps = fps > 0 ? fps : 30;
  const totalSeconds = Math.trunc(f / safeFps);
  const frames = f % safeFps;
  const seconds = totalSeconds % 60;
  const minutes = Math.trunc(totalSeconds / 60) % 60;
  const hours = Math.trunc(totalSeconds / 3600);
  const p = (n: number) => String(n).padStart(2, "0");
  if (hours > 0) return `${p(hours)}:${p(minutes)}:${p(seconds)}:${p(frames)}`;
  return `${p(minutes)}:${p(seconds)}:${p(frames)}`;
}

/** Compact duration label for clip label bars: same as timecode but always
 *  MM:SS:FF (used in `name  timecode`). */
export function formatClipDuration(durationFrames: number, fps: number): string {
  return formatTimecode(durationFrames, fps);
}
