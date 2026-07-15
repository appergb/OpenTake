# Data Safety Completion Design

**Gap group:** `data-safety`

**Records:** 32

**Implementation slices:** 14

## Architecture

Close each record as the smallest end-to-end vertical slice while preserving Rust-authoritative state, command/API parity, transactional safety, and explicit pending/empty/failure UI states. A record changes status only after its exact acceptance contract and strongest relevant runtime path pass.

## Record contracts

### requirement-365ac4943b157d3e

- Kind: requirement
- Implementation slice: `implementation-slice-0e3d4d7415150b82`
- Candidate: `doc-4e47fc5b7ea3e13a`
- Source citation: `docs/architecture/MODULE-PORT-MAP.md:145`
- Exact files/symbols: `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-domain/src/clip.rs#Clip`, `crates/opentake-domain/src/media.rs#MediaManifest`, `crates/opentake-project/src/gen_log.rs#GenerationLogEntry`, `docs/architecture/MODULE-PORT-MAP.md`
- Target resolution: `reviewed-mapping-report:DS-legacy-default-matrix`; matched `Project`, `Clip`, `MediaManifest`, `GenerationLogEntry`.
- Resolution rationale: Core mapping report DS-legacy-default-matrix: existing migration tests do not cover the exhaustive documented field plus save/reopen matrix.
- Test ownership:
  - `crates/opentake-project/tests/upstream_compat.rs#applies_clip_defaults_for_omitted_fields` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/upstream_compat.rs#migrates_legacy_transform_xy_to_center` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/upstream_compat.rs#migrates_generation_log_legacy_cost_and_version` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/upstream_compat.rs#exhaustive_legacy_default_matrix` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Decode every listed upstream legacy/default field migration without data loss.
- Acceptance criteria: Implementation: Create fixtures covering every listed missing Timeline/Track/Clip/Manifest field, legacy Transform x/y migration, and GenerationLog cost conversion; make decoding match the documented defaults; block unsafe writes on unknown future fields; pass round-trip compatibility tests. Add deterministic fixtures for every named current, legacy, missing, malformed, and fail-closed branch; the focused round-trip and compatibility suites must pass. Exercise open, edit, save, and reopen on representative bundles and attach the exact implementation symbols plus test or runtime evidence before reclassification.

### requirement-473d4379da3bd4cc

- Kind: requirement
- Implementation slice: `implementation-slice-078ed22bfa23f28a`
- Candidate: `doc-c97237b55fcc36c9`
- Source citation: `docs/modules/opentake-agent/SPEC.md:1069`
- Exact files/symbols: `crates/opentake-agent/src/mcp/server.rs#McpServer`, `crates/opentake-agent/src/mcp/server.rs#serve_with_bridge`, `docs/modules/opentake-agent/SPEC.md`
- Target resolution: `reviewed-mapping-report:DS-mcp-transport`; matched `McpServer`, `serve_with_bridge`.
- Resolution rationale: Core mapping report DS-mcp-transport: loopback bind and protocol-version rejection remain unproved production constraints.
- Test ownership:
  - `crates/opentake-agent/tests/mcp_http.rs#non_local_origin_is_rejected` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#oversized_request_body_is_rejected` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#serve_rejects_non_loopback_bind` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/tests/mcp_http.rs#unsupported_protocol_version_is_400` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Enforce the complete MCP transport guard set on a loopback-only listener.
- Acceptance criteria: Server startup rejects non-loopback bind addresses, not merely the default constant. Origin and Host DNS-rebinding guards, request-size limit, and MCP protocol-version validation are all active and independently tested. HTTP integration tests prove external Host/Origin and unsupported protocol versions never invoke a tool.

### requirement-d317ca3e45fba737

- Kind: requirement
- Implementation slice: `implementation-slice-a5dcb81bd0966174`
- Candidate: `doc-122beba700691cb7`
- Source citation: `docs/modules/opentake-agent/SPEC.md:1071`
- Exact files/symbols: `crates/opentake-agent/src/tools/errors.rs#decode_tool_args`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `src-tauri/src/mcp.rs#TauriMediaBridge`, `docs/modules/opentake-agent/SPEC.md`
- Target resolution: `reviewed-mapping-report:DS-mcp-tool-import`; matched `decode_tool_args`, `Dispatcher`, `TauriMediaBridge`.
- Resolution rationale: Core mapping report DS-mcp-tool-import: HTTPS-only streaming, typed MIME and decoded-size enforcement are not wired through the bridge.
- Test ownership:
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_requires_exactly_one_source` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_rejects_unknown_nested_source_key` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#import_media_bytes_rejects_oversized_base64_before_bridge` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/tool_argument_contract.rs#all_tool_schemas_reject_unknown_missing_wrong_type` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `src-tauri/src/mcp.rs#https_url_import_enforces_scheme_mime_and_decoded_limit` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Validate every tool argument and make URL media import HTTPS-only, whitelist typed media, and stream with a hard 1 GB decoded-byte ceiling.
- Acceptance criteria: Complete the nested argument guards described by doc-d1dee193811e9dbb. Reject non-HTTPS/userinfo/redirect-to-non-HTTPS URLs before I/O; infer or validate the extension/MIME allowlist; stream to a staging file while enforcing the 1 GB decoded-byte cap across redirects. Publish only after download, type/probe, and retained-project validation; cancellation/error cleans staging and leaves manifest unchanged.

### requirement-2e9e6066655d5846

- Kind: requirement
- Implementation slice: `implementation-slice-673f9e3f6002f97b`
- Candidate: `doc-e13f2cbc316a11fa`
- Source citation: `docs/modules/opentake-agent/SPEC.md:1072`
- Exact files/symbols: `src-tauri/src/mcp.rs#TauriMediaBridge`, `crates/opentake-agent/src/mcp/convert.rs#to_call_tool_result`, `docs/modules/opentake-agent/SPEC.md`
- Target resolution: `reviewed-mapping-report:DS-mcp-redaction`; matched `TauriMediaBridge`, `to_call_tool_result`.
- Resolution rationale: Core mapping report DS-mcp-redaction: bridge errors still need a focused LLM-boundary redaction matrix for paths, credentials, headers and provider bodies.
- Test ownership:
  - `crates/opentake-agent/tests/mcp_error_redaction.rs#llm_errors_redact_paths_credentials_headers_provider_bodies` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Return actionable MCP errors without exposing filesystem paths, credentials, authorization headers, provider response bodies, or internal stack detail.
- Acceptance criteria: Introduce a boundary sanitizer with typed safe error codes/details and private structured logging. Adversarial tests inject a home path, API key, bearer token, signed URL query, provider body, and nested source error and assert none appear in MCP content while remediation remains actionable.

