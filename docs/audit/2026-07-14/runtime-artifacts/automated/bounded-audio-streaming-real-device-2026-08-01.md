# Bounded audio streaming packaged real-device evidence — 2026-08-01

Parent: `MR-bounded-audio-streaming`

Environment: macOS arm64, packaged release application, bundled FFmpeg 6.0,
Chinese UI. Desktop actions were performed through the packaged GUI. Shell
probes only generated fixtures, inspected processes, signatures, and completed
media artifacts.

## Packaged artifact and process identity

- Application executable SHA-256:
  `a92c3e31d503300b87806ecd36aec48ba3128c49f202c9c78167e301b6f75711`.
- DMG SHA-256:
  `040d44146f224c0aedc45cafe0cf333bae5fa10eaad3ae8c820e1f2b0fc99e57`.
- The whole `.app`, signed DMG, and the `.app` mounted from that DMG pass
  strict/deep code-signature verification. The mounted executable has the same
  SHA-256 as the source bundle.
- The signature is ad hoc (`Signature=adhoc`, `TeamIdentifier=not set`), so this
  is local package-integrity evidence, not Developer ID/notarization evidence.
- An already-running pre-build process was detected and explicitly quit. The
  final probes below ran only after PID `10645` was launched from the rebuilt
  executable above.

## Fixture

- Source:
  `/private/tmp/opentake-task16-bounded-audio-60s-20260801.mp4`.
- SHA-256:
  `98c2f4eb83acf1a18628b008434bf129f961bef090b6ba711aaacdc23c827daa`.
- H.264 640×360 at 30 fps plus AAC 48 kHz mono, exactly 60.000 seconds.
- Project:
  `/private/tmp/opentake-task16-bounded-audio-real-device-20260801.opentake`.

## Packaged playback

The rebuilt application opened the 60-second project at `00:00:00 / 01:00:00`.

- Playback advanced across the two-second scheduling boundary to
  `00:03:28 / 01:00:00`, then settled at `00:04:06` after pause. A second
  observation 2.2 seconds later still reported `00:04:06`.
- Jump-to-end reported `01:00:00`; jump-to-start reported `00:00:00`.
- Resume after that seek advanced to `00:03:17`, proving the new generation was
  decoded and consumed after the stale queue was discarded.
- The preview continued rendering changing test-pattern frames while its audio
  device clock advanced; no frozen playhead or visible black fallback occurred.

## Packaged full export

The final rebuilt process exported the entire 60-second timeline through the
GUI at the 720p preset:

- Output:
  `/private/tmp/opentake-task16-final-binary-export-20260801.mp4`.
- SHA-256:
  `13fc06a151f07b85cdfe30ad2d497d4893a1afe94e247a70d73e90fe8bb5d381`.
- Size: 12,670,639 bytes; duration: exactly 60.000 seconds.
- Video: H.264, 1280×720.
- Audio: AAC, 48 kHz, mono.

During export, the encoder owned a private `audio.pcm` spool and received only
bounded mix windows. Final AAC mux succeeded after all 1,800 video frames were
rendered; the public output remained zero bytes until atomic completion.

## Packaged WAV save-as-media

An 8-second audio clip backed by the same source was selected in the final
packaged timeline and `另存为媒体` was invoked from its context menu. The media
panel changed from two to three entries. After normal project save,
`media.json` persisted the new result as a project-relative audio source.

- Output:
  `/private/tmp/opentake-task16-audio-save-real-device-20260801.opentake/media/clip_22d2704d-929f-41b4-a46e-2cc1ef9f7ec2_18c780113066a660_0.wav`.
- SHA-256:
  `4444ced67f9b66ac37e1e4e7d537e75f12ac4b38f40322c2ed3d1e190e8378ee`.
- PCM s16le, 48 kHz, mono, exactly 8.000 seconds and 768,044 bytes including
  the WAV header.

This path writes each bounded window directly after a pre-sized WAV header; it
does not collect the full mix in a production `Vec`.

## Automated ownership and regression gates

- RED was recorded when
  `long_timeline_mix_has_constant_peak_allocation_and_matches_short_reference`
  could not compile because `mix_stereo_windows` did not exist.
- Reviewed focused tests:
  `large_mix_observes_cancellation_between_chunks` and
  `long_timeline_mix_has_constant_peak_allocation_and_matches_short_reference`
  both pass.
- Added ownership covers stale seek generations, underrun-as-silence, pause
  retaining the next valid chunk, decode cancellation, and encoder PCM spool
  growth without a retained timeline mix.
- Export integration `export_with_audio_clip_mux_aac_stream` passes with a real
  AAC output. The real save-as-media Rust test also writes and imports WAV.
- `cargo fmt --all -- --check` passes.
- `cargo test --workspace --no-fail-fast` passes. Environment-dependent tests
  remain explicitly ignored; no executed test failed.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- Web: 93 files / 824 tests pass; TypeScript and production Vite build pass.

Cancellation was verified at the owned decode/mix/encoder boundaries by the
focused and workspace tests. A separate attempt to drive the packaged cancel
button during the saturated full-resolution GPU export could not obtain a
responsive accessibility window before that export completed, so it is not
claimed as packaged-GUI cancellation evidence.

## Result

`MR-bounded-audio-streaming` is **PASS** for its declared implementation slice:
long playback, full video export, and WAV save-as-media no longer retain the
whole timeline PCM mix; seek, pause/resume, underrun, cancellation, and teardown
have explicit owned tests, and the successful package probes cover the actual
release application.
