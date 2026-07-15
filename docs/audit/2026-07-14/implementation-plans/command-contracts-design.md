# Command Contracts Completion Design

**Gap group:** `command-contracts`

**Records:** 45

**Implementation slices:** 19

## Architecture

Close each record as the smallest end-to-end vertical slice while preserving Rust-authoritative state, command/API parity, transactional safety, and explicit pending/empty/failure UI states. A record changes status only after its exact acceptance contract and strongest relevant runtime path pass.

## Record contracts

### requirement-0a3a8376bab5c2b6

- Kind: requirement
- Implementation slice: `implementation-slice-c067e28b3aa59ea5`
- Candidate: `doc-6759415bbf395caf`
- Source citation: `docs/architecture/BUGS.md:20`
- Exact files/symbols: `web/src/store/editActions.ts#mediaDurationFrames`, `web/src/store/editActions.ts#momentDurationFrames`, `web/src/lib/timelineInsert.ts#buildInsertPlan`, `docs/architecture/BUGS.md`
- Target resolution: `reviewed-mapping-report:CC-seconds-truncation`; matched `mediaDurationFrames`, `momentDurationFrames`, `buildInsertPlan`.
- Resolution rationale: Core mapping report CC-seconds-truncation: tracked second-to-frame helpers round fractional boundaries instead of truncating them.
- Test ownership:
  - `web/src/store/editActions.test.ts#seconds_to_frame_truncates_fractional_boundaries` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: All seconds-to-frame conversions obey the truncation contract at fractional boundaries.
- Acceptance criteria: Replace Math.round at mediaDurationFrames, momentDurationFrames, moment trim-start, and text-duration contract boundaries with the same truncation rule used by Rust. Keep explicitly nearest-frame UI interactions documented and separate so the conversion policy cannot drift between call sites. Add editActions cases for positive fractional values 0.49, 0.5, 0.99, and 1.01 frames at 24/30 fps plus negative/invalid inputs; assert exact parity with Rust conversion vectors.

### requirement-76b4b09bfdb405be

- Kind: requirement
- Implementation slice: `implementation-slice-699b13ae9742edd8`
- Candidate: `doc-1db26894665394a3`
- Source citation: `docs/architecture/MODULE-PORT-MAP.md:151`
- Exact files/symbols: `web/src/store/editActions.ts#addMediaToTimelineAt`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-ops/src/ops/settings.rs#set_timeline_settings`, `docs/architecture/MODULE-PORT-MAP.md`
- Target resolution: `reviewed-mapping-report:CC-first-video-settings`; matched `addMediaToTimelineAt`, `EditCommand`, `set_timeline_settings`.
- Resolution rationale: Core mapping report CC-first-video-settings: the shared settings command exists, but first-import configuration and mismatch prompting are not connected.
- Test ownership:
  - `crates/opentake-ops/src/command.rs#set_timeline_settings_is_undoable` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/projectSettings.test.ts#first_video_auto_configures_and_only_configured_empty_mismatch_prompts` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Apply first-clip project settings automatically and prompt only for the documented configured-empty mismatch case.
- Acceptance criteria: Implementation: Implement a single checkProjectSettings decision function with the four documented branches, wire import UI to a ProjectSettingsMismatch dialog, apply settings through the undoable Rust command, and test no-video/fresh/nonempty/configured-empty mismatch paths. Add request/response tests for every named success, boundary, rejection, validation, and secrecy rule; the affected Rust and TypeScript suites must pass. Exercise the production IPC, MCP, or browser entry point end to end and record the exact command payload, result, and test names before reclassification.

### requirement-5c2a4b2f9e5abba6