### requirement-b9010e6717b5d5ea

- Kind: requirement
- Implementation slice: `implementation-slice-4f2d8bcaff47a37f`
- Candidate: `doc-d0e72203255f0a41`
- Source citation: `docs/modules/opentake-core/SPEC.md:423`
- Exact files/symbols: `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/session.rs#EditorSession`, `docs/modules/opentake-core/SPEC.md`
- Target resolution: `reviewed-mapping-report:DS-generation-seed`; matched `Project`, `EditorSession`.
- Resolution rationale: Core mapping report DS-generation-seed: project open currently defaults an absent generation log instead of seeding manifest provenance once.
- Test ownership:
  - `crates/opentake-project/tests/roundtrip.rs#malformed_generation_log_is_ignored` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/tests/project_open.rs#missing_generation_log_seeds_manifest_provenance_once` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: When generation-log.json is absent, seed the generation log from manifest entries carrying generation provenance, matching the upstream project-open migration.
- Acceptance criteria: Define a deterministic seed_generation_log_from_assets conversion with stable ids, model, cost, and created-at semantics. Call it only when no valid generation log exists, never duplicate entries when a valid log is present, and persist the seeded log on the next save. Tests cover empty manifest, mixed generated/imported assets, duplicate provenance, malformed optional log, reopen, and byte-stable save.

### requirement-52da354b46176a99

- Kind: requirement
- Implementation slice: `implementation-slice-5af4f1ababc7b495`
- Candidate: `doc-1cb2f0539425a14e`
- Source citation: `docs/modules/opentake-media/SPEC.md:163`
- Exact files/symbols: `crates/opentake-media/src/cache_key.rs#file_identity_key`, `crates/opentake-media/src/cache_key.rs#identity_hex`, `docs/modules/opentake-media/SPEC.md`
- Target resolution: `reviewed-mapping-report:DS-cache-identity-complete`; matched `file_identity_key`, `identity_hex`.
- Resolution rationale: Core mapping report evidence closure: cache identity has direct tracked implementation and focused stability/missing-file tests, pending exact ledger closure.
- Test ownership:
  - `crates/opentake-media/src/cache_key.rs#identity_hex_is_stable_and_lowercase` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/cache_key.rs#identity_hex_matches_swift_for_whole_second_mtime` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/cache_key.rs#file_identity_key_missing_file_is_none` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/modules/opentake-media/SPEC.md:163 under “1.4 通用缓存键(三处共用)” (gap-marker), the source “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。” requires this exact behavior: Derive a stable lowercase 32-hex file identity from path, Swift-style mtime, and size, returning None for missing metadata.
- Acceptance criteria: Source binding: docs/modules/opentake-media/SPEC.md:163; signal=gap-marker; heading=1.4 通用缓存键(三处共用); candidate=> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。 Expected behavior: Derive a stable lowercase 32-hex file identity from path, Swift-style mtime, and size, returning None for missing metadata. This closes only the promise expressed by “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。” in “1.4 通用缓存键(三处共用)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。” with the scenario below and register test:crates/opentake-project/tests/completion_1cb2f0539425a14e.rs#completion_1cb2f0539425a14e_derive_a_stable_lowercase_32_hex_file_identity_f Initial state/input/event: start from the smallest valid fixture for “Derive a stable lowercase 32-hex file identity from path, Swift-style mtime, and size, returning None for missing metadata.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “> ⚠️ **逐字节核对点**:`MediaVisualCache` 用 `digest.prefix(16).map{%02x}` → 16 字节 → **32 hex 字符**;`EmbeddingStore`/`TranscriptCache` 用 `.map{%02x}.joined().prefix(32)` → **32 hex 字符 = 16 字节**。两者最终都是 32 个 hex 字符、16 字节熵,但代码路径不同。实施时统一为「取 SHA256 前 16 字节 → 32 hex」即与三处全部一致。`file_identity_key(path, 32)` 返回 32 hex 字符即可。`mtime`/`size` 缺失返回 `None`(对应上游 `guard let … else return nil`)。”. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Derive a stable lowercase 32-hex file identity from path, Swift-style mtime, and size, returning None for missing metadata.” once. Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-project/tests/completion_1cb2f0539425a14e.rs#completion_1cb2f0539425a14e_derive_a_stable_lowercase_32_hex_file_identity_f.

### requirement-44e32f4f5265f1dc

- Kind: requirement
- Implementation slice: `implementation-slice-63ec0e639957e775`
- Candidate: `doc-7b33a412558e55a9`
- Source citation: `docs/specs/core/1-editor-state.md:1`
- Exact files/symbols: `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/1-editor-state.md`
- Target resolution: `reviewed-mapping-report:DS-shared-core-command-complete`; matched `EditorState`, `EditCommand`, `AppCore`, `EventBus`.
- Resolution rationale: Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Test ownership:
  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/1-editor-state.md:1 under “1. EditorState 结构” (heading), the source “## 1. EditorState 结构” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Acceptance criteria: Source binding: docs/specs/core/1-editor-state.md:1; signal=heading; heading=1. EditorState 结构; candidate=## 1. EditorState 结构 Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “1. EditorState 结构” in “1. EditorState 结构”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “1. EditorState 结构” with the scenario below and register test:tools/completion-tests/doc-7b33a412558e55a9.test.mjs#completion_7b33a412558e55a9_editor_state_and_edits_are_owned_by_the_shared_r Initial state/input/event: create an isolated temporary project fixture representing the source state in “1. EditorState 结构”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-7b33a412558e55a9.test.mjs#completion_7b33a412558e55a9_editor_state_and_edits_are_owned_by_the_shared_r.

### requirement-adb4fb8f4521a41f

- Kind: requirement
- Implementation slice: `implementation-slice-63ec0e639957e775`
- Candidate: `doc-715cc47e6276015e`
- Source citation: `docs/specs/core/1-editor-state.md:3`
- Exact files/symbols: `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/1-editor-state.md`
- Target resolution: `reviewed-mapping-report:DS-shared-core-command-complete`; matched `EditorState`, `EditCommand`, `AppCore`, `EventBus`.
- Resolution rationale: Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Test ownership:
  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/1-editor-state.md:3 under “1.1 职责与对应上游” (heading), the source “### 1.1 职责与对应上游” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Acceptance criteria: Source binding: docs/specs/core/1-editor-state.md:3; signal=heading; heading=1.1 职责与对应上游; candidate=### 1.1 职责与对应上游 Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “1.1 职责与对应上游” in “1.1 职责与对应上游”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “1.1 职责与对应上游” with the scenario below and register test:tools/completion-tests/doc-715cc47e6276015e.test.mjs#completion_715cc47e6276015e_editor_state_and_edits_are_owned_by_the_shared_r Initial state/input/event: create an isolated temporary project fixture representing the source state in “1.1 职责与对应上游”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-715cc47e6276015e.test.mjs#completion_715cc47e6276015e_editor_state_and_edits_are_owned_by_the_shared_r.

