# Documentation Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 46 verified incomplete records in the `documentation` gap group.

**Architecture:** Implement 32 primary evidence-bound slices and reference 0 shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.

---

### Task 1: documentation-process (implementation-slice-b22168acdf13d83f)

**Covered records:**
- `requirement-00a3f0fb209b8e83` (requirement)
- `requirement-a1f71c7bc9cb56fa` (requirement)
- `requirement-ec37ffdfaa3fdd06` (requirement)
- `requirement-227310df1383ca5d` (requirement)
- `requirement-74cf932a74d589b3` (requirement)

**Files:**
- Modify: `README.md`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-00a3f0fb209b8e83 closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-a1f71c7bc9cb56fa closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-ec37ffdfaa3fdd06 closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-227310df1383ca5d closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-74cf932a74d589b3 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-00a3f0fb209b8e83

- Candidate/source: `doc-5819971af9000b9f` at `README.md:309` (requirement)
- Expected behavior: Publish and document the 0.2.0 persistence/media import/thumbnails/waveform milestone.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Implementation: Reconcile the implemented milestone against release criteria, update package versions, run the release gate, create the 0.2.0 tag/release, and replace the TBD date with the actual release date.
  - Verify that every named version, date, link, authority, tag, release artifact, and cross-language statement exists and is mutually consistent.
  - Run the document coverage and link/release checks, and attach exact repository or GitHub evidence before reclassification.

#### requirement-a1f71c7bc9cb56fa

- Candidate/source: `doc-add79c3f48b39b35` at `README.md:310` (requirement)
- Expected behavior: Publish and document the 0.3.0 timeline UI/preview/MCP milestone.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Implementation: Reconcile the implemented milestone against release criteria, update package versions, run the release gate, create the 0.3.0 tag/release, and replace the TBD date with the actual release date.
  - Verify that every named version, date, link, authority, tag, release artifact, and cross-language statement exists and is mutually consistent.
  - Run the document coverage and link/release checks, and attach exact repository or GitHub evidence before reclassification.

#### requirement-ec37ffdfaa3fdd06

- Candidate/source: `doc-a1f908caacc36b59` at `README.md:311` (requirement)
- Expected behavior: Publish and document the 0.4.0 GPU compositor/text rasterization milestone.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Implementation: Reconcile the implemented milestone against release criteria, update package versions, run the release gate, create the 0.4.0 tag/release, and replace the TBD date with the actual release date.
  - Verify that every named version, date, link, authority, tag, release artifact, and cross-language statement exists and is mutually consistent.
  - Run the document coverage and link/release checks, and attach exact repository or GitHub evidence before reclassification.

#### requirement-227310df1383ca5d

- Candidate/source: `doc-b7456d38d0a1c034` at `README.md:312` (requirement)
- Expected behavior: Publish the 1.0.0 full-parity release milestone.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Implementation: Close all active completion-ledger gaps, pass the full release gate, tag 1.0.0, publish a GitHub release with reproducible artifacts, and replace the TBD date.
  - Verify that every named version, date, link, authority, tag, release artifact, and cross-language statement exists and is mutually consistent.
  - Run the document coverage and link/release checks, and attach exact repository or GitHub evidence before reclassification.

#### requirement-74cf932a74d589b3

- Candidate/source: `doc-ebced43ba8715197` at `README.md:322` (requirement)
- Expected behavior: Document the project's WeChat community channel without a placeholder.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Implementation: Replace TBD with a verified official WeChat entry point, or explicitly state that no WeChat channel is offered; apply the same decision to all README translations.
  - Verify that every named version, date, link, authority, tag, release artifact, and cross-language statement exists and is mutually consistent.
  - Run the document coverage and link/release checks, and attach exact repository or GitHub evidence before reclassification.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-00a3f0fb209b8e83 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-a1f71c7bc9cb56fa closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-ec37ffdfaa3fdd06 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-227310df1383ca5d closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-74cf932a74d589b3 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-00a3f0fb209b8e83 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-a1f71c7bc9cb56fa closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-ec37ffdfaa3fdd06 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-227310df1383ca5d closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-74cf932a74d589b3 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 5 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `README.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-00a3f0fb209b8e83 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-a1f71c7bc9cb56fa closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-ec37ffdfaa3fdd06 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-227310df1383ca5d closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-74cf932a74d589b3 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

### Task 2: documentation-process (implementation-slice-de37a3256ffaa8f4)

**Covered records:**
- `requirement-0396b3bd374f5ed5` (requirement)

**Files:**
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Modify: `crates/opentake-domain/src/grade.rs#ColorGrade`
- Modify: `tools/completion-tests/doc-81ec33652e4189f1.test.mjs#completion_81ec33652e4189f1_the_implemented_grade_chain_uses_floating_point_`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-0396b3bd374f5ed5 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-0396b3bd374f5ed5

- Candidate/source: `doc-81ec33652e4189f1` at `docs/architecture/CAPCUT-GAP.md:121` (requirement)
- Expected behavior: At docs/architecture/CAPCUT-GAP.md:121 under “高阶浮点调色引擎 — `missing` · 难度 high · 优先级 p0” (heading), the source “### 高阶浮点调色引擎 — `missing` · 难度 high · 优先级 p0” requires this exact behavior: The implemented grade chain uses floating-point parameters and deterministic ordering.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/architecture/CAPCUT-GAP.md:121; signal=heading; heading=高阶浮点调色引擎 — `missing` · 难度 high · 优先级 p0; candidate=### 高阶浮点调色引擎 — `missing` · 难度 high · 优先级 p0
  - Expected behavior: The implemented grade chain uses floating-point parameters and deterministic ordering. This closes only the promise expressed by “高阶浮点调色引擎 — `missing` · 难度 high · 优先级 p0” in “高阶浮点调色引擎 — `missing` · 难度 high · 优先级 p0”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “高阶浮点调色引擎 — `missing` · 难度 high · 优先级 p0” with the scenario below and register test:tools/completion-tests/doc-81ec33652e4189f1.test.mjs#completion_81ec33652e4189f1_the_implemented_grade_chain_uses_floating_point_
  - Initial state/input/event: start from the smallest valid fixture for “The implemented grade chain uses floating-point parameters and deterministic ordering.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “高阶浮点调色引擎 — `missing` · 难度 high · 优先级 p0”.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “The implemented grade chain uses floating-point parameters and deterministic ordering.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-81ec33652e4189f1.test.mjs#completion_81ec33652e4189f1_the_implemented_grade_chain_uses_floating_point_.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-0396b3bd374f5ed5 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-0396b3bd374f5ed5 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/CAPCUT-GAP.md`, `crates/opentake-domain/src/grade.rs#ColorGrade`, `tools/completion-tests/doc-81ec33652e4189f1.test.mjs#completion_81ec33652e4189f1_the_implemented_grade_chain_uses_floating_point_` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-0396b3bd374f5ed5 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 3: documentation-process (implementation-slice-6fb3d332f10ab755)

**Covered records:**
- `requirement-36acf41981ef337a` (requirement)

**Files:**
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Modify: `src-tauri/src/commands.rs#resolve_start_timecodes`
- Modify: `crates/opentake-project/src/fcpxml.rs#export_xmeml_with_timecodes`
- Modify: `crates/opentake-render/tests/completion_4313d7b1bdc3efbf.rs#completion_4313d7b1bdc3efbf_read_source_start_timecode_via_ffprobe_cache_per`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-36acf41981ef337a closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-36acf41981ef337a

- Candidate/source: `doc-4313d7b1bdc3efbf` at `docs/architecture/MODULE-PORT-MAP.md:439` (requirement)
- Expected behavior: At docs/architecture/MODULE-PORT-MAP.md:439 under “Export · `mixed` → **needs-replacement**” (gap-marker), the source “- [XMEML 源起始时码读取 readStartTimecodeFrame] 用 AVAssetReader 读 .timecode 轨第一个 sample 的 data buffer 前 4 字节(大端 UInt32)作为起始帧。跳过没有 data buffer 的前导编辑边界。结果按 mediaRef 缓存(startFrameCache)，video/audio 共用一次读取。无时码轨返回 nil→用 0。FFmpeg 移植：用 ffprobe 读 timecode 流或 tmcd，缺失则 0。” requires this exact behavior: Read source start timecode via ffprobe, cache per media reference, and inject it into XMEML with zero fallback.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/architecture/MODULE-PORT-MAP.md:439; signal=gap-marker; heading=Export  ·  `mixed` → **needs-replacement**; candidate=- [XMEML 源起始时码读取 readStartTimecodeFrame] 用 AVAssetReader 读 .timecode 轨第一个 sample 的 data buffer 前 4 字节(大端 UInt32)作为起始帧。跳过没有 data buffer 的前导编辑边界。结果按 mediaRef 缓存(startFrameCache)，video/audio 共用一次读取。无时码轨返回 nil→用 0。FFmpeg 移植：用 ffprobe 读 timecode 流或 tmcd，缺失则 0。
  - Expected behavior: Read source start timecode via ffprobe, cache per media reference, and inject it into XMEML with zero fallback. This closes only the promise expressed by “[XMEML 源起始时码读取 readStartTimecodeFrame] 用 AVAssetReader 读 .timecode 轨第一个 sample 的 data buffer 前 4 字节(大端 UInt32)作为起始帧。跳过没有 data buffer 的前导编辑边界。结果按 mediaRef 缓存(startFrameCache)，video/audio 共用一次读取。无时码轨返回 nil→用 0。FFmpeg 移植：用 ffprobe 读 timecode 流或 tmcd，缺失则 0。” in “Export · `mixed` → **needs-replacement**”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “[XMEML 源起始时码读取 readStartTimecodeFrame] 用 AVAssetReader 读 .timecode 轨第一个 sample 的 data buffer 前 4 字节(大端 UInt32)作为起始帧。跳过没有 data buffer 的前导编辑边界。结果按 mediaRef 缓存(startFrameCache)，video/audio 共用一次读取。无时码轨返回 nil→用 0。FFmpeg 移植：用 ffprobe 读 timecode 流或 tmcd，缺失则 0。” with the scenario below and register test:crates/opentake-render/tests/completion_4313d7b1bdc3efbf.rs#completion_4313d7b1bdc3efbf_read_source_start_timecode_via_ffprobe_cache_per
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “[XMEML 源起始时码读取 readStartTimecodeFrame] 用 AVAssetReader 读 .timecode 轨第一个 sample 的 data buffer 前 4 字节(大端 UInt32)作为起始帧。跳过没有 data buffer 的前导编辑边界。结果按 mediaRef 缓存(startFrameCache)，video/audio 共用一次读取。无时码轨返回 nil→用 0。FFmpeg 移植：用 ffprobe 读 timecode 流或 tmcd，缺失则 0。”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Read source start timecode via ffprobe, cache per media reference, and inject it into XMEML with zero fallback.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_4313d7b1bdc3efbf.rs#completion_4313d7b1bdc3efbf_read_source_start_timecode_via_ffprobe_cache_per.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-36acf41981ef337a closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-36acf41981ef337a closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/MODULE-PORT-MAP.md`, `src-tauri/src/commands.rs#resolve_start_timecodes`, `crates/opentake-project/src/fcpxml.rs#export_xmeml_with_timecodes`, `crates/opentake-render/tests/completion_4313d7b1bdc3efbf.rs#completion_4313d7b1bdc3efbf_read_source_start_timecode_via_ffprobe_cache_per` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-36acf41981ef337a closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 4: documentation-process (implementation-slice-4bcd329850374fe1)

