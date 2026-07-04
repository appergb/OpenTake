# commands-ipc — 命令边界层与 IPC 序列化

> 上级：[本模块目录](INDEX.md) · [总览](OVERVIEW.md) · [模块文档树](../INDEX.md)
>
> 源码：[`../../../src-tauri/src/commands.rs`](../../../src-tauri/src/commands.rs)

## 定位

这是前端唯一的写 / 读 / 生命周期入口。每个 `#[tauri::command]` 都是薄转接层：自身不持锁，委派给 `opentake_core::dto::handle_*`（内部包 `AppCore`），并把边界错误 `CmdError` 映射成 `String`，让前端拿到一个普通的 rejected Promise（`AGENTS.md`：「边界层转 Tauri 的 `Err(String)`」）。

## 命令清单（commands.rs）

### 读 / 生命周期（直接 DTO 透传）

| 命令 | 入参 | 返回 | 委派到 |
|---|---|---|---|
| `get_timeline` | — | `TimelineSnapshotDto`（`{ timeline, version }`） | `handle_get_timeline`，**不可失败** |
| `undo` / `redo` | — | `EditResultDto` | `handle_undo` / `handle_redo` |
| `can_undo` / `can_redo` | — | `bool` | `core.can_undo()` / `can_redo()`（驱动工具栏可用态） |
| `project_new` | — | — | `handle_project_new`（替换为全新未存工程） |
| `project_open` | `path` | `TimelineSnapshotDto` | `handle_project_open`（开 `.opentake` bundle） |
| `project_save` | `path?` | `String`（写入路径） | `handle_project_save`（`None`=存回原 bundle，`Some`=另存为） |
| `get_default_project_dir` | — | `String` | `~/Documents/OpenTake`（首次用时创建）；对应上游 `Project.storageDirectory` |
| `export_fcpxml` | `path` | — | 见下「命名陷阱」 |
| `check_path_exists` | `path` | `bool` | `Path::exists()` |

### 唯一编辑入口

| 命令 | 入参 | 返回 |
|---|---|---|
| `edit_apply` | `command: EditRequest` | `EditResultDto` |

`edit_apply` 把前端构造的 `EditRequest` 经 `into_command()` 映射成 `EditCommand`，再交 `AppCore::apply`（执行快照 / 提交 / version++ 事务，并发 `TimelineChanged`）。

> 其余命令（媒体 / 渲染 / 导出 / 密钥 / 库）定义在各自模块文件里，但同样在 `lib.rs` 的 `generate_handler!` 注册。见 [setup-lib.md](setup-lib.md) 的完整注册表。

## IPC 序列化陷阱（核心）

### 为什么需要 `EditRequest`

`opentake_ops::EditCommand` 携带引擎值类型（`ClipMove`、`TrimEdit`、`KeyframeTrack`…），**没有 `Deserialize`**。所以 IPC 层另立一个 serde 友好的镜像枚举 `EditRequest`，1:1 对应前端 v1 发出的全部变体；引擎值类型也各有本地 serde DTO（`ClipMoveDto`、`TrimEditDto`、`FrameRangeDto`、`ClipPropertiesDto`、`KeyframePayloadDto`…），在 `into_command()` 里转换。

### 三条铁律

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]   // ← 枚举级：变体名 addClips/removeClips…
pub enum EditRequest {
    #[serde(rename_all = "camelCase")]              // ← 变体级：必须再写一遍！
    RemoveClips { clip_ids: Vec<String> },          //    否则线上的 clipIds 反序列化失败
    #[serde(rename_all = "camelCase")]
    SplitClip { clip_id: String, at_frame: i32 },
    …
}
```

1. **多词字段线上一律 camelCase**：`atFrame` / `trackIndex` / `clipIds` / `clipId` / `mediaRef` / `fromFrame` / `toFrame` / `assetIds` / `folderIds`…
2. **枚举级 `rename_all` 不会下传到结构体变体的字段**——每个变体都要再加 `#[serde(rename_all = "camelCase")]`。漏掉 → `missing field clip_ids` → 该命令静默失效。**历史真实事故**：删除 / 分割 / Inspector 全挂，根因正是变体级 camelCase 未对齐。`commands.rs` 末尾的 `deserializes_camelcase_multiword_commands` 测试就是这条的回归守卫。
3. **改字段要三边同步**：
   - Rust DTO：[`../../../src-tauri/src/commands.rs`](../../../src-tauri/src/commands.rs) 的 `EditRequest` 及对应 `*Dto`；
   - 前端类型：`web/src/lib/types.ts` 的 `EditRequest` 判别联合；
   - 调用处：`web/src/lib/api.ts` 的 `editApply()`。
   - IPC 内若静默吞错，**先加 `try/catch` 把错误暴露出来**再排查。

### `EditRequest` 覆盖的变体（节选）

片段：`AddClips` / `InsertClips` / `MoveClips` / `DuplicateClips` / `RemoveClips` / `SplitClip` / `TrimClips` / `SetClipProperties` / `SwapMedia`
关键帧：`SetKeyframes` / `StampKeyframe` / `RemoveKeyframe` / `MoveKeyframe` / `SetKeyframeInterpolation`
效果：`SetColorGrade` / `SetChromaKey` / `SetMasks` / `SetEffects`
波纹：`RippleDeleteRanges` / `RippleDeleteClips`
文本 / 链接 / 轨道：`AddTexts` / `Link` / `Unlink` / `RemoveTracks` / `InsertTrack` / `SetTrackProps`
素材库（领域内）：`CreateFolder` / `MoveToFolder` / `RenameMedia` / `RenameFolder` / `DeleteMedia` / `DeleteFolder`

> 注意区分：上面这些「素材库领域命令」走 `EditCommand`（进时间线事务、可撤销）；而**全局跨工程素材库**是另一套独立命令（`library_*`），见 [library-media.md](library-media.md)。

### 关键帧载荷的二级 tag

`KeyframePayloadDto` 用 `#[serde(tag = "kind", rename_all = "camelCase")]` 再分 `Scalar` / `Pair` / `Crop` 三种轨；每个关键帧 `{ frame, value, interpolationOut? }`，`interpolation_out` 缺省时落到 `Keyframe::new`（默认插值），否则 `Keyframe::with_interpolation`。

## 命名陷阱：`export_fcpxml` 其实导出 XMEML

命令名叫 `export_fcpxml`，但产出的是 **XMEML 4（Final Cut Pro 7 XML，`.xml`）**，不是 FCPXML。原因：Premiere Pro 不原生读 FCPXML，上游遂导出 XMEML；DaVinci / FCP 仍能导入 FCP7 XML。实现读 core 的 timeline / media manifest / project dir，调纯函数 `opentake_project::export_xmeml` 生成 XML 后写盘。这与 [export.md](export.md) 的 `export_video`（真实视频文件）是**两条不同的导出路径**。

## 错误约定

`fn msg(e: CmdError) -> String { e.message }`——边界只把内部错误的 message 字段透给前端。不可失败的命令（`get_timeline` / `can_undo` 等）直接返回值类型，不包 `Result`。

---

> 相关：[setup-lib.md](setup-lib.md)（命令注册 + 状态装配）· [export.md](export.md)（`export_video` 对比）· 跨模块 [opentake-core](../opentake-core/INDEX.md)（`AppCore::apply` 事务）/ [opentake-ops](../opentake-ops/INDEX.md)（`EditCommand` 定义）
>
> 导航：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
