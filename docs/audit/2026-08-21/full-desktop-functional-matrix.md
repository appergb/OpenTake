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
last_verified: 2026-08-21T20:57:59+08:00
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
| Home / 标准档 / 小屏 | 不超过工作区，内容有合理最大宽度 | 新构建标准档按 monitor workArea 裁剪到约 `1331×768`，无白边；标准档仍会铺满小屏工作区，Home 最近项目卡留白偏多但不溢出 | 通过（窗口）/部分（Home 内容密度） | `src-tauri/tauri.conf.json`；`web/src/store/settingsStore.ts`；Task 2 5 files / 50 tests；新构建 Computer Use 截图 |
| 设置 → 外观 → 紧凑 | 窗口收缩且内容仍可操作 | 新构建首次启动/切换后约 `1066×666`，紧凑 radio 状态正确，Home/编辑器可见 | 通过 | `settingsStore` + Home/SplitPane/Settings 50 tests；新构建 Computer Use 紧凑档截图 |
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
| Timeline | 入点/出点范围与范围边缘 | `TimelineContainer` 已接入 Shift+标尺 range mark、已有范围 start/end 边缘命中与拖动、clip-edge/playhead snap、pointercancel 回滚和同帧清除；时间轴目录 8 files / 88 tests、tsc、Web build 通过；最终安装包 SHA-256 `03bdcef3…`。Computer Use 音频 QA 工程能显示 V1+A1 和波形，但当前 sky 坐标拖动返回 `noWindowsAvailable`，因此没有把范围拖动及 `Shift+Delete` 的实际工程 diff 算作通过 | 部分 | 需要可复现的安装版坐标拖动证据，并对删除前后 Timeline、Undo/Redo 做状态对拍 |
| Inspector | 文本属性 | 可读出 Welcome to OpenTake、字体、字号、颜色、对齐、阴影、关键帧 | 通过 | 补编辑后保存重开 |
| Preview | 带音频 fixture 的导入/波形/seek/暂停 | `nested-timeline-compound-export-2026-07-31.mp4`：7.700s、H.264 1280×720/30fps/231帧 + AAC 48kHz 单声道；安装版出现 V1+A1、A1 波形和音频 Inspector，已操作起点/中点/终点 seek，连续暂停状态稳定 | 部分 | 实时听感级同步和最新 cleanup 包的导出复测仍待 |
| Preview | compositor temporal 预览 | 最新安装版在 QA 工程中设置 `speed=1.5` 和曝光 `0.50` compositor 属性后不再出现 unsupported surface；画面可见，播放头可到尾帧，暂停后抓帧按钮保持可用；speed/reversed 的首次 native render、JPEG publication、source-frame 映射由 Rust integration tests 锁定 | 通过（代码/native/屏幕组合证据） | 预览与导出起/中/尾帧、音频同步和取消仍在后续闭环 |
| Preview | 多素材 tabs | 打开 `audit-4k60-h264` 与 `export-h264-frame30` 后显示 Timeline/两个媒体 tab；关闭第二个回退第一个 | 通过 | 安装版 AX：两个 Close 按钮；Preview slice 4 files / 91 tests |
| Agent | 面板入口 | Agent 面板、新建对话标签、输入框出现；发送按钮禁用 | 通过 | 不发送外部消息；补本地失败/取消路径 |
| Motion | Studio 入口 | HTML/CSS 编辑器、预览、参数检查器、关键帧时间线出现；播放帧推进 | 通过（既有桌面证据） | 用最新包补发布、保存重开和导入时间线 |
| Motion | 透明发布 | Rust/Web 已贯通透明模板→Chromium alpha→ProRes 4444 `.mov`→manifest straight-alpha provenance；Web 透明背景开关和发布请求测试通过；最新包已安装但 Computer Use 仍因 Mac 锁屏无法点击开关并完成时间线画面对拍 | 部分（代码/自动化通过） | 解锁后执行透明开关、发布、时间线预览/导出和保存重开四步状态对拍 |
| Export | 导出面板 | 导出视频面板可打开，格式/分辨率/取消/导出可见 | 通过 | 复制目标路径后完成导出并 ffprobe |
| Export | temporal 视频实际导出 | 修复 SavePanel filters 后，原生 Save 按钮可用；H.264 temporal QA 导出成功，应用显示 3840×2160 / 160 帧，ffprobe 显示 H.264、60fps、2.666667s | 通过（H.264 视频） | H.265/ProRes、带音频/字幕、取消和 preview/export 像素对拍仍待矩阵化 |
| Export | 带音频 H.264/AAC 实际导出 | 安装包 `03bdcef3…` 生成 `qa-h264-aac-export.mp4`；ffprobe：H.264 1280×720/30fps/231 帧、AAC 48kHz 单声道/362 包、7.700s；音视频均 `ffmpeg -xerror` 通过；源/导出首中尾 SSIM `0.999997 / 0.999997 / 1.000000` | 通过（一次真实 QA 包）/当前包部分 | 当前安装包已更新为 `40c21ce0…e7f08`（代码 `741ff07`），但 SavePanel/取消/最新包重跑仍待；H.265/ProRes、字幕和实时音画同步仍待 |
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
- 全量 Web 串行门禁：151 files / 1412 tests passed；运行时使用已初始化 localStorage file 和单 worker，包含无选区 split/trim no-op 回归。
- 本轮 alpha slice Web 串行门禁：`NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage.json pnpm exec vitest run --pool=forks --maxWorkers=1` → 151 files / 1413 tests passed；`pnpm build` exit 0。
- Rust workspace：`cargo test --workspace --jobs 1 -- --test-threads=1` 与 `cargo fmt --all -- --check` 通过；本轮包含缺失媒体 Preview/Playback fail-closed、透明 Motion ProRes 4444 集成和 RenderLoop 集成测试。全 workspace clippy 仍被既有 `chunks_exact`/`chunks_exact_mut` lint 阻塞，当前输出涉及 `opentake-media`、`opentake-motion` 等旧模块；一次未限并发运行在 motion sandbox 180s 超时，单独串行重跑通过，保留为 GPU/Chromium 资源争用风险；仅保留明确标记为 ignored 的 real-device / optional fixture tests。
- 最终 `.app`：`web/node_modules/.bin/tauri build --bundles app` exit 0；当前安装包 SHA-256 `38c814f4ac4fddc729f2f3733c637dfd171a8c67ad1bfa7195257d58033bc838`，构建产物 hash 一致；本轮 Computer Use 屏幕 smoke 因 Mac 锁定 blocked，媒体 Preview tabs/小屏等桌面证据仍引用之前的已完成专项包，透明 Motion 开关/发布也未升级为桌面通过。

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
