# Headless Chromium real-device verification — 2026-08-01

Scope: Task 24 / `MR-native-chromium`. This receipt closes the optional native
HTML/CSS/JS renderer only. It does not claim that the desktop app has wired a
motion-graphic timeline tool, and it is not Beta-release approval.

## Runtime

- Host: macOS, Apple Silicon, Asia/Shanghai
- Browser: Google Chrome `150.0.7871.187`
- Executable: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`
- Backend: direct Chrome DevTools Protocol over a local WebSocket
- Cargo feature: `opentake-motion/chromium`

## RED

The reviewed-planned integration owner was added first and run with the live
feature. Compilation failed because the baseline had no
`MotionCancellationToken`, browser discovery/path override, cancellation API,
`MotionError::Cancelled`, or live renderer. The baseline feature branch still
returned `RendererUnavailable` unconditionally.

## Live acceptance

Command:

```text
cargo test -p opentake-motion --features chromium --test chromium virtual_time_network_csp_timeout_cleanup_and_frame_identity -- --exact --nocapture
```

Result: PASS, one exact test executed. It verified all of the following through
a real Chrome process:

- The same three-frame animation rendered into separate cache roots twice with
  byte-identical PNGs.
- Frame 0 and frame 2 differed visibly, proving `OpenTake.seek(i/fps)` advanced
  virtual time rather than returning a static screenshot.
- A real browser PNG decoded through the injected `MotionClipSource` decoder as
  a `48x32` RGBA frame.
- A loopback HTTP origin explicitly added to `SandboxPolicy` was requested and
  served; the render completed.
- A non-allowlisted HTTPS resource and a `file:///etc/passwd` resource both
  failed closed as `MotionError::Sandbox`.
- A runaway `while(true)` author script hit the 500 ms budget and returned
  `MotionError::Timeout`.
- A deliberately crashing browser executable returned `RenderFailed`; empty
  source returned `InvalidSource`.
- An in-flight runaway render was cancelled from another thread and returned
  `MotionError::Cancelled`.
- Policy failure removed partial `frame_*.png` output. Across success, sandbox
  failure, timeout, crash, malformed input, and cancellation, no process-scoped
  `opentake-chromium-*` profile directory remained.

The allowlist pure tests additionally prove that an origin prefix lookalike such
as `https://cdn.jsdelivr.net.evil.example` and a loopback lookalike such as
`http://localhost.evil.example` are rejected.

## Gates

- Default focused owner: PASS; one exact fail-closed test executed.
- Default planned integration owner: PASS; one exact test executed and asserted
  `RendererUnavailable` without launching a browser.
- Feature-enabled motion package: PASS — 55 unit + 1 Chromium integration + 3
  pipeline tests.
- `cargo clippy -p opentake-motion --all-targets -- -D warnings`: PASS.
- `cargo clippy -p opentake-motion --all-targets --features chromium -- -D warnings`:
  PASS.
- `cargo fmt --all -- --check`: PASS.
- `cargo test --workspace --no-fail-fast`: PASS. The first attempt stopped at
  archive creation with `No space left on device`; only generated
  `target/debug/deps` and `target/debug/incremental` were deleted, preserving
  `target/release`, and the identical command then passed.

## Remaining boundary

`HeadlessChromiumRenderer` remains feature-gated and `MotionClipSource` remains
an unconnected library adapter. Desktop/core/agent motion materialization,
timeline placement, packaged browser provisioning on Windows, and the planned
Motion Canvas plugin are separate owned tasks and remain open.
