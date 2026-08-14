# Task 6 implementer report

Status: COMPLETE — review round 2/5 remediations and CloseRequested parity are implemented.

## Scope completed

- Replaced ordinary project Save's source-only cover grab with a stable frame rendered by the existing preview/export `composite_timeline_frame` compositor. The media layer owns only representative-frame selection, bounded 16:9 cover geometry, and deterministic JPEG encoding; it does not contain alternate render logic.
- Selected a visible, resolvable clip midpoint, preferring the midpoint of a valid outgoing cross-dissolve. Empty and all-offline projects produce no replacement bytes.
- Preserved the prior thumbnail whenever representative selection, composite rendering, RGBA conversion, JPEG encoding, or the project writer's atomic replacement fails.
- Made Home advertise only bounded, regular, non-symlink JPEG thumbnails. Invalid prior bytes remain on disk but are not exposed to the UI.
- Added real authoritative-render evidence for background transition mixing, a transformed overlay, overlay bounds, and styled text, plus deterministic bounds, empty/offline, invalid-prior, capture-failure, stale-project, and atomic-write-failure coverage.

## TDD evidence

### RED

```text
cargo test -p opentake-media thumbnail::project::tests::composite_ -- --nocapture
```

Exit 101: three assertions failed against the source-only implementation: layered output remained the source red, a `200×200` bound yielded `320×180` instead of `192×108`, and the representative frame was `0` instead of the transition midpoint `25`. Empty and offline cases already failed closed.

```text
cargo test -p opentake-tauri home::tests::thumbnail_ --lib -- --nocapture
```

Exit 101: Home advertised invalid `thumbnail.jpg` bytes instead of retaining them without exposing them.

### Phase 1 GREEN

```text
cargo test -p opentake-media thumbnail::project::tests -- --nocapture
cargo test -p opentake-tauri home::tests::thumbnail_ --lib -- --nocapture
cargo test -p opentake-tauri project_open_async_tests::thumbnail_ --lib -- --nocapture
cargo clippy -p opentake-media --lib -- -D warnings
cargo check -p opentake-tauri --lib --no-default-features
```

Observed results:

- Media project-thumbnail suite: 12 passed, including all five `composite_` cases.
- Home thumbnail suite: 2 passed.
- Tauri project thumbnail suite: 3 passed, including the real FFmpeg/GPU authoritative compositor fixture.
- Media clippy and Tauri no-default-features check passed.

## Commit

`feat(home): save composited project cover frames`

## Review round 1/5 remediation

- Strict cover mode now reports any planned image, video, text, or Lottie materialization failure. `CaptureFailed` preserves the prior component; `NoVisibleContent` explicitly removes it; `Captured` atomically replaces it.
- External media is accepted only from a persisted/dialog scope after validating both requested and retained final paths. Project media is opened solely through the session's retained `ProjectRoot` no-follow authority. Strict rendering has no ambient pathname fallback.
- The representative frame is selected by the authoritative flattened render plan, including its sorted clip order and transition adjacency, and requires a finite, non-degenerate, non-transparent meaningful draw.
- Cover geometry changed from crop/fill to opaque-black resize-to-fit. Portrait and 4:3 canvas boundary tests prove no authored canvas content is cropped.
- `project_save` is asynchronous. Capture, JPEG encode, and save run on a bounded blocking worker with a cancellation/deadline checkpoint and final project identity check before commit. `CloseRequested` awaits the same helper asynchronously before hiding the window.
- The project/core storage contract now carries `ThumbnailUpdate::{Preserve, Replace, Remove}` while retaining the former `Option<Vec<u8>>` methods as compatibility wrappers.

### Additional RED evidence

```text
cargo test -p opentake-media thumbnail::project::tests::composite_thumbnail_letterboxes_ -- --nocapture
```

Exit 101: portrait and 4:3 cases showed cropped content at the exact expected black-bar boundaries.

```text
cargo test -p opentake-render representative_frame_ --lib -- --nocapture
```

Exit 101: `RenderPlan::representative_frame` did not exist; selection was still an independent stored-track traversal.

```text
cargo test -p opentake-project explicit_thumbnail_removal_deletes_only_the_retained_optional_component --lib -- --nocapture
```

Exit 101: `ThumbnailUpdate` did not exist, so no-visible and capture-failure outcomes were indistinguishable.

### Final focused GREEN evidence

