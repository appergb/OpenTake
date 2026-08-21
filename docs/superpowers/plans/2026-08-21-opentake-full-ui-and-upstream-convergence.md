# OpenTake Full UI And Upstream Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 OpenTake 在小屏上的窗口和布局问题，并以可复现的模块化证据把上游 Palmier Pro 与 OpenTake 自有能力收敛为可正常使用的完整产品。

**Architecture:** 以 capability ledger 作为唯一收敛清单，每项能力沿 Rust domain → EditCommand/事务 → Tauri 命令 → React UI/MCP → 预览/播放/导出走一条可验证的纵切。窗口几何由 Tauri 与 CSS 共用一套可测量契约；桌面功能使用安装后的 `.app` 做真实操作，自动化测试只作为补充。所有上游代码只读，所有实现只落在 `OpenTake-generation/`。

**Tech Stack:** Rust workspace, Tauri 2, React 18, TypeScript, Zustand, Vite, FFmpeg, wgpu, cpal, Vitest, Cargo test/clippy/fmt, macOS Computer Use。

**Spec:** `docs/superpowers/specs/2026-07-10-opentake-full-convergence-design.md`、`docs/superpowers/specs/2026-08-13-opentake-beta5-design.md` 与本轮用户目标。

**Current checkpoint (2026-08-22, Asia/Shanghai):** 受影响 crate/Tauri/Web 定向门禁保持全绿；最新安装包二进制 SHA-256 `acd7b105a75d957bc761f9128f9f89e7ee563f65031dc7e22952fa7a6c65c319`，已重新打包并安装。此前包已完成小屏、媒体/预览、时间轴范围删除与 Undo、H.264/H.265/ProRes 422、Motion 透明发布的既有屏幕证据。上游审计确认的两个缺口已在代码侧补齐：Agent 文件夹批量 `entries` 单步 Undo，以及 JSON/`.lottie` Lottie 导入、Velato 校验和 metadata。当前 Mac 仍处于锁屏，透明 ProRes 4444 专用 SavePanel、Option trim、字幕 SavePanel/模型下载、Lottie 屏幕路径和取消导出的最新安装版动作不能伪造为已完成。2026-08-22 workspace 全量复跑在既有 4K Chromium budget smoke 超时并 poison gate，已确认与本次 diff 无直接关系；clippy 仍被既有 `chunks_exact` lint 阻塞。工作区只保留用户已有审计素材的 dirty/untracked 状态。

## Global Constraints

- 只修改 `/Users/trip/TRUE 开发/PRIMARY-CN/OpenTake-generation`；`OpenTake/` 只作对照；`palmier-pro-upstream/` 永远只读。
- Rust 持有权威 `Timeline`；所有公共编辑时间使用整数帧；前端只保存镜像和 UI 状态。
- UI、MCP、应用内 Agent 都必须汇聚到同一条编辑命令/事务路径，撤销不能跨越新的用户编辑。
- 预览、播放、截图和导出共享 RenderPlan 语义；不支持的内容必须显式报错，不能静默消失。
- 所有 serde 模型保留旧工程兼容性；未知字段不得被静默丢弃。
- 真机验收使用复制出来的 QA 工程和测试媒体，不改用户原始项目/媒体，不提交 sidecar 二进制。
- 不记录或输出 token、密钥、Cookie、私有路径中的敏感内容；外部生成能力没有凭据时只验证失败边界。
- 每个代码切片先跑最相关的小测试，再跑受影响模块测试；最终必须复核 diff、构建、测试、clippy/fmt 和安装包 UI 证据。
- 当前 worktree 的 `.git` 文件仍指向旧用户路径；在提交、分支切换或合并前必须先完成只读元数据核查并确定可用的 Git 路径，不能假装已有提交证据。

---

### Task 1: 建立当前基线、能力清单和证据目录

**Files:**
- Create: `docs/capabilities/requirements.json`
- Create: `docs/capabilities/CAPABILITY-LEDGER.md`
- Create: `docs/capabilities/README.md`
- Modify: `docs/INDEX.md`
- Modify: `AGENTS.md`

