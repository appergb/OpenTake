# Task 5 implementer report

Status: DONE

## Scope

- Removed Home's generation-activity state, request, region, dedicated tests, and unused front-end API wrapper; backend audit storage remains unchanged.
- Replaced the 48px project placeholder with responsive 16:9 semantic preview figures, covered real thumbnails, and a named structural fallback.
- Preserved card selection/open, keyboard Enter, loading, and context-menu flows.

## TDD evidence

### RED

`pnpm -C web test -- src/components/home/HomeView.test.tsx src/components/home/HomeView.interaction.test.tsx src/components/home/HomeView.visual.test.ts`

Exit 1: new assertions proved the old Home still called `generationLog`, retained its generation section, and rendered neither semantic 16:9 preview figures nor the named structured fallback.

### GREEN

- `pnpm -C web exec vitest run src/components/home/HomeView.test.tsx src/components/home/HomeView.interaction.test.tsx src/components/home/HomeView.visual.test.ts --reporter=verbose` — 3 files / 39 tests passed.
- `pnpm -C web test` — 144 files / 1255 tests passed.
- `pnpm -C web build` — TypeScript and Vite production build passed.
- `git diff --check` — no whitespace errors.

## Self-review

- `generationLog` had no remaining front-end consumer after this change, so its wrapper was removed while the core command/storage was untouched.
- Thumbnail rendering still requires native path validation and falls back after image failure, offline, or missing projects.
- The wrapper preserves right-click handling; native card focus, Enter-to-open, double-click, action locking, and unavailable-project guards remain covered by existing tests.

## Commit

`fix(home): simplify activity and show useful project previews`

## Review fix round 1 — truthful preview metadata

Review found that the first fallback invented a `16:9` project ratio and three
tracks even though Home had no corresponding project metadata. The fallback
now has two explicit states:

- When native Home validation can decode explicit, positive canvas dimensions
  and at most 64 valid track types from the retained, no-follow
  `project.json`, the DTO/cache/card show that real ratio and those real track
  kinds. A `1080×1920` two-track fixture verifies `9:16` plus exactly video and
  audio rows.
- When metadata is missing, malformed, unsafe, oversized, or unavailable, the
  DTO omits `preview` and the card renders only its actual project name plus a
  non-quantified decorative structure. It claims neither a ratio nor a track
  count. The card surface itself remains responsive 16:9.

The localStorage startup cache validates positive i32 dimensions, the five
domain track kinds, and the same 64-track bound before retaining an optional
preview. Successful saves and opens also refresh the cached preview from the
authoritative in-memory Rust timeline snapshot, so the card does not have to
wait for a later Home filesystem probe.

### RED

- `cargo test -p opentake-tauri filesystem_probe_reports_explicit_canvas_and_actual_track_kinds --lib` — failed because `preview.canvasWidth` was null.
- `pnpm -C web test --run src/components/home/HomeView.test.tsx src/store/recentStore.test.ts` — 3 expected failures: the fallback still claimed `16:9`/three tracks, the portrait fixture still rendered `16:9`, and valid cached metadata was discarded.

### GREEN

- `cargo test -p opentake-tauri home::tests --lib` — 16 passed.
- `pnpm -C web test --run src/components/home/HomeView.test.tsx src/components/home/HomeView.interaction.test.tsx src/store/recentStore.test.ts` — 3 files / 31 tests passed.
- `pnpm -C web test --run` — 144 files / 1264 tests passed.
- `pnpm -C web build` — TypeScript and Vite production build passed (existing dynamic-import and chunk-size warnings only).
- `cargo fmt --check` — passed.
- `cargo clippy -p opentake-tauri --lib -- -D warnings` — passed (Cargo emitted only the existing future-incompatibility notice for `block 0.1.6`).
- `git diff --check` — no whitespace errors.

### Commit

`fix(home): use truthful project preview metadata`
