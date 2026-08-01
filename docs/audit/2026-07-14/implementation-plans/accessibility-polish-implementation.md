# Accessibility Polish Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 47 verified incomplete records in the `accessibility-polish` gap group.

**Architecture:** Implement 22 primary evidence-bound slices and reference 0 shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.

---

### Task 1: feedback-version-metadata (implementation-slice-f4ae7e178a9f7979)

**Covered records:**
- `requirement-f81d46b5f91e9e68` (requirement)

**Files:**
- Modify: `src-tauri/src/feedback.rs#submit_feedback`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Test (reviewed-planned): `src-tauri/tests/feedback.rs#submission_includes_app_and_os_version`

**Candidate-bound contracts:**

#### requirement-f81d46b5f91e9e68

- Candidate/source: `doc-e74198ba473d1688` at `docs/architecture/MODULE-PORT-MAP.md:977` (requirement)
- Expected behavior: Attach application and OS version metadata to every feedback submission.
- Resolution: `reviewed-mapping-report:feedback-version-metadata` — No tracked feedback-submission owner was found.
- Exact acceptance contract:
  - Implementation: Create a typed feedback payload populated from Tauri package version/build and OS major.minor.patch, send it on every feedback path, redact user data, and test missing-version fallback plus serialization.
  - Add token, semantic-role, keyboard-focus, state, privacy, and visual assertions for every named surface; the affected lint, typecheck, and web suites must pass.
  - Exercise the named surface with keyboard and browser or packaged-app visual inspection, and retain exact accessibility or screenshot evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/tests/feedback.rs#submission_includes_app_and_os_version` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri --test feedback submission_includes_app_and_os_version -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/feedback.rs#submit_feedback`, `docs/architecture/MODULE-PORT-MAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri --test feedback submission_includes_app_and_os_version -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Completed 2026-08-01. The owning integration test was observed RED before the
  module existed, then GREEN after the typed submission boundary was registered.
  It covers package/build and OS metadata, missing-version fallbacks, camel-case
  serialization, no-email contact suppression, and debug redaction. The runtime
  remains offline unless an HTTPS endpoint is explicitly configured; redirects
  are disabled and requests have a 15-second timeout. The formatting check and
  `cargo test --workspace --no-fail-fast` pass. This metadata slice
  has no user-visible surface; the disabled Beta feedback menu is not claimed as
  an implemented feedback form by this task.

### Task 2: centralized-design-token-table + AP-design-token-consistency (implementation-slice-d434285f35d42846)

**Covered records:**
- `requirement-87e46425b74f6667` (requirement)
- `requirement-d4624679e2d13a62` (requirement)
- `requirement-4ed0d164ab8ff383` (requirement)
- `requirement-1e22f85e7e3b8f8a` (requirement)
- `requirement-d0cfcfa29fbae42c` (requirement)
- `requirement-25d4cfadbec947f6` (requirement)
- `requirement-efc9de03c45d49eb` (requirement)
- `requirement-4e81a79521708f05` (requirement)
- `requirement-e26133887e66e54a` (requirement)
- `requirement-5a1da3c2f30572ab` (requirement)
- `requirement-0c45c9ee70dc990f` (requirement)
- `requirement-cfcd1cc3cb758c27` (requirement)
- `requirement-9ce0276c82bdcfe6` (requirement)
- `requirement-47fce8f3ec8e470a` (requirement)
- `requirement-8dc2ce5cb56b0640` (requirement)
- `requirement-a32e4c4537f326df` (requirement)
- `requirement-787494db54513c33` (requirement)

**Files:**
- Modify: `web/src/components/ui/PanelShell.tsx#PanelShell`
- Modify: `web/src/lib/theme.ts#ACCENT`
- Modify: `web/src/lib/theme.ts#BG`
- Modify: `web/src/lib/theme.ts#BORDER`
- Modify: `web/src/lib/theme.ts#FS`
- Modify: `web/src/lib/theme.ts#LAYOUT`
- Modify: `web/src/lib/theme.ts#RADIUS`
- Modify: `web/src/lib/theme.ts#SPACE`
- Modify: `web/src/lib/theme.ts#TEXT`
- Modify: `web/src/lib/theme.ts#TRACK_COLOR`
- Modify: `web/src/styles/tokens.css`
- Modify: `web/src/styles/tokens.css#--bg-raised`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/1-design-tokens.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Test (existing-owned): `web/src/components/preview/TransformOverlay.test.tsx#renders 4 corner handles at the OpenTake spacing/opacity tokens matching upstream AppTheme.Spacing.smMd / Opacity.strong`
- Test (existing-owned): `web/src/components/shell/TitleBar.visual.test.ts#TitleBar alignment`
- Test (reviewed-planned): `web/src/lib/theme.contract.test.ts#complete_upstream_token_table_and_css_projection`
- Test (reviewed-planned): `web/src/styles/tokenUsage.test.ts#representative_panel_spacing_type_radius_and_color_matrix`

**Candidate-bound contracts:**

#### requirement-87e46425b74f6667

- Candidate/source: `doc-37a2d8c4b12f45eb` at `docs/architecture/MODULE-PORT-MAP.md:1144` (requirement)
- Expected behavior: Represent and consistently use the complete upstream design-token table.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Implementation: Define every listed token once in typed/CSS sources, replace hard-coded duplicates in production components, add value and usage audits, and perform visual comparison of all major surfaces.
  - Add token, semantic-role, keyboard-focus, state, privacy, and visual assertions for every named surface; the affected lint, typecheck, and web suites must pass.
  - Exercise the named surface with keyboard and browser or packaged-app visual inspection, and retain exact accessibility or screenshot evidence before reclassification.

#### requirement-d4624679e2d13a62

- Candidate/source: `doc-f5f0f491c3e61fe0` at `docs/modules/web/SPEC.md:1264` (requirement)
- Expected behavior: Use the specified design tokens consistently across every panel and verify representative spacing/type/radius/color values.
- Resolution: `validated-ledger-evidence:AP-design-token-consistency` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Create a pinned upstream/current screenshot-state matrix at the same viewport, scale, locale, theme, and fixture.
  - Add deterministic component/browser assertions for normal, hover, focus, disabled, selected, empty, error, and compact states.
  - Record reviewed pixel/geometry evidence for every item named by the checklist, with no unchecked item remaining.

#### requirement-4ed0d164ab8ff383

- Candidate/source: `doc-8b18542ae6733059` at `docs/specs/frontend/1-design-tokens.md:1` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:1 under “设计令牌表（AppTheme → CSS variables）” (heading), the source “# 设计令牌表（AppTheme → CSS variables）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:1; signal=heading; heading=设计令牌表（AppTheme → CSS variables）; candidate=# 设计令牌表（AppTheme → CSS variables）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “设计令牌表（AppTheme → CSS variables）” in “设计令牌表（AppTheme → CSS variables）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “设计令牌表（AppTheme → CSS variables）” with the scenario below and register test:web/src/__tests__/completion/doc-8b18542ae6733059.test.ts#completion_8b18542ae6733059_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “设计令牌表（AppTheme → CSS variables）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “设计令牌表（AppTheme → CSS variables）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-8b18542ae6733059.test.ts#completion_8b18542ae6733059_the_specified_colors_spacing_typography_radii_op.

#### requirement-1e22f85e7e3b8f8a

- Candidate/source: `doc-20734b14a586c5ba` at `docs/specs/frontend/1-design-tokens.md:6` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:6 under “1.1 背景 Background（`AppTheme.swift:8-23`）” (heading), the source “### 1.1 背景 Background（`AppTheme.swift:8-23`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:6; signal=heading; heading=1.1 背景 Background（`AppTheme.swift:8-23`）; candidate=### 1.1 背景 Background（`AppTheme.swift:8-23`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.1 背景 Background（`AppTheme.swift:8-23`）” in “1.1 背景 Background（`AppTheme.swift:8-23`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.1 背景 Background（`AppTheme.swift:8-23`）” with the scenario below and register test:web/src/__tests__/completion/doc-20734b14a586c5ba.test.ts#completion_20734b14a586c5ba_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.1 背景 Background（`AppTheme.swift:8-23`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.1 背景 Background（`AppTheme.swift:8-23`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-20734b14a586c5ba.test.ts#completion_20734b14a586c5ba_the_specified_colors_spacing_typography_radii_op.

#### requirement-d0cfcfa29fbae42c

- Candidate/source: `doc-0078822a0bf5e928` at `docs/specs/frontend/1-design-tokens.md:17` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:17 under “1.2 边框 Border（`AppTheme.swift:27-43`）” (heading), the source “### 1.2 边框 Border（`AppTheme.swift:27-43`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:17; signal=heading; heading=1.2 边框 Border（`AppTheme.swift:27-43`）; candidate=### 1.2 边框 Border（`AppTheme.swift:27-43`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.2 边框 Border（`AppTheme.swift:27-43`）” in “1.2 边框 Border（`AppTheme.swift:27-43`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.2 边框 Border（`AppTheme.swift:27-43`）” with the scenario below and register test:web/src/__tests__/completion/doc-0078822a0bf5e928.test.ts#completion_0078822a0bf5e928_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.2 边框 Border（`AppTheme.swift:27-43`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.2 边框 Border（`AppTheme.swift:27-43`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-0078822a0bf5e928.test.ts#completion_0078822a0bf5e928_the_specified_colors_spacing_typography_radii_op.

#### requirement-25d4cfadbec947f6

- Candidate/source: `doc-ce2238741d8a172d` at `docs/specs/frontend/1-design-tokens.md:31` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:31 under “1.3 文本 Text（`AppTheme.swift:104-114`）” (heading), the source “### 1.3 文本 Text（`AppTheme.swift:104-114`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:31; signal=heading; heading=1.3 文本 Text（`AppTheme.swift:104-114`）; candidate=### 1.3 文本 Text（`AppTheme.swift:104-114`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.3 文本 Text（`AppTheme.swift:104-114`）” in “1.3 文本 Text（`AppTheme.swift:104-114`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.3 文本 Text（`AppTheme.swift:104-114`）” with the scenario below and register test:web/src/__tests__/completion/doc-ce2238741d8a172d.test.ts#completion_ce2238741d8a172d_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.3 文本 Text（`AppTheme.swift:104-114`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.3 文本 Text（`AppTheme.swift:104-114`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-ce2238741d8a172d.test.ts#completion_ce2238741d8a172d_the_specified_colors_spacing_typography_radii_op.

#### requirement-efc9de03c45d49eb

- Candidate/source: `doc-69664131a16b091b` at `docs/specs/frontend/1-design-tokens.md:40` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:40 under “1.4 强调色 Accent / 状态 / 玻璃（`AppTheme.swift:47-100`）” (heading), the source “### 1.4 强调色 Accent / 状态 / 玻璃（`AppTheme.swift:47-100`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:40; signal=heading; heading=1.4 强调色 Accent / 状态 / 玻璃（`AppTheme.swift:47-100`）; candidate=### 1.4 强调色 Accent / 状态 / 玻璃（`AppTheme.swift:47-100`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.4 强调色 Accent / 状态 / 玻璃（`AppTheme.swift:47-100`）” in “1.4 强调色 Accent / 状态 / 玻璃（`AppTheme.swift:47-100`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.4 强调色 Accent / 状态 / 玻璃（`AppTheme.swift:47-100`）” with the scenario below and register test:web/src/__tests__/completion/doc-69664131a16b091b.test.ts#completion_69664131a16b091b_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.4 强调色 Accent / 状态 / 玻璃（`AppTheme.swift:47-100`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.4 强调色 Accent / 状态 / 玻璃（`AppTheme.swift:47-100`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-69664131a16b091b.test.ts#completion_69664131a16b091b_the_specified_colors_spacing_typography_radii_op.

#### requirement-4e81a79521708f05

- Candidate/source: `doc-531c27845e0fcc40` at `docs/specs/frontend/1-design-tokens.md:53` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:53 under “1.5 轨道类型色 TrackColor（`AppTheme.swift:133-139`；`ClipType.themeColor` 映射 `AppTheme.swift:307-317`）” (heading), the source “### 1.5 轨道类型色 TrackColor（`AppTheme.swift:133-139`；`ClipType.themeColor` 映射 `AppTheme.swift:307-317`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:53; signal=heading; heading=1.5 轨道类型色 TrackColor（`AppTheme.swift:133-139`；`ClipType.themeColor` 映射 `AppTheme.swift:307-317`）; candidate=### 1.5 轨道类型色 TrackColor（`AppTheme.swift:133-139`；`ClipType.themeColor` 映射 `AppTheme.swift:307-317`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.5 轨道类型色 TrackColor（`AppTheme.swift:133-139`；`ClipType.themeColor` 映射 `AppTheme.swift:307-317`）” in “1.5 轨道类型色 TrackColor（`AppTheme.swift:133-139`；`ClipType.themeColor` 映射 `AppTheme.swift:307-317`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.5 轨道类型色 TrackColor（`AppTheme.swift:133-139`；`ClipType.themeColor` 映射 `AppTheme.swift:307-317`）” with the scenario below and register test:web/src/__tests__/completion/doc-531c27845e0fcc40.test.ts#completion_531c27845e0fcc40_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.5 轨道类型色 TrackColor（`AppTheme.swift:133-139`；`ClipType.themeColor` 映射 `AppTheme.swift:307-317`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.5 轨道类型色 TrackColor（`AppTheme.swift:133-139`；`ClipType.themeColor` 映射 `AppTheme.swift:307-317`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-531c27845e0fcc40.test.ts#completion_531c27845e0fcc40_the_specified_colors_spacing_typography_radii_op.

