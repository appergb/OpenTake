# Media Render Playback Export Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 68 verified incomplete records in the `media-render-playback-export` gap group.

**Architecture:** Implement 41 primary evidence-bound slices and reference 1 shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.

---

### Task 1: MR-capcut-composite (implementation-slice-a1b6e0f7c700bf1c)

**Covered records:**
- `requirement-60af541ae52187eb` (requirement)

**Files:**
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-render/tests/composite_acceptance.rs#capcut_children_close_one_composite_acceptance`

**Candidate-bound contracts:**

#### requirement-60af541ae52187eb

- Candidate/source: `doc-6f9ca70867547ae7` at `docs/architecture/CAPCUT-GAP.md:7` (requirement)
- Expected behavior: Close the explicitly chosen CapCut-parity gaps: 50-track performance, curve speed, nested sequences, multicam alignment, and optical-flow interpolation.
- Resolution: `reviewed-mapping-report:MR-capcut-composite` — This record combines 50-track performance, curve speed, nested sequences, multicam alignment and optical flow; nested and optical have child records and the other three need new child records.
- Exact acceptance contract:
  - Implementation: Define measurable 50-track preview/export budgets and meet them; add non-destructive nested timelines, audio-aligned multicam, speed keyframe curves with render-time mapping, and optical-flow interpolation; cover each with model/ops/render/UI tests and runtime fixtures.
  - Add deterministic media fixtures and golden frame, audio, timeline, or export assertions for every named capability and boundary; the affected render/media suites must pass.
  - Run the packaged preview/export path on representative media and retain exact output or runtime evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/composite_acceptance.rs#capcut_children_close_one_composite_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test composite_acceptance capcut_children_close_one_composite_acceptance -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test composite_acceptance capcut_children_close_one_composite_acceptance -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

Completion evidence (2026-07-31): the reviewed GPU test first failed on the
polygon fixture (`pixel=(0,0) expected=0 actual=255`) while polygon masks were
still encoded as a no-op, preserving the required RED receipt. The completed
slice adds bounded polygon uniforms, CPU/GPU-matched transform geometry,
Inspector creation/edit/delete controls, an on-canvas point editor, persistence,
and command-routed undo/redo with explicit capacity validation. Both exact
owning GPU tests pass; Web's 89 files / 805 tests, production build,
`cargo fmt --check`, workspace Clippy with `-D warnings`, and
`cargo test --workspace --no-fail-fast` pass. The ad-hoc-signed packaged macOS
app also passes creation, point drag/add/delete, transform, feather, invert,
undo/redo, save/reopen, preview capture, and 120-frame H.264 export. Preview and
export frame 60 score SSIM 0.999753 and PSNR 62.854540 dB. Full receipt:
`docs/audit/2026-07-14/runtime-artifacts/automated/mask-rendering-2026-07-31.md`.

### Task 2: MR-nested-timeline (implementation-slice-b8f61feebde4e2ab)

**Covered records:**
- `requirement-bedfdc6edfa147b9` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/clip.rs`
- Modify: `crates/opentake-render/src/plan/build.rs#build_frame_plan`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-render/tests/nested_timeline.rs#nested_edits_preview_and_export_same_frames`

**Candidate-bound contracts:**

#### requirement-bedfdc6edfa147b9

- Candidate/source: `doc-2c9413903831faff` at `docs/architecture/CAPCUT-GAP.md:16` (requirement)
- Expected behavior: Represent a clip that references an editable nested timeline.
- Resolution: `reviewed-mapping-report:MR-nested-timeline` — A nested clip reference and recursive render-plan flattening do not exist; the same plan must feed preview and export.
- Exact acceptance contract:
  - Implementation: Add a serialized nested-sequence reference type, dependency/cycle validation, edit commands, RenderPlan recursion/flatten caching, UI enter/exit controls, export support, and round-trip/render tests.
  - Add deterministic media fixtures and golden frame, audio, timeline, or export assertions for every named capability and boundary; the affected render/media suites must pass.
  - Run the packaged preview/export path on representative media and retain exact output or runtime evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/nested_timeline.rs#nested_edits_preview_and_export_same_frames` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test nested_timeline nested_edits_preview_and_export_same_frames -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/clip.rs`, `crates/opentake-render/src/plan/build.rs#build_frame_plan`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test nested_timeline nested_edits_preview_and_export_same_frames -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Completion evidence (2026-07-31):
`runtime-artifacts/automated/nested-timeline-compound-real-device-2026-07-31.md`.
The historical parent failed because the owning targets were absent; the exact
focused tests, full Rust/Web gates, strict local package verification, packaged
create/edit/reopen/preview, and 231-frame H.264/AAC export now pass.

### Task 3: MR-optical-flow (implementation-slice-c85c1acc35668396)

**Covered records:**
- `requirement-5933e802c9dfe372` (requirement)

**Files:**
- Modify: `crates/opentake-render/src/gpu/compositor.rs#TextureResolver`
- Modify: `crates/opentake-media/src/decode/frame.rs`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-render/tests/optical_flow.rs#two_frame_fixture_is_deterministic_and_matches_preview_export`

**Candidate-bound contracts:**

#### requirement-5933e802c9dfe372

- Candidate/source: `doc-e423ae30effded27` at `docs/architecture/CAPCUT-GAP.md:39` (requirement)
- Expected behavior: Optical-flow interpolation produces deterministic preview/export frames.
- Resolution: `reviewed-mapping-report:MR-optical-flow` — The report identified no current optical-flow backend; source-frame interpolation must enter the shared resolver path.
- Exact acceptance contract:
  - Add an optical-flow interpolation mode with explicit source/target frame-rate and fallback policy in the render model.
  - Convert a 24 fps motion fixture to 60 fps with exactly the expected output-frame count and unchanged first/last timestamps.
  - Add pixel/temporal regression tests plus a deterministic unsupported-device fallback; preview and export must select the same interpolation mode.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/optical_flow.rs#two_frame_fixture_is_deterministic_and_matches_preview_export` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test optical_flow two_frame_fixture_is_deterministic_and_matches_preview_export -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-render/src/gpu/compositor.rs#TextureResolver`, `crates/opentake-media/src/decode/frame.rs`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test optical_flow two_frame_fixture_is_deterministic_and_matches_preview_export -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Completion evidence (2026-07-31):
`runtime-artifacts/automated/optical-flow-24-to-60-real-device-2026-07-31.md`.
The historical owning target failed before the backend existed; the exact
focused tests, opposing-local-motion regression, full Rust gates, strict local
package verification, packaged 60 fps preview, and 120-frame H.264 export now
pass with matching frame-1 motion bounds and SSIM 0.999515.

### Task 4: MR-mask-rendering (implementation-slice-dacb1d7732ff3450)

**Covered records:**
- `requirement-0609f4de4f001a49` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/grade.rs#Mask`
- Modify: `crates/opentake-render/src/plan/build.rs`
- Modify: `crates/opentake-render/src/gpu/compositor.rs#pack_masks`
- Modify: `crates/opentake-render/src/gpu/shader.wgsl`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (existing-owned): `crates/opentake-render/tests/gpu_effects.rs#circle_mask_clips_to_center`
- Test (reviewed-planned): `crates/opentake-render/tests/gpu_effects.rs#linear_circle_and_polygon_masks_match_cpu_reference_in_preview_and_export`

**Candidate-bound contracts:**

#### requirement-0609f4de4f001a49

- Candidate/source: `doc-8b5862a78556aa8c` at `docs/architecture/CAPCUT-GAP.md:61` (requirement)
- Expected behavior: Linear, circular, and pen/polygon masks render consistently in preview and export.
- Resolution: `reviewed-mapping-report:MR-mask-rendering` — Linear and circular mask paths exist, while polygon masks are deliberately encoded as a no-op and need shared preview/export proof.
- Exact acceptance contract:
  - Persist linear, circular, and polygon/pen masks with feather, invert, and transform parameters on clips.
  - Expose mask creation, point editing, delete, and undo/redo in Inspector/Preview without mutating source media.
  - Add GPU pixel fixtures for all three shapes at feather 0 and nonzero feather; preview and exported boundary frames must match within the project pixel-diff tolerance.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/gpu_effects.rs#circle_mask_clips_to_center` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/tests/gpu_effects.rs#linear_circle_and_polygon_masks_match_cpu_reference_in_preview_and_export` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test gpu_effects circle_mask_clips_to_center -- --exact`
  - Run: `cargo test -p opentake-render --test gpu_effects linear_circle_and_polygon_masks_match_cpu_reference_in_preview_and_export -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/grade.rs#Mask`, `crates/opentake-render/src/plan/build.rs`, `crates/opentake-render/src/gpu/compositor.rs#pack_masks`, `crates/opentake-render/src/gpu/shader.wgsl`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test gpu_effects circle_mask_clips_to_center -- --exact`
  - Run: `cargo test -p opentake-render --test gpu_effects linear_circle_and_polygon_masks_match_cpu_reference_in_preview_and_export -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 5: MR-stabilization (implementation-slice-7ef9369889a0a0d6)

**Covered records:**
- `requirement-20198476e9083261` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/analysis/stabilization.rs`
- Modify: `crates/opentake-ops/src/command.rs`
- Modify: `crates/opentake-render/src/plan/build.rs`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-render/tests/stabilization.rs#synthetic_shake_produces_editable_undoable_preview_export_solution`

**Candidate-bound contracts:**

#### requirement-20198476e9083261

- Candidate/source: `doc-9f4727115dbcfd34` at `docs/architecture/CAPCUT-GAP.md:85` (requirement)
- Expected behavior: Stabilization analyzes motion and applies an editable crop/transform solution.
- Resolution: `reviewed-mapping-report:MR-stabilization` — No tracked stabilization analyzer or command owner exists; the solution must be represented as editable crop or transform state.
- Exact acceptance contract:
  - Persist stabilization analysis as an editable transform/crop track with model/version and source identity.
  - Expose analyze, strength/crop adjustment, cancellation, apply, reset, and undo without overwriting source media.
  - For a synthetic jitter fixture, demonstrate lower frame-to-frame tracked displacement, no uncovered pixels, and preview/export transform parity.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/stabilization.rs#synthetic_shake_produces_editable_undoable_preview_export_solution` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test stabilization synthetic_shake_produces_editable_undoable_preview_export_solution -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/analysis/stabilization.rs`, `crates/opentake-ops/src/command.rs`, `crates/opentake-render/src/plan/build.rs`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test stabilization synthetic_shake_produces_editable_undoable_preview_export_solution -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 6: MR-generic-effects (implementation-slice-ee2ab54c1e0ae863)

**Covered records:**
- `requirement-0086c30d6dc64f51` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/grade.rs#Effect`
- Modify: `crates/opentake-render/src/plan/types.rs#LayerDraw`
- Modify: `crates/opentake-render/src/gpu/compositor.rs`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-render/tests/gpu_effects.rs#advertised_effect_registry_has_preview_export_golden_fixtures`

**Candidate-bound contracts:**

#### requirement-0086c30d6dc64f51

- Candidate/source: `doc-a3897bfa0c91c4fb` at `docs/architecture/CAPCUT-GAP.md:103` (requirement)
- Expected behavior: Advertised effects and filters have real GPU/CPU render implementations with preview/export parity.
- Resolution: `reviewed-mapping-report:MR-generic-effects` — Effect metadata reaches the render plan but the generic effect chain has no production pass implementation.
- Exact acceptance contract:
  - Define a closed effect registry whose persisted parameter schema is validated; unknown effects must return a typed error instead of silently rendering unchanged.
  - Expose add/reorder/parameter-change/remove operations through undoable Inspector commands.
  - Add one pixel fixture per advertised effect/filter and assert preview/export parity at default and non-default parameters.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/gpu_effects.rs#advertised_effect_registry_has_preview_export_golden_fixtures` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test gpu_effects advertised_effect_registry_has_preview_export_golden_fixtures -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/grade.rs#Effect`, `crates/opentake-render/src/plan/types.rs#LayerDraw`, `crates/opentake-render/src/gpu/compositor.rs`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test gpu_effects advertised_effect_registry_has_preview_export_golden_fixtures -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Runtime and packaged-app evidence: `docs/audit/2026-07-14/runtime-artifacts/automated/generic-effects-real-device-2026-07-31.md`.

### Task 7: MR-transitions (implementation-slice-36596c6aa0eb94d6)

**Covered records:**
- `requirement-f5c136c992515801` (requirement)
- `requirement-73810a059793a1b8` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/transition.rs`
- Modify: `crates/opentake-render/src/plan/build.rs`
- Modify: `crates/opentake-render/src/gpu/compositor.rs`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Modify: `docs/architecture/HANDOFF-2026-07.md`
- Test (reviewed-planned): `crates/opentake-render/tests/transitions.rs#adjacent_clip_transition_is_editable_undoable_and_matches_preview_export`

**Candidate-bound contracts:**

#### requirement-f5c136c992515801

- Candidate/source: `doc-ed457b544b875446` at `docs/architecture/CAPCUT-GAP.md:109` (requirement)
- Expected behavior: Transitions are editable and render with preview/export parity.
- Resolution: `reviewed-mapping-report:MR-transitions` — Both records describe one missing editable transition model and overlap render pass.
- Exact acceptance contract:
  - Persist a transition with kind, duration, and both adjacent clip IDs, rejecting overlaps longer than either available handle.
  - Expose add/change/remove transition actions in the enabled transition surface with undo/redo.
  - For cut, midpoint, and end frames of each advertised transition, assert preview/export pixels match and save/reopen preserves the transition exactly.

#### requirement-73810a059793a1b8

- Candidate/source: `doc-470e38525e685a91` at `docs/architecture/HANDOFF-2026-07.md:178` (requirement)
- Expected behavior: Transitions are selectable, editable, and rendered.
- Resolution: `reviewed-mapping-report:MR-transitions` — Both records describe one missing editable transition model and overlap render pass.
- Exact acceptance contract:
  - Define transition model and edit commands.
  - Enable the transitions media surface.
  - Add pixel/runtime tests for preview/export parity and undo.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/transitions.rs#adjacent_clip_transition_is_editable_undoable_and_matches_preview_export` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test transitions adjacent_clip_transition_is_editable_undoable_and_matches_preview_export -- --exact`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/transition.rs`, `crates/opentake-render/src/plan/build.rs`, `crates/opentake-render/src/gpu/compositor.rs`, `docs/architecture/CAPCUT-GAP.md`, `docs/architecture/HANDOFF-2026-07.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test transitions adjacent_clip_transition_is_editable_undoable_and_matches_preview_export -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Runtime and packaged-app evidence: `docs/audit/2026-07-14/runtime-artifacts/automated/transition-real-device-2026-07-31.md`.

### Task 8: MR-lgg-proof (implementation-slice-4c614d7762698953)

**Covered records:**
- `requirement-6ee2e382b1d3733c` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/grade.rs#ColorGrade`
- Modify: `crates/opentake-render/src/gpu/compositor.rs#grade_blocks`
- Modify: `crates/opentake-render/src/gpu/shader.wgsl`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (existing-owned): `crates/opentake-domain/src/grade.rs#lift_gamma_gain_gain_scales`
- Test (reviewed-planned): `crates/opentake-render/tests/gpu_effects.rs#lift_gamma_gain_matches_cpu_reference`

**Candidate-bound contracts:**

#### requirement-6ee2e382b1d3733c

- Candidate/source: `doc-2016fc49884f6dc7` at `docs/architecture/CAPCUT-GAP.md:127` (requirement)
- Expected behavior: At docs/architecture/CAPCUT-GAP.md:127 under “色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1” (heading), the source “### 色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1” requires this exact behavior: Lift, gamma, and gain controls are represented and rendered.
- Resolution: `reviewed-mapping-report:MR-lgg-proof` — Representation and shader code exist, but the record should remain incomplete until GPU output is checked against the CPU reference.
- Exact acceptance contract:
  - Source binding: docs/architecture/CAPCUT-GAP.md:127; signal=heading; heading=色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1; candidate=### 色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1
  - Expected behavior: Lift, gamma, and gain controls are represented and rendered. This closes only the promise expressed by “色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1” in “色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1” with the scenario below and register test:crates/opentake-render/tests/completion_2016fc49884f6dc7.rs#completion_2016fc49884f6dc7_lift_gamma_and_gain_controls_are_represented_and
  - Initial state/input/event: start from the smallest valid fixture for “Lift, gamma, and gain controls are represented and rendered.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1”.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Lift, gamma, and gain controls are represented and rendered.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_2016fc49884f6dc7.rs#completion_2016fc49884f6dc7_lift_gamma_and_gain_controls_are_represented_and.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-domain/src/grade.rs#lift_gamma_gain_gain_scales` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/tests/gpu_effects.rs#lift_gamma_gain_matches_cpu_reference` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-domain lift_gamma_gain_gain_scales`
  - Run: `cargo test -p opentake-render --test gpu_effects lift_gamma_gain_matches_cpu_reference -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/grade.rs#ColorGrade`, `crates/opentake-render/src/gpu/compositor.rs#grade_blocks`, `crates/opentake-render/src/gpu/shader.wgsl`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-domain lift_gamma_gain_gain_scales`
  - Run: `cargo test -p opentake-render --test gpu_effects lift_gamma_gain_matches_cpu_reference -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Runtime and packaged-app evidence: `docs/audit/2026-07-14/runtime-artifacts/automated/lgg-real-device-2026-07-31.md`.

### Task 9: MR-hsl-secondary (implementation-slice-0e1b61977fdbc412)

**Covered records:**
- `requirement-30c371e348da001c` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/grade.rs`
- Modify: `crates/opentake-render/src/gpu/shader.wgsl`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-render/tests/gpu_effects.rs#hsl_secondary_hue_boundary_feather_and_isolation`

**Candidate-bound contracts:**

#### requirement-30c371e348da001c

