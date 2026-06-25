/**
 * Snap indicator (SPEC §5.7): a yellow dashed vertical line shown while a drag
 * is snapped to a clip edge / playhead. Viewport-relative, pointer-events off.
 */

import { ACCENT, LAYOUT } from "../../lib/theme";

interface Props {
  frame: number | null;
  pixelsPerFrame: number;
  scrollLeft: number;
  height: number;
}

export function SnapIndicator({ frame, pixelsPerFrame, scrollLeft, height }: Props) {
  if (frame === null) return null;
  const x = frame * pixelsPerFrame - scrollLeft + LAYOUT.trackHeaderWidth;
  return (
    <div
      aria-hidden
      style={{
        position: "absolute",
        left: x - 0.5,
        top: LAYOUT.rulerHeight,
        width: 1,
        height: height - LAYOUT.rulerHeight,
        backgroundImage: `repeating-linear-gradient(to bottom, ${ACCENT.systemYellow} 0 4px, transparent 4px 8px)`,
        zIndex: 90,
        pointerEvents: "none",
      }}
    />
  );
}
