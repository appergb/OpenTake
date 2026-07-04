# opentake-media — 模块总览

> 上级：[模块目录 INDEX.md](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)

---

## 1. 一句话定位与依赖分层

`opentake-media` 是**媒体读取与离线分析层**：把上游 Palmier Pro 基于 AVFoundation / DSWaveformImage / macOS Speech / CoreML 的媒体栈，移植为跨平台 Rust——**探测 / 解码 / 编码、缩略图、波形、转写、SigLIP2 语义搜索、节拍/静音/自动裁剪分析、全局素材库**。它**不含 wgpu 帧合成器**（那是 [opentake-render](../opentake-render/INDEX.md)），只产出纯值类型（`RgbaFrame` / `PcmBuffer` / 领域类型）供上层消费。

依赖分层（只向下依赖）：

```
opentake-domain          值语义叶子层（Timeline/Clip/MediaAsset…，禁 I/O）
   ▲
opentake-media           ← 本模块：第一层允许 I/O 的 crate（ffmpeg / 文件 / 网络）
   ▲
opentake-core / src-tauri / opentake-agent / opentake-render   调用方
```

- **依赖**：仅 `opentake-domain`（消费 `MediaAsset` 等值类型）。
- **被调用**：`opentake-core`（会话 / 后台索引调度运行时）、`src-tauri`（媒体/库/导出 Tauri 命令）、`opentake-agent`（MCP 工具如 `search_media` / `detect_beats`）、`opentake-render`（导出时的 `VideoEncoder` 编码后端、`decode_frame_at` 取源帧）。
- **运行期依赖**：**FFmpeg ≥ 6.0 在 `PATH`**（打包后从 `OPENTAKE_FFMPEG` / `OPENTAKE_FFPROBE` 读取，见 [probe-ff.md](probe-ff.md)）。重 ML 后端（whisper / ort）藏在 feature 之后，**默认 build 与测试完全离线、不链接任何原生 ML**。

顶层门面是 `MediaEngine`（`lib.rs`）：持有缓存根目录（Tauri `app_cache_dir`）+ 模型目录（`app_data_dir`）+ 共享的 `ExportPause` 信号，把高层操作（probe / 缩略图 / 波形 / 转写 / 口语搜索 / 抽音轨）包成方法；重 ML 方法接收调用方构造好的后端实例（feature 实现或 mock），自身**不绑定后端**。

---

## 2. 职责边界（做什么 / 不做什么）

**做：**
- 媒体探测（时长 / 旋转校正后的分辨率 / fps / 有无音视频轨）。
- 解码：seek 解单帧/批量帧为 `RgbaFrame`；抽音轨为单声道 f32 `PcmBuffer`。
- 编码：把合成好的 RGBA 帧序列 + 混音 PCM 编码成容器（导出后端的编码器 + 预设表 + 线性混音）。
- 缩略图：视频缩略图序列 + JPEG 雪碧图磁盘缓存；图片单缩略图。
- 波形：解 PCM → RMS 降采样 → 归一化桶 + `.waveform` 二进制缓存。
- 转写：`Transcriber` trait + 数据模型 + locale 匹配 + 双层缓存 + 转写内关键词搜索（whisper 后端在 feature 后）。
- 语义搜索：SigLIP2 双编码器、视觉去重抽帧、单素材索引、`PALMEMB1` 嵌入存储、纯函数排名、模型下载校验（ort 后端在 feature 后）。
- 离线分析：节拍检测、静音检测、自动裁剪（黑边裁剪）。
- 全局素材库：内容寻址去重 + JSON manifest 原子写。
- 后台索引/转写调度的**可移植内核**（判断该做什么 + 导出暂停计数器）。

