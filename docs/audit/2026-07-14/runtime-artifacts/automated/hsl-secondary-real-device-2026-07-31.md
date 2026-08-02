# HSL secondary packaged-app verification (2026-07-31)

## Scope

This record closes implementation-plan Task 9 (`MR-hsl-secondary`). It covers a
persisted feathered hue qualifier, CPU/WGSL agreement, selected-hue isolation,
Inspector editing and recovery, and the complete packaged preview/export path.

This is local functional evidence only. It is not Developer ID signing,
notarization, or a Beta release claim.

## RED and GREEN evidence

The owning test was added before implementation and initially failed to compile:

- unresolved import `opentake_domain::HslSecondary`
- `ColorGrade` had no `hsl_secondary` field

Exact RED/GREEN command:

`cargo test -p opentake-render --test gpu_effects hsl_secondary_hue_boundary_feather_and_isolation -- --exact`

The same command now passes on the real Metal/wgpu device. Its 16 x 16 chart
contains red, feather-boundary orange, green, and blue bars. It verifies:

- selected red changes by more than 20 code values;
- boundary orange changes by more than 2 but less than selected red;
- isolated green and blue remain within 2 code values;
- fresh preview and export renders are byte-identical;
- JSON save/reopen preserves every HSL parameter exactly.

Domain tests additionally cover red-range wraparound (`0 == 1`), grey-pixel
isolation, and stable rejection of a zero-width selector. The command test
persists the nested grade and restores it through undo and redo.

## Implementation boundary

- `HslSecondary` persists normalized hue center, full range width, feather, hue
  rotation, relative saturation, and additive lightness.
- Validation rejects non-finite/out-of-range values before command mutation and
  before source resolution in the compositor.
- The CPU reference and WGSL both use circular hue distance and an inward
  smoothstep feather. Achromatic pixels are never selected.
- Two named vec4 uniform blocks carry qualifier metadata and adjustments after
  the primary exposure/white-balance/LGG/contrast/saturation chain.
- Inspector exposes enable, six editable parameters, and reset. Every edit uses
  the existing transactional `SetColorGrade` undo/redo path.
- Browser fallback normalization and validation mirror the Rust ranges.

## Workspace and package gates

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed. Cargo only
  repeated the repository's existing future-incompatibility notice for
  `block 0.1.6`.
- `cargo test --workspace --no-fail-fast`: passed; explicit real-device-only
  probes remained ignored by design.
- `pnpm -C web test`: 89 files, 809 tests passed.
- `pnpm -C web build`: passed with the existing non-blocking bundle-size and
  dynamic-import warnings.
- `web/node_modules/.bin/tauri build --bundles app --no-sign`: passed.

Tested application:
`target/release/bundle/macos/OpenTake.app`

- executable SHA-256:
  `348d9664cc7e4b72fa32e07aa0cdf6ad7e85e8ffca669076afbb10393f94f9df`
- ad-hoc CDHash: `88ce65b3757e702ae63100c9ace54854fec2e584`
- `codesign --verify --deep --strict --verbose=2`: passed, including bundled
  `ffmpeg` and `ffprobe`.
- `Signature=adhoc`, `TeamIdentifier=not set`; this proves local bundle
  integrity only.

## Packaged application workflow

Fixture:
`/private/tmp/opentake-hsl-ui-real-device-20260731.opentake`
(native Save As copy of the real 30 fps talking-head project).

Through the visible packaged UI:

1. Selected blue video clip `id-4`, enabled HSL Secondary, and authored center
   `0.667`, width `0.240`, feather `0.080`, hue shift `0.250`, saturation
   `-0.250`, and lightness `0.100`.
2. Preview changed immediately from blue to purple, proving the Inspector state
   reaches the packaged GPU render path.
3. Five undo operations restored the absent HSL qualifier and blue preview;
   five redo operations restored all values and the purple preview.
4. Reset cleared the qualifier. Undo restored all six values, redo cleared it,
   and a final undo restored the authored state.
5. Saved, returned Home, reopened from Recents, and confirmed all values and the
   purple preview persisted. The fresh session correctly disabled undo/redo.
6. Native playback advanced from frame 0 to `00:01:07` with HSL active.
7. Exported the complete timeline from the packaged H.264 export dialog.

Precise persisted state:

```json
{"id":"id-4","colorGrade":{"exposure":0.0,"temperature":0.0,"tint":0.0,"liftGammaGain":{"lift":{"r":0.0,"g":0.0,"b":0.0},"gamma":{"r":1.0,"g":1.0,"b":1.0},"gain":{"r":1.0,"g":1.0,"b":1.0}},"contrast":0.0,"saturation":1.0,"hslSecondary":{"hueCenter":0.667,"hueWidth":0.24,"feather":0.08,"hueShift":0.25,"saturation":-0.25,"lightness":0.1}}}
```

- `project.json` SHA-256:
  `5e3e028d18f254cfd2751679979163feec3c59d2a3009b72c534e32654b6c857`

## Complete export

`/private/tmp/opentake-hsl-ui-real-device-20260731.mp4` probes as:

- H.264, 1920 x 1080, 30/1 fps, 920 frames
- AAC audio, 1439 frames
- duration `30.666667` seconds
- size `281291` bytes
- SHA-256:
  `42e121dc2d3814b4af7d1f5fc184da85501821a9b3c8ff4dd3e865952c413707`

Bundled FFmpeg sampled the opaque center of frame 0:

- source: RGB `[52, 89, 138]`
- packaged export: RGB `[169, 100, 158]`

The expected blue-to-purple rotation is therefore present in the complete
encoded result, while the owning chart test supplies the exact preview/export
isolation tolerance for hues outside the qualifier.

## Result

Task 9 is verified from bounded persisted model and transactional editing through
CPU math, real wgpu chart isolation, packaged Inspector recovery, save/reopen,
native playback, and complete export.
