# Media Render Playback Export Completion Design

**Gap group:** `media-render-playback-export`

**Records:** 68

**Implementation slices:** 42

## Architecture

Close each record as the smallest end-to-end vertical slice while preserving Rust-authoritative state, command/API parity, transactional safety, and explicit pending/empty/failure UI states. A record changes status only after its exact acceptance contract and strongest relevant runtime path pass.

## Record contracts

### requirement-60af541ae52187eb

- Kind: requirement
- Implementation slice: `implementation-slice-a1b6e0f7c700bf1c`
- Candidate: `doc-6f9ca70867547ae7`
- Source citation: `docs/architecture/CAPCUT-GAP.md:7`
- Exact files/symbols: `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-capcut-composite`; matched the exact process/control contract.
- Resolution rationale: This record combines 50-track performance, curve speed, nested sequences, multicam alignment and optical flow; nested and optical have child records and the other three need new child records.
- Test ownership:
  - `crates/opentake-render/tests/composite_acceptance.rs#capcut_children_close_one_composite_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Close the explicitly chosen CapCut-parity gaps: 50-track performance, curve speed, nested sequences, multicam alignment, and optical-flow interpolation.
- Acceptance criteria: Implementation: Define measurable 50-track preview/export budgets and meet them; add non-destructive nested timelines, audio-aligned multicam, speed keyframe curves with render-time mapping, and optical-flow interpolation; cover each with model/ops/render/UI tests and runtime fixtures. Add deterministic media fixtures and golden frame, audio, timeline, or export assertions for every named capability and boundary; the affected render/media suites must pass. Run the packaged preview/export path on representative media and retain exact output or runtime evidence before reclassification.

### requirement-bedfdc6edfa147b9

- Kind: requirement
- Implementation slice: `implementation-slice-b8f61feebde4e2ab`
- Candidate: `doc-2c9413903831faff`
- Source citation: `docs/architecture/CAPCUT-GAP.md:16`
- Exact files/symbols: `crates/opentake-domain/src/clip.rs`, `crates/opentake-render/src/plan/build.rs#build_frame_plan`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-nested-timeline`; matched `build_frame_plan`.
- Resolution rationale: A nested clip reference and recursive render-plan flattening do not exist; the same plan must feed preview and export.
- Test ownership:
  - `crates/opentake-render/tests/nested_timeline.rs#nested_edits_preview_and_export_same_frames` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Represent a clip that references an editable nested timeline.
- Acceptance criteria: Implementation: Add a serialized nested-sequence reference type, dependency/cycle validation, edit commands, RenderPlan recursion/flatten caching, UI enter/exit controls, export support, and round-trip/render tests. Add deterministic media fixtures and golden frame, audio, timeline, or export assertions for every named capability and boundary; the affected render/media suites must pass. Run the packaged preview/export path on representative media and retain exact output or runtime evidence before reclassification.

### requirement-5933e802c9dfe372

- Kind: requirement
- Implementation slice: `implementation-slice-c85c1acc35668396`
- Candidate: `doc-e423ae30effded27`
- Source citation: `docs/architecture/CAPCUT-GAP.md:39`
- Exact files/symbols: `crates/opentake-render/src/gpu/compositor.rs#TextureResolver`, `crates/opentake-media/src/decode/frame.rs`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-optical-flow`; matched `TextureResolver`.
- Resolution rationale: The report identified no current optical-flow backend; source-frame interpolation must enter the shared resolver path.
- Test ownership:
  - `crates/opentake-render/tests/optical_flow.rs#two_frame_fixture_is_deterministic_and_matches_preview_export` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Optical-flow interpolation produces deterministic preview/export frames.
- Acceptance criteria: Add an optical-flow interpolation mode with explicit source/target frame-rate and fallback policy in the render model. Convert a 24 fps motion fixture to 60 fps with exactly the expected output-frame count and unchanged first/last timestamps. Add pixel/temporal regression tests plus a deterministic unsupported-device fallback; preview and export must select the same interpolation mode.

### requirement-0609f4de4f001a49

- Kind: requirement
- Implementation slice: `implementation-slice-dacb1d7732ff3450`
- Candidate: `doc-8b5862a78556aa8c`
- Source citation: `docs/architecture/CAPCUT-GAP.md:61`
- Exact files/symbols: `crates/opentake-domain/src/grade.rs#Mask`, `crates/opentake-render/src/plan/build.rs`, `crates/opentake-render/src/gpu/compositor.rs#pack_masks`, `crates/opentake-render/src/gpu/shader.wgsl`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-mask-rendering`; matched `Mask`, `pack_masks`.
- Resolution rationale: Linear and circular mask paths exist, while polygon masks are deliberately encoded as a no-op and need shared preview/export proof.
- Test ownership:
  - `crates/opentake-render/tests/gpu_effects.rs#circle_mask_clips_to_center` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/tests/gpu_effects.rs#linear_circle_and_polygon_masks_match_cpu_reference_in_preview_and_export` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Linear, circular, and pen/polygon masks render consistently in preview and export.
- Acceptance criteria: Persist linear, circular, and polygon/pen masks with feather, invert, and transform parameters on clips. Expose mask creation, point editing, delete, and undo/redo in Inspector/Preview without mutating source media. Add GPU pixel fixtures for all three shapes at feather 0 and nonzero feather; preview and exported boundary frames must match within the project pixel-diff tolerance.

### requirement-20198476e9083261

- Kind: requirement
- Implementation slice: `implementation-slice-7ef9369889a0a0d6`
- Candidate: `doc-9f4727115dbcfd34`
- Source citation: `docs/architecture/CAPCUT-GAP.md:85`
- Exact files/symbols: `crates/opentake-media/src/analysis/stabilization.rs`, `crates/opentake-ops/src/command.rs`, `crates/opentake-render/src/plan/build.rs`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-stabilization`; matched the exact process/control contract.
- Resolution rationale: No tracked stabilization analyzer or command owner exists; the solution must be represented as editable crop or transform state.
- Test ownership:
  - `crates/opentake-render/tests/stabilization.rs#synthetic_shake_produces_editable_undoable_preview_export_solution` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Stabilization analyzes motion and applies an editable crop/transform solution.
- Acceptance criteria: Persist stabilization analysis as an editable transform/crop track with model/version and source identity. Expose analyze, strength/crop adjustment, cancellation, apply, reset, and undo without overwriting source media. For a synthetic jitter fixture, demonstrate lower frame-to-frame tracked displacement, no uncovered pixels, and preview/export transform parity.

### requirement-0086c30d6dc64f51

- Kind: requirement
- Implementation slice: `implementation-slice-ee2ab54c1e0ae863`
- Candidate: `doc-a3897bfa0c91c4fb`
- Source citation: `docs/architecture/CAPCUT-GAP.md:103`
- Exact files/symbols: `crates/opentake-domain/src/grade.rs#Effect`, `crates/opentake-render/src/plan/types.rs#LayerDraw`, `crates/opentake-render/src/gpu/compositor.rs`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-generic-effects`; matched `Effect`, `LayerDraw`.
- Resolution rationale: Effect metadata reaches the render plan but the generic effect chain has no production pass implementation.
- Test ownership:
  - `crates/opentake-render/tests/gpu_effects.rs#advertised_effect_registry_has_preview_export_golden_fixtures` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Advertised effects and filters have real GPU/CPU render implementations with preview/export parity.
- Acceptance criteria: Define a closed effect registry whose persisted parameter schema is validated; unknown effects must return a typed error instead of silently rendering unchanged. Expose add/reorder/parameter-change/remove operations through undoable Inspector commands. Add one pixel fixture per advertised effect/filter and assert preview/export parity at default and non-default parameters.

### requirement-f5c136c992515801

- Kind: requirement
- Implementation slice: `implementation-slice-36596c6aa0eb94d6`
- Candidate: `doc-ed457b544b875446`
- Source citation: `docs/architecture/CAPCUT-GAP.md:109`
- Exact files/symbols: `crates/opentake-domain/src/transition.rs`, `crates/opentake-render/src/plan/build.rs`, `crates/opentake-render/src/gpu/compositor.rs`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-transitions`; matched the exact process/control contract.
- Resolution rationale: Both records describe one missing editable transition model and overlap render pass.
- Test ownership:
  - `crates/opentake-render/tests/transitions.rs#adjacent_clip_transition_is_editable_undoable_and_matches_preview_export` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Transitions are editable and render with preview/export parity.
- Acceptance criteria: Persist a transition with kind, duration, and both adjacent clip IDs, rejecting overlaps longer than either available handle. Expose add/change/remove transition actions in the enabled transition surface with undo/redo. For cut, midpoint, and end frames of each advertised transition, assert preview/export pixels match and save/reopen preserves the transition exactly.

### requirement-6ee2e382b1d3733c

- Kind: requirement
- Implementation slice: `implementation-slice-4c614d7762698953`
- Candidate: `doc-2016fc49884f6dc7`
- Source citation: `docs/architecture/CAPCUT-GAP.md:127`
- Exact files/symbols: `crates/opentake-domain/src/grade.rs#ColorGrade`, `crates/opentake-render/src/gpu/compositor.rs#grade_blocks`, `crates/opentake-render/src/gpu/shader.wgsl`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-lgg-proof`; matched `ColorGrade`, `grade_blocks`.
- Resolution rationale: Representation and shader code exist, but the record should remain incomplete until GPU output is checked against the CPU reference.
- Test ownership:
  - `crates/opentake-domain/src/grade.rs#lift_gamma_gain_gain_scales` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/tests/gpu_effects.rs#lift_gamma_gain_matches_cpu_reference` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/architecture/CAPCUT-GAP.md:127 under “色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1” (heading), the source “### 色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1” requires this exact behavior: Lift, gamma, and gain controls are represented and rendered.
- Acceptance criteria: Source binding: docs/architecture/CAPCUT-GAP.md:127; signal=heading; heading=色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1; candidate=### 色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1 Expected behavior: Lift, gamma, and gain controls are represented and rendered. This closes only the promise expressed by “色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1” in “色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1” with the scenario below and register test:crates/opentake-render/tests/completion_2016fc49884f6dc7.rs#completion_2016fc49884f6dc7_lift_gamma_and_gain_controls_are_represented_and Initial state/input/event: start from the smallest valid fixture for “Lift, gamma, and gain controls are represented and rendered.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1”. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Lift, gamma, and gain controls are represented and rendered.” once. Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_2016fc49884f6dc7.rs#completion_2016fc49884f6dc7_lift_gamma_and_gain_controls_are_represented_and.

### requirement-30c371e348da001c

- Kind: requirement
- Implementation slice: `implementation-slice-0e1b61977fdbc412`
- Candidate: `doc-9edde6aa1ce22995`
- Source citation: `docs/architecture/CAPCUT-GAP.md:139`
- Exact files/symbols: `crates/opentake-domain/src/grade.rs`, `crates/opentake-render/src/gpu/shader.wgsl`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-hsl-secondary`; matched the exact process/control contract.
- Resolution rationale: No tracked HSL-secondary representation or render path was found.
- Test ownership:
  - `crates/opentake-render/tests/gpu_effects.rs#hsl_secondary_hue_boundary_feather_and_isolation` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: HSL secondary controls are editable and rendered.
- Acceptance criteria: Persist bounded hue-range, feather, hue, saturation, and lightness adjustments in the grade model. Expose range selection and parameter edits with reset and undo/redo in Inspector. Use a color-chart fixture to verify selected hues change while pixels outside the feathered range remain within the project pixel-diff tolerance in preview and export.

### requirement-2156cc0bdb849391

- Kind: requirement
- Implementation slice: `implementation-slice-10ed256c8194fc18`
- Candidate: `doc-0b8b56b94a928929`
- Source citation: `docs/architecture/CAPCUT-GAP.md:145`
- Exact files/symbols: `crates/opentake-domain/src/lut.rs`, `crates/opentake-render/src/gpu/compositor.rs`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-lut`; matched the exact process/control contract.
- Resolution rationale: No validated LUT import, persisted reference, GPU 3D texture or preview/export path exists.
- Test ownership:
  - `crates/opentake-render/tests/lut.rs#malformed_and_oversized_luts_fail_closed_and_valid_lut_matches_preview_export` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Validated 3D LUT files can be imported, previewed, and exported.
- Acceptance criteria: Parse and validate .cube LUT metadata, domain bounds, and 17- and 33-point tables; reject malformed or oversized input with typed errors. Import, select, set intensity, remove, and undo LUT changes without copying arbitrary files outside project-managed storage. Compare identity and known-transform LUT fixtures in GPU preview and export using the existing pixel-diff threshold, including save/reopen.

### requirement-25da8163d71af1bf

- Kind: requirement
- Implementation slice: `implementation-slice-e668f03cd9c414d2`
- Candidate: `doc-30e9287b5c8858dd`
- Source citation: `docs/architecture/CAPCUT-GAP.md:161`
- Exact files/symbols: `crates/opentake-media/src/decode/pcm.rs#extract_pcm`, `crates/opentake-media/src/analysis/loudness.rs`, `src-tauri/src/playback/audio.rs`, `src-tauri/src/export.rs`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-loudness`; matched `extract_pcm`.
- Resolution rationale: PCM, playback and export owners exist, but no loudness analysis or normalization target path exists.
- Test ownership:
  - `crates/opentake-media/tests/loudness.rs#normalization_reaches_configured_lufs_within_tolerance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Audio loudness normalization targets and verifies a configured LUFS value.
- Acceptance criteria: Persist a target integrated loudness and true-peak ceiling as an undoable audio operation. Expose analyze/apply/reset with progress and typed errors for silent or unreadable audio. On speech and music fixtures, exported integrated loudness must be within ±1 LU of the configured target without exceeding the configured true-peak ceiling; preview gain must use the same computed adjustment.

### requirement-58c2159d21084d01

- Kind: requirement
- Implementation slice: `implementation-slice-3d159672cfd1fc67`
- Candidate: `doc-d4fe9b623de65daa`
- Source citation: `docs/architecture/CAPCUT-GAP.md:167`
- Exact files/symbols: `crates/opentake-media/src/analysis/denoise.rs`, `src-tauri/src/playback/audio.rs`, `src-tauri/src/export.rs`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-denoise`; matched the exact process/control contract.
- Resolution rationale: No tracked denoise node exists; playback and export must consume the same processing owner.
- Test ownership:
  - `crates/opentake-media/tests/denoise.rs#deterministic_noise_fixture_and_bypass` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `src-tauri/src/playback/audio.rs#denoise_preview_uses_shared_processing_owner` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `src-tauri/src/export.rs#denoise_export_uses_shared_processing_owner` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Audio denoise is available in preview and export.
