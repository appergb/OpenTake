/**
 * 素材「我的（收藏）」的持久化存储 — 全局库后端版（#91-C）。
 *
 * 星标不再存 localStorage，而是路由到 Rust 全局素材库（#37/#54/#106）：
 * `library_favorite` 把源文件 copy-on-favorite 进 `<data dir>/OpenTake/Library`，
 * 跨项目可用、内容哈希去重；`library_unfavorite` 移除；`library_list` 拉全量。
 *
 * 项目内一个 MediaItem 的收藏状态靠 **source path** 匹配全局库条目的 `source`
 * 字段判定（同一文件路径 = 同一收藏）。条目 id 是内容哈希，与项目资产 id 不同
 * 域，所以 unfavorite 要先查 `entryIds` map 拿到条目 id。
 *
 * 一次性迁移：首次启动时若 localStorage 有旧星标（项目资产 id 数组），用当前
 * mediaStore items 把每个 id 解析回 source path，批量 `library_favorite` 入库，
 * 然后清 localStorage 并写迁移完成标志。找不到对应 item 的旧 id 静默跳过（原
 * 文件可能已不在当前项目）。
 *
 * 用 zustand store 暴露给 React 订阅；不可变更新（new Set / new Map）。
 */

import { create } from "zustand";
import type { MediaItem } from "../../lib/types";
import { libraryFavorite, libraryList, libraryUnfavorite } from "../../lib/libraryApi";

const LS_FAVORITES = "opentake.favorites";
const LS_MIGRATED = "opentake.favorites.migratedToLibrary";

interface FavoritesState {
  /** 已收藏的 source path 集合。命中 item.path 即视为已收藏。 */
  sources: Set<string>;
  /** source path → 全局库条目 id（内容哈希），用于 unfavorite。 */
  entryIds: Map<string, string>;
  /** 切换收藏。无 path（missing/unresolvable）的 item 直接 no-op。 */
  toggle: (item: MediaItem) => Promise<void>;
}

export const useFavoritesStore = create<FavoritesState>((set, get) => ({
  sources: new Set(),
  entryIds: new Map(),
  toggle: async (item) => {
    const path = item.path;
    if (!path) return; // 无可定位的源文件，无法入库
    const { sources, entryIds } = get();
    if (sources.has(path)) {
      const id = entryIds.get(path);
      if (!id) return; // 状态不一致：本地以为已收藏但无 id，静默放弃
      try {
        await libraryUnfavorite(id);
      } catch (e) {
        console.warn("library_unfavorite failed:", e);
        return;
      }
      const next = new Set(sources);
      next.delete(path);
      const nextMap = new Map(entryIds);
      nextMap.delete(path);
      set({ sources: next, entryIds: nextMap });
      return;
    }
    let entry;
    try {
      entry = await libraryFavorite(path, item.type, undefined, item.thumbnail ?? undefined);
    } catch (e) {
      console.warn("library_favorite failed:", e);
      return;
    }
    const next = new Set(sources);
    next.add(path);
    const nextMap = new Map(entryIds);
    nextMap.set(path, entry.id);
    set({ sources: next, entryIds: nextMap });
  },
}));

/** 订阅单个 source path 的收藏状态（供 MediaCard 用，仅在该 path 变化时重渲染）。 */
export function useIsFavorite(path: string | null | undefined): boolean {
  return useFavoritesStore((s) => path != null && s.sources.has(path));
}

let started = false;

/** 拉取全局库条目填充 sources/entryIds，并一次性迁移 localStorage 旧星标。
 *  MediaPanel 在首次拿到 items 后调用一次（`started` 守卫防重入）。 */
export async function startFavoritesSync(items: MediaItem[]): Promise<void> {
  if (started) return;
  started = true;

  // 1) 拉全量，按 source 建索引（source path → 收藏状态）。
  try {
    const list = await libraryList();
    const sources = new Set<string>();
    const entryIds = new Map<string, string>();
    for (const e of list) {
      if (e.source) {
        sources.add(e.source);
        entryIds.set(e.source, e.id);
      }
    }
    useFavoritesStore.setState({ sources, entryIds });
  } catch (e) {
    console.warn("library_list failed:", e);
  }

  // 2) 一次性迁移：localStorage 旧星标（项目资产 id）→ library_favorite。
  if (typeof localStorage === "undefined") return;
  if (localStorage.getItem(LS_MIGRATED)) return;
  const raw = localStorage.getItem(LS_FAVORITES);
  if (!raw) {
    localStorage.setItem(LS_MIGRATED, "1");
    return;
  }
  let ids: unknown = null;
  try {
    ids = JSON.parse(raw);
  } catch {
    // 损坏的存储值：标记已迁移并清掉，不阻塞 UI。
    localStorage.removeItem(LS_FAVORITES);
    localStorage.setItem(LS_MIGRATED, "1");
    return;
  }
  if (!Array.isArray(ids)) {
    localStorage.removeItem(LS_FAVORITES);
    localStorage.setItem(LS_MIGRATED, "1");
    return;
  }
  const byId = new Map(items.map((i) => [i.id, i] as const));
  const store = useFavoritesStore.getState();
  const nextSources = new Set(store.sources);
  const nextEntryIds = new Map(store.entryIds);
  for (const id of ids.filter((v): v is string => typeof v === "string")) {
    const item = byId.get(id);
    if (!item?.path) continue; // 旧 id 在当前项目已无对应资产，跳过
    try {
      const entry = await libraryFavorite(
        item.path,
        item.type,
        undefined,
        item.thumbnail ?? undefined,
      );
      nextSources.add(item.path);
      nextEntryIds.set(item.path, entry.id);
    } catch {
      // 单条失败不阻断其余迁移。
    }
  }
  useFavoritesStore.setState({ sources: nextSources, entryIds: nextEntryIds });
  localStorage.removeItem(LS_FAVORITES);
  localStorage.setItem(LS_MIGRATED, "1");
}