#### requirement-e26133887e66e54a

- Candidate/source: `doc-3ea7643b06f00000` at `docs/specs/frontend/1-design-tokens.md:63` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:63 under “1.6 圆角 Radius（`AppTheme.swift:143-155`）” (heading), the source “### 1.6 圆角 Radius（`AppTheme.swift:143-155`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:63; signal=heading; heading=1.6 圆角 Radius（`AppTheme.swift:143-155`）; candidate=### 1.6 圆角 Radius（`AppTheme.swift:143-155`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.6 圆角 Radius（`AppTheme.swift:143-155`）” in “1.6 圆角 Radius（`AppTheme.swift:143-155`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.6 圆角 Radius（`AppTheme.swift:143-155`）” with the scenario below and register test:web/src/__tests__/completion/doc-3ea7643b06f00000.test.ts#completion_3ea7643b06f00000_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.6 圆角 Radius（`AppTheme.swift:143-155`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.6 圆角 Radius（`AppTheme.swift:143-155`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-3ea7643b06f00000.test.ts#completion_3ea7643b06f00000_the_specified_colors_spacing_typography_radii_op.

#### requirement-5a1da3c2f30572ab

- Candidate/source: `doc-cccea9264066e494` at `docs/specs/frontend/1-design-tokens.md:77` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:77 under “1.7 间距 Spacing（`AppTheme.swift:159-171`）” (heading), the source “### 1.7 间距 Spacing（`AppTheme.swift:159-171`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:77; signal=heading; heading=1.7 间距 Spacing（`AppTheme.swift:159-171`）; candidate=### 1.7 间距 Spacing（`AppTheme.swift:159-171`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.7 间距 Spacing（`AppTheme.swift:159-171`）” in “1.7 间距 Spacing（`AppTheme.swift:159-171`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.7 间距 Spacing（`AppTheme.swift:159-171`）” with the scenario below and register test:web/src/__tests__/completion/doc-cccea9264066e494.test.ts#completion_cccea9264066e494_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.7 间距 Spacing（`AppTheme.swift:159-171`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.7 间距 Spacing（`AppTheme.swift:159-171`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-cccea9264066e494.test.ts#completion_cccea9264066e494_the_specified_colors_spacing_typography_radii_op.

#### requirement-0c45c9ee70dc990f

- Candidate/source: `doc-f0ed0277b667ebb1` at `docs/specs/frontend/1-design-tokens.md:93` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:93 under “1.8 字号 FontSize（`AppTheme.swift:175-188`）+ 字重/字距” (heading), the source “### 1.8 字号 FontSize（`AppTheme.swift:175-188`）+ 字重/字距” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:93; signal=heading; heading=1.8 字号 FontSize（`AppTheme.swift:175-188`）+ 字重/字距; candidate=### 1.8 字号 FontSize（`AppTheme.swift:175-188`）+ 字重/字距
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.8 字号 FontSize（`AppTheme.swift:175-188`）+ 字重/字距” in “1.8 字号 FontSize（`AppTheme.swift:175-188`）+ 字重/字距”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.8 字号 FontSize（`AppTheme.swift:175-188`）+ 字重/字距” with the scenario below and register test:web/src/__tests__/completion/doc-f0ed0277b667ebb1.test.ts#completion_f0ed0277b667ebb1_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.8 字号 FontSize（`AppTheme.swift:175-188`）+ 字重/字距” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.8 字号 FontSize（`AppTheme.swift:175-188`）+ 字重/字距”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-f0ed0277b667ebb1.test.ts#completion_f0ed0277b667ebb1_the_specified_colors_spacing_typography_radii_op.

#### requirement-cfcd1cc3cb758c27

- Candidate/source: `doc-5cf5b17068551566` at `docs/specs/frontend/1-design-tokens.md:115` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:115 under “1.9 图标尺寸 IconSize（square frame，`AppTheme.swift:210-220`）” (heading), the source “### 1.9 图标尺寸 IconSize（square frame，`AppTheme.swift:210-220`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:115; signal=heading; heading=1.9 图标尺寸 IconSize（square frame，`AppTheme.swift:210-220`）; candidate=### 1.9 图标尺寸 IconSize（square frame，`AppTheme.swift:210-220`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.9 图标尺寸 IconSize（square frame，`AppTheme.swift:210-220`）” in “1.9 图标尺寸 IconSize（square frame，`AppTheme.swift:210-220`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.9 图标尺寸 IconSize（square frame，`AppTheme.swift:210-220`）” with the scenario below and register test:web/src/__tests__/completion/doc-5cf5b17068551566.test.ts#completion_5cf5b17068551566_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “1.9 图标尺寸 IconSize（square frame，`AppTheme.swift:210-220`）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-5cf5b17068551566.test.ts#completion_5cf5b17068551566_the_specified_colors_spacing_typography_radii_op.

#### requirement-9ce0276c82bdcfe6

- Candidate/source: `doc-6ec500b7866bf34a` at `docs/specs/frontend/1-design-tokens.md:127` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:127 under “1.10 不透明度 Opacity（`AppTheme.swift:118-129`）” (heading), the source “### 1.10 不透明度 Opacity（`AppTheme.swift:118-129`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:127; signal=heading; heading=1.10 不透明度 Opacity（`AppTheme.swift:118-129`）; candidate=### 1.10 不透明度 Opacity（`AppTheme.swift:118-129`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.10 不透明度 Opacity（`AppTheme.swift:118-129`）” in “1.10 不透明度 Opacity（`AppTheme.swift:118-129`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.10 不透明度 Opacity（`AppTheme.swift:118-129`）” with the scenario below and register test:web/src/__tests__/completion/doc-6ec500b7866bf34a.test.ts#completion_6ec500b7866bf34a_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.10 不透明度 Opacity（`AppTheme.swift:118-129`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.10 不透明度 Opacity（`AppTheme.swift:118-129`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-6ec500b7866bf34a.test.ts#completion_6ec500b7866bf34a_the_specified_colors_spacing_typography_radii_op.

#### requirement-47fce8f3ec8e470a

- Candidate/source: `doc-fcfaba8dde0fe791` at `docs/specs/frontend/1-design-tokens.md:137` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:137 under “1.11 阴影 Shadow（`AppTheme.swift:274-278`，签名 `267-272`）” (heading), the source “### 1.11 阴影 Shadow（`AppTheme.swift:274-278`，签名 `267-272`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:137; signal=heading; heading=1.11 阴影 Shadow（`AppTheme.swift:274-278`，签名 `267-272`）; candidate=### 1.11 阴影 Shadow（`AppTheme.swift:274-278`，签名 `267-272`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.11 阴影 Shadow（`AppTheme.swift:274-278`，签名 `267-272`）” in “1.11 阴影 Shadow（`AppTheme.swift:274-278`，签名 `267-272`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.11 阴影 Shadow（`AppTheme.swift:274-278`，签名 `267-272`）” with the scenario below and register test:web/src/__tests__/completion/doc-fcfaba8dde0fe791.test.ts#completion_fcfaba8dde0fe791_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.11 阴影 Shadow（`AppTheme.swift:274-278`，签名 `267-272`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.11 阴影 Shadow（`AppTheme.swift:274-278`，签名 `267-272`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-fcfaba8dde0fe791.test.ts#completion_fcfaba8dde0fe791_the_specified_colors_spacing_typography_radii_op.

#### requirement-8dc2ce5cb56b0640

- Candidate/source: `doc-a3132c42710780f1` at `docs/specs/frontend/1-design-tokens.md:147` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:147 under “1.12 动画时长 Anim（`AppTheme.swift:282-285`）” (heading), the source “### 1.12 动画时长 Anim（`AppTheme.swift:282-285`）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:147; signal=heading; heading=1.12 动画时长 Anim（`AppTheme.swift:282-285`）; candidate=### 1.12 动画时长 Anim（`AppTheme.swift:282-285`）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.12 动画时长 Anim（`AppTheme.swift:282-285`）” in “1.12 动画时长 Anim（`AppTheme.swift:282-285`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.12 动画时长 Anim（`AppTheme.swift:282-285`）” with the scenario below and register test:web/src/__tests__/completion/doc-a3132c42710780f1.test.ts#completion_a3132c42710780f1_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.12 动画时长 Anim（`AppTheme.swift:282-285`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.12 动画时长 Anim（`AppTheme.swift:282-285`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-a3132c42710780f1.test.ts#completion_a3132c42710780f1_the_specified_colors_spacing_typography_radii_op.

#### requirement-a32e4c4537f326df

- Candidate/source: `doc-5c391130191270bc` at `docs/specs/frontend/1-design-tokens.md:156` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:156 under “1.13 复合/局部尺寸常量（散落，必须照搬）” (heading), the source “### 1.13 复合/局部尺寸常量（散落，必须照搬）” requires this exact behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:156; signal=heading; heading=1.13 复合/局部尺寸常量（散落，必须照搬）; candidate=### 1.13 复合/局部尺寸常量（散落，必须照搬）
  - Expected behavior: The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI. This closes only the promise expressed by “1.13 复合/局部尺寸常量（散落，必须照搬）” in “1.13 复合/局部尺寸常量（散落，必须照搬）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.13 复合/局部尺寸常量（散落，必须照搬）” with the scenario below and register test:web/src/__tests__/completion/doc-5c391130191270bc.test.ts#completion_5c391130191270bc_the_specified_colors_spacing_typography_radii_op
  - Initial state/input/event: render the exact “1.13 复合/局部尺寸常量（散落，必须照搬）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “1.13 复合/局部尺寸常量（散落，必须照搬）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The specified colors, spacing, typography, radii, opacity, shadows, animation, and local dimensions are centralized and consumed by the React UI.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-5c391130191270bc.test.ts#completion_5c391130191270bc_the_specified_colors_spacing_typography_radii_op.

#### requirement-787494db54513c33

- Candidate/source: `doc-7d4f1268d36e7de1` at `docs/specs/frontend/13-implementation.md:21` (requirement)
- Expected behavior: Use the exact design tokens for spacing, type, radius, and color throughout every panel.
- Resolution: `reviewed-mapping-report:centralized-design-token-table` — Both slices cover the centralized theme/token table and representative cross-panel token consumption through the same theme.ts and tokens.css boundary.
- Exact acceptance contract:
  - Implementation: Create a token-usage audit, replace remaining production literals, sample at least five values per panel against the spec, and add automated style/visual checks.
  - Add token, semantic-role, keyboard-focus, state, privacy, and visual assertions for every named surface; the affected lint, typecheck, and web suites must pass.
  - Exercise the named surface with keyboard and browser or packaged-app visual inspection, and retain exact accessibility or screenshot evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/TransformOverlay.test.tsx#renders 4 corner handles at the OpenTake spacing/opacity tokens matching upstream AppTheme.Spacing.smMd / Opacity.strong` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/shell/TitleBar.visual.test.ts#TitleBar alignment` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/theme.contract.test.ts#complete_upstream_token_table_and_css_projection` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/styles/tokenUsage.test.ts#representative_panel_spacing_type_radius_and_color_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.test.tsx -t "renders 4 corner handles at the OpenTake spacing/opacity tokens matching upstream AppTheme.Spacing.smMd / Opacity.strong"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.visual.test.ts -t "TitleBar alignment"`
  - Run: `pnpm -C web test -- --run src/lib/theme.contract.test.ts -t "complete_upstream_token_table_and_css_projection"`
  - Run: `pnpm -C web test -- --run src/styles/tokenUsage.test.ts -t "representative_panel_spacing_type_radius_and_color_matrix"`

  Expected: FAIL because one or more of the 17 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/ui/PanelShell.tsx#PanelShell`, `web/src/lib/theme.ts#ACCENT`, `web/src/lib/theme.ts#BG`, `web/src/lib/theme.ts#BORDER`, `web/src/lib/theme.ts#FS`, `web/src/lib/theme.ts#LAYOUT`, `web/src/lib/theme.ts#RADIUS`, `web/src/lib/theme.ts#SPACE`, `web/src/lib/theme.ts#TEXT`, `web/src/lib/theme.ts#TRACK_COLOR`, `web/src/styles/tokens.css`, `web/src/styles/tokens.css#--bg-raised`, `docs/architecture/MODULE-PORT-MAP.md`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/1-design-tokens.md`, `docs/specs/frontend/13-implementation.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.test.tsx -t "renders 4 corner handles at the OpenTake spacing/opacity tokens matching upstream AppTheme.Spacing.smMd / Opacity.strong"`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.visual.test.ts -t "TitleBar alignment"`
  - Run: `pnpm -C web test -- --run src/lib/theme.contract.test.ts -t "complete_upstream_token_table_and_css_projection"`
  - Run: `pnpm -C web test -- --run src/styles/tokenUsage.test.ts -t "representative_panel_spacing_type_radius_and_color_matrix"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Code acceptance completed 2026-08-01. Both reviewed-planned owners were
  observed RED, then all four focused runners passed (17 assertions). The full
  Web gate passes with 97 files / 834 tests and a production build; only the
  pre-existing dynamic-import and chunk-size advisories remain. The typed table
  now covers every documented token and local dimension, the CSS projection has
  no undefined production references, and captions/keyframe UI consume their
  typed local constants. Keyframe rows were corrected from 24px to the specified
  22px.

- [ ] **Runtime evidence gate:** inspect the packaged editor at the pinned
  viewport/scale/locale and retain the six-surface Shell/Toolbar/Media/Inspector/
  Preview/Timeline token-parity evidence during the sequential GUI phase.

### Task 3: dsn-gated-telemetry-init (implementation-slice-6fd8dbcff3a60a4b)

**Covered records:**
- `requirement-c1b2d10efb06a9d7` (requirement)

**Files:**
- Modify: `src-tauri/src/telemetry.rs#init_telemetry`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Test (reviewed-planned): `src-tauri/tests/telemetry_init.rs#starts_only_with_explicit_packaged_or_environment_dsn`

