# Branch Integration Register

## Policy

"Merge all branches" means integrate all still-relevant active branch work onto
`recovery/superpowers-integration-20260708-v2` without regressing current
`origin/main`.

Direct stale branch merges are rejected when the branch would delete current
main-line work. Selective replay is the integration method.

## Current Base

| Ref | SHA | Meaning |
|---|---|---|
| `origin/main` | `ac50dc8` | Canonical integration base |
| `recovery/superpowers-integration-20260708-v2` | `942518e` | Recovery branch with approved Superpowers design |
| `backup/before-rollback-20260708-163646` | `9eceadb` | Evidence source for reverse clip and prior planning |

## Queue

| Source | Status | Evidence | Action |
|---|---|---|---|
| `opentake-pr9` dirty worktree | Integrated | reverse-clip contract/render/web tests passed across Tasks 2-4 | Ported useful diff; excluded `.claude` deletions |
| `backup/before-rollback-20260708-163646` | Integrated | commits `0b72a10`, `1cfee93`, `9eceadb` checked against recovery branch in Tasks 2-4 | Ported remaining relevant fixes; docs restored |
| `fix/text-raster-alignment` | Integrated | commit `89bf38c`, `154/1` drift; direct stale merge rejected because branch head is 154 behind and inspection already showed broad stale-branch direct merge risk against current main-line work | Selective replay only: ported `text_engine.rs` text-style/shadow alignment delta plus focused `gpu_text.rs` coverage, then verified with current text tests and full `gpu_text` binary run |
| `test/render-pixel-diff` | Integrated | commit `eb6e429`, `154/1` drift; direct stale merge rejected because branch head is 154 behind and inspection already showed stale-branch direct merge risk against current main-line work | Selective replay only: restored `crates/opentake-render/tests/pixel_diff.rs` harness and verified with focused pixel-diff tests plus full `pixel_diff` binary run |
| `fix/91-media-library-rewrite` | Integrated | commit `b9e4954`, `154/1` drift; direct stale merge rejected because the branch diff still spans 128 files with broad docs/spec deletions and unrelated agent/core/render/tauri/preview churn | Selective replay only: restored missing audio waveform cards and `mediaStore` id-dedup, kept the current manifest-backed favorite migration, deferred stale `ai` subtab replay |
| `feat/save-clip-as-media` | Integrated | commit `708fd44`, `154/1` drift; direct stale merge rejected because branch head is 154 behind and queue inspection already established stale-branch broad-deletion risk against current main-line work | Selective replay only: ported range-aware `export_range`, `ExportFormat`/`audioWav`, PCM slicing + WAV writing, and the current explicit frontend export args without touching stale branch head wholesale |
| `feat/freeze-frame` | Integrated | commit `da3e934`, `154/1` drift; direct stale merge rejected because branch head is 154 behind and queue inspection already established stale-branch broad-deletion risk against current main-line work | Selective replay only: ported the dedicated freeze-frame command, Tauri capture-before-edit hook, request/action/menu wiring, and i18n without merging the stale head |
| `feat/account-scaffold` | Deferred | one old commit `4986716`, 154 behind / 1 ahead | Defer until editing recovery is settled; then inspect and replay account scaffold work |
| `feat/agent-chat-panel` | Integrated | commit `dd9f224`, `154/1` drift; direct stale merge rejected because the branch diff still spans 135 files with broad docs/spec deletions plus unrelated core/media/preview churn | Selective replay only: ported chat loop/session/LLM modules, Tauri chat commands with the current media bridge + explicit provider handling, and the web chat panel/store/types/api/i18n surface |
| `feat/generative-ui` | No-op | branch head equals `origin/main` | No functional delta at discovery |
| `feat/inspector-ai-edit-tab` | No-op | branch head equals `origin/main` | No functional delta at discovery |
| `feat/proxy-media` | No-op | branch head equals `origin/main` | No functional delta at discovery |

## Evidence Commands

- `git status --short --branch`
- `git worktree list --porcelain`
- `git rev-list --left-right --count origin/main...<branch>`
- `git log --oneline --no-merges origin/main..<branch>`
- `git diff --name-status origin/main..<branch>`
- `git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/opentake-pr9 diff --stat`
- `git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/opentake-pr9 diff --name-status`

## Task 5 Batch A

