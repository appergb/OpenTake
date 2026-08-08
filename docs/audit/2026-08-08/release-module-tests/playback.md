# OpenTake v1.0.0-beta.2 发布前独立测试：播放引擎

- 日期：2026-08-08（Asia/Shanghai）
- 仓库：`OpenTake-generation`
- 分支：`audit/remediation-2026-08-05`（相对远端 ahead 2）
- 被测状态：当前未提交工作树；测试前已有媒体、safe asset protocol、Preview 等他人改动，本次未修改产品代码
- 结论：**FAIL（发布门禁）**

## 结论摘要

默认 `playback-engine` 路径本身通过本轮 focused 验证：播放模块单测 101/101、播放集成 7/7、真实 loopback transport 集成 6/6 均通过。transport/frame lookup、seek mailbox、pause/resume、终态、项目/时间线/session 隔离均有通过证据。前端 focused 播放组 7 文件 85/85，通过；当前完整 Web 套件也通过 137 文件 1143/1143。

发布门禁判为 FAIL 的唯一自动化 blocker 是：`--no-default-features` Rust 最小壳可以测试和构建，但“前端自动回退 legacy `<video>`”没有形成可验证闭环，且当前源码显示 feature 缺失时会把 Tauri 仍误判为 Rust engine 可用。该问题不影响默认 feature 已开启的正常候选包路径，但不满足本轮必须证明 fallback 的验收项。

另外，4 条需要 GPU/音频设备/真实媒体的 playback probe 全部仍为 ignored；本报告不能替代候选包 GUI、A/V 同步、Windows WebView2 或安装包实机验收。

## 测试命令与结果

下表列出本轮全部测试/构建命令；`rg`、`sed`、`nl`、`git status/diff` 等只读源码盘点命令不计入测试计数。

| # | 工作目录 | 命令 | Exit code | 结果 |
|---|---|---|---:|---|
| 1 | 仓库根 | `cargo test -p opentake-tauri --lib playback:: -- --test-threads=1` | 0 | 101 passed；0 failed；0 ignored；436 filtered out |
| 2 | 仓库根 | `cargo test -p opentake-tauri --test playback_transport_integration -- --test-threads=1` | 0 | 6 passed；0 failed；0 ignored |
| 3 | 仓库根 | `cargo test -p opentake-tauri --test playback_integration -- --test-threads=1` | 0 | 7 passed；0 failed；0 ignored |
| 4 | 仓库根 | `cargo test -p opentake-tauri --test playback_probe -- --test-threads=1` | 0 | 0 passed；0 failed；**4 ignored**（GPU/音频设备/真实 HEVC/ProRes/视频素材） |
| 5 | 仓库根 | `cargo test -p opentake-tauri --no-default-features --lib -- --test-threads=1` | 0 | 433 passed；0 failed；0 ignored；证明最小 lib 测试配置可用，但不证明 UI fallback |
| 6 | 仓库根 | `cargo check -p opentake-tauri --no-default-features --bin opentake` | 0 | build/check 通过；无测试计数；证明最小 Tauri binary 可编译 |
| 7 | `web/` | `pnpm test -- src/components/preview/nativePlaybackSession.test.ts src/components/preview/rustFrameBuffer.test.ts src/components/preview/RustFrameBuffer.dom.test.tsx src/components/preview/rustEngine.test.ts src/components/preview/playbackRoute.test.ts src/components/preview/timelinePlayback.test.ts src/components/preview/TimelinePlaybackLayer.dom.test.tsx` | 0 | Vitest 将 `--` 后参数按当前脚本行为跑成完整套件：137 files、1143 tests passed |
| 8 | `web/` | `pnpm exec vitest run src/components/preview/nativePlaybackSession.test.ts src/components/preview/rustFrameBuffer.test.ts src/components/preview/RustFrameBuffer.dom.test.tsx src/components/preview/rustEngine.test.ts src/components/preview/playbackRoute.test.ts src/components/preview/timelinePlayback.test.ts src/components/preview/TimelinePlaybackLayer.dom.test.tsx` | 0 | 精确 focused：7 files、85 tests passed |
| 9 | `web/` | `pnpm exec vitest run src/components/preview/Preview.test.tsx src/components/preview/previewEngine.test.ts` | 0 | 2 files、34 tests passed |