- Kind: requirement
- Implementation slice: `implementation-slice-7167819f4cff454a`
- Candidate: `doc-c2871cf84ffdba1d`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:11`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`, `crates/opentake-ops/src/intent.rs#plan_smart_reframe`, `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md`
- Target resolution: `reviewed-mapping-report:CC-automation-composite-headings`; matched `Dispatcher`, `detect_autocrop`, `plan_smart_reframe`, `detect_beats`.
- Resolution rationale: Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Test ownership:
  - `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:11 under “Design Rule” (heading), the source “## Design Rule” requires this exact behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:11; signal=heading; heading=Design Rule; candidate=## Design Rule Expected behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics. This closes only the promise expressed by “Design Rule” in “Design Rule”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Design Rule” with the scenario below and register test:tools/completion-tests/doc-c2871cf84ffdba1d.test.mjs#completion_c2871cf84ffdba1d_automation_plans_remain_deterministic_reviewable Initial state/input/event: construct the smallest deterministic state that exposes “Design Rule”, apply the precise input or event implied by “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “Design Rule”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-c2871cf84ffdba1d.test.mjs#completion_c2871cf84ffdba1d_automation_plans_remain_deterministic_reviewable.

### requirement-a3187560b02b641a

- Kind: requirement
- Implementation slice: `implementation-slice-7167819f4cff454a`
- Candidate: `doc-2f4721e90945ec6a`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:27`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`, `crates/opentake-ops/src/intent.rs#plan_smart_reframe`, `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md`
- Target resolution: `reviewed-mapping-report:CC-automation-composite-headings`; matched `Dispatcher`, `detect_autocrop`, `plan_smart_reframe`, `detect_beats`.
- Resolution rationale: Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Test ownership:
  - `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:27 under “Core Invariants” (heading), the source “## Core Invariants” requires this exact behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:27; signal=heading; heading=Core Invariants; candidate=## Core Invariants Expected behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics. This closes only the promise expressed by “Core Invariants” in “Core Invariants”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Core Invariants” with the scenario below and register test:tools/completion-tests/doc-2f4721e90945ec6a.test.mjs#completion_2f4721e90945ec6a_automation_plans_remain_deterministic_reviewable Initial state/input/event: construct the smallest deterministic state that exposes “Core Invariants”, apply the precise input or event implied by “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “Core Invariants”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-2f4721e90945ec6a.test.mjs#completion_2f4721e90945ec6a_automation_plans_remain_deterministic_reviewable.

### requirement-2cf919a757c83ded

- Kind: requirement
- Implementation slice: `implementation-slice-7167819f4cff454a`
- Candidate: `doc-20499c491064a5e1`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:57`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`, `crates/opentake-ops/src/intent.rs#plan_smart_reframe`, `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md`
- Target resolution: `reviewed-mapping-report:CC-automation-composite-headings`; matched `Dispatcher`, `detect_autocrop`, `plan_smart_reframe`, `detect_beats`.
- Resolution rationale: Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Test ownership:
  - `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:57 under “Failure Semantics” (heading), the source “## Failure Semantics” requires this exact behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md:57; signal=heading; heading=Failure Semantics; candidate=## Failure Semantics Expected behavior: Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics. This closes only the promise expressed by “Failure Semantics” in “Failure Semantics”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Failure Semantics” with the scenario below and register test:tools/completion-tests/doc-20499c491064a5e1.test.mjs#completion_20499c491064a5e1_automation_plans_remain_deterministic_reviewable Initial state/input/event: construct the smallest deterministic state that exposes “Failure Semantics”, apply the precise input or event implied by “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Automation plans remain deterministic, reviewable, and command-routed with typed failure semantics.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “Failure Semantics”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-20499c491064a5e1.test.mjs#completion_20499c491064a5e1_automation_plans_remain_deterministic_reviewable.

### requirement-fca4710352ee2ce7

- Kind: requirement
- Implementation slice: `implementation-slice-7167819f4cff454a`
- Candidate: `doc-5e0b6607a2fa6485`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:9`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`, `crates/opentake-ops/src/intent.rs#plan_smart_reframe`, `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md`
- Target resolution: `reviewed-mapping-report:CC-automation-composite-headings`; matched `Dispatcher`, `detect_autocrop`, `plan_smart_reframe`, `detect_beats`.
- Resolution rationale: Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Test ownership:
  - `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:9 under “Documentation Checks” (heading), the source “## Documentation Checks” requires this exact behavior: The named automation checks have matching source and focused tests.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:9; signal=heading; heading=Documentation Checks; candidate=## Documentation Checks Expected behavior: The named automation checks have matching source and focused tests. This closes only the promise expressed by “Documentation Checks” in “Documentation Checks”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Documentation Checks” with the scenario below and register test:tools/completion-tests/doc-5e0b6607a2fa6485.test.mjs#completion_5e0b6607a2fa6485_the_named_automation_checks_have_matching_source Initial state/input/event: construct the smallest deterministic state that exposes “Documentation Checks”, apply the precise input or event implied by “The named automation checks have matching source and focused tests.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “The named automation checks have matching source and focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “Documentation Checks”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-5e0b6607a2fa6485.test.mjs#completion_5e0b6607a2fa6485_the_named_automation_checks_have_matching_source.

### requirement-45ae268014531007

- Kind: requirement
- Implementation slice: `implementation-slice-7167819f4cff454a`
- Candidate: `doc-bddbf14c3d9a9ce7`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:20`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`, `crates/opentake-ops/src/intent.rs#plan_smart_reframe`, `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md`
- Target resolution: `reviewed-mapping-report:CC-automation-composite-headings`; matched `Dispatcher`, `detect_autocrop`, `plan_smart_reframe`, `detect_beats`.
- Resolution rationale: Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Test ownership:
  - `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:20 under “Shared Implementation Checks” (heading), the source “## Shared Implementation Checks” requires this exact behavior: The named automation checks have matching source and focused tests.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:20; signal=heading; heading=Shared Implementation Checks; candidate=## Shared Implementation Checks Expected behavior: The named automation checks have matching source and focused tests. This closes only the promise expressed by “Shared Implementation Checks” in “Shared Implementation Checks”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Shared Implementation Checks” with the scenario below and register test:tools/completion-tests/doc-bddbf14c3d9a9ce7.test.mjs#completion_bddbf14c3d9a9ce7_the_named_automation_checks_have_matching_source Initial state/input/event: construct the smallest deterministic state that exposes “Shared Implementation Checks”, apply the precise input or event implied by “The named automation checks have matching source and focused tests.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “The named automation checks have matching source and focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “Shared Implementation Checks”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-bddbf14c3d9a9ce7.test.mjs#completion_bddbf14c3d9a9ce7_the_named_automation_checks_have_matching_source.

### requirement-ed0b686740d56057

- Kind: requirement
- Implementation slice: `implementation-slice-7167819f4cff454a`
- Candidate: `doc-25baf0950c06110c`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:38`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`, `crates/opentake-ops/src/intent.rs#plan_smart_reframe`, `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md`
- Target resolution: `reviewed-mapping-report:CC-automation-composite-headings`; matched `Dispatcher`, `detect_autocrop`, `plan_smart_reframe`, `detect_beats`.
- Resolution rationale: Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Test ownership:
  - `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:38 under “Beat Sync Checks” (heading), the source “## Beat Sync Checks” requires this exact behavior: The named automation checks have matching source and focused tests.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:38; signal=heading; heading=Beat Sync Checks; candidate=## Beat Sync Checks Expected behavior: The named automation checks have matching source and focused tests. This closes only the promise expressed by “Beat Sync Checks” in “Beat Sync Checks”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Beat Sync Checks” with the scenario below and register test:tools/completion-tests/doc-25baf0950c06110c.test.mjs#completion_25baf0950c06110c_the_named_automation_checks_have_matching_source Initial state/input/event: construct the smallest deterministic state that exposes “Beat Sync Checks”, apply the precise input or event implied by “The named automation checks have matching source and focused tests.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “The named automation checks have matching source and focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “Beat Sync Checks”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-25baf0950c06110c.test.mjs#completion_25baf0950c06110c_the_named_automation_checks_have_matching_source.

### requirement-c8405288bd463439

- Kind: requirement
- Implementation slice: `implementation-slice-7167819f4cff454a`
- Candidate: `doc-6b9188efd87853b7`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:56`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`, `crates/opentake-ops/src/intent.rs#plan_smart_reframe`, `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md`
- Target resolution: `reviewed-mapping-report:CC-automation-composite-headings`; matched `Dispatcher`, `detect_autocrop`, `plan_smart_reframe`, `detect_beats`.
- Resolution rationale: Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.
- Test ownership:
  - `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:56 under “Minimum Local Verification” (heading), the source “## Minimum Local Verification” requires this exact behavior: The named automation checks have matching source and focused tests.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:56; signal=heading; heading=Minimum Local Verification; candidate=## Minimum Local Verification Expected behavior: The named automation checks have matching source and focused tests. This closes only the promise expressed by “Minimum Local Verification” in “Minimum Local Verification”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Minimum Local Verification” with the scenario below and register test:tools/completion-tests/doc-6b9188efd87853b7.test.mjs#completion_6b9188efd87853b7_the_named_automation_checks_have_matching_source Initial state/input/event: construct the smallest deterministic state that exposes “Minimum Local Verification”, apply the precise input or event implied by “The named automation checks have matching source and focused tests.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “The named automation checks have matching source and focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “Minimum Local Verification”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-6b9188efd87853b7.test.mjs#completion_6b9188efd87853b7_the_named_automation_checks_have_matching_source.

### requirement-6697c7b7e306b700

- Kind: requirement
- Implementation slice: `implementation-slice-d9ac8c92f3d41fea`
- Candidate: `doc-ba234d45702696c8`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:9`
- Exact files/symbols: `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `web/src/store/editActions.ts#applyAutomationCommands`, `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md`
- Target resolution: `reviewed-mapping-report:CC-beat-auto-cut`; matched `detect_beats`, `Dispatcher`, `applyAutomationCommands`.
- Resolution rationale: Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.
- Test ownership:
  - `crates/opentake-media/src/analysis/beat.rs#pulse_audio_detects_beat_frame_with_strength` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_false_is_read_only` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_true_is_one_atomic_command_and_preserves_links` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-media/src/analysis/beat.rs#low_energy_speech_is_not_overdetected` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:9 under “V1 Scope” (heading), the source “## V1 Scope” requires this exact behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:9; signal=heading; heading=V1 Scope; candidate=## V1 Scope Expected behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands. This closes only the promise expressed by “V1 Scope” in “V1 Scope”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “V1 Scope” with the scenario below and register test:tools/completion-tests/doc-ba234d45702696c8.test.mjs#completion_ba234d45702696c8_beat_detection_inputs_produce_bounded_reviewable Initial state/input/event: construct the smallest deterministic state that exposes “V1 Scope”, apply the precise input or event implied by “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “V1 Scope”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-ba234d45702696c8.test.mjs#completion_ba234d45702696c8_beat_detection_inputs_produce_bounded_reviewable.

### requirement-b96e0201a585b70a

- Kind: requirement
- Implementation slice: `implementation-slice-d9ac8c92f3d41fea`
- Candidate: `doc-edfd998c6ede77c2`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:25`
- Exact files/symbols: `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `web/src/store/editActions.ts#applyAutomationCommands`, `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md`
- Target resolution: `reviewed-mapping-report:CC-beat-auto-cut`; matched `detect_beats`, `Dispatcher`, `applyAutomationCommands`.
- Resolution rationale: Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.
- Test ownership:
  - `crates/opentake-media/src/analysis/beat.rs#pulse_audio_detects_beat_frame_with_strength` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_false_is_read_only` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_true_is_one_atomic_command_and_preserves_links` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-media/src/analysis/beat.rs#low_energy_speech_is_not_overdetected` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:25 under “Detection Contract” (heading), the source “## Detection Contract” requires this exact behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:25; signal=heading; heading=Detection Contract; candidate=## Detection Contract Expected behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands. This closes only the promise expressed by “Detection Contract” in “Detection Contract”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Detection Contract” with the scenario below and register test:tools/completion-tests/doc-edfd998c6ede77c2.test.mjs#completion_edfd998c6ede77c2_beat_detection_inputs_produce_bounded_reviewable Initial state/input/event: construct the smallest deterministic state that exposes “Detection Contract”, apply the precise input or event implied by “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “Detection Contract”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-edfd998c6ede77c2.test.mjs#completion_edfd998c6ede77c2_beat_detection_inputs_produce_bounded_reviewable.

### requirement-408dfad717402883

- Kind: requirement
- Implementation slice: `implementation-slice-d9ac8c92f3d41fea`
- Candidate: `doc-9b466eb6a0df9e51`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:51`
- Exact files/symbols: `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `web/src/store/editActions.ts#applyAutomationCommands`, `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md`
- Target resolution: `reviewed-mapping-report:CC-beat-auto-cut`; matched `detect_beats`, `Dispatcher`, `applyAutomationCommands`.
- Resolution rationale: Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.
- Test ownership:
  - `crates/opentake-media/src/analysis/beat.rs#pulse_audio_detects_beat_frame_with_strength` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_false_is_read_only` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_true_is_one_atomic_command_and_preserves_links` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-media/src/analysis/beat.rs#low_energy_speech_is_not_overdetected` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:51 under “Auto Cut Contract” (heading), the source “## Auto Cut Contract” requires this exact behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:51; signal=heading; heading=Auto Cut Contract; candidate=## Auto Cut Contract Expected behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands. This closes only the promise expressed by “Auto Cut Contract” in “Auto Cut Contract”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Auto Cut Contract” with the scenario below and register test:tools/completion-tests/doc-9b466eb6a0df9e51.test.mjs#completion_9b466eb6a0df9e51_beat_detection_inputs_produce_bounded_reviewable Initial state/input/event: construct the smallest deterministic state that exposes “Auto Cut Contract”, apply the precise input or event implied by “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “Auto Cut Contract”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-9b466eb6a0df9e51.test.mjs#completion_9b466eb6a0df9e51_beat_detection_inputs_produce_bounded_reviewable.

### requirement-1eaff83716e1d766

