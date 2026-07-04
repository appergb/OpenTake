/**
 * 素材「我的（收藏）」的持久化存储。剪映式星标收藏：把 media id 集合存到
 * localStorage（跨项目级，与 #37 全局素材库方向一致；本期「我的」按 id 命中
 * 当前已加载的 items 过滤，命中不到则不显示，不会误显其他项目的素材）。
 *
 * 用 zustand store 暴露给 React 订阅，与项目现有 store 风格一致；toggle 用不可变
 * 方式（new Set）以遵守 immutability 规则，并同步写回 localStorage。
 */

import { create } from "zustand";

const LS_FAVORITES = "opentake.favorites";

/** 从 localStorage 读取收藏 id（SSR/测试安全：localStorage 不存在时回退空集）。 */
function loadFavorites(): Set<string> {
  if (typeof localStorage === "undefined") return new Set();
  try {
    const raw = localStorage.getItem(LS_FAVORITES);
    if (!raw) return new Set();
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? new Set(parsed.filter((v): v is string => typeof v === "string")) : new Set();
  } catch {
    // 损坏的存储值不应让面板崩溃：回退空集。
    return new Set();
  }
}

function persistFavorites(ids: Set<string>): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(LS_FAVORITES, JSON.stringify([...ids]));
}

interface FavoritesState {
  ids: Set<string>;
  toggle: (id: string) => void;
}

export const useFavoritesStore = create<FavoritesState>((set, get) => ({
  ids: loadFavorites(),
  toggle: (id) => {
    const next = new Set(get().ids);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    persistFavorites(next);
    set({ ids: next });
  },
}));

/** 订阅单个 id 的收藏状态（供 MediaCard 用，仅在该 id 变化时重渲染）。 */
export function useIsFavorite(id: string): boolean {
  return useFavoritesStore((s) => s.ids.has(id));
}
