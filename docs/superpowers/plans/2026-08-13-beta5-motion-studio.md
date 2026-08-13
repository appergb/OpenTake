# OpenTake Beta 5 Motion Studio Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a first-level Motion Studio where users and Agent edit the same real HTML/CSS files, preview deterministic visible animation, and publish through OpenTake's existing Chromium/FFmpeg atomic timeline path.

**Architecture:** Each project owns a capability-confined motion document directory containing a manifest, `index.html`, and `styles.css`. Tauri provides typed atomic document commands and delegates preview/render to `opentake-motion`. React presents a CodeMirror editor, live 16:9 canvas, parameters, and keyframe timeline. MCP document tools use the same backend with baseline hashes, so user and Agent edits converge on one source of truth.

**Tech Stack:** React 18, CodeMirror 6 MIT packages, TypeScript, Rust, Tauri 2, cap-std, SHA-256, headless Chromium CDP, FFmpeg, existing `motion_add`/`motion_edit`.

## Global Constraints

- Motion Studio is a top-level app view between Chat and Panel Management, never an Agent sub-tab.
- Only UTF-8 `index.html` and `styles.css` under the current project's controlled motion root are editable.
- Reject absolute paths, traversal, symlinks, network access, filesystem URLs, oversized documents, stale hashes, and unbounded render dimensions/duration.
- Save atomically and keep the last successful preview visible when a new preview fails.
- Preview and final render use the same source, dimensions, fps, duration, deterministic clock, and network-disabled Chromium sandbox.
- A failed or cancelled publish leaves media manifest and timeline unchanged.
- Record exact CodeMirror packages, versions, copyright, repository, and MIT license in third-party notices.

---

### Task 1: Add the CodeMirror dependency contract and license evidence

**Files:**
- Modify: `web/package.json`
- Modify: `web/pnpm-lock.yaml`
- Modify: `THIRD_PARTY_NOTICES.md`
- Modify: `scripts/check_license_inventory.py`
- Modify: `scripts/test_check_license_inventory.py`

**Interfaces:**
- Runtime packages: `codemirror`, `@codemirror/lang-html`, `@codemirror/lang-css`, `@codemirror/theme-one-dark`.
- License inventory maps each resolved package/version to its official MIT source and installed license file.

- [ ] **Step 1: Write the failing license inventory test**

  Require all four packages and their license entries, and add a mutation fixture that removes one notice or changes one resolved version.

- [ ] **Step 2: Verify RED**

  Run `python3 -B -m unittest scripts/test_check_license_inventory.py`. Expected: missing CodeMirror package/notice failures.

- [ ] **Step 3: Install pinned compatible dependencies**

  Use `pnpm -C web add codemirror@6.0.2 @codemirror/lang-html@6.4.12 @codemirror/lang-css@6.3.1 @codemirror/theme-one-dark@6.1.3`. Update notices from the installed packages and official repositories; do not add Animate.css, EasyLogic, or Motionity code.

- [ ] **Step 4: Verify GREEN**

  Run the license test, `pnpm -C web build`, and `pnpm -C web licenses list --prod` if the installed pnpm supports the command.

- [ ] **Step 5: Commit the dependency boundary**

  Commit as `build(motion): add licensed CodeMirror editor dependencies`.

### Task 2: Build the project-confined motion document store

