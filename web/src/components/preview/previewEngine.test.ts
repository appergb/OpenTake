import { describe, expect, it } from "vitest";
import * as previewEngine from "./previewEngine";
import { pausedSeekToleranceSec, previewElementKey, shouldSyncPausedMediaToFrame } from "./previewEngine";
import type { ActiveMedia } from "./timelinePlayback";
import type { Clip, ClipType, Timeline, Track } from "../../lib/types";

function clip(over: Partial<Clip> & { id: string; mediaType: ClipType }): Clip {
  return {
    id: over.id,
    mediaRef: over.mediaRef ?? "asset",
    mediaType: over.mediaType,
    sourceClipType: over.mediaType,
    startFrame: over.startFrame ?? 0,
    durationFrames: over.durationFrames ?? 100,
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

describe("shouldSyncPausedMediaToFrame", () => {
  it("does not seek on the play-to-pause edge", () => {
    expect(
      shouldSyncPausedMediaToFrame({
        isPlaying: false,
        isScrubbing: false,
        wasPlaying: true,
        wasScrubbing: false,
      }),
    ).toBe(false);
  });

  it("syncs DOM media only after transport is already settled paused", () => {
    expect(
      shouldSyncPausedMediaToFrame({
        isPlaying: false,
        isScrubbing: false,
        wasPlaying: false,
        wasScrubbing: false,
      }),
    ).toBe(true);
    expect(
      shouldSyncPausedMediaToFrame({
        isPlaying: true,
        isScrubbing: false,
        wasPlaying: false,
        wasScrubbing: false,
      }),
    ).toBe(false);
    expect(
      shouldSyncPausedMediaToFrame({
        isPlaying: false,
        isScrubbing: true,
        wasPlaying: false,
        wasScrubbing: false,
      }),
    ).toBe(false);
  });
});

describe("pausedSeekToleranceSec", () => {
  it("allows sub-frame pause differences without forcing a seek", () => {
    expect(pausedSeekToleranceSec(30)).toBeGreaterThan(0.5 / 30);
    expect(pausedSeekToleranceSec(0)).toBe(pausedSeekToleranceSec(30));
  });

  it("scales by speed because frame rounding expands source-time tolerance", () => {
    const fastTolerance = (pausedSeekToleranceSec as (fps: number, speed?: number) => number)(30, 2);
    expect(fastTolerance).toBeGreaterThan(1 / 30);
  });
});

describe("pausedPlayheadFrameFromFrozenVideo", () => {
  it("maps a frozen video element clock back to the timeline frame", () => {
    const fn = (
      previewEngine as {
        pausedPlayheadFrameFromFrozenVideo?: (
          media: ActiveMedia | null,
          currentTimeSec: number,
          fps: number,
        ) => number | null;
      }
    ).pausedPlayheadFrameFromFrozenVideo;
    const media = {
      trackIndex: 0,
      track: { id: "v1" },
      clip: {
        id: "clip-1",
        startFrame: 100,
        trimStartFrame: 30,
        speed: 2,
        mediaRef: "camera-a",
        mediaType: "video",
      },
    } as ActiveMedia;

    expect(typeof fn).toBe("function");
    expect(fn?.(media, 2, 30)).toBe(115);
  });
});

describe("activeVideoForPausedSnap", () => {
  it("uses the topmost active video even when an image layer is above it", () => {
    const fn = (
      previewEngine as {
        activeVideoForPausedSnap?: (timeline: Timeline, frame: number) => ActiveMedia | null;
      }
    ).activeVideoForPausedSnap;
    const tl = timeline([
      track({ id: "v1", type: "video", clips: [clip({ id: "video", mediaType: "video" })] }),
      track({ id: "v2", type: "video", clips: [clip({ id: "image", mediaType: "image" })] }),
    ]);

    expect(typeof fn).toBe("function");
    expect(fn?.(tl, 10)?.clip.id).toBe("video");
  });
});

describe("shouldSeekPlayingFollower", () => {
  it("forces a seek when a reused playback key switches clip identity", () => {
    const fn = (
      previewEngine as {
        shouldSeekPlayingFollower?: (args: {
          previousClipId: string | null;
          currentClipId: string;
          currentTimeSec: number;
          desiredTimeSec: number;
        }) => boolean;
      }
    ).shouldSeekPlayingFollower;

    expect(typeof fn).toBe("function");
    expect(
      fn?.({
        previousClipId: "left-half",
        currentClipId: "right-half",
        currentTimeSec: 1.05,
        desiredTimeSec: 1.1,
      }),
    ).toBe(true);
  });
});

describe("previewElementKey", () => {
  it("keeps linked video and audio elements separate even when clip ids match", () => {
    const base = {
      trackIndex: 0,
      track: { id: "v1" },
      clip: { id: "id-5", mediaRef: "asset", mediaType: "video" },
    } as ActiveMedia;
    const linkedAudio = {
      trackIndex: 1,
      track: { id: "a1" },
      clip: { id: "id-5", mediaRef: "asset", mediaType: "audio" },
    } as ActiveMedia;

    expect(previewElementKey(base)).not.toBe(previewElementKey(linkedAudio));
  });

  it("reuses the same element for adjacent split clips on the same track and source", () => {
    const left = {
      trackIndex: 0,
      track: { id: "v1" },
      clip: { id: "left-half", mediaRef: "interview", mediaType: "video" },
    } as ActiveMedia;
    const right = {
      trackIndex: 0,
      track: { id: "v1" },
      clip: { id: "right-half", mediaRef: "interview", mediaType: "video" },
    } as ActiveMedia;

    expect(previewElementKey(left)).toBe(previewElementKey(right));
  });

  it("keeps different source media separate on the same track", () => {
    const first = {
      trackIndex: 0,
      track: { id: "v1" },
      clip: { id: "a", mediaRef: "camera-a", mediaType: "video" },
    } as ActiveMedia;
    const second = {
      trackIndex: 0,
      track: { id: "v1" },
      clip: { id: "b", mediaRef: "camera-b", mediaType: "video" },
    } as ActiveMedia;

    expect(previewElementKey(first)).not.toBe(previewElementKey(second));
  });

  it("keeps the same source separate across tracks for picture-in-picture", () => {
    const base = {
      trackIndex: 0,
      track: { id: "v1" },
      clip: { id: "main", mediaRef: "interview", mediaType: "video" },
    } as ActiveMedia;
    const pip = {
      trackIndex: 1,
      track: { id: "v2" },
      clip: { id: "pip", mediaRef: "interview", mediaType: "video" },
    } as ActiveMedia;

    expect(previewElementKey(base)).not.toBe(previewElementKey(pip));
  });
});