所有 Rust 命令均仅出现仓库既有的 `block v0.1.6` future-incompatibility warning；没有测试失败。

## 覆盖证据

### 1. 默认 feature 与播放 transport

- [`src-tauri/Cargo.toml`](../../../../src-tauri/Cargo.toml) 声明 `default = ["playback-engine"]`。命令 #1 未显式加 feature 仍收集并通过 101 条 `playback::*` 测试，直接证明当前默认 feature 配置实际启用了播放模块。
- `playback_transport_integration` 通过真实随机 loopback 端口覆盖：初始 204 → 发布后 200、完整可解码 JPEG/body length、错误 session 返回 204、跨 Origin 返回 403、MJPEG part 完整性。
- `playback_integration` 覆盖精确 trimmed source 冷启动、decode failure 不发布黑帧、首次 readiness 取消、运行中 stop 取消、连续流推进与 seek、双轨合成、独立 engine thread sink/emitter。

### 2. frame lookup、慢消费者与 future/cross-session 拒绝

后端最新帧语义覆盖充分且本轮通过：

- `playback::transport::tests::slow_consumer_keeps_receiving_the_newest_published_frame` 连续发布 0..59，验证同 session 的旧请求均返回当前最新帧；frame 60 的 future 请求拒绝。
- `playback::transport::tests::frame_route_never_serves_another_session_latest` 同时覆盖 session、project epoch、timeline version 三个硬边界以及 future frame 拒绝。
- `frame_route_returns_204_for_wrong_session_identity` 在真实 HTTP handler 上验证 cross-session 为 204。
- lookup 实现只允许身份完全一致且 `query.frame <= latest.frame`；sequence 在同 session 的慢消费者路径中不再作为精确命中条件。这与测试声明的最新帧语义一致。

覆盖边界：future frame 拒绝目前是 store 单测，不是独立 HTTP integration case；cross-session 已同时有 store 单测和 HTTP integration。

### 3. seek、pause/resume 与 session isolation

- `rapid_seek_mailbox_keeps_only_the_latest_frame_and_one_wake`、`a_new_seek_generation_supersedes_an_inflight_render` 覆盖 latest-only seek 与 in-flight 失效。
- `render_loop_streams_frames_advances_and_seeks` 覆盖连续 render loop 的实际推进/seek。
- 音频单测覆盖 pause 保留当前 generation 下一块、seek 丢弃旧 chunk、wall/device clock、underrun 静音等。
- `retained_paused_resume_succeeds_when_reaper_slots_are_occupied`、`retained_audio_resume_failure_restores_paused_session` 覆盖保留会话 resume 和失败回滚。
- `stale_session_control_cannot_pause_seek_or_stop_replacement` 证明旧 session 不能 pause/seek/stop 新 session。
- 项目 epoch、timeline version、安装 generation、project transition、迟到 render publication、旧 latest 清理均有对应通过测试。
- 前端 `nativePlaybackSession.test.ts` 覆盖 exact revision 的 pause/resume 保持 session id、工程/版本改变重新 mint、stale cleanup 不控制 replacement、frame sequence 单调发布。

### 4. 前端 204 retry

有覆盖，但只到状态机边界：

- `rustFrameBuffer.test.ts` 的 `keeps the last good slot visible and retries a live-frame error once` 验证 live frame 首次加载失败追加 `retry=1`、保持上一张好帧，第二次失败清空 pending 而不清空 active。
- terminal frame 有两次 retry，并覆盖 exhausted 后停止 transport、保持最后好帧。
- 后端真实 HTTP 测试覆盖 204 → 发布 → 同 URL 200，以及错误 session 204。

覆盖边界：没有浏览器/WebView 集成测试把真实 HTTP 204、`<img onError>`、`retry=1` 和后端最新帧响应串成一条链；`RustFrameBuffer.dom.test.tsx` 也没有专门 dispatch `error` 的 retry case。现有状态机与后端两端测试能证明各自语义，但不能替代 WebView2/WKWebView 跨边界验证。

## Blocker：`--no-default-features` 自动 fallback 未闭环

