# web — 总览

> 上级：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)

## 一句话定位

**React/TypeScript + Vite + Zustand 前端**（`web/`，包管理器 pnpm、测试 vitest）：Palmier Pro 桌面编辑器的全部可视层与交互层。在架构中处于最上层，且是**纯消费者**——**只持后端 `Timeline` 只读镜像 + 版本号，不做撤销、不持任何领域逻辑**。真理状态在 Rust，前端读镜像、发命令、等事件回流刷新。

### 在依赖分层中的位置（前端在最上，只向下依赖）

```
opentake-domain                                值语义叶子层
   ▲
opentake-ops                                   纯引擎 + EditCommand + 撤销栈（撤销在这里，不在前端）
   ▲
opentake-project / render / media / motion / agent / gen   能力层
   ▲
opentake-core                                  会话 / DI / 事件总线
   ▲
src-tauri                                       Tauri 壳 + 命令注册 + 事件桥（IPC 对端）
   ▲
web  ★ 本模块                                   React/TS 前端（只读镜像 + 版本号）
```

## 职责边界

**做：**
- 把后端 `timeline` 镜像渲染成时间线（Canvas）、预览、检查器、媒体面板、工具栏等界面。
- 接收用户手势/快捷键/拖放，归一成 `EditRequest` 经 `edit_apply` 发给 Rust。
- 持有纯 UI 态（选择、缩放、滚动、播放头、面板布局、标签、剪贴板、Toast、设置、最近项目）。
- 像素↔帧换算、吸附、命中测试等「投影几何」。
- 非 Tauri（纯浏览器）下降级为内存 demo，使 UI 壳可独立浏览。

**不做：**
- 不持权威 `Timeline`、不实现撤销/重做栈（都在 `opentake-ops`/Rust）。
- 不做帧↔秒换算（在 Rust）、不解码/编码媒体（在 `opentake-media`）、不做 GPU 合成（在 `opentake-render`）。
- 组件不持领域逻辑——只渲染快照 + 派发动作。

## 关键概念与数据流

**单一真理 + 命令事务**：Rust 持权威 `Timeline`；前端 Zustand 只持只读镜像 + `timelineVersion`。

```
UI 手势 / 快捷键 / 拖放
  → editActions.*（把手势映射成 EditRequest）
  → lib/api.ts editApply()
  → Tauri 命令 edit_apply（Rust 走 withTimelineSwap 事务，有变更才 version++）
  → 监听 timeline_changed{version}
  → 版本前进则 get_timeline() 刷新镜像（store/sync.ts）
```

- **换算分工铁律**：**像素↔帧换算放前端**（`lib/geometry.ts`，截断取整、对齐上游 `TimelineGeometry`）；**帧↔秒换算放 Rust**。
- **IPC camelCase 契约**：`EditRequest` 是带 `"type"` 标签的 serde DTO（`#[serde(tag="type", rename_all="camelCase")]`），对端是 `src-tauri/src/commands.rs`。**多词字段线上必须 camelCase**（`atFrame`/`trackIndex`/`offsetFrames`…）；三边（Rust DTO ↔ `lib/types.ts` ↔ 调用处）须同步，否则反序列化静默失败（历史「删除/分割/Inspector 全挂」根因）。IPC 静默吞错先加 try/catch 暴露。
- **非 Tauri 降级**：`lib/api.ts` 用 `isTauri` 判定（`window.__TAURI_INTERNALS__`）；纯 `vite dev`/`vite preview` 下 `isTauri=false`，命令落到 `lib/fallback.ts` 内存 demo，UI 壳可浏览（但真实编辑真理只在 Rust 下成立）。fallback 无事件，编辑后由 actions 显式 `forceRefresh()`。
- **单表面 + 单时钟预览**：播放/暂停/拖拽共用同一组 `<video>/<audio>`，由唯一一个 `requestAnimationFrame` 循环驱动播放头（`components/preview/previewEngine.ts`）。
- **顶层装配**：`main.tsx` → `App.tsx`，挂载 `startSync()`/`startMediaSync()`/`initI18n`/`initTheme`，并常驻 `useKeyboardShortcuts`/`useTimelinePlaybackEngine`/`useAutosave`。

## 完成状态

**已实现：**
- 镜像同步与版本去重；编辑命令全量映射（clip/效果/链接/轨道/关键帧/波纹/库/交换）。
- 时间线 Canvas 绘制 + 手势（刮擦/缩放/平移/移动/修剪/切割/拖放落轨）+ 吸附/命中。
- 单时钟预览引擎（播放/暂停/拖拽）+ 单素材与时间线合成两种模式。
- 检查器（Video/Audio/Text、现场采样、关键帧面板、可拖拽数值）。
- 媒体面板（导入/拖放/双击/星标/萃取音频/Relink）、全局库页、工具栏、主页启动器、设置（含 BYOK keychain）。
- 复制/剪切/粘贴、删除健壮化、自动保存、双语（zh-CN/en）、AppTheme 全令牌。

**计划中 / 占位：**
- Agent 面板真实对话；媒体面板部分标签（Text/Sticker/Effect/Transition/Captions/Smart Wrap）为置灰占位。
- 检查器文本样式（字号/颜色/对齐）、媒体资产检查；fallback 的关键帧与库命令未模拟。
- 与上游逐项 1:1 截图/行为/几何对拍验收（见 [SPEC.md](SPEC.md)）；高负载流式预览引擎。

## 代码风格

- **数值/颜色常量走 `lib/theme.ts`（`AppTheme`）或对应 CSS 变量，不硬编码**（逐字镜像上游 `AppTheme.swift`）。
- **图标用 `lucide-react`**（统一经 `components/ui/Icon.tsx` 包装，继承 `currentColor`）。
- **组件不持领域逻辑**——只渲染快照、派发动作；像素↔帧换算放前端，帧↔秒换算放 Rust。
- 文件组织按特性/面板分目录；TS 严格类型，`EditRequest` 等公共契约显式建模。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
