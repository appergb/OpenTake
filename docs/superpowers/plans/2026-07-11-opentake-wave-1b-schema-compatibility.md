# OpenTake Wave 1B Schema Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Open projects containing unknown persisted fields without data loss, expose an explicit compatibility read-only mode, and refuse every application-facing project mutation or project-bundle write that would discard those fields.

**Architecture:** `opentake-project` decodes all persisted JSON through `serde_ignored`, records sorted file-qualified unknown paths, and refuses writes before filesystem mutation. That compatibility state travels with `Project` into the authoritative `EditorSession`, where it blocks every persisted mutation, and into one lock-consistent runtime snapshot used by Tauri bundle export. The Web mirror renders a persistent compatibility banner; the project remains inspectable and normal video/interchange export remains available, but editing, save, save-as, and self-contained project export are rejected.

**Tech Stack:** Rust 2021, serde/serde_json/serde_ignored, Tauri 2, React 18, Zustand, TypeScript 5.6, Vitest 4, pnpm 10.

---

## Scope And Invariants

- Wave 1A delivery parent: `b149f7d11e9f9886935421941b4ba4c44bc8e53b` on `integration/opentake-full-convergence-20260710`.
- Execution starts from a clean commit whose sole parent delta is this reviewed plan.
- Canonical checkout `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake` remains read-only and unchanged.
- An unknown field at timeline, track, clip, keyframe/effect/mask, manifest, folder, entry, media source, generation input, generation log, or generation-log entry depth opens successfully and appears in the compatibility blocker list.
- Trailing JSON and malformed required components retain their current strict failure behavior. A malformed optional generation log remains readable under the existing lenient policy but makes the project compatibility read-only.
- Compatibility read-only is enforced in Rust for UI, MCP, Chat, and direct Core callers. Rejected operations do not change timeline, manifest, version, project epoch, files, or history.
- `Project::save`, `Project::save_to`, Core save/save-as, and Tauri self-contained `export_bundle` refuse before creating, deleting, truncating, copying, or replacing destination content.
- Normal known-schema projects retain existing read/write behavior.
- This plan does not claim path authorization, archive source containment, or atomic archive publication. Those remain blocking Wave 1B-C security work; until that plan passes, self-contained project export is not release-approved even for known-schema projects.
- Each code slice is committed, independently specification-reviewed, independently quality-reviewed, fixed, and re-reviewed before the next code slice starts.

## File Map

- `Cargo.toml`, `Cargo.lock`, `crates/opentake-project/Cargo.toml`: add `serde_ignored`.
- `crates/opentake-project/src/bundle.rs`: decode with ignored-path collection and enforce write refusal.
- `crates/opentake-project/src/error.rs`: typed compatibility write error.
- `crates/opentake-project/src/lib.rs`: export compatibility state.
- `crates/opentake-project/tests/schema_compat.rs`: strict/lenient/unknown-field byte-preservation tests.
- `crates/opentake-core/src/session.rs`: carry compatibility and reject persisted mutations.
- `crates/opentake-core/src/core.rs`: expose compatibility in timeline snapshots plus a dedicated lock-consistent bundle-export snapshot.
- `crates/opentake-core/src/error.rs`: typed compatibility read-only mutation error.
- `crates/opentake-core/src/dto.rs`: compatibility fields in the timeline snapshot DTO.
- `crates/opentake-core/tests/schema_compat.rs`: Core non-mutation tests.
- `src-tauri/src/export.rs`: one runtime snapshot plus compatibility preflights before bundle/range export creates output.
- `src-tauri/src/media.rs`: propagate import/favorite errors and preflight UI imports plus save-as-media output.
- `src-tauri/src/mcp.rs`: propagate path-import mutations and preflight path/bytes imports before work.
- `src-tauri/src/render.rs`: propagate the fallible internal import helper and preflight before capture files are rendered.
- `src-tauri/tests/schema_compat_integration.rs`: bundle-export refusal with destination receipts.
- `src-tauri/tests/bundle_export_integration.rs`: pass writable compatibility through the existing known-schema export contract.
- `web/src/lib/api.ts`: provide writable compatibility defaults for browser fallback snapshots.
- `web/src/lib/api.test.ts`, `web/src/store/projectActions.test.ts`, `web/src/store/sync.test.ts`, `web/src/store/editActions.test.ts`: update known-schema snapshot fixtures to the required runtime contract.
- `web/src/lib/types.ts`: compatibility snapshot contract.
- `web/src/store/projectStore.ts`: mirrored compatibility state reset atomically on project epoch change.
- `web/src/store/projectActions.ts`, `web/src/store/sync.ts`: replace project snapshots atomically instead of setting mirror/path piecemeal.
- `web/src/store/recentStore.ts`: clear active project and compatibility fields through one reset action.
- `web/src/store/projectStore.schemaCompatibility.test.ts`: store contract tests.
- `web/src/components/shell/CompatibilityBanner.tsx`: persistent read-only explanation.
- `web/src/components/shell/CompatibilityBanner.test.tsx`: rendered banner tests.
- `web/src/App.tsx`, `web/src/i18n/dict.ts`: mount and localize the banner.

## Mandatory Per-Slice Review Gate

Before dispatching each Task 2–4 implementer, the controller creates exactly one immutable `$SAFETY/slice-bases/task-$TASK.txt` via `apply_patch`, containing `slice base <40-hex HEAD>`, and asserts that commit is an ancestor of the integration HEAD. The base file is never replaced across fix attempts. After every initial or fix commit, and before starting the next task:

