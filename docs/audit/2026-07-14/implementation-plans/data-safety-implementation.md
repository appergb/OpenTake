# Data Safety Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 32 verified incomplete records in the `data-safety` gap group.

**Architecture:** Implement 14 primary evidence-bound slices and reference 0 shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.

---

### Task 1: DS-legacy-default-matrix (implementation-slice-0e3d4d7415150b82)

**Covered records:**
- `requirement-365ac4943b157d3e` (requirement)

**Files:**
- Modify: `crates/opentake-project/src/bundle.rs#Project`
- Modify: `crates/opentake-domain/src/clip.rs#Clip`
- Modify: `crates/opentake-domain/src/media.rs#MediaManifest`
- Modify: `crates/opentake-project/src/gen_log.rs#GenerationLogEntry`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Test (existing-owned): `crates/opentake-project/tests/upstream_compat.rs#applies_clip_defaults_for_omitted_fields`
- Test (existing-owned): `crates/opentake-project/tests/upstream_compat.rs#migrates_legacy_transform_xy_to_center`
- Test (existing-owned): `crates/opentake-project/tests/upstream_compat.rs#migrates_generation_log_legacy_cost_and_version`
- Test (reviewed-planned): `crates/opentake-project/tests/upstream_compat.rs#exhaustive_legacy_default_matrix`

**Candidate-bound contracts:**

#### requirement-365ac4943b157d3e

- Candidate/source: `doc-4e47fc5b7ea3e13a` at `docs/architecture/MODULE-PORT-MAP.md:145` (requirement)
- Expected behavior: Decode every listed upstream legacy/default field migration without data loss.
- Resolution: `reviewed-mapping-report:DS-legacy-default-matrix` — Core mapping report DS-legacy-default-matrix: existing migration tests do not cover the exhaustive documented field plus save/reopen matrix.
- Exact acceptance contract:
  - Implementation: Create fixtures covering every listed missing Timeline/Track/Clip/Manifest field, legacy Transform x/y migration, and GenerationLog cost conversion; make decoding match the documented defaults; block unsafe writes on unknown future fields; pass round-trip compatibility tests.
  - Add deterministic fixtures for every named current, legacy, missing, malformed, and fail-closed branch; the focused round-trip and compatibility suites must pass.
  - Exercise open, edit, save, and reopen on representative bundles and attach the exact implementation symbols plus test or runtime evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-project/tests/upstream_compat.rs#applies_clip_defaults_for_omitted_fields` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/upstream_compat.rs#migrates_legacy_transform_xy_to_center` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/upstream_compat.rs#migrates_generation_log_legacy_cost_and_version` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/upstream_compat.rs#exhaustive_legacy_default_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-project --test upstream_compat applies_clip_defaults_for_omitted_fields -- --exact`
  - Run: `cargo test -p opentake-project --test upstream_compat migrates_legacy_transform_xy_to_center -- --exact`
  - Run: `cargo test -p opentake-project --test upstream_compat migrates_generation_log_legacy_cost_and_version -- --exact`
  - Run: `cargo test -p opentake-project --test upstream_compat exhaustive_legacy_default_matrix -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-domain/src/clip.rs#Clip`, `crates/opentake-domain/src/media.rs#MediaManifest`, `crates/opentake-project/src/gen_log.rs#GenerationLogEntry`, `docs/architecture/MODULE-PORT-MAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-project --test upstream_compat applies_clip_defaults_for_omitted_fields -- --exact`
  - Run: `cargo test -p opentake-project --test upstream_compat migrates_legacy_transform_xy_to_center -- --exact`
  - Run: `cargo test -p opentake-project --test upstream_compat migrates_generation_log_legacy_cost_and_version -- --exact`
  - Run: `cargo test -p opentake-project --test upstream_compat exhaustive_legacy_default_matrix -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Completion evidence: [`data-safety-legacy-default-matrix-real-device-2026-07-31.md`](../runtime-artifacts/automated/data-safety-legacy-default-matrix-real-device-2026-07-31.md). The four exact owning tests already passed at the initial baseline, so Task 1 required evidence closure and a packaged-app round trip rather than a new production patch; no artificial RED failure was introduced.

### Task 2: DS-mcp-transport (implementation-slice-078ed22bfa23f28a)

**Covered records:**
- `requirement-473d4379da3bd4cc` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/server.rs#McpServer`
- Modify: `crates/opentake-agent/src/mcp/server.rs#serve_with_bridge`
- Modify: `docs/modules/opentake-agent/SPEC.md`
- Test (existing-owned): `crates/opentake-agent/tests/mcp_http.rs#non_local_origin_is_rejected`
- Test (existing-owned): `crates/opentake-agent/tests/mcp_http.rs#oversized_request_body_is_rejected`
- Test (reviewed-planned): `crates/opentake-agent/tests/mcp_http.rs#serve_rejects_non_loopback_bind`
- Test (reviewed-planned): `crates/opentake-agent/tests/mcp_http.rs#unsupported_protocol_version_is_400`

**Candidate-bound contracts:**

#### requirement-473d4379da3bd4cc

- Candidate/source: `doc-c97237b55fcc36c9` at `docs/modules/opentake-agent/SPEC.md:1069` (requirement)
- Expected behavior: Enforce the complete MCP transport guard set on a loopback-only listener.
- Resolution: `reviewed-mapping-report:DS-mcp-transport` — Core mapping report DS-mcp-transport: loopback bind and protocol-version rejection remain unproved production constraints.
- Exact acceptance contract:
  - Server startup rejects non-loopback bind addresses, not merely the default constant.
  - Origin and Host DNS-rebinding guards, request-size limit, and MCP protocol-version validation are all active and independently tested.
  - HTTP integration tests prove external Host/Origin and unsupported protocol versions never invoke a tool.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/mcp_http.rs#non_local_origin_is_rejected` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#oversized_request_body_is_rejected` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#serve_rejects_non_loopback_bind` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/tests/mcp_http.rs#unsupported_protocol_version_is_400` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test mcp_http non_local_origin_is_rejected -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_http oversized_request_body_is_rejected -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_http serve_rejects_non_loopback_bind -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_http unsupported_protocol_version_is_400 -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/server.rs#McpServer`, `crates/opentake-agent/src/mcp/server.rs#serve_with_bridge`, `docs/modules/opentake-agent/SPEC.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test mcp_http non_local_origin_is_rejected -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_http oversized_request_body_is_rejected -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_http serve_rejects_non_loopback_bind -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_http unsupported_protocol_version_is_400 -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Completion evidence: [`data-safety-mcp-transport-real-device-2026-07-31.md`](../runtime-artifacts/automated/data-safety-mcp-transport-real-device-2026-07-31.md). The four exact owning tests already passed at the initial baseline, so Task 2 required packaged-server verification and evidence closure rather than a new production patch; no artificial RED failure was introduced.

### Task 3: DS-mcp-tool-import (implementation-slice-a5dcb81bd0966174)

**Covered records:**
- `requirement-d317ca3e45fba737` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/tools/errors.rs#decode_tool_args`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`
- Modify: `src-tauri/src/mcp.rs#TauriMediaBridge`
- Modify: `docs/modules/opentake-agent/SPEC.md`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#import_media_requires_exactly_one_source`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#import_media_rejects_unknown_nested_source_key`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#import_media_bytes_rejects_oversized_base64_before_bridge`
- Test (reviewed-planned): `crates/opentake-agent/tests/tool_argument_contract.rs#all_tool_schemas_reject_unknown_missing_wrong_type`
- Test (reviewed-planned): `src-tauri/src/mcp.rs#https_url_import_enforces_scheme_mime_and_decoded_limit`

**Candidate-bound contracts:**

#### requirement-d317ca3e45fba737

