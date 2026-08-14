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

## Review fix — ordered event sequences (`931edb9` protocol)

- Mirrored Rust's per-message `sequence` on every decoded stream event. The
  reducer accepts only the exact next sequence and poisons only the addressed
  message on a gap or stale out-of-order event. Two identical deltas at
  consecutive valid sequences are both preserved; exact retry validation is
  detailed in review fix round 2 below.
- Separated per-message poison from per-session history re-sync de-duplication.
  A bad message cannot stop a sibling message in the same session, while at
  most one history reload request remains pending until authoritative history
  is installed.
- Made block discriminants and tool identities immutable at an occupied block
  index (`toolUse.id/name`, `toolResult.toolUseId`). Final assistant messages
  are deeply validated, bounded, exact-ID replacements.
- Bounded retained inactive histories, draft/final sequence records, deleted
  session tombstones, blocked keys, and pending re-sync state. Authoritative
  history clears all poison and sequence state for its session.

## Review fix round 2 — exact retry validation

- Retained a bounded 64-event replay fingerprint window on each message draft.
  Fingerprints cover the event discriminant, session/message address, block
  index, and canonical payload while storing only fixed-size hashes.
- Exact delayed retries inside the window are idempotent. Reusing a retained
  sequence with a different event kind, address, index, or payload poisons only
  that message with `sequence_conflict`; retries older than the retained window
  fail closed as `sequence_out_of_order`.
- Added focused regressions for delayed identical text, cross-kind/index/payload
  conflicts, and an exact retry outside the bounded replay window.

## TDD and verification

The initial Task 2 RED was 8 failing reducer/decoder tests. For the first
sequence review fix, the expanded 18-case suite was run against the pre-fix
production code and produced 15 failures / 3 passes. Failures covered all newly
requested sequence, identity, deep-validation, and retention behaviors. For
review fix round 2, the three retry-window regressions were added before the
production change; the focused suite then produced 2 failures / 18 passes,
covering delayed exact retry and conflicting sequence reuse.

GREEN verification on the final tree:

- `pnpm -C web exec vitest run src/store/chatStore.test.ts --reporter=verbose`
  — 20/20 focused tests passed.
- `pnpm -C web test -- src/store/chatStore.test.ts`
  — 1277/1278 tests passed. The sole integration failure is the expected Task 3
  migration point: `AgentPanel.persistence.test.tsx` still expects legacy
  `finalize(message)` to append an unaddressed Done event. The reviewed store
  now deliberately fails that overload closed unless it exactly matches an
  active message ID; Task 3 owns listener/reducer migration and the test update.
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