1. Record `SHA=$(git rev-parse HEAD)`, the complete cumulative `git diff --name-status "$SLICE_BASE" "$SHA"`, and `git diff "$SLICE_BASE" "$SHA" --`. `SLICE_BASE` never changes across fix attempts for that task.
2. Detach `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review` at `SHA` and assert both integration and review worktrees are clean.
3. Run `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260711-wave1b-schema/run-code-slice-gate.zsh "$REVIEW" "task-$TASK-$SHA-attempt-$ATTEMPT" "$SAFETY/slice-bases/task-$TASK.txt"` against the detached review tree. The script creates that exact immutable log directory, verifies the pinned base, and records exact test listings under the Wave 1B safety root.
4. Dispatch a specification reviewer that did not implement the slice. Persist its report as `$LOG/spec-review.md`, then run the Wave 1B `verify-review-report.zsh` with the exact `SLICE_BASE`, `SHA`, `spec`, and expected verdict.
5. Only after specification approval, dispatch a different code-quality reviewer. Persist its report as `$LOG/quality-review.md` and verify it with role `quality`. Extract each report's single `reviewer agent:` value and assert they differ. Both reports must independently be `APPROVE` with Critical/Important/Minor `0/0/0`; there is no controller-authored combined verdict that can mask either reviewer.
6. Any finding receives a new fix commit, new immutable attempt directory, and both reviewers re-read the full cumulative `SLICE_BASE..SHA`. The next task is blocked until both reports for the latest cumulative attempt pass the strict verifier.

During RED, when new APIs intentionally do not compile, test presence is proved by exact source-level `rg` counts before the failing cargo command. During GREEN and every review gate, Rust runs `-- --list`, saves the listing, and asserts the exact expected `: test` count before running tests. A zero-test exit is a gate failure.

### Task 1: Commit And Pin The Reviewed Plan Baseline

**Files:**
- Create outside repository: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260711-wave1b-schema/baseline.txt`
- Create outside repository: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260711-wave1b-schema/run-code-slice-gate.zsh`
- Create outside repository: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260711-wave1b-schema/verify-review-report.zsh`

- [ ] **Step 1: Commit only this plan after independent approval**

After both independent plan reviewers approve this exact file, run:

```zsh
set -euo pipefail
ROOT='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
PLAN='docs/superpowers/plans/2026-07-11-opentake-wave-1b-schema-compatibility.md'
cd "$ROOT"
test "$(git rev-parse HEAD)" = b149f7d11e9f9886935421941b4ba4c44bc8e53b
test "$(git status --porcelain)" = "?? $PLAN"
git add "$PLAN"
git commit -m 'docs: plan Wave 1B schema compatibility safety'
test "$(git diff-tree --no-commit-id --name-only -r HEAD)" = "$PLAN"
test "$(git rev-parse HEAD^)" = b149f7d11e9f9886935421941b4ba4c44bc8e53b
test -z "$(git status --porcelain)"
```

Expected: one plan-only commit with the exact Wave 1A delivery parent and a clean worktree.

- [ ] **Step 2: Create and verify the non-overwriting safety baseline**

Run the following read-only value capture, then use `apply_patch` to create `baseline.txt` from its exact five output lines:

```zsh
set -euo pipefail
ROOT='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
CANON='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake'
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260711-wave1b-schema'
WAVE1A='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/manifest.txt'
test ! -e "$SAFETY"
mkdir -p "$SAFETY/logs" "$SAFETY/slice-bases"
printf 'plan_head %s\n' "$(git -C "$ROOT" rev-parse HEAD)"
printf 'plan_tree %s\n' "$(git -C "$ROOT" rev-parse HEAD^{tree})"
printf 'canonical_head %s\n' "$(git -C "$CANON" rev-parse HEAD)"
printf 'canonical_status_sha256 %s\n' "$(git -C "$CANON" status --porcelain=v1 | shasum -a 256 | awk '{print $1}')"
printf 'wave1a_manifest_sha256 %s\n' "$(shasum -a 256 "$WAVE1A" | awk '{print $1}')"
```

Recompute each value and compare it to its unique `baseline.txt` line. Expected: all match and both Git checkouts retain their prior status.

- [ ] **Step 3: Create the Wave 1B code-slice gate**

Use `apply_patch` to create `run-code-slice-gate.zsh` with this exact content, then `chmod +x` it:

```zsh
#!/bin/zsh
set -euo pipefail
REVIEW="${1:?review worktree path required}"
SLICE="${2:?unique slice id required}"
BASE_FILE="${3:?immutable slice-base file required}"
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260711-wave1b-schema'
CARGO_AUDIT_BIN='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/target/cargo-tools/bin/cargo-audit'
LOG="$SAFETY/logs/$SLICE"
test ! -e "$LOG"
mkdir -p "$LOG"
cd "$REVIEW"
test -x "$CARGO_AUDIT_BIN"
test -s "$BASE_FILE"
SLICE_BASE=$(sed -n 's/^slice base //p' "$BASE_FILE")
test "$(rg -c '^slice base [0-9a-f]{40}$' "$BASE_FILE" || true)" -eq 1
git merge-base --is-ancestor "$SLICE_BASE" HEAD
printf 'slice base %s\nreview SHA %s\n' "$SLICE_BASE" "$(git rev-parse HEAD)" \
  | tee "$LOG/revision-receipt.txt"