- Kind: requirement
- Implementation slice: `implementation-slice-d9ac8c92f3d41fea`
- Candidate: `doc-9d16b84d588273ce`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:68`
- Exact files/symbols: `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `web/src/store/editActions.ts#applyAutomationCommands`, `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md`
- Target resolution: `reviewed-mapping-report:CC-beat-auto-cut`; matched `detect_beats`, `Dispatcher`, `applyAutomationCommands`.
- Resolution rationale: Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.
- Test ownership:
  - `crates/opentake-media/src/analysis/beat.rs#pulse_audio_detects_beat_frame_with_strength` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_false_is_read_only` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_true_is_one_atomic_command_and_preserves_links` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-media/src/analysis/beat.rs#low_energy_speech_is_not_overdetected` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:68 under “Safety Rules” (heading), the source “## Safety Rules” requires this exact behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:68; signal=heading; heading=Safety Rules; candidate=## Safety Rules Expected behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands. This closes only the promise expressed by “Safety Rules” in “Safety Rules”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Safety Rules” with the scenario below and register test:tools/completion-tests/doc-9d16b84d588273ce.test.mjs#completion_9d16b84d588273ce_beat_detection_inputs_produce_bounded_reviewable Initial state/input/event: start from the smallest valid fixture for “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “Safety Rules”. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.” once. Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-9d16b84d588273ce.test.mjs#completion_9d16b84d588273ce_beat_detection_inputs_produce_bounded_reviewable.

### requirement-122ba722ed1c9d81

- Kind: requirement
- Implementation slice: `implementation-slice-d9ac8c92f3d41fea`
- Candidate: `doc-e7610731e7410ad4`
- Source citation: `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:76`
- Exact files/symbols: `crates/opentake-media/src/analysis/beat.rs#detect_beats`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `web/src/store/editActions.ts#applyAutomationCommands`, `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md`
- Target resolution: `reviewed-mapping-report:CC-beat-auto-cut`; matched `detect_beats`, `Dispatcher`, `applyAutomationCommands`.
- Resolution rationale: Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.
- Test ownership:
  - `crates/opentake-media/src/analysis/beat.rs#pulse_audio_detects_beat_frame_with_strength` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_false_is_read_only` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/src/mcp/dispatch.rs#auto_cut_to_beats_write_true_is_one_atomic_command_and_preserves_links` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-media/src/analysis/beat.rs#low_energy_speech_is_not_overdetected` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:76 under “Acceptance Hooks” (heading), the source “## Acceptance Hooks” requires this exact behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.
- Acceptance criteria: Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md:76; signal=heading; heading=Acceptance Hooks; candidate=## Acceptance Hooks Expected behavior: Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands. This closes only the promise expressed by “Acceptance Hooks” in “Acceptance Hooks”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Acceptance Hooks” with the scenario below and register test:tools/completion-tests/doc-e7610731e7410ad4.test.mjs#completion_e7610731e7410ad4_beat_detection_inputs_produce_bounded_reviewable Initial state/input/event: construct the smallest deterministic state that exposes “Acceptance Hooks”, apply the precise input or event implied by “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.”, and include one success plus one boundary/failure case. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Beat detection inputs produce bounded, reviewable cut/placement plans without bypassing shared commands.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation. Visible/returned assertion: assert the exact returned success/error and the observable state described by “Acceptance Hooks”, including deterministic no-op behavior when the operation is rejected. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-e7610731e7410ad4.test.mjs#completion_e7610731e7410ad4_beat_detection_inputs_produce_bounded_reviewable.

### requirement-1990a4b0e0df5397

- Kind: requirement
- Implementation slice: `implementation-slice-078ed22bfa23f28a`
- Candidate: `doc-5f4c21fe798697e4`
- Source citation: `docs/modules/opentake-agent/SPEC.md:118`
- Exact files/symbols: `crates/opentake-agent/src/mcp/server.rs#McpServer`, `crates/opentake-agent/src/mcp/server.rs#serve_with_bridge`, `docs/modules/opentake-agent/SPEC.md`
- Target resolution: `reviewed-mapping-report:CC-mcp-transport`; matched `McpServer`, `serve_with_bridge`.
- Resolution rationale: Core mapping report CC-mcp-transport: these records share the data-safety MCP transport capability rather than separate command implementations.
- Test ownership:
  - `crates/opentake-agent/tests/mcp_http.rs#non_local_origin_is_rejected` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#oversized_request_body_is_rejected` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#serve_rejects_non_loopback_bind` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/tests/mcp_http.rs#unsupported_protocol_version_is_400` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Validate an optional MCP-Protocol-Version header against the rmcp-supported protocol set and reject unsupported values with HTTP 400.
- Acceptance criteria: Add a protocol-version guard to the MCP router after request-size and loopback checks. Transport tests cover absent, every supported, malformed, and unsupported version headers and prove the unsupported request never reaches dispatch.

### requirement-729157ed3496b88e

- Kind: requirement
- Implementation slice: `implementation-slice-a5dcb81bd0966174`
- Candidate: `doc-d1dee193811e9dbb`
- Source citation: `docs/modules/opentake-agent/SPEC.md:1036`
- Exact files/symbols: `crates/opentake-agent/src/tools/errors.rs#decode_tool_args`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `src-tauri/src/mcp.rs#TauriMediaBridge`, `docs/modules/opentake-agent/SPEC.md`
- Target resolution: `reviewed-mapping-report:CC-mcp-validation-import`; matched `decode_tool_args`, `Dispatcher`, `TauriMediaBridge`.
- Resolution rationale: Core mapping report CC-mcp-validation-import: command validation and safe URL import are the shared data-safety MCP import capability.
- Test ownership:
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_requires_exactly_one_source` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_rejects_unknown_nested_source_key` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_bytes_rejects_oversized_base64_before_bridge` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/tool_argument_contract.rs#all_tool_schemas_reject_unknown_missing_wrong_type` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `src-tauri/src/mcp.rs#https_url_import_enforces_scheme_mime_and_decoded_limit` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Apply precise-path unknown-field, finite-number, missing-field, and type validation to every top-level and nested tool argument object.
- Acceptance criteria: Every nested object type, including transform, RGB, mask center/radius, effect params policy, motion params, and import source, has an explicit allowed-key/value validator rather than relying on serde's unknown-field dropping. Table-driven dispatch tests cover missing/type/unknown/non-finite cases at paths such as entries[3].startFrame and assert exact LLM-facing messages. Delete or replace the current non-finite unit test that cannot construct a non-finite serde_json::Number with a transport/parser-level proof.

### requirement-8c2d9157594b8d92

- Kind: requirement
- Implementation slice: `implementation-slice-c35c845cbc3492dd`
- Candidate: `doc-6c58cffd16de2d71`
- Source citation: `docs/modules/src-tauri/setup-lib.md:71`
- Exact files/symbols: `crates/opentake-core/src/events.rs#CoreEvent`, `src-tauri/src/lib.rs#forward_event`, `docs/modules/src-tauri/setup-lib.md`
- Target resolution: `reviewed-mapping-report:CC-event-forwarding-complete`; matched `CoreEvent`, `forward_event`.
- Resolution rationale: Core mapping report CC-event-forwarding-complete: tagged events and intentional nonfatal WebView forwarding are implemented and need ledger evidence closure.
- Test ownership:
  - `crates/opentake-core/src/events.rs#core_event_serializes_with_kind_tag` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/events.rs#media_changed_serializes_with_kind_tag` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: Forward core events with their tagged payload and treat WebView teardown emit failures as non-fatal.
- Acceptance criteria: Extract or inject an event emitter boundary that can be exercised without a live Tauri window. Focused tests assert every CoreEvent maps to the expected event name and tagged payload, and an emit failure is swallowed without panicking or affecting the core session.

### requirement-8134b0c5922567c8

- Kind: requirement
- Implementation slice: `implementation-slice-6d2ce8b116a3ccd9`
- Candidate: `doc-c54c15a51ba84dc2`
- Source citation: `docs/modules/web/SPEC.md:1288`
- Exact files/symbols: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`, `web/src/store/editActions.ts#buildMediaInsertPlan`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `docs/modules/web/SPEC.md`
- Target resolution: `reviewed-mapping-report:CC-edit-gesture-parity`; matched `TimelineContainer`, `buildMediaInsertPlan`, `editApply`, `EditRequest`.
- Resolution rationale: Core mapping report CC-edit-gesture-parity: focused gesture tests exist, but no closed inventory proves every UI path emits the exact shared request.
- Test ownership:
  - `web/src/store/editActions.test.ts#forwards swapTracks for whole-track reordering` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/store/commandRouting.test.ts#every_edit_action_emits_exact_edit_request` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Prove every editing gesture maps to the exact intended edit_apply DTO and that no component mutates timeline data directly.
- Acceptance criteria: Build a generated/maintained mapping from each UI gesture and shortcut to one EditRequest variant plus expected backend command. Component-level tests invoke every gesture, assert exact camelCase DTOs and failure/no-op UI, and a static check rejects direct timeline mutation outside fallback/projectStore internals. Native contract tests deserialize every emitted DTO and prove it reaches the intended EditCommand.

### requirement-a4194cf440c740ca