- Acceptance criteria: Persist denoise mode/strength parameters and keep the source audio immutable. Expose preview toggle, apply/reset, cancellation, and undo/redo in the Audio Inspector. On a speech-plus-noise fixture, assert at least 3 dB SNR improvement with no clipping, and verify preview/export use identical denoise parameters.

### requirement-7c518a7042e8d780

- Kind: requirement
- Implementation slice: `implementation-slice-9139657f9c8c7ff5`
- Candidate: `doc-8a0fdfbeaab482c5`
- Source citation: `docs/architecture/CAPCUT-GAP.md:176`
- Exact files/symbols: `crates/opentake-media/src/analysis/stems.rs`, `crates/opentake-gen/src/stems.rs`, `crates/opentake-core/src/session.rs`, `docs/architecture/CAPCUT-GAP.md`
- Target resolution: `reviewed-mapping-report:MR-stems`; matched the exact process/control contract.
- Resolution rationale: No stem separation implementation was found; results must re-enter the shared media import and provenance path.
- Test ownership:
  - `crates/opentake-media/tests/stems.rs#local_or_explicit_provider_selection_cancellation_provenance_and_cleanup` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Separate vocals/music/stems locally or through an explicitly configured generation provider.
- Acceptance criteria: Implementation: Implement an asynchronous stem-separation job with model installation/integrity checks, progress/cancel, derived-asset import, privacy/error UX, and audio-quality/integration fixtures; document local versus hosted execution. Add deterministic media fixtures and golden frame, audio, timeline, or export assertions for every named capability and boundary; the affected render/media suites must pass. Run the packaged preview/export path on representative media and retain exact output or runtime evidence before reclassification.

### requirement-06faee34d4b29a33

- Kind: requirement
- Implementation slice: `implementation-slice-cbce9a4174a73347`
- Candidate: `doc-1c3d81a8b3ab2d59`
- Source citation: `docs/architecture/EDITING-ENGINE-PLAN.md:33`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs`, `crates/opentake-ops/src/ops/place.rs`, `docs/architecture/EDITING-ENGINE-PLAN.md`
- Target resolution: `reviewed-mapping-report:MR-linked-audio-complete`; matched the exact process/control contract.
- Resolution rationale: Both add and insert command paths directly prove that a probed silent video does not create linked audio.
- Test ownership:
  - `crates/opentake-agent/src/mcp/dispatch.rs#add_clips_does_not_link_audio_when_source_has_no_audio` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#insert_clips_does_not_link_audio_when_source_has_no_audio` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/architecture/EDITING-ENGINE-PLAN.md:33 under “2. 链接音频(linked audio)—— 这是上游既定设计,不是 bug” (gap-marker), the source “- **无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。” requires this exact behavior: Do not create a linked audio track when probe proves a video has zero audio channels.
- Acceptance criteria: Source binding: docs/architecture/EDITING-ENGINE-PLAN.md:33; signal=gap-marker; heading=2. 链接音频(linked audio)—— 这是上游既定设计,不是 bug; candidate=- **无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。 Expected behavior: Do not create a linked audio track when probe proves a video has zero audio channels. This closes only the promise expressed by “**无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。” in “2. 链接音频(linked audio)—— 这是上游既定设计,不是 bug”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “**无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。” with the scenario below and register test:crates/opentake-render/tests/completion_1c3d81a8b3ab2d59.rs#completion_1c3d81a8b3ab2d59_do_not_create_a_linked_audio_track_when_probe_pr Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “**无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Do not create a linked audio track when probe proves a video has zero audio channels.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_1c3d81a8b3ab2d59.rs#completion_1c3d81a8b3ab2d59_do_not_create_a_linked_audio_track_when_probe_pr.

### requirement-3333d0cfd4a2fa31

- Kind: requirement
- Implementation slice: `implementation-slice-d2a7a5861f5ebc9b`
- Candidate: `doc-4c04a063f12b37c9`
- Source citation: `docs/architecture/HANDOFF-2026-07.md:171`
- Exact files/symbols: `docs/architecture/HANDOFF-2026-07.md`
- Target resolution: `reviewed-mapping-report:MR-hdr-proxy-account-composite`; matched the exact process/control contract.
- Resolution rationale: HDR, proxy media and account state belong to three different product owners and must be split.
- Test ownership:
  - `crates/opentake-render/tests/composite_acceptance.rs#hdr_proxy_account_children_close_one_composite_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: HDR, proxy media, and account state work as integrated desktop features.
- Acceptance criteria: Implement HDR metadata/color handling and validate preview/export. Implement proxy creation, switching, relink, and persistence. Finish account/provider session integration and cover offline/reopen behavior.

### requirement-73810a059793a1b8

- Kind: requirement
- Implementation slice: `implementation-slice-36596c6aa0eb94d6`
- Candidate: `doc-470e38525e685a91`
- Source citation: `docs/architecture/HANDOFF-2026-07.md:178`
- Exact files/symbols: `crates/opentake-domain/src/transition.rs`, `crates/opentake-render/src/plan/build.rs`, `crates/opentake-render/src/gpu/compositor.rs`, `docs/architecture/HANDOFF-2026-07.md`
- Target resolution: `reviewed-mapping-report:MR-transitions`; matched the exact process/control contract.
- Resolution rationale: Both records describe one missing editable transition model and overlap render pass.
- Test ownership:
  - `crates/opentake-render/tests/transitions.rs#adjacent_clip_transition_is_editable_undoable_and_matches_preview_export` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Transitions are selectable, editable, and rendered.
- Acceptance criteria: Define transition model and edit commands. Enable the transitions media surface. Add pixel/runtime tests for preview/export parity and undo.

### requirement-b598e7822e10bf65

- Kind: requirement
- Implementation slice: `implementation-slice-d1864a5db0605004`
- Candidate: `doc-af7c3db39bbed7c9`
- Source citation: `docs/architecture/HANDOFF-2026-07.md:186`
- Exact files/symbols: `src-tauri/src/playback/audio.rs#mix_timeline_stereo`, `src-tauri/src/export.rs#mix_timeline_audio`, `crates/opentake-media/src/encode/mod.rs#VideoEncoder`, `docs/architecture/HANDOFF-2026-07.md`
- Target resolution: `reviewed-mapping-report:MR-bounded-audio-streaming`; matched `mix_timeline_stereo`, `mix_timeline_audio`, `VideoEncoder`.
- Resolution rationale: Current chunking serves cancellation loops but the whole timeline mix is still allocated in memory.
- Test ownership:
  - `src-tauri/src/playback/audio.rs#large_mix_observes_cancellation_between_chunks` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/playback/audio.rs#long_timeline_mix_has_constant_peak_allocation_and_matches_short_reference` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Long timelines stream bounded audio chunks without loading the entire mix.
- Acceptance criteria: Replace preload-mix playback with bounded chunk scheduling. Handle seek, pause, resume, underrun, and cancellation. Add long-duration memory and A/V sync runtime tests.

### requirement-4b55f25a5196f7e3

- Kind: requirement
- Implementation slice: `implementation-slice-827681eebfb87194`
- Candidate: `doc-41a8a7a62b46a85e`
- Source citation: `docs/architecture/HANDOFF-2026-07.md:191`
- Exact files/symbols: `docs/architecture/HANDOFF-2026-07.md`
- Target resolution: `reviewed-mapping-report:MR-renderer-debt-composite`; matched the exact process/control contract.
- Resolution rationale: Renderer, packaging, settings, solo and stub debt cannot be owned or accepted as one implementation slice.
- Test ownership:
  - `crates/opentake-render/tests/composite_acceptance.rs#renderer_debt_children_close_one_composite_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: The listed renderer, packaging, settings, solo, and stub debt is closed with automated evidence.
- Acceptance criteria: Implement Lottie/motion materialization, track-solo playback/export semantics, settings/model/provider persistence, and all residual InspectMedia/generate/upscale/motion/batch library dispatch paths. Bundle verified FFmpeg/ffprobe sidecars for packaged macOS/Windows and return typed unsupported errors for any renderer/tool capability still unavailable; advertise no placeholder success. Pass motion/Lottie pixel fixtures, solo A/V mix tests, settings secret/restart/list_models tests, per-tool schema/failure/undo matrix, and installed-app probe/decode/playback/export smoke on both targets.

### requirement-dd35062c2778f365

- Kind: requirement
- Implementation slice: `implementation-slice-843431dc47b8e0d0`
- Candidate: `doc-75908a18e877e120`
- Source citation: `docs/architecture/PLAYBACK-ENGINE.md:6`
- Exact files/symbols: `web/src/components/preview/playbackRoute.ts#resolveTimelinePlaybackRoute`, `web/src/components/preview/nativePlaybackSession.ts`, `web/src/components/preview/rustFrameBuffer.ts`, `src-tauri/src/playback/engine.rs#PlaybackEngine`, `src-tauri/src/playback/resolver.rs#PlaybackResolverState`, `docs/architecture/PLAYBACK-ENGINE.md`
- Target resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete`; matched `resolveTimelinePlaybackRoute`, `PlaybackEngine`, `PlaybackResolverState`.
- Resolution rationale: Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Test ownership:
  - `web/src/components/preview/playbackRoute.test.ts#routes plain forward video to WebKit` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/nativePlaybackSession.test.ts#publishes only increasing matching frame sequences` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/rustFrameBuffer.test.ts#keeps two stable Rust frame image slots mounted` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/playback/resolver.rs#drain_propagates_stream_failure_instead_of_freezing_cache` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:6 under “Capability route is the sole authority” (heading), the source “## Capability route is the sole authority” requires this exact behavior: The playback subsystem implements capability route is the sole authority with focused route/lifecycle tests.
- Acceptance criteria: Source binding: docs/architecture/PLAYBACK-ENGINE.md:6; signal=heading; heading=Capability route is the sole authority; candidate=## Capability route is the sole authority Expected behavior: The playback subsystem implements capability route is the sole authority with focused route/lifecycle tests. This closes only the promise expressed by “Capability route is the sole authority” in “Capability route is the sole authority”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Capability route is the sole authority” with the scenario below and register test:crates/opentake-render/tests/completion_75908a18e877e120.rs#completion_75908a18e877e120_the_playback_subsystem_implements_capability_rou Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Capability route is the sole authority”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements capability route is the sole authority with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_75908a18e877e120.rs#completion_75908a18e877e120_the_playback_subsystem_implements_capability_rou.

### requirement-b8176cd58b4673fc

- Kind: requirement
- Implementation slice: `implementation-slice-843431dc47b8e0d0`
- Candidate: `doc-a6e62065f311d1d7`
- Source citation: `docs/architecture/PLAYBACK-ENGINE.md:24`
- Exact files/symbols: `web/src/components/preview/playbackRoute.ts#resolveTimelinePlaybackRoute`, `web/src/components/preview/nativePlaybackSession.ts`, `web/src/components/preview/rustFrameBuffer.ts`, `src-tauri/src/playback/engine.rs#PlaybackEngine`, `src-tauri/src/playback/resolver.rs#PlaybackResolverState`, `docs/architecture/PLAYBACK-ENGINE.md`
- Target resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete`; matched `resolveTimelinePlaybackRoute`, `PlaybackEngine`, `PlaybackResolverState`.
- Resolution rationale: Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Test ownership:
  - `web/src/components/preview/playbackRoute.test.ts#routes plain forward video to WebKit` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/nativePlaybackSession.test.ts#publishes only increasing matching frame sequences` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/rustFrameBuffer.test.ts#keeps two stable Rust frame image slots mounted` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/playback/resolver.rs#drain_propagates_stream_failure_instead_of_freezing_cache` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:24 under “WebKit route” (heading), the source “## WebKit route” requires this exact behavior: The playback subsystem implements webkit route with focused route/lifecycle tests.
- Acceptance criteria: Source binding: docs/architecture/PLAYBACK-ENGINE.md:24; signal=heading; heading=WebKit route; candidate=## WebKit route Expected behavior: The playback subsystem implements webkit route with focused route/lifecycle tests. This closes only the promise expressed by “WebKit route” in “WebKit route”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “WebKit route” with the scenario below and register test:web/src/__tests__/completion/doc-a6e62065f311d1d7.test.ts#completion_a6e62065f311d1d7_the_playback_subsystem_implements_webkit_route_w Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “WebKit route”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements webkit route with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-a6e62065f311d1d7.test.ts#completion_a6e62065f311d1d7_the_playback_subsystem_implements_webkit_route_w.

### requirement-e6e1400ed457bbb9

- Kind: requirement
- Implementation slice: `implementation-slice-843431dc47b8e0d0`
- Candidate: `doc-30a2bd351d4d52a2`
- Source citation: `docs/architecture/PLAYBACK-ENGINE.md:32`
- Exact files/symbols: `web/src/components/preview/playbackRoute.ts#resolveTimelinePlaybackRoute`, `web/src/components/preview/nativePlaybackSession.ts`, `web/src/components/preview/rustFrameBuffer.ts`, `src-tauri/src/playback/engine.rs#PlaybackEngine`, `src-tauri/src/playback/resolver.rs#PlaybackResolverState`, `docs/architecture/PLAYBACK-ENGINE.md`
- Target resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete`; matched `resolveTimelinePlaybackRoute`, `PlaybackEngine`, `PlaybackResolverState`.
- Resolution rationale: Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Test ownership:
  - `web/src/components/preview/playbackRoute.test.ts#routes plain forward video to WebKit` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/nativePlaybackSession.test.ts#publishes only increasing matching frame sequences` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/rustFrameBuffer.test.ts#keeps two stable Rust frame image slots mounted` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/playback/resolver.rs#drain_propagates_stream_failure_instead_of_freezing_cache` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:32 under “Rust route and exact publication” (heading), the source “## Rust route and exact publication” requires this exact behavior: The playback subsystem implements rust route and exact publication with focused route/lifecycle tests.
- Acceptance criteria: Source binding: docs/architecture/PLAYBACK-ENGINE.md:32; signal=heading; heading=Rust route and exact publication; candidate=## Rust route and exact publication Expected behavior: The playback subsystem implements rust route and exact publication with focused route/lifecycle tests. This closes only the promise expressed by “Rust route and exact publication” in “Rust route and exact publication”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Rust route and exact publication” with the scenario below and register test:crates/opentake-render/tests/completion_30a2bd351d4d52a2.rs#completion_30a2bd351d4d52a2_the_playback_subsystem_implements_rust_route_and Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Rust route and exact publication”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements rust route and exact publication with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_30a2bd351d4d52a2.rs#completion_30a2bd351d4d52a2_the_playback_subsystem_implements_rust_route_and.

### requirement-3aa21ae6148b5fcd

