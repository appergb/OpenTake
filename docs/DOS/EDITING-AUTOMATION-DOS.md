# Editing Automation DOS

This document is the shared technical contract for automation features that edit an OpenTake timeline. It is deliberately narrow: reuse the current command path, keep the editing engine authoritative, and put automation-specific analysis outside `opentake-ops`.

## Current Baseline

Use these documents as current baseline: [Editing engine plan](../EDITING-ENGINE-PLAN.md), [CapCut gap report](../CAPCUT-GAP.md), [Agent context signal](../AGENT-CONTEXT-SIGNAL.md), [Workflow plugin system](../WORKFLOW-PLUGIN-SYSTEM.md), [Module port map](../MODULE-PORT-MAP.md), [Known bugs](../BUGS.md), and specs: [agent](../specs/agent-SPEC.md), [core](../specs/core-SPEC.md), [frontend UI](../specs/frontend-UI-1to1-SPEC.md), [media](../specs/media-SPEC.md), [render](../specs/render-SPEC.md), [gen](../specs/gen-SPEC.md).

[PORT-1TO1-GAP.md](../PORT-1TO1-GAP.md) is historical reference only, not current fact.

## Design Rule

Automation may analyze media, propose edits, and build commands. It must not bypass the edit transaction path or mutate timeline mirrors directly.

Authoritative UI chain:

`TimelineContainer/Inspector/Toolbar` -> `web/src/store/editActions.ts` -> `web/src/lib/api.ts editApply()` -> `src-tauri/src/commands.rs edit_apply` -> `AppCore::apply()` -> `opentake-ops::EditCommand` -> `ops/*` -> `timeline_changed` -> `sync.ts`.

Authoritative MCP/Agent chain:

`Dispatcher::dispatch()` -> short-id expansion -> typed args -> `EditCommand` -> `CoreHandle::apply()` -> `context_signal` -> short-id shortening.

Swift alignment chain:

`EditorViewModel` gesture methods -> `withTimelineSwap` -> `OverwriteEngine/RippleEngine/SnapEngine` -> `Timeline/Clip` pure value model.

## Core Invariants

- Intervals are half-open. Clip occupancy is `[startFrame, endFrame)`, where `endFrame = startFrame + durationFrames`.
- Keyframe storage is clip-relative. Incoming Agent/UI frames that are timeline absolute must be converted at the command boundary.
- `trimStartFrame` and `trimEndFrame` are source-frame trims. They are not timeline coordinates.
- Speed math consumes source frames as `round(durationFrames * speed)`. Any derived v1 automation must avoid inventing alternate frame math.
- Linked group sync is preserved for trim, move, split, delete, and ripple unless the command is `Unlink`.
- Track partition is structural: visual tracks are `[0, firstAudioIndex)`, audio tracks are `[firstAudioIndex, trackCount)`.
- Every edit is one atomic `EditCommand` transaction. If analysis cannot produce a valid command, return a suggestion or error, not a partial edit.

## Automation Surfaces

The v1 editing automation set is:

- `smart_reframe`: compute crop/transform changes for aspect adaptation, black-bar removal, and stable subject framing.
- `detect_beats`: read audio PCM and return beat/onset candidates without changing the timeline.
- `auto_cut_to_beats`: align selected clips or media ranges to beat candidates through existing edit commands.
- `tighten_silences`: find low-energy gaps and produce ripple delete ranges.
- `remove_filler_words`: disabled until timeline transcript tooling is truly wired; it depends on word-level transcript frames.

## Scope Boundaries

Automatic crop v1 covers smart reframe, black-bar removal, and aspect-ratio adaptation. It does not include ML face tracking.

Automatic music beat sync v1 uses PCM energy/onset detection. It must not add heavy ML or FFT dependencies. If later work needs spectral methods, add them as an explicit v2 design with a dependency and performance budget.

Agent tools may suggest edits without applying them. A write path must apply via `EditCommand` only.

Current MCP status: `detect_beats`, `auto_cut_to_beats`, and `tighten_silences` are typed tools backed by `CoreHandle::extract_analysis_pcm`, so they can produce PCM-based frame hints and candidate edit commands without mutating the timeline. `smart_reframe` is still a typed preflight surface that returns a vision-backend diagnostic until sampled-frame/saliency access is exposed. `remove_filler_words` remains disabled until transcript access is truly wired.

## Failure Semantics

- No media decode: return a structured diagnostic and no edit.
- Ambiguous short IDs: fail before typed args or command creation.
- Analysis confidence below threshold: return suggestions, not writes.
- Transcript unavailable: `remove_filler_words` remains unavailable; `tighten_silences` can still use PCM energy.
- `ripple_delete_ranges` accepts exactly one of `trackIndex` or `clipId`. `units="frames"` is the default; `units="seconds"` is valid only with `clipId` and is converted through the timeline fps plus the clip's source-frame trim/speed mapping before producing half-open project-frame ranges.
- `add_clips` with omitted `trackIndex` must route through one atomic auto-track `EditCommand`; track creation and clip placement must undo together.
- `swapMedia` consumes only `clipId` + `mediaRef`. Frontend types and wrappers must not expose duration/type/trim options unless the backend starts consuming them.
