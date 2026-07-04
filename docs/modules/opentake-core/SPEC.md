# opentake-core 实现就绪规格(Issue #11)

> **范围**:`crates/opentake-core/` —— EditorState 组装、命令路由(= 上游单一能力层)、事件总线、Tauri 边界契约。
> **本 crate 的一句话职责**:把 `opentake-{domain,ops,project,render,agent}` 装配成**一个权威可观测状态容器 `EditorState`**,对 UI / Agent / MCP 三个对等客户端暴露**唯一一条编辑入口**(`EditorCore::apply`),并把状态变更通过**单调递增版本号 + 事件广播**推给前端。
> **证据基线**:逐行精读 palmier-pro-upstream 以下文件得出(下文引用均为「文件:行号」,均来自 `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/`):
> - `Agent/Tools/ToolExecutor.swift`(单一能力层、统一执行壳、agentUndoStack)
> - `Agent/Tools/ToolExecutor.swift:22-70`(execute 壳)、`:108-123`(助手专属 undo)
> - `Agent/Tools/ToolExecutor+ShortId.swift`(短 ID 系统)
> - `App/AppState.swift`(进程级生命周期、MCPService 装配)
> - `Editor/ViewModel/EditorViewModel.swift`(状态容器:timeline+manifest+log+revision)
> - `Editor/ViewModel/EditorViewModel+ClipMutations.swift:230-252`(`withTimelineSwap`/`registerTimelineSwap` 事务核心)
> - `Project/VideoProject.swift`(持久化读写、装配顺序)
> - `Agent/MCP/MCPService.swift` + `Agent/MCP/MCPHTTPServer.swift`(MCP 边界、loopback+Origin 校验)
> - `Agent/AgentService.swift:215-217,424-441`(应用内 chat 走同一 ToolExecutor 逻辑)
> - `Utilities/Constants.swift:105-115`(工程目录包文件名常量)
> - `Export/ExportService.swift` + `Export/ExportView.swift:13-26`(导出表面)
> - `Agent/Tools/ToolExecutor+Clips.swift:129-209`(写工具样板:decode→validate→withUndoGroup→mutate)
> 以及 `docs/ARCHITECTURE.md` §2/§5/§7 与 `docs/ROADMAP.md` Phase 6/7。

---

## 0. 设计基线:跨进程改变了什么(必读)

上游是**单进程、单实例**:UI(SwiftUI)、应用内 chat(`AgentService`)、MCP server(`MCPService`)全部活在同一个 macOS 进程里,共享同一个 `EditorViewModel` 引用(证据:`AppState.swift:23-27` MCPService 的 `editorProvider` 闭包返回 `activeProject?.editorViewModel`;`EditorViewModel.swift:178` `agentService.editor = self`)。因此三客户端**天然看到同一份内存里的 `timeline`**,无需任何同步协议——SwiftUI 靠 `@Observable` 自动重渲,MCP/chat 直接读 `editor.timeline`。

OpenTake 是 **Rust core + Tauri + React 跨「逻辑进程边界」**(前端在 WebView,core 在 Rust 侧):
- 前端**不能**持有权威 `Timeline`,只能持**只读镜像**;
- 必须由 Rust 侧单一持有权威 `Timeline`;
- 每次编辑后,前端镜像必须靠**单调递增版本号**(对应上游 `timelineRenderRevision`,`EditorViewModel.swift:27-28,76`)判定失效并重取。

> **§2 架构原话**:「真相源在 Rust,前端只持镜像……前端拿快照 + 单调递增版本号(对应上游 `timelineRenderRevision`),每次 `edit_apply` 广播 `timeline_changed{version}`,前端据此重取。」本 crate 即落地这句话。

**一个必须显式处理、上游"免费"得到的事实**:上游三客户端共享同一个 `EditorViewModel`,但**各自持有独立的 ToolExecutor 实例**——MCP 经 `MCPService.init → ToolExecutor(editorProvider:)`(`MCPService.swift:31-33`),应用内 chat 经 `AgentService.swift:215 didSet { toolExecutor = editor.map { ToolExecutor(editor: $0) } }`。两个 executor 实例**各自维护自己的 `agentUndoStack`**(`ToolExecutor.swift:20`),但都对**同一个 `EditorViewModel.undoManager`**(同一棵撤销树)操作。OpenTake 必须复刻这个分工:**编辑逻辑唯一一份(`EditorCore`),撤销栈唯一一份(在 `EditorState`),但"助手专属 undo 游标"按客户端会话隔离**(见 §2.4)。

---

## 1. EditorState 结构

### 1.1 职责与对应上游

