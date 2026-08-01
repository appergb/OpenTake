# Command Contracts Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 45 verified incomplete records in the `command-contracts` gap group.

**Architecture:** Implement 16 primary evidence-bound slices and reference 3 shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.

---

### Task 1: CC-seconds-truncation (implementation-slice-c067e28b3aa59ea5)

**Covered records:**
- `requirement-0a3a8376bab5c2b6` (requirement)

**Files:**
- Modify: `web/src/store/editActions.ts#mediaDurationFrames`
- Modify: `web/src/store/editActions.ts#momentDurationFrames`
- Modify: `web/src/lib/timelineInsert.ts#buildInsertPlan`
- Modify: `docs/architecture/BUGS.md`
- Test (reviewed-planned): `web/src/store/editActions.test.ts#seconds_to_frame_truncates_fractional_boundaries`

**Candidate-bound contracts:**

#### requirement-0a3a8376bab5c2b6

- Candidate/source: `doc-6759415bbf395caf` at `docs/architecture/BUGS.md:20` (requirement)
- Expected behavior: All seconds-to-frame conversions obey the truncation contract at fractional boundaries.
- Resolution: `reviewed-mapping-report:CC-seconds-truncation` — Core mapping report CC-seconds-truncation: tracked second-to-frame helpers round fractional boundaries instead of truncating them.
- Exact acceptance contract:
  - Replace Math.round at mediaDurationFrames, momentDurationFrames, moment trim-start, and text-duration contract boundaries with the same truncation rule used by Rust.
  - Keep explicitly nearest-frame UI interactions documented and separate so the conversion policy cannot drift between call sites.
  - Add editActions cases for positive fractional values 0.49, 0.5, 0.99, and 1.01 frames at 24/30 fps plus negative/invalid inputs; assert exact parity with Rust conversion vectors.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/store/editActions.test.ts#seconds_to_frame_truncates_fractional_boundaries` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

  Result: The exact owner covers 24/30 fps at sub-frame, half-frame,
  near-next-frame, and multi-frame fractional boundaries. It also exercises the
  media-drop, ripple-insert, search-moment, and default-text call sites plus
  negative, NaN, and infinite inputs.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/store/editActions.test.ts -t "seconds_to_frame_truncates_fractional_boundaries"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

  Result: RED reproduced. A 10.5-frame media duration returned 11 rather than
  the Rust-compatible truncated value 10; Vitest reported 1 failed test and a
  nonzero pnpm exit.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/store/editActions.ts#mediaDurationFrames`, `web/src/store/editActions.ts#momentDurationFrames`, `web/src/lib/timelineInsert.ts#buildInsertPlan`, `docs/architecture/BUGS.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Result: Duration and offset conversions now truncate toward zero with finite
  fallbacks. Media placement, moment trim start/length, ripple insertion, and
  default text duration share the Rust conversion policy. Explicit
  nearest-frame playhead interactions retain `Math.round` and are documented as
  a separate UI policy.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/store/editActions.test.ts -t "seconds_to_frame_truncates_fractional_boundaries"`

  Expected: PASS with every candidate-bound assertion executed.

  Result: GREEN; the exact owner passed and the command executed the complete
  82-file Web suite: 775 tests passed, 0 failed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Result: `pnpm -C web test -- --run` passed 82 files / 775 tests and
  `pnpm -C web build` passed. Vite reported only the existing dynamic-import
  and bundle-size advisory warnings.

### Task 2: CC-first-video-settings (implementation-slice-699b13ae9742edd8)

**Covered records:**
- `requirement-76b4b09bfdb405be` (requirement)

**Files:**
- Modify: `web/src/store/editActions.ts#addMediaToTimelineAt`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `crates/opentake-ops/src/ops/settings.rs#set_timeline_settings`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Test (existing-owned): `crates/opentake-ops/src/command.rs#set_timeline_settings_is_undoable`
- Test (reviewed-planned): `web/src/lib/projectSettings.test.ts#first_video_auto_configures_and_only_configured_empty_mismatch_prompts`

**Candidate-bound contracts:**

#### requirement-76b4b09bfdb405be

- Candidate/source: `doc-1db26894665394a3` at `docs/architecture/MODULE-PORT-MAP.md:151` (requirement)
- Expected behavior: Apply first-clip project settings automatically and prompt only for the documented configured-empty mismatch case.
- Resolution: `reviewed-mapping-report:CC-first-video-settings` — Core mapping report CC-first-video-settings: the shared settings command exists, but first-import configuration and mismatch prompting are not connected.
- Exact acceptance contract:
  - Implementation: Implement a single checkProjectSettings decision function with the four documented branches, wire import UI to a ProjectSettingsMismatch dialog, apply settings through the undoable Rust command, and test no-video/fresh/nonempty/configured-empty mismatch paths.
  - Add request/response tests for every named success, boundary, rejection, validation, and secrecy rule; the affected Rust and TypeScript suites must pass.
  - Exercise the production IPC, MCP, or browser entry point end to end and record the exact command payload, result, and test names before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-ops/src/command.rs#set_timeline_settings_is_undoable` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/projectSettings.test.ts#first_video_auto_configures_and_only_configured_empty_mismatch_prompts` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

  Result: The exact reviewed owner now covers no-video, fresh-project,
  configured-nonempty, configured-empty matching/mismatching, and incomplete
  metadata branches. `editActions.test.ts` additionally proves the production
  placement command order and both mismatch choices; the dialog DOM owner
  verifies its accessible role, labels, focus, values, Match response, and
  Escape/Keep response. The Rust DTO owner asserts the `sourceFps` wire field.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-ops set_timeline_settings_is_undoable`
  - Run: `pnpm -C web test -- --run src/lib/projectSettings.test.ts -t "first_video_auto_configures_and_only_configured_empty_mismatch_prompts"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

  Result: RED reproduced before implementation: Vitest could not resolve
  `./projectSettings` from the declared exact owner because the decision module
  and its production integration did not exist.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/store/editActions.ts#addMediaToTimelineAt`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-ops/src/ops/settings.rs#set_timeline_settings`, `docs/architecture/MODULE-PORT-MAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Result: Added the pure four-branch decision, projected probed source FPS
  through the Tauri DTO, and reconciled settings on both append and positioned
  media entry points before placement. Fresh projects apply the existing
  undoable `setTimelineSettings` command first. Configured-empty mismatches wait
  for a globally mounted, keyboard-accessible dialog; keeping proceeds without
  mutation and matching applies the same command before placement. Pending
  choices are safely resolved when project runtime state resets.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-ops set_timeline_settings_is_undoable`
  - Run: `pnpm -C web test -- --run src/lib/projectSettings.test.ts -t "first_video_auto_configures_and_only_configured_empty_mismatch_prompts"`

  Expected: PASS with every candidate-bound assertion executed.

  Result: GREEN. The three focused Web owners passed 33/33 tests, including the
  exact named owner and production command/dialog integration. The exact Rust
  undo owner passed 1/1, and the Tauri media DTO owner passed 1/1.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: `cargo fmt --all -- --check`, `cargo test --workspace --no-fail-fast`,
  `pnpm -C web test -- --run`, and `pnpm -C web build` passed. The Web gate
  executed 84 files / 779 tests; Cargo's seven explicitly ignored real-device
  probes remain owned by the later packaged-app verification phase. Vite
  reported only the pre-existing dynamic-import and chunk-size advisories.

### Task 3: CC-automation-composite-headings (implementation-slice-7167819f4cff454a)

**Covered records:**
- `requirement-5c2a4b2f9e5abba6` (requirement)
- `requirement-a3187560b02b641a` (requirement)
- `requirement-2cf919a757c83ded` (requirement)
- `requirement-fca4710352ee2ce7` (requirement)
- `requirement-45ae268014531007` (requirement)
- `requirement-ed0b686740d56057` (requirement)
- `requirement-c8405288bd463439` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`
- Modify: `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`
- Modify: `crates/opentake-ops/src/intent.rs#plan_smart_reframe`
- Modify: `crates/opentake-media/src/analysis/beat.rs#detect_beats`
- Modify: `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md`
- Modify: `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed`

**Candidate-bound contracts:**

#### requirement-5c2a4b2f9e5abba6

- Candidate/source: `doc-c2871cf84ffdba1d` at `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:11` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:11 under “Design Rule” (heading), the source “## Design Rule” requires this exact behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.
- Resolution: `reviewed-mapping-report:CC-automation-composite-headings` — Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:11; signal=heading; heading=Design Rule; candidate=## Design Rule
  - Expected behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics. This closes only the promise expressed by “Design Rule” in “Design Rule”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Design Rule” with the scenario below and register test:tools/completion-tests/doc-c2871cf84ffdba1d.test.mjs#completion_c2871cf84ffdba1d_automation_plans_remain_deterministic_reviewable
  - Initial state/input/event: construct the smallest deterministic state that exposes “Design Rule”, apply the precise input or event implied by “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Design Rule”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-c2871cf84ffdba1d.test.mjs#completion_c2871cf84ffdba1d_automation_plans_remain_deterministic_reviewable.

#### requirement-a3187560b02b641a

- Candidate/source: `doc-2f4721e90945ec6a` at `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:27` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:27 under “Core Invariants” (heading), the source “## Core Invariants” requires this exact behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.
- Resolution: `reviewed-mapping-report:CC-automation-composite-headings` — Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:27; signal=heading; heading=Core Invariants; candidate=## Core Invariants
  - Expected behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics. This closes only the promise expressed by “Core Invariants” in “Core Invariants”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Core Invariants” with the scenario below and register test:tools/completion-tests/doc-2f4721e90945ec6a.test.mjs#completion_2f4721e90945ec6a_automation_plans_remain_deterministic_reviewable
  - Initial state/input/event: construct the smallest deterministic state that exposes “Core Invariants”, apply the precise input or event implied by “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Core Invariants”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-2f4721e90945ec6a.test.mjs#completion_2f4721e90945ec6a_automation_plans_remain_deterministic_reviewable.

