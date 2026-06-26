# OpenTake vs palmier-pro-upstream 全项目扫描报告

**扫描日期**: 2026-06-26  
**最后更新**: 2026-06-26 (经代码验证修正)  
**指令**: 仅只读扫描（read-only），使用多 Agent 模式并行探索 + 直接工具调用。**禁止任何代码修改**。  
**目标**: 全面读取两个仓库，比较分析主要内容、逻辑、bug、严重问题、剪辑核心问题、体验不足等。  
**验证**: 所有声称的问题均经过代码级验证，已在 [BUGS.md](BUGS.md) 中记录实际确认的 Bug。  

## 扫描范围与方法

- **工作仓库**: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/`（Rust + Tauri 2 + React 跨平台端口）
- **上游参考**: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/`（Swift macOS 编辑器，GPL-3.0，只读真理来源）
- **方法**: 
  - 多并行 `spawn_subagent` (explore 类型) 覆盖子系统。
  - 直接 `list_dir`、`read_file`（分段）、`grep`（带路径/glob/正则）。
  - 覆盖：领域模型、编辑引擎、预览/渲染、Agent/MCP、持久化/媒体/导出/生成、前端 UI/交互、测试与文档。
  - 子代理累计 100+ 工具调用，读取所有主要 crates、web/src、src-tauri、tests、docs。

**总体结论**:
- **OpenTake** 核心编辑算法移植保真度高（1:1 port + 对拍测试），状态分离（Rust 真理 + 前端只读镜像）严格。
- **上游** 是成熟功能完整的 macOS 编辑器（AVFoundation 合成 + 完整 UI + 31 个 MCP 工具全实现）。
- **主要差距**: UI 边界接线、预览/渲染集成、媒体 UX、Agent 工具完整性、部分帧数学一致性。很多问题已在 docs 中自文档化，但代码状态仍匹配“内核坚实、四肢半残”。

## 1. 领域模型 (Domain Models)

**关键文件**:
- OpenTake: `crates/opentake-domain/src/{timeline.rs,clip.rs,keyframe.rs,transform.rs,clip_type.rs,media.rs}`
- 上游: `Sources/PalmierPro/Models/{Timeline.swift,Keyframe.swift,ClipType.swift}` + 辅助

