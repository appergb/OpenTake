# Changelog

本文件记录 OpenTake 的重要改动。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/)。

## [1.0.0-beta.2] — 2026-08-03

### 新增（Added）

- ChatGPT 的 AI 登录新增官方 Codex CLI 直接登录；登录、退出和状态完全交由官方 CLI 管理，
  OpenTake 不读取、复制或保存 ChatGPT 凭据。
- 素材放置、复制/移动到新轨和完整片段粘贴改为单一 Rust 事务，支持根/嵌套序列、链接音视频、
  字幕组、转场、文本与 compound 字段的身份重映射和一次撤销。
- 播放头多选/链接片段分割和画布 Transform 写入新增原子命令；整批预检后只产生一次版本推进和
  一次撤销，动画 Transform 在当前帧写入关键帧。

### 修复（Fixed）

- 修复跨工程、另存为、序列切换和外部版本推进期间的迟到媒体/AI 结果、队列串扰与未处理
  Promise；失败保持原状态并显示可见错误。
- 修复极端帧值、trim、ripple、split、nested sequence 和 script 组装可能触发的整数溢出、
  部分写入或 ID 消耗；Rust 与浏览器 fallback 现在均先完整预检再原子提交。
- 保持 Home / Library / Editor mounted state 与滚动位置，补齐弹窗焦点圈、24px 有效命中区、
  小窗口分栏和 reduced-motion 行为。
- 修复预览逐帧在片段边界回跳、非零起点音量关键帧坐标、末端关键帧拖动、razor 边界拒绝、
  片段外动画字段误写，以及 Inspector/字幕控件缺少上下文可访问名称。

### 安全与可靠性（Security / Reliability）

- 官方 Codex 每轮使用独立、工程绑定、带 256-bit Bearer 的临时 loopback MCP；切工程、取消、
  deadline 和进程树清理均 fail closed。未认证的固定端口外部 MCP 在本 Beta 默认关闭。
- 导入路径使用 retained capability、no-follow/reparse 检查、目录预算、可取消 ffprobe；远程 URL
  拒绝内网/环回/保留地址、逐跳重解析并固定 DNS 结果，禁用代理绕过。
- Agent 所有模型可见结果改为白名单 DTO；本机路径、签名 URL、provider request ID、prompt、
  hash 与底层诊断不会进入 MCP 返回或工程聊天记录。

### Beta 已知边界

- macOS 包仍为本地 ad-hoc Beta，不是 Developer ID 签名/公证分发包。
- Windows Job Object、reparse-point 与安装器运行测试由候选提交的 exact-SHA Windows CI 执行；
  macOS 本机结果不能替代真实 Windows 证据。
- Claude、Cursor 等外部 MCP 客户端连接等待后续带身份验证的显式配对流程；官方 Codex / ChatGPT
  登录使用独立的逐轮认证通道，不受影响。

## [1.0.0-beta.1] — 2026-08-01

OpenTake 的第一个可安装 Beta。核心闭环为：创建/打开工程 → 导入与管理素材 →
多轨剪辑、字幕、关键帧、特效/蒙版/调色/转场 → 本地 AI 与生成式 AI 审阅工作流 →
预览、保存重开、H.264/H.265/ProRes 与字幕/交换格式导出。

### 新增（Added）

- 可恢复的工程持久化、全局素材库、缩略图/波形/代理媒体、缺失素材重链接。
- Rust/WGPU 预览与导出共享合成路径，支持文本、调色、绿幕、蒙版、LUT、HSL、
  Lift/Gamma/Gain、通用特效、交叉溶解、嵌套时间线、补帧与防抖。
- Agent/MCP 编辑、内置聊天、官方 Codex CLI / ChatGPT 登录、BYOK 生成作业、Motion Canvas
  动效与原生 Chromium fallback。Codex 登录态完全由官方 CLI 管理，OpenTake 不读取或保存令牌。
- 本地口播清理、响度统一、降噪、声部分离、RVM 抠像、智能擦除、参考色彩匹配和
  可视化运动追踪；字幕翻译、图文成片、数字人与音色克隆提供审阅/同意/成本边界。
- 完整键盘、焦点、菜单、拖拽、撤销/重做与辅助功能回归门禁。

### 安全与可靠性（Security / Reliability）

- 生产 CSP、最小 asset scope、凭据 keychain 边界、URL/重定向/大小限制、下载校验与
  项目修订原子提交。
- macOS/Linux/Windows 安全文件系统契约、打包 FFmpeg sidecar 供应链校验、取消与失败
  清理、生成/导出恢复与防陈旧结果提交。

### Beta 已知边界

