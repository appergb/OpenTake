# opentake-media 实现就绪规格 (Issue #8)

> 范围:`crates/opentake-media`。把上游基于 AVFoundation / DSWaveformImage / macOS 26 Speech / CoreML 的媒体读取层,移植为跨平台 Rust:**ffmpeg-next 解码/编码/缩略图/抽 PCM、Symphonia 波形、whisper-rs 转写、candle/ort + SigLIP2 语义搜索、ort 通用推理 worker**。本 crate 是媒体**读取与离线分析**层,不含 wgpu 帧合成器(那在 `opentake-render`,见 §9 边界)。
>
> 状态:设计规格。对应 ROADMAP **Phase 2**(缩略图/波形,易)与 **Phase 8**(转写/语义搜索/进阶 AI worker)。本 crate 在 workspace 已有空壳 `crates/opentake-media/{Cargo.toml,src/lib.rs}`(`crate_compiles` 占位测试)。
>
> 真理来源(均为只读上游,绝对路径):
> - 解码/编码/缩略图/PCM:`palmier-pro-upstream/Sources/PalmierPro/Preview/{ImageVideoGenerator,AlphaVideoNormalizer,TimelineRenderer}.swift`、`Transcription/Transcription.swift`(`extractAudioTrack`)、`Timeline/MediaVisualCache.swift`(缩略图 sprite + 波形 + 磁盘缓存)。
> - 转写:`Transcription/{Transcription,TranscriptCache,TranscriptSearch}.swift`。
> - 语义搜索:`Search/{SearchIndexConfig,SearchIndexCoordinator}.swift`、`Search/Models/{VisualEmbedder,VisualModelLoader,ModelDownloader,TextTokenizer}.swift`、`Search/Indexing/{FrameSampler,VisualIndexer,EmbeddingStore}.swift`、`Search/Query/VisualSearch.swift`。
> - 横切分析:`docs/_analysis/02-苹果框架可移植性.md`、`docs/_analysis/01-架构与数据流.md`、`docs/MODULE-PORT-MAP.md`(行级算法笔记 L833–883、L923–940、L1211)。
> - 架构/相位:`docs/ARCHITECTURE.md` §1/§6/§7、`docs/ROADMAP.md` Phase 2/8、`docs/ADVANCED-FEATURES.md`(ort worker 复用方)。
> - 领域契约:`crates/opentake-domain/src/{media,clip_type,timeline,clip}.rs`(本 crate 消费方,不可改)。

---

## 0. 设计原则与移植铁律(本 crate 必须遵守)

来自 `OpenTake/AGENTS.md`「Rust 代码风格 / 移植法则」与 `docs/_analysis/02`,逐条落地:

1. **时间单位分层**:本 crate 一律用**秒(`f64`)**与**源采样位置**作 IO 边界量;帧↔秒换算(`Int(s*fps)` 截断)留在 `opentake-domain`/调用层,本 crate**不做** fps 折算。证据:上游 `Transcription`/`MediaVisualCache`/`FrameSampler` 全用 `seconds`,`secondsToFrame` 在 `MediaTab`(上层)。
2. **零硬编码常量**:所有阈值(promoteDiff=12、coverageFloor=8.0、imageSize=256、dim=768、relativeCutoff=0.85、cosineFloor=0.05、波形 count 公式 150/帧 与 20000 上限、缩略图 maximumSize=120×68、sprite 列数=50 …)以 `pub const` / `Options` 结构体集中声明,值**逐字照搬**上游。
3. **缓存键与磁盘格式逐字节复刻**:`SHA256("path|mtime_unix_f64|size")` 取前 N hex、`PALMEMB1` 二进制布局、`.waveform`/`.thumbs.jpg`+`.thumbs.json` sidecar、转写 JSON。理由:让 OpenTake 与上游/旧工程的缓存目录**可互读**(同机迁移),并保证幂等判定一致。
4. **错误用 `thiserror` 定义本 crate 错误,内部传播用 `anyhow`,边界返回 `Result<T, MediaError>`**;`opentake-domain` 零依赖,本 crate 是第一层允许 IO 的 crate。
5. **不可变 / 纯函数优先**:排名(`VisualSearch`)、波形降采样、采样判定、转写过滤等都是无副作用纯函数,可全单测;有状态的只有索引调度器(§7.7)与模型加载器(§5.6)。
6. **后端推理可插拔**:`Embedder` / `Transcriber` / `OrtWorker` 定义为 trait,默认实现走 ort 或 candle;测试注入 mock(协议化 DI)。
7. **导出期让路**:任何后台任务(索引/缩略图/波形)在导出活跃时暂停。证据:上游 `ExportService.isExporting.didSet → SearchIndexCoordinator.exportDidBegin/End`(`MODULE-PORT-MAP` L457)、`SearchIndexCoordinator.waitWhileExportActive`(`SearchIndexCoordinator.swift:49`)。
8. **L2 归一化对齐风险**:上游裸点积 `cblas_sgemv` 是否等价余弦,取决于导出模型是否在图内 L2 归一化(`MODULE-PORT-MAP` L860)。本 crate **必须复用上游同一份权重转 ONNX**,并在 `Embedder::encode` 后做一次**条件 L2 归一化开关**(`Spec.normalized: bool`),默认 false 以匹配上游(模型内已归一化)——除非验证证明需要外部归一化。

---

## 1. crate 结构与依赖

### 1.1 模块布局(`many small files`,每文件 <400 行)

```
crates/opentake-media/
├── Cargo.toml
└── src/
    ├── lib.rs              # pub re-export + MediaError + 顶层 facade(MediaEngine)
    ├── error.rs            # MediaError(thiserror)
    ├── cache_key.rs        # SHA256("path|mtime|size") 通用缓存键(被 §2/§4/§6 共用)
    ├── probe.rs            # ffprobe 等价:时长/分辨率(应用旋转)/fps/hasAudio  → MediaProbe
    ├── decode/
    │   ├── mod.rs          # Decoder facade
    │   ├── frame.rs        # seek+解一帧(缩略图/采样/取帧共用) → RgbaFrame
    │   ├── pcm.rs          # 抽音轨 → f32/i16 PCM(16k mono;任意 sr/ch/range)
    │   └── reader.rs       # 顺序解帧迭代器(供 render 预览/导出后端复用)
    ├── encode/
    │   ├── mod.rs          # Encoder facade(供 render 导出后端调用)
    │   ├── still.rs        # 图片→编码(取代 ImageVideoGenerator,见 §3.3:多数场景不再需要)
    │   └── preset.rs       # 导出预设映射(H.264/H.265/ProRes × 720/1080/4K)
    ├── thumbnail/
    │   ├── mod.rs          # 视频缩略图序列 + 图片单缩略图
    │   └── sprite.rs       # JPEG sprite 网格 + JSON sidecar 磁盘缓存(照搬 MediaVisualCache)
    ├── waveform/
    │   ├── mod.rs          # Symphonia 解 PCM → RMS 降采样 → 归一化 0..1
    │   └── store.rs        # .waveform 二进制缓存(Vec<f32> LE)
    ├── transcribe/
    │   ├── mod.rs          # Transcriber trait + 数据模型(TranscriptionResult/Word/Segment)
    │   ├── whisper.rs      # whisper-rs 后端(word/segment 时间戳)
    │   ├── locale.rs       # matchLocale / bestSupportedLocale(纯逻辑,照搬)
    │   ├── cache.rs        # TranscriptCache(内存 LRU=4 + 磁盘 JSON + filter)
    │   └── search.rs       # TranscriptSearch(精确关键词,AND 子串,折叠大小写/变音)
    ├── search/
    │   ├── mod.rs          # 视觉语义搜索 facade
    │   ├── embedder.rs     # Embedder trait + SigLIP2 预处理(squash-resize 黑底 256²)
    │   ├── tokenizer.rs    # SigLIP tokenize(HF tokenizers,截断 64 + 右填 0)
    │   ├── frame_sampler.rs# 视觉去重抽帧(luma 8×8 grid + 镜头边界 + 覆盖下限)
    │   ├── indexer.rs      # 单素材索引(帧→embedding→store,幂等)
    │   ├── embed_store.rs  # EmbeddingStore PALMEMB1 二进制(f16 落盘 / f32 内存)
    │   ├── ranker.rs       # VisualSearch 打分(矩阵·向量 + best-per-shot + 截断)
    │   ├── model_download.rs# 权重下载/SHA256 校验/解压/安装(reqwest+sha2+zip)
    │   └── config.rs       # SearchIndexConfig 等价(manifest、阈值、host URL)
    ├── ort_worker/
    │   ├── mod.rs          # 通用 OrtWorker / OrtModel(供超分/抠像/追踪等进阶特性复用)
    │   └── tensor.rs       # ndarray ↔ ort Value 互转、NCHW/NHWC、归一化辅助
    └── index_coordinator.rs# 后台索引/转写调度(tokio 单 worker 队列 + 导出暂停)
```

### 1.2 Cargo 依赖(建议版本范围,实施时锁定)

> ⚠️ 本规格**禁止**直接改 `Cargo.toml`/跑 `cargo`;下列为实施 PR 应填入的依赖,供评审。

```toml
[dependencies]
opentake-domain = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = "2"
anyhow = "1"
sha2 = "0.10"            # 缓存键 / 下载校验
half = "2"              # f16 ↔ f32(EmbeddingStore)
byteorder = "1"         # PALMEMB1 LE 读写
ndarray = "0.16"        # 向量矩阵、tensor
tracing = "0.1"         # 替 Sentry/Log,本地结构化日志
tokio = { version = "1", features = ["rt", "sync", "time", "macros"] }
unicode-normalization = "0.1" # TranscriptSearch 变音不敏感(NFD)

# 媒体编解码
ffmpeg-next = "7"       # libav*(解码/编码/缩略图/抽 PCM);构建依赖系统 FFmpeg
# 波形
symphonia = { version = "0.5", features = ["all-codecs", "all-formats"] }
# 转写
whisper-rs = "0.14"     # whisper.cpp 绑定(word/segment 时间戳)
# 语义搜索 / 推理 worker(二选一为主,另一备选)
ort = "2"               # ONNX Runtime(默认推理后端,跨平台、可 EP 加速)
tokenizers = "0.20"     # HF Rust tokenizer(= swift-transformers 同源)
# 下载/解压(模型权重)
reqwest = { version = "0.12", features = ["stream"] }
zip = "2"               # 替代 /usr/bin/ditto
image = "0.25"          # 图片解码/缩放/JPEG sprite 编码(无 ffmpeg 时的图片路径)

[features]
default = ["ort-backend"]
ort-backend = []        # 默认用 ort 跑 SigLIP2
candle-backend = []     # 备选:candle-core/candle-transformers(纯 Rust,见 §5.7)

[dev-dependencies]
serde_json = { workspace = true }
tempfile = "3"
```