`EditorState` = 上游 `EditorViewModel` 的「持久化状态 + 撤销基础设施 + 版本号」子集(`EditorViewModel.swift:26-32,76,184`),**剥离全部 UI-only 瞬态**(selection/zoom/panel 可见性/scrubbing 等,`EditorViewModel.swift:55-107` 那一大片)。UI-only 态在 OpenTake 归前端 Zustand(§2 架构图、ARCHITECTURE §2),**不进 Rust core**。

> 上游把权威态和 UI 态揉在一个 `EditorViewModel` 里(因为单进程、SwiftUI 直接绑)。OpenTake 必须切分:Rust 只持**可序列化的真相**,前端持**交互态**。这是跨进程架构的硬性边界,不是风格选择。

### 1.2 字段(草案)

```rust
// crates/opentake-core/src/state.rs
use opentake_domain::Timeline;
use opentake_project::MediaManifest;       // entries + folders
use opentake_project::GenerationLog;       // append-only AI 审计
use opentake_ops::UndoStack;               // 整树快照栈(Phase 1 已建)
use opentake_media::MediaAsset;            // 运行时富对象(非磁盘 entry)

/// 权威编辑状态容器。对应上游 EditorViewModel 的「持久化 + 撤销 + 版本」子集。
/// 跨线程通过 EditorCore(Arc<Mutex<EditorState>>)访问,自身不需 Send 约束之外的并发设施。
pub struct EditorState {
    // ── 持久化真相(对应 EditorViewModel.swift:27,30,31) ──
    timeline: Timeline,                    // 唯一权威 timeline
    manifest: MediaManifest,               // 媒体清单(磁盘 entries + folders)
    generation_log: GenerationLog,         // AI 生成审计(append-only)

    // ── 撤销基础设施(对应 EditorViewModel.undoManager + withTimelineSwap) ──
    undo: UndoStack,                       // 整树快照撤销/重做栈(在 Rust,见 §2.3)

    // ── 版本号(对应 EditorViewModel.swift:27-28 didSet 里的 timelineRenderRevision &+= 1) ──
    version: u64,                          // 单调递增;timeline 每次"被替换"即 +1

    // ── 运行时媒体库(内存,工程打开时重建,对应 EditorViewModel.swift:110 mediaAssets) ──
    assets: Vec<MediaAsset>,               // 由 manifest 在 open 时物化(VideoProject.swift:304-339)
    offline_media: HashSet<String>,        // 对应 offlineMediaRefs
    unprocessable_media: HashSet<String>,  // 对应 unprocessableMediaRefs

    // ── 工程引用(对应 EditorViewModel.swift:115-126) ──
    project_dir: Option<PathBuf>,          // .opentake 目录;None = 未保存
    project_id: Option<String>,            // 遥测用稳定 id
    dirty: bool,                           // 对应 isDocumentEdited(VideoProject.swift:130-138)
}
```

**版本号 = OpenTake 跨进程同步的命脉**。上游 `timelineRenderRevision` 仅用于 SwiftUI diff(`TimelineContainerView.swift:61`);OpenTake 把它**升级为前端镜像一致性的权威信标**:`version` 只在「timeline 真正被替换且 before != after」时递增(§2.3 步骤 4),每个 `get_timeline` 快照都带 `version`,每个 `timeline_changed` 事件都带 `version`,前端用它做幂等重取(§4)。

> **不变量(必须单测)**:`version` 单调递增;同一个 `version` 下取到的 timeline 快照逐字节一致;任何不改变 timeline 的命令(`before == after`)**不** bump `version`、**不**发 `timeline_changed`(直接复刻 `withTimelineSwap` 的 `guard before != after else { return }`,`ClipMutations.swift:246`)。

### 1.3 访问封装:`EditorCore`

`EditorState` 自身是纯数据 + 同步方法,不持有锁。对外句柄是 `EditorCore`:

```rust
// crates/opentake-core/src/core.rs
#[derive(Clone)]
pub struct EditorCore {
    state: Arc<Mutex<EditorState>>,        // 单一权威实例(对应单进程单 EditorViewModel)
    events: EventBus,                      // 见 §3
    deps: Arc<CoreDeps>,                   // render/media/project/gen 的注入句柄,见 §5
}
```

`EditorCore` 是 `Clone`(克隆只复制 `Arc`),因此 Tauri `State`、MCP server handler、in-app agent loop 可各持一份**指向同一 `Mutex<EditorState>`** 的句柄——这正是上游「三客户端共享同一 `EditorViewModel`」在 Rust 跨线程下的等价物(`AppState.swift:23-27` 的 `editorProvider` 闭包在 OpenTake 退化为「克隆 EditorCore」)。

