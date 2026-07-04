/**
 * Timeline hit-testing (SPEC §5.8). Maps a document-space point to a clip and a
 * sub-region (left trim handle / right trim handle / body), following the
 * upstream priority (handles before body). Coordinates are document-space
 * (already offset for the header column and scroll by the caller).
 */

import { TRIM, CLIP, FADE } from "../../lib/theme";
import { clipRect } from "../../lib/geometry";
import type { Timeline, Clip } from "../../lib/types";

export type ClipRegion = "trimLeft" | "trimRight" | "body";

export interface ClipHit {
  trackIndex: number;
  clipIndex: number;
  clip: Clip;
  region: ClipRegion;
  /** x within the clip rect. */
  localX: number;
}

export function hitTestClip(
  timeline: Timeline,
  docX: number,
  docY: number,
  pixelsPerFrame: number,
  trackHeights: Record<string, number>,
): ClipHit | null {
  for (let ti = 0; ti < timeline.tracks.length; ti++) {
    const track = timeline.tracks[ti];
    if (track.hidden) continue;
    for (let ci = 0; ci < track.clips.length; ci++) {
      const clip = track.clips[ci];
      const rect = clipRect(timeline, ti, clip, pixelsPerFrame, trackHeights);
      if (
        docX >= rect.x &&
        docX <= rect.x + rect.width &&
        docY >= rect.y &&
        docY <= rect.y + rect.height
      ) {
        const localX = docX - rect.x;
        let region: ClipRegion = "body";
        if (localX <= TRIM.handleWidth) region = "trimLeft";
        else if (localX >= rect.width - TRIM.handleWidth) region = "trimRight";
        return { trackIndex: ti, clipIndex: ci, clip, region, localX };
      }
    }
  }
  return null;
}

/** Expand a clip id set to its full link groups (SPEC §9.1 linked selection). */
export function expandLinkGroup(timeline: Timeline, ids: Set<string>): Set<string> {
  const groups = new Set<string>();
  for (const t of timeline.tracks) {
    for (const c of t.clips) {
      if (ids.has(c.id) && c.linkGroupId) groups.add(c.linkGroupId);
    }
  }
  if (groups.size === 0) return new Set(ids);
  const out = new Set(ids);
  for (const t of timeline.tracks) {
    for (const c of t.clips) {
      if (c.linkGroupId && groups.has(c.linkGroupId)) out.add(c.id);
    }
  }
  return out;
}

/** All clips intersecting a document-space rectangle (marquee). */
export function clipsInRect(
  timeline: Timeline,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  pixelsPerFrame: number,
  trackHeights: Record<string, number>,
): Set<string> {
  const minX = Math.min(x0, x1);
  const maxX = Math.max(x0, x1);
  const minY = Math.min(y0, y1);
  const maxY = Math.max(y0, y1);
  const out = new Set<string>();
  for (let ti = 0; ti < timeline.tracks.length; ti++) {
    const track = timeline.tracks[ti];
    if (track.hidden) continue;
    for (const clip of track.clips) {
      const rect = clipRect(timeline, ti, clip, pixelsPerFrame, trackHeights);
      const intersects =
        rect.x <= maxX &&
        rect.x + rect.width >= minX &&
        rect.y <= maxY &&
        rect.y + rect.height >= minY;
      if (intersects) out.add(clip.id);
    }
  }
  return out;
}

/** Hit radius (px) for a volume-keyframe dot — the dot is drawn at 5px radius,
 *  plus 3px of grab tolerance so a fast click still grabs it. */
const VOLUME_KF_HIT_RADIUS = 8;

/** Result of hitting a draggable volume-keyframe dot. `frame` is clip-relative
 *  (0 = clip start), matching `Keyframe.frame` storage. */
export interface VolumeKfHit {
  clipId: string;
  /** Clip-relative keyframe frame. */
  frame: number;
}

export type FadeEdge = "left" | "right";

export interface FadeKneeHit {
  clipId: string;
  trackIndex: number;
  edge: FadeEdge;
  currentFrames: number;
}

/**
 * Hit-test the draggable volume-keyframe dots drawn by `drawVolumeEnvelope`
 * (SPEC §5.4). Returns the first audio clip's volume kf within the grab radius,
 * or null. The dot position math mirrors `drawVolumeEnvelope` exactly so a
 * visible dot is always grabbable. `docX`/`docY` are document-space (already
 * scroll-adjusted by the caller, same convention as `hitTestClip`).
 */
