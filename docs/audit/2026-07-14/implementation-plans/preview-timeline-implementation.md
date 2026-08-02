# Preview Timeline Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 96 verified incomplete records in the `preview-timeline` gap group.

**Architecture:** Implement 30 primary evidence-bound slices and reference 0 shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.

---

### Task 1: timeline-scale-performance (implementation-slice-f52ff83824ba0379)

**Covered records:**
- `requirement-9f4b49115b7e6a5b` (requirement)

**Files:**
- Modify: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`
- Modify: `web/src/components/timeline/timelineCanvas.ts#paintTimeline`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `web/src/components/timeline/TimelineContainer.scale.test.tsx#fifty_track_edit_seek_play_save_budget`

**Candidate-bound contracts:**

#### requirement-9f4b49115b7e6a5b

- Candidate/source: `doc-f9f8b45c54e52979` at `docs/architecture/CAPCUT-GAP.md:9` (requirement)
- Expected behavior: Large 50-track projects remain responsive during edit, seek, playback, and save.
- Resolution: `reviewed-mapping-report:timeline-scale-performance` — The timeline surface exists and owns rendering, but there is no trustworthy 50-track edit, seek, playback, and save performance budget.
- Exact acceptance contract:
  - Load a deterministic fixture containing at least 50 tracks and 1,000 clips without dropping or reordering clips.
  - Perform select, move, trim, seek, undo, save, and reopen operations while preserving exact frame positions and track ownership.
  - Add a repeatable benchmark that records p95 interaction latency, playback underruns, peak memory, and save/reopen equality; set and enforce the release thresholds in CI.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TimelineContainer.scale.test.tsx#fifty_track_edit_seek_play_save_budget` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.scale.test.tsx -t "fifty_track_edit_seek_play_save_budget"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`, `web/src/components/timeline/timelineCanvas.ts#paintTimeline`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.scale.test.tsx -t "fifty_track_edit_seek_play_save_budget"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 2: compound-clips (implementation-slice-0ef5268789b6e13f)

**Covered records:**
- `requirement-b49e0f5ed8c2415c` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/clip.rs#CompoundClip`
- Modify: `crates/opentake-render/src/plan/types.rs#RenderClip::Compound`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-project/tests/compound_roundtrip.rs#compound_clip_roundtrips_nested_timeline`
- Test (reviewed-planned): `crates/opentake-render/tests/compound_render.rs#compound_clip_preview_export_frames_match`

**Candidate-bound contracts:**

#### requirement-b49e0f5ed8c2415c

- Candidate/source: `doc-e33b5d6fcfa4c56c` at `docs/architecture/CAPCUT-GAP.md:15` (requirement)
- Expected behavior: Nested compound clips preserve timing, media references, rendering, and undo.
- Resolution: `reviewed-mapping-report:compound-clips` — No tracked compound model, command, or render owner was found; ordinary Clip and build_render_plan are not evidence of nested compound semantics.
- Exact acceptance contract:
  - Add a persisted compound-clip model that references a nested timeline without flattening source clips.
  - Create, open, trim, move, duplicate, and dissolve a two-level nested compound through undoable shared commands and the timeline UI.
  - Prove save/reopen equality plus preview/export frame parity at the compound in/out boundaries and fail clearly on recursive nesting cycles.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-project/tests/compound_roundtrip.rs#compound_clip_roundtrips_nested_timeline` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-render/tests/compound_render.rs#compound_clip_preview_export_frames_match` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-project --test compound_roundtrip compound_clip_roundtrips_nested_timeline -- --exact`
  - Run: `cargo test -p opentake-render --test compound_render compound_clip_preview_export_frames_match -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/clip.rs#CompoundClip`, `crates/opentake-render/src/plan/types.rs#RenderClip::Compound`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-project --test compound_roundtrip compound_clip_roundtrips_nested_timeline -- --exact`
  - Run: `cargo test -p opentake-render --test compound_render compound_clip_preview_export_frames_match -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Completion evidence (2026-07-31):
`runtime-artifacts/automated/nested-timeline-compound-real-device-2026-07-31.md`.
The exact owning tests, full Rust/Web gates, packaged create/open/trim/move/copy
and paste flow, save/reopen persistence, paused/continuous preview, and retained
export artifacts all pass; deterministic graph validation covers recursive
cycle rejection.

### Task 3: multicam (implementation-slice-5902337034fd8e89)

**Covered records:**
- `requirement-d179f69761518eaa` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/clip.rs#MulticamClip`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SwitchMulticamAngle`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-domain/tests/multicam.rs#sync_and_non_destructive_angle_switch`

**Candidate-bound contracts:**

#### requirement-d179f69761518eaa

- Candidate/source: `doc-181430c7c345f95e` at `docs/architecture/CAPCUT-GAP.md:21` (requirement)
- Expected behavior: Multicam sources can be synchronized and switched non-destructively.
- Resolution: `reviewed-mapping-report:multicam` — No tracked multicam source, synchronization, or non-destructive angle-switch identifier was found.
- Exact acceptance contract:
  - Represent synchronized camera angles and the selected-angle cuts in the domain/project schema.
  - Auto-align a three-camera fixture to its reference audio within one frame, expose angle switching in the timeline, and make sync/switch edits undoable.
  - Persist/reopen the multicam edit and compare preview/export audio and selected frames at every switch boundary.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-domain/tests/multicam.rs#sync_and_non_destructive_angle_switch` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-domain --test multicam sync_and_non_destructive_angle_switch -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/clip.rs#MulticamClip`, `crates/opentake-ops/src/command.rs#EditCommand::SwitchMulticamAngle`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-domain --test multicam sync_and_non_destructive_angle_switch -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 4: PT-clip-speed-end-to-end (implementation-slice-bd9d72148c0e3ec9)

**Covered records:**
- `requirement-fec364601c09f3ce` (requirement)

**Files:**
- Modify: `web/src/store/editActions.ts#setClipProperties`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetClipProperties`
- Modify: `crates/opentake-render/src/plan/build.rs#source_frame_index`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (existing-owned): `web/src/components/timeline/clipRenderer.test.ts#speed changes the consumed source span`
- Test (existing-owned): `crates/opentake-render/src/plan/tests.rs#source_frame_video_with_trim_and_speed`

**Candidate-bound contracts:**

#### requirement-fec364601c09f3ce

- Candidate/source: `doc-30951d7266fce2a1` at `docs/architecture/CAPCUT-GAP.md:27` (requirement)
- Expected behavior: At docs/architecture/CAPCUT-GAP.md:27 under “基础线性变速 — `has` · 难度 low · 优先级 p1” (heading), the source “### 基础线性变速 — `has` · 难度 low · 优先级 p1” requires this exact behavior: Clip speed is represented, edited, previewed, and rendered.
- Resolution: `validated-ledger-evidence:PT-clip-speed-end-to-end` — Speed is represented, edited via shared commands and consumed by render source-frame mapping.
- Exact acceptance contract:
  - Source binding: docs/architecture/CAPCUT-GAP.md:27; signal=heading; heading=基础线性变速 — `has` · 难度 low · 优先级 p1; candidate=### 基础线性变速 — `has` · 难度 low · 优先级 p1
  - Expected behavior: Clip speed is represented, edited, previewed, and rendered. This closes only the promise expressed by “基础线性变速 — `has` · 难度 low · 优先级 p1” in “基础线性变速 — `has` · 难度 low · 优先级 p1”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “基础线性变速 — `has` · 难度 low · 优先级 p1” with the scenario below and register test:crates/opentake-render/tests/completion_30951d7266fce2a1.rs#completion_30951d7266fce2a1_clip_speed_is_represented_edited_previewed_and_r
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “基础线性变速 — `has` · 难度 low · 优先级 p1”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Clip speed is represented, edited, previewed, and rendered.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_30951d7266fce2a1.rs#completion_30951d7266fce2a1_clip_speed_is_represented_edited_previewed_and_r.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/clipRenderer.test.ts#speed changes the consumed source span` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/src/plan/tests.rs#source_frame_video_with_trim_and_speed` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/clipRenderer.test.ts -t "speed changes the consumed source span"`
  - Run: `cargo test -p opentake-render source_frame_video_with_trim_and_speed`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/store/editActions.ts#setClipProperties`, `crates/opentake-ops/src/command.rs#EditCommand::SetClipProperties`, `crates/opentake-render/src/plan/build.rs#source_frame_index`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/clipRenderer.test.ts -t "speed changes the consumed source span"`
  - Run: `cargo test -p opentake-render source_frame_video_with_trim_and_speed`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 5: timeline-snap-geometry + PT-timeline-geometry + PT-snap-hysteresis-acceptance (implementation-slice-d820d4eefccf6d46)

**Covered records:**
- `requirement-c8564305ae9fa4bb` (requirement)
- `requirement-e8f0a3053bf5ae40` (requirement)
- `requirement-07854d86720411f5` (requirement)
- `requirement-d35fe204b3828ff1` (requirement)
- `requirement-38807e6a1df065a0` (requirement)
- `requirement-daa6ec4af3aeb4a9` (requirement)
- `requirement-824a12344f992e03` (requirement)
- `requirement-e471db5faf7f4480` (requirement)

**Files:**
- Modify: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`
- Modify: `web/src/lib/geometry.ts#clipRect`
- Modify: `web/src/lib/geometry.ts#dropTargetAt`
- Modify: `web/src/lib/geometry.ts#frameAt`
- Modify: `web/src/lib/geometry.ts#trackY`
- Modify: `web/src/lib/snap.ts#collectTargets`
- Modify: `web/src/lib/snap.ts#findSnap`
- Modify: `web/src/lib/snap.ts#findSnapDelta`
- Modify: `docs/architecture/FULL_PROJECT_SCAN_REPORT.md`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Modify: `docs/specs/frontend/5-timeline.md`
- Test (existing-owned): `web/src/components/timeline/TimelineContainer.test.ts#collectMoveSnapTargets`
- Test (existing-owned): `web/src/lib/geometry.test.ts#dropTargetAt boundaries`
- Test (existing-owned): `web/src/lib/geometry.test.ts#matches SPEC §5.2: x=frame*ppf, y=trackY+2, width=dur*ppf, height=trackH-4`
- Test (existing-owned): `web/src/lib/geometry.test.ts#matches upstream dropTargetAt boundary behavior`
- Test (existing-owned): `web/src/lib/snap.test.ts#snapFrameToEdge`
- Test (existing-owned): `web/src/lib/snap.test.ts#snaps to the nearest clip edge within threshold`
- Test (reviewed-planned): `web/src/lib/snap.test.ts#sticky_multi_probe_and_playhead_multiplier_matrix`
- Test (reviewed-planned): `web/src/lib/snap.test.ts#sticky_multi_target_and_both_1_5x_thresholds`

**Candidate-bound contracts:**

#### requirement-c8564305ae9fa4bb

- Candidate/source: `doc-2b2f7d97f5c37463` at `docs/architecture/FULL_PROJECT_SCAN_REPORT.md:62` (requirement)
- Expected behavior: Use scale-correct snap tolerances and an upstream-equivalent cross-track insertion threshold.
- Resolution: `reviewed-mapping-report:timeline-snap-geometry` — The report slice already combines geometry targets, snap helpers and TimelineContainer integration; the two ledger slices partition that same acceptance boundary, so one unified capability prevents duplicate geometry/snap ownership.
- Exact acceptance contract:
  - Implementation: Define the pointer/device scale contract, make every snap threshold operate in CSS pixels under zoom/device scale, implement the cross-track insertion-zone threshold, and add high-DPI plus boundary interaction tests.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-e8f0a3053bf5ae40

- Candidate/source: `doc-0dc52b4024587b37` at `docs/architecture/FULL_PROJECT_SCAN_REPORT.md:151` (requirement)
- Expected behavior: Match upstream snap and cross-track insertion thresholds.
- Resolution: `reviewed-mapping-report:timeline-snap-geometry` — The report slice already combines geometry targets, snap helpers and TimelineContainer integration; the two ledger slices partition that same acceptance boundary, so one unified capability prevents duplicate geometry/snap ownership.
- Exact acceptance contract:
  - Implementation: Implement and test the remaining high-DPI and cross-track insertion-threshold behavior at exact boundary values.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-07854d86720411f5

- Candidate/source: `doc-8fb96c6bbd6743ab` at `docs/modules/web/SPEC.md:1282` (requirement)
- Expected behavior: Match every geometry helper named by the spec, including insertion-line placement and hit testing outside track content.
- Resolution: `validated-ledger-evidence:PT-timeline-geometry` — The report slice already combines geometry targets, snap helpers and TimelineContainer integration; the two ledger slices partition that same acceptance boundary, so one unified capability prevents duplicate geometry/snap ownership.
- Exact acceptance contract:
  - Implement or explicitly reconcile the missing insertionLineY helper and pin its upstream formula.
  - Correct trackAt so ruler/drop-zone coordinates are not silently treated as track 0 unless the upstream source proves that behavior.
  - Add table-driven upstream vectors for clipRect, frameAt, xForFrame, trackY, dropTargetAt, insertionLineY, custom heights, and all boundaries.

#### requirement-d35fe204b3828ff1

- Candidate/source: `doc-55d07a36fed45f49` at `docs/modules/web/SPEC.md:1284` (requirement)
- Expected behavior: Match upstream snap selection, sticky hysteresis, and the larger playhead threshold without losing the held target kind.
- Resolution: `validated-ledger-evidence:PT-snap-hysteresis-acceptance` — The report slice already combines geometry targets, snap helpers and TimelineContainer integration; the two ledger slices partition that same acceptance boundary, so one unified capability prevents duplicate geometry/snap ownership.
- Exact acceptance contract:
  - Make playhead tie priority explicit and independent of target iteration order.
  - Preserve the held snap target kind through sticky hysteresis; do not always return clipEdge for a held playhead.
  - Add direct findSnap/findSnapDelta tests for clip threshold, playhead 1.5x threshold, exact ties in both target orders, sticky 1.5x release, multi-probe ownership, zero/invalid zoom, and target disappearance.

#### requirement-38807e6a1df065a0

- Candidate/source: `doc-3f4ffcb6391ffbee` at `docs/specs/frontend/13-implementation.md:39` (requirement)
- Expected behavior: Match all upstream geometry pure functions.
- Resolution: `reviewed-mapping-report:timeline-snap-geometry` — The report slice already combines geometry targets, snap helpers and TimelineContainer integration; the two ledger slices partition that same acceptance boundary, so one unified capability prevents duplicate geometry/snap ownership.
- Exact acceptance contract:
  - Implementation: Add a complete upstream vector corpus for clipRect/frameAt/xForFrame/trackY/dropTargetAt/insertionLineY including boundaries and high-DPI cases; fix any deviation.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-daa6ec4af3aeb4a9

- Candidate/source: `doc-f0e04ccd5d84f1bf` at `docs/specs/frontend/13-implementation.md:41` (requirement)
- Expected behavior: Match sticky multi-target snapping including both 1.5x thresholds.
- Resolution: `reviewed-mapping-report:timeline-snap-geometry` — The report slice already combines geometry targets, snap helpers and TimelineContainer integration; the two ledger slices partition that same acceptance boundary, so one unified capability prevents duplicate geometry/snap ownership.
- Exact acceptance contract:
  - Implementation: Add exact vector and pointer tests for acquisition, sticky release, playhead multiplier, multi-probe tie-breaking and haptic re-arm at threshold boundaries.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-824a12344f992e03

- Candidate/source: `doc-3765b9e3cd0a11c5` at `docs/specs/frontend/5-timeline.md:20` (requirement)
- Expected behavior: Complete the exact 5.2 几何（必须逐字照搬，`TimelineGeometry.swift`） behavior from the upstream interaction specification.
- Resolution: `validated-ledger-evidence:PT-timeline-geometry` — The report slice already combines geometry targets, snap helpers and TimelineContainer integration; the two ledger slices partition that same acceptance boundary, so one unified capability prevents duplicate geometry/snap ownership.
- Exact acceptance contract:
  - Use one frame↔pixel transform for ruler, clips, playhead, hit testing, drag, trim, and overlays at minimum/default/maximum zoom.
  - Clamp scroll/zoom and convert negative/out-of-range pointer coordinates without NaN, drift, or off-by-one frame changes.
  - Add golden coordinate cases at frames 0, 1, timeline end, half-pixel boundaries, and DPR 1/2; every round trip must differ by at most one frame.

#### requirement-e471db5faf7f4480

- Candidate/source: `doc-471be7874a190339` at `docs/specs/frontend/5-timeline.md:104` (requirement)
- Expected behavior: At docs/specs/frontend/5-timeline.md:104 under “5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）” (heading), the source “### 5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）” requires this exact behavior: The named timeline surface is implemented with focused geometry/interaction tests.
- Resolution: `reviewed-mapping-report:timeline-snap-geometry` — The report slice already combines geometry targets, snap helpers and TimelineContainer integration; the two ledger slices partition that same acceptance boundary, so one unified capability prevents duplicate geometry/snap ownership.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/5-timeline.md:104; signal=heading; heading=5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）; candidate=### 5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）
  - Expected behavior: The named timeline surface is implemented with focused geometry/interaction tests. This closes only the promise expressed by “5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）” in “5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）” with the scenario below and register test:web/src/__tests__/completion/doc-471be7874a190339.test.ts#completion_471be7874a190339_the_named_timeline_surface_is_implemented_with_f
  - Initial state/input/event: render the exact “5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The named timeline surface is implemented with focused geometry/interaction tests.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-471be7874a190339.test.ts#completion_471be7874a190339_the_named_timeline_surface_is_implemented_with_f.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TimelineContainer.test.ts#collectMoveSnapTargets` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/geometry.test.ts#dropTargetAt boundaries` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/geometry.test.ts#matches SPEC §5.2: x=frame*ppf, y=trackY+2, width=dur*ppf, height=trackH-4` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/geometry.test.ts#matches upstream dropTargetAt boundary behavior` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/snap.test.ts#snapFrameToEdge` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/snap.test.ts#snaps to the nearest clip edge within threshold` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/lib/snap.test.ts#sticky_multi_probe_and_playhead_multiplier_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/lib/snap.test.ts#sticky_multi_target_and_both_1_5x_thresholds` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.test.ts -t "collectMoveSnapTargets"`
  - Run: `pnpm -C web test -- --run src/lib/geometry.test.ts -t "dropTargetAt boundaries"`
  - Run: `pnpm -C web test -- --run src/lib/geometry.test.ts -t "matches SPEC §5.2: x=frame*ppf, y=trackY+2, width=dur*ppf, height=trackH-4"`
  - Run: `pnpm -C web test -- --run src/lib/geometry.test.ts -t "matches upstream dropTargetAt boundary behavior"`
  - Run: `pnpm -C web test -- --run src/lib/snap.test.ts -t "snapFrameToEdge"`
  - Run: `pnpm -C web test -- --run src/lib/snap.test.ts -t "snaps to the nearest clip edge within threshold"`
  - Run: `pnpm -C web test -- --run src/lib/snap.test.ts -t "sticky_multi_probe_and_playhead_multiplier_matrix"`
  - Run: `pnpm -C web test -- --run src/lib/snap.test.ts -t "sticky_multi_target_and_both_1_5x_thresholds"`

  Expected: FAIL because one or more of the 8 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`, `web/src/lib/geometry.ts#clipRect`, `web/src/lib/geometry.ts#dropTargetAt`, `web/src/lib/geometry.ts#frameAt`, `web/src/lib/geometry.ts#trackY`, `web/src/lib/snap.ts#collectTargets`, `web/src/lib/snap.ts#findSnap`, `web/src/lib/snap.ts#findSnapDelta`, `docs/architecture/FULL_PROJECT_SCAN_REPORT.md`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/13-implementation.md`, `docs/specs/frontend/5-timeline.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.test.ts -t "collectMoveSnapTargets"`
  - Run: `pnpm -C web test -- --run src/lib/geometry.test.ts -t "dropTargetAt boundaries"`
  - Run: `pnpm -C web test -- --run src/lib/geometry.test.ts -t "matches SPEC §5.2: x=frame*ppf, y=trackY+2, width=dur*ppf, height=trackH-4"`
  - Run: `pnpm -C web test -- --run src/lib/geometry.test.ts -t "matches upstream dropTargetAt boundary behavior"`
  - Run: `pnpm -C web test -- --run src/lib/snap.test.ts -t "snapFrameToEdge"`
  - Run: `pnpm -C web test -- --run src/lib/snap.test.ts -t "snaps to the nearest clip edge within threshold"`
  - Run: `pnpm -C web test -- --run src/lib/snap.test.ts -t "sticky_multi_probe_and_playhead_multiplier_matrix"`
  - Run: `pnpm -C web test -- --run src/lib/snap.test.ts -t "sticky_multi_target_and_both_1_5x_thresholds"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 6: timeline-toolbar-ruler-visuals + PT-timeline-visual-surface (implementation-slice-e31d299528ad4d0c)

**Covered records:**
- `requirement-d82235254f1ad987` (requirement)
- `requirement-c83bfe0b365d0d7f` (requirement)
- `requirement-65354598d0059701` (requirement)
- `requirement-fa8b90c9589d4792` (requirement)
- `requirement-a8a43bc740569dc6` (requirement)
- `requirement-22535d21627595e1` (requirement)
- `requirement-a68911b741563de7` (requirement)
- `requirement-923f74b3f2c06d84` (requirement)
- `requirement-32c4ef714d183571` (requirement)
- `requirement-1ec5bd08013418ac` (requirement)
- `requirement-6cdaf1b0bc759131` (requirement)
- `requirement-7f8b415cc15cc3a6` (requirement)
- `requirement-6f3d4e423997080c` (requirement)
- `requirement-1f8eaadf44b47e85` (requirement)
- `requirement-81f8ffc84e4f7032` (requirement)
- `requirement-01ab68dad9bef233` (requirement)
- `requirement-9e5580044b5f8307` (requirement)

**Files:**
- Modify: `web/src/components/timeline/Playhead.tsx#Playhead`
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn`
- Modify: `web/src/components/timeline/clipRenderer.ts#drawClip`
- Modify: `web/src/components/timeline/rulerCanvas.ts#paintRuler`
- Modify: `web/src/components/timeline/timelineCanvas.ts#paintTimeline`
- Modify: `web/src/components/toolbar/Toolbar.tsx#Toolbar`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Modify: `docs/specs/frontend/4-toolbar.md`
- Modify: `docs/specs/frontend/5-timeline.md`
- Test (reviewed-planned): `web/src/components/timeline/TimelineParity.test.tsx#all_named_canvas_affordances`
- Test (reviewed-planned): `web/src/components/timeline/TimelineParity.test.tsx#toolbar_ruler_header_clip_playhead_overlay_visual_matrix`
- Test (existing-owned): `web/src/components/timeline/clipRenderer.test.ts#drawClip fade knees`
- Test (existing-owned): `web/src/components/timeline/clipRenderer.test.ts#paints the systemRed wash + border when missing`
- Test (existing-owned): `web/src/components/timeline/timelineOverlays.test.ts#marked-range overlay (content canvas)`
- Test (existing-owned): `web/src/components/timeline/timelineOverlays.test.ts#marked-range overlay (ruler canvas)`
- Test (existing-owned): `web/src/components/timeline/timelineOverlays.test.ts#paints a dashed box on the gap's track`
- Test (reviewed-planned): `web/src/components/toolbar/Toolbar.test.tsx#upstream_group_order_and_zoom_mapping`