依据 `docs/_analysis/02` 成熟度判断:ffmpeg-next/symphonia/whisper-rs/ort 全部「成熟、机械移植」,blocker 不在本 crate(blocker 是 `opentake-render` 的 wgpu 合成器)。

### 1.3 顶层错误类型

```rust
// error.rs
#[derive(thiserror::Error, Debug)]
pub enum MediaError {
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("ffmpeg: {0}")] Ffmpeg(String),
    #[error("no {0} track in {1}")] NoTrack(&'static str, String),   // ("audio","x.mp4")
    #[error("decode failed: {0}")] Decode(String),
    #[error("encode failed: {0}")] Encode(String),
    #[error("transcription unsupported locale: {0}")] UnsupportedLocale(String),
    #[error("transcription failed: {0}")] Transcribe(String),
    #[error("model install: {0}")] ModelInstall(String),
    #[error("checksum mismatch: {0}")] Checksum(String),
    #[error("embedding store corrupt")] StoreCorrupt,
    #[error("bad model output")] BadModelOutput,
    #[error("cancelled")] Cancelled,
    #[error(transparent)] Other(#[from] anyhow::Error),
}
pub type Result<T> = std::result::Result<T, MediaError>;
```
对齐上游错误枚举:`ImageVideoError`(`ImageVideoGenerator.swift:208`)、`NormalizeError`(`AlphaVideoNormalizer.swift:151`)、`TranscriptionError`(`Transcription.swift:41`)、`DownloadError`(`ModelDownloader.swift:39`)、`VisualEmbedder.ModelError`(`VisualEmbedder.swift:20`)、`EmbeddingStore.StoreError`(`EmbeddingStore.swift:28`)。

### 1.4 通用缓存键(三处共用)

上游三个独立实现完全同构(只前缀字符数不同),归一为一个函数:

```rust
// cache_key.rs
/// SHA256("<path>|<mtime_secs_unix_f64>|<size_bytes>") 的小写 hex,取前 `prefix_chars` 字符。
/// 注意:mtime 是相对 1970 的浮点秒(Swift timeIntervalSince1970)。文件不存在/无属性 → None。
pub fn file_identity_key(path: &Path, prefix_chars: usize) -> Option<String>;
```
- 嵌入向量 / 转写:`prefix_chars = 32`。证据:`EmbeddingStore.key`(`EmbeddingStore.swift:36-42`,`.prefix(32)`)、`TranscriptCache.key`(`TranscriptCache.swift:82-88`,`.prefix(32)`)。
- 缩略图 / 波形:`prefix_chars = 16`。证据:`MediaVisualCache.diskCacheKey`(`MediaVisualCache.swift:209-216`,`digest.prefix(16)` 即 16 字节 → 32 hex;**注意上游这里取的是 16 字节= 32 hex**,而 Embeddings 取 32 hex 字符= 16 字节)。

> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。

---

## 2. ffmpeg-next:解码 / 编码 / 缩略图(seek 解帧)/ 抽 PCM

### 2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)

上游 `MediaAsset.loadMetadata`(`MediaAsset.swift:96-162`)用 AVFoundation 读:时长(优先 video track timeRange,否则 asset duration)、`naturalSize.applying(preferredTransform)` 校正后的像素宽高、`nominalFrameRate`、是否有音轨。Rust 用 ffprobe 等价。

```rust
// probe.rs
#[derive(Clone, Debug, PartialEq)]
pub struct MediaProbe {
    pub duration_secs: f64,        // 优先视频流 duration,回退容器 duration
    pub width: Option<u32>,        // 已应用 rotate side-data / display matrix
    pub height: Option<u32>,
    pub fps: Option<f64>,          // avg_frame_rate(对齐 nominalFrameRate 语义)
    pub has_audio: bool,
    pub has_video: bool,
}

/// 打开容器,读首个视频流 + 是否存在音频流。零解码(仅读头/流参数)。
pub fn probe(path: &Path) -> Result<MediaProbe>;
```
实现要点(逐项对齐上游):
- **旋转校正**:ffmpeg `AV_PKT_DATA_DISPLAYMATRIX` side-data → 90/270° 时交换宽高,等价 `appliesPreferredTrackTransform`/`size.applying(transform)`(`MediaAsset.swift:133-136`)。
- **时长回退顺序**:video stream `duration` → 容器 `duration`(`MediaAsset.swift:141-147`)。
- **fps**:用 `avg_frame_rate`(若 0 用 `r_frame_rate`),映射 `sourceFPS`(`MediaAsset.swift:138`)。
- **has_audio**:存在 audio stream(`MediaAsset.swift:148-150`)。

填回 `opentake-domain::media::MediaAsset` 的 `duration/source_width/source_height/source_fps/has_audio` 字段(见 §9.1)。

### 2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)

上游三处用 `AVAssetImageGenerator`:`MediaVisualCache`(缩略图,`maximumSize=120×68`,tolerance 1s,`MediaVisualCache.swift:113-118`)、`FrameSampler`(采样,`maximumSize=512×512`,tolerance `max(interval/2,1)`,`FrameSampler.swift:54-60`)、`MediaAsset`(首帧缩略图 320²,`MediaAsset.swift:152-159`)。统一为一个带 tolerance 的「seek 到最近关键帧 + 解码 + 缩放」函数。

```rust
// decode/frame.rs
#[derive(Clone)]
pub struct RgbaFrame { pub width: u32, pub height: u32, pub rgba: Vec<u8> } // 紧凑 RGBA8

pub struct FrameRequest {
    pub time_secs: f64,
    pub max_size: (u32, u32),     // 等比缩放上界(对齐 maximumSize 语义:不放大,保宽高比)
    pub tolerance_secs: f64,      // 允许落到最近的解码可达帧(≈ requestedTimeTolerance)
    pub apply_rotation: bool,     // 默认 true(= appliesPreferredTrackTransform)
}

/// `-ss` 到 time-tolerance 起点附近的关键帧,解码至首个 pts ≥ (time-tolerance) 的帧;
/// 返回其实际 pts 与 RGBA。失败/越界返回 Err。
pub fn decode_frame_at(path: &Path, req: &FrameRequest) -> Result<(f64 /*actual_secs*/, RgbaFrame)>;

/// 批量(用于缩略图序列/采样):升序时间点,内部单次顺序解码尽量复用 decoder,
/// 对每个目标时间产出最近帧;跳过解不出的点。返回按实际时间升序。
pub fn decode_frames_at<'a>(path: &'a Path, times_secs: &'a [f64], opts: &'a FrameRequest)
    -> impl Iterator<Item = Result<(f64, RgbaFrame)>> + 'a;
```
要点:
- **缩放语义**:`max_size` = 等比缩放上界(上游 `maximumSize` 是「不超过此框、保宽高比」),用 swscale `SWS_BILINEAR`/`area`。**注意区分** §5 的 SigLIP 预处理用的是 squash-resize(忽略宽高比,见 §5.2),二者不同函数。
- **tolerance**:上游用 `requestedTimeToleranceBefore/After` 让解码器取最近 sync frame(避免逐帧解到精确帧)。Rust 用 `-ss` 到 `time-tolerance` 处 seek 到关键帧后顺序解到第一个 `pts ≥ time - tolerance`,即可。
- **去重**:批量路径要复刻 `t > lastTime` 去重(`FrameSampler.swift:74`)——同一关键帧被多个近邻时间点命中只产一次。

### 2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)

上游 `extractAudioTrack`(`Transcription.swift:203-280`)用 `AVAssetReaderTrackOutput` 解码音轨为 **16kHz / 单声道 / s16le / interleaved**,可选 `CMTimeRange` 截取,落 `.caf`。whisper 需要 **16k mono f32**。

```rust
// decode/pcm.rs
pub struct PcmSpec { pub sample_rate: u32, pub channels: u16, pub format: PcmFormat }
pub enum PcmFormat { S16Le, F32 }

/// 解码 `path` 的首条音轨为指定 PCM;`range` = 绝对秒区间(等价 CMTimeRange 端到端)。
/// 无音轨 → Err(NoTrack("audio", …))。
pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Result<PcmBuffer>;

pub struct PcmBuffer { pub spec: PcmSpec, pub samples_f32: Vec<f32> } // 始终内部转 f32 mono 供下游
```
要点:
- **重采样到 16k mono**:swresample。whisper-rs 吃 `&[f32]`(16k mono),所以 `extract_pcm(path, &PcmSpec{16000,1,F32}, range)` 直接喂 whisper。
- **range 语义**:`-ss lower -to upper`(绝对秒),对齐 `reader.timeRange`(`Transcription.swift:226-231`)。下游转写对截取结果做 `offsetting(by: lower)` 把时间码移回源时间(见 §6.1)。
- **波形复用**:波形(§4)默认走 Symphonia(纯 Rust,无 ffmpeg 链接成本);但若已 ffmpeg 解码,也可复用本函数取整段 PCM。两条路径都要产出**相同的归一化样本**(测试断言一致),实施时以 Symphonia 为准。

### 2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)

上游导出在 `Export/ExportService.swift`(`AVAssetExportSession`,preset 名按 H.264/H.265/ProRes × 720/1080/4K 映射,`docs/_analysis/02` §1.3)。本 crate 提供**编码器后端 + 预设表**;**逐帧合成由 `opentake-render` 的 wgpu 完成**,本 crate 只负责把合成出的 RGBA 帧序列 + 混音 PCM 编码成容器。

```rust
// encode/preset.rs
pub enum VideoCodec { H264, H265, ProRes422 }
pub enum ExportResolution { P720, P1080, P2160 } // 短边

pub struct ExportPreset {
    pub codec: VideoCodec,
    pub resolution: ExportResolution,
    // 实施时逐项调参逼近上游 preset 的码率/profile/色彩(BT.709)。详见 §10 验收。
}
```
```rust
// encode/mod.rs
pub struct VideoEncoder { /* ffmpeg encoder ctx */ }
impl VideoEncoder {
    pub fn new(out: &Path, w: u32, h: u32, fps: i32, preset: &ExportPreset) -> Result<Self>;
    pub fn push_frame(&mut self, rgba: &RgbaFrame, pts_frame: i64) -> Result<()>;
    pub fn push_audio(&mut self, pcm: &PcmBuffer) -> Result<()>;
    pub fn finish(self) -> Result<()>;
}
```
要点:
- **renderSize 取偶数**:`even(value) = max(2, round/2*2)`,逐字照搬 `TimelineRenderer.even`(`TimelineRenderer.swift:85`)与 `ImageVideoGenerator.encoderDimension`(`ImageVideoGenerator.swift:68-72`,H.264 拒绝奇数尺寸/`max(2, pixels - pixels%2)`)。该函数放 `opentake-render`(渲染尺寸决策),本 crate 编码器只接收已偶数化的尺寸。
- **色彩管线**:H.264/H.265 写 BT.709 primaries/transfer/matrix,对齐 `ImageVideoGenerator.writeStillVideo`(`ImageVideoGenerator.swift:168-174`)与 `CompositionBuilder` 锁 BT.709(`docs/_analysis/02` L31)。
- **ProRes422 + LPCM**:对齐上游 ProRes preset(`docs/_analysis/02` §1.3 L45)。
- **alpha 预乘**(`AlphaVideoNormalizer.swift`):上游为「直 alpha 视频 → 预乘」做单独转码。在 OpenTake **整类消失**——wgpu 合成器内直接处理 premultiplied alpha(`docs/_analysis/02` 表 L75)。本 crate**不**移植 `AlphaVideoNormalizer`;仅在解码层暴露「该帧是否带 alpha / 是否直 alpha」元数据(读 `pix_fmt`),供 render 决定着色器分支。记录于此以示**有意省略**。

