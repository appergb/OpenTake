# core.rs — AppCore 命令路由与并发外壳

> 上级：本模块目录 [INDEX.md](INDEX.md) · 总览 [OVERVIEW.md](OVERVIEW.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 源码：[`../../../crates/opentake-core/src/core.rs`](../../../crates/opentake-core/src/core.rs)

## 定位

`AppCore` 是装配层的**公开句柄**：一个 `Clone` 的、并发安全、可观测的 [`EditorSession`](session.md) 外壳。它是 UI / 内置 Agent / MCP 三客户端的**共同汇聚点**。

```
AppCore（#[derive(Clone)]）
├── session: Arc<Mutex<EditorSession>>      // 单一权威实例（对应上游单进程单 EditorViewModel）
├── events:  EventBus                       // 见 events-bus.md（克隆共享同一订阅列表）
├── deps:    Arc<CoreDeps>                  // 见 deps-di.md（注入式能力后端）
└── ids:     Arc<dyn IdGen + Send + Sync>   // 命令铸新 id 用；默认 CoreIdGen（原子计数）
```

`AppCore` 是 `Clone`——**克隆只复制 `Arc`**。Tauri `State`、MCP handler、内置 agent loop 各持一份**指向同一个 `Mutex<EditorSession>`** 的句柄，这正是上游「三客户端共享一个 view model」在跨线程下的等价物。一条编译期断言 `assert_send_sync::<AppCore>()` 守住"句柄必须可跨线程共享"这一跨进程设计前提。

## AppCore 在 EditorSession 之上只多两件事

`EditorSession` 已把编辑 + 撤销/版本事务委派给 `opentake-ops`。`AppCore` 只补会话给不了的两点：

1. **串行化所有变更**：一把 `Mutex`，使 `version` 在并发客户端下**严格单调**、无写竞争（见 [SPEC.md](SPEC.md) §4.3）。
2. **变更广播**：committing 的 edit / undo / redo 之后发 `CoreEvent::TimelineChanged`，让观察者重新同步镜像。事件在**锁释放之后**才发——订阅回调因此可安全地重入 core 而不死锁。

它**刻意不**重实现任何编辑、事务、持久化逻辑——那些活在 `opentake-ops` / `opentake-project`，经会话触达。

## 唯一编辑入口：`AppCore::apply`

```rust
pub fn apply(&self, command: EditCommand) -> Result<EditResult> {
    let result = {
        let mut session = self.lock();
        session.apply(command, self.ids.as_ref())?   // 锁内跑 ops 事务
    };                                                 // ← 锁在此释放
    if result.changed {
        self.events.emit(&CoreEvent::TimelineChanged { version: result.timeline_version });
    }
    Ok(result)
}
```

这是本 crate 命令路由的核心。逐步：

1. **取锁** → `EditorSession::apply` → `opentake_ops::command::apply`（事务本体：snapshot → 纯函数变更 → before!=after 才提交 + version++）。
2. **释放锁**（`{}` 作用域结束）。
3. `result.changed` 为真 → 发**恰好一次** `TimelineChanged { version }`；为假（无变更）或 `Err`（被拒）→ **不发事件、不动 version**。

> **三处不变量**（均有单测固化）：committing 命令 version 恰好 +1 且发一次事件；无变更命令（如空历史 undo）`changed == false`、version 不变、无事件；被拒命令（如 ops 层校验失败的空 `AddClips`）返回 `Err`、version 不变、无事件。

### 三客户端如何共享

- **UI**：React 手势 → `src-tauri` 的 `edit_apply` → `handle_edit_apply`（[dto.md](dto.md)）→ `AppCore::apply`。UI **不**经 agent 工具层，直接构造 `EditCommand`（对应上游 SwiftUI 直接调 `editor.addClips(...)` 而非伪装成工具）。
- **内置 Agent / MCP**：工具调用 → `opentake-agent` 把工具 args 翻译成 `EditCommand` → 同一个 `AppCore::apply`。工具层只做"短 id 展开/缩短 + args 校验 + 命令构造 + summary 渲染"，编辑本体全归 core/ops。
- **三者共享**：同一 `AppCore` 句柄（克隆）= 同一 `Mutex<EditorSession>` = 同一份权威 timeline + 同一 version 序列 + 同一全局撤销栈。这就是「单一能力层、多前端」在跨进程下的精确实现。

## Undo / Redo（薄包装，复用同一路径）

```rust
pub fn undo(&self) -> Result<EditResult> { self.apply(EditCommand::Undo) }
pub fn redo(&self) -> Result<EditResult> { self.apply(EditCommand::Redo) }
```

全局撤销（UI 的 Cmd+Z）是 `EditCommand::Undo`/`Redo`，**经同一个 `apply`**——因此复用同一事务 + 事件路径。ops 层在成功 undo 时 bump version，前端镜像据此重取。

> **关键决策：撤销也 bump version + 发事件**。上游撤销 = 整 timeline 替换 + 触发 rebuild（等价 `revision &+= 1`）；OpenTake 撤销/重做**必须** bump version 并发 `TimelineChanged`，否则前端镜像与权威态不一致。
>
> **助手专属 undo**（上游 `ToolExecutor.undo` 的拒绝语义）**不在 core**——core 只暴露通用 `undo()`/`redo()`；agent 层的 `AgentUndoCursor`（记录哪些 version 是本会话造成的）先校验再调 core，属 [opentake-agent](../opentake-agent/INDEX.md)。理由见 [SPEC.md](SPEC.md) §2.4：跨进程下一个 core 服务多个并发 MCP 连接，助手栈天然是 per-session 的，放 core 会串话。

## 工程生命周期

| 方法 | 行为 | 发的事件 |
|---|---|---|
| `new_project()` | 会话换成全新未保存工程 | `ProjectOpened { path: "", version: 0 }` |
| `open_project(path)` | 打开 `.opentake` 包替换会话；返回首个快照 | `ProjectOpened { path, version: 0 }`（**不**发 `TimelineChanged`——前端自取首快照，[SPEC.md](SPEC.md) §5.4 步骤 6） |
| `save_project(path)` | `None` 存回包（autosave）/ `Some` 另存为；返回写入路径 | `ProjectSaved { path }` |

均**先在锁内**完成会话变更、**释放锁后**才发事件（与 `apply` 同纪律）。

## 媒体导入（句柄层）

`import_media_file(path, name, probe)` / `relink_media_file(asset_id, path, probe)`：

- 在 `import` 路径上**从 core 的 id 生成器铸 asset id**（`self.ids.next_id()`），再调会话同名方法；
- **锁释放后**发 `CoreEvent::MediaChanged { count }`（count = 变更后 manifest entry 数，供廉价过期检查）；
- 导入**不动 timeline version**（manifest 在撤销事务之外，见 [session.md](session.md)）。

> 这是**同步**媒体路径（调用方供 `ProbedMedia`）。异步能力后端 `CoreDeps::media: MediaImporter`（含缩略图/波形）是另一条路、仍是接缝，见 [deps-di.md](deps-di.md)。

## 读 API

`get_timeline()` → `TimelineSnapshot { timeline, version }`（前端存为 `{ mirror, mirrorVersion }`，用 version 做幂等重取）；`version()` / `can_undo()` / `can_redo()`（供 UI 启停撤销按钮）；`media()`（manifest 克隆）/ `project_dir()`（解析 `MediaSource::Project` 相对路径用）。

## id 生成：`CoreIdGen`

`opentake_ops::SeqIdGen` 刻意 `!Sync`（经 `&self` 穿 `Cell`），适合单线程 ops 测试但不适合共享的 `Send + Sync` `AppCore`。`CoreIdGen` 用 `AtomicU64` 铸同样的 `"{prefix}{n}"` id 且可跨线程共享，**不把 `uuid` 依赖拉进装配层**。生产装配（`src-tauri`）可经 `set_id_gen` 注入 UUID 版生成器；影响后续命令铸的 id。

## 并发与锁纪律

- **一把 `Mutex` 串行化所有变更** → version 严格单调、无并发写竞争。
- **锁内只做值类型 timeline 操作**（ops 纯 CPU、无 IO）；解码/导出/生成在锁外、由 deps 在独立 task 跑。
- **锁外发事件**（`drop` 锁后 `emit`）→ 订阅回调可安全重入。
- `lock()` 内部从中毒互斥恢复（取内层 guard）：命令体是无 panic 的值类型操作，中毒不预期；恢复使某个观察者里的杂散 panic 不至于卡死整个 core。

## 测试覆盖（本文件 `#[cfg(test)]`）

`AppCore` 是 `Send + Sync`（编译期断言）；`CoreIdGen` 从 1 单调；克隆共享同一会话（一个 apply 后另一个读到新 version 与 clip）；apply bump version 并发一次事件；无变更命令不发不动；undo/redo 经 core bump version 并按序发 `[1,2,3]`；被拒命令返回 Err 不发事件；`new_project` 重置并发 `ProjectOpened`；open/save 往返发生命周期事件且第二 core 重开 timeline 一致；导入铸 id、追加、发 `MediaChanged` 且不动 version；不支持扩展名报错不发事件。

---

> 上级：本模块目录 [INDEX.md](INDEX.md) · 总览 [OVERVIEW.md](OVERVIEW.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
