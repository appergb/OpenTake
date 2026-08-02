# MR-denoise real-device evidence — 2026-08-01

## Scope and acceptance

- Plan task: `Task 12: MR-denoise (implementation-slice-3d159672cfd1fc67)`.
- Contract: persist mode/strength without modifying the source; expose preview, apply/reset, cancellation and undo/redo; use the same processing owner in preview and export; improve a deterministic speech-plus-noise fixture by at least 3 dB without clipping.
- Runtime target: rebuilt release bundle at `target/release/bundle/macos/OpenTake.app`.

## TDD and code verification

- Initial RED: the three reviewed owning tests could not compile because the shared denoise processing owner and domain contract did not exist.
- Regression RED: after the first packaged export exposed an AAC peak near 0 dBFS, `deterministic_noise_fixture_and_bypass` gained a no-new-peak assertion and failed with `input=0.388362`, `output=1.000000`.
- GREEN: unpadded STFT boundary samples now crossfade to the immutable dry input, and processed samples are bounded by the input peak. Focused media, playback-owner and export-owner tests passed.
- `CARGO_INCREMENTAL=0 cargo test --workspace --no-fail-fast --quiet`: PASS.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.
- `pnpm -C web test`: PASS, 91 files / 818 tests.
- `pnpm -C web build`: PASS; only the pre-existing chunk-size and ineffective-dynamic-import warnings were reported.

The first full gate attempt stopped because the regenerable dev target cache filled the disk. `cargo clean --profile dev` recovered 16 GiB; the complete gate was then rerun from the beginning and passed.

## Package identity

- Executable SHA-256: `d9bac8eb031345b02239e9ddb63588fd780eebba70652134ee1e98947e5506a1`.
- Bundle identifier: `com.opentake.desktop`.
- CDHash: `cffb5e7d81e609ce5d1092ee28af718fc8f70988`.
- `codesign --verify --deep --strict --verbose=2`: PASS for the app and bundled `ffmpeg`/`ffprobe` sidecars.
- Signature: ad hoc (`TeamIdentifier=not set`). This proves package integrity for local runtime validation, not Developer ID distribution readiness or Beta publication eligibility.

## Deterministic fixtures

- Clean reference: `/private/tmp/opentake-denoise-clean-20260801.wav`, SHA-256 `3b0fd01951c94ceea1536c9a8d5e6af53b716fba1c5260e5f03ad10ef88c33b4`.
- Noisy input: `/private/tmp/opentake-denoise-noisy-20260801.wav`, SHA-256 `a6011c2dd045e7222cd91c3d4b64916c51f4d2210a685334f71070a0a14084ed`.
- Cancellation input: `/private/tmp/opentake-denoise-noisy-long-20260801.wav`, 300 seconds, SHA-256 `5b766d758f3b1edfb9dacf36c2f3019c029829a03d22931d3a3d704f5469fda5`.
- All inputs are mono 48 kHz PCM float. The five-second fixture combines a speech-envelope sine signal with deterministic white noise.

## Packaged desktop workflow

The exact release bundle was launched through the macOS accessibility-driven desktop path. In project `/private/tmp/opentake-denoise-real-device-20260801.opentake`:

1. Imported the deterministic noisy fixture and added it to A1.
2. Applied adaptive mode at 90%; toggled preview off/on; confirmed native playback advanced; verified undo/redo restored the preview state.
3. Saved, quit, relaunched the exact bundle, opened the recent project, and confirmed adaptive/90% persisted in a writable project.
4. Reset denoise and undid reset, restoring the prior configuration.
5. Added the 300-second fixture, observed asynchronous progress at 56%, cancelled, and confirmed the clip remained unapplied with no error or stale denoise configuration; then removed the long timeline clip.
6. In the rebuilt fixed package, applied voice mode at 84%, confirmed the result label, and undid it back to adaptive/90%.
7. Exported while preview was deliberately disabled, proving preview bypass does not bypass export processing.

## Independent output measurements

- Final export: `/private/tmp/opentake-denoise-real-device-v2-20260801.mp4`.
- SHA-256: `2ab36d02bbf2cbb4a1af74df901741b75d7d9ea9bcd9f749e89c02f033701d07`.
- `ffprobe`: H.264 1920×1080 at 30 fps; mono AAC at 48 kHz; duration 5.000 seconds; size 62,215 bytes.
- FFmpeg `asdr`, clean reference versus noisy input: `10.414 dB`.
- FFmpeg `asdr`, clean reference versus exported AAC: `16.2872 dB`.
- Improvement: `+5.8732 dB`, exceeding the required `+3 dB`.
- FFmpeg `astats` exported peak: `-8.645324 dBFS`; no clipping and no boundary peak regression.

## Result

PASS for Task 12's code and packaged macOS runtime acceptance contract. This evidence closes only MR-denoise; it does not remove repository-wide Beta blockers or authorize publication.