**Candidate-bound contracts:**

#### requirement-d82235254f1ad987

- Candidate/source: `doc-d0be4fc00ea996e5` at `docs/architecture/MODULE-PORT-MAP.md:240` (requirement)
- Expected behavior: Render every listed timeline canvas affordance with upstream-equivalent geometry and state.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Implementation: Add deterministic render/hit-test coverage for every listed background, separator, clip, thumbnail, waveform, envelope/fade/keyframe, offset, missing, and generating affordance; complete missing implementations and run visual comparison fixtures.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-c83bfe0b365d0d7f

- Candidate/source: `doc-ceda19e2165badd7` at `docs/architecture/ROADMAP.md:50` (requirement)
- Expected behavior: Complete the exhaustive React 1:1 interaction and visual acceptance checklist.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Close every unchecked row in docs/specs/frontend/2-layout.md through 9-interactions.md with a linked implementation and test.
  - Route all timeline/project mutations through typed edit actions and preserve keyboard, focus, drag, trim, menu, Inspector, MediaPanel, and Preview behavior.
  - Run unit/component tests plus fixed-size visual and packaged-desktop interaction smoke suites with zero unchecked acceptance rows.

#### requirement-65354598d0059701

- Candidate/source: `doc-2e8dd014e5d572a8` at `docs/modules/web/SPEC.md:1265` (requirement)
- Expected behavior: Match timeline toolbar icons, separators, and logarithmic zoom slider appearance and behavior.
- Resolution: `validated-ledger-evidence:PT-timeline-visual-surface` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Create a pinned upstream/current screenshot-state matrix at the same viewport, scale, locale, theme, and fixture.
  - Add deterministic component/browser assertions for normal, hover, focus, disabled, selected, empty, error, and compact states.
  - Record reviewed pixel/geometry evidence for every item named by the checklist, with no unchecked item remaining.

#### requirement-fa8b90c9589d4792

- Candidate/source: `doc-672861b902fd730f` at `docs/modules/web/SPEC.md:1266` (requirement)
- Expected behavior: Match every listed ruler, track-header, clip, playhead, snap, marquee, insertion, and razor visual state to upstream.
- Resolution: `validated-ledger-evidence:PT-timeline-visual-surface` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Create a pinned upstream/current screenshot-state matrix at the same viewport, scale, locale, theme, and fixture.
  - Add deterministic component/browser assertions for normal, hover, focus, disabled, selected, empty, error, and compact states.
  - Record reviewed pixel/geometry evidence for every item named by the checklist, with no unchecked item remaining.

#### requirement-a8a43bc740569dc6

- Candidate/source: `doc-ac0019a3d0fe1550` at `docs/specs/frontend/13-implementation.md:22` (requirement)
- Expected behavior: Match Toolbar icons, separators, and logarithmic zoom slider.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Implementation: Reconcile each toolbar control against the upstream table, implement logarithmic slider mapping, and add interaction plus visual tests for every state.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-22535d21627595e1

- Candidate/source: `doc-bd66cc1d20a6f674` at `docs/specs/frontend/13-implementation.md:23` (requirement)
- Expected behavior: Match every listed timeline ruler/track/clip/playhead/snap/marquee/new-track/razor visual.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Implementation: Create golden canvas fixtures covering every listed visual and state, close missing drawing/hit-test behavior, and verify high-DPI alignment at multiple zoom levels.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-a68911b741563de7

- Candidate/source: `doc-7c32dea050bb434a` at `docs/specs/frontend/13-implementation.md:40` (requirement)
- Expected behavior: Match ruler major/minor tick selection.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Implementation: Port the exact interval algorithm and add golden vector tests across fps, duration and zoom thresholds including boundary transitions.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-923f74b3f2c06d84

- Candidate/source: `doc-42b30459a843ac10` at `docs/specs/frontend/4-toolbar.md:1` (requirement)
- Expected behavior: At docs/specs/frontend/4-toolbar.md:1 under “Toolbar（工具条）” (heading), the source “# Toolbar（工具条）” requires this exact behavior: Toolbar groups, button states, shortcuts, and zoom controls are implemented.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/4-toolbar.md:1; signal=heading; heading=Toolbar（工具条）; candidate=# Toolbar（工具条）
  - Expected behavior: Toolbar groups, button states, shortcuts, and zoom controls are implemented. This closes only the promise expressed by “Toolbar（工具条）” in “Toolbar（工具条）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Toolbar（工具条）” with the scenario below and register test:web/src/__tests__/completion/doc-42b30459a843ac10.test.ts#completion_42b30459a843ac10_toolbar_groups_button_states_shortcuts_and_zoom_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Toolbar（工具条）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Toolbar groups, button states, shortcuts, and zoom controls are implemented.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-42b30459a843ac10.test.ts#completion_42b30459a843ac10_toolbar_groups_button_states_shortcuts_and_zoom_.

#### requirement-32c4ef714d183571

- Candidate/source: `doc-022af89f05a87cd4` at `docs/specs/frontend/4-toolbar.md:7` (requirement)
- Expected behavior: At docs/specs/frontend/4-toolbar.md:7 under “4.1 左侧按钮组（从左到右，组间用竖直 Divider，高 `Spacing.xl=20`，`ToolbarView.swift:15-16`）” (heading), the source “### 4.1 左侧按钮组（从左到右，组间用竖直 Divider，高 `Spacing.xl=20`，`ToolbarView.swift:15-16`）” requires this exact behavior: Toolbar groups, button states, shortcuts, and zoom controls are implemented.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/4-toolbar.md:7; signal=heading; heading=4.1 左侧按钮组（从左到右，组间用竖直 Divider，高 `Spacing.xl=20`，`ToolbarView.swift:15-16`）; candidate=### 4.1 左侧按钮组（从左到右，组间用竖直 Divider，高 `Spacing.xl=20`，`ToolbarView.swift:15-16`）
  - Expected behavior: Toolbar groups, button states, shortcuts, and zoom controls are implemented. This closes only the promise expressed by “4.1 左侧按钮组（从左到右，组间用竖直 Divider，高 `Spacing.xl=20`，`ToolbarView.swift:15-16`）” in “4.1 左侧按钮组（从左到右，组间用竖直 Divider，高 `Spacing.xl=20`，`ToolbarView.swift:15-16`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “4.1 左侧按钮组（从左到右，组间用竖直 Divider，高 `Spacing.xl=20`，`ToolbarView.swift:15-16`）” with the scenario below and register test:web/src/__tests__/completion/doc-022af89f05a87cd4.test.ts#completion_022af89f05a87cd4_toolbar_groups_button_states_shortcuts_and_zoom_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “4.1 左侧按钮组（从左到右，组间用竖直 Divider，高 `Spacing.xl=20`，`ToolbarView.swift:15-16`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Toolbar groups, button states, shortcuts, and zoom controls are implemented.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-022af89f05a87cd4.test.ts#completion_022af89f05a87cd4_toolbar_groups_button_states_shortcuts_and_zoom_.

#### requirement-1ec5bd08013418ac

- Candidate/source: `doc-1c35e09aee8c1318` at `docs/specs/frontend/4-toolbar.md:20` (requirement)
- Expected behavior: At docs/specs/frontend/4-toolbar.md:20 under “4.2 按钮样式（三种，`ToolbarView.swift:67-122`）” (heading), the source “### 4.2 按钮样式（三种，`ToolbarView.swift:67-122`）” requires this exact behavior: Toolbar groups, button states, shortcuts, and zoom controls are implemented.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/4-toolbar.md:20; signal=heading; heading=4.2 按钮样式（三种，`ToolbarView.swift:67-122`）; candidate=### 4.2 按钮样式（三种，`ToolbarView.swift:67-122`）
  - Expected behavior: Toolbar groups, button states, shortcuts, and zoom controls are implemented. This closes only the promise expressed by “4.2 按钮样式（三种，`ToolbarView.swift:67-122`）” in “4.2 按钮样式（三种，`ToolbarView.swift:67-122`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “4.2 按钮样式（三种，`ToolbarView.swift:67-122`）” with the scenario below and register test:web/src/__tests__/completion/doc-1c35e09aee8c1318.test.ts#completion_1c35e09aee8c1318_toolbar_groups_button_states_shortcuts_and_zoom_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “4.2 按钮样式（三种，`ToolbarView.swift:67-122`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Toolbar groups, button states, shortcuts, and zoom controls are implemented.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-1c35e09aee8c1318.test.ts#completion_1c35e09aee8c1318_toolbar_groups_button_states_shortcuts_and_zoom_.

#### requirement-6cdaf1b0bc759131

- Candidate/source: `doc-97158a1d823d838e` at `docs/specs/frontend/4-toolbar.md:27` (requirement)
- Expected behavior: At docs/specs/frontend/4-toolbar.md:27 under “4.3 右侧缩放滑块（`ToolbarView.swift:45-61`）” (heading), the source “### 4.3 右侧缩放滑块（`ToolbarView.swift:45-61`）” requires this exact behavior: Toolbar groups, button states, shortcuts, and zoom controls are implemented.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/4-toolbar.md:27; signal=heading; heading=4.3 右侧缩放滑块（`ToolbarView.swift:45-61`）; candidate=### 4.3 右侧缩放滑块（`ToolbarView.swift:45-61`）
  - Expected behavior: Toolbar groups, button states, shortcuts, and zoom controls are implemented. This closes only the promise expressed by “4.3 右侧缩放滑块（`ToolbarView.swift:45-61`）” in “4.3 右侧缩放滑块（`ToolbarView.swift:45-61`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “4.3 右侧缩放滑块（`ToolbarView.swift:45-61`）” with the scenario below and register test:web/src/__tests__/completion/doc-97158a1d823d838e.test.ts#completion_97158a1d823d838e_toolbar_groups_button_states_shortcuts_and_zoom_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “4.3 右侧缩放滑块（`ToolbarView.swift:45-61`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Toolbar groups, button states, shortcuts, and zoom controls are implemented.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-97158a1d823d838e.test.ts#completion_97158a1d823d838e_toolbar_groups_button_states_shortcuts_and_zoom_.

#### requirement-7f8b415cc15cc3a6

- Candidate/source: `doc-3e4d8d81c360ed3d` at `docs/specs/frontend/5-timeline.md:6` (requirement)
- Expected behavior: At docs/specs/frontend/5-timeline.md:6 under “5.1 容器结构（`TimelineContainerView.swift:6-57`）” (heading), the source “### 5.1 容器结构（`TimelineContainerView.swift:6-57`）” requires this exact behavior: The named timeline surface is implemented with focused geometry/interaction tests.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/5-timeline.md:6; signal=heading; heading=5.1 容器结构（`TimelineContainerView.swift:6-57`）; candidate=### 5.1 容器结构（`TimelineContainerView.swift:6-57`）
  - Expected behavior: The named timeline surface is implemented with focused geometry/interaction tests. This closes only the promise expressed by “5.1 容器结构（`TimelineContainerView.swift:6-57`）” in “5.1 容器结构（`TimelineContainerView.swift:6-57`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.1 容器结构（`TimelineContainerView.swift:6-57`）” with the scenario below and register test:web/src/__tests__/completion/doc-3e4d8d81c360ed3d.test.ts#completion_3e4d8d81c360ed3d_the_named_timeline_surface_is_implemented_with_f
  - Initial state/input/event: render the exact “5.1 容器结构（`TimelineContainerView.swift:6-57`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “5.1 容器结构（`TimelineContainerView.swift:6-57`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The named timeline surface is implemented with focused geometry/interaction tests.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-3e4d8d81c360ed3d.test.ts#completion_3e4d8d81c360ed3d_the_named_timeline_surface_is_implemented_with_f.

#### requirement-6f3d4e423997080c

- Candidate/source: `doc-47e9104a5ac65d31` at `docs/specs/frontend/5-timeline.md:39` (requirement)
- Expected behavior: At docs/specs/frontend/5-timeline.md:39 under “5.3 刻度 Ruler（`TimelineRuler.swift`）” (heading), the source “### 5.3 刻度 Ruler（`TimelineRuler.swift`）” requires this exact behavior: The named timeline surface is implemented with focused geometry/interaction tests.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/5-timeline.md:39; signal=heading; heading=5.3 刻度 Ruler（`TimelineRuler.swift`）; candidate=### 5.3 刻度 Ruler（`TimelineRuler.swift`）
  - Expected behavior: The named timeline surface is implemented with focused geometry/interaction tests. This closes only the promise expressed by “5.3 刻度 Ruler（`TimelineRuler.swift`）” in “5.3 刻度 Ruler（`TimelineRuler.swift`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.3 刻度 Ruler（`TimelineRuler.swift`）” with the scenario below and register test:web/src/__tests__/completion/doc-47e9104a5ac65d31.test.ts#completion_47e9104a5ac65d31_the_named_timeline_surface_is_implemented_with_f
  - Initial state/input/event: render the exact “5.3 刻度 Ruler（`TimelineRuler.swift`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “5.3 刻度 Ruler（`TimelineRuler.swift`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The named timeline surface is implemented with focused geometry/interaction tests.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-47e9104a5ac65d31.test.ts#completion_47e9104a5ac65d31_the_named_timeline_surface_is_implemented_with_f.

#### requirement-1f8eaadf44b47e85

- Candidate/source: `doc-636145055cbafd60` at `docs/specs/frontend/5-timeline.md:48` (requirement)
- Expected behavior: Complete the exact 5.4 Clip 渲染（`ClipRenderer.draw`，`ClipRenderer.swift:51-158`）—— 逐字复刻 behavior from the upstream interaction specification.
- Resolution: `validated-ledger-evidence:PT-timeline-visual-surface` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Draw video, audio, image, text, selected, offline, locked, and disabled clip states with correct trim/speed/media offsets.
  - Render thumbnail/waveform/text fallbacks without blocking interaction or leaking stale cache state.
  - Add canvas snapshots at three zoom levels and assert hit-test bounds exactly equal rendered clip bounds for each clip kind.

#### requirement-81f8ffc84e4f7032

- Candidate/source: `doc-84b44e65cafcf89f` at `docs/specs/frontend/5-timeline.md:75` (requirement)
- Expected behavior: Complete the exact 5.5 轨道头列（`TimelineHeaderView.swift`，AppKit draw） behavior from the upstream interaction specification.
- Resolution: `validated-ledger-evidence:PT-timeline-visual-surface` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Render one header row per visible track with name, type color, mute, solo, lock, visibility, selection, and height aligned to its timeline row.
  - Route header toggles through authoritative actions and enforce lock/mute/solo effects in edit/playback state with undo where specified.
  - Test mixed-height tracks, scrolling alignment, disabled states, keyboard focus, save/reopen, and solo/mute playback routing.

#### requirement-01ab68dad9bef233

- Candidate/source: `doc-04bc2ff53a8441a7` at `docs/specs/frontend/5-timeline.md:94` (requirement)
- Expected behavior: At docs/specs/frontend/5-timeline.md:94 under “5.6 Playhead（`PlayheadOverlay.swift`）” (heading), the source “### 5.6 Playhead（`PlayheadOverlay.swift`）” requires this exact behavior: The named timeline surface is implemented with focused geometry/interaction tests.
- Resolution: `reviewed-mapping-report:timeline-toolbar-ruler-visuals` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/5-timeline.md:94; signal=heading; heading=5.6 Playhead（`PlayheadOverlay.swift`）; candidate=### 5.6 Playhead（`PlayheadOverlay.swift`）
  - Expected behavior: The named timeline surface is implemented with focused geometry/interaction tests. This closes only the promise expressed by “5.6 Playhead（`PlayheadOverlay.swift`）” in “5.6 Playhead（`PlayheadOverlay.swift`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.6 Playhead（`PlayheadOverlay.swift`）” with the scenario below and register test:web/src/__tests__/completion/doc-04bc2ff53a8441a7.test.ts#completion_04bc2ff53a8441a7_the_named_timeline_surface_is_implemented_with_f
  - Initial state/input/event: render the exact “5.6 Playhead（`PlayheadOverlay.swift`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “5.6 Playhead（`PlayheadOverlay.swift`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The named timeline surface is implemented with focused geometry/interaction tests.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-04bc2ff53a8441a7.test.ts#completion_04bc2ff53a8441a7_the_named_timeline_surface_is_implemented_with_f.

#### requirement-9e5580044b5f8307

- Candidate/source: `doc-700d14880678f528` at `docs/specs/frontend/5-timeline.md:169` (requirement)
- Expected behavior: Complete the exact 5.9 时间线叠加绘制（`TimelineView.drawContent`，`TimelineView.swift:201-265`） behavior from the upstream interaction specification.
- Resolution: `validated-ledger-evidence:PT-timeline-visual-surface` — Both slices cover the same toolbar, ruler, track-header, clip and overlay painter surface and share three product paths plus three test paths.
- Exact acceptance contract:
  - Draw range selection, drag ghost, insertion line, trim guide, snap guide, marquee, and playhead in one coordinate system above clip content.
  - Clear overlays on commit, cancel, pointer loss, selection change, and project close without retaining stale positions.
  - Add DPR 1/2 snapshots and coordinate assertions at viewport edges, during auto-scroll, and at frame-zero/timeline-end boundaries.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TimelineParity.test.tsx#all_named_canvas_affordances` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/components/timeline/TimelineParity.test.tsx#toolbar_ruler_header_clip_playhead_overlay_visual_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/components/timeline/clipRenderer.test.ts#drawClip fade knees` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/timeline/clipRenderer.test.ts#paints the systemRed wash + border when missing` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/timeline/timelineOverlays.test.ts#marked-range overlay (content canvas)` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/timeline/timelineOverlays.test.ts#marked-range overlay (ruler canvas)` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/timeline/timelineOverlays.test.ts#paints a dashed box on the gap's track` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/toolbar/Toolbar.test.tsx#upstream_group_order_and_zoom_mapping` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineParity.test.tsx -t "all_named_canvas_affordances"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineParity.test.tsx -t "toolbar_ruler_header_clip_playhead_overlay_visual_matrix"`
  - Run: `pnpm -C web test -- --run src/components/timeline/clipRenderer.test.ts -t "drawClip fade knees"`
  - Run: `pnpm -C web test -- --run src/components/timeline/clipRenderer.test.ts -t "paints the systemRed wash + border when missing"`
  - Run: `pnpm -C web test -- --run src/components/timeline/timelineOverlays.test.ts -t "marked-range overlay (content canvas)"`
  - Run: `pnpm -C web test -- --run src/components/timeline/timelineOverlays.test.ts -t "marked-range overlay (ruler canvas)"`
  - Run: `pnpm -C web test -- --run src/components/timeline/timelineOverlays.test.ts -t "paints a dashed box on the gap's track"`
  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.test.tsx -t "upstream_group_order_and_zoom_mapping"`

  Expected: FAIL because one or more of the 17 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/Playhead.tsx#Playhead`, `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn`, `web/src/components/timeline/clipRenderer.ts#drawClip`, `web/src/components/timeline/rulerCanvas.ts#paintRuler`, `web/src/components/timeline/timelineCanvas.ts#paintTimeline`, `web/src/components/toolbar/Toolbar.tsx#Toolbar`, `docs/architecture/MODULE-PORT-MAP.md`, `docs/architecture/ROADMAP.md`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/13-implementation.md`, `docs/specs/frontend/4-toolbar.md`, `docs/specs/frontend/5-timeline.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineParity.test.tsx -t "all_named_canvas_affordances"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineParity.test.tsx -t "toolbar_ruler_header_clip_playhead_overlay_visual_matrix"`
  - Run: `pnpm -C web test -- --run src/components/timeline/clipRenderer.test.ts -t "drawClip fade knees"`
  - Run: `pnpm -C web test -- --run src/components/timeline/clipRenderer.test.ts -t "paints the systemRed wash + border when missing"`
  - Run: `pnpm -C web test -- --run src/components/timeline/timelineOverlays.test.ts -t "marked-range overlay (content canvas)"`
  - Run: `pnpm -C web test -- --run src/components/timeline/timelineOverlays.test.ts -t "marked-range overlay (ruler canvas)"`
  - Run: `pnpm -C web test -- --run src/components/timeline/timelineOverlays.test.ts -t "paints a dashed box on the gap's track"`
  - Run: `pnpm -C web test -- --run src/components/toolbar/Toolbar.test.tsx -t "upstream_group_order_and_zoom_mapping"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 7: PT-missing-media-red-wash (implementation-slice-d1b06208ca0b29d5)

**Covered records:**
- `requirement-6745f22112430b2a` (requirement)
- `requirement-ebadb3f6cf1a5de0` (requirement)
- `requirement-49881ee62b3013d4` (requirement)

**Files:**
- Modify: `web/src/components/timeline/TimelineContainer.tsx#missingMediaRefs`
- Modify: `web/src/components/timeline/clipRenderer.ts#drawClip`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/modules/web/timeline-ui.md`
- Modify: `docs/specs/frontend/5-timeline.md`
- Test (existing-owned): `web/src/components/timeline/clipRenderer.test.ts#paints the systemRed wash + border when missing`
- Test (existing-owned): `web/src/components/timeline/clipRenderer.test.ts#draws no red wash when the asset is present`

**Candidate-bound contracts:**

#### requirement-6745f22112430b2a

- Candidate/source: `doc-1c948f7d718c286e` at `docs/modules/web/SPEC.md:527` (requirement)
- Expected behavior: At docs/modules/web/SPEC.md:527 under “5.4 Clip 渲染（`ClipRenderer.draw`，`ClipRenderer.swift:51-158`）—— 逐字复刻” (gap-marker), the source “6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。” requires this exact behavior: Paint missing media clips with the specified red wash and border while preserving the rest of the layered timeline rendering.
- Resolution: `validated-ledger-evidence:PT-missing-media-red-wash` — Three records map to the same tracked offline-ID derivation and layered drawClip state.
- Exact acceptance contract:
  - Source binding: docs/modules/web/SPEC.md:527; signal=gap-marker; heading=5.4 Clip 渲染（`ClipRenderer.draw`，`ClipRenderer.swift:51-158`）—— 逐字复刻; candidate=6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。
  - Expected behavior: Paint missing media clips with the specified red wash and border while preserving the rest of the layered timeline rendering. This closes only the promise expressed by “6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。” in “5.4 Clip 渲染（`ClipRenderer.draw`，`ClipRenderer.swift:51-158`）—— 逐字复刻”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。” with the scenario below and register test:web/src/__tests__/completion/doc-1c948f7d718c286e.test.ts#completion_1c948f7d718c286e_paint_missing_media_clips_with_the_specified_red
  - Initial state/input/event: start from the smallest valid fixture for “Paint missing media clips with the specified red wash and border while preserving the rest of the layered timeline rendering.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。”.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Paint missing media clips with the specified red wash and border while preserving the rest of the layered timeline rendering.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-1c948f7d718c286e.test.ts#completion_1c948f7d718c286e_paint_missing_media_clips_with_the_specified_red.

#### requirement-ebadb3f6cf1a5de0

