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