> **锁粒度**:命令执行(§2.3)全程持 `state` 锁;命令本体是纯 CPU 的值类型操作(`opentake-ops`,Phase 1),无 IO,临界区短,`std::sync::Mutex` 足够。**IO(解码/导出/生成)绝不在锁内**——它们在命令完成、锁释放后,由 `deps` 在独立 task 上跑(§5、§6)。

---

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

## 3. 事件总线

### 3.1 `EventBus` 与事件类型

上游靠 SwiftUI `@Observable` 自动传播(`EditorViewModel.swift:21-22` 的 `@Observable`),无显式事件总线。OpenTake 跨进程,需要显式总线把 core 的状态变更推到 Tauri 边界,再由 Tauri 转成前端 `emit`。

```rust
// crates/opentake-core/src/events.rs
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoreEvent {
    TimelineChanged { version: u64 },                          // 对应 §2.3(5) / notifyTimelineChanged
    PreviewFrame { frame: i64, width: u32, height: u32 },      // 预览帧就绪(像素经 §3.3 旁路)
    ExportProgress { job_id: String, progress: f64,            // 0.0..1.0
                     phase: ExportPhase, eta_secs: Option<f64> },
    ExportDone { job_id: String, output_path: String },
    ExportFailed { job_id: String, message: String },
    GenerationProgress { job_id: String, status: GenStatus },  // 见 ARCHITECTURE §8 job 状态机
    MediaImported { asset_id: String },                        // 对应 mediaPanelRevealAssetId 流(AppState.swift:90-101)
}

pub struct EventBus { tx: tokio::sync::broadcast::Sender<CoreEvent> }
impl EventBus {
    pub fn emit(&self, ev: CoreEvent) { let _ = self.tx.send(ev); }   // 无订阅者不 panic
    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> { self.tx.subscribe() }
}
```

> 用 `tokio::broadcast`:Tauri 桥接 task 订阅一份转发给前端;未来其他订阅者(如自动保存、遥测)可独立订阅,不互相阻塞。`emit` 永不阻塞命令路径(§2.3 在 `drop(st)` 后才 emit)。

### 3.2 Tauri 桥接(src-tauri,薄)

`src-tauri` 启动时 `subscribe()` 一次,起一个 task 把 `CoreEvent` 映射成 Tauri 前端事件名:

| CoreEvent | Tauri event name | payload |
|---|---|---|
| `TimelineChanged{version}` | `"timeline_changed"` | `{ version: number }` |
| `PreviewFrame{..}` | `"preview_frame"` | `{ frame, width, height }`(像素见 §3.3) |
| `ExportProgress{..}` | `"export_progress"` | `{ jobId, progress, phase, etaSecs }` |
| `ExportDone/Failed` | `"export_progress"` | 同上,`phase: "done"\|"failed"` |
| `GenerationProgress` | `"generation_progress"` | `{ jobId, status }` |
| `MediaImported` | `"media_imported"` | `{ assetId }` |

> ARCHITECTURE §2 只点名了三个核心 event(`timeline_changed{version}` / `preview_frame` / `export_progress`)。本规格把它们定全签名,并补 `generation_progress` / `media_imported`(从上游 `GenerationService`、`mediaPanelRevealAssetId` 流推得,属同类"core→前端单向通知")。`generation_progress` 在 Phase 9 才实装,Phase 6/7 可只发前三。

### 3.3 `preview_frame` 的像素旁路(关键工程决策)

`preview_frame` **事件本身只带元数据(frame/width/height),不带像素**。原始 RGBA 帧不走 Tauri 事件(JSON 序列化大帧会卡)。三选一(按 ARCHITECTURE §2「Preview(canvas 显示 Rust 合成帧)」):

1. **共享内存 / 自定义 URI scheme**:core 把帧写入 Tauri `asset://` 或自定义协议端点,前端 `<canvas>` 经 `<img>`/`createImageBitmap` 拉。
2. **IPC 二进制通道**:Tauri 2 的 `Channel<&[u8]>`(比 event 高效,适合连续帧)。
3. **WebGL 直接上屏**(ARCHITECTURE §2 提到的 WebGL):core 出纹理句柄,前端 GL 直采。

> 本 crate 只定义**契约**:`PreviewFrame` 事件 = "第 N 帧已就绪,去拉",像素传输属 `opentake-render` 播放后端(Phase 4)与 src-tauri 协商的实现细节,不在 core 逻辑内。core 的 `seek` 命令(§3.4)触发 render 后端解码合成,render 后端就绪后经 `events.emit(PreviewFrame{..})` 通知。

### 3.4 谁触发 `preview_frame`

`seek` / 播放不属于 `EditCommand`(它们不改 timeline、不进撤销栈)。core 暴露独立的 `seek(frame)` API:

