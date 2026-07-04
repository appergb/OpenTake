# opentake-media — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> `opentake-media` = 媒体读取与离线分析层：ffmpeg sidecar 探测/解码/编码、缩略图/雪碧图、波形、转写(whisper)、SigLIP2 语义搜索、节拍/静音/自动裁剪分析、全局素材库。依赖只向下：仅依赖 `opentake-domain`，被 `opentake-core` / `src-tauri` / `opentake-agent` / `opentake-render` 调用。运行期需 **FFmpeg ≥ 6.0 在 PATH**。

---

## 总览

- **[OVERVIEW.md](OVERVIEW.md)** — 定位与依赖分层、职责边界（做什么/不做什么）、关键概念与数据流（ffmpeg sidecar 编解码、缩略图/波形/转写/语义搜索总管线、与 render/agent 关系）、对应上游 Swift（AVFoundation→FFmpeg）、完成状态（已实现 vs 计划中）、移植铁律（整数帧、波形用 ffmpeg 而非 symphonia）。

## 子系统文档

- **[probe-ff.md](probe-ff.md)** — `ff.rs`（ffmpeg/ffprobe 二进制发现 + ffprobe JSON 查询 + 可用性探测；`OPENTAKE_FFMPEG`/`OPENTAKE_FFPROBE` 覆盖）+ `probe.rs`（`MediaProbe`：时长/旋转校正分辨率/fps/有无音视频轨，纯函数 JSON→Probe）。含**为何用 CLI sidecar 而非 libav 绑定**的根因。
- **[decode.md](decode.md)** — `decode/`：`frame.rs`（`decode_frame_at`/`decode_frames_at` seek 解帧为 `RgbaFrame`，`fit_within` 等比缩放，批量去重）+ `pcm.rs`（`extract_pcm` 抽首音轨为单声道 f32 `PcmBuffer`，多通道下混）+ 顶层 `frame.rs`（`RgbaFrame` 像素值类型）。
- **[encode.md](encode.md)** — `encode/`：`mod.rs`（`VideoEncoder` 两趟编码：rawvideo RGBA→视频，再 mux 音频）+ `preset.rs`（`ExportPreset`：codec/分辨率→ffmpeg token，BT.709，`even_dimension`）+ `mix.rs`（`mix_clips` 线性混音 + 硬限幅，f32→s16le）。含**有意省略 `AlphaVideoNormalizer`** 的说明。
- **[thumbnail.md](thumbnail.md)** — `thumbnail/`：`mod.rs`（视频缩略图序列时间点公式 + 渐进回调 + 图片单缩略图）+ `sprite.rs`（JPEG 雪碧图网格几何 + 磁盘缓存 `.thumbs.jpg`/`.thumbs.json`，sidecar 完整标记，与上游 camelCase 互读）。
- **[waveform.md](waveform.md)** — `waveform/`：`mod.rs`（`waveform`/`waveform_cached`，22050Hz 抽 PCM）+ `dsp.rs`（样本数公式 150/秒、4000/20000 边界、RMS 降采样 + 归一化「0=响,1=静」）+ `store.rs`（`.waveform` 裸 f32 LE 缓存）。强调**改用 ffmpeg 解 PCM 而非 symphonia** 的根因。
- **[transcribe.md](transcribe.md)** — `transcribe/`：`mod.rs`（`Transcriber` trait + `TranscriptionResult/Word/Segment` 模型 + `transcribe_file` + `offsetting` 时间码回移）+ `whisper.rs`（whisper.cpp 后端，feature `whisper-backend`，厘秒→秒）+ `locale.rs`（BCP-47 语言/区域匹配）+ `cache.rs`（内存 LRU=4 + 磁盘 JSON + range 过滤）+ `search.rs`（AND 子串 + NFD 折叠大小写/变音的转写内搜索）。
- **[semantic-search.md](semantic-search.md)** — `search/` + `ort_worker/`：SigLIP2 双编码器（`embedder`/`ort_embedder` squash-resize 黑底 256²）、`tokenizer`（截断 64 右填 0）、`frame_sampler`（luma 8×8 grid + 镜头边界 promoteDiff + 覆盖下限）、`indexer`（索引累积幂等）、`embed_store`（`PALMEMB1` f16 落盘）、`ranker`（矩阵·向量 best-per-shot + 截断）、`model_download`（下载/SHA256 校验/解压）、`config`（常量/manifest）、`ort_worker`（通用 ONNX 推理面 + tensor 互转）。
- **[analysis.md](analysis.md)** — `analysis/`：`beat.rs`（能量包络 onset 节拍检测 → `detect_beats`/`auto_cut_to_beats`）+ `silence.rs`（RMS 阈值静音检测 → `tighten_silences`）+ `autocrop.rs`（黑边/透明区扫描裁剪 → `smart_reframe`，**当前非人脸/显著性**）。三者纯算法，PCM/帧由调用层从 ffmpeg 抽取。
- **[library-index.md](library-index.md)** — `library.rs`（全局素材库：SHA-256 内容寻址去重 + copy-on-favorite + JSON manifest 原子写 + 写锁；上游无对应，#37/#104）+ `index_coordinator.rs`（后台索引/转写调度内核：`work_needed`/`visual_share`/`ExportPause` 引用计数；tokio 运行时在 core）+ `cache_key.rs`（统一缓存键 `SHA256("path|mtime|size")` 前 32 hex，Swift `Double.description` 整数补 `.0` 对齐）+ `error.rs`（`MediaError`）。

