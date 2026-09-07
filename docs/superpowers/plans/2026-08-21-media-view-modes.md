# Media View Modes Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 补齐上游 MediaTab 的 `folder / flat / grouped` 三态媒体组织视图，同时保留现有 grid/list 密度切换、搜索、排序、筛选、选择、拖拽和文件夹导航。

**Architecture:** 新增纯函数把 `MediaItem[] + MediaFolder[] + currentFolderId + mode` 投影为 folder items、flat items 或按文件夹分组的 sections；MediaPanel 只负责选择模式和渲染，Rust media mirror 不变。现有 grid/list 是展示密度，新的组织模式是独立状态，避免把两个概念混在一个枚举里。

**Tech Stack:** React/TypeScript, Zustand UI-local state, Vitest/happy-dom。

**Spec:** 上游 `MediaPanel/MediaTab/MediaTab.swift` 的 `ViewMode.folder/flat/grouped`，主计划 `UP-MEDIA-VIEW-MODES`。

## Global Constraints

- 只修改 `OpenTake-generation/`；不改 Rust importer、媒体 manifest 或上游目录。
- folder 模式保持现有层级浏览；flat 模式显示所有可见媒体且不显示 folder tiles；grouped 模式按 folder 分组显示 root/嵌套目录，空组不渲染。
- 搜索、类型筛选、收藏、排序、拖拽和 selection 使用同一 MediaItem ID，不改变媒体镜像。
- grid/list 只控制每个 section 的密度；folder/flat/grouped 切换不能丢失媒体选择或 Preview tab。
- 先写纯函数/DOM 失败测试，再实现；不引入依赖。

---

### Task 1: Add folder/flat/grouped projection and UI control

**Files:**
- Create: `web/src/lib/mediaViewModes.ts`
- Create: `web/src/lib/mediaViewModes.test.ts`
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/components/media/MediaPanel.test.tsx`
- Modify: `web/src/i18n/dict.ts`

**Interfaces:**
- `type MediaOrganizationMode = "folder" | "flat" | "grouped"`。
- `projectMediaView({ mode, items, folders, currentFolderId, query, typeFilter, favoriteOnly })` 返回不可变的 `MediaViewProjection`：`folders`, `items`, `groups`；`groups` 每项包含 `folderId: string | null`, `label`, `items`。
- `MediaPanel` 新增一个可访问 `媒体组织方式` menu，保留现有 `视图模式` grid/list button。

- [x] **Step 1: Write failing projection tests**

  Cover folder mode direct children, flat mode all folders, grouped mode root + nested labels, empty groups omitted, search/type/favorite filters applied consistently, and input arrays not mutated. Run the new test file and observe failure because the helper does not exist.

- [x] **Step 2: Implement pure projection helper**

  Implement deterministic projection with manifest order, stable folder labels, cycle-safe parent lookup, and no React/store dependencies.

- [x] **Step 3: Write failing MediaPanel interaction tests**

  Render a panel with two folders and three assets. Assert folder/flat/grouped menu changes `data-media-organization`, folder tiles disappear in flat, grouped headings show only non-empty groups, and existing grid/list toggle still changes `data-media-layout` inside the selected organization.

- [x] **Step 4: Implement the MediaPanel integration**

  Route current filtering through the helper, render folder mode using existing folder tiles, flat mode using existing cards/rows, and grouped mode using section headings plus existing cards/rows. Keep preview/selection handlers unchanged.

- [x] **Step 5: Run focused tests and build**

  Run:

  `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-media-modes.json pnpm exec vitest run src/lib/mediaViewModes.test.ts src/components/media/MediaPanel.test.tsx`

  Then `pnpm build` and `git diff --check`.

- [x] **Step 6: Verify the installed app and commit**

  Build/install the `.app`, use Computer Use to switch folder/flat/grouped with imported QA assets, verify search/selection/preview remain usable, then commit only the listed source/test/i18n files with `feat(media): add folder flat grouped view modes`.

## Acceptance

- All three upstream organization modes are visibly selectable and behaviorally distinct.
- Existing grid/list density, search, sort, filters, folders, selection, drag-to-timeline and Preview tabs continue to work.
- Pure projection tests and MediaPanel interaction tests pass; installed app shows all three modes without overflow.

## Verification Record

- Code review: initial review found and fixed dangling folder IDs plus empty-string root normalization; second independent review returned Ready with no P0–P3 findings.
- Focused tests: `mediaViewModes.test.ts` + `folderTree.test.ts` + `MediaPanel.test.tsx` → 3 files / 64 tests passed.
- Build: `pnpm build` and `web/node_modules/.bin/tauri build --bundles app` both exited 0; only pre-existing Vite chunk/dynamic-import and Rust future-incompatibility warnings remain.
- Installed app: `/Applications/OpenTake.app` SHA-256 `fb46fbcaa96ddd10d8e760160b47674c6f233e55effd88eca6d3d8b552092a51`.
- GUI evidence: folder/flat/grouped menu, real folder import, folder tile hiding in flat, root/nested grouped headings, grid/list density, folder breadcrumb and audio import/favorites subpages were exercised with Computer Use.
