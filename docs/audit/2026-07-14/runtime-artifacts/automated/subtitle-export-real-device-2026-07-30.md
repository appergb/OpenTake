# Subtitle SRT/VTT export automated + real-device evidence — 2026-07-30

Scope: requirement `requirement-7043d6a44938bd27` / implementation slice `implementation-slice-3b2b0548dab34607`, plus the owning title-bar controls `control-c035467e6746e570` and `control-f54f4037ab7bffbe`.

## Production path

- `TitleBar.tsx#onExportSubtitles` exposes SRT and VTT from the editor title bar, closes the format menu, opens the native save panel with the correct extension filter, appends a missing extension, and reports done/empty/failure through the visible toast state.
- `web/src/lib/api.ts#exportSubtitles` invokes the typed `export_subtitles` Tauri command with the selected path and format.
- `src-tauri/src/commands.rs#export_subtitles` serializes the current caption clips through the domain SRT/VTT writers and returns the written cue count.

## Automated evidence

Focused tests:

```text
cargo test -p opentake-tauri exports_non_empty_srt_with_cue_count
cargo test -p opentake-tauri exports_vtt_with_header
pnpm -C web exec vitest run src/components/shell/TitleBar.visual.test.ts src/components/shell/TitleBar.interaction.test.tsx
```

The Rust tests each passed with one matching test. The two title-bar files passed with the exact planned SRT/VTT routing test and owning DOM controls for menu dismissal, native save-panel arguments, extension completion, typed API arguments, success, zero-cue empty state, write failure, user cancellation, and default-directory fallback.

Regression gates:

```text
pnpm -C web test
pnpm -C web build
cargo fmt --check
git diff --check
```

The final web regression passed 70 files / 709 tests; the focused owning filter passed all four matching subtitle-export cases. The production build completed. The full Rust workspace passed with zero failures and only the repository-declared ignored hardware probes. Workspace clippy passed with warnings denied. Vite emitted only the existing ineffective-dynamic-import and large-chunk warnings. Formatting and diff checks passed.

## Real macOS application loop

Application: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-generation/target/debug/bundle/macos/OpenTake.app`.

Project: `/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake`, reopened from the saved talking-head cleanup fixture with four regenerated caption clips.

Using the visible title-bar `导出字幕` popup and the native macOS save panel:

1. `字幕 · SubRip (.srt)` exported `/tmp/opentake-beta-qa-20260729/TalkingHeadQA-cleaned.srt` and the application reported `字幕已导出 · 4 条`.
2. `字幕 · WebVTT (.vtt)` exported `/tmp/opentake-beta-qa-20260729/TalkingHeadQA-cleaned.vtt` and the application reported `字幕已导出 · 4 条`.

Both files are non-empty (307 bytes) and contain exactly these four cues in timeline order:

1. `Hello and welcome to this open take beta test`
2. `today we are testing the talking head clean up workflow`
3. `the accepted cuts should keep`
4. `the audio and video perfectly synchronized`

Format verification:

- SRT contains numbered cues and comma millisecond timestamps, beginning `00:00:00,000 --> 00:00:02,866`.
- VTT begins with `WEBVTT`, omits SRT numbering, and uses dot millisecond timestamps, beginning `00:00:00.000 --> 00:00:02.866`.
- Cue 4 ends at `00:00:16.400`, within the 890-frame / 29.666-second edited timeline.

SHA-256:

- SRT: `8b68f2abff929d100b3b2c290aa3e4768825458a785eb99b465e2fbe786afe6b`
- VTT: `fffe46a61b6a42e59dccea2484782b8feb9a09e6ea25ce35e03582017e8d4fe0`

## Result

The planned caption-export requirement is verified through code, owning automated tests, the Tauri command boundary, the real OpenTake UI, the native save panel, and the generated file contents. This closes only the SRT/VTT export slice; adjacent caption authoring, styling, and provider-driven workflows remain independently tracked.