- Candidate/source: `doc-9edde6aa1ce22995` at `docs/architecture/CAPCUT-GAP.md:139` (requirement)
- Expected behavior: HSL secondary controls are editable and rendered.
- Resolution: `implemented-and-verified:MR-hsl-secondary` — A bounded feathered HSL qualifier now persists through `ColorGrade`, renders through the shared CPU/WGSL chain, and is editable/resettable through the transactional Inspector path. Real GPU chart isolation and packaged preview/playback/export evidence are recorded below.
- Exact acceptance contract:
  - Persist bounded hue-range, feather, hue, saturation, and lightness adjustments in the grade model.
  - Expose range selection and parameter edits with reset and undo/redo in Inspector.
  - Use a color-chart fixture to verify selected hues change while pixels outside the feathered range remain within the project pixel-diff tolerance in preview and export.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/gpu_effects.rs#hsl_secondary_hue_boundary_feather_and_isolation` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test gpu_effects hsl_secondary_hue_boundary_feather_and_isolation -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/grade.rs`, `crates/opentake-render/src/gpu/shader.wgsl`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test gpu_effects hsl_secondary_hue_boundary_feather_and_isolation -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Runtime and packaged-app evidence: `docs/audit/2026-07-14/runtime-artifacts/automated/hsl-secondary-real-device-2026-07-31.md`.

### Task 10: MR-lut (implementation-slice-10ed256c8194fc18)

**Covered records:**
- `requirement-2156cc0bdb849391` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/lut.rs`
- Modify: `crates/opentake-render/src/gpu/compositor.rs`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-render/tests/lut.rs#malformed_and_oversized_luts_fail_closed_and_valid_lut_matches_preview_export`

**Candidate-bound contracts:**

#### requirement-2156cc0bdb849391

- Candidate/source: `doc-0b8b56b94a928929` at `docs/architecture/CAPCUT-GAP.md:145` (requirement)
- Expected behavior: Validated 3D LUT files can be imported, previewed, and exported.
- Resolution: `implemented-and-verified:MR-lut` — Validated 17/33-point `.cube` tables now publish into content-addressed project storage, persist as path-free clip references, and render through one shared 3D-texture path for preview, playback, inspection, and export. Transactional Inspector editing, real-GPU identity/known-transform parity, save/reopen, and complete packaged export evidence are recorded below.
- Exact acceptance contract:
  - Parse and validate .cube LUT metadata, domain bounds, and 17- and 33-point tables; reject malformed or oversized input with typed errors.
  - Import, select, set intensity, remove, and undo LUT changes without copying arbitrary files outside project-managed storage.
  - Compare identity and known-transform LUT fixtures in GPU preview and export using the existing pixel-diff threshold, including save/reopen.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/lut.rs#malformed_and_oversized_luts_fail_closed_and_valid_lut_matches_preview_export` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test lut malformed_and_oversized_luts_fail_closed_and_valid_lut_matches_preview_export -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/lut.rs`, `crates/opentake-render/src/gpu/compositor.rs`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test lut malformed_and_oversized_luts_fail_closed_and_valid_lut_matches_preview_export -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Runtime and packaged-app evidence: `docs/audit/2026-07-14/runtime-artifacts/automated/lut-real-device-2026-07-31.md`.

### Task 11: MR-loudness (implementation-slice-e668f03cd9c414d2)

**Covered records:**
- `requirement-25da8163d71af1bf` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/decode/pcm.rs#extract_pcm`
- Modify: `crates/opentake-media/src/analysis/loudness.rs`
- Modify: `src-tauri/src/playback/audio.rs`
- Modify: `src-tauri/src/export.rs`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-media/tests/loudness.rs#normalization_reaches_configured_lufs_within_tolerance`

**Candidate-bound contracts:**

#### requirement-25da8163d71af1bf

- Candidate/source: `doc-30e9287b5c8858dd` at `docs/architecture/CAPCUT-GAP.md:161` (requirement)
- Expected behavior: Audio loudness normalization targets and verifies a configured LUFS value.
- Resolution: `implemented-and-verified:MR-loudness` — shared PCM analysis, undoable persistence, Inspector controls, native preview and export now form one verified vertical slice.
- Exact acceptance contract:
  - Persist a target integrated loudness and true-peak ceiling as an undoable audio operation.
  - Expose analyze/apply/reset with progress and typed errors for silent or unreadable audio.
  - On speech and music fixtures, exported integrated loudness must be within ±1 LU of the configured target without exceeding the configured true-peak ceiling; preview gain must use the same computed adjustment.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/tests/loudness.rs#normalization_reaches_configured_lufs_within_tolerance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media --test loudness normalization_reaches_configured_lufs_within_tolerance -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/decode/pcm.rs#extract_pcm`, `crates/opentake-media/src/analysis/loudness.rs`, `src-tauri/src/playback/audio.rs`, `src-tauri/src/export.rs`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media --test loudness normalization_reaches_configured_lufs_within_tolerance -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. The focused runner first failed to compile before the loudness owner existed, then passed with the known FFmpeg-cross-checked fixture and deterministic speech/music fixtures. The complete Rust workspace passed (environment-only integrations remain ignored), warnings-denied workspace Clippy, formatting and diff checks passed; the Web suite passed 90 files / 814 tests and the production build passed with only the pre-existing chunk/dynamic-import warnings. The rebuilt ad-hoc-signed macOS app verified analyze/apply/reanalyze/reset, undo/redo, native playback, writable save/reopen persistence and a typed silent-audio error. Independent FFmpeg measurements of the exported AAC deliverables were `-16.07 LUFS / -1.15 dBTP` for speech and `-16.02 LUFS / -1.74 dBTP` for music. Runtime evidence: [`loudness-real-device-2026-08-01.md`](../runtime-artifacts/automated/loudness-real-device-2026-08-01.md).

### Task 12: MR-denoise (implementation-slice-3d159672cfd1fc67)

**Covered records:**
- `requirement-58c2159d21084d01` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/analysis/denoise.rs`
- Modify: `src-tauri/src/playback/audio.rs`
- Modify: `src-tauri/src/export.rs`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-media/tests/denoise.rs#deterministic_noise_fixture_and_bypass`
- Test (reviewed-planned): `src-tauri/src/playback/audio.rs#denoise_preview_uses_shared_processing_owner`
- Test (reviewed-planned): `src-tauri/src/export.rs#denoise_export_uses_shared_processing_owner`

**Candidate-bound contracts:**

#### requirement-58c2159d21084d01

- Candidate/source: `doc-d4fe9b623de65daa` at `docs/architecture/CAPCUT-GAP.md:167` (requirement)
- Expected behavior: Audio denoise is available in preview and export.
- Resolution: `reviewed-mapping-report:MR-denoise` — No tracked denoise node exists; playback and export must consume the same processing owner.
- Exact acceptance contract:
  - Persist denoise mode/strength parameters and keep the source audio immutable.
  - Expose preview toggle, apply/reset, cancellation, and undo/redo in the Audio Inspector.
  - On a speech-plus-noise fixture, assert at least 3 dB SNR improvement with no clipping, and verify preview/export use identical denoise parameters.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/tests/denoise.rs#deterministic_noise_fixture_and_bypass` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `src-tauri/src/playback/audio.rs#denoise_preview_uses_shared_processing_owner` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `src-tauri/src/export.rs#denoise_export_uses_shared_processing_owner` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media --test denoise deterministic_noise_fixture_and_bypass -- --exact`
  - Run: `cargo test -p opentake-tauri --no-default-features --features playback-engine denoise_preview_uses_shared_processing_owner`
  - Run: `cargo test -p opentake-tauri --no-default-features denoise_export_uses_shared_processing_owner`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/analysis/denoise.rs`, `src-tauri/src/playback/audio.rs`, `src-tauri/src/export.rs`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media --test denoise deterministic_noise_fixture_and_bypass -- --exact`
  - Run: `cargo test -p opentake-tauri --no-default-features --features playback-engine denoise_preview_uses_shared_processing_owner`
  - Run: `cargo test -p opentake-tauri --no-default-features denoise_export_uses_shared_processing_owner`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. The reviewed owning tests first failed because no shared denoise owner existed, then passed after the domain contract, pure-Rust spectral processor, playback/export integration, command/undo path, cancellable Tauri job and Inspector controls were connected. A second RED regression caught a rare STFT boundary peak (`input=0.388362`, `output=1.000000`); the edge crossfade and no-new-peak bound fixed it. The complete Rust workspace, warnings-denied Clippy, formatting and diff checks passed; the Web suite passed 91 files / 818 tests and the production build passed with only the pre-existing chunk/dynamic-import warnings. In the rebuilt ad-hoc-signed macOS app, adaptive and voice modes, strength, preview toggle, apply/reset, cancellation, undo/redo, native playback, save/reopen persistence, and export with preview disabled were exercised. The deterministic speech-plus-noise fixture improved from `10.414 dB` to `16.2872 dB` SDR (`+5.8732 dB`), while the exported AAC peak remained `-8.645 dB`. Runtime evidence: [`denoise-real-device-2026-08-01.md`](../runtime-artifacts/automated/denoise-real-device-2026-08-01.md).

### Task 13: MR-stems (implementation-slice-9139657f9c8c7ff5)

**Covered records:**
- `requirement-7c518a7042e8d780` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/analysis/stems.rs`
- Modify: `crates/opentake-gen/src/stems.rs`
- Modify: `crates/opentake-core/src/session.rs`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-media/tests/stems.rs#local_or_explicit_provider_selection_cancellation_provenance_and_cleanup`

**Candidate-bound contracts:**

#### requirement-7c518a7042e8d780

- Candidate/source: `doc-8a0fdfbeaab482c5` at `docs/architecture/CAPCUT-GAP.md:176` (requirement)
- Expected behavior: Separate vocals/music/stems locally or through an explicitly configured generation provider.
- Resolution: `reviewed-mapping-report:MR-stems` — Completed for the explicitly scoped local centre/side extractor: both results re-enter the shared media import and provenance path; broader semantic separation remains tracked as a separate partial gap.
- Exact acceptance contract:
  - Implementation: Implement an asynchronous stem-separation job with model installation/integrity checks, progress/cancel, derived-asset import, privacy/error UX, and audio-quality/integration fixtures; document local versus hosted execution.
  - Add deterministic media fixtures and golden frame, audio, timeline, or export assertions for every named capability and boundary; the affected render/media suites must pass.
  - Run the packaged preview/export path on representative media and retain exact output or runtime evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/tests/stems.rs#local_or_explicit_provider_selection_cancellation_provenance_and_cleanup` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media --test stems local_or_explicit_provider_selection_cancellation_provenance_and_cleanup -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/analysis/stems.rs`, `crates/opentake-gen/src/stems.rs`, `crates/opentake-core/src/session.rs`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media --test stems local_or_explicit_provider_selection_cancellation_provenance_and_cleanup -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. The reviewed owning test first failed because no stem owner existed, then passed after adding the bundled `opentake-center-v1` integrity-checked local profile, cancellable progress reporting, atomic two-output publication, explicit hosted-provider/privacy policy, atomic derived-asset import and source/model provenance. A packaged-runtime defect review then produced three additional RED regressions: selecting a generated source entered a Zustand selector update loop, project-relative derived media did not expose its resolved preview path, and the original side-channel accompaniment cancelled in the current mono export mixdown. Stable selectors, resolved DTO paths and dual-mono stem publication fixed those failures. The rebuilt ad-hoc-signed macOS app exercised local privacy copy, hosted consent/configuration fail-closed behavior, successful separation, save/reopen provenance, direct preview, timeline playback, independent vocals/accompaniment export, and cancellation of a 1,800-second job with no derived manifest entries or partial files. The local profile is deliberately a deterministic centre/side extractor for centred voice/dialogue, not semantic Demucs/MDX separation; hosted transport remains unavailable until an adapter is configured. Architecture and execution boundaries are documented in [`STEM-SEPARATION.md`](../../../architecture/STEM-SEPARATION.md). Runtime evidence: [`stems-real-device-2026-08-01.md`](../runtime-artifacts/automated/stems-real-device-2026-08-01.md).

### Task 14: MR-linked-audio-complete (implementation-slice-cbce9a4174a73347)

**Covered records:**
- `requirement-06faee34d4b29a33` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs`
- Modify: `crates/opentake-ops/src/ops/place.rs`
- Modify: `docs/architecture/EDITING-ENGINE-PLAN.md`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#add_clips_does_not_link_audio_when_source_has_no_audio`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#insert_clips_does_not_link_audio_when_source_has_no_audio`

**Candidate-bound contracts:**

#### requirement-06faee34d4b29a33

- Candidate/source: `doc-1c3d81a8b3ab2d59` at `docs/architecture/EDITING-ENGINE-PLAN.md:33` (requirement)
- Expected behavior: At docs/architecture/EDITING-ENGINE-PLAN.md:33 under “2. 链接音频(linked audio)—— 这是上游既定设计,不是 bug” (gap-marker), the source “- **无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。” requires this exact behavior: Do not create a linked audio track when probe proves a video has zero audio channels.
- Resolution: `reviewed-mapping-report:MR-linked-audio-complete` — Both add and insert command paths directly prove that a probed silent video does not create linked audio.
- Exact acceptance contract:
  - Source binding: docs/architecture/EDITING-ENGINE-PLAN.md:33; signal=gap-marker; heading=2. 链接音频(linked audio)—— 这是上游既定设计,不是 bug; candidate=- **无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。
  - Expected behavior: Do not create a linked audio track when probe proves a video has zero audio channels. This closes only the promise expressed by “**无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。” in “2. 链接音频(linked audio)—— 这是上游既定设计,不是 bug”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “**无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。” with the scenario below and register test:crates/opentake-render/tests/completion_1c3d81a8b3ab2d59.rs#completion_1c3d81a8b3ab2d59_do_not_create_a_linked_audio_track_when_probe_pr
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “**无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Do not create a linked audio track when probe proves a video has zero audio channels.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_1c3d81a8b3ab2d59.rs#completion_1c3d81a8b3ab2d59_do_not_create_a_linked_audio_track_when_probe_pr.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/mcp/dispatch.rs#add_clips_does_not_link_audio_when_source_has_no_audio` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#insert_clips_does_not_link_audio_when_source_has_no_audio` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and adjudicate the inherited baseline**

  - Run: `cargo test -p opentake-agent add_clips_does_not_link_audio_when_source_has_no_audio`
  - Run: `cargo test -p opentake-agent insert_clips_does_not_link_audio_when_source_has_no_audio`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

  Result: The current baseline was already GREEN, not RED. The two exact owning tests entered history in `a2f34cb` with the Agent linked-audio fix, while the zero-channel probe regression entered in `9920468`. No artificial failure was introduced merely to recreate historical RED; the current acceptance run executed both Agent tests and the probe regression directly.

- [x] **Step 3: Verify the existing vertical slice and update its acceptance record**

  Modify only `crates/opentake-agent/src/mcp/dispatch.rs`, `crates/opentake-ops/src/ops/place.rs`, `docs/architecture/EDITING-ENGINE-PLAN.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent add_clips_does_not_link_audio_when_source_has_no_audio`
  - Run: `cargo test -p opentake-agent insert_clips_does_not_link_audio_when_source_has_no_audio`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Result: PASS. `cargo test -p opentake-agent does_not_link_audio_when_source_has_no_audio` executed both exact add/insert tests with 2/2 passing, and `cargo test -p opentake-media video_with_zero_channel_audio_has_no_audio` executed the probe test with 1/1 passing. The complete workspace gate had already passed on the same executable source immediately before this documentation-only reclassification. In the rebuilt release `.app`, an isolated project imported a five-second H.264 file with no audio stream, persisted `hasAudio:false`, created only a V1 clip with no `linkGroupId`, played normally, and exported 150 frames. Independent FFprobe inspection of the exported MP4 found one H.264 1280×720/30 fps stream and no audio stream. Runtime evidence: [`linked-audio-real-device-2026-08-01.md`](../runtime-artifacts/automated/linked-audio-real-device-2026-08-01.md).

### Task 15: MR-hdr-proxy-account-composite (implementation-slice-d2a7a5861f5ebc9b)

**Covered records:**
- `requirement-3333d0cfd4a2fa31` (requirement)

**Files:**
- Modify: `docs/architecture/HANDOFF-2026-07.md`
- Test (reviewed-planned): `crates/opentake-render/tests/composite_acceptance.rs#hdr_proxy_account_children_close_one_composite_acceptance`

**Candidate-bound contracts:**

#### requirement-3333d0cfd4a2fa31

- Candidate/source: `doc-4c04a063f12b37c9` at `docs/architecture/HANDOFF-2026-07.md:171` (requirement)
- Expected behavior: HDR, proxy media, and account state work as integrated desktop features.
- Resolution: `reviewed-mapping-report:MR-hdr-proxy-account-composite` — HDR, proxy media and account state belong to three different product owners and must be split.
- Exact acceptance contract:
  - Implement HDR metadata/color handling and validate preview/export.
  - Implement proxy creation, switching, relink, and persistence.
  - Finish account/provider session integration and cover offline/reopen behavior.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/composite_acceptance.rs#hdr_proxy_account_children_close_one_composite_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test composite_acceptance hdr_proxy_account_children_close_one_composite_acceptance -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/HANDOFF-2026-07.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test composite_acceptance hdr_proxy_account_children_close_one_composite_acceptance -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

  Current result (2026-08-01): product verification is GREEN (`cargo test
  --workspace --no-fail-fast`, workspace Clippy with `-D warnings`, Rust fmt,
  Web 93 files / 824 tests, and the production build). The exact completion
  audit ran 206 tests with 205 passing; its only failure is the protected
  `repository-files.json` inventory omitting previously tracked files. This
  step remains unchecked until that user-owned inventory is reconciled rather
  than overwritten by this slice.

Functional completion evidence (2026-08-01): the owning composite test first
failed because the target did not exist, then passed after separate HDR, proxy,
and account children were implemented and verified. Packaged macOS testing
found and fixed bundled-FFmpeg HDR black output, missing asset-protocol scope
for proxies, and a final no-follow ancestor-directory boundary. The latest
ad-hoc-signed app and its DMG copy pass strict deep verification; packaged GUI
remove/recreate/persist/playback and original-only export pass. Full receipt:
`../runtime-artifacts/automated/hdr-proxy-account-real-device-2026-08-01.md`.

### Task 16: MR-bounded-audio-streaming (implementation-slice-d1864a5db0605004)

**Covered records:**
- `requirement-b598e7822e10bf65` (requirement)

