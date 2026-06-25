<!-- OPENSPEC:START -->
# OpenTake — AI Agent 协作指南

OpenTake 是 Palmier Pro 的跨平台社区分支：Rust core（Tauri 2 + React）桌面端，媒体引擎 FFmpeg + wgpu，GPL-3.0 开源。

## 项目结构

```
PRIMARY-CN/
├── palmier-pro-upstream/   # 上游只读参考（Swift macOS 视频编辑器，GPL-3.0）
│   └── Sources/PalmierPro/ # 上游编辑逻辑的真理来源
└── OpenTake/               # 本项目（当前工作仓库）
    ├── assets/             # 品牌图标与静态资源
    ├── crates/             # Rust workspace（domain / ops / project / media / render / agent / gen / core）
    ├── docs/               # 架构 / 路线图 / 规格 / 上游拆解
    │   └── _analysis/      # 上游拆解报告（4 份横切分析）
    ├── src-tauri/          # Tauri 2 桌面壳
    └── web/                # React + TypeScript 前端
```

## 从何处开始

| 你要做什么 | 先看这个 |
|---|---|
| 了解项目全局 | [README.md](README.md) |
| 理解目标架构 | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| 知道当前阶段 + 下一步做什么 | [docs/ROADMAP.md](docs/ROADMAP.md) |
| 理解 Agent 如何与软件协作 | [docs/AGENT-CONTEXT-SIGNAL.md](docs/AGENT-CONTEXT-SIGNAL.md) |
| 移植某个上游模块 | [docs/MODULE-PORT-MAP.md](docs/MODULE-PORT-MAP.md) |
| 了解为何选了 Rust / Tauri / GPL-3.0 | [DECISIONS.md](DECISIONS.md) |
| 查找某个上游模块的源码 | `palmier-pro-upstream/Sources/PalmierPro/` |
| 前端 UI 1:1 复刻规格 | [docs/specs/frontend-UI-1to1-SPEC.md](docs/specs/frontend-UI-1to1-SPEC.md) |
| Agent/MCP server 规格 | [docs/specs/agent-SPEC.md](docs/specs/agent-SPEC.md) |
| 媒体引擎规格 (ffmpeg/缩略图/波形/搜索/转写) | [docs/specs/media-SPEC.md](docs/specs/media-SPEC.md) |
| wgpu 帧合成器规格 | [docs/specs/render-SPEC.md](docs/specs/render-SPEC.md) |
| 核心编排层规格 | [docs/specs/core-SPEC.md](docs/specs/core-SPEC.md) |
| 生成式 AI 客户端规格 | [docs/specs/gen-SPEC.md](docs/specs/gen-SPEC.md) |
| 工作流插件系统 | [docs/WORKFLOW-PLUGIN-SYSTEM.md](docs/WORKFLOW-PLUGIN-SYSTEM.md) |
| 动效/图形插件 | [docs/MOTION-GRAPHICS-PLUGIN.md](docs/MOTION-GRAPHICS-PLUGIN.md) |
| 进阶能力 (对标剪映) | [docs/ADVANCED-FEATURES.md](docs/ADVANCED-FEATURES.md) |
| 上游拆解分析 | [docs/_analysis/README.md](docs/_analysis/README.md) |
| 需求与问题汇总 | [docs/需求与问题汇总.md](docs/需求与问题汇总.md) |

## 核心设计原则（来自上游拆解）

1. **单一可观测状态容器**：Rust 持有权威 `Timeline`，前端只持只读镜像 + 版本号。
2. **纯函数编辑算法**：OverwriteEngine / RippleEngine / SnapEngine 全部纯函数，无副作用，可全单测。
3. **命令层 = 唯一编辑入口**：所有 UI 手势、Agent、MCP 工具归一到一个 `EditCommand` 枚举。
4. **撤销栈在 Rust**：整树快照（`Timeline` derive `Clone`），前端不做撤销。
5. **预览与导出共享 RenderPlan**：纯函数 `Timeline → 每帧属性`，保证预览与导出像素一致。

