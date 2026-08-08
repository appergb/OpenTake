# 剪辑主体审计 B 证据线：导入、资源授权与加载有界性

审计时间：2026-08-07（Asia/Shanghai）
审计工作树：`OpenTake-generation`
分支 / HEAD：`audit/remediation-2026-08-05` / `f9398a9302e57690caa3ea21ff995fdd0ce97706`
比较基线：`v1.0.0-beta.1` / `c73c192f6fdc01e35a1711089b6e59edc3312309`

## 结论

导入、缺失媒体识别、重链、缩略图缓存、项目级预热取消等主体契约成立；但“资源加载不存在过度加载”不能判定为 PASS。素材网格中的音频波形会随所有已挂载音频卡片直接触发 `get_waveform`，没有共享并发上限、排队上限、同键合并或卸载后端取消。冷缓存大音频工程可同时启动大量 FFmpeg 解码，资源有界性判定为 **FAIL（E1）**。

`f9398a9` 的项目重开授权确实使用 Tauri `allow_file` 的转义精确文件模式，且只授权已落盘的普通文件、跳过丢失/云占位文件；“不扩目录”结论成立。可是该授权被 `tauri-plugin-persisted-scope` 自动持久化，项目切换/关闭没有对应的旧媒体撤权，历史精确文件权限会累积。因此这一修复在功能契约上为 PASS，在最小权限生命周期上只能是 **PARTIAL（E1+E2）**。

打包应用的完整 GUI/CPU/RSS/子进程压力测量由主审计 Agent 独占单实例执行；本证据线按协调要求停止 GUI，避免污染其测量窗口。因此本文的实际 GUI 解码、缩略图显示与资源峰值结论为 **BLOCKED（协调隔离）**，不以单元测试冒充端到端验证。

## 判定矩阵

证据等级：E1 = 代码路径/依赖实现；E2 = 可重复自动化契约测试；E3 = 打包 GUI/真实进程测量；E4 = 输出文件解析或跨实现比对。

| 检查项 | 判定 | 证据等级 | 关键证据与边界 |
|---|---|---:|---|
| 单文件/多文件导入与持久化 | PASS（功能）/ PARTIAL（资源） | E2 + E1 | `import_` 38/38；支持类型过滤、批量原子持久化、每个已提交素材安排 poster。公开 `import_media` 是同步命令，路径数量无硬上限，逐个使用无超时 `probe_media`，也无取消令牌。 |
| 平面/递归目录导入 | PASS（功能）/ PARTIAL（可操作性） | E2 + E1 | `import_` 38/38、`recursive_mirror` 9/9；深度 32、条目 10,000、文件 5,000、操作 7,500、总字节 100 GiB，每次 probe 15 秒上限；公开命令传 `cancel=None`，用户不能取消长扫描/探测。 |
| 保存后项目重开媒体授权 | PARTIAL | E2 + E1 | `asset_scope` 3/3；存在 external 文件获准、丢失文件不获准。Tauri 2.11.3 `allow_file` 使用 `escaped_pattern`，不是目录通配。缺少“相邻文件/目录仍拒绝”的专项断言，也未完成独立打包 GUI 重开。 |
| 旧项目/丢失媒体/重链 | PASS（契约）/ BLOCKED（GUI） | E2 | `project_open` 路径策略 2/2；`relink_` 3/3。丢失/未物化源在 DTO 中标 `missing=true` 且不发缩略图；重链保持 asset id、清除 missing，类型不匹配拒绝。`/Volumes/mac` 在审计机未挂载，既有真实工程无法用于 materialized 重开显示。 |
| 网格缩略图 | PASS（主路径）/ PARTIAL（退化路径） | E2 + E1 | 前端共享并发 4、同键 in-flight 合并、LRU 256；IntersectionObserver 仅加载视口附近。待处理数组本身无容量上限；无 IntersectionObserver 时所有卡片都会排队。 |
| 时间线 sprite | PASS | E2 + E1 | 最多 240 帧；主时间线使用全片代表性采样；独立低优先级队列 8 / 单 worker，交互时取消，项目 epoch 变更取消且旧作业不能提交缓存。 |
| 预览 poster / 搜索命中缩略图 | PARTIAL | E1 + E2 | 搜索 Moments/Spoken 各最多 20，但 `HitThumbnail` 直接调用同步缩略图命令，最多形成约 40 个冷解码突发；快速切换预览时 cleanup 只忽略 React 结果，不能取消后端 1920×1080 解码，也绕过 scheduler 的同键合并。 |
| 音频波形 | FAIL（加载有界性） | E1；算法 E2 | waveform 算法/缓存测试 21/21，桶数有界；素材卡片通过 `items.map` 全量挂载，每张音频卡立即直接 `getWaveform`，无队列、并发、全局同键合并或后端取消。时间线路径本身有 prewarm gate 和 in-flight 合并。 |
| 项目 prewarm | PASS | E2 + E1 | 主队列 64、3 workers；sprite 队列 8、1 worker；过载返回 Busy；同键合并；项目切换取消 queued/running 子进程；9/9 测试通过。 |
| asset 读取 | PASS | E2 + E1 | 4 个并发读取、32 个 pending、5 秒排队/IO deadline；普通 body 32 MiB、图片 128 MiB、range 1000 KiB；安全协议 12/12。 |
| proxy | PARTIAL | E1 + 前端契约 E2 | 后端全局 single-flight、`spawn_blocking`、可取消并 kill/wait、partial 清理后原子 rename；但 `cargo test -p opentake-media proxy::tests` 实际运行 0 个测试，没有真实转码/取消/残留物回归证据。 |
| 工程打开是否自动全量加载 | PASS（不会） | E1 + E2 | 项目打开只恢复 manifest、项目 epoch 与精确 asset scope；无自动 proxy 或全量 sprite 生成。视觉资源由视口、时间线/选择触发，并由有界 prewarm 管理；例外是已挂载音频卡片的直接 waveform 路径。 |

