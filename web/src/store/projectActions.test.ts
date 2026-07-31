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
    createdPath: null as string | null,
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
    projectNew: vi.fn(async (path: string | null = null) => {
      order.push("new");
      srv.createdPath = path;
      return {
        timeline,
        projectEpoch: 5,
        version: 0,
        projectPath: path,
        compatibilityReadOnly: false,
        compatibilityBlockers: [],
      };
    }),
    projectSave: vi.fn(async (path: string | null) => path ?? ""),
    sampleProjectMaterialize: vi.fn(async () => "/tmp/cache/quick-tutorial/Tutorial.opentake"),
    getMedia: vi.fn(async () => media),
    openDialog: vi.fn(async () => undefined),
  };
});

vi.mock("../lib/api", () => ({
  projectOpen: srv.projectOpen,
  projectNew: srv.projectNew,
  projectSave: srv.projectSave,
  sampleProjectMaterialize: srv.sampleProjectMaterialize,
  getDefaultProjectDir: async () => "",
  getTimeline: async () => ({
    timeline: srv.timeline,
    projectEpoch: 5,
    version: 0,
    projectPath: srv.createdPath,
    compatibilityReadOnly: false,
    compatibilityBlockers: [],
  }),
  canUndo: async () => false,
  canRedo: async () => false,
  getMedia: srv.getMedia,
}));

vi.mock("../components/preview/nativePlaybackSession", () => ({
  stopNativePlaybackForProjectBoundary: srv.stopBoundary,
}));

vi.mock("../lib/dialog", () => ({
  saveDialog: async () => async () => "/tmp/fresh.opentake",
  openDialog: srv.openDialog,
}));

import {
  newProjectAndEnter,
  openProjectPath,
  openProjectViaDialog,
  openSampleProject,
  saveCurrentProject,
  saveCurrentProjectAs,
} from "./projectActions";
import { useEditorUiStore } from "./uiStore";
import { useMediaStore } from "./mediaStore";
import { useProjectStore } from "./projectStore";
import { useRecentStore } from "./recentStore";
import { useI18nStore } from "../i18n";

