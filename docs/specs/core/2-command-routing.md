## 2. 命令路由 = 上游 ToolExecutor 单一能力层

### 2.1 核心原则:一处定义,三客户端共享

ARCHITECTURE §5 原话:「UI 手势、应用内 Agent、外部 MCP **全部归一到一个 `EditCommand` 枚举**,撤销/校验/遥测只写一遍。」§0 洞察:「编辑能力只有一处真实定义(ToolExecutor → EditorViewModel),UI / 应用内 Agent / 外部 MCP 是它的三个对等客户端。」

**上游的事实分层**(读代码得出,不是想当然):
- 上游**没有**一个显式的 `EditCommand` 枚举。它的"单一能力层"是 **`ToolExecutor.run(_:_:_:)` 里那张 `switch tool`**(`ToolExecutor.swift:72-106`),每个 case 调 `EditorViewModel` 的一个 mutator(如 `addClips → editor.…` 经 `ToolExecutor+Clips.swift:129`)。
- 真正的"撤销/校验/遥测只写一遍"发生在**两层**:
  1. **统一执行壳** `ToolExecutor.execute(name:args:)`(`ToolExecutor.swift:22-70`):快照 before → 展开短 ID → 跑 → 若变更则压 agentUndoStack → 遥测 → 缩短 ID。
  2. **事务核心** `EditorViewModel.withTimelineSwap`(`ClipMutations.swift:240-252`):每个 mutator 内部用它包裹,负责 before/after diff + 注册双向 undo swap + `notifyTimelineChanged`。

OpenTake 把这两层**显式化、合并**成 `EditorCore::apply`,并在其上引入 ARCHITECTURE §5 草拟的 `EditCommand` 枚举(上游隐式、OpenTake 显式——这是有意的改进,让 UI 手势不必伪装成"工具调用"也能走同一入口)。

### 2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)

```rust
// crates/opentake-ops/src/command.rs  —— 枚举与 apply 算法属 ops crate(Phase 1)
// crates/opentake-core 只负责"路由 + 事务 + 事件",不重新定义编辑算法。
pub enum EditCommand {
    AddClips { entries: Vec<AddEntry> },
    InsertClips { track_index: usize, at_frame: i64, entries: Vec<InsertEntry> }, // ripple
    MoveClips { moves: Vec<Move> },
    RemoveClips { clip_ids: Vec<String> },
    RemoveTracks { track_ids: Vec<String> },
    SplitClip { clip_id: String, at_frame: i64 },
    TrimClips { /* … */ },
    SetClipProperties { clip_ids: Vec<String>, props: ClipPropsPatch },
    SetKeyframes { clip_id: String, property: KeyframeProperty, keyframes: Vec<Keyframe> },
    RippleDeleteRanges { clip_id: Option<String>, track_index: Option<usize>,
                         ranges: Vec<(f64, f64)>, units: RangeUnits },
    AddTexts { /* … */ },
    AddCaptions { /* … */ },
    Link { clip_ids: Vec<String> },
    Unlink { clip_ids: Vec<String> },
    CreateFolder { /* … */ },
    MoveToFolder { /* … */ },
    // 注意:Undo/Redo 不是 EditCommand —— 见 §2.4。它们是 core 的独立 API。
}

pub struct EditResult {
    pub changed: bool,                 // before != after(对应 withTimelineSwap 的短路判定)
    pub action_name: String,           // 撤销标签(对应 setActionName,如 "Add Clips")
    pub affected_clip_ids: Vec<String>,
    pub timeline_version: u64,         // 命令后的权威 version(changed=false 时 = 命令前 version)
    pub summary: String,              // 面向 LLM 的人类可读摘要(对应工具返回的文本)
}
```

> **边界纪律**:`EditCommand` + 其 `apply`(编辑算法本体)住在 `opentake-ops`(Phase 1 已做,含 OverwriteEngine/RippleEngine 等纯函数,ARCHITECTURE §5)。`opentake-core` **只**做"拿到命令 → 起事务 → 调 `command::apply` → 处理 diff/undo/version/event"。core **不含**任何帧算术或重叠求解逻辑。这与上游一致:`ToolExecutor` 不含编辑算法,只调 `EditorViewModel` 的 mutator。

### 2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)

这是本 crate 最关键的 30 行。**逐句对应** `ClipMutations.swift:240-252` + `230-237`:

