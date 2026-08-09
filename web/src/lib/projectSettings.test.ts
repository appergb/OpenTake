import { describe, expect, it } from "vitest";
import type { MediaItem, Timeline } from "./types";
import { checkProjectSettings } from "./projectSettings";

function timeline(overrides: Partial<Timeline> = {}): Timeline {
  return {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: false,
    tracks: [],
    ...overrides,
  };
}

function media(overrides: Partial<MediaItem> = {}): MediaItem {
  return {
    id: "video",
    name: "video.mp4",
    type: "video",
    duration: 5,
    width: 3840,
    height: 2160,
    sourceFps: 23.976,
    hasAudio: true,
    favorite: false,
    ...overrides,
  };
}

describe("checkProjectSettings", () => {
  it("first_video_auto_configures_and_only_configured_empty_mismatch_prompts", () => {
    expect(checkProjectSettings(timeline(), [media({ type: "audio" })])).toEqual({
      kind: "proceed",
    });

    expect(checkProjectSettings(timeline(), [media()])).toEqual({
      kind: "apply",
      settings: { fps: 24, width: 3840, height: 2160 },
    });

    const occupied = timeline({
      settingsConfigured: true,
      tracks: [
        {
          id: "track",
          type: "video",
          muted: false,
          hidden: false,
          syncLocked: true,
          clips: [
            {
              id: "clip",
              mediaRef: "old",
              mediaType: "video",
              sourceClipType: "video",
              startFrame: 0,
              durationFrames: 30,
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
            },
          ],
        },
      ],
    });
    expect(checkProjectSettings(occupied, [media()])).toEqual({ kind: "proceed" });

    const configuredEmpty = timeline({ settingsConfigured: true });
    expect(
      checkProjectSettings(configuredEmpty, [
        media({ sourceFps: 30, width: 1920, height: 1080 }),
      ]),
    ).toEqual({ kind: "proceed" });
    expect(checkProjectSettings(configuredEmpty, [media()])).toEqual({
      kind: "prompt",
      settings: { fps: 24, width: 3840, height: 2160 },
    });

    expect(
      checkProjectSettings(timeline(), [
        media({ sourceFps: null, width: null, height: null }),
      ]),
    ).toEqual({
      kind: "apply",
      settings: { fps: 30, width: 1920, height: 1080 },
    });
  });
});
