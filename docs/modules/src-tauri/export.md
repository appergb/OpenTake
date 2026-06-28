# export — 整条时间线视频导出

> 上级：[本模块目录](INDEX.md) · [总览](OVERVIEW.md) · [模块文档树](../INDEX.md)
>
> 源码：[`../../../src-tauri/src/export.rs`](../../../src-tauri/src/export.rs)

## 定位

`export_video` 命令：把当前时间线的**每一帧**在 GPU 上合成（wgpu 合成器 `opentake-render`），把 RGBA 帧喂给系统 ffmpeg 编码器（`opentake_media::VideoEncoder`），产出磁盘上真实的 `.mp4`。它是单帧预览路径 [render.md](render.md) 的「整片」对应物。

## 完成状态（首版切片，SPEC §2.4 / §8.2）

| 维度 | 状态 |
|---|---|
| 视频编码 | 🟡 **仅 H.264 / .mp4**。编码器本身已支持 H.265 / ProRes preset，但本命令未接线 |
| 音频 | ✅ **线性混音**：每个含音频 clip 的源窗解码成 mono f32 @ 混音采样率，按帧推导的样本偏移落位，乘 `volume_at` 包络，求和、硬限幅，由编码器 mux 入（AAC） |
| 分辨率 | ✅ 全量导出分辨率（`export_render_size`），非预览降采样上限 |
| 进度 / 取消 | ❌ 未实现——编排器在 GPU 锁下逐帧跑到完 |
| 文本层 | ✅ 支持（`CosmicTextRasterizer`） |
| Lottie 层 | ❌ resolver 返回 `None`（跳过） |

H.265 / ProRes 在 `resolve_preset` 里**显式报错**（`"H.265 export is not wired yet (TODO)"` / ProRes 同理），而非默默失败。

## IPC 入参（ExportRequest）

```rust
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub out_path: String,            // 必须 .mp4（H.264 路径）
    #[serde(default)] pub codec: ExportCodec,     // 默认 H264
    #[serde(default)] pub quality: ExportQuality, // 默认 1080p
}
```

- `ExportCodec`（`rename_all = "lowercase"`）：`h264`（默认）/ `h265`（留位）/ `prores`（留位）。
- `ExportQuality`：`720p` / `1080p`（默认）/ `4k`——每档同时映射到 render-crate 分辨率与 encode-crate 分辨率两个选择器。
- `#[serde(default)]` 让 bare 载荷 `{ "outPath": "…" }` 即导出 H.264 / 1080p。

返回 `ExportSummary { outPath, width, height, fps, frameCount }`（camelCase）。

## 编排流程（run_export）

`export_video` 命令只做「快照 live 会话 → 委派」，真正逻辑在 `run_export()`（与 Tauri / `AppCore` 解耦，便于 ffmpeg-gated 集成测试 `tests/export_integration.rs` 直接用手搭的 timeline + manifest 驱动）：

```
resolve_preset(codec, quality, out)          // 校验扩展名匹配容器；拒未接线 codec
project_text(timeline)                        // 文本 clip → {content, style, box} 按 clip id
project_media(manifest, project_dir)          // manifest → (sizes, media 路径)；解析 Project 相对路径
export_render_size((w,h), quality)            // 全量导出尺寸（偶数化）
build_render_plan(timeline, size, metrics)
RenderDevice::try_new()                        // 本地一次性 GPU 上下文（不复用预览的缓存上下文）
VideoEncoder::new(out, w, h, fps, preset)
for f in 0..total_frames {
    plan.frame(timeline, f) → MediaResolver（每帧新建，cache cap=64）
    compositor.render_to_rgba(...) → encoder.push_frame(RgbaFrame)
}
mix_timeline_audio(timeline, media) → encoder.push_audio(pcm)   // 无音频则视频-only
encoder.finish()
```

要点：
- **GPU 上下文本地一次性**：导出是一次性批处理，不复用预览缓存的上下文，避免与预览锁竞争。
- 空时间线仍产出合法（可能零帧）文件；越界帧合成为不透明黑（正确的 clear color，非错误）。
- GPU 获取 / 解码 / 编码失败均转 `Err(String)`（Tauri 边界约定）。

## 纹理解析（MediaResolver，导出版）

`TextureResolver` 实现：video 按源帧 key、image 一次 key、text 栅格化其 box、Lottie 返回 `None`。与预览版相比，导出版 `FrameRequest.tolerance_secs = 0.0`（精确落帧，质量优先；预览用 0.1s 宽容差换 scrub 速度）。这份 resolver / metrics / 投影逻辑是预览路径逻辑的**自包含拷贝**（有意留在本模块，不动 `render.rs`）；待两条路径稳定后再把共享投影上提为 `pub(crate)` 辅助。

## 音频混音（mix_timeline_audio）

- 解码规格 `AUDIO_DECODE_SPEC`：mono / f32 / `MIX_SAMPLE_RATE`——在混音率上解码，使混音成为样本对齐的纯加法（本切片不做逐 clip 重采样）。
- 仅 `Audio` / `Video` 类型 clip 贡献声音（text/image/lottie 无声）；**muted 轨被跳过**。
- 每个 clip 经 `project_clip_audio`：解码可见源窗 → 落到帧推导的起始样本 → 按 `volume_at` 逐样本建增益包络（全 unity 则塌缩为空包络）。
- clip 指向无音轨的视频 → `MediaError::NoTrack` 被吞为「贡献静音」，**不是导出失败**；其它解码错误才上抛。
- 全部 clip 无音频 → 返回 `None` → 保持视频-only 输出。

## 与 `export_fcpxml` 的区别

`export_video`（本文件）产出**像素级渲染的视频文件**；`export_fcpxml`（[commands-ipc.md](commands-ipc.md)）产出 **XMEML 工程交换 XML**（给 Premiere/DaVinci/FCP）。两者无关。

---

> 相关：[render.md](render.md)（共享逻辑的单帧版）· [commands-ipc.md](commands-ipc.md)（`export_fcpxml` 对比）· 跨模块 [opentake-render](../opentake-render/INDEX.md)（合成器 / RenderPlan）· [opentake-media](../opentake-media/INDEX.md)（VideoEncoder / PCM）
>
> 导航：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