**Interfaces:**
- `requirements.json` 为每项能力提供稳定 ID、上游来源、OpenTake 代码路径、自动化测试、桌面场景、状态和证据位置。
- `CAPABILITY-LEDGER.md` 汇总按模块的人工审计结论；README 与 release 文档只能引用状态为 `verified` 的能力。

- [x] **Step 1: 固定本地事实**

  记录当前分支/工作树可见状态、安装包路径和哈希、Cargo/Web 版本、Tauri 窗口配置、已有 Beta 5 审计入口；Git 指针异常单独登记为环境风险。

- [x] **Step 2: 导入上游能力索引**

  遍历 `palmier-pro-upstream/Sources/PalmierPro/` 的 Models、Editor、Timeline、Preview、Export、Agent、MediaPanel、Inspector、Settings、Help、Toolbar 和 Engine，给每个用户可见行为分配稳定 `UP-*` ID。

- [x] **Step 3: 登记 OpenTake 增强项**

  为 Context Signal、工作流插件、全局素材库、Motion Studio、反向/冻结帧、EDL/OTIO/SRT/VTT、BYOK 生成和播放恢复分配 `OT-*` ID，保留已有历史 ID，不覆盖旧记录。

- [x] **Step 4: 校验清单格式和可达性**

  使用 JSON 解析器检查 ID 唯一、路径存在或明确标记缺失；从 `docs/INDEX.md` 能到达 ledger 和证据目录。

  Run: `python3 -m json.tool docs/capabilities/requirements.json >/dev/null`

- [x] **Step 5: 记录基线验证**

  Run: `pnpm -C web test -- --runInBand`（若 Vitest 不接受该参数则改为 `pnpm -C web test`）；`cargo test --workspace`；`pnpm -C web build`。

  将实际结果和无法执行的环境原因写回 ledger，不把历史文档中的绿色结果复制成新证据。

### Task 2: 修复小屏窗口默认值和响应式布局

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `web/src/store/settingsStore.ts`
- Modify: `web/src/store/settingsStore.test.ts`
- Modify: `web/src/styles/tokens.css`
- Modify: `web/src/styles/global.css`
- Modify: `web/src/components/home/HomeView.tsx`
- Modify: `web/src/components/shell/EditorSplit.tsx`
- Modify: `web/src/components/shell/SplitPane.tsx`
- Modify: `web/src/components/settings/SettingsView.tsx`
- Test: `web/src/components/home/HomeView.visual.test.ts`
- Test: `web/src/components/shell/SplitPane.interaction.test.tsx`
- Test: `web/src/components/settings/SettingsView.visual.test.ts`

**Interfaces:**
- `applyWindowSize(size)` 仍接受 `"standard" | "compact"`，但最终尺寸不得超过当前 monitor 可用工作区，并且标准/紧凑选择在失败时回滚。
- CSS titlebar safe-area、面板 min-width/min-height 和首页卡片必须在 1066×666、1280×720、1330×768 三个逻辑尺寸下可测量。

- [x] **Step 1: 先把问题变成可回归的几何断言**

  在 Home、EditorSplit、SplitPane 的视觉/交互测试中加入 viewport 约束，断言没有横向溢出、没有固定 1600×1000 的最小内容、项目预览仍保持 16:9、侧栏和时间轴能滚动而不撑大外层。

- [x] **Step 2: 收敛窗口启动策略**

  把静态启动尺寸改成小屏安全尺寸；`applyWindowSize` 查询可用 monitor 后对标准尺寸做上限裁剪，保留用户选择并保持居中。紧凑档继续提供约 2/3 的窗口，但两档都必须服从屏幕工作区。

- [x] **Step 3: 收敛 CSS 约束**

  对所有外层 flex/grid 容器补齐 `min-width: 0` / `min-height: 0` / `overflow` 边界，移除会把首页项目卡片拉满窗口的固定高度；编辑器在小屏时优先压缩面板、再允许内部滚动，不通过缩放整个 WebView 解决。

- [x] **Step 4: 在真实打包 app 上验证**

  Build: `pnpm -C web build`；`cargo tauri build`。安装到 `/Applications/OpenTake.app` 后，用 Computer Use 依次验证标准/紧凑、小窗口拖动、首页、编辑器、设置、素材库和 Motion 页面。

