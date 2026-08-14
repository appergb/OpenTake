# Task 4 report — composited result after clearing the timeline

## Outcome

A successful admitted mutation now records authoritative visible-clip counts
before and after commit. A transition from `> 0` visible clips to `0` schedules
one post-commit capture. The tool result keeps its text summary first and then
adds a real `image/png` block.

The empty result is rendered by the Rust GPU compositor at the project's bounded
canvas size. It contains a deterministic language-neutral empty-set marker and
the current clamped root-timeline playhead as an `HH:MM:SS:FF` bitmap timecode.
Non-empty inputs delegate to the existing strict authoritative compositor.

Capture is deliberately outside the edit transaction. Capture failure preserves
the successful edit, emits only the fixed warning `Timeline preview unavailable.`,
and returns no image. Cancellation or a project/timeline revision change before
or after capture also prevents image publication.

## RED evidence

The required tests were written before production implementation.

- `cargo test -p opentake-agent mcp::dispatch::tests::timeline_image_ -- --nocapture`
  failed to compile because `TimelineResultCaptureRequest` and the new
  `MediaBridge` visibility/capture methods did not exist.
- `cargo test -p opentake-tauri render::tests::empty_timeline_ --lib -- --nocapture`
  failed to compile because `EmptyTimelineCanvasInput`,
  `render_timeline_result_png`, and the PNG bound/semantic canvas contracts did
  not exist.

## Implementation

- `crates/opentake-agent/src/mcp/media_bridge.rs`
  - Added mutation/capture receipts and the host-side authoritative visibility
    and result-capture bridge methods.
  - Added the 1 MiB base64 response ceiling shared with persisted chat.
- `crates/opentake-agent/src/mcp/dispatch.rs`
  - Split dispatch into commit and post-commit completion phases so host project
    gates can release lifecycle locks before GPU work.
  - Schedules exactly one capture only for a successful admitted non-Undo
    mutation whose authoritative count changes from nonzero to zero.
  - Rejects canceled/stale/malformed/oversized capture output and inserts only a
    fixed sanitized warning; successful output is ordered text then PNG.
- `src-tauri/src/render.rs`
  - Reused the authoritative render-plan visibility semantics.
  - Added bounded empty-canvas composition, deterministic semantic marker and
    bitmap timecode, PNG encoding, and current root playhead tracking.
  - Proved non-empty input is byte-identical to the strict authoritative
    compositor path.
- `src-tauri/src/mcp.rs`
  - Added the production bridge, bounded base64 conversion, and exact
    epoch/path/version/timeline checks before and after GPU work.
  - Changed the MCP project gate to commit under its project identity lease,
    release the lease for capture, and recheck identity before returning.

The chat project gate uses the same deferred sequence in its Task 3-owned
`src-tauri/src/chat.rs` integration: commit under the lease, release, capture,
then perform a short final project check.

## Verification

- `cargo check -p opentake-tauri` — passed.
- Focused mutation tests — 10 passed, 0 failed. Covers last-visible deletion,
  deletion leaving content, non-visual mutation, rollback, pre/during-capture
  cancellation, Undo, batch single capture with exact counts, stale revision,
  and sanitized capture failure.
- `cargo test -p opentake-agent mcp:: -- --nocapture` — 180 passed, 0 failed.
- Tauri production bridge focused tests — 2 passed, 0 failed (real bounded PNG
  and stale-project rejection).
- Tauri live-project MCP gate focused tests — 4 passed, 0 failed.
- `cargo test -p opentake-tauri --lib render::tests:: -- --nocapture` — 17
  passed, 0 failed, including the authoritative visibility and two Task 4
  compositor fixtures.
- Task 3-owned chat gate regression after deferred integration — 24 passed, 0
  failed, reported by that file's owner.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p opentake-agent -p opentake-tauri --all-targets -- -D warnings`
  — passed. Cargo emitted only the pre-existing future-incompatibility notice for
  dependency `block v0.1.6`.
- `git diff --check` on the four owned implementation files — passed.

The workspace-wide `cargo test` was intentionally not duplicated because the
Task 3 owner was already running it against the same shared build directory.
No `docs/audit/2026-08-07` path was edited, staged, or committed by Task 4.
