# B 证据线命令账本

所有命令均在 `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-generation` 运行，时区 `+0800`。除 `cargo`/`pnpm` 自身的 build/cache 输出外，本 Agent 只写 `docs/audit/2026-08-07/editor-core-validation-evidence-b/`。

## 高信号测试

| 时间 | 命令 | Exit | 结果 | `/usr/bin/time -lp` |
|---|---|---:|---|---|
| 18:53:01–18:53:02 | `cargo test -p opentake-tauri media::prewarm::tests --lib -- --nocapture` | 0 | 9 passed / 0 failed / 518 filtered | real 0.31s；max RSS 181,600,256 B；peak footprint 156,500,664 B |
| 18:53:14–18:53:18 | `cargo test -p opentake-tauri import_ --lib -- --nocapture` | 0 | 38 passed / 0 failed / 489 filtered | real 4.45s；test 3.19s；max RSS 181,141,504 B；peak 156,091,064 B |
| 18:53:27–18:53:28 | `cargo test -p opentake-tauri recursive_mirror --lib -- --nocapture` | 0 | 9 passed / 0 failed / 518 filtered | real 0.81s；test 0.17s；max RSS 181,698,560 B |
| 18:53:34–18:53:35 | `cargo test -p opentake-tauri asset_scope --lib -- --nocapture` | 0 | 3 passed / 0 failed / 524 filtered | real 0.70s；test 0.01s；max RSS 181,829,632 B |
| 18:53:40–18:53:41 | `cargo test -p opentake-tauri safe_asset_protocol::tests --lib -- --nocapture` | 0 | 12 passed / 0 failed / 515 filtered | real 0.70s；test 0.12s；max RSS 181,698,560 B |
| 18:53:47–18:53:48 | `cargo test -p opentake-media thumbnail:: --lib -- --nocapture` | 0 | 27 passed / 0 failed / 388 filtered | real 0.88s；test 0.02s；max RSS 129,449,984 B |
| 18:53:52–18:53:53 | `cargo test -p opentake-media waveform:: --lib -- --nocapture` | 0 | 21 passed / 0 failed / 394 filtered | real 0.15s；max RSS 129,761,280 B |
| 18:53:58 | `cargo test -p opentake-media proxy::tests --lib -- --nocapture` | 0 | **0 tests run** / 415 filtered；这是验证缺口，不记 PASS | real 0.15s |
| 18:54:38–18:54:39 | `pnpm exec vitest run src/components/media/MediaPanel.test.tsx src/components/media/MediaSearch.test.ts src/store/mediaActions.test.ts src/store/mediaStore.test.ts src/store/projectActions.test.ts src/components/timeline/TimelineContainer.test.ts src/lib/asset.test.ts src/lib/api.proxy.test.ts --reporter=default` | 0 | 8 files passed；136 tests passed / 0 failed；有 React `act(...)` warning | real 1.29s；Vitest 0.929s；max RSS 231,522,304 B |
| 19:00:58–19:01:01 | `cargo test -p opentake-project project_open --lib -- --nocapture` | 0 | 2 passed / 0 failed / 163 filtered | real 3.42s；test 0.13s；max RSS 398,360,576 B；peak 94,552,688 B |
| 19:00:58–19:01:16 | `cargo test -p opentake-tauri relink_ --lib -- --nocapture` | 0 | 3 passed / 0 failed / 524 filtered | real 18.23s；test 0.08s；max RSS 1,695,268,864 B；peak 156,074,656 B；含编译与 Cargo 锁等待 |

18:52:51 曾首次执行同一 prewarm 测试，测试本身 9/9 通过；命令包装器随后错误使用 zsh 只读变量 `status`，因此 shell 命令失败。已于 18:53:01 使用 `rc` 干净重跑，正式证据采用后者。

## 基线与静态证据采集

| 时间 | 命令/采集批次 | Exit | 原始结果摘要 |
|---|---|---:|---|
| 19:01:28 | 检查应用数据候选目录并检查 `/Volumes/mac` | 0 | 未找到候选目录输出；`VOLUME_MAC=ABSENT` |
| 19:01:38 | `git branch --show-current`、`git rev-parse HEAD`、`git rev-parse v1.0.0-beta.1`、`git status --short`、`git show --stat f9398a9` | 0 | 分支 `audit/remediation-2026-08-05`；HEAD `f9398a9...`；base `c73c192...`；f939 为 5 files / +182 / -6。`docs/audit/2026-08-07/` 已由并行审计 Agent 写入，未改产品源码。 |
| 19:02:07 | 单次 `rg -n` 采集 prewarm / asset / import / thumbnail / waveform / proxy / grant 的常量与调用点 | 0 | 主 prewarm 64/3，low 8/1；asset 4+32；folder 上限；波形/搜索/preview 直接调用；exact `allow_file`；proxy single-flight/cancel/partial cleanup。摘录写入 `README.md`。 |
| 19:02:15 | `rg -n` + `nl -ba` 检查 `MediaPanel` 渲染规模 | 0 | grid/list 均直接 `items.map`，无分页/虚拟化；网格每个 audio `MediaCard` 挂载 `AudioWaveform`。 |
| 19:02:40 | 全仓检索 `allow_file` / `forbid_file` / persisted-scope / asset scope | 0 | 项目媒体只 grant；旧媒体无 transition revoke；proxy 删除是唯一对应 revoke 路径；persisted-scope 在 `src-tauri/src/lib.rs:128` 初始化。 |
| 19:02:50–19:02:51 | 读取 `src-tauri/src/lib.rs`、`tauri.conf.json` 与本机 registry 的 `tauri-plugin-persisted-scope-2.3.7/src/lib.rs` | 0 | 插件启动恢复 `.persisted-scope-asset`；每个 asset `PathAllowed` 事件保存完整 allowed/forbidden 集。 |

## 覆盖含义

- `import_` 38 项覆盖显式、平面目录、递归目录的 manifest 持久化、批量原子性、导入 poster 调度、取消/限制内部路径及 MCP 导入；它不证明 native dialog / GUI 反馈。
- `recursive_mirror` 9 项覆盖普通树、symlink/reparse、FIFO、深度/条目/文件/操作/字节限制、扫描期间 namespace 变化及取消；公开 GUI 命令仍没有 cancel 参数。
- `asset_scope` 3 项覆盖 proxy scope grant/revoke、symlink reject，以及 `f9398a9` 的 external existing/missing；没有 sibling/目录拒绝与项目切换撤权测试。
- `safe_asset_protocol` 12 项覆盖 range/body 限制、scope、symlink/FIFO、symlink ancestor、超时、helper reap 与 project-root replacement。
- 前端 136 项是 jsdom/IPC contract；没有真实 WebView、FFmpeg、asset HTTP 或 CPU/RSS 测量。
- `proxy::tests` 过滤结果为 0 个匹配测试，不能被解释为 proxy 后端验证通过。

## 未运行 / BLOCKED

- 未由本 Agent 启动或重启 `/Applications/OpenTake.app`，未启动 safe-asset-helper。
- 未做独立 GUI 项目重开、缩略图/预览显示、音频卡 100 项峰值、proxy 真转码；原因是主审计 Agent 独占同一 GUI 实例做单实例资源测量，继续操作会污染 CPU/RSS/子进程窗口。
- 未用既有“未命名”项目做 materialized 正向验证，因为其外部卷 `/Volumes/mac` 未挂载。
- 未运行全仓 527 Rust / 1114 前端套件；本线只运行与导入、授权、缓存、资源边界直接相关的高信号子集。
