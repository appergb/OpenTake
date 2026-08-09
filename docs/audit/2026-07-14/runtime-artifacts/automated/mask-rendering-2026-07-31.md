# MR-mask-rendering real-device receipt — 2026-07-31

## Candidate

- Source branch: `agent/advanced-ai-workflows`
- Packaged app: `target/release/bundle/macos/OpenTake.app`
- Executable SHA-256: `05e9b2c16b8e418170330082d0996991af6b9bc614d8f33eb5eb5ea07b69656e`
- Code signature: local ad-hoc signature; `codesign --verify --deep --strict`
  passed after the exact candidate bundle was re-signed.
- Validation project:
  `target/runtime-validation/mr-optical-flow/OpticalFlow60.opentake`

## Required RED receipt

The reviewed planned test
`linear_circle_and_polygon_masks_match_cpu_reference_in_preview_and_export`
was added before implementation. It failed on the first polygon pixel with
`pixel=(0,0) expected=0 actual=255`, proving that the previous GPU polygon path
was a full-coverage no-op.

## Code verification

- `cargo test -p opentake-render --test gpu_effects circle_mask_clips_to_center -- --exact` — pass.
- `cargo test -p opentake-render --test gpu_effects linear_circle_and_polygon_masks_match_cpu_reference_in_preview_and_export -- --exact` — pass on the local GPU for linear, circle, and polygon shapes at feather `0` and `0.18`, with preview/export bytes equal and every channel within 3 levels of the CPU reference.
- `pnpm -C web test` — 89 files, 805 tests passed.
- `pnpm -C web build` — production build passed.
- `cargo fmt --all -- --check` — pass.
- `cargo clippy --workspace --all-targets -- -D warnings` — pass.
- `cargo test --workspace --no-fail-fast` — pass; seven pre-existing real-device probes remain intentionally ignored by the unit gate.

The command layer rejects more than four masks or polygon paths outside 3–16
points, so the fixed GPU uniform cannot silently truncate editor/Agent changes.

## Packaged UI verification

All actions below were executed through the packaged `.app`, not a development
server:

1. Reopened the saved 60 fps project and enabled a mask on its video clip.
2. Switched Circle to the enabled `Pen / Polygon` option.
3. Dragged P1 directly in Preview from `(0.250, 0.250)` to approximately
   `(0.143, 0.140)`; the overlay and Inspector updated together.
4. Added P5 `(0.300, 0.800)`, then deleted P5.
5. Undid the point drag (P1 returned to `0.250`) and redid it (P1 returned to
   `0.143`).
6. Set mask offset X `0.100`, rotation `20°`, feather `0.100`, and invert on.
   Preview showed the rotated/translated polygon and the expected reversed soft
   boundary over the moving white-square fixture.
7. Saved, quit, reopened, and reselected the clip. Shape, four points, offset,
   scale, rotation, feather, and invert persisted exactly; the reopened undo
   stack was correctly empty.
8. Exported `H.264`, `1920×1080`, `60/1 fps`, `2.000 s`, exactly `120` decoded
   frames, then captured the same preview frames through the packaged app.

The persisted mask payload in `project.json` contains the four edited polygon
points plus `feather: 0.1`, `invert: true`, `offset.x: 0.1`, identity scale, and
`rotationDegrees: 20.0`.

## Preview/export pixel comparison

Artifacts live in
`runtime-artifacts/automated/mask-rendering-2026-07-31/`:

- `preview-frame-000.png` / `export-frame-000.png`: pixel-identical, SSIM 1.0 and infinite PSNR.
- `preview-frame-060.png` / `export-frame-060.png`: soft inverted polygon boundary is visible; SSIM `0.999753`, average PSNR `62.854540 dB`.
- `mask-runtime-h264-1080p.mp4`: playable H.264 export, 120 frames.

SHA-256:

- `export-frame-000.png`: `bc17b329a39fdc77b8db61cd72ecf8b8da7a7881e329096c412bce91fc531ab8`
- `export-frame-060.png`: `eb5c7e2399a9b4c6d77c96ec3bcd1e30a9778d8c040d5372085c86c83f241cc4`
- `mask-runtime-h264-1080p.mp4`: `362f5f959ccb3756b7a30d3fb387a78870620c64e8e8ed3923dbf8bf3fcd7231`
- `preview-frame-000.png`: `564fe64317b8abae9bc593ee47aca81a6790fc8fea2e1fbfdb34e093f6a10add`
- `preview-frame-060.png`: `1feb50b313e7072920cc486199bea36749f49ba5d2345e7937742380e08e147d`