export function audioVolumeKfHit(
  timeline: Timeline,
  docX: number,
  docY: number,
  pixelsPerFrame: number,
  trackHeights: Record<string, number>,
): VolumeKfHit | null {
  for (let ti = 0; ti < timeline.tracks.length; ti++) {
    const track = timeline.tracks[ti];
    if (track.hidden) continue;
    for (const clip of track.clips) {
      if (clip.mediaType !== "audio") continue;
      const track2 = clip.volumeTrack;
      if (!track2 || track2.keyframes.length === 0) continue;
      const rect = clipRect(timeline, ti, clip, pixelsPerFrame, trackHeights);
      if (clip.durationFrames <= 0) continue;
      const ppf = (rect.width - 2 * TRIM.handleWidth) / clip.durationFrames;
      if (ppf <= 0) continue;
      const baseX = rect.x + TRIM.handleWidth;
      const bodyTop = rect.y + CLIP.labelBarHeight;
      const bodyH = rect.height - CLIP.labelBarHeight;
      for (const kf of track2.keyframes) {
        if (kf.frame < 0 || kf.frame > clip.durationFrames) continue;
        const kx = baseX + kf.frame * ppf;
        const c = Math.max(0, Math.min(1, kf.value));
        const ky = bodyTop + bodyH * (1 - c);
        const dx = docX - kx;
        const dy = docY - ky;
        if (dx * dx + dy * dy <= VOLUME_KF_HIT_RADIUS * VOLUME_KF_HIT_RADIUS) {
          return { clipId: clip.id, frame: kf.frame };
        }
      }
    }
  }
  return null;
}

function fadeKneePoint(
  clip: Clip,
  rect: { x: number; y: number; width: number; height: number },
  edge: FadeEdge,
) {
  const ppf = clip.durationFrames > 0 ? rect.width / clip.durationFrames : 0;
  const offset =
    edge === "left"
      ? Math.min(clip.fadeInFrames, clip.durationFrames)
      : Math.max(0, clip.durationFrames - clip.fadeOutFrames);
  const actualX = rect.x + offset * ppf;
  const x =
    edge === "left"
      ? Math.max(rect.x + FADE.edgeInset, actualX)
      : Math.min(rect.x + rect.width - FADE.edgeInset, actualX);
  const y = rect.y + CLIP.labelBarHeight + FADE.kneeTopInset;
  return { x, y };
}

/**
 * Hit-test the draggable opacity fade knees drawn by `drawFades`. The math mirrors
 * upstream `TimelineGeometry.fadeKneeRect`: a 14px hit square centered on the
 * fixed fade lane near the clip-body top. Audio fades are intentionally excluded
 * until their handles are drawn locally; this avoids invisible hit zones over the
 * audio volume rubber band.
 */
export function fadeKneeHit(
  timeline: Timeline,
  docX: number,
  docY: number,
  pixelsPerFrame: number,
  trackHeights: Record<string, number>,
): FadeKneeHit | null {
  const half = FADE.hitSize / 2;
  for (let ti = 0; ti < timeline.tracks.length; ti++) {
    const track = timeline.tracks[ti];
    if (track.hidden) continue;
    for (const clip of track.clips) {
      if (clip.mediaType === "audio" || clip.durationFrames <= 0) continue;
      const rect = clipRect(timeline, ti, clip, pixelsPerFrame, trackHeights);
      if (docX < rect.x || docX > rect.x + rect.width || docY < rect.y || docY > rect.y + rect.height) {
        continue;
      }
      for (const edge of ["left", "right"] as const) {
        const p = fadeKneePoint(clip, rect, edge);
        if (docX >= p.x - half && docX <= p.x + half && docY >= p.y - half && docY <= p.y + half) {
          return {
            clipId: clip.id,
            trackIndex: ti,
            edge,
            currentFrames: edge === "left" ? clip.fadeInFrames : clip.fadeOutFrames,
          };
        }
      }
    }
  }
  return null;
}

/** Port of upstream `applyFadeKneeDrag`: cursor movement changes the grabbed fade
 * length and clamps against the opposite fade so the two ramps never overlap. */
export function fadeFramesForDrag(
  clip: Pick<Clip, "durationFrames" | "fadeInFrames" | "fadeOutFrames">,
  edge: FadeEdge,
  originalFrames: number,
  grabFrame: number,
  cursorFrame: number,
): number {
  const delta = cursorFrame - grabFrame;
  const proposed = edge === "left" ? originalFrames + delta : originalFrames - delta;
  const counterFade = edge === "left" ? clip.fadeOutFrames : clip.fadeInFrames;
  const cap = Math.max(0, clip.durationFrames - counterFade);
  return Math.max(0, Math.min(cap, proposed));
}
