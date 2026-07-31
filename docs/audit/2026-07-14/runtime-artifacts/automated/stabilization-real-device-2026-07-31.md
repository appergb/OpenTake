# Stabilization real-device evidence (2026-07-31)

Status: **macOS packaged acceptance complete for Media Render Task 5.** This
receipt closes `MR-stabilization`; it does not claim Developer ID signing,
notarization, Windows acceptance, or Beta release readiness.

## Bound contract

- `media-render-playback-export-implementation.md`, Task 5,
  `implementation-slice-7ef9369889a0a0d6` /
  `requirement-20198476e9083261`.
- Owning test:
  `crates/opentake-render/tests/stabilization.rs#synthetic_shake_produces_editable_undoable_preview_export_solution`.

## Historical RED and GREEN receipts

The owning test was added before implementation. The exact planned command
exited 101 with eight missing API/type errors across the domain, media, ops, and
render boundaries. After implementation:

```text
$ cargo test -p opentake-render --test stabilization synthetic_shake_produces_editable_undoable_preview_export_solution -- --exact
1 passed; 0 failed

$ pnpm -C web test
89 files passed; 806 tests passed

$ cargo fmt --all -- --check
exit 0

$ cargo clippy --workspace --all-targets -- -D warnings
exit 0 (only the existing future-incompatibility notice for block 0.1.6)

$ cargo test --workspace --no-fail-fast
all workspace unit, integration, and doc-test binaries passed; only the seven
explicit real-device probe tests remained ignored
```

The owning test verifies that a deterministic jitter sequence has lower
post-stabilization displacement, the safety crop covers every output corner,
preview/export sample the exact same composed transform, apply/reset are
non-destructive, and undo restores the prior document. Media tests cover block
motion and cooperative cancellation; Tauri tests cover single-flight token
cancellation and command routing; Web tests cover the Inspector and native
playback route.

## Packaged application receipt

The release application was rebuilt with the repository-locked Tauri CLI. The
final binary SHA-256 was:

```text
fcea9121b4b26fe1fdf191cd03734c54e5685fc0ddf7909dfec15c39235aed4a
```

`codesign --verify --deep --strict --verbose=2` reported the ad-hoc signed app
valid on disk and satisfying its designated requirement, including packaged
`ffmpeg` and `ffprobe`. This proves local integrity only.

## Packaged GUI receipt

An isolated 1920×1080, 60 fps project containing a 2-second 320×180, 24 fps
synthetic jitter clip was opened through the native project picker in the exact
release `.app`. The Inspector analysis produced
`opentake.motion-smoothing v1` with 48 motion samples. The UI then verified:

- strength 100% → 65%; additional crop 0% → 3%;
- undo restored crop to 0%, redo returned it to 3%;
- reset removed the track and undo restored the complete 65% / 3% solution;
- explicit save and quit/reopen retained model/version, source identity, 48
  keyframes, strength, and crop while starting with an empty history stack;
- the clip media reference and source file were unchanged.

The persisted correction track had maximum absolute horizontal correction
0.0075 and conservative coverage zoom. A playback-routing defect found during
this run was fixed so every stabilized clip uses the desktop compositor rather
than WebKit's source-video path.

## Cancellation receipt

A separate 60-second 3840×2160 long-GOP fixture kept analysis running long
enough to exercise cancellation. The packaged UI remained interactive while
showing `正在分析运动…` and `取消分析`. Clicking cancel immediately restored the
existing 48-sample solution, emitted no error, and left both undo and redo
disabled. The decode/motion work runs on a background blocking task so the
cancel command can be dispatched concurrently.

## Export and preview/export parity

The packaged Export dialog produced `stabilization-export-2026-07-31.mp4`:

```text
codec=h264
width=1920
height=1080
pixel_format=yuv420p
avg_frame_rate=60/1
duration=2.000000
nb_frames=120
preview/export frame 0 SSIM: 0.999557
preview/export frame 0 PSNR: 56.843845 dB
```

Tracked artifacts:

- `stabilization-input-2026-07-31.mp4` — SHA-256
  `2c344578e303e5c2c4a727eac1ea3ce79bb1a9e4aa6b40ab6535e4b8363f3c69`
- `stabilization-preview-frame-000-2026-07-31.png` — SHA-256
  `24e959f29c3f324923f77322d7cab8225df3cca6b98eec1a7a05f153cba598f5`
- `stabilization-export-2026-07-31.mp4` — SHA-256
  `18440c7d62fb4afc38c28a1399a5f31a833fe3606befb9a2b3f22bca54a6ed4f`
- `stabilization-export-frame-000-2026-07-31.png` — SHA-256
  `58c14f1af53bd275e15e934b442a6d1526c3e4ee8e777587944d307bb818749a`