#### requirement-2cf919a757c83ded

- Candidate/source: `doc-20499c491064a5e1` at `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:57` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:57 under “Failure Semantics” (heading), the source “## Failure Semantics” requires this exact behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.
- Resolution: `reviewed-mapping-report:CC-automation-composite-headings` — Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:57; signal=heading; heading=Failure Semantics; candidate=## Failure Semantics
  - Expected behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics. This closes only the promise expressed by “Failure Semantics” in “Failure Semantics”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Failure Semantics” with the scenario below and register test:tools/completion-tests/doc-20499c491064a5e1.test.mjs#completion_20499c491064a5e1_automation_plans_remain_deterministic_reviewable
  - Initial state/input/event: construct the smallest deterministic state that exposes “Failure Semantics”, apply the precise input or event implied by “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Failure Semantics”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-20499c491064a5e1.test.mjs#completion_20499c491064a5e1_automation_plans_remain_deterministic_reviewable.

#### requirement-fca4710352ee2ce7

- Candidate/source: `doc-5e0b6607a2fa6485` at `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:9` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:9 under “Documentation Checks” (heading), the source “## Documentation Checks” requires this exact behavior: The named automation checks have matching source and focused tests.
- Resolution: `reviewed-mapping-report:CC-automation-composite-headings` — Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:9; signal=heading; heading=Documentation Checks; candidate=## Documentation Checks
  - Expected behavior: The named automation checks have matching source and focused tests. This closes only the promise expressed by “Documentation Checks” in “Documentation Checks”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Documentation Checks” with the scenario below and register test:tools/completion-tests/doc-5e0b6607a2fa6485.test.mjs#completion_5e0b6607a2fa6485_the_named_automation_checks_have_matching_source
  - Initial state/input/event: construct the smallest deterministic state that exposes “Documentation Checks”, apply the precise input or event implied by “The named automation checks have matching source and focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “The named automation checks have matching source and focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Documentation Checks”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-5e0b6607a2fa6485.test.mjs#completion_5e0b6607a2fa6485_the_named_automation_checks_have_matching_source.

#### requirement-45ae268014531007

- Candidate/source: `doc-bddbf14c3d9a9ce7` at `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:20` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:20 under “Shared Implementation Checks” (heading), the source “## Shared Implementation Checks” requires this exact behavior: The named automation checks have matching source and focused tests.
- Resolution: `reviewed-mapping-report:CC-automation-composite-headings` — Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:20; signal=heading; heading=Shared Implementation Checks; candidate=## Shared Implementation Checks
  - Expected behavior: The named automation checks have matching source and focused tests. This closes only the promise expressed by “Shared Implementation Checks” in “Shared Implementation Checks”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Shared Implementation Checks” with the scenario below and register test:tools/completion-tests/doc-bddbf14c3d9a9ce7.test.mjs#completion_bddbf14c3d9a9ce7_the_named_automation_checks_have_matching_source
  - Initial state/input/event: construct the smallest deterministic state that exposes “Shared Implementation Checks”, apply the precise input or event implied by “The named automation checks have matching source and focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “The named automation checks have matching source and focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Shared Implementation Checks”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-bddbf14c3d9a9ce7.test.mjs#completion_bddbf14c3d9a9ce7_the_named_automation_checks_have_matching_source.

#### requirement-ed0b686740d56057

- Candidate/source: `doc-25baf0950c06110c` at `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:38` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:38 under “Beat Sync Checks” (heading), the source “## Beat Sync Checks” requires this exact behavior: The named automation checks have matching source and focused tests.
- Resolution: `reviewed-mapping-report:CC-automation-composite-headings` — Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:38; signal=heading; heading=Beat Sync Checks; candidate=## Beat Sync Checks
  - Expected behavior: The named automation checks have matching source and focused tests. This closes only the promise expressed by “Beat Sync Checks” in “Beat Sync Checks”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Beat Sync Checks” with the scenario below and register test:tools/completion-tests/doc-25baf0950c06110c.test.mjs#completion_25baf0950c06110c_the_named_automation_checks_have_matching_source
  - Initial state/input/event: construct the smallest deterministic state that exposes “Beat Sync Checks”, apply the precise input or event implied by “The named automation checks have matching source and focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “The named automation checks have matching source and focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Beat Sync Checks”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-25baf0950c06110c.test.mjs#completion_25baf0950c06110c_the_named_automation_checks_have_matching_source.

#### requirement-c8405288bd463439

- Candidate/source: `doc-6b9188efd87853b7` at `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:56` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:56 under “Minimum Local Verification” (heading), the source “## Minimum Local Verification” requires this exact behavior: The named automation checks have matching source and focused tests.
- Resolution: `reviewed-mapping-report:CC-automation-composite-headings` — Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:56; signal=heading; heading=Minimum Local Verification; candidate=## Minimum Local Verification
  - Expected behavior: The named automation checks have matching source and focused tests. This closes only the promise expressed by “Minimum Local Verification” in “Minimum Local Verification”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Minimum Local Verification” with the scenario below and register test:tools/completion-tests/doc-6b9188efd87853b7.test.mjs#completion_6b9188efd87853b7_the_named_automation_checks_have_matching_source
  - Initial state/input/event: construct the smallest deterministic state that exposes “Minimum Local Verification”, apply the precise input or event implied by “The named automation checks have matching source and focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “The named automation checks have matching source and focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Minimum Local Verification”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-6b9188efd87853b7.test.mjs#completion_6b9188efd87853b7_the_named_automation_checks_have_matching_source.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

  Result: Added the exact integration owner plus all seven candidate-bound Node
  completion owners. The integration fixture exercises deterministic beat and
  autocrop analysis, review-only MCP beat/silence results, typed smart-reframe
  unavailability, single-command reframe/beat plans, linked-A/V intent flags,
  rejected-plan no-op behavior, command-routed mutation, and exact undo.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test editing_automation_acceptance automation_children_are_atomic_reviewable_and_command_routed -- --exact`

  Expected: FAIL because one or more of the 7 candidate-bound contracts are not yet satisfied.

  Result: RED reproduced: Cargo reported that no
  `editing_automation_acceptance` test target existed, so the reviewed owner
  could not execute.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`, `crates/opentake-ops/src/intent.rs#plan_smart_reframe`, `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md`, `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Result: Review confirmed the mapped production children already satisfy this
  shared umbrella: pure detectors are deterministic/fail closed; Dispatcher
  preview tools do not apply; smart reframe returns a typed unavailable result;
  intent planners emit exactly one existing `EditCommand`; and the command layer
  owns mutation/undo. Added the missing cross-boundary acceptance suite and
  source-specific completion adapters, then recorded the concrete evidence in
  both source documents. No redundant production branch was introduced.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test editing_automation_acceptance automation_children_are_atomic_reviewable_and_command_routed -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

  Result: GREEN; the exact Rust owner passed 1/1 with `--exact`. All seven Node
  completion owners passed 7/7 and each verifies that Cargo executed exactly one
  owning test.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: `cargo fmt --all -- --check`, `cargo clippy -p opentake-agent --tests
  -- -D warnings`, `cargo test --workspace --no-fail-fast`, the exact Rust
  owner, all seven source owners, and the local relative Markdown-link gate
  passed. The workspace retained only its seven explicitly ignored real-device
  probes, which belong to packaged-app verification.

### Task 4: CC-beat-auto-cut (implementation-slice-d9ac8c92f3d41fea)

**Covered records:**
- `requirement-6697c7b7e306b700` (requirement)
- `requirement-b96e0201a585b70a` (requirement)
- `requirement-408dfad717402883` (requirement)
- `requirement-1eaff83716e1d766` (requirement)
- `requirement-122ba722ed1c9d81` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/analysis/beat.rs#detect_beats`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`
- Modify: `web/src/store/editActions.ts#applyAutomationCommands`
- Modify: `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md`
- Test (existing-owned): `crates/opentake-media/src/analysis/beat.rs#pulse_audio_detects_beat_frame_with_strength`
- Test (reviewed-planned): `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_false_is_read_only`
- Test (reviewed-planned): `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_true_is_one_atomic_command_and_preserves_links`
- Test (reviewed-planned): `crates/opentake-media/src/analysis/beat.rs#low_energy_speech_is_not_overdetected`

**Candidate-bound contracts:**

#### requirement-6697c7b7e306b700

- Candidate/source: `doc-ba234d45702696c8` at `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:9` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:9 under “V1 Scope” (heading), the source “## V1 Scope” requires this exact behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.
- Resolution: `reviewed-mapping-report:CC-beat-auto-cut` — Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:9; signal=heading; heading=V1 Scope; candidate=## V1 Scope
  - Expected behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands. This closes only the promise expressed by “V1 Scope” in “V1 Scope”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “V1 Scope” with the scenario below and register test:tools/completion-tests/doc-ba234d45702696c8.test.mjs#completion_ba234d45702696c8_beat_detection_inputs_produce_bounded_reviewable
  - Initial state/input/event: construct the smallest deterministic state that exposes “V1 Scope”, apply the precise input or event implied by “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “V1 Scope”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-ba234d45702696c8.test.mjs#completion_ba234d45702696c8_beat_detection_inputs_produce_bounded_reviewable.

#### requirement-b96e0201a585b70a

- Candidate/source: `doc-edfd998c6ede77c2` at `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:25` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:25 under “Detection Contract” (heading), the source “## Detection Contract” requires this exact behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.
- Resolution: `reviewed-mapping-report:CC-beat-auto-cut` — Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:25; signal=heading; heading=Detection Contract; candidate=## Detection Contract
  - Expected behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands. This closes only the promise expressed by “Detection Contract” in “Detection Contract”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Detection Contract” with the scenario below and register test:tools/completion-tests/doc-edfd998c6ede77c2.test.mjs#completion_edfd998c6ede77c2_beat_detection_inputs_produce_bounded_reviewable
  - Initial state/input/event: construct the smallest deterministic state that exposes “Detection Contract”, apply the precise input or event implied by “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Detection Contract”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-edfd998c6ede77c2.test.mjs#completion_edfd998c6ede77c2_beat_detection_inputs_produce_bounded_reviewable.

