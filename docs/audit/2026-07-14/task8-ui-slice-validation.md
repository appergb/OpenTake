# OpenTake Task 8 UI Slice Map Validation

## Verdict

**Needs correction before the map is treated as an authoritative evidence ledger.**

Finding count: **C=0, I=2, M=1**.

- No `planned=false` product path or symbol reference is missing from the current tracked tree.
- Every `existing-owned` test path exists, is tracked, and belongs to a recognized test runner.
- One `existing-owned` test name is stale and does not exist.
- One `reviewed-planned` test location has the wrong crate ownership for part of the behavior named by the test.
- One existing Rust test is collected only when the `whisper-backend` feature is active; the documented workspace runner currently activates it through feature unification.

## Validation snapshot

- Repository: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence`
- HEAD: `961f28dc48f18570d8f107a302114eafc0543794`
- Input: `docs/audit/2026-07-14/task8-ui-slice-map.json`
- Input SHA-256: `4d17a7e72e9a83bd09795c64a1f37959c67ce77aae0662ae0ff89c9c602dad1a`
- The tracked tree was dirty at validation time (34 tracked status entries). Validation was read-only and evaluated the files exactly as present; it did not mutate the repository.

## Machine statistics

| Check | Total | Unique | Failures |
|---|---:|---:|---:|
| Scope groups | 4 | 4 | 0 |
| Slices | 43 | 43 | 0 |
| Record IDs | 176 | 176 | 0 |
| `planned=false` product references | 139 | 122 path/symbol pairs | 0 |
| `planned=false` product paths | 139 refs | 75 paths | 0 missing; 0 untracked |
| Non-null product symbols | 135 refs | 119 path/symbol pairs | 0 missing from referenced file |
| `existing-owned` test references | 66 | 63 path/name pairs | 1 invalid name |
| `existing-owned` test paths | 66 refs | 46 paths | 0 missing; 0 untracked |
| `reviewed-planned` test references | 39 | 39 path/name pairs | 1 ownership issue |
| `reviewed-planned` test paths | 39 refs | 38 paths | 0 unrecognized runner paths |

Runner distribution:

| Evidence class | Vitest | Cargo | Node test |
|---|---:|---:|---:|
| `existing-owned` refs | 20 | 46 | 0 |
| `reviewed-planned` refs | 18 | 20 | 1 |

## Findings

### I-01 — Stale `existing-owned` test name

The map references:

`crates/opentake-domain/src/caption_sync.rs#syncs_style_across_tracks`

The file is tracked and its unit-test module is collected by Cargo, but no test with that name exists. The relevant tracked test is:

`crates/opentake-domain/src/caption_sync.rs:172#multi_track_same_group_all_restyled`

Evidence: the invalid map entry is at `docs/audit/2026-07-14/task8-ui-slice-map.json:263`; the actual `#[test]` begins at `crates/opentake-domain/src/caption_sync.rs:171` and the function name is at line 172.

Recommended correction: replace the stale name with `multi_track_same_group_all_restyled` if that test is the intended proof. Do not retain the old name as `existing-owned`.

### I-02 — Planned module-tree test crosses the crate dependency boundary

The planned entry is:

`crates/opentake-agent/tests/module_tree.rs#documented_exports_and_tauri_entrypoints_compile`

An integration test under `opentake-agent` can directly compile/check that crate's public exports, but it does not own the private Tauri entrypoints in `src-tauri/src/mcp.rs` and `src-tauri/src/chat.rs`. The dependency direction is `opentake-tauri -> opentake-agent` (`src-tauri/Cargo.toml:47`); `opentake-agent` has no dependency on `opentake-tauri` (`crates/opentake-agent/Cargo.toml:9-41`). Adding the reverse dependency for this test would create the wrong architecture and potentially a cycle.

The map itself explicitly names both responsibilities at `docs/audit/2026-07-14/task8-ui-slice-map.json:623`.

Recommended correction: keep an agent-export compile test under `crates/opentake-agent/tests/`, and place Tauri entrypoint compile witnesses in co-located `#[cfg(test)]` modules under `src-tauri/src/mcp.rs` / `src-tauri/src/chat.rs`, or expose a deliberate public Tauri seam and test it from `src-tauri/tests/`.

### M-01 — Whisper test collection is feature-dependent

`crates/opentake-media/src/transcribe/whisper.rs#centiseconds_convert_to_seconds` is a real `#[test]` (`whisper.rs:160-161`), but the module is behind `#[cfg(feature = "whisper-backend")]` (`transcribe/mod.rs:17-18`) and `opentake-media` has `default = []` (`Cargo.toml:64-67`). Therefore:

- `cargo test -p opentake-media` with default features does **not** collect this test.
- The repository's documented `cargo test --workspace` runner does collect it in the current workspace graph because `opentake-tauri` enables `opentake-media/whisper-backend` (`src-tauri/Cargo.toml:44`); `cargo tree --workspace -e features -i opentake-media` confirmed that activation.

This is not a failure for the documented workspace runner, but the map should record the feature/runner condition if entries are expected to be independently runnable at package scope.

## Runner and ownership validation details

### Existing-owned tests

- **Vitest:** all 20 references use tracked `*.test.ts` / `*.test.tsx` files. `web/package.json:10` defines `vitest run`, and `web/vite.config.ts` contains no custom include/exclude that removes these files. Named entries exist as test/suite labels. The one path-only entry, `PanelShell.test.tsx`, contains a collected `describe` with `it.each` cases.
- **Cargo:** all 46 references use tracked Rust unit-test source files or crate `tests/*.rs` integration files. Exact `#[test]` / `#[tokio::test]` function checks passed for every named reference except I-01. Module inclusion is unconditional except the feature-gated Whisper case described in M-01.
- No existing-owned path is missing or untracked.

### Reviewed-planned tests

- All 39 references map to a recognized runner convention: 18 Vitest paths, 20 Cargo unit/integration paths, and 1 Node `--test` path.
- Thirty-eight of 39 planned references have reasonable target ownership. I-02 is the exception.
- `crates/opentake-domain/tests/` does not yet exist for the two planned domain integration tests (`multicam.rs`, `speed_curve.rs`), but creating that directory is standard Cargo layout and is therefore valid; file existence was not required for reviewed-planned evidence.
- `tools/completion-audit.test.mjs` is correctly assigned to Node's test runner; project code names `node --test tools/completion-audit.test.mjs` as its command.

## Method and limits

- Parsed every group/slice entry and checked duplicate-aware and unique counts.
- Checked product and existing-test paths with filesystem existence plus `git ls-files --error-unmatch`.
- Checked every non-null product symbol in its declared file; qualified fields, enum variants, trait methods, and local/function symbols were resolved to their terminal declaration token in that file.
- Checked every non-null existing-test name as an exact string; Rust names additionally had to appear as an attributed test function.
- Validated collection from Cargo module/feature structure, Vitest naming/configuration, and the Node test command. No test suite was executed because this task was a read-only map-to-tree validation; no pass/fail claim is made about test behavior.
