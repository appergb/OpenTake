# Workflow Plugin Recipes DOS

## Purpose

Define reusable workflow recipes that bind automation tools to editing intent. Recipes are Agent-level orchestration; they do not modify Rust core editing algorithms.

Parent contract: [Editing Automation DOS](../EDITING-AUTOMATION-DOS.md). Source baseline: [Workflow plugin system](../../WORKFLOW-PLUGIN-SYSTEM.md), [Agent context signal](../../AGENT-CONTEXT-SIGNAL.md), [agent spec](../../specs/agent-SPEC.md), [module port map](../../MODULE-PORT-MAP.md). [PORT-1TO1-GAP.md](../../PORT-1TO1-GAP.md) is historical reference only.

## Recipe Format

A workflow recipe lives in `plugin.json` and optional `instructions.md`.

Required fields for automation:

- `video_type.primary`
- `workflow.approach`
- `workflow.stages`
- `workflow.rules.do`
- `workflow.rules.dont`
- `track_roles`

Plugin instructions may guide the Agent, but write operations still use:

`Dispatcher::dispatch()` -> short-id expansion -> typed args -> `EditCommand` -> `CoreHandle::apply()` -> `context_signal` -> short-id shortening.

## Built-In Recipes

### Talking Head Cleanup

Approach: `audio_driven`.

Stages:

1. `get_transcript` when available.
2. `tighten_silences` on the `VoiceOver` track.
3. `remove_filler_words` only after transcript is truly wired.
4. `smart_reframe` for vertical repurposing if target aspect differs.

Rules:

- Do not cut inside a word.
- Do not remove all breathing room; preserve configurable padding.
- Keep linked audio/video synchronized.

### Montage Beat Cut

Approach: `montage_beat`.

Stages:

1. Mark BGM track role.
2. `detect_beats` on BGM.
3. Select visual source ranges.
4. `auto_cut_to_beats(write=false)` for preview.
5. Apply `auto_cut_to_beats(write=true)` only after the proposal is coherent.

Rules:

- Prefer visual cuts on beats; do not ripple the BGM track in v1.
- Avoid using low-confidence beats for hard cuts.
- Keep shot durations above the configured minimum.

### Vertical Repurpose

Approach: `audio_driven` or `montage_beat`, depending on source.

Stages:

1. Set target aspect.
2. `smart_reframe(write=false)` for every selected visual clip.
3. Apply accepted crop/transform edits.
4. Re-run Preview/Inspector checks.

Rules:

- No ML face tracking in v1.
- Reject audio-only clips.
- Keep output edits visible as Inspector crop/transform properties.

### Silence Tighten

Approach: `audio_driven`.

Stages:

1. Identify `VoiceOver` or main linked audio track.
2. `tighten_silences(write=false)` using PCM energy.
3. Apply as `RippleDeleteRanges` project-frame ranges after review.

Rules:

- Use `trackIndex` mode for project-frame ranges. Use `clipId + units=seconds` only when a workflow is expressing source-relative clip ranges.
- Preserve linked group synchronization.
- Refuse the whole edit if sync-locked tracks cannot shift safely.

## Plugin Signal Integration

When active, plugin declarations override automatic `video_type` and `track_roles`, then append stage guidance and rules to `context_signal`. Plugin content does not replace built-in safety warnings.

## Acceptance Hooks

See [acceptance tests](acceptance-tests.md). Required checks:

- plugin track roles influence `detect_beats` target selection;
- `workflow.rules.dont` warnings appear in `context_signal`;
- recipes can run in proposal mode without emitting `timeline_changed`;
- all writes still route through `EditCommand`.
