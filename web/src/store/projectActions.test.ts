import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaList, Timeline } from "../lib/types";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

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
      return {
        timeline,
        projectEpoch: 4,
        version: 7,
        projectPath: "/tmp/core-resolved.opentake",
        compatibilityReadOnly: false,
        compatibilityBlockers: [],
      };
    }),
    projectNew: vi.fn(async () => {
      order.push("new");
      return {
        timeline,
        projectEpoch: 5,
        version: 0,
        projectPath: null,
        compatibilityReadOnly: false,
        compatibilityBlockers: [],
      };
    }),
    projectSave: vi.fn(async (path: string | null) => path ?? ""),
  };
});

vi.mock("../lib/api", () => ({
  projectOpen: srv.projectOpen,
  projectNew: srv.projectNew,
  projectSave: srv.projectSave,
  getDefaultProjectDir: async () => "",
  getTimeline: async () => ({
    timeline: srv.timeline,
    projectEpoch: 5,
    version: 0,
    projectPath: null,
    compatibilityReadOnly: false,
    compatibilityBlockers: [],
  }),
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

import { newProjectAndEnter, openProjectPath, saveCurrentProject } from "./projectActions";
import { useEditorUiStore } from "./uiStore";
import { useMediaStore } from "./mediaStore";
import { useProjectStore } from "./projectStore";
import { useRecentStore } from "./recentStore";
import { useI18nStore } from "../i18n";

describe("openProjectPath", () => {
  beforeEach(() => {
    srv.order.length = 0;
    srv.stopBoundary.mockClear();
    srv.projectOpen.mockClear();
    srv.projectNew.mockClear();
    srv.projectSave.mockReset();
    srv.projectSave.mockImplementation(async (path: string | null) => path ?? "");
    useMediaStore.getState().setItems([]);
    useRecentStore.setState({ recents: [] });
    useProjectStore.setState({ projectPath: null, timelineVersion: 0 });
    useEditorUiStore.setState({ view: "home", toast: null });
  });

  it("refreshes the media mirror after opening a project", async () => {
    await openProjectPath("/tmp/demo.opentake");

    expect(useProjectStore.getState().projectPath).toBe("/tmp/core-resolved.opentake");
    expect(useRecentStore.getState().recents[0]?.path).toBe("/tmp/core-resolved.opentake");
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
    expect(useProjectStore.getState().projectPath).toBe("/tmp/fresh.opentake");
  });
});

describe("saveCurrentProject", () => {
  beforeEach(() => {
    srv.projectSave.mockReset();
    srv.projectSave.mockImplementation(async (path: string | null) => path ?? "");
    useProjectStore.setState({
      snapshotMutationRevision: 0,
      projectEpoch: 1,
      projectPath: "/tmp/unknown.opentake",
      timelineVersion: 9,
      lastSavedVersion: 8,
    });
    useEditorUiStore.setState({ toast: null });
    useI18nStore.setState({ locale: "zh-CN" });
  });

  it("surfaces a production-shaped string rejection and keeps the document dirty", async () => {
    srv.projectSave.mockRejectedValueOnce(
      "project is compatibility read-only because this build does not understand future fields",
    );

    await saveCurrentProject();

    expect(useEditorUiStore.getState().toast?.message).toBe(
      "保存失败：project is compatibility read-only because this build does not understand future fields",
    );
    expect(useProjectStore.getState().lastSavedVersion).toBe(8);
  });

  it("queues one follow-up save when the document advances during an in-flight save", async () => {
    const first = deferred<string>();
    const second = deferred<string>();
    srv.projectSave
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);

    const saving = saveCurrentProject();
    useProjectStore.getState().setMirror(srv.timeline, 10, 1);
    first.resolve("/tmp/unknown.opentake");

    await vi.waitFor(() => expect(srv.projectSave).toHaveBeenCalledTimes(2));
    expect(useProjectStore.getState().lastSavedVersion).toBe(8);

    second.resolve("/tmp/unknown.opentake");
    await saving;
    expect(useProjectStore.getState().lastSavedVersion).toBe(10);
  });

  it("suppresses a stale failure after switching projects", async () => {
    const first = deferred<string>();
    srv.projectSave.mockImplementationOnce(() => first.promise);

    const saving = saveCurrentProject();
    useProjectStore.getState().replaceProjectSnapshot({
      timeline: srv.timeline,
      projectEpoch: 2,
      version: 3,
      projectPath: "/tmp/new.opentake",
      compatibilityReadOnly: false,
      compatibilityBlockers: [],
    });
    first.reject("old project save failed");
    await saving;

    expect(useEditorUiStore.getState().toast).toBeNull();
    expect(useProjectStore.getState().lastSavedVersion).toBe(3);
  });

  it("coalesces overlapping autosave and keyboard requests", async () => {
    const first = deferred<string>();
    srv.projectSave.mockImplementationOnce(() => first.promise);

    const autosave = saveCurrentProject();
    const keyboardSave = saveCurrentProject();
    expect(srv.projectSave).toHaveBeenCalledTimes(1);

    first.resolve("/tmp/unknown.opentake");
    await Promise.all([autosave, keyboardSave]);
    expect(srv.projectSave).toHaveBeenCalledTimes(1);
    expect(useProjectStore.getState().lastSavedVersion).toBe(9);
  });
});
