# opentake-core — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> `opentake-core` = **装配层（命令路由层）**：把 `opentake-{domain,ops,project}` + 注入式能力句柄装配成一个权威可观测的会话 `EditorSession`，对 UI / Agent / MCP 三客户端暴露唯一编辑入口 `AppCore::apply`，经版本号 + 事件广播驱动前端只读镜像。依赖只向下：依赖 `domain` / `ops` / `project`，被 `src-tauri` 调用（`opentake-agent` 也作为客户端持其句柄）。

---

## 总览

- **[OVERVIEW.md](OVERVIEW.md)** — 一句话定位与依赖分层位置、职责边界、关键概念与数据流（单一权威 + 命令事务、会话管理、依赖注入、事件总线 `timeline_changed`、命令路由 `AppCore::apply`）、对应上游 Swift（App + Editor + Project 的纯逻辑装配子集）、完成状态（已实现 vs 计划中）、移植铁律。

## 子系统文档

- **[session.md](session.md)** — `session.rs`：`EditorSession` 会话管理（按值持 `opentake_ops::EditorState` + `project_dir` + `GenerationLog`）、`new_project` / `open_project` / `save_project` 装配顺序、同步 `import_media_file` / `relink_media_file`（重链保持同 id 治愈在位 clip）、导入白名单。
- **[core-router.md](core-router.md)** — `core.rs`：`AppCore` 可克隆句柄（`Arc<Mutex<EditorSession>>` + 事件总线 + 注入 deps + id 生成器）、唯一编辑入口 `AppCore::apply` 命令路由（取锁 → 透传 ops 事务 → **锁释放后**广播）、`undo`/`redo` = `EditCommand::Undo`/`Redo`、并发串行化使 version 单调、`CoreIdGen`。
- **[deps-di.md](deps-di.md)** — `deps.rs`：`CoreDeps` 四个注入式能力 trait（`PreviewBackend` / `ExportBackend` / `MediaImporter` / `GenBackend`）+ `UnsupportedBackends` 占位（返回 `CoreError::Unsupported` 而非 panic 的纪律）；与会话内同步媒体导入的区别。
- **[events-bus.md](events-bus.md)** — `events.rs`：`CoreEvent`（`TimelineChanged` / `ProjectOpened` / `ProjectSaved` / `MediaChanged`）+ `EventBus`（回调 `Vec` 同步扇出，零运行时依赖，替代上游 SwiftUI `@Observable`）；`kind` 标签 + camelCase 序列化；订阅/退订。
- **[dto.md](dto.md)** — `dto.rs`：Tauri 边界 DTO（`TimelineSnapshotDto` / `EditResultDto` / `CmdError`，全 camelCase，**无 tauri 依赖**）+ `handle_*` handler 函数（`src-tauri` 用一行 `#[tauri::command]` 包住）；`error.rs`：`CoreError` 折叠下层错误 + `code()` 分类（`validation` / `internal`）。

## 规格

- **[SPEC.md](SPEC.md)** — Issue #11 实现就绪规格（core + ops 范围）：`EditorState` 结构、`EditCommand` 与事务、IPC 边界契约、撤销模型、前端镜像 + 版本号同步协议、安全与并发边界、Tauri 命令表面草案、实施清单。**注意**：SPEC 为草案，部分命名/结构与代码不一致——SPEC 写的 `EditorCore` / `opentake_ops::UndoStack` 在实际代码中分别是 `AppCore` 与 `opentake_ops::EditorState` 内置的撤销栈；**以代码（及本目录子系统文档）为准**。

## 相关跨切面（架构）

- [ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) — 总体架构：单一真理状态 + 命令事务（§5）、真相源在 Rust / 前端持镜像 + 版本号（§2）、`.opentake` 包结构（§9）。
- [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md) — 逐模块上游 Swift → Rust 移植地图（本 crate 对应上游 `App`（AppState / 生命周期 / MCPService 装配）、`Editor`（EditorViewModel 的状态+事务子集）、`Project`（NSDocument 读写）的纯逻辑装配子集）。
- [ROADMAP.md](../../architecture/ROADMAP.md) — 分阶段路线图（本 crate 横跨 Phase 1 命令路由/事务，与 Phase 6/7 Tauri 边界 + 事件桥 + Agent 接入）。

## 相关模块（交叉链）

- [opentake-ops](../opentake-ops/INDEX.md) — **编辑引擎**：`EditCommand` / `apply` 事务 / `EditorState`（含整树快照撤销栈与 `version`）的真正定义处；core 仅 re-export 并经 `EditorSession` 透传，**不重定义、不重实现**。
- [opentake-domain](../opentake-domain/INDEX.md) — 提供 `Timeline` / `MediaManifest` / `MediaManifestEntry` / `MediaAsset` / `MediaSource` / `ClipType` 等值类型；core 的快照 DTO 直接复用其 serde（= `project.json` schema）。
- [opentake-project](../opentake-project/INDEX.md) — `.opentake` 包读写（`Project::open` / `save`）+ `GenerationLog`；core 在 `open_project` / `save_project` 中编排它。
- [src-tauri](../src-tauri/INDEX.md) — 用薄 `#[tauri::command]` 包住本 crate 的 `handle_*`，并起事件桥 task 把 `CoreEvent` 转成前端 `emit`（暂缺也照写）。
- [opentake-agent](../opentake-agent/INDEX.md) — 作为 core 的客户端持 `AppCore` 句柄，把工具 args 翻译成 `EditCommand` → `apply`；助手专属 undo 游标 `AgentUndoCursor` 在 agent 层（不在 core）。

## 源码

```
crates/opentake-core/src/
├── lib.rs       模块声明 + 公开 API re-export（含从 opentake-ops re-export 的 EditCommand/EditResult/EditError/EditorState）
├── core.rs      AppCore（Clone 句柄）+ apply 命令路由 + undo/redo + 工程生命周期 + 媒体导入 + CoreIdGen + TimelineSnapshot
├── session.rs   EditorSession（持 EditorState + project_dir + GenerationLog）+ open/save 顺序 + import/relink + 导入白名单 + ProbedMedia
├── deps.rs      CoreDeps + 四个能力 trait（PreviewBackend/ExportBackend/MediaImporter/GenBackend）+ UnsupportedBackends 占位
├── events.rs    CoreEvent 枚举 + EventBus（回调 Vec 同步扇出）+ SubscriptionId
├── dto.rs       Tauri 边界 DTO（TimelineSnapshotDto/EditResultDto/CmdError）+ handle_* handler（无 tauri 依赖）
└── error.rs     CoreError（thiserror，折叠 EditError/ProjectError + 装配级错误）+ code() 分类 + Result 别名
```

源文件树根：[`../../../crates/opentake-core/src/`](../../../crates/opentake-core/src/)

---

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
