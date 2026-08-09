/**
 * 全局素材库 store(#56)。与 `mediaStore`(项目内媒体)彼此独立:这里持有的是
 * 跨项目的全局收藏库镜像,真值在 Rust(#54 存储 + #55 命令)。store 只保存条目列表 +
 * 一个加载/错误标志,所有写操作走 `libraryApi`,成功后重新拉全量(命令返回的是单条,
 * 列表用一次 `library_list` 保持与服务端一致,避免本地拼接漂移)。
 *
 * 视图态(选中的分类、搜索词、排序)也放这里,因为库页是独立全屏视图,不污染
 * 编辑器的 `uiStore`。
 */

import { create } from "zustand";
import * as lib from "../lib/libraryApi";
import type { LibraryEntry, LibraryImport, LibraryImportWarning } from "../lib/libraryApi";
import { t } from "../i18n";
import { refreshMedia } from "./mediaStore";
import { useEditorUiStore } from "./uiStore";

let lifecycleGeneration = 0;
let refreshRequestToken = 0;

/** 内置分类(与素材类型/音效一一对应);自建分类从条目的 `category` 聚合而来。
 *  `all` 是聚合视图(跨所有子库可见全部收藏)。`sound` 为音效库(独立于 audio)。 */
export type BuiltinCategory =
  | "all"
  | "video"
  | "audio"
  | "image"
  | "effect"
  | "sound";

export type SortKey = "recent" | "oldest" | "type";

function getErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

interface LibraryState {
  entries: LibraryEntry[];
  loading: boolean;
  error: string | null;
  lastImportWarning: LibraryImportWarning | null;

  // 视图态
  selectedCategory: string; // BuiltinCategory | 自建分类名
  search: string;
  sort: SortKey;

  setSelectedCategory: (category: string) => void;
  setSearch: (search: string) => void;
  setSort: (sort: SortKey) => void;

  refresh: () => Promise<void>;
  unfavorite: (id: string) => Promise<void>;
  categorize: (id: string, category: string | null) => Promise<void>;
  renameCategory: (from: string, to: string | null) => Promise<void>;
  remove: (id: string) => Promise<void>;
  /** 把库条目导入当前项目;成功后刷新项目媒体目录。返回新资产引用(失败返回 null)。 */
  importToProject: (id: string) => Promise<LibraryImport | null>;
}

type LibraryMutation =
  | "unfavorite"
  | "categorize"
  | "renameCategory"
  | "remove"
  | "importToProject";

const mutationRequestTokens: Record<LibraryMutation, number> = {
  unfavorite: 0,
  categorize: 0,
  renameCategory: 0,
  remove: 0,
  importToProject: 0,
};

function beginMutation(mutation: LibraryMutation): () => boolean {
  const generation = lifecycleGeneration;
  const requestToken = ++mutationRequestTokens[mutation];
  return () =>
    generation === lifecycleGeneration &&
    requestToken === mutationRequestTokens[mutation];
}

type LibraryStateSetter = (state: Partial<LibraryState>) => void;

async function refreshLibraryState(
  set: LibraryStateSetter,
  operationCurrent: () => boolean = () => true,
): Promise<void> {
  const generation = lifecycleGeneration;
  const requestToken = ++refreshRequestToken;
  const requestCurrent = () =>
    generation === lifecycleGeneration &&
    requestToken === refreshRequestToken;
  const commitIfCurrent = (state: Partial<LibraryState>) => {
    if (requestCurrent() && operationCurrent()) set(state);
  };

  commitIfCurrent({ loading: true, error: null });
  try {
    const entries = await lib.libraryList();
    commitIfCurrent({ entries, loading: false });
  } catch (error: unknown) {
    commitIfCurrent({ error: getErrorMessage(error), loading: false });
  } finally {
    // A newer invocation of the same mutation may supersede this post-write
    // reload before it settles. Release only this request's loading lease.
    if (requestCurrent() && !operationCurrent()) set({ loading: false });
  }
}

/** Reconcile the catalog after a native mutation has already committed.
 *
 * This reload is intentionally independent from the initiating gesture's
 * stale UI token: an older request may commit after a newer request already
 * refreshed. Every later-started reload is authoritative and therefore
 * contains all earlier local commits. Lifecycle changes retry against the new
 * owner, while a still-newer refresh safely supersedes this one. Existing
 * errors/warnings are preserved so a stale success cannot erase newer UI
 * feedback. */
