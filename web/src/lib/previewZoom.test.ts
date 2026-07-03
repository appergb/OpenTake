import { describe, expect, it } from "vitest";
import {
  CANVAS_ZOOM_MAX,
  CANVAS_ZOOM_MIN,
  applyScrollZoom,
  clampCanvasZoom,
  zoomAroundPoint,
} from "./previewZoom";

describe("clampCanvasZoom", () => {
  it("clamps to the upstream [0.1, 8.0] bounds", () => {
    expect(clampCanvasZoom(0.0001)).toBe(CANVAS_ZOOM_MIN);
    expect(clampCanvasZoom(100)).toBe(CANVAS_ZOOM_MAX);
    expect(clampCanvasZoom(1.5)).toBe(1.5);
  });
  it("falls back to Fit on non-finite input (NaN / Infinity)", () => {
    expect(clampCanvasZoom(NaN)).toBe(1.0);
    expect(clampCanvasZoom(Infinity)).toBe(1.0);
    expect(clampCanvasZoom(-Infinity)).toBe(1.0);
  });
});

describe("zoomAroundPoint", () => {
  // Mirrors upstream PreviewView.swift:21-32 exactly, with a hand-computable case.
  it("keeps the cursor anchor fixed as the centered stage scales", () => {
    // Fit box 800x450 at oldZoom=1 → viewSize 800x450. Zoom to 2 around the
    // top-left corner (0,0): dx = 800*(2-1)/2 + 0 = 400; dy = 450/2 = 225.
    const out = zoomAroundPoint({
      oldZoom: 1,
      newZoom: 2,
      pointTopDown: { x: 0, y: 0 },
      viewSize: { width: 800, height: 450 },
      offset: { width: 0, height: 0 },
    });
    expect(out.width).toBeCloseTo(400, 6);
    expect(out.height).toBeCloseTo(225, 6);
  });

  it("adds the shift onto the pre-existing offset", () => {
    const out = zoomAroundPoint({
      oldZoom: 2,
      newZoom: 2, // no scale change → shift terms are all 0
      pointTopDown: { x: 123, y: 45 },
      viewSize: { width: 1600, height: 900 },
      offset: { width: 10, height: -20 },
    });
    expect(out.width).toBeCloseTo(10, 6);
    expect(out.height).toBeCloseTo(-20, 6);
  });

  it("anchoring the center point yields the pure recentering term", () => {
    // Center of an 800x450 view at zoom 1 is (400,225). Zoom to 1.5:
    // dx = 800*0.5/2 + 400*(1 - 1.5) = 200 - 200 = 0 (center stays put).
    const out = zoomAroundPoint({
      oldZoom: 1,
      newZoom: 1.5,
      pointTopDown: { x: 400, y: 225 },
      viewSize: { width: 800, height: 450 },
      offset: { width: 0, height: 0 },
    });
    expect(out.width).toBeCloseTo(0, 6);
    expect(out.height).toBeCloseTo(0, 6);
  });

  it("returns the offset unchanged for a degenerate zoom", () => {
    const off = { width: 5, height: 7 };
    expect(zoomAroundPoint({ oldZoom: 0, newZoom: 2, pointTopDown: { x: 1, y: 1 }, viewSize: { width: 10, height: 10 }, offset: off })).toBe(off);
  });
});

describe("applyScrollZoom", () => {
  it("zooms in and anchors the offset when above Fit", () => {
    const res = applyScrollZoom({
      oldZoom: 1,
      deltaZoom: Math.log(2), // factor = exp(ln2) = 2
      pointTopDown: { x: 0, y: 0 },
      viewSize: { width: 800, height: 450 },
      offset: { width: 0, height: 0 },
    });
    expect(res.zoom).toBeCloseTo(2, 6);
    expect(res.offset.width).toBeCloseTo(400, 6);
    expect(res.offset.height).toBeCloseTo(225, 6);
  });

  it("resets the offset to zero when zooming out to or below Fit", () => {
    const res = applyScrollZoom({
      oldZoom: 1.2,
      deltaZoom: Math.log(0.5), // → 0.6, below Fit
      pointTopDown: { x: 100, y: 100 },
      viewSize: { width: 960, height: 540 },
      offset: { width: 30, height: 40 },
    });
    expect(res.zoom).toBeCloseTo(0.6, 6);
    expect(res.offset).toEqual({ width: 0, height: 0 });
  });

  it("is a no-op when the clamp already pins zoom at a bound", () => {
    const off = { width: 3, height: 4 };
    const res = applyScrollZoom({
      oldZoom: CANVAS_ZOOM_MAX,
      deltaZoom: Math.log(2), // would exceed max → clamp pins it
      pointTopDown: { x: 1, y: 1 },
      viewSize: { width: 100, height: 100 },
      offset: off,
    });
    expect(res.zoom).toBe(CANVAS_ZOOM_MAX);
    expect(res.offset).toBe(off);
  });
});
