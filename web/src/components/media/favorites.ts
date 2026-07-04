/**
 * 素材「我的（收藏）」的一次性迁移（#91）。
 *
 * 收藏状态已从浏览器 localStorage 迁到后端项目 manifest：每个 MediaItem 直接带
 * `favorite` 标志（见 `toggle_favorite` 命令），面板「我的」tab 读它、星标按钮写它。
 * 本模块只负责把遗留的 `opentake.favorites` localStorage 键排空进当前项目：对每个
 * 命中当前已加载 item 且尚未收藏的 id 调后端收藏，再把已迁移的 id 从 localStorage
 * 移除（清空后删键）。它自带幂等守卫——迁移完再跑就是空操作。跨项目场景下按项目逐个
 * 迁移（每次项目媒体加载时对命中的 id 迁移），localStorage 逐步收缩直至清空。
 */

import * as api from "../../lib/api";

const LS_FAVORITES = "opentake.favorites";

/** 读取遗留的收藏 id（存储不存在/损坏时回退空集，绝不抛出）。 */
function loadLegacyFavorites(): Set<string> {
  if (typeof localStorage === "undefined") return new Set();
  try {
    const raw = localStorage.getItem(LS_FAVORITES);
    if (!raw) return new Set();
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed)
      ? new Set(parsed.filter((v): v is string => typeof v === "string"))
      : new Set();
  } catch {
    return new Set();
  }
}

/**
 * 把遗留 localStorage 收藏迁进当前项目的 manifest。命中当前 `items` 且未收藏的 id
 * 调 `toggle_favorite(true)`；随后把命中的 id 从 localStorage 移除（清空后删键）。
 * 返回是否实际写入了收藏（调用方据此刷新一次媒体镜像）。幂等：无遗留数据或无命中即
 * 返回 false。
 */
export async function migrateLocalFavorites(
  items: ReadonlyArray<{ id: string; favorite?: boolean }>,
): Promise<boolean> {
  const stored = loadLegacyFavorites();
  if (stored.size === 0) return false;

  const present = items.filter((i) => stored.has(i.id));
  if (present.length === 0) return false; // 存的 id 都不属于当前项目 → 留待其他项目迁移

  const toApply = present.filter((i) => !i.favorite).map((i) => i.id);
  if (toApply.length > 0) {
    try {
      await api.toggleFavorite(toApply, true);
    } catch {
      return false; // 迁移失败：保留 localStorage，下次加载再试
    }
  }

  // 移除已迁移（命中）的 id；清空后整个删键。
  const presentIds = new Set(present.map((i) => i.id));
  const remaining = [...stored].filter((id) => !presentIds.has(id));
  if (typeof localStorage !== "undefined") {
    if (remaining.length === 0) localStorage.removeItem(LS_FAVORITES);
    else localStorage.setItem(LS_FAVORITES, JSON.stringify(remaining));
  }
  return toApply.length > 0;
}
