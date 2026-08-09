# Home Shell Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 43 verified incomplete records in the `home-shell` gap group.

**Architecture:** Implement 16 primary evidence-bound slices and reference 0 shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.

---

### Task 1: HS-upstream-home-composite (implementation-slice-754223344a749699)

**Covered records:**
- `requirement-0ccbf12f850bf335` (requirement)

**Files:**
- Modify: `web/src/components/home/HomeView.tsx#HomeView`
- Modify: `web/src/store/recentStore.ts#useRecentStore`
- Modify: `web/src/store/projectActions.ts#openProjectPath`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Test (reviewed-planned): `web/src/components/home/HomeView.test.tsx#upstream_home_children_close_one_composite_acceptance`

**Candidate-bound contracts:**

#### requirement-0ccbf12f850bf335

- Candidate/source: `doc-f5044a046aaa65d8` at `docs/architecture/MODULE-PORT-MAP.md:122` (requirement)
- Expected behavior: Match the upstream Home shell: sidebar, project cards, missing/delete/context states, samples, welcome, and update surfaces.
- Resolution: `reviewed-mapping-report:HS-upstream-home-composite` — Core mapping report HS-upstream-home-composite: this umbrella aggregates registry, card, sample, welcome and update child contracts.
- Exact acceptance contract:
  - Implementation: Implement every listed Home surface with cross-platform equivalents, including verified missing-project and safe trash/delete behavior; add keyboard/accessibility/visual tests and a packaged-app runtime walkthrough.
  - Add browser interaction tests for every named Home, project, persistence, error, keyboard, and state transition; the affected web and Rust suites must pass.
  - Exercise the packaged application through create/open/close/reopen or the named Home path and retain exact runtime evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/home/HomeView.test.tsx#upstream_home_children_close_one_composite_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/home/HomeView.test.tsx -t "upstream_home_children_close_one_composite_acceptance"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/home/HomeView.tsx#HomeView`, `web/src/store/recentStore.ts#useRecentStore`, `web/src/store/projectActions.ts#openProjectPath`, `docs/architecture/MODULE-PORT-MAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/home/HomeView.test.tsx -t "upstream_home_children_close_one_composite_acceptance"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. The missing exact composite runner first produced a zero-candidate skipped RED baseline, then passed 1/1 after the Home boundary covered sidebar, samples, first-run welcome, version update, project-card keyboard open, missing/context state, and safe-trash confirmation. Welcome/update state uses the build-time application version, persists only after dismissal, receives initial focus, and supports Escape. The complete Web suite passed 82 files / 774 tests and the production build passed with only the pre-existing bundle warnings. The rebuilt macOS app displayed the v1.0.0 update surface, returned to the complete Home shell, and did not redisplay it after quit/relaunch; child runtime records cover native new/open/sample, safe trash, autosave/reopen, and secondary controls. Runtime evidence: [`home-upstream-composite-real-device-2026-07-31.md`](../runtime-artifacts/automated/home-upstream-composite-real-device-2026-07-31.md).

### Task 2: HS-new-open-sample (implementation-slice-406b2853a5d6f67b)

**Covered records:**
- `requirement-31a4c6a115076c19` (requirement)

**Files:**
- Modify: `web/src/components/home/HomeView.tsx#HomeView`
- Modify: `web/src/store/projectActions.ts#newProjectAndEnter`
- Modify: `web/src/store/projectActions.ts#openProjectPath`
- Modify: `web/src/store/projectActions.ts#openProjectViaDialog`
- Modify: `src-tauri/src/samples.rs#SampleProjectService`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Test (reviewed-planned): `src-tauri/src/samples.rs#failed_materialization_rolls_back_entire_sample_directory`
- Test (reviewed-planned): `web/src/components/home/HomeView.test.tsx#new_open_sample_register_only_after_success_and_route_tutorial`

**Candidate-bound contracts:**

#### requirement-31a4c6a115076c19

- Candidate/source: `doc-3b039486a387af31` at `docs/architecture/MODULE-PORT-MAP.md:1025` (requirement)
- Expected behavior: Provide new/open/sample project flows with safe save/open errors, recent registration, and tutorial routing.
- Resolution: `reviewed-mapping-report:HS-new-open-sample` — Core mapping report HS-new-open-sample: new, open and sample flows must register only after success, roll back atomically and route tutorial state.
- Exact acceptance contract:
  - Implementation: Implement cross-platform save/open/sample flows covering default location/name, corrupt-open error UI, sample materialization/progress without recent registration, optional tutorial start, and test packaged-app behavior.
  - Add browser interaction tests for every named Home, project, persistence, error, keyboard, and state transition; the affected web and Rust suites must pass.
  - Exercise the packaged application through create/open/close/reopen or the named Home path and retain exact runtime evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/samples.rs#failed_materialization_rolls_back_entire_sample_directory` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/components/home/HomeView.test.tsx#new_open_sample_register_only_after_success_and_route_tutorial` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri failed_materialization_rolls_back_entire_sample_directory`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.test.tsx -t "new_open_sample_register_only_after_success_and_route_tutorial"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/home/HomeView.tsx#HomeView`, `web/src/store/projectActions.ts#newProjectAndEnter`, `web/src/store/projectActions.ts#openProjectPath`, `web/src/store/projectActions.ts#openProjectViaDialog`, `src-tauri/src/samples.rs#SampleProjectService`, `docs/architecture/MODULE-PORT-MAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri failed_materialization_rolls_back_entire_sample_directory`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.test.tsx -t "new_open_sample_register_only_after_success_and_route_tutorial"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Runtime evidence: [`home-sample-project-real-device-2026-07-31.md`](../runtime-artifacts/automated/home-sample-project-real-device-2026-07-31.md).

### Task 3: HS-project-card-lifecycle (implementation-slice-1136a3fb2728f04d)

**Covered records:**
- `requirement-c4af18634c223a8d` (requirement)

**Files:**
- Modify: `src-tauri/src/home.rs#ProjectRegistry`
- Modify: `web/src/store/recentStore.ts#useRecentStore`
- Modify: `web/src/components/home/HomeView.tsx#ProjectGridCard`
- Modify: `docs/architecture/PORT-1TO1-GAP.md`
- Test (reviewed-planned): `src-tauri/src/home.rs#missing_entry_survives_registry_load_and_safe_trash_removes_only_after_success`
- Test (reviewed-planned): `web/src/components/home/HomeView.test.tsx#missing_card_reveal_remove_and_trash_states`

**Candidate-bound contracts:**

#### requirement-c4af18634c223a8d

- Candidate/source: `doc-556463403ce36ef2` at `docs/architecture/PORT-1TO1-GAP.md:106` (requirement)
- Expected behavior: Home project cards show missing state and offer reveal plus safe trash/delete actions.
- Resolution: `reviewed-mapping-report:HS-project-card-lifecycle` — Core mapping report HS-project-card-lifecycle: persistent missing, reveal and safe-trash states require one shared registry-backed card lifecycle.
- Exact acceptance contract:
  - Implementation: Preserve missing entries instead of silently filtering them, render a clear missing state, add reveal and capability-safe move-to-trash/delete confirmation actions, update recents atomically, and test existing/missing/permission-denied cases.
  - Add browser interaction tests for every named Home, project, persistence, error, keyboard, and state transition; the affected web and Rust suites must pass.
  - Exercise the packaged application through create/open/close/reopen or the named Home path and retain exact runtime evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/home.rs#missing_entry_survives_registry_load_and_safe_trash_removes_only_after_success` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/components/home/HomeView.test.tsx#missing_card_reveal_remove_and_trash_states` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri missing_entry_survives_registry_load_and_safe_trash_removes_only_after_success`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.test.tsx -t "missing_card_reveal_remove_and_trash_states"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/home.rs#ProjectRegistry`, `web/src/store/recentStore.ts#useRecentStore`, `web/src/components/home/HomeView.tsx#ProjectGridCard`, `docs/architecture/PORT-1TO1-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri missing_entry_survives_registry_load_and_safe_trash_removes_only_after_success`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.test.tsx -t "missing_card_reveal_remove_and_trash_states"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Runtime evidence: [`home-project-lifecycle-real-device-2026-07-31.md`](../runtime-artifacts/automated/home-project-lifecycle-real-device-2026-07-31.md).

### Task 4: HS-autosave-metadata-mixed (implementation-slice-c50679ed41628b06)

**Covered records:**
- `requirement-034f4a07aa8a3c47` (requirement)

**Files:**
- Modify: `web/src/store/recentStore.ts#useRecentStore`
- Modify: `web/src/store/projectActions.ts#saveCurrentProject`
- Modify: `docs/architecture/PORT-1TO1-GAP.md`
- Test (reviewed-planned): `web/src/store/recentStore.test.ts#autosave_and_home_metadata_have_separate_owners`

**Candidate-bound contracts:**

#### requirement-034f4a07aa8a3c47

- Candidate/source: `doc-46fa377617586b27` at `docs/architecture/PORT-1TO1-GAP.md:157` (requirement)
- Expected behavior: Autosave/close flush and Home project metadata all meet the handoff acceptance criteria.
- Resolution: `reviewed-mapping-report:HS-autosave-metadata-mixed` — Core mapping report HS-autosave-metadata-mixed: autosave evidence and Home registry metadata must be accepted as separately owned child contracts.
- Exact acceptance contract:
  - Implementation: Keep the existing debounced save and close flush; add persisted project thumbnail/modified metadata to recents, render relative time and missing state, and integration-test edit→autosave→close→reopen plus Home metadata.
  - Add browser interaction tests for every named Home, project, persistence, error, keyboard, and state transition; the affected web and Rust suites must pass.
  - Exercise the packaged application through create/open/close/reopen or the named Home path and retain exact runtime evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/store/recentStore.test.ts#autosave_and_home_metadata_have_separate_owners` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/store/recentStore.test.ts -t "autosave_and_home_metadata_have_separate_owners"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/store/recentStore.ts#useRecentStore`, `web/src/store/projectActions.ts#saveCurrentProject`, `docs/architecture/PORT-1TO1-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/store/recentStore.test.ts -t "autosave_and_home_metadata_have_separate_owners"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

Runtime evidence: [`home-autosave-metadata-real-device-2026-07-31.md`](../runtime-artifacts/automated/home-autosave-metadata-real-device-2026-07-31.md).

### Task 5: HS-layout-geometry + CC-layout-misgrouped (implementation-slice-395babc4fe771bb4)

**Covered records:**
- `requirement-2269e95a893ffbfe` (requirement)
- `requirement-bb4951d728a9bcff` (requirement)
- `requirement-c531ff6b55858bc0` (requirement)
- `requirement-30c0b150880b10ef` (requirement)
- `requirement-7fed82947ea8910f` (requirement)
- `requirement-798893a778f1813e` (requirement)
- `requirement-eeba721f50c7b42d` (requirement)
- `requirement-e36316e0aa8cd79f` (requirement)
- `requirement-6bb9a832824dd470` (requirement)
- `requirement-2d93bc046644f4d8` (requirement)

**Files:**
- Modify: `web/src/store/uiStore.ts#useEditorUiStore`
- Modify: `web/src/components/shell/EditorSplit.tsx#EditorSplit`
- Modify: `web/src/components/shell/EditorSplit.tsx#DefaultLayout`
- Modify: `web/src/components/shell/EditorSplit.tsx#MediaLayout`
- Modify: `web/src/components/shell/EditorSplit.tsx#VerticalLayout`
- Modify: `web/src/components/ui/PanelShell.tsx#PanelShell`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `web/src/components/shell/ViewMenu.tsx#ViewMenu`
- Modify: `docs/specs/frontend/2-layout.md`
- Test (reviewed-planned): `web/src/components/shell/EditorSplit.test.tsx#all_presets_match_geometry_visibility_maximize_and_focus_shell`

**Candidate-bound contracts:**

#### requirement-2269e95a893ffbfe