## 规格

- **[SPEC.md](SPEC.md)** — 实现就绪规格（Issue #8）：逐子系统的接口、常量、磁盘格式、上游行级对照、验收门槛。⚠️ 规格成文于实现前，部分技术选型（`ffmpeg-next` / Symphonia 波形）实际已变更为 ffmpeg-sidecar / ffmpeg 抽 PCM；以代码与本目录文档为准。

## 相关跨切面（架构）

- [ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) — 总体架构：§1「媒体引擎 = FFmpeg + wgpu」、§6 渲染管线双 FFmpeg 后端、媒体能力→栈映射表。
- [ROADMAP.md](../../architecture/ROADMAP.md) — Phase 2（缩略图/波形/导入）、Phase 6（导出）、Phase 8（转写/语义搜索/字幕）+ AI 推理与音频工程扩展。
- [ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md) — B 层 ort/candle 本地推理（复用 `ort_worker`：超分/抠像/追踪/防抖/补帧）、C 层 FFmpeg 音频工程（loudnorm/降噪/人声分离）。
- [PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) — 1:1 复刻差距（缩略图接线 P1-2/P1-3、预览取源帧 P1-9/P1-10、导入兜底）。⚠️ 历史参考。
- [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md) — 逐模块上游 Swift → Rust 移植地图（Search 子树行级算法、AVFoundation→FFmpeg 笔记）。

## 上游拆解参考

- [上游拆解 · 苹果框架可移植性](../../upstream-analysis/02-苹果框架可移植性.md) — AVFoundation/CoreML/DSWaveformImage/Speech 的可移植性评级与替换方案。
- [上游拆解 · 架构与数据流](../../upstream-analysis/01-架构与数据流.md) — 上游媒体读取层在整体数据流中的位置。

## 相关模块

- [opentake-render](../opentake-render/INDEX.md) — 消费本模块的 `decode_frame_at`（取源帧）与 `VideoEncoder`（导出编码）；`RgbaFrame` 是两侧像素交换类型。
- [opentake-agent](../opentake-agent/INDEX.md) — MCP 工具（`search_media`/`detect_beats`/`tighten_silences`/`smart_reframe`/`get_transcript`）落到本模块分析/搜索函数。
- [opentake-core](../opentake-core/INDEX.md) — 持有 `MediaEngine`、运行后台索引 worker、绕导出 `begin`/`end` 暂停信号。
- [opentake-domain](../opentake-domain/INDEX.md) — 本模块消费的 `MediaAsset` 等值类型来源。

## 源码

