import { describe, expect, it } from "vitest";
import { normalizedMotionRegion } from "./MotionTrackingOverlay";

describe("normalizedMotionRegion", () => {
  it("normalizes_reverse_drags_and_clamps_them_to_the_preview", () => {
    expect(normalizedMotionRegion({ x: 0.8, y: 0.7 }, { x: 0.2, y: 0.1 })).toEqual({
      x: 0.2,
      y: 0.1,
      width: 0.6000000000000001,
      height: 0.6,
    });
    expect(normalizedMotionRegion({ x: -1, y: -1 }, { x: 2, y: 2 })).toEqual({
      x: 0,
      y: 0,
      width: 1,
      height: 1,
    });
  });

  it("keeps_clicks_as_a_non_empty_trackable_region", () => {
    expect(normalizedMotionRegion({ x: 1, y: 1 }, { x: 1, y: 1 })).toEqual({
      x: 0.98,
      y: 0.98,
      width: 0.02,
      height: 0.02,
    });
  });
});
