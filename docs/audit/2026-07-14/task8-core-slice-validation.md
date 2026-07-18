# OpenTake Task 8 Core Slice Map Validation

## Verdict

**Needs correction before the Core map is used as an authoritative evidence ledger.**

Finding count: **C=0, I=4, M=2**.

- All 86 unique `planned=false` product paths exist in the current checkout: 82 tracked files plus 4 directories containing tracked module files.
- Four of the 118 unique non-null path/symbol anchors are invalid: one uses a nonexistent associated method, one uses a stale function name, and two point at the wrong source file.
- All 40 unique `existing-owned` test paths exist and are tracked. One named test reference points to the wrong file.
- One `reviewed-planned` test claims Tauri preview/export parity from a lower-level crate that cannot own those Tauri call paths.
- One planned Ruby test has no current repository runner or CI invocation.
- Core record IDs are correctly unprefixed: all 177 IDs are unique and none begins with `requirement-`.

## Validation snapshot

- Repository: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence`
- HEAD: `961f28dc48f18570d8f107a302114eafc0543794`
- Input: `docs/audit/2026-07-14/task8-core-slice-map.json`
- Input SHA-256: `e097640b5062a93290f132182f3e577f200dd0e767d49144ddd780a4dc2f6dc3`
- The tracked tree had 34 tracked status entries at validation time. Validation was read-only and did not mutate the repository.

## Machine statistics

| Check | Total | Unique | Failures / qualifications |
|---|---:|---:|---:|
| Scope groups | 5 | 5 | 0 |
| Slices | 85 | 85 | 0 |
| Record IDs | 177 | 177 | 0 duplicates; 0 prefixed IDs |
| Declared group record total | 177 | — | matches parsed IDs |
| `planned=false` product references | 182 | 146 path/symbol pairs | 4 invalid symbol anchors |
| `planned=false` product paths | 182 refs | 86 paths | 0 absent; 82 files + 4 directories |
| Non-null product symbols | 148 refs | 118 path/symbol pairs | 4 invalid semantic anchors |
| `existing-owned` test references | 74 | 67 path/name pairs | 1 invalid path/name pairing |
| `existing-owned` test paths | 74 refs | 40 paths | 0 missing; 0 untracked |
| `reviewed-planned` test references | 63 | 58 path/name pairs | 1 crate-ownership issue; 1 unassigned runner |
| `reviewed-planned` test paths | 63 refs | 44 paths | 1 parent directory to create |

Runner distribution by reference:

| Evidence class | Vitest | Cargo | Node test | Ruby/direct | GitHub Actions |
|---|---:|---:|---:|---:|---:|
| `existing-owned` | 8 | 65 | 1 | 0 | 0 |
| `reviewed-planned` | 13 | 48 | 0 | 1 | 1 |

## Findings

### I-01 — `EditCommand::apply` is not a tracked symbol

The map declares:

`crates/opentake-ops/src/command.rs#EditCommand::apply`

`EditCommand` is an enum (`command.rs:220`), and there is no inherent `impl EditCommand { fn apply(...) }`. The implementation is the free function `apply` at `crates/opentake-ops/src/command.rs:454`.

Evidence: the invalid map entry is `docs/audit/2026-07-14/task8-core-slice-map.json:191`.

Recommended correction: use `crates/opentake-ops/src/command.rs#apply`. Do not retain the qualified associated-method form.

### I-02 — Timeline-settings product and existing-test anchors are stale

The `CC-first-video-settings` slice declares both:

- `crates/opentake-ops/src/ops/settings.rs#apply_timeline_settings`
- `crates/opentake-ops/src/ops/settings.rs#set_timeline_settings_is_undoable` as `existing-owned`

Neither symbol/test name exists in that file. The tracked implementation is `set_timeline_settings` at `crates/opentake-ops/src/ops/settings.rs:31`, while the tracked undoability test is `set_timeline_settings_is_undoable` at `crates/opentake-ops/src/command.rs:3970`.

Evidence: the stale map entries are `docs/audit/2026-07-14/task8-core-slice-map.json:245` and `:250`.

Recommended correction: rename the product anchor to `settings.rs#set_timeline_settings` and move the existing-owned test reference to `command.rs#set_timeline_settings_is_undoable`.

### I-03 — Render texture symbols point to the wrong file

The map contains two invalid path/symbol pairs:

- `crates/opentake-render/src/source.rs#TextureResolver` (`docs/audit/2026-07-14/task8-core-slice-map.json:712`)
- `crates/opentake-render/src/source.rs#TextureSource::Text` (`docs/audit/2026-07-14/task8-core-slice-map.json:897`)

Neither symbol is defined in `source.rs`. Their tracked owners are:

- `TextureResolver`: `crates/opentake-render/src/gpu/compositor.rs:155`
- `TextureSource` (including `Text`): `crates/opentake-render/src/plan/types.rs:45`

Recommended correction: update both map paths to their defining files. Re-exporting them from `lib.rs` does not make `source.rs` a valid owner.

