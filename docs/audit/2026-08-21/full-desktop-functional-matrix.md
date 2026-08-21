---
id: audit.desktop-functional-matrix.2026-08-21
title: OpenTake Desktop Functional Matrix — 2026-08-21
summary: 记录安装版 OpenTake 在约 1331×768 小屏上的首轮真实 UI 巡检，以及后续模块化回归的入口和判定规则。
kind: audit
status: draft
content_stage: partial-implementation
scope:
  - packaged-macos-app
  - home
  - editor
  - timeline
  - preview
  - media
  - settings
  - agent
  - motion
triggers:
  - 真机验收
  - 屏幕过大
  - 时间轴回归
  - 预览回归
read_when:
  - 修改窗口几何或桌面 UI
  - 修改时间轴、预览、媒体导入、设置、Agent、Motion
skip_when:
  - 仅修改零 IO 的 domain 算法且不改变 UI 合同
priority: must
freshness_class: project
last_verified: 2026-08-21T23:09:09+08:00
owners:
  - OpenTake-generation
source_of_truth:
  - ../../src/
  - ../../web/src/
related:
  prerequisites:
    - ../../superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md
  next:
    - ../../capabilities/CAPABILITY-LEDGER.md
supersedes: []
tags:
  - audit
  - desktop
  - ui
  - playback
---

# 首轮真实巡检

## 环境

- 安装应用：`/Applications/OpenTake.app`
- 观测方式：Computer Use `@oai/sky`，每次动作后重新获取 AX tree 和截图。
- 截图逻辑尺寸：首轮首页约 `1331×768`；切换紧凑窗口后约 `1066×666`。
- 本轮没有提交代码、没有改上游、没有永久删除文件、没有发送 Agent 消息、没有点击最终导出。

## 布局与窗口

| 场景 | 期望 | 实际 | 状态 | 证据 |
|---|---|---|---|---|
| Home / 标准档 / 小屏 | 不超过工作区，内容有合理最大宽度 | 最新包标准档 Home 截图无白边、卡片不溢出；标准档工作区完整可见 | 通过（窗口）/部分（Home 内容密度） | `src-tauri/tauri.conf.json`；`web/src/store/settingsStore.ts`；Task 2 5 files / 50 tests；最新包 Computer Use 截图 |
| 设置 → 外观 → 紧凑 | 窗口收缩且内容仍可操作 | 最新包切换后约 `1066×666`，紧凑 radio 状态正确，Home、时间线、预览、Inspector 和 Motion Studio 均可见 | 通过 | `settingsStore` + Home/SplitPane/Settings 50 tests；最新包 Computer Use 紧凑档截图 |
| Home → 设置 | 设置面板可见、可操作 | 复现时曾抓到 AX 已更新但截图未重绘；关闭/重开后及当前复测设置面板可见，判定为 Computer Use 重绘时序风险，不先当产品黑屏根因 | 部分 | 当前复测 AX + 设置截图 |

## 功能场景

