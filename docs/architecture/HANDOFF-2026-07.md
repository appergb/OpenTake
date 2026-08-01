# 交接 · 未完成工作规划(2026-07-04)

## 2026-07-10/11 Wave 1A reviewed addendum

This dated addendum supersedes the **current-status** playback and real-device
claims in §0, §1, and §3.1 below. Those sections remain intact as the
2026-07-04 historical snapshot; they are no longer instructions to enable a
default-off Rust renderer or to perform the then-missing first device check.

- Playback capability routing is now the sole authority. Plain media routes to
  WebKit; timelines needing text/color-grade/chroma-key/supported-mask
  compositing route to Rust when available and enabled; Lottie, enabled generic
  effects, polygon/overflow masks, and compositing combined with reverse or any
  non-unit speed are `unsupported` and fail closed.
- Rust publication is exact and session-scoped:
  `{projectEpoch, timelineVersion, sessionId, frame, sequence}` selects a finite
  `/frame` JPEG. It is not a long-lived `<img src=MJPEG>` transport.
  `PublicationGate`, bounded preparation/reaping, exact cold bootstrap,
  identity-scoped control, and the retained two-slot frame handoff prevent late
  work from replacing a newer visible frame. The historical Promise-tail
  interrupt was superseded by the reviewed identity/cancellation controller;
  pause/stop/project boundaries still interrupt obsolete startup before reap.
- Media prewarm uses a bounded 24-job, three-worker scheduler. Project epoch,
  media id/kind/source, and canonical source identity gate poster, preview,
  waveform, and timeline-visual cache admission/publication, including the
  same-ID/different-path case.
- Exact detached-bundle QA passed for Task 6.2 (`dc83284319bd…`) and the final
  Task 6.3 bundle (`1f2bf4e49877…`). These are distinct from the older
  `/Applications/OpenTake.app`; their hashes must not be conflated. See
  [2026-07-10 playback/cache QA](../superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md).
- Still not yet complete: installed-app export UI artifact verification,
  Windows transport/CSP/sidecar validation and security hardening, signed and
  notarized packaging, and renderer support for the fail-closed capability set.