---

## 3. 缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`)

对应 `Timeline/MediaVisualCache.swift`。上游有三类:视频缩略图序列(sprite 网格缓存)、图片单缩略图、波形(§4)。

### 3.1 视频缩略图序列 + 时间点公式

```rust
// thumbnail/mod.rs
pub struct VideoThumb { pub time_secs: f64, pub image: RgbaFrame }

/// 生成视频缩略图序列(命中缓存直接返回)。`on_partial` 用于长视频渐进回调
/// (对齐上游每 50 帧 publish 一次,MediaVisualCache.swift:123)。
pub fn video_thumbnails(path: &Path, duration_secs: f64,
    on_partial: Option<&dyn Fn(&[VideoThumb])>) -> Result<Vec<VideoThumb>>;
```
- **时间点**:`videoThumbnailTimes`(`MediaVisualCache.swift:192-202`)——`interval = duration < 10 ? 1.0 : 2.0`,`stride(from:0, to:duration, by:interval)`。逐字照搬。
- **缩放上界**:`max_size = (120, 68)`(`MediaVisualCache.swift:114`),tolerance 1.0s(`:116-117`),apply_rotation=true(`:115`)。
- **去重**:批量解帧的 `t > lastTime`(本质同 FrameSampler)。
- **渐进发布**:每 50 帧回调一次(`MediaVisualCache.swift:123-129`)。Rust 用回调闭包(UI 进度交给上层 Tauri event)。

### 3.2 图片单缩略图

```rust
pub fn image_thumbnail(path: &Path, max_pixel: u32) -> Result<RgbaFrame>; // max_pixel 默认 120
```
对齐 `makeImageThumbnail`(`MediaVisualCache.swift:152-163`,`kCGImageSourceThumbnailMaxPixelSize:120`、应用 EXIF transform)。Rust 用 `image` crate 解码 + `kamadak-exif`/`image` 的方向处理 + 等比缩放到长边 ≤ 120。

### 3.3 sprite 网格磁盘缓存(逐字节复刻)

上游把缩略图序列拼成**一张 JPEG sprite + JSON sidecar**;sidecar 最后写、作为「完整条目」标记(`MediaVisualCache.swift:236-293`)。

```rust
// thumbnail/sprite.rs
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ThumbnailCacheMeta {
    pub tile_width: u32, pub tile_height: u32, pub columns: u32, pub times: Vec<f64>,
}

/// 缓存目录:<cache_root>/MediaVisualCache/<key>.thumbs.jpg + <key>.thumbs.json
pub fn load_sprite(cache_root: &Path, key: &str) -> Option<Vec<VideoThumb>>;
pub fn save_sprite(cache_root: &Path, key: &str, thumbs: &[VideoThumb]) -> Result<()>;
```
逐项对齐(`MediaVisualCache.swift`):
- **布局**:`columns = min(50, count)`,`rows = ceil(count/columns)`(`:268-269`)。tile 尺寸 = 首帧像素宽高(`:266-267`)。
- **坐标系**:CGContext 原点左下,行 0 在顶部 → `y = (rows-1-row)*tileH`(`:277-279`)。Rust 用 `image` crate(原点左上),则**直接** `y = row*tileH`(无需翻转,因为 `image` 已是左上原点);但裁剪/写入要与读取一致——**自成闭环即可**(写时按行优先左上,读时按同样规则裁剪),不必复刻 CG 的翻转;只需保证 `times` 顺序与 tile 顺序一致。
- **JPEG 质量**:0.75(`:286`,`kCGImageDestinationLossyCompressionQuality:0.75`)。
- **读校验**:`sprite.width ≥ tileW*min(columns,count)` 且 `sprite.height ≥ tileH*rows`,否则视为无效返回 None(`:249-250`)。逐 tile `cropping`(`:253-260`)。
- **原子写**:sidecar JSON 最后写;读时以「JSON 可解码 + sprite 可解码 + 尺寸校验通过」为完整标记(`:238-247`)。

> 兼容性目标:OpenTake 写出的 `.thumbs.jpg/.json` 与上游可互读(同 key、同 meta 字段名 `tileWidth/tileHeight/columns/times`)。`ThumbnailCacheMeta` 用 `#[serde(rename_all="camelCase")]`。

### 3.4 缩略图并发闸门

上游用 `AsyncSemaphore`:波形 gate=2、图片 gate=4(`MediaVisualCache.swift:16/27`),视频缩略图无显式 gate 但 `Task.detached(.userInitiated)`。Rust 用 `tokio::sync::Semaphore`,值照搬;调度集中在 §7.7 / §3.5 的服务层。

### 3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表)

上游 `MediaVisualCache` 持三张内存表(`waveformSamples`/`videoThumbnails`/`imageThumbnails`)+ in-flight 去重 + 触发重绘。在 OpenTake,**内存缓存与「触发重绘」属于上层**(Rust core / 前端);本 crate 只提供**纯生成 + 磁盘缓存**函数(上面 §3.1–3.4 + §4)。内存表 + in-flight 去重 + 进度回调放 `opentake-core`(或 render 的预览侧),用 Tauri event 推前端。理由:`opentake-media` 保持「无 UI 状态」的可测纯度;`needsDisplay` 是 AppKit 概念,跨平台由前端订阅事件实现。

---

## 4. Symphonia 波形(RMS 降采样,归一化 0..1,缓存格式)

对应 `MediaVisualCache` 波形分支(上游外包给 `DSWaveformImage.WaveformAnalyzer`,`MediaVisualCache.swift:181`)。`docs/_analysis/02` 表 L77:「Symphonia 解 PCM + 自算 RMS/peak 降采样」,成熟、低风险。

### 4.1 接口

```rust
// waveform/mod.rs
/// 归一化样本:0 = 响,1 = 静(对齐上游注释 "normalized 0=loud, 1=silence",
/// MediaVisualCache.swift:11)。长度 = sample_count(duration)。
pub fn waveform(path: &Path, duration_secs: f64) -> Result<Vec<f32>>;

/// 带磁盘缓存:命中 <cache_root>/MediaVisualCache/<key>.waveform 直接返回。
pub fn waveform_cached(cache_root: &Path, path: &Path, duration_secs: f64) -> Result<Vec<f32>>;
```

### 4.2 样本数量公式(逐字照搬)

`waveformSampleCount`(`MediaVisualCache.swift:186-190`):
```text
duration 非有限或 ≤ 0          -> 4000
duration ≥ 20000/150 (≈133.3s) -> 20000(硬上限)
否则                           -> max(4000, floor(duration * 150))
```
即每秒 150 个桶,下限 4000,上限 20000。Rust 复刻为 `pub fn waveform_sample_count(duration: f64) -> usize`,纯函数 + 单测边界(0、1s、100s、133.3s、1000s)。

### 4.3 降采样与归一化(RMS,对齐「0=响,1=静」)

DSWaveformImage 默认输出的是「振幅包络」,上游语义是 **0=loud,1=silence**(注意是反的)。复刻策略:
1. Symphonia 解码整轨为 f32 mono(多声道下混为均值)。
2. 把样本切成 `count` 个等长桶,每桶算 **RMS**(`sqrt(mean(x²))`)→ `amp ∈ [0,1]`(已是归一化幅度;若源 >1 截断)。
3. 归一化到上游语义:`out = 1 - clamp(amp_normalized, 0, 1)`,其中 `amp_normalized` 按整轨峰值或固定满刻度归一。**关键风险**:DSWaveformImage 的精确归一化方式未在上游代码内(是第三方),无法逐位复刻。

> **决策**:波形仅用于 UI 绘制(时间线音轨直观),**非帧级编辑量**,不要求与上游逐位一致(对齐 `docs/_analysis/02` 风险登记:波形列为 🟢 低)。规格采用「RMS → 满刻度归一 → `1 - x`」,在单测中断言:全静音→全 1±ε、满幅正弦→接近 0、单调性正确。缓存格式与文件名严格复刻(可互读),但样本值容许与上游有视觉等价差异。若后续要求逐位一致,再换 peak 包络并标定缩放(留 `WaveformMode { Rms, Peak }` 扩展位)。

### 4.4 缓存格式(逐字节复刻)

`.waveform` 文件 = 裸 `[f32]` little-endian(`MediaVisualCache.swift:218-227`):写 `samples.withUnsafeBytes`,读校验 `!data.isEmpty && data.count % 4 == 0` 后 `bindMemory(to: Float)`。

```rust
// waveform/store.rs
pub fn load_waveform(cache_root: &Path, key: &str) -> Option<Vec<f32>>; // 读 <key>.waveform
pub fn save_waveform(cache_root: &Path, key: &str, samples: &[f32]) -> Result<()>;
```
- 文件名:`<key>.waveform`(key = `file_identity_key(path, 32)`)。
- 格式:小端 f32 连续;`byteorder` 写、读校验 `len%4==0 && len>0`。
- ⚠️ 字节序:上游 `Data($0)` 是宿主端序(macOS arm64 = LE);跨平台固定 LE,与 arm64 mac 写出的互读一致。

---

## 5. candle/ort + SigLIP2 视觉/口语搜索

> 「口语搜索」= 转写关键词检索(§6.4,`TranscriptSearch`)。本节聚焦**视觉语义搜索**(SigLIP2 双编码器),完整复刻 `Search/` 子树。模型:`siglip2-base-patch16-256`,dim=768,imageSize=256,contextLength=64(`SearchIndexConfig.swift:22-45`)。

### 5.1 双编码器 trait + Spec

