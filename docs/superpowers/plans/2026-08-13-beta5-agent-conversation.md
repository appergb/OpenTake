# OpenTake Beta 5 Agent Conversation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the timeline Agent into a stable continuous conversation whose text and tools render in authoritative order, whose streams never cross sessions, and whose destructive timeline results include a real composited PNG.

**Architecture:** Rust chat messages remain the persisted truth, but streaming events address an explicit message and block. The Web store applies those events only to the active matching session and the panel renders blocks as one borderless assistant turn. Timeline mutations are observed at the dispatcher boundary; the transition from visible content to empty requests a bounded PNG from the Rust compositor and adds it to the matching tool-result block.

**Tech Stack:** Rust/Serde, OpenTake dispatcher and compositor, Tauri events, React/TypeScript, Zustand, Vitest.

## Global Constraints

- Remove the Agent-local Chat/Motion mode completely; Motion Studio is a separate app view.
- Treat `ChatMessage.blocks` as the only render order when present; legacy `content` and `toolCalls` exist only for wire compatibility and migration.
- Assistant turns have no enclosing bubble or card border. Tool detail disclosure stays inline with the same response.
- Every stream mutation identifies both `sessionId` and `messageId`; stale or inactive sessions may persist independently but cannot alter the visible draft.
- Generate timeline images through Rust timeline/compositor code, never by screenshotting the WebView.
- Bound image dimensions and encoded bytes and keep image data out of text logs.

---

### Task 1: Make block order explicit in the Rust chat event protocol

**Files:**
- Modify: `crates/opentake-agent/src/chat/session.rs`
- Modify: `crates/opentake-agent/src/chat/loop.rs`
- Modify: `crates/opentake-agent/src/chat/mod.rs`
- Modify: `src-tauri/src/chat.rs`

**Interfaces:**
- stable `AgentContentBlock` serialization for text, tool use, and tool result
- `LoopEvent::BlockDelta { session_id, message_id, block_index, delta }`
- `LoopEvent::BlockUpsert { session_id, message_id, block_index, block }`
- `LoopEvent::Done { session_id, message_id, message }`

- [ ] **Step 1: Write failing order and migration tests**

  Test an assistant message ordered as text A, tool use 1, text B, tool use 2; a tool result containing text and image content; round-trip serialization; and deserialization of legacy content/toolCalls into the equivalent stable order. Test that `refresh_legacy_fields` derives compatibility fields without reordering blocks.

- [ ] **Step 2: Verify RED**

  Run `cargo test -p opentake-agent chat::session::tests::blocks_ chat::loop::tests::events_ -- --nocapture`. Expected: block-addressed events and ordered construction helpers are missing.

- [ ] **Step 3: Implement ordered block mutation**

  Add constructors and mutation methods that append text/tool blocks in event order and consolidate only adjacent text deltas. Keep legacy field derivation one-way from blocks. Extend loop events with stable message ids generated before the first delta and preserve those ids in the final message.

- [ ] **Step 4: Adapt Tauri emission and Codex compatibility paths**

  Emit the new event payloads from both normal provider streaming and Codex execution. Ensure every error/cancellation finalizes the same message id. Retain a temporary decoder for prior event fields only where an already-open Beta 4 window can receive them during development.

- [ ] **Step 5: Verify GREEN**

  Run `cargo test -p opentake-agent chat:: -- --nocapture` and `cargo test -p opentake-tauri chat::tests --lib`.

- [ ] **Step 6: Commit the protocol**

  Commit as `refactor(agent): stream authoritative ordered content blocks`.

### Task 2: Make the front-end chat store session- and block-safe

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/api.ts`
- Modify: `web/src/store/chatStore.ts`
- Modify: `web/src/store/chatStore.test.ts`

**Interfaces:**
- `beginMessage(sessionId, messageId)`
- `appendBlockDelta(sessionId, messageId, blockIndex, delta)`
- `upsertBlock(sessionId, messageId, blockIndex, block)`
- `finalize(sessionId, messageId, message)`

- [ ] **Step 1: Write failing reducer tests**

  Cover text/tool/text order, multiple tool rounds, duplicate retry events, out-of-order block indices, final replacement, switching sessions mid-stream, deleting a session mid-stream, and late events from a previous session. Assert no tool call can merge into the nearest unrelated assistant message.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/store/chatStore.test.ts`. Expected: current nearest-assistant merging fails the session isolation and block-order cases.

- [ ] **Step 3: Replace proximity matching with exact identity matching**

  Track drafts by session/message identity, apply immutable block updates, ignore malformed negative/huge indices, and preserve inactive session drafts separately until persisted history is reloaded. Derive visible `messages` only for the selected session.

- [ ] **Step 4: Wire the typed event decoder**

  Validate event discriminants and required ids before dispatch. On a sequence gap or malformed payload, stop applying that message and request authoritative history instead of guessing an order.

- [ ] **Step 5: Verify GREEN**

  Run `pnpm -C web test -- src/store/chatStore.test.ts src/components/agent/AgentPanel.persistence.test.tsx` and `pnpm -C web build`.

- [ ] **Step 6: Commit the store**

  Commit as `fix(agent): isolate ordered streams by session and message`.

### Task 3: Render one continuous borderless assistant turn

