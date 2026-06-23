<div align="center">
  <img src="./assets/opentake-logo.png" alt="OpenTake" width="128" />

  <h1>OpenTake</h1>

  <p><strong>Agent-Native Video Production Engine</strong></p>

  <p>
    <a href="#-installation"><img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-6e7385?logo=rust" alt="Platforms" /></a>
    <a href="https://github.com/appergb/OpenTake/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue.svg" alt="License" /></a>
    <a href="https://github.com/appergb/OpenTake/stargazers"><img src="https://img.shields.io/github/stars/appergb/OpenTake?style=flat&color=f5c542" alt="Stars" /></a>
    <a href="https://discord.gg/opentake"><img src="https://img.shields.io/badge/Discord-EN-5865F2?logo=discord&logoColor=white" alt="Discord EN" /></a>
    <a href="https://discord.gg/opentake-cn"><img src="https://img.shields.io/badge/Discord-中文-5865F2?logo=discord&logoColor=white" alt="Discord CN" /></a>
    <a href="https://github.com/appergb/OpenTake/actions"><img src="https://img.shields.io/github/actions/workflow/status/appergb/OpenTake/ci.yml?branch=main" alt="CI" /></a>
  </p>

  <p>
    <sub>
      <a href="README.zh-CN.md">中文</a> &nbsp;|&nbsp;
      <a href="README.ja.md">日本語</a>
    </sub>
  </p>
</div>

## Table of Contents

