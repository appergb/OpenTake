# web — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> `web/` = OpenTake 的 **React/TypeScript + Vite + Zustand** 前端（包管理器 pnpm，测试 vitest）。它是架构最上层的**纯消费者**：只持后端 `Timeline` 只读镜像 + 版本号，**不做撤销、不持领域逻辑**；编辑经 `edit_apply` 发往 Rust，由 `timeline_changed` 事件回流刷新。像素↔帧换算放前端，帧↔秒换算放 Rust。非 Tauri 下 `isTauri=false`，命令落内存 fallback。

---

## 总览

- **[OVERVIEW.md](OVERVIEW.md)** — 一句话定位与架构位置、职责边界（做/不做）、关键概念与数据流（手势→editActions→`editApply`→`edit_apply`→`timeline_changed`→`get_timeline`；camelCase 契约；单表面单时钟预览；非 Tauri 降级）、完成状态、代码风格。

## 子系统文档

- **[state-stores.md](state-stores.md)** — `store/`：Zustand 各 store + actions + 镜像同步。`projectStore`（只读镜像 + 版本 + canUndo/canRedo，**无撤销栈**）、`uiStore`/`settingsStore`/`clipboardStore`/`recentStore`（纯 UI 态）、`mediaStore`/`libraryStore`（后端镜像）、`editActions`（手势→`EditRequest`，删除健壮化、媒体落轨串行化、复制/剪切/粘贴）、`mediaActions`/`projectActions`（对话框驱动）、`sync.ts`（镜像更新唯一入口 + `forceRefresh`）。
- **[ipc-api.md](ipc-api.md)** — `lib/` 对接面：`api.ts`（IPC + `isTauri`，`editApply`/`getTimeline`/`getWaveform` try/catch/`compositeFrame`/`secret_*` 事件）、`types.ts`（领域镜像 + `EditRequest` 全变体 + **camelCase 对齐铁律**）、`asset.ts`（`convertFileSrc` 资产协议）、`libraryApi.ts`（全局库通道）、`dialog.ts`（对话框懒加载）、`fallback.ts`（浏览器内存 demo 子集）。
- **[timeline-ui.md](timeline-ui.md)** — `components/timeline/` + `lib/geometry.ts`/`snap.ts`/`ruler.ts`/`zones.ts`/`clip.ts`：像素↔帧（前端、截断）、Canvas 绘制（`timelineCanvas`/`clipRenderer`/`rulerCanvas`）、吸附/多探针、命中测试、刮擦/缩放/平移/移动/修剪/切割与触控板手势、轨道头、右键菜单、媒体交换。
- **[preview-ui.md](preview-ui.md)** — `components/preview/`：单表面 + 单时钟模型，`previewEngine`（rAF 三态 PLAY/SCRUB/PAUSE）、`timelinePlayback`（纯逻辑）、`TimelinePlaybackLayer`（被动 DOM 注册）、`previewLayerStyles`（样式采样）、`Preview`（单素材/合成两模式 + 运输控制）。
- **[panels-ui.md](panels-ui.md)** — `components/` 其余：inspector（检查器 + 关键帧面板 + 可拖拽数值 + 文本）、media（媒体面板 + 全局库页 + 星标）、toolbar、home（启动器）、settings（含 BYOK keychain）、agent（占位）、shell（五面板布局 + 分割条 + 标题栏）、ui（lucide `Icon` / `HoverButton` / `Dropdown` / `PanelShell`）。
- **[hooks-i18n-theme.md](hooks-i18n-theme.md)** — `hooks/`（`useAutosave` 防抖保存、`useKeyboardShortcuts` 快捷键）+ `i18n/`（zh-CN 默认 / en）+ `lib/theme.ts`（`AppTheme` 数值常量单一源）+ `styles/`（`tokens.css` CSS 变量、`global.css` 全局基础）。

## 规格

- **[SPEC.md](SPEC.md)** — 前端 1:1 复刻上游实现就绪规格（Issue #12）。只读、只链接、不改：设计令牌表、五面板布局、组件地图、Toolbar/Timeline/Inspector/MediaPanel/Preview 逐项常量与交互、Zustand 状态拆分、Tauri command/event 对接点、数据模型镜像、1:1 验收方式（截图/行为/几何对拍）。

## 相关跨切面（架构）

- [ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) — 总体架构：单一真理状态 + 命令事务、IPC 边界、前端只读镜像在数据流中的位置。
- [ROADMAP.md](../../architecture/ROADMAP.md) — 各 Phase 的前端工作（编辑器外壳、时间线、预览、检查器、媒体面板）。
- [PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) — 1:1 复刻差距（预览取源帧、缩略图接线等，含前端项）。⚠️ 历史参考。
- [CAPCUT-GAP.md](../../architecture/CAPCUT-GAP.md) — 与剪映的功能差距（媒体面板占位标签对应项）。
- [BUGS.md](../../architecture/BUGS.md) — 已知 Bug（含 IPC camelCase、删除/分割静默失效的历史根因）。
- [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md) — 上游 Swift → 前端移植图（几何算式真理来源）。

