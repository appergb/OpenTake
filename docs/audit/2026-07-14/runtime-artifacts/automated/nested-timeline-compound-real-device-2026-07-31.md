# Nested timeline / compound clip real-device evidence (2026-07-31)

Status: **macOS packaged acceptance complete.** This receipt closes the shared
runtime criteria for Media Render Task 2 (`MR-nested-timeline`) and Preview /
Timeline Task 2 (`compound-clips`). It does not claim Developer ID signing,
notarization, or Windows UI acceptance.

## Bound contracts

- `media-render-playback-export-implementation.md`, Task 2,
  `implementation-slice-b8f61feebde4e2ab` /
  `requirement-bedfdc6edfa147b9`.
- `preview-timeline-implementation.md`, Task 2,
  `implementation-slice-0ef5268789b6e13f` /
  `requirement-b49e0f5ed8c2415c`.
- Owning tests:
  - `crates/opentake-render/tests/nested_timeline.rs#nested_edits_preview_and_export_same_frames`
  - `crates/opentake-project/tests/compound_roundtrip.rs#compound_clip_roundtrips_nested_timeline`
  - `crates/opentake-render/tests/compound_render.rs#compound_clip_preview_export_frames_match`

## Historical RED receipt

The exact planned tests were run in an isolated detached worktree at
`8bea548`, the parent of the nested-timeline implementation commit. All three
commands exited 101 because their owning test targets did not exist:

```text
no test target named `nested_timeline`
no test target named `compound_roundtrip`
no test target named `compound_render`
historical-red-exits nested=101 roundtrip=101 render=101
```

During packaged validation, the first build also exposed a real linked-audio
paste failure:

```text
粘贴失败：TauriCommandError: entries[1]: asset type is not compatible with the destination track
```

The audio lane correctly retained `sourceClipType=video` because it decodes
from the original video asset. Destination validation incorrectly used that
source-container type instead of the placed `mediaType=audio`. The corrected
validator and regression test now preserve both meanings.

## Automated GREEN receipts

```text
$ cargo test -p opentake-render --test nested_timeline nested_edits_preview_and_export_same_frames -- --exact
1 passed; 0 failed

$ cargo test -p opentake-project --test compound_roundtrip compound_clip_roundtrips_nested_timeline -- --exact
1 passed; 0 failed

$ cargo test -p opentake-render --test compound_render compound_clip_preview_export_frames_match -- --exact
1 passed; 0 failed

$ cargo fmt --all -- --check
exit 0

$ cargo clippy --workspace --all-targets -- -D warnings
exit 0 (only the existing future-incompatibility notice for block 0.1.6)

$ cargo test --workspace --no-fail-fast
all workspace unit, integration, and doc-test binaries passed; only the seven
explicit real-device probe tests remained ignored

$ npm --prefix web test -- --run
88 files passed; 801 tests passed

$ npm --prefix web run build
exit 0 (existing bundle-size and ineffective-dynamic-import warnings only)
```

## Packaged application receipt

The release bundle was produced with the repository-locked Tauri CLI 2.11.3:

```text
$ web/node_modules/.bin/tauri build --bundles app
Finished 1 bundle at target/release/bundle/macos/OpenTake.app
```

The unsigned local bundle was fully ad-hoc signed for this machine, including
the packaged `ffmpeg` and `ffprobe` sidecars. Strict verification passed:

```text
$ codesign --verify --deep --strict --verbose=2 target/release/bundle/macos/OpenTake.app
OpenTake.app: valid on disk
OpenTake.app: satisfies its Designated Requirement
```

This local ad-hoc signature is runtime evidence only; it is not a distributable
Developer ID / notarization receipt.

## Packaged GUI and persistence receipt

The exact release `.app` was operated through the macOS accessibility surface:

1. Created `MRNestedTimeline.opentake` through the native Save panel.
2. Imported deterministic 4-second red/440 Hz and blue/660 Hz H.264/AAC media
   through the native multi-file Open panel.
3. Added both assets to the root timeline, selected the linked blue video/audio
   pair, and invoked **创建复合片段** from the timeline context menu.
4. Double-clicked the compound to enter its child timeline. The breadcrumb
   displayed **← 主时间线 / 复合片段**.
5. Trimmed the linked child pair by 10 source frames and nudged it right by 5
   frames. The persisted pair became `startFrame=15`, `durationFrames=110`,
   `trimStartFrame=10` with the original shared link group.
6. Copied the pair, moved the child playhead to its corrected local end
   (`00:04:05 / 00:04:05`), and pasted. The new video/audio pair began at frame
   125, preserved the trim and `sourceClipType=video`, and received a fresh
   shared link group.
7. Returned to the root timeline. At `00:04:10`, the paused composite showed
   the nested blue frame. Continuous packaged playback advanced to `00:06:19`
   while retaining the blue nested composite.
8. Saved, returned Home, reopened the project, re-entered the compound, and
   observed all four child clip IDs across V1/A1.

The persisted `project.json` used for the final reopen had SHA-256
`53373a35cb56f4adf3a09c3212000a4168e29c6737496623cfaacf262811b2f4`.

## Export receipt

The packaged Export dialog completed 231 frames and reported
`导出完成 · 1280×720 · 231 帧`. Packaged `ffprobe` then reported:

```text
video: h264, 1280x720, 30/1 fps
audio: aac, 48000 Hz, mono
duration: 7.700000 seconds
size: 86668 bytes
```

Tracked exact artifacts:

- `nested-timeline-compound-export-2026-07-31.mp4` — SHA-256
  `96a8a3cbb772336e389a101f98a2c77fe965a7a0459ac16d8a558474901f9434`
- `nested-timeline-export-frame-1s-2026-07-31.png` — red root frame,
  SHA-256 `d82eff581346e21a5633ae9927b982b4e3838786c49ee6429028cb321b7fbf0a`
- `nested-timeline-export-frame-5s-2026-07-31.png` — blue nested frame,
  SHA-256 `fe97190f271c2abe496a3658073c238eafe4c11e3649920be56be39aec7f9ecd`

The two decoded frames prove the exported file crosses from the root red clip
into the live nested blue sequence; the AAC stream proves linked nested audio
was included by the same flattened render plan.
