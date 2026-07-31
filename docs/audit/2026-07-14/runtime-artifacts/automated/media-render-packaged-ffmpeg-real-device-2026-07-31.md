# Media Render Task 33 — Packaged FFmpeg real-device evidence (2026-07-31)

Status: **macOS acceptance complete; Windows installed-app CI receipt pending.**
Do not close `MR-packaged-ffmpeg` until the exact-SHA Windows product job has
installed the NSIS artifact and passed the same owning smoke test.

## Bound contract

- Plan: `media-render-playback-export-implementation.md`, Task 33
  (`implementation-slice-ddfcf34d5292a998`).
- Candidate: `requirement-ff2faf0938e25f39`.
- Owning test:
  `scripts/tests/packaged-sidecars-test.rb#packaged_macos_windows_sidecars_resolve_and_execute`.

## RED receipt

The owning test was added before the supply implementation and run exactly as
planned:

```text
$ ruby scripts/tests/packaged-sidecars-test.rb --name packaged_macos_windows_sidecars_resolve_and_execute
missing sidecar supply-chain lock: .../scripts/ffmpeg-sidecars.lock.json
exit 1
```

The first provisioning attempt also correctly rejected the upstream tag label
as insufficient evidence: the pinned Apple Silicon asset under release
`b6.1.1` actually reports `ffmpeg version 6.0`. The final lock therefore records
the executable-reported version per target/tool in addition to URL and SHA-256.

## Code and supply-chain evidence

- `scripts/ffmpeg-sidecars.lock.json`: immutable release URLs, exact reported
  versions, and SHA-256 for macOS arm64/x64 and Windows x64.
- `scripts/provision_ffmpeg_sidecars.py`: bounded target allowlist, retrying
  download, streaming SHA-256, executable version verification, atomic publish,
  and removal of only its own named partial files.
- `src-tauri/tauri.macos.conf.json` and `src-tauri/tauri.windows.conf.json`:
  platform-specific Tauri `externalBin` entries for both tools.
- `crates/opentake-media/src/ff.rs#packaged_sidecar_beside`: accepts only a
  regular non-symlink sibling of the running executable.
- `src-tauri/src/lib.rs#resolve_media_tools`: an intact packaged pair overrides
  ambient variables; a release package with a missing tool pins the missing
  sibling path and fails closed instead of searching developer `PATH`.
- `.github/workflows/ci.yml#windows-product`: provisions the Windows target,
  runs the source smoke with an empty PATH, builds MSI/NSIS, silently installs
  NSIS, then runs the owning test against the installed directory.
- The package includes the repository GPL/NOTICE and
  `resources/ffmpeg/SOURCE.md`.

## Automated GREEN receipts (macOS arm64)

```text
$ python3 scripts/provision_ffmpeg_sidecars.py --verify-only
verified src-tauri/binaries/ffmpeg-aarch64-apple-darwin
verified src-tauri/binaries/ffprobe-aarch64-apple-darwin

$ ruby scripts/tests/packaged-sidecars-test.rb --name packaged_macos_windows_sidecars_resolve_and_execute
PASS: packaged_macos_windows_sidecars_resolve_and_execute

$ cargo test -p opentake-media packaged_sidecar
2 passed; 0 failed

$ cargo test -p opentake-tauri --test security_config
4 passed; 0 failed

$ cargo test --workspace --no-fail-fast
all workspace unit, integration, and doc-test binaries passed; only tests marked
with their existing explicit real-device fixture requirements were ignored

$ cargo clippy --workspace --all-targets -- -D warnings
exit 0

$ cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings
exit 0

$ pnpm -C web test
82 files passed; 774 tests passed
```

The owning smoke clears `PATH` for every sidecar invocation, creates a 64×36
video, verifies ffprobe metadata, decodes one exact RGBA frame, encodes a 32×18
output, and probes the output again.

## Packaged `.app` receipt

Build:

```text
$ web/node_modules/.bin/tauri build --debug --bundles app
Finished 1 bundle at target/debug/bundle/macos/OpenTake.app
```

The bundle contained `Contents/MacOS/opentake`, `ffmpeg`, and `ffprobe`, plus the
GPL/NOTICE/source resources. Before local signing, the two packaged sidecars
matched the locked source digests exactly:

```text
ffmpeg  a90e3db6a3fd35f6074b013f948b1aa45b31c6375489d39e572bea3f18336584
ffprobe bb2db6f5d8cef919da12fbf592119a987202a8c060a886f3cab091f9cab90b64
```

The app and nested sidecars were then ad-hoc signed for this local candidate.
Signing changes the Mach-O file digest, so the final installed/package gate
verifies each nested code signature, locked reported version, and the complete
media smoke rather than incorrectly requiring the pre-sign digest:

```text
$ ruby scripts/tests/packaged-sidecars-test.rb --name packaged_macos_windows_sidecars_resolve_and_execute --package target/debug/bundle/macos/OpenTake.app
PASS: packaged_macos_windows_sidecars_resolve_and_execute

$ codesign --verify --deep --strict --verbose=2 target/debug/bundle/macos/OpenTake.app
... ffmpeg validated
... ffprobe validated
OpenTake.app: valid on disk
OpenTake.app: satisfies its Designated Requirement
```

## Packaged UI/runtime receipt

The exact app binary was launched with a minimal system `PATH` and deliberately
invalid `OPENTAKE_FFMPEG` / `OPENTAKE_FFPROBE` values. The packaged sibling pair
therefore had to override both injected values for media decoding to work.

In the real Tauri WebView:

1. Home loaded with six persisted recent projects.
2. `opentake-ds-legacy-real-device.opentake` opened and rendered its imported
   PNG plus the existing audio waveform.
3. `TalkingHeadQA.opentake` opened in explicit compatibility read-only mode
   (because it contains the independently tracked `transitionOut` field), while
   its real 30-second video still produced a visible `talking-head-30s` image,
   duration, tracks, and audio waveform. There was no blank media library or
   missing-FFmpeg error despite the poisoned ambient paths.
4. The application was exited after inspection; no project edit was made.

## Remaining cross-platform receipt

The first exact-head Windows run for `21f7e9ebe1a4e16a16d1ef7931f48d7ee9e9fc62`
([run 30612593449](https://github.com/appergb/OpenTake/actions/runs/30612593449))
passed provisioning and the source-side empty-`PATH` probe/decode/encode smoke.
It did not reach installer creation because a Web documentation-owner test
hard-coded the local checkout directory name. The same run also exposed two
Windows Tauri test jobs that compiled before provisioning the new external
binaries. Both CI portability defects are now covered by regression contracts;
neither partial run is an installed-package receipt.

Record the next exact workflow URL, source SHA, installer digests, installed
path, offline WebView2 installation, and passing owning test here before
declaring Task 33 or Data Safety Task 10 complete.
