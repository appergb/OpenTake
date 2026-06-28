# 帧源与尺寸 — SourceMetrics / FrameProvider / 偶数化

> 上级：本模块目录 [INDEX.md](INDEX.md)

## 职责

提供本 crate 与外界的两类边界：
1. **媒体源契约**（`source.rs`）：render 层**定义** trait，`opentake-media`（或调用方）**实现**。这把 render crate 与解码/文件系统彻底解耦——plan 构建只问源的固有尺寸/朝向/alpha 标志，合成器只按需要拉解码像素。`media_ref → 路径`的解析是调用方的事（上游 `MediaResolver`），render 永不碰文件系统。
2. **渲染尺寸纯函数**（`size.rs`）：导出后端用的偶数化与短边缩放，对拍上游 `ExportResolution.renderSize` / `TimelineRenderer.even`。

## 关键类型与算法

### 媒体源契约（`source.rs`）
- `DecodedFrame { width, height, rgba, premultiplied }`：紧凑 RGBA8（行优先、左上原点），与 `opentake_media::RgbaFrame` 字段同构——刻意定义在此以免 render 依赖 media，集成层平凡互转。`new` 带 `debug_assert` 校验 `rgba.len() == w*h*4`。
- `SourceMetrics`（建 plan 时一次性纯元数据查询，不解码）：
  - `natural_size(media_ref)`：视频解码帧尺寸 / 图片像素尺寸 / Lottie 画布尺寸（上游 `imageNativeSize` / `naturalSize`）。
  - `preferred_transform(media_ref)`：容器 display matrix → 行优先 6 元组（默认单位，上游 `preferredTransform`，media 侧用 ffprobe rotate/display matrix 实现）。
  - `needs_premultiply(media_ref)`：源是否直通 alpha 需预乘（默认 false，上游 `trackContainsAlpha`）。
  - `lottie_frame_count(media_ref)`：Lottie 内部总帧数（取模用，默认 None）。
  - 除 `natural_size` 外均有默认实现，最小实现只需 `natural_size`。
- `FrameProvider`（合成时逐帧惰性拉像素）：`decoded_frame(media_ref, source_frame)`（预览解到最近关键帧丢帧 / 导出顺序解码）、`image_pixels(media_ref)`（单帧，上游 `createPixelBuffer` sRGB 预乘等价）、`lottie_frame(media_ref, frame)`（预乘 RGBA）。

> 合成器实际取纹理走的是 `TextureResolver`（见 [gpu-compositor.md](gpu-compositor.md)）——典型集成是「`FrameProvider` 出 `DecodedFrame` → `upload_rgba` → `TextureCache` 缓存」组成 resolver。`FrameProvider` 定义「像素从哪来」，`TextureResolver` 定义「GPU 纹理怎么给合成器」。

### 渲染尺寸（`size.rs`）
- `even(v)`：四舍五入 → 整除 2 → 乘 2 → 下钳 2（上游 `even` + `ExportResolution.renderSize` 的 `Int(...)/2*2` 惯用法）。
- `ExportResolution`：`R720p`/`R1080p`/`R4k`，`short_side_pixels()` = 720/1080/2160。
- `export_render_size(canvas, resolution)`：按画布**短边**缩放到目标短边后逐轴 `even`（≥2）。**不夹 1.0**——小画布导 4K 会放大，与正式导出一致（区别于 `TimelineRenderer` 的任意区间渲染语义，SPEC §5.2）；退化画布（短边 ≤0）回退偶数化画布。

## 源文件
- [`crates/opentake-render/src/source.rs`](../../../crates/opentake-render/src/source.rs) — `DecodedFrame` / `SourceMetrics` / `FrameProvider`。
- [`crates/opentake-render/src/size.rs`](../../../crates/opentake-render/src/size.rs) — `even` / `ExportResolution` / `export_render_size`。

## 不变量
- **render 不碰 IO**：解码、文件系统、`media_ref` 解析全在实现侧（media / 调用方）；本 crate 只持 trait 与纯函数。
- **`DecodedFrame` 形状**：`rgba.len() == width*height*4`，行优先左上原点；`premultiplied` 如实标注（合成器据此决定是否 un-premultiply）。
- **导出尺寸偶数 ≥2**：编码器要求；短边缩放语义按 `ExportResolution`（不夹 1.0）。
- **`preferred_transform` 默认单位、`needs_premultiply` 默认 false、`lottie_frame_count` 默认 None**：最小实现安全可用。

## 关系
- `SourceMetrics` 被 [render-plan.md](render-plan.md) 的 `build_render_plan` 调用，决定 `nat_size` / `preferred_transform` / `needs_premultiply` / `lottie_frame_count`。
- `FrameProvider` / `DecodedFrame` 供 [gpu-compositor.md](gpu-compositor.md) 经 `TextureResolver` 取像素；实现侧见 [opentake-media 规格 SPEC](../opentake-media/SPEC.md)（ffmpeg 解码 + display matrix + alpha 探测）。
- `export_render_size` 供导出后端（src-tauri）定画布像素尺寸。

## 计划中
- media 侧 `FrameProvider` 实现 + 预览 `composite_frame` 接线（PORT-1TO1-GAP P1-9）、真实播放引擎的连续解码/最近关键帧丢帧（P1-10）——本 crate 只提供契约，实现与接线在 media / src-tauri / 前端。

---

> 上级：本模块目录 [INDEX.md](INDEX.md)