| 模块 | 场景 | 实际结果 | 状态 | 下一步 |
|---|---|---|---|---|
| Home | 新建项目 | 原生保存 sheet 可打开，可取消 | 通过 | 在 QA fixture 中补成功落盘/重开 |
| Home | 打开项目 | 原生打开 sheet 可打开，可取消 | 通过 | 用复制的 QA 工程补成功打开 |
| Home | 示例项目 | `产品演示` 可进入编辑器，AX 可读完整结构 | 通过 | 保存真实截图和工程路径 |
| Media | 导入文件 | 新构建 app 中 MP4、PNG、双文件 Cmd 多选后 Open 均可用，导入数从 1→3 | 通过 | `fix(media): make native file import selectable`；Computer Use 原生 dialog；`mediaActions`/`MediaPanel` 59 tests |
| Media | 导入文件夹 | 选择 `editor-core-after-fix-assets` 后媒体库出现目录，AX 提示已跳过 64 个 unsupported 文件 | 通过 | 新构建 app Computer Use 12:xx；文件夹 dialog 未改 filters |
| Media | 文件夹 / 平铺 / 分组 | 安装版 `媒体组织方式` 菜单可访问；真实文件夹导入后文件夹模式显示目录卡片，平铺模式显示 37 个媒体且隐藏目录卡片，分组模式显示“全部 / editor-core-after-fix-assets”两组；分组网格/列表均可切换 | 通过 | `mediaViewModes` + `MediaPanel` 64 tests；安装版 Computer Use 13:xx；二次 code review Ready |
| Media | Relink | 选择原始 MP4 后离线媒体恢复，Preview tab 和 Inspector 路径更新 | 通过 | 新构建 app packaged relink 验收 |
| Media | 素材预览 | 导入后真实 MP4 缩略图/画面可见，PNG 也能作为 source preview 打开 | 通过 | Computer Use：素材 tab、Preview tab、Inspector 来源/尺寸/路径 |
| Timeline | 播放/暂停 | 播放头从 0 推进到约 54，时间从 00:00:00 推进到约 00:01:24 | 通过 | 增加暂停/恢复/seek/尾帧证据 |
| Timeline | 选中片段 | 选中 `sample-text-0` 后 Inspector 切换到文本属性 | 通过 | 补选区、拖拽、删除和 undo |
| Timeline | 播放头处分割 | 初始在帧 0 或片段外尝试时无变化；补齐有效前置条件（选中 `sample-text-0`、播放头推进到帧 15）后成功新增片段、撤销变可用 | 通过（有效前置条件） | 新构建 app AX：分割后出现 UUID 片段；`cargo test -p opentake-ops split_clip_distributes_keyframes_at_cut` 通过 |
| Timeline | 入点/出点范围与范围边缘 | `TimelineContainer` 已接入 Shift+标尺 range mark、已有范围 start/end 边缘命中与拖动、clip-edge/playhead snap、pointercancel 回滚和同帧清除；Option trim 改为只修剪主 clip，普通 trim 保持 linked propagation；最新包按上游顺序完成 I/O 标记整段范围→再选 V1 锚点→Shift+Backspace，V1/A1 联动删除、Motion 保留，删除后播放头从 7.7s 合法收敛到 3.0s，Undo 恢复三条轨道；Web 全量 151 files / 1415 tests 通过 | 通过（范围删除/Undo/播放头屏幕对拍；Option trim 仍 partial） | 继续补可复现的安装版 Option trim，以及部分范围/多片段场景 |
| Inspector | 片段属性与 AI 编辑 | 选中 V1 后视频 Inspector 展开 131 项，覆盖变换、裁剪、翻转、淡入淡出、运动追踪、防抖、RVM 抠像、速度和调色；水平翻转可切换并由 Undo 恢复；音频页能显示音量/响度/降噪/人声分离；AI 编辑页能生成本地建议，应用后显示“可撤销编辑命令”，撤销成功 | 通过（入口/可撤销行为） | 数值控件逐项边界、关键帧落盘重开和本地模型分析仍待 |
| Preview | 带音频 fixture 的导入/波形/seek/暂停 | `nested-timeline-compound-export-2026-07-31.mp4`：7.700s、H.264 1280×720/30fps/231帧 + AAC 48kHz 单声道；安装版出现 V1+A1、A1 波形和音频 Inspector，已操作起点/中点/终点 seek，连续暂停状态稳定 | 部分 | 实时听感级同步和最新 cleanup 包的导出复测仍待 |
| Preview | compositor temporal 预览 | 最新安装版在 QA 工程中设置 `speed=1.5` 和曝光 `0.50` compositor 属性后不再出现 unsupported surface；画面可见，播放头可到尾帧，暂停后抓帧按钮保持可用；speed/reversed 的首次 native render、JPEG publication、source-frame 映射由 Rust integration tests 锁定 | 通过（代码/native/屏幕组合证据） | 预览与导出起/中/尾帧、音频同步和取消仍在后续闭环 |
| Preview | 多素材 tabs | 打开 `audit-4k60-h264` 与 `export-h264-frame30` 后显示 Timeline/两个媒体 tab；关闭第二个回退第一个 | 通过 | 安装版 AX：两个 Close 按钮；Preview slice 4 files / 91 tests |
| Agent | 面板、对话标签和本地边界 | Agent 面板可打开；新建/关闭对话标签成功；无配置通道时输入框存在但发送按钮禁用；用 `⌘⌥A` 可收起并恢复编辑器布局 | 通过（入口/本地边界） | 不发送外部消息；MCP 握手、失败/取消、真实工具调用仍待 |
| Subtitles | 字幕入口与缺失模型边界 | 字幕页可打开，来源/样式/位置/翻译同意项均可见；点击生成字幕后明确提示需下载约 141 MB 的 multilingual 转写模型，并提供“下载模型”入口；未同意外部 Provider 时翻译按钮保持禁用 | 通过（入口/失败边界） | 模型下载、转写结果编辑、SRT/VTT 导出和翻译真实闭环仍待 |
| Motion | Studio 入口 | HTML/CSS 编辑器、预览、参数检查器、关键帧时间线出现；播放帧推进 | 通过（既有桌面证据） | 用最新包补发布、保存重开和导入时间线 |
| Motion | 透明发布 | 最新包 Motion Studio Inspector 可见并可点击“透明背景（ProRes 4444）”；真实发布进度出现，完成后素材库新增 Motion Graphic、时间线新增 V2 片段，预览/Inspector 均可见；QA 工程内真实媒体叶子为 ProRes 4444 `ap4h` / `yuva444p12le`，90 帧/3.000s；`alphaextract` 可读取非全黑 alpha 样本；manifest 的 `transparent: true` 与项目媒体引用一致 | 通过（代码/自动化/发布屏幕/文件） | 透明 Motion 的专用导出 UI、opaque clip 编辑时切换透明、H.265/字幕/取消导出仍待矩阵化 |
| Export | 导出面板 | 导出视频面板可打开，格式/分辨率/取消/导出可见 | 通过 | 复制目标路径后完成导出并 ffprobe |
| Export | temporal 视频实际导出 | 修复 SavePanel filters 后，原生 Save 按钮可用；H.264 temporal QA 导出成功，应用显示 3840×2160 / 160 帧，ffprobe 显示 H.264、60fps、2.666667s | 通过（H.264 视频） | H.265/ProRes、带音频/字幕、取消和 preview/export 像素对拍仍待矩阵化 |
| Export | 带音频 H.264/AAC 实际导出 | 最新包 `918eac84…9551b` 通过 SavePanel 导出 `/private/tmp/opentake-audio-desktop-qa-L8Nvwh/opentake-latest-smoke.mp4`；ffprobe：H.264 1280×720/30fps/231 帧、AAC 48kHz 单声道/362 包、7.700s；完整 `ffmpeg -xerror` 通过 | 通过（最新包） | H.265/ProRes、字幕、取消和实时音画同步仍待矩阵化 |
| Export | 失败/取消输出清理 | `0bae80e` 增加普通输出 identity-safe guard、replacement race、双 cancel source fail-closed；`a7d98d6` 增加 symlink/reparse 最终路径拒绝；export unit 70/70、media cancel 18/18、audio/video integration 5/5，顺序 workspace 全量通过 | 通过（代码/测试） | 仍需最新安装包触发一次错误/取消并确认 UI toast/文件清理 |
| Help | 菜单 | 快捷键/MCP 说明可展开；教程/反馈显示 Beta 禁用 | 通过 | 记录禁用原因和可用边界 |
| Library | 全局素材库 | AX 可读分类、搜索、排序；部分截图曾未重绘，当前复测仍需重复 | 部分 | 在窗口修复后重跑可见性和空态 |

