# Task 5 Batch B Report

## What I inspected

- Task brief: `.superpowers/sdd/task-5-brief.md`
- Branch drift:
  - `git rev-list --left-right --count origin/main...feat/save-clip-as-media` -> `154 1`
  - `git rev-list --left-right --count origin/main...feat/freeze-frame` -> `154 1`
- Save/export commit:
  - `git show --stat --summary 708fd443f1d2c8796b71acb7bc0ad1042dabd88c`
  - `git show --name-only --format=medium 708fd443f1d2c8796b71acb7bc0ad1042dabd88c`
- Freeze commit:
  - `git show --stat --summary da3e934feb6a4cb4a55f8251172839f67fd23ca8`
  - `git show --name-only --format=medium da3e934feb6a4cb4a55f8251172839f67fd23ca8`
- Current HEAD files before replay:
  - `src-tauri/src/export.rs`
  - `src-tauri/src/lib.rs`
  - `src-tauri/src/commands.rs`
  - `src-tauri/src/render.rs`
  - `crates/opentake-ops/src/command.rs`
  - `web/src/components/timeline/ClipContextMenu.tsx`
  - `web/src/components/timeline/ClipContextMenu.test.tsx`
  - `web/src/i18n/dict.ts`
  - `web/src/lib/api.ts`
  - `web/src/lib/types.ts`
  - `web/src/store/editActions.ts`
  - `docs/superpowers/archive/2026-07-08-branch-integration-register.md`

## Direct-merge decision

- `feat/save-clip-as-media`: rejected direct stale merge. Evidence: branch is `154/1` behind/ahead, and the branch commit predates current export/media shape. Replayed only the still-missing range/audio export delta.
- `feat/freeze-frame`: rejected direct stale merge. Evidence: branch is `154/1` behind/ahead, and current tree already had partial freeze prerequisites (`capture` plumbing and timeline primitives) but not the end-user command chain. Replayed only the still-missing command/request/menu/i18n delta.

## What I ported

### feat/save-clip-as-media

- Added `ExportFormat` (`video` / `audioWav`) in `src-tauri/src/export.rs`
- Extended `run_export_with_control(...)` to accept optional `(start_frame, end_frame)` and slice both video progress and mixed audio to that range
- Added `slice_pcm(...)`
- Added `write_wav_s16le(...)`
- Added `export_range(...)` Tauri command
- Registered `export_range` in `src-tauri/src/lib.rs`
- Updated web bridge to send explicit `clipId | null`, `inFrame`, `outFrame`, `format`, and `trackIndex`
- Kept current UI behavior clip-focused; the context menu now passes explicit clip span + track index to the new range-aware backend path
- Intentionally did **not** touch `src-tauri/src/media.rs`; current single-clip `save_clip_as_media` remains intact but unused by the updated menu path

### feat/freeze-frame

- Added `EditCommand::FreezeFrame` plus apply-path handling in `crates/opentake-ops/src/command.rs`
- Added focused freeze-frame validation + ripple/undo tests in `crates/opentake-ops/src/command.rs`
- Updated `src-tauri/src/commands.rs` so `edit_apply` intercepts `freezeFrame`, captures a still first, imports it, then forwards a real `media_ref` into the edit transaction
- Restored `capture_freeze_frame(...)` in `src-tauri/src/render.rs`
- Added `freezeFrame` to the web `EditRequest` union in `web/src/lib/types.ts`
- Added `DEFAULT_FREEZE_FRAMES`, `freezeFrame(...)`, and `freezeClipAtPlayhead(...)` in `web/src/store/editActions.ts`
- Added `Freeze Frame` / `冻结帧` menu wiring and prompt path in `web/src/components/timeline/ClipContextMenu.tsx`
- Added focused menu coverage in `web/src/components/timeline/ClipContextMenu.test.tsx`
- Added i18n keys in `web/src/i18n/dict.ts`

## Verification command output

### Save/export replay

```text
$ cargo test --manifest-path /Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/src-tauri/Cargo.toml -p opentake-tauri save_clip --lib -- --nocapture
Finished `test` profile [unoptimized + debuginfo] target(s) in 1.53s
Running unittests src/lib.rs (target/debug/deps/opentake_tauri_lib-084cfae97cd202b7)

running 2 tests
test export::tests::save_clip_slice_pcm_cuts_requested_frame_window ... ok
test export::tests::save_clip_export_format_parses_audio_wav ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 151 filtered out; finished in 0.00s
```

```text
$ pnpm -C /Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/web exec tsc -b --pretty false
[passed with no diagnostics]
```

```text
$ git diff --check -- src-tauri/src/export.rs src-tauri/src/lib.rs web/src/lib/api.ts web/src/store/editActions.ts web/src/components/timeline/ClipContextMenu.tsx
[no output]
```