**Covered records:**
- `requirement-0deb906632919b9c` (requirement)

**Files:**
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `crates/opentake-core/src/events.rs#EventBus`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `tools/completion-tests/doc-0aab3e1ee98f7363.test.mjs#completion_0aab3e1ee98f7363_the_functional_scope_named_by_phase_0_is_present`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-0deb906632919b9c closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-0deb906632919b9c

- Candidate/source: `doc-0aab3e1ee98f7363` at `docs/architecture/ROADMAP.md:7` (requirement)
- Expected behavior: At docs/architecture/ROADMAP.md:7 under “Phase 0 — 工程脚手架” (heading), the source “## Phase 0 — 工程脚手架” requires this exact behavior: The functional scope named by Phase 0 — 工程脚手架 is present with automated coverage; later advanced gaps are classified separately.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/architecture/ROADMAP.md:7; signal=heading; heading=Phase 0 — 工程脚手架; candidate=## Phase 0 — 工程脚手架
  - Expected behavior: The functional scope named by Phase 0 — 工程脚手架 is present with automated coverage; later advanced gaps are classified separately. This closes only the promise expressed by “Phase 0 — 工程脚手架” in “Phase 0 — 工程脚手架”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Phase 0 — 工程脚手架” with the scenario below and register test:tools/completion-tests/doc-0aab3e1ee98f7363.test.mjs#completion_0aab3e1ee98f7363_the_functional_scope_named_by_phase_0_is_present
  - Initial state/input/event: freeze the implementation and test inventory referenced by “Phase 0 — 工程脚手架” at the audited HEAD, then enumerate every concrete promise within “The functional scope named by Phase 0 — 工程脚手架 is present with automated coverage; later advanced gaps are classified separately.” without treating the heading itself as proof.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, use N/A for product mutation; resolve each enumerated promise to a real owning code symbol and exact automated test, leaving any unresolved promise incomplete.
  - Visible/returned assertion: assert a one-to-one inventory with no missing, duplicate, placeholder, or evidence-free promise and emit a deterministic failure naming the unresolved source line.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-0aab3e1ee98f7363.test.mjs#completion_0aab3e1ee98f7363_the_functional_scope_named_by_phase_0_is_present.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-0deb906632919b9c closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-0deb906632919b9c closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/ROADMAP.md`, `crates/opentake-core/src/events.rs#EventBus`, `crates/opentake-ops/src/command.rs#EditCommand`, `tools/completion-tests/doc-0aab3e1ee98f7363.test.mjs#completion_0aab3e1ee98f7363_the_functional_scope_named_by_phase_0_is_present` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-0deb906632919b9c closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 5: documentation-process (implementation-slice-e234135f316b7443)

**Covered records:**
- `requirement-7110b6ea236f23ba` (requirement)

**Files:**
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `crates/opentake-core/src/events.rs#EventBus`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `tools/completion-tests/doc-afcbfd192e8443ce.test.mjs#completion_afcbfd192e8443ce_the_functional_scope_named_by_phase_1_rust_io_is`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-7110b6ea236f23ba closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-7110b6ea236f23ba

- Candidate/source: `doc-afcbfd192e8443ce` at `docs/architecture/ROADMAP.md:11` (requirement)
- Expected behavior: At docs/architecture/ROADMAP.md:11 under “Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先” (heading), the source “## Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先” requires this exact behavior: The functional scope named by Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先 is present with automated coverage; later advanced gaps are classified separately.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/architecture/ROADMAP.md:11; signal=heading; heading=Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先; candidate=## Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先
  - Expected behavior: The functional scope named by Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先 is present with automated coverage; later advanced gaps are classified separately. This closes only the promise expressed by “Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先” in “Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先” with the scenario below and register test:tools/completion-tests/doc-afcbfd192e8443ce.test.mjs#completion_afcbfd192e8443ce_the_functional_scope_named_by_phase_1_rust_io_is
  - Initial state/input/event: freeze the implementation and test inventory referenced by “Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先” at the audited HEAD, then enumerate every concrete promise within “The functional scope named by Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先 is present with automated coverage; later advanced gaps are classified separately.” without treating the heading itself as proof.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, use N/A for product mutation; resolve each enumerated promise to a real owning code symbol and exact automated test, leaving any unresolved promise incomplete.
  - Visible/returned assertion: assert a one-to-one inventory with no missing, duplicate, placeholder, or evidence-free promise and emit a deterministic failure naming the unresolved source line.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-afcbfd192e8443ce.test.mjs#completion_afcbfd192e8443ce_the_functional_scope_named_by_phase_1_rust_io_is.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-7110b6ea236f23ba closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-7110b6ea236f23ba closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/ROADMAP.md`, `crates/opentake-core/src/events.rs#EventBus`, `crates/opentake-ops/src/command.rs#EditCommand`, `tools/completion-tests/doc-afcbfd192e8443ce.test.mjs#completion_afcbfd192e8443ce_the_functional_scope_named_by_phase_1_rust_io_is` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-7110b6ea236f23ba closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 6: documentation-process (implementation-slice-50445a81a4a34a0c)

**Covered records:**
- `requirement-a7ce2ba925ab53ff` (requirement)

**Files:**
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `tools/completion-tests/doc-a14d6e768f45af3d.test.mjs#completion_a14d6e768f45af3d_the_functional_scope_named_by_phase_2_is_present`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-a7ce2ba925ab53ff closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-a7ce2ba925ab53ff

- Candidate/source: `doc-a14d6e768f45af3d` at `docs/architecture/ROADMAP.md:20` (requirement)
- Expected behavior: At docs/architecture/ROADMAP.md:20 under “Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢” (heading), the source “## Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢” requires this exact behavior: The functional scope named by Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢 is present with automated coverage; later advanced gaps are classified separately.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/architecture/ROADMAP.md:20; signal=heading; heading=Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢; candidate=## Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢
  - Expected behavior: The functional scope named by Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢 is present with automated coverage; later advanced gaps are classified separately. This closes only the promise expressed by “Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢” in “Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢” with the scenario below and register test:tools/completion-tests/doc-a14d6e768f45af3d.test.mjs#completion_a14d6e768f45af3d_the_functional_scope_named_by_phase_2_is_present
  - Initial state/input/event: freeze the implementation and test inventory referenced by “Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢” at the audited HEAD, then enumerate every concrete promise within “The functional scope named by Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢 is present with automated coverage; later advanced gaps are classified separately.” without treating the heading itself as proof.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, use N/A for product mutation; resolve each enumerated promise to a real owning code symbol and exact automated test, leaving any unresolved promise incomplete.
  - Visible/returned assertion: assert a one-to-one inventory with no missing, duplicate, placeholder, or evidence-free promise and emit a deterministic failure naming the unresolved source line.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-a14d6e768f45af3d.test.mjs#completion_a14d6e768f45af3d_the_functional_scope_named_by_phase_2_is_present.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-a7ce2ba925ab53ff closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-a7ce2ba925ab53ff closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/ROADMAP.md`, `tools/completion-tests/doc-a14d6e768f45af3d.test.mjs#completion_a14d6e768f45af3d_the_functional_scope_named_by_phase_2_is_present` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-a7ce2ba925ab53ff closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

### Task 7: documentation-process (implementation-slice-c9391ed9c44488e3)

**Covered records:**
- `requirement-85b7d89e1e61930a` (requirement)

**Files:**
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `crates/opentake-render/tests/completion_26feba8c36e43909.rs#completion_26feba8c36e43909_the_functional_scope_named_by_phase_3_wgpu_poc_i`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-85b7d89e1e61930a closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-85b7d89e1e61930a

- Candidate/source: `doc-26feba8c36e43909` at `docs/architecture/ROADMAP.md:27` (requirement)
- Expected behavior: At docs/architecture/ROADMAP.md:27 under “Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证)” (heading), the source “## Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证)” requires this exact behavior: The functional scope named by Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证) is present with automated coverage; later advanced gaps are classified separately.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/architecture/ROADMAP.md:27; signal=heading; heading=Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证); candidate=## Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证)
  - Expected behavior: The functional scope named by Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证) is present with automated coverage; later advanced gaps are classified separately. This closes only the promise expressed by “Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证)” in “Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证)” with the scenario below and register test:crates/opentake-render/tests/completion_26feba8c36e43909.rs#completion_26feba8c36e43909_the_functional_scope_named_by_phase_3_wgpu_poc_i
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证)”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “The functional scope named by Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证) is present with automated coverage; later advanced gaps are classified separately.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_26feba8c36e43909.rs#completion_26feba8c36e43909_the_functional_scope_named_by_phase_3_wgpu_poc_i.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-85b7d89e1e61930a closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-85b7d89e1e61930a closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/ROADMAP.md`, `crates/opentake-render/tests/completion_26feba8c36e43909.rs#completion_26feba8c36e43909_the_functional_scope_named_by_phase_3_wgpu_poc_i` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-85b7d89e1e61930a closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 8: documentation-process (implementation-slice-ceec4bafa6b46f92)

**Covered records:**
- `requirement-efc5992bc3a16452` (requirement)

**Files:**
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `tools/completion-tests/doc-0863fc440bf209d5.test.mjs#completion_0863fc440bf209d5_the_functional_scope_named_by_phase_4_is_present`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-efc5992bc3a16452 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-efc5992bc3a16452

- Candidate/source: `doc-0863fc440bf209d5` at `docs/architecture/ROADMAP.md:40` (requirement)
- Expected behavior: At docs/architecture/ROADMAP.md:40 under “Phase 4 — 🔴 播放/预览引擎” (heading), the source “## Phase 4 — 🔴 播放/预览引擎” requires this exact behavior: The functional scope named by Phase 4 — 🔴 播放/预览引擎 is present with automated coverage; later advanced gaps are classified separately.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/architecture/ROADMAP.md:40; signal=heading; heading=Phase 4 — 🔴 播放/预览引擎; candidate=## Phase 4 — 🔴 播放/预览引擎
  - Expected behavior: The functional scope named by Phase 4 — 🔴 播放/预览引擎 is present with automated coverage; later advanced gaps are classified separately. This closes only the promise expressed by “Phase 4 — 🔴 播放/预览引擎” in “Phase 4 — 🔴 播放/预览引擎”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Phase 4 — 🔴 播放/预览引擎” with the scenario below and register test:tools/completion-tests/doc-0863fc440bf209d5.test.mjs#completion_0863fc440bf209d5_the_functional_scope_named_by_phase_4_is_present
  - Initial state/input/event: freeze the implementation and test inventory referenced by “Phase 4 — 🔴 播放/预览引擎” at the audited HEAD, then enumerate every concrete promise within “The functional scope named by Phase 4 — 🔴 播放/预览引擎 is present with automated coverage; later advanced gaps are classified separately.” without treating the heading itself as proof.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, use N/A for product mutation; resolve each enumerated promise to a real owning code symbol and exact automated test, leaving any unresolved promise incomplete.
  - Visible/returned assertion: assert a one-to-one inventory with no missing, duplicate, placeholder, or evidence-free promise and emit a deterministic failure naming the unresolved source line.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-0863fc440bf209d5.test.mjs#completion_0863fc440bf209d5_the_functional_scope_named_by_phase_4_is_present.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-efc5992bc3a16452 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-efc5992bc3a16452 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/ROADMAP.md`, `tools/completion-tests/doc-0863fc440bf209d5.test.mjs#completion_0863fc440bf209d5_the_functional_scope_named_by_phase_4_is_present` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-efc5992bc3a16452 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

