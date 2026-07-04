import { describe, expect, it } from "vitest";
import {
  cropAspectLockPixelAspect,
  cropFittingAspect,
  cropForPreset,
  CROP_ASPECT_LOCKS,
  lockedAspectNormalized,
  pannedCrop,
  resizedCrop,
  visibleHeightFraction,
  visibleWidthFraction,
} from "./cropOverlay";
import type { Crop } from "./types";

const IDENTITY: Crop = { left: 0, top: 0, right: 0, bottom: 0 };
const clipRect = { width: 1000, height: 1000 };

describe("pannedCrop (upstream CropOverlayView.pannedCrop port)", () => {
  it("shifts left/top by dx/dy fraction while preserving visible size", () => {
    const start: Crop = { left: 0.2, top: 0.2, right: 0.2, bottom: 0.2 };
    const next = pannedCrop(start, { width: 100, height: 50 }, clipRect); // dx=0.1, dy=0.05
    expect(next.left).toBeCloseTo(0.3);
    expect(next.top).toBeCloseTo(0.25);
    // Visible fraction (0.6 x 0.6) unchanged.
    expect(visibleWidthFraction(next)).toBeCloseTo(visibleWidthFraction(start));
    expect(visibleHeightFraction(next)).toBeCloseTo(visibleHeightFraction(start));
  });

  it("clamps at the left/top edge (left never goes below 0)", () => {
    const start: Crop = { left: 0.2, top: 0.2, right: 0.2, bottom: 0.2 };
    const next = pannedCrop(start, { width: -1000, height: -1000 }, clipRect); // huge negative delta
    expect(next.left).toBe(0);
    expect(next.top).toBe(0);
    expect(visibleWidthFraction(next)).toBeCloseTo(visibleWidthFraction(start));
  });

  it("clamps at the right/bottom edge (left maxes at 1 - visibleWidth)", () => {
    const start: Crop = { left: 0.2, top: 0.2, right: 0.2, bottom: 0.2 };
    const next = pannedCrop(start, { width: 1000, height: 1000 }, clipRect); // huge positive delta
    const visW = visibleWidthFraction(start);
    const visH = visibleHeightFraction(start);
    expect(next.left).toBeCloseTo(1 - visW);
    expect(next.top).toBeCloseTo(1 - visH);
    expect(next.right).toBeCloseTo(0);
    expect(next.bottom).toBeCloseTo(0);
  });

  it("returns start unchanged for a degenerate (zero-size) clipRect", () => {
    const start: Crop = { left: 0.1, top: 0.1, right: 0.1, bottom: 0.1 };
    expect(pannedCrop(start, { width: 50, height: 50 }, { width: 0, height: 0 })).toBe(start);
  });
});

describe("resizedCrop — free (unlocked, aspectN=null)", () => {
  it("topLeft corner: dragging right+down increases left/top", () => {
    const next = resizedCrop(IDENTITY, "topLeft", { width: 100, height: 50 }, clipRect, null);
    expect(next.left).toBeCloseTo(0.1);
    expect(next.top).toBeCloseTo(0.05);
    expect(next.right).toBe(0);
    expect(next.bottom).toBe(0);
  });

  // Non-zero start so the per-corner sign convention is observable without the
  // max(0, …) floor kicking in (insets can never go negative — upstream
  // CropOverlayView.swift:144-147 clamps exactly like the implementation).
  const START: Crop = { left: 0.2, top: 0.2, right: 0.2, bottom: 0.2 };

  it("bottomRight corner: dragging right+down decreases right/bottom", () => {
    const next = resizedCrop(START, "bottomRight", { width: 100, height: 50 }, clipRect, null);
    expect(next.right).toBeCloseTo(0.1);
    expect(next.bottom).toBeCloseTo(0.15);
    expect(next.left).toBeCloseTo(0.2);
    expect(next.top).toBeCloseTo(0.2);
  });

  it("bottomRight outward drag from the identity crop floors at 0 (never negative)", () => {
    const next = resizedCrop(IDENTITY, "bottomRight", { width: 100, height: 50 }, clipRect, null);
    expect(next).toEqual({ left: 0, top: 0, right: 0, bottom: 0 });
  });

  it("topRight corner: dx decreases right (right -= dx), dy increases top", () => {
    const next = resizedCrop(START, "topRight", { width: 100, height: 50 }, clipRect, null);
    expect(next.right).toBeCloseTo(0.1);
    expect(next.top).toBeCloseTo(0.25);
  });

  it("bottomLeft corner: dx increases left, dy decreases bottom", () => {
    const next = resizedCrop(START, "bottomLeft", { width: 100, height: 50 }, clipRect, null);
    expect(next.left).toBeCloseTo(0.3);
    expect(next.bottom).toBeCloseTo(0.15);
  });

  it("clamps left+right to the MIN_VISIBLE (0.05) floor", () => {
    // Drag topLeft far enough right that left would exceed 1 - 0.05 - right(0).
    const next = resizedCrop(IDENTITY, "topLeft", { width: 2000, height: 0 }, clipRect, null);
    expect(next.left).toBeCloseTo(0.95);
    expect(visibleWidthFraction(next)).toBeCloseTo(0.05);
  });

  it("clamps left at 0 when dragging outward past the source edge", () => {
    const start: Crop = { left: 0.2, top: 0, right: 0, bottom: 0 };
    const next = resizedCrop(start, "topLeft", { width: -2000, height: 0 }, clipRect, null);
    expect(next.left).toBe(0);
  });

  it("returns start unchanged for a degenerate (zero-size) clipRect", () => {
    expect(resizedCrop(IDENTITY, "bottomRight", { width: 50, height: 50 }, { width: 0, height: 0 }, null)).toBe(
      IDENTITY,
    );
  });
});