**Files:**
- Create: `src-tauri/src/motion_documents.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- `MotionDocumentSummary { id, title, revision_hash, updated_at }`
- `MotionDocument { summary, html, css, parameters }`
- `MotionDocumentStore::{list,create,read,save_patch}`
- patch request includes `document_id`, `file`, `baseline_hash`, replacement edits, and expected result hash.

- [ ] **Step 1: Write failing store and confinement tests**

  Cover initial template creation, visible Chinese/English title/subtitle, atomic restart persistence, concurrent stale hash, traversal, absolute path, symlink escape, invalid UTF-8, oversized input, invalid manifest, and failed rename preserving prior content.

- [ ] **Step 2: Verify RED**

  Run `cargo test -p opentake-tauri motion_documents::tests --lib`. Expected: module and commands are absent.

- [ ] **Step 3: Implement capability-confined storage**

  Resolve the motion root from the currently saved project bundle, open it through `cap-std`, map document ids to generated safe directory names, and expose only the two known files. Normalize line endings, hash exact UTF-8 bytes, apply non-overlapping bounded edits, and atomically replace files plus manifest.

- [ ] **Step 4: Add typed commands**

  Register `motion_document_list`, `motion_document_create`, `motion_document_read`, and `motion_document_patch`; each captures current project identity and fails closed if the project changes before commit.

- [ ] **Step 5: Verify GREEN**

  Run the focused tests and `cargo test -p opentake-tauri motion_documents:: --lib`.

- [ ] **Step 6: Commit the store**

  Commit as `feat(motion): persist confined HTML and CSS documents`.

### Task 3: Add deterministic single-frame preview from the production renderer

**Files:**
- Modify: `crates/opentake-motion/src/source.rs`
- Modify: `crates/opentake-motion/src/renderer.rs`
- Modify: `crates/opentake-motion/src/sandbox.rs`
- Modify: `crates/opentake-motion/src/integration.rs`
- Modify: `src-tauri/src/motion.rs`

**Interfaces:**
- `MotionPreviewRequest { document_id, revision_hash, width, height, fps, duration_frames, frame }`
- `MotionPreviewResponse { revision_hash, frame, png_data_url, diagnostics }`
- HTML runtime calls `window.OpenTake.seek(seconds)` before deterministic capture.

- [ ] **Step 1: Write failing source and deterministic-clock tests**

  Assert generated source contains the document's real title/subtitle, local CSS, OpenTake seek bridge, blocked network policy, and no filesystem URL. Render the same frame twice and require identical decoded pixel hashes; render two animation frames and require a meaningful pixel difference.

- [ ] **Step 2: Verify RED**

  Run `cargo test -p opentake-motion preview_ --features chromium -- --nocapture`. Expected: document preview API is absent; live tests may explicitly skip only when the pinned Chromium sidecar is unavailable.

- [ ] **Step 3: Implement the preview source and capture path**

  Generate a self-contained HTML document from sanitized user HTML/CSS, inject deterministic animation controls before user content, disable fetch/XHR/WebSocket/navigation, apply dimensions/fps/frame bounds, and capture PNG through the existing CDP process manager. Return structured line/column diagnostics and retain no browser process after cancellation.

- [ ] **Step 4: Expose the Tauri command**

  Read the requested revision from `MotionDocumentStore`, reject a stale hash, call the production renderer, bound PNG/data URL size, and return sanitized diagnostics.

- [ ] **Step 5: Verify GREEN**

  Run default offline motion tests, feature-gated live preview tests, and `cargo test -p opentake-tauri motion::tests --lib`.

- [ ] **Step 6: Commit preview support**

  Commit as `feat(motion): preview real HTML and CSS deterministically`.

### Task 4: Add the Motion Studio top-level view and navigation entry

**Files:**
- Modify: `web/src/store/uiStore.ts`
- Modify: `web/src/store/uiStore.test.ts`
- Modify: `web/src/App.tsx`
- Modify: `web/src/App.lifecycle.test.tsx`
- Modify: `web/src/components/shell/TitleBar.tsx`
- Modify: `web/src/components/shell/TitleBar.interaction.test.tsx`
- Create: `web/src/components/motion/MotionStudio.tsx`
- Create: `web/src/components/motion/MotionStudio.test.tsx`
- Modify: `web/src/i18n/dict.ts`

**Interfaces:**
- `AppView` adds `motion`.
- Title-bar order: Home, Chat, Motion Studio, Panel Management.
- Motion view mounts independently while editor/chat state remains in stores.

- [ ] **Step 1: Write failing navigation tests**

  Assert the four buttons exist in exact order with 26px hit areas, selecting Motion mounts only Motion Studio, returning to Chat preserves the active chat session, and reloading an invalid persisted view falls back safely.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/store/uiStore.test.ts src/components/shell/TitleBar.interaction.test.tsx src/components/motion/MotionStudio.test.tsx`. Expected: `motion` is not a valid app view and no entry exists.

