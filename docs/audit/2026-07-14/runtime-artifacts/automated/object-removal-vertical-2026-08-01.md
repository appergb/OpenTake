# Object Removal Vertical — 2026-08-01

## Delivered surface

- The strict `remove_object` Agent contract is advertised by the desktop host
  because this build always includes the local implementation.
- The provider/model pair is explicitly reported as `opentake-local` /
  `opentake-boundary-fill-v1`; unsupported provider or model names fail closed
  instead of implying an unavailable hosted service.
- The selected editable vector mask and absolute project-frame range form part
  of a content-addressed derivative key together with source SHA-256, trim,
  duration, and timeline FPS.
- Preview decodes the ordinary forward 1x source, applies deterministic
  boundary propagation only inside the selected range, preserves feathered
  mask edges, encodes ProRes 422, retains source audio, and does not mutate the
  project.
- The Inspector provides mask editing through the existing mask controls plus
  range fields, preview, cancellation, retry, inline video review, Apply, and
  Undo. Editing the mask invalidates a previously reviewed preview.
- Apply publishes the exact reviewed cache bytes through the retained no-follow
  project-media capability, registers source/range/mask/provider/model
  provenance, and swaps the selected clip to the derivative.
- Media registration, clip replacement, and clearing the now-baked editable
  masks form one durable edit transaction. Undo restores source media and
  masks; redo and save/reopen restore the derivative. The original asset stays
  in the media manifest.

## Automated evidence

- `CARGO_INCREMENTAL=0 cargo test -p opentake-tauri advanced::tests --lib -- --nocapture`
  - five advanced-workflow tests passed;
  - the object-removal test uses a real eight-frame H.264/AAC fixture;
  - a sampled frame outside the requested range retains the object while a
    sampled frame inside the range removes it;
  - preview leaves timeline and manifest unchanged;
  - a nonexistent mask fails without changing timeline, manifest, or version;
  - Apply preserves audio and provenance, and the published file SHA-256 equals
    the reviewed preview SHA-256 consumed by playback/export;
  - Undo/redo and save/reopen restore the expected media reference and masks;
  - a pre-cancelled cached request returns typed cancellation.
- `CARGO_INCREMENTAL=0 cargo test -p opentake-ops motion_media_transaction_tests --lib`
  - four tests passed, including one-entry swap/clear Undo and Redo.
- `CARGO_INCREMENTAL=0 cargo test -p opentake-core motion_media_commit --lib`
  - three durable media-commit tests passed, including outside-bundle and
    symlink refusal without mutation.
- `CARGO_INCREMENTAL=0 cargo test -p opentake-core generated_media_commit_refuses_version_drift_without_mutation --lib`
  - a project edit made while generation is running refuses the stale commit
    atomically, preventing a reviewed mask or range from being overwritten.
- `CARGO_INCREMENTAL=0 cargo clippy -p opentake-tauri -p opentake-core -p opentake-ops --all-targets -- -D warnings`
  - passed.
- `npm test -- --run`
  - all 113 web test files passed: 867 tests, including preview failure/retry,
    cancellation with stale-completion rejection, apply/undo, compatibility,
    missing-mask refusal, and an Apply response that arrives after the baked
    mask has already been cleared from the timeline mirror.
- `npm run build`
  - TypeScript and the production Vite bundle passed; existing chunk-size and
    ineffective-dynamic-import warnings remain non-fatal.

## Remaining acceptance evidence

Packaged macOS GUI verification and final delivery-export inspection remain
required before `requirement-be73dca02523d3b0` is closed. These run after the
full code gate so the evidence represents the Beta candidate rather than an
intermediate development build.