**Files:**
- Modify: `web/src/components/agent/AgentPanel.tsx`
- Modify: `web/src/components/agent/AgentPanel.persistence.test.tsx`
- Create: `web/src/components/agent/AgentConversation.test.tsx`
- Modify: `web/src/styles/components.css`
- Modify: `web/src/i18n/dict.ts`

**Interfaces:**
- `AssistantTurn` renders blocks sequentially.
- `InlineToolActivity` exposes collapsed status and accessible expanded arguments/results/images.
- User messages keep a quiet surface; assistant messages do not.

- [ ] **Step 1: Write failing DOM/order tests**

  Render text A, tool use, tool result image, text B and assert exact DOM order, no assistant bubble class, no tool card border class, accessible expand/collapse, error state, and reduced-motion behavior. Assert the Chat/Motion tablist and its stored mode are absent.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/components/agent/AgentConversation.test.tsx src/components/agent/AgentPanel.persistence.test.tsx`. Expected: current bubble/card and panel-mode tests fail.

- [ ] **Step 3: Build the continuous renderer**

  Replace the split content/tool rendering with a block switch. Keep tool rows on the text baseline, animate only detail height/opacity, render image blocks with constrained dimensions and alt text, and use live status labels without adding card chrome.

- [ ] **Step 4: Remove Motion mode from AgentPanel**

  Delete local mode state, tab controls, motion-specific conditional content, and persistence keys. Keep current session selection/input operational across top-level app navigation.

- [ ] **Step 5: Verify GREEN**

  Run the two focused test files, `pnpm -C web test -- src/components/agent`, and `pnpm -C web build`.

- [ ] **Step 6: Commit the conversation UI**

  Commit as `feat(agent): render tools inline in continuous replies`.

### Task 4: Attach a real PNG when the last visible timeline content is deleted

**Files:**
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs`
- Modify: `crates/opentake-agent/src/mcp/media_bridge.rs`
- Modify: `src-tauri/src/mcp.rs`
- Modify: `src-tauri/src/render.rs`
- Modify: `src-tauri/src/chat.rs`

**Interfaces:**
- mutation receipt records visible clip count before/after execution
- `MediaBridge::capture_timeline_result(request) -> AgentToolResultContentBlock::Image`
- explicit empty-canvas compositor input with project width, height, fps, and playhead timecode

- [ ] **Step 1: Write failing mutation receipt tests**

  Test deletion from one visible clip to zero, deletion that leaves another visible clip, non-visual mutations, failure/rollback, undo, and batched delete. Only the successful visible-to-empty transition must require an image.

- [ ] **Step 2: Write failing compositor tests**

  Create a small deterministic empty project and assert returned bytes decode as PNG, dimensions are bounded, pixels include the canvas background and semantic empty-state overlay, and the tool result carries the image after its text summary. Add a non-empty fixture proving the authoritative compositor path is used.

- [ ] **Step 3: Verify RED**

  Run `cargo test -p opentake-agent mcp::dispatch::tests::timeline_image_ -- --nocapture` and `cargo test -p opentake-tauri render::tests::empty_timeline_ --lib`. Expected: mutation receipts and empty-canvas PNG capture are absent.

- [ ] **Step 4: Implement post-commit capture**

  Compare Rust timeline visibility before and after an admitted successful mutation. After commit, call the compositor at the current clamped playhead; for zero visible clips, render the explicit project canvas with timecode and localized-neutral empty marker. PNG-encode with the existing image crate, cap dimensions/bytes, and add the image to the same tool result.

- [ ] **Step 5: Keep failure semantics atomic**

  A capture failure must not roll back a successful edit, but it must append a sanitized warning block. A failed edit, cancelled edit, or stale-project edit must never return a success image.

- [ ] **Step 6: Verify GREEN**

  Run the focused tests, `cargo test -p opentake-agent mcp:: -- --nocapture`, and `cargo test -p opentake-tauri chat::tests render::tests --lib`.

- [ ] **Step 7: Commit timeline result images**

  Commit as `feat(agent): show composited result after clearing timeline`.

### Task 5: Verify conversation behavior in the packaged application

**Files:**
- Create: `docs/audit/2026-08-13/beta5-agent-conversation.md`
- Create: `docs/audit/2026-08-13/screenshots/agent-continuous-conversation.png`
- Create: `docs/audit/2026-08-13/screenshots/agent-empty-timeline-result.png`

- [ ] **Step 1: Run all automated Agent gates**

  Run `cargo test -p opentake-agent`, `cargo test -p opentake-tauri chat::tests --lib`, `pnpm -C web test -- src/store/chatStore.test.ts src/components/agent`, and `pnpm -C web build`.

- [ ] **Step 2: Exercise a real multi-tool conversation**

  In the packaged Tauri app, execute a request that produces text, at least two tools, and final text. Switch sessions during a stream and return. Confirm order, no cross-session mutation, inline disclosure, and no standalone tool cards.

- [ ] **Step 3: Exercise clear-to-empty**

  Place visible content, ask Agent to delete it, and confirm the ordered tool result includes a decodable PNG showing the empty project canvas rather than an empty JSON result or WebView screenshot.

- [ ] **Step 4: Record evidence and commit**

  Record exact commands and observed packaged-app behavior, include the two screenshots, and commit as `test(agent): verify Beta 5 continuous conversation`.
