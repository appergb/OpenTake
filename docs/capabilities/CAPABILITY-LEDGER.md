---
id: capabilities.ledger
title: OpenTake Capability Ledger
summary: 首轮上游对齐和安装版 UI 实测的模块级状态；详细证据按能力 ID 继续累积。
kind: engineering
status: draft
content_stage: partial-implementation
scope:
  - all-modules
triggers:
  - parity
  - 上游对齐
  - 功能完成度
  - 真实验收
read_when:
  - 修改公共能力或判断模块是否完成
skip_when:
  - 仅查看单个内部函数
priority: must
freshness_class: project
last_verified: 2026-08-22T01:03:46+08:00
owners:
  - OpenTake-generation
source_of_truth:
  - ./requirements.json
  - ../audit/2026-08-21/full-desktop-functional-matrix.md
related:
  prerequisites:
    - ./README.md
  next:
    - ../superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md
supersedes: []
tags:
  - capabilities
  - parity
  - evidence
---

# 首轮状态

这不是最终完成声明，而是 2026-08-21 的可恢复基线。机器可读条目见 [`requirements.json`](./requirements.json)；安装版 UI 场景见 [`full-desktop-functional-matrix.md`](../audit/2026-08-21/full-desktop-functional-matrix.md)。首轮上游 agent 按实际代码和两条定向测试落锚，未把历史 handoff 当成当前证据。

## 已有代码链路

- Models、Timeline、Track、Clip、Keyframe、Transform、TextStyle：domain crate 已有实现，需继续做完整行为级枚举。
- EditCommand、事务边界、undo/redo、split/trim/move/ripple：ops/core 已有上游对齐实现；`split_clip_distributes_keyframes_at_cut` 通过。
- Inspector、字幕/转写/搜索、Agent/MCP、Motion Studio opaque MP4：代码和对应测试入口已存在；仍需安装版场景证据才能升级为 verified。
- Motion Studio transparent alpha：Rust/Web 纵切已接通并通过全量自动化；最新包已完成透明开关、发布进度、时间线落轨和保存重开屏幕验收；导出面板已新增透明 ProRes 4444 codec，真实 GPU/FFmpeg integration 已复核 `yuva444` alpha；透明导出屏幕仍待解锁后复验。
- Agent/MCP linked move parity：`move_clips` 现在和上游约定一致，显式 `toFrame` 会把 frame delta 传播到 linked A/V partner，track-only move 不传播；新增两条 dispatcher 回归测试。最新安装版还验证了 Agent 面板标签生命周期、无通道时发送禁用和快捷键收起布局。
- 上游收敛新切片：`create_folder.entries` 与 `move_to_folder.entries` 已接入单一 EditCommand/Undo 事务；普通媒体导入现在接受有效 `.json` / `.lottie`，用 Velato 校验并写入 Lottie metadata，坏文档在批量入口跳过，MCP 单文件入口返回明确错误。

## 首轮明确缺口

