# Changelog

本文件记录 OpenTake 的重要改动。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/)。

## [未发布] — 2026-06-23 第三轮（自动 PR 审核：全局素材库 + 文本工具 + 字幕/视频导出 + list_models）

本轮为**自动 PR 审核流程**:逐 PR 专家审核 + 对抗验证 + 对照开发文档,审核通过且 CI 双绿的纯新增项合并,其余 @作者 rebase/修改。

### 新增（Added）

- **全局可复用素材库后端**（#37）:
  - **#37-A / #54**（PR #104）后端存储层 `crates/opentake-media/src/library.rs`:copy-on-favorite + SHA-256 内容寻址去重 + JSON manifest 原子写（`.tmp` rename）+ Mutex 并发安全。9 个单测。
  - **#37-B / #55**（PR #106）Tauri 命令层 `src-tauri/src/library.rs`:7 个命令 `library_list` / `favorite` / `unfavorite` / `categorize` / `rename` / `delete` / `import_to_project`。
  - 上游 palmier-pro 无此模块,#37 为 OpenTake 新增子系统,不要求 1:1。前端（#37-C / #56）尚未实现,收藏暂仍走前端 localStorage。
- **文本工具 MVP**（#96,PR #107）:Toolbar `T` 按钮接线 `addTextClip()`、新增 `TextTab.tsx` 文字内容编辑、Inspector 路由 text tab。字体/字号/颜色等 `textStyle` 留后续（依赖后端 ClipProperties 扩展）。
- **SRT/VTT 字幕导出纯逻辑**（#29 D 层切片,PR #110）:`crates/opentake-domain/src/subtitle_export.rs` 把按 `caption_group_id` 分组的字幕 clip 序列化为 SubRip/WebVTT 字符串（零 IO、零新依赖、16 单测）。导出层/agent 工具/前端对话框留后续切片。
- **整条时间线视频导出编排**（Phase 5 spine,PR #112）:`src-tauri/src/export.rs` + `export_video` 命令,逐帧 `Compositor::render_to_rgba` → `VideoEncoder::push_frame` → `finish`,全分辨率 H.264/.mp4（H.265/ProRes/音频/进度取消留后续）。含 ffmpeg/GPU 门控集成测试。自包含复制 preview 路径,未碰 `composite_frame`。
- **`list_models` 工具接线**（#9/#10 切片,PR #111）:`opentake-agent` 从存根接 `opentake-gen` 内置静态 catalog,`?type=` 过滤 + `{ models, loaded }` JSON,纯本地无网络/BYOK。

### 审核处置（本轮）

| PR | 处置 | 说明 |
|---|---|---|
| #104 #106 #107 #110 #111 #112 | **已合并** | CI 双绿 + 审核通过 + 对抗验证 CONFIRM 的纯新增项 |
| #76 | **已关闭** | bundle id 改名冗余（main 已是 `com.opentake.desktop`,#74 已合） |
| #77 #78 #79 #105 #108 | **请修改（@作者）** | 详见下「待审 PR」 |

---

## [未发布] — 2026-06-23 第二轮（剪映式 UI + 时间线剪辑修复 + 导出）

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

## 未完成 / 已知问题（已建 Issue 跟踪）

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