### Task 9: documentation-process (implementation-slice-b60d5759ed865814)

**Covered records:**
- `requirement-2c76653030e970c4` (requirement)

**Files:**
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `src-tauri/src/export.rs#resolve_preset`
- Modify: `tools/completion-tests/doc-be65c68d4317b49d.test.mjs#completion_be65c68d4317b49d_the_functional_scope_named_by_phase_5_is_present`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-2c76653030e970c4 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-2c76653030e970c4

- Candidate/source: `doc-be65c68d4317b49d` at `docs/architecture/ROADMAP.md:45` (requirement)
- Expected behavior: At docs/architecture/ROADMAP.md:45 under “Phase 5 — 导出” (heading), the source “## Phase 5 — 导出” requires this exact behavior: The functional scope named by Phase 5 — 导出 is present with automated coverage; later advanced gaps are classified separately.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/architecture/ROADMAP.md:45; signal=heading; heading=Phase 5 — 导出; candidate=## Phase 5 — 导出
  - Expected behavior: The functional scope named by Phase 5 — 导出 is present with automated coverage; later advanced gaps are classified separately. This closes only the promise expressed by “Phase 5 — 导出” in “Phase 5 — 导出”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Phase 5 — 导出” with the scenario below and register test:tools/completion-tests/doc-be65c68d4317b49d.test.mjs#completion_be65c68d4317b49d_the_functional_scope_named_by_phase_5_is_present
  - Initial state/input/event: freeze the implementation and test inventory referenced by “Phase 5 — 导出” at the audited HEAD, then enumerate every concrete promise within “The functional scope named by Phase 5 — 导出 is present with automated coverage; later advanced gaps are classified separately.” without treating the heading itself as proof.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, use N/A for product mutation; resolve each enumerated promise to a real owning code symbol and exact automated test, leaving any unresolved promise incomplete.
  - Visible/returned assertion: assert a one-to-one inventory with no missing, duplicate, placeholder, or evidence-free promise and emit a deterministic failure naming the unresolved source line.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-be65c68d4317b49d.test.mjs#completion_be65c68d4317b49d_the_functional_scope_named_by_phase_5_is_present.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-2c76653030e970c4 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-2c76653030e970c4 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/ROADMAP.md`, `src-tauri/src/export.rs#resolve_preset`, `tools/completion-tests/doc-be65c68d4317b49d.test.mjs#completion_be65c68d4317b49d_the_functional_scope_named_by_phase_5_is_present` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-2c76653030e970c4 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 10: documentation-process (implementation-slice-d250c0acd843e3e0)

**Covered records:**
- `requirement-5492792ae4ebcbfe` (requirement)

**Files:**
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `tools/completion-tests/doc-65c3edf3e7629d05.test.mjs#completion_65c3edf3e7629d05_the_functional_scope_named_by_phase_8_is_present`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-5492792ae4ebcbfe closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-5492792ae4ebcbfe

- Candidate/source: `doc-65c3edf3e7629d05` at `docs/architecture/ROADMAP.md:65` (requirement)
- Expected behavior: At docs/architecture/ROADMAP.md:65 under “Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索” (heading), the source “## Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索” requires this exact behavior: The functional scope named by Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索 is present with automated coverage; later advanced gaps are classified separately.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/architecture/ROADMAP.md:65; signal=heading; heading=Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索; candidate=## Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索
  - Expected behavior: The functional scope named by Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索 is present with automated coverage; later advanced gaps are classified separately. This closes only the promise expressed by “Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索” in “Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索” with the scenario below and register test:tools/completion-tests/doc-65c3edf3e7629d05.test.mjs#completion_65c3edf3e7629d05_the_functional_scope_named_by_phase_8_is_present
  - Initial state/input/event: freeze the implementation and test inventory referenced by “Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索” at the audited HEAD, then enumerate every concrete promise within “The functional scope named by Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索 is present with automated coverage; later advanced gaps are classified separately.” without treating the heading itself as proof.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, use N/A for product mutation; resolve each enumerated promise to a real owning code symbol and exact automated test, leaving any unresolved promise incomplete.
  - Visible/returned assertion: assert a one-to-one inventory with no missing, duplicate, placeholder, or evidence-free promise and emit a deterministic failure naming the unresolved source line.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-65c3edf3e7629d05.test.mjs#completion_65c3edf3e7629d05_the_functional_scope_named_by_phase_8_is_present.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-5492792ae4ebcbfe closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-5492792ae4ebcbfe closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/architecture/ROADMAP.md`, `tools/completion-tests/doc-65c3edf3e7629d05.test.mjs#completion_65c3edf3e7629d05_the_functional_scope_named_by_phase_8_is_present` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-5492792ae4ebcbfe closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

### Task 11: documentation-process (implementation-slice-08e68c8fc2a3a635)

**Covered records:**
- `requirement-18ba7990fd2368aa` (requirement)

**Files:**
- Modify: `docs/modules/opentake-core/session.md`
- Modify: `src-tauri/src/media.rs#relink_media`
- Modify: `crates/opentake-core/src/session.rs#relink_media_file`
- Modify: `crates/opentake-project/tests/completion_1a9d061c1e415357.rs#completion_1a9d061c1e415357_relink_an_offline_asset_in_place_while_preservin`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-18ba7990fd2368aa closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-18ba7990fd2368aa

- Candidate/source: `doc-1a9d061c1e415357` at `docs/modules/opentake-core/session.md:101` (requirement)
- Expected behavior: At docs/modules/opentake-core/session.md:101 under “`relink_media_file`” (gap-marker), the source “> **这修复的 bug**：直接 re-import 会铸**新** id，把旧 clip 永久孤立在缺失 entry 上。重链复用原 id 在位治愈。” requires this exact behavior: Relink an offline asset in place while preserving its id and rejecting media-type changes.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-core/session.md:101; signal=gap-marker; heading=`relink_media_file`; candidate=> **这修复的 bug**：直接 re-import 会铸**新** id，把旧 clip 永久孤立在缺失 entry 上。重链复用原 id 在位治愈。
  - Expected behavior: Relink an offline asset in place while preserving its id and rejecting media-type changes. This closes only the promise expressed by “> **这修复的 bug**：直接 re-import 会铸**新** id，把旧 clip 永久孤立在缺失 entry 上。重链复用原 id 在位治愈。” in “`relink_media_file`”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “> **这修复的 bug**：直接 re-import 会铸**新** id，把旧 clip 永久孤立在缺失 entry 上。重链复用原 id 在位治愈。” with the scenario below and register test:crates/opentake-project/tests/completion_1a9d061c1e415357.rs#completion_1a9d061c1e415357_relink_an_offline_asset_in_place_while_preservin
  - Initial state/input/event: start from the smallest valid fixture for “Relink an offline asset in place while preserving its id and rejecting media-type changes.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “> **这修复的 bug**：直接 re-import 会铸**新** id，把旧 clip 永久孤立在缺失 entry 上。重链复用原 id 在位治愈。”.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Relink an offline asset in place while preserving its id and rejecting media-type changes.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_1a9d061c1e415357.rs#completion_1a9d061c1e415357_relink_an_offline_asset_in_place_while_preservin.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-18ba7990fd2368aa closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-18ba7990fd2368aa closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/modules/opentake-core/session.md`, `src-tauri/src/media.rs#relink_media`, `crates/opentake-core/src/session.rs#relink_media_file`, `crates/opentake-project/tests/completion_1a9d061c1e415357.rs#completion_1a9d061c1e415357_relink_an_offline_asset_in_place_while_preservin` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-18ba7990fd2368aa closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 12: documentation-process (implementation-slice-9ddf6c2bd750e35f)

**Covered records:**
- `requirement-c353dee4825a05de` (requirement)

**Files:**
- Modify: `docs/modules/opentake-media/probe-ff.md`
- Modify: `crates/opentake-media/src/ff.rs#ffprobe_json`
- Modify: `crates/opentake-project/tests/completion_02d2b0e38fef041e.rs#completion_02d2b0e38fef041e_return_notfound_for_an_absent_media_file_and_def`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-c353dee4825a05de closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-c353dee4825a05de

- Candidate/source: `doc-02d2b0e38fef041e` at `docs/modules/opentake-media/probe-ff.md:60` (requirement)
- Expected behavior: At docs/modules/opentake-media/probe-ff.md:60 under “不变量 / 边界（含一处 bug 修复）” (gap-marker), the source “- 文件不存在 → `MediaError::Io(NotFound)`；流缺失各字段 → 对应 `None` / `0.0` / `false`。” requires this exact behavior: Return NotFound for an absent media file and default absent ffprobe fields without inventing streams.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-media/probe-ff.md:60; signal=gap-marker; heading=不变量 / 边界（含一处 bug 修复）; candidate=- 文件不存在 → `MediaError::Io(NotFound)`；流缺失各字段 → 对应 `None` / `0.0` / `false`。
  - Expected behavior: Return NotFound for an absent media file and default absent ffprobe fields without inventing streams. This closes only the promise expressed by “文件不存在 → `MediaError::Io(NotFound)`；流缺失各字段 → 对应 `None` / `0.0` / `false`。” in “不变量 / 边界（含一处 bug 修复）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “文件不存在 → `MediaError::Io(NotFound)`；流缺失各字段 → 对应 `None` / `0.0` / `false`。” with the scenario below and register test:crates/opentake-project/tests/completion_02d2b0e38fef041e.rs#completion_02d2b0e38fef041e_return_notfound_for_an_absent_media_file_and_def
  - Initial state/input/event: start from the smallest valid fixture for “Return NotFound for an absent media file and default absent ffprobe fields without inventing streams.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “文件不存在 → `MediaError::Io(NotFound)`；流缺失各字段 → 对应 `None` / `0.0` / `false`。”.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Return NotFound for an absent media file and default absent ffprobe fields without inventing streams.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_02d2b0e38fef041e.rs#completion_02d2b0e38fef041e_return_notfound_for_an_absent_media_file_and_def.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-c353dee4825a05de closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-c353dee4825a05de closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/modules/opentake-media/probe-ff.md`, `crates/opentake-media/src/ff.rs#ffprobe_json`, `crates/opentake-project/tests/completion_02d2b0e38fef041e.rs#completion_02d2b0e38fef041e_return_notfound_for_an_absent_media_file_and_def` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-c353dee4825a05de closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 13: documentation-process (implementation-slice-591db03bc57cdfca)

