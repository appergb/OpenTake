## 2. 命令路由 = 上游 ToolExecutor 单一能力层

### 2.1 核心原则：一处定义，三客户端共享

上游没有显式的 `EditCommand` 枚举。`ToolExecutor.run` 负责工具分发，`EditorViewModel.withTimelineSwap` 负责 before/after、undo 注册与变更通知。OpenTake 将这条隐式链显式分成：

1. `opentake_ops::EditCommand`：所有可编辑动作的类型化集合。
2. `opentake_ops::command::apply`：唯一编辑算法入口。
3. `command::transact` + `EditorState`：快照、错误回滚、no-op 判定、commit、version 与 undo/redo。
4. `opentake_core::AppCore::apply`：共享 session 锁和提交后的 domain 事件。

UI、应用内 Agent、外部 MCP 最终都调用同一个 `AppCore`，不会各自实现帧算术、重叠求解或撤销。

### 2.2 `EditCommand` 与 `EditResult`

`EditCommand` 的权威定义在 `crates/opentake-ops/src/command.rs`。它包含 timeline、clip、keyframe、媒体文件夹和全局 `Undo`/`Redo` 等变体；文档不复制完整枚举，避免新增命令后形成第二份陈旧清单。

```rust
pub struct EditResult {
    pub changed: bool,
    pub timeline_changed: bool,
    pub manifest_changed: bool,
    pub action_name: String,
    pub affected_clip_ids: Vec<String>,
    pub timeline_version: u64,
    pub summary: String,
}
```

边界职责如下：

- `opentake-ops`：参数语义校验、编辑算法、整文档事务、undo/redo 与 version。
- `opentake-core`：会话可变性检查、互斥访问、跨客户端共享、事件和项目生命周期。
- `src-tauri` / `opentake-agent`：把 DTO 或工具参数转换成 `EditCommand`，不得重写编辑算法。

### 2.3 `command::transact` 与 `AppCore::apply`

`command::transact` 是 `withTimelineSwap` 的当前等价实现：

```rust
let before = state.snapshot();
let affected = match work(state) {
    Ok(affected) => affected,
    Err(error) => {
        state.restore(before);
        return Err(error);
    }
};
let changed = before != state.snapshot();
if changed {
    state.commit(before);
}
```

这保证成功命令只提交一次，no-op 不提交，错误路径恢复 timeline 与 manifest 且不改变历史/version。批量命令在一个 `EditCommand` 内完成，所以仍只产生一个 undo 项。

`AppCore::apply` 不重复事务算法，只在锁内委托给 session，并在锁外处理事件：

```rust
pub fn apply(&self, command: EditCommand) -> Result<EditResult> {
    let (result, project_epoch, media_count) = {
        let mut session = self.lock();
        let result = session.editor.apply(command, self.ids.as_ref())?;
        let media_count =
            result.manifest_changed.then(|| session.editor.media().entries.len());
        (result, session.project_epoch, media_count)
    };
    if result.changed {
        self.events.emit(&CoreEvent::TimelineChanged {
            project_epoch,
            version: result.timeline_version,
        });
    }
    if let Some(count) = media_count {
        self.events.emit(&CoreEvent::MediaChanged { project_epoch, count });
    }
    Ok(result)
}
```

已验证的结果矩阵：

| 路径 | 文档 | version | undo/redo | `TimelineChanged` | `MediaChanged` |
|---|---|---:|---|---:|---:|
| 成功且改变 timeline | 提交 after | +1 | push before / clear redo | 1 | 0 |
| 成功且改变 manifest | 提交 after | +1 | push before / clear redo | 1 | 1 |
| 成功但 no-op | 不变 | 不变 | 不变 | 0 | 0 |
| 校验或执行错误 | 恢复 before | 不变 | 不变 | 0 | 0 |
| 成功 undo/redo | 恢复目标快照 | +1 | 双栈对称移动 | 1 | manifest 改变时 1 |
| 空历史 undo/redo | 不变 | 不变 | 不变 | 0 | 0 |

对应测试为 `failed_transaction_restores_document_and_history`、`apply_bumps_version_and_emits_once`、`manifest_edit_and_undo_emit_media_changed`、`unchanged_command_does_not_emit_or_bump` 和 `undo_redo_through_core_bumps_version_and_emits`。

### 2.4 Undo / Redo —— 全局历史与助手会话所有权

全局 undo/redo 由 `EditorState` 的两个 `Vec<DocSnapshot>` 实现。`EditCommand::Undo` 与 `EditCommand::Redo` 走同一个 ops 入口；`AppCore::undo()` / `redo()` 是面向 UI 的薄封装，因此 commit、version 和事件规则没有第二套实现。空历史返回成功的 `changed=false`，不是错误。

上游还有独立的助手会话所有权规则：每个 `ToolExecutor` 只能撤销本会话 Agent 的最近编辑，并在用户编辑成为全局栈顶时拒绝。这个规则属于 `opentake-agent::Dispatcher`，不属于全局 core 历史。本节的 DS7 证据只关闭共享 core 的全局 undo/version/event 路径；助手冲突安全由独立实施片 `AG-execution-shell-and-agent-undo` 及其“用户编辑位于栈顶”故障测试关闭，不能用 DS7 的全局测试替代。

### 2.5 三客户端如何共享（装配视角）

```text
                       AppCore (Clone)
             Arc<Mutex<CoreSessionSlot>> + EventBus
                  /              |              \
       Tauri edit_apply     in-app Agent      MCP Dispatcher
       undo / redo          CoreHandle        CoreHandle
             \                  |                  /
                 EditCommand -> AppCore::apply
```

- UI：`web/src/lib/api.ts::editApply` → Tauri `src-tauri/src/commands.rs::edit_apply` → `handle_edit_apply` → `AppCore::apply`。
- UI 全局撤销：Tauri `undo`/`redo` → core DTO handler → `AppCore::undo`/`redo`。
- Agent/MCP：`Dispatcher` 将类型化工具参数构造成 `EditCommand`，通过 `CoreHandle::apply` 进入同一个 `AppCore`。
- 事件：所有 clone 共享 `EventBus`；只有已提交的文档变化才发送携带 `project_epoch` 与 `version` 的事件。

因此三类客户端共享同一权威文档、同一全局历史和同一版本序列，同时保留各自边界层的参数解析与会话级策略。

---
