import { describe, expect, it } from "vitest";
import { dropTargetAt, trackY } from "./geometry";
import type { ClipType, Timeline, Track } from "./types";

function track(id: string, type: ClipType): Track {
  return {
    id,
    type,
    muted: false,
    hidden: false,
    syncLocked: true,
    clips: [],
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

describe("timeline geometry", () => {
  it("matches upstream dropTargetAt boundary behavior", () => {
    const tl = timeline([track("v1", "video"), track("v2", "video"), track("a1", "audio")]);
    const heights = {};

    expect(trackY(tl, 0, heights)).toBe(84);
    expect(trackY(tl, 1, heights)).toBe(134);
    expect(trackY(tl, 2, heights)).toBe(184);

    expect(dropTargetAt(tl, 50, heights)).toEqual({ kind: "newTrack", index: 0 });
    expect(dropTargetAt(tl, 100, heights)).toEqual({ kind: "existing", trackIndex: 0 });
    expect(dropTargetAt(tl, 130, heights)).toEqual({ kind: "newTrack", index: 1 });
    expect(dropTargetAt(tl, 134, heights)).toEqual({ kind: "newTrack", index: 1 });
    expect(dropTargetAt(tl, 200, heights)).toEqual({ kind: "existing", trackIndex: 2 });
    expect(dropTargetAt(tl, 250, heights)).toEqual({ kind: "newTrack", index: 3 });
  });

  it("targets a new first track on an empty timeline", () => {
    expect(dropTargetAt(timeline([]), 100, {})).toEqual({ kind: "newTrack", index: 0 });
  });
});
