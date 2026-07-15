# Media Library Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 78 verified incomplete records in the `media-library` gap group.

**Architecture:** Implement 31 primary evidence-bound slices and reference 1 shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.

---

### Task 1: ML-library-workflow-composite (implementation-slice-e92b802219f01917)

**Covered records:**
- `requirement-bf21a3ef0e947dad` (requirement)

**Files:**
- Modify: `web/src/components/media/MediaPanel.tsx#MediaPanel`
- Modify: `web/src/components/media/LibraryView.tsx#LibraryView`
- Modify: `web/src/store/libraryStore.ts`
- Modify: `docs/architecture/HANDOFF-2026-07.md`
- Test (reviewed-planned): `web/src/components/media/MediaLibrary.workflow.test.tsx#library_workflow_children_close_one_composite_acceptance`

**Candidate-bound contracts:**

#### requirement-bf21a3ef0e947dad

- Candidate/source: `doc-5a7db277482cbc17` at `docs/architecture/HANDOFF-2026-07.md:126` (requirement)
- Expected behavior: Media library folder, import, preview, drag, favorite, rename, delete, and persistence workflows reach current parity.
- Resolution: `reviewed-mapping-report:ML-library-workflow-composite` — Core mapping report ML-library-workflow-composite: this umbrella aggregates import, preview, drag, favorite, rename, delete and persistence child slices.
- Exact acceptance contract:
  - Support create/rename/delete/move folders, import files/folders, favorites, search, preview, and direct library-item drag to empty or compatible timeline tracks.
  - Persist stable media/folder IDs and all mutations atomically; reject folder cycles and roll back failed batches without orphaned files or index entries.
  - Run import→folder move→favorite→preview→timeline drop→undo/redo→save/reopen plus missing-file/relink, duplicate import, invalid cycle, and persistence-failure fixtures.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaLibrary.workflow.test.tsx#library_workflow_children_close_one_composite_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaLibrary.workflow.test.tsx -t "library_workflow_children_close_one_composite_acceptance"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaPanel.tsx#MediaPanel`, `web/src/components/media/LibraryView.tsx#LibraryView`, `web/src/store/libraryStore.ts`, `docs/architecture/HANDOFF-2026-07.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaLibrary.workflow.test.tsx -t "library_workflow_children_close_one_composite_acceptance"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 2: ML-manifest-compat-complete (implementation-slice-7c796760c26c993d)

**Covered records:**
- `requirement-6afe067d8c7d1190` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/media.rs#MediaManifest::deserialize`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Test (existing-owned): `crates/opentake-domain/src/media.rs#manifest_missing_version_falls_back_to_one`
- Test (existing-owned): `crates/opentake-domain/src/media.rs#manifest_empty_object_decodes`
- Test (existing-owned): `crates/opentake-project/tests/roundtrip.rs#save_then_open_is_lossless`

**Candidate-bound contracts:**

#### requirement-6afe067d8c7d1190

- Candidate/source: `doc-58785416fc762d24` at `docs/architecture/MODULE-PORT-MAP.md:136` (requirement)
- Expected behavior: At docs/architecture/MODULE-PORT-MAP.md:136 under “Project · `mixed` → **needs-replacement**” (gap-marker), the source “- `MediaManifest` (struct) — 素材清单(序列化为 media.json):version=2、entries[]、folders[]。自定义 init(from:) 对缺失字段降级到 version=1/空数组。” requires this exact behavior: Persist MediaManifest v2 entries/folders while defaulting absent version/arrays for legacy bundles.
- Resolution: `reviewed-mapping-report:ML-manifest-compat-complete` — Core mapping report ML-manifest-compat-complete: legacy version and array defaults plus lossless reopen have direct evidence.
- Exact acceptance contract:
  - Source binding: docs/architecture/MODULE-PORT-MAP.md:136; signal=gap-marker; heading=Project  ·  `mixed` → **needs-replacement**; candidate=- `MediaManifest` (struct) — 素材清单(序列化为 media.json):version=2、entries[]、folders[]。自定义 init(from:) 对缺失字段降级到 version=1/空数组。
  - Expected behavior: Persist MediaManifest v2 entries/folders while defaulting absent version/arrays for legacy bundles. This closes only the promise expressed by “`MediaManifest` (struct) — 素材清单(序列化为 media.json):version=2、entries[]、folders[]。自定义 init(from:) 对缺失字段降级到 version=1/空数组。” in “Project · `mixed` → **needs-replacement**”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “`MediaManifest` (struct) — 素材清单(序列化为 media.json):version=2、entries[]、folders[]。自定义 init(from:) 对缺失字段降级到 version=1/空数组。” with the scenario below and register test:crates/opentake-project/tests/completion_58785416fc762d24.rs#completion_58785416fc762d24_persist_mediamanifest_v2_entries_folders_while_d
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “`MediaManifest` (struct) — 素材清单(序列化为 media.json):version=2、entries[]、folders[]。自定义 init(from:) 对缺失字段降级到 version=1/空数组。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Persist MediaManifest v2 entries/folders while defaulting absent version/arrays for legacy bundles.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_58785416fc762d24.rs#completion_58785416fc762d24_persist_mediamanifest_v2_entries_folders_while_d.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-domain/src/media.rs#manifest_missing_version_falls_back_to_one` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-domain/src/media.rs#manifest_empty_object_decodes` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/roundtrip.rs#save_then_open_is_lossless` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-domain manifest_missing_version_falls_back_to_one`
  - Run: `cargo test -p opentake-domain manifest_empty_object_decodes`
  - Run: `cargo test -p opentake-project --test roundtrip save_then_open_is_lossless -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/media.rs#MediaManifest::deserialize`, `docs/architecture/MODULE-PORT-MAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-domain manifest_missing_version_falls_back_to_one`
  - Run: `cargo test -p opentake-domain manifest_empty_object_decodes`
  - Run: `cargo test -p opentake-project --test roundtrip save_then_open_is_lossless -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 3: ML-card-state-machine (implementation-slice-a634b7042afa9d08)

**Covered records:**
- `requirement-af77196f5a214264` (requirement)
- `requirement-3c7deb61c7ee69cf` (requirement)
- `requirement-a81e230c3a053c6e` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/media.rs#MediaResolver`
- Modify: `src-tauri/src/media.rs#MediaItemDto::from_entry`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaCard`
- Modify: `web/src/store/mediaActions.ts`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Modify: `docs/需求与问题汇总.md`
- Test (reviewed-planned): `src-tauri/src/media.rs#dto_distinguishes_offline_generating_downloading_failed`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.stateMatrix.test.tsx#state_precedence_actions_and_scoped_relink`

**Candidate-bound contracts:**

#### requirement-af77196f5a214264

- Candidate/source: `doc-8388a588a78466f7` at `docs/architecture/MODULE-PORT-MAP.md:640` (requirement)
- Expected behavior: Media cards expose thumbnail/duration/AI/offline/generating/failure states, rename/context actions, and add-to-chat hover.
- Resolution: `reviewed-mapping-report:ML-card-state-machine` — Core mapping report ML-card-state-machine: the DTO and card need one explicit offline, generating, downloading and failed state machine.
- Exact acceptance contract:
  - Implementation: Implement and test every listed state/action with mutually exclusive visual precedence, backend rename/delete/relink contracts, keyboard/context-menu access, and Agent-chat handoff.
  - Add backend and UI integration tests for every named catalog state, hierarchy, action, precedence, missing-file, and failure branch; the affected suites must pass.
  - Exercise import, navigation, restart, relink, generation-state, or Agent handoff paths named by this record and retain exact runtime evidence before reclassification.

#### requirement-3c7deb61c7ee69cf

- Candidate/source: `doc-4bfc3ee943a2032f` at `docs/architecture/MODULE-PORT-MAP.md:683` (requirement)
- Expected behavior: Compute offline state from resolver failures while keeping generating/downloading/failed states distinct and applying exact border precedence.
- Resolution: `reviewed-mapping-report:ML-card-state-machine` — Core mapping report ML-card-state-machine: the DTO and card need one explicit offline, generating, downloading and failed state machine.
- Exact acceptance contract:
  - Implementation: Centralize media-card state precedence, distinguish missing/unprocessable/resolver-missing from generation states, implement swap-hover/selected border rules, and add exhaustive state-matrix visual tests.
  - Add backend and UI integration tests for every named catalog state, hierarchy, action, precedence, missing-file, and failure branch; the affected suites must pass.
  - Exercise import, navigation, restart, relink, generation-state, or Agent handoff paths named by this record and retain exact runtime evidence before reclassification.

#### requirement-a81e230c3a053c6e

- Candidate/source: `doc-01f8588b6bc15285` at `docs/需求与问题汇总.md:33` (requirement)
- Expected behavior: Offline media shows a scoped offline state and recovers after relink without corrupting unrelated clips.
- Resolution: `reviewed-mapping-report:ML-card-state-machine` — Core mapping report ML-card-state-machine: the DTO and card need one explicit offline, generating, downloading and failed state machine.
- Exact acceptance contract:
  - Implement relink/recovery for offline media references.
  - Scope offline rendering to affected assets instead of a global red failure state.
  - Add save/reopen/relink regression tests.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/media.rs#dto_distinguishes_offline_generating_downloading_failed` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `web/src/components/media/MediaPanel.stateMatrix.test.tsx#state_precedence_actions_and_scoped_relink` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri dto_distinguishes_offline_generating_downloading_failed`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.stateMatrix.test.tsx -t "state_precedence_actions_and_scoped_relink"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/media.rs#MediaResolver`, `src-tauri/src/media.rs#MediaItemDto::from_entry`, `web/src/components/media/MediaPanel.tsx#MediaCard`, `web/src/store/mediaActions.ts`, `docs/architecture/MODULE-PORT-MAP.md`, `docs/需求与问题汇总.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri dto_distinguishes_offline_generating_downloading_failed`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.stateMatrix.test.tsx -t "state_precedence_actions_and_scoped_relink"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 4: ML-import-feedback (implementation-slice-92b1ff0841524d7e)

**Covered records:**
- `requirement-cc98eb192fb7a536` (requirement)

**Files:**
- Modify: `src-tauri/src/media.rs#import_media`
- Modify: `src-tauri/src/media.rs#import_folder`
- Modify: `web/src/store/mediaActions.ts`
- Modify: `docs/architecture/PORT-1TO1-GAP.md`
- Test (existing-owned): `src-tauri/src/media.rs#import_media_imports_supported_and_skips_others`
- Test (reviewed-planned): `web/src/store/mediaActions.test.ts#unsupported_skips_and_folder_failure_remain_visible`

**Candidate-bound contracts:**

#### requirement-cc98eb192fb7a536

- Candidate/source: `doc-a7e1ad75e79901bd` at `docs/architecture/PORT-1TO1-GAP.md:42` (requirement)
- Expected behavior: At docs/architecture/PORT-1TO1-GAP.md:42 under “P0-2 双重导入 + 文件夹浏览（剪映式钻取）” (gap-marker), the source “4. **"没反应"兜底**：非 Tauri / dialog 缺失给 toast 或 disabled;不支持扩展回传 skipped 计数并提示(对齐 `mediaPanelToast`);核实 ffmpeg/ffprobe sidecar 就绪。” requires this exact behavior: Report skipped unsupported imports and keep folder-import failures visible.
- Resolution: `reviewed-mapping-report:ML-import-feedback` — Core mapping report ML-import-feedback: supported imports work, but unsupported skips and recursive-folder failures need visible feedback.
- Exact acceptance contract:
  - Source binding: docs/architecture/PORT-1TO1-GAP.md:42; signal=gap-marker; heading=P0-2 双重导入 + 文件夹浏览（剪映式钻取）; candidate=4. **"没反应"兜底**：非 Tauri / dialog 缺失给 toast 或 disabled;不支持扩展回传 skipped 计数并提示(对齐 `mediaPanelToast`);核实 ffmpeg/ffprobe sidecar 就绪。
  - Expected behavior: Report skipped unsupported imports and keep folder-import failures visible. This closes only the promise expressed by “4. **"没反应"兜底**：非 Tauri / dialog 缺失给 toast 或 disabled;不支持扩展回传 skipped 计数并提示(对齐 `mediaPanelToast`);核实 ffmpeg/ffprobe sidecar 就绪。” in “P0-2 双重导入 + 文件夹浏览（剪映式钻取）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “4. **"没反应"兜底**：非 Tauri / dialog 缺失给 toast 或 disabled;不支持扩展回传 skipped 计数并提示(对齐 `mediaPanelToast`);核实 ffmpeg/ffprobe sidecar 就绪。” with the scenario below and register test:web/src/__tests__/completion/doc-a7e1ad75e79901bd.test.ts#completion_a7e1ad75e79901bd_report_skipped_unsupported_imports_and_keep_fold
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “4. **"没反应"兜底**：非 Tauri / dialog 缺失给 toast 或 disabled;不支持扩展回传 skipped 计数并提示(对齐 `mediaPanelToast`);核实 ffmpeg/ffprobe sidecar 就绪。”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Report skipped unsupported imports and keep folder-import failures visible.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-a7e1ad75e79901bd.test.ts#completion_a7e1ad75e79901bd_report_skipped_unsupported_imports_and_keep_fold.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/media.rs#import_media_imports_supported_and_skips_others` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/store/mediaActions.test.ts#unsupported_skips_and_folder_failure_remain_visible` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri import_media_imports_supported_and_skips_others`
  - Run: `pnpm -C web test -- --run src/store/mediaActions.test.ts -t "unsupported_skips_and_folder_failure_remain_visible"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/media.rs#import_media`, `src-tauri/src/media.rs#import_folder`, `web/src/store/mediaActions.ts`, `docs/architecture/PORT-1TO1-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri import_media_imports_supported_and_skips_others`
  - Run: `pnpm -C web test -- --run src/store/mediaActions.test.ts -t "unsupported_skips_and_folder_failure_remain_visible"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 5: ML-thumbnail-pipeline (implementation-slice-b4b3201929284a2f)

**Covered records:**
- `requirement-d553b2fbbc1e1ee4` (requirement)
- `requirement-99cd2a44bcaaec80` (requirement)
- `requirement-7bf2edec161473dc` (requirement)
- `requirement-f4f57526b9da4d45` (requirement)
- `requirement-9b5bee769d832f02` (requirement)
- `requirement-fe0e0332f4cb42d7` (requirement)
- `requirement-9158059d3142515c` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/thumbnail/mod.rs#video_thumbnail_times`
- Modify: `crates/opentake-media/src/thumbnail/mod.rs#video_thumbnails`
- Modify: `crates/opentake-media/src/thumbnail/mod.rs#image_thumbnail`
- Modify: `crates/opentake-media/src/thumbnail/sprite.rs#load_sprite`
- Modify: `src-tauri/src/media.rs#generate_thumbnail`
- Modify: `docs/modules/opentake-media/thumbnail.md`
- Modify: `docs/specs/media/3-thumbnails.md`
- Test (existing-owned): `crates/opentake-media/src/thumbnail/sprite.rs#sprite_roundtrip_preserves_times_and_pixels`
- Test (existing-owned): `crates/opentake-media/src/thumbnail/sprite.rs#load_corrupt_sidecar_is_none`
- Test (reviewed-planned): `crates/opentake-media/src/thumbnail/sprite.rs#load_invalid_zero_or_undersized_geometry_is_none`
- Test (reviewed-planned): `crates/opentake-media/tests/thumbnail_pipeline.rs#video_image_sprite_cache_and_bounded_concurrency_roundtrip`

**Candidate-bound contracts:**

#### requirement-d553b2fbbc1e1ee4

- Candidate/source: `doc-9b573095eff9f032` at `docs/modules/opentake-media/thumbnail.md:76` (requirement)
- Expected behavior: At docs/modules/opentake-media/thumbnail.md:76 under “落盘 / 读取” (gap-marker), the source “- `load_sprite`：先读 JSON（缺失/解析失败 → `None`）→ 校验 meta（尺寸 > 0、times 非空）→ 开 JPG 转 RGBA8 → **尺寸校验** `sprite ≥ (tile_w×cols_used, tile_h×rows)`，否则 `None`（对齐 `:249-250`）→ 逐 tile 裁回 `VideoThumb`。” requires this exact behavior: Load thumbnail sprites fail-closed on missing/corrupt metadata and invalid geometry, otherwise recover every tile.
- Resolution: `reviewed-mapping-report:ML-thumbnail-pipeline` — Core mapping report ML-thumbnail-pipeline: invalid sprite geometry and the service/cache/concurrency path remain unproved.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-media/thumbnail.md:76; signal=gap-marker; heading=落盘 / 读取; candidate=- `load_sprite`：先读 JSON（缺失/解析失败 → `None`）→ 校验 meta（尺寸 > 0、times 非空）→ 开 JPG 转 RGBA8 → **尺寸校验** `sprite ≥ (tile_w×cols_used, tile_h×rows)`，否则 `None`（对齐 `:249-250`）→ 逐 tile 裁回 `VideoThumb`。
  - Expected behavior: Load thumbnail sprites fail-closed on missing/corrupt metadata and invalid geometry, otherwise recover every tile. This closes only the promise expressed by “`load_sprite`：先读 JSON（缺失/解析失败 → `None`）→ 校验 meta（尺寸 > 0、times 非空）→ 开 JPG 转 RGBA8 → **尺寸校验** `sprite ≥ (tile_w×cols_used, tile_h×rows)`，否则 `None`（对齐 `:249-250`）→ 逐 tile 裁回 `VideoThumb`。” in “落盘 / 读取”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “`load_sprite`：先读 JSON（缺失/解析失败 → `None`）→ 校验 meta（尺寸 > 0、times 非空）→ 开 JPG 转 RGBA8 → **尺寸校验** `sprite ≥ (tile_w×cols_used, tile_h×rows)`，否则 `None`（对齐 `:249-250`）→ 逐 tile 裁回 `VideoThumb`。” with the scenario below and register test:crates/opentake-project/tests/completion_9b573095eff9f032.rs#completion_9b573095eff9f032_load_thumbnail_sprites_fail_closed_on_missing_co
  - Initial state/input/event: start from the smallest valid fixture for “Load thumbnail sprites fail-closed on missing/corrupt metadata and invalid geometry, otherwise recover every tile.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “`load_sprite`：先读 JSON（缺失/解析失败 → `None`）→ 校验 meta（尺寸 > 0、times 非空）→ 开 JPG 转 RGBA8 → **尺寸校验** `sprite ≥ (tile_w×cols_used, tile_h×rows)`，否则 `None`（对齐 `:249-250`）→ 逐 tile 裁回 `VideoThumb`。”.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Load thumbnail sprites fail-closed on missing/corrupt metadata and invalid geometry, otherwise recover every tile.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_9b573095eff9f032.rs#completion_9b573095eff9f032_load_thumbnail_sprites_fail_closed_on_missing_co.

#### requirement-99cd2a44bcaaec80

