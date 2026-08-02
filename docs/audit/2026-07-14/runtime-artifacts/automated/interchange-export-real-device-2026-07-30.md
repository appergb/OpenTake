# XMEML/FCPXML/OTIO/EDL export automated + real-device evidence — 2026-07-30

Scope: control `control-0d98e5e5a0c417ed` / implementation slice `implementation-slice-a85196307f399399`.

## Production path

- `web/src/components/shell/TitleBar.tsx#onExportInterchange` closes the title-bar menu, opens the native save panel, preserves the project-derived directory and filename, appends the selected extension when it is missing, routes to the selected format command, and reports success/failure through the visible toast.
- The interchange dialog intentionally omits the native extension filter. On macOS 26.5.2, the `tauri-plugin-dialog` 2.7.1 / `rfd` 0.16 `allowedFileTypes` path disabled the native Save button for these interchange formats. The application still enforces `.xml`, `.fcpxml`, `.otio`, or `.edl` through `withExt()` after confirmation.
- `web/src/lib/api.ts` invokes the typed Tauri command for XMEML, modern FCPXML, OTIO, or EDL; `src-tauri/src/commands.rs` passes the current timeline and media manifest to the matching `opentake-project` writer.
- `crates/opentake-project/src/edl.rs#top_video_clips` selects the first track containing actual video/image media. Text/Lottie overlay tracks are excluded so captions cannot shadow the editorial video track in CMX3600 output.

## Defects found by real-device verification

The first real EDL export, `/tmp/opentake-beta-qa-20260729/TalkingHeadQA-interchange.edl`, was 608 bytes with SHA-256 `120550f84260121e85b8bcf4bb3567ad29bc29387e543d28ee20c44ac575836c`. It contained four caption events named `Offline` and no source-video events. This proved that the previous `is_visual()` selection treated the top caption overlay as the single CMX3600 video track.

The rebuilt application also exposed the macOS 26 native-filter regression: with an interchange extension filter present, the Save button remained disabled. Removing only that filter while keeping the project-derived default path restored the button and retained extension safety in application code.

Corrections:

- Filter EDL candidate clips to `ClipType::Video | ClipType::Image` and skip overlay-only visual tracks.
- Add `caption_overlay_track_does_not_shadow_video_track` as a Rust regression.
- Add the exact planned title-bar control test for all four formats, success, failure, cancellation, default-directory fallback, extension completion, menu closure, and the macOS-compatible dialog options.

## Automated evidence

Focused checks:

```text
cargo test -p opentake-project edl::tests --lib
pnpm -C web test -- --run web/src/components/shell/TitleBar.interaction.test.tsx
```

The EDL module passed 14/14 tests. The web command exercised the repository suite and passed 70 files / 721 tests, including six exact `control-0d98e5e5a0c417ed export XMEML/FCPXML/OTIO/EDL` cases.

Regression gates:

```text
cargo test --workspace --no-fail-fast -q
cargo clippy --workspace --all-targets -- -D warnings
pnpm -C web test
pnpm -C web build
cargo fmt --all -- --check
git diff --check
```

All executed checks passed. The Rust workspace retained only its declared ignored hardware probes. Vite emitted only the existing ineffective-dynamic-import and large-chunk warnings.

## Real macOS application loop

Environment:

- macOS 26.5.2 (25F84), arm64.
- Application: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-generation/target/debug/bundle/macos/OpenTake.app`, rebuilt from the working tree and ad-hoc signed with all bundle resources sealed before launch.
- Project: `/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake`, 890 frames at 30 fps, with caption overlay tracks above three video/audio edit fragments.

Using the visible title-bar `导出` menu and native macOS save panel, the rebuilt application exported all four formats and displayed `已导出` after each write:

| Format | Output | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| XMEML | `/tmp/opentake-beta-qa-20260729/TalkingHeadQA-final-xmeml.xml` | 11,846 | `392b3f4e6024a66fa595449c63748ce8ad8bc5ff1c6a474a6e8b2b15b1d19c87` |
| FCPXML | `/tmp/opentake-beta-qa-20260729/TalkingHeadQA-final-fcpxml.fcpxml` | 2,750 | `751c357ecaa1003721d13801f47226456f51fab5b21b7371c74ca8363b933de4` |
| OTIO | `/tmp/opentake-beta-qa-20260729/TalkingHeadQA-final-otio.otio` | 15,628 | `972582868654e94db85000aa785abab3454e9bdde14d352803db4d22fe0afebc` |
| EDL | `/tmp/opentake-beta-qa-20260729/TalkingHeadQA-final-edl.edl` | 533 | `a7f9b82d3503dacd4817878ea8eaed69bae7a707cfc963972221ad617198a8c7` |

Validation:

- `xmllint --noout` parsed both XML outputs.
- `jq` parsed OTIO and found four tracks with child counts `[2,5,3,3]`, matching two overlay tracks plus three video and three audio edits with explicit gaps.
- The fixed EDL contains exactly three events, all named `talking-head-30s`, with no `Offline` entry. Its record ranges are `00:00:00;00–00:00:05;01`, `00:00:05;01–00:00:11;25`, and `00:00:11;25–00:00:29;20`; source trims are preserved in the source timecode columns.
- A second fixed EDL export was byte-identical, confirming deterministic output.

## Result

The planned interchange control is verified across the owning UI test, Tauri command boundary, format writers, a discovered-and-fixed EDL overlay regression, the macOS native save panel, visible success feedback, and parsed generated artifacts. This closes only implementation slice `implementation-slice-a85196307f399399`; adjacent plan slices remain independently tracked.