### fix/text-raster-alignment

- Direct merge rejection evidence:
  - `git rev-list --left-right --count origin/main...fix/text-raster-alignment` -> `154 1`
  - `git show --stat --summary 89bf38c81016b656d5b0c5ef911b9ee7e7962432` -> touched only `crates/opentake-render/src/gpu/text_engine.rs` and `crates/opentake-render/tests/gpu_text.rs`, but branch inspection for this queue item already established stale-branch direct merge risk at `154/1` drift.
  - `git show --name-only --format=medium 89bf38c81016b656d5b0c5ef911b9ee7e7962432` -> `crates/opentake-render/src/gpu/text_engine.rs`, `crates/opentake-render/tests/gpu_text.rs`
- Selective replay result:
  - Replayed still-relevant `text_engine.rs` delta: expanded font weight/style inference, upstream-aligned shadow blur radius, 3-pass blur helper, and explanatory invariants for transform/geometry-flipped behavior.
  - Replayed still-relevant `gpu_text.rs` delta: focused non-GPU raster assertions for font scaling, shadow footprint growth, alignment centroid ordering, wrapping span, deterministic raster output, and natural-size shadow padding.
- Verification:
  - `cargo test -p opentake-render text_raster -- --nocapture`
    - matched only existing `gpu::text_raster::tests::null_rasterizer_returns_none_without_panicking`; `tests/gpu_text.rs` current test names do not contain `text_raster`
    - result: `1 passed; 0 failed`
  - `cargo test -p opentake-render --test gpu_text -- --nocapture`
    - result: `7 passed; 0 failed`
  - `cargo test -p opentake-render font_size_scales_with_canvas_height -- --nocapture`
    - result: `test font_size_scales_with_canvas_height ... ok`
  - `cargo test -p opentake-render shadow_paints_pixels_outside_glyph_footprint -- --nocapture`
    - result: `test shadow_paints_pixels_outside_glyph_footprint ... ok`
  - `cargo test -p opentake-render alignment_shifts_glyph_x_centroid -- --nocapture`
    - result: `test alignment_shifts_glyph_x_centroid ... ok`

### test/render-pixel-diff

- Direct merge rejection evidence:
  - `git rev-list --left-right --count origin/main...test/render-pixel-diff` -> `154 1`
  - `git show --stat --summary eb6e4294f6f3397a336b15bf0c67ad8007a62f0f` -> created only `crates/opentake-render/tests/pixel_diff.rs`, but branch inspection for this queue item already established stale-branch direct merge risk at `154/1` drift.
  - `git show --name-only --format=medium eb6e4294f6f3397a336b15bf0c67ad8007a62f0f` -> `crates/opentake-render/tests/pixel_diff.rs`
- Selective replay result:
  - Replayed the missing `crates/opentake-render/tests/pixel_diff.rs` harness into the current tree because the current checkout had no corresponding file or equivalent coverage.
- Verification:
  - `cargo test -p opentake-render pixel -- --nocapture`
    - matched existing `plan::affine::tests::full_canvas_quad_maps_source_pixels_to_canvas_pixels`; current `tests/pixel_diff.rs` names do not contain the literal `pixel`
    - result: `1 passed; 0 failed`
  - `cargo test -p opentake-render --test pixel_diff -- --nocapture`
    - result: `14 passed; 0 failed`
  - `cargo test -p opentake-render quadrant_round_trip_psnr_is_high -- --nocapture`
    - result: `test quadrant_round_trip_psnr_is_high ... ok`
  - `cargo test -p opentake-render half_opacity_two_track_blend_matches_hand_computed -- --nocapture`
    - result: `test half_opacity_two_track_blend_matches_hand_computed ... ok`
  - `cargo test -p opentake-render ssim_identical_frames_score_near_one -- --nocapture`
    - result: `test ssim_identical_frames_score_near_one ... ok`

## Task 5 Batch B

### feat/save-clip-as-media

