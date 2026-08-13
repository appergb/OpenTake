# OpenTake Beta 5 Interface Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove misleading appearance controls, animate conditional copy without layout flashes, relocate library navigation, replace Home placeholders with useful 16:9 previews, and align macOS traffic lights with every title-bar icon.

**Architecture:** A shared disclosure primitive owns conditional text motion and reduced-motion behavior. Settings retain only the real dark standard/compact window choice with optimistic rollback. Library and Home are simplified at their source components. Project saves request an authoritative representative composite for thumbnails. All window chrome uses one CSS geometry contract mirrored by Tauri's macOS traffic-light configuration.

**Tech Stack:** React/TypeScript, Zustand, CSS tokens, Tauri 2, Rust compositor/image encoding, Vitest, packaged macOS GUI measurement.

## Global Constraints

- Always use the dark token set in Beta 5; do not display dark/light choices or persist a theme setting.
- Standard/compact options have stable equal geometry and no checkmark; failed native resize restores the prior selection.
- Conditional copy enters and exits through the shared primitive in 150–200ms, or immediately under `prefers-reduced-motion`.
- Put Library Home navigation at the top of the left category rail only.
- Remove Home generation activity UI and its Home-specific request/effect, but retain backend audit data used elsewhere.
- Use `aspect-ratio: 16 / 9` for project preview surfaces and actual project content where available.
- Final alignment acceptance comes from a packaged `.app`, not browser-only CSS inspection.

---

### Task 1: Create a shared disclosure motion primitive

**Files:**
- Create: `web/src/components/ui/Reveal.tsx`
- Create: `web/src/components/ui/Reveal.test.tsx`
- Modify: `web/src/styles/tokens.css`
- Modify: `web/src/styles/components.css`

**Interfaces:**
- `Reveal { open, children, id?, role?, onExited? }`
- CSS tokens `--motion-disclosure-duration` and `--motion-disclosure-ease`.

- [ ] **Step 1: Write failing lifecycle tests**

  Assert open content mounts, close retains content through exit then unmounts, rapid reopen cancels unmount, measured block size does not flash from auto/zero, focus leaves hidden content, and reduced-motion closes synchronously.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/components/ui/Reveal.test.tsx`. Expected: component and tokens are absent.

- [ ] **Step 3: Implement measured disclosure**

  Use a wrapper and inner content element with ResizeObserver, CSS custom block size, opacity, and translate. Keep layout reserved throughout exit, clean timers/listeners, handle dynamic content height, and set the duration token to zero in the existing reduced-motion media query.

- [ ] **Step 4: Verify GREEN**

  Run the focused test and `pnpm -C web build`.

- [ ] **Step 5: Commit the primitive**

  Commit as `feat(ui): add shared disclosure motion`.

### Task 2: Remove theme switching and make standard/compact reliable

**Files:**
- Modify: `web/src/store/settingsStore.ts`
- Create: `web/src/store/settingsStore.test.ts`
- Modify: `web/src/components/settings/SettingsView.tsx`
- Modify: `web/src/components/settings/SettingsView.interaction.test.tsx`
- Modify: `web/src/components/settings/SettingsView.visual.test.ts`
- Modify: `web/src/components/ui/Dropdown.tsx`
- Modify: `web/src/App.tsx`
- Modify: `web/src/App.lifecycle.test.tsx`
- Modify: `web/src/i18n/dict.ts`

**Interfaces:**
- delete `Theme`, `theme`, `setTheme`, `applyTheme`, and `initTheme`.
- startup clears legacy `theme` and versioned theme keys and sets `document.documentElement.dataset.theme = "dark"` only if compatibility CSS still requires it.
- `setWindowSize(mode)` returns/awaits native success and rolls state back on rejection.

- [ ] **Step 1: Write failing migration and rollback tests**

  Seed dark/light legacy keys and assert startup removes them and stays dark. Assert Appearance contains exactly two equal-width choices, “深色 · 标准” and “深色 · 紧凑”, no check icon, stable text offsets, native resize success, and rollback/error on rejection.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/store/settingsStore.test.ts src/components/settings/SettingsView.interaction.test.tsx src/components/settings/SettingsView.visual.test.ts`. Expected: current theme control/checkmark and optimistic failure behavior violate assertions.

- [ ] **Step 3: Remove the unused theme state and UI**

  Delete theme loading/storage/actions and App initialization. Replace the generic checked segmented appearance control with two stable layout cells whose selected state changes color/background only. Keep keyboard radiogroup semantics.

- [ ] **Step 4: Make native window selection transactional**

  Store the previous mode, call the existing Tauri resize command, commit/persist only on success, and restore plus toast on failure. Ignore a stale failure from an earlier click after a later choice has succeeded.