| ID | 优先级 | 当前状态 | 最小实施切片 | 验收 |
|---|---:|---|---|---|
| `OT-WINDOW-SMALL-SCREEN` | P0 | verified | Tauri 安全初始尺寸 + monitor-aware standard/compact 裁剪 + Home/Editor min-size 回归 | 1066×666、1280×720、1331×768 contract 测试；安装版紧凑/标准切换与 Home/编辑器截图 |
| `UP-MEDIA-IMPORT-AND-FOLDERS` | P0 | verified | 文件/文件夹导入、跳过 unsupported、relink 和预览均已通过安装版验收 | 新构建 app 中 MP4、PNG、双文件多选 Open 可用；导入数 1→3；文件夹导入出现目录并提示跳过 64；relink 恢复离线媒体 |
| `UP-MEDIA-LOTTIE-IMPORT` | P1 | partial | 对齐上游 `.json/.lottie` 导入；Velato JSON 校验、`.lottie` ZIP `animations/*.json` 提取、metadata 写入和坏文件 fail-soft 已通过自动化 | `cargo test -p opentake-core importable_clip_type_covers_whitelist_and_rejects_others`；Tauri Lottie JSON/容器/MCP path 定向测试；安装版素材预览/落轨/重开屏幕待解锁 |
| `UP-TIMELINE-UI` | P0 | partial | 已补 Shift+标尺范围标记、范围 start/end 边缘命中/拖动、标尺吸附、取消回滚、同帧清除、PointerEvent 回归，以及上游 playhead split/trim 的无选区 no-op 语义；继续补真实删除、撤销及拖拽对拍 | 时间轴目录 8 files / 88 tests；linked slice 新增 command routing 2 个 no-selection 测试，Web 全量 151 files / 1412 tests；最新包 `de4cb52b…` 已安装但本轮屏幕重跑因 Mac 锁定 blocked |
| `UP-PREVIEW-PLAYBACK` | P0 | partial | compositor temporal route、speed/reversed 原生帧映射、音频整轨解码和缺失媒体 fail-closed 已补齐；安装版与 preview/export 全面对拍仍继续 | Render 18 项、Playback resolver 15 项、Playback integration 8 项和顺序 Rust workspace 通过；preview/export 同帧语义、音画同步、取消和最新包屏幕证据仍待 |
| `UP-EXPORT` | P0 | partial | H.264/AAC、H.265、ProRes 422 和字幕命令已有实现；普通输出失败清理、external cancel 全链路、输出父目录/文件 identity 校验已补齐 | 最新包已补 H.265/AAC 与 ProRes 422 HQ/PCM 的 SavePanel/文件证据；透明 ProRes 4444、字幕、取消、实时音画同步和首中尾对拍仍待 |
| `UP-PREVIEW-TABS` | P1 | verified | `previewTabIds + previewTabHistory + activeTabId`，含 legacy 归一和删除清理 | 安装版同时显示 Timeline/两个素材 tab；关闭第二个回退第一个；91 个定向测试通过 |
| `UP-MEDIA-VIEW-MODES` | P1 | verified | folder/flat/grouped 三态投影、网格/列表密度、文件夹导航和音频导入入口已通过测试及安装版验收 | 搜索、选择、拖拽和预览沿用同一 MediaItem ID 链路，继续纳入后续模块化 QA |
| `OT-MOTION-ALPHA` | P1 | partial | Motion Studio 透明发布开关、导出面板 ProRes 4444/yuva444、manifest straight-alpha provenance | 透明 motion 覆盖底图、预览/导出一致，并完成安装版时间线导入对拍；代码/自动化已通过，透明导出屏幕和 opaque→transparent 编辑仍待 |
| `UP-ACCOUNT-CLOUD-BOUNDARY` | P0 | blocked | 先定义 BYOK/provider 替代还是追同构云契约，再补 capability gate | 未授权时不广告/不发送；有凭据时生成→媒体→落轨闭环 |
| `UP-AGENT-FOLDER-BATCH` | P1 | verified | `create_folder.entries` / `move_to_folder.entries` 批量参数与上游一致，分别返回 folder records/汇总并保持单步 Undo | `cargo test -p opentake-agent folder_batch`；Agent lib 417/417 |
| `UP-SETTINGS-TAXONOMY` | P2 | partial | 明确 models/agent/storage/general/account 语义映射 | 设置入口和上游语义一一可解释 |

## 真实 UI 证据摘要

## OT-MOTION-ALPHA 当前纵切

