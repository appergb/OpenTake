## _analysis 目录说明

本目录包含对上游 `palmier-pro-upstream` 的拆解分析。

### 结构化报告（.md）

| 文件 | 内容 |
|---|---|
| `01-架构与数据流.md` | 上游总体架构、启动链、三层对象模型、所有数据流分析 |
| `02-苹果框架可移植性.md` | AVFoundation / AppKit / SwiftUI → Rust 对应移植方案 |
| `03-闭源云边界.md` | 上游闭源部分（生成 AI 云服务）与开源编辑器的边界 |
| `04-MCP与Agent工具.md` | 31 个 MCP 工具定义 + 双客户端架构分析 |

### 原始数据（.raw.json）

这些是子 Agent 在执行分析时产生的原始输出日志：

| 文件 | 行数 | 来源 |
|---|---|---|
| `upstream-analysis.raw.json` | ~3565 | 上游逐模块拆解时的 20 个 subagent 原始输出 |
| `capcut-gap.raw.json` | ~368 | 剪映模块 1-5 特性差距分析时的 5 个 subagent 原始输出 |

`.raw.json` 文件已提取为上述 `.md` 报告，仅保留为审计/回溯记录，日常开发不需要阅读。