### Freeze replay

```text
$ cargo test --manifest-path /Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/src-tauri/Cargo.toml -p opentake-ops freeze_frame -- --nocapture
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
Running unittests src/lib.rs (target/debug/deps/opentake_ops-54ea1c0d7e3c7f3b)

running 10 tests
test command::freeze_frame_tests::freeze_frame_audio_clip_rejected ... ok
test command::freeze_frame_tests::freeze_frame_missing_clip_rejected ... ok
test command::freeze_frame_tests::freeze_frame_at_end_endpoint_rejected ... ok
test command::freeze_frame_tests::freeze_frame_text_clip_rejected ... ok
test command::freeze_frame_tests::freeze_frame_zero_duration_rejected ... ok
test command::freeze_frame_tests::freeze_frame_at_start_endpoint_rejected ... ok
test command::freeze_frame_tests::freeze_frame_splits_and_inserts_image_clip ... ok
test command::freeze_frame_tests::freeze_frame_preserves_real_media_ref ... ok
test command::freeze_frame_tests::freeze_frame_undo_restores_original_in_one_step ... ok
test command::freeze_frame_tests::freeze_frame_shifts_a_follower_on_same_track ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 172 filtered out; finished in 0.00s
```

```text
$ cargo test --manifest-path /Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/src-tauri/Cargo.toml -p opentake-tauri deserializes_freeze_frame --lib -- --nocapture
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.77s
Running unittests src/lib.rs (target/debug/deps/opentake_tauri_lib-084cfae97cd202b7)

running 1 test
test commands::edit_request_serde_tests::deserializes_freeze_frame ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 152 filtered out; finished in 0.00s
```

```text
$ pnpm -C /Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/web test -- src/components/timeline/ClipContextMenu.test.tsx
Test Files  45 passed (45)
Tests  514 passed (514)
```

```text
$ pnpm -C /Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/web exec tsc -b --pretty false
[passed with no diagnostics]
```

```text
$ git diff --check -- crates/opentake-ops/src/command.rs src-tauri/src/commands.rs src-tauri/src/render.rs web/src/components/timeline/ClipContextMenu.tsx web/src/components/timeline/ClipContextMenu.test.tsx web/src/i18n/dict.ts web/src/lib/types.ts web/src/store/editActions.ts docs/superpowers/archive/2026-07-08-branch-integration-register.md
[no output]
```

## Commits created

- `f25d07b` `feat(export): replay range save as media from branch queue`
- `935b55b` `feat(editing): replay freeze frame from branch queue`

## Files changed

- `docs/superpowers/archive/2026-07-08-branch-integration-register.md`
- `src-tauri/src/export.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/render.rs`
- `crates/opentake-ops/src/command.rs`
- `web/src/components/timeline/ClipContextMenu.tsx`
- `web/src/components/timeline/ClipContextMenu.test.tsx`
- `web/src/i18n/dict.ts`
- `web/src/lib/api.ts`
- `web/src/lib/types.ts`
- `web/src/store/editActions.ts`

## Concerns

- The backend/API now support `clipId = null` range export, but the current UI still exposes only clip-context save-as-media; there is no new dedicated marked-range menu affordance in this batch.
- `pnpm exec prettier ...` was not runnable in this checkout because `prettier` is not installed as a command here; Rust files were formatted with `cargo fmt`, and TS/TSX passed `tsc` + tests as written.

## Review Fixes

### Root cause per finding

1. Save as Media regression: the Batch B replay reused the new `export_range` bridge for the clip-context action, which changed behavior from the established single-clip durable-import command to timeline-range cache export.
2. Freeze preflight side effects: `edit_apply` validated only after capture/import work had already started, so invalid requests could still create avoidable side effects before the ops layer rejected them.
3. Freeze capture isolation: `capture_freeze_frame` reused the normal full-timeline composite helper, so the PNG could include unrelated tracks/overlays instead of the target clip alone.
4. Register audit gap: the Batch B register entry did not include the exact branch log/name-status/stat commands requested by the brief, and it did not reflect the post-review verification matrix.

### What changed

- Restored clip-context Save as Media to the existing `save_clip_as_media` command in:
  - `web/src/lib/api.ts`
  - `web/src/store/editActions.ts`
  - `web/src/components/timeline/ClipContextMenu.tsx`
- Kept `export_range` as a backend/future range API only; it is no longer the clip-context path.
- Added `web/src/store/editActions.saveClipAsMedia.test.ts` to lock the single-clip API signature.
- Added `validate_freeze_frame_request(...)` preflight enforcement before capture/import side effects in `src-tauri/src/commands.rs`, while leaving ops-layer validation in place.
- Refactored `src-tauri/src/render.rs` so freeze capture composites a temporary one-track snapshot containing only the target clip and its referenced media entry.
- Updated `docs/superpowers/archive/2026-07-08-branch-integration-register.md` with the required exact branch evidence commands and the fix verification commands/results.