async function reconcileCommittedLibraryMutation(
  set: LibraryStateSetter,
  operationCurrent: () => boolean,
): Promise<void> {
  for (;;) {
    const generation = lifecycleGeneration;
    const requestToken = ++refreshRequestToken;
    try {
      const entries = await lib.libraryList();
      if (generation !== lifecycleGeneration) continue;
      if (requestToken === refreshRequestToken) {
        set({ entries, loading: false });
      }
      return;
    } catch (error: unknown) {
      if (generation === lifecycleGeneration && requestToken === refreshRequestToken) {
        set(
          operationCurrent()
            ? { error: getErrorMessage(error), loading: false }
            : { loading: false },
        );
      }
      return;
    }
  }
}

export const useLibraryStore = create<LibraryState>((set) => ({
  entries: [],
  loading: false,
  error: null,
  lastImportWarning: null,

  selectedCategory: "all",
  search: "",
  sort: "recent",

  setSelectedCategory: (selectedCategory) => set({ selectedCategory }),
  setSearch: (search) => set({ search }),
  setSort: (sort) => set({ sort }),

  // 总是拉全量(不带 category 过滤),分类/搜索/排序在前端派生,这样切分类无需重拉,
  // 且「跨视图聚合」(收藏在所有子库视图都可见)天然成立。
  refresh: () => refreshLibraryState(set),

  unfavorite: async (id) => {
    const mutationCurrent = beginMutation("unfavorite");
    try {
      await lib.libraryUnfavorite(id);
      if (mutationCurrent()) set({ error: null });
      await reconcileCommittedLibraryMutation(set, mutationCurrent);
    } catch (error: unknown) {
      if (mutationCurrent()) set({ error: getErrorMessage(error) });
    }
  },

  categorize: async (id, category) => {
    const mutationCurrent = beginMutation("categorize");
    try {
      await lib.libraryCategorize(id, category);
      if (mutationCurrent()) set({ error: null });
      await reconcileCommittedLibraryMutation(set, mutationCurrent);
    } catch (error: unknown) {
      if (mutationCurrent()) set({ error: getErrorMessage(error) });
    }
  },

  renameCategory: async (from, to) => {
    const mutationCurrent = beginMutation("renameCategory");
    try {
      await lib.libraryRename(from, to);
      if (mutationCurrent()) set({ error: null });
      await reconcileCommittedLibraryMutation(set, mutationCurrent);
    } catch (error: unknown) {
      if (mutationCurrent()) set({ error: getErrorMessage(error) });
    }
  },

  remove: async (id) => {
    const mutationCurrent = beginMutation("remove");
    try {
      await lib.libraryDelete(id);
      if (mutationCurrent()) set({ error: null });
      await reconcileCommittedLibraryMutation(set, mutationCurrent);
    } catch (error: unknown) {
      if (mutationCurrent()) set({ error: getErrorMessage(error) });
    }
  },

  // 库与项目媒体是两套数据:导入后用 #55 命令在项目 manifest 里造新 asset,再调
  // mediaStore 的 refreshMedia 拉项目全量目录(库本身不变,无需 refresh 本 store)。
  importToProject: async (id) => {
    const mutationCurrent = beginMutation("importToProject");
    let imported: lib.LibraryImport;
    try {
      imported = await lib.libraryImportToProject(id);
    } catch (error: unknown) {
      if (mutationCurrent()) {
        set({ error: getErrorMessage(error), lastImportWarning: null });
      }
      return null;
    }
    if (!mutationCurrent()) return imported;
    set({ error: null, lastImportWarning: imported.warning ?? null });
    if (imported.warning?.kind === "postconditionRollbackFailed") {
      useEditorUiStore.getState().pushToast(t("library.importCommittedWarning"));
    }
    try {
      await refreshMedia();
    } catch (error: unknown) {
      // The Rust transaction is already committed. A mirror refresh failure
      // must not turn that success into null or encourage an unsafe retry.
      if (mutationCurrent()) set({ error: getErrorMessage(error) });
    }
    return imported;
  },
}));

/** 内置分类集合(用于判断一个 category 字符串是否为自建分类)。 */
const BUILTIN: ReadonlySet<string> = new Set<BuiltinCategory>([
  "all",
  "video",
  "audio",
  "image",
  "effect",
  "sound",
]);

