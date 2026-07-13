# Task 5 Batch A Render Report

## What I inspected

- Task brief: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/.superpowers/sdd/task-5-brief.md`
- Branch evidence: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/.superpowers/sdd/task-5-branch-inspection-a.md`
- Commit inspection:
  - `git show --stat --summary 89bf38c81016b656d5b0c5ef911b9ee7e7962432`
  - `git show --name-only --format=medium 89bf38c81016b656d5b0c5ef911b9ee7e7962432`
  - `git show --stat --summary eb6e4294f6f3397a336b15bf0c67ad8007a62f0f`
  - `git show --name-only --format=medium eb6e4294f6f3397a336b15bf0c67ad8007a62f0f`
- Current-file comparison:
  - `crates/opentake-render/src/gpu/text_engine.rs`
  - `crates/opentake-render/tests/gpu_text.rs`
  - `crates/opentake-render/tests/pixel_diff.rs`
  - `docs/superpowers/archive/2026-07-08-branch-integration-register.md`

## What I ported or why no-op

### fix/text-raster-alignment

- Direct merge rejected: branch is `154/1` relative to `origin/main`, and Task 5 branch inspection already marked stale-branch direct merge risk.
- Ported from `89bf38c81016b656d5b0c5ef911b9ee7e7962432`:
  - `crates/opentake-render/src/gpu/text_engine.rs`
    - expanded font weight/style inference from PostScript-like font names
    - aligned shadow blur radius with upstream scaling
    - replaced simple 2-pass box blur with 3-pass-per-axis blur helper
    - documented the geometry/placement invariants that justify the mapping
  - `crates/opentake-render/tests/gpu_text.rs`
    - added focused non-GPU raster tests for scaling, shadow spread, alignment, wrapping, determinism, and upstream padding parity

### test/render-pixel-diff

- Direct merge rejected: branch is `154/1` relative to `origin/main`, and Task 5 branch inspection already marked stale-branch direct merge risk.
- Ported from `eb6e4294f6f3397a336b15bf0c67ad8007a62f0f`:
  - restored `crates/opentake-render/tests/pixel_diff.rs`
- No no-op path here: current checkout had no equivalent file or comparable coverage.

## Exact verification command output

### `cargo test -p opentake-render text_raster -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 3.80s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
Running unittests src/lib.rs (target/debug/deps/opentake_render-3ef118c72e802360)

running 1 test
test gpu::text_raster::tests::null_rasterizer_returns_none_without_panicking ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 56 filtered out; finished in 0.00s

Running tests/gpu_downscaled_nat.rs (target/debug/deps/gpu_downscaled_nat-af2e570018457ee0)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

