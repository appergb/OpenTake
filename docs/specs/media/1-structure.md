# crate 结构与依赖

## 1.1 模块布局(`many small files`,每文件 <400 行)

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

## 1.2 Cargo 依赖(建议版本范围,实施时锁定)

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
ffmpeg-sidecar          # 调用系统 ffmpeg/ffprobe;不直接链接 libav*
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

依据 `docs/_analysis/02` 成熟度判断:ffmpeg-sidecar/symphonia/whisper-rs/ort 全部「成熟、机械移植」,blocker 不在本 crate(blocker 是 `opentake-render` 的 wgpu 合成器)。

## 1.3 顶层错误类型

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

## 1.4 通用缓存键(三处共用)

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
