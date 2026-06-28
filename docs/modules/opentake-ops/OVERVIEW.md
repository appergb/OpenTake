# opentake-ops 总览

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md) · 本模块目录：[INDEX.md](INDEX.md)

## 一句话定位

`opentake-ops` 是 OpenTake 的**纯编辑引擎 + 命令事务层**：它持有唯一的编辑入口 `EditCommand`，把所有 UI 手势 / Agent / MCP 工具归一成一条 `apply()` 事务，内含整树快照撤销/重做栈与三大无副作用引擎（Overwrite / Ripple / Snap）。

### 依赖分层位置

```
opentake-domain        值语义叶子层（Timeline/Track/Clip/Keyframe）
   ▲
opentake-ops    ★本模块  纯引擎 + EditCommand + apply 事务 + 撤销栈
   ▲
opentake-core          会话 / DI / 事件总线（命令路由层，调用本模块）
   ▲
src-tauri / web        Tauri 壳 + React 只读镜像
```

依赖**只向下**：本 crate 只依赖 `opentake-domain`，是一个零业务外部依赖的叶子级引擎（连 `uuid` 都不引，见 [intent-id.md](intent-id.md)）。它被 `opentake-core` 装配进权威状态容器后，再经 `src-tauri` 的 `edit_apply` 命令暴露给前端。

## 职责边界

**做：**
- 定义统一编辑命令枚举 `EditCommand`，与执行函数 `apply()`（命令事务模型）。
- 持有可编辑文档 `EditorState`（`Timeline` + `MediaManifest`）及整树快照撤销/重做栈、单调版本号。
- 三个纯函数引擎：覆盖清区（Overwrite）、波纹位移（Ripple）、拖拽吸附（Snap）。
- 各编辑算法（放置 / 分割 / 修剪 / 移动 / 复制 / 波纹删插 / 链接 / 建删轨 / 文件夹），逐个 1:1 移植上游 `EditorViewModel` 的纯逻辑部分。
- 高层编辑意图的预检与归一（`intent.rs`：自动建轨、卡点放置、修剪到播放头等）。

**不做：**
- **不碰 I/O**：无 `std::fs`、无网络、无媒体解码（FFmpeg / 缩略图 / 波形归 `opentake-media`）。
- **不做媒体感知**：不解析素材尺寸 / 时长，放置时的视觉 `Transform` 由上层传入（见 [ops-algorithms.md](ops-algorithms.md) 的 place 说明）。
- **不做序列化**：`EditCommand` 是纯枚举，**无 serde derive**；IPC 的 DTO 在 `src-tauri`（见下文「序列化陷阱」）。
- **不做帧↔秒换算**：本层一切以**整数帧**为单位，秒↔帧换算归调用方。
- **不持 UI 瞬态**：选区 / 缩放 / 面板可见性等归前端 Zustand；本层只持可序列化真相 + 撤销基础设施。
- **不触发平台反馈**：吸附的触觉反馈、波纹拒绝的提示音由 UI 层根据返回值自行触发。

## 关键概念与数据流

### 唯一编辑入口：EditCommand → apply() 事务

所有编辑都收敛到一条路径，撤销 / 校验 / 版本号因此只写一次：

```
UI 手势 / Agent / MCP 工具
  → （src-tauri）EditRequest DTO → 映射成 EditCommand
  → opentake-ops::apply(state, command, ids)
       1. snapshot   ：克隆整个文档（timeline + manifest）
       2. mutate     ：跑命令的纯函数变更（校验失败 → Err，文档不动）
       3. commit-if-changed：before != after（PartialEq 短路）才推快照入撤销栈 + version++
       4. → EditResult{ changed, action_name, affected_clip_ids, timeline_version, summary }
  → 前端据 version 失效并重取只读镜像
```

这就是上游 `withTimelineSwap` 事务的泛化（从「整 timeline 交换」扩到「整文档交换」）。实现见 `command.rs` 的 `transact()`，撤销栈见 `editor_state.rs`。详见 [command-apply.md](command-apply.md)。

### 撤销栈：整树 Clone 快照

撤销模型是**整文档值快照**，不是逆操作 / diff：`DocSnapshot { timeline, manifest }` 整棵 `Clone`，`commit` 推入 `undo_stack` 并清空 `redo_stack`，`undo`/`redo` 互相倒栈。`version` 在每次提交、每次撤销/重做时都 +1。用内存换实现简单与正确性，对齐 ARCHITECTURE「撤销栈在 Rust、整树快照」的决策。

### 原子性与不变量（贯穿全模块）

- **原子性**：校验失败（`EditError::Invalid`）或波纹拒绝（`EditError::Refused`）时，`apply` 直接返回 `Err`，事务不提交，**文档保持原样**。
- **视频轨在音频轨之上的分区不变量**：可视轨（video/image/text/lottie）恒占 `[0, first_audio_index)`，音频轨占 `[first_audio_index, count)`；建轨索引被钳进各自分区（`tracks.rs` 的 `partitioned_insertion_index`）。
- **链接音视频组同步**：共享 `link_group_id` 的片段在移动 / 修剪 / 分割 / 删除时作为一个整体联动（`linking.rs`）。
- **sync-lock 跨轨联动与拒绝**：同步锁轨随波纹整体位移；若某跟随轨片段会移过帧 0 或与相邻片段碰撞，则整次波纹**拒绝**、不改任何状态。