```rust
// search/embedder.rs
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbedderSpec {       // = VisualEmbedder.Spec(VisualEmbedder.swift:7)
    pub model: String, pub version: i32,
    pub embedding_dim: usize,   // 768
    pub image_size: u32,        // 256
    pub context_length: usize,  // 64
    #[serde(default)] pub normalized: bool, // 模型图内是否已 L2 归一化(见 §0.8)
}

pub trait Embedder: Send + Sync {
    fn spec(&self) -> &EmbedderSpec;
    fn encode_image(&self, frame: &RgbaFrame) -> Result<Vec<f32>>; // 长度 = embedding_dim
    fn encode_text(&self, text: &str) -> Result<Vec<f32>>;
}
```
对齐 `VisualEmbedder.encode(image:)`/`encode(text:)`(`VisualEmbedder.swift:37-51`)与输出提取 `vector(from:dim:)`(`:53-61`,断言 `count == dim`,否则 `BadModelOutput`)。

### 5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻

`VisualEmbedder.pixelBuffer`(`VisualEmbedder.swift:63-87`)+ `MODULE-PORT-MAP` L858:
1. 目标 256×256(`image_size²`)。
2. **黑底**:先用黑(`gray:0, alpha:1`)填满整张——因为缓冲内存复用未清零,带 alpha 源必须在黑底上混合(`VisualEmbedder.swift:81-83`)。
3. **squash-resize**:直接拉伸到正方形,**不裁剪、不保宽高比**(`:84-85`,注释 "SigLIP preprocessing squash-resizes to a square (no aspect crop)")。
4. 像素布局上游是 BGRA premultipliedFirst + byteOrder32Little + sRGB;Rust 喂模型用 RGB f32 张量(NCHW),按 SigLIP 预处理的 mean/std 归一(来自模型预处理配置,通常 `[0.5,0.5,0.5]`/`[0.5,0.5,0.5]`,实施时以导出模型附带的 `preprocessor_config.json` 为准)。

```rust
// 预处理:RgbaFrame -> ndarray::Array4<f32> (1,3,256,256)
fn preprocess_image(frame: &RgbaFrame, size: u32, mean: [f32;3], std: [f32;3]) -> ndarray::Array4<f32>;
```
- squash-resize 用 `image::imageops::resize(.., Triangle)` 到精确 `size×size`(忽略宽高比),对齐 swscale squash(`docs/_analysis/02` L75 / MODULE-PORT-MAP L858「FFmpeg/swscale 直接 scale 到 256x256,忽略宽高比」)。
- 带 alpha 源:先 over 黑底合成再丢 alpha。

### 5.3 文本 tokenize(SigLIP,定长 64,右填 0)

`TextTokenizer`(`TextTokenizer.swift:4-24`):`AutoTokenizer.from(modelFolder:)`,`encode` → 截断到 `contextLength` → 右填 `padToken=0` 到定长(无 attention mask,匹配 Python 参考)。

```rust
// search/tokenizer.rs
pub struct SiglipTokenizer { inner: tokenizers::Tokenizer, context_length: usize }
impl SiglipTokenizer {
    pub fn from_folder(folder: &Path, context_length: usize) -> Result<Self>; // 读 tokenizer.json
    /// 截断到 context_length,右填 0 到定长 context_length。返回 i64(ort)/i32 视后端。
    pub fn tokenize(&self, text: &str) -> Vec<i64>;
}
```
- HF `tokenizers` crate 与 swift-transformers 同源(`docs/_analysis/02` 表 L82 / MODULE-PORT-MAP L881「tokenizers crate(HF Rust 原生,与 swift-transformers 同源)」)。
- 关闭 padding/truncation 的自动行为,手动 `prefix(64)` + 右填 0,逐字对齐 `TextTokenizer.swift:18-22`。

### 5.4 视觉去重抽帧 `FrameSampler`(luma 8×8 + 镜头边界 + 覆盖下限)

`Search/Indexing/FrameSampler.swift` + `MODULE-PORT-MAP` L854。**纯算法 + ffmpeg 解帧**。

```rust
// search/frame_sampler.rs
pub const SAMPLER_VERSION: i32 = 1; // FrameSampler.swift:8

pub struct SamplerOptions {        // FrameSampler.Options(:10-16)
    pub candidate_interval: f64,   // 2.0
    pub coverage_floor: f64,       // 8.0
    pub promote_diff: f32,         // 12.0
    pub max_size: (u32, u32),      // (512,512)
    pub high_res_edge: u32,        // 3000
}
impl Default for SamplerOptions { /* 上游默认值 */ }

pub struct SampledFrame { pub time_secs: f64, pub image: RgbaFrame, pub is_new_shot: bool }

/// 流式产出视觉上不同的帧。FFmpeg 解帧替代 AVAssetImageGenerator。
pub fn sample_frames(path: &Path, duration_secs: f64, opts: &SamplerOptions)
    -> Result<impl Iterator<Item = Result<SampledFrame>>>;
```
算法(逐步对齐 `FrameSampler.sample`,`:40-90`):
1. 取首条视频流;若 `max(|w|,|h|) ≥ high_res_edge(3000)` 则 `interval *= 2`(2.0→4.0)(`:48-52`)。
2. 候选时间:`stride(from: interval/2, to: duration, by: interval)`(严格 `< duration`);为空则 `[duration/2]`(`:62-64`)。
3. 解帧:`max_size=512²`、`apply_rotation=true`、tolerance `max(interval/2, 1.0)`(`:54-60`)。
4. 每成功帧:`t = actual_secs`;丢 `t ≤ lastTime`(去重,`:74`);算 8×8 luma grid;有上一 grid → `is_new_shot = meanDiff > promote_diff(12)`,否则首帧 `is_new_shot=true`(`:78-84`);更新 `lastGrid`(**用所有解码帧更新**)。
5. 保留:`is_new_shot || t - lastKeptTime ≥ coverage_floor(8.0)`;满足则 `lastKeptTime=t` 并产出(`:86-88`)。注意 **luma 用所有帧更新,但 lastKeptTime 只在被保留时推进**(`MODULE-PORT-MAP` L854 末句)。

```rust
// LumaGrid(FrameSampler.swift:94-117)
pub const LUMA_CELLS: usize = 8;
/// 8×8 下采样,每格 Rec.601 luma = 0.299R + 0.587G + 0.114B(对 sRGB 字节)。
pub fn luma_grid(frame: &RgbaFrame) -> [f32; 64];
pub fn luma_mean_diff(a: &[f32;64], b: &[f32;64]) -> f32; // L1 平均差
```
- 系数 `.299/.587/.114` 逐字照搬(`:108`);8×8 下采样用高质量插值(`interpolationQuality=.high`,`:105`)。`meanDiff` = `Σ|a-b| / 64`(`:112-116`)。

### 5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等)

`Search/Indexing/VisualIndexer.swift` + `MODULE-PORT-MAP` L856。

```rust
// search/indexer.rs
pub fn needs_index(path: &Path, spec: &EmbedderSpec) -> bool;

pub fn index_video(path: &Path, duration_secs: f64, embedder: &dyn Embedder,
    opts: &SamplerOptions, on_progress: Option<&dyn Fn(f64)>,
    cancel: &CancelToken) -> Result<()>;

pub fn index_image(path: &Path, embedder: &dyn Embedder, cancel: &CancelToken) -> Result<()>;
```
视频累积算法(`VisualIndexer.index`,`:15-51` + `MODULE-PORT-MAP` L856):
- 维护 `shot_starts: Vec<f64>`。每遇 `is_new_shot`:`push(if empty {0.0} else {frame.time})` —— **第一个镜头起点强制 0**(无论首帧实际时间)(`:34-36`)。
- 每帧:`vectors += encode_image(frame)`;`times.push(t)`;`shot_indices.push(shot_starts.len()-1)`(`:37-39`)。
- Row:`shotStart = shot_starts[shot]`;`shotEnd = if shot+1 < len {shot_starts[shot+1]} else {duration}`(`:43-49`)。
- 进度:`min(t/duration, 1)`(`:40`)。
- **导出让路 + 取消**:每帧前 `cancel.check()?` 与 `wait_while_export_active()`(`:32-33`,见 §7.7)。
图像(`indexImage`,`:54-67`):解码到 512 长边缩略图(`:69-77`)→ 单 embedding,`Row{time:0, shotStart:0, shotEnd:0}`(零长 shot)。
保存:构造 `Header{model,modelVersion,samplerVersion,dim,count}` 写 `EmbeddingStore`(`:79-86`)。

### 5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻

`Search/Indexing/EmbeddingStore.swift` + `MODULE-PORT-MAP` L861/L924。**精确格式,可与上游互读**。

```rust
// search/embed_store.rs
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Clone)]
pub struct Header { pub model: String, pub model_version: i32, pub sampler_version: i32,
                    pub dim: usize, pub count: usize }   // JSON 字段 camelCase
pub struct Row { pub time: f64, pub shot_start: f64, pub shot_end: f64 }
pub struct AssetIndex { pub header: Header, pub rows: Vec<Row>, pub vectors: Vec<f32> } // count*dim, f32

pub const MAGIC: &[u8;8] = b"PALMEMB1";

pub fn key(path: &Path) -> Option<String>;             // file_identity_key(path, 32)
pub fn header(cache_root: &Path, key: &str) -> Option<Header>;
pub fn is_current(cache_root: &Path, key: &str, model: &str, mv: i32, sv: i32) -> bool;
pub fn load(cache_root: &Path, key: &str) -> Result<AssetIndex>;
pub fn save(cache_root: &Path, key: &str, header: &Header, rows: &[Row], vectors: &[f32]) -> Result<()>;
pub fn clear_all(cache_root: &Path) -> Result<()>;
```
布局(`EmbeddingStore.swift:30/63-115` + `MODULE-PORT-MAP` L861):
```
magic "PALMEMB1" (8 bytes ASCII)
u32 headerLen     (4 bytes, little-endian)
JSON(Header)      (headerLen bytes)
count 行,每行 rowBytes = 3*8 + dim*2 = 24 + dim*2:
    f64 time      (8 bytes LE)
    f64 shotStart (8 bytes LE)
    f64 shotEnd   (8 bytes LE)
    dim × f16     (每个 2 bytes LE)        # half crate f16→f32 读 / f32→f16 写
```
- `dim=768` ⇒ `rowBytes = 24 + 1536 = 1560`(`MODULE-PORT-MAP` L861)。
- **严格校验**:`total == 8 + 4 + headerLen + count*rowBytes`,否则 `StoreCorrupt`(`EmbeddingStore.swift:69/74`)。
- 全部 **little-endian、无对齐**(`loadUnaligned`,`:79-92`);用 `byteorder` LE。
- 写 **atomic**(`:114`,先写临时再 rename)。
- 文件:`<cache_root>/Embeddings/<key>.embed`(`:32-46`)。
- `is_current`:`header.model==model && model_version==mv && sampler_version==sv`(`:58-61`);任一不符即需重索引。
- 内存向量 f32 连续(供 §5.8 矩阵·向量),落盘 f16(`AssetIndex` 注释 `:24`)。