#### requirement-408dfad717402883

- Candidate/source: `doc-9b466eb6a0df9e51` at `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:51` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:51 under “Auto Cut Contract” (heading), the source “## Auto Cut Contract” requires this exact behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.
- Resolution: `reviewed-mapping-report:CC-beat-auto-cut` — Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:51; signal=heading; heading=Auto Cut Contract; candidate=## Auto Cut Contract
  - Expected behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands. This closes only the promise expressed by “Auto Cut Contract” in “Auto Cut Contract”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Auto Cut Contract” with the scenario below and register test:tools/completion-tests/doc-9b466eb6a0df9e51.test.mjs#completion_9b466eb6a0df9e51_beat_detection_inputs_produce_bounded_reviewable
  - Initial state/input/event: construct the smallest deterministic state that exposes “Auto Cut Contract”, apply the precise input or event implied by “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Auto Cut Contract”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-9b466eb6a0df9e51.test.mjs#completion_9b466eb6a0df9e51_beat_detection_inputs_produce_bounded_reviewable.

#### requirement-1eaff83716e1d766

- Candidate/source: `doc-9d16b84d588273ce` at `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:68` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:68 under “Safety Rules” (heading), the source “## Safety Rules” requires this exact behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.
- Resolution: `reviewed-mapping-report:CC-beat-auto-cut` — Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:68; signal=heading; heading=Safety Rules; candidate=## Safety Rules
  - Expected behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands. This closes only the promise expressed by “Safety Rules” in “Safety Rules”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Safety Rules” with the scenario below and register test:tools/completion-tests/doc-9d16b84d588273ce.test.mjs#completion_9d16b84d588273ce_beat_detection_inputs_produce_bounded_reviewable
  - Initial state/input/event: start from the smallest valid fixture for “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “Safety Rules”.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-9d16b84d588273ce.test.mjs#completion_9d16b84d588273ce_beat_detection_inputs_produce_bounded_reviewable.

#### requirement-122ba722ed1c9d81

- Candidate/source: `doc-e7610731e7410ad4` at `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:76` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:76 under “Acceptance Hooks” (heading), the source “## Acceptance Hooks” requires this exact behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.
- Resolution: `reviewed-mapping-report:CC-beat-auto-cut` — Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:76; signal=heading; heading=Acceptance Hooks; candidate=## Acceptance Hooks
  - Expected behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands. This closes only the promise expressed by “Acceptance Hooks” in “Acceptance Hooks”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Acceptance Hooks” with the scenario below and register test:tools/completion-tests/doc-e7610731e7410ad4.test.mjs#completion_e7610731e7410ad4_beat_detection_inputs_produce_bounded_reviewable
  - Initial state/input/event: construct the smallest deterministic state that exposes “Acceptance Hooks”, apply the precise input or event implied by “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Acceptance Hooks”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-e7610731e7410ad4.test.mjs#completion_e7610731e7410ad4_beat_detection_inputs_produce_bounded_reviewable.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/src/analysis/beat.rs#pulse_audio_detects_beat_frame_with_strength` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_false_is_read_only` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_true_is_one_atomic_command_and_preserves_links` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-media/src/analysis/beat.rs#low_energy_speech_is_not_overdetected` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

  Result: Added both exact Dispatcher owners and the low-energy detector owner,
  retained the pulse owner, and added a frontend atomic-adapter regression. Five
  source-bound Node owners execute all mapped boundaries and reject zero-test
  filters.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media pulse_audio_detects_beat_frame_with_strength`
  - Run: `cargo test -p opentake-agent auto_cut_to_beats_write_false_is_read_only`
  - Run: `cargo test -p opentake-agent auto_cut_to_beats_write_true_is_one_atomic_command_and_preserves_links`
  - Run: `cargo test -p opentake-media low_energy_speech_is_not_overdetected`

  Expected: FAIL because one or more of the 5 candidate-bound contracts are not yet satisfied.

  Result: RED reproduced in two independent failures. Quiet alternating speech
  energy generated false beats, and both Dispatcher owners rejected `write` as
  an unknown field before any auto-cut write path existed.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `web/src/store/editActions.ts#applyAutomationCommands`, `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Result: Added an absolute onset-energy floor before relative normalization.
  `auto_cut_to_beats` now accepts explicit `write` (default false), validates
  visual roots and beat capacity, expands linked A/V members with one shared
  delta, and applies exactly one existing `MoveClips` command. Preview remains
  read-only. The frontend automation adapter now refuses multi-request
  pseudo-transactions instead of serially committing partial edits; tool schema,
  descriptions, and DOS documents reflect the production behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media pulse_audio_detects_beat_frame_with_strength`
  - Run: `cargo test -p opentake-agent auto_cut_to_beats_write_false_is_read_only`
  - Run: `cargo test -p opentake-agent auto_cut_to_beats_write_true_is_one_atomic_command_and_preserves_links`
  - Run: `cargo test -p opentake-media low_energy_speech_is_not_overdetected`

  Expected: PASS with every candidate-bound assertion executed.

  Result: GREEN. Each of the four exact Rust owners passed 1/1, the frontend
  atomic owner passed 1/1, and all five source-bound owners passed 5/5.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. `cargo fmt --all -- --check`, the complete Web suite (84
  files / 780 tests), production Web build, and
  `cargo test --workspace --no-fail-fast` all completed successfully. The only
  Web build diagnostics were the repository's existing dynamic-import and
  chunk-size warnings; `git diff --check` was clean.

### Task 5: CC-event-forwarding-complete (implementation-slice-c35c845cbc3492dd)

**Covered records:**
- `requirement-8c2d9157594b8d92` (requirement)

**Files:**
- Modify: `crates/opentake-core/src/events.rs#CoreEvent`
- Modify: `src-tauri/src/lib.rs#forward_event`
- Modify: `docs/modules/src-tauri/setup-lib.md`
- Test (existing-owned): `crates/opentake-core/src/events.rs#core_event_serializes_with_kind_tag`
- Test (existing-owned): `crates/opentake-core/src/events.rs#media_changed_serializes_with_kind_tag`
- Test (reviewed-planned): `src-tauri/src/lib.rs#core_event_forwarding_maps_every_name_and_tagged_payload`
- Test (reviewed-planned): `src-tauri/src/lib.rs#core_event_forwarding_swallows_emit_failure_and_delivery_continues`

**Candidate-bound contracts:**

#### requirement-8c2d9157594b8d92

- Candidate/source: `doc-6c58cffd16de2d71` at `docs/modules/src-tauri/setup-lib.md:71` (requirement)
- Expected behavior: Forward core events with their tagged payload and treat WebView teardown emit failures as non-fatal.
- Resolution: `reviewed-mapping-report:CC-event-forwarding-complete` — Core mapping report CC-event-forwarding-complete: tagged events and intentional nonfatal WebView forwarding are implemented and need ledger evidence closure.
- Exact acceptance contract:
  - Extract or inject an event emitter boundary that can be exercised without a live Tauri window.
  - Focused tests assert every CoreEvent maps to the expected event name and tagged payload, and an emit failure is swallowed without panicking or affecting the core session.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-core/src/events.rs#core_event_serializes_with_kind_tag` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/events.rs#media_changed_serializes_with_kind_tag` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/lib.rs#core_event_forwarding_maps_every_name_and_tagged_payload` (reviewed-planned) — The emitted name and unchanged tagged payload for every variant are owned at the shell boundary.
  - `src-tauri/src/lib.rs#core_event_forwarding_swallows_emit_failure_and_delivery_continues` (reviewed-planned) — The best-effort failure policy is owned at the shell boundary without a live WebView.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

  Result: Extended the exact core serialization owner across all four variants.
  Added Tauri-boundary owners
  `core_event_forwarding_maps_every_name_and_tagged_payload` and
  `core_event_forwarding_swallows_emit_failure_and_delivery_continues`, using
  no live window or `AppHandle`.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-core core_event_serializes_with_kind_tag`
  - Run: `cargo test -p opentake-core media_changed_serializes_with_kind_tag`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

  Result: RED reproduced at the actual missing seam: the new Tauri owner failed
  to compile because `forward_core_event` did not exist. The two pre-existing
  core serialization tests alone could not exercise the WebView emit-failure
  policy.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-core/src/events.rs#CoreEvent`, `src-tauri/src/lib.rs#forward_event`, `docs/modules/src-tauri/setup-lib.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Result: Extracted a typed `forward_core_event` seam that exhaustively maps
  every variant, forwards the unchanged tagged event, and consumes emitter
  errors. `forward_event` retains the session side effects and delegates only
  emission; module documentation now records that boundary.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-core core_event_serializes_with_kind_tag`
  - Run: `cargo test -p opentake-core media_changed_serializes_with_kind_tag`

  Expected: PASS with every candidate-bound assertion executed.

  Result: GREEN. Both exact core owners and both Tauri boundary owners passed
  1/1. The failure owner also proved a later `EventBus` observer still receives
  the event after a simulated WebView teardown error.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. `cargo fmt --all -- --check` and
  `cargo test --workspace --no-fail-fast` completed successfully. The only
  diagnostic was the repository's existing future-incompatibility notice for
  transitive `block v0.1.6`.

### Task 6: CC-edit-gesture-parity (implementation-slice-6d2ce8b116a3ccd9)

**Covered records:**
- `requirement-8134b0c5922567c8` (requirement)
- `requirement-538b644e2cb07926` (requirement)

**Files:**
- Modify: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`
- Modify: `web/src/store/editActions.ts#buildMediaInsertPlan`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#EditRequest`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Test (existing-owned): `web/src/store/editActions.test.ts#forwards swapTracks for whole-track reordering`
- Test (reviewed-planned): `web/src/store/commandRouting.test.ts#every_edit_action_emits_exact_edit_request`
- Test (reviewed-planned): `web/src/lib/api.editApply.test.ts#edit_apply_forwards_exact_command_envelope`
- Test (reviewed-planned): `src-tauri/src/commands.rs#every_frontend_edit_request_deserializes_to_intended_command`

