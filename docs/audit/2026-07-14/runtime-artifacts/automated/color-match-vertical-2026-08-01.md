# Reference Color Match Vertical — 2026-08-01

## Delivered surface

- The capability-gated `match_color` Agent tool is backed by the desktop host
  and accepts an ordinary forward 1x image/video target, target timeline frame,
  reference image/video asset, and reference source frame.
- Both frames are decoded at bounded resolution and sampled in linear BT.709.
  The `opentake-luma-preserving-mean-match` v1 algorithm normalizes reference
  chromaticity to the target luma, then emits an ordinary editable per-channel
  gain grade.
- Preview performs no project mutation and reports target/reference/matched
  linear means, CIE Delta E 1976 before/after, target luma before/after, the
  exact generated grade, and algorithm version.
- The Inspector lists visual media references, exposes reference/target frame
  inputs, preview, cancellation, retry, color swatches, measured Delta E/luma,
  Apply, and Undo. The ordinary Color Grade controls remain the editor for the
  applied result.
- Apply commits the grade and persisted `ColorMatchInput` together in one
  optimistic-revision edit. Provenance includes reference asset/frame, target
  frame, algorithm/version, sampled means, Delta E, and luma. Any later manual
  grade edit clears that provenance so the project cannot mislabel an altered
  grade as the original sampled match.
- The existing shared render plan carries the same `Clip.color_grade` into
  interactive preview and export; there is no separate export approximation.

## Automated evidence

- `CARGO_INCREMENTAL=0 cargo test -p opentake-tauri color_match_improves_delta_e_preserves_luma_and_persists_editable_grade --lib -- --nocapture`
  - passed with fixed PNG target/reference/black fixtures;
  - preview reduced CIE Delta E to below 0.01 and below its input value while
    keeping absolute target-luma drift below 0.01;
  - black low-confidence analysis failed without changing timeline/version;
  - Apply persisted an editable grade and complete provenance;
  - Undo/redo and save/reopen restored the exact grade and provenance;
  - pre-cancelled analysis returned typed cancellation.
- `CARGO_INCREMENTAL=0 cargo test -p opentake-ops color_match_command_tests --lib`
  - the grade/provenance pair forms one undo entry, and a later manual grade
    edit clears sampled-match provenance.
- `CARGO_INCREMENTAL=0 cargo clippy -p opentake-domain -p opentake-ops -p opentake-tauri --all-targets -- -D warnings`
  - passed.
- `npm test -- --run`
  - all 114 web test files passed: 870 tests;
  - preview/measurement, Apply/Undo, cancellation with stale-result rejection,
    missing-reference refusal, and reversed-clip refusal passed.
- `npm run build`
  - TypeScript and the production Vite bundle passed; existing chunk-size and
    ineffective-dynamic-import warnings remain non-fatal.

## Remaining acceptance evidence

Packaged macOS GUI verification and a final preview/export inspection remain
required before `requirement-7d79665fbcb91584` is closed. These run against the
assembled Beta candidate after the complete code gate.
