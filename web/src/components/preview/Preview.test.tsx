import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Clip, ClipType, PlaybackFrameEvent, Timeline, Track } from "../../lib/types";

const store = vi.hoisted(() => ({
  projectEpoch: 3,
  timelineVersion: 7,
  timeline: { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks: [] } as Timeline,
  ui: {
    activeFrame: 42,
    currentFrame: 42,
    isPlaying: false,
    isScrubbing: false,
    previewMediaId: null as string | null,
    selectedClipIds: new Set<string>(),
    canvasZoom: 1,
    canvasOffset: { width: 0, height: 0 },
    setCanvasZoom: vi.fn(),
    setCanvasOffset: vi.fn(),
    previewQualityShortEdge: null as number | null,
    setPreviewQualityShortEdge: vi.fn(),
    setCurrentFrame: vi.fn(),
    setScrubbing: vi.fn(),
    togglePlay: vi.fn(),
    requestMediaPreviewToggle: vi.fn(),
    mediaPreviewToggleRequest: 0,
    rustEngineFailed: false,
    setRustEngineFailed: vi.fn(),
    webkitPlaybackFailedRevision: null as string | null,
    setWebkitPlaybackFailedRevision: vi.fn(),
  },
  media: {
    items: [] as Array<{
      id: string;
      name: string;
      type: ClipType;
      duration: number;
      hasAudio: boolean;
      path: string;
      missing?: boolean;
    }>,
  },
  nativeFrame: null as PlaybackFrameEvent | null,
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
  refreshMedia: vi.fn(),
}));

vi.mock("../../lib/asset", () => ({
  assetUrl: (path: string | null | undefined) => (path ? `asset://${path}` : null),
}));

vi.mock("../../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/api")>()),
  isTauri: true,
}));

