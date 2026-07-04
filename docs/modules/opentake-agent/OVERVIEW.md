# opentake-agent — 总览

> 上级：[模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)

---

## 一句话定位

`opentake-agent` = OpenTake 的 **AI Agent 子系统**：把"单一能力层 + 多前端"模型落到 Rust——一套工具层（= 上游 `ToolExecutor`）经统一派发壳归一到 `EditCommand`，对外由本地 **MCP server**（rmcp Streamable-HTTP，`127.0.0.1:19789/mcp`）暴露；同时软件**主动**向模型回发 Context Signal（视频类型 / 轨道角色 / 剪辑阶段 / 规则告警），并以**工作流插件**注入领域剪辑知识。

### 依赖分层（依赖只能向下）

```
opentake-domain   值语义（Timeline/Clip/VideoType/TrackRole/ContextSignal…）
opentake-ops      EditCommand + apply 事务（唯一编辑入口）
opentake-media    PCM 抽取 + 节拍/静音分析（detect_beats / detect_silences）
opentake-gen      生成模型静态目录（list_models 的数据源，BYOK）
opentake-core     AppCore 会话（CoreHandle 适配的真理）
   ▲
opentake-agent    ← 本模块（能力层）
   ▲
src-tauri         桌面壳：build_registry + server::serve 起 MCP（src-tauri/src/mcp.rs）
```

本 crate 依赖 `domain / ops / core / gen / media`（见 `crates/opentake-agent/Cargo.toml`），被 `src-tauri` 集成启动。

---

## 职责边界

**做什么**

- 定义 **44 个工具**的名称 / 描述 / JSON Schema / 类型化参数（= 上游工具单一事实源）。
- 一条统一派发管线包裹**每个**工具：解析名 → 快照 → 展开短 id → 解码（精确路径错误）→ 跑 body → 附 Context Signal → 缩短 id。
- 把编辑类工具归一到 `opentake-ops::EditCommand`，经 `CoreHandle` 应用到权威 `AppCore`。
- 起 rmcp MCP server（回环绑定 + Origin/Host 守卫 + OAuth well-known）。
- 生成并附挂 Context Signal；维护工作流插件注册表；组装内置 Agent 系统提示。
- 维护**会话级** agent-undo 栈（`undo` 只回退本会话 Agent 自己的编辑）。

**不做什么**

- 不持有领域编辑逻辑——所有变更都委托 `opentake-ops`（本 crate 只构造 `EditCommand`）。
- 不做撤销/重做底层实现（整树快照栈在 `ops` / `core`）。
- 不直接触 `AppCore`——只经窄接口 `CoreHandle`（可注入测试假实现）。
- 不含 LLM 聊天 UI（前端 React 重建）；本阶段也**未落地**应用内聊天客户端的网络循环。

---

## 关键概念与数据流

### 1. MCP server（网络面）

rmcp 的 `StreamableHttpService` 挂在 `/mcp`，默认绑 `127.0.0.1:19789`（`server::DEFAULT_ADDR`）。axum 路由外层套一个 **loopback 守卫**：`Host` / `Origin` 头若存在且非 `localhost`/`127.0.0.1`/`::1` 一律 403（DNS-rebinding 防御；头缺省放行，原生 MCP 客户端常不带 `Origin`）。另暴露 `/.well-known/oauth-protected-resource` 明确"无需鉴权"。`get_info` 广告系统提示 + tools 能力，`list_tools` 返回 44 个工具 schema，`call_tool` 经 `spawn_blocking` 派发。详见 [mcp-server.md](mcp-server.md)。

### 2. 工具派发层（归一到 EditCommand）

`Dispatcher::dispatch` 是唯一管线（= 上游 `ToolExecutor.execute`）。编辑类工具 body 解码类型化参数后构造一个 `EditCommand`，经 `CoreHandle::apply` 走 `ops` 的 `withTimelineSwap` 事务；读类工具序列化状态。短 id 系统在入口展开前缀、出口缩短为项目唯一前缀（≥8 字符）。详见 [dispatch-tools.md](dispatch-tools.md)、[core-handle-convert.md](core-handle-convert.md)。

### 3. Context Signal（软件主动发信号）

工具跑完后（短 id 缩短前），`signal::engine::attach` 给**带信号的工具**结果追加一个 `context_signal` JSON 块：`get_timeline` 附完整信号（视频类型 + 轨道角色 + 阶段指引 + 剪辑骨架 + 逐轨建议），写工具附规则告警。这是"软件主动告诉模型该怎么剪"，对应 `AGENT-CONTEXT-SIGNAL.md` 设计。详见 [context-signal.md](context-signal.md)。

### 4. 工作流插件

纯 JSON（`plugin.json`，无 Rust 编译 / 无 WASM）声明视频类型覆盖、轨道角色、分阶段动作提示、do/dont 规则。`PluginRegistry` 从磁盘扫描 + 校验 + 激活；激活后其 `instructions.md` 进系统提示、其覆盖项进 Context Signal（优先级：插件 > 手动 > 自动）。内置一个 `audio-first`（音频先入）默认工作流，`include_str!` 编译进二进制。详见 [plugin-system.md](plugin-system.md)、`WORKFLOW-PLUGIN-SYSTEM.md`。

### 5. 内置 Agent 提示

分段 base 提示（移植自上游 `AgentInstructions`，Palmier→OpenTake，契约关键句逐字保留）+ 激活插件的 `instructions.md` / 轨道角色 / 规则。插件内容被"不可信"围栏包裹，防冒充系统指令。详见 [prompt.md](prompt.md)。

### 数据流（一次工具调用）

