# 1.0.0-beta.2 导入与导出独立测试

- 日期：2026-08-08（Asia/Shanghai）
- 范围：import / folder import / proxy / transcode / audio / subtitles / interchange / 既有 GUI 导出证据
- 被测工作树：`f9398a9302e57690caa3ea21ff995fdd0ce97706` 上的当前并行工作树；测试时 `src-tauri/src/media.rs`、`crates/opentake-media/src/proxy.rs` 等存在未提交改动，因此本报告不能替代最终 exact-SHA CI 记录。
- 约束：只读审查和测试；未修改产品代码，未重做 GUI 工程，未提交或推送。

## 结论

**PASS（macOS 候选 sidecar 路径）**。

要求内的 focused 测试共 **147 passed / 0 failed**；候选包所带 FFmpeg/ffprobe 7.0 的 H.264、H.265、ProRes 整链 probe 为 **3/3**，现有 GUI/编码产物的 fresh ffprobe 为 **4/4**。本轮没有 macOS 候选 sidecar 发布 blocker。

不把该结论外推到任意系统 FFmpeg：PATH 上 FFmpeg/ffprobe 8.1.2 的 ProRes 全时间线 probe 可重复失败，详见“风险与未覆盖边界”。

## 命令账本

| # | 命令（仓库根目录执行） | Exit | 结果 |
|---|---|---:|---|
| 1 | `cargo test -p opentake-tauri --lib explicit_import_ -- --nocapture --test-threads=1` | 0 | 5 passed：持久化、文件数/聚合字节预算、路径替换、原地增长 retained 元数据复验、FIFO 非阻塞 |
| 2 | `cargo test -p opentake-tauri --lib recursive_mirror_ -- --nocapture --test-threads=1` | 0 | 8 passed：取消、entry/file/operation/bytes、正常树、symlink、FIFO、深度、root swap |
| 3 | `cargo test -p opentake-tauri --lib directory_import_beta_limits_are_fixed -- --nocapture --test-threads=1` | 0 | 1 passed：depth 32、entries 10,000、files 5,000、operations 7,500、bytes 100 GiB |
| 4 | `cargo test -p opentake-media proxy::tests:: -- --nocapture --test-threads=1` | 0 | **11 passed**；真实 ffmpeg/ffprobe，含 cancel、source change、ABA retained source、destination conflict、private stage、FIFO/symlink、unlink |
| 5 | `cargo test -p opentake-media --test proxy -- --nocapture --test-threads=1` | 0 | **1 passed** integration |
| 6 | `cargo test -p opentake-tauri --lib export::tests:: -- --nocapture --test-threads=1` | 0 | 61 passed：codec/container preset、音频窗口/补静音/重定时、取消、保留输出句柄、post-encode probe、WAV real probe 等 |
| 7 | `cargo test -p opentake-media --test ffmpeg_integration encode_ -- --nocapture --test-threads=1` | 0 | 3 passed：H.264 playable roundtrip、H.265/HEVC MP4、ProRes MOV |
| 8 | `cargo test -p opentake-tauri --test export_integration export_with_audio_clip_mux_aac_stream -- --nocapture --test-threads=1` | 0 | 1 passed：真实 GPU/FFmpeg 时间线导出并 mux AAC |
| 9 | `OPENTAKE_FFMPEG=.../src-tauri/binaries/ffmpeg-aarch64-apple-darwin OPENTAKE_FFPROBE=.../src-tauri/binaries/ffprobe-aarch64-apple-darwin cargo test -p opentake-tauri --test export_probe -- --ignored --nocapture --test-threads=1` | 0 | **3 passed**：H.264/H.265/ProRes，均 1280×720、30 fps、60 frames；ffprobe codec 分别 h264/hevc/prores |
| 10 | `cargo test -p opentake-domain subtitle_export::tests:: -- --nocapture --test-threads=1` | 0 | 16 passed：SRT/VTT 时间戳、排序、多行、空值/边界 |
| 11 | `cargo test -p opentake-tauri --lib subtitle_export_tests:: -- --nocapture --test-threads=1` | 0 | 5 passed：SRT/VTT 写盘、cue count、DTO/serde |
| 12 | `cargo test -p opentake-project fcpxml -- --nocapture --test-threads=1` | 0 | 32 passed：XMEML 4 + FCPXML 1.10，含 A/V、链接、变速、关键帧、文字、转义、timecode |
| 13 | 候选 `ffprobe` 对 1 个 after-fix GUI MP4 + 3 个编码证据产物执行 `-count_frames`/stream/format 检查 | 0 | 4/4 可解析，详见下节 |

Cargo 的 `block v0.1.6` future-incompatibility warning 不影响本轮 exit code，但应由依赖维护单独跟踪。

## 关键证据

### 显式导入与目录导入