**Files:**
- Modify: `src-tauri/src/playback/audio.rs#mix_timeline_stereo`
- Modify: `src-tauri/src/export.rs#mix_timeline_audio`
- Modify: `crates/opentake-media/src/encode/mod.rs#VideoEncoder`
- Modify: `docs/architecture/HANDOFF-2026-07.md`
- Test (existing-owned): `src-tauri/src/playback/audio.rs#large_mix_observes_cancellation_between_chunks`
- Test (reviewed-planned): `src-tauri/src/playback/audio.rs#long_timeline_mix_has_constant_peak_allocation_and_matches_short_reference`

**Candidate-bound contracts:**

#### requirement-b598e7822e10bf65

- Candidate/source: `doc-af7c3db39bbed7c9` at `docs/architecture/HANDOFF-2026-07.md:186` (requirement)
- Expected behavior: Long timelines stream bounded audio chunks without loading the entire mix.
- Resolution: `reviewed-mapping-report:MR-bounded-audio-streaming` — Current chunking serves cancellation loops but the whole timeline mix is still allocated in memory.
- Exact acceptance contract:
  - Replace preload-mix playback with bounded chunk scheduling.
  - Handle seek, pause, resume, underrun, and cancellation.
  - Add long-duration memory and A/V sync runtime tests.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/playback/audio.rs#large_mix_observes_cancellation_between_chunks` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/playback/audio.rs#long_timeline_mix_has_constant_peak_allocation_and_matches_short_reference` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri large_mix_observes_cancellation_between_chunks`
  - Run: `cargo test -p opentake-tauri long_timeline_mix_has_constant_peak_allocation_and_matches_short_reference`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/playback/audio.rs#mix_timeline_stereo`, `src-tauri/src/export.rs#mix_timeline_audio`, `crates/opentake-media/src/encode/mod.rs#VideoEncoder`, `docs/architecture/HANDOFF-2026-07.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri large_mix_observes_cancellation_between_chunks`
  - Run: `cargo test -p opentake-tauri long_timeline_mix_has_constant_peak_allocation_and_matches_short_reference`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Completed 2026-08-01. The reviewed RED first failed because
`mix_stereo_windows` did not exist. Playback now uses a bounded 2-second/4-slot
generation queue; seek, pause/resume, underrun, cancellation, and teardown are
covered. Export and save-as-WAV stream the same bounded windows to file-backed
output. Both reviewed focused tests, the workspace gate, strict Clippy, Web
tests/build, and packaged macOS playback/export/WAV probes pass. Full receipt:
`../runtime-artifacts/automated/bounded-audio-streaming-real-device-2026-08-01.md`.

### Task 17: MR-renderer-debt-composite (implementation-slice-827681eebfb87194)

**Covered records:**
- `requirement-4b55f25a5196f7e3` (requirement)

**Files:**
- Modify: `docs/architecture/HANDOFF-2026-07.md`
- Test (reviewed-planned): `crates/opentake-render/tests/composite_acceptance.rs#renderer_debt_children_close_one_composite_acceptance`

**Candidate-bound contracts:**

#### requirement-4b55f25a5196f7e3

- Candidate/source: `doc-41a8a7a62b46a85e` at `docs/architecture/HANDOFF-2026-07.md:191` (requirement)
- Expected behavior: The listed renderer, packaging, settings, solo, and stub debt is closed with automated evidence.
- Resolution: `reviewed-mapping-report:MR-renderer-debt-composite` — Renderer, packaging, settings, solo and stub debt cannot be owned or accepted as one implementation slice.
- Exact acceptance contract:
  - Implement Lottie/motion materialization, track-solo playback/export semantics, settings/model/provider persistence, and all residual InspectMedia/generate/upscale/motion/batch library dispatch paths.
  - Bundle verified FFmpeg/ffprobe sidecars for packaged macOS/Windows and return typed unsupported errors for any renderer/tool capability still unavailable; advertise no placeholder success.
  - Pass motion/Lottie pixel fixtures, solo A/V mix tests, settings secret/restart/list_models tests, per-tool schema/failure/undo matrix, and installed-app probe/decode/playback/export smoke on both targets.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/composite_acceptance.rs#renderer_debt_children_close_one_composite_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test composite_acceptance renderer_debt_children_close_one_composite_acceptance -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/HANDOFF-2026-07.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test composite_acceptance renderer_debt_children_close_one_composite_acceptance -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

### Task 18: MR-playback-route-lifecycle-complete (implementation-slice-843431dc47b8e0d0)

**Covered records:**
- `requirement-dd35062c2778f365` (requirement)
- `requirement-b8176cd58b4673fc` (requirement)
- `requirement-e6e1400ed457bbb9` (requirement)
- `requirement-3aa21ae6148b5fcd` (requirement)
- `requirement-55ea08b7a51b30ed` (requirement)
- `requirement-34258095f740e104` (requirement)

**Files:**
- Modify: `web/src/components/preview/playbackRoute.ts#resolveTimelinePlaybackRoute`
- Modify: `web/src/components/preview/nativePlaybackSession.ts`
- Modify: `web/src/components/preview/rustFrameBuffer.ts`
- Modify: `src-tauri/src/playback/engine.rs#PlaybackEngine`
- Modify: `src-tauri/src/playback/resolver.rs#PlaybackResolverState`
- Modify: `docs/architecture/PLAYBACK-ENGINE.md`
- Test (existing-owned): `web/src/components/preview/playbackRoute.test.ts#routes plain forward video to WebKit`
- Test (existing-owned): `web/src/components/preview/nativePlaybackSession.test.ts#publishes only increasing matching frame sequences`
- Test (existing-owned): `web/src/components/preview/rustFrameBuffer.test.ts#keeps two stable Rust frame image slots mounted`
- Test (existing-owned): `src-tauri/src/playback/resolver.rs#drain_propagates_stream_failure_instead_of_freezing_cache`

**Candidate-bound contracts:**

#### requirement-dd35062c2778f365

- Candidate/source: `doc-75908a18e877e120` at `docs/architecture/PLAYBACK-ENGINE.md:6` (requirement)
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:6 under “Capability route is the sole authority” (heading), the source “## Capability route is the sole authority” requires this exact behavior: The playback subsystem implements capability route is the sole authority with focused route/lifecycle tests.
- Resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete` — Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Exact acceptance contract:
  - Source binding: docs/architecture/PLAYBACK-ENGINE.md:6; signal=heading; heading=Capability route is the sole authority; candidate=## Capability route is the sole authority
  - Expected behavior: The playback subsystem implements capability route is the sole authority with focused route/lifecycle tests. This closes only the promise expressed by “Capability route is the sole authority” in “Capability route is the sole authority”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Capability route is the sole authority” with the scenario below and register test:crates/opentake-render/tests/completion_75908a18e877e120.rs#completion_75908a18e877e120_the_playback_subsystem_implements_capability_rou
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Capability route is the sole authority”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements capability route is the sole authority with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_75908a18e877e120.rs#completion_75908a18e877e120_the_playback_subsystem_implements_capability_rou.

#### requirement-b8176cd58b4673fc

- Candidate/source: `doc-a6e62065f311d1d7` at `docs/architecture/PLAYBACK-ENGINE.md:24` (requirement)
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:24 under “WebKit route” (heading), the source “## WebKit route” requires this exact behavior: The playback subsystem implements webkit route with focused route/lifecycle tests.
- Resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete` — Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Exact acceptance contract:
  - Source binding: docs/architecture/PLAYBACK-ENGINE.md:24; signal=heading; heading=WebKit route; candidate=## WebKit route
  - Expected behavior: The playback subsystem implements webkit route with focused route/lifecycle tests. This closes only the promise expressed by “WebKit route” in “WebKit route”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “WebKit route” with the scenario below and register test:web/src/__tests__/completion/doc-a6e62065f311d1d7.test.ts#completion_a6e62065f311d1d7_the_playback_subsystem_implements_webkit_route_w
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “WebKit route”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements webkit route with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-a6e62065f311d1d7.test.ts#completion_a6e62065f311d1d7_the_playback_subsystem_implements_webkit_route_w.

#### requirement-e6e1400ed457bbb9

- Candidate/source: `doc-30a2bd351d4d52a2` at `docs/architecture/PLAYBACK-ENGINE.md:32` (requirement)
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:32 under “Rust route and exact publication” (heading), the source “## Rust route and exact publication” requires this exact behavior: The playback subsystem implements rust route and exact publication with focused route/lifecycle tests.
- Resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete` — Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Exact acceptance contract:
  - Source binding: docs/architecture/PLAYBACK-ENGINE.md:32; signal=heading; heading=Rust route and exact publication; candidate=## Rust route and exact publication
  - Expected behavior: The playback subsystem implements rust route and exact publication with focused route/lifecycle tests. This closes only the promise expressed by “Rust route and exact publication” in “Rust route and exact publication”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Rust route and exact publication” with the scenario below and register test:crates/opentake-render/tests/completion_30a2bd351d4d52a2.rs#completion_30a2bd351d4d52a2_the_playback_subsystem_implements_rust_route_and
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Rust route and exact publication”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements rust route and exact publication with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_30a2bd351d4d52a2.rs#completion_30a2bd351d4d52a2_the_playback_subsystem_implements_rust_route_and.

#### requirement-3aa21ae6148b5fcd

- Candidate/source: `doc-ac35095c99a0761b` at `docs/architecture/PLAYBACK-ENGINE.md:52` (requirement)
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:52 under “Lifecycle, control, and bootstrap” (heading), the source “## Lifecycle, control, and bootstrap” requires this exact behavior: The playback subsystem implements lifecycle, control, and bootstrap with focused route/lifecycle tests.
- Resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete` — Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Exact acceptance contract:
  - Source binding: docs/architecture/PLAYBACK-ENGINE.md:52; signal=heading; heading=Lifecycle, control, and bootstrap; candidate=## Lifecycle, control, and bootstrap
  - Expected behavior: The playback subsystem implements lifecycle, control, and bootstrap with focused route/lifecycle tests. This closes only the promise expressed by “Lifecycle, control, and bootstrap” in “Lifecycle, control, and bootstrap”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Lifecycle, control, and bootstrap” with the scenario below and register test:crates/opentake-render/tests/completion_ac35095c99a0761b.rs#completion_ac35095c99a0761b_the_playback_subsystem_implements_lifecycle_cont
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Lifecycle, control, and bootstrap”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements lifecycle, control, and bootstrap with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_ac35095c99a0761b.rs#completion_ac35095c99a0761b_the_playback_subsystem_implements_lifecycle_cont.

#### requirement-55ea08b7a51b30ed

- Candidate/source: `doc-cd317d4fe595484f` at `docs/architecture/PLAYBACK-ENGINE.md:78` (requirement)
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:78 under “Retained-frame handoff” (heading), the source “## Retained-frame handoff” requires this exact behavior: The playback subsystem implements retained-frame handoff with focused route/lifecycle tests.
- Resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete` — Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Exact acceptance contract:
  - Source binding: docs/architecture/PLAYBACK-ENGINE.md:78; signal=heading; heading=Retained-frame handoff; candidate=## Retained-frame handoff
  - Expected behavior: The playback subsystem implements retained-frame handoff with focused route/lifecycle tests. This closes only the promise expressed by “Retained-frame handoff” in “Retained-frame handoff”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Retained-frame handoff” with the scenario below and register test:crates/opentake-render/tests/completion_cd317d4fe595484f.rs#completion_cd317d4fe595484f_the_playback_subsystem_implements_retained_frame
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Retained-frame handoff”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements retained-frame handoff with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_cd317d4fe595484f.rs#completion_cd317d4fe595484f_the_playback_subsystem_implements_retained_frame.

#### requirement-34258095f740e104

- Candidate/source: `doc-428f02e756802f9f` at `docs/architecture/PLAYBACK-ENGINE.md:88` (requirement)
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:88 under “Project/source identity and prewarm/cache” (heading), the source “## Project/source identity and prewarm/cache” requires this exact behavior: The playback subsystem implements project/source identity and prewarm/cache with focused route/lifecycle tests.
- Resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete` — Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Exact acceptance contract:
  - Source binding: docs/architecture/PLAYBACK-ENGINE.md:88; signal=heading; heading=Project/source identity and prewarm/cache; candidate=## Project/source identity and prewarm/cache
  - Expected behavior: The playback subsystem implements project/source identity and prewarm/cache with focused route/lifecycle tests. This closes only the promise expressed by “Project/source identity and prewarm/cache” in “Project/source identity and prewarm/cache”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Project/source identity and prewarm/cache” with the scenario below and register test:crates/opentake-project/tests/completion_428f02e756802f9f.rs#completion_428f02e756802f9f_the_playback_subsystem_implements_project_source
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Project/source identity and prewarm/cache”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements project/source identity and prewarm/cache with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_428f02e756802f9f.rs#completion_428f02e756802f9f_the_playback_subsystem_implements_project_source.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/preview/playbackRoute.test.ts#routes plain forward video to WebKit` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/nativePlaybackSession.test.ts#publishes only increasing matching frame sequences` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/rustFrameBuffer.test.ts#keeps two stable Rust frame image slots mounted` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/playback/resolver.rs#drain_propagates_stream_failure_instead_of_freezing_cache` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/preview/playbackRoute.test.ts -t "routes plain forward video to WebKit"`
  - Run: `pnpm -C web test -- --run src/components/preview/nativePlaybackSession.test.ts -t "publishes only increasing matching frame sequences"`
  - Run: `pnpm -C web test -- --run src/components/preview/rustFrameBuffer.test.ts -t "keeps two stable Rust frame image slots mounted"`
  - Run: `cargo test -p opentake-tauri drain_propagates_stream_failure_instead_of_freezing_cache`

  Expected: FAIL because one or more of the 6 candidate-bound contracts are not yet satisfied.

  Reconciliation note (2026-08-01): this slice was implemented and reviewed in
  the historical commits already named by `PLAYBACK-ENGINE.md` before this
  generated checklist existed. The current baseline therefore produced GREEN;
  no approved implementation was reverted merely to manufacture a new RED.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/preview/playbackRoute.ts#resolveTimelinePlaybackRoute`, `web/src/components/preview/nativePlaybackSession.ts`, `web/src/components/preview/rustFrameBuffer.ts`, `src-tauri/src/playback/engine.rs#PlaybackEngine`, `src-tauri/src/playback/resolver.rs#PlaybackResolverState`, `docs/architecture/PLAYBACK-ENGINE.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/preview/playbackRoute.test.ts -t "routes plain forward video to WebKit"`
  - Run: `pnpm -C web test -- --run src/components/preview/nativePlaybackSession.test.ts -t "publishes only increasing matching frame sequences"`
  - Run: `pnpm -C web test -- --run src/components/preview/rustFrameBuffer.test.ts -t "keeps two stable Rust frame image slots mounted"`
  - Run: `cargo test -p opentake-tauri drain_propagates_stream_failure_instead_of_freezing_cache`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Verified 2026-08-01 as a reconciliation-only slice: all four exact owning tests
pass on the current implementation; the complete Web suite is 93 files / 824
tests, and the Task16 workspace gate already passed without an intervening code
change. The final packaged application advanced and paused both WebKit and Rust
routes, then reopened the WebKit project at its own zero playhead and 60-second
duration after the Rust project boundary. Full receipt:
`../runtime-artifacts/automated/playback-route-lifecycle-real-device-2026-08-01.md`.

### Task 19: MR-release-readiness-composite (implementation-slice-dd9855c810140649)

**Covered records:**
- `requirement-36fc46ebba504588` (requirement)

**Files:**
- Modify: `docs/architecture/PLAYBACK-ENGINE.md`
- Test (reviewed-planned): `crates/opentake-render/tests/composite_acceptance.rs#release_readiness_children_close_one_composite_acceptance`

**Candidate-bound contracts:**

#### requirement-36fc46ebba504588

- Candidate/source: `doc-ef91c43e6cb92a53` at `docs/architecture/PLAYBACK-ENGINE.md:117` (requirement)
- Expected behavior: Playback/export is release-ready across packaged macOS and Windows with all declared capabilities rendered or explicitly rejected.
- Resolution: `reviewed-mapping-report:MR-release-readiness-composite` — Packaged macOS and Windows release readiness is a cross-slice runtime evidence gate, not one product implementation.
- Exact acceptance contract:
  - Complete installed-app export UI artifact and packaged FFmpeg/sidecar validation.
  - Close Windows WebView2/CSP/sidecar and signing/notarization acceptance.
  - Implement or fail closed for Lottie, polygon masks, unsupported effects, composited reverse/speed, and complete ProRes/A/V device probes.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/composite_acceptance.rs#release_readiness_children_close_one_composite_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test composite_acceptance release_readiness_children_close_one_composite_acceptance -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/PLAYBACK-ENGINE.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test composite_acceptance release_readiness_children_close_one_composite_acceptance -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

### Task 20: MR-advanced-shader-composite (implementation-slice-0720e74ad73f5976)

**Covered records:**
- `requirement-ec4d078ac55f6037` (requirement)

**Files:**
- Modify: `docs/architecture/ROADMAP.md`
- Test (reviewed-planned): `crates/opentake-render/tests/composite_acceptance.rs#advanced_shader_children_close_one_composite_acceptance`

**Candidate-bound contracts:**

#### requirement-ec4d078ac55f6037

- Candidate/source: `doc-c79d6d91b3f2e347` at `docs/architecture/ROADMAP.md:35` (requirement)
- Expected behavior: Complete the advanced shader/effect framework, including transitions, masks, LUT/curves, and preview/export parity.
- Resolution: `reviewed-mapping-report:MR-advanced-shader-composite` — Transitions, masks, LUT and curves have independent owners and child slices.
- Exact acceptance contract:
  - Implement persisted transitions, polygon masks, RGB/HSL/LUT grading, and every advertised shader effect with typed unsupported-mode errors.
  - Expose parameter editing and undo/redo for each effect family and keep one capability route for preview/export.
  - Add GPU pixel fixtures for default/non-default parameters and assert preview/export parity on supported hardware plus deterministic fallback on unsupported hardware.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/composite_acceptance.rs#advanced_shader_children_close_one_composite_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test composite_acceptance advanced_shader_children_close_one_composite_acceptance -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/ROADMAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test composite_acceptance advanced_shader_children_close_one_composite_acceptance -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