- [x] **Step 5: 写回布局证据**

  在 `docs/capabilities/CAPABILITY-LEDGER.md` 登记每个尺寸的截图路径、窗口逻辑尺寸、溢出检查结果和剩余限制；只把有打包 app 证据的窗口能力标为 `verified`。

  结果：Task 2 已通过四轮 review、5 个 Web 测试文件 50/50、Web build、fmt、release `.app` 及 Computer Use 紧凑/标准切换；DMG 尾脚本和全量 Web 的既有 uiStore 失败单独登记。

### Task 3: 完成桌面功能巡检矩阵并固化 QA 夹具

**Files:**
- Create: `docs/audit/2026-08-21/full-desktop-functional-matrix.md`
- Create: `docs/audit/2026-08-21/fixtures/README.md`
- Modify: `docs/capabilities/CAPABILITY-LEDGER.md`
- Modify: `scripts/` 中与 QA 夹具清理/验证相关的脚本（仅在已有脚本无法复用时）

**Interfaces:**
- 每个场景包含入口、前置夹具、动作序列、期望结果、实际结果、证据和清理动作。
- 模块范围至少覆盖 Home/项目生命周期、媒体库/导入/预览、时间轴编辑、Inspector/关键帧、视频预览/播放/seek、字幕、导出、设置、Agent/MCP、Motion Studio。

- [x] **Step 1: 接收 UI agent 的初始报告**

  合并其真实操作过的动作和阻塞原因，不重复把“AX 元素存在”当成“功能通过”。

- [x] **Step 2: 准备可重复 QA 工程**

  使用复制的 fixture 媒体和新建的 `.opentake` 工程，包含至少一个视频、一个音频、一个图片、一个文字/字幕片段和一个关键帧场景；原始文件保持只读。

- [x] **Step 3: 编写模块化场景**

  每个场景只验证一个可判定行为，失败时能定位到 domain、命令、Tauri、UI 或媒体引擎层。

- [x] **Step 4: 固化失败证据**

  截图、日志、ffprobe 摘要和测试命令按场景编号存放；敏感路径和凭据脱敏。

  结果：首轮和第二轮 UI agent 报告已合并；窗口、项目生命周期、文件/文件夹/relink 导入、视频素材预览、播放/seek、有效分割/撤销、Inspector、设置、Preview tabs、导出面板和帮助均有证据；媒体 folder/flat/grouped 三态、网格/列表密度、真实文件夹导航和音频导入/收藏子页也已在安装版验收；Lottie JSON/`.lottie` 已补代码/自动化导入与 metadata，安装版 Lottie 卡片/预览/落轨/重开、音频独立预览、全局素材库/Motion 的稳定截图、最终导出仍在后续矩阵。

### Task 4: 收敛 Models、EditCommand 和时间轴编辑 parity

**Files:**
- Modify: `crates/opentake-domain/src/`
- Modify: `crates/opentake-ops/src/`
- Modify: `src-tauri/src/`
- Modify: `web/src/store/editActions.ts`
- Modify: `web/src/components/timeline/`
- Test: 对应 crate `#[cfg(test)]` 模块和 `web/src/**/*.test.*`

**Interfaces:**
- 所有用户编辑仍通过 `EditCommand`/事务入口；UI 与 MCP 对同一命令序列产生相同 Timeline。
- 关键帧以内存片段相对帧存储，对外 API 使用绝对帧；split、trim、speed、fade、link、ripple、snap、undo/redo 均保持上游语义。

- [ ] **Step 1: 逐项读取上游模型和 Editor 符号**

  以 Task 1 的 `UP-MODELS-*` 和 `UP-EDITOR-*` 为输入，为每个缺失/部分项锁定单一 Rust/TS 写入范围。

- [ ] **Step 2: 为每个缺口先写失败单测**

  测试包含边界帧、空选择、链接音视频、关键帧切割连续性、变速源帧、波纹拒绝和撤销栈归属。

- [ ] **Step 3: 用最小实现使测试通过**

  不引入绕过 `EditCommand` 的 UI 特判；domain 保持零 IO，边界层转换为 `Err(String)`。

