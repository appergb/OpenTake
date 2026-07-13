# Task 5C: `fix/91-media-library-rewrite`

- Inspected: `task-5-brief.md`, branch register, `b9e4954`, current owned media files, and the required branch evidence commands.
- Branch decision: reject direct merge (`154/1`; `128 files changed, 633 insertions(+), 10282 deletions(-)`), selectively replay only still-useful deltas.
- Replayed: audio-card waveform thumbnails in `web/src/components/media/MediaPanel.tsx`; `refreshMedia()` dedup-by-`item.id` in `web/src/store/mediaStore.ts`.
- Kept as already integrated: localStorage favorites drain now targets current manifest-backed `toggle_favorite` flow (`favorites.ts` + `item.favorite` UI).
- Rejected/deferred: stale global-library star routing, plus `ai` subtab / `MediaSubTabId = "ai"` / `media.subtab.ai`, because current IA already moved to `import|mine|extract|sound` with a disabled Generate affordance.
- Tests: `pnpm -C web test -- src/components/media/favorites.test.ts src/store/mediaStore.test.ts src/store/uiStore.test.ts`; `pnpm -C web exec tsc -b --pretty false`; `git diff --check -- <touched files>` — all passed.
- Files changed: `web/src/components/media/MediaPanel.tsx`, `web/src/store/mediaStore.ts`, `docs/superpowers/archive/2026-07-08-branch-integration-register.md`.
- Replay commit SHA: `11eb57c`

## Review Fixes

- Root cause: replayed audio cards routed all thumbnail-less audio assets through `AudioWaveform`, but the component returned `null` for null/empty waveform buckets, so failed/empty waveform loads could leave the thumbnail area blank; the replay also lacked regression tests for duplicate-id collapse and waveform fallback.
- Fix: `AudioWaveform` now owns the fallback path and renders the caller-supplied type icon until valid waveform buckets exist; it also accepts a narrow test-only `bucketsOverride` so node-environment tests can statically verify waveform vs fallback rendering without adding new test dependencies.
- Tests: added `mediaStore` duplicate-id regression coverage asserting deterministic last-item wins for duplicate ids; added `MediaPanel` waveform tests covering successful bar rendering plus null/empty fallback; reran `pnpm -C web test -- src/components/media/MediaPanel.test.tsx src/components/media/favorites.test.ts src/store/mediaStore.test.ts src/store/uiStore.test.ts`, `pnpm -C web exec tsc -b --pretty false`, and `git diff --check -- <touched files>`.
- Files changed: `web/src/components/media/MediaPanel.tsx`, `web/src/components/media/MediaPanel.test.tsx`, `web/src/store/mediaStore.test.ts`.
- Fix commit SHA: `88eb1db`