- Beta 常量：显式选择最多 5,000 files、聚合 100 GiB；目录导入另有 depth 32、entries 10,000、plan operations 7,500。
- `explicit_import_rejects_file_count_and_aggregate_bytes_before_mutation` 分别触发数量和字节预算，且 live/persisted/reopen manifest 均不变。
- `explicit_import_rejects_a_path_replacement_before_atomic_commit` 在 selected pathname 被换 inode 后原子拒绝。
- `explicit_import_rejects_in_place_growth_before_atomic_commit` 证明 retained identity 之外还复验 admission bytes。
- `explicit_import_rejects_a_fifo_without_waiting_for_a_writer` 在 2 秒门槛内跳过 FIFO；目录镜像则 fail closed 且不发布。

### Proxy

- 单元 **11/11** + integration **1/1**。
- 成功产物经真实 probe，尺寸受限且保留至多一条音频流；源字节/hash 不变。
- 覆盖转码取消、verification/prepublication 取消、源变更、A→B→A pathname ABA、unlink 后 retained source、目标竞争、ambient partial/FIFO rebound、初始 FIFO/symlink 非阻塞拒绝。

### 编码、音频与候选 sidecar

- 候选 sidecar：FFmpeg/ffprobe 7.0；SHA-256 分别为 `326895b16940f238d76e902fc71150f10c388c281985756f9850ff800a2f1499`、`307e09bc01bd72bde5f441a1a6df68769da3b2b6e431accfbfc9cf3893ad00c4`。
- 真实整链 probe：H.264/H.265/ProRes **3/3**；每个 summary 与 ffprobe 均为 1280×720、30 fps、60 frames、无音频测试素材。
- 音频时间线集成 **1/1**：真实输出含 AAC stream；另有 WAV real probe 与 PCM 窗口、静音补齐、speed retime 单测。

### 既有 GUI/编码产物（未重做工程）

使用候选 ffprobe 7.0 fresh 核对，命令整体 exit 0：

| 产物 | Fresh ffprobe |
|---|---|
| `editor-core-after-fix-assets/gui-export-after-fix-720p.mp4` | h264 High，1280×720，yuv420p，60 fps，240 frames，4.000 s，489,960 bytes |
| `export-h264-text-720p.mp4` | h264 High，1280×720，yuv420p，30 fps，60 frames，2.000 s，396,263 bytes |
| `export-h265-text-720p.mp4` | hevc Main，1280×720，yuv420p，30 fps，60 frames，2.000 s，228,904 bytes |
| `export-prores422-text-720p.mov` | prores HQ，1280×720，yuv422p10le，30 fps，60 frames，2.000 s，14,454,615 bytes |

GUI MP4 的 fresh SHA-256 为 `6ac341e9e8f89be91f65063fa21e65d296d8aed861d3f3b04939cc0a9c63824f`，与既有证据 README 记录一致。既有截图 `14-gui-export-completed.png`、`15-gui-export-frame-2s.png` 和 ffprobe JSON 均存在；本轮按授权未再次操作 GUI。

### 字幕与交换格式

- SRT/VTT 纯逻辑 **16/16**，Tauri 写盘/DTO **5/5**。
- XMEML 4 与 FCPXML 1.10 **32/32**；覆盖合法转义、共享 asset、A/V 属性、链接关系、变速、变换、音量、timecode 和文字 title。
- 既有证据目录中的 `captions.srt`、`captions.vtt`、`timeline-xmeml.xml`、`timeline-modern.fcpxml`、`timeline.edl`、`timeline.otio` 均存在；本轮未重生成或覆盖。

## 风险与未覆盖边界

1. **系统 FFmpeg fallback 兼容性（非候选 sidecar blocker）**：PATH FFmpeg/ffprobe 8.1.2 下，三编码整组首次为 exit 101（H.264 通过，H.265/ProRes 失败）；H.265 单独重跑 exit 0，ProRes 单独重跑仍 exit 101，失败点为 post-encode ffprobe validation。由于候选包固定使用并已验证 sidecar 7.0，macOS 候选结论仍为 PASS；若产品承诺任意 PATH FFmpeg 开发 fallback，则 ProRes 8.1.2 是待修兼容性缺陷。
2. **Windows**：未覆盖 Windows retained output handle、Job Object、reparse point/junction、安装包 sidecar 与 WebView；必须由最终 exact-SHA Windows CI/实机验证，macOS 结果不能替代。
3. **真实设备/长时边界**：本轮未重做 GUI；未验证长时多轨 A/V drift、音频设备试听、超大 5,000 文件/100 GiB 的真实磁盘压力、目录导入公开 UI 的主动取消、网络/云盘 recall。
4. **交换格式接收端**：Rust 结构/写盘与已有产物存在性已覆盖，但未在 Final Cut Pro、Premiere、DaVinci、剪映中执行真实 roundtrip import。
5. **exact-SHA**：共享工作树测试期间存在并行未提交修改；发版前仍需在冻结 SHA 上重放候选 sidecar gates 并保存 CI 日志。

## Blocker

- 当前 macOS 候选 sidecar范围：**无 blocker**。
- 跨平台发布的外部 blocker：最终 exact-SHA Windows CI/实机证据尚不在本轮范围内。
