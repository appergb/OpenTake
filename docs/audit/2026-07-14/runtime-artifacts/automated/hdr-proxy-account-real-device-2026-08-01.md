# HDR / proxy / account packaged real-device evidence — 2026-08-01

Parent: `MR-hdr-proxy-account-composite`

Environment: macOS arm64, packaged `target/release/bundle/macos/OpenTake.app`,
bundled FFmpeg/FFprobe, Chinese UI. All desktop interaction below was performed
through the packaged GUI; shell probes only generated fixtures or inspected the
resulting project/output artifacts.

## Packaged artifact under test

- Executable SHA-256: `38c3c1c2aec593d8b9fea79691cec0eb69fdad5d5eae6e193897fcc4ae65cf95`
- DMG SHA-256: `8a317e64d60ec78a67a096bff7eab5b57d56117a68c863563e5e8020eaefeae2`
- Whole `.app` and the app copied from the rebuilt DMG pass
  `codesign --verify --deep --strict`.
- Signature is ad hoc (`Signature=adhoc`, `TeamIdentifier=not set`). This is
  local verification evidence, not Developer ID/notarization evidence.

## HDR child

Fixture:

- `/private/tmp/opentake-task15-hdr-pq-20260801.mp4`
- SHA-256: `52cd239ae9f29dd9e990a914b00c01e075d66e3283d1644372aaa942f818ce11`
- HEVC Main10, 640x360, `bt2020nc / smpte2084 / bt2020`.

Packaged GUI evidence:

- Import persisted the exact source metadata in `media.json`.
- Source Inspector displayed `HDR 色彩` and
  `smpte2084 · 预览与导出映射为 BT.709 SDR`.
- Double-click insertion created a 48-frame clip at 24 fps.
- Timeline preview rendered the color test frame and playback advanced from
  `00:00:00` to `00:01:06 / 00:02:00`.
- GUI export completed as `1280×720 · 48 帧`.

The first packaged export exposed a real defect: it produced a 4,215-byte,
all-black H.264 file (SHA-256
`97a4035e2f35429609e7ba5ad69cafce6521581866cc45a86ad1b482f1eb812f`,
Y=16..16). The active developer FFmpeg exposes `scale_vt`, while the bundled
sidecar exposes `zscale`; the old implementation selected by OS instead of the
actual binary capability. A full-timeline regression test reproduced RGB
`0..=0` with the bundled sidecar. Runtime filter detection fixed the path and
the same test passed.

Final packaged GUI output:

- `/private/tmp/opentake-task15-hdr-export-fixed-20260801.mp4`
- SHA-256: `58a40abad454a5126360f9f7cd3d9a87716f6587958fb8c28ba8ae68a223b13e`
- H.264, 1280x720, 2.000 s, yuv420p.
- `color_space=bt709`, `color_transfer=bt709`,
  `color_primaries=bt709`.
- Decoded signal range Y=15..249 with non-zero saturation; the fixed delivery
  retains picture contrast and is not a black fallback.

Focused checks:

- `cargo test -p opentake-media --test hdr` — 3 passed.
- `cargo test -p opentake-media color::tests::` — 2 passed.
- Full HDR timeline export with `OPENTAKE_FFMPEG` and `OPENTAKE_FFPROBE`
  pointed at the packaged sidecars — passed after reproducing the RED failure.

HDR child result: **PASS** for the declared v1 SDR-delivery policy. This is not
an HDR-passthrough claim.

## Proxy child

Fixture and generated proxy:

- Original source:
  `/private/tmp/opentake-task15-proxy-source-20260801.mp4`
- Original SHA-256:
  `633043ad47858461c8939a3e57634c1070fbb219fa66e0021d434cd1836a8ca4`
- Source: H.264 1920x1080 plus AAC, 3.000 s.
- Project proxy after the final packaged remove/recreate probe:
  `media/proxies/89e91938-3ce0-49b8-b955-775a1871ad40.mp4`
- Proxy SHA-256:
  `88c645547be8b47f05383301cbbec3465f227c18b5b2501082e2bacb2fdc12b6`
- Proxy: H.264 1280x720 plus AAC.
- `media.json` stores only the project-relative path, the exact original-source
  digest, and 1280x720 dimensions.

Packaged GUI evidence:

- Inspector moved from `生成 720p 代理` to
  `代理媒体 已就绪 · 1280 × 720` and `移除代理`.
- Project close/reopen retained the proxy metadata and file.
- General Settings persisted `优先使用代理媒体播放` across app restart.

The first packaged playback attempt exposed a second real defect: the proxy was
outside the static asset-protocol scope, so enabling proxy playback produced
`00:00 / 00:00` while the original poster remained visible. The fix grants only
the exact regular proxy file at creation/catalog load, rejects symlinks, and
durably denies removed proxy paths. Focused scope tests pass (2/2).