describe("resizedCrop — aspect-locked (upstream resizedCropLocked port)", () => {
  // aspectN = target/source. Using aspectN=1 makes hand-derivation simple:
  // dragging topLeft inward by dx=0.1 (horizontal only) drives BOTH axes
  // equally to hold the 1:1 crop aspect, landing at left=top=0.1.
  it("topLeft: drives the non-dragged axis to hold aspectN=1 (unclamped)", () => {
    const next = resizedCrop(IDENTITY, "topLeft", { width: 100, height: 0 }, clipRect, 1);
    expect(next.left).toBeCloseTo(0.1);
    expect(next.top).toBeCloseTo(0.1);
    expect(next.right).toBe(0);
    expect(next.bottom).toBe(0);
    // Aspect held: visW / visH === aspectN.
    expect(visibleWidthFraction(next) / visibleHeightFraction(next)).toBeCloseTo(1);
  });

  it("bottomRight: clamps to the tighter of the horizontal/vertical ceiling, preserving aspectN", () => {
    // aspectN=0.5625 (e.g. cropping a 16:9 source down to a 1:1-normalized target).
    // The vertical extent (full height available) caps visW at aspectN*1=0.5625,
    // well below what an unclamped horizontal-only drag would ask for.
    const aspectN = 0.5625;
    const next = resizedCrop(IDENTITY, "bottomRight", { width: 100, height: 0 }, clipRect, aspectN);
    expect(next.right).toBeCloseTo(1 - 0.5625);
    expect(next.bottom).toBeCloseTo(0);
    expect(visibleWidthFraction(next) / visibleHeightFraction(next)).toBeCloseTo(aspectN);
  });

  it("drives from whichever axis moved further (width-equivalent units)", () => {
    // A pure-vertical drag (dx=0) must still shrink both axes together at aspectN=1.
    const next = resizedCrop(IDENTITY, "topLeft", { width: 0, height: 100 }, clipRect, 1);
    expect(next.left).toBeCloseTo(0.1);
    expect(next.top).toBeCloseTo(0.1);
  });

  it("returns start unchanged when the aspect-constrained ceiling is below the min-visible floor", () => {
    // left already at 0.98 leaves only 0.02 of width available (< MIN_VISIBLE=0.05
    // ceiling for a bottomRight resize, whose sMaxFromX = 1 - left).
    const start: Crop = { left: 0.98, top: 0, right: 0, bottom: 0 };
    const next = resizedCrop(start, "bottomRight", { width: 10, height: 0 }, clipRect, 1);
    expect(next).toBe(start);
  });

  it("returns start unchanged for a degenerate (zero-size) clipRect", () => {
    expect(resizedCrop(IDENTITY, "topLeft", { width: 50, height: 50 }, { width: 0, height: 0 }, 1)).toBe(IDENTITY);
  });
});

