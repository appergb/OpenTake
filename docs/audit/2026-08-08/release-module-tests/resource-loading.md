# OpenTake v1.0.0-beta.2 发布前独立模块测试：资源加载 / 过度加载

日期：2026-08-08（Asia/Shanghai）
测试对象：`OpenTake-generation` 当前共享工作树，HEAD `f9398a9302e57690caa3ea21ff995fdd0ce97706`
执行约束：只读审查与现有测试；未修改产品代码、未提交、未推送。唯一写入是本报告。

## 结论

**PASS（本模块发布门槛）**。

- safe-asset helper 的 EOF、retained authority、scope、Home exact grant、当前工程媒体授权和单段 Range 边界，fresh focused 回归 **18/18 通过**。
- Rust media prewarm 的有界队列、三 worker、同键合并、project epoch 取消、running decoder 取消、旧 epoch 禁止发布和低优先级 sprite 隔离，fresh focused 回归 **9/9 通过**。
- 前端统一派生资源调度器及 MediaPanel / MediaSearch / Preview 接线，fresh focused 回归 **6 files / 90 tests 通过**；覆盖 shared active/pending 上限、typed single-flight、epoch/latest-wins、取消/重挂和饱和淘汰。
- 正式安装包的既有 GUI 资源证据中，双轨播放 24 次采样的进程族同时 RSS 最大和为 **540,912 KiB（528.234 MiB）**，FFmpeg/ffprobe family 每次均为 **0**；GUI 导出 60 次采样中同时最多 **1 个持续 encoder + 1 个短 helper/ffprobe**。在该 fixture 和观察窗口内没有无界派生加载或子进程风暴迹象。

这不是“任意项目都无性能问题”的证明：没有 100 项冷缓存压力、长时间多轨、低内存设备或 Windows 实机采样。因此发布表述应保持为“确认的无界路径已修复；现有正式包样本未见过度加载”，不得升级为全面性能认证。该边界是已披露的 E4 覆盖限制，不构成本模块 blocker。

**Blocker：无。**

## 1. 静态边界核对

### 1.1 safe-asset / helper

实现：`src-tauri/src/safe_asset_protocol.rs`、`safe_asset_protocol/helper.rs`、`response.rs`、`tests.rs`。

| 边界 | 当前真实值 | 核对结果 |
|---|---:|---|
| asset active reads | 4 | `MAX_CONCURRENT_READS`；worker semaphore 有界 |
| asset pending reads | 32 | `try_acquire_owned` 满载立即 503 + `Retry-After: 1` |
| helper global process slots | 4 | spawn 前 `try_acquire_owned`；不可回收 child 连 permit 一并 quarantine |
| worker / helper I/O deadline | 5 s | 排队/读 helper 超时为 504；kill 后 reap 另有 1 s 有界期限 |
| Range | 单段；每响应至多 1000 KiB | multi-range 为 416；返回真实 `Content-Range`、ETag |
| full body | 普通 32 MiB；图片 128 MiB | 超限 413，要求客户端使用 Range；HEAD 不读 body |
| MIME | inert image/audio/video/octet-stream | SVG 拒绝；`nosniff` + sandbox CSP + origin bound |

EOF 根因对应的当前代码在写完 helper JSON 后显式 `drop(stdin)`，然后才等待 stdout；没有依赖 Unix 上无效的 `ChildStdin::shutdown()`。focused 回归用 `/bin/sh` 的 `read_to_end` 等价路径在 500 ms 内观察 EOF。

权限边界为二次 fail-closed：