- Direct merge rejection evidence:
  - `git rev-list --left-right --count origin/main...feat/save-clip-as-media` -> `154 1`
  - `git log --oneline --no-merges origin/main..feat/save-clip-as-media` -> `708fd44 feat(export): save clip/range as media (#48 tail)`
  - `git diff --name-status origin/main..feat/save-clip-as-media` -> broad stale-branch diff including `.cargo/config.toml` deletion, many `docs/specs/*` deletions, and wide unrelated `M` changes across Rust/web surfaces; direct stale merge remains rejected and `.claude`-style deletions stay excluded
  - `git diff --stat origin/main..feat/save-clip-as-media` -> `127 files changed, 790 insertions(+), 10274 deletions(-)`
  - `git show --stat --summary 708fd443f1d2c8796b71acb7bc0ad1042dabd88c` -> touched `src-tauri/src/export.rs`, `src-tauri/src/lib.rs`, `web/src/components/timeline/ClipContextMenu.tsx`, `web/src/i18n/dict.ts`, `web/src/lib/api.ts`, `web/src/lib/types.ts`
  - `git show --name-only --format=medium 708fd443f1d2c8796b71acb7bc0ad1042dabd88c` -> same six files; branch remains `154/1` stale, so direct head merge stays rejected in favor of replaying only the missing delta on top of current main-line code.
- Selective replay result:
  - Kept the current `save_clip_as_media` command as the clip-context Save as Media path so the existing single-clip semantics remain: render exactly one clip, bake clip-local trim/speed/effects/text, write under project `media/`, and import it durably.
  - Replayed the missing future range-export backend into `src-tauri/src/export.rs`: `ExportFormat`, optional frame-range export plumbing, audio PCM slicing, WAV writing, and `export_range`.
  - Registered `export_range` in `src-tauri/src/lib.rs`.
  - Restored the clip-context UI/API path in `web/src/lib/api.ts`, `web/src/store/editActions.ts`, and `web/src/components/timeline/ClipContextMenu.tsx` to call `save_clip_as_media` again; the new range arguments remain backend-only for future range UX.
  - Added focused web coverage in `web/src/store/editActions.saveClipAsMedia.test.ts` so the clip-context action keeps the single-clip command signature.
- Verification:
  - `cargo test -p opentake-tauri save_clip --lib -- --nocapture`
    - result: `running 2 tests` -> `save_clip_export_format_parses_audio_wav ... ok`, `save_clip_slice_pcm_cuts_requested_frame_window ... ok`
  - `pnpm -C web test -- src/store/editActions.saveClipAsMedia.test.ts src/components/timeline/ClipContextMenu.test.tsx`
    - result: `46 passed (46 test files), 515 passed (515 tests)`; includes the new single-clip Save as Media call-shape check and the context-menu coverage
  - `pnpm -C web exec tsc -b --pretty false`
    - result: passed with no diagnostics after the updated API/type signature
  - `git diff --check -- src-tauri/src/export.rs src-tauri/src/lib.rs web/src/lib/api.ts web/src/store/editActions.ts web/src/store/editActions.saveClipAsMedia.test.ts web/src/components/timeline/ClipContextMenu.tsx`
    - result: clean

### feat/freeze-frame

- Direct merge rejection evidence:
  - `git rev-list --left-right --count origin/main...feat/freeze-frame` -> `154 1`
  - `git log --oneline --no-merges origin/main..feat/freeze-frame` -> `da3e934 feat(ops): freeze frame composite command (split + insert image clip)`
  - `git diff --name-status origin/main..feat/freeze-frame` -> broad stale-branch diff including `.cargo/config.toml` deletion, many `docs/specs/*` deletions, and wide unrelated `M` changes across Rust/web surfaces; direct stale merge remains rejected and `.claude`-style deletions stay excluded
  - `git diff --stat origin/main..feat/freeze-frame` -> `127 files changed, 770 insertions(+), 10242 deletions(-)`
  - `git show --stat --summary da3e934feb6a4cb4a55f8251172839f67fd23ca8` -> touched `crates/opentake-ops/src/command.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/render.rs`, `web/src/components/timeline/ClipContextMenu.tsx`, `web/src/i18n/dict.ts`, `web/src/lib/api.ts`, `web/src/lib/types.ts`, `web/src/store/editActions.ts`
  - `git show --name-only --format=medium da3e934feb6a4cb4a55f8251172839f67fd23ca8` -> same eight files; branch remains `154/1` stale, so direct head merge stays rejected in favor of replaying the missing command chain on top of current code.