- Candidate/source: `doc-550f4accbf989647` at `docs/specs/media/3-thumbnails.md:1` (requirement)
- Expected behavior: At docs/specs/media/3-thumbnails.md:1 under “缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`)” (heading), the source “# 缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`)” requires this exact behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.
- Resolution: `reviewed-mapping-report:ML-thumbnail-pipeline` — Core mapping report ML-thumbnail-pipeline: invalid sprite geometry and the service/cache/concurrency path remain unproved.
- Exact acceptance contract:
  - Source binding: docs/specs/media/3-thumbnails.md:1; signal=heading; heading=缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`); candidate=# 缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`)
  - Expected behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented. This closes only the promise expressed by “缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`)” in “缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`)” with the scenario below and register test:tools/completion-tests/doc-550f4accbf989647.test.mjs#completion_550f4accbf989647_thumbnail_sequences_sprite_cache_concurrency_and
  - Initial state/input/event: construct the smallest deterministic state that exposes “缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`)”, apply the precise input or event implied by “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-550f4accbf989647.test.mjs#completion_550f4accbf989647_thumbnail_sequences_sprite_cache_concurrency_and.

#### requirement-7bf2edec161473dc

- Candidate/source: `doc-cb51e0909708397b` at `docs/specs/media/3-thumbnails.md:5` (requirement)
- Expected behavior: At docs/specs/media/3-thumbnails.md:5 under “3.1 视频缩略图序列 + 时间点公式” (heading), the source “## 3.1 视频缩略图序列 + 时间点公式” requires this exact behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.
- Resolution: `reviewed-mapping-report:ML-thumbnail-pipeline` — Core mapping report ML-thumbnail-pipeline: invalid sprite geometry and the service/cache/concurrency path remain unproved.
- Exact acceptance contract:
  - Source binding: docs/specs/media/3-thumbnails.md:5; signal=heading; heading=3.1 视频缩略图序列 + 时间点公式; candidate=## 3.1 视频缩略图序列 + 时间点公式
  - Expected behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented. This closes only the promise expressed by “3.1 视频缩略图序列 + 时间点公式” in “3.1 视频缩略图序列 + 时间点公式”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “3.1 视频缩略图序列 + 时间点公式” with the scenario below and register test:tools/completion-tests/doc-cb51e0909708397b.test.mjs#completion_cb51e0909708397b_thumbnail_sequences_sprite_cache_concurrency_and
  - Initial state/input/event: construct the smallest deterministic state that exposes “3.1 视频缩略图序列 + 时间点公式”, apply the precise input or event implied by “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “3.1 视频缩略图序列 + 时间点公式”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-cb51e0909708397b.test.mjs#completion_cb51e0909708397b_thumbnail_sequences_sprite_cache_concurrency_and.

#### requirement-f4f57526b9da4d45

- Candidate/source: `doc-8d2c8f4a1afc42ea` at `docs/specs/media/3-thumbnails.md:21` (requirement)
- Expected behavior: At docs/specs/media/3-thumbnails.md:21 under “3.2 图片单缩略图” (heading), the source “## 3.2 图片单缩略图” requires this exact behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.
- Resolution: `reviewed-mapping-report:ML-thumbnail-pipeline` — Core mapping report ML-thumbnail-pipeline: invalid sprite geometry and the service/cache/concurrency path remain unproved.
- Exact acceptance contract:
  - Source binding: docs/specs/media/3-thumbnails.md:21; signal=heading; heading=3.2 图片单缩略图; candidate=## 3.2 图片单缩略图
  - Expected behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented. This closes only the promise expressed by “3.2 图片单缩略图” in “3.2 图片单缩略图”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “3.2 图片单缩略图” with the scenario below and register test:tools/completion-tests/doc-8d2c8f4a1afc42ea.test.mjs#completion_8d2c8f4a1afc42ea_thumbnail_sequences_sprite_cache_concurrency_and
  - Initial state/input/event: construct the smallest deterministic state that exposes “3.2 图片单缩略图”, apply the precise input or event implied by “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “3.2 图片单缩略图”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-8d2c8f4a1afc42ea.test.mjs#completion_8d2c8f4a1afc42ea_thumbnail_sequences_sprite_cache_concurrency_and.

#### requirement-9b5bee769d832f02

- Candidate/source: `doc-884a3592f10ababf` at `docs/specs/media/3-thumbnails.md:28` (requirement)
- Expected behavior: At docs/specs/media/3-thumbnails.md:28 under “3.3 sprite 网格磁盘缓存(逐字节复刻)” (heading), the source “## 3.3 sprite 网格磁盘缓存(逐字节复刻)” requires this exact behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.
- Resolution: `reviewed-mapping-report:ML-thumbnail-pipeline` — Core mapping report ML-thumbnail-pipeline: invalid sprite geometry and the service/cache/concurrency path remain unproved.
- Exact acceptance contract:
  - Source binding: docs/specs/media/3-thumbnails.md:28; signal=heading; heading=3.3 sprite 网格磁盘缓存(逐字节复刻); candidate=## 3.3 sprite 网格磁盘缓存(逐字节复刻)
  - Expected behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented. This closes only the promise expressed by “3.3 sprite 网格磁盘缓存(逐字节复刻)” in “3.3 sprite 网格磁盘缓存(逐字节复刻)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “3.3 sprite 网格磁盘缓存(逐字节复刻)” with the scenario below and register test:tools/completion-tests/doc-884a3592f10ababf.test.mjs#completion_884a3592f10ababf_thumbnail_sequences_sprite_cache_concurrency_and
  - Initial state/input/event: construct the smallest deterministic state that exposes “3.3 sprite 网格磁盘缓存(逐字节复刻)”, apply the precise input or event implied by “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “3.3 sprite 网格磁盘缓存(逐字节复刻)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-884a3592f10ababf.test.mjs#completion_884a3592f10ababf_thumbnail_sequences_sprite_cache_concurrency_and.

#### requirement-fe0e0332f4cb42d7

- Candidate/source: `doc-aa4ffec5a58ff1e0` at `docs/specs/media/3-thumbnails.md:52` (requirement)
- Expected behavior: At docs/specs/media/3-thumbnails.md:52 under “3.4 缩略图并发闸门” (heading), the source “## 3.4 缩略图并发闸门” requires this exact behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.
- Resolution: `reviewed-mapping-report:ML-thumbnail-pipeline` — Core mapping report ML-thumbnail-pipeline: invalid sprite geometry and the service/cache/concurrency path remain unproved.
- Exact acceptance contract:
  - Source binding: docs/specs/media/3-thumbnails.md:52; signal=heading; heading=3.4 缩略图并发闸门; candidate=## 3.4 缩略图并发闸门
  - Expected behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented. This closes only the promise expressed by “3.4 缩略图并发闸门” in “3.4 缩略图并发闸门”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “3.4 缩略图并发闸门” with the scenario below and register test:tools/completion-tests/doc-aa4ffec5a58ff1e0.test.mjs#completion_aa4ffec5a58ff1e0_thumbnail_sequences_sprite_cache_concurrency_and
  - Initial state/input/event: construct the smallest deterministic state that exposes “3.4 缩略图并发闸门”, apply the precise input or event implied by “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “3.4 缩略图并发闸门”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-aa4ffec5a58ff1e0.test.mjs#completion_aa4ffec5a58ff1e0_thumbnail_sequences_sprite_cache_concurrency_and.

#### requirement-9158059d3142515c

- Candidate/source: `doc-34af53787857ecb3` at `docs/specs/media/3-thumbnails.md:56` (requirement)
- Expected behavior: At docs/specs/media/3-thumbnails.md:56 under “3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表)” (heading), the source “## 3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表)” requires this exact behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.
- Resolution: `reviewed-mapping-report:ML-thumbnail-pipeline` — Core mapping report ML-thumbnail-pipeline: invalid sprite geometry and the service/cache/concurrency path remain unproved.
- Exact acceptance contract:
  - Source binding: docs/specs/media/3-thumbnails.md:56; signal=heading; heading=3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表); candidate=## 3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表)
  - Expected behavior: Thumbnail sequences, sprite cache, concurrency, and service caching are implemented. This closes only the promise expressed by “3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表)” in “3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表)” with the scenario below and register test:tools/completion-tests/doc-34af53787857ecb3.test.mjs#completion_34af53787857ecb3_thumbnail_sequences_sprite_cache_concurrency_and
  - Initial state/input/event: construct the smallest deterministic state that exposes “3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表)”, apply the precise input or event implied by “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Thumbnail sequences, sprite cache, concurrency, and service caching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-34af53787857ecb3.test.mjs#completion_34af53787857ecb3_thumbnail_sequences_sprite_cache_concurrency_and.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/src/thumbnail/sprite.rs#sprite_roundtrip_preserves_times_and_pixels` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/thumbnail/sprite.rs#load_corrupt_sidecar_is_none` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/thumbnail/sprite.rs#load_invalid_zero_or_undersized_geometry_is_none` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-media/tests/thumbnail_pipeline.rs#video_image_sprite_cache_and_bounded_concurrency_roundtrip` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media sprite_roundtrip_preserves_times_and_pixels`
  - Run: `cargo test -p opentake-media load_corrupt_sidecar_is_none`
  - Run: `cargo test -p opentake-media load_invalid_zero_or_undersized_geometry_is_none`
  - Run: `cargo test -p opentake-media --test thumbnail_pipeline video_image_sprite_cache_and_bounded_concurrency_roundtrip -- --exact`

  Expected: FAIL because one or more of the 7 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/thumbnail/mod.rs#video_thumbnail_times`, `crates/opentake-media/src/thumbnail/mod.rs#video_thumbnails`, `crates/opentake-media/src/thumbnail/mod.rs#image_thumbnail`, `crates/opentake-media/src/thumbnail/sprite.rs#load_sprite`, `src-tauri/src/media.rs#generate_thumbnail`, `docs/modules/opentake-media/thumbnail.md`, `docs/specs/media/3-thumbnails.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media sprite_roundtrip_preserves_times_and_pixels`
  - Run: `cargo test -p opentake-media load_corrupt_sidecar_is_none`
  - Run: `cargo test -p opentake-media load_invalid_zero_or_undersized_geometry_is_none`
  - Run: `cargo test -p opentake-media --test thumbnail_pipeline video_image_sprite_cache_and_bounded_concurrency_roundtrip -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 6: ML-best-effort-probe-complete (implementation-slice-7e04d3acfdfa12ec)

**Covered records:**
- `requirement-022507a3bb431224` (requirement)

**Files:**
- Modify: `src-tauri/src/media.rs#import_one`
- Modify: `src-tauri/src/media.rs#probe_media`
- Modify: `docs/modules/src-tauri/library-media.md`
- Test (existing-owned): `src-tauri/src/media.rs#import_media_imports_supported_and_skips_others`

**Candidate-bound contracts:**

#### requirement-022507a3bb431224

- Candidate/source: `doc-82b4b6aba42b9118` at `docs/modules/src-tauri/library-media.md:18` (requirement)
- Expected behavior: At docs/modules/src-tauri/library-media.md:18 under “A. 媒体导入命令（media.rs）” (gap-marker), the source “对应上游 `addMediaAsset(from:)` → `finalizeImportedAsset`：先按路径建**外部引用**条目（文件不拷进 bundle），再 probe 回填元数据。**probe 是 best-effort**：ffprobe 不可用或文件不可读时，资产仍以零 / 空元数据导入，不让整批失败（缺失 / 离线文件是编辑器已建模的可恢复状态）。” requires this exact behavior: Import a supported file even when metadata probing fails, retaining a recoverable zero/empty metadata entry.
- Resolution: `reviewed-mapping-report:ML-best-effort-probe-complete` — Core mapping report ML-best-effort-probe-complete: supported files remain importable with default metadata after probe failure.
- Exact acceptance contract:
  - Source binding: docs/modules/src-tauri/library-media.md:18; signal=gap-marker; heading=A. 媒体导入命令（media.rs）; candidate=对应上游 `addMediaAsset(from:)` → `finalizeImportedAsset`：先按路径建**外部引用**条目（文件不拷进 bundle），再 probe 回填元数据。**probe 是 best-effort**：ffprobe 不可用或文件不可读时，资产仍以零 / 空元数据导入，不让整批失败（缺失 / 离线文件是编辑器已建模的可恢复状态）。
  - Expected behavior: Import a supported file even when metadata probing fails, retaining a recoverable zero/empty metadata entry. This closes only the promise expressed by “对应上游 `addMediaAsset(from:)` → `finalizeImportedAsset`：先按路径建**外部引用**条目（文件不拷进 bundle），再 probe 回填元数据。**probe 是 best-effort**：ffprobe 不可用或文件不可读时，资产仍以零 / 空元数据导入，不让整批失败（缺失 / 离线文件是编辑器已建模的可恢复状态）。” in “A. 媒体导入命令（media.rs）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “对应上游 `addMediaAsset(from:)` → `finalizeImportedAsset`：先按路径建**外部引用**条目（文件不拷进 bundle），再 probe 回填元数据。**probe 是 best-effort**：ffprobe 不可用或文件不可读时，资产仍以零 / 空元数据导入，不让整批失败（缺失 / 离线文件是编辑器已建模的可恢复状态）。” with the scenario below and register test:crates/opentake-project/tests/completion_82b4b6aba42b9118.rs#completion_82b4b6aba42b9118_import_a_supported_file_even_when_metadata_probi
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “对应上游 `addMediaAsset(from:)` → `finalizeImportedAsset`：先按路径建**外部引用**条目（文件不拷进 bundle），再 probe 回填元数据。**probe 是 best-effort**：ffprobe 不可用或文件不可读时，资产仍以零 / 空元数据导入，不让整批失败（缺失 / 离线文件是编辑器已建模的可恢复状态）。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Import a supported file even when metadata probing fails, retaining a recoverable zero/empty metadata entry.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_82b4b6aba42b9118.rs#completion_82b4b6aba42b9118_import_a_supported_file_even_when_metadata_probi.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/media.rs#import_media_imports_supported_and_skips_others` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri import_media_imports_supported_and_skips_others`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/media.rs#import_one`, `src-tauri/src/media.rs#probe_media`, `docs/modules/src-tauri/library-media.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri import_media_imports_supported_and_skips_others`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 7: ML-relink-in-place-complete (implementation-slice-92e783044be117da)

**Covered records:**
- `requirement-2ea72933632ab3d7` (requirement)

**Files:**
- Modify: `crates/opentake-core/src/session.rs#EditorSession::relink_media_file`
- Modify: `src-tauri/src/media.rs#relink_media`
- Modify: `docs/modules/src-tauri/library-media.md`
- Test (existing-owned): `src-tauri/src/media.rs#relink_keeps_same_id_and_clears_missing`
- Test (existing-owned): `src-tauri/src/media.rs#relink_rejects_type_mismatch`

**Candidate-bound contracts:**

#### requirement-2ea72933632ab3d7

- Candidate/source: `doc-88b94f728982f54b` at `docs/modules/src-tauri/library-media.md:40` (requirement)
- Expected behavior: At docs/modules/src-tauri/library-media.md:40 under “relink（关键修复）” (gap-marker), the source “`relink_media` 把缺失 / 离线资产指向新选文件，**保留同一 asset id**，使每个引用它的 clip 就地恢复。这是「丢失媒体重选路径后仍红」的修复：旧流程只有 `import_media`，会铸**新 id**、把现有 clip 永远晾在缺失条目上。镜像上游 `EditorViewModel.relinkAsset(id:to:)`——新文件类型必须与原一致（否则拒绝），新 probe 元数据刷新条目。命令层先校验类型匹配再触碰目录（给精确报错、省一次无谓 probe）。” requires this exact behavior: Relink an offline media item in place, preserving its id and validating the replacement type before mutation.
- Resolution: `reviewed-mapping-report:ML-relink-in-place-complete` — Core mapping report ML-relink-in-place-complete: relink preserves id, validates type before mutation and clears missing state.
- Exact acceptance contract:
  - Source binding: docs/modules/src-tauri/library-media.md:40; signal=gap-marker; heading=relink（关键修复）; candidate=`relink_media` 把缺失 / 离线资产指向新选文件，**保留同一 asset id**，使每个引用它的 clip 就地恢复。这是「丢失媒体重选路径后仍红」的修复：旧流程只有 `import_media`，会铸**新 id**、把现有 clip 永远晾在缺失条目上。镜像上游 `EditorViewModel.relinkAsset(id:to:)`——新文件类型必须与原一致（否则拒绝），新 probe 元数据刷新条目。命令层先校验类型匹配再触碰目录（给精确报错、省一次无谓 probe）。
  - Expected behavior: Relink an offline media item in place, preserving its id and validating the replacement type before mutation. This closes only the promise expressed by “`relink_media` 把缺失 / 离线资产指向新选文件，**保留同一 asset id**，使每个引用它的 clip 就地恢复。这是「丢失媒体重选路径后仍红」的修复：旧流程只有 `import_media`，会铸**新 id**、把现有 clip 永远晾在缺失条目上。镜像上游 `EditorViewModel.relinkAsset(id:to:)`——新文件类型必须与原一致（否则拒绝），新 probe 元数据刷新条目。命令层先校验类型匹配再触碰目录（给精确报错、省一次无谓 probe）。” in “relink（关键修复）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “`relink_media` 把缺失 / 离线资产指向新选文件，**保留同一 asset id**，使每个引用它的 clip 就地恢复。这是「丢失媒体重选路径后仍红」的修复：旧流程只有 `import_media`，会铸**新 id**、把现有 clip 永远晾在缺失条目上。镜像上游 `EditorViewModel.relinkAsset(id:to:)`——新文件类型必须与原一致（否则拒绝），新 probe 元数据刷新条目。命令层先校验类型匹配再触碰目录（给精确报错、省一次无谓 probe）。” with the scenario below and register test:tools/completion-tests/doc-88b94f728982f54b.test.mjs#completion_88b94f728982f54b_relink_an_offline_media_item_in_place_preserving
  - Initial state/input/event: construct the smallest deterministic state that exposes “`relink_media` 把缺失 / 离线资产指向新选文件，**保留同一 asset id**，使每个引用它的 clip 就地恢复。这是「丢失媒体重选路径后仍红」的修复：旧流程只有 `import_media`，会铸**新 id**、把现有 clip 永远晾在缺失条目上。镜像上游 `EditorViewModel.relinkAsset(id:to:)`——新文件类型必须与原一致（否则拒绝），新 probe 元数据刷新条目。命令层先校验类型匹配再触碰目录（给精确报错、省一次无谓 probe）。”, apply the precise input or event implied by “Relink an offline media item in place, preserving its id and validating the replacement type before mutation.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Relink an offline media item in place, preserving its id and validating the replacement type before mutation.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “`relink_media` 把缺失 / 离线资产指向新选文件，**保留同一 asset id**，使每个引用它的 clip 就地恢复。这是「丢失媒体重选路径后仍红」的修复：旧流程只有 `import_media`，会铸**新 id**、把现有 clip 永远晾在缺失条目上。镜像上游 `EditorViewModel.relinkAsset(id:to:)`——新文件类型必须与原一致（否则拒绝），新 probe 元数据刷新条目。命令层先校验类型匹配再触碰目录（给精确报错、省一次无谓 probe）。”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-88b94f728982f54b.test.mjs#completion_88b94f728982f54b_relink_an_offline_media_item_in_place_preserving.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/media.rs#relink_keeps_same_id_and_clears_missing` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/media.rs#relink_rejects_type_mismatch` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri relink_keeps_same_id_and_clears_missing`
  - Run: `cargo test -p opentake-tauri relink_rejects_type_mismatch`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-core/src/session.rs#EditorSession::relink_media_file`, `src-tauri/src/media.rs#relink_media`, `docs/modules/src-tauri/library-media.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri relink_keeps_same_id_and_clears_missing`
  - Run: `cargo test -p opentake-tauri relink_rejects_type_mismatch`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 8: ML-panel-workflows (implementation-slice-41b839a404501358)

**Covered records:**
- `requirement-3509681bdf85be16` (requirement)
- `requirement-d306c6f423c5a3d0` (requirement)
- `requirement-90c09b1777afb897` (requirement)
- `requirement-ab96875cf2e266c3` (requirement)
- `requirement-b7977faf236f2a6e` (requirement)
- `requirement-e8142ea555c62a5f` (requirement)
- `requirement-f2fac315139c9dce` (requirement)

**Files:**
- Modify: `web/src/components/media/MediaTabBar.tsx#MediaTabBar`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaPanel`
- Modify: `web/src/components/media/LibraryView.tsx#LibraryView`
- Modify: `web/src/components/media/SoundLibraryTab.tsx#SoundLibraryTab`
- Modify: `web/src/store/libraryStore.ts`
- Modify: `src-tauri/src/library.rs`
- Modify: `docs/modules/web/SPEC.md`
- Modify: `docs/specs/frontend/13-implementation.md`
- Modify: `docs/specs/frontend/7-media-panel.md`
- Modify: `docs/需求与问题汇总.md`
- Test (existing-owned): `web/src/components/media/LibraryView.test.tsx#renders global-library entries in the reusable Mine grid`
- Test (reviewed-planned): `web/src/components/media/MediaLibrary.workflow.test.tsx#tab_folder_breadcrumb_preview_drag_favorite_rename_delete_and_music_flow`

**Candidate-bound contracts:**

#### requirement-3509681bdf85be16

- Candidate/source: `doc-0a7c5673da838820` at `docs/modules/web/SPEC.md:1268` (requirement)
- Expected behavior: Match MediaPanel tab rail, grid, hover/selection, and breadcrumb states to upstream.
- Resolution: `reviewed-mapping-report:ML-panel-workflows` — Core mapping report ML-panel-workflows: the rail, folders, preview, drag, favorite, rename, delete and music paths need one workflow acceptance slice.
- Exact acceptance contract:
  - Create a pinned upstream/current screenshot-state matrix at the same viewport, scale, locale, theme, and fixture.
  - Add deterministic component/browser assertions for normal, hover, focus, disabled, selected, empty, error, and compact states.
  - Record reviewed pixel/geometry evidence for every item named by the checklist, with no unchecked item remaining.

#### requirement-d306c6f423c5a3d0

- Candidate/source: `doc-fd849a2120a07f2c` at `docs/specs/frontend/13-implementation.md:25` (requirement)
- Expected behavior: Match MediaPanel rail, grid, and breadcrumbs.
- Resolution: `reviewed-mapping-report:ML-panel-workflows` — Core mapping report ML-panel-workflows: the rail, folders, preview, drag, favorite, rename, delete and music paths need one workflow acceptance slice.
- Exact acceptance contract:
  - Implementation: Verify exact rail capsule/hover states, grid geometry, breadcrumbs, folder navigation and keyboard semantics with visual/interaction tests.
  - Add backend and UI integration tests for every named catalog state, hierarchy, action, precedence, missing-file, and failure branch; the affected suites must pass.
  - Exercise import, navigation, restart, relink, generation-state, or Agent handoff paths named by this record and retain exact runtime evidence before reclassification.

#### requirement-90c09b1777afb897

- Candidate/source: `doc-9d08fa46a592b390` at `docs/specs/frontend/7-media-panel.md:1` (requirement)
- Expected behavior: MediaPanel tab rail and Music workflow are fully interactive and production-backed.
- Resolution: `reviewed-mapping-report:ML-panel-workflows` — Core mapping report ML-panel-workflows: the rail, folders, preview, drag, favorite, rename, delete and music paths need one workflow acceptance slice.
- Exact acceptance contract:
  - Enable Media, Captions, and Music tabs with preserved per-tab navigation/selection and accessible tab semantics.
  - Complete import/folder/favorite/preview/drag, caption edit/export, and music browse/audition/place using live commands and persisted media IDs.
  - Test tab switching, keyboard focus, offline/error/loading states, drag to empty/existing timeline, save/reopen, and packaged visual layouts.

#### requirement-ab96875cf2e266c3