- Kind: requirement
- Implementation slice: `implementation-slice-078ed22bfa23f28a`
- Candidate: `doc-a9bed8f0d9151ee5`
- Source citation: `docs/specs/agent/1-mcp-server.md:82`
- Exact files/symbols: `crates/opentake-agent/src/mcp/server.rs#McpServer`, `crates/opentake-agent/src/mcp/server.rs#serve_with_bridge`, `docs/specs/agent/1-mcp-server.md`
- Target resolution: `reviewed-mapping-report:CC-mcp-transport`; matched `McpServer`, `serve_with_bridge`.
- Resolution rationale: Core mapping report CC-mcp-transport: these records share the data-safety MCP transport capability rather than separate command implementations.
- Test ownership:
  - `crates/opentake-agent/tests/mcp_http.rs#non_local_origin_is_rejected` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#oversized_request_body_is_rejected` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#serve_rejects_non_loopback_bind` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/tests/mcp_http.rs#unsupported_protocol_version_is_400` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Reject unsupported MCP-Protocol-Version request headers with HTTP 400.
- Acceptance criteria: Implementation: Add a protocol-version middleware using rmcp's supported negotiation set, accept a missing header, return 400 for unsupported values before session dispatch, and add raw HTTP integration tests for supported/missing/unsupported headers. Add request/response tests for every named success, boundary, rejection, validation, and secrecy rule; the affected Rust and TypeScript suites must pass. Exercise the production IPC, MCP, or browser entry point end to end and record the exact command payload, result, and test names before reclassification.

### requirement-12ca71ff2bf25b39

- Kind: requirement
- Implementation slice: `implementation-slice-078ed22bfa23f28a`
- Candidate: `doc-788d61b7ed7a0e56`
- Source citation: `docs/specs/agent/10-implementation.md:84`
- Exact files/symbols: `crates/opentake-agent/src/mcp/server.rs#McpServer`, `crates/opentake-agent/src/mcp/server.rs#serve_with_bridge`, `docs/specs/agent/10-implementation.md`
- Target resolution: `reviewed-mapping-report:CC-mcp-transport`; matched `McpServer`, `serve_with_bridge`.
- Resolution rationale: Core mapping report CC-mcp-transport: these records share the data-safety MCP transport capability rather than separate command implementations.
- Test ownership:
  - `crates/opentake-agent/tests/mcp_http.rs#non_local_origin_is_rejected` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#oversized_request_body_is_rejected` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#serve_rejects_non_loopback_bind` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/tests/mcp_http.rs#unsupported_protocol_version_is_400` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Bind only loopback and enforce every required transport guard.
- Acceptance criteria: Implementation: Retain 127.0.0.1 binding, Origin/Host rejection, and body cap; add the missing MCP protocol-version guard and raw transport tests proving all guards execute before dispatch. Add request/response tests for every named success, boundary, rejection, validation, and secrecy rule; the affected Rust and TypeScript suites must pass. Exercise the production IPC, MCP, or browser entry point end to end and record the exact command payload, result, and test names before reclassification.

### requirement-4440efda84f45010

- Kind: requirement
- Implementation slice: `implementation-slice-a5dcb81bd0966174`
- Candidate: `doc-0d9a59b08aebafdd`
- Source citation: `docs/specs/agent/10-implementation.md:86`
- Exact files/symbols: `crates/opentake-agent/src/tools/errors.rs#decode_tool_args`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `src-tauri/src/mcp.rs#TauriMediaBridge`, `docs/specs/agent/10-implementation.md`
- Target resolution: `reviewed-mapping-report:CC-mcp-validation-import`; matched `decode_tool_args`, `Dispatcher`, `TauriMediaBridge`.
- Resolution rationale: Core mapping report CC-mcp-validation-import: command validation and safe URL import are the shared data-safety MCP import capability.
- Test ownership:
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_requires_exactly_one_source` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_rejects_unknown_nested_source_key` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_bytes_rejects_oversized_base64_before_bridge` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/tool_argument_contract.rs#all_tool_schemas_reject_unknown_missing_wrong_type` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `src-tauri/src/mcp.rs#https_url_import_enforces_scheme_mime_and_decoded_limit` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Validate all tool inputs and support safe HTTPS URL import with whitelist and 1 GiB cap.
- Acceptance criteria: Implementation: Audit every tool/nested object through ToolArgs validation; implement streamed HTTPS-only URL download with redirect/host policy, extension or MIME whitelist, 1 GiB hard cap before/during transfer, atomic staging and cleanup; add SSRF/redirect/oversize/type tests. Add request/response tests for every named success, boundary, rejection, validation, and secrecy rule; the affected Rust and TypeScript suites must pass. Exercise the production IPC, MCP, or browser entry point end to end and record the exact command payload, result, and test names before reclassification.

### requirement-72876d064ac1a1c3

- Kind: requirement
- Implementation slice: `implementation-slice-673f9e3f6002f97b`
- Candidate: `doc-02b35d8f21d13198`
- Source citation: `docs/specs/agent/10-implementation.md:87`
- Exact files/symbols: `src-tauri/src/mcp.rs#TauriMediaBridge`, `crates/opentake-agent/src/mcp/convert.rs#to_call_tool_result`, `docs/specs/agent/10-implementation.md`
- Target resolution: `reviewed-mapping-report:CC-mcp-error-redaction`; matched `TauriMediaBridge`, `to_call_tool_result`.
- Resolution rationale: Core mapping report CC-mcp-error-redaction: this record is the command-contract view of the shared LLM error redaction boundary.
- Test ownership:
  - `crates/opentake-agent/tests/mcp_error_redaction.rs#llm_errors_redact_paths_credentials_headers_provider_bodies` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: LLM-facing errors never expose internal paths, credentials, or sensitive service details.
- Acceptance criteria: Implementation: Introduce a typed error boundary that logs redacted diagnostics locally and returns stable public codes/messages; scrub filesystem paths, URLs with credentials, keyring/provider errors, and model paths; add adversarial secret/path leakage tests across MCP and chat. Add request/response tests for every named success, boundary, rejection, validation, and secrecy rule; the affected Rust and TypeScript suites must pass. Exercise the production IPC, MCP, or browser entry point end to end and record the exact command payload, result, and test names before reclassification.

### requirement-cc6851a640c8585d

- Kind: requirement
- Implementation slice: `implementation-slice-dc651cd267aea077`
- Candidate: `doc-6f44d7d3a7a9eabb`
- Source citation: `docs/specs/core/4-frontend-sync.md:1`
- Exact files/symbols: `src-tauri/src/lib.rs#forward_event`, `web/src/store/sync.ts#startSync`, `web/src/store/projectStore.ts#useProjectStore`, `docs/specs/core/4-frontend-sync.md`
- Target resolution: `reviewed-mapping-report:CC-readonly-versioned-mirror`; matched `forward_event`, `startSync`, `useProjectStore`.
- Resolution rationale: Core mapping report CC-readonly-versioned-mirror: the versioned refresh path exists, but whole-store read-only projection ownership lacks a closed proof.
- Test ownership:
  - `web/src/store/sync.test.ts#does not let a late old snapshot replace a newer project` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/store/commandRouting.test.ts#project_store_has_no_timeline_mutator_and_refreshes_only_from_native_events` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Frontend timeline state is a read-only, version-ordered projection of Rust in every supported runtime.
- Acceptance criteria: Make Rust TimelineDTO plus monotonically increasing version the sole production source for timeline/tracks/clips/media projection. All UI/Agent edits must return/apply or refetch authoritative state; late events/responses may never overwrite a newer version. Test two concurrent edits, response/event reordering, failed edit, undo/redo, project switch, and browser fallback isolation with exact final timeline/version equality.

### requirement-521a6bc7a7b845c6

- Kind: requirement
- Implementation slice: `implementation-slice-dc651cd267aea077`
- Candidate: `doc-bd21d9f90422bf64`
- Source citation: `docs/specs/core/4-frontend-sync.md:3`
- Exact files/symbols: `src-tauri/src/lib.rs#forward_event`, `web/src/store/sync.ts#startSync`, `web/src/store/projectStore.ts#useProjectStore`, `docs/specs/core/4-frontend-sync.md`
- Target resolution: `reviewed-mapping-report:CC-readonly-versioned-mirror`; matched `forward_event`, `startSync`, `useProjectStore`.
- Resolution rationale: Core mapping report CC-readonly-versioned-mirror: the versioned refresh path exists, but whole-store read-only projection ownership lacks a closed proof.
- Test ownership:
  - `web/src/store/sync.test.ts#does not let a late old snapshot replace a newer project` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/store/commandRouting.test.ts#project_store_has_no_timeline_mutator_and_refreshes_only_from_native_events` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Frontend timeline state is a read-only, version-ordered projection of Rust in every supported runtime.
- Acceptance criteria: Enforce the three protocol rules at snapshot application and every edit call site: no direct mirror mutation, monotonic version acceptance, authoritative refetch after mutation. Reject or ignore snapshots older than current and reset version/project identity atomically on project switch. Instrument and test all edit/undo/redo paths with reordered versions N, N+2, N+1 plus a failed command; final state must equal Rust N+2.

### requirement-687bc44b4c9243a2

