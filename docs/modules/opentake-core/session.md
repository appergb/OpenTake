# session.rs — EditorSession 会话管理

> 上级：本模块目录 [INDEX.md](INDEX.md) · 总览 [OVERVIEW.md](OVERVIEW.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 源码：[`../../../crates/opentake-core/src/session.rs`](../../../crates/opentake-core/src/session.rs)

## 定位

`EditorSession` 是装配层的**数据半边**：一份内存中的"打开的工程文档"。它本身不并发、不发事件——并发与可观测由 [`AppCore`](core-router.md) 在其外包一层锁 + 事件总线提供。

## 它持什么（以及为何不是第二个 EditorState）

`opentake_ops::EditorState` 已经拥有可编辑真相（timeline + manifest）与**整套撤销/版本事务机制**。`EditorSession` **不复制其中任何一项**——它按值持有 `EditorState`，把每次编辑委派给 `opentake_ops::command::apply`。它只补 `EditorState` 刻意省略（`EditorState` 是持久化无关的）的两块**工程级状态**：

```
EditorSession
├── state: EditorState            // 来自 opentake-ops：timeline + manifest + 撤销/重做 + version
├── project_dir: Option<PathBuf>  // .opentake 包路径；None = 未保存（上游 EditorViewModel.projectURL）
└── generation_log: GenerationLog // append-only AI 审计；持久化为 generation-log.json（类型在 opentake-project）
```

> `generation_log` 的类型 `GenerationLog` 来自 `opentake-project`，**不在** `opentake-domain`——它是工程持久化的一部分，不是领域值语义。`version` 直接来自 `state.version()`，**不是重复计数器**。

## 构造与生命周期

| 方法 | 行为 | 上游对应 |
|---|---|---|
| `new_project()` | 空 timeline + 空 manifest，version 0，无包路径，空生成日志 | 新建文档（任何 save 之前） |
| `open_project(path)` | 打开 `.opentake` 包到一个全新会话；version 从 0 起；**open 不发变更事件**（调用方自取首个快照） | `VideoProject.read` + `makeWindowControllers` 装配顺序 |
| `save_project(path)` | 写盘；`None` = 存回 `project_dir`（autosave）；`Some(p)` = 另存为并采纳新目录；返回写入的包路径 | `VideoProject.save` / `captureSaveSnapshot` / `fileWrapper` |

### open 装配顺序（照搬上游 `makeWindowControllers`）

`open_project` 走经实战的上游顺序（见 [SPEC.md](SPEC.md) §5.4）：

1. `Project::open(path)` decode `timeline` → `EditorState::new(timeline, manifest)`，**版本 0、空历史**（恰是 post-open 想要的状态）；
2. 记录 `project_dir = Some(bundle_path)`；
3. decode `manifest` 进 `EditorState`；
4. decode `generation_log`（**宽松**：缺失/损坏由 `opentake-project` 降级为 `None`，这里 `unwrap_or_default()`）。

> **容错分级**（上游一致）：`project.json` 缺失/损坏才致命（`Project::open` 报错，作为 `CoreError::Project` 上抛）；`media.json` / `generation-log.json` 的容错由 `opentake-project` 层承担。**素材物化 / 缩略图 / 波形**（上游装配尾部）是媒体层职责、经 [`CoreDeps`](deps-di.md) 注入，**不在本文件**做。

### save 机制

`save_project` 从**活动 timeline/manifest 的克隆**组装一个全新 `Project`（保存绝不改动文档），加上生成日志，交 `opentake-project` 原子写盘：

- 目标路径 = `path` 或回退 `self.project_dir`；二者皆无 → `CoreError::NoProjectOpen`。
- 仅当 `generation_log.entries` 非空才写日志组件（对齐上游"有则写"的容错）。
- 成功后 `project_dir = Some(target)`（另存为采纳新目录），返回写入路径。

## 唯一编辑入口（薄包装）

```rust
pub fn apply(&mut self, command: EditCommand, ids: &dyn IdGen) -> Result<EditResult>
```

把一条 `EditCommand` 路由进 `opentake_ops::command::apply`，**整个 snapshot/commit/version 事务全权委派 ops**。`Undo` / `Redo` 在这里是**普通命令**（ops 层就把它们建模为命令），所以会话**无需任何独立的撤销管线**。`ids` 是注入的 id 生成器（由 `AppCore` 持有并传入，见 [core-router.md](core-router.md)）。

> 事务的"为何"（snapshot → before!=after 短路 → 推快照 + version++）属 `opentake-ops`，详见 [opentake-ops command-apply.md](../opentake-ops/INDEX.md)。本会话只是透传 + 把 `opentake_ops::EditError` 经 `?` 转成 `CoreError`。

## 媒体导入 / 重链（同步，在撤销事务之外）

这是 core 内**已实现**的同步媒体路径，与 [`CoreDeps::media`](deps-di.md) 的异步能力后端是两回事：调用方（`src-tauri`，持媒体引擎）先探测文件，把纯值 `ProbedMedia` 传入，使本逻辑**单测无需调 ffprobe**。

### `ProbedMedia`

会话物化一个 asset 所需的探测事实子集（镜像上游 `MediaAsset.loadMetadata` 读取的：时长 / 尺寸 / fps / 是否有音轨）：

```
ProbedMedia { duration_secs, width: Option<i32>, height: Option<i32>, fps: Option<f64>, has_audio }
```

### 导入白名单

按映射到的 `ClipType` 分组的扩展名常量 + `importable_clip_type(path)` 判定（小写化扩展名）：

| 常量 | 扩展名 | → ClipType |
|---|---|---|
| `SUPPORTED_VIDEO_EXTENSIONS` | `mov` `mp4` `m4v` | `Video` |
| `SUPPORTED_AUDIO_EXTENSIONS` | `mp3` `wav` `aac` `m4a` | `Audio` |
| `SUPPORTED_IMAGE_EXTENSIONS` | `png` `jpg` `jpeg` `tiff` `heic` `webp` | `Image` |

> **JSON / Lottie 刻意排除**：Lottie 需内容嗅探，裸扩展名给不了，故 JSON 文件不在此自动导入（镜像上游 `ClipType(fileExtension:)` 减去 Lottie 特例）。

### `import_media_file`

镜像上游 `addMediaAsset` + `importMediaAsset` + `finalizeImportedAsset`：构造 `MediaAsset`（[`MediaSource::External`]——文件**在原地被引用，不拷进包**）→ 折入探测元数据 → 派生持久化 entry → 推入 `manifest.entries`。clip 层只存 asset id（`media_ref`），manifest 是 id→文件的桥。

`has_audio` 按类型修正：`Audio` 恒 true；`Video` 取 `probe.has_audio`；其余恒 false（图片即使探测谎报有音轨也置 false）。错误：扩展名不在白名单 → `CoreError::Unsupported("media")`（可恢复值，命令层映射成清晰消息，绝非 panic）。

> **关键不变量**：manifest 变更**刻意在撤销事务之外**——上游把导入直接追加 manifest，只文件夹移动（经 [`apply`](core-router.md)）可撤销。**导入不 bump timeline version。**

### `relink_media_file`

把已有 asset（按 id）重链到新磁盘文件，**保持同 id**，使每个引用它的 clip 在位恢复（镜像上游 `EditorViewModel+Relink.applyRelink`：同 asset、换 url、刷新元数据）：

- id 必须存在（否则 `CoreError::Media("unknown media asset: …")`）；
- 新文件类型必须匹配原 `kind`（否则 `CoreError::Media("cannot relink a … to a …")`——上游拒绝类型变更）；
- 只改 `source`（→ `External`）+ 探测元数据；面板从"文件存在性"派生的 `missing` 态在源指回真实文件后自动清除。

> **这修复的 bug**：直接 re-import 会铸**新** id，把旧 clip 永久孤立在缺失 entry 上。重链复用原 id 在位治愈。

## 读访问

`media()`（manifest 克隆）/ `media_entry(id)`（不克隆整 manifest 的查找）/ `version()` / `timeline()`（克隆，供只读镜像）/ `can_undo()` / `can_redo()` / `project_dir()` / `generation_log()`。

测试缝 `seed_from_timeline`（`#[cfg(test)]`）从手搭 timeline 重置可编辑态（空 manifest、version 0），让测试不经磁盘即可在自定 timeline 上站起会话，同时把生产态变更全漏斗进 `apply` / `open_project`。

## 错误

全部经 `crate::error::Result<T>`（= `Result<T, CoreError>`，见 [dto.md](dto.md)）：`opentake_ops::EditError` / `opentake_project::ProjectError` 经 `#[from]` 折叠，加装配级 `NoProjectOpen` / `Unsupported` / `Media`。

## 测试覆盖（本文件 `#[cfg(test)]`）

新工程空且 version 0；无路径/无目录 save 报 `NoProjectOpen`；new→save→open 往返保留 timeline 且重开 version 0、空历史；apply→undo→redo 经会话（version 1→2→3）；白名单判定（含大小写、拒 json/txt/无扩展名）；视频导入建 External entry 带探测元数据；图片恒无音轨；音频 has_audio=true；不支持扩展名报错且不动 manifest。

---

> 上级：本模块目录 [INDEX.md](INDEX.md) · 总览 [OVERVIEW.md](OVERVIEW.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
