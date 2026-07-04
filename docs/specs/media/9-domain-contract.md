# 跨平台与合规要点

- **路径**:全部用 `&Path`/`PathBuf`;缓存/模型根用 Tauri `app_cache_dir`/`app_data_dir`(替上游 `~/Library/Caches/PalmierPro`、`~/Library/Application Support/PalmierPro`)。缓存子目录名沿用 `MediaVisualCache`/`Embeddings`/`Transcripts`/`Models` 以便同机迁移可读(`MODULE-PORT-MAP` L923-927)。
- **字节序**:`.waveform`/`.embed` 固定 little-endian(arm64 mac 写出可互读)。
- **FFmpeg 许可**:GPL-3.0 项目兼容 FFmpeg (L)GPL(`docs/_analysis/02` 表末、`DECISIONS.md`);动态链接 + NOTICE 标注。
- **模型许可/托管**:SigLIP2 与 whisper 权重需自托管为 ONNX/gguf;在 NOTICE/README 标注来源(对齐 `DECISIONS.md` 合规栏)。**唯一网络请求**是模型权重一次性下载(非闭源云);转写/索引/查询全本地(对齐 `Search` 模块「无闭源云接触」,`MODULE-PORT-MAP` L879)。
- **遥测**:上游 `Log.*.notice(..., telemetry:)` 经 Sentry;OpenTake 改 `tracing` 本地结构化日志,内容仅计数/状态(MODULE-PORT-MAP L879/L1211)。
- **隐私**:转写/embedding 不外发;只有模型权重下载是出网。