**Candidate-bound contracts:**

#### requirement-8134b0c5922567c8

- Candidate/source: `doc-c54c15a51ba84dc2` at `docs/modules/web/SPEC.md:1288` (requirement)
- Expected behavior: Prove every editing gesture maps to the exact intended edit_apply DTO and that no component mutates timeline data directly.
- Resolution: `reviewed-mapping-report:CC-edit-gesture-parity` — Core mapping report CC-edit-gesture-parity: focused gesture tests exist, but no closed inventory proves every UI path emits the exact shared request.
- Exact acceptance contract:
  - Build a generated/maintained mapping from each UI gesture and shortcut to one EditRequest variant plus expected backend command.
  - Component-level tests invoke every gesture, assert exact camelCase DTOs and failure/no-op UI, and a static check rejects direct timeline mutation outside fallback/projectStore internals.
  - Native contract tests deserialize every emitted DTO and prove it reaches the intended EditCommand.

#### requirement-538b644e2cb07926

- Candidate/source: `doc-8037f1b0a9056497` at `docs/specs/frontend/13-implementation.md:45` (requirement)
- Expected behavior: Map every editing gesture to the exact edit_apply command.
- Resolution: `reviewed-mapping-report:CC-edit-gesture-parity` — Core mapping report CC-edit-gesture-parity: focused gesture tests exist, but no closed inventory proves every UI path emits the exact shared request.
- Exact acceptance contract:
  - Implementation: Create a gesture-to-command contract matrix from spec 11.1, exercise every gesture and modifier, assert exact payload/no-op behavior, and verify one-step undo semantics.
  - Add request/response tests for every named success, boundary, rejection, validation, and secrecy rule; the affected Rust and TypeScript suites must pass.
  - Exercise the production IPC, MCP, or browser entry point end to end and record the exact command payload, result, and test names before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/store/editActions.test.ts#forwards swapTracks for whole-track reordering` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/store/commandRouting.test.ts#every_edit_action_emits_exact_edit_request` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/api.editApply.test.ts#edit_apply_forwards_exact_command_envelope` (reviewed-planned) — Production Tauri invocation must retain the exact command wrapper.
  - `src-tauri/src/commands.rs#every_frontend_edit_request_deserializes_to_intended_command` (reviewed-planned) — Native serde and command routing must cover the same exhaustive request set.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

  Result: Added the exact planned Web owner plus production IPC and native
  routing owners. The Web owner executes all 41 typed action routes, exact DTO
  shapes, representative no-ops, and a static direct-mutation guard.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/store/editActions.test.ts -t "forwards swapTracks for whole-track reordering"`
  - Run: `pnpm -C web test -- --run src/store/commandRouting.test.ts -t "every_edit_action_emits_exact_edit_request"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

  Result: RED reproduced: the planned owner loaded an undefined
  `EDIT_GESTURE_COMMAND_MATRIX`, proving no exhaustive production inventory
  existed. Audit also found `addTexts` and `removeTracks` request variants with
  no shared action wrapper.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`, `web/src/store/editActions.ts#buildMediaInsertPlan`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/13-implementation.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Result: Added an exhaustively typed 41-row gesture/action/request/backend
  matrix and the missing direct action wrappers. Every low-level action emits
  exactly one `EditRequest`, empty/same-target boundaries emit none, and the
  obsolete sequential `editApplyMany` pseudo-transaction was removed. The
  production IPC envelope and every Rust serde route now have focused owners;
  both acceptance documents record the concrete evidence.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/store/editActions.test.ts -t "forwards swapTracks for whole-track reordering"`
  - Run: `pnpm -C web test -- --run src/store/commandRouting.test.ts -t "every_edit_action_emits_exact_edit_request"`

  Expected: PASS with every candidate-bound assertion executed.

  Result: GREEN. The existing swap owner, exhaustive Web owner, production IPC
  owner, and 41-case native route owner each passed exactly once.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. Rust formatting, the complete Web suite (86 files / 782
  tests), production Web build, `cargo test --workspace --no-fail-fast`, and
  `git diff --check` all completed successfully. Only the repository's existing
  dynamic-import/chunk-size and transitive `block v0.1.6` diagnostics remained.

### Task 7: CC-readonly-versioned-mirror (implementation-slice-dc651cd267aea077)

**Covered records:**
- `requirement-cc6851a640c8585d` (requirement)
- `requirement-521a6bc7a7b845c6` (requirement)
- `requirement-dbce32105bffaabf` (requirement)

**Files:**
- Modify: `src-tauri/src/lib.rs#forward_event`
- Modify: `web/src/store/sync.ts#startSync`
- Modify: `web/src/store/projectStore.ts#useProjectStore`
- Modify: `docs/specs/core/4-frontend-sync.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Test (existing-owned): `web/src/store/sync.test.ts#does not let a late old snapshot replace a newer project`
- Test (reviewed-planned): `web/src/store/commandRouting.test.ts#project_store_has_no_timeline_mutator_and_refreshes_only_from_native_events`
- Test (reviewed-planned): `web/src/store/sync.test.ts#refetches when an event-promised version is newer than the first snapshot`
- Test (reviewed-planned): `web/src/store/sync.test.ts#converges to N+2 when N+1 and N+2 event refreshes complete out of order`
- Test (reviewed-planned): `web/src/store/sync.test.ts#never publishes a stale snapshot when catch-up retries are exhausted`

**Candidate-bound contracts:**

#### requirement-cc6851a640c8585d

- Candidate/source: `doc-6f44d7d3a7a9eabb` at `docs/specs/core/4-frontend-sync.md:1` (requirement)
- Expected behavior: Frontend timeline state is a read-only, version-ordered projection of Rust in every supported runtime.
- Resolution: `reviewed-mapping-report:CC-readonly-versioned-mirror` — Core mapping report CC-readonly-versioned-mirror: the versioned refresh path exists, but whole-store read-only projection ownership lacks a closed proof.
- Exact acceptance contract:
  - Make Rust TimelineDTO plus monotonically increasing version the sole production source for timeline/tracks/clips/media projection.
  - All UI/Agent edits must return/apply or refetch authoritative state; late events/responses may never overwrite a newer version.
  - Test two concurrent edits, response/event reordering, failed edit, undo/redo, project switch, and browser fallback isolation with exact final timeline/version equality.

#### requirement-521a6bc7a7b845c6

- Candidate/source: `doc-bd21d9f90422bf64` at `docs/specs/core/4-frontend-sync.md:3` (requirement)
- Expected behavior: Frontend timeline state is a read-only, version-ordered projection of Rust in every supported runtime.
- Resolution: `reviewed-mapping-report:CC-readonly-versioned-mirror` — Core mapping report CC-readonly-versioned-mirror: the versioned refresh path exists, but whole-store read-only projection ownership lacks a closed proof.
- Exact acceptance contract:
  - Enforce the three protocol rules at snapshot application and every edit call site: no direct mirror mutation, monotonic version acceptance, authoritative refetch after mutation.
  - Reject or ignore snapshots older than current and reset version/project identity atomically on project switch.
  - Instrument and test all edit/undo/redo paths with reordered versions N, N+2, N+1 plus a failed command; final state must equal Rust N+2.

#### requirement-dbce32105bffaabf

- Candidate/source: `doc-e216f163870c8652` at `docs/specs/frontend/13-implementation.md:44` (requirement)
- Expected behavior: Keep the frontend timeline mirror read-only and refresh it only from timeline_changed/get_timeline.
- Resolution: `reviewed-mapping-report:CC-readonly-versioned-mirror` — Core mapping report CC-readonly-versioned-mirror: the versioned refresh path exists, but whole-store read-only projection ownership lacks a closed proof.
- Exact acceptance contract:
  - Implementation: Audit every timeline write, enforce read-only types/store boundaries, route browser fallback through equivalent command handling, and add mutation-detection plus event-race tests.
  - Add request/response tests for every named success, boundary, rejection, validation, and secrecy rule; the affected Rust and TypeScript suites must pass.
  - Exercise the production IPC, MCP, or browser entry point end to end and record the exact command payload, result, and test names before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/store/sync.test.ts#does not let a late old snapshot replace a newer project` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/store/commandRouting.test.ts#project_store_has_no_timeline_mutator_and_refreshes_only_from_native_events` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/store/sync.test.ts#refetches when an event-promised version is newer than the first snapshot` (reviewed-planned) — An event's version floor must force a stale first response to retry.
  - `web/src/store/sync.test.ts#converges to N+2 when N+1 and N+2 event refreshes complete out of order` (reviewed-planned) — Concurrent edit event responses must converge to the newest authority.
  - `web/src/store/sync.test.ts#never publishes a stale snapshot when catch-up retries are exhausted` (reviewed-planned) — Bounded retry failure must remain a deterministic no-op.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

  Result: Added the exact planned store owner and three event-floor/concurrency
  owners. Existing project-switch/history tests remain in the same runner;
  schema and project-action suites now use only authoritative full snapshots.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/store/sync.test.ts -t "does not let a late old snapshot replace a newer project"`
  - Run: `pnpm -C web test -- --run src/store/commandRouting.test.ts -t "project_store_has_no_timeline_mutator_and_refreshes_only_from_native_events"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

  Result: RED reproduced twice. The project Store still exposed `setMirror`,
  and a `timeline_changed` promise for version 2 committed the first fetched
  version-1 snapshot without retrying.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/lib.rs#forward_event`, `web/src/store/sync.ts#startSync`, `web/src/store/projectStore.ts#useProjectStore`, `docs/specs/core/4-frontend-sync.md`, `docs/specs/frontend/13-implementation.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Result: Removed the bypass mutator. Full native/fallback snapshots now clone
  and recursively freeze on acceptance, reject older epochs and same-project
  versions, and preserve project identity atomically. Event-driven refreshes
  carry an epoch/version floor, retry stale responses up to a bounded limit,
  and publish nothing if the floor remains unmet; refresh generations make
  concurrent N+1/N+2 responses converge only to N+2.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/store/sync.test.ts -t "does not let a late old snapshot replace a newer project"`
  - Run: `pnpm -C web test -- --run src/store/commandRouting.test.ts -t "project_store_has_no_timeline_mutator_and_refreshes_only_from_native_events"`

  Expected: PASS with every candidate-bound assertion executed.

  Result: GREEN. Both originally named owners and all three explicit event
  concurrency/failure owners passed. The project schema, lifecycle, and edit
  action regression suites also passed after moving fixtures to the production
  full-snapshot boundary.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. Rust formatting, the complete Web suite (86 files / 786
  tests), production Web build, `cargo test --workspace --no-fail-fast`, and
  `git diff --check` all completed successfully. Only the pre-existing build
  diagnostics remained.

### Task 8: CC-tauri-command-contract (implementation-slice-00bdb59d43b2ad0c)

**Covered records:**
- `requirement-687bc44b4c9243a2` (requirement)
- `requirement-5ca0aab886c28dde` (requirement)
- `requirement-2c03433376733eb5` (requirement)
- `requirement-5558df9d0b133739` (requirement)
- `requirement-c924246b4d2b7ff9` (requirement)
- `requirement-cd691581ab4342b3` (requirement)
- `requirement-29da2e6af9281076` (requirement)

**Files:**
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#EditRequest`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `docs/specs/core/6-tauri-commands.md`
- Modify: `docs/specs/frontend/11-tauri.md`
- Test (existing-owned): `src-tauri/src/commands.rs#deserializes_camelcase_multiword_commands`
- Test (existing-owned): `src-tauri/src/commands.rs#deserializes_add_captions_camelcase_and_maps_to_command`
- Test (existing-owned): `src-tauri/src/commands.rs#deserializes_effect_commands_and_maps_to_ops_variants`
- Test (existing-owned): `src-tauri/src/commands.rs#deserializes_media_library_commands_and_maps_to_ops_variants`
- Test (reviewed-planned): `src-tauri/src/commands.rs#every_edit_request_maps_to_exact_edit_command`
- Test (reviewed-planned): `web/src/lib/api.commandContract.test.ts#frontend_command_names_match_invoke_handler`

