import { describe, expect, it } from "vitest";
import { dbFromLinear, drawClip, waveformSampleRange } from "./clipRenderer";
import type { Clip } from "../../lib/types";

/** Minimal canvas-2d stub that records the fill/stroke styles used. Enough to
 *  observe which colors `drawClip` paints without a real canvas. */
function makeCtx() {
  const fills: string[] = [];
  const strokes: string[] = [];
  const filledPaths: Array<{ fillStyle: string; firstMoveTo: [number, number] | null }> = [];
  const fillRects: Array<{ fillStyle: string; x: number; y: number; width: number; height: number }> = [];
  const arcs: Array<{ x: number; y: number; radius: number }> = [];
  let firstMoveTo: [number, number] | null = null;
  const ctx = {
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 0,
    font: "",
    textBaseline: "",
    save() {},
    restore() {},
    beginPath() {
      firstMoveTo = null;
    },
    moveTo(x: number, y: number) {
      if (!firstMoveTo) firstMoveTo = [x, y];
    },
    lineTo() {},
    arc(x: number, y: number, radius: number) {
      arcs.push({ x, y, radius });
    },
    arcTo() {},
    closePath() {},
    clip() {},
    rect() {},
    fillText() {},
    measureText() {
      return { width: 10 };
    },
    fill() {
      fills.push(String(this.fillStyle));
      filledPaths.push({ fillStyle: String(this.fillStyle), firstMoveTo });
      firstMoveTo = null;
    },
    stroke() {
      strokes.push(String(this.strokeStyle));
    },
    fillRect(x: number, y: number, width: number, height: number) {
      fills.push(String(this.fillStyle));
      fillRects.push({ fillStyle: String(this.fillStyle), x, y, width, height });
    },
    strokeRect() {
      strokes.push(String(this.strokeStyle));
    },
  };
  return { ctx: ctx as unknown as CanvasRenderingContext2D, fills, strokes, filledPaths, fillRects, arcs };
}

const testClip = {
  id: "c1",
  mediaRef: "m1",
  mediaType: "audio",
  sourceClipType: "audio",
  startFrame: 0,
  durationFrames: 100,
  trimStartFrame: 0,
  trimEndFrame: 0,
  speed: 1,
  volume: 1,
  fadeInFrames: 0,
  fadeOutFrames: 0,
  fadeInInterpolation: "linear",
  fadeOutInterpolation: "linear",
  opacity: 1,
  transform: {},
  crop: {},
} as unknown as Clip;

describe("drawClip missing wash", () => {
  const rect = { x: 0, y: 0, width: 200, height: 60 };

  it("paints the systemRed wash + border when missing", () => {
    const { ctx, fills, strokes } = makeCtx();
    drawClip(ctx, testClip, rect, { isSelected: false, fps: 30, missing: true });
    expect(fills).toContain("rgba(255,59,48,0.35)");
    expect(strokes).toContain("rgb(255,59,48)");
  });

  it("draws no red wash when the asset is present", () => {
    const { ctx, fills } = makeCtx();
    drawClip(ctx, testClip, rect, { isSelected: false, fps: 30 });
    expect(fills).not.toContain("rgba(255,59,48,0.35)");
  });
});

describe("drawClip linkOffset badge", () => {
  const rect = { x: 0, y: 0, width: 200, height: 60 };

  it("draws the red offset badge when linkOffset is nonzero", () => {
    const { ctx, fills } = makeCtx();
    drawClip(ctx, testClip, rect, { isSelected: false, fps: 30, linkOffset: 5 });
    expect(fills).toContain("rgb(255,71,71)");
  });

  it("positions the red offset badge at the clip top-right", () => {
    const { ctx, filledPaths } = makeCtx();
    drawClip(ctx, testClip, rect, { isSelected: false, fps: 30, linkOffset: 5 });

    const badgePath = filledPaths.find((p) => p.fillStyle === "rgb(255,71,71)");
    expect(badgePath?.firstMoveTo?.[0]).toBeGreaterThan(170);
    expect(badgePath?.firstMoveTo?.[1]).toBeLessThan(10);
  });

  it("skips the badge when linkOffset is zero", () => {
    const { ctx, fills } = makeCtx();
    drawClip(ctx, testClip, rect, { isSelected: false, fps: 30, linkOffset: 0 });
    expect(fills).not.toContain("rgb(255,71,71)");
  });

  it("skips the badge when linkOffset is undefined", () => {
    const { ctx, fills } = makeCtx();
    drawClip(ctx, testClip, rect, { isSelected: false, fps: 30 });
    expect(fills).not.toContain("rgb(255,71,71)");
  });

  it("skips the badge on narrow clips (badge would overlap trim handle)", () => {
    const narrow = { x: 0, y: 0, width: 30, height: 40 };
    const { ctx, fills } = makeCtx();
    drawClip(ctx, testClip, narrow, { isSelected: false, fps: 30, linkOffset: 5 });
    // The width guard suppresses badges that would collide with both trim handles.
    expect(fills).not.toContain("rgb(255,71,71)");
  });

  it("keeps the duplicate ghost badge from covering the link-offset badge", () => {
    const { ctx, filledPaths, arcs } = makeCtx();
    drawClip(ctx, testClip, rect, {
      isSelected: false,
      fps: 30,
      ghost: true,
      isDuplicate: true,
      linkOffset: 5,
    });

    const offsetBadge = filledPaths.find((p) => p.fillStyle === "rgb(255,71,71)");
    const duplicateBadge = arcs[0];
    expect(offsetBadge?.firstMoveTo).toBeDefined();
    expect(duplicateBadge).toBeDefined();
    expect(duplicateBadge.x + duplicateBadge.radius).toBeLessThan(offsetBadge!.firstMoveTo![0]);
  });

  it("skips the duplicate ghost badge when the offset badge leaves no room", () => {
    const barelyOffsetEligible = { x: 0, y: 0, width: 31, height: 40 };
    const { ctx, filledPaths, arcs } = makeCtx();
    drawClip(ctx, testClip, barelyOffsetEligible, {
      isSelected: false,
      fps: 30,
      ghost: true,
      isDuplicate: true,
      linkOffset: 5,
    });

    expect(filledPaths.some((p) => p.fillStyle === "rgb(255,71,71)")).toBe(true);
    expect(arcs).toHaveLength(0);
  });
});