- Kind: requirement
- Implementation slice: `implementation-slice-00bdb59d43b2ad0c`
- Candidate: `doc-47dfcfcf41dd2784`
- Source citation: `docs/specs/core/6-tauri-commands.md:1`
- Exact files/symbols: `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `src-tauri/src/commands.rs#edit_apply`, `docs/specs/core/6-tauri-commands.md`
- Target resolution: `reviewed-mapping-report:CC-tauri-command-contract`; matched `editApply`, `EditRequest`, `edit_apply`.
- Resolution rationale: Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Test ownership:
  - `src-tauri/src/commands.rs#deserializes_camelcase_multiword_commands` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_add_captions_camelcase_and_maps_to_command` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_effect_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_media_library_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#every_edit_request_maps_to_exact_edit_command` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/api.commandContract.test.ts#frontend_command_names_match_invoke_handler` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/core/6-tauri-commands.md:1 under “6. Tauri command 表面(精确签名草案)” (heading), the source “## 6. Tauri command 表面(精确签名草案)” requires this exact behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts.
- Acceptance criteria: Source binding: docs/specs/core/6-tauri-commands.md:1; signal=heading; heading=6. Tauri command 表面(精确签名草案); candidate=## 6. Tauri command 表面(精确签名草案) Expected behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts. This closes only the promise expressed by “6. Tauri command 表面(精确签名草案)” in “6. Tauri command 表面(精确签名草案)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “6. Tauri command 表面(精确签名草案)” with the scenario below and register test:tools/completion-tests/doc-47dfcfcf41dd2784.test.mjs#completion_47dfcfcf41dd2784_tauri_exposes_typed_core_edit_commands_with_stab Initial state/input/event: start from the smallest valid fixture for “Tauri exposes typed core/edit commands with stable DTO and error contracts.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6. Tauri command 表面(精确签名草案)”. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Tauri exposes typed core/edit commands with stable DTO and error contracts.” once. Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-47dfcfcf41dd2784.test.mjs#completion_47dfcfcf41dd2784_tauri_exposes_typed_core_edit_commands_with_stab.

### requirement-5ca0aab886c28dde

- Kind: requirement
- Implementation slice: `implementation-slice-00bdb59d43b2ad0c`
- Candidate: `doc-b0fa71fc2443ba5a`
- Source citation: `docs/specs/core/6-tauri-commands.md:5`
- Exact files/symbols: `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `src-tauri/src/commands.rs#edit_apply`, `docs/specs/core/6-tauri-commands.md`
- Target resolution: `reviewed-mapping-report:CC-tauri-command-contract`; matched `editApply`, `EditRequest`, `edit_apply`.
- Resolution rationale: Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Test ownership:
  - `src-tauri/src/commands.rs#deserializes_camelcase_multiword_commands` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_add_captions_camelcase_and_maps_to_command` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_effect_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_media_library_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#every_edit_request_maps_to_exact_edit_command` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/api.commandContract.test.ts#frontend_command_names_match_invoke_handler` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/core/6-tauri-commands.md:5 under “6.1 命令清单” (heading), the source “### 6.1 命令清单” requires this exact behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts.
- Acceptance criteria: Source binding: docs/specs/core/6-tauri-commands.md:5; signal=heading; heading=6.1 命令清单; candidate=### 6.1 命令清单 Expected behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts. This closes only the promise expressed by “6.1 命令清单” in “6.1 命令清单”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “6.1 命令清单” with the scenario below and register test:tools/completion-tests/doc-b0fa71fc2443ba5a.test.mjs#completion_b0fa71fc2443ba5a_tauri_exposes_typed_core_edit_commands_with_stab Initial state/input/event: start from the smallest valid fixture for “Tauri exposes typed core/edit commands with stable DTO and error contracts.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6.1 命令清单”. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Tauri exposes typed core/edit commands with stable DTO and error contracts.” once. Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-b0fa71fc2443ba5a.test.mjs#completion_b0fa71fc2443ba5a_tauri_exposes_typed_core_edit_commands_with_stab.

### requirement-2c03433376733eb5

- Kind: requirement
- Implementation slice: `implementation-slice-00bdb59d43b2ad0c`
- Candidate: `doc-5ec52eb6e8513033`
- Source citation: `docs/specs/core/6-tauri-commands.md:49`
- Exact files/symbols: `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `src-tauri/src/commands.rs#edit_apply`, `docs/specs/core/6-tauri-commands.md`
- Target resolution: `reviewed-mapping-report:CC-tauri-command-contract`; matched `editApply`, `EditRequest`, `edit_apply`.
- Resolution rationale: Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Test ownership:
  - `src-tauri/src/commands.rs#deserializes_camelcase_multiword_commands` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_add_captions_camelcase_and_maps_to_command` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_effect_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_media_library_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#every_edit_request_maps_to_exact_edit_command` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/api.commandContract.test.ts#frontend_command_names_match_invoke_handler` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/core/6-tauri-commands.md:49 under “6.2 关键参数类型(对齐上游)” (heading), the source “### 6.2 关键参数类型(对齐上游)” requires this exact behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts.
- Acceptance criteria: Source binding: docs/specs/core/6-tauri-commands.md:49; signal=heading; heading=6.2 关键参数类型(对齐上游); candidate=### 6.2 关键参数类型(对齐上游) Expected behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts. This closes only the promise expressed by “6.2 关键参数类型(对齐上游)” in “6.2 关键参数类型(对齐上游)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “6.2 关键参数类型(对齐上游)” with the scenario below and register test:tools/completion-tests/doc-5ec52eb6e8513033.test.mjs#completion_5ec52eb6e8513033_tauri_exposes_typed_core_edit_commands_with_stab Initial state/input/event: start from the smallest valid fixture for “Tauri exposes typed core/edit commands with stable DTO and error contracts.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6.2 关键参数类型(对齐上游)”. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Tauri exposes typed core/edit commands with stable DTO and error contracts.” once. Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-5ec52eb6e8513033.test.mjs#completion_5ec52eb6e8513033_tauri_exposes_typed_core_edit_commands_with_stab.

### requirement-5558df9d0b133739

- Kind: requirement
- Implementation slice: `implementation-slice-00bdb59d43b2ad0c`
- Candidate: `doc-4cd2927d804dc4a3`
- Source citation: `docs/specs/core/6-tauri-commands.md:73`
- Exact files/symbols: `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `src-tauri/src/commands.rs#edit_apply`, `docs/specs/core/6-tauri-commands.md`
- Target resolution: `reviewed-mapping-report:CC-tauri-command-contract`; matched `editApply`, `EditRequest`, `edit_apply`.
- Resolution rationale: Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Test ownership:
  - `src-tauri/src/commands.rs#deserializes_camelcase_multiword_commands` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_add_captions_camelcase_and_maps_to_command` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_effect_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_media_library_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#every_edit_request_maps_to_exact_edit_command` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/api.commandContract.test.ts#frontend_command_names_match_invoke_handler` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/core/6-tauri-commands.md:73 under “6.3 错误约定” (heading), the source “### 6.3 错误约定” requires this exact behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts.
- Acceptance criteria: Source binding: docs/specs/core/6-tauri-commands.md:73; signal=heading; heading=6.3 错误约定; candidate=### 6.3 错误约定 Expected behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts. This closes only the promise expressed by “6.3 错误约定” in “6.3 错误约定”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “6.3 错误约定” with the scenario below and register test:tools/completion-tests/doc-4cd2927d804dc4a3.test.mjs#completion_4cd2927d804dc4a3_tauri_exposes_typed_core_edit_commands_with_stab Initial state/input/event: start from the smallest valid fixture for “Tauri exposes typed core/edit commands with stable DTO and error contracts.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6.3 错误约定”. Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Tauri exposes typed core/edit commands with stable DTO and error contracts.” once. Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-4cd2927d804dc4a3.test.mjs#completion_4cd2927d804dc4a3_tauri_exposes_typed_core_edit_commands_with_stab.

### requirement-c924246b4d2b7ff9

- Kind: requirement
- Implementation slice: `implementation-slice-00bdb59d43b2ad0c`
- Candidate: `doc-43312d5e9f613913`
- Source citation: `docs/specs/core/6-tauri-commands.md:84`
- Exact files/symbols: `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `src-tauri/src/commands.rs#edit_apply`, `docs/specs/core/6-tauri-commands.md`
- Target resolution: `reviewed-mapping-report:CC-tauri-command-contract`; matched `editApply`, `EditRequest`, `edit_apply`.
- Resolution rationale: Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Test ownership:
  - `src-tauri/src/commands.rs#deserializes_camelcase_multiword_commands` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_add_captions_camelcase_and_maps_to_command` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_effect_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_media_library_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#every_edit_request_maps_to_exact_edit_command` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/api.commandContract.test.ts#frontend_command_names_match_invoke_handler` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/core/6-tauri-commands.md:84 under “6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)” (heading), the source “### 6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)” requires this exact behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts.
