# Inspector Text Keyframes Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 84 verified incomplete records in the `inspector-text-keyframes` gap group.

**Architecture:** Implement 28 primary evidence-bound slices and reference 0 shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.

---

### Task 1: IN-inspector-text-ai-keyframe-surface + inspector-component-surface (implementation-slice-80a795528c39094d)

**Covered records:**
- `requirement-c9c677ffc0cea514` (requirement)
- `requirement-55e9cb9d202147e9` (requirement)
- `requirement-f7024f37d8c971b6` (requirement)
- `requirement-9bdc97e09465edfc` (requirement)
- `requirement-6aba9217bbeca297` (requirement)
- `requirement-e419b88df9110885` (requirement)
- `requirement-c0129772e5592014` (requirement)
- `requirement-6e37ac641ccf5107` (requirement)
- `requirement-33b669f5ab381ec8` (requirement)
- `requirement-5215757bd00518c6` (requirement)
- `requirement-1cd16e183b6c6b7e` (requirement)
- `requirement-3c7ac4a40d6f21a5` (requirement)
- `requirement-c13354b986564337` (requirement)
- `requirement-8c8c904054874383` (requirement)
- `requirement-86e3768c2d581515` (requirement)

**Files:**
- Modify: `web/src/components/inspector/AiEditTab.tsx#AiEditTab`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`
- Modify: `web/src/components/inspector/KeyframesPanel.tsx#KeyframesPanel`
- Modify: `web/src/components/inspector/KeyframesRuler.tsx#KeyframesRuler`
- Modify: `web/src/components/inspector/ScrubbableNumberField.tsx#ScrubbableNumberField`
- Modify: `web/src/components/inspector/TextTab.tsx#TextTab`
- Modify: `docs/architecture/BUGS.md`
- Modify: `docs/architecture/HANDOFF-2026-07.md`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Modify: `docs/specs/frontend/6-inspector.md`
- Modify: `docs/specs/frontend/9-interactions.md`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.test.tsx#four_states_tabs_fields_and_lanes`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.test.tsx#selection_tabs_text_ai_keyframe_and_razor_matrix`
- Test (reviewed-planned): `web/src/components/inspector/ScrubbableNumberField.test.tsx#drag_commit_cancel`
- Test (existing-owned): `web/src/lib/keyframeNav.test.ts#maps clip-relative offsets to ABSOLUTE timeline frames via startFrame`
- Test (existing-owned): `web/src/lib/keyframeNav.test.ts#previousKeyframeFrame / nextKeyframeFrame`
- Test (existing-owned): `web/src/lib/keyframeSnap.test.ts#snapFrame`
- Test (existing-owned): `web/src/lib/keyframeValue.test.ts#scaleKeyframeValue`

**Candidate-bound contracts:**

#### requirement-c9c677ffc0cea514

- Candidate/source: `doc-416d69dd4e84ab7d` at `docs/architecture/BUGS.md:83` (requirement)
- Expected behavior: Inspector text editing and AI Edit are complete user-facing workflows.
- Resolution: `validated-ledger-evidence:IN-inspector-text-ai-keyframe-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Implement AI Edit suggestion generation in the Inspector.
  - Apply accepted suggestions through the shared command layer with undo/redo.
  - Add component and command integration tests for success, rejection, and undo.

#### requirement-55e9cb9d202147e9

- Candidate/source: `doc-622bc358e0f4dc11` at `docs/architecture/HANDOFF-2026-07.md:161` (requirement)
- Expected behavior: AI Edit and Music tabs provide working, undoable editing workflows.
- Resolution: `validated-ledger-evidence:IN-inspector-text-ai-keyframe-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Implement AI Edit proposal/apply/undo in Inspector.
  - Implement Music browse/import/place in MediaPanel.
  - Add UI-to-command integration tests for both tabs.

#### requirement-f7024f37d8c971b6

- Candidate/source: `doc-1a263224cb43653f` at `docs/modules/web/SPEC.md:1267` (requirement)
- Expected behavior: Match all Inspector selection states, tabs, fields, AI affordance, and keyframe lanes to upstream.
- Resolution: `validated-ledger-evidence:IN-inspector-text-ai-keyframe-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Create a pinned upstream/current screenshot-state matrix at the same viewport, scale, locale, theme, and fixture.
  - Add deterministic component/browser assertions for normal, hover, focus, disabled, selected, empty, error, and compact states.
  - Record reviewed pixel/geometry evidence for every item named by the checklist, with no unchecked item remaining.

#### requirement-9bdc97e09465edfc

- Candidate/source: `doc-f1f204e49f21c520` at `docs/specs/frontend/13-implementation.md:24` (requirement)
- Expected behavior: Match Inspector four states, tabs, fields, AI Edit, and keyframe lanes.
- Resolution: `reviewed-mapping-report:inspector-component-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Implementation: Implement all four Inspector states and every specified field/tab including AI Edit, then add edit/undo/keyframe and visual parity tests.
  - Add command, undo/redo, serialization, sampling, and Inspector interaction tests for every named field, keyframe, text, or font behavior; the affected Rust and web suites must pass.
  - Exercise the named Inspector or timeline editing path end to end and retain exact visible-result or runtime evidence before reclassification.

#### requirement-6aba9217bbeca297

- Candidate/source: `doc-3b5a0db034e8d137` at `docs/specs/frontend/13-implementation.md:32` (requirement)
- Expected behavior: Satisfy all eight razor/split/keyframe/fade/volume behaviors in frontend spec 9.4.
- Resolution: `reviewed-mapping-report:inspector-component-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Implementation: Add one named interaction/command test per row, cover linked clips and boundary frames, and verify Inspector/timeline state after undo/redo.
  - Add command, undo/redo, serialization, sampling, and Inspector interaction tests for every named field, keyframe, text, or font behavior; the affected Rust and web suites must pass.
  - Exercise the named Inspector or timeline editing path end to end and retain exact visible-result or runtime evidence before reclassification.

#### requirement-e419b88df9110885

- Candidate/source: `doc-94615219b098e1e7` at `docs/specs/frontend/6-inspector.md:1` (requirement)
- Expected behavior: Inspector tabs include complete text/AI Edit and keyframe workflows backed by undoable commands.
- Resolution: `validated-ledger-evidence:IN-inspector-text-ai-keyframe-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Complete project and clip Inspector tabs, including Text, AI Edit, Video, Audio, keyframes, and utility controls with selection-aware empty/multi states.
  - All domain changes must use command-backed actions, update preview, persist, and support the specified undo/redo grouping.
  - Test project/no selection/single/multi/offline states, every tab, AI proposal accept/reject, keyframe edits, save/reopen, and preview/export value parity.

#### requirement-c0129772e5592014

- Candidate/source: `doc-0891c6f16cac94e1` at `docs/specs/frontend/6-inspector.md:5` (requirement)
- Expected behavior: At docs/specs/frontend/6-inspector.md:5 under “6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）” (heading), the source “### 6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）” requires this exact behavior: The named Inspector structure/control is implemented and covered by component tests.
- Resolution: `reviewed-mapping-report:inspector-component-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/6-inspector.md:5; signal=heading; heading=6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）; candidate=### 6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）
  - Expected behavior: The named Inspector structure/control is implemented and covered by component tests. This closes only the promise expressed by “6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）” in “6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）” with the scenario below and register test:web/src/__tests__/completion/doc-0891c6f16cac94e1.test.ts#completion_0891c6f16cac94e1_the_named_inspector_structure_control_is_impleme
  - Initial state/input/event: render the exact “6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The named Inspector structure/control is implemented and covered by component tests.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-0891c6f16cac94e1.test.ts#completion_0891c6f16cac94e1_the_named_inspector_structure_control_is_impleme.

#### requirement-6e37ac641ccf5107

- Candidate/source: `doc-494e8df7adc0dd74` at `docs/specs/frontend/6-inspector.md:18` (requirement)
- Expected behavior: At docs/specs/frontend/6-inspector.md:18 under “6.2 工程元数据态（`projectMetadataContent`，`95-161`）” (heading), the source “### 6.2 工程元数据态（`projectMetadataContent`，`95-161`）” requires this exact behavior: The named Inspector structure/control is implemented and covered by component tests.
- Resolution: `reviewed-mapping-report:inspector-component-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/6-inspector.md:18; signal=heading; heading=6.2 工程元数据态（`projectMetadataContent`，`95-161`）; candidate=### 6.2 工程元数据态（`projectMetadataContent`，`95-161`）
  - Expected behavior: The named Inspector structure/control is implemented and covered by component tests. This closes only the promise expressed by “6.2 工程元数据态（`projectMetadataContent`，`95-161`）” in “6.2 工程元数据态（`projectMetadataContent`，`95-161`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.2 工程元数据态（`projectMetadataContent`，`95-161`）” with the scenario below and register test:web/src/__tests__/completion/doc-494e8df7adc0dd74.test.ts#completion_494e8df7adc0dd74_the_named_inspector_structure_control_is_impleme
  - Initial state/input/event: render the exact “6.2 工程元数据态（`projectMetadataContent`，`95-161`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “6.2 工程元数据态（`projectMetadataContent`，`95-161`）”.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, have the React handler call the typed store/API/Tauri action for “The named Inspector structure/control is implemented and covered by component tests.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-494e8df7adc0dd74.test.ts#completion_494e8df7adc0dd74_the_named_inspector_structure_control_is_impleme.

#### requirement-33b669f5ab381ec8

- Candidate/source: `doc-500cb7c987a6566c` at `docs/specs/frontend/6-inspector.md:24` (requirement)
- Expected behavior: Inspector tabs include complete text/AI Edit and keyframe workflows backed by undoable commands.
- Resolution: `validated-ledger-evidence:IN-inspector-text-ai-keyframe-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Expose the exact tab set for the selected clip kind, preserve tab state safely across selection changes, and show typed unavailable states instead of scaffold content.
  - Implement Text and AI Edit proposal/review/apply plus Video/Audio changes through shared commands with one undo entry per accepted edit.
  - Test video/audio/image/text/multi/offline selection, tab switching, AI success/failure/cancel/reject/apply, undo/redo, and save/reopen.

#### requirement-5215757bd00518c6

- Candidate/source: `doc-b8715cdf701fa467` at `docs/specs/frontend/6-inspector.md:37` (requirement)
- Expected behavior: At docs/specs/frontend/6-inspector.md:37 under “6.4 Video tab（`videoTabContent` `285-311`）” (heading), the source “### 6.4 Video tab（`videoTabContent` `285-311`）” requires this exact behavior: The named Inspector structure/control is implemented and covered by component tests.
- Resolution: `reviewed-mapping-report:inspector-component-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/6-inspector.md:37; signal=heading; heading=6.4 Video tab（`videoTabContent` `285-311`）; candidate=### 6.4 Video tab（`videoTabContent` `285-311`）
  - Expected behavior: The named Inspector structure/control is implemented and covered by component tests. This closes only the promise expressed by “6.4 Video tab（`videoTabContent` `285-311`）” in “6.4 Video tab（`videoTabContent` `285-311`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.4 Video tab（`videoTabContent` `285-311`）” with the scenario below and register test:web/src/__tests__/completion/doc-b8715cdf701fa467.test.ts#completion_b8715cdf701fa467_the_named_inspector_structure_control_is_impleme
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “6.4 Video tab（`videoTabContent` `285-311`）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named Inspector structure/control is implemented and covered by component tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-b8715cdf701fa467.test.ts#completion_b8715cdf701fa467_the_named_inspector_structure_control_is_impleme.

#### requirement-1cd16e183b6c6b7e

- Candidate/source: `doc-8b39dc07174d3662` at `docs/specs/frontend/6-inspector.md:56` (requirement)
- Expected behavior: At docs/specs/frontend/6-inspector.md:56 under “6.5 Audio tab（`audioTabContent` `338-383`）” (heading), the source “### 6.5 Audio tab（`audioTabContent` `338-383`）” requires this exact behavior: The named Inspector structure/control is implemented and covered by component tests.
- Resolution: `reviewed-mapping-report:inspector-component-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/6-inspector.md:56; signal=heading; heading=6.5 Audio tab（`audioTabContent` `338-383`）; candidate=### 6.5 Audio tab（`audioTabContent` `338-383`）
  - Expected behavior: The named Inspector structure/control is implemented and covered by component tests. This closes only the promise expressed by “6.5 Audio tab（`audioTabContent` `338-383`）” in “6.5 Audio tab（`audioTabContent` `338-383`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.5 Audio tab（`audioTabContent` `338-383`）” with the scenario below and register test:web/src/__tests__/completion/doc-8b39dc07174d3662.test.ts#completion_8b39dc07174d3662_the_named_inspector_structure_control_is_impleme
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “6.5 Audio tab（`audioTabContent` `338-383`）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named Inspector structure/control is implemented and covered by component tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-8b39dc07174d3662.test.ts#completion_8b39dc07174d3662_the_named_inspector_structure_control_is_impleme.

#### requirement-3c7ac4a40d6f21a5

- Candidate/source: `doc-16d02e079820e864` at `docs/specs/frontend/6-inspector.md:63` (requirement)
- Expected behavior: At docs/specs/frontend/6-inspector.md:63 under “6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件” (heading), the source “### 6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件” requires this exact behavior: The named Inspector structure/control is implemented and covered by component tests.
- Resolution: `reviewed-mapping-report:inspector-component-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/6-inspector.md:63; signal=heading; heading=6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件; candidate=### 6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件
  - Expected behavior: The named Inspector structure/control is implemented and covered by component tests. This closes only the promise expressed by “6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件” in “6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件” with the scenario below and register test:web/src/__tests__/completion/doc-16d02e079820e864.test.ts#completion_16d02e079820e864_the_named_inspector_structure_control_is_impleme
  - Initial state/input/event: render the exact “6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The named Inspector structure/control is implemented and covered by component tests.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-16d02e079820e864.test.ts#completion_16d02e079820e864_the_named_inspector_structure_control_is_impleme.

#### requirement-c13354b986564337

- Candidate/source: `doc-cb319383618328cd` at `docs/specs/frontend/6-inspector.md:73` (requirement)
- Expected behavior: At docs/specs/frontend/6-inspector.md:73 under “6.7 关键帧面板/泳道（`Keyframes/KeyframesLane.swift`）” (heading), the source “### 6.7 关键帧面板/泳道（`Keyframes/KeyframesLane.swift`）” requires this exact behavior: The named Inspector structure/control is implemented and covered by component tests.
- Resolution: `reviewed-mapping-report:inspector-component-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/6-inspector.md:73; signal=heading; heading=6.7 关键帧面板/泳道（`Keyframes/KeyframesLane.swift`）; candidate=### 6.7 关键帧面板/泳道（`Keyframes/KeyframesLane.swift`）
  - Expected behavior: The named Inspector structure/control is implemented and covered by component tests. This closes only the promise expressed by “6.7 关键帧面板/泳道（`Keyframes/KeyframesLane.swift`）” in “6.7 关键帧面板/泳道（`Keyframes/KeyframesLane.swift`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.7 关键帧面板/泳道（`Keyframes/KeyframesLane.swift`）” with the scenario below and register test:web/src/__tests__/completion/doc-cb319383618328cd.test.ts#completion_cb319383618328cd_the_named_inspector_structure_control_is_impleme
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “6.7 关键帧面板/泳道（`Keyframes/KeyframesLane.swift`）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named Inspector structure/control is implemented and covered by component tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-cb319383618328cd.test.ts#completion_cb319383618328cd_the_named_inspector_structure_control_is_impleme.

#### requirement-8c8c904054874383

- Candidate/source: `doc-34b7b5e9aaa2b273` at `docs/specs/frontend/6-inspector.md:79` (requirement)
- Expected behavior: At docs/specs/frontend/6-inspector.md:79 under “6.8 小组件” (heading), the source “### 6.8 小组件” requires this exact behavior: The named Inspector structure/control is implemented and covered by component tests.
- Resolution: `reviewed-mapping-report:inspector-component-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/6-inspector.md:79; signal=heading; heading=6.8 小组件; candidate=### 6.8 小组件
  - Expected behavior: The named Inspector structure/control is implemented and covered by component tests. This closes only the promise expressed by “6.8 小组件” in “6.8 小组件”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.8 小组件” with the scenario below and register test:web/src/__tests__/completion/doc-34b7b5e9aaa2b273.test.ts#completion_34b7b5e9aaa2b273_the_named_inspector_structure_control_is_impleme
  - Initial state/input/event: render the exact “6.8 小组件” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “6.8 小组件”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The named Inspector structure/control is implemented and covered by component tests.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-34b7b5e9aaa2b273.test.ts#completion_34b7b5e9aaa2b273_the_named_inspector_structure_control_is_impleme.

#### requirement-86e3768c2d581515

- Candidate/source: `doc-31b90d4d2b8b2e01` at `docs/specs/frontend/9-interactions.md:42` (requirement)
- Expected behavior: Complete the exhaustive 9.4 Razor / Split / 关键帧 / 淡变 / 音量 interaction matrix with parity across mouse, trackpad, and keyboard inputs.
- Resolution: `validated-ledger-evidence:IN-inspector-text-ai-keyframe-surface` — Both slices cover the Inspector text/keyframe component surface and share Inspector, KeyframesPanel and their focused component/navigation tests.
- Exact acceptance contract:
  - Razor/split, keyframe add/move/delete, fade handles, and volume edits use typed commands and maintain valid frame/value ranges.
  - Each gesture commits one undo step and preserves linked media plus keyframe ordering when clips are trimmed, split, or moved.
  - Test boundary splits, duplicate-frame keyframes, fade overlap, dB limits, cancel, undo/redo, save/reopen, and preview/export values.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.test.tsx#four_states_tabs_fields_and_lanes` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/components/inspector/Inspector.test.tsx#selection_tabs_text_ai_keyframe_and_razor_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/components/inspector/ScrubbableNumberField.test.tsx#drag_commit_cancel` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/keyframeNav.test.ts#maps clip-relative offsets to ABSOLUTE timeline frames via startFrame` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/keyframeNav.test.ts#previousKeyframeFrame / nextKeyframeFrame` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/keyframeSnap.test.ts#snapFrame` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/keyframeValue.test.ts#scaleKeyframeValue` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.test.tsx -t "four_states_tabs_fields_and_lanes"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.test.tsx -t "selection_tabs_text_ai_keyframe_and_razor_matrix"`
  - Run: `pnpm -C web test -- --run src/components/inspector/ScrubbableNumberField.test.tsx -t "drag_commit_cancel"`
  - Run: `pnpm -C web test -- --run src/lib/keyframeNav.test.ts -t "maps clip-relative offsets to ABSOLUTE timeline frames via startFrame"`
  - Run: `pnpm -C web test -- --run src/lib/keyframeNav.test.ts -t "previousKeyframeFrame / nextKeyframeFrame"`
  - Run: `pnpm -C web test -- --run src/lib/keyframeSnap.test.ts -t "snapFrame"`
  - Run: `pnpm -C web test -- --run src/lib/keyframeValue.test.ts -t "scaleKeyframeValue"`

  Expected: FAIL because one or more of the 15 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/AiEditTab.tsx#AiEditTab`, `web/src/components/inspector/Inspector.tsx#Inspector`, `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`, `web/src/components/inspector/KeyframesPanel.tsx#KeyframesPanel`, `web/src/components/inspector/KeyframesRuler.tsx#KeyframesRuler`, `web/src/components/inspector/ScrubbableNumberField.tsx#ScrubbableNumberField`, `web/src/components/inspector/TextTab.tsx#TextTab`, `docs/architecture/BUGS.md`, `docs/architecture/HANDOFF-2026-07.md`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/13-implementation.md`, `docs/specs/frontend/6-inspector.md`, `docs/specs/frontend/9-interactions.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.test.tsx -t "four_states_tabs_fields_and_lanes"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.test.tsx -t "selection_tabs_text_ai_keyframe_and_razor_matrix"`
  - Run: `pnpm -C web test -- --run src/components/inspector/ScrubbableNumberField.test.tsx -t "drag_commit_cancel"`
  - Run: `pnpm -C web test -- --run src/lib/keyframeNav.test.ts -t "maps clip-relative offsets to ABSOLUTE timeline frames via startFrame"`
  - Run: `pnpm -C web test -- --run src/lib/keyframeNav.test.ts -t "previousKeyframeFrame / nextKeyframeFrame"`
  - Run: `pnpm -C web test -- --run src/lib/keyframeSnap.test.ts -t "snapFrame"`
  - Run: `pnpm -C web test -- --run src/lib/keyframeValue.test.ts -t "scaleKeyframeValue"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 2: nonlinear-speed-curves (implementation-slice-819e7573ee4b179a)

**Covered records:**
- `requirement-751e3573915c4311` (requirement)
- `requirement-96ebb3ccd95b7798` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/clip.rs#Clip::speed`
- Modify: `crates/opentake-render/src/plan/build.rs#source_frame_index`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Modify: `docs/architecture/HANDOFF-2026-07.md`
- Test (existing-owned): `crates/opentake-render/src/plan/tests.rs#source_frame_video_with_trim_and_speed`
- Test (reviewed-planned): `crates/opentake-domain/tests/speed_curve.rs#nonlinear_speed_curve_roundtrips_and_maps_source_frames`

