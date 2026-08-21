# Task 3 Report — Preview Temporal Remap Parity

## Scope

- Task: prove RenderPlan/playback temporal parity at the native publication boundary.
- Baseline commit: `69a958fc816c2a7dafda87b479e58fd5c59383d1`
- Allowed edit surface honored:
  - `crates/opentake-render/src/plan/tests.rs`
  - `src-tauri/tests/playback_transport_integration.rs`
- No production Rust code changed.

## What I changed

1. Added `source_frame_video_with_trim_and_fractional_speed` to `crates/opentake-render/src/plan/tests.rs`.
   - Covers `trim_start_frame = 2`, `speed = 1.5`, timeline frames `0 / 3 / 5`.
   - Verifies `source_frame_index` resolves to source frames `2 / 7 / 10`.

2. Added `frame_route_preserves_fractional_speed_publication_order_and_decodability` to `src-tauri/tests/playback_transport_integration.rs`.
   - Builds a deterministic CFR ffmpeg fixture.
   - Uses the existing `RenderLoop -> MjpegSink -> /frame` native publication seam.
   - Waits for `RenderLoop` to converge to the exact expected source frame for timeline frames `0 / 3 / 5`.
   - Verifies `/frame` returns decodable JPEGs for each committed publication.
   - Verifies emitted publication payloads stay monotonic:
     - frames: `0, 3, 5`
     - sequences: `1, 2, 3`
     - terminal flags: `false, false, true`

## Investigation notes

- Existing pure `RenderPlan` tests already covered `reversed` and non-`1.0` speed cases, but not the exact `1.5x` fractional case in this brief.
- Existing production playback/render code already behaved correctly for this case.
- I initially tried asserting the published JPEG decodes were pairwise different; that was too indirect for this seam and not required by the brief, so I replaced it with a stronger parity check against exact decoded source frames before publication.

## Verification

Ran after the test changes:

- `cargo test -p opentake-render source_frame_video_with_trim_and_fractional_speed`
- `cargo test -p opentake-tauri --features playback-engine --test playback_transport_integration frame_route_preserves_fractional_speed_publication_order_and_decodability -- --nocapture`
- `cargo test -p opentake-render plan`
- `cargo test -p opentake-tauri --features playback-engine --test playback_transport_integration`
- `cargo test -p opentake-tauri --features playback-engine playback::transport`
- `cargo test -p opentake-tauri --features playback-engine playback::engine`
- `cargo fmt --all`
- `cargo fmt --all -- --check`

All commands passed in this environment.

## Concerns

- The worktree `.git` file points at an old absolute path under `/Users/lvbaiqing/...`; git still works when driven with explicit `GIT_DIR` / `GIT_COMMON_DIR` / `GIT_WORK_TREE`, but the pointer itself is stale workspace state outside this task’s allowed code surface.
- The worktree contains unrelated user changes under `docs/audit/` and `.playwright-cli/`; they were left untouched and must not be included in this task commit.
