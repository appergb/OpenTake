# OpenTake — 项目入口

> 状态：canonical · 阶段：implementation-backed · 同步日期：2026-09-06

OpenTake 是 Palmier Pro 的 GPL-3.0-or-later 社区项目，以 Rust + Tauri 2 + React/TypeScript 构建视频编辑器和 Agent 工作流。编辑逻辑对照只读上游移植。

- **活动工作树**：`OpenTake-generation/`；同级 `OpenTake/` 仅作旧代码对照，`palmier-pro-upstream/` 只读。每个独立 Git 仓库仅保留根目录这一份 `AGENTS.md` 自动入口。
- **基线**：同步起点 `release/v1.0.0-beta.5` / `33ee8e2`；主代理已切换到 `release/v1.0.0-beta.6`，清单版本 `1.0.0-beta.6`。Beta 1–5 已发布；Beta 6 为未发布候选；进入任务先读取实际分支、HEAD、工作区状态。
- **远端**：`origin` = `https://github.com/appergb/OpenTake.git`。
- **技术栈**：10 个库 crate + `src-tauri`（11 个 Cargo 成员），React 18 / TypeScript / Vite / Zustand；FFmpeg、wgpu、cpal；Agent 使用 rmcp 与本地/BYOK 能力。
- **用户意图**：同步全项目文档并完成可验证的 Beta 候选。源码接线、原生 GUI 验证和实际发布分开记录；不把候选写成已发布或全功能完成。
- **风险**：保留用户未提交改动；平台、provider、安装包验收按真实环境与候选提交记录；Sticker 已接图片/Lottie 导入、预览和落轨，待新包原生验收。

## 当前活动计划与路由

| 内容 | 唯一来源 / 入口 |
|---|---|
| 当前执行与发布门槛 | [公开 Beta 活动计划](docs/plans/active/2026-09-06-public-beta.md) |
| Beta 6 候选（待主代理完成、待验证） | [候选发布文档](docs/releases/1.0.0-beta.6.md) |
| 文档总目录 | [docs/INDEX.md](docs/INDEX.md) |
| 开发规范、构建与验证 | [docs/project/conventions.md](docs/project/conventions.md) |
| 意图与工作区约束 | [docs/project/intent.md](docs/project/intent.md) |
| 模块源码路由 | [docs/modules/INDEX.md](docs/modules/INDEX.md) |
| 能力与证据 | [能力账本](docs/capabilities/CAPABILITY-LEDGER.md) |
| 文档同步范围与遗留 | [本次同步报告](docs/documentation-sync-2026-09-06.md) |

`CLAUDE.md` 是历史快照，不是当前交接或额外自动入口。按路由读取所需文档，禁止递归加载整棵文档树。
