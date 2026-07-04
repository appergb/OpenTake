# Agent Editing Suggestions DOS

## Purpose

Define how the Agent proposes or applies editing automation without moving frame math into the LLM.

Parent contract: [Editing Automation DOS](../EDITING-AUTOMATION-DOS.md). Source baseline: [Agent context signal](../../../modules/opentake-agent/AGENT-CONTEXT-SIGNAL.md), [Workflow plugin system](../../../modules/opentake-agent/WORKFLOW-PLUGIN-SYSTEM.md), [agent spec](../../../modules/opentake-agent/SPEC.md), [core spec](../../../modules/opentake-core/SPEC.md), [known bugs](../../BUGS.md). [PORT-1TO1-GAP.md](../../PORT-1TO1-GAP.md) is historical reference only.

## Dispatcher Contract

All Agent tools follow:

`Dispatcher::dispatch()` -> short-id expansion -> typed args -> `EditCommand` -> `CoreHandle::apply()` -> `context_signal` -> short-id shortening.

The Agent sees short IDs. The dispatcher expands them before typed args and shortens newly created IDs after `context_signal` attachment.

## Tool Set

V1 automation tools:

- `detect_beats`: read-only, returns beat/onset candidates.
- `auto_cut_to_beats`: proposal or write mode, applies beat-aligned edit commands.
- `smart_reframe`: proposal or write mode, applies crop/transform commands.
- `tighten_silences`: detects low-energy PCM ranges and maps them to `RippleDeleteRanges`.

Deferred:

- `remove_filler_words`: depends on word-level `get_transcript` being truly wired through timeline frames. Until then, it must report unavailable rather than guessing from captions or segments.

## Suggestion Shape

Read-only suggestions should be structured:

```text
{
  tool,
  confidence,
  proposedCommands,
  affectedClipIds,
  frameRanges,
  warnings,
  requiresTranscript?: boolean
}
```

`proposedCommands` must be valid `EditCommand` mirrors. The LLM can choose among proposals, but it should not hand-calculate clip-relative keyframes, source trims, or ripple shifts.

## Context Signal

Attach `context_signal` after every tool run. For automation, it should include:

- inferred video type and workflow, for example `montage_beat` or `audio_driven`;
- track roles, especially `BGM`, `VoiceOver`, `MainCamera`, and `B_RollOverlay`;
- warnings such as "do not cut within a word" or "BGM beat detection was low confidence";
- plugin-derived rules when a workflow is active.

Workflow plugin rules are additive. Built-in signal rules still apply.

## Ripple Range Contract

`ripple_delete_ranges` must pass exactly one of `trackIndex` or `clipId`. `trackIndex` mode takes project-frame ranges only. `clipId` mode may use `units="frames"` or `units="seconds"`; seconds are converted to source frames with timeline fps and then mapped through clip trim/speed into project-frame half-open ranges.

## Current Tool Availability

The analysis-driven tool names are intentionally visible in MCP. `detect_beats`, `auto_cut_to_beats`, and `tighten_silences` validate args and use PCM analysis through the `CoreHandle` boundary to return preview data or candidate edit commands. `smart_reframe` validates args but still returns a vision-backend diagnostic until sampled-frame/saliency access is available.

## Acceptance Hooks

See [acceptance tests](acceptance-tests.md). Required checks:

- ambiguous short ID fails before command execution;
- `write=false` never calls `CoreHandle::apply()`;
- successful writes return shortened IDs;
- `context_signal` survives both success and no-op proposal paths;
- `remove_filler_words` is unavailable until transcript is wired.
