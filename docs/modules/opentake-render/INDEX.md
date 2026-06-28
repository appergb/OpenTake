# opentake-render — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> `opentake-render` = RenderPlan（纯函数 `Timeline → 每帧属性`）+ wgpu 帧合成器 + 文本栅格化。**预览与导出共用同一条 RenderPlan + 同一个合成器，保证像素一致**。依赖只向下：仅依赖 `opentake-domain`，被 `opentake-core` / `src-tauri` 的预览与导出后端调用。

---

## 总览

- **[OVERVIEW.md](OVERVIEW.md)** — 一句话定位、依赖分层位置、职责边界、关键概念与数据流（两层 RenderPlan → `render_to_rgba`，黑底=clear、混合序、几何投影方向）、对应上游 Swift（`CompositionBuilder` / `CATextLayer` / `TimelineRenderer`）、完成状态（已实现 vs 计划中）、移植铁律（整数帧 / smoothstep / 仿射一致）。

## 子系统文档

- **[render-plan.md](render-plan.md)** — `plan/`：`RenderPlan`/`ClipPlan`/`LayerDraw`/`FramePlan` 数据结构（`types.rs`），`build_render_plan` 由 Timeline 构建静态计划 + `RenderPlan::frame` 逐帧求值 + `source_frame_index` 变速源帧换算（`build.rs`），render 层独有几何投影 `affine_transform`/`compose`/`crop_to_uv`（`affine.rs`）。
- **[gpu-compositor.md](gpu-compositor.md)** — `gpu/`：wgpu 设备获取（`device.rs`）、纹理上传 + LRU 缓存（`texture.rs`）、`Compositor::render_to_rgba` 逐帧合成 + 回读 + `TextureResolver`（`compositor.rs`）、顶点/片元着色器含调色·抠像·蒙版（`shader.wgsl`）、sRGB↔linear（`color.rs`）。
- **[text-rasterizer.md](text-rasterizer.md)** — `gpu/text_engine.rs` + `text_raster.rs`：`CosmicTextRasterizer`（cosmic-text 排版 + swash 栅格 → 预乘 RGBA）与 `NullTextRasterizer` 占位，对应上游 `CATextLayer`。
- **[source-size.md](source-size.md)** — `source.rs`：`SourceMetrics`/`FrameProvider`/`DecodedFrame` 媒体源契约（render 定义、media 实现）；`size.rs`：`even`/`export_render_size` 导出尺寸偶数化与短边缩放。

## 规格与设计

- **[SPEC.md](SPEC.md)** — 实现就绪规格（Issue #7）：上游 `CompositionBuilder` 合成模型逐项拆解（带行号）、RenderPlan 数据结构与算法、wgpu render graph、媒体物化策略、与 domain/media 的接口契约、PoC 像素 diff 验收 + 分步实施清单。

## 相关跨切面（架构）

- [ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md) — 进阶能力设计：A 层 wgpu 着色器特性（特效/转场/调色/绿幕/蒙版）以本合成器为承载层（上游做不到的反超窗口）。
- [ROADMAP.md](../../architecture/ROADMAP.md) — 分阶段路线图：Phase 3（合成器 PoC，项目命门）、Phase 3.5（着色器特性框架）、Phase 4（播放/预览引擎）、Phase 5（导出 #112/#117）、Phase 8（文字渲染）。
- [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md) — 逐模块上游 Swift → Rust 移植地图（Preview/ 与 Export/ 目录，verdict needs-replacement）。
- [PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) — 1:1 复刻差距（历史参考）：P1-9 成片预览接通 wgpu、P1-10 真实播放引擎。
- [ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) — 总体架构 §1/§6：渲染管线、预览/导出共享 RenderPlan 像素一致、媒体物化 hack 整类消失。

## 规格交叉链

- [opentake-domain 目录](../opentake-domain/INDEX.md) — 采样真相源：`Clip::opacity_at/transform_at/crop_at`、`smoothstep`、`ColorGrade`/`ChromaKey`/`Mask` 像素数学、`TextStyle`（render 一律调用、绝不重实现）。
- [opentake-media 规格 SPEC](../opentake-media/SPEC.md) — `SourceMetrics`/`FrameProvider` 的实现侧：ffmpeg 解码、display matrix、alpha 探测、图片/Lottie 物化。
- [模块文档树](../INDEX.md) 的 src-tauri 条目 — 导出后端（`export.rs` 逐帧 `render_to_rgba` → 编码）与预览命令（计划中）调用本模块。

## 源码

```
crates/opentake-render/src/
├── lib.rs              模块声明 + 公开 API re-export + wgpu 重导出
├── plan/
│   ├── mod.rs          re-export
│   ├── types.rs        RenderPlan/ClipPlan/LayerDraw/FramePlan/RenderSize/TextureSource
│   ├── build.rs        build_render_plan + RenderPlan::frame + source_frame_index + make_clip_plan + normalize_box
│   ├── affine.rs       affine_transform / compose / crop_to_uv（render 层几何投影，含内联单测）
│   └── tests.rs        纯函数单测（无 GPU）
├── gpu/
│   ├── mod.rs          RenderError + re-export
│   ├── device.rs       RenderDevice::try_new（无 GPU 优雅跳过）
│   ├── texture.rs      GpuTexture / upload_rgba / TextureCache（content-hash LRU）
│   ├── compositor.rs   Compositor / render_to_rgba / TextureResolver / uniform 打包 / 回读
│   ├── color.rs        srgb_to_linear / linear_to_srgb
│   ├── shader.wgsl     顶点 + 片元（投影约定 + 调色/抠像/蒙版）
│   ├── text_raster.rs  TextRasterizer trait + TextRasterRequest + NullTextRasterizer
│   └── text_engine.rs  CosmicTextRasterizer（cosmic-text + swash）
├── source.rs           SourceMetrics / FrameProvider / DecodedFrame（媒体源契约）
└── size.rs             even / ExportResolution / export_render_size
```

源文件树根：`../../../crates/opentake-render/src/`

---

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