- Selective replay result:
  - Replayed `EditCommand::FreezeFrame` into `crates/opentake-ops/src/command.rs` with focused validation and undo/ripple coverage.
  - Tightened the Tauri-side capture-before-edit hook in `src-tauri/src/commands.rs` so freeze requests are preflight-validated before any capture/import side effect; transaction-time validation in ops remains in place.
  - Refactored `src-tauri/src/render.rs` so freeze capture renders a temporary one-track snapshot containing only the target clip, preserving that clip's own source-frame/transform/crop/speed behavior without baking other tracks or overlays into the PNG.
  - Replayed the front-end request/action/menu/i18n path in `web/src/lib/types.ts`, `web/src/store/editActions.ts`, `web/src/components/timeline/ClipContextMenu.tsx`, `web/src/components/timeline/ClipContextMenu.test.tsx`, and `web/src/i18n/dict.ts`.
- Verification:
  - `cargo test -p opentake-ops freeze_frame -- --nocapture`
    - result: `10 passed; 0 failed`
  - `cargo test -p opentake-tauri freeze_frame_preflight --lib -- --nocapture`
    - result: `test commands::edit_request_serde_tests::freeze_frame_preflight_rejects_bad_requests_before_capture ... ok`
  - `cargo test -p opentake-tauri freeze_capture_snapshot --lib -- --nocapture`
    - result: `test render::tests::freeze_capture_snapshot_isolates_target_clip_and_media ... ok`
  - `cargo test -p opentake-tauri deserializes_freeze_frame --lib -- --nocapture`
    - result: `test commands::edit_request_serde_tests::deserializes_freeze_frame ... ok`
  - `pnpm -C web test -- src/components/timeline/ClipContextMenu.test.tsx`
    - result: `45 passed (45 test files), 514 passed (514 tests)`; the focused file passed after the freeze item was exposed for image/video clips
  - `pnpm -C web exec tsc -b --pretty false`
    - result: passed with no diagnostics
  - `git diff --check -- crates/opentake-ops/src/command.rs src-tauri/src/commands.rs src-tauri/src/render.rs web/src/components/timeline/ClipContextMenu.tsx web/src/components/timeline/ClipContextMenu.test.tsx web/src/i18n/dict.ts web/src/lib/types.ts web/src/store/editActions.ts`
    - result: clean

### fix/91-media-library-rewrite

- Direct merge rejection evidence:
  - `git rev-list --left-right --count origin/main...fix/91-media-library-rewrite` -> `154 1`
  - `git log --oneline --no-merges origin/main..fix/91-media-library-rewrite` -> `b9e4954 fix(media): finish media library rewrite + migrate star to backend (#91, #49, #58)`
  - `git diff --name-status origin/main..fix/91-media-library-rewrite` -> broad stale-branch diff including `.cargo/config.toml` deletion, `docs/port-map/*` + `docs/specs/*` deletions, and unrelated `M` changes across agent/core/render/tauri/preview surfaces; direct head merge remains rejected
  - `git diff --stat origin/main..fix/91-media-library-rewrite` -> `128 files changed, 633 insertions(+), 10282 deletions(-)`
  - `git show --stat --summary b9e4954` -> branch intent was limited to `web/src/components/media/MediaPanel.tsx`, `web/src/components/media/MediaTabBar.tsx`, `web/src/components/media/favorites.ts`, `web/src/components/media/favorites.test.ts`, `web/src/i18n/dict.ts`, `web/src/store/mediaStore.ts`, `web/src/store/uiStore.ts`
  - `git show --name-only --format=medium b9e4954` -> same seven owned web files; replay was evaluated file-by-file instead of merging the stale head
- Selective replay decision:
  - Replayed the still-missing audio waveform card path in `web/src/components/media/MediaPanel.tsx`: current tree still exposes `getWaveform()` in `web/src/lib/api.ts`, but the card fallback had regressed to a plain type icon for audio assets.
  - Replayed the still-missing `mediaStore` dedup guard in `web/src/store/mediaStore.ts`: current `refreshMedia()` had regressed to `store.setItems(list.items)`, so duplicate ids from overlapping refresh snapshots were no longer collapsed.
  - Kept the current favorite migration / star toggle path as already integrated: `MediaPanel` already runs `migrateLocalFavorites(items)` on load, `favorites.ts` drains legacy `opentake.favorites` ids via `toggle_favorite`, and the panel filters on `item.favorite`; the stale branch's global-library `library_favorite` / `library_unfavorite` routing was rejected.
  - Deferred the stale `ai` subtab placeholder (`MediaSubTabId = "ai"` and `media.subtab.ai`) because the current IA now uses `MediaSubTabId = "import" | "mine" | "extract" | "sound"` plus `AUDIO_SUB_TABS` and a disabled toolbar Generate affordance; replaying the old subtab would overwrite newer audio-tab structure.
