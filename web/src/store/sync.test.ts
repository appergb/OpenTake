import { afterEach, describe, expect, it, vi } from "vitest";
import type { Timeline } from "../lib/types";

const srv = vi.hoisted(() => {
  const timeline: Timeline = {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks: [],
  };
  return {
    timeline,
    order: [] as string[],
    onProjectOpened: null as null | ((path: string, projectEpoch: number, version: number) => Promise<void> | void),
    invalidate: vi.fn(async () => {
      srv.order.push("invalidate");
    }),
  };
});

vi.mock("../lib/api", () => ({
  getTimeline: async () => {
    srv.order.push("refresh");
    return { timeline: srv.timeline, projectEpoch: 1, version: 0 };
  },
  canUndo: async () => false,
  canRedo: async () => false,
  onTimelineChanged: async () => () => {},
  onProjectOpened: async (
    handler: (path: string, projectEpoch: number, version: number) => Promise<void> | void,
  ) => {
    srv.onProjectOpened = handler;
    return () => {};
  },
}));

vi.mock("../components/preview/nativePlaybackSession", () => ({
  stopNativePlaybackForProjectBoundary: srv.invalidate,
}));

import { startSync, stopSync } from "./sync";
import { useProjectStore } from "./projectStore";
import { useEditorUiStore } from "./uiStore";

afterEach(() => {
  stopSync();
  srv.order.length = 0;
  srv.invalidate.mockClear();
  srv.onProjectOpened = null;
});

describe("project event sync", () => {
  it("invalidates project scoped playback on externally initiated project_opened", async () => {
    useEditorUiStore.setState({
      isPlaying: true,
      currentFrame: 77,
      activeFrame: 77,
      selectedClipIds: new Set(["old-clip"]),
      layoutPreset: "media",
    });
    await startSync();
    srv.order.length = 0;

    await srv.onProjectOpened?.("/tmp/external.opentake", 7, 0);

    expect(srv.order.slice(0, 2)).toEqual(["invalidate", "refresh"]);
    expect(useProjectStore.getState().projectEpoch).toBe(1);
    expect(useProjectStore.getState().projectPath).toBe("/tmp/external.opentake");
    const ui = useEditorUiStore.getState();
    expect(ui.isPlaying).toBe(false);
    expect(ui.currentFrame).toBe(0);
    expect(ui.activeFrame).toBe(0);
    expect(ui.selectedClipIds.size).toBe(0);
    expect(ui.layoutPreset).toBe("media");
  });
});