- 本地 macOS Beta 包未使用 Developer ID 签名或 Apple 公证；首次打开需要用户明确允许。
- 数字人、音色克隆和通用云生成需要用户自己的 provider key，并可能产生第三方费用；
  Agent 可选择 provider key，也可复用官方 Codex CLI 的 ChatGPT 登录。无可用登录或 key 时
  功能会显式不可用或拒绝，不会静默调用。
- Windows 安装包由精确 SHA CI 构建和验证；原生 Windows WebView 的最终人工交互烟测仍是
  平台发布门槛，不影响本次 Apple Silicon macOS 本地 Beta。
- 任意 Motion Canvas TSX、透明动效、神经语义级任意人声分离等属于后续 Beta 范围。

## 历史开发记录 — 2026-06-23 第三轮（已由 Beta 1/2 状态替代）

> 本节及后续“未完成”表记录当时的开发现场，不代表 Beta 2 的当前阻断项。

本轮为**自动 PR 审核流程**:逐 PR 专家审核 + 对抗验证 + 对照开发文档,审核通过且 CI 双绿的纯新增项合并,其余 @作者 rebase/修改。

### 新增（Added）

- **全局可复用素材库后端**（#37）:
  - **#37-A / #54**（PR #104）后端存储层 `crates/opentake-media/src/library.rs`:copy-on-favorite + SHA-256 内容寻址去重 + JSON manifest 原子写（`.tmp` rename）+ Mutex 并发安全。9 个单测。
  - **#37-B / #55**（PR #106）Tauri 命令层 `src-tauri/src/library.rs`:7 个命令 `library_list` / `favorite` / `unfavorite` / `categorize` / `rename` / `delete` / `import_to_project`。
  - **#37-C / #56**（PR #115)前端 `web/src/components/media/LibraryView.tsx` + `libraryStore`/`libraryApi`:分类树（视频/音频/音效/图片/特效/自建）+ 网格 + 搜索 + 排序 + 跨视图聚合 + 音效库;Home/TitleBar 入口;前端↔后端 7 命令契约已核实。**#37 epic 收口**(后端 #104/#106 + 前端 #115)。
  - 上游 palmier-pro 无此模块,#37 为 OpenTake 新增子系统,不要求 1:1。剩:库→时间线拖拽（现用「导入当前项目」按钮)、媒体面板「星标→library_favorite」接线、收藏从 localStorage 迁后端。
- **文本工具 MVP**（#96,PR #107）:Toolbar `T` 按钮接线 `addTextClip()`、新增 `TextTab.tsx` 文字内容编辑、Inspector 路由 text tab。字体/字号/颜色等 `textStyle` 留后续（依赖后端 ClipProperties 扩展）。
- **SRT/VTT 字幕导出纯逻辑**（#29 D 层切片,PR #110）:`crates/opentake-domain/src/subtitle_export.rs` 把按 `caption_group_id` 分组的字幕 clip 序列化为 SubRip/WebVTT 字符串（零 IO、零新依赖、16 单测）。导出层/agent 工具/前端对话框留后续切片。
- **整条时间线视频导出编排**（Phase 5 spine,PR #112）:`src-tauri/src/export.rs` + `export_video` 命令,逐帧 `Compositor::render_to_rgba` → `VideoEncoder::push_frame` → `finish`,全分辨率 H.264/.mp4（H.265/ProRes/进度取消留后续）。含 ffmpeg/GPU 门控集成测试。自包含复制 preview 路径,未碰 `composite_frame`。
- **导出线性音频混音**（Phase 5,PR #117）:`crates/opentake-media/src/encode/mix.rs` 逐 clip 单声道 PCM 按帧偏移铺放 + `volume_at` 增益包络 + 叠加 + 硬限幅;`VideoEncoder::finish()` 第二趟 ffmpeg mux AAC（`-c:v copy`/`-shortest`,mux 失败回退视频-only),真接通原 dead `push_audio`;含 AAC 轨 + 视频-only 不漏音轨 + 临时件清理集成测试。无音频时仍产出与 #112 相同视频-only 文件。
- **`list_models` 工具接线**（#9/#10 切片,PR #111）:`opentake-agent` 从存根接 `opentake-gen` 内置静态 catalog,`?type=` 过滤 + `{ models, loaded }` JSON,纯本地无网络/BYOK。
- **caption-group 样式批量同步纯逻辑**（#29 切片,PR #113）:`crates/opentake-domain/src/caption_sync.rs` 把某 `text_style` 批量套到同一 `caption_group_id` 的所有字幕 clip（不可变 / 跨组隔离 / legacy 安全,12 单测）。接 UI/命令留后续。
- **关键帧编辑**（#95,PR #119）:后端 4 个细粒度 EditCommand（`StampKeyframe`/`RemoveKeyframe`/`MoveKeyframe`/`SetKeyframeInterpolation`,clip 相对帧 + 半开区间校验 + move 占用/越界守卫,12 单测）+ 前端 Inspector `KeyframesPanel`/`KeyframesLaneRow` 菱形可拖编辑（每属性一行、拖动/删除/盖章/改插值）。**确立关键帧『后端命令』路线**（优于纯前端 read-modify-write,更接近上游 1:1 + 走真命令/undo）。