## 对应上游 Swift 模块

对照 [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md)（上游路径 `palmier-pro-upstream/Sources/PalmierPro/`）：

| 本模块 | 上游 Swift |
|---|---|
| `engines/overwrite.rs` → `OverwriteEngine` | `Editor/OverwriteEngine.swift`（`computeOverwrite`） |
| `engines/ripple.rs` → `RippleEngine` | `Editor/RippleEngine.swift`（`computeRippleShifts*` / `computeRipplePush` / `mergeRanges`） |
| `engines/snap.rs` → `SnapEngine` | `Timeline/SnapEngine.swift`（`collectTargets` / `findSnap` / `SnapState`） |
| `command.rs` 的 `apply()` 事务 | `EditorViewModel+ClipMutations.swift` 的 `withTimelineSwap` / `registerTimelineSwap` |
| `command.rs` 的 `EditCommand` 入口 | `Agent/Tools/ToolExecutor`（统一执行壳） |
| `ops/*`（place/split/trim/move/ripple/link/tracks/folders） | `EditorViewModel` 及其 `+ClipMutations` / `+Ripple` / `+Linking` / `+Tracks` / `+Folders` 扩展的**纯逻辑部分**（剥离 AppKit / UndoManager 胶水） |

移植中**剥离**的上游部分：`NSHapticFeedbackManager`（吸附触觉）、`NSSound.beep`（拒绝提示音）、`UndoManager` 闭包注册三套策略（统一收敛为整树快照栈）、`UUID().uuidString` 内联（改为注入式 `IdGen`）。

## 完成状态：已实现 vs 计划中

对照 [ROADMAP.md](../../architecture/ROADMAP.md)、[EDITING-ENGINE-PLAN.md](../../architecture/EDITING-ENGINE-PLAN.md)、[PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) 与代码现状：

**已实现（代码中存在且带单测）：**
- 三大纯引擎 Overwrite / Ripple / Snap，逐个对齐上游数值（含 sticky 滞回、播放头优先、多探针）。
- `EditCommand` + `apply()` 事务（snapshot → mutate → commit-if-changed → version++）。
- 整树快照撤销/重做栈（`EditorState`，含 `version` / `can_undo` / `can_redo`）。
- 各 ops 算法：place（含链接音频）、split（速度感知 source 重分配 + 关键帧边界拆 + 链接重组）、trim（source delta → timeline delta 经 `round(delta/speed)`）、move（先拔再写 + clearRegion + pin-by-id + prune）、ripple delete/ranges/insert（含拒绝语义）、link/unlink、duplicate（Alt 拖拽深拷贝 + 链接重映射）、tracks（分区建删 / prune / 音轨解析）、folders（建 / 移 / 改名 / 级联删除）。
- 命令层属性类操作：`SetClipProperties` / 关键帧族（`SetKeyframes` / `StampKeyframe` / `RemoveKeyframe` / `MoveKeyframe` / `SetKeyframeInterpolation`）/ `SetColorGrade` / `SetChromaKey` / `SetMasks` / `SetEffects` / `SetTrackProps` / `SwapMedia`。
- `intent.rs` 高层意图预检：自动建轨放置、卡点放置、修剪到播放头、单区间波纹删除、smart-reframe。

**计划中（仅 ROADMAP / GAP 规划，本 crate 代码尚未落地）：**
- 与上游 1:1 的若干接线层 / 模型扩展缺口主要在**前端与 domain**，不在本 crate（见 [EDITING-ENGINE-PLAN.md](../../architecture/EDITING-ENGINE-PLAN.md) §3）：如 fade knee 拖拽态、隐藏轨 hitTest 过滤、`Clip.isSoloed` 字段（需前后端 DTO 扩展）、轨间插入阈值 `insertThreshold`、Snap 容差按 DPI 缩放。
- 曲线变速（speed 升级为关键帧轨）、复合片段嵌套等属 ROADMAP 后期能力，本 crate 当前无对应命令。

> 结论：剪辑「算法核」（`ops/*` + `engines/*`）已基本 1:1 写通；真正的待收口集中在前端接线与 domain 字段扩展。**不要重写算法核。**

## 移植铁律（Swift → Rust，本模块强约束）

对照上游复刻算法时务必逐处对齐，否则跨片段会累积帧漂移：

- **一切以整数帧为单位**；秒↔帧换算不在本层发生。
- `secondsToFrame` 用**截断** `Int(s*fps)`，不是四舍五入（本层不做，但下游约定一致）。
- `round()` 方向：Swift `.rounded()` = Rust `f64::round()`，即 **half-away-from-zero**（.5 远离零）。source↔timeline 帧折算（trim / split / overwrite 的 `round(delta*speed)` 与 `round(delta/speed)`）全部用它。
- **关键帧存储用 clip 相对帧偏移**，公开 API 用绝对时间线帧；分割时插边界关键帧保曲线连续（实际拆分逻辑在 `opentake-domain`）。
- `smoothstep(t) = t*t*(3-2t)`，不换公式（采样在 domain，本层只调用）。
- image / text 片段 trim 可为负（无源材料约束）；video / audio 钳制移动边在 0。
- 所有 serde 模型加 `#[serde(default)]` + `Option<T>` 保旧工程兼容——这是 domain 的约束，本层不序列化但所操作的类型遵守它。

---

> 本模块目录：[INDEX.md](INDEX.md) · 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