### Task 21: MR-generation-job-serde-complete (implementation-slice-103e52462204b36a)

**Covered records:**
- `requirement-f28087239427d70e` (requirement)

**Files:**
- Modify: `crates/opentake-gen/src/job.rs#GenerationJob`
- Modify: `docs/modules/opentake-gen/client-transport.md`
- Test (existing-owned): `crates/opentake-gen/src/job.rs#deserializes_proxy_shape_with_id`
- Test (existing-owned): `crates/opentake-gen/src/job.rs#deserializes_upstream_shape_with_underscore_id`

**Candidate-bound contracts:**

#### requirement-f28087239427d70e

- Candidate/source: `doc-9900e773f8c063a8` at `docs/modules/opentake-gen/client-transport.md:59` (requirement)
- Expected behavior: At docs/modules/opentake-gen/client-transport.md:59 under “`job.rs` —— 统一 Job 抽象” (gap-marker), the source “- 字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。” requires this exact behavior: Decode both proxy id and upstream _id job shapes while tolerating missing optional result, error, cost, and completion fields.
- Resolution: `reviewed-mapping-report:MR-generation-job-serde-complete` — Both id shapes and absent optional fields are handled and directly tested.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-gen/client-transport.md:59; signal=gap-marker; heading=`job.rs` —— 统一 Job 抽象; candidate=- 字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。
  - Expected behavior: Decode both proxy id and upstream _id job shapes while tolerating missing optional result, error, cost, and completion fields. This closes only the promise expressed by “字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。” in “`job.rs` —— 统一 Job 抽象”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。” with the scenario below and register test:crates/opentake-project/tests/completion_9900e773f8c063a8.rs#completion_9900e773f8c063a8_decode_both_proxy_id_and_upstream_id_job_shapes_
  - Initial state/input/event: start from the smallest valid fixture for “Decode both proxy id and upstream _id job shapes while tolerating missing optional result, error, cost, and completion fields.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。”.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Decode both proxy id and upstream _id job shapes while tolerating missing optional result, error, cost, and completion fields.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_9900e773f8c063a8.rs#completion_9900e773f8c063a8_decode_both_proxy_id_and_upstream_id_job_shapes_.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-gen/src/job.rs#deserializes_proxy_shape_with_id` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-gen/src/job.rs#deserializes_upstream_shape_with_underscore_id` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-gen deserializes_proxy_shape_with_id`
  - Run: `cargo test -p opentake-gen deserializes_upstream_shape_with_underscore_id`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

  Reconciliation note (2026-08-01): the serde alias/default implementation and
  both named tests predate this generated checklist, so the current baseline was
  already GREEN. The proxy-shape owner was extended to assert all four absent
  optional fields rather than reverting valid code to manufacture RED.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-gen/src/job.rs#GenerationJob`, `docs/modules/opentake-gen/client-transport.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-gen deserializes_proxy_shape_with_id`
  - Run: `cargo test -p opentake-gen deserializes_upstream_shape_with_underscore_id`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Verified 2026-08-01. Both exact job-shape tests and the complete
`opentake-gen` package pass; formatting and diff checks pass. The immediately
preceding unchanged-code workspace gate passed in Task16.

### Task 22: MR-cli-sidecar-boundary-complete (implementation-slice-bdb1294b5e15ccf0)

**Covered records:**
- `requirement-e71abff7d9b41127` (requirement)
- `requirement-d7bd977b01924877` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/ff.rs#ffmpeg_path`
- Modify: `crates/opentake-media/src/ff.rs#ffprobe_path`
- Modify: `crates/opentake-media/Cargo.toml`
- Modify: `docs/modules/opentake-media/OVERVIEW.md`
- Modify: `docs/modules/opentake-media/probe-ff.md`
- Test (existing-owned): `crates/opentake-media/src/ff.rs#env_override_is_respected_for_ffmpeg`
- Test (existing-owned): `crates/opentake-media/src/ff.rs#default_ffprobe_is_ffprobe`

**Candidate-bound contracts:**

#### requirement-e71abff7d9b41127

- Candidate/source: `doc-ee5f3f6b2ccb9cc6` at `docs/modules/opentake-media/OVERVIEW.md:55` (requirement)
- Expected behavior: At docs/modules/opentake-media/OVERVIEW.md:55 under “FFmpeg sidecar 编解码（与 SPEC 的关键实现偏差）” (gap-marker), the source “**实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](probe-ff.md)。” requires this exact behavior: Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding.
- Resolution: `reviewed-mapping-report:MR-cli-sidecar-boundary-complete` — The media crate deliberately uses ffmpeg-sidecar and does not link a libav ABI binding.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-media/OVERVIEW.md:55; signal=gap-marker; heading=FFmpeg sidecar 编解码（与 SPEC 的关键实现偏差）; candidate=**实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](probe-ff.md)。
  - Expected behavior: Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding. This closes only the promise expressed by “*实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](probe-ff.md)。” in “FFmpeg sidecar 编解码（与 SPEC 的关键实现偏差）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “*实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](probe-ff.md)。” with the scenario below and register test:crates/opentake-project/tests/completion_ee5f3f6b2ccb9cc6.rs#completion_ee5f3f6b2ccb9cc6_use_the_ffmpeg_ffprobe_sidecar_boundary_rather_t
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “*实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](probe-ff.md)。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_ee5f3f6b2ccb9cc6.rs#completion_ee5f3f6b2ccb9cc6_use_the_ffmpeg_ffprobe_sidecar_boundary_rather_t.

#### requirement-d7bd977b01924877

- Candidate/source: `doc-fd42ddbda4988918` at `docs/modules/opentake-media/probe-ff.md:17` (requirement)
- Expected behavior: At docs/modules/opentake-media/probe-ff.md:17 under “关键决策：为何 CLI sidecar 而非 libav 绑定” (gap-marker), the source “`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。” requires this exact behavior: Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding.
- Resolution: `reviewed-mapping-report:MR-cli-sidecar-boundary-complete` — The media crate deliberately uses ffmpeg-sidecar and does not link a libav ABI binding.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-media/probe-ff.md:17; signal=gap-marker; heading=关键决策：为何 CLI sidecar 而非 libav 绑定; candidate=`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。
  - Expected behavior: Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding. This closes only the promise expressed by “`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。” in “关键决策：为何 CLI sidecar 而非 libav 绑定”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。” with the scenario below and register test:crates/opentake-project/tests/completion_fd42ddbda4988918.rs#completion_fd42ddbda4988918_use_the_ffmpeg_ffprobe_sidecar_boundary_rather_t
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_fd42ddbda4988918.rs#completion_fd42ddbda4988918_use_the_ffmpeg_ffprobe_sidecar_boundary_rather_t.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/src/ff.rs#env_override_is_respected_for_ffmpeg` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/ff.rs#default_ffprobe_is_ffprobe` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media env_override_is_respected_for_ffmpeg`
  - Run: `cargo test -p opentake-media default_ffprobe_is_ffprobe`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

  RED recorded 2026-08-01: the corrected owning tests failed to compile because
  the pure `resolve_cli_path` decision boundary did not exist.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/ff.rs#ffmpeg_path`, `crates/opentake-media/src/ff.rs#ffprobe_path`, `crates/opentake-media/Cargo.toml`, `docs/modules/opentake-media/OVERVIEW.md`, `docs/modules/opentake-media/probe-ff.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media env_override_is_respected_for_ffmpeg`
  - Run: `cargo test -p opentake-media default_ffprobe_is_ffprobe`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Completed 2026-08-01. The resolver now has a deterministic tested priority of
environment override, regular non-symlink packaged sidecar, then PATH. Exact
owners and package regressions pass. The final macOS app contains runnable
FFmpeg/ffprobe 6.0 and has no dynamic libav link; its current `--enable-nonfree`
configuration remains a distribution-license blocker outside this boundary.
Receipt: `../runtime-artifacts/automated/cli-sidecar-boundary-real-device-2026-08-01.md`.

### Task 23: MR-motion-decoder-injection-complete (implementation-slice-ee5b0fb9f6f3c487)

**Covered records:**
- `requirement-b08d2435a038bd02` (requirement)
- `requirement-4e59a00921e5e252` (requirement)

**Files:**
- Modify: `crates/opentake-motion/src/integration.rs#MotionClipSource::new`
- Modify: `docs/modules/opentake-motion/OVERVIEW.md`
- Modify: `docs/modules/opentake-motion/integration.md`
- Test (existing-owned): `crates/opentake-motion/src/integration.rs#decoded_frame_returns_rgba_of_right_shape`
- Test (existing-owned): `crates/opentake-motion/tests/pipeline.rs#full_pipeline_render_cache_and_ingest`

**Candidate-bound contracts:**

#### requirement-b08d2435a038bd02

- Candidate/source: `doc-01ed7e50adfe68f9` at `docs/modules/opentake-motion/OVERVIEW.md:88` (requirement)
- Expected behavior: At docs/modules/opentake-motion/OVERVIEW.md:88 under “与 render 的集成桥” (gap-marker), the source “- 帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。” requires this exact behavior: Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion.
- Resolution: `reviewed-mapping-report:MR-motion-decoder-injection-complete` — Frame decoding is injected through a closure and the production crate does not own an image decoder dependency.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-motion/OVERVIEW.md:88; signal=gap-marker; heading=与 render 的集成桥; candidate=- 帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。
  - Expected behavior: Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion. This closes only the promise expressed by “帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。” in “与 render 的集成桥”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。” with the scenario below and register test:crates/opentake-project/tests/completion_01ed7e50adfe68f9.rs#completion_01ed7e50adfe68f9_inject_frame_decoding_into_motionclipsource_with
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_01ed7e50adfe68f9.rs#completion_01ed7e50adfe68f9_inject_frame_decoding_into_motionclipsource_with.

#### requirement-4e59a00921e5e252

- Candidate/source: `doc-c837d207c03a37cb` at `docs/modules/opentake-motion/integration.md:26` (requirement)
- Expected behavior: At docs/modules/opentake-motion/integration.md:26 under “解码器注入（为什么不硬接 PNG 库）” (gap-marker), the source “所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。” requires this exact behavior: Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion.
- Resolution: `reviewed-mapping-report:MR-motion-decoder-injection-complete` — Frame decoding is injected through a closure and the production crate does not own an image decoder dependency.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-motion/integration.md:26; signal=gap-marker; heading=解码器注入（为什么不硬接 PNG 库）; candidate=所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。
  - Expected behavior: Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion. This closes only the promise expressed by “所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。” in “解码器注入（为什么不硬接 PNG 库）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。” with the scenario below and register test:crates/opentake-project/tests/completion_c837d207c03a37cb.rs#completion_c837d207c03a37cb_inject_frame_decoding_into_motionclipsource_with
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_c837d207c03a37cb.rs#completion_c837d207c03a37cb_inject_frame_decoding_into_motionclipsource_with.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-motion/src/integration.rs#decoded_frame_returns_rgba_of_right_shape` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/tests/pipeline.rs#full_pipeline_render_cache_and_ingest` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-motion decoded_frame_returns_rgba_of_right_shape`
  - Run: `cargo test -p opentake-motion --test pipeline full_pipeline_render_cache_and_ingest -- --exact`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

  Reconciliation note (2026-08-01): the closure-injection implementation and
  both named owners predate this generated checklist, so the current baseline
  is already GREEN; valid code was not reverted to manufacture RED.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-motion/src/integration.rs#MotionClipSource::new`, `docs/modules/opentake-motion/OVERVIEW.md`, `docs/modules/opentake-motion/integration.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-motion decoded_frame_returns_rgba_of_right_shape`
  - Run: `cargo test -p opentake-motion --test pipeline full_pipeline_render_cache_and_ingest -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Verified 2026-08-01. Both exact owners, all 58 motion tests, formatting, and
warnings-denied motion Clippy pass. Production dependencies contain no image or
ffmpeg decoder; `image` remains test-only. This closes decoder injection only,
not the separately owned desktop motion/Lottie materialization.

### Task 24: MR-native-chromium (implementation-slice-1687c4455b65f8a6)

**Covered records:**
- `requirement-d11b3310d27d0350` (requirement)

**Files:**
- Modify: `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer::render`
- Modify: `crates/opentake-motion/src/integration.rs#MotionClipSource`
- Modify: `docs/modules/opentake-motion/OVERVIEW.md`
- Test (existing-owned): `crates/opentake-motion/src/renderer.rs#chromium_skeleton_reports_unavailable_not_panic`
- Test (reviewed-planned): `crates/opentake-motion/tests/chromium.rs#virtual_time_network_csp_timeout_cleanup_and_frame_identity`

**Candidate-bound contracts:**

#### requirement-d11b3310d27d0350

- Candidate/source: `doc-ff0fb2b094c8ba65` at `docs/modules/opentake-motion/OVERVIEW.md:127` (requirement)
- Expected behavior: Provide the deferred native Headless Chromium renderer with deterministic virtual time and fail-closed network/CSP/timeout controls.
- Resolution: `reviewed-mapping-report:MR-native-chromium` — The tracked renderer is a fail-closed skeleton and explicitly reports that live Chromium is not implemented.
- Exact acceptance contract:
  - When the chromium feature is enabled, locate/launch a supported browser and render every requested frame instead of returning RendererUnavailable.
  - Enforce request interception allowlists, CSP, document limits, cancellation, timeout, deterministic clock, and no ambient filesystem/network access.
  - Integration tests render a fixed animation twice byte-identically and cover blocked network, timeout, crash, malformed source, and cancellation.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-motion/src/renderer.rs#chromium_skeleton_reports_unavailable_not_panic` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/tests/chromium.rs#virtual_time_network_csp_timeout_cleanup_and_frame_identity` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-motion chromium_skeleton_reports_unavailable_not_panic`
  - Run: `cargo test -p opentake-motion --test chromium virtual_time_network_csp_timeout_cleanup_and_frame_identity -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

  RED recorded 2026-08-01 with `--features chromium`: the planned test failed
  to compile because cancellation, browser discovery/path injection, and the
  live backend did not exist. The feature flag is required to exercise the live
  owner; the unfeatured command intentionally verifies only fail-closed
  `RendererUnavailable` behavior.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer::render`, `crates/opentake-motion/src/integration.rs#MotionClipSource`, `docs/modules/opentake-motion/OVERVIEW.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-motion chromium_skeleton_reports_unavailable_not_panic`
  - Run: `cargo test -p opentake-motion --test chromium virtual_time_network_csp_timeout_cleanup_and_frame_identity -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

  GREEN verified 2026-08-01 against Google Chrome 150.0.7871.187. The live
  exact owner rendered a fixed animation twice byte-identically, advanced
  visible virtual-time frames, decoded a real browser PNG through
  `MotionClipSource`, allowed an exact loopback origin, blocked a disallowed
  origin, fused a runaway script, handled process crash and malformed source,
  cancelled an in-flight render, removed partial frames, and left no browser
  profile. The existing owner passes both default and feature-enabled builds.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01. Formatting, default and feature-enabled warnings-denied
  Clippy, all 59 feature-enabled motion tests, and the full workspace Rust suite
  pass. The first workspace attempt stopped only because generated debug
  artifacts filled the disk; after deleting `target/debug/deps` and
  `target/debug/incremental` (release bundle preserved), the identical command
  passed. Runtime receipt:
  `runtime-artifacts/automated/headless-chromium-real-device-2026-08-01.md`.

### Task 25: MR-motion-missing-frame-complete (implementation-slice-f16a70f238444e28)

**Covered records:**
- `requirement-758f9639111ca4be` (requirement)

**Files:**
- Modify: `crates/opentake-motion/src/integration.rs#MotionClipSource::frame`
- Modify: `docs/modules/opentake-motion/integration.md`
- Test (existing-owned): `crates/opentake-motion/src/integration.rs#missing_decoder_result_is_none`

**Candidate-bound contracts:**

#### requirement-758f9639111ca4be

- Candidate/source: `doc-379e138c3e674f1b` at `docs/modules/opentake-motion/integration.md:32` (requirement)
- Expected behavior: At docs/modules/opentake-motion/integration.md:32 under “解码器注入（为什么不硬接 PNG 库）” (gap-marker), the source “解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。” requires this exact behavior: Treat a missing/corrupt decoded motion frame as an absent source frame.
- Resolution: `reviewed-mapping-report:MR-motion-missing-frame-complete` — Missing or corrupt decoder output is represented as an absent source frame.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-motion/integration.md:32; signal=gap-marker; heading=解码器注入（为什么不硬接 PNG 库）; candidate=解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。
  - Expected behavior: Treat a missing/corrupt decoded motion frame as an absent source frame. This closes only the promise expressed by “解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。” in “解码器注入（为什么不硬接 PNG 库）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。” with the scenario below and register test:crates/opentake-project/tests/completion_379e138c3e674f1b.rs#completion_379e138c3e674f1b_treat_a_missing_corrupt_decoded_motion_frame_as_
  - Initial state/input/event: start from the smallest valid fixture for “Treat a missing/corrupt decoded motion frame as an absent source frame.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。”.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Treat a missing/corrupt decoded motion frame as an absent source frame.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_379e138c3e674f1b.rs#completion_379e138c3e674f1b_treat_a_missing_corrupt_decoded_motion_frame_as_.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-motion/src/integration.rs#missing_decoder_result_is_none` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-motion missing_decoder_result_is_none`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

  Reconciliation note (2026-08-01): `MotionClipSource::frame` already returned
  the injected decoder's `None` unchanged, so the baseline owner was GREEN.
  The owner was strengthened to use one valid PNG, one actually corrupted PNG,
  and one actually deleted frame and still remained GREEN; valid code was not
  reverted to manufacture RED.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-motion/src/integration.rs#MotionClipSource::frame`, `docs/modules/opentake-motion/integration.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-motion missing_decoder_result_is_none`

  Expected: PASS with every candidate-bound assertion executed.

  Verified 2026-08-01. The valid frame decodes at `6x4`; corrupt and missing
  frames both return `None`; decoder calls do not recreate the missing file,
  rewrite the corrupt bytes, or change the cache directory entry count.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01. Focused owner, warnings-denied motion Clippy,
  formatting, and the full workspace Rust suite pass. Evidence:
  code:`crates/opentake-motion/src/integration.rs#MotionClipSource::frame` and
  test:`crates/opentake-motion/src/integration.rs#missing_decoder_result_is_none`.