- Kind: requirement
- Implementation slice: `implementation-slice-843431dc47b8e0d0`
- Candidate: `doc-ac35095c99a0761b`
- Source citation: `docs/architecture/PLAYBACK-ENGINE.md:52`
- Exact files/symbols: `web/src/components/preview/playbackRoute.ts#resolveTimelinePlaybackRoute`, `web/src/components/preview/nativePlaybackSession.ts`, `web/src/components/preview/rustFrameBuffer.ts`, `src-tauri/src/playback/engine.rs#PlaybackEngine`, `src-tauri/src/playback/resolver.rs#PlaybackResolverState`, `docs/architecture/PLAYBACK-ENGINE.md`
- Target resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete`; matched `resolveTimelinePlaybackRoute`, `PlaybackEngine`, `PlaybackResolverState`.
- Resolution rationale: Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Test ownership:
  - `web/src/components/preview/playbackRoute.test.ts#routes plain forward video to WebKit` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/nativePlaybackSession.test.ts#publishes only increasing matching frame sequences` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/rustFrameBuffer.test.ts#keeps two stable Rust frame image slots mounted` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/playback/resolver.rs#drain_propagates_stream_failure_instead_of_freezing_cache` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:52 under “Lifecycle, control, and bootstrap” (heading), the source “## Lifecycle, control, and bootstrap” requires this exact behavior: The playback subsystem implements lifecycle, control, and bootstrap with focused route/lifecycle tests.
- Acceptance criteria: Source binding: docs/architecture/PLAYBACK-ENGINE.md:52; signal=heading; heading=Lifecycle, control, and bootstrap; candidate=## Lifecycle, control, and bootstrap Expected behavior: The playback subsystem implements lifecycle, control, and bootstrap with focused route/lifecycle tests. This closes only the promise expressed by “Lifecycle, control, and bootstrap” in “Lifecycle, control, and bootstrap”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Lifecycle, control, and bootstrap” with the scenario below and register test:crates/opentake-render/tests/completion_ac35095c99a0761b.rs#completion_ac35095c99a0761b_the_playback_subsystem_implements_lifecycle_cont Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Lifecycle, control, and bootstrap”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements lifecycle, control, and bootstrap with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_ac35095c99a0761b.rs#completion_ac35095c99a0761b_the_playback_subsystem_implements_lifecycle_cont.

### requirement-55ea08b7a51b30ed

- Kind: requirement
- Implementation slice: `implementation-slice-843431dc47b8e0d0`
- Candidate: `doc-cd317d4fe595484f`
- Source citation: `docs/architecture/PLAYBACK-ENGINE.md:78`
- Exact files/symbols: `web/src/components/preview/playbackRoute.ts#resolveTimelinePlaybackRoute`, `web/src/components/preview/nativePlaybackSession.ts`, `web/src/components/preview/rustFrameBuffer.ts`, `src-tauri/src/playback/engine.rs#PlaybackEngine`, `src-tauri/src/playback/resolver.rs#PlaybackResolverState`, `docs/architecture/PLAYBACK-ENGINE.md`
- Target resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete`; matched `resolveTimelinePlaybackRoute`, `PlaybackEngine`, `PlaybackResolverState`.
- Resolution rationale: Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Test ownership:
  - `web/src/components/preview/playbackRoute.test.ts#routes plain forward video to WebKit` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/nativePlaybackSession.test.ts#publishes only increasing matching frame sequences` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/rustFrameBuffer.test.ts#keeps two stable Rust frame image slots mounted` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/playback/resolver.rs#drain_propagates_stream_failure_instead_of_freezing_cache` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:78 under “Retained-frame handoff” (heading), the source “## Retained-frame handoff” requires this exact behavior: The playback subsystem implements retained-frame handoff with focused route/lifecycle tests.
- Acceptance criteria: Source binding: docs/architecture/PLAYBACK-ENGINE.md:78; signal=heading; heading=Retained-frame handoff; candidate=## Retained-frame handoff Expected behavior: The playback subsystem implements retained-frame handoff with focused route/lifecycle tests. This closes only the promise expressed by “Retained-frame handoff” in “Retained-frame handoff”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Retained-frame handoff” with the scenario below and register test:crates/opentake-render/tests/completion_cd317d4fe595484f.rs#completion_cd317d4fe595484f_the_playback_subsystem_implements_retained_frame Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Retained-frame handoff”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements retained-frame handoff with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_cd317d4fe595484f.rs#completion_cd317d4fe595484f_the_playback_subsystem_implements_retained_frame.

### requirement-34258095f740e104

- Kind: requirement
- Implementation slice: `implementation-slice-843431dc47b8e0d0`
- Candidate: `doc-428f02e756802f9f`
- Source citation: `docs/architecture/PLAYBACK-ENGINE.md:88`
- Exact files/symbols: `web/src/components/preview/playbackRoute.ts#resolveTimelinePlaybackRoute`, `web/src/components/preview/nativePlaybackSession.ts`, `web/src/components/preview/rustFrameBuffer.ts`, `src-tauri/src/playback/engine.rs#PlaybackEngine`, `src-tauri/src/playback/resolver.rs#PlaybackResolverState`, `docs/architecture/PLAYBACK-ENGINE.md`
- Target resolution: `reviewed-mapping-report:MR-playback-route-lifecycle-complete`; matched `resolveTimelinePlaybackRoute`, `PlaybackEngine`, `PlaybackResolverState`.
- Resolution rationale: Capability routing, WebKit and Rust ownership, session identity, retained-frame handoff and source identity have direct focused tests.
- Test ownership:
  - `web/src/components/preview/playbackRoute.test.ts#routes plain forward video to WebKit` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/nativePlaybackSession.test.ts#publishes only increasing matching frame sequences` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/preview/rustFrameBuffer.test.ts#keeps two stable Rust frame image slots mounted` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/playback/resolver.rs#drain_propagates_stream_failure_instead_of_freezing_cache` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/architecture/PLAYBACK-ENGINE.md:88 under “Project/source identity and prewarm/cache” (heading), the source “## Project/source identity and prewarm/cache” requires this exact behavior: The playback subsystem implements project/source identity and prewarm/cache with focused route/lifecycle tests.
- Acceptance criteria: Source binding: docs/architecture/PLAYBACK-ENGINE.md:88; signal=heading; heading=Project/source identity and prewarm/cache; candidate=## Project/source identity and prewarm/cache Expected behavior: The playback subsystem implements project/source identity and prewarm/cache with focused route/lifecycle tests. This closes only the promise expressed by “Project/source identity and prewarm/cache” in “Project/source identity and prewarm/cache”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “Project/source identity and prewarm/cache” with the scenario below and register test:crates/opentake-project/tests/completion_428f02e756802f9f.rs#completion_428f02e756802f9f_the_playback_subsystem_implements_project_source Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Project/source identity and prewarm/cache”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The playback subsystem implements project/source identity and prewarm/cache with focused route/lifecycle tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_428f02e756802f9f.rs#completion_428f02e756802f9f_the_playback_subsystem_implements_project_source.

### requirement-36fc46ebba504588

- Kind: requirement
- Implementation slice: `implementation-slice-dd9855c810140649`
- Candidate: `doc-ef91c43e6cb92a53`
- Source citation: `docs/architecture/PLAYBACK-ENGINE.md:117`
- Exact files/symbols: `docs/architecture/PLAYBACK-ENGINE.md`
- Target resolution: `reviewed-mapping-report:MR-release-readiness-composite`; matched the exact process/control contract.
- Resolution rationale: Packaged macOS and Windows release readiness is a cross-slice runtime evidence gate, not one product implementation.
- Test ownership:
  - `crates/opentake-render/tests/composite_acceptance.rs#release_readiness_children_close_one_composite_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Playback/export is release-ready across packaged macOS and Windows with all declared capabilities rendered or explicitly rejected.
- Acceptance criteria: Complete installed-app export UI artifact and packaged FFmpeg/sidecar validation. Close Windows WebView2/CSP/sidecar and signing/notarization acceptance. Implement or fail closed for Lottie, polygon masks, unsupported effects, composited reverse/speed, and complete ProRes/A/V device probes.

### requirement-ec4d078ac55f6037

- Kind: requirement
- Implementation slice: `implementation-slice-0720e74ad73f5976`
- Candidate: `doc-c79d6d91b3f2e347`
- Source citation: `docs/architecture/ROADMAP.md:35`
- Exact files/symbols: `docs/architecture/ROADMAP.md`
- Target resolution: `reviewed-mapping-report:MR-advanced-shader-composite`; matched the exact process/control contract.
- Resolution rationale: Transitions, masks, LUT and curves have independent owners and child slices.
- Test ownership:
  - `crates/opentake-render/tests/composite_acceptance.rs#advanced_shader_children_close_one_composite_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Complete the advanced shader/effect framework, including transitions, masks, LUT/curves, and preview/export parity.
- Acceptance criteria: Implement persisted transitions, polygon masks, RGB/HSL/LUT grading, and every advertised shader effect with typed unsupported-mode errors. Expose parameter editing and undo/redo for each effect family and keep one capability route for preview/export. Add GPU pixel fixtures for default/non-default parameters and assert preview/export parity on supported hardware plus deterministic fallback on unsupported hardware.

### requirement-f28087239427d70e

- Kind: requirement
- Implementation slice: `implementation-slice-103e52462204b36a`
- Candidate: `doc-9900e773f8c063a8`
- Source citation: `docs/modules/opentake-gen/client-transport.md:59`
- Exact files/symbols: `crates/opentake-gen/src/job.rs#GenerationJob`, `docs/modules/opentake-gen/client-transport.md`
- Target resolution: `reviewed-mapping-report:MR-generation-job-serde-complete`; matched `GenerationJob`.
- Resolution rationale: Both id shapes and absent optional fields are handled and directly tested.
- Test ownership:
  - `crates/opentake-gen/src/job.rs#deserializes_proxy_shape_with_id` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-gen/src/job.rs#deserializes_upstream_shape_with_underscore_id` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-gen/client-transport.md:59 under “`job.rs` —— 统一 Job 抽象” (gap-marker), the source “- 字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。” requires this exact behavior: Decode both proxy id and upstream _id job shapes while tolerating missing optional result, error, cost, and completion fields.
- Acceptance criteria: Source binding: docs/modules/opentake-gen/client-transport.md:59; signal=gap-marker; heading=`job.rs` —— 统一 Job 抽象; candidate=- 字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。 Expected behavior: Decode both proxy id and upstream _id job shapes while tolerating missing optional result, error, cost, and completion fields. This closes only the promise expressed by “字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。” in “`job.rs` —— 统一 Job 抽象”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。” with the scenario below and register test:crates/opentake-project/tests/completion_9900e773f8c063a8.rs#completion_9900e773f8c063a8_decode_both_proxy_id_and_upstream_id_job_shapes_ Initial state/input/event: start from the smallest valid fixture for “Decode both proxy id and upstream _id job shapes while tolerating missing optional result, error, cost, and completion fields.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。”. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Decode both proxy id and upstream _id job shapes while tolerating missing optional result, error, cost, and completion fields.” once. Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_9900e773f8c063a8.rs#completion_9900e773f8c063a8_decode_both_proxy_id_and_upstream_id_job_shapes_.

### requirement-e71abff7d9b41127

- Kind: requirement
- Implementation slice: `implementation-slice-bdb1294b5e15ccf0`
- Candidate: `doc-ee5f3f6b2ccb9cc6`
- Source citation: `docs/modules/opentake-media/OVERVIEW.md:55`
- Exact files/symbols: `crates/opentake-media/src/ff.rs#ffmpeg_path`, `crates/opentake-media/src/ff.rs#ffprobe_path`, `crates/opentake-media/Cargo.toml`, `docs/modules/opentake-media/OVERVIEW.md`
- Target resolution: `reviewed-mapping-report:MR-cli-sidecar-boundary-complete`; matched `ffmpeg_path`, `ffprobe_path`.
- Resolution rationale: The media crate deliberately uses ffmpeg-sidecar and does not link a libav ABI binding.
- Test ownership:
  - `crates/opentake-media/src/ff.rs#env_override_is_respected_for_ffmpeg` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/ff.rs#default_ffprobe_is_ffprobe` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-media/OVERVIEW.md:55 under “FFmpeg sidecar 编解码（与 SPEC 的关键实现偏差）” (gap-marker), the source “**实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](../../../modules/opentake-media/SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](../../../modules/opentake-media/probe-ff.md)。” requires this exact behavior: Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding.
- Acceptance criteria: Source binding: docs/modules/opentake-media/OVERVIEW.md:55; signal=gap-marker; heading=FFmpeg sidecar 编解码（与 SPEC 的关键实现偏差）; candidate=**实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](../../../modules/opentake-media/SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](../../../modules/opentake-media/probe-ff.md)。 Expected behavior: Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding. This closes only the promise expressed by “*实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](../../../modules/opentake-media/SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](../../../modules/opentake-media/probe-ff.md)。” in “FFmpeg sidecar 编解码（与 SPEC 的关键实现偏差）”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “*实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](../../../modules/opentake-media/SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](../../../modules/opentake-media/probe-ff.md)。” with the scenario below and register test:crates/opentake-project/tests/completion_ee5f3f6b2ccb9cc6.rs#completion_ee5f3f6b2ccb9cc6_use_the_ffmpeg_ffprobe_sidecar_boundary_rather_t Initial state/input/event: create an isolated temporary project fixture representing the source state in “*实现走 `ffmpeg` / `ffprobe` 命令行二进制（ffmpeg-sidecar），不链接 `libav*`。** 原因：本机工具链为 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持，且 `pkg-config` 缺失。`ff.rs` 封装二进制发现与一次性 ffprobe JSON 查询，上层解码模块用裸 stdin/stdout 管道交换原始像素/PCM。这与多处架构文档（[ARCHITECTURE.md](../../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../../architecture/ROADMAP.md) Phase 2、本模块 [SPEC.md](../../../modules/opentake-media/SPEC.md) §1.2 仍写 `ffmpeg-next`）存在偏差——以代码为准，详见 [probe-ff.md](../../../modules/opentake-media/probe-ff.md)。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_ee5f3f6b2ccb9cc6.rs#completion_ee5f3f6b2ccb9cc6_use_the_ffmpeg_ffprobe_sidecar_boundary_rather_t.

### requirement-d7bd977b01924877

