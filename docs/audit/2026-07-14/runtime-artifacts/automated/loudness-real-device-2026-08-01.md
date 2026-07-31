# MR-loudness packaged runtime evidence — 2026-08-01

## Scope and package identity

- Exact application: `target/release/bundle/macos/OpenTake.app`
- Executable SHA-256: `43ca7cdc03657dfeb3c361595aeba0b17a5fcae625c74d7ede405bd9557095f8`
- Code signature: valid strict/deep ad-hoc signature; CDHash `38b143d0e3f43a22d778e30ec647fd9905837baf`; no TeamIdentifier. This is runtime evidence, not a notarized Beta artifact.
- Project: `/private/tmp/opentake-loudness-real-device-20260801.opentake`

## Code verification

- RED: `cargo test -p opentake-media --test loudness normalization_reaches_configured_lufs_within_tolerance -- --exact` failed before the new analysis owner existed.
- GREEN: focused media loudness, domain persistence/wire-schema, project compatibility, undo/redo, Tauri routing/playback/export and Web Inspector/API tests passed.
- `cargo test --workspace --all-targets --quiet`: PASS; existing environment-only integrations remained ignored.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo fmt --all -- --check` and `git diff --check`: PASS.
- `npm --prefix web test -- --run`: PASS, 90 files / 814 tests.
- `npm --prefix web run build`: PASS with the pre-existing large-chunk and ineffective-dynamic-import warnings only.
- `tauri build --bundles app --no-sign`: PASS; the resulting bundle was ad-hoc signed and passed strict/deep verification.

## Packaged application workflow

All UI actions below were performed against the exact release `.app` through macOS accessibility and native file/save panels.

- Imported deterministic 48 kHz mono speech and music WAV fixtures.
- Speech analysis displayed `-32.0 → -16.0 LUFS`, `+18.0 dB`, `-2.1 dBTP` after the final codec-margin fix.
- Music analysis displayed `-27.4 → -16.0 LUFS`, `+11.8 dB`, `-2.3 dBTP`.
- Analyze/apply, reanalyze, reset, undo and redo changed/restored the expected Inspector state.
- Native playback advanced the transport for normalized speech and music.
- Save and application relaunch reopened the project writable and restored the persisted speech normalization. A missing compatibility descriptor initially caused a read-only reopen; adding `loudnessNormalization` to `Clip::WIRE_FIELDS` plus the known-schema/wire-schema regression fixtures fixed it before acceptance.
- A real one-second silent WAV returned `loudness_silent_audio: no block passed the EBU R128 absolute gate` and did not create a normalization result.

## Independent export measurements

The first speech export exposed an AAC reconstruction overshoot (`-16.09 LUFS / -0.23 dBTP`) with a one-dB codec margin. The shared preview/export safety margin was raised to two dB, the package was rebuilt, and both deliverables were re-exported from the GUI.

| Fixture | Deliverable | FFmpeg `loudnorm` input I | input TP | Result |
| --- | --- | ---: | ---: | --- |
| Speech | `/private/tmp/opentake-loudness-speech-export-v2-20260801.mp4` | -16.07 LUFS | -1.15 dBTP | PASS |
| Music | `/private/tmp/opentake-loudness-music-export-20260801.mp4` | -16.02 LUFS | -1.74 dBTP | PASS |

Both files are five-second 1920×1080 H.264/AAC, 30 fps, 48 kHz mono exports. Acceptance is target `-16 LUFS ±1 LU` and true peak no hotter than `-1 dBTP`.

- Speech SHA-256: `39985024152dd103db6ba02579dc16379d9dcb28855b12ae75588903d9017fd8`
- Music SHA-256: `6f87d13f5f56377a8bb8cb78e169a12248c3056f656b4cf5d6b6194eb6f7493c`

## Verdict

MR-loudness is PASS for code, packaged macOS runtime, persistence, error handling, native preview and exported-deliverable measurements. This verdict covers Task 11 only and does not remove the project-wide Beta release blockers.