### 审核处置（本轮）

| PR | 处置 | 说明 |
|---|---|---|
| #104 #106 #107 #110 #111 #112 #113 #115 #117 #119 | **已合并** | CI 双绿 + 审核通过（对抗验证 CONFIRM / 主控亲审）的纯新增项;**#115 收口 #37 全局库 epic**;#117 导出加音频混音;#119 关键帧编辑（确立后端命令路线）|
| #120 #121 #122 #123 | **请修改（@作者）** | 与 #119 同批的时间线/Inspector PR:#120 关键帧段与 #119 路线冲突需让位;#121 SwapMedia 违 1:1（缺类型校验/自创时长改动/缺链接级联）;#122 Inspector 可编辑字段误用 fade 采样值（写坏静态属性）+ 缺 live 预览;#123 duplicate 未重映射 link_group。合 #119 后均需 rebase |
| #76 | **已关闭** | bundle id 改名冗余（main 已是 `com.opentake.desktop`,#74 已合） |
| #77 #78 #79 #105 #108 | **请修改（@作者）** | 详见下「待审 PR」 |

---

## 历史开发记录 — 2026-06-23 第二轮（已由 Beta 1/2 状态替代）

合并自 PR #102（基于已合并的 #81）。多 Agent 协作：主控修 Bug + 编排 workflow 做功能。

### 修复（Fixed）

- **暂停无法停止**：`TimelinePlaybackLayer` 在 ref-detach 路径先 `pause()` 再删除——React 卸载时 ref detach 先于 effect cleanup（旧 cleanup 拿到空 Map），且从 DOM 移除的媒体元素不会自停。
- **音频波形不渲染**：波形解码改用项目统一的 ffmpeg `extract_pcm`（原 Symphonia 解不了 `.mov`/非 AAC 容器），移除 symphonia 依赖，前后端补失败日志，新增 mp4 视频容器波形集成测试。
- **丢失素材重选后全红不恢复**：新增 `relink_media` 命令（保持同一 media id，只改 source 路径），`MediaItemDto.missing` 按文件存在性实时派生，时间线红色 wash + 卡片离线覆盖层 + 「重新链接」。
- **拖剪辑不跟手**：画布在拖拽时不重绘的问题——被拖片段现以半透明 ghost 实时跟随光标（move/trim）。
- **顶栏不常驻 / 媒体库不能滚动**：媒体面板内层 flex 容器补 `minHeight:0`，标签栏/工具栏固定、仅网格滚动。
- **暂停态预览跑到角落/很小**：合成帧改稳健等比盒，始终居中铺满。
- **时间线空状态文案未翻译**：「Drop media here to start」改走 i18n。

### 新增（Added）

- **剪映式触控板 + 编辑键**：捏合 / Cmd / Ctrl + 滚轮 = 缩放（光标锚定），Option + 滚轮 = 横向滚动，裸双指 = 平移，⌘± 缩放，⇧Z 适配窗口；Q/W = 删除播放头左/右（修剪到播放头），⌘B/B = 分割/切割，A/V = 选择，Toolbar `[`/`]` 接线。原生 `{passive:false}` wheel 监听（修复捏合误缩放整页）。
- **剪映式顶部素材面板**：8 主标签（素材/音频可用，文本/贴纸/特效/转场/字幕/智能包裹置灰占位）+ 导入/我的二级 + 卡片星标收藏（localStorage）。
- **时间线导出 XMEML 4（Final Cut Pro 7 XML，`.xml`）**：`crates/opentake-project/src/fcpxml.rs`，1:1 端口上游 `XMLExporter.swift`，可被 Premiere / DaVinci / FCP 打开；传输位置/裁剪/速度(time remap)/音量·透明·变换·裁剪关键帧/淡变/链接 A-V 互链/源帧率 NTSC 标记。`Clip::keyframe_frames` 领域方法。
- 移除标题栏「切换 Agent 面板」按钮（面板仍经 View 菜单/快捷键）。

### 性能（Performance）

- **缓解整机卡死**：暂停态合成帧改为播放头停稳 ~140ms 后才取（`Preview.tsx useDebounced`），scrub 全程不再逐帧触发 ffmpeg/wgpu。**仅为缓解，需真机验证；彻底修见 #92 / #100。**

### 验证

web `tsc` 干净 + `vitest` 43；Rust `fmt`/`clippy` 干净 + `opentake-project` 30（含 13 个 fcpxml 测试）/ `opentake-tauri` 22 / `opentake-media` 测试全绿。CI（Rust + Web）双绿后合并。