- Rust：`DocumentMotionAddRequest.transparent` 已贯通到模板渲染、`MotionRenderRequest.with_transparent(true)`、ProRes 4444 `.mov` 编码和项目媒体落盘；title-card/lower-third 模板在透明模式下不填充不透明底色；编辑已有透明 Motion 时从 manifest provenance 保留 `.mov`/alpha 格式。
- Manifest：`GenerationInput.transparent` 持久化为 `true`，`MediaManifestEntry::carries_straight_alpha()` 对透明 Motion 返回 `true`；既有 RVM matting provenance 规则保持不变。
- Web/Agent：Motion Studio Inspector 新增“透明背景（ProRes 4444）”开关；预览请求保持原 schema，只有发布请求带 `transparent` 字段，切换项目时重置为关闭；Agent `add_motion_graphic` 和 `publish_motion_document` 的新增 clip 路径也接受透明发布，已有透明 clip 的文档编辑保留 alpha。
- 自动化证据：`src-tauri/tests/motion_command.rs` 的透明发布集成测试验证 `.mov`、ProRes、64×36 尺寸、完全透明像素和半透明动画像素；Rust workspace 串行全量通过（Tauri 720 tests、Motion Chromium/integration、导出/播放等）；Web 全量 `151 files / 1415 tests`、Web build 通过。
- 文件级证据：`/private/tmp/opentake-audio-desktop-qa-L8Nvwh/audio-preview-export-qa.opentake/media/motion-0e4cacc9-161a-4704-a790-7e233397c8c4.mov` 被 `ffprobe` 识别为 `prores` profile `4444`、tag `ap4h`、pixel format `yuva444p12le`、90 帧/3.000s；`ffmpeg -vf alphaextract` 成功，抽样 alpha 平面为非全黑值；对应 `media.json` 的 `generationInput.transparent` 为 `true`。
- 安装包：当前 `/Applications/OpenTake.app` 二进制 SHA-256 `acd7b105a75d957bc761f9128f9f89e7ee563f65031dc7e22952fa7a6c65c319`，对应 `c1db732`；已重新打包安装，但 Mac 锁屏，透明 ProRes 4444 专用导出、Lottie 屏幕路径和其它待验收动作仍不升级为通过。

## 上游 linked A/V parity 当前切片

- Agent/MCP 的 `move_clips` 入口已补齐上游 `partnerMoves` 语义：只提供 `toFrame` 时按 lead 的时间差移动同组伙伴，并保留伙伴所在轨道；只提供 `toTrack` 时不改伙伴时间；调用方显式列出伙伴时不再生成重复 move。
- 证据：`cargo test -p opentake-agent --lib -- --test-threads=1` → 415 passed；新增测试覆盖 frame delta propagation 和 track-only no propagation。Web Timeline 拖拽原有 linked expansion 保持不变。

## 最新安装版模块入口证据

- Inspector：选中 V1 后视频页展开变换/裁剪/翻转/淡入淡出/运动追踪/防抖/抠像/速度/调色；水平翻转切换后由 Undo 恢复。音频页显示音量、响度归一化、降噪和人声/伴奏分离；AI 编辑页生成本地启发式建议，应用后显示“可撤销编辑命令”，再撤销恢复。
- 字幕：字幕样式、位置和翻译同意项可见；点击生成字幕时在无模型环境明确提示下载约 141 MB 的 multilingual 转写模型；无同意时翻译按钮保持禁用。
- Agent：面板可打开，新建/关闭对话标签可用；未配置通道时发送保持禁用；`⌘⌥A` 可收起并恢复编辑器布局。未发送外部消息。