pnpm --dir web install --frozen-lockfile 2>&1 | tee "$LOG/pnpm-install.log"
cargo fmt --all --check 2>&1 | tee "$LOG/cargo-fmt.log"
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tee "$LOG/cargo-clippy.log"
PROJECT_LIST="$LOG/project-schema-test-list.log"
cargo test -p opentake-project --test schema_compat -- --list 2>&1 | tee "$PROJECT_LIST"
test "$(rg -c ': test$' "$PROJECT_LIST")" -eq 7
if [[ -f crates/opentake-core/tests/schema_compat.rs ]]; then
  CORE_LIST="$LOG/core-schema-test-list.log"
  TAURI_LIST="$LOG/tauri-schema-test-list.log"
  cargo test -p opentake-core --test schema_compat -- --list 2>&1 | tee "$CORE_LIST"
  cargo test -p opentake-tauri --test schema_compat_integration -- --list 2>&1 | tee "$TAURI_LIST"
  test "$(rg -c ': test$' "$CORE_LIST")" -eq 4
  test "$(rg -c ': test$' "$TAURI_LIST")" -eq 2
  for filter in \
    favorite_command_refuses_unknown_project_without_manifest_change \
    import_commands_refuse_unknown_project_without_manifest_or_folder_change \
    save_clip_as_media_refuses_before_media_output_creation \
    capture_frame_to_media_refuses_before_capture_creation \
    capture_freeze_frame_refuses_before_capture_creation \
    export_range_refuses_before_save_output_creation \
    mcp_path_import_refuses_unknown_project_without_manifest_or_folder_change \
    mcp_bytes_import_refuses_before_media_tree_mutation; do
    UNIT_LIST="$LOG/$filter-test-list.log"
    cargo test -p opentake-tauri "$filter" --lib -- --list 2>&1 | tee "$UNIT_LIST"
    test "$(rg -c ': test$' "$UNIT_LIST")" -eq 1
  done
fi
cargo test --workspace --all-targets -- --nocapture 2>&1 | tee "$LOG/cargo-test.log"
cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings \
  2>&1 | tee "$LOG/cargo-clippy-no-default.log"
pnpm --dir web build 2>&1 | tee "$LOG/web-build.log"
pnpm --dir web test 2>&1 | tee "$LOG/web-test.log"
pnpm --dir web audit --audit-level high 2>&1 | tee "$LOG/pnpm-audit.log"
"$CARGO_AUDIT_BIN" audit 2>&1 | tee "$LOG/cargo-audit.log"
test -z "$(git status --porcelain)"
```

Run `zsh -n` on the file and assert the old Wave 1A script is unchanged. Expected: syntax passes and the new script uniquely contains the Wave 1B safety root plus `pnpm --dir web`.

- [ ] **Step 4: Create the strict two-reviewer report verifier**

Use `apply_patch` to create `verify-review-report.zsh` with this exact content, then `chmod +x` it:

```zsh
#!/bin/zsh
set -euo pipefail
REVIEW="${1:?review worktree path required}"
REPORT="${2:?report path required}"
SLICE_BASE="${3:?slice base required}"
EXPECTED_SHA="${4:?exact reviewed SHA required}"
EXPECTED_ROLE="${5:?expected role required}"
EXPECTED_VERDICT="${6:?expected APPROVE or REJECT required}"
case "$EXPECTED_ROLE" in spec|quality) ;; *) exit 2 ;; esac
case "$EXPECTED_VERDICT" in APPROVE|REJECT) ;; *) exit 2 ;; esac
test -s "$REPORT"
test "$(git -C "$REVIEW" rev-parse HEAD)" = "$EXPECTED_SHA"
test -z "$(git -C "$REVIEW" status --porcelain)"
git -C "$REVIEW" merge-base --is-ancestor "$SLICE_BASE" "$EXPECTED_SHA"
for field in 'reviewer agent' 'review role' 'slice base' 'exact SHA' \
  'inspected files' requirements tests 'failure modes' Critical Important Minor verdict; do
  test "$(rg -c "^${field}:" "$REPORT" || true)" -eq 1
done
test "$(sed -n 's/^review role: //p' "$REPORT")" = "$EXPECTED_ROLE"
test "$(sed -n 's/^slice base: //p' "$REPORT")" = "$SLICE_BASE"
test "$(sed -n 's/^exact SHA: //p' "$REPORT")" = "$EXPECTED_SHA"
test "$(sed -n 's/^verdict: //p' "$REPORT")" = "$EXPECTED_VERDICT"
CRITICAL=$(sed -n 's/^Critical: //p' "$REPORT")
IMPORTANT=$(sed -n 's/^Important: //p' "$REPORT")
MINOR=$(sed -n 's/^Minor: //p' "$REPORT")
for count in "$CRITICAL" "$IMPORTANT" "$MINOR"; do
  printf '%s\n' "$count" | rg -q '^[0-9]+$'
done
if [[ "$EXPECTED_VERDICT" = APPROVE ]]; then
  test "$CRITICAL" -eq 0
  test "$IMPORTANT" -eq 0
  test "$MINOR" -eq 0
else
  test $((CRITICAL + IMPORTANT + MINOR)) -gt 0