**不做（有意省略，证据见各子系统）：**
- **不做 wgpu 帧合成**：多轨叠加 / transform / crop / opacity ramp 合成在 `opentake-render`。本模块只给单帧解码与单帧编码原语。
- **不做帧↔秒折算**：本 crate 一律以**秒（`f64`）**作 IO 边界量；帧↔秒换算（`Int(s*fps)` 截断）留在 `opentake-domain` / 调用层（移植铁律，见 §6）。
- **不持 UI 状态 / 内存缓存表 / "触发重绘"**：上游 `MediaVisualCache` 的 @MainActor 内存表 + in-flight 去重 + `needsDisplay` 属于上层（`opentake-core` / 前端），本模块只提供纯生成 + 磁盘缓存函数。
- **不做后台 worker 运行时**：`index_coordinator.rs` 只是判断内核（`work_needed` / `ExportPause`），tokio 队列 / 重试 / 并发转写在 `opentake-core`。
- **不移植 `AlphaVideoNormalizer`（直 alpha → 预乘转码）**：wgpu 合成器内直接处理 premultiplied alpha，此类整类消失（见 [encode.md](encode.md)）。

---

## 3. 关键概念与数据流

### FFmpeg sidecar 编解码（与 SPEC 的关键实现偏差）
**实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](probe-ff.md)。

### 缩略图 / 波形 / 转写 / 语义搜索的总体管线

| 管线 | 数据流 | 缓存与磁盘格式 |
|---|---|---|
| 缩略图 | `video_thumbnail_times(duration)` → `decode_frames_at`（ffmpeg seek 解帧到 120×68 RGBA） → 拼 JPEG 雪碧图 | `<cache>/MediaVisualCache/<key>.thumbs.jpg` + `.thumbs.json`（sidecar 最后写 = 完整标记） |
| 波形 | `extract_pcm`（**ffmpeg** 抽 22050Hz 单声道 f32） → RMS 降采样到 N 桶 → 归一化（0=响,1=静） | `<cache>/MediaVisualCache/<key>.waveform`（裸 `[f32]` LE） |
| 转写 | `extract_pcm`（16k mono f32） → `Transcriber`（whisper） → `offsetting` 时间码回移 | `<cache>/Transcripts/<key>.json` + 内存 LRU=4 |
| 语义搜索 | `sample_frames`（视觉去重抽帧） → `Embedder::encode_image`（768 维） → `accumulate_rows` → 存储；查询：`encode_text` → 矩阵·向量 → best-per-shot → 截断 | `<cache>/Embeddings/<key>.embed`（`PALMEMB1` 二进制，f16 落盘） |

缓存键统一为 `cache_key::file_identity_key(path, 32)` = `SHA256("<path>|<mtime_secs_f64>|<size>")` 前 16 字节 → 32 hex，**与上游同机缓存目录可互读**（含 Swift `Double.description` 整数补 `.0` 的逐字节对齐，见 [library-index.md](library-index.md)）。

### 与 render / agent 的关系
- **→ render**：导出时 render 逐帧 `Compositor::render_to_rgba` → 本模块 `VideoEncoder::push_frame`；暂停态预览时 render 经 `FrameProvider` 适配器调 `decode_frame_at`。`RgbaFrame` 是两侧唯一的像素交换类型（不泄漏 wgpu / ffmpeg 类型）。
- **→ agent / MCP**：`search_media`（语义+口语搜索）、`detect_beats` / `auto_cut_to_beats`（节拍）、`tighten_silences`（静音）、`smart_reframe`（自动裁剪）、`get_transcript`（转写）等 MCP 工具最终落到本模块的分析 / 搜索函数。
- **→ core**：`ExportPause` 由 render 导出时 `begin`/`end`，core 后台索引 worker 轮询让路；`work_needed` 判断每个素材是否需要视觉索引 / 转写。

---

## 4. 对应上游 Swift（AVFoundation → FFmpeg）

逐模块映射见 [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md)（行级算法笔记）；可移植性评级见 [上游拆解 · 苹果框架可移植性](../../upstream-analysis/02-苹果框架可移植性.md)。

