# Task 3 report — continuous borderless Agent conversation

Date: 2026-08-14 (Asia/Shanghai)

Base: `64e3695`

Integrated protocol/store follow-ups: `5c214b8`, `5015b02`

Branch: `release/v1.0.0-beta.5`

## Result

- Replaced assistant bubbles and detached tool cards with one continuous,
  borderless `AssistantTurn`. Authoritative `ChatMessage.blocks` render in
  exact DOM order; only messages without `blocks` use the Beta 4 compatibility
  fields.
- Grouped adjacent assistant/tool/assistant messages into one visible turn, so
  native role=`tool` result blocks remain inline with the originating assistant
  tool round and its follow-up text. User messages retain a quiet, separate
  surface.
- Added inline tool disclosures through the shared `Reveal`: collapsed
  running/complete/failed live status, accessible expansion state and controlled
  region, readable arguments/results, bounded raster images with contextual alt
  text, and fail-closed MIME/base64 handling. Tool details animate only block
  size and opacity; reduced-motion disclosure is immediate and keeps trigger
  focus.
- Removed Agent-local Chat/Motion mode state, controls, conditional rendering,
  labels, and storage access. The selected chat session and unsent composer text
  survive top-level AgentPanel unmount/remount.
- Migrated `AgentPanel` from the legacy proximity/finalize path to exact
  `(sessionId, messageId, sequence, blockIndex)` reducers. All three listeners
  subscribe independently, filter project identity, accept inactive-session
  streams safely, and unsubscribe even when registration resolves after
  unmount.
- Malformed/gapped streams request authoritative history once per session and
  replace that session exactly. Re-syncs are bound to the project identity that
  observed the event, so a queued old-project request cannot load into a Save As
  replacement. The strict role=`tool` BlockUpsert/Done sequence finalizes without
  leaving an orphan assistant draft.
- Session switching uses store-backed per-session history, closed sessions are
  tombstoned, first-event/malformed handling releases the local pending-composer
  lock, and local send errors are installed as exact block-backed messages.
- Added visible focus rings and accessible composer/action labels. Error and
  tool status are expressed in text rather than color alone.

## TDD evidence

RED was observed before production changes with the brief's literal command:

- The new conversation tests failed because `AssistantTurn` did not exist and
  the old UI still rendered assistant bubbles, detached bordered tool cards,
  and the Agent-local Chat/Motion switch.
- The persistence regressions failed against legacy unaddressed listeners and
  finalize behavior, missing listener teardown/re-sync handling, and missing
  navigation retention.
- Additional regressions were added and observed RED before their fixes for
  native role=`tool` inline rendering, adjacent multi-round grouping,
  authoritative user blocks, malformed first-event composer release,
  old-project queued-gap isolation, composer accessibility, and strict
  BlockUpsert(seq 0) / role=`tool` Done(seq 1) finalization.

GREEN verification on the final integrated tree:

- `pnpm -C web exec vitest run src/components/agent/AgentConversation.test.tsx src/components/agent/AgentPanel.persistence.test.tsx --reporter=verbose`
  — 24/24 focused tests passed.
- `pnpm -C web exec vitest run src/components/agent --reporter=verbose`
  — 25/25 Agent component tests passed. The existing MotionPanel test still
  emits its pre-existing React `act(...)` warnings but passes.
- `pnpm -C web exec vitest run src/store/chatStore.test.ts --reporter=verbose`
  — 28/28 ordered-store/decoder tests passed.
- `pnpm -C web test -- src/components/agent/AgentConversation.test.tsx src/components/agent/AgentPanel.persistence.test.tsx`
  — 145/145 files and 1303/1303 tests passed (the current pnpm/Vitest command
  form runs the complete suite).
- `pnpm -C web test -- src/components/agent`
  — 145/145 files and 1303/1303 tests passed.
- `pnpm -C web build` — passed (`tsc -b` and Vite production build).
- `git diff --check` — passed.

The build retains existing Vite warnings for ineffective dynamic imports and a
chunk above 500 kB; neither warning is introduced by Task 3 and the build exits
successfully.

## Scope review

Owned changes are limited to:

- `web/src/components/agent/AgentPanel.tsx`
- `web/src/components/agent/AgentPanel.persistence.test.tsx`
- `web/src/components/agent/AgentConversation.test.tsx`
- `web/src/styles/components.css`
- `web/src/i18n/dict.ts`
- this report

Concurrent Rust core/project/render, Tauri commands/home, audit artifacts, and
other task files were not staged, changed, or reverted.

Commit target: `feat(agent): render tools inline in continuous replies`

## Review fix round 1 — authoritative re-sync

- Added `chat_history_authoritative`: a gap re-sync now waits for the exact
  project/session turn to cross its terminal event boundary, without retaining
  the turn-registry mutex across an async suspension, and only then reads the
  durable history. Project replacement wakes the waiter and fails closed on
  identity revalidation.
- Added project-generation and per-session version gates. A late startup
  `chat_sessions` response or authoritative re-sync cannot overwrite a session
  touched while the request was in flight, including an inactive session.
- Added atomic `resetProject(epoch, path)`, which clears all project-scoped
  histories, drafts, sequence poison, re-sync state, versions, and tombstones
  on a real identity change while preserving selection and composer state for
  an ordinary same-project panel remount.
- Validated all `chat_history`, `chat_history_authoritative`, and
  `chat_sessions` responses before they reach the store. Raster base64 is
  bounded by the shared `MAX_CHAT_IMAGE_BASE64_CHARS` ceiling at both decode
  and render boundaries.
- Tool disclosure status is now a live sibling described by the trigger;
  Escape closes the disclosure, stops propagation, and retains trigger focus.
