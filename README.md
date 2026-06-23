<div align="center">
  <img src="./assets/opentake-logo.png" alt="OpenTake" width="128" />

  <h1>OpenTake</h1>

  <p>
    <strong>Agent-Native Video Production Engine</strong><br />
    エージェントネイティブな動画制作エンジン<br />
    Agent 原生的视频制作引擎
  </p>

  <p>
    <a href="#-installation"><img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-6e7385?logo=rust" alt="Platforms" /></a>
    <a href="https://github.com/appergb/OpenTake/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue.svg" alt="License" /></a>
    <a href="https://github.com/appergb/OpenTake/stargazers"><img src="https://img.shields.io/github/stars/appergb/OpenTake?style=flat&color=f5c542" alt="Stars" /></a>
    <a href="https://discord.gg/opentake"><img src="https://img.shields.io/badge/Discord-EN-5865F2?logo=discord&logoColor=white" alt="Discord EN" /></a>
    <a href="https://discord.gg/opentake-cn"><img src="https://img.shields.io/badge/Discord-中文-5865F2?logo=discord&logoColor=white" alt="Discord CN" /></a>
    <a href="https://github.com/appergb/OpenTake/actions"><img src="https://img.shields.io/github/actions/workflow/status/appergb/OpenTake/ci.yml?branch=main" alt="CI" /></a>
  </p>

  <hr style="width: 60%; margin: 16px auto;" />
</div>

## 目录 / Table of Contents / 目次