- Candidate/source: `doc-122beba700691cb7` at `docs/modules/opentake-agent/SPEC.md:1071` (requirement)
- Expected behavior: Validate every tool argument and make URL media import HTTPS-only, whitelist typed media, and stream with a hard 1 GB decoded-byte ceiling.
- Resolution: `reviewed-mapping-report:DS-mcp-tool-import` — Core mapping report DS-mcp-tool-import: HTTPS-only streaming, typed MIME and decoded-size enforcement are not wired through the bridge.
- Exact acceptance contract:
  - Complete the nested argument guards described by doc-d1dee193811e9dbb.
  - Reject non-HTTPS/userinfo/redirect-to-non-HTTPS URLs before I/O; infer or validate the extension/MIME allowlist; stream to a staging file while enforcing the 1 GB decoded-byte cap across redirects.
  - Publish only after download, type/probe, and retained-project validation; cancellation/error cleans staging and leaves manifest unchanged.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_requires_exactly_one_source` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_rejects_unknown_nested_source_key` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_bytes_rejects_oversized_base64_before_bridge` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/tool_argument_contract.rs#all_tool_schemas_reject_unknown_missing_wrong_type` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `src-tauri/src/mcp.rs#https_url_import_enforces_scheme_mime_and_decoded_limit` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent import_media_requires_exactly_one_source`
  - Run: `cargo test -p opentake-agent import_media_rejects_unknown_nested_source_key`
  - Run: `cargo test -p opentake-agent import_media_bytes_rejects_oversized_base64_before_bridge`
  - Run: `cargo test -p opentake-agent --test tool_argument_contract all_tool_schemas_reject_unknown_missing_wrong_type -- --exact`
  - Run: `cargo test -p opentake-tauri https_url_import_enforces_scheme_mime_and_decoded_limit`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/tools/errors.rs#decode_tool_args`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `src-tauri/src/mcp.rs#TauriMediaBridge`, `docs/modules/opentake-agent/SPEC.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent import_media_requires_exactly_one_source`
  - Run: `cargo test -p opentake-agent import_media_rejects_unknown_nested_source_key`
  - Run: `cargo test -p opentake-agent import_media_bytes_rejects_oversized_base64_before_bridge`
  - Run: `cargo test -p opentake-agent --test tool_argument_contract all_tool_schemas_reject_unknown_missing_wrong_type -- --exact`
  - Run: `cargo test -p opentake-tauri https_url_import_enforces_scheme_mime_and_decoded_limit`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Completion evidence: [`data-safety-mcp-tool-import-real-device-2026-07-31.md`](../runtime-artifacts/automated/data-safety-mcp-tool-import-real-device-2026-07-31.md). The initial exact argument-matrix command executed zero tests because its owning test had an extra suffix; the test was renamed to the plan-declared contract and then passed 1/1. Production import guards were already complete, and the packaged server passed path, bytes, HTTPS, rejection, persistence, and UI checks.

### Task 4: DS-mcp-redaction (implementation-slice-673f9e3f6002f97b)

**Covered records:**
- `requirement-2e9e6066655d5846` (requirement)

**Files:**
- Modify: `src-tauri/src/mcp.rs#TauriMediaBridge`
- Modify: `crates/opentake-agent/src/mcp/convert.rs#to_call_tool_result`
- Modify: `docs/modules/opentake-agent/SPEC.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/mcp_error_redaction.rs#llm_errors_redact_paths_credentials_headers_provider_bodies`

**Candidate-bound contracts:**

#### requirement-2e9e6066655d5846

- Candidate/source: `doc-e13f2cbc316a11fa` at `docs/modules/opentake-agent/SPEC.md:1072` (requirement)
- Expected behavior: Return actionable MCP errors without exposing filesystem paths, credentials, authorization headers, provider response bodies, or internal stack detail.
- Resolution: `reviewed-mapping-report:DS-mcp-redaction` — Core mapping report DS-mcp-redaction: bridge errors still need a focused LLM-boundary redaction matrix for paths, credentials, headers and provider bodies.
- Exact acceptance contract:
  - Introduce a boundary sanitizer with typed safe error codes/details and private structured logging.
  - Adversarial tests inject a home path, API key, bearer token, signed URL query, provider body, and nested source error and assert none appear in MCP content while remediation remains actionable.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/mcp_error_redaction.rs#llm_errors_redact_paths_credentials_headers_provider_bodies` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test mcp_error_redaction llm_errors_redact_paths_credentials_headers_provider_bodies -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `src-tauri/src/mcp.rs#TauriMediaBridge`, `crates/opentake-agent/src/mcp/convert.rs#to_call_tool_result`, `docs/modules/opentake-agent/SPEC.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test mcp_error_redaction llm_errors_redact_paths_credentials_headers_provider_bodies -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Completion evidence: [`data-safety-mcp-redaction-real-device-2026-07-31.md`](../runtime-artifacts/automated/data-safety-mcp-redaction-real-device-2026-07-31.md). The exact owning matrix already passed at the initial baseline; the packaged MCP server independently proved adversarial path, credential, authorization, signed-query, and provider-style strings never reached response content.

### Task 5: DS-generation-seed (implementation-slice-4f2d8bcaff47a37f)

**Covered records:**
- `requirement-b9010e6717b5d5ea` (requirement)
- `requirement-1f35cc4131f8f0b7` (requirement)

**Files:**
- Modify: `crates/opentake-project/src/bundle.rs#Project`
- Modify: `crates/opentake-core/src/session.rs#EditorSession`
- Modify: `docs/modules/opentake-core/SPEC.md`
- Modify: `docs/specs/core/5-assembly.md`
- Test (existing-owned): `crates/opentake-project/tests/roundtrip.rs#malformed_generation_log_is_ignored`
- Test (reviewed-planned): `crates/opentake-core/tests/project_open.rs#missing_generation_log_seeds_manifest_provenance_once`

**Candidate-bound contracts:**

#### requirement-b9010e6717b5d5ea

- Candidate/source: `doc-d0e72203255f0a41` at `docs/modules/opentake-core/SPEC.md:423` (requirement)
- Expected behavior: When generation-log.json is absent, seed the generation log from manifest entries carrying generation provenance, matching the upstream project-open migration.
- Resolution: `reviewed-mapping-report:DS-generation-seed` — Core mapping report DS-generation-seed: project open currently defaults an absent generation log instead of seeding manifest provenance once.
- Exact acceptance contract:
  - Define a deterministic seed_generation_log_from_assets conversion with stable ids, model, cost, and created-at semantics.
  - Call it only when no valid generation log exists, never duplicate entries when a valid log is present, and persist the seeded log on the next save.
  - Tests cover empty manifest, mixed generated/imported assets, duplicate provenance, malformed optional log, reopen, and byte-stable save.

#### requirement-1f35cc4131f8f0b7

- Candidate/source: `doc-10feadcab19f56dd` at `docs/specs/core/5-assembly.md:54` (requirement)
- Expected behavior: Load the optional generation log and seed legacy generated assets when it is absent.
- Resolution: `reviewed-mapping-report:DS-generation-seed` — Core mapping report DS-generation-seed: project open currently defaults an absent generation log instead of seeding manifest provenance once.
- Exact acceptance contract:
  - Implementation: After lenient generation-log decode, scan manifest generation metadata to reconstruct deterministic legacy log entries without duplicates; persist on the next safe save; test absent/corrupt/existing/partial logs and idempotent reopen.
  - Add deterministic fixtures for every named current, legacy, missing, malformed, and fail-closed branch; the focused round-trip and compatibility suites must pass.
  - Exercise open, edit, save, and reopen on representative bundles and attach the exact implementation symbols plus test or runtime evidence before reclassification.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-project/tests/roundtrip.rs#malformed_generation_log_is_ignored` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/tests/project_open.rs#missing_generation_log_seeds_manifest_provenance_once` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-project --test roundtrip malformed_generation_log_is_ignored -- --exact`
  - Run: `cargo test -p opentake-core --test project_open missing_generation_log_seeds_manifest_provenance_once -- --exact`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/session.rs#EditorSession`, `docs/modules/opentake-core/SPEC.md`, `docs/specs/core/5-assembly.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-project --test roundtrip malformed_generation_log_is_ignored -- --exact`
  - Run: `cargo test -p opentake-core --test project_open missing_generation_log_seeds_manifest_provenance_once -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 6: DS-cache-identity-complete (implementation-slice-5af4f1ababc7b495)

**Covered records:**
- `requirement-52da354b46176a99` (requirement)

**Files:**
- Modify: `crates/opentake-media/src/cache_key.rs#file_identity_key`
- Modify: `crates/opentake-media/src/cache_key.rs#identity_hex`
- Modify: `docs/modules/opentake-media/SPEC.md`
- Test (existing-owned): `crates/opentake-media/src/cache_key.rs#identity_hex_is_stable_and_lowercase`
- Test (existing-owned): `crates/opentake-media/src/cache_key.rs#identity_hex_matches_swift_for_whole_second_mtime`
- Test (existing-owned): `crates/opentake-media/src/cache_key.rs#file_identity_key_missing_file_is_none`

**Candidate-bound contracts:**

#### requirement-52da354b46176a99