```rust
impl EditorCore {
    pub fn seek(&self, frame: i64, mode: SeekMode) {
        // 对应 EditorViewModel.seekToFrame(:259-267):钳制到 [0, totalFrames]
        let clamped = { let st = self.state.lock().unwrap();
                        frame.clamp(0, st.timeline.total_frames()) };
        // 交给 render 播放后端(§5 deps);后端合成完单帧后 emit PreviewFrame
        self.deps.preview.request_frame(clamped, mode);
    }
}
```

> `total_frames()` 是 domain 派生函数(`Timeline.swift:16`)。钳制语义照搬 `seekToFrame`(`EditorViewModel.swift:260`:`min(max(0, frame), max(0, totalFrames))`)。`SeekMode`(exact / interactiveScrub)对应上游 `PreviewSeekMode`,scrub 节流(30Hz)在 render 后端做(Phase 4),core 只透传。

---

## 4. 前端只读镜像 + 版本号同步协议

### 4.1 协议(三条规则)

1. **快照都带版本号**:`get_timeline` 返回 `{ timeline: TimelineDTO, version: u64 }`。前端 Zustand 存 `{ mirror, mirrorVersion }`。
2. **写命令返回新版本号**:`edit_apply` / `undo` / `redo` 返回 `EditResult`(含 `timeline_version`)。前端可乐观地用返回值直接判断是否需重取,或纯靠事件(见 4.3)。
3. **事件触发重取**:收到 `timeline_changed{version}`,若 `version > mirrorVersion`,则 `invoke('get_timeline')` 重取并替换镜像 + 更新 `mirrorVersion`。`version <= mirrorVersion` 的事件**幂等忽略**(防重复/乱序)。

> 对应 ARCHITECTURE §2 原话:「每次 `edit_apply` 广播 `timeline_changed{version}`,前端据此重取。」`version` 即上游 `timelineRenderRevision` 的跨进程版,只是上游靠 `@Observable` 自动 diff,OpenTake 靠"版本号比较 + 显式重取"。

### 4.2 为何"重取整棵 timeline"而非增量 patch(明确取舍)

- 上游撤销/重做 = **整棵 timeline 替换**(`vm.timeline = undoState`,`ClipMutations.swift:232`),`Timeline` 是值类型整树(ARCHITECTURE §4「全是 Clone+Serialize+PartialEq 的值类型」)。OpenTake 沿用整树快照模型(§2.3、ARCHITECTURE §5「撤销栈在 Rust,整树快照」)。
- 因此**最简单且与撤销模型一致的同步 = 整树重取**。timeline 是元数据(帧号/引用/关键帧),非像素,JSON 体量可控;媒体像素走 §3.3 旁路,从不进 timeline DTO。
- **不做增量 patch**(CLAUDE.md「Simplicity First / 不做未要求的灵活性」):整树重取在正确性上零歧义(版本号即一致性令牌),patch 会引入顺序/丢失/合并的复杂度。若未来 profiling 证明大工程重取过重,再在**不破坏版本号契约**的前提下加 patch 通道。

### 4.3 乱序与并发(跨进程必须显式处理,上游不存在)

跨进程下,多个客户端(UI + 若干 MCP 连接)可并发 `edit_apply`。core 的 `Mutex` 串行化所有 `apply`,故 `version` 严格单调、无并发写竞争。但前端可能:
- 在 `get_timeline` 在途时收到更新的 `timeline_changed` → **以更高 `version` 为准**(收到的快照若 `version < latestEventVersion`,丢弃并重取)。
- 收到旧 `version` 事件(broadcast 缓冲)→ **幂等忽略**(规则 3)。

> 这是上游"单进程 @Observable"模型免费给的、OpenTake 必须自己保证的一致性。`version` 单调 + "永远向最高版本收敛" 是协议正确性的全部基础。**必须有集成测试**:并发触发 N 个 `edit_apply`,断言前端最终镜像 `version` == core 最终 `version` 且内容一致。

### 4.4 TimelineDTO 边界

`get_timeline` 返回的不是 Rust `Timeline` 本体,而是**面向前端的 DTO**(serde 序列化)。Phase 6 前端态(ARCHITECTURE §2:「Timeline 只读镜像 + UI-only 态(selection/zoom)」)只读 DTO,**永不写回**——所有变更经 `edit_apply`。DTO 可直接 `serde` 复用 domain 的 `Serialize`(与 `.opentake/project.json` 同 schema,ARCHITECTURE §9),保证"持久化格式 = 线上格式",减少一套映射。

---

## 5. 与 ops / project / render / agent 的装配关系

