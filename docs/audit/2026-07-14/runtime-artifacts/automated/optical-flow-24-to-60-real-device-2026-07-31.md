# Optical-flow 24-to-60 real-device evidence (2026-07-31)

Status: **macOS packaged acceptance complete for Media Render Task 3.** This
receipt closes `MR-optical-flow`; it does not claim Developer ID signing,
notarization, Windows acceptance, or Beta release readiness.

## Bound contract

- `media-render-playback-export-implementation.md`, Task 3,
  `implementation-slice-c85c1acc35668396` /
  `requirement-5933e802c9dfe372`.
- Owning test:
  `crates/opentake-render/tests/optical_flow.rs#two_frame_fixture_is_deterministic_and_matches_preview_export`.
- Quality regression:
  `crates/opentake-render/tests/optical_flow.rs#optical_flow_tracks_opposing_local_motion_without_global_frame_shift`.

## Historical RED receipt

The reviewed owning test was added before the implementation and the exact
planned command exited 101. Compilation reported the five missing product
boundaries: media interpolation types, media conversion/interpolation
functions, render interpolation config/types, and the resolver interpolation
method. This is the expected RED for a previously absent backend.

## Automated GREEN receipts

```text
$ cargo test -p opentake-render --test optical_flow two_frame_fixture_is_deterministic_and_matches_preview_export -- --exact
1 passed; 0 failed

$ cargo test -p opentake-render --test optical_flow
2 passed; 0 failed

$ cargo fmt --all -- --check
exit 0

$ cargo clippy --workspace --all-targets -- -D warnings
exit 0 (only the existing future-incompatibility notice for block 0.1.6)

$ cargo test --workspace --no-fail-fast
all workspace unit, integration, and doc-test binaries passed; only the seven
explicit real-device probe tests remained ignored
```

The owning test verifies endpoint-stable 24-to-60 mapping, exact source alpha
values, deterministic pixel output, monotonic motion, preview/export policy
parity, explicit nearest/blend/error fallback behavior, and invalid-rate
rejection. The second regression uses two regions moving in opposite directions
so a single whole-frame translation cannot satisfy the fixture.

## Packaged application receipt

The release application was rebuilt with the repository-locked Tauri CLI and
the final local block-motion implementation:

```text
$ web/node_modules/.bin/tauri build --bundles app
Finished 1 bundle at target/release/bundle/macos/OpenTake.app

$ codesign --verify --deep --strict --verbose=2 target/release/bundle/macos/OpenTake.app
OpenTake.app: valid on disk
OpenTake.app: satisfies its Designated Requirement
```

The local bundle was ad-hoc signed, including packaged `ffmpeg` / `ffprobe`.
That proves local integrity only and is not a distributable signature.

## Packaged GUI receipt

The exact release `.app` was operated through the macOS accessibility surface.
A project was created through the native Save panel. Because the current
project-format UI displays FPS but does not edit it, the saved fixture's
`project.json` was set from 30 to 60 fps while the application was closed, then
reopened through the Home screen. The packaged UI displayed both the `60` badge
and `帧率 60 fps`. Importing the 320x180, 24 fps motion fixture raised the
expected format-mismatch dialog; **保持当前设置** retained the 1920x1080,
60 fps project.

The deterministic fixture is 48 H.264 frames over 2 seconds. Its white square
moves from source x=20..59 to x=22..61 between frames 0 and 1; at the project
scale those left edges are x=120 and x=132. At project frame 1 (1/60 second),
the packaged paused preview capture had bounding box:

```text
x=126..363, y=421..658
```

The x=126 left edge lies strictly between the two source positions, proving an
interpolated motion frame rather than nearest-frame repetition. A second
packaged capture using the multi-region `testsrc2` fixture visually retained
the stationary color regions while interpolating local motion; it exposed no
whole-frame translation.

## Export and preview/export parity receipt

The packaged Export dialog produced `optical-flow-24-to-60-export-2026-07-31.mp4`.
`ffprobe` and the decoded output frame reported:

```text
codec=h264
width=1920
height=1080
avg_frame_rate=60/1
duration=2.000000
nb_frames=120
export frame 1 bbox: x=126..363, y=421..658
preview/export frame 1 SSIM: 0.999515
preview/export frame 1 PSNR: 63.644069 dB
```

The identical motion bounding box proves temporal parity. The remaining pixel
difference is the expected H.264 encode loss; the in-memory owning test asserts
bit-exact preview/export resolver output before encoding.

Tracked exact artifacts:

- `optical-flow-moving-square-input-2026-07-31.mp4` — SHA-256
  `2c344578e303e5c2c4a727eac1ea3ce79bb1a9e4aa6b40ab6535e4b8363f3c69`
- `optical-flow-preview-frame-001-2026-07-31.png` — SHA-256
  `7d306ac1e3e5e891404d89bfcca2c4b067ca3324f8c63d02c55ff077cc5eedd4`
- `optical-flow-24-to-60-export-2026-07-31.mp4` — SHA-256
  `35d7c230db896e9559509a203421cc31a4d0651dea30c9a24a68db90e7d459f6`
- `optical-flow-export-frame-001-2026-07-31.png` — SHA-256
  `5adfddf3bc52bd666d24a24158fba4baa159af1f9e68d2ce86e6c37d3582fdae`
- `optical-flow-complex-preview-frame-001-2026-07-31.png` — SHA-256
  `02eb6dee137db6053833fe02d923014c770f86c00a98450923bcd056cb13c130`

This task's mapped preview boundary is the high-quality paused composite path.
The separately governed low-latency continuous-playback stream retains its
existing real-time frame-normalization policy; this receipt makes no claim that
the streaming decoder runs the same block-motion algorithm.