- Candidate/source: `doc-1cb2f0539425a14e` at `docs/modules/opentake-media/SPEC.md:163` (requirement)
- Expected behavior: At docs/modules/opentake-media/SPEC.md:163 under “1.4 通用缓存键(三处共用)” (gap-marker), the source “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。” requires this exact behavior: Derive a stable lowercase 32-hex file identity from path, Swift-style mtime, and size, returning None for missing metadata.
- Resolution: `reviewed-mapping-report:DS-cache-identity-complete` — Core mapping report evidence closure: cache identity has direct tracked implementation and focused stability/missing-file tests, pending exact ledger closure.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-media/SPEC.md:163; signal=gap-marker; heading=1.4 通用缓存键(三处共用); candidate=> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。
  - Expected behavior: Derive a stable lowercase 32-hex file identity from path, Swift-style mtime, and size, returning None for missing metadata. This closes only the promise expressed by “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。” in “1.4 通用缓存键(三处共用)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。” with the scenario below and register test:crates/opentake-project/tests/completion_1cb2f0539425a14e.rs#completion_1cb2f0539425a14e_derive_a_stable_lowercase_32_hex_file_identity_f
  - Initial state/input/event: start from the smallest valid fixture for “Derive a stable lowercase 32-hex file identity from path, Swift-style mtime, and size, returning None for missing metadata.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。”.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Derive a stable lowercase 32-hex file identity from path, Swift-style mtime, and size, returning None for missing metadata.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_1cb2f0539425a14e.rs#completion_1cb2f0539425a14e_derive_a_stable_lowercase_32_hex_file_identity_f.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-media/src/cache_key.rs#identity_hex_is_stable_and_lowercase` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/cache_key.rs#identity_hex_matches_swift_for_whole_second_mtime` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/cache_key.rs#file_identity_key_missing_file_is_none` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-media identity_hex_is_stable_and_lowercase`
  - Run: `cargo test -p opentake-media identity_hex_matches_swift_for_whole_second_mtime`
  - Run: `cargo test -p opentake-media file_identity_key_missing_file_is_none`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-media/src/cache_key.rs#file_identity_key`, `crates/opentake-media/src/cache_key.rs#identity_hex`, `docs/modules/opentake-media/SPEC.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-media identity_hex_is_stable_and_lowercase`
  - Run: `cargo test -p opentake-media identity_hex_matches_swift_for_whole_second_mtime`
  - Run: `cargo test -p opentake-media file_identity_key_missing_file_is_none`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 7: DS-shared-core-command-complete (implementation-slice-63ec0e639957e775)

**Covered records:**
- `requirement-44e32f4f5265f1dc` (requirement)
- `requirement-adb4fb8f4521a41f` (requirement)
- `requirement-978fe275600796df` (requirement)
- `requirement-a7fcfeeabb9eca2b` (requirement)
- `requirement-2d95c75bfdf43afd` (requirement)
- `requirement-90e95608564f8bc4` (requirement)
- `requirement-479246e3a4dd5891` (requirement)
- `requirement-1598d28c23e8cd9b` (requirement)
- `requirement-26e566086bff8227` (requirement)
- `requirement-a2923e1fa84e4a81` (requirement)

**Files:**
- Modify: `crates/opentake-ops/src/editor_state.rs#EditorState`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `crates/opentake-core/src/core.rs#AppCore`
- Modify: `crates/opentake-core/src/events.rs#EventBus`
- Modify: `docs/specs/core/1-editor-state.md`
- Modify: `docs/specs/core/2-command-routing.md`
- Test (existing-owned): `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions`
- Test (existing-owned): `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once`
- Test (existing-owned): `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump`
- Test (existing-owned): `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits`

**Candidate-bound contracts:**

#### requirement-44e32f4f5265f1dc

- Candidate/source: `doc-7b33a412558e55a9` at `docs/specs/core/1-editor-state.md:1` (requirement)
- Expected behavior: At docs/specs/core/1-editor-state.md:1 under “1. EditorState 结构” (heading), the source “## 1. EditorState 结构” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Resolution: `reviewed-mapping-report:DS-shared-core-command-complete` — Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/1-editor-state.md:1; signal=heading; heading=1. EditorState 结构; candidate=## 1. EditorState 结构
  - Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “1. EditorState 结构” in “1. EditorState 结构”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1. EditorState 结构” with the scenario below and register test:tools/completion-tests/doc-7b33a412558e55a9.test.mjs#completion_7b33a412558e55a9_editor_state_and_edits_are_owned_by_the_shared_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “1. EditorState 结构”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-7b33a412558e55a9.test.mjs#completion_7b33a412558e55a9_editor_state_and_edits_are_owned_by_the_shared_r.

#### requirement-adb4fb8f4521a41f

- Candidate/source: `doc-715cc47e6276015e` at `docs/specs/core/1-editor-state.md:3` (requirement)
- Expected behavior: At docs/specs/core/1-editor-state.md:3 under “1.1 职责与对应上游” (heading), the source “### 1.1 职责与对应上游” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Resolution: `reviewed-mapping-report:DS-shared-core-command-complete` — Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/1-editor-state.md:3; signal=heading; heading=1.1 职责与对应上游; candidate=### 1.1 职责与对应上游
  - Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “1.1 职责与对应上游” in “1.1 职责与对应上游”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.1 职责与对应上游” with the scenario below and register test:tools/completion-tests/doc-715cc47e6276015e.test.mjs#completion_715cc47e6276015e_editor_state_and_edits_are_owned_by_the_shared_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “1.1 职责与对应上游”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-715cc47e6276015e.test.mjs#completion_715cc47e6276015e_editor_state_and_edits_are_owned_by_the_shared_r.

#### requirement-978fe275600796df

- Candidate/source: `doc-8faa0d04f0c889c1` at `docs/specs/core/1-editor-state.md:9` (requirement)
- Expected behavior: At docs/specs/core/1-editor-state.md:9 under “1.2 字段(草案)” (heading), the source “### 1.2 字段(草案)” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Resolution: `reviewed-mapping-report:DS-shared-core-command-complete` — Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/1-editor-state.md:9; signal=heading; heading=1.2 字段(草案); candidate=### 1.2 字段(草案)
  - Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “1.2 字段(草案)” in “1.2 字段(草案)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.2 字段(草案)” with the scenario below and register test:tools/completion-tests/doc-8faa0d04f0c889c1.test.mjs#completion_8faa0d04f0c889c1_editor_state_and_edits_are_owned_by_the_shared_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “1.2 字段(草案)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-8faa0d04f0c889c1.test.mjs#completion_8faa0d04f0c889c1_editor_state_and_edits_are_owned_by_the_shared_r.

#### requirement-a7fcfeeabb9eca2b

- Candidate/source: `doc-cf44660f6e5bd373` at `docs/specs/core/1-editor-state.md:49` (requirement)
- Expected behavior: At docs/specs/core/1-editor-state.md:49 under “1.3 访问封装:`EditorCore`” (heading), the source “### 1.3 访问封装:`EditorCore`” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Resolution: `reviewed-mapping-report:DS-shared-core-command-complete` — Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/1-editor-state.md:49; signal=heading; heading=1.3 访问封装:`EditorCore`; candidate=### 1.3 访问封装:`EditorCore`
  - Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “1.3 访问封装:`EditorCore`” in “1.3 访问封装:`EditorCore`”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “1.3 访问封装:`EditorCore`” with the scenario below and register test:tools/completion-tests/doc-cf44660f6e5bd373.test.mjs#completion_cf44660f6e5bd373_editor_state_and_edits_are_owned_by_the_shared_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “1.3 访问封装:`EditorCore`”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-cf44660f6e5bd373.test.mjs#completion_cf44660f6e5bd373_editor_state_and_edits_are_owned_by_the_shared_r.

#### requirement-2d95c75bfdf43afd

- Candidate/source: `doc-38f0c6dea35ccc52` at `docs/specs/core/2-command-routing.md:1` (requirement)
- Expected behavior: At docs/specs/core/2-command-routing.md:1 under “2. 命令路由 = 上游 ToolExecutor 单一能力层” (heading), the source “## 2. 命令路由 = 上游 ToolExecutor 单一能力层” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Resolution: `reviewed-mapping-report:DS-shared-core-command-complete` — Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/2-command-routing.md:1; signal=heading; heading=2. 命令路由 = 上游 ToolExecutor 单一能力层; candidate=## 2. 命令路由 = 上游 ToolExecutor 单一能力层
  - Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2. 命令路由 = 上游 ToolExecutor 单一能力层” in “2. 命令路由 = 上游 ToolExecutor 单一能力层”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2. 命令路由 = 上游 ToolExecutor 单一能力层” with the scenario below and register test:crates/opentake-agent/tests/completion_38f0c6dea35ccc52.rs#completion_38f0c6dea35ccc52_editor_state_and_edits_are_owned_by_the_shared_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “2. 命令路由 = 上游 ToolExecutor 单一能力层”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_38f0c6dea35ccc52.rs#completion_38f0c6dea35ccc52_editor_state_and_edits_are_owned_by_the_shared_r.

#### requirement-90e95608564f8bc4