### Task 26: MR-motion-sandbox-complete (implementation-slice-70b5cbcce858ecde)

**Covered records:**
- `requirement-d6ca662045d019df` (requirement)
- `requirement-1f1781dbcd30d20c` (requirement)
- `requirement-69723d1c0bd77577` (requirement)

**Files:**
- Modify: `crates/opentake-motion/src/renderer.rs#StubRenderer::render`
- Modify: `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer::render`
- Modify: `crates/opentake-motion/src/sandbox.rs#SandboxPolicy::check_document_size`
- Modify: `docs/modules/opentake-motion/renderer.md`
- Modify: `docs/modules/opentake-motion/sandbox.md`
- Test (existing-owned): `crates/opentake-motion/src/sandbox.rs#document_size_ceiling_enforced`
- Test (existing-owned): `crates/opentake-motion/src/renderer.rs#chromium_applies_sandbox_size_before_unavailable`

**Candidate-bound contracts:**

#### requirement-d6ca662045d019df

- Candidate/source: `doc-d7ca9a44e2f69fde` at `docs/modules/opentake-motion/renderer.md:47` (requirement)
- Expected behavior: At docs/modules/opentake-motion/renderer.md:47 under “`StubRenderer`（已实现）” (gap-marker), the source “- 即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。” requires this exact behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.
- Resolution: `reviewed-mapping-report:MR-motion-sandbox-complete` — Both stub and unavailable Chromium paths enforce document size before rendering or returning unavailable.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-motion/renderer.md:47; signal=gap-marker; heading=`StubRenderer`（已实现）; candidate=- 即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。
  - Expected behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path. This closes only the promise expressed by “即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。” in “`StubRenderer`（已实现）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。” with the scenario below and register test:crates/opentake-project/tests/completion_d7ca9a44e2f69fde.rs#completion_d7ca9a44e2f69fde_apply_sandbox_document_size_checks_before_both_s
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_d7ca9a44e2f69fde.rs#completion_d7ca9a44e2f69fde_apply_sandbox_document_size_checks_before_both_s.

#### requirement-1f1781dbcd30d20c

- Candidate/source: `doc-d4dc621dbf47ec34` at `docs/modules/opentake-motion/renderer.md:91` (requirement)
- Expected behavior: At docs/modules/opentake-motion/renderer.md:91 under “移植铁律落地” (gap-marker), the source “- **沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。” requires this exact behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.
- Resolution: `reviewed-mapping-report:MR-motion-sandbox-complete` — Both stub and unavailable Chromium paths enforce document size before rendering or returning unavailable.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-motion/renderer.md:91; signal=gap-marker; heading=移植铁律落地; candidate=- **沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。
  - Expected behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path. This closes only the promise expressed by “**沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。” in “移植铁律落地”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “**沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。” with the scenario below and register test:crates/opentake-project/tests/completion_d4dc621dbf47ec34.rs#completion_d4dc621dbf47ec34_apply_sandbox_document_size_checks_before_both_s
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “**沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_d4dc621dbf47ec34.rs#completion_d4dc621dbf47ec34_apply_sandbox_document_size_checks_before_both_s.

#### requirement-69723d1c0bd77577

- Candidate/source: `doc-3a71829a7b489aae` at `docs/modules/opentake-motion/sandbox.md:67` (requirement)
- Expected behavior: At docs/modules/opentake-motion/sandbox.md:67 under “谁在调用” (gap-marker), the source “- `StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](renderer.md)）。” requires this exact behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.
- Resolution: `reviewed-mapping-report:MR-motion-sandbox-complete` — Both stub and unavailable Chromium paths enforce document size before rendering or returning unavailable.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-motion/sandbox.md:67; signal=gap-marker; heading=谁在调用; candidate=- `StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](renderer.md)）。
  - Expected behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path. This closes only the promise expressed by “`StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](renderer.md)）。” in “谁在调用”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “`StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](renderer.md)）。” with the scenario below and register test:crates/opentake-project/tests/completion_3a71829a7b489aae.rs#completion_3a71829a7b489aae_apply_sandbox_document_size_checks_before_both_s
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “`StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](renderer.md)）。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_3a71829a7b489aae.rs#completion_3a71829a7b489aae_apply_sandbox_document_size_checks_before_both_s.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-motion/src/sandbox.rs#document_size_ceiling_enforced` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/src/renderer.rs#chromium_applies_sandbox_size_before_unavailable` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-motion document_size_ceiling_enforced`
  - Run: `cargo test -p opentake-motion chromium_applies_sandbox_size_before_unavailable`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

  Reconciliation note (2026-08-01): both checks predated this generated plan,
  so the baseline owners were already GREEN. Owners were extended with exact
  byte/UTF-8 boundaries, both renderer paths, both Chromium feature modes, and
  zero cache-directory side effects; valid code was not reverted to manufacture
  RED.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-motion/src/renderer.rs#StubRenderer::render`, `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer::render`, `crates/opentake-motion/src/sandbox.rs#SandboxPolicy::check_document_size`, `docs/modules/opentake-motion/renderer.md`, `docs/modules/opentake-motion/sandbox.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-motion document_size_ceiling_enforced`
  - Run: `cargo test -p opentake-motion chromium_applies_sandbox_size_before_unavailable`

  Expected: PASS with every candidate-bound assertion executed.

  Verified 2026-08-01. Equality at the byte ceiling passes; one byte over and a
  multi-byte UTF-8 overflow fail. Stub, unfeatured Chromium, and feature-enabled
  Chromium all return `Sandbox` before creating a content-hash directory.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01. Both focused owners in default mode, the Chromium owner
  with the live feature, feature-enabled warnings-denied Clippy, formatting, and
  the full workspace Rust suite pass.

### Task 27: MR-motion-png-complete (implementation-slice-329d3a7fb3b066f7)

**Covered records:**
- `requirement-4ed41de96871fedf` (requirement)

**Files:**
- Modify: `crates/opentake-motion/src/renderer.rs#encode_solid_rgba_png`
- Modify: `docs/modules/opentake-motion/renderer.md`
- Test (existing-owned): `crates/opentake-motion/src/renderer.rs#stub_output_is_deterministic`
- Test (existing-owned): `crates/opentake-motion/src/renderer.rs#stub_png_decodes_with_correct_dimensions_and_alpha`

**Candidate-bound contracts:**

#### requirement-4ed41de96871fedf

- Candidate/source: `doc-2071153f8053805d` at `docs/modules/opentake-motion/renderer.md:51` (requirement)
- Expected behavior: At docs/modules/opentake-motion/renderer.md:51 under “自制 PNG 编码器（无依赖）” (gap-marker), the source “lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：” requires this exact behavior: Emit deterministic RGBA PNG frames from the stub renderer without a production image dependency.
- Resolution: `reviewed-mapping-report:MR-motion-png-complete` — The dependency-free PNG encoder is deterministic and tested for dimensions and alpha.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-motion/renderer.md:51; signal=gap-marker; heading=自制 PNG 编码器（无依赖）; candidate=lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：
  - Expected behavior: Emit deterministic RGBA PNG frames from the stub renderer without a production image dependency. This closes only the promise expressed by “lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：” in “自制 PNG 编码器（无依赖）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：” with the scenario below and register test:crates/opentake-project/tests/completion_2071153f8053805d.rs#completion_2071153f8053805d_emit_deterministic_rgba_png_frames_from_the_stub
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Emit deterministic RGBA PNG frames from the stub renderer without a production image dependency.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_2071153f8053805d.rs#completion_2071153f8053805d_emit_deterministic_rgba_png_frames_from_the_stub.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-motion/src/renderer.rs#stub_output_is_deterministic` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/src/renderer.rs#stub_png_decodes_with_correct_dimensions_and_alpha` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-motion stub_output_is_deterministic`
  - Run: `cargo test -p opentake-motion stub_png_decodes_with_correct_dimensions_and_alpha`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

  Reconciliation note (2026-08-01): the dependency-free encoder and both
  owners predated this generated plan, so baseline was already GREEN. Owners
  were strengthened with PNG magic, exact RGBA, direct byte identity, and a
  multi-block stored-deflate boundary; valid code was not reverted to
  manufacture RED.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-motion/src/renderer.rs#encode_solid_rgba_png`, `docs/modules/opentake-motion/renderer.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-motion stub_output_is_deterministic`
  - Run: `cargo test -p opentake-motion stub_png_decodes_with_correct_dimensions_and_alpha`

  Expected: PASS with every candidate-bound assertion executed.

  Verified 2026-08-01. Separate cache roots emit byte-identical PNGs; the real
  decoder confirms dimensions/alpha and exact corner RGBA for a `200x100`
  image whose scanlines cross the 65,535-byte deflate block boundary. `image`
  remains dev-only in `opentake-motion/Cargo.toml`.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01. Both focused owners, warnings-denied motion Clippy,
  formatting, and the full workspace Rust suite pass. `image` is listed only
  under `opentake-motion` dev-dependencies.

### Task 28: MR-project-serde-complete (implementation-slice-9c460dd3f289f9bd)

**Covered records:**
- `requirement-3c0728a9184da517` (requirement)

**Files:**
- Modify: `crates/opentake-project/src/bundle.rs#Project::open_from_root`
- Modify: `crates/opentake-domain/src/media.rs#MediaManifest::deserialize`
- Modify: `docs/modules/opentake-project/OVERVIEW.md`
- Test (existing-owned): `crates/opentake-project/tests/upstream_compat.rs#applies_clip_defaults_for_omitted_fields`
- Test (existing-owned): `crates/opentake-project/tests/upstream_compat.rs#migrates_generation_log_legacy_cost_and_version`

**Candidate-bound contracts:**

#### requirement-3c0728a9184da517

- Candidate/source: `doc-f2992815147bed14` at `docs/modules/opentake-project/OVERVIEW.md:110` (requirement)
- Expected behavior: At docs/modules/opentake-project/OVERVIEW.md:110 under “移植铁律（本模块重点）” (gap-marker), the source “1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。” requires this exact behavior: Decode persisted project/domain models compatibly, defaulting absent optional fields and preserving explicit version migrations.
- Resolution: `reviewed-mapping-report:MR-project-serde-complete` — The narrower optional-field and explicit-version migration contract has direct project and domain tests, though broader exhaustive migration remains in data-safety.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-project/OVERVIEW.md:110; signal=gap-marker; heading=移植铁律（本模块重点）; candidate=1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。
  - Expected behavior: Decode persisted project/domain models compatibly, defaulting absent optional fields and preserving explicit version migrations. This closes only the promise expressed by “1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。” in “移植铁律（本模块重点）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。” with the scenario below and register test:crates/opentake-agent/tests/completion_f2992815147bed14.rs#completion_f2992815147bed14_decode_persisted_project_domain_models_compatibl
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Decode persisted project/domain models compatibly, defaulting absent optional fields and preserving explicit version migrations.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_f2992815147bed14.rs#completion_f2992815147bed14_decode_persisted_project_domain_models_compatibl.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-project/tests/upstream_compat.rs#applies_clip_defaults_for_omitted_fields` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/upstream_compat.rs#migrates_generation_log_legacy_cost_and_version` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and reconcile the baseline state**

  - Run: `cargo test -p opentake-project --test upstream_compat applies_clip_defaults_for_omitted_fields -- --exact`
  - Run: `cargo test -p opentake-project --test upstream_compat migrates_generation_log_legacy_cost_and_version -- --exact`

  Reconciled 2026-08-01: both strengthened owners remained GREEN before the
  documentation-only implementation pass. The existing deserializers already
  satisfied the candidate contract, so no artificial production regression
  was introduced solely to manufacture RED.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-project/src/bundle.rs#Project::open_from_root`, `crates/opentake-domain/src/media.rs#MediaManifest::deserialize`, `docs/modules/opentake-project/OVERVIEW.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-project --test upstream_compat applies_clip_defaults_for_omitted_fields -- --exact`
  - Run: `cargo test -p opentake-project --test upstream_compat migrates_generation_log_legacy_cost_and_version -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01. Both focused owners pass with save/reopen equality,
  explicit and absent manifest-version assertions, legacy generation-cost
  migration, and stable synthesized IDs. Formatting, warnings-denied Clippy
  for `opentake-project` and `opentake-domain`, and the full workspace Rust
  suite pass.

### Task 29: MR-mask-effect-mixed-duplicate (implementation-slice-35e4fa716888cc28)

**Covered records:**
- `requirement-157653b425a21810` (requirement)

**Files:**
- Modify: `crates/opentake-render/src/gpu/compositor.rs`
- Modify: `docs/modules/opentake-render/OVERVIEW.md`
- Test (reviewed-planned): `crates/opentake-render/tests/composite_acceptance.rs#mask_and_effect_records_have_separate_child_owners`

**Candidate-bound contracts:**

#### requirement-157653b425a21810

- Candidate/source: `doc-7ffe9312478aa940` at `docs/modules/opentake-render/OVERVIEW.md:102` (requirement)
- Expected behavior: Render polygon masks and the generic Effect chain instead of carrying them as no-op/pass-through metadata.
- Resolution: `reviewed-mapping-report:MR-mask-effect-mixed-duplicate` — This record mixes polygon masks and generic effects, which are already represented by separate mask and effect slices.
- Exact acceptance contract:
  - Encode polygon points in a bounded GPU storage representation with deterministic overflow behavior and match domain SDF/feather/invert semantics.
  - Implement an explicit registry/pass pipeline for every shipped Effect name; reject unsupported names at the command boundary rather than silently passing metadata.
  - GPU/pixel-diff tests cover polygon inside/outside/edge/feather/invert, multiple masks, effect order, disabled effects, preview/export parity, and headless skip semantics.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/composite_acceptance.rs#mask_and_effect_records_have_separate_child_owners` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test composite_acceptance mask_and_effect_records_have_separate_child_owners -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-render/src/gpu/compositor.rs`, `docs/modules/opentake-render/OVERVIEW.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test composite_acceptance mask_and_effect_records_have_separate_child_owners -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01. The parent owner first failed on the stale module claim
  that polygon masks and generic effects were no-op metadata, then passed after
  the documentation and defensive compositor contract were corrected. Both
  real-GPU child owners pass, including all three mask shapes, hard/feathered
  and inverted coverage, multiple-mask intersection, every registered effect,
  disabled effects, ordered chains, and byte-identical preview/export output.
  The parent also proves mask/point overflow and unknown effects fail before
  mutation. Formatting, warnings-denied render Clippy, and the full workspace
  Rust suite pass.

### Task 30: MR-text-parity (implementation-slice-f92ff19f85ab4082)

**Covered records:**
- `requirement-c74866cce1e8dd58` (requirement)

**Files:**
- Modify: `crates/opentake-render/src/gpu/text_engine.rs#CosmicTextRasterizer`
- Modify: `crates/opentake-render/src/plan/types.rs#TextureSource::Text`
- Modify: `docs/modules/opentake-render/text-rasterizer.md`
- Test (existing-owned): `crates/opentake-render/tests/gpu_text.rs#rasterize_is_deterministic_ssim_one`
- Test (existing-owned): `crates/opentake-render/tests/gpu_text.rs#natural_size_shadow_padding_matches_upstream`
- Test (reviewed-planned): `crates/opentake-render/tests/gpu_text.rs#fallback_font_no_font_scaled_stroke_and_structural_golden_matrix`

**Candidate-bound contracts:**

#### requirement-c74866cce1e8dd58

- Candidate/source: `doc-734ee146d5000580` at `docs/modules/opentake-render/text-rasterizer.md:44` (requirement)
- Expected behavior: Close text raster parity with deterministic structural image comparisons and the remaining fallback/layout details.
- Resolution: `reviewed-mapping-report:MR-text-parity` — Substantial text rendering and structural tests exist, while the module document still lists fallback and layout parity work.
- Exact acceptance contract:
  - Add a pinned upstream comparison fixture for wrapping, fallback fonts, shadow padding, stroke width, size, and alignment across Chinese/Latin text.
  - Pass deterministic structural/pixel thresholds on macOS and the headless fallback while preserving non-crashing no-font behavior.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/gpu_text.rs#rasterize_is_deterministic_ssim_one` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/tests/gpu_text.rs#natural_size_shadow_padding_matches_upstream` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/tests/gpu_text.rs#fallback_font_no_font_scaled_stroke_and_structural_golden_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test gpu_text rasterize_is_deterministic_ssim_one -- --exact`
  - Run: `cargo test -p opentake-render --test gpu_text natural_size_shadow_padding_matches_upstream -- --exact`
  - Run: `cargo test -p opentake-render --test gpu_text fallback_font_no_font_scaled_stroke_and_structural_golden_matrix -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-render/src/gpu/text_engine.rs#CosmicTextRasterizer`, `crates/opentake-render/src/plan/types.rs#TextureSource::Text`, `docs/modules/opentake-render/text-rasterizer.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test gpu_text rasterize_is_deterministic_ssim_one -- --exact`
  - Run: `cargo test -p opentake-render --test gpu_text natural_size_shadow_padding_matches_upstream -- --exact`
  - Run: `cargo test -p opentake-render --test gpu_text fallback_font_no_font_scaled_stroke_and_structural_golden_matrix -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01. The new owner first failed to compile because the
  explicit no-font constructor did not exist; after adding it, the real empty
  font database exposed cosmic-text's `no default font found` panic. The
  production rasterizer now branches before shaping and returns the correctly
  sized transparent premultiplied frame while preserving background/border
  rendering. The pinned structural matrix passes for missing-family fallback,
  Chinese/Latin text, wrapping, alignment, 12px-per-side shadow padding, and
  1/2/4px border scaling at 540p/1080p/2160p. All eight text integration tests,
  warnings-denied render Clippy, formatting, and the full workspace Rust suite
  pass on macOS with real system fonts.

### Task 31: MR-media-principles-headings (implementation-slice-726e8186554da9b6)

**Covered records:**
- `requirement-1a638c73abd21e1e` (requirement)
- `requirement-9c7360588ced0198` (requirement)

**Files:**
- Modify: `docs/specs/media/0-principles.md`
- Modify: `docs/specs/media/9-domain-contract.md`
- Test (reviewed-planned): `crates/opentake-render/tests/composite_acceptance.rs#media_principles_headings_reference_exact_child_capabilities`