- Candidate/source: `doc-2d70844ce3ae4394` at `docs/specs/frontend/7-media-panel.md:5` (requirement)
- Expected behavior: MediaPanel tab rail and Music workflow are fully interactive and production-backed.
- Resolution: `reviewed-mapping-report:ML-panel-workflows` — Core mapping report ML-panel-workflows: the rail, folders, preview, drag, favorite, rename, delete and music paths need one workflow acceptance slice.
- Exact acceptance contract:
  - Render the outer rail with enabled Media/Captions/Music tabs, correct active/hover/focus states, labels/tooltips, and panel sizing.
  - Switching tabs must preserve or intentionally reset documented selection/navigation without stopping unrelated playback or losing imports.
  - Test keyboard/ARIA tab behavior, narrow/default/wide layouts, theme states, disabled backend errors, and focus restoration.

#### requirement-b7977faf236f2a6e

- Candidate/source: `doc-42fb79b18e00a49b` at `docs/specs/frontend/7-media-panel.md:14` (requirement)
- Expected behavior: At docs/specs/frontend/7-media-panel.md:14 under “7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）” (heading), the source “### 7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）” requires this exact behavior: Media import, browse, folder, favorite, and preview surfaces are implemented.
- Resolution: `reviewed-mapping-report:ML-panel-workflows` — Core mapping report ML-panel-workflows: the rail, folders, preview, drag, favorite, rename, delete and music paths need one workflow acceptance slice.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/7-media-panel.md:14; signal=heading; heading=7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）; candidate=### 7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）
  - Expected behavior: Media import, browse, folder, favorite, and preview surfaces are implemented. This closes only the promise expressed by “7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）” in “7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）” with the scenario below and register test:web/src/__tests__/completion/doc-42fb79b18e00a49b.test.ts#completion_42fb79b18e00a49b_media_import_browse_folder_favorite_and_preview_
  - Initial state/input/event: render the exact “7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “Media import, browse, folder, favorite, and preview surfaces are implemented.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-42fb79b18e00a49b.test.ts#completion_42fb79b18e00a49b_media_import_browse_folder_favorite_and_preview_.

#### requirement-e8142ea555c62a5f

- Candidate/source: `doc-e2e5ee7ae51343d1` at `docs/specs/frontend/7-media-panel.md:41` (requirement)
- Expected behavior: MediaPanel tab rail and Music workflow are fully interactive and production-backed.
- Resolution: `reviewed-mapping-report:ML-panel-workflows` — Core mapping report ML-panel-workflows: the rail, folders, preview, drag, favorite, rename, delete and music paths need one workflow acceptance slice.
- Exact acceptance contract:
  - Implement Music search/browse, loading/error/empty states, audition play/pause, import/favorite, and drag/place to an audio track.
  - Imported music must receive a stable media ID, deduplicate correctly, use one undoable placement command, and persist through save/reopen.
  - Test browse success/failure, rapid audition switching, offline item, duplicate import, empty/existing timeline drop, undo/redo, and A/V preview/export inclusion.

#### requirement-f2fac315139c9dce

- Candidate/source: `doc-16d778915863ca83` at `docs/需求与问题汇总.md:50` (requirement)
- Expected behavior: The top media panel matches the target browse/import/folder/drag workflows.
- Resolution: `reviewed-mapping-report:ML-panel-workflows` — Core mapping report ML-panel-workflows: the rail, folders, preview, drag, favorite, rename, delete and music paths need one workflow acceptance slice.
- Exact acceptance contract:
  - Provide the target top MediaPanel with Media/Captions/Music tabs, folder drill-down, file/folder import, favorites, search, thumbnail/waveform, preview, and direct drag to timeline.
  - Use stable library/media IDs and preserve navigation/selection across tab switches and save/reopen; offline or failed imports must show scoped recovery actions.
  - Test empty/loaded folders, recursive import, duplicate files, favorite/search, source preview, drag to empty/existing tracks, offline/relink, undo/redo, save/reopen, keyboard focus, and fixed-size visual snapshots.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/LibraryView.test.tsx#renders global-library entries in the reusable Mine grid` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/media/MediaLibrary.workflow.test.tsx#tab_folder_breadcrumb_preview_drag_favorite_rename_delete_and_music_flow` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.test.tsx -t "renders global-library entries in the reusable Mine grid"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaLibrary.workflow.test.tsx -t "tab_folder_breadcrumb_preview_drag_favorite_rename_delete_and_music_flow"`

  Expected: FAIL because one or more of the 7 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaTabBar.tsx#MediaTabBar`, `web/src/components/media/MediaPanel.tsx#MediaPanel`, `web/src/components/media/LibraryView.tsx#LibraryView`, `web/src/components/media/SoundLibraryTab.tsx#SoundLibraryTab`, `web/src/store/libraryStore.ts`, `src-tauri/src/library.rs`, `docs/modules/web/SPEC.md`, `docs/specs/frontend/13-implementation.md`, `docs/specs/frontend/7-media-panel.md`, `docs/需求与问题汇总.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.test.tsx -t "renders global-library entries in the reusable Mine grid"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaLibrary.workflow.test.tsx -t "tab_folder_breadcrumb_preview_drag_favorite_rename_delete_and_music_flow"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 9: ML-basic-media-composite (implementation-slice-520acc129ff04913)

**Covered records:**
- `requirement-96ace0bac4ba142d` (requirement)

**Files:**
- Modify: `src-tauri/src/media.rs#generate_thumbnail`
- Modify: `src-tauri/src/media.rs#get_waveform`
- Modify: `crates/opentake-media/src/lib.rs#MediaEngine`
- Modify: `docs/specs/media/10-acceptance.md`
- Test (reviewed-planned): `crates/opentake-media/tests/media_pipeline.rs#ffmpeg_thumbnail_and_waveform_children_close_one_composite_acceptance`

**Candidate-bound contracts:**

#### requirement-96ace0bac4ba142d

- Candidate/source: `doc-fd3270a92e00568b` at `docs/specs/media/10-acceptance.md:5` (requirement)
- Expected behavior: At docs/specs/media/10-acceptance.md:5 under “Phase 2 子集(基础媒体,先做)” (heading), the source “## Phase 2 子集(基础媒体,先做)” requires this exact behavior: Basic probe/decode/encode/thumbnail/waveform media acceptance is implemented.
- Resolution: `reviewed-mapping-report:ML-basic-media-composite` — Core mapping report ML-basic-media-composite: FFmpeg, thumbnail and waveform contracts are child capabilities, not one implementation owner.
- Exact acceptance contract:
  - Source binding: docs/specs/media/10-acceptance.md:5; signal=heading; heading=Phase 2 子集(基础媒体,先做); candidate=## Phase 2 子集(基础媒体,先做)
  - Expected behavior: Basic probe/decode/encode/thumbnail/waveform media acceptance is implemented. This closes only the promise expressed by “Phase 2 子集(基础媒体,先做)” in “Phase 2 子集(基础媒体,先做)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Phase 2 子集(基础媒体,先做)” with the scenario below and register test:tools/completion-tests/doc-fd3270a92e00568b.test.mjs#completion_fd3270a92e00568b_basic_probe_decode_encode_thumbnail_waveform_med
  - Initial state/input/event: construct the smallest deterministic state that exposes “Phase 2 子集(基础媒体,先做)”, apply the precise input or event implied by “Basic probe/decode/encode/thumbnail/waveform media acceptance is implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Basic probe/decode/encode/thumbnail/waveform media acceptance is implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “Phase 2 子集(基础媒体,先做)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-fd3270a92e00568b.test.mjs#completion_fd3270a92e00568b_basic_probe_decode_encode_thumbnail_waveform_med.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/tests/media_pipeline.rs#ffmpeg_thumbnail_and_waveform_children_close_one_composite_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media --test media_pipeline ffmpeg_thumbnail_and_waveform_children_close_one_composite_acceptance -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/media.rs#generate_thumbnail`, `src-tauri/src/media.rs#get_waveform`, `crates/opentake-media/src/lib.rs#MediaEngine`, `docs/specs/media/10-acceptance.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media --test media_pipeline ffmpeg_thumbnail_and_waveform_children_close_one_composite_acceptance -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 10: ML-waveform-pipeline (implementation-slice-a56887e004b9e318)

**Covered records:**
- `requirement-35d9eddb71d90a1e` (requirement)
- `requirement-9a6455e155fbcce4` (requirement)
- `requirement-3712d4cdd2646865` (requirement)
- `requirement-978bde045890440e` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/waveform/mod.rs#waveform_cached_cancellable`
- Modify: `crates/opentake-media/src/waveform/store.rs`
- Modify: `src-tauri/src/media.rs#get_waveform`
- Modify: `web/src/components/media/MediaPanel.tsx#AudioWaveform`
- Modify: `docs/specs/media/4-waveform.md`
- Test (existing-owned): `crates/opentake-media/src/waveform/store.rs#save_then_load_roundtrips`
- Test (existing-owned): `web/src/components/media/MediaPanel.test.tsx#renders waveform bars when normalized buckets are available`
- Test (reviewed-planned): `crates/opentake-media/tests/waveform_pipeline.rs#waveform_sample_count_rms_orientation_and_cache_roundtrip`

**Candidate-bound contracts:**

#### requirement-35d9eddb71d90a1e

- Candidate/source: `doc-21a21e4071ae8465` at `docs/specs/media/4-waveform.md:5` (requirement)
- Expected behavior: At docs/specs/media/4-waveform.md:5 under “4.1 接口” (heading), the source “## 4.1 接口” requires this exact behavior: Waveform sample count, RMS normalization, orientation, and cache format are implemented.
- Resolution: `reviewed-mapping-report:ML-waveform-pipeline` — Core mapping report ML-waveform-pipeline: a deterministic PCM fixture must jointly prove sample count, RMS normalization, orientation and cache behavior, including format/source discrimination between valid all-silent cache data and poisoned legacy output; all == 1.0 alone cannot classify the cache.
- Exact acceptance contract:
  - Source binding: docs/specs/media/4-waveform.md:5; signal=heading; heading=4.1 接口; candidate=## 4.1 接口
  - Expected behavior: Waveform sample count, RMS normalization, orientation, and cache format are implemented. This closes only the promise expressed by “4.1 接口” in “4.1 接口”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “4.1 接口” with the scenario below and register test:tools/completion-tests/doc-21a21e4071ae8465.test.mjs#completion_21a21e4071ae8465_waveform_sample_count_rms_normalization_orientat
  - Initial state/input/event: construct the smallest deterministic state that exposes “4.1 接口”, apply the precise input or event implied by “Waveform sample count, RMS normalization, orientation, and cache format are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Waveform sample count, RMS normalization, orientation, and cache format are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “4.1 接口”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-21a21e4071ae8465.test.mjs#completion_21a21e4071ae8465_waveform_sample_count_rms_normalization_orientat.

#### requirement-9a6455e155fbcce4

- Candidate/source: `doc-f61f8686fd52c4d2` at `docs/specs/media/4-waveform.md:17` (requirement)
- Expected behavior: At docs/specs/media/4-waveform.md:17 under “4.2 样本数量公式(逐字照搬)” (heading), the source “## 4.2 样本数量公式(逐字照搬)” requires this exact behavior: Waveform sample count, RMS normalization, orientation, and cache format are implemented.
- Resolution: `reviewed-mapping-report:ML-waveform-pipeline` — Core mapping report ML-waveform-pipeline: a deterministic PCM fixture must jointly prove sample count, RMS normalization, orientation and cache behavior, including format/source discrimination between valid all-silent cache data and poisoned legacy output; all == 1.0 alone cannot classify the cache.
- Exact acceptance contract:
  - Source binding: docs/specs/media/4-waveform.md:17; signal=heading; heading=4.2 样本数量公式(逐字照搬); candidate=## 4.2 样本数量公式(逐字照搬)
  - Expected behavior: Waveform sample count, RMS normalization, orientation, and cache format are implemented. This closes only the promise expressed by “4.2 样本数量公式(逐字照搬)” in “4.2 样本数量公式(逐字照搬)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “4.2 样本数量公式(逐字照搬)” with the scenario below and register test:tools/completion-tests/doc-f61f8686fd52c4d2.test.mjs#completion_f61f8686fd52c4d2_waveform_sample_count_rms_normalization_orientat
  - Initial state/input/event: construct the smallest deterministic state that exposes “4.2 样本数量公式(逐字照搬)”, apply the precise input or event implied by “Waveform sample count, RMS normalization, orientation, and cache format are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Waveform sample count, RMS normalization, orientation, and cache format are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “4.2 样本数量公式(逐字照搬)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-f61f8686fd52c4d2.test.mjs#completion_f61f8686fd52c4d2_waveform_sample_count_rms_normalization_orientat.

#### requirement-3712d4cdd2646865

- Candidate/source: `doc-bcf993883880f984` at `docs/specs/media/4-waveform.md:27` (requirement)
- Expected behavior: At docs/specs/media/4-waveform.md:27 under “4.3 降采样与归一化(RMS,对齐「0=响,1=静」)” (heading), the source “## 4.3 降采样与归一化(RMS,对齐「0=响,1=静」)” requires this exact behavior: Waveform sample count, RMS normalization, orientation, and cache format are implemented.
- Resolution: `reviewed-mapping-report:ML-waveform-pipeline` — Core mapping report ML-waveform-pipeline: a deterministic PCM fixture must jointly prove sample count, RMS normalization, orientation and cache behavior, including format/source discrimination between valid all-silent cache data and poisoned legacy output; all == 1.0 alone cannot classify the cache.
- Exact acceptance contract:
  - Source binding: docs/specs/media/4-waveform.md:27; signal=heading; heading=4.3 降采样与归一化(RMS,对齐「0=响,1=静」); candidate=## 4.3 降采样与归一化(RMS,对齐「0=响,1=静」)
  - Expected behavior: Waveform sample count, RMS normalization, orientation, and cache format are implemented. This closes only the promise expressed by “4.3 降采样与归一化(RMS,对齐「0=响,1=静」)” in “4.3 降采样与归一化(RMS,对齐「0=响,1=静」)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “4.3 降采样与归一化(RMS,对齐「0=响,1=静」)” with the scenario below and register test:tools/completion-tests/doc-bcf993883880f984.test.mjs#completion_bcf993883880f984_waveform_sample_count_rms_normalization_orientat
  - Initial state/input/event: construct the smallest deterministic state that exposes “4.3 降采样与归一化(RMS,对齐「0=响,1=静」)”, apply the precise input or event implied by “Waveform sample count, RMS normalization, orientation, and cache format are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Waveform sample count, RMS normalization, orientation, and cache format are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “4.3 降采样与归一化(RMS,对齐「0=响,1=静」)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-bcf993883880f984.test.mjs#completion_bcf993883880f984_waveform_sample_count_rms_normalization_orientat.

#### requirement-978bde045890440e

- Candidate/source: `doc-761ea549564923de` at `docs/specs/media/4-waveform.md:36` (requirement)
- Expected behavior: At docs/specs/media/4-waveform.md:36 under “4.4 缓存格式(逐字节复刻)” (heading), the source “## 4.4 缓存格式(逐字节复刻)” requires this exact behavior: Waveform sample count, RMS normalization, orientation, and cache format are implemented.
- Resolution: `reviewed-mapping-report:ML-waveform-pipeline` — Core mapping report ML-waveform-pipeline: a deterministic PCM fixture must jointly prove sample count, RMS normalization, orientation and cache behavior, including format/source discrimination between valid all-silent cache data and poisoned legacy output; all == 1.0 alone cannot classify the cache.
- Exact acceptance contract:
  - Source binding: docs/specs/media/4-waveform.md:36; signal=heading; heading=4.4 缓存格式(逐字节复刻); candidate=## 4.4 缓存格式(逐字节复刻)
  - Expected behavior: Waveform sample count, RMS normalization, orientation, and cache format are implemented. This closes only the promise expressed by “4.4 缓存格式(逐字节复刻)” in “4.4 缓存格式(逐字节复刻)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “4.4 缓存格式(逐字节复刻)” with the scenario below and register test:tools/completion-tests/doc-761ea549564923de.test.mjs#completion_761ea549564923de_waveform_sample_count_rms_normalization_orientat
  - Initial state/input/event: construct the smallest deterministic state that exposes “4.4 缓存格式(逐字节复刻)”, apply the precise input or event implied by “Waveform sample count, RMS normalization, orientation, and cache format are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Waveform sample count, RMS normalization, orientation, and cache format are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “4.4 缓存格式(逐字节复刻)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-761ea549564923de.test.mjs#completion_761ea549564923de_waveform_sample_count_rms_normalization_orientat.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/src/waveform/store.rs#save_then_load_roundtrips` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/media/MediaPanel.test.tsx#renders waveform bars when normalized buckets are available` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/tests/waveform_pipeline.rs#waveform_sample_count_rms_orientation_and_cache_roundtrip` (reviewed-planned) — The pipeline test must distinguish a valid all-silent cache from poisoned legacy output using format/source provenance; an all == 1.0 value check is not sufficient.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media save_then_load_roundtrips`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.test.tsx -t "renders waveform bars when normalized buckets are available"`
  - Run: `cargo test -p opentake-media --test waveform_pipeline waveform_sample_count_rms_orientation_and_cache_roundtrip -- --exact`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/waveform/mod.rs#waveform_cached_cancellable`, `crates/opentake-media/src/waveform/store.rs`, `src-tauri/src/media.rs#get_waveform`, `web/src/components/media/MediaPanel.tsx#AudioWaveform`, `docs/specs/media/4-waveform.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media save_then_load_roundtrips`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.test.tsx -t "renders waveform bars when normalized buckets are available"`
  - Run: `cargo test -p opentake-media --test waveform_pipeline waveform_sample_count_rms_orientation_and_cache_roundtrip -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 11: misgrouped-media-search (implementation-slice-47d76bac3f7f8d4f)

**Covered records:**
- `requirement-cbdc477af446a4ea` (requirement)
- `requirement-45fa0c2b840442cd` (requirement)
- `requirement-83d62f78aa13e484` (requirement)
- `requirement-2f2d03d0f0d8b62a` (requirement)
- `requirement-bd5095a19a955167` (requirement)
- `requirement-43fbc8e32bd126c4` (requirement)
- `requirement-ae88b5f4d3f80eb6` (requirement)
- `requirement-7b2d77c2b48b0238` (requirement)
- `requirement-8d06ffbfdd515f03` (requirement)
- `requirement-980f6325823ecf64` (requirement)
- `requirement-f917123fb8d790f1` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/search/embedder.rs#Embedder`
- Modify: `crates/opentake-media/src/search/tokenizer.rs#SiglipTokenizer`
- Modify: `crates/opentake-media/src/search/indexer.rs#index_video`
- Modify: `crates/opentake-media/src/search/embed_store.rs#AssetIndex`
- Modify: `crates/opentake-media/src/search/ranker.rs#search`
- Modify: `src-tauri/src/search.rs#search_query`
- Modify: `docs/specs/media/5-search.md`
- Test (existing-owned): `crates/opentake-media/src/search/embedder.rs#preprocess_squashes_non_square_to_square`
- Test (existing-owned): `crates/opentake-media/src/search/tokenizer.rs#pads_short_sequence_to_context_length`
- Test (existing-owned): `crates/opentake-media/src/search/embed_store.rs#encode_decode_roundtrip_f16_quantized`
- Test (existing-owned): `crates/opentake-media/src/search/ranker.rs#best_per_shot_dedupes_same_shot`
- Test (existing-owned): `crates/opentake-media/src/search/mod.rs#index_then_rank_finds_brightest_match`

**Candidate-bound contracts:**

#### requirement-cbdc477af446a4ea

- Candidate/source: `doc-82e668279fcb56c8` at `docs/specs/media/5-search.md:1` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:1 under “ort + SigLIP2 + tokenizers 视觉/口语搜索” (heading), the source “# ort + SigLIP2 + tokenizers 视觉/口语搜索” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:1; signal=heading; heading=ort + SigLIP2 + tokenizers 视觉/口语搜索; candidate=# ort + SigLIP2 + tokenizers 视觉/口语搜索
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “ort + SigLIP2 + tokenizers 视觉/口语搜索” in “ort + SigLIP2 + tokenizers 视觉/口语搜索”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “ort + SigLIP2 + tokenizers 视觉/口语搜索” with the scenario below and register test:tools/completion-tests/doc-82e668279fcb56c8.test.mjs#completion_82e668279fcb56c8_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: construct the smallest deterministic state that exposes “ort + SigLIP2 + tokenizers 视觉/口语搜索”, apply the precise input or event implied by “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “ort + SigLIP2 + tokenizers 视觉/口语搜索”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-82e668279fcb56c8.test.mjs#completion_82e668279fcb56c8_visual_text_encoding_sampling_storage_ranking_mo.

#### requirement-45fa0c2b840442cd

- Candidate/source: `doc-af2134f977f96479` at `docs/specs/media/5-search.md:5` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:5 under “5.1 双编码器 trait + Spec” (heading), the source “## 5.1 双编码器 trait + Spec” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:5; signal=heading; heading=5.1 双编码器 trait + Spec; candidate=## 5.1 双编码器 trait + Spec
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “5.1 双编码器 trait + Spec” in “5.1 双编码器 trait + Spec”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.1 双编码器 trait + Spec” with the scenario below and register test:tools/completion-tests/doc-af2134f977f96479.test.mjs#completion_af2134f977f96479_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: construct the smallest deterministic state that exposes “5.1 双编码器 trait + Spec”, apply the precise input or event implied by “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “5.1 双编码器 trait + Spec”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-af2134f977f96479.test.mjs#completion_af2134f977f96479_visual_text_encoding_sampling_storage_ranking_mo.