- Candidate/source: `doc-b2159a4af40efa5d` at `docs/specs/core/2-command-routing.md:3` (requirement)
- Expected behavior: At docs/specs/core/2-command-routing.md:3 under “2.1 核心原则:一处定义,三客户端共享” (heading), the source “### 2.1 核心原则:一处定义,三客户端共享” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Resolution: `reviewed-mapping-report:DS-shared-core-command-complete` — Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/2-command-routing.md:3; signal=heading; heading=2.1 核心原则:一处定义,三客户端共享; candidate=### 2.1 核心原则:一处定义,三客户端共享
  - Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2.1 核心原则:一处定义,三客户端共享” in “2.1 核心原则:一处定义,三客户端共享”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.1 核心原则:一处定义,三客户端共享” with the scenario below and register test:tools/completion-tests/doc-b2159a4af40efa5d.test.mjs#completion_b2159a4af40efa5d_editor_state_and_edits_are_owned_by_the_shared_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.1 核心原则:一处定义,三客户端共享”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-b2159a4af40efa5d.test.mjs#completion_b2159a4af40efa5d_editor_state_and_edits_are_owned_by_the_shared_r.

#### requirement-479246e3a4dd5891

- Candidate/source: `doc-149099f8c0e60570` at `docs/specs/core/2-command-routing.md:15` (requirement)
- Expected behavior: At docs/specs/core/2-command-routing.md:15 under “2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)” (heading), the source “### 2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Resolution: `reviewed-mapping-report:DS-shared-core-command-complete` — Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/2-command-routing.md:15; signal=heading; heading=2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5); candidate=### 2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)
  - Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)” in “2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)” with the scenario below and register test:tools/completion-tests/doc-149099f8c0e60570.test.mjs#completion_149099f8c0e60570_editor_state_and_edits_are_owned_by_the_shared_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-149099f8c0e60570.test.mjs#completion_149099f8c0e60570_editor_state_and_edits_are_owned_by_the_shared_r.

#### requirement-1598d28c23e8cd9b

- Candidate/source: `doc-dc4980e1cf95b7d2` at `docs/specs/core/2-command-routing.md:52` (requirement)
- Expected behavior: At docs/specs/core/2-command-routing.md:52 under “2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)” (heading), the source “### 2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Resolution: `reviewed-mapping-report:DS-shared-core-command-complete` — Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/2-command-routing.md:52; signal=heading; heading=2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`); candidate=### 2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)
  - Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)” in “2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)” with the scenario below and register test:web/src/__tests__/completion/doc-dc4980e1cf95b7d2.test.ts#completion_dc4980e1cf95b7d2_editor_state_and_edits_are_owned_by_the_shared_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-dc4980e1cf95b7d2.test.ts#completion_dc4980e1cf95b7d2_editor_state_and_edits_are_owned_by_the_shared_r.

#### requirement-26e566086bff8227

- Candidate/source: `doc-97047ee2cef7bff4` at `docs/specs/core/2-command-routing.md:102` (requirement)
- Expected behavior: At docs/specs/core/2-command-routing.md:102 under “2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)” (heading), the source “### 2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Resolution: `reviewed-mapping-report:DS-shared-core-command-complete` — Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/2-command-routing.md:102; signal=heading; heading=2.4 Undo / Redo —— 双层撤销模型(精确复刻上游); candidate=### 2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)
  - Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)” in “2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)” with the scenario below and register test:tools/completion-tests/doc-97047ee2cef7bff4.test.mjs#completion_97047ee2cef7bff4_editor_state_and_edits_are_owned_by_the_shared_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-97047ee2cef7bff4.test.mjs#completion_97047ee2cef7bff4_editor_state_and_edits_are_owned_by_the_shared_r.

#### requirement-a2923e1fa84e4a81

- Candidate/source: `doc-c95515ae8f048e8a` at `docs/specs/core/2-command-routing.md:140` (requirement)
- Expected behavior: At docs/specs/core/2-command-routing.md:140 under “2.5 三客户端如何共享(装配视角)” (heading), the source “### 2.5 三客户端如何共享(装配视角)” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Resolution: `reviewed-mapping-report:DS-shared-core-command-complete` — Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Exact acceptance contract:
  - Source binding: docs/specs/core/2-command-routing.md:140; signal=heading; heading=2.5 三客户端如何共享(装配视角); candidate=### 2.5 三客户端如何共享(装配视角)
  - Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2.5 三客户端如何共享(装配视角)” in “2.5 三客户端如何共享(装配视角)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.5 三客户端如何共享(装配视角)” with the scenario below and register test:tools/completion-tests/doc-c95515ae8f048e8a.test.mjs#completion_c95515ae8f048e8a_editor_state_and_edits_are_owned_by_the_shared_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.5 三客户端如何共享(装配视角)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-c95515ae8f048e8a.test.mjs#completion_c95515ae8f048e8a_editor_state_and_edits_are_owned_by_the_shared_r.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-ops commit_undo_redo_cycle_restores_and_versions`
  - Run: `cargo test -p opentake-core apply_bumps_version_and_emits_once`
  - Run: `cargo test -p opentake-core unchanged_command_does_not_emit_or_bump`
  - Run: `cargo test -p opentake-core undo_redo_through_core_bumps_version_and_emits`

  Expected: FAIL because one or more of the 10 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/1-editor-state.md`, `docs/specs/core/2-command-routing.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-ops commit_undo_redo_cycle_restores_and_versions`
  - Run: `cargo test -p opentake-core apply_bumps_version_and_emits_once`
  - Run: `cargo test -p opentake-core unchanged_command_does_not_emit_or_bump`
  - Run: `cargo test -p opentake-core undo_redo_through_core_bumps_version_and_emits`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 8: DS-project-open-composite-headings (implementation-slice-0d21c3ea6ba4327b)

**Covered records:**
- `requirement-6748d221ef0d9a4c` (requirement)
- `requirement-9335bc98b18f8d8d` (requirement)
- `requirement-706e744a85684655` (requirement)
- `requirement-b38818cf815e0f1e` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/core_handle.rs#AppCoreHandle::media_path`
- Modify: `crates/opentake-core/src/core.rs#AppCore::apply_at_revision`
- Modify: `crates/opentake-core/tests/project_open.rs#project_open_composite_acceptance`
- Modify: `src-tauri/src/captions.rs#generate_captions`
- Modify: `src-tauri/src/commands.rs#project_open_with_playback_and_prewarm`
- Modify: `src-tauri/src/export.rs#export_video`
- Modify: `src-tauri/src/mcp.rs#TauriMediaBridge`
- Modify: `src-tauri/src/media.rs#MediaListDto::from_core_with_import_results`
- Modify: `src-tauri/src/render.rs#composite_rgba`
- Modify: `src-tauri/src/playback/commands.rs#PlaybackState`
- Modify: `src-tauri/src/search.rs#resolve_assets`
- Modify: `src-tauri/src/transcribe.rs#resolve_asset_from_snapshot`
- Modify: `docs/audit/2026-07-14/implementation-plans/data-safety-design.md`
- Modify: `docs/audit/2026-07-14/implementation-plans/data-safety-implementation.md`
- Modify: `docs/specs/core/5-assembly.md`
- Test (reviewed-planned): `crates/opentake-project/tests/upstream_compat.rs#exhaustive_legacy_default_matrix`
- Test (reviewed-planned): `crates/opentake-core/tests/project_open.rs#missing_generation_log_seeds_manifest_provenance_once`
- Test (reviewed-planned): `crates/opentake-core/tests/project_open.rs#project_open_composite_acceptance`
- Test (reviewed-added): `crates/opentake-core/src/core.rs#deferred_apply_rejects_version_and_project_drift_without_mutation`
- Test (reviewed-added): `crates/opentake-agent/src/mcp/core_handle.rs#app_core_media_path_stress_never_mixes_project_snapshots`
- Test (reviewed-added): `src-tauri/src/captions.rs#caption_commit_rejects_stale_project_revision`
- Test (reviewed-added): `src-tauri/src/commands.rs#project_open_mapped_boundaries_composite_acceptance`
- Test (reviewed-added): `src-tauri/src/playback/commands.rs#prewarm_rejection_restores_active_playback_without_project_publish`
- Test (reviewed-added): `src-tauri/src/mcp.rs#transcript_batch_resolution_uses_one_snapshot_and_authoritative_types`

**Candidate-bound contracts:**

#### requirement-6748d221ef0d9a4c

- Candidate/source: `doc-5a5785b4e46c5875` at `docs/specs/core/5-assembly.md:1` (requirement)
- Expected behavior: Project open assembles validated project/media/generation state without silent loss or placeholder dependencies.
- Resolution: `reviewed-mapping-report:DS-project-open-composite-headings` — Core mapping report: four project-open umbrellas are composite acceptance over the legacy-default and generation-seed child slices, not independent top-one features.
- Exact acceptance contract:
  - Validate every persisted component and synchronously fallible playback/prewarm admission before publishing editor state; exercise render, playback projection, cache, and Agent consumers against the committed atomic snapshot.
  - A failed dependency or malformed file must return a typed error/recovery result without silently replacing user data or partially publishing a session.
  - Test valid, legacy, missing optional, malformed media.json, malformed generation log, dependency failure, save, replacement/close, and reopen with byte/semantic equality.

#### requirement-9335bc98b18f8d8d

