/**
 * mediaStore 单测：refreshMedia 把后端 get_media 的 { items, folders } 双双写入
 * 镜像 store（文件夹浏览需要 folders 不再被丢弃），且 setters 为不可变替换。
 */
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaFolder, MediaItem, MediaList } from "../lib/types";

const srv: { media: MediaList } = {
  media: { items: [], folders: [] },
};

vi.mock("../lib/api", () => ({
  getMedia: async (): Promise<MediaList> => srv.media,
}));

import { useMediaStore, refreshMedia } from "./mediaStore";

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
    useMediaStore.getState().setItems([]);
    useMediaStore.getState().setFolders([]);
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
});
