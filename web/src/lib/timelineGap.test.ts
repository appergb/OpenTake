import { describe, expect, it } from "vitest";
import type { Clip, Timeline, Track } from "./types";
import { gapAtFrame } from "./timelineGap";

function clip(id: string, startFrame: number, durationFrames: number): Clip {
  return {
    id,
    mediaRef: `${id}-m`,
    mediaType: "video",
    sourceClipType: "video",
    startFrame,
    durationFrames,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform: { centerX: 0.5, centerY: 0.5, width: 1, height: 1, rotation: 0, flipHorizontal: false, flipVertical: false },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
  };
}

function timeline(clips: Clip[]): Timeline {
  const track: Track = { id: "t1", type: "video", muted: false, hidden: false, syncLocked: true, clips };
  return { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks: [track] };
}

describe("gapAtFrame", () => {
  // clips at [0,100) and [200,300): gap is [100,200).
  const tl = timeline([clip("a", 0, 100), clip("b", 200, 100)]);

  it("selects the gap between two clips", () => {
    expect(gapAtFrame(tl, 0, 150)).toEqual({ trackIndex: 0, startFrame: 100, endFrame: 200 });
  });

  it("uses the clip's end as the left edge at the gap boundary", () => {
    // frame 100 is the first empty frame (clip a ends exclusive at 100).
    expect(gapAtFrame(tl, 0, 100)).toEqual({ trackIndex: 0, startFrame: 100, endFrame: 200 });
  });

  it("returns null when the frame is inside a clip", () => {
    expect(gapAtFrame(tl, 0, 50)).toBeNull();
    expect(gapAtFrame(tl, 0, 250)).toBeNull();
  });

  it("returns null past the last clip (no right-bounding clip)", () => {
    expect(gapAtFrame(tl, 0, 400)).toBeNull();
  });

  it("selects a leading gap [0, firstStart) before the first clip", () => {
    const lead = timeline([clip("b", 200, 100)]);
    expect(gapAtFrame(lead, 0, 50)).toEqual({ trackIndex: 0, startFrame: 0, endFrame: 200 });
  });

  it("returns null for an out-of-range track", () => {
    expect(gapAtFrame(tl, 5, 150)).toBeNull();
  });

  it("returns null for an empty track (no right bound)", () => {
    expect(gapAtFrame(timeline([]), 0, 10)).toBeNull();
  });
});