## 原始代码证据

### 1. 导入路径与失败模式

- `src-tauri/src/media.rs:1399-1457`：公开 `import_folder` 是同步 Tauri command；调用 `prepare_directory_import(..., None, ...)`，即无用户取消 token。扫描、probe、命名空间复核全部在一次提交前完成，失败不会发布半成品 manifest。
- `src-tauri/src/media.rs:1575-1580`：目录导入硬上限分别为深度 32、条目 10,000、文件 5,000、计划操作 7,500、聚合 100 GiB；单文件 probe 15 秒。
- `src-tauri/src/media.rs:1997-2017`：文件按顺序 probe 并写入内存计划；不支持类型只进入 skipped。
- `src-tauri/src/media.rs:2222-2235`：目录导入通过已打开普通文件句柄调用可取消、带 15 秒 deadline 的 probe。
- `src-tauri/src/media.rs:2367-2415`：公开 `import_media` 逐路径构建完整 plan 后一次持久化；没有条数/字节上限。
- `src-tauri/src/media.rs:1162-1179` 与 `1016-1020`：显式文件导入的 `prepare_import_file` 即使收到 cancel 也只在 probe 前后检查；中间使用 `engine.probe(path)`，没有 timeout/cancellable handle。
- `src-tauri/src/media.rs:2258-2300`：导入完成后的 poster 调度最多等待 Busy 队列 2 秒，每 20 ms 重试，不会无限等待。

失败模式：不存在路径被静默忽略；不支持扩展名进入 `skipped`；目录发生 symlink/reparse、FIFO/特殊项、深度/数量/字节越界、扫描期间 namespace 变化时整批 fail closed；probe 失败降级为默认元数据，仍允许导入支持扩展名。

### 2. `f9398a9` 项目重开授权

- `src-tauri/src/commands.rs:415-463`、`:512-537`：有/无 playback feature 的 `project_open` 成功提交后都调用 `grant_project_media_asset_scope`。
- `src-tauri/src/media.rs:4421-4446`：遍历当前 manifest；只对 `is_materialized_regular_file` 的解析路径调用 `allow_file`，不调用 `allow_directory`。
- `src-tauri/src/fs_availability.rs:110-120`：最终路径必须为已落盘 regular file，拒绝最终 symlink 及 dataless/partial placeholder。
- Tauri 2.11.3 `scope/fs.rs:367-378`：`allow_file` 把路径交给 `escaped_pattern`；目录授权另由 `allow_directory` 追加 `*`/`**`。因此授权粒度是精确文件。
- `crates/opentake-project/src/bundle.rs:44-69`、`:660-735` 与 `path_policy.rs:7-18`：bundle 内项目相对媒体/代理路径只能由普通相对组件组成，拒绝 `..`、绝对路径、反斜线、盘符/ADS；相应测试通过。
- `src-tauri/src/media.rs:455-475`：丢失/未物化 finalized source 标记 `missing=true`，并拒绝返回缓存缩略图。
- `src-tauri/src/media.rs:7480-7525`：当前回归测试覆盖 existing external 被允许、missing external 被拒绝；没有 sibling/目录拒绝断言。

