# opentake-motion — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> `opentake-motion` = **原生 web 动态图形 fallback 渲染原语层**：把内联 HTML/CSS/JS（或模板 + 参数）确定性逐帧栅格化为磁盘 RGBA PNG 帧序列、内容寻址缓存、安全沙箱，并适配成 `opentake-render` 的 clip source。
> ⚠️ **当前是脚手架 / fallback**：动效 / AI Video 的 **v1 主路径走外部 Motion Canvas 插件**（产 `mp4` 按普通视频导入；插件目录 `plugins/motion-canvas-studio/` **尚未存在**）。本 crate 保留给后续透明 alpha overlay / frame-sequence / HTML-CSS fallback。**不是 Lottie 渲染器**（Lottie 在 [opentake-render](../opentake-render/INDEX.md)）。
> 依赖只向下：依赖 `opentake-render`（实现其 source 契约）+ `opentake-domain`；设计上由 `opentake-core`/`src-tauri`/`opentake-agent` 调用（**v1 未接线**）。真实 headless-Chromium 后端在 `chromium` feature 后，**默认 build/CI 离线、不需浏览器**。

---

## 总览

- **[OVERVIEW.md](OVERVIEW.md)** — 定位与依赖分层、职责边界（做什么/不做什么，含"不是 Lottie 渲染器""不是 v1 主渲染器"）、关键概念与数据流（fallback 管线、确定性时钟、沙箱、与 render 集成桥）、对应上游 Swift（**无直接对应**，上游动态图形=Lottie 落 render/media）、完成状态（已实现纯逻辑 vs 计划中真实 CDP/Motion Canvas 插件）、移植铁律。

## 子系统文档

- **[renderer.md](renderer.md)** — `renderer.rs`：`MotionRenderer` trait + `deterministic_clock_script()`（注入页面、冻结时钟、`OpenTake.seek`）+ `StubRenderer`（确定性纯色帧、无依赖自制 PNG 编码器）+ `HeadlessChromiumRenderer`（真实 CDP 后端**骨架**：`data_url_for_code`/`frame_time_grid`，live 渲染未实现，返回 `RendererUnavailable`）。
- **[sandbox.md](sandbox.md)** — `sandbox.rs`：`SandboxPolicy`（网络默认全拒 / 超时熔断 / 文档大小上限 / 无文件系统访问，**建模为类型**）+ `AllowedOrigin`（仅 https/loopback、无通配、拒明文远程）+ 纯检查 `check_url`（`data:` 放行）/ `check_document_size`。
- **[manifest-source.md](manifest-source.md)** — `source.rs`（值类型：`MotionSource` Code/Template、`MotionRenderRequest` camelCase + 范围校验、`RenderedClip` 磁盘帧 + 末帧定格、`ParamValue`、`limits` 硬上限）+ `manifest.rs`（`MotionPlugin` 模板清单：容错解码 + 严格 `validate`/`validate_params`、`DurationMode`/`FpsPolicy`/`ParamSpec`）+ `error.rs`（`MotionError` thiserror）。
- **[cache.md](cache.md)** — `cache.rs`：`content_hash`（SHA-256 over 源+参数+fps+尺寸+透明，规范字节流 + `opentake-motion/v1` 版本前缀 + 参数类型标签 + `-0.0` 归一）+ `MotionCache`（`root/<hash>/`、`is_cached` 按帧数完整性判定 partial=miss、`frame_file` 零填充）。
- **[integration.md](integration.md)** — `integration.rs`：`MotionClipSource` 把 `RenderedClip` 适配成 render 的 `SourceMetrics` + `FrameProvider`（`natural_size`/`needs_premultiply` 跟透明/过末端钳位/缺帧 None）+ `FrameDecoder` 解码器注入（PNG→RGBA 不硬接，调用层提供）。含 **Lottie 方法澄清**（`lottie_frame` 仅转发，非真 Lottie 源）。

## 规格与设计

- **[MOTION-GRAPHICS-PLUGIN.md](MOTION-GRAPHICS-PLUGIN.md)** — 动效 / AI Video 插件**设计规划**（Issue #34）：方向修正（Motion Canvas 优先 / 本 crate 转 fallback）、用户体验（Motion Panel）、模块边界（待新增 `plugins/motion-canvas-studio/` + Tauri `motion_canvas.rs`）、持久化、license/README 要求、v1 MP4 → v2 图片序列 → v3 原生 HTML/CSS fallback 的渲染策略、实施顺序、验证标准、风险、已完成/待做清单。⚠️ 只读规格，本目录文档以**代码现况**为准。

## 相关跨切面（架构）

- [ROADMAP.md](../../architecture/ROADMAP.md) — **Phase 10**（Motion Canvas 动效 / AI Video 插件 + `opentake-motion` fallback）。
- [ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) — 总体架构：crate 分层中 `opentake-motion`（Lottie / web 动态图形）的位置、渲染管线。
- [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md) — 逐模块上游 Swift → Rust 移植地图（上游 `LottieVideoGenerator` 落 render/media；本 crate 上游无对应）。
- [ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md) — 进阶能力（AI 运动追踪等与动效相关的后续方向）。

## 相关模块

- [opentake-render](../opentake-render/INDEX.md) — **定义** `SourceMetrics`/`FrameProvider`/`DecodedFrame`（本 crate 实现之）；**真正的 Lottie 渲染**（`TextureSource::Lottie`）在此。
- [opentake-domain](../opentake-domain/INDEX.md) — workspace 依赖（当前模块内未直接消费其类型）。
- [opentake-agent](../opentake-agent/INDEX.md) — `add_motion_graphic` / `edit_motion_graphic` 工具（语义已改为 Motion Canvas scene/template；dispatch 待接线）。

## 源码

```
crates/opentake-motion/src/
├── lib.rs           模块声明 + 公开 API 扁平 re-export + crate 级管线/确定性/安全文档
├── error.rs         MotionError（thiserror：InvalidSource/Request、Manifest、RendererUnavailable、Timeout、Sandbox、RenderFailed、Io）+ MotionResult
├── source.rs        值类型：MotionSource(Code/Template)、MotionRenderRequest(camelCase+validate)、RenderedClip(磁盘帧+末帧定格)、ParamValue、limits 硬上限
├── manifest.rs      MotionPlugin 模板清单（plugin.json，容错解码 + validate/validate_params）、DurationMode/DurationSpec/FpsPolicy/ParamSpec/MotionPluginAuthor
├── cache.rs         content_hash（内容寻址键 + v1 版本前缀）+ MotionCache（root/<hash>/、完整性判定、零填充帧名）
├── renderer.rs      MotionRenderer trait + deterministic_clock_script + StubRenderer(自制 PNG 编码器) + HeadlessChromiumRenderer(骨架，feature `chromium`)
├── sandbox.rs       SandboxPolicy（网络/超时/文档大小/无 FS，建模为类型）+ AllowedOrigin + check_url/check_document_size
└── integration.rs   MotionClipSource（impl SourceMetrics + FrameProvider）+ FrameDecoder 解码器注入
```

源文件树根：`../../../crates/opentake-motion/src/`

---

## 页脚

- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
