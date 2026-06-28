# library-media — 全局素材库与媒体导入命令

> 上级：[本模块目录](INDEX.md) · [总览](OVERVIEW.md) · [模块文档树](../INDEX.md)
>
> 源码：[`../../../src-tauri/src/library.rs`](../../../src-tauri/src/library.rs) · [`../../../src-tauri/src/media.rs`](../../../src-tauri/src/media.rs)

本文件覆盖两组相邻命令：**媒体导入**（把本地文件带进当前工程）与**全局素材库**（跨工程的收藏夹）。

---

## A. 媒体导入命令（media.rs）

媒体面板把本地文件带进**当前工程**的命令。架在两个 managed state 上：

- `AppCore`——权威会话；导入向其 manifest 追加 `MediaManifestEntry` 并发 `MediaChanged`（经 [setup-lib.md](setup-lib.md) 的 `forward_event` 转给 WebView）。
- `MediaState`——`MediaEngine` 的薄包装，此处仅用于 **probe** 每个文件（时长 / 尺寸 / fps / 是否有音）。

对应上游 `addMediaAsset(from:)` → `finalizeImportedAsset`：先按路径建**外部引用**条目（文件不拷进 bundle），再 probe 回填元数据。**probe 是 best-effort**：ffprobe 不可用或文件不可读时，资产仍以零 / 空元数据导入，不让整批失败（缺失 / 离线文件是编辑器已建模的可恢复状态）。

### 命令清单（6 个）

| 命令 | 入参 | 返回 | 说明 |
|---|---|---|---|
| `get_media` | — | `MediaListDto` | 当前目录树快照，**不可失败** |
| `import_media` | `paths: Vec<String>` | `MediaListDto` | 导入显式文件列表；不支持 / 不可读的跳过（非致命） |
| `import_folder` | `path`、`recursive?` | `MediaListDto` | `recursive=false`：扁平导入顶层文件到库根；`recursive=true`：**镜像目录树**（剪映式，#49），为每个子目录建库文件夹，空目录也建 |
| `relink_media` | `media_ref`、`new_path` | `MediaListDto` | 见下「relink」 |
| `extract_audio` | `media_id`、`out_path` | `String` | 抽取资产音轨成独立文件（`.m4a`/`.mp3`/`.wav`，编码由扩展名定）（#39） |
| `get_waveform` | `media_ref` | `Vec<f32>` | 归一化波形桶（0=响,1=静），引擎计算 + 磁盘缓存；跨**整个源**，时间线自行映射各 clip 的 trim 子区间 |

### MediaItemDto（面板项，camelCase）

`id` / `name` / `type`（`ClipType` 小写）/ `duration`（秒）/ `width` / `height` / `hasAudio` / `path` / `thumbnail` / `folderId` / `missing`。

- `thumbnail` 当前恒为 `None`（本阶段面板回退到类型占位）；持久化 + 服务缩略图是后续阶段。
- `missing`：**每次读取按文件是否存在重算**（镜像上游 `MediaResolver.isMissing`），故 `relink_media` 指回真实文件后自动清除。无法解析的（如纯远程）源不标 missing。

### relink（关键修复）

`relink_media` 把缺失 / 离线资产指向新选文件，**保留同一 asset id**，使每个引用它的 clip 就地恢复。这是「丢失媒体重选路径后仍红」的修复：旧流程只有 `import_media`，会铸**新 id**、把现有 clip 永远晾在缺失条目上。镜像上游 `EditorViewModel.relinkAsset(id:to:)`——新文件类型必须与原一致（否则拒绝），新 probe 元数据刷新条目。命令层先校验类型匹配再触碰目录（给精确报错、省一次无谓 probe）。

### 路径解析

导入只产外部资产（`MediaSource::External { absolute_path }`，绝对）；已存工程会把媒体拷进 bundle、改写为 `MediaSource::Project { relative_path }`，解析需 join bundle dir（`core.project_dir()`）。未存工程时 `Project` 相对资产无法解析路径——`extract_audio` / `get_waveform` 对此返回明确错误。

---

## B. 全局素材库命令（library.rs）

跨工程、copy-on-favorite 的素材库（#55，#37「全局可复用素材库」的一部分），架在 `opentake_media::library::LibraryStore`（#54）之上——根目录 `<data dir>/OpenTake/Library`。Store owns 全部持久化（原子 manifest、内容寻址文件、进程内写锁）；每个命令都是薄 shim：自身不持锁，调 store 方法，把 `MediaError` 转 `String`。

> 与 [commands-ipc.md](commands-ipc.md) 里的 `CreateFolder`/`MoveToFolder`/`DeleteMedia` 等区分：那些是**工程内**素材库领域命令（走 `EditCommand`、进时间线事务、可撤销）；这里的 `library_*` 是**跨工程**全局库，独立持久化，不进撤销栈。

### 命令清单（7 个）

| 命令 | 入参 | 返回 | 说明 |
|---|---|---|---|
| `library_list` | `category?` | `Vec<LibraryEntryDto>` | `None`/空 = 全部；非空 = 该分类 |
| `library_favorite` | `source`、`kind`、`category?`、`thumb?` | `LibraryEntryDto` | 把本地文件拷进库（按内容 hash 去重）；`favoritedAt` 由服务端钟取 |
| `library_unfavorite` | `id` | `bool` | 按 id 删条目 + 拷贝；未知 id 返回 `false`（幂等） |
| `library_categorize` | `id`、`category?` | `LibraryEntryDto` | 设 / 清单条目分类 |
| `library_rename` | `from`、`to?` | `usize` | 重命名分类（移动该分类全部条目）；返回改动数 |
| `library_delete` | `id` | `bool` | `library_unfavorite` 的别名（前端「从库删除」） |
| `library_import_to_project` | `id` | `LibraryImportDto` | 把库条目带进**当前**工程 |

### LibraryEntryDto / LibraryImportDto

`LibraryEntryDto`（camelCase）：`id`（内容 SHA-256 hex，库内 id）/ `type` / `category?` / `favoritedAt`（epoch 秒）/ `source?` / `thumb?`。是 `LibraryEntry` 的 serde-稳定镜像，命令层独立持有线上形状。

`library_import_to_project` 桥接全局库回当前工程：解析条目的存储拷贝 → 用 `MediaState` 引擎 probe（失败降级默认值，importing 绝不因元数据失败）→ 以**新工程 asset id** 追加进 `AppCore` manifest（故同一收藏可导入多个工程）。返回 `LibraryImportDto { id, name, path }`（just-created 项，供前端乐观更新；前端随后用 `get_media` 重取全量）。错误：未知 id / 存储文件丢失 / 类型不可导入 / core 拒绝。

---

> 相关：[setup-lib.md](setup-lib.md)（`MediaState` / `LibraryState` 装配）· [render.md](render.md) / [export.md](export.md)（媒体路径解析同源）· [commands-ipc.md](commands-ipc.md)（工程内素材库 `EditCommand`）· 跨模块 [opentake-media](../opentake-media/INDEX.md)（`MediaEngine` / `LibraryStore`）· [opentake-core](../opentake-core/INDEX.md)（manifest）
>
> 导航：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