Final code review exposed a third boundary before publication: a regular proxy
leaf under a symlinked `media/proxies` ancestor could still escape the project.
A regression first failed because nested proxy paths were accepted. Creation,
playback, catalog authorization, relink, and removal now share an exact
`media/proxies/<leaf>.mp4` resolver that rejects symlink/reparse ancestors and
confirms both directory parents remain inside the project. Project replacement
also cancels the single in-flight transcode while holding the identity lease.
The core nested-path and Tauri symlinked-ancestor regressions both pass.
Relink and removal now retain the project-identity workflow lease through old
proxy deletion and scope revocation. The concurrency regression starts a
project replacement during file cleanup and proves it cannot complete until
cleanup returns.

Path-switch proof:

- The generated proxy was backed up, then temporarily replaced with a compatible
  pure-magenta H.264/AAC file (SHA-256
  `4f6d6b76d31737d30c8adfd021d55154f1a85d35ebf9bd0c459d0f0981010d82`).
- With proxy preference on, packaged preview played the magenta file and
  advanced to `00:00:14 / 00:03:00`.
- Turning proxy preference off immediately restored the original moving color
  test source and advanced to `00:00:13 / 00:03:00`.
- The original generated proxy was restored afterward; its SHA-256 again equals
  `88c645...12b6`.

Original-only export proof:

- Proxy preference stayed on and packaged preview visibly showed the magenta
  proxy before export.
- The GUI exported the five-second HDR+proxy-source timeline as 1280x720,
  120 frames.
- Output:
  `/private/tmp/opentake-task15-proxy-export-original-proof-20260801.mp4`
- Output SHA-256:
  `6cc5479a3559b64da503290890668d913ebea09c366b2ac04c49e32a6264e5d0`
- Output has H.264 + AAC, duration 5.000 s, and BT.709 tags.
- At 2.5 s (inside the proxy-source clip), decoded Y=15..235 with the moving
  source pattern. A pure-magenta proxy would have a near-constant luma plane.
  Export therefore used the original source, not the enabled proxy.
- The generated proxy was restored after this probe and its original SHA-256
  was re-verified.

Latest-package repeat after the security fix:

- The process path was the rebuilt bundle under test, not the older installed
  `/Applications` copy.
- Settings showed proxy playback `on`; packaged playback inside the proxy-backed
  clip advanced from `00:03:17` to `00:04:16`.
- Inspector executed `移除代理`, returned to `生成 720p 代理`, then regenerated
  to `已就绪 · 1280 × 720`.
- The new UUID leaf probes as H.264 1280×720 plus AAC. Its SHA-256 is again
  `88c645547be8b47f05383301cbbec3465f227c18b5b2501082e2bacb2fdc12b6`,
  and `media.json` still binds it to the exact original-source digest.

Final packaged repeat after the identity-lease fix:

- The rebuilt bundle was quit and relaunched before the probe so the test used
  executable SHA-256 `38c3c1c2...cf95`, not an already-running prior inode.
- Inspector executed `移除代理`, returned to `生成 720p 代理`, then regenerated
  to `已就绪 · 1280 × 720`.
- Packaged playback advanced from `00:00:00` to `00:01:20 / 00:03:00`.
- The regenerated UUID leaf is the path recorded above, probes as H.264
  1280×720 plus AAC, has the deterministic proxy SHA-256 above, and remains
  bound to source digest `633043ad...ca4`.

Proxy child result: **PASS**.

## Account child

Packaged GUI evidence:

- Initial state: no backend address, login disabled, `未登录`.
- Saving `http://example.com` failed with
  `Remote account backends must use HTTPS`; login remained disabled.
- Saving `http://127.0.0.1:9` succeeded as the explicitly allowed local
  development exception.
- A non-secret dummy token attempted only
  `http://127.0.0.1:9/api/auth/verify` and failed closed with a controlled
  connection error because no service was listening.
- During that error state, the saved project reopened, the three timeline clips
  remained present, export stayed enabled, and local playback advanced to
  `00:00:18 / 00:05:00`.
- The test backend was cleared afterward; the UI returned to an empty address,
  disabled login button, and `未登录`. The token field was cleared after the
  failed attempt.

Focused Rust check: `cargo test -p opentake-tauri account::tests::` — 21 passed.
The full Web suite also covers AccountPane state and error rendering.

Account child result: **PASS**. Local editing remains the default and is not
gated by account availability.

## Composite conclusion

`HDR child PASS + proxy child PASS + account child PASS` closes one composite
acceptance for `MR-hdr-proxy-account-composite`, subject to the repository-wide
gates and the project-level external release blockers recorded in the handoff.
The functional Rust/Web gates pass. The completion-audit runner has 205/206
passing; its sole failure is the protected file inventory omitting previously
tracked files, so that ledger step remains explicitly open.
