## 1. EditorState 结构

### 1.1 职责与对应上游

`opentake_ops::EditorState` 是上游 `EditorViewModel` 的可编辑文档与撤销基础设施子集。它只保存 timeline、media manifest、整文档 undo/redo 快照和单调版本号；selection、zoom、面板可见性、scrubbing 等 UI-only 状态仍归前端 Zustand。

OpenTake 将上游单进程 `EditorViewModel` 拆成三层：

| 层 | 当前实现 | 所有权 |
|---|---|---|
| 可编辑文档与历史 | `crates/opentake-ops/src/editor_state.rs::EditorState` | timeline、manifest、undo/redo、version |
| 工程会话 | `crates/opentake-core/src/session.rs::EditorSession` | `EditorState`、project root/path、generation log、兼容性 |
| 并发与事件门面 | `crates/opentake-core/src/core.rs::AppCore` | 单一 session 锁、事件总线、依赖与 ID 生成器 |

这三层没有复制权威 timeline：`EditorSession` 按值持有一个 `EditorState`，`AppCore` 的所有 clone 共享同一个 session。

### 1.2 已落地字段与不变量

```rust
// crates/opentake-ops/src/editor_state.rs
pub struct DocSnapshot {
    pub timeline: Timeline,
    pub manifest: MediaManifest,
}

pub struct EditorState {
    pub timeline: Timeline,
    pub manifest: MediaManifest,
    undo_stack: Vec<DocSnapshot>,
    redo_stack: Vec<DocSnapshot>,
    version: u64,
}
```

普通编辑命令通过 `crates/opentake-ops/src/command.rs::transact` 执行；Undo/Redo 和带 refusal 结果的 ripple 路径在同一文件内执行等价的 snapshot/restore/commit 流程：

1. 同时快照 timeline 与 manifest。
2. 执行命令；若命令返回错误，恢复快照并原样返回错误。
3. `before == after` 时不写历史、不清 redo、不递增 version。
4. 文档改变时把 `before` 压入 undo、清空 redo，并把 version 递增一次。
5. undo/redo 成功时交换整文档快照并各递增 version 一次；空历史是 `changed=false` 的 no-op。

`project_epoch + version` 标识前端读取的权威 timeline 镜像，不是整个 manifest 的修订号：导入、relink、favorite 等独立媒体工作流可在 timeline version 不变时提交 manifest，并通过 `MediaChanged` 同步。对于 `EditCommand` 管理的 timeline + manifest 历史，失败命令不能留下部分写入。对应证据：

- `commit_undo_redo_cycle_restores_and_versions`
- `failed_transaction_restores_document_and_history`
- `unchanged_command_does_not_emit_or_bump`

`generation_log`、工程路径、运行时兼容性和 retained project-root authority 不属于编辑算法，因此由 `EditorSession` 持有；媒体解码、导出和生成后端由 `CoreDeps` 注入，也不进入 `EditorState`。

### 1.3 访问封装：`AppCore`

```rust
// crates/opentake-core/src/core.rs
#[derive(Clone)]
pub struct AppCore {
    session: Arc<Mutex<CoreSessionSlot>>,
    project_identity_workflow: Arc<RwLock<()>>,
    events: EventBus,
    deps: Arc<CoreDeps>,
    ids: Arc<dyn IdGen + Send + Sync>,
}
```

Tauri `State<AppCore>`、应用内 Agent 和 MCP handler 持有的都是 clone；这些 clone 指向同一个 `CoreSessionSlot`，所以共享同一 timeline、manifest、undo/redo 和 version 序列。

`AppCore::apply` 在 session 锁内调用 `EditorSession::apply`，后者只委托给 `opentake_ops::command::apply`。命令结束后先释放 session 锁，再在 `changed=true` 时恰好发送一个 `CoreEvent::TimelineChanged { project_epoch, version }`；manifest 也改变时额外发送一个 `MediaChanged`，让媒体镜像重取。订阅者可安全地重入读取 API，编辑临界区不执行媒体解码、导出或网络 IO。

对应证据：

- `apply_bumps_version_and_emits_once`
- `manifest_edit_and_undo_emit_media_changed`
- `rejected_command_returns_err_without_emitting`
- `undo_redo_through_core_bumps_version_and_emits`

---