- Verification:
  - `pnpm -C web test -- src/components/media/favorites.test.ts src/store/mediaStore.test.ts src/store/uiStore.test.ts`
    - result: passed
  - `pnpm -C web exec tsc -b --pretty false`
    - result: passed with no diagnostics
  - `git diff --check -- web/src/components/media/MediaPanel.tsx web/src/store/mediaStore.ts web/src/components/media/favorites.ts web/src/components/media/favorites.test.ts web/src/components/media/MediaTabBar.tsx web/src/store/uiStore.ts web/src/i18n/dict.ts docs/superpowers/archive/2026-07-08-branch-integration-register.md .superpowers/sdd/task-5-media-library-report.md`
    - result: clean

## Task 5 Batch C

### feat/agent-chat-panel

- Direct merge rejection evidence:
  - `git rev-list --left-right --count origin/main...feat/agent-chat-panel` -> `154 1`
  - `git log --oneline --no-merges origin/main..feat/agent-chat-panel` -> `dd9f224 feat(agent): chat panel with streaming + tool dispatch (#HANDOFF-3.3)`
  - `git diff --name-status origin/main..feat/agent-chat-panel` -> branch head deletes `.cargo/config.toml`, many `docs/port-map/*`, `docs/specs/*`, `src-tauri/src/mpv_bootstrap.rs`, `src-tauri/tests/*`, and unrelated web/media/preview files while also modifying broad core/ops/media/render surfaces; direct stale merge rejected.
  - `git diff --stat origin/main..feat/agent-chat-panel` -> `135 files changed, 2669 insertions(+), 10289 deletions(-)`
  - `git show --stat --summary dd9f224` -> chat intent is narrow: `crates/opentake-agent/src/chat/{llm,loop,mod,session}.rs`, `crates/opentake-agent/src/lib.rs`, `src-tauri/src/chat.rs`, `src-tauri/src/lib.rs`, `web/src/components/agent/AgentPanel.tsx`, `web/src/i18n/dict.ts`, `web/src/lib/api.ts`, `web/src/lib/types.ts`, `web/src/store/chatStore.ts`
- Selective replay result:
  - Replayed the chat modules into `crates/opentake-agent/src/chat/*` and exported them from `crates/opentake-agent/src/lib.rs`.
  - Adapted provider handling away from stale auto-pick: `chat_send` now takes the explicit UI-selected `chatProvider`, OpenAI/Anthropic are honored as selected, and `google` fails clearly instead of silently falling back.
  - Reused the current desktop bridge contract by wiring chat through the same workflow registry + media bridge shape as `src-tauri/src/mcp.rs`; the chat tool catalog also hides bridge-dependent tools when the dispatcher lacks that bridge.
  - Spliced chat state/commands into the current Tauri app without disturbing newer playback/export/media wiring.
  - Restored the web chat surface with minimal additions to `AgentPanel`, `chatStore`, `api`, `types`, and `dict`, opening the current Settings modal for no-key guidance.
- Verification:
  - `cargo test -p opentake-agent chat -- --nocapture`
    - result: `20 passed; 0 failed`
  - `cargo test -p opentake-agent mcp::dispatch -- --nocapture`
    - result: `100 passed; 0 failed`
  - `cargo test --manifest-path src-tauri/Cargo.toml -p opentake-tauri chat --lib -- --nocapture`
    - result: `2 passed; 0 failed`
  - `cargo test --manifest-path src-tauri/Cargo.toml -p opentake-tauri secret --lib -- --nocapture`
    - result: `5 passed; 0 failed`
  - `pnpm -C web test -- src/store/chatStore.test.ts`
    - result: `48 passed (48 files), 522 passed (522 tests)`; includes the new `chatStore` coverage
  - `pnpm -C web exec tsc -b --pretty false`
    - result: passed with no diagnostics