Rust 侧最小配置是健康的：命令 #5 为 433/433，命令 #6 binary check exit 0。但这只能证明“能编译/测试”，不能证明 Cargo 注释声称的“前端随后回退 legacy `<video>`”。源码链路如下：

1. 关闭默认 feature 时，[`src-tauri/src/lib.rs`](../../../../src-tauri/src/lib.rs) 通过 `#[cfg(feature = "playback-engine")]` 完全移除 playback 模块、state、`playback_start/pause/stop/seek` 和 `get_preview_endpoint` 命令。
2. [`Preview.tsx`](../../../../web/src/components/preview/Preview.tsx) 与 [`previewEngine.ts`](../../../../web/src/components/preview/previewEngine.ts) 把 `rustAvailable` 设为 `isTauri`，没有 feature/capability handshake；在最小 Tauri 壳中 `isTauri` 仍为 true。
3. `Preview.tsx` 启动时调用缺失的 `get_preview_endpoint`，当前 effect 只有 `.then(...)`，没有 rejection 处理。
4. native playback 只对结构化 `{code: "engine"}` 错误 fallback；[`decodePlaybackCommandError`](../../../../web/src/lib/api.ts) 明确拒绝 plain string，现有 API 测试也断言 plain string 返回 null。因此 Tauri 的 unknown-command 错误不会触发 `setEngineFailed(true)`，而是停止播放。
5. `rustEngineEnabled(false)` 和显式 localStorage `"0"` 有单测，但生产调用均使用无参 `rustEngineEnabled()`，默认返回 true；没有测试模拟 feature 缺失/unknown command。

影响范围：默认候选包开启 `playback-engine`，所以正常发布路径未复现该故障；但 minimal escape hatch 不能宣称具备自动 fallback。需要产品实现 feature/capability handshake，或让缺失命令在初始化时将 Rust availability 置 false，并新增最小 Tauri 壳 → WebKit/unsupported 路由的集成测试后重跑本门禁。

## Windows 源码/cfg/测试覆盖盘点

本轮未在 Windows 执行，以下仅为源码与 CI 定义盘点，不能替代 Windows Agent：

- playback 源码内唯一 Windows 专属测试是 `audio::tests::default_output_rate_survives_sequential_short_lived_callers`，验证 WASAPI rate probe 的短生命周期调用者安全。
- transport/session/engine 主体没有 Windows 专属 cfg；portable loopback 测试理论上会在 Windows workspace suite 收集。
- [`.github/workflows/ci.yml`](../../../../.github/workflows/ci.yml) 的 `windows-product` 使用 `windows-2022`，执行默认-feature workspace clippy/test、完整 Web test、minimal-feature clippy、MSI/NSIS 构建和安装启动 smoke；因此已提交 exact-SHA 可获得 Windows 编译/单测证据。
- 当前工作树未提交，不能把既有/未来 exact-SHA CI 结果归因于本轮候选状态。
- Windows product job 没有自动化实际点击播放、检查 `/frame` 204/retry、WebView2 Origin/CSP、A/V 同步或 seek/pause；架构文档也明确把 Windows WebView2 transport/CSP/sidecar 行为列为未验证。

## 未覆盖与发布边界

- 4 条 `playback_probe` ignored：实时 A/V、安全音频 fallback、HEVC Main10 黑/绿帧、ProRes、调色可见性。
- 未进行候选安装包 GUI 播放、scrub、pause/resume、音画同步和项目切换观察。
- 未进行真实慢消费者压力/4K 多轨资源测试；本轮仅运行确定性 latest-frame 单测。
- 未验证 Windows WebView2、Windows 音频设备、Windows 安装包播放或 exact-SHA CI。
- 未跑整个 Rust workspace；Rust 范围限定为 Tauri playback focused、transport/integration、probe 和 minimal-feature Tauri lib/bin。

## 最终裁决

- **默认 `playback-engine`：PASS**
- **transport / frame lookup / seek / pause / session isolation：PASS（上述边界内）**
- **慢消费者最新帧、future/cross-session 拒绝：PASS**
- **前端 204 retry：PASS（状态机级）；真实 WebView 跨边界未覆盖**
- **`--no-default-features` 编译/测试：PASS**
- **`--no-default-features` 自动 legacy fallback：FAIL / blocker**
- **播放模块发布总门禁：FAIL**
