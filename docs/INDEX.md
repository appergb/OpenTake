# OpenTake 文档索引

> 本文档是 OpenTake 项目所有规划文档的索引，使用链接将各文档关联起来。
> 最后更新：2026-06-26

---

## 📋 快速导航

| 你想了解什么 | 文档 |
|---|---|
| **项目概览** | [README.zh-CN.md](../README.zh-CN.md) |
| **总体架构** | [ARCHITECTURE.md](ARCHITECTURE.md) |
| **当前阶段与路线图** | [ROADMAP.md](ROADMAP.md) |
| **AI Agent 协作指南** | [AGENTS.md](../AGENTS.md) |
| **技术选型决策** | [DECISIONS.md](../DECISIONS.md) |
| **变更历史** | [CHANGELOG.md](../CHANGELOG.md) |
| **贡献指南** | [CONTRIBUTING.md](../CONTRIBUTING.md) |
| **Tauri 桌面壳说明** | [src-tauri/README.md](../src-tauri/README.md) |
| **已知 Bug 与问题** | [BUGS.md](BUGS.md) |

---

## 🏗️ 架构与规划

| 文档 | 行数 | 内容概要 |
|---|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | 213 | 总体架构设计：分层 crate 结构、数据流、领域模型、渲染管线、Agent 集成 |
| [ROADMAP.md](ROADMAP.md) | 129 | 10 个阶段路线图：Phase 0 脚手架 → Phase 10 Motion Canvas 插件 |
| [EDITING-ENGINE-PLAN.md](EDITING-ENGINE-PLAN.md) | 73 | 剪辑引擎现况与规划：已 1:1 移植的 ops 层 + 待收口的 gap |
| [PORT-1TO1-GAP.md](PORT-1TO1-GAP.md) | 198 | 1:1 复刻差距与实现计划：P0/P1/P2 逐项差距 |
| [ADVANCED-FEATURES.md](ADVANCED-FEATURES.md) | 117 | 进阶能力设计：wgpu 着色器、AI 推理、FFmpeg 音频工程、跨平台特性 |
| [CAPCUT-GAP.md](CAPCUT-GAP.md) | 236 | OpenTake vs 剪映特性差距报告（逐模块核对） |
| [BUGS.md](BUGS.md) | — | 实际发现的 Bug 和有问题部分（本文档创建时维护） |

**依赖关系**：ROADMAP → EDITING-ENGINE-PLAN → PORT-1TO1-GAP → BUGS

---

## 🧩 模块规格文档（specs/）

| 文档 | 行数 | 对应 Crate | 内容概要 |
|---|---|---|---|
| [specs/core-SPEC.md](specs/core-SPEC.md) | 585 | `opentake-core` + `opentake-ops` | 核心引擎规格：Timeline 模型、编辑命令、IPC 协议、撤销/重做 |
| [specs/media-SPEC.md](specs/media-SPEC.md) | 966 | `opentake-media` | 媒体引擎规格：FFmpeg 编解码、缩略图、波形、转写、语义搜索 |
| [specs/render-SPEC.md](specs/render-SPEC.md) | 565 | `opentake-render` | 渲染管线规格：RenderPlan、wgpu 合成器、文本栅格化 |
| [specs/agent-SPEC.md](specs/agent-SPEC.md) | 1,089 | `opentake-agent` | Agent/MCP 规格：31 工具定义、Context Signal、工作流插件 |
| [specs/gen-SPEC.md](specs/gen-SPEC.md) | 893 | `opentake-gen` | 生成式 AI 规格：BYOK Provider、模型目录、生成参数 |
| [specs/frontend-UI-1to1-SPEC.md](specs/frontend-UI-1to1-SPEC.md) | 1,340 | `web/` | 前端 UI 1:1 规格：布局系统、时间线、预览、检查器、工具 |

---

## 📐 上游分析参考（_analysis/）

