# 跨平台与合规要点

- **路径**:全部用 `&Path`/`PathBuf`;缓存/模型根用 Tauri `app_cache_dir`/`app_data_dir`(替上游 `~/Library/Caches/PalmierPro`、`~/Library/Application Support/PalmierPro`)。缓存子目录名沿用 `MediaVisualCache`/`Embeddings`/`Transcripts`/`Models` 以便同机迁移可读(`MODULE-PORT-MAP` L923-927)。
- **字节序**:`.waveform`/`.embed` 固定 little-endian(arm64 mac 写出可互读)。
- **FFmpeg 架构与许可**:运行时通过独立的 **FFmpeg 子进程 sidecar** 驱动，不做 libav 动态链接。许可证判断必须针对实际随包二进制的 configure flags 与组件逐项完成，而不能由主项目许可证代替。当前 macOS 包内 FFmpeg 6.0 报告 `--enable-nonfree` 且含 GPL 组件；在替换为锁定的可再分发构建、补齐来源/许可证/NOTICE 并重新验证前，这是明确的 **Beta 发布阻塞**，不得公开分发。
- **模型许可/托管**:SigLIP2 与 whisper 权重需自托管为 ONNX/gguf;在 NOTICE/README 标注来源(对齐 `DECISIONS.md` 合规栏)。**唯一网络请求**是模型权重一次性下载(非闭源云);转写/索引/查询全本地(对齐 `Search` 模块「无闭源云接触」,`MODULE-PORT-MAP` L879)。
- **遥测**:上游 `Log.*.notice(..., telemetry:)` 经 Sentry;OpenTake 改 `tracing` 本地结构化日志,内容仅计数/状态(MODULE-PORT-MAP L879/L1211)。
- **隐私**:转写/embedding 不外发;只有模型权重下载是出网。

## 可执行子能力集合

本标题聚合可运行的本地媒体边界，并保留需外部/制品审查的合规阻塞；它不把原则文字本身当成完成证据：

- `crates/opentake-media/tests/ffmpeg_integration.rs#probe_reports_dimensions_fps_and_audio`
- `crates/opentake-media/tests/ffmpeg_integration.rs#decode_frame_returns_rgba_of_expected_size`
- `crates/opentake-media/tests/ffmpeg_integration.rs#extract_pcm_yields_16k_mono`
- `crates/opentake-media/tests/ffmpeg_integration.rs#waveform_has_expected_bucket_count`
- `crates/opentake-media/tests/ffmpeg_integration.rs#encode_roundtrip_produces_playable_video`
- `crates/opentake-media/src/index_coordinator.rs#export_pause_ref_counts`

这些拥有者分别证明固定本地素材的探测、秒制解码、PCM、缓存派生数据、真实编码往返和导出压力原语。路径均从宿主注入的 `app_cache_dir` / `app_data_dir` 根派生，公共 facade 返回 `MediaError`，源媒体保持只读。网络与许可证还必须通过发布制品审计：模型下载只允许清单指定 URL + 摘要校验；媒体分析不外发；FFmpeg sidecar 的 `--enable-nonfree` 阻塞未解除前，本集合即使代码测试全绿也不代表 Beta 可发布。
