# OpenTake 文档总目录

> 全项目文档的**唯一入口**。文档按「模块」组织成超链接树：
> **要开发某个模块，只需读该模块的 `OVERVIEW.md`（总览）+ `INDEX.md`（目录）**，目录里再链到该模块的各子系统文档与规格。

---

## 🧭 如何使用本文档树

```
docs/
├── INDEX.md            ← 你在这里（总目录）
├── modules/            ← ★ 按 crate / 前端分的模块文档树
│   ├── INDEX.md        ← 模块总目录（11 个模块一览）
│   └── <模块>/
│       ├── OVERVIEW.md ← 模块总览：职责 / 依赖 / 数据流 / 完成状态 / 对应上游
│       ├── INDEX.md    ← 模块目录：链到本模块所有子系统文档 + 规格 + 源码
│       └── *.md        ← 子系统文档（模块/子系统级，不逐函数）
├── architecture/       ← 跨切面：总体架构 / 路线图 / 移植图 / gap / bug / 编辑自动化 DOS
└── upstream-analysis/  ← 上游 Palmier Pro 拆解参考
```

**典型路径**：接到「改 X 模块」的活 → 打开 [modules/INDEX.md](modules/INDEX.md) 找到该模块 → 读它的 `OVERVIEW.md` 建立全貌 → 从它的 `INDEX.md` 进入需要的子系统文档 → 需要历史/规划背景时再回 [architecture/](architecture/INDEX.md)。

---

## 📦 模块文档树 → [modules/INDEX.md](modules/INDEX.md)

| 层 | 模块 | 一句话 | 入口 |
|---|---|---|---|
| 领域 | `opentake-domain` | Timeline/Track/Clip/Keyframe 纯值语义（叶子 crate） | [总览](modules/opentake-domain/OVERVIEW.md) · [目录](modules/opentake-domain/INDEX.md) |
| 引擎 | `opentake-ops` | 纯引擎(Overwrite/Ripple/Snap) + EditCommand + 撤销栈 | [总览](modules/opentake-ops/OVERVIEW.md) · [目录](modules/opentake-ops/INDEX.md) |
| 能力 | `opentake-project` | 工程持久化 / bundle / archive / 导出 | [总览](modules/opentake-project/OVERVIEW.md) · [目录](modules/opentake-project/INDEX.md) |
| 能力 | `opentake-render` | wgpu 合成器 + 文本栅格化（预览/导出共享 RenderPlan） | [总览](modules/opentake-render/OVERVIEW.md) · [目录](modules/opentake-render/INDEX.md) |
| 能力 | `opentake-media` | FFmpeg 编解码 / 缩略图 / 波形 / 转写 / 语义搜索 | [总览](modules/opentake-media/OVERVIEW.md) · [目录](modules/opentake-media/INDEX.md) |
| 能力 | `opentake-motion` | Lottie / web 动态图形 | [总览](modules/opentake-motion/OVERVIEW.md) · [目录](modules/opentake-motion/INDEX.md) |
| 能力 | `opentake-agent` | MCP server(44 工具) + 内置 Agent + Context Signal | [总览](modules/opentake-agent/OVERVIEW.md) · [目录](modules/opentake-agent/INDEX.md) |
| 能力 | `opentake-gen` | 生成式 AI 客户端(BYOK，无后端) | [总览](modules/opentake-gen/OVERVIEW.md) · [目录](modules/opentake-gen/INDEX.md) |
| 装配 | `opentake-core` | 会话管理 / DI / 事件总线（命令路由层） | [总览](modules/opentake-core/OVERVIEW.md) · [目录](modules/opentake-core/INDEX.md) |
| 装配 | `src-tauri` | Tauri 2 桌面壳 + Tauri 命令 | [总览](modules/src-tauri/OVERVIEW.md) · [目录](modules/src-tauri/INDEX.md) |
| 前端 | `web` | React/TS 前端（只读镜像 + 版本号） | [总览](modules/web/OVERVIEW.md) · [目录](modules/web/INDEX.md) |

---

## 🏗️ 架构与规划 → [architecture/INDEX.md](architecture/INDEX.md)

跨切面、不属单一模块的设计/规划/报告：总体架构、路线图、1:1 移植图与差距、剪映 gap、已知 Bug、编辑自动化 DOS。

## 📐 上游拆解参考 → [upstream-analysis/README.md](upstream-analysis/README.md)

上游 Palmier Pro（Swift）的架构、Apple 框架可移植性、闭源云边界、MCP/Agent 工具拆解。

---

## 📄 仓库根级文档

| 文档 | 用途 |
|---|---|
| [README.md](../README.md) · [README.zh-CN.md](../README.zh-CN.md) · [README.ja.md](../README.ja.md) | 项目概览（多语言） |
| [CLAUDE.md](../CLAUDE.md) | 工作交接状态文档（压缩上下文后先读） |
| [AGENTS.md](../AGENTS.md) | AI Agent 协作指南 |
| [DECISIONS.md](../DECISIONS.md) | 技术选型决策（为何 Rust/Tauri/GPL-3.0） |
| [CHANGELOG.md](../CHANGELOG.md) | 变更历史 |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | 贡献指南 |