**Candidate-bound contracts:**

#### requirement-751e3573915c4311

- Candidate/source: `doc-d1b269f710d4dfdb` at `docs/architecture/CAPCUT-GAP.md:33` (requirement)
- Expected behavior: Clips support editable nonlinear speed curves in preview and export.
- Resolution: `reviewed-mapping-report:nonlinear-speed-curves` — Tracked owners implement a single constant speed only; no editable nonlinear curve model or preview/export parity exists.
- Exact acceptance contract:
  - Persist ordered speed-curve control points with validated positive speed and monotonic source-time mapping.
  - Expose add/move/delete curve points in Inspector and route curve edits through one undoable command surface.
  - For a fixture spanning 0.25x, 1x, and 4x segments, assert duration and preview/export source-frame selection agree within one frame, including reverse/trim boundaries.

#### requirement-96ebb3ccd95b7798

- Candidate/source: `doc-cb8332cd636b0d69` at `docs/architecture/HANDOFF-2026-07.md:182` (requirement)
- Expected behavior: Speed curves are editable and deterministic in preview/export.
- Resolution: `reviewed-mapping-report:nonlinear-speed-curves` — Tracked owners implement a single constant speed only; no editable nonlinear curve model or preview/export parity exists.
- Exact acceptance contract:
  - Add speed-curve model and commands.
  - Add curve editor and keyframe interactions.
  - Verify preview/export timing with nonlinear fixtures.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/src/plan/tests.rs#source_frame_video_with_trim_and_speed` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-domain/tests/speed_curve.rs#nonlinear_speed_curve_roundtrips_and_maps_source_frames` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render source_frame_video_with_trim_and_speed`
  - Run: `cargo test -p opentake-domain --test speed_curve nonlinear_speed_curve_roundtrips_and_maps_source_frames -- --exact`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/clip.rs#Clip::speed`, `crates/opentake-render/src/plan/build.rs#source_frame_index`, `docs/architecture/CAPCUT-GAP.md`, `docs/architecture/HANDOFF-2026-07.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render source_frame_video_with_trim_and_speed`
  - Run: `cargo test -p opentake-domain --test speed_curve nonlinear_speed_curve_roundtrips_and_maps_source_frames -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 3: advanced-keyframe-easing (implementation-slice-52027446cb9c7149)

**Covered records:**
- `requirement-cfeb61ebe0c11259` (requirement)
- `requirement-c1f53c97751f9a26` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/keyframe.rs#Interpolation`
- Modify: `crates/opentake-domain/src/keyframe.rs#KeyframeTrack::sample`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (existing-owned): `crates/opentake-domain/src/keyframe.rs#sample_linear_branch`
- Test (existing-owned): `crates/opentake-domain/src/keyframe.rs#sample_hold_branch_uses_left_value`
- Test (existing-owned): `crates/opentake-domain/src/keyframe.rs#sample_smooth_branch_uses_smoothstep`
- Test (reviewed-planned): `crates/opentake-domain/src/keyframe.rs#advanced_easing_preview_render_parity`

**Candidate-bound contracts:**

#### requirement-cfeb61ebe0c11259

- Candidate/source: `doc-f16bad5f9e5d0ad9` at `docs/architecture/CAPCUT-GAP.md:55` (requirement)
- Expected behavior: Keyframes support the specified easing curves across UI, preview, and export.
- Resolution: `reviewed-mapping-report:advanced-keyframe-easing` — The enum and sampler support only linear, hold, and smooth; advanced easing remains unimplemented.
- Exact acceptance contract:
  - Persist easing metadata per keyframe segment, including linear, hold, smooth, and cubic-Bezier control points.
  - Expose easing selection/control-point editing in the keyframe UI with add/change/remove undo and redo.
  - Sample boundary, midpoint, and overshoot cases for transform and opacity; assert preview/export values match the reference evaluator within 1e-4.

#### requirement-c1f53c97751f9a26

- Candidate/source: `doc-56aadea89db5984e` at `docs/architecture/CAPCUT-GAP.md:56` (requirement)
- Expected behavior: Support advanced easing beyond linear/hold/smooth.
- Resolution: `reviewed-mapping-report:advanced-keyframe-easing` — The enum and sampler support only linear, hold, and smooth; advanced easing remains unimplemented.
- Exact acceptance contract:
  - Implementation: Extend the interpolation model with cubic-bezier handles and selected easing presets, define deterministic sampling/export approximation, expose editing UI, migrate old projects safely, and add sampling/round-trip/preview-export tests.
  - Add command, undo/redo, serialization, sampling, and Inspector interaction tests for every named field, keyframe, text, or font behavior; the affected Rust and web suites must pass.
  - Exercise the named Inspector or timeline editing path end to end and retain exact visible-result or runtime evidence before reclassification.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-domain/src/keyframe.rs#sample_linear_branch` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-domain/src/keyframe.rs#sample_hold_branch_uses_left_value` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-domain/src/keyframe.rs#sample_smooth_branch_uses_smoothstep` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-domain/src/keyframe.rs#advanced_easing_preview_render_parity` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-domain sample_linear_branch`
  - Run: `cargo test -p opentake-domain sample_hold_branch_uses_left_value`
  - Run: `cargo test -p opentake-domain sample_smooth_branch_uses_smoothstep`
  - Run: `cargo test -p opentake-domain advanced_easing_preview_render_parity`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/keyframe.rs#Interpolation`, `crates/opentake-domain/src/keyframe.rs#KeyframeTrack::sample`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-domain sample_linear_branch`
  - Run: `cargo test -p opentake-domain sample_hold_branch_uses_left_value`
  - Run: `cargo test -p opentake-domain sample_smooth_branch_uses_smoothstep`
  - Run: `cargo test -p opentake-domain advanced_easing_preview_render_parity`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 4: rgb-color-curves (implementation-slice-3df6b9fbff98b25f)

**Covered records:**
- `requirement-a447463cadbc5867` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/grade.rs#ColorCurve`
- Modify: `crates/opentake-render/src/plan/types.rs#ColorCurve`
- Modify: `web/src/components/inspector/ColorCurvesPanel.tsx#ColorCurvesPanel`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-render/tests/color_curves.rs#editable_curve_applies_in_grade_chain`

**Candidate-bound contracts:**

#### requirement-a447463cadbc5867

- Candidate/source: `doc-0002d911fb89d534` at `docs/architecture/CAPCUT-GAP.md:133` (requirement)
- Expected behavior: RGB curves are editable and applied in the grade chain.
- Resolution: `reviewed-mapping-report:rgb-color-curves` — No tracked RGB curve model or editor was found; generic color-grade and color conversion code are not curve ownership.
- Exact acceptance contract:
  - Persist ordered control points independently for master, red, green, and blue curves with monotonic input coordinates.
  - Expose point add/move/delete/reset through one undoable grade command.
  - Apply channel-ramp fixtures and assert endpoint/midpoint values plus preview/export pixels match the curve evaluator within 1e-4.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/color_curves.rs#editable_curve_applies_in_grade_chain` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test color_curves editable_curve_applies_in_grade_chain -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/grade.rs#ColorCurve`, `crates/opentake-render/src/plan/types.rs#ColorCurve`, `web/src/components/inspector/ColorCurvesPanel.tsx#ColorCurvesPanel`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test color_curves editable_curve_applies_in_grade_chain -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 5: desktop-asr-captions (implementation-slice-a59427ce457cfe29)

**Covered records:**
- `requirement-099fd03f333fd8c5` (requirement)

**Files:**
- Modify: `web/src/components/media/CaptionsTab.tsx#CaptionsTab`
- Modify: `web/src/store/editActions.ts#generateCaptions`
- Modify: `src-tauri/src/captions.rs#generate_captions`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (existing-owned): `src-tauri/src/captions.rs#eligible_auto_keeps_audio_drops_silent_video`
- Test (existing-owned): `crates/opentake-media/src/transcribe/captions.rs#caption_specs_builds_and_cases_clips`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.test.tsx#generate_timestamped_captions_from_desktop_surface`

**Candidate-bound contracts:**

#### requirement-099fd03f333fd8c5

- Candidate/source: `doc-451879b4e33d34d6` at `docs/architecture/CAPCUT-GAP.md:179` (requirement)
- Expected behavior: At docs/architecture/CAPCUT-GAP.md:179 under “高精度 ASR 一键转字幕 — `has` · 难度 medium · 优先级 p0” (heading), the source “### 高精度 ASR 一键转字幕 — `has` · 难度 medium · 优先级 p0” requires this exact behavior: ASR creates timestamped captions through the desktop surface.
- Resolution: `reviewed-mapping-report:desktop-asr-captions` — The desktop-to-Rust call chain exists with strong backend tests; a focused desktop component acceptance is the remaining evidence closure.
- Exact acceptance contract:
  - Source binding: docs/architecture/CAPCUT-GAP.md:179; signal=heading; heading=高精度 ASR 一键转字幕 — `has` · 难度 medium · 优先级 p0; candidate=### 高精度 ASR 一键转字幕 — `has` · 难度 medium · 优先级 p0
  - Expected behavior: ASR creates timestamped captions through the desktop surface. This closes only the promise expressed by “高精度 ASR 一键转字幕 — `has` · 难度 medium · 优先级 p0” in “高精度 ASR 一键转字幕 — `has` · 难度 medium · 优先级 p0”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “高精度 ASR 一键转字幕 — `has` · 难度 medium · 优先级 p0” with the scenario below and register test:tools/completion-tests/doc-451879b4e33d34d6.test.mjs#completion_451879b4e33d34d6_asr_creates_timestamped_captions_through_the_des
  - Initial state/input/event: construct the smallest deterministic state that exposes “高精度 ASR 一键转字幕 — `has` · 难度 medium · 优先级 p0”, apply the precise input or event implied by “ASR creates timestamped captions through the desktop surface.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “ASR creates timestamped captions through the desktop surface.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “高精度 ASR 一键转字幕 — `has` · 难度 medium · 优先级 p0”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-451879b4e33d34d6.test.mjs#completion_451879b4e33d34d6_asr_creates_timestamped_captions_through_the_des.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/captions.rs#eligible_auto_keeps_audio_drops_silent_video` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/transcribe/captions.rs#caption_specs_builds_and_cases_clips` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/media/CaptionsTab.test.tsx#generate_timestamped_captions_from_desktop_surface` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri eligible_auto_keeps_audio_drops_silent_video`
  - Run: `cargo test -p opentake-media caption_specs_builds_and_cases_clips`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.test.tsx -t "generate_timestamped_captions_from_desktop_surface"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/CaptionsTab.tsx#CaptionsTab`, `web/src/store/editActions.ts#generateCaptions`, `src-tauri/src/captions.rs#generate_captions`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri eligible_auto_keeps_audio_drops_silent_video`
  - Run: `cargo test -p opentake-media caption_specs_builds_and_cases_clips`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.test.tsx -t "generate_timestamped_captions_from_desktop_surface"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 6: caption-group-style-atomicity (implementation-slice-b23fd67041eec738)

**Covered records:**
- `requirement-1e1b83d4d6eab6b3` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/caption_sync.rs#sync_caption_group_style`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::AddCaptions`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (existing-owned): `crates/opentake-domain/src/caption_sync.rs#multi_track_same_group_all_restyled`
- Test (existing-owned): `crates/opentake-ops/src/command.rs#add_captions_is_one_undo_step`

**Candidate-bound contracts:**

#### requirement-1e1b83d4d6eab6b3

- Candidate/source: `doc-40ca9af8f8f1f16d` at `docs/architecture/CAPCUT-GAP.md:191` (requirement)
- Expected behavior: Caption style changes can be applied as one undoable command to a group.
- Resolution: `reviewed-mapping-report:caption-group-style-atomicity` — Tracked group-style and one-undo-step implementations match the requirement; the outstanding work is precise ledger evidence linkage.
- Exact acceptance contract:
  - Add one batch command that applies selected style fields to a caption group while preserving text and frame ranges.
  - Expose group selection, field-level apply, reset, and a single undo/redo step in Captions.
  - Test mixed-style groups, partial field updates, save/reopen, and render parity across the first/middle/last caption.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-domain/src/caption_sync.rs#multi_track_same_group_all_restyled` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-ops/src/command.rs#add_captions_is_one_undo_step` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-domain multi_track_same_group_all_restyled`
  - Run: `cargo test -p opentake-ops add_captions_is_one_undo_step`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/caption_sync.rs#sync_caption_group_style`, `crates/opentake-ops/src/command.rs#EditCommand::AddCaptions`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-domain multi_track_same_group_all_restyled`
  - Run: `cargo test -p opentake-ops add_captions_is_one_undo_step`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 7: IN-subtitle-srt-export (implementation-slice-3b2b0548dab34607)

**Covered records:**
- `requirement-7043d6a44938bd27` (requirement)

**Files:**
- Modify: `web/src/components/shell/TitleBar.tsx#onExportSubtitles`
- Modify: `web/src/lib/api.ts#exportSubtitles`
- Modify: `src-tauri/src/commands.rs#export_subtitles`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (existing-owned): `src-tauri/src/commands.rs#exports_non_empty_srt_with_cue_count`
- Test (reviewed-planned): `web/src/components/shell/TitleBar.visual.test.ts#subtitle export menu routes srt and vtt`

**Candidate-bound contracts:**

#### requirement-7043d6a44938bd27

- Candidate/source: `doc-457715e9d0e73d1b` at `docs/architecture/CAPCUT-GAP.md:197` (requirement)
- Expected behavior: At docs/architecture/CAPCUT-GAP.md:197 under “导出 .srt 字幕文件 — `missing` · 难度 low · 优先级 p1” (heading), the source “### 导出 .srt 字幕文件 — `missing` · 难度 low · 优先级 p1” requires this exact behavior: Captions export as valid SRT through the UI/command boundary.
- Resolution: `validated-ledger-evidence:IN-subtitle-srt-export` — The UI/API/Tauri boundary and valid SRT writer already exist; only focused UI routing evidence is planned.
- Exact acceptance contract:
  - Source binding: docs/architecture/CAPCUT-GAP.md:197; signal=heading; heading=导出 .srt 字幕文件 — `missing` · 难度 low · 优先级 p1; candidate=### 导出 .srt 字幕文件 — `missing` · 难度 low · 优先级 p1
  - Expected behavior: Captions export as valid SRT through the UI/command boundary. This closes only the promise expressed by “导出 .srt 字幕文件 — `missing` · 难度 low · 优先级 p1” in “导出 .srt 字幕文件 — `missing` · 难度 low · 优先级 p1”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “导出 .srt 字幕文件 — `missing` · 难度 low · 优先级 p1” with the scenario below and register test:crates/opentake-render/tests/completion_457715e9d0e73d1b.rs#completion_457715e9d0e73d1b_captions_export_as_valid_srt_through_the_ui_comm
  - Initial state/input/event: start from the smallest valid fixture for “Captions export as valid SRT through the UI/command boundary.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “导出 .srt 字幕文件 — `missing` · 难度 low · 优先级 p1”.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Captions export as valid SRT through the UI/command boundary.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_457715e9d0e73d1b.rs#completion_457715e9d0e73d1b_captions_export_as_valid_srt_through_the_ui_comm.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/commands.rs#exports_non_empty_srt_with_cue_count` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/shell/TitleBar.visual.test.ts#subtitle export menu routes srt and vtt` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri exports_non_empty_srt_with_cue_count`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.visual.test.ts -t "subtitle export menu routes srt and vtt"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/TitleBar.tsx#onExportSubtitles`, `web/src/lib/api.ts#exportSubtitles`, `src-tauri/src/commands.rs#export_subtitles`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri exports_non_empty_srt_with_cue_count`
  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.visual.test.ts -t "subtitle export menu routes srt and vtt"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 8: safe-preview-font-catalog (implementation-slice-bbb6af2d5d755acb)

**Covered records:**
- `requirement-f354675301c951ec` (requirement)

**Files:**
- Modify: `web/src/components/inspector/TextTab.tsx#TextTab`
- Modify: `crates/opentake-render/src/gpu/text_engine.rs#CosmicTextRasterizer`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Test (reviewed-planned): `web/src/components/inspector/TextTab.fonts.test.tsx#safe_preview_fonts_fallback_without_layout_shift`
- Test (reviewed-planned): `crates/opentake-render/src/gpu/text_engine.rs#bundled_and_preview_capable_font_catalog`

**Candidate-bound contracts:**

#### requirement-f354675301c951ec