## 技术栈（已定）

| 关注点 | 选型 |
|---|---|
| 核心语言 | Rust（workspace，多 crate） |
| 桌面壳 | Tauri 2 |
| 前端 | React + TypeScript + Vite |
| 状态管理 | Zustand（前端只读镜像） |
| 编解码 | ffmpeg-sidecar（调用系统 ffmpeg/ffprobe） |
| 帧合成 | wgpu（自写合成器） |
| 音频播放 | cpal |
| 语义搜索 | ort + SigLIP2（tokenizers 预处理） |
| MCP server | rmcp（streamable-http-server） |

## 移植法则

编辑算法从 Swift → Rust 时的转换铁律：

- **一切以整数帧为单位**，`secondsToFrame` 用截断（`Int(s * fps)`），非四舍五入。
- **关键帧存储用 clip 相对帧偏移**，公开 API 用绝对时间线帧。
- **`round()` 方向与上游一致**：Swift `.rounded()` = Rust `f64::round()`（.5 向偶取整），MODULE-PORT-MAP 中有标注差异处。
- **smoothstep(t) = t*t*(3-2t)**，不要换公式。
- **所有 serde 模型加 `#[serde(default)]` + `Option<T>`**，保证读旧工程不破坏。

## Rust 代码风格

- 用 `Result<T, anyhow::Error>` 做内部错误，边界层转 Tauri 的 `Err(String)`。
- `crates/opentake-domain/` 零依赖叶子 crate，不允许 `std::fs` 或网络调用。
- 单测用 `#[cfg(test)]`，每个命令一个 test module，覆盖率 ≥80%。
- 保持注释最小，只在 why 不显然时写一条短行。

## React / TypeScript 代码风格

- 组件不持有领域逻辑，只渲染 Tauri 命令返回的快照。
- Timeline 的像素↔帧换算放前端，帧↔秒换算放 Rust。
- 所有数值常量走 `AppTheme`，不硬编码。
- 悬停态用 CSS `:hover` + 圆角背景，图标用 lucide-react。

## 构建

```bash
# Rust core
cargo build
cargo test
cargo clippy

# 前端
cd web && pnpm install && pnpm build

# 启动 Tauri 开发模式
cargo tauri dev
```

当前状态：核心实现已落地，Rust workspace、Tauri 壳、React 前端、MCP/Agent 层和主要文档都在仓库中；ROADMAP 继续追踪剩余的高风险差距和后续阶段。

## 上游参考

上游克隆 `palmier-pro-upstream/` 只读。查找编辑逻辑时直接在该目录 grep。禁止修改上游文件。

常用查找路径：
- 领域模型：`palmier-pro-upstream/Sources/PalmierPro/Models/`
- 编辑算法：`palmier-pro-upstream/Sources/PalmierPro/Editor/`
- Agent/MCP 工具：`palmier-pro-upstream/Sources/PalmierPro/Agent/`
<!-- OPENSPEC:END -->

## Agent Context Signal — 软件主动发信号

OpenTake 的核心创新之一是 **软件主动向 Agent 发送剪辑指引**，而不是让 Agent 自己去读技能文件。

当 Agent 通过 MCP 操作时间线和轨道时，软件会在每次工具返回中附带 `context_signal`：
- 视频类型判定（口播 / Vlog / 混剪 / 采访 / 短剧 / 长视频）
- 每个轨道的角色和用途（主画面 / B-roll / 旁白 / BGM / SFX / 文字）
- 当前剪辑阶段和下一步建议
- 该视频类型适用的剪辑规则

这些指引内化自 ClipSkills 技能套件（[appergb/ClipSkills](https://github.com/appergb/ClipSkills)，MIT 许可）。详见 [docs/AGENT-CONTEXT-SIGNAL.md](docs/AGENT-CONTEXT-SIGNAL.md) 和 [docs/WORKFLOW-PLUGIN-SYSTEM.md](docs/WORKFLOW-PLUGIN-SYSTEM.md)。