- Kind: requirement
- Implementation slice: `implementation-slice-bdb1294b5e15ccf0`
- Candidate: `doc-fd42ddbda4988918`
- Source citation: `docs/modules/opentake-media/probe-ff.md:17`
- Exact files/symbols: `crates/opentake-media/src/ff.rs#ffmpeg_path`, `crates/opentake-media/src/ff.rs#ffprobe_path`, `crates/opentake-media/Cargo.toml`, `docs/modules/opentake-media/probe-ff.md`
- Target resolution: `reviewed-mapping-report:MR-cli-sidecar-boundary-complete`; matched `ffmpeg_path`, `ffprobe_path`.
- Resolution rationale: The media crate deliberately uses ffmpeg-sidecar and does not link a libav ABI binding.
- Test ownership:
  - `crates/opentake-media/src/ff.rs#env_override_is_respected_for_ffmpeg` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/ff.rs#default_ffprobe_is_ffprobe` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-media/probe-ff.md:17 under “关键决策：为何 CLI sidecar 而非 libav 绑定” (gap-marker), the source “`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。” requires this exact behavior: Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding.
- Acceptance criteria: Source binding: docs/modules/opentake-media/probe-ff.md:17; signal=gap-marker; heading=关键决策：为何 CLI sidecar 而非 libav 绑定; candidate=`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。 Expected behavior: Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding. This closes only the promise expressed by “`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。” in “关键决策：为何 CLI sidecar 而非 libav 绑定”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。” with the scenario below and register test:crates/opentake-project/tests/completion_fd42ddbda4988918.rs#completion_fd42ddbda4988918_use_the_ffmpeg_ffprobe_sidecar_boundary_rather_t Initial state/input/event: create an isolated temporary project fixture representing the source state in “`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Use the ffmpeg/ffprobe sidecar boundary rather than a libav ABI binding.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_fd42ddbda4988918.rs#completion_fd42ddbda4988918_use_the_ffmpeg_ffprobe_sidecar_boundary_rather_t.

### requirement-b08d2435a038bd02

- Kind: requirement
- Implementation slice: `implementation-slice-ee5b0fb9f6f3c487`
- Candidate: `doc-01ed7e50adfe68f9`
- Source citation: `docs/modules/opentake-motion/OVERVIEW.md:88`
- Exact files/symbols: `crates/opentake-motion/src/integration.rs#MotionClipSource::new`, `docs/modules/opentake-motion/OVERVIEW.md`
- Target resolution: `reviewed-mapping-report:MR-motion-decoder-injection-complete`; matched `MotionClipSource::new`.
- Resolution rationale: Frame decoding is injected through a closure and the production crate does not own an image decoder dependency.
- Test ownership:
  - `crates/opentake-motion/src/integration.rs#decoded_frame_returns_rgba_of_right_shape` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/tests/pipeline.rs#full_pipeline_render_cache_and_ingest` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-motion/OVERVIEW.md:88 under “与 render 的集成桥” (gap-marker), the source “- 帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。” requires this exact behavior: Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion.
- Acceptance criteria: Source binding: docs/modules/opentake-motion/OVERVIEW.md:88; signal=gap-marker; heading=与 render 的集成桥; candidate=- 帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。 Expected behavior: Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion. This closes only the promise expressed by “帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。” in “与 render 的集成桥”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。” with the scenario below and register test:crates/opentake-project/tests/completion_01ed7e50adfe68f9.rs#completion_01ed7e50adfe68f9_inject_frame_decoding_into_motionclipsource_with Initial state/input/event: create an isolated temporary project fixture representing the source state in “帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_01ed7e50adfe68f9.rs#completion_01ed7e50adfe68f9_inject_frame_decoding_into_motionclipsource_with.

### requirement-d11b3310d27d0350

- Kind: requirement
- Implementation slice: `implementation-slice-1687c4455b65f8a6`
- Candidate: `doc-ff0fb2b094c8ba65`
- Source citation: `docs/modules/opentake-motion/OVERVIEW.md:127`
- Exact files/symbols: `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer::render`, `crates/opentake-motion/src/integration.rs#MotionClipSource`, `docs/modules/opentake-motion/OVERVIEW.md`
- Target resolution: `reviewed-mapping-report:MR-native-chromium`; matched `HeadlessChromiumRenderer::render`, `MotionClipSource`.
- Resolution rationale: The tracked renderer is a fail-closed skeleton and explicitly reports that live Chromium is not implemented.
- Test ownership:
  - `crates/opentake-motion/src/renderer.rs#chromium_skeleton_reports_unavailable_not_panic` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/tests/chromium.rs#virtual_time_network_csp_timeout_cleanup_and_frame_identity` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Provide the deferred native Headless Chromium renderer with deterministic virtual time and fail-closed network/CSP/timeout controls.
- Acceptance criteria: When the chromium feature is enabled, locate/launch a supported browser and render every requested frame instead of returning RendererUnavailable. Enforce request interception allowlists, CSP, document limits, cancellation, timeout, deterministic clock, and no ambient filesystem/network access. Integration tests render a fixed animation twice byte-identically and cover blocked network, timeout, crash, malformed source, and cancellation.

### requirement-4e59a00921e5e252

- Kind: requirement
- Implementation slice: `implementation-slice-ee5b0fb9f6f3c487`
- Candidate: `doc-c837d207c03a37cb`
- Source citation: `docs/modules/opentake-motion/integration.md:26`
- Exact files/symbols: `crates/opentake-motion/src/integration.rs#MotionClipSource::new`, `docs/modules/opentake-motion/integration.md`
- Target resolution: `reviewed-mapping-report:MR-motion-decoder-injection-complete`; matched `MotionClipSource::new`.
- Resolution rationale: Frame decoding is injected through a closure and the production crate does not own an image decoder dependency.
- Test ownership:
  - `crates/opentake-motion/src/integration.rs#decoded_frame_returns_rgba_of_right_shape` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/tests/pipeline.rs#full_pipeline_render_cache_and_ingest` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-motion/integration.md:26 under “解码器注入（为什么不硬接 PNG 库）” (gap-marker), the source “所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。” requires this exact behavior: Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion.
- Acceptance criteria: Source binding: docs/modules/opentake-motion/integration.md:26; signal=gap-marker; heading=解码器注入（为什么不硬接 PNG 库）; candidate=所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。 Expected behavior: Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion. This closes only the promise expressed by “所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。” in “解码器注入（为什么不硬接 PNG 库）”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。” with the scenario below and register test:crates/opentake-project/tests/completion_c837d207c03a37cb.rs#completion_c837d207c03a37cb_inject_frame_decoding_into_motionclipsource_with Initial state/input/event: create an isolated temporary project fixture representing the source state in “所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Inject frame decoding into MotionClipSource without adding a production image decoder dependency to opentake-motion.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_c837d207c03a37cb.rs#completion_c837d207c03a37cb_inject_frame_decoding_into_motionclipsource_with.

### requirement-758f9639111ca4be

- Kind: requirement
- Implementation slice: `implementation-slice-f16a70f238444e28`
- Candidate: `doc-379e138c3e674f1b`
- Source citation: `docs/modules/opentake-motion/integration.md:32`
- Exact files/symbols: `crates/opentake-motion/src/integration.rs#MotionClipSource::frame`, `docs/modules/opentake-motion/integration.md`
- Target resolution: `reviewed-mapping-report:MR-motion-missing-frame-complete`; matched `MotionClipSource::frame`.
- Resolution rationale: Missing or corrupt decoder output is represented as an absent source frame.
- Test ownership:
  - `crates/opentake-motion/src/integration.rs#missing_decoder_result_is_none` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-motion/integration.md:32 under “解码器注入（为什么不硬接 PNG 库）” (gap-marker), the source “解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。” requires this exact behavior: Treat a missing/corrupt decoded motion frame as an absent source frame.
- Acceptance criteria: Source binding: docs/modules/opentake-motion/integration.md:32; signal=gap-marker; heading=解码器注入（为什么不硬接 PNG 库）; candidate=解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。 Expected behavior: Treat a missing/corrupt decoded motion frame as an absent source frame. This closes only the promise expressed by “解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。” in “解码器注入（为什么不硬接 PNG 库）”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。” with the scenario below and register test:crates/opentake-project/tests/completion_379e138c3e674f1b.rs#completion_379e138c3e674f1b_treat_a_missing_corrupt_decoded_motion_frame_as_ Initial state/input/event: start from the smallest valid fixture for “Treat a missing/corrupt decoded motion frame as an absent source frame.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。”. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Treat a missing/corrupt decoded motion frame as an absent source frame.” once. Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_379e138c3e674f1b.rs#completion_379e138c3e674f1b_treat_a_missing_corrupt_decoded_motion_frame_as_.

### requirement-d6ca662045d019df

- Kind: requirement
- Implementation slice: `implementation-slice-70b5cbcce858ecde`
- Candidate: `doc-d7ca9a44e2f69fde`
- Source citation: `docs/modules/opentake-motion/renderer.md:47`
- Exact files/symbols: `crates/opentake-motion/src/renderer.rs#StubRenderer::render`, `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer::render`, `crates/opentake-motion/src/sandbox.rs#SandboxPolicy::check_document_size`, `docs/modules/opentake-motion/renderer.md`
- Target resolution: `reviewed-mapping-report:MR-motion-sandbox-complete`; matched `StubRenderer::render`, `HeadlessChromiumRenderer::render`, `SandboxPolicy::check_document_size`.
- Resolution rationale: Both stub and unavailable Chromium paths enforce document size before rendering or returning unavailable.
- Test ownership:
  - `crates/opentake-motion/src/sandbox.rs#document_size_ceiling_enforced` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/src/renderer.rs#chromium_applies_sandbox_size_before_unavailable` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-motion/renderer.md:47 under “`StubRenderer`（已实现）” (gap-marker), the source “- 即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。” requires this exact behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.
- Acceptance criteria: Source binding: docs/modules/opentake-motion/renderer.md:47; signal=gap-marker; heading=`StubRenderer`（已实现）; candidate=- 即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。 Expected behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path. This closes only the promise expressed by “即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。” in “`StubRenderer`（已实现）”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。” with the scenario below and register test:crates/opentake-project/tests/completion_d7ca9a44e2f69fde.rs#completion_d7ca9a44e2f69fde_apply_sandbox_document_size_checks_before_both_s Initial state/input/event: create an isolated temporary project fixture representing the source state in “即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_d7ca9a44e2f69fde.rs#completion_d7ca9a44e2f69fde_apply_sandbox_document_size_checks_before_both_s.

### requirement-4ed41de96871fedf

- Kind: requirement
- Implementation slice: `implementation-slice-329d3a7fb3b066f7`
- Candidate: `doc-2071153f8053805d`
- Source citation: `docs/modules/opentake-motion/renderer.md:51`
- Exact files/symbols: `crates/opentake-motion/src/renderer.rs#encode_solid_rgba_png`, `docs/modules/opentake-motion/renderer.md`
- Target resolution: `reviewed-mapping-report:MR-motion-png-complete`; matched `encode_solid_rgba_png`.
- Resolution rationale: The dependency-free PNG encoder is deterministic and tested for dimensions and alpha.
- Test ownership:
  - `crates/opentake-motion/src/renderer.rs#stub_output_is_deterministic` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/src/renderer.rs#stub_png_decodes_with_correct_dimensions_and_alpha` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-motion/renderer.md:51 under “自制 PNG 编码器（无依赖）” (gap-marker), the source “lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：” requires this exact behavior: Emit deterministic RGBA PNG frames from the stub renderer without a production image dependency.
- Acceptance criteria: Source binding: docs/modules/opentake-motion/renderer.md:51; signal=gap-marker; heading=自制 PNG 编码器（无依赖）; candidate=lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用： Expected behavior: Emit deterministic RGBA PNG frames from the stub renderer without a production image dependency. This closes only the promise expressed by “lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：” in “自制 PNG 编码器（无依赖）”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：” with the scenario below and register test:crates/opentake-project/tests/completion_2071153f8053805d.rs#completion_2071153f8053805d_emit_deterministic_rgba_png_frames_from_the_stub Initial state/input/event: create an isolated temporary project fixture representing the source state in “lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Emit deterministic RGBA PNG frames from the stub renderer without a production image dependency.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_2071153f8053805d.rs#completion_2071153f8053805d_emit_deterministic_rgba_png_frames_from_the_stub.

### requirement-1f1781dbcd30d20c

- Kind: requirement
- Implementation slice: `implementation-slice-70b5cbcce858ecde`
- Candidate: `doc-d4dc621dbf47ec34`
- Source citation: `docs/modules/opentake-motion/renderer.md:91`
- Exact files/symbols: `crates/opentake-motion/src/renderer.rs#StubRenderer::render`, `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer::render`, `crates/opentake-motion/src/sandbox.rs#SandboxPolicy::check_document_size`, `docs/modules/opentake-motion/renderer.md`
- Target resolution: `reviewed-mapping-report:MR-motion-sandbox-complete`; matched `StubRenderer::render`, `HeadlessChromiumRenderer::render`, `SandboxPolicy::check_document_size`.
- Resolution rationale: Both stub and unavailable Chromium paths enforce document size before rendering or returning unavailable.
- Test ownership:
  - `crates/opentake-motion/src/sandbox.rs#document_size_ceiling_enforced` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/src/renderer.rs#chromium_applies_sandbox_size_before_unavailable` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-motion/renderer.md:91 under “移植铁律落地” (gap-marker), the source “- **沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。” requires this exact behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.
- Acceptance criteria: Source binding: docs/modules/opentake-motion/renderer.md:91; signal=gap-marker; heading=移植铁律落地; candidate=- **沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。 Expected behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path. This closes only the promise expressed by “**沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。” in “移植铁律落地”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “**沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。” with the scenario below and register test:crates/opentake-project/tests/completion_d4dc621dbf47ec34.rs#completion_d4dc621dbf47ec34_apply_sandbox_document_size_checks_before_both_s Initial state/input/event: create an isolated temporary project fixture representing the source state in “**沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_d4dc621dbf47ec34.rs#completion_d4dc621dbf47ec34_apply_sandbox_document_size_checks_before_both_s.

### requirement-69723d1c0bd77577

- Kind: requirement
- Implementation slice: `implementation-slice-70b5cbcce858ecde`
- Candidate: `doc-3a71829a7b489aae`
- Source citation: `docs/modules/opentake-motion/sandbox.md:67`
- Exact files/symbols: `crates/opentake-motion/src/renderer.rs#StubRenderer::render`, `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer::render`, `crates/opentake-motion/src/sandbox.rs#SandboxPolicy::check_document_size`, `docs/modules/opentake-motion/sandbox.md`
- Target resolution: `reviewed-mapping-report:MR-motion-sandbox-complete`; matched `StubRenderer::render`, `HeadlessChromiumRenderer::render`, `SandboxPolicy::check_document_size`.
- Resolution rationale: Both stub and unavailable Chromium paths enforce document size before rendering or returning unavailable.
- Test ownership:
  - `crates/opentake-motion/src/sandbox.rs#document_size_ceiling_enforced` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/src/renderer.rs#chromium_applies_sandbox_size_before_unavailable` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-motion/sandbox.md:67 under “谁在调用” (gap-marker), the source “- `StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](../../../modules/opentake-motion/renderer.md)）。” requires this exact behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.
