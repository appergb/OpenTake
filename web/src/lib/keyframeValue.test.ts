import { describe, expect, it } from "vitest";
import { linearFromDb } from "./clip";
import {
  cropEdgeKeyframeValue,
  opacityKeyframeValue,
  positionXKeyframeValue,
  positionYKeyframeValue,
  rotationKeyframeValue,
  scaleKeyframeValue,
  volumeKeyframeValue,
} from "./keyframeValue";
import type { Crop, Transform } from "./types";

function tf(over: Partial<Transform> = {}): Transform {
  return {
    centerX: 0.5,
    centerY: 0.5,
    width: 1,
    height: 1,
    rotation: 0,
    flipHorizontal: false,
    flipVertical: false,
    ...over,
  };
}

function crop(over: Partial<Crop> = {}): Crop {
  return { left: 0, top: 0, right: 0, bottom: 0, ...over };
}

describe("opacityKeyframeValue", () => {
  it("converts a 0-100 percent field value into a 0-1 scalar", () => {
    expect(opacityKeyframeValue(100)).toEqual({ kind: "scalar", value: 1 });
    expect(opacityKeyframeValue(50)).toEqual({ kind: "scalar", value: 0.5 });
    expect(opacityKeyframeValue(0)).toEqual({ kind: "scalar", value: 0 });
  });
});

describe("rotationKeyframeValue", () => {
  it("passes degrees through unchanged", () => {
    expect(rotationKeyframeValue(45)).toEqual({ kind: "scalar", value: 45 });
    expect(rotationKeyframeValue(-90)).toEqual({ kind: "scalar", value: -90 });
  });
});

describe("volumeKeyframeValue", () => {
  it("converts linear amplitude (the field's control value) to dB (the track's unit)", () => {
    const result = volumeKeyframeValue(1);
    expect(result.kind).toBe("scalar");
    expect((result as { kind: "scalar"; value: number }).value).toBeCloseTo(0, 5); // 0 dB at unity gain
  });

  it("round-trips through linearFromDb -> volumeKeyframeValue -> ~same dB", () => {
    const dbIn = -6;
    const linear = linearFromDb(dbIn);
    const result = volumeKeyframeValue(linear);
    expect(result.kind).toBe("scalar");
    expect((result as { kind: "scalar"; value: number }).value).toBeCloseTo(dbIn, 5);
  });

  it("clamps silence to the volume floor (-60 dB), matching dbFromLinear", () => {
    const result = volumeKeyframeValue(0);
    expect(result).toEqual({ kind: "scalar", value: -60 });
  });
});

describe("scaleKeyframeValue", () => {
  it("builds an AnimPair {a: width, b: height} using the known media aspect", () => {
    // aspect 2 (source is twice as wide as tall relative to canvas): width 0.5 -> height 0.25
    const result = scaleKeyframeValue(tf({ width: 1, height: 0.5 }), 0.5, 2);
    expect(result).toEqual({ kind: "pair", value: { a: 0.5, b: 0.25 } });
  });

  it("falls back to the clip transform's own aspect when aspect is null (matches resizeTransformKeepingSourceAspect)", () => {
    // transform aspect = 1/0.5 = 2 -> width 0.4 -> height 0.2
    const result = scaleKeyframeValue(tf({ width: 1, height: 0.5 }), 0.4, null);
    expect(result).toEqual({ kind: "pair", value: { a: 0.4, b: 0.2 } });
  });
});

describe("positionXKeyframeValue", () => {
  it("writes the new X into `a` and preserves the sampled Y in `b`", () => {
    expect(positionXKeyframeValue(0.3, 0.75)).toEqual({ kind: "pair", value: { a: 0.3, b: 0.75 } });
  });
});

describe("positionYKeyframeValue", () => {
  it("preserves the sampled X in `a` and writes the new Y into `b`", () => {
    expect(positionYKeyframeValue(0.1, 0.9)).toEqual({ kind: "pair", value: { a: 0.1, b: 0.9 } });
  });
});

describe("cropEdgeKeyframeValue", () => {
  it("changes only the given edge, preserving the other three from the sampled crop", () => {
    const sampled = crop({ left: 0.1, top: 0.2, right: 0.3, bottom: 0.4 });
    expect(cropEdgeKeyframeValue(sampled, "left", 0.15)).toEqual({
      kind: "crop",
      value: { left: 0.15, top: 0.2, right: 0.3, bottom: 0.4 },
    });
    expect(cropEdgeKeyframeValue(sampled, "bottom", 0.5)).toEqual({
      kind: "crop",
      value: { left: 0.1, top: 0.2, right: 0.3, bottom: 0.5 },
    });
  });

  it("does not mutate the sampled crop object passed in", () => {
    const sampled = crop({ left: 0.1, top: 0.2, right: 0.3, bottom: 0.4 });
    const snapshot = { ...sampled };
    cropEdgeKeyframeValue(sampled, "right", 0.99);
    expect(sampled).toEqual(snapshot);
  });
});
