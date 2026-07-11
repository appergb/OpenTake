import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaList, Timeline } from "../lib/types";

const srv = vi.hoisted(() => {
  const timeline: Timeline = {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks: [],
  };
  const media: MediaList = {
    items: [
      {
        id: "m1",
        name: "clip",
        type: "video",
        duration: 10,
        hasAudio: true,
        path: "/tmp/clip.mov",
      },
    ],
    folders: [],
  };
  const order: string[] = [];
  return {
    timeline,
    media,
    order,
    stopBoundary: vi.fn(async () => order.push("stop")),
    projectOpen: vi.fn(async () => {
      order.push("open");
      return { timeline, projectEpoch: 4, version: 7 };
    }),
    projectNew: vi.fn(async () => {
      order.push("new");
      return { timeline, projectEpoch: 5, version: 0 };
    }),
  };
});

vi.mock("../lib/api", () => ({
  projectOpen: srv.projectOpen,
  projectNew: srv.projectNew,
  projectSave: async (path: string | null) => path ?? "",
  getDefaultProjectDir: async () => "",
  getTimeline: async () => ({ timeline: srv.timeline, projectEpoch: 5, version: 0 }),
  canUndo: async () => false,
  canRedo: async () => false,
  getMedia: async () => srv.media,
}));

vi.mock("../components/preview/nativePlaybackSession", () => ({
  stopNativePlaybackForProjectBoundary: srv.stopBoundary,
}));

vi.mock("../lib/dialog", () => ({
  saveDialog: async () => async () => "/tmp/fresh.opentake",
  openDialog: async () => undefined,
}));

import { newProjectAndEnter, openProjectPath } from "./projectActions";
import { useEditorUiStore } from "./uiStore";
import { useMediaStore } from "./mediaStore";
import { useProjectStore } from "./projectStore";

describe("openProjectPath", () => {
  beforeEach(() => {
    srv.order.length = 0;
    srv.stopBoundary.mockClear();
    srv.projectOpen.mockClear();
    srv.projectNew.mockClear();
    useMediaStore.getState().setItems([]);
    useProjectStore.setState({ projectPath: null, timelineVersion: 0 });
    useEditorUiStore.setState({ view: "home" });
  });

  it("refreshes the media mirror after opening a project", async () => {
    await openProjectPath("/tmp/demo.opentake");

    expect(useProjectStore.getState().projectPath).toBe("/tmp/demo.opentake");
    expect(useMediaStore.getState().items.map((item) => item.id)).toEqual(["m1"]);
    expect(useEditorUiStore.getState().view).toBe("editor");
  });

  it("resets project-scoped UI runtime only after a successful project open", async () => {
    useEditorUiStore.setState({
      isPlaying: true,
      currentFrame: 91,
      activeFrame: 91,
      selectedClipIds: new Set(["old-clip"]),
      previewMediaId: "old-media",
      layoutPreset: "vertical",
      agentPanelVisible: false,
    });

    await openProjectPath("/tmp/reset.opentake");

    const ui = useEditorUiStore.getState();
    expect(ui.isPlaying).toBe(false);
    expect(ui.currentFrame).toBe(0);
    expect(ui.activeFrame).toBe(0);
    expect(ui.selectedClipIds.size).toBe(0);
    expect(ui.previewMediaId).toBeNull();
    expect(ui.layoutPreset).toBe("vertical");
    expect(ui.agentPanelVisible).toBe(false);
  });

  it("stops native playback before opening a project whose version collides", async () => {
    useProjectStore.setState({ projectEpoch: 3, timelineVersion: 7 });

    await openProjectPath("/tmp/collision.opentake");

    expect(srv.order.slice(0, 2)).toEqual(["stop", "open"]);
    expect(useProjectStore.getState().projectEpoch).toBe(4);
  });

  it("stops native playback before creating a fresh project", async () => {
    await newProjectAndEnter();

    expect(srv.order.slice(0, 2)).toEqual(["stop", "new"]);
    expect(useProjectStore.getState().projectEpoch).toBe(5);
  });
});