### requirement-978fe275600796df

- Kind: requirement
- Implementation slice: `implementation-slice-63ec0e639957e775`
- Candidate: `doc-8faa0d04f0c889c1`
- Source citation: `docs/specs/core/1-editor-state.md:9`
- Exact files/symbols: `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/1-editor-state.md`
- Target resolution: `reviewed-mapping-report:DS-shared-core-command-complete`; matched `EditorState`, `EditCommand`, `AppCore`, `EventBus`.
- Resolution rationale: Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Test ownership:
  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/1-editor-state.md:9 under “1.2 字段(草案)” (heading), the source “### 1.2 字段(草案)” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Acceptance criteria: Source binding: docs/specs/core/1-editor-state.md:9; signal=heading; heading=1.2 字段(草案); candidate=### 1.2 字段(草案) Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “1.2 字段(草案)” in “1.2 字段(草案)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “1.2 字段(草案)” with the scenario below and register test:tools/completion-tests/doc-8faa0d04f0c889c1.test.mjs#completion_8faa0d04f0c889c1_editor_state_and_edits_are_owned_by_the_shared_r Initial state/input/event: create an isolated temporary project fixture representing the source state in “1.2 字段(草案)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-8faa0d04f0c889c1.test.mjs#completion_8faa0d04f0c889c1_editor_state_and_edits_are_owned_by_the_shared_r.

### requirement-a7fcfeeabb9eca2b

- Kind: requirement
- Implementation slice: `implementation-slice-63ec0e639957e775`
- Candidate: `doc-cf44660f6e5bd373`
- Source citation: `docs/specs/core/1-editor-state.md:49`
- Exact files/symbols: `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/1-editor-state.md`
- Target resolution: `reviewed-mapping-report:DS-shared-core-command-complete`; matched `EditorState`, `EditCommand`, `AppCore`, `EventBus`.
- Resolution rationale: Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Test ownership:
  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/1-editor-state.md:49 under “1.3 访问封装:`EditorCore`” (heading), the source “### 1.3 访问封装:`EditorCore`” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Acceptance criteria: Source binding: docs/specs/core/1-editor-state.md:49; signal=heading; heading=1.3 访问封装:`EditorCore`; candidate=### 1.3 访问封装:`EditorCore` Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “1.3 访问封装:`EditorCore`” in “1.3 访问封装:`EditorCore`”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “1.3 访问封装:`EditorCore`” with the scenario below and register test:tools/completion-tests/doc-cf44660f6e5bd373.test.mjs#completion_cf44660f6e5bd373_editor_state_and_edits_are_owned_by_the_shared_r Initial state/input/event: create an isolated temporary project fixture representing the source state in “1.3 访问封装:`EditorCore`”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-cf44660f6e5bd373.test.mjs#completion_cf44660f6e5bd373_editor_state_and_edits_are_owned_by_the_shared_r.

### requirement-2d95c75bfdf43afd

- Kind: requirement
- Implementation slice: `implementation-slice-63ec0e639957e775`
- Candidate: `doc-38f0c6dea35ccc52`
- Source citation: `docs/specs/core/2-command-routing.md:1`
- Exact files/symbols: `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/2-command-routing.md`
- Target resolution: `reviewed-mapping-report:DS-shared-core-command-complete`; matched `EditorState`, `EditCommand`, `AppCore`, `EventBus`.
- Resolution rationale: Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Test ownership:
  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/2-command-routing.md:1 under “2. 命令路由 = 上游 ToolExecutor 单一能力层” (heading), the source “## 2. 命令路由 = 上游 ToolExecutor 单一能力层” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Acceptance criteria: Source binding: docs/specs/core/2-command-routing.md:1; signal=heading; heading=2. 命令路由 = 上游 ToolExecutor 单一能力层; candidate=## 2. 命令路由 = 上游 ToolExecutor 单一能力层 Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2. 命令路由 = 上游 ToolExecutor 单一能力层” in “2. 命令路由 = 上游 ToolExecutor 单一能力层”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “2. 命令路由 = 上游 ToolExecutor 单一能力层” with the scenario below and register test:crates/opentake-agent/tests/completion_38f0c6dea35ccc52.rs#completion_38f0c6dea35ccc52_editor_state_and_edits_are_owned_by_the_shared_r Initial state/input/event: create an isolated temporary project fixture representing the source state in “2. 命令路由 = 上游 ToolExecutor 单一能力层”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_38f0c6dea35ccc52.rs#completion_38f0c6dea35ccc52_editor_state_and_edits_are_owned_by_the_shared_r.

### requirement-90e95608564f8bc4

- Kind: requirement
- Implementation slice: `implementation-slice-63ec0e639957e775`
- Candidate: `doc-b2159a4af40efa5d`
- Source citation: `docs/specs/core/2-command-routing.md:3`
- Exact files/symbols: `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/2-command-routing.md`
- Target resolution: `reviewed-mapping-report:DS-shared-core-command-complete`; matched `EditorState`, `EditCommand`, `AppCore`, `EventBus`.
- Resolution rationale: Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Test ownership:
  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/2-command-routing.md:3 under “2.1 核心原则:一处定义,三客户端共享” (heading), the source “### 2.1 核心原则:一处定义,三客户端共享” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Acceptance criteria: Source binding: docs/specs/core/2-command-routing.md:3; signal=heading; heading=2.1 核心原则:一处定义,三客户端共享; candidate=### 2.1 核心原则:一处定义,三客户端共享 Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2.1 核心原则:一处定义,三客户端共享” in “2.1 核心原则:一处定义,三客户端共享”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “2.1 核心原则:一处定义,三客户端共享” with the scenario below and register test:tools/completion-tests/doc-b2159a4af40efa5d.test.mjs#completion_b2159a4af40efa5d_editor_state_and_edits_are_owned_by_the_shared_r Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.1 核心原则:一处定义,三客户端共享”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-b2159a4af40efa5d.test.mjs#completion_b2159a4af40efa5d_editor_state_and_edits_are_owned_by_the_shared_r.

### requirement-479246e3a4dd5891

