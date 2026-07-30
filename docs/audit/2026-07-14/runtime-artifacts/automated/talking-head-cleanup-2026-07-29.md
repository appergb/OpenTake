# Talking-head cleanup automated + real-device evidence — 2026-07-29/30

Scope: Task 3 sub-slice `requirement-9b922be7c8e92147`. The automated contract and a real macOS application loop now pass. This artifact still does not close the complete requirement because the Agent chat/provider-driven review surface was not exercised with a configured model credential.

## Production path

- `remove_filler_words` is a strict typed MCP/Chat tool backed by the production `MediaBridge::transcribe` path through `get_transcript`.
- Configurable single- and multi-word filler phrases are normalized and matched against word-level project-frame timestamps.
- Results are preview-only, carry stable review ids and `accepted: true`, and group selected half-open ranges into one undoable `ripple_delete_ranges` command per affected track.
- The catalog omits transcript-dependent tools when no media bridge is present; direct dispatch then returns `Tool is not advertised`.
- `tighten_silences` remains the PCM/RMS half of the same workflow and returns reviewable ripple commands without direct mutation.

## RED evidence and defect found

The initial `remove_filler_words_returns_reviewable_word_aligned_ranges` test failed with `Unknown tool: remove_filler_words` before the tool was added.

After adding a fixed 30-second linked A/V fixture, `reviewed_filler_cut_applies_once_and_undo_restores_the_timeline` failed because the audio track retained a duplicate `[6,12)` middle fragment while the video track did not. Root cause: `clear_region` searched all tracks for the right half created by a linked split and could select the partner track's fragment. The fix scopes that lookup to the original clip's track, and a lower-level ripple regression now pins the behavior.

The first real Whisper run exposed three additional production defects that synthetic word fixtures could not reveal:

- whisper.cpp's experimental absolute token timestamps spread early words backwards through long leading silence, so filler cuts targeted silence rather than speech;
- zero-duration lexical tokens such as `You [t,t]` were discarded, preventing `you know` from being reviewed as one phrase;
- a fresh desktop process restarted the sequential core id generator at `id-1`, allowing regenerated captions to collide with ids loaded from the saved project.

PCM edge alignment now tightens only long silent segment edges and remaps token positions into the audible interval. Lexical normalization retains zero-duration words and gives them non-overlapping reviewable spans. The production desktop wiring now uses UUID ids while deterministic sequential ids remain available to tests.

The first caption regeneration after accepted cuts exposed one further defect: cached source-segment text still contained deleted fillers, so captions resurrected `Um`. Caption generation now rebuilds only cut segments from visible word runs, preserving original punctuation and split-token spelling (for example `synchron` + `ized` remains `synchronized`). A sync-locked derived caption follower also correctly refused a colliding ripple operation atomically; the validated workflow removes and regenerates derived captions around the linked A/V cleanup.

## Focused GREEN commands

```text
cargo test -p opentake-ops ripple_delete_ranges_keeps_linked_av_frame_exact
cargo test -p opentake-agent filler -- --nocapture
cargo test -p opentake-agent --test advertised_tool_acceptance every_advertised_tool_is_live_or_absent -- --exact
cargo test -p opentake-agent without_bridge
cargo test -p opentake-agent system_prompt_includes_context_signal
cargo test -p opentake-agent --test tool_argument_contract
```

All commands passed on the working tree. The fixed 30-second, 30 fps fixture proves:

- `um` maps to `[6,12)` and `you know` maps to `[15,27)`.
- Applying only `[6,12)` leaves `you know` in transcript order.
- Linked video and audio both become `[(0,6),(6,894)]`, with no drift or duplicate fragment.
- Selecting the linked video clip expands to its transcribed audio partner before matching, so a normal talking-head video selection does not silently return zero cuts.
- One `undo` restores the exact prior timeline.

## Regression gates

```text
cargo test -p opentake-ops --no-fail-fast
cargo test -p opentake-agent --no-fail-fast
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast -q
cd web && pnpm build
cd web && pnpm test
git diff --check
```

All gates passed. The affected subsystem totals include 184 `opentake-ops` unit tests plus 55 command integration tests and 332 `opentake-agent` unit tests plus its integration suites. The full workspace completed with zero failures; only repository-declared ignored tests remained ignored. Web verification passed 70 files / 703 tests and the production build. Vite emitted the pre-existing ineffective-dynamic-import and large-chunk warnings; they are warnings, not build failures.

## Real macOS application loop

Application: exact current-branch debug bundle at `target/debug/bundle/macos/OpenTake.app`.

Fixture and outputs:

- source: `/tmp/opentake-beta-qa-20260729/talking-head-30s.mp4`, SHA-256 `fd4eae86104ed966d82bd1dfbbdf3feb5be4370fdfbacaba0a426f8acbe771bb`;
- saved project: `/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake`;
- Whisper model: `ggml-base.bin` (local 141 MB model);
- export: `/tmp/opentake-beta-qa-20260729/TalkingHeadQA-cleaned.mp4`, SHA-256 `4f0724b28e1762c9a532f402f99fc2f28065b2fbe1c86a0681c880007558ffa6`.

Observed and accepted review results:

- corrected real-word spans placed `Um` at `[150,155)` and `You know` across `[357,366)`, inside the spoken audio rather than leading silence;
- default filler review proposed `Um [151,154)` and `You know [358,365)`; both were accepted;
- the linked video and audio tracks each became the same three fragments: timeline `[0,151)`, `[151,355)`, `[355,890)`, with source trims `0`, `154`, and `365` respectively;
- the operation removed exactly 10 frames across the two linked tracks and shifted four fragments; post-cut transcription contained no `Um`, `You`, or `know`;
- regenerated captions were exactly: `Hello and welcome to this open take beta test`; `today we are testing the talking head clean up workflow`; `the accepted cuts should keep`; `the audio and video perfectly synchronized`;
- all regenerated caption ids were UUIDs and did not collide with existing A/V ids.

The project was saved, the desktop process was fully terminated, the same build was restarted, and the saved project reopened at 890 frames (`00:29:20` at 30 fps) with all three linked A/V fragments and four captions intact.

The real UI export produced H.264 1920×1080 at 30 fps with 890 decoded video frames and 29.666667 s video duration; AAC audio duration was 29.666000 s (0.000667 s difference). Visual inspection of seam and caption contact sheets found no black/broken seam frames and showed the four expected captions without deleted filler text. Contact-sheet hashes are `ed5c51e95860929c3d93a5405bf2cd146d8646fd7aa68f2b1808a8b7b29d3f6d` and `97f2ef75ec54a38229e93a70ab73f1cc712d022b892e034b9656804a82c7b674`.

## Remaining requirement evidence

- Exercise the same review/apply flow through the Agent chat UI with a configured model provider; the local tool contract was invoked directly because no provider credential was available in this test environment.
- Exercise the combined filler + configured-silence review in that user-facing surface. Filler review/apply passed independently, while the existing silence workflow remains covered by automated PCM/RMS tests.
- Retain durable release-run screenshots or packaged CI artifacts rather than relying only on the local temporary contact sheets recorded above.