- [ ] **Step 5: Verify GREEN**

  Run focused tests, full settings tests, App lifecycle tests, and `pnpm -C web build`.

- [ ] **Step 6: Commit appearance changes**

  Commit as `fix(settings): keep only stable dark window layouts`.

### Task 3: Animate model-clear confirmation without a text jump

**Files:**
- Modify: `web/src/components/settings/StoragePane.tsx`
- Modify: `web/src/components/settings/StoragePane.test.tsx`
- Modify: `web/src/styles/components.css`

**Interfaces:**
- clear confirmation uses `Reveal` within the selected model row.
- destructive action remains disabled during deletion and returns to the stable row on completion/cancel.

- [ ] **Step 1: Write failing geometry and interaction tests**

  Assert the first clear click expands confirmation inside the row, explanation remains mounted during exit, sibling row top offsets change through the disclosure wrapper instead of immediate insertion, cancel/delete animate closed, repeated clicks do not duplicate copy, and reduced-motion is immediate.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/components/settings/StoragePane.test.tsx`. Expected: direct conditional text insertion fails lifecycle/geometry assertions.

- [ ] **Step 3: Integrate the shared primitive**

  Keep action and confirmation in a fixed row layout, animate the explanation/action group with `Reveal`, preserve focus on cancel, move focus to the next valid control after successful deletion, and keep backend failures visible through the same disclosure.

- [ ] **Step 4: Verify GREEN**

  Run Storage and Reveal tests plus `pnpm -C web build`.

- [ ] **Step 5: Commit storage motion**

  Commit as `fix(settings): animate model removal confirmation`.

### Task 4: Move Library Home navigation into the category rail

**Files:**
- Modify: `web/src/components/media/LibraryView.tsx`
- Modify: `web/src/components/media/LibraryView.test.tsx`
- Modify: `web/src/styles/components.css`

**Interfaces:**
- `CategoryTree` owns the single Home button above category content and beneath title-bar safe space.
- The right content header contains only category title, search, sort, and filter controls.

- [ ] **Step 1: Write the failing structure test**

  Assert one Home button exists, it is the first interactive element in the left navigation, it is absent from the right header for every category, and the category selection remains unchanged after return/re-entry.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/components/media/LibraryView.test.tsx`. Expected: current Home action is in the content header.

- [ ] **Step 3: Relocate the action and apply safe-area spacing**

  Pass the navigation callback into `CategoryTree`, render it once above the category list, and use shared title-bar safe-area tokens. Remove the old content-header button and any duplicate mobile rendering.

- [ ] **Step 4: Verify GREEN**

  Run Library tests and `pnpm -C web build`.

- [ ] **Step 5: Commit navigation adjustment**

  Commit as `fix(library): place Home navigation in the global rail`.

### Task 5: Remove Home generation activity and build useful 16:9 project cards

**Files:**
- Modify: `web/src/components/home/HomeView.tsx`
- Modify: `web/src/components/home/HomeView.test.tsx`
- Modify: `web/src/components/home/HomeView.interaction.test.tsx`
- Modify: `web/src/components/home/HomeView.visual.test.ts`
- Modify: `web/src/lib/api.ts`
- Modify: `web/src/styles/components.css`

**Interfaces:**
- no `GenerationActivity` state/effect/render branch in Home.
- project preview is a semantic figure with `aspect-ratio: 16 / 9`.
- fallback shows project name, canvas ratio, and a small track structure visualization.