- Candidate/source: `doc-7be80db04be852c6` at `docs/specs/core/5-assembly.md:21` (requirement)
- Expected behavior: Project open assembles validated project/media/generation state without silent loss or placeholder dependencies.
- Resolution: `reviewed-mapping-report:DS-project-open-composite-headings` — Core mapping report: four project-open umbrellas are composite acceptance over the legacy-default and generation-seed child slices, not independent top-one features.
- Exact acceptance contract:
  - Remove stale claims that project/render/playback/Agent are production CoreDeps fields; map the real Project/EditorSession/AppCore, src-tauri coordinator/runtime-snapshot, and Agent CoreHandle interfaces.
  - Use deterministic owning-boundary fixtures and ensure core never imports Tauri/UI/provider concrete types.
  - Test prepare failure, overlapping transition, prewarm rejection/cancellation with incumbent playback, and successful epoch activation without partial project publication or leaked transition ownership.

#### requirement-706e744a85684655

- Candidate/source: `doc-8fb894b0ea5ce82e` at `docs/specs/core/5-assembly.md:36` (requirement)
- Expected behavior: Project open assembles validated project/media/generation state without silent loss or placeholder dependencies.
- Resolution: `reviewed-mapping-report:DS-project-open-composite-headings` — Core mapping report: four project-open umbrellas are composite acceptance over the legacy-default and generation-seed child slices, not independent top-one features.
- Exact acceptance contract:
  - Map each project/media/render/Agent assembly point to its current implementation symbol and remove stale upstream-only or placeholder references.
  - Exercise combined media/path reads through one AppCore runtime snapshot and Agent reads through the production CoreHandle rather than direct frontend or private Agent state mutation.
  - Add a desktop contract test that opens one fixture and invokes media lookup, render-plan construction, playback media projection, Agent read/path resolution, save, and reopen through the mapped interfaces.

#### requirement-b38818cf815e0f1e

- Candidate/source: `doc-e286de761da93671` at `docs/specs/core/5-assembly.md:48` (requirement)
- Expected behavior: Project open assembles validated project/media/generation state without silent loss or placeholder dependencies.
- Resolution: `reviewed-mapping-report:DS-project-open-composite-headings` — Core mapping report: four project-open umbrellas are composite acceptance over the legacy-default and generation-seed child slices, not independent top-one features.
- Exact acceptance contract:
  - Implement open ordering as validate bundle → load timeline/media/generation log → build candidate session → admit playback/prewarm transition → publish snapshot/events → activate epoch → allow optional background work.
  - On every pre-commit failure, drop prepared capabilities, restore acquired transitions, publish no usable session, and preserve all project files unchanged.
  - Fault-inject the owned failure boundaries and assert cleanup/event order, then open/save/reopen valid and migrated fixtures with identical IDs, frames, media, and generation history.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-project/tests/upstream_compat.rs#exhaustive_legacy_default_matrix` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/tests/project_open.rs#missing_generation_log_seeds_manifest_provenance_once` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/tests/project_open.rs#project_open_composite_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/src/core.rs#deferred_apply_rejects_version_and_project_drift_without_mutation` (reviewed-added) — A long-running workflow cannot commit against a replaced or edited project.
  - `crates/opentake-agent/src/mcp/core_handle.rs#app_core_media_path_stress_never_mixes_project_snapshots` (reviewed-added) — Stress concurrent project switching against production CoreHandle path resolution.
  - `src-tauri/src/captions.rs#caption_commit_rejects_stale_project_revision` (reviewed-added) — Caption generation cannot write a stale result into a new project.
  - `src-tauri/src/commands.rs#project_open_mapped_boundaries_composite_acceptance` (reviewed-added) — One committed fixture reaches UI media, render plan, playback projection, Agent, save, and reopen boundaries.
  - `src-tauri/src/playback/commands.rs#prewarm_rejection_restores_active_playback_without_project_publish` (reviewed-added) — A rejected dependency restores incumbent playback without publishing the prepared project.
  - `src-tauri/src/mcp.rs#transcript_batch_resolution_uses_one_snapshot_and_authoritative_types` (reviewed-added) — The production batch resolver uses one snapshot for every source and ignores stale caller type hints.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-project --test upstream_compat exhaustive_legacy_default_matrix -- --exact`
  - Run: `cargo test -p opentake-core --test project_open missing_generation_log_seeds_manifest_provenance_once -- --exact`
  - Run: `cargo test -p opentake-core --test project_open project_open_composite_acceptance -- --exact`
  - Run: `cargo test -p opentake-core core::tests::deferred_apply_rejects_version_and_project_drift_without_mutation -- --exact`
  - Run: `cargo test -p opentake-agent mcp::core_handle::tests::app_core_media_path_stress_never_mixes_project_snapshots -- --exact`
  - Run: `cargo test -p opentake-tauri captions::tests::caption_commit_rejects_stale_project_revision -- --exact`
  - Run: `cargo test -p opentake-tauri commands::project_prewarm_lifecycle_tests::project_open_mapped_boundaries_composite_acceptance -- --exact`
  - Run: `cargo test -p opentake-tauri playback::commands::tests::prewarm_rejection_restores_active_playback_without_project_publish -- --exact`
  - Run: `cargo test -p opentake-tauri mcp::tests::transcript_batch_resolution_uses_one_snapshot_and_authoritative_types -- --exact`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only the files listed for Task 8 as required to satisfy every listed acceptance criterion, including atomic combined reads, visible success, and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-project --test upstream_compat exhaustive_legacy_default_matrix -- --exact`
  - Run: `cargo test -p opentake-core --test project_open missing_generation_log_seeds_manifest_provenance_once -- --exact`
  - Run: `cargo test -p opentake-core --test project_open project_open_composite_acceptance -- --exact`
  - Run: `cargo test -p opentake-core core::tests::deferred_apply_rejects_version_and_project_drift_without_mutation -- --exact`
  - Run: `cargo test -p opentake-agent mcp::core_handle::tests::app_core_media_path_stress_never_mixes_project_snapshots -- --exact`
  - Run: `cargo test -p opentake-tauri captions::tests::caption_commit_rejects_stale_project_revision -- --exact`
  - Run: `cargo test -p opentake-tauri commands::project_prewarm_lifecycle_tests::project_open_mapped_boundaries_composite_acceptance -- --exact`
  - Run: `cargo test -p opentake-tauri playback::commands::tests::prewarm_rejection_restores_active_playback_without_project_publish -- --exact`
  - Run: `cargo test -p opentake-tauri mcp::tests::transcript_batch_resolution_uses_one_snapshot_and_authoritative_types -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 9: DS-manifest-corruption-conflict (implementation-slice-38c616831a6a7bd5)

**Covered records:**
- `requirement-7908bd026b5a91f6` (requirement)

**Files:**
- Modify: `crates/opentake-project/src/bundle.rs#Project`
- Modify: `crates/opentake-project/tests/common/mod.rs#tree_receipt`
- Modify: `crates/opentake-project/tests/roundtrip.rs#malformed_manifest_is_an_error`
- Modify: `crates/opentake-project/tests/schema_compat.rs#malformed_manifest_contract_matches_authoritative_source`
- Modify: `docs/audit/2026-07-14/implementation-plans/data-safety-design.md`
- Modify: `docs/audit/2026-07-14/implementation-plans/data-safety-implementation.md`
- Modify: `docs/specs/core/5-assembly.md`
- Test (existing-owned): `crates/opentake-project/tests/roundtrip.rs#malformed_manifest_is_an_error`
- Test (reviewed-planned): `crates/opentake-project/tests/schema_compat.rs#malformed_manifest_contract_matches_authoritative_source`

**Candidate-bound contracts:**

#### requirement-7908bd026b5a91f6

- Candidate/source: `doc-c5eeda123dd2c6ca` at `docs/specs/core/5-assembly.md:58` (requirement)
- Expected behavior: Open legacy projects with defaulted optional manifest/generation data while keeping project.json strict.
- Resolution: `reviewed-mapping-report:DS-manifest-corruption-conflict` — Reconciliation confirmed that the authoritative upstream and current product both require malformed `media.json` to fail closed. The conflict was in this plan text, not in product behavior.
- Exact acceptance contract:
  - Keep `project.json` strict; default a missing `media.json` to an empty current manifest; fail closed with typed `Json(media.json)` when a present manifest is syntactically or structurally malformed.
  - Keep malformed `generation-log.json` as the sole lenient read recovery: open with a compatibility blocker, reject same-path save and Save As before any write, and preserve the damaged bytes.
  - Add deterministic fixtures for every named current, legacy, missing, malformed, and fail-closed branch; the focused round-trip and compatibility suites must pass.
  - Exercise open, edit, save, and reopen on a complete current version-2 bundle. Prove every rejected open/save leaves the full nofollow bundle tree unchanged, and prove rejected Save As creates no destination, journal, staging, symlink, or other sibling artifact.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-project/tests/roundtrip.rs#malformed_manifest_is_an_error` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/schema_compat.rs#malformed_manifest_contract_matches_authoritative_source` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-project --test roundtrip malformed_manifest_is_an_error -- --exact`
  - Run: `cargo test -p opentake-project --test schema_compat malformed_manifest_contract_matches_authoritative_source -- --exact`

  Expected: the existing round-trip smoke passes, while the reviewed-planned
  schema contract is absent (`running 0 tests`), so the gate is not satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Add the missing owning test and reconcile the comments/spec/plan in the files
  listed for Task 9. Do not change the already-correct `Project` or
  `MediaManifest` runtime behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-project --test roundtrip malformed_manifest_is_an_error -- --exact`
  - Run: `cargo test -p opentake-project --test schema_compat malformed_manifest_contract_matches_authoritative_source -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 10: DS-cross-cutting-security-headings (implementation-slice-b97a3dae0f6b3e32)

