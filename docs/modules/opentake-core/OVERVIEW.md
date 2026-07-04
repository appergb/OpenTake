# opentake-core 总览

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md) · 本模块目录：[INDEX.md](INDEX.md)

## 一句话定位

`opentake-core` 是 OpenTake 的**装配层（命令路由层）**：它把 `opentake-{domain,ops,project}`（外加经注入句柄的 render/media/gen 能力层）装配成**一个权威、可观测的编辑会话 `EditorSession`**，对 UI / 内置 Agent / MCP 三个对等客户端暴露**唯一一条编辑入口** `AppCore::apply`，并把状态变更通过**单调递增版本号 + 事件广播**推给观察者（前端镜像据此重取）。

它**不是编辑层**：不含帧算术、不含重叠求解、自身也不实现事务逻辑——这些全在 `opentake-ops`；它只负责装配、路由、序列化、广播。

### 依赖分层位置

```
opentake-domain        值语义叶子层（Timeline/Track/Clip/MediaManifest…）
   ▲
opentake-ops           纯引擎 + EditCommand + apply 事务 + EditorState（含撤销/版本）
   ▲
opentake-project       工程持久化（.opentake 包读写 + GenerationLog）
   ▲
opentake-core   ★本模块  会话装配 / 依赖注入 / 事件总线 / DTO / 命令路由
   ▲
src-tauri              Tauri 壳：薄 #[tauri::command] 包住本 crate 的 handler + 事件桥
   ▲
web                   React/TS 前端（只读镜像 + 版本号同步）
```

依赖**只向下**：core 依赖 `domain`（类型）、`ops`（`EditCommand` + `apply` + `EditorState`）、`project`（读写 + `GenerationLog`）。能力层（render / media / gen）**不直接 `use`**，而是经 [`CoreDeps`](deps-di.md) 的 trait 句柄注入，使 core 与尚未完成的重栈解耦、可单测。

> **反向依赖（重要）**：`opentake-agent` 依赖 core（agent 是 core 的客户端，持 `AppCore` 句柄把工具翻译成 `EditCommand`）；**core 不依赖 agent**，否则成环。MCP server / LLM 客户端属 agent crate，core 无端口、无网络、无外部输入面。

## 职责边界

**做：**
- 装配权威会话 `EditorSession`：持 `opentake_ops::EditorState`（timeline + manifest + 撤销/重做 + version）+ `.opentake` 包路径 + `GenerationLog`（见 [session.md](session.md)）。
- 提供并发、可观测的句柄 `AppCore`：`Arc<Mutex<EditorSession>>` + 事件总线 + 注入 deps + id 生成器（见 [core-router.md](core-router.md)）。
- **唯一编辑入口** `AppCore::apply(EditCommand)`：在锁内调 `opentake-ops` 事务，提交后（**锁释放后**）广播变更事件。撤销/重做即 `EditCommand::Undo`/`Redo`，复用同一路径。
- 工程生命周期：`new_project` / `open_project` / `save_project`（编排 `opentake-project`，照搬上游 `VideoProject` 装配顺序）。
- 媒体清单：同步的 `import_media_file` / `relink_media_file`（调用方供探测元数据），读 `media()` / `project_dir()`。
- 事件总线 `EventBus` + `CoreEvent`：跨进程替代上游 SwiftUI `@Observable`（见 [events-bus.md](events-bus.md)）。
- Tauri 边界契约：`dto.rs` 的 DTO + `handle_*` 函数（**不依赖 tauri**），`CoreError` 折叠下层错误（见 [dto.md](dto.md)）。