Running tests/gpu_effects.rs (target/debug/deps/gpu_effects-19b27241a720679b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_smoke.rs (target/debug/deps/gpu_smoke-fa65a9b385c31364)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s

Running tests/gpu_text.rs (target/debug/deps/gpu_text-a32db4861fd1be37)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_y_orientation.rs (target/debug/deps/gpu_y_orientation-3e4afbf091dcf27c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

Running tests/pixel_diff.rs (target/debug/deps/pixel_diff-ff21385e0ad7fa72)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

Reason recorded: the literal `text_raster` filter hit the existing `NullTextRasterizer` unit test, not the current `tests/gpu_text.rs` test names.

### `cargo test -p opentake-render font_size_scales_with_canvas_height -- --nocapture`

```text
Compiling opentake-render v1.0.0 (/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/crates/opentake-render)
Finished `test` profile [unoptimized + debuginfo] target(s) in 3.73s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
Running unittests src/lib.rs (target/debug/deps/opentake_render-3ef118c72e802360)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out; finished in 0.00s

Running tests/gpu_downscaled_nat.rs (target/debug/deps/gpu_downscaled_nat-af2e570018457ee0)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

Running tests/gpu_effects.rs (target/debug/deps/gpu_effects-19b27241a720679b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_smoke.rs (target/debug/deps/gpu_smoke-fa65a9b385c31364)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s

Running tests/gpu_text.rs (target/debug/deps/gpu_text-a32db4861fd1be37)
running 1 test
test font_size_scales_with_canvas_height ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.76s

Running tests/gpu_y_orientation.rs (target/debug/deps/gpu_y_orientation-3e4afbf091dcf27c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

Running tests/pixel_diff.rs (target/debug/deps/pixel_diff-ff21385e0ad7fa72)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

### `cargo test -p opentake-render shadow_paints_pixels_outside_glyph_footprint -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.30s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
Running unittests src/lib.rs (target/debug/deps/opentake_render-3ef118c72e802360)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out; finished in 0.00s

Running tests/gpu_downscaled_nat.rs (target/debug/deps/gpu_downscaled_nat-af2e570018457ee0)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

Running tests/gpu_effects.rs (target/debug/deps/gpu_effects-19b27241a720679b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_smoke.rs (target/debug/deps/gpu_smoke-fa65a9b385c31364)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s

Running tests/gpu_text.rs (target/debug/deps/gpu_text-a32db4861fd1be37)
running 1 test
test shadow_paints_pixels_outside_glyph_footprint ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.26s

Running tests/gpu_y_orientation.rs (target/debug/deps/gpu_y_orientation-3e4afbf091dcf27c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

Running tests/pixel_diff.rs (target/debug/deps/pixel_diff-ff21385e0ad7fa72)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

### `cargo test -p opentake-render alignment_shifts_glyph_x_centroid -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.30s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
Running unittests src/lib.rs (target/debug/deps/opentake_render-3ef118c72e802360)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out; finished in 0.00s

Running tests/gpu_downscaled_nat.rs (target/debug/deps/gpu_downscaled_nat-af2e570018457ee0)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

Running tests/gpu_effects.rs (target/debug/deps/gpu_effects-19b27241a720679b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_smoke.rs (target/debug/deps/gpu_smoke-fa65a9b385c31364)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s

Running tests/gpu_text.rs (target/debug/deps/gpu_text-a32db4861fd1be37)
running 1 test
test alignment_shifts_glyph_x_centroid ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.12s

Running tests/gpu_y_orientation.rs (target/debug/deps/gpu_y_orientation-3e4afbf091dcf27c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

Running tests/pixel_diff.rs (target/debug/deps/pixel_diff-ff21385e0ad7fa72)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

### `cargo test -p opentake-render pixel -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.31s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
Running unittests src/lib.rs (target/debug/deps/opentake_render-3ef118c72e802360)

running 1 test
test plan::affine::tests::full_canvas_quad_maps_source_pixels_to_canvas_pixels ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 56 filtered out; finished in 0.00s

Running tests/gpu_downscaled_nat.rs (target/debug/deps/gpu_downscaled_nat-af2e570018457ee0)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

Running tests/gpu_effects.rs (target/debug/deps/gpu_effects-19b27241a720679b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_smoke.rs (target/debug/deps/gpu_smoke-fa65a9b385c31364)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s

Running tests/gpu_text.rs (target/debug/deps/gpu_text-a32db4861fd1be37)
running 2 tests
test shadow_paints_pixels_outside_glyph_footprint ... ok
test text_clip_composites_visible_pixels ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.28s

Running tests/gpu_y_orientation.rs (target/debug/deps/gpu_y_orientation-3e4afbf091dcf27c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

Running tests/pixel_diff.rs (target/debug/deps/pixel_diff-ff21385e0ad7fa72)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

Reason recorded: the literal `pixel` filter hit an existing affine test and did not match the new `tests/pixel_diff.rs` names.

### `cargo test -p opentake-render quadrant_round_trip_psnr_is_high -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.33s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
Running unittests src/lib.rs (target/debug/deps/opentake_render-3ef118c72e802360)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out; finished in 0.00s

Running tests/gpu_downscaled_nat.rs (target/debug/deps/gpu_downscaled_nat-af2e570018457ee0)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

Running tests/gpu_effects.rs (target/debug/deps/gpu_effects-19b27241a720679b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_smoke.rs (target/debug/deps/gpu_smoke-fa65a9b385c31364)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s

Running tests/gpu_text.rs (target/debug/deps/gpu_text-a32db4861fd1be37)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_y_orientation.rs (target/debug/deps/gpu_y_orientation-3e4afbf091dcf27c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

Running tests/pixel_diff.rs (target/debug/deps/pixel_diff-ff21385e0ad7fa72)
running 1 test
test quadrant_round_trip_psnr_is_high ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.58s
```

### `cargo test -p opentake-render half_opacity_two_track_blend_matches_hand_computed -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.29s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
Running unittests src/lib.rs (target/debug/deps/opentake_render-3ef118c72e802360)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out; finished in 0.00s

Running tests/gpu_downscaled_nat.rs (target/debug/deps/gpu_downscaled_nat-af2e570018457ee0)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

Running tests/gpu_effects.rs (target/debug/deps/gpu_effects-19b27241a720679b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_smoke.rs (target/debug/deps/gpu_smoke-fa65a9b385c31364)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s

Running tests/gpu_text.rs (target/debug/deps/gpu_text-a32db4861fd1be37)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_y_orientation.rs (target/debug/deps/gpu_y_orientation-3e4afbf091dcf27c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

Running tests/pixel_diff.rs (target/debug/deps/pixel_diff-ff21385e0ad7fa72)
running 1 test
test half_opacity_two_track_blend_matches_hand_computed ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.27s
```

### `cargo test -p opentake-render ssim_identical_frames_score_near_one -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.28s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
Running unittests src/lib.rs (target/debug/deps/opentake_render-3ef118c72e802360)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out; finished in 0.00s

Running tests/gpu_downscaled_nat.rs (target/debug/deps/gpu_downscaled_nat-af2e570018457ee0)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

Running tests/gpu_effects.rs (target/debug/deps/gpu_effects-19b27241a720679b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_smoke.rs (target/debug/deps/gpu_smoke-fa65a9b385c31364)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s

Running tests/gpu_text.rs (target/debug/deps/gpu_text-a32db4861fd1be37)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

Running tests/gpu_y_orientation.rs (target/debug/deps/gpu_y_orientation-3e4afbf091dcf27c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

Running tests/pixel_diff.rs (target/debug/deps/pixel_diff-ff21385e0ad7fa72)
running 1 test
test ssim_identical_frames_score_near_one ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.46s
```

## Commits created

- `4fdd575` `fix(render): replay text raster alignment from branch queue`
- `75db4d1` `test(render): replay pixel diff coverage from branch queue`

## Files changed

- `crates/opentake-render/src/gpu/text_engine.rs`
- `crates/opentake-render/tests/gpu_text.rs`
- `crates/opentake-render/tests/pixel_diff.rs`
- `docs/superpowers/archive/2026-07-08-branch-integration-register.md`
- `.superpowers/sdd/task-5-render-report.md`

## Concerns

- The required `text_raster` / `pixel` filters do not directly map to the newly replayed test names, so I recorded the mismatch and added focused current-test runs as compensating verification.
- Cargo emitted the existing future-incompatibility warning for `block v0.1.6`; it did not block these test runs.

## Fixes After Review

- Applied rustfmt-equivalent formatting to the replayed test files by running `cargo fmt --all`.
- Added stronger replay evidence by running the full `gpu_text` and `pixel_diff` test binaries and updating the register to point at those complete runs.

### `cargo fmt --all --check`

```text
```

### `cargo test -p opentake-render --test gpu_text -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 1.78s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
Running tests/gpu_text.rs (target/debug/deps/gpu_text-a32db4861fd1be37)

running 7 tests
test natural_size_shadow_padding_matches_upstream ... ok
test long_text_wraps_in_narrow_box ... ok
test alignment_shifts_glyph_x_centroid ... ok
test text_clip_composites_visible_pixels ... ok
test shadow_paints_pixels_outside_glyph_footprint ... ok
test rasterize_is_deterministic_ssim_one ... ok
test font_size_scales_with_canvas_height ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s
```

### `cargo test -p opentake-render --test pixel_diff -- --nocapture`

```text
Compiling opentake-render v1.0.0 (/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/crates/opentake-render)
Finished `test` profile [unoptimized + debuginfo] target(s) in 2.11s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
Running tests/pixel_diff.rs (target/debug/deps/pixel_diff-ff21385e0ad7fa72)

running 14 tests
test crop_to_uv_visible_insets_match_domain_fractions ... ok
test compose_with_identity_is_noop ... ok
test affine_flip_horizontal_with_smaller_source ... ok
test affine_centered_half_canvas_matches_hand_computed ... ok
test affine_rotation_45_matches_compose_chain ... ok
test fade_envelope_smoothstep_endpoints_and_midpoint ... ok
test crop_keyframe_uv_varies_across_frames ... ok
test transform_keyframe_midframe_affine_equals_transform_at ... ok
test fade_midframe_renders_half_brightness ... ok
test quadrant_round_trip_psnr_is_high ... ok
test half_opacity_two_track_blend_matches_hand_computed ... ok
test quadrant_markers_land_in_authored_corners ... ok
test ssim_identical_frames_score_near_one ... ok
test text_overlay_visible_above_video ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
```

### `git diff --check -- crates/opentake-render/src/gpu/text_engine.rs crates/opentake-render/tests/gpu_text.rs crates/opentake-render/tests/pixel_diff.rs docs/superpowers/archive/2026-07-08-branch-integration-register.md`

```text
```