- Candidate/source: `doc-44fb31e4c4e8ba52` at `docs/modules/web/timeline-ui.md:30` (requirement)
- Expected behavior: At docs/modules/web/timeline-ui.md:30 under “渲染：Canvas 2D（非 DOM 堆叠）” (gap-marker), the source “- `clipRenderer.ts`：单个 clip 的分层绘制——填充 → 淡入淡出楔 → 左色带 → 边框 → 缺失底纹 → 标签栏 → 关键帧菱形 → 修剪把手；支持波形缓存、半透明幽灵、链接偏移徽章、音量 KF 幽灵。” requires this exact behavior: Paint missing media clips with the specified red wash and border while preserving the rest of the layered timeline rendering.
- Resolution: `validated-ledger-evidence:PT-missing-media-red-wash` — Three records map to the same tracked offline-ID derivation and layered drawClip state.
- Exact acceptance contract:
  - Source binding: docs/modules/web/timeline-ui.md:30; signal=gap-marker; heading=渲染：Canvas 2D（非 DOM 堆叠）; candidate=- `clipRenderer.ts`：单个 clip 的分层绘制——填充 → 淡入淡出楔 → 左色带 → 边框 → 缺失底纹 → 标签栏 → 关键帧菱形 → 修剪把手；支持波形缓存、半透明幽灵、链接偏移徽章、音量 KF 幽灵。
  - Expected behavior: Paint missing media clips with the specified red wash and border while preserving the rest of the layered timeline rendering. This closes only the promise expressed by “`clipRenderer.ts`：单个 clip 的分层绘制——填充 → 淡入淡出楔 → 左色带 → 边框 → 缺失底纹 → 标签栏 → 关键帧菱形 → 修剪把手；支持波形缓存、半透明幽灵、链接偏移徽章、音量 KF 幽灵。” in “渲染：Canvas 2D（非 DOM 堆叠）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “`clipRenderer.ts`：单个 clip 的分层绘制——填充 → 淡入淡出楔 → 左色带 → 边框 → 缺失底纹 → 标签栏 → 关键帧菱形 → 修剪把手；支持波形缓存、半透明幽灵、链接偏移徽章、音量 KF 幽灵。” with the scenario below and register test:web/src/__tests__/completion/doc-44fb31e4c4e8ba52.test.ts#completion_44fb31e4c4e8ba52_paint_missing_media_clips_with_the_specified_red
  - Initial state/input/event: start from the smallest valid fixture for “Paint missing media clips with the specified red wash and border while preserving the rest of the layered timeline rendering.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “`clipRenderer.ts`：单个 clip 的分层绘制——填充 → 淡入淡出楔 → 左色带 → 边框 → 缺失底纹 → 标签栏 → 关键帧菱形 → 修剪把手；支持波形缓存、半透明幽灵、链接偏移徽章、音量 KF 幽灵。”.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Paint missing media clips with the specified red wash and border while preserving the rest of the layered timeline rendering.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-44fb31e4c4e8ba52.test.ts#completion_44fb31e4c4e8ba52_paint_missing_media_clips_with_the_specified_red.

#### requirement-49881ee62b3013d4

- Candidate/source: `doc-92e06193058b0f2b` at `docs/specs/frontend/5-timeline.md:60` (requirement)
- Expected behavior: At docs/specs/frontend/5-timeline.md:60 under “5.4 Clip 渲染（`ClipRenderer.draw`，`ClipRenderer.swift:51-158`）—— 逐字复刻” (gap-marker), the source “6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。” requires this exact behavior: Paint the specified red wash/border for missing non-generating clips.
- Resolution: `validated-ledger-evidence:PT-missing-media-red-wash` — Three records map to the same tracked offline-ID derivation and layered drawClip state.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/5-timeline.md:60; signal=gap-marker; heading=5.4 Clip 渲染（`ClipRenderer.draw`，`ClipRenderer.swift:51-158`）—— 逐字复刻; candidate=6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。
  - Expected behavior: Paint the specified red wash/border for missing non-generating clips. This closes only the promise expressed by “6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。” in “5.4 Clip 渲染（`ClipRenderer.draw`，`ClipRenderer.swift:51-158`）—— 逐字复刻”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。” with the scenario below and register test:web/src/__tests__/completion/doc-92e06193058b0f2b.test.ts#completion_92e06193058b0f2b_paint_the_specified_red_wash_border_for_missing_
  - Initial state/input/event: start from the smallest valid fixture for “Paint the specified red wash/border for missing non-generating clips.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。”.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Paint the specified red wash/border for missing non-generating clips.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-92e06193058b0f2b.test.ts#completion_92e06193058b0f2b_paint_the_specified_red_wash_border_for_missing_.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/clipRenderer.test.ts#paints the systemRed wash + border when missing` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/timeline/clipRenderer.test.ts#draws no red wash when the asset is present` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/clipRenderer.test.ts -t "paints the systemRed wash + border when missing"`
  - Run: `pnpm -C web test -- --run src/components/timeline/clipRenderer.test.ts -t "draws no red wash when the asset is present"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineContainer.tsx#missingMediaRefs`, `web/src/components/timeline/clipRenderer.ts#drawClip`, `docs/modules/web/SPEC.md`, `docs/modules/web/timeline-ui.md`, `docs/specs/frontend/5-timeline.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/clipRenderer.test.ts -t "paints the systemRed wash + border when missing"`
  - Run: `pnpm -C web test -- --run src/components/timeline/clipRenderer.test.ts -t "draws no red wash when the asset is present"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 8: PT-preview-status-overlays (implementation-slice-b31a4a22f5466e5b)

**Covered records:**
- `requirement-e1ede16014348bab` (requirement)
- `requirement-e413cc13c2bc68a1` (requirement)
- `requirement-2a219d0f70cc70b1` (requirement)

**Files:**
- Modify: `web/src/components/preview/Preview.tsx#Preview`
- Modify: `web/src/store/mediaStore.ts#refreshMedia`
- Modify: `web/src/components/preview/PreviewStatusOverlay.tsx#PreviewStatusOverlay`
- Modify: `web/src/lib/types.ts#GenerationStatus`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/8-preview.md`
- Test (existing-owned): `web/src/components/preview/Preview.test.tsx#renders a user visible unsupported surface instead of incomplete DOM media`
- Test (reviewed-planned): `web/src/components/preview/Preview.status.test.tsx#offline_failed_generating_progress_and_recovery_matrix`

**Candidate-bound contracts:**

#### requirement-e1ede16014348bab

- Candidate/source: `doc-9136dce886b36a8c` at `docs/modules/web/SPEC.md:1269` (requirement)
- Expected behavior: Match preview tabs, transport, scrub states, settings badge, and overlays to upstream.
- Resolution: `validated-ledger-evidence:PT-preview-status-overlays` — Preview lacks the required offline/failed/generating state union and overlay state machine.
- Exact acceptance contract:
  - Create a pinned upstream/current screenshot-state matrix at the same viewport, scale, locale, theme, and fixture.
  - Add deterministic component/browser assertions for normal, hover, focus, disabled, selected, empty, error, and compact states.
  - Record reviewed pixel/geometry evidence for every item named by the checklist, with no unchecked item remaining.

#### requirement-e413cc13c2bc68a1

- Candidate/source: `doc-645fc0e62b36e447` at `docs/specs/frontend/8-preview.md:1` (requirement)
- Expected behavior: Preview shows accurate offline, failure, and generation progress overlays without hiding recoverable state.
- Resolution: `validated-ledger-evidence:PT-preview-status-overlays` — Preview lacks the required offline/failed/generating state union and overlay state machine.
- Exact acceptance contract:
  - Render source/timeline tabs, canvas, transport, scrub, transform/crop, and offline/failure/generating overlays through the single capability route.
  - Overlay actions must provide relink/retry/cancel and never obscure retained last frame or publish stale frames after project/session changes.
  - Test WebKit/Rust routes, source/timeline switch, offline/relink, render failure/retry, generation progress/cancel, retained frame, transform/crop, and packaged playback/export parity.

#### requirement-2a219d0f70cc70b1

- Candidate/source: `doc-0a82024735e77686` at `docs/specs/frontend/8-preview.md:56` (requirement)
- Expected behavior: Preview shows accurate offline, failure, and generation progress overlays without hiding recoverable state.
- Resolution: `validated-ledger-evidence:PT-preview-status-overlays` — Preview lacks the required offline/failed/generating state union and overlay state machine.
- Exact acceptance contract:
  - Show distinct offline, decode/render failure, and generation queued/running/cancelling states with asset/job identity and accessible status text.
  - Wire offline→relink, failure→retry, and generation→cancel/retry; stale completion from an old asset/project/job must be ignored.
  - Test every state transition, rapid selection/project switch, duplicate job events, relink success/failure, cancellation race, retained-frame visibility, and visual snapshots.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/Preview.test.tsx#renders a user visible unsupported surface instead of incomplete DOM media` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/Preview.status.test.tsx#offline_failed_generating_progress_and_recovery_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.test.tsx -t "renders a user visible unsupported surface instead of incomplete DOM media"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.status.test.tsx -t "offline_failed_generating_progress_and_recovery_matrix"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/Preview.tsx#Preview`, `web/src/store/mediaStore.ts#refreshMedia`, `web/src/components/preview/PreviewStatusOverlay.tsx#PreviewStatusOverlay`, `web/src/lib/types.ts#GenerationStatus`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/8-preview.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.test.tsx -t "renders a user visible unsupported surface instead of incomplete DOM media"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.status.test.tsx -t "offline_failed_generating_progress_and_recovery_matrix"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 9: PT-editor-interaction-matrix + timeline-selection-drag-trim-context + timeline-trackpad-gestures (implementation-slice-6d2d918b083e57c0)

**Covered records:**
- `requirement-aee5b6064f63d4ed` (requirement)
- `requirement-e819a53e21abdd61` (requirement)
- `requirement-2a4cb9892d0b01ad` (requirement)
- `requirement-390cb44707fd921c` (requirement)
- `requirement-489198df8df16deb` (requirement)
- `requirement-a548b853da826bc3` (requirement)
- `requirement-d93395512a23997b` (requirement)
- `requirement-a35e49bb4f4b3488` (requirement)
- `requirement-f53dbd6c1c65375f` (requirement)
- `requirement-6db3895c5fd3fd75` (requirement)
- `requirement-92ec4c8b557a6ee3` (requirement)
- `requirement-508312eff31773fb` (requirement)
- `requirement-1779ecc2d9f24651` (requirement)
- `requirement-64a0fdf590212573` (requirement)
- `requirement-8e0afbf35b51e9e9` (requirement)
- `requirement-e6604beaade77879` (requirement)
- `requirement-5757937ea0526240` (requirement)
- `requirement-be7ce2bc7e82ad78` (requirement)
- `requirement-67ec865065b862f1` (requirement)
- `requirement-238be717bbf8bfdd` (requirement)
- `requirement-a5977b149ca8a9ec` (requirement)

**Files:**
- Modify: `web/src/components/timeline/ClipContextMenu.tsx#clipContextMenuItems`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#onWheel`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#resolveExistingTrackMove`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#resolveNewTrackMove`
- Modify: `web/src/components/timeline/TimelineRangeContextMenu.tsx#rangeContextMenuItems`
- Modify: `web/src/store/editActions.ts#buildMediaInsertPlan`
- Modify: `web/src/store/editActions.ts#splitAtPlayhead`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Modify: `docs/specs/frontend/5-timeline.md`
- Modify: `docs/specs/frontend/9-interactions.md`
- Modify: `docs/需求与问题汇总.md`
- Test (existing-owned): `web/src/components/timeline/ClipContextMenu.test.tsx#clipContextMenuItems`
- Test (existing-owned): `web/src/components/timeline/ClipContextMenu.test.tsx#wires copy, paste, link, and swap actions`
- Test (reviewed-planned): `web/src/components/timeline/TimelineContainer.gestures.test.tsx#trackpad_pinch_horizontal_vertical_navigation`
- Test (reviewed-planned): `web/src/components/timeline/TimelineContainer.interactions.test.tsx#frontend_spec_9_selection_drag_trim_scrub_matrix`
- Test (reviewed-planned): `web/src/components/timeline/TimelineContainer.interactions.test.tsx#selection_drag_trim_razor_scrub_zoom_context_full_matrix`
- Test (existing-owned): `web/src/components/timeline/TimelineContainer.test.ts#moves a pure video multi-selection as one rigid track delta`
- Test (existing-owned): `web/src/components/timeline/TimelineContainer.test.ts#resolveExistingTrackMove`
- Test (existing-owned): `web/src/components/timeline/hitTest.test.ts#fade knee drag math`

**Candidate-bound contracts:**

#### requirement-aee5b6064f63d4ed

- Candidate/source: `doc-37138c743cfb09e6` at `docs/modules/web/SPEC.md:1272` (requirement)
- Expected behavior: Verify every §9.1 selection gesture and edge case.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Translate every numbered checklist row into a named deterministic interaction test and a native/browser runtime evidence row where applicable.
  - Assert the exact store/API command, visible success/failure state, and no-op behavior for rejected/cancelled paths.
  - Pass the full matrix against the pinned upstream semantics with no unchecked rows.

#### requirement-e819a53e21abdd61

- Candidate/source: `doc-c817968d9049822a` at `docs/modules/web/SPEC.md:1273` (requirement)
- Expected behavior: Verify every §9.2 drag/drop rule, including no-op return, cross-track clamping, and linked movement.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Translate every numbered checklist row into a named deterministic interaction test and a native/browser runtime evidence row where applicable.
  - Assert the exact store/API command, visible success/failure state, and no-op behavior for rejected/cancelled paths.
  - Pass the full matrix against the pinned upstream semantics with no unchecked rows.

#### requirement-2a4cb9892d0b01ad

- Candidate/source: `doc-f17cf6d289a59abe` at `docs/modules/web/SPEC.md:1274` (requirement)
- Expected behavior: Verify every §9.3 trim rule, including one-frame minimum, unbounded stills, and linked propagation.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Translate every numbered checklist row into a named deterministic interaction test and a native/browser runtime evidence row where applicable.
  - Assert the exact store/API command, visible success/failure state, and no-op behavior for rejected/cancelled paths.
  - Pass the full matrix against the pinned upstream semantics with no unchecked rows.

#### requirement-390cb44707fd921c

- Candidate/source: `doc-e190fc860d9a9a47` at `docs/modules/web/SPEC.md:1275` (requirement)
- Expected behavior: Verify every §9.4 razor, split, keyframe, fade, and volume gesture.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Translate every numbered checklist row into a named deterministic interaction test and a native/browser runtime evidence row where applicable.
  - Assert the exact store/API command, visible success/failure state, and no-op behavior for rejected/cancelled paths.
  - Pass the full matrix against the pinned upstream semantics with no unchecked rows.

#### requirement-489198df8df16deb

- Candidate/source: `doc-966cdb4abb2f2272` at `docs/modules/web/SPEC.md:1276` (requirement)
- Expected behavior: Verify every §9.5 playhead, scrub, wheel, pinch, zoom-anchor, and fit gesture.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Translate every numbered checklist row into a named deterministic interaction test and a native/browser runtime evidence row where applicable.
  - Assert the exact store/API command, visible success/failure state, and no-op behavior for rejected/cancelled paths.
  - Pass the full matrix against the pinned upstream semantics with no unchecked rows.

#### requirement-a548b853da826bc3

- Candidate/source: `doc-30f66ce99381dde1` at `docs/modules/web/SPEC.md:1279` (requirement)
- Expected behavior: Verify every §5.10 context-menu item, label, grouping, enabled state, command path, and result.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Translate every numbered checklist row into a named deterministic interaction test and a native/browser runtime evidence row where applicable.
  - Assert the exact store/API command, visible success/failure state, and no-op behavior for rejected/cancelled paths.
  - Pass the full matrix against the pinned upstream semantics with no unchecked rows.

#### requirement-d93395512a23997b

- Candidate/source: `doc-c1323e900de88bfe` at `docs/specs/frontend/13-implementation.md:29` (requirement)
- Expected behavior: Satisfy all eight selection behaviors in frontend spec 9.1.
- Resolution: `reviewed-mapping-report:timeline-selection-drag-trim-context` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Implementation: Turn each of the eight rows into a named interaction test and fix selection/link/marquee/hidden-track behavior until all pass in browser and canvas accessibility paths.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-a35e49bb4f4b3488

- Candidate/source: `doc-40e4354008e6d057` at `docs/specs/frontend/13-implementation.md:30` (requirement)
- Expected behavior: Satisfy all seven drag/drop behaviors in frontend spec 9.2.
- Resolution: `reviewed-mapping-report:timeline-selection-drag-trim-context` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Implementation: Add named tests for all seven rows including same-position no-op, cross-track type clamp and linked propagation; verify command payloads and undo.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-f53dbd6c1c65375f

- Candidate/source: `doc-67688338100efb97` at `docs/specs/frontend/13-implementation.md:31` (requirement)
- Expected behavior: Satisfy all seven trim behaviors in frontend spec 9.3.
- Resolution: `reviewed-mapping-report:timeline-selection-drag-trim-context` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Implementation: Add named tests for all seven trim rows including one-frame minimum, image extension and linked propagation; verify preview and undo at boundaries.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-6db3895c5fd3fd75

- Candidate/source: `doc-124219a85a6826c2` at `docs/specs/frontend/13-implementation.md:33` (requirement)
- Expected behavior: Satisfy all nine playhead/scrub/zoom behaviors in frontend spec 9.5.
- Resolution: `reviewed-mapping-report:timeline-selection-drag-trim-context` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Implementation: Add one test per row, including cursor-anchored zoom and exact sensitivity constants, then validate browser and native playback routes.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-92ec4c8b557a6ee3

- Candidate/source: `doc-367b6ff35eb14895` at `docs/specs/frontend/13-implementation.md:36` (requirement)
- Expected behavior: Match every context-menu item, label, order, grouping, and enabled state.
- Resolution: `reviewed-mapping-report:timeline-selection-drag-trim-context` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Implementation: Compare the generated menu model with spec 5.10 for every clip type/selection/clipboard/range state and add exact snapshot/behavior tests.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-508312eff31773fb

- Candidate/source: `doc-0e55f7ddcfeed8c5` at `docs/specs/frontend/5-timeline.md:117` (requirement)
- Expected behavior: Complete the exact 5.8 时间线全部手势（`TimelineInputController.swift` + `TimelineView` 输入转发） behavior from the upstream interaction specification.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Implement click/modifier selection, marquee, move/copy drag, trim, razor, range select, playhead scrub, pan, and pinch/wheel zoom with explicit gesture arbitration.
  - Reject mutations on locked tracks, clamp all edits to valid frames, and commit each gesture as one undoable command.
  - Add pointer/trackpad cases for cancel/Escape, pointer loss, crossing clip boundaries, auto-scroll, modifiers, and undo/redo at DPR 1/2.

#### requirement-1779ecc2d9f24651

- Candidate/source: `doc-7000e750c310343a` at `docs/specs/frontend/5-timeline.md:188` (requirement)
- Expected behavior: At docs/specs/frontend/5-timeline.md:188 under “5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）” (heading), the source “### 5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）” requires this exact behavior: The named timeline surface is implemented with focused geometry/interaction tests.
- Resolution: `reviewed-mapping-report:timeline-selection-drag-trim-context` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/5-timeline.md:188; signal=heading; heading=5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）; candidate=### 5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）
  - Expected behavior: The named timeline surface is implemented with focused geometry/interaction tests. This closes only the promise expressed by “5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）” in “5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）” with the scenario below and register test:web/src/__tests__/completion/doc-7000e750c310343a.test.ts#completion_7000e750c310343a_the_named_timeline_surface_is_implemented_with_f
  - Initial state/input/event: render the exact “5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The named timeline surface is implemented with focused geometry/interaction tests.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-7000e750c310343a.test.ts#completion_7000e750c310343a_the_named_timeline_surface_is_implemented_with_f.

#### requirement-64a0fdf590212573

- Candidate/source: `doc-35a93d6bec1cfb7e` at `docs/specs/frontend/5-timeline.md:203` (requirement)
- Expected behavior: At docs/specs/frontend/5-timeline.md:203 under “5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）” (heading), the source “### 5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）” requires this exact behavior: The named timeline surface is implemented with focused geometry/interaction tests.
- Resolution: `reviewed-mapping-report:timeline-selection-drag-trim-context` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/5-timeline.md:203; signal=heading; heading=5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）; candidate=### 5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）
  - Expected behavior: The named timeline surface is implemented with focused geometry/interaction tests. This closes only the promise expressed by “5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）” in “5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）” with the scenario below and register test:web/src/__tests__/completion/doc-35a93d6bec1cfb7e.test.ts#completion_35a93d6bec1cfb7e_the_named_timeline_surface_is_implemented_with_f
  - Initial state/input/event: render the exact “5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “The named timeline surface is implemented with focused geometry/interaction tests.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-35a93d6bec1cfb7e.test.ts#completion_35a93d6bec1cfb7e_the_named_timeline_surface_is_implemented_with_f.

#### requirement-8e0afbf35b51e9e9

- Candidate/source: `doc-8ee58baaa4911dc9` at `docs/specs/frontend/9-interactions.md:1` (requirement)
- Expected behavior: Complete the exhaustive 交互细节逐项清单（1:1 关键） interaction matrix with parity across mouse, trackpad, and keyboard inputs.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Every row in §9.1–§9.8 has a linked implementation and automated assertion for mouse, keyboard, and relevant trackpad input.
  - Equivalent toolbar/menu/shortcut gestures must call the same action and create the same undo entry.
  - Run the interaction matrix on the packaged desktop shell with no unchecked selection, drag, trim, keyframe, transport, shortcut, focus, or context-menu rows.

#### requirement-e6604beaade77879

- Candidate/source: `doc-c8ad5b2fa6ba31d2` at `docs/specs/frontend/9-interactions.md:5` (requirement)
- Expected behavior: Complete the exhaustive 9.1 选择（clip） interaction matrix with parity across mouse, trackpad, and keyboard inputs.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Implement single, additive, toggle, range, marquee, track, and empty-canvas deselection exactly as specified.
  - Selection changes must not mutate timeline content and must remain valid after delete, undo, project switch, and remote version refresh.
  - Test click/modifier combinations, overlapping clips, locked tracks, offscreen clips, marquee direction, and focus transfer.

#### requirement-5757937ea0526240

- Candidate/source: `doc-0b456e029525c42c` at `docs/specs/frontend/9-interactions.md:18` (requirement)
- Expected behavior: Complete the exhaustive 9.2 拖放（移动/复制/落轨） interaction matrix with parity across mouse, trackpad, and keyboard inputs.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Move/copy one or many clips across compatible tracks with snap, insertion preview, collision policy, auto-scroll, and exact frame conversion.
  - Commit one shared command per gesture; Escape/pointer loss/invalid target must restore the pre-drag timeline with no orphaned tracks.
  - Test video/audio/image groups, modifier copy, linked items, locked/incompatible tracks, frame-zero/end clamps, undo/redo, save, and reopen.

#### requirement-be7ce2bc7e82ad78