**Covered records:**
- `requirement-83260a5aafaf9a1a` (requirement)

**Files:**
- Modify: `docs/modules/opentake-ops/ops-algorithms.md`
- Modify: `crates/opentake-ops/src/ops/duplicate.rs#duplicate_clips`
- Modify: `crates/opentake-project/tests/completion_7a68dd7a871f196d.rs#completion_7a68dd7a871f196d_duplicate_clips_by_deep_copy_preserve_sources_re`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-83260a5aafaf9a1a closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-83260a5aafaf9a1a

- Candidate/source: `doc-7a68dd7a871f196d` at `docs/modules/opentake-ops/ops-algorithms.md:52` (requirement)
- Expected behavior: At docs/modules/opentake-ops/ops-algorithms.md:52 under “duplicate.rs —— 复制片段（Alt 拖拽）” (gap-marker), the source “- **关键不变量**：与 `move_clips` 同构（同样的目标清区 + pin-by-id + sort + prune），但源片段留原位、目标落深拷贝。链接组重映射：被复制的多片段共享组（如 A/V 对）映射到**全新共享 id** 使副本彼此仍链接；单片段组（或无组）清为 `None`。目标越界 / 类型不兼容 / 片段缺失 → 静默跳过。” requires this exact behavior: Duplicate clips by deep copy, preserve sources, remap multi-clip link groups, and skip invalid destinations without corrupting tracks.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-ops/ops-algorithms.md:52; signal=gap-marker; heading=duplicate.rs —— 复制片段（Alt 拖拽）; candidate=- **关键不变量**：与 `move_clips` 同构（同样的目标清区 + pin-by-id + sort + prune），但源片段留原位、目标落深拷贝。链接组重映射：被复制的多片段共享组（如 A/V 对）映射到**全新共享 id** 使副本彼此仍链接；单片段组（或无组）清为 `None`。目标越界 / 类型不兼容 / 片段缺失 → 静默跳过。
  - Expected behavior: Duplicate clips by deep copy, preserve sources, remap multi-clip link groups, and skip invalid destinations without corrupting tracks. This closes only the promise expressed by “**关键不变量**：与 `move_clips` 同构（同样的目标清区 + pin-by-id + sort + prune），但源片段留原位、目标落深拷贝。链接组重映射：被复制的多片段共享组（如 A/V 对）映射到**全新共享 id** 使副本彼此仍链接；单片段组（或无组）清为 `None`。目标越界 / 类型不兼容 / 片段缺失 → 静默跳过。” in “duplicate.rs —— 复制片段（Alt 拖拽）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “**关键不变量**：与 `move_clips` 同构（同样的目标清区 + pin-by-id + sort + prune），但源片段留原位、目标落深拷贝。链接组重映射：被复制的多片段共享组（如 A/V 对）映射到**全新共享 id** 使副本彼此仍链接；单片段组（或无组）清为 `None`。目标越界 / 类型不兼容 / 片段缺失 → 静默跳过。” with the scenario below and register test:crates/opentake-project/tests/completion_7a68dd7a871f196d.rs#completion_7a68dd7a871f196d_duplicate_clips_by_deep_copy_preserve_sources_re
  - Initial state/input/event: start from the smallest valid fixture for “Duplicate clips by deep copy, preserve sources, remap multi-clip link groups, and skip invalid destinations without corrupting tracks.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “**关键不变量**：与 `move_clips` 同构（同样的目标清区 + pin-by-id + sort + prune），但源片段留原位、目标落深拷贝。链接组重映射：被复制的多片段共享组（如 A/V 对）映射到**全新共享 id** 使副本彼此仍链接；单片段组（或无组）清为 `None`。目标越界 / 类型不兼容 / 片段缺失 → 静默跳过。”.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Duplicate clips by deep copy, preserve sources, remap multi-clip link groups, and skip invalid destinations without corrupting tracks.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_7a68dd7a871f196d.rs#completion_7a68dd7a871f196d_duplicate_clips_by_deep_copy_preserve_sources_re.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-83260a5aafaf9a1a closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-83260a5aafaf9a1a closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/modules/opentake-ops/ops-algorithms.md`, `crates/opentake-ops/src/ops/duplicate.rs#duplicate_clips`, `crates/opentake-project/tests/completion_7a68dd7a871f196d.rs#completion_7a68dd7a871f196d_duplicate_clips_by_deep_copy_preserve_sources_re` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-83260a5aafaf9a1a closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 14: documentation-process (implementation-slice-dd10b14b4df65347)

**Covered records:**
- `requirement-44234ed74b22e723` (requirement)

**Files:**
- Modify: `docs/modules/web/SPEC.md`
- Modify: `web/src/lib/ruler.ts#chooseTicks`
- Modify: `web/src/__tests__/completion/doc-c6c9cb96e3530098.test.ts#completion_c6c9cb96e3530098_choose_the_same_major_tick_interval_and_minor_su`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-44234ed74b22e723 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-44234ed74b22e723

- Candidate/source: `doc-c6c9cb96e3530098` at `docs/modules/web/SPEC.md:1283` (requirement)
- Expected behavior: At docs/modules/web/SPEC.md:1283 under “13.2 1:1 验收清单（逐项打勾）” (unchecked), the source “- [ ] 刻度间隔/次刻度选择算法输出一致。” requires this exact behavior: Choose the same major tick interval and minor subdivision as upstream across zoom/fps edge cases.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/modules/web/SPEC.md:1283; signal=unchecked; heading=13.2 1:1 验收清单（逐项打勾）; candidate=- [ ] 刻度间隔/次刻度选择算法输出一致。
  - Expected behavior: Choose the same major tick interval and minor subdivision as upstream across zoom/fps edge cases. This closes only the promise expressed by “刻度间隔/次刻度选择算法输出一致。” in “13.2 1:1 验收清单（逐项打勾）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “刻度间隔/次刻度选择算法输出一致。” with the scenario below and register test:web/src/__tests__/completion/doc-c6c9cb96e3530098.test.ts#completion_c6c9cb96e3530098_choose_the_same_major_tick_interval_and_minor_su
  - Initial state/input/event: render the exact “13.2 1:1 验收清单（逐项打勾）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “刻度间隔/次刻度选择算法输出一致。”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “Choose the same major tick interval and minor subdivision as upstream across zoom/fps edge cases.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-c6c9cb96e3530098.test.ts#completion_c6c9cb96e3530098_choose_the_same_major_tick_interval_and_minor_su.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-44234ed74b22e723 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-44234ed74b22e723 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/modules/web/SPEC.md`, `web/src/lib/ruler.ts#chooseTicks`, `web/src/__tests__/completion/doc-c6c9cb96e3530098.test.ts#completion_c6c9cb96e3530098_choose_the_same_major_tick_interval_and_minor_su` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-44234ed74b22e723 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 15: documentation-process (implementation-slice-16976b7b539c7e7a)

**Covered records:**
- `requirement-1359445886b2677c` (requirement)

**Files:**
- Modify: `docs/specs/core/5-assembly.md`
- Modify: `crates/opentake-core/src/events.rs#EventBus`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `tools/completion-tests/doc-13a1d8111c7e9441.test.mjs#completion_13a1d8111c7e9441_the_crate_dependency_direction_remains_acyclic_a`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-1359445886b2677c closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-1359445886b2677c

- Candidate/source: `doc-13a1d8111c7e9441` at `docs/specs/core/5-assembly.md:5` (requirement)
- Expected behavior: At docs/specs/core/5-assembly.md:5 under “5.1 依赖方向(谁依赖谁)” (heading), the source “### 5.1 依赖方向(谁依赖谁)” requires this exact behavior: The crate dependency direction remains acyclic and core-owned.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/core/5-assembly.md:5; signal=heading; heading=5.1 依赖方向(谁依赖谁); candidate=### 5.1 依赖方向(谁依赖谁)
  - Expected behavior: The crate dependency direction remains acyclic and core-owned. This closes only the promise expressed by “5.1 依赖方向(谁依赖谁)” in “5.1 依赖方向(谁依赖谁)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.1 依赖方向(谁依赖谁)” with the scenario below and register test:tools/completion-tests/doc-13a1d8111c7e9441.test.mjs#completion_13a1d8111c7e9441_the_crate_dependency_direction_remains_acyclic_a
  - Initial state/input/event: freeze the implementation and test inventory referenced by “5.1 依赖方向(谁依赖谁)” at the audited HEAD, then enumerate every concrete promise within “The crate dependency direction remains acyclic and core-owned.” without treating the heading itself as proof.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, use N/A for product mutation; resolve each enumerated promise to a real owning code symbol and exact automated test, leaving any unresolved promise incomplete.
  - Visible/returned assertion: assert a one-to-one inventory with no missing, duplicate, placeholder, or evidence-free promise and emit a deterministic failure naming the unresolved source line.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-13a1d8111c7e9441.test.mjs#completion_13a1d8111c7e9441_the_crate_dependency_direction_remains_acyclic_a.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-1359445886b2677c closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-1359445886b2677c closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/core/5-assembly.md`, `crates/opentake-core/src/events.rs#EventBus`, `crates/opentake-ops/src/command.rs#EditCommand`, `tools/completion-tests/doc-13a1d8111c7e9441.test.mjs#completion_13a1d8111c7e9441_the_crate_dependency_direction_remains_acyclic_a` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-1359445886b2677c closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 16: documentation-process (implementation-slice-482dc9e0e5815eeb)

**Covered records:**
- `requirement-9881174f36216cae` (requirement)

**Files:**
- Modify: `docs/specs/core/8-implementation.md`
- Modify: `crates/opentake-core/src/events.rs#EventBus`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `tools/completion-tests/doc-a755104ad6ee38c7.test.mjs#completion_a755104ad6ee38c7_the_named_core_implementation_stage_is_integrate`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-9881174f36216cae closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-9881174f36216cae