### 5.7 推理后端实现(ort 默认 / candle 备选)

`VisualEmbedder` 用 CoreML(`VisualEmbedder.swift:1/29-35`),跨平台不可移植 → `docs/_analysis/02` 表 L80 / MODULE-PORT-MAP L881:**ort(ONNX Runtime)或 candle**。

```rust
// search/embedder.rs(默认实现)
#[cfg(feature = "ort-backend")]
pub struct OrtEmbedder { image: ort::Session, text: ort::Session,
                         tok: SiglipTokenizer, spec: EmbedderSpec }
#[cfg(feature = "ort-backend")]
impl Embedder for OrtEmbedder { /* encode_image/encode_text */ }
```
- **输入/输出名**:上游 CoreML 用 `"image"`/`"tokens"` 输入、`"embedding"` 输出(`VisualEmbedder.swift:39/48/54`)。ONNX 导出可能用不同名(如 `pixel_values`/`input_ids`/`image_embeds`),实施时按导出图实际名绑定,并在 `EmbedderSpec` 外补 `io_names` 配置或硬编码到 `OrtEmbedder::new`。
- **图像输入**:NCHW f32(1,3,256,256),mean/std 见 §5.2。
- **文本输入**:int64 (1,64),右填 0(§5.3)。
- **输出**:f32 (1,768);断言 `len==dim`(对齐 `vector(from:)` 的 `count==dim` 断言)。
- **L2 归一化**:若 `spec.normalized==false`(上游默认,模型内已归一)则**不**额外归一;否则 `v /= ‖v‖₂`。务必与导出模型一致(§0.8 风险)。
- **候选后端 candle**(`candle-backend` feature):`candle-transformers` 有 SigLIP 实现可加载 safetensors,纯 Rust 无 C++ 依赖;作为 ort 不可用平台的回退。两后端必须产出**同一向量**(同权重/同预处理),用「同图同文 → 余弦 > 0.999」单测交叉验证。

### 5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数

`Search/Query/VisualSearch.swift`(`cblas_sgemv`)+ `MODULE-PORT-MAP` L863。上游用 Accelerate BLAS;Rust 用 `ndarray`(`gemv`)或手写点积。

```rust
// search/ranker.rs
#[derive(Clone, PartialEq, Debug)]
pub struct Hit { pub asset_id: String, pub time: f64,
                 pub shot_start: f64, pub shot_end: f64, pub score: f32 }

pub fn search(query: &[f32], indexes: &[(String, AssetIndex)],
    limit: usize,            // 20
    relative_cutoff: f32,    // 0.85
    min_score: Option<f32>,  // visualMatchCosineFloor 0.05
) -> Vec<Hit>;
```
算法(逐步,`VisualSearch.search` `:16-56` + `MODULE-PORT-MAP` L863):
1. 每个 `(asset_id, index)`:若 `dim != query.len() || count==0` 跳过(`:24`)。
2. `scores = vectors(count×dim, row-major) · query`(`cblas_sgemv(RowMajor,NoTrans,M=count,N=dim,α=1,A=vectors,lda=dim,x=query,β=0,y=scores)`,`:27-32`)。Rust:`Array2::from_shape(vectors).dot(&query_vec)`。
3. **best-per-shot**:按 `row.shotStart` 分组,只留最高分帧;**同分保留先出现**(`existing.score >= score` 则跳过,`:34-39`)。Rust 用 `HashMap<OrderedFloat<f64>, (usize, f32)>`(或对 `shot_start` 量化为 bits key)。
4. 每 shot 最佳 → `Hit`(`:40-47`);全局 `sort_by score desc`(`:49`)。
5. 若 `min_score`:先 `filter(score >= min_score)`(`:50-52`)。
6. `top = hits[0].score`;`top <= 0` → 返回空(`:53`)。
7. `floor = top * relative_cutoff`;返回 **`hits.prefix(limit)` 再 `filter(score >= floor)`**(顺序关键:先截 limit 再过 floor,最终 ≤ limit,`:54-55`)。

> 纯函数 + 全单测:多素材、单镜头去重、minScore 过滤、relativeCutoff 顺序、空结果、dim 不匹配跳过。

### 5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip)

`Search/Models/ModelDownloader.swift` + `MODULE-PORT-MAP` L927。需求降低:**ONNX/safetensors 无需编译步骤**(去掉 `MLModel.compileModel`)。

```rust
// search/model_download.rs
pub struct ManifestFile { pub name: String, pub sha256: String, pub bytes: i64 }
pub struct Manifest {
    pub model: String, pub version: i32,
    pub embedding_dim: usize, pub image_size: u32, pub context_length: usize,
    pub image_encoder: ManifestFile, pub text_encoder: ManifestFile, pub tokenizer: ManifestFile,
}
pub struct InstalledModel { pub image_encoder: PathBuf, pub text_encoder: PathBuf,
                            pub tokenizer_folder: PathBuf, pub spec: EmbedderSpec }

pub fn installed(models_dir: &Path, m: &Manifest) -> Option<InstalledModel>;
pub async fn install(models_dir: &Path, m: &Manifest, base_url: &str,
    on_progress: impl Fn(f64)) -> Result<InstalledModel>;
pub fn verify_sha256(path: &Path, expected: &str) -> Result<()>;
```
- **安装目录**:`<app_support>/OpenTake/Models/<model>-v<version>/{image_encoder.onnx, text_encoder.onnx, tokenizer/, spec.json}`(对齐 `ModelDownloader.swift:46-64`,把 `.mlmodelc` 换成 `.onnx`/`.safetensors`)。跨平台用 `dirs`/Tauri `app_data_dir`(不再硬编码 `~/Library/Application Support`)。
- **进度**:三文件按 bytes 加权 0..1(`:79-99`)。
- **校验**:流式 SHA256(1 MiB 分块)对比 manifest(`:146-155`),不符 `Checksum`。
- **解压**:`zip` crate 替 `/usr/bin/ditto`;每 zip 恰好一个顶层条目(`:157-172`)。
- **幂等**:已安装直接返回(`:72`);全部 staged 后原子 move 到安装目录(`:101-113`)。
- **idempotent install + 安装完整性**:三文件都存在且 tokenizer 含 `tokenizer.json` 才算已装(`:54-64`)。

> ⚠️ **模型来源调整**:上游托管 `huggingface.co/palmier-io/siglip2-base-coreml`(CoreML zip,`SearchIndexConfig.swift:6`)。OpenTake 需**自托管或指向 ONNX/safetensors 版**的 SigLIP2-base-patch16-256(转换或复用现成 ONNX 导出)。`Manifest` 的 sha256/bytes 重新计算;`config.rs` 的 `manifest` 常量替换为 ONNX 版三文件。**这是本节唯一需要外部资产准备的点**(模型转换/托管),记入 §8 实施清单 T8.0。

### 5.10 配置 `SearchIndexConfig` 等价

`Search/SearchIndexConfig.swift`:
```rust
// search/config.rs
pub const VISUAL_MATCH_COSINE_FLOOR: f32 = 0.05;   // :4
pub const RELATIVE_CUTOFF: f32 = 0.85;             // VisualSearch.swift:19 默认
pub const SEARCH_LIMIT: usize = 20;                // 多处默认
pub fn enabled() -> bool;  // 默认 true;Tauri Store/settings.json 持久化(替 UserDefaults)
pub fn manifest() -> Manifest; // siglip2-base-patch16-256, v1, dim768, size256, ctx64(ONNX 版)
pub fn base_url() -> String;   // 自托管;DEBUG 可被环境变量覆盖(替 UserDefaults override)
```
- `enabled` 默认 true,键语义照搬(`SearchIndexConfig.swift:8-11`);存储后端换 Tauri Store/`settings.json`(`MODULE-PORT-MAP` L940 (1))。
- `base_url` DEBUG 覆盖用环境变量替 `UserDefaults "searchIndexModelBaseURL"`(`:13-20`)。

---

## 6. whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型)

对应 `Transcription/{Transcription,TranscriptCache,TranscriptSearch}.swift`。上游用 macOS 26 `SpeechAnalyzer`/`SpeechTranscriber`;跨平台换 **whisper-rs**(`docs/_analysis/02` 表 L79、MODULE-PORT-MAP L1211)。**上层算法(数据模型 / offsetting / 缓存 / filter / 关键词搜索 / locale 匹配)逐行复刻,只换 ASR 后端。**

### 6.1 数据模型(逐行 1:1 port)

`Transcription.swift:5-39`:
```rust
// transcribe/mod.rs
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TranscriptionWord { pub text: String, pub start: Option<f64>, pub end: Option<f64> }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TranscriptionSegment { pub text: String, pub start: f64, pub end: f64 }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: Option<String>,
    pub words: Vec<TranscriptionWord>,
    pub segments: Vec<TranscriptionSegment>,
}
impl TranscriptionResult {
    /// 把时间码移回源时间(转写截取区间后)。offset==0 原样返回。
    pub fn offsetting(&self, offset: f64) -> TranscriptionResult; // Transcription.swift:26-38
}
```
- `offsetting`:`offset==0` 直接 clone;否则 words 的 `start/end` 各 `+offset`(None 保持 None),segments 的 `start/end` `+offset`(`:26-38`)。
- JSON 字段保持同名(`text/language/words/segments/start/end`),与上游 `TranscriptCache` 磁盘 JSON 互读。

### 6.2 转写后端 trait + whisper 实现