- Candidate/source: `doc-d648b47b2688abde` at `docs/specs/frontend/9-interactions.md:30` (requirement)
- Expected behavior: Complete the exhaustive 9.3 Trim（修剪） interaction matrix with parity across mouse, trackpad, and keyboard inputs.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Trim both edges with source-handle, minimum-duration, speed, linked-media, snap, and ripple rules enforced in frames.
  - Preview updates may be interactive, but pointer-up must commit one command and Escape must restore the original clip exactly.
  - Test fractional pointer positions, frame-zero/media-end clamps, reverse/speed clips, linked audio, collision/ripple, undo/redo, and preview/export boundary frames.

#### requirement-67ec865065b862f1

- Candidate/source: `doc-9ffa7b2f8efdc31d` at `docs/specs/frontend/9-interactions.md:55` (requirement)
- Expected behavior: Complete the exhaustive 9.5 Playhead / scrub / 缩放 interaction matrix with parity across mouse, trackpad, and keyboard inputs.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Playhead click/drag, scrub, wheel/pinch zoom, and pan share the timeline coordinate transform and capability-routed seek path.
  - Throttle interactive seeks without losing the final exact frame; pause/resume and route changes must not create competing sessions.
  - Test frame 0/end, rapid 200-event scrubs, zoom anchored under pointer, trackpad directions, pause/resume, and final-frame equality.

#### requirement-238be717bbf8bfdd

- Candidate/source: `doc-c22f62433b427c2e` at `docs/specs/frontend/9-interactions.md:116` (requirement)
- Expected behavior: Complete the exhaustive 9.8 右键菜单（汇总，见 §5.10 详表） interaction matrix with parity across mouse, trackpad, and keyboard inputs.
- Resolution: `validated-ledger-evidence:PT-editor-interaction-matrix` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Implement every context-menu row and state from §5.10 for clip, range, gap, track, and empty canvas.
  - Menu commands must match keyboard/toolbar behavior, honor lock/offline/selection state, and create identical undo entries.
  - Table-drive open/enable/check/invoke/dismiss behavior for every menu item and compare resulting commands/state to the corresponding action.

#### requirement-a5977b149ca8a9ec

- Candidate/source: `doc-346554cd9aa77cc1` at `docs/需求与问题汇总.md:42` (requirement)
- Expected behavior: Trackpad pan, pinch zoom, and horizontal/vertical navigation match the editor interaction contract.
- Resolution: `reviewed-mapping-report:timeline-trackpad-gestures` — The ledger interaction matrix is the umbrella owner for the report's mouse/context/trim matrix and its focused trackpad gesture slice; merging keeps one exhaustive interaction gate.
- Exact acceptance contract:
  - Implement and normalize trackpad gesture handling.
  - Prevent gesture conflicts with clip drag, trim, and scrub.
  - Add browser event and desktop interaction tests.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/ClipContextMenu.test.tsx#clipContextMenuItems` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/timeline/ClipContextMenu.test.tsx#wires copy, paste, link, and swap actions` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/timeline/TimelineContainer.gestures.test.tsx#trackpad_pinch_horizontal_vertical_navigation` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/components/timeline/TimelineContainer.interactions.test.tsx#frontend_spec_9_selection_drag_trim_scrub_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/components/timeline/TimelineContainer.interactions.test.tsx#selection_drag_trim_razor_scrub_zoom_context_full_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/components/timeline/TimelineContainer.test.ts#moves a pure video multi-selection as one rigid track delta` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/timeline/TimelineContainer.test.ts#resolveExistingTrackMove` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/timeline/hitTest.test.ts#fade knee drag math` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/ClipContextMenu.test.tsx -t "clipContextMenuItems"`
  - Run: `pnpm -C web test -- --run src/components/timeline/ClipContextMenu.test.tsx -t "wires copy, paste, link, and swap actions"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.gestures.test.tsx -t "trackpad_pinch_horizontal_vertical_navigation"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interactions.test.tsx -t "frontend_spec_9_selection_drag_trim_scrub_matrix"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interactions.test.tsx -t "selection_drag_trim_razor_scrub_zoom_context_full_matrix"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.test.ts -t "moves a pure video multi-selection as one rigid track delta"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.test.ts -t "resolveExistingTrackMove"`
  - Run: `pnpm -C web test -- --run src/components/timeline/hitTest.test.ts -t "fade knee drag math"`

  Expected: FAIL because one or more of the 21 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/ClipContextMenu.tsx#clipContextMenuItems`, `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`, `web/src/components/timeline/TimelineContainer.tsx#onWheel`, `web/src/components/timeline/TimelineContainer.tsx#resolveExistingTrackMove`, `web/src/components/timeline/TimelineContainer.tsx#resolveNewTrackMove`, `web/src/components/timeline/TimelineRangeContextMenu.tsx#rangeContextMenuItems`, `web/src/store/editActions.ts#buildMediaInsertPlan`, `web/src/store/editActions.ts#splitAtPlayhead`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/13-implementation.md`, `docs/specs/frontend/5-timeline.md`, `docs/specs/frontend/9-interactions.md`, `docs/需求与问题汇总.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/ClipContextMenu.test.tsx -t "clipContextMenuItems"`
  - Run: `pnpm -C web test -- --run src/components/timeline/ClipContextMenu.test.tsx -t "wires copy, paste, link, and swap actions"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.gestures.test.tsx -t "trackpad_pinch_horizontal_vertical_navigation"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interactions.test.tsx -t "frontend_spec_9_selection_drag_trim_scrub_matrix"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interactions.test.tsx -t "selection_drag_trim_razor_scrub_zoom_context_full_matrix"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.test.ts -t "moves a pure video multi-selection as one rigid track delta"`
  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.test.ts -t "resolveExistingTrackMove"`
  - Run: `pnpm -C web test -- --run src/components/timeline/hitTest.test.ts -t "fade knee drag math"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 10: PT-versioned-frontend-sync + ui-rust-state-boundary (implementation-slice-e68ac3640402a80f)

**Covered records:**
- `requirement-4e8c2a4123667b92` (requirement)
- `requirement-18f2106f33522079` (requirement)
- `requirement-0858e3409610b51b` (requirement)
- `requirement-63e2699bd9d27494` (requirement)

**Files:**
- Modify: `src-tauri/src/commands.rs#get_timeline`
- Modify: `web/src/store/projectStore.ts#replaceProjectSnapshot`
- Modify: `web/src/store/sync.ts#startSync`
- Modify: `web/src/store/uiStore.ts#useEditorUiStore`
- Modify: `docs/specs/core/4-frontend-sync.md`
- Modify: `docs/specs/frontend/10-state.md`
- Test (existing-owned): `web/src/store/sync.test.ts#does not let a late old snapshot replace a newer project`
- Test (existing-owned): `web/src/store/sync.test.ts#does not let pending history cross a same-identity compatibility replacement`
- Test (reviewed-planned): `web/src/store/uiStore.boundary.test.ts#derived_selectors_do_not_mutate_timeline_projection`
- Test (existing-owned): `web/src/store/uiStore.test.ts#timeline playback state`

**Candidate-bound contracts:**

#### requirement-4e8c2a4123667b92

- Candidate/source: `doc-495a0452ecb65986` at `docs/specs/core/4-frontend-sync.md:17` (requirement)
- Expected behavior: At docs/specs/core/4-frontend-sync.md:17 under “4.3 乱序与并发(跨进程必须显式处理,上游不存在)” (heading), the source “### 4.3 乱序与并发(跨进程必须显式处理,上游不存在)” requires this exact behavior: Version ordering and TimelineDTO boundaries are explicit and tested.
- Resolution: `validated-ledger-evidence:PT-versioned-frontend-sync` — Both slices own the Rust-projected, version/epoch ordered frontend mirror boundary through web/src/store/sync.ts; keep one capability and union the pure-selector plus race evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/4-frontend-sync.md:17; signal=heading; heading=4.3 乱序与并发(跨进程必须显式处理,上游不存在); candidate=### 4.3 乱序与并发(跨进程必须显式处理,上游不存在)
  - Expected behavior: Version ordering and TimelineDTO boundaries are explicit and tested. This closes only the promise expressed by “4.3 乱序与并发(跨进程必须显式处理,上游不存在)” in “4.3 乱序与并发(跨进程必须显式处理,上游不存在)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “4.3 乱序与并发(跨进程必须显式处理,上游不存在)” with the scenario below and register test:web/src/__tests__/completion/doc-495a0452ecb65986.test.ts#completion_495a0452ecb65986_version_ordering_and_timelinedto_boundaries_are_
  - Initial state/input/event: render the exact “4.3 乱序与并发(跨进程必须显式处理,上游不存在)” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “4.3 乱序与并发(跨进程必须显式处理,上游不存在)”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “Version ordering and TimelineDTO boundaries are explicit and tested.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-495a0452ecb65986.test.ts#completion_495a0452ecb65986_version_ordering_and_timelinedto_boundaries_are_.

#### requirement-18f2106f33522079

- Candidate/source: `doc-106d7b0b9ad776db` at `docs/specs/core/4-frontend-sync.md:25` (requirement)
- Expected behavior: At docs/specs/core/4-frontend-sync.md:25 under “4.4 TimelineDTO 边界” (heading), the source “### 4.4 TimelineDTO 边界” requires this exact behavior: Version ordering and TimelineDTO boundaries are explicit and tested.
- Resolution: `validated-ledger-evidence:PT-versioned-frontend-sync` — Both slices own the Rust-projected, version/epoch ordered frontend mirror boundary through web/src/store/sync.ts; keep one capability and union the pure-selector plus race evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/4-frontend-sync.md:25; signal=heading; heading=4.4 TimelineDTO 边界; candidate=### 4.4 TimelineDTO 边界
  - Expected behavior: Version ordering and TimelineDTO boundaries are explicit and tested. This closes only the promise expressed by “4.4 TimelineDTO 边界” in “4.4 TimelineDTO 边界”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “4.4 TimelineDTO 边界” with the scenario below and register test:web/src/__tests__/completion/doc-106d7b0b9ad776db.test.ts#completion_106d7b0b9ad776db_version_ordering_and_timelinedto_boundaries_are_
  - Initial state/input/event: render the exact “4.4 TimelineDTO 边界” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “4.4 TimelineDTO 边界”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “Version ordering and TimelineDTO boundaries are explicit and tested.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-106d7b0b9ad776db.test.ts#completion_106d7b0b9ad776db_version_ordering_and_timelinedto_boundaries_are_.

#### requirement-0858e3409610b51b

- Candidate/source: `doc-91c8ce5b149f0146` at `docs/specs/frontend/10-state.md:26` (requirement)
- Expected behavior: At docs/specs/frontend/10-state.md:26 under “10.2 UI-only 态 —— `useEditorUiStore`（前端自管）” (heading), the source “### 10.2 UI-only 态 —— `useEditorUiStore`（前端自管）” requires this exact behavior: UI-only state and pure derived selectors remain separate from the Rust timeline projection.
- Resolution: `reviewed-mapping-report:ui-rust-state-boundary` — Both slices own the Rust-projected, version/epoch ordered frontend mirror boundary through web/src/store/sync.ts; keep one capability and union the pure-selector plus race evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/10-state.md:26; signal=heading; heading=10.2 UI-only 态 —— `useEditorUiStore`（前端自管）; candidate=### 10.2 UI-only 态 —— `useEditorUiStore`（前端自管）
  - Expected behavior: UI-only state and pure derived selectors remain separate from the Rust timeline projection. This closes only the promise expressed by “10.2 UI-only 态 —— `useEditorUiStore`（前端自管）” in “10.2 UI-only 态 —— `useEditorUiStore`（前端自管）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “10.2 UI-only 态 —— `useEditorUiStore`（前端自管）” with the scenario below and register test:web/src/__tests__/completion/doc-91c8ce5b149f0146.test.ts#completion_91c8ce5b149f0146_ui_only_state_and_pure_derived_selectors_remain_
  - Initial state/input/event: render the exact “10.2 UI-only 态 —— `useEditorUiStore`（前端自管）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “10.2 UI-only 态 —— `useEditorUiStore`（前端自管）”.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, have the React handler call the typed store/API/Tauri action for “UI-only state and pure derived selectors remain separate from the Rust timeline projection.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-91c8ce5b149f0146.test.ts#completion_91c8ce5b149f0146_ui_only_state_and_pure_derived_selectors_remain_.

#### requirement-63e2699bd9d27494

- Candidate/source: `doc-7bcd544ecb82bee0` at `docs/specs/frontend/10-state.md:92` (requirement)
- Expected behavior: At docs/specs/frontend/10-state.md:92 under “10.3 派生选择器（前端纯函数，不进 store）” (heading), the source “### 10.3 派生选择器（前端纯函数，不进 store）” requires this exact behavior: UI-only state and pure derived selectors remain separate from the Rust timeline projection.
- Resolution: `reviewed-mapping-report:ui-rust-state-boundary` — Both slices own the Rust-projected, version/epoch ordered frontend mirror boundary through web/src/store/sync.ts; keep one capability and union the pure-selector plus race evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/10-state.md:92; signal=heading; heading=10.3 派生选择器（前端纯函数，不进 store）; candidate=### 10.3 派生选择器（前端纯函数，不进 store）
  - Expected behavior: UI-only state and pure derived selectors remain separate from the Rust timeline projection. This closes only the promise expressed by “10.3 派生选择器（前端纯函数，不进 store）” in “10.3 派生选择器（前端纯函数，不进 store）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “10.3 派生选择器（前端纯函数，不进 store）” with the scenario below and register test:web/src/__tests__/completion/doc-7bcd544ecb82bee0.test.ts#completion_7bcd544ecb82bee0_ui_only_state_and_pure_derived_selectors_remain_
  - Initial state/input/event: render the exact “10.3 派生选择器（前端纯函数，不进 store）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “10.3 派生选择器（前端纯函数，不进 store）”.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, have the React handler call the typed store/API/Tauri action for “UI-only state and pure derived selectors remain separate from the Rust timeline projection.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-7bcd544ecb82bee0.test.ts#completion_7bcd544ecb82bee0_ui_only_state_and_pure_derived_selectors_remain_.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/store/sync.test.ts#does not let a late old snapshot replace a newer project` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/store/sync.test.ts#does not let pending history cross a same-identity compatibility replacement` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/store/uiStore.boundary.test.ts#derived_selectors_do_not_mutate_timeline_projection` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/store/uiStore.test.ts#timeline playback state` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/store/sync.test.ts -t "does not let a late old snapshot replace a newer project"`
  - Run: `pnpm -C web test -- --run src/store/sync.test.ts -t "does not let pending history cross a same-identity compatibility replacement"`
  - Run: `pnpm -C web test -- --run src/store/uiStore.boundary.test.ts -t "derived_selectors_do_not_mutate_timeline_projection"`
  - Run: `pnpm -C web test -- --run src/store/uiStore.test.ts -t "timeline playback state"`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/commands.rs#get_timeline`, `web/src/store/projectStore.ts#replaceProjectSnapshot`, `web/src/store/sync.ts#startSync`, `web/src/store/uiStore.ts#useEditorUiStore`, `docs/specs/core/4-frontend-sync.md`, `docs/specs/frontend/10-state.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/store/sync.test.ts -t "does not let a late old snapshot replace a newer project"`
  - Run: `pnpm -C web test -- --run src/store/sync.test.ts -t "does not let pending history cross a same-identity compatibility replacement"`
  - Run: `pnpm -C web test -- --run src/store/uiStore.boundary.test.ts -t "derived_selectors_do_not_mutate_timeline_projection"`
  - Run: `pnpm -C web test -- --run src/store/uiStore.test.ts -t "timeline playback state"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 11: PT-frontend-vertical-acceptance (implementation-slice-45d194b8795207e0)

**Covered records:**
- `requirement-0a83743add78e662` (requirement)
- `requirement-48dccc643ac6d250` (requirement)
- `requirement-76c0b87108cd8353` (requirement)
- `requirement-81a0932850a78412` (requirement)

**Files:**
- Modify: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`
- Modify: `web/src/components/preview/Preview.tsx#Preview`
- Modify: `web/src/components/inspector/Inspector.tsx#Inspector`
- Modify: `docs/specs/frontend/13-implementation.md`
- Modify: `docs/specs/frontend/5-timeline.md`
- Test (reviewed-planned): `web/src/components/frontendAcceptance.test.tsx#implementation_visual_and_interaction_evidence_for_every_frontend_check`

**Candidate-bound contracts:**

#### requirement-0a83743add78e662

- Candidate/source: `doc-d9b273ee27550d87` at `docs/specs/frontend/13-implementation.md:1` (requirement)
- Expected behavior: Every item in the 1:1 frontend acceptance checklist has implementation, visual, and interaction evidence.
- Resolution: `validated-ledger-evidence:PT-frontend-vertical-acceptance` — Whole-frontend/timeline umbrellas must aggregate child capability evidence, not one semantic owner.
- Exact acceptance contract:
  - Link every layout, toolbar, timeline, Inspector, MediaPanel, Preview, state, Tauri, and interaction acceptance row to a current component/action and test.
  - Resolve every unchecked row without bypassing authoritative state or leaving disabled/scaffold UI for advertised features.
  - Run typecheck/unit/component/visual suites and packaged desktop create-import-edit-preview-save-reopen-export smoke with zero unchecked rows.

#### requirement-48dccc643ac6d250

- Candidate/source: `doc-dfcc87fff041f2e7` at `docs/specs/frontend/13-implementation.md:3` (requirement)
- Expected behavior: Every item in the 1:1 frontend acceptance checklist has implementation, visual, and interaction evidence.
- Resolution: `validated-ledger-evidence:PT-frontend-vertical-acceptance` — Whole-frontend/timeline umbrellas must aggregate child capability evidence, not one semantic owner.
- Exact acceptance contract:
  - Implement in dependency order: tokens/state/typed API → shell/layout → timeline/preview → Inspector/MediaPanel → exhaustive interactions and polish.
  - Each stage must keep build/typecheck green and land command/undo/state tests before dependent visual components.
  - Machine-check that no later-stage component imports a missing placeholder API and that each stage's documented test gate passes before the next.

#### requirement-76c0b87108cd8353

- Candidate/source: `doc-c3417fcf23a48a7f` at `docs/specs/frontend/13-implementation.md:17` (requirement)
- Expected behavior: Every item in the 1:1 frontend acceptance checklist has implementation, visual, and interaction evidence.
- Resolution: `validated-ledger-evidence:PT-frontend-vertical-acceptance` — Whole-frontend/timeline umbrellas must aggregate child capability evidence, not one semantic owner.
- Exact acceptance contract:
  - Convert every checkbox into an automated or explicitly recorded packaged-desktop assertion with source/test links and expected result.
  - Cover fixed window sizes, DPR 1/2, mouse/trackpad/keyboard, light/dark/high-contrast, empty/loaded/offline/generating states, and undo/save/reopen.
  - The acceptance report must contain zero unchecked or evidence-free rows and no visual diff above the repository threshold.

#### requirement-81a0932850a78412

- Candidate/source: `doc-4cf10dd284589451` at `docs/specs/frontend/5-timeline.md:1` (requirement)
- Expected behavior: Complete the exact Timeline（时间线）—— 核心 behavior from the upstream interaction specification.
- Resolution: `validated-ledger-evidence:PT-frontend-vertical-acceptance` — Whole-frontend/timeline umbrellas must aggregate child capability evidence, not one semantic owner.
- Exact acceptance contract:
  - Complete and link acceptance evidence for geometry, clip drawing, headers, gestures, overlays, context menus, and external/media drag-drop.
  - All mutations must use editActions/Tauri commands and support the specified undo/redo and selection semantics.
  - Run the full timeline unit/component suite plus DPR 1 and DPR 2 visual fixtures at minimum/default/maximum zoom with no unchecked §5 rows.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/frontendAcceptance.test.tsx#implementation_visual_and_interaction_evidence_for_every_frontend_check` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/frontendAcceptance.test.tsx -t "implementation_visual_and_interaction_evidence_for_every_frontend_check"`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`, `web/src/components/preview/Preview.tsx#Preview`, `web/src/components/inspector/Inspector.tsx#Inspector`, `docs/specs/frontend/13-implementation.md`, `docs/specs/frontend/5-timeline.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/frontendAcceptance.test.tsx -t "implementation_visual_and_interaction_evidence_for_every_frontend_check"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 12: preview-playback-controls-overlays (implementation-slice-f03397ff8d00113a)

**Covered records:**
- `requirement-dbd3f79b7e0c2f11` (requirement)
- `requirement-64629892b7fae65d` (requirement)
- `requirement-6a6a5fd7ad27f724` (requirement)
- `requirement-a81cc5996343e86d` (requirement)
- `requirement-aa1d38d1befcc428` (requirement)
- `requirement-087ed63292e24e0b` (requirement)
- `requirement-685b894531563174` (requirement)

**Files:**
- Modify: `web/src/components/preview/Preview.tsx#Preview`
- Modify: `web/src/components/preview/TimelinePlaybackLayer.tsx#TimelinePlayback`
- Modify: `web/src/components/preview/TransformOverlay.tsx#TransformOverlay`
- Modify: `web/src/components/preview/CropOverlay.tsx#CropOverlay`
- Modify: `docs/specs/frontend/13-implementation.md`
- Modify: `docs/specs/frontend/8-preview.md`
- Test (existing-owned): `web/src/components/preview/Preview.test.tsx#Preview timeline rendering`
- Test (existing-owned): `web/src/components/preview/TransformOverlay.test.tsx#TransformOverlay`
- Test (existing-owned): `web/src/components/preview/timelinePlayback.test.ts#sourceTimeSec / frameForSourceTime`
- Test (reviewed-planned): `web/src/components/preview/Preview.controls.test.tsx#tabs_transport_scrub_settings_crop_complete_matrix`

**Candidate-bound contracts:**

#### requirement-dbd3f79b7e0c2f11

- Candidate/source: `doc-96b0748ddf50b684` at `docs/specs/frontend/13-implementation.md:26` (requirement)
- Expected behavior: Match Preview tabs, transport, scrub, settings badge, and overlays.
- Resolution: `reviewed-mapping-report:preview-playback-controls-overlays` — Preview imports and mounts the playback and overlay owners, but existing tests emphasize routing and transform behavior rather than the complete control surface.
- Exact acceptance contract:
  - Implementation: Implement/verify every listed preview state across paused/playing/scrubbing/unsupported routes and add golden plus transport interaction tests.
  - Add exact geometry, hit-test, command-payload, interaction, boundary, high-DPI, and visual assertions for every named timeline behavior; the affected web suites must pass.
  - Exercise the named pointer, keyboard, playback, or canvas path in a real browser or packaged app and retain exact runtime or golden evidence before reclassification.

#### requirement-64629892b7fae65d