**Candidate-bound contracts:**

#### requirement-687bc44b4c9243a2

- Candidate/source: `doc-47dfcfcf41dd2784` at `docs/specs/core/6-tauri-commands.md:1` (requirement)
- Expected behavior: At docs/specs/core/6-tauri-commands.md:1 under “6. Tauri command 表面(精确签名草案)” (heading), the source “## 6. Tauri command 表面(精确签名草案)” requires this exact behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts.
- Resolution: `reviewed-mapping-report:CC-tauri-command-contract` — Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Exact acceptance contract:
  - Source binding: docs/specs/core/6-tauri-commands.md:1; signal=heading; heading=6. Tauri command 表面(精确签名草案); candidate=## 6. Tauri command 表面(精确签名草案)
  - Expected behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts. This closes only the promise expressed by “6. Tauri command 表面(精确签名草案)” in “6. Tauri command 表面(精确签名草案)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6. Tauri command 表面(精确签名草案)” with the scenario below and register test:tools/completion-tests/doc-47dfcfcf41dd2784.test.mjs#completion_47dfcfcf41dd2784_tauri_exposes_typed_core_edit_commands_with_stab
  - Initial state/input/event: start from the smallest valid fixture for “Tauri exposes typed core/edit commands with stable DTO and error contracts.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6. Tauri command 表面(精确签名草案)”.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Tauri exposes typed core/edit commands with stable DTO and error contracts.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-47dfcfcf41dd2784.test.mjs#completion_47dfcfcf41dd2784_tauri_exposes_typed_core_edit_commands_with_stab.

#### requirement-5ca0aab886c28dde

- Candidate/source: `doc-b0fa71fc2443ba5a` at `docs/specs/core/6-tauri-commands.md:5` (requirement)
- Expected behavior: At docs/specs/core/6-tauri-commands.md:5 under “6.1 命令清单” (heading), the source “### 6.1 命令清单” requires this exact behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts.
- Resolution: `reviewed-mapping-report:CC-tauri-command-contract` — Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Exact acceptance contract:
  - Source binding: docs/specs/core/6-tauri-commands.md:5; signal=heading; heading=6.1 命令清单; candidate=### 6.1 命令清单
  - Expected behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts. This closes only the promise expressed by “6.1 命令清单” in “6.1 命令清单”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.1 命令清单” with the scenario below and register test:tools/completion-tests/doc-b0fa71fc2443ba5a.test.mjs#completion_b0fa71fc2443ba5a_tauri_exposes_typed_core_edit_commands_with_stab
  - Initial state/input/event: start from the smallest valid fixture for “Tauri exposes typed core/edit commands with stable DTO and error contracts.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6.1 命令清单”.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Tauri exposes typed core/edit commands with stable DTO and error contracts.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-b0fa71fc2443ba5a.test.mjs#completion_b0fa71fc2443ba5a_tauri_exposes_typed_core_edit_commands_with_stab.

#### requirement-2c03433376733eb5

- Candidate/source: `doc-5ec52eb6e8513033` at `docs/specs/core/6-tauri-commands.md:49` (requirement)
- Expected behavior: At docs/specs/core/6-tauri-commands.md:49 under “6.2 关键参数类型(对齐上游)” (heading), the source “### 6.2 关键参数类型(对齐上游)” requires this exact behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts.
- Resolution: `reviewed-mapping-report:CC-tauri-command-contract` — Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Exact acceptance contract:
  - Source binding: docs/specs/core/6-tauri-commands.md:49; signal=heading; heading=6.2 关键参数类型(对齐上游); candidate=### 6.2 关键参数类型(对齐上游)
  - Expected behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts. This closes only the promise expressed by “6.2 关键参数类型(对齐上游)” in “6.2 关键参数类型(对齐上游)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.2 关键参数类型(对齐上游)” with the scenario below and register test:tools/completion-tests/doc-5ec52eb6e8513033.test.mjs#completion_5ec52eb6e8513033_tauri_exposes_typed_core_edit_commands_with_stab
  - Initial state/input/event: start from the smallest valid fixture for “Tauri exposes typed core/edit commands with stable DTO and error contracts.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6.2 关键参数类型(对齐上游)”.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Tauri exposes typed core/edit commands with stable DTO and error contracts.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-5ec52eb6e8513033.test.mjs#completion_5ec52eb6e8513033_tauri_exposes_typed_core_edit_commands_with_stab.

#### requirement-5558df9d0b133739

- Candidate/source: `doc-4cd2927d804dc4a3` at `docs/specs/core/6-tauri-commands.md:73` (requirement)
- Expected behavior: At docs/specs/core/6-tauri-commands.md:73 under “6.3 错误约定” (heading), the source “### 6.3 错误约定” requires this exact behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts.
- Resolution: `reviewed-mapping-report:CC-tauri-command-contract` — Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Exact acceptance contract:
  - Source binding: docs/specs/core/6-tauri-commands.md:73; signal=heading; heading=6.3 错误约定; candidate=### 6.3 错误约定
  - Expected behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts. This closes only the promise expressed by “6.3 错误约定” in “6.3 错误约定”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.3 错误约定” with the scenario below and register test:tools/completion-tests/doc-4cd2927d804dc4a3.test.mjs#completion_4cd2927d804dc4a3_tauri_exposes_typed_core_edit_commands_with_stab
  - Initial state/input/event: start from the smallest valid fixture for “Tauri exposes typed core/edit commands with stable DTO and error contracts.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6.3 错误约定”.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Tauri exposes typed core/edit commands with stable DTO and error contracts.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-4cd2927d804dc4a3.test.mjs#completion_4cd2927d804dc4a3_tauri_exposes_typed_core_edit_commands_with_stab.

#### requirement-c924246b4d2b7ff9

- Candidate/source: `doc-43312d5e9f613913` at `docs/specs/core/6-tauri-commands.md:84` (requirement)
- Expected behavior: At docs/specs/core/6-tauri-commands.md:84 under “6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)” (heading), the source “### 6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)” requires this exact behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts.
- Resolution: `reviewed-mapping-report:CC-tauri-command-contract` — Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Exact acceptance contract:
  - Source binding: docs/specs/core/6-tauri-commands.md:84; signal=heading; heading=6.4 `edit_apply` 与 agent 工具的关系(避免重复定义); candidate=### 6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)
  - Expected behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts. This closes only the promise expressed by “6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)” in “6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)” with the scenario below and register test:crates/opentake-agent/tests/completion_43312d5e9f613913.rs#completion_43312d5e9f613913_tauri_exposes_typed_core_edit_commands_with_stab
  - Initial state/input/event: start from the smallest valid fixture for “Tauri exposes typed core/edit commands with stable DTO and error contracts.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)”.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Tauri exposes typed core/edit commands with stable DTO and error contracts.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_43312d5e9f613913.rs#completion_43312d5e9f613913_tauri_exposes_typed_core_edit_commands_with_stab.

#### requirement-cd691581ab4342b3

- Candidate/source: `doc-adfd8e77f6674234` at `docs/specs/frontend/11-tauri.md:1` (requirement)
- Expected behavior: Every command listed by the frontend specification maps to a live Rust implementation with typed errors.
- Resolution: `reviewed-mapping-report:CC-tauri-command-contract` — Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Exact acceptance contract:
  - Keep one typed TypeScript wrapper and one registered Rust handler for every advertised command/event, with matching names, request/response fields, and errors.
  - Remove or capability-gate generation/advanced-media commands whose backend remains unavailable; browser fallback must never report production success for them.
  - Generate/inventory parity tests and run success, malformed request, typed error, cancellation, and event ordering cases across the Tauri boundary.