describe("lockedAspectNormalized (upstream lockedAspectNormalized(for:) port)", () => {
  it("divides target by source pixel aspect", () => {
    expect(lockedAspectNormalized(1, 16 / 9)).toBeCloseTo(9 / 16);
  });

  it("returns null when target is null (free/original presets)", () => {
    expect(lockedAspectNormalized(null, 16 / 9)).toBeNull();
  });

  it("returns null when source pixel aspect is unknown", () => {
    expect(lockedAspectNormalized(1, null)).toBeNull();
  });

  it("returns null for a non-positive source aspect", () => {
    expect(lockedAspectNormalized(1, 0)).toBeNull();
    expect(lockedAspectNormalized(1, -2)).toBeNull();
  });
});

describe("CropAspectLock preset table (Models/Timeline.swift:513-540 port)", () => {
  it("has the exact 8-preset order upstream's CaseIterable synthesizes", () => {
    expect(CROP_ASPECT_LOCKS).toEqual([
      "free",
      "original",
      "r16x9",
      "r9x16",
      "r1x1",
      "r4x3",
      "r3x4",
      "r21x9",
    ]);
  });

  it("free and original have no pixel aspect (unlocked)", () => {
    expect(cropAspectLockPixelAspect("free")).toBeNull();
    expect(cropAspectLockPixelAspect("original")).toBeNull();
  });

  it("sized presets resolve their exact pixel aspect", () => {
    expect(cropAspectLockPixelAspect("r16x9")).toBeCloseTo(16 / 9);
    expect(cropAspectLockPixelAspect("r9x16")).toBeCloseTo(9 / 16);
    expect(cropAspectLockPixelAspect("r1x1")).toBe(1);
    expect(cropAspectLockPixelAspect("r4x3")).toBeCloseTo(4 / 3);
    expect(cropAspectLockPixelAspect("r3x4")).toBeCloseTo(3 / 4);
    expect(cropAspectLockPixelAspect("r21x9")).toBeCloseTo(21 / 9);
  });
});

describe("cropFittingAspect (upstream EditorViewModel.cropFittingAspect(for:targetPixelAspect:) port)", () => {
  it("insets left/right for a wider-than-target source", () => {
    // 16:9 source (1.778) cropped to 1:1 (1.0): visibleWidthFrac = 1/1.778 = 0.5625.
    const crop = cropFittingAspect(16 / 9, 1);
    expect(crop.left).toBeCloseTo((1 - 9 / 16) / 2);
    expect(crop.right).toBeCloseTo((1 - 9 / 16) / 2);
    expect(crop.top).toBe(0);
    expect(crop.bottom).toBe(0);
  });

  it("insets top/bottom for a taller-than-target source", () => {
    // 9:16 source (0.5625) cropped to 1:1 (1.0): visibleHeightFrac = 0.5625.
    const crop = cropFittingAspect(9 / 16, 1);
    expect(crop.top).toBeCloseTo((1 - 9 / 16) / 2);
    expect(crop.bottom).toBeCloseTo((1 - 9 / 16) / 2);
    expect(crop.left).toBe(0);
    expect(crop.right).toBe(0);
  });

  it("returns the identity crop when source already matches target within tolerance", () => {
    expect(cropFittingAspect(16 / 9, 16 / 9)).toEqual(IDENTITY);
  });

  it("returns the identity crop when source pixel aspect is unavailable", () => {
    expect(cropFittingAspect(null, 1)).toEqual(IDENTITY);
    expect(cropFittingAspect(0, 1)).toEqual(IDENTITY);
    expect(cropFittingAspect(-1, 1)).toEqual(IDENTITY);
  });
});

describe("cropForPreset (upstream applyCropPreset(_:on:) crop-mutation branches)", () => {
  it("free returns null (no crop mutation — user keeps current shape)", () => {
    expect(cropForPreset("free", 16 / 9)).toBeNull();
  });

  it("original resets to the identity crop", () => {
    expect(cropForPreset("original", 16 / 9)).toEqual(IDENTITY);
  });

  it("a sized preset commits the largest centered crop at that pixel aspect", () => {
    const crop = cropForPreset("r1x1", 16 / 9);
    expect(crop).not.toBeNull();
    expect(crop!.left).toBeCloseTo((1 - 9 / 16) / 2);
    expect(crop!.top).toBe(0);
  });

  it("a sized preset with unavailable source aspect falls back to the identity crop", () => {
    expect(cropForPreset("r1x1", null)).toEqual(IDENTITY);
  });
});