#### requirement-83d62f78aa13e484

- Candidate/source: `doc-62ed1b93c7082013` at `docs/specs/media/5-search.md:26` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:26 under “5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻” (heading), the source “## 5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:26; signal=heading; heading=5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻; candidate=## 5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻” in “5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻” with the scenario below and register test:tools/completion-tests/doc-62ed1b93c7082013.test.mjs#completion_62ed1b93c7082013_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: construct the smallest deterministic state that exposes “5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻”, apply the precise input or event implied by “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-62ed1b93c7082013.test.mjs#completion_62ed1b93c7082013_visual_text_encoding_sampling_storage_ranking_mo.

#### requirement-2f2d03d0f0d8b62a

- Candidate/source: `doc-e8e85420b5b4510c` at `docs/specs/media/5-search.md:41` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:41 under “5.3 文本 tokenize(SigLIP,定长 64,右填 0)” (heading), the source “## 5.3 文本 tokenize(SigLIP,定长 64,右填 0)” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:41; signal=heading; heading=5.3 文本 tokenize(SigLIP,定长 64,右填 0); candidate=## 5.3 文本 tokenize(SigLIP,定长 64,右填 0)
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “5.3 文本 tokenize(SigLIP,定长 64,右填 0)” in “5.3 文本 tokenize(SigLIP,定长 64,右填 0)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.3 文本 tokenize(SigLIP,定长 64,右填 0)” with the scenario below and register test:tools/completion-tests/doc-e8e85420b5b4510c.test.mjs#completion_e8e85420b5b4510c_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: construct the smallest deterministic state that exposes “5.3 文本 tokenize(SigLIP,定长 64,右填 0)”, apply the precise input or event implied by “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “5.3 文本 tokenize(SigLIP,定长 64,右填 0)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-e8e85420b5b4510c.test.mjs#completion_e8e85420b5b4510c_visual_text_encoding_sampling_storage_ranking_mo.

#### requirement-bd5095a19a955167

- Candidate/source: `doc-6f1469d84caf79d7` at `docs/specs/media/5-search.md:57` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:57 under “5.4 视觉去重抽帧 `FrameSampler`(luma 8×8 + 镜头边界 + 覆盖下限)” (heading), the source “## 5.4 视觉去重抽帧 `FrameSampler`(luma 8×8 + 镜头边界 + 覆盖下限)” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:57; signal=heading; heading=5.4 视觉去重抽帧 `FrameSampler`(luma 8×8 + 镜头边界 + 覆盖下限); candidate=## 5.4 视觉去重抽帧 `FrameSampler`(luma 8×8 + 镜头边界 + 覆盖下限)
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “5.4 视觉去重抽帧 `FrameSampler`(luma 8×8 + 镜头边界 + 覆盖下限)” in “5.4 视觉去重抽帧 `FrameSampler`(luma 8×8 + 镜头边界 + 覆盖下限)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.4 视觉去重抽帧 `FrameSampler`(luma 8×8 + 镜头边界 + 覆盖下限)” with the scenario below and register test:crates/opentake-render/tests/completion_6f1469d84caf79d7.rs#completion_6f1469d84caf79d7_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “5.4 视觉去重抽帧 `FrameSampler`(luma 8×8 + 镜头边界 + 覆盖下限)”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_6f1469d84caf79d7.rs#completion_6f1469d84caf79d7_visual_text_encoding_sampling_storage_ranking_mo.

#### requirement-43fbc8e32bd126c4

- Candidate/source: `doc-1811f73f01d9e6ee` at `docs/specs/media/5-search.md:96` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:96 under “5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等)” (heading), the source “## 5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等)” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:96; signal=heading; heading=5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等); candidate=## 5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等)
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等)” in “5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等)” with the scenario below and register test:tools/completion-tests/doc-1811f73f01d9e6ee.test.mjs#completion_1811f73f01d9e6ee_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: construct the smallest deterministic state that exposes “5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等)”, apply the precise input or event implied by “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-1811f73f01d9e6ee.test.mjs#completion_1811f73f01d9e6ee_visual_text_encoding_sampling_storage_ranking_mo.

#### requirement-ae88b5f4d3f80eb6

- Candidate/source: `doc-d6e117c9221f7212` at `docs/specs/media/5-search.md:119` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:119 under “5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻” (heading), the source “## 5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:119; signal=heading; heading=5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻; candidate=## 5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻” in “5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻” with the scenario below and register test:tools/completion-tests/doc-d6e117c9221f7212.test.mjs#completion_d6e117c9221f7212_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: construct the smallest deterministic state that exposes “5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻”, apply the precise input or event implied by “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-d6e117c9221f7212.test.mjs#completion_d6e117c9221f7212_visual_text_encoding_sampling_storage_ranking_mo.

#### requirement-7b2d77c2b48b0238

- Candidate/source: `doc-a21452bee152729e` at `docs/specs/media/5-search.md:159` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:159 under “5.7 推理后端实现(ort 默认 / candle 仅备选)” (heading), the source “## 5.7 推理后端实现(ort 默认 / candle 仅备选)” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:159; signal=heading; heading=5.7 推理后端实现(ort 默认 / candle 仅备选); candidate=## 5.7 推理后端实现(ort 默认 / candle 仅备选)
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “5.7 推理后端实现(ort 默认 / candle 仅备选)” in “5.7 推理后端实现(ort 默认 / candle 仅备选)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.7 推理后端实现(ort 默认 / candle 仅备选)” with the scenario below and register test:tools/completion-tests/doc-a21452bee152729e.test.mjs#completion_a21452bee152729e_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: construct the smallest deterministic state that exposes “5.7 推理后端实现(ort 默认 / candle 仅备选)”, apply the precise input or event implied by “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “5.7 推理后端实现(ort 默认 / candle 仅备选)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-a21452bee152729e.test.mjs#completion_a21452bee152729e_visual_text_encoding_sampling_storage_ranking_mo.

#### requirement-8d06ffbfdd515f03

- Candidate/source: `doc-ee740cbfb7d925d3` at `docs/specs/media/5-search.md:178` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:178 under “5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数” (heading), the source “## 5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:178; signal=heading; heading=5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数; candidate=## 5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数” in “5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数” with the scenario below and register test:tools/completion-tests/doc-ee740cbfb7d925d3.test.mjs#completion_ee740cbfb7d925d3_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: construct the smallest deterministic state that exposes “5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数”, apply the precise input or event implied by “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-ee740cbfb7d925d3.test.mjs#completion_ee740cbfb7d925d3_visual_text_encoding_sampling_storage_ranking_mo.

#### requirement-980f6325823ecf64

- Candidate/source: `doc-b8caaa2d7b93a6a5` at `docs/specs/media/5-search.md:205` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:205 under “5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip)” (heading), the source “## 5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip)” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:205; signal=heading; heading=5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip); candidate=## 5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip)
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip)” in “5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip)” with the scenario below and register test:tools/completion-tests/doc-b8caaa2d7b93a6a5.test.mjs#completion_b8caaa2d7b93a6a5_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: construct the smallest deterministic state that exposes “5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip)”, apply the precise input or event implied by “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-b8caaa2d7b93a6a5.test.mjs#completion_b8caaa2d7b93a6a5_visual_text_encoding_sampling_storage_ranking_mo.

#### requirement-f917123fb8d790f1

- Candidate/source: `doc-3de70a17881eb555` at `docs/specs/media/5-search.md:234` (requirement)
- Expected behavior: At docs/specs/media/5-search.md:234 under “5.10 配置 `SearchIndexConfig` 等价” (heading), the source “## 5.10 配置 `SearchIndexConfig` 等价” requires this exact behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.
- Resolution: `reviewed-mapping-report:misgrouped-media-search` — These eleven heading-derived records belong to media search, not Inspector/text/keyframes, and duplicate one implemented search capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/5-search.md:234; signal=heading; heading=5.10 配置 `SearchIndexConfig` 等价; candidate=## 5.10 配置 `SearchIndexConfig` 等价
  - Expected behavior: Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests. This closes only the promise expressed by “5.10 配置 `SearchIndexConfig` 等价” in “5.10 配置 `SearchIndexConfig` 等价”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.10 配置 `SearchIndexConfig` 等价” with the scenario below and register test:tools/completion-tests/doc-3de70a17881eb555.test.mjs#completion_3de70a17881eb555_visual_text_encoding_sampling_storage_ranking_mo
  - Initial state/input/event: construct the smallest deterministic state that exposes “5.10 配置 `SearchIndexConfig` 等价”, apply the precise input or event implied by “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Visual/text encoding, sampling, storage, ranking, model install, and configuration are implemented with focused tests.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “5.10 配置 `SearchIndexConfig` 等价”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-3de70a17881eb555.test.mjs#completion_3de70a17881eb555_visual_text_encoding_sampling_storage_ranking_mo.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/src/search/embedder.rs#preprocess_squashes_non_square_to_square` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/search/tokenizer.rs#pads_short_sequence_to_context_length` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/search/embed_store.rs#encode_decode_roundtrip_f16_quantized` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/search/ranker.rs#best_per_shot_dedupes_same_shot` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/search/mod.rs#index_then_rank_finds_brightest_match` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media preprocess_squashes_non_square_to_square`
  - Run: `cargo test -p opentake-media pads_short_sequence_to_context_length`
  - Run: `cargo test -p opentake-media encode_decode_roundtrip_f16_quantized`
  - Run: `cargo test -p opentake-media best_per_shot_dedupes_same_shot`
  - Run: `cargo test -p opentake-media index_then_rank_finds_brightest_match`

  Expected: FAIL because one or more of the 11 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/search/embedder.rs#Embedder`, `crates/opentake-media/src/search/tokenizer.rs#SiglipTokenizer`, `crates/opentake-media/src/search/indexer.rs#index_video`, `crates/opentake-media/src/search/embed_store.rs#AssetIndex`, `crates/opentake-media/src/search/ranker.rs#search`, `src-tauri/src/search.rs#search_query`, `docs/specs/media/5-search.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media preprocess_squashes_non_square_to_square`
  - Run: `cargo test -p opentake-media pads_short_sequence_to_context_length`
  - Run: `cargo test -p opentake-media encode_decode_roundtrip_f16_quantized`
  - Run: `cargo test -p opentake-media best_per_shot_dedupes_same_shot`
  - Run: `cargo test -p opentake-media index_then_rank_finds_brightest_match`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 12: misgrouped-transcription (implementation-slice-41e75cf13fabe7b1)

**Covered records:**
- `requirement-5ee9b7ba12472dd2` (requirement)
- `requirement-182ee5a96ef748eb` (requirement)
- `requirement-033385c9654f4781` (requirement)
- `requirement-1cfe5d8575e5cc93` (requirement)
- `requirement-291a7211a4ee4b55` (requirement)
- `requirement-94aebecde1fc0542` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/transcribe/mod.rs#Transcriber`
- Modify: `crates/opentake-media/src/transcribe/mod.rs#transcribe_file`
- Modify: `crates/opentake-media/src/transcribe/cache.rs#TranscriptCache`
- Modify: `crates/opentake-media/src/transcribe/search.rs#search`
- Modify: `crates/opentake-media/src/transcribe/locale.rs#match_locale`
- Modify: `crates/opentake-media/src/transcribe/whisper.rs#WhisperTranscriber`
- Modify: `src-tauri/src/transcribe.rs#transcribe_media`
- Modify: `docs/specs/media/6-transcribe.md`
- Test (existing-owned): `crates/opentake-media/src/transcribe/cache.rs#memory_lru_clears_wholesale_at_capacity`
- Test (existing-owned): `crates/opentake-media/src/transcribe/search.rs#search_collects_and_respects_limit`
- Test (existing-owned): `crates/opentake-media/src/transcribe/locale.rs#picks_same_language_same_region`
- Test (existing-owned): `crates/opentake-media/src/transcribe/whisper.rs#centiseconds_convert_to_seconds`

**Candidate-bound contracts:**

#### requirement-5ee9b7ba12472dd2

- Candidate/source: `doc-3b20f57c5c6710a2` at `docs/specs/media/6-transcribe.md:1` (requirement)
- Expected behavior: At docs/specs/media/6-transcribe.md:1 under “whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型)” (heading), the source “# whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型)” requires this exact behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented.
- Resolution: `reviewed-mapping-report:misgrouped-transcription` — These six heading records belong to media transcription, not accessibility polish, and duplicate one implemented transcription capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/6-transcribe.md:1; signal=heading; heading=whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型); candidate=# whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型)
  - Expected behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented. This closes only the promise expressed by “whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型)” in “whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型)” with the scenario below and register test:tools/completion-tests/doc-3b20f57c5c6710a2.test.mjs#completion_3b20f57c5c6710a2_timestamped_transcription_cache_keyword_search_a
  - Initial state/input/event: construct the smallest deterministic state that exposes “whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型)”, apply the precise input or event implied by “Timestamped transcription, cache, keyword search, and locale matching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Timestamped transcription, cache, keyword search, and locale matching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-3b20f57c5c6710a2.test.mjs#completion_3b20f57c5c6710a2_timestamped_transcription_cache_keyword_search_a.

#### requirement-182ee5a96ef748eb

- Candidate/source: `doc-291383ebe93c38ff` at `docs/specs/media/6-transcribe.md:5` (requirement)
- Expected behavior: At docs/specs/media/6-transcribe.md:5 under “6.1 数据模型(逐行 1:1 port)” (heading), the source “## 6.1 数据模型(逐行 1:1 port)” requires this exact behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented.
- Resolution: `reviewed-mapping-report:misgrouped-transcription` — These six heading records belong to media transcription, not accessibility polish, and duplicate one implemented transcription capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/6-transcribe.md:5; signal=heading; heading=6.1 数据模型(逐行 1:1 port); candidate=## 6.1 数据模型(逐行 1:1 port)
  - Expected behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented. This closes only the promise expressed by “6.1 数据模型(逐行 1:1 port)” in “6.1 数据模型(逐行 1:1 port)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.1 数据模型(逐行 1:1 port)” with the scenario below and register test:tools/completion-tests/doc-291383ebe93c38ff.test.mjs#completion_291383ebe93c38ff_timestamped_transcription_cache_keyword_search_a
  - Initial state/input/event: construct the smallest deterministic state that exposes “6.1 数据模型(逐行 1:1 port)”, apply the precise input or event implied by “Timestamped transcription, cache, keyword search, and locale matching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Timestamped transcription, cache, keyword search, and locale matching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “6.1 数据模型(逐行 1:1 port)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-291383ebe93c38ff.test.mjs#completion_291383ebe93c38ff_timestamped_transcription_cache_keyword_search_a.

#### requirement-033385c9654f4781

- Candidate/source: `doc-b2fe3d2d5016c10f` at `docs/specs/media/6-transcribe.md:31` (requirement)
- Expected behavior: At docs/specs/media/6-transcribe.md:31 under “6.2 转写后端 trait + whisper 实现” (heading), the source “## 6.2 转写后端 trait + whisper 实现” requires this exact behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented.
- Resolution: `reviewed-mapping-report:misgrouped-transcription` — These six heading records belong to media transcription, not accessibility polish, and duplicate one implemented transcription capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/6-transcribe.md:31; signal=heading; heading=6.2 转写后端 trait + whisper 实现; candidate=## 6.2 转写后端 trait + whisper 实现
  - Expected behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented. This closes only the promise expressed by “6.2 转写后端 trait + whisper 实现” in “6.2 转写后端 trait + whisper 实现”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.2 转写后端 trait + whisper 实现” with the scenario below and register test:tools/completion-tests/doc-b2fe3d2d5016c10f.test.mjs#completion_b2fe3d2d5016c10f_timestamped_transcription_cache_keyword_search_a
  - Initial state/input/event: construct the smallest deterministic state that exposes “6.2 转写后端 trait + whisper 实现”, apply the precise input or event implied by “Timestamped transcription, cache, keyword search, and locale matching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Timestamped transcription, cache, keyword search, and locale matching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “6.2 转写后端 trait + whisper 实现”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-b2fe3d2d5016c10f.test.mjs#completion_b2fe3d2d5016c10f_timestamped_transcription_cache_keyword_search_a.

#### requirement-1cfe5d8575e5cc93

- Candidate/source: `doc-68e466bc9820121a` at `docs/specs/media/6-transcribe.md:63` (requirement)
- Expected behavior: At docs/specs/media/6-transcribe.md:63 under “6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter)” (heading), the source “## 6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter)” requires this exact behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented.
- Resolution: `reviewed-mapping-report:misgrouped-transcription` — These six heading records belong to media transcription, not accessibility polish, and duplicate one implemented transcription capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/6-transcribe.md:63; signal=heading; heading=6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter); candidate=## 6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter)
  - Expected behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented. This closes only the promise expressed by “6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter)” in “6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter)” with the scenario below and register test:tools/completion-tests/doc-68e466bc9820121a.test.mjs#completion_68e466bc9820121a_timestamped_transcription_cache_keyword_search_a
  - Initial state/input/event: construct the smallest deterministic state that exposes “6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter)”, apply the precise input or event implied by “Timestamped transcription, cache, keyword search, and locale matching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Timestamped transcription, cache, keyword search, and locale matching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-68e466bc9820121a.test.mjs#completion_68e466bc9820121a_timestamped_transcription_cache_keyword_search_a.

#### requirement-291a7211a4ee4b55

- Candidate/source: `doc-bb063f771cb44bd2` at `docs/specs/media/6-transcribe.md:87` (requirement)
- Expected behavior: At docs/specs/media/6-transcribe.md:87 under “6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数)” (heading), the source “## 6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数)” requires this exact behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented.
- Resolution: `reviewed-mapping-report:misgrouped-transcription` — These six heading records belong to media transcription, not accessibility polish, and duplicate one implemented transcription capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/6-transcribe.md:87; signal=heading; heading=6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数); candidate=## 6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数)
  - Expected behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented. This closes only the promise expressed by “6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数)” in “6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数)” with the scenario below and register test:tools/completion-tests/doc-bb063f771cb44bd2.test.mjs#completion_bb063f771cb44bd2_timestamped_transcription_cache_keyword_search_a
  - Initial state/input/event: construct the smallest deterministic state that exposes “6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数)”, apply the precise input or event implied by “Timestamped transcription, cache, keyword search, and locale matching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Timestamped transcription, cache, keyword search, and locale matching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-bb063f771cb44bd2.test.mjs#completion_bb063f771cb44bd2_timestamped_transcription_cache_keyword_search_a.

#### requirement-94aebecde1fc0542

- Candidate/source: `doc-91f61f957b4d8680` at `docs/specs/media/6-transcribe.md:105` (requirement)
- Expected behavior: At docs/specs/media/6-transcribe.md:105 under “6.5 locale 匹配(纯逻辑,照搬)” (heading), the source “## 6.5 locale 匹配(纯逻辑,照搬)” requires this exact behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented.
- Resolution: `reviewed-mapping-report:misgrouped-transcription` — These six heading records belong to media transcription, not accessibility polish, and duplicate one implemented transcription capability.
- Exact acceptance contract:
  - Source binding: docs/specs/media/6-transcribe.md:105; signal=heading; heading=6.5 locale 匹配(纯逻辑,照搬); candidate=## 6.5 locale 匹配(纯逻辑,照搬)
  - Expected behavior: Timestamped transcription, cache, keyword search, and locale matching are implemented. This closes only the promise expressed by “6.5 locale 匹配(纯逻辑,照搬)” in “6.5 locale 匹配(纯逻辑,照搬)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.5 locale 匹配(纯逻辑,照搬)” with the scenario below and register test:tools/completion-tests/doc-91f61f957b4d8680.test.mjs#completion_91f61f957b4d8680_timestamped_transcription_cache_keyword_search_a
  - Initial state/input/event: construct the smallest deterministic state that exposes “6.5 locale 匹配(纯逻辑,照搬)”, apply the precise input or event implied by “Timestamped transcription, cache, keyword search, and locale matching are implemented.”, and include one success plus one boundary/failure case.
  - Code/store/API/Rust effect: at the narrowest typed code/store/API/Rust boundary that owns the named behavior, perform only “Timestamped transcription, cache, keyword search, and locale matching are implemented.” at the narrowest typed code/store/API/Rust boundary that owns the named behavior, with no unrelated state, file, network, or UI mutation.
  - Visible/returned assertion: assert the exact returned success/error and the observable state described by “6.5 locale 匹配(纯逻辑,照搬)”, including deterministic no-op behavior when the operation is rejected.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-91f61f957b4d8680.test.mjs#completion_91f61f957b4d8680_timestamped_transcription_cache_keyword_search_a.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/src/transcribe/cache.rs#memory_lru_clears_wholesale_at_capacity` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/transcribe/search.rs#search_collects_and_respects_limit` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/transcribe/locale.rs#picks_same_language_same_region` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/transcribe/whisper.rs#centiseconds_convert_to_seconds` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media memory_lru_clears_wholesale_at_capacity`
  - Run: `cargo test -p opentake-media search_collects_and_respects_limit`
  - Run: `cargo test -p opentake-media picks_same_language_same_region`
  - Run: `cargo test -p opentake-media --features whisper-backend centiseconds_convert_to_seconds`

  Expected: FAIL because one or more of the 6 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/transcribe/mod.rs#Transcriber`, `crates/opentake-media/src/transcribe/mod.rs#transcribe_file`, `crates/opentake-media/src/transcribe/cache.rs#TranscriptCache`, `crates/opentake-media/src/transcribe/search.rs#search`, `crates/opentake-media/src/transcribe/locale.rs#match_locale`, `crates/opentake-media/src/transcribe/whisper.rs#WhisperTranscriber`, `src-tauri/src/transcribe.rs#transcribe_media`, `docs/specs/media/6-transcribe.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media memory_lru_clears_wholesale_at_capacity`
  - Run: `cargo test -p opentake-media search_collects_and_respects_limit`
  - Run: `cargo test -p opentake-media picks_same_language_same_region`
  - Run: `cargo test -p opentake-media --features whisper-backend centiseconds_convert_to_seconds`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 13: control-acceptance (implementation-slice-9fbd867c5461047f)