- 约 `1331×768`：标准窗口配置偏大，最近项目区域宽度控制弱。
- 紧凑档约 `1066×666`：可见、可操作，当前切换行为通过。
- 新建/打开项目可弹原生 sheet 并取消；示例项目可打开；播放头会推进；选中文本片段能联动 Inspector；Agent/Motion/导出/帮助入口可达。
- 首轮导入图片时选中后 Open 仍禁用，导入失败阻塞素材预览闭环；时间线分割在帧 0/片段外无变化，但在有效选区和帧 15 前置条件下已实测新增片段并启用撤销。
- 本轮修复后，文件导入 MP4/PNG/双文件多选均能点 Open，导入后素材预览可见；文件夹导入成功出现目录并提示跳过 unsupported；relink 成功恢复离线媒体。
- Preview tabs 已在安装版打开两个素材并验证关闭回退；当前 Preview/Media slice 的定向测试为 4 files / 91 tests passed。
- Preview temporal parity 已补 route、native surface 和 source-frame rewind reset；安装版 QA 工程设置 `speed=1.5` + 曝光 compositor 属性后没有 unsupported surface，时间线可见、播放头可到尾帧、暂停后抓帧按钮可用。
- 时间轴范围切片已补 Shift+标尺 range mark、已有范围 start/end 边缘命中/拖动、clip-edge/playhead snap、pointercancel/lost-capture 回滚和同帧清除；最新安装包按上游顺序完成 I/O 标记范围→选 V1 锚点→Shift+Backspace，V1/A1 联动删除、Motion 保留、播放头/时间码同步到新 3 秒长度、Undo 恢复。同步刷新新增 playhead clamp；时间轴目录 8 files / 88 tests，Web 全量 151 files / 1415 tests、tsc、Web build 通过。Option trim 屏幕坐标拖动仍待。
- 带音频 fixture `nested-timeline-compound-export-2026-07-31.mp4` 已通过 `ffmpeg -xerror` 音视频解码；安装版出现 V1+A1、A1 波形和音频 Inspector，并完成起点/中点/终点 seek 与暂停稳定性检查。fixture 为 7.700s H.264 1280×720/30fps/231帧 + AAC 48kHz 单声道；独立实时听感级音画同步仍未完成。
- 带音频导出真实证据：在安装包 `03bdcef36def17646c64e6c40978e26b6b675a5d90874e3264d93a0e3351ce2e` 中生成 `/private/tmp/opentake-audio-export-qa-YPhWaR/qa-h264-aac-export.mp4`；ffprobe 为 H.264 1280×720/30fps/231 帧 + AAC 48kHz 单声道/362 包/7.700s，视频和音频均通过 `ffmpeg -xerror`。源/导出首中尾 SSIM：`0.999997 / 0.999997 / 1.000000`，帧文件在 `/private/tmp/opentake-audio-frame-compare.M2Jvfu/`。这证明一次真实 preview-source/export 对拍，但不是听感级实时同步证据；cleanup 版最新包 `ca08edf97441ce3b3c69b5683a3b7383997371952490c8897021772e521eed64` 的 SavePanel 复测仍保持 partial。
- 导出文件证据：包 `893b6ed0…d0ba` 生成 H.265/AAC `/private/tmp/opentake-audio-desktop-qa-L8Nvwh/audio-preview-export-qa.mp4`（HEVC `hev1` + AAC，7.700s）和 ProRes 422 HQ/PCM `/private/tmp/opentake-audio-desktop-qa-L8Nvwh/audio-preview-export-qa.mov`（`apch`/`yuv422p10le` + PCM，7.700s），两者均通过完整 `ffmpeg -xerror`。新增 `OPENTAKE_RUN_FFMPEG_TESTS=1 cargo test -p opentake-tauri --test export_integration export_prores_4444_preserves_transparent_text_alpha` 验证 ProRes 4444 `yuva444p10le/12le` 和 alpha 0→非零。导出失败边界：`0bae80e` 增加 identity-safe 普通输出 guard、replacement race 测试、双 cancel source fail-closed；`a7d98d6` 再拒绝最终输出路径的 symlink/reparse identity，并新增替换为 symlink 的回归测试。当前 `export::tests` 72/72、`opentake-media` cancel 18/18、带音频 integration 6/6 通过。
- 上游 playhead parity：`f05bac8` 修复无选区 `splitAtPlayhead` 不应发编辑请求，`478e1b4` 修复无选区 Q/W trim 不应发编辑请求；相关 Web 全量门禁 `151 files / 1412 tests` 通过。最新 app `de4cb52b…` 已构建并安装，但 Computer Use 因 Mac 锁定未能完成本轮屏幕 smoke。
- 缺失媒体 fail-closed：`741ff07` 让普通 Preview 缺失图片返回 materialization error，Playback Image/Text 缺失通过 `RenderLoop::render_frame` 返回错误而不是发布黑帧，并统一 Playback/Export 错误文案；新增 render 18、resolver 15、playback integration 8 项测试，Tauri lib 719 项和顺序 workspace 全量均通过。
- 曾出现 AX 更新但截图未重绘的黑屏瞬间；关闭/重开后页面正常，后续以窗口重绘时序风险记录，不把瞬态截图直接等同于产品黑屏。