**不做：**
- **不含编辑算法**：overlap / ripple / split / keyframe 求解全在 `opentake-ops`；core 只调 `command::apply`。
- **不含事务/撤销实现**：snapshot → commit-if-changed → version++ 与整树快照撤销栈都在 `opentake_ops::EditorState`；core 仅经 `EditorSession::apply` 透传。
- **不持 UI 瞬态**：selection / zoom / 面板可见性 / scrubbing 归前端 Zustand。
- **不碰网络**：MCP server / LLM 客户端在 `opentake-agent`。
- **不碰像素/解码/编码**：预览 / 导出 / 媒体探测 / 生成全在 `opentake-render` / `opentake-media` / `opentake-gen`，经 `CoreDeps` 注入。
- **不重定义编辑命令**：`EditCommand` / `EditResult` / `EditError` 由 `opentake-ops` 定义，core 仅 re-export（下游只依赖 core 即可驱动编辑器）。

## 关键概念与数据流

### 1. 单一权威 + 命令事务（跨进程边界）

上游是**单进程单实例**：SwiftUI、内置 chat、MCP server 共享同一个 `EditorViewModel` 引用，靠 `@Observable` 自动重渲。OpenTake 跨**逻辑进程边界**（core 在 Rust，UI 在 WebView），所以：

- Rust 侧**单一持有**权威 `EditorSession`（内含权威 `Timeline`）；
- 前端**不能**持权威 timeline，只持**只读镜像 + 版本号**；
- `AppCore` 是 `Clone` 句柄，克隆只复制 `Arc`，三客户端各持一份**指向同一 `Mutex<EditorSession>`** 的句柄——这是上游「三客户端共享一个 view model」在跨线程下的等价物。

```
UI 手势 / 内置 Agent / MCP 工具
  → 构造 EditCommand
  → AppCore::apply(cmd)           [本 crate：取锁 → 透传 → 释放锁 → 广播]
       └ EditorSession::apply()   [本 crate：薄包装]
            └ opentake_ops::command::apply(&mut state, cmd, ids)   [ops：事务本体]
                 snapshot → 纯函数变更（校验失败则 Err，文档不动）
                 → before != after 才推快照入撤销栈 + version++
                 → EditResult{ changed, action_name, affected_clip_ids, timeline_version, summary }
  → result.changed 为真 → events.emit(TimelineChanged{version})
  → 前端收到事件，若 version 更高则 get_timeline 重取镜像
```

`AppCore` 在 `EditorSession` 之上**只多两件事**（见 [core-router.md](core-router.md)）：
1. **串行化所有变更**（一把 `Mutex`），使 `version` 在并发客户端下严格单调；
2. **变更广播**：committing 的 edit / undo / redo 之后、**锁释放后**发 `CoreEvent::TimelineChanged`，订阅者据此重取镜像（锁外发事件，使订阅回调可安全重入 core 而不死锁）。

### 2. 会话管理（EditorSession）

`EditorSession` 是装配层的**数据半边**：它**不复制** `EditorState` 的任何能力，而是按值持有 `EditorState` 并把每次编辑委派给 `opentake_ops::command::apply`。它只补 `EditorState` 刻意省略（持久化无关）的两块工程级状态：

- `project_dir`：`.opentake` 包路径，使无参 save 知道写哪（上游 `EditorViewModel.projectURL`）；
- `generation_log`：append-only AI 审计日志，持久化为 `generation-log.json`（类型在 `opentake-project`，非 `opentake-domain`）。

`version` 直接来自 `EditorState`，**不是重复计数器**。媒体导入/重链直接改 manifest（**在撤销事务之外**，照搬上游：仅文件夹移动可撤销），不 bump timeline 版本。详见 [session.md](session.md)。

### 3. 依赖注入（CoreDeps）

core 编排但不实现的能力（preview / export / media import / generation）以**注入式 trait 对象**而非硬 `use` 具体函数的形式存在。它们是后续阶段的接缝。在那些 crate 落地前，core 内置 [`UnsupportedBackends`]——所有方法返回 `CoreError::Unsupported`（一个**真实、可恢复的错误值，绝非 panic**），保证整 crate 可编译、每条路径可被测试触发。详见 [deps-di.md](deps-di.md)。

