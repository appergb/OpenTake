# 架构与规划 — 目录

> 状态：canonical · 阶段：implementation-backed · 路由同步：2026-09-06。
> 当前执行与验收：[公开 Beta 计划](../plans/active/2026-09-06-public-beta.md) · [当日验证](../audit/2026-09-06/public-beta-validation.md)。

> 上级：[docs 总目录](../INDEX.md) · 同级：[模块文档树](../modules/INDEX.md) · [上游拆解](../upstream-analysis/README.md)
>
> 这里收录**跨切面、不属单一模块**的设计、规划与报告文档。具体某个模块的实现文档见 [模块文档树](../modules/INDEX.md)。

---

## 当前执行入口

[公开 Beta 活动计划](../plans/active/2026-09-06-public-beta.md) · [Beta 6 候选（待验证、未发布）](../releases/1.0.0-beta.6.md) · [同步报告](../documentation-sync-2026-09-06.md)。旧交接、差距和扫描报告保留原时点结论，不能作为当前待办真值。

## 总体设计

| 文档 | 内容 |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | 总体架构：分层 crate、数据流、单一真理状态 + 命令事务、渲染管线、Agent 集成 |
| [ADVANCED-FEATURES.md](ADVANCED-FEATURES.md) | 进阶能力设计：wgpu 着色器、AI 推理、FFmpeg 音频工程、跨平台特性 |
| [UPDATER.md](UPDATER.md) | GitHub Beta 发现、Tauri 签名验证、一键安装、安全保存/任务 gate 与发布密钥契约 |

## 路线与移植

| 文档 | 内容 |
|---|---|
| [../releases/1.0.0-beta.2.md](../releases/1.0.0-beta.2.md) | 历史 Beta 2 范围与验收记录 |
| [HANDOFF-2026-07.md](HANDOFF-2026-07.md) | 2026-07-04 交接时点快照；保留作历史参考，不是当前 TODO 真值 |
| [ROADMAP.md](ROADMAP.md) | 分阶段路线图（Phase 0 脚手架 → Motion Canvas 插件） |
| [EDITING-ENGINE-PLAN.md](EDITING-ENGINE-PLAN.md) | 剪辑引擎现况与规划：已移植的 ops 层 + 待收口 gap |
| [PORT-1TO1-GAP.md](PORT-1TO1-GAP.md) | 1:1 复刻差距与实现计划（P0/P1/P2 逐项）。⚠️ 历史参考，以更新的 DOS / 模块文档为准 |
| [MODULE-PORT-MAP.md](MODULE-PORT-MAP.md) | 逐模块移植地图：上游 Swift 模块 → Rust crate 对应方案 |
| [CAPCUT-GAP.md](CAPCUT-GAP.md) | OpenTake vs 剪映特性差距报告（逐模块核对） |

## 报告与质量

| 文档 | 内容 |
|---|---|
| [BUGS.md](BUGS.md) | 实际发现的 Bug 与有问题部分 |
| [FULL_PROJECT_SCAN_REPORT.md](FULL_PROJECT_SCAN_REPORT.md) | OpenTake vs 上游全项目扫描报告 |

## 编辑自动化 DOS（Design Operating Spec）

实现独立编辑自动化（Auto Crop / Beat Sync / Agent 建议 / 工作流配方）的契约与验收门槛，见子目录：

→ **[editing-automation/README.md](editing-automation/README.md)**