- Deleted the dead `MotionPanel` implementation/test and its obsolete
  translation strings.

Review RED was observed before each production fix: active-turn gaps installed
stale persisted history, startup snapshots overwrote touched inactive sessions,
project replacement retained same-ID state, the disclosure lacked the required
Escape/live-region relationship, oversized raster data rendered, and the dead
Motion panel remained reachable on disk.

Final GREEN verification:

- Focused Agent/store/API command-contract: 4 files, 64/64 tests passed.
- Rust `chat::tests`: 23/23 passed, including the exact terminal-boundary wait.
- Full Web suite: 144 files, 1313/1313 tests passed.
- `cargo clippy -p opentake-tauri --lib -- -D warnings` passed.
- `cargo build -p opentake-tauri` passed.
- `pnpm -C web build` passed (`tsc -b` and Vite production build).
- `git diff --check` passed.

The existing Vite ineffective-dynamic-import and >500 kB chunk warnings, plus
the existing Rust `block v0.1.6` future-incompatibility notice, remain
non-failing and are outside Task 3.

Review-fix scope additionally owns `src-tauri/src/chat.rs`,
`src-tauri/src/lib.rs`, `web/src/lib/api.ts`, `web/src/lib/types.ts`,
`web/src/store/chatStore.ts` and its test, and the two deleted MotionPanel
files. Concurrent editor audit artifacts were preserved and not staged.

Review-fix commit target: `fix(agent): make conversation resync authoritative`

## Review fix round 2 — exact-turn snapshots and resumable re-sync

- Bound each authoritative request to the single `TurnCancel` owner observed at
  request time. The owner now completes with an immutable clone of the durable
  terminal history, so turn A returns its own snapshot even when turn B reserves
  the same session before the waiting command resumes.
- Made authoritative store installation independent of `AgentPanel` mount
  lifetime. Project-generation and per-session-version CAS still protect the
  write; rejected writes requeue the same poisoned session, and a remount
  resumes the queued request without allowing the startup session list to clear
  or overwrite it. The selected session composer stays disabled while re-sync
  is active.
- Split snapshot validation from the one-event ceiling: messages remain bounded
  to 1 MiB, histories to 8 MiB per session, and session lists to 32 MiB / 256
  sessions. Aggregate bytes are counted incrementally without canonicalizing a
  whole project-sized response at once.
- Integrated the deferred timeline-result gate needed by the empty-timeline
  result path. The edit commits under the exact project identity lease, GPU
  capture runs after that lease is released, and a final identity/cancellation
  check discards any result captured across a project replacement. Capture
  warnings remain non-transactional and never roll back the committed edit.

Round 2 RED evidence was observed before production fixes:

- The authoritative history regression timed out after turn A completed and
  turn B registered, showing the old loop had rebound the request to B.
- The unmount regression left no installed snapshot, the CAS rejection left no
  retry request, and the selected composer remained enabled during poison.
- A valid multi-message history above 1 MiB and a valid 256-session list were
  rejected by the former event-sized decoder.
- The blocking capture fixture showed Save As timing out while the old chat gate
  retained its project identity read lease across GPU work.

Round 2 GREEN verification on the integrated tree:

- `pnpm -C web exec vitest run src/components/agent/AgentPanel.persistence.test.tsx`
  — 21/21 passed.
- `pnpm -C web exec vitest run src/store/chatStore.test.ts`
  — 36/36 passed.
- `pnpm -C web test` — 144 files and 1317/1317 tests passed.
- `cargo test -p opentake-tauri --lib chat::tests` — 24/24 passed,
  including exact-turn ownership and the deferred-capture project transition.
- `cargo clippy -p opentake-tauri --lib -- -D warnings` passed.
- `cargo build -p opentake-tauri` passed.
- `pnpm -C web build` passed (`tsc -b` and Vite production build).
- `git diff --check` passed for all owned round 2 files.

`cargo test --workspace` advanced through the Agent, core, domain, media, and
motion unit suites before the live Chromium 4K smoke timed out after 180 seconds;
the two later Chromium cases then reported the shared gate as poisoned. These
three failures are confined to `opentake-motion/tests/chromium.rs` and do not
exercise the Task 3 chat/Web changes.

Round 2 commit target: `fix(agent): bind resync to exact turn snapshot`

## Final independent review — rejected authoritative request recovery

- Replaced the empty authoritative-history rejection handler with a
  mount-independent retry schedule. The request carries a bounded retry attempt
  and is requeued after exponential backoff from 250 ms to a 4 s ceiling.
- Requeue still passes through the store's exact project-generation,
  resyncing-session, and deleted-session guards. A project reset clears the
  poison and makes an old timer a no-op; an ordinary panel unmount does not own
  or discard the project-scoped repair.
- Added a component regression that rejects the first authoritative request,
  verifies there is no immediate retry loop and the composer remains locked,
  unmounts/remounts the panel, advances the retry clock, and observes the exact
  terminal snapshot installation and poison cleanup.
- Updated the chat test bridge for the concurrent cancellable timeline-capture
  trait signature; this is test-only plumbing with no chat behavior change.

The regression was observed RED before production changes: after 5 seconds of
virtual time the authoritative API still had only one call and the resync state
remained poisoned. Final verification:

- Focused Agent conversation/persistence/store: 3 files, 68/68 tests passed.
- Full Web suite: 144 files, 1318/1318 tests passed.
- `pnpm -C web build` passed (`tsc -b` and Vite production build).
- Rust `chat::tests`: 24/24 passed.
- `cargo clippy -p opentake-tauri --lib -- -D warnings` passed.
- `cargo build -p opentake-tauri` passed.
- Owned-file `git diff --check` passed.

Final-review commit target: `fix(agent): retry failed history resync`
