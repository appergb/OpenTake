# Lift / gamma / gain packaged-app verification (2026-07-31)

## Scope

This record closes implementation-plan Task 8 (`MR-lgg-proof`). It verifies the
authored lift/gamma/gain formula against the CPU reference and real wgpu output,
then exercises the same controls through the rebuilt packaged macOS application.

This is local functional evidence only. It is not Developer ID signing,
notarization, or a Beta release claim.

## RED evidence

The existing CPU model and WGSL shader both used
`pow(gain * (x + lift), 1 / gamma)`. That made lift a uniform offset across the
whole range and put gain inside the gamma power, contrary to the documented
color-wheel contract
`gain * pow(x + lift * (1 - x), 1 / gamma)`.

- `cargo test -p opentake-domain lift_gamma_gain_gain_scales` failed to compile
  after the validation contract was added to the owning test because
  `ColorGrade::validate` did not exist.
- `cargo test -p opentake-render --test gpu_effects lift_gamma_gain_matches_cpu_reference -- --exact`
  failed with `expected 0.32364660303542436, got 0.36317811652316606`.

These failures independently proved the missing hostile-input boundary and the
old CPU/GPU formula drift from the source requirement.

## Implementation and GREEN evidence

- `ColorGrade::apply_linear` and WGSL now share
  `gain * pow(max(x + lift * (1 - x), 0), 1 / gamma)` per channel.
- Lift rolls off to zero influence at white, gamma shapes mid-tones, and gain
  remains the final channel multiplier.
- `ColorGrade::validate` provides stable typed field/rule errors for every
  Inspector range and rejects NaN/Inf. Gamma is strictly within `(0, 4]`.
- `SetColorGrade` validates before transaction mutation. A zero-gamma request
  leaves the clip, timeline version, and undo stack unchanged.
- The compositor validates persisted grades before source resolution, so a
  damaged project cannot degrade into an unchanged frame or submit NaN/Inf GPU
  uniforms.
- `lift_gamma_gain_matches_cpu_reference` checks a non-neutral three-channel
  fixture, preview/export byte equality, CPU/source-formula equality, GPU pixel
  tolerance, and the pre-resolution invalid-grade refusal.

Focused GREEN commands:

- `cargo test -p opentake-domain lift_gamma_gain_gain_scales`
- `cargo test -p opentake-domain color_grade_rejects_non_finite_and_zero_gamma`
- `cargo test -p opentake-ops --test command_apply set_color_grade_rejects_invalid_without_mutation -- --exact`
- `cargo test -p opentake-render --test gpu_effects lift_gamma_gain_matches_cpu_reference -- --exact`

All passed.

## Workspace and package gates

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed. Cargo only
  repeated the repository's existing future-incompatibility notice for
  `block 0.1.6`.
- `cargo test --workspace --no-fail-fast`: passed; explicit real-device-only
  probes remained ignored by design.
- `pnpm -C web test`: 89 files, 808 tests passed. The browser fallback rejects
  the same invalid gamma before mutation, and the Inspector's gamma minimum is
  `0.01`, so its visible control cannot author a value the Rust boundary rejects.
- `web/node_modules/.bin/tauri build --bundles app --no-sign`: passed. Its
  `pnpm -C web build` prerequisite also passed with the existing non-blocking
  bundle-size/dynamic-import warnings.

Tested application:
`target/release/bundle/macos/OpenTake.app`

- executable SHA-256:
  `638e22fd2c56157d08a778e31122ab90ae133c02327ca584cf4a523564baf246`
- ad-hoc CDHash: `6ba8caf5dcac7835c773158e335e0d8eb5df8666`
- `codesign --verify --deep --strict --verbose=2`: passed, including bundled
  `ffmpeg` and `ffprobe`.
- `Signature=adhoc`, `TeamIdentifier=not set`; this proves local bundle
  integrity only.

## Packaged application workflow

Fixture:
`/private/tmp/opentake-lgg-real-device-20260731.opentake`
(isolated copy of the real 30 fps talking-head project).

Through the visible packaged UI:

1. Selected video clip `id-4` and scrolled to the Inspector color-grade section.
2. Scrubbed Lift R to `0.10`, Gamma R to `1.79`, and Gain R to `0.82`.
3. The native preview changed immediately from the source blue to purple,
   visibly proving that the controls are rendered rather than inert metadata.
4. Four undo operations restored Lift/Gamma/Gain to `0.00/1.00/1.00` and the
   default blue image. Four redo operations restored `0.10/1.79/0.82` and the
   purple image.
5. Saved, returned Home, reopened the project from Recents, selected `id-4`, and
   confirmed all values persisted. The fresh session correctly showed disabled
   undo and redo controls.
6. Native playback advanced from frame 0 to frame 224 (`00:07:14`) with the grade
   active and without a stall or crash.
7. Captured native frame 0 and exported the complete timeline through the
   packaged application's export dialog.

The precise persisted values (scrubbing retains sub-display precision) are:

```json
{"id":"id-4","colorGrade":{"exposure":0.0,"temperature":0.0,"tint":0.0,"liftGammaGain":{"lift":{"r":0.0961456298828125,"g":0.0,"b":0.0},"gamma":{"r":1.793203125,"g":1.0,"b":1.0},"gain":{"r":0.8197265625,"g":1.0,"b":1.0}},"contrast":0.0,"saturation":1.0}}
```

- `project.json` SHA-256:
  `213859fd864f1717ade84c0c291a07a7ad51127e87c22b6ee897c0e65e96f6a6`
- post-capture `media.json` SHA-256:
  `0189ca41d6ae79eab408b4dd7081a4105151fd186321c91926cc69d47041a2fe`

## Preview/export parity

The complete packaged export probes as:

- H.264, 1920 x 1080, 30/1 fps, 920 frames
- AAC audio
- duration `30.666667` seconds
- size `280672` bytes
- SHA-256:
  `620d1b4133feeabf2a4e75a90ee2204cfffa91fa29829ed8575af975c2033b34`

Native preview frame 0 and exact exported frame 0 were compared with the bundled
FFmpeg:

- SSIM: `0.999283` (`31.445265` dB)
- PSNR average: `38.469386` dB
- both frames: PNG, 1920 x 1080

The only difference is expected H.264 chroma/quantization loss; the purple LGG
result and geometry match visually.

## Artifacts

- `lgg-packaged-ui-2026-07-31.jpg` — packaged Inspector plus visibly graded
  preview.
- `lgg-preview-frame-0-2026-07-31.png` — native packaged preview.
- `lgg-export-2026-07-31.mp4` — complete packaged-app export.
- `lgg-export-frame-0-2026-07-31.png` — exact exported comparison frame.

## Result

Task 8 is verified from typed model/command boundaries through CPU math, wgpu,
packaged Inspector editing, undo/redo, save/reopen, native playback, and complete
export parity.