```rust
// transcribe/mod.rs
pub struct TranscribeOptions {
    pub censor_profanity: bool,        // 上游 etiquetteReplacements(:121);whisper 无内建 → 见下
    pub preferred_language: Option<String>, // bcp47/iso639;whisper language 参数
    pub source_range: Option<(f64, f64)>,   // 绝对秒;先抽 PCM 再 offsetting
}
pub trait Transcriber: Send + Sync {
    fn transcribe_pcm(&self, pcm: &PcmBuffer, opts: &TranscribeOptions) -> Result<TranscriptionResult>;
}

// transcribe/whisper.rs
pub struct WhisperTranscriber { ctx: whisper_rs::WhisperContext /* 模型路径 */ }
impl Transcriber for WhisperTranscriber { /* full params + token timestamps */ }
```
顶层便捷函数(对齐 `Transcription.transcribe*`,`:65-200`):
```rust
pub fn transcribe_file(path: &Path, t: &dyn Transcriber, opts: &TranscribeOptions) -> Result<TranscriptionResult>;
pub fn transcribe_video_audio(path: &Path, t: &dyn Transcriber, opts: &TranscribeOptions) -> Result<TranscriptionResult>;
```
实现要点:
- **音频抽取**:`extract_pcm(path, &PcmSpec{16000,1,F32}, opts.source_range)`(§2.3);whisper 吃 16k mono f32。对齐 `extractAudioTrack`(`Transcription.swift:203-280`)。
- **range → offsetting**:有 `source_range` 时,抽截取段 → 转写 → `result.offsetting(lower)`(`Transcription.swift:65-70/92-98`)。
- **word/segment 时间戳**:whisper-rs 开启 `set_token_timestamps(true)`;遍历 segments 取 `start_t/end_t`(厘秒→秒);遍历 tokens 取 token 级时间组装 `TranscriptionWord`(过滤特殊 token / 空白)。对齐 `decodeResults`(`:284-322`):每个 segment = 一条 `TranscriptionSegment`(text trim + start/end),每个非空 token run = 一条 `TranscriptionWord`(trim + 可空 start/end)。`fullText` = 所有 segment 文本拼接后 trim(`:317`)。
- **language**:whisper 自动检测或用 `preferred_language`;回填 `result.language`(对齐 `locale.identifier(.bcp47)`,`:320`)。
- **profanity**:whisper 无 `etiquetteReplacements`;`censor_profanity=true` 时用**可选脏词表后处理**(MODULE-PORT-MAP L1211 (3)),默认关闭。
- **不要求逐 token 对齐 Apple**:下游字幕「词中点落区间计数」「短语重叠归属」只需秒值精度(MODULE-PORT-MAP L1211 (3) 末句)。

> ⚠️ whisper 词级时间戳精度低于 Apple `audioTimeRange`;`TranscriptionWord.start/end` 为 `Option` 已容许缺失。模型权重(ggml/gguf,如 `base`/`small` 多语种)由 §5.9 同款下载器或单独 catalog 管理(记入 T8.0)。

### 6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter)

`Transcription/TranscriptCache.swift`。上游是 actor;Rust 用 `tokio::sync::Mutex` 包内存表 + 同步磁盘读路径。

```rust
// transcribe/cache.rs
pub struct TranscriptCache { /* memory: HashMap<String, TranscriptionResult>(max 4) */ }
impl TranscriptCache {
    pub async fn transcript(&self, path: &Path, is_video: bool, range: Option<(f64,f64)>,
        t: &dyn Transcriber) -> Result<TranscriptionResult>;

    /// 半开重叠过滤(段 + 词),段文本以空格 join 为整体 text。
    pub fn filter(r: &TranscriptionResult, range: (f64,f64)) -> TranscriptionResult; // :29-39

    pub fn has_cached_on_disk(cache_root: &Path, path: &Path) -> bool;   // :70-73
    pub fn cached_on_disk(cache_root: &Path, path: &Path) -> Option<TranscriptionResult>; // :76-80
}
```
逐项(`TranscriptCache.swift`):
- **只缓存全量转写**;窗口请求通过 `filter` 全量缓存得到(`:4-5/12-27`)。命中(内存或磁盘)且有 range → 返回 `filter(full, range)`;否则若有 range 直接转写该 range(不缓存,`:17-21`);否则全量转写并缓存(`:22-26`)。
- **filter**:段 `end > lower && start < upper`;词需 `start/end` 非空且 `end > lower && start < upper`;`text = segments.map(text).join(" ")`(`:29-39`)。
- **内存 LRU**:`max=4`,满则 `removeAll()`(粗暴清空,逐字照搬 `:57-60`)。
- **磁盘**:`<cache_root>/Transcripts/<key>.json`,key=`file_identity_key(path,32)`(`:62-88`)。`cached_on_disk`/`has_cached_on_disk` 同步读(供索引调度器判断,§7.7)。

### 6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数)

`Transcription/TranscriptSearch.swift` + `MODULE-PORT-MAP` L684/L1211 (1)。

```rust
// transcribe/search.rs
#[derive(Clone, PartialEq, Debug)]
pub struct SpokenHit { pub asset_id: String, pub start: f64, pub end: f64, pub text: String }

/// 在磁盘缓存转写中,对每素材的 segments 做「所有词项 AND 子串、大小写/变音不敏感」命中。
pub fn search(cache_root: &Path, query: &str, assets: &[(String, PathBuf)], limit: usize) -> Vec<SpokenHit>;
pub fn terms(query: &str) -> Vec<String>;          // 分词 + 去边缘标点(:27-32)
pub fn matches(text: &str, terms: &[String]) -> bool; // 全词 AND 子串(:34-36)
```
- `terms`:按空白分词,每词去**边缘**标点(`"budget," → "budget"`),去空(`:27-32`)。
- `matches`:`terms.all(|t| text.contains_ci_diacritic_insensitive(t))`(`:34-36`)。**变音不敏感**用 `unicode-normalization` NFD 去组合标记 + 大小写折叠后 `contains`(MODULE-PORT-MAP L1211 (1))。
- 遍历素材:读 `cached_on_disk`,对每条 `segment` 命中即收 `SpokenHit{asset_id,start,end,text}`,达 `limit(20)` 立即返回(`:12-25`)。

### 6.5 locale 匹配(纯逻辑,照搬)

`Transcription.swift:72-90`(`supportedLocales`/`bestSupportedLocale`/`matchLocale`):
```rust
// transcribe/locale.rs
/// 语言码优先、地区次之。candidates 用 [preferred] 或 [系统语言…, 当前]。
pub fn match_locale(candidates: &[&str], supported: &[&str]) -> Option<String>; // :81-90
pub fn best_supported_locale(supported: &[&str]) -> Option<String>; // 系统首选 + current(:76-79)
```
- whisper 支持语言集近乎固定(不像 Apple 需运行时查询),`supported` 可来自 whisper 静态语言表;`match_locale` 逻辑(同语言码下选同地区否则首个)逐行照搬(`:82-89`)。用于把用户/系统语言映射到 whisper `language` 参数(MODULE-PORT-MAP L1211 (3))。

---

## 7. ort 推理 worker 通用接口(供进阶 AI 特性复用)

上游无此抽象(CoreML 直接在 `VisualEmbedder`)。`docs/ROADMAP.md` Phase 8 与 `docs/ADVANCED-FEATURES.md` B/C/D 层要求「统一 ort worker」承载:超分(Real-ESRGAN/SeedVR)、AI 抠像(RVM/BiRefNet)、运动追踪(CoTracker)、人声分离(Demucs)等。SigLIP2 的 `OrtEmbedder`(§5.7)是它的第一个使用者。

### 7.1 通用模型抽象

```rust
// ort_worker/mod.rs
/// 一个已加载的 ONNX 模型 + 其 IO 约定。线程安全,可被多任务共享。
pub struct OrtModel { session: ort::Session, io: IoSpec }
pub struct IoSpec { pub inputs: Vec<IoTensor>, pub outputs: Vec<IoTensor> }
pub struct IoTensor { pub name: String, pub dtype: TensorDType, pub shape: Vec<i64> } // -1=动态

impl OrtModel {
    pub fn load(path: &Path, ep: ExecutionProvider) -> Result<Self>;
    /// 多输入多输出推理;输入/输出按名映射 ndarray。
    pub fn run(&self, inputs: &[(&str, TensorRef<'_>)]) -> Result<HashMap<String, OwnedTensor>>;
}

pub enum ExecutionProvider { Cpu, CoreML, Cuda, DirectMl, Tensorrt } // 按平台可用性回退到 Cpu
```

### 7.2 worker(序列化 GPU/重负载,导出期让路)

```rust
/// 单后台执行器:把推理任务排队,序列化访问昂贵 EP,导出活跃时暂停(与 §7.7 共享暂停信号)。
pub struct OrtWorker { /* tokio mpsc + 单 worker */ }
impl OrtWorker {
    pub fn spawn(export_pause: ExportPause) -> Self;
    pub async fn submit<F, T>(&self, job: F) -> Result<T>
        where F: FnOnce(&OrtModelRegistry) -> Result<T> + Send + 'static, T: Send + 'static;
}
pub struct OrtModelRegistry { /* 按 key 懒加载并缓存 OrtModel,避免重复 load */ }
```
- **张量辅助**(`ort_worker/tensor.rs`):`ndarray ↔ ort::Value`、NCHW/NHWC 转换、mean/std 归一、`Array4<f32>` ↔ 图像。SigLIP 预处理(§5.2)即复用这里。
- **EP 回退**:首选平台 EP(CoreML/CUDA/DirectML),不可用回退 CPU,日志 `tracing::warn`。
- **复用点**:`OrtEmbedder`(§5.7)内部即一个 `OrtModel`(image)+ 一个 `OrtModel`(text);进阶特性各自定义自己的预处理/后处理,共用 `OrtModel::run` + `OrtWorker` 调度。

> 本 crate 只交付**框架 + SigLIP2 使用者**;具体进阶模型(Real-ESRGAN 等)在各自 Phase 8+ PR 落地,复用本接口。记此以明确「worker 通用接口」的交付边界 = §7.1/§7.2 + 至少一个真实使用者(SigLIP2)。

### 7.3 后台索引/转写调度 `IndexCoordinator`(替 `SearchIndexCoordinator`)

`Search/SearchIndexCoordinator.swift` + `MODULE-PORT-MAP` L864/L867。上游是 `@MainActor @Observable`;Rust 用 **tokio 单 worker 队列 + AtomicUsize 导出暂停 + 事件向前端推进度**(MODULE-PORT-MAP L881 (6))。**注意:UI 状态(进度/`@Observable`)属上层**;本 crate 提供调度内核 + 进度回调,UI 镜像在 `opentake-core`/前端。