> **注意区分两条媒体路径**：`EditorSession::import_media_file`（同步、由 `src-tauri` 探测后传值、单测无需 ffprobe）与 `CoreDeps::media: MediaImporter`（异步能力后端，含缩略图/波形，后续阶段接 `opentake-media`）是**两回事**——前者已实现，后者仍是接缝。

### 4. 事件总线（timeline_changed 等）

跨进程下变更信号必须显式化。`EventBus` 是一个 `Vec<callback>` 置于 `Mutex` 之后的**同步扇出**（**零运行时依赖**，无订阅者即 no-op、永不 panic），`AppCore` 的每次 committing 变更经它发 `CoreEvent`。`src-tauri` 的桥接订阅者只是把 `CoreEvent` 转成 `app_handle.emit(...)` 发给前端。

当前实现的事件（`CoreEvent`，内部 `kind` 标签序列化）：

| 变体 | 触发 | 前端用途 |
|---|---|---|
| `TimelineChanged { version }` | committing 的 edit / undo / redo | `version` 更高则 `get_timeline` 重取只读镜像 |
| `ProjectOpened { path, version }` | `new_project`（path 空）/ `open_project` | 打开后前端自取首个快照（open **不**发 `TimelineChanged`） |
| `ProjectSaved { path }` | `save_project` 成功 | 提示已保存 / 更新窗口标题 |
| `MediaChanged { count }` | `import_media_file` / `relink_media_file` 成功 | 经 `get_media` 重取媒体面板目录 |

详见 [events-bus.md](events-bus.md)。

### 5. 命令路由 = 上游单一能力层

上游的"单一能力层"是 `ToolExecutor.run` 里那张 `switch tool`（每个 case 调一个 `EditorViewModel` mutator），撤销/校验只写一遍发生在 `ToolExecutor.execute` 壳 + `EditorViewModel.withTimelineSwap` 事务两层。OpenTake 把这两层**显式化、下沉到 `opentake-ops`**（`EditCommand` + `apply`），core 只做"拿命令 → 起锁 → 透传 → 广播"。三客户端因此共享：同一份权威 timeline + 同一个 version 序列 + 同一个全局撤销栈。详见 [core-router.md](core-router.md)。

## 对应上游 Swift 模块

对照 [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md)（上游路径 `palmier-pro-upstream/Sources/PalmierPro/`）。`opentake-core` 是**跨多个上游模块的纯逻辑装配子集**——它把上游 `App`（生命周期/装配）、`Editor`（view model 的状态+事务子集）、`Project`（NSDocument 读写）的**逻辑核**收敛到一个跨进程装配层：

| 本模块 | 上游 Swift | 说明 |
|---|---|---|
| `AppCore`（`core.rs`） | `App/AppState.swift`（`activeProject` / `editorProvider` 闭包）+ `Editor/ViewModel/EditorViewModel.swift`（@Observable 中枢） | 上游单进程共享一个 view model；core 跨线程退化为「克隆 `AppCore`」 |
| `AppCore::apply`（命令路由） | `Agent/Tools/ToolExecutor`（`run` switch + `execute` 壳）+ `EditorViewModel+ClipMutations.swift` 的 `withTimelineSwap` | 上游隐式工具链 → OpenTake 显式 `EditCommand`，事务本体已下沉 ops |
| `EditorSession`（`session.rs`） | `EditorViewModel` 的「持久化 + 撤销 + 版本」子集 + `projectURL` / `generationLog` | 剥离全部 UI-only 瞬态（selection/zoom/scrub） |
| `open_project` / `save_project` | `Project/VideoProject.swift`（`read` / `captureSaveSnapshot` / `fileWrapper` / `makeWindowControllers` 装配顺序） | NSDocument 读写生命周期的纯逻辑部分 |
| `import_media_file` / `relink_media_file` | `EditorViewModel` 的 `addMediaAsset` / `importMediaAsset` / `+Relink.applyRelink` | 重链保持同 id 治愈在位的 clip（修复 re-import 造孤儿的 bug） |
| `EventBus` / `CoreEvent`（`events.rs`） | SwiftUI `@Observable`（`EditorViewModel` 自动传播）+ `AppState` 通知流（`mediaPanelRevealAssetId`） | 跨进程把"自动传播"显式化为单向事件 |
| `CoreDeps`（`deps.rs`） | `AppState` 注入 `MCPService` / 各 service 的装配点 | trait 注入替代单进程直接持引用 |
| `CoreError`（`error.rs`） | 上游分散的 `fileReadCorruptFile` / 工具校验错误 / `formatDecodingError` | 折叠为统一边界错误 + `code()` 分类 |