- Candidate/source: `doc-16da74fadae787ab` at `docs/specs/frontend/8-preview.md:5` (requirement)
- Expected behavior: At docs/specs/frontend/8-preview.md:5 under “8.1 结构（`PreviewContainerView.swift:12-60`）” (heading), the source “### 8.1 结构（`PreviewContainerView.swift:12-60`）” requires this exact behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.
- Resolution: `reviewed-mapping-report:preview-playback-controls-overlays` — Preview imports and mounts the playback and overlay owners, but existing tests emphasize routing and transform behavior rather than the complete control surface.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/8-preview.md:5; signal=heading; heading=8.1 结构（`PreviewContainerView.swift:12-60`）; candidate=### 8.1 结构（`PreviewContainerView.swift:12-60`）
  - Expected behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions. This closes only the promise expressed by “8.1 结构（`PreviewContainerView.swift:12-60`）” in “8.1 结构（`PreviewContainerView.swift:12-60`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.1 结构（`PreviewContainerView.swift:12-60`）” with the scenario below and register test:web/src/__tests__/completion/doc-16da74fadae787ab.test.ts#completion_16da74fadae787ab_the_named_preview_structure_control_or_transform
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “8.1 结构（`PreviewContainerView.swift:12-60`）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-16da74fadae787ab.test.ts#completion_16da74fadae787ab_the_named_preview_structure_control_or_transform.

#### requirement-6a6a5fd7ad27f724

- Candidate/source: `doc-b41fb3ddfedef110` at `docs/specs/frontend/8-preview.md:13` (requirement)
- Expected behavior: At docs/specs/frontend/8-preview.md:13 under “8.2 画布区（`18-51`）” (heading), the source “### 8.2 画布区（`18-51`）” requires this exact behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.
- Resolution: `reviewed-mapping-report:preview-playback-controls-overlays` — Preview imports and mounts the playback and overlay owners, but existing tests emphasize routing and transform behavior rather than the complete control surface.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/8-preview.md:13; signal=heading; heading=8.2 画布区（`18-51`）; candidate=### 8.2 画布区（`18-51`）
  - Expected behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions. This closes only the promise expressed by “8.2 画布区（`18-51`）” in “8.2 画布区（`18-51`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.2 画布区（`18-51`）” with the scenario below and register test:web/src/__tests__/completion/doc-b41fb3ddfedef110.test.ts#completion_b41fb3ddfedef110_the_named_preview_structure_control_or_transform
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “8.2 画布区（`18-51`）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-b41fb3ddfedef110.test.ts#completion_b41fb3ddfedef110_the_named_preview_structure_control_or_transform.

#### requirement-a81cc5996343e86d

- Candidate/source: `doc-82ee3e322fd5bb28` at `docs/specs/frontend/8-preview.md:23` (requirement)
- Expected behavior: At docs/specs/frontend/8-preview.md:23 under “8.3 tab 栏（`tabBar` `529-559`）” (heading), the source “### 8.3 tab 栏（`tabBar` `529-559`）” requires this exact behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.
- Resolution: `reviewed-mapping-report:preview-playback-controls-overlays` — Preview imports and mounts the playback and overlay owners, but existing tests emphasize routing and transform behavior rather than the complete control surface.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/8-preview.md:23; signal=heading; heading=8.3 tab 栏（`tabBar` `529-559`）; candidate=### 8.3 tab 栏（`tabBar` `529-559`）
  - Expected behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions. This closes only the promise expressed by “8.3 tab 栏（`tabBar` `529-559`）” in “8.3 tab 栏（`tabBar` `529-559`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.3 tab 栏（`tabBar` `529-559`）” with the scenario below and register test:web/src/__tests__/completion/doc-82ee3e322fd5bb28.test.ts#completion_82ee3e322fd5bb28_the_named_preview_structure_control_or_transform
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “8.3 tab 栏（`tabBar` `529-559`）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-82ee3e322fd5bb28.test.ts#completion_82ee3e322fd5bb28_the_named_preview_structure_control_or_transform.

#### requirement-aa1d38d1befcc428

- Candidate/source: `doc-ccf8d7c0171df9af` at `docs/specs/frontend/8-preview.md:30` (requirement)
- Expected behavior: At docs/specs/frontend/8-preview.md:30 under “8.4 transport 条（`transportBar` `64-101`，高 36）” (heading), the source “### 8.4 transport 条（`transportBar` `64-101`，高 36）” requires this exact behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.
- Resolution: `reviewed-mapping-report:preview-playback-controls-overlays` — Preview imports and mounts the playback and overlay owners, but existing tests emphasize routing and transform behavior rather than the complete control surface.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/8-preview.md:30; signal=heading; heading=8.4 transport 条（`transportBar` `64-101`，高 36）; candidate=### 8.4 transport 条（`transportBar` `64-101`，高 36）
  - Expected behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions. This closes only the promise expressed by “8.4 transport 条（`transportBar` `64-101`，高 36）” in “8.4 transport 条（`transportBar` `64-101`，高 36）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.4 transport 条（`transportBar` `64-101`，高 36）” with the scenario below and register test:web/src/__tests__/completion/doc-ccf8d7c0171df9af.test.ts#completion_ccf8d7c0171df9af_the_named_preview_structure_control_or_transform
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “8.4 transport 条（`transportBar` `64-101`，高 36）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-ccf8d7c0171df9af.test.ts#completion_ccf8d7c0171df9af_the_named_preview_structure_control_or_transform.

#### requirement-087ed63292e24e0b

- Candidate/source: `doc-d32884ffe67a72ec` at `docs/specs/frontend/8-preview.md:43` (requirement)
- Expected behavior: At docs/specs/frontend/8-preview.md:43 under “8.5 scrub 条（`scrubBar` `651-...`）” (heading), the source “### 8.5 scrub 条（`scrubBar` `651-...`）” requires this exact behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.
- Resolution: `reviewed-mapping-report:preview-playback-controls-overlays` — Preview imports and mounts the playback and overlay owners, but existing tests emphasize routing and transform behavior rather than the complete control surface.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/8-preview.md:43; signal=heading; heading=8.5 scrub 条（`scrubBar` `651-...`）; candidate=### 8.5 scrub 条（`scrubBar` `651-...`）
  - Expected behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions. This closes only the promise expressed by “8.5 scrub 条（`scrubBar` `651-...`）” in “8.5 scrub 条（`scrubBar` `651-...`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.5 scrub 条（`scrubBar` `651-...`）” with the scenario below and register test:web/src/__tests__/completion/doc-d32884ffe67a72ec.test.ts#completion_d32884ffe67a72ec_the_named_preview_structure_control_or_transform
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “8.5 scrub 条（`scrubBar` `651-...`）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-d32884ffe67a72ec.test.ts#completion_d32884ffe67a72ec_the_named_preview_structure_control_or_transform.

#### requirement-685b894531563174

- Candidate/source: `doc-d6822f322c27038b` at `docs/specs/frontend/8-preview.md:49` (requirement)
- Expected behavior: At docs/specs/frontend/8-preview.md:49 under “8.6 Transform / Crop 叠加（`TransformOverlayView.swift` / `CropOverlayView.swift`）” (heading), the source “### 8.6 Transform / Crop 叠加（`TransformOverlayView.swift` / `CropOverlayView.swift`）” requires this exact behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.
- Resolution: `reviewed-mapping-report:preview-playback-controls-overlays` — Preview imports and mounts the playback and overlay owners, but existing tests emphasize routing and transform behavior rather than the complete control surface.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/8-preview.md:49; signal=heading; heading=8.6 Transform / Crop 叠加（`TransformOverlayView.swift` / `CropOverlayView.swift`）; candidate=### 8.6 Transform / Crop 叠加（`TransformOverlayView.swift` / `CropOverlayView.swift`）
  - Expected behavior: The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions. This closes only the promise expressed by “8.6 Transform / Crop 叠加（`TransformOverlayView.swift` / `CropOverlayView.swift`）” in “8.6 Transform / Crop 叠加（`TransformOverlayView.swift` / `CropOverlayView.swift`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.6 Transform / Crop 叠加（`TransformOverlayView.swift` / `CropOverlayView.swift`）” with the scenario below and register test:web/src/__tests__/completion/doc-d6822f322c27038b.test.ts#completion_d6822f322c27038b_the_named_preview_structure_control_or_transform
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “8.6 Transform / Crop 叠加（`TransformOverlayView.swift` / `CropOverlayView.swift`）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named preview structure, control, or transform/crop overlay is implemented with routed playback/edit actions.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-d6822f322c27038b.test.ts#completion_d6822f322c27038b_the_named_preview_structure_control_or_transform.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/Preview.test.tsx#Preview timeline rendering` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/TransformOverlay.test.tsx#TransformOverlay` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/timelinePlayback.test.ts#sourceTimeSec / frameForSourceTime` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/Preview.controls.test.tsx#tabs_transport_scrub_settings_crop_complete_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.test.tsx -t "Preview timeline rendering"`
  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.test.tsx -t "TransformOverlay"`
  - Run: `pnpm -C web test -- --run src/components/preview/timelinePlayback.test.ts -t "sourceTimeSec / frameForSourceTime"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.controls.test.tsx -t "tabs_transport_scrub_settings_crop_complete_matrix"`

  Expected: FAIL because one or more of the 7 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/Preview.tsx#Preview`, `web/src/components/preview/TimelinePlaybackLayer.tsx#TimelinePlayback`, `web/src/components/preview/TransformOverlay.tsx#TransformOverlay`, `web/src/components/preview/CropOverlay.tsx#CropOverlay`, `docs/specs/frontend/13-implementation.md`, `docs/specs/frontend/8-preview.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.test.tsx -t "Preview timeline rendering"`
  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.test.tsx -t "TransformOverlay"`
  - Run: `pnpm -C web test -- --run src/components/preview/timelinePlayback.test.ts -t "sourceTimeSec / frameForSourceTime"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.controls.test.tsx -t "tabs_transport_scrub_settings_crop_complete_matrix"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 13: control-acceptance (implementation-slice-e8e759d2be27e50c)

**Covered records:**
- `control-record-31e21adc33d1e3b0` (control)
- `control-record-dc4c413ea51e1e01` (control)

**Files:**
- Modify: `web/src/components/preview/CropOverlay.tsx`
- Modify: `web/src/store/editActions.ts#upsertKeyframe`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::UpsertKeyframe`
- Modify: `web/src/components/preview/CropOverlay.tsx#CropOverlay`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/preview/CropOverlay.interaction.test.tsx#control-edf6733951275239 pan crop rectangle`
- Test (reviewed-planned): `web/src/components/preview/CropOverlay.interaction.test.tsx#control-1ca8aa06b3fee95e resize crop rectangle from four corners`

**Candidate-bound contracts:**

#### control-record-31e21adc33d1e3b0

- Candidate/source: `control-edf6733951275239` at `web/src/components/preview/CropOverlay.tsx:292:7` (control)
- Expected behavior: pan crop rectangle: assert exactly handlePanDown begins drag using pannedCrop(restCrop, rotatedPointerDelta, clipRectPx); pointerup commits exactly one edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }) when cropAnimated, otherwise edit.setClipProperties([clip.id], { crop: next }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-edf6733951275239.
  - Test: web/src/components/preview/CropOverlay.interaction.test.tsx#control-edf6733951275239 pan crop rectangle.
  - Initial state: visibility=Visible only during on-canvas crop editing for one supported selected visual clip.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["pointer/drag event sequence","coordinates and modifiers","current editor state","pointerup/cancel boundary"]; handler={handlePanDown}.
  - Exact call/state/backend: stateTransition=pan crop rectangle: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/CropOverlay.tsx:292::handler {handlePanDown} -> handlePanDown begins drag using pannedCrop(restCrop, rotatedPointerDelta, clipRectPx); pointerup commits exactly one edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }) when cropAnimated, otherwise edit.setClipProperties([clip.id], { crop: next })","web/src/store/editActions.ts::upsertKeyframe('crop') or setClipProperties({crop}) after pan","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::UpsertKeyframe or SetClipProperties","code:web/src/components/preview/CropOverlay.tsx#CropOverlay","code:web/src/store/editActions.ts#upsertKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=pan crop rectangle: assert exactly handlePanDown begins drag using pannedCrop(restCrop, rotatedPointerDelta, clipRectPx); pointerup commits exactly one edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }) when cropAnimated, otherwise edit.setClipProperties([clip.id], { crop: next }) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Pointerup/cancel releases the gesture and remains on the same editor surface.","A keyboard equivalent must retain/restore focus; current custom drag surface does not prove one."].
  - Outcome matrix: {"success":"pan crop rectangle: assert exactly handlePanDown begins drag using pannedCrop(restCrop, rotatedPointerDelta, clipRectPx); pointerup commits exactly one edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }) when cropAnimated, otherwise edit.setClipProperties([clip.id], { crop: next }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Pointerup/cancel follows the exact profile; Escape rollback is not otherwise implemented.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-dc4c413ea51e1e01

- Candidate/source: `control-1ca8aa06b3fee95e` at `web/src/components/preview/CropOverlay.tsx:308:11` (control)
- Expected behavior: resize crop rectangle from four corners: assert exactly handleResizeDown(e, corner) begins drag using resizedCrop(restCrop, corner, rotatedPointerDelta, clipRectPx, lockedAspect); pointerup commits exactly one edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }) when cropAnimated, otherwise edit.setClipProperties([clip.id], { crop: next }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-1ca8aa06b3fee95e.
  - Test: web/src/components/preview/CropOverlay.interaction.test.tsx#control-1ca8aa06b3fee95e resize crop rectangle from four corners.
  - Initial state: visibility=Visible only during on-canvas crop editing for one supported selected visual clip.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["pointer/drag event sequence","coordinates and modifiers","current editor state","pointerup/cancel boundary"]; handler={(e) => handleResizeDown(e, corner)}.
  - Exact call/state/backend: stateTransition=resize crop rectangle from four corners: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/CropOverlay.tsx:308::handler {(e) => handleResizeDown(e, corner)} -> handleResizeDown(e, corner) begins drag using resizedCrop(restCrop, corner, rotatedPointerDelta, clipRectPx, lockedAspect); pointerup commits exactly one edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }) when cropAnimated, otherwise edit.setClipProperties([clip.id], { crop: next })","web/src/store/editActions.ts::upsertKeyframe('crop') or setClipProperties({crop}) after corner resize","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::UpsertKeyframe or SetClipProperties","code:web/src/components/preview/CropOverlay.tsx#CropOverlay","code:web/src/store/editActions.ts#upsertKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=resize crop rectangle from four corners: assert exactly handleResizeDown(e, corner) begins drag using resizedCrop(restCrop, corner, rotatedPointerDelta, clipRectPx, lockedAspect); pointerup commits exactly one edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }) when cropAnimated, otherwise edit.setClipProperties([clip.id], { crop: next }) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Pointerup/cancel releases the gesture and remains on the same editor surface.","A keyboard equivalent must retain/restore focus; current custom drag surface does not prove one."].
  - Outcome matrix: {"success":"resize crop rectangle from four corners: assert exactly handleResizeDown(e, corner) begins drag using resizedCrop(restCrop, corner, rotatedPointerDelta, clipRectPx, lockedAspect); pointerup commits exactly one edit.upsertKeyframe(clip.id, 'crop', activeFrame, { kind: 'crop', value: next }) when cropAnimated, otherwise edit.setClipProperties([clip.id], { crop: next }) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Pointerup/cancel follows the exact profile; Escape rollback is not otherwise implemented.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/CropOverlay.interaction.test.tsx#control-edf6733951275239 pan crop rectangle` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/preview/CropOverlay.interaction.test.tsx#control-1ca8aa06b3fee95e resize crop rectangle from four corners` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/CropOverlay.interaction.test.tsx -t "control-edf6733951275239 pan crop rectangle"`
  - Run: `pnpm -C web test -- --run src/components/preview/CropOverlay.interaction.test.tsx -t "control-1ca8aa06b3fee95e resize crop rectangle from four corners"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/CropOverlay.tsx`, `web/src/store/editActions.ts#upsertKeyframe`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::UpsertKeyframe`, `web/src/components/preview/CropOverlay.tsx#CropOverlay`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/CropOverlay.interaction.test.tsx -t "control-edf6733951275239 pan crop rectangle"`
  - Run: `pnpm -C web test -- --run src/components/preview/CropOverlay.interaction.test.tsx -t "control-1ca8aa06b3fee95e resize crop rectangle from four corners"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 14: control-acceptance (implementation-slice-bdfdef755d74ba71)

**Covered records:**
- `control-record-9003925a1c0b11ef` (control)
- `control-record-3a269b82dd610006` (control)
- `control-record-8e2a99909935ea49` (control)
- `control-record-ab0ff38c81e40b21` (control)
- `control-record-f27e30b6d14c4aab` (control)

**Files:**
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/preview/Preview.tsx#Preview`
- Test (reviewed-planned): `web/src/components/preview/Preview.interaction.test.tsx#control-575921b92e2f3135 jump preview to start`
- Test (reviewed-planned): `web/src/components/preview/Preview.interaction.test.tsx#control-baaf4052cb5bdc8e step preview back one frame`
- Test (reviewed-planned): `web/src/components/preview/Preview.interaction.test.tsx#control-37ea2cd22696fd55 play or pause preview`
- Test (reviewed-planned): `web/src/components/preview/Preview.interaction.test.tsx#control-c6f5ed51aa2b00c2 step preview forward one frame`
- Test (reviewed-planned): `web/src/components/preview/Preview.interaction.test.tsx#control-9812bb3e84b940c5 jump preview to end`

**Candidate-bound contracts:**

#### control-record-9003925a1c0b11ef

- Candidate/source: `control-575921b92e2f3135` at `web/src/components/preview/Preview.tsx:492:11` (control)
- Expected behavior: jump preview to start: assert exactly seekTo(0): clamp to 0; media preview sets mediaRef.current.currentTime = 0/fps, timeline preview calls setCurrentFrame(snapFrameToEdge(timeline,0,threshold).frame) and maybeSnapFeedback and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-575921b92e2f3135.
  - Test: web/src/components/preview/Preview.interaction.test.tsx#control-575921b92e2f3135 jump preview to start.
  - Initial state: visibility=Visible on the Preview surface; content and playback capability determine actionable branches.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => seekTo(0)}.
  - Exact call/state/backend: stateTransition=jump preview to start: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/Preview.tsx:492::handler {() => seekTo(0)} -> seekTo(0): clamp to 0; media preview sets mediaRef.current.currentTime = 0/fps, timeline preview calls setCurrentFrame(snapFrameToEdge(timeline,0,threshold).frame) and maybeSnapFeedback","web/src/components/preview/Preview.tsx::Preview.seekTo(0) -> media element or uiStore.setCurrentFrame","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/preview/Preview.tsx#Preview"].
  - Visible/accessibility/return path: success=jump preview to start: assert exactly seekTo(0): clamp to 0; media preview sets mediaRef.current.currentTime = 0/fps, timeline preview calls setCurrentFrame(snapFrameToEdge(timeline,0,threshold).frame) and maybeSnapFeedback and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"preview.jumpStart\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"jump preview to start: assert exactly seekTo(0): clamp to 0; media preview sets mediaRef.current.currentTime = 0/fps, timeline preview calls setCurrentFrame(snapFrameToEdge(timeline,0,threshold).frame) and maybeSnapFeedback and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

#### control-record-3a269b82dd610006

- Candidate/source: `control-baaf4052cb5bdc8e` at `web/src/components/preview/Preview.tsx:495:11` (control)
- Expected behavior: step preview back one frame: assert exactly seekTo(activeShownFrame - 1): clamp; media preview sets currentTime = clamped/fps, timeline preview sets snapped current frame and maybeSnapFeedback and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-baaf4052cb5bdc8e.
  - Test: web/src/components/preview/Preview.interaction.test.tsx#control-baaf4052cb5bdc8e step preview back one frame.
  - Initial state: visibility=Visible on the Preview surface; content and playback capability determine actionable branches.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => seekTo(activeShownFrame - 1)}.
  - Exact call/state/backend: stateTransition=step preview back one frame: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/Preview.tsx:495::handler {() => seekTo(activeShownFrame - 1)} -> seekTo(activeShownFrame - 1): clamp; media preview sets currentTime = clamped/fps, timeline preview sets snapped current frame and maybeSnapFeedback","web/src/components/preview/Preview.tsx::Preview.seekTo(activeShownFrame - 1) -> media element or uiStore.setCurrentFrame","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/preview/Preview.tsx#Preview"].
  - Visible/accessibility/return path: success=step preview back one frame: assert exactly seekTo(activeShownFrame - 1): clamp; media preview sets currentTime = clamped/fps, timeline preview sets snapped current frame and maybeSnapFeedback and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"preview.stepBack\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"step preview back one frame: assert exactly seekTo(activeShownFrame - 1): clamp; media preview sets currentTime = clamped/fps, timeline preview sets snapped current frame and maybeSnapFeedback and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

#### control-record-8e2a99909935ea49

- Candidate/source: `control-37ea2cd22696fd55` at `web/src/components/preview/Preview.tsx:498:11` (control)
- Expected behavior: play or pause preview: assert exactly togglePlay(): media preview calls mediaRef.current.play() when paused or pause() otherwise; timeline preview returns when unsupported, otherwise togglePlayTimeline() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-37ea2cd22696fd55.
  - Test: web/src/components/preview/Preview.interaction.test.tsx#control-37ea2cd22696fd55 play or pause preview.
  - Initial state: visibility=Visible on the Preview surface; content and playback capability determine actionable branches.; enabledWhen=Enabled when !previewing && !timelinePlaybackAllowed is false..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={togglePlay}.
  - Exact call/state/backend: stateTransition=play or pause preview: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/Preview.tsx:498::handler {togglePlay} -> togglePlay(): media preview calls mediaRef.current.play() when paused or pause() otherwise; timeline preview returns when unsupported, otherwise togglePlayTimeline()","web/src/components/preview/Preview.tsx::Preview.togglePlay -> HTMLMediaElement or uiStore.togglePlay","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/preview/Preview.tsx#Preview"].
  - Visible/accessibility/return path: success=play or pause preview: assert exactly togglePlay(): media preview calls mediaRef.current.play() when paused or pause() otherwise; timeline preview returns when unsupported, otherwise togglePlayTimeline() and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"preview.playPause\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"play or pause preview: assert exactly togglePlay(): media preview calls mediaRef.current.play() when paused or pause() otherwise; timeline preview returns when unsupported, otherwise togglePlayTimeline() and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"The rendered control is disabled by {!previewing && !timelinePlaybackAllowed}.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"HTMLMediaElement.play() rejection is not caught or shown; timeline toggle is synchronous local state."}.

#### control-record-ab0ff38c81e40b21

- Candidate/source: `control-c6f5ed51aa2b00c2` at `web/src/components/preview/Preview.tsx:505:11` (control)
- Expected behavior: step preview forward one frame: assert exactly seekTo(activeShownFrame + 1): clamp; media preview sets currentTime = clamped/fps, timeline preview sets snapped current frame and maybeSnapFeedback and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c6f5ed51aa2b00c2.
  - Test: web/src/components/preview/Preview.interaction.test.tsx#control-c6f5ed51aa2b00c2 step preview forward one frame.
  - Initial state: visibility=Visible on the Preview surface; content and playback capability determine actionable branches.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => seekTo(activeShownFrame + 1)}.
  - Exact call/state/backend: stateTransition=step preview forward one frame: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/Preview.tsx:505::handler {() => seekTo(activeShownFrame + 1)} -> seekTo(activeShownFrame + 1): clamp; media preview sets currentTime = clamped/fps, timeline preview sets snapped current frame and maybeSnapFeedback","web/src/components/preview/Preview.tsx::Preview.seekTo(activeShownFrame + 1) -> media element or uiStore.setCurrentFrame","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/preview/Preview.tsx#Preview"].
  - Visible/accessibility/return path: success=step preview forward one frame: assert exactly seekTo(activeShownFrame + 1): clamp; media preview sets currentTime = clamped/fps, timeline preview sets snapped current frame and maybeSnapFeedback and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"preview.stepForward\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"step preview forward one frame: assert exactly seekTo(activeShownFrame + 1): clamp; media preview sets currentTime = clamped/fps, timeline preview sets snapped current frame and maybeSnapFeedback and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

