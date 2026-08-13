# Task 1 report — ordered Rust chat block protocol

Date: 2026-08-14 (Asia/Shanghai)

Base: `26c6182`

Branch: `release/v1.0.0-beta.5`

## Result

- `ChatMessage.blocks` is authoritative. Ordered constructors and mutations preserve interleaved text/tool blocks, consolidate only adjacent text deltas, and derive Beta 4 `content` / `toolCalls` fields one-way through `refresh_legacy_fields`.
- Legacy messages without blocks migrate deterministically to text followed by their persisted tool-call order. Native tool-result text/image blocks retain their exact serialization order.
- `LoopEvent` now exposes block-addressed `BlockDelta` and `BlockUpsert` variants and an ID-addressed `Done`. The normal provider loop mints the message ID before streaming and reuses the active ID for completion, cancellation, and provider errors.
- The Tauri normal-provider and Official Codex paths emit `sessionId`, `messageId`, and `blockIndex`. Codex tool updates and final text share one pre-minted message ID. Cancellation, history-save errors, and other terminal failures reuse the active message ID.
- During the Beta 4 development-window transition, Tauri retains the existing `chat_delta` / `chat_tool_call` event names and the legacy `toolCall` field for tool-use upserts; Beta 5 decoders additionally consume the authoritative block address and block payload.

## TDD evidence

RED was observed before implementation. The plan's literal command has two positional Cargo test filters and Cargo rejected the second filter. Running the filters separately produced the intended compile failures: missing ordered constructors/mutations, missing `LoopEvent::BlockDelta` / `BlockUpsert`, and missing `Done.message_id`.

GREEN verification on the final tree:

