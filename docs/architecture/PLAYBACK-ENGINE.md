# Playback engine architecture

> Status: draft · Stage: implementation-backed · Source review: 2026-09-06.
> Earlier dated QA remains historical. Current candidate evidence is in the [public Beta validation record](../audit/2026-09-06/public-beta-validation.md).

## Capability route is the sole authority

`resolveTimelinePlaybackRoute` selects exactly one route before runtime
preference or fallback is considered:

- `webkit`: ordinary video playback, including single-track reverse/speed changes where no compositor-only property requires Rust.
- `rust`: text, Lottie, color grade, chroma key, stabilization, supported masks/effects and native video stacks when the runtime supports them. Composited temporal remapping is implemented; it is no longer rejected merely because speed differs from one or the clip is reversed.
- `unsupported`: unknown effect names, more than four masks, or a required Rust path that is unavailable/disabled. A multi-video temporal stack requires Rust and fails closed if unavailable.

The exact branching, including ordinary multi-video runtime fallback behavior, is defined by [playbackRoute.ts](../../web/src/components/preview/playbackRoute.ts). Missing media/source failures are also handled by the native session, rather than silently omitting authored layers.

Unsupported capability is fail closed. It receives a localized explanation and
disabled Play/Capture/Space controls; it is not silently rendered by an engine
that would omit authored content. A compositor-only timeline whose previous
native startup failed keeps Play enabled for an explicit retry. Only a typed
Rust `engine` failure may use the WebKit runtime fallback; `busy`, `cancelled`,
and `superseded` are control states.

## WebKit route

WebKit is the normal route for a single ordinary video stack. One frontend clock drives the
mounted `<video>`/`<audio>` elements; pause/resume keeps the same decoded DOM
surface. Reverse and positive-speed media can remain on this route when no
compositor-only content is present. This route has no libmpv dependency or
transparent native-player hole.

## Source-media preview route

The source tab uses the Rust streaming engine whenever the packaged playback
capability is available. `playback_start(mediaId)` projects the selected video
into a private one-track timeline at the project fps, then reuses the same
FFmpeg RGBA decode, compositor, cpal audio clock, exact publication, and
identity-scoped pause/seek/stop lifecycle as timeline playback. The source
asset is read directly; this route does not create a whole-file proxy or
transcode by default.

Paused source frames and paused seeks use `composite_frame(sourceMediaId)` so
they also decode through FFmpeg instead of switching back to WebKit. A terminal
frame remains painted, Play rewinds it to frame zero, and selecting another
asset retires the previous source session before accepting its publications.
The ordinary `<video>` source preview remains only for the browser shell and a
minimal build where the native playback capability handshake is unavailable.

## Rust route and exact publication

Rust continuously decodes, builds the RenderPlan, composites with wgpu, and
uses cpal audio (or a wall clock when audio is unavailable). Transport to the
preview is a session-scoped finite request, not `<img src=MJPEG>`:

```text
rendered RGBA -> JPEG stage -> PublicationGate commit
  -> playback_frame {projectEpoch, timelineVersion, sessionId, frame, sequence, terminal}
  -> GET /frame with those exact fields -> 200 JPEG or 204 on mismatch
```

The encoded JPEG, exact-frame store entry, sequence, terminal flag, and Tauri
event share one publication coordinator. `PublicationGate` is closed before
timeout quarantine, stop, project/timeline invalidation, or teardown, so an
in-flight old encoder cannot publish into a replacement session. The live
transport gate verifies complete finite JPEG responses, exact identity, and
fail-closed Origin handling; `/stream` and `/ws` remain diagnostic/server
surfaces, not the production preview image contract.

## Lifecycle, control, and bootstrap

Playback identity is `{projectEpoch, timelineVersion, sessionId}`. Start takes
one atomic runtime snapshot, prepares off the async executor, and rechecks the
revision during installation. Pause, seek, stop, events, and frame requests are
accepted only for the exact identity; an exact paused revision may retain and
resume its session.

Pause is an immediate publication boundary, not a render-thread barrier. The
frontend freezes the authoritative playhead before awaiting IPC and rejects
late native events/image loads. The backend closes `PublicationGate` before
audio control, enqueues the pause without waiting for an in-flight decode, and
discards that render before publication. Resume repositions the retained engine
before reopening audible output; only the exact retained session can reopen its
gate. WebKit's decoded frame may advance a pause by a fractional frame, but it
can never rewrite the playhead backwards when decoding lags.

The cpal sample position remains the preferred master clock. Startup and resume
still require sustained callback liveness; additionally, a callback that stops
advancing during playback switches after 150 ms to a monotonic wall-clock
continuation. Recovery never rewinds the timeline, while an explicit transport
seek remains authoritative and may move backward.