vi.mock("./nativePlaybackSession", () => ({
  useNativePlaybackPublication: () => store.nativeFrame,
  nativePlaybackController: { stop: vi.fn() },
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
      selectedClipIds: new Set<string>(),
      rustEngineFailed: false,
      webkitPlaybackFailedRevision: null,
    };
    store.media.items = [
      { id: "base", name: "base", type: "video", duration: 10, hasAudio: true, path: "/base.mov" },
      { id: "pip", name: "pip", type: "video", duration: 10, hasAudio: true, path: "/pip.mov" },
    ];
    store.nativeFrame = null;
  });

  it("keeps paused timeline on DOM video without a composite image overlay", () => {
    store.timeline = timeline([
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "base-clip", mediaRef: "base", mediaType: "video" })],
      }),
    ]);

    const html = renderToStaticMarkup(<Preview />);

    expect(html).toContain("<video");
    expect(html.match(/data-rust-frame-slot=/g)).toHaveLength(2);
    expect(html).not.toContain("data:image/png");
  });

  it("keeps timeline DOM video visible while playing", () => {
    store.timeline = timeline([
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "base-clip", mediaRef: "base", mediaType: "video" })],
      }),
    ]);

    store.ui.isPlaying = true;
    const playingHtml = renderToStaticMarkup(<Preview />);

    expect(playingHtml).toContain("<video");
    expect(playingHtml).toContain('data-playback-surface="webkit"');
  });

  it("uses the native surface for a multi-video-track timeline", () => {
    store.timeline = timeline([
      track({
        id: "v2",
        type: "video",
        clips: [clip({ id: "upper", mediaRef: "pip", mediaType: "video" })],
      }),
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "lower-main10", mediaRef: "base", mediaType: "video" })],
      }),
    ]);

    const html = renderToStaticMarkup(<Preview />);

    expect(html).not.toContain('data-playback-surface="webkit"');
    expect(html).not.toContain("<video");
    expect(html.match(/data-rust-frame-slot=/g)).toHaveLength(2);
  });

  it("mounts WebKit again when native startup fails after a WebKit decode failure", () => {
    store.timeline = timeline([
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "main10", mediaRef: "base", mediaType: "video" })],
      }),
    ]);
    store.ui.webkitPlaybackFailedRevision = "3:7";
    store.ui.rustEngineFailed = true;

    const html = renderToStaticMarkup(<Preview />);

    expect(html).toContain('data-playback-surface="webkit"');
    expect(html).toContain("<video");
  });

  it("switches the failed WebKit revision to native before native startup fails", () => {
    store.timeline = timeline([
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "main10", mediaRef: "base", mediaType: "video" })],
      }),
    ]);
    store.ui.webkitPlaybackFailedRevision = "3:7";
    store.ui.rustEngineFailed = false;

    const html = renderToStaticMarkup(<Preview />);

    expect(html).not.toContain('data-playback-surface="webkit"');
    expect(html).not.toContain("<video");
    expect(html.match(/data-rust-frame-slot=/g)).toHaveLength(2);
  });

  it("renders a user visible unsupported surface instead of incomplete DOM media", () => {
    store.timeline = timeline([
      track({
        id: "v1",
        type: "text",
        clips: [clip({ id: "text-clip", mediaRef: "base", mediaType: "text", reversed: true })],
      }),
    ]);

    const html = renderToStaticMarkup(<Preview />);

    expect(html).toContain('data-testid="unsupported-playback-surface"');
    expect(html).toContain("当前时间线无法完整预览");
    expect(html).not.toContain("<video");
  });

  it("disables play and capture for unsupported playback", () => {
    store.timeline = timeline([
      track({
        id: "v1",
        type: "text",
        clips: [clip({ id: "text-clip", mediaRef: "base", mediaType: "text", reversed: true })],
      }),
    ]);

    const html = renderToStaticMarkup(<Preview />);

    expect(html).toMatch(/aria-label="播放\/暂停 \(空格\)"[^>]*disabled/);
    expect(html).toMatch(/aria-label="截取当前帧到素材库"[^>]*disabled/);
  });

  it("keeps play available so a compositor-only timeline can retry native startup", () => {
    store.timeline = timeline([
      track({
        id: "v1",
        type: "text",
        clips: [clip({ id: "text-clip", mediaRef: "base", mediaType: "text" })],
      }),
    ]);
    store.ui.rustEngineFailed = true;

    const html = renderToStaticMarkup(<Preview />);

    expect(html).toContain('data-testid="unsupported-playback-surface"');
    expect(html).not.toMatch(/aria-label="播放\/暂停 \(空格\)"[^>]*disabled/);
  });

  it("ignores native publications on a WebKit playback route", () => {
    store.timeline = timeline([
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "base-clip", mediaRef: "base", mediaType: "video" })],
      }),
    ]);
    store.ui.isPlaying = true;
    store.nativeFrame = {
      projectEpoch: 3,
      timelineVersion: 4,
      sessionId: "session-5",
      frame: 42,
      sequence: 8,
      terminal: false,
    };

    const html = renderToStaticMarkup(<Preview />);

    expect(html).toContain("<video");
    expect(html).toContain('data-playback-surface="webkit"');
    expect(html).not.toContain("sessionId=session-5");
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

  it("does not hand an offline source to the WebKit asset protocol", () => {
    const tl = timeline([
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "offline-clip", mediaRef: "base", mediaType: "video" })],
      }),
    ]);
    store.media.items = [{
      id: "base",
      name: "Offline",
      type: "video",
      duration: 1,
      hasAudio: false,
      path: "/cloud-placeholder.mov",
      missing: true,
    }];

    const html = renderToStaticMarkup(<TimelinePlayback timeline={tl} fps={30} />);

    expect(html).not.toContain("cloud-placeholder.mov");
    expect(html).not.toContain("<video");
  });

  it("paints an overlay on upstream visual track 0 after the base layer", () => {
    const tl = timeline([
      track({
        id: "v0",
        type: "video",
        clips: [clip({ id: "overlay-clip", mediaRef: "pip", mediaType: "video" })],
      }),
      track({
        id: "v1",
        type: "video",
        clips: [clip({ id: "base-clip", mediaRef: "base", mediaType: "video" })],
      }),
    ]);

    const html = renderToStaticMarkup(<TimelinePlayback timeline={tl} fps={30} />);
    const baseIndex = html.indexOf("asset:///base.mov");
    const overlayIndex = html.indexOf("asset:///pip.mov");

    expect(html.match(/<video/g)?.length).toBe(2);
    expect(baseIndex).toBeGreaterThanOrEqual(0);
    expect(overlayIndex).toBeGreaterThan(baseIndex);
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

  // Note: only the NEGATIVE guard cases are provable here. `renderToStaticMarkup`
  // never runs `useEffect`, so the ResizeObserver that measures `stageSize` (and
  // therefore `fittedCanvas`, Preview.tsx:156) never fires — `fittedCanvas` is
  // always null in this test file, and TransformOverlay correctly (and
  // deliberately) renders nothing for a degenerate canvas. The POSITIVE
  // rendering path — given a real canvasPx — is covered by TransformOverlay.test.tsx,
  // which renders the component directly instead of through Preview's sizing.
  describe("Transform overlay mount guard (T3-10)", () => {
    beforeEach(() => {
      store.timeline = timeline([
        track({
          id: "v1",
          type: "video",
          clips: [
            clip({ id: "base-clip", mediaRef: "base", mediaType: "video" }),
            clip({ id: "text-clip", mediaRef: "base", mediaType: "text", startFrame: 10 }),
          ],
        }),
        track({
          id: "a1",
          type: "audio",
          clips: [clip({ id: "audio-clip", mediaRef: "base", mediaType: "audio", startFrame: 20 })],
        }),
      ]);
    });

    it("stays hidden when no clip is selected", () => {
      store.ui.selectedClipIds = new Set();

      const html = renderToStaticMarkup(<Preview />);

      expect(html).not.toContain('data-testid="transform-overlay"');
    });

    it("stays hidden when more than one clip is selected (marquee)", () => {
      store.ui.selectedClipIds = new Set(["base-clip", "text-clip"]);

      const html = renderToStaticMarkup(<Preview />);

      expect(html).not.toContain('data-testid="transform-overlay"');
    });

    it("stays hidden when the single selected clip is on an audio track", () => {
      store.ui.selectedClipIds = new Set(["audio-clip"]);

      const html = renderToStaticMarkup(<Preview />);

      expect(html).not.toContain('data-testid="transform-overlay"');
    });

    it("stays hidden while viewing a media-library preview, even with a visual clip selected", () => {
      store.ui.selectedClipIds = new Set(["base-clip"]);
      store.ui.previewMediaId = "still";
      store.media.items = [
        { id: "still", name: "still", type: "image", duration: 1, hasAudio: false, path: "/still.png" },
      ];

      const html = renderToStaticMarkup(<Preview />);

      expect(html).not.toContain('data-testid="transform-overlay"');
    });
  });
});