- Candidate/source: `doc-318b5bedc8a1b507` at `docs/architecture/MODULE-PORT-MAP.md:1100` (requirement)
- Expected behavior: Load bundled fonts safely and expose only preview-capable system families.
- Resolution: `reviewed-mapping-report:safe-preview-font-catalog` — TextTab accepts free text and the renderer scans system fonts; no bundled-safe, preview-capable catalog owner exists.
- Exact acceptance contract:
  - Implementation: Add idempotent recursive bundled-font registration, family deduplication, system-family enumeration excluding bundled families, Aa1 coverage checks, font-picker wiring, and missing-resource/cross-platform tests.
  - Add command, undo/redo, serialization, sampling, and Inspector interaction tests for every named field, keyframe, text, or font behavior; the affected Rust and web suites must pass.
  - Exercise the named Inspector or timeline editing path end to end and retain exact visible-result or runtime evidence before reclassification.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/TextTab.fonts.test.tsx#safe_preview_fonts_fallback_without_layout_shift` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-render/src/gpu/text_engine.rs#bundled_and_preview_capable_font_catalog` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.fonts.test.tsx -t "safe_preview_fonts_fallback_without_layout_shift"`
  - Run: `cargo test -p opentake-render bundled_and_preview_capable_font_catalog`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/TextTab.tsx#TextTab`, `crates/opentake-render/src/gpu/text_engine.rs#CosmicTextRasterizer`, `docs/architecture/MODULE-PORT-MAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.fonts.test.tsx -t "safe_preview_fonts_fallback_without_layout_shift"`
  - Run: `cargo test -p opentake-render bundled_and_preview_capable_font_catalog`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 9: keyframe-track-invariants (implementation-slice-02d00c6158446e9a)

**Covered records:**
- `requirement-a1882b9a12702441` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/keyframe.rs#KeyframeTrack::upsert`
- Modify: `crates/opentake-domain/src/keyframe.rs#KeyframeTrack::remove`
- Modify: `crates/opentake-domain/src/keyframe.rs#KeyframeTrack::move_keyframe`
- Modify: `docs/modules/opentake-domain/keyframe-transform.md`
- Test (existing-owned): `crates/opentake-domain/src/keyframe.rs#upsert_keeps_sorted_and_replaces`
- Test (existing-owned): `crates/opentake-domain/src/keyframe.rs#move_keyframe_abandons_on_occupied_target`
- Test (existing-owned): `crates/opentake-domain/src/keyframe.rs#move_keyframe_noop_when_source_missing`

**Candidate-bound contracts:**

#### requirement-a1882b9a12702441

- Candidate/source: `doc-dee167ccdf9ec109` at `docs/modules/opentake-domain/keyframe-transform.md:23` (requirement)
- Expected behavior: At docs/modules/opentake-domain/keyframe-transform.md:23 under “关键帧（keyframe.rs）” (gap-marker), the source “- `KeyframeTrack<V> { keyframes }`：`is_active()` = 非空；`upsert` 保持按 `frame` 升序、同帧替换；`remove(frame)`；`move_keyframe(old, new)`（源缺失则 no-op，目标被占则**放弃**，对齐上游 `move(from:to:)`）。” requires this exact behavior: Keep keyframes sorted, replace on equal frame, no-op on missing source, and abandon a move onto an occupied destination.
- Resolution: `reviewed-mapping-report:keyframe-track-invariants` — The exact invariants and focused tests exist; the record requires evidence rebinding rather than a new product implementation.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-domain/keyframe-transform.md:23; signal=gap-marker; heading=关键帧（keyframe.rs）; candidate=- `KeyframeTrack<V> { keyframes }`：`is_active()` = 非空；`upsert` 保持按 `frame` 升序、同帧替换；`remove(frame)`；`move_keyframe(old, new)`（源缺失则 no-op，目标被占则**放弃**，对齐上游 `move(from:to:)`）。
  - Expected behavior: Keep keyframes sorted, replace on equal frame, no-op on missing source, and abandon a move onto an occupied destination. This closes only the promise expressed by “`KeyframeTrack<V> { keyframes }`：`is_active()` = 非空；`upsert` 保持按 `frame` 升序、同帧替换；`remove(frame)`；`move_keyframe(old, new)`（源缺失则 no-op，目标被占则**放弃**，对齐上游 `move(from:to:)`）。” in “关键帧（keyframe.rs）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “`KeyframeTrack<V> { keyframes }`：`is_active()` = 非空；`upsert` 保持按 `frame` 升序、同帧替换；`remove(frame)`；`move_keyframe(old, new)`（源缺失则 no-op，目标被占则**放弃**，对齐上游 `move(from:to:)`）。” with the scenario below and register test:crates/opentake-project/tests/completion_dee167ccdf9ec109.rs#completion_dee167ccdf9ec109_keep_keyframes_sorted_replace_on_equal_frame_no_
  - Initial state/input/event: start from the smallest valid fixture for “Keep keyframes sorted, replace on equal frame, no-op on missing source, and abandon a move onto an occupied destination.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “`KeyframeTrack<V> { keyframes }`：`is_active()` = 非空；`upsert` 保持按 `frame` 升序、同帧替换；`remove(frame)`；`move_keyframe(old, new)`（源缺失则 no-op，目标被占则**放弃**，对齐上游 `move(from:to:)`）。”.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Keep keyframes sorted, replace on equal frame, no-op on missing source, and abandon a move onto an occupied destination.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_dee167ccdf9ec109.rs#completion_dee167ccdf9ec109_keep_keyframes_sorted_replace_on_equal_frame_no_.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-domain/src/keyframe.rs#upsert_keeps_sorted_and_replaces` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-domain/src/keyframe.rs#move_keyframe_abandons_on_occupied_target` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-domain/src/keyframe.rs#move_keyframe_noop_when_source_missing` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-domain upsert_keeps_sorted_and_replaces`
  - Run: `cargo test -p opentake-domain move_keyframe_abandons_on_occupied_target`
  - Run: `cargo test -p opentake-domain move_keyframe_noop_when_source_missing`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/keyframe.rs#KeyframeTrack::upsert`, `crates/opentake-domain/src/keyframe.rs#KeyframeTrack::remove`, `crates/opentake-domain/src/keyframe.rs#KeyframeTrack::move_keyframe`, `docs/modules/opentake-domain/keyframe-transform.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-domain upsert_keeps_sorted_and_replaces`
  - Run: `cargo test -p opentake-domain move_keyframe_abandons_on_occupied_target`
  - Run: `cargo test -p opentake-domain move_keyframe_noop_when_source_missing`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 10: IN-text-rasterizer-contract (implementation-slice-b664d802fb27c156)

**Covered records:**
- `requirement-8766c1ebeaab1411` (requirement)
- `requirement-875e3773ed19fa15` (requirement)

**Files:**
- Modify: `crates/opentake-render/src/gpu/text_raster.rs#TextRasterizer`
- Modify: `crates/opentake-render/src/gpu/text_engine.rs#CosmicTextRasterizer`
- Modify: `crates/opentake-render/src/gpu/text_engine.rs#rasterize_box`
- Modify: `docs/modules/opentake-render/text-rasterizer.md`
- Test (existing-owned): `crates/opentake-render/src/gpu/text_engine.rs#empty_content_is_none`
- Test (existing-owned): `crates/opentake-render/src/gpu/text_engine.rs#degenerate_box_is_none`
- Test (existing-owned): `crates/opentake-render/src/gpu/text_engine.rs#renders_box_sized_premultiplied_frame`
- Test (existing-owned): `crates/opentake-render/src/gpu/text_engine.rs#background_fill_paints_even_without_glyphs`

**Candidate-bound contracts:**

#### requirement-8766c1ebeaab1411

- Candidate/source: `doc-ea0cb7db962a9172` at `docs/modules/opentake-render/text-rasterizer.md:15` (requirement)
- Expected behavior: At docs/modules/opentake-render/text-rasterizer.md:15 under “trait 边界（`gpu/text_raster.rs`）” (gap-marker), the source “- `TextRasterizer::rasterize(req) -> Option<DecodedFrame>`：栅格化，文字栈不可用（如 headless 无字体）或请求退化时返回 `None`（**永不 `todo!()`/`unimplemented!()`**）。” requires this exact behavior: Rasterize text without panicking on empty/degenerate/no-font inputs and return premultiplied box-sized frames when drawable.
- Resolution: `validated-ledger-evidence:IN-text-rasterizer-contract` — Empty/degenerate handling and premultiplied box-sized no-glyph background rendering are directly tested.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-render/text-rasterizer.md:15; signal=gap-marker; heading=trait 边界（`gpu/text_raster.rs`）; candidate=- `TextRasterizer::rasterize(req) -> Option<DecodedFrame>`：栅格化，文字栈不可用（如 headless 无字体）或请求退化时返回 `None`（**永不 `todo!()`/`unimplemented!()`**）。
  - Expected behavior: Rasterize text without panicking on empty/degenerate/no-font inputs and return premultiplied box-sized frames when drawable. This closes only the promise expressed by “`TextRasterizer::rasterize(req) -> Option<DecodedFrame>`：栅格化，文字栈不可用（如 headless 无字体）或请求退化时返回 `None`（**永不 `todo!()`/`unimplemented!()`**）。” in “trait 边界（`gpu/text_raster.rs`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “`TextRasterizer::rasterize(req) -> Option<DecodedFrame>`：栅格化，文字栈不可用（如 headless 无字体）或请求退化时返回 `None`（**永不 `todo!()`/`unimplemented!()`**）。” with the scenario below and register test:crates/opentake-project/tests/completion_ea0cb7db962a9172.rs#completion_ea0cb7db962a9172_rasterize_text_without_panicking_on_empty_degene
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “`TextRasterizer::rasterize(req) -> Option<DecodedFrame>`：栅格化，文字栈不可用（如 headless 无字体）或请求退化时返回 `None`（**永不 `todo!()`/`unimplemented!()`**）。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Rasterize text without panicking on empty/degenerate/no-font inputs and return premultiplied box-sized frames when drawable.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_ea0cb7db962a9172.rs#completion_ea0cb7db962a9172_rasterize_text_without_panicking_on_empty_degene.

#### requirement-875e3773ed19fa15

- Candidate/source: `doc-972b4ab2aed2045e` at `docs/modules/opentake-render/text-rasterizer.md:38` (requirement)
- Expected behavior: At docs/modules/opentake-render/text-rasterizer.md:38 under “不变量” (gap-marker), the source “- **无字体不崩**：headless 无字面时仍产出框尺寸帧（背景/描边照画），仅 glyph 像素缺失。” requires this exact behavior: Rasterize text without panicking on empty/degenerate/no-font inputs and return premultiplied box-sized frames when drawable.
- Resolution: `validated-ledger-evidence:IN-text-rasterizer-contract` — Empty/degenerate handling and premultiplied box-sized no-glyph background rendering are directly tested.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-render/text-rasterizer.md:38; signal=gap-marker; heading=不变量; candidate=- **无字体不崩**：headless 无字面时仍产出框尺寸帧（背景/描边照画），仅 glyph 像素缺失。
  - Expected behavior: Rasterize text without panicking on empty/degenerate/no-font inputs and return premultiplied box-sized frames when drawable. This closes only the promise expressed by “**无字体不崩**：headless 无字面时仍产出框尺寸帧（背景/描边照画），仅 glyph 像素缺失。” in “不变量”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “**无字体不崩**：headless 无字面时仍产出框尺寸帧（背景/描边照画），仅 glyph 像素缺失。” with the scenario below and register test:crates/opentake-project/tests/completion_972b4ab2aed2045e.rs#completion_972b4ab2aed2045e_rasterize_text_without_panicking_on_empty_degene
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “**无字体不崩**：headless 无字面时仍产出框尺寸帧（背景/描边照画），仅 glyph 像素缺失。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Rasterize text without panicking on empty/degenerate/no-font inputs and return premultiplied box-sized frames when drawable.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_972b4ab2aed2045e.rs#completion_972b4ab2aed2045e_rasterize_text_without_panicking_on_empty_degene.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/src/gpu/text_engine.rs#empty_content_is_none` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/src/gpu/text_engine.rs#degenerate_box_is_none` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/src/gpu/text_engine.rs#renders_box_sized_premultiplied_frame` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/src/gpu/text_engine.rs#background_fill_paints_even_without_glyphs` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render empty_content_is_none`
  - Run: `cargo test -p opentake-render degenerate_box_is_none`
  - Run: `cargo test -p opentake-render renders_box_sized_premultiplied_frame`
  - Run: `cargo test -p opentake-render background_fill_paints_even_without_glyphs`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-render/src/gpu/text_raster.rs#TextRasterizer`, `crates/opentake-render/src/gpu/text_engine.rs#CosmicTextRasterizer`, `crates/opentake-render/src/gpu/text_engine.rs#rasterize_box`, `docs/modules/opentake-render/text-rasterizer.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render empty_content_is_none`
  - Run: `cargo test -p opentake-render degenerate_box_is_none`
  - Run: `cargo test -p opentake-render renders_box_sized_premultiplied_frame`
  - Run: `cargo test -p opentake-render background_fill_paints_even_without_glyphs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 11: control-acceptance (implementation-slice-50bf8a368fef0eac)

**Covered records:**
- `control-record-d48e9aa692d0d167` (control)
- `control-record-bbd0dcd94549808c` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/components/inspector/Inspector.tsx#KeyframeRowControls`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-287044a170afa328 previous keyframe`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-e99f1fdf89f06557 next keyframe`

**Candidate-bound contracts:**

#### control-record-d48e9aa692d0d167

- Candidate/source: `control-287044a170afa328` at `web/src/components/inspector/Inspector.tsx:297:7` (control)
- Expected behavior: previous keyframe: assert exactly if (prev !== null) setActiveFrame(prev); otherwise no-op and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-287044a170afa328.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-287044a170afa328 previous keyframe.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled when prev === null is false..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => prev !== null && setActiveFrame(prev)}.
  - Exact call/state/backend: stateTransition=previous keyframe: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:297::handler {() => prev !== null && setActiveFrame(prev)} -> if (prev !== null) setActiveFrame(prev); otherwise no-op","web/src/components/inspector/Inspector.tsx::KeyframeRowControls -> uiStore.setActiveFrame","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/components/inspector/Inspector.tsx#KeyframeRowControls"].
  - Visible/accessibility/return path: success=previous keyframe: assert exactly if (prev !== null) setActiveFrame(prev); otherwise no-op and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"inspector.keyframe.prev\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"previous keyframe: assert exactly if (prev !== null) setActiveFrame(prev); otherwise no-op and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"The rendered control is disabled by {prev === null}.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

#### control-record-bbd0dcd94549808c

- Candidate/source: `control-e99f1fdf89f06557` at `web/src/components/inspector/Inspector.tsx:318:7` (control)
- Expected behavior: next keyframe: assert exactly if (next !== null) setActiveFrame(next); otherwise no-op and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-e99f1fdf89f06557.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-e99f1fdf89f06557 next keyframe.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled when next === null is false..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => next !== null && setActiveFrame(next)}.
  - Exact call/state/backend: stateTransition=next keyframe: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:318::handler {() => next !== null && setActiveFrame(next)} -> if (next !== null) setActiveFrame(next); otherwise no-op","web/src/components/inspector/Inspector.tsx::KeyframeRowControls -> uiStore.setActiveFrame","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/components/inspector/Inspector.tsx#KeyframeRowControls"].
  - Visible/accessibility/return path: success=next keyframe: assert exactly if (next !== null) setActiveFrame(next); otherwise no-op and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"inspector.keyframe.next\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"next keyframe: assert exactly if (next !== null) setActiveFrame(next); otherwise no-op and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"The rendered control is disabled by {next === null}.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-287044a170afa328 previous keyframe` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-e99f1fdf89f06557 next keyframe` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-287044a170afa328 previous keyframe"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-e99f1fdf89f06557 next keyframe"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/components/inspector/Inspector.tsx#KeyframeRowControls`, `web/src/components/inspector/Inspector.tsx#Inspector` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-287044a170afa328 previous keyframe"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-e99f1fdf89f06557 next keyframe"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 12: control-acceptance (implementation-slice-9aa99bba06e22056)

**Covered records:**
- `control-record-f04cdbe03d43ee79` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/store/editActions.ts#removeKeyframe`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::RemoveKeyframe`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-cae48d1cbb87b634 add or remove keyframe at playhead`

**Candidate-bound contracts:**

#### control-record-f04cdbe03d43ee79

- Candidate/source: `control-cae48d1cbb87b634` at `web/src/components/inspector/Inspector.tsx:305:7` (control)
- Expected behavior: add or remove keyframe at playhead: assert exactly if (!inRange) no-op; else if (onKeyframe) edit.removeKeyframe(clip.id, property, activeFrame); else edit.stampKeyframe(clip.id, property, activeFrame) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-cae48d1cbb87b634.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-cae48d1cbb87b634 add or remove keyframe at playhead.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled when !inRange is false..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={toggle}.
  - Exact call/state/backend: stateTransition=add or remove keyframe at playhead: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:305::handler {toggle} -> if (!inRange) no-op; else if (onKeyframe) edit.removeKeyframe(clip.id, property, activeFrame); else edit.stampKeyframe(clip.id, property, activeFrame)","web/src/store/editActions.ts::removeKeyframe or stampKeyframe with the exact clip.id/property/activeFrame arguments","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::RemoveKeyframe or StampKeyframe","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#removeKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=add or remove keyframe at playhead: assert exactly if (!inRange) no-op; else if (onKeyframe) edit.removeKeyframe(clip.id, property, activeFrame); else edit.stampKeyframe(clip.id, property, activeFrame) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: diamondTitle.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"add or remove keyframe at playhead: assert exactly if (!inRange) no-op; else if (onKeyframe) edit.removeKeyframe(clip.id, property, activeFrame); else edit.stampKeyframe(clip.id, property, activeFrame) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"The rendered control is disabled by {!inRange}.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-cae48d1cbb87b634 add or remove keyframe at playhead` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-cae48d1cbb87b634 add or remove keyframe at playhead"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/store/editActions.ts#removeKeyframe`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::RemoveKeyframe`, `web/src/components/inspector/Inspector.tsx#Inspector`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-cae48d1cbb87b634 add or remove keyframe at playhead"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 13: control-acceptance (implementation-slice-1bf384c9c8f84ddd)

**Covered records:**
- `control-record-ed009f95abf0a745` (control)
- `control-record-75da1d2e675d6ebf` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/components/inspector/Inspector.tsx#ClipInspector`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-ff45bd17a9877c19 Video/Audio inspector tab`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-239a5889cdb86de3 show or hide keyframes panel`

**Candidate-bound contracts:**

#### control-record-ed009f95abf0a745

- Candidate/source: `control-ff45bd17a9877c19` at `web/src/components/inspector/Inspector.tsx:449:13` (control)
- Expected behavior: Video/Audio inspector tab: assert exactly setTab(tabId) for the clicked rendered tabId and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-ff45bd17a9877c19.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-ff45bd17a9877c19 Video/Audio inspector tab.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => setTab(tabId)}.
  - Exact call/state/backend: stateTransition=Video/Audio inspector tab: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:449::handler {() => setTab(tabId)} -> setTab(tabId) for the clicked rendered tabId","web/src/components/inspector/Inspector.tsx::ClipInspector -> uiStore.setInspectorTab","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/components/inspector/Inspector.tsx#ClipInspector"].
  - Visible/accessibility/return path: success=Video/Audio inspector tab: assert exactly setTab(tabId) for the clicked rendered tabId and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"Video/Audio inspector tab: assert exactly setTab(tabId) for the clicked rendered tabId and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

#### control-record-75da1d2e675d6ebf

