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
| `fix/text-raster-alignment` | Integrated | commit `89bf38c`, `154/1` drift; direct stale merge rejected because branch head is 154 behind and inspection already showed broad stale-branch direct merge risk against current main-line work | Selective replay only: ported `text_engine.rs` text-style/shadow alignment delta plus focused `gpu_text.rs` coverage, then verified with current text tests |
| `test/render-pixel-diff` | Deferred | one old commit `eb6e429`, 154 behind / 1 ahead | Defer until text raster is committed; then inspect and replay pixel diff work |
| `fix/91-media-library-rewrite` | Deferred | one old commit `b9e4954`, 154 behind / 1 ahead | Defer until reverse clip and media surfaces are stable; then inspect for non-regressive pieces |
| `feat/save-clip-as-media` | Deferred | one old commit `708fd44`, 154 behind / 1 ahead | Defer until reverse and media surfaces are stable; then inspect save-clip-as-media work |
| `feat/freeze-frame` | Deferred | one old commit `da3e934`, 154 behind / 1 ahead | Defer until source-frame mapping is stable; then inspect and replay freeze-frame work |
| `feat/account-scaffold` | Deferred | one old commit `4986716`, 154 behind / 1 ahead | Defer until editing recovery is settled; then inspect and replay account scaffold work |
| `feat/agent-chat-panel` | Deferred | one old commit `dd9f224`, 154 behind / 1 ahead | Defer until the wide-surface tasks are done; then inspect and replay chat panel work |
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
  - `cargo test -p opentake-render font_size_scales_with_canvas_height -- --nocapture`
    - result: `test font_size_scales_with_canvas_height ... ok`
  - `cargo test -p opentake-render shadow_paints_pixels_outside_glyph_footprint -- --nocapture`
    - result: `test shadow_paints_pixels_outside_glyph_footprint ... ok`
  - `cargo test -p opentake-render alignment_shifts_glyph_x_centroid -- --nocapture`
    - result: `test alignment_shifts_glyph_x_centroid ... ok`
