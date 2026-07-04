# opentake-agent — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> `opentake-agent` = 工具层（= 上游 `ToolExecutor`，**44 工具**）+ MCP server（rmcp Streamable-HTTP，`127.0.0.1:19789/mcp`）+ Context Signal + 工作流插件 + 内置 Agent 提示。能力层：依赖 `domain / ops / core / media / gen`，被 `src-tauri` 集成启动。

---

## 总览

- **[OVERVIEW.md](OVERVIEW.md)** — 一句话定位与依赖分层、职责边界、关键概念与数据流（MCP 网络面 / 派发归一 / Context Signal / 工作流插件 / 内置提示）、对应上游 Swift、完成状态（已实现 vs stub）、工具总数 44（31 上游 + 13 扩展）。

## 子系统文档

- **[mcp-server.md](mcp-server.md)** — `mcp/server.rs`：rmcp `StreamableHttpService` 网络面、端口 `127.0.0.1:19789`、`Host`/`Origin` 回环守卫（DNS-rebinding 防御）、OAuth well-known、`McpServer`（`get_info`/`list_tools`/`call_tool`）+ `serve()`。
- **[dispatch-tools.md](dispatch-tools.md)** — `mcp/dispatch.rs` 统一派发壳（单一能力管线，归一到 `EditCommand` + agent-undo 栈）+ `tools/`：`names`（44 工具枚举 / `ALL` / `UPSTREAM`）、`args`（类型化参数）、`descriptions`（逐字描述 + Schema）、`errors`（精确路径错误）、`result`（中立结果类型）、`short_id`（短 id 展开/缩短）、`encode_timeline`（`get_timeline` 紧凑编码）。
- **[core-handle-convert.md](core-handle-convert.md)** — `mcp/core_handle.rs`（`CoreHandle` 窄接口 + `AppCoreHandle` 生产实现，适配 `AppCore`）+ `mcp/convert.rs`（`ToolResult` → rmcp `CallToolResult`）+ `mcp/gen_catalog.rs`（`list_models` 投影 `opentake-gen` 静态目录）。
- **[context-signal.md](context-signal.md)** — `signal/`：`classify`（视频类型结构化判定）、`track_roles`（轨道角色检测 + 逐角色建议）、`stages`（剪辑阶段推断 + 阶段指引 + 剪辑骨架）、`rules`（内置规则告警 + `OpContext`）、`engine`（构建并附挂 `context_signal`，应用插件覆盖）。对应 [AGENT-CONTEXT-SIGNAL.md](AGENT-CONTEXT-SIGNAL.md)。
- **[plugin-system.md](plugin-system.md)** — `plugin/`：`model`（`plugin.json` 容错 serde 模型）、`registry`（扫描/校验/激活 + 内置 `audio-first`）、`rules`（插件 `dont` 规则层）、`builtin/audio-first/`（编译进二进制的默认工作流）。对应 [WORKFLOW-PLUGIN-SYSTEM.md](WORKFLOW-PLUGIN-SYSTEM.md)。
- **[prompt.md](prompt.md)** — `prompt/`：`base`（分段 base 系统提示，移植自上游 `AgentInstructions`，契约关键句逐字保留）、`assemble`（base + 激活插件 `instructions.md` / 轨道角色 / 规则，不可信围栏）。

## 规格与设计（只读，本目录已有）

- **[SPEC.md](SPEC.md)** — 模块完整规格（§2-4 工具层、§6 Context Signal、§7 工作流插件、§8 派发壳与 MCP、§9 安全）。
- **[AGENT-CONTEXT-SIGNAL.md](AGENT-CONTEXT-SIGNAL.md)** — Context Signal 设计：视频类型 / 轨道角色 / 剪辑阶段 / 剪辑骨架 / 规则。
- **[WORKFLOW-PLUGIN-SYSTEM.md](WORKFLOW-PLUGIN-SYSTEM.md)** — 工作流插件系统设计：`plugin.json` 模型、注册/校验/激活、与提示及信号的关系。

## 相关跨切面（架构）