```rust
// index_coordinator.rs
#[derive(Clone)] pub struct ExportPause(Arc<AtomicUsize>); // 引用计数,跨窗口
impl ExportPause {
    pub fn begin(&self); pub fn end(&self);      // exportDidBegin/End(:46-47)
    pub fn is_active(&self) -> bool;             // exportActive(:45)
    pub async fn wait_while_active(&self);       // 每 2s 轮询(:49-53)
}

pub struct IndexCoordinator { /* queue, failed set, single worker, loaded_indexes cache */ }
impl IndexCoordinator {
    pub fn new(export_pause: ExportPause, embedder: Arc<dyn Embedder>,
               transcriber: Arc<dyn Transcriber>, cache_root: PathBuf) -> Self;

    /// 入队需要(重)索引的素材(视觉 needsIndex 或 转写无磁盘缓存)。
    pub fn schedule(&self, asset: &opentake_domain::media::MediaAsset);
    pub fn sweep(&self, assets: &[opentake_domain::media::MediaAsset]);
    pub async fn cancel_all(&self);

    /// 查询:快照候选 → off-thread 加载/编码/排名 → 返回 Hit(视觉)。
    pub async fn search_visual(&self, query: &str, limit: usize,
        within: Option<&HashSet<String>>, assets: &[MediaAsset]) -> Vec<Hit>;

    pub fn progress(&self) -> IndexProgress; // {batch_total, batch_completed, current_fraction}
}
```
逐项对齐(`SearchIndexCoordinator.swift`):
- **schedule 条件**:enabled 且有 embedder 且 `!asset.is_generating`;id 不在 queue/failed;`needsVisual(video|image 且 VisualIndexer.needs_index)` 或 `needsTranscript(audio|video+hasAudio 且 转写无磁盘缓存)` 成立才入队;`batch_total+=1`;`ensure_worker`(`:107-124`)。
- **worker**:单个(`tokio::spawn`),`utility` 优先级;循环 dequeue,`export_pause.wait_while_active()` 每 2s 轮询(`:148-160`);`index_one`(`:178-221`)。
- **index_one**:需转写则视觉占进度 0.5 否则 1.0(`visualShare`,`:181-185`);`async let`/`tokio::join!` 并发跑转写(`TranscriptCache.transcript`)与视觉索引;视觉完成后置 `current_fraction = visualShare` 再 await 转写(`:189-214`)。失败(非取消)记 `failed`(`:217-220`)。
- **dequeue**:跳过已不存在的 id(`batch_completed+=1`);队列空 `reset_batch` 返回 None(`:162-170`)。
- **search**:main 快照候选 `(id,url)` + `loaded_indexes`;off-thread 算 key、命中内存缓存(key 相等)复用否则 `EmbeddingStore::load`、`encode_text(query)`、`VisualSearch::search`;回主合并 `loaded_indexes`;空 query → `[]`(`:225-257`)。
- **generation 让路**:`exportPause` 跨窗口引用计数(`ExportPauseCounter`,`:37-47`);导出开始/结束由 `opentake-render` 调 `begin/end`(对齐上游 `ExportService.isExporting.didSet`)。

---

## 8. 与 domain / render 的接口

### 8.1 消费 `opentake-domain`(不可改)

本 crate 依赖 `opentake-domain`,消费以下类型(均已存在,见 `crates/opentake-domain/src/`):
- `media::MediaAsset`(`media.rs:283`):`id/url:PathBuf/kind:ClipType/duration:f64/source_width/source_height/source_fps/has_audio/...`。本 crate 的 `probe()` 结果**回填**这些字段(由 `opentake-core` 调用,§8.4);索引/转写调度直接读 `MediaAsset`(`kind/url/has_audio/is_generating`)。
- `media::MediaResolver`(`media.rs:226`):`expected_path(asset_id)` 把 asset id 解析为 `PathBuf`(零 IO);本 crate 所有 IO 函数收 `&Path`,由调用层先经 resolver 解析。
- `clip_type::ClipType`(`clip_type.rs:783`):`Video/Audio/Image/Text/Lottie`,`is_visual()`/`from_file_extension()`。调度按 `kind` 路由(video→缩略图+波形+视觉索引+转写;audio→波形+转写;image→图片缩略图+视觉索引)。
- `timeline::{Timeline, Track}`、`clip::Clip`:仅 §8.3 的「物化纹理」需要读 clip 属性;本 crate 不直接消费 Timeline(渲染在 render)。

**单向依赖**:`opentake-domain` ← `opentake-media`;本 crate **不**反向暴露类型给 domain(domain 零 IO 叶子)。

### 8.2 被 `opentake-render` 复用的解码/编码

`opentake-render`(RenderPlan + wgpu 合成 + 双 ffmpeg 后端)**复用本 crate 的**:
- `decode::frame::{decode_frame_at, decode_frames_at}`(预览/导出取源帧 → 上传纹理)。
- `decode::reader`(顺序解帧迭代器,导出后端逐帧喂合成器)。
- `decode::pcm::extract_pcm`(导出混音前取各 clip 音频 PCM)。
- `encode::{VideoEncoder, ExportPreset}`(导出后端把合成 RGBA 帧序列 + 混音编码成容器)。
- `MediaProbe`(渲染尺寸/源 fps 决策)。

**职责切分**(`docs/ARCHITECTURE.md` §1/§6):
- `opentake-media` = **读取/编码 + 离线分析**(解码到 RGBA、抽 PCM、缩略图、波形、转写、语义索引/搜索、ort worker)。
- `opentake-render` = **合成 + 调度**(RenderPlan 纯函数、wgpu 逐帧合成、媒体物化为纹理、预览/导出后端、A/V 同步)。`renderSize` 偶数化、BT.709 instruction、关键帧 ramp **全在 render**。
- 二者通过 **`RgbaFrame` / `PcmBuffer`** 这两个朴素值类型交换帧/样本,无 wgpu/ffmpeg 类型泄漏到边界。

### 8.3 媒体物化(图片/Lottie → 纹理)的归属

上游用 `ImageVideoGenerator`(图片烧静止视频)、`LottieVideoGenerator`(Lottie 烧 ProRes)、`AlphaVideoNormalizer`(直 alpha 预乘)绕开 AVPlayer 限制。`docs/_analysis/02` 表 L74/L75/L81 与 `docs/ARCHITECTURE.md` §6 L130:**自建 wgpu 合成器后,这三类 hack 整类消失**——图片/Lottie 在合成前**物化为纹理**(content-hash 缓存),由 `opentake-render` 负责。
- 本 crate **提供**:图片解码 → `RgbaFrame`(§3.2 / `image` crate);(可选)Lottie 解码用 `rlottie` FFI 或 `velato`(`docs/_analysis/02` 表 L81),渲成 `RgbaFrame` 序列。**建议** Lottie 放 render 的物化层或独立 `opentake-motion`(Phase 10),本 crate 仅暴露图片解码;Lottie 列为**有意暂不归本 crate**。
- 本 crate **不提供**:静止视频烧制、ProRes 烧制、alpha 预乘转码(整类删除)。

### 8.4 facade `MediaEngine`(供 `opentake-core` 调用)

```rust
// lib.rs
pub struct MediaEngine {
    cache_root: PathBuf,          // 缩略图/波形/转写/embedding 缓存根(Tauri app_cache_dir)
    models_dir: PathBuf,          // 模型安装根(Tauri app_data_dir)
    coordinator: IndexCoordinator,
    transcript_cache: TranscriptCache,
    ort: OrtWorker,
}
impl MediaEngine {
    pub fn probe(&self, path: &Path) -> Result<MediaProbe>;
    pub fn video_thumbnails(&self, path: &Path, dur: f64, cb: Option<&dyn Fn(&[VideoThumb])>) -> Result<Vec<VideoThumb>>;
    pub fn image_thumbnail(&self, path: &Path) -> Result<RgbaFrame>;
    pub fn waveform(&self, path: &Path, dur: f64) -> Result<Vec<f32>>;
    pub async fn transcribe(&self, path: &Path, is_video: bool, range: Option<(f64,f64)>) -> Result<TranscriptionResult>;
    pub fn search_spoken(&self, query: &str, assets: &[(String, PathBuf)], limit: usize) -> Vec<SpokenHit>;
    pub async fn search_visual(&self, query: &str, limit: usize, assets: &[MediaAsset]) -> Vec<Hit>;
    pub fn index_sweep(&self, assets: &[MediaAsset]);
    pub fn export_pause(&self) -> ExportPause; // 交给 render 在导出期 begin/end
}
```
- 错误边界:`MediaEngine` 返回 `Result<_, MediaError>`;`opentake-core` 转 Tauri `Err(String)`(AGENTS.md Rust 风格)。
- 缓存根/模型根由 core 注入(跨平台路径,替上游硬编码的 `~/Library/...`)。

---

## 9. 跨平台与合规要点

- **路径**:全部用 `&Path`/`PathBuf`;缓存/模型根用 Tauri `app_cache_dir`/`app_data_dir`(替上游 `~/Library/Caches/PalmierPro`、`~/Library/Application Support/PalmierPro`)。缓存子目录名沿用 `MediaVisualCache`/`Embeddings`/`Transcripts`/`Models` 以便同机迁移可读(`MODULE-PORT-MAP` L923-927)。
- **字节序**:`.waveform`/`.embed` 固定 little-endian(arm64 mac 写出可互读)。
- **FFmpeg 许可**:GPL-3.0 项目兼容 FFmpeg (L)GPL(`docs/_analysis/02` 表末、`DECISIONS.md`);动态链接 + NOTICE 标注。
- **模型许可/托管**:SigLIP2 与 whisper 权重需自托管为 ONNX/gguf;在 NOTICE/README 标注来源(对齐 `DECISIONS.md` 合规栏)。**唯一网络请求**是模型权重一次性下载(非闭源云);转写/索引/查询全本地(对齐 `Search` 模块「无闭源云接触」,`MODULE-PORT-MAP` L879)。
- **遥测**:上游 `Log.*.notice(..., telemetry:)` 经 Sentry;OpenTake 改 `tracing` 本地结构化日志,内容仅计数/状态(MODULE-PORT-MAP L879/L1211)。
- **隐私**:转写/embedding 不外发;只有模型权重下载是出网。

---

## 10. 分步实施清单与验收

> 依赖:`opentake-domain`(M1,已完成 media/clip_type/timeline)。本 crate 分 **Phase 2 子集**(缩略图/波形/解码/探测,易)与 **Phase 8 子集**(转写/语义搜索/ort worker)。每步独立可测,覆盖率 ≥80%(AGENTS.md / common testing)。

### Phase 2 子集(基础媒体,先做)

- **T2.1 cache_key + error**:`file_identity_key`(SHA256 path|mtime|size,前 16 字节 32 hex)+ `MediaError`。验收:同输入稳定、不同 mtime/size 变 key、缺文件 None。
- **T2.2 probe**:ffmpeg-next 读时长/旋转校正宽高/fps/has_audio。验收:对一组样本(横屏/竖屏/旋转 90°/纯音频/无音轨视频)字段正确;旋转视频宽高已交换;与 `ffprobe` 交叉核对。
- **T2.3 decode_frame_at / decode_frames_at**:seek+tolerance+缩放(保宽高比)+ `t>lastTime` 去重。验收:指定秒取到最近帧、实际时间单调、越界 Err;批量去重生效。
- **T2.4 extract_pcm**:解音轨 → 16k mono f32,支持 range。验收:输出采样率/声道/长度正确;range `(a,b)` 长度≈`(b-a)*16000`;无音轨 `NoTrack`。
- **T2.5 thumbnail + sprite**:`videoThumbnailTimes` 公式、120×68、渐进回调、sprite 网格 + JSON sidecar(camelCase 字段)+ 原子写 + 读校验。验收:时间点序列与公式逐一相符;**写出的 `.thumbs.jpg/.json` 能被上游 `MediaVisualCache.loadThumbnails` 读回**(字段名/列数/tile 尺寸/times 一致);坏文件返回 None。
- **T2.6 waveform**:`waveform_sample_count` 公式逐字 + RMS 降采样 + 归一化「0=响,1=静」+ `.waveform` LE 缓存。验收:count 公式边界(0/1s/100s/133.3s/1000s)精确;全静音→≈1、满幅→≈0、单调;`.waveform` 字节 = `len*4`、可往返。
- **T2.7 encode + preset**:H.264/H.265/ProRes × 720/1080/4K 编码器 + 偶数尺寸(逻辑在 render,本 crate 接收偶数)+ BT.709。验收:导出 mp4 可播放、时长/分辨率正确、色彩 BT.709 tag;ProRes 带 LPCM。**画质逐预设逼近上游**留 §10 末持续校准(非阻塞)。

