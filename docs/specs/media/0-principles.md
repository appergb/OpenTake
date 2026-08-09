# 设计原则与移植铁律(本 crate 必须遵守)

来自 `OpenTake/AGENTS.md`「Rust 代码风格 / 移植法则」与 `docs/_analysis/02`,逐条落地:

1. **时间单位分层**:本 crate 一律用**秒(`f64`)**与**源采样位置**作 IO 边界量;帧↔秒换算(`Int(s*fps)` 截断)留在 `opentake-domain`/调用层,本 crate**不做** fps 折算。证据:上游 `Transcription`/`MediaVisualCache`/`FrameSampler` 全用 `seconds`,`secondsToFrame` 在 `MediaTab`(上层)。
2. **零硬编码常量**:所有阈值(promoteDiff=12、coverageFloor=8.0、imageSize=256、dim=768、relativeCutoff=0.85、cosineFloor=0.05、波形 count 公式 150/帧 与 20000 上限、缩略图 maximumSize=120×68、sprite 列数=50 …)以 `pub const` / `Options` 结构体集中声明,值**逐字照搬**上游。
3. **缓存键与磁盘格式逐字节复刻**:`SHA256("path|mtime_unix_f64|size")` 取前 N hex、`PALMEMB1` 二进制布局、`.waveform`/`.thumbs.jpg`+`.thumbs.json` sidecar、转写 JSON。理由:让 OpenTake 与上游/旧工程的缓存目录**可互读**(同机迁移),并保证幂等判定一致。
4. **错误用 `thiserror` 定义本 crate 错误,内部传播用 `anyhow`,边界返回 `Result<T, MediaError>`**;`opentake-domain` 零依赖,本 crate 是第一层允许 IO 的 crate。
5. **不可变 / 纯函数优先**:排名(`VisualSearch`)、波形降采样、采样判定、转写过滤等都是无副作用纯函数,可全单测;有状态的只有索引调度器(§7.7)与模型加载器(§5.6)。
6. **后端推理可插拔**:`Embedder` / `Transcriber` / `OrtWorker` 定义为 trait；启用相应 feature 时使用 ORT/whisper.cpp，测试注入 mock（协议化 DI）。当前没有可调用的 candle 后端，不得把规划中的回退实现写成已交付能力。
7. **导出期让路**:`ExportPause` 提供引用计数式压力信号；生产队列是否真正让索引/转写/缩略图/波形暂停，必须由运行期协调器的拥有测试证明，不能只凭该原语宣称完成（生产队列收口由同计划 Task 32 独立负责）。
8. **L2 归一化对齐风险**:上游裸点积 `cblas_sgemv` 是否等价余弦,取决于导出模型是否在图内 L2 归一化(`MODULE-PORT-MAP` L860)。本 crate **必须复用上游同一份权重转 ONNX**,并在 `Embedder::encode` 后做一次**条件 L2 归一化开关**(`Spec.normalized: bool`),默认 false 以匹配上游(模型内已归一化)——除非验证证明需要外部归一化。

## 可执行子能力集合

本标题是合规/验收集合，不是一个额外的“大功能”。它只在下列独立所有者都通过时成立：

- `crates/opentake-media/tests/ffmpeg_integration.rs#probe_reports_dimensions_fps_and_audio`：本地固定素材的尺寸、帧率和音轨探测。
- `crates/opentake-media/tests/ffmpeg_integration.rs#decode_frame_returns_rgba_of_expected_size`：秒制 IO 边界上的真实 RGBA 解码。
- `crates/opentake-media/tests/ffmpeg_integration.rs#extract_pcm_yields_16k_mono`：确定的 16 kHz 单声道 PCM 边界。
- `crates/opentake-media/tests/ffmpeg_integration.rs#waveform_has_expected_bucket_count`：命名常量驱动的波形桶数量。
- `crates/opentake-media/tests/ffmpeg_integration.rs#encode_roundtrip_produces_playable_video`：真实编码、探测和可播放容器往返。
- `crates/opentake-media/src/index_coordinator.rs#export_pause_ref_counts`：嵌套导出压力的引用计数原语；这不替代 Task 32 的生产队列验证。

配套纯函数/磁盘拥有测试继续分别锁定 `cache_key`、`waveform::store`、`thumbnail::sprite`、`search::embed_store`、转写缓存和条件 L2 归一化。跨平台路径、网络、许可证与隐私边界见 [`9-domain-contract.md`](9-domain-contract.md)；任一子项失败时，本标题不得作为完成证据。