- [ ] **Step 4: 做上游对拍**

  使用同一 project JSON 和等价命令序列，比较 Timeline、片段区间、关键帧和 linked A/V 结果；把差异写回 ledger。

- [ ] **Step 5: 连接时间轴真实交互**

  验证选中、拖动、修剪、分割、删除、复制/粘贴、范围、轨道操作、吸附、播放头和键盘快捷键，确保不只是在 store 测试里通过。

  当前进展（2026-08-21）：`TimelineContainer` 已完成一个 bounded range slice：Shift+标尺 range mark、已有范围 start/end 边缘命中/拖动、clip-edge/playhead snap、pointercancel/lostpointercapture 回滚、同帧清除和最终端点 sticky snap 时序；新增 happy-dom PointerEvent 测试后时间轴目录为 8 files / 88 tests，tsc、Web build 通过。最终安装包 `03bdcef3…` 已安装，但 Computer Use 坐标拖动返回 `noWindowsAvailable`，范围拖动、`Shift+Delete` 以及删除前后 Undo/Redo 的安装版对拍继续保持未完成。

  2026-08-22 窄范围上游对拍确认 Rust `ripple_delete`、`ripple_delete_ranges_on_track`、`ripple_insert`、`trim_clips`、linked partner propagation 和 command 原子拒绝语义与上游一致；现有 ops/command 测试覆盖这些边界。前端 `web/src/components/timeline` 尚未完成逐函数对拍，因此 Timeline 能力仍保持 partial，不能仅凭 Rust parity 升级完成。

  前端代码基线（2026-08-22 01:20）：TimelineContainer、范围交互、TrackHeader sync-lock、overlay、range context menu、clip context menu 定向门禁 6 files / 55 tests 通过；这只证明当前前端路由和交互单测绿，不替代安装版 Timeline 全流程屏幕对拍。

  2026-08-22 01:28：新增 `editActions.test.ts` 的 ripple action routing 覆盖，选中片段、标记范围、选中 gap 和 out-of-band filled-gap 拒绝 42/42 通过；同时锁定前端 `rippleDeleteRanges` 的实际 wire 形状 `{start,end}`。仍不替代安装版屏幕验收。

### Task 5: 收敛预览、播放、RenderPlan 和导出闭环

**Files:**
- Modify: `crates/opentake-render/src/`
- Modify: `crates/opentake-media/src/`
- Modify: `src-tauri/src/export.rs`
- Modify: `src-tauri/src/preview.rs` 或当前预览命令实现
- Modify: `web/src/components/preview/`、`web/src/components/timeline/`
- Test: RenderPlan、composite_frame、playback、export 相关 Rust/Web 测试

**Interfaces:**
- 任意时间线帧可稳定 seek；暂停、播放、scrub、尾帧保持、音画同步和取消行为有明确结果。
- 预览和导出使用同一合成语义；H.264/AAC、H.265/ProRes、字幕和不支持内容分别有能力门控与失败证据。

- [x] **Step 1: 建立代表性媒体 fixture**

  准备短视频、带音频视频、透明/静态图片、文字和字幕输入，并用 ffprobe 固定帧率、时长、分辨率和音轨事实。

  结果：带音频 fixture `nested-timeline-compound-export-2026-07-31.mp4` 已固定为 7.700s、H.264 1280×720/30fps/231 帧、AAC 48kHz 单声道/362 包；源视频和音频均通过 `ffmpeg -xerror`。

- [ ] **Step 2: 验证单帧合成**

  对首帧、中帧、尾帧和关键帧边界做 RGBA/PNG 证据；任何缺失片段返回结构化错误而不是黑帧成功。

- [x] **Step 3: 验证播放和 seek**

  在安装包中执行播放/暂停/恢复/scrub/seek/尾帧保持，记录实际画面和音频状态；失败按解码、时钟、UI ref、合成或资源回收分类。

  结果：安装版 QA 工程出现 V1/A1 和 A1 波形，起点/中点/终点 seek 与暂停状态均有截图；实时听感级 A/V 同步仍未证明。