- Acceptance criteria: Source binding: docs/specs/core/6-tauri-commands.md:84; signal=heading; heading=6.4 `edit_apply` 与 agent 工具的关系(避免重复定义); candidate=### 6.4 `edit_apply` 与 agent 工具的关系(避免重复定义) Expected behavior: Tauri exposes typed core/edit commands with stable DTO and error contracts. This closes only the promise expressed by “6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)” in “6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)” with the scenario below and register test:crates/opentake-agent/tests/completion_43312d5e9f613913.rs#completion_43312d5e9f613913_tauri_exposes_typed_core_edit_commands_with_stab Initial state/input/event: start from the smallest valid fixture for “Tauri exposes typed core/edit commands with stable DTO and error contracts.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)”. Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Tauri exposes typed core/edit commands with stable DTO and error contracts.” once. Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_43312d5e9f613913.rs#completion_43312d5e9f613913_tauri_exposes_typed_core_edit_commands_with_stab.

### requirement-224726c57b0ebb49

- Kind: requirement
- Implementation slice: `implementation-slice-9296b54263dc8038`
- Candidate: `doc-38a7e973e95ff30d`
- Source citation: `docs/specs/frontend/10-state.md:1`
- Exact files/symbols: `web/src/store/sync.ts#startSync`, `web/src/store/projectStore.ts#useProjectStore`, `web/src/store/uiStore.ts#useEditorUiStore`, `docs/specs/frontend/10-state.md`
- Target resolution: `reviewed-mapping-report:CC-authority-persistence-mixed`; matched `startSync`, `useProjectStore`, `useEditorUiStore`.
- Resolution rationale: Core mapping report CC-authority-persistence-mixed: these mixed records require composite acceptance across Rust authority projection and UI-only persistence.
- Test ownership:
  - `web/src/store/commandRouting.test.ts#rust_authority_and_ui_persistence_are_independently_owned` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Rust timeline state remains authoritative and all required UI-only persistence survives restart without stale project data.
- Acceptance criteria: Keep timeline/project domain data as a version-ordered read-only Rust projection and isolate selection/layout/dialog/hover state in the UI store. Persist only approved UI keys with schema version and project scoping; never restore stale timeline/media state from localStorage. Test out-of-order snapshots, concurrent commands, project switch, restart, migration/corrupt storage, and fallback isolation with authoritative refetch equality.

### requirement-d9d443c2efd6fafd

- Kind: requirement
- Implementation slice: `implementation-slice-9296b54263dc8038`
- Candidate: `doc-8c9bed704675867f`
- Source citation: `docs/specs/frontend/10-state.md:5`
- Exact files/symbols: `web/src/store/sync.ts#startSync`, `web/src/store/projectStore.ts#useProjectStore`, `web/src/store/uiStore.ts#useEditorUiStore`, `docs/specs/frontend/10-state.md`
- Target resolution: `reviewed-mapping-report:CC-authority-persistence-mixed`; matched `startSync`, `useProjectStore`, `useEditorUiStore`.
- Resolution rationale: Core mapping report CC-authority-persistence-mixed: these mixed records require composite acceptance across Rust authority projection and UI-only persistence.
- Test ownership:
  - `web/src/store/commandRouting.test.ts#rust_authority_and_ui_persistence_are_independently_owned` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Rust timeline state remains authoritative and all required UI-only persistence survives restart without stale project data.
- Acceptance criteria: Prevent production code from directly mutating mirrored timeline/tracks/clips/media/version outside snapshot application. Apply only snapshots whose version is not older than current and refetch authoritative TimelineDTO after every edit/undo/redo. Static-check write sites and test out-of-order events, two concurrent edits, failed command, undo/redo, and project switch without stale rollback.

### requirement-12a60880e3a58578

- Kind: requirement
- Implementation slice: `implementation-slice-9296b54263dc8038`
- Candidate: `doc-9cd688fa7c3b983c`
- Source citation: `docs/specs/frontend/10-state.md:96`
- Exact files/symbols: `web/src/store/sync.ts#startSync`, `web/src/store/projectStore.ts#useProjectStore`, `web/src/store/uiStore.ts#useEditorUiStore`, `docs/specs/frontend/10-state.md`
- Target resolution: `reviewed-mapping-report:CC-authority-persistence-mixed`; matched `startSync`, `useProjectStore`, `useEditorUiStore`.
- Resolution rationale: Core mapping report CC-authority-persistence-mixed: these mixed records require composite acceptance across Rust authority projection and UI-only persistence.
- Test ownership:
  - `web/src/store/commandRouting.test.ts#rust_authority_and_ui_persistence_are_independently_owned` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Rust timeline state remains authoritative and all required UI-only persistence survives restart without stale project data.
- Acceptance criteria: Persist only the specified layout, panel visibility/size, theme, zoom, and recent UI preferences with schema version and bounded values. Exclude timeline, clips, media, credentials, transient drag/playback/dialog state, and clear or migrate malformed/old records safely. Restart-test each approved key plus corrupt JSON, old schema, invalid bounds, project switch, logout, and secret scan.

### requirement-cd691581ab4342b3

- Kind: requirement
- Implementation slice: `implementation-slice-00bdb59d43b2ad0c`
- Candidate: `doc-adfd8e77f6674234`
- Source citation: `docs/specs/frontend/11-tauri.md:1`
- Exact files/symbols: `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `src-tauri/src/commands.rs#edit_apply`, `docs/specs/frontend/11-tauri.md`
- Target resolution: `reviewed-mapping-report:CC-tauri-command-contract`; matched `editApply`, `EditRequest`, `edit_apply`.
- Resolution rationale: Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Test ownership:
  - `src-tauri/src/commands.rs#deserializes_camelcase_multiword_commands` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_add_captions_camelcase_and_maps_to_command` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_effect_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_media_library_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#every_edit_request_maps_to_exact_edit_command` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/api.commandContract.test.ts#frontend_command_names_match_invoke_handler` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Every command listed by the frontend specification maps to a live Rust implementation with typed errors.
- Acceptance criteria: Keep one typed TypeScript wrapper and one registered Rust handler for every advertised command/event, with matching names, request/response fields, and errors. Remove or capability-gate generation/advanced-media commands whose backend remains unavailable; browser fallback must never report production success for them. Generate/inventory parity tests and run success, malformed request, typed error, cancellation, and event ordering cases across the Tauri boundary.

### requirement-29da2e6af9281076

- Kind: requirement
- Implementation slice: `implementation-slice-00bdb59d43b2ad0c`
- Candidate: `doc-96f7321f1d38c5ef`
- Source citation: `docs/specs/frontend/11-tauri.md:5`
- Exact files/symbols: `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `src-tauri/src/commands.rs#edit_apply`, `docs/specs/frontend/11-tauri.md`
- Target resolution: `reviewed-mapping-report:CC-tauri-command-contract`; matched `editApply`, `EditRequest`, `edit_apply`.
- Resolution rationale: Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.
- Test ownership:
  - `src-tauri/src/commands.rs#deserializes_camelcase_multiword_commands` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_add_captions_camelcase_and_maps_to_command` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_effect_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#deserializes_media_library_commands_and_maps_to_ops_variants` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/commands.rs#every_edit_request_maps_to_exact_edit_command` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/api.commandContract.test.ts#frontend_command_names_match_invoke_handler` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Every command listed by the frontend specification maps to a live Rust implementation with typed errors.
- Acceptance criteria: Map every listed edit/read/playback/project command to a live Rust handler and strict TS request/response type. Unknown fields, invalid frames/IDs/paths, unavailable capability, and cancellation must return typed errors without partial mutation. Table-drive invoke registration/schema parity plus success/failure/undo for each mutating command and packaged desktop smoke the command surface.

### requirement-dbce32105bffaabf

- Kind: requirement
- Implementation slice: `implementation-slice-dc651cd267aea077`
- Candidate: `doc-e216f163870c8652`
- Source citation: `docs/specs/frontend/13-implementation.md:44`
- Exact files/symbols: `src-tauri/src/lib.rs#forward_event`, `web/src/store/sync.ts#startSync`, `web/src/store/projectStore.ts#useProjectStore`, `docs/specs/frontend/13-implementation.md`
- Target resolution: `reviewed-mapping-report:CC-readonly-versioned-mirror`; matched `forward_event`, `startSync`, `useProjectStore`.
- Resolution rationale: Core mapping report CC-readonly-versioned-mirror: the versioned refresh path exists, but whole-store read-only projection ownership lacks a closed proof.
- Test ownership:
  - `web/src/store/sync.test.ts#does not let a late old snapshot replace a newer project` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/store/commandRouting.test.ts#project_store_has_no_timeline_mutator_and_refreshes_only_from_native_events` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Keep the frontend timeline mirror read-only and refresh it only from timeline_changed/get_timeline.
- Acceptance criteria: Implementation: Audit every timeline write, enforce read-only types/store boundaries, route browser fallback through equivalent command handling, and add mutation-detection plus event-race tests. Add request/response tests for every named success, boundary, rejection, validation, and secrecy rule; the affected Rust and TypeScript suites must pass. Exercise the production IPC, MCP, or browser entry point end to end and record the exact command payload, result, and test names before reclassification.

### requirement-538b644e2cb07926

