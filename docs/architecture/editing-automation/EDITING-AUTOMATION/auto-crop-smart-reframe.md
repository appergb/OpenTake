# Auto Crop / Smart Reframe DOS

## Purpose

Define v1 automatic framing for timeline clips. It should produce deterministic crop/transform edits through the shared edit path, not a separate render-only effect.

Parent contract: [Editing Automation DOS](../EDITING-AUTOMATION-DOS.md). Source baseline: [Editing engine plan](../../EDITING-ENGINE-PLAN.md), [CapCut gap report](../../CAPCUT-GAP.md), [render spec](../../../modules/opentake-render/SPEC.md), [frontend UI spec](../../../modules/web/SPEC.md), [known bugs](../../BUGS.md). [PORT-1TO1-GAP.md](../../PORT-1TO1-GAP.md) is historical reference only.

## V1 Scope

Included:

- Smart reframe for aspect-ratio adaptation, for example 16:9 source to 9:16 or 1:1 timeline.
- Black-bar removal by detecting stable letterbox or pillarbox regions.
- Crop/transform output that remains inspectable and editable in the Inspector.
- Optional keyframe smoothing only when the subject window changes gradually and the result can stay clip-relative.

Excluded:

- ML face tracking.
- Multi-person identity tracking.
- Scene understanding that requires a remote model.
- Render-only dynamic crops that are invisible to `Timeline/Clip`.

## Command Contract

Recommended tool shape:

```text
smart_reframe {
  clipIds: string[],
  targetAspect?: "timeline" | "9:16" | "16:9" | "1:1" | "4:5",
  mode?: "fit" | "fill" | "remove_black_bars" | "stable_subject",
  write?: boolean
}
```

`write=false` returns proposed `SetClipProperties` or `SetKeyframes` payloads. `write=true` applies through:

`Dispatcher::dispatch()` -> short-id expansion -> typed args -> `EditCommand::SetClipProperties` or `EditCommand::SetKeyframes` -> `CoreHandle::apply()` -> `context_signal` -> short-id shortening.

UI writes use the same chain:

`TimelineContainer/Inspector/Toolbar` -> `web/src/store/editActions.ts` -> `web/src/lib/api.ts editApply()` -> `src-tauri/src/commands.rs edit_apply` -> `AppCore::apply()` -> `opentake-ops::EditCommand` -> `ops/*` -> `timeline_changed` -> `sync.ts`.

## Algorithm Sketch

1. Sample a bounded set of frames from each target clip after trim and speed mapping.
2. Detect black bars with edge luminance/variance tests. Require stability across sampled frames before modifying crop.
3. Estimate a content bounding box from non-bar pixels and motion/contrast energy. This is not face detection.
4. Convert desired visible source rectangle into `Crop` plus `Transform` using existing normalized coordinate semantics.
5. Smooth across samples only if resulting keyframes are sparse and clip-relative.
6. Apply as one atomic edit command per user action.

## Invariants

- Output must respect half-open clip intervals.
- Crop keyframes are clip-relative.
- Source trim remains source-frame trim; reframe must not change trim unless explicitly requested by another command.
- Linked audio partners must not receive visual crop/transform edits.
- The visual/audio track partition must not change.
- Undo must restore the exact prior timeline snapshot through the shared `EditCommand` transaction.

## Acceptance Hooks

See [acceptance tests](acceptance-tests.md). Minimum cases:

- 16:9 landscape clip reframed to 9:16 without changing duration or trim.
- Letterboxed clip gets black bars cropped only when bars are stable.
- Audio-only clip is rejected with no edit.
- `write=false` returns a proposal and does not emit `timeline_changed`.
