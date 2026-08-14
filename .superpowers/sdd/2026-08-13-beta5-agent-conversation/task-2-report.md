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

## Review fix round 3 — bounded exact canonical replay

- Replaced the two 32-bit hashes with the collision-free canonical event text.
  Canonicalization uses deterministic UTF-16 code-unit key order, rejects
  sparse arrays and non-JSON containers, and streams into a writer that aborts
  above the 1 MiB aggregate event limit. Image content uses the same individual
  ceiling, and each draft retains at most 64 entries and 1 MiB of canonical
  replay text in total.
- Split sequence-address preflight from payload comparison. Gaps and events
  older than the retained window now fail closed before deep payload validation
  or canonical construction; only a current event or a retained retry pays the
  bounded canonicalization cost.
- Extended terminal decoding and exact final replacement to Rust `role: "tool"`
  messages. Tool terminals require non-empty `toolResult` blocks, an empty
  `toolCalls` list, exact `toolCallId`/`toolUseId` identity, and aligned error
  state. Assistant terminals continue to reject tool-only blocks and fields.

## TDD and verification

The initial Task 2 RED was 8 failing reducer/decoder tests. For the first
sequence review fix, the expanded 18-case suite was run against the pre-fix
production code and produced 15 failures / 3 passes. Failures covered all newly
requested sequence, identity, deep-validation, and retention behaviors. For
review fix round 2, the three retry-window regressions were added before the
production change; the focused suite then produced 2 failures / 18 passes,
covering delayed exact retry and conflicting sequence reuse. Round 3 initially
produced 7 failures / 21 passes across the new bounded-canonical and tool
terminal cases; the corrected sparse-collision fixture was also run alone and
failed with the old implementation silently treating it as a retry.

GREEN verification on the final tree:

- `pnpm -C web exec vitest run src/store/chatStore.test.ts --reporter=verbose`
  — 28/28 focused tests passed.
- `pnpm -C web test -- src/store/chatStore.test.ts`
  — the earlier round-2 full run passed 1277/1278 tests. Its sole integration
  failure was the expected Task 3
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
