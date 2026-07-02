import { describe, it, expect } from "vitest";
import { snapFrameToEdge } from "./snap";
import type { Timeline } from "./types";

/** Minimal timeline fixture — `snapFrameToEdge` only reads clip start/duration. */
function tl(clips: Array<[number, number]>): Timeline {
  return {
    fps: 30,
    width: 1920,
    height: 1080,
    version: 0,
    tracks: [
      {
        id: "v1",
        type: "video",
        clips: clips.map(([startFrame, durationFrames], i) => ({
          id: `c${i}`,
          startFrame,
          durationFrames,
        })),
      },
    ],
  } as unknown as Timeline;
}

describe("snapFrameToEdge", () => {
  const t = tl([
    [0, 30],
    [100, 50],
  ]); // edges: 0, 30, 100, 150

  it("snaps to the nearest clip edge within threshold", () => {
    expect(snapFrameToEdge(t, 32, 5)).toEqual({ frame: 30, snappedTo: 30 });
    expect(snapFrameToEdge(t, 98, 5)).toEqual({ frame: 100, snappedTo: 100 });
    expect(snapFrameToEdge(t, 148, 5)).toEqual({ frame: 150, snappedTo: 150 });
  });

  it("does not snap when no edge is within threshold", () => {
    expect(snapFrameToEdge(t, 60, 5)).toEqual({ frame: 60, snappedTo: null });
  });

  it("returns the frame unchanged on an empty timeline", () => {
    expect(snapFrameToEdge(tl([]), 42, 5)).toEqual({ frame: 42, snappedTo: null });
  });
});