- [ ] **Step 3: Add the view and semantic shell**

  Extend the store and App view switch, add the title-bar button with localized label/tooltip, and create landmarks for file rail, editor, preview canvas, inspector, and timeline. Keep editor hooks mounted without rendering the editor layout in Motion view.

- [ ] **Step 4: Verify GREEN**

  Run focused tests and `pnpm -C web build`.

- [ ] **Step 5: Commit navigation**

  Commit as `feat(motion): add Motion Studio as a primary view`.

### Task 5: Implement the editor, live canvas, parameters, and keyframe strip

**Files:**
- Modify: `web/src/components/motion/MotionStudio.tsx`
- Create: `web/src/components/motion/MotionCodeEditor.tsx`
- Create: `web/src/components/motion/MotionPreview.tsx`
- Create: `web/src/components/motion/MotionTimeline.tsx`
- Create: `web/src/components/motion/MotionStudio.interaction.test.tsx`
- Create: `web/src/store/motionStudioStore.ts`
- Create: `web/src/store/motionStudioStore.test.ts`
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/api.ts`
- Modify: `web/src/styles/components.css`

**Interfaces:**
- HTML/CSS tabs backed by one controlled CodeMirror instance.
- 300ms debounced atomic patch, then preview; revision conflict offers reload or explicit reapply.
- Preview controls update frame without changing saved source.
- Publish parameters share width/height/fps/duration with backend request.

- [ ] **Step 1: Write failing store and UI tests**

  Cover document load, HTML/CSS tab state, visible initial text, debounced save, stale response suppression, compile diagnostic line/column, retained last-good frame, play/pause/replay/scrub, parameter bounds, narrow layout folding order, keyboard focus, and reduced motion.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/store/motionStudioStore.test.ts src/components/motion/MotionStudio.interaction.test.tsx`. Expected: store/components do not exist.

- [ ] **Step 3: Implement state and CodeMirror lifecycle**

  Create one editor view per mounted code panel, swap language extensions by active file, dispatch controlled source updates without cursor reset, dispose on unmount, and serialize saves through revision hashes. Keep errors adjacent to the affected source tab.

- [ ] **Step 4: Implement the authoring layout**

  Build the Songxia/Codex-inspired dark workspace with low chrome: files/templates/history left, code and 16:9 preview center, parameters right, frame ruler/keyframes below. Use real text in the starter document and semantic buttons/sliders.

- [ ] **Step 5: Implement deterministic preview scheduling**

  Abort superseded requests, ignore stale revision/frame replies, retain last success on failure, and ensure playback advances integer frames based on the configured fps.

- [ ] **Step 6: Verify GREEN**

  Run all Motion front-end tests, `pnpm -C web test`, and `pnpm -C web build`.

- [ ] **Step 7: Commit the authoring UI**

  Commit as `feat(motion): build HTML and CSS authoring workspace`.

### Task 6: Publish the document through the existing atomic timeline path

**Files:**
- Modify: `src-tauri/src/motion.rs`
- Modify: `crates/opentake-motion/src/integration.rs`
- Modify: `web/src/components/motion/MotionStudio.tsx`
- Modify: `web/src/lib/api.ts`
- Modify: `src-tauri/tests/motion_integration.rs`

**Interfaces:**
- `MotionAddCommand` and edit request accept `document_id` plus revision hash while retaining legacy code/template inputs.
- Publish response identifies the committed clip, media asset, render hash, and source document.

- [ ] **Step 1: Write failing publish integration tests**

  Publish a short visible text animation and verify decoded beginning/middle/end frames contain expected non-background pixels and differ over time. Cover cancellation, FFmpeg failure, stale document, invalid dimensions, reopen/re-render equivalence, and edit replacement without duplicate media registration.