**Covered records:**
- `requirement-37acd430c8e1e82b` (requirement)
- `requirement-43bc7b61f5b484f7` (requirement)
- `requirement-47a9f1e3f7c11b4c` (requirement)
- `requirement-debab57411dd52fb` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`
- Modify: `crates/opentake-project/src/bundle.rs#Project`
- Modify: `crates/opentake-core/src/core.rs#AppCore`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `docs/specs/core/7-security.md`
- Modify: `docs/specs/core/8-implementation.md`
- Test (reviewed-planned): `crates/opentake-agent/src/mcp/dispatch.rs#cross_cutting_mcp_acceptance`
- Test (reviewed-planned): `crates/opentake-project/tests/schema_compat.rs#cross_cutting_project_safety_acceptance`
- Test (reviewed-planned): `crates/opentake-core/src/core.rs#cross_cutting_runtime_acceptance`
- Test (reviewed-planned): `crates/opentake-ops/tests/command_apply.rs#cross_cutting_command_acceptance`

**Candidate-bound contracts:**

#### requirement-37acd430c8e1e82b

- Candidate/source: `doc-e1c7a6f1c3605364` at `docs/specs/core/7-security.md:1` (requirement)
- Expected behavior: Desktop command, asset, sidecar, and MCP boundaries fail closed on every packaged platform.
- Resolution: `reviewed-mapping-report:DS-cross-cutting-security-headings` — Core mapping report: these umbrellas cross MCP, safe-fs, playback, export and Agent boundaries and therefore require composite child-slice acceptance.
- Exact acceptance contract:
  - Close Windows CSP/WebView2/sidecar permission and path validation gaps.
  - Add packaged-build security tests for asset scope, origins, traversal, and sidecar invocation.
  - Document signing/notarization and secret-handling evidence.

#### requirement-43bc7b61f5b484f7

- Candidate/source: `doc-954a8799d333db54` at `docs/specs/core/8-implementation.md:1` (requirement)
- Expected behavior: Core assembly and cross-cutting validation cover safe reopen, sync, playback, export, and Agent clients.
- Resolution: `reviewed-mapping-report:DS-cross-cutting-security-headings` — Core mapping report: these umbrellas cross MCP, safe-fs, playback, export and Agent boundaries and therefore require composite child-slice acceptance.
- Exact acceptance contract:
  - Close assembly data safety, frontend authority, packaged security, and cross-client parity while preserving completed pure-core/Tauri/Agent stages.
  - Every open/edit/undo/save/reopen path must use typed DTOs/commands and fail without silent data replacement.
  - Pass corrupt-fixture, migration, schema parity, concurrent version, playback/export, Agent, and packaged desktop integration suites.

#### requirement-47a9f1e3f7c11b4c

- Candidate/source: `doc-b309f10e20832ec1` at `docs/specs/core/8-implementation.md:18` (requirement)
- Expected behavior: Core assembly and cross-cutting validation cover safe reopen, sync, playback, export, and Agent clients.
- Resolution: `reviewed-mapping-report:DS-cross-cutting-security-headings` — Core mapping report: these umbrellas cross MCP, safe-fs, playback, export and Agent boundaries and therefore require composite child-slice acceptance.
- Exact acceptance contract:
  - Inject production project/media/render/playback dependencies, seed generation-log state, and parse media.json without silent fallback.
  - Open failures must clean up partial sessions and leave bundle bytes unchanged; save must atomically persist timeline/media/generation state.
  - Fault-inject each dependency and run valid/legacy/corrupt save-reopen fixtures asserting IDs, frames, media, and generation history.

#### requirement-debab57411dd52fb

- Candidate/source: `doc-30748aed19c57fcd` at `docs/specs/core/8-implementation.md:37` (requirement)
- Expected behavior: Core assembly and cross-cutting validation cover safe reopen, sync, playback, export, and Agent clients.
- Resolution: `reviewed-mapping-report:DS-cross-cutting-security-headings` — Core mapping report: these umbrellas cross MCP, safe-fs, playback, export and Agent boundaries and therefore require composite child-slice acceptance.
- Exact acceptance contract:
  - Prove Rust/Web DTO schema parity, all-client command equivalence, version ordering, undo/redo, save/reopen, and event sequencing.
  - Run the same edit vectors through desktop UI and Agent dispatch and compare EditResult, timeline JSON, version, and undo outcome exactly.
  - Add concurrency, malformed/corrupt project, playback/export artifact, security-boundary, and packaged macOS/Windows smoke coverage.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/mcp/dispatch.rs#cross_cutting_mcp_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/tests/schema_compat.rs#cross_cutting_project_safety_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/src/core.rs#cross_cutting_runtime_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-ops/tests/command_apply.rs#cross_cutting_command_acceptance` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent cross_cutting_mcp_acceptance`
  - Run: `cargo test -p opentake-project --test schema_compat cross_cutting_project_safety_acceptance -- --exact`
  - Run: `cargo test -p opentake-core cross_cutting_runtime_acceptance`
  - Run: `cargo test -p opentake-ops --test command_apply cross_cutting_command_acceptance -- --exact`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-ops/src/command.rs#EditCommand`, `docs/specs/core/7-security.md`, `docs/specs/core/8-implementation.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent cross_cutting_mcp_acceptance`
  - Run: `cargo test -p opentake-project --test schema_compat cross_cutting_project_safety_acceptance -- --exact`
  - Run: `cargo test -p opentake-core cross_cutting_runtime_acceptance`
  - Run: `cargo test -p opentake-ops --test command_apply cross_cutting_command_acceptance -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 11: DS-unix-consuming-tests (implementation-slice-e5c7ae10bcfd7edf)

**Status:** `REJECTED/BLOCKED` — the portable Unix directory/stage-create identity
contract is unsatisfiable with Linux/macOS public primitives. Production typed-refuses
before namespace mutation; downstream algorithm tests use test-only trusted fixtures.

**Covered records:**
- `requirement-67e50cfe6dd7f49a` (requirement)

**Files:**
- Modify: `Cargo.lock#opentake-project`
- Modify: `crates/opentake-project/Cargo.toml`
- Modify: `crates/opentake-project/src/safe_fs/unix.rs`
- Modify: `crates/opentake-project/src/safe_fs/test_seam.rs#serialize_unix_test`
- Modify: `docs/superpowers/plans/c1b/2026-07-12-c1b-common-unix-normative.md`
- Test (independent-review): `crates/opentake-project/src/safe_fs/tests.rs#read_parent_cannot_escalate_child_directory_access`
- Test (independent-review): `crates/opentake-project/src/safe_fs/tests.rs#read_parent_cannot_escalate_file_access`
- Test (architecture-reconciliation): `crates/opentake-project/src/safe_fs/tests.rs#create_directory_typed_refuses_before_namespace_mutation`
- Test (architecture-reconciliation): `crates/opentake-project/src/safe_fs/tests.rs#create_stage_directory_typed_refuses_before_namespace_mutation`
- Test (reviewed-planned): `crates/opentake-project/src/safe_fs/tests.rs#source_swap_before_quarantine_restores_without_deletion`
- Test (reviewed-planned): `crates/opentake-project/src/safe_fs/tests.rs#restore_collision_fail_leaks_original_and_quarantine`
- Test (reviewed-planned): `crates/opentake-project/src/safe_fs/tests.rs#final_unix_name_window_is_explicit_same_account_boundary`
- Test (reviewed-planned): `crates/opentake-project/src/safe_fs/tests.rs#cleanup_capability_records_identity_before_consuming_delete`
- Test (reviewed-planned): `crates/opentake-project/src/safe_fs/tests.rs#nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories`
- Test (reviewed-planned): `crates/opentake-project/src/safe_fs/tests.rs#destination_collision_preserves_stage_and_every_destination_kind`

**Candidate-bound contracts:**

#### requirement-67e50cfe6dd7f49a