fi
```

Run `zsh -n` on both Wave 1B scripts. For every review attempt, invoke:

```zsh
VERIFY="$SAFETY/verify-review-report.zsh"
"$VERIFY" "$REVIEW" "$LOG/spec-review.md" "$SLICE_BASE" "$SHA" spec APPROVE
"$VERIFY" "$REVIEW" "$LOG/quality-review.md" "$SLICE_BASE" "$SHA" quality APPROVE
SPEC_AGENT=$(sed -n 's/^reviewer agent: //p' "$LOG/spec-review.md")
QUALITY_AGENT=$(sed -n 's/^reviewer agent: //p' "$LOG/quality-review.md")
test -n "$SPEC_AGENT"
test -n "$QUALITY_AGENT"
test "$SPEC_AGENT" != "$QUALITY_AGENT"
```

When either reviewer rejects, verify that report with expected `REJECT`, commit every fix, and create a new attempt directory before asking both reviewers to re-read the cumulative slice.

### Task 2: Detect Unknown Fields And Refuse Project Writes

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/opentake-project/Cargo.toml`
- Modify: `crates/opentake-project/src/bundle.rs`
- Modify: `crates/opentake-project/src/error.rs`
- Modify: `crates/opentake-project/src/lib.rs`
- Create: `crates/opentake-project/tests/schema_compat.rs`
- Modify: `crates/opentake-project/tests/roundtrip.rs`

- [ ] **Step 1: Write seven RED compatibility tests**

Create exactly these tests:

```rust
unknown_top_level_timeline_field_blocks_writes_without_changing_bytes
unknown_nested_clip_field_blocks_writes_without_changing_bytes
unknown_nested_manifest_entry_and_source_fields_block_writes
unknown_generation_log_entry_field_blocks_writes
malformed_optional_generation_log_opens_but_blocks_writes
trailing_required_json_remains_a_strict_open_error
known_schema_remains_writable
```

Every blocker test records source component bytes, attempts same-path `save`, attempts `save_to` at a nonexistent destination, and asserts source bytes unchanged plus destination absent. Without increasing the seven-test count, fixtures inject and assert unknown keys at every promised depth: timeline top level; track; clip; keyframe track and keyframe value; effect; mask; manifest top level; folder; manifest entry; `source`; `generationInput`; generation-log top level; and generation-log entry. The tests assert the exact globally sorted file-qualified blocker list, not only `contains()` checks.

- [ ] **Step 2: Prove the tests are present and RED**

Run:

```zsh
set -euo pipefail
test "$(rg -c '^fn (unknown_|malformed_|trailing_|known_)' \
  crates/opentake-project/tests/schema_compat.rs)" -eq 7
cargo test -p opentake-project --test schema_compat -- --nocapture
```

Expected: seven test functions exist in source; compilation/test run fails because compatibility APIs and errors do not exist.

- [ ] **Step 3: Add strict ignored-field decoding**

Add `serde_ignored = "0.1"` to workspace dependencies and the project crate. Implement:

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectCompatibility {
    blockers: Vec<String>,
}

impl ProjectCompatibility {
    pub fn is_read_only(&self) -> bool { !self.blockers.is_empty() }
    pub fn blockers(&self) -> &[String] { &self.blockers }
    fn extend(&mut self, blockers: impl IntoIterator<Item = String>) {
        self.blockers.extend(blockers);
        self.blockers.sort();
        self.blockers.dedup();
    }
    pub fn ensure_writable(&self) -> Result<()> {
        if self.is_read_only() {
            return Err(ProjectError::CompatibilityReadOnly {
                blockers: self.blockers.clone(),
            });
        }
        Ok(())
    }
}
```

Decode each component with a helper that calls `decoder.end()`:

```rust
fn decode_component<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    file: &str,
) -> Result<(T, Vec<String>)> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let mut ignored = Vec::new();
    let value = serde_ignored::deserialize(&mut decoder, |path| {
        ignored.push(format!("{file}:{path}"));
    })
    .map_err(|error| ProjectError::json(file, error))?;
    decoder.end().map_err(|error| ProjectError::json(file, error))?;
    ignored.sort();
    ignored.dedup();
    Ok((value, ignored))
}
```

`project.json` and present `media.json` remain strict. A present generation log that is malformed/unreadable keeps the existing `None` decode behavior but adds the literal blocker `generation-log.json:invalid-or-unreadable`.

`Project::open` merges every component through `ProjectCompatibility::extend` after each decode; sorting only each component's temporary vector is insufficient. The tests assert the exact globally sorted/deduplicated blocker vector.

- [ ] **Step 4: Refuse before filesystem mutation**

Add:

```rust
#[error("project is compatibility read-only because this build does not understand: {blockers:?}")]
CompatibilityReadOnly { blockers: Vec<String> },
```

Store compatibility on `Project`; default it in `Project::new`; expose `compatibility()` and `Project::new_with_compatibility(bundle_path, compatibility)` so the Core can assemble a save without discarding the opened project's blockers. Call `ensure_writable()` as the first operation in `save_to`, before directory creation or serialization. `save()` delegates to this checked path.

The compatibility field is private so callers cannot forge writable state. Update the external `sample_project` fixture in `tests/roundtrip.rs` to start from `Project::new(bundle)` and assign its existing public payload fields; do not restore an external struct literal or make compatibility public merely to preserve that fixture.

- [ ] **Step 5: Run GREEN project checks**

```zsh
LIST=$(cargo test -p opentake-project --test schema_compat -- --list)
test "$(printf '%s\n' "$LIST" | rg -c ': test$')" -eq 7
cargo test -p opentake-project --test schema_compat -- --nocapture
cargo test -p opentake-project
cargo clippy -p opentake-project --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: seven compatibility tests and all existing project tests pass.

- [ ] **Step 6: Commit the project compatibility slice**

