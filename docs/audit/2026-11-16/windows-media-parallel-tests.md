# Windows 媒体并行测试验证

> 状态：verified · 阶段：local-test-verified · 日期：2026-11-16（Asia/Shanghai）

范围为独立分支 `fix/windows-media-tests` 的提交前源码，基线为 `282278e`；不依赖 issue #248 的控制台窗口修复。环境为 Windows 11 Pro 10.0.26200 x64、Rust 1.100.0、FFmpeg/FFprobe 9.0.2。本文不声明整仓、桌面 GUI、安装包或其他平台通过验收。

## 失败与修复

- 帧取消测试原先直接调用 Unix `mkfifo`，Windows 在创建 fixture 时即失败。现改为临时 loopback TCP 输入：确认真实 FFmpeg 已建连、reader 活跃后保持零数据，再取消并检查子进程及 reader 收尾。
- FFprobe 可用性检查与媒体探测共用 8 个准入位。满载时，旧实现立即返回准入错误，再被可用性 API 转为 `false`。新增隔离子进程回归在修复前失败；修复后，可用性检查在原操作截止时间内等待，排队计入预算，超时与错误结果不做缓存。
- 并行成功路径测试曾同时提交超出业务容量的探测，实际报 `ffprobe admission limit reached`。相关单元测试与 FFmpeg 集成测试现在共用测试专用的 4 并行工作流预算；名额覆盖整个测试生命周期，60 秒内拿不到即失败。产品仍保留 8 个探测的即时拒绝策略，独立满载回归继续检查第 9 个业务请求被拒绝。

实现说明见[FFmpeg/FFprobe 模块](../../modules/opentake-media/probe-ff.md)与[解码模块](../../modules/opentake-media/decode.md)。

## 验证

在独立 E 盘 Rust 工具链、现有 FFmpeg/FFprobe 可用的环境中运行：

```powershell
cargo test --locked --offline -p opentake-media
cargo test --locked --offline -p opentake-media --lib -- --test-threads=32
cargo test --locked --offline -p opentake-media --test ffmpeg_integration -- --test-threads=32
cargo fmt --all -- --check
cargo clippy --locked --offline -p opentake-media -p opentake-process-tree --all-targets
```

- 默认并行的完整媒体测试：451 通过、0 失败、2 个原有忽略项。
- 32 线程媒体单元测试：连续 5 轮，每轮 427 通过、0 失败、1 个原有忽略项。
- 32 线程 FFmpeg 集成测试：连续 5 轮，每轮 15 通过、0 失败、1 个原有忽略项。
- 格式检查与 Clippy 通过。Rust 1.100 对 `index_coordinator.rs`、`ort_worker/mod.rs` 中既有 `fetch_update` 用法报告弃用警告，本次未修改这些无关实现。

没有新增忽略项。原有忽略项分别为显式运行的音频提取测试，以及需要 `OPENTAKE_MAIN10_FIXTURE` 的真实 HEVC Main10 素材测试；上述通过数量不包括这两项。
