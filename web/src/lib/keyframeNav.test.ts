import { describe, expect, it } from "vitest";
import {
  clipContainsFrame,
  hasKeyframeAt,
  keyframeFrames,
  nextKeyframeFrame,
  previousKeyframeFrame,
} from "./keyframeNav";
import type { Clip, Crop, Interpolation, KeyframeTrack, Transform } from "./types";

const tf: Transform = {
  centerX: 0.5,
  centerY: 0.5,
  width: 1,
  height: 1,
  rotation: 0,
  flipHorizontal: false,
  flipVertical: false,
};
const crop: Crop = { left: 0, top: 0, right: 0, bottom: 0 };
const smooth: Interpolation = "smooth";

/** Build a minimal clip at `startFrame` with `durationFrames`. Optional
 *  keyframe tracks carry CLIP-RELATIVE offsets (as stored). */
function clip(over: Partial<Clip> = {}): Clip {
  return {
    id: "c1",
    mediaRef: "m1",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 100,
    durationFrames: 60,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: smooth,
    fadeOutInterpolation: smooth,
    opacity: 1,
    transform: tf,
    crop,
    ...over,
  };
}

function scalarTrack(...offsets: number[]): KeyframeTrack<number> {
  return { keyframes: offsets.map((frame) => ({ frame, value: 0, interpolationOut: smooth })) };
}

describe("keyframeFrames", () => {
  it("maps clip-relative offsets to ABSOLUTE timeline frames via startFrame", () => {
    // startFrame 100, offsets {0, 30} -> absolute {100, 130}.
    const c = clip({ opacityTrack: scalarTrack(0, 30) });
    expect(keyframeFrames(c, "opacity")).toEqual([100, 130]);
  });

  it("returns an empty array when the property has no track", () => {
    expect(keyframeFrames(clip(), "opacity")).toEqual([]);
    expect(keyframeFrames(clip(), "position")).toEqual([]);
  });
});

describe("hasKeyframeAt", () => {
  it("is true only at an absolute frame that has a keyframe", () => {
    const c = clip({ opacityTrack: scalarTrack(0, 30) }); // abs 100, 130
    expect(hasKeyframeAt(c, "opacity", 100)).toBe(true);
    expect(hasKeyframeAt(c, "opacity", 130)).toBe(true);
    expect(hasKeyframeAt(c, "opacity", 115)).toBe(false);
    // The stored OFFSET (30) is not the absolute frame — must not match.
    expect(hasKeyframeAt(c, "opacity", 30)).toBe(false);
  });

  it("is false for a property with no track", () => {
    expect(hasKeyframeAt(clip(), "rotation", 100)).toBe(false);
  });
});

describe("previousKeyframeFrame / nextKeyframeFrame", () => {
  it("finds the nearest neighbor strictly before/after the frame (absolute)", () => {
    const c = clip({ opacityTrack: scalarTrack(0, 30, 50) }); // abs 100, 130, 150
    expect(previousKeyframeFrame(c, "opacity", 130)).toBe(100);
    expect(nextKeyframeFrame(c, "opacity", 130)).toBe(150);
  });

  it("excludes a keyframe exactly AT the frame (strict inequality)", () => {
    const c = clip({ opacityTrack: scalarTrack(0, 30, 50) }); // abs 100, 130, 150
    // At 130 the neighbors are 100 and 150 — 130 itself is neither prev nor next.
    expect(previousKeyframeFrame(c, "opacity", 130)).toBe(100);
    expect(nextKeyframeFrame(c, "opacity", 130)).toBe(150);
  });

  it("returns null when there is no neighbor on that side", () => {
    const c = clip({ opacityTrack: scalarTrack(0, 30) }); // abs 100, 130
    expect(previousKeyframeFrame(c, "opacity", 100)).toBeNull();
    expect(nextKeyframeFrame(c, "opacity", 130)).toBeNull();
    // Empty track: both null.
    expect(previousKeyframeFrame(clip(), "opacity", 120)).toBeNull();
    expect(nextKeyframeFrame(clip(), "opacity", 120)).toBeNull();
  });
});

describe("clipContainsFrame", () => {
  it("uses a half-open span [startFrame, startFrame + durationFrames)", () => {
    const c = clip({ startFrame: 100, durationFrames: 60 }); // [100, 160)
    expect(clipContainsFrame(c, 100)).toBe(true); // inclusive start
    expect(clipContainsFrame(c, 159)).toBe(true);
    expect(clipContainsFrame(c, 160)).toBe(false); // exclusive end
    expect(clipContainsFrame(c, 99)).toBe(false);
    expect(clipContainsFrame(c, 200)).toBe(false);
  });
});