- [📖 项目介绍](#-项目介绍--about--プロジェクトについて)
- [🎯 为什么选 OpenTake](#-为什么选-opentake--why-opentake--なぜopentakeか)
- [⚡ 竞品对比优势](#-竞品对比优势--competitive-edge--競合との違い)
- [✨ 核心特性](#-核心特性--features--主な機能)
- [🖥️ 支持平台](#-支持平台--platforms--対応プラットフォーム)
- [🦀 Rust 工作空间](#-rust-工作空间--rust-workspace--rustワークスペース)
- [🏗️ 架构一览](#-架构一览--architecture--アーキテクチャ)
- [📚 文档](#-文档--docs--ドキュメント)
- [🚀 快速开始](#-快速开始--quick-start--クイックスタート)
- [📋 版本历史](#-版本历史--version-history--バージョン履歴)
- [🌍 社区](#-社区--community--コミュニティ)
- [📜 许可证](#-许可证--license--ライセンス)

---

## 📖 项目介绍 / About / プロジェクトについて

**OpenTake** 是一个基于 **Rust + Tauri 2** 构建的**跨平台视频制作引擎**，可在 macOS / Windows / Linux 三大平台上运行，旨在将 AI Agent 与专业视频编辑工作流深度集成。

**OpenTake** is a **cross-platform video production engine** built on **Rust + Tauri 2**, running on macOS / Windows / Linux, designed to deeply integrate AI Agents into professional video editing workflows.

> 🌟 **核心创新 / Core Innovation**: 我们不让 Agent 去翻技能文档。OpenTake 会**主动向 Agent 发送编辑指导（Context Signal）**——时间线的每条轨道、每段素材、每个剪辑阶段，软件都能精准告知 Agent「这段该怎么做」。これが、他の動画編集ツールと OpenTake の決定的な違いです。

### 定位 / Positioning

OpenTake 不是剪映 / DaVinci Resolve / Final Cut Pro 的替代品——它是**为 AI Agent 工作流设计的视频引擎**。传统剪辑软件的设计哲学是「让人用」，界面围绕鼠标和键盘搭建。OpenTake 的设计哲学是「让人和 Agent 一起用」——它的时间线、预览、关键帧系统天然可被 MCP 协议操控，Agent 可以像人类剪辑师一样在时间线上放置素材、调整属性、添加特效。

OpenTake is not a replacement for CapCut / DaVinci Resolve / Final Cut Pro — it's a **video engine designed for AI Agent workflows**. Traditional editors are built for humans; OpenTake is built for humans *and* Agents, with its timeline, preview, and keyframe systems natively controllable via the MCP protocol.

---

## 🎯 为什么选 OpenTake / Why OpenTake / なぜOpenTakeか

| 痛点 / Pain Point | 传统做法 / Traditional | OpenTake 的做法 / Our Approach |
|:--|:--|:--|
| Agent 不知道素材怎么剪 | Agent 自己去读 Skill 文档 | 软件主动发射 Context Signal，告诉 Agent「这条轨是主画面，素材该用口播手法剪」 |
| 跨平台需要三套代码 | macOS 用 Swift/AVFoundation，Windows 用 C++/DirectShow | Rust 单一代码库，FFmpeg + wgpu 跨平台编译，三平台体验一致 |
| 我想用自己的 AI Key | 被锁定在厂商的云服务里 | BYOK（自带 Key）直连 fal.ai / Replicate / OpenAI，零后端、零运营成本 |
| Agent 只能聊不能操作 | CLI Agent 读文本输出 | MCP Server 31 个工具——Agent 直接在时间线上 add_clips / split_clip / set_keyframes |
| 每个视频类型都要重新写提示词 | 每次重复「你要剪一个评测视频...」 | 工作流插件系统：评测/科普/游戏/婚礼每种类型封装好方法论，Agent 开机即用 |
| 学新软件成本高 | 界面复杂，学习曲线陡 | Agent 替你操作，你只需要告诉它「帮我把这个采访剪成 3 分钟的精华」 |

---

## ⚡ 竞品对比优势 / Competitive Edge / 競合との違い

| 维度 / Dimension | 剪映 / CapCut | DaVinci Resolve | Final Cut Pro | **OpenTake** |
|:--|:--|:--|:--|:--|
| **Agent 原生集成** | ❌ | ❌ | ❌ | ✅ MCP 31 工具 + Context Signal |
| **跨平台** | ✅ macOS / Win | ✅ macOS / Win / Linux | ❌ macOS only | ✅ macOS / Win / Linux |
| **BYOK AI 生成** | 内置模板付费 | ❌ | ❌ | ✅ 直连 fal.ai / Replicate / OpenAI |
| **本地语音转写** | ❌ 云端 | ❌ 需插件 | ❌ 需插件 | ✅ whisper-rs 端侧推理 |
| **本地语义搜索** | ❌ | ❌ | ❌ | ✅ SigLIP2 + Ort 本地索引 |
| **工作流插件** | ❌ 固定模板 | ❌ | ❌ | ✅ JSON+MD 社区插件系统 |
| **开源** | ❌ | ❌ | ❌ | ✅ GPL-3.0 |
| **Agent 可操控所有关键帧属性** | ❌ | ❌ | ❌ | ✅ opacity / position / scale / rotation / crop / volume |

---

## ✨ 核心特性 / Features / 主な機能

### 🧠 Agent Context Signal 系统

> 软件主动向 Agent 发送剪辑指导，而非让 Agent 读文件。
> The software pushes editing guidance *to* the Agent, instead of making the Agent parse documentation.

Agent 操作时间线时，每次工具返回附带 `context_signal`：
- **视频类型自动判定**: 口播 / Vlog / 混剪 / 采访 / 短剧 / 长视频
- **轨道角色标注**: 主画面 / B-roll / 旁白 / BGM / SFX / 文字
- **剪辑规则实时校验**: 气口规则、B-roll 五大注意、时钟理论、波峰制动

知识来源: [ClipSkills](https://github.com/appergb/ClipSkills) — 12 册专业剪辑知识内核（MIT 许可），融合影视飓风等专业课程方法论。

📖 [Context Signal 设计文档](docs/AGENT-CONTEXT-SIGNAL.md)

### 🔌 MCP Server — 31 个工具

完整的 MCP server (`127.0.0.1:19789`)，Agent 可直接操控时间线：

| Group 分组 | Count | 代表工具 |
|:--|:--:|:--|
| Read / Introspect 读 / 内省 | 7 | `get_timeline`, `get_media`, `inspect_media`, `search_media` |
| Timeline Edit 时间线编辑 | 11 | `add_clips`, `split_clip`, `set_clip_properties`, `set_keyframes`, `ripple_delete_ranges` |
| Generate / Import 生成 / 导入 | 5 | `generate_video`, `generate_image`, `generate_audio`, `import_media` |
| Library 库组织 | 7 | `create_folder`, `move_to_folder`, `rename_media` |
| Resources | 2 | `models/video`, `models/image` |

内置 Agent chat panel，与 MCP 共享工具定义和系统提示词。

### 🎬 跨平台媒体引擎

| 能力 / Capability | 技术 / Tech |
|:--|:--|
| 编解码 / Codec | FFmpeg (`ffmpeg-next`) — 成熟 Rust 绑定 |
| 帧合成 / Compositor | wgpu 自写合成器 — 多轨叠加 + 逐帧属性采样 + 仿射/裁剪/混合 |
| 音频播放 / Audio Playback | cpal |
| 语音转写 / Transcription | whisper-rs (word/segment 时间戳) |
| 语义搜索 / Semantic Search | candle / ort + SigLIP2 图文双编码器 |

### 🌐 BYOK 生成式 AI

**自带 Key**（Bring Your Own Key）：直连 fal.ai / Replicate / OpenAI，零后端、零运营成本。可选自建托管代理。

### 📋 工作流插件系统

社区为每种视频类型编写 JSON + Markdown 插件——评测 / 科普 / 游戏 / 婚礼 / 口播——每个插件封装专业剪辑方法论，Agent 激活即用。

📖 [Workflow Plugin System 设计](docs/WORKFLOW-PLUGIN-SYSTEM.md)

---

## 🖥️ 支持平台 / Platforms / 対応プラットフォーム

| 平台 | 状态 | 说明 |
|:--|:--|:--|
| **macOS** (Apple Silicon + Intel) | ✅ 主要开发平台 | 原生 ARM64 + x86_64，GPU 加速 via Metal (wgpu) |
| **Windows** (10/11 x86_64) | ✅ 支持 | Vulkan / DX12 backend (wgpu)，完整 Tauri 2 支持 |
| **Linux** (x86_64) | ✅ 支持 | Vulkan backend，AppImage / deb 打包 |
| **Backend / Headless** | ✅ 支持 | 纯 Rust 核心可在无 GUI 环境下运行，用于 CI / 服务端渲染 / Agent 批量处理 |

<sub>📋 macOS 需 ≥12.0 (Monterey)，Windows 需 ≥10 (1809+)，Linux 需 glibc ≥2.31</sub>

---

## 🦀 Rust 工作空间 / Rust Workspace / Rustワークスペース

```
crates/
├── opentake-domain     # Timeline / Track / Clip / Keyframe — 纯函数式值语义
├── opentake-ops        # OverwriteEngine / RippleEngine / SnapEngine — 编辑算法层
├── opentake-project    # 项目持久化 / bundle / archive / export
├── opentake-media      # FFmpeg 编解码 / 缩略图 / 波形 / 转写 / 语义搜索
├── opentake-render     # wgpu 帧合成器 + 文字光栅化
├── opentake-motion     # Lottie / Web 动效渲染
├── opentake-agent      # MCP Server + Agent chat + 上下文信号系统
├── opentake-gen        # 生成式 AI 客户端 (fal.ai / Replicate / OpenAI)
├── opentake-core       # 会话管理 / 依赖注入 / 事件总线
└── src-tauri           # Tauri 2 桌面外壳
```

```bash
> cargo build --workspace   # 全 crate 编译
> cargo test --workspace    # 全 crate 测试 (≥80% coverage target)
```

---

## 🏗️ 架构一览 / Architecture / アーキテクチャ

```
┌─────────────────────────────────────────────────────────┐
│ React + TypeScript Frontend                              │
│ TimelineView · Preview · Inspector · MediaPanel          │
│ Zustand: Timeline 只读镜像 + UI-only 状态                 │
└─────────────────────┬───────────────────────────────────┘
                      │ Tauri invoke + event
┌─────────────────────▼───────────────────────────────────┐
│ 🦀 Rust Core — Source of Truth                           │
│                                                          │
│  opentake-domain    Timeline / Track / Clip / Keyframe   │
│  opentake-ops       EditCommand apply / Undo / Ripple    │
│  opentake-project   Bundle / Archive / Export             │
│  opentake-render    wgpu Compositor + Text Rasterizer    │
│  opentake-media     FFmpeg / Waveform / Transcribe        │
│  opentake-agent     MCP Server + Agent Chat + Signals     │
│  opentake-gen       fal.ai / Replicate / OpenAI Client   │
│  opentake-core      Session / DI / Events                 │
│                                                          │
│         ▲                              │                 │
│   MCP Server (:19789)         调用     ▼                 │
│   In-app Agent Chat     FFmpeg + wgpu + cpal + whisper   │
└─────────────────────────────────────────────────────────┘
```

📖 [详细架构文档 / Architecture Docs](docs/ARCHITECTURE.md)

---

## 📚 文档 / Docs / ドキュメント

| Document | 内容 / Content |
|:--|:--|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | 目标架构、分层、crate 布局、命令层、渲染管线 |
| [ROADMAP.md](docs/ROADMAP.md) | Phase 0–10 路线图，含验证标准与风险登记 |
| [MODULE-PORT-MAP.md](docs/MODULE-PORT-MAP.md) | 20 个上游模块逐项移植规格、核心算法 |
| [AGENT-CONTEXT-SIGNAL.md](docs/AGENT-CONTEXT-SIGNAL.md) | Agent 上下文信号系统设计 |
| [WORKFLOW-PLUGIN-SYSTEM.md](docs/WORKFLOW-PLUGIN-SYSTEM.md) | 工作流插件系统 (JSON + Markdown) |
| [ADVANCED-FEATURES.md](docs/ADVANCED-FEATURES.md) | 对标剪映的进阶能力设计 |
| [CAPCUT-GAP.md](docs/CAPCUT-GAP.md) | 与剪映的 33 项特性差距分析 |
| [DECISIONS.md](DECISIONS.md) | 技术栈 / 许可 / 品牌决策记录 (ADR) |
| [PORT-1TO1-GAP.md](docs/PORT-1TO1-GAP.md) | 1:1 端口差距分析 |

---

## 🚀 快速开始 / Quick Start / クイックスタート

### 前置依赖 / Prerequisites

- **Rust** ≥ 1.82 (via [rustup](https://rustup.rs))
- **Node.js** ≥ 20 + **pnpm**
- **FFmpeg** ≥ 6.0 (`brew install ffmpeg` / `winget install ffmpeg` / `apt install ffmpeg`)

### 构建 / Build

```bash
# Clone
git clone https://github.com/appergb/OpenTake.git
cd OpenTake

# Rust core
cargo build
cargo test
cargo clippy

# Frontend
cd web && pnpm install && pnpm build

# Launch Tauri dev mode
cd .. && cargo tauri dev
```

> ⚠️ **当前状态 / Current Status**: 早期设计阶段。架构设计、路线图、模块移植地图已完成，代码正在落地中。
> Early design phase. Architecture, roadmap, and module port maps are complete; code is being implemented.

项目根目录同级 `palmier-pro-upstream/` 含上游 Swift 源码，用于编辑逻辑移植时的对照参考。

---

## 📋 版本历史 / Version History / バージョン履歴

| 版本 / Version | 日期 / Date | 里程碑 / Milestone |
|:--|:--|:--|
| `0.1.0-dev` | 2026-06 | Phase 0+1: Cargo workspace + Domain models + Edit ops + Tauri scaffold |
| *(planned)* `0.2.0` | TBD | Phase 2: Persistence + Media import + Thumbnails + Waveform |
| *(planned)* `0.3.0` | TBD | Phase 3: Timeline UI + Preview + MCP Server |
| *(planned)* `0.4.0` | TBD | Phase 4: GPU Compositor (wgpu) + Text rasterization |
| *(planned)* `1.0.0` | TBD | Phase 10: 全功能发布 — 对标剪映 + Agent 深度集成 |

📖 [完整路线图 / Full Roadmap](docs/ROADMAP.md)

---

## 🌍 社区 / Community / コミュニティ

| Discord (English) | Discord (中文) | WeChat 联系群 |
|:--:|:--:|:--:|
| [![Discord EN](https://img.shields.io/badge/Join-EN-5865F2?logo=discord&logoColor=white)](https://discord.gg/opentake) | [![Discord CN](https://img.shields.io/badge/加入-中文-5865F2?logo=discord&logoColor=white)](https://discord.gg/opentake-cn) | 联系群信息稍后提供 |

<br/>

<p align="center">
  <img src="https://img.shields.io/github/stars/appergb/OpenTake?style=social" alt="Stars" />
  <img src="https://img.shields.io/github/forks/appergb/OpenTake?style=social" alt="Forks" />
  <img src="https://img.shields.io/github/issues/appergb/OpenTake?style=social" alt="Issues" />
</p>

### 贡献 / Contributing

欢迎在 [Issues](https://github.com/appergb/OpenTake/issues) 中提交建议或设计讨论。

---

## 致谢 / Acknowledgments / 謝辞

OpenTake 建立在以下优秀开源项目的肩膀之上。

| 项目 / Project | License | 用途 / Usage |
|:--|:--|:--|
| [Palmier Pro](https://github.com/palmier-io/palmier-pro) | GPL-3.0 | 编辑逻辑与领域模型来源于此社区分支 |
| [ClipSkills](https://github.com/appergb/ClipSkills) | MIT | 12 册剪辑知识内核，内化为 Agent Context Signal 系统 |
| [FFmpeg](https://ffmpeg.org) | LGPL-2.1+ / GPL-2.0+ | 媒体编解码引擎 |
| [Tauri](https://tauri.app) | MIT / Apache 2.0 | 跨平台桌面应用框架 |
| [wgpu](https://wgpu.rs) | MIT / Apache 2.0 | GPU 渲染引擎 |
| [whisper.cpp](https://github.com/ggerganov/whisper.cpp) | MIT | 语音转写推理引擎 |
| [rmcp](https://github.com/nicholasxuu/rmcp) | MIT | Rust MCP server SDK |

> "Palmier" / "Palmier Pro" 是其各自所有者的名称/商标，此处仅用于说明 OpenTake 的来源（指明性合理使用）。
> "Palmier" / "Palmier Pro" are names/trademarks of their respective owners, used here only for nominative fair use.

---

## 📜 许可证 / License / ライセンス

Copyright (C) 2026 OpenTake contributors

OpenTake is free software: you can redistribute it and/or modify it under the terms of the **GNU General Public License version 3 (GPLv3)** or (at your option) any later version.

OpenTake is distributed in the hope that it will be useful, but **WITHOUT ANY WARRANTY**; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the [GNU General Public License](LICENSE) for more details.

This program is based on [Palmier Pro](https://github.com/palmier-io/palmier-pro) (Copyright (C) 2026 Palmier, Inc.), also distributed under GPLv3. See [NOTICE](NOTICE).

---

<div align="center">
  <sub>Built with 🦀 Rust + 💙 Open Source</sub>
</div>