| 本模块子系统 | 上游 Swift 真理来源 | 替换点 |
|---|---|---|
| 探测 `probe.rs` | `MediaAsset.loadMetadata`（`MediaAsset.swift:96-162`） | AVFoundation → ffprobe JSON |
| 解码帧 `decode/frame.rs` | `AVAssetImageGenerator`（多处） | → ffmpeg `-ss` seek + rawvideo RGBA |
| 抽 PCM `decode/pcm.rs` | `Transcription.extractAudioTrack`（`Transcription.swift:203-280`） | `AVAssetReaderTrackOutput` → ffmpeg `-vn -ac -ar -f` |
| 编码 / 预设 `encode/` | `ExportService` / `ImageVideoGenerator`（BT.709） | `AVAssetExportSession` → ffmpeg `libx264`/`libx265`/`prores_ks` |
| 缩略图 `thumbnail/` | `MediaVisualCache`（缩略图 sprite，`MediaVisualCache.swift`） | CGContext → `image` crate + ffmpeg seek |
| 波形 `waveform/` | `MediaVisualCache` 波形分支（外包 `DSWaveformImage`） | DSWaveformImage → ffmpeg PCM + 自算 RMS |
| 转写 `transcribe/` | `Transcription` / `TranscriptCache` / `TranscriptSearch` | macOS Speech → whisper.cpp（`whisper-rs`） |
| 语义搜索 `search/` | `Search/`（`VisualEmbedder`/`FrameSampler`/`EmbeddingStore`/`VisualSearch`/`TextTokenizer`/`ModelDownloader`） | CoreML → ONNX Runtime（`ort`） |
| ort worker `ort_worker/` | （新增，无直接上游） | 进阶 AI 特性的通用 ONNX 推理面 |
| 节拍/静音 `analysis/beat,silence` | （新增，对应 MCP `detect_beats`/`tighten_silences`） | 纯算法 |
| 自动裁剪 `analysis/autocrop` | （对应 MCP `smart_reframe`） | 当前仅黑边裁剪（非人脸/显著性） |
| 素材库 `library.rs` | **上游无对应**（OpenTake 新增，#37/#104） | 不要求 1:1 |

---

## 5. 完成状态（已实现 vs 计划中）

对照 [ROADMAP.md](../../architecture/ROADMAP.md) Phase 2 / Phase 8、[ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md)、[PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) 与代码现况：

**已实现（代码 + 单测齐备）：**
- 探测 / 解码帧 / 抽 PCM / ffmpeg sidecar 封装（含旋转校正、零声道音轨防幻影链接）。
- 视频缩略图序列 + JPEG 雪碧图磁盘缓存（与上游 key/meta 互读）、图片单缩略图。
- 波形（ffmpeg PCM → RMS → 归一化 + `.waveform` 缓存）。
- 编码器 + 预设表（H.264/H.265/ProRes）+ 线性音频混音（逐 clip 偏移 + 增益 + 硬限幅，第二趟 ffmpeg mux AAC / `-shortest`，mux 失败回退视频-only）。
- 转写数据模型 + locale 匹配 + 双层缓存 + 转写内关键词搜索（纯逻辑全测）；whisper 后端在 `whisper-backend` feature 后。
- 语义搜索全链路纯函数（预处理 / tokenize / 视觉去重抽帧 / 索引累积 / `PALMEMB1` 存储 / 排名 / 模型下载校验）；ort 后端在 `ort-backend` feature 后；默认 build 用 mock 离线可测。
- 节拍检测、静音检测、自动裁剪（黑边）。
- 全局素材库（内容寻址去重 + 原子 manifest，#104；Tauri 命令层在 src-tauri，#106）。
- 后台索引调度内核（`work_needed` / `visual_share` / `ExportPause`）。