- [ ] **Step 4: 验证导出**

  导出后用 ffprobe、完整解码和首/中/尾帧检查；音频轨、时长、取消不提交和失败清理必须分别验证。

  当前结果：安装包 `893b6ed0…d0ba` 已真实生成 H.265/AAC 与 ProRes 422 HQ/PCM 文件；H.265 为 HEVC `hev1` + AAC，ProRes 为 `apch`/`yuv422p10le` + PCM，两者均为 7.700s、完整 `ffmpeg -xerror` 通过；`cca9327` 新增透明 ProRes 4444 codec、alpha 清屏和真实 GPU/FFmpeg integration，`export_prores_4444_preserves_transparent_text_alpha` 通过；`0bae80e` 补了普通输出失败清理和 external cancel，`a7d98d6` 补了最终输出 symlink/reparse identity 拒绝。当前 export unit 72/72、media cancel 18/18、audio/video integration 6/6 和顺序 Rust workspace 全量通过；本步骤保持 partial，透明 ProRes 4444 SavePanel 屏幕、字幕、取消、实时音画同步和首中尾对拍仍待。

  下一执行切片：Linked A/V Timeline Parity。对齐 `crates/opentake-ops/src/ops/linking.rs`、`trim.rs`、`move_clips.rs`、`ripple.rs`、`command.rs` 与 `web/src/components/timeline/TimelineContainer.tsx`、`web/src/lib/editActions.ts`、`web/src/hooks/useKeyboardShortcuts.ts`；linked move Agent parity 和 Option trim only-main 已补齐并通过自动化，继续补 linked trim/ripple sync-lock/atomic collision、Shift+Delete Undo/Redo，再做安装版状态对拍。

  已完成的 parity slices：上游 `splitAtPlayhead`、`trimStartToPlayhead`、`trimEndToPlayhead` 无选区 no-op；Agent `move_clips` linked partner delta；Timeline Option/Alt trim only-main（普通 trim 仍同步 linked group）。Web 全量当前 `151 files / 1415 tests` 通过。最新包屏幕已完成有效分割/撤销、播放到尾帧，以及按上游顺序的 I/O 范围标记→锚点选中→linked V1/A1 Shift+Backspace ripple delete/Undo；同步刷新新增 playhead clamp，删除后时间码与预览时长保持一致。Option trim 坐标操作仍返回 `noWindowsAvailable`，透明 ProRes 专用导出仍待。

  同期补齐缺失媒体错误边界：提交 `741ff07` 让普通 Preview、Playback Image/Text 和 RenderLoop 集成路径都返回显式 materialization error，不再发布黑帧；render 18、resolver 15、playback integration 8、Tauri lib 719 和顺序 workspace 全量通过。`6b31449` 收口导出 clippy 建议；全 workspace clippy 仍有既有 `chunks_exact` lint。当前安装包 `7ad988f3…a9799` 已重建，最新屏幕 smoke 仍因 Mac 锁定未完成。

### Task 6: 收敛媒体、Inspector、字幕和项目生命周期

**Files:**
- Modify: `crates/opentake-media/src/`
- Modify: `crates/opentake-project/src/`
- Modify: `src-tauri/src/library.rs`, `src-tauri/src/project.rs`, `src-tauri/src/captions.rs`
- Modify: `web/src/components/media/`, `web/src/components/inspector/`, `web/src/store/`
- Test: 媒体、库、文件夹、重链接、Inspector、字幕和 save/reopen 相关测试

**Interfaces:**
- 导入白名单、缩略图、波形、嵌套文件夹、收藏/去重、缺失/重链接、预览媒体、Inspector 变换/裁剪/音量/关键帧、SRT/VTT 和 save/reopen 共享同一项目状态。

- [ ] **Step 1: 对码 MediaPanel/Inspector/Settings 上游能力**

  把 UI agent 和 parity agent 的结果映射到具体文件和命令，不以置灰按钮作为完成。

- [ ] **Step 2: 补全媒体夹具和非破坏性错误路径**

  覆盖缺失文件、错误类型、坏媒体、重复导入、取消导入、波形失败、重链接和文件夹重启恢复。