- Home 只例外允许 `.opentake/thumbnail.jpg` 的 **exact file grant**，递归 bundle grant 本身不足以触发例外。
- 当前 `.opentake` 内嵌套媒体由 retained `ProjectRootIdentity` 授权，不要求扩大成递归 Tauri scope；bundle 路径反弹/identity 变化拒绝。
- 外部媒体除 app cache、`OpenTake/Library`、resource root 外，必须同时在当前 `AppCore` manifest 中；A→B 切工程后旧 A grant 失效。
- `ScopeOnly` / `ProjectMedia` 都绑定 normalized exact requested path、initial final path 和 ETag；发布前持 project identity workflow lease 并复验 final path/token，ancestor rebinding 和同路径 inode 替换均拒绝。

### 1.2 Rust media prewarm

实现：`src-tauri/src/media/prewarm.rs`。

- 主 poster/visual 队列为 **64 pending / 3 workers**；单独低优先级 timeline sprite 队列为 **8 pending / 1 worker**。
- reservation 覆盖 queued + running；同 `kind + cache_key` 返回 `Duplicate`，不重复占槽。
- 满载 `try_send` 返回 `Busy`，reservation 会清理，不存在无界 mpsc。
- project transition 同时取消主/低优先级 token；queued 不运行、running decoder token 会终止 child/readers；旧 epoch staging 文件不得 rename 发布。
- 交互期取消单独的低优先级 sprite worker，不阻塞 poster workers。

模块头注释仍写“24-job queue”，而常量/测试的真实主队列是 64；本报告按可执行常量和测试记账，不按漂移注释记账。该文档漂移不改变运行边界。

### 1.3 前端统一调度与调用点

实现：`web/src/lib/derivedResourceScheduler.ts`、`MediaPanel.tsx`、`MediaSearch.tsx`、`Preview.tsx`。

- waveform、card thumbnail、search thumbnail、preview poster 共用一个 admission domain：**4 active / 64 pending**。
- identity 包含 `projectEpoch + invariant typed resource kind + key`；同 raw key 的不同结果类型不会误合并；相同 identity 为 single-flight。
- 单订阅取消互不误伤；queued 最后订阅取消会真移出队列。active 最后订阅取消后仍保留 keyed physical flight，立即重挂/A→B→A 可复用，不再开启第二个同键 FFmpeg 工作。
- `activateProject` 使旧 epoch queued/active 订阅不可发布；active IPC 当前只能 abort 前端订阅，不能物理 kill 已进入 Rust 的现有 Tauri command，因此继续占 active 槽直至结算。
- Preview 使用 `latestGroup="preview-poster"` + interactive priority；旧 poster 清空、abort 且不可晚到写入。队列满时 interactive 可淘汰低优先级项；全 interactive 时淘汰最旧同级项，pending 仍不超过 64。
- MediaCard thumbnail / audio waveform 只在 IntersectionObserver 的 **160 px** 近视口 admission；缩略图 path LRU 为 **256**。视频 `<video preload="metadata">`，不会以前端 `auto` 预加载整文件。

## 2. Fresh focused 测试证据

### 2.1 safe-asset authority / scope / EOF / Range

命令：

```text
/usr/bin/time -lp cargo test -p opentake-tauri safe_asset_protocol::tests --lib -- --nocapture
```

结果：**exit 0；18 passed / 0 failed / 0 ignored；519 filtered；test 0.17 s，real 1.11 s**。

直接覆盖的关键规格测试包括：

- `helper_request_pipe_reaches_eof_before_waiting_for_the_response`
- `serves_only_a_bounded_single_range_from_the_retained_file`
- `stale_if_range_never_splices_bytes_from_a_different_identity`
- `rejects_multi_range_and_oversized_full_body`
- `response_for_request_rejects_scope_only_alias_that_resolves_outside_scope`
- `home_thumbnail_exception_requires_an_exact_file_grant`
- `current_project_authority_allows_nested_media_without_recursive_scope`
- `exact_external_grants_follow_the_active_project_while_static_roots_remain_available`
- ancestor rebinding、same-path identity replacement、bundle replacement、FIFO/symlink、timeout/reap/capacity recovery 回归。