**Candidate-bound contracts:**

#### requirement-c1b2d10efb06a9d7

- Candidate/source: `doc-b2acd8885847b4fd` at `docs/architecture/MODULE-PORT-MAP.md:1235` (requirement)
- Expected behavior: Initialize error telemetry only when an explicit packaged/environment DSN is present.
- Resolution: `reviewed-mapping-report:dsn-gated-telemetry-init` — No Sentry or DSN initialization owner was found; a settings label is not implementation evidence.
- Exact acceptance contract:
  - Implementation: Add opt-in DSN configuration, keep telemetry disabled on empty/missing DSN, redact sensitive paths/keys, document privacy behavior, and test enabled/disabled initialization.
  - Add token, semantic-role, keyboard-focus, state, privacy, and visual assertions for every named surface; the affected lint, typecheck, and web suites must pass.
  - Exercise the named surface with keyboard and browser or packaged-app visual inspection, and retain exact accessibility or screenshot evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/tests/telemetry_init.rs#starts_only_with_explicit_packaged_or_environment_dsn` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri --test telemetry_init starts_only_with_explicit_packaged_or_environment_dsn -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/telemetry.rs#init_telemetry`, `docs/architecture/MODULE-PORT-MAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri --test telemetry_init starts_only_with_explicit_packaged_or_environment_dsn -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: the reviewed test first failed while the telemetry owner
  was absent, then passed after the DSN gate and scrubber were integrated. The
  exact owner, telemetry unit test, `cargo fmt --all -- --check`, and
  `CARGO_INCREMENTAL=0 cargo test --workspace --no-fail-fast` all passed. The
  workspace gate reported only the existing explicitly ignored real-device
  probes.

### Task 4: release-accessibility-visual-parity (implementation-slice-75d12e695c4bc7e4)

**Covered records:**
- `requirement-2ac9b3f7dfdb31aa` (requirement)

**Files:**
- Modify: `web/src/components/ui/PanelShell.tsx#PanelShell`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#accessibleClipRects`
- Modify: `web/src/styles/global.css`
- Modify: `docs/architecture/PORT-1TO1-GAP.md`
- Test (reviewed-planned): `web/src/releaseParity.test.tsx#sample_projects_accessibility_visual_and_interaction_gate`

**Candidate-bound contracts:**

#### requirement-2ac9b3f7dfdb31aa

- Candidate/source: `doc-e60fd5e9eda5907d` at `docs/architecture/PORT-1TO1-GAP.md:176` (requirement)
- Expected behavior: Sample projects, accessibility, visual polish, and final interaction parity are release-ready.
- Resolution: `reviewed-mapping-report:release-accessibility-visual-parity` — This is a release-level umbrella spanning sample projects, accessibility, visual polish, and interaction parity; no local test can close it alone.
- Exact acceptance contract:
  - Close keyboard/focus/hover/accessibility gaps in the exhaustive UI checklist.
  - Validate sample-project load and core edit flows.
  - Run visual and accessibility regression checks on packaged desktop builds.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/releaseParity.test.tsx#sample_projects_accessibility_visual_and_interaction_gate` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/releaseParity.test.tsx -t "sample_projects_accessibility_visual_and_interaction_gate"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/ui/PanelShell.tsx#PanelShell`, `web/src/components/timeline/TimelineContainer.tsx#accessibleClipRects`, `web/src/styles/global.css`, `docs/architecture/PORT-1TO1-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/releaseParity.test.tsx -t "sample_projects_accessibility_visual_and_interaction_gate"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: the focused release owner failed first because panel
  regions were not keyboard-focusable. It passed after keyboard panel focus and
  global focus-visible, reduced-motion, and forced-colors behavior were added.
  The release owner also binds the existing executable sample materialization,
  rollback, Home routing, and 24px timeline clip-access contracts. The full Web
  gate passed with 98 files / 835 tests and the production build passed; only
  the existing dynamic-import and chunk-size advisories remained.

- [ ] **Runtime evidence gate:** on the packaged desktop build, open an offline
  sample, perform the core edit flow, traverse panel and timeline controls by
  keyboard, inspect focus/hover/high-contrast/reduced-motion presentation, and
  retain screenshots plus the exact interaction result before release-ready
  reclassification.

### Task 5: AP-keyboard-shortcut-matrix + complete-shortcut-table (implementation-slice-c9e6c68dcdf6b5bb)

**Covered records:**
- `requirement-31e4396c279ec423` (requirement)
- `requirement-9187f701d82e5081` (requirement)
- `requirement-58486b72211ee376` (requirement)

**Files:**
- Modify: `web/src/App.tsx#App`
- Modify: `web/src/hooks/useKeyboardShortcuts.ts#handleProjectSaveKeyDown`
- Modify: `web/src/hooks/useKeyboardShortcuts.ts#handleTransportSpaceKeyDown`
- Modify: `web/src/hooks/useKeyboardShortcuts.ts#useKeyboardShortcuts`
- Modify: `web/src/store/editActions.ts#splitAtPlayhead`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Modify: `docs/specs/frontend/9-interactions.md`
- Test (reviewed-planned): `web/src/hooks/useKeyboardShortcuts.matrix.test.ts#all_shortcuts_conflicts_editable_suppression_and_platform_modifiers`
- Test (reviewed-planned): `web/src/hooks/useKeyboardShortcuts.test.ts#complete_documented_shortcut_table`
- Test (existing-owned): `web/src/hooks/useKeyboardShortcuts.test.ts#handles plain Space in the editor`
- Test (existing-owned): `web/src/hooks/useKeyboardShortcuts.test.ts#keyboard transport Space shortcut`
- Test (existing-owned): `web/src/hooks/useKeyboardShortcuts.test.ts#prevents the native shortcut but ignores repeated KeyS events`
- Test (existing-owned): `web/src/hooks/useKeyboardShortcuts.test.ts#project save shortcut`

**Candidate-bound contracts:**

#### requirement-31e4396c279ec423

- Candidate/source: `doc-5735972aa68fd924` at `docs/modules/web/SPEC.md:1277` (requirement)
- Expected behavior: Verify the complete §9.6 shortcut table, conflicts, editable-target suppression, and platform modifiers.
- Resolution: `validated-ledger-evidence:AP-keyboard-shortcut-matrix` — Both slices require the complete documented shortcut table, conflict/editable-target rules and platform modifiers through the same hook and test file.
- Exact acceptance contract:
  - Translate every numbered checklist row into a named deterministic interaction test and a native/browser runtime evidence row where applicable.
  - Assert the exact store/API command, visible success/failure state, and no-op behavior for rejected/cancelled paths.
  - Pass the full matrix against the pinned upstream semantics with no unchecked rows.

#### requirement-9187f701d82e5081

- Candidate/source: `doc-b224743997144fb9` at `docs/specs/frontend/13-implementation.md:34` (requirement)
- Expected behavior: Implement and verify the complete shortcut table.
- Resolution: `reviewed-mapping-report:complete-shortcut-table` — Both slices require the complete documented shortcut table, conflict/editable-target rules and platform modifiers through the same hook and test file.
- Exact acceptance contract:
  - Implementation: Generate a shortcut conformance test from the spec table covering modifiers, repeat suppression, focus exclusions, enabled state and command payloads on macOS/Windows semantics.
  - Add token, semantic-role, keyboard-focus, state, privacy, and visual assertions for every named surface; the affected lint, typecheck, and web suites must pass.
  - Exercise the named surface with keyboard and browser or packaged-app visual inspection, and retain exact accessibility or screenshot evidence before reclassification.

#### requirement-58486b72211ee376

- Candidate/source: `doc-5e1e0aa550a580ff` at `docs/specs/frontend/9-interactions.md:69` (requirement)
- Expected behavior: Complete the exhaustive 9.6 键盘快捷键全表（`EditorWindowController.handleKeyDown` `38-164` + `MainMenu.swift`） interaction matrix with parity across mouse, trackpad, and keyboard inputs.
- Resolution: `validated-ledger-evidence:AP-keyboard-shortcut-matrix` — Both slices require the complete documented shortcut table, conflict/editable-target rules and platform modifiers through the same hook and test file.
- Exact acceptance contract:
  - Implement every documented shortcut with platform modifiers, focus/input-field exclusions, menu state, and shared action equivalence.
  - Conflicting or disabled shortcuts must not mutate state; repeat behavior and undo grouping must match the specified command.
  - Table-drive all shortcut rows across editor focus, text input, modal, locked selection, playback, and no-project states.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/hooks/useKeyboardShortcuts.matrix.test.ts#all_shortcuts_conflicts_editable_suppression_and_platform_modifiers` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/hooks/useKeyboardShortcuts.test.ts#complete_documented_shortcut_table` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/hooks/useKeyboardShortcuts.test.ts#handles plain Space in the editor` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/hooks/useKeyboardShortcuts.test.ts#keyboard transport Space shortcut` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/hooks/useKeyboardShortcuts.test.ts#prevents the native shortcut but ignores repeated KeyS events` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/hooks/useKeyboardShortcuts.test.ts#project save shortcut` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.matrix.test.ts -t "all_shortcuts_conflicts_editable_suppression_and_platform_modifiers"`
  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.test.ts -t "complete_documented_shortcut_table"`
  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.test.ts -t "handles plain Space in the editor"`
  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.test.ts -t "keyboard transport Space shortcut"`
  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.test.ts -t "prevents the native shortcut but ignores repeated KeyS events"`
  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.test.ts -t "project save shortcut"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/App.tsx#App`, `web/src/hooks/useKeyboardShortcuts.ts#handleProjectSaveKeyDown`, `web/src/hooks/useKeyboardShortcuts.ts#handleTransportSpaceKeyDown`, `web/src/hooks/useKeyboardShortcuts.ts#useKeyboardShortcuts`, `web/src/store/editActions.ts#splitAtPlayhead`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/13-implementation.md`, `docs/specs/frontend/9-interactions.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.matrix.test.ts -t "all_shortcuts_conflicts_editable_suppression_and_platform_modifiers"`
  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.test.ts -t "complete_documented_shortcut_table"`
  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.test.ts -t "handles plain Space in the editor"`
  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.test.ts -t "keyboard transport Space shortcut"`
  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.test.ts -t "prevents the native shortcut but ignores repeated KeyS events"`
  - Run: `pnpm -C web test -- --run src/hooks/useKeyboardShortcuts.test.ts -t "project save shortcut"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: both reviewed owners first failed because the complete
  resolver/table did not exist. They passed after one physical-key resolver was
  integrated with the hook and native-menu semantic command boundary. All six
  named existing/focused owners passed; the full Web gate passed with 99 files /
  837 tests and the production build passed, with only the existing dynamic-
  import and chunk-size advisories.

- [ ] **Runtime evidence gate:** exercise every §9.6 row in the packaged desktop
  app on the macOS accelerator path, including input focus, modal, repeat,
  disabled/read-only and no-project states; retain the exact command/result and
  visible-state evidence before final parity reclassification.

### Task 6: AP-hover-focus-cursor-matrix + complete-hover-cursor-table (implementation-slice-7dce3a0f53283ac9)

**Covered records:**
- `requirement-a6156af582cbf335` (requirement)
- `requirement-682854f8d348f2f3` (requirement)
- `requirement-88ea38135949439a` (requirement)

**Files:**
- Modify: `web/src/components/inspector/ScrubbableNumberField.tsx#ScrubbableNumberField`
- Modify: `web/src/components/preview/CropOverlay.tsx#CropOverlay`
- Modify: `web/src/components/preview/TransformOverlay.tsx#CORNER_CURSOR`
- Modify: `web/src/components/preview/TransformOverlay.tsx#TransformOverlay`
- Modify: `web/src/components/shell/SplitPane.tsx#SplitPane`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#toolMode`
- Modify: `web/src/components/ui/HoverButton.tsx#HoverButton`
- Modify: `web/src/styles/global.css`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Modify: `docs/specs/frontend/9-interactions.md`
- Test (existing-owned): `web/src/components/preview/TransformOverlay.test.tsx#TransformOverlay`
- Test (existing-owned): `web/src/components/preview/TransformOverlay.test.tsx#renders 4 corner handles at the OpenTake spacing/opacity tokens matching upstream AppTheme.Spacing.smMd / Opacity.strong`
- Test (existing-owned): `web/src/components/ui/HoverButton.test.tsx#does not opt icon buttons out of normal keyboard focus`
- Test (reviewed-planned): `web/src/components/ui/interactionStateMatrix.test.tsx#all_enabled_disabled_hover_focus_and_cursor_modes`
- Test (reviewed-planned): `web/src/interactionParity.test.tsx#complete_hover_cursor_matrix`

