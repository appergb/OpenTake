/**
 * Timeline content painter (SPEC §5.9). Draws track backgrounds, the video/audio
 * region divider, range fills, and all clips (with move/trim ghosts) into the
 * scrolling document canvas. The ruler and playhead are separate sticky overlays
 * (SPEC §5.11), painted by the container.
 */

import { BG, BORDER, TEXT, LAYOUT, TRACK_SIZE, TRIM, GHOST } from "../../lib/theme";
import { clipRect, trackDisplayHeight, trackY } from "../../lib/geometry";
import { linkOffsetForClip } from "../../lib/clip";
import { drawClip, roundRectPath, type ClipThumbnailStrip } from "./clipRenderer";
import type { Timeline, ClipType } from "../../lib/types";

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
  /** Loaded visual thumbnail sprites/single images per media asset id. */
  thumbnails: Map<string, ClipThumbnailStrip>;
  /** Media asset ids whose source file is offline (moved/deleted). Clips that
   *  reference one render with the error wash. */
  missingMediaRefs: Set<string>;
  /** Localized "drop media here" hint shown when the timeline has no tracks. */
  emptyLabel: string;
  /** Active drag, so clips follow the cursor (ghost). Absent when not dragging. */
  drag?: DragPaint;
  /** Drag from the media panel hovering the timeline: a gray ghost at the
   *  resolved track + frame span (and a "new track" lane when the drop creates
   *  one). Absent when no media drag is over the timeline. */
  mediaGhost?: MediaGhostPaint;
}

/** A media-panel drag projected over the timeline, for the drop-ghost preview. */
export interface MediaGhostPaint {
  /** Snapped start frame the clip would occupy. */
  startFrame: number;
  /** Clip length in frames (== the clip that will land). */
  durationFrames: number;
  /** Existing track it will land on, or null when a new track is created. */
  trackIndex: number | null;
  /** Insert index of the new track to create, or null for an existing track. */
  newTrackIndex: number | null;
}