### I-04 — Planned denoise parity test has the wrong crate ownership

The planned test is:

`crates/opentake-media/tests/denoise.rs#deterministic_noise_fixture_bypass_and_preview_export_parity`

The slice explicitly names `src-tauri/src/playback/audio.rs` and `src-tauri/src/export.rs` as the preview/export consumers, but `opentake-tauri` depends on `opentake-media` (`src-tauri/Cargo.toml:44`). A media integration test cannot depend back on the Tauri crate without reversing the dependency and creating a cycle; it also cannot directly exercise Tauri-private wiring. The invalid ownership claim appears at `docs/audit/2026-07-14/task8-core-slice-map.json:844`.

Recommended correction: keep deterministic algorithm/bypass tests in `crates/opentake-media/tests/denoise.rs`, then add co-located Tauri wiring tests under `src-tauri/src/playback/audio.rs` and `src-tauri/src/export.rs`, or expose a deliberate higher-level integration seam owned by Tauri.

### M-01 — Planned Ruby evidence validator is not assigned to a current runner

`scripts/tests/validate-c1b-evidence-test.rb` is a reasonable location for a standalone process-contract test, and file existence was not required. However, the repository currently has no `scripts/` directory, Ruby test harness, package task, or CI command that would collect/invoke it. Unlike Cargo, Vitest, and Node's named completion-audit command, a `.rb` file is not automatically collected.

Evidence: the planned entry is `docs/audit/2026-07-14/task8-core-slice-map.json:134`.

Recommended correction: record and add an explicit runner such as `ruby scripts/tests/validate-c1b-evidence-test.rb` in the relevant CI/evidence plan, preferably using standard-library Minitest so no new dependency is required.

### M-02 — Four product anchors identify directories rather than tracked files

The media-facade slice uses these `planned=false` paths:

- `crates/opentake-media/src/decode`
- `crates/opentake-media/src/encode`
- `crates/opentake-media/src/search`
- `crates/opentake-media/src/transcribe`

All four directories exist and contain tracked files, so they pass an existence check. Git does not track directories as objects, though; the precise tracked module owners are each directory's `mod.rs`. The entries are at `docs/audit/2026-07-14/task8-core-slice-map.json:957-960`.

Recommended correction: use `decode/mod.rs`, `encode/mod.rs`, `search/mod.rs`, and `transcribe/mod.rs` so future validation is anchored to tracked files rather than checkout-derived directories.

## Runner, feature, and platform validation

### Existing-owned tests

- **Cargo:** all 65 references use tracked Rust source/unit-test files or crate integration-test files. Every non-null name resolves to an attributed `#[test]` / `#[tokio::test]` function except the wrong-file reference in I-02. Path-only Rust evidence files contain collected test functions.
- **Vitest:** all 8 references use tracked `*.test.ts` / `*.test.tsx` files. `web/package.json:10` defines `vitest run`, and the Vite config has no custom exclusion that removes them.
- **Node:** `tools/completion-audit.test.mjs` is explicitly run with `node --test tools/completion-audit.test.mjs` by the project audit command.
- The two Tauri playback references are gated by `playback-engine`, but that feature is currently a default (`src-tauri/Cargo.toml:96`), so the documented `cargo test --workspace` runner collects them. They are intentionally absent under `--no-default-features`.
- FFmpeg and GPU integration test files are collected by Cargo but may self-skip at runtime when FFmpeg/GPU prerequisites are absent. Current Linux CI installs FFmpeg; GPU collection is not the same as native GPU execution evidence.

### Reviewed-planned tests

- Cargo and Vitest paths use conventional owning-crate/source locations; planned files were not required to exist.
- Unix and Windows safe-filesystem tests in `safe_fs/tests.rs` require appropriate `cfg` guards and OS runners. The current CI has Linux and Windows Cargo jobs, so the intended ownership is reasonable.
- Planned playback tests are under the Tauri crate and are valid under the default `playback-engine` feature.
- `.github/workflows/ci.yml#packaged_macos_windows_sidecars_resolve_and_execute` is treated as a planned GitHub Actions acceptance step, not as a unit-test function; the workflow is the correct owner for packaged cross-OS sidecar execution.
- I-04 is the one substantive crate-ownership failure. M-01 is a runner-registration gap.

## Method and limits

- Parsed all five groups and all 85 slices; compared declared record totals with 177 unique unprefixed IDs.
- Checked product paths and existing-test paths against the filesystem and `git ls-files`; directory anchors were validated by tracked descendants.
- Checked all 118 unique non-null product anchors in their declared files, including qualification semantics for methods, enum variants, and trait implementations.
- Checked every non-null existing-test name exactly; Rust entries additionally had to be attributed test functions in the declared file.
- Validated Cargo module/features, Vitest discovery, Node direct execution, OS-specific CI ownership, and planned crate dependency direction.
- No test suite was executed because this was a read-only map-to-tree validation. This report establishes collection/ownership, not behavioral pass results.