---

## 当时未完成 / 已知问题（历史 Issue 快照）

每个 Issue 含「现状位置 + 如何完成 + 上游/剪映参照」。

| # | 优先级 | 模块 |
|---|---|---|
| [#91](https://github.com/appergb/OpenTake/issues/91) | 🔴 CRITICAL | **素材/媒体管理系统数据流错误**——需删除后端+管理逻辑按剪映完全重写（文件夹不成文件夹/素材重复显示/音频 tab 语义错/收藏混乱/无波形预览） |
| [#92](https://github.com/appergb/OpenTake/issues/92) | 🔴 CRITICAL | 拖动/暂停 scrub 逐帧合成致整机卡死（本轮仅防抖缓解，待真机验证 + 真实播放引擎彻底修） |
| [#93](https://github.com/appergb/OpenTake/issues/93) | 🟠 | 片段右键菜单缺失 |
| [#94](https://github.com/appergb/OpenTake/issues/94) | 🟠 | 复制/剪切/粘贴（⌘C/⌘X/⌘V）缺失 |
| [#95](https://github.com/appergb/OpenTake/issues/95) | 🟠 | 关键帧编辑入口缺失（只读不可编辑） |
| [#96](https://github.com/appergb/OpenTake/issues/96) | 🟠 | 文本工具 T 死按钮 + 文本编辑 UI 缺失 |
| [#97](https://github.com/appergb/OpenTake/issues/97) | 🟠 | Inspector 三段式 live + 缺字段（位置/裁剪/翻转/fade） |
| [#98](https://github.com/appergb/OpenTake/issues/98) | 🟡 | 拖放落点路由 / 拖到新轨 / Option 拖拽复制 |
| [#99](https://github.com/appergb/OpenTake/issues/99) | 🟡 | 吸附迟滞+多探针 / 链接 offset 角标 / 音量橡皮筋 |
| [#100](https://github.com/appergb/OpenTake/issues/100) | 🟡 | 真实播放/scrub 引擎（彻底修 #92） |
| [#101](https://github.com/appergb/OpenTake/issues/101) | 🟡 | Swap Media / Save as Media / Extract Audio 后端命令 |

## 后续计划（建议顺序）

1. **#91 素材系统重写**（最高优先，用户点名）：单一权威 manifest（folderId 层级）+ 文件夹钻取浏览 + 音频提取/收藏入后端 + 波形卡片；前端只读镜像不去重拼接。
2. **#92 / #100 卡死与播放**：先真机验证防抖是否够；不够则做真实播放/scrub 引擎（连续解码 + 音画同步 + 精确 seek + 预览降档）。
3. **基础剪辑手感**：#93 右键菜单 + #94 复制粘贴 → #96 文本 T → #95 关键帧 → #97 Inspector 三段式（后端 ClipProperties 需扩 crop/fade）。
4. **打磨**：#98 落点路由/Option 复制 · #99 吸附迟滞/offset 角标/音量线 · #101 后端命令。

## 待审 PR（经第三轮审核：请修改 / @作者）

以下 PR 经第三轮自动审核**未合并**,均已在 PR 上 @作者 给出具体阻塞项与修改要求（CHANGES_REQUESTED）:

- **#108** 片段右键菜单（#93）— **CONFLICTING**,需 rebase;菜单 `position:fixed` 缺 `top/left`（永远渲染在 0,0,功能不可用）;渲染期直接调 `onClose()` 违反 React 规则;缺 Swap/Save/Extract Audio 菜单项（即使 disabled）。
- **#105** 复制/剪切/粘贴 ⌘C/⌘X/⌘V（#94）— **CONFLICTING**,落后 main 39 commit,`editActions.ts` import 段冲突需 rebase;粘贴视频丢链接音轨（`addLinkedAudio:false`）、空剪贴板无提示待补。
- **#79** 提取音频到本地（#39）— **CONFLICTING**,基于旧版 patch 上下文不匹配（命令注册不上）;「提取音频」星标与 main 已有收藏星标在同坐标叠层;缺验收测试;out_path 无路径校验;触碰 #91 范围。
- **#78** 设置 7-pane + MCP + 主页（#40）— 验收仅约 50%（缺 Models/Privacy/Shortcuts/Account pane、主页 Sign in/File missing/SampleProjects/Update 徽标）;`SettingsView.tsx` 合并后超 800 行规约,需按上游每 pane 拆文件。
- **#77** 文件夹浏览前端（#58）— **CONFLICTING**,Rust diff 基于旧版 media.rs（字段重复）、`FolderTile` 未导入 `Folder` 图标（tsc 报错）、`createFolder` 签名与 main 重复;缺上游选中/重命名/右键/文件夹互拖;与 **#91 素材重写重叠,建议并入 #91**。