```zsh
git add Cargo.toml Cargo.lock crates/opentake-project/Cargo.toml \
  crates/opentake-project/src/bundle.rs crates/opentake-project/src/error.rs \
  crates/opentake-project/src/lib.rs crates/opentake-project/tests/schema_compat.rs \
  crates/opentake-project/tests/roundtrip.rs
git commit -m 'fix: protect unknown project fields from destructive writes'
```

- [ ] **Step 7: Pass the mandatory two-stage review gate**

Set `TASK=2` and `ATTEMPT=1`, execute the Mandatory Per-Slice Review Gate, fix every finding, and repeat with incremented attempts until the latest exact SHA is approved at 0/0/0. Do not begin Task 3 earlier.

### Task 3: Enforce Compatibility Read-Only In Core And Bundle Export

**Files:**
- Modify: `crates/opentake-core/src/session.rs`
- Modify: `crates/opentake-core/src/core.rs`
- Modify: `crates/opentake-core/src/error.rs`
- Modify: `crates/opentake-core/src/dto.rs`
- Create: `crates/opentake-core/tests/schema_compat.rs`
- Modify: `src-tauri/src/export.rs`
- Modify: `src-tauri/src/media.rs`
- Modify: `src-tauri/src/mcp.rs`
- Modify: `src-tauri/src/render.rs`
- Create: `src-tauri/tests/schema_compat_integration.rs`
- Modify: `src-tauri/tests/bundle_export_integration.rs`

- [ ] **Step 1: Write fourteen RED Core/Tauri tests**

Create exactly four Core tests:

```rust
unknown_project_apply_is_rejected_without_state_or_history_change
unknown_project_media_mutations_are_rejected_without_manifest_change
unknown_project_save_and_save_as_keep_source_and_destination_unchanged
bundle_snapshot_is_single_epoch_and_snapshots_carry_sorted_compatibility
```

Create exactly two Tauri integration tests:

```rust
bundle_export_refuses_unknown_project_before_new_destination_creation
bundle_export_refuses_unknown_project_without_replacing_existing_destination
```

Add these eight Tauri unit tests at the owning module boundaries:

```rust
// src-tauri/src/media.rs
favorite_command_refuses_unknown_project_without_manifest_change
import_commands_refuse_unknown_project_without_manifest_or_folder_change
save_clip_as_media_refuses_before_media_output_creation

// src-tauri/src/render.rs
capture_frame_to_media_refuses_before_capture_creation
capture_freeze_frame_refuses_before_capture_creation

// src-tauri/src/export.rs
export_range_refuses_before_save_output_creation

// src-tauri/src/mcp.rs
mcp_path_import_refuses_unknown_project_without_manifest_or_folder_change
mcp_bytes_import_refuses_before_media_tree_mutation
```

The Core media test covers import, identical/different relink, and favorite mutation. The Tauri integration tests compare a recursive destination manifest before/after, not only existence. The media import test covers explicit files plus flat, recursive, and empty-directory import, proving errors propagate without manifest/folder change. The MCP path test covers a single file and an empty directory so neither can become a successful no-op. Each file-producing unit test opens an unknown-field project, invokes the application-facing entrypoint (or the smallest extracted helper called as its first operation), and proves the complete relevant output tree is byte-for-byte unchanged. It must fail if rendering, directory creation, or file writing happens before compatibility rejection. The bytes-import case invokes the existing MCP bridge and asserts the complete `media/` tree is unchanged.

- [ ] **Step 2: Prove fourteen tests are present and RED**

```zsh
set -euo pipefail
test "$(rg -c '^fn (unknown_|bundle_)' \
  crates/opentake-core/tests/schema_compat.rs)" -eq 4
test "$(rg -c '^fn bundle_export_' \
  src-tauri/tests/schema_compat_integration.rs)" -eq 2
test "$(rg -c '^    fn favorite_command_refuses_unknown_project_without_manifest_change' \
  src-tauri/src/media.rs)" -eq 1
test "$(rg -c '^    fn save_clip_as_media_refuses_before_media_output_creation' \
  src-tauri/src/media.rs)" -eq 1
test "$(rg -c '^    fn import_commands_refuse_unknown_project_without_manifest_or_folder_change' \
  src-tauri/src/media.rs)" -eq 1
test "$(rg -c '^    fn capture_(frame_to_media|freeze_frame)_refuses_before_capture_creation' \
  src-tauri/src/render.rs)" -eq 2
test "$(rg -c '^    fn export_range_refuses_before_save_output_creation' \
  src-tauri/src/export.rs)" -eq 1
test "$(rg -c '^    fn mcp_bytes_import_refuses_before_media_tree_mutation' \
  src-tauri/src/mcp.rs)" -eq 1
test "$(rg -c '^    fn mcp_path_import_refuses_unknown_project_without_manifest_or_folder_change' \
  src-tauri/src/mcp.rs)" -eq 1
expect_red() {
  local label="$1"
  shift
  if "$@"; then
    printf 'expected RED but passed: %s\n' "$label" >&2
    return 1
  fi
}
expect_red core cargo test -p opentake-core --test schema_compat -- --nocapture
expect_red tauri-integration cargo test -p opentake-tauri --test schema_compat_integration -- --nocapture
expect_red favorite cargo test -p opentake-tauri favorite_command_refuses_unknown_project_without_manifest_change --lib -- --nocapture
expect_red save-clip cargo test -p opentake-tauri save_clip_as_media_refuses_before_media_output_creation --lib -- --nocapture
expect_red import-commands cargo test -p opentake-tauri import_commands_refuse_unknown_project_without_manifest_or_folder_change --lib -- --nocapture
expect_red capture cargo test -p opentake-tauri capture_frame_to_media_refuses_before_capture_creation --lib -- --nocapture
expect_red freeze cargo test -p opentake-tauri capture_freeze_frame_refuses_before_capture_creation --lib -- --nocapture
expect_red range cargo test -p opentake-tauri export_range_refuses_before_save_output_creation --lib -- --nocapture
expect_red mcp-path cargo test -p opentake-tauri mcp_path_import_refuses_unknown_project_without_manifest_or_folder_change --lib -- --nocapture
expect_red mcp cargo test -p opentake-tauri mcp_bytes_import_refuses_before_media_tree_mutation --lib -- --nocapture
```