`time -lp` 的测试命令进程最大 RSS 为 **182,173,696 B**、peak memory footprint **157,106,896 B**。这是 Cargo/test harness 的短时进程指标，不是正式应用运行时 RSS。

### 2.2 Rust prewarm

命令：

```text
/usr/bin/time -lp cargo test -p opentake-tauri media::prewarm::tests --lib -- --nocapture
```

结果：**exit 0；9 passed / 0 failed / 0 ignored；528 filtered；test 0.01 s，real 0.30 s**。

覆盖：3-worker 峰值、64 pending 后 `Busy`、同键合并、queued/running epoch cancel、旧 epoch 无 cache publish、取消后新 epoch 重挂、1 个独立低优先级 worker和交互取消。

测试命令最大 RSS **182,009,856 B**、peak memory footprint **156,943,032 B**；同样不作为应用运行时指标。

### 2.3 Web scheduler / Media / Preview

命令（在 `web/`）：

```text
/usr/bin/time -lp pnpm exec vitest run \
  src/lib/derivedResourceScheduler.test.ts \
  src/components/media/MediaPanel.test.tsx \
  src/components/media/MediaSearch.test.ts \
  src/components/preview/Preview.poster.test.tsx \
  src/components/preview/Preview.test.tsx \
  src/components/preview/Preview.interaction.test.tsx \
  --reporter=default
```

结果：**exit 0；6 files / 90 tests passed / 0 failed；Vitest 996 ms，real 1.36 s**。

文件计数：scheduler 13、MediaPanel 40、MediaSearch 15、Preview poster 1、Preview 17、Preview interaction 4。scheduler 的 13 项覆盖 shared active/pending、single-flight、subscriber cancel、unmount/remount、pending cancel、epoch invalidation、latest-wins/AbortSignal、priority、低优先级/同级交互饱和淘汰、typed kind、A→B→A physical-flight reuse。组件证据另验证 6 个 waveform 只有 4 active + 2 pending、unmount 清 pending；旧 epoch waveform、旧 observer callback、旧 preview poster 均不发布。

测试命令最大 RSS **237,125,632 B**、peak memory footprint **78,763,120 B**，仍只是并行 Vitest workers/happy-dom 的命令级指标。stderr 有既有 React `act(...)` warning，但没有 assertion failure 或 unhandled test failure；不把 warning 隐藏，也不把它解释为资源失败。

## 3. 正式 GUI / RSS / 子进程既有证据复核

本轮没有启动第二个 GUI 实例或重跑资源测量；只解析正式安装包既有原始采样，避免污染同一单实例证据。来源：

- `docs/audit/2026-08-07/editor-core-after-fix-assets/dual-playback-active-process-sample.txt`
- `docs/audit/2026-08-07/editor-core-after-fix-assets/gui-export-resource-sample.txt`
- 同目录 `README.md` 与 `p0-asset-scope-import.md`。

### 双轨播放

- 24 次采样，时间窗 `13:25:07.901620+08:00` 至 `13:25:14.599630+08:00`，约 6.70 s。
- 同一采样中 OpenTake main + WebKit GPU + Networking + WebContent 的最大 RSS 和：**540,912 KiB = 528.234 MiB**（发生于首个采样）。
- 24/24 的 `ffmpeg_family=0`；原始行中应用 FFmpeg/ffprobe 进程计数也是 0。
- 该结果只说明 fresh 双轨 4K60 fixture 的短窗口没有后台 helper fan-out；不外推到长时播放、更多轨或不同机器。

### GUI 导出

- 60 次采样，约 `13:26:44.203888+08:00` 至 `13:27:16.250930+08:00`，约 32.05 s。
- 每次同时最多 **1 encoder**；另最多 **1 个短帧 helper 或 ffprobe**。
- 观察到 **1 个唯一 encoder PID**、29 个短 helper PID、5 个 ffprobe PID；它们是逐帧/探测的序列化短进程，不是 29/5 个同时并发。原始采样的最大并发计数才是判定边界。
- 这与导出架构中持续 encoder + bounded per-frame helper 相符；样本内未见并发数随时间增长。

