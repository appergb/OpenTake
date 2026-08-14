# Motion Studio Task 5 report

Status: COMPLETE — independently reviewed and approved before commit.

## Scope

- Replaced the shell placeholder with a Songxia/Codex-inspired dark authoring
  workspace: document/template/history rail, controlled HTML/CSS CodeMirror
  editor, semantic 16:9 preview region, parameter inspector, and bounded
  keyframe strip.
- Added project-confined HTML/CSS document creation, loading, prospective hash,
  atomic patch, preview, and cancellation commands. The browser-only fallback
  now has a monotonic project identity and cannot leak Motion documents across
  New/Open boundaries.
- Added 300 ms serialized autosave with Rust-authoritative revision hashes,
  exact conflict reload/reapply behavior, source-version CAS guards, and a
  project-boundary flush that blocks New/Open/sample/Save As when a Motion
  edit cannot be persisted safely.
- Added deterministic integer-frame playback and preview scheduling with
  supersession cancellation, stale-response rejection, document-bound
  last-good frames, pause/suspend/resume lifecycle handling, and safe immediate
  unmount/remount recovery.
- Added controlled CodeMirror language and source updates without cursor or
  selection collapse, per-file diagnostics, keyboard/focus semantics,
  reduced-motion behavior, and responsive folding at the real 760 px minimum.
- Added a parser-backed, deduplicated, 24-item keyframe scan that ignores CSS
  comments and strings and cannot degrade quadratically on a 1 MiB source.
- Added CodeMirror packages under their MIT license and recorded them in the
  third-party inventory.

## TDD evidence

Initial RED covered the absent store/editor and the required load, edit,
debounce, conflict, stale response, diagnostics, playback, narrow-layout,
keyboard, and reduced-motion behavior.

Review-driven RED regressions additionally proved:

- JS JSON hashing disagreed with Rust for integral floats and Unicode key order.
- document switches and conflict reads could overwrite input typed after the
  operation began;
- preview cancellation, last-good frames, unmount/remount, and rapid
  suspend/resume/suspend ordering could publish stale state;
- edits made inside the debounce window could be lost at project boundaries;
- CodeMirror external updates collapsed reverse selections;
- a regex keyframe scan parsed comments/strings and performed quadratic work;
- browser fallback Motion state crossed New/Open project identities.

Every regression was observed failing before the corresponding production
change. The final browser isolation regression was 2 failures / 8 passes before
the monotonic identity and reset implementation, then 10/10 green.

## Final fresh verification

```text
pnpm -C web exec vitest run src/lib/api.test.ts
1 file / 10 tests passed

pnpm -C web exec vitest run \
  src/components/motion/MotionCodeEditor.test.tsx \
  src/components/motion/MotionStudio.test.tsx \
  src/components/motion/MotionStudio.interaction.test.tsx \
  src/components/motion/MotionTimeline.test.ts \
  src/store/motionStudioStore.test.ts \
  src/store/projectActions.test.ts
6 files / 62 tests passed

pnpm -C web exec vitest run \
  src/components/media/MediaPanel.test.tsx \
  src/components/shell/TitleBar.interaction.test.tsx \
  src/store/recentStore.test.ts src/store/projectActions.test.ts
4 files / 104 tests passed

pnpm -C web test
149 files / 1352 tests passed

pnpm -C web build
passed (only existing dynamic-import and large-chunk warnings)

cargo test -p opentake-tauri motion_documents::tests --lib -- --nocapture
17/17 passed

cargo test -p opentake-tauri motion::tests --lib -- --nocapture
7/7 passed

cargo clippy -p opentake-tauri --all-targets -- -D warnings
passed (only existing block 0.1.6 future-incompatibility notice)

cargo fmt --all -- --check
passed

python3 -B -m unittest scripts/test_check_license_inventory.py
7/7 passed

python3 -B scripts/check_license_inventory.py
passed

pnpm -C web install --frozen-lockfile --offline
passed

pnpm -C web licenses list --prod
passed; CodeMirror packages report MIT
```

Browser visual QA exercised the real wide workspace and the 760 px compact
layout. The semantic snapshot contained every required region and control; a
fresh reload had zero console warnings/errors. Temporary screenshots and
browser traces were removed before commit.

## Review

The independent review initially identified release-blocking issues in hash
authority, save/switch/conflict CAS, stale preview and last-good behavior,
visibility cancellation, disposal/remount, project-boundary persistence, and
keyframe scanning. All were reproduced and fixed. Its final main-path verdict
was **Spec PASS / Quality APPROVE** with no CRITICAL or HIGH findings.

The sole remaining MEDIUM concerned browser fallback state crossing project
boundaries. That was then fixed with monotonic browser epochs, accurate paths,
and New/Open resets. The reviewer re-ran the focused API suite and returned a
final **APPROVE**, with zero findings at every severity.

## Commit

Pending: `feat(motion): build HTML and CSS authoring workspace`.
