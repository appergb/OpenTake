/**
 * favorites store 单测（#91-C 后端版）：toggle 路由到全局库 IPC，startFavoritesSync
 * 从 library_list 拉取填充 sources，并一次性迁移 localStorage 旧星标。
 *
 * libraryApi 被 mock：不真正调 Tauri，只验证调用契约 + store 状态变化。
 * localStorage stub 提供 migration flag / 旧 favorites 读写。
 */
import { beforeEach, describe, expect, it, vi } from "vitest";

// Hoisted mock — 工厂在每次 resetModules 重新加载时重新执行，产出新的 vi.fn()。
vi.mock("../../lib/libraryApi", () => ({
  libraryList: vi.fn(),
  libraryFavorite: vi.fn(),
  libraryUnfavorite: vi.fn(),
}));

import type { MediaItem } from "../../lib/types";

function makeLocalStorage(): Storage {
  const map = new Map<string, string>();
  return {
    getItem: (k) => (map.has(k) ? (map.get(k) as string) : null),
    setItem: (k, v) => void map.set(k, String(v)),
    removeItem: (k) => void map.delete(k),
    clear: () => map.clear(),
    key: (i) => [...map.keys()][i] ?? null,
    get length() {
      return map.size;
    },
  } as Storage;
}

function item(id: string, path: string, type: MediaItem["type"] = "video"): MediaItem {
  return { id, name: id, type, duration: 0, hasAudio: type === "video", path };
}

describe("favorites store (global library backend)", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    vi.stubGlobal("localStorage", makeLocalStorage());
  });

  it("startFavoritesSync pulls library entries into sources by source path", async () => {
    const libApi = await import("../../lib/libraryApi");
    (libApi.libraryList as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: "h1", type: "video", favoritedAt: 0, source: "/a.mp4" },
      { id: "h2", type: "audio", favoritedAt: 0, source: "/b.wav" },
    ]);
    const { useFavoritesStore, startFavoritesSync } = await import("./favorites");
    await startFavoritesSync([item("a", "/a.mp4")]);
    const sources = useFavoritesStore.getState().sources;
    expect(sources.has("/a.mp4")).toBe(true);
    expect(sources.has("/b.wav")).toBe(true);
  });

  it("toggle favorites a path via libraryFavorite", async () => {
    const libApi = await import("../../lib/libraryApi");
    (libApi.libraryList as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    (libApi.libraryFavorite as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "h1",
      type: "video",
      favoritedAt: 0,
      source: "/a.mp4",
    });
    const { useFavoritesStore } = await import("./favorites");
    await useFavoritesStore.getState().toggle(item("a", "/a.mp4"));
    expect(useFavoritesStore.getState().sources.has("/a.mp4")).toBe(true);
    expect(libApi.libraryFavorite).toHaveBeenCalledWith("/a.mp4", "video", undefined, undefined);
  });

  it("toggle unfavorites an already-favorited path via libraryUnfavorite", async () => {
    const libApi = await import("../../lib/libraryApi");
    (libApi.libraryList as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: "h1", type: "video", favoritedAt: 0, source: "/a.mp4" },
    ]);
    (libApi.libraryUnfavorite as ReturnType<typeof vi.fn>).mockResolvedValue(true);
    const { useFavoritesStore, startFavoritesSync } = await import("./favorites");
    await startFavoritesSync([item("a", "/a.mp4")]);
    expect(useFavoritesStore.getState().sources.has("/a.mp4")).toBe(true);
    await useFavoritesStore.getState().toggle(item("a", "/a.mp4"));
    expect(useFavoritesStore.getState().sources.has("/a.mp4")).toBe(false);
    expect(libApi.libraryUnfavorite).toHaveBeenCalledWith("h1");
  });

  it("toggle no-ops for an item without a resolvable path", async () => {
    const libApi = await import("../../lib/libraryApi");
    (libApi.libraryList as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    const { useFavoritesStore } = await import("./favorites");
    const noPath = { ...item("a", "/a.mp4"), path: null } as MediaItem;
    await useFavoritesStore.getState().toggle(noPath);
    expect(libApi.libraryFavorite).not.toHaveBeenCalled();
    expect(useFavoritesStore.getState().sources.size).toBe(0);
  });

  it("migrates localStorage legacy favorites to library on first sync", async () => {
    const ls = makeLocalStorage();
    ls.setItem("opentake.favorites", JSON.stringify(["a"]));
    vi.stubGlobal("localStorage", ls);
    const libApi = await import("../../lib/libraryApi");
    (libApi.libraryList as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    (libApi.libraryFavorite as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "h1",
      type: "video",
      favoritedAt: 0,
      source: "/a.mp4",
    });
    const { useFavoritesStore, startFavoritesSync } = await import("./favorites");
    await startFavoritesSync([item("a", "/a.mp4")]);
    expect(libApi.libraryFavorite).toHaveBeenCalledWith("/a.mp4", "video", undefined, undefined);
    // 旧 localStorage 键清掉，迁移完成标志置位。
    expect(ls.getItem("opentake.favorites")).toBeNull();
    expect(ls.getItem("opentake.favorites.migratedToLibrary")).toBe("1");
    // 迁移成功的 path 进入 sources。
    expect(useFavoritesStore.getState().sources.has("/a.mp4")).toBe(true);
  });

  it("does not re-migrate once the migration flag is set", async () => {
    const ls = makeLocalStorage();
    ls.setItem("opentake.favorites.migratedToLibrary", "1");
    vi.stubGlobal("localStorage", ls);
    const libApi = await import("../../lib/libraryApi");
    (libApi.libraryList as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    const { startFavoritesSync } = await import("./favorites");
    await startFavoritesSync([item("a", "/a.mp4")]);
    expect(libApi.libraryFavorite).not.toHaveBeenCalled();
  });
});