- [ ] **Step 1: Write failing absence and card tests**

  Assert Home never calls the generation-activity API, contains no generation record region, renders thumbnail URLs as images with 16:9 geometry and object-fit cover, and renders a structured named fallback rather than a lone Film icon.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/components/home/HomeView.test.tsx src/components/home/HomeView.interaction.test.tsx src/components/home/HomeView.visual.test.ts`. Expected: generation region and 48px preview fail.

- [ ] **Step 3: Remove the Home-only generation request**

  Delete the component, state, effect, polling, imports, and Home invocation. Retain shared API methods only if another view/test calls them; otherwise remove the dead front-end wrapper without changing backend audit storage.

- [ ] **Step 4: Rebuild the card visual hierarchy**

  Use responsive card columns, a 16:9 preview figure, actual thumbnail when present, and a structured CSS fallback that exposes project title and known aspect/track metadata. Preserve project open, context menu, keyboard focus, and loading states.

- [ ] **Step 5: Verify GREEN**

  Run all Home tests and `pnpm -C web build`.

- [ ] **Step 6: Commit Home UI changes**

  Commit as `fix(home): simplify activity and show useful project previews`.

### Task 6: Generate project covers from the authoritative composite

**Files:**
- Modify: `crates/opentake-media/src/thumbnail/project.rs`
- Modify: `crates/opentake-media/src/thumbnail/mod.rs`
- Modify: `crates/opentake-media/src/lib.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/home.rs`

**Interfaces:**
- `capture_project_composite_thumbnail(snapshot, manifest, frame, bounds) -> Option<Vec<u8>>`
- save/close writes `thumbnail.jpg` atomically only after successful composite encode.

- [ ] **Step 1: Write failing composite thumbnail tests**

  Use a project with background video, overlay image/text, transform, and transition; assert the cover includes composite-layer evidence rather than the decoded representative source alone. Cover empty project, missing/offline source, invalid prior thumbnail, deterministic bounds, and atomic write failure.

- [ ] **Step 2: Verify RED**

  Run `cargo test -p opentake-media thumbnail::project::tests::composite_ -- --nocapture` and `cargo test -p opentake-tauri home::tests::thumbnail_ --lib`. Expected: current source-only capture fails layered fixtures.

- [ ] **Step 3: Reuse the compositor at a representative frame**

  Select a stable frame from visible content, build the same render snapshot as preview/export, composite at a bounded 16:9 output, encode JPEG, and atomically replace the bundle thumbnail. On capture failure retain the last valid thumbnail rather than deleting it.

- [ ] **Step 4: Verify GREEN**

  Run focused media/Tauri tests and the existing project save/open test suites.

- [ ] **Step 5: Commit composite covers**

  Commit as `feat(home): save composited project cover frames`.

### Task 7: Align traffic lights and title-bar controls to one geometry contract

**Files:**
- Modify: `web/src/components/shell/TitleBar.tsx`
- Modify: `web/src/components/shell/TitleBar.visual.test.ts`
- Modify: `web/src/components/shell/ShellComponentMapping.test.tsx`
- Modify: `web/src/styles/tokens.css`
- Modify: `web/src/styles/components.css`
- Modify: `src-tauri/tauri.conf.json`
- Create: `scripts/measure_titlebar_alignment.py`
- Create: `scripts/test_measure_titlebar_alignment.py`

**Interfaces:**
- CSS variables `--titlebar-height`, `--titlebar-center-y`, `--titlebar-control-size`, `--titlebar-safe-left`.
- measurement script accepts packaged-app PNG plus traffic-light/icon sample rectangles and fails above 1 CSS px center deviation.

- [ ] **Step 1: Write failing static and measurement tests**

  Require every left/right title-bar button to use the shared size/alignment class, forbid local vertical transforms/margins, and test the image measurement math with aligned and 2px-offset synthetic fixtures.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/components/shell/TitleBar.visual.test.ts src/components/shell/ShellComponentMapping.test.tsx` and `python3 -B -m unittest scripts/test_measure_titlebar_alignment.py`. Expected: missing shared geometry/script and current offsets fail.

- [ ] **Step 3: Consolidate title-bar geometry**

  Use one grid/flex center line and 26px control boxes for all navigation/action icons, remove per-button y nudges, and derive safe areas from tokens. Set Tauri's `trafficLightPosition.y` to the matching packaged macOS center after accounting for native button radius.

- [ ] **Step 4: Verify static GREEN**

  Run focused tests and `pnpm -C web build`.

- [ ] **Step 5: Measure the packaged app**

  Build the release `.app`, capture Home, Library, Motion Studio, and Editor title bars at 1x CSS scale, run the measurement script, and require the traffic-light group center and all icon centers to differ by no more than 1 CSS px.

- [ ] **Step 6: Commit geometry and evidence tooling**

  Commit as `fix(shell): align traffic lights and title-bar controls`.

### Task 8: Record packaged UI evidence

**Files:**
- Create: `docs/audit/2026-08-13/beta5-interface-polish.md`
- Create: `docs/audit/2026-08-13/screenshots/settings-dark-layouts.png`
- Create: `docs/audit/2026-08-13/screenshots/library-home-rail.png`
- Create: `docs/audit/2026-08-13/screenshots/home-project-cards.png`
- Create: `docs/audit/2026-08-13/screenshots/titlebar-alignment.png`

- [ ] **Step 1: Run automated UI gates**

  Run focused tests from Tasks 1–7, full Web tests/build, relevant Rust thumbnail/home tests, visual contract scripts, and `git diff --check`.

- [ ] **Step 2: Exercise model removal and layout switching**

  In the packaged app, record enter/exit for model clear at normal and reduced motion; switch standard/compact repeatedly; provoke one native resize error in a test harness and confirm rollback. Verify no text/checkmark shift and no light option.

- [ ] **Step 3: Exercise Library and Home**

  Verify the sole Library Home control is atop the left rail, Home has no generation activity, and saved projects with content display composite 16:9 covers while an empty legacy project displays the structured fallback.

- [ ] **Step 4: Record alignment receipts and commit**

  Store exact commands, screenshots, measured centers, and limitations; commit as `test(ui): verify Beta 5 interface polish`.