describe("drawClip fade knees", () => {
  const rect = { x: 0, y: 0, width: 200, height: 60 };
  const visualClip = {
    ...testClip,
    mediaType: "video",
    sourceClipType: "video",
  } as Clip;

  it("draws both selected knee handles even when fade lengths are zero", () => {
    const { ctx, fillRects } = makeCtx();
    drawClip(ctx, visualClip, rect, { isSelected: true, fps: 30 });

    const kneeRects = fillRects.filter(
      (r) => r.fillStyle === "rgba(255,255,255,0.95)" && r.width === 7 && r.height === 7,
    );
    expect(kneeRects).toHaveLength(2);
    expect(kneeRects[0].x).toBeCloseTo(2.5);
    expect(kneeRects[1].x).toBeCloseTo(190.5);
  });

  it("draws full-length fade handles at upstream edge-specific centers", () => {
    const { ctx: inCtx, fillRects: inRects } = makeCtx();
    drawClip(
      inCtx,
      { ...visualClip, fadeInFrames: 100 },
      rect,
      { isSelected: true, fps: 30 },
    );
    const fullInKnees = inRects.filter(
      (r) => r.fillStyle === "rgba(255,255,255,0.95)" && r.width === 7 && r.height === 7,
    );
    expect(fullInKnees[0].x).toBeCloseTo(196.5);

    const { ctx: outCtx, fillRects: outRects } = makeCtx();
    drawClip(
      outCtx,
      { ...visualClip, fadeOutFrames: 100 },
      rect,
      { isSelected: true, fps: 30 },
    );
    const fullOutKnees = outRects.filter(
      (r) => r.fillStyle === "rgba(255,255,255,0.95)" && r.width === 7 && r.height === 7,
    );
    expect(fullOutKnees[1].x).toBeCloseTo(-3.5);
  });
});

describe("dbFromLinear", () => {
  it("maps unity to 0 dB", () => {
    expect(dbFromLinear(1)).toBeCloseTo(0);
  });
  it("clamps to the floor at/below silence", () => {
    expect(dbFromLinear(0)).toBe(-60);
    expect(dbFromLinear(-1)).toBe(-60);
    expect(dbFromLinear(0.0001)).toBe(-60); // -80dB clamps up to floor
  });
  it("clamps to the +15 dB ceiling", () => {
    expect(dbFromLinear(100)).toBe(15);
  });
  it("is ~-6 dB at half amplitude", () => {
    expect(dbFromLinear(0.5)).toBeCloseTo(-6.02, 1);
  });
});

describe("waveformSampleRange", () => {
  const base = { durationFrames: 100, speed: 1, trimStartFrame: 0, trimEndFrame: 0 };

  it("untrimmed clip spans the whole sample array", () => {
    expect(waveformSampleRange(base, 1000)).toEqual({ start: 0, end: 1000 });
  });

  it("leading + trailing trim map proportionally", () => {
    // consumed=100, total=100+50+50=200 → start=50/200=0.25, end=150/200=0.75.
    const r = waveformSampleRange({ ...base, trimStartFrame: 50, trimEndFrame: 50 }, 1000);
    expect(r).toEqual({ start: 250, end: 750 });
  });

  it("speed changes the consumed source span", () => {
    // speed 2 → consumed=200, total=200 (no trim) → full range.
    const r = waveformSampleRange({ ...base, speed: 2 }, 1000);
    expect(r).toEqual({ start: 0, end: 1000 });
    // with leading trim 100: total=300, start=100/300, end=300/300.
    const r2 = waveformSampleRange({ ...base, speed: 2, trimStartFrame: 100 }, 900);
    expect(r2).toEqual({ start: 300, end: 900 });
  });

  it("returns an empty range for degenerate input", () => {
    expect(waveformSampleRange({ ...base, durationFrames: 0 }, 1000)).toEqual({ start: 0, end: 0 });
    expect(waveformSampleRange(base, 0)).toEqual({ start: 0, end: 0 });
  });
});