- Candidate/source: `control-239a5889cdb86de3` at `web/src/components/inspector/Inspector.tsx:629:9` (control)
- Expected behavior: show or hide keyframes panel: assert exactly onToggleKeyframes() -> toggleKeyframesPanel() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-239a5889cdb86de3.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-239a5889cdb86de3 show or hide keyframes panel.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={onToggleKeyframes}.
  - Exact call/state/backend: stateTransition=show or hide keyframes panel: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:629::handler {onToggleKeyframes} -> onToggleKeyframes() -> toggleKeyframesPanel()","web/src/components/inspector/Inspector.tsx::ClipInspector -> uiStore.toggleKeyframesPanel","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/components/inspector/Inspector.tsx#ClipInspector"].
  - Visible/accessibility/return path: success=show or hide keyframes panel: assert exactly onToggleKeyframes() -> toggleKeyframesPanel() and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"show or hide keyframes panel: assert exactly onToggleKeyframes() -> toggleKeyframesPanel() and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-ff45bd17a9877c19 Video/Audio inspector tab` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-239a5889cdb86de3 show or hide keyframes panel` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-ff45bd17a9877c19 Video/Audio inspector tab"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-239a5889cdb86de3 show or hide keyframes panel"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/components/inspector/Inspector.tsx#ClipInspector`, `web/src/components/inspector/Inspector.tsx#Inspector` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-ff45bd17a9877c19 Video/Audio inspector tab"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-239a5889cdb86de3 show or hide keyframes panel"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 14: control-acceptance (implementation-slice-a6800ce623424fa1)

**Covered records:**
- `control-record-c99259dbc9065cf4` (control)
- `control-record-6965fbef94d9fa4e` (control)
- `control-record-eda693c8eb2b99b3` (control)
- `control-record-92ab23701b633e3d` (control)
- `control-record-820451bca928a167` (control)
- `control-record-ea6daad26590e1c0` (control)
- `control-record-5debc87819b3e009` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/store/editActions.ts#upsertKeyframe`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::UpsertKeyframe`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-90b6142c9b28af54 clip volume`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-08376197cc04d5c7 clip scale`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-081219e1fb457cfe clip rotation`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-16114827f9ea0a12 clip opacity`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-cd756dccc56343eb clip position X`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-11ebd75be0f93008 clip position Y`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-f8d159f2ffe6aa33 crop edge value`

**Candidate-bound contracts:**

#### control-record-c99259dbc9065cf4