- Candidate/source: `doc-f67f7b3bb62f249d` at `docs/modules/web/SPEC.md:1263` (requirement)
- Expected behavior: Match the three panel-layout presets, gutter geometry, corner radii, and focus ring to the pinned upstream UI.
- Resolution: `reviewed-mapping-report:HS-layout-geometry` — Core mapping report HS-layout-geometry: the native home-shell layout capability owns preset geometry, visibility, maximize and focus behavior.
- Exact acceptance contract:
  - Create a pinned upstream/current screenshot-state matrix at the same viewport, scale, locale, theme, and fixture.
  - Add deterministic component/browser assertions for normal, hover, focus, disabled, selected, empty, error, and compact states.
  - Record reviewed pixel/geometry evidence for every item named by the checklist, with no unchecked item remaining.

#### requirement-bb4951d728a9bcff

- Candidate/source: `doc-cb6331d0a87b31ff` at `docs/specs/frontend/2-layout.md:1` (requirement)
- Expected behavior: At docs/specs/frontend/2-layout.md:1 under “窗口外壳与五面板布局” (heading), the source “# 窗口外壳与五面板布局” requires this exact behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.
- Resolution: `reviewed-mapping-report:CC-layout-misgrouped` — Core mapping report CC-layout-misgrouped: nine command-classified records are acceptance inputs to the single home-shell layout capability.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/2-layout.md:1; signal=heading; heading=窗口外壳与五面板布局; candidate=# 窗口外壳与五面板布局
  - Expected behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar. This closes only the promise expressed by “窗口外壳与五面板布局” in “窗口外壳与五面板布局”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “窗口外壳与五面板布局” with the scenario below and register test:web/src/__tests__/completion/doc-cb6331d0a87b31ff.test.ts#completion_cb6331d0a87b31ff_the_editor_shell_implements_the_specified_panel_
  - Initial state/input/event: render the exact “窗口外壳与五面板布局” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “窗口外壳与五面板布局”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-cb6331d0a87b31ff.test.ts#completion_cb6331d0a87b31ff_the_editor_shell_implements_the_specified_panel_.

#### requirement-c531ff6b55858bc0

- Candidate/source: `doc-27a0886bb2101ab9` at `docs/specs/frontend/2-layout.md:5` (requirement)
- Expected behavior: At docs/specs/frontend/2-layout.md:5 under “2.1 五个面板 + LayoutPreset” (heading), the source “### 2.1 五个面板 + LayoutPreset” requires this exact behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.
- Resolution: `reviewed-mapping-report:CC-layout-misgrouped` — Core mapping report CC-layout-misgrouped: nine command-classified records are acceptance inputs to the single home-shell layout capability.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/2-layout.md:5; signal=heading; heading=2.1 五个面板 + LayoutPreset; candidate=### 2.1 五个面板 + LayoutPreset
  - Expected behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar. This closes only the promise expressed by “2.1 五个面板 + LayoutPreset” in “2.1 五个面板 + LayoutPreset”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.1 五个面板 + LayoutPreset” with the scenario below and register test:web/src/__tests__/completion/doc-27a0886bb2101ab9.test.ts#completion_27a0886bb2101ab9_the_editor_shell_implements_the_specified_panel_
  - Initial state/input/event: render the exact “2.1 五个面板 + LayoutPreset” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “2.1 五个面板 + LayoutPreset”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-27a0886bb2101ab9.test.ts#completion_27a0886bb2101ab9_the_editor_shell_implements_the_specified_panel_.

#### requirement-30c0b150880b10ef

- Candidate/source: `doc-5bf653fb82f34c4d` at `docs/specs/frontend/2-layout.md:20` (requirement)
- Expected behavior: At docs/specs/frontend/2-layout.md:20 under “2.2 Default 布局（`EditorView.swift:207-230`）” (heading), the source “### 2.2 Default 布局（`EditorView.swift:207-230`）” requires this exact behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.
- Resolution: `reviewed-mapping-report:CC-layout-misgrouped` — Core mapping report CC-layout-misgrouped: nine command-classified records are acceptance inputs to the single home-shell layout capability.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/2-layout.md:20; signal=heading; heading=2.2 Default 布局（`EditorView.swift:207-230`）; candidate=### 2.2 Default 布局（`EditorView.swift:207-230`）
  - Expected behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar. This closes only the promise expressed by “2.2 Default 布局（`EditorView.swift:207-230`）” in “2.2 Default 布局（`EditorView.swift:207-230`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.2 Default 布局（`EditorView.swift:207-230`）” with the scenario below and register test:web/src/__tests__/completion/doc-5bf653fb82f34c4d.test.ts#completion_5bf653fb82f34c4d_the_editor_shell_implements_the_specified_panel_
  - Initial state/input/event: render the exact “2.2 Default 布局（`EditorView.swift:207-230`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “2.2 Default 布局（`EditorView.swift:207-230`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-5bf653fb82f34c4d.test.ts#completion_5bf653fb82f34c4d_the_editor_shell_implements_the_specified_panel_.

#### requirement-7fed82947ea8910f

- Candidate/source: `doc-00898f0c99694bd0` at `docs/specs/frontend/2-layout.md:34` (requirement)
- Expected behavior: At docs/specs/frontend/2-layout.md:34 under “2.3 Media 布局（`EditorView.swift:235-261`）” (heading), the source “### 2.3 Media 布局（`EditorView.swift:235-261`）” requires this exact behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.
- Resolution: `reviewed-mapping-report:CC-layout-misgrouped` — Core mapping report CC-layout-misgrouped: nine command-classified records are acceptance inputs to the single home-shell layout capability.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/2-layout.md:34; signal=heading; heading=2.3 Media 布局（`EditorView.swift:235-261`）; candidate=### 2.3 Media 布局（`EditorView.swift:235-261`）
  - Expected behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar. This closes only the promise expressed by “2.3 Media 布局（`EditorView.swift:235-261`）” in “2.3 Media 布局（`EditorView.swift:235-261`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.3 Media 布局（`EditorView.swift:235-261`）” with the scenario below and register test:web/src/__tests__/completion/doc-00898f0c99694bd0.test.ts#completion_00898f0c99694bd0_the_editor_shell_implements_the_specified_panel_
  - Initial state/input/event: render the exact “2.3 Media 布局（`EditorView.swift:235-261`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “2.3 Media 布局（`EditorView.swift:235-261`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-00898f0c99694bd0.test.ts#completion_00898f0c99694bd0_the_editor_shell_implements_the_specified_panel_.

#### requirement-798893a778f1813e

- Candidate/source: `doc-86004c9831252243` at `docs/specs/frontend/2-layout.md:48` (requirement)
- Expected behavior: At docs/specs/frontend/2-layout.md:48 under “2.4 Vertical 布局（`EditorView.swift:266-288`）” (heading), the source “### 2.4 Vertical 布局（`EditorView.swift:266-288`）” requires this exact behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.
- Resolution: `reviewed-mapping-report:CC-layout-misgrouped` — Core mapping report CC-layout-misgrouped: nine command-classified records are acceptance inputs to the single home-shell layout capability.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/2-layout.md:48; signal=heading; heading=2.4 Vertical 布局（`EditorView.swift:266-288`）; candidate=### 2.4 Vertical 布局（`EditorView.swift:266-288`）
  - Expected behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar. This closes only the promise expressed by “2.4 Vertical 布局（`EditorView.swift:266-288`）” in “2.4 Vertical 布局（`EditorView.swift:266-288`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.4 Vertical 布局（`EditorView.swift:266-288`）” with the scenario below and register test:web/src/__tests__/completion/doc-86004c9831252243.test.ts#completion_86004c9831252243_the_editor_shell_implements_the_specified_panel_
  - Initial state/input/event: render the exact “2.4 Vertical 布局（`EditorView.swift:266-288`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “2.4 Vertical 布局（`EditorView.swift:266-288`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-86004c9831252243.test.ts#completion_86004c9831252243_the_editor_shell_implements_the_specified_panel_.

#### requirement-eeba721f50c7b42d

- Candidate/source: `doc-d8ead309f068d66a` at `docs/specs/frontend/2-layout.md:64` (requirement)
- Expected behavior: At docs/specs/frontend/2-layout.md:64 under “2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）” (heading), the source “### 2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）” requires this exact behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.
- Resolution: `reviewed-mapping-report:CC-layout-misgrouped` — Core mapping report CC-layout-misgrouped: nine command-classified records are acceptance inputs to the single home-shell layout capability.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/2-layout.md:64; signal=heading; heading=2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）; candidate=### 2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）
  - Expected behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar. This closes only the promise expressed by “2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）” in “2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）” with the scenario below and register test:web/src/__tests__/completion/doc-d8ead309f068d66a.test.ts#completion_d8ead309f068d66a_the_editor_shell_implements_the_specified_panel_
  - Initial state/input/event: render the exact “2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-d8ead309f068d66a.test.ts#completion_d8ead309f068d66a_the_editor_shell_implements_the_specified_panel_.

#### requirement-e36316e0aa8cd79f

- Candidate/source: `doc-178b40f8bf33a4bd` at `docs/specs/frontend/2-layout.md:75` (requirement)
- Expected behavior: At docs/specs/frontend/2-layout.md:75 under “2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）” (heading), the source “### 2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）” requires this exact behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.
- Resolution: `reviewed-mapping-report:CC-layout-misgrouped` — Core mapping report CC-layout-misgrouped: nine command-classified records are acceptance inputs to the single home-shell layout capability.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/2-layout.md:75; signal=heading; heading=2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）; candidate=### 2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）
  - Expected behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar. This closes only the promise expressed by “2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）” in “2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）” with the scenario below and register test:web/src/__tests__/completion/doc-178b40f8bf33a4bd.test.ts#completion_178b40f8bf33a4bd_the_editor_shell_implements_the_specified_panel_
  - Initial state/input/event: render the exact “2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-178b40f8bf33a4bd.test.ts#completion_178b40f8bf33a4bd_the_editor_shell_implements_the_specified_panel_.

#### requirement-6bb9a832824dd470

- Candidate/source: `doc-e0ba239c4bb5fb3c` at `docs/specs/frontend/2-layout.md:81` (requirement)
- Expected behavior: At docs/specs/frontend/2-layout.md:81 under “2.7 面板可见性 / 折叠 / 最大化” (heading), the source “### 2.7 面板可见性 / 折叠 / 最大化” requires this exact behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.
- Resolution: `reviewed-mapping-report:CC-layout-misgrouped` — Core mapping report CC-layout-misgrouped: nine command-classified records are acceptance inputs to the single home-shell layout capability.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/2-layout.md:81; signal=heading; heading=2.7 面板可见性 / 折叠 / 最大化; candidate=### 2.7 面板可见性 / 折叠 / 最大化
  - Expected behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar. This closes only the promise expressed by “2.7 面板可见性 / 折叠 / 最大化” in “2.7 面板可见性 / 折叠 / 最大化”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.7 面板可见性 / 折叠 / 最大化” with the scenario below and register test:web/src/__tests__/completion/doc-e0ba239c4bb5fb3c.test.ts#completion_e0ba239c4bb5fb3c_the_editor_shell_implements_the_specified_panel_
  - Initial state/input/event: render the exact “2.7 面板可见性 / 折叠 / 最大化” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “2.7 面板可见性 / 折叠 / 最大化”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-e0ba239c4bb5fb3c.test.ts#completion_e0ba239c4bb5fb3c_the_editor_shell_implements_the_specified_panel_.

#### requirement-2d93bc046644f4d8

