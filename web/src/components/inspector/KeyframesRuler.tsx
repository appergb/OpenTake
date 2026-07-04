/**
 * KeyframesRuler (SPEC §6.4, T2-8 part 3). Real tick ruler for the top of
 * KeyframesPanel, replacing the old static tinted strip. 1:1 reuse of the
 * main timeline's tick-interval logic — upstream's `ClipRulerBlock` wraps a
 * `RulerView` that itself just calls the shared `TimelineRuler.draw`
 * (Inspector/Keyframes/KeyframesLane.swift:335-362); OpenTake's equivalent
 * shared logic is `chooseTicks` (lib/ruler.ts, already a 1:1 port of
 * `TimelineRuler.swift`) plus `formatTimecode` for the major-tick labels,
 * which is exactly what the main timeline's own ruler painter
 * (components/timeline/rulerCanvas.ts) uses.
 *
 * Rendered as absolutely-positioned divs (not canvas) to match this panel's
 * existing JSX/CSS-var idiom (diamonds and the playhead overlay are divs too).
 * Ticks are clip-relative: frame 0 = clip start, so `pixelsPerFrame = width /
 * duration` — unlike the scrolling main-timeline ruler, there is no
 * `scrollLeft` to account for.
 */

import { useLayoutEffect, useRef, useState } from "react";
import { chooseTicks } from "../../lib/ruler";
import { formatTimecode } from "../../lib/geometry";

const RULER_HEIGHT = 18; // upstream KeyframesMetrics.rulerHeight (KeyframesLane.swift:5)

export function KeyframesRuler({ duration, fps }: { duration: number; fps: number }) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width;
      if (typeof w === "number") setWidth(w);
    });
    observer.observe(el);
    setWidth(el.getBoundingClientRect().width);
    return () => observer.disconnect();
  }, []);

  const pixelsPerFrame = width > 0 ? width / duration : 0;
  const ticks = pixelsPerFrame > 0 ? buildTicks(duration, fps, pixelsPerFrame) : [];

  return (
    <div
      ref={ref}
      style={{
        height: RULER_HEIGHT,
        position: "relative",
        marginBottom: "var(--space-xs)",
        borderBottom: "var(--bw-hairline) solid var(--border-subtle)",
      }}
    >
      {ticks.map((tick) => (
        <div
          key={tick.frame}
          style={{
            position: "absolute",
            left: tick.frame * pixelsPerFrame,
            bottom: 0,
            width: 1,
            height: tick.major ? 8 : 4,
            background: "var(--text-muted)",
          }}
        >
          {tick.major && tick.label && (
            <span
              style={{
                position: "absolute",
                left: 3,
                top: -RULER_HEIGHT + 8,
                fontSize: "var(--fs-micro)",
                color: "var(--text-tertiary)",
                fontFamily: "var(--font-mono)",
                whiteSpace: "nowrap",
              }}
            >
              {tick.label}
            </span>
          )}
        </div>
      ))}
    </div>
  );
}

interface Tick {
  frame: number;
  major: boolean;
  label: string | null;
}

/** Build clip-relative ticks spanning [0, duration] using the shared
 *  major/minor interval selection (chooseTicks, a 1:1 port of
 *  TimelineRuler.swift's own interval logic). */
function buildTicks(duration: number, fps: number, pixelsPerFrame: number): Tick[] {
  const { majorInterval, minorSubdivisions } = chooseTicks(pixelsPerFrame, fps);
  const ticks: Tick[] = [];
  const minorInterval = majorInterval / minorSubdivisions;
  for (let f = 0; f <= duration; f += minorInterval) {
    const frame = Math.round(f);
    const isMajor = frame % majorInterval === 0;
    ticks.push({ frame, major: isMajor, label: isMajor ? formatTimecode(frame, fps) : null });
  }
  return ticks;
}