**Covered records:**
- `control-record-7c67e4d4dadb412e` (control)
- `control-record-cadeb85d8abf6e81` (control)
- `control-record-3d8dfba2646937dd` (control)
- `control-record-53cb5ef9b3ae03e8` (control)
- `control-record-103842922285be40` (control)
- `control-record-315e8e7eb8012ae8` (control)

**Files:**
- Modify: `web/src/components/media/LibraryView.tsx`
- Modify: `web/src/components/media/LibraryView.tsx#LibraryView`
- Test (reviewed-planned): `web/src/components/media/LibraryView.interaction.test.tsx#control-41a94f001615c840 return from Library to Home`
- Test (reviewed-planned): `web/src/components/media/LibraryView.interaction.test.tsx#control-34cc86ad5cb6f2bc filter global Library entries`
- Test (reviewed-planned): `web/src/components/media/LibraryView.interaction.test.tsx#control-c402fbc7cbe118be sort global Library entries`
- Test (reviewed-planned): `web/src/components/media/LibraryView.interaction.test.tsx#control-4d5be5e3030fbe57 choose a built-in Library category`
- Test (reviewed-planned): `web/src/components/media/LibraryView.interaction.test.tsx#control-37c952a4d43081bc choose a custom Library category`
- Test (reviewed-planned): `web/src/components/media/LibraryView.interaction.test.tsx#control-c01f19ca0521a8eb reveal Library card actions on hover`

**Candidate-bound contracts:**

#### control-record-7c67e4d4dadb412e