- Kind: requirement
- Implementation slice: `implementation-slice-63ec0e639957e775`
- Candidate: `doc-149099f8c0e60570`
- Source citation: `docs/specs/core/2-command-routing.md:15`
- Exact files/symbols: `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/2-command-routing.md`
- Target resolution: `reviewed-mapping-report:DS-shared-core-command-complete`; matched `EditorState`, `EditCommand`, `AppCore`, `EventBus`.
- Resolution rationale: Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Test ownership:
  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/2-command-routing.md:15 under “2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)” (heading), the source “### 2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Acceptance criteria: Source binding: docs/specs/core/2-command-routing.md:15; signal=heading; heading=2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5); candidate=### 2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5) Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)” in “2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)” with the scenario below and register test:tools/completion-tests/doc-149099f8c0e60570.test.mjs#completion_149099f8c0e60570_editor_state_and_edits_are_owned_by_the_shared_r Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.2 `EditCommand` 与 `EditResult`(落地 ARCHITECTURE §5)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-149099f8c0e60570.test.mjs#completion_149099f8c0e60570_editor_state_and_edits_are_owned_by_the_shared_r.

### requirement-1598d28c23e8cd9b

- Kind: requirement
- Implementation slice: `implementation-slice-63ec0e639957e775`
- Candidate: `doc-dc4980e1cf95b7d2`
- Source citation: `docs/specs/core/2-command-routing.md:52`
- Exact files/symbols: `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/2-command-routing.md`
- Target resolution: `reviewed-mapping-report:DS-shared-core-command-complete`; matched `EditorState`, `EditCommand`, `AppCore`, `EventBus`.
- Resolution rationale: Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Test ownership:
  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/2-command-routing.md:52 under “2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)” (heading), the source “### 2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Acceptance criteria: Source binding: docs/specs/core/2-command-routing.md:52; signal=heading; heading=2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`); candidate=### 2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`) Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)” in “2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)” with the scenario below and register test:web/src/__tests__/completion/doc-dc4980e1cf95b7d2.test.ts#completion_dc4980e1cf95b7d2_editor_state_and_edits_are_owned_by_the_shared_r Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.3 `EditorCore::apply` —— 事务核心(直译 `withTimelineSwap`)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-dc4980e1cf95b7d2.test.ts#completion_dc4980e1cf95b7d2_editor_state_and_edits_are_owned_by_the_shared_r.

### requirement-26e566086bff8227

- Kind: requirement
- Implementation slice: `implementation-slice-63ec0e639957e775`
- Candidate: `doc-97047ee2cef7bff4`
- Source citation: `docs/specs/core/2-command-routing.md:102`
- Exact files/symbols: `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/2-command-routing.md`
- Target resolution: `reviewed-mapping-report:DS-shared-core-command-complete`; matched `EditorState`, `EditCommand`, `AppCore`, `EventBus`.
- Resolution rationale: Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Test ownership:
  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/2-command-routing.md:102 under “2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)” (heading), the source “### 2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Acceptance criteria: Source binding: docs/specs/core/2-command-routing.md:102; signal=heading; heading=2.4 Undo / Redo —— 双层撤销模型(精确复刻上游); candidate=### 2.4 Undo / Redo —— 双层撤销模型(精确复刻上游) Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)” in “2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)” with the scenario below and register test:tools/completion-tests/doc-97047ee2cef7bff4.test.mjs#completion_97047ee2cef7bff4_editor_state_and_edits_are_owned_by_the_shared_r Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.4 Undo / Redo —— 双层撤销模型(精确复刻上游)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-97047ee2cef7bff4.test.mjs#completion_97047ee2cef7bff4_editor_state_and_edits_are_owned_by_the_shared_r.

### requirement-a2923e1fa84e4a81

- Kind: requirement
- Implementation slice: `implementation-slice-63ec0e639957e775`
- Candidate: `doc-c95515ae8f048e8a`
- Source citation: `docs/specs/core/2-command-routing.md:140`
- Exact files/symbols: `crates/opentake-ops/src/editor_state.rs#EditorState`, `crates/opentake-ops/src/command.rs#EditCommand`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-core/src/events.rs#EventBus`, `docs/specs/core/2-command-routing.md`
- Target resolution: `reviewed-mapping-report:DS-shared-core-command-complete`; matched `EditorState`, `EditCommand`, `AppCore`, `EventBus`.
- Resolution rationale: Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.
- Test ownership:
  - `crates/opentake-ops/src/editor_state.rs#commit_undo_redo_cycle_restores_and_versions` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#apply_bumps_version_and_emits_once` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#unchanged_command_does_not_emit_or_bump` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#undo_redo_through_core_bumps_version_and_emits` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/specs/core/2-command-routing.md:140 under “2.5 三客户端如何共享(装配视角)” (heading), the source “### 2.5 三客户端如何共享(装配视角)” requires this exact behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.
- Acceptance criteria: Source binding: docs/specs/core/2-command-routing.md:140; signal=heading; heading=2.5 三客户端如何共享(装配视角); candidate=### 2.5 三客户端如何共享(装配视角) Expected behavior: Editor state and edits are owned by the shared Rust core/command path with undo and versioned results. This closes only the promise expressed by “2.5 三客户端如何共享(装配视角)” in “2.5 三客户端如何共享(装配视角)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “2.5 三客户端如何共享(装配视角)” with the scenario below and register test:tools/completion-tests/doc-c95515ae8f048e8a.test.mjs#completion_c95515ae8f048e8a_editor_state_and_edits_are_owned_by_the_shared_r Initial state/input/event: create an isolated temporary project fixture representing the source state in “2.5 三客户端如何共享(装配视角)”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Editor state and edits are owned by the shared Rust core/command path with undo and versioned results.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-c95515ae8f048e8a.test.mjs#completion_c95515ae8f048e8a_editor_state_and_edits_are_owned_by_the_shared_r.

### requirement-6748d221ef0d9a4c

- Kind: requirement
- Implementation slice: `implementation-slice-0d21c3ea6ba4327b`
- Candidate: `doc-5a5785b4e46c5875`
- Source citation: `docs/specs/core/5-assembly.md:1`
- Exact files/symbols: `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/session.rs#EditorSession`, `crates/opentake-domain/src/media.rs#MediaManifest`, `crates/opentake-project/src/gen_log.rs#GenerationLogEntry`, `docs/specs/core/5-assembly.md`
- Target resolution: `reviewed-mapping-report:DS-project-open-composite-headings`; matched `Project`, `EditorSession`, `MediaManifest`, `GenerationLogEntry`.
- Resolution rationale: Core mapping report: four project-open umbrellas are composite acceptance over the legacy-default and generation-seed child slices, not independent top-one features.
- Test ownership:
  - `crates/opentake-project/tests/upstream_compat.rs#exhaustive_legacy_default_matrix` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/tests/project_open.rs#missing_generation_log_seeds_manifest_provenance_once` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/tests/project_open.rs#project_open_composite_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Project open assembles validated project/media/generation state without silent loss or placeholder dependencies.
