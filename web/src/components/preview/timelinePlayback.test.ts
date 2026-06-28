import { describe, expect, it } from "vitest";
import type { Clip, ClipType, Timeline, Track } from "../../lib/types";
import {
  activeAudioClips,
  activeVisualClip,
  activeVisualClips,
  advancePlayhead,
  clipCoversFrame,
  clipOpacity,
  clipVolume,
  frameForSourceTime,
  isExternalSeekWhilePlaying,
  playbackFrameFromActiveFrame,
  sourceTimeSec,
  visualAudioIsDuplicated,
} from "./timelinePlayback";

describe("isExternalSeekWhilePlaying", () => {
  it("returns false before the engine has emitted a frame", () => {
    expect(isExternalSeekWhilePlaying({ activeFrame: 100, lastEngineFrame: null })).toBe(false);
  });

  it("returns false for the engine's own per-frame advance (within epsilon)", () => {
    expect(isExternalSeekWhilePlaying({ activeFrame: 31, lastEngineFrame: 30 })).toBe(false);
    expect(isExternalSeekWhilePlaying({ activeFrame: 30, lastEngineFrame: 30 })).toBe(false);
  });

  it("detects an external jump beyond epsilon (keyboard step / transport click)", () => {
    expect(isExternalSeekWhilePlaying({ activeFrame: 300, lastEngineFrame: 30 })).toBe(true);
    expect(isExternalSeekWhilePlaying({ activeFrame: 0, lastEngineFrame: 120 })).toBe(true);
  });

  it("floors a fractional activeFrame and honors a custom epsilon", () => {
    // 33.9 floors to 33; |33 - 30| = 3 > default eps 2 → true.
    expect(isExternalSeekWhilePlaying({ activeFrame: 33.9, lastEngineFrame: 30 })).toBe(true);
    // Same delta, eps 5 → false.
    expect(
      isExternalSeekWhilePlaying({ activeFrame: 33.9, lastEngineFrame: 30, epsilonFrames: 5 }),
    ).toBe(false);
  });
});

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

describe("clipCoversFrame", () => {
  it("is half-open [start, start+duration)", () => {
    const c = clip({ id: "c", mediaType: "video", startFrame: 10, durationFrames: 5 });
    expect(clipCoversFrame(c, 9)).toBe(false);
    expect(clipCoversFrame(c, 10)).toBe(true);
    expect(clipCoversFrame(c, 14)).toBe(true);
    expect(clipCoversFrame(c, 15)).toBe(false);
  });
});

describe("playbackFrameFromActiveFrame", () => {
  it("floors fractional RAF playhead updates to the current rendered frame", () => {
    expect(playbackFrameFromActiveFrame(12.1)).toBe(12);
    expect(playbackFrameFromActiveFrame(12.49)).toBe(12);
    expect(playbackFrameFromActiveFrame(12.99)).toBe(12);
  });
});

describe("activeVisualClip", () => {
  it("returns the video under the playhead", () => {
    const tl = timeline([
      track({ id: "v1", type: "video", clips: [clip({ id: "a", mediaType: "video", startFrame: 0, durationFrames: 50 })] }),
    ]);
    expect(activeVisualClip(tl, 10)?.clip.id).toBe("a");
    expect(activeVisualClip(tl, 60)).toBeNull();
  });

  it("prefers the lowest visual track index because upstream track 0 is topmost", () => {
    const tl = timeline([
      track({ id: "v1", type: "video", clips: [clip({ id: "top", mediaType: "video" })] }),
      track({ id: "v2", type: "video", clips: [clip({ id: "bottom", mediaType: "image" })] }),
    ]);
    expect(activeVisualClip(tl, 10)?.clip.id).toBe("top");
  });

  it("returns every visible visual clip in bottom-to-top track order", () => {
    const tl = timeline([
      track({ id: "v1", type: "video", clips: [clip({ id: "top", mediaType: "video" })] }),
      track({ id: "v2", type: "video", clips: [clip({ id: "bottom", mediaType: "image" })] }),
    ]);

    expect(activeVisualClips(tl, 10).map((v) => v.clip.id)).toEqual(["bottom", "top"]);
  });

  it("skips hidden tracks and audio/text", () => {
    const tl = timeline([
      track({ id: "v1", type: "video", hidden: true, clips: [clip({ id: "hid", mediaType: "video" })] }),
      track({ id: "t1", type: "text", clips: [clip({ id: "txt", mediaType: "text" })] }),
    ]);
    expect(activeVisualClip(tl, 10)).toBeNull();
  });
});

