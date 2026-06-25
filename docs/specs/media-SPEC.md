# opentake-media 实现就绪规格 (Issue #8)

> 范围:`crates/opentake-media`。把上游基于 AVFoundation / DSWaveformImage / macOS 26 Speech / CoreML 的媒体读取层,移植为跨平台 Rust:**ffmpeg-sidecar 解码/编码/缩略图/抽 PCM、Symphonia 波形、whisper-rs 转写、ort + SigLIP2 + tokenizers 语义搜索、ort 通用推理 worker**。本 crate 是媒体**读取与离线分析**层,不含 wgpu 帧合成器(那在 `opentake-render`,见 §9 边界)。candle 仅作可选备选后端。
>
> 状态:设计规格。对应 ROADMAP **Phase 2**(缩略图/波形,易)与 **Phase 8**(转写/语义搜索/进阶 AI worker)。本 crate 在 workspace 已有 `src/lib.rs` 的 `MediaEngine` facade 与基础 smoke tests,不是空壳。
>
> 真理来源(均为只读上游,绝对路径):
> - 解码/编码/缩略图/PCM:`palmier-pro-upstream/Sources/PalmierPro/Preview/{ImageVideoGenerator,AlphaVideoNormalizer,TimelineRenderer}.swift`、`Transcription/Transcription.swift`(`extractAudioTrack`)、`Timeline/MediaVisualCache.swift`(缩略图 sprite + 波形 + 磁盘缓存)。
> - 转写:`Transcription/{Transcription,TranscriptCache,TranscriptSearch}.swift`。
> - 语义搜索:`Search/{SearchIndexConfig,SearchIndexCoordinator}.swift`、`Search/Models/{VisualEmbedder,VisualModelLoader,ModelDownloader,TextTokenizer}.swift`、`Search/Indexing/{FrameSampler,VisualIndexer,EmbeddingStore}.swift`、`Search/Query/VisualSearch.swift`。
> - 横切分析:`docs/_analysis/02-苹果框架可移植性.md`、`docs/_analysis/01-架构与数据流.md`、`docs/MODULE-PORT-MAP.md`(行级算法笔记 L833–883、L923–940、L1211)。
> - 架构/相位:`docs/ARCHITECTURE.md` §1/§6/§7、`docs/ROADMAP.md` Phase 2/8、`docs/ADVANCED-FEATURES.md`(ort worker 复用方)。
> - 领域契约:`crates/opentake-domain/src/{media,clip_type,timeline,clip}.rs`(本 crate 消费方,不可改)。

---

## 目录

0. [设计原则与移植铁律](media/0-principles.md)
1. [crate 结构与依赖](media/1-structure.md)
2. [ffmpeg-sidecar 解码/编码](media/2-ffmpeg.md)
3. [缩略图 + sprite 网格缓存](media/3-thumbnails.md)
4. [Symphonia 波形](media/4-waveform.md)
5. [ort + SigLIP2 视觉搜索](media/5-search.md)
6. [whisper-rs 转写](media/6-transcribe.md)
7. [ort 推理 worker 通用接口](media/7-ort-worker.md)
8. [与 domain / render 的接口](media/8-coordinator.md)
9. [跨平台与合规要点](media/9-domain-contract.md)
10. [分步实施清单与验收](media/10-acceptance.md)
11. [有意省略与关键证据索引](media/11-implementation.md)