## 上游拆解参考

- [上游拆解 · 架构与数据流](../../upstream-analysis/01-架构与数据流.md) — 上游 `EditorViewModel` 单一可观测容器 → 前端 store 映射的来源。
- [上游拆解 · 苹果框架可移植性](../../upstream-analysis/02-苹果框架可移植性.md) — AppKit/AVFoundation 投影 → Web 等价（Canvas/HTML5 媒体）。

## 相关模块

- [src-tauri](../src-tauri/INDEX.md) — **IPC 对端**：`edit_apply`/`get_timeline`/导出/库/媒体/密钥/MCP 命令与 `timeline_changed`/`media_changed`/`project_opened`/`go_home` 事件桥。
- [opentake-ops](../opentake-ops/INDEX.md) — `EditRequest` 最终映射到的 `EditCommand` + 撤销栈（撤销在此，不在前端）。
- [opentake-render](../opentake-render/INDEX.md) — 预览/导出的 GPU 合成对端（`composite_frame` 来源）。
- [opentake-media](../opentake-media/INDEX.md) — 波形/媒体探测/萃取音频/全局库的后端能力（`get_waveform`/`extract_audio`/库命令）。
- [opentake-domain](../opentake-domain/INDEX.md) — `types.ts` 所镜像的 `Timeline`/`Track`/`Clip`/`Keyframe` 等值类型源。

## 源码

```
web/src/
├── main.tsx                入口：createRoot 渲染 App + 导入 global.css
├── App.tsx                 顶层装配：startSync/startMediaSync/initI18n/initTheme + 常驻钩子 + 视图路由 + Toast
├── store/                  Zustand 状态层（镜像 + UI 态 + actions）
│   ├── projectStore.ts     只读镜像 + timelineVersion + canUndo/canRedo（无撤销栈）
│   ├── sync.ts             镜像同步唯一入口（startSync/refreshMirror/forceRefresh）
│   ├── editActions.ts      手势 → EditRequest 映射（含删除健壮化/落轨串行化/剪贴板）
│   ├── mediaStore.ts       项目媒体镜像 + media_changed 订阅
│   ├── mediaActions.ts     导入/Relink（对话框驱动）
│   ├── libraryStore.ts     全局库镜像 + 视图态派生
│   ├── projectActions.ts   新建/打开/保存（对话框驱动）
│   ├── clipboardStore.ts   复制缓冲（UI-only）
│   ├── recentStore.ts      最近项目（localStorage）
│   ├── settingsStore.ts    主题/导入目录/BYOK provider/窗口尺寸
│   └── uiStore.ts          UI-only 态合集（选择/缩放/播放/布局/标签/Toast）
├── lib/                    IPC 边界 + 数据契约 + 几何/工具
│   ├── api.ts              Tauri 桥 + isTauri 判定 + 命令/事件封装
│   ├── types.ts            领域镜像类型 + EditRequest（camelCase 契约）
│   ├── fallback.ts         浏览器内存 demo（命令子集）
│   ├── asset.ts            assetUrl：本地路径 → Tauri asset 协议 URL
│   ├── libraryApi.ts       全局库 invoke 包装
│   ├── dialog.ts           原生对话框懒加载（open/save）
│   ├── geometry.ts         像素↔帧 + clipRect/trackY（前端换算）
│   ├── snap.ts             吸附点收集 + 多探针查找
│   ├── ruler.ts            刻度选择（主/次间隔）
│   ├── zones.ts            视觉/音频区划分 + 轨道标签
│   ├── clip.ts             clip 辅助（变换适配/修剪到播放头）
│   └── theme.ts            AppTheme：数值/颜色常量单一源
├── components/
│   ├── timeline/           Canvas 时间线 + 手势 + 叠加层（容器/区域/轨道头/播放头/吸附线/右键菜单/交换选择器/绘制模块/命中）
│   ├── preview/            单时钟预览引擎 + 播放层 + 样式采样 + 面板
│   ├── inspector/          检查器 + 关键帧面板/行 + 可拖拽数值 + 文本 + 交换素材
│   ├── media/              媒体面板 + 全局库页 + 标签栏 + 星标
│   ├── toolbar/            顶部工具栏
│   ├── home/               启动器
│   ├── settings/           设置（含 BYOK）
│   ├── agent/              Agent 面板（占位）
│   ├── shell/              五面板布局 + 分割条 + 标题栏 + 视图菜单
│   └── ui/                 通用原始件（lucide Icon / HoverButton / Dropdown / PanelShell）
├── hooks/                  useAutosave / useKeyboardShortcuts
├── i18n/                   dict（zh-CN 默认 / en）+ index（运行时 + useT/t）
└── styles/                 tokens.css（CSS 变量）+ global.css（@import tokens + 全局基础）
```

源文件树根：`../../../web/src/`

---

## 页脚

- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
