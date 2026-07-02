import { describe, expect, it } from "vitest";
import type { Clip, Timeline, Track } from "./types";
import { planNudge } from "./timelineNudge";

function clip(id: string, startFrame: number, mediaType: Clip["mediaType"] = "video"): Clip {
  return {
    id,
    mediaRef: `${id}-m`,
    mediaType,
    sourceClipType: mediaType,
    startFrame,
    durationFrames: 50,
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

function timeline(tracks: Track[]): Timeline {
  return { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks };
}

function videoTrack(id: string, clips: Clip[]): Track {
  return { id, type: "video", muted: false, hidden: false, syncLocked: true, clips };
}

describe("planNudge", () => {
  const tl = timeline([videoTrack("v1", [clip("a", 100), clip("b", 300)])]);

  it("returns [] when nothing is selected", () => {
    expect(planNudge(tl, new Set(), 1)).toEqual([]);
  });

  it("returns [] for a zero delta", () => {
    expect(planNudge(tl, new Set(["a"]), 0)).toEqual([]);
  });

  it("nudges one clip forward preserving its track", () => {
    expect(planNudge(tl, new Set(["a"]), 5)).toEqual([{ clipId: "a", toTrack: 0, toFrame: 105 }]);
  });

  it("nudges one clip backward", () => {
    expect(planNudge(tl, new Set(["b"]), -1)).toEqual([{ clipId: "b", toTrack: 0, toFrame: 299 }]);
  });

  it("floors the whole group at frame 0 (never negative)", () => {
    const t = timeline([videoTrack("v1", [clip("a", 2), clip("b", 20)])]);
    // delta -5 would push 'a' to -3; group floors so 'a' lands at 0 and 'b'
    // shifts by the same clamped -2.
    expect(planNudge(t, new Set(["a", "b"]), -5)).toEqual([
      { clipId: "a", toTrack: 0, toFrame: 0 },
      { clipId: "b", toTrack: 0, toFrame: 18 },
    ]);
  });

  it("returns [] when a group already at 0 is nudged backward", () => {
    const t = timeline([videoTrack("v1", [clip("a", 0)])]);
    expect(planNudge(t, new Set(["a"]), -3)).toEqual([]);
  });

  it("moves clips across tracks together, each preserving its own track", () => {
    const t = timeline([
      videoTrack("v1", [clip("v", 100)]),
      { id: "a1", type: "audio", muted: false, hidden: false, syncLocked: true, clips: [clip("a", 100, "audio")] },
    ]);
    expect(planNudge(t, new Set(["v", "a"]), 1)).toEqual([
      { clipId: "v", toTrack: 0, toFrame: 101 },
      { clipId: "a", toTrack: 1, toFrame: 101 },
    ]);
  });
});
