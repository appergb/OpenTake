# Preview Tabs Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 OpenTake 当前的单一媒体预览状态收敛为上游 Palmier Pro 的时间线 + 多个媒体素材 tab 栈，支持打开、切换、关闭和键盘导航，同时不破坏时间线预览。

**Architecture:** Zustand 保存稳定的 tab 顺序和 active tab ID，时间线永远保留为不可关闭的 `timeline` tab；媒体 tab 只保存媒体 ID，名称和类型从当前媒体镜像解析。所有现有 `setPreviewMedia(id|null)` 调用保留为兼容入口：非空值幂等打开并激活媒体 tab，空值激活时间线；删除媒体、进入文件夹和切换项目时通过统一 close/clear action 清理失效 tab。

**Tech Stack:** React 18, TypeScript, Zustand, Vitest, happy-dom, OpenTake `MediaStore` / `EditorUiStore`。

**Spec:** 上游 `palmier-pro-upstream/Sources/PalmierPro/Preview/PreviewTab.swift` 与 `Editor/ViewModel/EditorViewModel+PreviewTabs.swift`；主计划 `docs/superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md` 的 `UP-PREVIEW-TABS` 条目。

## Global Constraints

- 只修改 `OpenTake-generation/`；上游目录只读；不修改 `OpenTake/` 对照树。
- 时间线 tab ID 固定为 `timeline`，永远存在且不可关闭；媒体 tab ID 使用 `media_<assetId>`，重复打开同一素材必须幂等。
- 现有 `previewMediaId: string | null` 和 `setPreviewMedia(id | null)` 保持可用，外部调用不需要一次性迁移。
- 切换媒体 tab 时不改变 Timeline、播放头或项目权威状态；只同步媒体选中状态和 Preview 内容。
- 删除当前媒体或切换文件夹/项目不得留下指向不存在媒体的 active tab；失效 tab 自动关闭并回到 timeline 或最近有效 tab。
- 先写失败测试并实际看到失败，再写生产代码；不引入依赖，不修改媒体导入或 RenderPlan。

---

### Task 1: Preview tab store contract and UI tab stack

**Files:**
- Modify: `web/src/store/uiStore.ts`
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/store/mediaDeleteActions.ts`
- Modify: `web/src/hooks/useKeyboardShortcuts.ts`
- Test: `web/src/store/uiStore.test.ts`
- Test: `web/src/components/preview/Preview.interaction.test.tsx`
- Test: `web/src/components/preview/Preview.test.tsx`
- Test: `web/src/components/media/MediaPanel.test.tsx`

**Interfaces:**
- Add state `previewTabIds: string[]` with default `[]`; keep `previewActiveTabId: string` with default `timeline`.
- Add actions:
  - `openPreviewTab(mediaId: string): void`
  - `selectPreviewTab(tabId: string): void`
  - `closePreviewTab(tabId: string): void`
  - `closeAllPreviewTabs(): void`
  - `setPreviewMedia(id: string | null): void` as compatibility wrapper.
- `PreviewTabs` reads `previewTabIds` and current `MediaStore.items`, renders one timeline tab plus one tab per valid media ID, and exposes a close button only on media tabs.

- [x] **Step 1: Write failing store tests**

  Add tests that start with `{ previewTabIds: [], previewActiveTabId: "timeline", previewMediaId: null }` and assert:

  - `openPreviewTab("a")` produces `previewTabIds=["a"]`, active ID `media_a`, and `previewMediaId="a"`.
  - Opening `"a"` twice does not duplicate it; opening `"b"` appends it and activates `media_b`.
  - `selectPreviewTab("timeline")` clears `previewMediaId` without changing tab order.
  - Closing active `media_b` selects the previous valid tab; closing the last media tab returns to timeline; closing `timeline` is a no-op.
  - `closeAllPreviewTabs()` restores the exact timeline-only state.

  Run:

  `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-preview-tabs.json pnpm exec vitest run src/store/uiStore.test.ts`

  Expected: FAIL because the new actions/state do not exist.

- [x] **Step 2: Write failing interaction tests**

  Render `PreviewTabs` with media items `a` and `b`, open both through the store, then assert the tablist contains Timeline, A, B; clicking B activates it; clicking A's close button removes only A; ArrowLeft/ArrowRight/Home/End move focus and activation among tabs; the timeline tab has no close button.

  Run:

  `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-preview-tabs.json pnpm exec vitest run src/components/preview/Preview.interaction.test.tsx`

  Expected: FAIL because the current component renders at most one source tab and has no close controls.

- [x] **Step 3: Implement the minimal store contract**

  Implement the state/actions above in `createEditorUiStore`. Keep `previewMediaId` as derived-compatible state updated by every action so existing Preview, Inspector, keyboard shortcut, and media deletion callers keep working. Use a helper that validates tab IDs and always keeps `timeline` implicit rather than storing it in `previewTabIds`.

- [x] **Step 4: Implement the tab stack UI**

  Replace the current two-button `PreviewTabs({ item })` branch with a tab list derived from `previewTabIds`; resolve labels from `useMediaStore`. Use stable IDs `preview-timeline-tab` and `preview-media-tab-<assetId>`, `aria-selected`, `aria-controls`, and a close button with an accessible label. Keep the current source preview rendering keyed by the active `previewMediaId`.

- [x] **Step 5: Route existing callers through the contract**

  Replace direct `setState({ previewMediaId: null })` cleanup in MediaPanel and media deletion with `closeAllPreviewTabs()` or `closePreviewTab()` as appropriate. Keep keyboard shortcuts and timeline pointer actions on `setPreviewMedia(null)` so they continue to activate the timeline tab.

- [x] **Step 6: Run the green slice**

  Run:

  `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-preview-tabs.json pnpm exec vitest run src/store/uiStore.test.ts src/components/preview/Preview.interaction.test.tsx src/components/preview/Preview.test.tsx src/components/media/MediaPanel.test.tsx`

  Expected: all selected files pass, including existing single-source preview, media delete, folder navigation, keyboard navigation, and transform/crop guards.

- [x] **Step 7: Build and commit**

  Run `pnpm build` and `git diff --check`; commit only the listed source/test files with `feat(preview): support multiple media preview tabs` using the explicit worktree Git directory if needed.

  Result: commits `b22d822` + `bd06d62` + `4c8b761`; final tree contains only the listed source/test files; review clean; 4 files / 91 tests and Web build pass; installed app verified with two media tabs and close fallback.

## Acceptance

- Opening media A, then B, shows Timeline/A/B simultaneously; switching tabs changes only the preview source.
- Closing B activates the most recent remaining valid tab; Timeline cannot be closed.
- Deleting the active media or switching project/folder never leaves a dead active tab.
- Existing timeline playback, media playback, Inspector selection, and keyboard shortcuts continue to pass.
