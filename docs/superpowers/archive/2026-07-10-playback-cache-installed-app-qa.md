# 2026-07-10/11 Playback, Cache, and Artifact QA

## Provenance and scope

This record restores the historical untracked 2026-07-10 installed-app QA from
the safety snapshot and adds the independently reviewed 2026-07-11 detached-
bundle evidence. It keeps three artifact identities separate. A result belongs
only to the binary and app tree named in its section; the hashes do not match.
Unless an absolute QA root is shown, `logs/...` paths below are relative to the
safety root
`/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence`.

## A. Older installed app: 2026-07-10 observations

Artifact exercised: `/Applications/OpenTake.app` on the recovery checkout
`recovery/superpowers-integration-20260708-v2`.

- Executable SHA-256:
  `c5cf2a827d718574cdbc68580d77562bdf86a99cb561052ba20143f96e1956aa`
- Installed/release app-tree manifest digest:
  `df505845b71c01b7fc2223759cb8d2ba799c2bb8aebb46945ec34918c159746f`
- Historical backups:
  `/Applications/OpenTake.app.pre-tailframe-cachefix-20260710-002300` and
  `/Applications/OpenTake.app.before-publication-gate-20260710-112652`.

### Implemented recovery behavior recorded that day

- Cold Rust playback bootstrapped a synchronous first video texture rather than
  publishing an all-black clear while stream warm-up lagged.
- `playback_frame` became a clock signal; the frontend retained the previous
  visible surface until the requested JPEG loaded.
- Import/search prewarm was widened and search click/drag joined the prewarm
  path. Timeline-end lookup clamped to the final drawable frame.
- Ordinary media used WebKit; supported compositor content stayed on Rust;
  incomplete renderer combinations were excluded rather than called verified.
- Bounded/cancellable control, publication gating, tail interruption, and
  terminal-frame parking were introduced to prevent obsolete startup or frame
  work from replacing a newer session.

### Installed-app observations

The real project
`/Volumes/mac/剪辑/课程1-2 第二节课/未命名.opentake` loaded with V2/V1/A1 media.
Its initial black frame at `00:00:00` corresponded to a timeline gap before the
first video. Playback then advanced visibly; pause retained the frame; resume
advanced without the observed black/white swap. The final publication-gate
build reached `00:04:21`, held `00:17:09` for 2.2 seconds, and resumed to
`00:20:18` after three seconds with a visible frame.

Earlier in the same pass, the app imported and played
`/Volumes/mac/剪辑/课程1-2 第二节课/第二节课-练习素材包/视频/Aroll/Tim-Macbook Neo Talking节选.mp4`.
Thumbnail, linked V1/A1 insertion, play, pause, and resume were observed.

The process sentinel observed only
`/Applications/OpenTake.app/Contents/MacOS/opentake`: no separate mpv player and
no persistent ffmpeg/ffprobe process. Historical pixel evidence reported mean
RGB `[149.78, 151.22, 156.91]`, near-black ratio `0.0385`, and visible ratio
`0.8636` for `/tmp/opentake-qa/installed-project-playback-visible.png`.

These are dated observations of the older installed artifact. They are not
proof for either detached bundle below, and they do not complete installed-app
export UI artifact verification.

## B. Fresh detached Task 6.2 bundle: 2026-07-11

Artifact and evidence:

- Commit: `dc83284319bdd7ec816a0175f1e97b90a9bc5e1a`.
- Executable SHA-256:
  `166d1ab4fab5110e5ca84c51f37da03766b54df770e45d9e049d351b489dcbc5`.
- App-tree manifest digest:
  `5b968d29f2d97dec8e6445c93d7af8d35e83d4fda18e9e91b80c6b5e678057a4`.
- QA root:
  `/Volumes/mac/OpenTake-QA/20260711-150117/wave1a-fresh-bundle-smoke`.
- Final review:
  `logs/03b-preview-retained-frames-dc83284319bd-qa-repair-review/reviewer-report.md`.

The repaired QA was approved with Critical 0 / Important 0 / Minor 0. It used
the exact detached-review executable and UI-created disposable projects:

- Plain WebKit advanced, paused, and retained the visible terminal frame at
  `00:13:24 / 00:13:27`.