- Acceptance criteria: Open a project by validating bundle, timeline, media index, generation log, caches, render/playback, and Agent dependencies before publishing editor state. A failed dependency or malformed file must return a typed error/recovery result without silently replacing user data or partially publishing a session. Test valid, legacy, missing optional, malformed media.json, malformed generation log, dependency failure, save, close, and reopen with byte/semantic equality.

### requirement-9335bc98b18f8d8d

- Kind: requirement
- Implementation slice: `implementation-slice-0d21c3ea6ba4327b`
- Candidate: `doc-7be80db04be852c6`
- Source citation: `docs/specs/core/5-assembly.md:21`
- Exact files/symbols: `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/session.rs#EditorSession`, `crates/opentake-domain/src/media.rs#MediaManifest`, `crates/opentake-project/src/gen_log.rs#GenerationLogEntry`, `docs/specs/core/5-assembly.md`
- Target resolution: `reviewed-mapping-report:DS-project-open-composite-headings`; matched `Project`, `EditorSession`, `MediaManifest`, `GenerationLogEntry`.
- Resolution rationale: Core mapping report: four project-open umbrellas are composite acceptance over the legacy-default and generation-seed child slices, not independent top-one features.
- Test ownership:
  - `crates/opentake-project/tests/upstream_compat.rs#exhaustive_legacy_default_matrix` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/tests/project_open.rs#missing_generation_log_seeds_manifest_provenance_once` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/tests/project_open.rs#project_open_composite_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Project open assembles validated project/media/generation state without silent loss or placeholder dependencies.
- Acceptance criteria: Replace placeholder CoreDeps paths with explicit project/media/render/playback/Agent interfaces whose production implementations are injected by src-tauri. Provide deterministic fakes for each dependency and ensure core never imports Tauri/UI/provider concrete types. Compile dependency-direction checks and test each dependency failure/cancellation without partial state or leaked background work.

### requirement-706e744a85684655

- Kind: requirement
- Implementation slice: `implementation-slice-0d21c3ea6ba4327b`
- Candidate: `doc-8fb894b0ea5ce82e`
- Source citation: `docs/specs/core/5-assembly.md:36`
- Exact files/symbols: `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/session.rs#EditorSession`, `crates/opentake-domain/src/media.rs#MediaManifest`, `crates/opentake-project/src/gen_log.rs#GenerationLogEntry`, `docs/specs/core/5-assembly.md`
- Target resolution: `reviewed-mapping-report:DS-project-open-composite-headings`; matched `Project`, `EditorSession`, `MediaManifest`, `GenerationLogEntry`.
- Resolution rationale: Core mapping report: four project-open umbrellas are composite acceptance over the legacy-default and generation-seed child slices, not independent top-one features.
- Test ownership:
  - `crates/opentake-project/tests/upstream_compat.rs#exhaustive_legacy_default_matrix` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/tests/project_open.rs#missing_generation_log_seeds_manifest_provenance_once` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/tests/project_open.rs#project_open_composite_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Project open assembles validated project/media/generation state without silent loss or placeholder dependencies.
- Acceptance criteria: Map each project/media/render/Agent assembly point to its current implementation symbol and remove stale upstream-only or placeholder references. Exercise each mapped boundary through CoreDeps/CoreHandle rather than direct frontend or Agent state mutation. Add a contract test that opens one fixture and invokes media lookup, render plan, playback route, Agent read, save, and reopen through the mapped interfaces.

### requirement-b38818cf815e0f1e

- Kind: requirement
- Implementation slice: `implementation-slice-0d21c3ea6ba4327b`
- Candidate: `doc-e286de761da93671`
- Source citation: `docs/specs/core/5-assembly.md:48`
- Exact files/symbols: `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/session.rs#EditorSession`, `crates/opentake-domain/src/media.rs#MediaManifest`, `crates/opentake-project/src/gen_log.rs#GenerationLogEntry`, `docs/specs/core/5-assembly.md`
- Target resolution: `reviewed-mapping-report:DS-project-open-composite-headings`; matched `Project`, `EditorSession`, `MediaManifest`, `GenerationLogEntry`.
- Resolution rationale: Core mapping report: four project-open umbrellas are composite acceptance over the legacy-default and generation-seed child slices, not independent top-one features.
- Test ownership:
  - `crates/opentake-project/tests/upstream_compat.rs#exhaustive_legacy_default_matrix` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/tests/project_open.rs#missing_generation_log_seeds_manifest_provenance_once` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/tests/project_open.rs#project_open_composite_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Project open assembles validated project/media/generation state without silent loss or placeholder dependencies.
- Acceptance criteria: Implement open ordering as validate bundle → load timeline/media/generation log → build deps/session → publish snapshot/events → start optional background work. On failure at every step, close already-created resources, publish no usable session, and preserve all project files unchanged. Fault-inject every open step and assert cleanup/event order, then open/save/reopen a valid and migrated fixture with identical IDs, frames, media, and generation history.

### requirement-1f35cc4131f8f0b7

- Kind: requirement
- Implementation slice: `implementation-slice-4f2d8bcaff47a37f`
- Candidate: `doc-10feadcab19f56dd`
- Source citation: `docs/specs/core/5-assembly.md:54`
- Exact files/symbols: `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/session.rs#EditorSession`, `docs/specs/core/5-assembly.md`
- Target resolution: `reviewed-mapping-report:DS-generation-seed`; matched `Project`, `EditorSession`.
- Resolution rationale: Core mapping report DS-generation-seed: project open currently defaults an absent generation log instead of seeding manifest provenance once.
- Test ownership:
  - `crates/opentake-project/tests/roundtrip.rs#malformed_generation_log_is_ignored` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/tests/project_open.rs#missing_generation_log_seeds_manifest_provenance_once` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Load the optional generation log and seed legacy generated assets when it is absent.
- Acceptance criteria: Implementation: After lenient generation-log decode, scan manifest generation metadata to reconstruct deterministic legacy log entries without duplicates; persist on the next safe save; test absent/corrupt/existing/partial logs and idempotent reopen. Add deterministic fixtures for every named current, legacy, missing, malformed, and fail-closed branch; the focused round-trip and compatibility suites must pass. Exercise open, edit, save, and reopen on representative bundles and attach the exact implementation symbols plus test or runtime evidence before reclassification.

### requirement-7908bd026b5a91f6

