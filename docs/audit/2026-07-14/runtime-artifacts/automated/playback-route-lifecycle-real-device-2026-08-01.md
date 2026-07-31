# Playback route/lifecycle packaged real-device evidence — 2026-08-01

Parent: `MR-playback-route-lifecycle-complete`

Environment: macOS arm64, final packaged release application, Chinese UI. The
application executable SHA-256 was
`a92c3e31d503300b87806ecd36aec48ba3128c49f202c9c78167e301b6f75711`;
the DMG SHA-256 was
`040d44146f224c0aedc45cafe0cf333bae5fa10eaad3ae8c820e1f2b0fc99e57`.
The whole app, signed DMG, and mounted-DMG app passed strict/deep signature
verification. The signature is ad hoc, so this is package-integrity evidence,
not Developer ID/notarization evidence.

## Reviewed code owners

The current baseline already contained the reviewed Wave 1A implementation, so
the focused baseline was GREEN rather than an artificially recreated RED.

- `playbackRoute.test.ts#routes plain forward video to WebKit` passed.
- `nativePlaybackSession.test.ts#publishes only increasing matching frame sequences`
  passed.
- `rustFrameBuffer.test.ts#keeps two stable Rust frame image slots mounted`
  passed.
- `playback::resolver::tests::drain_propagates_stream_failure_instead_of_freezing_cache`
  passed.
- The focused Web commands execute the current Vitest configuration's full
  suite; each run passed 93 files / 824 tests.
- Task16's immediately preceding full Rust workspace gate, strict Clippy,
  formatting, Web suite, and production build passed, and no product source
  changed between that gate and this reconciliation.

## Packaged WebKit route

Project:
`/private/tmp/opentake-task16-bounded-audio-real-device-20260801.opentake`.
Its project document contains one visible ordinary video track and no
compositor-only property, which deterministically selects the WebKit route.

- The project opened at `00:00:00 / 01:00:00` after a different project had
  been active.
- Playback advanced to `00:02:23 / 01:00:00` and paused normally.
- The correct 60-second duration and zero start after the project boundary show
  that the preceding Rust session's playhead/publication did not leak into the
  replacement project.

## Packaged Rust route

Project: `/private/tmp/opentake-hsl-ui-real-device-20260731.opentake`. Its
persisted timeline has visible text, a color-graded video, multiple visible
video tracks, and an audio track. `resolveTimelinePlaybackRoute` therefore
selects Rust rather than WebKit.

- Playback advanced from `00:00:00 / 00:30:20` to
  `00:03:07 / 00:30:20`.
- Pause settled at `00:03:16`; a second observation 1.8 seconds later remained
  `00:03:16`.
- The composited project remained visible while the playhead advanced; no stale
  terminal image, black fallback, or previous-project duration was exposed.

## Result

`MR-playback-route-lifecycle-complete` is **PASS** as a reconciliation slice.
The authoritative route, exact session identity, monotonic publication,
retained two-slot handoff, decode failure propagation, and project-boundary
reset have direct owning tests; the final package smoke covers both production
routes and a cross-project lifecycle boundary.