describe("openProjectPath", () => {
  beforeEach(() => {
    srv.order.length = 0;
    srv.createdPath = null;
    srv.stopBoundary.mockClear();
    srv.projectOpen.mockClear();
    srv.projectNew.mockReset();
    srv.projectNew.mockImplementation(async (path: string | null = null) => {
      srv.order.push("new");
      srv.createdPath = path;
      return {
        timeline: srv.timeline,
        projectEpoch: 5,
        version: 0,
        projectPath: path,
        compatibilityReadOnly: false,
        compatibilityBlockers: [],
      };
    });
    srv.getMedia.mockReset();
    srv.getMedia.mockImplementation(async () => srv.media);
    srv.projectSave.mockReset();
    srv.projectSave.mockImplementation(async (path: string | null) => path ?? "");
    srv.openDialog.mockReset();
    srv.openDialog.mockResolvedValue(undefined);
    useMediaStore.setState({ items: [], folders: [], importing: false, error: null });
    useRecentStore.setState({ recents: [] });
    useProjectStore.setState({ projectPath: null, timelineVersion: 0 });
    useEditorUiStore.setState({ view: "home", toast: null });
    useI18nStore.setState({ locale: "zh-CN" });
  });

  it("refreshes the media mirror after opening a project", async () => {
    await openProjectPath("/tmp/demo.opentake");

    expect(useProjectStore.getState().projectPath).toBe("/tmp/core-resolved.opentake");
    expect(useRecentStore.getState().recents[0]?.path).toBe("/tmp/core-resolved.opentake");
    expect(useMediaStore.getState().items.map((item) => item.id)).toEqual(["m1"]);
    expect(useEditorUiStore.getState().view).toBe("editor");
  });

  it("clears a media error from the previously open project", async () => {
    useMediaStore.getState().setError("old project import failed");

    await openProjectPath("/tmp/demo.opentake");

    expect(useMediaStore.getState().error).toBeNull();
  });

  it("reports a native picker failure before project-open delegation", async () => {
    srv.openDialog.mockRejectedValueOnce(new Error("picker unavailable"));

    await expect(openProjectViaDialog()).rejects.toThrow("picker unavailable");

    expect(useEditorUiStore.getState().toast?.message).toBe("打开失败：picker unavailable");
    expect(srv.projectOpen).not.toHaveBeenCalled();
  });

  it("preserves media transient state when project open fails", async () => {
    const oldFolder = { id: "old-folder", name: "Old", parentFolderId: null };
    useMediaStore.setState({
      items: srv.media.items,
      folders: [oldFolder],
      importing: true,
      error: "old project error",
    });
    const failure = { code: "engine", message: "project open timed out after 15s" };
    srv.projectOpen.mockRejectedValueOnce(failure);

    await expect(openProjectPath("/tmp/broken.opentake")).rejects.toBe(failure);

    expect(useMediaStore.getState().importing).toBe(true);
    expect(useMediaStore.getState().error).toBe("old project error");
    expect(useMediaStore.getState().items).toEqual(srv.media.items);
    expect(useMediaStore.getState().folders).toEqual([oldFolder]);
    expect(useEditorUiStore.getState().toast?.message).toBe(
      "打开失败：project open timed out after 15s",
    );
  });

  it("clears the old catalog immediately, then installs the opened project catalog", async () => {
    useMediaStore.setState({
      items: [{ ...srv.media.items[0]!, id: "old-item" }],
      folders: [{ id: "old-folder", name: "Old", parentFolderId: null }],
    });
    const nextCatalog = deferred<MediaList>();
    srv.getMedia.mockImplementationOnce(() => nextCatalog.promise);

    const opening = openProjectPath("/tmp/demo.opentake");
    await vi.waitFor(() => {
      expect(useProjectStore.getState().projectPath).toBe("/tmp/core-resolved.opentake");
    });
    expect(useMediaStore.getState().items).toEqual([]);
    expect(useMediaStore.getState().folders).toEqual([]);

    nextCatalog.resolve(srv.media);
    await opening;
    expect(useMediaStore.getState().items.map((item) => item.id)).toEqual(["m1"]);
  });

  it("never restores the old catalog when the opened project media refresh fails", async () => {
    useMediaStore.setState({
      items: [{ ...srv.media.items[0]!, id: "old-item" }],
      folders: [{ id: "old-folder", name: "Old", parentFolderId: null }],
    });
    srv.getMedia.mockRejectedValueOnce(new Error("media refresh failed"));

    await expect(openProjectPath("/tmp/demo.opentake")).rejects.toThrow("media refresh failed");

    expect(useProjectStore.getState().projectPath).toBe("/tmp/core-resolved.opentake");
    expect(useMediaStore.getState().items).toEqual([]);
    expect(useMediaStore.getState().folders).toEqual([]);
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
    expect(srv.projectNew).toHaveBeenCalledWith("/tmp/fresh.opentake");
    expect(srv.projectSave).not.toHaveBeenCalled();
    expect(useProjectStore.getState().projectEpoch).toBe(5);
    expect(useProjectStore.getState().projectPath).toBe("/tmp/fresh.opentake");
  });

  it("preserves the current project when initial creation fails", async () => {
    const oldTimeline: Timeline = {
      ...srv.timeline,
      tracks: [
        {
          id: "old-track",
          type: "video",
          muted: false,
          hidden: false,
          syncLocked: true,
          clips: [],
        },
      ],
    };
    useProjectStore.setState({
      projectEpoch: 3,
      timelineVersion: 12,
      timeline: oldTimeline,
      projectPath: "/tmp/current.opentake",
      lastSavedVersion: 11,
    });
    useMediaStore.setState({ items: srv.media.items, folders: [], error: "old media state" });
    const failure = { code: "engine", message: "project create timed out after 15s" };
    srv.projectNew.mockRejectedValueOnce(failure);

    await expect(newProjectAndEnter()).rejects.toBe(failure);

    const project = useProjectStore.getState();
    expect(project.projectEpoch).toBe(3);
    expect(project.timelineVersion).toBe(12);
    expect(project.timeline).toBe(oldTimeline);
    expect(project.projectPath).toBe("/tmp/current.opentake");
    expect(useMediaStore.getState().items).toEqual(srv.media.items);
    expect(useMediaStore.getState().error).toBe("old media state");
    expect(useEditorUiStore.getState().view).toBe("home");
    expect(useEditorUiStore.getState().toast?.message).toBe(
      "创建失败：project create timed out after 15s",
    );
    expect(srv.projectSave).not.toHaveBeenCalled();
  });
});