- Kind: requirement
- Implementation slice: `implementation-slice-38c616831a6a7bd5`
- Candidate: `doc-c5eeda123dd2c6ca`
- Source citation: `docs/specs/core/5-assembly.md:58`
- Exact files/symbols: `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-domain/src/media.rs#MediaManifest`, `docs/specs/core/5-assembly.md`
- Target resolution: `reviewed-mapping-report:DS-manifest-corruption-conflict`; matched `Project`, `MediaManifest`.
- Resolution rationale: Core mapping report: the source contradicts the fail-closed malformed-manifest contract; acceptance text must be reconciled before product work.
- Test ownership:
  - `crates/opentake-project/tests/roundtrip.rs#malformed_manifest_is_an_error` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/tests/schema_compat.rs#malformed_manifest_contract_matches_authoritative_source` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Open legacy projects with defaulted optional manifest/generation data while keeping project.json strict.
- Acceptance criteria: Implementation: Keep project.json strict; default missing media.json/generation-log.json; decide and implement recovery for malformed media.json consistent with the spec (including compatibility blocker/read-only safety); test every missing/malformed combination and safe-save behavior. Add deterministic fixtures for every named current, legacy, missing, malformed, and fail-closed branch; the focused round-trip and compatibility suites must pass. Exercise open, edit, save, and reopen on representative bundles and attach the exact implementation symbols plus test or runtime evidence before reclassification.

### requirement-37acd430c8e1e82b

- Kind: requirement
- Implementation slice: `implementation-slice-b97a3dae0f6b3e32`
- Candidate: `doc-e1c7a6f1c3605364`
- Source citation: `docs/specs/core/7-security.md:1`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-ops/src/command.rs#EditCommand`, `docs/specs/core/7-security.md`
- Target resolution: `reviewed-mapping-report:DS-cross-cutting-security-headings`; matched `Dispatcher`, `Project`, `AppCore`, `EditCommand`.
- Resolution rationale: Core mapping report: these umbrellas cross MCP, safe-fs, playback, export and Agent boundaries and therefore require composite child-slice acceptance.
- Test ownership:
  - `crates/opentake-agent/src/mcp/dispatch.rs#cross_cutting_mcp_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/tests/schema_compat.rs#cross_cutting_project_safety_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/src/core.rs#cross_cutting_runtime_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-ops/tests/command_apply.rs#cross_cutting_command_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Desktop command, asset, sidecar, and MCP boundaries fail closed on every packaged platform.
- Acceptance criteria: Close Windows CSP/WebView2/sidecar permission and path validation gaps. Add packaged-build security tests for asset scope, origins, traversal, and sidecar invocation. Document signing/notarization and secret-handling evidence.

### requirement-43bc7b61f5b484f7

- Kind: requirement
- Implementation slice: `implementation-slice-b97a3dae0f6b3e32`
- Candidate: `doc-954a8799d333db54`
- Source citation: `docs/specs/core/8-implementation.md:1`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-ops/src/command.rs#EditCommand`, `docs/specs/core/8-implementation.md`
- Target resolution: `reviewed-mapping-report:DS-cross-cutting-security-headings`; matched `Dispatcher`, `Project`, `AppCore`, `EditCommand`.
- Resolution rationale: Core mapping report: these umbrellas cross MCP, safe-fs, playback, export and Agent boundaries and therefore require composite child-slice acceptance.
- Test ownership:
  - `crates/opentake-agent/src/mcp/dispatch.rs#cross_cutting_mcp_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/tests/schema_compat.rs#cross_cutting_project_safety_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/src/core.rs#cross_cutting_runtime_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-ops/tests/command_apply.rs#cross_cutting_command_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Core assembly and cross-cutting validation cover safe reopen, sync, playback, export, and Agent clients.
- Acceptance criteria: Close assembly data safety, frontend authority, packaged security, and cross-client parity while preserving completed pure-core/Tauri/Agent stages. Every open/edit/undo/save/reopen path must use typed DTOs/commands and fail without silent data replacement. Pass corrupt-fixture, migration, schema parity, concurrent version, playback/export, Agent, and packaged desktop integration suites.

### requirement-47a9f1e3f7c11b4c

- Kind: requirement
- Implementation slice: `implementation-slice-b97a3dae0f6b3e32`
- Candidate: `doc-b309f10e20832ec1`
- Source citation: `docs/specs/core/8-implementation.md:18`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-ops/src/command.rs#EditCommand`, `docs/specs/core/8-implementation.md`
- Target resolution: `reviewed-mapping-report:DS-cross-cutting-security-headings`; matched `Dispatcher`, `Project`, `AppCore`, `EditCommand`.
- Resolution rationale: Core mapping report: these umbrellas cross MCP, safe-fs, playback, export and Agent boundaries and therefore require composite child-slice acceptance.
- Test ownership:
  - `crates/opentake-agent/src/mcp/dispatch.rs#cross_cutting_mcp_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/tests/schema_compat.rs#cross_cutting_project_safety_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/src/core.rs#cross_cutting_runtime_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-ops/tests/command_apply.rs#cross_cutting_command_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Core assembly and cross-cutting validation cover safe reopen, sync, playback, export, and Agent clients.
- Acceptance criteria: Inject production project/media/render/playback dependencies, seed generation-log state, and parse media.json without silent fallback. Open failures must clean up partial sessions and leave bundle bytes unchanged; save must atomically persist timeline/media/generation state. Fault-inject each dependency and run valid/legacy/corrupt save-reopen fixtures asserting IDs, frames, media, and generation history.

### requirement-debab57411dd52fb

- Kind: requirement
- Implementation slice: `implementation-slice-b97a3dae0f6b3e32`
- Candidate: `doc-30748aed19c57fcd`
- Source citation: `docs/specs/core/8-implementation.md:37`
- Exact files/symbols: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher`, `crates/opentake-project/src/bundle.rs#Project`, `crates/opentake-core/src/core.rs#AppCore`, `crates/opentake-ops/src/command.rs#EditCommand`, `docs/specs/core/8-implementation.md`
- Target resolution: `reviewed-mapping-report:DS-cross-cutting-security-headings`; matched `Dispatcher`, `Project`, `AppCore`, `EditCommand`.
- Resolution rationale: Core mapping report: these umbrellas cross MCP, safe-fs, playback, export and Agent boundaries and therefore require composite child-slice acceptance.
- Test ownership:
  - `crates/opentake-agent/src/mcp/dispatch.rs#cross_cutting_mcp_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/tests/schema_compat.rs#cross_cutting_project_safety_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-core/src/core.rs#cross_cutting_runtime_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-ops/tests/command_apply.rs#cross_cutting_command_acceptance` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Core assembly and cross-cutting validation cover safe reopen, sync, playback, export, and Agent clients.
- Acceptance criteria: Prove Rust/Web DTO schema parity, all-client command equivalence, version ordering, undo/redo, save/reopen, and event sequencing. Run the same edit vectors through desktop UI and Agent dispatch and compare EditResult, timeline JSON, version, and undo outcome exactly. Add concurrency, malformed/corrupt project, playback/export artifact, security-boundary, and packaged macOS/Windows smoke coverage.