- Candidate/source: `doc-a755104ad6ee38c7` at `docs/specs/core/8-implementation.md:5` (requirement)
- Expected behavior: At docs/specs/core/8-implementation.md:5 under “阶段 A — 纯逻辑核(随 Phase 1,无 IO,可全单测)” (heading), the source “### 阶段 A — 纯逻辑核(随 Phase 1,无 IO,可全单测)” requires this exact behavior: The named core implementation stage is integrated with automated coverage.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/core/8-implementation.md:5; signal=heading; heading=阶段 A — 纯逻辑核(随 Phase 1,无 IO,可全单测); candidate=### 阶段 A — 纯逻辑核(随 Phase 1,无 IO,可全单测)
  - Expected behavior: The named core implementation stage is integrated with automated coverage. This closes only the promise expressed by “阶段 A — 纯逻辑核(随 Phase 1,无 IO,可全单测)” in “阶段 A — 纯逻辑核(随 Phase 1,无 IO,可全单测)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “阶段 A — 纯逻辑核(随 Phase 1,无 IO,可全单测)” with the scenario below and register test:tools/completion-tests/doc-a755104ad6ee38c7.test.mjs#completion_a755104ad6ee38c7_the_named_core_implementation_stage_is_integrate
  - Initial state/input/event: freeze the implementation and test inventory referenced by “阶段 A — 纯逻辑核(随 Phase 1,无 IO,可全单测)” at the audited HEAD, then enumerate every concrete promise within “The named core implementation stage is integrated with automated coverage.” without treating the heading itself as proof.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, use N/A for product mutation; resolve each enumerated promise to a real owning code symbol and exact automated test, leaving any unresolved promise incomplete.
  - Visible/returned assertion: assert a one-to-one inventory with no missing, duplicate, placeholder, or evidence-free promise and emit a deterministic failure naming the unresolved source line.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-a755104ad6ee38c7.test.mjs#completion_a755104ad6ee38c7_the_named_core_implementation_stage_is_integrate.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-9881174f36216cae closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-9881174f36216cae closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/core/8-implementation.md`, `crates/opentake-core/src/events.rs#EventBus`, `crates/opentake-ops/src/command.rs#EditCommand`, `tools/completion-tests/doc-a755104ad6ee38c7.test.mjs#completion_a755104ad6ee38c7_the_named_core_implementation_stage_is_integrate` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-9881174f36216cae closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 17: documentation-process (implementation-slice-61cfc2e1a1800e4e)

**Covered records:**
- `requirement-c871a0685b710893` (requirement)

**Files:**
- Modify: `docs/specs/core/8-implementation.md`
- Modify: `crates/opentake-core/src/events.rs#EventBus`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `tools/completion-tests/doc-545c86b2057f9b05.test.mjs#completion_545c86b2057f9b05_the_named_core_implementation_stage_is_integrate`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-c871a0685b710893 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-c871a0685b710893

- Candidate/source: `doc-545c86b2057f9b05` at `docs/specs/core/8-implementation.md:26` (requirement)
- Expected behavior: At docs/specs/core/8-implementation.md:26 under “阶段 C — Tauri 边界(随 Phase 6)” (heading), the source “### 阶段 C — Tauri 边界(随 Phase 6)” requires this exact behavior: The named core implementation stage is integrated with automated coverage.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/core/8-implementation.md:26; signal=heading; heading=阶段 C — Tauri 边界(随 Phase 6); candidate=### 阶段 C — Tauri 边界(随 Phase 6)
  - Expected behavior: The named core implementation stage is integrated with automated coverage. This closes only the promise expressed by “阶段 C — Tauri 边界(随 Phase 6)” in “阶段 C — Tauri 边界(随 Phase 6)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “阶段 C — Tauri 边界(随 Phase 6)” with the scenario below and register test:tools/completion-tests/doc-545c86b2057f9b05.test.mjs#completion_545c86b2057f9b05_the_named_core_implementation_stage_is_integrate
  - Initial state/input/event: freeze the implementation and test inventory referenced by “阶段 C — Tauri 边界(随 Phase 6)” at the audited HEAD, then enumerate every concrete promise within “The named core implementation stage is integrated with automated coverage.” without treating the heading itself as proof.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, use N/A for product mutation; resolve each enumerated promise to a real owning code symbol and exact automated test, leaving any unresolved promise incomplete.
  - Visible/returned assertion: assert a one-to-one inventory with no missing, duplicate, placeholder, or evidence-free promise and emit a deterministic failure naming the unresolved source line.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-545c86b2057f9b05.test.mjs#completion_545c86b2057f9b05_the_named_core_implementation_stage_is_integrate.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-c871a0685b710893 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-c871a0685b710893 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/core/8-implementation.md`, `crates/opentake-core/src/events.rs#EventBus`, `crates/opentake-ops/src/command.rs#EditCommand`, `tools/completion-tests/doc-545c86b2057f9b05.test.mjs#completion_545c86b2057f9b05_the_named_core_implementation_stage_is_integrate` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-c871a0685b710893 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 18: documentation-process (implementation-slice-2b6d33658267b20e)

**Covered records:**
- `requirement-cb9b0f1d9b518ab2` (requirement)

**Files:**
- Modify: `docs/specs/core/8-implementation.md`
- Modify: `crates/opentake-core/src/events.rs#EventBus`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `crates/opentake-agent/tests/completion_0d782ae190a07c02.rs#completion_0d782ae190a07c02_the_named_core_implementation_stage_is_integrate`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-cb9b0f1d9b518ab2 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-cb9b0f1d9b518ab2

- Candidate/source: `doc-0d782ae190a07c02` at `docs/specs/core/8-implementation.md:33` (requirement)
- Expected behavior: At docs/specs/core/8-implementation.md:33 under “阶段 D — Agent 接入点(随 Phase 7,core 仅需就绪)” (heading), the source “### 阶段 D — Agent 接入点(随 Phase 7,core 仅需就绪)” requires this exact behavior: The named core implementation stage is integrated with automated coverage.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/core/8-implementation.md:33; signal=heading; heading=阶段 D — Agent 接入点(随 Phase 7,core 仅需就绪); candidate=### 阶段 D — Agent 接入点(随 Phase 7,core 仅需就绪)
  - Expected behavior: The named core implementation stage is integrated with automated coverage. This closes only the promise expressed by “阶段 D — Agent 接入点(随 Phase 7,core 仅需就绪)” in “阶段 D — Agent 接入点(随 Phase 7,core 仅需就绪)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “阶段 D — Agent 接入点(随 Phase 7,core 仅需就绪)” with the scenario below and register test:crates/opentake-agent/tests/completion_0d782ae190a07c02.rs#completion_0d782ae190a07c02_the_named_core_implementation_stage_is_integrate
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “阶段 D — Agent 接入点(随 Phase 7,core 仅需就绪)” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The named core implementation stage is integrated with automated coverage.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_0d782ae190a07c02.rs#completion_0d782ae190a07c02_the_named_core_implementation_stage_is_integrate.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-cb9b0f1d9b518ab2 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-cb9b0f1d9b518ab2 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/core/8-implementation.md`, `crates/opentake-core/src/events.rs#EventBus`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-agent/tests/completion_0d782ae190a07c02.rs#completion_0d782ae190a07c02_the_named_core_implementation_stage_is_integrate` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-cb9b0f1d9b518ab2 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 19: documentation-process (implementation-slice-a647aac99c5d586e)

**Covered records:**
- `requirement-e553ffb2235c0645` (requirement)

**Files:**
- Modify: `docs/specs/frontend/11-tauri.md`
- Modify: `web/src/__tests__/completion/doc-0c9eda82706e9fdb.test.ts#completion_0c9eda82706e9fdb_implemented_events_binary_transports_and_interac`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-e553ffb2235c0645 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-e553ffb2235c0645

- Candidate/source: `doc-0c9eda82706e9fdb` at `docs/specs/frontend/11-tauri.md:41` (requirement)
- Expected behavior: At docs/specs/frontend/11-tauri.md:41 under “11.2 事件（listen）—— Rust → 前端推送” (heading), the source “### 11.2 事件（listen）—— Rust → 前端推送” requires this exact behavior: Implemented events, binary transports, and interactive seek throttling use the typed Tauri boundary.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/11-tauri.md:41; signal=heading; heading=11.2 事件（listen）—— Rust → 前端推送; candidate=### 11.2 事件（listen）—— Rust → 前端推送
  - Expected behavior: Implemented events, binary transports, and interactive seek throttling use the typed Tauri boundary. This closes only the promise expressed by “11.2 事件（listen）—— Rust → 前端推送” in “11.2 事件（listen）—— Rust → 前端推送”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “11.2 事件（listen）—— Rust → 前端推送” with the scenario below and register test:web/src/__tests__/completion/doc-0c9eda82706e9fdb.test.ts#completion_0c9eda82706e9fdb_implemented_events_binary_transports_and_interac
  - Initial state/input/event: render the exact “11.2 事件（listen）—— Rust → 前端推送” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “11.2 事件（listen）—— Rust → 前端推送”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “Implemented events, binary transports, and interactive seek throttling use the typed Tauri boundary.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-0c9eda82706e9fdb.test.ts#completion_0c9eda82706e9fdb_implemented_events_binary_transports_and_interac.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-e553ffb2235c0645 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-e553ffb2235c0645 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/frontend/11-tauri.md`, `web/src/__tests__/completion/doc-0c9eda82706e9fdb.test.ts#completion_0c9eda82706e9fdb_implemented_events_binary_transports_and_interac` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-e553ffb2235c0645 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 20: documentation-process (implementation-slice-71b134d406e19be5)

**Covered records:**
- `requirement-cc08eca38375beda` (requirement)

**Files:**
- Modify: `docs/specs/frontend/11-tauri.md`
- Modify: `web/src/__tests__/completion/doc-7c03ef08c4591508.test.ts#completion_7c03ef08c4591508_implemented_events_binary_transports_and_interac`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-cc08eca38375beda closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-cc08eca38375beda

- Candidate/source: `doc-7c03ef08c4591508` at `docs/specs/frontend/11-tauri.md:54` (requirement)
- Expected behavior: At docs/specs/frontend/11-tauri.md:54 under “11.3 缩略图 / 波形 / 帧 的传输” (heading), the source “### 11.3 缩略图 / 波形 / 帧 的传输” requires this exact behavior: Implemented events, binary transports, and interactive seek throttling use the typed Tauri boundary.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/11-tauri.md:54; signal=heading; heading=11.3 缩略图 / 波形 / 帧 的传输; candidate=### 11.3 缩略图 / 波形 / 帧 的传输
  - Expected behavior: Implemented events, binary transports, and interactive seek throttling use the typed Tauri boundary. This closes only the promise expressed by “11.3 缩略图 / 波形 / 帧 的传输” in “11.3 缩略图 / 波形 / 帧 的传输”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “11.3 缩略图 / 波形 / 帧 的传输” with the scenario below and register test:web/src/__tests__/completion/doc-7c03ef08c4591508.test.ts#completion_7c03ef08c4591508_implemented_events_binary_transports_and_interac
  - Initial state/input/event: render the exact “11.3 缩略图 / 波形 / 帧 的传输” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “11.3 缩略图 / 波形 / 帧 的传输”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “Implemented events, binary transports, and interactive seek throttling use the typed Tauri boundary.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-7c03ef08c4591508.test.ts#completion_7c03ef08c4591508_implemented_events_binary_transports_and_interac.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-cc08eca38375beda closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-cc08eca38375beda closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/frontend/11-tauri.md`, `web/src/__tests__/completion/doc-7c03ef08c4591508.test.ts#completion_7c03ef08c4591508_implemented_events_binary_transports_and_interac` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-cc08eca38375beda closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 21: documentation-process (implementation-slice-8f8773b38296bae9)