- Acceptance criteria: Source binding: docs/modules/opentake-motion/sandbox.md:67; signal=gap-marker; heading=谁在调用; candidate=- `StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](../../../modules/opentake-motion/renderer.md)）。 Expected behavior: Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path. This closes only the promise expressed by “`StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](../../../modules/opentake-motion/renderer.md)）。” in “谁在调用”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “`StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](../../../modules/opentake-motion/renderer.md)）。” with the scenario below and register test:crates/opentake-project/tests/completion_3a71829a7b489aae.rs#completion_3a71829a7b489aae_apply_sandbox_document_size_checks_before_both_s Initial state/input/event: create an isolated temporary project fixture representing the source state in “`StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](../../../modules/opentake-motion/renderer.md)）。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “Apply sandbox document-size checks before both stub rendering and the unavailable Chromium path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_3a71829a7b489aae.rs#completion_3a71829a7b489aae_apply_sandbox_document_size_checks_before_both_s.

### requirement-3c0728a9184da517

- Kind: requirement
- Implementation slice: `implementation-slice-9c460dd3f289f9bd`
- Candidate: `doc-f2992815147bed14`
- Source citation: `docs/modules/opentake-project/OVERVIEW.md:110`
- Exact files/symbols: `crates/opentake-project/src/bundle.rs#Project::open_from_root`, `crates/opentake-domain/src/media.rs#MediaManifest::deserialize`, `docs/modules/opentake-project/OVERVIEW.md`
- Target resolution: `reviewed-mapping-report:MR-project-serde-complete`; matched `Project::open_from_root`, `MediaManifest::deserialize`.
- Resolution rationale: The narrower optional-field and explicit-version migration contract has direct project and domain tests, though broader exhaustive migration remains in data-safety.
- Test ownership:
  - `crates/opentake-project/tests/upstream_compat.rs#applies_clip_defaults_for_omitted_fields` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/upstream_compat.rs#migrates_generation_log_legacy_cost_and_version` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-project/OVERVIEW.md:110 under “移植铁律（本模块重点）” (gap-marker), the source “1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。” requires this exact behavior: Decode persisted project/domain models compatibly, defaulting absent optional fields and preserving explicit version migrations.
- Acceptance criteria: Source binding: docs/modules/opentake-project/OVERVIEW.md:110; signal=gap-marker; heading=移植铁律（本模块重点）; candidate=1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。 Expected behavior: Decode persisted project/domain models compatibly, defaulting absent optional fields and preserving explicit version migrations. This closes only the promise expressed by “1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。” in “移植铁律（本模块重点）”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。” with the scenario below and register test:crates/opentake-agent/tests/completion_f2992815147bed14.rs#completion_f2992815147bed14_decode_persisted_project_domain_models_compatibl Initial state/input/event: create an isolated temporary project fixture representing the source state in “1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Decode persisted project/domain models compatibly, defaulting absent optional fields and preserving explicit version migrations.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_f2992815147bed14.rs#completion_f2992815147bed14_decode_persisted_project_domain_models_compatibl.

### requirement-157653b425a21810

- Kind: requirement
- Implementation slice: `implementation-slice-35e4fa716888cc28`
- Candidate: `doc-7ffe9312478aa940`
- Source citation: `docs/modules/opentake-render/OVERVIEW.md:102`
- Exact files/symbols: `crates/opentake-render/src/gpu/compositor.rs`, `docs/modules/opentake-render/OVERVIEW.md`
- Target resolution: `reviewed-mapping-report:MR-mask-effect-mixed-duplicate`; matched the exact process/control contract.
- Resolution rationale: This record mixes polygon masks and generic effects, which are already represented by separate mask and effect slices.
- Test ownership:
  - `crates/opentake-render/tests/composite_acceptance.rs#mask_and_effect_records_have_separate_child_owners` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Render polygon masks and the generic Effect chain instead of carrying them as no-op/pass-through metadata.
- Acceptance criteria: Encode polygon points in a bounded GPU storage representation with deterministic overflow behavior and match domain SDF/feather/invert semantics. Implement an explicit registry/pass pipeline for every shipped Effect name; reject unsupported names at the command boundary rather than silently passing metadata. GPU/pixel-diff tests cover polygon inside/outside/edge/feather/invert, multiple masks, effect order, disabled effects, preview/export parity, and headless skip semantics.

### requirement-c74866cce1e8dd58

- Kind: requirement
- Implementation slice: `implementation-slice-f92ff19f85ab4082`
- Candidate: `doc-734ee146d5000580`
- Source citation: `docs/modules/opentake-render/text-rasterizer.md:44`
- Exact files/symbols: `crates/opentake-render/src/gpu/text_engine.rs#CosmicTextRasterizer`, `crates/opentake-render/src/plan/types.rs#TextureSource::Text`, `docs/modules/opentake-render/text-rasterizer.md`
- Target resolution: `reviewed-mapping-report:MR-text-parity`; matched `CosmicTextRasterizer`, `TextureSource::Text`.
- Resolution rationale: Substantial text rendering and structural tests exist, while the module document still lists fallback and layout parity work.
- Test ownership:
  - `crates/opentake-render/tests/gpu_text.rs#rasterize_is_deterministic_ssim_one` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/tests/gpu_text.rs#natural_size_shadow_padding_matches_upstream` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-render/tests/gpu_text.rs#fallback_font_no_font_scaled_stroke_and_structural_golden_matrix` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Close text raster parity with deterministic structural image comparisons and the remaining fallback/layout details.
- Acceptance criteria: Add a pinned upstream comparison fixture for wrapping, fallback fonts, shadow padding, stroke width, size, and alignment across Chinese/Latin text. Pass deterministic structural/pixel thresholds on macOS and the headless fallback while preserving non-crashing no-font behavior.

### requirement-c3edeb41fee8e2a1

- Kind: requirement
- Implementation slice: `implementation-slice-c35c845cbc3492dd`
- Candidate: `doc-d0dbfb003dbb6afe`
- Source citation: `docs/specs/core/3-event-bus.md:1`
- Exact files/symbols: `crates/opentake-core/src/events.rs#EventBus`, `src-tauri/src/lib.rs#forward_event`, `docs/specs/core/3-event-bus.md`
- Target resolution: `reviewed-mapping-report:MR-event-bus-complete`; matched `EventBus`, `forward_event`.
- Resolution rationale: Typed state events cross the Tauri bridge and raw frame pixels remain on the separate playback transport.
- Test ownership:
  - `crates/opentake-core/src/events.rs#core_event_serializes_with_kind_tag` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/events.rs#media_changed_serializes_with_kind_tag` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/3-event-bus.md:1 under “3. 事件总线” (heading), the source “## 3. 事件总线” requires this exact behavior: Core events cross the Tauri bridge without carrying raw frame pixels through the state event bus.
- Acceptance criteria: Source binding: docs/specs/core/3-event-bus.md:1; signal=heading; heading=3. 事件总线; candidate=## 3. 事件总线 Expected behavior: Core events cross the Tauri bridge without carrying raw frame pixels through the state event bus. This closes only the promise expressed by “3. 事件总线” in “3. 事件总线”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “3. 事件总线” with the scenario below and register test:crates/opentake-render/tests/completion_d0dbfb003dbb6afe.rs#completion_d0dbfb003dbb6afe_core_events_cross_the_tauri_bridge_without_carry Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “3. 事件总线”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Core events cross the Tauri bridge without carrying raw frame pixels through the state event bus.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_d0dbfb003dbb6afe.rs#completion_d0dbfb003dbb6afe_core_events_cross_the_tauri_bridge_without_carry.

### requirement-2c6c54de3cd8a488

- Kind: requirement
- Implementation slice: `implementation-slice-c35c845cbc3492dd`
- Candidate: `doc-3285416a6778562c`
- Source citation: `docs/specs/core/3-event-bus.md:3`
- Exact files/symbols: `crates/opentake-core/src/events.rs#EventBus`, `src-tauri/src/lib.rs#forward_event`, `docs/specs/core/3-event-bus.md`
- Target resolution: `reviewed-mapping-report:MR-event-bus-complete`; matched `EventBus`, `forward_event`.
- Resolution rationale: Typed state events cross the Tauri bridge and raw frame pixels remain on the separate playback transport.
- Test ownership:
  - `crates/opentake-core/src/events.rs#core_event_serializes_with_kind_tag` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/events.rs#media_changed_serializes_with_kind_tag` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/3-event-bus.md:3 under “3.1 `EventBus` 与事件类型” (heading), the source “### 3.1 `EventBus` 与事件类型” requires this exact behavior: Core events cross the Tauri bridge without carrying raw frame pixels through the state event bus.
- Acceptance criteria: Source binding: docs/specs/core/3-event-bus.md:3; signal=heading; heading=3.1 `EventBus` 与事件类型; candidate=### 3.1 `EventBus` 与事件类型 Expected behavior: Core events cross the Tauri bridge without carrying raw frame pixels through the state event bus. This closes only the promise expressed by “3.1 `EventBus` 与事件类型” in “3.1 `EventBus` 与事件类型”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “3.1 `EventBus` 与事件类型” with the scenario below and register test:crates/opentake-render/tests/completion_3285416a6778562c.rs#completion_3285416a6778562c_core_events_cross_the_tauri_bridge_without_carry Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “3.1 `EventBus` 与事件类型”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Core events cross the Tauri bridge without carrying raw frame pixels through the state event bus.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_3285416a6778562c.rs#completion_3285416a6778562c_core_events_cross_the_tauri_bridge_without_carry.

### requirement-30dda70b4f22a7d2

- Kind: requirement
- Implementation slice: `implementation-slice-c35c845cbc3492dd`
- Candidate: `doc-57f29052d66fc174`
- Source citation: `docs/specs/core/3-event-bus.md:31`
- Exact files/symbols: `crates/opentake-core/src/events.rs#EventBus`, `src-tauri/src/lib.rs#forward_event`, `docs/specs/core/3-event-bus.md`
- Target resolution: `reviewed-mapping-report:MR-event-bus-complete`; matched `EventBus`, `forward_event`.
- Resolution rationale: Typed state events cross the Tauri bridge and raw frame pixels remain on the separate playback transport.
- Test ownership:
  - `crates/opentake-core/src/events.rs#core_event_serializes_with_kind_tag` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/events.rs#media_changed_serializes_with_kind_tag` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/3-event-bus.md:31 under “3.2 Tauri 桥接(src-tauri,薄)” (heading), the source “### 3.2 Tauri 桥接(src-tauri,薄)” requires this exact behavior: Core events cross the Tauri bridge without carrying raw frame pixels through the state event bus.
- Acceptance criteria: Source binding: docs/specs/core/3-event-bus.md:31; signal=heading; heading=3.2 Tauri 桥接(src-tauri,薄); candidate=### 3.2 Tauri 桥接(src-tauri,薄) Expected behavior: Core events cross the Tauri bridge without carrying raw frame pixels through the state event bus. This closes only the promise expressed by “3.2 Tauri 桥接(src-tauri,薄)” in “3.2 Tauri 桥接(src-tauri,薄)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “3.2 Tauri 桥接(src-tauri,薄)” with the scenario below and register test:crates/opentake-render/tests/completion_57f29052d66fc174.rs#completion_57f29052d66fc174_core_events_cross_the_tauri_bridge_without_carry Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “3.2 Tauri 桥接(src-tauri,薄)”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Core events cross the Tauri bridge without carrying raw frame pixels through the state event bus.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_57f29052d66fc174.rs#completion_57f29052d66fc174_core_events_cross_the_tauri_bridge_without_carry.

### requirement-1a638c73abd21e1e

- Kind: requirement
- Implementation slice: `implementation-slice-726e8186554da9b6`
- Candidate: `doc-3a72ee4de1f4a46f`
- Source citation: `docs/specs/media/0-principles.md:1`
- Exact files/symbols: `docs/specs/media/0-principles.md`
- Target resolution: `reviewed-mapping-report:MR-media-principles-headings`; matched the exact process/control contract.
- Resolution rationale: These are architecture principle and compliance headings; they should become verifier or acceptance collections rather than feature records.
- Test ownership:
  - `crates/opentake-render/tests/composite_acceptance.rs#media_principles_headings_reference_exact_child_capabilities` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/media/0-principles.md:1 under “设计原则与移植铁律(本 crate 必须遵守)” (heading), the source “# 设计原则与移植铁律(本 crate 必须遵守)” requires this exact behavior: Media code follows cross-platform, frame-time, cache, and domain-boundary contracts.
- Acceptance criteria: Source binding: docs/specs/media/0-principles.md:1; signal=heading; heading=设计原则与移植铁律(本 crate 必须遵守); candidate=# 设计原则与移植铁律(本 crate 必须遵守) Expected behavior: Media code follows cross-platform, frame-time, cache, and domain-boundary contracts. This closes only the promise expressed by “设计原则与移植铁律(本 crate 必须遵守)” in “设计原则与移植铁律(本 crate 必须遵守)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “设计原则与移植铁律(本 crate 必须遵守)” with the scenario below and register test:crates/opentake-render/tests/completion_3a72ee4de1f4a46f.rs#completion_3a72ee4de1f4a46f_media_code_follows_cross_platform_frame_time_cac Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “设计原则与移植铁律(本 crate 必须遵守)”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Media code follows cross-platform, frame-time, cache, and domain-boundary contracts.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_3a72ee4de1f4a46f.rs#completion_3a72ee4de1f4a46f_media_code_follows_cross_platform_frame_time_cac.

### requirement-821266d8c284a554

- Kind: requirement
- Implementation slice: `implementation-slice-603290a188109040`
- Candidate: `doc-31663a59f89938e0`
- Source citation: `docs/specs/media/10-acceptance.md:15`
- Exact files/symbols: `src-tauri/src/search.rs#search_index_start`, `src-tauri/src/search.rs#index_assets`, `crates/opentake-media/src/index_coordinator.rs#ExportPause`, `crates/opentake-media/src/ort_worker/mod.rs#OrtModel`, `docs/specs/media/10-acceptance.md`
- Target resolution: `reviewed-mapping-report:MR-bounded-index-runtime`; matched `search_index_start`, `index_assets`, `ExportPause`, `OrtModel`.
- Resolution rationale: The media crate explicitly defers the runtime queue; current Tauri search code does not prove a bounded serialized production coordinator.
- Test ownership:
  - `crates/opentake-media/src/index_coordinator.rs#export_pause_ref_counts` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/search.rs#bounded_single_worker_cancels_skips_stale_and_yields_to_playback_export` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Phase 8 search/transcription worker scheduling runs through a bounded production coordinator.
