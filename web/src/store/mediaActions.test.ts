import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaList } from "../lib/types";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const srv = vi.hoisted(() => ({
  selected: ["/tmp/new.mov"],
  imported: {
    items: [
      { id: "old", name: "old", type: "video", duration: 10, hasAudio: true, path: "/tmp/old.mov" },
      { id: "fresh-video", name: "fresh", type: "video", duration: 10, hasAudio: true, path: "/tmp/new.mov" },
      { id: "fresh-audio", name: "audio", type: "audio", duration: 10, hasAudio: false, path: "/tmp/a.wav" },
      { id: "fresh-image", name: "still", type: "image", duration: 1, hasAudio: false, path: "/tmp/i.png" },
    ],
    folders: [],
  } as MediaList,
  importMedia: vi.fn(),
  importFolder: vi.fn(),
  relinkMedia: vi.fn(),
  getMedia: vi.fn(),
  preloadMedia: vi.fn(),
  loadOpenDialog: vi.fn(),
  open: vi.fn(),
}));

vi.mock("../lib/api", () => ({
  importMedia: srv.importMedia,
  importFolder: srv.importFolder,
  relinkMedia: srv.relinkMedia,
  getMedia: srv.getMedia,
  preloadMedia: srv.preloadMedia,
}));

vi.mock("../lib/dialog", () => ({
  openDialog: srv.loadOpenDialog,
}));

import {
  importFilesViaDialog,
  importFolderViaDialog,
  relinkMediaViaDialog,
} from "./mediaActions";
import { useMediaStore } from "./mediaStore";
import { useProjectStore } from "./projectStore";
import { useEditorUiStore } from "./uiStore";