#### requirement-29da2e6af9281076

- Candidate/source: `doc-96f7321f1d38c5ef` at `docs/specs/frontend/11-tauri.md:5` (requirement)
- Expected behavior: Every command listed by the frontend specification maps to a live Rust implementation with typed errors.
- Resolution: `reviewed-mapping-report:CC-tauri-command-contract` — Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Exact acceptance contract:
  - Map every listed edit/read/playback/project command to a live Rust handler and strict TS request/response type.
  - Unknown fields, invalid frames/IDs/paths, unavailable capability, and cancellation must return typed errors without partial mutation.
  - Table-drive invoke registration/schema parity plus success/failure/undo for each mutating command and packaged desktop smoke the command surface.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/commands.rs#deserializes_camelcase_multiword_commands` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_add_captions_camelcase_and_maps_to_command` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_effect_commands_and_maps_to_ops_variants` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_media_library_commands_and_maps_to_ops_variants` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#every_edit_request_maps_to_exact_edit_command` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/api.commandContract.test.ts#frontend_command_names_match_invoke_handler` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and reconcile the historical baseline**

  - Run: `cargo test -p opentake-tauri deserializes_camelcase_multiword_commands`
  - Run: `cargo test -p opentake-tauri deserializes_add_captions_camelcase_and_maps_to_command`
  - Run: `cargo test -p opentake-tauri deserializes_effect_commands_and_maps_to_ops_variants`
  - Run: `cargo test -p opentake-tauri deserializes_media_library_commands_and_maps_to_ops_variants`
  - Run: `cargo test -p opentake-tauri every_edit_request_maps_to_exact_edit_command`
  - Run: `pnpm -C web test -- --run src/lib/api.commandContract.test.ts -t "frontend_command_names_match_invoke_handler"`

  Expected: FAIL because one or more of the 7 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `src-tauri/src/commands.rs#edit_apply`, `docs/specs/core/6-tauri-commands.md`, `docs/specs/frontend/11-tauri.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri deserializes_camelcase_multiword_commands`
  - Run: `cargo test -p opentake-tauri deserializes_add_captions_camelcase_and_maps_to_command`
  - Run: `cargo test -p opentake-tauri deserializes_effect_commands_and_maps_to_ops_variants`
  - Run: `cargo test -p opentake-tauri deserializes_media_library_commands_and_maps_to_ops_variants`
  - Run: `cargo test -p opentake-tauri every_edit_request_maps_to_exact_edit_command`
  - Run: `pnpm -C web test -- --run src/lib/api.commandContract.test.ts -t "frontend_command_names_match_invoke_handler"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Reconciled 2026-08-01 as an already-integrated baseline. Git history proves
  the exhaustive Rust mapping owner, frontend invoke/handler parity owner,
  typed error boundary, strict DTO updates, and both specification corrections
  landed together in `f2805fc`; its parent contains neither planned owner, so
  manufacturing a RED state would be dishonest. All six declared focused
  owners pass on the current descendant, as do the four Node documentation
  completion tests and the Agent-side Rust completion test. Rustfmt and the
  full workspace gate pass, while the current full Web suite/build passed in
  the immediately preceding control slice. No production change was required.

### Task 9: CC-authority-persistence-mixed (implementation-slice-9296b54263dc8038)

**Covered records:**
- `requirement-224726c57b0ebb49` (requirement)
- `requirement-d9d443c2efd6fafd` (requirement)
- `requirement-12a60880e3a58578` (requirement)

**Files:**
- Modify: `web/src/store/sync.ts#startSync`
- Modify: `web/src/store/projectStore.ts#useProjectStore`
- Modify: `web/src/store/uiStore.ts#useEditorUiStore`
- Modify: `docs/specs/frontend/10-state.md`
- Test (reviewed-planned): `web/src/store/commandRouting.test.ts#rust_authority_and_ui_persistence_are_independently_owned`

**Candidate-bound contracts:**

#### requirement-224726c57b0ebb49

- Candidate/source: `doc-38a7e973e95ff30d` at `docs/specs/frontend/10-state.md:1` (requirement)
- Expected behavior: Rust timeline state remains authoritative and all required UI-only persistence survives restart without stale project data.
- Resolution: `reviewed-mapping-report:CC-authority-persistence-mixed` — Core mapping report CC-authority-persistence-mixed: these mixed records require composite acceptance across Rust authority projection and UI-only persistence.
- Exact acceptance contract:
  - Keep timeline/project domain data as a version-ordered read-only Rust projection and isolate selection/layout/dialog/hover state in the UI store.
  - Persist only approved UI keys with schema version and project scoping; never restore stale timeline/media state from localStorage.
  - Test out-of-order snapshots, concurrent commands, project switch, restart, migration/corrupt storage, and fallback isolation with authoritative refetch equality.

#### requirement-d9d443c2efd6fafd

- Candidate/source: `doc-8c9bed704675867f` at `docs/specs/frontend/10-state.md:5` (requirement)
- Expected behavior: Rust timeline state remains authoritative and all required UI-only persistence survives restart without stale project data.
- Resolution: `reviewed-mapping-report:CC-authority-persistence-mixed` — Core mapping report CC-authority-persistence-mixed: these mixed records require composite acceptance across Rust authority projection and UI-only persistence.
- Exact acceptance contract:
  - Prevent production code from directly mutating mirrored timeline/tracks/clips/media/version outside snapshot application.
  - Apply only snapshots whose version is not older than current and refetch authoritative TimelineDTO after every edit/undo/redo.
  - Static-check write sites and test out-of-order events, two concurrent edits, failed command, undo/redo, and project switch without stale rollback.

#### requirement-12a60880e3a58578

- Candidate/source: `doc-9cd688fa7c3b983c` at `docs/specs/frontend/10-state.md:96` (requirement)
- Expected behavior: Rust timeline state remains authoritative and all required UI-only persistence survives restart without stale project data.
- Resolution: `reviewed-mapping-report:CC-authority-persistence-mixed` — Core mapping report CC-authority-persistence-mixed: these mixed records require composite acceptance across Rust authority projection and UI-only persistence.
- Exact acceptance contract:
  - Persist only the specified layout, panel visibility/size, theme, zoom, and recent UI preferences with schema version and bounded values.
  - Exclude timeline, clips, media, credentials, transient drag/playback/dialog state, and clear or migrate malformed/old records safely.
  - Restart-test each approved key plus corrupt JSON, old schema, invalid bounds, project switch, logout, and secret scan.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/store/commandRouting.test.ts#rust_authority_and_ui_persistence_are_independently_owned` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and reconcile the historical baseline**

  - Run: `pnpm -C web test -- --run src/store/commandRouting.test.ts -t "rust_authority_and_ui_persistence_are_independently_owned"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/store/sync.ts#startSync`, `web/src/store/projectStore.ts#useProjectStore`, `web/src/store/uiStore.ts#useEditorUiStore`, `docs/specs/frontend/10-state.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/store/commandRouting.test.ts -t "rust_authority_and_ui_persistence_are_independently_owned"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Reconciled 2026-08-01 as an already-integrated baseline. The exact composite
  owner passes and proves the schema-versioned persistence allowlist, absence of
  project identifiers from persisted values, restart defaults for transient
  selection/playback/preview state, stale snapshot rejection, concurrent and
  failed edit isolation, and project-boundary UI reset. Git history shows the
  owner, persistence hardening, and corrected specification landed together in
  `8da75f9`; the parent lacks the owner, so there is no honest historical RED to
  replay. The current full Web suite (95 files / 832 tests) and production build
  passed in the immediately preceding slice. No production change was required.

### Task 10: control-acceptance (implementation-slice-251a11401ef969e1)

**Covered records:**
- `control-record-ba5f8f9b19f0fb1d` (control)

**Files:**
- Modify: `web/src/components/toolbar/Toolbar.tsx`
- Modify: `web/src/store/editActions.ts#undo`
- Modify: `web/src/lib/api.ts#undo`
- Modify: `src-tauri/src/commands.rs#undo`
- Modify: `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Test (reviewed-planned): `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-3be71cae61006e08 undo the last edit`

**Candidate-bound contracts:**

#### control-record-ba5f8f9b19f0fb1d

- Candidate/source: `control-3be71cae61006e08` at `web/src/components/toolbar/Toolbar.tsx:104:9` (control)
- Expected behavior: undo the last edit: edit.undo -> backend undo -> refresh mirror/canUndo/canRedo
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-3be71cae61006e08.
  - Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-3be71cae61006e08 undo the last edit.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=canUndo.
  - Event: inputs=["event/prop handler: {() => edit.undo()}","click or native keyboard activation plus current owning state"]; handler={() => edit.undo()}.
  - Exact call/state/backend: stateTransition=edit.undo -> backend undo -> refresh mirror/canUndo/canRedo; backendTrace=["web/src/components/toolbar/Toolbar.tsx:104::candidate handler -> {() => edit.undo()}","actual branch/state -> edit.undo -> backend undo -> refresh mirror/canUndo/canRedo","exact call/arguments -> edit.undo() -> api.undo() -> invoke('undo') exactly once when canUndo is true","web/src/store/editActions.ts::undo -> web/src/lib/api.ts::undo","web/src/lib/api.ts::undo -> invoke('undo')","src-tauri/src/commands.rs::undo -> opentake-core handle_undo","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#undo","code:web/src/lib/api.ts#undo","code:src-tauri/src/commands.rs#undo"].
  - Visible/accessibility/return path: success=undo the last edit: edit.undo -> backend undo -> refresh mirror/canUndo/canRedo; accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"toolbar.undo\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."].
  - Outcome matrix: {"success":"undo the last edit: edit.undo -> backend undo -> refresh mirror/canUndo/canRedo","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:104; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/toolbar/Toolbar.tsx:104; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {!canUndo}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.undo -> backend undo -> refresh mirror/canUndo/canRedo.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:104; the missing DOM test must prove whether it is surfaced or silent."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-3be71cae61006e08 undo the last edit` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and reconcile the historical baseline**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-3be71cae61006e08 undo the last edit"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#undo`, `web/src/lib/api.ts#undo`, `src-tauri/src/commands.rs#undo`, `web/src/components/toolbar/Toolbar.tsx#Toolbar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-3be71cae61006e08 undo the last edit"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Reconciled 2026-08-01 as an already-integrated baseline. The exact DOM owner
  passes and exercises disabled no-op, one-shot pending behavior, focus
  retention, successful completion, rejection feedback, retry readiness, and
  the source chain through `edit.undo`, the typed API wrapper, and Rust
  `handle_undo`. Git history shows the owner and recoverability fix landed
  together in `99661b7`; the parent has no owner to replay as RED. The current
  full Web/build and Rust workspace gates already pass. No production change
  was required.