- Kind: requirement
- Implementation slice: `implementation-slice-6d2ce8b116a3ccd9`
- Candidate: `doc-8037f1b0a9056497`
- Source citation: `docs/specs/frontend/13-implementation.md:45`
- Exact files/symbols: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`, `web/src/store/editActions.ts#buildMediaInsertPlan`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#EditRequest`, `docs/specs/frontend/13-implementation.md`
- Target resolution: `reviewed-mapping-report:CC-edit-gesture-parity`; matched `TimelineContainer`, `buildMediaInsertPlan`, `editApply`, `EditRequest`.
- Resolution rationale: Core mapping report CC-edit-gesture-parity: focused gesture tests exist, but no closed inventory proves every UI path emits the exact shared request.
- Test ownership:
  - `web/src/store/editActions.test.ts#forwards swapTracks for whole-track reordering` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/store/commandRouting.test.ts#every_edit_action_emits_exact_edit_request` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Map every editing gesture to the exact edit_apply command.
- Acceptance criteria: Implementation: Create a gesture-to-command contract matrix from spec 11.1, exercise every gesture and modifier, assert exact payload/no-op behavior, and verify one-step undo semantics. Add request/response tests for every named success, boundary, rejection, validation, and secrecy rule; the affected Rust and TypeScript suites must pass. Exercise the production IPC, MCP, or browser entry point end to end and record the exact command payload, result, and test names before reclassification.

### control-record-ba5f8f9b19f0fb1d

- Kind: control
- Implementation slice: `implementation-slice-251a11401ef969e1`
- Candidate: `control-3be71cae61006e08`
- Source citation: `web/src/components/toolbar/Toolbar.tsx:104:9`
- Exact files/symbols: `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#undo`, `web/src/lib/api.ts#undo`, `src-tauri/src/commands.rs#undo`, `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-3be71cae61006e08 undo the last edit` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: undo the last edit: edit.undo -> backend undo -> refresh mirror/canUndo/canRedo
- Acceptance criteria: Candidate: control-3be71cae61006e08. Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-3be71cae61006e08 undo the last edit. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=canUndo. Event: inputs=["event/prop handler: {() => edit.undo()}","click or native keyboard activation plus current owning state"]; handler={() => edit.undo()}. Exact call/state/backend: stateTransition=edit.undo -> backend undo -> refresh mirror/canUndo/canRedo; backendTrace=["web/src/components/toolbar/Toolbar.tsx:104::candidate handler -> {() => edit.undo()}","actual branch/state -> edit.undo -> backend undo -> refresh mirror/canUndo/canRedo","exact call/arguments -> edit.undo() -> api.undo() -> invoke('undo') exactly once when canUndo is true","web/src/store/editActions.ts::undo -> web/src/lib/api.ts::undo","web/src/lib/api.ts::undo -> invoke('undo')","src-tauri/src/commands.rs::undo -> opentake-core handle_undo","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#undo","code:web/src/lib/api.ts#undo","code:src-tauri/src/commands.rs#undo"]. Visible/accessibility/return path: success=undo the last edit: edit.undo -> backend undo -> refresh mirror/canUndo/canRedo; accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"toolbar.undo\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."]. Outcome matrix: {"success":"undo the last edit: edit.undo -> backend undo -> refresh mirror/canUndo/canRedo","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:104; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/toolbar/Toolbar.tsx:104; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {!canUndo}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.undo -> backend undo -> refresh mirror/canUndo/canRedo.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:104; the missing DOM test must prove whether it is surfaced or silent."}.

### control-record-a9840dea7abafb72

- Kind: control
- Implementation slice: `implementation-slice-a085393217648051`
- Candidate: `control-b001ac6b21c97ad0`
- Source citation: `web/src/components/toolbar/Toolbar.tsx:107:9`
- Exact files/symbols: `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#redo`, `web/src/lib/api.ts#redo`, `src-tauri/src/commands.rs#redo`, `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-b001ac6b21c97ad0 redo the last undone edit` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: redo the last undone edit: edit.redo -> backend redo -> refresh mirror/canUndo/canRedo
- Acceptance criteria: Candidate: control-b001ac6b21c97ad0. Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-b001ac6b21c97ad0 redo the last undone edit. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=canRedo. Event: inputs=["event/prop handler: {() => edit.redo()}","click or native keyboard activation plus current owning state"]; handler={() => edit.redo()}. Exact call/state/backend: stateTransition=edit.redo -> backend redo -> refresh mirror/canUndo/canRedo; backendTrace=["web/src/components/toolbar/Toolbar.tsx:107::candidate handler -> {() => edit.redo()}","actual branch/state -> edit.redo -> backend redo -> refresh mirror/canUndo/canRedo","exact call/arguments -> edit.redo() -> api.redo() -> invoke('redo') exactly once when canRedo is true","web/src/store/editActions.ts::redo -> web/src/lib/api.ts::redo","web/src/lib/api.ts::redo -> invoke('redo')","src-tauri/src/commands.rs::redo -> opentake-core handle_redo","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#redo","code:web/src/lib/api.ts#redo","code:src-tauri/src/commands.rs#redo"]. Visible/accessibility/return path: success=redo the last undone edit: edit.redo -> backend redo -> refresh mirror/canUndo/canRedo; accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"toolbar.redo\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."]. Outcome matrix: {"success":"redo the last undone edit: edit.redo -> backend redo -> refresh mirror/canUndo/canRedo","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:107; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/toolbar/Toolbar.tsx:107; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {!canRedo}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.redo -> backend redo -> refresh mirror/canUndo/canRedo.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:107; the missing DOM test must prove whether it is surfaced or silent."}.

### control-record-48e26828d85cd366