**Candidate-bound contracts:**

#### requirement-1a638c73abd21e1e

- Candidate/source: `doc-3a72ee4de1f4a46f` at `docs/specs/media/0-principles.md:1` (requirement)
- Expected behavior: At docs/specs/media/0-principles.md:1 under “设计原则与移植铁律(本 crate 必须遵守)” (heading), the source “# 设计原则与移植铁律(本 crate 必须遵守)” requires this exact behavior: Media code follows cross-platform, frame-time, cache, and domain-boundary contracts.
- Resolution: `reviewed-mapping-report:MR-media-principles-headings` — These are architecture principle and compliance headings; they should become verifier or acceptance collections rather than feature records.
- Exact acceptance contract:
  - Source binding: docs/specs/media/0-principles.md:1; signal=heading; heading=设计原则与移植铁律(本 crate 必须遵守); candidate=# 设计原则与移植铁律(本 crate 必须遵守)
  - Expected behavior: Media code follows cross-platform, frame-time, cache, and domain-boundary contracts. This closes only the promise expressed by “设计原则与移植铁律(本 crate 必须遵守)” in “设计原则与移植铁律(本 crate 必须遵守)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “设计原则与移植铁律(本 crate 必须遵守)” with the scenario below and register test:crates/opentake-render/tests/completion_3a72ee4de1f4a46f.rs#completion_3a72ee4de1f4a46f_media_code_follows_cross_platform_frame_time_cac
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “设计原则与移植铁律(本 crate 必须遵守)”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Media code follows cross-platform, frame-time, cache, and domain-boundary contracts.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_3a72ee4de1f4a46f.rs#completion_3a72ee4de1f4a46f_media_code_follows_cross_platform_frame_time_cac.

#### requirement-9c7360588ced0198

- Candidate/source: `doc-3336fb8bef6f3a49` at `docs/specs/media/9-domain-contract.md:1` (requirement)
- Expected behavior: At docs/specs/media/9-domain-contract.md:1 under “跨平台与合规要点” (heading), the source “# 跨平台与合规要点” requires this exact behavior: Media code follows cross-platform, frame-time, cache, and domain-boundary contracts.
- Resolution: `reviewed-mapping-report:MR-media-principles-headings` — These are architecture principle and compliance headings; they should become verifier or acceptance collections rather than feature records.
- Exact acceptance contract:
  - Source binding: docs/specs/media/9-domain-contract.md:1; signal=heading; heading=跨平台与合规要点; candidate=# 跨平台与合规要点
  - Expected behavior: Media code follows cross-platform, frame-time, cache, and domain-boundary contracts. This closes only the promise expressed by “跨平台与合规要点” in “跨平台与合规要点”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “跨平台与合规要点” with the scenario below and register test:crates/opentake-render/tests/completion_3336fb8bef6f3a49.rs#completion_3336fb8bef6f3a49_media_code_follows_cross_platform_frame_time_cac
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “跨平台与合规要点”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Media code follows cross-platform, frame-time, cache, and domain-boundary contracts.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_3336fb8bef6f3a49.rs#completion_3336fb8bef6f3a49_media_code_follows_cross_platform_frame_time_cac.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-render/tests/composite_acceptance.rs#media_principles_headings_reference_exact_child_capabilities` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-render --test composite_acceptance media_principles_headings_reference_exact_child_capabilities -- --exact`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/media/0-principles.md`, `docs/specs/media/9-domain-contract.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-render --test composite_acceptance media_principles_headings_reference_exact_child_capabilities -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01. The aggregate owner first failed because neither
  principle heading bound itself to executable children. Both documents now
  reference the exact real-media probe, RGBA decode, 16 kHz PCM, waveform,
  encode/reprobe, and export-pause owners; every child passes against local
  generated fixtures. The compliance document now describes the actual FFmpeg
  subprocess-sidecar architecture and explicitly retains the bundled
  `--enable-nonfree` binary as a Beta release blocker. The focused aggregate,
  formatting, and completion-audit Node gate pass.

### Task 32: MR-bounded-index-runtime (implementation-slice-603290a188109040)

**Covered records:**
- `requirement-821266d8c284a554` (requirement)
- `requirement-c3118cc0b3ca139f` (requirement)
- `requirement-37700810e0c01c49` (requirement)
- `requirement-2305bfdee6a62e76` (requirement)

**Files:**
- Modify: `src-tauri/src/search.rs#search_index_start`
- Modify: `src-tauri/src/search.rs#index_assets`
- Modify: `crates/opentake-media/src/index_coordinator.rs#ExportPause`
- Modify: `crates/opentake-media/src/ort_worker/mod.rs#OrtModel`
- Modify: `docs/specs/media/10-acceptance.md`
- Modify: `docs/specs/media/7-ort-worker.md`
- Test (existing-owned): `crates/opentake-media/src/index_coordinator.rs#export_pause_ref_counts`
- Test (reviewed-planned): `src-tauri/src/search.rs#bounded_single_worker_cancels_skips_stale_and_yields_to_playback_export`

**Candidate-bound contracts:**

#### requirement-821266d8c284a554

- Candidate/source: `doc-31663a59f89938e0` at `docs/specs/media/10-acceptance.md:15` (requirement)
- Expected behavior: Phase 8 search/transcription worker scheduling runs through a bounded production coordinator.
- Resolution: `reviewed-mapping-report:MR-bounded-index-runtime` — The media crate explicitly defers the runtime queue; current Tauri search code does not prove a bounded serialized production coordinator.
- Exact acceptance contract:
  - Connect IndexCoordinator to the runtime queue rather than deferred orchestration.
  - Serialize heavy inference and yield during export/playback pressure.
  - Add concurrent indexing/transcription/export tests.

#### requirement-c3118cc0b3ca139f

- Candidate/source: `doc-53d7eb7f42379bad` at `docs/specs/media/7-ort-worker.md:1` (requirement)
- Expected behavior: Heavy inference and indexing are serialized, cancellable, and yield to playback/export in the production runtime.
- Resolution: `reviewed-mapping-report:MR-bounded-index-runtime` — The media crate explicitly defers the runtime queue; current Tauri search code does not prove a bounded serialized production coordinator.
- Exact acceptance contract:
  - Run all heavy ORT tasks through one bounded production queue with model identity, priority, cancellation, and typed result/error.
  - Serialize GPU-heavy inference and pause/yield queued indexing/transcription while playback/export holds higher priority, then resume without duplicate work.
  - Stress-test concurrent search/index/transcribe requests, export preemption, cancellation, model failure, shutdown, and restart with bounded worker count and no lost terminal result.

#### requirement-37700810e0c01c49

- Candidate/source: `doc-1643c9517d5fcf4f` at `docs/specs/media/7-ort-worker.md:23` (requirement)
- Expected behavior: Heavy inference and indexing are serialized, cancellable, and yield to playback/export in the production runtime.
- Resolution: `reviewed-mapping-report:MR-bounded-index-runtime` — The media crate explicitly defers the runtime queue; current Tauri search code does not prove a bounded serialized production coordinator.
- Exact acceptance contract:
  - Implement a bounded worker queue that serializes GPU-heavy model sessions and exposes queued/running/cancelled/completed states.
  - Higher-priority playback/export pressure must prevent new heavy jobs and cause cooperative yield at defined batch boundaries without corrupting model/session state.
  - Test FIFO within priority, starvation bound, cancel queued/running, panic/model error recovery, export preemption latency, and clean shutdown with zero active jobs.

#### requirement-2305bfdee6a62e76

- Candidate/source: `doc-f6ed8457d263b44c` at `docs/specs/media/7-ort-worker.md:41` (requirement)
- Expected behavior: Heavy inference and indexing are serialized, cancellable, and yield to playback/export in the production runtime.
- Resolution: `reviewed-mapping-report:MR-bounded-index-runtime` — The media crate explicitly defers the runtime queue; current Tauri search code does not prove a bounded serialized production coordinator.
- Exact acceptance contract:
  - Connect IndexCoordinator to the production worker queue for idempotent media indexing/transcription keyed by source fingerprint and model version.
  - Deduplicate duplicate requests, persist completed state atomically, invalidate changed media/model, and resume interrupted work after restart.
  - Test duplicate enqueue, source change, model upgrade, export pause/resume, cancellation, crash/restart, failure retry, and final index/transcript equality.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/src/index_coordinator.rs#export_pause_ref_counts` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/search.rs#bounded_single_worker_cancels_skips_stale_and_yields_to_playback_export` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media export_pause_ref_counts`
  - Run: `cargo test -p opentake-tauri bounded_single_worker_cancels_skips_stale_and_yields_to_playback_export`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/search.rs#search_index_start`, `src-tauri/src/search.rs#index_assets`, `crates/opentake-media/src/index_coordinator.rs#ExportPause`, `crates/opentake-media/src/ort_worker/mod.rs#OrtModel`, `docs/specs/media/10-acceptance.md`, `docs/specs/media/7-ort-worker.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media export_pause_ref_counts`
  - Run: `cargo test -p opentake-tauri bounded_single_worker_cancels_skips_stale_and_yields_to_playback_export`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01. The owning tests first failed because the shared
  pressure primitive had no balanced guard/wait protocol and no bounded worker
  existed. Production `search_index_start` now submits one source/model-keyed
  job to a process-wide capacity-8 `OrtWorker`; that job uses a lazy typed model
  registry and serially performs missing visual and transcript work, checking
  cancellation and playback/export pressure at every asset boundary. The
  executor proves single-worker execution, live-key result dedupe, priority
  FIFO, a four-interactive-job starvation bound, queue-full rejection,
  queued/running cancellation, model-error and panic recovery, immediate failure
  retry, balanced nested pressure wakeup, source/model invalidation, restart,
  and clean shutdown with zero active jobs. Both focused owners, rustfmt,
  all-feature clippy with `-D warnings`, and the full workspace suite pass.

### Task 33: MR-packaged-ffmpeg (implementation-slice-ddfcf34d5292a998)

**Covered records:**
- `requirement-ff2faf0938e25f39` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/ff.rs#ffmpeg_path`
- Modify: `crates/opentake-media/src/ff.rs#ffprobe_path`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `docs/specs/media/2-ffmpeg.md`
- Test (reviewed-planned): `scripts/tests/packaged-sidecars-test.rb#packaged_macos_windows_sidecars_resolve_and_execute`

**Candidate-bound contracts:**

#### requirement-ff2faf0938e25f39

- Candidate/source: `doc-f06463b86285d17a` at `docs/specs/media/2-ffmpeg.md:1` (requirement)
- Expected behavior: FFmpeg is resolved as a verified bundled sidecar in packaged macOS and Windows builds.
- Resolution: `reviewed-mapping-report:MR-packaged-ffmpeg` — Runtime path helpers exist, but no verified packaged external-binary configuration or macOS and Windows execution receipt was found.
- Exact acceptance contract:
  - Package target-specific FFmpeg/ffprobe binaries and verify checksum/version.
  - Resolve packaged sidecar paths without relying on developer PATH.
  - Run installed-app probe/decode/encode smoke tests on macOS and Windows.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `scripts/tests/packaged-sidecars-test.rb#packaged_macos_windows_sidecars_resolve_and_execute` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `ruby scripts/tests/packaged-sidecars-test.rb --name "packaged_macos_windows_sidecars_resolve_and_execute"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/ff.rs#ffmpeg_path`, `crates/opentake-media/src/ff.rs#ffprobe_path`, `src-tauri/tauri.conf.json`, `docs/specs/media/2-ffmpeg.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `ruby scripts/tests/packaged-sidecars-test.rb --name "packaged_macos_windows_sidecars_resolve_and_execute"`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Reconciled 2026-08-01 from the tracked RED/GREEN receipt and reverified on
  the current descendant. The original owner failed before the immutable
  sidecar lock existed, then passed after checksum/version-pinned macOS arm64,
  macOS x64, and Windows x64 supplies plus both platform `externalBin` configs
  were added. Current macOS arm64 source binaries pass verify-only and the
  empty-`PATH` probe/decode/encode smoke; the exact release `.app` sibling pair
  passes the same installed-package owner and nested codesign verification.
  GitHub Actions run `30614001607`, exact SHA
  `9eeeb6ffe3088a16f19946ceb7db5e90090356ac`, remains a successful ancestor
  receipt: its Windows full-product job built MSI/NSIS, silently installed NSIS,
  and passed the installed-directory empty-`PATH` smoke. Current packaged-path
  tests, Windows CI/config contracts, rustfmt, and the full workspace suite pass.
  This closes sidecar resolution/execution only; the separately tracked
  `--enable-nonfree` licensing replacement and Developer-ID/notarization gates
  remain Beta-release blockers.

### Task 34: MR-ffmpeg-contract-complete (implementation-slice-2edbd096c204bad4)

**Covered records:**
- `requirement-bfc002de0fad03b1` (requirement)
- `requirement-d35161912b562397` (requirement)
- `requirement-48b0e0781eeb48c1` (requirement)
- `requirement-6b7f592b1be2c09b` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/probe.rs#probe`
- Modify: `crates/opentake-media/src/decode/frame.rs#decode_frame_at`
- Modify: `crates/opentake-media/src/decode/pcm.rs#extract_pcm`
- Modify: `crates/opentake-media/src/encode/mod.rs#VideoEncoder`
- Modify: `docs/specs/media/2-ffmpeg.md`
- Test (existing-owned): `crates/opentake-media/tests/ffmpeg_integration.rs#probe_reports_dimensions_fps_and_audio`
- Test (existing-owned): `crates/opentake-media/tests/ffmpeg_integration.rs#decode_frame_returns_rgba_of_expected_size`
- Test (existing-owned): `crates/opentake-media/tests/ffmpeg_integration.rs#extract_pcm_yields_16k_mono`
- Test (existing-owned): `crates/opentake-media/tests/ffmpeg_integration.rs#encode_roundtrip_produces_playable_video`

**Candidate-bound contracts:**

#### requirement-bfc002de0fad03b1

- Candidate/source: `doc-c874eae5e1eebd3a` at `docs/specs/media/2-ffmpeg.md:3` (requirement)
- Expected behavior: At docs/specs/media/2-ffmpeg.md:3 under “2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)” (heading), the source “## 2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)” requires this exact behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.
- Resolution: `reviewed-mapping-report:MR-ffmpeg-contract-complete` — Probe, frame decode, PCM extraction and encoding have tracked end-to-end FFmpeg integration fixtures.
- Exact acceptance contract:
  - Source binding: docs/specs/media/2-ffmpeg.md:3; signal=heading; heading=2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支); candidate=## 2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)
  - Expected behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures. This closes only the promise expressed by “2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)” in “2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)” with the scenario below and register test:crates/opentake-render/tests/completion_c874eae5e1eebd3a.rs#completion_c874eae5e1eebd3a_the_named_ffmpeg_probe_decode_pcm_encode_contrac
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_c874eae5e1eebd3a.rs#completion_c874eae5e1eebd3a_the_named_ffmpeg_probe_decode_pcm_encode_contrac.

#### requirement-d35161912b562397

- Candidate/source: `doc-e36487856d25a4d1` at `docs/specs/media/2-ffmpeg.md:30` (requirement)
- Expected behavior: At docs/specs/media/2-ffmpeg.md:30 under “2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)” (heading), the source “## 2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)” requires this exact behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.
- Resolution: `reviewed-mapping-report:MR-ffmpeg-contract-complete` — Probe, frame decode, PCM extraction and encoding have tracked end-to-end FFmpeg integration fixtures.
- Exact acceptance contract:
  - Source binding: docs/specs/media/2-ffmpeg.md:30; signal=heading; heading=2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座); candidate=## 2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)
  - Expected behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures. This closes only the promise expressed by “2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)” in “2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)” with the scenario below and register test:crates/opentake-render/tests/completion_e36487856d25a4d1.rs#completion_e36487856d25a4d1_the_named_ffmpeg_probe_decode_pcm_encode_contrac
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_e36487856d25a4d1.rs#completion_e36487856d25a4d1_the_named_ffmpeg_probe_decode_pcm_encode_contrac.

#### requirement-48b0e0781eeb48c1