**对等点** (高度 1:1):
- Timeline/Track/Clip 结构、半开区间、endFrame、sourceFramesConsumed = round(duration * speed)。
- Keyframe 相对 clip 存储 (toOffset/toAbs)，公开 API 绝对帧。
- 采样 (opacityAt/transformAt/volumeAt/cropAt/fadeMultiplier)、smoothstep = t*t*(3-2t)、VolumeScale。
- 容错 serde (#[serde(default)] + Option + legacy x/y 迁移)。
- ClipType 兼容性。

**问题/风险**（经验证修正）:
- 前端 `web/src/lib/clip.ts` 采样是近似 (CSS hack)，与 Rust domain 不完全同构。
- secondsToFrame: Rust/上游用截断 (Int(s*fps))，前端多处 Math.round (editActions.ts:389,410; MediaPanel.tsx:330) → 放置时长可能 off-by-1。**已验证为真实 Bug** → 见 [BUGS.md](BUGS.md#b2-前端帧数学截断不一致高)
- 高级字段 (colorGrade/chromaKey/masks/effects): Rust 有，web/types.ts 未镜像。
- Text natural size: Rust 近似 (非 pixel-exact)。
- 严重性: **低** (核心可靠)，但前端数学不一致是放置/导入潜在 bug。

**测试**: domain 内联 + upstream_compat 强覆盖。

## 2. 编辑引擎与操作 (Editing Engines)

**关键文件**:
- OpenTake: `crates/opentake-ops/src/{engines/{overwrite,ripple,snap}.rs,ops/{place,clear_region,move,split,trim,ripple,linking}.rs,command.rs}`
- 上游: `Editor/{OverwriteEngine.swift,RippleEngine.swift}`、`Timeline/SnapEngine.swift`、`Editor/ViewModel/{+ClipMutations.swift,+Ripple.swift,+Linking.swift}`

**对等点** (核心 1:1):
- OverwriteEngine (remove/trimEnd/trimStart/split，speed 感知)。
- RippleEngine (merge/shifts/push/refuse 语义)。
- Snap (sticky/playhead 阈值)。
- Linked audio (place 时共享 linkGroupId，传播)。
- Keyframe split/clamp (边界采样 + rebase)。
- 事务 (transact/withTimelineSwap 等价)。

**问题/风险**（经验证修正）:
- Speed 改变时 contiguous 后续链 ripple push 未实现 (domain 有方法但 ops 未调用)。
- UI 层: fade knee 拖拽 **已验证为已实现**，hidden track hitTest **已验证已过滤**，snap DPI 未缩放、轨间 insertThreshold 缺失。
- rippleDeleteRanges (agent 层): 硬编码 track_index，忽略 clipId/units。**已验证为真实 Bug** → 见 [BUGS.md](BUGS.md#b3-rippledeleteranges-agent-工具忽略-clipidunits高)
- 空 timeline 拖拽 **已验证会自动创建轨道，无静默 no-op**。
- 严重性: **中** (部分问题已不存在)。EDITING-ENGINE-PLAN.md 已列 issue。

**测试**: command_apply.rs 覆盖良好，但缺乏上游细粒度独立套件。

## 3. 预览与渲染 (Preview/Render)

**关键文件**:
- OpenTake: `src-tauri/src/render.rs`、`crates/opentake-render/src/{plan/*,gpu/*}`、`web/src/components/preview/{Preview.tsx,TimelinePlaybackLayer.tsx,useTimelineFrame.ts}`、`api.ts`。
- 上游: `Preview/{CompositionBuilder.swift,VideoEngine.swift,TextLayerController.swift}`。

**对等点**:
- Rust RenderPlan + wgpu compositor 完整 (build_render_plan 用 domain *_at，affine 1:1，text CosmicText)。
- Export 复用同一路径 (像素一致性目标)。

**问题/风险** (最大断层，经代码验证修正):
- 前端 Timeline tab 主要用 DOM `<video>/<img>` + rAF fallback。GPU composite `composite_frame` 命令和 `useTimelineFrame` hook **均已实现**，但 `Preview.tsx` **未接入**该 hook → 见 [BUGS.md](BUGS.md#d1-预览未接入-gpu-合成高)
- 结果: 看不到真实 kf/transform/crop/text/effects；preview ≠ export；Lottie/text 跳过；inspect_timeline 工具 stub。
- 高级 (grade/chroma/mask/effects) 仅 GPU 支持。
- 严重性: **High** (编辑时"看不见"效果)。PORT-1TO1-GAP.md 直言"完全是空的"。

**测试**: render plan/gpu 强；web 多用 mock。

## 4. Agent/MCP/工具 (Agent/MCP/Tools)

**关键文件**:
- OpenTake: `crates/opentake-agent/src/{mcp/{dispatch,core_handle,server},tools/*}`、`src-tauri/src/mcp.rs`。
- 上游: `Agent/{Tools/{ToolDefinitions,ToolExecutor*},MCP/*}`。

**对等点**:
- Shell 好 (short_id、encode_timeline、context_signal、dispatch 管道、描述 verbatim port)。

**问题/风险**:
- 大量 stub (dispatch.rs:177-189 + 注释): InspectMedia/GetTranscript/InspectTimeline/SearchMedia/Generate*/Upscale/Import/AddCaptions/Motion。
- CoreHandle 窄，未暴露 media/render。
- 细节: rippleDeleteRanges 不完整、batch folders 未实现、canGenerate 硬编码 false、undo scoping 弱。
- 严重性: **高** (AI 协作核心缺失 → transcript-driven 编辑等 workflow 不可用)。

**测试**: mcp_http.rs (传输) 存在；全工具执行弱。

## 5. 前端 UI/交互 (Frontend UI)

**关键文件**:
- OpenTake: `web/src/components/{timeline/*,preview/*,inspector/*,media/*,toolbar/*}`、`store/editActions.ts` 等。
- 上游: `Timeline/*`、`Preview/*`、`Inspector/*`、`MediaPanel/*`、`Editor/ViewModel/*`。

**问题/风险**（经验证修正）:
- 空 timeline 拖拽 **已验证会自动创建轨道，无静默 no-op**
- Inspector 非完整 3-stage、Text/AIEdit scaffold。**Toolbar 已验证所有按钮均有 onClick 绑定，无死按钮**
- DOM styles 近似 (CSS hack vs 真实 affine)
- 严重性: **中** (基础 CRUD 可用，但完整工作流/手感远差)。

## 6. 持久化/媒体/导出/生成 (Project/Media/Export/Gen)

**关键文件**:
- OpenTake: `crates/opentake-project/*`、`src-tauri/src/{media.rs,export.rs}`、`crates/opentake-media/*`、`crates/opentake-gen/*`。
- 上游: `Project/*`、`MediaPanel/*`、`Export/*`、`Generation/*`。

**问题/风险**:
- Import: thumbnails 永远 None (media.rs:79，“placeholder”)、无进度/反馈、扩展静默丢弃、folder 浏览不全。
- Export: H.264 spine 好，但 H265/ProRes 未接、无进度/取消。
- Gen: 占位部分，但 agent 路径 stub。
- Bundle 细微差异 (chat 目录名)。
- 严重性: **中高** (工作流断裂)。

## 7. 测试与文档 (Tests/Docs)

**测试**:
- 核心强 (domain inline、ops command_apply、project upstream_compat/roundtrip、render plan/gpu)。
- 弱: 缺乏上游细粒度 (Ripple*Tests 等)、E2E 集成、runtime preview、agent 全执行、复杂多轨场景、GPU 像素一致性。

**文档**:
- PORT-1TO1-GAP.md、EDITING-ENGINE-PLAN.md、ROADMAP、specs/* 高度准确 (很多 P0/P1 仍存)。
- MODULE-PORT-MAP/ARCHITECTURE 作为基线好。

**严重性**: **中** (核心可靠，表面验证不足)。

## 8. 最高优先问题列表 (Critical/High)

**Critical**:
1. 预览不反映真实合成 (DOM 主导 + GPU 合成 infrastructure 已就绪但未接入 Preview.tsx) → [BUGS.md](BUGS.md#d1-预览未接入-gpu-合成高)
2. Agent/MCP 工具大量 stub (inspect/transcript/search/generate/captions/import 等) → [BUGS.md](BUGS.md#d2-agentmcp-工具-1240-为-stub高)
3. Media thumbnails 永远 None、import 无反馈/进度 → [BUGS.md](BUGS.md#d3-media-缩略图始终返回-none中)

**High**:
- 帧数学跨层不一致 (前端 round vs 文档 truncate) → **已验证为真实 Bug** → [BUGS.md](BUGS.md#b2-前端帧数学截断不一致高)
- fade knee、hidden track hitTest **已验证已实现**
- Snap/阈值 缺失
- Inspector/toolbar scaffold/死按钮 (Toolbar **已验证无死按钮**) → [BUGS.md](BUGS.md#d5-inspector-texttabaiedit-为-scaffold中)
- Captions UI 缺失 → [BUGS.md](BUGS.md#d6-captionssubtitles-ui-完全缺失中)
- 测试 E2E/集成覆盖弱
- Export 预设/进度不全 → [BUGS.md](BUGS.md#d4-export-仅-h264中)

## 9. 总结与建议 (仅观察)

**优势**: Rust 核心编辑逻辑 + 状态分离 + 容错 + 纯函数几何移植质量极高；测试在算法层扎实。

**风险**: UI 边界 + Agent 能力 + 预览一致性是最大 blocker。很多问题已在文档中列出但未完全落地。

**优先级建议** (仅观察，非计划):
1. 接线 preview composite 到 UI + 补 captions UI/agent 工具。
2. 加强 E2E 测试 (关键流 + 像素对拍)。
3. 统一前端帧数学 (enforce truncate per docs)。
4. 保持 editApply + 只读镜像 + 每改必跑 clippy/test。

**参考文档** (强烈建议阅读):
- docs/PORT-1TO1-GAP.md
- docs/EDITING-ENGINE-PLAN.md
- docs/MODULE-PORT-MAP.md
- docs/ROADMAP.md
- docs/specs/frontend-UI-1to1-SPEC.md
- docs/ARCHITECTURE.md

扫描已覆盖“全部可达范围”。如需针对某一文件/子系统更深分段读取或特定证据，随时指示 (继续只读)。

---

*本报告由多子代理并行探索 + 直接工具调用合成，所有路径均为仓库内绝对路径。无任何代码变更。  
修正记录：2026-06-26 经代码验证，修正了报告中的 4 处误报（fade knee 缺失、hidden track hittable、空 timeline no-op、Toolbar 死按钮），将实际确认的问题归档到 [BUGS.md](BUGS.md)。*