- Candidate/source: `doc-59fbdae28c9200a7` at `docs/specs/frontend/2-layout.md:87` (requirement)
- Expected behavior: At docs/specs/frontend/2-layout.md:87 under “2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）” (heading), the source “### 2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）” requires this exact behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.
- Resolution: `reviewed-mapping-report:CC-layout-misgrouped` — Core mapping report CC-layout-misgrouped: nine command-classified records are acceptance inputs to the single home-shell layout capability.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/2-layout.md:87; signal=heading; heading=2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）; candidate=### 2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）
  - Expected behavior: The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar. This closes only the promise expressed by “2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）” in “2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）” with the scenario below and register test:web/src/__tests__/completion/doc-59fbdae28c9200a7.test.ts#completion_59fbdae28c9200a7_the_editor_shell_implements_the_specified_panel_
  - Initial state/input/event: render the exact “2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The editor shell implements the specified panel presets, visibility, focus treatment, and titlebar.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-59fbdae28c9200a7.test.ts#completion_59fbdae28c9200a7_the_editor_shell_implements_the_specified_panel_.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/EditorSplit.test.tsx#all_presets_match_geometry_visibility_maximize_and_focus_shell` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/shell/EditorSplit.test.tsx -t "all_presets_match_geometry_visibility_maximize_and_focus_shell"`

  Expected: FAIL because one or more of the 10 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/store/uiStore.ts#useEditorUiStore`, `web/src/components/shell/EditorSplit.tsx#EditorSplit`, `web/src/components/shell/EditorSplit.tsx#DefaultLayout`, `web/src/components/shell/EditorSplit.tsx#MediaLayout`, `web/src/components/shell/EditorSplit.tsx#VerticalLayout`, `web/src/components/ui/PanelShell.tsx#PanelShell`, `docs/modules/web/SPEC.md`, `web/src/components/shell/ViewMenu.tsx#ViewMenu`, `docs/specs/frontend/2-layout.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/EditorSplit.test.tsx -t "all_presets_match_geometry_visibility_maximize_and_focus_shell"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 6: HS-schema-safe-persistence (implementation-slice-b1933047ba301b69)

**Covered records:**
- `requirement-1a4a6f9bc9a1ee88` (requirement)
- `requirement-6cc3ccf2f7420f7f` (requirement)

**Files:**
- Modify: `web/src/store/uiStore.ts#loadBool`
- Modify: `web/src/store/uiStore.ts#loadPreset`
- Modify: `web/src/store/uiStore.ts#persist`
- Modify: `web/src/store/uiStore.ts#useEditorUiStore`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Test (reviewed-planned): `web/src/store/uiStore.persistence.test.ts#schema_safe_layout_panel_and_keyframe_state_survive_restart`

**Candidate-bound contracts:**

#### requirement-1a4a6f9bc9a1ee88

- Candidate/source: `doc-2a61d6818d85d37f` at `docs/modules/web/SPEC.md:1289` (requirement)
- Expected behavior: Persist layout preset, panel visibility, and keyframe-panel visibility across sessions with schema-safe defaults.
- Resolution: `reviewed-mapping-report:HS-schema-safe-persistence` — Core mapping report HS-schema-safe-persistence: layout, panel and keyframe state need one restart-safe persistence contract.
- Exact acceptance contract:
  - Add isolated localStorage tests for every persisted key, valid/invalid legacy value, unavailable/throwing storage, and new-store rehydration.
  - Verify each toggle writes only its own key and the next store instance restores the same UI state without leaking project-scoped state.

#### requirement-6cc3ccf2f7420f7f

- Candidate/source: `doc-1daa44ecea72d622` at `docs/specs/frontend/13-implementation.md:46` (requirement)
- Expected behavior: Persist layout preset, panel visibility, and keyframe UI state across sessions.
- Resolution: `reviewed-mapping-report:HS-schema-safe-persistence` — Core mapping report HS-schema-safe-persistence: layout, panel and keyframe state need one restart-safe persistence contract.
- Exact acceptance contract:
  - Implementation: Define versioned storage keys/defaults/migration, reload all required states in a fresh store/browser context, and add corruption plus cross-session tests.
  - Add browser interaction tests for every named Home, project, persistence, error, keyboard, and state transition; the affected web and Rust suites must pass.
  - Exercise the packaged application through create/open/close/reopen or the named Home path and retain exact runtime evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/store/uiStore.persistence.test.ts#schema_safe_layout_panel_and_keyframe_state_survive_restart` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/store/uiStore.persistence.test.ts -t "schema_safe_layout_panel_and_keyframe_state_survive_restart"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/store/uiStore.ts#loadBool`, `web/src/store/uiStore.ts#loadPreset`, `web/src/store/uiStore.ts#persist`, `web/src/store/uiStore.ts#useEditorUiStore`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/13-implementation.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/store/uiStore.persistence.test.ts -t "schema_safe_layout_panel_and_keyframe_state_survive_restart"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 7: HS-menu-contract (implementation-slice-31072734c8bdcb47)

**Covered records:**
- `requirement-28dc2449c81b0010` (requirement)

**Files:**
- Modify: `web/src/components/shell/ViewMenu.tsx#ViewMenu`
- Modify: `web/src/store/uiStore.ts#useEditorUiStore`
- Modify: `docs/specs/frontend/2-layout.md`
- Test (reviewed-planned): `web/src/components/shell/ViewMenu.test.tsx#commands_shortcuts_checked_state_and_disabled_rules`

**Candidate-bound contracts:**

#### requirement-28dc2449c81b0010

- Candidate/source: `doc-75ea54d0d363d8ae` at `docs/specs/frontend/2-layout.md:94` (requirement)
- Expected behavior: The application/main menu exposes the specified commands, state, shortcuts, and disabled rules.
- Resolution: `reviewed-mapping-report:HS-menu-contract` — Core mapping report HS-menu-contract: the complete command, shortcut, checked-state and disabled-rule matrix belongs to ViewMenu.
- Exact acceptance contract:
  - Implement every specified main-menu entry through shared actions.
  - Match shortcut, enabled, checked, and focus behavior.
  - Add menu action and packaged desktop tests.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/ViewMenu.test.tsx#commands_shortcuts_checked_state_and_disabled_rules` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.test.tsx -t "commands_shortcuts_checked_state_and_disabled_rules"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/ViewMenu.tsx#ViewMenu`, `web/src/store/uiStore.ts#useEditorUiStore`, `docs/specs/frontend/2-layout.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.test.tsx -t "commands_shortcuts_checked_state_and_disabled_rules"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 8: HS-component-mapping-composite (implementation-slice-b7a46a3fba87c355)

**Covered records:**
- `requirement-047e16af5d02f827` (requirement)

**Files:**
- Modify: `web/src/components/home/HomeView.tsx#HomeView`
- Modify: `web/src/components/shell/EditorSplit.tsx#EditorSplit`
- Modify: `web/src/components/shell/ViewMenu.tsx#ViewMenu`
- Modify: `docs/specs/frontend/3-components.md`
- Test (reviewed-planned): `web/src/components/shell/ShellComponentMapping.test.tsx#every_documented_shell_component_has_exact_owner`

**Candidate-bound contracts:**

#### requirement-047e16af5d02f827

- Candidate/source: `doc-82644e70920f2984` at `docs/specs/frontend/3-components.md:23` (requirement)
- Expected behavior: Every component in the mapping exists with the specified layout and interactions.
- Resolution: `reviewed-mapping-report:HS-component-mapping-composite` — Core mapping report HS-component-mapping-composite: every-component mapping is a shell acceptance index, not an independent feature.
- Exact acceptance contract:
  - Implement remaining AI Edit, Music, transition, and advanced overlay components.
  - Wire each component to live commands/state.
  - Add component-map and visual acceptance coverage.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/ShellComponentMapping.test.tsx#every_documented_shell_component_has_exact_owner` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/shell/ShellComponentMapping.test.tsx -t "every_documented_shell_component_has_exact_owner"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/home/HomeView.tsx#HomeView`, `web/src/components/shell/EditorSplit.tsx#EditorSplit`, `web/src/components/shell/ViewMenu.tsx#ViewMenu`, `docs/specs/frontend/3-components.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/ShellComponentMapping.test.tsx -t "every_documented_shell_component_has_exact_owner"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. Added the executable component-owner index and completed the missing AI Edit, Music, and cross-dissolve transition vertical slices through shared project commands/state. The complete Web suite passed 82 files / 773 tests, the production Web build passed with only the pre-existing chunk/dynamic-import warnings, the complete Rust workspace passed, and formatting plus diff checks passed. The rebuilt macOS application verified linked A/V selection, AI proposal/reject/apply/undo, project and library music placement, paid-generation confirmation handoff, transition apply/remove/undo, save/reopen persistence, and a parseable 720p H.264/AAC export. Runtime evidence: [`home-component-mapping-real-device-2026-07-31.md`](../runtime-artifacts/automated/home-component-mapping-real-device-2026-07-31.md).

### Task 9: control-acceptance (implementation-slice-60d775675af9091c)

**Covered records:**
- `control-record-07769c794a939f08` (control)
- `control-record-975396ce576cbe5a` (control)
- `control-record-30c4fcd3fcca51ed` (control)