- Kind: control
- Implementation slice: `implementation-slice-dc2e1dbf0bbdec19`
- Candidate: `control-9d69468ce3479312`
- Source citation: `web/src/components/toolbar/Toolbar.tsx:116:9`
- Exact files/symbols: `web/src/components/toolbar/Toolbar.tsx`, `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-9d69468ce3479312 switch to Pointer tool` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: switch to Pointer tool: setToolMode('pointer')
- Acceptance criteria: Candidate: control-9d69468ce3479312. Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-9d69468ce3479312 switch to Pointer tool. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {() => setToolMode(\"pointer\")}","click or native keyboard activation plus current owning state"]; handler={() => setToolMode("pointer")}. Exact call/state/backend: stateTransition=setToolMode('pointer'); backendTrace=["web/src/components/toolbar/Toolbar.tsx:116::candidate handler -> {() => setToolMode(\"pointer\")}","actual branch/state -> setToolMode('pointer')","exact call -> setToolMode('pointer')","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/toolbar/Toolbar.tsx#Toolbar"]. Visible/accessibility/return path: success=switch to Pointer tool: setToolMode('pointer'); accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"toolbar.pointer\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."]. Outcome matrix: {"success":"switch to Pointer tool: setToolMode('pointer')","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

### control-record-f74d763ee91db31c

- Kind: control
- Implementation slice: `implementation-slice-dc2e1dbf0bbdec19`
- Candidate: `control-8105812f9d07bc93`
- Source citation: `web/src/components/toolbar/Toolbar.tsx:123:9`
- Exact files/symbols: `web/src/components/toolbar/Toolbar.tsx`, `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-8105812f9d07bc93 switch to Razor tool` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: switch to Razor tool: setToolMode('razor')
- Acceptance criteria: Candidate: control-8105812f9d07bc93. Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-8105812f9d07bc93 switch to Razor tool. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {() => setToolMode(\"razor\")}","click or native keyboard activation plus current owning state"]; handler={() => setToolMode("razor")}. Exact call/state/backend: stateTransition=setToolMode('razor'); backendTrace=["web/src/components/toolbar/Toolbar.tsx:123::candidate handler -> {() => setToolMode(\"razor\")}","actual branch/state -> setToolMode('razor')","exact call -> setToolMode('razor')","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/toolbar/Toolbar.tsx#Toolbar"]. Visible/accessibility/return path: success=switch to Razor tool: setToolMode('razor'); accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"toolbar.razor\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."]. Outcome matrix: {"success":"switch to Razor tool: setToolMode('razor')","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

### control-record-f44c6f98dd0013ee

- Kind: control
- Implementation slice: `implementation-slice-964535d9ff93a4e8`
- Candidate: `control-c96abe84259649a3`
- Source citation: `web/src/components/toolbar/Toolbar.tsx:136:9`
- Exact files/symbols: `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#splitAtPlayhead`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-c96abe84259649a3 split selected clips at playhead` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: split selected clips at playhead: edit.splitAtPlayhead
- Acceptance criteria: Candidate: control-c96abe84259649a3. Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-c96abe84259649a3 split selected clips at playhead. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {() => edit.splitAtPlayhead()}","click or native keyboard activation plus current owning state"]; handler={() => edit.splitAtPlayhead()}. Exact call/state/backend: stateTransition=edit.splitAtPlayhead; backendTrace=["web/src/components/toolbar/Toolbar.tsx:136::candidate handler -> {() => edit.splitAtPlayhead()}","actual branch/state -> edit.splitAtPlayhead","exact call/arguments -> splitAtPlayhead(): frame=Math.round(activeFrame); target selected clips or all clips strictly intersecting frame; for each target call editApply({type:'splitClip',clipId,atFrame:frame})","web/src/store/editActions.ts::splitAtPlayhead -> splitClip(id,frame) -> applyAndRefresh({type:'splitClip',clipId:id,atFrame:frame})","web/src/lib/api.ts::editApply -> invoke('edit_apply',{command})","src-tauri/src/commands.rs::edit_apply -> EditRequest::SplitClip -> crates/opentake-ops/src/command.rs","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#splitAtPlayhead","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"]. Visible/accessibility/return path: success=split selected clips at playhead: edit.splitAtPlayhead; accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"toolbar.split\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."]. Outcome matrix: {"success":"split selected clips at playhead: edit.splitAtPlayhead","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:136; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/toolbar/Toolbar.tsx:136; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.splitAtPlayhead.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:136; the missing DOM test must prove whether it is surfaced or silent."}.

### control-record-4338636c52466c65

- Kind: control
- Implementation slice: `implementation-slice-922fa77f774ce4f6`
- Candidate: `control-f38c30bc83d65d2e`
- Source citation: `web/src/components/toolbar/Toolbar.tsx:139:9`
- Exact files/symbols: `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#trimStartToPlayhead`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-f38c30bc83d65d2e trim selected clip starts to playhead` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: trim selected clip starts to playhead: edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips
- Acceptance criteria: Candidate: control-f38c30bc83d65d2e. Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-f38c30bc83d65d2e trim selected clip starts to playhead. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {() => edit.trimStartToPlayhead()}","click or native keyboard activation plus current owning state"]; handler={() => edit.trimStartToPlayhead()}. Exact call/state/backend: stateTransition=edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips; backendTrace=["web/src/components/toolbar/Toolbar.tsx:139::candidate handler -> {() => edit.trimStartToPlayhead()}","actual branch/state -> edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips","exact call/arguments -> trimStartToPlayhead(): frame=Math.round(activeFrame); clipsUnderPlayhead(); trimToPlayheadEdits(clips,frame,'left'); editApply({type:'trimClips',edits})","web/src/store/editActions.ts::trimStartToPlayhead -> trimClips(left edits) -> applyAndRefresh({type:'trimClips',edits})","web/src/lib/api.ts::editApply -> invoke('edit_apply',{command:{type:'trimClips',edits}})","src-tauri/src/commands.rs::edit_apply -> EditRequest::TrimClips -> crates/opentake-ops/src/command.rs","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#trimStartToPlayhead","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"]. Visible/accessibility/return path: success=trim selected clip starts to playhead: edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips; accessibility={"focus":"Custom GlyphButton focus behavior depends on its implementation","label":"t(\"toolbar.trimStart\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."]. Outcome matrix: {"success":"trim selected clip starts to playhead: edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:139; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/toolbar/Toolbar.tsx:139; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.trimStartToPlayhead computes left-edge trim edits for selected/intersecting clips.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:139; the missing DOM test must prove whether it is surfaced or silent."}.

### control-record-db567c80607eddf4

- Kind: control
- Implementation slice: `implementation-slice-29b311273420852d`
- Candidate: `control-eeff92d3d70361d9`
- Source citation: `web/src/components/toolbar/Toolbar.tsx:144:9`
- Exact files/symbols: `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#trimEndToPlayhead`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-eeff92d3d70361d9 trim selected clip ends to playhead` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: trim selected clip ends to playhead: edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips
- Acceptance criteria: Candidate: control-eeff92d3d70361d9. Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-eeff92d3d70361d9 trim selected clip ends to playhead. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {() => edit.trimEndToPlayhead()}","click or native keyboard activation plus current owning state"]; handler={() => edit.trimEndToPlayhead()}. Exact call/state/backend: stateTransition=edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips; backendTrace=["web/src/components/toolbar/Toolbar.tsx:144::candidate handler -> {() => edit.trimEndToPlayhead()}","actual branch/state -> edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips","exact call/arguments -> trimEndToPlayhead(): frame=Math.round(activeFrame); clipsUnderPlayhead(); trimToPlayheadEdits(clips,frame,'right'); editApply({type:'trimClips',edits})","web/src/store/editActions.ts::trimEndToPlayhead -> trimClips(right edits) -> applyAndRefresh({type:'trimClips',edits})","web/src/lib/api.ts::editApply -> invoke('edit_apply',{command:{type:'trimClips',edits}})","src-tauri/src/commands.rs::edit_apply -> EditRequest::TrimClips -> crates/opentake-ops/src/command.rs","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#trimEndToPlayhead","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"]. Visible/accessibility/return path: success=trim selected clip ends to playhead: edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips; accessibility={"focus":"Custom GlyphButton focus behavior depends on its implementation","label":"t(\"toolbar.trimEnd\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."]. Outcome matrix: {"success":"trim selected clip ends to playhead: edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:144; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/toolbar/Toolbar.tsx:144; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.trimEndToPlayhead computes right-edge trim edits for selected/intersecting clips.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:144; the missing DOM test must prove whether it is surfaced or silent."}.

### control-record-8125c2e3848a32d1

- Kind: control
- Implementation slice: `implementation-slice-10afd9e138d27853`
- Candidate: `control-c6a658045b9e1d6c`
- Source citation: `web/src/components/toolbar/Toolbar.tsx:150:7`
- Exact files/symbols: `web/src/components/toolbar/Toolbar.tsx`, `web/src/store/editActions.ts#addTextClip`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-c6a658045b9e1d6c add a text clip` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: add a text clip: edit.addTextClip inserts/selects a new top-track text clip
- Acceptance criteria: Candidate: control-c6a658045b9e1d6c. Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-c6a658045b9e1d6c add a text clip. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {() => edit.addTextClip()}","click or native keyboard activation plus current owning state"]; handler={() => edit.addTextClip()}. Exact call/state/backend: stateTransition=edit.addTextClip inserts/selects a new top-track text clip; backendTrace=["web/src/components/toolbar/Toolbar.tsx:150::candidate handler -> {() => edit.addTextClip()}","actual branch/state -> edit.addTextClip inserts/selects a new top-track text clip","exact call/arguments -> addTextClip(): build one TextAutoTrackEntryReq at activeFrame with duration max(1,round(5*fps)), empty content/default style+transform; editApply({type:'addTextsAutoTrack',entries:[entry]})","web/src/store/editActions.ts::addTextClip -> applyAndRefresh({type:'addTextsAutoTrack',entries:[entry]}) -> optional forceRefresh/select affected ids","web/src/lib/api.ts::editApply -> invoke('edit_apply',{command:{type:'addTextsAutoTrack',entries:[entry]}})","src-tauri/src/commands.rs::edit_apply -> EditRequest::AddTextsAutoTrack -> crates/opentake-ops/src/command.rs","code:web/src/components/toolbar/Toolbar.tsx#Toolbar","code:web/src/store/editActions.ts#addTextClip","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"]. Visible/accessibility/return path: success=add a text clip: edit.addTextClip inserts/selects a new top-track text clip; accessibility={"focus":"Custom GlyphButton focus behavior depends on its implementation","label":"t(\"toolbar.addText\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."]. Outcome matrix: {"success":"add a text clip: edit.addTextClip inserts/selects a new top-track text clip","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/toolbar/Toolbar.tsx:150; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in edit.addTextClip inserts/selects a new top-track text clip.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/toolbar/Toolbar.tsx:150; the missing DOM test must prove whether it is surfaced or silent."}.

### control-record-06c8bde125e1a97d

- Kind: control
- Implementation slice: `implementation-slice-dc2e1dbf0bbdec19`
- Candidate: `control-582e8fdf1d3d9e7e`
- Source citation: `web/src/components/toolbar/Toolbar.tsx:159:9`
- Exact files/symbols: `web/src/components/toolbar/Toolbar.tsx`, `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/toolbar/Toolbar.interaction.test.tsx#control-582e8fdf1d3d9e7e change timeline zoom` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: change timeline zoom: logarithmic slider -> setZoomScale
- Acceptance criteria: Candidate: control-582e8fdf1d3d9e7e. Test: web/src/components/toolbar/Toolbar.interaction.test.tsx#control-582e8fdf1d3d9e7e change timeline zoom. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {onSlider}","current control value and deterministic replacement value"]; handler={onSlider}. Exact call/state/backend: stateTransition=logarithmic slider -> setZoomScale; backendTrace=["web/src/components/toolbar/Toolbar.tsx:159::candidate handler -> {onSlider}","actual branch/state -> logarithmic slider -> setZoomScale","exact call -> logarithmic slider -> setZoomScale","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/toolbar/Toolbar.tsx#Toolbar"]. Visible/accessibility/return path: success=change timeline zoom: logarithmic slider -> setZoomScale; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"toolbar.zoom\")","shortcut":"None declared on this control"}; returnPath=["Focus remains on the toolbar control; edit commands update the editor mirror/selection."]. Outcome matrix: {"success":"change timeline zoom: logarithmic slider -> setZoomScale","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.
