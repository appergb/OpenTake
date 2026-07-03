/**
 * Preview canvas zoom + pan math (SPEC §8, Item 1). Pure functions ported 1:1
 * from upstream's cursor-anchored scroll-to-zoom, so the frame under the cursor
 * stays put while zooming. Kept UI-free (no React/store) so it is unit-tested in
 * isolation; `Preview.tsx` wires the wheel event and canvas transform to it.
 *
 * Upstream refs (palmier-pro):
 *  - PreviewView.swift:14-34 `onCmdScroll` — the anchor math below.
 *  - PreviewView.swift:18 — clamp `min(max(old*factor, 0.1), 8.0)`.
 *  - EditorViewModel.swift:69-72 — `canvasZoom` didSet resets offset when <= 1.0.
 *  - PreviewContainerView.swift:20-49 — stage is `fitSize * zoom`, centered, then
 *    translated by `canvasOffset`.
 */

/** Canvas offset in device px, applied as a post-scale translate on the stage.
 *  Field names mirror upstream `CGSize` (`width`/`height`) so the store DTO and
 *  the TransformOverlay/CropOverlay pointer math read identically. */
export interface CanvasOffset {
  width: number;
  height: number;
}

/** Zoom clamp bounds (PreviewView.swift:18). */
export const CANVAS_ZOOM_MIN = 0.1;
export const CANVAS_ZOOM_MAX = 8.0;

/** Below this the canvas fills at most its fit box; upstream recenters (offset
 *  = zero) at or under it (EditorViewModel.swift:69-72). */
export const CANVAS_ZOOM_FIT = 1.0;

/** Clamp a raw zoom to `[CANVAS_ZOOM_MIN, CANVAS_ZOOM_MAX]`. */
export function clampCanvasZoom(zoom: number): number {
  if (!Number.isFinite(zoom)) return CANVAS_ZOOM_FIT;
  return Math.min(Math.max(zoom, CANVAS_ZOOM_MIN), CANVAS_ZOOM_MAX);
}

/**
 * The offset that keeps `pointTopDown` (a point in the stage's own top-left
 * coordinates, px) stationary while zoom goes `oldZoom -> newZoom`.
 *
 * 1:1 port of PreviewView.swift:21-32. `F` (the fit-canvas size) is recovered as
 * `viewSize / oldZoom`; the shift keeps the on-screen anchor fixed as the
 * centered stage grows/shrinks about its midpoint:
 *   dx = F.w*(new-old)/2 + point.x*(1 - new/old)
 * (dy symmetric). Returns the NEW absolute offset (previous offset + shift).
 */
export function zoomAroundPoint(params: {
  oldZoom: number;
  newZoom: number;
  /** Pointer location in stage-local top-down px (0,0 = stage top-left). */
  pointTopDown: { x: number; y: number };
  /** Current stage size in px (the fit box already multiplied by oldZoom). */
  viewSize: { width: number; height: number };
  /** Offset in effect before this zoom step. */
  offset: CanvasOffset;
}): CanvasOffset {
  const { oldZoom, newZoom, pointTopDown, viewSize, offset } = params;
  if (!(oldZoom > 0) || !(newZoom > 0)) return offset;
  const fitW = viewSize.width / oldZoom;
  const fitH = viewSize.height / oldZoom;
  const dx = (fitW * (newZoom - oldZoom)) / 2 + pointTopDown.x * (1 - newZoom / oldZoom);
  const dy = (fitH * (newZoom - oldZoom)) / 2 + pointTopDown.y * (1 - newZoom / oldZoom);
  return { width: offset.width + dx, height: offset.height + dy };
}

/**
 * Apply one scroll-zoom step. Combines the clamp, the "no-op when the clamp
 * pins zoom" short-circuit (PreviewView.swift:19), and the offset anchor. When
 * the resulting zoom is at or below `CANVAS_ZOOM_FIT`, the offset is reset to
 * zero (EditorViewModel.swift:69-72) rather than anchored — so scrolling back
 * out always re-centers.
 *
 * `deltaZoom` is the raw wheel delta already scaled to a log factor exponent
 * (upstream `factor = exp(deltaY)`, where `deltaY = scrollingDeltaY *
 * sensitivity`). Positive zooms in.
 */
export function applyScrollZoom(params: {
  oldZoom: number;
  deltaZoom: number;
  pointTopDown: { x: number; y: number };
  viewSize: { width: number; height: number };
  offset: CanvasOffset;
}): { zoom: number; offset: CanvasOffset } {
  const { oldZoom, deltaZoom, pointTopDown, viewSize, offset } = params;
  const factor = Math.exp(deltaZoom);
  const newZoom = clampCanvasZoom(oldZoom * factor);
  // Clamp pinned zoom (already at a bound): nothing moves.
  if (Math.abs(newZoom - oldZoom) < 0.0001) {
    return { zoom: oldZoom, offset };
  }
  if (newZoom <= CANVAS_ZOOM_FIT) {
    return { zoom: newZoom, offset: { width: 0, height: 0 } };
  }
  return {
    zoom: newZoom,
    offset: zoomAroundPoint({ oldZoom, newZoom, pointTopDown, viewSize, offset }),
  };
}