Expected: all fourteen exact source assertions pass, and every independently executed RED test command fails on absent compatibility propagation/guards.

- [ ] **Step 3: Carry compatibility through `EditorSession`**

Add `compatibility: ProjectCompatibility`, default writable for new projects, and carry it from `Project::open`. `save_project_with_thumbnail` must assemble through `Project::new_with_compatibility`; constructing a fresh writable `Project` here is a test failure. Add one private `ensure_mutable()` and call it before `apply`, `import_media_file`, `relink_media_file`, and `set_media_favorite`. Expose a read-only `AppCore::ensure_project_mutable()` preflight for Tauri operations that must touch disk before a Core mutation. Change `EditorSession::set_media_favorite` and `AppCore::set_media_favorite` from bare `usize` to `Result<usize, CoreError>`. Change the real Tauri `toggle_favorite` contract from `MediaListDto` to `Result<MediaListDto, String>`, propagate `core.set_media_favorite(...)?`, then build the DTO. Update the existing direct favorite count assertions in `src-tauri/src/media.rs` to unwrap the successful known-schema results. Map the new Core error to the validation class.

Change `src-tauri::media::import_one` from `Option<MediaManifestEntry>` to `Result<Option<MediaManifestEntry>, CoreError>`: `Ok(None)` remains the unsupported-extension skip, while compatibility and other Core failures propagate. Update every call site in media, render, export, and MCP code; never convert the compatibility error back to `None`. Make `create_folder`, `mirror_dir`, `mirror_dir_scheduled`, and `mirror_dir_impl` return `Result`; propagate folder creation, per-file import, and `MoveToFolder` failures rather than discarding them. Make MCP `apply_import_metadata` return `Result<MediaManifestEntry, BridgeError>` and propagate rename/move failures.

Call `ensure_project_mutable()` before validation or traversal in the UI `import_media` and `import_folder` commands and MCP `import_from_path`, so a read-only project cannot turn a supported file, recursive directory, or empty directory request into a successful/no-op catalog response. Keep unsupported-extension skip behavior only after that preflight on writable projects.

Call `AppCore::ensure_project_mutable()` as the first fallible operation, before snapshots, rendering, cache/destination path construction, `create_dir_all`, or `write`, in every application-facing operation that produces a file and then mutates the project:

- `src-tauri::media::save_clip_as_media`
- `src-tauri::render::capture_frame_to_media`
- `src-tauri::render::capture_freeze_frame`
- `src-tauri::export::export_range`
- `src-tauri::mcp::import_from_bytes`

The entrypoints may delegate that first operation to one small shared helper, but may not perform observable work before it. Refusal requires no cleanup and leaves the relevant media/cache/saves tree byte-identical.

- [ ] **Step 4: Make runtime snapshot and IPC contracts explicit**

Add a dedicated `BundleExportSnapshot` containing timeline, manifest, generation log, project path, project epoch, and compatibility, plus `AppCore::bundle_export_snapshot()` that clones them under one session lock. Do not add generation log to the hot-path `ProjectRuntimeSnapshot`. Extend `TimelineSnapshot` with plain Rust fields for current project path and compatibility; project serde attributes belong only on `TimelineSnapshotDto`:

```rust
#[serde(rename = "compatibilityReadOnly")]
pub compatibility_read_only: bool,
#[serde(rename = "compatibilityBlockers")]
pub compatibility_blockers: Vec<String>,
```

Serialize the path as camelCase `projectPath: string | null`. Keep blockers sorted/deduplicated. This is a runtime DTO addition; the persisted domain `Timeline` JSON schema does not change.

- [ ] **Step 5: Block self-contained project export before destination work**

Make `export_bundle` take exactly one `core.bundle_export_snapshot()`. Pass `snapshot.compatibility` into `run_bundle_export`; its first line calls `ensure_writable()` before constructing a destination path or calling `archive`. Timeline, manifest, generation log, source bundle, epoch, and compatibility now come from the same lock-consistent snapshot.

- [ ] **Step 6: Run GREEN Core/Tauri checks**

