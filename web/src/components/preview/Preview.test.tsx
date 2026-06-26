import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Clip, ClipType, Timeline, Track } from "../../lib/types";

const store = vi.hoisted(() => ({
  timeline: { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks: [] } as Timeline,
  ui: {
    activeFrame: 42,
    currentFrame: 42,
    isPlaying: false,
    isScrubbing: false,
    previewMediaId: null as string | null,
    setCurrentFrame: vi.fn(),
    setScrubbing: vi.fn(),
    togglePlay: vi.fn(),
    requestMediaPreviewToggle: vi.fn(),
    mediaPreviewToggleRequest: 0,
  },
  media: {
    items: [] as Array<{
      id: string;
      name: string;
      type: ClipType;
      duration: number;
      hasAudio: boolean;
      path: string;
    }>,
  },
  timelineFrame: {
    url: "data:image/png;base64,current-paused-composite",
    frame: 42 as number | null,
  },
}));

vi.mock("../../store/projectStore", () => ({
  useProjectStore: Object.assign((selector: (state: { timeline: Timeline }) => unknown) => selector(store), {
    getState: () => store,
  }),
}));

vi.mock("../../store/uiStore", () => ({
  useEditorUiStore: Object.assign((selector: (state: typeof store.ui) => unknown) => selector(store.ui), {
    getState: () => store.ui,
  }),
}));

vi.mock("../../store/mediaStore", () => ({
  useMediaStore: Object.assign((selector: (state: typeof store.media) => unknown) => selector(store.media), {
    getState: () => store.media,
  }),
}));

vi.mock("../../lib/asset", () => ({
  assetUrl: (path: string | null | undefined) => (path ? `asset://${path}` : null),
}));

vi.mock("./useTimelineFrame", () => ({
  useTimelineFrame: () => store.timelineFrame,
}));

import { Preview } from "./Preview";
import { TimelinePlayback } from "./TimelinePlaybackLayer";

function clip(over: Partial<Clip> & { id: string; mediaRef: string; mediaType: ClipType }): Clip {
  return {
    id: over.id,
    mediaRef: over.mediaRef,
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

describe("Preview timeline rendering", () => {
  beforeEach(() => {
    store.ui = {
      ...store.ui,
      activeFrame: 42,
      currentFrame: 42,
      isPlaying: false,
      isScrubbing: false,
      previewMediaId: null,
    };
    store.media.items = [
      { id: "base", name: "base", type: "video", duration: 10, hasAudio: true, path: "/base.mov" },
      { id: "pip", name: "pip", type: "video", duration: 10, hasAudio: true, path: "/pip.mov" },
    ];
  });

  it("overlays the current paused Rust composite without unmounting DOM video", () => {
    store.timeline = timeline([
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "base-clip", mediaRef: "base", mediaType: "video" })],
      }),
    ]);

    const html = renderToStaticMarkup(<Preview />);

    expect(html).toContain("<video");
    expect(html).toContain("current-paused-composite");
  });

  it("does not show a stale Rust composite for a different frame", () => {
    store.timelineFrame = {
      url: "data:image/png;base64,stale-paused-composite",
      frame: 41,
    };
    store.timeline = timeline([
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "base-clip", mediaRef: "base", mediaType: "video" })],
      }),
    ]);

    const html = renderToStaticMarkup(<Preview />);

    expect(html).toContain("<video");
    expect(html).not.toContain("stale-paused-composite");
  });

  it("renders every visible visual layer on the shared timeline canvas", () => {
    const tl = timeline([
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "base-clip", mediaRef: "base", mediaType: "video" })],
      }),
      track({
        id: "v2",
        type: "video",
        clips: [clip({ id: "pip-clip", mediaRef: "pip", mediaType: "video", opacity: 0.7 })],
      }),
    ]);

    const html = renderToStaticMarkup(<TimelinePlayback timeline={tl} fps={30} />);

    expect(html.match(/<video/g)?.length).toBe(2);
    expect(html).toContain("asset:///base.mov");
    expect(html).toContain("asset:///pip.mov");
  });

  it("keeps read-only source image previews from intercepting transport clicks", () => {
    store.ui.previewMediaId = "still";
    store.media.items = [
      { id: "still", name: "still", type: "image", duration: 1, hasAudio: false, path: "/still.png" },
    ];

    const html = renderToStaticMarkup(<Preview />);

    expect(html).toContain("<img");
    expect(html).toContain("pointer-events:none");
  });
});
