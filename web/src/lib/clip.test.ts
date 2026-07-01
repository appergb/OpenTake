import { describe, expect, it } from "vitest";
import {
  clampTrimDeltaFrames,
  dbFromLinear,
  fitTransformForMedia,
  liveVolumeKfLinearAt,
  mediaCanvasAspect,
  opacityAt,
  rawOpacityAt,
  resizeTransformKeepingSourceAspect,
  trimSourceValues,
  trimToPlayheadEdits,
  volumeAt,
} from "./clip";
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

describe("media aspect transform helpers", () => {
  it("fits 16:9 media to a 16:9 canvas without changing normalized size", () => {
    expect(fitTransformForMedia(1920, 1080, 1920, 1080)).toMatchObject({
      width: 1,
      height: 1,
    });
    expect(mediaCanvasAspect(1920, 1080, 1920, 1080)).toBeCloseTo(1);
  });

  it("fits vertical media inside a horizontal canvas like upstream fitTransform", () => {
    const fitted = fitTransformForMedia(1080, 1920, 1920, 1080);

    expect(fitted.width).toBeCloseTo(0.31640625);
    expect(fitted.height).toBe(1);
    expect(fitted.centerX).toBe(0.5);
    expect(fitted.centerY).toBe(0.5);
    expect(mediaCanvasAspect(1080, 1920, 1920, 1080)).toBeCloseTo(0.31640625);
  });

  it("uses upstream's loose aspect tolerance for nearly matching media", () => {
    expect(fitTransformForMedia(1910, 1080, 1920, 1080)).toMatchObject({
      width: 1,
      height: 1,
    });
  });

  it("resizes by width while preserving source aspect relative to the canvas", () => {
    const resized = resizeTransformKeepingSourceAspect(
      {
        centerX: 0.5,
        centerY: 0.5,
        width: 0.31640625,
        height: 1,
        rotation: 0,
        flipHorizontal: false,
        flipVertical: false,
      },
      0.5,
      mediaCanvasAspect(1080, 1920, 1920, 1080),
    );

    expect(resized.width).toBeCloseTo(0.5);
    expect(resized.height).toBeCloseTo(1.5802469);
    expect(resized.centerX).toBe(0.5);
    expect(resized.centerY).toBe(0.5);
  });

  it("keeps current transform aspect when source dimensions are unavailable", () => {
    const resized = resizeTransformKeepingSourceAspect(
      {
        centerX: 0.4,
        centerY: 0.6,
        width: 0.8,
        height: 0.2,
        rotation: 12,
        flipHorizontal: true,
        flipVertical: false,
      },
      0.5,
      null,
    );

    expect(resized).toMatchObject({
      centerX: 0.4,
      centerY: 0.6,
      width: 0.5,
      height: 0.125,
      rotation: 12,
      flipHorizontal: true,
    });
  });
});

function fullClip(overrides: Partial<Clip> = {}): Clip {
  return {
    id: "c1",
    mediaRef: "c1-media",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 10,
    durationFrames: 40,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform: {
      centerX: 0.5,
      centerY: 0.5,
      width: 1,
      height: 1,
      rotation: 0,
      flipHorizontal: false,
      flipVertical: false,
    },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    ...overrides,
  };
}

// Regression guard for the Inspector "edit an animated value" seed. Upstream
// (InspectorView.swift writeVolume/writeOpacity) seeds the editable field from
// the RAW keyframe-track sample — NOT the composited output — so editing an
// animated value upserts the authored value instead of baking in the static
// outer volume / fade envelope. See liveVolumeKfLinearAt + rawOpacityAt.
describe("animated-value inspector seed (raw track sample, no fade/gain)", () => {
  it("liveVolumeKfLinearAt is null when the clip has no volume keyframes", () => {
    expect(liveVolumeKfLinearAt(fullClip(), 15)).toBeNull();
  });

  it("liveVolumeKfLinearAt returns the raw track gain, excluding static volume and fade", () => {
    // -6 dB authored keyframe, 2x static outer volume, sampled inside the fade-in.
    const clip = fullClip({
      mediaType: "audio",
      sourceClipType: "audio",
      volume: 2,
      fadeInFrames: 20,
      volumeTrack: { keyframes: [{ frame: 0, value: -6, interpolationOut: "linear" }] },
    });
    const raw = liveVolumeKfLinearAt(clip, 15);
    expect(raw).not.toBeNull();
    // Round-trips back to the authored -6 dB → editing without change is idempotent.
    expect(dbFromLinear(raw as number)).toBeCloseTo(-6);
    // The composited output differs (× static volume 2 × the 0.25 fade ramp at
    // rel-frame 5), proving the seed is NOT taken from it.
    expect(volumeAt(clip, 15)).toBeCloseTo((raw as number) * 2 * 0.25);
  });

  it("rawOpacityAt returns the authored track value, excluding the fade envelope", () => {
    const clip = fullClip({
      fadeInFrames: 20,
      opacityTrack: { keyframes: [{ frame: 0, value: 0.8, interpolationOut: "linear" }] },
    });
    // Raw = authored 0.8; effective = 0.8 × 0.25 fade at rel-frame 5.
    expect(rawOpacityAt(clip, 15)).toBeCloseTo(0.8);
    expect(opacityAt(clip, 15)).toBeCloseTo(0.2);
  });
});