**Candidate-bound contracts:**

#### requirement-a6156af582cbf335

- Candidate/source: `doc-f84c669a72e32d27` at `docs/modules/web/SPEC.md:1278` (requirement)
- Expected behavior: Verify the complete §9.7 hover and cursor table in every enabled/disabled interaction mode.
- Resolution: `validated-ledger-evidence:AP-hover-focus-cursor-matrix` — Both slices require one cross-component hover/focus/cursor state matrix and share the same timeline, transform and scrubbable owners plus overlay test boundary.
- Exact acceptance contract:
  - Translate every numbered checklist row into a named deterministic interaction test and a native/browser runtime evidence row where applicable.
  - Assert the exact store/API command, visible success/failure state, and no-op behavior for rejected/cancelled paths.
  - Pass the full matrix against the pinned upstream semantics with no unchecked rows.

#### requirement-682854f8d348f2f3

- Candidate/source: `doc-022d673015d34ed0` at `docs/specs/frontend/13-implementation.md:35` (requirement)
- Expected behavior: Implement and verify the complete hover/cursor table.
- Resolution: `reviewed-mapping-report:complete-hover-cursor-table` — Both slices require one cross-component hover/focus/cursor state matrix and share the same timeline, transform and scrubbable owners plus overlay test boundary.
- Exact acceptance contract:
  - Implementation: Add interaction/visual tests for every hover and cursor row across enabled/disabled/dragging states and remove inconsistent cursor literals.
  - Add token, semantic-role, keyboard-focus, state, privacy, and visual assertions for every named surface; the affected lint, typecheck, and web suites must pass.
  - Exercise the named surface with keyboard and browser or packaged-app visual inspection, and retain exact accessibility or screenshot evidence before reclassification.

#### requirement-88ea38135949439a

- Candidate/source: `doc-d8d420611581c37d` at `docs/specs/frontend/9-interactions.md:102` (requirement)
- Expected behavior: Complete the exhaustive 9.7 Hover / 焦点 / 游标态 interaction matrix with parity across mouse, trackpad, and keyboard inputs.
- Resolution: `validated-ledger-evidence:AP-hover-focus-cursor-matrix` — Both slices require one cross-component hover/focus/cursor state matrix and share the same timeline, transform and scrubbable owners plus overlay test boundary.
- Exact acceptance contract:
  - Expose specified hover, pressed, focus-visible, drag, resize, trim, razor, forbidden, and loading cursors/states without keyboard-only regressions.
  - Focus order and restoration must remain deterministic across panels, menus, dialogs, project switches, and hidden/maximized panels.
  - Run keyboard-only focus traversal plus pointer-state visual snapshots at default and high-contrast themes, with no focus traps or unlabeled controls.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/TransformOverlay.test.tsx#TransformOverlay` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/TransformOverlay.test.tsx#renders 4 corner handles at the OpenTake spacing/opacity tokens matching upstream AppTheme.Spacing.smMd / Opacity.strong` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/ui/HoverButton.test.tsx#does not opt icon buttons out of normal keyboard focus` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/ui/interactionStateMatrix.test.tsx#all_enabled_disabled_hover_focus_and_cursor_modes` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/interactionParity.test.tsx#complete_hover_cursor_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.test.tsx -t "TransformOverlay"`
  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.test.tsx -t "renders 4 corner handles at the OpenTake spacing/opacity tokens matching upstream AppTheme.Spacing.smMd / Opacity.strong"`
  - Run: `pnpm -C web test -- --run src/components/ui/HoverButton.test.tsx -t "does not opt icon buttons out of normal keyboard focus"`
  - Run: `pnpm -C web test -- --run src/components/ui/interactionStateMatrix.test.tsx -t "all_enabled_disabled_hover_focus_and_cursor_modes"`
  - Run: `pnpm -C web test -- --run src/interactionParity.test.tsx -t "complete_hover_cursor_matrix"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/ScrubbableNumberField.tsx#ScrubbableNumberField`, `web/src/components/preview/CropOverlay.tsx#CropOverlay`, `web/src/components/preview/TransformOverlay.tsx#CORNER_CURSOR`, `web/src/components/preview/TransformOverlay.tsx#TransformOverlay`, `web/src/components/shell/SplitPane.tsx#SplitPane`, `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`, `web/src/components/timeline/TimelineContainer.tsx#toolMode`, `web/src/components/ui/HoverButton.tsx#HoverButton`, `web/src/styles/global.css`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/13-implementation.md`, `docs/specs/frontend/9-interactions.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.test.tsx -t "TransformOverlay"`
  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.test.tsx -t "renders 4 corner handles at the OpenTake spacing/opacity tokens matching upstream AppTheme.Spacing.smMd / Opacity.strong"`
  - Run: `pnpm -C web test -- --run src/components/ui/HoverButton.test.tsx -t "does not opt icon buttons out of normal keyboard focus"`
  - Run: `pnpm -C web test -- --run src/components/ui/interactionStateMatrix.test.tsx -t "all_enabled_disabled_hover_focus_and_cursor_modes"`
  - Run: `pnpm -C web test -- --run src/interactionParity.test.tsx -t "complete_hover_cursor_matrix"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: the two reviewed matrix owners first failed because the
  shared timeline cursor projection and interaction-state attributes did not
  exist. All five named focused owners passed after the cursor/focus/disabled
  boundary was integrated. The full Web gate passed with 101 files / 839 tests
  and the production build passed, with only the existing dynamic-import and
  chunk-size advisories.

- [ ] **Runtime evidence gate:** traverse the packaged app by keyboard and
  pointer, then capture default/high-contrast hover, focus-visible, pressed,
  disabled, resize, trim, razor, forbidden, loading and dragging states with no
  focus traps or unlabeled controls before final parity reclassification.

### Task 7: AP-i18n-runtime-contract (implementation-slice-2123bc4e99fa84c2)

**Covered records:**
- `requirement-c90ae1f0093038d9` (requirement)

**Files:**
- Modify: `web/src/i18n/index.ts#DEFAULT_LOCALE`
- Modify: `web/src/i18n/index.ts#translate`
- Modify: `web/src/i18n/dict.ts#DICTS`
- Modify: `docs/modules/web/hooks-i18n-theme.md`
- Test (reviewed-planned): `web/src/i18n/index.test.ts#defaults_zh_cn_supports_en_and_preserves_unknown_named_placeholders`

**Candidate-bound contracts:**

#### requirement-c90ae1f0093038d9

- Candidate/source: `doc-742eba941ad541a0` at `docs/modules/web/hooks-i18n-theme.md:27` (requirement)
- Expected behavior: Default to zh-CN, support en, and interpolate named placeholders without deleting unknown placeholders.
- Resolution: `validated-ledger-evidence:AP-i18n-runtime-contract` — Runtime already defaults zh-CN, supports en, interpolates names and preserves unknown placeholders; focused proof is absent.
- Exact acceptance contract:
  - Add deterministic unit tests for default/invalid persisted locale, zh-CN/en lookup, missing-key fallback, numeric/string interpolation, unknown placeholder preservation, locale persistence, and document.lang updates.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/i18n/index.test.ts#defaults_zh_cn_supports_en_and_preserves_unknown_named_placeholders` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/i18n/index.test.ts -t "defaults_zh_cn_supports_en_and_preserves_unknown_named_placeholders"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/i18n/index.ts#DEFAULT_LOCALE`, `web/src/i18n/index.ts#translate`, `web/src/i18n/dict.ts#DICTS`, `docs/modules/web/hooks-i18n-theme.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/i18n/index.test.ts -t "defaults_zh_cn_supports_en_and_preserves_unknown_named_placeholders"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: the reviewed owner first failed because an invalid
  persisted locale remained in storage after the runtime fell back to zh-CN.
  The runtime now removes unsupported persisted values, tolerates unavailable
  storage, and exports the default/translation boundaries for deterministic
  proof. The full Web gate passed with 102 files / 840 tests and the production
  build passed, with only the existing dynamic-import and chunk-size advisories.

### Task 8: AP-bg-placeholder-token (implementation-slice-2c3937178aa8f103)

**Covered records:**
- `requirement-4a3f468f9a960cca` (requirement)

**Files:**
- Modify: `web/src/styles/tokens.css#--bg-placeholder`
- Modify: `web/src/styles/tokens.css#--bg-raised`
- Modify: `docs/specs/frontend/1-design-tokens.md`
- Test (reviewed-planned): `web/src/styles/tokens.test.ts#bg_placeholder_equals_raised_rgb_30`

**Candidate-bound contracts:**

#### requirement-4a3f468f9a960cca

- Candidate/source: `doc-de0f95b975d2b2c6` at `docs/specs/frontend/1-design-tokens.md:14` (requirement)
- Expected behavior: At docs/specs/frontend/1-design-tokens.md:14 under “1.1 背景 Background（`AppTheme.swift:8-23`）” (gap-marker), the source “| placeholder | = raised | `--bg-placeholder` | `rgb(30,30,30)` |” requires this exact behavior: Define bg-placeholder exactly as rgb(30,30,30), equal to bg-raised.
- Resolution: `validated-ledger-evidence:AP-bg-placeholder-token` — Both CSS properties are exactly rgb(30, 30, 30); only focused evidence is missing.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/1-design-tokens.md:14; signal=gap-marker; heading=1.1 背景 Background（`AppTheme.swift:8-23`）; candidate=| placeholder | = raised | `--bg-placeholder` | `rgb(30,30,30)` |
  - Expected behavior: Define bg-placeholder exactly as rgb(30,30,30), equal to bg-raised. This closes only the promise expressed by “placeholder | = raised | `--bg-placeholder` | `rgb(30,30,30)`” in “1.1 背景 Background（`AppTheme.swift:8-23`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “placeholder | = raised | `--bg-placeholder` | `rgb(30,30,30)`” with the scenario below and register test:web/src/__tests__/completion/doc-de0f95b975d2b2c6.test.ts#completion_de0f95b975d2b2c6_define_bg_placeholder_exactly_as_rgb_30_30_30_eq
  - Initial state/input/event: render the exact “1.1 背景 Background（`AppTheme.swift:8-23`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “placeholder | = raised | `--bg-placeholder` | `rgb(30,30,30)`”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “Define bg-placeholder exactly as rgb(30,30,30), equal to bg-raised.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-de0f95b975d2b2c6.test.ts#completion_de0f95b975d2b2c6_define_bg_placeholder_exactly_as_rgb_30_30_30_eq.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/styles/tokens.test.ts#bg_placeholder_equals_raised_rgb_30` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/styles/tokens.test.ts -t "bg_placeholder_equals_raised_rgb_30"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/styles/tokens.css#--bg-placeholder`, `web/src/styles/tokens.css#--bg-raised`, `docs/specs/frontend/1-design-tokens.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/styles/tokens.test.ts -t "bg_placeholder_equals_raised_rgb_30"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: focused RED was the reviewed owner being absent. The
  existing CSS projection already held both tokens at exact `rgb(30,30,30)`,
  so GREEN adds the missing direct equality proof without changing production
  values. The focused owner passes. The full completion-audit regression gate
  ran 206 assertions (203 passed); after removing an induced frozen-source
  drift, the two remaining focused failures are both caused by the four
  preserved, user-owned audit outputs not matching their normative
  renders/inventory. Step 5 remains open until that external dirty state is
  reconciled.