- Candidate/source: `doc-72e29816dc2090ed` at `docs/superpowers/plans/c1b/2026-07-12-c1b-common-unix-normative.md:2187` (requirement)
- Expected behavior: The test-only commit adds exactly these six public consuming mutation/cleanup tests and no others: `source_swap_before_quarantine_restores_without_deletion`, `restore_collision_fail_leaks_original_and_quarantine`, `final_unix_name_window_is_explicit_same_account_boundary`, `cleanup_capability_records_identity_before_consuming_delete`, `nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories`, and `destination_collision_preserves_stage_and_every_destination_kind`. All ten post-create rollback regressions were already committed before their Task 4 implementation and passed at the reviewed Task 4 GREEN SHA; Task 5 neither moves nor re-adds them. The focused RED is the single named recursive-cleanup test below and fails only because Task 4's approved public mutation stub refuses.
- Resolution: `REJECTED/BLOCKED:portable-unix-directory-create-identity` — Linux/macOS expose no portable atomic mkdir-and-return-fd primitive. `mkdirat` followed by `openat` admits a same-account name replacement window, and a randomized temporary name only moves that race. Production directory/stage create therefore returns exact `UnsupportedAtomicPublish(PrimitiveUnavailable)` before mutation. Test-only trusted fixtures exercise rollback/quarantine/publish/cleanup algorithms without claiming production directory-create support. Future options are a single-file fd-backed container or a privileged/private namespace broker.
- Exact acceptance contract:
  - crates/opentake-project/src/safe_fs/unix.rs no longer includes unsupported.rs and implements capability-relative no-follow acquisition, regular-file I/O/create, quarantine, no-replace publish, and recursive cleanup; production directory/stage create must typed-refuse before mutation.
  - The six named consuming mutation/cleanup regressions and directory-specific rollback regressions pass only through `#[cfg(test)]` trusted fixture creation; regular-file rollback and production directory/stage typed refusal pass against production Unix entry points.
  - Symlink, FIFO, source-swap, identity-change, restore-collision, and destination-collision cases fail closed without data loss. The final Unix read-to-name-syscall window is name-linearized and outside the approved threat boundary; it does not provide a no-data-loss guarantee against a same-account namespace actor.
  - Rust workspace tests, warnings-denied clippy, and native macOS receipt gates
    pass on the exact reviewed tree. Linux cross-compilation is additive and
    does not replace the still-required native Linux receipt.

- [ ] **Step 1: Write or extend every reviewed owning test — executed, task remains blocked**

  Controller reconciliation restores the complete normative Unix test group in
  the sole owning runner: eleven authority/access/I/O/probe/refusal names, ten
  post-create rollback names, and the six consuming names below. On macOS, 25
  tests are collected; the two Linux-only probe names remain source-checked and
  cross-compiled rather than misreported as native executions.

  The directory rollback and six consuming names use only
  `create_*_trusted_fixture`, which is `#[cfg(test)]`; they prove downstream
  algorithms, not production directory/stage creation.

  - `crates/opentake-project/src/safe_fs/tests.rs#source_swap_before_quarantine_restores_without_deletion` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#restore_collision_fail_leaks_original_and_quarantine` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#final_unix_name_window_is_explicit_same_account_boundary` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#cleanup_capability_records_identity_before_consuming_delete` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#destination_collision_preserves_stage_and_every_destination_kind` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED — executed, task remains blocked**

  - Run: `cargo test -p opentake-project source_swap_before_quarantine_restores_without_deletion`
  - Run: `cargo test -p opentake-project restore_collision_fail_leaks_original_and_quarantine`
  - Run: `cargo test -p opentake-project final_unix_name_window_is_explicit_same_account_boundary`
  - Run: `cargo test -p opentake-project cleanup_capability_records_identity_before_consuming_delete`
  - Run: `cargo test -p opentake-project nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories`
  - Run: `cargo test -p opentake-project destination_collision_preserves_stage_and_every_destination_kind`

  The original interactive turn observed one-test failures but did not create
  the required separate test-only commit or persistent receipt, so those
  observations do not satisfy the normative RED protocol. A later hash-bound
  isolated replay reproduced three original `UnsupportedTarget` failures, both
  access-escalation failures, and both production create-refusal failures with
  exact `exit=101`, `running 1 test`, and `0 passed; 1 failed` markers. See
  `.superpowers/sdd/task-11-red-replay.md`. Post-hoc replay does not cure the
  missing original protocol evidence, and the task remains blocked.

- [ ] **Step 3: Implement the safe partial vertical slice — production contract rejected**

  Replace the Unix unsupported adapter with retained-fd, capability-relative,
  no-follow acquisition, regular-file create/I/O, quarantine, publish, and cleanup.
  Production directory/stage create typed-refuses before mutation. The former
  mkdirat/openat body is available only as explicitly named `#[cfg(test)]` trusted
  fixtures. Do not modify the common capability/ops facade or Windows adapter.

- [ ] **Step 4: Run all focused tests and verify GREEN — partial implementation only**

  - Run: `cargo test -p opentake-project source_swap_before_quarantine_restores_without_deletion`
  - Run: `cargo test -p opentake-project restore_collision_fail_leaks_original_and_quarantine`
  - Run: `cargo test -p opentake-project final_unix_name_window_is_explicit_same_account_boundary`
  - Run: `cargo test -p opentake-project cleanup_capability_records_identity_before_consuming_delete`
  - Run: `cargo test -p opentake-project nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories`
  - Run: `cargo test -p opentake-project destination_collision_preserves_stage_and_every_destination_kind`

  Observed: all candidate-bound downstream assertions and exact refusal/access
  regressions pass, but they cannot satisfy the rejected production directory-create
  requirement because consuming tests are seeded through trusted fixtures.

- [ ] **Step 5: Run the subsystem regression gate — validation does not unblock contract**

  Run the one-shot Unix seam tests serialized, then run formatting, warnings-denied
  clippy, native macOS project tests, Linux/Windows cross-checks, and the workspace
  gate. Native Linux behavior still requires a Linux receipt.

  Expected: PASS with no new warnings or unrelated changes. A passing gate is
  verification of the safe partial implementation, not completion of Task 11.

### Task 12: DS-windows-safe-fs (implementation-slice-c7f1cd8463f97ad5)

**Covered records:**
- `requirement-db25fa5cad389d60` (requirement)
- `requirement-03a0d53b4d783fb7` (requirement)
- `requirement-e448f9531bbdb638` (requirement)

**Files:**
- Modify: `crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority`
- Modify: `crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory`
- Modify: `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md`
- Test (reviewed-planned): `crates/opentake-project/src/safe_fs/tests.rs#windows_contract`
- Test (reviewed-planned): `crates/opentake-project/src/safe_fs/tests.rs#synchronous_nt_pending_is_invariant_error`

**Candidate-bound contracts:**

#### requirement-db25fa5cad389d60

- Candidate/source: `doc-d32f0d9172a7d88e` at `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md:96` (requirement)
- Expected behavior: 下面是 `windows.rs` 的首段；sections 3–11 的后续 code blocks 按文档顺序紧接其后，所有同名函数均只有一个定义。整份附录没有 stub、`_real` 或 `_impl` 旁路：
- Resolution: `reviewed-mapping-report:DS-windows-safe-fs` — Core mapping report DS-windows-safe-fs: windows.rs is still the unsupported backend and needs the complete NT contract plus cross-compile gate.
- Exact acceptance contract:
  - crates/opentake-project/src/safe_fs/windows.rs no longer includes unsupported.rs and provides one compile-complete capability-relative implementation without todo, unimplemented, panic, or bypass variants.
  - Synchronous NT I/O treats STATUS_PENDING as a structured invariant failure without reading unfinished output or silently waiting.
  - Acquisition, DACL checks, I/O, quarantine, no-replace publish, cleanup, reparse-point, source-swap, and cancellation regressions pass on native Windows.
  - The x86_64-pc-windows-msvc build, warnings-denied clippy, repository Windows jobs, and independent review all pass for the exact tree.

#### requirement-03a0d53b4d783fb7

- Candidate/source: `doc-5347223598d9a9f2` at `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md:511` (requirement)
- Expected behavior: 1. `returned_status == STATUS_PENDING` 在同步 handle 上属于 invariant failure；记录 raw status 并返回 `Os`，不得读取未完成 output，不另建 event，也不偷偷异步等待。
- Resolution: `reviewed-mapping-report:DS-windows-safe-fs` — Core mapping report DS-windows-safe-fs: windows.rs is still the unsupported backend and needs the complete NT contract plus cross-compile gate.
- Exact acceptance contract:
  - crates/opentake-project/src/safe_fs/windows.rs no longer includes unsupported.rs and provides one compile-complete capability-relative implementation without todo, unimplemented, panic, or bypass variants.
  - Synchronous NT I/O treats STATUS_PENDING as a structured invariant failure without reading unfinished output or silently waiting.
  - Acquisition, DACL checks, I/O, quarantine, no-replace publish, cleanup, reparse-point, source-swap, and cancellation regressions pass on native Windows.
  - The x86_64-pc-windows-msvc build, warnings-denied clippy, repository Windows jobs, and independent review all pass for the exact tree.

