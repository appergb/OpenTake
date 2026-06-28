# OpenTake Editing DOS

DOS means Design Operating Spec: a compact contract for workers implementing independent editing automation. This set covers the user plan's Key Changes 1 and 2:

1. Define the independent editing automation surface without rewriting the existing editing engine.
2. Define Agent/workflow recipes and acceptance gates for automation tools.

## Documents

- [Editing Automation DOS](EDITING-AUTOMATION-DOS.md) - shared contracts, call chains, invariants, and scope.
- [Auto Crop / Smart Reframe](EDITING-AUTOMATION/auto-crop-smart-reframe.md) - v1 framing automation.
- [Beat Sync / Auto Cut](EDITING-AUTOMATION/beat-sync-auto-cut.md) - v1 PCM energy and onset based cutting.
- [Agent Editing Suggestions](EDITING-AUTOMATION/agent-editing-suggestions.md) - tool contracts and context-signal behavior.
- [Workflow Plugin Recipes](EDITING-AUTOMATION/workflow-plugin-recipes.md) - reusable workflow plugin patterns.
- [Acceptance Tests](EDITING-AUTOMATION/acceptance-tests.md) - verification matrix for this DOS.

## Source Baseline

Current facts should be taken from:

- [Editing engine plan](../EDITING-ENGINE-PLAN.md)
- [CapCut gap report](../CAPCUT-GAP.md)
- [Agent context signal](../../modules/opentake-agent/AGENT-CONTEXT-SIGNAL.md)
- [Workflow plugin system](../../modules/opentake-agent/WORKFLOW-PLUGIN-SYSTEM.md)
- [Module port map](../MODULE-PORT-MAP.md)
- [Known bugs](../BUGS.md)
- Specs: [agent](../../modules/opentake-agent/SPEC.md), [core](../../modules/opentake-core/SPEC.md), [frontend UI](../../modules/web/SPEC.md), [media](../../modules/opentake-media/SPEC.md), [render](../../modules/opentake-render/SPEC.md), [gen](../../modules/opentake-gen/SPEC.md)

[PORT-1TO1-GAP.md](../PORT-1TO1-GAP.md) is historical reference only. Do not treat it as current implementation truth unless a newer document points back to a specific item.

## Authoritative Call Chains

UI editing:

`TimelineContainer/Inspector/Toolbar` -> `web/src/store/editActions.ts` -> `web/src/lib/api.ts editApply()` -> `src-tauri/src/commands.rs edit_apply` -> `AppCore::apply()` -> `opentake-ops::EditCommand` -> `ops/*` -> `timeline_changed` -> `sync.ts`.

MCP/Agent editing:

`Dispatcher::dispatch()` -> short-id expansion -> typed args -> `EditCommand` -> `CoreHandle::apply()` -> `context_signal` -> short-id shortening.

Swift alignment:

`EditorViewModel` gesture methods -> `withTimelineSwap` -> `OverwriteEngine/RippleEngine/SnapEngine` -> `Timeline/Clip` pure value model.

## Non-Negotiable Invariants

- Frame intervals are half-open: `[startFrame, startFrame + durationFrames)`.
- Keyframes are stored clip-relative; public APIs may use absolute timeline frames.
- Trim values are source-frame offsets, not timeline-frame positions.
- Linked audio/video groups must remain synchronized unless a command explicitly unlinks.
- Visual tracks live above audio tracks; insertion and drop routing preserve the partition.
- `EditCommand` application is atomic: validation failure or ripple refusal leaves the document unchanged.