### Task 9: five-panel-layout-focus (implementation-slice-b99fb8c63f2086b0)

**Covered records:**
- `requirement-1b9ada0db4670130` (requirement)

**Files:**
- Modify: `web/src/components/shell/EditorSplit.tsx#EditorSplit`
- Modify: `web/src/components/shell/EditorSplit.tsx#DefaultLayout`
- Modify: `web/src/components/shell/EditorSplit.tsx#MediaLayout`
- Modify: `web/src/components/shell/EditorSplit.tsx#VerticalLayout`
- Modify: `web/src/components/ui/PanelShell.tsx#PanelShell`
- Modify: `docs/specs/frontend/13-implementation.md`
- Test (existing-owned): `web/src/components/ui/PanelShell.test.tsx#PanelShell preview surface`
- Test (reviewed-planned): `web/src/components/shell/EditorSplit.test.tsx#all_presets_ratios_gutters_surfaces_focus`

**Candidate-bound contracts:**

#### requirement-1b9ada0db4670130

- Candidate/source: `doc-2b26469de8b23218` at `docs/specs/frontend/13-implementation.md:20` (requirement)
- Expected behavior: Match all default/media/vertical five-panel layout ratios, 5px gutters, 6px surfaces, and focus rings.
- Resolution: `reviewed-mapping-report:five-panel-layout-focus` — Layout and focus owners exist, but current tests cover PanelShell focus only and not all presets, ratios, gutters, and surfaces.
- Exact acceptance contract:
  - Implementation: Build deterministic layout fixtures at representative window sizes for all presets; match every ratio/gutter/radius/focus state and add visual plus keyboard-focus assertions.
  - Add token, semantic-role, keyboard-focus, state, privacy, and visual assertions for every named surface; the affected lint, typecheck, and web suites must pass.
  - Exercise the named surface with keyboard and browser or packaged-app visual inspection, and retain exact accessibility or screenshot evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/ui/PanelShell.test.tsx#PanelShell preview surface` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/shell/EditorSplit.test.tsx#all_presets_ratios_gutters_surfaces_focus` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/ui/PanelShell.test.tsx -t "PanelShell preview surface"`
  - Run: `pnpm -C web test -- --run src/components/shell/EditorSplit.test.tsx -t "all_presets_ratios_gutters_surfaces_focus"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/EditorSplit.tsx#EditorSplit`, `web/src/components/shell/EditorSplit.tsx#DefaultLayout`, `web/src/components/shell/EditorSplit.tsx#MediaLayout`, `web/src/components/shell/EditorSplit.tsx#VerticalLayout`, `web/src/components/ui/PanelShell.tsx#PanelShell`, `docs/specs/frontend/13-implementation.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/ui/PanelShell.test.tsx -t "PanelShell preview surface"`
  - Run: `pnpm -C web test -- --run src/components/shell/EditorSplit.test.tsx -t "all_presets_ratios_gutters_surfaces_focus"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Reverified 2026-08-01: both owning runners pass (2 files / 4 tests), covering
  all three presets at 1600×1000 plus reduced viewport, documented initial
  ratios, panel visibility/maximize behavior, keyboard-adjustable separators,
  semantic regions and focus transfer. The full Web gate passes with 103 files
  / 841 tests and the production build passes, with only the existing
  dynamic-import and chunk-size advisories.

- [ ] **Runtime evidence gate:** in the packaged app at 1600×1000, capture all
  three presets with the five panels enabled and verify 5px gutters, 6px
  surfaces, focused/unfocused rings, Tab focus and keyboard separator resize.

### Task 10: control-acceptance (implementation-slice-7729e824b5300938)

**Covered records:**
- `control-record-41941cd0c20e6390` (control)

**Files:**
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx`
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`
- Test (reviewed-planned): `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-75a9964d0b81961a keyframe lane seek`

**Candidate-bound contracts:**

#### control-record-41941cd0c20e6390

- Candidate/source: `control-75a9964d0b81961a` at `web/src/components/inspector/KeyframesLaneRow.tsx:279:7` (control)
- Expected behavior: keyframe lane seek: assert exactly if (e.target === e.currentTarget) setActiveFrame(startFrame + xToFrame(e.clientX)); contextmenu only preventDefault() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-75a9964d0b81961a.
  - Test: web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-75a9964d0b81961a keyframe lane seek.
  - Initial state: visibility=Visible when one clip is selected and the Inspector keyframes panel/owning row or menu is mounted.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={handleTrackClick} {(e) => e.preventDefault()}.
  - Exact call/state/backend: stateTransition=keyframe lane seek: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/KeyframesLaneRow.tsx:279::handler {handleTrackClick} {(e) => e.preventDefault()} -> if (e.target === e.currentTarget) setActiveFrame(startFrame + xToFrame(e.clientX)); contextmenu only preventDefault()","web/src/components/inspector/KeyframesLaneRow.tsx::KeyframesLaneRow.handleTrackClick -> uiStore.setActiveFrame","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow"].
  - Visible/accessibility/return path: success=keyframe lane seek: assert exactly if (e.target === e.currentTarget) setActiveFrame(startFrame + xToFrame(e.clientX)); contextmenu only preventDefault() and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"keyframe lane seek: assert exactly if (e.target === e.currentTarget) setActiveFrame(startFrame + xToFrame(e.clientX)); contextmenu only preventDefault() and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-75a9964d0b81961a keyframe lane seek` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-75a9964d0b81961a keyframe lane seek"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/KeyframesLaneRow.tsx`, `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-75a9964d0b81961a keyframe lane seek"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: the focused owner failed first because the lane had no
  queryable/focusable semantic boundary. It now proves empty-lane pointer seek,
  child-click isolation, context-menu no-op, zero edit-command emission,
  horizontal slider semantics and keyboard frame seek with retained focus. The
  full Web gate passes with 104 files / 842 tests and the production build
  passes, with only the existing dynamic-import and chunk-size advisories.

- [ ] **Runtime evidence gate:** in the packaged Inspector keyframes panel,
  verify pointer and keyboard seek, visible focus, frame updates, child click
  isolation and right-click no-op before final control reclassification.

### Task 11: control-acceptance (implementation-slice-0fa7eb23d9e9be73)

**Covered records:**
- `control-record-7e8bb4389dd145f5` (control)

**Files:**
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx`
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx#handleDiamondMouseDown`
- Modify: `web/src/store/editActions.ts#moveKeyframe`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::MoveKeyframe`
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-4e0a20c7d0e54f3e keyframe diamond drag/context menu`

**Candidate-bound contracts:**

#### control-record-7e8bb4389dd145f5

- Candidate/source: `control-4e0a20c7d0e54f3e` at `web/src/components/inspector/KeyframesLaneRow.tsx:293:11` (control)
- Expected behavior: keyframe diamond drag/context menu: assert exactly onMouseDown starts local drag; window mouseup calls edit.moveKeyframe(clip.id, property, fromFrame, currentFrame) only when the frame changed; onContextMenu sets menu { x: e.clientX, y: e.clientY, frame: kf.key } and emits no edit command and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-4e0a20c7d0e54f3e.
  - Test: web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-4e0a20c7d0e54f3e keyframe diamond drag/context menu.
  - Initial state: visibility=Visible when one clip is selected and the Inspector keyframes panel/owning row or menu is mounted.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(e) => handleDiamondMouseDown(e, kf.key)} {(e) => handleDiamondContextMenu(e, kf.key)}.
  - Exact call/state/backend: stateTransition=keyframe diamond drag/context menu: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/KeyframesLaneRow.tsx:293::handler {(e) => handleDiamondMouseDown(e, kf.key)} {(e) => handleDiamondContextMenu(e, kf.key)} -> onMouseDown starts local drag; window mouseup calls edit.moveKeyframe(clip.id, property, fromFrame, currentFrame) only when the frame changed; onContextMenu sets menu { x: e.clientX, y: e.clientY, frame: kf.key } and emits no edit command","web/src/components/inspector/KeyframesLaneRow.tsx::handleDiamondMouseDown/handleDiamondContextMenu","web/src/store/editActions.ts::moveKeyframe(clip.id, property, fromFrame, currentFrame) on changed drag","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::MoveKeyframe","code:web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow","code:web/src/components/inspector/KeyframesLaneRow.tsx#handleDiamondMouseDown","code:web/src/store/editActions.ts#moveKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=keyframe diamond drag/context menu: assert exactly onMouseDown starts local drag; window mouseup calls edit.moveKeyframe(clip.id, property, fromFrame, currentFrame) only when the frame changed; onContextMenu sets menu { x: e.clientX, y: e.clientY, frame: kf.key } and emits no edit command and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"keyframe diamond drag/context menu: assert exactly onMouseDown starts local drag; window mouseup calls edit.moveKeyframe(clip.id, property, fromFrame, currentFrame) only when the frame changed; onContextMenu sets menu { x: e.clientX, y: e.clientY, frame: kf.key } and emits no edit command and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-4e0a20c7d0e54f3e keyframe diamond drag/context menu` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-4e0a20c7d0e54f3e keyframe diamond drag/context menu"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/KeyframesLaneRow.tsx`, `web/src/components/inspector/KeyframesLaneRow.tsx#handleDiamondMouseDown`, `web/src/store/editActions.ts#moveKeyframe`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::MoveKeyframe`, `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-4e0a20c7d0e54f3e keyframe diamond drag/context menu"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: RED proved the diamond lacked an accessible owner. The
  focused test now covers unchanged-drag no-op, changed-drag exact single
  command, right-click menu coordinates with zero edit calls, keyboard
  one-frame movement with focus retention, and visible toast recovery on a
  rejected command. The Web gate passes with 104 files / 843 tests plus a
  production build. Formatting and the complete Rust workspace pass; only the
  seven pre-existing real-device probes remain ignored.

- [ ] **Runtime evidence gate:** in the packaged Inspector, drag a diamond,
  open its menu by pointer and keyboard, verify focus/labels, and force a safe
  rejected move to confirm visible recovery before final control
  reclassification.

### Task 12: control-acceptance (implementation-slice-40b57db02ead9758)

**Covered records:**
- `control-record-db596f1698835aa1` (control)

**Files:**
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx`
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframeContextMenu`
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`
- Test (reviewed-planned): `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-6e36c47f93f0d4fb dismiss keyframe context menu`

**Candidate-bound contracts:**

#### control-record-db596f1698835aa1

- Candidate/source: `control-6e36c47f93f0d4fb` at `web/src/components/inspector/KeyframesLaneRow.tsx:364:7` (control)
- Expected behavior: dismiss keyframe context menu: assert exactly click calls onClose(); contextmenu preventDefault() then onClose() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-6e36c47f93f0d4fb.
  - Test: web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-6e36c47f93f0d4fb dismiss keyframe context menu.
  - Initial state: visibility=Visible when one clip is selected and the Inspector keyframes panel/owning row or menu is mounted.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={onClose} {(e) => {
          e.preventDefault();
          onClose();
        }}.
  - Exact call/state/backend: stateTransition=dismiss keyframe context menu: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/KeyframesLaneRow.tsx:364::handler {onClose} {(e) => {\n          e.preventDefault();\n          onClose();\n        }} -> click calls onClose(); contextmenu preventDefault() then onClose()","web/src/components/inspector/KeyframesLaneRow.tsx::KeyframeContextMenu backdrop -> closeMenu -> setMenu(null)","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow","code:web/src/components/inspector/KeyframesLaneRow.tsx#KeyframeContextMenu"].
  - Visible/accessibility/return path: success=dismiss keyframe context menu: assert exactly click calls onClose(); contextmenu preventDefault() then onClose() and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"dismiss keyframe context menu: assert exactly click calls onClose(); contextmenu preventDefault() then onClose() and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-6e36c47f93f0d4fb dismiss keyframe context menu` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-6e36c47f93f0d4fb dismiss keyframe context menu"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/KeyframesLaneRow.tsx`, `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframeContextMenu`, `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-6e36c47f93f0d4fb dismiss keyframe context menu"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: RED proved the context menu had no menu semantic or
  focus contract. The owning test now proves outside click, outside
  context-menu and Escape each dismiss without an edit command, and each path
  restores focus to the invoking diamond. The full Web gate passes with 104
  files / 844 tests and the production build passes, with only the existing
  dynamic-import and chunk-size advisories.

- [ ] **Runtime evidence gate:** open and dismiss the packaged keyframe menu by
  pointer, right-click and Escape, confirming visible focus returns to the
  invoking diamond before final control reclassification.

### Task 13: control-acceptance (implementation-slice-29f0b31549c65faf)