## 定向验证

| 命令/场景 | 结果 | 说明 |
|---|---|---|
| `cargo test -p opentake-ops split_clip_distributes_keyframes_at_cut` | 通过 | 上游 split 关键帧边界对拍锚点 |
| `cargo test -p opentake-agent --test advertised_tool_acceptance` | 通过 | 工具广告面锚点 |
| `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage.json pnpm exec vitest run src/store/settingsStore.test.ts src/components/home/HomeView.visual.test.ts src/components/shell/SplitPane.interaction.test.tsx` | 3 files / 22 tests 通过 | Node 26 需要显式 localStorage 文件 |
| `pnpm test`（无 Node localStorage 参数） | 15 files / 125 tests 初始化失败 | 环境假红：`localStorage` 未提供，不作产品失败结论 |

## Task 2 新鲜验收证据

- 提交范围：`ff6177e8`（主实现）、`2235c638`（SplitPane 测试）、`32c8c3c`（review 修复）、`70e56c9`（Settings contract 修复）。
- 受影响 Web 回归：
  `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-final.json pnpm exec vitest run src/store/settingsStore.test.ts src/components/home/HomeView.visual.test.ts src/components/shell/SplitPane.interaction.test.tsx src/components/settings/SettingsView.visual.test.ts src/components/settings/SettingsView.interaction.test.tsx`
  → 5 files / 50 tests passed。
- Web build：`pnpm build` → exit 0；保留既有 dynamic-import / large-chunk warnings。
- Rust 格式：`cargo fmt --all -- --check` → exit 0。
- 上游 split 锚点：`cargo test -p opentake-ops split_clip_distributes_keyframes_at_cut` → 1 passed。
- Release app：`web/node_modules/.bin/tauri build` 已完成 release 编译并生成 `/Users/trip/TRUE 开发/PRIMARY-CN/OpenTake-generation/target/release/bundle/macos/OpenTake.app`；DMG `bundle_dmg.sh` 无输出后手动停止，不能声明 DMG 完成。
- 安装版二进制：最终 token 修复后的 `.app` 已复制到 `/Applications/OpenTake.app`，SHA-256 `02150854e418cd3c3dedad97d905f972ddad99e3a23d287ed1fcac42b15b40a0`。
- Computer Use：新构建首次进入约 `1066×666` 紧凑档；Home/编辑器/预览/时间轴/Inspector/设置可见；切换标准档时窗口铺满约 `1331×768` 工作区、无白边，再切回紧凑档恢复约 `1066×666`。

## 当前未闭环的全量门禁

