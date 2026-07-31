# 3D LUT packaged-app verification (2026-07-31)

## Scope

This record closes implementation-plan Task 10 (`MR-lut`). It covers bounded
`.cube` parsing, project-managed content-addressed storage, transactional clip
editing, real GPU sampling, packaged preview/playback/export, and save/reopen.

This is local functional evidence only. It is not Developer ID signing,
notarization, or a Beta release claim.

## RED and GREEN evidence

The owning test was added before implementation and initially failed to compile
because `CubeLut`, `LutReference`, `GpuLutTexture`, `upload_lut_3d`, the render
plan field, and the resolver method did not exist.

Exact RED/GREEN command:

`cargo test -p opentake-render --test lut malformed_and_oversized_luts_fail_closed_and_valid_lut_matches_preview_export -- --exact --nocapture`

It now passes on the real Metal/wgpu device. The test verifies:

- malformed, oversized, and unsupported 16-point inputs fail closed;
- valid 17-point identity and 33-point known-transform tables parse;
- identity output remains within two code values;
- a known transform at 75% intensity visibly changes the expected channels;
- fresh preview and export renders are byte-identical;
- JSON save/reopen preserves the path-free reference.

The command test additionally verifies apply, intensity adjustment, removal,
undo, and redo. Project storage tests verify bounded no-follow reads and complete
Save As copying of `media/luts`. Tauri tests verify absolute-path enforcement,
symlink refusal on Unix, pre-copy validation, and SHA-256-addressed publication.

## Implementation boundary

- `CubeLut` accepts UTF-8 `.cube` input up to 4 MiB, exactly one ordered domain,
  and complete 17- or 33-point 3D tables with finite bounded values.
- A clip persists only `{id, name, intensity}`. The external import path is not
  retained; runtime paths are derived as `media/luts/<sha256>.cube`.
- Import validates a no-follow regular source through a retained project identity
  workflow before atomically publishing the managed asset.
- Preview, native playback, MCP inspection, and export resolve the same managed
  bytes, verify their content hash, parse them again, and fail closed on missing
  or tampered assets.
- RGBA16F 3D textures use texel-center-aligned hardware trilinear sampling after
  the primary grade/HSL chain and before the generic effect chain.
- Inspector provides native `.cube` selection, filename, intensity, removal,
  explicit errors, and the shared transactional undo/redo route.

## Workspace and package gates

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed. Cargo only
  repeated the repository's existing future-incompatibility notice for
  `block 0.1.6`.
- `cargo test --workspace --all-targets --quiet`: passed; existing tests that
  explicitly require optional external fixtures remained ignored by design.
- `pnpm -C web test`: 89 files, 810 tests passed.
- `pnpm -C web build`: passed with the existing non-blocking bundle-size and
  ineffective-dynamic-import warnings.
- `web/node_modules/.bin/tauri build --bundles app --no-sign`: passed.

Tested application:
`target/release/bundle/macos/OpenTake.app`

- executable SHA-256:
  `f8fbeaa1149652dde3175d2421274b6cb38280835ac755f2254f2106c4e5318f`
- ad-hoc CDHash: `acd9d8f381353d0bdbf10a4d498850ea965a15e5`
- `codesign --verify --deep --strict --verbose=2`: passed, including bundled
  `ffmpeg` and `ffprobe`.
- `Signature=adhoc`, `TeamIdentifier=not set`; this proves local bundle integrity
  only.

## Packaged application workflow

Fixture project:
`/private/tmp/opentake-lut-ui-real-device-20260731.opentake`

Imported LUT:
`/private/tmp/opentake-lut-ui-swap-rb-17.cube`

- size: 147473 bytes / 4917 lines
- SHA-256:
  `f9828fe9741ec855e6e2b6184de48471171ff906108d7c5e1f28bf1ca897d7ee`
- transform: `[r,g,b] -> [b,g*0.5,r]`

Through the visible packaged UI:

1. Created a native Save As copy of the prior real-device project and selected
   blue video clip `id-4`.
2. Imported the valid 17-point LUT through the native `.cube` picker. Inspector
   showed `opentake-lut-ui-swap-rb-17`, the undo action became available, and the
   preview changed immediately from lavender-purple to vivid magenta.
3. Set intensity to `0.350`; preview moved to the expected intermediate color.
   Undo restored `1.000`, and redo restored `0.350`.
4. Removed the LUT, observed `未选择`, then undid removal and recovered the LUT
   and its `0.350` intensity.
5. Saved, returned Home, reopened from Recents, and confirmed the name, intensity,
   and preview persisted while the new-session undo/redo stack was empty.
6. Native playback advanced from `00:00:00` to `00:06:18` with the LUT active.
7. Exported the complete timeline through the packaged H.264 export dialog.

After the final browser-fallback validation hardening, the bundle was rebuilt,
re-signed, reopened from Home, and again showed the persisted LUT and `0.350`
intensity with an empty history. Its native playback advanced from `00:00:00`
to `00:08:02`, and the complete export below was repeated from that final hash.

Persisted clip state:

```json
{"id":"id-4","lut":{"id":"f9828fe9741ec855e6e2b6184de48471171ff906108d7c5e1f28bf1ca897d7ee","name":"opentake-lut-ui-swap-rb-17","intensity":0.35}}
```

The managed copy exists only at
`media/luts/f9828fe9741ec855e6e2b6184de48471171ff906108d7c5e1f28bf1ca897d7ee.cube`
and has the same SHA-256 as the import. The project contains no external LUT
source path.

- `project.json` SHA-256:
  `e8d96cfa1a4fc2d227dfcc22ecf9542db93139b75dc0708572b075784ff8beb2`

## Complete export

`/private/tmp/opentake-lut-ui-final-package-20260731.mp4` probes as:

- H.264, 1920 x 1080, 30/1 fps, 920 frames
- AAC audio, 1439 frames
- duration `30.666667` seconds
- size `279902` bytes
- SHA-256:
  `908bde76040bbb653fc8f791a22dae5b0c776aff8da84fa33f5501e23f6cfc37`

Bundled FFmpeg sampled the opaque center of frame 0:

- source: RGB `[52, 89, 138]`
- prior packaged HSL-only export: RGB `[169, 100, 158]`
- packaged HSL + 35% LUT export: RGB `[167, 86, 161]`

The red/blue movement and green attenuation match the authored transform and
intensity. The owning real-GPU test supplies the exact identity and
preview/export byte-parity tolerances without lossy codec noise.

## Result

Task 10 is verified from typed bounded parsing and project-managed storage
through transactional Inspector editing, fresh-session persistence, native
playback, and complete packaged export.
