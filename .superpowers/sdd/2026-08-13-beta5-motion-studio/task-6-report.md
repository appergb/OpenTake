# Motion Studio Task 6 report

Status: COMPLETE — independently reviewed and approved before commit.

## Scope

- Added document-bound Motion add/edit requests that resolve the exact saved
  HTML/CSS revision, compile that source through the same deterministic
  Chromium renderer used by preview, encode a validated MP4, and preserve only
  the document id/revision hash as project provenance.
- Added project-authority, revision, dimensions, duration, cancellation, and
  source/timeline FPS validation. A 10 fps six-frame document published into a
  20 fps timeline becomes a twelve-frame clip without changing duration.
- Added one-step atomic add and exact clip replacement. Failed render, probe,
  cancellation, stale revision, project switch, Save As, or whole-bundle
  replacement cannot mutate the timeline/manifest or leave a project orphan.
- Serialized retained media creation, copy, file sync, Unix directory sync,
  identity verification, and durable core commit under the established
  publication → project-identity-read → session lock order.
- Added deferred core events so retained-file rollback is disarmed and both
  publication locks are released before synchronous subscribers run. EventBus
  now isolates a panicking listener from committed commands and later mirrors.
- Added real per-frame progress from Chromium frame writes/cache hits through a
  tagged Tauri event, strict TypeScript decoder, operation/terminal CAS, store,
  bilingual button text, and an accessible live status.
- Added publish/save/preview/conflict gates, cancellation reconciliation,
  project-boundary blocking, exact committed-clip selection, and lifecycle,
  project, and current-view CAS before navigating back to the editor.

## TDD evidence

Initial RED evidence included missing document Motion request/bridge contracts,
three absent Web publish behaviors, stale document/dimension rejection, and the
live publishing integration target.

Review-driven RED regressions additionally proved:

- a complete generation-bundle replacement could interleave with a partial
  Motion media copy;
- Save As could copy an incomplete orphan without an identity read lease;
- deferred events were required to avoid publication-lock re-entry deadlock and
  rollback of an already-referenced media file;
- a panicking event subscriber turned an already-durable commit into an
  apparent command failure;
- leaving Motion or unmounting before a late success navigated back to the
  editor and selected the clip;
- the UI exposed only coarse phases rather than completed/total render frames.

Every regression was observed failing before its production fix. The final
publication tests exercise a real complete-bundle replacement and real Save As
copy, then reopen the destination and verify both the manifest reference and
the exact media bytes.

## Final fresh verification

```text
cargo test -p opentake-core --lib
76/76 passed

cargo test -p opentake-agent --test advertised_tool_acceptance motion -- --nocapture
2/2 passed

cargo test -p opentake-agent mcp:: -- --nocapture
184/184 passed

cargo test -p opentake-tauri motion::tests --lib -- --nocapture
12/12 passed

cargo test -p opentake-tauri --test motion_command -- --nocapture
1/1 passed; live 4-frame progress was 0 → 1 → 2 → 3 → 4

OPENTAKE_RUN_FFMPEG_TESTS=1 cargo test -p opentake-tauri --test motion_integration -- --nocapture
1/1 passed; real Chromium/FFmpeg add, edit, reopen, cancellation, glyph pixels,
CSS animation frame differences, and 10 fps → 20 fps duration conversion

cargo test -p opentake-motion --all-features
97 unit + 7 live Chromium + 3 pipeline tests passed

cargo clippy -p opentake-core -p opentake-motion -p opentake-agent -p opentake-tauri --all-targets -- -D warnings
passed (only existing block 0.1.6 future-incompatibility notice)

cargo fmt --all -- --check
passed

pnpm -C web test
149 files / 1360 tests passed

pnpm -C web build
passed (only existing dynamic-import and large-chunk warnings)

git diff --check
passed
```

## Review

The independent reviewer found release-blocking races in whole-bundle
replacement, Save As, retained-file durability/event ordering, event-subscriber
panic handling, and late UI navigation. It also identified the missing numeric
frame progress. All findings were reproduced, fixed, and re-reviewed.

Final verdict: **Spec PASS / Quality APPROVE**, with zero CRITICAL, HIGH,
MEDIUM, or LOW findings.

## Commit

Pending: `feat(motion): publish Studio documents atomically`.