#### control-record-f27e30b6d14c4aab

- Candidate/source: `control-9812bb3e84b940c5` at `web/src/components/preview/Preview.tsx:508:11` (control)
- Expected behavior: jump preview to end: assert exactly seekTo(total): clamp to total; media preview sets currentTime = total/fps, timeline preview sets snapped current frame and maybeSnapFeedback and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-9812bb3e84b940c5.
  - Test: web/src/components/preview/Preview.interaction.test.tsx#control-9812bb3e84b940c5 jump preview to end.
  - Initial state: visibility=Visible on the Preview surface; content and playback capability determine actionable branches.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => seekTo(total)}.
  - Exact call/state/backend: stateTransition=jump preview to end: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/Preview.tsx:508::handler {() => seekTo(total)} -> seekTo(total): clamp to total; media preview sets currentTime = total/fps, timeline preview sets snapped current frame and maybeSnapFeedback","web/src/components/preview/Preview.tsx::Preview.seekTo(total) -> media element or uiStore.setCurrentFrame","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/preview/Preview.tsx#Preview"].
  - Visible/accessibility/return path: success=jump preview to end: assert exactly seekTo(total): clamp to total; media preview sets currentTime = total/fps, timeline preview sets snapped current frame and maybeSnapFeedback and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"preview.jumpEnd\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"jump preview to end: assert exactly seekTo(total): clamp to total; media preview sets currentTime = total/fps, timeline preview sets snapped current frame and maybeSnapFeedback and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/Preview.interaction.test.tsx#control-575921b92e2f3135 jump preview to start` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/preview/Preview.interaction.test.tsx#control-baaf4052cb5bdc8e step preview back one frame` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/preview/Preview.interaction.test.tsx#control-37ea2cd22696fd55 play or pause preview` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/preview/Preview.interaction.test.tsx#control-c6f5ed51aa2b00c2 step preview forward one frame` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/preview/Preview.interaction.test.tsx#control-9812bb3e84b940c5 jump preview to end` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-575921b92e2f3135 jump preview to start"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-baaf4052cb5bdc8e step preview back one frame"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-37ea2cd22696fd55 play or pause preview"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-c6f5ed51aa2b00c2 step preview forward one frame"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-9812bb3e84b940c5 jump preview to end"`

  Expected: FAIL because one or more of the 5 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/Preview.tsx`, `web/src/components/preview/Preview.tsx#Preview` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-575921b92e2f3135 jump preview to start"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-baaf4052cb5bdc8e step preview back one frame"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-37ea2cd22696fd55 play or pause preview"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-c6f5ed51aa2b00c2 step preview forward one frame"`
  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-9812bb3e84b940c5 jump preview to end"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 15: control-acceptance (implementation-slice-5c4f08d4c681f056)

**Covered records:**
- `control-record-c7001a3dbcec3ed4` (control)

**Files:**
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/preview/Preview.tsx#captureFrame`
- Modify: `web/src/store/editActions.ts#captureFrameToMedia`
- Modify: `web/src/lib/api.ts#captureFrameToMedia`
- Modify: `src-tauri/src/render.rs#capture_frame_to_media`
- Modify: `web/src/components/preview/Preview.tsx#Preview`
- Test (reviewed-planned): `web/src/components/preview/Preview.interaction.test.tsx#control-d3805a1e5b0f0237 capture current frame to media library`

**Candidate-bound contracts:**

#### control-record-c7001a3dbcec3ed4

- Candidate/source: `control-d3805a1e5b0f0237` at `web/src/components/preview/Preview.tsx:513:9` (control)
- Expected behavior: capture current frame to media library: assert exactly if (!canCapture) no-op; else await captureFrameToMedia(frame, nameBase, mediaPanelCurrentFolderId, sourceMediaId); null result shows unavailable toast; success awaits refreshMedia() then saved toast; catch shows failed toast and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d3805a1e5b0f0237.
  - Test: web/src/components/preview/Preview.interaction.test.tsx#control-d3805a1e5b0f0237 capture current frame to media library.
  - Initial state: visibility=Visible on the Preview surface; content and playback capability determine actionable branches.; enabledWhen=Enabled when !canCapture is false..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => void captureFrame()}.
  - Exact call/state/backend: stateTransition=capture current frame to media library: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/Preview.tsx:513::handler {() => void captureFrame()} -> if (!canCapture) no-op; else await captureFrameToMedia(frame, nameBase, mediaPanelCurrentFolderId, sourceMediaId); null result shows unavailable toast; success awaits refreshMedia() then saved toast; catch shows failed toast","web/src/components/preview/Preview.tsx::captureFrame","web/src/store/editActions.ts::captureFrameToMedia","web/src/lib/api.ts::captureFrameToMedia -> invoke('capture_frame_to_media', exact frame/name/folder/sourceMediaId)","src-tauri/src/render.rs::capture_frame_to_media","code:web/src/components/preview/Preview.tsx#Preview","code:web/src/components/preview/Preview.tsx#captureFrame","code:web/src/lib/api.ts#captureFrameToMedia","code:src-tauri/src/render.rs#capture_frame_to_media"].
  - Visible/accessibility/return path: success=capture current frame to media library: assert exactly if (!canCapture) no-op; else await captureFrameToMedia(frame, nameBase, mediaPanelCurrentFolderId, sourceMediaId); null result shows unavailable toast; success awaits refreshMedia() then saved toast; catch shows failed toast and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: t(\"preview.captureFrame\").","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"capture current frame to media library: assert exactly if (!canCapture) no-op; else await captureFrameToMedia(frame, nameBase, mediaPanelCurrentFolderId, sourceMediaId); null result shows unavailable toast; success awaits refreshMedia() then saved toast; catch shows failed toast and no sibling branch/command.","pending":"The promise is pending with no explicit busy/disabled progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"The rendered control is disabled by {!canCapture}.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"A null result shows captureFrameUnavailable; a thrown rejection logs a warning and shows captureFrameFailed."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/Preview.interaction.test.tsx#control-d3805a1e5b0f0237 capture current frame to media library` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-d3805a1e5b0f0237 capture current frame to media library"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/Preview.tsx`, `web/src/components/preview/Preview.tsx#captureFrame`, `web/src/store/editActions.ts#captureFrameToMedia`, `web/src/lib/api.ts#captureFrameToMedia`, `src-tauri/src/render.rs#capture_frame_to_media`, `web/src/components/preview/Preview.tsx#Preview` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-d3805a1e5b0f0237 capture current frame to media library"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 16: control-acceptance (implementation-slice-a2f5989eab28936c)

**Covered records:**
- `control-record-71eb7c9b7ae9ed64` (control)

**Files:**
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/preview/Preview.tsx#PreviewTabs`
- Modify: `web/src/components/preview/Preview.tsx#Preview`
- Test (reviewed-planned): `web/src/components/preview/Preview.interaction.test.tsx#control-f87f5291a1a74dcc return to timeline preview tab`

**Candidate-bound contracts:**

#### control-record-71eb7c9b7ae9ed64

- Candidate/source: `control-f87f5291a1a74dcc` at `web/src/components/preview/Preview.tsx:664:7` (control)
- Expected behavior: return to timeline preview tab: assert exactly setPreviewMedia(null) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-f87f5291a1a74dcc.
  - Test: web/src/components/preview/Preview.interaction.test.tsx#control-f87f5291a1a74dcc return to timeline preview tab.
  - Initial state: visibility=Visible on the Preview surface; content and playback capability determine actionable branches.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => setPreviewMedia(null)}.
  - Exact call/state/backend: stateTransition=return to timeline preview tab: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/Preview.tsx:664::handler {() => setPreviewMedia(null)} -> setPreviewMedia(null)","web/src/components/preview/Preview.tsx::PreviewTabs -> uiStore.setPreviewMedia(null)","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/preview/Preview.tsx#Preview","code:web/src/components/preview/Preview.tsx#PreviewTabs"].
  - Visible/accessibility/return path: success=return to timeline preview tab: assert exactly setPreviewMedia(null) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"return to timeline preview tab: assert exactly setPreviewMedia(null) and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/Preview.interaction.test.tsx#control-f87f5291a1a74dcc return to timeline preview tab` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-f87f5291a1a74dcc return to timeline preview tab"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/Preview.tsx`, `web/src/components/preview/Preview.tsx#PreviewTabs`, `web/src/components/preview/Preview.tsx#Preview` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-f87f5291a1a74dcc return to timeline preview tab"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 17: control-acceptance (implementation-slice-21de358fb1b1a767)

**Covered records:**
- `control-record-d29715d091827dcc` (control)

**Files:**
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/preview/Preview.tsx#BadgeMenu`
- Modify: `web/src/components/preview/Preview.tsx#Preview`
- Test (reviewed-planned): `web/src/components/preview/Preview.interaction.test.tsx#control-0d59d3bb27911a7c aspect/quality/zoom badge-menu trigger`

**Candidate-bound contracts:**

#### control-record-d29715d091827dcc

- Candidate/source: `control-0d59d3bb27911a7c` at `web/src/components/preview/Preview.tsx:852:7` (control)
- Expected behavior: aspect/quality/zoom badge-menu trigger: assert exactly setOpen((v) => !v) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-0d59d3bb27911a7c.
  - Test: web/src/components/preview/Preview.interaction.test.tsx#control-0d59d3bb27911a7c aspect/quality/zoom badge-menu trigger.
  - Initial state: visibility=Visible on the Preview surface; content and playback capability determine actionable branches.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => setOpen((v) => !v)}.
  - Exact call/state/backend: stateTransition=aspect/quality/zoom badge-menu trigger: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/Preview.tsx:852::handler {() => setOpen((v) => !v)} -> setOpen((v) => !v)","web/src/components/preview/Preview.tsx::BadgeMenu local open state","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/preview/Preview.tsx#Preview","code:web/src/components/preview/Preview.tsx#BadgeMenu"].
  - Visible/accessibility/return path: success=aspect/quality/zoom badge-menu trigger: assert exactly setOpen((v) => !v) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: ariaLabel.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"aspect/quality/zoom badge-menu trigger: assert exactly setOpen((v) => !v) and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/Preview.interaction.test.tsx#control-0d59d3bb27911a7c aspect/quality/zoom badge-menu trigger` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-0d59d3bb27911a7c aspect/quality/zoom badge-menu trigger"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/Preview.tsx`, `web/src/components/preview/Preview.tsx#BadgeMenu`, `web/src/components/preview/Preview.tsx#Preview` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-0d59d3bb27911a7c aspect/quality/zoom badge-menu trigger"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 18: control-acceptance (implementation-slice-26e28a1d03c88aa5)

**Covered records:**
- `control-record-66d48b693d546924` (control)

**Files:**
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/preview/Preview.tsx#BadgeMenu`
- Modify: `web/src/store/editActions.ts#setTimelineSettings`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetTimelineSettings`
- Modify: `web/src/store/uiStore.ts#setPreviewQualityShortEdge`
- Modify: `web/src/store/uiStore.ts#setCanvasOffset`
- Modify: `web/src/components/preview/Preview.tsx#Preview`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/preview/Preview.interaction.test.tsx#control-38902ec9a22f4dc1 aspect/quality/zoom badge-menu option`

**Candidate-bound contracts:**

#### control-record-66d48b693d546924

- Candidate/source: `control-38902ec9a22f4dc1` at `web/src/components/preview/Preview.tsx:900:13` (control)
- Expected behavior: aspect/quality/zoom badge-menu option: assert exactly opt.onSelect(); setOpen(false). For aspect options only: applyAspect(p) -> setTimelineSettings(fps, p.width, p.height). For quality options only: setPreviewQualityShortEdge(active shortEdge ? null : p.shortEdge). For zoom options only: setCanvasOffset({width:0,height:0}) then setCanvasZoom(p.value) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-38902ec9a22f4dc1.
  - Test: web/src/components/preview/Preview.interaction.test.tsx#control-38902ec9a22f4dc1 aspect/quality/zoom badge-menu option.
  - Initial state: visibility=Visible on the Preview surface; content and playback capability determine actionable branches.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => {
                opt.onSelect();
                setOpen(false);
              }}.
  - Exact call/state/backend: stateTransition=aspect/quality/zoom badge-menu option: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/Preview.tsx:900::handler {() => {\n                opt.onSelect();\n                setOpen(false);\n              }} -> opt.onSelect(); setOpen(false). For aspect options only: applyAspect(p) -> setTimelineSettings(fps, p.width, p.height). For quality options only: setPreviewQualityShortEdge(active shortEdge ? null : p.shortEdge). For zoom options only: setCanvasOffset({width:0,height:0}) then setCanvasZoom(p.value)","web/src/components/preview/Preview.tsx::BadgeMenu option -> exact caller opt.onSelect","aspect branch only: web/src/store/editActions.ts::setTimelineSettings(fps,width,height) -> web/src/store/editActions.ts::applyAndRefresh -> web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback -> src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command -> crates/opentake-core/src/dto.rs::handle_edit_apply -> crates/opentake-ops/src/command.rs::EditCommand::SetTimelineSettings","quality branch only: web/src/store/uiStore.ts::setPreviewQualityShortEdge","zoom branch only: web/src/store/uiStore.ts::setCanvasOffset then setCanvasZoom","code:web/src/components/preview/Preview.tsx#Preview","code:web/src/components/preview/Preview.tsx#BadgeMenu","code:web/src/store/editActions.ts#setTimelineSettings","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=aspect/quality/zoom badge-menu option: assert exactly opt.onSelect(); setOpen(false). For aspect options only: applyAspect(p) -> setTimelineSettings(fps, p.width, p.height). For quality options only: setPreviewQualityShortEdge(active shortEdge ? null : p.shortEdge). For zoom options only: setCanvasOffset({width:0,height:0}) then setCanvasZoom(p.value) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"aspect/quality/zoom badge-menu option: assert exactly opt.onSelect(); setOpen(false). For aspect options only: applyAspect(p) -> setTimelineSettings(fps, p.width, p.height). For quality options only: setPreviewQualityShortEdge(active shortEdge ? null : p.shortEdge). For zoom options only: setCanvasOffset({width:0,height:0}) then setCanvasZoom(p.value) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/Preview.interaction.test.tsx#control-38902ec9a22f4dc1 aspect/quality/zoom badge-menu option` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-38902ec9a22f4dc1 aspect/quality/zoom badge-menu option"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/Preview.tsx`, `web/src/components/preview/Preview.tsx#BadgeMenu`, `web/src/store/editActions.ts#setTimelineSettings`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetTimelineSettings`, `web/src/store/uiStore.ts#setPreviewQualityShortEdge`, `web/src/store/uiStore.ts#setCanvasOffset`, `web/src/components/preview/Preview.tsx#Preview`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/Preview.interaction.test.tsx -t "control-38902ec9a22f4dc1 aspect/quality/zoom badge-menu option"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 19: control-acceptance (implementation-slice-d35426a6c65831dc)

**Covered records:**
- `control-record-a8367accdd1a0cf7` (control)
- `control-record-f0cd3731b15cc16e` (control)

**Files:**
- Modify: `web/src/components/preview/TransformOverlay.tsx`
- Modify: `web/src/store/editActions.ts#setClipProperties`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SetClipProperties`
- Modify: `web/src/components/preview/TransformOverlay.tsx#TransformOverlay`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/preview/TransformOverlay.interaction.test.tsx#control-c5a345c42bf6e3e3 move selected visual clip on canvas`
- Test (reviewed-planned): `web/src/components/preview/TransformOverlay.interaction.test.tsx#control-044765402e575182 resize selected visual clip from four corners`

**Candidate-bound contracts:**

#### control-record-a8367accdd1a0cf7