- [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md) — 逐模块上游 Swift → Rust 移植地图（**Agent** 段：`ToolExecutor` / `MCPHTTPServer` / `AgentInstructions` 等处置）。
- [ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) — 总体架构：单一真理状态 + 命令事务（唯一编辑入口 `EditCommand`）。
- [ROADMAP.md](../../architecture/ROADMAP.md) — 分阶段路线图（MCP server / Agent 阶段）。
- [ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md) — A-tier 着色效果（`set_color_grade` / `chroma_key` / `set_mask` / `apply_effect` 工具对应能力）。

## 上游拆解参考

- [04-MCP与Agent工具.md](../../upstream-analysis/04-MCP与Agent工具.md) — 上游 MCP 与 Agent 工具逐项拆解（工具集、ShortId、ToolExecutor、MCPHTTPServer）。
- [01-架构与数据流.md](../../upstream-analysis/01-架构与数据流.md) — 上游整体架构与数据流。
- [03-闭源云边界.md](../../upstream-analysis/03-闭源云边界.md) — 上游闭源云边界（Anthropic / Convex / Clerk；本模块对应的 BYOK 改造方向）。

## 相关模块

- [opentake-ops](../opentake-ops/INDEX.md) — `EditCommand` + `apply` 事务（本模块所有编辑工具的落地终点）。
- [opentake-core](../opentake-core/INDEX.md) — `AppCore` 会话（`CoreHandle` 适配的权威真理）。
- [opentake-media](../opentake-media/INDEX.md) — PCM 抽取 + 节拍/静音分析（分析驱动工具的后端）。
- [opentake-gen](../opentake-gen/INDEX.md) — 生成模型静态目录（`list_models` 数据源）。
- [opentake-motion](../opentake-motion/INDEX.md) — Motion Canvas（`add_motion_graphic` 计划接线目标，Issue #34）。
- [src-tauri](../src-tauri/INDEX.md) — 桌面壳：`src-tauri/src/mcp.rs` 构建注册表并启动 MCP server。

## 源码

```
crates/opentake-agent/src/
├── lib.rs                  模块声明（mcp / plugin / prompt / signal / tools）
├── mcp/
│   ├── mod.rs              派发壳模块说明
│   ├── server.rs           rmcp Streamable-HTTP server + loopback 守卫 + serve()
│   ├── dispatch.rs         统一派发壳 Dispatcher（归一到 EditCommand + agent-undo 栈）
│   ├── core_handle.rs      CoreHandle 窄接口 + AppCoreHandle 生产实现
│   ├── convert.rs          ToolResult → rmcp CallToolResult
│   └── gen_catalog.rs      list_models 投影 opentake-gen 目录
├── tools/
│   ├── mod.rs              工具层模块说明
│   ├── names.rs            ToolName 枚举 + ALL(44) / UPSTREAM(31)
│   ├── args.rs             类型化参数结构 + ALLOWED_KEYS
│   ├── descriptions.rs     工具描述（逐字）+ 输入 Schema
│   ├── errors.rs           ToolError + 精确路径解码错误
│   ├── result.rs           中立 ToolResult / Block 类型
│   ├── short_id.rs         短 id 展开（入）/ 缩短（出）
│   └── encode_timeline.rs  get_timeline 紧凑编码
├── signal/
│   ├── mod.rs              Context Signal 模块说明
│   ├── classify.rs         视频类型结构化自动判定
│   ├── track_roles.rs      轨道角色检测 + 逐角色建议
│   ├── stages.rs           剪辑阶段推断 + 阶段指引 + 剪辑骨架
│   ├── rules.rs            内置规则告警 + OpContext
│   └── engine.rs           构建并附挂 context_signal（应用插件覆盖）
├── plugin/
│   ├── mod.rs              工作流插件模块说明
│   ├── model.rs            plugin.json 容错 serde 模型
│   ├── registry.rs         注册表（扫描/校验/激活）+ 内置插件
│   ├── rules.rs            插件 dont 规则层
│   └── builtin/audio-first/   内置默认工作流（plugin.json + instructions.md）
└── prompt/
    ├── mod.rs              系统提示模块说明
    ├── base.rs             分段 base 系统提示（逐字契约句）
    └── assemble.rs         base + 激活插件注入（不可信围栏）
```

源文件树根：`../../../crates/opentake-agent/src/`

---

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