`opentake-core` 是**装配中枢**(ARCHITECTURE §3:「`opentake-core/` # 组装:EditorState…、command 路由、事件总线」)。依赖法则(ARCHITECTURE §3 末「依赖法则」):`domain` 零依赖叶子;`ops` 只依赖 `domain`;`command` 是唯一编辑入口;UI/Agent/MCP 是三个对等客户端。

### 5.1 依赖方向(谁依赖谁)

```
opentake-domain  ← ops ← core
                  ↑        ↑  ↑  ↑
            project ──────┘  │  │   (core 持 project 句柄做 open/save)
            render ─────────┘  │   (core 持 render 句柄做 seek/preview/export)
            media  ────────────┘   (core 持 media 句柄做 import/缩略图/波形)
            agent  →  core           (agent 依赖 core,反向:agent 是 core 的客户端)
            src-tauri → core, agent  (Tauri 装配二者)
```

- **core 依赖** `domain`(类型)、`ops`(`EditCommand`+`apply`)、`project`(读写)、`render`(seek/export)、`media`(import/物化)。
- **agent 依赖 core**(agent 是 core 的客户端,持 `EditorCore` 句柄,把工具翻译成 `EditCommand`)。**core 不依赖 agent**(单向),否则成环。
- **`src-tauri` 装配** core + agent + 前端,注册 `#[tauri::command]` 与事件桥(§3.2)。

### 5.2 `CoreDeps`:注入而非硬连(可测、解耦)

core 不直接 `use` render/media/project 的具体实现函数,而是持 trait 句柄(便于 Phase 1 单测时 mock,符合 CLAUDE.md 依赖注入/可测性):

```rust
// crates/opentake-core/src/deps.rs
pub struct CoreDeps {
    pub project: Arc<dyn ProjectStore>,   // open/save .opentake(opentake-project,Phase 2)
    pub media:   Arc<dyn MediaImporter>,  // 导入 + 物化 assets + 缩略图/波形(opentake-media,Phase 2)
    pub preview: Arc<dyn PreviewBackend>, // request_frame → emit PreviewFrame(opentake-render,Phase 4)
    pub export:  Arc<dyn ExportBackend>,  // start_export → emit ExportProgress(opentake-render,Phase 5)
    pub gen:     Option<Arc<dyn GenBackend>>, // BYOK/托管生成(opentake-gen,Phase 9;前期 None)
}
```

### 5.3 各装配点对应的上游证据

| 装配点 | core 做什么 | 上游对应 |
|---|---|---|
| **ops** | `apply` 内调 `ops::apply(&mut timeline, &cmd, &assets)`(§2.3 步骤 2) | `ToolExecutor.run` 各 case → `EditorViewModel` mutator(`ToolExecutor.swift:72-106`) |
| **project.open** | `open(dir)`:读 project.json/media.json/generation-log.json → 填 `EditorState`,调 media 物化 assets,version 归 0 | `VideoProject.read`(`:31-64`)+ `makeWindowControllers`(`:186-255`)装配顺序 + `restoreAssetsFromManifest`(`:304-339`) |
| **project.save** | `save()`:把 `timeline/manifest/generation_log` 序列化进 `.opentake` 目录 | `VideoProject.captureSaveSnapshot`(`:99-110`)+ `fileWrapper`(`:75-97`) |
| **media.import** | `import_media`:落地媒体 → 加 manifest entry → 物化 `MediaAsset` → 触发缩略图/波形(异步) → emit `MediaImported` | `ToolExecutor+Import.swift` + `VideoProject.swift:323-332`(restore 时生成缩略图/波形) |
| **render.seek/preview** | `seek` 透传 render 后端,后端 emit `PreviewFrame`(§3.4) | `EditorViewModel.seekToFrame`(`:259-267`)→ `videoEngine?.seek` |
| **render.export** | `export_start` 起后台导出 job,流式 emit `ExportProgress/Done/Failed` | `ExportService.export(format:resolution:)`(`ExportService.swift:73-159`) |
| **generation_log** | AI 生成成功后 append `generation_log`(append-only) | `EditorViewModel.generationLog`(`:31`)+ `seedGenerationLogFromAssets`(`VideoProject.swift:246`) |

### 5.4 装配顺序(open 流程,照搬上游 `makeWindowControllers`)

`VideoProject.swift:186-255` 的顺序是经实战的,必须照搬到 `EditorCore::open`:
1. 读并 decode `timeline`(`:31-42`)→ 设 `state.timeline`,`version = 0`。
2. 设 `project_dir` / 派生 `project_id`(`:192` + `EditorViewModel.swift:116-125`)。
3. decode `manifest`(`:43-50`)→ `state.manifest` → **从 manifest 物化 `assets`**(`restoreAssetsFromManifest`,`:304-339`):每个 entry 解析 URL → `MediaAsset` → 文件存在则触发波形/缩略图(异步)→ `loadMetadata`。
4. decode `generation_log`(`:51-53`);缺失则 `seed_generation_log_from_assets`(`:246`)。
5. `search_index.project_opened()`(`:248`,Phase 8 才实装)。
6. 不发 `timeline_changed`(open 是初始化,前端 open 后主动 `get_timeline`)。

> **容错**:所有 decode 用 `#[serde(default)] + Option`(ARCHITECTURE §9、§4「向后兼容容错解码」);`manifest`/`generation_log` 缺失或损坏**不**致命(上游 `loadedGenerationLog = try?`,`:52`),只 `timeline` 缺失才报错(`VideoProject.swift:32-34`)。

---

## 6. Tauri command 表面(精确签名草案)

> 全部 `#[tauri::command] async`,在 `src-tauri` 定义,**薄**(ARCHITECTURE §2:「Tauri command 边界(薄胶水:序列化 + 路由)」),body 仅:取 `State<EditorCore>` → 调 core 方法 → map 错误。**零业务逻辑**。命名采用 ARCHITECTURE §2 列出的集合。前端命名 camelCase,Rust snake_case(Tauri 自动转换;DTO 字段用 `#[serde(rename_all="camelCase")]`)。

### 6.1 命令清单

```rust
// ───────── 读 ─────────
#[tauri::command]
async fn get_timeline(core: State<'_, EditorCore>) -> Result<TimelineSnapshot, CmdError>;
// → { timeline: TimelineDTO, version: u64 }  (§4.1 规则1)

// ───────── 写(唯一编辑入口) ─────────
#[tauri::command]
async fn edit_apply(core: State<'_, EditorCore>, command: EditCommand) -> Result<EditResult, CmdError>;
// command: 见 §2.2;返回 EditResult(含 timeline_version, changed, summary)

#[tauri::command]
async fn undo(core: State<'_, EditorCore>) -> Result<EditResult, CmdError>;  // 全局撤销(Cmd+Z),§2.4
#[tauri::command]
async fn redo(core: State<'_, EditorCore>) -> Result<EditResult, CmdError>;

// ───────── 工程生命周期 ─────────
#[tauri::command]
async fn project_open(core: State<'_, EditorCore>, path: String) -> Result<TimelineSnapshot, CmdError>;
// 打开 .opentake 目录;成功后返回首个快照(§5.4)。对应 AppState.openProject(:143-154)

#[tauri::command]
async fn project_save(core: State<'_, EditorCore>, path: Option<String>) -> Result<(), CmdError>;
// path=None: 存回 project_dir(对应 autosave);path=Some: 另存为。对应 VideoProject.save(:66-73)

// ───────── 播放 / 预览(不进撤销栈) ─────────
#[tauri::command]
async fn seek(core: State<'_, EditorCore>, frame: i64, mode: SeekMode) -> Result<(), CmdError>;
// 钳制 + 透传 render 后端;帧经 preview_frame 事件回(§3.4)。对应 seekToFrame(:259)

// ───────── 媒体导入 ─────────
#[tauri::command]
async fn import_media(core: State<'_, EditorCore>, source: ImportSource) -> Result<ImportedMedia, CmdError>;
// ImportSource = Path(String) | Url(String) | Bytes{name,data}(ARCHITECTURE Phase2「本地/URL/bytes」)
// 返回 { assetId, ... };异步缩略图/波形就绪后另发事件。对应 ToolExecutor+Import.swift

// ───────── 导出(后台 job + 进度事件) ─────────
#[tauri::command]
async fn export_start(core: State<'_, EditorCore>, opts: ExportOptions) -> Result<ExportHandle, CmdError>;
// 立即返回 { jobId };进度走 export_progress 事件(§3.2)。对应 ExportService.export(:73)
```

### 6.2 关键参数类型(对齐上游)

```rust
pub enum SeekMode { Exact, InteractiveScrub }   // 对应 PreviewSeekMode(EditorViewModel.swift:259)

pub enum ImportSource {                          // ARCHITECTURE Phase 2「本地/URL/bytes,扩展名白名单」
    Path(String),
    Url(String),
    Bytes { name: String, data: Vec<u8> },
}

pub struct ExportOptions {                       // 对应 ExportService.export(format:resolution:)
    pub format: ExportFormat,                    // H264 | H265 | ProRes | Xml(对应 ExportService.swift:5 / ExportView.swift:13-16)
    pub resolution: ExportResolution,            // R720p | R1080p | R4K(对应 ExportView.swift:26 默认 1080p)
    pub output_path: String,
}
pub enum ExportFormat { H264, H265, ProRes, Xml }
pub enum ExportResolution { R720p, R1080p, R4K }

pub struct ExportHandle { pub job_id: String }   // 后续进度经事件(§3.2)
```

> **导出表面证据**:`ExportService.swift:73-78` `func export(... format: ExportFormat, resolution: ExportResolution)`;`ExportService.swift:5` `enum {h264,h265,prores,xml}`;`ExportView.swift:13-26` `VideoCodec{h264,h265}` + `ExportResolution` 默认 `.r1080p`。OpenTake 把 `xml`(FCPXML 导出,`XMLExporter.swift:40`)保留为 `ExportFormat::Xml`(纯逻辑,无需 wgpu,可早做)。**预设码率/profile** 对齐属 Phase 5(ARCHITECTURE §6、ROADMAP Phase 5),core 只定 `ExportOptions` 契约。

### 6.3 错误约定

```rust
#[derive(Serialize)]
pub struct CmdError { pub code: String, pub message: String }  // code 机读,message 人读
```

- 校验失败(如越界帧、未知 clipId)→ `code: "validation"`,`message` 带**精确路径**(ARCHITECTURE §7:`entries[3].startFrame: missing required field`,用 `serde_path_to_error`)。对应上游 `formatDecodingError`(`ToolExecutor.swift:210-229`)与各工具的 `entries[idx]: …` 报错(`ToolExecutor+Clips.swift:148-167`)。
- core 内部错(IO/解码)→ `code: "internal"`,`message` 友好化,详细上下文记日志(CLAUDE.md 错误处理:UI 友好 + 服务端详细)。
- **错误不致命**:命令失败时 timeline 不变、version 不变、无事件(§2.3 步骤 2 早返回)——前端镜像保持一致。

### 6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)

UI 直接传 `EditCommand` 给 `edit_apply`。Agent/MCP **不**经 `edit_apply` Tauri 命令(它们在 Rust 进程内直接持 `EditorCore` 句柄,§2.5),但**最终汇入同一个 `EditorCore::apply`**。即:`edit_apply` Tauri 命令 = UI 客户端的入口;`EditorCore::apply` = 三客户端的共同汇聚点。二者不重复——前者是后者的一个调用方。

---

## 7. 安全与并发边界(跨进程新增,必须显式)

| 关注点 | 措施 | 证据 / 依据 |
|---|---|---|
| **MCP 仅本机** | MCP server 绑 `127.0.0.1:19789`,**只 loopback** | `MCPHTTPServer.swift:25-26` `requiredLocalEndpoint = 127.0.0.1`;`MCPService.swift:9` port 19789 |
| **DNS-rebinding 防护** | Origin 校验 + Content-Type + Protocol-Version 三段校验管线(tower layer 复刻) | `MCPHTTPServer.swift:46-50` `StandardValidationPipeline([OriginValidator.localhost, ContentTypeValidator, ProtocolVersionValidator])`;ARCHITECTURE §7「只绑 loopback + Origin 校验」 |
| **MCP 属 agent crate** | 以上全在 `opentake-agent`(Phase 7),**不在 core** | core 不含网络;agent 持 `EditorCore` 句柄调 `apply` |
| **命令串行化** | 所有 `apply/undo/redo` 经 `Mutex<EditorState>` 串行;`version` 因此严格单调 | §2.3、§4.3 |
| **锁内无 IO** | 临界区只做值类型 timeline 操作;解码/导出/生成在锁外 task | §1.3 锁粒度、§5.2 deps 异步 |
| **密钥** | LLM/生成 key 存 OS keychain(`keyring`),不入工程文件 | ARCHITECTURE §8 末、§10;swift/security.md(Keychain) |

> core 自身**无外部攻击面**(不开端口、不收网络输入)。攻击面在 agent crate 的 MCP server,其安全契约见上表,实装在 Phase 7。本 crate 的安全责任 = 命令路径的输入校验(§6.3 精确路径错误)+ 并发一致性(§4.3)。

---

## 8. 实施清单

> 与 ROADMAP 对齐:opentake-core 横跨 Phase 1(命令路由/事务/version,可纯逻辑单测)与 Phase 6/7(Tauri 边界 + 事件桥)。下列按"先纯逻辑、可对拍,再接边界"排序(ROADMAP 关键里程碑顺序)。

### 阶段 A — 纯逻辑核(随 Phase 1,无 IO,可全单测)
1. **`EditorState` 结构**(§1.2):字段 = timeline+manifest+generation_log+undo+version+assets+project 引用。去掉一切 UI-only 态。
   - 验证:构造空 state,断言 `version == 0`、`dirty == false`。
2. **`EditorCore` + `Arc<Mutex<EditorState>>`**(§1.3),`Clone` 只复制 Arc。
   - 验证:克隆两个句柄,一个 `apply` 后另一个 `get_timeline` 读到新 `version`(共享同一 state)。
3. **`EditorCore::apply` 事务**(§2.3),逐句对应 `withTimelineSwap`:before 快照 → ops::apply → `before==after` 短路 → push undo + version+1 → emit。
   - 验证(对拍):喂上游导出的 `project.json`,跑等价 `EditCommand` 序列,断言结果 timeline 帧级一致(ROADMAP Phase 1「对拍测试」)。
   - 验证(不变量):无变更命令 `changed==false` 且 version 不变、无事件;有变更命令 version 恰好 +1。
4. **`undo()/redo()`**(§2.4):基于 `UndoStack`,**撤销也 bump version + emit**。
   - 验证:apply→undo 回到 before 且 version 递增;redo 回到 after;空栈 undo 返回 `NothingToUndo`。
5. **`EventBus`**(§3.1):`tokio::broadcast`,`emit` 无订阅者不 panic。
   - 验证:subscribe 后 apply,收到 `TimelineChanged{version}`,version 与返回值一致。

### 阶段 B — 装配(随 Phase 2/4/5,接 deps)
6. **`CoreDeps` trait 句柄**(§5.2):project/media/preview/export/gen,先用 mock 实现跑通 core,再接真实 crate。
7. **`open()/save()`**(§5.3/§5.4):照搬 `VideoProject` 装配顺序(timeline→manifest→物化 assets→generation_log);容错 decode(`#[serde(default)]`)。
   - 验证:打开上游导出工程还原 timeline(ROADMAP Phase 2);往返 save/open 无损;缺 manifest 不致命。
8. **`seek()`**(§3.4):钳制 `[0,total_frames]`(照搬 `seekToFrame`),透传 preview 后端。
9. **`import_media()`**(§5.3):落地 → manifest entry → 物化 asset → 异步缩略图/波形 → `MediaImported` 事件。
10. **`export_start()`**(§6.2):起后台 job,流式 `ExportProgress/Done/Failed`。

### 阶段 C — Tauri 边界(随 Phase 6)
11. **`#[tauri::command]` 薄封装**(§6.1):8+3 命令,body 仅取 State→调 core→map `CmdError`。
12. **事件桥 task**(§3.2):src-tauri 启动时 `subscribe()`,把 `CoreEvent` map 成前端 `emit`。
13. **`preview_frame` 像素旁路**(§3.3):与 render 后端协商 Channel/asset 协议(契约在 core,传输在 src-tauri+render)。
14. **`TimelineDTO`**(§4.4):serde 复用 domain 序列化(= project.json schema)。
15. **前端同步协议**(§4):Zustand `{mirror, mirrorVersion}` + `timeline_changed` 幂等重取(前端代码,Phase 6;core 侧只保证 version 契约)。

### 阶段 D — Agent 接入点(随 Phase 7,core 仅需就绪)
16. 确认 `EditorCore` 句柄可被 `opentake-agent` 克隆持有;agent 工具层把工具 args → `EditCommand` → `core.apply`(§2.5)。core **不**改动——它对 agent 是只读 API 契约。
17. 助手专属 undo 的 `AgentUndoCursor` 在 agent 层实现,基于 core 的通用 `undo()` + version 追踪(§2.4),复刻 `ToolExecutor.undo` 拒绝语义(`ToolExecutor.swift:108-123`)。

### 横切验证(贯穿)
- **并发一致性集成测试**(§4.3):N 路并发 `apply`,断言最终 version == 命令次数(全变更时)、前端镜像内容与 core 一致。
- **覆盖率 ≥ 80%**(CLAUDE.md testing)。事务核(§2.3)与 version 不变量是重点。
- **不变量断言固化为测试**:version 单调;无变更不发事件;撤销/重做 version 行为(§1.2、§2.4)。

---

## 附录:opentake-core 不做什么(边界纪律)

- **不含编辑算法**:overlap/ripple/split/keyframe 求解全在 `opentake-ops`(§2.2)。core 只起事务、调 `ops::apply`。
- **不含 UI 态**:selection/zoom/panel/scrub 在前端 Zustand(§1.1)。
- **不含网络**:MCP server / LLM 客户端在 `opentake-agent`(§7)。core 无端口、无外部输入面。
- **不含像素/解码/编码**:全在 `opentake-render` / `opentake-media`,经 `CoreDeps` 注入(§5.2)。core 只持句柄、定事件契约。
- **不含撤销算法细节**:`UndoStack`(整树快照)在 `opentake-ops`;core 只调用。
- 一句话:**core = 装配 + 路由 + 事务 + 版本 + 事件**,其余皆为它编排的对象。
