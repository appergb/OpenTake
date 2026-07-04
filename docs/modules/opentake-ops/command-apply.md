# 子系统：命令与事务（command.rs + editor_state.rs）

> 上级：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)

## 职责

这是本模块的中枢：定义**唯一编辑入口** `EditCommand` 枚举与执行函数 `apply()`，把所有编辑收敛成一条 `withTimelineSwap` 式事务；并由 `EditorState` 持有可编辑文档与整树快照撤销/重做栈。撤销 / 校验 / 版本号因此只写一次。

源文件：
- `../../../crates/opentake-ops/src/command.rs`（`EditCommand` + `apply` + 各命令实现）
- `../../../crates/opentake-ops/src/editor_state.rs`（`EditorState` + `DocSnapshot` + 撤销栈）

## 关键类型 / 函数 / 算法

### EditCommand —— 统一编辑命令枚举

`EditCommand`（`command.rs`）是覆盖全部编辑表面的命令枚举。**注意：它是纯枚举，没有 serde derive**（见下「序列化陷阱」）。变体含：

- 放置 / 插入：`AddClips` / `AddClipsAutoTrack`（在按媒体类型新建的共享轨上放置）/ `InsertClips`（波纹插入）。
- 片段结构：`MoveClips` / `DuplicateClips`（Alt 拖拽深拷贝）/ `RemoveClips` / `SplitClip` / `TrimClips`。
- 属性：`SetClipProperties` / `SetColorGrade` / `SetChromaKey` / `SetMasks` / `SetEffects` / `SwapMedia`（原位换 `media_ref` 保留全部编辑属性）。
- 关键帧：`SetKeyframes` / `StampKeyframe` / `RemoveKeyframe` / `MoveKeyframe` / `SetKeyframeInterpolation`（公开 API 用**绝对时间线帧**）。
- 波纹删除：`RippleDeleteRanges`（按帧区间）/ `RippleDeleteClips`（按选中片段）。
- 轨道 / 文本 / 链接：`AddTexts` / `Link` / `Unlink` / `RemoveTracks` / `InsertTrack` / `SetTrackProps`（mute/hide/sync-lock 切换）。
- 文件夹 / 媒体库：`CreateFolder` / `MoveToFolder` / `RenameMedia` / `RenameFolder` / `DeleteMedia` / `DeleteFolder`。
- 历史：`Undo` / `Redo`。

辅助载荷类型：`ClipEntry`（→ `PlaceSpec`）、`RenameEntry`、`TextEntry`、`ClipProperties`（`None` 字段不变；设标量值会清掉对应关键帧轨）、`KeyframeProperty` / `KeyframePayload`。

### apply() —— 事务执行壳

`apply(state: &mut EditorState, command: EditCommand, ids: &dyn IdGen) -> Result<EditResult, EditError>`：把命令分派到各实现函数。除 `Undo`/`Redo` 外，每个实现都走 `transact()`：

```
fn transact(state, action_name, summarize, work):
    before  = state.snapshot()        // 整文档 Clone
    affected = work(state)?            // 跑纯函数变更；Err 直接传播，不提交
    after   = state.snapshot()
    changed = before != after          // PartialEq 短路
    if changed: state.commit(before)   // 推 before 入撤销栈、清 redo、version++
    return EditResult{ changed, action_name, affected_clip_ids, timeline_version, summary }
```

即上游 `withTimelineSwap` 的泛化：从「整 timeline 交换」扩到「整文档（timeline + manifest）交换」。

### EditResult / EditError

- `EditResult { changed, action_name, affected_clip_ids, timeline_version, summary }`：1:1 形态来自 ARCHITECTURE §5。`changed` 驱动前端是否需重取镜像；未变更的命令报告**先前**的 version。
- `EditError::Invalid(String)`：输入校验失败（坏索引 / 缺片段 / 空载荷）。
- `EditError::Refused(String)`：波纹拒绝（sync-lock 跟随轨无法吸收位移）。

