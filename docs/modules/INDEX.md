# 模块文档树 — 总目录

> 上级：[docs 总目录](../INDEX.md)
>
> OpenTake = Rust 多 crate workspace + Tauri 2 桌面壳 + React/TS 前端。**依赖只能向下**：领域层不依赖任何上层，前端只持后端只读镜像。
> 每个模块固定三类文档：`OVERVIEW.md`（总览）、`INDEX.md`（目录，链到下面所有文档）、若干**子系统文档**（模块/子系统级）。部分模块另含 `SPEC.md`（完整规格）与设计文档。

---

## 依赖分层（自底向上）

```
opentake-domain                      值语义叶子层（禁 std::fs / 网络）
   ▲
opentake-ops                         纯编辑引擎 + EditCommand + 撤销栈
   ▲
opentake-project / render / media / motion / agent / gen   能力层
   ▲
opentake-core                        会话 / DI / 事件总线（命令路由）
   ▲
src-tauri                            Tauri 桌面壳 + 命令
   ▲
web                                  React/TS 前端（只读镜像）
```

---

## 模块清单

### 领域层
- **[opentake-domain](opentake-domain/INDEX.md)** — Timeline/Track/Clip/Keyframe/Transform/Text/Grade 纯值语义；序列化模型。叶子 crate，禁止 I/O。
  [总览](opentake-domain/OVERVIEW.md)

### 引擎层
- **[opentake-ops](opentake-ops/INDEX.md)** — Overwrite/Ripple/Snap 纯引擎、`EditCommand` 枚举、`apply()` 事务、撤销/重做栈、各 ops 算法（trim/move/split/ripple/link…）。
  [总览](opentake-ops/OVERVIEW.md)

### 能力层
- **[opentake-project](opentake-project/INDEX.md)** — 工程持久化、bundle/archive、布局、FCPXML(XMEML) 导出、生成日志。
  [总览](opentake-project/OVERVIEW.md)
- **[opentake-render](opentake-render/INDEX.md)** — RenderPlan（纯函数 Timeline→每帧属性）、wgpu 合成器、文本栅格化；预览与导出像素一致。
  [总览](opentake-render/OVERVIEW.md) · [规格 SPEC](opentake-render/SPEC.md)
- **[opentake-media](opentake-media/INDEX.md)** — FFmpeg 编解码、缩略图/雪碧图、波形、转写(whisper)、语义搜索(SigLIP2+ort)、节拍/静音/自动裁剪分析。
  [总览](opentake-media/OVERVIEW.md) · [规格 SPEC](opentake-media/SPEC.md)
- **[opentake-motion](opentake-motion/INDEX.md)** — Lottie / web 动态图形渲染、沙箱、缓存、与渲染管线集成。
  [总览](opentake-motion/OVERVIEW.md) · [Motion Graphics 插件设计](opentake-motion/MOTION-GRAPHICS-PLUGIN.md)
- **[opentake-agent](opentake-agent/INDEX.md)** — MCP server(rmcp, 44 工具)、工具派发、Context Signal、工作流插件、内置 Agent 提示。
  [总览](opentake-agent/OVERVIEW.md) · [规格 SPEC](opentake-agent/SPEC.md) · [Context Signal](opentake-agent/AGENT-CONTEXT-SIGNAL.md) · [工作流插件](opentake-agent/WORKFLOW-PLUGIN-SYSTEM.md)
- **[opentake-gen](opentake-gen/INDEX.md)** — 生成式 AI 客户端(fal.ai/Replicate/OpenAI/ElevenLabs)、模型目录、生成参数、BYOK 密钥（无后端）。
  [总览](opentake-gen/OVERVIEW.md) · [规格 SPEC](opentake-gen/SPEC.md)

### 装配层
- **[opentake-core](opentake-core/INDEX.md)** — 会话管理、依赖注入、事件总线、DTO、命令路由。
  [总览](opentake-core/OVERVIEW.md) · [规格 SPEC](opentake-core/SPEC.md)
- **[src-tauri](src-tauri/INDEX.md)** — Tauri 2 桌面壳、Tauri 命令（`edit_apply`/导出/库/媒体/渲染/密钥/MCP）、`generate_handler!` 注册。
  [总览](src-tauri/OVERVIEW.md)

### 前端
- **[web](web/INDEX.md)** — React/TS + Vite + Zustand；时间线/预览/检查器/媒体/工具栏 UI、像素↔帧换算、Tauri IPC 封装、非 Tauri 内存降级。
  [总览](web/OVERVIEW.md) · [规格 SPEC](web/SPEC.md)
