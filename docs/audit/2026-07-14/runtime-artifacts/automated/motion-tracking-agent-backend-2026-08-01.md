# Motion tracking Agent/backend evidence — 2026-08-01

Scope: Task 3 sub-slice `requirement-fdd45062091b48f3`. The production backend,
Agent boundary, Inspector/Preview interaction, persistence, and export path are
now complete. Only the final packaged Beta GUI pass remains open.

## Production path

- `track_motion` is a strict, capability-gated MCP/Chat tool. Hosts without a production advanced-workflow bridge do not advertise it and direct dispatch fails closed.
- The desktop bridge resolves the selected ordinary video clip from one authoritative project snapshot, decodes at most 48 source-aligned frames, tracks only the normalized selected rectangle, and checks cancellation throughout.
- Successful analysis returns editable linear position keyframes, algorithm/version provenance, and minimum confidence. Preview is the default and does not mutate the timeline.
- Confidence below 0.25 returns `MCP_ANALYSIS_LOW_CONFIDENCE`; invalid rectangles/ranges, missing sources, cancellation, and stale project revisions do not commit.
- `apply=true` writes the complete position track through `EditCommand::SetKeyframes` at the analyzed project revision. It is one ordinary undoable transaction.
- The desktop command boundary exposes the same capability through
  `advanced_track_motion`. The Inspector accepts an exact half-open clip range,
  normalized numeric region fields, cancellation/retry, result confidence and
  keyframe count, Apply, and visible Undo.
- “Select region in preview” replaces the ordinary transform overlay with a
  crosshair rectangle editor. Reverse-direction drags and out-of-canvas drags
  are normalized and clamped; any region/range change invalidates the reviewed
  result before Apply can be used.

## RED/GREEN evidence

The reviewed Agent test first failed because `track_motion` was not a typed tool. The media test then failed to compile because `track_region_motion` and `NormalizedMotionRegion` did not exist.

Passing focused commands:

```text
CARGO_INCREMENTAL=0 cargo test -p opentake-media analysis::stabilization::tests::region_tracker_keeps_known_subject_center_within_five_pixels -- --exact --nocapture
CARGO_INCREMENTAL=0 cargo test -p opentake-tauri advanced::tests:: --lib -- --nocapture
CARGO_INCREMENTAL=0 cargo test -p opentake-agent --test advanced_ai_workflows
CARGO_INCREMENTAL=0 cargo clippy -p opentake-agent -p opentake-media -p opentake-tauri --all-targets -- -D warnings
```

The synthetic 96×72 three-frame target moves by `(8,4)` pixels. The final normalized track converts back within five pixels on both axes. The desktop test generates a 12-frame H.264 MP4 with FFmpeg, imports and places it through the real `AppCore`, obtains a mutation-free preview, applies the returned keyframes, saves and reopens the exact position track, exports all 12 frames through the production H.264 renderer, performs one undo, and verifies a pre-cancelled retry leaves the timeline unchanged.

The final integrated code gate passed:

- `cargo fmt --all -- --check`;
- `CARGO_INCREMENTAL=0 cargo test -p opentake-tauri --lib`: 416/416;
- default and no-default-feature Tauri Clippy with `-D warnings`;
- `npm test`: 119 files / 880 tests, including the Inspector flow, preview
  rectangle normalization, cancellation/stale-result isolation, and Tauri
  command parity;
- `npm run build` (existing chunk advisories only).

## Remaining acceptance

- Packaged macOS GUI evidence retained with the Beta release run.