**Covered records:**
- `control-record-30ae12e323308b6c` (control)

**Files:**
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx`
- Modify: `web/src/store/editActions.ts#removeKeyframe`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::RemoveKeyframe`
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-c191a17716450b1a delete keyframe`

**Candidate-bound contracts:**

#### control-record-30ae12e323308b6c

- Candidate/source: `control-c191a17716450b1a` at `web/src/components/inspector/KeyframesLaneRow.tsx:386:9` (control)
- Expected behavior: delete keyframe: assert exactly onDelete() -> edit.removeKeyframe(clip.id, property, menu.frame); then closeMenu() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c191a17716450b1a.
  - Test: web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-c191a17716450b1a delete keyframe.
  - Initial state: visibility=Visible when one clip is selected and the Inspector keyframes panel/owning row or menu is mounted.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={onDelete}.
  - Exact call/state/backend: stateTransition=delete keyframe: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/KeyframesLaneRow.tsx:386::handler {onDelete} -> onDelete() -> edit.removeKeyframe(clip.id, property, menu.frame); then closeMenu()","web/src/store/editActions.ts::removeKeyframe(clip.id, property, menu.frame)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::RemoveKeyframe","code:web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow","code:web/src/store/editActions.ts#removeKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=delete keyframe: assert exactly onDelete() -> edit.removeKeyframe(clip.id, property, menu.frame); then closeMenu() and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"delete keyframe: assert exactly onDelete() -> edit.removeKeyframe(clip.id, property, menu.frame); then closeMenu() and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-c191a17716450b1a delete keyframe` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-c191a17716450b1a delete keyframe"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/KeyframesLaneRow.tsx`, `web/src/store/editActions.ts#removeKeyframe`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::RemoveKeyframe`, `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-c191a17716450b1a delete keyframe"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: RED proved the delete item lacked a native accessible
  action boundary. The owner now proves one exact remove command, no sibling
  edit command, immediate menu close, trigger-focus restoration and visible
  toast recovery after backend rejection. The Web gate passes with 104 files /
  845 tests plus a production build. Formatting and the complete Rust workspace
  pass; only the seven pre-existing real-device probes remain ignored.

- [ ] **Runtime evidence gate:** delete a disposable keyframe from the packaged
  menu by pointer and keyboard, verify focus restoration and safely exercise a
  rejected deletion before final control reclassification.

### Task 14: control-acceptance (implementation-slice-a43944e5aef30cb5)

**Covered records:**
- `control-record-1f80de3d071a1896` (control)
- `control-record-af6a4f3e12360ec6` (control)
- `control-record-b4a1d0f0dae13986` (control)

**Files:**
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx`
- Modify: `web/src/store/editActions.ts#setKeyframeInterpolation`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetKeyframeInterpolation`
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-3b4230aba22c9422 linear keyframe interpolation`
- Test (reviewed-planned): `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-16737eebbe9cb784 hold keyframe interpolation`
- Test (reviewed-planned): `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-ab879256f29c4e0a smooth keyframe interpolation`

**Candidate-bound contracts:**

#### control-record-1f80de3d071a1896

- Candidate/source: `control-3b4230aba22c9422` at `web/src/components/inspector/KeyframesLaneRow.tsx:398:9` (control)
- Expected behavior: linear keyframe interpolation: assert exactly onSetInterpolation('linear') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'linear'); then closeMenu() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-3b4230aba22c9422.
  - Test: web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-3b4230aba22c9422 linear keyframe interpolation.
  - Initial state: visibility=Visible when one clip is selected and the Inspector keyframes panel/owning row or menu is mounted.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => onSetInterpolation("linear")}.
  - Exact call/state/backend: stateTransition=linear keyframe interpolation: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/KeyframesLaneRow.tsx:398::handler {() => onSetInterpolation(\"linear\")} -> onSetInterpolation('linear') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'linear'); then closeMenu()","web/src/store/editActions.ts::setKeyframeInterpolation(...,'linear')","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetKeyframeInterpolation","code:web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow","code:web/src/store/editActions.ts#setKeyframeInterpolation","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=linear keyframe interpolation: assert exactly onSetInterpolation('linear') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'linear'); then closeMenu() and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"linear keyframe interpolation: assert exactly onSetInterpolation('linear') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'linear'); then closeMenu() and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-af6a4f3e12360ec6

- Candidate/source: `control-16737eebbe9cb784` at `web/src/components/inspector/KeyframesLaneRow.tsx:402:9` (control)
- Expected behavior: hold keyframe interpolation: assert exactly onSetInterpolation('hold') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'hold'); then closeMenu() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-16737eebbe9cb784.
  - Test: web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-16737eebbe9cb784 hold keyframe interpolation.
  - Initial state: visibility=Visible when one clip is selected and the Inspector keyframes panel/owning row or menu is mounted.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => onSetInterpolation("hold")}.
  - Exact call/state/backend: stateTransition=hold keyframe interpolation: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/KeyframesLaneRow.tsx:402::handler {() => onSetInterpolation(\"hold\")} -> onSetInterpolation('hold') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'hold'); then closeMenu()","web/src/store/editActions.ts::setKeyframeInterpolation(...,'hold')","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetKeyframeInterpolation","code:web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow","code:web/src/store/editActions.ts#setKeyframeInterpolation","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=hold keyframe interpolation: assert exactly onSetInterpolation('hold') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'hold'); then closeMenu() and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"hold keyframe interpolation: assert exactly onSetInterpolation('hold') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'hold'); then closeMenu() and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-b4a1d0f0dae13986

- Candidate/source: `control-ab879256f29c4e0a` at `web/src/components/inspector/KeyframesLaneRow.tsx:406:9` (control)
- Expected behavior: smooth keyframe interpolation: assert exactly onSetInterpolation('smooth') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'smooth'); then closeMenu() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-ab879256f29c4e0a.
  - Test: web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-ab879256f29c4e0a smooth keyframe interpolation.
  - Initial state: visibility=Visible when one clip is selected and the Inspector keyframes panel/owning row or menu is mounted.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => onSetInterpolation("smooth")}.
  - Exact call/state/backend: stateTransition=smooth keyframe interpolation: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/KeyframesLaneRow.tsx:406::handler {() => onSetInterpolation(\"smooth\")} -> onSetInterpolation('smooth') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'smooth'); then closeMenu()","web/src/store/editActions.ts::setKeyframeInterpolation(...,'smooth')","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetKeyframeInterpolation","code:web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow","code:web/src/store/editActions.ts#setKeyframeInterpolation","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=smooth keyframe interpolation: assert exactly onSetInterpolation('smooth') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'smooth'); then closeMenu() and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"smooth keyframe interpolation: assert exactly onSetInterpolation('smooth') -> edit.setKeyframeInterpolation(clip.id, property, menu.frame, 'smooth'); then closeMenu() and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-3b4230aba22c9422 linear keyframe interpolation` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-16737eebbe9cb784 hold keyframe interpolation` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-ab879256f29c4e0a smooth keyframe interpolation` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-3b4230aba22c9422 linear keyframe interpolation"`
  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-16737eebbe9cb784 hold keyframe interpolation"`
  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-ab879256f29c4e0a smooth keyframe interpolation"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/KeyframesLaneRow.tsx`, `web/src/store/editActions.ts#setKeyframeInterpolation`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetKeyframeInterpolation`, `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-3b4230aba22c9422 linear keyframe interpolation"`
  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-16737eebbe9cb784 hold keyframe interpolation"`
  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-ab879256f29c4e0a smooth keyframe interpolation"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: all three focused owners first failed only on absent
  failure recovery. Linear, hold and smooth now each prove one exact
  interpolation command, no sibling command, native menuitem activation,
  immediate close, trigger-focus restoration and a visible rejected-command
  toast. The Web gate passes with 104 files / 848 tests plus a production build.
  Formatting and the complete Rust workspace pass; only the seven pre-existing
  real-device probes remain ignored.

- [ ] **Runtime evidence gate:** set linear, hold and smooth on disposable
  keyframes in the packaged menu using pointer and keyboard, verify focus
  restoration and a safe rejection state before final control
  reclassification.

### Task 15: control-acceptance (implementation-slice-112e03f8a358e250)

**Covered records:**
- `control-record-8d6d316f2d51973f` (control)
- `control-record-fb15dd12a9e06926` (control)

**Files:**
- Modify: `web/src/components/inspector/ScrubbableNumberField.tsx`
- Modify: `web/src/components/inspector/ScrubbableNumberField.tsx#ScrubbableNumberField`
- Test (reviewed-planned): `web/src/components/inspector/ScrubbableNumberField.interaction.test.tsx#control-481c7d66573516a6 numeric text-entry mode`
- Test (reviewed-planned): `web/src/components/inspector/ScrubbableNumberField.interaction.test.tsx#control-3e4fc80f4dde046e pointer-scrubbable numeric value`

**Candidate-bound contracts:**

#### control-record-8d6d316f2d51973f