**Covered records:**
- `requirement-ae3a17d6b196737c` (requirement)

**Files:**
- Modify: `docs/specs/frontend/11-tauri.md`
- Modify: `web/src/__tests__/completion/doc-ed92cc8fff6d879e.test.ts#completion_ed92cc8fff6d879e_implemented_events_binary_transports_and_interac`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-ae3a17d6b196737c closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-ae3a17d6b196737c

- Candidate/source: `doc-ed92cc8fff6d879e` at `docs/specs/frontend/11-tauri.md:58` (requirement)
- Expected behavior: At docs/specs/frontend/11-tauri.md:58 under “11.4 命令节流（复刻上游 debounce）” (heading), the source “### 11.4 命令节流（复刻上游 debounce）” requires this exact behavior: Implemented events, binary transports, and interactive seek throttling use the typed Tauri boundary.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/11-tauri.md:58; signal=heading; heading=11.4 命令节流（复刻上游 debounce）; candidate=### 11.4 命令节流（复刻上游 debounce）
  - Expected behavior: Implemented events, binary transports, and interactive seek throttling use the typed Tauri boundary. This closes only the promise expressed by “11.4 命令节流（复刻上游 debounce）” in “11.4 命令节流（复刻上游 debounce）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “11.4 命令节流（复刻上游 debounce）” with the scenario below and register test:web/src/__tests__/completion/doc-ed92cc8fff6d879e.test.ts#completion_ed92cc8fff6d879e_implemented_events_binary_transports_and_interac
  - Initial state/input/event: render the exact “11.4 命令节流（复刻上游 debounce）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “11.4 命令节流（复刻上游 debounce）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “Implemented events, binary transports, and interactive seek throttling use the typed Tauri boundary.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-ed92cc8fff6d879e.test.ts#completion_ed92cc8fff6d879e_implemented_events_binary_transports_and_interac.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-ae3a17d6b196737c closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-ae3a17d6b196737c closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/frontend/11-tauri.md`, `web/src/__tests__/completion/doc-ed92cc8fff6d879e.test.ts#completion_ed92cc8fff6d879e_implemented_events_binary_transports_and_interac` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-ae3a17d6b196737c closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 22: documentation-process (implementation-slice-f9fa78ee0e807004)

**Covered records:**
- `requirement-976e8521b44b129b` (requirement)

**Files:**
- Modify: `docs/specs/frontend/12-data-models.md`
- Modify: `web/src/__tests__/completion/doc-f2c9acf9f0813f19.test.ts#completion_f2c9acf9f0813f19_typescript_models_mirror_serialized_rust_dto_con`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-976e8521b44b129b closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-976e8521b44b129b

- Candidate/source: `doc-f2c9acf9f0813f19` at `docs/specs/frontend/12-data-models.md:1` (requirement)
- Expected behavior: At docs/specs/frontend/12-data-models.md:1 under “数据模型镜像（TS 类型）” (heading), the source “# 数据模型镜像（TS 类型）” requires this exact behavior: TypeScript models mirror serialized Rust DTO contracts.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/12-data-models.md:1; signal=heading; heading=数据模型镜像（TS 类型）; candidate=# 数据模型镜像（TS 类型）
  - Expected behavior: TypeScript models mirror serialized Rust DTO contracts. This closes only the promise expressed by “数据模型镜像（TS 类型）” in “数据模型镜像（TS 类型）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “数据模型镜像（TS 类型）” with the scenario below and register test:web/src/__tests__/completion/doc-f2c9acf9f0813f19.test.ts#completion_f2c9acf9f0813f19_typescript_models_mirror_serialized_rust_dto_con
  - Initial state/input/event: render the exact “数据模型镜像（TS 类型）” surface with a deterministic project, selection, focus, viewport, and enabled/disabled state, then dispatch the pointer, keyboard, drag, dialog, or navigation event stated by “数据模型镜像（TS 类型）”.
  - Code/store/API/Rust effect: at the React event handler, typed store action, and Tauri API call named by the behavior, have the React handler call the typed store/API/Tauri action for “TypeScript models mirror serialized Rust DTO contracts.” exactly once, producing only the specified selection, timeline, layout, or persisted UI-state transition.
  - Visible/returned assertion: assert the exact visible text/control/state/focus result and the returned success or typed failure, including a no-op assertion for disabled, cancelled, or rejected input.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-f2c9acf9f0813f19.test.ts#completion_f2c9acf9f0813f19_typescript_models_mirror_serialized_rust_dto_con.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-976e8521b44b129b closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-976e8521b44b129b closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/frontend/12-data-models.md`, `web/src/__tests__/completion/doc-f2c9acf9f0813f19.test.ts#completion_f2c9acf9f0813f19_typescript_models_mirror_serialized_rust_dto_con` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-976e8521b44b129b closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 23: documentation-process (implementation-slice-7778767012001d9b)

**Covered records:**
- `requirement-1ddb0327de8a0a32` (requirement)

**Files:**
- Modify: `docs/specs/media/1-structure.md`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-1ddb0327de8a0a32 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-1ddb0327de8a0a32

- Candidate/source: `doc-a35679e2cfdbfd28` at `docs/specs/media/1-structure.md:1` (requirement)
- Expected behavior: Media modules remain small and cohesive with no source file over the documented 400-line limit.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Split oversized media implementation files into cohesive probe/decode/encode/thumbnail/waveform/search/transcribe/worker modules while preserving public MediaEngine APIs.
  - No production Rust source under crates/opentake-media/src may exceed the documented 400-line limit after excluding generated data, and dependencies must remain acyclic.
  - Run rustfmt/clippy, all media unit tests, ffmpeg integration fixtures, schema/API compile checks, and compare public rustdoc/API symbols before and after.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-1ddb0327de8a0a32 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-1ddb0327de8a0a32 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/media/1-structure.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-1ddb0327de8a0a32 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

### Task 24: documentation-process (implementation-slice-f25b23eb26f6effa)

**Covered records:**
- `requirement-7316742bb416f57a` (requirement)

**Files:**
- Modify: `docs/specs/media/1-structure.md`
- Modify: `crates/opentake-media/src/**/*.rs`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-7316742bb416f57a closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-7316742bb416f57a

- Candidate/source: `doc-ccafda95c3c24233` at `docs/specs/media/1-structure.md:3` (requirement)
- Expected behavior: Media modules remain small and cohesive with no source file over the documented 400-line limit.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Move each >400-line production module into named submodules with one responsibility and no duplicated parsing/cache/error logic.
  - Preserve module visibility, serialized types, error variants, cache keys, and downstream imports without behavior changes.
  - Add a CI line-count check for crates/opentake-media/src/**/*.rs and run media/render/src-tauri dependent tests with zero source file above 400 lines.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-7316742bb416f57a closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-7316742bb416f57a closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/media/1-structure.md`, `crates/opentake-media/src/**/*.rs` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-7316742bb416f57a closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 25: documentation-process (implementation-slice-9a6958f870ec0636)

**Covered records:**
- `requirement-fa9a94bbfd92c91a` (requirement)

**Files:**
- Modify: `docs/specs/media/1-structure.md`
- Modify: `crates/opentake-media/src/probe.rs#MediaProbe`
- Modify: `tools/completion-tests/doc-5cfb34af6436a866.test.mjs#completion_5cfb34af6436a866_media_dependency_error_and_cache_key_contracts_a`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-fa9a94bbfd92c91a closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-fa9a94bbfd92c91a

- Candidate/source: `doc-5cfb34af6436a866` at `docs/specs/media/1-structure.md:50` (requirement)
- Expected behavior: At docs/specs/media/1-structure.md:50 under “1.2 Cargo 依赖(建议版本范围,实施时锁定)” (heading), the source “## 1.2 Cargo 依赖(建议版本范围,实施时锁定)” requires this exact behavior: Media dependency, error, and cache-key contracts are implemented.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/media/1-structure.md:50; signal=heading; heading=1.2 Cargo 依赖(建议版本范围,实施时锁定); candidate=## 1.2 Cargo 依赖(建议版本范围,实施时锁定)
  - Expected behavior: Media dependency, error, and cache-key contracts are implemented. This closes only the promise expressed by “1.2 Cargo 依赖(建议版本范围,实施时锁定)” in “1.2 Cargo 依赖(建议版本范围,实施时锁定)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.2 Cargo 依赖(建议版本范围,实施时锁定)” with the scenario below and register test:tools/completion-tests/doc-5cfb34af6436a866.test.mjs#completion_5cfb34af6436a866_media_dependency_error_and_cache_key_contracts_a
  - Initial state/input/event: start from the smallest valid fixture for “Media dependency, error, and cache-key contracts are implemented.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “1.2 Cargo 依赖(建议版本范围,实施时锁定)”.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Media dependency, error, and cache-key contracts are implemented.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-5cfb34af6436a866.test.mjs#completion_5cfb34af6436a866_media_dependency_error_and_cache_key_contracts_a.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-fa9a94bbfd92c91a closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-fa9a94bbfd92c91a closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/media/1-structure.md`, `crates/opentake-media/src/probe.rs#MediaProbe`, `tools/completion-tests/doc-5cfb34af6436a866.test.mjs#completion_5cfb34af6436a866_media_dependency_error_and_cache_key_contracts_a` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-fa9a94bbfd92c91a closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 26: documentation-process (implementation-slice-fe76231912aef344)

**Covered records:**
- `requirement-446787e5ad94cd64` (requirement)

**Files:**
- Modify: `docs/specs/media/1-structure.md`
- Modify: `crates/opentake-media/src/probe.rs#MediaProbe`
- Modify: `tools/completion-tests/doc-4d1d2a83b559d148.test.mjs#completion_4d1d2a83b559d148_media_dependency_error_and_cache_key_contracts_a`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-446787e5ad94cd64 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-446787e5ad94cd64

- Candidate/source: `doc-4d1d2a83b559d148` at `docs/specs/media/1-structure.md:95` (requirement)
- Expected behavior: At docs/specs/media/1-structure.md:95 under “1.3 顶层错误类型” (heading), the source “## 1.3 顶层错误类型” requires this exact behavior: Media dependency, error, and cache-key contracts are implemented.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/media/1-structure.md:95; signal=heading; heading=1.3 顶层错误类型; candidate=## 1.3 顶层错误类型
  - Expected behavior: Media dependency, error, and cache-key contracts are implemented. This closes only the promise expressed by “1.3 顶层错误类型” in “1.3 顶层错误类型”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.3 顶层错误类型” with the scenario below and register test:tools/completion-tests/doc-4d1d2a83b559d148.test.mjs#completion_4d1d2a83b559d148_media_dependency_error_and_cache_key_contracts_a
  - Initial state/input/event: start from the smallest valid fixture for “Media dependency, error, and cache-key contracts are implemented.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “1.3 顶层错误类型”.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Media dependency, error, and cache-key contracts are implemented.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-4d1d2a83b559d148.test.mjs#completion_4d1d2a83b559d148_media_dependency_error_and_cache_key_contracts_a.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-446787e5ad94cd64 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-446787e5ad94cd64 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/media/1-structure.md`, `crates/opentake-media/src/probe.rs#MediaProbe`, `tools/completion-tests/doc-4d1d2a83b559d148.test.mjs#completion_4d1d2a83b559d148_media_dependency_error_and_cache_key_contracts_a` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-446787e5ad94cd64 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 27: documentation-process (implementation-slice-dc53ce6f237399a5)