### Task 11: control-acceptance (implementation-slice-a085393217648051)

**Covered records:**
- `control-record-a9840dea7abafb72` (control)

**Files:**
- Modify: `web/src/components/toolbar/Toolbar.tsx`
- Modify: `web/src/store/editActions.ts#redo`
- Modify: `web/src/lib/api.ts#redo`
- Modify: `src-tauri/src/commands.rs#redo`
- Modify: `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Test (reviewed-planned): `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-b001ac6b21c97ad0 redo the last undone edit`

**Candidate-bound contracts:**

#### control-record-a9840dea7abafb72

- Candidate/source: `control-b001ac6b21c97ad0` at `web/src/components/toolbar/Toolbar.tsx:107:9` (control)
- Expected behavior: redo the last undone edit: edit.redo -> backend redo -> refresh mirror/canUndo/canRedo
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-b001ac6b21c97ad0.
  - Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-b001ac6b21c97ad0 redo the last undone edit.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=canRedo.
  - Event: inputs=["event/prop handler: {() => edit.redo()}","click or native keyboard activation plus current owning state"]; handler={() => edit.redo()}.
  - Exact call/state/backend: stateTransition=edit.redo -> backend redo -> refresh mirror/canUndo/canRedo; backendTrace=["web/src/components/toolbar/Toolbar.tsx:107::candidate handler -> {() => edit.redo()}","actual branch/state -> edit.redo -> backend redo -> refresh mirror/canUndo/canRedo","exact call/arguments -> edit.redo() -> api.redo() -> invoke('redo') exactly once when canRedo is true","web/src/store/editActions.ts::redo -> web/src/lib/api.ts::redo","web/src/lib/api.ts::redo -> invoke('redo')","src-tauri/src/commands.rs::redo -> opentake-core handle_redo","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#redo","code:web/src/lib/api.ts#redo","code:src-tauri/src/commands.rs#redo"].
  - Visible/accessibility/return path: success=redo the last undone edit: edit.redo -> backend redo -> refresh mirror/canUndo/canRedo; accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"toolbar.redo\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."].
  - Outcome matrix: {"success":"redo the last undone edit: edit.redo -> backend redo -> refresh mirror/canUndo/canRedo","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:107; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/toolbar/Toolbar.tsx:107; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {!canRedo}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.redo -> backend redo -> refresh mirror/canUndo/canRedo.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:107; the missing DOM test must prove whether it is surfaced or silent."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-b001ac6b21c97ad0 redo the last undone edit` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and reconcile the historical baseline**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-b001ac6b21c97ad0 redo the last undone edit"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#redo`, `web/src/lib/api.ts#redo`, `src-tauri/src/commands.rs#redo`, `web/src/components/toolbar/Toolbar.tsx#Toolbar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-b001ac6b21c97ad0 redo the last undone edit"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Reconciled 2026-08-01 as an already-integrated baseline. The exact Redo DOM
  owner passes across disabled no-op, pending duplicate suppression, focus,
  success, failure feedback and retry while statically binding the frontend and
  Rust command chain. The owner and recoverability fix landed together in
  `436a30e`, and its parent lacks the owner; no artificial RED was introduced.
  The current complete Web/build and Rust workspace gates pass. No production
  change was required.

### Task 12: control-acceptance (implementation-slice-dc2e1dbf0bbdec19)

**Covered records:**
- `control-record-48e26828d85cd366` (control)
- `control-record-f74d763ee91db31c` (control)
- `control-record-06c8bde125e1a97d` (control)

**Files:**
- Modify: `web/src/components/toolbar/Toolbar.tsx`
- Modify: `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Test (reviewed-planned): `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-9d69468ce3479312 switch to Pointer tool`
- Test (reviewed-planned): `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-8105812f9d07bc93 switch to Razor tool`
- Test (reviewed-planned): `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-582e8fdf1d3d9e7e change timeline zoom`

**Candidate-bound contracts:**

#### control-record-48e26828d85cd366

- Candidate/source: `control-9d69468ce3479312` at `web/src/components/toolbar/Toolbar.tsx:116:9` (control)
- Expected behavior: switch to Pointer tool: setToolMode('pointer')
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-9d69468ce3479312.
  - Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-9d69468ce3479312 switch to Pointer tool.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setToolMode(\"pointer\")}","click or native keyboard activation plus current owning state"]; handler={() => setToolMode("pointer")}.
  - Exact call/state/backend: stateTransition=setToolMode('pointer'); backendTrace=["web/src/components/toolbar/Toolbar.tsx:116::candidate handler -> {() => setToolMode(\"pointer\")}","actual branch/state -> setToolMode('pointer')","exact call -> setToolMode('pointer')","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/toolbar/Toolbar.tsx#Toolbar"].
  - Visible/accessibility/return path: success=switch to Pointer tool: setToolMode('pointer'); accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"toolbar.pointer\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."].
  - Outcome matrix: {"success":"switch to Pointer tool: setToolMode('pointer')","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-f74d763ee91db31c

- Candidate/source: `control-8105812f9d07bc93` at `web/src/components/toolbar/Toolbar.tsx:123:9` (control)
- Expected behavior: switch to Razor tool: setToolMode('razor')
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-8105812f9d07bc93.
  - Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-8105812f9d07bc93 switch to Razor tool.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setToolMode(\"razor\")}","click or native keyboard activation plus current owning state"]; handler={() => setToolMode("razor")}.
  - Exact call/state/backend: stateTransition=setToolMode('razor'); backendTrace=["web/src/components/toolbar/Toolbar.tsx:123::candidate handler -> {() => setToolMode(\"razor\")}","actual branch/state -> setToolMode('razor')","exact call -> setToolMode('razor')","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/toolbar/Toolbar.tsx#Toolbar"].
  - Visible/accessibility/return path: success=switch to Razor tool: setToolMode('razor'); accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"toolbar.razor\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."].
  - Outcome matrix: {"success":"switch to Razor tool: setToolMode('razor')","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-06c8bde125e1a97d

- Candidate/source: `control-582e8fdf1d3d9e7e` at `web/src/components/toolbar/Toolbar.tsx:159:9` (control)
- Expected behavior: change timeline zoom: logarithmic slider -> setZoomScale
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-582e8fdf1d3d9e7e.
  - Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-582e8fdf1d3d9e7e change timeline zoom.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onSlider}","current control value and deterministic replacement value"]; handler={onSlider}.
  - Exact call/state/backend: stateTransition=logarithmic slider -> setZoomScale; backendTrace=["web/src/components/toolbar/Toolbar.tsx:159::candidate handler -> {onSlider}","actual branch/state -> logarithmic slider -> setZoomScale","exact call -> logarithmic slider -> setZoomScale","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/toolbar/Toolbar.tsx#Toolbar"].
  - Visible/accessibility/return path: success=change timeline zoom: logarithmic slider -> setZoomScale; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"toolbar.zoom\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."].
  - Outcome matrix: {"success":"change timeline zoom: logarithmic slider -> setZoomScale","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-9d69468ce3479312 switch to Pointer tool` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-8105812f9d07bc93 switch to Razor tool` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-582e8fdf1d3d9e7e change timeline zoom` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and reconcile the historical baseline**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-9d69468ce3479312 switch to Pointer tool"`
  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-8105812f9d07bc93 switch to Razor tool"`
  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-582e8fdf1d3d9e7e change timeline zoom"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/toolbar/Toolbar.tsx`, `web/src/components/toolbar/Toolbar.tsx#Toolbar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-9d69468ce3479312 switch to Pointer tool"`
  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-8105812f9d07bc93 switch to Razor tool"`
  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-582e8fdf1d3d9e7e change timeline zoom"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

  Reconciled 2026-08-01 as an already-integrated baseline. All three exact DOM
  owners pass and prove keyboard/click tool switching, selected-state feedback,
  focus behavior, and logarithmic zoom mapping with bounded store updates. The
  owners were added over the existing production implementation in `186f377`;
  its parent lacks the tests but not the behavior, so the initial honest result
  was GREEN. The current full Web suite and production build pass. No production
  change was required.

### Task 13: control-acceptance (implementation-slice-964535d9ff93a4e8)

**Covered records:**
- `control-record-f44c6f98dd0013ee` (control)

**Files:**
- Modify: `web/src/components/toolbar/Toolbar.tsx`
- Modify: `web/src/store/editActions.ts#splitAtPlayhead`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-ops/src/command.rs`
- Modify: `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Test (reviewed-planned): `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-c96abe84259649a3 split selected clips at playhead`

**Candidate-bound contracts:**

#### control-record-f44c6f98dd0013ee

