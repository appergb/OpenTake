import { describe, expect, it } from "vitest";
import type { MediaItem, Timeline, Track, Transform } from "./types";
import { buildInsertPlan, resolveInsertTrack } from "./timelineInsert";

const identity: Transform = {
  centerX: 0.5,
  centerY: 0.5,
  width: 1,
  height: 1,
  rotation: 0,
  flipHorizontal: false,
  flipVertical: false,
};
const fit = () => identity;

function track(id: string, type: Track["type"]): Track {
  return { id, type, muted: false, hidden: false, syncLocked: true, clips: [] };
}

function timeline(tracks: Track[]): Timeline {
  return { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks };
}

function media(overrides: Partial<MediaItem> = {}): MediaItem {
  return {
    id: "m1",
    name: "clip.mp4",
    type: "video",
    duration: 4,
    width: 1920,
    height: 1080,
    hasAudio: true,
    ...overrides,
  } as MediaItem;
}

describe("resolveInsertTrack", () => {
  const tl = timeline([track("v1", "video"), track("a1", "audio")]);

  it("prefers the given track when compatible", () => {
    expect(resolveInsertTrack(tl, "video", 0)).toBe(0);
  });

  it("falls back to the first compatible track when the preferred is incompatible", () => {
    // audio item preferring the video track → first audio track (index 1).
    expect(resolveInsertTrack(tl, "audio", 0)).toBe(1);
  });

  it("returns null when no compatible track exists", () => {
    expect(resolveInsertTrack(timeline([track("v1", "video")]), "audio", null)).toBeNull();
  });

  it("treats image/text/lottie as visual (video zone)", () => {
    expect(resolveInsertTrack(tl, "image", null)).toBe(0);
  });
});

describe("buildInsertPlan", () => {
  const tl = timeline([track("v1", "video"), track("a1", "audio")]);

  it("builds a plan placing at the (clamped) drop frame on the resolved track", () => {
    const plan = buildInsertPlan(tl, media(), 120, 0, fit, 5);
    expect(plan).not.toBeNull();
    expect(plan!.trackIndex).toBe(0);
    expect(plan!.atFrame).toBe(120);
    expect(plan!.entries).toHaveLength(1);
    const [entry] = plan!.entries;
    expect(entry.startFrame).toBe(120);
    // 4s * 30fps = 120 frames.
    expect(entry.durationFrames).toBe(120);
    // A video with audio requests a linked audio partner (upstream parity).
    expect(entry.addLinkedAudio).toBe(true);
  });

  it("uses the still-image default duration when the item has no length", () => {
    const plan = buildInsertPlan(tl, media({ type: "image", duration: 0, hasAudio: false }), 0, 0, fit, 5);
    // 5s default * 30fps.
    expect(plan!.entries[0].durationFrames).toBe(150);
    expect(plan!.entries[0].addLinkedAudio).toBe(false);
  });

  it("clamps a negative drop frame to 0", () => {
    const plan = buildInsertPlan(tl, media(), -30, 0, fit, 5);
    expect(plan!.atFrame).toBe(0);
    expect(plan!.entries[0].startFrame).toBe(0);
  });

  it("returns null when no compatible track exists", () => {
    const audioOnly = timeline([track("v1", "video")]);
    expect(buildInsertPlan(audioOnly, media({ type: "audio" }), 0, null, fit, 5)).toBeNull();
  });
});