```
crates/opentake-media/src/
├── lib.rs                模块声明 + 公开 API re-export + MediaEngine 门面 + extract_audio
├── error.rs              MediaError（thiserror）+ Result 别名
├── cache_key.rs          file_identity_key：SHA256("path|mtime|size") 前 32 hex（上游互读）
├── frame.rs              RgbaFrame 像素值类型（紧凑 RGBA8，跨 media/render 边界）
├── ff.rs                 ffmpeg/ffprobe 二进制发现 + ffprobe JSON + 可用性探测（内部 mod）
├── probe.rs              MediaProbe + parse_probe（纯函数：旋转/时长/fps/音视频轨）
├── library.rs            LibraryStore：内容寻址去重 + 原子 manifest（全局素材库）
├── index_coordinator.rs  work_needed / visual_share / ExportPause（调度内核）
├── decode/
│   ├── mod.rs            解码门面 re-export
│   ├── frame.rs          decode_frame_at / decode_frames_at / fit_within（seek 解帧）
│   └── pcm.rs            extract_pcm / PcmSpec / PcmBuffer（抽音轨→单声道 f32）
├── encode/
│   ├── mod.rs            VideoEncoder（两趟：rawvideo→视频，mux 音频）
│   ├── preset.rs         ExportPreset / VideoCodec / ExportResolution / even_dimension
│   └── mix.rs            mix_clips 线性混音 + 硬限幅 + mono_f32_to_s16le
├── thumbnail/
│   ├── mod.rs            video_thumbnails / video_thumbnail_times / image_thumbnail
│   └── sprite.rs         JPEG 雪碧图网格 + load/save（.thumbs.jpg + .thumbs.json）
├── waveform/
│   ├── mod.rs            waveform / waveform_cached（ffmpeg 抽 PCM）
│   ├── dsp.rs            waveform_sample_count + rms_downsample_normalized（纯算法）
│   └── store.rs          load/save_waveform（.waveform 裸 f32 LE）
├── transcribe/
│   ├── mod.rs            Transcriber trait + 数据模型 + transcribe_file + offsetting
│   ├── whisper.rs        WhisperTranscriber（feature whisper-backend）
│   ├── locale.rs         match_locale / best_supported_locale（BCP-47）
│   ├── cache.rs          TranscriptCache（内存 LRU=4 + 磁盘 JSON + range 过滤）
│   └── search.rs         search / SpokenHit（AND 子串 + NFD 折叠）
├── search/
│   ├── mod.rs            语义搜索 facade（Embedder/Hit/SamplerOptions/CancelToken…）
│   ├── config.rs         SearchIndexConfig 常量 + manifest（dim/imageSize/contextLength…）
│   ├── embedder.rs       Embedder trait + EmbedderSpec + preprocess_image（squash 黑底）
│   ├── ort_embedder.rs   OrtEmbedder（feature ort-backend）
│   ├── tokenizer.rs      SiglipTokenizer + pad_or_truncate（截断 64 右填 0）
│   ├── frame_sampler.rs  sample_frames + luma_grid + ShotDetector（去重抽帧）
│   ├── indexer.rs        index_video / index_image / needs_index / accumulate_rows
│   ├── embed_store.rs    PALMEMB1 二进制 load/save（f16 落盘 / f32 内存）
│   ├── ranker.rs         rank：矩阵·向量 + best-per-shot + limit-then-floor
│   └── model_download.rs Manifest + 下载 + SHA256 校验 + 解压安装
├── ort_worker/
│   ├── mod.rs            ExecutionProvider + IoSpec + OrtModel（通用 ONNX 推理面）
│   └── tensor.rs         frame_to_hwc / hwc_to_nchw_normalized / mean_pool
└── analysis/
    ├── mod.rs            分析模块 re-export
    ├── beat.rs           detect_beats（能量包络 onset）
    ├── silence.rs        detect_silences（RMS 阈值）
    └── autocrop.rs       detect_autocrop（黑边/透明区扫描）
```

源文件树根：`../../../crates/opentake-media/src/`

---

## 页脚

- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