| 文档 | 行数 | 内容概要 |
|---|---|---|
| [_analysis/01-架构与数据流.md](_analysis/01-架构与数据流.md) | 372 | 上游 Palmier Pro 架构全面拆解：应用启动链、三层顶层对象、核心领域模型 |
| [_analysis/02-苹果框架可移植性.md](_analysis/02-苹果框架可移植性.md) | 153 | Apple 框架 → Rust 可移植性评估：AVFoundation/AppKit/SwiftUI 替代方案 |
| [_analysis/03-闭源云边界.md](_analysis/03-闭源云边界.md) | 302 | 后端闭源边界分析：Convex+Clerk+Stripe → OpenTake 自建建议 |
| [_analysis/04-MCP与Agent工具.md](_analysis/04-MCP与Agent工具.md) | 255 | MCP 与 Agent 工具拆解：31 工具全集按域分组 |

---

## 🤖 Agent 相关文档

| 文档 | 行数 | 内容概要 |
|---|---|---|
| [AGENT-CONTEXT-SIGNAL.md](AGENT-CONTEXT-SIGNAL.md) | 256 | Agent Context Signal 设计：信号发射时机、数据结构、工作流插件增强 |
| [WORKFLOW-PLUGIN-SYSTEM.md](WORKFLOW-PLUGIN-SYSTEM.md) | 156 | 工作流插件系统设计：plugin.json schema、规则系统、Agent 集成 |
| [MOTION-GRAPHICS-PLUGIN.md](MOTION-GRAPHICS-PLUGIN.md) | 280 | Motion Canvas 动效插件规划：技术路线、模块边界、Agent 集成 |
| [specs/agent-SPEC.md](specs/agent-SPEC.md) | 1,089 | Agent 模块完整规格 |

---

## 🔬 扫描与审查报告

| 文档 | 行数 | 内容概要 |
|---|---|---|
| [FULL_PROJECT_SCAN_REPORT.md](FULL_PROJECT_SCAN_REPORT.md) | 180 | OpenTake vs palmier-pro-upstream 全项目扫描报告 |
| [MODULE-PORT-MAP.md](MODULE-PORT-MAP.md) | 1,295 | 逐模块移植地图：20 个模块的上游拆解 + Rust 对应方案 |

---

## 🔗 文档引用关系图

```
README.zh-CN.md ──→ ARCHITECTURE.md ──→ ROADMAP.md
                      │                    ├── EDITING-ENGINE-PLAN.md
                      │                    │     └── PORT-1TO1-GAP.md
                      │                    │           └── BUGS.md
                      │                    ├── ADVANCED-FEATURES.md
                      │                    └── CAPCUT-GAP.md
                      │
                      ├── specs/
                      │     ├── core-SPEC.md ←→ crates/opentake-core, opentake-ops
                      │     ├── media-SPEC.md ←→ crates/opentake-media
                      │     ├── render-SPEC.md ←→ crates/opentake-render
                      │     ├── agent-SPEC.md ←→ crates/opentake-agent
                      │     ├── gen-SPEC.md ←→ crates/opentake-gen
                      │     └── frontend-UI-1to1-SPEC.md ←→ web/
                      │
                      ├── _analysis/
                      │     └── 01/02/03/04 → 上游反推参考
                      │
                      ├── AGENT-CONTEXT-SIGNAL.md → specs/agent-SPEC.md
                      ├── WORKFLOW-PLUGIN-SYSTEM.md → specs/agent-SPEC.md
                      └── MOTION-GRAPHICS-PLUGIN.md → specs/render-SPEC.md

AGENTS.md ──→ docs/ARCHITECTURE.md, ROADMAP.md, MODULE-PORT-MAP.md
DECISIONS.md ──→ LICENSE
CLAUDE.md ──→ 常用命令、架构大图、移植铁律
```

---

## 📊 文档统计

| 位置 | 文档数 | 总行数 |
|---|---|---|
| `docs/`（根目录） | 11 + 索引 | 3,132 |
| `docs/specs/` | 6 | 5,438 |
| `docs/_analysis/` | 5 | 1,105 |
| **总计** | **22** | **9,675** |