#### requirement-e448f9531bbdb638

- Candidate/source: `doc-842bd9e2b189fe18` at `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md:3411` (requirement)
- Expected behavior: Task 6A commit `feat(project): add fail-closed Windows platform scaffold` 发生在任何 Windows behavior test 之前。它加入 Windows target dependency，并把 Task 2A 的 `include!("unsupported.rs")` 替换为下面这份完整 `windows.rs`。该 scaffold 的唯一职责是让 common facade 在 `x86_64-pc-windows-msvc` 上完整编译；所有 acquisition、I/O、DACL、quarantine、publish、cleanup 都以结构化错误拒绝。它没有 `todo!`、`unimplemented!`、panic 或缺失 symbol。
- Resolution: `reviewed-mapping-report:DS-windows-safe-fs` — Core mapping report DS-windows-safe-fs: windows.rs is still the unsupported backend and needs the complete NT contract plus cross-compile gate.
- Exact acceptance contract:
  - crates/opentake-project/src/safe_fs/windows.rs no longer includes unsupported.rs and provides one compile-complete capability-relative implementation without todo, unimplemented, panic, or bypass variants.
  - Synchronous NT I/O treats STATUS_PENDING as a structured invariant failure without reading unfinished output or silently waiting.
  - Acquisition, DACL checks, I/O, quarantine, no-replace publish, cleanup, reparse-point, source-swap, and cancellation regressions pass on native Windows.
  - The x86_64-pc-windows-msvc build, warnings-denied clippy, repository Windows jobs, and independent review all pass for the exact tree.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-project/src/safe_fs/tests.rs#windows_contract` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#synchronous_nt_pending_is_invariant_error` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-project windows_contract`
  - Run: `cargo test -p opentake-project synchronous_nt_pending_is_invariant_error`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority`, `crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory`, `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-project windows_contract`
  - Run: `cargo test -p opentake-project synchronous_nt_pending_is_invariant_error`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 13: DS-native-receipt-validator (implementation-slice-47753aa6eb656ed7)

**Covered records:**
- `requirement-eb7ba5012fb1494f` (requirement)

**Files:**
- Modify: `crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority`
- Modify: `crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory`
- Modify: `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md`
- Test (reviewed-planned): `scripts/tests/validate-c1b-evidence-test.rb#validates_c1b_receipt_provenance_and_rejects_forgery`

**Candidate-bound contracts:**

#### requirement-eb7ba5012fb1494f

- Candidate/source: `doc-705739b578c75fc8` at `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md:3194` (requirement)
- Expected behavior: 三份 receipt 必须属于同一 workflow run/attempt；receipt id、job id 和 artifact id 各自唯一。每份 `results.md` 必须列 exact task、固定 baseline SHA、predecessor SHA、final SHA、pre/post status、每个 local gate exit、三份 run id/attempt/job id/artifact id/name/digest/SHA、两份 gate-local audit 相对路径、aggregate。`command-ledger.json`、`results.md`、review reports、REST metadata、receipt/log/raw-exit 和 `artifact.zip` 全部必须以 gate-relative path 通过 `confined_file!`；任何 absolute/越界 path 或解析到 gate 外的 symlink 必须拒绝。review reports 先复制到 gate 内的 `reviews/`，validator 不接受外部 report path。validation 脚本必须拒绝：`gh` 缺失/未认证/API 失败、repo/run/job/artifact/workflow/head SHA/digest 不合约、predecessor proof/task-chain 不合约、任一本地自造 JSON 无法被 live API 证实、以及 SHA/receipt/command/audit/clean-status 任一不合约。
- Resolution: `reviewed-mapping-report:DS-native-receipt-validator` — Core mapping report DS-native-receipt-validator: this process/evidence slice must add the tracked Windows RED harness and receipt validator around the safe-fs boundary.
- Exact acceptance contract:
  - All three required receipts come from one live workflow run and attempt with unique receipt, job, and artifact IDs.
  - The validator confines every ledger, result, review, metadata, log, exit, and artifact path to the gate root and rejects absolute, escaping, or external-symlink paths.
  - Live API metadata proves repository, workflow, run, attempt, job, artifact, head SHA, digest, predecessor chain, clean status, and command/audit evidence.
  - Negative fixtures cover missing or unauthenticated gh, API failure, locally fabricated JSON, mismatched identities, and out-of-root paths; the live exact-tree validation passes.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `scripts/tests/validate-c1b-evidence-test.rb#validates_c1b_receipt_provenance_and_rejects_forgery` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `ruby scripts/tests/validate-c1b-evidence-test.rb --name "validates_c1b_receipt_provenance_and_rejects_forgery"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority`, `crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory`, `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `ruby scripts/tests/validate-c1b-evidence-test.rb --name "validates_c1b_receipt_provenance_and_rejects_forgery"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 14: DS-media-resolver-complete (implementation-slice-b9e895cb7d3aef6b)

**Covered records:**
- `requirement-c6caf6aefea9247a` (requirement)

**Files:**
- Modify: `crates/opentake-domain/src/media.rs#MediaResolver`
- Modify: `src-tauri/src/media.rs#relink_media`
- Modify: `docs/upstream-analysis/01-架构与数据流.md`
- Test (existing-owned): `crates/opentake-domain/src/media.rs#resolver_expected_path_external`
- Test (existing-owned): `src-tauri/src/media.rs#dto_reports_file_size_for_present_source`
- Test (existing-owned): `src-tauri/src/media.rs#relink_keeps_same_id_and_clears_missing`

**Candidate-bound contracts:**

#### requirement-c6caf6aefea9247a

- Candidate/source: `doc-bf6666ce4bc65294` at `docs/upstream-analysis/01-架构与数据流.md:128` (requirement)
- Expected behavior: At docs/upstream-analysis/01-架构与数据流.md:128 under “2.5 媒体资产模型(运行时 vs 持久化分离 —— 重要设计)” (gap-marker), the source “| 解析器 | `MediaResolver`(`Models/MediaResolver.swift`) | `media_ref → URL`,区分 offline(文件缺失)/present | 全文件 |” requires this exact behavior: Resolve media_ref to an expected path while distinguishing present/offline sources.
- Resolution: `reviewed-mapping-report:DS-media-resolver-complete` — Core mapping report evidence closure: MediaResolver and relink paths directly prove present/offline recovery semantics, pending exact ledger closure.
- Exact acceptance contract:
  - Source binding: docs/upstream-analysis/01-架构与数据流.md:128; signal=gap-marker; heading=2.5 媒体资产模型(运行时 vs 持久化分离 —— 重要设计); candidate=| 解析器 | `MediaResolver`(`Models/MediaResolver.swift`) | `media_ref → URL`,区分 offline(文件缺失)/present | 全文件 |
  - Expected behavior: Resolve media_ref to an expected path while distinguishing present/offline sources. This closes only the promise expressed by “解析器 | `MediaResolver`(`Models/MediaResolver.swift`) | `media_ref → URL`,区分 offline(文件缺失)/present | 全文件” in “2.5 媒体资产模型(运行时 vs 持久化分离 —— 重要设计)”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “解析器 | `MediaResolver`(`Models/MediaResolver.swift`) | `media_ref → URL`,区分 offline(文件缺失)/present | 全文件” with the scenario below and register test:tools/completion-tests/doc-bf6666ce4bc65294.test.mjs#completion_bf6666ce4bc65294_resolve_media_ref_to_an_expected_path_while_dist
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “解析器 | `MediaResolver`(`Models/MediaResolver.swift`) | `media_ref → URL`,区分 offline(文件缺失)/present | 全文件”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Resolve media_ref to an expected path while distinguishing present/offline sources.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-bf6666ce4bc65294.test.mjs#completion_bf6666ce4bc65294_resolve_media_ref_to_an_expected_path_while_dist.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-domain/src/media.rs#resolver_expected_path_external` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/media.rs#dto_reports_file_size_for_present_source` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/media.rs#relink_keeps_same_id_and_clears_missing` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-domain resolver_expected_path_external`
  - Run: `cargo test -p opentake-tauri dto_reports_file_size_for_present_source`
  - Run: `cargo test -p opentake-tauri relink_keeps_same_id_and_clears_missing`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-domain/src/media.rs#MediaResolver`, `src-tauri/src/media.rs#relink_media`, `docs/upstream-analysis/01-架构与数据流.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-domain resolver_expected_path_external`
  - Run: `cargo test -p opentake-tauri dto_reports_file_size_for_present_source`
  - Run: `cargo test -p opentake-tauri relink_keeps_same_id_and_clears_missing`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.