- [ ] **Step 3: 验证 Inspector 写入命令**

  逐项验证静态值、关键帧、裁剪、变换、音量、淡变、文字/字幕样式和批量同步，确保刷新/重开后仍一致。

- [ ] **Step 4: 验证项目边界**

  新建、打开、保存、另存、自动保存、退出 flush、切换工程、恢复和旧 schema 读取都必须有可复现证据。

  当前进展（2026-08-22）：字幕纵切的代码/自动化证据已补齐：`cargo test -p opentake-domain subtitle_export` 16/16、`cargo test -p opentake-tauri subtitle_export_tests -- --nocapture` 5/5、`CaptionsTab.test.tsx + TitleBar.visual.test.ts` 10/10；安装版模型下载、转写结果编辑和 SavePanel SRT/VTT 导出仍须在解锁后补桌面证据。

  同期补齐上游媒体/Agent 缺口：`create_folder.entries` 与 `move_to_folder.entries` 通过 `CreateFolders` / `MoveToFolders` 在一个 EditCommand 事务中执行，Agent 返回上游约定形状；core 现在识别 `.json` / `.lottie` 为 Lottie，Tauri 用 Velato 校验 JSON、从 `.lottie` ZIP 的 `animations/*.json` 提取 composition 并写入尺寸/帧率/时长。Agent lib 417/417、ops 209/209、Lottie JSON/容器/MCP path 定向测试通过；最新安装包屏幕路径待解锁。

### Task 7: 收敛 Agent/MCP、生成、Motion 和设置/帮助能力

**Files:**
- Modify: `crates/opentake-agent/src/`, `crates/opentake-gen/src/`
- Modify: `src-tauri/src/mcp.rs`, `src-tauri/src/chat.rs`, `src-tauri/src/motion.rs`, `src-tauri/src/settings.rs`
- Modify: `web/src/components/agent/`, `web/src/components/motion/`, `web/src/components/settings/`, `web/src/components/help/`
- Test: dispatcher、MCP、chat、generation、motion、settings、help 的单测/集成测

**Interfaces:**
- 能力不可用时返回结构化原因；Agent/UI/MCP 共用 dispatcher；生成作业可启动、取消、失败、恢复和物化；本地编辑不依赖网络或凭据。

- [ ] **Step 1: 生成完整工具能力表**

  将上游 Agent 工具逐个映射到 dispatcher、参数校验、命令、项目作用域、撤销行为和测试；没有真实调用链的工具不得标为已完成。

  当前进展（2026-08-21）：发现并修复 Agent/MCP `move_clips` 与上游不一致的入口缺口：显式 `toFrame` 现在会通过同一原子 `MoveClips` payload 传播 frame delta 到 linked A/V partner，track-only move 不传播；新增 frame-propagation 和 track-only 回归测试，Agent lib 415/415 通过。继续按同一方法审计其余 Agent/MCP 工具入口。

  2026-08-22 上游审计又确认并修复两个入口缺口：批量文件夹参数此前已广告但 dispatcher 返回 not implemented，现改为单事务 `CreateFolders` / `MoveToFolders`；普通媒体导入此前排除 JSON/Lottie，现已接入 Lottie JSON 和 `.lottie` ZIP container，坏文档 fail-soft。剩余 Agent/MCP 重点是安装版本地调用证据与安全失败边界，不再把已确认的两个缺口留在“待实现”。

- [ ] **Step 2: 验证 MCP 安全边界**

  覆盖 initialize、Origin/Host、随机端口/凭据、工程切换隔离、取消、stale hash、错误结构和旧凭据失效。

- [ ] **Step 3: 验证应用内 Agent 和生成**

  覆盖会话切换、工具卡、流式取消、BYOK 缺失、provider 失败、成本确认和真实/模拟 provider 边界；不提交付费请求。

