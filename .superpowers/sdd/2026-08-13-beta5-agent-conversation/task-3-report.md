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