**Covered records:**
- `requirement-06118da74632e6da` (requirement)

**Files:**
- Modify: `docs/specs/media/1-structure.md`
- Modify: `crates/opentake-media/src/probe.rs#MediaProbe`
- Modify: `tools/completion-tests/doc-586aa5a5055a7c81.test.mjs#completion_586aa5a5055a7c81_media_dependency_error_and_cache_key_contracts_a`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-06118da74632e6da closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-06118da74632e6da

- Candidate/source: `doc-586aa5a5055a7c81` at `docs/specs/media/1-structure.md:119` (requirement)
- Expected behavior: At docs/specs/media/1-structure.md:119 under “1.4 通用缓存键(三处共用)” (heading), the source “## 1.4 通用缓存键(三处共用)” requires this exact behavior: Media dependency, error, and cache-key contracts are implemented.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/media/1-structure.md:119; signal=heading; heading=1.4 通用缓存键(三处共用); candidate=## 1.4 通用缓存键(三处共用)
  - Expected behavior: Media dependency, error, and cache-key contracts are implemented. This closes only the promise expressed by “1.4 通用缓存键(三处共用)” in “1.4 通用缓存键(三处共用)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.4 通用缓存键(三处共用)” with the scenario below and register test:tools/completion-tests/doc-586aa5a5055a7c81.test.mjs#completion_586aa5a5055a7c81_media_dependency_error_and_cache_key_contracts_a
  - Initial state/input/event: start from the smallest valid fixture for “Media dependency, error, and cache-key contracts are implemented.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “1.4 通用缓存键(三处共用)”.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Media dependency, error, and cache-key contracts are implemented.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-586aa5a5055a7c81.test.mjs#completion_586aa5a5055a7c81_media_dependency_error_and_cache_key_contracts_a.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-06118da74632e6da closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-06118da74632e6da closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/media/1-structure.md`, `crates/opentake-media/src/probe.rs#MediaProbe`, `tools/completion-tests/doc-586aa5a5055a7c81.test.mjs#completion_586aa5a5055a7c81_media_dependency_error_and_cache_key_contracts_a` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-06118da74632e6da closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 28: documentation-process (implementation-slice-b10e03b9fa89d229)

**Covered records:**
- `requirement-68a9a7e616a6af7e` (requirement)

**Files:**
- Modify: `docs/specs/media/1-structure.md`
- Modify: `crates/opentake-media/src/search/embed_store.rs#cache_key`
- Modify: `tools/completion-tests/doc-b281200afefb5385.test.mjs#completion_b281200afefb5385_use_the_first_16_sha_256_bytes_as_a_32_lowercase`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-68a9a7e616a6af7e closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-68a9a7e616a6af7e

- Candidate/source: `doc-b281200afefb5385` at `docs/specs/media/1-structure.md:132` (requirement)
- Expected behavior: At docs/specs/media/1-structure.md:132 under “1.4 通用缓存键(三处共用)” (gap-marker), the source “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。” requires this exact behavior: Use the first 16 SHA-256 bytes as a 32-lowercase-hex file identity key and return None when metadata is missing.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/media/1-structure.md:132; signal=gap-marker; heading=1.4 通用缓存键(三处共用); candidate=> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。
  - Expected behavior: Use the first 16 SHA-256 bytes as a 32-lowercase-hex file identity key and return None when metadata is missing. This closes only the promise expressed by “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。” in “1.4 通用缓存键(三处共用)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。” with the scenario below and register test:tools/completion-tests/doc-b281200afefb5385.test.mjs#completion_b281200afefb5385_use_the_first_16_sha_256_bytes_as_a_32_lowercase
  - Initial state/input/event: start from the smallest valid fixture for “Use the first 16 SHA-256 bytes as a 32-lowercase-hex file identity key and return None when metadata is missing.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。”.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Use the first 16 SHA-256 bytes as a 32-lowercase-hex file identity key and return None when metadata is missing.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-b281200afefb5385.test.mjs#completion_b281200afefb5385_use_the_first_16_sha_256_bytes_as_a_32_lowercase.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-68a9a7e616a6af7e closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-68a9a7e616a6af7e closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/media/1-structure.md`, `crates/opentake-media/src/search/embed_store.rs#cache_key`, `tools/completion-tests/doc-b281200afefb5385.test.mjs#completion_b281200afefb5385_use_the_first_16_sha_256_bytes_as_a_32_lowercase` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-68a9a7e616a6af7e closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 29: documentation-process (implementation-slice-5fd437b30ca38722)

**Covered records:**
- `requirement-11b53ef2ccad53e3` (requirement)

**Files:**
- Modify: `docs/specs/media/6-transcribe.md`
- Modify: `crates/opentake-render/tests/completion_e1802949ad93969f.rs#completion_e1802949ad93969f_allow_optional_word_timestamps_and_manage_a_down`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-11b53ef2ccad53e3 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-11b53ef2ccad53e3

- Candidate/source: `doc-e1802949ad93969f` at `docs/specs/media/6-transcribe.md:61` (requirement)
- Expected behavior: At docs/specs/media/6-transcribe.md:61 under “6.2 转写后端 trait + whisper 实现” (gap-marker), the source “> ⚠️ whisper 词级时间戳精度低于 Apple `audioTimeRange`;`TranscriptionWord.start/end` 为 `Option` 已容许缺失。模型权重(ggml/gguf,如 `base`/`small` 多语种)由 §5.9 同款下载器或单独 catalog 管理(记入 T8.0)。” requires this exact behavior: Allow optional word timestamps and manage a downloadable multilingual whisper model with integrity checks.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/media/6-transcribe.md:61; signal=gap-marker; heading=6.2 转写后端 trait + whisper 实现; candidate=> ⚠️ whisper 词级时间戳精度低于 Apple `audioTimeRange`;`TranscriptionWord.start/end` 为 `Option` 已容许缺失。模型权重(ggml/gguf,如 `base`/`small` 多语种)由 §5.9 同款下载器或单独 catalog 管理(记入 T8.0)。
  - Expected behavior: Allow optional word timestamps and manage a downloadable multilingual whisper model with integrity checks. This closes only the promise expressed by “> ⚠️ whisper 词级时间戳精度低于 Apple `audioTimeRange`;`TranscriptionWord.start/end` 为 `Option` 已容许缺失。模型权重(ggml/gguf,如 `base`/`small` 多语种)由 §5.9 同款下载器或单独 catalog 管理(记入 T8.0)。” in “6.2 转写后端 trait + whisper 实现”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “> ⚠️ whisper 词级时间戳精度低于 Apple `audioTimeRange`;`TranscriptionWord.start/end` 为 `Option` 已容许缺失。模型权重(ggml/gguf,如 `base`/`small` 多语种)由 §5.9 同款下载器或单独 catalog 管理(记入 T8.0)。” with the scenario below and register test:crates/opentake-render/tests/completion_e1802949ad93969f.rs#completion_e1802949ad93969f_allow_optional_word_timestamps_and_manage_a_down
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “> ⚠️ whisper 词级时间戳精度低于 Apple `audioTimeRange`;`TranscriptionWord.start/end` 为 `Option` 已容许缺失。模型权重(ggml/gguf,如 `base`/`small` 多语种)由 §5.9 同款下载器或单独 catalog 管理(记入 T8.0)。”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust media/render/playback pipeline and its typed preview/export adapter, route “Allow optional word timestamps and manage a downloadable multilingual whisper model with integrity checks.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-render/tests/completion_e1802949ad93969f.rs#completion_e1802949ad93969f_allow_optional_word_timestamps_and_manage_a_down.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-11b53ef2ccad53e3 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-11b53ef2ccad53e3 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/media/6-transcribe.md`, `crates/opentake-render/tests/completion_e1802949ad93969f.rs#completion_e1802949ad93969f_allow_optional_word_timestamps_and_manage_a_down` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-11b53ef2ccad53e3 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 30: documentation-process (implementation-slice-46f0e7bf2bf0a81b)

**Covered records:**
- `requirement-f8c265189c0539f4` (requirement)

**Files:**
- Modify: `docs/specs/media/7-ort-worker.md`
- Modify: `tools/completion-tests/doc-f61a77f55483a296.test.mjs#completion_f61a77f55483a296_heavy_inference_has_a_shared_model_abstraction`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-f8c265189c0539f4 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-f8c265189c0539f4

- Candidate/source: `doc-f61a77f55483a296` at `docs/specs/media/7-ort-worker.md:5` (requirement)
- Expected behavior: At docs/specs/media/7-ort-worker.md:5 under “7.1 通用模型抽象” (heading), the source “## 7.1 通用模型抽象” requires this exact behavior: Heavy inference has a shared model abstraction.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Source binding: docs/specs/media/7-ort-worker.md:5; signal=heading; heading=7.1 通用模型抽象; candidate=## 7.1 通用模型抽象
  - Expected behavior: Heavy inference has a shared model abstraction. This closes only the promise expressed by “7.1 通用模型抽象” in “7.1 通用模型抽象”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “7.1 通用模型抽象” with the scenario below and register test:tools/completion-tests/doc-f61a77f55483a296.test.mjs#completion_f61a77f55483a296_heavy_inference_has_a_shared_model_abstraction
  - Initial state/input/event: freeze the implementation and test inventory referenced by “7.1 通用模型抽象” at the audited HEAD, then enumerate every concrete promise within “Heavy inference has a shared model abstraction.” without treating the heading itself as proof.
  - Code/store/API/Rust effect: at N/A for prose mutation; the audit must instead enumerate each concrete implementation claim and bind it to code plus tests, use N/A for product mutation; resolve each enumerated promise to a real owning code symbol and exact automated test, leaving any unresolved promise incomplete.
  - Visible/returned assertion: assert a one-to-one inventory with no missing, duplicate, placeholder, or evidence-free promise and emit a deterministic failure naming the unresolved source line.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-f61a77f55483a296.test.mjs#completion_f61a77f55483a296_heavy_inference_has_a_shared_model_abstraction.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-f8c265189c0539f4 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-f8c265189c0539f4 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/specs/media/7-ort-worker.md`, `tools/completion-tests/doc-f61a77f55483a296.test.mjs#completion_f61a77f55483a296_heavy_inference_has_a_shared_model_abstraction` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-f8c265189c0539f4 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

