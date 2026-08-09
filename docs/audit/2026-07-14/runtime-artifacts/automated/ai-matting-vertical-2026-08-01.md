# AI Matting Vertical — 2026-08-01

## Delivered surface

- The capability-gated `ai_matte` Agent tool is advertised only after the
  pinned RVM model passes regular-file, exact-byte-size, and SHA-256 checks.
- The desktop host runs the official RVM MobileNetV3 FP32 ONNX graph through
  the shared ORT worker with recurrent state preserved across frames.
- Preview produces a content-addressed ProRes 4444 cache without modifying the
  timeline or manifest. The Inspector shows that transparent result over a
  checkerboard and reuses it for Apply.
- Apply copies through a retained no-follow project-media capability, registers
  generation provenance, replaces the selected clip in one durable undo entry,
  and keeps the source media in the manifest.
- Generated derivatives retain source audio, preserve their alpha plane, and
  request straight-alpha premultiplication in still preview, continuous
  playback, MCP timeline inspection, and export render plans.
- The model installer is explicit, cancellable, reports byte progress, leaves
  no partial model after cancellation/failure, and verifies the official
  14,975,696-byte payload against
  `88d4531297118f595bf2fd60f6f566aec2e559393802d1f436c380f0cbbd2828`.
- `NOTICE` records the official Robust Video Matting project, authors,
  ByteDance origin, and GPL-3.0 license. The model is downloaded on demand and
  is not embedded in the application bundle.

## Automated evidence

- `OPENTAKE_TEST_RVM_MODEL=... CARGO_INCREMENTAL=0 cargo test -p opentake-tauri advanced::tests::official_matting_preview_apply_undo_and_reopen -- --nocapture`
  - official ONNX inference on a real H.264/AAC fixture;
  - preview leaves project state unchanged;
  - apply preserves audio and provenance;
  - undo/redo and save/reopen restore the correct media reference;
  - a pre-cancelled cached request still returns the typed cancellation result.
- `CARGO_INCREMENTAL=0 cargo test -p opentake-media --test ffmpeg_integration prores_4444_roundtrip_preserves_alpha_plane -- --exact`
  - alpha samples survive the real FFmpeg encode/decode round trip.
- `CARGO_INCREMENTAL=0 cargo test -p opentake-media --features ort-backend,model-download analysis::matting::tests::pre_cancelled_download_never_creates_a_partial_model -- --exact`
  - cancellation completes before network access and publishes no partial file.
- `CARGO_INCREMENTAL=0 cargo clippy -p opentake-media --features ort-backend,model-download --all-targets -- -D warnings`
  - passed.
- `CARGO_INCREMENTAL=0 cargo clippy -p opentake-tauri --all-targets -- -D warnings`
  - passed.
- `pnpm test -- MattingSection.test.tsx`
  - the repository runner executed all 112 test files: 863 tests passed.
- `pnpm build`
  - TypeScript build and production Vite bundle passed; existing chunk-size and
    ineffective-dynamic-import warnings remain non-fatal.

## Remaining acceptance evidence

Packaged macOS GUI verification and final delivery export inspection are still
required before the planning checkbox is closed. Those steps intentionally run
after the complete code gate so the evidence represents the Beta candidate.