describe("openSampleProject", () => {
  beforeEach(() => {
    srv.order.length = 0;
    srv.sampleProjectMaterialize.mockClear();
    srv.projectOpen.mockClear();
    srv.projectOpen.mockImplementation(async () => ({
      timeline: srv.timeline,
      projectEpoch: 8,
      version: 2,
      projectPath: "/tmp/cache/quick-tutorial/Tutorial.opentake",
      compatibilityReadOnly: false,
      compatibilityBlockers: [],
    }));
    srv.getMedia.mockResolvedValue(srv.media);
    useRecentStore.setState({
      recents: [{ path: "/tmp/User.opentake", name: "User", openedAt: 1 }],
    });
    useEditorUiStore.setState({ view: "home", toast: null });
    useI18nStore.setState({ locale: "en" });
  });

  it("opens a completed tutorial sample without registering its cache path", async () => {
    await openSampleProject("quick-tutorial", true);

    expect(srv.sampleProjectMaterialize).toHaveBeenCalledWith("quick-tutorial");
    expect(srv.projectOpen).toHaveBeenCalledWith(
      "/tmp/cache/quick-tutorial/Tutorial.opentake",
    );
    expect(useRecentStore.getState().recents.map(({ name }) => name)).toEqual(["User"]);
    expect(useEditorUiStore.getState().view).toBe("editor");
    expect(useEditorUiStore.getState().toast?.message).toContain("Tutorial project opened");
  });

  it("does not open or mutate recents when materialization fails", async () => {
    srv.sampleProjectMaterialize.mockRejectedValueOnce(new Error("download failed"));

    await expect(openSampleProject("quick-tutorial", true)).rejects.toThrow("download failed");

    expect(srv.projectOpen).not.toHaveBeenCalled();
    expect(useRecentStore.getState().recents.map(({ name }) => name)).toEqual(["User"]);
    expect(useEditorUiStore.getState().view).toBe("home");
    expect(useEditorUiStore.getState().toast?.message).toContain("download failed");
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

  it("queues an explicit save for a clean project opened during another project save", async () => {
    const first = deferred<string>();
    srv.projectSave
      .mockImplementationOnce(() => first.promise)
      .mockRejectedValueOnce("project B is compatibility read-only");

    const projectASave = saveCurrentProject();
    useProjectStore.getState().replaceProjectSnapshot({
      timeline: srv.timeline,
      projectEpoch: 2,
      version: 3,
      projectPath: "/tmp/project-b.opentake",
      compatibilityReadOnly: true,
      compatibilityBlockers: ["project.json:futureTimeline"],
    });
    const projectBSave = saveCurrentProject();
    first.resolve("/tmp/unknown.opentake");
    await Promise.all([projectASave, projectBSave]);

    expect(srv.projectSave).toHaveBeenCalledTimes(2);
    expect(useEditorUiStore.getState().toast?.message).toBe(
      "保存失败：project B is compatibility read-only",
    );
    expect(useProjectStore.getState().lastSavedVersion).toBe(3);
  });

  it("suppresses a failure after the initiating snapshot revision changes", async () => {
    const first = deferred<string>();
    srv.projectSave.mockImplementationOnce(() => first.promise);
    useProjectStore.setState({ lastSavedVersion: 9 });

    const saving = saveCurrentProject();
    useProjectStore.getState().setMirror(srv.timeline, 9, 1);
    first.reject("stale save failure");
    await saving;

    expect(useEditorUiStore.getState().toast).toBeNull();
    expect(srv.projectSave).toHaveBeenCalledTimes(1);
  });

  it("does not redirect a stale queued request to a different dirty project", async () => {
    const first = deferred<string>();
    srv.projectSave.mockImplementationOnce(() => first.promise);

    const projectASave = saveCurrentProject();
    useProjectStore.getState().replaceProjectSnapshot({
      timeline: srv.timeline,
      projectEpoch: 2,
      version: 3,
      projectPath: "/tmp/project-b.opentake",
      compatibilityReadOnly: true,
      compatibilityBlockers: ["project.json:futureTimeline"],
    });
    const staleProjectBSave = saveCurrentProject();
    useProjectStore.getState().replaceProjectSnapshot({
      timeline: srv.timeline,
      projectEpoch: 3,
      version: 4,
      projectPath: "/tmp/project-c.opentake",
      compatibilityReadOnly: false,
      compatibilityBlockers: [],
    });
    useProjectStore.getState().setMirror(srv.timeline, 5, 3);
    first.resolve("/tmp/unknown.opentake");
    await Promise.all([projectASave, staleProjectBSave]);

    expect(srv.projectSave).toHaveBeenCalledTimes(1);
    expect(useProjectStore.getState().lastSavedVersion).toBe(4);
    expect(useProjectStore.getState().timelineVersion).toBe(5);
  });
});

describe("saveCurrentProjectAs", () => {
  beforeEach(() => {
    srv.projectSave.mockReset();
    srv.projectSave.mockImplementation(async (path: string | null) => path ?? "");
    useRecentStore.setState({ recents: [] });
    useProjectStore.setState({
      snapshotMutationRevision: 0,
      projectEpoch: 1,
      projectPath: "/tmp/current.opentake",
      compatibilityReadOnly: false,
      timelineVersion: 9,
      lastSavedVersion: 8,
    });
    useEditorUiStore.setState({ toast: null });
    useI18nStore.setState({ locale: "zh-CN" });
  });

  it("adopts the core-returned Save As path only after publication succeeds", async () => {
    srv.projectSave.mockResolvedValueOnce("/tmp/canonical-fresh.opentake");

    await saveCurrentProjectAs();

    expect(srv.projectSave).toHaveBeenCalledWith("/tmp/fresh.opentake");
    expect(useProjectStore.getState().projectPath).toBe("/tmp/canonical-fresh.opentake");
    expect(useProjectStore.getState().lastSavedVersion).toBe(9);
    expect(useRecentStore.getState().recents[0]?.path).toBe(
      "/tmp/canonical-fresh.opentake",
    );
  });

  it("preserves the active path and dirty state when Save As fails", async () => {
    srv.projectSave.mockRejectedValueOnce(new Error("destination denied"));

    await expect(saveCurrentProjectAs()).rejects.toThrow("destination denied");

    expect(useProjectStore.getState().projectPath).toBe("/tmp/current.opentake");
    expect(useProjectStore.getState().lastSavedVersion).toBe(8);
    expect(useRecentStore.getState().recents).toEqual([]);
    expect(useEditorUiStore.getState().toast?.message).toBe("保存失败：destination denied");
  });

  it("does not open a Save As flow for compatibility read-only projects", async () => {
    useProjectStore.setState({ compatibilityReadOnly: true });

    await saveCurrentProjectAs();

    expect(srv.projectSave).not.toHaveBeenCalled();
    expect(useProjectStore.getState().projectPath).toBe("/tmp/current.opentake");
  });
});