```zsh
CORE_LIST=$(cargo test -p opentake-core --test schema_compat -- --list)
TAURI_LIST=$(cargo test -p opentake-tauri --test schema_compat_integration -- --list)
FAVORITE_LIST=$(cargo test -p opentake-tauri favorite_command_refuses_unknown_project_without_manifest_change --lib -- --list)
IMPORT_LIST=$(cargo test -p opentake-tauri import_commands_refuse_unknown_project_without_manifest_or_folder_change --lib -- --list)
SAVE_CLIP_LIST=$(cargo test -p opentake-tauri save_clip_as_media_refuses_before_media_output_creation --lib -- --list)
CAPTURE_LIST=$(cargo test -p opentake-tauri capture_frame_to_media_refuses_before_capture_creation --lib -- --list)
FREEZE_LIST=$(cargo test -p opentake-tauri capture_freeze_frame_refuses_before_capture_creation --lib -- --list)
RANGE_LIST=$(cargo test -p opentake-tauri export_range_refuses_before_save_output_creation --lib -- --list)
MCP_PATH_LIST=$(cargo test -p opentake-tauri mcp_path_import_refuses_unknown_project_without_manifest_or_folder_change --lib -- --list)
MCP_LIST=$(cargo test -p opentake-tauri mcp_bytes_import_refuses_before_media_tree_mutation --lib -- --list)
test "$(printf '%s\n' "$CORE_LIST" | rg -c ': test$')" -eq 4
test "$(printf '%s\n' "$TAURI_LIST" | rg -c ': test$')" -eq 2
test "$(printf '%s\n' "$FAVORITE_LIST" | rg -c ': test$')" -eq 1
test "$(printf '%s\n' "$IMPORT_LIST" | rg -c ': test$')" -eq 1
test "$(printf '%s\n' "$SAVE_CLIP_LIST" | rg -c ': test$')" -eq 1
test "$(printf '%s\n' "$CAPTURE_LIST" | rg -c ': test$')" -eq 1
test "$(printf '%s\n' "$FREEZE_LIST" | rg -c ': test$')" -eq 1
test "$(printf '%s\n' "$RANGE_LIST" | rg -c ': test$')" -eq 1
test "$(printf '%s\n' "$MCP_PATH_LIST" | rg -c ': test$')" -eq 1
test "$(printf '%s\n' "$MCP_LIST" | rg -c ': test$')" -eq 1
cargo test -p opentake-core --test schema_compat -- --nocapture
cargo test -p opentake-tauri --test schema_compat_integration -- --nocapture
cargo test -p opentake-tauri favorite_command_refuses_unknown_project_without_manifest_change --lib -- --nocapture
cargo test -p opentake-tauri import_commands_refuse_unknown_project_without_manifest_or_folder_change --lib -- --nocapture
cargo test -p opentake-tauri save_clip_as_media_refuses_before_media_output_creation --lib -- --nocapture
cargo test -p opentake-tauri capture_frame_to_media_refuses_before_capture_creation --lib -- --nocapture
cargo test -p opentake-tauri capture_freeze_frame_refuses_before_capture_creation --lib -- --nocapture
cargo test -p opentake-tauri export_range_refuses_before_save_output_creation --lib -- --nocapture
cargo test -p opentake-tauri mcp_path_import_refuses_unknown_project_without_manifest_or_folder_change --lib -- --nocapture
cargo test -p opentake-tauri mcp_bytes_import_refuses_before_media_tree_mutation --lib -- --nocapture
cargo test -p opentake-core
cargo test -p opentake-tauri --lib
cargo clippy -p opentake-core -p opentake-tauri --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: all fourteen new tests plus existing Core/Tauri tests pass.

- [ ] **Step 7: Commit the Core/Tauri compatibility slice**

```zsh
git add crates/opentake-core/src/session.rs crates/opentake-core/src/core.rs \
  crates/opentake-core/src/error.rs crates/opentake-core/src/dto.rs \
  crates/opentake-core/tests/schema_compat.rs src-tauri/src/export.rs \
  src-tauri/src/media.rs src-tauri/src/mcp.rs src-tauri/src/render.rs \
  src-tauri/tests/schema_compat_integration.rs \
  src-tauri/tests/bundle_export_integration.rs
git commit -m 'fix: enforce project compatibility read only mode'
```

- [ ] **Step 8: Pass the mandatory two-stage review gate**

Set `TASK=3` and `ATTEMPT=1`, execute the Mandatory Per-Slice Review Gate, fix every finding, and repeat until the latest exact SHA is approved at 0/0/0. Do not begin Task 4 earlier.

### Task 4: Surface Compatibility Read-Only In The Web App

**Files:**
- Modify: `web/src/lib/api.ts`
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/store/projectStore.ts`
- Modify: `web/src/store/projectActions.ts`
- Modify: `web/src/store/sync.ts`
- Modify: `web/src/lib/api.test.ts`
- Modify: `web/src/store/projectActions.test.ts`
- Modify: `web/src/store/sync.test.ts`
- Modify: `web/src/store/editActions.test.ts`
- Modify: `web/src/store/recentStore.ts`
- Create: `web/src/store/recentStore.schemaCompatibility.test.ts`
- Create: `web/src/store/projectStore.schemaCompatibility.test.ts`
- Create: `web/src/components/shell/CompatibilityBanner.tsx`
- Create: `web/src/components/shell/CompatibilityBanner.test.tsx`
- Modify: `web/src/App.tsx`
- Modify: `web/src/i18n/dict.ts`

- [ ] **Step 1: Write RED store and rendered-banner tests**

Store tests prove one `replaceProjectSnapshot` action atomically sets epoch, timeline/version, project path, `compatibilityReadOnly`, and blockers; a later known project atomically clears both compatibility fields. A recent-store test removes the active read-only project and proves `clearProjectSnapshot()` resets path, empty mirror, epoch/version/history, and compatibility fields together. Banner tests prove Chinese and English messages render, name the mode as read-only, explain that the project remains inspectable, and do not render for a writable project.

- [ ] **Step 2: Prove the three test files are RED**

```zsh
set -euo pipefail
pnpm --dir web exec vitest run \
  src/store/projectStore.schemaCompatibility.test.ts \
  src/store/recentStore.schemaCompatibility.test.ts \
  src/components/shell/CompatibilityBanner.test.tsx
```

Expected: the run fails because the snapshot action and banner do not exist.