**计划中 / 待收口：**
- **whisper / ort 后端真实接线**：trait + feature 后端已就位，模型托管与端到端验证属 Phase 8。`model_download` 的 `Manifest` sha256/bytes 仍为占位空值，待填实际 ONNX 资产。
- **自动裁剪升级**：当前 `autocrop` 仅做黑边/透明区扫描，**未集成人脸/显著性 ML**（SPEC 的 `smart_reframe` 完整语义为计划中）。
- **编码导出**：H.264/.mp4 全分辨率逐帧导出 spine 已落地（#112），**H.265/ProRes 预设 + 进度/取消**待补；音频重采样曲线 / pan / 立体声 / 动态处理为后续。
- **进阶 AI 推理（ADVANCED-FEATURES B 层，复用 `ort_worker`）**：超分 / 抠像 / 运动追踪 / 防抖 / 补帧——`ort_worker` 通用面已铺好，特性本身计划中。
- **进阶音频工程（C 层）**：loudnorm/EBU R128、降噪、人声分离（FFmpeg 滤镜 / Demucs via ort）计划中。
- **缩略图接线 gap**：底层 `video_thumbnails`/`image_thumbnail` 已实现，但导入路径 `MediaItemDto.thumbnail` 一度写死 `None`——属上层接线问题（[PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) P1-2/P1-3），不是本模块缺能力。
- **素材库前端**（#37-C/#56）未做；后端存储层已完成。

---

## 6. 移植铁律（本模块必须遵守）

来自 [SPEC.md](SPEC.md) §0、[AGENTS.md](../../../AGENTS.md) 与上游拆解，落地为：

1. **整数帧 / 秒分层**：本 crate 一律用**秒（`f64`）**与源采样位置作 IO 边界量；**不做** fps 折算（留给 domain / 调用层，`secondsToFrame` 用截断 `Int(s*fps)`）。上游 `Transcription`/`MediaVisualCache`/`FrameSampler` 同样全用秒。
2. **波形用 ffmpeg `extract_pcm`，不用 symphonia**：SPEC / ARCHITECTURE 原计划 Symphonia 纯 Rust 解 PCM，但实测 Symphonia 解不出 `.mov` 容器里的非 AAC 编码等情形导致波形失效，故波形改走与 probe/缩略图同一条 ffmpeg sidecar 路径，成功率与 ffmpeg 一致（证据见 [waveform.md](waveform.md) 与 `waveform/mod.rs` 注释）。
3. **缓存键与磁盘格式逐字节复刻**：`SHA256("path|mtime_f64|size")` 前 32 hex、`PALMEMB1` 布局、`.waveform`/`.thumbs.jpg`+`.thumbs.json`、转写 JSON——保证与上游/旧工程同机缓存可互读。注意 Swift `Double.description` 对整数秒补 `.0` 的对齐（[library-index.md](library-index.md)）。
4. **数值常量逐字照搬、零硬编码散落**：promoteDiff=12、coverageFloor=8.0、imageSize=256、dim=768、contextLength=64、relativeCutoff=0.85、cosineFloor=0.05、波形 150 桶/秒 与 4000/20000 边界、缩略图 120×68、雪碧图 50 列、Rec.601 luma 系数 .299/.587/.114……均以 `pub const` / `Options` 集中声明，值照搬上游。
5. **`#[serde(default)]` + `Option<T>` 容旧**：所有落盘模型（manifest / header / 转写 JSON）字段加默认，读旧工程不破坏。
6. **错误用 `thiserror`（`MediaError`），内部传播 `anyhow`，边界返回 `Result<T, MediaError>`**；`opentake-domain` 零 I/O，本 crate 是第一层允许 I/O 的 crate。
7. **纯函数优先 + 后端 trait 可插拔**：排名 / 降采样 / 抽帧判定 / 转写过滤 / 帧请求合成全是无副作用纯函数，可全单测；`Embedder` / `Transcriber` 是 trait，重后端 feature-gated，测试注入 mock。
8. **导出让路**：任何后台任务（索引 / 转写）在导出活跃时暂停（`ExportPause` 引用计数跨窗口共享）。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
