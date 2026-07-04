# ipc-api — IPC 边界与数据契约（lib/）

> 上级：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 覆盖 `web/src/lib/` 中与 Rust/Tauri 对接的部分：`api.ts`（IPC + `isTauri` 判定）、`types.ts`（含 `EditRequest`）、`asset.ts`、`libraryApi.ts`、`dialog.ts`、`fallback.ts`。这是**前后端唯一的契约面**——多词字段的 camelCase 对齐是高频 bug 来源。

---

## 一句话职责

把 Tauri `invoke`/`listen` 封装成类型化的前端 API，把 Rust 领域模型镜像成 TS 类型，并在非 Tauri 环境（纯浏览器）整体降级为空操作 / 内存 demo。

---

## api.ts — Tauri 桥

- **`isTauri`**：检测 `window.__TAURI_INTERNALS__`。Tauri 注入则为 `true`；纯 `vite dev`/`vite preview` 为 `false`。
- **`ensureTauri()`**：懒加载 `@tauri-apps/api` 的 `invoke`/`listen`，浏览器下不引入。
- 命令封装（每个都「Tauri → `invoke`；否则 → fallback / 空值」）：
  - 编辑：`getTimeline` / `editApply(command)` / `editApplyMany`（串行，每条仍走单一 `EditCommand` 权威）/ `undo` / `redo` / `canUndo` / `canRedo`。
  - 工程：`projectNew` / `projectOpen` / `projectSave` / `getDefaultProjectDir` / `exportFcpxml`（实际产物是 XMEML，注释解释命名）。
  - 媒体：`importFolder` / `importMedia` / `getMedia` / `extractAudio` / `relinkMedia`（保留资产 id 就地恢复离线 clip）。
  - 预览/波形：`compositeFrame(frame, maxSize?)`（GPU 合成一帧 PNG dataURL；`Math.floor(frame)` 后再传，避免 Tauri 反序列化非整数不一致）/ `getWaveform`（**try/catch 包裹**，解码失败 `console.warn` 而非静默吞——这正是当年波形整类失效的根因）。
  - BYOK 密钥：`secretSave/secretLoad/secretDelete`（明文仅在保存时上行，返回掩码 `SecretStatus`，密钥不进 JS 内存/localStorage）。
  - 事件：`onTimelineChanged` / `onProjectOpened` / `onMediaChanged` / `onGoHome`，均返回 unlisten；非 Tauri 返回空 unlisten。

文件头注释点题：所有编辑走 `edit_apply`，镜像经 `get_timeline` + `timeline_changed` 刷新；非 Tauri 优雅降级，「真理始终在 Rust」。

## types.ts — Rust 领域模型的 TS 镜像 + `EditRequest`

字段名**逐字匹配 Rust serde 的 camelCase 输出**，也是 `project.json` 的 schema。

- 领域值类型：`Timeline` / `Track` / `Clip` / `Keyframe<V>` / `Transform` / `Crop` / `ColorGrade` / `ChromaKey` / `Mask` / `Effect`（注：`Clip.displayHeight` 是 UI-only，不入 JSON）。
- 上行参数 DTO：`ClipEntryReq` / `ClipMoveReq` / `TrimEditReq` / `ClipPropertiesReq` / `TextEntryReq` / `KeyframePayloadReq` / `RenameEntryReq` / `FrameRangeReq`。
- 返回：`EditResult`（含 `changed` + `affectedClipIds`）/ `TimelineSnapshot`（`timeline`+`version`）/ `MediaList` / `SecretStatus`。

### `EditRequest`（serde 判别联合体）—— camelCase 对齐铁律

`EditRequest` 是带 `"type"` 标签的联合体，**对端是 `src-tauri/src/commands.rs` 的 serde DTO `EditRequest`**，标了 `#[serde(tag = "type", rename_all = "camelCase")]`。注意：`opentake-ops::EditCommand` 本身是**无 serde 的纯枚举**，`edit_apply` 负责把 DTO 映射成它。因此**多词字段在前端线上必须是 camelCase**。