### requirement-67e50cfe6dd7f49a

- Kind: requirement
- Implementation slice: `implementation-slice-e5c7ae10bcfd7edf`
- Candidate: `doc-72e29816dc2090ed`
- Source citation: `docs/superpowers/plans/c1b/2026-07-12-c1b-common-unix-normative.md:2187`
- Exact files/symbols: `crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority`, `crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory`, `docs/superpowers/plans/c1b/2026-07-12-c1b-common-unix-normative.md`
- Target resolution: `reviewed-mapping-report:DS-unix-consuming-tests`; matched `DirectoryAuthority`, `capture_absolute_directory`.
- Resolution rationale: Core mapping report DS-unix-consuming-tests: the six named mutation/cleanup contracts are absent and unix.rs still includes the unsupported backend.
- Test ownership:
  - `crates/opentake-project/src/safe_fs/tests.rs#source_swap_before_quarantine_restores_without_deletion` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#restore_collision_fail_leaks_original_and_quarantine` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#final_unix_name_window_is_explicit_same_account_boundary` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#cleanup_capability_records_identity_before_consuming_delete` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#destination_collision_preserves_stage_and_every_destination_kind` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: The test-only commit adds exactly these six public consuming mutation/cleanup tests and no others: `source_swap_before_quarantine_restores_without_deletion`, `restore_collision_fail_leaks_original_and_quarantine`, `final_unix_name_window_is_explicit_same_account_boundary`, `cleanup_capability_records_identity_before_consuming_delete`, `nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories`, and `destination_collision_preserves_stage_and_every_destination_kind`. All ten post-create rollback regressions were already committed before their Task 4 implementation and passed at the reviewed Task 4 GREEN SHA; Task 5 neither moves nor re-adds them. The focused RED is the single named recursive-cleanup test below and fails only because Task 4's approved public mutation stub refuses.
- Acceptance criteria: crates/opentake-project/src/safe_fs/unix.rs no longer includes unsupported.rs and implements capability-relative no-follow acquisition, I/O, quarantine, no-replace publish, and recursive cleanup. The six named public consuming mutation/cleanup regressions and all ten post-create rollback regressions pass against production Unix code. Symlink, FIFO, source-swap, identity-change, restore-collision, destination-collision, and cross-account/name-window cases fail closed without data loss. Rust workspace tests, warnings-denied clippy, and native Unix/Linux receipt gates pass on the exact reviewed tree.

### requirement-db25fa5cad389d60

- Kind: requirement
- Implementation slice: `implementation-slice-c7f1cd8463f97ad5`
- Candidate: `doc-d32f0d9172a7d88e`
- Source citation: `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md:96`
- Exact files/symbols: `crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority`, `crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory`, `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md`
- Target resolution: `reviewed-mapping-report:DS-windows-safe-fs`; matched `DirectoryAuthority`, `capture_absolute_directory`.
- Resolution rationale: Core mapping report DS-windows-safe-fs: windows.rs is still the unsupported backend and needs the complete NT contract plus cross-compile gate.
- Test ownership:
  - `crates/opentake-project/src/safe_fs/tests.rs#windows_contract` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#synchronous_nt_pending_is_invariant_error` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: 下面是 `windows.rs` 的首段；sections 3–11 的后续 code blocks 按文档顺序紧接其后，所有同名函数均只有一个定义。整份附录没有 stub、`_real` 或 `_impl` 旁路：
- Acceptance criteria: crates/opentake-project/src/safe_fs/windows.rs no longer includes unsupported.rs and provides one compile-complete capability-relative implementation without todo, unimplemented, panic, or bypass variants. Synchronous NT I/O treats STATUS_PENDING as a structured invariant failure without reading unfinished output or silently waiting. Acquisition, DACL checks, I/O, quarantine, no-replace publish, cleanup, reparse-point, source-swap, and cancellation regressions pass on native Windows. The x86_64-pc-windows-msvc build, warnings-denied clippy, repository Windows jobs, and independent review all pass for the exact tree.

### requirement-03a0d53b4d783fb7

- Kind: requirement
- Implementation slice: `implementation-slice-c7f1cd8463f97ad5`
- Candidate: `doc-5347223598d9a9f2`
- Source citation: `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md:511`
- Exact files/symbols: `crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority`, `crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory`, `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md`
- Target resolution: `reviewed-mapping-report:DS-windows-safe-fs`; matched `DirectoryAuthority`, `capture_absolute_directory`.
- Resolution rationale: Core mapping report DS-windows-safe-fs: windows.rs is still the unsupported backend and needs the complete NT contract plus cross-compile gate.
- Test ownership:
  - `crates/opentake-project/src/safe_fs/tests.rs#windows_contract` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#synchronous_nt_pending_is_invariant_error` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: 1. `returned_status == STATUS_PENDING` 在同步 handle 上属于 invariant failure；记录 raw status 并返回 `Os`，不得读取未完成 output，不另建 event，也不偷偷异步等待。
- Acceptance criteria: crates/opentake-project/src/safe_fs/windows.rs no longer includes unsupported.rs and provides one compile-complete capability-relative implementation without todo, unimplemented, panic, or bypass variants. Synchronous NT I/O treats STATUS_PENDING as a structured invariant failure without reading unfinished output or silently waiting. Acquisition, DACL checks, I/O, quarantine, no-replace publish, cleanup, reparse-point, source-swap, and cancellation regressions pass on native Windows. The x86_64-pc-windows-msvc build, warnings-denied clippy, repository Windows jobs, and independent review all pass for the exact tree.

### requirement-eb7ba5012fb1494f

- Kind: requirement
- Implementation slice: `implementation-slice-47753aa6eb656ed7`
- Candidate: `doc-705739b578c75fc8`
- Source citation: `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md:3194`
- Exact files/symbols: `crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority`, `crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory`, `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md`
- Target resolution: `reviewed-mapping-report:DS-native-receipt-validator`; matched `DirectoryAuthority`, `capture_absolute_directory`.
- Resolution rationale: Core mapping report DS-native-receipt-validator: this process/evidence slice must add the tracked Windows RED harness and receipt validator around the safe-fs boundary.
- Test ownership:
  - `scripts/tests/validate-c1b-evidence-test.rb#validates_c1b_receipt_provenance_and_rejects_forgery` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: 三份 receipt 必须属于同一 workflow run/attempt；receipt id、job id 和 artifact id 各自唯一。每份 `results.md` 必须列 exact task、固定 baseline SHA、predecessor SHA、final SHA、pre/post status、每个 local gate exit、三份 run id/attempt/job id/artifact id/name/digest/SHA、两份 gate-local audit 相对路径、aggregate。`command-ledger.json`、`results.md`、review reports、REST metadata、receipt/log/raw-exit 和 `artifact.zip` 全部必须以 gate-relative path 通过 `confined_file!`；任何 absolute/越界 path 或解析到 gate 外的 symlink 必须拒绝。review reports 先复制到 gate 内的 `reviews/`，validator 不接受外部 report path。validation 脚本必须拒绝：`gh` 缺失/未认证/API 失败、repo/run/job/artifact/workflow/head SHA/digest 不合约、predecessor proof/task-chain 不合约、任一本地自造 JSON 无法被 live API 证实、以及 SHA/receipt/command/audit/clean-status 任一不合约。