**Files:**
- Modify: `web/src/components/home/HomeView.tsx`
- Modify: `web/src/store/projectActions.ts#newProjectAndEnter`
- Modify: `web/src/lib/api.ts#getDefaultProjectDir`
- Modify: `src-tauri/src/commands.rs#get_default_project_dir`
- Modify: `web/src/lib/api.ts#projectNew`
- Modify: `src-tauri/src/commands.rs#project_new`
- Modify: `crates/opentake-core/src/dto.rs#handle_project_new`
- Modify: `crates/opentake-core/src/core.rs`
- Modify: `web/src/components/home/HomeView.tsx#HomeView`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-e2d0f1ed3415ea45 create a new project from the Home sidebar`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-575978f9bced5959 create a new project from the empty launcher`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-74f414d717baaed3 create a new project from the populated launcher`

**Candidate-bound contracts:**

#### control-record-07769c794a939f08

- Candidate/source: `control-e2d0f1ed3415ea45` at `web/src/components/home/HomeView.tsx:102:9` (control)
- Expected behavior: create a new project from the Home sidebar: newProjectAndEnter creates/saves a project and switches to editor
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-e2d0f1ed3415ea45.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-e2d0f1ed3415ea45 create a new project from the Home sidebar.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => void newProjectAndEnter()}","click or native keyboard activation plus current owning state"]; handler={() => void newProjectAndEnter()}.
  - Exact call/state/backend: stateTransition=newProjectAndEnter creates/saves a project and switches to editor; backendTrace=["web/src/components/home/HomeView.tsx:102::candidate handler -> {() => void newProjectAndEnter()}","actual branch/state -> newProjectAndEnter creates/saves a project and switches to editor","exact call/arguments -> newProjectAndEnter(); after saveDialog, either projectNew() in browser/no-dialog mode or getDefaultProjectDir(), native save(), projectNew(), then projectSave(chosen .opentake path)","web/src/store/projectActions.ts::newProjectAndEnter -> saveDialog; stopNativePlaybackForProjectBoundary; api.projectNew(); optional api.projectSave(path); replace snapshot; reset media/runtime; setView('editor')","web/src/lib/api.ts::getDefaultProjectDir -> invoke('get_default_project_dir') and src-tauri/src/commands.rs::get_default_project_dir when a native save dialog is available","web/src/lib/api.ts::projectNew/projectSave -> invoke('project_new') and invoke('project_save',{path})","src-tauri/src/commands.rs::project_new/project_save -> crates/opentake-core/src/dto.rs::handle_project_new and crates/opentake-core/src/core.rs project save lifecycle","code:web/src/components/home/HomeView.tsx#HomeView","code:web/src/store/projectActions.ts#newProjectAndEnter","code:web/src/lib/api.ts#getDefaultProjectDir","code:src-tauri/src/commands.rs#get_default_project_dir","code:web/src/lib/api.ts#projectNew","code:src-tauri/src/commands.rs#project_new","code:crates/opentake-core/src/dto.rs#handle_project_new"].
  - Visible/accessibility/return path: success=create a new project from the Home sidebar: newProjectAndEnter creates/saves a project and switches to editor; accessibility={"focus":"Custom SidebarRow focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"create a new project from the Home sidebar: newProjectAndEnter creates/saves a project and switches to editor","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/home/HomeView.tsx:102; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/home/HomeView.tsx:102; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in newProjectAndEnter creates/saves a project and switches to editor.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/home/HomeView.tsx:102; the missing DOM test must prove whether it is surfaced or silent."}.

#### control-record-975396ce576cbe5a

- Candidate/source: `control-575978f9bced5959` at `web/src/components/home/HomeView.tsx:234:11` (control)
- Expected behavior: create a new project from the empty launcher: newProjectAndEnter enters a fresh editor project
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-575978f9bced5959.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-575978f9bced5959 create a new project from the empty launcher.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => void newProjectAndEnter()}","click or native keyboard activation plus current owning state"]; handler={() => void newProjectAndEnter()}.
  - Exact call/state/backend: stateTransition=newProjectAndEnter enters a fresh editor project; backendTrace=["web/src/components/home/HomeView.tsx:234::candidate handler -> {() => void newProjectAndEnter()}","actual branch/state -> newProjectAndEnter enters a fresh editor project","exact call/arguments -> newProjectAndEnter(); after saveDialog, either projectNew() in browser/no-dialog mode or getDefaultProjectDir(), native save(), projectNew(), then projectSave(chosen .opentake path)","web/src/store/projectActions.ts::newProjectAndEnter -> saveDialog; stopNativePlaybackForProjectBoundary; api.projectNew(); optional api.projectSave(path); replace snapshot; reset media/runtime; setView('editor')","web/src/lib/api.ts::getDefaultProjectDir -> invoke('get_default_project_dir') and src-tauri/src/commands.rs::get_default_project_dir when a native save dialog is available","web/src/lib/api.ts::projectNew/projectSave -> invoke('project_new') and invoke('project_save',{path})","src-tauri/src/commands.rs::project_new/project_save -> crates/opentake-core/src/dto.rs::handle_project_new and crates/opentake-core/src/core.rs project save lifecycle","code:web/src/components/home/HomeView.tsx#HomeView","code:web/src/store/projectActions.ts#newProjectAndEnter","code:web/src/lib/api.ts#getDefaultProjectDir","code:src-tauri/src/commands.rs#get_default_project_dir","code:web/src/lib/api.ts#projectNew","code:src-tauri/src/commands.rs#project_new","code:crates/opentake-core/src/dto.rs#handle_project_new"].
  - Visible/accessibility/return path: success=create a new project from the empty launcher: newProjectAndEnter enters a fresh editor project; accessibility={"focus":"Custom LauncherButton focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"create a new project from the empty launcher: newProjectAndEnter enters a fresh editor project","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/home/HomeView.tsx:234; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/home/HomeView.tsx:234; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in newProjectAndEnter enters a fresh editor project.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/home/HomeView.tsx:234; the missing DOM test must prove whether it is surfaced or silent."}.

#### control-record-30c4fcd3fcca51ed

- Candidate/source: `control-74f414d717baaed3` at `web/src/components/home/HomeView.tsx:421:9` (control)
- Expected behavior: create a new project from the populated launcher: newProjectAndEnter enters a fresh editor project
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-74f414d717baaed3.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-74f414d717baaed3 create a new project from the populated launcher.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => void newProjectAndEnter()}","click or native keyboard activation plus current owning state"]; handler={() => void newProjectAndEnter()}.
  - Exact call/state/backend: stateTransition=newProjectAndEnter enters a fresh editor project; backendTrace=["web/src/components/home/HomeView.tsx:421::candidate handler -> {() => void newProjectAndEnter()}","actual branch/state -> newProjectAndEnter enters a fresh editor project","exact call/arguments -> newProjectAndEnter(); after saveDialog, either projectNew() in browser/no-dialog mode or getDefaultProjectDir(), native save(), projectNew(), then projectSave(chosen .opentake path)","web/src/store/projectActions.ts::newProjectAndEnter -> saveDialog; stopNativePlaybackForProjectBoundary; api.projectNew(); optional api.projectSave(path); replace snapshot; reset media/runtime; setView('editor')","web/src/lib/api.ts::getDefaultProjectDir -> invoke('get_default_project_dir') and src-tauri/src/commands.rs::get_default_project_dir when a native save dialog is available","web/src/lib/api.ts::projectNew/projectSave -> invoke('project_new') and invoke('project_save',{path})","src-tauri/src/commands.rs::project_new/project_save -> crates/opentake-core/src/dto.rs::handle_project_new and crates/opentake-core/src/core.rs project save lifecycle","code:web/src/components/home/HomeView.tsx#HomeView","code:web/src/store/projectActions.ts#newProjectAndEnter","code:web/src/lib/api.ts#getDefaultProjectDir","code:src-tauri/src/commands.rs#get_default_project_dir","code:web/src/lib/api.ts#projectNew","code:src-tauri/src/commands.rs#project_new","code:crates/opentake-core/src/dto.rs#handle_project_new"].
  - Visible/accessibility/return path: success=create a new project from the populated launcher: newProjectAndEnter enters a fresh editor project; accessibility={"focus":"Custom LauncherButton focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"create a new project from the populated launcher: newProjectAndEnter enters a fresh editor project","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/home/HomeView.tsx:421; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/home/HomeView.tsx:421; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in newProjectAndEnter enters a fresh editor project.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/home/HomeView.tsx:421; the missing DOM test must prove whether it is surfaced or silent."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/home/HomeView.interaction.test.tsx#control-e2d0f1ed3415ea45 create a new project from the Home sidebar` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/home/HomeView.interaction.test.tsx#control-575978f9bced5959 create a new project from the empty launcher` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/home/HomeView.interaction.test.tsx#control-74f414d717baaed3 create a new project from the populated launcher` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-e2d0f1ed3415ea45 create a new project from the Home sidebar"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-575978f9bced5959 create a new project from the empty launcher"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-74f414d717baaed3 create a new project from the populated launcher"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/home/HomeView.tsx`, `web/src/store/projectActions.ts#newProjectAndEnter`, `web/src/lib/api.ts#getDefaultProjectDir`, `src-tauri/src/commands.rs#get_default_project_dir`, `web/src/lib/api.ts#projectNew`, `src-tauri/src/commands.rs#project_new`, `crates/opentake-core/src/dto.rs#handle_project_new`, `crates/opentake-core/src/core.rs`, `web/src/components/home/HomeView.tsx#HomeView` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-e2d0f1ed3415ea45 create a new project from the Home sidebar"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-575978f9bced5959 create a new project from the empty launcher"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-74f414d717baaed3 create a new project from the populated launcher"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 10: control-acceptance (implementation-slice-526c91ba76edbda6)

**Covered records:**
- `control-record-7ecf0e540513dd48` (control)
- `control-record-4c4c5a8e3a90f2aa` (control)
- `control-record-843206830fdd31a4` (control)

**Files:**
- Modify: `web/src/components/home/HomeView.tsx`
- Modify: `web/src/store/projectActions.ts#openProjectViaDialog`
- Modify: `web/src/store/projectActions.ts#openProjectPath`
- Modify: `web/src/lib/api.ts#projectOpen`
- Modify: `src-tauri/src/commands.rs#project_open`
- Modify: `crates/opentake-core/src/core.rs#AppCore::prepare_project_open`
- Modify: `web/src/components/home/HomeView.tsx#HomeView`
- Modify: `crates/opentake-core/src/core.rs#AppCore`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-ef78873f98fcab84 open a project from the Home sidebar`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-2121d7b9fdc279b9 open a project from the empty launcher`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-ab109c708bb0efbf open a project from the populated launcher`

**Candidate-bound contracts:**

#### control-record-7ecf0e540513dd48

- Candidate/source: `control-ef78873f98fcab84` at `web/src/components/home/HomeView.tsx:103:9` (control)
- Expected behavior: open a project from the Home sidebar: sets opening while native dialog/openProjectPath runs
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-ef78873f98fcab84.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-ef78873f98fcab84 open a project from the Home sidebar.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => void handleOpen()}","click or native keyboard activation plus current owning state"]; handler={() => void handleOpen()}.
  - Exact call/state/backend: stateTransition=sets opening while native dialog/openProjectPath runs; backendTrace=["web/src/components/home/HomeView.tsx:103::candidate handler -> {() => void handleOpen()}","actual branch/state -> sets opening while native dialog/openProjectPath runs","exact call/arguments -> openProjectViaDialog(); open({directory:true,multiple:false}); when selected is a string call openProjectPath(selected)","web/src/store/projectActions.ts::openProjectViaDialog -> openDialog/open({directory:true,multiple:false}) -> openProjectPath(selected)","web/src/store/projectActions.ts::openProjectPath -> api.projectOpen(path), replace snapshot, refreshMedia(), reset runtime, setView('editor')","web/src/lib/api.ts::projectOpen -> invoke('project_open',{path})","src-tauri/src/commands.rs::project_open(path) -> crates/opentake-core/src/core.rs::AppCore::prepare_project_open/commit_project_open","code:web/src/components/home/HomeView.tsx#HomeView","code:web/src/store/projectActions.ts#openProjectViaDialog","code:web/src/store/projectActions.ts#openProjectPath","code:web/src/lib/api.ts#projectOpen","code:src-tauri/src/commands.rs#project_open","code:crates/opentake-core/src/core.rs#AppCore"].
  - Visible/accessibility/return path: success=open a project from the Home sidebar: sets opening while native dialog/openProjectPath runs; accessibility={"focus":"Custom SidebarRow focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"open a project from the Home sidebar: sets opening while native dialog/openProjectPath runs","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/home/HomeView.tsx:103; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/home/HomeView.tsx:103; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in sets opening while native dialog/openProjectPath runs; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in sets opening while native dialog/openProjectPath runs.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/home/HomeView.tsx:103; the missing DOM test must prove whether it is surfaced or silent."}.

#### control-record-4c4c5a8e3a90f2aa

- Candidate/source: `control-2121d7b9fdc279b9` at `web/src/components/home/HomeView.tsx:235:11` (control)
- Expected behavior: open a project from the empty launcher: sets opening while openProjectViaDialog runs
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-2121d7b9fdc279b9.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-2121d7b9fdc279b9 open a project from the empty launcher.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => void handleOpen()}","click or native keyboard activation plus current owning state"]; handler={() => void handleOpen()}.
  - Exact call/state/backend: stateTransition=sets opening while openProjectViaDialog runs; backendTrace=["web/src/components/home/HomeView.tsx:235::candidate handler -> {() => void handleOpen()}","actual branch/state -> sets opening while openProjectViaDialog runs","exact call/arguments -> openProjectViaDialog(); open({directory:true,multiple:false}); when selected is a string call openProjectPath(selected)","web/src/store/projectActions.ts::openProjectViaDialog -> openDialog/open({directory:true,multiple:false}) -> openProjectPath(selected)","web/src/store/projectActions.ts::openProjectPath -> api.projectOpen(path), replace snapshot, refreshMedia(), reset runtime, setView('editor')","web/src/lib/api.ts::projectOpen -> invoke('project_open',{path})","src-tauri/src/commands.rs::project_open(path) -> crates/opentake-core/src/core.rs::AppCore::prepare_project_open/commit_project_open","code:web/src/components/home/HomeView.tsx#HomeView","code:web/src/store/projectActions.ts#openProjectViaDialog","code:web/src/store/projectActions.ts#openProjectPath","code:web/src/lib/api.ts#projectOpen","code:src-tauri/src/commands.rs#project_open","code:crates/opentake-core/src/core.rs#AppCore"].
  - Visible/accessibility/return path: success=open a project from the empty launcher: sets opening while openProjectViaDialog runs; accessibility={"focus":"Custom LauncherButton focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"open a project from the empty launcher: sets opening while openProjectViaDialog runs","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/home/HomeView.tsx:235; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/home/HomeView.tsx:235; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in sets opening while openProjectViaDialog runs; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in sets opening while openProjectViaDialog runs.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/home/HomeView.tsx:235; the missing DOM test must prove whether it is surfaced or silent."}.

#### control-record-843206830fdd31a4

- Candidate/source: `control-ab109c708bb0efbf` at `web/src/components/home/HomeView.tsx:422:9` (control)
- Expected behavior: open a project from the populated launcher: sets opening while openProjectViaDialog runs
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-ab109c708bb0efbf.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-ab109c708bb0efbf open a project from the populated launcher.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onOpen}","click or native keyboard activation plus current owning state"]; handler={onOpen}.
  - Exact call/state/backend: stateTransition=sets opening while openProjectViaDialog runs; backendTrace=["web/src/components/home/HomeView.tsx:422::candidate handler -> {onOpen}","actual branch/state -> sets opening while openProjectViaDialog runs","exact call/arguments -> openProjectViaDialog(); open({directory:true,multiple:false}); when selected is a string call openProjectPath(selected)","web/src/store/projectActions.ts::openProjectViaDialog -> openDialog/open({directory:true,multiple:false}) -> openProjectPath(selected)","web/src/store/projectActions.ts::openProjectPath -> api.projectOpen(path), replace snapshot, refreshMedia(), reset runtime, setView('editor')","web/src/lib/api.ts::projectOpen -> invoke('project_open',{path})","src-tauri/src/commands.rs::project_open(path) -> crates/opentake-core/src/core.rs::AppCore::prepare_project_open/commit_project_open","code:web/src/components/home/HomeView.tsx#HomeView","code:web/src/store/projectActions.ts#openProjectViaDialog","code:web/src/store/projectActions.ts#openProjectPath","code:web/src/lib/api.ts#projectOpen","code:src-tauri/src/commands.rs#project_open","code:crates/opentake-core/src/core.rs#AppCore"].
  - Visible/accessibility/return path: success=open a project from the populated launcher: sets opening while openProjectViaDialog runs; accessibility={"focus":"Custom LauncherButton focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"open a project from the populated launcher: sets opening while openProjectViaDialog runs","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/home/HomeView.tsx:422; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/home/HomeView.tsx:422; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in sets opening while openProjectViaDialog runs; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in sets opening while openProjectViaDialog runs.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/home/HomeView.tsx:422; the missing DOM test must prove whether it is surfaced or silent."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/home/HomeView.interaction.test.tsx#control-ef78873f98fcab84 open a project from the Home sidebar` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/home/HomeView.interaction.test.tsx#control-2121d7b9fdc279b9 open a project from the empty launcher` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/home/HomeView.interaction.test.tsx#control-ab109c708bb0efbf open a project from the populated launcher` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-ef78873f98fcab84 open a project from the Home sidebar"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-2121d7b9fdc279b9 open a project from the empty launcher"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-ab109c708bb0efbf open a project from the populated launcher"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/home/HomeView.tsx`, `web/src/store/projectActions.ts#openProjectViaDialog`, `web/src/store/projectActions.ts#openProjectPath`, `web/src/lib/api.ts#projectOpen`, `src-tauri/src/commands.rs#project_open`, `crates/opentake-core/src/core.rs#AppCore::prepare_project_open`, `web/src/components/home/HomeView.tsx#HomeView`, `crates/opentake-core/src/core.rs#AppCore` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-ef78873f98fcab84 open a project from the Home sidebar"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-2121d7b9fdc279b9 open a project from the empty launcher"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-ab109c708bb0efbf open a project from the populated launcher"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 11: control-acceptance (implementation-slice-94554777e3aef548)

