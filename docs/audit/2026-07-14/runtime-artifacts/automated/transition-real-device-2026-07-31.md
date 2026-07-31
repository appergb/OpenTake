# Editable cross-dissolve packaged-app verification (2026-07-31)

## Scope

This record closes implementation-plan Task 7 (`MR-transitions`) for the first
editable clip-to-clip transition vertical slice. It verifies the Rust product
boundary first, then the rebuilt packaged macOS application against an isolated
copy of a real 30 fps project.

This is local functional evidence only. It is not Developer ID signing,
notarization, or a Beta release claim.

## Test-first implementation evidence

- Owning test added at
  `crates/opentake-render/tests/transitions.rs#adjacent_clip_transition_is_editable_undoable_and_matches_preview_export`.
- Historical RED command:
  `cargo test -p opentake-render --test transitions adjacent_clip_transition_is_editable_undoable_and_matches_preview_export -- --exact`
- RED failure: the serialized transition did not contain
  `"fromClipId":"a"`, proving the previous model did not persist both
  adjacent clip identities.
- The same exact command passed after implementation.
- The owning test covers add/change/remove, undo/redo, save/reopen, explicit
  rejection of an overlong transition without history mutation, and fresh GPU
  preview/export equality at the cut, midpoint, last mixed frame, and end.
- Its synthetic red-to-blue fixture also guards against midpoint darkening: the
  midpoint is purple, not two half-transparent layers over black.

## Code and build gates

All commands completed successfully against the Task 7 working tree:

- `npm test`: 89 test files, 807 tests passed.
- `npm run build`: passed (existing non-blocking bundle-size warnings only).
- Focused web tests for the transition tab, fallback command path, and timeline
  overlay: 3 files, 29 tests passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed. Cargo only
  repeated the repository's existing future-incompatibility notice for
  `block 0.1.6`.
- `cargo test --workspace --no-fail-fast`: passed. Explicit real-device-only
  tests remained ignored by design.
- `web/node_modules/.bin/tauri build --bundles app --no-sign`: passed.

The tested application was
`target/release/bundle/macos/OpenTake.app`:

- executable SHA-256:
  `a0fd7fdc734909699eb3c54cba96018fa566b87cea8e14b5638fc72a23f10f54`
- ad-hoc CDHash: `3325a88d8ac1bb1a7bfa777e64fc355fc9c34276`
- `codesign --verify --deep --strict --verbose=2`: passed, including the bundled
  `ffmpeg` and `ffprobe` helpers.
- `Signature=adhoc`, `TeamIdentifier=not set`; therefore this proves local
  bundle integrity only.

## Packaged application workflow

Fixture:
`/private/tmp/opentake-transition-real-device-20260731.opentake`
(isolated copy; source media remained external and unmodified).

The packaged application completed the following workflow through its visible
UI:

1. Opened the fixture and selected the enabled **Transition** media tab.
2. Loaded a legacy cross-dissolve that contained only `toClipId`; the UI showed
   it correctly without corrupting the project.
3. Removed the transition, undid the removal, redid it, then added it again.
4. Changed its duration to 20 frames (0.67 seconds), applied it, and saved.
5. Returned to Home, reopened the project from Recents, and confirmed the
   20-frame transition persisted while the fresh session had no stale undo or
   redo history.
6. Started playback before the transition at frame 140. Playback advanced past
   the cut at frame 151 and reached frame 919 (`00:30:19`), proving native
   playback crossed the transition boundary without stalling or crashing.
7. Returned exactly to frame 140 (`00:04:20`), captured the native preview, and
   exported the complete timeline through the packaged application's export
   dialog.

The saved model contains both adjacent identities and the requested duration:

```json
{"id":"id-4","transitionOut":{"fromClipId":"id-4","toClipId":"c7e1e6d4-6be2-4844-a827-c4fa9e20d402","kind":"crossDissolve","durationFrames":20}}
```

- `project.json` SHA-256:
  `e7e7d1aede80a0dd2c647ef5c13106aa1c27621c08011a3cc8e2b731d3e090f9`
- post-capture `media.json` SHA-256:
  `7df2035ed2a328495dabac49a9ec6801be971b1ed265153276f1a0ba7f98f981`

## Preview/export parity

The packaged application export probes as:

- H.264 video, 1920 x 1080, 30/1 fps, 920 frames
- AAC audio
- duration `30.666667` seconds
- file size `273566` bytes
- SHA-256:
  `132f00e123a805006da55e93cc7074ac2b78762eaebfd16e36a19ffe2065a0ee`

Frame 140 was extracted from the complete export and compared with the native
preview capture at the same timeline frame using the bundled FFmpeg:

- SSIM: `0.997060` (`25.317063` dB)
- PSNR average: `37.911328` dB
- both frames: PNG, 1920 x 1080

The small difference is the expected H.264 chroma/quantization loss; geometry,
transition composition, and the active text overlay match visually.

## Artifacts

- `transition-packaged-ui-2026-07-31.png` — reopened packaged UI with the
  persisted 20-frame transition.
- `transition-preview-frame-140-2026-07-31.png` — native packaged preview.
- `transition-export-2026-07-31.mp4` — complete packaged-app export.
- `transition-export-frame-140-2026-07-31.png` — exact exported comparison
  frame.

## Result

Task 7's first cross-dissolve vertical slice is verified through its owning
tests, workspace gates, save/reopen behavior, undo/redo, packaged playback, and
complete export. Additional wipe, slide, and 3D transition types remain future
library expansion and are not part of this task's acceptance contract.
