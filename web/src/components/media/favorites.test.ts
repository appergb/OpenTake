/**
 * migrateLocalFavorites 单测（#91）：把遗留 localStorage 星标迁进后端 manifest。
 * 只对命中当前 items 且未收藏的 id 调 toggle_favorite，并把命中的 id 从 localStorage
 * 移除（清空后删键），其余留给拥有它们的其他项目。vitest node 环境无 localStorage，
 * 这里注入内存 stub；后端 api 用 vi.mock 拦截以观察调用。
 */
import { beforeEach, describe, expect, it, vi } from "vitest";

// vi.hoisted so the mock fn exists before vi.mock's hoisted factory runs.
const { toggleFavorite } = vi.hoisted(() => ({
  toggleFavorite: vi.fn(async () => ({ items: [], folders: [] })),
}));
vi.mock("../../lib/api", () => ({ toggleFavorite }));

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

const KEY = "opentake.favorites";

describe("migrateLocalFavorites", () => {
  beforeEach(() => {
    vi.resetModules();
    toggleFavorite.mockClear();
    vi.stubGlobal("localStorage", makeLocalStorage());
  });

  it("favorites only the unfavorited matches and drains migrated ids", async () => {
    localStorage.setItem(KEY, JSON.stringify(["a", "b", "other"]));
    const { migrateLocalFavorites } = await import("./favorites");

    const applied = await migrateLocalFavorites([
      { id: "a", favorite: false }, // needs favoriting
      { id: "b", favorite: true }, // already a favorite -> skip the backend call
    ]);

    expect(applied).toBe(true);
    expect(toggleFavorite).toHaveBeenCalledTimes(1);
    expect(toggleFavorite).toHaveBeenCalledWith(["a"], true);
    // a and b were present in this project -> removed; "other" stays for its project.
    expect(JSON.parse(localStorage.getItem(KEY) as string)).toEqual(["other"]);
  });

  it("removes the key once every stored id is migrated", async () => {
    localStorage.setItem(KEY, JSON.stringify(["a"]));
    const { migrateLocalFavorites } = await import("./favorites");
    await migrateLocalFavorites([{ id: "a", favorite: false }]);
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it("is a no-op with no legacy data or no matching items", async () => {
    const { migrateLocalFavorites } = await import("./favorites");
    // No localStorage entry at all.
    expect(await migrateLocalFavorites([{ id: "a", favorite: false }])).toBe(false);
    // Stored ids that don't match the current project's items are left untouched.
    localStorage.setItem(KEY, JSON.stringify(["ghost"]));
    expect(await migrateLocalFavorites([{ id: "a", favorite: false }])).toBe(false);
    expect(toggleFavorite).not.toHaveBeenCalled();
    expect(JSON.parse(localStorage.getItem(KEY) as string)).toEqual(["ghost"]);
  });
});
