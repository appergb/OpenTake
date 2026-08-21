# File Import Dialog Reliability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 macOS 原生“导入文件/重新链接文件”对话框选中文件后 Open 按钮仍禁用的问题，同时保留 Rust 后端的真实媒体白名单和文件夹导入行为。

**Architecture:** 文件/重链接 picker 不再依赖 macOS 上不稳定的混合扩展 `setAllowedFileTypes` 映射；后端继续负责扩展名、文件类型、路径安全和跳过原因校验。文件夹 picker 不改。前端测试锁定传给 native `open()` 的 options，安装版用同一 QA fixture 验证单选、多选、PNG/MP4 和文件夹导入。

**Tech Stack:** React/TypeScript, Tauri dialog plugin 2.7.x, Rust media importer, Vitest, macOS Computer Use。

**Spec:** `docs/audit/2026-08-21/full-desktop-functional-matrix.md` 的 Media Import 场景；Tauri `tauri-plugin-dialog` 本地类型和 `rfd` macOS panel implementation。

## Global Constraints

- 只修改 `OpenTake-generation/`；上游和 OpenTake 对照树只读。
- 不放宽 Rust `import_media` / `relink_media` 的安全校验，不复制或提交媒体 fixture/sidecar 二进制。
- 文件夹导入保持现状；文件/重链接 dialog 的 `directory:false`、`multiple`、`defaultPath` 语义不能改变。
- 先写失败 options contract 测试并看到失败，再改生产代码；不引入新依赖。
- 真实验收使用复制的 QA fixture，不覆盖用户原始工程/媒体；最终失败项必须写回 capability ledger。

---

### Task 1: Make native file selection actionable

**Files:**
- Modify: `web/src/store/mediaActions.ts`
- Test: `web/src/store/mediaActions.test.ts`

**Interfaces:**
- `importFilesViaDialog()` 继续调用 `open({ directory: false, multiple: true, defaultPath })`，但在受影响的 macOS native picker path 上不传混合 `filters`；后端仍决定支持/跳过。
- `relinkMediaViaDialog()` 继续调用 `open({ directory: false, multiple: false, defaultPath })`，避免同一 filter bug。

- [x] **Step 1: Add failing dialog options tests**

  Extend the existing dialog mock to capture the options passed to `open()`. Add tests asserting file import and relink do not pass the mixed media filter that maps to macOS `setAllowedFileTypes`; assert `directory`, `multiple`, and `defaultPath` remain unchanged. Keep the folder test asserting `directory:true` and no file picker options.

  Run:

  `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-import-dialog.json pnpm exec vitest run src/store/mediaActions.test.ts`

  Expected: FAIL because current file and relink actions pass `filters`.

- [x] **Step 2: Implement the minimum picker change**

  Remove or platform-gate only the file/relink `filters` field as selected by the failing test. Do not alter `importMedia(paths)`, `importFolder(path,true)`, `reportSkipped`, or Rust commands. If platform detection is needed, use the existing browser/runtime signal without adding a package and test both branches explicitly.

- [x] **Step 3: Run focused green tests**

  Run:

  `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-import-dialog.json pnpm exec vitest run src/store/mediaActions.test.ts src/components/media/MediaPanel.test.tsx`

  Expected: all selected tests pass, including project-switch cancellation, error handling, prewarm, folder import routing, and dialog options.

- [x] **Step 4: Build and verify source hygiene**

  Run `pnpm build` and `git diff --check`.

- [x] **Step 5: Verify the installed app**

  Build with the local CLI `web/node_modules/.bin/tauri build` from the project root, install the generated `.app`, and use Computer Use on a copied QA project:

  1. Open “导入媒体 → 导入文件”.
  2. Select one `.mp4`, one `.png`, and two files with Cmd multi-select; Open must be enabled.
  3. Confirm imported cards and real source preview appear.
  4. Repeat “导入文件夹”; confirm nested folder/media list and skipped unsupported count still work.
  5. If a missing asset is available, verify relink file selection and same asset identity.

- [x] **Step 6: Commit and write evidence**

  Commit only `web/src/store/mediaActions.ts` and `web/src/store/mediaActions.test.ts` with `fix(media): make native file import selectable`; update `docs/audit/2026-08-21/full-desktop-functional-matrix.md` and `docs/capabilities/CAPABILITY-LEDGER.md` after the real app result, not before.

  Result: commit `0bd7c6a5d4cf`; final tree contains only the two listed files. Installed app verified MP4/PNG/multi-select, relink, and folder import with nested directory plus skipped unsupported count.

  Mainline evidence: file/folder/relink verification completed in the installed app; the folder fixture produced a visible directory and `已跳过 64 个不支持的文件`.

## Acceptance

- On the installed macOS app, Open is enabled for supported `.mp4` and `.png` selections and multi-select.
- File/folder import both complete without weakening backend safety checks.
- Unsupported files remain explicitly skipped/reported rather than silently imported.
- Existing media action tests and build pass.