- Candidate/source: `control-c96abe84259649a3` at `web/src/components/toolbar/Toolbar.tsx:136:9` (control)
- Expected behavior: split selected clips at playhead: edit.splitAtPlayhead
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c96abe84259649a3.
  - Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-c96abe84259649a3 split selected clips at playhead.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => edit.splitAtPlayhead()}","click or native keyboard activation plus current owning state"]; handler={() => edit.splitAtPlayhead()}.
  - Exact call/state/backend: stateTransition=edit.splitAtPlayhead; backendTrace=["web/src/components/toolbar/Toolbar.tsx:136::candidate handler -> {() => edit.splitAtPlayhead()}","actual branch/state -> edit.splitAtPlayhead","exact call/arguments -> splitAtPlayhead(): frame=Math.round(activeFrame); target selected clips or all clips strictly intersecting frame; for each target call editApply({type:'splitClip',clipId,atFrame:frame})","web/src/store/editActions.ts::splitAtPlayhead -> splitClip(id,frame) -> applyAndRefresh({type:'splitClip',clipId:id,atFrame:frame})","web/src/lib/api.ts::editApply -> invoke('edit_apply',{command})","src-tauri/src/commands.rs::edit_apply -> EditRequest::SplitClip -> crates/opentake-ops/src/command.rs","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#splitAtPlayhead","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"].
  - Visible/accessibility/return path: success=split selected clips at playhead: edit.splitAtPlayhead; accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"toolbar.split\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."].
  - Outcome matrix: {"success":"split selected clips at playhead: edit.splitAtPlayhead","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:136; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/toolbar/Toolbar.tsx:136; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.splitAtPlayhead.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:136; the missing DOM test must prove whether it is surfaced or silent."}.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-c96abe84259649a3 split selected clips at playhead` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and reconcile the historical baseline**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-c96abe84259649a3 split selected clips at playhead"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#splitAtPlayhead`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/toolbar/Toolbar.tsx#Toolbar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-c96abe84259649a3 split selected clips at playhead"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Reconciled 2026-08-01 as an already-integrated baseline. The exact Split DOM
  owner passes across selected and playhead-intersecting targets, no-target
  no-op, pending duplicate suppression, error feedback, retry and the typed
  command chain. The owner and toolbar guard fix landed together in `be4f186`;
  its parent has no owner to replay as RED. Current Web/build and Rust workspace
  gates pass. No production change was required.

### Task 14: control-acceptance (implementation-slice-922fa77f774ce4f6)

**Covered records:**
- `control-record-4338636c52466c65` (control)

**Files:**
- Modify: `web/src/components/toolbar/Toolbar.tsx`
- Modify: `web/src/store/editActions.ts#trimStartToPlayhead`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-ops/src/command.rs`
- Modify: `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Test (reviewed-planned): `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-f38c30bc83d65d2e trim selected clip starts to playhead`

**Candidate-bound contracts:**

#### control-record-4338636c52466c65

- Candidate/source: `control-f38c30bc83d65d2e` at `web/src/components/toolbar/Toolbar.tsx:139:9` (control)
- Expected behavior: trim selected clip starts to playhead: edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-f38c30bc83d65d2e.
  - Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-f38c30bc83d65d2e trim selected clip starts to playhead.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => edit.trimStartToPlayhead()}","click or native keyboard activation plus current owning state"]; handler={() => edit.trimStartToPlayhead()}.
  - Exact call/state/backend: stateTransition=edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips; backendTrace=["web/src/components/toolbar/Toolbar.tsx:139::candidate handler -> {() => edit.trimStartToPlayhead()}","actual branch/state -> edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips","exact call/arguments -> trimStartToPlayhead(): frame=Math.round(activeFrame); clipsUnderPlayhead(); trimToPlayheadEdits(clips,frame,'left'); editApply({type:'trimClips',edits})","web/src/store/editActions.ts::trimStartToPlayhead -> trimClips(left edits) -> applyAndRefresh({type:'trimClips',edits})","web/src/lib/api.ts::editApply -> invoke('edit_apply',{command:{type:'trimClips',edits}})","src-tauri/src/commands.rs::edit_apply -> EditRequest::TrimClips -> crates/opentake-ops/src/command.rs","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#trimStartToPlayhead","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"].
  - Visible/accessibility/return path: success=trim selected clip starts to playhead: edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips; accessibility={"focus":"Custom GlyphButton focus behavior depends on its implementation","label":"t(\"toolbar.trimStart\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."].
  - Outcome matrix: {"success":"trim selected clip starts to playhead: edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:139; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/toolbar/Toolbar.tsx:139; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:139; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-f38c30bc83d65d2e trim selected clip starts to playhead` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-f38c30bc83d65d2e trim selected clip starts to playhead"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#trimStartToPlayhead`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/toolbar/Toolbar.tsx#Toolbar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-f38c30bc83d65d2e trim selected clip starts to playhead"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 15: control-acceptance (implementation-slice-29b311273420852d)

**Covered records:**
- `control-record-db567c80607eddf4` (control)

**Files:**
- Modify: `web/src/components/toolbar/Toolbar.tsx`
- Modify: `web/src/store/editActions.ts#trimEndToPlayhead`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-ops/src/command.rs`
- Modify: `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Test (reviewed-planned): `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-eeff92d3d70361d9 trim selected clip ends to playhead`

**Candidate-bound contracts:**

#### control-record-db567c80607eddf4

- Candidate/source: `control-eeff92d3d70361d9` at `web/src/components/toolbar/Toolbar.tsx:144:9` (control)
- Expected behavior: trim selected clip ends to playhead: edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-eeff92d3d70361d9.
  - Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-eeff92d3d70361d9 trim selected clip ends to playhead.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => edit.trimEndToPlayhead()}","click or native keyboard activation plus current owning state"]; handler={() => edit.trimEndToPlayhead()}.
  - Exact call/state/backend: stateTransition=edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips; backendTrace=["web/src/components/toolbar/Toolbar.tsx:144::candidate handler -> {() => edit.trimEndToPlayhead()}","actual branch/state -> edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips","exact call/arguments -> trimEndToPlayhead(): frame=Math.round(activeFrame); clipsUnderPlayhead(); trimToPlayheadEdits(clips,frame,'right'); editApply({type:'trimClips',edits})","web/src/store/editActions.ts::trimEndToPlayhead -> trimClips(right edits) -> applyAndRefresh({type:'trimClips',edits})","web/src/lib/api.ts::editApply -> invoke('edit_apply',{command:{type:'trimClips',edits}})","src-tauri/src/commands.rs::edit_apply -> EditRequest::TrimClips -> crates/opentake-ops/src/command.rs","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#trimEndToPlayhead","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"].
  - Visible/accessibility/return path: success=trim selected clip ends to playhead: edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips; accessibility={"focus":"Custom GlyphButton focus behavior depends on its implementation","label":"t(\"toolbar.trimEnd\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."].
  - Outcome matrix: {"success":"trim selected clip ends to playhead: edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:144; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/toolbar/Toolbar.tsx:144; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:144; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-eeff92d3d70361d9 trim selected clip ends to playhead` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-eeff92d3d70361d9 trim selected clip ends to playhead"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#trimEndToPlayhead`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/toolbar/Toolbar.tsx#Toolbar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-eeff92d3d70361d9 trim selected clip ends to playhead"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 16: control-acceptance (implementation-slice-10afd9e138d27853)

**Covered records:**
- `control-record-8125c2e3848a32d1` (control)

**Files:**
- Modify: `web/src/components/toolbar/Toolbar.tsx`
- Modify: `web/src/store/editActions.ts#addTextClip`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-ops/src/command.rs`
- Modify: `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Test (reviewed-planned): `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-c6a658045b9e1d6c add a text clip`

**Candidate-bound contracts:**

#### control-record-8125c2e3848a32d1

- Candidate/source: `control-c6a658045b9e1d6c` at `web/src/components/toolbar/Toolbar.tsx:150:7` (control)
- Expected behavior: add a text clip: edit.addTextClip inserts/selects a new top-track text clip
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c6a658045b9e1d6c.
  - Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-c6a658045b9e1d6c add a text clip.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => edit.addTextClip()}","click or native keyboard activation plus current owning state"]; handler={() => edit.addTextClip()}.
  - Exact call/state/backend: stateTransition=edit.addTextClip inserts/selects a new top-track text clip; backendTrace=["web/src/components/toolbar/Toolbar.tsx:150::candidate handler -> {() => edit.addTextClip()}","actual branch/state -> edit.addTextClip inserts/selects a new top-track text clip","exact call/arguments -> addTextClip(): build one TextAutoTrackEntryReq at activeFrame with duration max(1,round(5*fps)), empty content/default style+transform; editApply({type:'addTextsAutoTrack',entries:[entry]})","web/src/store/editActions.ts::addTextClip -> applyAndRefresh({type:'addTextsAutoTrack',entries:[entry]}) -> optional forceRefresh/select affected ids","web/src/lib/api.ts::editApply -> invoke('edit_apply',{command:{type:'addTextsAutoTrack',entries:[entry]}})","src-tauri/src/commands.rs::edit_apply -> EditRequest::AddTextsAutoTrack -> crates/opentake-ops/src/command.rs","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#addTextClip","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"].
  - Visible/accessibility/return path: success=add a text clip: edit.addTextClip inserts/selects a new top-track text clip; accessibility={"focus":"Custom GlyphButton focus behavior depends on its implementation","label":"t(\"toolbar.addText\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."].
  - Outcome matrix: {"success":"add a text clip: edit.addTextClip inserts/selects a new top-track text clip","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:150; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.addTextClip inserts/selects a new top-track text clip.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:150; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-c6a658045b9e1d6c add a text clip` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-c6a658045b9e1d6c add a text clip"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#addTextClip`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/toolbar/Toolbar.tsx#Toolbar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.interaction.test.tsx -t "control-c6a658045b9e1d6c add a text clip"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

## Shared capability references

- `mcp-transport` / `implementation-slice-078ed22bfa23f28a`: implemented once in `data-safety`; this group contributes records `requirement-1990a4b0e0df5397`, `requirement-a4194cf440c740ca`, `requirement-12ca71ff2bf25b39` as acceptance references.
- `mcp-tool-import` / `implementation-slice-a5dcb81bd0966174`: implemented once in `data-safety`; this group contributes records `requirement-729157ed3496b88e`, `requirement-4440efda84f45010` as acceptance references.
- `mcp-error-redaction` / `implementation-slice-673f9e3f6002f97b`: implemented once in `data-safety`; this group contributes records `requirement-72876d064ac1a1c3` as acceptance references.