describe("mediaActions import warmup", () => {
  beforeEach(() => {
    srv.importMedia.mockReset();
    srv.importFolder.mockReset();
    srv.relinkMedia.mockReset();
    srv.getMedia.mockReset();
    srv.preloadMedia.mockReset();
    srv.loadOpenDialog.mockReset();
    srv.open.mockReset();
    srv.loadOpenDialog.mockResolvedValue(srv.open);
    srv.open.mockResolvedValue(srv.selected);
    srv.importMedia.mockResolvedValue(srv.imported);
    srv.importFolder.mockResolvedValue(srv.imported);
    srv.relinkMedia.mockResolvedValue(undefined);
    srv.getMedia.mockResolvedValue(srv.imported);
    useMediaStore.setState({
      items: [
        { id: "old", name: "old", type: "video", duration: 10, hasAudio: true, path: "/tmp/old.mov" },
      ],
      folders: [],
      importing: false,
      error: null,
    });
    useProjectStore.setState({
      projectEpoch: 1,
      projectPath: "/tmp/project-a.opentake",
    });
    useEditorUiStore.setState({ toast: null });
  });

  it("preloads newly imported timeline-capable media after file import", async () => {
    await importFilesViaDialog();

    expect(srv.preloadMedia).toHaveBeenCalledTimes(2);
    expect(srv.preloadMedia).toHaveBeenNthCalledWith(1, "fresh-video");
    expect(srv.preloadMedia).toHaveBeenNthCalledWith(2, "fresh-audio");
    expect(srv.preloadMedia).not.toHaveBeenCalledWith("old");
    expect(srv.preloadMedia).not.toHaveBeenCalledWith("fresh-image");
  });

  it("does not publish an old project import failure after a project switch", async () => {
    const pending = deferred<MediaList>();
    srv.importMedia.mockImplementationOnce(() => pending.promise);

    const importing = importFilesViaDialog();
    await vi.waitFor(() => expect(srv.importMedia).toHaveBeenCalledTimes(1));
    useProjectStore.setState({
      projectEpoch: 2,
      projectPath: "/tmp/project-b.opentake",
    });
    pending.reject("old project import failed");
    await importing;

    expect(useMediaStore.getState().error).toBeNull();
  });

  it("does not warm media returned for a project that is no longer current", async () => {
    const pending = deferred<MediaList>();
    srv.importMedia.mockImplementationOnce(() => pending.promise);

    const importing = importFilesViaDialog();
    await vi.waitFor(() => expect(srv.importMedia).toHaveBeenCalledTimes(1));
    useProjectStore.setState({
      projectEpoch: 2,
      projectPath: "/tmp/project-b.opentake",
    });
    pending.resolve(srv.imported);
    await importing;

    expect(srv.preloadMedia).not.toHaveBeenCalled();
  });

  it("surfaces a production-shaped string failure for the current project", async () => {
    srv.importMedia.mockRejectedValueOnce("current project import failed");

    await importFilesViaDialog();

    expect(useMediaStore.getState().error).toBe("current project import failed");
  });

  it("keeps importing visible when a second picker is cancelled", async () => {
    const pending = deferred<MediaList>();
    srv.importMedia.mockImplementationOnce(() => pending.promise);
    const firstImport = importFilesViaDialog();
    await vi.waitFor(() => expect(useMediaStore.getState().importing).toBe(true));

    srv.open.mockResolvedValueOnce(null);
    await importFilesViaDialog();
    expect(useMediaStore.getState().importing).toBe(true);

    pending.resolve(srv.imported);
    await firstImport;
    expect(useMediaStore.getState().importing).toBe(false);
  });

  it("keeps importing visible until both concurrent imports finish", async () => {
    const first = deferred<MediaList>();
    const second = deferred<MediaList>();
    srv.importMedia
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);

    const firstImport = importFilesViaDialog();
    const secondImport = importFilesViaDialog();
    await vi.waitFor(() => expect(srv.importMedia).toHaveBeenCalledTimes(2));

    first.resolve(srv.imported);
    await firstImport;
    expect(useMediaStore.getState().importing).toBe(true);

    second.resolve(srv.imported);
    await secondImport;
    expect(useMediaStore.getState().importing).toBe(false);
  });

  it("does not start a folder import selected after the project changed", async () => {
    const selected = deferred<string>();
    srv.open.mockImplementationOnce(() => selected.promise);

    const importing = importFolderViaDialog();
    useProjectStore.setState({
      projectEpoch: 2,
      projectPath: "/tmp/project-b.opentake",
    });
    selected.resolve("/tmp/folder");
    await importing;

    expect(srv.importFolder).not.toHaveBeenCalled();
  });

  it.each([
    ["file import", () => importFilesViaDialog()],
    ["folder import", () => importFolderViaDialog()],
    ["relink", () => relinkMediaViaDialog("asset-1")],
  ])(
    "does not open a stale %s picker or clear the new project error after dialog loading",
    async (_label, runAction) => {
      const dialogLoader = deferred<typeof srv.open>();
      srv.loadOpenDialog.mockImplementationOnce(() => dialogLoader.promise);

      const action = runAction();
      useProjectStore.setState({
        projectEpoch: 2,
        projectPath: "/tmp/project-b.opentake",
      });
      useMediaStore.setState({ error: "new project error" });
      dialogLoader.resolve(srv.open);
      await action;

      expect(srv.open).not.toHaveBeenCalled();
      expect(useMediaStore.getState().error).toBe("new project error");
    },
  );

  it("surfaces a production-shaped relink failure for the current project", async () => {
    srv.open.mockResolvedValueOnce("/tmp/relink.mov");
    srv.relinkMedia.mockRejectedValueOnce("current project relink failed");

    await relinkMediaViaDialog("asset-1");

    expect(useMediaStore.getState().error).toBe("current project relink failed");
  });

  it("does not report skipped media after switching projects during refresh", async () => {
    const pendingRefresh = deferred<MediaList>();
    srv.importMedia.mockResolvedValueOnce({
      ...srv.imported,
      skipped: ["unsupported.bin"],
    });
    srv.getMedia.mockImplementationOnce(() => pendingRefresh.promise);

    const importing = importFilesViaDialog();
    await vi.waitFor(() => expect(srv.getMedia).toHaveBeenCalledTimes(1));
    useProjectStore.setState({
      projectEpoch: 2,
      projectPath: "/tmp/project-b.opentake",
    });
    pendingRefresh.resolve(srv.imported);
    await importing;

    expect(useEditorUiStore.getState().toast).toBeNull();
  });
});