- Acceptance criteria: Connect IndexCoordinator to the runtime queue rather than deferred orchestration. Serialize heavy inference and yield during export/playback pressure. Add concurrent indexing/transcription/export tests.

### requirement-ff2faf0938e25f39

- Kind: requirement
- Implementation slice: `implementation-slice-ddfcf34d5292a998`
- Candidate: `doc-f06463b86285d17a`
- Source citation: `docs/specs/media/2-ffmpeg.md:1`
- Exact files/symbols: `crates/opentake-media/src/ff.rs#ffmpeg_path`, `crates/opentake-media/src/ff.rs#ffprobe_path`, `src-tauri/tauri.conf.json`, `docs/specs/media/2-ffmpeg.md`
- Target resolution: `reviewed-mapping-report:MR-packaged-ffmpeg`; matched `ffmpeg_path`, `ffprobe_path`.
- Resolution rationale: Runtime path helpers exist, but no verified packaged external-binary configuration or macOS and Windows execution receipt was found.
- Test ownership:
  - `scripts/tests/packaged-sidecars-test.rb#packaged_macos_windows_sidecars_resolve_and_execute` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: FFmpeg is resolved as a verified bundled sidecar in packaged macOS and Windows builds.
- Acceptance criteria: Package target-specific FFmpeg/ffprobe binaries and verify checksum/version. Resolve packaged sidecar paths without relying on developer PATH. Run installed-app probe/decode/encode smoke tests on macOS and Windows.

### requirement-bfc002de0fad03b1

- Kind: requirement
- Implementation slice: `implementation-slice-2edbd096c204bad4`
- Candidate: `doc-c874eae5e1eebd3a`
- Source citation: `docs/specs/media/2-ffmpeg.md:3`
- Exact files/symbols: `crates/opentake-media/src/probe.rs#probe`, `crates/opentake-media/src/decode/frame.rs#decode_frame_at`, `crates/opentake-media/src/decode/pcm.rs#extract_pcm`, `crates/opentake-media/src/encode/mod.rs#VideoEncoder`, `docs/specs/media/2-ffmpeg.md`
- Target resolution: `reviewed-mapping-report:MR-ffmpeg-contract-complete`; matched `probe`, `decode_frame_at`, `extract_pcm`, `VideoEncoder`.
- Resolution rationale: Probe, frame decode, PCM extraction and encoding have tracked end-to-end FFmpeg integration fixtures.
- Test ownership:
  - `crates/opentake-media/tests/ffmpeg_integration.rs#probe_reports_dimensions_fps_and_audio` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#decode_frame_returns_rgba_of_expected_size` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#extract_pcm_yields_16k_mono` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#encode_roundtrip_produces_playable_video` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/media/2-ffmpeg.md:3 under “2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)” (heading), the source “## 2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)” requires this exact behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.
- Acceptance criteria: Source binding: docs/specs/media/2-ffmpeg.md:3; signal=heading; heading=2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支); candidate=## 2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支) Expected behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures. This closes only the promise expressed by “2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)” in “2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)” with the scenario below and register test:crates/opentake-render/tests/completion_c874eae5e1eebd3a.rs#completion_c874eae5e1eebd3a_the_named_ffmpeg_probe_decode_pcm_encode_contrac Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_c874eae5e1eebd3a.rs#completion_c874eae5e1eebd3a_the_named_ffmpeg_probe_decode_pcm_encode_contrac.

### requirement-d35161912b562397

- Kind: requirement
- Implementation slice: `implementation-slice-2edbd096c204bad4`
- Candidate: `doc-e36487856d25a4d1`
- Source citation: `docs/specs/media/2-ffmpeg.md:30`
- Exact files/symbols: `crates/opentake-media/src/probe.rs#probe`, `crates/opentake-media/src/decode/frame.rs#decode_frame_at`, `crates/opentake-media/src/decode/pcm.rs#extract_pcm`, `crates/opentake-media/src/encode/mod.rs#VideoEncoder`, `docs/specs/media/2-ffmpeg.md`
- Target resolution: `reviewed-mapping-report:MR-ffmpeg-contract-complete`; matched `probe`, `decode_frame_at`, `extract_pcm`, `VideoEncoder`.
- Resolution rationale: Probe, frame decode, PCM extraction and encoding have tracked end-to-end FFmpeg integration fixtures.
- Test ownership:
  - `crates/opentake-media/tests/ffmpeg_integration.rs#probe_reports_dimensions_fps_and_audio` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#decode_frame_returns_rgba_of_expected_size` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#extract_pcm_yields_16k_mono` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#encode_roundtrip_produces_playable_video` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/media/2-ffmpeg.md:30 under “2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)” (heading), the source “## 2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)” requires this exact behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.
- Acceptance criteria: Source binding: docs/specs/media/2-ffmpeg.md:30; signal=heading; heading=2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座); candidate=## 2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座) Expected behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures. This closes only the promise expressed by “2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)” in “2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)” with the scenario below and register test:crates/opentake-render/tests/completion_e36487856d25a4d1.rs#completion_e36487856d25a4d1_the_named_ffmpeg_probe_decode_pcm_encode_contrac Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_e36487856d25a4d1.rs#completion_e36487856d25a4d1_the_named_ffmpeg_probe_decode_pcm_encode_contrac.

### requirement-48b0e0781eeb48c1

- Kind: requirement
- Implementation slice: `implementation-slice-2edbd096c204bad4`
- Candidate: `doc-c2c95cd120af9730`
- Source citation: `docs/specs/media/2-ffmpeg.md:60`
- Exact files/symbols: `crates/opentake-media/src/probe.rs#probe`, `crates/opentake-media/src/decode/frame.rs#decode_frame_at`, `crates/opentake-media/src/decode/pcm.rs#extract_pcm`, `crates/opentake-media/src/encode/mod.rs#VideoEncoder`, `docs/specs/media/2-ffmpeg.md`
- Target resolution: `reviewed-mapping-report:MR-ffmpeg-contract-complete`; matched `probe`, `decode_frame_at`, `extract_pcm`, `VideoEncoder`.
- Resolution rationale: Probe, frame decode, PCM extraction and encoding have tracked end-to-end FFmpeg integration fixtures.
- Test ownership:
  - `crates/opentake-media/tests/ffmpeg_integration.rs#probe_reports_dimensions_fps_and_audio` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#decode_frame_returns_rgba_of_expected_size` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#extract_pcm_yields_16k_mono` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#encode_roundtrip_produces_playable_video` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/media/2-ffmpeg.md:60 under “2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)” (heading), the source “## 2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)” requires this exact behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.
- Acceptance criteria: Source binding: docs/specs/media/2-ffmpeg.md:60; signal=heading; heading=2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`); candidate=## 2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`) Expected behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures. This closes only the promise expressed by “2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)” in “2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)” with the scenario below and register test:crates/opentake-render/tests/completion_c2c95cd120af9730.rs#completion_c2c95cd120af9730_the_named_ffmpeg_probe_decode_pcm_encode_contrac Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_c2c95cd120af9730.rs#completion_c2c95cd120af9730_the_named_ffmpeg_probe_decode_pcm_encode_contrac.

### requirement-6b7f592b1be2c09b

- Kind: requirement
- Implementation slice: `implementation-slice-2edbd096c204bad4`
- Candidate: `doc-376576e91988107c`
- Source citation: `docs/specs/media/2-ffmpeg.md:80`
- Exact files/symbols: `crates/opentake-media/src/probe.rs#probe`, `crates/opentake-media/src/decode/frame.rs#decode_frame_at`, `crates/opentake-media/src/decode/pcm.rs#extract_pcm`, `crates/opentake-media/src/encode/mod.rs#VideoEncoder`, `docs/specs/media/2-ffmpeg.md`
- Target resolution: `reviewed-mapping-report:MR-ffmpeg-contract-complete`; matched `probe`, `decode_frame_at`, `extract_pcm`, `VideoEncoder`.
- Resolution rationale: Probe, frame decode, PCM extraction and encoding have tracked end-to-end FFmpeg integration fixtures.
- Test ownership:
  - `crates/opentake-media/tests/ffmpeg_integration.rs#probe_reports_dimensions_fps_and_audio` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#decode_frame_returns_rgba_of_expected_size` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#extract_pcm_yields_16k_mono` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/ffmpeg_integration.rs#encode_roundtrip_produces_playable_video` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/media/2-ffmpeg.md:80 under “2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)” (heading), the source “## 2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)” requires this exact behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.
- Acceptance criteria: Source binding: docs/specs/media/2-ffmpeg.md:80; signal=heading; heading=2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用); candidate=## 2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用) Expected behavior: The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures. This closes only the promise expressed by “2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)” in “2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)” with the scenario below and register test:crates/opentake-project/tests/completion_376576e91988107c.rs#completion_376576e91988107c_the_named_ffmpeg_probe_decode_pcm_encode_contrac Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “The named FFmpeg probe/decode/PCM/encode contract is implemented with integration fixtures.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_376576e91988107c.rs#completion_376576e91988107c_the_named_ffmpeg_probe_decode_pcm_encode_contrac.

### requirement-c3118cc0b3ca139f

- Kind: requirement
- Implementation slice: `implementation-slice-603290a188109040`
- Candidate: `doc-53d7eb7f42379bad`
- Source citation: `docs/specs/media/7-ort-worker.md:1`
- Exact files/symbols: `src-tauri/src/search.rs#search_index_start`, `src-tauri/src/search.rs#index_assets`, `crates/opentake-media/src/index_coordinator.rs#ExportPause`, `crates/opentake-media/src/ort_worker/mod.rs#OrtModel`, `docs/specs/media/7-ort-worker.md`
- Target resolution: `reviewed-mapping-report:MR-bounded-index-runtime`; matched `search_index_start`, `index_assets`, `ExportPause`, `OrtModel`.
- Resolution rationale: The media crate explicitly defers the runtime queue; current Tauri search code does not prove a bounded serialized production coordinator.
- Test ownership:
  - `crates/opentake-media/src/index_coordinator.rs#export_pause_ref_counts` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/search.rs#bounded_single_worker_cancels_skips_stale_and_yields_to_playback_export` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Heavy inference and indexing are serialized, cancellable, and yield to playback/export in the production runtime.
- Acceptance criteria: Run all heavy ORT tasks through one bounded production queue with model identity, priority, cancellation, and typed result/error. Serialize GPU-heavy inference and pause/yield queued indexing/transcription while playback/export holds higher priority, then resume without duplicate work. Stress-test concurrent search/index/transcribe requests, export preemption, cancellation, model failure, shutdown, and restart with bounded worker count and no lost terminal result.

### requirement-37700810e0c01c49

- Kind: requirement
- Implementation slice: `implementation-slice-603290a188109040`
- Candidate: `doc-1643c9517d5fcf4f`
- Source citation: `docs/specs/media/7-ort-worker.md:23`
- Exact files/symbols: `src-tauri/src/search.rs#search_index_start`, `src-tauri/src/search.rs#index_assets`, `crates/opentake-media/src/index_coordinator.rs#ExportPause`, `crates/opentake-media/src/ort_worker/mod.rs#OrtModel`, `docs/specs/media/7-ort-worker.md`
- Target resolution: `reviewed-mapping-report:MR-bounded-index-runtime`; matched `search_index_start`, `index_assets`, `ExportPause`, `OrtModel`.
- Resolution rationale: The media crate explicitly defers the runtime queue; current Tauri search code does not prove a bounded serialized production coordinator.
- Test ownership:
  - `crates/opentake-media/src/index_coordinator.rs#export_pause_ref_counts` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/search.rs#bounded_single_worker_cancels_skips_stale_and_yields_to_playback_export` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Heavy inference and indexing are serialized, cancellable, and yield to playback/export in the production runtime.
- Acceptance criteria: Implement a bounded worker queue that serializes GPU-heavy model sessions and exposes queued/running/cancelled/completed states. Higher-priority playback/export pressure must prevent new heavy jobs and cause cooperative yield at defined batch boundaries without corrupting model/session state. Test FIFO within priority, starvation bound, cancel queued/running, panic/model error recovery, export preemption latency, and clean shutdown with zero active jobs.

### requirement-2305bfdee6a62e76

- Kind: requirement
- Implementation slice: `implementation-slice-603290a188109040`
- Candidate: `doc-f6ed8457d263b44c`
- Source citation: `docs/specs/media/7-ort-worker.md:41`
- Exact files/symbols: `src-tauri/src/search.rs#search_index_start`, `src-tauri/src/search.rs#index_assets`, `crates/opentake-media/src/index_coordinator.rs#ExportPause`, `crates/opentake-media/src/ort_worker/mod.rs#OrtModel`, `docs/specs/media/7-ort-worker.md`
- Target resolution: `reviewed-mapping-report:MR-bounded-index-runtime`; matched `search_index_start`, `index_assets`, `ExportPause`, `OrtModel`.
- Resolution rationale: The media crate explicitly defers the runtime queue; current Tauri search code does not prove a bounded serialized production coordinator.
- Test ownership:
  - `crates/opentake-media/src/index_coordinator.rs#export_pause_ref_counts` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/search.rs#bounded_single_worker_cancels_skips_stale_and_yields_to_playback_export` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Heavy inference and indexing are serialized, cancellable, and yield to playback/export in the production runtime.
- Acceptance criteria: Connect IndexCoordinator to the production worker queue for idempotent media indexing/transcription keyed by source fingerprint and model version. Deduplicate duplicate requests, persist completed state atomically, invalidate changed media/model, and resume interrupted work after restart. Test duplicate enqueue, source change, model upgrade, export pause/resume, cancellation, crash/restart, failure retry, and final index/transcript equality.

### requirement-0fc6018b00cc4a4a

- Kind: requirement
- Implementation slice: `implementation-slice-3adf63f547b1f57b`
- Candidate: `doc-aea782f4b3e1e8c4`
- Source citation: `docs/specs/media/8-coordinator.md:1`
- Exact files/symbols: `crates/opentake-media/src/lib.rs#MediaEngine`, `crates/opentake-media/src/probe.rs`, `crates/opentake-media/src/decode/mod.rs`, `crates/opentake-media/src/encode/mod.rs`, `crates/opentake-media/src/search/mod.rs`, `crates/opentake-media/src/transcribe/mod.rs`, `docs/specs/media/8-coordinator.md`
- Target resolution: `reviewed-mapping-report:MR-media-facade`; matched `MediaEngine`.
- Resolution rationale: MediaEngine exposes several services, while decode and encode remain mostly flat exports and dependency direction lacks a closed contract test.
- Test ownership:
  - `crates/opentake-media/src/lib.rs#crate_public_types_are_reachable` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/facade_contract.rs#all_services_are_reachable_only_through_facade_and_dependencies_stay_acyclic` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/media/8-coordinator.md:1 under “与 domain / render 的接口” (heading), the source “# 与 domain / render 的接口” requires this exact behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.
