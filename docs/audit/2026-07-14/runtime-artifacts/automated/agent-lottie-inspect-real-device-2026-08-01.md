# Agent Lottie inspection real-device verification — 2026-08-01

Scope: Agent Settings Generation Task 1 / advertised MCP tool reachability. This
receipt covers the native Lottie source-inspection and composited-timeline
materialization paths. Packaged desktop GUI verification remains part of the
later plan-by-plan UI gate.

## Runtime

- Host: macOS, Apple Silicon, Asia/Shanghai
- GPU: Apple M4, 8 cores, Metal 4
- Display: built-in Liquid Retina, 2560×1664
- Renderer: Velato/Vello on the shared `wgpu` device path
- Test fixture: deterministic two-frame, 16×16, 2 fps Lottie document

## RED

The owning end-to-end test was added first and executed against the production
`TauriMediaBridge`. It failed with the baseline typed result:

```text
inspect_media: Lottie rendering is not available in this build
```

No manifest or source bytes were mutated.

## GREEN acceptance

Command:

```text
CARGO_INCREMENTAL=0 cargo test -p opentake-tauri mcp::tests::inspect_media_renders_lottie_frames_over_gray_end_to_end -- --exact --nocapture
```

Result: PASS, one exact GPU-backed test executed. It verified:

- the project-relative Lottie source is resolved through the live project
  snapshot and rejected if it is not a retained regular file;
- two evenly sampled source times (`0.25`, `0.75`) produce two different JPEG
  frames through real Velato/Vello rasterization;
- the first sample is red-dominant and the second green-dominant;
- transparent pixels are composited over neutral gray;
- returned width, height, frame rate, duration, MIME type, and source byte size
  are authoritative;
- no paid provider or network access is involved.

The same `LottieMaterializer` is now used by `inspect_timeline`; Lottie layers
are no longer silently omitted from Agent timeline inspection.

## Gates

- `cargo test -p opentake-agent hidden_tool_is_rejected_as_unadvertised`: PASS.
- `cargo test -p opentake-agent --test advertised_tool_acceptance every_advertised_tool_is_live_or_absent -- --exact`: PASS.
- `cargo clippy -p opentake-tauri -p opentake-agent --all-targets -- -D warnings`:
  PASS.
- `cargo fmt --all -- --check`: PASS.
- `CARGO_INCREMENTAL=0 cargo test --workspace --no-fail-fast`: PASS.