- Candidate/source: `control-c5a345c42bf6e3e3` at `web/src/components/preview/TransformOverlay.tsx:211:9` (control)
- Expected behavior: move selected visual clip on canvas: assert exactly handleMoveDown computes moveTransformByDeltaWithSnap; pointerup calls edit.setClipProperties([clip.id], { transform: cur }) exactly once when cur exists and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c5a345c42bf6e3e3.
  - Test: web/src/components/preview/TransformOverlay.interaction.test.tsx#control-c5a345c42bf6e3e3 move selected visual clip on canvas.
  - Initial state: visibility=Visible for one selected visual clip while crop editing is inactive.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["pointer/drag event sequence","coordinates and modifiers","current editor state","pointerup/cancel boundary"]; handler={handleMoveDown}.
  - Exact call/state/backend: stateTransition=move selected visual clip on canvas: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/TransformOverlay.tsx:211::handler {handleMoveDown} -> handleMoveDown computes moveTransformByDeltaWithSnap; pointerup calls edit.setClipProperties([clip.id], { transform: cur }) exactly once when cur exists","web/src/store/editActions.ts::setClipProperties({transform}) after move","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/preview/TransformOverlay.tsx#TransformOverlay","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=move selected visual clip on canvas: assert exactly handleMoveDown computes moveTransformByDeltaWithSnap; pointerup calls edit.setClipProperties([clip.id], { transform: cur }) exactly once when cur exists and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Pointerup/cancel releases the gesture and remains on the same editor surface.","A keyboard equivalent must retain/restore focus; current custom drag surface does not prove one."].
  - Outcome matrix: {"success":"move selected visual clip on canvas: assert exactly handleMoveDown computes moveTransformByDeltaWithSnap; pointerup calls edit.setClipProperties([clip.id], { transform: cur }) exactly once when cur exists and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Pointerup/cancel follows the exact profile; Escape rollback is not otherwise implemented.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

#### control-record-f0cd3731b15cc16e

- Candidate/source: `control-044765402e575182` at `web/src/components/preview/TransformOverlay.tsx:222:11` (control)
- Expected behavior: resize selected visual clip from four corners: assert exactly handleResizeDown(e, corner) computes resizeTransformFromCorner using local rotated delta; pointerup calls edit.setClipProperties([clip.id], { transform: cur }) exactly once when cur exists and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-044765402e575182.
  - Test: web/src/components/preview/TransformOverlay.interaction.test.tsx#control-044765402e575182 resize selected visual clip from four corners.
  - Initial state: visibility=Visible for one selected visual clip while crop editing is inactive.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["pointer/drag event sequence","coordinates and modifiers","current editor state","pointerup/cancel boundary"]; handler={(e) => handleResizeDown(e, corner)}.
  - Exact call/state/backend: stateTransition=resize selected visual clip from four corners: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/preview/TransformOverlay.tsx:222::handler {(e) => handleResizeDown(e, corner)} -> handleResizeDown(e, corner) computes resizeTransformFromCorner using local rotated delta; pointerup calls edit.setClipProperties([clip.id], { transform: cur }) exactly once when cur exists","web/src/store/editActions.ts::setClipProperties({transform}) after corner resize","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SetClipProperties","code:web/src/components/preview/TransformOverlay.tsx#TransformOverlay","code:web/src/store/editActions.ts#setClipProperties","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=resize selected visual clip from four corners: assert exactly handleResizeDown(e, corner) computes resizeTransformFromCorner using local rotated delta; pointerup calls edit.setClipProperties([clip.id], { transform: cur }) exactly once when cur exists and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Pointerup/cancel releases the gesture and remains on the same editor surface.","A keyboard equivalent must retain/restore focus; current custom drag surface does not prove one."].
  - Outcome matrix: {"success":"resize selected visual clip from four corners: assert exactly handleResizeDown(e, corner) computes resizeTransformFromCorner using local rotated delta; pointerup calls edit.setClipProperties([clip.id], { transform: cur }) exactly once when cur exists and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Pointerup/cancel follows the exact profile; Escape rollback is not otherwise implemented.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/TransformOverlay.interaction.test.tsx#control-c5a345c42bf6e3e3 move selected visual clip on canvas` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/preview/TransformOverlay.interaction.test.tsx#control-044765402e575182 resize selected visual clip from four corners` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.interaction.test.tsx -t "control-c5a345c42bf6e3e3 move selected visual clip on canvas"`
  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.interaction.test.tsx -t "control-044765402e575182 resize selected visual clip from four corners"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/TransformOverlay.tsx`, `web/src/store/editActions.ts#setClipProperties`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SetClipProperties`, `web/src/components/preview/TransformOverlay.tsx#TransformOverlay`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.interaction.test.tsx -t "control-c5a345c42bf6e3e3 move selected visual clip on canvas"`
  - Run: `pnpm -C web test -- --run src/components/preview/TransformOverlay.interaction.test.tsx -t "control-044765402e575182 resize selected visual clip from four corners"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 20: control-acceptance (implementation-slice-945d57943c971af2)

**Covered records:**
- `control-record-7809bfe999939b54` (control)

**Files:**
- Modify: `web/src/components/timeline/ClipContextMenu.tsx`
- Modify: `web/src/components/timeline/ClipContextMenu.tsx#clipContextMenuItems`
- Modify: `web/src/store/editActions.ts#copyClips`
- Modify: `web/src/store/uiStore.ts#setPendingSwapClipId`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-ops/src/command.rs`
- Modify: `web/src/components/timeline/ClipContextMenu.tsx#ClipContextMenu`
- Test (reviewed-planned): `web/src/components/timeline/ClipContextMenu.interaction.test.tsx#control-08a5f9a11ab18ba1 clip/fade/range context-menu action`

**Candidate-bound contracts:**

#### control-record-7809bfe999939b54

- Candidate/source: `control-08a5f9a11ab18ba1` at `web/src/components/timeline/ClipContextMenu.tsx:379:9` (control)
- Expected behavior: clip/fade/range context-menu action: assert exactly execute only the clicked item.action then onClose(). The actual item branches are: copyClips; pasteClipsAtPlayhead; splitAtPlayhead; deleteSelectedClips; linkClips(selected ids); unlinkClips(selected ids); setPendingSwapClipId(clipId); saveClipAsMedia(clipId); freezeClipAtPlayhead(currentClip, validated frames); setClipProperties([clipId], { reversed: !currentClip.reversed }); setClipProperties([clipId], { fadeInInterpolation:value } or { fadeOutInterpolation:value }); saveMarkedRangeAsMedia(range); or clearTimelineRange() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-08a5f9a11ab18ba1.
  - Test: web/src/components/timeline/ClipContextMenu.interaction.test.tsx#control-08a5f9a11ab18ba1 clip/fade/range context-menu action.
  - Initial state: visibility=Visible only while the corresponding menu is open.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => {
            item.action();
            onClose();
          }} {(e) => {
            (e.currentTarget as HTMLElement).style.background = "var(--bg-hover, rgba(255,255,255,0.08))";
          }} {(e) => {
            (e.currentTarget as HTMLElement).style.background = "transparent";
          }}.
  - Exact call/state/backend: stateTransition=clip/fade/range context-menu action: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/ClipContextMenu.tsx:379::handler {() => {\n            item.action();\n            onClose();\n          }} {(e) => {\n            (e.currentTarget as HTMLElement).style.background = \"var(--bg-hover, rgba(255,255,255,0.08))\";\n          }} {(e) => {\n            (e.currentTarget as HTMLElement).style.background = \"transparent\";\n          }} -> execute only the clicked item.action then onClose(). The actual item branches are: copyClips; pasteClipsAtPlayhead; splitAtPlayhead; deleteSelectedClips; linkClips(selected ids); unlinkClips(selected ids); setPendingSwapClipId(clipId); saveClipAsMedia(clipId); freezeClipAtPlayhead(currentClip, validated frames); setClipProperties([clipId], { reversed: !currentClip.reversed }); setClipProperties([clipId], { fadeInInterpolation:value } or { fadeOutInterpolation:value }); saveMarkedRangeAsMedia(range); or clearTimelineRange()","web/src/components/timeline/ClipContextMenu.tsx::clipContextMenuItems/fadeInterpolationMenuItems/rangeContextMenuItems","web/src/store/editActions.ts::copyClips/pasteClipsAtPlayhead/splitAtPlayhead/deleteSelectedClips/linkClips/unlinkClips/saveClipAsMedia/freezeClipAtPlayhead/setClipProperties/saveMarkedRangeAsMedia as selected","web/src/store/uiStore.ts::setPendingSwapClipId or clearTimelineRange for the two local-only selected branches","edit branches only: web/src/lib/api.ts::editApply -> src-tauri/src/commands.rs::edit_apply -> crates/opentake-ops/src/command.rs exact selected EditCommand","code:web/src/components/timeline/ClipContextMenu.tsx#ClipContextMenu","code:web/src/components/timeline/ClipContextMenu.tsx#clipContextMenuItems","code:web/src/store/editActions.ts#copyClips","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"].
  - Visible/accessibility/return path: success=clip/fade/range context-menu action: assert exactly execute only the clicked item.action then onClose(). The actual item branches are: copyClips; pasteClipsAtPlayhead; splitAtPlayhead; deleteSelectedClips; linkClips(selected ids); unlinkClips(selected ids); setPendingSwapClipId(clipId); saveClipAsMedia(clipId); freezeClipAtPlayhead(currentClip, validated frames); setClipProperties([clipId], { reversed: !currentClip.reversed }); setClipProperties([clipId], { fadeInInterpolation:value } or { fadeOutInterpolation:value }); saveMarkedRangeAsMedia(range); or clearTimelineRange() and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"clip/fade/range context-menu action: assert exactly execute only the clicked item.action then onClose(). The actual item branches are: copyClips; pasteClipsAtPlayhead; splitAtPlayhead; deleteSelectedClips; linkClips(selected ids); unlinkClips(selected ids); setPendingSwapClipId(clipId); saveClipAsMedia(clipId); freezeClipAtPlayhead(currentClip, validated frames); setClipProperties([clipId], { reversed: !currentClip.reversed }); setClipProperties([clipId], { fadeInInterpolation:value } or { fadeOutInterpolation:value }); saveMarkedRangeAsMedia(range); or clearTimelineRange() and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Most promise-returning item actions are fire-and-forget and expose no rejection state at the menu."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/ClipContextMenu.interaction.test.tsx#control-08a5f9a11ab18ba1 clip/fade/range context-menu action` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/ClipContextMenu.interaction.test.tsx -t "control-08a5f9a11ab18ba1 clip/fade/range context-menu action"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/ClipContextMenu.tsx`, `web/src/components/timeline/ClipContextMenu.tsx#clipContextMenuItems`, `web/src/store/editActions.ts#copyClips`, `web/src/store/uiStore.ts#setPendingSwapClipId`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/timeline/ClipContextMenu.tsx#ClipContextMenu` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/ClipContextMenu.interaction.test.tsx -t "control-08a5f9a11ab18ba1 clip/fade/range context-menu action"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 21: control-acceptance (implementation-slice-6570f00c6b73f133)

**Covered records:**
- `control-record-cf5a9f24e5410f3d` (control)
- `control-record-ed7c0dfc21854180` (control)

**Files:**
- Modify: `web/src/components/timeline/SwapMediaPicker.tsx`
- Modify: `web/src/components/timeline/SwapMediaPicker.tsx#SwapMediaPicker`
- Test (reviewed-planned): `web/src/components/timeline/SwapMediaPicker.interaction.test.tsx#control-cb057164b9983720 dismiss swap-media modal from backdrop`
- Test (reviewed-planned): `web/src/components/timeline/SwapMediaPicker.interaction.test.tsx#control-eeee485fbeb07e6e close swap-media modal`

**Candidate-bound contracts:**

#### control-record-cf5a9f24e5410f3d

- Candidate/source: `control-cb057164b9983720` at `web/src/components/timeline/SwapMediaPicker.tsx:75:5` (control)
- Expected behavior: dismiss swap-media modal from backdrop: assert exactly setPendingSwapClipId(null) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-cb057164b9983720.
  - Test: web/src/components/timeline/SwapMediaPicker.interaction.test.tsx#control-cb057164b9983720 dismiss swap-media modal from backdrop.
  - Initial state: visibility=Visible while pendingSwapClipId resolves to a live clip.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => setPendingSwapClipId(null)}.
  - Exact call/state/backend: stateTransition=dismiss swap-media modal from backdrop: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/SwapMediaPicker.tsx:75::handler {() => setPendingSwapClipId(null)} -> setPendingSwapClipId(null)","web/src/components/timeline/SwapMediaPicker.tsx::SwapMediaPicker backdrop -> uiStore.setPendingSwapClipId(null)","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/timeline/SwapMediaPicker.tsx#SwapMediaPicker"].
  - Visible/accessibility/return path: success=dismiss swap-media modal from backdrop: assert exactly setPendingSwapClipId(null) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"dismiss swap-media modal from backdrop: assert exactly setPendingSwapClipId(null) and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

#### control-record-ed7c0dfc21854180

- Candidate/source: `control-eeee485fbeb07e6e` at `web/src/components/timeline/SwapMediaPicker.tsx:113:11` (control)
- Expected behavior: close swap-media modal: assert exactly setPendingSwapClipId(null) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-eeee485fbeb07e6e.
  - Test: web/src/components/timeline/SwapMediaPicker.interaction.test.tsx#control-eeee485fbeb07e6e close swap-media modal.
  - Initial state: visibility=Visible while pendingSwapClipId resolves to a live clip.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => setPendingSwapClipId(null)}.
  - Exact call/state/backend: stateTransition=close swap-media modal: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/SwapMediaPicker.tsx:113::handler {() => setPendingSwapClipId(null)} -> setPendingSwapClipId(null)","web/src/components/timeline/SwapMediaPicker.tsx::SwapMediaPicker close button -> uiStore.setPendingSwapClipId(null)","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/timeline/SwapMediaPicker.tsx#SwapMediaPicker"].
  - Visible/accessibility/return path: success=close swap-media modal: assert exactly setPendingSwapClipId(null) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"Accessible-name source recorded by discovery: close.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"close swap-media modal: assert exactly setPendingSwapClipId(null) and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/SwapMediaPicker.interaction.test.tsx#control-cb057164b9983720 dismiss swap-media modal from backdrop` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/timeline/SwapMediaPicker.interaction.test.tsx#control-eeee485fbeb07e6e close swap-media modal` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/SwapMediaPicker.interaction.test.tsx -t "control-cb057164b9983720 dismiss swap-media modal from backdrop"`
  - Run: `pnpm -C web test -- --run src/components/timeline/SwapMediaPicker.interaction.test.tsx -t "control-eeee485fbeb07e6e close swap-media modal"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/SwapMediaPicker.tsx`, `web/src/components/timeline/SwapMediaPicker.tsx#SwapMediaPicker` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/SwapMediaPicker.interaction.test.tsx -t "control-cb057164b9983720 dismiss swap-media modal from backdrop"`
  - Run: `pnpm -C web test -- --run src/components/timeline/SwapMediaPicker.interaction.test.tsx -t "control-eeee485fbeb07e6e close swap-media modal"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 22: control-acceptance (implementation-slice-378d3f8aea41f91a)

**Covered records:**
- `control-record-8604f9324f6fc895` (control)

**Files:**
- Modify: `web/src/components/timeline/SwapMediaPicker.tsx`
- Modify: `web/src/store/editActions.ts#swapMedia`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SwapMedia`
- Modify: `web/src/components/timeline/SwapMediaPicker.tsx#SwapMediaPicker`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/timeline/SwapMediaPicker.interaction.test.tsx#control-b2a698e85b905b5c choose replacement media`

**Candidate-bound contracts:**

#### control-record-8604f9324f6fc895

