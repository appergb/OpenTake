/**
 * Playhead overlay (SPEC §5.6): a systemRed vertical line from the ruler down,
 * with a downward triangle at the top. Positioned relative to the viewport:
 * left = frame*ppf - scrollLeft. pointer-events disabled.
 */

import { ACCENT, LAYOUT, PLAYHEAD_TRIANGLE } from "../../lib/theme";

interface Props {
  frame: number;
  pixelsPerFrame: number;
  scrollLeft: number;
  height: number;
}

export function Playhead({ frame, pixelsPerFrame, scrollLeft, height }: Props) {
  const x = frame * pixelsPerFrame - scrollLeft + LAYOUT.trackHeaderWidth;
  if (x < LAYOUT.trackHeaderWidth - 1) return null;
  const t = PLAYHEAD_TRIANGLE;
  return (
    <div
      aria-hidden
      style={{
        position: "absolute",
        left: x,
        top: LAYOUT.rulerHeight - t,
        height: height - LAYOUT.rulerHeight + t,
        width: 0,
        zIndex: 100,
        pointerEvents: "none",
      }}
    >
      {/* downward triangle */}
      <div
        style={{
          position: "absolute",
          left: -t / 2,
          top: 0,
          width: 0,
          height: 0,
          borderLeft: `${t / 2}px solid transparent`,
          borderRight: `${t / 2}px solid transparent`,
          borderTop: `${t}px solid ${ACCENT.systemRed}`,
        }}
      />
      {/* vertical line */}
      <div
        style={{
          position: "absolute",
          left: -0.5,
          top: t,
          width: 1,
          height: height - LAYOUT.rulerHeight,
          background: ACCENT.systemRed,
        }}
      />
    </div>
  );
}