## 初始回归顺序

1. 先完成窗口尺寸裁剪和小屏布局，避免大窗口影响所有 UI 判断。
2. 修复/验证媒体导入状态，准备视频、音频、图片、文字和字幕 fixture。（文件/文件夹/relink 已通过，音频独立预览仍待补。）
3. 修复时间轴分割命令的真实选区/命令链路。（有效前置条件已通过，拖拽/删除/复杂音视频仍待补。）
4. 重跑预览、播放、Inspector、导出和保存重开。
5. Media folder/flat/grouped 已通过；继续验 Motion alpha、Settings taxonomy 和 generation/account envelope。

## 证据边界

AX tree 只能证明节点存在，不能证明用户看到或能操作；截图只能证明当次绘制，不能替代导出文件/工程重开/帧级结果。后续条目必须同时记录动作、结果和可定位证据，不能把历史 Beta 审计文字直接升级为当前 verified。

## Task 2 复核结果

- 代码 review：首轮 F1（无 monitor fallback）、F2（fresh install 默认档位）、F3（Home 真实 DOM/CSS contract）、F4（Settings 高度 contract）均已在 review loop 收口；最终 scoped review 无新 Critical/Important。
- App 构建：release `.app` 已生成并安装；DMG bundle 脚本因超过一分钟无输出被停止，DMG 不在本轮交付证据内。
- 本轮主线 fresh verification：Preview/Store/Media 4 files / 91 tests passed；MediaActions/MediaPanel 2 files / 59 tests passed；`pnpm build` exit 0。
- 全量 Web 串行门禁：`NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-range-refresh-full.json pnpm exec vitest run --pool=forks --maxWorkers=1` → 151 files / 1415 tests passed；`pnpm build` exit 0。
- Rust workspace：`cargo test --workspace --jobs 1 -- --test-threads=1` 与 `cargo fmt --all -- --check` 通过；本轮包含缺失媒体 Preview/Playback fail-closed、透明 Motion ProRes 4444 集成和 RenderLoop 集成测试。全 workspace clippy 仍被既有 `chunks_exact`/`chunks_exact_mut` lint 阻塞，当前输出涉及 `opentake-media`、`opentake-motion` 等旧模块；一次未限并发运行在 motion sandbox 180s 超时，单独串行重跑通过，保留为 GPU/Chromium 资源争用风险；仅保留明确标记为 ignored 的 real-device / optional fixture tests。
- 最终 `.app`：`web/node_modules/.bin/tauri build --bundles app` exit 0；当前安装包 SHA-256 `893b6ed082f6f8f4d70eeeb974d00ef11dafa2020075d69fc7629df83085d0ba`；最新包已完成 Home 小屏、媒体预览 tab、播放/尾帧、分割/撤销、范围删除/Undo、导出面板和 Motion 透明发布屏幕 smoke；Option trim/其它编码/字幕/取消导出仍待。

