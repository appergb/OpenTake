## 0. 设计基线:跨进程改变了什么(必读)

上游是**单进程、单实例**:UI(SwiftUI)、应用内 chat(`AgentService`)、MCP server(`MCPService`)全部活在同一个 macOS 进程里,共享同一个 `EditorViewModel` 引用(证据:`AppState.swift:23-27` MCPService 的 `editorProvider` 闭包返回 `activeProject?.editorViewModel`;`EditorViewModel.swift:178` `agentService.editor = self`)。因此三客户端**天然看到同一份内存里的 `timeline`**,无需任何同步协议——SwiftUI 靠 `@Observable` 自动重渲,MCP/chat 直接读 `editor.timeline`。

OpenTake 是 **Rust core + Tauri + React 跨「逻辑进程边界」**(前端在 WebView,core 在 Rust 侧):
- 前端**不能**持有权威 `Timeline`,只能持**只读镜像**;
- 必须由 Rust 侧单一持有权威 `Timeline`;
- 每次编辑后,前端镜像必须靠**单调递增版本号**(对应上游 `timelineRenderRevision`,`EditorViewModel.swift:27-28,76`)判定失效并重取。

> **§2 架构原话**:「真相源在 Rust,前端只持镜像……前端拿快照 + 单调递增版本号(对应上游 `timelineRenderRevision`),每次 `edit_apply` 广播 `timeline_changed{version}`,前端据此重取。」本 crate 即落地这句话。

**一个必须显式处理、上游"免费"得到的事实**:上游三客户端共享同一个 `EditorViewModel`,但**各自持有独立的 ToolExecutor 实例**——MCP 经 `MCPService.init → ToolExecutor(editorProvider:)`(`MCPService.swift:31-33`),应用内 chat 经 `AgentService.swift:215 didSet { toolExecutor = editor.map { ToolExecutor(editor: $0) } }`。两个 executor 实例**各自维护自己的 `agentUndoStack`**(`ToolExecutor.swift:20`),但都对**同一个 `EditorViewModel.undoManager`**(同一棵撤销树)操作。OpenTake 必须复刻这个分工:**编辑逻辑唯一一份(`EditorCore`),撤销栈唯一一份(在 `EditorState`),但"助手专属 undo 游标"按客户端会话隔离**(见 §2.4)。

---