- Candidate/source: `control-481c7d66573516a6` at `web/src/components/inspector/ScrubbableNumberField.tsx:101:7` (control)
- Expected behavior: numeric text-entry mode: assert exactly onChange setDraft(e.target.value); blur or Enter parses suffix/decimal comma, clamps, and calls p.onCommit(clampedParsedValue) only when finite, then setEditing(false); Escape only setEditing(false) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-481c7d66573516a6.
  - Test: web/src/components/inspector/ScrubbableNumberField.interaction.test.tsx#control-481c7d66573516a6 numeric text-entry mode.
  - Initial state: visibility=Visible on the mounted timeline editor surface subject to the exact handler guards.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["changed value","blur/keyboard event when present","current candidate-specific model state"]; handler={(e) => setDraft(e.target.value)} {commitEdit} {(e) => {
          if (e.key === "Enter") commitEdit();
          else if (e.key === "Escape") setEditing(false);
        }}.
  - Exact call/state/backend: stateTransition=numeric text-entry mode: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/ScrubbableNumberField.tsx:101::handler {(e) => setDraft(e.target.value)} {commitEdit} {(e) => {\n          if (e.key === \"Enter\") commitEdit();\n          else if (e.key === \"Escape\") setEditing(false);\n        }} -> onChange setDraft(e.target.value); blur or Enter parses suffix/decimal comma, clamps, and calls p.onCommit(clampedParsedValue) only when finite, then setEditing(false); Escape only setEditing(false)","web/src/components/inspector/ScrubbableNumberField.tsx::ScrubbableNumberField.commitEdit -> parent p.onCommit","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/inspector/ScrubbableNumberField.tsx#ScrubbableNumberField"].
  - Visible/accessibility/return path: success=numeric text-entry mode: assert exactly onChange setDraft(e.target.value); blur or Enter parses suffix/decimal comma, clamps, and calls p.onCommit(clampedParsedValue) only when finite, then setEditing(false); Escape only setEditing(false) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"numeric text-entry mode: assert exactly onChange setDraft(e.target.value); blur or Enter parses suffix/decimal comma, clamps, and calls p.onCommit(clampedParsedValue) only when finite, then setEditing(false); Escape only setEditing(false) and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Escape exits text mode without invoking p.onCommit.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

#### control-record-fb15dd12a9e06926

- Candidate/source: `control-3e4fc80f4dde046e` at `web/src/components/inspector/ScrubbableNumberField.tsx:126:5` (control)
- Expected behavior: pointer-scrubbable numeric value: assert exactly pointerdown captures start; pointermove computes clamped startValue + delta*sensitivity (Shift x10, Command x0.1) and calls p.onChange?.(next); pointerup calls p.onCommit(provisionalValue) exactly once when moved, otherwise enters text editing and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-3e4fc80f4dde046e.
  - Test: web/src/components/inspector/ScrubbableNumberField.interaction.test.tsx#control-3e4fc80f4dde046e pointer-scrubbable numeric value.
  - Initial state: visibility=Visible on the mounted timeline editor surface subject to the exact handler guards.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["pointer/drag event sequence","coordinates and modifiers","current editor state","pointerup/cancel boundary"]; handler={onPointerDown} {onPointerMove} {onPointerUp}.
  - Exact call/state/backend: stateTransition=pointer-scrubbable numeric value: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/ScrubbableNumberField.tsx:126::handler {onPointerDown} {onPointerMove} {onPointerUp} -> pointerdown captures start; pointermove computes clamped startValue + delta*sensitivity (Shift x10, Command x0.1) and calls p.onChange?.(next); pointerup calls p.onCommit(provisionalValue) exactly once when moved, otherwise enters text editing","web/src/components/inspector/ScrubbableNumberField.tsx::ScrubbableNumberField pointer handlers -> parent p.onChange/p.onCommit","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/inspector/ScrubbableNumberField.tsx#ScrubbableNumberField"].
  - Visible/accessibility/return path: success=pointer-scrubbable numeric value: assert exactly pointerdown captures start; pointermove computes clamped startValue + delta*sensitivity (Shift x10, Command x0.1) and calls p.onChange?.(next); pointerup calls p.onCommit(provisionalValue) exactly once when moved, otherwise enters text editing and no sibling branch/command.; accessibility={"focus":"Pointer-only span has no tabIndex or keyboard adjustment.","label":"No role or accessible name exists on the displayed-value span.","shortcut":"Shift/Command change pointer sensitivity only."}; returnPath=["Pointerup/cancel releases the gesture and remains on the same editor surface.","A keyboard equivalent must retain/restore focus; current custom drag surface does not prove one."].
  - Outcome matrix: {"success":"pointer-scrubbable numeric value: assert exactly pointerdown captures start; pointermove computes clamped startValue + delta*sensitivity (Shift x10, Command x0.1) and calls p.onChange?.(next); pointerup calls p.onCommit(provisionalValue) exactly once when moved, otherwise enters text editing and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Pointer capture loss/Escape cancellation is not implemented for the drag.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/ScrubbableNumberField.interaction.test.tsx#control-481c7d66573516a6 numeric text-entry mode` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/ScrubbableNumberField.interaction.test.tsx#control-3e4fc80f4dde046e pointer-scrubbable numeric value` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/ScrubbableNumberField.interaction.test.tsx -t "control-481c7d66573516a6 numeric text-entry mode"`
  - Run: `pnpm -C web test -- --run src/components/inspector/ScrubbableNumberField.interaction.test.tsx -t "control-3e4fc80f4dde046e pointer-scrubbable numeric value"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/ScrubbableNumberField.tsx`, `web/src/components/inspector/ScrubbableNumberField.tsx#ScrubbableNumberField` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/ScrubbableNumberField.interaction.test.tsx -t "control-481c7d66573516a6 numeric text-entry mode"`
  - Run: `pnpm -C web test -- --run src/components/inspector/ScrubbableNumberField.interaction.test.tsx -t "control-3e4fc80f4dde046e pointer-scrubbable numeric value"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: RED exposed missing post-edit focus restoration and
  pointer-cancellation cleanup. The two owners now prove suffix/decimal-comma
  parsing, finite-only clamped commit, invalid/blur and Escape behavior,
  thresholded live scrub, Shift/Command multipliers, exactly-once pointerup
  commit, pointercancel/lost-capture/Escape cancellation and focus retention.
  The full Web gate passes with 105 files / 850 tests and the production build
  passes, with only the existing dynamic-import and chunk-size advisories.

- [ ] **Runtime evidence gate:** exercise text entry, modifier scrubbing,
  pointer-capture loss and Escape cancellation in the packaged Inspector,
  confirming focus and visible values before final control reclassification.

### Task 16: control-acceptance (implementation-slice-aea2e7e06f8f518b)

**Covered records:**
- `control-record-ddec81c32f6fc580` (control)

**Files:**
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/preview/Preview.tsx#ScrubBar`
- Modify: `web/src/components/preview/Preview.tsx#Preview`
- Test (reviewed-planned): `web/src/components/preview/Preview.interaction.test.tsx#control-200c9fd6ec3f0f35 pointer scrub preview playhead`

**Candidate-bound contracts:**

#### control-record-ddec81c32f6fc580

- Candidate/source: `control-200c9fd6ec3f0f35` at `web/src/components/preview/Preview.tsx:724:5` (control)
- Expected behavior: pointer scrub preview playhead: assert exactly pointerdown sets capture, calls onScrubbingChange?.(true), then seekFromEvent(clientX) -> onSeek(Math.round(ratio * total)); pointermove with buttons===1 repeats seek; pointerup/lost capture calls onScrubbingChange?.(false); hover only changes local hover state and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-200c9fd6ec3f0f35.
  - Test: web/src/components/preview/Preview.interaction.test.tsx#control-200c9fd6ec3f0f35 pointer scrub preview playhead.
  - Initial state: visibility=Visible on the Preview surface; content and playback capability determine actionable branches.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["pointer/drag event sequence","coordinates and modifiers","current editor state","pointerup/cancel boundary"]; handler={() => setHover(true)} {() => setHover(false)} {(e) => {
        (e.target as HTMLElement).setPointerCapture(e.pointerId);
        onScrubbingChange?.(true);
        seekFromEvent(e.clientX);
      }} {(e) => {
        if (e.buttons === 1) seekFromEvent(e.clientX);
      }} {() => onScrubbingChange?.(false)} {() => onScrubbingChange?.(false)}.
  - Exact call/state/backend: stateTransition=pointer scrub preview playhead: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/Preview.tsx:724::handler {() => setHover(true)} {() => setHover(false)} {(e) => {\n        (e.target as HTMLElement).setPointerCapture(e.pointerId);\n        onScrubbingChange?.(true);\n        seekFromEvent(e.clientX);\n      }} {(e) => {\n        if (e.buttons === 1) seekFromEvent(e.clientX);\n      }} {() => onScrubbingChange?.(false)} {() => onScrubbingChange?.(false)} -> pointerdown sets capture, calls onScrubbingChange?.(true), then seekFromEvent(clientX) -> onSeek(Math.round(ratio * total)); pointermove with buttons===1 repeats seek; pointerup/lost capture calls onScrubbingChange?.(false); hover only changes local hover state","web/src/components/preview/Preview.tsx::ScrubBar.seekFromEvent -> parent seekTo and optional uiStore.setScrubbing","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/preview/Preview.tsx#Preview","code:web/src/components/preview/Preview.tsx#ScrubBar"].
  - Visible/accessibility/return path: success=pointer scrub preview playhead: assert exactly pointerdown sets capture, calls onScrubbingChange?.(true), then seekFromEvent(clientX) -> onSeek(Math.round(ratio * total)); pointermove with buttons===1 repeats seek; pointerup/lost capture calls onScrubbingChange?.(false); hover only changes local hover state and no sibling branch/command.; accessibility={"focus":"Scrub div has no role or tabIndex.","label":"No slider accessible name/value semantics exist.","shortcut":"No Arrow/Home/End seek handling exists."}; returnPath=["Pointerup/cancel releases the gesture and remains on the same editor surface.","A keyboard equivalent must retain/restore focus; current custom drag surface does not prove one."].
  - Outcome matrix: {"success":"pointer scrub preview playhead: assert exactly pointerdown sets capture, calls onScrubbingChange?.(true), then seekFromEvent(clientX) -> onSeek(Math.round(ratio * total)); pointermove with buttons===1 repeats seek; pointerup/lost capture calls onScrubbingChange?.(false); hover only changes local hover state and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Pointerup or lost capture clears scrubbing; no keyboard/Escape path exists.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/Preview.interaction.test.tsx#control-200c9fd6ec3f0f35 pointer scrub preview playhead` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-200c9fd6ec3f0f35 pointer scrub preview playhead"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/Preview.tsx`, `web/src/components/preview/Preview.tsx#ScrubBar`, `web/src/components/preview/Preview.tsx#Preview` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-200c9fd6ec3f0f35 pointer scrub preview playhead"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: RED exposed a real sticky-scrubbing defect: the cancel
  state transition intentionally has no seek effect, but the component also
  suppressed the required `scrubbing=false` notification. The owner now proves
  capture, down/move/exact-up seeks, buttons guard, hover-only state,
  lost-capture/pointercancel cleanup, horizontal slider semantics and
  Arrow/Home/End keyboard seeks with retained focus. The full Web gate passes
  with 106 files / 851 tests and the production build passes, with only the
  existing dynamic-import and chunk-size advisories.

- [ ] **Runtime evidence gate:** scrub the packaged Preview by pointer, force
  capture loss/cancel, hover the control and seek by keyboard while confirming
  playhead/focus/scrubbing state before final control reclassification.

### Task 17: control-acceptance (implementation-slice-c09938dfb4f83d4c)

**Covered records:**
- `control-record-337520407e26ed36` (control)

**Files:**
- Modify: `web/src/components/shell/SplitPane.tsx`
- Modify: `web/src/components/shell/SplitPane.tsx#SplitPane`
- Test (reviewed-planned): `web/src/components/shell/SplitPane.interaction.test.tsx#control-d88c7103e09bb382 resize two editor panes`

**Candidate-bound contracts:**

#### control-record-337520407e26ed36

- Candidate/source: `control-d88c7103e09bb382` at `web/src/components/shell/SplitPane.tsx:82:7` (control)
- Expected behavior: resize two editor panes: pointer capture; move clamps first pane to min and secondMin
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d88c7103e09bb382.
  - Test: web/src/components/shell/SplitPane.interaction.test.tsx#control-d88c7103e09bb382 resize two editor panes.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onPointerDown} {onPointerMove} {onPointerUp}","pointer/drag coordinates, button/key, and current owning state"]; handler={onPointerDown} {onPointerMove} {onPointerUp}.
  - Exact call/state/backend: stateTransition=pointer capture; move clamps first pane to min and secondMin; backendTrace=["web/src/components/shell/SplitPane.tsx:82::candidate handler -> {onPointerDown} {onPointerMove} {onPointerUp}","actual branch/state -> pointer capture; move clamps first pane to min and secondMin","exact call -> pointer capture; move clamps first pane to min and secondMin","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/SplitPane.tsx#SplitPane"].
  - Visible/accessibility/return path: success=resize two editor panes: pointer capture; move clamps first pane to min and secondMin; accessibility={"focus":"Non-focusable div","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Pointer capture releases on pointerup; no keyboard focus path exists."].
  - Outcome matrix: {"success":"resize two editor panes: pointer capture; move clamps first pane to min and secondMin","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/SplitPane.interaction.test.tsx#control-d88c7103e09bb382 resize two editor panes` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/shell/SplitPane.interaction.test.tsx -t "control-d88c7103e09bb382 resize two editor panes"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/SplitPane.tsx`, `web/src/components/shell/SplitPane.tsx#SplitPane` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/SplitPane.interaction.test.tsx -t "control-d88c7103e09bb382 resize two editor panes"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01: RED proved pointer cancellation left the separator in
  its dragging state. The owner now covers stable separator capture/release,
  horizontal and vertical geometry, both min clamps, pointerup/cancel/lost
  capture/Escape cleanup, semantic separator values and Home/End keyboard
  resize with retained focus. The full Web gate passes with 107 files / 852
  tests and the production build passes, with only the existing dynamic-import
  and chunk-size advisories.

- [ ] **Runtime evidence gate:** resize horizontal and vertical packaged panes
  by pointer and keyboard, force cancellation/capture loss and confirm clamps,
  cursor, focus and dragging state before final control reclassification.

### Task 18: control-acceptance (implementation-slice-30967cbef194812a)

**Covered records:**
- `control-record-f4b0b6a427b2339a` (control)
- `control-record-f0588991f2ea1a3a` (control)
- `control-record-72dcda9f7e21364d` (control)

**Files:**
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx`
- Modify: `web/src/store/editActions.ts#setTrackProps`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetTrackProps`
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-4c72d4f81e47c57d mute audio track`
- Test (reviewed-planned): `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-74289f5806f8162a hide visual track`
- Test (reviewed-planned): `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-71e7fa7fcd6aa730 toggle track sync lock`

**Candidate-bound contracts:**

#### control-record-f4b0b6a427b2339a

- Candidate/source: `control-4c72d4f81e47c57d` at `web/src/components/timeline/TrackHeaderColumn.tsx:197:11` (control)
- Expected behavior: mute audio track: assert exactly setTrackProps(p.index, { muted: !p.muted }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-4c72d4f81e47c57d.
  - Test: web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-4c72d4f81e47c57d mute audio track.
  - Initial state: visibility=Visible for each timeline track; reorder items require an adjacent same-type track.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(e) => e.stopPropagation()} {() => void setTrackProps(p.index, { muted: !p.muted })}.
  - Exact call/state/backend: stateTransition=mute audio track: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TrackHeaderColumn.tsx:197::handler {(e) => e.stopPropagation()} {() => void setTrackProps(p.index, { muted: !p.muted })} -> setTrackProps(p.index, { muted: !p.muted })","web/src/store/editActions.ts::setTrackProps(index,{muted:!muted})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetTrackProps","code:web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn","code:web/src/store/editActions.ts#setTrackProps","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=mute audio track: assert exactly setTrackProps(p.index, { muted: !p.muted }) and no sibling branch/command.; accessibility={"focus":"role=button span has no tabIndex and is not keyboard-focusable.","label":"title supplies text but aria-pressed is absent.","shortcut":"No Enter/Space handler exists."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"mute audio track: assert exactly setTrackProps(p.index, { muted: !p.muted }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-f0588991f2ea1a3a

- Candidate/source: `control-74289f5806f8162a` at `web/src/components/timeline/TrackHeaderColumn.tsx:207:11` (control)
- Expected behavior: hide visual track: assert exactly setTrackProps(p.index, { hidden: !p.hidden }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-74289f5806f8162a.
  - Test: web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-74289f5806f8162a hide visual track.
  - Initial state: visibility=Visible for each timeline track; reorder items require an adjacent same-type track.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(e) => e.stopPropagation()} {() => void setTrackProps(p.index, { hidden: !p.hidden })}.
  - Exact call/state/backend: stateTransition=hide visual track: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TrackHeaderColumn.tsx:207::handler {(e) => e.stopPropagation()} {() => void setTrackProps(p.index, { hidden: !p.hidden })} -> setTrackProps(p.index, { hidden: !p.hidden })","web/src/store/editActions.ts::setTrackProps(index,{hidden:!hidden})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetTrackProps","code:web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn","code:web/src/store/editActions.ts#setTrackProps","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=hide visual track: assert exactly setTrackProps(p.index, { hidden: !p.hidden }) and no sibling branch/command.; accessibility={"focus":"role=button span has no tabIndex and is not keyboard-focusable.","label":"title supplies text but aria-pressed is absent.","shortcut":"No Enter/Space handler exists."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"hide visual track: assert exactly setTrackProps(p.index, { hidden: !p.hidden }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-72dcda9f7e21364d

- Candidate/source: `control-71e7fa7fcd6aa730` at `web/src/components/timeline/TrackHeaderColumn.tsx:217:9` (control)
- Expected behavior: toggle track sync lock: assert exactly setTrackProps(p.index, { syncLocked: !p.syncLocked }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-71e7fa7fcd6aa730.
  - Test: web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-71e7fa7fcd6aa730 toggle track sync lock.
  - Initial state: visibility=Visible for each timeline track; reorder items require an adjacent same-type track.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(e) => e.stopPropagation()} {() => void setTrackProps(p.index, { syncLocked: !p.syncLocked })}.
  - Exact call/state/backend: stateTransition=toggle track sync lock: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TrackHeaderColumn.tsx:217::handler {(e) => e.stopPropagation()} {() => void setTrackProps(p.index, { syncLocked: !p.syncLocked })} -> setTrackProps(p.index, { syncLocked: !p.syncLocked })","web/src/store/editActions.ts::setTrackProps(index,{syncLocked:!syncLocked})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetTrackProps","code:web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn","code:web/src/store/editActions.ts#setTrackProps","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=toggle track sync lock: assert exactly setTrackProps(p.index, { syncLocked: !p.syncLocked }) and no sibling branch/command.; accessibility={"focus":"role=button span has no tabIndex and is not keyboard-focusable.","label":"title supplies text but aria-pressed is absent.","shortcut":"No Enter/Space handler exists."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"toggle track sync lock: assert exactly setTrackProps(p.index, { syncLocked: !p.syncLocked }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-4c72d4f81e47c57d mute audio track` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-74289f5806f8162a hide visual track` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-71e7fa7fcd6aa730 toggle track sync lock` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-4c72d4f81e47c57d mute audio track"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-74289f5806f8162a hide visual track"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-71e7fa7fcd6aa730 toggle track sync lock"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TrackHeaderColumn.tsx`, `web/src/store/editActions.ts#setTrackProps`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetTrackProps`, `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-4c72d4f81e47c57d mute audio track"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-74289f5806f8162a hide visual track"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-71e7fa7fcd6aa730 toggle track sync lock"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 19: control-acceptance (implementation-slice-d5ef678a62684383)

**Covered records:**
- `control-record-6d9e2d3db1fc08f5` (control)

**Files:**
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx`
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderRow`
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn`
- Test (reviewed-planned): `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-9f9173ff2ee37464 resize track display height`

**Candidate-bound contracts:**

#### control-record-6d9e2d3db1fc08f5

- Candidate/source: `control-9f9173ff2ee37464` at `web/src/components/timeline/TrackHeaderColumn.tsx:245:7` (control)
- Expected behavior: resize track display height: assert exactly pointerdown captures startY; pointermove calls p.onResize(delta), whose wrapper computes clamp(h + delta, TRACK_SIZE.minHeight, TRACK_SIZE.maxHeight) then setTrackHeight(track.id,next); pointerup releases capture and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-9f9173ff2ee37464.
  - Test: web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-9f9173ff2ee37464 resize track display height.
  - Initial state: visibility=Visible for each timeline track; reorder items require an adjacent same-type track.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["pointer/drag event sequence","coordinates and modifiers","current editor state","pointerup/cancel boundary"]; handler={onPointerDown} {onPointerMove} {onPointerUp}.
  - Exact call/state/backend: stateTransition=resize track display height: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TrackHeaderColumn.tsx:245::handler {onPointerDown} {onPointerMove} {onPointerUp} -> pointerdown captures startY; pointermove calls p.onResize(delta), whose wrapper computes clamp(h + delta, TRACK_SIZE.minHeight, TRACK_SIZE.maxHeight) then setTrackHeight(track.id,next); pointerup releases capture","web/src/components/timeline/TrackHeaderColumn.tsx::TrackHeaderRow resize -> uiStore.setTrackHeight","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn","code:web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderRow"].
  - Visible/accessibility/return path: success=resize track display height: assert exactly pointerdown captures startY; pointermove calls p.onResize(delta), whose wrapper computes clamp(h + delta, TRACK_SIZE.minHeight, TRACK_SIZE.maxHeight) then setTrackHeight(track.id,next); pointerup releases capture and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Pointerup/cancel releases the gesture and remains on the same editor surface.","A keyboard equivalent must retain/restore focus; current custom drag surface does not prove one."].
  - Outcome matrix: {"success":"resize track display height: assert exactly pointerdown captures startY; pointermove calls p.onResize(delta), whose wrapper computes clamp(h + delta, TRACK_SIZE.minHeight, TRACK_SIZE.maxHeight) then setTrackHeight(track.id,next); pointerup releases capture and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Pointerup/cancel follows the exact profile; Escape rollback is not otherwise implemented.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-9f9173ff2ee37464 resize track display height` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-9f9173ff2ee37464 resize track display height"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TrackHeaderColumn.tsx`, `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderRow`, `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-9f9173ff2ee37464 resize track display height"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 20: control-acceptance (implementation-slice-a9b2786eddf478dc)

**Covered records:**
- `control-record-370c657212c24842` (control)

**Files:**
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx`
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderContextMenu`
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn`
- Test (reviewed-planned): `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-db7fbb7edbcca44d dismiss track reorder menu`

**Candidate-bound contracts:**

#### control-record-370c657212c24842

- Candidate/source: `control-db7fbb7edbcca44d` at `web/src/components/timeline/TrackHeaderColumn.tsx:286:5` (control)
- Expected behavior: dismiss track reorder menu: assert exactly backdrop mousedown calls onClose(); contextmenu preventDefault() then onClose() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-db7fbb7edbcca44d.
  - Test: web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-db7fbb7edbcca44d dismiss track reorder menu.
  - Initial state: visibility=Visible for each timeline track; reorder items require an adjacent same-type track.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={onClose} {(e) => {
        e.preventDefault();
        onClose();
      }}.
  - Exact call/state/backend: stateTransition=dismiss track reorder menu: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TrackHeaderColumn.tsx:286::handler {onClose} {(e) => {\n        e.preventDefault();\n        onClose();\n      }} -> backdrop mousedown calls onClose(); contextmenu preventDefault() then onClose()","web/src/components/timeline/TrackHeaderColumn.tsx::TrackHeaderContextMenu backdrop -> setMenu(null)","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn","code:web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderContextMenu"].
  - Visible/accessibility/return path: success=dismiss track reorder menu: assert exactly backdrop mousedown calls onClose(); contextmenu preventDefault() then onClose() and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"dismiss track reorder menu: assert exactly backdrop mousedown calls onClose(); contextmenu preventDefault() then onClose() and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-db7fbb7edbcca44d dismiss track reorder menu` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-db7fbb7edbcca44d dismiss track reorder menu"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TrackHeaderColumn.tsx`, `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderContextMenu`, `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-db7fbb7edbcca44d dismiss track reorder menu"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 21: control-acceptance (implementation-slice-440d530594b95cd0)

**Covered records:**
- `control-record-5b9e0c6b2a86effb` (control)

**Files:**
- Modify: `web/src/components/ui/Dropdown.tsx`
- Modify: `web/src/components/ui/Dropdown.tsx#Dropdown`
- Test (reviewed-planned): `web/src/components/ui/Dropdown.interaction.test.tsx#control-f1370db38b24cf33 open/close a reusable enum Dropdown`

**Candidate-bound contracts:**

#### control-record-5b9e0c6b2a86effb

- Candidate/source: `control-f1370db38b24cf33` at `web/src/components/ui/Dropdown.tsx:62:7` (control)
- Expected behavior: open/close a reusable enum Dropdown: setOpen toggles; outside/Escape closes
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-f1370db38b24cf33.
  - Test: web/src/components/ui/Dropdown.interaction.test.tsx#control-f1370db38b24cf33 open/close a reusable enum Dropdown.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setOpen((v) => !v)}","click or native keyboard activation plus current owning state"]; handler={() => setOpen((v) => !v)}.
  - Exact call/state/backend: stateTransition=setOpen toggles; outside/Escape closes; backendTrace=["web/src/components/ui/Dropdown.tsx:62::candidate handler -> {() => setOpen((v) => !v)}","actual branch/state -> setOpen toggles; outside/Escape closes","exact call -> setOpen toggles; outside/Escape closes","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/ui/Dropdown.tsx#Dropdown"].
  - Visible/accessibility/return path: success=open/close a reusable enum Dropdown: setOpen toggles; outside/Escape closes; accessibility={"focus":"Native keyboard-focusable control","label":"ariaLabel","shortcut":"None declared on this control"}; returnPath=["Outside click/Escape closes visually; DOM focus is not explicitly restored."].
  - Outcome matrix: {"success":"open/close a reusable enum Dropdown: setOpen toggles; outside/Escape closes","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setOpen toggles; outside/Escape closes; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/ui/Dropdown.interaction.test.tsx#control-f1370db38b24cf33 open/close a reusable enum Dropdown` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/ui/Dropdown.interaction.test.tsx -t "control-f1370db38b24cf33 open/close a reusable enum Dropdown"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/ui/Dropdown.tsx`, `web/src/components/ui/Dropdown.tsx#Dropdown` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/ui/Dropdown.interaction.test.tsx -t "control-f1370db38b24cf33 open/close a reusable enum Dropdown"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 22: control-acceptance (implementation-slice-ba029e02db838d36)

**Covered records:**
- `control-record-f580ef2f93663b3e` (control)

**Files:**
- Modify: `web/src/components/ui/PanelShell.tsx`
- Modify: `web/src/components/ui/PanelShell.tsx#PanelShell`
- Test (reviewed-planned): `web/src/components/ui/PanelShell.interaction.test.tsx#control-bbc125bbbf2275f2 focus an editor panel`

**Candidate-bound contracts:**

#### control-record-f580ef2f93663b3e

- Candidate/source: `control-bbc125bbbf2275f2` at `web/src/components/ui/PanelShell.tsx:22:5` (control)
- Expected behavior: focus an editor panel: onMouseDown -> focusPanel(panel) -> focus ring
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-bbc125bbbf2275f2.
  - Test: web/src/components/ui/PanelShell.interaction.test.tsx#control-bbc125bbbf2275f2 focus an editor panel.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => focusPanel(panel)}","pointer/drag coordinates, button/key, and current owning state"]; handler={() => focusPanel(panel)}.
  - Exact call/state/backend: stateTransition=onMouseDown -> focusPanel(panel) -> focus ring; backendTrace=["web/src/components/ui/PanelShell.tsx:22::candidate handler -> {() => focusPanel(panel)}","actual branch/state -> onMouseDown -> focusPanel(panel) -> focus ring","exact call -> onMouseDown -> focusPanel(panel) -> focus ring","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/ui/PanelShell.tsx#PanelShell"].
  - Visible/accessibility/return path: success=focus an editor panel: onMouseDown -> focusPanel(panel) -> focus ring; accessibility={"focus":"Outer div is not keyboard focusable","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Focus state remains on the selected panel until another panel is clicked/shortcut-selected."].
  - Outcome matrix: {"success":"focus an editor panel: onMouseDown -> focusPanel(panel) -> focus ring","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/ui/PanelShell.interaction.test.tsx#control-bbc125bbbf2275f2 focus an editor panel` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/ui/PanelShell.interaction.test.tsx -t "control-bbc125bbbf2275f2 focus an editor panel"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/ui/PanelShell.tsx`, `web/src/components/ui/PanelShell.tsx#PanelShell` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/ui/PanelShell.interaction.test.tsx -t "control-bbc125bbbf2275f2 focus an editor panel"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.