### Phase 8 子集(转写/语义搜索/AI worker)

- **T8.0 模型资产准备**(前置):SigLIP2-base-patch16-256 转/取 ONNX(image+text encoder)+ tokenizer.json;whisper 多语种 ggml/gguf;自托管 + 计算 sha256/bytes 填 `Manifest`。验收:`installed()` 能识别、`verify_sha256` 通过、`OrtEmbedder` warm-up `encode_text("warm up")` 成功(对齐 `VisualModelLoader.load` 的 warm-up,`VisualModelLoader.swift:99`)。
- **T8.1 transcribe 数据模型 + offsetting + locale**:`TranscriptionResult/Word/Segment` serde(字段名与上游互读)、`offsetting`、`match_locale`/`best_supported_locale`。验收:`offsetting(0)` 恒等、`offsetting(k)` 全时码 +k、None 保持;locale 同语言选同地区否则首个(对拍 `Transcription.matchLocale` 用例)。
- **T8.2 whisper 后端**:`extract_pcm` → whisper-rs(token timestamps)→ segments/words/text/language;range→offsetting。验收:已知短音频转写文本合理、segment start<end、word 时码落在 segment 内、`text` = segments 拼接 trim;range 转写后时码回到源时间。
- **T8.3 TranscriptCache + TranscriptSearch**:内存 LRU=4(满清空)、磁盘 JSON、`filter` 半开重叠、`terms`/`matches`(AND 子串、大小写/变音不敏感 NFD)。验收:`filter` 段/词重叠判定精确、`text` 空格 join;`terms("budget, plan")→["budget","plan"]`;`matches` 变音不敏感(café~cafe)、AND 语义;磁盘 JSON 与上游 `TranscriptCache` 互读。
- **T8.4 FrameSampler + LumaGrid**:高分辨率间隔翻倍、stride 候选、tolerance、`t>lastTime` 去重、luma 8×8 Rec.601、meanDiff>12 镜头、coverageFloor 8、lastKeptTime 只在保留时推进。验收:对合成测试视频(已知镜头切点)产出帧的 `is_new_shot` 与切点对应;候选时间序列与公式相符;高分辨率源 interval 翻倍。
- **T8.5 EmbeddingStore**:PALMEMB1 LE 布局、rowBytes=24+dim*2(dim768→1560)、f16 落盘/f32 内存、严格长度校验、atomic、key/is_current。验收:`save→load` 往返 bit 级(f16 量化后)一致;**上游 `EmbeddingStore.load` 能读本 crate 写的 `.embed`**(magic/headerLen/JSON/行布局逐字节)、反之亦然;截断/篡改→`StoreCorrupt`;`is_current` 版本三元组判定。
- **T8.6 Embedder(ort)+ 预处理 + tokenizer**:squash-resize 黑底 256²、NCHW、mean/std、tokenize 截断 64 右填 0、输出 dim768 断言。验收:预处理输出尺寸/通道/范围正确;tokenize 长度恒 64、超长截断、短补 0;`encode_image/encode_text` 返回 768 维;**(若有 candle 后端)同图同文两后端余弦 >0.999**;`normalized` 开关行为正确。
- **T8.7 VisualSearch 排名**:矩阵·向量、best-per-shot(同分保留先出现)、minScore 过滤、`prefix(limit)` 再 `filter(floor)` 顺序、top≤0 空。验收:构造已知向量集断言命中顺序/去重/截断边界与上游 `VisualSearch.search` 逐用例一致(尤其「先 limit 后 floor」最终 ≤ limit)。
- **T8.8 VisualIndexer**:视频 shot 累积(首镜头起点强制 0、shotEnd=下一镜头起点/末镜头=duration)、图片单 embedding 零长 shot、needsIndex 幂等、导出让路 + 取消。验收:对已知镜头视频断言 rows 的 `shot_start/shot_end`;重复 index 跳过(幂等);取消中途 `Cancelled`。
- **T8.9 ModelDownloader**:reqwest 流式下载 + 加权进度 + 流式 SHA256 + zip 解压(单顶层条目)+ 原子安装 + 幂等。验收:mock HTTP 下载三文件、进度单调到 1、错校验和→`Checksum`、安装目录结构正确、二次 install 直接返回。
- **T8.10 ort worker 框架**:`OrtModel::{load,run}`、`OrtWorker::submit`(序列化 + 导出暂停)、`OrtModelRegistry` 懒加载、EP 回退 CPU、tensor 辅助。验收:加载 SigLIP image encoder 跑通(即 `OrtEmbedder` 复用它);EP 不可用回退 CPU 有日志;worker 串行执行、导出暂停期间不取模型。
- **T8.11 IndexCoordinator**:schedule 条件、单 worker、dequeue 跳过失效 id、index_one 进度分配(转写 0.5)、并发转写+视觉、failed 集、search 快照 off-thread、ExportPause 引用计数 + 2s 轮询。验收:入队/去重/失败重试(failedIds 仅批内去重)行为与上游一致;导出 begin 后 worker 暂停、end 后恢复;search 空 query 返回空、命中合理。

### 持续校准(非阻塞,跨 Phase)

- **导出画质对齐**:逐预设调码率/profile/色彩,与上游 `AVAssetExportSession` 导出对比(画质/时长/音画同步)。`docs/_analysis/02` 风险登记列为 🟠 中、非阻塞核心。
- **L2 归一化标定**:确认导出 SigLIP2 是否图内归一,据此设 `EmbedderSpec.normalized`,使裸点积分数语义与上游一致(§0.8)。
- **波形视觉等价**:若后续要求与上游逐位一致,切 `WaveformMode::Peak` 并标定缩放(§4.3)。

---

## 11. 有意省略 / 不归本 crate(避免范围蔓延)

| 上游 | 处置 | 依据 |
|---|---|---|
| `ImageVideoGenerator`(图片烧静止视频) | **整类删除**;图片在 render 物化为纹理 | `docs/_analysis/02` 表 L74;ARCHITECTURE §6 L130 |
| `AlphaVideoNormalizer`(直 alpha 预乘转码) | **整类删除**;wgpu 着色器内处理 premultiplied alpha | `docs/_analysis/02` 表 L75 |
| `LottieVideoGenerator`(Lottie 烧 ProRes) | 暂不归本 crate;render 物化层或 `opentake-motion`(Phase 10) | ROADMAP Phase 10;§8.3 |
| `CompositionBuilder` / `VideoEngine` / wgpu 合成 / RenderPlan / 关键帧 ramp / renderSize 偶数化 / BT.709 instruction | **属 `opentake-render`** | ARCHITECTURE §6;ROADMAP Phase 3/4/5 |
| `MediaVisualCache` 的 @MainActor 内存表 + `needsDisplay` 触发重绘 | 内存镜像/事件推送属 `opentake-core`/前端;本 crate 只做纯生成 + 磁盘缓存 | §3.5 |
| `VisualModelLoader` 的 `@Observable` UI 状态机 | UI 镜像属上层;本 crate 提供 `installed/install/load` + warm-up | §5.6;`VisualModelLoader.swift` |
| Settings/Storage UI、账户/计费/Clerk/Convex | UI-rebuild / cloud-rebuild,非本 crate | `MODULE-PORT-MAP` L940 |

---

## 附:关键证据索引(绝对路径 + 行号)

- 解码/读写:`palmier-pro-upstream/Sources/PalmierPro/Preview/AlphaVideoNormalizer.swift:34-148`(alpha 检测/预乘转码);`Preview/ImageVideoGenerator.swift:16-206`(静止视频/偶数尺寸 `encoderDimension:68-72`/BT.709 `:168-174`);`Transcription/Transcription.swift:203-280`(`extractAudioTrack` 16k mono PCM)。
- 缩略图/波形:`Timeline/MediaVisualCache.swift`(`videoThumbnailTimes:192-202`、缩略图 `:113-118`、`makeImageThumbnail:152-163`、sprite `loadThumbnails:238-262`/`saveThumbnails:264-293`、`waveformSampleCount:186-190`、`.waveform` LE `:218-227`、缓存键 `:209-216`、闸门 `:16/27`)。
- 转写:`Transcription/Transcription.swift:5-39`(模型/`offsetting:26-38`)、`:72-90`(locale)、`:284-322`(`decodeResults` segment/word);`Transcription/TranscriptCache.swift:12-88`(缓存/filter/key);`Transcription/TranscriptSearch.swift:12-36`(terms/matches)。
- 语义搜索:`Search/SearchIndexConfig.swift:4-45`(阈值/manifest);`Search/Models/VisualEmbedder.swift:7-87`(Spec/encode/预处理 squash 黑底 `:81-85`/输出断言 `:53-61`);`Search/Models/TextTokenizer.swift:16-23`(截断 64 右填 0);`Search/Models/ModelDownloader.swift:46-172`(安装/校验/解压);`Search/Models/VisualModelLoader.swift:86-110`(load + warm-up);`Search/Indexing/FrameSampler.swift:40-117`(采样/LumaGrid `:94-117`);`Search/Indexing/VisualIndexer.swift:15-86`(累积/幂等);`Search/Indexing/EmbeddingStore.swift:30-115`(PALMEMB1);`Search/Query/VisualSearch.swift:16-56`(sgemv 排名);`Search/SearchIndexCoordinator.swift:37-257`(调度/导出暂停/查询)。
- 领域契约:`OpenTake/crates/opentake-domain/src/media.rs:226-440`(MediaResolver/MediaAsset)、`clip_type.rs:781-832`(ClipType)、`timeline.rs:931-1031`、`clip.rs`。
- 横切:`OpenTake/docs/_analysis/02-苹果框架可移植性.md`(能力→栈映射表 L66-83、攻坚清单 L98-136);`OpenTake/docs/MODULE-PORT-MAP.md`(L833-883 搜索、L923-940 存储、L1211 转写);`OpenTake/docs/ARCHITECTURE.md` §1/§6/§7;`OpenTake/docs/ROADMAP.md` Phase 2/8。