### Tests run with results

```text
$ cargo test --manifest-path src-tauri/Cargo.toml -p opentake-tauri save_clip --lib -- --nocapture
running 2 tests
test export::tests::save_clip_export_format_parses_audio_wav ... ok
test export::tests::save_clip_slice_pcm_cuts_requested_frame_window ... ok
test result: ok. 2 passed; 0 failed
```

```text
$ cargo test --manifest-path src-tauri/Cargo.toml -p opentake-ops freeze_frame -- --nocapture
running 10 tests
test result: ok. 10 passed; 0 failed
```

```text
$ cargo test --manifest-path src-tauri/Cargo.toml -p opentake-tauri freeze_frame_preflight --lib -- --nocapture
running 1 test
test commands::edit_request_serde_tests::freeze_frame_preflight_rejects_bad_requests_before_capture ... ok
test result: ok. 1 passed; 0 failed
```

```text
$ cargo test --manifest-path src-tauri/Cargo.toml -p opentake-tauri freeze_capture_snapshot --lib -- --nocapture
running 1 test
test render::tests::freeze_capture_snapshot_isolates_target_clip_and_media ... ok
test result: ok. 1 passed; 0 failed
```

```text
$ cargo test --manifest-path src-tauri/Cargo.toml -p opentake-tauri deserializes_freeze_frame --lib -- --nocapture
running 1 test
test commands::edit_request_serde_tests::deserializes_freeze_frame ... ok
test result: ok. 1 passed; 0 failed
```

```text
$ pnpm -C web test -- src/store/editActions.saveClipAsMedia.test.ts src/components/timeline/ClipContextMenu.test.tsx
Test Files  46 passed (46)
Tests  515 passed (515)
```

```text
$ pnpm -C web exec tsc -b --pretty false
[passed with no diagnostics]
```

```text
$ git diff --check -- src-tauri/src/commands.rs src-tauri/src/render.rs src-tauri/src/export.rs src-tauri/src/lib.rs web/src/lib/api.ts web/src/store/editActions.ts web/src/store/editActions.saveClipAsMedia.test.ts web/src/components/timeline/ClipContextMenu.tsx web/src/components/timeline/ClipContextMenu.test.tsx web/src/lib/types.ts docs/superpowers/archive/2026-07-08-branch-integration-register.md .superpowers/sdd/task-5-export-freeze-report.md
[no output]
```

### Files changed

- `src-tauri/src/commands.rs`
- `src-tauri/src/render.rs`
- `web/src/lib/api.ts`
- `web/src/store/editActions.ts`
- `web/src/store/editActions.saveClipAsMedia.test.ts`
- `web/src/components/timeline/ClipContextMenu.tsx`
- `docs/superpowers/archive/2026-07-08-branch-integration-register.md`
- `.superpowers/sdd/task-5-export-freeze-report.md`

### Commit SHA

- `2583232` `fix(task5): address batch b review findings`

## Full Task 5 Review Fixes

### Root cause

- `capture_freeze_frame(...)` wrote `freeze_{clip_id}_{frame}.png`, so freezing the same clip/frame again reused the same source path and let import dedup or overwrite the previous asset.
- `export_range(...)` wrote `save_{name_id}_{start}_{end}.{ext}`, so repeated exports of the same range reused the same source path and could be treated as the same imported asset.

### Changes

- Updated `src-tauri/src/render.rs` so freeze captures now go through `freeze_capture_png_path(...)`, which keeps the existing readable prefix but appends a fresh `uuid_like()` suffix on every write.
- Hardened `uuid_like()` with a timestamp-plus-counter suffix so same-process back-to-back captures stay unique even if clock resolution is coarse.
- Updated `src-tauri/src/export.rs` so range exports now go through `unique_export_range_path(...)`, which appends a fresh unique suffix to every `save_*` output path.
- Added focused unit tests that call the production path helpers twice with identical inputs and assert the resulting paths differ.

### Tests

- `cargo fmt --all --check`
- `cargo test --manifest-path src-tauri/Cargo.toml -p opentake-tauri freeze_capture --lib -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml -p opentake-tauri export_range --lib -- --nocapture`
- `git diff --check -- src-tauri/src/render.rs src-tauri/src/export.rs .superpowers/sdd/task-5-export-freeze-report.md`

### Files changed

- `src-tauri/src/render.rs`
- `src-tauri/src/export.rs`
- `.superpowers/sdd/task-5-export-freeze-report.md`

### Commit SHA

- `fba5b98` `fix(task5): make freeze and export asset paths unique`
