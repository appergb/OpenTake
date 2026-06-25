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

## 目录

0. [设计基线:跨进程改变了什么(必读)](core/0-design-baseline.md)
1. [EditorState 结构](core/1-editor-state.md)
2. [命令路由 = 上游 ToolExecutor 单一能力层](core/2-command-routing.md)
3. [事件总线](core/3-event-bus.md)
4. [前端只读镜像 + 版本号同步协议](core/4-frontend-sync.md)
5. [与 ops / project / render / agent 的装配关系](core/5-assembly.md)
6. [Tauri command 表面(精确签名草案)](core/6-tauri-commands.md)
7. [安全与并发边界(跨进程新增,必须显式)](core/7-security.md)
8. [实施清单](core/8-implementation.md)
