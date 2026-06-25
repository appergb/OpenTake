# opentake-agent 实现就绪规格（Issue #9）

> 范围：`crates/opentake-agent` —— rmcp MCP server（`127.0.0.1:19789`）+ 31 个工具 + 短 ID 系统 + 统一执行壳 + 面向 LLM 的精确路径错误 + 应用内 chat（reqwest→Anthropic，BYOK + prompt cache）+ **Agent Context Signal 注入** + **Workflow Plugin 系统**。
>
> 设计来源（已逐行核读）：上游 `palmier-pro-upstream/Sources/PalmierPro/Agent/`（29 文件），以及 OpenTake `docs/AGENT-CONTEXT-SIGNAL.md`、`docs/WORKFLOW-PLUGIN-SYSTEM.md`、`docs/ARCHITECTURE.md §7/§9`、`docs/MODULE-PORT-MAP.md`「Agent」、`docs/_analysis/04-MCP与Agent工具.md`、`docs/ROADMAP.md` Phase 7/S/W。
>
> 核心架构原则（上游验证，OpenTake 照搬）：**编辑能力只有一处真实定义**（`opentake-core` 的 `EditCommand` 路由 → `opentake-ops`），**MCP server 与应用内 chat 是它的两个对等前端**，不写两套。Agent 层「非常薄」——31 个工具是 `opentake-core` 命令的薄包装；真正的编辑算法在 `opentake-ops`/`opentake-domain`（不在本 crate）。
>
> 约束注记：本规格只描述 `opentake-agent`。它**不实现**编辑算法（`opentake-ops`）、领域模型（`opentake-domain`）、媒体引擎（`opentake-media`）、渲染（`opentake-render`）、生成后端（`opentake-gen`）——这些由各自 crate 提供，本 crate 通过 `opentake-core` 调用（见 §8）。

---

## 目录

0. [上游证据索引](agent/0-evidence-index.md)
1. [rmcp MCP server](agent/1-mcp-server.md)
2. [31 个工具完整提取](agent/2-tools.md)
3. [短 ID 系统](agent/3-short-id.md)
4. [统一执行壳 + 面向 LLM 的精确路径错误](agent/4-execution-shell.md)
5. [应用内 chat](agent/5-chat.md)
6. [Context Signal 注入](agent/6-context-signal.md)
7. [Workflow Plugin (系统提示词)](agent/7-system-prompt.md)
8. [opentake-core 命令路由](agent/8-core-dispatch.md)
9. [遥测与日志](agent/9-telemetry.md)
10. [实施清单与验证](agent/10-implementation.md)
