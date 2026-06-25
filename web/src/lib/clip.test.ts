import { describe, expect, it } from "vitest";
import { clampTrimDeltaFrames, trimSourceValues, trimToPlayheadEdits } from "./clip";
import type { Clip, ClipType } from "./types";

function tc(over: Partial<{ durationFrames: number; speed: number; trimStartFrame: number; trimEndFrame: number; mediaType: ClipType }> = {}) {
  return {
    durationFrames: over.durationFrames ?? 100,
    speed: over.speed ?? 1,
    trimStartFrame: over.trimStartFrame ?? 0,
    trimEndFrame: over.trimEndFrame ?? 0,
    mediaType: over.mediaType ?? ("video" as ClipType),
  };
}

describe("trimSourceValues", () => {
  it("left edge: source delta = round(delta*speed), clamped at 0 for video", () => {
    expect(trimSourceValues(tc({ trimStartFrame: 5 }), "left", 20)).toEqual({ trimStartFrame: 25, trimEndFrame: 0 });
    // speed 2 → source delta 40
    expect(trimSourceValues(tc({ speed: 2, trimStartFrame: 10 }), "left", 20)).toEqual({ trimStartFrame: 50, trimEndFrame: 0 });
    // negative past 0 clamps for video
    expect(trimSourceValues(tc({ trimStartFrame: 5 }), "left", -10)).toEqual({ trimStartFrame: 0, trimEndFrame: 0 });
  });

  it("left edge: image/text are unbounded (may go negative)", () => {
    expect(trimSourceValues(tc({ trimStartFrame: 5, mediaType: "text" as ClipType }), "left", -10)).toEqual({
      trimStartFrame: -5,
      trimEndFrame: 0,
    });
  });

  it("right edge: newEnd = trimEnd - round(delta*speed)", () => {
    expect(trimSourceValues(tc({ trimEndFrame: 50 }), "right", 10)).toEqual({ trimStartFrame: 0, trimEndFrame: 40 });
  });
});

describe("clampTrimDeltaFrames", () => {
  it("left: caps positive delta so duration stays >=1", () => {
    expect(clampTrimDeltaFrames(tc({ durationFrames: 30 }), "left", 100)).toBe(29);
  });
  it("left: caps negative extend by available leading source (video)", () => {
    // trimStart 10, speed 1 → can extend left at most 10 timeline frames
    expect(clampTrimDeltaFrames(tc({ trimStartFrame: 10 }), "left", -50)).toBe(-10);
  });
  it("right: caps negative delta so duration stays >=1", () => {
    expect(clampTrimDeltaFrames(tc({ durationFrames: 30 }), "right", -100)).toBe(-29);
  });
  it("right: caps positive extend by available trailing source (video)", () => {
    expect(clampTrimDeltaFrames(tc({ trimEndFrame: 8 }), "right", 50)).toBe(8);
  });
  it("image/text left: no source floor on negative extend", () => {
    expect(clampTrimDeltaFrames(tc({ trimStartFrame: 0, mediaType: "image" as ClipType }), "left", -50)).toBe(-50);
  });
});

describe("trimToPlayheadEdits", () => {
  // clip [100, 200), speed 1, 10 frames trimmed off each end.
  const c = {
    id: "c",
    startFrame: 100,
    durationFrames: 100,
    speed: 1,
    trimStartFrame: 10,
    trimEndFrame: 10,
    mediaType: "video" as ClipType,
  } as unknown as Clip;

  it("left edge: trims the in-point to the playhead (剪映 Q, delete left)", () => {
    // delta 50 → source +50 → trimStart 10+50=60, right edge fixed.
    expect(trimToPlayheadEdits([c], 150, "left")).toEqual([
      { clipId: "c", trimStartFrame: 60, trimEndFrame: 10 },
    ]);
  });

  it("right edge: trims the out-point to the playhead (剪映 W, delete right)", () => {
    // delta -50 → trimEnd 10-(-50)=60, left edge fixed.
    expect(trimToPlayheadEdits([c], 150, "right")).toEqual([
      { clipId: "c", trimStartFrame: 10, trimEndFrame: 60 },
    ]);
  });

  it("skips clips the playhead is not strictly inside", () => {
    expect(trimToPlayheadEdits([c], 100, "left")).toEqual([]); // exactly at start
    expect(trimToPlayheadEdits([c], 200, "right")).toEqual([]); // exactly at end
    expect(trimToPlayheadEdits([c], 40, "left")).toEqual([]); // before the clip
    expect(trimToPlayheadEdits([c], 999, "right")).toEqual([]); // after the clip
  });
});
