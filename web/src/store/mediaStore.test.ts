/**
 * mediaStore 单测：refreshMedia 把后端 get_media 的 { items, folders } 双双写入
 * 镜像 store（文件夹浏览需要 folders 不再被丢弃），且 setters 为不可变替换。
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaFolder, MediaItem, MediaList } from "../lib/types";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const srv = vi.hoisted(() => ({
  media: { items: [], folders: [] } as MediaList,
  getMedia: vi.fn(),
  onMediaChanged: vi.fn(async () => () => {}),
  mediaChangedHandler: null as null | (() => Promise<void> | void),
}));

vi.mock("../lib/api", () => ({
  getMedia: srv.getMedia,
  onMediaChanged: srv.onMediaChanged,
}));

import {
  applyMediaErrorForProject,
  applyMediaListForProject,
  useMediaStore,
  refreshMedia,
  resetProjectMediaState,
  startMediaSync,
  stopMediaSync,
} from "./mediaStore";
import { useProjectStore } from "./projectStore";
import { useEditorUiStore } from "./uiStore";

const item = (
  id: string,
  folderId: string | null,
  over: Partial<MediaItem> = {},
): MediaItem => ({
  id,
  name: id,
  type: "video",
  duration: 1,
  hasAudio: false,
  folderId,
  ...over,
});
const folder = (id: string, parentFolderId: string | null): MediaFolder => ({
  id,
  name: id,
  parentFolderId,
});

describe("mediaStore", () => {
  beforeEach(() => {
    srv.getMedia.mockReset();
    srv.getMedia.mockImplementation(async () => srv.media);
    srv.onMediaChanged.mockReset();
    srv.onMediaChanged.mockImplementation(async (handler: () => Promise<void> | void) => {
      srv.mediaChangedHandler = handler;
      return () => {};
    });
    srv.mediaChangedHandler = null;
    resetProjectMediaState();
    useProjectStore.setState({
      projectEpoch: 1,
      projectPath: "/tmp/project-a.opentake",
    });
    useEditorUiStore.setState({ toast: null });
  });

  afterEach(() => {
    stopMediaSync();
  });

  it("starts with empty items and folders", () => {
    expect(useMediaStore.getState().items).toEqual([]);
    expect(useMediaStore.getState().folders).toEqual([]);
  });

  it("can retry startup after the initial media refresh rejects", async () => {
    srv.getMedia
      .mockRejectedValueOnce(new Error("initial media unavailable"))
      .mockImplementation(async () => srv.media);

    await expect(startMediaSync()).rejects.toThrow("initial media unavailable");
    await startMediaSync();

    expect(srv.getMedia).toHaveBeenCalledTimes(3);
    expect(srv.onMediaChanged).toHaveBeenCalledOnce();
  });

  it("can retry startup after media-listener registration rejects", async () => {
    srv.onMediaChanged.mockRejectedValueOnce(new Error("media listener unavailable"));

    await expect(startMediaSync()).rejects.toThrow("media listener unavailable");
    await startMediaSync();

    expect(srv.getMedia).toHaveBeenCalledTimes(3);
    expect(srv.onMediaChanged).toHaveBeenCalledTimes(2);
  });

  it("unsubscribes a media listener registered after startup was stopped", async () => {
    const registration = deferred<() => void>();
    const unsubscribe = vi.fn();
    srv.onMediaChanged.mockImplementationOnce(() => registration.promise);

    const startup = startMediaSync();
    await vi.waitFor(() => expect(srv.onMediaChanged).toHaveBeenCalledOnce());
    stopMediaSync();
    registration.resolve(unsubscribe);
    await startup;

    expect(unsubscribe).toHaveBeenCalledOnce();
  });

  it("refreshes again after listener registration so a setup-gap event is not lost", async () => {
    const oldCatalog = {
      items: [item("old", null)],
      folders: [folder("old-folder", null)],
    };
    const newCatalog = {
      items: [item("new", null)],
      folders: [folder("new-folder", null)],
    };
    srv.getMedia.mockResolvedValueOnce(oldCatalog).mockResolvedValueOnce(newCatalog);

    await startMediaSync();

    expect(srv.getMedia).toHaveBeenCalledTimes(2);
    expect(srv.onMediaChanged).toHaveBeenCalledOnce();
    expect(useMediaStore.getState().items.map(({ id }) => id)).toEqual(["new"]);
  });

  it("retries a rejected media event refresh and converges without leaking a rejection", async () => {
    await startMediaSync();
    srv.getMedia
      .mockRejectedValueOnce(new Error("transient media read failed"))
      .mockResolvedValueOnce({
        items: [item("recovered", null)],
        folders: [],
      });

    await srv.mediaChangedHandler?.();

    expect(useMediaStore.getState().items.map(({ id }) => id)).toEqual(["recovered"]);
    expect(useMediaStore.getState().error).toBeNull();
  });

  it("reports a media event refresh that still fails after its bounded retry", async () => {
    await startMediaSync();
    srv.getMedia
      .mockRejectedValueOnce(new Error("media read unavailable"))
      .mockRejectedValueOnce(new Error("media read still unavailable"));

    await srv.mediaChangedHandler?.();

    expect(useMediaStore.getState().error).toBe("media read still unavailable");
    expect(useEditorUiStore.getState().toast?.message).toContain(
      "media read still unavailable",
    );
  });

  it("clears an old sync error after a later authoritative event refresh succeeds", async () => {
    await startMediaSync();
    srv.getMedia
      .mockRejectedValueOnce(new Error("media sync failed"))
      .mockRejectedValueOnce(new Error("media sync still failed"));

    await srv.mediaChangedHandler?.();
    expect(useMediaStore.getState().error).toBe("media sync still failed");

    srv.getMedia.mockResolvedValueOnce({
      items: [item("recovered", null)],
      folders: [],
    });
    await srv.mediaChangedHandler?.();

    expect(useMediaStore.getState().items.map(({ id }) => id)).toEqual(["recovered"]);
    expect(useMediaStore.getState().error).toBeNull();
  });

  it("does not clear an independent import error after media sync recovers", async () => {
    await startMediaSync();
    srv.getMedia
      .mockRejectedValueOnce(new Error("media sync failed"))
      .mockRejectedValueOnce(new Error("media sync still failed"));
    await srv.mediaChangedHandler?.();

    expect(
      applyMediaErrorForProject(
        { projectEpoch: 1, projectPath: "/tmp/project-a.opentake" },
        "import failed independently",
      ),
    ).toBe(true);
    srv.getMedia.mockResolvedValueOnce({
      items: [item("recovered", null)],
      folders: [],
    });
    await srv.mediaChangedHandler?.();

    expect(useMediaStore.getState().items.map(({ id }) => id)).toEqual(["recovered"]);
    expect(useMediaStore.getState().error).toBe("import failed independently");
  });

  it("refreshMedia loads both items and the folder tree", async () => {
    srv.media = {
      items: [item("a", null), item("b", "trip")],
      folders: [folder("trip", null), folder("day1", "trip")],
    };

    await refreshMedia();

    const state = useMediaStore.getState();
    expect(state.items.map((i) => i.id)).toEqual(["a", "b"]);
    expect(state.folders.map((f) => f.id)).toEqual(["trip", "day1"]);
    expect(state.folders[1].parentFolderId).toBe("trip");
  });

  it("refreshMedia dedups duplicate ids and keeps the last item deterministically", async () => {
    srv.media = {
      items: [
        item("dup", null, { name: "first", duration: 1 }),
        item("keep", "trip", { name: "keep", duration: 2 }),
        item("dup", "trip", { name: "second", duration: 99, hasAudio: true }),
      ],
      folders: [folder("trip", null)],
    };

    await refreshMedia();

    const state = useMediaStore.getState();
    expect(state.items).toHaveLength(2);
    expect(state.items.map((i) => i.id)).toEqual(["dup", "keep"]);
    expect(state.items[0]).toMatchObject({
      id: "dup",
      name: "second",
      duration: 99,
      hasAudio: true,
      folderId: "trip",
    });
  });

  it("setFolders replaces immutably (new array reference)", () => {
    const before = useMediaStore.getState().folders;
    useMediaStore.getState().setFolders([folder("x", null)]);
    const after = useMediaStore.getState().folders;
    expect(after).not.toBe(before);
    expect(after).toHaveLength(1);
  });

  it("clears the complete old-project media state at a successful project boundary", () => {
    useMediaStore.setState({
      items: [item("project-a", "project-a-folder")],
      folders: [folder("project-a-folder", null)],
      importing: true,
      error: "project A import failed",
    });

    resetProjectMediaState();

    expect(useMediaStore.getState()).toMatchObject({
      items: [],
      folders: [],
      importing: false,
      error: null,
    });
  });

  it("does not let an older project refresh overwrite the current catalog", async () => {
    const projectA = deferred<MediaList>();
    srv.getMedia.mockImplementationOnce(() => projectA.promise);
    const staleRefresh = refreshMedia();

    useProjectStore.setState({
      projectEpoch: 2,
      projectPath: "/tmp/project-b.opentake",
    });
    srv.media = {
      items: [item("project-b", null)],
      folders: [folder("project-b-folder", null)],
    };
    await refreshMedia();

    projectA.resolve({
      items: [item("project-a", null)],
      folders: [folder("project-a-folder", null)],
    });
    await staleRefresh;

    expect(useMediaStore.getState().items.map((entry) => entry.id)).toEqual(["project-b"]);
    expect(useMediaStore.getState().folders.map((entry) => entry.id)).toEqual([
      "project-b-folder",
    ]);
  });

  it("does not let an older same-project refresh overwrite a migration result", async () => {
    const older = deferred<MediaList>();
    srv.getMedia.mockImplementationOnce(() => older.promise);
    const staleRefresh = refreshMedia();
    const project = {
      projectEpoch: 1,
      projectPath: "/tmp/project-a.opentake",
    };

    expect(
      applyMediaListForProject(project, {
        items: [item("migrated", null)],
        folders: [folder("migrated-folder", null)],
      }),
    ).toBe(true);
    older.resolve({
      items: [item("pre-migration", null)],
      folders: [folder("old-folder", null)],
    });

    expect(await staleRefresh).toBe(false);
    expect(useMediaStore.getState().items.map((entry) => entry.id)).toEqual(["migrated"]);
    expect(useMediaStore.getState().folders.map((entry) => entry.id)).toEqual([
      "migrated-folder",
    ]);
  });
});