- [ ] **Step 4: 验证 Motion Studio**

  覆盖编辑、预览、发布视频、取消、结果校验、导入时间线、保存重开和预览/导出一致性。

  当前进展（2026-08-21）：`OT-MOTION-ALPHA-1` 的透明模板、Chromium alpha、ProRes 4444、manifest provenance、Motion Studio/Agent 发布开关、alpha-preserving edit 和透明导出 codec 均已实现并有自动化/真实 FFmpeg 证据。最新包 `3ce3c1d0…c7eef` 已完成透明开关、发布进度、时间线落轨、保存重开和 Inspector/预览屏幕验收；导出面板新增 `ProRes 4444 透明 / .mov`，GPU/FFmpeg integration 验证透明清屏、`yuva444` 和 alpha 0→非零；同包 H.264/AAC、H.265/AAC、ProRes 422/PCM 文件证据均通过。Web 全量 `151 files / 1416 tests`、`pnpm build`，Rust workspace 722 tests 通过。仍未勾选本步骤：透明 ProRes 4444 屏幕 SavePanel/状态复验（Mac 当前锁屏）、opaque clip 在编辑时切换透明，以及字幕/取消导出矩阵仍待。

- [ ] **Step 5: 验证设置和帮助**

  覆盖窗口大小、代理播放、provider 选择、密钥仅进钥匙串、快捷键/MCP 帮助和无网络启动。

### Task 8: 完成模块化回归、文档写回和交付门禁

**Files:**
- Modify: `docs/capabilities/requirements.json`
- Modify: `docs/capabilities/CAPABILITY-LEDGER.md`
- Modify: `docs/audit/2026-08-21/full-desktop-functional-matrix.md`
- Modify: `docs/INDEX.md`, `AGENTS.md`, 受影响模块 `OVERVIEW.md`/`INDEX.md`
- Modify: `CHANGELOG.md`（仅在真实交付切片完成时）

**Interfaces:**
- 每项能力状态只能是 `implemented`、`verified`、`partial`、`missing` 或 `blocked`；`verified` 必须有最新自动化或桌面证据。
- 文档入口、当前分支/工作树摘要、已知风险、执行计划和代码现实保持一致。

- [ ] **Step 1: 按模块运行测试矩阵**

  依次运行 domain/ops/project/render/media/agent/gen/core/src-tauri 的 `cargo test`，Web `pnpm -C web test`、`pnpm -C web build`，再运行 `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets --all-features -- -D warnings`（环境允许时）。

- [ ] **Step 2: 复跑桌面 smoke**

  重新安装打包 app，执行首页、设置、素材导入/预览、时间轴编辑、播放/seek、导出、素材库、Agent、Motion 的最小路径；截图和日志引用对应 capability ID。

- [ ] **Step 3: 做最终 diff 和一致性审查**

  检查无上游修改、无 sidecar 二进制、无秘密、无无关文件、无未记录失败；检查文档内部链接和 JSON ID 唯一。

- [ ] **Step 4: 形成下一轮可恢复入口**

  更新本计划的里程碑、已完成模块、待处理 gap、阻塞证据和下一组 disjoint write scopes；如果仍有未完成项，不声称全功能完成。

## Execution Order And Parallel Boundaries

1. Task 1 与 Task 3 的审计材料可并行；Task 2 只在 UI agent 的初始布局报告收敛后实施。
2. Task 4、Task 5、Task 6、Task 7 按 gap matrix 拆成不重叠写入范围；domain/ops 先于 UI 接线，render/playback 先于最终桌面验收。
3. 每个实施 agent 必须返回：结论、证据、修改文件、测试命令/结果、未解决问题和文档写回点；主线复核后再进入下一切片。
4. UI 真机 agent 只负责观察/操作/证据，不与代码实施 agent 共享写入文件；需要修复时由主线或独立实现 agent 按模块接手。

## Definition Of Done

- 小屏窗口不再启动为超出工作区的尺寸，Home/编辑器/时间轴/预览在目标尺寸下没有溢出并且可操作。
- 每个上游能力和 OpenTake 增强项都有稳定 ID、实现状态、自动化测试和桌面场景；缺失或阻塞项有具体证据。
- 剪辑、时间轴、预览/播放、导出、媒体、Inspector、字幕、Agent/MCP、Motion、设置和项目生命周期均完成分模块回归。
- 代码、测试、安装包行为和 docs/ledger 一致；最终验证命令和失败项真实可核验。