- [ ] **Step 3: Implement the Web contract and banner**

Add to `RuntimeTimelineSnapshot` and the project store:

```ts
projectPath: string | null;
compatibilityReadOnly: boolean;
compatibilityBlockers: string[];
```

Every browser fallback snapshot in `web/src/lib/api.ts` supplies `projectPath: null`, `compatibilityReadOnly: false`, and an empty blocker array. Add `replaceProjectSnapshot(snapshot)` and `clearProjectSnapshot()` so an epoch/reset cannot retain an old path or compatibility state. Open and sync consume the snapshot path directly. The current new-project flow still receives a null path before its first save, so after `projectSave(path)` succeeds it must retain the existing explicit `setProjectPath(path)` step (or immediately fetch and replace with a fresh snapshot); never clear the chosen path by blindly replaying the pre-save snapshot. Recent removal uses `clearProjectSnapshot()` instead of piecemeal setters. Mount `CompatibilityBanner` above the active view in `App.tsx`; render nothing when writable. The banner shows a stable localized explanation and a collapsed blocker count, not raw filesystem paths.

Update every existing `getTimeline`, `projectOpen`, and `projectNew` Web test fixture to include those same known-schema defaults; do not make the new fields optional just to avoid fixture updates.

- [ ] **Step 4: Run GREEN Web checks**

```zsh
pnpm --dir web exec vitest run \
  src/store/projectStore.schemaCompatibility.test.ts \
  src/store/recentStore.schemaCompatibility.test.ts \
  src/components/shell/CompatibilityBanner.test.tsx
pnpm --dir web test
pnpm --dir web build
```

Expected: focused tests, full Web tests, TypeScript compilation, and production build pass.

- [ ] **Step 5: Commit the Web compatibility slice**

```zsh
git add web/src/lib/api.ts web/src/lib/types.ts web/src/store/projectStore.ts \
  web/src/store/projectActions.ts web/src/store/sync.ts \
  web/src/lib/api.test.ts web/src/store/projectActions.test.ts \
  web/src/store/sync.test.ts web/src/store/editActions.test.ts \
  web/src/store/recentStore.ts web/src/store/recentStore.schemaCompatibility.test.ts \
  web/src/store/projectStore.schemaCompatibility.test.ts \
  web/src/components/shell/CompatibilityBanner.tsx \
  web/src/components/shell/CompatibilityBanner.test.tsx \
  web/src/App.tsx web/src/i18n/dict.ts
git commit -m 'feat: show project compatibility read only mode'
```

- [ ] **Step 6: Pass the mandatory two-stage review gate**

Set `TASK=4` and `ATTEMPT=1`, execute the Mandatory Per-Slice Review Gate, fix every finding, and repeat until the latest exact SHA is approved at 0/0/0.

### Task 5: Branch Gate And Exact-Bundle Compatibility QA

**Files:**
- Create outside repository: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260711-wave1b-schema/qa/$QA_TIMESTAMP/results.md`
- Create outside repository: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260711-wave1b-schema/qa/$QA_TIMESTAMP/commands.md`
- Create outside repository: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260711-wave1b-schema/qa/$QA_TIMESTAMP/screenshots/`

- [ ] **Step 1: Run the branch-wide gate**

```zsh
set -euo pipefail
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
FINAL_SHA=$(git -C '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence' rev-parse HEAD)
CARGO_AUDIT_BIN='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/target/cargo-tools/bin/cargo-audit'
test "$(git -C "$REVIEW" rev-parse HEAD)" = "$FINAL_SHA"
test -z "$(git -C "$REVIEW" status --porcelain)"
cd "$REVIEW"
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo fmt --all -- --check
pnpm --dir web test
pnpm --dir web build
pnpm --dir web audit --audit-level high
"$CARGO_AUDIT_BIN" audit
```

Expected: zero failures. Any skipped hardware/environment probe is listed and not counted as a pass.

- [ ] **Step 2: Build and identify the exact desktop bundle**

```zsh
set -euo pipefail
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
FINAL_SHA=$(git -C '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence' rev-parse HEAD)
test "$(git -C "$REVIEW" rev-parse HEAD)" = "$FINAL_SHA"
test -z "$(git -C "$REVIEW" status --porcelain)"
cd "$REVIEW"
pnpm --dir web exec tauri build
```

Record exact Git SHA, binary SHA-256, complete app-tree digest, `codesign -dv --verbose=4`, and `otool -L`. Launch only `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review/target/release/bundle/macos/OpenTake.app` built at the reviewed SHA.

- [ ] **Step 3: Run real compatibility scenarios on copied projects**

Under a fresh external-disk QA root, create copies whose combined fixtures cover every Task 2 depth: timeline, track, clip, keyframe/value, effect, mask, manifest, folder, entry, source, generation input, generation-log top level, and generation-log entry. With Computer Use prove: open succeeds; banner remains visible; timeline/media remain inspectable; edit/import/relink/favorite/MCP path and bytes edits are rejected; save/save-as/self-contained bundle export visibly fail; source component hashes, `media/` tree, and project epoch remain unchanged; an existing export destination keeps the same recursive manifest. Then open a known-schema copy, edit, save, reopen, and prove normal persistence still works.

- [ ] **Step 4: Final independent full-slice review**

Dispatch a reviewer that implemented none of Tasks 2–4. It inspects the Wave 1A parent-to-head diff, all immutable per-slice reports, branch gate, exact app identity, and QA receipts. Completion requires `APPROVE`, Critical/Important/Minor 0/0/0, and clean integration/review worktrees.