- Candidate/source: `doc-c2c95cd120af9730` at `docs/specs/media/2-ffmpeg.md:60` (requirement)
- Expected behavior: At docs/specs/media/2-ffmpeg.md:60 under “2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)” (heading), the source “## 2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)” requires this exact behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.
- Resolution: `reviewed-mapping-report:MR-ffmpeg-contract-complete` — Probe, frame decode, PCM extraction and encoding have tracked end-to-end FFmpeg integration fixtures.
- Exact acceptance contract:
  - Source binding: docs/specs/media/2-ffmpeg.md:60; signal=heading; heading=2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`); candidate=## 2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)
  - Expected behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures. This closes only the promise expressed by “2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)” in “2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)” with the scenario below and register test:crates/opentake-render/tests/completion_c2c95cd120af9730.rs#completion_c2c95cd120af9730_the_named_ffmpeg_probe_decode_pcm_encode_contrac
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_c2c95cd120af9730.rs#completion_c2c95cd120af9730_the_named_ffmpeg_probe_decode_pcm_encode_contrac.

#### requirement-6b7f592b1be2c09b

- Candidate/source: `doc-376576e91988107c` at `docs/specs/media/2-ffmpeg.md:80` (requirement)
- Expected behavior: At docs/specs/media/2-ffmpeg.md:80 under “2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)” (heading), the source “## 2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)” requires this exact behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.
- Resolution: `reviewed-mapping-report:MR-ffmpeg-contract-complete` — Probe, frame decode, PCM extraction and encoding have tracked end-to-end FFmpeg integration fixtures.
- Exact acceptance contract:
  - Source binding: docs/specs/media/2-ffmpeg.md:80; signal=heading; heading=2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用); candidate=## 2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)
  - Expected behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures. This closes only the promise expressed by “2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)” in “2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)” with the scenario below and register test:crates/opentake-project/tests/completion_376576e91988107c.rs#completion_376576e91988107c_the_named_ffmpeg_probe_decode_pcm_encode_contrac
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_376576e91988107c.rs#completion_376576e91988107c_the_named_ffmpeg_probe_decode_pcm_encode_contrac.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/tests/ffmpeg_integration.rs#probe_reports_dimensions_fps_and_audio` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#decode_frame_returns_rgba_of_expected_size` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#extract_pcm_yields_16k_mono` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#encode_roundtrip_produces_playable_video` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and reconcile the historical baseline**

  - Run: `cargo test -p opentake-media --test ffmpeg_integration probe_reports_dimensions_fps_and_audio -- --exact`
  - Run: `cargo test -p opentake-media --test ffmpeg_integration decode_frame_returns_rgba_of_expected_size -- --exact`
  - Run: `cargo test -p opentake-media --test ffmpeg_integration extract_pcm_yields_16k_mono -- --exact`
  - Run: `cargo test -p opentake-media --test ffmpeg_integration encode_roundtrip_produces_playable_video -- --exact`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/probe.rs#probe`, `crates/opentake-media/src/decode/frame.rs#decode_frame_at`, `crates/opentake-media/src/decode/pcm.rs#extract_pcm`, `crates/opentake-media/src/encode/mod.rs#VideoEncoder`, `docs/specs/media/2-ffmpeg.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media --test ffmpeg_integration probe_reports_dimensions_fps_and_audio -- --exact`
  - Run: `cargo test -p opentake-media --test ffmpeg_integration decode_frame_returns_rgba_of_expected_size -- --exact`
  - Run: `cargo test -p opentake-media --test ffmpeg_integration extract_pcm_yields_16k_mono -- --exact`
  - Run: `cargo test -p opentake-media --test ffmpeg_integration encode_roundtrip_produces_playable_video -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Reconciled 2026-08-01 as an already-integrated baseline. Git history shows
  all four named owning fixtures and their production paths landed together in
  `d9b2812`; its parent does not contain the tests, so there is no honest
  test-present/implementation-missing RED command to replay. On the current
  descendant, each exact owner independently passes against generated local
  media: probe returns dimensions/FPS/audio, frame decode returns the expected
  RGBA shape, PCM extraction returns 16 kHz mono, and encoded output re-probes
  as playable video. The checksum-pinned packaged sidecars also pass the
  empty-`PATH` installed-app smoke, and rustfmt plus the full workspace suite
  pass. No production change was required for this reconciliation.

### Task 35: MR-media-facade (implementation-slice-3adf63f547b1f57b)

**Covered records:**
- `requirement-0fc6018b00cc4a4a` (requirement)
- `requirement-d24e542069e91a59` (requirement)
- `requirement-fccb7646732532d1` (requirement)
- `requirement-47f02756ed829ddc` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/lib.rs#MediaEngine`
- Modify: `crates/opentake-media/src/probe.rs`
- Modify: `crates/opentake-media/src/decode/mod.rs`
- Modify: `crates/opentake-media/src/encode/mod.rs`
- Modify: `crates/opentake-media/src/search/mod.rs`
- Modify: `crates/opentake-media/src/transcribe/mod.rs`
- Modify: `docs/specs/media/8-coordinator.md`
- Test (existing-owned): `crates/opentake-media/src/lib.rs#crate_public_types_are_reachable`
- Test (reviewed-planned): `crates/opentake-media/tests/facade_contract.rs#all_services_are_reachable_only_through_facade_and_dependencies_stay_acyclic`

**Candidate-bound contracts:**

#### requirement-0fc6018b00cc4a4a

- Candidate/source: `doc-aea782f4b3e1e8c4` at `docs/specs/media/8-coordinator.md:1` (requirement)
- Expected behavior: At docs/specs/media/8-coordinator.md:1 under “与 domain / render 的接口” (heading), the source “# 与 domain / render 的接口” requires this exact behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.
- Resolution: `reviewed-mapping-report:MR-media-facade` — MediaEngine exposes several services, while decode and encode remain mostly flat exports and dependency direction lacks a closed contract test.
- Exact acceptance contract:
  - Source binding: docs/specs/media/8-coordinator.md:1; signal=heading; heading=与 domain / render 的接口; candidate=# 与 domain / render 的接口
  - Expected behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies. This closes only the promise expressed by “与 domain / render 的接口” in “与 domain / render 的接口”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “与 domain / render 的接口” with the scenario below and register test:crates/opentake-render/tests/completion_aea782f4b3e1e8c4.rs#completion_aea782f4b3e1e8c4_the_media_facade_exposes_probe_decode_encode_sea
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “与 domain / render 的接口”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_aea782f4b3e1e8c4.rs#completion_aea782f4b3e1e8c4_the_media_facade_exposes_probe_decode_encode_sea.

#### requirement-d24e542069e91a59

- Candidate/source: `doc-9056237f261e5966` at `docs/specs/media/8-coordinator.md:3` (requirement)
- Expected behavior: At docs/specs/media/8-coordinator.md:3 under “8.1 消费 `opentake-domain`(不可改)” (heading), the source “## 8.1 消费 `opentake-domain`(不可改)” requires this exact behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.
- Resolution: `reviewed-mapping-report:MR-media-facade` — MediaEngine exposes several services, while decode and encode remain mostly flat exports and dependency direction lacks a closed contract test.
- Exact acceptance contract:
  - Source binding: docs/specs/media/8-coordinator.md:3; signal=heading; heading=8.1 消费 `opentake-domain`(不可改); candidate=## 8.1 消费 `opentake-domain`(不可改)
  - Expected behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies. This closes only the promise expressed by “8.1 消费 `opentake-domain`(不可改)” in “8.1 消费 `opentake-domain`(不可改)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.1 消费 `opentake-domain`(不可改)” with the scenario below and register test:crates/opentake-project/tests/completion_9056237f261e5966.rs#completion_9056237f261e5966_the_media_facade_exposes_probe_decode_encode_sea
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “8.1 消费 `opentake-domain`(不可改)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_9056237f261e5966.rs#completion_9056237f261e5966_the_media_facade_exposes_probe_decode_encode_sea.

#### requirement-fccb7646732532d1

- Candidate/source: `doc-78e63fc5876e6f1a` at `docs/specs/media/8-coordinator.md:13` (requirement)
- Expected behavior: At docs/specs/media/8-coordinator.md:13 under “8.2 被 `opentake-render` 复用的解码/编码” (heading), the source “## 8.2 被 `opentake-render` 复用的解码/编码” requires this exact behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.
- Resolution: `reviewed-mapping-report:MR-media-facade` — MediaEngine exposes several services, while decode and encode remain mostly flat exports and dependency direction lacks a closed contract test.
- Exact acceptance contract:
  - Source binding: docs/specs/media/8-coordinator.md:13; signal=heading; heading=8.2 被 `opentake-render` 复用的解码/编码; candidate=## 8.2 被 `opentake-render` 复用的解码/编码
  - Expected behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies. This closes only the promise expressed by “8.2 被 `opentake-render` 复用的解码/编码” in “8.2 被 `opentake-render` 复用的解码/编码”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.2 被 `opentake-render` 复用的解码/编码” with the scenario below and register test:crates/opentake-project/tests/completion_78e63fc5876e6f1a.rs#completion_78e63fc5876e6f1a_the_media_facade_exposes_probe_decode_encode_sea
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “8.2 被 `opentake-render` 复用的解码/编码”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_78e63fc5876e6f1a.rs#completion_78e63fc5876e6f1a_the_media_facade_exposes_probe_decode_encode_sea.

#### requirement-47f02756ed829ddc

- Candidate/source: `doc-94ba8c5254bc852d` at `docs/specs/media/8-coordinator.md:33` (requirement)
- Expected behavior: At docs/specs/media/8-coordinator.md:33 under “8.4 facade `MediaEngine`(供 `opentake-core` 调用)” (heading), the source “## 8.4 facade `MediaEngine`(供 `opentake-core` 调用)” requires this exact behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.
- Resolution: `reviewed-mapping-report:MR-media-facade` — MediaEngine exposes several services, while decode and encode remain mostly flat exports and dependency direction lacks a closed contract test.
- Exact acceptance contract:
  - Source binding: docs/specs/media/8-coordinator.md:33; signal=heading; heading=8.4 facade `MediaEngine`(供 `opentake-core` 调用); candidate=## 8.4 facade `MediaEngine`(供 `opentake-core` 调用)
  - Expected behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies. This closes only the promise expressed by “8.4 facade `MediaEngine`(供 `opentake-core` 调用)” in “8.4 facade `MediaEngine`(供 `opentake-core` 调用)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.4 facade `MediaEngine`(供 `opentake-core` 调用)” with the scenario below and register test:crates/opentake-project/tests/completion_94ba8c5254bc852d.rs#completion_94ba8c5254bc852d_the_media_facade_exposes_probe_decode_encode_sea
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “8.4 facade `MediaEngine`(供 `opentake-core` 调用)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_94ba8c5254bc852d.rs#completion_94ba8c5254bc852d_the_media_facade_exposes_probe_decode_encode_sea.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/src/lib.rs#crate_public_types_are_reachable` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/facade_contract.rs#all_services_are_reachable_only_through_facade_and_dependencies_stay_acyclic` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media crate_public_types_are_reachable`
  - Run: `cargo test -p opentake-media --test facade_contract all_services_are_reachable_only_through_facade_and_dependencies_stay_acyclic -- --exact`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/lib.rs#MediaEngine`, `crates/opentake-media/src/probe.rs`, `crates/opentake-media/src/decode/mod.rs`, `crates/opentake-media/src/encode/mod.rs`, `crates/opentake-media/src/search/mod.rs`, `crates/opentake-media/src/transcribe/mod.rs`, `docs/specs/media/8-coordinator.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media crate_public_types_are_reachable`
  - Run: `cargo test -p opentake-media --test facade_contract all_services_are_reachable_only_through_facade_and_dependencies_stay_acyclic -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-08-01. The planned facade contract first failed to compile
  because `MediaEngine` had no decode, PCM extraction, encoder-construction,
  or visual-ranking methods. The minimal implementation now owns those four
  boundaries while retaining the existing probe, transcript, and spoken-search
  services. The owning integration test generates a deterministic 32x18 A/V
  fixture and passes probe -> frame decode -> 16 kHz mono PCM -> fixture
  transcriber -> H.264 encode -> re-probe through `MediaEngine`; it also ranks
  a persisted visual index and asserts the workspace dependency DAG remains
  acyclic. Both exact owning tests pass, strict all-feature Clippy is clean,
  rustfmt is clean, and the full workspace suite passes.

### Task 36: MR-image-lottie-materialization (implementation-slice-a90703f04ca7e8c5)

**Covered records:**
- `requirement-db6267b61deb36fe` (requirement)

**Files:**
- Modify: `src-tauri/src/render.rs#MediaResolver::resolve`
- Modify: `src-tauri/src/export.rs#MediaResolver::resolve`
- Modify: `src-tauri/src/playback/resolver.rs#StreamingResolver::resolve`
- Modify: `docs/specs/media/8-coordinator.md`
- Test (reviewed-planned): `src-tauri/src/playback/resolver.rs#lottie_cache_lifecycle_frame_modulo_and_preview_export_parity`

**Candidate-bound contracts:**

#### requirement-db6267b61deb36fe

- Candidate/source: `doc-93576f776eb389a0` at `docs/specs/media/8-coordinator.md:27` (requirement)
- Expected behavior: Image and Lottie materialization produce renderable textures with cache/lifecycle ownership.
- Resolution: `reviewed-mapping-report:MR-image-lottie-materialization` — Image materialization exists, while all three production resolver paths return None for Lottie.
- Exact acceptance contract:
  - Implement image and Lottie materialization in the media/render boundary.
  - Define cache invalidation and device-loss behavior.
  - Add pixel, lifecycle, and export tests.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/playback/resolver.rs#lottie_cache_lifecycle_frame_modulo_and_preview_export_parity` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri lottie_cache_lifecycle_frame_modulo_and_preview_export_parity`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/render.rs#MediaResolver::resolve`, `src-tauri/src/export.rs#MediaResolver::resolve`, `src-tauri/src/playback/resolver.rs#StreamingResolver::resolve`, `docs/specs/media/8-coordinator.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri lottie_cache_lifecycle_frame_modulo_and_preview_export_parity`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

**Verified 2026-08-01:** RED was reproduced because the declared
`LottieMaterializer` production boundary did not exist and all three product
resolvers skipped Lottie. The shared Velato/Vello materializer now renders
bounded, content-hashed textures on the existing wgpu device; preview,
dedicated playback, and export own caches at their documented lifetimes and
surface unsupported/invalid documents as explicit failures. The owning GPU
pixel test passes and proves frame modulo, content invalidation, preview/export
byte parity, and device-context rebuild behavior. `cargo fmt --all -- --check`,
`cargo clippy -p opentake-tauri --all-targets --all-features -- -D warnings`,
the focused owning test, and
`CARGO_INCREMENTAL=0 cargo test --workspace --no-fail-fast --quiet` all pass.

### Task 37: MR-interchange-export-complete (implementation-slice-efcc28e98cc9b40e)

**Covered records:**
- `requirement-0c4a155835f2cd05` (requirement)

**Files:**
- Modify: `crates/opentake-project/src/fcpxml.rs#export_xmeml`
- Modify: `crates/opentake-project/src/fcpxml_modern.rs#export_fcpxml`
- Modify: `crates/opentake-project/src/otio.rs#export_otio`
- Modify: `crates/opentake-project/src/edl.rs#export_edl`
- Modify: `src-tauri/src/commands.rs`
- Modify: `web/src/components/shell/TitleBar.tsx#TitleBar`
- Modify: `docs/需求与问题汇总.md`
- Test (existing-owned): `web/src/components/shell/TitleBar.visual.test.ts#offers all four interchange formats with their extensions and commands`
- Test (existing-owned): `crates/opentake-project/src/fcpxml_modern_tests.rs#document_has_fcpxml_header_and_version`
- Test (existing-owned): `crates/opentake-project/src/otio.rs#top_level_is_timeline_schema_with_stack_tracks`
- Test (existing-owned): `crates/opentake-project/src/edl.rs#header_has_title_and_fcm`

**Candidate-bound contracts:**

#### requirement-0c4a155835f2cd05

- Candidate/source: `doc-9ae7ed1acecc8998` at `docs/需求与问题汇总.md:58` (requirement)
- Expected behavior: At docs/需求与问题汇总.md:58 under “F4. 工程文件互操作（可被剪映/PR 等打开）” (heading), the source “### F4. 工程文件互操作（可被剪映/PR 等打开）” requires this exact behavior: Projects export interoperable FCPXML, OTIO, and EDL representations.
- Resolution: `reviewed-mapping-report:MR-interchange-export-complete` — FCPXML and XMEML, OTIO and EDL all have tracked exporters, Tauri and frontend routing, and format tests.
- Exact acceptance contract:
  - Source binding: docs/需求与问题汇总.md:58; signal=heading; heading=F4. 工程文件互操作（可被剪映/PR 等打开）; candidate=### F4. 工程文件互操作（可被剪映/PR 等打开）
  - Expected behavior: Projects export interoperable FCPXML, OTIO, and EDL representations. This closes only the promise expressed by “F4. 工程文件互操作（可被剪映/PR 等打开）” in “F4. 工程文件互操作（可被剪映/PR 等打开）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “F4. 工程文件互操作（可被剪映/PR 等打开）” with the scenario below and register test:crates/opentake-project/tests/completion_9ae7ed1acecc8998.rs#completion_9ae7ed1acecc8998_projects_export_interoperable_fcpxml_otio_and_ed
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “F4. 工程文件互操作（可被剪映/PR 等打开）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Projects export interoperable FCPXML, OTIO, and EDL representations.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_9ae7ed1acecc8998.rs#completion_9ae7ed1acecc8998_projects_export_interoperable_fcpxml_otio_and_ed.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/TitleBar.visual.test.ts#offers all four interchange formats with their extensions and commands` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/src/fcpxml_modern_tests.rs#document_has_fcpxml_header_and_version` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/src/otio.rs#top_level_is_timeline_schema_with_stack_tracks` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/src/edl.rs#header_has_title_and_fcm` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and reconcile the historical baseline**

  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.visual.test.ts -t "offers all four interchange formats with their extensions and commands"`
  - Run: `cargo test -p opentake-project document_has_fcpxml_header_and_version`
  - Run: `cargo test -p opentake-project top_level_is_timeline_schema_with_stack_tracks`
  - Run: `cargo test -p opentake-project header_has_title_and_fcm`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-project/src/fcpxml.rs#export_xmeml`, `crates/opentake-project/src/fcpxml_modern.rs#export_fcpxml`, `crates/opentake-project/src/otio.rs#export_otio`, `crates/opentake-project/src/edl.rs#export_edl`, `src-tauri/src/commands.rs`, `web/src/components/shell/TitleBar.tsx#TitleBar`, `docs/需求与问题汇总.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/TitleBar.visual.test.ts -t "offers all four interchange formats with their extensions and commands"`
  - Run: `cargo test -p opentake-project document_has_fcpxml_header_and_version`
  - Run: `cargo test -p opentake-project top_level_is_timeline_schema_with_stack_tracks`
  - Run: `cargo test -p opentake-project header_has_title_and_fcm`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Reconciled 2026-08-01 as an already-integrated baseline. Git history shows
  the EDL, OTIO and modern FCPXML writers and their format tests landed together
  in `5713d22`, followed by the four-format title-bar route in `3d09eb6`; there
  is therefore no honest test-present/implementation-missing RED command to
  replay. The four declared owners pass on the current descendant, the exact
  title-bar interaction suite covers success, failure, cancellation, extension
  completion and all four Tauri routes, and the full workspace suite passes.
  Existing packaged-app evidence additionally parsed both XML outputs with
  `xmllint`, parsed OTIO with `jq`, verified the corrected three-event EDL, and
  proved deterministic re-export. The historical requirement source now records
  the shipped formats and their explicit degradation boundary; no production
  code change was required for this reconciliation. Runtime evidence:
  [`interchange-export-real-device-2026-07-30.md`](../runtime-artifacts/automated/interchange-export-real-device-2026-07-30.md).

### Task 38: control-acceptance (implementation-slice-5148c914ccdac250)