```text
cargo test -p opentake-project explicit_thumbnail_removal_deletes_only_the_retained_optional_component --lib
cargo test -p opentake-core explicit_thumbnail_remove_is_distinct_from_capture_failure_preserve --lib
cargo test -p opentake-render representative_frame_ --lib
cargo test -p opentake-media thumbnail::project::tests::composite_thumbnail_letterboxes_ --lib
cargo test -p opentake-tauri project_open_async_tests --lib -- --test-threads=1
cargo clippy -p opentake-project -p opentake-core -p opentake-media -p opentake-render --all-targets -- -D warnings
cargo clippy -p opentake-tauri --lib -- -D warnings
rustfmt --edition 2021 --config skip_children=true <Task 6 Rust files>
git diff --check
```

Observed results: project 1/1, core 1/1, render 3/3, media 2/2, and Tauri 19/19 passed. The Tauri group covers corrupt image/video/text, unauthorized external media, retained project-local media, layered output, stale identity, cancellation-before-commit, tri-state removal/preservation, and CloseRequested parity. Strict clippy passed for all changed library targets and the Tauri library. After those runs, concurrent chat changes temporarily broke the Tauri test target (`BlockDeltaPayload` / `assistant_with_id` / `DonePayload.message_id` mismatch), so the final post-checkpoint compile evidence is `cargo check -p opentake-core -p opentake-tauri`. Task 6 files pass rustfmt and diff checks; workspace-wide fmt and `--all-targets` Tauri clippy remain blocked by those unrelated concurrent chat edits.

## Review round 2/5 remediation

- Restored the source-compatible public `Project.thumbnail: Option<Vec<u8>>`. Explicit authoritative removal now travels through additive `save_to_root_with_thumbnail_update` and `publish_complete_to_with_thumbnail_update` APIs; ordinary callers retain the original `Some = replace`, `None = preserve` contract.
- Replaced the single-midpoint representative probe with deterministic groups derived from flattened render-plan span boundaries, opacity/position/scale/rotation/crop keyframes, fade boundaries, and validated transition intervals. Midpoint remains the stable first choice, but candidates at and around every visual event prevent animated opacity or scale from making a visible project look empty.
- Added a capture → precommit → commit atomic handshake. Timeout atomically cancels capture/precommit work and may then return; if the worker has entered commit, the async Save/Close caller awaits it and reports the actual publication result. The final transition into commit occurs under the existing session identity checkpoint immediately before persistence.
- Added a barrier regression after the identity checkpoint and before publication. It triggers timeout, observes the old thumbnail at response time, releases the detached worker, and proves that worker still cannot publish afterward. A complementary test proves a commit already in progress returns its real result instead of a timeout.

### Round 2 RED evidence

```text
cargo test -p opentake-project --test roundtrip project_thumbnail_field_remains_option_compatible --no-run
```

Exit 101: assigning `Option<Vec<u8>>` to `Project.thumbnail` failed because round 1 had changed the public field to `ThumbnailUpdate`.

```text
cargo test -p opentake-render representative_frame_finds_ -- --nocapture
```

Exit 101: both opacity and scale fixtures returned `None` instead of frame 8 because only frame 4 (the invisible midpoint) was sampled.

```text
cargo test -p opentake-tauri timed_out_cover_precommit_barrier_prevents_post_response_publication --lib --no-run
```

Exit 101: the precommit gate, timeout await helper, and gate-bound save helper did not exist.

### Round 2 GREEN evidence

```text
cargo test -p opentake-project
cargo test -p opentake-core
cargo test -p opentake-render
cargo test -p opentake-media
cargo test -p opentake-tauri commands::project_open_async_tests --lib -- --nocapture
cargo test -p opentake-tauri home::tests::thumbnail_ --lib -- --nocapture
cargo test -p opentake-tauri --lib
cargo clippy -p opentake-project -p opentake-core -p opentake-render -p opentake-media -p opentake-tauri --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Observed results: project 166 unit + all integration suites passed; core 70 unit + all integration suites passed; render 63 unit + all integration suites passed; media 432 passed / 1 environment-gated ignored plus all integration suites (one Main10 fixture test ignored); Tauri focused project-save/open 21/21 and Home thumbnail 2/2 passed; the complete Tauri library suite passed 667/667. Strict all-target clippy, workspace rustfmt, and diff checks passed. Unrelated concurrent chat/web and audit working-tree files were not included.