```rust
impl EditorCore {
    /// 唯一编辑入口。UI/Agent/MCP 全部经此。对应上游 withTimelineSwap 事务。
    pub fn apply(&self, cmd: EditCommand) -> Result<EditResult, EditError> {
        let mut st = self.state.lock().unwrap();

        // (1) 快照 before(对应 `let before = timeline`,ClipMutations.swift:241)
        let before = st.timeline.clone();
        let action_name = cmd.action_name();          // 命令自带标签

        // (2) 改:调 ops 层纯函数 apply(可校验失败 → 早返回,timeline 不动)
        //     校验/精确路径错误在 ops::apply 内产生(serde_path_to_error,见 §7 / ARCHITECTURE §7)
        let outcome = opentake_ops::apply(&mut st.timeline, &cmd, &st.assets)
            .map_err(EditError::from)?;               // 失败:锁释放,version 不变,无事件

        // (3) before != after 短路(对应 `guard before != after else { return }`,:246)
        //     Timeline derive PartialEq(ARCHITECTURE §5 "PartialEq 短路")
        if st.timeline == before {
            let v = st.version;
            return Ok(EditResult { changed: false, action_name, timeline_version: v,
                                   affected_clip_ids: vec![], summary: outcome.summary });
        }

        // (4) 压 UndoStack(整树快照)+ version+1(对应 registerTimelineSwap + revision&+=1)
        let after = st.timeline.clone();
        st.undo.push_swap(before, after);             // 双向 swap:undo 回 before、redo 回 after
        st.version += 1;
        st.dirty = true;                              // 对应 updateChangeCount(.changeDone)
        let version = st.version;
        let affected = outcome.affected_clip_ids;
        let summary = outcome.summary;
        drop(st);                                     // 先释放锁,再发事件(避免事件订阅者回调时重入锁)

        // (5) 广播 timeline_changed{version}(对应 notifyTimelineChanged → 触发 rebuild)
        self.events.emit(CoreEvent::TimelineChanged { version });

        Ok(EditResult { changed: true, action_name, timeline_version: version,
                        affected_clip_ids: affected, summary })
    }
}
```

**与上游 `registerTimelineSwap`(`:230-237`)的对应**:上游用 `UndoManager.registerUndo` 闭包递归注册双向 swap;OpenTake 用 `UndoStack`(Phase 1 整树快照栈)把 `(before, after)` 压栈,`undo()` pop 并 `timeline = before`、`redo()` 反之。语义等价、更简单(无闭包递归)。

**上游"嵌套抑制"(`:247-249`)在 OpenTake 自动消失**:上游因为 `withUndoGroup`(`ToolExecutor.swift:151-158`)能嵌套 `withTimelineSwap`,需要 `isUndoRegistrationEnabled` 守卫防止重复注册。OpenTake 的 `apply` 是**单层、不可重入**(一条 `EditCommand` = 一个事务),批量操作在 `EditCommand` 内部完成(如 `AddClips{entries:[...]}` 一次性加多个 clip,对应上游 `addClips` 用一个 `withUndoGroup` 包整批,`ToolExecutor+Clips.swift:178`),**不存在事务嵌套**,故无需该守卫。这是显式 `EditCommand` 相对隐式工具链的结构性收益。

### 2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)

上游有**两套撤销概念**,OpenTake 必须都复刻:

| 概念 | 上游载体 | OpenTake 载体 | 谁能调 |
|---|---|---|---|
| **全局撤销树** | `EditorViewModel.undoManager`(`VideoProject.swift:191` 注入,系统 `UndoManager`) | `EditorState.undo: UndoStack`(在 Rust) | UI(Cmd+Z)经 `undo()/redo()` Tauri 命令 |
| **助手专属 undo 游标** | `ToolExecutor.agentUndoStack: [String]`(`ToolExecutor.swift:20`) | 每个 agent 会话一份 `AgentUndoCursor`(在 `opentake-agent`,**不在 core**) | 仅 Agent/MCP 的 `undo` 工具 |

**全局 undo/redo**(对应 UI 的 Cmd+Z):是 `EditorCore` 的独立方法,**不是 `EditCommand`**(因为它操作的是撤销栈本身,不是 timeline 内容):