describe("activeAudioClips", () => {
  it("collects non-muted audio-track clips only", () => {
    const tl = timeline([
      track({ id: "v1", type: "video", clips: [clip({ id: "vid", mediaType: "video" })] }),
      track({ id: "a1", type: "audio", clips: [clip({ id: "music", mediaType: "audio" })] }),
      track({ id: "a2", type: "audio", muted: true, clips: [clip({ id: "muted", mediaType: "audio" })] }),
    ]);
    const ids = activeAudioClips(tl, 10).map((a) => a.clip.id);
    expect(ids).toEqual(["music"]);
  });
});

describe("sourceTimeSec / frameForSourceTime", () => {
  it("maps timeline frame -> source seconds with trim and start", () => {
    const c = clip({ id: "c", mediaType: "video", startFrame: 30, trimStartFrame: 60, durationFrames: 90 });
    // frame 30 -> trim 60 / 30fps = 2s; frame 60 -> (60 + 30)/30 = 3s.
    expect(sourceTimeSec(c, 30, 30)).toBeCloseTo(2);
    expect(sourceTimeSec(c, 60, 30)).toBeCloseTo(3);
  });

  it("respects speed", () => {
    const c = clip({ id: "c", mediaType: "video", startFrame: 0, trimStartFrame: 0, speed: 2 });
    // frame 30 at 2x -> source frame 60 -> 2s.
    expect(sourceTimeSec(c, 30, 30)).toBeCloseTo(2);
  });

  it("round-trips frame <-> source time", () => {
    const c = clip({ id: "c", mediaType: "video", startFrame: 30, trimStartFrame: 45, speed: 1.5 });
    const ts = sourceTimeSec(c, 90, 30);
    expect(frameForSourceTime(c, ts, 30)).toBeCloseTo(90);
  });

  it("clamps source time at 0", () => {
    const c = clip({ id: "c", mediaType: "video", startFrame: 100, trimStartFrame: 0 });
    expect(sourceTimeSec(c, 0, 30)).toBe(0);
  });
});

describe("clipVolume / clipOpacity", () => {
  it("zeroes volume on a muted track", () => {
    const t = track({ id: "a", type: "audio", muted: true, clips: [] });
    const c = clip({ id: "c", mediaType: "audio", volume: 1 });
    expect(clipVolume(t, c)).toBe(0);
  });

  it("clamps volume and opacity to 0..1", () => {
    const t = track({ id: "a", type: "audio", clips: [] });
    expect(clipVolume(t, clip({ id: "c", mediaType: "audio", volume: 2 }))).toBe(1);
    expect(clipOpacity(clip({ id: "c", mediaType: "video", opacity: -1 }))).toBe(0);
  });
});

describe("visualAudioIsDuplicated", () => {
  it("flags a video whose source is also on an audio track", () => {
    const visual = { clip: clip({ id: "v", mediaType: "video", mediaRef: "m1" }), track: track({ id: "v1", type: "video", clips: [] }), trackIndex: 0 };
    const audios = [{ clip: clip({ id: "a", mediaType: "audio", mediaRef: "m1" }), track: track({ id: "a1", type: "audio", clips: [] }), trackIndex: 1 }];
    expect(visualAudioIsDuplicated(visual, audios)).toBe(true);
  });

  it("does not flag separate sources", () => {
    const visual = { clip: clip({ id: "v", mediaType: "video", mediaRef: "m1" }), track: track({ id: "v1", type: "video", clips: [] }), trackIndex: 0 };
    const audios = [{ clip: clip({ id: "a", mediaType: "audio", mediaRef: "music" }), track: track({ id: "a1", type: "audio", clips: [] }), trackIndex: 1 }];
    expect(visualAudioIsDuplicated(visual, audios)).toBe(false);
  });
});

describe("advancePlayhead", () => {
  it("advances by dt*fps", () => {
    // 0.5s elapsed at 30fps -> +15 frames.
    expect(advancePlayhead({ currentFrame: 100, dtSec: 0.5, fps: 30 })).toBeCloseTo(115);
  });

  it("does not read a DOM media clock as timeline authority", () => {
    expect(advancePlayhead({ currentFrame: 100, dtSec: 0.25, fps: 30 })).toBeCloseTo(107.5);
  });

  it("falls back to 30fps when fps is non-positive", () => {
    expect(advancePlayhead({ currentFrame: 0, dtSec: 1, fps: 0 })).toBeCloseTo(30);
  });
});
