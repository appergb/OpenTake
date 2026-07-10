import { afterEach, describe, expect, it, vi } from "vitest";

const hookHarness = vi.hoisted(() => ({
  effects: [] as Array<() => void | (() => void)>,
  refs: [] as Array<{ current: unknown }>,
  refCursor: 0,
}));

// Substitute only React's lifecycle scheduler so the production hook, Zustand
// stores/actions, media registry, and rAF clock remain the code under test.
vi.mock("react", async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>();
  const useEffect = (effect: () => void | (() => void)) => {
    hookHarness.effects.push(effect);
  };
  const useRef = <T,>(initialValue: T) => {
    const index = hookHarness.refCursor++;
    const ref = hookHarness.refs[index] ?? { current: initialValue };
    hookHarness.refs[index] = ref;
    return ref as { current: T };
  };
  return {
    ...actual,
    useEffect,
    useRef,
  };
});

vi.mock("../../store/projectStore", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../store/projectStore")>();
  const store = actual.useProjectStore;
  const directStore = Object.assign(
    <T,>(selector: (state: ReturnType<typeof store.getState>) => T) =>
      selector(store.getState()),
    store,
  );
  return { ...actual, useProjectStore: directStore };
});

// Zustand's bound hook normally delegates to React.useSyncExternalStore. The
// direct selectors keep the same real stores while the lifecycle is controlled.
vi.mock("../../store/uiStore", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../store/uiStore")>();
  const store = actual.useEditorUiStore;
  const directStore = Object.assign(
    <T,>(selector: (state: ReturnType<typeof store.getState>) => T) =>
      selector(store.getState()),
    store,
  );
  return { ...actual, useEditorUiStore: directStore };
});

import * as previewEngine from "./previewEngine";
import { pausedSeekToleranceSec, previewElementKey, shouldSyncPausedMediaToFrame } from "./previewEngine";
import type { ActiveMedia } from "./timelinePlayback";
import type { Clip, ClipType, Timeline, Track } from "../../lib/types";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";

function runPlaybackHookEffects(): Array<() => void> {
  hookHarness.effects = [];
  hookHarness.refCursor = 0;
  previewEngine.useTimelinePlaybackEngine();
  return hookHarness.effects
    .map((effect) => effect())
    .filter((cleanup): cleanup is () => void => typeof cleanup === "function");
}

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
  it("registers exactly one playback frame listener before starting the session", async () => {
    const order: string[] = [];
    let resolveListener: (() => void) | null = null;
    const listenerReady = new Promise<void>((resolve) => {
      resolveListener = () => {
        order.push("listen");
        resolve();
      };
    });
    const result = previewEngine.startNativePlaybackAfterListener(listenerReady, async () => {
      order.push("start");
      return "started";
    });

    expect(order).toEqual([]);
    resolveListener?.();
    await expect(result).resolves.toBe("started");
    expect(order).toEqual(["listen", "start"]);
  });

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

describe("WebKit playback transport", () => {
  afterEach(() => {
    hookHarness.effects = [];
    hookHarness.refs = [];
    hookHarness.refCursor = 0;
    vi.unstubAllGlobals();
  });

  it("drives a registered media element through play and pause from the owning clock", async () => {
    const tl = timeline([
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "clip-1", mediaRef: "asset", mediaType: "video" })],
      }),
    ]);
    const active = {
      trackIndex: 0,
      track: tl.tracks[0],
      clip: tl.tracks[0].clips[0],
    } as ActiveMedia;
    const key = previewElementKey(active);
    let paused = true;
    const play = vi.fn(async () => {
      paused = false;
    });
    const pause = vi.fn(() => {
      paused = true;
    });
    const element = {
      currentTime: 0,
      muted: false,
      volume: 1,
      get paused() {
        return paused;
      },
      play,
      pause,
    } as unknown as HTMLMediaElement;
    const rafCallbacks = new Map<number, FrameRequestCallback>();
    let nextRafId = 1;
    vi.stubGlobal(
      "requestAnimationFrame",
      vi.fn((callback: FrameRequestCallback) => {
        const id = nextRafId++;
        rafCallbacks.set(id, callback);
        return id;
      }),
    );
    vi.stubGlobal(
      "cancelAnimationFrame",
      vi.fn((id: number) => {
        rafCallbacks.delete(id);
      }),
    );
    useProjectStore.setState({ timeline: tl, timelineVersion: 1 });
    useEditorUiStore.setState({
      currentFrame: 0,
      activeFrame: 0,
      isPlaying: false,
      isScrubbing: false,
    });
    previewEngine.previewElements.set(key, element);

    useEditorUiStore.getState().togglePlay();
    const playingCleanups = runPlaybackHookEffects();
    const firstFrame = rafCallbacks.values().next().value as FrameRequestCallback | undefined;
    expect(firstFrame).toBeTypeOf("function");
    firstFrame?.(16);
    await Promise.resolve();
    expect(useEditorUiStore.getState().isPlaying).toBe(true);
    expect(play).toHaveBeenCalled();

    useEditorUiStore.getState().togglePlay();
    playingCleanups.reverse().forEach((cleanup) => cleanup());
    runPlaybackHookEffects().reverse().forEach((cleanup) => cleanup());
    expect(useEditorUiStore.getState().isPlaying).toBe(false);
    expect(pause).toHaveBeenCalled();
    expect(previewEngine).not.toHaveProperty("setWebKitMediaPlayback");

    previewEngine.previewElements.remove(key);
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
