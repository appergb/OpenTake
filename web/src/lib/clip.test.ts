import { describe, expect, it } from "vitest";
import {
  clampTrimDeltaFrames,
  dbFromLinear,
  findCropEditingClip,
  findSelectedVisualClip,
  fitTransformForMedia,
  liveVolumeKfLinearAt,
  mediaCanvasAspect,
  moveTransformByDelta,
  opacityAt,
  rawOpacityAt,
  resizeTransformKeepingSourceAspect,
  rotateDeltaIntoLocalFrame,
  sampledTransform,
  trimSourceValues,
  trimToPlayheadEdits,
  volumeAt,
} from "./clip";
import type { Clip, ClipType, Timeline, Track, Transform } from "./types";

/** Full-Clip fixture, matching previewLayerStyles.test.ts / Preview.test.tsx's
 *  shape so fixtures stay consistent across the codebase. */
function clip(over: Partial<Clip> = {}): Clip {
  return {
    id: "clip",
    mediaRef: "asset",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 0,
    durationFrames: 100,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "smooth",
    fadeOutInterpolation: "smooth",
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
    ...over,
  };
}

function track(over: Partial<Track> & { id: string; type: ClipType; clips: Clip[] }): Track {
  return {
    id: over.id,
    type: over.type,
    muted: over.muted ?? false,
    hidden: over.hidden ?? false,
    syncLocked: over.syncLocked ?? true,
    clips: over.clips,
  };
}

function timeline(tracks: Track[]): Timeline {
  return { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks };
}

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

describe("sampledTransform (upstream Clip.transformAt(frame:) port)", () => {
  it("with no active tracks, recomposes the clip's static transform unchanged", () => {
    const c = clip({
      transform: {
        centerX: 0.3,
        centerY: 0.7,
        width: 0.4,
        height: 0.25,
        rotation: 15,
        flipHorizontal: true,
        flipVertical: false,
      },
    });
    const s = sampledTransform(c, 0);
    expect(s.centerX).toBeCloseTo(c.transform.centerX);
    expect(s.centerY).toBeCloseTo(c.transform.centerY);
    expect(s.width).toBe(c.transform.width);
    expect(s.height).toBe(c.transform.height);
    expect(s.rotation).toBe(c.transform.rotation);
    expect(s.flipHorizontal).toBe(c.transform.flipHorizontal);
    expect(s.flipVertical).toBe(c.transform.flipVertical);
  });

  it("follows an active position/scale/rotation track at an exact keyframe", () => {
    const c = clip({
      startFrame: 100,
      positionTrack: {
        keyframes: [
          { frame: 0, value: { a: 0.1, b: 0.2 }, interpolationOut: "linear" },
          { frame: 50, value: { a: 0.6, b: 0.1 }, interpolationOut: "hold" },
        ],
      },
      scaleTrack: {
        keyframes: [{ frame: 0, value: { a: 0.5, b: 0.3 }, interpolationOut: "hold" }],
      },
      rotationTrack: {
        keyframes: [
          { frame: 0, value: 45, interpolationOut: "hold" },
          { frame: 50, value: 45, interpolationOut: "hold" },
        ],
      },
    });

    // Absolute frame 150 = clip-relative 50 = the second (held) keyframe.
    const s = sampledTransform(c, 150);
    expect(s.rotation).toBe(45);
    expect(s.width).toBeCloseTo(0.5);
    expect(s.height).toBeCloseTo(0.3);
    // topLeftAt returns the sampled position directly; center = topLeft + size/2.
    expect(s.centerX).toBeCloseTo(0.6 + 0.5 / 2);
    expect(s.centerY).toBeCloseTo(0.1 + 0.3 / 2);
  });

  it("passes flipHorizontal/flipVertical through from the static transform (never keyframed)", () => {
    const c = clip({ transform: { ...clip().transform, flipHorizontal: true, flipVertical: true } });
    const s = sampledTransform(c, 0);
    expect(s.flipHorizontal).toBe(true);
    expect(s.flipVertical).toBe(true);
  });
});

describe("moveTransformByDelta (upstream TransformOverlayView.movedTransform port)", () => {
  const canvas = { width: 1000, height: 1000 };
  const start: Transform = {
    centerX: 0.5,
    centerY: 0.5,
    width: 0.2,
    height: 0.2,
    rotation: 0,
    flipHorizontal: false,
    flipVertical: false,
  };

  it("translates the center by delta/canvasPx on each axis", () => {
    const moved = moveTransformByDelta(start, { width: 100, height: 50 }, canvas, false);
    expect(moved.centerX).toBeCloseTo(0.6);
    expect(moved.centerY).toBeCloseTo(0.55);
    expect(moved.width).toBe(0.2);
    expect(moved.height).toBe(0.2);
  });

  it("snaps the left edge to the canvas boundary within threshold", () => {
    // topLeft.x after translate = 0.5-0.1 + delta/1000. Land it 3px from x=0.
    const moved = moveTransformByDelta(start, { width: -397, height: 0 }, canvas, false, 8);
    expect(moved.centerX).toBeCloseTo(0.1); // left edge pinned to exactly 0
  });

  it("snaps the center to the canvas center within threshold", () => {
    const nudged = { ...start, centerX: 0.503, centerY: 0.497 };
    const moved = moveTransformByDelta(nudged, { width: 0, height: 0 }, canvas, false, 8);
    expect(moved.centerX).toBe(0.5);
    expect(moved.centerY).toBe(0.5);
  });

  it("skips all snapping while rotated, even within threshold", () => {
    const rotatedStart: Transform = { ...start, rotation: 30, centerX: 0.503 };
    const moved = moveTransformByDelta(rotatedStart, { width: 0, height: 0 }, canvas, true, 8);
    expect(moved.centerX).toBe(0.503); // not snapped to 0.5
  });

  it("skips snapping when snapThresholdPx is omitted (defaults to 0)", () => {
    const nudged = { ...start, centerX: 0.5001 };
    const moved = moveTransformByDelta(nudged, { width: 0, height: 0 }, canvas, false);
    expect(moved.centerX).toBe(0.5001);
  });

  it("returns start unchanged for a degenerate (zero-size) canvas", () => {
    expect(moveTransformByDelta(start, { width: 50, height: 50 }, { width: 0, height: 0 }, false)).toBe(start);
  });
});

