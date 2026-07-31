# CLI sidecar boundary packaged evidence — 2026-08-01

Parent: `MR-cli-sidecar-boundary-complete`

## Code boundary

- RED: corrected owners first failed because `resolve_cli_path` was absent.
- GREEN: environment override, PATH fallback, regular packaged-sidecar
  selection, and Unix symlink rejection pass without mutating process-wide
  environment variables.
- `ffmpeg-sidecar` is built with default features disabled, so OpenTake does not
  use its download stack and does not link a libav ABI binding.

## Final packaged macOS application

- App executable SHA-256:
  `a92c3e31d503300b87806ecd36aec48ba3128c49f202c9c78167e301b6f75711`.
- Bundled `ffmpeg` SHA-256:
  `68249fcf774472381c5ee0fd7bb91065ab8a8cd8ed3ab6d0672ebc2f33974190`.
- Bundled `ffprobe` SHA-256:
  `acf7b76a64dddcc099f272d2dfee4ae9b185244bd532ca54dc7d4d2379765940`.
- Both sidecars report version 6.0 and execute from
  `OpenTake.app/Contents/MacOS` beside `opentake`.
- `otool -L` on `opentake` reports no `libavcodec`, `libavformat`, `libavutil`,
  `libswscale`, or `libswresample` dynamic dependency.
- `codesign --verify --deep --strict` passes for the whole app. The signature is
  ad hoc; this is not Developer ID/notarization evidence.
- Packaged GUI probe/decode/playback/export paths used these sidecars in the
  immediately preceding Task15/16 real-device receipts.

## Distribution limitation

The current FFmpeg 6.0 binary reports a configuration containing both GPL
components and `--enable-nonfree`. This proves the technical CLI boundary, but
it is not a public-distribution license clearance. Replace it with a pinned,
license-compatible build and publish the matching notices before a public Beta.

## Result

`MR-cli-sidecar-boundary-complete` is **PASS** for the declared implementation
slice. Public release packaging remains blocked on the separately owned
sidecar-license/signing acceptance.