- 旧的并行 Web 门禁曾受 Node 26 共享 localStorage 影响；当前统一使用显式 localStorage 文件和单 worker，最新串行门禁已 151/151 files、1415/1415 tests 通过。
- 文件导入定向回归：`mediaActions` + `MediaPanel` 为 2 files / 59 tests passed；Preview/媒体/Store 定向回归为 4 files / 91 tests passed；两条切片均已安装版验证。
- 主线 fresh verification（2026-08-21 12:09 +08:00）：Preview/Store/Media 4 files / 91 tests passed；MediaActions/MediaPanel 2 files / 59 tests passed；`pnpm build` exit 0；仅保留既有 Vite chunk warnings。
- 全量 Web 串行门禁（2026-08-21 16:46 +08:00）：`NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-temporal-final.json pnpm -C web exec vitest run --maxWorkers=1` → 151 files / 1410 tests passed；使用新 localStorage 文件会触发既有 locale/异步测试状态假失败，已保留为运行约束。
- Rust workspace 门禁（2026-08-21 20:19 +08:00）：`cargo test --workspace --jobs 1 -- --test-threads=1` 通过；包含缺失媒体 fail-closed、export cleanup/external cancel、symlink identity、H.264/AAC integration、full-track PCM、RenderPlan temporal、Motion Chromium 和 native playback tests。`cargo fmt --all -- --check` 同样通过。全 workspace clippy 仍被既有 `chunks_exact`/`chunks_exact_mut` lint 阻塞，当前输出涉及 `opentake-media`、`opentake-motion` 等旧模块；本次导出改动触发的两个 clippy 建议已在 `6b31449` 收口。此前未限并发的 workspace 运行曾出现一次 motion sandbox 180s 超时，单独串行重跑 4 次均通过，保留为并发 GPU/Chromium 资源争用风险，不作为代码绿证据。

## Media View Modes 新鲜验收证据

- 实现提交：`745a190`（folder/flat/grouped UI）与 `5b1f45d`（空字符串根节点、悬挂 folderId fail-soft）。
- 聚焦测试：`NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-media-modes-final.json pnpm exec vitest run src/lib/mediaViewModes.test.ts src/lib/folderTree.test.ts src/components/media/MediaPanel.test.tsx` → 3 files / 64 tests passed。
- 安装版：重新构建并复制 `/Applications/OpenTake.app`，当前二进制 SHA-256 为 `fb46fbcaa96ddd10d8e760160b47674c6f233e55effd88eca6d3d8b552092a51`。
- Computer Use：菜单 `媒体组织方式` 可访问地提供“文件夹 / 平铺 / 分组”；带真实文件夹导入后，文件夹模式显示目录卡片，平铺模式显示 37 个媒体且隐藏目录卡片，分组模式显示“全部 / editor-core-after-fix-assets”分组；分组网格/列表均可用；进入目录后面包屑、返回上级及 17 个目录内项目可见；音频“导入”有同一模式入口，“我的/收藏”保持任务型空态。
- Preview token 修复：`tokenUsage.test.ts` 曾捕获未声明的 `--space-2xs`，已改为现有 `--space-xxs`；token、Preview、Store 48 tests 和 Web build 通过。
- 最终原生包：`web/node_modules/.bin/tauri build --bundles app` exit 0，release `.app` 完成生成；最终包 Computer Use 再次验证两个媒体 Preview tabs、关闭回退和紧凑窗口。
- `cargo tauri build` 的历史文档入口不可用（没有全局 cargo-tauri）；本轮使用项目本地 Tauri CLI。DMG 尾脚本仍需单独排查。

## Preview Temporal Remap 新鲜验收证据

- 代码提交：`a833764`（compositor temporal 路由）、`1013355`（多视频 temporal fail-closed）、`dafd5eb`（native Preview surface）、`3e124dc`（source frame 回退时 reset stream）。
- Web：路由 16/16，Preview/engine 44/44，串行全量 151 files / 1410 tests，`pnpm build` 均通过。
- Rust：`opentake-media` 432 tests + integration 通过；`opentake-render plan` 46 tests；native playback transport 8 tests；engine 11、transport 7 tests；workspace 全量通过。
- 安装版：当前 `/Applications/OpenTake.app` SHA-256 `7ad988f3d00c50af4758c0c036753d690101c2b4c66db17302b29b308bda9799`，对应代码提交 `6b31449`；temporal 与带音频导出证据仍见旧 QA 包，最新包的 SavePanel/取消/实时 A/V 同步和屏幕 smoke 尚未重新跑完，因此不把旧包导出证据静默升级到当前包。
- 最新 range slice 安装包：`549a6e50a6de52c9653bcb50a2be09b41c0566892f6783d5276c2847c64698c9`；仅用于本轮范围交互代码和 Web build 的 packaged smoke，不能替代范围拖动/Undo/Redo 的完整桌面证据。
- 带音频桌面 QA 证据：`/private/tmp/opentake-audio-desktop-qa-L8Nvwh/01-imported-timeline-waveform.png`、`02-preview-start-paused.png`、`03-preview-middle.png`、`04-preview-end-paused.png`、`05-playback-paused-stable.png`；原生 H.264 导出已打开并在开始前取消，不能声称有安装版带音频导出文件。