- [About](#-about)
- [Why OpenTake](#-why-opentake)
- [Competitive Edge](#-competitive-edge)
- [Features](#-features)
- [Platforms](#-platforms)
- [Rust Workspace](#-rust-workspace)
- [Architecture](#-architecture)
- [Docs](#-docs)
- [Quick Start](#-quick-start)
- [Version History](#-version-history)
- [Community](#-community)
- [License](#-license)

---

## 📖 About

**OpenTake** is a **cross-platform video production engine** built on **Rust + Tauri 2**, running on macOS / Windows / Linux, designed to deeply integrate AI Agents into professional video editing workflows.

> 🌟 **Core Innovation**: Instead of making the Agent parse lengthy skill documents, OpenTake **actively pushes editing guidance (Context Signal)** to the Agent — telling it exactly what each track does, how each clip should be cut, and what rules apply at every stage.

### Positioning

OpenTake is not a replacement for CapCut / DaVinci Resolve / Final Cut Pro — it's a **video engine designed for AI Agent workflows**. Traditional editors are built for humans; OpenTake is built for humans *and* Agents, with its timeline, preview, and keyframe systems natively controllable via the MCP protocol.

---

## 🎯 Why OpenTake

| Pain Point | Traditional Approach | OpenTake Approach |
|:--|:--|:--|
| Agent doesn't know how to edit | Agent reads skill docs on its own | Software pushes Context Signal — "this track is A-roll, cut with talking-head rhythm" |
| Cross-platform needs 3 codebases | macOS: Swift/AVFoundation, Windows: C++/DirectShow | Single Rust codebase, FFmpeg + wgpu, identical experience on all 3 platforms |
| I want to use my own AI keys | Locked into vendor cloud services | BYOK — direct to fal.ai / Replicate / OpenAI, zero backend, zero ops cost |
| Agent can chat but can't act | CLI agent reads text output | MCP Server with 31 tools — Agent directly runs add_clips / split_clip / set_keyframes |
| Rewriting prompts for every video type | "You are editing a product review..." every time | Workflow Plugin System: review/tutorial/gaming/wedding, each pre-packaged with methodology |
| Steep learning curve for new tools | Complex UI, long onboarding | Agent operates for you — just say "edit this interview into a 3-minute highlight" |

---

## ⚡ Competitive Edge

| Dimension | CapCut | DaVinci Resolve | Final Cut Pro | **OpenTake** |
|:--|:--|:--|:--|:--|
| **Agent-Native Integration** | ❌ | ❌ | ❌ | ✅ MCP 31 tools + Context Signal |
| **Cross-Platform** | ✅ macOS / Win | ✅ macOS / Win / Linux | ❌ macOS only | ✅ macOS / Win / Linux |
| **BYOK AI Generation** | Paid templates | ❌ | ❌ | ✅ Direct fal.ai / Replicate / OpenAI |
| **Local Transcription** | ❌ Cloud-only | ❌ Plugins needed | ❌ Plugins needed | ✅ whisper-rs on-device |
| **Local Semantic Search** | ❌ | ❌ | ❌ | ✅ SigLIP2 + Ort on-device |
| **Workflow Plugins** | ❌ Fixed templates | ❌ | ❌ | ✅ JSON+MD community plugins |
| **Open Source** | ❌ | ❌ | ❌ | ✅ GPL-3.0 |
| **Agent-Controllable Keyframes** | ❌ | ❌ | ❌ | ✅ All 6 kf tracks via MCP |

---

## ✨ Features

### 🧠 Agent Context Signal System

> The software pushes editing guidance *to* the Agent, instead of making the Agent parse documentation.

Every MCP tool response carries a `context_signal`:
- **Auto Genre Detection**: talking-head / vlog / montage / interview / short-drama / long-form
- **Track Role Annotation**: A-roll / B-roll / voiceover / BGM / SFX / text
- **Real-time Rule Checking**: breathing-point rules, B-roll five cautions, clock theory, peak-detection pacing

Knowledge source: [ClipSkills](https://github.com/appergb/ClipSkills) — 12-volume professional editing knowledge base (MIT-licensed).

📖 [Context Signal Design](docs/AGENT-CONTEXT-SIGNAL.md)

### 🔌 MCP Server — 31 Tools

Full MCP server at `127.0.0.1:19789`. Agents control the timeline directly:

| Group | Count | Key Tools |
|:--|:--:|:--|
| Read / Introspect | 7 | `get_timeline`, `get_media`, `inspect_media`, `search_media` |
| Timeline Edit | 11 | `add_clips`, `split_clip`, `set_clip_properties`, `set_keyframes`, `ripple_delete_ranges` |
| Generate / Import | 5 | `generate_video`, `generate_image`, `generate_audio`, `import_media` |
| Library | 7 | `create_folder`, `move_to_folder`, `rename_media` |
| Resources | 2 | `models/video`, `models/image` |

Built-in Agent chat panel shares tool definitions and system prompt with MCP.

### 🎬 Cross-Platform Media Engine

| Capability | Technology |
|:--|:--|
| Codec | FFmpeg (`ffmpeg-next`) — battle-tested Rust bindings |
| Compositor | wgpu custom compositor — multi-track layering + per-frame property sampling + affine/crop/blend |
| Audio Playback | cpal |
| Transcription | whisper-rs (word/segment timestamps) |
| Semantic Search | candle / ort + SigLIP2 dual-encoder |

### 🌐 BYOK AI Generation

**Bring Your Own Key**: Direct connection to fal.ai / Replicate / OpenAI. Zero backend, zero operational cost. Optional self-hosted proxy.

### 📋 Workflow Plugin System

Community-authored JSON + Markdown plugins per video genre — review / tutorial / gaming / wedding / talking-head — each encapsulating professional editing methodology. Agent activates, methodology loads.

📖 [Workflow Plugin System Design](docs/WORKFLOW-PLUGIN-SYSTEM.md)

---

## 🖥️ Platforms

| Platform | Status | Notes |
|:--|:--|:--|
| **macOS** (Apple Silicon + Intel) | ✅ Primary dev platform | Native ARM64 + x86_64; GPU via Metal (wgpu) |
| **Windows** (10/11 x86_64) | ✅ Supported | Vulkan / DX12 backend (wgpu); full Tauri 2 support |
| **Linux** (x86_64) | ✅ Supported | Vulkan backend; AppImage / deb packaging |
| **Backend / Headless** | ✅ Supported | Pure Rust core runs without GUI for CI / server rendering / Agent batch processing |

<sub>📋 macOS ≥12.0 (Monterey), Windows ≥10 (1809+), Linux glibc ≥2.31</sub>

---

## 🦀 Rust Workspace

```
crates/
├── opentake-domain     # Timeline / Track / Clip / Keyframe — pure value semantics
├── opentake-ops        # OverwriteEngine / RippleEngine / SnapEngine — edit algorithm layer
├── opentake-project    # Project persistence / bundle / archive / export
├── opentake-media      # FFmpeg codec / thumbnails / waveform / transcription / semantic search
├── opentake-render     # wgpu compositor + text rasterizer
├── opentake-motion     # Lottie / web motion graphics rendering
├── opentake-agent      # MCP Server + Agent chat + context signal system
├── opentake-gen        # Generative AI clients (fal.ai / Replicate / OpenAI)
├── opentake-core       # Session management / DI / event bus
└── src-tauri           # Tauri 2 desktop shell
```

```bash
> cargo build --workspace   # Build all crates
> cargo test --workspace    # Test all crates (≥80% coverage target)
```

---

## 🏗️ Architecture

```
┌──────────────────────────────────────────────────────┐
│ React + TypeScript Frontend                          │
│ TimelineView · Preview · Inspector · MediaPanel      │
│ Zustand: read-only Timeline mirror + UI-only state   │
└────────────────────┬─────────────────────────────────┘
                     │ Tauri invoke + event
┌────────────────────▼─────────────────────────────────┐
│ 🦀 Rust Core — Source of Truth                       │
│                                                      │
│  opentake-domain    Timeline / Track / Clip / KF     │
│  opentake-ops       EditCommand apply / Undo         │
│  opentake-project   Bundle / Archive / Export         │
│  opentake-render    wgpu Compositor + Text Raster    │
│  opentake-media     FFmpeg / Waveform / Transcribe   │
│  opentake-agent     MCP Server + Chat + Signals      │
│  opentake-gen       fal.ai / Replicate / OpenAI      │
│  opentake-core      Session / DI / Events             │
│                                                      │
│         ▲                          │                 │
│   MCP Server (:19789)      invokes ▼                 │
│   In-app Agent Chat   FFmpeg + wgpu + cpal + whisper │
└──────────────────────────────────────────────────────┘
```

📖 [Architecture Docs](docs/ARCHITECTURE.md)

---

## 📚 Docs

| Document | Content |
|:--|:--|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Target architecture, layering, crate layout, command layer, render pipeline |
| [ROADMAP.md](docs/ROADMAP.md) | Phase 0–10 roadmap with verification criteria and risk register |
| [MODULE-PORT-MAP.md](docs/MODULE-PORT-MAP.md) | 20 upstream module port specs with core algorithms |
| [AGENT-CONTEXT-SIGNAL.md](docs/AGENT-CONTEXT-SIGNAL.md) | Agent Context Signal system design |
| [WORKFLOW-PLUGIN-SYSTEM.md](docs/WORKFLOW-PLUGIN-SYSTEM.md) | Workflow Plugin System (JSON + Markdown) |
| [ADVANCED-FEATURES.md](docs/ADVANCED-FEATURES.md) | Advanced features vs CapCut |
| [CAPCUT-GAP.md](docs/CAPCUT-GAP.md) | 33-item gap analysis vs CapCut |
| [DECISIONS.md](DECISIONS.md) | Tech stack / license / branding ADRs |
| [PORT-1TO1-GAP.md](docs/PORT-1TO1-GAP.md) | 1:1 port gap analysis |

---
---

## 🔗 Upstream Reference

When porting editing logic, compare against the original Palmier Pro Swift source:

```bash
# Clone upstream alongside OpenTake (sibling directory)
cd ..
git clone https://github.com/palmier-io/palmier-pro.git palmier-pro-upstream
cd OpenTake
```

Expected layout:

```
PRIMARY-CN/
├── OpenTake/                  # This repo
└── palmier-pro-upstream/      # Upstream Swift source (GPL-3.0)
```

Key files for comparison:

| Module | Upstream (Swift) | OpenTake (Rust/TS) |
|:--|:--|:--|
| Timeline models | `Sources/PalmierPro/Models/Timeline.swift` | `crates/opentake-domain/src/timeline.rs` |
| Clip model | `Sources/PalmierPro/Models/Timeline.swift` (Clip struct) | `crates/opentake-domain/src/clip.rs` |
| Clip renderer | `Sources/PalmierPro/Timeline/ClipRenderer.swift` | `web/src/components/timeline/clipRenderer.ts` |
| Timeline geometry | `Sources/PalmierPro/Timeline/TimelineGeometry.swift` | `web/src/lib/geometry.ts` |
| Snap engine | `Sources/PalmierPro/Timeline/SnapEngine.swift` | `web/src/lib/snap.ts` |
| Edit operations | `Sources/PalmierPro/Editor/ViewModel/EditorViewModel+ClipMutations.swift` | `crates/opentake-ops/src/ops/` |
| MCP tools | `Sources/PalmierPro/Agent/Tools/ToolExecutor+Timeline.swift` | `crates/opentake-agent/src/tools/` |

> The upstream directory is gitignored from OpenTake — each collaborator clones it independently.

## 🚀 Quick Start

### Prerequisites

- **Rust** ≥ 1.82 (via [rustup](https://rustup.rs))
- **Node.js** ≥ 20 + **pnpm**
- **FFmpeg** ≥ 6.0 (`brew install ffmpeg` / `winget install ffmpeg` / `apt install ffmpeg`)

### Build

```bash
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

> ⚠️ **Current Status**: Early design phase. Architecture, roadmap, and module port maps are complete; code implementation in progress.

The sibling directory `palmier-pro-upstream/` contains upstream Swift sources for reference during porting.

---

## 📋 Version History

| Version | Date | Milestone |
|:--|:--|:--|
| `0.1.0-dev` | 2026-06 | Phase 0+1: Cargo workspace + Domain models + Edit ops + Tauri scaffold |
| *(planned)* `0.2.0` | TBD | Phase 2: Persistence + Media import + Thumbnails + Waveform |
| *(planned)* `0.3.0` | TBD | Phase 3: Timeline UI + Preview + MCP Server |
| *(planned)* `0.4.0` | TBD | Phase 4: GPU Compositor (wgpu) + Text rasterization |
| *(planned)* `1.0.0` | TBD | Phase 10: Full release — CapCut parity + deep Agent integration |

📖 [Full Roadmap](docs/ROADMAP.md)

---

## 🌍 Community

| Discord (English) | Discord (Chinese) | WeChat |
|:--:|:--:|:--:|
| [![Discord EN](https://img.shields.io/badge/Join-EN-5865F2?logo=discord&logoColor=white)](https://discord.gg/opentake) | [![Discord CN](https://img.shields.io/badge/加入-中文-5865F2?logo=discord&logoColor=white)](https://discord.gg/opentake-cn) | TBD |

<br/>

<p align="center">
  <img src="https://img.shields.io/github/stars/appergb/OpenTake?style=social" alt="Stars" />
  <img src="https://img.shields.io/github/forks/appergb/OpenTake?style=social" alt="Forks" />
  <img src="https://img.shields.io/github/issues/appergb/OpenTake?style=social" alt="Issues" />
</p>

### Contributing

Discussions and suggestions welcome at [Issues](https://github.com/appergb/OpenTake/issues).

---

## Acknowledgments

OpenTake stands on the shoulders of these excellent open-source projects.

| Project | License | Usage |
|:--|:--|:--|
| [Palmier Pro](https://github.com/palmier-io/palmier-pro) | GPL-3.0 | Edit logic and domain models originate from this community fork |
| [ClipSkills](https://github.com/appergb/ClipSkills) | MIT | 12-volume editing knowledge base, internalized as Agent Context Signal system |
| [FFmpeg](https://ffmpeg.org) | LGPL-2.1+ / GPL-2.0+ | Media codec engine |
| [Tauri](https://tauri.app) | MIT / Apache 2.0 | Cross-platform desktop app framework |
| [wgpu](https://wgpu.rs) | MIT / Apache 2.0 | GPU rendering engine |
| [whisper.cpp](https://github.com/ggerganov/whisper.cpp) | MIT | Transcription inference engine |
| [rmcp](https://github.com/nicholasxuu/rmcp) | MIT | Rust MCP server SDK |

> "Palmier" / "Palmier Pro" are names/trademarks of their respective owners, used here only for nominative fair use to describe OpenTake's origin.

---

## 📜 License

Copyright (C) 2026 OpenTake contributors

OpenTake is free software: you can redistribute it and/or modify it under the terms of the **GNU General Public License version 3 (GPLv3)** or (at your option) any later version.

OpenTake is distributed in the hope that it will be useful, but **WITHOUT ANY WARRANTY**; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the [GNU General Public License](LICENSE) for more details.

This program is based on [Palmier Pro](https://github.com/palmier-io/palmier-pro) (Copyright (C) 2026 Palmier, Inc.), also distributed under GPLv3. See [NOTICE](NOTICE).

---

<div align="center">
  <sub>Built with 🦀 Rust + 💙 Open Source</sub>
</div>
