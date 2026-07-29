# Talking-head cleanup automated evidence — 2026-07-29

Scope: Task 3 sub-slice `requirement-9b922be7c8e92147`. This artifact records code-level evidence only. It does not close the requirement; real-media UI, save/reopen, and export verification remain outstanding.

## Production path

- `remove_filler_words` is a strict typed MCP/Chat tool backed by the production `MediaBridge::transcribe` path through `get_transcript`.
- Configurable single- and multi-word filler phrases are normalized and matched against word-level project-frame timestamps.
- Results are preview-only, carry stable review ids and `accepted: true`, and group selected half-open ranges into one undoable `ripple_delete_ranges` command per affected track.
- The catalog omits transcript-dependent tools when no media bridge is present; direct dispatch then returns `Tool is not advertised`.
- `tighten_silences` remains the PCM/RMS half of the same workflow and returns reviewable ripple commands without direct mutation.

## RED evidence and defect found

The initial `remove_filler_words_returns_reviewable_word_aligned_ranges` test failed with `Unknown tool: remove_filler_words` before the tool was added.

After adding a fixed 30-second linked A/V fixture, `reviewed_filler_cut_applies_once_and_undo_restores_the_timeline` failed because the audio track retained a duplicate `[6,12)` middle fragment while the video track did not. Root cause: `clear_region` searched all tracks for the right half created by a linked split and could select the partner track's fragment. The fix scopes that lookup to the original clip's track, and a lower-level ripple regression now pins the behavior.

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

## Remaining release evidence

- Exercise the combined filler + configured silence workflow through the user-facing review UI on a real media file.
- Save, close, reopen, and compare the resulting timeline.
- Export and verify duration, speech order, and A/V sync against the preview.
- Retain screenshots/logs/output hashes before promoting the audit record.
