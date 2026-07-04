import { describe, expect, it } from "vitest";
import { trackDisplayLabel } from "./zones";
import type { ClipType, Timeline, Track } from "./types";

function track(type: ClipType, id: string): Track {
  return { id, type, muted: false, hidden: false, syncLocked: true, clips: [] };
}
function tl(types: ClipType[]): Timeline {
  return {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks: types.map((t, i) => track(t, `t${i}`)),
  };
}

describe("trackDisplayLabel", () => {
  it("numbers the topmost video highest, bottom video V1 (above audio)", () => {
    // V V V A  → top video is V3, bottom video (above audio) is V1.
    const t = tl(["video", "video", "video", "audio"]);
    expect(trackDisplayLabel(t, 0)).toBe("V3");
    expect(trackDisplayLabel(t, 1)).toBe("V2");
    expect(trackDisplayLabel(t, 2)).toBe("V1");
    expect(trackDisplayLabel(t, 3)).toBe("A1");
  });

  it("numbers audio top-down", () => {
    const t = tl(["video", "audio", "audio"]);
    expect(trackDisplayLabel(t, 0)).toBe("V1");
    expect(trackDisplayLabel(t, 1)).toBe("A1");
    expect(trackDisplayLabel(t, 2)).toBe("A2");
  });

  it("works with no audio track (visualEnd = track count)", () => {
    const t = tl(["video", "video"]);
    expect(trackDisplayLabel(t, 0)).toBe("V2");
    expect(trackDisplayLabel(t, 1)).toBe("V1");
  });

  it("counts only same-kind tracks in the visual zone", () => {
    // V I V A → for the two video tracks, image between is skipped.
    const t = tl(["video", "image", "video", "audio"]);
    expect(trackDisplayLabel(t, 0)).toBe("V2");
    expect(trackDisplayLabel(t, 1)).toBe("I1");
    expect(trackDisplayLabel(t, 2)).toBe("V1");
  });
});