- Acceptance criteria: All three required receipts come from one live workflow run and attempt with unique receipt, job, and artifact IDs. The validator confines every ledger, result, review, metadata, log, exit, and artifact path to the gate root and rejects absolute, escaping, or external-symlink paths. Live API metadata proves repository, workflow, run, attempt, job, artifact, head SHA, digest, predecessor chain, clean status, and command/audit evidence. Negative fixtures cover missing or unauthenticated gh, API failure, locally fabricated JSON, mismatched identities, and out-of-root paths; the live exact-tree validation passes.

### requirement-e448f9531bbdb638

- Kind: requirement
- Implementation slice: `implementation-slice-c7f1cd8463f97ad5`
- Candidate: `doc-842bd9e2b189fe18`
- Source citation: `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md:3411`
- Exact files/symbols: `crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority`, `crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory`, `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md`
- Target resolution: `reviewed-mapping-report:DS-windows-safe-fs`; matched `DirectoryAuthority`, `capture_absolute_directory`.
- Resolution rationale: Core mapping report DS-windows-safe-fs: windows.rs is still the unsupported backend and needs the complete NT contract plus cross-compile gate.
- Test ownership:
  - `crates/opentake-project/src/safe_fs/tests.rs#windows_contract` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-project/src/safe_fs/tests.rs#synchronous_nt_pending_is_invariant_error` (reviewed-planned): Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
- Expected behavior: Task 6A commit `feat(project): add fail-closed Windows platform scaffold` 发生在任何 Windows behavior test 之前。它加入 Windows target dependency，并把 Task 2A 的 `include!("unsupported.rs")` 替换为下面这份完整 `windows.rs`。该 scaffold 的唯一职责是让 common facade 在 `x86_64-pc-windows-msvc` 上完整编译；所有 acquisition、I/O、DACL、quarantine、publish、cleanup 都以结构化错误拒绝。它没有 `todo!`、`unimplemented!`、panic 或缺失 symbol。
- Acceptance criteria: crates/opentake-project/src/safe_fs/windows.rs no longer includes unsupported.rs and provides one compile-complete capability-relative implementation without todo, unimplemented, panic, or bypass variants. Synchronous NT I/O treats STATUS_PENDING as a structured invariant failure without reading unfinished output or silently waiting. Acquisition, DACL checks, I/O, quarantine, no-replace publish, cleanup, reparse-point, source-swap, and cancellation regressions pass on native Windows. The x86_64-pc-windows-msvc build, warnings-denied clippy, repository Windows jobs, and independent review all pass for the exact tree.

### requirement-c6caf6aefea9247a

- Kind: requirement
- Implementation slice: `implementation-slice-b9e895cb7d3aef6b`
- Candidate: `doc-bf6666ce4bc65294`
- Source citation: `docs/upstream-analysis/01-架构与数据流.md:128`
- Exact files/symbols: `crates/opentake-domain/src/media.rs#MediaResolver`, `src-tauri/src/media.rs#relink_media`, `docs/upstream-analysis/01-架构与数据流.md`
- Target resolution: `reviewed-mapping-report:DS-media-resolver-complete`; matched `MediaResolver`, `relink_media`.
- Resolution rationale: Core mapping report evidence closure: MediaResolver and relink paths directly prove present/offline recovery semantics, pending exact ledger closure.
- Test ownership:
  - `crates/opentake-domain/src/media.rs#resolver_expected_path_external` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/media.rs#dto_reports_file_size_for_present_source` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/media.rs#relink_keeps_same_id_and_clears_missing` (existing-owned): Exact named test already exists in the reviewed owning runner and records current boundary behavior.
- Expected behavior: At docs/upstream-analysis/01-架构与数据流.md:128 under “2.5 媒体资产模型(运行时 vs 持久化分离 —— 重要设计)” (gap-marker), the source “| 解析器 | `MediaResolver`(`Models/MediaResolver.swift`) | `media_ref → URL`,区分 offline(文件缺失)/present | 全文件 |” requires this exact behavior: Resolve media_ref to an expected path while distinguishing present/offline sources.
- Acceptance criteria: Source binding: docs/upstream-analysis/01-架构与数据流.md:128; signal=gap-marker; heading=2.5 媒体资产模型(运行时 vs 持久化分离 —— 重要设计); candidate=| 解析器 | `MediaResolver`(`Models/MediaResolver.swift`) | `media_ref → URL`,区分 offline(文件缺失)/present | 全文件 | Expected behavior: Resolve media_ref to an expected path while distinguishing present/offline sources. This closes only the promise expressed by “解析器 | `MediaResolver`(`Models/MediaResolver.swift`) | `media_ref → URL`,区分 offline(文件缺失)/present | 全文件” in “2.5 媒体资产模型(运行时 vs 持久化分离 —— 重要设计)”; adjacent headings or similarly named features remain independently adjudicated. Deterministic test: exercise “解析器 | `MediaResolver`(`Models/MediaResolver.swift`) | `media_ref → URL`,区分 offline(文件缺失)/present | 全文件” with the scenario below and register test:tools/completion-tests/doc-bf6666ce4bc65294.test.mjs#completion_bf6666ce4bc65294_resolve_media_ref_to_an_expected_path_while_dist Initial state/input/event: create an isolated temporary project fixture representing the source state in “解析器 | `MediaResolver`(`Models/MediaResolver.swift`) | `media_ref → URL`,区分 offline(文件缺失)/present | 全文件”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable. Code/store/API/Rust effect: at the Rust project/core persistence boundary and its atomic filesystem effects, apply “Resolve media_ref to an expected path while distinguishing present/offline sources.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata. Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result. Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:tools/completion-tests/doc-bf6666ce4bc65294.test.mjs#completion_bf6666ce4bc65294_resolve_media_ref_to_an_expected_path_while_dist.
