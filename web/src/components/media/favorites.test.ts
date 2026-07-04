/**
 * favorites store 单测：toggle 切换收藏 + localStorage 持久化往返。
 * vitest 默认 node 环境无 localStorage，这里注入一个内存版 stub。
 */
import { beforeEach, describe, expect, it, vi } from "vitest";

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

describe("favorites store", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("localStorage", makeLocalStorage());
  });

  it("toggle 添加/移除收藏 id", async () => {
    const { useFavoritesStore } = await import("./favorites");
    const { toggle } = useFavoritesStore.getState();

    toggle("a");
    expect(useFavoritesStore.getState().ids.has("a")).toBe(true);

    toggle("a");
    expect(useFavoritesStore.getState().ids.has("a")).toBe(false);
  });

  it("toggle 用不可变方式更新（new Set 引用变化）", async () => {
    const { useFavoritesStore } = await import("./favorites");
    const before = useFavoritesStore.getState().ids;
    useFavoritesStore.getState().toggle("x");
    const after = useFavoritesStore.getState().ids;
    expect(after).not.toBe(before);
  });

  it("写回 localStorage 并在重新加载时恢复", async () => {
    const mod = await import("./favorites");
    mod.useFavoritesStore.getState().toggle("keep");
    expect(localStorage.getItem("opentake.favorites")).toContain("keep");

    // 重新加载模块（同一 localStorage stub）应从存储恢复收藏。
    vi.resetModules();
    const reloaded = await import("./favorites");
    expect(reloaded.useFavoritesStore.getState().ids.has("keep")).toBe(true);
  });
});