- Candidate/source: `control-b2a698e85b905b5c` at `web/src/components/timeline/SwapMediaPicker.tsx:156:15` (control)
- Expected behavior: choose replacement media: assert exactly if (busy) no-op; else setBusy(true), clear error, await edit.swapMedia(clip.id,item.id); on success setPendingSwapClipId(null); on rejection setError(message); finally setBusy(false) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-b2a698e85b905b5c.
  - Test: web/src/components/timeline/SwapMediaPicker.interaction.test.tsx#control-b2a698e85b905b5c choose replacement media.
  - Initial state: visibility=Visible while pendingSwapClipId resolves to a live clip.; enabledWhen=Enabled when busy is false..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => pick(m)} {(e) => {
                  if (!busy)
                    e.currentTarget.style.background =
                      "var(--bg-hover, rgba(255,255,255,0.08))";
                }} {(e) => {
                  e.currentTarget.style.background = "transparent";
                }}.
  - Exact call/state/backend: stateTransition=choose replacement media: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/SwapMediaPicker.tsx:156::handler {() => pick(m)} {(e) => {\n                  if (!busy)\n                    e.currentTarget.style.background =\n                      \"var(--bg-hover, rgba(255,255,255,0.08))\";\n                }} {(e) => {\n                  e.currentTarget.style.background = \"transparent\";\n                }} -> if (busy) no-op; else setBusy(true), clear error, await edit.swapMedia(clip.id,item.id); on success setPendingSwapClipId(null); on rejection setError(message); finally setBusy(false)","web/src/store/editActions.ts::swapMedia(clip.id,item.id)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SwapMedia","code:web/src/components/timeline/SwapMediaPicker.tsx#SwapMediaPicker","code:web/src/store/editActions.ts#swapMedia","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=choose replacement media: assert exactly if (busy) no-op; else setBusy(true), clear error, await edit.swapMedia(clip.id,item.id); on success setPendingSwapClipId(null); on rejection setError(message); finally setBusy(false) and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"choose replacement media: assert exactly if (busy) no-op; else setBusy(true), clear error, await edit.swapMedia(clip.id,item.id); on success setPendingSwapClipId(null); on rejection setError(message); finally setBusy(false) and no sibling branch/command.","pending":"busy=true disables all candidate buttons until finally resets it.","empty":"When no same-type alternatives exist, the empty message renders and this candidate is absent.","disabled":"The rendered control is disabled by {busy}.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"The rejection message is rendered in the modal and the user can retry after busy=false."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/SwapMediaPicker.interaction.test.tsx#control-b2a698e85b905b5c choose replacement media` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/SwapMediaPicker.interaction.test.tsx -t "control-b2a698e85b905b5c choose replacement media"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/SwapMediaPicker.tsx`, `web/src/store/editActions.ts#swapMedia`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SwapMedia`, `web/src/components/timeline/SwapMediaPicker.tsx#SwapMediaPicker`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/SwapMediaPicker.interaction.test.tsx -t "control-b2a698e85b905b5c choose replacement media"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 23: control-acceptance (implementation-slice-580e6bc13f1224c8)

**Covered records:**
- `control-record-e98ff70fe45b929b` (control)

**Files:**
- Modify: `web/src/components/timeline/TimelineContainer.tsx`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#onMediaDragOver`
- Modify: `web/src/store/editActions.ts#insertClips`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-ops/src/command.rs`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`
- Test (reviewed-planned): `web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-224e0433b0aefb2a timeline media drag-over/drop viewport`

**Candidate-bound contracts:**

#### control-record-e98ff70fe45b929b

- Candidate/source: `control-224e0433b0aefb2a` at `web/src/components/timeline/TimelineContainer.tsx:1969:5` (control)
- Expected behavior: timeline media drag-over/drop viewport: assert exactly dragover validates MEDIA_DND_TYPE, computes exact ghost start/duration/track and optional ripple; dragleave clears only when leaving viewport; drop clears ghost/drag state and preview media, then calls exactly one applicable branch: insertClips(trackIndex,atFrame,entries) for valid ripple plan; addMomentToTimelineAt(item,startFrame,preferredTrack,momentRange,insertTrackAt) for a moment; or addMediaToTimelineAt(item,startFrame,preferredTrack,insertTrackAt) for a full asset and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-224e0433b0aefb2a.
  - Test: web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-224e0433b0aefb2a timeline media drag-over/drop viewport.
  - Initial state: visibility=Visible on the mounted timeline editor surface subject to the exact handler guards.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["pointer/drag event sequence","coordinates and modifiers","current editor state","pointerup/cancel boundary"]; handler={onMediaDragOver} {onMediaDragLeave} {onMediaDrop}.
  - Exact call/state/backend: stateTransition=timeline media drag-over/drop viewport: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TimelineContainer.tsx:1969::handler {onMediaDragOver} {onMediaDragLeave} {onMediaDrop} -> dragover validates MEDIA_DND_TYPE, computes exact ghost start/duration/track and optional ripple; dragleave clears only when leaving viewport; drop clears ghost/drag state and preview media, then calls exactly one applicable branch: insertClips(trackIndex,atFrame,entries) for valid ripple plan; addMomentToTimelineAt(item,startFrame,preferredTrack,momentRange,insertTrackAt) for a moment; or addMediaToTimelineAt(item,startFrame,preferredTrack,insertTrackAt) for a full asset","web/src/components/timeline/TimelineContainer.tsx::onMediaDragOver/onMediaDragLeave/onMediaDrop","web/src/store/editActions.ts::insertClips or addMomentToTimelineAt or addMediaToTimelineAt according to the resolved drop branch","web/src/lib/api.ts::editApply -> src-tauri/src/commands.rs::edit_apply -> crates/opentake-ops/src/command.rs exact InsertClips/AddClips/track branch","code:web/src/components/timeline/TimelineContainer.tsx#TimelineContainer","code:web/src/components/timeline/TimelineContainer.tsx#onMediaDragOver","code:web/src/store/editActions.ts#insertClips","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"].
  - Visible/accessibility/return path: success=timeline media drag-over/drop viewport: assert exactly dragover validates MEDIA_DND_TYPE, computes exact ghost start/duration/track and optional ripple; dragleave clears only when leaving viewport; drop clears ghost/drag state and preview media, then calls exactly one applicable branch: insertClips(trackIndex,atFrame,entries) for valid ripple plan; addMomentToTimelineAt(item,startFrame,preferredTrack,momentRange,insertTrackAt) for a moment; or addMediaToTimelineAt(item,startFrame,preferredTrack,insertTrackAt) for a full asset and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Pointerup/cancel releases the gesture and remains on the same editor surface.","A keyboard equivalent must retain/restore focus; current custom drag surface does not prove one."].
  - Outcome matrix: {"success":"timeline media drag-over/drop viewport: assert exactly dragover validates MEDIA_DND_TYPE, computes exact ghost start/duration/track and optional ripple; dragleave clears only when leaving viewport; drop clears ghost/drag state and preview media, then calls exactly one applicable branch: insertClips(trackIndex,atFrame,entries) for valid ripple plan; addMomentToTimelineAt(item,startFrame,preferredTrack,momentRange,insertTrackAt) for a moment; or addMediaToTimelineAt(item,startFrame,preferredTrack,insertTrackAt) for a full asset and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"Foreign/invalid payload, missing item, or unresolved insert plan is ignored or falls back to point resolution without emitting a sibling branch.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Pointerup/cancel follows the exact profile; Escape rollback is not otherwise implemented.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-224e0433b0aefb2a timeline media drag-over/drop viewport` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interaction.test.tsx -t "control-224e0433b0aefb2a timeline media drag-over/drop viewport"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineContainer.tsx`, `web/src/components/timeline/TimelineContainer.tsx#onMediaDragOver`, `web/src/store/editActions.ts#insertClips`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interaction.test.tsx -t "control-224e0433b0aefb2a timeline media drag-over/drop viewport"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 24: control-acceptance (implementation-slice-62ffc7071b31e970)

**Covered records:**
- `control-record-92622e3492468d55` (control)

**Files:**
- Modify: `web/src/components/timeline/TimelineContainer.tsx`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#onPointerDown`
- Modify: `web/src/store/editActions.ts#splitClip`
- Modify: `web/src/store/uiStore.ts#setCurrentFrame`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-ops/src/command.rs`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`
- Test (reviewed-planned): `web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-68307f78b80da87e timeline canvas pointer/context interaction`

**Candidate-bound contracts:**

#### control-record-92622e3492468d55

- Candidate/source: `control-68307f78b80da87e` at `web/src/components/timeline/TimelineContainer.tsx:1977:7` (control)
- Expected behavior: timeline canvas pointer/context interaction: assert exactly owned branches are exact: ruler scrub -> setCurrentFrame; razor hit -> edit.splitClip(hit.clip.id, snappedFrame); Cmd audio-line click -> edit.stampKeyframe(hit.clip.id,'volume',clipFrame); pointerup changed move -> insertTrack then moveClips/duplicateClips, or swapClips, or moveClips; changed volume dot -> moveKeyframe(clipId,'volume',fromFrame,ghostFrame); changed fade knee -> setClipProperties([clipId],{fadeInFrames|fadeOutFrames}); changed trim -> trimClips(edits); marquee/selection/gap branches update uiStore only; contextmenu sets only the exact audioVolumeKeyframe/range/clip menu; pointercancel/lost capture calls endDrag without edit and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-68307f78b80da87e.
  - Test: web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-68307f78b80da87e timeline canvas pointer/context interaction.
  - Initial state: visibility=Visible on the mounted timeline editor surface subject to the exact handler guards.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["pointer/drag event sequence","coordinates and modifiers","current editor state","pointerup/cancel boundary"]; handler={onPointerDown} {onPointerMove} {onPointerUp} {onContextMenu} {endDrag} {endDrag}.
  - Exact call/state/backend: stateTransition=timeline canvas pointer/context interaction: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TimelineContainer.tsx:1977::handler {onPointerDown} {onPointerMove} {onPointerUp} {onContextMenu} {endDrag} {endDrag} -> owned branches are exact: ruler scrub -> setCurrentFrame; razor hit -> edit.splitClip(hit.clip.id, snappedFrame); Cmd audio-line click -> edit.stampKeyframe(hit.clip.id,'volume',clipFrame); pointerup changed move -> insertTrack then moveClips/duplicateClips, or swapClips, or moveClips; changed volume dot -> moveKeyframe(clipId,'volume',fromFrame,ghostFrame); changed fade knee -> setClipProperties([clipId],{fadeInFrames|fadeOutFrames}); changed trim -> trimClips(edits); marquee/selection/gap branches update uiStore only; contextmenu sets only the exact audioVolumeKeyframe/range/clip menu; pointercancel/lost capture calls endDrag without edit","web/src/components/timeline/TimelineContainer.tsx::onPointerDown/onPointerMove/onPointerUp/onContextMenu/endDrag","web/src/store/editActions.ts::splitClip/stampKeyframe/insertTrack/moveClips/duplicateClips/swapClips/moveKeyframe/setClipProperties/trimClips only for the matched gesture branch","web/src/store/uiStore.ts::setCurrentFrame/selectClips/selectGap/setMenu for local-only matched branches","edit branches only: web/src/lib/api.ts::editApply -> src-tauri/src/commands.rs::edit_apply -> crates/opentake-ops/src/command.rs exact selected EditCommand","code:web/src/components/timeline/TimelineContainer.tsx#TimelineContainer","code:web/src/components/timeline/TimelineContainer.tsx#onPointerDown","code:web/src/store/editActions.ts#splitClip","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply"].
  - Visible/accessibility/return path: success=timeline canvas pointer/context interaction: assert exactly owned branches are exact: ruler scrub -> setCurrentFrame; razor hit -> edit.splitClip(hit.clip.id, snappedFrame); Cmd audio-line click -> edit.stampKeyframe(hit.clip.id,'volume',clipFrame); pointerup changed move -> insertTrack then moveClips/duplicateClips, or swapClips, or moveClips; changed volume dot -> moveKeyframe(clipId,'volume',fromFrame,ghostFrame); changed fade knee -> setClipProperties([clipId],{fadeInFrames|fadeOutFrames}); changed trim -> trimClips(edits); marquee/selection/gap branches update uiStore only; contextmenu sets only the exact audioVolumeKeyframe/range/clip menu; pointercancel/lost capture calls endDrag without edit and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Pointerup/cancel releases the gesture and remains on the same editor surface.","A keyboard equivalent must retain/restore focus; current custom drag surface does not prove one."].
  - Outcome matrix: {"success":"timeline canvas pointer/context interaction: assert exactly owned branches are exact: ruler scrub -> setCurrentFrame; razor hit -> edit.splitClip(hit.clip.id, snappedFrame); Cmd audio-line click -> edit.stampKeyframe(hit.clip.id,'volume',clipFrame); pointerup changed move -> insertTrack then moveClips/duplicateClips, or swapClips, or moveClips; changed volume dot -> moveKeyframe(clipId,'volume',fromFrame,ghostFrame); changed fade knee -> setClipProperties([clipId],{fadeInFrames|fadeOutFrames}); changed trim -> trimClips(edits); marquee/selection/gap branches update uiStore only; contextmenu sets only the exact audioVolumeKeyframe/range/clip menu; pointercancel/lost capture calls endDrag without edit and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"pointercancel/lost capture calls endDrag, clears local drag/snap/scrub state, and emits no edit command.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Gesture edit promises are mostly fire-and-forget; a backend rejection has no visible recovery state."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-68307f78b80da87e timeline canvas pointer/context interaction` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interaction.test.tsx -t "control-68307f78b80da87e timeline canvas pointer/context interaction"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineContainer.tsx`, `web/src/components/timeline/TimelineContainer.tsx#onPointerDown`, `web/src/store/editActions.ts#splitClip`, `web/src/store/uiStore.ts#setCurrentFrame`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs`, `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interaction.test.tsx -t "control-68307f78b80da87e timeline canvas pointer/context interaction"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 25: control-acceptance (implementation-slice-cb66199784101fc2)

**Covered records:**
- `control-record-478cd6484ebf572b` (control)

**Files:**
- Modify: `web/src/components/timeline/TimelineContainer.tsx`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`
- Test (reviewed-planned): `web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-54b7338ed4253568 keyboard-accessible clip proxy`

**Candidate-bound contracts:**

#### control-record-478cd6484ebf572b

- Candidate/source: `control-54b7338ed4253568` at `web/src/components/timeline/TimelineContainer.tsx:2029:11` (control)
- Expected behavior: keyboard-accessible clip proxy: assert exactly click calls selectClips(clipSelectionForInteraction(timeline,selectedClipIds,rect.clipId,event)); ContextMenu or Shift+F10 selects the clip if needed and sets the clip menu at proxy bounds; native contextmenu selects if needed and sets the clip menu at event coordinates/range frame and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-54b7338ed4253568.
  - Test: web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-54b7338ed4253568 keyboard-accessible clip proxy.
  - Initial state: visibility=Visible on the mounted timeline editor surface subject to the exact handler guards.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(event) =>
              selectClips(clipSelectionForInteraction(timeline, selectedClipIds, rect.clipId, event))
            } {(event) => {
              if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
                event.preventDefault();
                if (!selectedClipIds.has(rect.clipId)) {
                  selectClips(clipSelectionForInteraction(timeline, selectedClipIds, rect.clipId, {}));
                }
                const bounds = event.currentTarget.getBoundingClientRect();
                setMenu({
                  kind: "clip",
                  clipId: rect.clipId,
                  range: rangeAtContextFrame(selectedTimelineRange, activeFrame) ?? undefined,
                  x: bounds.left + bounds.width / 2,
                  y: bounds.top + bounds.height / 2,
                });
              }
            }} {(event) => {
              event.preventDefault();
              if (!selectedClipIds.has(rect.clipId)) {
                selectClips(clipSelectionForInteraction(timeline, selectedClipIds, rect.clipId, {}));
              }
              setMenu({
                kind: "clip",
                clipId: rect.clipId,
                range:
                  rangeAtContextFrame(
                    selectedTimelineRange,
                    frameAt(toDoc(event).docX, zoomScale),
                  ) ?? undefined,
                x: event.clientX,
                y: event.clientY,
              });
            }}.
  - Exact call/state/backend: stateTransition=keyboard-accessible clip proxy: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TimelineContainer.tsx:2029::handler {(event) =>\n              selectClips(clipSelectionForInteraction(timeline, selectedClipIds, rect.clipId, event))\n            } {(event) => {\n              if (event.key === \"ContextMenu\" || (event.shiftKey && event.key === \"F10\")) {\n                event.preventDefault();\n                if (!selectedClipIds.has(rect.clipId)) {\n                  selectClips(clipSelectionForInteraction(timeline, selectedClipIds, rect.clipId, {}));\n                }\n                const bounds = event.currentTarget.getBoundingClientRect();\n                setMenu({\n                  kind: \"clip\",\n                  clipId: rect.clipId,\n                  range: rangeAtContextFrame(selectedTimelineRange, activeFrame) ?? undefined,\n                  x: bounds.left + bounds.width / 2,\n                  y: bounds.top + bounds.height / 2,\n                });\n              }\n            }} {(event) => {\n              event.preventDefault();\n              if (!selectedClipIds.has(rect.clipId)) {\n                selectClips(clipSelectionForInteraction(timeline, selectedClipIds, rect.clipId, {}));\n              }\n              setMenu({\n                kind: \"clip\",\n                clipId: rect.clipId,\n                range:\n                  rangeAtContextFrame(\n                    selectedTimelineRange,\n                    frameAt(toDoc(event).docX, zoomScale),\n                  ) ?? undefined,\n                x: event.clientX,\n                y: event.clientY,\n              });\n            }} -> click calls selectClips(clipSelectionForInteraction(timeline,selectedClipIds,rect.clipId,event)); ContextMenu or Shift+F10 selects the clip if needed and sets the clip menu at proxy bounds; native contextmenu selects if needed and sets the clip menu at event coordinates/range frame","web/src/components/timeline/TimelineContainer.tsx::TimelineContainer accessibility proxy -> uiStore.selectClips/setMenu","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/timeline/TimelineContainer.tsx#TimelineContainer"].
  - Visible/accessibility/return path: success=keyboard-accessible clip proxy: assert exactly click calls selectClips(clipSelectionForInteraction(timeline,selectedClipIds,rect.clipId,event)); ContextMenu or Shift+F10 selects the clip if needed and sets the clip menu at proxy bounds; native contextmenu selects if needed and sets the clip menu at event coordinates/range frame and no sibling branch/command.; accessibility={"focus":"Native button proxy is keyboard-focusable.","label":"aria-label comes from rect.label and aria-pressed exposes selection.","shortcut":"ContextMenu or Shift+F10 opens the clip menu."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"keyboard-accessible clip proxy: assert exactly click calls selectClips(clipSelectionForInteraction(timeline,selectedClipIds,rect.clipId,event)); ContextMenu or Shift+F10 selects the clip if needed and sets the clip menu at proxy bounds; native contextmenu selects if needed and sets the clip menu at event coordinates/range frame and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-54b7338ed4253568 keyboard-accessible clip proxy` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interaction.test.tsx -t "control-54b7338ed4253568 keyboard-accessible clip proxy"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineContainer.tsx`, `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interaction.test.tsx -t "control-54b7338ed4253568 keyboard-accessible clip proxy"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 26: control-acceptance (implementation-slice-f31490008a77d5f8)

**Covered records:**
- `control-record-57ab6449414fac31` (control)

**Files:**
- Modify: `web/src/components/timeline/TimelineContainer.tsx`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#volumeKeyframeMenuItems`
- Modify: `web/src/store/editActions.ts#removeKeyframe`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::RemoveKeyframe`
- Modify: `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-554409b9d0b8d6bf audio-volume-keyframe menu action`

**Candidate-bound contracts:**

#### control-record-57ab6449414fac31

- Candidate/source: `control-554409b9d0b8d6bf` at `web/src/components/timeline/TimelineContainer.tsx:2208:9` (control)
- Expected behavior: audio-volume-keyframe menu action: assert exactly execute only clicked item.action then onClose(): delete branch edit.removeKeyframe(clipId,'volume',frame); interpolation branch edit.setKeyframeInterpolation(clipId,'volume',frame,'linear'|'smooth'|'hold') and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-554409b9d0b8d6bf.
  - Test: web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-554409b9d0b8d6bf audio-volume-keyframe menu action.
  - Initial state: visibility=Visible on the mounted timeline editor surface subject to the exact handler guards.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => {
            item.action();
            onClose();
          }} {(e) => {
            (e.currentTarget as HTMLElement).style.background = "var(--bg-hover, rgba(255,255,255,0.08))";
          }} {(e) => {
            (e.currentTarget as HTMLElement).style.background = "transparent";
          }}.
  - Exact call/state/backend: stateTransition=audio-volume-keyframe menu action: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TimelineContainer.tsx:2208::handler {() => {\n            item.action();\n            onClose();\n          }} {(e) => {\n            (e.currentTarget as HTMLElement).style.background = \"var(--bg-hover, rgba(255,255,255,0.08))\";\n          }} {(e) => {\n            (e.currentTarget as HTMLElement).style.background = \"transparent\";\n          }} -> execute only clicked item.action then onClose(): delete branch edit.removeKeyframe(clipId,'volume',frame); interpolation branch edit.setKeyframeInterpolation(clipId,'volume',frame,'linear'|'smooth'|'hold')","web/src/components/timeline/TimelineContainer.tsx::volumeKeyframeMenuItems/AudioVolumeKeyframeContextMenu","web/src/store/editActions.ts::removeKeyframe(clipId,'volume',frame) or setKeyframeInterpolation(clipId,'volume',frame,selected interpolation)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::RemoveKeyframe or SetKeyframeInterpolation","code:web/src/components/timeline/TimelineContainer.tsx#TimelineContainer","code:web/src/components/timeline/TimelineContainer.tsx#volumeKeyframeMenuItems","code:web/src/store/editActions.ts#removeKeyframe","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=audio-volume-keyframe menu action: assert exactly execute only clicked item.action then onClose(): delete branch edit.removeKeyframe(clipId,'volume',frame); interpolation branch edit.setKeyframeInterpolation(clipId,'volume',frame,'linear'|'smooth'|'hold') and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"audio-volume-keyframe menu action: assert exactly execute only clicked item.action then onClose(): delete branch edit.removeKeyframe(clipId,'volume',frame); interpolation branch edit.setKeyframeInterpolation(clipId,'volume',frame,'linear'|'smooth'|'hold') and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TimelineContainer.interaction.test.tsx#control-554409b9d0b8d6bf audio-volume-keyframe menu action` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interaction.test.tsx -t "control-554409b9d0b8d6bf audio-volume-keyframe menu action"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineContainer.tsx`, `web/src/components/timeline/TimelineContainer.tsx#volumeKeyframeMenuItems`, `web/src/store/editActions.ts#removeKeyframe`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::RemoveKeyframe`, `web/src/components/timeline/TimelineContainer.tsx#TimelineContainer`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineContainer.interaction.test.tsx -t "control-554409b9d0b8d6bf audio-volume-keyframe menu action"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 27: control-acceptance (implementation-slice-0f0f1f01ae6971d5)

**Covered records:**
- `control-record-1a45c4996f42ec2d` (control)

**Files:**
- Modify: `web/src/components/timeline/TimelineRangeContextMenu.tsx`
- Modify: `web/src/components/timeline/TimelineRangeContextMenu.tsx#rangeContextMenuItems`
- Modify: `web/src/store/editActions.ts#saveMarkedRangeAsMedia`
- Modify: `web/src/store/uiStore.ts#clearTimelineRange`
- Modify: `web/src/components/timeline/TimelineRangeContextMenu.tsx#TimelineRangeContextMenu`
- Test (reviewed-planned): `web/src/components/timeline/TimelineRangeContextMenu.interaction.test.tsx#control-2ae4c810c751ea2f marked-range save/clear action`

**Candidate-bound contracts:**

#### control-record-1a45c4996f42ec2d

- Candidate/source: `control-2ae4c810c751ea2f` at `web/src/components/timeline/TimelineRangeContextMenu.tsx:99:9` (control)
- Expected behavior: marked-range save/clear action: assert exactly execute only clicked item.action then onClose(): save branch edit.saveMarkedRangeAsMedia(range); clear branch uiStore.clearTimelineRange() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-2ae4c810c751ea2f.
  - Test: web/src/components/timeline/TimelineRangeContextMenu.interaction.test.tsx#control-2ae4c810c751ea2f marked-range save/clear action.
  - Initial state: visibility=Visible only while the corresponding menu is open.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => {
            item.action();
            onClose();
          }}.
  - Exact call/state/backend: stateTransition=marked-range save/clear action: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TimelineRangeContextMenu.tsx:99::handler {() => {\n            item.action();\n            onClose();\n          }} -> execute only clicked item.action then onClose(): save branch edit.saveMarkedRangeAsMedia(range); clear branch uiStore.clearTimelineRange()","web/src/components/timeline/TimelineRangeContextMenu.tsx::rangeContextMenuItems","save branch: web/src/store/editActions.ts::saveMarkedRangeAsMedia -> Tauri save operation","clear branch: web/src/store/uiStore.ts::clearTimelineRange; N/A API/Tauri/Rust","code:web/src/components/timeline/TimelineRangeContextMenu.tsx#TimelineRangeContextMenu","code:web/src/components/timeline/TimelineRangeContextMenu.tsx#rangeContextMenuItems","code:web/src/store/editActions.ts#saveMarkedRangeAsMedia"].
  - Visible/accessibility/return path: success=marked-range save/clear action: assert exactly execute only clicked item.action then onClose(): save branch edit.saveMarkedRangeAsMedia(range); clear branch uiStore.clearTimelineRange() and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"marked-range save/clear action: assert exactly execute only clicked item.action then onClose(): save branch edit.saveMarkedRangeAsMedia(range); clear branch uiStore.clearTimelineRange() and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TimelineRangeContextMenu.interaction.test.tsx#control-2ae4c810c751ea2f marked-range save/clear action` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineRangeContextMenu.interaction.test.tsx -t "control-2ae4c810c751ea2f marked-range save/clear action"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineRangeContextMenu.tsx`, `web/src/components/timeline/TimelineRangeContextMenu.tsx#rangeContextMenuItems`, `web/src/store/editActions.ts#saveMarkedRangeAsMedia`, `web/src/store/uiStore.ts#clearTimelineRange`, `web/src/components/timeline/TimelineRangeContextMenu.tsx#TimelineRangeContextMenu` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineRangeContextMenu.interaction.test.tsx -t "control-2ae4c810c751ea2f marked-range save/clear action"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 28: control-acceptance (implementation-slice-4825d091abd363f2)

**Covered records:**
- `control-record-4333a49344d81873` (control)

**Files:**
- Modify: `web/src/components/timeline/TimelineRegion.tsx`
- Modify: `web/src/components/timeline/TimelineRegion.tsx#onPointerDownCapture`
- Modify: `web/src/store/uiStore.ts#setPreviewMedia`
- Modify: `web/src/store/editActions.ts#addMediaToTimeline`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::InsertTrack`
- Modify: `web/src/components/timeline/TimelineRegion.tsx#TimelineRegion`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/timeline/TimelineRegion.interaction.test.tsx#control-c916e490cc27a73d timeline focus and empty-timeline media drop`

**Candidate-bound contracts:**

#### control-record-4333a49344d81873

- Candidate/source: `control-c916e490cc27a73d` at `web/src/components/timeline/TimelineRegion.tsx:60:7` (control)
- Expected behavior: timeline focus and empty-timeline media drop: assert exactly pointerdown capture calls setPreviewMedia(null); valid MEDIA_DND_TYPE dragover sets local dragOver; dragleave clears it only when leaving region; drop clears dragOver, resolves item by id, and only when found calls addMediaToTimeline(item) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c916e490cc27a73d.
  - Test: web/src/components/timeline/TimelineRegion.interaction.test.tsx#control-c916e490cc27a73d timeline focus and empty-timeline media drop.
  - Initial state: visibility=Visible on the mounted timeline editor surface subject to the exact handler guards.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => setPreviewMedia(null)} {onDragOver} {onDragLeave} {onDrop}.
  - Exact call/state/backend: stateTransition=timeline focus and empty-timeline media drop: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TimelineRegion.tsx:60::handler {() => setPreviewMedia(null)} {onDragOver} {onDragLeave} {onDrop} -> pointerdown capture calls setPreviewMedia(null); valid MEDIA_DND_TYPE dragover sets local dragOver; dragleave clears it only when leaving region; drop clears dragOver, resolves item by id, and only when found calls addMediaToTimeline(item)","web/src/components/timeline/TimelineRegion.tsx::onPointerDownCapture/onDragOver/onDragLeave/onDrop","web/src/store/uiStore.ts::setPreviewMedia(null) and local dragOver state","web/src/store/editActions.ts::addMediaToTimeline(item) only for a valid resolved drop item","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::InsertTrack/AddClips as resolved","code:web/src/components/timeline/TimelineRegion.tsx#TimelineRegion","code:web/src/store/editActions.ts#addMediaToTimeline","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=timeline focus and empty-timeline media drop: assert exactly pointerdown capture calls setPreviewMedia(null); valid MEDIA_DND_TYPE dragover sets local dragOver; dragleave clears it only when leaving region; drop clears dragOver, resolves item by id, and only when found calls addMediaToTimeline(item) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"No dedicated shortcut is declared."}; returnPath=["Remain on the owning editor surface after local state or authoritative mirror refresh.","Retain focus on the native control or explicitly restore it when conditional UI closes; this must be asserted."].
  - Outcome matrix: {"success":"timeline focus and empty-timeline media drop: assert exactly pointerdown capture calls setPreviewMedia(null); valid MEDIA_DND_TYPE dragover sets local dragOver; dragleave clears it only when leaving region; drop clears dragOver, resolves item by id, and only when found calls addMediaToTimeline(item) and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"Invalid payload or missing media id emits no addMediaToTimeline call.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"N/A — no separate cancellation phase.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TimelineRegion.interaction.test.tsx#control-c916e490cc27a73d timeline focus and empty-timeline media drop` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineRegion.interaction.test.tsx -t "control-c916e490cc27a73d timeline focus and empty-timeline media drop"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TimelineRegion.tsx`, `web/src/components/timeline/TimelineRegion.tsx#onPointerDownCapture`, `web/src/store/uiStore.ts#setPreviewMedia`, `web/src/store/editActions.ts#addMediaToTimeline`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::InsertTrack`, `web/src/components/timeline/TimelineRegion.tsx#TimelineRegion`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TimelineRegion.interaction.test.tsx -t "control-c916e490cc27a73d timeline focus and empty-timeline media drop"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 29: control-acceptance (implementation-slice-f474c635b693c7af)

**Covered records:**
- `control-record-359d2ab0f59a6585` (control)

**Files:**
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx`
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderRow`
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn`
- Test (reviewed-planned): `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-0af71bdcd21afd85 open track-header context menu`

**Candidate-bound contracts:**

#### control-record-359d2ab0f59a6585

- Candidate/source: `control-0af71bdcd21afd85` at `web/src/components/timeline/TrackHeaderColumn.tsx:161:5` (control)
- Expected behavior: open track-header context menu: assert exactly preventDefault(); setMenu({ x: e.clientX, y: e.clientY }) and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-0af71bdcd21afd85.
  - Test: web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-0af71bdcd21afd85 open track-header context menu.
  - Initial state: visibility=Visible for each timeline track; reorder items require an adjacent same-type track.; enabledWhen=Enabled whenever visible; the listed exact-call guards may deliberately no-op..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={(e) => {
        e.preventDefault();
        setMenu({ x: e.clientX, y: e.clientY });
      }}.
  - Exact call/state/backend: stateTransition=open track-header context menu: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TrackHeaderColumn.tsx:161::handler {(e) => {\n        e.preventDefault();\n        setMenu({ x: e.clientX, y: e.clientY });\n      }} -> preventDefault(); setMenu({ x: e.clientX, y: e.clientY })","web/src/components/timeline/TrackHeaderColumn.tsx::TrackHeaderRow local menu state","N/A — no API/Tauri/Rust backend for this exact candidate branch","code:web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn","code:web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderRow"].
  - Visible/accessibility/return path: success=open track-header context menu: assert exactly preventDefault(); setMenu({ x: e.clientX, y: e.clientY }) and no sibling branch/command.; accessibility={"focus":"Custom rendered element has no proven tabIndex/keyboard equivalent at this candidate.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"open track-header context menu: assert exactly preventDefault(); setMenu({ x: e.clientX, y: e.clientY }) and no sibling branch/command.","pending":"N/A — synchronous local/browser state only.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"No explicit disabled attribute beyond the listed visibility/handler guards.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"N/A — repeated activation is a new synchronous action.","failure":"N/A — no API/Tauri/Rust failure route."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-0af71bdcd21afd85 open track-header context menu` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-0af71bdcd21afd85 open track-header context menu"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TrackHeaderColumn.tsx`, `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderRow`, `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-0af71bdcd21afd85 open track-header context menu"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 30: control-acceptance (implementation-slice-e9ffdda6f71c8135)

**Covered records:**
- `control-record-3debf3f50c8910db` (control)

**Files:**
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx`
- Modify: `web/src/store/editActions.ts#swapTracks`
- Modify: `web/src/store/editActions.ts#applyAndRefresh`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-core/src/dto.rs#handle_edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::SwapTracks`
- Modify: `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-f74314c85d70667e move track up or down`

**Candidate-bound contracts:**

#### control-record-3debf3f50c8910db

- Candidate/source: `control-f74314c85d70667e` at `web/src/components/timeline/TrackHeaderColumn.tsx:311:11` (control)
- Expected behavior: move track up or down: assert exactly only when enabled, clicked move-up item calls swapTracks(p.index,p.index-1) or clicked move-down item calls swapTracks(p.index,p.index+1); then onClose() and no sibling branch/command.
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-f74314c85d70667e.
  - Test: web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-f74314c85d70667e move track up or down.
  - Initial state: visibility=Visible for each timeline track; reorder items require an adjacent same-type track.; enabledWhen=Enabled when !item.enabled is false..
  - Event: inputs=["click/keyboard activation supported by the rendered element","current candidate-specific model state"]; handler={() => {
              item.action();
              onClose();
            }} {(e) => {
              if (item.enabled) e.currentTarget.style.background = "var(--bg-hover, rgba(255,255,255,0.08))";
            }} {(e) => {
              e.currentTarget.style.background = "transparent";
            }}.
  - Exact call/state/backend: stateTransition=move track up or down: execute only the candidate-owned branches in exactCall; persisted branches refresh the authoritative timeline mirror and remain undoable.; backendTrace=["web/src/components/timeline/TrackHeaderColumn.tsx:311::handler {() => {\n              item.action();\n              onClose();\n            }} {(e) => {\n              if (item.enabled) e.currentTarget.style.background = \"var(--bg-hover, rgba(255,255,255,0.08))\";\n            }} {(e) => {\n              e.currentTarget.style.background = \"transparent\";\n            }} -> only when enabled, clicked move-up item calls swapTracks(p.index,p.index-1) or clicked move-down item calls swapTracks(p.index,p.index+1); then onClose()","web/src/store/editActions.ts::swapTracks(index,index-1 or index+1 for clicked enabled item)","web/src/store/editActions.ts::applyAndRefresh","web/src/lib/api.ts::editApply -> Tauri invoke('edit_apply',{command}) or browser fallback","src-tauri/src/commands.rs::edit_apply -> EditRequest::into_command","crates/opentake-core/src/dto.rs::handle_edit_apply","crates/opentake-ops/src/command.rs::EditCommand::SwapTracks","code:web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn","code:web/src/store/editActions.ts#swapTracks","code:web/src/store/editActions.ts#applyAndRefresh","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-core/src/dto.rs#handle_edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=move track up or down: assert exactly only when enabled, clicked move-up item calls swapTracks(p.index,p.index-1) or clicked move-down item calls swapTracks(p.index,p.index+1); then onClose() and no sibling branch/command.; accessibility={"focus":"Native rendered control is keyboard-focusable.","label":"No explicit aria-label/title association was discovered at this candidate.","shortcut":"Escape/outside close exists where mounted, but arrow-key roving focus and invoking-control focus restoration are not proven."}; returnPath=["Close through selected item, Escape, or outside action exactly where implemented.","Restore focus to the invoking control; current code does not explicitly prove this."].
  - Outcome matrix: {"success":"move track up or down: assert exactly only when enabled, clicked move-up item calls swapTracks(p.index,p.index-1) or clicked move-down item calls swapTracks(p.index,p.index+1); then onClose() and no sibling branch/command.","pending":"A promise may be pending, but this candidate exposes no explicit progress state.","empty":"N/A — owning visibility/handler guards prevent an absent target from emitting a command.","disabled":"The rendered control is disabled by {!item.enabled}.","cancel":"Dismiss/close leaves authoritative state unchanged; invoking-control focus restoration is not proven.","retry":"No automatic retry; repeated activation resubmits after the candidate becomes actionable.","failure":"Backend rejection is not caught/rendered at this fire-and-forget candidate; visible recovery evidence is required."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/timeline/TrackHeaderColumn.interaction.test.tsx#control-f74314c85d70667e move track up or down` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-f74314c85d70667e move track up or down"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/timeline/TrackHeaderColumn.tsx`, `web/src/store/editActions.ts#swapTracks`, `web/src/store/editActions.ts#applyAndRefresh`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-core/src/dto.rs#handle_edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand::SwapTracks`, `web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/timeline/TrackHeaderColumn.interaction.test.tsx -t "control-f74314c85d70667e move track up or down"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.
