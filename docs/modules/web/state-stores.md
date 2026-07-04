# state-stores — 状态层（Zustand store + actions + 镜像同步）

> 上级：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 覆盖 `web/src/store/`。前端状态分两类：**①后端只读镜像**（`projectStore`，由 Rust 真理同步而来）+ **②纯 UI 态**（`uiStore`/`settingsStore`/…，前端自有、Rust 不可见）。**前端不持撤销栈、不持领域逻辑**——所有编辑都打包成 `EditRequest` 发给 Rust，再由事件驱动重取镜像。

---

## 一句话职责

把「单一真理在 Rust」这条铁律落到前端：store 只缓存快照与 UI 瞬态，`*Actions` 把手势翻译成命令并触发刷新，`sync.ts` 是镜像更新的唯一入口。

---

## 镜像 vs UI 态（核心区分）

| store | 类别 | 持有什么 | 谁写 |
|---|---|---|---|
| `projectStore` | **后端镜像** | `timeline`（只读快照）+ `timelineVersion` + `canUndo/canRedo` + `projectPath` + `lastSavedVersion` | 仅 `sync.ts` |
| `mediaStore` | 后端镜像 | 项目媒体清单 `items` + `importing`/`error` | `refreshMedia()`（事件驱动） |
| `libraryStore` | 后端镜像 + 视图态 | 全局库 `entries` + 本页 `selectedCategory`/`search`/`sort` | 各操作后主动 `refresh()` |
| `uiStore` | **纯 UI 态** | 播放头/选择/缩放/滚动/工具模式/面板布局/标签/Toast 等 | 前端自由改，从不来自 Rust |
| `settingsStore` | 纯 UI 态 | 主题 / 默认导入目录 / BYOK provider / 窗口尺寸 | localStorage |
| `clipboardStore` | 纯 UI 态 | 复制的 clip 深快照 + 源首帧（粘贴算偏移用） | `copyClips()` |
| `recentStore` | 纯 UI 态 | 最近项目列表（≤12，localStorage） | 本地 |

镜像类 store 的注释把规则写死了（`projectStore.ts`）：「The UI never mutates `timeline` directly — every edit is an `edit_apply` command to Rust, whose event triggers a re-fetch.」

---

## projectStore.ts — 后端只读镜像

字段：`timelineVersion` / `timeline`（`EMPTY_TIMELINE` 兜底）/ `projectPath` / `lastSavedVersion` / `canUndo` / `canRedo`。
动作只有四个 setter：`setMirror(timeline, version)`、`setProjectPath`、`setHistory(canUndo, canRedo)`、`markSaved()`。**没有任何撤销/重做逻辑**——`canUndo/canRedo` 只是从 Rust 查来的「能否撤销」开关，撤销栈整体在 `opentake-ops`。脏标志 = `timelineVersion > lastSavedVersion`，驱动自动保存与「未保存」态。

## sync.ts — 镜像同步的唯一入口

- `startSync()`：幂等引导。先 `refreshMirror()`，再订阅 `timeline_changed{version}`（版本前进才重取，天然去重）与 `project_opened`。
- `refreshMirror()`：`api.getTimeline()` → `setMirror`，并 `Promise.all([canUndo, canRedo])` → `setHistory`。
- `forceRefresh()`：手动刷新。**浏览器 fallback 无事件通道**，编辑后由 actions 显式调用它对齐镜像。
- `stopSync()`：解绑监听。

事件流：UI 手势 → `editApply` →（Tauri）`timeline_changed` → `refreshMirror`；（浏览器）无事件 → actions `forceRefresh`。

## editActions.ts — 手势 → EditCommand 映射（最重）

所有编辑都经内部 `applyAndRefresh(cmd)`：`await api.editApply(cmd)`，且 `if (!isTauri && res.changed) await forceRefresh()`，使两端行为一致。导出动作分组：