- Candidate/source: `control-90b6142c9b28af54` at `web/src/components/inspector/Inspector.tsx:475:15` (control)
- Expected behavior: clip volume: assert exactly if (volumeAnimated) edit.upsertKeyframe(clip.id, 'volume', activeFrame, volumeKeyframeValue(v)); else edit.setClipProperties([clip.id], { volume: v }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-90b6142c9b28af54.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-90b6142c9b28af54 clip volume.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) =>
                  volumeAnimated
                    ? edit.upsertKeyframe(clip.id, "volume", activeFrame, volumeKeyframeValue(v))
                    : commit({ volume: v })
                }.
  - Exact call/state/backend: stateTransition=clip volume: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:475::handler {(v) =>\n                  volumeAnimated\n                    ? edit.upsertKeyframe(clip.id, \"volume\", activeFrame, volumeKeyframeValue(v))\n                    : commit({ volume: v })\n                } -> if (volumeAnimated) edit.upsertKeyframe(clip.id, 'volume', activeFrame, volumeKeyframeValue(v)); else edit.setClipProperties([clip.id], { volume: v }) via commit","web/src/store/editActions.ts::upsertKeyframe('volume') or setClipProperties({volume})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::UpsertKeyframe or SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#upsertKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=clip volume: assert exactly if (volumeAnimated) edit.upsertKeyframe(clip.id, 'volume', activeFrame, volumeKeyframeValue(v)); else edit.setClipProperties([clip.id], { volume: v }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"clip volume: assert exactly if (volumeAnimated) edit.upsertKeyframe(clip.id, 'volume', activeFrame, volumeKeyframeValue(v)); else edit.setClipProperties([clip.id], { volume: v }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-6965fbef94d9fa4e

- Candidate/source: `control-08376197cc04d5c7` at `web/src/components/inspector/Inspector.tsx:509:17` (control)
- Expected behavior: clip scale: assert exactly if (scaleAnimated) edit.upsertKeyframe(clip.id, 'scale', activeFrame, scaleKeyframeValue(clip.transform, v, aspect)); else edit.setClipProperties([clip.id], { transform: resizeTransformKeepingSourceAspect(clip.transform, v, aspect) }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-08376197cc04d5c7.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-08376197cc04d5c7 clip scale.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) =>
                    scaleAnimated
                      ? edit.upsertKeyframe(
                          clip.id,
                          "scale",
                          activeFrame,
                          scaleKeyframeValue(clip.transform, v, aspect),
                        )
                      : commit({
                          transform: resizeTransformKeepingSourceAspect(clip.transform, v, aspect),
                        })
                  }.
  - Exact call/state/backend: stateTransition=clip scale: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:509::handler {(v) =>\n                    scaleAnimated\n                      ? edit.upsertKeyframe(\n                          clip.id,\n                          \"scale\",\n                          activeFrame,\n                          scaleKeyframeValue(clip.transform, v, aspect),\n                        )\n                      : commit({\n                          transform: resizeTransformKeepingSourceAspect(clip.transform, v, aspect),\n                        })\n                  } -> if (scaleAnimated) edit.upsertKeyframe(clip.id, 'scale', activeFrame, scaleKeyframeValue(clip.transform, v, aspect)); else edit.setClipProperties([clip.id], { transform: resizeTransformKeepingSourceAspect(clip.transform, v, aspect) }) via commit","web/src/store/editActions.ts::upsertKeyframe('scale') or setClipProperties({transform})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::UpsertKeyframe or SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#upsertKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=clip scale: assert exactly if (scaleAnimated) edit.upsertKeyframe(clip.id, 'scale', activeFrame, scaleKeyframeValue(clip.transform, v, aspect)); else edit.setClipProperties([clip.id], { transform: resizeTransformKeepingSourceAspect(clip.transform, v, aspect) }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"clip scale: assert exactly if (scaleAnimated) edit.upsertKeyframe(clip.id, 'scale', activeFrame, scaleKeyframeValue(clip.transform, v, aspect)); else edit.setClipProperties([clip.id], { transform: resizeTransformKeepingSourceAspect(clip.transform, v, aspect) }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-eda693c8eb2b99b3

- Candidate/source: `control-081219e1fb457cfe` at `web/src/components/inspector/Inspector.tsx:534:17` (control)
- Expected behavior: clip rotation: assert exactly if (rotationAnimated) edit.upsertKeyframe(clip.id, 'rotation', activeFrame, rotationKeyframeValue(v)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, rotation: v } }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-081219e1fb457cfe.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-081219e1fb457cfe clip rotation.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) =>
                    rotationAnimated
                      ? edit.upsertKeyframe(clip.id, "rotation", activeFrame, rotationKeyframeValue(v))
                      : commit({ transform: { ...clip.transform, rotation: v } })
                  }.
  - Exact call/state/backend: stateTransition=clip rotation: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:534::handler {(v) =>\n                    rotationAnimated\n                      ? edit.upsertKeyframe(clip.id, \"rotation\", activeFrame, rotationKeyframeValue(v))\n                      : commit({ transform: { ...clip.transform, rotation: v } })\n                  } -> if (rotationAnimated) edit.upsertKeyframe(clip.id, 'rotation', activeFrame, rotationKeyframeValue(v)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, rotation: v } }) via commit","web/src/store/editActions.ts::upsertKeyframe('rotation') or setClipProperties({transform.rotation})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::UpsertKeyframe or SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#upsertKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=clip rotation: assert exactly if (rotationAnimated) edit.upsertKeyframe(clip.id, 'rotation', activeFrame, rotationKeyframeValue(v)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, rotation: v } }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"clip rotation: assert exactly if (rotationAnimated) edit.upsertKeyframe(clip.id, 'rotation', activeFrame, rotationKeyframeValue(v)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, rotation: v } }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-92ab23701b633e3d

- Candidate/source: `control-16114827f9ea0a12` at `web/src/components/inspector/Inspector.tsx:552:17` (control)
- Expected behavior: clip opacity: assert exactly if (opacityAnimated) edit.upsertKeyframe(clip.id, 'opacity', activeFrame, opacityKeyframeValue(v * 100)); else edit.setClipProperties([clip.id], { opacity: v }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-16114827f9ea0a12.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-16114827f9ea0a12 clip opacity.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) =>
                    opacityAnimated
                      ? edit.upsertKeyframe(
                          clip.id,
                          "opacity",
                          activeFrame,
                          opacityKeyframeValue(v * 100),
                        )
                      : commit({ opacity: v })
                  }.
  - Exact call/state/backend: stateTransition=clip opacity: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:552::handler {(v) =>\n                    opacityAnimated\n                      ? edit.upsertKeyframe(\n                          clip.id,\n                          \"opacity\",\n                          activeFrame,\n                          opacityKeyframeValue(v * 100),\n                        )\n                      : commit({ opacity: v })\n                  } -> if (opacityAnimated) edit.upsertKeyframe(clip.id, 'opacity', activeFrame, opacityKeyframeValue(v * 100)); else edit.setClipProperties([clip.id], { opacity: v }) via commit","web/src/store/editActions.ts::upsertKeyframe('opacity') or setClipProperties({opacity})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::UpsertKeyframe or SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#upsertKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=clip opacity: assert exactly if (opacityAnimated) edit.upsertKeyframe(clip.id, 'opacity', activeFrame, opacityKeyframeValue(v * 100)); else edit.setClipProperties([clip.id], { opacity: v }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"clip opacity: assert exactly if (opacityAnimated) edit.upsertKeyframe(clip.id, 'opacity', activeFrame, opacityKeyframeValue(v * 100)); else edit.setClipProperties([clip.id], { opacity: v }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-820451bca928a167

- Candidate/source: `control-cd756dccc56343eb` at `web/src/components/inspector/Inspector.tsx:673:9` (control)
- Expected behavior: clip position X: assert exactly if (animated) edit.upsertKeyframe(clip.id, 'position', activeFrame, positionXKeyframeValue(v, sampledTopLeft.y)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, centerX: v + w / 2 } }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-cd756dccc56343eb.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-cd756dccc56343eb clip position X.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) =>
            animated
              ? edit.upsertKeyframe(
                  clip.id,
                  "position",
                  activeFrame,
                  positionXKeyframeValue(v, sampledTopLeft.y),
                )
              : commit({ transform: { ...clip.transform, centerX: v + w / 2 } })
          }.
  - Exact call/state/backend: stateTransition=clip position X: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:673::handler {(v) =>\n            animated\n              ? edit.upsertKeyframe(\n                  clip.id,\n                  \"position\",\n                  activeFrame,\n                  positionXKeyframeValue(v, sampledTopLeft.y),\n                )\n              : commit({ transform: { ...clip.transform, centerX: v + w / 2 } })\n          } -> if (animated) edit.upsertKeyframe(clip.id, 'position', activeFrame, positionXKeyframeValue(v, sampledTopLeft.y)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, centerX: v + w / 2 } }) via commit","web/src/store/editActions.ts::upsertKeyframe('position', positionXKeyframeValue) or setClipProperties({transform.centerX})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::UpsertKeyframe or SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#upsertKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=clip position X: assert exactly if (animated) edit.upsertKeyframe(clip.id, 'position', activeFrame, positionXKeyframeValue(v, sampledTopLeft.y)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, centerX: v + w / 2 } }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"clip position X: assert exactly if (animated) edit.upsertKeyframe(clip.id, 'position', activeFrame, positionXKeyframeValue(v, sampledTopLeft.y)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, centerX: v + w / 2 } }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-ea6daad26590e1c0

- Candidate/source: `control-11ebd75be0f93008` at `web/src/components/inspector/Inspector.tsx:695:9` (control)
- Expected behavior: clip position Y: assert exactly if (animated) edit.upsertKeyframe(clip.id, 'position', activeFrame, positionYKeyframeValue(sampledTopLeft.x, v)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, centerY: v + h / 2 } }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-11ebd75be0f93008.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-11ebd75be0f93008 clip position Y.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) =>
            animated
              ? edit.upsertKeyframe(
                  clip.id,
                  "position",
                  activeFrame,
                  positionYKeyframeValue(sampledTopLeft.x, v),
                )
              : commit({ transform: { ...clip.transform, centerY: v + h / 2 } })
          }.
  - Exact call/state/backend: stateTransition=clip position Y: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:695::handler {(v) =>\n            animated\n              ? edit.upsertKeyframe(\n                  clip.id,\n                  \"position\",\n                  activeFrame,\n                  positionYKeyframeValue(sampledTopLeft.x, v),\n                )\n              : commit({ transform: { ...clip.transform, centerY: v + h / 2 } })\n          } -> if (animated) edit.upsertKeyframe(clip.id, 'position', activeFrame, positionYKeyframeValue(sampledTopLeft.x, v)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, centerY: v + h / 2 } }) via commit","web/src/store/editActions.ts::upsertKeyframe('position', positionYKeyframeValue) or setClipProperties({transform.centerY})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::UpsertKeyframe or SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#upsertKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=clip position Y: assert exactly if (animated) edit.upsertKeyframe(clip.id, 'position', activeFrame, positionYKeyframeValue(sampledTopLeft.x, v)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, centerY: v + h / 2 } }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"clip position Y: assert exactly if (animated) edit.upsertKeyframe(clip.id, 'position', activeFrame, positionYKeyframeValue(sampledTopLeft.x, v)); else edit.setClipProperties([clip.id], { transform: { ...clip.transform, centerY: v + h / 2 } }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-5debc87819b3e009

- Candidate/source: `control-f8d159f2ffe6aa33` at `web/src/components/inspector/Inspector.tsx:775:7` (control)
- Expected behavior: crop edge value: assert exactly for this rendered edge, if (animated) edit.upsertKeyframe(clip.id, 'crop', activeFrame, cropEdgeKeyframeValue(sampledCrop, edge, v)); else edit.setClipProperties([clip.id], { crop: { ...clip.crop, [edge]: v } }) via commitEdge(edge, v) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-f8d159f2ffe6aa33.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-f8d159f2ffe6aa33 crop edge value.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) =>
          animated
            ? edit.upsertKeyframe(
                clip.id,
                "crop",
                activeFrame,
                cropEdgeKeyframeValue(sampledCrop, edge, v),
              )
            : commitEdge(edge, v)
        }.
  - Exact call/state/backend: stateTransition=crop edge value: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:775::handler {(v) =>\n          animated\n            ? edit.upsertKeyframe(\n                clip.id,\n                \"crop\",\n                activeFrame,\n                cropEdgeKeyframeValue(sampledCrop, edge, v),\n              )\n            : commitEdge(edge, v)\n        } -> for this rendered edge, if (animated) edit.upsertKeyframe(clip.id, 'crop', activeFrame, cropEdgeKeyframeValue(sampledCrop, edge, v)); else edit.setClipProperties([clip.id], { crop: { ...clip.crop, [edge]: v } }) via commitEdge(edge, v)","web/src/store/editActions.ts::upsertKeyframe('crop', cropEdgeKeyframeValue) or setClipProperties({crop})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::UpsertKeyframe or SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#upsertKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=crop edge value: assert exactly for this rendered edge, if (animated) edit.upsertKeyframe(clip.id, 'crop', activeFrame, cropEdgeKeyframeValue(sampledCrop, edge, v)); else edit.setClipProperties([clip.id], { crop: { ...clip.crop, [edge]: v } }) via commitEdge(edge, v) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"crop edge value: assert exactly for this rendered edge, if (animated) edit.upsertKeyframe(clip.id, 'crop', activeFrame, cropEdgeKeyframeValue(sampledCrop, edge, v)); else edit.setClipProperties([clip.id], { crop: { ...clip.crop, [edge]: v } }) via commitEdge(edge, v) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-90b6142c9b28af54 clip volume` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-08376197cc04d5c7 clip scale` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-081219e1fb457cfe clip rotation` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-16114827f9ea0a12 clip opacity` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-cd756dccc56343eb clip position X` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-11ebd75be0f93008 clip position Y` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-f8d159f2ffe6aa33 crop edge value` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-90b6142c9b28af54 clip volume"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-08376197cc04d5c7 clip scale"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-081219e1fb457cfe clip rotation"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-16114827f9ea0a12 clip opacity"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-cd756dccc56343eb clip position X"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-11ebd75be0f93008 clip position Y"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-f8d159f2ffe6aa33 crop edge value"`

  Expected: FAIL because one or more of the 7 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/store/editActions.ts#upsertKeyframe`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::UpsertKeyframe`, `web/src/components/inspector/Inspector.tsx#Inspector`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-90b6142c9b28af54 clip volume"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-08376197cc04d5c7 clip scale"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-081219e1fb457cfe clip rotation"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-16114827f9ea0a12 clip opacity"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-cd756dccc56343eb clip position X"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-11ebd75be0f93008 clip position Y"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-f8d159f2ffe6aa33 crop edge value"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 15: control-acceptance (implementation-slice-3e5a047348756ee3)

**Covered records:**
- `control-record-05de57e42c104ddc` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/store/editActions.ts#resetTransform`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::ResetTransform`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-96a5dfea7df05ba9 reset transform`

**Candidate-bound contracts:**

#### control-record-05de57e42c104ddc

- Candidate/source: `control-96a5dfea7df05ba9` at `web/src/components/inspector/Inspector.tsx:500:17` (control)
- Expected behavior: reset transform: assert exactly edit.resetTransform([clip.id]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-96a5dfea7df05ba9.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-96a5dfea7df05ba9 reset transform.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => edit.resetTransform([clip.id])}.
  - Exact call/state/backend: stateTransition=reset transform: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:500::handler {() => edit.resetTransform([clip.id])} -> edit.resetTransform([clip.id])","web/src/store/editActions.ts::resetTransform([clip.id])","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::ResetTransform","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#resetTransform","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=reset transform: assert exactly edit.resetTransform([clip.id]) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"inspector.action.resetTransform\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"reset transform: assert exactly edit.resetTransform([clip.id]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-96a5dfea7df05ba9 reset transform` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-96a5dfea7df05ba9 reset transform"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/store/editActions.ts#resetTransform`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::ResetTransform`, `web/src/components/inspector/Inspector.tsx#Inspector`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-96a5dfea7df05ba9 reset transform"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 16: control-acceptance (implementation-slice-04b713e96776e0d9)

**Covered records:**
- `control-record-fef29d3f2f2b843c` (control)
- `control-record-2c7a2cf4f9262786` (control)
- `control-record-c55824ea009f70b5` (control)
- `control-record-36f11fcd72093874` (control)
- `control-record-da0243da2da5d7ae` (control)
- `control-record-3b0dca4679aaac13` (control)
- `control-record-9daae7306ad8edf3` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/store/editActions.ts#setClipProperties`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetClipProperties`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-bcede743dbdd39c4 clip speed`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-8e3c200b9433f78e flip horizontal`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-c188975bcfe4ec9b flip vertical`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-1e99a2bcbd62178e fade-in frames`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-870eaf3988273d41 fade-in interpolation`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-82d3756769e4c60c fade-out frames`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-eda8168f82218100 fade-out interpolation`

**Candidate-bound contracts:**

#### control-record-fef29d3f2f2b843c

- Candidate/source: `control-bcede743dbdd39c4` at `web/src/components/inspector/Inspector.tsx:602:17` (control)
- Expected behavior: clip speed: assert exactly edit.setClipProperties([clip.id], { speed: v }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-bcede743dbdd39c4.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-bcede743dbdd39c4 clip speed.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => commit({ speed: v })}.
  - Exact call/state/backend: stateTransition=clip speed: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:602::handler {(v) => commit({ speed: v })} -> edit.setClipProperties([clip.id], { speed: v }) via commit","web/src/store/editActions.ts::setClipProperties({speed})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=clip speed: assert exactly edit.setClipProperties([clip.id], { speed: v }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"clip speed: assert exactly edit.setClipProperties([clip.id], { speed: v }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-2c7a2cf4f9262786

- Candidate/source: `control-8e3c200b9433f78e` at `web/src/components/inspector/Inspector.tsx:858:9` (control)
- Expected behavior: flip horizontal: assert exactly edit.setClipProperties([clip.id], { flipHorizontal: e.target.checked }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-8e3c200b9433f78e.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-8e3c200b9433f78e flip horizontal.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["changed value","blur/keyboard event when present","current candidate-specific model state"]; handler={(e) => commit({ flipHorizontal: e.target.checked })}.
  - Exact call/state/backend: stateTransition=flip horizontal: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:858::handler {(e) => commit({ flipHorizontal: e.target.checked })} -> edit.setClipProperties([clip.id], { flipHorizontal: e.target.checked }) via commit","web/src/store/editActions.ts::setClipProperties({flipHorizontal})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=flip horizontal: assert exactly edit.setClipProperties([clip.id], { flipHorizontal: e.target.checked }) via commit and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"flip horizontal: assert exactly edit.setClipProperties([clip.id], { flipHorizontal: e.target.checked }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-c55824ea009f70b5

- Candidate/source: `control-c188975bcfe4ec9b` at `web/src/components/inspector/Inspector.tsx:866:9` (control)
- Expected behavior: flip vertical: assert exactly edit.setClipProperties([clip.id], { flipVertical: e.target.checked }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c188975bcfe4ec9b.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-c188975bcfe4ec9b flip vertical.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["changed value","blur/keyboard event when present","current candidate-specific model state"]; handler={(e) => commit({ flipVertical: e.target.checked })}.
  - Exact call/state/backend: stateTransition=flip vertical: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:866::handler {(e) => commit({ flipVertical: e.target.checked })} -> edit.setClipProperties([clip.id], { flipVertical: e.target.checked }) via commit","web/src/store/editActions.ts::setClipProperties({flipVertical})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=flip vertical: assert exactly edit.setClipProperties([clip.id], { flipVertical: e.target.checked }) via commit and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"flip vertical: assert exactly edit.setClipProperties([clip.id], { flipVertical: e.target.checked }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-36f11fcd72093874

- Candidate/source: `control-1e99a2bcbd62178e` at `web/src/components/inspector/Inspector.tsx:892:9` (control)
- Expected behavior: fade-in frames: assert exactly edit.setClipProperties([clip.id], { fadeInFrames: Math.round(v) }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-1e99a2bcbd62178e.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-1e99a2bcbd62178e fade-in frames.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => commit({ fadeInFrames: Math.round(v) })}.
  - Exact call/state/backend: stateTransition=fade-in frames: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:892::handler {(v) => commit({ fadeInFrames: Math.round(v) })} -> edit.setClipProperties([clip.id], { fadeInFrames: Math.round(v) }) via commit","web/src/store/editActions.ts::setClipProperties({fadeInFrames})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=fade-in frames: assert exactly edit.setClipProperties([clip.id], { fadeInFrames: Math.round(v) }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"fade-in frames: assert exactly edit.setClipProperties([clip.id], { fadeInFrames: Math.round(v) }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-da0243da2da5d7ae

- Candidate/source: `control-870eaf3988273d41` at `web/src/components/inspector/Inspector.tsx:903:9` (control)
- Expected behavior: fade-in interpolation: assert exactly edit.setClipProperties([clip.id], { fadeInInterpolation: v }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-870eaf3988273d41.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-870eaf3988273d41 fade-in interpolation.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => commit({ fadeInInterpolation: v })}.
  - Exact call/state/backend: stateTransition=fade-in interpolation: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:903::handler {(v) => commit({ fadeInInterpolation: v })} -> edit.setClipProperties([clip.id], { fadeInInterpolation: v }) via commit","web/src/store/editActions.ts::setClipProperties({fadeInInterpolation})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=fade-in interpolation: assert exactly edit.setClipProperties([clip.id], { fadeInInterpolation: v }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"fade-in interpolation: assert exactly edit.setClipProperties([clip.id], { fadeInInterpolation: v }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-3b0dca4679aaac13

- Candidate/source: `control-82d3756769e4c60c` at `web/src/components/inspector/Inspector.tsx:910:9` (control)
- Expected behavior: fade-out frames: assert exactly edit.setClipProperties([clip.id], { fadeOutFrames: Math.round(v) }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-82d3756769e4c60c.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-82d3756769e4c60c fade-out frames.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => commit({ fadeOutFrames: Math.round(v) })}.
  - Exact call/state/backend: stateTransition=fade-out frames: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:910::handler {(v) => commit({ fadeOutFrames: Math.round(v) })} -> edit.setClipProperties([clip.id], { fadeOutFrames: Math.round(v) }) via commit","web/src/store/editActions.ts::setClipProperties({fadeOutFrames})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=fade-out frames: assert exactly edit.setClipProperties([clip.id], { fadeOutFrames: Math.round(v) }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"fade-out frames: assert exactly edit.setClipProperties([clip.id], { fadeOutFrames: Math.round(v) }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-9daae7306ad8edf3

- Candidate/source: `control-eda8168f82218100` at `web/src/components/inspector/Inspector.tsx:921:9` (control)
- Expected behavior: fade-out interpolation: assert exactly edit.setClipProperties([clip.id], { fadeOutInterpolation: v }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-eda8168f82218100.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-eda8168f82218100 fade-out interpolation.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => commit({ fadeOutInterpolation: v })}.
  - Exact call/state/backend: stateTransition=fade-out interpolation: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:921::handler {(v) => commit({ fadeOutInterpolation: v })} -> edit.setClipProperties([clip.id], { fadeOutInterpolation: v }) via commit","web/src/store/editActions.ts::setClipProperties({fadeOutInterpolation})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=fade-out interpolation: assert exactly edit.setClipProperties([clip.id], { fadeOutInterpolation: v }) via commit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"fade-out interpolation: assert exactly edit.setClipProperties([clip.id], { fadeOutInterpolation: v }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-bcede743dbdd39c4 clip speed` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-8e3c200b9433f78e flip horizontal` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-c188975bcfe4ec9b flip vertical` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-1e99a2bcbd62178e fade-in frames` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-870eaf3988273d41 fade-in interpolation` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-82d3756769e4c60c fade-out frames` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-eda8168f82218100 fade-out interpolation` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-bcede743dbdd39c4 clip speed"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-8e3c200b9433f78e flip horizontal"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-c188975bcfe4ec9b flip vertical"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-1e99a2bcbd62178e fade-in frames"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-870eaf3988273d41 fade-in interpolation"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-82d3756769e4c60c fade-out frames"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-eda8168f82218100 fade-out interpolation"`

  Expected: FAIL because one or more of the 7 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/store/editActions.ts#setClipProperties`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetClipProperties`, `web/src/components/inspector/Inspector.tsx#Inspector`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-bcede743dbdd39c4 clip speed"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-8e3c200b9433f78e flip horizontal"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-c188975bcfe4ec9b flip vertical"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-1e99a2bcbd62178e fade-in frames"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-870eaf3988273d41 fade-in interpolation"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-82d3756769e4c60c fade-out frames"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-eda8168f82218100 fade-out interpolation"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 17: control-acceptance (implementation-slice-6070f12f5be0e774)

**Covered records:**
- `control-record-bde8ad099e2de905` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/components/inspector/Inspector.tsx#CropSection`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-23078cfa22cdc9a5 start or stop on-canvas crop editing`

**Candidate-bound contracts:**

#### control-record-bde8ad099e2de905

- Candidate/source: `control-23078cfa22cdc9a5` at `web/src/components/inspector/Inspector.tsx:800:9` (control)
- Expected behavior: start or stop on-canvas crop editing: assert exactly toggleCropEditingActive() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-23078cfa22cdc9a5.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-23078cfa22cdc9a5 start or stop on-canvas crop editing.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={toggleCropEditingActive}.
  - Exact call/state/backend: stateTransition=start or stop on-canvas crop editing: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:800::handler {toggleCropEditingActive} -> toggleCropEditingActive()","web/src/components/inspector/Inspector.tsx::CropSection -> uiStore.toggleCropEditingActive","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/components/inspector/Inspector.tsx#CropSection"].
  - Visible/accessibility/return path: success=start or stop on-canvas crop editing: assert exactly toggleCropEditingActive() and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\n            cropEditingActive ? \"inspector.action.cropEditStop\" : \"inspector.action.cropEditStart\",\n          ).","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"start or stop on-canvas crop editing: assert exactly toggleCropEditingActive() and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-23078cfa22cdc9a5 start or stop on-canvas crop editing` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-23078cfa22cdc9a5 start or stop on-canvas crop editing"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/components/inspector/Inspector.tsx#CropSection`, `web/src/components/inspector/Inspector.tsx#Inspector` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-23078cfa22cdc9a5 start or stop on-canvas crop editing"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 18: control-acceptance (implementation-slice-f69934517ab49b1f)

**Covered records:**
- `control-record-48ee4f79448f3358` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/components/inspector/Inspector.tsx#applyCropPreset`
- Modify: `web/src/store/uiStore.ts#setCropAspectLock`
- Modify: `web/src/lib/cropOverlay.ts#cropForPreset`
- Modify: `web/src/store/editActions.ts#upsertKeyframe`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::UpsertKeyframe`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-5c3e9ca9c1713a09 crop aspect preset`

**Candidate-bound contracts:**

#### control-record-48ee4f79448f3358

- Candidate/source: `control-5c3e9ca9c1713a09` at `web/src/components/inspector/Inspector.tsx:810:9` (control)
- Expected behavior: crop aspect preset: assert exactly setCropAspectLock(preset); compute cropForPreset(preset, sourcePixelAspect); if result is null stop; else if (animated) edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }); else edit.setClipProperties([clip.id], { crop: next }) via commit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-5c3e9ca9c1713a09.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-5c3e9ca9c1713a09 crop aspect preset.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["selected option value","current candidate-specific model state"]; handler={(e) => applyCropPreset(e.target.value as CropAspectLock)}.
  - Exact call/state/backend: stateTransition=crop aspect preset: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:810::handler {(e) => applyCropPreset(e.target.value as CropAspectLock)} -> setCropAspectLock(preset); compute cropForPreset(preset, sourcePixelAspect); if result is null stop; else if (animated) edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }); else edit.setClipProperties([clip.id], { crop: next }) via commit","web/src/components/inspector/Inspector.tsx::applyCropPreset","web/src/store/uiStore.ts::setCropAspectLock","web/src/lib/cropOverlay.ts::cropForPreset","web/src/store/editActions.ts::upsertKeyframe('crop') or setClipProperties({crop})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::UpsertKeyframe or SetClipProperties","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/components/inspector/Inspector.tsx#applyCropPreset","code:web/src/lib/cropOverlay.ts#cropForPreset","code:web/src/store/editActions.ts#upsertKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=crop aspect preset: assert exactly setCropAspectLock(preset); compute cropForPreset(preset, sourcePixelAspect); if result is null stop; else if (animated) edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }); else edit.setClipProperties([clip.id], { crop: next }) via commit and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"inspector.field.cropAspect\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"crop aspect preset: assert exactly setCropAspectLock(preset); compute cropForPreset(preset, sourcePixelAspect); if result is null stop; else if (animated) edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }); else edit.setClipProperties([clip.id], { crop: next }) via commit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-5c3e9ca9c1713a09 crop aspect preset` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-5c3e9ca9c1713a09 crop aspect preset"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/components/inspector/Inspector.tsx#applyCropPreset`, `web/src/store/uiStore.ts#setCropAspectLock`, `web/src/lib/cropOverlay.ts#cropForPreset`, `web/src/store/editActions.ts#upsertKeyframe`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::UpsertKeyframe`, `web/src/components/inspector/Inspector.tsx#Inspector`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-5c3e9ca9c1713a09 crop aspect preset"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 19: control-acceptance (implementation-slice-6658fe8741882df0)

**Covered records:**
- `control-record-110dc21768d376aa` (control)
- `control-record-5b07b4e6bbbc0d4d` (control)
- `control-record-9d67d93bf280ea66` (control)
- `control-record-885fa4706b7998c2` (control)
- `control-record-4cf20d3e9252910b` (control)
- `control-record-de7f5fb083778ebc` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/store/editActions.ts#setColorGrade`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetColorGrade`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-483d3b87f9c031f5 color-grade exposure`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-d613004ab99583b3 color-grade temperature`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-be09ebcb1166277c color-grade tint`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-d9ed469bc367deec lift/gamma/gain RGB channels`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-e5ff7853d43491f6 color-grade contrast`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-58ec257f601a9924 color-grade saturation`

**Candidate-bound contracts:**

#### control-record-110dc21768d376aa

- Candidate/source: `control-483d3b87f9c031f5` at `web/src/components/inspector/Inspector.tsx:984:7` (control)
- Expected behavior: color-grade exposure: assert exactly onChange updateField('exposure', v) updates draft; onCommit commitGrade({ ...draft, exposure: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-483d3b87f9c031f5.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-483d3b87f9c031f5 color-grade exposure.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updateField("exposure", v)} {(v) => commitField("exposure", v)}.
  - Exact call/state/backend: stateTransition=color-grade exposure: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:984::handler {(v) => updateField(\"exposure\", v)} {(v) => commitField(\"exposure\", v)} -> onChange updateField('exposure', v) updates draft; onCommit commitGrade({ ...draft, exposure: v }) then edit.setColorGrade([clip.id], next)","web/src/store/editActions.ts::setColorGrade({exposure})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetColorGrade","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setColorGrade","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=color-grade exposure: assert exactly onChange updateField('exposure', v) updates draft; onCommit commitGrade({ ...draft, exposure: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"color-grade exposure: assert exactly onChange updateField('exposure', v) updates draft; onCommit commitGrade({ ...draft, exposure: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-5b07b4e6bbbc0d4d

- Candidate/source: `control-d613004ab99583b3` at `web/src/components/inspector/Inspector.tsx:994:7` (control)
- Expected behavior: color-grade temperature: assert exactly onChange updateField('temperature', v) updates draft; onCommit commitGrade({ ...draft, temperature: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d613004ab99583b3.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-d613004ab99583b3 color-grade temperature.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updateField("temperature", v)} {(v) => commitField("temperature", v)}.
  - Exact call/state/backend: stateTransition=color-grade temperature: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:994::handler {(v) => updateField(\"temperature\", v)} {(v) => commitField(\"temperature\", v)} -> onChange updateField('temperature', v) updates draft; onCommit commitGrade({ ...draft, temperature: v }) then edit.setColorGrade([clip.id], next)","web/src/store/editActions.ts::setColorGrade({temperature})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetColorGrade","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setColorGrade","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=color-grade temperature: assert exactly onChange updateField('temperature', v) updates draft; onCommit commitGrade({ ...draft, temperature: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"color-grade temperature: assert exactly onChange updateField('temperature', v) updates draft; onCommit commitGrade({ ...draft, temperature: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-9d67d93bf280ea66

- Candidate/source: `control-be09ebcb1166277c` at `web/src/components/inspector/Inspector.tsx:1004:7` (control)
- Expected behavior: color-grade tint: assert exactly onChange updateField('tint', v) updates draft; onCommit commitGrade({ ...draft, tint: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-be09ebcb1166277c.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-be09ebcb1166277c color-grade tint.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updateField("tint", v)} {(v) => commitField("tint", v)}.
  - Exact call/state/backend: stateTransition=color-grade tint: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1004::handler {(v) => updateField(\"tint\", v)} {(v) => commitField(\"tint\", v)} -> onChange updateField('tint', v) updates draft; onCommit commitGrade({ ...draft, tint: v }) then edit.setColorGrade([clip.id], next)","web/src/store/editActions.ts::setColorGrade({tint})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetColorGrade","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setColorGrade","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=color-grade tint: assert exactly onChange updateField('tint', v) updates draft; onCommit commitGrade({ ...draft, tint: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"color-grade tint: assert exactly onChange updateField('tint', v) updates draft; onCommit commitGrade({ ...draft, tint: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-885fa4706b7998c2

- Candidate/source: `control-d9ed469bc367deec` at `web/src/components/inspector/Inspector.tsx:1016:11` (control)
- Expected behavior: lift/gamma/gain RGB channels: assert exactly for the clicked rendered (band, channel), onChange updateLgg(band, channel, v) updates draft; onCommit commitGrade(setLggChannel(draft, band, channel, v)) then edit.setColorGrade([clip.id], next) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d9ed469bc367deec.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-d9ed469bc367deec lift/gamma/gain RGB channels.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updateLgg(band, channel, v)} {(v) => commitLgg(band, channel, v)}.
  - Exact call/state/backend: stateTransition=lift/gamma/gain RGB channels: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1016::handler {(v) => updateLgg(band, channel, v)} {(v) => commitLgg(band, channel, v)} -> for the clicked rendered (band, channel), onChange updateLgg(band, channel, v) updates draft; onCommit commitGrade(setLggChannel(draft, band, channel, v)) then edit.setColorGrade([clip.id], next)","web/src/store/editActions.ts::setColorGrade(setLggChannel for exact rendered band/channel)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetColorGrade","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setColorGrade","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=lift/gamma/gain RGB channels: assert exactly for the clicked rendered (band, channel), onChange updateLgg(band, channel, v) updates draft; onCommit commitGrade(setLggChannel(draft, band, channel, v)) then edit.setColorGrade([clip.id], next) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"lift/gamma/gain RGB channels: assert exactly for the clicked rendered (band, channel), onChange updateLgg(band, channel, v) updates draft; onCommit commitGrade(setLggChannel(draft, band, channel, v)) then edit.setColorGrade([clip.id], next) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-4cf20d3e9252910b

- Candidate/source: `control-e5ff7853d43491f6` at `web/src/components/inspector/Inspector.tsx:1030:7` (control)
- Expected behavior: color-grade contrast: assert exactly onChange updateField('contrast', v) updates draft; onCommit commitGrade({ ...draft, contrast: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-e5ff7853d43491f6.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-e5ff7853d43491f6 color-grade contrast.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updateField("contrast", v)} {(v) => commitField("contrast", v)}.
  - Exact call/state/backend: stateTransition=color-grade contrast: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1030::handler {(v) => updateField(\"contrast\", v)} {(v) => commitField(\"contrast\", v)} -> onChange updateField('contrast', v) updates draft; onCommit commitGrade({ ...draft, contrast: v }) then edit.setColorGrade([clip.id], next)","web/src/store/editActions.ts::setColorGrade({contrast})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetColorGrade","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setColorGrade","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=color-grade contrast: assert exactly onChange updateField('contrast', v) updates draft; onCommit commitGrade({ ...draft, contrast: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"color-grade contrast: assert exactly onChange updateField('contrast', v) updates draft; onCommit commitGrade({ ...draft, contrast: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-de7f5fb083778ebc

- Candidate/source: `control-58ec257f601a9924` at `web/src/components/inspector/Inspector.tsx:1040:7` (control)
- Expected behavior: color-grade saturation: assert exactly onChange updateField('saturation', v) updates draft; onCommit commitGrade({ ...draft, saturation: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-58ec257f601a9924.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-58ec257f601a9924 color-grade saturation.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updateField("saturation", v)} {(v) => commitField("saturation", v)}.
  - Exact call/state/backend: stateTransition=color-grade saturation: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1040::handler {(v) => updateField(\"saturation\", v)} {(v) => commitField(\"saturation\", v)} -> onChange updateField('saturation', v) updates draft; onCommit commitGrade({ ...draft, saturation: v }) then edit.setColorGrade([clip.id], next)","web/src/store/editActions.ts::setColorGrade({saturation})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetColorGrade","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setColorGrade","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=color-grade saturation: assert exactly onChange updateField('saturation', v) updates draft; onCommit commitGrade({ ...draft, saturation: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"color-grade saturation: assert exactly onChange updateField('saturation', v) updates draft; onCommit commitGrade({ ...draft, saturation: v }) then edit.setColorGrade([clip.id], next) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-483d3b87f9c031f5 color-grade exposure` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-d613004ab99583b3 color-grade temperature` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-be09ebcb1166277c color-grade tint` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-d9ed469bc367deec lift/gamma/gain RGB channels` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-e5ff7853d43491f6 color-grade contrast` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-58ec257f601a9924 color-grade saturation` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-483d3b87f9c031f5 color-grade exposure"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-d613004ab99583b3 color-grade temperature"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-be09ebcb1166277c color-grade tint"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-d9ed469bc367deec lift/gamma/gain RGB channels"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-e5ff7853d43491f6 color-grade contrast"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-58ec257f601a9924 color-grade saturation"`

  Expected: FAIL because one or more of the 6 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/store/editActions.ts#setColorGrade`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetColorGrade`, `web/src/components/inspector/Inspector.tsx#Inspector`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-483d3b87f9c031f5 color-grade exposure"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-d613004ab99583b3 color-grade temperature"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-be09ebcb1166277c color-grade tint"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-d9ed469bc367deec lift/gamma/gain RGB channels"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-e5ff7853d43491f6 color-grade contrast"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-58ec257f601a9924 color-grade saturation"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 20: control-acceptance (implementation-slice-213c6ebc649a8507)

**Covered records:**
- `control-record-e2dc11ccd269093f` (control)
- `control-record-a7f4988fdf17e70b` (control)
- `control-record-679cbe8e200cd71f` (control)
- `control-record-44ecdc8071f40679` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/store/editActions.ts#setChromaKey`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetChromaKey`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-d31487647c9706fc enable chroma key`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-40101f7c25e4854c chroma-key similarity`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-10bd77bd011e8aaf chroma-key smoothness`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-ee79ef68fe15ab96 chroma-key spill suppression`

**Candidate-bound contracts:**

#### control-record-e2dc11ccd269093f

- Candidate/source: `control-d31487647c9706fc` at `web/src/components/inspector/Inspector.tsx:1086:9` (control)
- Expected behavior: enable chroma key: assert exactly setEnabled(nextEnabled); if true setDraft(completeChromaKey(clip.chromaKey)) and edit.setChromaKey([clip.id], next); if false edit.setChromaKey([clip.id], null) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d31487647c9706fc.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-d31487647c9706fc enable chroma key.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["changed value","blur/keyboard event when present","current candidate-specific model state"]; handler={(e) => setKeyEnabled(e.target.checked)}.
  - Exact call/state/backend: stateTransition=enable chroma key: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1086::handler {(e) => setKeyEnabled(e.target.checked)} -> setEnabled(nextEnabled); if true setDraft(completeChromaKey(clip.chromaKey)) and edit.setChromaKey([clip.id], next); if false edit.setChromaKey([clip.id], null)","web/src/store/editActions.ts::setChromaKey(next or null)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetChromaKey","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setChromaKey","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=enable chroma key: assert exactly setEnabled(nextEnabled); if true setDraft(completeChromaKey(clip.chromaKey)) and edit.setChromaKey([clip.id], next); if false edit.setChromaKey([clip.id], null) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"enable chroma key: assert exactly setEnabled(nextEnabled); if true setDraft(completeChromaKey(clip.chromaKey)) and edit.setChromaKey([clip.id], next); if false edit.setChromaKey([clip.id], null) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-a7f4988fdf17e70b

- Candidate/source: `control-40101f7c25e4854c` at `web/src/components/inspector/Inspector.tsx:1113:11` (control)
- Expected behavior: chroma-key similarity: assert exactly onChange updateField('similarity', v) updates draft; onCommit commitKey({ ...draft, similarity: v }), which calls edit.setChromaKey([clip.id], next) only while enabled and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-40101f7c25e4854c.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-40101f7c25e4854c chroma-key similarity.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updateField("similarity", v)} {(v) => commitField("similarity", v)}.
  - Exact call/state/backend: stateTransition=chroma-key similarity: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1113::handler {(v) => updateField(\"similarity\", v)} {(v) => commitField(\"similarity\", v)} -> onChange updateField('similarity', v) updates draft; onCommit commitKey({ ...draft, similarity: v }), which calls edit.setChromaKey([clip.id], next) only while enabled","web/src/store/editActions.ts::setChromaKey({similarity}) when enabled","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetChromaKey","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setChromaKey","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=chroma-key similarity: assert exactly onChange updateField('similarity', v) updates draft; onCommit commitKey({ ...draft, similarity: v }), which calls edit.setChromaKey([clip.id], next) only while enabled and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"chroma-key similarity: assert exactly onChange updateField('similarity', v) updates draft; onCommit commitKey({ ...draft, similarity: v }), which calls edit.setChromaKey([clip.id], next) only while enabled and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-679cbe8e200cd71f

- Candidate/source: `control-10bd77bd011e8aaf` at `web/src/components/inspector/Inspector.tsx:1123:11` (control)
- Expected behavior: chroma-key smoothness: assert exactly onChange updateField('smoothness', v) updates draft; onCommit commitKey({ ...draft, smoothness: v }), which calls edit.setChromaKey([clip.id], next) only while enabled and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-10bd77bd011e8aaf.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-10bd77bd011e8aaf chroma-key smoothness.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updateField("smoothness", v)} {(v) => commitField("smoothness", v)}.
  - Exact call/state/backend: stateTransition=chroma-key smoothness: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1123::handler {(v) => updateField(\"smoothness\", v)} {(v) => commitField(\"smoothness\", v)} -> onChange updateField('smoothness', v) updates draft; onCommit commitKey({ ...draft, smoothness: v }), which calls edit.setChromaKey([clip.id], next) only while enabled","web/src/store/editActions.ts::setChromaKey({smoothness}) when enabled","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetChromaKey","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setChromaKey","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=chroma-key smoothness: assert exactly onChange updateField('smoothness', v) updates draft; onCommit commitKey({ ...draft, smoothness: v }), which calls edit.setChromaKey([clip.id], next) only while enabled and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"chroma-key smoothness: assert exactly onChange updateField('smoothness', v) updates draft; onCommit commitKey({ ...draft, smoothness: v }), which calls edit.setChromaKey([clip.id], next) only while enabled and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-44ecdc8071f40679

- Candidate/source: `control-ee79ef68fe15ab96` at `web/src/components/inspector/Inspector.tsx:1133:11` (control)
- Expected behavior: chroma-key spill suppression: assert exactly onChange updateField('spill', v) updates draft; onCommit commitKey({ ...draft, spill: v }), which calls edit.setChromaKey([clip.id], next) only while enabled and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-ee79ef68fe15ab96.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-ee79ef68fe15ab96 chroma-key spill suppression.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updateField("spill", v)} {(v) => commitField("spill", v)}.
  - Exact call/state/backend: stateTransition=chroma-key spill suppression: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1133::handler {(v) => updateField(\"spill\", v)} {(v) => commitField(\"spill\", v)} -> onChange updateField('spill', v) updates draft; onCommit commitKey({ ...draft, spill: v }), which calls edit.setChromaKey([clip.id], next) only while enabled","web/src/store/editActions.ts::setChromaKey({spill}) when enabled","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetChromaKey","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setChromaKey","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=chroma-key spill suppression: assert exactly onChange updateField('spill', v) updates draft; onCommit commitKey({ ...draft, spill: v }), which calls edit.setChromaKey([clip.id], next) only while enabled and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"chroma-key spill suppression: assert exactly onChange updateField('spill', v) updates draft; onCommit commitKey({ ...draft, spill: v }), which calls edit.setChromaKey([clip.id], next) only while enabled and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-d31487647c9706fc enable chroma key` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-40101f7c25e4854c chroma-key similarity` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-10bd77bd011e8aaf chroma-key smoothness` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-ee79ef68fe15ab96 chroma-key spill suppression` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-d31487647c9706fc enable chroma key"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-40101f7c25e4854c chroma-key similarity"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-10bd77bd011e8aaf chroma-key smoothness"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-ee79ef68fe15ab96 chroma-key spill suppression"`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/store/editActions.ts#setChromaKey`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetChromaKey`, `web/src/components/inspector/Inspector.tsx#Inspector`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-d31487647c9706fc enable chroma key"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-40101f7c25e4854c chroma-key similarity"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-10bd77bd011e8aaf chroma-key smoothness"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-ee79ef68fe15ab96 chroma-key spill suppression"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 21: control-acceptance (implementation-slice-bac161d50ef83c28)

**Covered records:**
- `control-record-7f7a127fc222b6f6` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/components/inspector/Inspector.tsx#keyColor`
- Modify: `web/src/store/editActions.ts#setChromaKey`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetChromaKey`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-a250e4a089de997f chroma key color`

**Candidate-bound contracts:**

#### control-record-7f7a127fc222b6f6

- Candidate/source: `control-a250e4a089de997f` at `web/src/components/inspector/Inspector.tsx:1096:13` (control)
- Expected behavior: chroma key color: assert exactly onChange setDraft({ ...draft, keyColor: hexToRgb(e.target.value) }); onBlur commitKey(draft), which calls edit.setChromaKey([clip.id], draft) only while enabled and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-a250e4a089de997f.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-a250e4a089de997f chroma key color.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["changed value","blur/keyboard event when present","current candidate-specific model state"]; handler={(e) => setDraft((k) => ({ ...k, keyColor: hexToRgb(e.target.value) }))} {() => commitKey(draft)}.
  - Exact call/state/backend: stateTransition=chroma key color: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1096::handler {(e) => setDraft((k) => ({ ...k, keyColor: hexToRgb(e.target.value) }))} {() => commitKey(draft)} -> onChange setDraft({ ...draft, keyColor: hexToRgb(e.target.value) }); onBlur commitKey(draft), which calls edit.setChromaKey([clip.id], draft) only while enabled","web/src/components/inspector/Inspector.tsx::keyColor onChange/onBlur -> commitKey","web/src/store/editActions.ts::setChromaKey(draft) when enabled","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetChromaKey","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setChromaKey","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=chroma key color: assert exactly onChange setDraft({ ...draft, keyColor: hexToRgb(e.target.value) }); onBlur commitKey(draft), which calls edit.setChromaKey([clip.id], draft) only while enabled and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"inspector.field.keyColor\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"chroma key color: assert exactly onChange setDraft({ ...draft, keyColor: hexToRgb(e.target.value) }); onBlur commitKey(draft), which calls edit.setChromaKey([clip.id], draft) only while enabled and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-a250e4a089de997f chroma key color` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-a250e4a089de997f chroma key color"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/components/inspector/Inspector.tsx#keyColor`, `web/src/store/editActions.ts#setChromaKey`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetChromaKey`, `web/src/components/inspector/Inspector.tsx#Inspector`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-a250e4a089de997f chroma key color"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 22: control-acceptance (implementation-slice-d2526a0c70c8dc7c)

**Covered records:**
- `control-record-04a1450213ff02c1` (control)
- `control-record-eac5d871d27e236a` (control)
- `control-record-69d6b02c48cfb78d` (control)
- `control-record-d3d7ec4b0e4874af` (control)
- `control-record-6738ba03c29040df` (control)
- `control-record-76852c7be063d308` (control)
- `control-record-6235c98a7ee867a1` (control)
- `control-record-dcde418d02888350` (control)
- `control-record-8eaff7c86e495e34` (control)
- `control-record-6affc7b83553bf04` (control)
- `control-record-6ddc3ebc24250e06` (control)
- `control-record-54962d9a3382feda` (control)

**Files:**
- Modify: `web/src/components/inspector/Inspector.tsx`
- Modify: `web/src/store/editActions.ts#setMasks`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetMasks`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-8b0f987c8bf46a8a enable mask`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-3f99a53815fd9130 mask shape type`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-0c7a02c6dae89f87 mask feather`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-b77d7a7eb91fedfa invert mask`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-fd4203443df4adcb circle-mask center X`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-18136ab8e305f7a7 circle-mask center Y`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-09ca7bab665f84ed circle-mask radius X`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-9e9cf7fd25863d20 circle-mask radius Y`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-47bf4ee974f14505 linear-mask point X`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-4b61e7b97ade8478 linear-mask point Y`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-bc82a346093bae7c linear-mask normal X`
- Test (reviewed-planned): `web/src/components/inspector/Inspector.interaction.test.tsx#control-d60d607d7448918f linear-mask normal Y`

**Candidate-bound contracts:**

#### control-record-04a1450213ff02c1

- Candidate/source: `control-8b0f987c8bf46a8a` at `web/src/components/inspector/Inspector.tsx:1182:9` (control)
- Expected behavior: enable mask: assert exactly setEnabled(nextEnabled); if true edit.setMasks([clip.id], [completeMask(clip.masks?.[0]), ...(clip.masks?.slice(1) ?? [])]); if false edit.setMasks([clip.id], []) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-8b0f987c8bf46a8a.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-8b0f987c8bf46a8a enable mask.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["changed value","blur/keyboard event when present","current candidate-specific model state"]; handler={(e) => setMaskEnabled(e.target.checked)}.
  - Exact call/state/backend: stateTransition=enable mask: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1182::handler {(e) => setMaskEnabled(e.target.checked)} -> setEnabled(nextEnabled); if true edit.setMasks([clip.id], [completeMask(clip.masks?.[0]), ...(clip.masks?.slice(1) ?? [])]); if false edit.setMasks([clip.id], [])","web/src/store/editActions.ts::setMasks(enabled array or [])","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=enable mask: assert exactly setEnabled(nextEnabled); if true edit.setMasks([clip.id], [completeMask(clip.masks?.[0]), ...(clip.masks?.slice(1) ?? [])]); if false edit.setMasks([clip.id], []) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"enable mask: assert exactly setEnabled(nextEnabled); if true edit.setMasks([clip.id], [completeMask(clip.masks?.[0]), ...(clip.masks?.slice(1) ?? [])]); if false edit.setMasks([clip.id], []) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-eac5d871d27e236a

- Candidate/source: `control-3f99a53815fd9130` at `web/src/components/inspector/Inspector.tsx:1192:13` (control)
- Expected behavior: mask shape type: assert exactly if selected kind is 'circle' commitMask({ ...draft, shape: defaultCircleShape() }); else if 'linear' commitMask({ ...draft, shape: defaultLinearShape() }); disabled 'poly' emits no call; commitMask calls edit.setMasks([clip.id], [next, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-3f99a53815fd9130.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-3f99a53815fd9130 mask shape type.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["selected option value","current candidate-specific model state"]; handler={(e) => {
                const kind = e.target.value;
                if (kind === "circle") setShape(defaultCircleShape());
                else if (kind === "linear") setShape(defaultLinearShape());
              }}.
  - Exact call/state/backend: stateTransition=mask shape type: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1192::handler {(e) => {\n                const kind = e.target.value;\n                if (kind === \"circle\") setShape(defaultCircleShape());\n                else if (kind === \"linear\") setShape(defaultLinearShape());\n              }} -> if selected kind is 'circle' commitMask({ ...draft, shape: defaultCircleShape() }); else if 'linear' commitMask({ ...draft, shape: defaultLinearShape() }); disabled 'poly' emits no call; commitMask calls edit.setMasks([clip.id], [next, ...remainingMasks])","web/src/store/editActions.ts::setMasks(circle or linear shape)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=mask shape type: assert exactly if selected kind is 'circle' commitMask({ ...draft, shape: defaultCircleShape() }); else if 'linear' commitMask({ ...draft, shape: defaultLinearShape() }); disabled 'poly' emits no call; commitMask calls edit.setMasks([clip.id], [next, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"mask shape type: assert exactly if selected kind is 'circle' commitMask({ ...draft, shape: defaultCircleShape() }); else if 'linear' commitMask({ ...draft, shape: defaultLinearShape() }); disabled 'poly' emits no call; commitMask calls edit.setMasks([clip.id], [next, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-69d6b02c48cfb78d

- Candidate/source: `control-0c7a02c6dae89f87` at `web/src/components/inspector/Inspector.tsx:1223:11` (control)
- Expected behavior: mask feather: assert exactly onChange updateCommon('feather', v) updates draft; onCommit commitMask({ ...draft, feather: v }) then edit.setMasks([clip.id], [next, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-0c7a02c6dae89f87.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-0c7a02c6dae89f87 mask feather.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updateCommon("feather", v)} {(v) => commitCommon("feather", v)}.
  - Exact call/state/backend: stateTransition=mask feather: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1223::handler {(v) => updateCommon(\"feather\", v)} {(v) => commitCommon(\"feather\", v)} -> onChange updateCommon('feather', v) updates draft; onCommit commitMask({ ...draft, feather: v }) then edit.setMasks([clip.id], [next, ...remainingMasks])","web/src/store/editActions.ts::setMasks({feather})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=mask feather: assert exactly onChange updateCommon('feather', v) updates draft; onCommit commitMask({ ...draft, feather: v }) then edit.setMasks([clip.id], [next, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"mask feather: assert exactly onChange updateCommon('feather', v) updates draft; onCommit commitMask({ ...draft, feather: v }) then edit.setMasks([clip.id], [next, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-d3d7ec4b0e4874af

- Candidate/source: `control-b77d7a7eb91fedfa` at `web/src/components/inspector/Inspector.tsx:1234:13` (control)
- Expected behavior: invert mask: assert exactly commitMask({ ...draft, invert: e.target.checked }) then edit.setMasks([clip.id], [next, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-b77d7a7eb91fedfa.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-b77d7a7eb91fedfa invert mask.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["changed value","blur/keyboard event when present","current candidate-specific model state"]; handler={(e) => commitCommon("invert", e.target.checked)}.
  - Exact call/state/backend: stateTransition=invert mask: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1234::handler {(e) => commitCommon(\"invert\", e.target.checked)} -> commitMask({ ...draft, invert: e.target.checked }) then edit.setMasks([clip.id], [next, ...remainingMasks])","web/src/store/editActions.ts::setMasks({invert})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=invert mask: assert exactly commitMask({ ...draft, invert: e.target.checked }) then edit.setMasks([clip.id], [next, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"invert mask: assert exactly commitMask({ ...draft, invert: e.target.checked }) then edit.setMasks([clip.id], [next, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-6738ba03c29040df

- Candidate/source: `control-fd4203443df4adcb` at `web/src/components/inspector/Inspector.tsx:1265:7` (control)
- Expected behavior: circle-mask center X: assert exactly onChange updatePoint('center','x',v) updates draft shape; onCommit commitPoint('center','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, center: { ...shape.center, x: v } }, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-fd4203443df4adcb.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-fd4203443df4adcb circle-mask center X.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updatePoint("center", "x", v)} {(v) => commitPoint("center", "x", v)}.
  - Exact call/state/backend: stateTransition=circle-mask center X: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1265::handler {(v) => updatePoint(\"center\", \"x\", v)} {(v) => commitPoint(\"center\", \"x\", v)} -> onChange updatePoint('center','x',v) updates draft shape; onCommit commitPoint('center','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, center: { ...shape.center, x: v } }, ...remainingMasks])","web/src/store/editActions.ts::setMasks(circle.center.x)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=circle-mask center X: assert exactly onChange updatePoint('center','x',v) updates draft shape; onCommit commitPoint('center','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, center: { ...shape.center, x: v } }, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"circle-mask center X: assert exactly onChange updatePoint('center','x',v) updates draft shape; onCommit commitPoint('center','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, center: { ...shape.center, x: v } }, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-76852c7be063d308

- Candidate/source: `control-18136ab8e305f7a7` at `web/src/components/inspector/Inspector.tsx:1271:7` (control)
- Expected behavior: circle-mask center Y: assert exactly onChange updatePoint('center','y',v) updates draft shape; onCommit commitPoint('center','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, center: { ...shape.center, y: v } }, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-18136ab8e305f7a7.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-18136ab8e305f7a7 circle-mask center Y.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updatePoint("center", "y", v)} {(v) => commitPoint("center", "y", v)}.
  - Exact call/state/backend: stateTransition=circle-mask center Y: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1271::handler {(v) => updatePoint(\"center\", \"y\", v)} {(v) => commitPoint(\"center\", \"y\", v)} -> onChange updatePoint('center','y',v) updates draft shape; onCommit commitPoint('center','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, center: { ...shape.center, y: v } }, ...remainingMasks])","web/src/store/editActions.ts::setMasks(circle.center.y)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=circle-mask center Y: assert exactly onChange updatePoint('center','y',v) updates draft shape; onCommit commitPoint('center','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, center: { ...shape.center, y: v } }, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"circle-mask center Y: assert exactly onChange updatePoint('center','y',v) updates draft shape; onCommit commitPoint('center','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, center: { ...shape.center, y: v } }, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-6235c98a7ee867a1

- Candidate/source: `control-09ca7bab665f84ed` at `web/src/components/inspector/Inspector.tsx:1277:7` (control)
- Expected behavior: circle-mask radius X: assert exactly onChange updatePoint('radius','x',v) updates draft shape; onCommit commitPoint('radius','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, radius: { ...shape.radius, x: v } }, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-09ca7bab665f84ed.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-09ca7bab665f84ed circle-mask radius X.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updatePoint("radius", "x", v)} {(v) => commitPoint("radius", "x", v)}.
  - Exact call/state/backend: stateTransition=circle-mask radius X: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1277::handler {(v) => updatePoint(\"radius\", \"x\", v)} {(v) => commitPoint(\"radius\", \"x\", v)} -> onChange updatePoint('radius','x',v) updates draft shape; onCommit commitPoint('radius','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, radius: { ...shape.radius, x: v } }, ...remainingMasks])","web/src/store/editActions.ts::setMasks(circle.radius.x)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=circle-mask radius X: assert exactly onChange updatePoint('radius','x',v) updates draft shape; onCommit commitPoint('radius','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, radius: { ...shape.radius, x: v } }, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"circle-mask radius X: assert exactly onChange updatePoint('radius','x',v) updates draft shape; onCommit commitPoint('radius','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, radius: { ...shape.radius, x: v } }, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-dcde418d02888350

- Candidate/source: `control-9e9cf7fd25863d20` at `web/src/components/inspector/Inspector.tsx:1285:7` (control)
- Expected behavior: circle-mask radius Y: assert exactly onChange updatePoint('radius','y',v) updates draft shape; onCommit commitPoint('radius','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, radius: { ...shape.radius, y: v } }, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-9e9cf7fd25863d20.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-9e9cf7fd25863d20 circle-mask radius Y.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updatePoint("radius", "y", v)} {(v) => commitPoint("radius", "y", v)}.
  - Exact call/state/backend: stateTransition=circle-mask radius Y: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1285::handler {(v) => updatePoint(\"radius\", \"y\", v)} {(v) => commitPoint(\"radius\", \"y\", v)} -> onChange updatePoint('radius','y',v) updates draft shape; onCommit commitPoint('radius','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, radius: { ...shape.radius, y: v } }, ...remainingMasks])","web/src/store/editActions.ts::setMasks(circle.radius.y)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=circle-mask radius Y: assert exactly onChange updatePoint('radius','y',v) updates draft shape; onCommit commitPoint('radius','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, radius: { ...shape.radius, y: v } }, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"circle-mask radius Y: assert exactly onChange updatePoint('radius','y',v) updates draft shape; onCommit commitPoint('radius','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, radius: { ...shape.radius, y: v } }, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-8eaff7c86e495e34

- Candidate/source: `control-47bf4ee974f14505` at `web/src/components/inspector/Inspector.tsx:1315:7` (control)
- Expected behavior: linear-mask point X: assert exactly onChange updatePoint('point','x',v) updates draft shape; onCommit commitPoint('point','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, point: { ...shape.point, x: v } }, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-47bf4ee974f14505.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-47bf4ee974f14505 linear-mask point X.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updatePoint("point", "x", v)} {(v) => commitPoint("point", "x", v)}.
  - Exact call/state/backend: stateTransition=linear-mask point X: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1315::handler {(v) => updatePoint(\"point\", \"x\", v)} {(v) => commitPoint(\"point\", \"x\", v)} -> onChange updatePoint('point','x',v) updates draft shape; onCommit commitPoint('point','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, point: { ...shape.point, x: v } }, ...remainingMasks])","web/src/store/editActions.ts::setMasks(linear.point.x)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=linear-mask point X: assert exactly onChange updatePoint('point','x',v) updates draft shape; onCommit commitPoint('point','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, point: { ...shape.point, x: v } }, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"linear-mask point X: assert exactly onChange updatePoint('point','x',v) updates draft shape; onCommit commitPoint('point','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, point: { ...shape.point, x: v } }, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-6affc7b83553bf04

- Candidate/source: `control-4b61e7b97ade8478` at `web/src/components/inspector/Inspector.tsx:1321:7` (control)
- Expected behavior: linear-mask point Y: assert exactly onChange updatePoint('point','y',v) updates draft shape; onCommit commitPoint('point','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, point: { ...shape.point, y: v } }, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-4b61e7b97ade8478.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-4b61e7b97ade8478 linear-mask point Y.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updatePoint("point", "y", v)} {(v) => commitPoint("point", "y", v)}.
  - Exact call/state/backend: stateTransition=linear-mask point Y: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1321::handler {(v) => updatePoint(\"point\", \"y\", v)} {(v) => commitPoint(\"point\", \"y\", v)} -> onChange updatePoint('point','y',v) updates draft shape; onCommit commitPoint('point','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, point: { ...shape.point, y: v } }, ...remainingMasks])","web/src/store/editActions.ts::setMasks(linear.point.y)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=linear-mask point Y: assert exactly onChange updatePoint('point','y',v) updates draft shape; onCommit commitPoint('point','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, point: { ...shape.point, y: v } }, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"linear-mask point Y: assert exactly onChange updatePoint('point','y',v) updates draft shape; onCommit commitPoint('point','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, point: { ...shape.point, y: v } }, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-6ddc3ebc24250e06

- Candidate/source: `control-bc82a346093bae7c` at `web/src/components/inspector/Inspector.tsx:1327:7` (control)
- Expected behavior: linear-mask normal X: assert exactly onChange updatePoint('normal','x',v) updates draft shape; onCommit commitPoint('normal','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, normal: { ...shape.normal, x: v } }, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-bc82a346093bae7c.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-bc82a346093bae7c linear-mask normal X.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updatePoint("normal", "x", v)} {(v) => commitPoint("normal", "x", v)}.
  - Exact call/state/backend: stateTransition=linear-mask normal X: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1327::handler {(v) => updatePoint(\"normal\", \"x\", v)} {(v) => commitPoint(\"normal\", \"x\", v)} -> onChange updatePoint('normal','x',v) updates draft shape; onCommit commitPoint('normal','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, normal: { ...shape.normal, x: v } }, ...remainingMasks])","web/src/store/editActions.ts::setMasks(linear.normal.x)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=linear-mask normal X: assert exactly onChange updatePoint('normal','x',v) updates draft shape; onCommit commitPoint('normal','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, normal: { ...shape.normal, x: v } }, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"linear-mask normal X: assert exactly onChange updatePoint('normal','x',v) updates draft shape; onCommit commitPoint('normal','x',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, normal: { ...shape.normal, x: v } }, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-54962d9a3382feda

- Candidate/source: `control-d60d607d7448918f` at `web/src/components/inspector/Inspector.tsx:1335:7` (control)
- Expected behavior: linear-mask normal Y: assert exactly onChange updatePoint('normal','y',v) updates draft shape; onCommit commitPoint('normal','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, normal: { ...shape.normal, y: v } }, ...remainingMasks]) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d60d607d7448918f.
  - Test: web/src/components/inspector/Inspector.interaction.test.tsx#control-d60d607d7448918f linear-mask normal Y.
  - Initial state: visibility=Visible in Inspector for one selected clip, gated by media type, active tab, section state, and any source-level condition.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => updatePoint("normal", "y", v)} {(v) => commitPoint("normal", "y", v)}.
  - Exact call/state/backend: stateTransition=linear-mask normal Y: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/Inspector.tsx:1335::handler {(v) => updatePoint(\"normal\", \"y\", v)} {(v) => commitPoint(\"normal\", \"y\", v)} -> onChange updatePoint('normal','y',v) updates draft shape; onCommit commitPoint('normal','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, normal: { ...shape.normal, y: v } }, ...remainingMasks])","web/src/store/editActions.ts::setMasks(linear.normal.y)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetMasks","code:web/src/components/inspector/Inspector.tsx#Inspector","code:web/src/store/editActions.ts#setMasks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=linear-mask normal Y: assert exactly onChange updatePoint('normal','y',v) updates draft shape; onCommit commitPoint('normal','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, normal: { ...shape.normal, y: v } }, ...remainingMasks]) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"linear-mask normal Y: assert exactly onChange updatePoint('normal','y',v) updates draft shape; onCommit commitPoint('normal','y',v) -> setShape -> edit.setMasks([clip.id], [{ ...shape, normal: { ...shape.normal, y: v } }, ...remainingMasks]) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-8b0f987c8bf46a8a enable mask` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-3f99a53815fd9130 mask shape type` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-0c7a02c6dae89f87 mask feather` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-b77d7a7eb91fedfa invert mask` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-fd4203443df4adcb circle-mask center X` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-18136ab8e305f7a7 circle-mask center Y` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-09ca7bab665f84ed circle-mask radius X` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-9e9cf7fd25863d20 circle-mask radius Y` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-47bf4ee974f14505 linear-mask point X` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-4b61e7b97ade8478 linear-mask point Y` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-bc82a346093bae7c linear-mask normal X` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/Inspector.interaction.test.tsx#control-d60d607d7448918f linear-mask normal Y` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-8b0f987c8bf46a8a enable mask"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-3f99a53815fd9130 mask shape type"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-0c7a02c6dae89f87 mask feather"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-b77d7a7eb91fedfa invert mask"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-fd4203443df4adcb circle-mask center X"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-18136ab8e305f7a7 circle-mask center Y"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-09ca7bab665f84ed circle-mask radius X"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-9e9cf7fd25863d20 circle-mask radius Y"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-47bf4ee974f14505 linear-mask point X"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-4b61e7b97ade8478 linear-mask point Y"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-bc82a346093bae7c linear-mask normal X"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-d60d607d7448918f linear-mask normal Y"`

  Expected: FAIL because one or more of the 12 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/Inspector.tsx`, `web/src/store/editActions.ts#setMasks`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetMasks`, `web/src/components/inspector/Inspector.tsx#Inspector`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-8b0f987c8bf46a8a enable mask"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-3f99a53815fd9130 mask shape type"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-0c7a02c6dae89f87 mask feather"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-b77d7a7eb91fedfa invert mask"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-fd4203443df4adcb circle-mask center X"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-18136ab8e305f7a7 circle-mask center Y"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-09ca7bab665f84ed circle-mask radius X"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-9e9cf7fd25863d20 circle-mask radius Y"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-47bf4ee974f14505 linear-mask point X"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-4b61e7b97ade8478 linear-mask point Y"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-bc82a346093bae7c linear-mask normal X"`
  - Run: `pnpm -C web test -- --run src/components/inspector/Inspector.interaction.test.tsx -t "control-d60d607d7448918f linear-mask normal Y"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 23: control-acceptance (implementation-slice-237ce65b3abb7cdc)

**Covered records:**
- `control-record-608aded71c58382e` (control)

**Files:**
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx`
- Modify: `web/src/store/editActions.ts#stampKeyframe`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::StampKeyframe`
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-9e0e61df5570f064 stamp keyframe`

**Candidate-bound contracts:**

#### control-record-608aded71c58382e

- Candidate/source: `control-9e0e61df5570f064` at `web/src/components/inspector/KeyframesLaneRow.tsx:259:11` (control)
- Expected behavior: stamp keyframe: assert exactly edit.stampKeyframe(clip.id, property, activeFrame) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-9e0e61df5570f064.
  - Test: web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-9e0e61df5570f064 stamp keyframe.
  - Initial state: visibility=Visible when one clip is selected and the Inspector keyframes panel/owning row or menu is mounted.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={handleStamp}.
  - Exact call/state/backend: stateTransition=stamp keyframe: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/KeyframesLaneRow.tsx:259::handler {handleStamp} -> edit.stampKeyframe(clip.id, property, activeFrame)","web/src/store/editActions.ts::stampKeyframe(clip.id, property, activeFrame)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::StampKeyframe","code:web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow","code:web/src/store/editActions.ts#stampKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=stamp keyframe: assert exactly edit.stampKeyframe(clip.id, property, activeFrame) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"inspector.keyframes.stamp\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"stamp keyframe: assert exactly edit.stampKeyframe(clip.id, property, activeFrame) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-9e0e61df5570f064 stamp keyframe` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-9e0e61df5570f064 stamp keyframe"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/KeyframesLaneRow.tsx`, `web/src/store/editActions.ts#stampKeyframe`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::StampKeyframe`, `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-9e0e61df5570f064 stamp keyframe"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 24: control-acceptance (implementation-slice-bd88d84f88039fb9)

**Covered records:**
- `control-record-0bc72a84c5cfd34f` (control)

**Files:**
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx`
- Modify: `web/src/store/editActions.ts#setKeyframes`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetKeyframes`
- Modify: `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-d8514c2403a21b7d clear property keyframe track`

**Candidate-bound contracts:**

#### control-record-0bc72a84c5cfd34f

- Candidate/source: `control-d8514c2403a21b7d` at `web/src/components/inspector/KeyframesLaneRow.tsx:267:13` (control)
- Expected behavior: clear property keyframe track: assert exactly if property is 'position' or 'scale' edit.setKeyframes(clip.id, property, { kind: 'pair', keyframes: [] }); else if property is 'crop' use { kind: 'crop', keyframes: [] }; otherwise use { kind: 'scalar', keyframes: [] } and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d8514c2403a21b7d.
  - Test: web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-d8514c2403a21b7d clear property keyframe track.
  - Initial state: visibility=Visible when one clip is selected and the Inspector keyframes panel/owning row or menu is mounted.; enabledWhen=Rendered only when this property track has at least one keyframe..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={handleClear}.
  - Exact call/state/backend: stateTransition=clear property keyframe track: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/KeyframesLaneRow.tsx:267::handler {handleClear} -> if property is 'position' or 'scale' edit.setKeyframes(clip.id, property, { kind: 'pair', keyframes: [] }); else if property is 'crop' use { kind: 'crop', keyframes: [] }; otherwise use { kind: 'scalar', keyframes: [] }","web/src/store/editActions.ts::setKeyframes(clip.id, property, exact typed empty track)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetKeyframes","code:web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow","code:web/src/store/editActions.ts#setKeyframes","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=clear property keyframe track: assert exactly if property is 'position' or 'scale' edit.setKeyframes(clip.id, property, { kind: 'pair', keyframes: [] }); else if property is 'crop' use { kind: 'crop', keyframes: [] }; otherwise use { kind: 'scalar', keyframes: [] } and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"inspector.keyframes.clear\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"clear property keyframe track: assert exactly if property is 'position' or 'scale' edit.setKeyframes(clip.id, property, { kind: 'pair', keyframes: [] }); else if property is 'crop' use { kind: 'crop', keyframes: [] }; otherwise use { kind: 'scalar', keyframes: [] } and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/KeyframesLaneRow.interaction.test.tsx#control-d8514c2403a21b7d clear property keyframe track` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-d8514c2403a21b7d clear property keyframe track"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/KeyframesLaneRow.tsx`, `web/src/store/editActions.ts#setKeyframes`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetKeyframes`, `web/src/components/inspector/KeyframesLaneRow.tsx#KeyframesLaneRow`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/KeyframesLaneRow.interaction.test.tsx -t "control-d8514c2403a21b7d clear property keyframe track"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 25: control-acceptance (implementation-slice-e08acd62e8da15ff)

**Covered records:**
- `control-record-5c245c32034f7a2c` (control)

**Files:**
- Modify: `web/src/components/inspector/SwapMediaSection.tsx`
- Modify: `web/src/components/inspector/SwapMediaSection.tsx#SwapMediaSection`
- Test (reviewed-planned): `web/src/components/inspector/SwapMediaSection.interaction.test.tsx#control-9e0ba2713550e944 toggle inline swap-media list`

**Candidate-bound contracts:**

#### control-record-5c245c32034f7a2c

- Candidate/source: `control-9e0ba2713550e944` at `web/src/components/inspector/SwapMediaSection.tsx:75:7` (control)
- Expected behavior: toggle inline swap-media list: assert exactly setOpen((v) => !v) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-9e0ba2713550e944.
  - Test: web/src/components/inspector/SwapMediaSection.interaction.test.tsx#control-9e0ba2713550e944 toggle inline swap-media list.
  - Initial state: visibility=Visible for an eligible non-text clip; replacement choices require a same-type alternative asset.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => setOpen((v) => !v)}.
  - Exact call/state/backend: stateTransition=toggle inline swap-media list: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/SwapMediaSection.tsx:75::handler {() => setOpen((v) => !v)} -> setOpen((v) => !v)","web/src/components/inspector/SwapMediaSection.tsx::SwapMediaSection local open state","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/inspector/SwapMediaSection.tsx#SwapMediaSection"].
  - Visible/accessibility/return path: success=toggle inline swap-media list: assert exactly setOpen((v) => !v) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"toggle inline swap-media list: assert exactly setOpen((v) => !v) and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/SwapMediaSection.interaction.test.tsx#control-9e0ba2713550e944 toggle inline swap-media list` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/SwapMediaSection.interaction.test.tsx -t "control-9e0ba2713550e944 toggle inline swap-media list"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/SwapMediaSection.tsx`, `web/src/components/inspector/SwapMediaSection.tsx#SwapMediaSection` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/SwapMediaSection.interaction.test.tsx -t "control-9e0ba2713550e944 toggle inline swap-media list"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 26: control-acceptance (implementation-slice-31427cfd158b2471)

**Covered records:**
- `control-record-29d55028e760880e` (control)

**Files:**
- Modify: `web/src/components/inspector/SwapMediaSection.tsx`
- Modify: `web/src/store/editActions.ts#swapMedia`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SwapMedia`
- Modify: `web/src/components/inspector/SwapMediaSection.tsx#SwapMediaSection`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/SwapMediaSection.interaction.test.tsx#control-c68a368591406bef replace clip media from inline list`

**Candidate-bound contracts:**

#### control-record-29d55028e760880e

- Candidate/source: `control-c68a368591406bef` at `web/src/components/inspector/SwapMediaSection.tsx:129:15` (control)
- Expected behavior: replace clip media from inline list: assert exactly handlePick(item) calls edit.swapMedia(clip.id, item.id) and immediately setOpen(false) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c68a368591406bef.
  - Test: web/src/components/inspector/SwapMediaSection.interaction.test.tsx#control-c68a368591406bef replace clip media from inline list.
  - Initial state: visibility=Visible for an eligible non-text clip; replacement choices require a same-type alternative asset.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => handlePick(item)} {(e) => {
                  e.currentTarget.style.background = "var(--bg-hover)";
                }} {(e) => {
                  e.currentTarget.style.background = "none";
                }}.
  - Exact call/state/backend: stateTransition=replace clip media from inline list: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/SwapMediaSection.tsx:129::handler {() => handlePick(item)} {(e) => {\n                  e.currentTarget.style.background = \"var(--bg-hover)\";\n                }} {(e) => {\n                  e.currentTarget.style.background = \"none\";\n                }} -> handlePick(item) calls edit.swapMedia(clip.id, item.id) and immediately setOpen(false)","web/src/store/editActions.ts::swapMedia(clip.id,item.id)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SwapMedia","code:web/src/components/inspector/SwapMediaSection.tsx#SwapMediaSection","code:web/src/store/editActions.ts#swapMedia","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=replace clip media from inline list: assert exactly handlePick(item) calls edit.swapMedia(clip.id, item.id) and immediately setOpen(false) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"replace clip media from inline list: assert exactly handlePick(item) calls edit.swapMedia(clip.id, item.id) and immediately setOpen(false) and no sibling branch/command.","pending":"The promise can be pending after the list closes; no busy indicator is shown.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"The rejected swapMedia promise is not caught and the list has already closed."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/SwapMediaSection.interaction.test.tsx#control-c68a368591406bef replace clip media from inline list` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/SwapMediaSection.interaction.test.tsx -t "control-c68a368591406bef replace clip media from inline list"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/SwapMediaSection.tsx`, `web/src/store/editActions.ts#swapMedia`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SwapMedia`, `web/src/components/inspector/SwapMediaSection.tsx#SwapMediaSection`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/SwapMediaSection.interaction.test.tsx -t "control-c68a368591406bef replace clip media from inline list"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 27: control-acceptance (implementation-slice-29b71f1324a25efb)

**Covered records:**
- `control-record-84ec280f0998c447` (control)

**Files:**
- Modify: `web/src/components/inspector/TextTab.tsx`
- Modify: `web/src/components/inspector/TextTab.tsx#commitText`
- Modify: `web/src/store/editActions.ts#setClipProperties`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetClipProperties`
- Modify: `web/src/components/inspector/TextTab.tsx#TextTab`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/TextTab.interaction.test.tsx#control-e0302cdc885e0acc text clip content`

**Candidate-bound contracts:**

#### control-record-84ec280f0998c447

- Candidate/source: `control-e0302cdc885e0acc` at `web/src/components/inspector/TextTab.tsx:92:9` (control)
- Expected behavior: text clip content: assert exactly onChange setValue(e.target.value); onBlur commitText compares against clip.textContent and, only when changed, calls edit.setClipProperties([clip.id], { textContent: value }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-e0302cdc885e0acc.
  - Test: web/src/components/inspector/TextTab.interaction.test.tsx#control-e0302cdc885e0acc text clip content.
  - Initial state: visibility=Visible for a selected text clip on the Text tab; conditional color controls require the corresponding style toggle.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["changed value","blur/keyboard event when present","current candidate-specific model state"]; handler={(e) => setValue(e.target.value)} {commitText}.
  - Exact call/state/backend: stateTransition=text clip content: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/TextTab.tsx:92::handler {(e) => setValue(e.target.value)} {commitText} -> onChange setValue(e.target.value); onBlur commitText compares against clip.textContent and, only when changed, calls edit.setClipProperties([clip.id], { textContent: value })","web/src/components/inspector/TextTab.tsx::commitText","web/src/store/editActions.ts::setClipProperties({textContent}) only when changed","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/TextTab.tsx#TextTab","code:web/src/components/inspector/TextTab.tsx#commitText","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=text clip content: assert exactly onChange setValue(e.target.value); onBlur commitText compares against clip.textContent and, only when changed, calls edit.setClipProperties([clip.id], { textContent: value }) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"text clip content: assert exactly onChange setValue(e.target.value); onBlur commitText compares against clip.textContent and, only when changed, calls edit.setClipProperties([clip.id], { textContent: value }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/TextTab.interaction.test.tsx#control-e0302cdc885e0acc text clip content` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-e0302cdc885e0acc text clip content"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/TextTab.tsx`, `web/src/components/inspector/TextTab.tsx#commitText`, `web/src/store/editActions.ts#setClipProperties`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetClipProperties`, `web/src/components/inspector/TextTab.tsx#TextTab`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-e0302cdc885e0acc text clip content"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 28: control-acceptance (implementation-slice-994048d7e601725b)

**Covered records:**
- `control-record-4f1150d3091b54cb` (control)
- `control-record-fd22e8d3b4ec9151` (control)
- `control-record-d1711a444e8319f7` (control)
- `control-record-f9736741e4c8b8b4` (control)
- `control-record-d48f508645edcefe` (control)
- `control-record-68d58bc12d0a2261` (control)
- `control-record-6005682e6413a54c` (control)

**Files:**
- Modify: `web/src/components/inspector/TextTab.tsx`
- Modify: `web/src/store/editActions.ts#setClipProperties`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetClipProperties`
- Modify: `web/src/components/inspector/TextTab.tsx#TextTab`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/inspector/TextTab.interaction.test.tsx#control-8687a779cfae399f text font family`
- Test (reviewed-planned): `web/src/components/inspector/TextTab.interaction.test.tsx#control-976b535bb1c6f753 text font size`
- Test (reviewed-planned): `web/src/components/inspector/TextTab.interaction.test.tsx#control-e21293de512b1bfe text color`
- Test (reviewed-planned): `web/src/components/inspector/TextTab.interaction.test.tsx#control-da906f8def0b4db0 text alignment`
- Test (reviewed-planned): `web/src/components/inspector/TextTab.interaction.test.tsx#control-a815d566ee9e2534 text background enable/color`
- Test (reviewed-planned): `web/src/components/inspector/TextTab.interaction.test.tsx#control-2c6f533fe3117eef text border enable/color`
- Test (reviewed-planned): `web/src/components/inspector/TextTab.interaction.test.tsx#control-b5e10feb28294b06 text shadow enable/color`

**Candidate-bound contracts:**

#### control-record-4f1150d3091b54cb

- Candidate/source: `control-8687a779cfae399f` at `web/src/components/inspector/TextTab.tsx:118:11` (control)
- Expected behavior: text font family: assert exactly commitStyle({ ...style, fontName: e.target.value }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-8687a779cfae399f.
  - Test: web/src/components/inspector/TextTab.interaction.test.tsx#control-8687a779cfae399f text font family.
  - Initial state: visibility=Visible for a selected text clip on the Text tab; conditional color controls require the corresponding style toggle.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["selected option value","current candidate-specific model state"]; handler={(e) => commitStyle({ ...style, fontName: e.target.value })}.
  - Exact call/state/backend: stateTransition=text font family: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/TextTab.tsx:118::handler {(e) => commitStyle({ ...style, fontName: e.target.value })} -> commitStyle({ ...style, fontName: e.target.value }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next })","web/src/store/editActions.ts::setClipProperties({textStyle.fontName})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/TextTab.tsx#TextTab","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=text font family: assert exactly commitStyle({ ...style, fontName: e.target.value }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"inspector.field.fontFamily\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"text font family: assert exactly commitStyle({ ...style, fontName: e.target.value }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-fd22e8d3b4ec9151

- Candidate/source: `control-976b535bb1c6f753` at `web/src/components/inspector/TextTab.tsx:144:11` (control)
- Expected behavior: text font size: assert exactly commitStyle({ ...style, fontSize: v }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-976b535bb1c6f753.
  - Test: web/src/components/inspector/TextTab.interaction.test.tsx#control-976b535bb1c6f753 text font size.
  - Initial state: visibility=Visible for a selected text clip on the Text tab; conditional color controls require the corresponding style toggle.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(v) => commitStyle({ ...style, fontSize: v })}.
  - Exact call/state/backend: stateTransition=text font size: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/TextTab.tsx:144::handler {(v) => commitStyle({ ...style, fontSize: v })} -> commitStyle({ ...style, fontSize: v }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next })","web/src/store/editActions.ts::setClipProperties({textStyle.fontSize})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/TextTab.tsx#TextTab","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=text font size: assert exactly commitStyle({ ...style, fontSize: v }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"text font size: assert exactly commitStyle({ ...style, fontSize: v }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-d1711a444e8319f7

- Candidate/source: `control-e21293de512b1bfe` at `web/src/components/inspector/TextTab.tsx:156:11` (control)
- Expected behavior: text color: assert exactly commitStyle({ ...style, color }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-e21293de512b1bfe.
  - Test: web/src/components/inspector/TextTab.interaction.test.tsx#control-e21293de512b1bfe text color.
  - Initial state: visibility=Visible for a selected text clip on the Text tab; conditional color controls require the corresponding style toggle.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(color) => commitStyle({ ...style, color })}.
  - Exact call/state/backend: stateTransition=text color: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/TextTab.tsx:156::handler {(color) => commitStyle({ ...style, color })} -> commitStyle({ ...style, color }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next })","web/src/store/editActions.ts::setClipProperties({textStyle.color})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/TextTab.tsx#TextTab","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=text color: assert exactly commitStyle({ ...style, color }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"text color: assert exactly commitStyle({ ...style, color }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-f9736741e4c8b8b4

- Candidate/source: `control-da906f8def0b4db0` at `web/src/components/inspector/TextTab.tsx:166:15` (control)
- Expected behavior: text alignment: assert exactly for clicked rendered alignment a, commitStyle({ ...style, alignment: a }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-da906f8def0b4db0.
  - Test: web/src/components/inspector/TextTab.interaction.test.tsx#control-da906f8def0b4db0 text alignment.
  - Initial state: visibility=Visible for a selected text clip on the Text tab; conditional color controls require the corresponding style toggle.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => commitStyle({ ...style, alignment: a })}.
  - Exact call/state/backend: stateTransition=text alignment: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/TextTab.tsx:166::handler {() => commitStyle({ ...style, alignment: a })} -> for clicked rendered alignment a, commitStyle({ ...style, alignment: a }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next })","web/src/store/editActions.ts::setClipProperties({textStyle.alignment})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/TextTab.tsx#TextTab","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=text alignment: assert exactly for clicked rendered alignment a, commitStyle({ ...style, alignment: a }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"Accessible-name source recorded by discovery: t(`inspector.align.${a}`).","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"text alignment: assert exactly for clicked rendered alignment a, commitStyle({ ...style, alignment: a }) -> setStyle(next) and edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-d48f508645edcefe

- Candidate/source: `control-a815d566ee9e2534` at `web/src/components/inspector/TextTab.tsx:177:9` (control)
- Expected behavior: text background enable/color: assert exactly checkbox branch commitStyle({ ...style, background: { ...style.background, enabled } }); color branch commitStyle({ ...style, background: { ...style.background, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-a815d566ee9e2534.
  - Test: web/src/components/inspector/TextTab.interaction.test.tsx#control-a815d566ee9e2534 text background enable/color.
  - Initial state: visibility=Visible for a selected text clip on the Text tab; conditional color controls require the corresponding style toggle.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(enabled) =>
            commitStyle({ ...style, background: { ...style.background, enabled } })
          } {(color) =>
            commitStyle({ ...style, background: { ...style.background, color } })
          }.
  - Exact call/state/backend: stateTransition=text background enable/color: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/TextTab.tsx:177::handler {(enabled) =>\n            commitStyle({ ...style, background: { ...style.background, enabled } })\n          } {(color) =>\n            commitStyle({ ...style, background: { ...style.background, color } })\n          } -> checkbox branch commitStyle({ ...style, background: { ...style.background, enabled } }); color branch commitStyle({ ...style, background: { ...style.background, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next })","web/src/store/editActions.ts::setClipProperties({textStyle.background.enabled or color})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/TextTab.tsx#TextTab","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=text background enable/color: assert exactly checkbox branch commitStyle({ ...style, background: { ...style.background, enabled } }); color branch commitStyle({ ...style, background: { ...style.background, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"text background enable/color: assert exactly checkbox branch commitStyle({ ...style, background: { ...style.background, enabled } }); color branch commitStyle({ ...style, background: { ...style.background, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-68d58bc12d0a2261

- Candidate/source: `control-2c6f533fe3117eef` at `web/src/components/inspector/TextTab.tsx:189:9` (control)
- Expected behavior: text border enable/color: assert exactly checkbox branch commitStyle({ ...style, border: { ...style.border, enabled } }); color branch commitStyle({ ...style, border: { ...style.border, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-2c6f533fe3117eef.
  - Test: web/src/components/inspector/TextTab.interaction.test.tsx#control-2c6f533fe3117eef text border enable/color.
  - Initial state: visibility=Visible for a selected text clip on the Text tab; conditional color controls require the corresponding style toggle.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(enabled) =>
            commitStyle({ ...style, border: { ...style.border, enabled } })
          } {(color) => commitStyle({ ...style, border: { ...style.border, color } })}.
  - Exact call/state/backend: stateTransition=text border enable/color: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/TextTab.tsx:189::handler {(enabled) =>\n            commitStyle({ ...style, border: { ...style.border, enabled } })\n          } {(color) => commitStyle({ ...style, border: { ...style.border, color } })} -> checkbox branch commitStyle({ ...style, border: { ...style.border, enabled } }); color branch commitStyle({ ...style, border: { ...style.border, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next })","web/src/store/editActions.ts::setClipProperties({textStyle.border.enabled or color})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/TextTab.tsx#TextTab","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=text border enable/color: assert exactly checkbox branch commitStyle({ ...style, border: { ...style.border, enabled } }); color branch commitStyle({ ...style, border: { ...style.border, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"text border enable/color: assert exactly checkbox branch commitStyle({ ...style, border: { ...style.border, enabled } }); color branch commitStyle({ ...style, border: { ...style.border, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-6005682e6413a54c

- Candidate/source: `control-b5e10feb28294b06` at `web/src/components/inspector/TextTab.tsx:199:9` (control)
- Expected behavior: text shadow enable/color: assert exactly checkbox branch commitStyle({ ...style, shadow: { ...style.shadow, enabled } }); color branch commitStyle({ ...style, shadow: { ...style.shadow, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-b5e10feb28294b06.
  - Test: web/src/components/inspector/TextTab.interaction.test.tsx#control-b5e10feb28294b06 text shadow enable/color.
  - Initial state: visibility=Visible for a selected text clip on the Text tab; conditional color controls require the corresponding style toggle.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(enabled) =>
            commitStyle({ ...style, shadow: { ...style.shadow, enabled } })
          } {(color) => commitStyle({ ...style, shadow: { ...style.shadow, color } })}.
  - Exact call/state/backend: stateTransition=text shadow enable/color: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/inspector/TextTab.tsx:199::handler {(enabled) =>\n            commitStyle({ ...style, shadow: { ...style.shadow, enabled } })\n          } {(color) => commitStyle({ ...style, shadow: { ...style.shadow, color } })} -> checkbox branch commitStyle({ ...style, shadow: { ...style.shadow, enabled } }); color branch commitStyle({ ...style, shadow: { ...style.shadow, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next })","web/src/store/editActions.ts::setClipProperties({textStyle.shadow.enabled or color})","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/inspector/TextTab.tsx#TextTab","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=text shadow enable/color: assert exactly checkbox branch commitStyle({ ...style, shadow: { ...style.shadow, enabled } }); color branch commitStyle({ ...style, shadow: { ...style.shadow, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"text shadow enable/color: assert exactly checkbox branch commitStyle({ ...style, shadow: { ...style.shadow, enabled } }); color branch commitStyle({ ...style, shadow: { ...style.shadow, color } }); each calls edit.setClipProperties([clip.id], { textStyle: next }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/inspector/TextTab.interaction.test.tsx#control-8687a779cfae399f text font family` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/TextTab.interaction.test.tsx#control-976b535bb1c6f753 text font size` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/TextTab.interaction.test.tsx#control-e21293de512b1bfe text color` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/TextTab.interaction.test.tsx#control-da906f8def0b4db0 text alignment` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/TextTab.interaction.test.tsx#control-a815d566ee9e2534 text background enable/color` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/TextTab.interaction.test.tsx#control-2c6f533fe3117eef text border enable/color` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/inspector/TextTab.interaction.test.tsx#control-b5e10feb28294b06 text shadow enable/color` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-8687a779cfae399f text font family"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-976b535bb1c6f753 text font size"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-e21293de512b1bfe text color"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-da906f8def0b4db0 text alignment"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-a815d566ee9e2534 text background enable/color"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-2c6f533fe3117eef text border enable/color"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-b5e10feb28294b06 text shadow enable/color"`

  Expected: FAIL because one or more of the 7 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/inspector/TextTab.tsx`, `web/src/store/editActions.ts#setClipProperties`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetClipProperties`, `web/src/components/inspector/TextTab.tsx#TextTab`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-8687a779cfae399f text font family"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-976b535bb1c6f753 text font size"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-e21293de512b1bfe text color"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-da906f8def0b4db0 text alignment"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-a815d566ee9e2534 text background enable/color"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-2c6f533fe3117eef text border enable/color"`
  - Run: `pnpm -C web test -- --run src/components/inspector/TextTab.interaction.test.tsx -t "control-b5e10feb28294b06 text shadow enable/color"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.