**Covered records:**
- `control-record-ebcd32e273d5d7b2` (control)
- `control-record-0b6d3a35f547a01a` (control)
- `control-record-9277cd38a9de8029` (control)
- `control-record-54996897683da264` (control)

**Files:**
- Modify: `web/src/components/home/HomeView.tsx`
- Modify: `web/src/components/home/HomeView.tsx#HomeView`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-f4a6b4f8789ea013 open the global Library from Home`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-810a7d793fcd8323 open Settings from Home`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-acd6238c08e790cc clear recent-project card selection`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-ec1cd7a2d49bb97a remove a recent project entry`

**Candidate-bound contracts:**

#### control-record-ebcd32e273d5d7b2

- Candidate/source: `control-f4a6b4f8789ea013` at `web/src/components/home/HomeView.tsx:111:7` (control)
- Expected behavior: open the global Library from Home: setView('library')
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-f4a6b4f8789ea013.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-f4a6b4f8789ea013 open the global Library from Home.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setView(\"library\")}","click or native keyboard activation plus current owning state"]; handler={() => setView("library")}.
  - Exact call/state/backend: stateTransition=setView('library'); backendTrace=["web/src/components/home/HomeView.tsx:111::candidate handler -> {() => setView(\"library\")}","actual branch/state -> setView('library')","exact call -> setView('library')","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/home/HomeView.tsx#HomeView"].
  - Visible/accessibility/return path: success=open the global Library from Home: setView('library'); accessibility={"focus":"Custom SidebarRow focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"open the global Library from Home: setView('library')","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/home/HomeView.tsx:111; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setView('library'); no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-0b6d3a35f547a01a

- Candidate/source: `control-810a7d793fcd8323` at `web/src/components/home/HomeView.tsx:115:7` (control)
- Expected behavior: open Settings from Home: setSettingsOpen(true)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-810a7d793fcd8323.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-810a7d793fcd8323 open Settings from Home.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setSettingsOpen(true)}","click or native keyboard activation plus current owning state"]; handler={() => setSettingsOpen(true)}.
  - Exact call/state/backend: stateTransition=setSettingsOpen(true); backendTrace=["web/src/components/home/HomeView.tsx:115::candidate handler -> {() => setSettingsOpen(true)}","actual branch/state -> setSettingsOpen(true)","exact call -> setSettingsOpen(true)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/home/HomeView.tsx#HomeView"].
  - Visible/accessibility/return path: success=open Settings from Home: setSettingsOpen(true); accessibility={"focus":"Custom SidebarRow focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"open Settings from Home: setSettingsOpen(true)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setSettingsOpen(true); no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-9277cd38a9de8029

- Candidate/source: `control-acd6238c08e790cc` at `web/src/components/home/HomeView.tsx:312:5` (control)
- Expected behavior: clear recent-project card selection: setSelectedPath(null)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-acd6238c08e790cc.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-acd6238c08e790cc clear recent-project card selection.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setSelectedPath(null)}","pointer/drag coordinates, button/key, and current owning state"]; handler={() => setSelectedPath(null)}.
  - Exact call/state/backend: stateTransition=setSelectedPath(null); backendTrace=["web/src/components/home/HomeView.tsx:312::candidate handler -> {() => setSelectedPath(null)}","actual branch/state -> setSelectedPath(null)","exact call -> setSelectedPath(null)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/home/HomeView.tsx#HomeView"].
  - Visible/accessibility/return path: success=clear recent-project card selection: setSelectedPath(null); accessibility={"focus":"Non-focusable div; pointer-only clearing surface","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"clear recent-project card selection: setSelectedPath(null)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/home/HomeView.tsx:312; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-54996897683da264

- Candidate/source: `control-ec1cd7a2d49bb97a` at `web/src/components/home/HomeView.tsx:521:9` (control)
- Expected behavior: remove a recent project entry: recentStore.remove(entry.path)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-ec1cd7a2d49bb97a.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-ec1cd7a2d49bb97a remove a recent project entry.
  - Initial state: visibility=Only while the card is hovered; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => { e.stopPropagation(); remove(entry.path); }}","click or native keyboard activation plus current owning state"]; handler={(e) => { e.stopPropagation(); remove(entry.path); }}.
  - Exact call/state/backend: stateTransition=recentStore.remove(entry.path); backendTrace=["web/src/components/home/HomeView.tsx:521::candidate handler -> {(e) => { e.stopPropagation(); remove(entry.path); }}","actual branch/state -> recentStore.remove(entry.path)","exact call -> recentStore.remove(entry.path)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/home/HomeView.tsx#HomeView"].
  - Visible/accessibility/return path: success=remove a recent project entry: recentStore.remove(entry.path); accessibility={"focus":"Native keyboard-focusable control","label":"t(\"home.remove\")","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"remove a recent project entry: recentStore.remove(entry.path)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/home/HomeView.tsx:521; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/home/HomeView.interaction.test.tsx#control-f4a6b4f8789ea013 open the global Library from Home` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/home/HomeView.interaction.test.tsx#control-810a7d793fcd8323 open Settings from Home` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/home/HomeView.interaction.test.tsx#control-acd6238c08e790cc clear recent-project card selection` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/home/HomeView.interaction.test.tsx#control-ec1cd7a2d49bb97a remove a recent project entry` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-f4a6b4f8789ea013 open the global Library from Home"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-810a7d793fcd8323 open Settings from Home"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-acd6238c08e790cc clear recent-project card selection"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-ec1cd7a2d49bb97a remove a recent project entry"`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

  Result: The file existed only because Task 12 had just introduced it, but all four Task 11 names were absent; the focused filter executed zero candidates and reported the file/test as skipped. This is the missing-owning-evidence baseline rather than a manufactured production failure.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/home/HomeView.tsx`, `web/src/components/home/HomeView.tsx#HomeView` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Result: Existing Library, Settings, background-clear, and recent-store removal handlers already satisfied the four contracts. Added the four exact owning tests; no further product code change was required after Task 12 made the card and removal control natively accessible.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-f4a6b4f8789ea013 open the global Library from Home"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-810a7d793fcd8323 open Settings from Home"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-acd6238c08e790cc clear recent-project card selection"`
  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-ec1cd7a2d49bb97a remove a recent project entry"`

  Expected: PASS with every candidate-bound assertion executed.

  Result: PASS, all four exact candidate tests executed in the 5/5 Home interaction runner.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS, 72 Web files / 731 tests and production build. The packaged macOS app opened Library and Settings, selected then cleared a project card, removed only the recent entry while preserving the bundle, and reopened the bundle to restore the 2-entry Home state. Runtime evidence: `runtime-artifacts/automated/home-secondary-controls-real-device-2026-07-30.md`.

### Task 12: control-acceptance (implementation-slice-8e481b772d7d2357)

**Covered records:**
- `control-record-5efcf3f943830784` (control)

**Files:**
- Modify: `web/src/components/home/HomeView.tsx`
- Modify: `web/src/components/home/HomeView.tsx#ProjectLauncher`
- Modify: `web/src/store/projectActions.ts#openProjectPath`
- Modify: `web/src/lib/api.ts#projectOpen`
- Modify: `src-tauri/src/commands.rs#project_open`
- Modify: `crates/opentake-core/src/core.rs#AppCore::prepare_project_open`
- Modify: `web/src/components/home/HomeView.tsx#HomeView`
- Modify: `crates/opentake-core/src/core.rs#AppCore`
- Test (reviewed-planned): `web/src/components/home/HomeView.interaction.test.tsx#control-9697b53d4d2cf1ca select or open a recent project card`

**Candidate-bound contracts:**

#### control-record-5efcf3f943830784

- Candidate/source: `control-9697b53d4d2cf1ca` at `web/src/components/home/HomeView.tsx:351:11` (control)
- Expected behavior: select or open a recent project card: onClick stops propagation and setSelectedPath(entry.path); onDoubleClick calls openProjectPath(entry.path); the launcher window Enter branch opens selectedPath
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-9697b53d4d2cf1ca.
  - Test: web/src/components/home/HomeView.interaction.test.tsx#control-9697b53d4d2cf1ca select or open a recent project card.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => { e.stopPropagation(); setSelectedPath(entry.path); }} {() => void openProjectPath(entry.path)}","click or native keyboard activation plus current owning state"]; handler={(e) => { e.stopPropagation(); setSelectedPath(entry.path); }} {() => void openProjectPath(entry.path)}.
  - Exact call/state/backend: stateTransition=onClick stops propagation and setSelectedPath(entry.path); onDoubleClick calls openProjectPath(entry.path); the launcher window Enter branch opens selectedPath; backendTrace=["web/src/components/home/HomeView.tsx:351::candidate handler -> {(e) => { e.stopPropagation(); setSelectedPath(entry.path); }} {() => void openProjectPath(entry.path)}","actual branch/state -> onClick stops propagation and setSelectedPath(entry.path); onDoubleClick calls openProjectPath(entry.path); the launcher window Enter branch opens selectedPath","exact call/arguments -> onDoubleClick calls openProjectPath(entry.path); single click only setSelectedPath(entry.path); window Enter calls openProjectPath(selectedPath) when selectedPath is a project path","web/src/components/home/HomeView.tsx::ProjectLauncher/ProjectGridCard concrete entry.path branches","web/src/store/projectActions.ts::openProjectPath(path) -> api.projectOpen(path), refreshMedia(), setView('editor')","web/src/lib/api.ts::projectOpen -> invoke('project_open',{path})","src-tauri/src/commands.rs::project_open(path) -> crates/opentake-core/src/core.rs::AppCore::prepare_project_open/commit_project_open","code:web/src/components/home/HomeView.tsx#HomeView","code:web/src/components/home/HomeView.tsx#ProjectLauncher","code:web/src/store/projectActions.ts#openProjectPath","code:web/src/lib/api.ts#projectOpen","code:src-tauri/src/commands.rs#project_open","code:crates/opentake-core/src/core.rs#AppCore"].
  - Visible/accessibility/return path: success=select or open a recent project card: onClick stops propagation and setSelectedPath(entry.path); onDoubleClick calls openProjectPath(entry.path); the launcher window Enter branch opens selectedPath; accessibility={"focus":"ProjectGridCard call site supplies pointer handlers; the implementation div is not keyboard focusable even though a window-level Enter handler exists","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Project actions enter the editor; Library/Settings provide explicit Home/close actions."].
  - Outcome matrix: {"success":"select or open a recent project card: onClick stops propagation and setSelectedPath(entry.path); onDoubleClick calls openProjectPath(entry.path); the launcher window Enter branch opens selectedPath","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/home/HomeView.tsx:351; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/home/HomeView.tsx:351; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in onClick stops propagation and setSelectedPath(entry.path); onDoubleClick calls openProjectPath(entry.path); the launcher window Enter branch opens selectedPath; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in onClick stops propagation and setSelectedPath(entry.path); onDoubleClick calls openProjectPath(entry.path); the launcher window Enter branch opens selectedPath.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/home/HomeView.tsx:351; the missing DOM test must prove whether it is surfaced or silent."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/home/HomeView.interaction.test.tsx#control-9697b53d4d2cf1ca select or open a recent project card` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-9697b53d4d2cf1ca select or open a recent project card"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

  Result: RED. The declared owning runner did not exist, and the direct focused Vitest command exited 1 with `No test files found`. The first packaged-app inspection also exposed the existing card as text rather than a keyboard-focusable action.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/home/HomeView.tsx`, `web/src/components/home/HomeView.tsx#ProjectLauncher`, `web/src/store/projectActions.ts#openProjectPath`, `web/src/lib/api.ts#projectOpen`, `src-tauri/src/commands.rs#project_open`, `crates/opentake-core/src/core.rs#AppCore::prepare_project_open`, `web/src/components/home/HomeView.tsx#HomeView`, `crates/opentake-core/src/core.rs#AppCore` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Result: Replaced the pointer-only card surface with a real focusable button carrying the project name and `aria-pressed` selection state. Kept the remove action as a separate sibling button, while preserving single-click selection, double-click open, focus selection, and the launcher-level Enter/Return open path. The global Return handler now ignores other focused interactive controls so it cannot open a stale selected project while activating another Home action.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/home/HomeView.interaction.test.tsx -t "control-9697b53d4d2cf1ca select or open a recent project card"`

  Expected: PASS with every candidate-bound assertion executed.

  Result: PASS, 1/1 exact candidate test. The nearby Home visual suite also passed 11/11.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. The complete Web suite passed 72 files / 727 tests, the production Web build passed with only the pre-existing bundle warnings, and the complete Rust workspace passed. The rebuilt packaged macOS app verified single-click selection, native keyboard focus plus Return, and native double-click opening the same project. Runtime evidence: `runtime-artifacts/automated/recent-project-card-real-device-2026-07-30.md`.

### Task 13: control-acceptance (implementation-slice-1f94f01d3b65a701)

**Covered records:**
- `control-record-dabd5238342f6df5` (control)
- `control-record-b3a679124a899688` (control)
- `control-record-5642d66795afc9d9` (control)
- `control-record-f628e22a4f21d719` (control)
- `control-record-b64db00c4bc1fd1d` (control)
- `control-record-435788a98760dfa5` (control)
- `control-record-eafd9398c6240bc0` (control)

**Files:**
- Modify: `web/src/components/shell/TitleBar.tsx`
- Modify: `web/src/components/shell/TitleBar.tsx#TitleBar`
- Test (reviewed-planned): `web/src/components/shell/TitleBar.interaction.test.tsx#control-f52cc89817361a19 return from editor to Home`
- Test (reviewed-planned): `web/src/components/shell/TitleBar.interaction.test.tsx#control-4bda8f075e1f3a14 open the global Library`
- Test (reviewed-planned): `web/src/components/shell/TitleBar.interaction.test.tsx#control-ff132f94a8c87906 open Settings from the editor`
- Test (reviewed-planned): `web/src/components/shell/TitleBar.interaction.test.tsx#control-d7ba227c6447e43e open Video Export`
- Test (reviewed-planned): `web/src/components/shell/TitleBar.interaction.test.tsx#control-c035467e6746e570 open/close subtitle export formats`
- Test (reviewed-planned): `web/src/components/shell/TitleBar.interaction.test.tsx#control-229710d0115f07bc open/close interchange export menu`
- Test (reviewed-planned): `web/src/components/shell/TitleBar.interaction.test.tsx#control-02d1bf7fff7c1e3a open Video Export from the interchange menu`

**Candidate-bound contracts:**

#### control-record-dabd5238342f6df5

- Candidate/source: `control-f52cc89817361a19` at `web/src/components/shell/TitleBar.tsx:168:7` (control)
- Expected behavior: return from editor to Home: setView('home')
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-f52cc89817361a19.
  - Test: web/src/components/shell/TitleBar.interaction.test.tsx#control-f52cc89817361a19 return from editor to Home.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setView(\"home\")}","click or native keyboard activation plus current owning state"]; handler={() => setView("home")}.
  - Exact call/state/backend: stateTransition=setView('home'); backendTrace=["web/src/components/shell/TitleBar.tsx:168::candidate handler -> {() => setView(\"home\")}","actual branch/state -> setView('home')","exact call -> setView('home')","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/TitleBar.tsx#TitleBar"].
  - Visible/accessibility/return path: success=return from editor to Home: setView('home'); accessibility={"focus":"Native keyboard-focusable control","label":"t(\"title.backHome\")","shortcut":"None declared on this control"}; returnPath=["Navigation changes the full-screen view; popup actions should close and return focus to the title-bar trigger."].
  - Outcome matrix: {"success":"return from editor to Home: setView('home')","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-b3a679124a899688

- Candidate/source: `control-4bda8f075e1f3a14` at `web/src/components/shell/TitleBar.tsx:191:7` (control)
- Expected behavior: open the global Library: setView('library')
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-4bda8f075e1f3a14.
  - Test: web/src/components/shell/TitleBar.interaction.test.tsx#control-4bda8f075e1f3a14 open the global Library.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setView(\"library\")}","click or native keyboard activation plus current owning state"]; handler={() => setView("library")}.
  - Exact call/state/backend: stateTransition=setView('library'); backendTrace=["web/src/components/shell/TitleBar.tsx:191::candidate handler -> {() => setView(\"library\")}","actual branch/state -> setView('library')","exact call -> setView('library')","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/TitleBar.tsx#TitleBar"].
  - Visible/accessibility/return path: success=open the global Library: setView('library'); accessibility={"focus":"Native keyboard-focusable control","label":"t(\"library.entry\")","shortcut":"None declared on this control"}; returnPath=["Navigation changes the full-screen view; popup actions should close and return focus to the title-bar trigger."].
  - Outcome matrix: {"success":"open the global Library: setView('library')","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/TitleBar.tsx:191; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setView('library'); no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-5642d66795afc9d9

- Candidate/source: `control-ff132f94a8c87906` at `web/src/components/shell/TitleBar.tsx:207:7` (control)
- Expected behavior: open Settings from the editor: setSettingsOpen(true)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-ff132f94a8c87906.
  - Test: web/src/components/shell/TitleBar.interaction.test.tsx#control-ff132f94a8c87906 open Settings from the editor.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setSettingsOpen(true)}","click or native keyboard activation plus current owning state"]; handler={() => setSettingsOpen(true)}.
  - Exact call/state/backend: stateTransition=setSettingsOpen(true); backendTrace=["web/src/components/shell/TitleBar.tsx:207::candidate handler -> {() => setSettingsOpen(true)}","actual branch/state -> setSettingsOpen(true)","exact call -> setSettingsOpen(true)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/TitleBar.tsx#TitleBar"].
  - Visible/accessibility/return path: success=open Settings from the editor: setSettingsOpen(true); accessibility={"focus":"Native keyboard-focusable control","label":"t(\"title.settings\")","shortcut":"None declared on this control"}; returnPath=["Navigation changes the full-screen view; popup actions should close and return focus to the title-bar trigger."].
  - Outcome matrix: {"success":"open Settings from the editor: setSettingsOpen(true)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setSettingsOpen(true); no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-f628e22a4f21d719

- Candidate/source: `control-d7ba227c6447e43e` at `web/src/components/shell/TitleBar.tsx:223:7` (control)
- Expected behavior: open Video Export: setExportDialogOpen(true)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d7ba227c6447e43e.
  - Test: web/src/components/shell/TitleBar.interaction.test.tsx#control-d7ba227c6447e43e open Video Export.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({!hasClips}).
  - Event: inputs=["event/prop handler: {() => setExportDialogOpen(true)}","click or native keyboard activation plus current owning state"]; handler={() => setExportDialogOpen(true)}.
  - Exact call/state/backend: stateTransition=setExportDialogOpen(true); backendTrace=["web/src/components/shell/TitleBar.tsx:223::candidate handler -> {() => setExportDialogOpen(true)}","actual branch/state -> setExportDialogOpen(true)","exact call -> setExportDialogOpen(true)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/TitleBar.tsx#TitleBar"].
  - Visible/accessibility/return path: success=open Video Export: setExportDialogOpen(true); accessibility={"focus":"Native keyboard-focusable control","label":"t(\"title.exportVideo\")","shortcut":"None declared on this control"}; returnPath=["Navigation changes the full-screen view; popup actions should close and return focus to the title-bar trigger."].
  - Outcome matrix: {"success":"open Video Export: setExportDialogOpen(true)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/TitleBar.tsx:223; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {!hasClips}.","cancel":"Cancellation/dismissal follows the exact guard in setExportDialogOpen(true); no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-b64db00c4bc1fd1d

- Candidate/source: `control-c035467e6746e570` at `web/src/components/shell/TitleBar.tsx:248:9` (control)
- Expected behavior: open/close subtitle export formats: setSubMenuOpen toggles; outside/Escape closes
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c035467e6746e570.
  - Test: web/src/components/shell/TitleBar.interaction.test.tsx#control-c035467e6746e570 open/close subtitle export formats.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setSubMenuOpen((v) => !v)}","click or native keyboard activation plus current owning state"]; handler={() => setSubMenuOpen((v) => !v)}.
  - Exact call/state/backend: stateTransition=setSubMenuOpen toggles; outside/Escape closes; backendTrace=["web/src/components/shell/TitleBar.tsx:248::candidate handler -> {() => setSubMenuOpen((v) => !v)}","actual branch/state -> setSubMenuOpen toggles; outside/Escape closes","exact call -> setSubMenuOpen toggles; outside/Escape closes","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/TitleBar.tsx#TitleBar"].
  - Visible/accessibility/return path: success=open/close subtitle export formats: setSubMenuOpen toggles; outside/Escape closes; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"title.exportSubtitles\")","shortcut":"None declared on this control"}; returnPath=["Navigation changes the full-screen view; popup actions should close and return focus to the title-bar trigger."].
  - Outcome matrix: {"success":"open/close subtitle export formats: setSubMenuOpen toggles; outside/Escape closes","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/TitleBar.tsx:248; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setSubMenuOpen toggles; outside/Escape closes; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-435788a98760dfa5

- Candidate/source: `control-229710d0115f07bc` at `web/src/components/shell/TitleBar.tsx:317:9` (control)
- Expected behavior: open/close interchange export menu: setExportMenuOpen toggles; outside/Escape closes
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-229710d0115f07bc.
  - Test: web/src/components/shell/TitleBar.interaction.test.tsx#control-229710d0115f07bc open/close interchange export menu.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setExportMenuOpen((v) => !v)}","click or native keyboard activation plus current owning state"]; handler={() => setExportMenuOpen((v) => !v)}.
  - Exact call/state/backend: stateTransition=setExportMenuOpen toggles; outside/Escape closes; backendTrace=["web/src/components/shell/TitleBar.tsx:317::candidate handler -> {() => setExportMenuOpen((v) => !v)}","actual branch/state -> setExportMenuOpen toggles; outside/Escape closes","exact call -> setExportMenuOpen toggles; outside/Escape closes","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/TitleBar.tsx#TitleBar"].
  - Visible/accessibility/return path: success=open/close interchange export menu: setExportMenuOpen toggles; outside/Escape closes; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"title.export\")","shortcut":"None declared on this control"}; returnPath=["Navigation changes the full-screen view; popup actions should close and return focus to the title-bar trigger."].
  - Outcome matrix: {"success":"open/close interchange export menu: setExportMenuOpen toggles; outside/Escape closes","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/TitleBar.tsx:317; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setExportMenuOpen toggles; outside/Escape closes; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-eafd9398c6240bc0

- Candidate/source: `control-02d1bf7fff7c1e3a` at `web/src/components/shell/TitleBar.tsx:359:13` (control)
- Expected behavior: open Video Export from the interchange menu: close menu -> open Export dialog
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-02d1bf7fff7c1e3a.
  - Test: web/src/components/shell/TitleBar.interaction.test.tsx#control-02d1bf7fff7c1e3a open Video Export from the interchange menu.
  - Initial state: visibility=Only while interchange menu is open; enabledWhen=not ({!hasClips}).
  - Event: inputs=["event/prop handler: {() => { setExportMenuOpen(false); setExportDialogOpen(true); }}","click or native keyboard activation plus current owning state"]; handler={() => { setExportMenuOpen(false); setExportDialogOpen(true); }}.
  - Exact call/state/backend: stateTransition=close menu -> open Export dialog; backendTrace=["web/src/components/shell/TitleBar.tsx:359::candidate handler -> {() => { setExportMenuOpen(false); setExportDialogOpen(true); }}","actual branch/state -> close menu -> open Export dialog","exact call -> close menu -> open Export dialog","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/TitleBar.tsx#TitleBar"].
  - Visible/accessibility/return path: success=open Video Export from the interchange menu: close menu -> open Export dialog; accessibility={"focus":"Native keyboard-focusable control","label":"menuitem","shortcut":"None declared on this control"}; returnPath=["Navigation changes the full-screen view; popup actions should close and return focus to the title-bar trigger."].
  - Outcome matrix: {"success":"open Video Export from the interchange menu: close menu -> open Export dialog","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/TitleBar.tsx:359; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {!hasClips}.","cancel":"Cancellation/dismissal follows the exact guard in close menu -> open Export dialog; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/TitleBar.interaction.test.tsx#control-f52cc89817361a19 return from editor to Home` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/TitleBar.interaction.test.tsx#control-4bda8f075e1f3a14 open the global Library` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/TitleBar.interaction.test.tsx#control-ff132f94a8c87906 open Settings from the editor` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/TitleBar.interaction.test.tsx#control-d7ba227c6447e43e open Video Export` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/TitleBar.interaction.test.tsx#control-c035467e6746e570 open/close subtitle export formats` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/TitleBar.interaction.test.tsx#control-229710d0115f07bc open/close interchange export menu` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/TitleBar.interaction.test.tsx#control-02d1bf7fff7c1e3a open Video Export from the interchange menu` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify the baseline**

  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-f52cc89817361a19 return from editor to Home"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-4bda8f075e1f3a14 open the global Library"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-ff132f94a8c87906 open Settings from the editor"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-d7ba227c6447e43e open Video Export"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-c035467e6746e570 open/close subtitle export formats"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-229710d0115f07bc open/close interchange export menu"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-02d1bf7fff7c1e3a open Video Export from the interchange menu"`

  Observed: all seven production controls already existed. The exact owning tests were absent, so the new candidate-bound tests passed against the existing implementation without introducing an artificial production failure.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/TitleBar.tsx`, `web/src/components/shell/TitleBar.tsx#TitleBar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Observed: production code required no change. The owning test now covers all seven planned controls, including empty-timeline video-export disablement and both Escape and outside-click menu dismissal.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-f52cc89817361a19 return from editor to Home"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-4bda8f075e1f3a14 open the global Library"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-ff132f94a8c87906 open Settings from the editor"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-d7ba227c6447e43e open Video Export"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-c035467e6746e570 open/close subtitle export formats"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-229710d0115f07bc open/close interchange export menu"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-02d1bf7fff7c1e3a open Video Export from the interchange menu"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS, 70 files / 715 web tests. See `docs/audit/2026-07-14/runtime-artifacts/automated/titlebar-controls-real-device-2026-07-30.md` for the real application loop.

### Task 14: control-acceptance (implementation-slice-2e210a4b1ab6c5e9)

**Covered records:**
- `control-record-2d94ad2184172017` (control)

**Files:**
- Modify: `web/src/components/shell/TitleBar.tsx`
- Modify: `web/src/components/shell/TitleBar.tsx#onExportSubtitles`
- Modify: `web/src/lib/api.ts#getDefaultProjectDir`
- Modify: `src-tauri/src/commands.rs`
- Modify: `web/src/lib/api.ts#exportSubtitles`
- Modify: `src-tauri/src/commands.rs#export_subtitles`
- Modify: `crates/opentake-domain/src/subtitle_export.rs#export_srt`
- Modify: `web/src/components/shell/TitleBar.tsx#TitleBar`
- Test (reviewed-planned): `web/src/components/shell/TitleBar.interaction.test.tsx#control-f54f4037ab7bffbe export SRT or VTT subtitles`

**Candidate-bound contracts:**

#### control-record-2d94ad2184172017

- Candidate/source: `control-f54f4037ab7bffbe` at `web/src/components/shell/TitleBar.tsx:289:15` (control)
- Expected behavior: export SRT or VTT subtitles: close menu -> save dialog -> done/empty/failure toast
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-f54f4037ab7bffbe.
  - Test: web/src/components/shell/TitleBar.interaction.test.tsx#control-f54f4037ab7bffbe export SRT or VTT subtitles.
  - Initial state: visibility=Only while subtitle menu is open; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => void onExportSubtitles(fmt)}","click or native keyboard activation plus current owning state"]; handler={() => void onExportSubtitles(fmt)}.
  - Exact call/state/backend: stateTransition=close menu -> save dialog -> done/empty/failure toast; backendTrace=["web/src/components/shell/TitleBar.tsx:289::candidate handler -> {() => void onExportSubtitles(fmt)}","actual branch/state -> close menu -> save dialog -> done/empty/failure toast","exact call/arguments -> onExportSubtitles(fmt): call getDefaultProjectDir() only when projectPath is null, saveDialog with fmt extension, then exportSubtitles(withExt(chosen,fmt),fmt)","web/src/components/shell/TitleBar.tsx::onExportSubtitles(format) -> api.exportSubtitles(path,format)","web/src/lib/api.ts::getDefaultProjectDir -> invoke('get_default_project_dir') via src-tauri/src/commands.rs when projectPath is null","web/src/lib/api.ts::exportSubtitles -> invoke('export_subtitles',{path,format})","src-tauri/src/commands.rs::export_subtitles(path,format) -> crates/opentake-domain/src/subtitle_export.rs::export_srt/export_vtt","code:web/src/components/shell/TitleBar.tsx#TitleBar","code:web/src/components/shell/TitleBar.tsx#onExportSubtitles","code:web/src/lib/api.ts#getDefaultProjectDir","code:web/src/lib/api.ts#exportSubtitles","code:src-tauri/src/commands.rs#export_subtitles","code:crates/opentake-domain/src/subtitle_export.rs#export_srt"].
  - Visible/accessibility/return path: success=export SRT or VTT subtitles: close menu -> save dialog -> done/empty/failure toast; accessibility={"focus":"Native keyboard-focusable control","label":"menuitem","shortcut":"None declared on this control"}; returnPath=["Navigation changes the full-screen view; popup actions should close and return focus to the title-bar trigger."].
  - Outcome matrix: {"success":"export SRT or VTT subtitles: close menu -> save dialog -> done/empty/failure toast","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/shell/TitleBar.tsx:289; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/TitleBar.tsx:289; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in close menu -> save dialog -> done/empty/failure toast; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in close menu -> save dialog -> done/empty/failure toast.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/shell/TitleBar.tsx:289; the missing DOM test must prove whether it is surfaced or silent."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/TitleBar.interaction.test.tsx#control-f54f4037ab7bffbe export SRT or VTT subtitles` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify the baseline**

  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-f54f4037ab7bffbe export SRT or VTT subtitles"`

  Observed: the production UI/API/Tauri slice already existed, so the newly registered owning test passed immediately. No artificial production failure was introduced; the missing deliverable was exact owning evidence.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/TitleBar.tsx`, `web/src/components/shell/TitleBar.tsx#onExportSubtitles`, `web/src/lib/api.ts#getDefaultProjectDir`, `src-tauri/src/commands.rs`, `web/src/lib/api.ts#exportSubtitles`, `src-tauri/src/commands.rs#export_subtitles`, `crates/opentake-domain/src/subtitle_export.rs#export_srt`, `web/src/components/shell/TitleBar.tsx#TitleBar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Observed: production code required no change. The owning DOM test now covers SRT/VTT success, zero-cue empty state, write failure, user cancellation, default-directory fallback, extension completion, and typed API arguments.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-f54f4037ab7bffbe export SRT or VTT subtitles"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. See `docs/audit/2026-07-14/runtime-artifacts/automated/subtitle-export-real-device-2026-07-30.md` for the native title-bar/save-panel run and generated-file validation.

### Task 15: control-acceptance (implementation-slice-a85196307f399399)

**Covered records:**
- `control-record-a555659f5f37bc1e` (control)

**Files:**
- Modify: `web/src/components/shell/TitleBar.tsx`
- Modify: `web/src/components/shell/TitleBar.tsx#INTERCHANGE_FORMATS`
- Modify: `web/src/lib/api.ts#getDefaultProjectDir`
- Modify: `src-tauri/src/commands.rs`
- Modify: `web/src/lib/api.ts`
- Modify: `src-tauri/src/commands.rs#selected`
- Modify: `crates/opentake-project/src/fcpxml.rs#export_xmeml`
- Modify: `web/src/components/shell/TitleBar.tsx#TitleBar`
- Test (reviewed-planned): `web/src/components/shell/TitleBar.interaction.test.tsx#control-0d98e5e5a0c417ed export XMEML/FCPXML/OTIO/EDL`

**Candidate-bound contracts:**

#### control-record-a555659f5f37bc1e

- Candidate/source: `control-0d98e5e5a0c417ed` at `web/src/components/shell/TitleBar.tsx:396:15` (control)
- Expected behavior: export XMEML/FCPXML/OTIO/EDL: close menu -> save dialog -> format command -> success/failure toast
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-0d98e5e5a0c417ed.
  - Test: web/src/components/shell/TitleBar.interaction.test.tsx#control-0d98e5e5a0c417ed export XMEML/FCPXML/OTIO/EDL.
  - Initial state: visibility=Only while interchange menu is open; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => void onExportInterchange(fmt)}","click or native keyboard activation plus current owning state"]; handler={() => void onExportInterchange(fmt)}.
  - Exact call/state/backend: stateTransition=close menu -> save dialog -> format command -> success/failure toast; backendTrace=["web/src/components/shell/TitleBar.tsx:396::candidate handler -> {() => void onExportInterchange(fmt)}","actual branch/state -> close menu -> save dialog -> format command -> success/failure toast","exact call/arguments -> onExportInterchange(fmt): call getDefaultProjectDir() only when projectPath is null, saveDialog with fmt.ext, then call exactly fmt.run(withExt(chosen,fmt.ext)) for the selected Xmeml/Fcpxml/Otio/Edl row","web/src/components/shell/TitleBar.tsx::INTERCHANGE_FORMATS/onExportInterchange -> selected api.exportXmeml/exportFcpxmlModern/exportOtio/exportEdl","web/src/lib/api.ts::getDefaultProjectDir -> invoke('get_default_project_dir') via src-tauri/src/commands.rs when projectPath is null","web/src/lib/api.ts -> invoke selected export_xmeml/export_fcpxml_modern/export_otio/export_edl with {path}","src-tauri/src/commands.rs::selected export command -> crates/opentake-project/src/fcpxml.rs::export_xmeml, fcpxml_modern.rs::export_fcpxml, otio.rs::export_otio, or edl.rs::export_edl","code:web/src/components/shell/TitleBar.tsx#TitleBar","code:web/src/components/shell/TitleBar.tsx#INTERCHANGE_FORMATS","code:web/src/lib/api.ts#getDefaultProjectDir","code:crates/opentake-project/src/fcpxml.rs#export_xmeml"].
  - Visible/accessibility/return path: success=export XMEML/FCPXML/OTIO/EDL: close menu -> save dialog -> format command -> success/failure toast; accessibility={"focus":"Native keyboard-focusable control","label":"menuitem","shortcut":"None declared on this control"}; returnPath=["Navigation changes the full-screen view; popup actions should close and return focus to the title-bar trigger."].
  - Outcome matrix: {"success":"export XMEML/FCPXML/OTIO/EDL: close menu -> save dialog -> format command -> success/failure toast","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/shell/TitleBar.tsx:396; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/TitleBar.tsx:396; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in close menu -> save dialog -> format command -> success/failure toast; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in close menu -> save dialog -> format command -> success/failure toast.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/shell/TitleBar.tsx:396; the missing DOM test must prove whether it is surfaced or silent."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/TitleBar.interaction.test.tsx#control-0d98e5e5a0c417ed export XMEML/FCPXML/OTIO/EDL` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

  Result: Added the exact planned control test for XMEML, FCPXML, OTIO, and EDL, plus failure, cancellation, default-directory, extension, menu-dismissal, and macOS 26 native-dialog compatibility coverage.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-0d98e5e5a0c417ed export XMEML/FCPXML/OTIO/EDL"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

  Result: The production interchange routes already existed, so the initial owning test passed rather than manufacturing an artificial RED. Real-device verification then supplied the missing failure evidence: EDL selected caption overlays as `Offline`, and the native extension filter disabled Save on macOS 26.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/TitleBar.tsx`, `web/src/components/shell/TitleBar.tsx#INTERCHANGE_FORMATS`, `web/src/lib/api.ts#getDefaultProjectDir`, `src-tauri/src/commands.rs`, `web/src/lib/api.ts`, `src-tauri/src/commands.rs#selected`, `crates/opentake-project/src/fcpxml.rs#export_xmeml`, `web/src/components/shell/TitleBar.tsx#TitleBar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Result: Preserved the project-derived save path while avoiding the incompatible macOS native filter, and corrected EDL to skip text/Lottie overlays in favor of actual video/image clips. See `docs/audit/2026-07-14/runtime-artifacts/automated/interchange-export-real-device-2026-07-30.md`.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.interaction.test.tsx -t "control-0d98e5e5a0c417ed export XMEML/FCPXML/OTIO/EDL"`

  Expected: PASS with every candidate-bound assertion executed.

  Result: PASS. The repository web suite passed 70 files / 721 tests, including all six exact interchange-control cases; the EDL module passed 14/14 focused tests.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. The full Rust workspace, warnings-denied workspace clippy, web build, formatting, and diff checks completed successfully. The rebuilt macOS application exported and parsed all four formats, and the corrected three-event EDL was deterministic.

### Task 16: control-acceptance (implementation-slice-4a4cd45e11211989)

**Covered records:**
- `control-record-24307094f1fe3df2` (control)
- `control-record-bffa991dea90b52b` (control)
- `control-record-91fd218fd746d615` (control)
- `control-record-80e2fc4a0dd0d790` (control)
- `control-record-b3151533cd6a49bc` (control)

**Files:**
- Modify: `web/src/components/shell/ViewMenu.tsx`
- Modify: `web/src/components/shell/ViewMenu.tsx#ViewMenu`
- Test (reviewed-planned): `web/src/components/shell/ViewMenu.interaction.test.tsx#control-d826a0ad433703cb open/close the View menu`
- Test (reviewed-planned): `web/src/components/shell/ViewMenu.interaction.test.tsx#control-a2d1b5cb37952878 select a layout preset`
- Test (reviewed-planned): `web/src/components/shell/ViewMenu.interaction.test.tsx#control-d606f9c3adb8762a toggle the Agent panel`
- Test (reviewed-planned): `web/src/components/shell/ViewMenu.interaction.test.tsx#control-acd9e30dacf466b6 toggle the Media panel`
- Test (reviewed-planned): `web/src/components/shell/ViewMenu.interaction.test.tsx#control-fb7422e825128973 toggle the Inspector panel`

**Candidate-bound contracts:**

#### control-record-24307094f1fe3df2

- Candidate/source: `control-d826a0ad433703cb` at `web/src/components/shell/ViewMenu.tsx:54:7` (control)
- Expected behavior: open/close the View menu: setOpen toggles; outside/Escape closes
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d826a0ad433703cb.
  - Test: web/src/components/shell/ViewMenu.interaction.test.tsx#control-d826a0ad433703cb open/close the View menu.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setOpen((v) => !v)}","click or native keyboard activation plus current owning state"]; handler={() => setOpen((v) => !v)}.
  - Exact call/state/backend: stateTransition=setOpen toggles; outside/Escape closes; backendTrace=["web/src/components/shell/ViewMenu.tsx:54::candidate handler -> {() => setOpen((v) => !v)}","actual branch/state -> setOpen toggles; outside/Escape closes","exact call -> setOpen toggles; outside/Escape closes","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ViewMenu.tsx#ViewMenu"].
  - Visible/accessibility/return path: success=open/close the View menu: setOpen toggles; outside/Escape closes; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"view.menu\")","shortcut":"None declared on this control"}; returnPath=["Preset selection closes the menu; panel toggles leave it open; focus return is not managed."].
  - Outcome matrix: {"success":"open/close the View menu: setOpen toggles; outside/Escape closes","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setOpen toggles; outside/Escape closes; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-bffa991dea90b52b

- Candidate/source: `control-a2d1b5cb37952878` at `web/src/components/shell/ViewMenu.tsx:92:13` (control)
- Expected behavior: select a layout preset: setLayoutPreset(p.id) and close menu
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-a2d1b5cb37952878.
  - Test: web/src/components/shell/ViewMenu.interaction.test.tsx#control-a2d1b5cb37952878 select a layout preset.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => { setLayoutPreset(p.id); setOpen(false); }}","click or native keyboard activation plus current owning state"]; handler={() => { setLayoutPreset(p.id); setOpen(false); }}.
  - Exact call/state/backend: stateTransition=setLayoutPreset(p.id) and close menu; backendTrace=["web/src/components/shell/ViewMenu.tsx:92::candidate handler -> {() => { setLayoutPreset(p.id); setOpen(false); }}","actual branch/state -> setLayoutPreset(p.id) and close menu","exact call -> setLayoutPreset(p.id) and close menu","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ViewMenu.tsx#ViewMenu"].
  - Visible/accessibility/return path: success=select a layout preset: setLayoutPreset(p.id) and close menu; accessibility={"focus":"Custom MenuItem focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Preset selection closes the menu; panel toggles leave it open; focus return is not managed."].
  - Outcome matrix: {"success":"select a layout preset: setLayoutPreset(p.id) and close menu","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setLayoutPreset(p.id) and close menu; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-91fd218fd746d615

- Candidate/source: `control-d606f9c3adb8762a` at `web/src/components/shell/ViewMenu.tsx:108:11` (control)
- Expected behavior: toggle the Agent panel: toggleAgentPanel
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d606f9c3adb8762a.
  - Test: web/src/components/shell/ViewMenu.interaction.test.tsx#control-d606f9c3adb8762a toggle the Agent panel.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {toggleAgent}","click or native keyboard activation plus current owning state"]; handler={toggleAgent}.
  - Exact call/state/backend: stateTransition=toggleAgentPanel; backendTrace=["web/src/components/shell/ViewMenu.tsx:108::candidate handler -> {toggleAgent}","actual branch/state -> toggleAgentPanel","exact call -> toggleAgentPanel","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ViewMenu.tsx#ViewMenu"].
  - Visible/accessibility/return path: success=toggle the Agent panel: toggleAgentPanel; accessibility={"focus":"Custom MenuItem focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Preset selection closes the menu; panel toggles leave it open; focus return is not managed."].
  - Outcome matrix: {"success":"toggle the Agent panel: toggleAgentPanel","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-80e2fc4a0dd0d790

- Candidate/source: `control-acd9e30dacf466b6` at `web/src/components/shell/ViewMenu.tsx:115:11` (control)
- Expected behavior: toggle the Media panel: toggleMediaPanel
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-acd9e30dacf466b6.
  - Test: web/src/components/shell/ViewMenu.interaction.test.tsx#control-acd9e30dacf466b6 toggle the Media panel.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {toggleMedia}","click or native keyboard activation plus current owning state"]; handler={toggleMedia}.
  - Exact call/state/backend: stateTransition=toggleMediaPanel; backendTrace=["web/src/components/shell/ViewMenu.tsx:115::candidate handler -> {toggleMedia}","actual branch/state -> toggleMediaPanel","exact call -> toggleMediaPanel","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ViewMenu.tsx#ViewMenu"].
  - Visible/accessibility/return path: success=toggle the Media panel: toggleMediaPanel; accessibility={"focus":"Custom MenuItem focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Preset selection closes the menu; panel toggles leave it open; focus return is not managed."].
  - Outcome matrix: {"success":"toggle the Media panel: toggleMediaPanel","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ViewMenu.tsx:115; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-b3151533cd6a49bc

- Candidate/source: `control-fb7422e825128973` at `web/src/components/shell/ViewMenu.tsx:122:11` (control)
- Expected behavior: toggle the Inspector panel: toggleInspectorPanel
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-fb7422e825128973.
  - Test: web/src/components/shell/ViewMenu.interaction.test.tsx#control-fb7422e825128973 toggle the Inspector panel.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {toggleInspector}","click or native keyboard activation plus current owning state"]; handler={toggleInspector}.
  - Exact call/state/backend: stateTransition=toggleInspectorPanel; backendTrace=["web/src/components/shell/ViewMenu.tsx:122::candidate handler -> {toggleInspector}","actual branch/state -> toggleInspectorPanel","exact call -> toggleInspectorPanel","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ViewMenu.tsx#ViewMenu"].
  - Visible/accessibility/return path: success=toggle the Inspector panel: toggleInspectorPanel; accessibility={"focus":"Custom MenuItem focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Preset selection closes the menu; panel toggles leave it open; focus return is not managed."].
  - Outcome matrix: {"success":"toggle the Inspector panel: toggleInspectorPanel","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/ViewMenu.interaction.test.tsx#control-d826a0ad433703cb open/close the View menu` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/ViewMenu.interaction.test.tsx#control-a2d1b5cb37952878 select a layout preset` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/ViewMenu.interaction.test.tsx#control-d606f9c3adb8762a toggle the Agent panel` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/ViewMenu.interaction.test.tsx#control-acd9e30dacf466b6 toggle the Media panel` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/ViewMenu.interaction.test.tsx#control-fb7422e825128973 toggle the Inspector panel` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.interaction.test.tsx -t "control-d826a0ad433703cb open/close the View menu"`
  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.interaction.test.tsx -t "control-a2d1b5cb37952878 select a layout preset"`
  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.interaction.test.tsx -t "control-d606f9c3adb8762a toggle the Agent panel"`
  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.interaction.test.tsx -t "control-acd9e30dacf466b6 toggle the Media panel"`
  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.interaction.test.tsx -t "control-fb7422e825128973 toggle the Inspector panel"`

  Expected: FAIL because one or more of the 5 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/ViewMenu.tsx`, `web/src/components/shell/ViewMenu.tsx#ViewMenu` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.interaction.test.tsx -t "control-d826a0ad433703cb open/close the View menu"`
  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.interaction.test.tsx -t "control-a2d1b5cb37952878 select a layout preset"`
  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.interaction.test.tsx -t "control-d606f9c3adb8762a toggle the Agent panel"`
  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.interaction.test.tsx -t "control-acd9e30dacf466b6 toggle the Media panel"`
  - Run: `pnpm -C web test -- --run src/components/shell/ViewMenu.interaction.test.tsx -t "control-fb7422e825128973 toggle the Inspector panel"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. The reviewed owning runner was the missing RED artifact; the existing `ViewMenu` implementation already satisfied all five contracts, so no production-code change was required. The true focused Vitest invocation passed 5/5, the complete web suite passed 726/726, and the production web build passed with only the pre-existing bundle warnings. The rebuilt macOS app then verified preset selection, checked states, three panel toggles, Escape dismissal, and outside-click dismissal. Runtime evidence: `runtime-artifacts/automated/view-menu-real-device-2026-07-30.md`.
