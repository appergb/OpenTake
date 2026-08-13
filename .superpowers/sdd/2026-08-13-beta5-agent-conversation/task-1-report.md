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