- Acceptance criteria: Source binding: docs/specs/media/8-coordinator.md:1; signal=heading; heading=与 domain / render 的接口; candidate=# 与 domain / render 的接口 Expected behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies. This closes only the promise expressed by “与 domain / render 的接口” in “与 domain / render 的接口”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “与 domain / render 的接口” with the scenario below and register test:crates/opentake-render/tests/completion_aea782f4b3e1e8c4.rs#completion_aea782f4b3e1e8c4_the_media_facade_exposes_probe_decode_encode_sea Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “与 domain / render 的接口”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_aea782f4b3e1e8c4.rs#completion_aea782f4b3e1e8c4_the_media_facade_exposes_probe_decode_encode_sea.

### requirement-d24e542069e91a59

- Kind: requirement
- Implementation slice: `implementation-slice-3adf63f547b1f57b`
- Candidate: `doc-9056237f261e5966`
- Source citation: `docs/specs/media/8-coordinator.md:3`
- Exact files/symbols: `crates/opentake-media/src/lib.rs#MediaEngine`, `crates/opentake-media/src/probe.rs`, `crates/opentake-media/src/decode/mod.rs`, `crates/opentake-media/src/encode/mod.rs`, `crates/opentake-media/src/search/mod.rs`, `crates/opentake-media/src/transcribe/mod.rs`, `docs/specs/media/8-coordinator.md`
- Target resolution: `reviewed-mapping-report:MR-media-facade`; matched `MediaEngine`.
- Resolution rationale: MediaEngine exposes several services, while decode and encode remain mostly flat exports and dependency direction lacks a closed contract test.
- Test ownership:
  - `crates/opentake-media/src/lib.rs#crate_public_types_are_reachable` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/facade_contract.rs#all_services_are_reachable_only_through_facade_and_dependencies_stay_acyclic` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/media/8-coordinator.md:3 under “8.1 消费 `opentake-domain`(不可改)” (heading), the source “## 8.1 消费 `opentake-domain`(不可改)” requires this exact behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.
- Acceptance criteria: Source binding: docs/specs/media/8-coordinator.md:3; signal=heading; heading=8.1 消费 `opentake-domain`(不可改); candidate=## 8.1 消费 `opentake-domain`(不可改) Expected behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies. This closes only the promise expressed by “8.1 消费 `opentake-domain`(不可改)” in “8.1 消费 `opentake-domain`(不可改)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “8.1 消费 `opentake-domain`(不可改)” with the scenario below and register test:crates/opentake-project/tests/completion_9056237f261e5966.rs#completion_9056237f261e5966_the_media_facade_exposes_probe_decode_encode_sea Initial state/input/event: create an isolated temporary project fixture representing the source state in “8.1 消费 `opentake-domain`(不可改)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_9056237f261e5966.rs#completion_9056237f261e5966_the_media_facade_exposes_probe_decode_encode_sea.

### requirement-fccb7646732532d1

- Kind: requirement
- Implementation slice: `implementation-slice-3adf63f547b1f57b`
- Candidate: `doc-78e63fc5876e6f1a`
- Source citation: `docs/specs/media/8-coordinator.md:13`
- Exact files/symbols: `crates/opentake-media/src/lib.rs#MediaEngine`, `crates/opentake-media/src/probe.rs`, `crates/opentake-media/src/decode/mod.rs`, `crates/opentake-media/src/encode/mod.rs`, `crates/opentake-media/src/search/mod.rs`, `crates/opentake-media/src/transcribe/mod.rs`, `docs/specs/media/8-coordinator.md`
- Target resolution: `reviewed-mapping-report:MR-media-facade`; matched `MediaEngine`.
- Resolution rationale: MediaEngine exposes several services, while decode and encode remain mostly flat exports and dependency direction lacks a closed contract test.
- Test ownership:
  - `crates/opentake-media/src/lib.rs#crate_public_types_are_reachable` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/facade_contract.rs#all_services_are_reachable_only_through_facade_and_dependencies_stay_acyclic` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/media/8-coordinator.md:13 under “8.2 被 `opentake-render` 复用的解码/编码” (heading), the source “## 8.2 被 `opentake-render` 复用的解码/编码” requires this exact behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.
- Acceptance criteria: Source binding: docs/specs/media/8-coordinator.md:13; signal=heading; heading=8.2 被 `opentake-render` 复用的解码/编码; candidate=## 8.2 被 `opentake-render` 复用的解码/编码 Expected behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies. This closes only the promise expressed by “8.2 被 `opentake-render` 复用的解码/编码” in “8.2 被 `opentake-render` 复用的解码/编码”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “8.2 被 `opentake-render` 复用的解码/编码” with the scenario below and register test:crates/opentake-project/tests/completion_78e63fc5876e6f1a.rs#completion_78e63fc5876e6f1a_the_media_facade_exposes_probe_decode_encode_sea Initial state/input/event: create an isolated temporary project fixture representing the source state in “8.2 被 `opentake-render` 复用的解码/编码”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_78e63fc5876e6f1a.rs#completion_78e63fc5876e6f1a_the_media_facade_exposes_probe_decode_encode_sea.

### requirement-db6267b61deb36fe

- Kind: requirement
- Implementation slice: `implementation-slice-a90703f04ca7e8c5`
- Candidate: `doc-93576f776eb389a0`
- Source citation: `docs/specs/media/8-coordinator.md:27`
- Exact files/symbols: `src-tauri/src/render.rs#MediaResolver::resolve`, `src-tauri/src/export.rs#MediaResolver::resolve`, `src-tauri/src/playback/resolver.rs#StreamingResolver::resolve`, `docs/specs/media/8-coordinator.md`
- Target resolution: `reviewed-mapping-report:MR-image-lottie-materialization`; matched `MediaResolver::resolve`, `MediaResolver::resolve`, `StreamingResolver::resolve`.
- Resolution rationale: Image materialization exists, while all three production resolver paths return None for Lottie.
- Test ownership:
  - `src-tauri/src/playback/resolver.rs#lottie_cache_lifecycle_frame_modulo_and_preview_export_parity` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Image and Lottie materialization produce renderable textures with cache/lifecycle ownership.
- Acceptance criteria: Implement image and Lottie materialization in the media/render boundary. Define cache invalidation and device-loss behavior. Add pixel, lifecycle, and export tests.

### requirement-47f02756ed829ddc

- Kind: requirement
- Implementation slice: `implementation-slice-3adf63f547b1f57b`
- Candidate: `doc-94ba8c5254bc852d`
- Source citation: `docs/specs/media/8-coordinator.md:33`
- Exact files/symbols: `crates/opentake-media/src/lib.rs#MediaEngine`, `crates/opentake-media/src/probe.rs`, `crates/opentake-media/src/decode/mod.rs`, `crates/opentake-media/src/encode/mod.rs`, `crates/opentake-media/src/search/mod.rs`, `crates/opentake-media/src/transcribe/mod.rs`, `docs/specs/media/8-coordinator.md`
- Target resolution: `reviewed-mapping-report:MR-media-facade`; matched `MediaEngine`.
- Resolution rationale: MediaEngine exposes several services, while decode and encode remain mostly flat exports and dependency direction lacks a closed contract test.
- Test ownership:
  - `crates/opentake-media/src/lib.rs#crate_public_types_are_reachable` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/facade_contract.rs#all_services_are_reachable_only_through_facade_and_dependencies_stay_acyclic` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/media/8-coordinator.md:33 under “8.4 facade `MediaEngine`(供 `opentake-core` 调用)” (heading), the source “## 8.4 facade `MediaEngine`(供 `opentake-core` 调用)” requires this exact behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.
- Acceptance criteria: Source binding: docs/specs/media/8-coordinator.md:33; signal=heading; heading=8.4 facade `MediaEngine`(供 `opentake-core` 调用); candidate=## 8.4 facade `MediaEngine`(供 `opentake-core` 调用) Expected behavior: The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies. This closes only the promise expressed by “8.4 facade `MediaEngine`(供 `opentake-core` 调用)” in “8.4 facade `MediaEngine`(供 `opentake-core` 调用)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “8.4 facade `MediaEngine`(供 `opentake-core` 调用)” with the scenario below and register test:crates/opentake-project/tests/completion_94ba8c5254bc852d.rs#completion_94ba8c5254bc852d_the_media_facade_exposes_probe_decode_encode_sea Initial state/input/event: create an isolated temporary project fixture representing the source state in “8.4 facade `MediaEngine`(供 `opentake-core` 调用)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, apply “The media facade exposes probe/decode/encode/search/transcribe services to core/render without reversing dependencies.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_94ba8c5254bc852d.rs#completion_94ba8c5254bc852d_the_media_facade_exposes_probe_decode_encode_sea.

### requirement-9c7360588ced0198

- Kind: requirement
- Implementation slice: `implementation-slice-726e8186554da9b6`
- Candidate: `doc-3336fb8bef6f3a49`
- Source citation: `docs/specs/media/9-domain-contract.md:1`
- Exact files/symbols: `docs/specs/media/9-domain-contract.md`
- Target resolution: `reviewed-mapping-report:MR-media-principles-headings`; matched the exact process/control contract.
- Resolution rationale: These are architecture principle and compliance headings; they should become verifier or acceptance collections rather than feature records.
- Test ownership:
  - `crates/opentake-render/tests/composite_acceptance.rs#media_principles_headings_reference_exact_child_capabilities` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: At docs/specs/media/9-domain-contract.md:1 under “跨平台与合规要点” (heading), the source “# 跨平台与合规要点” requires this exact behavior: Media code follows cross-platform, frame-time, cache, and domain-boundary contracts.
- Acceptance criteria: Source binding: docs/specs/media/9-domain-contract.md:1; signal=heading; heading=跨平台与合规要点; candidate=# 跨平台与合规要点 Expected behavior: Media code follows cross-platform, frame-time, cache, and domain-boundary contracts. This closes only the promise expressed by “跨平台与合规要点” in “跨平台与合规要点”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “跨平台与合规要点” with the scenario below and register test:crates/opentake-render/tests/completion_3336fb8bef6f3a49.rs#completion_3336fb8bef6f3a49_media_code_follows_cross_platform_frame_time_cac Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “跨平台与合规要点”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Media code follows cross-platform, frame-time, cache, and domain-boundary contracts.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_3336fb8bef6f3a49.rs#completion_3336fb8bef6f3a49_media_code_follows_cross_platform_frame_time_cac.

### requirement-0c4a155835f2cd05

- Kind: requirement
- Implementation slice: `implementation-slice-efcc28e98cc9b40e`
- Candidate: `doc-9ae7ed1acecc8998`
- Source citation: `docs/需求与问题汇总.md:58`
- Exact files/symbols: `crates/opentake-project/src/fcpxml.rs#export_xmeml`, `crates/opentake-project/src/fcpxml_modern.rs#export_fcpxml`, `crates/opentake-project/src/otio.rs#export_otio`, `crates/opentake-project/src/edl.rs#export_edl`, `src-tauri/src/commands.rs`, `web/src/components/shell/TitleBar.tsx#TitleBar`, `docs/需求与问题汇总.md`
- Target resolution: `reviewed-mapping-report:MR-interchange-export-complete`; matched `export_xmeml`, `export_fcpxml`, `export_otio`, `export_edl`, `TitleBar`.
- Resolution rationale: FCPXML and XMEML, OTIO and EDL all have tracked exporters, Tauri and frontend routing, and format tests.
- Test ownership:
  - `web/src/components/shell/TitleBar.visual.test.ts#offers all four interchange formats with their extensions and commands` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/src/fcpxml_modern_tests.rs#document_has_fcpxml_header_and_version` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/src/otio.rs#top_level_is_timeline_schema_with_stack_tracks` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/src/edl.rs#header_has_title_and_fcm` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/需求与问题汇总.md:58 under “F4. 工程文件互操作（可被剪映/PR 等打开）” (heading), the source “### F4. 工程文件互操作（可被剪映/PR 等打开）” requires this exact behavior: Projects export interoperable FCPXML, OTIO, and EDL representations.
- Acceptance criteria: Source binding: docs/需求与问题汇总.md:58; signal=heading; heading=F4. 工程文件互操作（可被剪映/PR 等打开）; candidate=### F4. 工程文件互操作（可被剪映/PR 等打开） Expected behavior: Projects export interoperable FCPXML, OTIO, and EDL representations. This closes only the promise expressed by “F4. 工程文件互操作（可被剪映/PR 等打开）” in “F4. 工程文件互操作（可被剪映/PR 等打开）”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “F4. 工程文件互操作（可被剪映/PR 等打开）” with the scenario below and register test:crates/opentake-project/tests/completion_9ae7ed1acecc8998.rs#completion_9ae7ed1acecc8998_projects_export_interoperable_fcpxml_otio_and_ed Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “F4. 工程文件互操作（可被剪映/PR 等打开）”, then invoke the named preview, playback, render, or export event. Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Projects export interoperable FCPXML, OTIO, and EDL representations.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media. Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_9ae7ed1acecc8998.rs#completion_9ae7ed1acecc8998_projects_export_interoperable_fcpxml_otio_and_ed.

### control-record-b984d500edfaa1c1