- Rust UI import created real `/Volumes/mac/OpenTake.mp4` video/audio clips and
  overlapping text; the composited overlap and terminal retained frame were
  visible at `00:35:08` and `00:37:22`.
- Unsupported UI import created a real reversed imported clip plus overlapping
  text; the localized fail-closed explanation appeared, Play/Capture were
  disabled, and Space did not advance `00:40:25`.
- Source project and external input hashes matched before/after; no matching
  OpenTake/mpv/ffmpeg/ffprobe process remained after quit.

This verifies Task 6.2 route and retained-frame acceptance for this exact
bundle. It does not verify the older installed binary or the later Task 6.3
bundle.

## C. Fresh detached Task 6.3 final bundle: 2026-07-11

Artifact and evidence:

- Commit: `1f2bf4e49877c145b7c7990e2a7ad85b32685aed`.
- Executable SHA-256:
  `a3543126087884ff4dd2129d7fee14595413581724e7b58ec6261b642301fe6b`.
- App-tree manifest digest:
  `f16e68b2604b3645c317c54ec60447b992a9309670633a047b9fc7515bb691c2`.
- QA root:
  `/Volumes/mac/OpenTake-QA/20260711-172547/wave1a-task6-3-fix5-smoke`.
- Final gate/review:
  `logs/03c-web-project-media-ui-1f2bf4e49877-review-fix-5`.

The final review was approved with Critical 0 / Important 0 / Minor 0. The
exact detached bundle first opened a project where media `id-1` referenced the
316 MB Tim course source and populated poster, preview, filmstrip, and waveform
caches. It then opened projects reusing `id-1` for
`/Volumes/mac/OpenTake.mp4`. Poster and preview changed immediately; the longer
project showed the new dark UI filmstrip and its different/flat waveform, with
no old Tim frames or old waveform retained. This verifies the reviewed same-ID,
different-project/source-path cache replacement behavior for the final bundle.

It does not retroactively change the installed-app observations or Task 6.2
artifact identity.

## 2026-07-11 exact pre-document verification

Persisted evidence in `logs/04a-tests-evidence-integration` reports:

- focused Web: 5 files, 62/62 tests passed;
- full Web: 54 files, 570/570 tests passed;
- playback integration: 7/7 passed, 0 ignored;
- live transport: 6/6 passed, 0 ignored;
- workspace Rust: passed.

The workspace run still reported seven deliberately ignored tests: one
`opentake-media` extraction probe requiring ffmpeg+ffprobe and six real-device
probes (three export and three playback). They were not counted as passes. All
named playback/media assertions in the explicit playback and transport targets
executed and passed.

## Preserved incomplete-risk inventory

Later reviewed slices closed playback publication, exact transport, retained
frame, and project/source cache identity defects. The following items from the
original 2026-07-10 risk record remain open unless separately proven elsewhere:

- dirty project switching/opening save guarantees, full upstream-equivalent
  recents/home behavior, and remaining home accessibility details;
- timeline gesture parity, audio fade/keyframe details, Finder/file-URL drops,
  animated transform keyframe writing, text resize/refit, and dB/linear UI
  parity;
- reverse export audio/interchange safety, Lottie export/render support, direct
  Jianying/CapCut app-import evidence, and installed-app render/export UI
  artifact verification;
- Agent undo ownership and linked `move_clips` safety;
- automatic semantic/transcription indexing, spoken-search cache dependency,
  and transcript cache language/options identity;
- end-to-end generation UI/MCP, complete BYOK provider coverage, and Account /
  Models / Help / Feedback / Privacy surfaces;
- signed/notarized packaging, CSP and asset-scope hardening, bundled
  ffmpeg/ffprobe sidecars, search-model manifest integrity, and Windows
  transport/sidecar behavior.

## Current disposition

Verified only within the artifact boundaries above: the tested installed app
showed visible play/pause/resume without a separate mpv process; Task 6.2 proved
the exact capability route and retained-frame handoff; Task 6.3 proved final
same-ID/different-path visual-cache replacement. Unsupported capabilities are
fail-closed and must not be marketed as verified. Installed-app export UI
artifact verification is still not yet complete.
