import { describe, expect, it } from "vitest";
import * as timelineContainer from "./TimelineContainer";
import type { Clip, ClipType, Timeline, Track } from "../../lib/types";

function clip(over: Partial<Clip> & { id: string; mediaType: ClipType }): Clip {
  return {
    id: over.id,
    mediaRef: over.mediaRef ?? "asset",
    mediaType: over.mediaType,
    sourceClipType: over.mediaType,
    startFrame: over.startFrame ?? 0,
    durationFrames: over.durationFrames ?? 30,
    trimStartFrame: over.trimStartFrame ?? 0,
    trimEndFrame: over.trimEndFrame ?? 0,
    speed: over.speed ?? 1,
    volume: over.volume ?? 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "smooth",
    fadeOutInterpolation: "smooth",
    opacity: over.opacity ?? 1,
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

function track(id: string, clips: Clip[]): Track {
  return {
    id,
    type: "video",
    muted: false,
    hidden: false,
    syncLocked: true,
    clips,
  };
}

function timeline(tracks: Track[]): Timeline {
  return { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks };
}

describe("collectMoveSnapTargets", () => {
  it("does not include the playhead when moving clips", () => {
    const fn = (
      timelineContainer as {
        collectMoveSnapTargets?: (
          timeline: Timeline,
          excluded: Set<string>,
          activeFrame: number,
        ) => Array<{ frame: number; kind: string }>;
      }
    ).collectMoveSnapTargets;
    const tl = timeline([
      track("v1", [
        clip({ id: "dragged", mediaType: "video", startFrame: 10, durationFrames: 20 }),
        clip({ id: "other", mediaType: "video", startFrame: 80, durationFrames: 10 }),
      ]),
    ]);

    expect(typeof fn).toBe("function");
    expect(fn?.(tl, new Set(["dragged"]), 42)).toEqual([
      { frame: 80, kind: "clipEdge" },
      { frame: 90, kind: "clipEdge" },
    ]);
  });
});