权限生命周期残留：`src-tauri/src/lib.rs:122-128` 启用 persisted-scope。插件 2.3.7 的 `src/lib.rs:211-249` 会在启动时恢复 asset 允许项，并在每个 `PathAllowed` 事件后把完整允许集写入 `.persisted-scope-asset`。仓内旧媒体没有与项目切换对应的撤权逻辑，只有唯一代理文件删除时的 `forbid_file`。所以授权“不扩目录”，但会跨项目/重启累积历史精确文件。

### 3. 缩略图、预览、波形与 proxy

- `web/src/components/media/MediaPanel.tsx:87-101,221-263`：网格缩略图并发 4、LRU 256、同键 promise 合并；pending 数组没有硬容量。
- `web/src/components/media/MediaPanel.tsx:1785-1835`：thumbnail 与 video poster prewarm 用 IntersectionObserver（rootMargin 160 px）延迟到近视口。
- `web/src/components/media/MediaPanel.tsx:945-995,1981-1986,2328-2350`：网格/列表全量 `items.map`，音频卡组件挂载即直接 `getWaveform`；cleanup 只阻止 `setState`，不取消后台命令。
- `src-tauri/src/media.rs:3601-3628`、`crates/opentake-media/src/waveform/mod.rs:84-103`：普通 `get_waveform` 使用新 token 的缓存函数；没有进程级 single-flight/队列；并发 cache miss 可重复解码同一文件。
- `web/src/components/timeline/TimelineContainer.tsx:1044-1098`：时间线音频路径先等 scheduler cache admission，并用 source key + in-flight Set 合并；问题只在直接素材卡路径。
- `web/src/components/media/MediaSearch.tsx:754-774`：每个 Moment/Spoken 命中直接 `generateThumbnail`，cleanup 不取消后端；两组后端 `SEARCH_LIMIT=20`，因此单次结果突发有上限但仍绕过共享并发 4。
- `web/src/components/preview/Preview.tsx:648-668` 与 `src-tauri/src/media.rs:3563-3592`：预览直接运行同步 1920×1080 poster decode，卸载只忽略结果；和视口 prewarm 可同时冷解码相同目标。
- `src-tauri/src/media/prewarm.rs:25-28,112-143,212-290`：主队列 64/3 workers，低优先级队列 8/1 worker，`try_send` 饱和即 Busy，reservation 覆盖 queued+running。
- `crates/opentake-media/src/thumbnail/mod.rs:33-35,59-78`、`src-tauri/src/media.rs:3418-3525`：时间线 sprite 最多 240 个代表性采样，逐帧 checkpoint，部分产物与最终产物在 epoch guard 下发布。
- `src-tauri/src/media.rs:112-156,4470-4621` 与 `crates/opentake-media/src/proxy.rs:70-213`：proxy state 单飞；取消 kill+wait；partial 文件清除；探测通过且源 hash 未变化后才 rename。

### 4. asset HTTP 有界性

`src-tauri/src/safe_asset_protocol.rs:38-48,77-131,390-461`：并发 4、pending 32、全局 helper 进程槽 4、5 秒 deadline；超载返回 503/Retry-After，排队超时 504；Range 单次 1000 KiB；普通非 Range 32 MiB、图片 128 MiB，超过要求客户端使用 range。

## 缺陷与建议

### B-01 — HIGH / P1：素材卡波形请求无界 fan-out

复现路径：创建含大量未缓存音频的项目 → 进入网格素材/音频视图 → 所有 `items.map` 卡片同时挂载 → 每卡直接 `get_waveform` → 每个 cache miss 独立 FFmpeg PCM 解码。组件卸载只忽略前端结果，不能停止 FFmpeg。