> **未由本 crate 承接的上游 `App`/`Project` 部分**：`AppDelegate` / 窗口编排 / 主菜单与快捷键 / Sparkle 更新 / changelog / 系统通知 / 缩略图生成 / `ProjectRegistry` / `SampleProjectService` —— 归 `src-tauri`、前端，或 `opentake-project`，或属计划中。core 只承接「会话装配 + 命令路由 + 工程读写编排 + 事件」这一逻辑核。

## 完成状态：已实现 vs 计划中

对照代码现状（每个源文件都带 `#[cfg(test)]` 测试模块）与 [ROADMAP.md](../../architecture/ROADMAP.md)、[SPEC.md](SPEC.md)：

**已实现（代码中存在且带单测）：**
- `AppCore`：`Clone` 句柄、`Arc<Mutex<EditorSession>>`、`Send + Sync`（带编译期断言）、可换的线程安全 id 生成器 `CoreIdGen`（原子计数，避开 `uuid` 依赖；生产可经 `set_id_gen` 注入 UUID 版）。
- 唯一编辑入口 `AppCore::apply` + `undo` / `redo`（= `EditCommand::Undo`/`Redo`），committing 才 bump version 并发**恰好一次** `TimelineChanged`；无变更/被拒命令不发事件、不动 version。
- 读 API：`get_timeline`（带 version 的快照）、`version` / `can_undo` / `can_redo`、`media` / `project_dir`。
- 工程生命周期：`new_project` / `open_project` / `save_project`（编排 `opentake-project`，open 后 version 归 0、不发 `TimelineChanged`；save 支持 autosave 与另存为），均发对应生命周期事件。
- 媒体：同步 `import_media_file`（扩展名白名单 + 探测元数据 → manifest entry，**不**进撤销事务）、`relink_media_file`（保持同 id、类型必须匹配）。
- `EventBus` + `CoreEvent`（4 变体：`TimelineChanged` / `ProjectOpened` / `ProjectSaved` / `MediaChanged`，`kind` 标签 + camelCase 序列化）。
- `CoreDeps` 四个能力 trait（preview / export / media / gen）+ `UnsupportedBackends` 占位（返回 `Unsupported` 而非 panic）。
- Tauri 边界 DTO + handler（`dto.rs`，**无 tauri 依赖**）：`get_timeline` / `edit_apply` / `undo` / `redo` / `project_open` / `project_save` / `project_new`，及 `CmdError`（`code` = `validation` | `internal`）。

**计划中（SPEC 草拟、代码尚未落地）：**
- **能力相关的 Tauri 命令尚未在 `dto.rs` 出现**：`seek` / `import_media`（异步后端版）/ `export_start`——它们依赖 `PreviewBackend` / `MediaImporter` / `ExportBackend` 的真实实现（`opentake-render` / `opentake-media`，后续阶段）。当前只有 trait 接缝 + `Unsupported` 占位。
- **更多 `CoreEvent` 变体**：`PreviewFrame` / `ExportProgress` / `ExportDone` / `ExportFailed` / `GenerationProgress`（随 render / export / gen 后端落地补齐）。
- **`preview_frame` 像素旁路**：事件只带元数据、像素走 Channel/asset 协议——契约属 core，传输属 `src-tauri` + `opentake-render` 协商（后续阶段）。
- **`src-tauri` 的 `#[tauri::command]` 薄壳 + 事件桥 task**：本 crate 已备好无 tauri 依赖的 handler 与 `EventBus::subscribe`，桥接代码在 `src-tauri`（后续阶段）。
- **`GenBackend`** 整体可选，早期为 `None`。
- **助手专属 undo 游标**（`AgentUndoCursor`）：属 `opentake-agent`，**不在 core**；core 只暴露通用 `undo()`/`redo()`。