/** 把内置分类映射到它筛选的素材类型;`all` 返回 null(不按类型过滤)。
 *  `sound`(音效)与 `audio`(音频/音乐)都落在 type==='audio',靠条目自身的
 *  `category` 区分:被显式归到 "sound" 分类的音频归音效库。 */
function categoryTypeFilter(category: string): string | null {
  switch (category) {
    case "video":
      return "video";
    case "image":
      return "image";
    case "effect":
      return "effect";
    case "audio":
    case "sound":
      return "audio";
    default:
      return null;
  }
}

/** 从条目派生出自建分类列表(去重 + 排除内置名),用于分类树的「我的分类」分组。 */
export function deriveCustomCategories(entries: ReadonlyArray<LibraryEntry>): string[] {
  const seen = new Set<string>();
  for (const e of entries) {
    if (e.category && !BUILTIN.has(e.category)) seen.add(e.category);
  }
  return [...seen].sort((a, b) => a.localeCompare(b));
}

/**
 * 按当前视图态(分类/搜索/排序)派生要显示的条目。纯函数,不可变,便于测试。
 * - 分类:内置分类按素材类型过滤;`sound` 仅显式归入 "sound" 分类的音频;
 *   自建分类按 `category` 精确匹配;`all` 聚合全部(跨视图聚合)。
 * - 搜索:对 source 文件名/分类做大小写无关子串匹配。
 * - 排序:recent(默认,favoritedAt 降序)/oldest/type(按类型字典序,稳定回退 recent)。
 */
export function selectEntries(
  entries: ReadonlyArray<LibraryEntry>,
  category: string,
  search: string,
  sort: SortKey,
): LibraryEntry[] {
  const typeFilter = categoryTypeFilter(category);
  const isCustom = !BUILTIN.has(category);
  const q = search.trim().toLowerCase();

  const filtered = entries.filter((e) => {
    // 音效库:仅显式归到 "sound" 分类的音频条目。
    if (category === "sound") {
      if (e.type !== "audio" || e.category !== "sound") return false;
    } else if (category === "audio") {
      // 音频/音乐库:type==='audio' 但排除被归入音效的(避免与音效库重复)。
      if (e.type !== "audio" || e.category === "sound") return false;
    } else if (isCustom) {
      if (e.category !== category) return false;
    } else if (typeFilter && e.type !== typeFilter) {
      return false;
    }

    if (q) {
      const name = sourceName(e.source).toLowerCase();
      const cat = (e.category ?? "").toLowerCase();
      if (!name.includes(q) && !cat.includes(q)) return false;
    }
    return true;
  });

  const sorted = [...filtered];
  if (sort === "oldest") {
    sorted.sort((a, b) => a.favoritedAt - b.favoritedAt);
  } else if (sort === "type") {
    sorted.sort(
      (a, b) => a.type.localeCompare(b.type) || b.favoritedAt - a.favoritedAt,
    );
  } else {
    sorted.sort((a, b) => b.favoritedAt - a.favoritedAt);
  }
  return sorted;
}

/** 从 source 绝对路径取文件名(用于显示/搜索);缺省返回空串。 */
export function sourceName(source: string | undefined): string {
  if (!source) return "";
  const base = source.split(/[\\/]/).pop();
  return base ?? source;
}

let started = false;

/** 幂等引导:首次进入库页时拉一次全量。库变更目前由页面动作主动 refresh,
 *  无独立事件通道(#55 命令同步返回),所以不订阅事件。 */
export async function startLibrarySync(): Promise<void> {
  if (started) return;
  started = true;
  const generation = ++lifecycleGeneration;
  await useLibraryStore.getState().refresh();
  if (generation !== lifecycleGeneration) return;
  // `refresh` intentionally reports failures through store state instead of
  // rejecting. Keep that UI-facing contract while reopening bootstrap so the
  // next automatic entry can retry a failed first load.
  if (useLibraryStore.getState().error) started = false;
}

/** Release the module-local startup lease. Primarily used when an owning view or
 * test lifecycle is torn down so the next mount performs a fresh authoritative load. */
export function stopLibrarySync(): void {
  lifecycleGeneration += 1;
  refreshRequestToken += 1;
  started = false;
  useLibraryStore.setState({ loading: false });
}