describe("rotateDeltaIntoLocalFrame", () => {
  it("is the identity at 0 degrees", () => {
    expect(rotateDeltaIntoLocalFrame({ width: 12, height: -7 }, 0)).toEqual({ width: 12, height: -7 });
  });

  it("maps a screen-down drag to local +X at 90 degrees (box rotated 90deg CW)", () => {
    const local = rotateDeltaIntoLocalFrame({ width: 0, height: 10 }, 90);
    expect(local.width).toBeCloseTo(10);
    expect(local.height).toBeCloseTo(0);
  });

  it("negates both axes at 180 degrees", () => {
    const local = rotateDeltaIntoLocalFrame({ width: 5, height: 3 }, 180);
    expect(local.width).toBeCloseTo(-5);
    expect(local.height).toBeCloseTo(-3);
  });

  it("maps a screen-down drag to local -X at -90 degrees", () => {
    const local = rotateDeltaIntoLocalFrame({ width: 0, height: 10 }, -90);
    expect(local.width).toBeCloseTo(-10);
    expect(local.height).toBeCloseTo(0);
  });

  it("preserves vector magnitude at a non-axis-aligned angle", () => {
    const input = { width: 8, height: -6 };
    const local = rotateDeltaIntoLocalFrame(input, 37);
    const beforeMag = Math.hypot(input.width, input.height);
    const afterMag = Math.hypot(local.width, local.height);
    expect(afterMag).toBeCloseTo(beforeMag);
  });
});

describe("findSelectedVisualClip", () => {
  const visual = clip({ id: "v1", mediaType: "video" });
  const textClip = clip({ id: "t1", mediaType: "text" });
  const audioClip = clip({ id: "a1", mediaType: "audio" });
  const tl = timeline([
    track({ id: "vt", type: "video", clips: [visual, textClip] }),
    track({ id: "at", type: "audio", clips: [audioClip] }),
  ]);

  it("resolves the single selected visual clip", () => {
    expect(findSelectedVisualClip(tl, new Set(["v1"]))).toBe(visual);
    expect(findSelectedVisualClip(tl, new Set(["t1"]))).toBe(textClip);
  });

  it("returns null when the single selected clip is on an audio track", () => {
    expect(findSelectedVisualClip(tl, new Set(["a1"]))).toBeNull();
  });

  it("returns null when zero clips are selected", () => {
    expect(findSelectedVisualClip(tl, new Set())).toBeNull();
  });

  it("returns null when more than one clip is selected (marquee)", () => {
    expect(findSelectedVisualClip(tl, new Set(["v1", "t1"]))).toBeNull();
  });

  it("returns null when selectedClipIds is undefined (defensive for test-store mocks)", () => {
    expect(findSelectedVisualClip(tl, undefined)).toBeNull();
  });

  it("returns null for an id that doesn't exist in the timeline", () => {
    expect(findSelectedVisualClip(tl, new Set(["missing"]))).toBeNull();
  });
});

describe("findCropEditingClip (upstream CropOverlayView.selectedClip port)", () => {
  const visual = clip({ id: "v1", mediaType: "video" });
  const textClip = clip({ id: "t1", mediaType: "text" });
  const audioClip = clip({ id: "a1", mediaType: "audio" });
  const tl = timeline([
    track({ id: "vt", type: "video", clips: [visual, textClip] }),
    track({ id: "at", type: "audio", clips: [audioClip] }),
  ]);

  it("resolves the single selected visual clip", () => {
    expect(findCropEditingClip(tl, new Set(["v1"]))).toBe(visual);
  });

  it("returns null when the single selected clip is a text clip (unlike findSelectedVisualClip)", () => {
    expect(findCropEditingClip(tl, new Set(["t1"]))).toBeNull();
  });

  it("returns null when the single selected clip is on an audio track", () => {
    expect(findCropEditingClip(tl, new Set(["a1"]))).toBeNull();
  });

  it("returns null when zero clips are selected", () => {
    expect(findCropEditingClip(tl, new Set())).toBeNull();
  });

  it("returns null when more than one clip is selected (marquee)", () => {
    expect(findCropEditingClip(tl, new Set(["v1", "t1"]))).toBeNull();
  });

  it("returns null when selectedClipIds is undefined (defensive for test-store mocks)", () => {
    expect(findCropEditingClip(tl, undefined)).toBeNull();
  });

  it("returns null for an id that doesn't exist in the timeline", () => {
    expect(findCropEditingClip(tl, new Set(["missing"]))).toBeNull();
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