证据：`MediaPanel.tsx:945-995,2328-2350`；`media.rs:3601-3628`；`waveform/mod.rs:84-103`。算法测试通过不覆盖这一跨层并发行为。

建议：所有 waveform/poster/搜索缩略图接入一个 project-epoch 资源协调器；按类型设置 2–4 个 worker、有限 pending、per-cache-key promise/reservation 合并、视口 admission 与卸载/项目切换取消。增加 100 个音频卡的 E2E，断言 FFmpeg 峰值和 pending 上限。

### B-02 — MEDIUM / P2：搜索/快速预览绕过 bounded prewarm

复现路径 A：冷缓存、已有视觉/语音索引时搜索，一次可挂载 Moments 20 + Spoken 20 个 `HitThumbnail`，各自直接解码。路径 B：快速切换多个冷视频，旧 `previewPoster` 只被前端忽略，后台 1080p 解码继续；同时近视口 prewarm 可能解码同一 cache key。

建议：`HitThumbnail` 和 preview 先通过 scheduler/reservation 请求；预览使用 latest-wins token；直接写缓存改为 epoch-safe staged rename，避免并发写同一 `.preview.png`。

### B-03 — MEDIUM / P2：公开导入命令同步且缺少完整取消/上限

显式 `import_media` 无输入条数/聚合字节上限，probe 无 deadline；目录导入虽有硬上限和单 probe 15 秒，但公开路径传 `cancel=None`。最坏情况下用户无法停止长时扫描/探测，命令结束前仍可能额外等待 poster 调度 2 秒。

建议：把两种导入统一为可取消异步 job，限制显式路径数/总字节，所有 probe 使用 retained handle + hard deadline；提供阶段/进度和 cancel command，保持现有一次性持久化提交语义。

### B-04 — MEDIUM / P2：项目媒体 asset 权限跨项目持久累积

`f9398a9` 使用精确文件，未扩目录；但每次自动 `allow_file` 都被 persisted-scope 保存，项目切换不清理旧外部媒体。长期使用会扩大 WebView 可请求的历史媒体集合。

建议：在自定义 `opentake-asset` 协议增加“当前项目 active media set”二次门禁，项目 transition 原子替换；静态 app/cache/resource scope 仍沿用 Tauri。这样无需永久 deny 以后可能再次使用的外部文件。增加 A→B 切换测试：A 文件在 B 激活后 403，B 文件可读；重启后仍成立。

### B-05 — LOW / P3：prewarm 文档与实现漂移

`src-tauri/src/media/prewarm.rs:3` 仍写“三 workers + 24-job queue”，实现实际是主队列 64/3 workers，外加低优先级队列 8/1 worker。建议同步注释和模块文档，避免容量评审误判。

### B-06 — MEDIUM / P2（验证缺口）：proxy 无真实转码回归测试

`cargo test -p opentake-media proxy::tests` 退出 0 但运行 0 个测试。静态实现具备 single-flight/cancel/partial cleanup，前端测试仅覆盖 IPC 映射，不能证明 FFmpeg 真实转码、取消回收和失败无残留。

建议：用 1–2 秒视频 fixture 覆盖成功、取消、源变更、目标冲突、无视频轨、partial 清理与输出 ffprobe；在 Tauri 层补 single-flight Busy、项目切换取消及 manifest 原子回滚。

## GUI / 资源实测边界

- 2026-08-07 19:01:28 +0800 检查 `/Volumes/mac`：`ABSENT`。历史项目外部源当前不可物化，不能用于重开后预览/缩略图正向验证。
- 本 Agent 未启动 OpenTake 或 safe-asset-helper。曾对既有单实例做关闭文件对话框的清理操作；19:00 后按主审计 Agent 要求完全停止 GUI。
- 因主审计 Agent 正在独占 `/Applications/OpenTake.app` 做资源测量，本 Agent 没有再启动独立实例或采集 CPU/RSS/FFmpeg 峰值。B-01 的动态数值、重开正向显示与 proxy 真转码因此是 BLOCKED，而不是 PASS。
- 自动化测试的 `/usr/bin/time -lp` RSS 是 Cargo/测试进程整体峰值，只用于命令复核，不代表应用运行时资源指标。

完整命令、时间、退出码与 pass/fail 数见 [command-ledger.md](command-ledger.md)。