> 结论：装配层的**纯逻辑核**（会话、命令路由、事务透传、版本、事件、工程读写、同步媒体导入、边界 DTO）已写通且可单测；待收口集中在**能力后端接线**（render/media/gen 的真实实现 + 对应 Tauri 命令与事件）。

## 移植铁律（Swift → Rust，本模块强约束）

core 自身不做帧算术，但作为装配/路由层必须守住若干跨进程一致性与编排不变量：

- **version 是跨进程同步的命脉**：`version` 严格单调（一把 `Mutex` 串行化所有变更保证）；committing 变更 +1，**撤销/重做也 +1**（上游撤销=整 timeline 替换 + 触发 rebuild，等价于 `revision &+= 1`，否则前端镜像与权威态不一致）；无变更命令**不** bump、**不**发事件（直译上游 `guard before != after`）。
- **锁外发事件**：`drop` 锁之后才 `emit`，避免订阅回调重入 `Mutex` 死锁。
- **锁内无 IO**：临界区只做值类型 timeline 操作；解码 / 导出 / 生成在锁外、由 deps 在独立 task 跑。
- **open 装配顺序照搬上游 `makeWindowControllers`**：先 decode `timeline`（version 归 0）→ 记 `project_dir` → decode `manifest` → decode `generation_log`（**宽松**：损坏降级为 `None`，不致命；只 `project.json` 缺失才报错）。open **不**发 `TimelineChanged`（前端自取首个快照）。
- **媒体导入在撤销事务之外**：照搬上游——只文件夹移动经 `apply` 可撤销；裸导入直接追加 manifest，不进撤销栈、不动 timeline version。
- **重链保持同 id**：re-import 会铸新 id 使旧 clip 永久孤立；`relink_media_file` 复用原 id 在位治愈，并拒绝类型变更（对齐上游 relink 拒绝语义）。
- **错误诚实分类**：校验失败（`Edit` / `Media`）→ `code: "validation"`，文档/目录不变、version 不动；IO/解码（`Project` / `NoProjectOpen` / `Unsupported`）→ `code: "internal"`。命令失败时前端镜像保持一致（事务早返回，无事件）。
- **DTO 多词字段 camelCase**：`dto.rs` 全部 `#[serde(rename_all = "camelCase")]`，对齐前端命名（历史上 IPC 字段 camelCase 没对齐导致编辑静默失效，见 [SPEC.md](SPEC.md) 与 [opentake-ops INDEX](../opentake-ops/INDEX.md) 的序列化陷阱）。`Timeline` 本体用 domain schema 序列化（= `project.json`），镜像与持久化同一形状。

## 子系统文档

| 文档 | 覆盖 |
|---|---|
| [session.md](session.md) | `session.rs` — `EditorSession` 会话装配、open/save 顺序、同步媒体导入/重链、白名单 |
| [core-router.md](core-router.md) | `core.rs` — `AppCore` 句柄、`apply` 命令路由与广播、并发串行化、id 生成 |
| [deps-di.md](deps-di.md) | `deps.rs` — `CoreDeps` 能力 trait 注入 + `UnsupportedBackends` 占位纪律 |
| [events-bus.md](events-bus.md) | `events.rs` — `CoreEvent` / `EventBus` 单向变更通知（替代 `@Observable`） |
| [dto.md](dto.md) | `dto.rs` Tauri 边界 DTO + handler（无 tauri 依赖）+ `error.rs` `CoreError` |

---

> 本模块目录：[INDEX.md](INDEX.md) · 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
