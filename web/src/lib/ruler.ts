/**
 * Ruler tick interval selection — 1:1 port of `Timeline/TimelineRuler.swift`
 * (SPEC §5.3). Major tick targets ~80px; minor subdivision picks the first
 * factor keeping each cell >= 12px.
 */

/** Candidate major intervals in seconds (TimelineRuler.swift:87-94), scaled by fps. */
const MAJOR_SECONDS = [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1200, 1800, 3600];
const MINOR_SUBDIVISIONS = [10, 5, 4, 2];

export interface RulerTicks {
  /** Major tick interval in frames. */
  majorInterval: number;
  /** Number of minor subdivisions per major (1 = none). */
  minorSubdivisions: number;
}

/** Choose the first interval whose pixel span is >= 80px (TimelineRuler:87-94)
 *  and the first subdivision keeping minor cells >= 12px (:97-106). */
export function chooseTicks(pixelsPerFrame: number, fps: number): RulerTicks {
  const safeFps = fps > 0 ? fps : 30;
  const targetPx = 80;
  let majorInterval = MAJOR_SECONDS[MAJOR_SECONDS.length - 1] * safeFps;
  for (const sec of MAJOR_SECONDS) {
    const frames = sec * safeFps;
    if (frames * pixelsPerFrame >= targetPx) {
      majorInterval = frames;
      break;
    }
  }

  let minorSubdivisions = 1;
  for (const sub of MINOR_SUBDIVISIONS) {
    const cellPx = (majorInterval / sub) * pixelsPerFrame;
    if (cellPx >= 12) {
      minorSubdivisions = sub;
      break;
    }
  }

  return { majorInterval, minorSubdivisions };
}
