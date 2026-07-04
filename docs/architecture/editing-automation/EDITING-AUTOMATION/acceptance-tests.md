# Editing Automation Acceptance Tests

## Purpose

Define acceptance checks for the DOS docs and the future implementation they describe. These tests are contract-level; implementation workers should add concrete unit, integration, and E2E tests in their crates.

Parent contract: [Editing Automation DOS](../EDITING-AUTOMATION-DOS.md). Source baseline: [Editing engine plan](../../EDITING-ENGINE-PLAN.md), [Known bugs](../../BUGS.md), [agent spec](../../../modules/opentake-agent/SPEC.md), [core spec](../../../modules/opentake-core/SPEC.md), [media spec](../../../modules/opentake-media/SPEC.md), [render spec](../../../modules/opentake-render/SPEC.md). [PORT-1TO1-GAP.md](../../PORT-1TO1-GAP.md) is historical reference only.

## Documentation Checks

- Every DOS Markdown link resolves locally.
- The UI call chain is present exactly:
  `TimelineContainer/Inspector/Toolbar` -> `web/src/store/editActions.ts` -> `web/src/lib/api.ts editApply()` -> `src-tauri/src/commands.rs edit_apply` -> `AppCore::apply()` -> `opentake-ops::EditCommand` -> `ops/*` -> `timeline_changed` -> `sync.ts`.
- The MCP/Agent call chain is present exactly:
  `Dispatcher::dispatch()` -> short-id expansion -> typed args -> `EditCommand` -> `CoreHandle::apply()` -> `context_signal` -> short-id shortening.
- The Swift alignment chain is present exactly:
  `EditorViewModel` gesture methods -> `withTimelineSwap` -> `OverwriteEngine/RippleEngine/SnapEngine` -> `Timeline/Clip` pure value model.
- [PORT-1TO1-GAP.md](../../PORT-1TO1-GAP.md) is marked historical reference only.

## Shared Implementation Checks

- `write=false` automation tools return proposals and do not call `CoreHandle::apply()`.
- `write=true` tools call exactly one atomic `EditCommand` transaction per user action.
- Validation failure leaves the document unchanged and emits no `timeline_changed`.
- Successful writes emit `timeline_changed`, then `sync.ts` refreshes the read-only mirror.
- Short IDs are expanded before typed args and shortened after `context_signal`.
- Linked group sync is preserved for every write.
- Visual/audio track partition is preserved.

## Smart Reframe Checks

- Landscape source to vertical timeline writes only crop/transform properties.
- Stable letterbox bars are removed; unstable dark content is not treated as bars.
- Audio-only clips are rejected without mutation.
- Clip-relative crop keyframes stay within `[0, durationFrames]`.
- Undo restores the exact prior crop/transform state.

## Beat Sync Checks

- Synthetic click or pulse audio yields expected beat frames within a defined tolerance.
- Low-energy speech does not generate dense montage beats.
- Beat detection is read-only.
- `auto_cut_to_beats` preserves linked A/V sync.
- V1 implementation uses PCM energy/onset and does not add heavy ML or FFT dependencies.

## Agent / Workflow Checks

- `detect_beats`, `auto_cut_to_beats`, `smart_reframe`, and `tighten_silences` are visible in tool metadata when implemented.
- `remove_filler_words` reports unavailable until word-level transcript is wired to timeline frames.
- Active workflow plugin roles affect tool target selection.
- Plugin rules appear in `context_signal` warnings without suppressing built-in warnings.
- Agent `ripple_delete_ranges` rejects calls that pass both `trackIndex` and `clipId`, accepts `clipId + units=seconds`, and emits half-open project-frame ranges after fps/source-trim conversion.
- Agent `add_clips` with omitted `trackIndex` creates shared auto tracks and clips in one undoable transaction; one `undo` removes both clips and auto-created tracks.
- PCM-backed MCP tools return deterministic preview data: `detect_beats` returns beat frame hints, `auto_cut_to_beats` returns beat/cut/placement suggestions, and `tighten_silences` returns `ripple_delete_ranges` candidate commands without mutating the timeline. `smart_reframe` still returns a deterministic vision-backend diagnostic until sampled-frame analysis is wired.

## Minimum Local Verification

Run a local Markdown link existence check over `docs/DOS/**/*.md`. This does not prove implementation behavior, but it prevents stale cross-document references in the DOS set.