/** A live move/trim, projected for ghost rendering. */
export type DragPaint =
  | {
      kind: "move";
      ids: Set<string>;
      deltaFrames: number;
      trackDelta: number;
      pinnedIds?: Set<string>;
      leadTrackIndex: number;
      /** Option/Alt-drag duplicate: ghost renders with a "+" badge. */
      isDuplicate?: boolean;
      /** Dropping on an insert zone creates a new track of this type. */
      newTrackType?: ClipType;
      /** Upstream `newTrackAt(index)` insertion index for the new-track drop. */
      newTrackIndex?: number;
      /** Cross-track swap preview: the clip being displaced ghosts at the slot
       *  the lead clip is vacating, so the two visibly trade places before the
       *  drop. Absent unless the drop would be a single-clip swap. */
      swap?: { clipId: string; toTrackIndex: number; toFrame: number };
    }
  | { kind: "trim"; clipId: string; edge: "left" | "right"; deltaFrames: number }
  | { kind: "volumeKf"; clipId: string; fromFrame: number; ghostFrame: number }
  | { kind: "fadeKnee"; clipId: string; edge: "left" | "right"; currentFrames: number };

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
  const insertionLineY = (index: number): number => {
    if (timeline.tracks.length === 0) return LAYOUT.rulerHeight + LAYOUT.dropZoneHeight;
    if (index <= 0) return trackY(timeline, 0, trackHeights);
    if (index >= timeline.tracks.length) return trackY(timeline, timeline.tracks.length, trackHeights);
    return trackY(timeline, index, trackHeights);
  };
  // Hint that a new track will be created at `laneY`: a solid YELLOW insertion
  // line across the lane's top edge (1:1 with upstream's `NSColor.systemYellow`
  // line) — NOT a full-width fill, which reads as "the whole row lit up". The
  // clip-sized ghost drawn at this lane is the "it lands here" indicator.
  const drawNewTrackHint = (laneY: number, laneH: number): void => {
    if (laneY + laneH <= scrollTop || laneY >= scrollTop + s.viewHeight) return;
    ctx.strokeStyle = GHOST.insertLine;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(scrollLeft, laneY + 1);
    ctx.lineTo(scrollLeft + s.viewWidth, laneY + 1);
    ctx.stroke();
  };
  // New-track drop indicator: insertion line at the upstream insertion index.
  if (drag?.kind === "move" && drag.newTrackType && timeline.tracks.length > 0) {
    const newTrackY = insertionLineY(drag.newTrackIndex ?? timeline.tracks.length);
    const newTrackH = trackDisplayHeight(timeline.tracks[0], trackHeights) || TRACK_SIZE.defaultHeight;
    drawNewTrackHint(newTrackY, newTrackH);
  }
  for (let ti = 0; ti < timeline.tracks.length; ti++) {
    const track = timeline.tracks[ti];
    for (const clip of track.clips) {
      let rect = clipRect(timeline, ti, clip, pixelsPerFrame, trackHeights);
      let ghost = false;
      let isDuplicate = false;
      if (drag?.kind === "move" && drag.ids.has(clip.id)) {
        const isPinned = drag.pinnedIds?.has(clip.id) === true;
        const onLeadRow = ti === drag.leadTrackIndex;
        if (drag.newTrackType && !isPinned && onLeadRow) {
          const newTrackIndex = drag.newTrackIndex ?? timeline.tracks.length;
          const newTrackY = insertionLineY(newTrackIndex);
          const ghostH = (trackDisplayHeight(timeline.tracks[0], trackHeights) || TRACK_SIZE.defaultHeight) - 4;
          // Upstream `TimelineGeometry.ghostY`: the new-track ghost sits ABOVE
          // the insertion line (lineY - height) for every insert except the very
          // bottom (index >= trackCount), where it sits at the line.
          const ghostTop = newTrackIndex < timeline.tracks.length ? newTrackY - ghostH - 2 : newTrackY + 2;
          rect = {
            x: (clip.startFrame + drag.deltaFrames) * pixelsPerFrame,
            y: ghostTop,
            width: clip.durationFrames * pixelsPerFrame,
            height: ghostH,
          };
        } else {
          const nti = isPinned
            ? ti
            : Math.max(0, Math.min(timeline.tracks.length - 1, ti + drag.trackDelta));
          rect = clipRect(
            timeline,
            nti,
            { ...clip, startFrame: clip.startFrame + drag.deltaFrames },
            pixelsPerFrame,
            trackHeights,
          );
        }
        ghost = true;
        isDuplicate = drag.isDuplicate === true;
      } else if (drag?.kind === "move" && drag.swap?.clipId === clip.id) {
        // The clip being displaced in a cross-track swap: ghost it at the slot
        // the lead clip is vacating, so the two visibly trade places.
        rect = clipRect(
          timeline,
          drag.swap.toTrackIndex,
          { ...clip, startFrame: drag.swap.toFrame },
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
      const paintClip =
        drag?.kind === "fadeKnee" && drag.clipId === clip.id
          ? {
              ...clip,
              fadeInFrames: drag.edge === "left" ? drag.currentFrames : clip.fadeInFrames,
              fadeOutFrames: drag.edge === "right" ? drag.currentFrames : clip.fadeOutFrames,
            }
          : clip;
      drawClip(ctx, paintClip, rect, {
        isSelected: s.selectedClipIds.has(clip.id),
        fps: timeline.fps,
        waveform: clip.mediaType === "audio" ? s.waveforms.get(clip.mediaRef) : undefined,
        thumbnailStrip:
          clip.mediaType !== "audio" && clip.mediaType !== "text"
            ? s.thumbnails.get(clip.mediaRef)
            : undefined,
        // Text clips have no source file; everything else is "missing" when its
        // asset's file is offline.
        missing: clip.mediaType !== "text" && s.missingMediaRefs.has(clip.mediaRef),
        ghost,
        linkOffset: linkOffsetForClip(timeline, clip.id),
        volumeKfGhost,
        isDuplicate,
      });
    }
  }

  // Media-panel drop ghost: a gray translucent rect at the resolved track +
  // frame span so the user sees exactly where the clip will land (like other
  // NLEs), plus a dashed "new track" lane when the drop will create one.
  const mg = s.mediaGhost;
  if (mg) {
    const ghostX = mg.startFrame * pixelsPerFrame;
    const ghostW = Math.max(1, mg.durationFrames * pixelsPerFrame);
    let ghostY: number | null = null;
    let ghostH = 0;
    if (mg.newTrackIndex !== null) {
      const laneY = insertionLineY(mg.newTrackIndex);
      const laneH =
        timeline.tracks.length > 0
          ? trackDisplayHeight(timeline.tracks[0], trackHeights)
          : TRACK_SIZE.defaultHeight;
      drawNewTrackHint(laneY, laneH);
      ghostH = laneH - 4;
      // Upstream `ghostY`: above the insertion line for every insert except the
      // very bottom (index >= trackCount), so a clip dropped to a new track
      // previews in the lane that opens ABOVE the line.
      ghostY = mg.newTrackIndex < timeline.tracks.length ? laneY - ghostH - 2 : laneY + 2;
    } else if (mg.trackIndex !== null && mg.trackIndex < timeline.tracks.length) {
      ghostY = trackY(timeline, mg.trackIndex, trackHeights) + 2;
      ghostH = trackDisplayHeight(timeline.tracks[mg.trackIndex], trackHeights) - 4;
    }
    if (ghostY !== null && ghostH > 0 && ghostX + ghostW >= scrollLeft && ghostX <= visRight) {
      roundRectPath(ctx, ghostX, ghostY, ghostW, ghostH, TRIM.clipCornerRadius);
      ctx.fillStyle = GHOST.fill;
      ctx.fill();
      ctx.strokeStyle = GHOST.border;
      ctx.lineWidth = 1;
      ctx.stroke();
    }
  }

  // Empty-state hint when no tracks (centered in the visible window). Hidden
  // while a media ghost is shown so the two don't overlap on an empty timeline.
  if (timeline.tracks.length === 0 && !mg) {
    ctx.fillStyle = TEXT.muted;
    ctx.font = '13px -apple-system, system-ui, sans-serif';
    ctx.textAlign = "center";
    ctx.fillText(s.emptyLabel, scrollLeft + s.viewWidth / 2, scrollTop + s.viewHeight / 2);
    ctx.textAlign = "left";
  }
  void width;
}
