# Task 2 report — session- and block-safe chat store

Date: 2026-08-14 (Asia/Shanghai)

Base: `d44787f`

Branch: `release/v1.0.0-beta.5`

## Result

- Replaced proximity-based assistant updates with drafts keyed by the exact
  `(sessionId, messageId)` pair. The visible `messages`, `streaming`, and
  `streamingId` fields are now derived only from the selected session.
- Added ordered, immutable block reducers: `beginMessage`,
  `appendBlockDelta`, `upsertBlock`, and ID-addressed `finalize`. Text/tool/text
  and multi-round tool streams retain their authoritative block positions;
  final messages replace drafts wholesale.
- Added bounded stream IDs, block indices, and deltas; strict Tauri event
  decoding returns a discriminated event or a structured malformed-payload
  failure. Gaps and malformed messages stop the affected stream and enqueue one
  authoritative-history re-sync per session, without retry loops.
- Retained Beta 4 store and `toolCall` adapter methods temporarily for an
  already-open window. Task 3 must wire the new identity reducers, malformed
  callback/re-sync queue, session-history replacement, and listener teardown in
  `AgentPanel`.
- Added tests for block order, multiple rounds, duplicate delivery, gaps,
  bounded indices, authoritative final replacement, inactive/deleted/late
  session isolation, explicit no-nearest-assistant merge, and decoder validity.

## TDD and verification

RED was observed with the requested focused command after adding the reducer
tests: 8 tests failed because `beginMessage`, the block reducers, the deleted
session isolation API, and `decodeChatStreamEvent` did not exist.

GREEN verification on the final tree:

- `pnpm -C web test -- src/store/chatStore.test.ts src/components/agent/AgentPanel.persistence.test.tsx`
  — 144 files, 1269 tests passed.
- `pnpm -C web build` — passed (`tsc -b` and Vite production build).
- `git diff --check` — passed.

The build retains pre-existing Vite warnings about ineffective dynamic imports
and a chunk above 500 kB; neither warning is introduced by Task 2 and both
commands exit successfully.

## Scope review

Owned changes are limited to:

- `web/src/lib/types.ts`
- `web/src/lib/api.ts`
- `web/src/store/chatStore.ts`
- `web/src/store/chatStore.test.ts`
- this report

Concurrent Rust protocol, core/project/render, commands/home, and audit files
were not staged, changed, or reverted. A code-reviewer agent was requested but
the shared agent limit was full; the coordinator will run the independent
review after this task releases its slot.
