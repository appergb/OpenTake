/**
 * Timeline content painter (SPEC §5.9). Draws track backgrounds, the video/audio
 * region divider, range fills, and all clips (with move/trim ghosts) into the
 * scrolling document canvas. The ruler and playhead are separate sticky overlays
 * (SPEC §5.11), painted by the container.
 */

import { BG, BORDER, TEXT } from "../../lib/theme";
import { clipRect, trackDisplayHeight, trackY } from "../../lib/geometry";
import { linkOffsetForClip } from "../../lib/clip";
import { drawClip } from "./clipRenderer";
import type { Timeline } from "../../lib/types";

export interface PaintState {
  timeline: Timeline;
  pixelsPerFrame: number;
  trackHeights: Record<string, number>;
  selectedClipIds: Set<string>;
  /** Device pixel ratio for crisp lines. */
  dpr: number;
  /** Document content size (CSS px). */
  width: number;
  height: number;
  /** Index of the first audio track, or -1, for the region divider. */
  firstAudioIndex: number;
  /** Scroll offset into the document (CSS px). */
  scrollLeft: number;
  scrollTop: number;
  /** Visible viewport size (CSS px). */
  viewWidth: number;
  viewHeight: number;
  /** Normalized waveform buckets per media asset id (`0 = loud, 1 = silence`),
   *  loaded on demand from the Rust media cache. Absent until resolved. */
  waveforms: Map<string, number[]>;
  /** Media asset ids whose source file is offline (moved/deleted). Clips that
   *  reference one render with the error wash. */
  missingMediaRefs: Set<string>;
  /** Localized "drop media here" hint shown when the timeline has no tracks. */
  emptyLabel: string;
  /** Active drag, so clips follow the cursor (ghost). Absent when not dragging. */
  drag?: DragPaint;
}

/** A live move/trim, projected for ghost rendering. */
export type DragPaint =
  | { kind: "move"; ids: Set<string>; deltaFrames: number; trackDelta: number }
  | { kind: "trim"; clipId: string; edge: "left" | "right"; deltaFrames: number }
  | { kind: "volumeKf"; clipId: string; fromFrame: number; ghostFrame: number };

export function paintTimeline(ctx: CanvasRenderingContext2D, s: PaintState) {
  const { timeline, pixelsPerFrame, trackHeights, width, dpr, scrollLeft, scrollTop } = s;

  // Document-space drawing: translate by -scroll so the visible window paints
  // into the canvas (SPEC §5.1 — content scrolls under a fixed viewport).
  ctx.setTransform(dpr, 0, 0, dpr, -scrollLeft * dpr, -scrollTop * dpr);
  ctx.clearRect(scrollLeft, scrollTop, s.viewWidth, s.viewHeight);

  const visRight = scrollLeft + s.viewWidth;

  // 1. Track backgrounds (drawTrackBackgrounds: surface + 1px borders). Fill the
  // visible window width so the surface reaches the right edge.
  for (let i = 0; i < timeline.tracks.length; i++) {
    const ty = trackY(timeline, i, trackHeights);
    const th = trackDisplayHeight(timeline.tracks[i], trackHeights);
    ctx.fillStyle = BG.surface;
    ctx.fillRect(scrollLeft, ty, s.viewWidth, th);
    ctx.fillStyle = BORDER.primary;
    ctx.fillRect(scrollLeft, ty, s.viewWidth, 1);
    ctx.fillRect(scrollLeft, ty + th - 1, s.viewWidth, 1);
  }

  // Video/audio region divider: 2px divider at first audio track top.
  if (s.firstAudioIndex > 0) {
    const dy = trackY(timeline, s.firstAudioIndex, trackHeights);
    ctx.fillStyle = BORDER.divider;
    ctx.fillRect(scrollLeft, dy, s.viewWidth, 2);
  }

  // 3. Clips (skip those fully outside the visible window). A clip being dragged
  // is drawn at its live (offset) position as a ghost so it follows the cursor.
  const drag = s.drag;
  for (let ti = 0; ti < timeline.tracks.length; ti++) {
    const track = timeline.tracks[ti];
    for (const clip of track.clips) {
      let rect = clipRect(timeline, ti, clip, pixelsPerFrame, trackHeights);
      let ghost = false;
      if (drag?.kind === "move" && drag.ids.has(clip.id)) {
        const nti = Math.max(0, Math.min(timeline.tracks.length - 1, ti + drag.trackDelta));
        rect = clipRect(
          timeline,
          nti,
          { ...clip, startFrame: clip.startFrame + drag.deltaFrames },
          pixelsPerFrame,
          trackHeights,
        );
        ghost = true;
      } else if (drag?.kind === "trim" && drag.clipId === clip.id) {
        const dx = drag.deltaFrames * pixelsPerFrame;
        rect =
          drag.edge === "left"
            ? { ...rect, x: rect.x + dx, width: rect.width - dx }
            : { ...rect, width: rect.width + dx };
        ghost = true;
      }
      if (rect.x + rect.width < scrollLeft || rect.x > visRight) continue;
      // Volume-kf drag ghost: when this clip is the one being dragged, tell the
      // renderer to draw the grabbed dot at its ghost frame instead of the
      // original, so the dot follows the cursor (SPEC §5.4).
      const volumeKfGhost =
        drag?.kind === "volumeKf" && drag.clipId === clip.id
          ? { fromFrame: drag.fromFrame, ghostFrame: drag.ghostFrame }
          : undefined;
      drawClip(ctx, clip, rect, {
        isSelected: s.selectedClipIds.has(clip.id),
        fps: timeline.fps,
        waveform: clip.mediaType === "audio" ? s.waveforms.get(clip.mediaRef) : undefined,
        // Text clips have no source file; everything else is "missing" when its
        // asset's file is offline.
        missing: clip.mediaType !== "text" && s.missingMediaRefs.has(clip.mediaRef),
        ghost,
        linkOffset: linkOffsetForClip(timeline, clip.id),
        volumeKfGhost,
      });
    }
  }

  // Empty-state hint when no tracks (centered in the visible window).
  if (timeline.tracks.length === 0) {
    ctx.fillStyle = TEXT.muted;
    ctx.font = '13px -apple-system, system-ui, sans-serif';
    ctx.textAlign = "center";
    ctx.fillText(s.emptyLabel, scrollLeft + s.viewWidth / 2, scrollTop + s.viewHeight / 2);
    ctx.textAlign = "left";
  }
  void width;
}