变体 `type` 值（不逐字段）：`addClips` / `insertClips` / `moveClips` / `duplicateClips` / `removeClips` / `splitClip` / `trimClips` / `setClipProperties` / `setColorGrade` / `setChromaKey` / `setMasks` / `setEffects` / `setKeyframes` / `stampKeyframe` / `removeKeyframe` / `moveKeyframe` / `setKeyframeInterpolation` / `rippleDeleteRanges` / `rippleDeleteClips` / `addTexts` / `link` / `unlink` / `insertTrack` / `setTrackProps` / `createFolder` / `moveToFolder` / `renameMedia` / `renameFolder` / `deleteMedia` / `deleteFolder` / `swapMedia`。

典型 camelCase 多词字段：`atFrame`、`trackIndex`、`toTrack`/`toFrame`、`offsetFrames`、`targetTrackIndexes`、`trimStartFrame`/`trimEndFrame`、`durationFrames`、`fadeInFrames`/`fadeInInterpolation`、`syncLocked`、`parentFolderId`、`textStyle`/`textContent`、`keyColor`。

> ⚠️ **改 IPC 字段时三边同步**：Rust DTO（`commands.rs`）↔ 本文件 `EditRequest` ↔ 调用处（`editActions.ts`）。历史上「删除/分割/Inspector 全静默失效」就是 DTO 的 camelCase 没对齐导致反序列化失败。IPC 若静默吞错，先加 try/catch 把错误暴露出来（见 `api.ts` 的 `getWaveform` 范式）。

## asset.ts — 本地文件 → 可加载 URL

`assetUrl(path)`：用 Tauri 的 `convertFileSrc` 把本地绝对路径转成 asset 协议 URL，让 `<img>/<video>/<audio>` 直接由 WebKit/WebView2 解码（规避一条独立的缩略图管线）。`convertFileSrc` 只是字符串拼接，浏览器下静态导入安全；实际仍 gate `isTauri`（asset scheme 只在 Tauri WebView 内可解析，且 fallback 媒体 `path` 为 null）。

## libraryApi.ts — 全局素材库通道（独立于项目媒体）

跨项目永久收藏库（project #55）的 invoke 包装：`libraryList(category?)` / `libraryFavorite(source, kind, category?, thumb?)`（内容 hash 去重）/ `libraryUnfavorite` / `libraryCategorize` / `libraryRename` / `libraryDelete` / `libraryImportToProject`（拷进当前项目，调用方随后 `refreshMedia`）。invoke 参数 snake_case，返回 camelCase（serde 处理）；非 Tauri 全部空操作。

## dialog.ts — 原生对话框懒加载

`openDialog()` / `saveDialog()`：仅在 Tauri 下动态导入 `tauri-plugin-dialog` 的 `open`/`save`，code-split 以免对话框依赖落进浏览器包；非 Tauri 返回 null 供调用方降级。

## fallback.ts — 浏览器内存 demo

`createFallbackStore()` 返回 `getTimeline` / `reset` / `noop(name)` / `editApply(cmd)`。**刻意保持小型化，不是第二个编辑引擎**——只模拟一个 `EditRequest` 子集（`insertTrack`/`addClips`/`removeClips`/`moveClips`/`duplicateClips`/`splitClip`/`setClipProperties`/`setColorGrade`/`setChromaKey`/`setMasks`/`setEffects`，含链接音频自动建/删），其余返回 `changed: false`。关键帧与库命令为 placeholder。注释：「the authoritative truth is always the Rust core under Tauri.」

---

## 完成状态

- **已实现**：编辑/工程/媒体/预览/波形/密钥/事件全套 Tauri 封装；`EditRequest` 全变体类型；asset 协议、库通道、对话框懒加载、浏览器内存 demo（核心 clip 编辑子集）。
- **计划中/占位**：fallback 的关键帧、库与文本命令未模拟；`exportFcpxml` 产物为 XMEML（命名沿用上游 F4 契约）。

## 相关文档

- 调用方（手势如何打包成 `EditRequest`）→ [state-stores.md](state-stores.md)
- `compositeFrame`/`getWaveform` 的消费端 → [preview-ui.md](preview-ui.md)、[timeline-ui.md](timeline-ui.md)
- 规格中的对接点章节 → [SPEC.md](SPEC.md)（§11 Tauri command/event、§12 数据模型镜像）
- IPC 对端实现 → [../src-tauri/INDEX.md](../src-tauri/INDEX.md)

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