- **clip 编辑**：`addClips` / `moveClips` / `duplicateClips`（Option 拖拽深拷贝、清 link group）/ `removeClips` / `splitClip` / `trimClips` / `setClipProperties`。
- **效果**：`setColorGrade` / `setChromaKey` / `setMasks` / `setEffects`。
- **链接/轨道**：`linkClips` / `unlinkClips` / `insertTrack(kind, at?)` / `setTrackProps`。
- **关键帧**：`setKeyframes` / `stampKeyframe` / `removeKeyframe` / `moveKeyframe` / `setKeyframeInterpolation`。
- **波纹**：`rippleDeleteRanges` / `rippleDeleteSelectedClips` / `tightenSilenceRanges`（复用 ripple）。
- **库**：`createFolder` / `moveToFolder` / `renameMedia` / `renameFolder` / `deleteMedia` / `deleteFolder` / `swapMedia`。
- **撤销/重做**：`undo()` / `redo()`（调 `api.undo/redo`，非 Tauri 时 `forceRefresh`）。
- **播放头/文本/剪贴板**：`splitAtPlayhead` / `trimStartToPlayhead` / `trimEndToPlayhead` / `addTextClip` / `deleteSelectedClips` / `rippleDeleteSelectedClips` / `copyClips` / `cutClips` / `pasteClipsAtPlayhead`。
- **媒体落轨**：`addMediaToTimeline` / `addMediaToTimelineAt`。

几个关键健壮性约定（注释固化，均为历史 bug 修复）：
- **`liveSelectedClipIds()`**：删除前先过滤掉选区里的「陈旧 id」。一个不存在的 id 会让 core 的 RemoveClips/RippleDelete 整批拒绝 → 表现为「删除毫无反应」。
- **`deleteSelectedClips()` 用 try/catch** 把后端拒绝转成 Toast，并 `clearSelection()`，不静默吞错。
- **`splitAtPlayhead()`**：有选区只切选中，无选区切播放头下所有 clip（无需先选）。
- **媒体落轨串行化**：`mediaAddQueue` 链式排队，避免连续拖放在第一个还在途时读到陈旧镜像、把 `startFrame` 算成 0 而被 overwrite 覆盖；空时间线落轨先 `insertTrack` 再 `forceRefresh` 取到新轨再放 clip（对齐上游 `placeClip` 自动建轨）。
- **粘贴**：`pasteClipsAtPlayhead()` 偏移 = `activeFrame - sourceFirstFrame`，新 clip 不 `addLinkedAudio`（链接音频本就在剪贴板里），落地后按旧 `linkGroupId` 分组重新 `linkClips`，源轨已不存在的 clip 静默跳过（对齐上游）。

## mediaActions.ts / projectActions.ts — 对话框驱动的生命周期手势

- `mediaActions`：`importFolderViaDialog` / `importFilesViaDialog` / `relinkMediaViaDialog` —— 「开原生对话框 → 调 `api.*` → `refreshMedia()`」。
- `projectActions`：`newProjectAndEnter`（保存对话框 → `projectNew`+`projectSave` → `setMirror`+记录 recent → 进编辑器）/ `saveCurrentProject` / `openProjectPath` / `openProjectViaDialog`。
- 非 Tauri 下对话框不可用（`dialog.ts` 返回 null），优雅降级。

## 其余 UI store

- `uiStore.ts`：UI-only 态合集（`view`/`activeFrame`/`isPlaying`/`isScrubbing`/`selectedClipIds`/`zoomScale`/`scrollLeft`/`toolMode`/`layoutPreset`/`focusedPanel`/`maximizedPanel`/各标签/`pendingSwapClipId`/`toast`…）。注释：「owned by the front end … never comes from Rust」；持久化键（布局/面板可见性）镜像到 localStorage。`togglePlay` 到末尾自动回 0；`focusPanel` 切面板时清理对应选择。
- `settingsStore.ts`：偏好 + `applyTheme()`/`setSize()` 副作用；BYOK **明文密钥不存这里**，走 OS keychain（见 [ipc-api.md](ipc-api.md)）。
- `clipboardStore.ts` / `recentStore.ts`：纯前端瞬态/持久化缓冲，无后端对应。

挂载点：`App.tsx` 的 `useEffect` 里 `startSync()` + `startMediaSync()`；`projectStore` 镜像与各 UI store 各自被组件订阅。

---

## 完成状态

- **已实现**：镜像同步、版本去重、编辑命令全量映射、媒体落轨串行化、复制/剪切/粘贴、删除健壮化、自动保存脏标志、库视图态派生。
- **计划中/占位**：fallback 下关键帧与库命令为 placeholder（见 [ipc-api.md](ipc-api.md)）；Agent 面板相关 UI 态尚未接通真实对话。

## 相关文档

- 命令的线上形态与 camelCase 契约 → [ipc-api.md](ipc-api.md)
- 像素↔帧换算（被 timeline 手势消费后转成 `EditRequest`）→ [timeline-ui.md](timeline-ui.md)
- IPC 对端（`edit_apply`/`get_timeline`/`timeline_changed`）→ [../src-tauri/INDEX.md](../src-tauri/INDEX.md)

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