- [ ] **Step 2: Verify RED**

  Run `OPENTAKE_RUN_FFMPEG_TESTS=1 cargo test -p opentake-tauri --test motion_integration -- --nocapture`. Expected: document-backed publish cases fail while existing Motion Canvas cases remain green.

- [ ] **Step 3: Connect document source to motion add/edit**

  Resolve the exact revision once, generate the same production source used by preview, render integer frames, encode through the provisioned FFmpeg sidecar, validate the output and cache manifest, then use the existing atomic add/edit commit. Remove staged output on every pre-commit failure/cancel path.

- [ ] **Step 4: Wire publish UI**

  Disable publish while unsaved or preview-invalid, show frame progress and cancellation, and navigate to/select the committed timeline clip only after the backend returns success.

- [ ] **Step 5: Verify GREEN**

  Run the integration suite, `cargo test -p opentake-motion --all-features`, the Tauri motion tests, and Motion front-end tests.

- [ ] **Step 6: Commit publishing**

  Commit as `feat(motion): publish Studio documents atomically`.

### Task 7: Give Agent conflict-safe Motion document tools

**Files:**
- Create: `crates/opentake-agent/src/mcp/motion_documents.rs`
- Modify: `crates/opentake-agent/src/mcp/mod.rs`
- Modify: `crates/opentake-agent/src/mcp/server.rs`
- Modify: `src-tauri/src/mcp.rs`
- Modify: `src-tauri/src/motion_documents.rs`

**Interfaces:**
- tools `list_motion_documents`, `read_motion_document`, `create_motion_document`, `patch_motion_document`, `preview_motion_document`, `publish_motion_document`.
- `MotionDocumentBridge` is capability-limited to current-project typed operations.

- [ ] **Step 1: Write failing schema and capability tests**

  Verify exact JSON schemas, read/list limits, hash-required patch, stale conflict, traversal/absolute/symlink rejection, preview bounds, publish admission, project switch cancellation, and no raw filesystem path in results.

- [ ] **Step 2: Verify RED**

  Run `cargo test -p opentake-agent mcp::motion_documents::tests -- --nocapture` and the Tauri MCP bridge tests. Expected: the bridge/tools are absent.

- [ ] **Step 3: Implement typed bridge and handlers**

  Keep filesystem access entirely behind the Tauri bridge, register tools in the shared plugin registry, return revision hashes on every read/write, translate conflicts into structured non-mutating results, and reuse the production preview/publish functions.

- [ ] **Step 4: Verify GREEN**

  Run all agent MCP tests and Tauri MCP tests, then use in-app Agent to change starter title/CSS and confirm the open editor receives the authoritative revision.

- [ ] **Step 5: Commit Agent integration**

  Commit as `feat(agent): edit Motion Studio documents with hash-safe tools`.

### Task 8: Verify real characters, animation, persistence, and cancellation

**Files:**
- Create: `docs/audit/2026-08-13/beta5-motion-studio.md`
- Create: `docs/audit/2026-08-13/screenshots/motion-studio-editor.png`
- Create: `docs/audit/2026-08-13/screenshots/motion-studio-preview.png`

- [ ] **Step 1: Run automated Motion gates**

  Run Rust default/all-feature tests, live Chromium tests, FFmpeg integration, all Motion UI tests, full Web tests/build, license inventory, and `git diff --check`.

- [ ] **Step 2: Exercise the packaged workflow**

  Create a document, visibly edit Chinese and English characters plus CSS animation, scrub frames, publish, reopen the project, and compare preview/published representative frames. Cancel a second publish and verify no new timeline/media entry.

- [ ] **Step 3: Exercise Agent co-editing**

  Ask Agent to patch the same document, provoke a stale hash conflict by typing concurrently, resolve explicitly, preview, and publish. Confirm no silent overwrite or path escape.

- [ ] **Step 4: Record evidence and commit**

  Capture source, preview, timeline result, exact hashes/commands, and screenshots; commit as `test(motion): verify Beta 5 Studio end to end`.