**Covered records:**
- `control-record-b984d500edfaa1c1` (control)
- `control-record-85b36a2d320f6b33` (control)
- `control-record-b3563b35584e0d40` (control)
- `control-record-328908d28b350e0c` (control)
- `control-record-612e325a2e35abe9` (control)

**Files:**
- Modify: `web/src/components/shell/ExportDialog.tsx`
- Modify: `web/src/components/shell/ExportDialog.tsx#ExportDialog`
- Test (reviewed-planned): `web/src/components/shell/ExportDialog.interaction.test.tsx#control-580ab884755388a9 dismiss Export by clicking the backdrop`
- Test (reviewed-planned): `web/src/components/shell/ExportDialog.interaction.test.tsx#control-6064916ed05a1362 close Export from its header`
- Test (reviewed-planned): `web/src/components/shell/ExportDialog.interaction.test.tsx#control-34646794727cf515 choose export mode`
- Test (reviewed-planned): `web/src/components/shell/ExportDialog.interaction.test.tsx#control-6846958c0e19c8e9 choose export codec`
- Test (reviewed-planned): `web/src/components/shell/ExportDialog.interaction.test.tsx#control-30862b5deb972fcd choose export resolution`

**Candidate-bound contracts:**

#### control-record-b984d500edfaa1c1

- Candidate/source: `control-580ab884755388a9` at `web/src/components/shell/ExportDialog.tsx:359:5` (control)
- Expected behavior: dismiss Export by clicking the backdrop: closes only when not busy
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-580ab884755388a9.
  - Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-580ab884755388a9 dismiss Export by clicking the backdrop.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => { if (!busy) setOpen(false); }}","pointer/drag coordinates, button/key, and current owning state"]; handler={() => { if (!busy) setOpen(false); }}.
  - Exact call/state/backend: stateTransition=closes only when not busy; backendTrace=["web/src/components/shell/ExportDialog.tsx:359::candidate handler -> {() => { if (!busy) setOpen(false); }}","actual branch/state -> closes only when not busy","exact call -> closes only when not busy","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ExportDialog.tsx#ExportDialog"].
  - Visible/accessibility/return path: success=dismiss Export by clicking the backdrop: closes only when not busy; accessibility={"focus":"Backdrop div is pointer-only; dialog has no focus trap/initial focus","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."].
  - Outcome matrix: {"success":"dismiss Export by clicking the backdrop: closes only when not busy","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:359; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in closes only when not busy; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-85b36a2d320f6b33

- Candidate/source: `control-6064916ed05a1362` at `web/src/components/shell/ExportDialog.tsx:401:11` (control)
- Expected behavior: close Export from its header: setExportDialogOpen(false) when not busy
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-6064916ed05a1362.
  - Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-6064916ed05a1362 close Export from its header.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({busy}).
  - Event: inputs=["event/prop handler: {() => setOpen(false)}","click or native keyboard activation plus current owning state"]; handler={() => setOpen(false)}.
  - Exact call/state/backend: stateTransition=setExportDialogOpen(false) when not busy; backendTrace=["web/src/components/shell/ExportDialog.tsx:401::candidate handler -> {() => setOpen(false)}","actual branch/state -> setExportDialogOpen(false) when not busy","exact call -> setExportDialogOpen(false) when not busy","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ExportDialog.tsx#ExportDialog"].
  - Visible/accessibility/return path: success=close Export from its header: setExportDialogOpen(false) when not busy; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"export.close\")","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."].
  - Outcome matrix: {"success":"close Export from its header: setExportDialogOpen(false) when not busy","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:401; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {busy}.","cancel":"Cancellation/dismissal follows the exact guard in setExportDialogOpen(false) when not busy; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-b3563b35584e0d40

- Candidate/source: `control-34646794727cf515` at `web/src/components/shell/ExportDialog.tsx:434:13` (control)
- Expected behavior: choose export mode: onModeChange clears stale error/missing report
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-34646794727cf515.
  - Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-34646794727cf515 choose export mode.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(id) => onModeChange(id)}","click or native keyboard activation plus current owning state"]; handler={(id) => onModeChange(id)}.
  - Exact call/state/backend: stateTransition=onModeChange clears stale error/missing report; backendTrace=["web/src/components/shell/ExportDialog.tsx:434::candidate handler -> {(id) => onModeChange(id)}","actual branch/state -> onModeChange clears stale error/missing report","exact call -> onModeChange clears stale error/missing report","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ExportDialog.tsx#ExportDialog"].
  - Visible/accessibility/return path: success=choose export mode: onModeChange clears stale error/missing report; accessibility={"focus":"Custom Dropdown focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."].
  - Outcome matrix: {"success":"choose export mode: onModeChange clears stale error/missing report","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:434; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in onModeChange clears stale error/missing report; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-328908d28b350e0c

- Candidate/source: `control-6846958c0e19c8e9` at `web/src/components/shell/ExportDialog.tsx:446:17` (control)
- Expected behavior: choose export codec: setCodec controls extension and export request
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-6846958c0e19c8e9.
  - Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-6846958c0e19c8e9 choose export codec.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(id) => setCodec(id)}","click or native keyboard activation plus current owning state"]; handler={(id) => setCodec(id)}.
  - Exact call/state/backend: stateTransition=setCodec controls extension and export request; backendTrace=["web/src/components/shell/ExportDialog.tsx:446::candidate handler -> {(id) => setCodec(id)}","actual branch/state -> setCodec controls extension and export request","exact call -> setCodec controls extension and export request","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ExportDialog.tsx#ExportDialog"].
  - Visible/accessibility/return path: success=choose export codec: setCodec controls extension and export request; accessibility={"focus":"Custom Dropdown focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."].
  - Outcome matrix: {"success":"choose export codec: setCodec controls extension and export request","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:446; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setCodec controls extension and export request; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-612e325a2e35abe9

- Candidate/source: `control-30862b5deb972fcd` at `web/src/components/shell/ExportDialog.tsx:462:17` (control)
- Expected behavior: choose export resolution: setQuality controls export preset
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-30862b5deb972fcd.
  - Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-30862b5deb972fcd choose export resolution.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(id) => setQuality(id)}","click or native keyboard activation plus current owning state"]; handler={(id) => setQuality(id)}.
  - Exact call/state/backend: stateTransition=setQuality controls export preset; backendTrace=["web/src/components/shell/ExportDialog.tsx:462::candidate handler -> {(id) => setQuality(id)}","actual branch/state -> setQuality controls export preset","exact call -> setQuality controls export preset","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ExportDialog.tsx#ExportDialog"].
  - Visible/accessibility/return path: success=choose export resolution: setQuality controls export preset; accessibility={"focus":"Custom Dropdown focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."].
  - Outcome matrix: {"success":"choose export resolution: setQuality controls export preset","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:462; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setQuality controls export preset; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-580ab884755388a9 dismiss Export by clicking the backdrop` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-6064916ed05a1362 close Export from its header` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-34646794727cf515 choose export mode` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-6846958c0e19c8e9 choose export codec` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-30862b5deb972fcd choose export resolution` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-580ab884755388a9 dismiss Export by clicking the backdrop"`
  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-6064916ed05a1362 close Export from its header"`
  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-34646794727cf515 choose export mode"`
  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-6846958c0e19c8e9 choose export codec"`
  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-30862b5deb972fcd choose export resolution"`

  Expected: FAIL because one or more of the 5 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/ExportDialog.tsx`, `web/src/components/shell/ExportDialog.tsx#ExportDialog` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-580ab884755388a9 dismiss Export by clicking the backdrop"`
  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-6064916ed05a1362 close Export from its header"`
  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-34646794727cf515 choose export mode"`
  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-6846958c0e19c8e9 choose export codec"`
  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-30862b5deb972fcd choose export resolution"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 39: control-acceptance (implementation-slice-c6fbd815566d6b64)

**Covered records:**
- `control-record-c6c9e81870a0cc68` (control)

**Files:**
- Modify: `web/src/components/shell/ExportDialog.tsx`
- Modify: `web/src/components/shell/ExportDialog.tsx#onCancel`
- Modify: `web/src/lib/api.ts#cancelExport`
- Modify: `src-tauri/src/export.rs#cancel_export`
- Modify: `web/src/components/shell/ExportDialog.tsx#ExportDialog`
- Test (reviewed-planned): `web/src/components/shell/ExportDialog.interaction.test.tsx#control-af586bdec82ebcc7 cancel/close Export`

**Candidate-bound contracts:**

#### control-record-c6c9e81870a0cc68

- Candidate/source: `control-af586bdec82ebcc7` at `web/src/components/shell/ExportDialog.tsx:569:11` (control)
- Expected behavior: cancel/close Export: idle closes; active video calls cancelExport(operationId)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-af586bdec82ebcc7.
  - Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-af586bdec82ebcc7 cancel/close Export.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onCancel}","click or native keyboard activation plus current owning state"]; handler={onCancel}.
  - Exact call/state/backend: stateTransition=idle closes; active video calls cancelExport(operationId); backendTrace=["web/src/components/shell/ExportDialog.tsx:569::candidate handler -> {onCancel}","actual branch/state -> idle closes; active video calls cancelExport(operationId)","exact call/arguments -> if idle setExportDialogOpen(false); if busy video and activeOperationId exists call cancelExport(activeOperationId)","web/src/components/shell/ExportDialog.tsx::onCancel -> activeOperationId.current -> api.cancelExport(operationId)","web/src/lib/api.ts::cancelExport -> invoke('cancel_export',{operationId})","src-tauri/src/export.rs::cancel_export(operation_id) -> generation-safe cooperative cancellation","code:web/src/components/shell/ExportDialog.tsx#ExportDialog","code:web/src/components/shell/ExportDialog.tsx#onCancel","code:web/src/lib/api.ts#cancelExport","code:src-tauri/src/export.rs#cancel_export"].
  - Visible/accessibility/return path: success=cancel/close Export: idle closes; active video calls cancelExport(operationId); accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."].
  - Outcome matrix: {"success":"cancel/close Export: idle closes; active video calls cancelExport(operationId)","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/shell/ExportDialog.tsx:569; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:569; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in idle closes; active video calls cancelExport(operationId); no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in idle closes; active video calls cancelExport(operationId).","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/shell/ExportDialog.tsx:569; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-af586bdec82ebcc7 cancel/close Export` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-af586bdec82ebcc7 cancel/close Export"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/ExportDialog.tsx`, `web/src/components/shell/ExportDialog.tsx#onCancel`, `web/src/lib/api.ts#cancelExport`, `src-tauri/src/export.rs#cancel_export`, `web/src/components/shell/ExportDialog.tsx#ExportDialog` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-af586bdec82ebcc7 cancel/close Export"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 40: control-acceptance (implementation-slice-73d069e581678e52)

**Covered records:**
- `control-record-f350ffbc0ca5cf8f` (control)

**Files:**
- Modify: `web/src/components/shell/ExportDialog.tsx`
- Modify: `web/src/components/shell/ExportDialog.tsx#onExport`
- Modify: `web/src/lib/api.ts#getDefaultProjectDir`
- Modify: `src-tauri/src/commands.rs`
- Modify: `web/src/lib/api.ts#exportVideo`
- Modify: `src-tauri/src/export.rs#export_video`
- Modify: `web/src/components/shell/ExportDialog.tsx#ExportDialog`
- Test (reviewed-planned): `web/src/components/shell/ExportDialog.interaction.test.tsx#control-543cacc54290eeba start video export`

**Candidate-bound contracts:**

#### control-record-f350ffbc0ca5cf8f

- Candidate/source: `control-543cacc54290eeba` at `web/src/components/shell/ExportDialog.tsx:587:11` (control)
- Expected behavior: start video export: save dialog -> busy/progress -> success/cancel/failure -> cleanup
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-543cacc54290eeba.
  - Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-543cacc54290eeba start video export.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({busy}).
  - Event: inputs=["event/prop handler: {mode === \"bundle\" ? onExportBundle : onExport}","click or native keyboard activation plus current owning state"]; handler={mode === "bundle" ? onExportBundle : onExport}.
  - Exact call/state/backend: stateTransition=save dialog -> busy/progress -> success/cancel/failure -> cleanup; backendTrace=["web/src/components/shell/ExportDialog.tsx:587::candidate handler -> {mode === \"bundle\" ? onExportBundle : onExport}","actual branch/state -> save dialog -> busy/progress -> success/cancel/failure -> cleanup","exact call/arguments -> save exact codec extension (calling getDefaultProjectDir() only when projectPath is null); createExportOperationId('video'); onExportProgress(operationId); exportVideo({outPath,codec,quality},operationId); rendered mode is video so onExportBundle is unreachable","web/src/components/shell/ExportDialog.tsx::onExport -> saveDialog; onExportProgress; exportVideo(req,operationId); toast/error/finally listener cleanup","web/src/lib/api.ts::getDefaultProjectDir -> invoke('get_default_project_dir') via src-tauri/src/commands.rs when projectPath is null","web/src/lib/api.ts::exportVideo -> invoke('export_video',{req,operationId}); cancelExport -> invoke('cancel_export',{operationId})","src-tauri/src/export.rs::export_video(req,operation_id)/cancel_export(operation_id) -> render/ffmpeg export control","code:web/src/components/shell/ExportDialog.tsx#ExportDialog","code:web/src/components/shell/ExportDialog.tsx#onExport","code:web/src/lib/api.ts#getDefaultProjectDir","code:web/src/lib/api.ts#exportVideo","code:src-tauri/src/export.rs#export_video"].
  - Visible/accessibility/return path: success=start video export: save dialog -> busy/progress -> success/cancel/failure -> cleanup; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."].
  - Outcome matrix: {"success":"start video export: save dialog -> busy/progress -> success/cancel/failure -> cleanup","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/shell/ExportDialog.tsx:587; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:587; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {busy}.","cancel":"Cancellation/dismissal follows the exact guard in save dialog -> busy/progress -> success/cancel/failure -> cleanup; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in save dialog -> busy/progress -> success/cancel/failure -> cleanup.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/shell/ExportDialog.tsx:587; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-543cacc54290eeba start video export` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-543cacc54290eeba start video export"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/ExportDialog.tsx`, `web/src/components/shell/ExportDialog.tsx#onExport`, `web/src/lib/api.ts#getDefaultProjectDir`, `src-tauri/src/commands.rs`, `web/src/lib/api.ts#exportVideo`, `src-tauri/src/export.rs#export_video`, `web/src/components/shell/ExportDialog.tsx#ExportDialog` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/ExportDialog.interaction.test.tsx -t "control-543cacc54290eeba start video export"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 41: control-acceptance (implementation-slice-010eea5507e6b9ca)

**Covered records:**
- `control-record-07b1607329b8dfad` (control)

**Files:**
- Modify: `web/src/components/shell/SaveAsProgress.tsx`
- Modify: `web/src/store/editActions.ts#cancelSaveAsMedia`
- Modify: `web/src/lib/api.ts#cancelExport`
- Modify: `src-tauri/src/export.rs#cancel_export`
- Modify: `web/src/components/shell/SaveAsProgress.tsx#SaveAsProgress`
- Test (reviewed-planned): `web/src/components/shell/SaveAsProgress.interaction.test.tsx#control-b0b920085e77d039 cancel Save Clip as Media`

**Candidate-bound contracts:**

#### control-record-07b1607329b8dfad

- Candidate/source: `control-b0b920085e77d039` at `web/src/components/shell/SaveAsProgress.tsx:42:7` (control)
- Expected behavior: cancel Save Clip as Media: when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-b0b920085e77d039.
  - Test: web/src/components/shell/SaveAsProgress.interaction.test.tsx#control-b0b920085e77d039 cancel Save Clip as Media.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({!progress.cancellable || progress.cancelling}).
  - Event: inputs=["event/prop handler: {() => void cancelSaveAsMedia()}","click or native keyboard activation plus current owning state"]; handler={() => void cancelSaveAsMedia()}.
  - Exact call/state/backend: stateTransition=when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast; backendTrace=["web/src/components/shell/SaveAsProgress.tsx:42::candidate handler -> {() => void cancelSaveAsMedia()}","actual branch/state -> when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast","exact call/arguments -> cancelSaveAsMedia(): guard current progress, set cancelling=true, call cancelExport(current.operationId); on rejection restore cancelling=false and toast","web/src/store/editActions.ts::cancelSaveAsMedia -> api.cancelExport(current.operationId)","web/src/lib/api.ts::cancelExport -> invoke('cancel_export',{operationId:current.operationId})","src-tauri/src/export.rs::cancel_export(operation_id)","code:web/src/components/shell/SaveAsProgress.tsx#SaveAsProgress","code:web/src/store/editActions.ts#cancelSaveAsMedia","code:web/src/lib/api.ts#cancelExport","code:src-tauri/src/export.rs#cancel_export"].
  - Visible/accessibility/return path: success=cancel Save Clip as Media: when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The non-modal status stays in the editor and disappears when store progress clears."].
  - Outcome matrix: {"success":"cancel Save Clip as Media: when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/shell/SaveAsProgress.tsx:42; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/SaveAsProgress.tsx:42; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {!progress.cancellable || progress.cancelling}.","cancel":"Cancellation/dismissal follows the exact guard in when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/shell/SaveAsProgress.tsx:42; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/shell/SaveAsProgress.interaction.test.tsx#control-b0b920085e77d039 cancel Save Clip as Media` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/shell/SaveAsProgress.interaction.test.tsx -t "control-b0b920085e77d039 cancel Save Clip as Media"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/shell/SaveAsProgress.tsx`, `web/src/store/editActions.ts#cancelSaveAsMedia`, `web/src/lib/api.ts#cancelExport`, `src-tauri/src/export.rs#cancel_export`, `web/src/components/shell/SaveAsProgress.tsx#SaveAsProgress` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/shell/SaveAsProgress.interaction.test.tsx -t "control-b0b920085e77d039 cancel Save Clip as Media"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

## Shared capability references

- `event-forwarding` / `implementation-slice-c35c845cbc3492dd`: implemented once in `command-contracts`; this group contributes records `requirement-c3edeb41fee8e2a1`, `requirement-2c6c54de3cd8a488`, `requirement-30dda70b4f22a7d2` as acceptance references.
