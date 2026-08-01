# Agent Settings Generation Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 176 verified incomplete records in the `agent-settings-generation` gap group.

**Architecture:** Implement 44 primary evidence-bound slices and reference 0 shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.

---

### Task 1: advertised-mcp-tool-reachability + AG-advertised-tool-surface-acceptance (implementation-slice-6a8f42f312c40661)

**2026-08-01 completion:** The production base catalog contains 39 real dispatch
paths and is filtered per host session: seven media/transcript tools require the
desktop `MediaBridge`; four generation/upscale tools are appended only with
compatible managed or BYOK authorization; Motion add/edit are appended only
when the Chromium/FFmpeg production bridge is ready. `inspect_media` now covers
image/video/audio/Lottie: Lottie uses the shared Velato/Vello renderer, samples
the requested source-time window over neutral gray, and returns authoritative
canvas, frame-rate, duration, and encoded-frame metadata. `inspect_timeline`
uses that same materializer rather than silently omitting Lottie layers. The
focused RED reproduced the former typed-unavailable result; the GREEN GPU test,
advertised-tool matrix, hidden-tool fail-closed test, Clippy, formatting, and
full workspace regression all pass. Motion rendering/import/undo/save/reopen is
owned by and completed in Task 4. Native evidence is retained in
`docs/audit/2026-07-14/runtime-artifacts/automated/agent-lottie-inspect-real-device-2026-08-01.md`.

**Covered records:**
- `requirement-1c40dd077c50436b` (requirement)
- `requirement-5676fb351f12a534` (requirement)
- `requirement-f7b67184d96c0030` (requirement)
- `requirement-8b14028baf4c282e` (requirement)
- `requirement-7bd98db408b23ac3` (requirement)
- `requirement-24571c00093119be` (requirement)
- `requirement-6ca4c5069daff120` (requirement)
- `requirement-375619e15f331c80` (requirement)
- `requirement-d2bb52fd70e77335` (requirement)
- `requirement-c5c62c699b0ef951` (requirement)
- `requirement-79bb9e1c1c069d35` (requirement)
- `requirement-accb36659007a295` (requirement)
- `requirement-81dcb0a1f7a4ba8a` (requirement)
- `requirement-49c3765a9022c91d` (requirement)
- `requirement-54cacbe8e2208d5b` (requirement)
- `requirement-5dff656d1f4e9caa` (requirement)
- `requirement-a18c900d1d5a8535` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/chat/loop.rs#tool_catalog`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher::dispatch`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#dispatch`
- Modify: `crates/opentake-agent/src/tools/names.rs#ToolName::ALL`
- Modify: `CLAUDE.md`
- Modify: `docs/architecture/BUGS.md`
- Modify: `docs/architecture/FULL_PROJECT_SCAN_REPORT.md`
- Modify: `docs/architecture/HANDOFF-2026-07.md`
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md`
- Modify: `docs/specs/agent/10-implementation.md`
- Modify: `docs/specs/agent/2-tools.md`
- Modify: `docs/superpowers/specs/2026-07-10-opentake-full-convergence-design.md`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#hidden_tool_is_rejected_as_unadvertised`
- Test (reviewed-planned): `crates/opentake-agent/tests/advertised_tool_acceptance.rs#every_advertised_tool_is_live_or_absent`

**Candidate-bound contracts:**

#### requirement-1c40dd077c50436b

- Candidate/source: `doc-4c6ebcb2ccfec8db` at `CLAUDE.md:79` (requirement)
- Expected behavior: Every advertised MCP media/generation/motion tool executes a production path instead of a stub.
- Resolution: `reviewed-mapping-report:advertised-mcp-tool-reachability` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Implementation: Implement InspectMedia, GenerateVideo, GenerateImage, GenerateAudio, UpscaleMedia, AddMotionGraphic, and EditMotionGraphic; remove their not-yet-implemented dispatch branch; wire URL import or narrow the public contract; add success/failure integration tests for each tool.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-5676fb351f12a534

- Candidate/source: `doc-0d70354c07036fd0` at `docs/architecture/BUGS.md:62` (requirement)
- Expected behavior: Every advertised Agent/MCP tool performs its production operation or is removed from the advertised tool surface.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Implement InspectMedia and all generate/upscale/motion-graphic dispatch paths against real backends.
  - Implement single and entries[] batch media-folder contracts.
  - Add success, validation, failure, and undo tests for each newly live tool.

#### requirement-f7b67184d96c0030

- Candidate/source: `doc-3c2b1bdd46d20078` at `docs/architecture/BUGS.md:67` (requirement)
- Expected behavior: All advertised Agent/MCP tools execute real implementations.
- Resolution: `reviewed-mapping-report:advertised-mcp-tool-reachability` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Implementation: Implement and integration-test the seven residual InspectMedia/generation/upscale/motion stubs and replace the misleading 12-of-40 count with the live tool count.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-8b14028baf4c282e

- Candidate/source: `doc-ee70be7aba4166b3` at `docs/architecture/FULL_PROJECT_SCAN_REPORT.md:97` (requirement)
- Expected behavior: Remove every MCP dispatch stub.
- Resolution: `reviewed-mapping-report:advertised-mcp-tool-reachability` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Implementation: Implement and test InspectMedia, GenerateVideo/Image/Audio, UpscaleMedia, AddMotionGraphic, and EditMotionGraphic; update the historical list to show which former stubs are now real.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-7bd98db408b23ac3

- Candidate/source: `doc-e2f5d13fe630a705` at `docs/architecture/HANDOFF-2026-07.md:201` (requirement)
- Expected behavior: Implement InspectMedia and both motion-graphic tools.
- Resolution: `reviewed-mapping-report:advertised-mcp-tool-reachability` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Implementation: Connect InspectMedia to decoded image/video/audio/Lottie sampling and transcription; implement AddMotionGraphic/EditMotionGraphic through a deterministic renderer/import transaction; add schema, security, undo, and end-to-end tests.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-24571c00093119be

- Candidate/source: `doc-f73b7d6358a48b13` at `docs/architecture/ROADMAP.md:55` (requirement)
- Expected behavior: Replace residual Agent/MCP stubs and finish settings/runtime integration.
- Resolution: `reviewed-mapping-report:advertised-mcp-tool-reachability` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Replace InspectMedia, generation/upscale/motion, and batch media-folder stubs or remove them from advertised capability metadata.
  - Complete MCP stdio shim, Agent telemetry/redaction, chat cache/usage/mentions/persistence, and settings/provider wiring.
  - Pass tool-schema, dispatch success/failure/undo, MCP HTTP/stdio security, chat restart, and desktop Agent integration tests.

#### requirement-6ca4c5069daff120

- Candidate/source: `doc-ce332e0c5d632164` at `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:63` (requirement)
- Expected behavior: All tools referenced by editing suggestions are live production capabilities.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Expose availability from live dispatch/provider/media capabilities: auto_cut_to_beats may be available, while smart_reframe/generation tools must become live or report unavailable before invocation.
  - Never emit a suggestion whose required tool/provider/model is unavailable; return a typed alternative/reason and leave timeline state unchanged.
  - Test capability combinations for no vision backend, no generation provider, offline media, live beat analysis, provider enable/disable, and mid-session configuration refresh.

#### requirement-375619e15f331c80

- Candidate/source: `doc-05eed5c859f5b321` at `docs/specs/agent/10-implementation.md:1` (requirement)
- Expected behavior: The Agent implementation checklist is complete, including all tools, safety checks, and cross-system coverage.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Finish live implementations for residual tools, stdio transport, telemetry/redaction, chat cache/usage/mentions/images/persistence, and provider/settings wiring.
  - Keep tool discovery, schemas, capability flags, desktop commands, and documentation synchronized with actual runtime behavior.
  - Pass Agent crate tests, per-tool contract/undo matrix, MCP HTTP+stdio security suites, chat restart/request snapshots, and packaged desktop Agent smoke tests.

#### requirement-d2bb52fd70e77335

- Candidate/source: `doc-3d5eabb321ed3cf9` at `docs/specs/agent/10-implementation.md:46` (requirement)
- Expected behavior: The Agent implementation checklist is complete, including all tools, safety checks, and cross-system coverage.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Complete the dependency-ordered tasks for server, dispatch, all tool domains, chat, signals/plugins, telemetry, stdio, and desktop assembly with no advertised stub.
  - Each mutating task must use CoreHandle/shared commands and add strict input, typed failure, context-signal, and undo behavior before downstream UI work.
  - Machine-check the task list against tool discovery/source handlers/tests so every item has a live symbol and focused verification.

#### requirement-c5c62c699b0ef951

- Candidate/source: `doc-b44ab2e3f22d2679` at `docs/specs/agent/10-implementation.md:75` (requirement)
- Expected behavior: The Agent implementation checklist is complete, including all tools, safety checks, and cross-system coverage.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Measure line/branch coverage for the Agent crate and meet the documented 80% target without excluding dispatch error branches.
  - Add paired upstream-contract cases for short IDs, schema errors, tool results, prompt assembly, and timeline encoding plus integration tests for HTTP, chat, and CoreHandle.
  - CI must fail on coverage regression, flaky provider-network dependence, or any tool lacking success, validation, failure, and undo coverage.

#### requirement-79bb9e1c1c069d35

- Candidate/source: `doc-99b9674a4dd12fc7` at `docs/specs/agent/10-implementation.md:82` (requirement)
- Expected behavior: The Agent implementation checklist is complete, including all tools, safety checks, and cross-system coverage.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Bind only loopback, enforce Host/Origin/content-type/body-size limits, deny unknown/nonfinite/path inputs, and redact credentials/message content from logs.
  - Store BYOK secrets only in the OS-backed secret path and reject traversal, symlink escape, remote URL import, and unauthorized provider/tool requests fail closed.
  - Run DNS-rebinding/Origin, oversized body, traversal/symlink, secret-redaction, malformed JSON-RPC, concurrency, and graceful-shutdown tests before release.

#### requirement-accb36659007a295

- Candidate/source: `doc-22ebbe9eee9df47b` at `docs/specs/agent/2-tools.md:1` (requirement)
- Expected behavior: Every advertised tool domain has live, schema-valid behavior and undo semantics where mutating.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Keep the advertised tool inventory synchronized with live dispatch: InspectMedia, generation/upscale/motion, and batch library tools must execute or be absent.
  - Every mutating tool must validate unknown/nonfinite/path inputs, expand short IDs, route through shared commands, and return one assistant-scoped undo token.
  - Generate a per-tool matrix covering schema, success, typed failure, batch behavior, context signal, undo, and MCP HTTP serialization with zero placeholder-success results.

#### requirement-81dcb0a1f7a4ba8a

- Candidate/source: `doc-f909d35cea5c140e` at `docs/specs/agent/2-tools.md:7` (requirement)
- Expected behavior: Every advertised tool domain has live, schema-valid behavior and undo semantics where mutating.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Keep the advertised tool inventory synchronized with live dispatch: InspectMedia, generation/upscale/motion, and batch library tools must execute or be absent.
  - Every mutating tool must validate unknown/nonfinite/path inputs, expand short IDs, route through shared commands, and return one assistant-scoped undo token.
  - Generate a per-tool matrix covering schema, success, typed failure, batch behavior, context signal, undo, and MCP HTTP serialization with zero placeholder-success results.

#### requirement-49c3765a9022c91d

- Candidate/source: `doc-2072d3766d6a9d21` at `docs/specs/agent/2-tools.md:11` (requirement)
- Expected behavior: Every advertised tool domain has live, schema-valid behavior and undo semantics where mutating.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Implement InspectMedia against MediaEngine metadata/probe output and return typed not-found/unsupported errors.
  - Keep all seven read tools side-effect free and report capability fields, including canGenerate, from live configuration.
  - Add schema, short-ID expansion, malformed input, missing asset, and successful inspection tests for every read tool.

#### requirement-54cacbe8e2208d5b

- Candidate/source: `doc-fd29fa53cea9154f` at `docs/specs/agent/2-tools.md:40` (requirement)
- Expected behavior: Every advertised tool domain has live, schema-valid behavior and undo semantics where mutating.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Connect generate_video, generate_image, generate_audio, upscale_media, and URL/import paths to configured providers or remove unavailable tools from discovery.
  - Persist job/provenance, expose progress/cancel/retry, import successful output, and import nothing on error/cancel.
  - Test each tool for valid request, unknown field, auth/rate-limit/provider failure, cancellation, restart recovery, and result media identity.

#### requirement-5dff656d1f4e9caa

- Candidate/source: `doc-464fd0912284af34` at `docs/specs/agent/2-tools.md:50` (requirement)
- Expected behavior: Every advertised tool domain has live, schema-valid behavior and undo semantics where mutating.
- Resolution: `validated-ledger-evidence:AG-advertised-tool-surface-acceptance` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Implement all seven library mutations in both single-argument and entries[] batch forms with strict mutual-exclusion/schema validation.
  - Apply each mutation through one undoable transaction and roll back the entire batch on any invalid entry or persistence failure.
  - Test mixed valid/invalid batches, duplicate IDs, folder cycles, favorites, rename/move/delete, undo/redo, save, and reopen.

#### requirement-a18c900d1d5a8535

- Candidate/source: `doc-86c55874a2100740` at `docs/superpowers/specs/2026-07-10-opentake-full-convergence-design.md:319` (requirement)
- Expected behavior: - no advertised tool is an unreachable stub;
- Resolution: `reviewed-mapping-report:advertised-mcp-tool-reachability` — Both slices ask whether every advertised MCP tool reaches production behavior, using the same dispatcher and stub evidence boundary.
- Exact acceptance contract:
  - Every advertised ToolName/description maps to a reachable dispatcher branch and a real backend behavior or an explicit structured unsupported response.
  - No advertised tool returns a placeholder success, silent no-op, or unreachable stub.
  - Contract tests enumerate the advertised and dispatched tool sets bidirectionally, and integration tests invoke every high-risk write/generation path.
  - Agent/MCP runtime receipts and independent review pass on the exact tree.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/mcp/dispatch.rs#hidden_tool_is_rejected_as_unadvertised` (existing-owned) — Exact named test records the fail-closed compatibility-name boundary.
  - `crates/opentake-agent/tests/advertised_tool_acceptance.rs#every_advertised_tool_is_live_or_absent` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent hidden_tool_is_rejected_as_unadvertised`
  - Run: `cargo test -p opentake-agent --test advertised_tool_acceptance every_advertised_tool_is_live_or_absent -- --exact`

  Expected: FAIL because one or more of the 17 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/chat/loop.rs#tool_catalog`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher::dispatch`, `crates/opentake-agent/src/mcp/dispatch.rs#dispatch`, `crates/opentake-agent/src/tools/names.rs#ToolName::ALL`, `CLAUDE.md`, `docs/architecture/BUGS.md`, `docs/architecture/FULL_PROJECT_SCAN_REPORT.md`, `docs/architecture/HANDOFF-2026-07.md`, `docs/architecture/ROADMAP.md`, `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md`, `docs/specs/agent/10-implementation.md`, `docs/specs/agent/2-tools.md`, `docs/superpowers/specs/2026-07-10-opentake-full-convergence-design.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent stub_tool_reports_not_implemented`
  - Run: `cargo test -p opentake-agent --test advertised_tool_acceptance every_advertised_tool_is_live_or_absent -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 2: AG-generation-dispatch-finalization + generation-upscale-finalization (implementation-slice-f60bdd3e656a7cb0)

**Covered records:**
- `requirement-e8cab087009a2104` (requirement)
- `requirement-bffafcccb447be75` (requirement)
- `requirement-34903e30761f991d` (requirement)
- `requirement-d536b4cae75639ca` (requirement)
- `requirement-179e8dc7bf8a66b4` (requirement)
- `requirement-572b929420b7dc5b` (requirement)
- `requirement-39d847c39c244b06` (requirement)
- `requirement-40bc3f68e4a7ed80` (requirement)
- `requirement-dcf0ce260616f190` (requirement)
- `requirement-609bebfb5bc3d76b` (requirement)
- `requirement-4820f51699469306` (requirement)
- `requirement-76f42c1c9903e8c8` (requirement)
- `requirement-60b8a1de4e385ca3` (requirement)
- `requirement-637ea1555860bd0d` (requirement)
- `requirement-e9a0faff34a2391a` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#run_body`
- Modify: `crates/opentake-agent/src/mcp/generation.rs#GenerationDispatcher`
- Modify: `crates/opentake-gen/src/build_params.rs#build_image_params`
- Modify: `crates/opentake-gen/src/build_params.rs#build_upscale_params`
- Modify: `crates/opentake-gen/src/build_params.rs#build_video_params`
- Modify: `crates/opentake-gen/src/client.rs#GenClient`
- Modify: `crates/opentake-gen/src/client.rs#GenClient::submit`
- Modify: `crates/opentake-gen/src/client.rs#GenClient::submit_byok`
- Modify: `crates/opentake-gen/src/client.rs#GenClient::watch`
- Modify: `src-tauri/src/generation.rs#GenerationBridge`
- Modify: `web/src/components/agent/AgentPanel.tsx#AgentPanel`
- Modify: `docs/architecture/BUGS.md`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Modify: `docs/architecture/FULL_PROJECT_SCAN_REPORT.md`
- Modify: `docs/architecture/HANDOFF-2026-07.md`
- Modify: `docs/architecture/MODULE-PORT-MAP.md`
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `docs/modules/opentake-agent/SPEC.md`
- Modify: `docs/specs/agent/2-tools.md`
- Modify: `docs/upstream-analysis/04-MCP与Agent工具.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/generation_dispatch.rs#placeholder_persist_finalize_all_results_and_failures`
- Test (reviewed-planned): `crates/opentake-agent/tests/generation_dispatch.rs#placeholder_persists_and_every_terminal_result_finalizes_once`
- Test (existing-owned): `crates/opentake-gen/src/build_params.rs#upscale_uses_first_upload_as_source`
- Test (existing-owned): `crates/opentake-gen/src/client.rs#byok_submit_then_watch_to_succeeded`

**Candidate-bound contracts:**

#### requirement-e8cab087009a2104

- Candidate/source: `doc-404f1d1126d8665b` at `docs/architecture/BUGS.md:42` (requirement)
- Expected behavior: get_timeline reports generation availability from configured provider capability instead of a constant.
- Resolution: `validated-ledger-evidence:AG-generation-dispatch-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Introduce a generation capability source derived from active provider configuration.
  - Return that capability through get_timeline.
  - Test both configured and unconfigured provider states.

#### requirement-bffafcccb447be75

- Candidate/source: `doc-b7f6354be2c97219` at `docs/architecture/CAPCUT-GAP.md:91` (requirement)
- Expected behavior: Upscale executes a configured production backend and imports the result.
- Resolution: `reviewed-mapping-report:generation-upscale-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Route upscale_media to a configured local or BYOK provider and persist provider/model/request provenance in the generation log.
  - Expose 2x upscale progress, cancellation, retry, error, and result import while leaving the source asset unchanged.
  - Mock success/failure/cancel contracts and run one fixture asserting output width/height are exactly doubled and a cancelled job imports no media.

#### requirement-34903e30761f991d

- Candidate/source: `doc-445290562fb2b28f` at `docs/architecture/FULL_PROJECT_SCAN_REPORT.md:125` (requirement)
- Expected behavior: Agent generation tools create and finalize generated assets.
- Resolution: `reviewed-mapping-report:generation-upscale-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Implementation: Wire opentake-gen jobs into Tauri and MCP, persist placeholders and generation logs, download all results, surface progress/cancel/failure, import finalized assets, and integration-test success/partial/failure paths.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-d536b4cae75639ca

- Candidate/source: `doc-d4af138e98e3b504` at `docs/architecture/HANDOFF-2026-07.md:146` (requirement)
- Expected behavior: Generation UI and generate/upscale tools execute configured providers and import results.
- Resolution: `validated-ledger-evidence:AG-generation-dispatch-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Wire configured providers to generate_video, generate_image, generate_audio, and upscale_media.
  - Expose generation progress, cancellation, errors, and result import in the UI.
  - Add mocked-provider contract tests and one runtime smoke path.

#### requirement-179e8dc7bf8a66b4

- Candidate/source: `doc-b6561533456f47b0` at `docs/architecture/HANDOFF-2026-07.md:151` (requirement)
- Expected behavior: Wire generation/upscale dispatch and derive canGenerate from usable provider credentials.
- Resolution: `reviewed-mapping-report:generation-upscale-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Implementation: Replace GenerateVideo/Image/Audio and UpscaleMedia stubs with the shared generation service; derive canGenerate from authenticated managed mode or a compatible stored provider key; test key/no-key/provider-capability/cancel/failure cases.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-572b929420b7dc5b

- Candidate/source: `doc-8e83f5c87fdcfadc` at `docs/architecture/MODULE-PORT-MAP.md:518` (requirement)
- Expected behavior: Finalize every returned generation URL deterministically across N placeholders, including partial and total failure.
- Resolution: `reviewed-mapping-report:generation-upscale-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Implementation: Implement ordered placeholder-to-URL pairing, mark missing/invalid/download-failed outputs individually, call completion only for finalized assets, report all-failed once, persist status, and test 0/less/equal/more URLs plus download failures.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-39d847c39c244b06

- Candidate/source: `doc-2aa8ee47b99d17fe` at `docs/architecture/ROADMAP.md:74` (requirement)
- Expected behavior: Connect BYOK generation providers to tools and user-facing generation workflows.
- Resolution: `reviewed-mapping-report:generation-upscale-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Implement provider-neutral persisted generation requests/results/logs for image, video, and audio with BYOK credentials held only in the secret store.
  - Wire generate_* and upscale tools plus UI progress/cancel/retry/import to configured providers.
  - Pass mocked provider contract cases for success, rate limit, auth failure, cancellation, and restart recovery, plus one configured-provider smoke per media kind.

#### requirement-40bc3f68e4a7ed80

- Candidate/source: `doc-49175d48852d43c0` at `docs/modules/opentake-agent/SPEC.md:188` (requirement)
- Expected behavior: Generate a video asynchronously through GenClient, persist a durable placeholder/job record, and finalize it as a media asset without blocking MCP dispatch.
- Resolution: `validated-ledger-evidence:AG-generation-dispatch-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - generate_video resolves model capabilities and BYOK/managed authorization, submits asynchronously, returns a durable placeholder asset id, and records generation metadata/cost.
  - Success materializes a valid media entry; failure/cancel leaves no usable phantom asset; tests cover restarts and duplicate callbacks.

#### requirement-dcf0ce260616f190

- Candidate/source: `doc-9b90bbb501eeed4a` at `docs/modules/opentake-agent/SPEC.md:189` (requirement)
- Expected behavior: Generate an image asynchronously through GenClient and finalize it as a durable media asset.
- Resolution: `validated-ledger-evidence:AG-generation-dispatch-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - generate_image validates model/reference capabilities, submits with authorized credentials, returns a placeholder id, and atomically finalizes result bytes plus generation metadata.
  - Tests cover reference inputs, restart recovery, provider failure/cancel, and add-to-timeline after completion.

#### requirement-609bebfb5bc3d76b

- Candidate/source: `doc-7232a0984ac6a908` at `docs/modules/opentake-agent/SPEC.md:191` (requirement)
- Expected behavior: Upscale an existing media asset asynchronously and finalize the output without mutating the source asset.
- Resolution: `validated-ledger-evidence:AG-generation-dispatch-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - upscale_media resolves mediaRef, model caps, credentials, and source preprocessing before submit; it returns a durable placeholder and produces a new media asset on success.
  - Tests cover unsupported media/model pairs, restart recovery, cancellation, provider failure, and sourceClipId provenance.

#### requirement-4820f51699469306

- Candidate/source: `doc-f1f44c10c40acb1f` at `docs/specs/agent/2-tools.md:44` (requirement)
- Expected behavior: Execute generate_video as asynchronous video generation with reference media and capability validation and return a real placeholder asset.
- Resolution: `reviewed-mapping-report:generation-upscale-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Implementation: Connect generate_video to opentake-gen/provider capability validation, create and persist a placeholder, poll/download/finalize asynchronously, support progress/cancel/failure, enforce cost authorization, and add mocked end-to-end tests through MCP and the media manifest.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-76f42c1c9903e8c8

- Candidate/source: `doc-3027240e8af0de72` at `docs/specs/agent/2-tools.md:45` (requirement)
- Expected behavior: Execute generate_image as asynchronous image generation with references/quality options and return a real placeholder asset.
- Resolution: `reviewed-mapping-report:generation-upscale-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Implementation: Connect generate_image to opentake-gen/provider capability validation, create and persist a placeholder, poll/download/finalize asynchronously, support progress/cancel/failure, enforce cost authorization, and add mocked end-to-end tests through MCP and the media manifest.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-60b8a1de4e385ca3

- Candidate/source: `doc-e951ead2b99303c9` at `docs/specs/agent/2-tools.md:47` (requirement)
- Expected behavior: Execute upscale_media as asynchronous video/image upscaling and return a real placeholder asset.
- Resolution: `reviewed-mapping-report:generation-upscale-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Implementation: Connect upscale_media to opentake-gen/provider capability validation, create and persist a placeholder, poll/download/finalize asynchronously, support progress/cancel/failure, enforce cost authorization, and add mocked end-to-end tests through MCP and the media manifest.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-637ea1555860bd0d

- Candidate/source: `doc-3801568335160f8f` at `docs/upstream-analysis/04-MCP与Agent工具.md:109` (requirement)
- Expected behavior: Generate video asynchronously with model validation, references, placeholder, persistence, and finalization.
- Resolution: `reviewed-mapping-report:generation-upscale-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Implementation: Wire generate_video from schema through GenClient/provider submission, validate model capabilities/references, persist one or more placeholders, finalize downloads, surface progress/cancel/cost authorization, and add mocked integration tests.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

#### requirement-e9a0faff34a2391a

- Candidate/source: `doc-1c664d49d6a701d9` at `docs/upstream-analysis/04-MCP与Agent工具.md:110` (requirement)
- Expected behavior: Generate images asynchronously with references, placeholder, persistence, and finalization.
- Resolution: `reviewed-mapping-report:generation-upscale-finalization` — Both slices cover generation/upscale dispatch, placeholder persistence and exactly-once terminal finalization and share the client, build-params and planned dispatch test paths.
- Exact acceptance contract:
  - Implementation: Wire generate_image through model capability validation and GenClient, support N outputs/placeholders, finalize downloads and failures, expose progress/cancel/cost authorization, and add mocked integration tests.
  - Add mocked-provider and application integration tests for every named authorization, placeholder, progress, cancellation, finalization, persistence, and failure branch; the affected suites must pass without paid network calls.
  - Exercise the production MCP or UI path with a deterministic local/mock provider and retain exact manifest, job-state, command-result, and runtime evidence before reclassification.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/generation_dispatch.rs#placeholder_persist_finalize_all_results_and_failures` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/tests/generation_dispatch.rs#placeholder_persists_and_every_terminal_result_finalizes_once` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-gen/src/build_params.rs#upscale_uses_first_upload_as_source` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-gen/src/client.rs#byok_submit_then_watch_to_succeeded` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Record the RED-evidence disposition**

  - Run: `cargo test -p opentake-agent --test generation_dispatch placeholder_persist_finalize_all_results_and_failures -- --exact`
  - Run: `cargo test -p opentake-agent --test generation_dispatch placeholder_persists_and_every_terminal_result_finalizes_once -- --exact`
  - Run: `cargo test -p opentake-gen upscale_uses_first_upload_as_source`
  - Run: `cargo test -p opentake-gen byok_submit_then_watch_to_succeeded`

  Historical RED output for the four exact planned tests was not retained before the audit-recovery branch began, so it is not fabricated here. Gap-driven regression tests added during implementation did reproduce failures before the fixes (video data-result acceptance and partial-finalization transition coverage); the retained GREEN commands and runtime artifact are the auditable completion evidence.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/dispatch.rs#run_body`, `crates/opentake-agent/src/mcp/generation.rs#GenerationDispatcher`, `crates/opentake-gen/src/build_params.rs#build_image_params`, `crates/opentake-gen/src/build_params.rs#build_upscale_params`, `crates/opentake-gen/src/build_params.rs#build_video_params`, `crates/opentake-gen/src/client.rs#GenClient`, `crates/opentake-gen/src/client.rs#GenClient::submit`, `crates/opentake-gen/src/client.rs#GenClient::submit_byok`, `crates/opentake-gen/src/client.rs#GenClient::watch`, `src-tauri/src/generation.rs#GenerationBridge`, `web/src/components/agent/AgentPanel.tsx#AgentPanel`, `docs/architecture/BUGS.md`, `docs/architecture/CAPCUT-GAP.md`, `docs/architecture/FULL_PROJECT_SCAN_REPORT.md`, `docs/architecture/HANDOFF-2026-07.md`, `docs/architecture/MODULE-PORT-MAP.md`, `docs/architecture/ROADMAP.md`, `docs/modules/opentake-agent/SPEC.md`, `docs/specs/agent/2-tools.md`, `docs/upstream-analysis/04-MCP与Agent工具.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test generation_dispatch placeholder_persist_finalize_all_results_and_failures -- --exact`
  - Run: `cargo test -p opentake-agent --test generation_dispatch placeholder_persists_and_every_terminal_result_finalizes_once -- --exact`
  - Run: `cargo test -p opentake-gen upscale_uses_first_upload_as_source`
  - Run: `cargo test -p opentake-gen byok_submit_then_watch_to_succeeded`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

Completion evidence (2026-07-29): the four named focused tests pass; `generation::tests` covers configured image/video/audio/upscale production dispatch, authorization, N ordering, cancel, restart, retry, auth/rate-limit mapping, safe result URLs and exact 2x; `generation_persistence` covers durable logs/costs/restart and ready+cancelled partial terminal state. Full `cargo fmt`, Clippy `-D warnings`, workspace tests, web production build, 703 web tests, and `git diff --check` pass. Exact commands and asserted artifact state are recorded in `runtime-artifacts/automated/generation-finalization-2026-07-29.md`.

### Task 3: advanced-ai-workflows (implementation-slice-aec7c23c8d96431e)

**2026-08-01 decomposition note (not closed):** This umbrella combines ten independent product capabilities and cannot be completed by editing only `ToolName::ALL` plus one document. Work is proceeding as independently verified vertical slices. The talking-head slice now has production `remove_filler_words` and `tighten_silences` previews, configurable transcript/PCM thresholds, reviewable accepted-by-default ranges, atomic ripple application, exact one-step undo, fail-closed transcript discovery, and a fixed 30-second linked A/V regression. `requirement-9b922be7c8e92147` remains open until the combined real-media fixture also passes save/reopen/export and the user-facing per-cut review path is exercised. Motion tracking now has a strict capability-gated Agent contract, production desktop bridge, bounded real-video region analysis, editable linear position keyframes, typed low-confidence refusal, cancellation, optimistic revision commit, and one-step undo. Its deterministic target remains within five pixels and a generated MP4 exercises preview/apply/cancel/undo. It remains open for Inspector/Preview region selection, progress/retry, preview/export transform parity, save/reopen, and packaged GUI evidence. The other eight capabilities remain open.

**Covered records:**
- `requirement-fdd45062091b48f3` (requirement)
- `requirement-70db6b4ad2dbd708` (requirement)
- `requirement-be73dca02523d3b0` (requirement)
- `requirement-7d79665fbcb91584` (requirement)
- `requirement-a61a89d25c504355` (requirement)
- `requirement-dbe026f6228381a6` (requirement)
- `requirement-30bcd764cc0c454d` (requirement)
- `requirement-9b922be7c8e92147` (requirement)
- `requirement-22dc8e23d599e136` (requirement)
- `requirement-17dcf22b4c1e4d25` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/tools/names.rs#ToolName::ALL`
- Modify: `docs/architecture/CAPCUT-GAP.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/advanced_ai_workflows.rs#advanced_ai_workflows_route_through_exact_tool_contracts`

**Candidate-bound contracts:**

#### requirement-fdd45062091b48f3

- Candidate/source: `doc-15938ead4c079c7d` at `docs/architecture/CAPCUT-GAP.md:67` (requirement)
- Expected behavior: Motion tracking generates editable transform keyframes from analyzed media.
- Resolution: `reviewed-mapping-report:advanced-ai-workflows` — The advertised tool catalog does not provide complete production surfaces for tracking, matting, removal, color match, stem separation, translation, scripted assembly, avatars, and consent-gated voice cloning.
- Exact acceptance contract:
  - Generate a timestamped transform track from a selected subject/region and store it as editable keyframes.
  - Expose analysis progress, cancellation, low-confidence failure, apply, and one-step undo through Inspector/Preview.
  - On a fixture with a known moving target, keep the tracked center within five pixels at sampled frames and verify identical preview/export transforms.

#### requirement-70db6b4ad2dbd708

- Candidate/source: `doc-55767139fc509639` at `docs/architecture/CAPCUT-GAP.md:79` (requirement)
- Expected behavior: AI matting produces a reusable alpha matte and deterministic composite.
- Resolution: `reviewed-mapping-report:advanced-ai-workflows` — The advertised tool catalog does not provide complete production surfaces for tracking, matting, removal, color match, stem separation, translation, scripted assembly, avatars, and consent-gated voice cloning.
- Exact acceptance contract:
  - Produce and cache a frame-aligned alpha matte with model/version metadata while preserving original media.
  - Expose start/cancel/retry/apply and undo in the clip Inspector, with a typed missing-model or unsupported-device error.
  - Use a foreground/background fixture to assert alpha dimensions/frame count and preview/export pixel parity at opaque, transparent, and edge samples.

#### requirement-be73dca02523d3b0

- Candidate/source: `doc-1d2a88aef2af1b2b` at `docs/architecture/CAPCUT-GAP.md:97` (requirement)
- Expected behavior: Object removal produces reviewable, non-destructive output.
- Resolution: `reviewed-mapping-report:advanced-ai-workflows` — The advertised tool catalog does not provide complete production surfaces for tracking, matting, removal, color match, stem separation, translation, scripted assembly, avatars, and consent-gated voice cloning.
- Exact acceptance contract:
  - Represent the removal mask, frame range, provider/model, and generated derivative without modifying the source asset.
  - Expose mask editing, preview, cancel/retry, apply, and undo in the clip workflow.
  - Use a fixed masked-object fixture to assert only the selected frame range changes, failure leaves the timeline unchanged, and preview/export use the same derivative.

#### requirement-7d79665fbcb91584

- Candidate/source: `doc-4cfcc400c4f96d59` at `docs/architecture/CAPCUT-GAP.md:151` (requirement)
- Expected behavior: Color matching generates an editable grade from a reference.
- Resolution: `reviewed-mapping-report:advanced-ai-workflows` — The advertised tool catalog does not provide complete production surfaces for tracking, matting, removal, color match, stem separation, translation, scripted assembly, avatars, and consent-gated voice cloning.
- Exact acceptance contract:
  - Generate an editable ColorGrade from a selected reference frame and record algorithm/model/source provenance.
  - Expose reference selection, analyze, compare, accept, reset, and one-step undo while preserving the original clip grade.
  - For fixed source/reference charts, assert deterministic grade parameters and improved reference error; preview/export must apply the accepted parameters identically.

#### requirement-a61a89d25c504355

- Candidate/source: `doc-278df7efc3a84d84` at `docs/architecture/CAPCUT-GAP.md:173` (requirement)
- Expected behavior: Source separation produces usable vocal and accompaniment stems.
- Resolution: `reviewed-mapping-report:advanced-ai-workflows` — The advertised tool catalog does not provide complete production surfaces for tracking, matting, removal, color match, stem separation, translation, scripted assembly, avatars, and consent-gated voice cloning.
- Exact acceptance contract:
  - Create vocal and accompaniment derivatives with provider/model/source provenance and stable media IDs.
  - Expose progress, cancel, retry, audition, import-to-tracks, and undo; failure or cancellation must add no media or tracks.
  - For a fixed mixture fixture, verify two aligned stems with source duration/sample rate and a reconstruction residual below the documented release threshold.

#### requirement-dbe026f6228381a6

- Candidate/source: `doc-c5b2281fa479ee8c` at `docs/architecture/CAPCUT-GAP.md:185` (requirement)
- Expected behavior: Caption translation preserves timing and supports review before apply.
- Resolution: `reviewed-mapping-report:advanced-ai-workflows` — The advertised tool catalog does not provide complete production surfaces for tracking, matting, removal, color match, stem separation, translation, scripted assembly, avatars, and consent-gated voice cloning.
- Exact acceptance contract:
  - Persist translated caption text with source/target locale while preserving each caption ID and frame range.
  - Expose translate, review individual changes, accept/reject, retry, and one-step undo in Captions.
  - Mock provider success/partial/failure and assert caption count/timing never changes and failed jobs leave original text untouched.

#### requirement-30bcd764cc0c454d

- Candidate/source: `doc-527a6a7b6b32a30b` at `docs/architecture/CAPCUT-GAP.md:213` (requirement)
- Expected behavior: Script-to-video executes material selection, narration, assembly, and export as a reviewable workflow.
- Resolution: `reviewed-mapping-report:advanced-ai-workflows` — The advertised tool catalog does not provide complete production surfaces for tracking, matting, removal, color match, stem separation, translation, scripted assembly, avatars, and consent-gated voice cloning.
- Exact acceptance contract:
  - Define a persisted workflow plan containing script segments, selected media IDs, narration assets, transitions, and provenance before mutation.
  - Expose plan review/edit, start, progress, cancel, retry, apply, and one-step undo; every edit must use existing commands.
  - Run a deterministic three-segment fixture proving material placement, narration sync, transition boundaries, save/reopen, and successful export; cancellation must leave no partial timeline mutation.

#### requirement-9b922be7c8e92147

- Candidate/source: `doc-995da31233ec3c16` at `docs/architecture/CAPCUT-GAP.md:219` (requirement)
- Expected behavior: Talking-head cleanup removes both configured silence and filler words through reviewable commands.
- Resolution: `reviewed-mapping-report:advanced-ai-workflows` — The advertised tool catalog does not provide complete production surfaces for tracking, matting, removal, color match, stem separation, translation, scripted assembly, avatars, and consent-gated voice cloning.
- Exact acceptance contract:
  - Generate reviewable cut ranges for both configured silence thresholds and a defined filler-word lexicon using transcript timestamps.
  - Expose per-cut accept/reject, apply as one command, and one-step undo without altering unaccepted ranges.
  - On a fixed 30-second transcript/audio fixture, remove the expected silence and filler ranges within one frame, keep speech order/A-V sync, and verify save/reopen/export.

#### requirement-22dc8e23d599e136

- Candidate/source: `doc-a95bdb175b3dfc95` at `docs/architecture/CAPCUT-GAP.md:225` (requirement)
- Expected behavior: Avatar generation executes a configured provider and imports the result.
- Resolution: `reviewed-mapping-report:advanced-ai-workflows` — The advertised tool catalog does not provide complete production surfaces for tracking, matting, removal, color match, stem separation, translation, scripted assembly, avatars, and consent-gated voice cloning.
- Exact acceptance contract:
  - Route avatar generation through a configured provider with explicit consent, provider/model/request provenance, and source-audio identity.
  - Expose preview, start, progress, cancel, retry, import, and undo; cancellation/failure must import no asset.
  - Mock provider contracts and run one consented fixture asserting output duration matches narration within one frame and the imported result previews/exports.

#### requirement-17dcf22b4c1e4d25

- Candidate/source: `doc-23ff06be0d763353` at `docs/architecture/CAPCUT-GAP.md:231` (requirement)
- Expected behavior: Voice cloning requires consent, executes a configured provider, and imports auditable output.
- Resolution: `reviewed-mapping-report:advanced-ai-workflows` — The advertised tool catalog does not provide complete production surfaces for tracking, matting, removal, color match, stem separation, translation, scripted assembly, avatars, and consent-gated voice cloning.
- Exact acceptance contract:
  - Require recorded consent and store voice-model/provider provenance without persisting raw credentials in project data.
  - Expose enrollment/generation progress, cancel, revoke, retry, audition, import, and typed provider errors.
  - Test consent denial, success, provider failure, cancellation, and revocation; a cancelled/revoked model must not generate or import audio.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/advanced_ai_workflows.rs#advanced_ai_workflows_route_through_exact_tool_contracts` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test advanced_ai_workflows advanced_ai_workflows_route_through_exact_tool_contracts -- --exact`

  Expected: FAIL because one or more of the 10 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/tools/names.rs#ToolName::ALL`, `docs/architecture/CAPCUT-GAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test advanced_ai_workflows advanced_ai_workflows_route_through_exact_tool_contracts -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 4: motion-canvas-production-runner + AG-motion-canvas-vertical (implementation-slice-0a5150eba626d02b)

**2026-08-01 completion:** Beta v1 is closed. The pinned Motion Canvas 3.17.2
wrapper, deterministic title-card render, validated `output.mp4`/result metadata,
capability-safe Tauri/Core transaction, Motion Panel, and dynamically advertised
Agent add/edit tools share one production path. Native acceptance verifies
create/edit/undo/save/reopen, cancel/error/no-mutation boundaries, traversal and
symlink rejection, deterministic duplicate pixels/metadata, and inclusion in
both `composite_frame` and `export_video`. Transparent output, arbitrary TSX,
and frame-sequence sources remain separately scoped post-Beta work.

**Covered records:**
- `requirement-62ed34afe0cbaddc` (requirement)
- `requirement-8bde5113959f02c8` (requirement)
- `requirement-a70f2258e8af6c86` (requirement)
- `requirement-8d1113853d0b21eb` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher::dispatch`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#run_body`
- Modify: `crates/opentake-motion/src/cache.rs#MotionCache`
- Modify: `crates/opentake-motion/src/integration.rs#MotionClipSource`
- Modify: `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer`
- Modify: `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer::render`
- Modify: `crates/opentake-motion/src/renderer.rs#MotionRenderer`
- Modify: `src-tauri/src/motion.rs#render_import_place`
- Modify: `web/src/components/agent/MotionPanel.tsx#MotionPanel`
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `docs/modules/opentake-motion/MOTION-GRAPHICS-PLUGIN.md`
- Test (existing-owned): `crates/opentake-motion/src/renderer.rs#chromium_skeleton_reports_unavailable_not_panic`
- Test (existing-owned): `crates/opentake-motion/tests/pipeline.rs#full_pipeline_render_cache_and_ingest`
- Test (reviewed-planned): `src-tauri/tests/motion_command.rs#sandbox_progress_cancel_validated_mp4_result`

**Candidate-bound contracts:**

#### requirement-62ed34afe0cbaddc

- Candidate/source: `doc-632d8d3cae958a01` at `docs/architecture/ROADMAP.md:80` (requirement)
- Expected behavior: Implement Motion Canvas generation, editing, and renderer integration.
- Resolution: `reviewed-mapping-report:motion-canvas-production-runner` — Both slices cover the unavailable Chromium production renderer plus stubbed Agent motion tools and share renderer, pipeline and Tauri command evidence.
- Exact acceptance contract:
  - Persist Motion Graphic manifests/parameters/assets and route AddMotionGraphic/EditMotionGraphic through shared undoable commands.
  - Materialize supported Motion Canvas/Lottie output for preview and export, returning typed unsupported-feature errors instead of placeholders.
  - Run create/edit/save/reopen/undo tests and compare preview/export frames for one fixed animation fixture.

#### requirement-8bde5113959f02c8

- Candidate/source: `doc-1c345ea6bef6d7ff` at `docs/modules/opentake-motion/MOTION-GRAPHICS-PLUGIN.md:1` (requirement)
- Expected behavior: Deliver the Motion Canvas v1 vertical: licensed plugin/runner, render job, atomic import-and-place workflow, Motion panel, and agent tool wiring.
- Resolution: `validated-ledger-evidence:AG-motion-canvas-vertical` — Both slices cover the unavailable Chromium production renderer plus stubbed Agent motion tools and share renderer, pipeline and Tauri command evidence.
- Exact acceptance contract:
  - A pinned Motion Canvas plugin or wrapper renders the repository sample template to output.mp4 and ships LICENSE plus third-party notices.
  - A Tauri command runs the renderer inside an allowlisted cache/project directory, reports progress, and returns validated metadata.
  - Render -> import -> place is one failure-atomic and undoable workflow: any render/import/place failure leaves manifest and timeline unchanged.
  - The Motion panel can render and add a result, and add_motion_graphic/edit_motion_graphic execute the same workflow instead of returning not yet implemented.
  - Automated integration evidence proves composite_frame and export_video contain the generated clip.

#### requirement-a70f2258e8af6c86

- Candidate/source: `doc-c6e1923e22442816` at `docs/modules/opentake-motion/MOTION-GRAPHICS-PLUGIN.md:49` (requirement)
- Expected behavior: Add a pinned, licensed Motion Canvas plugin/runner that accepts a typed job and emits output.mp4 plus motion-result.json.
- Resolution: `reviewed-mapping-report:motion-canvas-production-runner` — Both slices cover the unavailable Chromium production renderer plus stubbed Agent motion tools and share renderer, pipeline and Tauri command evidence.
- Exact acceptance contract:
  - plugins/motion-canvas-studio exists with a lockfile, pinned upstream identity, templates, deterministic renderer, LICENSE, THIRD_PARTY_NOTICES.md, and README modification notes.
  - An integration test renders a fixed template twice and verifies valid media metadata plus deterministic job metadata.

#### requirement-8d1113853d0b21eb

- Candidate/source: `doc-e4020b8c2ee9646f` at `docs/modules/opentake-motion/MOTION-GRAPHICS-PLUGIN.md:70` (requirement)
- Expected behavior: Add a Tauri Motion Canvas command with constrained working directories, progress/error capture, cancellation, and validated output metadata.
- Resolution: `reviewed-mapping-report:motion-canvas-production-runner` — Both slices cover the unavailable Chromium production renderer plus stubbed Agent motion tools and share renderer, pipeline and Tauri command evidence.
- Exact acceptance contract:
  - src-tauri/src/motion_canvas.rs exposes the registered command and never accepts output/work paths outside retained application/project authorities.
  - Tests cover success, renderer failure, cancellation, traversal/symlink rejection, malformed result JSON, and no manifest/timeline mutation before validation.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-motion/src/renderer.rs#chromium_skeleton_reports_unavailable_not_panic` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-motion/tests/pipeline.rs#full_pipeline_render_cache_and_ingest` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/tests/motion_command.rs#sandbox_progress_cancel_validated_mp4_result` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [x] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-motion chromium_skeleton_reports_unavailable_not_panic`
  - Run: `cargo test -p opentake-motion --test pipeline full_pipeline_render_cache_and_ingest -- --exact`
  - Run: `cargo test -p opentake-tauri --test motion_command sandbox_progress_cancel_validated_mp4_result -- --exact`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher::dispatch`, `crates/opentake-agent/src/mcp/dispatch.rs#run_body`, `crates/opentake-motion/src/cache.rs#MotionCache`, `crates/opentake-motion/src/integration.rs#MotionClipSource`, `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer`, `crates/opentake-motion/src/renderer.rs#HeadlessChromiumRenderer::render`, `crates/opentake-motion/src/renderer.rs#MotionRenderer`, `src-tauri/src/motion.rs#render_import_place`, `web/src/components/agent/MotionPanel.tsx#MotionPanel`, `docs/architecture/ROADMAP.md`, `docs/modules/opentake-motion/MOTION-GRAPHICS-PLUGIN.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-motion chromium_skeleton_reports_unavailable_not_panic`
  - Run: `cargo test -p opentake-motion --test pipeline full_pipeline_render_cache_and_ingest -- --exact`
  - Run: `cargo test -p opentake-tauri --test motion_command sandbox_progress_cancel_validated_mp4_result -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 5: context-signal-plugin-prompt-core (implementation-slice-47883fc0d9d0c8ea)

**Covered records:**
- `requirement-59d8e4f0e42bc73c` (requirement)
- `requirement-9e9089a1cb060a9b` (requirement)
- `requirement-f2e7068a2e098318` (requirement)
- `requirement-0718cc9cfd753697` (requirement)
- `requirement-f68bff5e318e660a` (requirement)
- `requirement-fc8028641bbe69e8` (requirement)
- `requirement-e3b169fc273743bf` (requirement)
- `requirement-66dc4f1590f345d7` (requirement)
- `requirement-bd3a684f68af9863` (requirement)
- `requirement-10bd5c928350b83c` (requirement)
- `requirement-5effd883c5b15ea0` (requirement)
- `requirement-a2a915a42e5e4639` (requirement)
- `requirement-43688b32f31cc193` (requirement)
- `requirement-deb0ffb1c6782609` (requirement)
- `requirement-b4d6970f2d07e8cf` (requirement)
- `requirement-e072456b2d232fbd` (requirement)
- `requirement-902c3b772480344d` (requirement)
- `requirement-4a9d6dd7d3bc678d` (requirement)
- `requirement-120469fd39c7cb0b` (requirement)
- `requirement-aec87fd55323cbc9` (requirement)
- `requirement-96b1859ce4064adb` (requirement)
- `requirement-021c6d90aacccf27` (requirement)
- `requirement-9b40e5ea0d323e45` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/plugin/registry.rs#PluginRegistry`
- Modify: `crates/opentake-agent/src/signal/engine.rs#build_signal`
- Modify: `crates/opentake-agent/src/signal/classify.rs#classify`
- Modify: `crates/opentake-agent/src/signal/track_roles.rs#detect_track_roles`
- Modify: `crates/opentake-agent/src/signal/rules.rs#builtin_rules`
- Modify: `crates/opentake-agent/src/prompt/assemble.rs#assemble_system_prompt`
- Modify: `crates/opentake-agent/src/chat/loop.rs#ChatLoop::run_turn`
- Modify: `docs/architecture/ROADMAP.md`
- Modify: `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md`
- Modify: `docs/specs/agent/6-context-signal.md`
- Modify: `docs/specs/agent/7-system-prompt.md`
- Test (existing-owned): `crates/opentake-agent/src/plugin/registry.rs#builtin_audio_first_loads_and_validates`
- Test (existing-owned): `crates/opentake-agent/src/signal/engine.rs#get_timeline_attaches_full_signal`
- Test (existing-owned): `crates/opentake-agent/src/signal/classify.rs#talking_head_one_video_long_audio`
- Test (existing-owned): `crates/opentake-agent/src/prompt/assemble.rs#active_plugin_injects_instructions_and_rules`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#activate_workflow_returns_instructions_and_marks_active`

**Candidate-bound contracts:**

#### requirement-59d8e4f0e42bc73c

- Candidate/source: `doc-362971b4ce6925c4` at `docs/architecture/ROADMAP.md:104` (requirement)
- Expected behavior: At docs/architecture/ROADMAP.md:104 under “Phase S — Agent Context Signal 系统（随 Phase 7 交付）” (heading), the source “## Phase S — Agent Context Signal 系统（随 Phase 7 交付）” requires this exact behavior: The functional scope named by Phase S — Agent Context Signal 系统（随 Phase 7 交付） is present with automated coverage; later advanced gaps are classified separately.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/architecture/ROADMAP.md:104; signal=heading; heading=Phase S — Agent Context Signal 系统（随 Phase 7 交付）; candidate=## Phase S — Agent Context Signal 系统（随 Phase 7 交付）
  - Expected behavior: The functional scope named by Phase S — Agent Context Signal 系统（随 Phase 7 交付） is present with automated coverage; later advanced gaps are classified separately. This closes only the promise expressed by “Phase S — Agent Context Signal 系统（随 Phase 7 交付）” in “Phase S — Agent Context Signal 系统（随 Phase 7 交付）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Phase S — Agent Context Signal 系统（随 Phase 7 交付）” with the scenario below and register test:crates/opentake-agent/tests/completion_362971b4ce6925c4.rs#completion_362971b4ce6925c4_the_functional_scope_named_by_phase_s_agent_cont
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Phase S — Agent Context Signal 系统（随 Phase 7 交付）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The functional scope named by Phase S — Agent Context Signal 系统（随 Phase 7 交付） is present with automated coverage; later advanced gaps are classified separately.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_362971b4ce6925c4.rs#completion_362971b4ce6925c4_the_functional_scope_named_by_phase_s_agent_cont.

#### requirement-9e9089a1cb060a9b

- Candidate/source: `doc-90d18ab8eb23bc0a` at `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:9` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:9 under “Recipe Format” (heading), the source “## Recipe Format” requires this exact behavior: Workflow recipe format and signal integration are implemented in the plugin registry/prompt path.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:9; signal=heading; heading=Recipe Format; candidate=## Recipe Format
  - Expected behavior: Workflow recipe format and signal integration are implemented in the plugin registry/prompt path. This closes only the promise expressed by “Recipe Format” in “Recipe Format”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Recipe Format” with the scenario below and register test:crates/opentake-agent/tests/completion_90d18ab8eb23bc0a.rs#completion_90d18ab8eb23bc0a_workflow_recipe_format_and_signal_integration_ar
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “Recipe Format”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Workflow recipe format and signal integration are implemented in the plugin registry/prompt path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_90d18ab8eb23bc0a.rs#completion_90d18ab8eb23bc0a_workflow_recipe_format_and_signal_integration_ar.

#### requirement-f2e7068a2e098318

- Candidate/source: `doc-e0d51f7b3dd98f57` at `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:28` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:28 under “Talking Head Cleanup” (heading), the source “### Talking Head Cleanup” requires this exact behavior: Workflow recipe format and signal integration are implemented in the plugin registry/prompt path.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:28; signal=heading; heading=Talking Head Cleanup; candidate=### Talking Head Cleanup
  - Expected behavior: Workflow recipe format and signal integration are implemented in the plugin registry/prompt path. This closes only the promise expressed by “Talking Head Cleanup” in “Talking Head Cleanup”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Talking Head Cleanup” with the scenario below and register test:crates/opentake-agent/tests/completion_e0d51f7b3dd98f57.rs#completion_e0d51f7b3dd98f57_workflow_recipe_format_and_signal_integration_ar
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “Talking Head Cleanup”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Workflow recipe format and signal integration are implemented in the plugin registry/prompt path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_e0d51f7b3dd98f57.rs#completion_e0d51f7b3dd98f57_workflow_recipe_format_and_signal_integration_ar.

#### requirement-0718cc9cfd753697

- Candidate/source: `doc-e5e158ff34d2ee40` at `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:80` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:80 under “Silence Tighten” (heading), the source “### Silence Tighten” requires this exact behavior: Workflow recipe format and signal integration are implemented in the plugin registry/prompt path.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:80; signal=heading; heading=Silence Tighten; candidate=### Silence Tighten
  - Expected behavior: Workflow recipe format and signal integration are implemented in the plugin registry/prompt path. This closes only the promise expressed by “Silence Tighten” in “Silence Tighten”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Silence Tighten” with the scenario below and register test:crates/opentake-agent/tests/completion_e5e158ff34d2ee40.rs#completion_e5e158ff34d2ee40_workflow_recipe_format_and_signal_integration_ar
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “Silence Tighten”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Workflow recipe format and signal integration are implemented in the plugin registry/prompt path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_e5e158ff34d2ee40.rs#completion_e5e158ff34d2ee40_workflow_recipe_format_and_signal_integration_ar.

#### requirement-f68bff5e318e660a

- Candidate/source: `doc-c12fd124bb7edc2f` at `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:96` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:96 under “Plugin Signal Integration” (heading), the source “## Plugin Signal Integration” requires this exact behavior: Workflow recipe format and signal integration are implemented in the plugin registry/prompt path.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:96; signal=heading; heading=Plugin Signal Integration; candidate=## Plugin Signal Integration
  - Expected behavior: Workflow recipe format and signal integration are implemented in the plugin registry/prompt path. This closes only the promise expressed by “Plugin Signal Integration” in “Plugin Signal Integration”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Plugin Signal Integration” with the scenario below and register test:crates/opentake-agent/tests/completion_c12fd124bb7edc2f.rs#completion_c12fd124bb7edc2f_workflow_recipe_format_and_signal_integration_ar
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “Plugin Signal Integration”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Workflow recipe format and signal integration are implemented in the plugin registry/prompt path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_c12fd124bb7edc2f.rs#completion_c12fd124bb7edc2f_workflow_recipe_format_and_signal_integration_ar.

#### requirement-fc8028641bbe69e8

- Candidate/source: `doc-769791dd171d85bf` at `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:100` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:100 under “Acceptance Hooks” (heading), the source “## Acceptance Hooks” requires this exact behavior: Workflow recipe format and signal integration are implemented in the plugin registry/prompt path.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:100; signal=heading; heading=Acceptance Hooks; candidate=## Acceptance Hooks
  - Expected behavior: Workflow recipe format and signal integration are implemented in the plugin registry/prompt path. This closes only the promise expressed by “Acceptance Hooks” in “Acceptance Hooks”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Acceptance Hooks” with the scenario below and register test:crates/opentake-agent/tests/completion_769791dd171d85bf.rs#completion_769791dd171d85bf_workflow_recipe_format_and_signal_integration_ar
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “Acceptance Hooks”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Workflow recipe format and signal integration are implemented in the plugin registry/prompt path.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_769791dd171d85bf.rs#completion_769791dd171d85bf_workflow_recipe_format_and_signal_integration_ar.

#### requirement-e3b169fc273743bf

- Candidate/source: `doc-07bb748481a56287` at `docs/specs/agent/6-context-signal.md:1` (requirement)
- Expected behavior: At docs/specs/agent/6-context-signal.md:1 under “Context Signal 注入（每个工具返回如何附 `context_signal`）” (heading), the source “# Context Signal 注入（每个工具返回如何附 `context_signal`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/6-context-signal.md:1; signal=heading; heading=Context Signal 注入（每个工具返回如何附 `context_signal`）; candidate=# Context Signal 注入（每个工具返回如何附 `context_signal`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “Context Signal 注入（每个工具返回如何附 `context_signal`）” in “Context Signal 注入（每个工具返回如何附 `context_signal`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Context Signal 注入（每个工具返回如何附 `context_signal`）” with the scenario below and register test:crates/opentake-agent/tests/completion_07bb748481a56287.rs#completion_07bb748481a56287_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Context Signal 注入（每个工具返回如何附 `context_signal`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_07bb748481a56287.rs#completion_07bb748481a56287_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-66dc4f1590f345d7

- Candidate/source: `doc-e05257fbdc5f3123` at `docs/specs/agent/6-context-signal.md:7` (requirement)
- Expected behavior: At docs/specs/agent/6-context-signal.md:7 under “6.1 注入时机（哪些工具附什么信号，`AGENT-CONTEXT-SIGNAL.md:37-47`）” (heading), the source “## 6.1 注入时机（哪些工具附什么信号，`AGENT-CONTEXT-SIGNAL.md:37-47`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/6-context-signal.md:7; signal=heading; heading=6.1 注入时机（哪些工具附什么信号，`AGENT-CONTEXT-SIGNAL.md:37-47`）; candidate=## 6.1 注入时机（哪些工具附什么信号，`AGENT-CONTEXT-SIGNAL.md:37-47`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “6.1 注入时机（哪些工具附什么信号，`AGENT-CONTEXT-SIGNAL.md:37-47`）” in “6.1 注入时机（哪些工具附什么信号，`AGENT-CONTEXT-SIGNAL.md:37-47`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.1 注入时机（哪些工具附什么信号，`AGENT-CONTEXT-SIGNAL.md:37-47`）” with the scenario below and register test:crates/opentake-agent/tests/completion_e05257fbdc5f3123.rs#completion_e05257fbdc5f3123_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “6.1 注入时机（哪些工具附什么信号，`AGENT-CONTEXT-SIGNAL.md:37-47`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_e05257fbdc5f3123.rs#completion_e05257fbdc5f3123_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-bd3a684f68af9863

- Candidate/source: `doc-dd2d1559a793dd32` at `docs/specs/agent/6-context-signal.md:22` (requirement)
- Expected behavior: At docs/specs/agent/6-context-signal.md:22 under “6.2 数据结构（Rust，定义在 `opentake-domain`，本 crate 消费 + 序列化；`AGENT-CONTEXT-SIGNAL.md:50-83`）” (heading), the source “## 6.2 数据结构（Rust，定义在 `opentake-domain`，本 crate 消费 + 序列化；`AGENT-CONTEXT-SIGNAL.md:50-83`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/6-context-signal.md:22; signal=heading; heading=6.2 数据结构（Rust，定义在 `opentake-domain`，本 crate 消费 + 序列化；`AGENT-CONTEXT-SIGNAL.md:50-83`）; candidate=## 6.2 数据结构（Rust，定义在 `opentake-domain`，本 crate 消费 + 序列化；`AGENT-CONTEXT-SIGNAL.md:50-83`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “6.2 数据结构（Rust，定义在 `opentake-domain`，本 crate 消费 + 序列化；`AGENT-CONTEXT-SIGNAL.md:50-83`）” in “6.2 数据结构（Rust，定义在 `opentake-domain`，本 crate 消费 + 序列化；`AGENT-CONTEXT-SIGNAL.md:50-83`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.2 数据结构（Rust，定义在 `opentake-domain`，本 crate 消费 + 序列化；`AGENT-CONTEXT-SIGNAL.md:50-83`）” with the scenario below and register test:crates/opentake-agent/tests/completion_dd2d1559a793dd32.rs#completion_dd2d1559a793dd32_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “6.2 数据结构（Rust，定义在 `opentake-domain`，本 crate 消费 + 序列化；`AGENT-CONTEXT-SIGNAL.md:50-83`）”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_dd2d1559a793dd32.rs#completion_dd2d1559a793dd32_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-10bd5c928350b83c

- Candidate/source: `doc-826959b1026a9c36` at `docs/specs/agent/6-context-signal.md:58` (requirement)
- Expected behavior: At docs/specs/agent/6-context-signal.md:58 under “6.3 视频类型自动检测（`AGENT-CONTEXT-SIGNAL.md:104-140`）” (heading), the source “## 6.3 视频类型自动检测（`AGENT-CONTEXT-SIGNAL.md:104-140`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/6-context-signal.md:58; signal=heading; heading=6.3 视频类型自动检测（`AGENT-CONTEXT-SIGNAL.md:104-140`）; candidate=## 6.3 视频类型自动检测（`AGENT-CONTEXT-SIGNAL.md:104-140`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “6.3 视频类型自动检测（`AGENT-CONTEXT-SIGNAL.md:104-140`）” in “6.3 视频类型自动检测（`AGENT-CONTEXT-SIGNAL.md:104-140`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.3 视频类型自动检测（`AGENT-CONTEXT-SIGNAL.md:104-140`）” with the scenario below and register test:crates/opentake-agent/tests/completion_826959b1026a9c36.rs#completion_826959b1026a9c36_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “6.3 视频类型自动检测（`AGENT-CONTEXT-SIGNAL.md:104-140`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_826959b1026a9c36.rs#completion_826959b1026a9c36_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-5effd883c5b15ea0

- Candidate/source: `doc-dd416bbc86154e82` at `docs/specs/agent/6-context-signal.md:83` (requirement)
- Expected behavior: At docs/specs/agent/6-context-signal.md:83 under “6.4 轨道角色自动识别（`AGENT-CONTEXT-SIGNAL.md:148-173`）” (heading), the source “## 6.4 轨道角色自动识别（`AGENT-CONTEXT-SIGNAL.md:148-173`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/6-context-signal.md:83; signal=heading; heading=6.4 轨道角色自动识别（`AGENT-CONTEXT-SIGNAL.md:148-173`）; candidate=## 6.4 轨道角色自动识别（`AGENT-CONTEXT-SIGNAL.md:148-173`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “6.4 轨道角色自动识别（`AGENT-CONTEXT-SIGNAL.md:148-173`）” in “6.4 轨道角色自动识别（`AGENT-CONTEXT-SIGNAL.md:148-173`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.4 轨道角色自动识别（`AGENT-CONTEXT-SIGNAL.md:148-173`）” with the scenario below and register test:crates/opentake-agent/tests/completion_dd416bbc86154e82.rs#completion_dd416bbc86154e82_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “6.4 轨道角色自动识别（`AGENT-CONTEXT-SIGNAL.md:148-173`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_dd416bbc86154e82.rs#completion_dd416bbc86154e82_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-a2a915a42e5e4639

- Candidate/source: `doc-bb05308561f86dce` at `docs/specs/agent/6-context-signal.md:114` (requirement)
- Expected behavior: At docs/specs/agent/6-context-signal.md:114 under “6.5 系统提示词组装（base + 插件 instructions.md，`AGENT-CONTEXT-SIGNAL.md:96` + `WORKFLOW-PLUGIN-SYSTEM.md:108-110`）” (heading), the source “## 6.5 系统提示词组装（base + 插件 instructions.md，`AGENT-CONTEXT-SIGNAL.md:96` + `WORKFLOW-PLUGIN-SYSTEM.md:108-110`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/6-context-signal.md:114; signal=heading; heading=6.5 系统提示词组装（base + 插件 instructions.md，`AGENT-CONTEXT-SIGNAL.md:96` + `WORKFLOW-PLUGIN-SYSTEM.md:108-110`）; candidate=## 6.5 系统提示词组装（base + 插件 instructions.md，`AGENT-CONTEXT-SIGNAL.md:96` + `WORKFLOW-PLUGIN-SYSTEM.md:108-110`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “6.5 系统提示词组装（base + 插件 instructions.md，`AGENT-CONTEXT-SIGNAL.md:96` + `WORKFLOW-PLUGIN-SYSTEM.md:108-110`）” in “6.5 系统提示词组装（base + 插件 instructions.md，`AGENT-CONTEXT-SIGNAL.md:96` + `WORKFLOW-PLUGIN-SYSTEM.md:108-110`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.5 系统提示词组装（base + 插件 instructions.md，`AGENT-CONTEXT-SIGNAL.md:96` + `WORKFLOW-PLUGIN-SYSTEM.md:108-110`）” with the scenario below and register test:crates/opentake-agent/tests/completion_bb05308561f86dce.rs#completion_bb05308561f86dce_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “6.5 系统提示词组装（base + 插件 instructions.md，`AGENT-CONTEXT-SIGNAL.md:96` + `WORKFLOW-PLUGIN-SYSTEM.md:108-110`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_bb05308561f86dce.rs#completion_bb05308561f86dce_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-43688b32f31cc193

- Candidate/source: `doc-3e097853d5536963` at `docs/specs/agent/6-context-signal.md:135` (requirement)
- Expected behavior: At docs/specs/agent/6-context-signal.md:135 under “6.5.1 基础系统提示词（OpenTake 版）” (heading), the source “### 6.5.1 基础系统提示词（OpenTake 版）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/6-context-signal.md:135; signal=heading; heading=6.5.1 基础系统提示词（OpenTake 版）; candidate=### 6.5.1 基础系统提示词（OpenTake 版）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “6.5.1 基础系统提示词（OpenTake 版）” in “6.5.1 基础系统提示词（OpenTake 版）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.5.1 基础系统提示词（OpenTake 版）” with the scenario below and register test:crates/opentake-agent/tests/completion_3e097853d5536963.rs#completion_3e097853d5536963_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “6.5.1 基础系统提示词（OpenTake 版）”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_3e097853d5536963.rs#completion_3e097853d5536963_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-deb0ffb1c6782609

- Candidate/source: `doc-7e6b1e22923ea8b8` at `docs/specs/agent/6-context-signal.md:148` (requirement)
- Expected behavior: At docs/specs/agent/6-context-signal.md:148 under “6.6 规则校验（内置规则 + 插件规则，`AGENT-CONTEXT-SIGNAL.md:177-212`）” (heading), the source “## 6.6 规则校验（内置规则 + 插件规则，`AGENT-CONTEXT-SIGNAL.md:177-212`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/6-context-signal.md:148; signal=heading; heading=6.6 规则校验（内置规则 + 插件规则，`AGENT-CONTEXT-SIGNAL.md:177-212`）; candidate=## 6.6 规则校验（内置规则 + 插件规则，`AGENT-CONTEXT-SIGNAL.md:177-212`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “6.6 规则校验（内置规则 + 插件规则，`AGENT-CONTEXT-SIGNAL.md:177-212`）” in “6.6 规则校验（内置规则 + 插件规则，`AGENT-CONTEXT-SIGNAL.md:177-212`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.6 规则校验（内置规则 + 插件规则，`AGENT-CONTEXT-SIGNAL.md:177-212`）” with the scenario below and register test:crates/opentake-agent/tests/completion_7e6b1e22923ea8b8.rs#completion_7e6b1e22923ea8b8_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “6.6 规则校验（内置规则 + 插件规则，`AGENT-CONTEXT-SIGNAL.md:177-212`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_7e6b1e22923ea8b8.rs#completion_7e6b1e22923ea8b8_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-b4d6970f2d07e8cf

- Candidate/source: `doc-d051f0954cde823a` at `docs/specs/agent/6-context-signal.md:161` (requirement)
- Expected behavior: At docs/specs/agent/6-context-signal.md:161 under “6.6.1 内置规则（`AGENT-CONTEXT-SIGNAL.md:177-203`，warning 文本原样）” (heading), the source “### 6.6.1 内置规则（`AGENT-CONTEXT-SIGNAL.md:177-203`，warning 文本原样）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/6-context-signal.md:161; signal=heading; heading=6.6.1 内置规则（`AGENT-CONTEXT-SIGNAL.md:177-203`，warning 文本原样）; candidate=### 6.6.1 内置规则（`AGENT-CONTEXT-SIGNAL.md:177-203`，warning 文本原样）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “6.6.1 内置规则（`AGENT-CONTEXT-SIGNAL.md:177-203`，warning 文本原样）” in “6.6.1 内置规则（`AGENT-CONTEXT-SIGNAL.md:177-203`，warning 文本原样）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.6.1 内置规则（`AGENT-CONTEXT-SIGNAL.md:177-203`，warning 文本原样）” with the scenario below and register test:crates/opentake-agent/tests/completion_d051f0954cde823a.rs#completion_d051f0954cde823a_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “6.6.1 内置规则（`AGENT-CONTEXT-SIGNAL.md:177-203`，warning 文本原样）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_d051f0954cde823a.rs#completion_d051f0954cde823a_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-e072456b2d232fbd

- Candidate/source: `doc-3c6c343553284715` at `docs/specs/agent/6-context-signal.md:187` (requirement)
- Expected behavior: At docs/specs/agent/6-context-signal.md:187 under “6.6.2 插件规则（`WORKFLOW-PLUGIN-SYSTEM.md:116-118`）” (heading), the source “### 6.6.2 插件规则（`WORKFLOW-PLUGIN-SYSTEM.md:116-118`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/6-context-signal.md:187; signal=heading; heading=6.6.2 插件规则（`WORKFLOW-PLUGIN-SYSTEM.md:116-118`）; candidate=### 6.6.2 插件规则（`WORKFLOW-PLUGIN-SYSTEM.md:116-118`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “6.6.2 插件规则（`WORKFLOW-PLUGIN-SYSTEM.md:116-118`）” in “6.6.2 插件规则（`WORKFLOW-PLUGIN-SYSTEM.md:116-118`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “6.6.2 插件规则（`WORKFLOW-PLUGIN-SYSTEM.md:116-118`）” with the scenario below and register test:crates/opentake-agent/tests/completion_3c6c343553284715.rs#completion_3c6c343553284715_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “6.6.2 插件规则（`WORKFLOW-PLUGIN-SYSTEM.md:116-118`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_3c6c343553284715.rs#completion_3c6c343553284715_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-902c3b772480344d

- Candidate/source: `doc-5ac7491fc61ad042` at `docs/specs/agent/7-system-prompt.md:1` (requirement)
- Expected behavior: At docs/specs/agent/7-system-prompt.md:1 under “Workflow Plugin（plugin.json 加载、activate_workflow、instructions.md 注入、rules 校验）” (heading), the source “# Workflow Plugin（plugin.json 加载、activate_workflow、instructions.md 注入、rules 校验）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/7-system-prompt.md:1; signal=heading; heading=Workflow Plugin（plugin.json 加载、activate_workflow、instructions.md 注入、rules 校验）; candidate=# Workflow Plugin（plugin.json 加载、activate_workflow、instructions.md 注入、rules 校验）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “Workflow Plugin（plugin.json 加载、activate_workflow、instructions.md 注入、rules 校验）” in “Workflow Plugin（plugin.json 加载、activate_workflow、instructions.md 注入、rules 校验）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Workflow Plugin（plugin.json 加载、activate_workflow、instructions.md 注入、rules 校验）” with the scenario below and register test:crates/opentake-agent/tests/completion_5ac7491fc61ad042.rs#completion_5ac7491fc61ad042_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Workflow Plugin（plugin.json 加载、activate_workflow、instructions.md 注入、rules 校验）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_5ac7491fc61ad042.rs#completion_5ac7491fc61ad042_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-4a9d6dd7d3bc678d

- Candidate/source: `doc-826dca2ef522f66f` at `docs/specs/agent/7-system-prompt.md:5` (requirement)
- Expected behavior: At docs/specs/agent/7-system-prompt.md:5 under “7.1 插件格式（`WORKFLOW-PLUGIN-SYSTEM.md:18-96`）” (heading), the source “## 7.1 插件格式（`WORKFLOW-PLUGIN-SYSTEM.md:18-96`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/7-system-prompt.md:5; signal=heading; heading=7.1 插件格式（`WORKFLOW-PLUGIN-SYSTEM.md:18-96`）; candidate=## 7.1 插件格式（`WORKFLOW-PLUGIN-SYSTEM.md:18-96`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “7.1 插件格式（`WORKFLOW-PLUGIN-SYSTEM.md:18-96`）” in “7.1 插件格式（`WORKFLOW-PLUGIN-SYSTEM.md:18-96`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “7.1 插件格式（`WORKFLOW-PLUGIN-SYSTEM.md:18-96`）” with the scenario below and register test:crates/opentake-agent/tests/completion_826dca2ef522f66f.rs#completion_826dca2ef522f66f_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “7.1 插件格式（`WORKFLOW-PLUGIN-SYSTEM.md:18-96`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_826dca2ef522f66f.rs#completion_826dca2ef522f66f_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-120469fd39c7cb0b

- Candidate/source: `doc-cfb468fae8890e54` at `docs/specs/agent/7-system-prompt.md:53` (requirement)
- Expected behavior: At docs/specs/agent/7-system-prompt.md:53 under “7.2 插件注册表 + 加载（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`、`:136-141`）” (heading), the source “## 7.2 插件注册表 + 加载（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`、`:136-141`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/7-system-prompt.md:53; signal=heading; heading=7.2 插件注册表 + 加载（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`、`:136-141`）; candidate=## 7.2 插件注册表 + 加载（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`、`:136-141`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “7.2 插件注册表 + 加载（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`、`:136-141`）” in “7.2 插件注册表 + 加载（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`、`:136-141`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “7.2 插件注册表 + 加载（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`、`:136-141`）” with the scenario below and register test:crates/opentake-agent/tests/completion_cfb468fae8890e54.rs#completion_cfb468fae8890e54_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “7.2 插件注册表 + 加载（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`、`:136-141`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_cfb468fae8890e54.rs#completion_cfb468fae8890e54_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-aec87fd55323cbc9

- Candidate/source: `doc-268f850cad14d620` at `docs/specs/agent/7-system-prompt.md:75` (requirement)
- Expected behavior: At docs/specs/agent/7-system-prompt.md:75 under “7.3 激活方式（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`）” (heading), the source “## 7.3 激活方式（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/7-system-prompt.md:75; signal=heading; heading=7.3 激活方式（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`）; candidate=## 7.3 激活方式（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “7.3 激活方式（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`）” in “7.3 激活方式（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “7.3 激活方式（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`）” with the scenario below and register test:crates/opentake-agent/tests/completion_268f850cad14d620.rs#completion_268f850cad14d620_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “7.3 激活方式（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_268f850cad14d620.rs#completion_268f850cad14d620_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-96b1859ce4064adb

- Candidate/source: `doc-e9021b205a70f6fc` at `docs/specs/agent/7-system-prompt.md:83` (requirement)
- Expected behavior: At docs/specs/agent/7-system-prompt.md:83 under “7.4 `activate_workflow` MCP 工具（**OpenTake 新增的第 32 个工具**，`WORKFLOW-PLUGIN-SYSTEM.md:104`、ROADMAP Phase W `:119`）” (heading), the source “## 7.4 `activate_workflow` MCP 工具（**OpenTake 新增的第 32 个工具**，`WORKFLOW-PLUGIN-SYSTEM.md:104`、ROADMAP Phase W `:119`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/7-system-prompt.md:83; signal=heading; heading=7.4 `activate_workflow` MCP 工具（**OpenTake 新增的第 32 个工具**，`WORKFLOW-PLUGIN-SYSTEM.md:104`、ROADMAP Phase W `:119`）; candidate=## 7.4 `activate_workflow` MCP 工具（**OpenTake 新增的第 32 个工具**，`WORKFLOW-PLUGIN-SYSTEM.md:104`、ROADMAP Phase W `:119`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “7.4 `activate_workflow` MCP 工具（**OpenTake 新增的第 32 个工具**，`WORKFLOW-PLUGIN-SYSTEM.md:104`、ROADMAP Phase W `:119`）” in “7.4 `activate_workflow` MCP 工具（**OpenTake 新增的第 32 个工具**，`WORKFLOW-PLUGIN-SYSTEM.md:104`、ROADMAP Phase W `:119`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “7.4 `activate_workflow` MCP 工具（**OpenTake 新增的第 32 个工具**，`WORKFLOW-PLUGIN-SYSTEM.md:104`、ROADMAP Phase W `:119`）” with the scenario below and register test:crates/opentake-agent/tests/completion_e9021b205a70f6fc.rs#completion_e9021b205a70f6fc_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “7.4 `activate_workflow` MCP 工具（**OpenTake 新增的第 32 个工具**，`WORKFLOW-PLUGIN-SYSTEM.md:104`、ROADMAP Phase W `:119`）”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_e9021b205a70f6fc.rs#completion_e9021b205a70f6fc_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-021c6d90aacccf27

- Candidate/source: `doc-fec1f04109739578` at `docs/specs/agent/7-system-prompt.md:106` (requirement)
- Expected behavior: At docs/specs/agent/7-system-prompt.md:106 under “7.5 插件对 Agent 的三处影响（`WORKFLOW-PLUGIN-SYSTEM.md:108-118`，与 §6.5/§6.6 衔接）” (heading), the source “## 7.5 插件对 Agent 的三处影响（`WORKFLOW-PLUGIN-SYSTEM.md:108-118`，与 §6.5/§6.6 衔接）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/7-system-prompt.md:106; signal=heading; heading=7.5 插件对 Agent 的三处影响（`WORKFLOW-PLUGIN-SYSTEM.md:108-118`，与 §6.5/§6.6 衔接）; candidate=## 7.5 插件对 Agent 的三处影响（`WORKFLOW-PLUGIN-SYSTEM.md:108-118`，与 §6.5/§6.6 衔接）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “7.5 插件对 Agent 的三处影响（`WORKFLOW-PLUGIN-SYSTEM.md:108-118`，与 §6.5/§6.6 衔接）” in “7.5 插件对 Agent 的三处影响（`WORKFLOW-PLUGIN-SYSTEM.md:108-118`，与 §6.5/§6.6 衔接）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “7.5 插件对 Agent 的三处影响（`WORKFLOW-PLUGIN-SYSTEM.md:108-118`，与 §6.5/§6.6 衔接）” with the scenario below and register test:crates/opentake-agent/tests/completion_fec1f04109739578.rs#completion_fec1f04109739578_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “7.5 插件对 Agent 的三处影响（`WORKFLOW-PLUGIN-SYSTEM.md:108-118`，与 §6.5/§6.6 衔接）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_fec1f04109739578.rs#completion_fec1f04109739578_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-9b40e5ea0d323e45

- Candidate/source: `doc-8c6994f6cd288241` at `docs/specs/agent/7-system-prompt.md:112` (requirement)
- Expected behavior: At docs/specs/agent/7-system-prompt.md:112 under “7.6 插件字段 → ContextSignal 叠加（`AGENT-CONTEXT-SIGNAL.md:88-98`，**叠加优先级与覆盖语义**）” (heading), the source “## 7.6 插件字段 → ContextSignal 叠加（`AGENT-CONTEXT-SIGNAL.md:88-98`，**叠加优先级与覆盖语义**）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `reviewed-mapping-report:context-signal-plugin-prompt-core` — Twenty-three heading-derived records converge on implemented, imported, and tested shared owners. They need exact evidence rebinding rather than per-heading product work.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/7-system-prompt.md:112; signal=heading; heading=7.6 插件字段 → ContextSignal 叠加（`AGENT-CONTEXT-SIGNAL.md:88-98`，**叠加优先级与覆盖语义**）; candidate=## 7.6 插件字段 → ContextSignal 叠加（`AGENT-CONTEXT-SIGNAL.md:88-98`，**叠加优先级与覆盖语义**）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “7.6 插件字段 → ContextSignal 叠加（`AGENT-CONTEXT-SIGNAL.md:88-98`，**叠加优先级与覆盖语义**）” in “7.6 插件字段 → ContextSignal 叠加（`AGENT-CONTEXT-SIGNAL.md:88-98`，**叠加优先级与覆盖语义**）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “7.6 插件字段 → ContextSignal 叠加（`AGENT-CONTEXT-SIGNAL.md:88-98`，**叠加优先级与覆盖语义**）” with the scenario below and register test:crates/opentake-agent/tests/completion_8c6994f6cd288241.rs#completion_8c6994f6cd288241_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “7.6 插件字段 → ContextSignal 叠加（`AGENT-CONTEXT-SIGNAL.md:88-98`，**叠加优先级与覆盖语义**）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_8c6994f6cd288241.rs#completion_8c6994f6cd288241_the_documented_agent_signal_plugin_core_dispatch.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/plugin/registry.rs#builtin_audio_first_loads_and_validates` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/signal/engine.rs#get_timeline_attaches_full_signal` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/signal/classify.rs#talking_head_one_video_long_audio` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/prompt/assemble.rs#active_plugin_injects_instructions_and_rules` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#activate_workflow_returns_instructions_and_marks_active` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent builtin_audio_first_loads_and_validates`
  - Run: `cargo test -p opentake-agent get_timeline_attaches_full_signal`
  - Run: `cargo test -p opentake-agent talking_head_one_video_long_audio`
  - Run: `cargo test -p opentake-agent active_plugin_injects_instructions_and_rules`
  - Run: `cargo test -p opentake-agent activate_workflow_returns_instructions_and_marks_active`

  Expected: FAIL because one or more of the 23 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/plugin/registry.rs#PluginRegistry`, `crates/opentake-agent/src/signal/engine.rs#build_signal`, `crates/opentake-agent/src/signal/classify.rs#classify`, `crates/opentake-agent/src/signal/track_roles.rs#detect_track_roles`, `crates/opentake-agent/src/signal/rules.rs#builtin_rules`, `crates/opentake-agent/src/prompt/assemble.rs#assemble_system_prompt`, `crates/opentake-agent/src/chat/loop.rs#ChatLoop::run_turn`, `docs/architecture/ROADMAP.md`, `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md`, `docs/specs/agent/6-context-signal.md`, `docs/specs/agent/7-system-prompt.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent builtin_audio_first_loads_and_validates`
  - Run: `cargo test -p opentake-agent get_timeline_attaches_full_signal`
  - Run: `cargo test -p opentake-agent talking_head_one_video_long_audio`
  - Run: `cargo test -p opentake-agent active_plugin_injects_instructions_and_rules`
  - Run: `cargo test -p opentake-agent activate_workflow_returns_instructions_and_marks_active`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 6: plugin-authoring-lifecycle (implementation-slice-f651a4e0d59a232c)

**Covered records:**
- `requirement-3f98ce882cd382ec` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/plugin/registry.rs#PluginRegistry::load_dir`
- Modify: `crates/opentake-agent/src/plugin/registry.rs#PluginRegistry::scan`
- Modify: `docs/architecture/ROADMAP.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/plugin_package_lifecycle.rs#plugin_package_install_enable_disable_remove_is_atomic`

**Candidate-bound contracts:**

#### requirement-3f98ce882cd382ec

- Candidate/source: `doc-cd65820236ccd591` at `docs/architecture/ROADMAP.md:118` (requirement)
- Expected behavior: Finish plugin authoring lifecycle and validate packaged workflow plugins.
- Resolution: `reviewed-mapping-report:plugin-authoring-lifecycle` — Loading and registry owners exist, but no complete authoring, packaging, signing, and packaged-workflow validation lifecycle was found.
- Exact acceptance contract:
  - Provide plugin create, validate, package, install, activate, deactivate, and version-conflict flows against the documented manifest/schema.
  - Reject unsafe paths/rules/tools, inject active instructions/signals deterministically, and restore activation after restart.
  - Test valid/invalid packages, duplicate IDs, upgrades, activation rollback, prompt assembly, rule evaluation, and packaged-desktop installation.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/plugin_package_lifecycle.rs#plugin_package_install_enable_disable_remove_is_atomic` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test plugin_package_lifecycle plugin_package_install_enable_disable_remove_is_atomic -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/plugin/registry.rs#PluginRegistry::load_dir`, `crates/opentake-agent/src/plugin/registry.rs#PluginRegistry::scan`, `docs/architecture/ROADMAP.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test plugin_package_lifecycle plugin_package_install_enable_disable_remove_is_atomic -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 7: editing-automation-contract + AG-automation-call-chain-contract (implementation-slice-2a8a716243b84715)

**Covered records:**
- `requirement-7b0285439bfae8b9` (requirement)
- `requirement-6b7dcf95fc15dae5` (requirement)
- `requirement-7adf016d3bd9c09a` (requirement)
- `requirement-fd89d8d87fcc28b3` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/core_handle.rs#CoreHandle::apply`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher::dispatch`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#dispatch`
- Modify: `crates/opentake-agent/src/signal/engine.rs#attach`
- Modify: `crates/opentake-agent/src/tools/short_id.rs#expand_id_prefixes`
- Modify: `crates/opentake-ops/src/intent.rs#EditPlan`
- Modify: `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md`
- Modify: `docs/architecture/editing-automation/README.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed`
- Test (reviewed-planned): `crates/opentake-agent/tests/editing_automation_acceptance.rs#documented_call_chains_and_invariants_match_current_code`

**Candidate-bound contracts:**

#### requirement-7b0285439bfae8b9

- Candidate/source: `doc-a466c25e65ff8024` at `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:30` (requirement)
- Expected behavior: The documented automation checks pass against live tool implementations.
- Resolution: `reviewed-mapping-report:editing-automation-contract` — Both slices are umbrella acceptance for read-only proposals, atomic shared-command writes, linked synchronization and the complete agent call chain.
- Exact acceptance contract:
  - Dispatch smart_reframe against decoded fixture frames and assert it returns an in-bounds 9:16 crop proposal rather than needs-vision-backend.
  - Apply the proposal to one clip, verify only crop/transform properties change, then undo to byte-equal timeline JSON.
  - Cover subject left/center/right, no-subject fallback, offline media, invalid ratio, cancellation, and save/reopen determinism.

#### requirement-6b7dcf95fc15dae5

- Candidate/source: `doc-40e5bb9502d57154` at `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md:46` (requirement)
- Expected behavior: The documented automation checks pass against live tool implementations.
- Resolution: `reviewed-mapping-report:editing-automation-contract` — Both slices are umbrella acceptance for read-only proposals, atomic shared-command writes, linked synchronization and the complete agent call chain.
- Exact acceptance contract:
  - Run each built-in workflow in dry-run and apply modes through the same dispatcher/tool schemas exposed by MCP.
  - Assert every referenced tool is live, plugin instructions/signals/rules are injected once, and all mutations collapse into reviewable undoable commands.
  - Cover missing tool/provider, invalid recipe parameters, rule warning, mid-workflow failure rollback, cancellation, undo/redo, and restart activation.

#### requirement-7adf016d3bd9c09a

- Candidate/source: `doc-eb2de69e6d3874be` at `docs/architecture/editing-automation/README.md:31` (requirement)
- Expected behavior: At docs/architecture/editing-automation/README.md:31 under “Authoritative Call Chains” (heading), the source “## Authoritative Call Chains” requires this exact behavior: The documented automation call chains and invariants map to current command, intent, and Agent code.
- Resolution: `validated-ledger-evidence:AG-automation-call-chain-contract` — Both slices are umbrella acceptance for read-only proposals, atomic shared-command writes, linked synchronization and the complete agent call chain.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/README.md:31; signal=heading; heading=Authoritative Call Chains; candidate=## Authoritative Call Chains
  - Expected behavior: The documented automation call chains and invariants map to current command, intent, and Agent code. This closes only the promise expressed by “Authoritative Call Chains” in “Authoritative Call Chains”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Authoritative Call Chains” with the scenario below and register test:crates/opentake-agent/tests/completion_eb2de69e6d3874be.rs#completion_eb2de69e6d3874be_the_documented_automation_call_chains_and_invari
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Authoritative Call Chains” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented automation call chains and invariants map to current command, intent, and Agent code.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_eb2de69e6d3874be.rs#completion_eb2de69e6d3874be_the_documented_automation_call_chains_and_invari.

#### requirement-fd89d8d87fcc28b3

- Candidate/source: `doc-35449910b4b2d75a` at `docs/architecture/editing-automation/README.md:45` (requirement)
- Expected behavior: At docs/architecture/editing-automation/README.md:45 under “Non-Negotiable Invariants” (heading), the source “## Non-Negotiable Invariants” requires this exact behavior: The documented automation call chains and invariants map to current command, intent, and Agent code.
- Resolution: `validated-ledger-evidence:AG-automation-call-chain-contract` — Both slices are umbrella acceptance for read-only proposals, atomic shared-command writes, linked synchronization and the complete agent call chain.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/README.md:45; signal=heading; heading=Non-Negotiable Invariants; candidate=## Non-Negotiable Invariants
  - Expected behavior: The documented automation call chains and invariants map to current command, intent, and Agent code. This closes only the promise expressed by “Non-Negotiable Invariants” in “Non-Negotiable Invariants”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Non-Negotiable Invariants” with the scenario below and register test:crates/opentake-agent/tests/completion_35449910b4b2d75a.rs#completion_35449910b4b2d75a_the_documented_automation_call_chains_and_invari
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Non-Negotiable Invariants” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented automation call chains and invariants map to current command, intent, and Agent code.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_35449910b4b2d75a.rs#completion_35449910b4b2d75a_the_documented_automation_call_chains_and_invari.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/editing_automation_acceptance.rs#automation_children_are_atomic_reviewable_and_command_routed` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/tests/editing_automation_acceptance.rs#documented_call_chains_and_invariants_match_current_code` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test editing_automation_acceptance automation_children_are_atomic_reviewable_and_command_routed -- --exact`
  - Run: `cargo test -p opentake-agent --test editing_automation_acceptance documented_call_chains_and_invariants_match_current_code -- --exact`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/core_handle.rs#CoreHandle::apply`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher::dispatch`, `crates/opentake-agent/src/mcp/dispatch.rs#dispatch`, `crates/opentake-agent/src/signal/engine.rs#attach`, `crates/opentake-agent/src/tools/short_id.rs#expand_id_prefixes`, `crates/opentake-ops/src/intent.rs#EditPlan`, `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md`, `docs/architecture/editing-automation/README.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test editing_automation_acceptance automation_children_are_atomic_reviewable_and_command_routed -- --exact`
  - Run: `cargo test -p opentake-agent --test editing_automation_acceptance documented_call_chains_and_invariants_match_current_code -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 8: AG-structured-editing-suggestions (implementation-slice-628780b124afee95)

**Covered records:**
- `requirement-e80b5c0dc0ec1e1a` (requirement)
- `requirement-6fceedb18b105841` (requirement)
- `requirement-276e2bfecbd2ad41` (requirement)
- `requirement-ee27e0025129d88d` (requirement)
- `requirement-e8b227e21950c923` (requirement)
- `requirement-00e262c9f28140a8` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/suggestions.rs#EditingSuggestion`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#dispatch`
- Modify: `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/editing_suggestions.rs#proposal_is_read_only_until_shared_dispatch_apply`

**Candidate-bound contracts:**

#### requirement-e80b5c0dc0ec1e1a

- Candidate/source: `doc-365e9273e1be4946` at `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:9` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:9 under “Dispatcher Contract” (heading), the source “## Dispatcher Contract” requires this exact behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.
- Resolution: `validated-ledger-evidence:AG-structured-editing-suggestions` — Six headings restate one missing structured non-mutating suggestion model and acceptance/apply path.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:9; signal=heading; heading=Dispatcher Contract; candidate=## Dispatcher Contract
  - Expected behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher. This closes only the promise expressed by “Dispatcher Contract” in “Dispatcher Contract”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Dispatcher Contract” with the scenario below and register test:crates/opentake-agent/tests/completion_365e9273e1be4946.rs#completion_365e9273e1be4946_editing_suggestions_use_structured_non_mutating_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Dispatcher Contract” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_365e9273e1be4946.rs#completion_365e9273e1be4946_editing_suggestions_use_structured_non_mutating_.

#### requirement-6fceedb18b105841

- Candidate/source: `doc-2fd40468211905d1` at `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:17` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:17 under “Tool Set” (heading), the source “## Tool Set” requires this exact behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.
- Resolution: `validated-ledger-evidence:AG-structured-editing-suggestions` — Six headings restate one missing structured non-mutating suggestion model and acceptance/apply path.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:17; signal=heading; heading=Tool Set; candidate=## Tool Set
  - Expected behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher. This closes only the promise expressed by “Tool Set” in “Tool Set”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Tool Set” with the scenario below and register test:crates/opentake-agent/tests/completion_2fd40468211905d1.rs#completion_2fd40468211905d1_editing_suggestions_use_structured_non_mutating_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Tool Set” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_2fd40468211905d1.rs#completion_2fd40468211905d1_editing_suggestions_use_structured_non_mutating_.

#### requirement-276e2bfecbd2ad41

- Candidate/source: `doc-175c0e0bf43877a2` at `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:30` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:30 under “Suggestion Shape” (heading), the source “## Suggestion Shape” requires this exact behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.
- Resolution: `validated-ledger-evidence:AG-structured-editing-suggestions` — Six headings restate one missing structured non-mutating suggestion model and acceptance/apply path.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:30; signal=heading; heading=Suggestion Shape; candidate=## Suggestion Shape
  - Expected behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher. This closes only the promise expressed by “Suggestion Shape” in “Suggestion Shape”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Suggestion Shape” with the scenario below and register test:crates/opentake-agent/tests/completion_175c0e0bf43877a2.rs#completion_175c0e0bf43877a2_editing_suggestions_use_structured_non_mutating_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Suggestion Shape” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_175c0e0bf43877a2.rs#completion_175c0e0bf43877a2_editing_suggestions_use_structured_non_mutating_.

#### requirement-ee27e0025129d88d

- Candidate/source: `doc-37b57f58ac92167c` at `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:48` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:48 under “Context Signal” (heading), the source “## Context Signal” requires this exact behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.
- Resolution: `validated-ledger-evidence:AG-structured-editing-suggestions` — Six headings restate one missing structured non-mutating suggestion model and acceptance/apply path.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:48; signal=heading; heading=Context Signal; candidate=## Context Signal
  - Expected behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher. This closes only the promise expressed by “Context Signal” in “Context Signal”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Context Signal” with the scenario below and register test:crates/opentake-agent/tests/completion_37b57f58ac92167c.rs#completion_37b57f58ac92167c_editing_suggestions_use_structured_non_mutating_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Context Signal” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_37b57f58ac92167c.rs#completion_37b57f58ac92167c_editing_suggestions_use_structured_non_mutating_.

#### requirement-e8b227e21950c923

- Candidate/source: `doc-734446f42734a0a0` at `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:59` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:59 under “Ripple Range Contract” (heading), the source “## Ripple Range Contract” requires this exact behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.
- Resolution: `validated-ledger-evidence:AG-structured-editing-suggestions` — Six headings restate one missing structured non-mutating suggestion model and acceptance/apply path.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:59; signal=heading; heading=Ripple Range Contract; candidate=## Ripple Range Contract
  - Expected behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher. This closes only the promise expressed by “Ripple Range Contract” in “Ripple Range Contract”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Ripple Range Contract” with the scenario below and register test:crates/opentake-agent/tests/completion_734446f42734a0a0.rs#completion_734446f42734a0a0_editing_suggestions_use_structured_non_mutating_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Ripple Range Contract” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_734446f42734a0a0.rs#completion_734446f42734a0a0_editing_suggestions_use_structured_non_mutating_.

#### requirement-00e262c9f28140a8

- Candidate/source: `doc-ec6b628e7d59055b` at `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:67` (requirement)
- Expected behavior: At docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:67 under “Acceptance Hooks” (heading), the source “## Acceptance Hooks” requires this exact behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.
- Resolution: `validated-ledger-evidence:AG-structured-editing-suggestions` — Six headings restate one missing structured non-mutating suggestion model and acceptance/apply path.
- Exact acceptance contract:
  - Source binding: docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md:67; signal=heading; heading=Acceptance Hooks; candidate=## Acceptance Hooks
  - Expected behavior: Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher. This closes only the promise expressed by “Acceptance Hooks” in “Acceptance Hooks”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Acceptance Hooks” with the scenario below and register test:crates/opentake-agent/tests/completion_ec6b628e7d59055b.rs#completion_ec6b628e7d59055b_editing_suggestions_use_structured_non_mutating_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “Acceptance Hooks” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Editing suggestions use structured, non-mutating proposals that apply through the shared command dispatcher.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_ec6b628e7d59055b.rs#completion_ec6b628e7d59055b_editing_suggestions_use_structured_non_mutating_.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/editing_suggestions.rs#proposal_is_read_only_until_shared_dispatch_apply` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test editing_suggestions proposal_is_read_only_until_shared_dispatch_apply -- --exact`

  Expected: FAIL because one or more of the 6 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/suggestions.rs#EditingSuggestion`, `crates/opentake-agent/src/mcp/dispatch.rs#dispatch`, `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test editing_suggestions proposal_is_read_only_until_shared_dispatch_apply -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 9: AG-smart-reframe + smart-reframe-backend (implementation-slice-9b1932e2d1ee1c26)

**Covered records:**
- `requirement-72fc950460ec6ee7` (requirement)
- `requirement-5b35d8adf0560779` (requirement)
- `requirement-eb82e776b03c5ac7` (requirement)
- `requirement-b08172f5d2b90ea8` (requirement)
- `requirement-528da39dbf930824` (requirement)
- `requirement-2c1080965d1666dc` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#smart_reframe`
- Modify: `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`
- Modify: `crates/opentake-ops/src/intent.rs#plan_smart_reframe`
- Modify: `src-tauri/src/mcp.rs#sampled_frame_bridge`
- Modify: `docs/architecture/editing-automation/EDITING-AUTOMATION/auto-crop-smart-reframe.md`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#smart_reframe_reports_needs_vision_backend`
- Test (reviewed-planned): `crates/opentake-agent/tests/smart_reframe.rs#sample_analyze_review_apply_is_one_undoable_command`
- Test (reviewed-planned): `crates/opentake-agent/tests/smart_reframe_dispatch.rs#production_dispatch_applies_reviewable_crop_or_transform`

**Candidate-bound contracts:**

#### requirement-72fc950460ec6ee7

- Candidate/source: `doc-c8e862559bf67ef1` at `docs/architecture/editing-automation/EDITING-AUTOMATION/auto-crop-smart-reframe.md:3` (requirement)
- Expected behavior: Smart reframe analyzes sampled frames and applies a reviewable crop/transform plan through shared commands.
- Resolution: `validated-ledger-evidence:AG-smart-reframe` — Both slices describe the same autocrop/intent/dispatcher path and the same deterministic missing-vision-backend contradiction.
- Exact acceptance contract:
  - Connect smart_reframe to decoded sample frames and subject/saliency analysis instead of returning needs-vision-backend.
  - Return a reviewable Crop/Transform proposal and apply it only through plan_smart_reframe as one undoable command.
  - On a 1920x1080 fixture with the subject at left, center, and right, produce in-bounds 9:16 crops containing the subject and identical save/reopen transforms.

#### requirement-5b35d8adf0560779

- Candidate/source: `doc-8fa83837670a7f25` at `docs/architecture/editing-automation/EDITING-AUTOMATION/auto-crop-smart-reframe.md:9` (requirement)
- Expected behavior: Smart reframe analyzes sampled frames and applies a reviewable crop/transform plan through shared commands.
- Resolution: `validated-ledger-evidence:AG-smart-reframe` — Both slices describe the same autocrop/intent/dispatcher path and the same deterministic missing-vision-backend contradiction.
- Exact acceptance contract:
  - V1 accepts clip IDs, target aspect ratio, optional focus point, and sampling interval; it rejects empty selection, invalid ratio, and unreadable media with typed paths.
  - Sample first/last plus interval frames, produce one bounded crop/transform plan per clip, and never mutate source media during analysis.
  - Test landscape-to-vertical, already-vertical, still image, offline media, cancelled analysis, and multi-clip ordering with deterministic outputs.

#### requirement-eb82e776b03c5ac7

- Candidate/source: `doc-f3a1d7ee045e0bf6` at `docs/architecture/editing-automation/EDITING-AUTOMATION/auto-crop-smart-reframe.md:25` (requirement)
- Expected behavior: Smart reframe analyzes sampled frames and applies a reviewable crop/transform plan through shared commands.
- Resolution: `validated-ledger-evidence:AG-smart-reframe` — Both slices describe the same autocrop/intent/dispatcher path and the same deterministic missing-vision-backend contradiction.
- Exact acceptance contract:
  - Expose strict smart_reframe arguments with deny-unknown-fields, finite/range validation, short-ID expansion, and a neutral proposal result.
  - Apply accepted crop/transform values through existing SetClipProperties-style commands as one assistant-scoped undo operation.
  - Test malformed paths, ambiguous IDs, proposal-only mode, apply, rejection, partial backend failure, undo/redo, and context-signal serialization.

#### requirement-b08172f5d2b90ea8

- Candidate/source: `doc-e834fb50fdd2ede5` at `docs/architecture/editing-automation/EDITING-AUTOMATION/auto-crop-smart-reframe.md:46` (requirement)
- Expected behavior: Smart reframe analyzes sampled frames and applies a reviewable crop/transform plan through shared commands.
- Resolution: `validated-ledger-evidence:AG-smart-reframe` — Both slices describe the same autocrop/intent/dispatcher path and the same deterministic missing-vision-backend contradiction.
- Exact acceptance contract:
  - Decode bounded representative frames, detect subject/saliency bounds, smooth focus positions, and solve a target-aspect crop clamped to normalized [0,1] coordinates.
  - Use a deterministic fallback to center crop when no confident subject is found and return confidence/reason metadata.
  - Golden-test known bounding boxes at image corners/center, abrupt subject motion, no-subject frames, and aspect ratios 16:9, 1:1, and 9:16; all crops must stay in bounds.

#### requirement-528da39dbf930824

- Candidate/source: `doc-013d3a9e13c271f6` at `docs/architecture/editing-automation/EDITING-AUTOMATION/auto-crop-smart-reframe.md:55` (requirement)
- Expected behavior: Smart reframe analyzes sampled frames and applies a reviewable crop/transform plan through shared commands.
- Resolution: `validated-ledger-evidence:AG-smart-reframe` — Both slices describe the same autocrop/intent/dispatcher path and the same deterministic missing-vision-backend contradiction.
- Exact acceptance contract:
  - Every proposed crop has finite normalized coordinates, positive width/height, exact requested aspect ratio within 1e-6, and no source/timeline mutation before apply.
  - Clip order, duration, trim, speed, media ID, and audio properties remain unchanged; one apply creates one undo entry.
  - Property-test random valid focus boxes/aspect ratios for bounds and determinism, plus cancel/error cases proving byte-identical timeline state.

#### requirement-2c1080965d1666dc

- Candidate/source: `doc-c4a6ccffceb6e34f` at `docs/architecture/editing-automation/EDITING-AUTOMATION/auto-crop-smart-reframe.md:64` (requirement)
- Expected behavior: Smart-reframe acceptance hooks execute against the production dispatch path.
- Resolution: `reviewed-mapping-report:smart-reframe-backend` — Both slices describe the same autocrop/intent/dispatcher path and the same deterministic missing-vision-backend contradiction.
- Exact acceptance contract:
  - Turn the documented hooks into automated dispatch and undo tests.
  - Add a fixture proving subject crop proposal and command application.
  - Verify graceful errors when vision analysis is unavailable.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/mcp/dispatch.rs#smart_reframe_reports_needs_vision_backend` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/smart_reframe.rs#sample_analyze_review_apply_is_one_undoable_command` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/tests/smart_reframe_dispatch.rs#production_dispatch_applies_reviewable_crop_or_transform` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent smart_reframe_reports_needs_vision_backend`
  - Run: `cargo test -p opentake-agent --test smart_reframe sample_analyze_review_apply_is_one_undoable_command -- --exact`
  - Run: `cargo test -p opentake-agent --test smart_reframe_dispatch production_dispatch_applies_reviewable_crop_or_transform -- --exact`

  Expected: FAIL because one or more of the 6 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/dispatch.rs#smart_reframe`, `crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop`, `crates/opentake-ops/src/intent.rs#plan_smart_reframe`, `src-tauri/src/mcp.rs#sampled_frame_bridge`, `docs/architecture/editing-automation/EDITING-AUTOMATION/auto-crop-smart-reframe.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent smart_reframe_reports_needs_vision_backend`
  - Run: `cargo test -p opentake-agent --test smart_reframe sample_analyze_review_apply_is_one_undoable_command -- --exact`
  - Run: `cargo test -p opentake-agent --test smart_reframe_dispatch production_dispatch_applies_reviewable_crop_or_transform -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 10: AG-workflow-recipes (implementation-slice-feedb4df238f347a)

**Covered records:**
- `requirement-427b5b7adcfa69d9` (requirement)
- `requirement-e1a3d692b15adbdd` (requirement)
- `requirement-77fa5759c7465280` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/plugin/registry.rs#PluginRegistry`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#activate_workflow`
- Modify: `crates/opentake-agent/src/plugin/builtin/audio-first/plugin.json`
- Modify: `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md`
- Test (existing-owned): `crates/opentake-agent/src/plugin/registry.rs#builtin_audio_first_loads_and_validates`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#activate_workflow_returns_instructions_and_marks_active`
- Test (reviewed-planned): `crates/opentake-agent/tests/workflow_recipe_acceptance.rs#every_builtin_recipe_uses_only_live_tools_and_is_undoable`

**Candidate-bound contracts:**

#### requirement-427b5b7adcfa69d9

- Candidate/source: `doc-f97ab6c6e575f770` at `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:26` (requirement)
- Expected behavior: Built-in recipes execute only live tools and produce reviewable, undoable edits.
- Resolution: `validated-ledger-evidence:AG-workflow-recipes` — Activation exists; each built-in recipe still needs live-tool and undo-semantics acceptance.
- Exact acceptance contract:
  - Validate every bundled recipe against the plugin/recipe schema and ensure all referenced tool names resolve to live dispatcher handlers.
  - Each recipe must support dry-run plan review, apply, cancellation, and rollback/undo without bypassing shared commands.
  - Run schema, missing-tool, success, mid-step failure, cancellation, signal/rule injection, and restart-activation cases for every built-in recipe.

#### requirement-e1a3d692b15adbdd

- Candidate/source: `doc-fc93c14907da0e0b` at `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:45` (requirement)
- Expected behavior: Built-in recipes execute only live tools and produce reviewable, undoable edits.
- Resolution: `validated-ledger-evidence:AG-workflow-recipes` — Activation exists; each built-in recipe still needs live-tool and undo-semantics acceptance.
- Exact acceptance contract:
  - Detect or accept beat frames, validate selected media IDs, and return an ordered placement/cut plan before mutation.
  - Apply the montage through shared add/move/split commands as one reviewable undo group while clamping cuts to clip handles.
  - On a fixed eight-beat fixture, assert one expected cut/placement per beat within one frame, then verify undo, save/reopen, preview timing, and export duration.

#### requirement-77fa5759c7465280

- Candidate/source: `doc-368724aa9c3edb58` at `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md:63` (requirement)
- Expected behavior: Built-in recipes execute only live tools and produce reviewable, undoable edits.
- Resolution: `validated-ledger-evidence:AG-workflow-recipes` — Activation exists; each built-in recipe still needs live-tool and undo-semantics acceptance.
- Exact acceptance contract:
  - Combine a live smart-reframe proposal with target 9:16 project/output settings and preserve source clip timing/audio.
  - Expose dry-run crop review, per-clip override, apply, cancellation, and one-step undo; no crop may leave normalized bounds.
  - Run a three-clip landscape fixture with left/center/right subjects and verify crop containment, save/reopen equality, and 9:16 preview/export dimensions.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/plugin/registry.rs#builtin_audio_first_loads_and_validates` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#activate_workflow_returns_instructions_and_marks_active` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/workflow_recipe_acceptance.rs#every_builtin_recipe_uses_only_live_tools_and_is_undoable` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent builtin_audio_first_loads_and_validates`
  - Run: `cargo test -p opentake-agent activate_workflow_returns_instructions_and_marks_active`
  - Run: `cargo test -p opentake-agent --test workflow_recipe_acceptance every_builtin_recipe_uses_only_live_tools_and_is_undoable -- --exact`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/plugin/registry.rs#PluginRegistry`, `crates/opentake-agent/src/mcp/dispatch.rs#activate_workflow`, `crates/opentake-agent/src/plugin/builtin/audio-first/plugin.json`, `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent builtin_audio_first_loads_and_validates`
  - Run: `cargo test -p opentake-agent activate_workflow_returns_instructions_and_marks_active`
  - Run: `cargo test -p opentake-agent --test workflow_recipe_acceptance every_builtin_recipe_uses_only_live_tools_and_is_undoable -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 11: AG-byok-keychain-boundary + byok-key-normalization-privacy (implementation-slice-2584e382e6a35820)

**Covered records:**
- `requirement-9490ea25dd967bf4` (requirement)
- `requirement-03f73b8c63a3dac8` (requirement)
- `requirement-632963638d577e50` (requirement)
- `requirement-b888bbde0d13a19b` (requirement)

**Files:**
- Modify: `crates/opentake-gen/src/keys.rs#KeyStore`
- Modify: `crates/opentake-gen/src/keys.rs#KeyringStore`
- Modify: `crates/opentake-gen/src/keys.rs#normalize`
- Modify: `src-tauri/src/chat.rs#ChatState::new`
- Modify: `src-tauri/src/secret.rs#secret_load`
- Modify: `src-tauri/src/secret.rs#secret_save`
- Modify: `docs/modules/opentake-agent/SPEC.md`
- Modify: `docs/modules/opentake-gen/keys-byok.md`
- Modify: `docs/specs/agent/10-implementation.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/secret_leakage.rs#keys_never_serialize_to_project_log_telemetry_mcp_webview`
- Test (existing-owned): `crates/opentake-gen/src/keys.rs#accounts_are_stable_and_distinct`
- Test (existing-owned): `crates/opentake-gen/src/keys.rs#empty_and_whitespace_values_are_none`
- Test (existing-owned): `crates/opentake-gen/src/keys.rs#load_trims_surrounding_whitespace`
- Test (existing-owned): `src-tauri/src/secret.rs#long_keys_reveal_only_last_four`
- Test (reviewed-planned): `src-tauri/src/secret.rs#production_anthropic_key_never_enters_project_log_or_telemetry`

**Candidate-bound contracts:**

#### requirement-9490ea25dd967bf4

- Candidate/source: `doc-e3ada055247fccae` at `docs/modules/opentake-agent/SPEC.md:1070` (requirement)
- Expected behavior: At docs/modules/opentake-agent/SPEC.md:1070 under “9.4 安全检查清单（提交前）” (unchecked), the source “- [ ] Anthropic key 存 OS keychain（`keyring`），绝不入 `project.json`/日志/遥测。” requires this exact behavior: Store the Anthropic BYOK key only in the OS credential store and expose only masked status to the WebView.
- Resolution: `validated-ledger-evidence:AG-byok-keychain-boundary` — Both slices cover normalized OS-keychain storage plus the negative project/log/telemetry leakage boundary and share the keys implementation and tests.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-agent/SPEC.md:1070; signal=unchecked; heading=9.4 安全检查清单（提交前）; candidate=- [ ] Anthropic key 存 OS keychain（`keyring`），绝不入 `project.json`/日志/遥测。
  - Expected behavior: Store the Anthropic BYOK key only in the OS credential store and expose only masked status to the WebView. This closes only the promise expressed by “Anthropic key 存 OS keychain（`keyring`），绝不入 `project.json`/日志/遥测。” in “9.4 安全检查清单（提交前）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “Anthropic key 存 OS keychain（`keyring`），绝不入 `project.json`/日志/遥测。” with the scenario below and register test:web/src/__tests__/completion/doc-e3ada055247fccae.test.ts#completion_e3ada055247fccae_store_the_anthropic_byok_key_only_in_the_os_cred
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “Anthropic key 存 OS keychain（`keyring`），绝不入 `project.json`/日志/遥测。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Store the Anthropic BYOK key only in the OS credential store and expose only masked status to the WebView.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-e3ada055247fccae.test.ts#completion_e3ada055247fccae_store_the_anthropic_byok_key_only_in_the_os_cred.

#### requirement-03f73b8c63a3dac8

- Candidate/source: `doc-f9ef92937ddd1f0f` at `docs/modules/opentake-gen/keys-byok.md:50` (requirement)
- Expected behavior: At docs/modules/opentake-gen/keys-byok.md:50 under “安全要点（移植铁律 + 安全）” (gap-marker), the source “- **值边界防御**：读出即 trim，空白等同缺失，避免「看似有 key 实为空格」。” requires this exact behavior: Trim credential values at the KeyStore boundary and treat blank values as absent under stable provider accounts.
- Resolution: `reviewed-mapping-report:byok-key-normalization-privacy` — Both slices cover normalized OS-keychain storage plus the negative project/log/telemetry leakage boundary and share the keys implementation and tests.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-gen/keys-byok.md:50; signal=gap-marker; heading=安全要点（移植铁律 + 安全）; candidate=- **值边界防御**：读出即 trim，空白等同缺失，避免「看似有 key 实为空格」。
  - Expected behavior: Trim credential values at the KeyStore boundary and treat blank values as absent under stable provider accounts. This closes only the promise expressed by “**值边界防御**：读出即 trim，空白等同缺失，避免「看似有 key 实为空格」。” in “安全要点（移植铁律 + 安全）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “**值边界防御**：读出即 trim，空白等同缺失，避免「看似有 key 实为空格」。” with the scenario below and register test:crates/opentake-agent/tests/completion_f9ef92937ddd1f0f.rs#completion_f9ef92937ddd1f0f_trim_credential_values_at_the_keystore_boundary_
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “**值边界防御**：读出即 trim，空白等同缺失，避免「看似有 key 实为空格」。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Trim credential values at the KeyStore boundary and treat blank values as absent under stable provider accounts.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_f9ef92937ddd1f0f.rs#completion_f9ef92937ddd1f0f_trim_credential_values_at_the_keystore_boundary_.

#### requirement-632963638d577e50

- Candidate/source: `doc-e03949e75a32885c` at `docs/modules/opentake-gen/keys-byok.md:55` (requirement)
- Expected behavior: At docs/modules/opentake-gen/keys-byok.md:55 under “对应上游 Swift” (gap-marker), the source “- `KeyStore` / `KeyringStore` ← `KeychainStore.swift:4-52`（service=bundle id、account=稳定串、trim、空即缺失）。” requires this exact behavior: Trim credential values at the KeyStore boundary and treat blank values as absent under stable provider accounts.
- Resolution: `reviewed-mapping-report:byok-key-normalization-privacy` — Both slices cover normalized OS-keychain storage plus the negative project/log/telemetry leakage boundary and share the keys implementation and tests.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-gen/keys-byok.md:55; signal=gap-marker; heading=对应上游 Swift; candidate=- `KeyStore` / `KeyringStore` ← `KeychainStore.swift:4-52`（service=bundle id、account=稳定串、trim、空即缺失）。
  - Expected behavior: Trim credential values at the KeyStore boundary and treat blank values as absent under stable provider accounts. This closes only the promise expressed by “`KeyStore` / `KeyringStore` ← `KeychainStore.swift:4-52`（service=bundle id、account=稳定串、trim、空即缺失）。” in “对应上游 Swift”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “`KeyStore` / `KeyringStore` ← `KeychainStore.swift:4-52`（service=bundle id、account=稳定串、trim、空即缺失）。” with the scenario below and register test:crates/opentake-agent/tests/completion_e03949e75a32885c.rs#completion_e03949e75a32885c_trim_credential_values_at_the_keystore_boundary_
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “`KeyStore` / `KeyringStore` ← `KeychainStore.swift:4-52`（service=bundle id、account=稳定串、trim、空即缺失）。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Trim credential values at the KeyStore boundary and treat blank values as absent under stable provider accounts.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_e03949e75a32885c.rs#completion_e03949e75a32885c_trim_credential_values_at_the_keystore_boundary_.

#### requirement-b888bbde0d13a19b

- Candidate/source: `doc-bd08e335548c7dcf` at `docs/specs/agent/10-implementation.md:85` (requirement)
- Expected behavior: At docs/specs/agent/10-implementation.md:85, the source “- [ ] Anthropic key 存 OS keychain（`keyring`），绝不入 `project.json`/日志/遥测。” requires this exact behavior: Store Anthropic and all BYOK secrets only in the OS keychain and ensure plaintext never appears in project.json, logs, telemetry, serialized settings, MCP responses, or the WebView.
- Resolution: `reviewed-mapping-report:byok-key-normalization-privacy` — Both slices cover normalized OS-keychain storage plus the negative project/log/telemetry leakage boundary and share the keys implementation and tests.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/10-implementation.md:85; signal=unchecked; heading=9.4 安全检查清单（提交前）; candidate=- [ ] Anthropic key 存 OS keychain（`keyring`），绝不入 `project.json`/日志/遥测。
  - Expected behavior: Store Anthropic and all BYOK secrets only in the OS keychain and ensure plaintext never appears in project.json, logs, telemetry, serialized settings, MCP responses, or the WebView.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_10_line_85_bd08e335548c7dcf.rs#spec_agent_10_line_85_bd08e335548c7dcf_byok_os_keychain_plaintext_non_disclosure
  - Initial state/input/event: write, read, replace, and delete a sentinel API key through a fake OS keychain while saving a project, emitting logs/telemetry, serializing settings, invoking MCP, and requesting the WebView-safe provider status.
  - Code/store/API/Rust effect: persist only the secret in keychain storage, expose redacted configured/unconfigured state to Tauri/WebView, and scrub the sentinel from every project, logs, telemetry, response, and error path.
  - Visible/returned assertion: assert keychain round-trip/delete behavior and a byte scan proving the plaintext sentinel is absent from project.json, logs, telemetry, settings, MCP output, and WebView payloads.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_10_line_85_bd08e335548c7dcf.rs#spec_agent_10_line_85_bd08e335548c7dcf_byok_os_keychain_plaintext_non_disclosure.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/secret_leakage.rs#keys_never_serialize_to_project_log_telemetry_mcp_webview` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-gen/src/keys.rs#accounts_are_stable_and_distinct` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-gen/src/keys.rs#empty_and_whitespace_values_are_none` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-gen/src/keys.rs#load_trims_surrounding_whitespace` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/secret.rs#long_keys_reveal_only_last_four` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `src-tauri/src/secret.rs#production_anthropic_key_never_enters_project_log_or_telemetry` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test secret_leakage keys_never_serialize_to_project_log_telemetry_mcp_webview -- --exact`
  - Run: `cargo test -p opentake-gen accounts_are_stable_and_distinct`
  - Run: `cargo test -p opentake-gen empty_and_whitespace_values_are_none`
  - Run: `cargo test -p opentake-gen load_trims_surrounding_whitespace`
  - Run: `cargo test -p opentake-tauri long_keys_reveal_only_last_four`
  - Run: `cargo test -p opentake-tauri production_anthropic_key_never_enters_project_log_or_telemetry`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-gen/src/keys.rs#KeyStore`, `crates/opentake-gen/src/keys.rs#KeyringStore`, `crates/opentake-gen/src/keys.rs#normalize`, `src-tauri/src/chat.rs#ChatState::new`, `src-tauri/src/secret.rs#secret_load`, `src-tauri/src/secret.rs#secret_save`, `docs/modules/opentake-agent/SPEC.md`, `docs/modules/opentake-gen/keys-byok.md`, `docs/specs/agent/10-implementation.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test secret_leakage keys_never_serialize_to_project_log_telemetry_mcp_webview -- --exact`
  - Run: `cargo test -p opentake-gen accounts_are_stable_and_distinct`
  - Run: `cargo test -p opentake-gen empty_and_whitespace_values_are_none`
  - Run: `cargo test -p opentake-gen load_trims_surrounding_whitespace`
  - Run: `cargo test -p opentake-tauri long_keys_reveal_only_last_four`
  - Run: `cargo test -p opentake-tauri production_anthropic_key_never_enters_project_log_or_telemetry`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 12: AG-plugin-prompt-fencing (implementation-slice-3daa1d464004f67f)

**Covered records:**
- `requirement-069e3265180ccaeb` (requirement)
- `requirement-81cd8f7162722063` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/prompt/assemble.rs#assemble_system_prompt`
- Modify: `docs/modules/opentake-agent/SPEC.md`
- Modify: `docs/specs/agent/10-implementation.md`
- Test (existing-owned): `crates/opentake-agent/src/prompt/assemble.rs#plugin_content_is_fenced_as_untrusted`
- Test (existing-owned): `crates/opentake-agent/src/prompt/assemble.rs#active_plugin_injects_instructions_and_rules`

**Candidate-bound contracts:**

#### requirement-069e3265180ccaeb

- Candidate/source: `doc-e3d0a5b22610544c` at `docs/modules/opentake-agent/SPEC.md:1073` (requirement)
- Expected behavior: At docs/modules/opentake-agent/SPEC.md:1073 under “9.4 安全检查清单（提交前）” (unchecked), the source “- [ ] 插件 `instructions.md` 注入系统提示词前不可信内容隔离（标注来源 `plugin:{id}`，避免提示注入冒充系统指令）。” requires this exact behavior: Fence plugin instructions as untrusted content labeled with plugin identity before adding them to the system prompt.
- Resolution: `validated-ledger-evidence:AG-plugin-prompt-fencing` — Assembly labels plugin identity and fences installed guidance as non-system advice.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-agent/SPEC.md:1073; signal=unchecked; heading=9.4 安全检查清单（提交前）; candidate=- [ ] 插件 `instructions.md` 注入系统提示词前不可信内容隔离（标注来源 `plugin:{id}`，避免提示注入冒充系统指令）。
  - Expected behavior: Fence plugin instructions as untrusted content labeled with plugin identity before adding them to the system prompt. This closes only the promise expressed by “插件 `instructions.md` 注入系统提示词前不可信内容隔离（标注来源 `plugin:{id}`，避免提示注入冒充系统指令）。” in “9.4 安全检查清单（提交前）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “插件 `instructions.md` 注入系统提示词前不可信内容隔离（标注来源 `plugin:{id}`，避免提示注入冒充系统指令）。” with the scenario below and register test:crates/opentake-agent/tests/completion_e3d0a5b22610544c.rs#completion_e3d0a5b22610544c_fence_plugin_instructions_as_untrusted_content_l
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “插件 `instructions.md` 注入系统提示词前不可信内容隔离（标注来源 `plugin:{id}`，避免提示注入冒充系统指令）。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Fence plugin instructions as untrusted content labeled with plugin identity before adding them to the system prompt.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_e3d0a5b22610544c.rs#completion_e3d0a5b22610544c_fence_plugin_instructions_as_untrusted_content_l.

#### requirement-81cd8f7162722063

- Candidate/source: `doc-cca17c3e055cd562` at `docs/specs/agent/10-implementation.md:88` (requirement)
- Expected behavior: At docs/specs/agent/10-implementation.md:88, the source “- [ ] 插件 `instructions.md` 注入系统提示词前不可信内容隔离（标注来源 `plugin:{id}`，避免提示注入冒充系统指令）。” requires this exact behavior: Fence plugin instructions.md as untrusted content with a plugin:{id} source label so prompt injection cannot impersonate or override system instructions.
- Resolution: `validated-ledger-evidence:AG-plugin-prompt-fencing` — Assembly labels plugin identity and fences installed guidance as non-system advice.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/10-implementation.md:88; signal=unchecked; heading=9.4 安全检查清单（提交前）; candidate=- [ ] 插件 `instructions.md` 注入系统提示词前不可信内容隔离（标注来源 `plugin:{id}`，避免提示注入冒充系统指令）。
  - Expected behavior: Fence plugin instructions.md as untrusted content with a plugin:{id} source label so prompt injection cannot impersonate or override system instructions.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_10_line_88_cca17c3e055cd562.rs#spec_agent_10_line_88_cca17c3e055cd562_plugin_instructions_untrusted_source_boundary
  - Initial state/input/event: activate a plugin whose instructions contain a benign rule plus adversarial text claiming to be a system message and requesting removal of prior constraints, then assemble the system prompt.
  - Code/store/API/Rust effect: place plugin text inside an explicit untrusted boundary, attach the exact plugin:{id} source label, preserve base system prompt precedence, and avoid parsing plugin text as system-role content.
  - Visible/returned assertion: assert source label and boundary placement, unchanged base system instructions, retained literal adversarial text only inside the plugin block, and no prompt injection effect on policy/order.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_10_line_88_cca17c3e055cd562.rs#spec_agent_10_line_88_cca17c3e055cd562_plugin_instructions_untrusted_source_boundary.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/prompt/assemble.rs#plugin_content_is_fenced_as_untrusted` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/prompt/assemble.rs#active_plugin_injects_instructions_and_rules` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent plugin_content_is_fenced_as_untrusted`
  - Run: `cargo test -p opentake-agent active_plugin_injects_instructions_and_rules`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/prompt/assemble.rs#assemble_system_prompt`, `docs/modules/opentake-agent/SPEC.md`, `docs/specs/agent/10-implementation.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent plugin_content_is_fenced_as_untrusted`
  - Run: `cargo test -p opentake-agent active_plugin_injects_instructions_and_rules`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 13: AG-short-id-prompt-contract + short-id-contract (implementation-slice-fd9d2fc45e51d9ab)

**Covered records:**
- `requirement-b3c9690870310f25` (requirement)
- `requirement-81e1bac10cf0afe3` (requirement)
- `requirement-e7f57e8e3030cffa` (requirement)
- `requirement-c8ece7bee7d231c6` (requirement)
- `requirement-bb0bf4e53d19f908` (requirement)
- `requirement-9f42f494f7bd639c` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/prompt/base.rs#CORE_MODEL`
- Modify: `crates/opentake-agent/src/tools/short_id.rs#current_id_universe`
- Modify: `crates/opentake-agent/src/tools/short_id.rs#expand_id_prefixes`
- Modify: `crates/opentake-agent/src/tools/short_id.rs#short_id_map`
- Modify: `crates/opentake-agent/src/tools/short_id.rs#shorten_ids`
- Modify: `docs/modules/opentake-agent/prompt.md`
- Modify: `docs/specs/agent/3-short-id.md`
- Test (existing-owned): `crates/opentake-agent/src/prompt/base.rs#short_id_contract_sentence_verbatim`
- Test (existing-owned): `crates/opentake-agent/src/tools/short_id.rs#expand_unique_prefix_resolves_to_full`
- Test (existing-owned): `crates/opentake-agent/src/tools/short_id.rs#shared_prefix_extends_until_unique`
- Test (existing-owned): `crates/opentake-agent/src/tools/short_id.rs#unique_id_shortens_to_floor`

**Candidate-bound contracts:**

#### requirement-b3c9690870310f25

- Candidate/source: `doc-9bc7c9cdadaaa081` at `docs/modules/opentake-agent/prompt.md:32` (requirement)
- Expected behavior: At docs/modules/opentake-agent/prompt.md:32 under “base.rs：分段 base 提示” (gap-marker), the source “- 短 id：`Pass them back exactly as given — never pad, complete, or guess a longer form.`（缺失则 [short_id](dispatch-tools.md) 契约失效）” requires this exact behavior: Tell the model to pass shortened IDs back exactly without padding or guessing.
- Resolution: `validated-ledger-evidence:AG-short-id-prompt-contract` — Both slices cover one no-padding/no-guessing short-ID prompt and resolver contract with the same implementation and focused tests.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-agent/prompt.md:32; signal=gap-marker; heading=base.rs：分段 base 提示; candidate=- 短 id：`Pass them back exactly as given — never pad, complete, or guess a longer form.`（缺失则 [short_id](dispatch-tools.md) 契约失效）
  - Expected behavior: Tell the model to pass shortened IDs back exactly without padding or guessing. This closes only the promise expressed by “短 id：`Pass them back exactly as given — never pad, complete, or guess a longer form.`（缺失则 [short_id](dispatch-tools.md) 契约失效）” in “base.rs：分段 base 提示”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “短 id：`Pass them back exactly as given — never pad, complete, or guess a longer form.`（缺失则 [short_id](dispatch-tools.md) 契约失效）” with the scenario below and register test:crates/opentake-agent/tests/completion_9bc7c9cdadaaa081.rs#completion_9bc7c9cdadaaa081_tell_the_model_to_pass_shortened_ids_back_exactl
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “短 id：`Pass them back exactly as given — never pad, complete, or guess a longer form.`（缺失则 [short_id](dispatch-tools.md) 契约失效）”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Tell the model to pass shortened IDs back exactly without padding or guessing.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_9bc7c9cdadaaa081.rs#completion_9bc7c9cdadaaa081_tell_the_model_to_pass_shortened_ids_back_exactl.

#### requirement-81e1bac10cf0afe3

- Candidate/source: `doc-6abcbfdb95355561` at `docs/specs/agent/3-short-id.md:1` (requirement)
- Expected behavior: At docs/specs/agent/3-short-id.md:1 under “短 ID 系统（出站缩短 / 入站展开）算法” (heading), the source “# 短 ID 系统（出站缩短 / 入站展开）算法” requires this exact behavior: Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.
- Resolution: `reviewed-mapping-report:short-id-contract` — Both slices cover one no-padding/no-guessing short-ID prompt and resolver contract with the same implementation and focused tests.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/3-short-id.md:1; signal=heading; heading=短 ID 系统（出站缩短 / 入站展开）算法; candidate=# 短 ID 系统（出站缩短 / 入站展开）算法
  - Expected behavior: Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors. This closes only the promise expressed by “短 ID 系统（出站缩短 / 入站展开）算法” in “短 ID 系统（出站缩短 / 入站展开）算法”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “短 ID 系统（出站缩短 / 入站展开）算法” with the scenario below and register test:crates/opentake-agent/tests/completion_6abcbfdb95355561.rs#completion_6abcbfdb95355561_short_ids_shorten_outbound_identifiers_and_expan
  - Initial state/input/event: start from the smallest valid fixture for “Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “短 ID 系统（出站缩短 / 入站展开）算法”.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_6abcbfdb95355561.rs#completion_6abcbfdb95355561_short_ids_shorten_outbound_identifiers_and_expan.

#### requirement-e7f57e8e3030cffa

- Candidate/source: `doc-5ce75bbe8c8bb673` at `docs/specs/agent/3-short-id.md:5` (requirement)
- Expected behavior: At docs/specs/agent/3-short-id.md:5 under “3.1 为什么” (heading), the source “## 3.1 为什么” requires this exact behavior: Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.
- Resolution: `reviewed-mapping-report:short-id-contract` — Both slices cover one no-padding/no-guessing short-ID prompt and resolver contract with the same implementation and focused tests.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/3-short-id.md:5; signal=heading; heading=3.1 为什么; candidate=## 3.1 为什么
  - Expected behavior: Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors. This closes only the promise expressed by “3.1 为什么” in “3.1 为什么”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “3.1 为什么” with the scenario below and register test:crates/opentake-agent/tests/completion_5ce75bbe8c8bb673.rs#completion_5ce75bbe8c8bb673_short_ids_shorten_outbound_identifiers_and_expan
  - Initial state/input/event: start from the smallest valid fixture for “Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “3.1 为什么”.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_5ce75bbe8c8bb673.rs#completion_5ce75bbe8c8bb673_short_ids_shorten_outbound_identifiers_and_expan.

#### requirement-c8ece7bee7d231c6

- Candidate/source: `doc-c82a1e01421de90f` at `docs/specs/agent/3-short-id.md:9` (requirement)
- Expected behavior: At docs/specs/agent/3-short-id.md:9 under “3.2 ID 宇宙（`currentIdUniverse`，`+ShortId.swift:26-39`）” (heading), the source “## 3.2 ID 宇宙（`currentIdUniverse`，`+ShortId.swift:26-39`）” requires this exact behavior: Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.
- Resolution: `reviewed-mapping-report:short-id-contract` — Both slices cover one no-padding/no-guessing short-ID prompt and resolver contract with the same implementation and focused tests.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/3-short-id.md:9; signal=heading; heading=3.2 ID 宇宙（`currentIdUniverse`，`+ShortId.swift:26-39`）; candidate=## 3.2 ID 宇宙（`currentIdUniverse`，`+ShortId.swift:26-39`）
  - Expected behavior: Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors. This closes only the promise expressed by “3.2 ID 宇宙（`currentIdUniverse`，`+ShortId.swift:26-39`）” in “3.2 ID 宇宙（`currentIdUniverse`，`+ShortId.swift:26-39`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “3.2 ID 宇宙（`currentIdUniverse`，`+ShortId.swift:26-39`）” with the scenario below and register test:crates/opentake-agent/tests/completion_c82a1e01421de90f.rs#completion_c82a1e01421de90f_short_ids_shorten_outbound_identifiers_and_expan
  - Initial state/input/event: start from the smallest valid fixture for “Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “3.2 ID 宇宙（`currentIdUniverse`，`+ShortId.swift:26-39`）”.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_c82a1e01421de90f.rs#completion_c82a1e01421de90f_short_ids_shorten_outbound_identifiers_and_expan.

#### requirement-bb0bf4e53d19f908

- Candidate/source: `doc-2edc94a307cfb1a0` at `docs/specs/agent/3-short-id.md:26` (requirement)
- Expected behavior: At docs/specs/agent/3-short-id.md:26 under “3.3 出站缩短（`shorteningIds` + `shortIdMap`，`:43-64`）” (heading), the source “## 3.3 出站缩短（`shorteningIds` + `shortIdMap`，`:43-64`）” requires this exact behavior: Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.
- Resolution: `reviewed-mapping-report:short-id-contract` — Both slices cover one no-padding/no-guessing short-ID prompt and resolver contract with the same implementation and focused tests.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/3-short-id.md:26; signal=heading; heading=3.3 出站缩短（`shorteningIds` + `shortIdMap`，`:43-64`）; candidate=## 3.3 出站缩短（`shorteningIds` + `shortIdMap`，`:43-64`）
  - Expected behavior: Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors. This closes only the promise expressed by “3.3 出站缩短（`shorteningIds` + `shortIdMap`，`:43-64`）” in “3.3 出站缩短（`shorteningIds` + `shortIdMap`，`:43-64`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “3.3 出站缩短（`shorteningIds` + `shortIdMap`，`:43-64`）” with the scenario below and register test:crates/opentake-agent/tests/completion_2edc94a307cfb1a0.rs#completion_2edc94a307cfb1a0_short_ids_shorten_outbound_identifiers_and_expan
  - Initial state/input/event: start from the smallest valid fixture for “Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “3.3 出站缩短（`shorteningIds` + `shortIdMap`，`:43-64`）”.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_2edc94a307cfb1a0.rs#completion_2edc94a307cfb1a0_short_ids_shorten_outbound_identifiers_and_expan.

#### requirement-9f42f494f7bd639c

- Candidate/source: `doc-442c526debd9dec3` at `docs/specs/agent/3-short-id.md:56` (requirement)
- Expected behavior: At docs/specs/agent/3-short-id.md:56 under “3.4 入站展开（`expandingIdPrefixes` + `expandOne`，`:68-99`）” (heading), the source “## 3.4 入站展开（`expandingIdPrefixes` + `expandOne`，`:68-99`）” requires this exact behavior: Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.
- Resolution: `reviewed-mapping-report:short-id-contract` — Both slices cover one no-padding/no-guessing short-ID prompt and resolver contract with the same implementation and focused tests.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/3-short-id.md:56; signal=heading; heading=3.4 入站展开（`expandingIdPrefixes` + `expandOne`，`:68-99`）; candidate=## 3.4 入站展开（`expandingIdPrefixes` + `expandOne`，`:68-99`）
  - Expected behavior: Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors. This closes only the promise expressed by “3.4 入站展开（`expandingIdPrefixes` + `expandOne`，`:68-99`）” in “3.4 入站展开（`expandingIdPrefixes` + `expandOne`，`:68-99`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “3.4 入站展开（`expandingIdPrefixes` + `expandOne`，`:68-99`）” with the scenario below and register test:crates/opentake-agent/tests/completion_442c526debd9dec3.rs#completion_442c526debd9dec3_short_ids_shorten_outbound_identifiers_and_expan
  - Initial state/input/event: start from the smallest valid fixture for “Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.”, then issue both the valid request and the exact malformed, missing, boundary, or hostile input named by “3.4 入站展开（`expandingIdPrefixes` + `expandOne`，`:68-99`）”.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, on invalid input, reject before any store, project, filesystem, provider, or timeline mutation; on valid input, execute exactly the typed operation “Short IDs shorten outbound identifiers and expand unique inbound prefixes with ambiguity errors.” once.
  - Visible/returned assertion: assert the exact success payload for the valid case and a stable typed error for the invalid case, including zero partial side effects and no leaked internal path or credential.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_442c526debd9dec3.rs#completion_442c526debd9dec3_short_ids_shorten_outbound_identifiers_and_expan.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/prompt/base.rs#short_id_contract_sentence_verbatim` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/tools/short_id.rs#expand_unique_prefix_resolves_to_full` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/tools/short_id.rs#shared_prefix_extends_until_unique` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/tools/short_id.rs#unique_id_shortens_to_floor` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent short_id_contract_sentence_verbatim`
  - Run: `cargo test -p opentake-agent expand_unique_prefix_resolves_to_full`
  - Run: `cargo test -p opentake-agent shared_prefix_extends_until_unique`
  - Run: `cargo test -p opentake-agent unique_id_shortens_to_floor`

  Expected: FAIL because one or more of the 6 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/prompt/base.rs#CORE_MODEL`, `crates/opentake-agent/src/tools/short_id.rs#current_id_universe`, `crates/opentake-agent/src/tools/short_id.rs#expand_id_prefixes`, `crates/opentake-agent/src/tools/short_id.rs#short_id_map`, `crates/opentake-agent/src/tools/short_id.rs#shorten_ids`, `docs/modules/opentake-agent/prompt.md`, `docs/specs/agent/3-short-id.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent short_id_contract_sentence_verbatim`
  - Run: `cargo test -p opentake-agent expand_unique_prefix_resolves_to_full`
  - Run: `cargo test -p opentake-agent shared_prefix_extends_until_unique`
  - Run: `cargo test -p opentake-agent unique_id_shortens_to_floor`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 14: AG-project-open-order (implementation-slice-959404020d24576c)

**Covered records:**
- `requirement-58cdfb5cd8505898` (requirement)

**Files:**
- Modify: `crates/opentake-project/src/bundle.rs#open_from_root`
- Modify: `crates/opentake-core/src/session.rs#open_project`
- Modify: `docs/modules/opentake-core/OVERVIEW.md`
- Test (existing-owned): `crates/opentake-project/tests/roundtrip.rs#malformed_generation_log_is_ignored`
- Test (existing-owned): `crates/opentake-core/src/core.rs#open_save_roundtrip_through_core_emits_lifecycle_events`

**Candidate-bound contracts:**

#### requirement-58cdfb5cd8505898

- Candidate/source: `doc-4b33ccc661231401` at `docs/modules/opentake-core/OVERVIEW.md:159` (requirement)
- Expected behavior: At docs/modules/opentake-core/OVERVIEW.md:159 under “移植铁律（Swift → Rust，本模块强约束）” (gap-marker), the source “- **open 装配顺序照搬上游 `makeWindowControllers`**：先 decode `timeline`（version 归 0）→ 记 `project_dir` → decode `manifest` → decode `generation_log`（**宽松**：损坏降级为 `None`，不致命；只 `project.json` 缺失才报错）。open **不**发 `TimelineChanged`（前端自取首个快照）。” requires this exact behavior: Open project components in retained-root order, reset runtime version, tolerate generation-log corruption, and emit no TimelineChanged during open.
- Resolution: `validated-ledger-evidence:AG-project-open-order` — Retained-root ordering, lenient generation-log decode and lifecycle-only open behavior are tracked under project/core.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-core/OVERVIEW.md:159; signal=gap-marker; heading=移植铁律（Swift → Rust，本模块强约束）; candidate=- **open 装配顺序照搬上游 `makeWindowControllers`**：先 decode `timeline`（version 归 0）→ 记 `project_dir` → decode `manifest` → decode `generation_log`（**宽松**：损坏降级为 `None`，不致命；只 `project.json` 缺失才报错）。open **不**发 `TimelineChanged`（前端自取首个快照）。
  - Expected behavior: Open project components in retained-root order, reset runtime version, tolerate generation-log corruption, and emit no TimelineChanged during open. This closes only the promise expressed by “**open 装配顺序照搬上游 `makeWindowControllers`**：先 decode `timeline`（version 归 0）→ 记 `project_dir` → decode `manifest` → decode `generation_log`（**宽松**：损坏降级为 `None`，不致命；只 `project.json` 缺失才报错）。open **不**发 `TimelineChanged`（前端自取首个快照）。” in “移植铁律（Swift → Rust，本模块强约束）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “**open 装配顺序照搬上游 `makeWindowControllers`**：先 decode `timeline`（version 归 0）→ 记 `project_dir` → decode `manifest` → decode `generation_log`（**宽松**：损坏降级为 `None`，不致命；只 `project.json` 缺失才报错）。open **不**发 `TimelineChanged`（前端自取首个快照）。” with the scenario below and register test:web/src/__tests__/completion/doc-4b33ccc661231401.test.ts#completion_4b33ccc661231401_open_project_components_in_retained_root_order_r
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “**open 装配顺序照搬上游 `makeWindowControllers`**：先 decode `timeline`（version 归 0）→ 记 `project_dir` → decode `manifest` → decode `generation_log`（**宽松**：损坏降级为 `None`，不致命；只 `project.json` 缺失才报错）。open **不**发 `TimelineChanged`（前端自取首个快照）。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Open project components in retained-root order, reset runtime version, tolerate generation-log corruption, and emit no TimelineChanged during open.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-4b33ccc661231401.test.ts#completion_4b33ccc661231401_open_project_components_in_retained_root_order_r.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-project/tests/roundtrip.rs#malformed_generation_log_is_ignored` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-core/src/core.rs#open_save_roundtrip_through_core_emits_lifecycle_events` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-project --test roundtrip malformed_generation_log_is_ignored -- --exact`
  - Run: `cargo test -p opentake-core open_save_roundtrip_through_core_emits_lifecycle_events`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-project/src/bundle.rs#open_from_root`, `crates/opentake-core/src/session.rs#open_project`, `docs/modules/opentake-core/OVERVIEW.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-project --test roundtrip malformed_generation_log_is_ignored -- --exact`
  - Run: `cargo test -p opentake-core open_save_roundtrip_through_core_emits_lifecycle_events`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 15: generation-log-credit-migration (implementation-slice-f88c9f906af39752)

**Covered records:**
- `requirement-75831345b1183120` (requirement)

**Files:**
- Modify: `crates/opentake-project/src/gen_log.rs#GenerationLogEntry::deserialize`
- Modify: `docs/modules/opentake-project/gen-log.md`
- Test (existing-owned): `crates/opentake-project/src/gen_log.rs#legacy_cost_dollars_migrates_to_credits_ceil`
- Test (existing-owned): `crates/opentake-project/src/gen_log.rs#cost_credits_wins_over_legacy_cost`

**Candidate-bound contracts:**

#### requirement-75831345b1183120

- Candidate/source: `doc-b23dea9b66ca0eea` at `docs/modules/opentake-project/gen-log.md:28` (requirement)
- Expected behavior: At docs/modules/opentake-project/gen-log.md:28 under “关键算法 / 容错（必须 1:1 复刻）” (gap-marker), the source “- **美元 → credits 迁移**（手写 `Deserialize`）— 当 `costCredits` 缺失但旧字段 `cost`（美元 float）在场时：`cost_credits = ceil(cost * 100)`（Swift `(dollars*100).rounded(.up)`，**向上取整，永不截断**，如 `0.005 USD → 1`）。`costCredits` 与 `cost` 同在时，**`costCredits` 优先**（上游仅在 `costCredits` 缺失时才看旧 `cost`）。” requires this exact behavior: Migrate legacy generation cost dollars to ceil(cost*100) credits while preferring an explicit costCredits field.
- Resolution: `reviewed-mapping-report:generation-log-credit-migration` — The exact migration and precedence rules have focused owned tests; ledger evidence closure remains.
- Exact acceptance contract:
  - Source binding: docs/modules/opentake-project/gen-log.md:28; signal=gap-marker; heading=关键算法 / 容错（必须 1:1 复刻）; candidate=- **美元 → credits 迁移**（手写 `Deserialize`）— 当 `costCredits` 缺失但旧字段 `cost`（美元 float）在场时：`cost_credits = ceil(cost * 100)`（Swift `(dollars*100).rounded(.up)`，**向上取整，永不截断**，如 `0.005 USD → 1`）。`costCredits` 与 `cost` 同在时，**`costCredits` 优先**（上游仅在 `costCredits` 缺失时才看旧 `cost`）。
  - Expected behavior: Migrate legacy generation cost dollars to ceil(cost*100) credits while preferring an explicit costCredits field. This closes only the promise expressed by “**美元 → credits 迁移**（手写 `Deserialize`）— 当 `costCredits` 缺失但旧字段 `cost`（美元 float）在场时：`cost_credits = ceil(cost * 100)`（Swift `(dollars*100).rounded(.up)`，**向上取整，永不截断**，如 `0.005 USD → 1`）。`costCredits` 与 `cost` 同在时，**`costCredits` 优先**（上游仅在 `costCredits` 缺失时才看旧 `cost`）。” in “关键算法 / 容错（必须 1:1 复刻）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “**美元 → credits 迁移**（手写 `Deserialize`）— 当 `costCredits` 缺失但旧字段 `cost`（美元 float）在场时：`cost_credits = ceil(cost * 100)`（Swift `(dollars*100).rounded(.up)`，**向上取整，永不截断**，如 `0.005 USD → 1`）。`costCredits` 与 `cost` 同在时，**`costCredits` 优先**（上游仅在 `costCredits` 缺失时才看旧 `cost`）。” with the scenario below and register test:crates/opentake-agent/tests/completion_b23dea9b66ca0eea.rs#completion_b23dea9b66ca0eea_migrate_legacy_generation_cost_dollars_to_ceil_c
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “**美元 → credits 迁移**（手写 `Deserialize`）— 当 `costCredits` 缺失但旧字段 `cost`（美元 float）在场时：`cost_credits = ceil(cost * 100)`（Swift `(dollars*100).rounded(.up)`，**向上取整，永不截断**，如 `0.005 USD → 1`）。`costCredits` 与 `cost` 同在时，**`costCredits` 优先**（上游仅在 `costCredits` 缺失时才看旧 `cost`）。”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “Migrate legacy generation cost dollars to ceil(cost*100) credits while preferring an explicit costCredits field.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_b23dea9b66ca0eea.rs#completion_b23dea9b66ca0eea_migrate_legacy_generation_cost_dollars_to_ceil_c.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-project/src/gen_log.rs#legacy_cost_dollars_migrates_to_credits_ceil` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-project/src/gen_log.rs#cost_credits_wins_over_legacy_cost` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-project legacy_cost_dollars_migrates_to_credits_ceil`
  - Run: `cargo test -p opentake-project cost_credits_wins_over_legacy_cost`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-project/src/gen_log.rs#GenerationLogEntry::deserialize`, `docs/modules/opentake-project/gen-log.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-project legacy_cost_dollars_migrates_to_credits_ceil`
  - Run: `cargo test -p opentake-project cost_credits_wins_over_legacy_cost`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 16: mcp-http-contract (implementation-slice-0a878dfb9ad0c7cb)

**Covered records:**
- `requirement-bee010fba8fcee16` (requirement)
- `requirement-b7f7ca79bcc38f15` (requirement)
- `requirement-083a4a1cb0deaffa` (requirement)
- `requirement-bd989e8bc74ad42f` (requirement)
- `requirement-f15de2f06b50010c` (requirement)
- `requirement-eedd77086273df30` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/server.rs#McpServer`
- Modify: `crates/opentake-agent/src/mcp/server.rs#build_router_with_bridge`
- Modify: `crates/opentake-agent/src/mcp/server.rs#serve_with_bridge`
- Modify: `crates/opentake-agent/src/mcp/server.rs#localhost_guard`
- Modify: `src-tauri/src/mcp.rs#spawn`
- Modify: `docs/specs/agent/1-mcp-server.md`
- Test (existing-owned): `crates/opentake-agent/tests/mcp_http.rs#initialize_handshake_advertises_server_and_instructions`
- Test (existing-owned): `crates/opentake-agent/tests/mcp_http.rs#non_local_origin_is_rejected`
- Test (existing-owned): `crates/opentake-agent/tests/mcp_http.rs#oversized_request_body_is_rejected`
- Test (reviewed-planned): `crates/opentake-agent/tests/mcp_contract.rs#mcp_http_transport_schema_and_redaction_contract`

**Candidate-bound contracts:**

#### requirement-bee010fba8fcee16

- Candidate/source: `doc-033081a1b6f3a61d` at `docs/specs/agent/1-mcp-server.md:1` (requirement)
- Expected behavior: At docs/specs/agent/1-mcp-server.md:1, the source “# rmcp MCP server：`127.0.0.1:19789` + loopback/Origin 校验（tower layer）” requires this exact behavior: The complete MCP server contract is loopback-only and stateless per connection, validates Origin/Host, Content-Type, and MCP protocol version, exposes the specified HTTP routes, and publishes the exact metadata, resources, and capabilities.
- Resolution: `reviewed-mapping-report:mcp-http-contract` — Loopback HTTP and basic guards exist, but stdio, exact resources, content-type and protocol guards, enabled preference, and metadata acceptance are incomplete.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/1-mcp-server.md:1; signal=heading; heading=rmcp MCP server：`127.0.0.1:19789` + loopback/Origin 校验（tower layer）; candidate=# rmcp MCP server：`127.0.0.1:19789` + loopback/Origin 校验（tower layer）
  - Expected behavior: The complete MCP server contract is loopback-only and stateless per connection, validates Origin/Host, Content-Type, and MCP protocol version, exposes the specified HTTP routes, and publishes the exact metadata, resources, and capabilities.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_1_line_1_033081a1b6f3a61d.rs#spec_agent_1_line_1_033081a1b6f3a61d_mcp_server_total_contract
  - Initial state/input/event: start the enabled server with a fake CoreHandle, issue a new pair of requests on two independent connections, and exercise POST/GET /mcp, GET /, the well-known resource route, an unknown route, forged Origin/Host, wrong Content-Type, and unsupported protocol version.
  - Code/store/API/Rust effect: bind no address except 127.0.0.1, allocate stateless rmcp service state per connection, run every guard before dispatch, and route only the documented endpoints without project or timeline mutation for rejected traffic.
  - Visible/returned assertion: prove no LAN listener exists; assert JSON-RPC/SSE/well-known/404 responses, 403/415/400 guard statuses, distinct connection state, and exact server capabilities.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_1_line_1_033081a1b6f3a61d.rs#spec_agent_1_line_1_033081a1b6f3a61d_mcp_server_total_contract.

#### requirement-b7f7ca79bcc38f15

- Candidate/source: `doc-fa5307d13cfbd251` at `docs/specs/agent/1-mcp-server.md:26` (requirement)
- Expected behavior: At docs/specs/agent/1-mcp-server.md:26, the source “## 1.2 OpenTake Rust 设计（rmcp + axum + tower）” requires this exact behavior: Assemble the OpenTake Rust server with rmcp StreamableHttpService nested at /mcp, an axum Router, tower guards, and the well-known route, without a custom TCP transport shell.
- Resolution: `reviewed-mapping-report:mcp-http-contract` — Loopback HTTP and basic guards exist, but stdio, exact resources, content-type and protocol guards, enabled preference, and metadata acceptance are incomplete.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/1-mcp-server.md:26; signal=heading; heading=1.2 OpenTake Rust 设计（rmcp + axum + tower）; candidate=## 1.2 OpenTake Rust 设计（rmcp + axum + tower）
  - Expected behavior: Assemble the OpenTake Rust server with rmcp StreamableHttpService nested at /mcp, an axum Router, tower guards, and the well-known route, without a custom TCP transport shell.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_1_line_26_fa5307d13cfbd251.rs#spec_agent_1_line_26_fa5307d13cfbd251_rmcp_axum_tower_assembly
  - Initial state/input/event: construct the rmcp + axum application from a fake core and bind an ephemeral loopback listener, then inspect the Service stack and send initialize, /mcp, and /.well-known/oauth-protected-resource requests.
  - Code/store/API/Rust effect: compose StreamableHttpService, axum, and tower in the documented order, clone only the intended shared handles into stateless ToolServer instances, and keep transport assembly independent from editing state.
  - Visible/returned assertion: assert initialize reaches rmcp, /mcp is served, the well-known JSON points to the loopback resource, every tower layer executes, and no handwritten or non-loopback TCP path is used.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_1_line_26_fa5307d13cfbd251.rs#spec_agent_1_line_26_fa5307d13cfbd251_rmcp_axum_tower_assembly.

#### requirement-083a4a1cb0deaffa

- Candidate/source: `doc-9bd256cf84a82d70` at `docs/specs/agent/1-mcp-server.md:47` (requirement)
- Expected behavior: At docs/specs/agent/1-mcp-server.md:47, the source “### 1.2.1 绑定 + 幂等开关（照搬上游语义）” requires this exact behavior: Honor the io.opentake.mcp.enabled preference with default enabled semantics and idempotent startup while binding exactly 127.0.0.1:19789 and never 0.0.0.0.
- Resolution: `reviewed-mapping-report:mcp-http-contract` — Loopback HTTP and basic guards exist, but stdio, exact resources, content-type and protocol guards, enabled preference, and metadata acceptance are incomplete.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/1-mcp-server.md:47; signal=heading; heading=1.2.1 绑定 + 幂等开关（照搬上游语义）; candidate=### 1.2.1 绑定 + 幂等开关（照搬上游语义）
  - Expected behavior: Honor the io.opentake.mcp.enabled preference with default enabled semantics and idempotent startup while binding exactly 127.0.0.1:19789 and never 0.0.0.0.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_1_line_47_9bd256cf84a82d70.rs#spec_agent_1_line_47_9bd256cf84a82d70_loopback_preference_and_idempotent_start
  - Initial state/input/event: run startup with the preference missing, false, and true, then call startup twice while the first listener is active and inspect the bound SocketAddr.
  - Code/store/API/Rust effect: default a missing preference to enabled, skip all binding when disabled, return the existing running handle on repeated start, and create at most one listener at 127.0.0.1:19789.
  - Visible/returned assertion: assert default enabled starts once, disabled starts zero times, repeated calls are idempotent, the address is 127.0.0.1:19789, and 0.0.0.0 is never attempted.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_1_line_47_9bd256cf84a82d70.rs#spec_agent_1_line_47_9bd256cf84a82d70_loopback_preference_and_idempotent_start.

#### requirement-bd989e8bc74ad42f

- Candidate/source: `doc-1aff0e302c62005f` at `docs/specs/agent/1-mcp-server.md:73` (requirement)
- Expected behavior: At docs/specs/agent/1-mcp-server.md:73, the source “### 1.2.2 三个 tower layer（**必须保留，DNS-rebinding 防护**）” requires this exact behavior: Apply all three tower guards: allowed localhost Origin/Host pairs pass, forged or wrong-port values return 403, non-application/json POST requests return 415, and unsupported MCP protocol versions return 400.
- Resolution: `reviewed-mapping-report:mcp-http-contract` — Loopback HTTP and basic guards exist, but stdio, exact resources, content-type and protocol guards, enabled preference, and metadata acceptance are incomplete.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/1-mcp-server.md:73; signal=heading; heading=1.2.2 三个 tower layer（**必须保留，DNS-rebinding 防护**）; candidate=### 1.2.2 三个 tower layer（**必须保留，DNS-rebinding 防护**）
  - Expected behavior: Apply all three tower guards: allowed localhost Origin/Host pairs pass, forged or wrong-port values return 403, non-application/json POST requests return 415, and unsupported MCP protocol versions return 400.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_1_line_73_1aff0e302c62005f.rs#spec_agent_1_line_73_1aff0e302c62005f_origin_content_type_protocol_guard_matrix
  - Initial state/input/event: table-drive absent Origin, localhost/127.0.0.1/[::1] with port 19789, evil or wrong-port Origin/Host, JSON and non-JSON POST bodies, and missing/supported/unsupported MCP-Protocol-Version headers.
  - Code/store/API/Rust effect: evaluate Origin and Host before dispatch, evaluate Content-Type only for POST /mcp, then validate the protocol version against rmcp support; rejected requests must not invoke ToolServer.
  - Visible/returned assertion: assert allowed local requests reach dispatch, absent Origin is accepted, forged Origin/Host returns 403, wrong Content-Type returns 415, unsupported protocol returns 400, and dispatch count remains zero for every rejection.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_1_line_73_1aff0e302c62005f.rs#spec_agent_1_line_73_1aff0e302c62005f_origin_content_type_protocol_guard_matrix.

#### requirement-f15de2f06b50010c

- Candidate/source: `doc-3b9a7571194c1a70` at `docs/specs/agent/1-mcp-server.md:86` (requirement)
- Expected behavior: A maintained stdio-to-loopback-HTTP shim exposes the MCP server to stdio-only clients.
- Resolution: `reviewed-mapping-report:mcp-http-contract` — Loopback HTTP and basic guards exist, but stdio, exact resources, content-type and protocol guards, enabled preference, and metadata acceptance are incomplete.
- Exact acceptance contract:
  - Implement the stdio shim with lifecycle and error propagation.
  - Document the launch/config contract.
  - Add JSON-RPC transport and shutdown integration tests.

#### requirement-eedd77086273df30

- Candidate/source: `doc-f8f3c1f8df2f32e5` at `docs/specs/agent/1-mcp-server.md:90` (requirement)
- Expected behavior: At docs/specs/agent/1-mcp-server.md:90, the source “## 1.3 server 元数据 + 能力（照搬 `MCPService.start()`）” requires this exact behavior: Publish server name opentake, version 1.0.0, assembled instructions, non-list-changing resource/tool capabilities, handlers for list/call/read, and exactly the read-only opentake://models/video and opentake://models/image resources.
- Resolution: `reviewed-mapping-report:mcp-http-contract` — Loopback HTTP and basic guards exist, but stdio, exact resources, content-type and protocol guards, enabled preference, and metadata acceptance are incomplete.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/1-mcp-server.md:90; signal=heading; heading=1.3 server 元数据 + 能力（照搬 `MCPService.start()`）; candidate=## 1.3 server 元数据 + 能力（照搬 `MCPService.start()`）
  - Expected behavior: Publish server name opentake, version 1.0.0, assembled instructions, non-list-changing resource/tool capabilities, handlers for list/call/read, and exactly the read-only opentake://models/video and opentake://models/image resources.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_1_line_90_f8f3c1f8df2f32e5.rs#spec_agent_1_line_90_f8f3c1f8df2f32e5_server_metadata_capabilities_and_resources
  - Initial state/input/event: initialize a ToolServer with fixed assembled instructions and a deterministic ModelCatalog, then call initialize, ListTools, CallTool, ListResources, and ReadResource for both model URIs and an unknown URI.
  - Code/store/API/Rust effect: return the exact name, version, instructions, and capabilities; expose only opentake://models/video and opentake://models/image as read-only catalog resources and route tool/resource handlers without mutation on reads.
  - Visible/returned assertion: assert exact initialize metadata, tools.listChanged=false, resources subscribe/listChanged=false, both model JSON payloads, and a typed not-found result for any other resource URI.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_1_line_90_f8f3c1f8df2f32e5.rs#spec_agent_1_line_90_f8f3c1f8df2f32e5_server_metadata_capabilities_and_resources.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/mcp_http.rs#initialize_handshake_advertises_server_and_instructions` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#non_local_origin_is_rejected` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_http.rs#oversized_request_body_is_rejected` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/mcp_contract.rs#mcp_http_transport_schema_and_redaction_contract` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test mcp_http initialize_handshake_advertises_server_and_instructions -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_http non_local_origin_is_rejected -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_http oversized_request_body_is_rejected -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_contract mcp_http_transport_schema_and_redaction_contract -- --exact`

  Expected: FAIL because one or more of the 6 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/server.rs#McpServer`, `crates/opentake-agent/src/mcp/server.rs#build_router_with_bridge`, `crates/opentake-agent/src/mcp/server.rs#serve_with_bridge`, `crates/opentake-agent/src/mcp/server.rs#localhost_guard`, `src-tauri/src/mcp.rs#spawn`, `docs/specs/agent/1-mcp-server.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test mcp_http initialize_handshake_advertises_server_and_instructions -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_http non_local_origin_is_rejected -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_http oversized_request_body_is_rejected -- --exact`
  - Run: `cargo test -p opentake-agent --test mcp_contract mcp_http_transport_schema_and_redaction_contract -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 17: agent-crate-tauri-assembly (implementation-slice-3e263d0acda0e51d)

**Covered records:**
- `requirement-c352f8fc7a62f904` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/lib.rs`
- Modify: `src-tauri/src/mcp.rs#spawn`
- Modify: `src-tauri/src/chat.rs#ChatState::new`
- Modify: `docs/specs/agent/10-implementation.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/module_tree.rs#documented_exports_compile`
- Test (reviewed-planned): `src-tauri/src/mcp.rs#documented_mcp_entrypoint_compiles`
- Test (reviewed-planned): `src-tauri/src/chat.rs#documented_chat_entrypoint_compiles`

**Candidate-bound contracts:**

#### requirement-c352f8fc7a62f904

- Candidate/source: `doc-2bf44733e7defdfd` at `docs/specs/agent/10-implementation.md:5` (requirement)
- Expected behavior: At docs/specs/agent/10-implementation.md:5, the source “## 9.1 crate 骨架（`crates/opentake-agent/`）” requires this exact behavior: Assemble the documented opentake-agent crate module tree, expose the required lib.rs exports, register it in the Cargo workspace, and wire the desktop shell/Tauri integration without missing modules.
- Resolution: `reviewed-mapping-report:agent-crate-tauri-assembly` — The crate exports and Tauri imports are present; a narrow structural compile/export gate is needed before evidence closure.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/10-implementation.md:5; signal=heading; heading=9.1 crate 骨架（`crates/opentake-agent/`）; candidate=## 9.1 crate 骨架（`crates/opentake-agent/`）
  - Expected behavior: Assemble the documented opentake-agent crate module tree, expose the required lib.rs exports, register it in the Cargo workspace, and wire the desktop shell/Tauri integration without missing modules.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_10_line_5_2bf44733e7defdfd.rs#spec_agent_10_line_5_2bf44733e7defdfd_agent_crate_workspace_exports_and_tauri_wiring
  - Initial state/input/event: load Cargo metadata and compile an integration fixture that imports start_mcp_server, AgentService, and ToolExecutor, then instantiate the desktop shell/Tauri agent wiring with fake dependencies.
  - Code/store/API/Rust effect: make every documented mcp/tools/chat/signal/plugin/prompt module reachable from the crate, keep lib.rs exports typed, include the crate in the workspace, and connect startup through the desktop shell.
  - Visible/returned assertion: assert Cargo metadata membership, successful imports and compilation, exact lib.rs exports, complete module-tree inventory, and one Tauri desktop startup path with no duplicate server.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_10_line_5_2bf44733e7defdfd.rs#spec_agent_10_line_5_2bf44733e7defdfd_agent_crate_workspace_exports_and_tauri_wiring.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/module_tree.rs#documented_exports_compile` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `src-tauri/src/mcp.rs#documented_mcp_entrypoint_compiles` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `src-tauri/src/chat.rs#documented_chat_entrypoint_compiles` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test module_tree documented_exports_compile -- --exact`
  - Run: `cargo test -p opentake-tauri documented_mcp_entrypoint_compiles`
  - Run: `cargo test -p opentake-tauri documented_chat_entrypoint_compiles`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/lib.rs`, `src-tauri/src/mcp.rs#spawn`, `src-tauri/src/chat.rs#ChatState::new`, `docs/specs/agent/10-implementation.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test module_tree documented_exports_compile -- --exact`
  - Run: `cargo test -p opentake-tauri documented_mcp_entrypoint_compiles`
  - Run: `cargo test -p opentake-tauri documented_chat_entrypoint_compiles`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 18: AG-typed-validation-contract (implementation-slice-efb8e52569008037)

**Covered records:**
- `requirement-8bd5296ed16bec1c` (requirement)
- `requirement-e6ff1d4ee0e4f11f` (requirement)
- `requirement-7bd485ebf0ccb154` (requirement)
- `requirement-7986c169e48b3f56` (requirement)
- `requirement-df3cc673479ddc0b` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/tools/errors.rs#decode_tool_args`
- Modify: `crates/opentake-agent/src/tools/errors.rs#validate_unknown_keys`
- Modify: `crates/opentake-agent/src/tools/errors.rs#ToolArgs`
- Modify: `docs/specs/agent/10-implementation.md`
- Modify: `docs/specs/agent/4-execution-shell.md`
- Test (existing-owned): `crates/opentake-agent/src/tools/errors.rs#unknown_field_lists_sorted_allowed`
- Test (existing-owned): `crates/opentake-agent/src/tools/errors.rs#nested_array_index_uses_brackets`
- Test (existing-owned): `crates/opentake-agent/src/tools/errors.rs#non_finite_number_rejected_with_path`
- Test (reviewed-planned): `crates/opentake-agent/tests/tool_argument_contract.rs#all_tool_schemas_reject_unknown_missing_wrong_type_and_nonfinite`

**Candidate-bound contracts:**

#### requirement-8bd5296ed16bec1c

- Candidate/source: `doc-2505e846ef691f9a` at `docs/specs/agent/10-implementation.md:51` (requirement)
- Expected behavior: At docs/specs/agent/10-implementation.md:51, the source “3. [ ] `tools/errors.rs` + `tools/args.rs`：`serde_path_to_error` 路径化 + `allowedKeys` 未知字段拒绝 + 非有限数拒绝。— 验证：构造 `entries[3].startFrame` 缺失/类型错/未知字段/NaN，输出措辞与 §4.2 一致。” requires this exact behavior: Implement tools/errors.rs and tools/args.rs so serde_path_to_error, allowedKeys unknown-field rejection, non-finite detection, and exact §4.2 wording cover entries[3].startFrame missing/type/unknown/NaN cases.
- Resolution: `validated-ledger-evidence:AG-typed-validation-contract` — Three-layer validation and precise paths exist; exhaustive all-tool coverage is planned.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/10-implementation.md:51; signal=gap-marker; heading=9.2 任务清单（按依赖序）; candidate=3. [ ] `tools/errors.rs` + `tools/args.rs`：`serde_path_to_error` 路径化 + `allowedKeys` 未知字段拒绝 + 非有限数拒绝。— 验证：构造 `entries[3].startFrame` 缺失/类型错/未知字段/NaN，输出措辞与 §4.2 一致。
  - Expected behavior: Implement tools/errors.rs and tools/args.rs so serde_path_to_error, allowedKeys unknown-field rejection, non-finite detection, and exact §4.2 wording cover entries[3].startFrame missing/type/unknown/NaN cases.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_10_line_51_2505e846ef691f9a.rs#spec_agent_10_line_51_2505e846ef691f9a_tool_args_entries3_exact_error_matrix
  - Initial state/input/event: construct four add_clips payloads that make entries[3].startFrame respectively missing, wrong type, accompanied by an unknown nested key, and non-finite, plus one valid control.
  - Code/store/API/Rust effect: apply allowedKeys and non-finite checks before serde_path_to_error, normalize the nested path to entries[3].startFrame, return the exact wording, and never call the tool implementation for invalid payloads.
  - Visible/returned assertion: assert distinct missing/type/unknown/value-must-be-finite messages with exact wording and allowed lists, the valid decode result, and zero timeline mutation for all four failures.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_10_line_51_2505e846ef691f9a.rs#spec_agent_10_line_51_2505e846ef691f9a_tool_args_entries3_exact_error_matrix.

#### requirement-e6ff1d4ee0e4f11f

- Candidate/source: `doc-f56371d6a088cb14` at `docs/specs/agent/4-execution-shell.md:31` (requirement)
- Expected behavior: At docs/specs/agent/4-execution-shell.md:31, the source “## 4.2 严格输入校验（三层，**面向 LLM 的错误工程**）” requires this exact behavior: Enforce the three-layer validation order: reject unknown fields, then locate the first non-finite number, then perform serde path decoding, with precise LLM-facing errors and no dispatch on failure.
- Resolution: `validated-ledger-evidence:AG-typed-validation-contract` — Three-layer validation and precise paths exist; exhaustive all-tool coverage is planned.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/4-execution-shell.md:31; signal=heading; heading=4.2 严格输入校验（三层，**面向 LLM 的错误工程**）; candidate=## 4.2 严格输入校验（三层，**面向 LLM 的错误工程**）
  - Expected behavior: Enforce the three-layer validation order: reject unknown fields, then locate the first non-finite number, then perform serde path decoding, with precise LLM-facing errors and no dispatch on failure.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_4_line_31_f56371d6a088cb14.rs#spec_agent_4_line_31_f56371d6a088cb14_three_layer_validation_order
  - Initial state/input/event: submit the same nested tool payload as valid, with an unknown field, with NaN/Infinity after unknown-field cleanup, and with a serde type/missing-field error while recording which validation layer runs.
  - Code/store/API/Rust effect: run unknown-fields validation before the first non-finite scan and serde_path_to_error decoding, stop at the first failing layer, and never invoke the edit command for rejected input.
  - Visible/returned assertion: assert validation order, one exact error per payload, normalized nested paths, zero mutation/dispatch, and successful decode only after all three layers pass.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_4_line_31_f56371d6a088cb14.rs#spec_agent_4_line_31_f56371d6a088cb14_three_layer_validation_order.

#### requirement-7bd485ebf0ccb154

- Candidate/source: `doc-4ea195544ead0961` at `docs/specs/agent/4-execution-shell.md:33` (requirement)
- Expected behavior: At docs/specs/agent/4-execution-shell.md:33, the source “### 4.2.1 未知字段拒绝（`validateUnknownKeys:166-171`）” requires this exact behavior: Reject unknown fields at every top-level and nested object path, report sorted allowed keys, and identify array members such as entries[3] without silently dropping data.
- Resolution: `validated-ledger-evidence:AG-typed-validation-contract` — Three-layer validation and precise paths exist; exhaustive all-tool coverage is planned.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/4-execution-shell.md:33; signal=heading; heading=4.2.1 未知字段拒绝（`validateUnknownKeys:166-171`）; candidate=### 4.2.1 未知字段拒绝（`validateUnknownKeys:166-171`）
  - Expected behavior: Reject unknown fields at every top-level and nested object path, report sorted allowed keys, and identify array members such as entries[3] without silently dropping data.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_4_line_33_4ea195544ead0961.rs#spec_agent_4_line_33_4ea195544ead0961_unknown_fields_nested_paths_and_allowed_list
  - Initial state/input/event: start from valid add_clips arguments, inject different unknown keys at the root and entries[3], vary insertion order, and provide the exact allowed-key list for each object type.
  - Code/store/API/Rust effect: compute keys minus allowedKeys independently at each nested object, sort unknown and allowed names for stable output, return before deserialization or command dispatch, and perform no mutation.
  - Visible/returned assertion: assert errors name entries[3], every unknown field, and the sorted Allowed list exactly; valid subsets pass and rejected payloads leave timeline and undo state unchanged.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_4_line_33_4ea195544ead0961.rs#spec_agent_4_line_33_4ea195544ead0961_unknown_fields_nested_paths_and_allowed_list.

#### requirement-7986c169e48b3f56

- Candidate/source: `doc-f9a68506ad6b2ac5` at `docs/specs/agent/4-execution-shell.md:42` (requirement)
- Expected behavior: At docs/specs/agent/4-execution-shell.md:42, the source “### 4.2.2 非有限数拒绝（`firstNonFiniteNumberPath:194-208`）” requires this exact behavior: Recursively return the first non-finite numeric path across arrays and objects using bracket indices and emit '<path>: value must be finite' for NaN or infinity.
- Resolution: `validated-ledger-evidence:AG-typed-validation-contract` — Three-layer validation and precise paths exist; exhaustive all-tool coverage is planned.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/4-execution-shell.md:42; signal=heading; heading=4.2.2 非有限数拒绝（`firstNonFiniteNumberPath:194-208`）; candidate=### 4.2.2 非有限数拒绝（`firstNonFiniteNumberPath:194-208`）
  - Expected behavior: Recursively return the first non-finite numeric path across arrays and objects using bracket indices and emit '<path>: value must be finite' for NaN or infinity.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_4_line_42_f9a68506ad6b2ac5.rs#spec_agent_4_line_42_f9a68506ad6b2ac5_first_non_finite_array_object_path
  - Initial state/input/event: construct deterministic nested objects and arrays containing multiple finite values followed by NaN, positive infinity, and negative infinity at distinct paths, plus an all-finite control payload.
  - Code/store/API/Rust effect: walk arrays in index order and objects in deterministic key order, stop at the first non-finite value, normalize array positions with brackets, and do not deserialize or dispatch after a hit.
  - Visible/returned assertion: assert the first non-finite path exactly, including entries[3].startFrame, the exact 'value must be finite' text, None for all-finite input, and zero edit side effects.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_4_line_42_f9a68506ad6b2ac5.rs#spec_agent_4_line_42_f9a68506ad6b2ac5_first_non_finite_array_object_path.

#### requirement-df3cc673479ddc0b

- Candidate/source: `doc-5d932c51ced061d6` at `docs/specs/agent/4-execution-shell.md:55` (requirement)
- Expected behavior: At docs/specs/agent/4-execution-shell.md:55, the source “### 4.2.3 路径化解码错误（`formatDecodingError:210-229` + `decodeToolArgs:177`）” requires this exact behavior: Map serde_path_to_error failures into the four precise categories keyNotFound, typeMismatch, valueNotFound, and dataCorrupted while normalizing numeric path segments to bracket indices.
- Resolution: `validated-ledger-evidence:AG-typed-validation-contract` — Three-layer validation and precise paths exist; exhaustive all-tool coverage is planned.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/4-execution-shell.md:55; signal=heading; heading=4.2.3 路径化解码错误（`formatDecodingError:210-229` + `decodeToolArgs:177`）; candidate=### 4.2.3 路径化解码错误（`formatDecodingError:210-229` + `decodeToolArgs:177`）
  - Expected behavior: Map serde_path_to_error failures into the four precise categories keyNotFound, typeMismatch, valueNotFound, and dataCorrupted while normalizing numeric path segments to bracket indices.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_4_line_55_5d932c51ced061d6.rs#spec_agent_4_line_55_5d932c51ced061d6_serde_error_categories_and_bracket_indices
  - Initial state/input/event: decode fixtures that independently trigger keyNotFound, typeMismatch, valueNotFound, and dataCorrupted at root and nested entries.3.startFrame paths.
  - Code/store/API/Rust effect: capture the serde path, convert entries.3.startFrame to entries[3].startFrame, classify the underlying error without losing its expected type or missing key, and return ToolError before dispatch.
  - Visible/returned assertion: assert exact category-specific wording and entries[3].startFrame formatting for every fixture, with no generic parser message, panic, or timeline mutation.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_4_line_55_5d932c51ced061d6.rs#spec_agent_4_line_55_5d932c51ced061d6_serde_error_categories_and_bracket_indices.

- [x] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/tools/errors.rs#unknown_field_lists_sorted_allowed` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/tools/errors.rs#nested_array_index_uses_brackets` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/tools/errors.rs#non_finite_number_rejected_with_path` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/tool_argument_contract.rs#all_tool_schemas_reject_unknown_missing_wrong_type_and_nonfinite` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent unknown_field_lists_sorted_allowed`
  - Run: `cargo test -p opentake-agent nested_array_index_uses_brackets`
  - Run: `cargo test -p opentake-agent non_finite_number_rejected_with_path`
  - Run: `cargo test -p opentake-agent --test tool_argument_contract all_tool_schemas_reject_unknown_missing_wrong_type_and_nonfinite -- --exact`

  Expected: FAIL because one or more of the 5 candidate-bound contracts are not yet satisfied.

  Historical note (2026-07-29): the missing exact-owned test was discovered by the completion audit, but a complete pre-fix RED transcript for all four focused commands was not retained. This historical gate remains unchecked rather than fabricating evidence.

- [x] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/tools/errors.rs#decode_tool_args`, `crates/opentake-agent/src/tools/errors.rs#validate_unknown_keys`, `crates/opentake-agent/src/tools/errors.rs#ToolArgs`, `docs/specs/agent/10-implementation.md`, `docs/specs/agent/4-execution-shell.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

  Rust boundary note (2026-07-29): standard `serde_json` rejects or loses the precise path for raw `NaN`/`Infinity` and exponent overflow before `decode_tool_args`. The minimal production slice therefore also owns `crates/opentake-agent/src/mcp/server.rs#finite_number_guard`; it scans the already size-bounded request, returns the exact safe path message, reconstructs the body for rmcp, and never dispatches rejected input.

- [x] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent unknown_field_lists_sorted_allowed`
  - Run: `cargo test -p opentake-agent nested_array_index_uses_brackets`
  - Run: `cargo test -p opentake-agent non_finite_number_rejected_with_path`
  - Run: `cargo test -p opentake-agent --test tool_argument_contract all_tool_schemas_reject_unknown_missing_wrong_type_and_nonfinite -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [x] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

  Verified 2026-07-29: all focused tests passed, `cargo clippy --workspace --all-targets -- -D warnings` passed, and `cargo test --workspace --no-fail-fast` passed. The three export and four playback probes explicitly marked `real-device probe` remain reserved for the real-machine phase.

### Task 19: AG-timeline-tool-schema-dispatch (implementation-slice-cb53b36cf984d605)

**Covered records:**
- `requirement-5518fcd7072d33e0` (requirement)
- `requirement-8ab9851e19f6570e` (requirement)
- `requirement-ef7ed3316ed7d320` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/tools/descriptions.rs#input_schema`
- Modify: `crates/opentake-agent/src/tools/errors.rs#ToolArgs`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#run_body`
- Modify: `docs/specs/agent/2-tools.md`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#add_clips_then_get_timeline_reflects_clip`
- Test (reviewed-planned): `crates/opentake-agent/tests/timeline_tool_contract.rs#all_timeline_schemas_descriptions_and_dispatch_paths_agree`

**Candidate-bound contracts:**

#### requirement-5518fcd7072d33e0

- Candidate/source: `doc-9436a28406cf13ab` at `docs/specs/agent/2-tools.md:23` (requirement)
- Expected behavior: At docs/specs/agent/2-tools.md:23 under “B. 时间线编辑（写，11 个）——核心剪辑能力” (heading), the source “### B. 时间线编辑（写，11 个）——核心剪辑能力” requires this exact behavior: The timeline tool schemas/descriptions dispatch through the shared edit layer.
- Resolution: `validated-ledger-evidence:AG-timeline-tool-schema-dispatch` — Three headings describe one exhaustive schema-description-dispatch parity gate.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/2-tools.md:23; signal=heading; heading=B. 时间线编辑（写，11 个）——核心剪辑能力; candidate=### B. 时间线编辑（写，11 个）——核心剪辑能力
  - Expected behavior: The timeline tool schemas/descriptions dispatch through the shared edit layer. This closes only the promise expressed by “B. 时间线编辑（写，11 个）——核心剪辑能力” in “B. 时间线编辑（写，11 个）——核心剪辑能力”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “B. 时间线编辑（写，11 个）——核心剪辑能力” with the scenario below and register test:web/src/__tests__/completion/doc-9436a28406cf13ab.test.ts#completion_9436a28406cf13ab_the_timeline_tool_schemas_descriptions_dispatch_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “B. 时间线编辑（写，11 个）——核心剪辑能力” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The timeline tool schemas/descriptions dispatch through the shared edit layer.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-9436a28406cf13ab.test.ts#completion_9436a28406cf13ab_the_timeline_tool_schemas_descriptions_dispatch_.

#### requirement-8ab9851e19f6570e

- Candidate/source: `doc-1e143fdf8f2f7775` at `docs/specs/agent/2-tools.md:62` (requirement)
- Expected behavior: At docs/specs/agent/2-tools.md:62 under “2.2 工具描述完整 JSON（供实现直接复制 `description` 字符串）” (heading), the source “## 2.2 工具描述完整 JSON（供实现直接复制 `description` 字符串）” requires this exact behavior: The timeline tool schemas/descriptions dispatch through the shared edit layer.
- Resolution: `validated-ledger-evidence:AG-timeline-tool-schema-dispatch` — Three headings describe one exhaustive schema-description-dispatch parity gate.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/2-tools.md:62; signal=heading; heading=2.2 工具描述完整 JSON（供实现直接复制 `description` 字符串）; candidate=## 2.2 工具描述完整 JSON（供实现直接复制 `description` 字符串）
  - Expected behavior: The timeline tool schemas/descriptions dispatch through the shared edit layer. This closes only the promise expressed by “2.2 工具描述完整 JSON（供实现直接复制 `description` 字符串）” in “2.2 工具描述完整 JSON（供实现直接复制 `description` 字符串）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.2 工具描述完整 JSON（供实现直接复制 `description` 字符串）” with the scenario below and register test:web/src/__tests__/completion/doc-1e143fdf8f2f7775.test.ts#completion_1e143fdf8f2f7775_the_timeline_tool_schemas_descriptions_dispatch_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “2.2 工具描述完整 JSON（供实现直接复制 `description` 字符串）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The timeline tool schemas/descriptions dispatch through the shared edit layer.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-1e143fdf8f2f7775.test.ts#completion_1e143fdf8f2f7775_the_timeline_tool_schemas_descriptions_dispatch_.

#### requirement-ef7ed3316ed7d320

- Candidate/source: `doc-5669d50724107e54` at `docs/specs/agent/2-tools.md:104` (requirement)
- Expected behavior: At docs/specs/agent/2-tools.md:104 under “2.3 工具入参 Schema 策略（Rust）” (heading), the source “## 2.3 工具入参 Schema 策略（Rust）” requires this exact behavior: The timeline tool schemas/descriptions dispatch through the shared edit layer.
- Resolution: `validated-ledger-evidence:AG-timeline-tool-schema-dispatch` — Three headings describe one exhaustive schema-description-dispatch parity gate.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/2-tools.md:104; signal=heading; heading=2.3 工具入参 Schema 策略（Rust）; candidate=## 2.3 工具入参 Schema 策略（Rust）
  - Expected behavior: The timeline tool schemas/descriptions dispatch through the shared edit layer. This closes only the promise expressed by “2.3 工具入参 Schema 策略（Rust）” in “2.3 工具入参 Schema 策略（Rust）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “2.3 工具入参 Schema 策略（Rust）” with the scenario below and register test:web/src/__tests__/completion/doc-5669d50724107e54.test.ts#completion_5669d50724107e54_the_timeline_tool_schemas_descriptions_dispatch_
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “2.3 工具入参 Schema 策略（Rust）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The timeline tool schemas/descriptions dispatch through the shared edit layer.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-5669d50724107e54.test.ts#completion_5669d50724107e54_the_timeline_tool_schemas_descriptions_dispatch_.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/mcp/dispatch.rs#add_clips_then_get_timeline_reflects_clip` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/timeline_tool_contract.rs#all_timeline_schemas_descriptions_and_dispatch_paths_agree` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent add_clips_then_get_timeline_reflects_clip`
  - Run: `cargo test -p opentake-agent --test timeline_tool_contract all_timeline_schemas_descriptions_and_dispatch_paths_agree -- --exact`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/tools/descriptions.rs#input_schema`, `crates/opentake-agent/src/tools/errors.rs#ToolArgs`, `crates/opentake-agent/src/mcp/dispatch.rs#run_body`, `docs/specs/agent/2-tools.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent add_clips_then_get_timeline_reflects_clip`
  - Run: `cargo test -p opentake-agent --test timeline_tool_contract all_timeline_schemas_descriptions_and_dispatch_paths_agree -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 20: AG-execution-shell-and-agent-undo (implementation-slice-189b02991e933989)

**Covered records:**
- `requirement-e98c0a98c018e09e` (requirement)
- `requirement-18b2a6cbebd24929` (requirement)
- `requirement-eb1f9e319e4b374d` (requirement)
- `requirement-671e976cc6e99d91` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#dispatch`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher::agent_undo`
- Modify: `crates/opentake-agent/src/tools/result.rs#ToolResult`
- Modify: `crates/opentake-agent/src/mcp/convert.rs#to_call_tool_result`
- Modify: `crates/opentake-agent/src/telemetry.rs#ToolTelemetry`
- Modify: `docs/specs/agent/4-execution-shell.md`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#undo_with_empty_stack_errors`
- Test (existing-owned): `crates/opentake-agent/src/tools/result.rs#ok_and_error_shapes`
- Test (existing-owned): `crates/opentake-agent/src/mcp/convert.rs#image_block_maps_to_image_content`
- Test (reviewed-planned): `crates/opentake-agent/tests/execution_shell.rs#sequence_serialization_telemetry_conflict_safe_undo_and_no_panic`

**Candidate-bound contracts:**

#### requirement-e98c0a98c018e09e

- Candidate/source: `doc-5c6815d70785903d` at `docs/specs/agent/4-execution-shell.md:1` (requirement)
- Expected behavior: At docs/specs/agent/4-execution-shell.md:1, the source “# 统一执行壳 + 面向 LLM 的精确路径错误” requires this exact behavior: The total execution-shell contract performs the documented execution sequence, emits precise path errors, owns an assistant-only undo stack, returns neutral ToolResult blocks, serializes edits, and never propagates a panic to MCP.
- Resolution: `validated-ledger-evidence:AG-execution-shell-and-agent-undo` — The shell/result/undo stack exist; telemetry, serialization and conflict-safe undo remain composite acceptance gaps.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/4-execution-shell.md:1; signal=heading; heading=统一执行壳 + 面向 LLM 的精确路径错误; candidate=# 统一执行壳 + 面向 LLM 的精确路径错误
  - Expected behavior: The total execution-shell contract performs the documented execution sequence, emits precise path errors, owns an assistant-only undo stack, returns neutral ToolResult blocks, serializes edits, and never propagates a panic to MCP.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_4_line_1_5c6815d70785903d.rs#spec_agent_4_line_1_5c6815d70785903d_execution_shell_total_contract
  - Initial state/input/event: execute one successful mutating tool, one read-only tool, one validation failure, one dispatch failure, and one assistant undo against a serialized fake editor while recording snapshot, ID expansion, run, context signal, and ID shortening calls.
  - Code/store/API/Rust effect: preserve the ordered snapshot→expand→run→conditional undo accounting→context signal→shorten pipeline, convert every ToolError to ToolResult, and isolate assistant undo ownership from user edits.
  - Visible/returned assertion: assert the exact call order, path-aware error, timeline-change telemetry, undo-stack transition, neutral result blocks, and no panic or thrown MCP error.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_4_line_1_5c6815d70785903d.rs#spec_agent_4_line_1_5c6815d70785903d_execution_shell_total_contract.

#### requirement-18b2a6cbebd24929

- Candidate/source: `doc-56589fcfa30eae4f` at `docs/specs/agent/4-execution-shell.md:5` (requirement)
- Expected behavior: At docs/specs/agent/4-execution-shell.md:5, the source “## 4.1 执行壳（`execute`，逐步照搬 + Rust 化）” requires this exact behavior: Execute tools in the exact sequence ToolName parse, editor availability, before snapshot, telemetry start, ID expand, run, conditional assistant undo record, telemetry finish, context signal attach, ID shorten, and ToolResult return under serialized editor access.
- Resolution: `validated-ledger-evidence:AG-execution-shell-and-agent-undo` — The shell/result/undo stack exist; telemetry, serialization and conflict-safe undo remain composite acceptance gaps.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/4-execution-shell.md:5; signal=heading; heading=4.1 执行壳（`execute`，逐步照搬 + Rust 化）; candidate=## 4.1 执行壳（`execute`，逐步照搬 + Rust 化）
  - Expected behavior: Execute tools in the exact sequence ToolName parse, editor availability, before snapshot, telemetry start, ID expand, run, conditional assistant undo record, telemetry finish, context signal attach, ID shorten, and ToolResult return under serialized editor access.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_4_line_5_56589fcfa30eae4f.rs#spec_agent_4_line_5_56589fcfa30eae4f_execute_sequence_snapshot_undo_context_shorten
  - Initial state/input/event: instrument each execution-shell dependency and run unknown-tool, unavailable-editor, successful mutating, successful non-mutating, Undo, and ToolError cases with deterministic IDs and action names.
  - Code/store/API/Rust effect: call ToolName parsing before editor access; snapshot before expand/run; record undo only for a successful non-Undo timeline change; attach context signal before shorten; serialize the whole edit sequence; convert errors with no panic.
  - Visible/returned assertion: assert the complete ordered trace, correct timelineChanged and duration fields, exact session stack contents, shortened newly created IDs, and a returned ToolResult for every branch.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_4_line_5_56589fcfa30eae4f.rs#spec_agent_4_line_5_56589fcfa30eae4f_execute_sequence_snapshot_undo_context_shorten.

#### requirement-eb1f9e319e4b374d

- Candidate/source: `doc-c499a90ad04eee20` at `docs/specs/agent/4-execution-shell.md:97` (requirement)
- Expected behavior: At docs/specs/agent/4-execution-shell.md:97, the source “## 4.3 助手专属 undo（`undo:109-123`）” requires this exact behavior: Provide assistant-only undo using a per-session undo stack, never consume a user undo, and refuse when the current core undo action conflicts with the assistant-owned expected action.
- Resolution: `validated-ledger-evidence:AG-execution-shell-and-agent-undo` — The shell/result/undo stack exist; telemetry, serialization and conflict-safe undo remain composite acceptance gaps.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/4-execution-shell.md:97; signal=heading; heading=4.3 助手专属 undo（`undo:109-123`）; candidate=## 4.3 助手专属 undo（`undo:109-123`）
  - Expected behavior: Provide assistant-only undo using a per-session undo stack, never consume a user undo, and refuse when the current core undo action conflicts with the assistant-owned expected action.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_4_line_97_c499a90ad04eee20.rs#spec_agent_4_line_97_c499a90ad04eee20_assistant_only_undo_ownership_and_conflict
  - Initial state/input/event: exercise an empty session undo stack, unavailable core undo, matching assistant action, and a conflict where a user edit is the most recent core action after an assistant edit.
  - Code/store/API/Rust effect: push only successful assistant timeline changes, clear stale session state when core cannot undo, compare undo_action_name before calling core.undo, pop only after a matching undo, and leave every user undo untouched.
  - Visible/returned assertion: assert the exact no-assistant-edit, nothing-to-undo, conflict 'not undoing', and successful 'Undid' messages; prove the conflict preserves both timeline and user undo history.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_4_line_97_c499a90ad04eee20.rs#spec_agent_4_line_97_c499a90ad04eee20_assistant_only_undo_ownership_and_conflict.

#### requirement-671e976cc6e99d91

- Candidate/source: `doc-334bbd6f24e91560` at `docs/specs/agent/4-execution-shell.md:113` (requirement)
- Expected behavior: At docs/specs/agent/4-execution-shell.md:113, the source “## 4.4 中立结果类型（`ToolResult.swift`）” requires this exact behavior: Represent neutral ToolResult content as Text and Image blocks with is_error, convert them exactly to rmcp CallToolResult content/isError, and preserve the same blocks for chat consumers.
- Resolution: `validated-ledger-evidence:AG-execution-shell-and-agent-undo` — The shell/result/undo stack exist; telemetry, serialization and conflict-safe undo remain composite acceptance gaps.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/4-execution-shell.md:113; signal=heading; heading=4.4 中立结果类型（`ToolResult.swift`）; candidate=## 4.4 中立结果类型（`ToolResult.swift`）
  - Expected behavior: Represent neutral ToolResult content as Text and Image blocks with is_error, convert them exactly to rmcp CallToolResult content/isError, and preserve the same blocks for chat consumers.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_4_line_113_334bbd6f24e91560.rs#spec_agent_4_line_113_334bbd6f24e91560_toolresult_text_image_is_error_rmcp_conversion
  - Initial state/input/event: construct ToolResult::ok/error values containing text, structured-JSON text, and base64 image blocks with MIME types, then convert each to rmcp and read it through the chat path.
  - Code/store/API/Rust effect: map Text to Content::text, Image to Content::image data/mime_type, is_error=true to Some(true), success to None, and perform no editor or undo operation during conversion.
  - Visible/returned assertion: assert byte-identical text/image content, exact MIME/base64 data, is_error semantics, rmcp CallToolResult fields, and unchanged chat content ordering.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_4_line_113_334bbd6f24e91560.rs#spec_agent_4_line_113_334bbd6f24e91560_toolresult_text_image_is_error_rmcp_conversion.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/mcp/dispatch.rs#undo_with_empty_stack_errors` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/tools/result.rs#ok_and_error_shapes` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/convert.rs#image_block_maps_to_image_content` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/execution_shell.rs#sequence_serialization_telemetry_conflict_safe_undo_and_no_panic` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent undo_with_empty_stack_errors`
  - Run: `cargo test -p opentake-agent ok_and_error_shapes`
  - Run: `cargo test -p opentake-agent image_block_maps_to_image_content`
  - Run: `cargo test -p opentake-agent --test execution_shell sequence_serialization_telemetry_conflict_safe_undo_and_no_panic -- --exact`

  Expected: FAIL because one or more of the 4 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/dispatch.rs#dispatch`, `crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher::agent_undo`, `crates/opentake-agent/src/tools/result.rs#ToolResult`, `crates/opentake-agent/src/mcp/convert.rs#to_call_tool_result`, `crates/opentake-agent/src/telemetry.rs#ToolTelemetry`, `docs/specs/agent/4-execution-shell.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent undo_with_empty_stack_errors`
  - Run: `cargo test -p opentake-agent ok_and_error_shapes`
  - Run: `cargo test -p opentake-agent image_block_maps_to_image_content`
  - Run: `cargo test -p opentake-agent --test execution_shell sequence_serialization_telemetry_conflict_safe_undo_and_no_panic -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 21: AG-business-guard-matrix (implementation-slice-0c448211ecba486b)

**Covered records:**
- `requirement-0cde3f8b52e25e35` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#run_body`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Modify: `docs/specs/agent/4-execution-shell.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/business_guard_matrix.rs#every_tool_rejects_exact_invalid_bounds_types_and_mixed_track_policy_before_mutation`

**Candidate-bound contracts:**

#### requirement-0cde3f8b52e25e35

- Candidate/source: `doc-f51cc80cdb49bd52` at `docs/specs/agent/4-execution-shell.md:84` (requirement)
- Expected behavior: At docs/specs/agent/4-execution-shell.md:84, the source “### 4.2.4 业务级守卫（照搬上游逐工具检查，举证）” requires this exact behavior: Apply per-tool guards with exact error messages for missing entries, track range/type compatibility, duration/start/trim bounds, and mixed trackIndex policy before any edit mutation.
- Resolution: `validated-ledger-evidence:AG-business-guard-matrix` — Distributed guards need an exhaustive exact-message and zero-mutation negative matrix.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/4-execution-shell.md:84; signal=heading; heading=4.2.4 业务级守卫（照搬上游逐工具检查，举证）; candidate=### 4.2.4 业务级守卫（照搬上游逐工具检查，举证）
  - Expected behavior: Apply per-tool guards with exact error messages for missing entries, track range/type compatibility, duration/start/trim bounds, and mixed trackIndex policy before any edit mutation.
  - Deterministic test: test:crates/opentake-agent/tests/spec_agent_4_line_84_f51cc80cdb49bd52.rs#spec_agent_4_line_84_f51cc80cdb49bd52_per_tool_guards_exact_messages_no_mutation
  - Initial state/input/event: table-drive add_clips with empty entries, out-of-range track, incompatible asset type, durationFrames<1, negative start/trim frames, and a Mixed trackIndex payload, alongside one valid payload.
  - Code/store/API/Rust effect: evaluate each per-tool guard at the owning ops/core boundary, forward its exact error message through the execution shell, and commit no partial clip, track, or undo mutation on rejection.
  - Visible/returned assertion: assert every exact error message including 'Mixed trackIndex', the offending entries[index] and value, no mutation for each rejected case, and one atomic edit for the valid case.
  - Evidence required: record the owning code:<tracked-file>#<declared-symbol> and the passing test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/spec_agent_4_line_84_f51cc80cdb49bd52.rs#spec_agent_4_line_84_f51cc80cdb49bd52_per_tool_guards_exact_messages_no_mutation.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/business_guard_matrix.rs#every_tool_rejects_exact_invalid_bounds_types_and_mixed_track_policy_before_mutation` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test business_guard_matrix every_tool_rejects_exact_invalid_bounds_types_and_mixed_track_policy_before_mutation -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/dispatch.rs#run_body`, `crates/opentake-ops/src/command.rs#EditCommand`, `docs/specs/agent/4-execution-shell.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test business_guard_matrix every_tool_rejects_exact_invalid_bounds_types_and_mixed_track_policy_before_mutation -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 22: AG-chat-complete-contract + chat-provider-sse-tool-loop (implementation-slice-a0e2d80f45f3e8c7)

**Covered records:**
- `requirement-a1ba9949bb843f4c` (requirement)
- `requirement-59732d045552bc15` (requirement)
- `requirement-e4c34875f6507835` (requirement)
- `requirement-73340597a252e2d9` (requirement)
- `requirement-fa7d690795a9cf2b` (requirement)
- `requirement-27f77ddcca543305` (requirement)
- `requirement-72c7d14797feeb3d` (requirement)
- `requirement-e2b750e813e5adcb` (requirement)
- `requirement-34322dbbdfbe0cc7` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/chat/content.rs#ImageAndMentionBlocks`
- Modify: `crates/opentake-agent/src/chat/llm.rs#anthropic_body`
- Modify: `crates/opentake-agent/src/chat/llm.rs#drain_sse_frames`
- Modify: `crates/opentake-agent/src/chat/llm.rs#provider_from_choice`
- Modify: `crates/opentake-agent/src/chat/llm.rs#stream_chat`
- Modify: `crates/opentake-agent/src/chat/loop.rs#ChatLoop::run_turn`
- Modify: `crates/opentake-agent/src/chat/persistence.rs#ChatRepository`
- Modify: `crates/opentake-agent/src/chat/session.rs#ChatSession`
- Modify: `crates/opentake-agent/src/chat/usage.rs#UsageRecorder`
- Modify: `src-tauri/src/chat.rs#ChatState`
- Modify: `src-tauri/src/chat.rs#chat_cancel`
- Modify: `src-tauri/src/chat.rs#chat_history`
- Modify: `src-tauri/src/chat.rs#chat_send`
- Modify: `docs/specs/agent/5-chat.md`
- Test (existing-owned): `crates/opentake-agent/src/chat/llm.rs#anthropic_system_prompt_hoisted_top_level`
- Test (existing-owned): `crates/opentake-agent/src/chat/llm.rs#drain_sse_frames_handles_split_utf8_chunks`
- Test (reviewed-planned): `crates/opentake-agent/src/chat/loop.rs#orphan_tool_use_is_repaired_before_next_provider_round`
- Test (existing-owned): `crates/opentake-agent/src/chat/loop.rs#tool_round_persists_assistant_before_tool_results`
- Test (existing-owned): `crates/opentake-agent/src/chat/session.rs#session_round_trip`
- Test (reviewed-planned): `crates/opentake-agent/tests/chat_contract.rs#cache_usage_mentions_images_and_restart_persistence`

**Candidate-bound contracts:**

#### requirement-a1ba9949bb843f4c

- Candidate/source: `doc-4d03b0b6dc8c27d4` at `docs/specs/agent/5-chat.md:1` (requirement)
- Expected behavior: The complete chat specification, including cache, usage, mentions/images, and persistence, is implemented.
- Resolution: `validated-ledger-evidence:AG-chat-complete-contract` — The report's provider/SSE/tool-loop gap is the implemented core of the ledger's complete chat contract; both share chat LLM/Tauri owners and SSE tests, so one capability should aggregate remaining cache, usage and persistence gaps.
- Exact acceptance contract:
  - Complete prompt-cache request boundaries, privacy-safe usage logging, @mention/image context, and schema-versioned durable sessions.
  - Provider/tool failures and restarts must preserve a repairable conversation without credentials or partial phantom messages.
  - Pass provider request snapshots, SSE/tool-loop tests, mention/image validation, usage redaction, and restart/migration round trips.

#### requirement-59732d045552bc15

- Candidate/source: `doc-37cdd1870f1604a3` at `docs/specs/agent/5-chat.md:5` (requirement)
- Expected behavior: At docs/specs/agent/5-chat.md:5 under “5.1 模型与双通道（`selectClient:52-59`）” (heading), the source “## 5.1 模型与双通道（`selectClient:52-59`）” requires this exact behavior: Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair.
- Resolution: `reviewed-mapping-report:chat-provider-sse-tool-loop` — The report's provider/SSE/tool-loop gap is the implemented core of the ledger's complete chat contract; both share chat LLM/Tauri owners and SSE tests, so one capability should aggregate remaining cache, usage and persistence gaps.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/5-chat.md:5; signal=heading; heading=5.1 模型与双通道（`selectClient:52-59`）; candidate=## 5.1 模型与双通道（`selectClient:52-59`）
  - Expected behavior: Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair. This closes only the promise expressed by “5.1 模型与双通道（`selectClient:52-59`）” in “5.1 模型与双通道（`selectClient:52-59`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.1 模型与双通道（`selectClient:52-59`）” with the scenario below and register test:crates/opentake-agent/tests/completion_37cdd1870f1604a3.rs#completion_37cdd1870f1604a3_chat_supports_provider_selection_streaming_parsi
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “5.1 模型与双通道（`selectClient:52-59`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_37cdd1870f1604a3.rs#completion_37cdd1870f1604a3_chat_supports_provider_selection_streaming_parsi.

#### requirement-e4c34875f6507835

- Candidate/source: `doc-d53d94454d26aa3d` at `docs/specs/agent/5-chat.md:32` (requirement)
- Expected behavior: At docs/specs/agent/5-chat.md:32 under “5.2 HTTP + SSE（`AnthropicClient.run:57-86` + `AnthropicSSE.parse:88-152`）” (heading), the source “## 5.2 HTTP + SSE（`AnthropicClient.run:57-86` + `AnthropicSSE.parse:88-152`）” requires this exact behavior: Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair.
- Resolution: `reviewed-mapping-report:chat-provider-sse-tool-loop` — The report's provider/SSE/tool-loop gap is the implemented core of the ledger's complete chat contract; both share chat LLM/Tauri owners and SSE tests, so one capability should aggregate remaining cache, usage and persistence gaps.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/5-chat.md:32; signal=heading; heading=5.2 HTTP + SSE（`AnthropicClient.run:57-86` + `AnthropicSSE.parse:88-152`）; candidate=## 5.2 HTTP + SSE（`AnthropicClient.run:57-86` + `AnthropicSSE.parse:88-152`）
  - Expected behavior: Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair. This closes only the promise expressed by “5.2 HTTP + SSE（`AnthropicClient.run:57-86` + `AnthropicSSE.parse:88-152`）” in “5.2 HTTP + SSE（`AnthropicClient.run:57-86` + `AnthropicSSE.parse:88-152`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.2 HTTP + SSE（`AnthropicClient.run:57-86` + `AnthropicSSE.parse:88-152`）” with the scenario below and register test:crates/opentake-agent/tests/completion_d53d94454d26aa3d.rs#completion_d53d94454d26aa3d_chat_supports_provider_selection_streaming_parsi
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “5.2 HTTP + SSE（`AnthropicClient.run:57-86` + `AnthropicSSE.parse:88-152`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_d53d94454d26aa3d.rs#completion_d53d94454d26aa3d_chat_supports_provider_selection_streaming_parsi.

#### requirement-73340597a252e2d9

- Candidate/source: `doc-3d1a5b9b21cdd0f3` at `docs/specs/agent/5-chat.md:56` (requirement)
- Expected behavior: At docs/specs/agent/5-chat.md:56 under “5.3 Agentic loop（`runLoop:341-396` + `runPendingToolUses:422-447`）” (heading), the source “## 5.3 Agentic loop（`runLoop:341-396` + `runPendingToolUses:422-447`）” requires this exact behavior: Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair.
- Resolution: `reviewed-mapping-report:chat-provider-sse-tool-loop` — The report's provider/SSE/tool-loop gap is the implemented core of the ledger's complete chat contract; both share chat LLM/Tauri owners and SSE tests, so one capability should aggregate remaining cache, usage and persistence gaps.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/5-chat.md:56; signal=heading; heading=5.3 Agentic loop（`runLoop:341-396` + `runPendingToolUses:422-447`）; candidate=## 5.3 Agentic loop（`runLoop:341-396` + `runPendingToolUses:422-447`）
  - Expected behavior: Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair. This closes only the promise expressed by “5.3 Agentic loop（`runLoop:341-396` + `runPendingToolUses:422-447`）” in “5.3 Agentic loop（`runLoop:341-396` + `runPendingToolUses:422-447`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.3 Agentic loop（`runLoop:341-396` + `runPendingToolUses:422-447`）” with the scenario below and register test:crates/opentake-agent/tests/completion_3d1a5b9b21cdd0f3.rs#completion_3d1a5b9b21cdd0f3_chat_supports_provider_selection_streaming_parsi
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “5.3 Agentic loop（`runLoop:341-396` + `runPendingToolUses:422-447`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_3d1a5b9b21cdd0f3.rs#completion_3d1a5b9b21cdd0f3_chat_supports_provider_selection_streaming_parsi.

#### requirement-fa7d690795a9cf2b

- Candidate/source: `doc-854e3006a2b4b37a` at `docs/specs/agent/5-chat.md:77` (requirement)
- Expected behavior: Anthropic prompt cache boundaries are represented in request bodies.
- Resolution: `validated-ledger-evidence:AG-chat-complete-contract` — The report's provider/SSE/tool-loop gap is the implemented core of the ledger's complete chat contract; both share chat LLM/Tauri owners and SSE tests, so one capability should aggregate remaining cache, usage and persistence gaps.
- Exact acceptance contract:
  - Emit Anthropic cache_control only at the documented system/tool boundary and omit it for providers that do not support prompt caching.
  - Keep cacheable prompt content stable across turns while dynamic context/messages remain outside the cached prefix.
  - Snapshot request bodies for first/follow-up/tool turns across Anthropic and OpenAI, including disabled cache and provider-error cases.

#### requirement-27f77ddcca543305

- Candidate/source: `doc-1f59bf8e8ffb0eed` at `docs/specs/agent/5-chat.md:99` (requirement)
- Expected behavior: Token usage is recorded with provider/model/session attribution.
- Resolution: `validated-ledger-evidence:AG-chat-complete-contract` — The report's provider/SSE/tool-loop gap is the implemented core of the ledger's complete chat contract; both share chat LLM/Tauri owners and SSE tests, so one capability should aggregate remaining cache, usage and persistence gaps.
- Exact acceptance contract:
  - Record input, output, cache-read, cache-write tokens plus provider, model, session, request, and timestamp without message content or credentials.
  - Persist or export usage atomically and tolerate missing provider counters without failing chat.
  - Test exact counter mapping, concurrent sessions, redaction, disabled logging, partial SSE failure, and restart aggregation.

#### requirement-72c7d14797feeb3d

- Candidate/source: `doc-aa3bc86cfb951376` at `docs/specs/agent/5-chat.md:103` (requirement)
- Expected behavior: At docs/specs/agent/5-chat.md:103 under “5.6 孤儿 tool_use 修复（`resolveOrphanToolUses:458-495`）” (heading), the source “## 5.6 孤儿 tool_use 修复（`resolveOrphanToolUses:458-495`）” requires this exact behavior: Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair.
- Resolution: `reviewed-mapping-report:chat-provider-sse-tool-loop` — The report's provider/SSE/tool-loop gap is the implemented core of the ledger's complete chat contract; both share chat LLM/Tauri owners and SSE tests, so one capability should aggregate remaining cache, usage and persistence gaps.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/5-chat.md:103; signal=heading; heading=5.6 孤儿 tool_use 修复（`resolveOrphanToolUses:458-495`）; candidate=## 5.6 孤儿 tool_use 修复（`resolveOrphanToolUses:458-495`）
  - Expected behavior: Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair. This closes only the promise expressed by “5.6 孤儿 tool_use 修复（`resolveOrphanToolUses:458-495`）” in “5.6 孤儿 tool_use 修复（`resolveOrphanToolUses:458-495`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “5.6 孤儿 tool_use 修复（`resolveOrphanToolUses:458-495`）” with the scenario below and register test:crates/opentake-agent/tests/completion_aa3bc86cfb951376.rs#completion_aa3bc86cfb951376_chat_supports_provider_selection_streaming_parsi
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “5.6 孤儿 tool_use 修复（`resolveOrphanToolUses:458-495`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “Chat supports provider selection, streaming parsing, tool loops, and orphan tool-use repair.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_aa3bc86cfb951376.rs#completion_aa3bc86cfb951376_chat_supports_provider_selection_streaming_parsi.

#### requirement-e2b750e813e5adcb

- Candidate/source: `doc-2f1eb0d06955d32a` at `docs/specs/agent/5-chat.md:107` (requirement)
- Expected behavior: Chat context supports explicit mentions and inline image blocks.
- Resolution: `validated-ledger-evidence:AG-chat-complete-contract` — The report's provider/SSE/tool-loop gap is the implemented core of the ledger's complete chat contract; both share chat LLM/Tauri owners and SSE tests, so one capability should aggregate remaining cache, usage and persistence gaps.
- Exact acceptance contract:
  - Resolve @mentions to existing project/media/clip IDs and serialize bounded text/image context with explicit MIME and size limits.
  - Reject ambiguous/missing mentions and unsupported/oversized images before provider dispatch without mutating the session.
  - Test media/clip/project mentions, multiple images, ordering, short IDs, ambiguity, missing assets, MIME/size rejection, and provider request-body blocks.

#### requirement-34322dbbdfbe0cc7

- Candidate/source: `doc-726ead15e18ef8d1` at `docs/specs/agent/5-chat.md:117` (requirement)
- Expected behavior: Chat sessions persist durably across application restart.
- Resolution: `validated-ledger-evidence:AG-chat-complete-contract` — The report's provider/SSE/tool-loop gap is the implemented core of the ledger's complete chat contract; both share chat LLM/Tauri owners and SSE tests, so one capability should aggregate remaining cache, usage and persistence gaps.
- Exact acceptance contract:
  - Persist sessions/messages/tool-use/tool-result/model metadata atomically with a schema version, excluding API keys and transient stream buffers.
  - Restore sessions after restart, repair or mark interrupted tool/stream turns, and support delete without leaving orphaned records.
  - Test create/stream/tool/restart, crash-truncated data, migration, concurrent sessions, delete, and secret-redaction round trips.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/chat/llm.rs#anthropic_system_prompt_hoisted_top_level` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/chat/llm.rs#drain_sse_frames_handles_split_utf8_chunks` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/chat/loop.rs#orphan_tool_use_is_repaired_before_next_provider_round` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.
  - `crates/opentake-agent/src/chat/loop.rs#tool_round_persists_assistant_before_tool_results` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/chat/session.rs#session_round_trip` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/chat_contract.rs#cache_usage_mentions_images_and_restart_persistence` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent anthropic_system_prompt_hoisted_top_level`
  - Run: `cargo test -p opentake-agent drain_sse_frames_handles_split_utf8_chunks`
  - Run: `cargo test -p opentake-agent orphan_tool_use_is_repaired_before_next_provider_round`
  - Run: `cargo test -p opentake-agent tool_round_persists_assistant_before_tool_results`
  - Run: `cargo test -p opentake-agent session_round_trip`
  - Run: `cargo test -p opentake-agent --test chat_contract cache_usage_mentions_images_and_restart_persistence -- --exact`

  Expected: FAIL because one or more of the 9 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/chat/content.rs#ImageAndMentionBlocks`, `crates/opentake-agent/src/chat/llm.rs#anthropic_body`, `crates/opentake-agent/src/chat/llm.rs#drain_sse_frames`, `crates/opentake-agent/src/chat/llm.rs#provider_from_choice`, `crates/opentake-agent/src/chat/llm.rs#stream_chat`, `crates/opentake-agent/src/chat/loop.rs#ChatLoop::run_turn`, `crates/opentake-agent/src/chat/persistence.rs#ChatRepository`, `crates/opentake-agent/src/chat/session.rs#ChatSession`, `crates/opentake-agent/src/chat/usage.rs#UsageRecorder`, `src-tauri/src/chat.rs#ChatState`, `src-tauri/src/chat.rs#chat_cancel`, `src-tauri/src/chat.rs#chat_history`, `src-tauri/src/chat.rs#chat_send`, `docs/specs/agent/5-chat.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent anthropic_system_prompt_hoisted_top_level`
  - Run: `cargo test -p opentake-agent drain_sse_frames_handles_split_utf8_chunks`
  - Run: `cargo test -p opentake-agent orphan_tool_use_is_repaired_before_next_provider_round`
  - Run: `cargo test -p opentake-agent tool_round_persists_assistant_before_tool_results`
  - Run: `cargo test -p opentake-agent session_round_trip`
  - Run: `cargo test -p opentake-agent --test chat_contract cache_usage_mentions_images_and_restart_persistence -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 23: AG-core-signal-dispatch-contract (implementation-slice-d624fd75d0dc1612)

**Covered records:**
- `requirement-50532f0038d53c5a` (requirement)
- `requirement-5a6069a145343925` (requirement)
- `requirement-d0e2a09d92774d3c` (requirement)
- `requirement-06c7024737024833` (requirement)
- `requirement-f831fcf031f05b7b` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/mcp/core_handle.rs#CoreHandle`
- Modify: `crates/opentake-agent/src/tools/encode_timeline.rs#encode_timeline`
- Modify: `crates/opentake-agent/src/signal/engine.rs#attach`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs#dispatch`
- Modify: `docs/specs/agent/8-core-dispatch.md`
- Test (existing-owned): `crates/opentake-agent/src/tools/encode_timeline.rs#window_paging_hides_outside_clips_and_reports_totals`
- Test (existing-owned): `crates/opentake-agent/src/signal/engine.rs#get_timeline_attaches_full_signal`
- Test (existing-owned): `crates/opentake-agent/src/mcp/dispatch.rs#short_id_round_trip_shortens_outbound_id`
- Test (reviewed-planned): `crates/opentake-agent/tests/core_dispatch_acceptance.rs#core_handle_encoding_signal_and_cross_process_version_contract`

**Candidate-bound contracts:**

#### requirement-50532f0038d53c5a

- Candidate/source: `doc-1e3332faaf915944` at `docs/specs/agent/8-core-dispatch.md:1` (requirement)
- Expected behavior: At docs/specs/agent/8-core-dispatch.md:1 under “与 opentake-core 的接口” (heading), the source “# 与 opentake-core 的接口” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `validated-ledger-evidence:AG-core-signal-dispatch-contract` — Five headings form one cross-process core/signal/encoding acceptance umbrella.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/8-core-dispatch.md:1; signal=heading; heading=与 opentake-core 的接口; candidate=# 与 opentake-core 的接口
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “与 opentake-core 的接口” in “与 opentake-core 的接口”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “与 opentake-core 的接口” with the scenario below and register test:crates/opentake-agent/tests/completion_1e3332faaf915944.rs#completion_1e3332faaf915944_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “与 opentake-core 的接口”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_1e3332faaf915944.rs#completion_1e3332faaf915944_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-5a6069a145343925

- Candidate/source: `doc-64ce1890fd97551b` at `docs/specs/agent/8-core-dispatch.md:5` (requirement)
- Expected behavior: At docs/specs/agent/8-core-dispatch.md:5 under “8.1 命令枚举与结果（ARCHITECTURE `:105-116`，已定义在 `opentake-core`/`opentake-ops`）” (heading), the source “## 8.1 命令枚举与结果（ARCHITECTURE `:105-116`，已定义在 `opentake-core`/`opentake-ops`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `validated-ledger-evidence:AG-core-signal-dispatch-contract` — Five headings form one cross-process core/signal/encoding acceptance umbrella.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/8-core-dispatch.md:5; signal=heading; heading=8.1 命令枚举与结果（ARCHITECTURE `:105-116`，已定义在 `opentake-core`/`opentake-ops`）; candidate=## 8.1 命令枚举与结果（ARCHITECTURE `:105-116`，已定义在 `opentake-core`/`opentake-ops`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “8.1 命令枚举与结果（ARCHITECTURE `:105-116`，已定义在 `opentake-core`/`opentake-ops`）” in “8.1 命令枚举与结果（ARCHITECTURE `:105-116`，已定义在 `opentake-core`/`opentake-ops`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.1 命令枚举与结果（ARCHITECTURE `:105-116`，已定义在 `opentake-core`/`opentake-ops`）” with the scenario below and register test:crates/opentake-agent/tests/completion_64ce1890fd97551b.rs#completion_64ce1890fd97551b_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “8.1 命令枚举与结果（ARCHITECTURE `:105-116`，已定义在 `opentake-core`/`opentake-ops`）”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_64ce1890fd97551b.rs#completion_64ce1890fd97551b_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-d0e2a09d92774d3c

- Candidate/source: `doc-26b3c72e7d2fad1b` at `docs/specs/agent/8-core-dispatch.md:19` (requirement)
- Expected behavior: At docs/specs/agent/8-core-dispatch.md:19 under “8.2 `opentake-agent` 需要 `opentake-core` 暴露的接口（`CoreHandle`）” (heading), the source “## 8.2 `opentake-agent` 需要 `opentake-core` 暴露的接口（`CoreHandle`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `validated-ledger-evidence:AG-core-signal-dispatch-contract` — Five headings form one cross-process core/signal/encoding acceptance umbrella.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/8-core-dispatch.md:19; signal=heading; heading=8.2 `opentake-agent` 需要 `opentake-core` 暴露的接口（`CoreHandle`）; candidate=## 8.2 `opentake-agent` 需要 `opentake-core` 暴露的接口（`CoreHandle`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “8.2 `opentake-agent` 需要 `opentake-core` 暴露的接口（`CoreHandle`）” in “8.2 `opentake-agent` 需要 `opentake-core` 暴露的接口（`CoreHandle`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.2 `opentake-agent` 需要 `opentake-core` 暴露的接口（`CoreHandle`）” with the scenario below and register test:crates/opentake-agent/tests/completion_26b3c72e7d2fad1b.rs#completion_26b3c72e7d2fad1b_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: create an isolated temporary project fixture representing the source state in “8.2 `opentake-agent` 需要 `opentake-core` 暴露的接口（`CoreHandle`）”, perform the named open/edit/save/reopen event, and include its absent, malformed, and existing-file boundary where applicable.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, apply “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” atomically in the Rust project/core layer, changing only the named project files and in-memory session state while preserving unrelated assets and metadata.
  - Visible/returned assertion: assert the returned project/error variant, exact post-operation files and decoded state, and save-then-reopen equality or the specified fail-closed no-write result.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_26b3c72e7d2fad1b.rs#completion_26b3c72e7d2fad1b_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-06c7024737024833

- Candidate/source: `doc-fe0b4a37fe9d3e5a` at `docs/specs/agent/8-core-dispatch.md:63` (requirement)
- Expected behavior: At docs/specs/agent/8-core-dispatch.md:63 under “8.3 get_timeline 编码（压缩规则，照搬 `ToolExecutor+Timeline.swift:17-112`）” (heading), the source “## 8.3 get_timeline 编码（压缩规则，照搬 `ToolExecutor+Timeline.swift:17-112`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `validated-ledger-evidence:AG-core-signal-dispatch-contract` — Five headings form one cross-process core/signal/encoding acceptance umbrella.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/8-core-dispatch.md:63; signal=heading; heading=8.3 get_timeline 编码（压缩规则，照搬 `ToolExecutor+Timeline.swift:17-112`）; candidate=## 8.3 get_timeline 编码（压缩规则，照搬 `ToolExecutor+Timeline.swift:17-112`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “8.3 get_timeline 编码（压缩规则，照搬 `ToolExecutor+Timeline.swift:17-112`）” in “8.3 get_timeline 编码（压缩规则，照搬 `ToolExecutor+Timeline.swift:17-112`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.3 get_timeline 编码（压缩规则，照搬 `ToolExecutor+Timeline.swift:17-112`）” with the scenario below and register test:web/src/__tests__/completion/doc-fe0b4a37fe9d3e5a.test.ts#completion_fe0b4a37fe9d3e5a_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “8.3 get_timeline 编码（压缩规则，照搬 `ToolExecutor+Timeline.swift:17-112`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-fe0b4a37fe9d3e5a.test.ts#completion_fe0b4a37fe9d3e5a_the_documented_agent_signal_plugin_core_dispatch.

#### requirement-f831fcf031f05b7b

- Candidate/source: `doc-b10373bd7b75dd64` at `docs/specs/agent/8-core-dispatch.md:74` (requirement)
- Expected behavior: At docs/specs/agent/8-core-dispatch.md:74 under “8.4 跨进程一致性（ARCHITECTURE §2 `:62`）” (heading), the source “## 8.4 跨进程一致性（ARCHITECTURE §2 `:62`）” requires this exact behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.
- Resolution: `validated-ledger-evidence:AG-core-signal-dispatch-contract` — Five headings form one cross-process core/signal/encoding acceptance umbrella.
- Exact acceptance contract:
  - Source binding: docs/specs/agent/8-core-dispatch.md:74; signal=heading; heading=8.4 跨进程一致性（ARCHITECTURE §2 `:62`）; candidate=## 8.4 跨进程一致性（ARCHITECTURE §2 `:62`）
  - Expected behavior: The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests. This closes only the promise expressed by “8.4 跨进程一致性（ARCHITECTURE §2 `:62`）” in “8.4 跨进程一致性（ARCHITECTURE §2 `:62`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “8.4 跨进程一致性（ARCHITECTURE §2 `:62`）” with the scenario below and register test:crates/opentake-agent/tests/completion_b10373bd7b75dd64.rs#completion_b10373bd7b75dd64_the_documented_agent_signal_plugin_core_dispatch
  - Initial state/input/event: open a deterministic project and invoke the exact Agent/MCP/tool event from “8.4 跨进程一致性（ARCHITECTURE §2 `:62`）” through a loopback request with a local fake provider and explicit capability/credential state.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, dispatch “The documented Agent signal/plugin/core-dispatch contract is implemented with focused tests.” through typed Rust arguments and the named bridge/provider, committing only the specified project or timeline mutation and never returning a stubbed success.
  - Visible/returned assertion: assert the exact MCP/Tauri result schema, capability/error code, persisted project mutation, and user-visible timeline or job state for success, rejection, cancellation, and provider failure.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:crates/opentake-agent/tests/completion_b10373bd7b75dd64.rs#completion_b10373bd7b75dd64_the_documented_agent_signal_plugin_core_dispatch.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/src/tools/encode_timeline.rs#window_paging_hides_outside_clips_and_reports_totals` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/signal/engine.rs#get_timeline_attaches_full_signal` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/src/mcp/dispatch.rs#short_id_round_trip_shortens_outbound_id` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-agent/tests/core_dispatch_acceptance.rs#core_handle_encoding_signal_and_cross_process_version_contract` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent window_paging_hides_outside_clips_and_reports_totals`
  - Run: `cargo test -p opentake-agent get_timeline_attaches_full_signal`
  - Run: `cargo test -p opentake-agent short_id_round_trip_shortens_outbound_id`
  - Run: `cargo test -p opentake-agent --test core_dispatch_acceptance core_handle_encoding_signal_and_cross_process_version_contract -- --exact`

  Expected: FAIL because one or more of the 5 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/mcp/core_handle.rs#CoreHandle`, `crates/opentake-agent/src/tools/encode_timeline.rs#encode_timeline`, `crates/opentake-agent/src/signal/engine.rs#attach`, `crates/opentake-agent/src/mcp/dispatch.rs#dispatch`, `docs/specs/agent/8-core-dispatch.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent window_paging_hides_outside_clips_and_reports_totals`
  - Run: `cargo test -p opentake-agent get_timeline_attaches_full_signal`
  - Run: `cargo test -p opentake-agent short_id_round_trip_shortens_outbound_id`
  - Run: `cargo test -p opentake-agent --test core_dispatch_acceptance core_handle_encoding_signal_and_cross_process_version_contract -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 24: agent-telemetry (implementation-slice-cb40f555a0e0e8e1)

**Covered records:**
- `requirement-adb52c4cc902f758` (requirement)

**Files:**
- Modify: `crates/opentake-agent/src/telemetry.rs#TelemetryEvent`
- Modify: `crates/opentake-agent/src/lib.rs#telemetry`
- Modify: `docs/specs/agent/9-telemetry.md`
- Test (reviewed-planned): `crates/opentake-agent/tests/telemetry.rs#stable_correlation_and_privacy_redaction_opt_in`

**Candidate-bound contracts:**

#### requirement-adb52c4cc902f758

- Candidate/source: `doc-eec32e23a189dd7e` at `docs/specs/agent/9-telemetry.md:1` (requirement)
- Expected behavior: Agent operations emit structured, privacy-aware logs and optional telemetry with stable correlation fields.
- Resolution: `reviewed-mapping-report:agent-telemetry` — No structured, correlation-stable, privacy-aware optional telemetry owner was found; scattered tracing is insufficient.
- Exact acceptance contract:
  - Define structured event and redaction contracts.
  - Instrument server, chat, provider, and tool execution paths.
  - Add redaction, disabled-mode, and correlation tests.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `crates/opentake-agent/tests/telemetry.rs#stable_correlation_and_privacy_redaction_opt_in` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-agent --test telemetry stable_correlation_and_privacy_redaction_opt_in -- --exact`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `crates/opentake-agent/src/telemetry.rs#TelemetryEvent`, `crates/opentake-agent/src/lib.rs#telemetry`, `docs/specs/agent/9-telemetry.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-agent --test telemetry stable_correlation_and_privacy_redaction_opt_in -- --exact`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 25: captions-media-panel (implementation-slice-7721331842709b85)

**Covered records:**
- `requirement-8e59f57762f7737c` (requirement)

**Files:**
- Modify: `web/src/components/media/CaptionsTab.tsx#CaptionsTab`
- Modify: `web/src/store/editActions.ts#generateCaptions`
- Modify: `src-tauri/src/captions.rs#generate_captions`
- Modify: `docs/specs/frontend/7-media-panel.md`
- Test (existing-owned): `src-tauri/src/captions.rs#eligible_auto_keeps_audio_drops_silent_video`
- Test (existing-owned): `crates/opentake-media/src/transcribe/captions.rs#caption_specs_builds_and_cases_clips`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.test.tsx#generate_edit_export_surface`

**Candidate-bound contracts:**

#### requirement-8e59f57762f7737c

- Candidate/source: `doc-b3dec0943bf868e3` at `docs/specs/frontend/7-media-panel.md:37` (requirement)
- Expected behavior: At docs/specs/frontend/7-media-panel.md:37 under “7.3 Captions tab（`CaptionsTab/CaptionTab.swift`）” (heading), the source “### 7.3 Captions tab（`CaptionsTab/CaptionTab.swift`）” requires this exact behavior: Captions generation, editing, and export are available in the MediaPanel.
- Resolution: `reviewed-mapping-report:captions-media-panel` — The generation call chain and Rust caption coverage exist; a focused MediaPanel UI generation, editing, and export acceptance remains.
- Exact acceptance contract:
  - Source binding: docs/specs/frontend/7-media-panel.md:37; signal=heading; heading=7.3 Captions tab（`CaptionsTab/CaptionTab.swift`）; candidate=### 7.3 Captions tab（`CaptionsTab/CaptionTab.swift`）
  - Expected behavior: Captions generation, editing, and export are available in the MediaPanel. This closes only the promise expressed by “7.3 Captions tab（`CaptionsTab/CaptionTab.swift`）” in “7.3 Captions tab（`CaptionsTab/CaptionTab.swift`）”; adjacent headings or similarly named features remain independently adjudicated.
  - Deterministic test: exercise “7.3 Captions tab（`CaptionsTab/CaptionTab.swift`）” with the scenario below and register test:web/src/__tests__/completion/doc-b3dec0943bf868e3.test.ts#completion_b3dec0943bf868e3_captions_generation_editing_and_export_are_avail
  - Initial state/input/event: load a deterministic local media fixture with fixed dimensions, frame rate, timestamps, tracks, and the parameters stated by “7.3 Captions tab（`CaptionsTab/CaptionTab.swift`）”, then invoke the named preview, playback, render, or export event.
  - Code/store/API/Rust effect: at the Rust Agent/MCP dispatch, typed argument boundary, and any named Tauri/provider bridge, route “Captions generation, editing, and export are available in the MediaPanel.” through the real decode/composition/audio/export pipeline with no placeholder success and without mutating source media.
  - Visible/returned assertion: assert the exact returned status plus deterministic frame/audio/container properties or typed unsupported/error output, and verify preview/export parity when both paths are named.
  - Evidence required: after the deterministic test passes, record code:<tracked-file>#<declared-symbol> and test:<tracked-test-file>#<exact-test-name>; proposed concrete evidence is test:web/src/__tests__/completion/doc-b3dec0943bf868e3.test.ts#completion_b3dec0943bf868e3_captions_generation_editing_and_export_are_avail.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `src-tauri/src/captions.rs#eligible_auto_keeps_audio_drops_silent_video` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `crates/opentake-media/src/transcribe/captions.rs#caption_specs_builds_and_cases_clips` (existing-owned) — Exact named test already exists in the reviewed owning runner and records current boundary behavior.
  - `web/src/components/media/CaptionsTab.test.tsx#generate_edit_export_surface` (reviewed-planned) — Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `cargo test -p opentake-tauri eligible_auto_keeps_audio_drops_silent_video`
  - Run: `cargo test -p opentake-media caption_specs_builds_and_cases_clips`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.test.tsx -t "generate_edit_export_surface"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/CaptionsTab.tsx#CaptionsTab`, `web/src/store/editActions.ts#generateCaptions`, `src-tauri/src/captions.rs#generate_captions`, `docs/specs/frontend/7-media-panel.md` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `cargo test -p opentake-tauri eligible_auto_keeps_audio_drops_silent_video`
  - Run: `cargo test -p opentake-media caption_specs_builds_and_cases_clips`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.test.tsx -t "generate_edit_export_surface"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 26: control-acceptance (implementation-slice-865ca32f36ea1a09)

**Covered records:**
- `control-record-ffb1b60cf840e2e7` (control)
- `control-record-a3c5e1bb19c5ab15` (control)
- `control-record-ace2d4490e5dee31` (control)

**Files:**
- Modify: `web/src/components/agent/AgentPanel.tsx`
- Modify: `web/src/components/agent/AgentPanel.tsx#AgentPanel`
- Test (reviewed-planned): `web/src/components/agent/AgentPanel.interaction.test.tsx#control-70d50e82dbf11502 clear the current chat`
- Test (reviewed-planned): `web/src/components/agent/AgentPanel.interaction.test.tsx#control-7a3ce16dc3806574 open Settings from a missing-key guidance message`
- Test (reviewed-planned): `web/src/components/agent/AgentPanel.interaction.test.tsx#control-f3d4846ff119b22c expand or collapse an Agent tool call`

**Candidate-bound contracts:**

#### control-record-ffb1b60cf840e2e7

- Candidate/source: `control-70d50e82dbf11502` at `web/src/components/agent/AgentPanel.tsx:146:9` (control)
- Expected behavior: clear the current chat: reset(mintSessionId()) replaces session/messages when not streaming
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-70d50e82dbf11502.
  - Test: web/src/components/agent/AgentPanel.interaction.test.tsx#control-70d50e82dbf11502 clear the current chat.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({streaming}).
  - Event: inputs=["event/prop handler: {clearChat}","click or native keyboard activation plus current owning state"]; handler={clearChat}.
  - Exact call/state/backend: stateTransition=reset(mintSessionId()) replaces session/messages when not streaming; backendTrace=["web/src/components/agent/AgentPanel.tsx:146::candidate handler -> {clearChat}","actual branch/state -> reset(mintSessionId()) replaces session/messages when not streaming","exact call -> reset(mintSessionId()) replaces session/messages when not streaming","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/agent/AgentPanel.tsx#AgentPanel"].
  - Visible/accessibility/return path: success=clear the current chat: reset(mintSessionId()) replaces session/messages when not streaming; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"agent.clear\")","shortcut":"None declared on this control"}; returnPath=["Focus remains in the Agent panel; settings actions return through closing Settings."].
  - Outcome matrix: {"success":"clear the current chat: reset(mintSessionId()) replaces session/messages when not streaming","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"Disabled when {streaming}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-a3c5e1bb19c5ab15

- Candidate/source: `control-7a3ce16dc3806574` at `web/src/components/agent/AgentPanel.tsx:321:9` (control)
- Expected behavior: open Settings from a missing-key guidance message: setSettingsOpen(true)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-7a3ce16dc3806574.
  - Test: web/src/components/agent/AgentPanel.interaction.test.tsx#control-7a3ce16dc3806574 open Settings from a missing-key guidance message.
  - Initial state: visibility=Only for assistant messages matching the missing-key guidance expression; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onOpenSettings}","click or native keyboard activation plus current owning state"]; handler={onOpenSettings}.
  - Exact call/state/backend: stateTransition=setSettingsOpen(true); backendTrace=["web/src/components/agent/AgentPanel.tsx:321::candidate handler -> {onOpenSettings}","actual branch/state -> setSettingsOpen(true)","exact call -> setSettingsOpen(true)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/agent/AgentPanel.tsx#AgentPanel"].
  - Visible/accessibility/return path: success=open Settings from a missing-key guidance message: setSettingsOpen(true); accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Focus remains in the Agent panel; settings actions return through closing Settings."].
  - Outcome matrix: {"success":"open Settings from a missing-key guidance message: setSettingsOpen(true)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setSettingsOpen(true); no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-ace2d4490e5dee31

- Candidate/source: `control-f3d4846ff119b22c` at `web/src/components/agent/AgentPanel.tsx:368:7` (control)
- Expected behavior: expand or collapse an Agent tool call: setOpen toggles the tool-call details
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-f3d4846ff119b22c.
  - Test: web/src/components/agent/AgentPanel.interaction.test.tsx#control-f3d4846ff119b22c expand or collapse an Agent tool call.
  - Initial state: visibility=Rendered for assistant tool-call records; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setOpen((value) => !value)}","click or native keyboard activation plus current owning state"]; handler={() => setOpen((value) => !value)}.
  - Exact call/state/backend: stateTransition=setOpen toggles the tool-call details; backendTrace=["web/src/components/agent/AgentPanel.tsx:368::candidate handler -> {() => setOpen((value) => !value)}","actual branch/state -> setOpen toggles the tool-call details","exact call -> setOpen toggles the tool-call details","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/agent/AgentPanel.tsx#AgentPanel"].
  - Visible/accessibility/return path: success=expand or collapse an Agent tool call: setOpen toggles the tool-call details; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Focus remains in the Agent panel; settings actions return through closing Settings."].
  - Outcome matrix: {"success":"expand or collapse an Agent tool call: setOpen toggles the tool-call details","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setOpen toggles the tool-call details; no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/agent/AgentPanel.interaction.test.tsx#control-70d50e82dbf11502 clear the current chat` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/agent/AgentPanel.interaction.test.tsx#control-7a3ce16dc3806574 open Settings from a missing-key guidance message` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/agent/AgentPanel.interaction.test.tsx#control-f3d4846ff119b22c expand or collapse an Agent tool call` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-70d50e82dbf11502 clear the current chat"`
  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-7a3ce16dc3806574 open Settings from a missing-key guidance message"`
  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-f3d4846ff119b22c expand or collapse an Agent tool call"`

  Expected: FAIL because one or more of the 3 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/agent/AgentPanel.tsx`, `web/src/components/agent/AgentPanel.tsx#AgentPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-70d50e82dbf11502 clear the current chat"`
  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-7a3ce16dc3806574 open Settings from a missing-key guidance message"`
  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-f3d4846ff119b22c expand or collapse an Agent tool call"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 27: control-acceptance (implementation-slice-ba3735bf9c552eba)

**Covered records:**
- `control-record-f634d04b0c084cd1` (control)

**Files:**
- Modify: `web/src/components/agent/AgentPanel.tsx`
- Modify: `web/src/components/agent/AgentPanel.tsx#textarea`
- Modify: `web/src/lib/api.ts#chatSend`
- Modify: `src-tauri/src/chat.rs#chat_send`
- Modify: `web/src/components/agent/AgentPanel.tsx#AgentPanel`
- Test (reviewed-planned): `web/src/components/agent/AgentPanel.interaction.test.tsx#control-9276ea9d0a1578bb edit and submit the Agent prompt`

**Candidate-bound contracts:**

#### control-record-f634d04b0c084cd1

- Candidate/source: `control-9276ea9d0a1578bb` at `web/src/components/agent/AgentPanel.tsx:211:9` (control)
- Expected behavior: edit and submit the Agent prompt: onChange calls setInput(event.target.value); Enter without Shift prevents default and calls send(), while Shift+Enter does not send
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-9276ea9d0a1578bb.
  - Test: web/src/components/agent/AgentPanel.interaction.test.tsx#control-9276ea9d0a1578bb edit and submit the Agent prompt.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({!isTauri || streaming}).
  - Event: inputs=["event/prop handler: {(event) => setInput(event.target.value)} {onKeyDown}","current control value and deterministic replacement value"]; handler={(event) => setInput(event.target.value)} {onKeyDown}.
  - Exact call/state/backend: stateTransition=onChange calls setInput(event.target.value); Enter without Shift prevents default and calls send(), while Shift+Enter does not send; backendTrace=["web/src/components/agent/AgentPanel.tsx:211::candidate handler -> {(event) => setInput(event.target.value)} {onKeyDown}","actual branch/state -> onChange calls setInput(event.target.value); Enter without Shift prevents default and calls send(), while Shift+Enter does not send","exact call/arguments -> onChange only setInput(event.target.value); Enter without Shift calls send() -> chatSend(sessionId,input.trim(),provider); Shift+Enter issues no chat call","web/src/components/agent/AgentPanel.tsx::textarea onChange/onKeyDown -> setInput or send -> pushUser/beginStream/chatSend","web/src/lib/api.ts::chatSend -> invoke('chat_send',{sessionId,text,chatProvider:provider})","src-tauri/src/chat.rs::chat_send(session_id,text,chat_provider) -> spawned ChatLoop; chat_delta/chat_tool/chat_done events return to AgentPanel listeners","code:web/src/components/agent/AgentPanel.tsx#AgentPanel","code:web/src/lib/api.ts#chatSend","code:src-tauri/src/chat.rs#chat_send"].
  - Visible/accessibility/return path: success=edit and submit the Agent prompt: onChange calls setInput(event.target.value); Enter without Shift prevents default and calls send(), while Shift+Enter does not send; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Focus remains in the Agent panel; settings actions return through closing Settings."].
  - Outcome matrix: {"success":"edit and submit the Agent prompt: onChange calls setInput(event.target.value); Enter without Shift prevents default and calls send(), while Shift+Enter does not send","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/agent/AgentPanel.tsx:211; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"Disabled when {!isTauri || streaming}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in onChange calls setInput(event.target.value); Enter without Shift prevents default and calls send(), while Shift+Enter does not send.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/agent/AgentPanel.tsx:211; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/agent/AgentPanel.interaction.test.tsx#control-9276ea9d0a1578bb edit and submit the Agent prompt` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-9276ea9d0a1578bb edit and submit the Agent prompt"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/agent/AgentPanel.tsx`, `web/src/components/agent/AgentPanel.tsx#textarea`, `web/src/lib/api.ts#chatSend`, `src-tauri/src/chat.rs#chat_send`, `web/src/components/agent/AgentPanel.tsx#AgentPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-9276ea9d0a1578bb edit and submit the Agent prompt"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 28: control-acceptance (implementation-slice-935ad24420c633a9)

**Covered records:**
- `control-record-1e34805ece38646c` (control)

**Files:**
- Modify: `web/src/components/agent/AgentPanel.tsx`
- Modify: `web/src/components/agent/AgentPanel.tsx#cancel`
- Modify: `web/src/lib/api.ts#chatCancel`
- Modify: `src-tauri/src/chat.rs#chat_cancel`
- Modify: `web/src/components/agent/AgentPanel.tsx#AgentPanel`
- Test (reviewed-planned): `web/src/components/agent/AgentPanel.interaction.test.tsx#control-14c7a2b773381f0d cancel the active Agent stream`

**Candidate-bound contracts:**

#### control-record-1e34805ece38646c

- Candidate/source: `control-14c7a2b773381f0d` at `web/src/components/agent/AgentPanel.tsx:235:11` (control)
- Expected behavior: cancel the active Agent stream: calls chatCancel for the current session; streaming ends only after backend event
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-14c7a2b773381f0d.
  - Test: web/src/components/agent/AgentPanel.interaction.test.tsx#control-14c7a2b773381f0d cancel the active Agent stream.
  - Initial state: visibility=Only rendered while streaming; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {cancel}","click or native keyboard activation plus current owning state"]; handler={cancel}.
  - Exact call/state/backend: stateTransition=calls chatCancel for the current session; streaming ends only after backend event; backendTrace=["web/src/components/agent/AgentPanel.tsx:235::candidate handler -> {cancel}","actual branch/state -> calls chatCancel for the current session; streaming ends only after backend event","exact call/arguments -> chatCancel(current sessionId) exactly once while the streaming-only Cancel button is rendered","web/src/components/agent/AgentPanel.tsx::cancel -> void chatCancel(sessionId).catch(() => {})","web/src/lib/api.ts::chatCancel -> invoke('chat_cancel',{sessionId})","src-tauri/src/chat.rs::chat_cancel(session_id) -> cancel the matching running turn","code:web/src/components/agent/AgentPanel.tsx#AgentPanel","code:web/src/components/agent/AgentPanel.tsx#cancel","code:web/src/lib/api.ts#chatCancel","code:src-tauri/src/chat.rs#chat_cancel"].
  - Visible/accessibility/return path: success=cancel the active Agent stream: calls chatCancel for the current session; streaming ends only after backend event; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"agent.cancel\")","shortcut":"None declared on this control"}; returnPath=["Focus remains in the Agent panel; settings actions return through closing Settings."].
  - Outcome matrix: {"success":"cancel the active Agent stream: calls chatCancel for the current session; streaming ends only after backend event","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/agent/AgentPanel.tsx:235; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in calls chatCancel for the current session; streaming ends only after backend event.","failure":"chatCancel rejection is swallowed, so cancellation failure is silent"}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/agent/AgentPanel.interaction.test.tsx#control-14c7a2b773381f0d cancel the active Agent stream` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-14c7a2b773381f0d cancel the active Agent stream"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/agent/AgentPanel.tsx`, `web/src/components/agent/AgentPanel.tsx#cancel`, `web/src/lib/api.ts#chatCancel`, `src-tauri/src/chat.rs#chat_cancel`, `web/src/components/agent/AgentPanel.tsx#AgentPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-14c7a2b773381f0d cancel the active Agent stream"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 29: control-acceptance (implementation-slice-7f664d6c0b7ef6ed)

**Covered records:**
- `control-record-ba14deed7d156da6` (control)

**Files:**
- Modify: `web/src/components/agent/AgentPanel.tsx`
- Modify: `web/src/components/agent/AgentPanel.tsx#Send`
- Modify: `web/src/lib/api.ts#chatSend`
- Modify: `src-tauri/src/chat.rs#chat_send`
- Modify: `web/src/components/agent/AgentPanel.tsx#AgentPanel`
- Test (reviewed-planned): `web/src/components/agent/AgentPanel.interaction.test.tsx#control-1caee0ff811948b1 send the Agent prompt`

**Candidate-bound contracts:**

#### control-record-ba14deed7d156da6

- Candidate/source: `control-1caee0ff811948b1` at `web/src/components/agent/AgentPanel.tsx:244:11` (control)
- Expected behavior: send the Agent prompt: click calls send(): guard trimmed input/streaming, clear input, push user, create assistant placeholder, await chatSend, and finalize rejection locally
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-1caee0ff811948b1.
  - Test: web/src/components/agent/AgentPanel.interaction.test.tsx#control-1caee0ff811948b1 send the Agent prompt.
  - Initial state: visibility=Only rendered while not streaming; enabledWhen=not ({!isTauri || !input.trim()}).
  - Event: inputs=["event/prop handler: {() => void send()}","click or native keyboard activation plus current owning state"]; handler={() => void send()}.
  - Exact call/state/backend: stateTransition=click calls send(): guard trimmed input/streaming, clear input, push user, create assistant placeholder, await chatSend, and finalize rejection locally; backendTrace=["web/src/components/agent/AgentPanel.tsx:244::candidate handler -> {() => void send()}","actual branch/state -> click calls send(): guard trimmed input/streaming, clear input, push user, create assistant placeholder, await chatSend, and finalize rejection locally","exact call/arguments -> send() -> chatSend(sessionId,input.trim(),provider) after the non-empty/not-streaming guard","web/src/components/agent/AgentPanel.tsx::Send button -> send -> clear input, pushUser(text), beginStream(placeholderId), await chatSend","web/src/lib/api.ts::chatSend -> invoke('chat_send',{sessionId,text,chatProvider:provider})","src-tauri/src/chat.rs::chat_send(session_id,text,chat_provider) -> ChatLoop; rejection finalizes the existing assistant placeholder locally","code:web/src/components/agent/AgentPanel.tsx#AgentPanel","code:web/src/lib/api.ts#chatSend","code:src-tauri/src/chat.rs#chat_send"].
  - Visible/accessibility/return path: success=send the Agent prompt: click calls send(): guard trimmed input/streaming, clear input, push user, create assistant placeholder, await chatSend, and finalize rejection locally; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"agent.send\")","shortcut":"None declared on this control"}; returnPath=["Focus remains in the Agent panel; settings actions return through closing Settings."].
  - Outcome matrix: {"success":"send the Agent prompt: click calls send(): guard trimmed input/streaming, clear input, push user, create assistant placeholder, await chatSend, and finalize rejection locally","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/agent/AgentPanel.tsx:244; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"Disabled when {!isTauri || !input.trim()}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in click calls send(): guard trimmed input/streaming, clear input, push user, create assistant placeholder, await chatSend, and finalize rejection locally.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/agent/AgentPanel.tsx:244; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/agent/AgentPanel.interaction.test.tsx#control-1caee0ff811948b1 send the Agent prompt` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-1caee0ff811948b1 send the Agent prompt"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/agent/AgentPanel.tsx`, `web/src/components/agent/AgentPanel.tsx#Send`, `web/src/lib/api.ts#chatSend`, `src-tauri/src/chat.rs#chat_send`, `web/src/components/agent/AgentPanel.tsx#AgentPanel` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/agent/AgentPanel.interaction.test.tsx -t "control-1caee0ff811948b1 send the Agent prompt"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 30: control-acceptance (implementation-slice-02274be72fda6062)

**Covered records:**
- `control-record-027ed20640dd920b` (control)
- `control-record-19af14d8c18ca46d` (control)
- `control-record-2d8554a167a0f6af` (control)
- `control-record-90e6f4d037e46ece` (control)
- `control-record-ed9c8babc87792d6` (control)
- `control-record-78655e0ed0914317` (control)
- `control-record-b4232643e6446993` (control)
- `control-record-07f09d92a9217bdd` (control)
- `control-record-db9a2ef981207d4c` (control)
- `control-record-d970cb3665dd82a6` (control)

**Files:**
- Modify: `web/src/components/media/CaptionsTab.tsx`
- Modify: `web/src/components/media/CaptionsTab.tsx#CaptionsTab`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-173a24d0a5b24d12 choose automatic/selected-track caption source`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-864a8d81ed4a3129 set caption recognition language`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-0e9b6de469ce569a set caption font size`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-7d0f0b390f97e2f5 set caption text color`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-b0da1b49ce3388d7 set caption background color`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-5c8afd77c8cc1f31 enable or disable caption background`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-808b8801f94e741c choose caption letter case`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-5c6792e7313e01a6 toggle profanity censorship`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-6541661abc79bec1 set caption horizontal position`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-1a5f143020b7161a set caption vertical position`

**Candidate-bound contracts:**

#### control-record-027ed20640dd920b

- Candidate/source: `control-173a24d0a5b24d12` at `web/src/components/media/CaptionsTab.tsx:198:13` (control)
- Expected behavior: choose automatic/selected-track caption source: setTrackId changes CaptionRequest.source
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-173a24d0a5b24d12.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-173a24d0a5b24d12 choose automatic/selected-track caption source.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => setTrackId(e.target.value === \"__auto__\" ? null : e.target.value)}","current control value and deterministic replacement value"]; handler={(e) => setTrackId(e.target.value === "__auto__" ? null : e.target.value)}.
  - Exact call/state/backend: stateTransition=setTrackId changes CaptionRequest.source; backendTrace=["web/src/components/media/CaptionsTab.tsx:198::candidate handler -> {(e) => setTrackId(e.target.value === \"__auto__\" ? null : e.target.value)}","actual branch/state -> setTrackId changes CaptionRequest.source","exact call -> setTrackId changes CaptionRequest.source","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab"].
  - Visible/accessibility/return path: success=choose automatic/selected-track caption source: setTrackId changes CaptionRequest.source; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"choose automatic/selected-track caption source: setTrackId changes CaptionRequest.source","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:198; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-19af14d8c18ca46d

- Candidate/source: `control-864a8d81ed4a3129` at `web/src/components/media/CaptionsTab.tsx:212:13` (control)
- Expected behavior: set caption recognition language: setLanguage updates optional CaptionRequest.language
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-864a8d81ed4a3129.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-864a8d81ed4a3129 set caption recognition language.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => setLanguage(e.target.value)}","current control value and deterministic replacement value"]; handler={(e) => setLanguage(e.target.value)}.
  - Exact call/state/backend: stateTransition=setLanguage updates optional CaptionRequest.language; backendTrace=["web/src/components/media/CaptionsTab.tsx:212::candidate handler -> {(e) => setLanguage(e.target.value)}","actual branch/state -> setLanguage updates optional CaptionRequest.language","exact call -> setLanguage updates optional CaptionRequest.language","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab"].
  - Visible/accessibility/return path: success=set caption recognition language: setLanguage updates optional CaptionRequest.language; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"captions.language\")","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"set caption recognition language: setLanguage updates optional CaptionRequest.language","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:212; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-2d8554a167a0f6af

- Candidate/source: `control-0e9b6de469ce569a` at `web/src/components/media/CaptionsTab.tsx:224:13` (control)
- Expected behavior: set caption font size: clamps to 12..300 and updates preview/request style
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-0e9b6de469ce569a.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-0e9b6de469ce569a set caption font size.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => setFontSize(clampNumber(Number(e.target.value), MIN_FONT_SIZE, MAX_FONT_SIZE))}","current control value and deterministic replacement value"]; handler={(e) => setFontSize(clampNumber(Number(e.target.value), MIN_FONT_SIZE, MAX_FONT_SIZE))}.
  - Exact call/state/backend: stateTransition=clamps to 12..300 and updates preview/request style; backendTrace=["web/src/components/media/CaptionsTab.tsx:224::candidate handler -> {(e) => setFontSize(clampNumber(Number(e.target.value), MIN_FONT_SIZE, MAX_FONT_SIZE))}","actual branch/state -> clamps to 12..300 and updates preview/request style","exact call -> clamps to 12..300 and updates preview/request style","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab"].
  - Visible/accessibility/return path: success=set caption font size: clamps to 12..300 and updates preview/request style; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"captions.style.size\")","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"set caption font size: clamps to 12..300 and updates preview/request style","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:224; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-90e6f4d037e46ece

- Candidate/source: `control-7d0f0b390f97e2f5` at `web/src/components/media/CaptionsTab.tsx:235:13` (control)
- Expected behavior: set caption text color: setColor updates preview/request TextStyle
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-7d0f0b390f97e2f5.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-7d0f0b390f97e2f5 set caption text color.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {setColor}","click or native keyboard activation plus current owning state"]; handler={setColor}.
  - Exact call/state/backend: stateTransition=setColor updates preview/request TextStyle; backendTrace=["web/src/components/media/CaptionsTab.tsx:235::candidate handler -> {setColor}","actual branch/state -> setColor updates preview/request TextStyle","exact call -> setColor updates preview/request TextStyle","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab"].
  - Visible/accessibility/return path: success=set caption text color: setColor updates preview/request TextStyle; accessibility={"focus":"Custom ColorSwatch focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"set caption text color: setColor updates preview/request TextStyle","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:235; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-ed9c8babc87792d6

- Candidate/source: `control-b0da1b49ce3388d7` at `web/src/components/media/CaptionsTab.tsx:239:15` (control)
- Expected behavior: set caption background color: updates background.color when enabled
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-b0da1b49ce3388d7.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-b0da1b49ce3388d7 set caption background color.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=background.enabled is true.
  - Event: inputs=["event/prop handler: {(c) => setBackground((b) => ({ ...b, color: c }))}","click or native keyboard activation plus current owning state"]; handler={(c) => setBackground((b) => ({ ...b, color: c }))}.
  - Exact call/state/backend: stateTransition=updates background.color when enabled; backendTrace=["web/src/components/media/CaptionsTab.tsx:239::candidate handler -> {(c) => setBackground((b) => ({ ...b, color: c }))}","actual branch/state -> updates background.color when enabled","exact call -> updates background.color when enabled","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab"].
  - Visible/accessibility/return path: success=set caption background color: updates background.color when enabled; accessibility={"focus":"Custom ColorSwatch focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"set caption background color: updates background.color when enabled","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:239; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {!background.enabled}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-78655e0ed0914317

- Candidate/source: `control-5c8afd77c8cc1f31` at `web/src/components/media/CaptionsTab.tsx:245:15` (control)
- Expected behavior: enable or disable caption background: updates background.enabled and ColorSwatch disabled state
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-5c8afd77c8cc1f31.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-5c8afd77c8cc1f31 enable or disable caption background.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => setBackground((b) => ({ ...b, enabled: e.target.checked }))}","current control value and deterministic replacement value"]; handler={(e) => setBackground((b) => ({ ...b, enabled: e.target.checked }))}.
  - Exact call/state/backend: stateTransition=updates background.enabled and ColorSwatch disabled state; backendTrace=["web/src/components/media/CaptionsTab.tsx:245::candidate handler -> {(e) => setBackground((b) => ({ ...b, enabled: e.target.checked }))}","actual branch/state -> updates background.enabled and ColorSwatch disabled state","exact call -> updates background.enabled and ColorSwatch disabled state","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab"].
  - Visible/accessibility/return path: success=enable or disable caption background: updates background.enabled and ColorSwatch disabled state; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"captions.style.background\")","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"enable or disable caption background: updates background.enabled and ColorSwatch disabled state","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:245; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-b4232643e6446993

- Candidate/source: `control-808b8801f94e741c` at `web/src/components/media/CaptionsTab.tsx:254:13` (control)
- Expected behavior: choose caption letter case: setTextCase updates CaptionRequest.textCase
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-808b8801f94e741c.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-808b8801f94e741c choose caption letter case.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => setTextCase(e.target.value as CaptionCase)}","current control value and deterministic replacement value"]; handler={(e) => setTextCase(e.target.value as CaptionCase)}.
  - Exact call/state/backend: stateTransition=setTextCase updates CaptionRequest.textCase; backendTrace=["web/src/components/media/CaptionsTab.tsx:254::candidate handler -> {(e) => setTextCase(e.target.value as CaptionCase)}","actual branch/state -> setTextCase updates CaptionRequest.textCase","exact call -> setTextCase updates CaptionRequest.textCase","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab"].
  - Visible/accessibility/return path: success=choose caption letter case: setTextCase updates CaptionRequest.textCase; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"captions.style.case\")","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"choose caption letter case: setTextCase updates CaptionRequest.textCase","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:254; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-07f09d92a9217bdd

- Candidate/source: `control-5c6792e7313e01a6` at `web/src/components/media/CaptionsTab.tsx:268:13` (control)
- Expected behavior: toggle profanity censorship: setCensorProfanity updates CaptionRequest
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-5c6792e7313e01a6.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-5c6792e7313e01a6 toggle profanity censorship.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => setCensorProfanity(e.target.checked)}","current control value and deterministic replacement value"]; handler={(e) => setCensorProfanity(e.target.checked)}.
  - Exact call/state/backend: stateTransition=setCensorProfanity updates CaptionRequest; backendTrace=["web/src/components/media/CaptionsTab.tsx:268::candidate handler -> {(e) => setCensorProfanity(e.target.checked)}","actual branch/state -> setCensorProfanity updates CaptionRequest","exact call -> setCensorProfanity updates CaptionRequest","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab"].
  - Visible/accessibility/return path: success=toggle profanity censorship: setCensorProfanity updates CaptionRequest; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"captions.censorProfanity\")","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"toggle profanity censorship: setCensorProfanity updates CaptionRequest","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-db9a2ef981207d4c

- Candidate/source: `control-6541661abc79bec1` at `web/src/components/media/CaptionsTab.tsx:280:13` (control)
- Expected behavior: set caption horizontal position: clamps 0..1 and snaps near 0.5
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-6541661abc79bec1.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-6541661abc79bec1 set caption horizontal position.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(v) => setCenterX(snapCenter(v))}","click or native keyboard activation plus current owning state"]; handler={(v) => setCenterX(snapCenter(v))}.
  - Exact call/state/backend: stateTransition=clamps 0..1 and snaps near 0.5; backendTrace=["web/src/components/media/CaptionsTab.tsx:280::candidate handler -> {(v) => setCenterX(snapCenter(v))}","actual branch/state -> clamps 0..1 and snaps near 0.5","exact call -> clamps 0..1 and snaps near 0.5","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab"].
  - Visible/accessibility/return path: success=set caption horizontal position: clamps 0..1 and snaps near 0.5; accessibility={"focus":"Custom PosField focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"set caption horizontal position: clamps 0..1 and snaps near 0.5","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:280; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-d970cb3665dd82a6

- Candidate/source: `control-1a5f143020b7161a` at `web/src/components/media/CaptionsTab.tsx:281:13` (control)
- Expected behavior: set caption vertical position: clamps 0..1 and snaps near 0.5
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-1a5f143020b7161a.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-1a5f143020b7161a set caption vertical position.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(v) => setCenterY(snapCenter(v))}","click or native keyboard activation plus current owning state"]; handler={(v) => setCenterY(snapCenter(v))}.
  - Exact call/state/backend: stateTransition=clamps 0..1 and snaps near 0.5; backendTrace=["web/src/components/media/CaptionsTab.tsx:281::candidate handler -> {(v) => setCenterY(snapCenter(v))}","actual branch/state -> clamps 0..1 and snaps near 0.5","exact call -> clamps 0..1 and snaps near 0.5","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab"].
  - Visible/accessibility/return path: success=set caption vertical position: clamps 0..1 and snaps near 0.5; accessibility={"focus":"Custom PosField focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"set caption vertical position: clamps 0..1 and snaps near 0.5","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:281; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-173a24d0a5b24d12 choose automatic/selected-track caption source` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-864a8d81ed4a3129 set caption recognition language` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-0e9b6de469ce569a set caption font size` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-7d0f0b390f97e2f5 set caption text color` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-b0da1b49ce3388d7 set caption background color` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-5c8afd77c8cc1f31 enable or disable caption background` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-808b8801f94e741c choose caption letter case` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-5c6792e7313e01a6 toggle profanity censorship` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-6541661abc79bec1 set caption horizontal position` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-1a5f143020b7161a set caption vertical position` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-173a24d0a5b24d12 choose automatic/selected-track caption source"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-864a8d81ed4a3129 set caption recognition language"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-0e9b6de469ce569a set caption font size"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-7d0f0b390f97e2f5 set caption text color"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-b0da1b49ce3388d7 set caption background color"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-5c8afd77c8cc1f31 enable or disable caption background"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-808b8801f94e741c choose caption letter case"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-5c6792e7313e01a6 toggle profanity censorship"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-6541661abc79bec1 set caption horizontal position"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-1a5f143020b7161a set caption vertical position"`

  Expected: FAIL because one or more of the 10 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/CaptionsTab.tsx`, `web/src/components/media/CaptionsTab.tsx#CaptionsTab` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-173a24d0a5b24d12 choose automatic/selected-track caption source"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-864a8d81ed4a3129 set caption recognition language"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-0e9b6de469ce569a set caption font size"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-7d0f0b390f97e2f5 set caption text color"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-b0da1b49ce3388d7 set caption background color"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-5c8afd77c8cc1f31 enable or disable caption background"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-808b8801f94e741c choose caption letter case"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-5c6792e7313e01a6 toggle profanity censorship"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-6541661abc79bec1 set caption horizontal position"`
  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-1a5f143020b7161a set caption vertical position"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 31: control-acceptance (implementation-slice-4c06767519f28ac9)

**Covered records:**
- `control-record-529a07e575d8fa62` (control)

**Files:**
- Modify: `web/src/components/media/CaptionsTab.tsx`
- Modify: `web/src/components/media/CaptionsTab.tsx#onDownloadModel`
- Modify: `web/src/lib/api.ts#downloadTranscribeModel`
- Modify: `src-tauri/src/transcribe.rs#download_transcribe_model`
- Modify: `web/src/store/editActions.ts#generateCaptions`
- Modify: `web/src/lib/api.ts#generateCaptions`
- Modify: `src-tauri/src/captions.rs#generate_captions`
- Modify: `web/src/components/media/CaptionsTab.tsx#CaptionsTab`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-74ac8d91228bc40b download the transcription model and continue captioning`

**Candidate-bound contracts:**

#### control-record-529a07e575d8fa62

- Candidate/source: `control-74ac8d91228bc40b` at `web/src/components/media/CaptionsTab.tsx:308:13` (control)
- Expected behavior: download the transcription model and continue captioning: downloading progress -> runGenerate -> idle/note
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-74ac8d91228bc40b.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-74ac8d91228bc40b download the transcription model and continue captioning.
  - Initial state: visibility=Only after model-status reports not installed; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {onDownloadModel}","click or native keyboard activation plus current owning state"]; handler={onDownloadModel}.
  - Exact call/state/backend: stateTransition=downloading progress -> runGenerate -> idle/note; backendTrace=["web/src/components/media/CaptionsTab.tsx:308::candidate handler -> {onDownloadModel}","actual branch/state -> downloading progress -> runGenerate -> idle/note","exact call/arguments -> onTranscribeProgress(handler), await downloadTranscribeModel(), unlisten(), then runGenerate() with the exact CaptionRequest built from source/style/position/case/censor/language","web/src/components/media/CaptionsTab.tsx::onDownloadModel -> onTranscribeProgress; downloadTranscribeModel; runGenerate","web/src/lib/api.ts::downloadTranscribeModel -> invoke('download_transcribe_model'); onTranscribeProgress listens to transcribe://progress","src-tauri/src/transcribe.rs::download_transcribe_model -> model download/progress","web/src/store/editActions.ts::generateCaptions(request) -> api.generateCaptions(request) and forceRefresh when captions were placed","web/src/lib/api.ts::generateCaptions -> invoke('generate_captions',{request}); src-tauri/src/captions.rs::generate_captions","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab","code:web/src/components/media/CaptionsTab.tsx#onDownloadModel","code:web/src/lib/api.ts#downloadTranscribeModel","code:src-tauri/src/transcribe.rs#download_transcribe_model","code:web/src/store/editActions.ts#generateCaptions","code:web/src/lib/api.ts#generateCaptions","code:src-tauri/src/captions.rs#generate_captions"].
  - Visible/accessibility/return path: success=download the transcription model and continue captioning: downloading progress -> runGenerate -> idle/note; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"download the transcription model and continue captioning: downloading progress -> runGenerate -> idle/note","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/CaptionsTab.tsx:308; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:308; the candidate-specific interaction test must assert that exact guard.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in downloading progress -> runGenerate -> idle/note; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in downloading progress -> runGenerate -> idle/note.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/CaptionsTab.tsx:308; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-74ac8d91228bc40b download the transcription model and continue captioning` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-74ac8d91228bc40b download the transcription model and continue captioning"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/CaptionsTab.tsx`, `web/src/components/media/CaptionsTab.tsx#onDownloadModel`, `web/src/lib/api.ts#downloadTranscribeModel`, `src-tauri/src/transcribe.rs#download_transcribe_model`, `web/src/store/editActions.ts#generateCaptions`, `web/src/lib/api.ts#generateCaptions`, `src-tauri/src/captions.rs#generate_captions`, `web/src/components/media/CaptionsTab.tsx#CaptionsTab` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-74ac8d91228bc40b download the transcription model and continue captioning"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 32: control-acceptance (implementation-slice-5152ebb633fb16d0)

**Covered records:**
- `control-record-86ca728e0d97d312` (control)

**Files:**
- Modify: `web/src/components/media/CaptionsTab.tsx`
- Modify: `web/src/components/media/CaptionsTab.tsx#onGenerate`
- Modify: `web/src/lib/api.ts#transcribeModelStatus`
- Modify: `src-tauri/src/transcribe.rs#transcribe_model_status`
- Modify: `web/src/store/editActions.ts#generateCaptions`
- Modify: `web/src/lib/api.ts#generateCaptions`
- Modify: `src-tauri/src/captions.rs#generate_captions`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand::AddCaptions`
- Modify: `web/src/components/media/CaptionsTab.tsx#CaptionsTab`
- Modify: `crates/opentake-ops/src/command.rs#EditCommand`
- Test (reviewed-planned): `web/src/components/media/CaptionsTab.interaction.test.tsx#control-b33351f0ed5519ab generate captions`

**Candidate-bound contracts:**

#### control-record-86ca728e0d97d312

- Candidate/source: `control-b33351f0ed5519ab` at `web/src/components/media/CaptionsTab.tsx:313:11` (control)
- Expected behavior: generate captions: model check -> needsModel or transcribing -> added/no-speech/error note -> idle
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-b33351f0ed5519ab.
  - Test: web/src/components/media/CaptionsTab.interaction.test.tsx#control-b33351f0ed5519ab generate captions.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not downloading or transcribing.
  - Event: inputs=["event/prop handler: {onGenerate}","click or native keyboard activation plus current owning state"]; handler={onGenerate}.
  - Exact call/state/backend: stateTransition=model check -> needsModel or transcribing -> added/no-speech/error note -> idle; backendTrace=["web/src/components/media/CaptionsTab.tsx:313::candidate handler -> {onGenerate}","actual branch/state -> model check -> needsModel or transcribing -> added/no-speech/error note -> idle","exact call/arguments -> transcribeModelStatus(); if installed (or probe rejects) call generateCaptions({source,style,centerX,centerY,textCase,censorProfanity,language})","web/src/components/media/CaptionsTab.tsx::onGenerate/runGenerate -> transcribeModelStatus then generateCaptions(request)","web/src/lib/api.ts::transcribeModelStatus -> invoke('transcribe_model_status') -> src-tauri/src/transcribe.rs::transcribe_model_status","web/src/store/editActions.ts::generateCaptions -> web/src/lib/api.ts::generateCaptions -> invoke('generate_captions',{request})","src-tauri/src/captions.rs::generate_captions -> caption pipeline and crates/opentake-ops/src/command.rs::EditCommand::AddCaptions","code:web/src/components/media/CaptionsTab.tsx#CaptionsTab","code:web/src/components/media/CaptionsTab.tsx#onGenerate","code:web/src/lib/api.ts#transcribeModelStatus","code:src-tauri/src/transcribe.rs#transcribe_model_status","code:web/src/store/editActions.ts#generateCaptions","code:web/src/lib/api.ts#generateCaptions","code:src-tauri/src/captions.rs#generate_captions","code:crates/opentake-ops/src/command.rs#EditCommand"].
  - Visible/accessibility/return path: success=generate captions: model check -> needsModel or transcribing -> added/no-speech/error note -> idle; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["The Captions tab remains open after generation/download and shows a note; no separate dialog."].
  - Outcome matrix: {"success":"generate captions: model check -> needsModel or transcribing -> added/no-speech/error note -> idle","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/media/CaptionsTab.tsx:313; no additional state is inferred beyond the source.","empty":"Required empty/no-selection behavior is the render/handler guard in web/src/components/media/CaptionsTab.tsx:313; the candidate-specific interaction test must assert that exact guard.","disabled":"Disabled when {busy}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in model check -> needsModel or transcribing -> added/no-speech/error note -> idle.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/media/CaptionsTab.tsx:313; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/media/CaptionsTab.interaction.test.tsx#control-b33351f0ed5519ab generate captions` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-b33351f0ed5519ab generate captions"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/media/CaptionsTab.tsx`, `web/src/components/media/CaptionsTab.tsx#onGenerate`, `web/src/lib/api.ts#transcribeModelStatus`, `src-tauri/src/transcribe.rs#transcribe_model_status`, `web/src/store/editActions.ts#generateCaptions`, `web/src/lib/api.ts#generateCaptions`, `src-tauri/src/captions.rs#generate_captions`, `crates/opentake-ops/src/command.rs#EditCommand::AddCaptions`, `web/src/components/media/CaptionsTab.tsx#CaptionsTab`, `crates/opentake-ops/src/command.rs#EditCommand` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/media/CaptionsTab.interaction.test.tsx -t "control-b33351f0ed5519ab generate captions"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 33: control-acceptance (implementation-slice-99266e41f1bb88f5)

**Covered records:**
- `control-record-f33fbba92d76183e` (control)

**Files:**
- Modify: `web/src/components/settings/AccountPane.tsx`
- Modify: `web/src/components/settings/AccountPane.tsx#URL`
- Modify: `web/src/lib/api.ts#accountSetBackendUrl`
- Modify: `src-tauri/src/account.rs#account_set_backend_url`
- Modify: `web/src/components/settings/AccountPane.tsx#AccountPane`
- Test (reviewed-planned): `web/src/components/settings/AccountPane.interaction.test.tsx#control-703fb0833631f951 edit/save the optional account backend URL`

**Candidate-bound contracts:**

#### control-record-f33fbba92d76183e

- Candidate/source: `control-703fb0833631f951` at `web/src/components/settings/AccountPane.tsx:222:11` (control)
- Expected behavior: edit/save the optional account backend URL: onChange updates urlDraft and clears message/error; Enter calls guarded saveUrl
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-703fb0833631f951.
  - Test: web/src/components/settings/AccountPane.interaction.test.tsx#control-703fb0833631f951 edit/save the optional account backend URL.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(event) => { setUrlDraft(event.target.value); setMessage(null); setError(null); }} {(event) => { if (event.key === \"Enter\") void saveUrl(); }}","current control value and deterministic replacement value"]; handler={(event) => { setUrlDraft(event.target.value); setMessage(null); setError(null); }} {(event) => { if (event.key === "Enter") void saveUrl(); }}.
  - Exact call/state/backend: stateTransition=onChange updates urlDraft and clears message/error; Enter calls guarded saveUrl; backendTrace=["web/src/components/settings/AccountPane.tsx:222::candidate handler -> {(event) => { setUrlDraft(event.target.value); setMessage(null); setError(null); }} {(event) => { if (event.key === \"Enter\") void saveUrl(); }}","actual branch/state -> onChange updates urlDraft and clears message/error; Enter calls guarded saveUrl","exact call/arguments -> onChange only updates urlDraft/message/error; Enter calls saveUrl() -> accountSetBackendUrl(trimmed-or-null), accountGetBackendUrl(), accountGetStatus()","web/src/components/settings/AccountPane.tsx::URL input onChange/onKeyDown -> local draft or saveUrl exact chain","web/src/lib/api.ts::accountSetBackendUrl/accountGetBackendUrl/accountGetStatus -> invoke account_set_backend_url{url}/account_get_backend_url/account_get_status","src-tauri/src/account.rs::account_set_backend_url/account_get_backend_url/account_get_status","code:web/src/components/settings/AccountPane.tsx#AccountPane","code:web/src/lib/api.ts#accountSetBackendUrl","code:src-tauri/src/account.rs#account_set_backend_url"].
  - Visible/accessibility/return path: success=edit/save the optional account backend URL: onChange updates urlDraft and clears message/error; Enter calls guarded saveUrl; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Remains in Account Settings; status/error is rendered with live semantics."].
  - Outcome matrix: {"success":"edit/save the optional account backend URL: onChange updates urlDraft and clears message/error; Enter calls guarded saveUrl","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/AccountPane.tsx:222; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in onChange updates urlDraft and clears message/error; Enter calls guarded saveUrl.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/settings/AccountPane.tsx:222; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/AccountPane.interaction.test.tsx#control-703fb0833631f951 edit/save the optional account backend URL` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-703fb0833631f951 edit/save the optional account backend URL"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/AccountPane.tsx`, `web/src/components/settings/AccountPane.tsx#URL`, `web/src/lib/api.ts#accountSetBackendUrl`, `src-tauri/src/account.rs#account_set_backend_url`, `web/src/components/settings/AccountPane.tsx#AccountPane` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-703fb0833631f951 edit/save the optional account backend URL"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 34: control-acceptance (implementation-slice-159cdb0074c44288)

**Covered records:**
- `control-record-72798d486cf6a42d` (control)

**Files:**
- Modify: `web/src/components/settings/AccountPane.tsx`
- Modify: `web/src/components/settings/AccountPane.tsx#Save`
- Modify: `web/src/lib/api.ts#accountSetBackendUrl`
- Modify: `src-tauri/src/account.rs#account_set_backend_url`
- Modify: `web/src/components/settings/AccountPane.tsx#AccountPane`
- Test (reviewed-planned): `web/src/components/settings/AccountPane.interaction.test.tsx#control-b1b49fe894904211 save the account backend URL`

**Candidate-bound contracts:**

#### control-record-72798d486cf6a42d

- Candidate/source: `control-b1b49fe894904211` at `web/src/components/settings/AccountPane.tsx:248:11` (control)
- Expected behavior: save the account backend URL: click calls guarded saveUrl; busyRef coalesces repeats; success reloads saved URL/status; failure alerts
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-b1b49fe894904211.
  - Test: web/src/components/settings/AccountPane.interaction.test.tsx#control-b1b49fe894904211 save the account backend URL.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({busy}).
  - Event: inputs=["event/prop handler: {() => void saveUrl()}","click or native keyboard activation plus current owning state"]; handler={() => void saveUrl()}.
  - Exact call/state/backend: stateTransition=click calls guarded saveUrl; busyRef coalesces repeats; success reloads saved URL/status; failure alerts; backendTrace=["web/src/components/settings/AccountPane.tsx:248::candidate handler -> {() => void saveUrl()}","actual branch/state -> click calls guarded saveUrl; busyRef coalesces repeats; success reloads saved URL/status; failure alerts","exact call/arguments -> saveUrl() -> accountSetBackendUrl(urlDraft.trim() || null), accountGetBackendUrl(), accountGetStatus() after beginAction","web/src/components/settings/AccountPane.tsx::Save button -> saveUrl -> beginAction/accountSetBackendUrl/accountGetBackendUrl/refreshStatus/finally endAction","web/src/lib/api.ts::accountSetBackendUrl/accountGetBackendUrl/accountGetStatus -> invoke account_set_backend_url{url}/account_get_backend_url/account_get_status","src-tauri/src/account.rs::account_set_backend_url/account_get_backend_url/account_get_status","code:web/src/components/settings/AccountPane.tsx#AccountPane","code:web/src/lib/api.ts#accountSetBackendUrl","code:src-tauri/src/account.rs#account_set_backend_url"].
  - Visible/accessibility/return path: success=save the account backend URL: click calls guarded saveUrl; busyRef coalesces repeats; success reloads saved URL/status; failure alerts; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Remains in Account Settings; status/error is rendered with live semantics."].
  - Outcome matrix: {"success":"save the account backend URL: click calls guarded saveUrl; busyRef coalesces repeats; success reloads saved URL/status; failure alerts","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/AccountPane.tsx:248; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"Disabled when {busy}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in click calls guarded saveUrl; busyRef coalesces repeats; success reloads saved URL/status; failure alerts.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/settings/AccountPane.tsx:248; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/AccountPane.interaction.test.tsx#control-b1b49fe894904211 save the account backend URL` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-b1b49fe894904211 save the account backend URL"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/AccountPane.tsx`, `web/src/components/settings/AccountPane.tsx#Save`, `web/src/lib/api.ts#accountSetBackendUrl`, `src-tauri/src/account.rs#account_set_backend_url`, `web/src/components/settings/AccountPane.tsx#AccountPane` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-b1b49fe894904211 save the account backend URL"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 35: control-acceptance (implementation-slice-e1b86bf20b23268a)

**Covered records:**
- `control-record-6bea2043e841d241` (control)

**Files:**
- Modify: `web/src/components/settings/AccountPane.tsx`
- Modify: `web/src/components/settings/AccountPane.tsx#clearUrl`
- Modify: `web/src/lib/api.ts#accountSetBackendUrl`
- Modify: `src-tauri/src/account.rs#account_set_backend_url`
- Modify: `web/src/components/settings/AccountPane.tsx#AccountPane`
- Test (reviewed-planned): `web/src/components/settings/AccountPane.interaction.test.tsx#control-49f0f22c0f1a93f0 clear the account backend URL`

**Candidate-bound contracts:**

#### control-record-6bea2043e841d241

- Candidate/source: `control-49f0f22c0f1a93f0` at `web/src/components/settings/AccountPane.tsx:267:13` (control)
- Expected behavior: clear the account backend URL: busy -> clear URL/token -> refresh status or alert
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-49f0f22c0f1a93f0.
  - Test: web/src/components/settings/AccountPane.interaction.test.tsx#control-49f0f22c0f1a93f0 clear the account backend URL.
  - Initial state: visibility=Only when a backend URL is saved; enabledWhen=not ({busy}).
  - Event: inputs=["event/prop handler: {() => void clearUrl()}","click or native keyboard activation plus current owning state"]; handler={() => void clearUrl()}.
  - Exact call/state/backend: stateTransition=busy -> clear URL/token -> refresh status or alert; backendTrace=["web/src/components/settings/AccountPane.tsx:267::candidate handler -> {() => void clearUrl()}","actual branch/state -> busy -> clear URL/token -> refresh status or alert","exact call/arguments -> clearUrl(): accountSetBackendUrl(null), clear local url/token drafts, then accountGetStatus()","web/src/components/settings/AccountPane.tsx::clearUrl -> beginAction; accountSetBackendUrl(null); refreshStatus; finally endAction","web/src/lib/api.ts::accountSetBackendUrl -> invoke('account_set_backend_url',{url:null}); accountGetStatus -> invoke('account_get_status')","src-tauri/src/account.rs::account_set_backend_url/account_get_status","code:web/src/components/settings/AccountPane.tsx#AccountPane","code:web/src/components/settings/AccountPane.tsx#clearUrl","code:web/src/lib/api.ts#accountSetBackendUrl","code:src-tauri/src/account.rs#account_set_backend_url"].
  - Visible/accessibility/return path: success=clear the account backend URL: busy -> clear URL/token -> refresh status or alert; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Remains in Account Settings; status/error is rendered with live semantics."].
  - Outcome matrix: {"success":"clear the account backend URL: busy -> clear URL/token -> refresh status or alert","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/AccountPane.tsx:267; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"Disabled when {busy}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in busy -> clear URL/token -> refresh status or alert.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/settings/AccountPane.tsx:267; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/AccountPane.interaction.test.tsx#control-49f0f22c0f1a93f0 clear the account backend URL` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-49f0f22c0f1a93f0 clear the account backend URL"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/AccountPane.tsx`, `web/src/components/settings/AccountPane.tsx#clearUrl`, `web/src/lib/api.ts#accountSetBackendUrl`, `src-tauri/src/account.rs#account_set_backend_url`, `web/src/components/settings/AccountPane.tsx#AccountPane` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-49f0f22c0f1a93f0 clear the account backend URL"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 36: control-acceptance (implementation-slice-17efd83b9b2dd1d3)

**Covered records:**
- `control-record-0e319fc905bb7301` (control)

**Files:**
- Modify: `web/src/components/settings/AccountPane.tsx`
- Modify: `web/src/components/settings/AccountPane.tsx#token`
- Modify: `web/src/lib/api.ts#accountLogin`
- Modify: `src-tauri/src/account.rs#account_login`
- Modify: `web/src/components/settings/AccountPane.tsx#AccountPane`
- Test (reviewed-planned): `web/src/components/settings/AccountPane.interaction.test.tsx#control-419844d19dfbcb73 enter/login with an account token`

**Candidate-bound contracts:**

#### control-record-0e319fc905bb7301

- Candidate/source: `control-419844d19dfbcb73` at `web/src/components/settings/AccountPane.tsx:311:11` (control)
- Expected behavior: enter/login with an account token: onChange updates tokenDraft and clears error; Enter calls guarded login; token clears on success
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-419844d19dfbcb73.
  - Test: web/src/components/settings/AccountPane.interaction.test.tsx#control-419844d19dfbcb73 enter/login with an account token.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(event) => { setTokenDraft(event.target.value); setError(null); }} {(event) => { if (event.key === \"Enter\") void login(); }}","current control value and deterministic replacement value"]; handler={(event) => { setTokenDraft(event.target.value); setError(null); }} {(event) => { if (event.key === "Enter") void login(); }}.
  - Exact call/state/backend: stateTransition=onChange updates tokenDraft and clears error; Enter calls guarded login; token clears on success; backendTrace=["web/src/components/settings/AccountPane.tsx:311::candidate handler -> {(event) => { setTokenDraft(event.target.value); setError(null); }} {(event) => { if (event.key === \"Enter\") void login(); }}","actual branch/state -> onChange updates tokenDraft and clears error; Enter calls guarded login; token clears on success","exact call/arguments -> onChange only updates tokenDraft/error; Enter calls login() -> accountLogin(tokenDraft.trim()), clear token draft, then accountGetStatus() when all guards pass","web/src/components/settings/AccountPane.tsx::token input onChange/onKeyDown -> local draft or guarded login exact chain","web/src/lib/api.ts::accountLogin -> invoke('account_login',{token:trimmed}); accountGetStatus -> invoke('account_get_status')","src-tauri/src/account.rs::account_login(token)/account_get_status","code:web/src/components/settings/AccountPane.tsx#AccountPane","code:web/src/lib/api.ts#accountLogin","code:src-tauri/src/account.rs#account_login"].
  - Visible/accessibility/return path: success=enter/login with an account token: onChange updates tokenDraft and clears error; Enter calls guarded login; token clears on success; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Remains in Account Settings; status/error is rendered with live semantics."].
  - Outcome matrix: {"success":"enter/login with an account token: onChange updates tokenDraft and clears error; Enter calls guarded login; token clears on success","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/AccountPane.tsx:311; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in onChange updates tokenDraft and clears error; Enter calls guarded login; token clears on success.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/settings/AccountPane.tsx:311; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/AccountPane.interaction.test.tsx#control-419844d19dfbcb73 enter/login with an account token` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-419844d19dfbcb73 enter/login with an account token"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/AccountPane.tsx`, `web/src/components/settings/AccountPane.tsx#token`, `web/src/lib/api.ts#accountLogin`, `src-tauri/src/account.rs#account_login`, `web/src/components/settings/AccountPane.tsx#AccountPane` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-419844d19dfbcb73 enter/login with an account token"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 37: control-acceptance (implementation-slice-0f0ef25e56516c43)

**Covered records:**
- `control-record-ee8bb1dcbdf993a5` (control)

**Files:**
- Modify: `web/src/components/settings/AccountPane.tsx`
- Modify: `web/src/components/settings/AccountPane.tsx#Login`
- Modify: `web/src/lib/api.ts#accountLogin`
- Modify: `src-tauri/src/account.rs#account_login`
- Modify: `web/src/components/settings/AccountPane.tsx#AccountPane`
- Test (reviewed-planned): `web/src/components/settings/AccountPane.interaction.test.tsx#control-e98fbd99572f9df0 log in to the optional account backend`

**Candidate-bound contracts:**

#### control-record-ee8bb1dcbdf993a5

- Candidate/source: `control-e98fbd99572f9df0` at `web/src/components/settings/AccountPane.tsx:335:11` (control)
- Expected behavior: log in to the optional account backend: click calls guarded login; connecting -> online/offline/error; busy and dirty-URL gates
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-e98fbd99572f9df0.
  - Test: web/src/components/settings/AccountPane.interaction.test.tsx#control-e98fbd99572f9df0 log in to the optional account backend.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=not ({busy || !backendUrl || urlDirty || tokenDraft.trim().length === 0}).
  - Event: inputs=["event/prop handler: {() => void login()}","click or native keyboard activation plus current owning state"]; handler={() => void login()}.
  - Exact call/state/backend: stateTransition=click calls guarded login; connecting -> online/offline/error; busy and dirty-URL gates; backendTrace=["web/src/components/settings/AccountPane.tsx:335::candidate handler -> {() => void login()}","actual branch/state -> click calls guarded login; connecting -> online/offline/error; busy and dirty-URL gates","exact call/arguments -> login() -> accountLogin(tokenDraft.trim()), clear token draft, then accountGetStatus() when backendUrl/non-dirty/non-empty/not-busy guards pass","web/src/components/settings/AccountPane.tsx::Login button -> guarded login/beginAction/accountLogin/refreshStatus/finally endAction","web/src/lib/api.ts::accountLogin -> invoke('account_login',{token:trimmed}); accountGetStatus -> invoke('account_get_status')","src-tauri/src/account.rs::account_login(token)/account_get_status","code:web/src/components/settings/AccountPane.tsx#AccountPane","code:web/src/lib/api.ts#accountLogin","code:src-tauri/src/account.rs#account_login"].
  - Visible/accessibility/return path: success=log in to the optional account backend: click calls guarded login; connecting -> online/offline/error; busy and dirty-URL gates; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Remains in Account Settings; status/error is rendered with live semantics."].
  - Outcome matrix: {"success":"log in to the optional account backend: click calls guarded login; connecting -> online/offline/error; busy and dirty-URL gates","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/AccountPane.tsx:335; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"Disabled when {busy || !backendUrl || urlDirty || tokenDraft.trim().length === 0}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in click calls guarded login; connecting -> online/offline/error; busy and dirty-URL gates.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/settings/AccountPane.tsx:335; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/AccountPane.interaction.test.tsx#control-e98fbd99572f9df0 log in to the optional account backend` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-e98fbd99572f9df0 log in to the optional account backend"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/AccountPane.tsx`, `web/src/components/settings/AccountPane.tsx#Login`, `web/src/lib/api.ts#accountLogin`, `src-tauri/src/account.rs#account_login`, `web/src/components/settings/AccountPane.tsx#AccountPane` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-e98fbd99572f9df0 log in to the optional account backend"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 38: control-acceptance (implementation-slice-fe9708c521b8cf96)

**Covered records:**
- `control-record-f5f00e955d6db45a` (control)

**Files:**
- Modify: `web/src/components/settings/AccountPane.tsx`
- Modify: `web/src/components/settings/AccountPane.tsx#logout`
- Modify: `web/src/lib/api.ts#accountLogout`
- Modify: `src-tauri/src/account.rs#account_logout`
- Modify: `web/src/components/settings/AccountPane.tsx#AccountPane`
- Test (reviewed-planned): `web/src/components/settings/AccountPane.interaction.test.tsx#control-ef7b4e65da007a65 log out of the optional account backend`

**Candidate-bound contracts:**

#### control-record-f5f00e955d6db45a

- Candidate/source: `control-ef7b4e65da007a65` at `web/src/components/settings/AccountPane.tsx:354:13` (control)
- Expected behavior: log out of the optional account backend: beginAction sets busy/busyRef; await accountLogout(); clear token; await refreshStatus(); catch renders errorMessage(reason); finally endAction re-enables controls
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-ef7b4e65da007a65.
  - Test: web/src/components/settings/AccountPane.interaction.test.tsx#control-ef7b4e65da007a65 log out of the optional account backend.
  - Initial state: visibility=Only for online/stored credentials; enabledWhen=not ({busy}).
  - Event: inputs=["event/prop handler: {() => void logout()}","click or native keyboard activation plus current owning state"]; handler={() => void logout()}.
  - Exact call/state/backend: stateTransition=beginAction sets busy/busyRef; await accountLogout(); clear token; await refreshStatus(); catch renders errorMessage(reason); finally endAction re-enables controls; backendTrace=["web/src/components/settings/AccountPane.tsx:354::candidate handler -> {() => void logout()}","actual branch/state -> beginAction sets busy/busyRef; await accountLogout(); clear token; await refreshStatus(); catch renders errorMessage(reason); finally endAction re-enables controls","exact call/arguments -> logout(): await accountLogout(), clear token draft, await accountGetStatus(); catch stores errorMessage(reason); finally endAction()","web/src/components/settings/AccountPane.tsx::logout -> beginAction; accountLogout; refreshStatus; catch; finally endAction","web/src/lib/api.ts::accountLogout -> invoke('account_logout'); accountGetStatus -> invoke('account_get_status')","src-tauri/src/account.rs::account_logout/account_get_status","code:web/src/components/settings/AccountPane.tsx#AccountPane","code:web/src/components/settings/AccountPane.tsx#logout","code:web/src/lib/api.ts#accountLogout","code:src-tauri/src/account.rs#account_logout"].
  - Visible/accessibility/return path: success=log out of the optional account backend: beginAction sets busy/busyRef; await accountLogout(); clear token; await refreshStatus(); catch renders errorMessage(reason); finally endAction re-enables controls; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"account.logout\")","shortcut":"None declared on this control"}; returnPath=["Remains in Account Settings; status/error is rendered with live semantics."].
  - Outcome matrix: {"success":"log out of the optional account backend: beginAction sets busy/busyRef; await accountLogout(); clear token; await refreshStatus(); catch renders errorMessage(reason); finally endAction re-enables controls","pending":"busyRef coalesces repeat activation; busy=true disables Logout until the awaited logout/status chain settles.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"Disabled when {busy}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"After finally/endAction clears busy, the still-visible online/stored Logout control can be activated again.","failure":"The catch branch stores errorMessage(reason) in the role=alert region, preserves the pre-refresh status, and finally clears busy so Logout can retry."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/AccountPane.interaction.test.tsx#control-ef7b4e65da007a65 log out of the optional account backend` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-ef7b4e65da007a65 log out of the optional account backend"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/AccountPane.tsx`, `web/src/components/settings/AccountPane.tsx#logout`, `web/src/lib/api.ts#accountLogout`, `src-tauri/src/account.rs#account_logout`, `web/src/components/settings/AccountPane.tsx#AccountPane` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/AccountPane.interaction.test.tsx -t "control-ef7b4e65da007a65 log out of the optional account backend"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 39: control-acceptance (implementation-slice-2d4dab13cb81a5d1)

**Covered records:**
- `control-record-f9b0ed9e3cee0eb5` (control)
- `control-record-ad20d723371d427b` (control)
- `control-record-3b2e44285d8c34e5` (control)
- `control-record-06669d3354e10f47` (control)
- `control-record-3efbe85ed599fcfa` (control)
- `control-record-ac604e9624ae6313` (control)
- `control-record-65a5c4166bb0f6ba` (control)

**Files:**
- Modify: `web/src/components/settings/SettingsView.tsx`
- Modify: `web/src/components/settings/SettingsView.tsx#SettingsView`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-fc5baa4f457e8778 close Settings with Done`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-ca0fb9e4faf6c987 close Settings with the icon button`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-6fcdff2d800c882d switch Settings panes`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-b019554a5961fa7a change application language`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-323d35d24ceb5ab2 change application theme`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-1f8b140411e09e22 change default window size`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-fe4bfbcf3bec4054 clear the default import folder`

**Candidate-bound contracts:**

#### control-record-f9b0ed9e3cee0eb5

- Candidate/source: `control-fc5baa4f457e8778` at `web/src/components/settings/SettingsView.tsx:141:13` (control)
- Expected behavior: close Settings with Done: setSettingsOpen(false)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-fc5baa4f457e8778.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-fc5baa4f457e8778 close Settings with Done.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setSettingsOpen(false)}","click or native keyboard activation plus current owning state"]; handler={() => setSettingsOpen(false)}.
  - Exact call/state/backend: stateTransition=setSettingsOpen(false); backendTrace=["web/src/components/settings/SettingsView.tsx:141::candidate handler -> {() => setSettingsOpen(false)}","actual branch/state -> setSettingsOpen(false)","exact call -> setSettingsOpen(false)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/settings/SettingsView.tsx#SettingsView"].
  - Visible/accessibility/return path: success=close Settings with Done: setSettingsOpen(false); accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"close Settings with Done: setSettingsOpen(false)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setSettingsOpen(false); no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-ad20d723371d427b

- Candidate/source: `control-ca0fb9e4faf6c987` at `web/src/components/settings/SettingsView.tsx:157:13` (control)
- Expected behavior: close Settings with the icon button: setSettingsOpen(false)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-ca0fb9e4faf6c987.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-ca0fb9e4faf6c987 close Settings with the icon button.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setSettingsOpen(false)}","click or native keyboard activation plus current owning state"]; handler={() => setSettingsOpen(false)}.
  - Exact call/state/backend: stateTransition=setSettingsOpen(false); backendTrace=["web/src/components/settings/SettingsView.tsx:157::candidate handler -> {() => setSettingsOpen(false)}","actual branch/state -> setSettingsOpen(false)","exact call -> setSettingsOpen(false)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/settings/SettingsView.tsx#SettingsView"].
  - Visible/accessibility/return path: success=close Settings with the icon button: setSettingsOpen(false); accessibility={"focus":"Native keyboard-focusable control","label":"Close","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"close Settings with the icon button: setSettingsOpen(false)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setSettingsOpen(false); no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-3b2e44285d8c34e5

- Candidate/source: `control-6fcdff2d800c882d` at `web/src/components/settings/SettingsView.tsx:211:13` (control)
- Expected behavior: switch Settings panes: onSelect(pane.id) changes activePane
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-6fcdff2d800c882d.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-6fcdff2d800c882d switch Settings panes.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => onSelect(pane.id)}","click or native keyboard activation plus current owning state"]; handler={() => onSelect(pane.id)}.
  - Exact call/state/backend: stateTransition=onSelect(pane.id) changes activePane; backendTrace=["web/src/components/settings/SettingsView.tsx:211::candidate handler -> {() => onSelect(pane.id)}","actual branch/state -> onSelect(pane.id) changes activePane","exact call -> onSelect(pane.id) changes activePane","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/settings/SettingsView.tsx#SettingsView"].
  - Visible/accessibility/return path: success=switch Settings panes: onSelect(pane.id) changes activePane; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"switch Settings panes: onSelect(pane.id) changes activePane","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-06669d3354e10f47

- Candidate/source: `control-b019554a5961fa7a` at `web/src/components/settings/SettingsView.tsx:374:11` (control)
- Expected behavior: change application language: setLocale updates/persists locale
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-b019554a5961fa7a.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-b019554a5961fa7a change application language.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {setLocale}","click or native keyboard activation plus current owning state"]; handler={setLocale}.
  - Exact call/state/backend: stateTransition=setLocale updates/persists locale; backendTrace=["web/src/components/settings/SettingsView.tsx:374::candidate handler -> {setLocale}","actual branch/state -> setLocale updates/persists locale","exact call -> setLocale updates/persists locale","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/settings/SettingsView.tsx#SettingsView"].
  - Visible/accessibility/return path: success=change application language: setLocale updates/persists locale; accessibility={"focus":"Custom Dropdown focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"change application language: setLocale updates/persists locale","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-3efbe85ed599fcfa

- Candidate/source: `control-323d35d24ceb5ab2` at `web/src/components/settings/SettingsView.tsx:399:11` (control)
- Expected behavior: change application theme: setTheme updates/persists theme
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-323d35d24ceb5ab2.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-323d35d24ceb5ab2 change application theme.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {setTheme}","click or native keyboard activation plus current owning state"]; handler={setTheme}.
  - Exact call/state/backend: stateTransition=setTheme updates/persists theme; backendTrace=["web/src/components/settings/SettingsView.tsx:399::candidate handler -> {setTheme}","actual branch/state -> setTheme updates/persists theme","exact call -> setTheme updates/persists theme","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/settings/SettingsView.tsx#SettingsView"].
  - Visible/accessibility/return path: success=change application theme: setTheme updates/persists theme; accessibility={"focus":"Custom Segmented focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"change application theme: setTheme updates/persists theme","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-ac604e9624ae6313

- Candidate/source: `control-1f8b140411e09e22` at `web/src/components/settings/SettingsView.tsx:413:11` (control)
- Expected behavior: change default window size: setWindowSize updates/persists window size
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-1f8b140411e09e22.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-1f8b140411e09e22 change default window size.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {setWindowSize}","click or native keyboard activation plus current owning state"]; handler={setWindowSize}.
  - Exact call/state/backend: stateTransition=setWindowSize updates/persists window size; backendTrace=["web/src/components/settings/SettingsView.tsx:413::candidate handler -> {setWindowSize}","actual branch/state -> setWindowSize updates/persists window size","exact call -> setWindowSize updates/persists window size","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/settings/SettingsView.tsx#SettingsView"].
  - Visible/accessibility/return path: success=change default window size: setWindowSize updates/persists window size; accessibility={"focus":"Custom Segmented focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"change default window size: setWindowSize updates/persists window size","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

#### control-record-65a5c4166bb0f6ba

- Candidate/source: `control-fe4bfbcf3bec4054` at `web/src/components/settings/SettingsView.tsx:467:15` (control)
- Expected behavior: clear the default import folder: setDefaultImportFolder(null)
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-fe4bfbcf3bec4054.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-fe4bfbcf3bec4054 clear the default import folder.
  - Initial state: visibility=Only when a folder is set; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => setFolder(null)}","click or native keyboard activation plus current owning state"]; handler={() => setFolder(null)}.
  - Exact call/state/backend: stateTransition=setDefaultImportFolder(null); backendTrace=["web/src/components/settings/SettingsView.tsx:467::candidate handler -> {() => setFolder(null)}","actual branch/state -> setDefaultImportFolder(null)","exact call -> setDefaultImportFolder(null)","API/Tauri/Rust -> N/A for this immediate activation; later submit/export/generate actions are separate candidates.","code:web/src/components/settings/SettingsView.tsx#SettingsView"].
  - Visible/accessibility/return path: success=clear the default import folder: setDefaultImportFolder(null); accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"clear the default import folder: setDefaultImportFolder(null)","pending":"Not applicable — this activation only changes local/caller state and starts no Promise, API, Tauri command, or Rust work.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in setDefaultImportFolder(null); no broader cancellation behavior is assumed.","retry":"Not applicable as an error-recovery state — the synchronous local action can simply be activated again.","failure":"Not applicable — this immediate activation calls no API/Tauri/Rust boundary, so there is no backend rejection path."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-fc5baa4f457e8778 close Settings with Done` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-ca0fb9e4faf6c987 close Settings with the icon button` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-6fcdff2d800c882d switch Settings panes` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-b019554a5961fa7a change application language` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-323d35d24ceb5ab2 change application theme` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-1f8b140411e09e22 change default window size` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-fe4bfbcf3bec4054 clear the default import folder` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-fc5baa4f457e8778 close Settings with Done"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-ca0fb9e4faf6c987 close Settings with the icon button"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-6fcdff2d800c882d switch Settings panes"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-b019554a5961fa7a change application language"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-323d35d24ceb5ab2 change application theme"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-1f8b140411e09e22 change default window size"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-fe4bfbcf3bec4054 clear the default import folder"`

  Expected: FAIL because one or more of the 7 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/SettingsView.tsx`, `web/src/components/settings/SettingsView.tsx#SettingsView` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-fc5baa4f457e8778 close Settings with Done"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-ca0fb9e4faf6c987 close Settings with the icon button"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-6fcdff2d800c882d switch Settings panes"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-b019554a5961fa7a change application language"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-323d35d24ceb5ab2 change application theme"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-1f8b140411e09e22 change default window size"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-fe4bfbcf3bec4054 clear the default import folder"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 40: control-acceptance (implementation-slice-b7dc0e2cf1f58d34)

**Covered records:**
- `control-record-e0e025ac523971b3` (control)

**Files:**
- Modify: `web/src/components/settings/SettingsView.tsx`
- Modify: `web/src/components/settings/SettingsView.tsx#ImportPane`
- Modify: `web/src/lib/dialog.ts#openDialog`
- Modify: `web/src/store/settingsStore.ts#setDefaultImportFolder`
- Modify: `web/src/components/settings/SettingsView.tsx#SettingsView`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-a6a47152d4fc53e0 choose the default import folder`

**Candidate-bound contracts:**

#### control-record-e0e025ac523971b3

- Candidate/source: `control-a6a47152d4fc53e0` at `web/src/components/settings/SettingsView.tsx:446:13` (control)
- Expected behavior: choose the default import folder: native directory dialog -> setDefaultImportFolder
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-a6a47152d4fc53e0.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-a6a47152d4fc53e0 choose the default import folder.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => void choose()}","click or native keyboard activation plus current owning state"]; handler={() => void choose()}.
  - Exact call/state/backend: stateTransition=native directory dialog -> setDefaultImportFolder; backendTrace=["web/src/components/settings/SettingsView.tsx:446::candidate handler -> {() => void choose()}","actual branch/state -> native directory dialog -> setDefaultImportFolder","exact call/arguments -> choose(): openDialog(); open({directory:true,multiple:false}); if selected is string call setDefaultImportFolder(selected)","web/src/components/settings/SettingsView.tsx::ImportPane.choose -> web/src/lib/dialog.ts::openDialog/open","web/src/store/settingsStore.ts::setDefaultImportFolder(selected) -> persist localStorage and update Zustand","API/Tauri/Rust -> N/A after the native dialog returns; the selected path is stored locally","code:web/src/components/settings/SettingsView.tsx#SettingsView","code:web/src/components/settings/SettingsView.tsx#ImportPane","code:web/src/lib/dialog.ts#openDialog"].
  - Visible/accessibility/return path: success=choose the default import folder: native directory dialog -> setDefaultImportFolder; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"choose the default import folder: native directory dialog -> setDefaultImportFolder","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/SettingsView.tsx:446; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Cancellation/dismissal follows the exact guard in native directory dialog -> setDefaultImportFolder; no broader cancellation behavior is assumed.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in native directory dialog -> setDefaultImportFolder.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/settings/SettingsView.tsx:446; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-a6a47152d4fc53e0 choose the default import folder` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-a6a47152d4fc53e0 choose the default import folder"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/SettingsView.tsx`, `web/src/components/settings/SettingsView.tsx#ImportPane`, `web/src/lib/dialog.ts#openDialog`, `web/src/store/settingsStore.ts#setDefaultImportFolder`, `web/src/components/settings/SettingsView.tsx#SettingsView` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-a6a47152d4fc53e0 choose the default import folder"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.

### Task 41: control-acceptance (implementation-slice-99f4fb233e94de44)

**Covered records:**
- `control-record-c9a82aa4ef4335fe` (control)

**Files:**
- Modify: `web/src/components/settings/SettingsView.tsx`
- Modify: `web/src/store/settingsStore.ts#setByokProvider`
- Modify: `web/src/components/settings/SettingsView.tsx#AiPane`
- Modify: `web/src/lib/api.ts#secretLoad`
- Modify: `src-tauri/src/secret.rs#secret_load`
- Modify: `web/src/components/settings/SettingsView.tsx#SettingsView`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-408c771762a48a6a select the BYOK provider`

**Candidate-bound contracts:**

#### control-record-c9a82aa4ef4335fe

- Candidate/source: `control-408c771762a48a6a` at `web/src/components/settings/SettingsView.tsx:569:11` (control)
- Expected behavior: select the BYOK provider: setByokProvider; effect reloads masked key status and clears draft
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-408c771762a48a6a.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-408c771762a48a6a select the BYOK provider.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {setProvider}","click or native keyboard activation plus current owning state"]; handler={setProvider}.
  - Exact call/state/backend: stateTransition=setByokProvider; effect reloads masked key status and clears draft; backendTrace=["web/src/components/settings/SettingsView.tsx:569::candidate handler -> {setProvider}","actual branch/state -> setByokProvider; effect reloads masked key status and clears draft","exact call/arguments -> setByokProvider(opt.id); provider effect clears draft/error and calls secretLoad(provider) for the newly selected provider","web/src/store/settingsStore.ts::setByokProvider(provider) -> persist localStorage/update Zustand","web/src/components/settings/SettingsView.tsx::AiPane provider effect -> secretLoad(provider)","web/src/lib/api.ts::secretLoad -> invoke('secret_load',{provider}); src-tauri/src/secret.rs::secret_load(provider)","code:web/src/components/settings/SettingsView.tsx#SettingsView","code:web/src/components/settings/SettingsView.tsx#AiPane","code:web/src/lib/api.ts#secretLoad","code:src-tauri/src/secret.rs#secret_load"].
  - Visible/accessibility/return path: success=select the BYOK provider: setByokProvider; effect reloads masked key status and clears draft; accessibility={"focus":"Custom Segmented focus behavior depends on its implementation","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"select the BYOK provider: setByokProvider; effect reloads masked key status and clears draft","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/SettingsView.tsx:569; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in setByokProvider; effect reloads masked key status and clears draft.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/settings/SettingsView.tsx:569; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-408c771762a48a6a select the BYOK provider` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-408c771762a48a6a select the BYOK provider"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/SettingsView.tsx`, `web/src/store/settingsStore.ts#setByokProvider`, `web/src/components/settings/SettingsView.tsx#AiPane`, `web/src/lib/api.ts#secretLoad`, `src-tauri/src/secret.rs#secret_load`, `web/src/components/settings/SettingsView.tsx#SettingsView` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-408c771762a48a6a select the BYOK provider"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 42: control-acceptance (implementation-slice-817afe72b6dea74e)

**Covered records:**
- `control-record-b9070bce133b034f` (control)
- `control-record-7ddb1158aa2e0d64` (control)

**Files:**
- Modify: `web/src/components/settings/SettingsView.tsx`
- Modify: `web/src/components/settings/SettingsView.tsx#AiPane`
- Modify: `web/src/lib/api.ts#secretSave`
- Modify: `src-tauri/src/secret.rs#secret_save`
- Modify: `web/src/components/settings/SettingsView.tsx#SettingsView`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-c51942132f080af2 enter/save a BYOK API key`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-66d58acb1e9d254b save a BYOK API key`

**Candidate-bound contracts:**

#### control-record-b9070bce133b034f

- Candidate/source: `control-c51942132f080af2` at `web/src/components/settings/SettingsView.tsx:577:11` (control)
- Expected behavior: enter/save a BYOK API key: onChange updates draft/clears error; Enter calls save() only when trimmed draft is non-empty and not busy
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-c51942132f080af2.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-c51942132f080af2 enter/save a BYOK API key.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {(e) => { setDraft(e.target.value); setError(null); }} {(e) => { if (e.key === \"Enter\") void save(); }}","current control value and deterministic replacement value"]; handler={(e) => { setDraft(e.target.value); setError(null); }} {(e) => { if (e.key === "Enter") void save(); }}.
  - Exact call/state/backend: stateTransition=onChange updates draft/clears error; Enter calls save() only when trimmed draft is non-empty and not busy; backendTrace=["web/src/components/settings/SettingsView.tsx:577::candidate handler -> {(e) => { setDraft(e.target.value); setError(null); }} {(e) => { if (e.key === \"Enter\") void save(); }}","actual branch/state -> onChange updates draft/clears error; Enter calls save() only when trimmed draft is non-empty and not busy","exact call/arguments -> onChange only updates draft/error; Enter calls save() -> secretSave(provider,draft.trim()) when non-empty/not busy","web/src/components/settings/SettingsView.tsx::AiPane key input onChange/onKeyDown -> local draft or save exact chain","web/src/lib/api.ts::secretSave -> invoke('secret_save',{provider,key:trimmed})","src-tauri/src/secret.rs::secret_save(provider,key) -> OS keychain","code:web/src/components/settings/SettingsView.tsx#SettingsView","code:web/src/components/settings/SettingsView.tsx#AiPane","code:web/src/lib/api.ts#secretSave","code:src-tauri/src/secret.rs#secret_save"].
  - Visible/accessibility/return path: success=enter/save a BYOK API key: onChange updates draft/clears error; Enter calls save() only when trimmed draft is non-empty and not busy; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"enter/save a BYOK API key: onChange updates draft/clears error; Enter calls save() only when trimmed draft is non-empty and not busy","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/SettingsView.tsx:577; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in onChange updates draft/clears error; Enter calls save() only when trimmed draft is non-empty and not busy.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/settings/SettingsView.tsx:577; the missing DOM test must prove whether it is surfaced or silent."}.

#### control-record-7ddb1158aa2e0d64

- Candidate/source: `control-66d58acb1e9d254b` at `web/src/components/settings/SettingsView.tsx:600:13` (control)
- Expected behavior: save a BYOK API key: click calls save(); busy -> secretSave(provider,trimmed) -> masked status/draft clear or visible error
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-66d58acb1e9d254b.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-66d58acb1e9d254b save a BYOK API key.
  - Initial state: visibility=Only when trimmed draft is non-empty; enabledWhen=not ({busy}).
  - Event: inputs=["event/prop handler: {() => void save()}","click or native keyboard activation plus current owning state"]; handler={() => void save()}.
  - Exact call/state/backend: stateTransition=click calls save(); busy -> secretSave(provider,trimmed) -> masked status/draft clear or visible error; backendTrace=["web/src/components/settings/SettingsView.tsx:600::candidate handler -> {() => void save()}","actual branch/state -> click calls save(); busy -> secretSave(provider,trimmed) -> masked status/draft clear or visible error","exact call/arguments -> save() -> secretSave(provider,draft.trim()) when non-empty/not busy; set returned masked status and clear draft","web/src/components/settings/SettingsView.tsx::AiPane Save button -> save -> secretSave(provider,trimmed)","web/src/lib/api.ts::secretSave -> invoke('secret_save',{provider,key:trimmed})","src-tauri/src/secret.rs::secret_save(provider,key) -> OS keychain","code:web/src/components/settings/SettingsView.tsx#SettingsView","code:web/src/components/settings/SettingsView.tsx#AiPane","code:web/src/lib/api.ts#secretSave","code:src-tauri/src/secret.rs#secret_save"].
  - Visible/accessibility/return path: success=save a BYOK API key: click calls save(); busy -> secretSave(provider,trimmed) -> masked status/draft clear or visible error; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"save a BYOK API key: click calls save(); busy -> secretSave(provider,trimmed) -> masked status/draft clear or visible error","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/SettingsView.tsx:600; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"Disabled when {busy}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in click calls save(); busy -> secretSave(provider,trimmed) -> masked status/draft clear or visible error.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/settings/SettingsView.tsx:600; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-c51942132f080af2 enter/save a BYOK API key` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.
  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-66d58acb1e9d254b save a BYOK API key` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-c51942132f080af2 enter/save a BYOK API key"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-66d58acb1e9d254b save a BYOK API key"`

  Expected: FAIL because one or more of the 2 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/SettingsView.tsx`, `web/src/components/settings/SettingsView.tsx#AiPane`, `web/src/lib/api.ts#secretSave`, `src-tauri/src/secret.rs#secret_save`, `web/src/components/settings/SettingsView.tsx#SettingsView` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-c51942132f080af2 enter/save a BYOK API key"`
  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-66d58acb1e9d254b save a BYOK API key"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 43: control-acceptance (implementation-slice-f762c5a4c5d21e50)

**Covered records:**
- `control-record-3b0e37cc3779e83a` (control)

**Files:**
- Modify: `web/src/components/settings/SettingsView.tsx`
- Modify: `web/src/components/settings/SettingsView.tsx#AiPane`
- Modify: `web/src/lib/api.ts#secretDelete`
- Modify: `src-tauri/src/secret.rs#secret_delete`
- Modify: `web/src/components/settings/SettingsView.tsx#SettingsView`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-6c6ad1d93d3c1f6c delete a BYOK API key`

**Candidate-bound contracts:**

#### control-record-3b0e37cc3779e83a

- Candidate/source: `control-6c6ad1d93d3c1f6c` at `web/src/components/settings/SettingsView.tsx:620:15` (control)
- Expected behavior: delete a BYOK API key: busy -> secretDelete -> empty status or visible error
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-6c6ad1d93d3c1f6c.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-6c6ad1d93d3c1f6c delete a BYOK API key.
  - Initial state: visibility=Only when a key exists and draft is empty; enabledWhen=not ({busy}).
  - Event: inputs=["event/prop handler: {() => void remove()}","click or native keyboard activation plus current owning state"]; handler={() => void remove()}.
  - Exact call/state/backend: stateTransition=busy -> secretDelete -> empty status or visible error; backendTrace=["web/src/components/settings/SettingsView.tsx:620::candidate handler -> {() => void remove()}","actual branch/state -> busy -> secretDelete -> empty status or visible error","exact call/arguments -> remove(): secretDelete(provider); set returned empty status and clear draft","web/src/components/settings/SettingsView.tsx::AiPane.remove -> secretDelete(provider)","web/src/lib/api.ts::secretDelete -> invoke('secret_delete',{provider})","src-tauri/src/secret.rs::secret_delete(provider) -> OS keychain","code:web/src/components/settings/SettingsView.tsx#SettingsView","code:web/src/components/settings/SettingsView.tsx#AiPane","code:web/src/lib/api.ts#secretDelete","code:src-tauri/src/secret.rs#secret_delete"].
  - Visible/accessibility/return path: success=delete a BYOK API key: busy -> secretDelete -> empty status or visible error; accessibility={"focus":"Native keyboard-focusable control","label":"t(\"settings.byokDelete\")","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"delete a BYOK API key: busy -> secretDelete -> empty status or visible error","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/SettingsView.tsx:620; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"Disabled when {busy}.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in busy -> secretDelete -> empty status or visible error.","failure":"Failure behavior is limited to the catch/void-call behavior visible in web/src/components/settings/SettingsView.tsx:620; the missing DOM test must prove whether it is surfaced or silent."}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-6c6ad1d93d3c1f6c delete a BYOK API key` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-6c6ad1d93d3c1f6c delete a BYOK API key"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/SettingsView.tsx`, `web/src/components/settings/SettingsView.tsx#AiPane`, `web/src/lib/api.ts#secretDelete`, `src-tauri/src/secret.rs#secret_delete`, `web/src/components/settings/SettingsView.tsx#SettingsView` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-6c6ad1d93d3c1f6c delete a BYOK API key"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast`

  Expected: PASS with no new warnings or unrelated changes.

### Task 44: control-acceptance (implementation-slice-02c98bb5d622c1e6)

**Covered records:**
- `control-record-ce3a8bcf735f8c95` (control)

**Files:**
- Modify: `web/src/components/settings/SettingsView.tsx`
- Modify: `web/src/components/settings/SettingsView.tsx#McpPane`
- Modify: `web/src/components/settings/SettingsView.tsx#SettingsView`
- Test (reviewed-planned): `web/src/components/settings/SettingsView.interaction.test.tsx#control-bc7e07c8d75e1cd2 copy an MCP setup value`

**Candidate-bound contracts:**

#### control-record-ce3a8bcf735f8c95

- Candidate/source: `control-bc7e07c8d75e1cd2` at `web/src/components/settings/SettingsView.tsx:769:13` (control)
- Expected behavior: copy an MCP setup value: clipboard.writeText -> transient copied indicator; rejection is swallowed
- Resolution: `control-acceptance` — The control acceptance contract explicitly names this owning component test runner.
- Exact acceptance contract:
  - Candidate: control-bc7e07c8d75e1cd2.
  - Test: web/src/components/settings/SettingsView.interaction.test.tsx#control-bc7e07c8d75e1cd2 copy an MCP setup value.
  - Initial state: visibility=Visible whenever the owning component/section is rendered.; enabledWhen=No explicit disabled gate on this candidate..
  - Event: inputs=["event/prop handler: {() => copy(row.key, row.code)}","click or native keyboard activation plus current owning state"]; handler={() => copy(row.key, row.code)}.
  - Exact call/state/backend: stateTransition=clipboard.writeText -> transient copied indicator; rejection is swallowed; backendTrace=["web/src/components/settings/SettingsView.tsx:769::candidate handler -> {() => copy(row.key, row.code)}","actual branch/state -> clipboard.writeText -> transient copied indicator; rejection is swallowed","exact call/arguments -> navigator.clipboard.writeText(row.code); on success setCopiedKey(row.key) and clear it after 1500ms; rejection is swallowed","web/src/components/settings/SettingsView.tsx::McpPane.copy(key,text) -> navigator.clipboard.writeText(text)","API/Tauri/Rust -> N/A for MCP copy","code:web/src/components/settings/SettingsView.tsx#SettingsView","code:web/src/components/settings/SettingsView.tsx#McpPane"].
  - Visible/accessibility/return path: success=copy an MCP setup value: clipboard.writeText -> transient copied indicator; rejection is swallowed; accessibility={"focus":"Native keyboard-focusable control","label":"Visible child text/title or caller-provided label; verify at runtime","shortcut":"None declared on this control"}; returnPath=["Done/Close returns to the invoking Home/editor surface; menus should return focus to their trigger."].
  - Outcome matrix: {"success":"copy an MCP setup value: clipboard.writeText -> transient copied indicator; rejection is swallowed","pending":"Pending behavior is the concrete busy/phase/disabled state in web/src/components/settings/SettingsView.tsx:769; no additional state is inferred beyond the source.","empty":"Not applicable — this control does not consume a collection, selection, or free-form payload that has an empty state.","disabled":"No explicit disabled prop on this candidate.","cancel":"Not applicable — this immediate handler has no cancellable operation or dismissible transient surface.","retry":"Retry is another activation after the source-defined pending guard clears; no separate retry command exists unless named in clipboard.writeText -> transient copied indicator; rejection is swallowed.","failure":"Clipboard denial is a silent no-op"}.

- [ ] **Step 1: Write or extend every reviewed owning test**

  - `web/src/components/settings/SettingsView.interaction.test.tsx#control-bc7e07c8d75e1cd2 copy an MCP setup value` (reviewed-planned) — The control acceptance contract explicitly names this owning component test runner.

  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.

- [ ] **Step 2: Run all focused tests and verify RED**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-bc7e07c8d75e1cd2 copy an MCP setup value"`

  Expected: FAIL because one or more of the 1 candidate-bound contracts are not yet satisfied.

- [ ] **Step 3: Implement the minimal vertical slice**

  Modify only `web/src/components/settings/SettingsView.tsx`, `web/src/components/settings/SettingsView.tsx#McpPane`, `web/src/components/settings/SettingsView.tsx#SettingsView` as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.

- [ ] **Step 4: Run all focused tests and verify GREEN**

  - Run: `pnpm -C web test -- --run src/components/settings/SettingsView.interaction.test.tsx -t "control-bc7e07c8d75e1cd2 copy an MCP setup value"`

  Expected: PASS with every candidate-bound assertion executed.

- [ ] **Step 5: Run the subsystem regression gate**

  Run: `pnpm -C web test -- --run && pnpm -C web build`

  Expected: PASS with no new warnings or unrelated changes.