```
MCP 客户端 → /mcp (loopback 守卫) → McpServer::call_tool
  → Dispatcher::dispatch
      1 解析 ToolName（未知→error）
      2 快照 before = timeline / manifest = media
      3 展开入站短 id 前缀
      4 解码类型化参数（精确路径错误）
      5 跑 body：编辑工具 → EditCommand → CoreHandle::apply → AppCore（ops 事务）
                 读工具   → 序列化状态
      6 signal::engine::attach 附 context_signal
      7 缩短出站 id → ToolResult
  → convert::to_call_tool_result → CallToolResult
```

---

## 对应上游 Swift

见 [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md) 的 **Agent** 段（`mixed → needs-replacement`）与 [`../../upstream-analysis/04-MCP与Agent工具.md`](../../upstream-analysis/04-MCP与Agent工具.md)。

| 上游 Swift | 本模块 Rust | 处置 |
|---|---|---|
| `ToolExecutor` + 13 扩展 | `mcp::dispatch` + `tools/` | direct-port（核心能力） |
| `ToolDefinitions / ToolName / AgentTool` | `tools::names` / `tools::descriptions` / `tools::args` | 工具单一事实源，schema 照抄 |
| `ToolExecutor+ShortId` | `tools::short_id` | 1:1 |
| `ToolExecutor+Timeline`（紧凑编码） | `tools::encode_timeline` | 1:1 |
| `ToolResult` | `tools::result` | 1:1 |
| `MCPHTTPServer / MCPService`（NWListener 手写 HTTP） | `mcp::server`（rmcp + axum） | needs-replacement（换 rmcp） |
| `AgentInstructions` | `prompt::base` / `prompt::assemble` | 逐字移植，改产品名 |
| `AgentService / AgentClient / AnthropicClient / PalmierClient`（SSE 聊天循环、Convex/Clerk 计费） | — | **未移植**（计划：reqwest+SSE，BYOK 直连） |
| Context Signal / 工作流插件 | `signal/` / `plugin/` | **OpenTake 新增**（上游无对应） |

---

## 完成状态：已实现 vs 计划中

### 已实现

- **44 工具的名称 / 描述 / Schema / 类型化参数**（`tools/`，描述逐字移植）。
- **统一派发管线**全链路（解析 → 快照 → 短 id 展开 → 解码 → body → 信号 → 短 id 缩短）。
- 编辑类工具接线到 `EditCommand`：`add_clips` / `insert_clips` / `move_clips` / `remove_clips` / `remove_tracks` / `split_clip` / `set_keyframes` / `ripple_delete_ranges` / `add_texts` / `set_clip_properties` / `create_folder` / `move_to_folder` / `rename_media` / `rename_folder` / `delete_media` / `delete_folder` / `undo`，以及 A-tier 效果 `set_color_grade` / `chroma_key` / `set_mask` / `apply_effect`。
- 读类工具：`get_timeline`（紧凑编码）/ `get_media` / `list_folders` / `list_models`（读 `opentake-gen` 静态目录，纯本地）。
- 分析驱动工具：`detect_beats` / `auto_cut_to_beats` / `tighten_silences`（经 `CoreHandle::extract_analysis_pcm` + `opentake-media` 分析，**返回预览/建议，不直接落地**——`applied:false`，由模型再调编辑工具落地）。
- **MCP server**：rmcp Streamable-HTTP + loopback 守卫 + OAuth well-known + `serve()`，已被 `src-tauri/src/mcp.rs` 集成启动。
- **Context Signal**：视频类型自动判定 / 轨道角色检测 + 逐轨建议 / 剪辑阶段推断 + 阶段指引 / 内置规则告警 + 插件规则。
- **工作流插件**：JSON 模型 + 注册表（扫描/校验/激活）+ 内置 `audio-first` + 三个工作流工具（`list_workflows` / `activate_workflow` / `deactivate_workflow`）。
- **系统提示**：分段 base + 插件围栏注入。
- **agent-undo 栈**：会话级，仅回退本会话 Agent 编辑。

### 计划中 / stub（如实标注）

- **honest stub**（解码参数后直接返回 "not yet implemented"）：`inspect_media` / `get_transcript` / `inspect_timeline` / `search_media`（需更宽的媒体后端 CoreHandle）、`generate_video` / `generate_image` / `generate_audio` / `upscale_media`（需异步 GenClient + BYOK 鉴权）、`import_media`、`add_captions`、`add_motion_graphic` / `edit_motion_graphic`（Motion Canvas，Issue #34）。
- `smart_reframe`：返回错误（需视觉/显著性分析后端，`CoreHandle` 尚未暴露采样帧）。
- `get_timeline` 的 `canGenerate` 恒为 `false`（生成后端未接线，让模型不会提议生成）。
- `create_folder` / `move_to_folder` 的批量 `entries` 形式未接线（仅单条形式）。
- **应用内聊天客户端**（`AgentService` 等价的 SSE 工具循环、BYOK Anthropic 直连）尚未落地。

## 工具总数：**44 个**

= **31 个上游对齐**（`tools::names::UPSTREAM`）+ **13 个 OpenTake 扩展**（`ALL` 减 `UPSTREAM`）。源：`crates/opentake-agent/src/tools/names.rs` 的 `ALL`（44）/ `UPSTREAM`（31）常量。

13 个扩展 = 分析驱动 4（`detect_beats` / `auto_cut_to_beats` / `smart_reframe` / `tighten_silences`）+ 工作流插件 3（`activate_workflow` / `list_workflows` / `deactivate_workflow`）+ A-tier 着色效果 4（`set_color_grade` / `chroma_key` / `set_mask` / `apply_effect`）+ Motion Canvas 2（`add_motion_graphic` / `edit_motion_graphic`）。

---

> 目录：[INDEX.md](INDEX.md) · 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
