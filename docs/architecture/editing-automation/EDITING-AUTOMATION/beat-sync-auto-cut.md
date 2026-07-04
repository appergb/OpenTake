# Beat Sync / Auto Cut DOS

## Purpose

Define v1 music beat detection and beat-aligned cutting. The first version must be cheap, local, and deterministic.

Parent contract: [Editing Automation DOS](../EDITING-AUTOMATION-DOS.md). Source baseline: [Agent context signal](../../../modules/opentake-agent/AGENT-CONTEXT-SIGNAL.md), [CapCut gap report](../../CAPCUT-GAP.md), [media spec](../../../modules/opentake-media/SPEC.md), [agent spec](../../../modules/opentake-agent/SPEC.md), [known bugs](../../BUGS.md). [PORT-1TO1-GAP.md](../../PORT-1TO1-GAP.md) is historical reference only.

## V1 Scope

Included:

- `detect_beats`: PCM energy and onset candidate detection.
- `auto_cut_to_beats`: align a selected set of clips or source ranges to detected beat frames.
- Local media decoding only.
- No timeline mutation unless `auto_cut_to_beats` is called with `write=true`.

Excluded:

- Heavy ML beat tracking.
- New FFT dependency or large DSP stack.
- Tempo maps requiring full musical structure analysis.
- Cloud audio analysis.

## Detection Contract

`detect_beats` reads a target audio clip, linked audio partner, or selected BGM track and returns:

```text
{
  fps,
  trackIndex,
  source: "clip" | "track",
  beats: [{ frame, strength, kind: "onset" | "energy_peak" }],
  confidence,
  warnings
}
```

V1 algorithm:

1. Decode audio to PCM using the existing media layer.
2. Downmix to mono and use fixed windows, for example 20-40 ms.
3. Compute RMS energy per window.
4. Smooth the envelope.
5. Detect positive energy deltas and local peaks with a refractory window.
6. Convert source time to project frames using timeline fps.

No FFT is required for v1. If a future version adds spectral flux, it must be documented as v2 with dependency review.

## Auto Cut Contract

`auto_cut_to_beats` consumes beat frames and selected visual material. It may:

- split clips on beat frames;
- place selected media ranges at beat-aligned starts;
- trim clip boundaries to nearest beat when within a small tolerance;
- return a proposal when confidence is low.

It must apply edits only through the shared path:

`TimelineContainer/Inspector/Toolbar` -> `web/src/store/editActions.ts` -> `web/src/lib/api.ts editApply()` -> `src-tauri/src/commands.rs edit_apply` -> `AppCore::apply()` -> `opentake-ops::EditCommand` -> `ops/*` -> `timeline_changed` -> `sync.ts`.

Agent path:

`Dispatcher::dispatch()` -> short-id expansion -> typed args -> `EditCommand` -> `CoreHandle::apply()` -> `context_signal` -> short-id shortening.

## Safety Rules

- Never cut audio voice tracks as if they were BGM unless the workflow plugin marks them as BGM.
- Linked A/V must remain synchronized.
- Beat alignment should prefer moving/placing visual clips; mutating BGM is out of scope for v1.
- When `syncLocked` tracks cannot absorb ripple shifts, the whole edit is refused.
- The Agent should receive `context_signal` warnings when a montage workflow is not active but the user requests aggressive beat cutting.

## Acceptance Hooks

See [acceptance tests](acceptance-tests.md). Minimum cases:

- Synthetic click track produces beat frames within tolerance.
- Low-energy speech track is not over-detected as montage beats.
- `auto_cut_to_beats(write=false)` emits no `timeline_changed`.
- Linked visual/audio pairs stay in the same `linkGroupId` alignment after auto cut.
