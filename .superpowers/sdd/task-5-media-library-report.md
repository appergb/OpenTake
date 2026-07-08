# Task 5C: `fix/91-media-library-rewrite`

- Inspected: `task-5-brief.md`, branch register, `b9e4954`, current owned media files, and the required branch evidence commands.
- Branch decision: reject direct merge (`154/1`; `128 files changed, 633 insertions(+), 10282 deletions(-)`), selectively replay only still-useful deltas.
- Replayed: audio-card waveform thumbnails in `web/src/components/media/MediaPanel.tsx`; `refreshMedia()` dedup-by-`item.id` in `web/src/store/mediaStore.ts`.
- Kept as already integrated: localStorage favorites drain now targets current manifest-backed `toggle_favorite` flow (`favorites.ts` + `item.favorite` UI).
- Rejected/deferred: stale global-library star routing, plus `ai` subtab / `MediaSubTabId = "ai"` / `media.subtab.ai`, because current IA already moved to `import|mine|extract|sound` with a disabled Generate affordance.
- Tests: `pnpm -C web test -- src/components/media/favorites.test.ts src/store/mediaStore.test.ts src/store/uiStore.test.ts`; `pnpm -C web exec tsc -b --pretty false`; `git diff --check -- <touched files>` — all passed.
- Files changed: `web/src/components/media/MediaPanel.tsx`, `web/src/store/mediaStore.ts`, `docs/superpowers/archive/2026-07-08-branch-integration-register.md`.
- Replay commit SHA: `11eb57c`
