# OpenTake 长媒体预览与播放交接计划

> 交接范围：2026-07-18 用户连续报告的长视频导入、时间线缩略图、拖动播放头、暂停恢复、多轨合成黑屏问题。下一位 Agent 必须先复现，再按 TDD 实现，并在提交前交给独立 review agent 审查。

## 1. 当前事实基线

- 工作仓库：`/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence`
- 交接前分支：`audit/opentake-completion-20260714`
- 交接前提交：`bde1c70bee336e818329a7927dc5ea635de30514`
- 云端 `origin/main` 基线：`acf07e5`
- 真实复现工程：`~/Documents/OpenTake/未命名.opentake/Untitled.opentake`
- 真实重素材：`Aroll-气口剪辑完成素材.mov`，3840×2160、HEVC Main10、约 2.5 GB、210.71 秒。
- 已确认没有打开的 GitHub PR。

## 2. 用户报告的问题总表

### P0-A：拖动播放头后，后续操作持续变卡

现象：拖动时间线进度条/播放头后，继续播放或继续编辑会变卡，不是只在拖动瞬间卡。

验收标准：

- 在真实 4K Main10 工程中连续快速 scrub 20 次，松开后 250 ms 内能恢复交互和播放。
- scrub 结束后连续播放 30 秒，帧发布速度和 CPU/内存占用不随 scrub 次数持续恶化。
- 任意时刻最多保留一个当前 seek/composite 请求；旧 project epoch、timeline version、session 和 seek generation 的结果必须丢弃。
- scrub 过程中保留最后一张成功帧；松开、暂停、恢复时不得出现黑屏。
- 验证没有遗留 FFmpeg 子进程、解码 worker、定时器或未清空的合成请求队列。

优先检查：

- `web/src/components/preview/TimelinePlaybackLayer.tsx`
- `web/src/components/preview/nativePlaybackSession.ts`
- `web/src/components/preview/RustFrameBuffer.tsx`
- `web/src/store/uiStore.ts`
- `src-tauri/src/playback/commands.rs`
- `src-tauri/src/playback/engine.rs`
- `src-tauri/src/playback/resolver.rs`

需要先写的回归测试：快速连续 seek 只提交最后一次；旧 seek 完成不能覆盖新帧；pointer-up 只触发一次最终精确 seek；多次 scrub 后 worker/请求数量回到基线。

### P0-B：底部时间线缩略图只显示第一张，不显示后续画面

现象：预览图片/视频时，底部线条框中的缩略图序列重复第一张 poster，没有按素材时间显示后续缩略图。

必须同时满足的目标：

- 素材落下后立即显示 poster，不阻塞导入、拖动、播放、暂停或合成。
- 后续 filmstrip/sprite 必须展示不同时间点，时间映射与 clip trim、speed、source fps 一致。
- 缓存未命中时不得在前端交互路径同步执行多次随机 seek。
- sprite 生成进入有界后台调度器；播放/scrub 优先，后台任务可取消、可按 project epoch 失效、可复用缓存。
- 前端读取缓存状态或接收渐进结果，不能等待整张 sprite 完成才恢复 UI。

优先检查：

- `web/src/components/timeline/TimelineContainer.tsx`
- `web/src/components/timeline/clipRenderer.ts`
- `src-tauri/src/media.rs` 中 `generate_thumbnail`、`video_sprite`、`PrewarmScheduler`
- `crates/opentake-media/src/decode/frame.rs`
- `docs/specs/media/3-thumbnails.md`
- 上游 `palmier-pro-upstream/.../Timeline/MediaVisualCache.swift`

重要约束：曾验证过“完全取消自动 sprite、只保留 poster”可以停止长视频随机解码，但它会直接造成用户现在报告的 filmstrip 退化，因此该方案已经撤回，不能作为最终修复。正确方向是后台、可取消、缓存优先的渐进 sprite。

### P0-C：长视频导入后长时间转圈，期间合成、暂停恢复异常

代码证据：素材导入本身主要做 ffprobe 和 manifest 写入；真正的重负载来自素材进入时间线后自动调用 `generateThumbnail(includeSprite: true)`，当前最多 24 个采样点，每个采样点可能单独启动一次 FFmpeg 随机 seek。该同步 Tauri 命令与播放/合成竞争资源。

验收标准：

- 导入 2.5 GB Main10 素材时，素材登记完成后 UI 立即可用。
- 后台缩略图生成不得阻塞 `composite_frame`、play、pause、resume、seek。
- 播放开始或 scrub 时，低优先级 sprite 工作必须让路或取消。
- 导入/预热状态必须区分 poster ready、sprite queued、sprite partial、sprite cached、failed/cancelled。

### P0-D：两个合成片段之间偶发黑屏；暂停后恢复会黑一下并卡顿

已完成但需要安装包真机复验的补丁：`RustFrameBuffer` 使用隐藏 `<img>` 解码，把成功帧绘制到单一稳定 canvas；只有 canvas 绘制成功后才晋升活动槽，首个实时帧成功前保留暂停合成帧。context 为空或 `drawImage` 抛错时保留旧活动帧/paused still。

仍需验证：

- 上层片段结束、只剩下层轨道时画面持续可见。
- 两个相邻片段交界前后逐帧非黑。
- pause → resume 连续 20 次无黑闪、无陈旧帧、无累积卡顿。
- WebKit 窗口实际合成路径，而不只是 happy-dom 或浏览器 fallback。