Audio preparation has one persistent admitted worker and checked memory bounds;
teardown uses one persistent reaper with at most two outstanding jobs. Queue,
panic, cancellation, timeout, and project-boundary paths release capacity and
return typed control errors. Publication is closed and audio muted before reap.

The 2026-07-10 Promise-tail interrupt established that pause/stop must not wait
behind obsolete slow startup. Wave 1A superseded that helper with the current
identity-scoped controller plus backend cancellation and bounded reaping. The
current invariant is still tail interruption: control/project boundaries
invalidate old work first, and later completion cannot control or publish into
the replacement.

Cold Rust startup requests the exact nonnegative source frame with zero
tolerance. Missing media, decode failure, worker failure, and premature stream
disconnect propagate instead of publishing a black clear. Cancellation remains
attached through audio preparation and first-frame readiness, and pending-to-
running installation is atomic under the playback slot.

## Retained-frame handoff

The frontend owns two stable image slots. A matching event loads into the
inactive slot; only that slot's matching completed `currentSrc` may be promoted.
The previous visible slot remains painted during loading. Terminal publication
is stopped only after the terminal image crosses the browser paint boundary;
bounded retries retain the last good frame on failure. A matching paused
composite releases the terminal Rust frame only after the replacement is loaded.
Paused/scrub composite requests are latest-only: one request may be in flight
and only the newest pending frame is retained. That paused composite stays
painted during native startup and is removed only after the first live slot
loads. Pausing cancels the pending decoder slot while preserving the promoted
canvas, so even a pre-pause image that finishes during a fast pause/resume cycle
cannot paint afterward. Stale identity, sequence, load, cleanup, and paint
callbacks are ignored.

## Project/source identity and prewarm/cache

One project epoch spans runtime snapshots, playback, and media prewarm. The
prewarm scheduler has three persistent workers and a bounded 24-job queue;
admission results are `queued`, `duplicate`, `cached`, `busy`, or
`staleProject`. Project transition cancels old queued/running work and guarded
same-directory publication rejects stale staged bytes.

Poster, preview, waveform, and timeline filmstrip identity includes project
epoch plus media/source identity. Reusing the same media id with a different
source path cannot satisfy or block the replacement request, while edits inside
the same project retain valid caches.

## Reviewed evidence

- Task 5.1-5.6 final commits `2eff907c`, `e2daeb279a33`, `ba5b1ceac463`,
  `24ab2590ce96`, `f99da16c27b4`, and `3fe09766819b`: approved.
- Task 6.1 `8b47e64a8e6c`: approved capability resolver.
- Task 6.2 `dc83284319bd`: approved after repaired exact-bundle external QA.
- Task 6.3 `1f2bf4e49877`: approved with final same-ID/different-path UI QA.
- 2026-07-11 pre-document gate: focused Web 5 files/62 tests; full Web
  54/570; playback integration 7/7; transport 6/6; workspace Rust passed.
  Workspace still reports seven deliberate ignored probes: one ffmpeg/ffprobe
  environment probe and six real-device probes (three export, three playback).
- 2026-08-01 reconciliation: the four reviewed route/lifecycle owners passed
  against the current code; Web passed 93 files/824 tests. The final packaged
  macOS application exercised both a single-video WebKit project and a
  text/color/multi-track Rust project, then switched back across the project
  boundary without retaining the previous duration or playhead. Receipt:
  [playback-route-lifecycle-real-device-2026-08-01.md](../audit/2026-07-14/runtime-artifacts/automated/playback-route-lifecycle-real-device-2026-08-01.md).
- 2026-08-09 source-preview regression: the production source projection and a
  seeked native start at frame 1572 decoded the reported 2.5 GB HEVC Main10,
  yuv420p10le, 3840x2160, ~100 Mbps + PCM asset for three seconds. It published
  61 frames, advanced to frame 1662, kept the minimum non-black pixel ratio at
  0.945, and observed zero neon-green corruption. The probe remains ignored by
  the default workspace gate because it requires that external real-device
  fixture.

Artifact hashes and the separation between older installed-app evidence and
fresh detached bundles are recorded in
[2026-07-10 playback/cache QA](../superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md).

## Not yet complete

- Installed-app export UI artifact verification is not yet complete; unit and
  integration export evidence does not replace that acceptance test.
- Windows WebView2 transport, CSP hardening, and sidecar/bundled FFmpeg behavior
  are not verified by the macOS detached-bundle QA.
- Signing, notarization, asset-protocol scope, search-model integrity, and other
  packaging/security hardening remain open.
- Lottie, generic effects, polygon/overflow masks, and composited reverse/speed
  are deliberately unsupported until their renderer paths exist.
- ProRes/high-bitrate and A/V real-device probe coverage remains ignored in the
  workspace gate and must not be described as newly verified by that gate.