> **本文档是当前权威 TODO / 交接文档**:哪些做完了、哪些 issue 该关、剩下什么没写完、每一项该怎么写。
> 上级:[架构目录](INDEX.md)。历史差距文档 [PORT-1TO1-GAP.md](PORT-1TO1-GAP.md)(已过时)与 2026-07 初审计相比,本文以 **main = `ace3236`(PR #188)+ PR #189** 为基线重新核对过代码,结论以本文为准。
> 逐项核对方式:grep/读码验证,非凭记忆;每项附证据(commit / 文件路径)。

---

## 0. 一页现状

**编辑内核、播放引擎、媒体管线主干已全部完成。** 2026-06 下旬至 07-04 合并的 #170–#189 把此前审计的"引擎建成没接 UI"主缺口基本清完:

| 已完成(证据) | |
|---|---|
| 流式播放引擎:连续解码→wgpu 合成→MJPEG + cpal 主时钟 | PR #170(#53/#63/#64/#160 立体声/#162) |
| **Rust 引擎默认开启 + 运行时回退** | **PR #189(在途,本次提交)** |
| whisper 转写端到端 + 字幕生成(Captions tab / add_captions) | PR #183 / #184 |
| SigLIP2 语义搜索接线(Moments UI / search_media) | PR #185 |
| .opentake 自包含打包导出 | PR #180 |
| 导入白名单扩宽 + 不支持提示 + 首帧海报 | PR #181 |
| save-as 复制 media/ + 项目缩略图 + XMEML 真实时码 | PR #179 |
| Inspector 关键帧编辑/钻石键/Reset/Source 检查器/截帧入库 | PR #173 #177 #187 |
| 画布 Transform/Crop overlay + zoom/pan + Aspect/Quality 菜单 | PR #176 #178 #188 |
| 时间线:标入出范围/间隙选择/ripple 插入/键盘 nudge | PR #186 |
| H.265/ProRes 导出 + 进度/取消 | PR #172 |
| agent MediaBridge(inspect_timeline/import_media 去 stub) | PR #182 |

**剩余工作集中在四块**(§2 详述):① 媒体库管理重写(#91,唯一 CRITICAL);② AI 面板两大件(Agent 聊天、生成 UI);③ 用户 2026-07-01 加单的五项(倒放/冻结帧/HDR/代理媒体/账号脚手架);④ 一批收尾接线。

---

## 1. PR #189(本次提交):Rust 引擎默认开

- `playback-engine` 进默认 cargo feature;`--no-default-features` 保留最小构建。
- 前端 `rustEngineEnabled()` 默认 ON,仅 localStorage `opentake.rustEngine="0"` 退回 legacy `<video>`。
- **运行时回退安全网**:`playback_start` 拒绝或 2s 内无 `playback_frame` → 自动降级 legacy + toast,下次播放自动重试引擎(`rustEngineFailed` 状态,纯函数 `shouldFallBackToLegacy` 有单测)。
- 本地 CI 六项全绿(fmt / clippy 两态 / workspace test 含 GPU 集成 / web build / 437 vitest)。
- ⚠️ **唯一欠账:真机视觉验收未做**(computer-use 被 Dock 拦截)。清单见 [PLAYBACK-ENGINE.md](PLAYBACK-ENGINE.md)。若真机翻车,运行时回退保证退化为 #170 之前的行为而非黑屏;届时按清单逐项排查。

---

## 2. Issue 盘点(2026-07-04 逐项核对)

### 2.1 已完成 → 本次关闭(14 个)

| Issue | 证据 |
|---|---|
| #24 缩略图条/波形/红洗/徽标 | `clipRenderer.ts:drawFilmstrip()`;波形走 ffmpeg `extract_pcm`;红洗(B3 `183e270`);offset 角标(#87) |
| #38 后台运行+自动保存 | `web/src/hooks/useAutosave.ts`(1500ms 防抖)+ 退出 flush;关窗常驻已真机实测 |
| #47 合成预览+播放 | `composite_frame` + PR #170 播放引擎 |
| #63 cpal 音频 | PR #170(`bb44647`/`dd42eac`) |
| #64 MJPEG 传输 | PR #170(`5227ccb` 含 HTTP 集成测试) |
| #83 音量橡皮筋 | `clipRenderer.ts:drawVolumeEnvelope()` + `hitTest.ts`(`28a218c`) |
| #92 scrub 逐帧合成卡死 | 单表面重写后 scrub 走 `<video>` 实时 seek,逐帧 ffmpeg+wgpu 已移除 |
| #99 吸附迟滞/多探针/角标/橡皮筋 | `28a218c` + #87 |
| #100 真实播放引擎 | 与 #53 同义,PR #170 |
| #125 合成帧不居中 | `previewLayerStyles.ts:aspectFitBox()`(`1df121c`) |
| #142 剪辑套件失效总根因 | IPC camelCase 已修 + 播放/预览已按上游重写 |
| #145 fade knee 拖拽 | `hitTest.ts:fadeKneeHit()` + `applyFadeKneeDrag` + 绘制 + 测试 |
| #151 播放引擎整体失效 | 按 #53 单表面流式引擎重写完成(#170/#189) |
| #162 播放态中途 seek | `70e376b`(PR #170) |

**#53 本体**在 PR #189 合并后关闭(引擎全链路已完,#189 是"默认开"收尾)。

### 2.2 部分完成 → 留开、范围收窄(8 个)

| Issue | 已做 | 剩余 |
|---|---|---|
| #13 进阶能力 | 调色链/绿幕/蒙版着色器(`8813df2`)、首个交叉溶解转场竖切(§3.9) | 扩展转场库、AI 推理(#27)、音频工程(#28) |
| #25 Inspector/MediaPanel tab | 关键帧钻石(#187)、Text MVP(#107)、Captions(#184) | AI-Edit tab、Music tab(§3.6) |
| #29 进阶纯逻辑 | SRT/VTT 已接 UI/Tauri 命令(#110)、字幕批量样式纯逻辑(#113) | 字幕批量样式接 UI/命令；曲线变速(§3.10)、多机位、复合片段 |
| #37 全局素材库 | 后端+前端全量(#104/#106/#115) | 库→时间线拖拽、媒体面板星标迁 `library_favorite` |
| #48 片段编辑收尾 | 右键菜单(Copy/Swap 已接)、nudge/间隙/ripple(#186) | **Save as Media 仅前端菜单占位,后端零实现**(§3.5) |
| #131 编解码/无缝切换 | 播放路径已被流式引擎取代 | ffmpeg 未随包分发(§3.12) |
| #147 Clip 上游字段 | `link_group_id` + 前端角标(#87) | `is_soloed`(solo 独奏)未做 |
| #160 流式音频 | 立体声(`dd42eac`) | 分块/后台填充解码(现为一次性全时间线预混,§3.11) |

### 2.3 未开工 → 留开(9 个)

**#91(CRITICAL,媒体管理重写,含 #49/#58 文件夹)、#27、#28、#30、#34/#65(Lottie)、#148(布局常量)、#161(CSP 加固,需真机)。**

---

## 3. TODO:没写完的内容 + 每项怎么写

> 排序 = 建议施工顺序。每项给【现状】【怎么写】【验收】。移植铁律不变:整数帧、`secondsToFrame` 截断、serde 全 `#[serde(default)]`、IPC 多词字段 camelCase 三边同步(Rust DTO / `web/src/lib/types.ts` / 调用处)。

### 3.1 【P0】PR #189 真机验收 + 合并

- **现状**:代码完毕、CI 在跑;真机视觉验收因 Dock 拦截 computer-use 未做。
- **怎么写**:`./web/node_modules/.bin/tauri build` → `cp -R target/release/bundle/macos/OpenTake.app /Applications/ && open -a OpenTake` → 按 [PLAYBACK-ENGINE.md](PLAYBACK-ENGINE.md) 验收清单:多轨/高码率不卡、ProRes 可放、调色/特效/多轨叠加在播放中可见、A/V 同步(左右声道)、scrub/暂停行为不变、**引擎故障时 2s 内降级 legacy 且有 toast**。
- **验收**:清单全绿;翻车项按 `rustEngineFailed` 日志定位。

### 3.2 【P0】#91 媒体库管理重写(含 #49/#58)

唯一 CRITICAL。issue 正文已有完整方案与文件清单,要点:
- **现状**:文件夹拍平、素材偶发重复显示、音频 tab 只是 `type==='audio'` 过滤、星标在 localStorage、音频卡片无波形。
- **怎么写**(按 #91 正文,权威参照剪映):
  1. 后端:`opentake-domain/src/media.rs` 的 `MediaFolder` 树 + `import_folder` 镜像目录结构(勿拍平);`MediaListDto` 带 folderId 层级;查重去重修数据流(重复显示根因在 mediaStore 聚合)。
  2. 前端:`MediaPanel.tsx` 文件夹钻取 + 面包屑;左侧分区(导入/我的/AI 生成);音频 tab = 提取音频入口 + 收藏 + 音效库(音效库后端 #115 已有);音频卡片波形缩略图(复用 `get_waveform`)。
  3. 星标迁移:`favorites.ts`(localStorage)→ `library_favorite`/`library_list`(#106 后端已在),含一次性迁移逻辑。
  4. 吸收 #37 尾巴:库视图→时间线拖拽(拖拽协议对齐 MediaPanel 现有 onDragStart)。
- **验收**:导入嵌套文件夹结构保持;切 tab 无重复;星标重启后仍在(后端);库素材可直接拖上时间线。注意与 `relink_media`/missing 覆盖层不回归。

### 3.3 【P1】Agent 聊天面板(前端占位 27 行,后端无 chat 模块)

- **现状**:`web/src/components/agent/AgentPanel.tsx` 仅渲染占位文案;`crates/opentake-agent/src/` 只有 mcp/plugin/prompt/signal/tools,**无会话循环**。
- **怎么写**(对照上游 `Sources/PalmierPro/` 的 Agent 聊天 UI 结构):
  1. 后端 `crates/opentake-agent/src/chat/`:会话结构(消息历史 + streaming)+ LLM 调用走 `opentake-gen` 的 provider/keys 基建(BYOK,钥匙串已有);工具调用直接复用统一 dispatch(基础集合最多 39 个、按主机能力过滤，另有 4 个动态生成工具；45 个兼容线名保留在 `KNOWN`)。
  2. `src-tauri` 命令:`chat_send`/`chat_history`/`chat_cancel` + `chat_delta` 事件流(参照 `transcribe` 的进度事件模式)。
  3. 前端:AgentPanel 消息列表 + streaming 渲染 + 工具调用卡片;Context Signal(`signal/`)注入系统提示已有,接上即可。
- **验收**:面板内发"把这段静音剪掉"能走 tighten_silences 工具链并落 EditCommand;无 key 时有引导文案。

### 3.4 【P1，代码竖切已完成】生成式 UI + `generate_*`/`upscale` 去 stub(#30 切片)

- **现状(2026-07-29)**:`GenerateVideo/Image/Audio | UpscaleMedia` 已通过 MCP/Chat 共用的 Tauri GenerationBridge 接入 fal/Replicate/OpenAI/ElevenLabs；先持久化占位和 generation log，再异步上传/提交/轮询/下载/探测/导入。`canGenerate` 由可用凭据派生，四工具仅在能力可用时动态发布。MediaPanel 卡片显示进度、取消、固定错误码与需重新确认成本的重试。
- **怎么写**:
  1. `src-tauri` 命令层:`generate_start(params)->job_id` / `generate_status` / `generate_cancel`,内部 `opentake-gen::client` 异步 job 轮询,产物落项目 `media/` 并走 `import_media` 现有通路(保证出现在素材面板 + 可撤销)。
  2. MCP dispatch 同一命令复用,去掉 4 个 stub;canGenerate 改为"有 BYOK key 即 true"(`keys.rs` 可查)。
  3. 前端:对照上游 GenerationView 做 MediaPanel「AI 生成」分区(与 #91 的左侧分区合流):模型选择(`catalog/` 已有 list_models)、提示词、参考图、进度卡片。
- **验收状态**:无付费网络的配置 provider 生产路径已覆盖 image/video/audio/upscale、N 图顺序、部分/全部失败、取消、恢复、鉴权/限流、重试和精确 2x；完整工作区测试通过。真实账号付费请求保留到 Beta 实机验证清单，未在自动化中消费用户额度。

### 3.5 【P1】Save as Media(#48 尾巴)

- **现状**:右键菜单项存在(`ClipContextMenu.tsx` + i18n),**后端零实现**。
- **怎么写**:参照导出编排(`src-tauri/src/export.rs`)做单 clip/标记范围的**局部渲染**:`export_range(track?, in, out) -> mp4/wav` 落项目 `media/`,再 `import_media` 入库;菜单项接 `edit.saveClipAsMedia()`。标记范围版本喂 #186 已有的 I/O 范围。
- **验收**:对一个带调色+文字的 clip 执行,产物素材回拖时间线画面一致(同一 RenderPlan 像素路径)。

### 3.6 【P1】Inspector AI-Edit tab + MediaPanel Music tab(#25 尾巴)

- **怎么写**:AI-Edit tab 列出 agent 自动化建议(`signal/` 的 Context Signal + `EDITING-AUTOMATION` 的 beat/silence 工具已可产建议,UI 呈现 + 一键应用);Music tab 依赖生成 UI(§3.4)的音频生成 + 音效库(#115 已有)聚合。上游对照 frontend-UI-1to1-SPEC §6/§7。

### 3.7 【P1·用户加单】倒放 / 冻结帧(上游没有,用户 2026-07-01 明确要)

- **倒放**:`opentake-domain::Clip` 加 `reversed: bool`(`#[serde(default)]`);`EditCommand::SetClipProperties` 带上;**解码侧**在 `opentake-media` 流式解码按 `reversed` 反向映射 `source_frame_index`(v1 可用 ffmpeg `reverse` filter 预生成反向代理段,避免逐帧倒退 seek 的性能坑);导出走同一 RenderPlan 映射。前端右键菜单加"倒放"。
- **冻结帧**:**复用 #187 截帧入库**:`FreezeFrame(clip, at_frame)` = 截当前帧成图片素材 + `SplitClip` + 中插图片 clip(时长可调),打包成一个 EditCommand 复合事务(undo 一步回退)。
- **验收**:倒放 clip 播放/导出帧序一致;冻结帧 undo 一步还原。

### 3.8 【P1·用户加单】HDR v1 / 代理媒体 / 账号脚手架

- **功能完成状态(2026-08-01)**:三个独立 child 均已通过，并由 `crates/opentake-render/tests/composite_acceptance.rs#hdr_proxy_account_children_close_one_composite_acceptance` 关闭同一个父验收。
- **HDR v1 PASS**:`MediaColorMetadata` 持久化 primaries/transfer/matrix/range；PQ/HLG 在单帧、流式预览和导出共用路径进入 BT.709 SDR。运行时按实际 FFmpeg filter 能力选择 `scale_vt` 或 `zscale`，已修复 bundled sidecar 导出全黑。当前是明确的 SDR delivery policy，不声称 HDR passthrough。
- **代理媒体 PASS**:项目内 `media/proxies/<uuid>.mp4` 以 H.264/AAC 原子生成并记录原件 SHA-256；设置开关驱动播放 resolver，导出只读原件。创建、目录祖先、授权、移除和重连均使用 exact-leaf/no-follow 约束，拒绝符号链接/Windows reparse；项目切换会取消在途转码，移除/重连在旧代理文件清理完成前持续持有项目身份租约。打包 GUI 已完成开关、播放、移除、重建、保存重开与原件导出验证。
- **账号脚手架 PASS**:设置页如实标注无官方后端；远端只接受 HTTPS，本机回环 HTTP 仅作开发例外；token 只进钥匙串。失败登录和断网时，播放、编辑、保存与导出不被账号状态门控。
- **证据与限制**:[`hdr-proxy-account-real-device-2026-08-01.md`](../audit/2026-07-14/runtime-artifacts/automated/hdr-proxy-account-real-device-2026-08-01.md)。最新本地 `.app`/DMG 仅为 ad-hoc 签名且未 notarize，不能作为 Developer ID Beta 发布证据；完成度账本仍需在不覆盖用户改动的前提下重建文件清单。

### 3.9 【P2】转场(#13 切片,对标剪映)

- **完成状态(2026-07-31)**:cross-dissolve 首个竖切已打通。`opentake-domain::Transition` 持久化双方 clip ID、类型和时长；`SetTransition` 校验同轨、视觉源、直接相邻和最大 handle，过长请求显式失败。素材面板转场页支持添加/调时长/删除，操作进入统一 undo/redo，时间线接缝显示标记。RenderPlan 在转场区保持出片完整底层、以进度 alpha-over 入片，消除了旧双透明度造成的中点变暗；拥有测试 `crates/opentake-render/tests/transitions.rs#adjacent_clip_transition_is_editable_undoable_and_matches_preview_export` 覆盖保存重开、拒绝/恢复和四个关键帧的像素/预览导出一致性。后续工作仅是扩展 wipe/slide/3D 等转场库，不再属于首个竖切缺口。

### 3.10 【P2】曲线变速(#29 切片)

- **怎么写**:`Clip.speed: f64` → 增加 `speed_curve: Option<Vec<SpeedPoint>>`(保持标量字段兼容旧工程,`#[serde(default)]`);`source_frame_index` 改为对曲线积分查表(预计算累积映射,整数帧);波形/缩略图条同步拉伸。**动全链路的帧映射,单独分支 + 全量回归 `cargo test -p opentake-ops`。**

### 3.11 【P2】流式音频分块解码(#160 剩余)

- **完成状态(2026-08-01) PASS**:播放以 2 秒窗口解码、4 窗口有界队列后台填充；cpal 回调只做非阻塞读取，underrun 输出静音且音频主时钟继续前进。seek 递增 generation、取消在途解码并丢弃旧块；pause 保留当前 generation 的下一块，resume 从同一音频时钟继续。停止会取消并回收解码、音频和协调线程。
- **导出与另存 PASS**:视频导出逐窗口混音并把 PCM 直接追加到 encoder 私有文件 spool，完成时再 mux，不保留全时间线 `Vec`；音频片段另存 WAV 也逐窗口写文件。60 秒打包 GUI 时间线完成跨窗口播放、暂停/继续、首尾跳转后继续、完整 H.264/AAC 导出和 8 秒 WAV 另存。自动化覆盖长时间线恒定峰值分配、短参考一致性、取消、underrun、暂停不吞块及增量 spool。证据见 [`bounded-audio-streaming-real-device-2026-08-01.md`](../audit/2026-07-14/runtime-artifacts/automated/bounded-audio-streaming-real-device-2026-08-01.md)。

### 3.12 【P2】收尾杂项

| 项 | 怎么写 |
|---|---|
| Lottie 接线(#34/#65) | `opentake-motion` 实现已在但 src-tauri 未依赖;评估 wgpu 直渲 vs 预烘焙 PNG 序列(后者先通:motion→PNG 序列→图片 clip) |
| ffmpeg 随包(#131) | tauri sidecar 打包 ffmpeg/ffprobe,`OPENTAKE_FFMPEG` 已支持自定义路径,只差 bundle 配置 + 许可证说明(GPL 兼容) |
| solo(#147) | `Clip.is_soloed` + 混音/合成时非 solo 轨静音/隐藏;前端轨头加 S 按钮 |
| 布局常量(#148) | 对照上游 rulerHeight/dropZoneHeight/trackHeight 改 `theme.ts`,纯数值对齐 |
| CSP 加固(#161) | null→非 null 白屏高风险,**必须真机逐项验证**,独立小 PR |
| Storage/Models 设置页 | `SettingsView.tsx` 加两 pane:模型缓存管理(whisper/SigLIP 已下载模型列表+删除)、存储占用(项目/缓存目录大小) |
| MCP 动态能力 | `InspectMedia` 已接 MediaBridge（剩 Lottie）；生成/超分按凭据动态发布；Motion add/edit 于 2026-08-01 接生产桥并按 Chromium/FFmpeg 能力动态发布 |
| 技术债 | `fcpxml.rs` 1489 行拆分;`export_fcpxml` 名实不符(产物 XMEML)可改名 `export_xml`;`library.rs:322` remove 静默吞错补 `tracing::warn!` |

---

## 4. 工程纪律(不变,踩坑总结)

1. 推送前本地五项:`cargo fmt --all` + `clippy --workspace -D warnings` + `test --workspace` + `pnpm -C web build` + `pnpm -C web test`;现另加 `clippy -p opentake-tauri --no-default-features`(#189 起 CI 有此步)。
2. 合并前 `gh run watch <id> --exit-status` 双作业绿;`gh pr merge --merge --admin --delete-branch`。
3. IPC 多词字段 camelCase 三边同步;静默失效先加 try/catch 暴露。
4. 大任务用 `git worktree` 隔离;提交只 `git add` 显式路径,绝不 `-A`;动手前先 `git log`/`gh pr list` 查并发协作者是否已做(#53 曾重复实现)。
5. 别 bundle 多个有 bug 的 PR;一 PR 一事。

---

**2026-07-14 完成度审计校准：** 本文中的状态声明已逐项归入 `docs/audit/2026-07-14/requirements.json`；当前裁决、证据与验收条件以同目录 `document-reconciliation.md` 为准，本文保留为交接时点记录。

**当前有效条目的 ledger 绑定：**

| 原始行 | Candidate ID | Gap group |
|---:|---|---|
| 126 | `doc-72fd9278412a314f` | media-library |
| 146 | `doc-fda8c3be3dfa4161` | agent-settings-generation |
| 151 | `doc-e4d48925632767a2` | agent-settings-generation |
| 161 | `doc-02ff53bbfaf2f4ac` | inspector-text-keyframes |
| 171 | `doc-74145abee768207c` | media-render-playback-export |
| 178 | `doc-2655a5de61309700` | media-render-playback-export |
| 182 | `doc-43c0111d8d467789` | inspector-text-keyframes |
| 186 | `doc-cf47d7775c7e1947` | media-render-playback-export |
| 191 | `doc-793e43b6ec894755` | media-render-playback-export |
| 201 | `doc-b7b124cd7a6d5ef2` | agent-settings-generation |