- Candidate/source: `control-41a94f001615c840` at `web/src/components/media/LibraryView.tsx:131:11` (control)
- Expected behavior: return from Library to Home: setView('home')
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-41a94f001615c840.
  - Test: web/src/components/media/LibraryView.interaction.test.tsx#control-41a94f001615c840 return from Library to Home.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setView(\"home\")}","click or native keyboard activation plus current owning state"]; handler={() => setView("home")}.
  - Exact call/state/backend: stateTransition=setView('home'); backendTrace=["web/src/components/media/LibraryView.tsx:131::candidate handler -> {() => setView(\"home\")}","actual branch/state -> setView('home')","exact call -> setView('home')","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/LibraryView.tsx#LibraryView"].
  - Visible/accessibility/return path: success=return from Library to Home: setView('home'); accessibility={"focus":"Native keyboard-focusable control","label":"t(\"title.backHome\")","shortcut":"None declared on this control"}; returnPath=["The Back Home button returns to Home; card operations remain in Library."].
  - Outcome matrix: {"success":"return from Library to Home: setView('home')","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/LibraryView.tsx:131; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-cadeb85d8abf6e81

- Candidate/source: `control-34cc86ad5cb6f2bc` at `web/src/components/media/LibraryView.tsx:162:11` (control)
- Expected behavior: filter global Library entries: setSearch -> selectEntries recomputes visible cards
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-34cc86ad5cb6f2bc.
  - Test: web/src/components/media/LibraryView.interaction.test.tsx#control-34cc86ad5cb6f2bc filter global Library entries.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {setSearch}","click or native keyboard activation plus current owning state"]; handler={setSearch}.
  - Exact call/state/backend: stateTransition=setSearch -> selectEntries recomputes visible cards; backendTrace=["web/src/components/media/LibraryView.tsx:162::candidate handler -> {setSearch}","actual branch/state -> setSearch -> selectEntries recomputes visible cards","exact call -> setSearch -> selectEntries recomputes visible cards","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/LibraryView.tsx#LibraryView"].
  - Visible/accessibility/return path: success=filter global Library entries: setSearch -> selectEntries recomputes visible cards; accessibility={"focus":"Custom SearchBox focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Back Home button returns to Home; card operations remain in Library."].
  - Outcome matrix: {"success":"filter global Library entries: setSearch -> selectEntries recomputes visible cards","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/LibraryView.tsx:162; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-3d8dfba2646937dd

- Candidate/source: `control-c402fbc7cbe118be` at `web/src/components/media/LibraryView.tsx:163:11` (control)
- Expected behavior: sort global Library entries: setSort -> selectEntries changes recent/oldest/type order
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c402fbc7cbe118be.
  - Test: web/src/components/media/LibraryView.interaction.test.tsx#control-c402fbc7cbe118be sort global Library entries.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {setSort}","click or native keyboard activation plus current owning state"]; handler={setSort}.
  - Exact call/state/backend: stateTransition=setSort -> selectEntries changes recent/oldest/type order; backendTrace=["web/src/components/media/LibraryView.tsx:163::candidate handler -> {setSort}","actual branch/state -> setSort -> selectEntries changes recent/oldest/type order","exact call -> setSort -> selectEntries changes recent/oldest/type order","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/LibraryView.tsx#LibraryView"].
  - Visible/accessibility/return path: success=sort global Library entries: setSort -> selectEntries changes recent/oldest/type order; accessibility={"focus":"Custom SortSelect focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Back Home button returns to Home; card operations remain in Library."].
  - Outcome matrix: {"success":"sort global Library entries: setSort -> selectEntries changes recent/oldest/type order","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/LibraryView.tsx:163; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-53cb5ef9b3ae03e8

- Candidate/source: `control-4d5be5e3030fbe57` at `web/src/components/media/LibraryView.tsx:220:9` (control)
- Expected behavior: choose a built-in Library category: setSelectedCategory(c.id) filters entries
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-4d5be5e3030fbe57.
  - Test: web/src/components/media/LibraryView.interaction.test.tsx#control-4d5be5e3030fbe57 choose a built-in Library category.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setSelectedCategory(c.id)}","click or native keyboard activation plus current owning state"]; handler={() => setSelectedCategory(c.id)}.
  - Exact call/state/backend: stateTransition=setSelectedCategory(c.id) filters entries; backendTrace=["web/src/components/media/LibraryView.tsx:220::candidate handler -> {() => setSelectedCategory(c.id)}","actual branch/state -> setSelectedCategory(c.id) filters entries","exact call -> setSelectedCategory(c.id) filters entries","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/LibraryView.tsx#LibraryView"].
  - Visible/accessibility/return path: success=choose a built-in Library category: setSelectedCategory(c.id) filters entries; accessibility={"focus":"Custom CategoryRow focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Back Home button returns to Home; card operations remain in Library."].
  - Outcome matrix: {"success":"choose a built-in Library category: setSelectedCategory(c.id) filters entries","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/LibraryView.tsx:220; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-103842922285be40

- Candidate/source: `control-37c952a4d43081bc` at `web/src/components/media/LibraryView.tsx:244:9` (control)
- Expected behavior: choose a custom Library category: setSelectedCategory(name) filters entries
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-37c952a4d43081bc.
  - Test: web/src/components/media/LibraryView.interaction.test.tsx#control-37c952a4d43081bc choose a custom Library category.
  - Initial state: visibility=Rendered when custom categories exist; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setSelectedCategory(name)}","click or native keyboard activation plus current owning state"]; handler={() => setSelectedCategory(name)}.
  - Exact call/state/backend: stateTransition=setSelectedCategory(name) filters entries; backendTrace=["web/src/components/media/LibraryView.tsx:244::candidate handler -> {() => setSelectedCategory(name)}","actual branch/state -> setSelectedCategory(name) filters entries","exact call -> setSelectedCategory(name) filters entries","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/LibraryView.tsx#LibraryView"].
  - Visible/accessibility/return path: success=choose a custom Library category: setSelectedCategory(name) filters entries; accessibility={"focus":"Custom CategoryRow focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Back Home button returns to Home; card operations remain in Library."].
  - Outcome matrix: {"success":"choose a custom Library category: setSelectedCategory(name) filters entries","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/LibraryView.tsx:244; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-315e8e7eb8012ae8

- Candidate/source: `control-c01f19ca0521a8eb` at `web/src/components/media/LibraryView.tsx:470:5` (control)
- Expected behavior: reveal Library card actions on hover: setHovered controls action-row visibility
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c01f19ca0521a8eb.
  - Test: web/src/components/media/LibraryView.interaction.test.tsx#control-c01f19ca0521a8eb reveal Library card actions on hover.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setHovered(true)} {() => setHovered(false)}","pointer/drag coordinates, button/key, and current owning state"]; handler={() => setHovered(true)} {() => setHovered(false)}.
  - Exact call/state/backend: stateTransition=setHovered controls action-row visibility; backendTrace=["web/src/components/media/LibraryView.tsx:470::candidate handler -> {() => setHovered(true)} {() => setHovered(false)}","actual branch/state -> setHovered controls action-row visibility","exact call -> setHovered controls action-row visibility","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/LibraryView.tsx#LibraryView"].
  - Visible/accessibility/return path: success=reveal Library card actions on hover: setHovered controls action-row visibility; accessibility={"focus":"Non-focusable hover container; keyboard users cannot reveal the actions","label":"name","shortcut":"None declared on this control"}; returnPath=["The Back Home button returns to Home; card operations remain in Library."].
  - Outcome matrix: {"success":"reveal Library card actions on hover: setHovered controls action-row visibility","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/LibraryView.tsx:470; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/LibraryView.interaction.test.tsx#control-41a94f001615c840 return from Library to Home` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/LibraryView.interaction.test.tsx#control-34cc86ad5cb6f2bc filter global Library entries` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/LibraryView.interaction.test.tsx#control-c402fbc7cbe118be sort global Library entries` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/LibraryView.interaction.test.tsx#control-4d5be5e3030fbe57 choose a built-in Library category` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/LibraryView.interaction.test.tsx#control-37c952a4d43081bc choose a custom Library category` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/LibraryView.interaction.test.tsx#control-c01f19ca0521a8eb reveal Library card actions on hover` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-41a94f001615c840 return from Library to Home"`
  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-34cc86ad5cb6f2bc filter global Library entries"`
  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-c402fbc7cbe118be sort global Library entries"`
  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-4d5be5e3030fbe57 choose a built-in Library category"`
  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-37c952a4d43081bc choose a custom Library category"`
  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-c01f19ca0521a8eb reveal Library card actions on hover"`

  Expected: FAIL because one or more of the 6 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/LibraryView.tsx`, `web/src/components/media/LibraryView.tsx#LibraryView` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-41a94f001615c840 return from Library to Home"`
  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-34cc86ad5cb6f2bc filter global Library entries"`
  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-c402fbc7cbe118be sort global Library entries"`
  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-4d5be5e3030fbe57 choose a built-in Library category"`
  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-37c952a4d43081bc choose a custom Library category"`
  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-c01f19ca0521a8eb reveal Library card actions on hover"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 14: control-acceptance (implementation-slice-98278a2f0e8d06ab)

**Covered records:**
- `control-record-f761d254918f732a` (control)

**Files:**
- Modify: `web/src/components/media/LibraryView.tsx`
- Modify: `web/src/store/libraryStore.ts#importToProject`
- Modify: `web/src/lib/libraryApi.ts#libraryImportToProject`
- Modify: `src-tauri/src/library.rs#library_import_to_project`
- Modify: `crates/opentake-media/src/library.rs`
- Modify: `web/src/components/media/LibraryView.tsx#LibraryView`
- Test (reviewed-planned): `web/src/components/media/LibraryView.interaction.test.tsx#control-4675df3783dc0137 import a global Library item into the current project`

**Candidate-bound contracts:**

#### control-record-f761d254918f732a

- Candidate/source: `control-4675df3783dc0137` at `web/src/components/media/LibraryView.tsx:520:13` (control)
- Expected behavior: import a global Library item into the current project: busy true -> libraryStore.importToProject -> media refresh -> busy false
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-4675df3783dc0137.
  - Test: web/src/components/media/LibraryView.interaction.test.tsx#control-4675df3783dc0137 import a global Library item into the current project.
  - Initial state: visibility=Only while card is hovered; enabledWhen=not ({busy}).
  - Event: inputs=["event/prop handler: {() => void handleImport()}","click or native keyboard activation plus current owning state"]; handler={() => void handleImport()}.
  - Exact call/state/backend: stateTransition=busy true -> libraryStore.importToProject -> media refresh -> busy false; backendTrace=["web/src/components/media/LibraryView.tsx:520::candidate handler -> {() => void handleImport()}","actual branch/state -> busy true -> libraryStore.importToProject -> media refresh -> busy false","exact call/arguments -> useLibraryStore.importToProject(entry.id) -> libraryImportToProject(id), then refreshMedia(); do not refresh the global library list","web/src/store/libraryStore.ts::importToProject(id) -> lib.libraryImportToProject(id) -> refreshMedia()","web/src/lib/libraryApi.ts::libraryImportToProject -> invoke('library_import_to_project',{id})","src-tauri/src/library.rs::library_import_to_project(id) -> library_import_to_project_impl -> crates/opentake-media/src/library.rs library store","code:web/src/components/media/LibraryView.tsx#LibraryView","code:web/src/lib/libraryApi.ts#libraryImportToProject","code:src-tauri/src/library.rs#library_import_to_project"].
  - Visible/accessibility/return path: success=import a global Library item into the current project: busy true -> libraryStore.importToProject -> media refresh -> busy false; accessibility={"focus":"Custom CardAction focus behavior depends on its implementation","label":"t(\"library.import\")","shortcut":"None declared on this control"}; returnPath=["The Back Home button returns to Home; card operations remain in Library."].
  - Outcome matrix: {"success":"import a global Library item into the current project: busy true -> libraryStore.importToProject -> media refresh -> busy false","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/LibraryView.tsx:520; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/LibraryView.tsx:520; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {busy}.","cancel":"Cancellation/dismissal follows the exact guard in busy true -> libraryStore.importToProject -> media refresh -> busy false; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in busy true -> libraryStore.importToProject -> media refresh -> busy false.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/LibraryView.tsx:520; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/LibraryView.interaction.test.tsx#control-4675df3783dc0137 import a global Library item into the current project` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-4675df3783dc0137 import a global Library item into the current project"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/LibraryView.tsx`, `web/src/store/libraryStore.ts#importToProject`, `web/src/lib/libraryApi.ts#libraryImportToProject`, `src-tauri/src/library.rs#library_import_to_project`, `crates/opentake-media/src/library.rs`, `web/src/components/media/LibraryView.tsx#LibraryView` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-4675df3783dc0137 import a global Library item into the current project"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 15: control-acceptance (implementation-slice-808c89de75839645)

**Covered records:**
- `control-record-0bd33d36966c4005` (control)

**Files:**
- Modify: `web/src/components/media/LibraryView.tsx`
- Modify: `web/src/components/media/LibraryView.tsx#handleCategorize`
- Modify: `web/src/store/libraryStore.ts#categorize`
- Modify: `web/src/lib/libraryApi.ts#libraryCategorize`
- Modify: `src-tauri/src/library.rs#library_categorize`
- Modify: `crates/opentake-media/src/library.rs#LibraryStore::set_category`
- Modify: `web/src/components/media/LibraryView.tsx#LibraryView`
- Modify: `crates/opentake-media/src/library.rs#LibraryStore`
- Test (reviewed-planned): `web/src/components/media/LibraryView.interaction.test.tsx#control-4154ff9083c075ad categorize a global Library item`

**Candidate-bound contracts:**

#### control-record-0bd33d36966c4005

- Candidate/source: `control-4154ff9083c075ad` at `web/src/components/media/LibraryView.tsx:526:13` (control)
- Expected behavior: categorize a global Library item: prompt result -> libraryStore.categorize -> refresh
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-4154ff9083c075ad.
  - Test: web/src/components/media/LibraryView.interaction.test.tsx#control-4154ff9083c075ad categorize a global Library item.
  - Initial state: visibility=Only while card is hovered; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {handleCategorize}","click or native keyboard activation plus current owning state"]; handler={handleCategorize}.
  - Exact call/state/backend: stateTransition=prompt result -> libraryStore.categorize -> refresh; backendTrace=["web/src/components/media/LibraryView.tsx:526::candidate handler -> {handleCategorize}","actual branch/state -> prompt result -> libraryStore.categorize -> refresh","exact call/arguments -> after prompt, categorize(entry.id, trimmed === '' ? null : trimmed) -> libraryCategorize(id,category), then refresh() -> libraryList()","web/src/components/media/LibraryView.tsx::handleCategorize -> window.prompt then useLibraryStore.categorize(entry.id,category)","web/src/store/libraryStore.ts::categorize -> libraryCategorize(id,category) then refresh/libraryList","web/src/lib/libraryApi.ts::libraryCategorize -> invoke('library_categorize',{id,category}); libraryList -> invoke('library_list',{category:undefined})","src-tauri/src/library.rs::library_categorize/library_list -> crates/opentake-media/src/library.rs::LibraryStore::set_category/entries","code:web/src/components/media/LibraryView.tsx#LibraryView","code:web/src/components/media/LibraryView.tsx#handleCategorize","code:web/src/lib/libraryApi.ts#libraryCategorize","code:src-tauri/src/library.rs#library_categorize","code:crates/opentake-media/src/library.rs#LibraryStore"].
  - Visible/accessibility/return path: success=categorize a global Library item: prompt result -> libraryStore.categorize -> refresh; accessibility={"focus":"Custom CardAction focus behavior depends on its implementation","label":"t(\"library.categorize\")","shortcut":"None declared on this control"}; returnPath=["The Back Home button returns to Home; card operations remain in Library."].
  - Outcome matrix: {"success":"categorize a global Library item: prompt result -> libraryStore.categorize -> refresh","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/LibraryView.tsx:526; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/LibraryView.tsx:526; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in prompt result -> libraryStore.categorize -> refresh.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/LibraryView.tsx:526; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/LibraryView.interaction.test.tsx#control-4154ff9083c075ad categorize a global Library item` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-4154ff9083c075ad categorize a global Library item"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/LibraryView.tsx`, `web/src/components/media/LibraryView.tsx#handleCategorize`, `web/src/store/libraryStore.ts#categorize`, `web/src/lib/libraryApi.ts#libraryCategorize`, `src-tauri/src/library.rs#library_categorize`, `crates/opentake-media/src/library.rs#LibraryStore::set_category`, `web/src/components/media/LibraryView.tsx#LibraryView`, `crates/opentake-media/src/library.rs#LibraryStore` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-4154ff9083c075ad categorize a global Library item"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 16: control-acceptance (implementation-slice-d54bfb66d66b8d48)

**Covered records:**
- `control-record-b6a924a9db445981` (control)

**Files:**
- Modify: `web/src/components/media/LibraryView.tsx`
- Modify: `web/src/store/libraryStore.ts#unfavorite`
- Modify: `web/src/lib/libraryApi.ts#libraryUnfavorite`
- Modify: `src-tauri/src/library.rs#library_unfavorite`
- Modify: `crates/opentake-media/src/library.rs#LibraryStore::remove`
- Modify: `web/src/components/media/LibraryView.tsx#LibraryView`
- Modify: `crates/opentake-media/src/library.rs#LibraryStore`
- Test (reviewed-planned): `web/src/components/media/LibraryView.interaction.test.tsx#control-31b29012d24167aa unfavorite a global Library item`

**Candidate-bound contracts:**

#### control-record-b6a924a9db445981

- Candidate/source: `control-31b29012d24167aa` at `web/src/components/media/LibraryView.tsx:527:13` (control)
- Expected behavior: unfavorite a global Library item: libraryStore.unfavorite -> refresh or visible error
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-31b29012d24167aa.
  - Test: web/src/components/media/LibraryView.interaction.test.tsx#control-31b29012d24167aa unfavorite a global Library item.
  - Initial state: visibility=Only while card is hovered; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => void unfavorite(entry.id)}","click or native keyboard activation plus current owning state"]; handler={() => void unfavorite(entry.id)}.
  - Exact call/state/backend: stateTransition=libraryStore.unfavorite -> refresh or visible error; backendTrace=["web/src/components/media/LibraryView.tsx:527::candidate handler -> {() => void unfavorite(entry.id)}","actual branch/state -> libraryStore.unfavorite -> refresh or visible error","exact call/arguments -> unfavorite(entry.id) -> libraryUnfavorite(id), then refresh() -> libraryList()","web/src/store/libraryStore.ts::unfavorite(id) -> libraryUnfavorite(id) then refresh/libraryList","web/src/lib/libraryApi.ts::libraryUnfavorite -> invoke('library_unfavorite',{id}); libraryList -> invoke('library_list',{category:undefined})","src-tauri/src/library.rs::library_unfavorite(id) -> remove_from_library_and_project -> crates/opentake-media/src/library.rs::LibraryStore::remove","code:web/src/components/media/LibraryView.tsx#LibraryView","code:web/src/lib/libraryApi.ts#libraryUnfavorite","code:src-tauri/src/library.rs#library_unfavorite","code:crates/opentake-media/src/library.rs#LibraryStore"].
  - Visible/accessibility/return path: success=unfavorite a global Library item: libraryStore.unfavorite -> refresh or visible error; accessibility={"focus":"Custom CardAction focus behavior depends on its implementation","label":"t(\"library.unfavorite\")","shortcut":"None declared on this control"}; returnPath=["The Back Home button returns to Home; card operations remain in Library."].
  - Outcome matrix: {"success":"unfavorite a global Library item: libraryStore.unfavorite -> refresh or visible error","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/LibraryView.tsx:527; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/LibraryView.tsx:527; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in libraryStore.unfavorite -> refresh or visible error.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/LibraryView.tsx:527; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/LibraryView.interaction.test.tsx#control-31b29012d24167aa unfavorite a global Library item` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-31b29012d24167aa unfavorite a global Library item"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/LibraryView.tsx`, `web/src/store/libraryStore.ts#unfavorite`, `web/src/lib/libraryApi.ts#libraryUnfavorite`, `src-tauri/src/library.rs#library_unfavorite`, `crates/opentake-media/src/library.rs#LibraryStore::remove`, `web/src/components/media/LibraryView.tsx#LibraryView`, `crates/opentake-media/src/library.rs#LibraryStore` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/LibraryView.interaction.test.tsx -t "control-31b29012d24167aa unfavorite a global Library item"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 17: control-acceptance (implementation-slice-a3cd1a10c72f6c61)

**Covered records:**
- `control-record-8715ffdcc0c6bf4d` (control)
- `control-record-8e7f994437742691` (control)
- `control-record-ee3deaa6f3d04478` (control)
- `control-record-c5270023f35f4983` (control)
- `control-record-1b8b22b916e31853` (control)
- `control-record-3ffb001e65d7176d` (control)
- `control-record-b4c69c3349f57d71` (control)
- `control-record-26ed66aad50e4668` (control)

**Files:**
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaPanel`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-74cf95e67ddd9208 AI media generation placeholder`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-b2ddee9974c7b88f change media view mode`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-5409108a04e7cf1c sort media`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-874419d78bdb9c98 filter media`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-fee61dd27bb05b04 jump to an ancestor media folder`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-9ddccdace2f550dc navigate to the parent media folder`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-d2d6cba7e151c478 open or close the media import menu`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-4bb077c70718eeda open a media folder`

**Candidate-bound contracts:**

#### control-record-8715ffdcc0c6bf4d

- Candidate/source: `control-74cf95e67ddd9208` at `web/src/components/media/MediaPanel.tsx:301:11` (control)
- Expected behavior: AI media generation placeholder: none; the button is permanently disabled
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-74cf95e67ddd9208.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-74cf95e67ddd9208 AI media generation placeholder.
  - Initial state: visibility=Always visible in Material/Audio tabs; enabledWhen=never.
  - Event: inputs=["event/prop handler: No handler declared","click or native keyboard activation plus current owning state"]; handler=No handler declared.
  - Exact call/state/backend: stateTransition=none; the button is permanently disabled; backendTrace=["web/src/components/media/MediaPanel.tsx:301::candidate handler -> No handler declared","actual branch/state -> none; the button is permanently disabled","exact call -> none; the button is permanently disabled","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/MediaPanel.tsx#MediaPanel"].
  - Visible/accessibility/return path: success=AI media generation placeholder: none; the button is permanently disabled; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"media.generateSoon\")","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"AI media generation placeholder: none; the button is permanently disabled","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:301; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when true.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-8e7f994437742691

- Candidate/source: `control-b2ddee9974c7b88f` at `web/src/components/media/MediaPanel.tsx:349:11` (control)
- Expected behavior: change media view mode: none
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-b2ddee9974c7b88f.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-b2ddee9974c7b88f change media view mode.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: No handler declared","click or native keyboard activation plus current owning state"]; handler=No handler declared.
  - Exact call/state/backend: stateTransition=none; backendTrace=["web/src/components/media/MediaPanel.tsx:349::candidate handler -> No handler declared","actual branch/state -> none","exact call -> none","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/MediaPanel.tsx#MediaPanel"].
  - Visible/accessibility/return path: success=change media view mode: none; accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"media.viewMode\")","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"change media view mode: none","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:349; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-ee3deaa6f3d04478

- Candidate/source: `control-5409108a04e7cf1c` at `web/src/components/media/MediaPanel.tsx:352:11` (control)
- Expected behavior: sort media: none
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-5409108a04e7cf1c.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-5409108a04e7cf1c sort media.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: No handler declared","click or native keyboard activation plus current owning state"]; handler=No handler declared.
  - Exact call/state/backend: stateTransition=none; backendTrace=["web/src/components/media/MediaPanel.tsx:352::candidate handler -> No handler declared","actual branch/state -> none","exact call -> none","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/MediaPanel.tsx#MediaPanel"].
  - Visible/accessibility/return path: success=sort media: none; accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"media.sort\")","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"sort media: none","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:352; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-c5270023f35f4983

- Candidate/source: `control-874419d78bdb9c98` at `web/src/components/media/MediaPanel.tsx:355:11` (control)
- Expected behavior: filter media: none
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-874419d78bdb9c98.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-874419d78bdb9c98 filter media.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: No handler declared","click or native keyboard activation plus current owning state"]; handler=No handler declared.
  - Exact call/state/backend: stateTransition=none; backendTrace=["web/src/components/media/MediaPanel.tsx:355::candidate handler -> No handler declared","actual branch/state -> none","exact call -> none","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/MediaPanel.tsx#MediaPanel"].
  - Visible/accessibility/return path: success=filter media: none; accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"media.filter\")","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"filter media: none","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:355; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-1b8b22b916e31853

- Candidate/source: `control-fee61dd27bb05b04` at `web/src/components/media/MediaPanel.tsx:475:7` (control)
- Expected behavior: jump to an ancestor media folder: onNavigate(target) updates mediaPanelCurrentFolderId
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-fee61dd27bb05b04.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-fee61dd27bb05b04 jump to an ancestor media folder.
  - Initial state: visibility=Only for non-current breadcrumb segments; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => onNavigate(target)}","click or native keyboard activation plus current owning state"]; handler={() => onNavigate(target)}.
  - Exact call/state/backend: stateTransition=onNavigate(target) updates mediaPanelCurrentFolderId; backendTrace=["web/src/components/media/MediaPanel.tsx:475::candidate handler -> {() => onNavigate(target)}","actual branch/state -> onNavigate(target) updates mediaPanelCurrentFolderId","exact call -> onNavigate(target) updates mediaPanelCurrentFolderId","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/MediaPanel.tsx#MediaPanel"].
  - Visible/accessibility/return path: success=jump to an ancestor media folder: onNavigate(target) updates mediaPanelCurrentFolderId; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"jump to an ancestor media folder: onNavigate(target) updates mediaPanelCurrentFolderId","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:475; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-3ffb001e65d7176d

- Candidate/source: `control-9ddccdace2f550dc` at `web/src/components/media/MediaPanel.tsx:507:9` (control)
- Expected behavior: navigate to the parent media folder: onNavigate(parentId)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-9ddccdace2f550dc.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-9ddccdace2f550dc navigate to the parent media folder.
  - Initial state: visibility=Only below media root; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => onNavigate(parentId)}","click or native keyboard activation plus current owning state"]; handler={() => onNavigate(parentId)}.
  - Exact call/state/backend: stateTransition=onNavigate(parentId); backendTrace=["web/src/components/media/MediaPanel.tsx:507::candidate handler -> {() => onNavigate(parentId)}","actual branch/state -> onNavigate(parentId)","exact call -> onNavigate(parentId)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/MediaPanel.tsx#MediaPanel"].
  - Visible/accessibility/return path: success=navigate to the parent media folder: onNavigate(parentId); accessibility={"focus":"Native keyboard-focusable control","label":"t(\"media.folderBack\")","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"navigate to the parent media folder: onNavigate(parentId)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:507; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-b4c69c3349f57d71

- Candidate/source: `control-d2d6cba7e151c478` at `web/src/components/media/MediaPanel.tsx:571:7` (control)
- Expected behavior: open or close the media import menu: setOpen toggles menu; outside mousedown closes
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d2d6cba7e151c478.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-d2d6cba7e151c478 open or close the media import menu.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setOpen((v) => !v)}","click or native keyboard activation plus current owning state"]; handler={() => setOpen((v) => !v)}.
  - Exact call/state/backend: stateTransition=setOpen toggles menu; outside mousedown closes; backendTrace=["web/src/components/media/MediaPanel.tsx:571::candidate handler -> {() => setOpen((v) => !v)}","actual branch/state -> setOpen toggles menu; outside mousedown closes","exact call -> setOpen toggles menu; outside mousedown closes","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/MediaPanel.tsx#MediaPanel"].
  - Visible/accessibility/return path: success=open or close the media import menu: setOpen toggles menu; outside mousedown closes; accessibility={"focus":"Custom HoverButton focus behavior depends on its implementation","label":"t(\"media.importHint\")","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"open or close the media import menu: setOpen toggles menu; outside mousedown closes","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:571; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setOpen toggles menu; outside mousedown closes; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-26ed66aad50e4668

- Candidate/source: `control-4bb077c70718eeda` at `web/src/components/media/MediaPanel.tsx:724:5` (control)
- Expected behavior: open a media folder: double-click/Enter/Space calls onOpen(folder.id)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-4bb077c70718eeda.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-4bb077c70718eeda open a media folder.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => onOpen(folder.id)} {() => setHovered(true)} {() => setHovered(false)} {(e) => { if (e.key === \"Enter\" || e.key === \" \") { e.preventDefault(); onOpen(folder.id); } }}","pointer/drag coordinates, button/key, and current owning state"]; handler={() => onOpen(folder.id)} {() => setHovered(true)} {() => setHovered(false)} {(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onOpen(folder.id); } }}.
  - Exact call/state/backend: stateTransition=double-click/Enter/Space calls onOpen(folder.id); backendTrace=["web/src/components/media/MediaPanel.tsx:724::candidate handler -> {() => onOpen(folder.id)} {() => setHovered(true)} {() => setHovered(false)} {(e) => { if (e.key === \"Enter\" || e.key === \" \") { e.preventDefault(); onOpen(folder.id); } }}","actual branch/state -> double-click/Enter/Space calls onOpen(folder.id)","exact call -> double-click/Enter/Space calls onOpen(folder.id)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/MediaPanel.tsx#MediaPanel"].
  - Visible/accessibility/return path: success=open a media folder: double-click/Enter/Space calls onOpen(folder.id); accessibility={"focus":"role=button with tabIndex=0 and Enter/Space support","label":"folder.name","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"open a media folder: double-click/Enter/Space calls onOpen(folder.id)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:724; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in double-click/Enter/Space calls onOpen(folder.id); no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-74cf95e67ddd9208 AI media generation placeholder` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-b2ddee9974c7b88f change media view mode` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-5409108a04e7cf1c sort media` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-874419d78bdb9c98 filter media` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-fee61dd27bb05b04 jump to an ancestor media folder` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-9ddccdace2f550dc navigate to the parent media folder` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-d2d6cba7e151c478 open or close the media import menu` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-4bb077c70718eeda open a media folder` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-74cf95e67ddd9208 AI media generation placeholder"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-b2ddee9974c7b88f change media view mode"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-5409108a04e7cf1c sort media"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-874419d78bdb9c98 filter media"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-fee61dd27bb05b04 jump to an ancestor media folder"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-9ddccdace2f550dc navigate to the parent media folder"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-d2d6cba7e151c478 open or close the media import menu"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-4bb077c70718eeda open a media folder"`

  Expected: FAIL because one or more of the 8 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaPanel.tsx`, `web/src/components/media/MediaPanel.tsx#MediaPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-74cf95e67ddd9208 AI media generation placeholder"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-b2ddee9974c7b88f change media view mode"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-5409108a04e7cf1c sort media"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-874419d78bdb9c98 filter media"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-fee61dd27bb05b04 jump to an ancestor media folder"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-9ddccdace2f550dc navigate to the parent media folder"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-d2d6cba7e151c478 open or close the media import menu"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-4bb077c70718eeda open a media folder"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 18: control-acceptance (implementation-slice-6b3fff9e495385aa)

**Covered records:**
- `control-record-ebdf687445b04cc2` (control)

**Files:**
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/components/media/MediaPanel.tsx#search`
- Modify: `web/src/components/media/MediaSearch.tsx#MediaSearchResults`
- Modify: `web/src/lib/api.ts#searchQuery`
- Modify: `src-tauri/src/search.rs#search_query`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaPanel`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-fbc0598258d77d6a search project/global media`

**Candidate-bound contracts:**

#### control-record-ebdf687445b04cc2

- Candidate/source: `control-fbc0598258d77d6a` at `web/src/components/media/MediaPanel.tsx:334:11` (control)
- Expected behavior: search project/global media: setSearch updates the local query immediately; non-empty query mounts MediaSearchResults, whose 250ms effect calls searchQueryApi(query) while filename filtering remains local
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-fbc0598258d77d6a.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-fbc0598258d77d6a search project/global media.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => setSearch(e.target.value)}","current control value and deterministic replacement value"]; handler={(e) => setSearch(e.target.value)}.
  - Exact call/state/backend: stateTransition=setSearch updates the local query immediately; non-empty query mounts MediaSearchResults, whose 250ms effect calls searchQueryApi(query) while filename filtering remains local; backendTrace=["web/src/components/media/MediaPanel.tsx:334::candidate handler -> {(e) => setSearch(e.target.value)}","actual branch/state -> setSearch updates the local query immediately; non-empty query mounts MediaSearchResults, whose 250ms effect calls searchQueryApi(query) while filename filtering remains local","exact call/arguments -> setSearch(event.target.value); when trimmed query is non-empty MediaSearchResults waits 250ms and calls searchQueryApi(trimmedQuery)","web/src/components/media/MediaPanel.tsx::search input -> setSearch(value) -> mounts MediaSearchResults(query)","web/src/components/media/MediaSearch.tsx::MediaSearchResults effect -> setTimeout(250ms) -> searchQueryApi(q)","web/src/lib/api.ts::searchQuery -> invoke('search_query',{query:q})","src-tauri/src/search.rs::search_query(query) -> moments/spoken/files result","code:web/src/components/media/MediaPanel.tsx#MediaPanel","code:web/src/components/media/MediaSearch.tsx#MediaSearchResults","code:web/src/lib/api.ts#searchQuery","code:src-tauri/src/search.rs#search_query"].
  - Visible/accessibility/return path: success=search project/global media: setSearch updates the local query immediately; non-empty query mounts MediaSearchResults, whose 250ms effect calls searchQueryApi(query) while filename filtering remains local; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"search project/global media: setSearch updates the local query immediately; non-empty query mounts MediaSearchResults, whose 250ms effect calls searchQueryApi(query) while filename filtering remains local","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaPanel.tsx:334; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:334; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in setSearch updates the local query immediately; non-empty query mounts MediaSearchResults, whose 250ms effect calls searchQueryApi(query) while filename filtering remains local.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/MediaPanel.tsx:334; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-fbc0598258d77d6a search project/global media` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-fbc0598258d77d6a search project/global media"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaPanel.tsx`, `web/src/components/media/MediaPanel.tsx#search`, `web/src/components/media/MediaSearch.tsx#MediaSearchResults`, `web/src/lib/api.ts#searchQuery`, `src-tauri/src/search.rs#search_query`, `web/src/components/media/MediaPanel.tsx#MediaPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-fbc0598258d77d6a search project/global media"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 19: control-acceptance (implementation-slice-20cdf147a199fca5)

**Covered records:**
- `control-record-e8b303d35becc46e` (control)

**Files:**
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/store/mediaActions.ts#importFolderViaDialog`
- Modify: `web/src/lib/api.ts#importFolder`
- Modify: `src-tauri/src/media.rs#import_folder`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaPanel`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-4f239be002df8094 import a folder`

**Candidate-bound contracts:**

#### control-record-e8b303d35becc46e

- Candidate/source: `control-4f239be002df8094` at `web/src/components/media/MediaPanel.tsx:590:11` (control)
- Expected behavior: import a folder: closes menu then importFolderViaDialog
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-4f239be002df8094.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-4f239be002df8094 import a folder.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => { setOpen(false); void importFolderViaDialog(); }}","click or native keyboard activation plus current owning state"]; handler={() => { setOpen(false); void importFolderViaDialog(); }}.
  - Exact call/state/backend: stateTransition=closes menu then importFolderViaDialog; backendTrace=["web/src/components/media/MediaPanel.tsx:590::candidate handler -> {() => { setOpen(false); void importFolderViaDialog(); }}","actual branch/state -> closes menu then importFolderViaDialog","exact call/arguments -> importFolderViaDialog(); open({directory:true,multiple:false,defaultPath}); then api.importFolder(selected,true) and refreshMedia()","web/src/store/mediaActions.ts::importFolderViaDialog -> openDialog/open exact directory options -> api.importFolder(selected,true)","web/src/lib/api.ts::importFolder -> invoke('import_folder',{path:selected,recursive:true})","src-tauri/src/media.rs::import_folder(path,recursive) -> import_folder_impl; then frontend refreshMedia","code:web/src/components/media/MediaPanel.tsx#MediaPanel","code:web/src/store/mediaActions.ts#importFolderViaDialog","code:web/src/lib/api.ts#importFolder","code:src-tauri/src/media.rs#import_folder"].
  - Visible/accessibility/return path: success=import a folder: closes menu then importFolderViaDialog; accessibility={"focus":"Custom ImportMenuItem focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"import a folder: closes menu then importFolderViaDialog","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaPanel.tsx:590; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in closes menu then importFolderViaDialog; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in closes menu then importFolderViaDialog.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/MediaPanel.tsx:590; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-4f239be002df8094 import a folder` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-4f239be002df8094 import a folder"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaPanel.tsx`, `web/src/store/mediaActions.ts#importFolderViaDialog`, `web/src/lib/api.ts#importFolder`, `src-tauri/src/media.rs#import_folder`, `web/src/components/media/MediaPanel.tsx#MediaPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-4f239be002df8094 import a folder"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 20: control-acceptance (implementation-slice-e13958033858e322)

**Covered records:**
- `control-record-d779160a7a65d199` (control)

**Files:**
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/store/mediaActions.ts#importFilesViaDialog`
- Modify: `web/src/lib/api.ts#importMedia`
- Modify: `src-tauri/src/media.rs#import_media`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaPanel`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-35bf776459532f2f import media files`

**Candidate-bound contracts:**

#### control-record-d779160a7a65d199

- Candidate/source: `control-35bf776459532f2f` at `web/src/components/media/MediaPanel.tsx:598:11` (control)
- Expected behavior: import media files: closes menu then importFilesViaDialog
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-35bf776459532f2f.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-35bf776459532f2f import media files.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => { setOpen(false); void importFilesViaDialog(); }}","click or native keyboard activation plus current owning state"]; handler={() => { setOpen(false); void importFilesViaDialog(); }}.
  - Exact call/state/backend: stateTransition=closes menu then importFilesViaDialog; backendTrace=["web/src/components/media/MediaPanel.tsx:598::candidate handler -> {() => { setOpen(false); void importFilesViaDialog(); }}","actual branch/state -> closes menu then importFilesViaDialog","exact call/arguments -> importFilesViaDialog(); open({directory:false,multiple:true,defaultPath,filters}); normalize selected to paths[]; api.importMedia(paths); refreshMedia()","web/src/store/mediaActions.ts::importFilesViaDialog -> openDialog/open exact file options -> api.importMedia(paths)","web/src/lib/api.ts::importMedia -> invoke('import_media',{paths})","src-tauri/src/media.rs::import_media(paths) -> import_media_impl; then frontend refreshMedia","code:web/src/components/media/MediaPanel.tsx#MediaPanel","code:web/src/store/mediaActions.ts#importFilesViaDialog","code:web/src/lib/api.ts#importMedia","code:src-tauri/src/media.rs#import_media"].
  - Visible/accessibility/return path: success=import media files: closes menu then importFilesViaDialog; accessibility={"focus":"Custom ImportMenuItem focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"import media files: closes menu then importFilesViaDialog","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaPanel.tsx:598; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:598; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in closes menu then importFilesViaDialog; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in closes menu then importFilesViaDialog.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/MediaPanel.tsx:598; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-35bf776459532f2f import media files` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-35bf776459532f2f import media files"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaPanel.tsx`, `web/src/store/mediaActions.ts#importFilesViaDialog`, `web/src/lib/api.ts#importMedia`, `src-tauri/src/media.rs#import_media`, `web/src/components/media/MediaPanel.tsx#MediaPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-35bf776459532f2f import media files"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 21: control-acceptance (implementation-slice-8fe6afd1d642828e)

**Covered records:**
- `control-record-22452d05287cdd20` (control)

**Files:**
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaCard`
- Modify: `web/src/lib/api.ts#preloadMedia`
- Modify: `src-tauri/src/media.rs#preload_media`
- Modify: `web/src/store/editActions.ts#addMediaToTimeline`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaPanel`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-56870ea51c57d201 preview, drag, or add a media card`

**Candidate-bound contracts:**

#### control-record-22452d05287cdd20

- Candidate/source: `control-56870ea51c57d201` at `web/src/components/media/MediaPanel.tsx:888:5` (control)
- Expected behavior: preview, drag, or add a media card: click selects preview/preloads; double-click adds; drag publishes media payload
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-56870ea51c57d201.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-56870ea51c57d201 preview, drag, or add a media card.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); // Warm poster/sprite/waveform caches so preview + a later timeline drop // are instant instead of decoding on the interaction path. void preloadMedia(item.id); }} {() => void addMediaToTimeline(item)} {() => setHovered(true)} {() => setHovered(false)}","pointer/drag coordinates, button/key, and current owning state"]; handler={onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); // Warm poster/sprite/waveform caches so preview + a later timeline drop // are instant instead of decoding on the interaction path. void preloadMedia(item.id); }} {() => void addMediaToTimeline(item)} {() => setHovered(true)} {() => setHovered(false)}.
  - Exact call/state/backend: stateTransition=click selects preview/preloads; double-click adds; drag publishes media payload; backendTrace=["web/src/components/media/MediaPanel.tsx:888::candidate handler -> {onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); // Warm poster/sprite/waveform caches so preview + a later timeline drop // are instant instead of decoding on the interaction path. void preloadMedia(item.id); }} {() => void addMediaToTimeline(item)} {() => setHovered(true)} {() => setHovered(false)}","actual branch/state -> click selects preview/preloads; double-click adds; drag publishes media payload","exact call/arguments -> click/drag calls preloadMedia(item.id); double-click calls addMediaToTimeline(item), which may insertTrack then addClips using that MediaItem","web/src/components/media/MediaPanel.tsx::MediaCard click/drag -> setPreviewMedia/setDraggingMedia and preloadMedia(item.id); double-click -> addMediaToTimeline(item)","web/src/lib/api.ts::preloadMedia -> invoke('preload_media',{mediaRef:item.id}) -> src-tauri/src/media.rs::preload_media","web/src/store/editActions.ts::addMediaToTimeline -> insertTrack when needed, then addClips([{mediaRef:item.id,...}])","web/src/lib/api.ts::editApply -> invoke('edit_apply',{command}); src-tauri/src/commands.rs::edit_apply -> crates/opentake-ops/src/command.rs::EditCommand","code:web/src/components/media/MediaPanel.tsx#MediaPanel","code:web/src/components/media/MediaPanel.tsx#MediaCard","code:web/src/lib/api.ts#preloadMedia","code:src-tauri/src/media.rs#preload_media","code:web/src/store/editActions.ts#addMediaToTimeline","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=preview, drag, or add a media card: click selects preview/preloads; double-click adds; drag publishes media payload; accessibility={"focus":"Draggable div has no role/tabIndex/keyboard activation","label":"item.name","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"preview, drag, or add a media card: click selects preview/preloads; double-click adds; drag publishes media payload","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaPanel.tsx:888; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:888; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in click selects preview/preloads; double-click adds; drag publishes media payload.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/MediaPanel.tsx:888; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-56870ea51c57d201 preview, drag, or add a media card` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-56870ea51c57d201 preview, drag, or add a media card"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaPanel.tsx`, `web/src/components/media/MediaPanel.tsx#MediaCard`, `web/src/lib/api.ts#preloadMedia`, `src-tauri/src/media.rs#preload_media`, `web/src/store/editActions.ts#addMediaToTimeline`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand`, `web/src/components/media/MediaPanel.tsx#MediaPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-56870ea51c57d201 preview, drag, or add a media card"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 22: control-acceptance (implementation-slice-6753641d93d35334)

**Covered records:**
- `control-record-0be569405c5bf0e2` (control)

**Files:**
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/store/mediaActions.ts#relinkMediaViaDialog`
- Modify: `web/src/lib/api.ts#relinkMedia`
- Modify: `src-tauri/src/media.rs#relink_media`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaPanel`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-23e08673fb5d2f66 relink an offline media asset`

**Candidate-bound contracts:**

#### control-record-0be569405c5bf0e2

- Candidate/source: `control-23e08673fb5d2f66` at `web/src/components/media/MediaPanel.tsx:980:13` (control)
- Expected behavior: relink an offline media asset: relinkMediaViaDialog preserves asset id and refreshes media
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-23e08673fb5d2f66.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-23e08673fb5d2f66 relink an offline media asset.
  - Initial state: visibility=Only for missing media; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => { e.stopPropagation(); void relinkMediaViaDialog(item.id); }}","click or native keyboard activation plus current owning state"]; handler={(e) => { e.stopPropagation(); void relinkMediaViaDialog(item.id); }}.
  - Exact call/state/backend: stateTransition=relinkMediaViaDialog preserves asset id and refreshes media; backendTrace=["web/src/components/media/MediaPanel.tsx:980::candidate handler -> {(e) => { e.stopPropagation(); void relinkMediaViaDialog(item.id); }}","actual branch/state -> relinkMediaViaDialog preserves asset id and refreshes media","exact call/arguments -> relinkMediaViaDialog(item.id); open({directory:false,multiple:false,defaultPath,Media filters}); then relinkMedia(item.id,selected) and refreshMedia()","web/src/store/mediaActions.ts::relinkMediaViaDialog(mediaRef) -> openDialog/open -> api.relinkMedia(mediaRef,selected)","web/src/lib/api.ts::relinkMedia -> invoke('relink_media',{mediaRef,newPath:selected})","src-tauri/src/media.rs::relink_media(media_ref,new_path) -> preserve asset id/update source; then frontend refreshMedia","code:web/src/components/media/MediaPanel.tsx#MediaPanel","code:web/src/store/mediaActions.ts#relinkMediaViaDialog","code:web/src/lib/api.ts#relinkMedia","code:src-tauri/src/media.rs#relink_media"].
  - Visible/accessibility/return path: success=relink an offline media asset: relinkMediaViaDialog preserves asset id and refreshes media; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"relink an offline media asset: relinkMediaViaDialog preserves asset id and refreshes media","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaPanel.tsx:980; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:980; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in relinkMediaViaDialog preserves asset id and refreshes media; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in relinkMediaViaDialog preserves asset id and refreshes media.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/MediaPanel.tsx:980; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-23e08673fb5d2f66 relink an offline media asset` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-23e08673fb5d2f66 relink an offline media asset"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaPanel.tsx`, `web/src/store/mediaActions.ts#relinkMediaViaDialog`, `web/src/lib/api.ts#relinkMedia`, `src-tauri/src/media.rs#relink_media`, `web/src/components/media/MediaPanel.tsx#MediaPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-23e08673fb5d2f66 relink an offline media asset"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 23: control-acceptance (implementation-slice-e8afd14f98f6a3eb)

**Covered records:**
- `control-record-04d1f99ed84466ee` (control)

**Files:**
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/components/media/MediaPanel.tsx#onExtractAudio`
- Modify: `web/src/lib/api.ts#extractAudio`
- Modify: `src-tauri/src/media.rs#extract_audio`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaPanel`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-421046c542a4e315 extract audio from a video`

**Candidate-bound contracts:**

#### control-record-04d1f99ed84466ee

- Candidate/source: `control-421046c542a4e315` at `web/src/components/media/MediaPanel.tsx:1017:11` (control)
- Expected behavior: extract audio from a video: native save -> extractAudio -> transient success/failure feedback
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-421046c542a4e315.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-421046c542a4e315 extract audio from a video.
  - Initial state: visibility=Only on hover for present video with audio; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onExtractAudio}","click or native keyboard activation plus current owning state"]; handler={onExtractAudio}.
  - Exact call/state/backend: stateTransition=native save -> extractAudio -> transient success/failure feedback; backendTrace=["web/src/components/media/MediaPanel.tsx:1017::candidate handler -> {onExtractAudio}","actual branch/state -> native save -> extractAudio -> transient success/failure feedback","exact call/arguments -> after saveDialog returns chosen path, extractAudio(item.id, chosen)","web/src/components/media/MediaPanel.tsx::onExtractAudio -> saveDialog exact audio filters -> extractAudio(item.id,chosen)","web/src/lib/api.ts::extractAudio -> invoke('extract_audio',{mediaId:item.id,outPath:chosen})","src-tauri/src/media.rs::extract_audio(media_id,out_path) -> ffmpeg extraction","code:web/src/components/media/MediaPanel.tsx#MediaPanel","code:web/src/components/media/MediaPanel.tsx#onExtractAudio","code:web/src/lib/api.ts#extractAudio","code:src-tauri/src/media.rs#extract_audio"].
  - Visible/accessibility/return path: success=extract audio from a video: native save -> extractAudio -> transient success/failure feedback; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"media.extractAudio\")","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"extract audio from a video: native save -> extractAudio -> transient success/failure feedback","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaPanel.tsx:1017; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in native save -> extractAudio -> transient success/failure feedback.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/MediaPanel.tsx:1017; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-421046c542a4e315 extract audio from a video` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-421046c542a4e315 extract audio from a video"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaPanel.tsx`, `web/src/components/media/MediaPanel.tsx#onExtractAudio`, `web/src/lib/api.ts#extractAudio`, `src-tauri/src/media.rs#extract_audio`, `web/src/components/media/MediaPanel.tsx#MediaPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-421046c542a4e315 extract audio from a video"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 24: control-acceptance (implementation-slice-c539058642a37b12)

**Covered records:**
- `control-record-bff8d59026f59694` (control)

**Files:**
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaFavoriteButton`
- Modify: `web/src/lib/api.ts#toggleFavorite`
- Modify: `src-tauri/src/media.rs#toggle_favorite`
- Modify: `crates/opentake-media/src/library.rs`
- Modify: `web/src/components/media/MediaPanel.tsx#MediaPanel`
- Test (reviewed-planned): `web/src/components/media/MediaPanel.interaction.test.tsx#control-12917008f22749ee toggle durable media favorite`

**Candidate-bound contracts:**

#### control-record-bff8d59026f59694

- Candidate/source: `control-12917008f22749ee` at `web/src/components/media/MediaPanel.tsx:1098:5` (control)
- Expected behavior: toggle durable media favorite: pending disables/aria-busy; current-project success refreshes mirrors; failure preserves star and alerts
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-12917008f22749ee.
  - Test: web/src/components/media/MediaPanel.interaction.test.tsx#control-12917008f22749ee toggle durable media favorite.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({pending}).
  - Event: inputs=["event/prop handler: {(event) => { event.stopPropagation(); const project = captureMediaProjectIdentity(); setPending(true); onStart?.(); void performToggle(assetId, !favorite, project) .then((media) => { if (!isCurrentMediaProject(project)) return; return onSuccess(media, project); }) .catch((error: unknown) => { if (!isCurrentMediaProject(project)) return; onError(String(error), project); }) .finally(() => setPending(false)); }}","click or native keyboard activation plus current owning state"]; handler={(event) => { event.stopPropagation(); const project = captureMediaProjectIdentity(); setPending(true); onStart?.(); void performToggle(assetId, !favorite, project) .then((media) => { if (!isCurrentMediaProject(project)) return; return onSuccess(media, project); }) .catch((error: unknown) => { if (!isCurrentMediaProject(project)) return; onError(String(error), project); }) .finally(() => setPending(false)); }}.
  - Exact call/state/backend: stateTransition=pending disables/aria-busy; current-project success refreshes mirrors; failure preserves star and alerts; backendTrace=["web/src/components/media/MediaPanel.tsx:1098::candidate handler -> {(event) => { event.stopPropagation(); const project = captureMediaProjectIdentity(); setPending(true); onStart?.(); void performToggle(assetId, !favorite, project) .then((media) => { if (!isCurrentMediaProject(project)) return; return onSuccess(media, project); }) .catch((error: unknown) => { if (!isCurrentMediaProject(project)) return; onError(String(error), project); }) .finally(() => setPending(false)); }}","actual branch/state -> pending disables/aria-busy; current-project success refreshes mirrors; failure preserves star and alerts","exact call/arguments -> performToggle(assetId, !favorite, capturedProjectIdentity); only current-project success calls onSuccess(media,project), rejection calls onError(String(error),project)","web/src/components/media/MediaPanel.tsx::MediaFavoriteButton -> performToggle(assetId,!favorite,project)","web/src/lib/api.ts::toggleFavorite -> invoke('toggle_favorite',{assetId,favorite:!favorite,expectedProjectEpoch,expectedProjectPath})","src-tauri/src/media.rs::toggle_favorite -> toggle_favorite_impl_for_project -> crates/opentake-media/src/library.rs durable library transaction","code:web/src/components/media/MediaPanel.tsx#MediaPanel","code:web/src/components/media/MediaPanel.tsx#MediaFavoriteButton","code:web/src/lib/api.ts#toggleFavorite","code:src-tauri/src/media.rs#toggle_favorite"].
  - Visible/accessibility/return path: success=toggle durable media favorite: pending disables/aria-busy; current-project success refreshes mirrors; failure preserves star and alerts; accessibility={"focus":"Native button; title/aria-label, aria-pressed, aria-busy and disabled are present","label":"title","shortcut":"None declared on this control"}; returnPath=["Media actions remain in the panel; native dialogs return focus to the invoking panel in platform behavior but this is not asserted."].
  - Outcome matrix: {"success":"toggle durable media favorite: pending disables/aria-busy; current-project success refreshes mirrors; failure preserves star and alerts","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaPanel.tsx:1098; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaPanel.tsx:1098; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {pending}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in pending disables/aria-busy; current-project success refreshes mirrors; failure preserves star and alerts.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/MediaPanel.tsx:1098; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaPanel.interaction.test.tsx#control-12917008f22749ee toggle durable media favorite` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-12917008f22749ee toggle durable media favorite"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaPanel.tsx`, `web/src/components/media/MediaPanel.tsx#MediaFavoriteButton`, `web/src/lib/api.ts#toggleFavorite`, `src-tauri/src/media.rs#toggle_favorite`, `crates/opentake-media/src/library.rs`, `web/src/components/media/MediaPanel.tsx#MediaPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaPanel.interaction.test.tsx -t "control-12917008f22749ee toggle durable media favorite"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 25: control-acceptance (implementation-slice-41297336707b24b5)

**Covered records:**
- `control-record-8530a295a682b8d5` (control)
- `control-record-6c5d3a5215957010` (control)

**Files:**
- Modify: `web/src/components/media/MediaSearch.tsx`
- Modify: `web/src/components/media/MediaSearch.tsx#onDownload`
- Modify: `web/src/lib/api.ts#downloadSearchModel`
- Modify: `src-tauri/src/search.rs#download_search_model`
- Modify: `web/src/components/media/MediaSearch.tsx#MediaSearchResults`
- Test (reviewed-planned): `web/src/components/media/MediaSearch.interaction.test.tsx#control-eff9191121500a17 download the semantic-search model`
- Test (reviewed-planned): `web/src/components/media/MediaSearch.interaction.test.tsx#control-068418d6a1a00a52 retry semantic-search model download`

**Candidate-bound contracts:**

#### control-record-8530a295a682b8d5

- Candidate/source: `control-eff9191121500a17` at `web/src/components/media/MediaSearch.tsx:260:7` (control)
- Expected behavior: download the semantic-search model: needsModel/failed -> downloading -> readyToIndex or failed
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-eff9191121500a17.
  - Test: web/src/components/media/MediaSearch.interaction.test.tsx#control-eff9191121500a17 download the semantic-search model.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onDownload}","click or native keyboard activation plus current owning state"]; handler={onDownload}.
  - Exact call/state/backend: stateTransition=needsModel/failed -> downloading -> readyToIndex or failed; backendTrace=["web/src/components/media/MediaSearch.tsx:260::candidate handler -> {onDownload}","actual branch/state -> needsModel/failed -> downloading -> readyToIndex or failed","exact call/arguments -> downloadSearchModel(); success sets readyToIndex, rejection sets failed; progress comes from search://progress","web/src/components/media/MediaSearch.tsx::onDownload -> downloadSearchModel with phase transitions","web/src/lib/api.ts::downloadSearchModel -> invoke('download_search_model'); onSearchModelProgress listens to search://progress","src-tauri/src/search.rs::download_search_model -> model download/progress","code:web/src/components/media/MediaSearch.tsx#MediaSearchResults","code:web/src/components/media/MediaSearch.tsx#onDownload","code:web/src/lib/api.ts#downloadSearchModel","code:src-tauri/src/search.rs#download_search_model"].
  - Visible/accessibility/return path: success=download the semantic-search model: needsModel/failed -> downloading -> readyToIndex or failed; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"search.smartSearchHint\", { size: formatModelBytes(0) })","shortcut":"None declared on this control"}; returnPath=["Search remains active; download/index progress replaces its initiating button."].
  - Outcome matrix: {"success":"download the semantic-search model: needsModel/failed -> downloading -> readyToIndex or failed","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaSearch.tsx:260; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaSearch.tsx:260; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in needsModel/failed -> downloading -> readyToIndex or failed; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in needsModel/failed -> downloading -> readyToIndex or failed.","failure":"Download failure exposes Retry, but error details are not announced"}.

#### control-record-6c5d3a5215957010

- Candidate/source: `control-068418d6a1a00a52` at `web/src/components/media/MediaSearch.tsx:286:7` (control)
- Expected behavior: retry semantic-search model download: failed -> downloading -> readyToIndex or failed
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-068418d6a1a00a52.
  - Test: web/src/components/media/MediaSearch.interaction.test.tsx#control-068418d6a1a00a52 retry semantic-search model download.
  - Initial state: visibility=Only after model download failure; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onDownload}","click or native keyboard activation plus current owning state"]; handler={onDownload}.
  - Exact call/state/backend: stateTransition=failed -> downloading -> readyToIndex or failed; backendTrace=["web/src/components/media/MediaSearch.tsx:286::candidate handler -> {onDownload}","actual branch/state -> failed -> downloading -> readyToIndex or failed","exact call/arguments -> downloadSearchModel(); success sets readyToIndex, rejection sets failed; progress comes from search://progress","web/src/components/media/MediaSearch.tsx::onDownload -> downloadSearchModel with phase transitions","web/src/lib/api.ts::downloadSearchModel -> invoke('download_search_model'); onSearchModelProgress listens to search://progress","src-tauri/src/search.rs::download_search_model -> model download/progress","code:web/src/components/media/MediaSearch.tsx#MediaSearchResults","code:web/src/components/media/MediaSearch.tsx#onDownload","code:web/src/lib/api.ts#downloadSearchModel","code:src-tauri/src/search.rs#download_search_model"].
  - Visible/accessibility/return path: success=retry semantic-search model download: failed -> downloading -> readyToIndex or failed; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"search.retryHint\")","shortcut":"None declared on this control"}; returnPath=["Search remains active; download/index progress replaces its initiating button."].
  - Outcome matrix: {"success":"retry semantic-search model download: failed -> downloading -> readyToIndex or failed","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaSearch.tsx:286; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaSearch.tsx:286; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in failed -> downloading -> readyToIndex or failed; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in failed -> downloading -> readyToIndex or failed.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/MediaSearch.tsx:286; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaSearch.interaction.test.tsx#control-eff9191121500a17 download the semantic-search model` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/MediaSearch.interaction.test.tsx#control-068418d6a1a00a52 retry semantic-search model download` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-eff9191121500a17 download the semantic-search model"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-068418d6a1a00a52 retry semantic-search model download"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaSearch.tsx`, `web/src/components/media/MediaSearch.tsx#onDownload`, `web/src/lib/api.ts#downloadSearchModel`, `src-tauri/src/search.rs#download_search_model`, `web/src/components/media/MediaSearch.tsx#MediaSearchResults` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-eff9191121500a17 download the semantic-search model"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-068418d6a1a00a52 retry semantic-search model download"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 26: control-acceptance (implementation-slice-791325bbc3148bd0)

**Covered records:**
- `control-record-ab960caf193a5615` (control)

**Files:**
- Modify: `web/src/components/media/MediaSearch.tsx`
- Modify: `web/src/components/media/MediaSearch.tsx#onIndex`
- Modify: `web/src/lib/api.ts#searchIndexStart`
- Modify: `src-tauri/src/search.rs#search_index_start`
- Modify: `web/src/components/media/MediaSearch.tsx#MediaSearchResults`
- Test (reviewed-planned): `web/src/components/media/MediaSearch.interaction.test.tsx#control-494379e44c0f6b9c build the semantic-search index`

**Candidate-bound contracts:**

#### control-record-ab960caf193a5615

- Candidate/source: `control-494379e44c0f6b9c` at `web/src/components/media/MediaSearch.tsx:273:7` (control)
- Expected behavior: build the semantic-search index: readyToIndex -> indexing -> hidden
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-494379e44c0f6b9c.
  - Test: web/src/components/media/MediaSearch.interaction.test.tsx#control-494379e44c0f6b9c build the semantic-search index.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onIndex}","click or native keyboard activation plus current owning state"]; handler={onIndex}.
  - Exact call/state/backend: stateTransition=readyToIndex -> indexing -> hidden; backendTrace=["web/src/components/media/MediaSearch.tsx:273::candidate handler -> {onIndex}","actual branch/state -> readyToIndex -> indexing -> hidden","exact call/arguments -> searchIndexStart(); success and rejection both currently set hidden; progress comes from search://index","web/src/components/media/MediaSearch.tsx::onIndex -> searchIndexStart with indexing/hidden phase transitions","web/src/lib/api.ts::searchIndexStart -> invoke('search_index_start'); onSearchIndexProgress listens to search://index","src-tauri/src/search.rs::search_index_start -> build semantic index/progress","code:web/src/components/media/MediaSearch.tsx#MediaSearchResults","code:web/src/components/media/MediaSearch.tsx#onIndex","code:web/src/lib/api.ts#searchIndexStart","code:src-tauri/src/search.rs#search_index_start"].
  - Visible/accessibility/return path: success=build the semantic-search index: readyToIndex -> indexing -> hidden; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"search.indexHint\")","shortcut":"None declared on this control"}; returnPath=["Search remains active; download/index progress replaces its initiating button."].
  - Outcome matrix: {"success":"build the semantic-search index: readyToIndex -> indexing -> hidden","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaSearch.tsx:273; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaSearch.tsx:273; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in readyToIndex -> indexing -> hidden.","failure":"Index-start rejection is converted to hidden, silently removing retry/error feedback"}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaSearch.interaction.test.tsx#control-494379e44c0f6b9c build the semantic-search index` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-494379e44c0f6b9c build the semantic-search index"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaSearch.tsx`, `web/src/components/media/MediaSearch.tsx#onIndex`, `web/src/lib/api.ts#searchIndexStart`, `src-tauri/src/search.rs#search_index_start`, `web/src/components/media/MediaSearch.tsx#MediaSearchResults` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-494379e44c0f6b9c build the semantic-search index"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 27: control-acceptance (implementation-slice-d8c82fd71075d71c)

**Covered records:**
- `control-record-e59d15721f169b9c` (control)

**Files:**
- Modify: `web/src/components/media/MediaSearch.tsx`
- Modify: `web/src/components/media/MediaSearch.tsx#MomentCard`
- Modify: `web/src/lib/api.ts#preloadMedia`
- Modify: `src-tauri/src/media.rs#preload_media`
- Modify: `web/src/components/media/MediaSearch.tsx#MediaSearchResults`
- Test (reviewed-planned): `web/src/components/media/MediaSearch.interaction.test.tsx#control-4eccac72d0056b24 preview/drag a visual Moment hit`

**Candidate-bound contracts:**

#### control-record-e59d15721f169b9c

- Candidate/source: `control-4eccac72d0056b24` at `web/src/components/media/MediaSearch.tsx:454:5` (control)
- Expected behavior: preview/drag a visual Moment hit: click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus hit range for video or null for image; dragEnd clears media and range
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-4eccac72d0056b24.
  - Test: web/src/components/media/MediaSearch.interaction.test.tsx#control-4eccac72d0056b24 preview/drag a visual Moment hit.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); void preloadMedia(item.id); }}","pointer/drag coordinates, button/key, and current owning state"]; handler={onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); void preloadMedia(item.id); }}.
  - Exact call/state/backend: stateTransition=click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus hit range for video or null for image; dragEnd clears media and range; backendTrace=["web/src/components/media/MediaSearch.tsx:454::candidate handler -> {onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); void preloadMedia(item.id); }}","actual branch/state -> click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus hit range for video or null for image; dragEnd clears media and range","exact call/arguments -> preloadMedia(item.id) only; drag writes MEDIA_DND_TYPE=item.id and for video {startSec:hit.startSec,endSec:hit.endSec}, for image null; no timeline edit occurs","web/src/components/media/MediaSearch.tsx::MomentCard click/drag branch -> setPreviewMedia/setDraggingMedia/setDraggingMomentRange and preloadMedia(item.id)","web/src/lib/api.ts::preloadMedia -> invoke('preload_media',{mediaRef:item.id})","src-tauri/src/media.rs::preload_media(media_ref) -> bounded media prewarm scheduler","timeline edit API/Tauri/Rust -> N/A for this candidate","code:web/src/components/media/MediaSearch.tsx#MediaSearchResults","code:web/src/components/media/MediaSearch.tsx#MomentCard","code:web/src/lib/api.ts#preloadMedia","code:src-tauri/src/media.rs#preload_media"].
  - Visible/accessibility/return path: success=preview/drag a visual Moment hit: click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus hit range for video or null for image; dragEnd clears media and range; accessibility={"focus":"Draggable div is pointer-only","label":"t(\"search.dragToTimeline\")","shortcut":"None declared on this control"}; returnPath=["Search remains active; download/index progress replaces its initiating button."].
  - Outcome matrix: {"success":"preview/drag a visual Moment hit: click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus hit range for video or null for image; dragEnd clears media and range","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaSearch.tsx:454; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus hit range for video or null for image; dragEnd clears media and range.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/MediaSearch.tsx:454; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaSearch.interaction.test.tsx#control-4eccac72d0056b24 preview/drag a visual Moment hit` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-4eccac72d0056b24 preview/drag a visual Moment hit"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaSearch.tsx`, `web/src/components/media/MediaSearch.tsx#MomentCard`, `web/src/lib/api.ts#preloadMedia`, `src-tauri/src/media.rs#preload_media`, `web/src/components/media/MediaSearch.tsx#MediaSearchResults` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-4eccac72d0056b24 preview/drag a visual Moment hit"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 28: control-acceptance (implementation-slice-f461b5a39acefb33)

**Covered records:**
- `control-record-f836fe5e7afdb6d6` (control)

**Files:**
- Modify: `web/src/components/media/MediaSearch.tsx`
- Modify: `web/src/components/media/MediaSearch.tsx#SpokenRow`
- Modify: `web/src/lib/api.ts#preloadMedia`
- Modify: `src-tauri/src/media.rs#preload_media`
- Modify: `web/src/components/media/MediaSearch.tsx#MediaSearchResults`
- Test (reviewed-planned): `web/src/components/media/MediaSearch.interaction.test.tsx#control-fd319d3edab2c00d preview/drag a Spoken hit`

**Candidate-bound contracts:**

#### control-record-f836fe5e7afdb6d6

- Candidate/source: `control-fd319d3edab2c00d` at `web/src/components/media/MediaSearch.tsx:508:5` (control)
- Expected behavior: preview/drag a Spoken hit: click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus {startSec:hit.startSec,endSec:hit.endSec}; dragEnd clears both
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-fd319d3edab2c00d.
  - Test: web/src/components/media/MediaSearch.interaction.test.tsx#control-fd319d3edab2c00d preview/drag a Spoken hit.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); void preloadMedia(item.id); }}","pointer/drag coordinates, button/key, and current owning state"]; handler={onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); void preloadMedia(item.id); }}.
  - Exact call/state/backend: stateTransition=click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus {startSec:hit.startSec,endSec:hit.endSec}; dragEnd clears both; backendTrace=["web/src/components/media/MediaSearch.tsx:508::candidate handler -> {onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); void preloadMedia(item.id); }}","actual branch/state -> click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus {startSec:hit.startSec,endSec:hit.endSec}; dragEnd clears both","exact call/arguments -> preloadMedia(item.id) only; drag writes MEDIA_DND_TYPE=item.id and {startSec:hit.startSec,endSec:hit.endSec}; no timeline edit occurs","web/src/components/media/MediaSearch.tsx::SpokenRow click/drag branch -> setPreviewMedia/setDraggingMedia/setDraggingMomentRange and preloadMedia(item.id)","web/src/lib/api.ts::preloadMedia -> invoke('preload_media',{mediaRef:item.id})","src-tauri/src/media.rs::preload_media(media_ref) -> bounded media prewarm scheduler","timeline edit API/Tauri/Rust -> N/A for this candidate","code:web/src/components/media/MediaSearch.tsx#MediaSearchResults","code:web/src/components/media/MediaSearch.tsx#SpokenRow","code:web/src/lib/api.ts#preloadMedia","code:src-tauri/src/media.rs#preload_media"].
  - Visible/accessibility/return path: success=preview/drag a Spoken hit: click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus {startSec:hit.startSec,endSec:hit.endSec}; dragEnd clears both; accessibility={"focus":"Draggable div is pointer-only","label":"t(\"search.dragToTimeline\")","shortcut":"None declared on this control"}; returnPath=["Search remains active; download/index progress replaces its initiating button."].
  - Outcome matrix: {"success":"preview/drag a Spoken hit: click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus {startSec:hit.startSec,endSec:hit.endSec}; dragEnd clears both","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaSearch.tsx:508; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in click sets previewMedia(item.id) and calls preloadMedia(item.id); drag sets MEDIA_DND_TYPE/item plus {startSec:hit.startSec,endSec:hit.endSec}; dragEnd clears both.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/MediaSearch.tsx:508; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaSearch.interaction.test.tsx#control-fd319d3edab2c00d preview/drag a Spoken hit` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-fd319d3edab2c00d preview/drag a Spoken hit"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaSearch.tsx`, `web/src/components/media/MediaSearch.tsx#SpokenRow`, `web/src/lib/api.ts#preloadMedia`, `src-tauri/src/media.rs#preload_media`, `web/src/components/media/MediaSearch.tsx#MediaSearchResults` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-fd319d3edab2c00d preview/drag a Spoken hit"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 29: control-acceptance (implementation-slice-94f68a92b05a3277)

**Covered records:**
- `control-record-c7a2088600adfca1` (control)

**Files:**
- Modify: `web/src/components/media/MediaSearch.tsx`
- Modify: `web/src/components/media/MediaSearch.tsx#FileCard`
- Modify: `web/src/lib/api.ts#preloadMedia`
- Modify: `src-tauri/src/media.rs#preload_media`
- Modify: `web/src/store/editActions.ts#addMediaToTimeline`
- Modify: `web/src/lib/api.ts#editApply`
- Modify: `src-tauri/src/commands.rs#edit_apply`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `web/src/components/media/MediaSearch.tsx#MediaSearchResults`
- Test (reviewed-planned): `web/src/components/media/MediaSearch.interaction.test.tsx#control-a98e8f89678dd470 preview/drag/double-click a filename hit`

**Candidate-bound contracts:**

#### control-record-c7a2088600adfca1

- Candidate/source: `control-a98e8f89678dd470` at `web/src/components/media/MediaSearch.tsx:573:5` (control)
- Expected behavior: preview/drag/double-click a filename hit: click previews; drag sets whole media; double-click adds to timeline
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-a98e8f89678dd470.
  - Test: web/src/components/media/MediaSearch.interaction.test.tsx#control-a98e8f89678dd470 preview/drag/double-click a filename hit.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); void preloadMedia(item.id); }} {() => void addMediaToTimeline(item)}","pointer/drag coordinates, button/key, and current owning state"]; handler={onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); void preloadMedia(item.id); }} {() => void addMediaToTimeline(item)}.
  - Exact call/state/backend: stateTransition=click previews; drag sets whole media; double-click adds to timeline; backendTrace=["web/src/components/media/MediaSearch.tsx:573::candidate handler -> {onDragStart} {onDragEnd} {() => { setPreviewMedia(item.id); void preloadMedia(item.id); }} {() => void addMediaToTimeline(item)}","actual branch/state -> click previews; drag sets whole media; double-click adds to timeline","exact call/arguments -> click/drag calls preloadMedia(item.id); double-click calls addMediaToTimeline(item); drag range is explicitly cleared for the whole file","web/src/components/media/MediaSearch.tsx::FileCard -> setPreviewMedia/preloadMedia, setDraggingMedia, or addMediaToTimeline(item)","web/src/lib/api.ts::preloadMedia -> invoke('preload_media',{mediaRef:item.id}) -> src-tauri/src/media.rs::preload_media","web/src/store/editActions.ts::addMediaToTimeline -> insertTrack/addClips -> web/src/lib/api.ts::editApply","src-tauri/src/commands.rs::edit_apply -> crates/opentake-ops/src/command.rs::EditCommand command application","code:web/src/components/media/MediaSearch.tsx#MediaSearchResults","code:web/src/components/media/MediaSearch.tsx#FileCard","code:web/src/lib/api.ts#preloadMedia","code:src-tauri/src/media.rs#preload_media","code:web/src/store/editActions.ts#addMediaToTimeline","code:web/src/lib/api.ts#editApply","code:src-tauri/src/commands.rs#edit_apply","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=preview/drag/double-click a filename hit: click previews; drag sets whole media; double-click adds to timeline; accessibility={"focus":"Draggable div is pointer-only","label":"item.name","shortcut":"None declared on this control"}; returnPath=["Search remains active; download/index progress replaces its initiating button."].
  - Outcome matrix: {"success":"preview/drag/double-click a filename hit: click previews; drag sets whole media; double-click adds to timeline","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/MediaSearch.tsx:573; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in click previews; drag sets whole media; double-click adds to timeline.","failure":"search_query and preload/add promise failures have no visible per-control feedback"}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaSearch.interaction.test.tsx#control-a98e8f89678dd470 preview/drag/double-click a filename hit` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-a98e8f89678dd470 preview/drag/double-click a filename hit"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaSearch.tsx`, `web/src/components/media/MediaSearch.tsx#FileCard`, `web/src/lib/api.ts#preloadMedia`, `src-tauri/src/media.rs#preload_media`, `web/src/store/editActions.ts#addMediaToTimeline`, `web/src/lib/api.ts#editApply`, `src-tauri/src/commands.rs#edit_apply`, `crates/opentake-ops/src/command.rs#EditCommand`, `web/src/components/media/MediaSearch.tsx#MediaSearchResults` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaSearch.interaction.test.tsx -t "control-a98e8f89678dd470 preview/drag/double-click a filename hit"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 30: control-acceptance (implementation-slice-0dbdbbc70a3e7331)

**Covered records:**
- `control-record-0f22db178cf0b335` (control)
- `control-record-ad2a244350397822` (control)

**Files:**
- Modify: `web/src/components/media/MediaTabBar.tsx`
- Modify: `web/src/components/media/MediaTabBar.tsx#MediaTabBar`
- Test (reviewed-planned): `web/src/components/media/MediaTabBar.interaction.test.tsx#control-d79545153721a517 select an enabled primary media tab`
- Test (reviewed-planned): `web/src/components/media/MediaTabBar.interaction.test.tsx#control-5e42760fbbe049cc select a media subtab`

**Candidate-bound contracts:**

#### control-record-0f22db178cf0b335

- Candidate/source: `control-d79545153721a517` at `web/src/components/media/MediaTabBar.tsx:67:11` (control)
- Expected behavior: select an enabled primary media tab: onSelect(tab.id); disabled placeholders do nothing
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-d79545153721a517.
  - Test: web/src/components/media/MediaTabBar.interaction.test.tsx#control-d79545153721a517 select an enabled primary media tab.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({!tab.enabled}).
  - Event: inputs=["event/prop handler: {() => tab.enabled && setHovered(tab.id)} {() => setHovered(null)} {() => { if (tab.enabled) onSelect(tab.id); }}","click or native keyboard activation plus current owning state"]; handler={() => tab.enabled && setHovered(tab.id)} {() => setHovered(null)} {() => { if (tab.enabled) onSelect(tab.id); }}.
  - Exact call/state/backend: stateTransition=onSelect(tab.id); disabled placeholders do nothing; backendTrace=["web/src/components/media/MediaTabBar.tsx:67::candidate handler -> {() => tab.enabled && setHovered(tab.id)} {() => setHovered(null)} {() => { if (tab.enabled) onSelect(tab.id); }}","actual branch/state -> onSelect(tab.id); disabled placeholders do nothing","exact call -> onSelect(tab.id); disabled placeholders do nothing","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/MediaTabBar.tsx#MediaTabBar"].
  - Visible/accessibility/return path: success=select an enabled primary media tab: onSelect(tab.id); disabled placeholders do nothing; accessibility={"focus":"Native keyboard-focusable control","label":"tab","shortcut":"None declared on this control"}; returnPath=["Focus should remain on the selected tab; current code relies on native button order."].
  - Outcome matrix: {"success":"select an enabled primary media tab: onSelect(tab.id); disabled placeholders do nothing","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaTabBar.tsx:67; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {!tab.enabled}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-ad2a244350397822

- Candidate/source: `control-5e42760fbbe049cc` at `web/src/components/media/MediaTabBar.tsx:158:11` (control)
- Expected behavior: select a media subtab: onSelect(tab.id)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-5e42760fbbe049cc.
  - Test: web/src/components/media/MediaTabBar.interaction.test.tsx#control-5e42760fbbe049cc select a media subtab.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => onSelect(tab.id)}","click or native keyboard activation plus current owning state"]; handler={() => onSelect(tab.id)}.
  - Exact call/state/backend: stateTransition=onSelect(tab.id); backendTrace=["web/src/components/media/MediaTabBar.tsx:158::candidate handler -> {() => onSelect(tab.id)}","actual branch/state -> onSelect(tab.id)","exact call -> onSelect(tab.id)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/MediaTabBar.tsx#MediaTabBar"].
  - Visible/accessibility/return path: success=select a media subtab: onSelect(tab.id); accessibility={"focus":"Native keyboard-focusable control","label":"tab","shortcut":"None declared on this control"}; returnPath=["Focus should remain on the selected tab; current code relies on native button order."].
  - Outcome matrix: {"success":"select a media subtab: onSelect(tab.id)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/MediaTabBar.tsx:158; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/MediaTabBar.interaction.test.tsx#control-d79545153721a517 select an enabled primary media tab` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/MediaTabBar.interaction.test.tsx#control-5e42760fbbe049cc select a media subtab` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/MediaTabBar.interaction.test.tsx -t "control-d79545153721a517 select an enabled primary media tab"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaTabBar.interaction.test.tsx -t "control-5e42760fbbe049cc select a media subtab"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/MediaTabBar.tsx`, `web/src/components/media/MediaTabBar.tsx#MediaTabBar` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/MediaTabBar.interaction.test.tsx -t "control-d79545153721a517 select an enabled primary media tab"`
  - Run: `pnpm -C web test -- --run src/components/media/MediaTabBar.interaction.test.tsx -t "control-5e42760fbbe049cc select a media subtab"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 31: control-acceptance (implementation-slice-eb3ebba559557179)

**Covered records:**
- `control-record-9cd2c8f726120fa9` (control)

**Files:**
- Modify: `web/src/components/media/SoundLibraryTab.tsx`
- Modify: `web/src/store/libraryStore.ts#importToProject`
- Modify: `web/src/lib/libraryApi.ts#libraryImportToProject`
- Modify: `src-tauri/src/library.rs#library_import_to_project`
- Modify: `crates/opentake-media/src/library.rs`
- Modify: `web/src/components/media/SoundLibraryTab.tsx#SoundLibraryTab`
- Test (reviewed-planned): `web/src/components/media/SoundLibraryTab.interaction.test.tsx#control-0ee3b0454ad2624a import a sound-library entry into the project`

**Candidate-bound contracts:**

#### control-record-9cd2c8f726120fa9

- Candidate/source: `control-0ee3b0454ad2624a` at `web/src/components/media/SoundLibraryTab.tsx:122:9` (control)
- Expected behavior: import a sound-library entry into the project: busy -> importToProject -> added/failed feedback -> button returns
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-0ee3b0454ad2624a.
  - Test: web/src/components/media/SoundLibraryTab.interaction.test.tsx#control-0ee3b0454ad2624a import a sound-library entry into the project.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({busy}).
  - Event: inputs=["event/prop handler: {handleImport}","click or native keyboard activation plus current owning state"]; handler={handleImport}.
  - Exact call/state/backend: stateTransition=busy -> importToProject -> added/failed feedback -> button returns; backendTrace=["web/src/components/media/SoundLibraryTab.tsx:122::candidate handler -> {handleImport}","actual branch/state -> busy -> importToProject -> added/failed feedback -> button returns","exact call/arguments -> useLibraryStore.importToProject(entry.id) -> libraryImportToProject(id), then refreshMedia(); do not refresh the global library list","web/src/store/libraryStore.ts::importToProject(id) -> lib.libraryImportToProject(id) -> refreshMedia()","web/src/lib/libraryApi.ts::libraryImportToProject -> invoke('library_import_to_project',{id})","src-tauri/src/library.rs::library_import_to_project(id) -> library_import_to_project_impl -> crates/opentake-media/src/library.rs library store","code:web/src/components/media/SoundLibraryTab.tsx#SoundLibraryTab","code:web/src/lib/libraryApi.ts#libraryImportToProject","code:src-tauri/src/library.rs#library_import_to_project"].
  - Visible/accessibility/return path: success=import a sound-library entry into the project: busy -> importToProject -> added/failed feedback -> button returns; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"media.sound.add\")","shortcut":"None declared on this control"}; returnPath=["Remains in the Sound subtab and replaces the button briefly with feedback."].
  - Outcome matrix: {"success":"import a sound-library entry into the project: busy -> importToProject -> added/failed feedback -> button returns","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/SoundLibraryTab.tsx:122; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/SoundLibraryTab.tsx:122; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {busy}.","cancel":"Cancellation/dismissal follows the exact guard in busy -> importToProject -> added/failed feedback -> button returns; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in busy -> importToProject -> added/failed feedback -> button returns.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/SoundLibraryTab.tsx:122; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/SoundLibraryTab.interaction.test.tsx#control-0ee3b0454ad2624a import a sound-library entry into the project` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/SoundLibraryTab.interaction.test.tsx -t "control-0ee3b0454ad2624a import a sound-library entry into the project"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/SoundLibraryTab.tsx`, `web/src/store/libraryStore.ts#importToProject`, `web/src/lib/libraryApi.ts#libraryImportToProject`, `src-tauri/src/library.rs#library_import_to_project`, `crates/opentake-media/src/library.rs`, `web/src/components/media/SoundLibraryTab.tsx#SoundLibraryTab` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/SoundLibraryTab.interaction.test.tsx -t "control-0ee3b0454ad2624a import a sound-library entry into the project"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

## Shared capability references

- `cache-identity` / `implementation-slice-5af4f1ababc7b495`: implemented once in `data-safety`; this group contributes records `requirement-73db19cc5b04bae3` as acceptance references.