- 视频导出 SavePanel：`35801f8` 移除 macOS 视频保存 filters，保留 codec→扩展名和 `withExt`；focused ExportDialog 11/11、H.264 临时导出 160 帧 / 2.666667s / H.264 通过。

## Related Documents

- Parent index: [Capabilities README](./README.md)
- Machine manifest: [requirements.json](./requirements.json)
- Desktop matrix: [2026-08-21 functional matrix](../audit/2026-08-21/full-desktop-functional-matrix.md)
- Long-term plan: [Full UI and upstream convergence](../superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md)

## Change Log

- `2026-08-21T10:06:00+08:00` — 写入首轮 UI 真机巡检和上游 parity audit；状态均保守记录为 implemented/partial/blocked。
- `2026-08-21T19:32:27+08:00` — 修复最终输出 symlink/reparse identity 校验并通过 RED→GREEN 回归；顺序 Rust workspace 全量通过；重新构建并安装对应提交 `a7d98d6` 的 app，记录 SHA-256 `12e1db…9455d9`。并发 workspace motion 超时保留为环境风险。
- `2026-08-21T19:48:03+08:00` — 对齐上游无选区 split/trim no-op（`f05bac8`、`478e1b4`），Web 全量更新为 151 files / 1412 tests；最新安装包 SHA-256 `de4cb52b…25405`。Computer Use 屏幕 smoke 因 Mac 锁定 blocked，未升级桌面证据。
- `2026-08-21T20:09:19+08:00` — 完成缺失媒体 Preview/Playback fail-closed（`741ff07`），补真实 RenderLoop 集成证据；顺序 workspace 全量通过，当前安装包 SHA-256 `40c21ce0…e7f08`。Mac 仍锁定，未升级屏幕证据。
- `2026-08-21T20:19:15+08:00` — 复跑当前提交 `6b31449` 的顺序 workspace 全量、导出/播放集成和 app 构建；最新安装包 SHA-256 `7ad988f3…a9799`。全 workspace clippy 的既有 `chunks_exact` lint 仍单独记录，Mac 屏幕 smoke 仍未完成。
- `2026-08-21T20:57:59+08:00` — 完成 `OT-MOTION-ALPHA` 首条 Rust/Web 纵切：透明模板→Chromium alpha→ProRes 4444 `.mov`→manifest provenance→Motion Studio 发布开关；Rust workspace 720 Tauri tests 与 Web 151/1413 全量通过，重新构建并安装包 SHA-256 `7634979e…607f00`。最新包屏幕验收仍因 Mac 锁屏阻塞，能力保持 partial。
- `2026-08-21T21:15:16+08:00` — 补齐 Agent/MCP `move_clips` 的 linked A/V frame-delta parity，并修正文档中 `add_motion_graphic.transparent` 已支持 ProRes 4444 的描述；Agent lib 415/415 通过。屏幕验收阻塞边界不变。
- `2026-08-21T21:21:26+08:00` — 将透明参数接入 Agent `publish_motion_document` 的新增 clip schema/host bridge；已有透明 Motion 的 document edit 仍需由旧 manifest provenance 保留 alpha，本轮随后补齐并用真实 FFmpeg integration 验证。
- `2026-08-21T21:21:26+08:00` — 重新构建并安装包含 Agent Motion 透明参数的 app，当前二进制 SHA-256 `38c814f4…bc838`；Computer Use 仍因 Mac locked 无法执行最新包屏幕 smoke。
- `2026-08-21T21:31:52+08:00` — alpha-preserving document edit 真实 FFmpeg integration 通过，重新构建并安装当前 app，二进制 SHA-256 `d5f8aa46…87ae7`；屏幕 smoke 仍因 Mac locked 阻塞。
- `2026-08-21T22:15:18+08:00` — 最新包 `918eac84…9551b` 完成真实桌面 smoke：小屏/标准 Home、媒体预览 tab、播放到尾帧、分割/撤销、导出面板和 Motion 透明发布落轨；Option trim 坐标接口仍返回 `noWindowsAvailable`，透明导出/保存重开保持待验证。
- `2026-08-21T22:27:41+08:00` — 最新包 H.264/AAC SavePanel 导出写入 `/private/tmp/opentake-audio-desktop-qa-L8Nvwh/opentake-latest-smoke.mp4`，ffprobe 和完整 `ffmpeg -xerror` 通过；透明 ProRes 专用导出仍待。
- `2026-08-21T22:32:49+08:00` — 最新包真实屏幕验证 linked V1/A1 的 Shift+Backspace ripple delete 与 Undo：伙伴同时删除、其它 Motion clip 保留、撤销恢复；时间轴剩余 Option trim/range 屏幕对拍继续保持 partial。
- `2026-08-21T22:37:23+08:00` — 复核透明 Motion 文件级产物：真实 QA `.mov` 为 ProRes 4444 `ap4h` / `yuva444p12le`，alphaextract 可读且非全黑；能力仍因透明专用导出 UI、opaque→transparent 编辑和其它导出矩阵保持 partial。
- `2026-08-21T22:47:48+08:00` — 写回最新安装版 Inspector、字幕和 Agent 入口/失败边界证据；完整模型下载、转写/字幕导出、MCP/Agent 工具调用仍未升级为完成。
- `2026-08-21T23:09:09+08:00` — 写回最新包范围删除/Undo 与 playhead clamp 屏幕证据；安装包 SHA-256 `893b6ed0…d0ba`，Web 全量 151/1415 通过；Option trim 和其它导出矩阵仍保持 partial。
- `2026-08-21T23:19:12+08:00` — 写回最新包 H.265/AAC、ProRes 422 HQ/PCM SavePanel 与文件证据；透明 ProRes 4444 专用导出、字幕、取消和实时音画同步仍未升级为完成。
- `2026-08-21T23:47:02+08:00` — 写回透明 ProRes 4444 导出实现和 GPU/FFmpeg alpha integration；最新安装包 `3ce3c1d0…c7eef`，Computer Use 因 Mac 锁屏未完成透明导出屏幕复验。
- `2026-08-22T00:46:36+08:00` — 完成上游 agent 确认的两个 P1 切片：Agent 文件夹批量 entries 进入单步 Undo；媒体导入接受 Lottie JSON/`.lottie`，Velato 校验、`.lottie` ZIP animation 提取和 metadata 进入 Tauri 边界；Agent 417/417、ops 209/209、Tauri Lottie JSON/容器/MCP path 定向测试通过，安装版 Lottie 屏幕验收仍待解锁。
- `2026-08-22T01:03:46+08:00` — Tauri lib 726/726、Web 151/1416、tsc/build 通过；workspace 全量在既有 Motion Chromium 4K budget smoke 超时 180s 后 poison gate，记录为环境/GPU/锁屏风险，未把该失败归因到本轮改动。
- `2026-08-22T01:08:00+08:00` — 基于 `c1db732` 重建并安装最新 `.app`，二进制 SHA-256 `acd7b105…c319`；Mac 锁屏，仅记录构建/安装证据。
- `2026-08-22T01:17:36+08:00` — 窄范围上游对拍确认 Rust ripple/range/insert/trim、linked partner 和原子拒绝语义一致；前端 Timeline 逐函数对拍与安装版屏幕验收仍未完成，能力保持 partial。