### Task 31: documentation-process (implementation-slice-80c6958ab3667e14)

**Covered records:**
- `requirement-94569c1419a6e4f3` (requirement)
- `requirement-90195cdaeb8cfa3f` (requirement)
- `requirement-b53aabb53c037778` (requirement)
- `requirement-fec91c0886a4a183` (requirement)
- `requirement-0fe59d47bc305446` (requirement)
- `requirement-044c028b44e60ffe` (requirement)
- `requirement-f537395c97c196c7` (requirement)
- `requirement-08dd6b7227f8a43d` (requirement)

**Files:**
- Modify: `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-94569c1419a6e4f3 closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-90195cdaeb8cfa3f closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-b53aabb53c037778 closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-fec91c0886a4a183 closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-0fe59d47bc305446 closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-044c028b44e60ffe closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-f537395c97c196c7 closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-08dd6b7227f8a43d closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-94569c1419a6e4f3

- Candidate/source: `doc-47e496ad59d5eba8` at `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md:773` (requirement)
- Expected behavior: Step 8: Independent audit review
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Fail-closed verifier tests cover missing inventory entries, duplicate IDs, unverified records, unsupported complete records, and unplanned incomplete records.
  - The verifier returns nonzero with stable IDs and exact paths for every invalid fixture.
  - Final inventories are regenerated after tracked audit files settle, and completion-ledger.json, completion-report.md, and implementation-plans/INDEX.md are produced.
  - All audit scopes and the independent exact-tree audit review pass.

#### requirement-90195cdaeb8cfa3f

- Candidate/source: `doc-2f7a726381650929` at `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md:777` (requirement)
- Expected behavior: Step 9: Commit the verified audit
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Fail-closed verifier tests cover missing inventory entries, duplicate IDs, unverified records, unsupported complete records, and unplanned incomplete records.
  - The verifier returns nonzero with stable IDs and exact paths for every invalid fixture.
  - Final inventories are regenerated after tracked audit files settle, and completion-ledger.json, completion-report.md, and implementation-plans/INDEX.md are produced.
  - All audit scopes and the independent exact-tree audit review pass.

#### requirement-b53aabb53c037778

- Candidate/source: `doc-70a2a0bd8648ff27` at `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md:796` (requirement)
- Expected behavior: Step 1: Execute plans in dependency order
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - All accepted subsystem implementation plans are executed in dependency order with focused regression tests.
  - Full Rust, Web, lint, format, build, browser/native, and repository-defined CI gates pass on one exact tree.
  - Affected requirements and controls are re-audited with no unverified record or unsupported completion claim.
  - A final independent exact-tree review passes and the reviewed branch is published without history rewriting.

#### requirement-fec91c0886a4a183

- Candidate/source: `doc-a922bcf431d0dd5f` at `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md:800` (requirement)
- Expected behavior: Step 2: Run focused tests before and after every change
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - All accepted subsystem implementation plans are executed in dependency order with focused regression tests.
  - Full Rust, Web, lint, format, build, browser/native, and repository-defined CI gates pass on one exact tree.
  - Affected requirements and controls are re-audited with no unverified record or unsupported completion claim.
  - A final independent exact-tree review passes and the reviewed branch is published without history rewriting.

#### requirement-0fe59d47bc305446

- Candidate/source: `doc-308ca9bbd62c3362` at `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md:804` (requirement)
- Expected behavior: Step 3: Re-audit affected controls and requirements after every subsystem
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - All accepted subsystem implementation plans are executed in dependency order with focused regression tests.
  - Full Rust, Web, lint, format, build, browser/native, and repository-defined CI gates pass on one exact tree.
  - Affected requirements and controls are re-audited with no unverified record or unsupported completion claim.
  - A final independent exact-tree review passes and the reviewed branch is published without history rewriting.

#### requirement-044c028b44e60ffe

- Candidate/source: `doc-da3d2c2a1507daf9` at `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md:808` (requirement)
- Expected behavior: Step 4: Run full product verification
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - All accepted subsystem implementation plans are executed in dependency order with focused regression tests.
  - Full Rust, Web, lint, format, build, browser/native, and repository-defined CI gates pass on one exact tree.
  - Affected requirements and controls are re-audited with no unverified record or unsupported completion claim.
  - A final independent exact-tree review passes and the reviewed branch is published without history rewriting.

#### requirement-f537395c97c196c7

- Candidate/source: `doc-2005c7887e722be8` at `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md:822` (requirement)
- Expected behavior: Step 5: Final independent exact-tree review
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - All accepted subsystem implementation plans are executed in dependency order with focused regression tests.
  - Full Rust, Web, lint, format, build, browser/native, and repository-defined CI gates pass on one exact tree.
  - Affected requirements and controls are re-audited with no unverified record or unsupported completion claim.
  - A final independent exact-tree review passes and the reviewed branch is published without history rewriting.

#### requirement-08dd6b7227f8a43d

- Candidate/source: `doc-89ae51c38e447fac` at `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md:826` (requirement)
- Expected behavior: Step 6: Publish without rewriting history
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - All accepted subsystem implementation plans are executed in dependency order with focused regression tests.
  - Full Rust, Web, lint, format, build, browser/native, and repository-defined CI gates pass on one exact tree.
  - Affected requirements and controls are re-audited with no unverified record or unsupported completion claim.
  - A final independent exact-tree review passes and the reviewed branch is published without history rewriting.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-94569c1419a6e4f3 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-90195cdaeb8cfa3f closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-b53aabb53c037778 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-fec91c0886a4a183 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-0fe59d47bc305446 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-044c028b44e60ffe closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-f537395c97c196c7 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-08dd6b7227f8a43d closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-94569c1419a6e4f3 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-90195cdaeb8cfa3f closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-b53aabb53c037778 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-fec91c0886a4a183 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-0fe59d47bc305446 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-044c028b44e60ffe closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-f537395c97c196c7 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-08dd6b7227f8a43d closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 8 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-94569c1419a6e4f3 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-90195cdaeb8cfa3f closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-b53aabb53c037778 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-fec91c0886a4a183 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-0fe59d47bc305446 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-044c028b44e60ffe closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-f537395c97c196c7 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-08dd6b7227f8a43d closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.

### Task 32: documentation-process (implementation-slice-63a11bede65392fa)

**Covered records:**
- `requirement-bbbed7ce4235be62` (requirement)
- `requirement-94e98db4bdd7b539` (requirement)
- `requirement-21d73a4f5676e25a` (requirement)
- `requirement-0705afa91d4b6fe1` (requirement)

**Files:**
- Modify: `docs/superpowers/specs/2026-07-14-opentake-completion-audit-design.md`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-bbbed7ce4235be62 closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-94e98db4bdd7b539 closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-21d73a4f5676e25a closes documented acceptance contract`
- Test (reviewed-planned): `tools/completion-audit.test.mjs#requirement-0705afa91d4b6fe1 closes documented acceptance contract`

**Candidate-bound contracts:**

#### requirement-bbbed7ce4235be62

- Candidate/source: `doc-51ed8934ad7c228a` at `docs/superpowers/specs/2026-07-14-opentake-completion-audit-design.md:123` (requirement)
- Expected behavior: For source files, extract public interfaces, commands, events, stores, TODO or
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Every tracked source file has deterministic extracted records for public interfaces, commands, events, stores, TODO/stub markers, disabled branches, feature/platform gates, and tests where applicable.
  - Extraction is parser-backed or otherwise language-aware for supported languages and records an explicit parse limitation instead of silently omitting a file.
  - Focused tests cover representative Rust, TypeScript/TSX, JavaScript, shell, workflow, and configuration inputs.
  - The repository-file verifier rejects any tracked source file missing the required source-surface evidence.

#### requirement-94e98db4bdd7b539

- Candidate/source: `doc-3354e0adc2908d38` at `docs/superpowers/specs/2026-07-14-opentake-completion-audit-design.md:124` (requirement)
- Expected behavior: stub markers, disabled branches, feature gates, platform gates, and tests. For
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Every tracked source file has deterministic extracted records for public interfaces, commands, events, stores, TODO/stub markers, disabled branches, feature/platform gates, and tests where applicable.
  - Extraction is parser-backed or otherwise language-aware for supported languages and records an explicit parse limitation instead of silently omitting a file.
  - Focused tests cover representative Rust, TypeScript/TSX, JavaScript, shell, workflow, and configuration inputs.
  - The repository-file verifier rejects any tracked source file missing the required source-surface evidence.

#### requirement-21d73a4f5676e25a

- Candidate/source: `doc-94f8aeebef68d331` at `docs/superpowers/specs/2026-07-14-opentake-completion-audit-design.md:199` (requirement)
- Expected behavior: an accurate reason. Placeholder panels cannot be counted as completed features.
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - Every tracked TSX interaction control has a static handler-to-state/backend trace and a deterministic behavior test or runtime receipt.
  - Browser-capable paths and native-only high-risk paths are exercised, including keyboard and accessibility behavior.
  - Silent no-ops, placeholder panels, missing handlers, and unproven state are fixed or remain explicitly incomplete.
  - The control-scope verifier exits 0 and an independent reviewer approves the exact tree.

#### requirement-0705afa91d4b6fe1

- Candidate/source: `doc-579c525816faf664` at `docs/superpowers/specs/2026-07-14-opentake-completion-audit-design.md:260` (requirement)
- Expected behavior: - no production stub, placeholder control, silent no-op, or unsupported claim
- Resolution: `documentation-process` — Documentation/process closure is verified by the tracked completion-audit runner.
- Exact acceptance contract:
  - All accepted subsystem implementation plans are executed in dependency order with focused regression tests.
  - Full Rust, Web, lint, format, build, browser/native, and repository-defined CI gates pass on one exact tree.
  - Affected requirements and controls are re-audited with no unverified record or unsupported completion claim.
  - A final independent exact-tree review passes and the reviewed branch is published without history rewriting.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `tools/completion-audit.test.mjs#requirement-bbbed7ce4235be62 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-94e98db4bdd7b539 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-21d73a4f5676e25a closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.
  - `tools/completion-audit.test.mjs#requirement-0705afa91d4b6fe1 closes documented acceptance contract` (reviewed-planned) — Documentation/process closure is verified by the tracked completion-audit runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `node --test --test-name-pattern="requirement-bbbed7ce4235be62 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-94e98db4bdd7b539 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-21d73a4f5676e25a closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-0705afa91d4b6fe1 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `docs/superpowers/specs/2026-07-14-opentake-completion-audit-design.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `node --test --test-name-pattern="requirement-bbbed7ce4235be62 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-94e98db4bdd7b539 closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-21d73a4f5676e25a closes documented acceptance contract" tools/completion-audit.test.mjs`
  - Run: `node --test --test-name-pattern="requirement-0705afa91d4b6fe1 closes documented acceptance contract" tools/completion-audit.test.mjs`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `node --test tools/completion-audit.test.mjs`

  Expected: PASS with no new warnings or unrelated changes.
