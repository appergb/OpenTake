# Motion tracking Agent/backend evidence — 2026-08-01

Scope: Task 3 sub-slice `requirement-fdd45062091b48f3`. This artifact records the production backend and Agent boundary only; the requirement remains open for the Inspector/Preview interaction and packaged GUI/export evidence.

## Production path

- `track_motion` is a strict, capability-gated MCP/Chat tool. Hosts without a production advanced-workflow bridge do not advertise it and direct dispatch fails closed.
- The desktop bridge resolves the selected ordinary video clip from one authoritative project snapshot, decodes at most 48 source-aligned frames, tracks only the normalized selected rectangle, and checks cancellation throughout.
- Successful analysis returns editable linear position keyframes, algorithm/version provenance, and minimum confidence. Preview is the default and does not mutate the timeline.
- Confidence below 0.25 returns `MCP_ANALYSIS_LOW_CONFIDENCE`; invalid rectangles/ranges, missing sources, cancellation, and stale project revisions do not commit.
- `apply=true` writes the complete position track through `EditCommand::SetKeyframes` at the analyzed project revision. It is one ordinary undoable transaction.

## RED/GREEN evidence

The reviewed Agent test first failed because `track_motion` was not a typed tool. The media test then failed to compile because `track_region_motion` and `NormalizedMotionRegion` did not exist.

Passing focused commands:

```text
CARGO_INCREMENTAL=0 cargo test -p opentake-media analysis::stabilization::tests::region_tracker_keeps_known_subject_center_within_five_pixels -- --exact --nocapture
CARGO_INCREMENTAL=0 cargo test -p opentake-tauri advanced::tests:: --lib -- --nocapture
CARGO_INCREMENTAL=0 cargo test -p opentake-agent --test advanced_ai_workflows
CARGO_INCREMENTAL=0 cargo clippy -p opentake-agent -p opentake-media -p opentake-tauri --all-targets -- -D warnings
```

The synthetic 96×72 three-frame target moves by `(8,4)` pixels. The final normalized track converts back within five pixels on both axes. The desktop test generates a 12-frame H.264 MP4 with FFmpeg, imports and places it through the real `AppCore`, obtains a mutation-free preview, applies the returned keyframes, verifies the persisted position track, performs one undo, and verifies a pre-cancelled retry leaves the timeline unchanged.

## Remaining acceptance

- Inspector/Preview region selection, progress, retry, apply, and visible undo.
- Save/reopen and preview/export transform parity on the same tracked fixture.
- Packaged macOS GUI evidence retained with the Beta release run.
