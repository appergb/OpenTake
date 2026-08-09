# Generic Effects Real-Device Verification — 2026-07-31

## Scope and contract

- Plan item: `MR-generic-effects` / Task 6.
- Closed persisted registry: `grayscale`, `sepia`, `invert`.
- Each effect accepts an optional finite `amount` in `0.0..=1.0` (default `1.0`).
- Unknown effect names, unknown parameters, non-finite values, out-of-range values, and chains longer than eight return typed validation errors. They do not silently render unchanged.
- Inspector add, reorder, parameter, enable/disable, and remove mutations route through the undoable `SetEffects` command.
- Preview and export use the same ordered GPU effect implementation.

## Reviewed RED/GREEN owning test

Owning test:

```text
crates/opentake-render/tests/gpu_effects.rs#advertised_effect_registry_has_preview_export_golden_fixtures
```

The required focused command was first run before implementation and failed to compile because `effect_registry` and `Effect::validate` did not exist. After implementation, the exact command passed:

```sh
cargo test -p opentake-render --test gpu_effects advertised_effect_registry_has_preview_export_golden_fixtures -- --exact
```

The test asserts the exact registry, default and non-default golden pixels for every advertised effect, observable chain order, fresh preview/export byte equality, and typed rejection of an unknown effect.

## Code and package gates

All gates completed successfully on the source used for this app bundle:

- `npm test`: 89 test files and 807 tests passed.
- `npm run build`: passed; only the repository's existing chunk-size warnings were emitted.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed; Cargo only reported the pre-existing `block 0.1.6` future-incompatibility notice.
- `cargo test --workspace --no-fail-fast`: passed; explicit real-device tests remained ignored as designed.
- `web/node_modules/.bin/tauri build --bundles app --no-sign`: passed.

Packaged binary:

```text
target/release/bundle/macos/OpenTake.app/Contents/MacOS/opentake
SHA-256 a4b77109d9b59a264c7657fa71d3aaaad8655d73669b7f922604e5dfeb1f17f3
```

`codesign --verify --deep --strict --verbose=2` passed for the app and bundled `ffmpeg`/`ffprobe`. The inspected signature is ad hoc (`Signature=adhoc`, no TeamIdentifier); this is local integrity evidence only, not Developer ID signing or notarization evidence.

## Packaged macOS GUI workflow

The packaged app was operated through macOS accessibility/Computer Use against an isolated copy of the real `TalkingHeadQA` project:

```text
/private/tmp/opentake-generic-effects-real-device-20260731.opentake
```

Observed sequence:

1. Selected visual clip `id-4` and added grayscale at 41%, then sepia.
2. Moved sepia above grayscale; undo restored the prior order and redo restored the new order.
3. Disabled and re-enabled grayscale.
4. Removed sepia and used undo to restore it.
5. Added invert, producing the ordered enabled chain `sepia(1.0) -> grayscale(0.41) -> invert(1.0)`.
6. Native playback advanced the playhead without an unsupported-effect error and the rendered preview visibly changed from the blue source to the expected light grayscale/inverted result.
7. Saved, returned home, and reopened the project from Recent. The exact chain, parameter, and enabled states persisted; undo and redo were disabled on this fresh project session.
8. Captured frame 0 through the app's native “capture current frame” action and exported the full timeline through the packaged H.264/1080p UI path.

Persisted `project.json` SHA-256:

```text
138ce3e2a7f7fb409a742119e987f8b1b70c9b5e65a967cf8d4fbfc427226aac
```

The persisted clip payload was inspected directly and contained:

```json
{
  "id": "id-4",
  "mediaRef": "id-1",
  "effects": [
    { "name": "sepia", "params": {}, "enabled": true },
    { "name": "grayscale", "params": { "amount": 0.41 }, "enabled": true },
    { "name": "invert", "params": {}, "enabled": true }
  ]
}
```

## Export and preview parity

Bundled `ffprobe` reported the UI-exported artifact as H.264 video plus AAC audio, 1920×1080, 30 fps, 920 video frames, and 30.666667 seconds. SHA-256:

```text
7af20a51688ff3d2522e318ffeb16f126d1fd55d8bcf5df461de1f2696609fab
```

The native preview capture and decoded export frame 0 are both 1920×1080. Bundled `ffmpeg` comparison produced:

```text
SSIM All: 0.999838 (37.902042)
PSNR average: 39.679823 dB
```

The small non-zero delta is consistent with H.264 encoding; both frames show the same ordered effect result. Exact uncompressed preview/export byte equality is separately enforced by the owning GPU test.

## Evidence artifacts

- `generic-effects-packaged-ui-2026-07-31.png` — reopened packaged UI showing the persisted ordered chain and 41% parameter.
- `generic-effects-preview-frame-2026-07-31.png` — native packaged-app frame capture.
- `generic-effects-export-2026-07-31.mp4` — full packaged-app H.264 export.
- `generic-effects-export-frame-2026-07-31.png` — decoded export frame 0 used for parity measurement.

This evidence completes Task 6 only. It does not close the project-wide Developer ID/notarization, Windows real-device, CI-dispatch, or Beta-release gates.