### Asset GUI 可用性

既有 fresh custom-protocol release 证据记录 Home 封面 `200`、工程 thumbnail cache `200`、源视频 Range `206`、preview cache `200`、timeline sprite `200`，并目视看到 Home 封面、素材卡和选中素材预览恢复。完整安装包随后重启/重开工程复验也保持可见。它补足 unit test 不会启动真实 WebView/custom protocol 的边界，但本轮没有独立抓包，故不重复声称新的 E3 样本。

## 4. 命令账本

以下命令均在 `OpenTake-generation/`（明确标注 `web/` 的除外）执行；退出码均来自本轮实际执行。

| 命令/用途 | Exit | 计数/结果 |
|---|---:|---|
| `pwd; git status --short --branch; rg --files ...`，工作树与指令/manifest 发现 | 0 | 当前共享树非 clean；最终统计 22 tracked modified + 7 untracked = 29 paths |
| `sed` 读取 workspace/仓库 `AGENTS.md`、`CLAUDE.md`、release spec、safe-asset/helper/response/tests、prewarm、scheduler、MediaPanel、Preview 与相关 tests | 0 | material context 已完整/分段读取；未写文件 |
| `rg --files` / `rg -n` 定位 prewarm、asset、scheduler、GUI/RSS 证据；`wc -l` 核对读取范围 | 0 | 代码/测试和正式证据路径均存在 |
| `/usr/bin/time -lp cargo test -p opentake-tauri safe_asset_protocol::tests --lib -- --nocapture` | 0 | 18/18 passed |
| `/usr/bin/time -lp cargo test -p opentake-tauri media::prewarm::tests --lib -- --nocapture` | 0 | 9/9 passed |
| 上述 6 文件 focused Vitest 命令（`web/`） | 0 | 6 files / 90 tests passed |
| `awk` 解析 `dual-playback-active-process-sample.txt` | 0 | 24 samples；max simultaneous RSS 540,912 KiB；max ffmpeg family 0 |
| `awk` 解析 `gui-export-resource-sample.txt` | 0 | 60 samples；max encoder 1；max helper/probe 1；unique PID 1/29/5 |
| `git rev-parse HEAD; git status --short | awk ...` | 0 | HEAD 如报告头；22 modified / 7 untracked |
| `ls -ld/-l docs/audit/2026-08-08/release-module-tests` | 0 | 输出目录已存在；写入前没有 `resource-loading.md` |
| `test -f .../resource-loading.md; git diff --check -- ...; wc -l -c ...; git status --short -- ...; sed -n ...` | 0 | 报告存在；Markdown diff 无 whitespace error；仅该报告显示为新文件；最终 179 行 |

## 5. 未覆盖与真实边界

- 未做 100 音频/视频冷缓存工程、滚动全库、长时 soak、低内存压力、热插拔/网络盘或 Windows 实机。
- JS scheduler 对 pending 可硬取消，对已进入 Rust 的 `generateThumbnail/getWaveform/previewPoster` 只能禁止发布和限制为 4 active，当前 IPC 没有 operation-id hard cancel。Rust prewarm 自身的 cancellable decoder 是另一条有 token 的路径，不能混为一谈。
- GUI RSS 是 `ps` 离散短窗采样；不是 Instruments、heap profile 或高频峰值捕获。540,912 KiB 是同一时刻四个进程 RSS 相加，不是泄漏斜率。
- export 的 29 helper PID / 5 ffprobe PID 是整个 60-sample 窗口内的不同短进程总数，不是并发数；并发上界以逐采样最大 1 为准。
- 测试发生在有其他 Agent 改动的共享 dirty worktree；本 Agent未回滚或覆盖任何改动。结果证明的是测试时的当前工作树，不等同于尚未打 tag 的 immutable SHA 候选资产。
