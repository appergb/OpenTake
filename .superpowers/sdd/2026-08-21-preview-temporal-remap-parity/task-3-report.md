# Task 3 Report — Preview Temporal Remap Parity

## Scope

- Task: prove RenderPlan/playback temporal parity at the native publication boundary.
- Baseline commit: `69a958fc816c2a7dafda87b479e58fd5c59383d1`
- Allowed edit surface honored:
  - `crates/opentake-render/src/plan/tests.rs`
  - `src-tauri/src/playback/engine.rs`
  - `src-tauri/tests/playback_transport_integration.rs`

## What I changed

1. Added `source_frame_video_with_trim_and_fractional_speed` to `crates/opentake-render/src/plan/tests.rs`.
   - Covers `trim_start_frame = 2`, `speed = 1.5`, timeline frames `0 / 3 / 5`.
   - Verifies `source_frame_index` resolves to source frames `2 / 7 / 10`.

2. Replaced the earlier native proof in `src-tauri/tests/playback_transport_integration.rs` with stricter first-publication checks.
   - `frame_route_first_publication_matches_fractional_speed_plan`
     - Uses a deterministic CFR ffmpeg fixture.
     - Fails explicitly if ffmpeg cannot run or if `RenderLoop::new` cannot acquire a GPU.
     - Reuses `build_render_plan` + `source_frame_index` to map timeline frames `0 / 3 / 5` to source frames `2 / 7 / 10`.
     - Asserts the first `RenderLoop::render_frame` result matches the exact decoded source frame.
     - Verifies `/frame` returns decodable JPEGs and native publication payloads remain monotonic:
       - frames: `0, 3, 5`
       - sequences: `1, 2, 3`
       - terminal flags: `false, false, true`
   - `frame_route_first_publication_matches_reversed_plan`
     - Reuses the same RenderPlan/source-frame mapping for `reversed = true`.
     - Verifies source frames `7 / 2` for timeline frames `0 / 5`.
     - Verifies the first native publication JPEGs decode and publication order remains monotonic:
       - frames: `0, 5`
       - sequences: `1, 2`
       - terminal flags: `false, true`

3. Fixed the reversed publication stale-frame bug in `src-tauri/src/playback/engine.rs`.
   - `RenderLoop` now tracks the last planned video `source_frame` per active clip.
   - When the next frame plan rewinds a clip's `source_frame`, playback clears continuous decode streams before syncing the frame.
   - This keeps reversed playback on the exact plan-mapped source frame for the first publication instead of reusing a stale later frame from the forward-only queue.
   - Added engine unit coverage for the rewind detection helper.

## Investigation notes

- Existing pure `RenderPlan` tests already covered `reversed` and non-`1.0` speed cases, but not the exact `1.5x` fractional case from this task.
- Fractional-speed playback already matched the plan on the first render/publication once the test stopped warming up in a loop.
- The stricter reversed native test exposed a real bug: timeline frames advanced monotonically while the clip's planned `source_frame` moved backward, and the forward decode stream reused stale cached/pending data on the first later publication.
- The previous warmup helper hid that behavior and was removed.

## Verification

Red/green checkpoints:

- `cargo test -p opentake-render source_frame_video_with_trim_and_fractional_speed`
- `cargo test -p opentake-tauri --features playback-engine --test playback_transport_integration frame_route_first_publication_matches_fractional_speed_plan -- --nocapture`
- `cargo test -p opentake-tauri --features playback-engine --test playback_transport_integration frame_route_first_publication_matches_reversed_plan -- --nocapture`

Fresh verification after the fix:

- `cargo test -p opentake-render plan`
- `cargo test -p opentake-tauri --features playback-engine --test playback_transport_integration`
- `cargo test -p opentake-tauri --features playback-engine playback::engine`
- `cargo test -p opentake-tauri --features playback-engine playback::transport`
- `cargo fmt --all`
- `cargo fmt --all -- --check`

All commands passed in this environment.

## Concerns

- The worktree `.git` file still points at an old absolute path under `/Users/lvbaiqing/...`; git operations succeed only when driven with explicit `GIT_DIR` / `GIT_COMMON_DIR` / `GIT_WORK_TREE`.
- The worktree contains unrelated user changes under `docs/audit/` and `.playwright-cli/`; they were left untouched and must stay out of this fix commit.