```rust
impl EditorCore {
    pub fn undo(&self) -> Result<EditResult, EditError> {
        let mut st = self.state.lock().unwrap();
        let Some(before) = st.undo.undo(&mut st.timeline) else {   // 无可撤销
            return Err(EditError::NothingToUndo);
        };
        st.version += 1; st.dirty = true;                          // 撤销也 bump version(前端镜像需刷新)
        let version = st.version; drop(st);
        self.events.emit(CoreEvent::TimelineChanged { version });
        Ok(EditResult { changed: true, action_name: "Undo".into(), timeline_version: version, .. })
    }
    pub fn redo(&self) -> Result<EditResult, EditError> { /* 对称 */ }
}
```

> **关键决策:撤销也递增 version**。上游撤销时 `vm.timeline = undoState; vm.notifyTimelineChanged()`(`ClipMutations.swift:232-233`)——timeline 被替换、触发 rebuild,等价于 `revision&+=1`。所以 OpenTake 撤销/重做**必须** bump `version` 并发 `timeline_changed`,否则前端镜像与权威态不一致。

**助手专属 undo**(对应 `ToolExecutor.undo`,`ToolExecutor.swift:108-123`)的**精妙拒绝语义必须照搬**:
- 「本会话没有助手编辑可撤销」→ 报错,不动(`:110-112`,原话:"The user's own edits are theirs to undo.")。
- 「栈顶 action 不是助手做的」→ 拒绝,不撤(`:117-119`)。这防止助手撤掉用户手动的编辑。
- 成功后从 agent 栈弹出,并提示「re-read with get_timeline before editing again」(`:122`)。

这套逻辑在 OpenTake **属于 `opentake-agent`(Phase 7)**,不属于 core。core 只暴露**通用** `undo()/redo()`;agent 的 `undo` 工具在 agent 层先校验它自己的 `AgentUndoCursor`(记录"哪些 version 是本会话造成的"),通过后再调 `core.undo()`。

> **为何 cursor 在 agent 层而非 core**:上游每个 `ToolExecutor` 实例(每个 MCP 连接 / 每个 chat 会话)有独立 `agentUndoStack`(`AgentService.swift:215` 与 `MCPService.swift:31` 各造一个 executor)。跨进程下,一个 core 会服务多个并发 MCP 连接,**助手栈天然是 per-session 的**,放 core 里会串话。core 持唯一全局撤销栈;agent 层持 per-session 游标。

### 2.5 三客户端如何共享(装配视角)

```
            ┌───────────────────── EditorCore (Clone, 内含 Arc<Mutex<EditorState>>) ─────────────────────┐
            │                                  唯一 EditCommand 路由 + 唯一 UndoStack + 唯一 version       │
            └───────▲───────────────────────────────▲───────────────────────────────────▲────────────────┘
                    │ apply / undo / get_timeline    │ apply (工具→EditCommand)            │ apply (工具→EditCommand)
        ┌───────────┴──────────┐        ┌────────────┴───────────┐            ┌───────────┴────────────┐
        │ Tauri command 层      │        │ in-app agent (opentake- │            │ MCP server (opentake-   │
        │ (src-tauri, §3)       │        │  agent, reqwest→LLM)    │            │  agent, rmcp, §7)       │
        │ ↑ React UI            │        │  per-session ToolCtx +  │            │  per-conn ToolCtx +     │
        │   (Cmd+Z, 拖拽, 裁剪) │        │  AgentUndoCursor        │            │  AgentUndoCursor        │
        └──────────────────────┘        └─────────────────────────┘            └─────────────────────────┘
```

- **UI**:React 手势 → Tauri `invoke('edit_apply', {command})` → core.apply。UI **不**经 agent 工具层(直接构造 `EditCommand`)。对应上游 SwiftUI 直接调 `editor.addClips(...)` 而非伪装成工具。
- **in-app agent / MCP**:工具调用 → agent 层把工具 args 翻译成 `EditCommand` → core.apply。**工具层只做"短 ID 展开/缩短 + args 校验 + EditCommand 构造 + summary 渲染"**,编辑本体全归 core(对应 `ToolExecutor.run` 的每个 case 实质只是参数搬运到 mutator)。
- **三者共享**:同一个 `EditorCore` 句柄(克隆),即同一个 `Mutex<EditorState>`,即同一份权威 timeline + 同一个 version 序列 + 同一个全局撤销栈。这就是「单一能力层、多前端」在跨进程下的精确实现。

---
