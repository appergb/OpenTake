import { describe, expect, it } from "vitest";
import { trackY } from "../../lib/geometry";
import type { Clip, Timeline, Track } from "../../lib/types";
import { audioVolumeKfHit, clipsInRect, fadeFramesForDrag, fadeKneeHit, hitTestClip } from "./hitTest";

function clip(id: string, overrides: Partial<Clip> = {}): Clip {
  return {
    id,
    mediaRef: `${id}-media`,
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

function track(id: string, hidden: boolean, clips: Clip[]): Track {
  return {
    id,
    type: clips[0]?.mediaType ?? "video",
    muted: false,
    hidden,
    syncLocked: true,
    clips,
  };
}

function timeline(tracks: Track[]): Timeline {
  return {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks,
  };
}

describe("timeline hit testing", () => {
  const heights = {};
  const pixelsPerFrame = 2;

  it("does not return clips on hidden tracks", () => {
    const tl = timeline([track("v1", true, [clip("hidden")])]);
    const y = trackY(tl, 0, heights) + 10;

    expect(hitTestClip(tl, 25, y, pixelsPerFrame, heights)).toBeNull();
  });

  it("returns a visible clip after skipping a hidden track", () => {
    const tl = timeline([
      track("v1", true, [clip("hidden")]),
      track("v2", false, [clip("visible")]),
    ]);
    const y = trackY(tl, 1, heights) + 10;

    expect(hitTestClip(tl, 25, y, pixelsPerFrame, heights)?.clip.id).toBe("visible");
  });

  it("does not marquee-select clips on hidden tracks", () => {
    const tl = timeline([
      track("v1", true, [clip("hidden")]),
      track("v2", false, [clip("visible")]),
    ]);
    const y0 = trackY(tl, 0, heights);
    const y1 = trackY(tl, 2, heights);

    expect(clipsInRect(tl, 0, y0, 200, y1, pixelsPerFrame, heights)).toEqual(
      new Set(["visible"]),
    );
  });

  it("does not return volume keyframes on hidden audio tracks", () => {
    const audioClip = clip("audio", {
      mediaType: "audio",
      sourceClipType: "audio",
      volumeTrack: { keyframes: [{ frame: 10, value: 0.5, interpolationOut: "linear" }] },
    });
    const tl = timeline([track("a1", true, [audioClip])]);
    const y = trackY(tl, 0, heights) + 35;

    expect(audioVolumeKfHit(tl, 40, y, pixelsPerFrame, heights)).toBeNull();
  });

  it("returns volume keyframes on visible audio tracks after a hidden track", () => {
    const audioClip = clip("audio", {
      mediaType: "audio",
      sourceClipType: "audio",
      volumeTrack: { keyframes: [{ frame: 10, value: 0.5, interpolationOut: "linear" }] },
    });
    const tl = timeline([
      track("v1", true, [clip("hidden")]),
      track("a1", false, [audioClip]),
    ]);
    const y = trackY(tl, 1, heights) + 33;

    expect(audioVolumeKfHit(tl, 42, y, pixelsPerFrame, heights)).toEqual({
      clipId: "audio",
      frame: 10,
    });
  });
});

describe("fade knee hit testing", () => {
  const heights = {};
  const pixelsPerFrame = 2;

  it("hits zero-length fade knees at the edge inset", () => {
    const tl = timeline([track("v1", false, [clip("c1")])]);
    const y = trackY(tl, 0, heights) + 22;

    expect(fadeKneeHit(tl, 26, y, pixelsPerFrame, heights)).toEqual({
      clipId: "c1",
      trackIndex: 0,
      edge: "left",
      currentFrames: 0,
    });
    expect(fadeKneeHit(tl, 94, y, pixelsPerFrame, heights)).toEqual({
      clipId: "c1",
      trackIndex: 0,
      edge: "right",
      currentFrames: 0,
    });
  });

  it("hits nonzero left and right knees", () => {
    const tl = timeline([
      track("v1", false, [clip("c1", { fadeInFrames: 8, fadeOutFrames: 12 })]),
    ]);
    const y = trackY(tl, 0, heights) + 22;

    expect(fadeKneeHit(tl, 36, y, pixelsPerFrame, heights)?.edge).toBe("left");
    expect(fadeKneeHit(tl, 76, y, pixelsPerFrame, heights)?.edge).toBe("right");
  });

  it("hits a full-length fade-in at the same edge-specific center as the renderer", () => {
    const fullIn = timeline([
      track("v1", false, [clip("full-in", { fadeInFrames: 40, fadeOutFrames: 0 })]),
    ]);
    const y = trackY(fullIn, 0, heights) + 22;

    expect(fadeKneeHit(fullIn, 100, y, pixelsPerFrame, heights)?.edge).toBe("left");
  });

  it("ignores hidden tracks", () => {
    const tl = timeline([track("v1", true, [clip("hidden", { fadeInFrames: 8 })])]);
    const y = trackY(tl, 0, heights) + 22;

    expect(fadeKneeHit(tl, 36, y, pixelsPerFrame, heights)).toBeNull();
  });

  it("prefers the left knee when left and right hit boxes overlap", () => {
    const tl = timeline([
      track("v1", false, [clip("short", { durationFrames: 6, fadeInFrames: 3, fadeOutFrames: 3 })]),
    ]);
    const y = trackY(tl, 0, heights) + 22;

    expect(fadeKneeHit(tl, 26, y, pixelsPerFrame, heights)?.edge).toBe("left");
  });
});

describe("fade knee drag math", () => {
  it("increases fade-in when dragging right and clamps against fade-out", () => {
    const c = clip("c1", { durationFrames: 40, fadeInFrames: 8, fadeOutFrames: 10 });

    expect(fadeFramesForDrag(c, "left", 8, 20, 25)).toBe(13);
    expect(fadeFramesForDrag(c, "left", 8, 20, 100)).toBe(30);
  });

  it("increases fade-out when dragging left and clamps at zero", () => {
    const c = clip("c1", { durationFrames: 40, fadeInFrames: 8, fadeOutFrames: 10 });

    expect(fadeFramesForDrag(c, "right", 10, 60, 55)).toBe(15);
    expect(fadeFramesForDrag(c, "right", 10, 60, 80)).toBe(0);
  });
});