- `cargo test -p opentake-agent chat:: -- --nocapture` — 47 passed, 0 failed.
- `cargo test -p opentake-tauri chat::tests --lib` — 19 passed, 0 failed.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p opentake-agent --all-targets -- -D warnings` — passed.
- `cargo clippy -p opentake-tauri --lib -- -D warnings` — passed.
- `git diff --check` for the four owned chat files — passed.

The combined Tauri all-target clippy gate was also attempted. It is blocked outside Task 1 by the concurrent unowned change at `src-tauri/src/commands.rs:2997` (`clippy::field_reassign_with_default`). This task did not modify or revert that file.

## Scope review

Owned changes are limited to:

- `crates/opentake-agent/src/chat/session.rs`
- `crates/opentake-agent/src/chat/loop.rs`
- `crates/opentake-agent/src/chat/mod.rs`
- `src-tauri/src/chat.rs`
- this report

Concurrent core, project, media, render, commands, home, and audit changes were not staged or reverted.

## Review fix round 1

Commit target: `fix(agent): preserve provider block order`

### Result

- The live Anthropic SSE decoder now treats provider `content_block_start`, `content_block_delta`, and `content_block_stop` indices as authoritative. It emits block upserts at start/stop and indexed text deltas between them, so `text A → tool use → text B` remains in that order through loop events and persistence.
- Anthropic `input_json_delta.partial_json` is accumulated on its addressed tool block. The live HTTP path and the deterministic SSE regression test share the same decoder.
- OpenAI and Anthropic request builders now derive text, tool calls, tool-use IDs, and native tool-result content directly from `ChatMessage.blocks`. In particular, the next Anthropic round serializes interleaved assistant blocks in their original order instead of reconstructing `text + tools` from Beta 4 compatibility fields.
- The deserialize wire uses `Option<Vec<AgentContentBlock>>`: only a missing `blocks` property migrates legacy fields. Explicit `blocks: []` remains empty, clears stale flat fields/tool metadata, and serializes back as a Beta 5 empty array.
- Large inline tests moved to `chat/session/tests.rs` and `chat/loop/tests.rs`; production `session.rs` is 533 lines and `loop.rs` is 610 lines.

### RED evidence

- The explicit-empty tests first failed with a missing serialized array (`null` versus `[]`) and with stale legacy text/tool calls being migrated into non-empty blocks.
- The authoritative Anthropic body test first failed with `text AB → tool` instead of `text A → tool → text B`.
- The interleaved SSE test first failed to compile because the indexed Anthropic decoder and shared loop event application path did not exist; after implementation it exercises split raw SSE chunks through loop events into the next-round request body.
- The explicit-empty tool-message test first failed because stale `toolCallId` survived an authoritative empty block array.

### Fresh verification

- `cargo test -p opentake-agent chat:: -- --nocapture` — 53 passed, 0 failed.
- `cargo test -p opentake-tauri chat::tests --lib -- --nocapture` — 19 passed, 0 failed.
- `cargo clippy -p opentake-agent --all-targets -- -D warnings` — passed.
- `cargo clippy -p opentake-tauri --lib -- -D warnings` — passed (only Cargo's existing future-incompatibility notice for `block v0.1.6`).
- `cargo fmt -p opentake-agent -- --check` — passed.
- Owned-file `git diff --check` — passed.
- `cargo fmt --all -- --check` was also run; its only diff is the concurrent unowned `crates/opentake-render/src/plan/build.rs` formatting change, so Task 1 did not rewrite it.

### Round 1 scope

- `crates/opentake-agent/src/chat/llm.rs`
- `crates/opentake-agent/src/chat/session.rs`
- `crates/opentake-agent/src/chat/session/tests.rs`
- `crates/opentake-agent/src/chat/loop.rs`
- `crates/opentake-agent/src/chat/loop/tests.rs`
- this report

## Review fix round 2

Commit target: `fix(agent): reject incomplete provider streams`

### Result

- Anthropic stream completion is now fail-closed: every opened content block must receive exactly one matching `content_block_stop`, the stream must receive exactly one `message_stop`, and EOF cannot finalize a partial text/tool block. Premature/repeated stops, deltas after block stop, and events after message stop return `LlmError::Stream`.
- A failed/truncated provider turn never reaches the loop's persistence or tool-dispatch phase; those phases remain after successful decoder finalization.
- `llm.rs` is now a 228-line façade. Provider production code lives in `chat/llm/openai.rs` (235 lines) and `chat/llm/anthropic.rs` (490 lines); tests live in `chat/llm/tests.rs` (401 lines). Every file is below 800 lines.
- Every Rust block delta/upsert/done carries a per-message monotonic `sequence: u64`, starting at 0 and increasing in emission order. Provider rounds, tool-result messages, guide/error/cancel terminal messages, save failures, and Official Codex events use the same contract.
- The Tauri payloads expose `sequence` in camelCase JSON and reject duplicate or gapped sequences per message before emitting to the window.

### RED evidence

- Seven Anthropic lifecycle tests initially accepted invalid partial streams: truncated text/tool blocks, missing `message_stop`, premature `message_stop`, repeated block/message stops, and a delta after block stop all returned partial `TurnResult` values.
- Sequence tests initially failed to compile because `LoopEvent`, `LoopError`, and the three Tauri payloads had no sequence field; the strict duplicate/gap gate did not exist.

### Fresh verification

- `cargo test -p opentake-agent chat:: -- --nocapture` — 60 passed, 0 failed.
- `cargo test -p opentake-tauri chat::tests --lib -- --nocapture` — 20 passed, 0 failed.
- `cargo clippy -p opentake-agent --all-targets -- -D warnings` — passed.
- `cargo clippy -p opentake-tauri --lib -- -D warnings` — passed (only Cargo's existing future-incompatibility notice for `block v0.1.6`).
- Direct `rustfmt --check` on the seven owned Rust chat files — passed.
- Owned-file `git diff --check` — passed.

### Round 2 scope

- `crates/opentake-agent/src/chat/llm.rs`
- `crates/opentake-agent/src/chat/llm/openai.rs`
- `crates/opentake-agent/src/chat/llm/anthropic.rs`
- `crates/opentake-agent/src/chat/llm/tests.rs`
- `crates/opentake-agent/src/chat/loop.rs`
- `crates/opentake-agent/src/chat/loop/tests.rs`
- `src-tauri/src/chat.rs`
- this report
