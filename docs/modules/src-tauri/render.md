# render — 单帧预览合成

> 上级：[本模块目录](INDEX.md) · [总览](OVERVIEW.md) · [模块文档树](../INDEX.md)
>
> 源码：[`../../../src-tauri/src/render.rs`](../../../src-tauri/src/render.rs)

## 定位

`composite_frame` 命令（#47-A）：把 ready-made 的 wgpu 合成器（`opentake-render`）接到 live 编辑会话——从当前 `Timeline` 建 `RenderPlan`，求值单帧成有序绘制列表，逐层经 ffmpeg 解码（`opentake-media`）取像素，GPU 合成、读回，返回一张 **base64 PNG data URL**，前端贴到 `<canvas>`（取代 Timeline 标签页原来的黑色占位）。

## 完成状态

- ✅ **视频 + 图片 + 文本** 层。文本经 `CosmicTextRasterizer`（cosmic-text 排版 + swash 栅格）成预乘 RGBA box 纹理，最后合成，对应上游 `CATextLayer`（#65）。
- ❌ **Lottie** 层仍跳过（resolver 返回 `None`，合成器略过），待 bake 路径接线（#65 后续）。
- 单 `Mutex` 串行化合成——正是预览所需（一次一帧，无 GPU 竞争）。连续播放引擎（#53）会把它移到专用渲染线程。

## IPC 接口

入参：`frame: i32`、`max_size: Option<u32>`（最长边 px 上限；省略用默认上限 `DEFAULT_PREVIEW_CAP = 1280`）。

返回 `CompositeFrameDto`（camelCase）：

```rust
pub struct CompositeFrameDto {
    pub width: u32,
    pub height: u32,
    pub data_url: String,   // "data:image/png;base64,..." 可直接赋给 <img>/canvas
}
```

越界帧（及空时间线）合成为不透明黑——正确的 clear color，非错误。

## GPU 上下文懒加载（RenderState）

```rust
#[derive(Default)]
pub struct RenderState { ctx: Mutex<Option<GpuContext>> }   // Tauri managed state
```

- `GpuContext` = `wgpu::Device` + `Queue` + `Compositor` + `CosmicTextRasterizer`（系统字体首次合成时发现一次）。
- **首次** `composite_frame` 才 `RenderDevice::try_new()` 建上下文，之后跨调用复用；获取失败（无 adapter / headless）转命令错误而非 panic。
- 锁**跨整个 render 持有**，使基于 `Rc` 的纹理缓存绝不跨线程。
- 仅每帧的 `TextureCache`（cap 64）是短命的。

## 数据流（composite_frame）

```
core.get_timeline().timeline / core.media() / core.project_dir()   // 各自锁下快照，GPU 前释放
project text clips → {content, style, box_norm} 按 clip id          // 供 resolver 按需栅格
project manifest → sizes + media 路径
    └─ MediaSource::Project 相对路径必须 join bundle dir（否则重开工程预览黑）
preview_render_size(w, h, cap)            // 偶数化 + 按上限等比降采样（不放大）
build_render_plan(...) → plan.frame(frame)
lock ctx（懒建）→ MediaResolver → compositor.render_to_rgba(...)
encode_png_data_url(composite)
```

## 纹理解析（MediaResolver，预览版）

`TextureResolver`：`Decoded`（video）按源帧 key、`Image` 一次 key、`Text` 栅格化 box、`Lottie` 返回 `None`。

- `FrameRequest.tolerance_secs = 0.1`：宽 seek 容差让 ffmpeg 落到附近关键帧、单次解码少浪费约 10×（scrub 期主导的 CPU/RSS 成本）。导出版用 0.0 精确落帧（见 [export.md](export.md)）。流式播放引擎（#53）将整体替换这条 seek-per-frame 路径。
- `apply_rotation: true`（ffmpeg 解码时自动旋正）。
- 帧时间用**时间线 fps** 时基（`project_frame_time_secs`），对齐 Swift `CompositionBuilder` 的 `CMTime(timescale: fps)`——59.94fps 源在 30fps 时间线上仍按项目帧折算。

## 渲染尺寸（preview_render_size）

偶数化画布，可选降采样使最长边 ≤ cap（cap=0 不降）；统一缩放保持 plan 的仿射数学。规则（有单测覆盖）：不放大、退化画布下限 2×2、1920×1080 @cap1280 → 1280×720。

## 与导出的关系

本文件是单帧路径；[export.md](export.md) 的 `export_video` 是整片路径，其 resolver / metrics / 投影是本文件逻辑的自包含拷贝（有意不互相耦合，待稳定后上提共享辅助）。两者都用 `opentake-render` 的同一 `RenderPlan` / `Compositor`，保证预览与导出像素一致。

---

> 相关：[export.md](export.md)（整片版）· [library-media.md](library-media.md)（媒体路径解析同源）· 跨模块 [opentake-render](../opentake-render/INDEX.md)（合成器 / RenderPlan / 文本栅格）· [opentake-media](../opentake-media/INDEX.md)（`decode_frame_at`）
>
> 导航：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