### EditorState —— 文档 + 撤销栈

`EditorState`（`editor_state.rs`）：
- 字段：`timeline` / `manifest`（文件夹命令改的是 manifest 不是 timeline）/ `undo_stack` / `redo_stack` / `version`。
- `DocSnapshot { timeline, manifest }`：`apply` 能触及的一切的不可变快照，整棵 `Clone` + `PartialEq`。
- 查询：`version()` / `can_undo()` / `can_redo()` / `undo_depth()` / `find_clip(id) -> Option<ClipLocation>`（1:1 上游 `findClip`）/ `track_index(track_id)`。
- 事务内部 API（`pub(crate)`）：`snapshot` / `restore` / `commit` / `undo` / `redo`。

## 不变量与上游对齐

- **原子性**：`work` 返回 `Err` 时 `transact` 不调 `commit`，文档保持原样；波纹拒绝（`Err(Refused)`）等价于校验失败的「整次不改」。
- **commit-if-changed**：只有 `before != after` 才入栈 + `version++`。无实质变化的命令（如 `SwapMedia` 换到相同 `media_ref`）返回 `changed = false`、不污染撤销栈。
- **撤销 = 整树快照交换**：`commit(before)` 推 before 入 `undo_stack` 并**清空 `redo_stack`**（新编辑使 redo 失效）；`undo` 把当前推入 redo、还原栈顶；`redo` 反之。`version` 在提交、撤销、重做时都 +1（前端据此判失效）。对齐 ARCHITECTURE「撤销栈在 Rust、整树快照」。
- **pin-by-id**：放置类命令在 `clear_region`（可能 prune / 移位索引）后用 `track_index(track_id)` 重新定位轨道，避免索引失效。
- **关键帧绝对帧**：命令公开 API 用绝对时间线帧，内部转 clip 相对偏移（拆分逻辑在 domain）。
- 单次 rename（媒体 / 文件夹）= 一元素 vec，与批量同走一个撤销组（对齐上游 `withUndoGroup`）。

## ⚠️ 序列化陷阱（高频 bug 来源）

- `EditCommand` 是**纯枚举，无 serde derive**。
- IPC 层另有 serde DTO `EditRequest`（在 `../../../src-tauri/src/commands.rs`），用 `#[serde(tag = "type", rename_all = "camelCase")]`，由 Tauri 命令 `edit_apply` 映射成 `EditCommand`。
- 因此**多词字段在前端线上必须是 camelCase**（如 `atFrame` / `trackIndex`）。历史上「删除 / 分割 / Inspector 全静默失效」就是 DTO 的 camelCase 没对齐导致反序列化失败。改 IPC 字段时，Rust DTO、前端 `web/src/lib/types.ts` 的 `EditRequest`、调用处三边必须同步；IPC 内若静默吞错，先加 `try/catch` 把错误暴露出来。
- 编辑命令 / IPC 的完整规格见 [opentake-core 规格 SPEC](../opentake-core/SPEC.md)。

## 与其他子系统关系

- **调用 `ops/*`**：各命令实现（`add_clips` / `move_clips` / `split` / `trim` / `ripple_delete*` 等）在 `transact` 的 `work` 闭包里调用 `ops/` 的算法函数与 `intent`（见 [ops-algorithms.md](ops-algorithms.md)）。
- **re-export `engines`**：`RippleDeleteRanges` 直接收 `FrameRange`（来自 [engines.md](engines.md)）。
- **依赖 `IdGen`**：新实体 id 由注入的生成器铸造（见 [intent-id.md](intent-id.md)）。
- **被 `opentake-core` 装配**：core 把 `EditorState` 包进 `Arc<Mutex<…>>` 权威容器，对 UI/Agent/MCP 暴露唯一 `apply` 入口，并经版本号 + 事件广播推前端。

---

> 上级：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
