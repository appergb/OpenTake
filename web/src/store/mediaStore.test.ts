/**
 * mediaStore 单测：refreshMedia 把后端 get_media 的 { items, folders } 双双写入
 * 镜像 store（文件夹浏览需要 folders 不再被丢弃），且 setters 为不可变替换。
 */
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaFolder, MediaItem, MediaList } from "../lib/types";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

const srv = vi.hoisted(() => ({
  media: { items: [], folders: [] } as MediaList,
  getMedia: vi.fn(),
}));

vi.mock("../lib/api", () => ({
  getMedia: srv.getMedia,
}));

import {
  applyMediaListForProject,
  useMediaStore,
  refreshMedia,
  resetProjectMediaState,
} from "./mediaStore";
import { useProjectStore } from "./projectStore";

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
    useMediaStore.setState({ items: [], folders: [], importing: false, error: null });
    useProjectStore.setState({
      projectEpoch: 1,
      projectPath: "/tmp/project-a.opentake",
    });
  });

  it("starts with empty items and folders", () => {
    expect(useMediaStore.getState().items).toEqual([]);
    expect(useMediaStore.getState().folders).toEqual([]);
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
