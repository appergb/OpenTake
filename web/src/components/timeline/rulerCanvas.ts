/**
 * Ruler painter (SPEC §5.3). Sticky to the viewport top; ticks scroll with the
 * content via `scrollLeft`. Major ticks target ~80px with timecode labels;
 * minor ticks subdivide while each cell stays >= 12px.
 */

import { ACCENT, BG, BORDER, LAYOUT, RANGE, TEXT } from "../../lib/theme";
import { chooseTicks } from "../../lib/ruler";
import { formatTimecode, xForFrame } from "../../lib/geometry";
import { validRange, type TimelineRange } from "../../lib/timelineRange";

export interface RulerState {
  fps: number;
  pixelsPerFrame: number;
  scrollLeft: number;
  width: number; // visible width (CSS px)
  dpr: number;
  /** Marked in/out range, painted as a translucent band + edge ticks on the
   *  ruler (upstream `drawTimelineRangeSelectionRulerFill` / `…Edges`). */
  selectedRange?: TimelineRange | null;
}

export function paintRuler(ctx: CanvasRenderingContext2D, s: RulerState) {
  const { fps, pixelsPerFrame, scrollLeft, width, dpr } = s;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, width, LAYOUT.rulerHeight);

  // Background + bottom divider.
  ctx.fillStyle = BG.surface;
  ctx.fillRect(0, 0, width, LAYOUT.rulerHeight);
  ctx.fillStyle = BORDER.primary;
  ctx.fillRect(0, LAYOUT.rulerHeight - 1, width, 1);

  // Marked-range band on the ruler (upstream `drawTimelineRangeSelectionRulerFill`
  // + `…Edges`): a soft fill across the range's x-span plus edge ticks. Drawn
  // under the ticks/labels so timecode stays legible. `x` is view-space here
  // (the ruler canvas is not scroll-translated), so subtract scrollLeft.
  const range = validRange(s.selectedRange ?? null);
  if (range) {
    const minX = xForFrame(range.startFrame, pixelsPerFrame) - scrollLeft;
    const maxX = xForFrame(range.endFrame, pixelsPerFrame) - scrollLeft;
    ctx.fillStyle = RANGE.rulerFill;
    ctx.fillRect(minX, 0, Math.max(0, maxX - minX), LAYOUT.rulerHeight);
    ctx.strokeStyle = RANGE.edge;
    ctx.lineWidth = 2;
    for (const x of [minX, maxX]) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, LAYOUT.rulerHeight);
      ctx.stroke();
    }
  }

  const { majorInterval, minorSubdivisions } = chooseTicks(pixelsPerFrame, fps);

  // First major frame at/after the left edge.
  const leftFrame = scrollLeft / pixelsPerFrame;
  const firstMajor = Math.floor(leftFrame / majorInterval) * majorInterval;
  const rightFrame = (scrollLeft + width) / pixelsPerFrame;

  // Minor ticks.
  if (minorSubdivisions > 1) {
    const minorInterval = majorInterval / minorSubdivisions;
    ctx.strokeStyle = "rgba(255,255,255,0.136)"; // text-muted * 0.4
    ctx.lineWidth = 0.5;
    for (let f = firstMajor; f <= rightFrame; f += minorInterval) {
      const x = f * pixelsPerFrame - scrollLeft;
      // Midpoint (half of an even subdivision) gets a taller 6px tick.
      const idx = Math.round((f - firstMajor) / minorInterval) % minorSubdivisions;
      const tall = minorSubdivisions % 2 === 0 && idx === minorSubdivisions / 2;
      const h = tall ? 6 : 4;
      ctx.beginPath();
      ctx.moveTo(x + 0.5, LAYOUT.rulerHeight - h);
      ctx.lineTo(x + 0.5, LAYOUT.rulerHeight);
      ctx.stroke();
    }
  }

  // Major ticks + labels.
  ctx.strokeStyle = TEXT.muted;
  ctx.lineWidth = 1;
  ctx.fillStyle = TEXT.tertiary;
  ctx.font = `10px ui-monospace, "SF Mono", Menlo, monospace`;
  ctx.textBaseline = "top";
  for (let f = firstMajor; f <= rightFrame; f += majorInterval) {
    const x = f * pixelsPerFrame - scrollLeft;
    ctx.beginPath();
    ctx.moveTo(x + 0.5, LAYOUT.rulerHeight - 8);
    ctx.lineTo(x + 0.5, LAYOUT.rulerHeight);
    ctx.stroke();
    ctx.fillText(formatTimecode(f, fps), x + 3, 2);
  }

  void ACCENT;
}
