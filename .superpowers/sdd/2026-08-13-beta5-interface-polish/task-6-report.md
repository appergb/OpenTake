# Task 6 implementer report

Status: IN PROGRESS — ordinary Save is complete; CloseRequested parity is pending the concurrent MCP `lib.rs` commit.

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

### GREEN so far

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

## Pending before final status

- Reuse `save_project_with_composite_cover` from `CloseRequested` and add close parity coverage after the concurrent MCP change to `src-tauri/src/lib.rs` is committed.
- Run the existing project save/open suites, Tauri clippy, workspace formatting check, and final diff review after integration.

## Commit

`feat(home): save composited project cover frames`
