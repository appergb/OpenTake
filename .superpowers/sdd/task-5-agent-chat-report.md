# Task 5D Report: `feat/agent-chat-panel`

## Branch decision

- Direct merge of `feat/agent-chat-panel` was rejected.
- Evidence:
  - `git rev-list --left-right --count origin/main...feat/agent-chat-panel` -> `154 1`
  - `git log --oneline --no-merges origin/main..feat/agent-chat-panel` -> `dd9f224 feat(agent): chat panel with streaming + tool dispatch (#HANDOFF-3.3)`
  - `git diff --name-status origin/main..feat/agent-chat-panel` -> broad stale deletions across docs/specs plus unrelated core/media/preview churn
  - `git diff --stat origin/main..feat/agent-chat-panel` -> `135 files changed, 2669 insertions(+), 10289 deletions(-)`
- Integration method: selective replay from `dd9f224` onto current `recovery/superpowers-integration-20260708-v2` / HEAD.

## What was replayed and adapted

- Replayed `crates/opentake-agent/src/chat/{mod,session,llm,loop}.rs` and exported `pub mod chat` from `crates/opentake-agent/src/lib.rs`.
- Replayed `src-tauri/src/chat.rs` and spliced chat state/commands into `src-tauri/src/lib.rs`.
- Factored `src-tauri/src/mcp.rs` so chat reuses the current workflow registry + media bridge construction rather than a stale no-bridge path.
- Replayed `web/src/store/chatStore.ts`, `web/src/components/agent/AgentPanel.tsx`, and minimal chat additions in `web/src/lib/api.ts`, `web/src/lib/types.ts`, and `web/src/i18n/dict.ts`.
- Added `web/src/store/chatStore.test.ts`.

## Current-head adaptations

- Provider handling is explicit now: `chat_send` takes `chatProvider`, matching the current Settings provider choice.
- Chat no longer auto-picks another provider when the selected one has no key.
- Unsupported `google` provider fails clearly with guidance to choose OpenAI or Anthropic.
- Chat uses the current Tauri media bridge path, and the tool catalog hides bridge-only tools when the dispatcher lacks that bridge.
- No key storage was duplicated; chat reads through the existing key-store boundary and remains aligned with the `secret_*` commands.
- The Agent panel opens the current Settings modal (`setSettingsOpen(true)`) for no-key guidance.

## Tests

- `cargo test -p opentake-agent chat -- --nocapture`
- `cargo test -p opentake-agent mcp::dispatch -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml -p opentake-tauri chat --lib -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml -p opentake-tauri secret --lib -- --nocapture`
- `pnpm -C web test -- src/store/chatStore.test.ts`
- `pnpm -C web exec tsc -b --pretty false`
- `git diff --check` on touched files: clean

## Files changed

- `crates/opentake-agent/src/chat/mod.rs`
- `crates/opentake-agent/src/chat/session.rs`
- `crates/opentake-agent/src/chat/llm.rs`
- `crates/opentake-agent/src/chat/loop.rs`
- `crates/opentake-agent/src/lib.rs`
- `crates/opentake-agent/src/mcp/dispatch.rs`
- `src-tauri/src/chat.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/mcp.rs`
- `web/src/store/chatStore.ts`
- `web/src/store/chatStore.test.ts`
- `web/src/components/agent/AgentPanel.tsx`
- `web/src/lib/api.ts`
- `web/src/lib/types.ts`
- `web/src/i18n/dict.ts`
- `docs/superpowers/archive/2026-07-08-branch-integration-register.md`
- `.superpowers/sdd/task-5-agent-chat-report.md`

## Commit SHA

- `4bd7208` (`feat: replay agent chat panel from branch queue`)

## Concerns

- The Settings UI still offers `google`, but chat intentionally rejects it until a Google chat transport is added.
- No focused `AgentPanel` DOM interaction test was added; the current node setup comfortably covered `chatStore` behavior and TypeScript integration, but not effect-driven panel callbacks without introducing a new DOM test harness.