## Related Documents

- Parent index: [Docs index](../../INDEX.md)
- Plan: [Full UI and upstream convergence plan](../../superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md)
- Capability ledger: [CAPABILITY-LEDGER.md](../../capabilities/CAPABILITY-LEDGER.md)

## Change Log

- `2026-08-21T10:06:00+08:00` — 首轮安装版 UI 巡检；记录窗口、导入、分割和真实回归边界。
- `2026-08-21T12:09:35+08:00` — 写回 Preview tabs、文件/文件夹/relink 导入和主线 fresh verification 结果。
- `2026-08-21T13:12:31+08:00` — 写回媒体 folder/flat/grouped 三态、网格/列表密度、文件夹导航及音频子页的安装版验收结果。
- `2026-08-21T14:51:41+08:00` — 写回 audio full-track 解码修复、compositor temporal preview/native parity、最新安装包和全量 Web/Rust 门禁结果。
- `2026-08-21T15:32:21+08:00` — 写回 SavePanel 修复、H.264 temporal 实际导出和 ffprobe 结果；带音频对拍仍保留为未闭环项。
- `2026-08-21T16:08:41+08:00` — 写回时间轴范围交互代码切片、85 个时间轴测试、最新安装包哈希，以及带音频 fixture 的 V1/A1 波形、seek、暂停证据；范围坐标拖动和带音频导出仍保持 partial。
- `2026-08-21T16:29:21+08:00` — 补充真实 happy-dom PointerEvent 覆盖，时间轴回归更新为 8 files / 88 tests；最终 range 修复包 `1e271571…` 已重新构建并安装，范围桌面拖动证据仍受 Computer Use 坐标接口限制。
- `2026-08-21T16:36:24+08:00` — 修复最终端点 sticky snap 时序和 Shift range 原始起点，补齐反向/同帧/mark cancel/lost-capture PointerEvent 覆盖；最终包 `03bdcef3…` 已安装。
- `2026-08-21T16:46:00+08:00` — 全量 Web 串行门禁更新为 151 files / 1410 tests passed；时间轴 range slice 仍保持 Ready、整体桌面 QA 仍为 partial。
- `2026-08-21T17:46:17+08:00` — 写回带音频 H.264/AAC 安装版导出、ffprobe、完整解码和首中尾 SSIM；新增 export cleanup/external cancel 代码与 workspace 门禁。最新 cleanup 包的 SavePanel 复测仍记录为 partial，不把旧包成功静默升级。
- `2026-08-21T19:32:27+08:00` — 写回 `a7d98d6` 的 symlink/reparse identity 防护、70 个 export 单测、顺序 workspace 全量门禁和最新安装包 `12e1db…9455d9`；并发 motion 超时保留为环境风险。
- `2026-08-21T19:48:03+08:00` — 写回 `f05bac8`/`478e1b4` 的无选区 split/trim parity、Web 151/1412 全量门禁和最新安装包 `de4cb52b…25405`；Computer Use 因 Mac 锁定未完成屏幕 smoke。
- `2026-08-21T20:09:19+08:00` — 写回 `741ff07` 的缺失媒体 Preview/Playback fail-closed、RenderLoop 集成 8 项和顺序 Rust workspace 全量结果；最新安装包 `40c21ce0…e7f08`，屏幕 smoke 仍受 Mac 锁定阻塞。
- `2026-08-21T20:19:15+08:00` — 写回当前提交 `6b31449` 的顺序 workspace 全量、clippy 既有 lint 边界和最新安装包 `7ad988f3…a9799`；屏幕 smoke 仍受 Mac 锁定阻塞。
- `2026-08-21T20:57:59+08:00` — 写回 `OT-MOTION-ALPHA` 首条透明发布纵切：ProRes 4444 `.mov`、alpha manifest provenance、Motion Studio 开关和 151/1413 Web + Rust workspace 全量证据；最新安装包 `7634979e…607f00`。Mac 锁屏仍阻塞透明发布桌面对拍。
- `2026-08-21T21:21:26+08:00` — Agent Motion 文档新增透明发布参数，重新构建并安装最新 app `38c814f4…bc838`；Computer Use 再次返回 Mac locked，屏幕验收边界不变。
- `2026-08-21T21:31:52+08:00` — alpha-preserving document edit 修复后重新构建并安装最新 app `d5f8aa46…87ae7`；真实 FFmpeg add→edit integration 通过，Computer Use 仍返回 Mac locked。
- `2026-08-21T22:15:18+08:00` — 最新包 `918eac84…9551b` 解锁后完成真实桌面 smoke：紧凑/标准 Home、媒体预览 tab、播放到尾帧、有效分割与撤销、导出面板、Motion 透明开关和透明发布落轨均通过；Option trim 坐标操作返回 `noWindowsAvailable`，保留为未完成屏幕证据。
- `2026-08-21T22:27:41+08:00` — 当前包完成 H.264/AAC SavePanel 导出；输出文件 ffprobe、完整 `ffmpeg -xerror` 和 7.700s A/V 流事实通过。透明 ProRes 专用导出、H.265/字幕/取消仍待。
- `2026-08-21T22:32:49+08:00` — 最新包完成 linked V1/A1 的 Shift+Backspace ripple delete 和 Undo 屏幕对拍：V1/A1 同时移除、Motion clip 保留、撤销恢复三条 clip；Option trim 坐标接口仍未提供可用窗口。
- `2026-08-21T22:37:23+08:00` — 对透明 Motion 发布产物做文件级复核：QA 工程中的 `.mov` 为 ProRes 4444 `ap4h` / `yuva444p12le`、90 帧/3.000s；`alphaextract` 成功读取非全黑 alpha；`media.json` 保留 `generationInput.transparent: true`。透明专用导出 UI、opaque→transparent 编辑和其它编码格式仍保持待验证。
- `2026-08-21T22:47:48+08:00` — 最新包完成 Inspector 视频/音频/AI 编辑页、可撤销翻转和本地 AI 建议应用/撤销；字幕页的缺失转写模型提示；Agent 对话标签新建/关闭、发送禁用和 `⌘⌥A` 收起布局恢复。模型下载、真实转写/字幕导出、MCP/Agent 工具调用仍保持待验证。
- `2026-08-21T23:09:09+08:00` — 最新安装包 `893b6ed0…d0ba` 完成范围删除屏幕对拍：I/O 标记全范围后再选 V1 锚点，Shift+Backspace 删除 V1/A1 并保留 Motion，预览时长和播放头同步到 3.000s，Undo 恢复三条轨道；同步刷新新增 playhead clamp，Web 全量更新为 151 files / 1415 tests。