### P1：此前同一问题链中已处理、仍需回归的项目

- 长片段拖动时，时间线只绘制视口可见的缩略图/波形，避免按整条长片段宽度重绘。
- 素材拖动使用自定义缩略图 drag image，不把文件名、时长等字符一起拖下来。
- 普通视频时间线预览统一走原生多轨合成，避免上轨结束后下轨不可见。
- 真实 HEVC Main10 连续解码和 GPU 合成未发现绿色污染；WebKit 呈现层仍需安装包视觉证据。

这些改动位于提交 `db3527b`，不能只凭单测判定完成，必须在最终安装包中复验。

## 3. 当前测试与证据

- Web：67 个测试文件、679 项测试通过。
- Rust：`cargo test --workspace` 通过。
- Web production build 通过；仅保留既有 dynamic-import/chunk-size 警告。
- `cargo fmt --all -- --check` 与 `git diff --check` 通过。
- 真实 Main10 FFmpeg 连续解码 90 帧通过，无绿色像素区域。
- 真实 Main10 GPU playback probe 通过：约 62–63 帧发布，playhead=90，整段 `min_nonblack` 约 0.946–0.970，`max_neon_green=0.000`。
- 独立 review agent 最终结论：当前 canvas/Main10 测试补丁 APPROVE，无 HIGH/MEDIUM/LOW 发现。
- macOS 应用包构建成功，但最终打开真实工程时出现系统“文稿文件夹访问”授权弹窗；交接时未替用户授权，已关闭本次候选进程。因此安装包视觉验收仍未完成。

## 4. 推荐实现顺序

1. 先为 scrub 后持续卡顿补观测与失败测试，确认是请求堆积、解码 worker 未取消、还是播放会话重复创建。
2. 实现 newest-wins seek/composite 协调：旧 generation 取消并拒绝发布，pointer-up 只执行一次最终精确 seek。
3. 把 sprite 生成从交互命令迁入 `PrewarmScheduler` 或同等有界后台队列；增加 cache-only 查询和渐进状态。
4. 让 playback/scrub 能暂停或取消低优先级 sprite 工作，再恢复后台预热。
5. 保留稳定 canvas 补丁，补相邻片段、多轨交界、pause/resume 的真实像素测试。
6. 全量单测、真实 Main10 probes、release build、独立审查、安装包真机循环，最后再提交/发布。

## 5. 禁止的“快速修复”

- 不得永久关闭 filmstrip，只重复 poster。
- 不得用文件大小、编码格式或轨道编号猜测是否走原生合成。
- 不得在每次 pointermove 启动新的不可取消 seek/composite。
- 不得在新帧真正画出前清空旧帧。
- 不得用浏览器 fallback 成功推断 Tauri/WebKit 真机成功。
- 不得降低像素正确性断言来让性能探针通过；吞吐基准与画面正确性探针必须分开。

## 6. 给下一位 AI 的提示词

```text
你接手 OpenTake 长媒体预览与播放问题。仓库是：
/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence

先完整阅读：
docs/superpowers/plans/2026-07-18-long-media-playback-handoff.md
以及仓库 AGENTS.md、CLAUDE.md、docs/specs/media/3-thumbnails.md、docs/architecture/PLAYBACK-ENGINE.md。

必须先检查 git status、当前 main SHA 和现有 diff，不要覆盖任何未提交内容。使用 systematic debugging + TDD + Karpathy Guidelines。先复现再修改，最终代码必须交给一个独立 review agent 审查。

目标只有两个 P0，但必须同时回归交接文档中的已知问题：
1. 连续快速拖动播放头后，应用后续播放/编辑持续变卡。找出并修复请求堆积、seek/composite 取消、解码 worker 或播放会话生命周期问题。要求 newest-wins，旧 project/timeline/session/seek generation 永不发布；松开后 250 ms 内恢复，连续 20 次 scrub 后资源数量回到基线，无黑屏。
2. 底部时间线 filmstrip 只重复第一张 poster。必须恢复不同时间点缩略图，但不能在导入/交互路径同步执行 24 次随机 FFmpeg seek。把 sprite 放进有界、低优先级、可取消、project-epoch 安全的后台调度；poster 立即显示，sprite 渐进或缓存命中后升级；播放/scrub 必须优先。

同时验证：长视频导入不长时间阻塞；相邻合成片段之间无黑屏；pause/resume 无黑闪和累积卡顿；上轨结束后下轨仍显示；长片段拖动不卡且拖拽预览不带文字。

真实工程：~/Documents/OpenTake/未命名.opentake/Untitled.opentake
真实 Main10 素材：Aroll-气口剪辑完成素材.mov（3840×2160、HEVC Main10、约 2.5 GB）。从工程 media.json 获取绝对路径，不要把用户路径硬编码进产品代码。

强制验证：focused regressions → 全部 Web 测试 → cargo test --workspace → fmt/check/build → Main10 FFmpeg 90 帧 probe → Main10 GPU whole-run pixel probe → release app build → Tauri/WebKit 真机反复 scrub、pause/resume、相邻片段与多轨视觉验收。浏览器 fallback 不能代替真机。每项只报告实际运行结果；失败不能伪装成通过。
```
