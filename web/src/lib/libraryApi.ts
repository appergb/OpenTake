/**
 * Tauri bridge for the global asset library (#55 命令层). 与项目内 `get_media`/
 * `import_*` 完全独立:这是跨项目的全局收藏库,数据通道全新,不复用 mediaStore。
 *
 * 所有命令在非 Tauri 环境(纯 `vite dev`/`preview`)降级为安全空操作,使浏览器外壳
 * 仍可渲染(库为空)。真正的库真值始终在 Rust 侧。invoke 名为 snake_case,返回均为
 * camelCase(serde 已处理)。错误为 rejected Promise 的 string(Tauri 边界转 Err(String))。
 */

import { isTauri } from "./api";

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

let invokeImpl: InvokeFn | null = null;

async function ensureTauri(): Promise<void> {
  if (!isTauri || invokeImpl) return;
  const core = await import("@tauri-apps/api/core");
  invokeImpl = core.invoke as InvokeFn;
}

/** 全局库条目(#55 LibraryEntryDto)。`type` 为素材类型(video/audio/image/...);
 *  `category` 为所属分类(未分类时缺省);`thumb` 为缩略图绝对路径(可缺省)。 */
export interface LibraryEntry {
  id: string;
  type: string;
  category?: string;
  favoritedAt: number;
  source?: string;
  thumb?: string;
}

/** 导入到当前项目后返回的新资产引用(#55 LibraryImportDto)。 */
export interface LibraryImport {
  id: string;
  name: string;
  path: string;
}

/** 列出库条目。`category` 省略/空串=全部;非空=按该分类过滤。 */
export async function libraryList(category?: string): Promise<LibraryEntry[]> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<LibraryEntry[]>("library_list", { category });
  return [];
}

/**
 * 收藏一个磁盘文件到全局库。invoke 参数名为 source/kind/category/thumb(不是 "type")。
 * favoritedAt 服务端取钟,前端不传。内容 hash 去重:重复返回既有条目。`source` 必须是
 * 磁盘存在的文件,否则 Err。非 Tauri 下无文件系统,抛错让调用方据实处理。
 */
export async function libraryFavorite(
  source: string,
  kind: string,
  category?: string,
  thumb?: string,
): Promise<LibraryEntry> {
  await ensureTauri();
  if (invokeImpl)
    return invokeImpl<LibraryEntry>("library_favorite", { source, kind, category, thumb });
  throw new Error("library unavailable outside Tauri");
}

/** 取消收藏(幂等)。true=删了,false=id 未知。 */
export async function libraryUnfavorite(id: string): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("library_unfavorite", { id });
  return false;
}

/** 给条目分类。`category` 传 null 清空;id 未知 → Err。 */
export async function libraryCategorize(
  id: string,
  category: string | null,
): Promise<LibraryEntry> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<LibraryEntry>("library_categorize", { id, category });
  throw new Error("library unavailable outside Tauri");
}

/**
 * 分类批量改名:把 category==from 的所有条目改成 to(to=null 取消分类)。返回改动条数,
 * 0=无匹配。注意这是分类层面的改名,不是改单条目名(LibraryEntry 无 name 字段)。
 */
export async function libraryRename(from: string, to: string | null): Promise<number> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<number>("library_rename", { from, to });
  return 0;
}

/** 删除条目(删条目+删文件),unfavorite 别名。true=删了,false=id 未知。 */
export async function libraryDelete(id: string): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("library_delete", { id });
  return false;
}

/**
 * 把库条目拷进当前打开项目的 media manifest,生成新 asset id(一份收藏可导入多项目)。
 * 调用方拿到结果后应再 `refreshMedia()` 拉全量目录刷新。id 未知/库文件丢失/类型不可
 * 导入 → Err(String)。
 */
export async function libraryImportToProject(id: string): Promise<LibraryImport> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<LibraryImport>("library_import_to_project", { id });
  throw new Error("library unavailable outside Tauri");
}