- Kind: control
- Implementation slice: `implementation-slice-5148c914ccdac250`
- Candidate: `control-580ab884755388a9`
- Source citation: `web/src/components/shell/ExportDialog.tsx:359:5`
- Exact files/symbols: `web/src/components/shell/ExportDialog.tsx`, `web/src/components/shell/ExportDialog.tsx#ExportDialog`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-580ab884755388a9 dismiss Export by clicking the backdrop` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: dismiss Export by clicking the backdrop: closes only when not busy
- Acceptance criteria: Candidate: control-580ab884755388a9. Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-580ab884755388a9 dismiss Export by clicking the backdrop. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {() => { if (!busy) setOpen(false); }}","pointer/drag coordinates, button/key, and current owning state"]; handler={() => { if (!busy) setOpen(false); }}. Exact call/state/backend: stateTransition=closes only when not busy; backendTrace=["web/src/components/shell/ExportDialog.tsx:359::candidate handler -> {() => { if (!busy) setOpen(false); }}","actual branch/state -> closes only when not busy","exact call -> closes only when not busy","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ExportDialog.tsx#ExportDialog"]. Visible/accessibility/return path: success=dismiss Export by clicking the backdrop: closes only when not busy; accessibility={"focus":"Backdrop div is pointer-only; dialog has no focus trap/initial focus","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."]. Outcome matrix: {"success":"dismiss Export by clicking the backdrop: closes only when not busy","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:359; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in closes only when not busy; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

### control-record-85b36a2d320f6b33

- Kind: control
- Implementation slice: `implementation-slice-5148c914ccdac250`
- Candidate: `control-6064916ed05a1362`
- Source citation: `web/src/components/shell/ExportDialog.tsx:401:11`
- Exact files/symbols: `web/src/components/shell/ExportDialog.tsx`, `web/src/components/shell/ExportDialog.tsx#ExportDialog`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-6064916ed05a1362 close Export from its header` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: close Export from its header: setExportDialogOpen(false) when not busy
- Acceptance criteria: Candidate: control-6064916ed05a1362. Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-6064916ed05a1362 close Export from its header. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({busy}). Event: inputs=["event/prop handler: {() => setOpen(false)}","click or native keyboard activation plus current owning state"]; handler={() => setOpen(false)}. Exact call/state/backend: stateTransition=setExportDialogOpen(false) when not busy; backendTrace=["web/src/components/shell/ExportDialog.tsx:401::candidate handler -> {() => setOpen(false)}","actual branch/state -> setExportDialogOpen(false) when not busy","exact call -> setExportDialogOpen(false) when not busy","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ExportDialog.tsx#ExportDialog"]. Visible/accessibility/return path: success=close Export from its header: setExportDialogOpen(false) when not busy; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"export.close\")","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."]. Outcome matrix: {"success":"close Export from its header: setExportDialogOpen(false) when not busy","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:401; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {busy}.","cancel":"Cancellation/dismissal follows the exact guard in setExportDialogOpen(false) when not busy; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

### control-record-b3563b35584e0d40

- Kind: control
- Implementation slice: `implementation-slice-5148c914ccdac250`
- Candidate: `control-34646794727cf515`
- Source citation: `web/src/components/shell/ExportDialog.tsx:434:13`
- Exact files/symbols: `web/src/components/shell/ExportDialog.tsx`, `web/src/components/shell/ExportDialog.tsx#ExportDialog`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-34646794727cf515 choose export mode` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: choose export mode: onModeChange clears stale error/missing report
- Acceptance criteria: Candidate: control-34646794727cf515. Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-34646794727cf515 choose export mode. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {(id) => onModeChange(id)}","click or native keyboard activation plus current owning state"]; handler={(id) => onModeChange(id)}. Exact call/state/backend: stateTransition=onModeChange clears stale error/missing report; backendTrace=["web/src/components/shell/ExportDialog.tsx:434::candidate handler -> {(id) => onModeChange(id)}","actual branch/state -> onModeChange clears stale error/missing report","exact call -> onModeChange clears stale error/missing report","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ExportDialog.tsx#ExportDialog"]. Visible/accessibility/return path: success=choose export mode: onModeChange clears stale error/missing report; accessibility={"focus":"Custom Dropdown focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."]. Outcome matrix: {"success":"choose export mode: onModeChange clears stale error/missing report","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:434; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in onModeChange clears stale error/missing report; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

### control-record-328908d28b350e0c

- Kind: control
- Implementation slice: `implementation-slice-5148c914ccdac250`
- Candidate: `control-6846958c0e19c8e9`
- Source citation: `web/src/components/shell/ExportDialog.tsx:446:17`
- Exact files/symbols: `web/src/components/shell/ExportDialog.tsx`, `web/src/components/shell/ExportDialog.tsx#ExportDialog`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-6846958c0e19c8e9 choose export codec` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: choose export codec: setCodec controls extension and export request
- Acceptance criteria: Candidate: control-6846958c0e19c8e9. Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-6846958c0e19c8e9 choose export codec. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {(id) => setCodec(id)}","click or native keyboard activation plus current owning state"]; handler={(id) => setCodec(id)}. Exact call/state/backend: stateTransition=setCodec controls extension and export request; backendTrace=["web/src/components/shell/ExportDialog.tsx:446::candidate handler -> {(id) => setCodec(id)}","actual branch/state -> setCodec controls extension and export request","exact call -> setCodec controls extension and export request","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ExportDialog.tsx#ExportDialog"]. Visible/accessibility/return path: success=choose export codec: setCodec controls extension and export request; accessibility={"focus":"Custom Dropdown focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."]. Outcome matrix: {"success":"choose export codec: setCodec controls extension and export request","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:446; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setCodec controls extension and export request; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

### control-record-612e325a2e35abe9

- Kind: control
- Implementation slice: `implementation-slice-5148c914ccdac250`
- Candidate: `control-30862b5deb972fcd`
- Source citation: `web/src/components/shell/ExportDialog.tsx:462:17`
- Exact files/symbols: `web/src/components/shell/ExportDialog.tsx`, `web/src/components/shell/ExportDialog.tsx#ExportDialog`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-30862b5deb972fcd choose export resolution` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: choose export resolution: setQuality controls export preset
- Acceptance criteria: Candidate: control-30862b5deb972fcd. Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-30862b5deb972fcd choose export resolution. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {(id) => setQuality(id)}","click or native keyboard activation plus current owning state"]; handler={(id) => setQuality(id)}. Exact call/state/backend: stateTransition=setQuality controls export preset; backendTrace=["web/src/components/shell/ExportDialog.tsx:462::candidate handler -> {(id) => setQuality(id)}","actual branch/state -> setQuality controls export preset","exact call -> setQuality controls export preset","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/shell/ExportDialog.tsx#ExportDialog"]. Visible/accessibility/return path: success=choose export resolution: setQuality controls export preset; accessibility={"focus":"Custom Dropdown focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."]. Outcome matrix: {"success":"choose export resolution: setQuality controls export preset","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:462; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setQuality controls export preset; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

### control-record-c6c9e81870a0cc68

- Kind: control
- Implementation slice: `implementation-slice-c6fbd815566d6b64`
- Candidate: `control-af586bdec82ebcc7`
- Source citation: `web/src/components/shell/ExportDialog.tsx:569:11`
- Exact files/symbols: `web/src/components/shell/ExportDialog.tsx`, `web/src/components/shell/ExportDialog.tsx#onCancel`, `web/src/lib/api.ts#cancelExport`, `src-tauri/src/export.rs#cancel_export`, `web/src/components/shell/ExportDialog.tsx#ExportDialog`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-af586bdec82ebcc7 cancel/close Export` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: cancel/close Export: idle closes; active video calls cancelExport(operationId)
- Acceptance criteria: Candidate: control-af586bdec82ebcc7. Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-af586bdec82ebcc7 cancel/close Export. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate.. Event: inputs=["event/prop handler: {onCancel}","click or native keyboard activation plus current owning state"]; handler={onCancel}. Exact call/state/backend: stateTransition=idle closes; active video calls cancelExport(operationId); backendTrace=["web/src/components/shell/ExportDialog.tsx:569::candidate handler -> {onCancel}","actual branch/state -> idle closes; active video calls cancelExport(operationId)","exact call/arguments -> if idle setExportDialogOpen(false); if busy video and activeOperationId exists call cancelExport(activeOperationId)","web/src/components/shell/ExportDialog.tsx::onCancel -> activeOperationId.current -> api.cancelExport(operationId)","web/src/lib/api.ts::cancelExport -> invoke('cancel_export',{operationId})","src-tauri/src/export.rs::cancel_export(operation_id) -> generation-safe cooperative cancellation","code:web/src/components/shell/ExportDialog.tsx#ExportDialog","code:web/src/components/shell/ExportDialog.tsx#onCancel","code:web/src/lib/api.ts#cancelExport","code:src-tauri/src/export.rs#cancel_export"]. Visible/accessibility/return path: success=cancel/close Export: idle closes; active video calls cancelExport(operationId); accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."]. Outcome matrix: {"success":"cancel/close Export: idle closes; active video calls cancelExport(operationId)","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/shell/ExportDialog.tsx:569; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:569; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in idle closes; active video calls cancelExport(operationId); no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in idle closes; active video calls cancelExport(operationId).","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/shell/ExportDialog.tsx:569; the missing DOM test must prove whether it is surfaced or silent."}.

### control-record-f350ffbc0ca5cf8f

- Kind: control
- Implementation slice: `implementation-slice-73d069e581678e52`
- Candidate: `control-543cacc54290eeba`
- Source citation: `web/src/components/shell/ExportDialog.tsx:587:11`
- Exact files/symbols: `web/src/components/shell/ExportDialog.tsx`, `web/src/components/shell/ExportDialog.tsx#onExport`, `web/src/lib/api.ts#getDefaultProjectDir`, `src-tauri/src/commands.rs`, `web/src/lib/api.ts#exportVideo`, `src-tauri/src/export.rs#export_video`, `web/src/components/shell/ExportDialog.tsx#ExportDialog`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/shell/ExportDialog.interaction.test.tsx#control-543cacc54290eeba start video export` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: start video export: save dialog -> busy/progress -> success/cancel/failure -> cleanup
- Acceptance criteria: Candidate: control-543cacc54290eeba. Test: web/src/components/shell/ExportDialog.interaction.test.tsx#control-543cacc54290eeba start video export. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({busy}). Event: inputs=["event/prop handler: {mode === \"bundle\" ? onExportBundle : onExport}","click or native keyboard activation plus current owning state"]; handler={mode === "bundle" ? onExportBundle : onExport}. Exact call/state/backend: stateTransition=save dialog -> busy/progress -> success/cancel/failure -> cleanup; backendTrace=["web/src/components/shell/ExportDialog.tsx:587::candidate handler -> {mode === \"bundle\" ? onExportBundle : onExport}","actual branch/state -> save dialog -> busy/progress -> success/cancel/failure -> cleanup","exact call/arguments -> save exact codec extension (calling getDefaultProjectDir() only when projectPath is null); createExportOperationId('video'); onExportProgress(operationId); exportVideo({outPath,codec,quality},operationId); rendered mode is video so onExportBundle is unreachable","web/src/components/shell/ExportDialog.tsx::onExport -> saveDialog; onExportProgress; exportVideo(req,operationId); toast/error/finally listener cleanup","web/src/lib/api.ts::getDefaultProjectDir -> invoke('get_default_project_dir') via src-tauri/src/commands.rs when projectPath is null","web/src/lib/api.ts::exportVideo -> invoke('export_video',{req,operationId}); cancelExport -> invoke('cancel_export',{operationId})","src-tauri/src/export.rs::export_video(req,operation_id)/cancel_export(operation_id) -> render/ffmpeg export control","code:web/src/components/shell/ExportDialog.tsx#ExportDialog","code:web/src/components/shell/ExportDialog.tsx#onExport","code:web/src/lib/api.ts#getDefaultProjectDir","code:web/src/lib/api.ts#exportVideo","code:src-tauri/src/export.rs#export_video"]. Visible/accessibility/return path: success=start video export: save dialog -> busy/progress -> success/cancel/failure -> cleanup; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Success/cancel closes to the editor; failure keeps the dialog open. Trigger focus restoration is not implemented/tested."]. Outcome matrix: {"success":"start video export: save dialog -> busy/progress -> success/cancel/failure -> cleanup","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/shell/ExportDialog.tsx:587; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/ExportDialog.tsx:587; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {busy}.","cancel":"Cancellation/dismissal follows the exact guard in save dialog -> busy/progress -> success/cancel/failure -> cleanup; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in save dialog -> busy/progress -> success/cancel/failure -> cleanup.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/shell/ExportDialog.tsx:587; the missing DOM test must prove whether it is surfaced or silent."}.

### control-record-07b1607329b8dfad

- Kind: control
- Implementation slice: `implementation-slice-010eea5507e6b9ca`
- Candidate: `control-b0b920085e77d039`
- Source citation: `web/src/components/shell/SaveAsProgress.tsx:42:7`
- Exact files/symbols: `web/src/components/shell/SaveAsProgress.tsx`, `web/src/store/editActions.ts#cancelSaveAsMedia`, `web/src/lib/api.ts#cancelExport`, `src-tauri/src/export.rs#cancel_export`, `web/src/components/shell/SaveAsProgress.tsx#SaveAsProgress`
- Target resolution: `control-acceptance`; matched the exact process/control contract.
- Resolution rationale: The control acceptance contract explicitly names this owning component test runner.
- Test ownership:
  - `web/src/components/shell/SaveAsProgress.interaction.test.tsx#control-b0b920085e77d039 cancel Save Clip as Media` (reviewed-planned): The control acceptance contract explicitly names this owning component test runner.
- Expected behavior: cancel Save Clip as Media: when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast
- Acceptance criteria: Candidate: control-b0b920085e77d039. Test: web/src/components/shell/SaveAsProgress.interaction.test.tsx#control-b0b920085e77d039 cancel Save Clip as Media. Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({!progress.cancellable || progress.cancelling}). Event: inputs=["event/prop handler: {() => void cancelSaveAsMedia()}","click or native keyboard activation plus current owning state"]; handler={() => void cancelSaveAsMedia()}. Exact call/state/backend: stateTransition=when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast; backendTrace=["web/src/components/shell/SaveAsProgress.tsx:42::candidate handler -> {() => void cancelSaveAsMedia()}","actual branch/state -> when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast","exact call/arguments -> cancelSaveAsMedia(): guard current progress, set cancelling=true, call cancelExport(current.operationId); on rejection restore cancelling=false and toast","web/src/store/editActions.ts::cancelSaveAsMedia -> api.cancelExport(current.operationId)","web/src/lib/api.ts::cancelExport -> invoke('cancel_export',{operationId:current.operationId})","src-tauri/src/export.rs::cancel_export(operation_id)","code:web/src/components/shell/SaveAsProgress.tsx#SaveAsProgress","code:web/src/store/editActions.ts#cancelSaveAsMedia","code:web/src/lib/api.ts#cancelExport","code:src-tauri/src/export.rs#cancel_export"]. Visible/accessibility/return path: success=cancel Save Clip as Media: when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The non-modal status stays in the editor and disappears when store progress clears."]. Outcome matrix: {"success":"cancel Save Clip as Media: when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/shell/SaveAsProgress.tsx:42; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/shell/SaveAsProgress.tsx:42; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {!progress.cancellable || progress.cancelling}.","cancel":"Cancellation/dismissal follows the exact guard in when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in when current progress is cancellable and not cancelling, set cancelling=true and call cancelExport(current.operationId); rejection restores cancelling=false and pushes a failure toast.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/shell/SaveAsProgress.tsx:42; the missing DOM test must prove whether it is surfaced or silent."}.
