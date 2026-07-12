# OpenTake Wave 1B-C1A Fail-Closed Bundle Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every intermediate branch state safe by removing the renderer-callable bundle exporter immediately and refusing any direct archive attempt that targets an existing path.

**Architecture:** First withdraw the Tauri command registration/wrapper and remove the bundle choice from the only UI mode selector in one coherent, compiling commit while retaining Rust/Web compatibility seams that can no longer reach a registered backend command. Then change the project crate from destructive replacement to typed new-destination-only refusal. The secure C1B–C1E implementation plans re-enable bundle export only after capability, disclosure, staging, CAS, and Rust-owned dialogs are complete in one integration commit.

**Tech Stack:** Rust 2021, Tauri 2, React 18, TypeScript 5.6, Vitest 4, Cargo workspace tests.

---

## Scope And Gates

- Approved design: `docs/superpowers/specs/2026-07-12-opentake-wave-1b-c-filesystem-security-design.md` at `31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace`.
- This plan deliberately removes a currently unsafe feature. It does not claim C1 completion and does not add temporary raw-path or renderer approval tokens.
- Every task ends in one focused commit. Before the next task begins, fast-forward the clean detached review tree to that exact commit and dispatch two fresh agents: spec/security and quality/implementation. Both reports must be `APPROVE` with Critical/Important/Minor `0/0/0`; every finding is fixed and both roles repeat.
- Product and plan edits use `apply_patch`. Formatting tools may make mechanical rewrites after the semantic patch.

## File Responsibilities

- `src-tauri/src/lib.rs` — remove bundle command registration so renderer `invoke` cannot reach the old path-taking exporter.
- `src-tauri/src/export.rs` — remove only the `#[tauri::command] export_bundle` wrapper; retain `run_bundle_export` as a non-command test seam until secure replacement lands.
- `src-tauri/tests/bundle_export_surface.rs` — source-contract regression proving no Tauri command/handler exposes bundle export during C1A.
- `web/src/components/shell/ExportDialog.tsx` / `.test.ts` — remove the bundle choice from the sole mode selector; retained dead compatibility code cannot reach a registered command.
- `crates/opentake-project/src/error.rs` — typed `DestinationExists` refusal.
- `crates/opentake-project/src/archive.rs` — fail before mutation when any destination entry exists; never delete it.
- `crates/opentake-project/tests/archive_security.rs` — black-box byte-preservation regressions using the repository's dependency-free `TempDir`.

### Task 1: Withdraw The Unsafe Production Surface

**Files:**
- Create: `src-tauri/tests/bundle_export_surface.rs`
- Modify: `src-tauri/src/lib.rs:167-236`
- Modify: `src-tauri/src/export.rs:964-1001`
- Modify: `web/src/components/shell/ExportDialog.tsx:186-193`
- Modify: `web/src/components/shell/ExportDialog.test.ts`

- [ ] **Step 1: Add a RED Rust source-contract test**

Create `src-tauri/tests/bundle_export_surface.rs` exactly as follows:

```rust
const LIB_RS: &str = include_str!("../src/lib.rs");
const EXPORT_RS: &str = include_str!("../src/export.rs");

#[test]
fn bundle_export_is_not_registered_or_exposed_as_a_tauri_command() {
    assert!(!LIB_RS.contains("export::export_bundle,"));
    assert!(!EXPORT_RS.contains("fn export_bundle"));
}
```

- [ ] **Step 2: Run the Rust test and record the intended RED result**

```bash
cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
```

Expected: FAIL because `export::export_bundle` is registered and the wrapper is a Tauri command.

- [ ] **Step 3: Add the UI-entry RED test**

Insert this import before the existing Vitest import:

```ts
import { readFileSync } from "node:fs";
```

Then append this test inside the existing file without adding a second import:

```ts
it("offers no bundle mode while the secure native workflow is under construction", () => {
  const source = readFileSync(new URL("./ExportDialog.tsx", import.meta.url), "utf8");
  expect(source).not.toContain('{ id: "bundle" as const');
});
```

- [ ] **Step 4: Run the Web tests and record RED**

```bash
pnpm -C web test -- src/components/shell/ExportDialog.test.ts
```

Expected: FAIL because the component's sole mode selector still offers bundle export.

- [ ] **Step 5: Remove the Tauri command surface but retain the pure Rust seam**

In `src-tauri/src/lib.rs`, delete only this handler entry:

```rust
export::export_bundle,
```

In `src-tauri/src/export.rs`, delete the complete wrapper:

```rust
#[tauri::command]
pub fn export_bundle(
    core: State<'_, AppCore>,
    out_path: String,
) -> Result<BundleReportDto, String> {
    let snapshot = core.bundle_export_snapshot();
    run_bundle_export(
        &snapshot.timeline,
        &snapshot.manifest,
        &snapshot.generation_log,
        snapshot.project_path.as_deref(),
        &snapshot.compatibility,
        out_path,
    )
}
```

Keep `BundleReportDto`, `MissingMediaDto`, and `run_bundle_export` so current Rust compatibility tests continue compiling while the safe replacement is built. Make these exact documentation changes:

1. Replace the complete comment immediately above `MissingMediaDto` with:

```rust
/// C1A missing-media compatibility DTO retained for Rust integration tests.
/// No registered Tauri command or Web UI entry exposes it while the secure
/// native workflow is under construction.
```

2. Replace the complete comment immediately above `BundleReportDto` with:

```rust
/// C1A bundle-report compatibility DTO retained for Rust integration tests.
/// No registered Tauri command or Web UI entry exposes it while the secure
/// native workflow is under construction.
```

3. Delete the complete obsolete command documentation beginning with
   ``/// `export_bundle`: write`` and ending with the `Err(String)` sentence,
   together with the wrapper shown above.

4. Replace the complete comment immediately above `run_bundle_export` with:

```rust
/// C1A non-command archive seam. It is public only for Rust integration tests;
/// the registered Tauri handler and UI entry are intentionally absent.
/// `source_bundle` remains optional so never-saved-project parity can be tested.
```

- [ ] **Step 6: Remove the bundle choice from the only UI selector**

Replace `modeOptions` with this exact block and leave all dead compatibility code unchanged until secure integration replaces it:

```ts
const modeOptions = useMemo(
  () => [
    // C1A fail closed: Rust-owned destination and disclosure are not integrated yet.
    { id: "video" as const, label: t("export.mode.video") },
  ],
  [t],
);
```

The component initializes `mode` to `"video"`; `Dropdown` can now emit only `"video"`. Even a compromised renderer calling the retained `api.exportBundle(outPath)` receives Tauri's unknown-command refusal because Step 5 removed the handler.

- [ ] **Step 7: Run focused GREEN checks**

```bash
cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
cargo test -p opentake-tauri --test bundle_export_integration -- --test-threads=1
pnpm -C web test -- src/components/shell/ExportDialog.test.ts src/components/shell/TitleBar.visual.test.ts
pnpm -C web exec tsc -b --pretty false
pnpm -C web build
cargo check --workspace --all-targets
cargo check -p opentake-tauri --no-default-features --all-targets
git diff --check
```

Expected: all pass. `run_bundle_export` remains testable from Rust, but no
registered Tauri handler exists and the Web UI offers no successful bundle
export route.

- [ ] **Step 8: Commit the fail-closed surface**

```bash
git add src-tauri/src/lib.rs src-tauri/src/export.rs src-tauri/tests/bundle_export_surface.rs web/src/components/shell/ExportDialog.tsx web/src/components/shell/ExportDialog.test.ts
git commit -m "fix: withdraw unsafe renderer bundle export"
```

- [ ] **Step 9: Run the exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, verify the integration tree is clean, fast-forward `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review` to `$SLICE_SHA`, and dispatch two fresh agents. The spec/security agent proves no renderer/Tauri path reaches `run_bundle_export`; the quality agent verifies video export and Rust test seams still compile. Reports are written under `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/logs/c1a-task-1-${SLICE_SHA}-attempt-1/` and both must state `APPROVE`, Critical 0, Important 0, Minor 0. Fix every finding in a new commit and repeat both roles.

### Task 2: Refuse Existing Archive Destinations Before Mutation

**Files:**
- Create: `crates/opentake-project/tests/archive_security.rs`
- Modify: `crates/opentake-project/src/error.rs`
- Modify: `crates/opentake-project/src/archive.rs:68-115`

- [ ] **Step 1: Create a complete RED integration test file with established fixtures**

Create `crates/opentake-project/tests/archive_security.rs`:

```rust
mod common;

use common::{write_file, TempDir};
use opentake_domain::{MediaManifest, Timeline};
use opentake_project::{archive, GenerationLog, ProjectError};

#[test]
fn archive_rejects_source_destination_without_mutation() {
    let tmp = TempDir::new("archive-source-destination");
    let source = tmp.child("Source.opentake");
    write_file(&source.join("sentinel"), b"source-bytes");

    let error = archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        Some(&source),
        &source,
    )
    .unwrap_err();

    assert!(matches!(error, ProjectError::DestinationExists { ref path } if path == &source));
    assert_eq!(std::fs::read(source.join("sentinel")).unwrap(), b"source-bytes");
}

#[test]
fn archive_rejects_existing_destination_without_mutation() {
    let tmp = TempDir::new("archive-existing-destination");
    let destination = tmp.child("Existing.opentake");
    write_file(&destination.join("sentinel"), b"destination-bytes");

    let error = archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        None,
        &destination,
    )
    .unwrap_err();

    assert!(matches!(error, ProjectError::DestinationExists { ref path } if path == &destination));
    assert_eq!(
        std::fs::read(destination.join("sentinel")).unwrap(),
        b"destination-bytes"
    );
}

#[test]
fn archive_rejects_existing_regular_file_without_mutation() {
    let tmp = TempDir::new("archive-existing-file");
    let destination = tmp.child("Existing.opentake");
    write_file(&destination, b"existing-file-bytes");

    let error = archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        None,
        &destination,
    )
    .unwrap_err();

    assert!(matches!(error, ProjectError::DestinationExists { ref path } if path == &destination));
    assert_eq!(std::fs::read(&destination).unwrap(), b"existing-file-bytes");
}

#[cfg(unix)]
#[test]
fn archive_rejects_dangling_destination_symlink_without_mutation() {
    use std::os::unix::fs::symlink;

    let tmp = TempDir::new("archive-dangling-symlink");
    let destination = tmp.child("Existing.opentake");
    let missing_target = tmp.child("missing-target");
    symlink(&missing_target, &destination).unwrap();

    let error = archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        None,
        &destination,
    )
    .unwrap_err();

    assert!(matches!(error, ProjectError::DestinationExists { ref path } if path == &destination));
    assert_eq!(std::fs::read_link(&destination).unwrap(), missing_target);
    assert!(std::fs::symlink_metadata(&destination)
        .unwrap()
        .file_type()
        .is_symlink());
}
```

- [ ] **Step 2: Run the first RED compile check**

```bash
cargo test -p opentake-project --test archive_security -- --test-threads=1
```

Expected: compile FAIL because `ProjectError::DestinationExists` does not exist.

- [ ] **Step 3: Add the typed error variant**

In `crates/opentake-project/src/error.rs` add:

```rust
#[error("bundle export destination already exists: {path}")]
DestinationExists {
    path: PathBuf,
},
```

- [ ] **Step 4: Run the behavioral RED check**

```bash
cargo test -p opentake-project --test archive_security -- --test-threads=1
```

Expected: the directory cases fail at `unwrap_err()` because current `archive()` removes and recreates the destination; the regular-file and dangling-symlink cases fail their typed-error assertion because the current implementation returns an IO error. Preserve this output in the task log.

- [ ] **Step 5: Replace destination deletion with fail-closed metadata handling**

At the beginning of `archive()`, delete the entire `dest_bundle.exists()` / `remove_dir_all` block and insert:

```rust
match std::fs::symlink_metadata(dest_bundle) {
    Ok(_) => {
        return Err(ProjectError::DestinationExists {
            path: dest_bundle.to_path_buf(),
        });
    }
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
    Err(error) => return Err(ProjectError::io(dest_bundle, error)),
}
```

This treats files, directories, and symlinks as existing and propagates permission/metadata errors. It performs no source resolution or output creation before this check.

Replace the complete destination-contract paragraph above `archive()` with:

```rust
/// `dest_bundle` must not exist. Any existing file, directory, or symlink
/// returns [`ProjectError::DestinationExists`] before source resolution or
/// output mutation.
```

Delete the obsolete `// Match upstream's "remove then land" semantics` comment
through `// function's doc contract.` together with the deletion block. The
new metadata check must be the first executable statement in `archive()`.

- [ ] **Step 6: Run focused and workspace GREEN checks**

```bash
cargo test -p opentake-project --test archive_security -- --test-threads=1
cargo test -p opentake-project --test archive -- --test-threads=1
cargo test -p opentake-tauri --test bundle_export_integration -- --test-threads=1
cargo fmt --all --check
cargo clippy -p opentake-project --all-targets -- -D warnings
cargo check --workspace --all-targets
cargo check -p opentake-tauri --no-default-features --all-targets
git diff --check
```

Expected: all pass. Source/destination sentinels, the existing regular file,
and the dangling symlink remain byte-identical or link-identical; the pure Rust
positive export still succeeds only to a fresh path. The Unix symlink fixture
exercises the macOS release host in C1A; C1B owns native Windows reparse-point
fixtures before the feature can be re-enabled on Windows.

- [ ] **Step 7: Commit the no-delete guard**

```bash
git add crates/opentake-project/src/error.rs crates/opentake-project/src/archive.rs crates/opentake-project/tests/archive_security.rs
git commit -m "fix: refuse existing bundle export destinations"
```

- [ ] **Step 8: Run the exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, fast-forward the clean detached review tree, and dispatch fresh spec/security and quality agents. The first checks every existing destination type and source byte preservation; the second checks upstream fresh-path parity and workspace integration. Reports go under `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/logs/c1a-task-2-${SLICE_SHA}-attempt-1/` and must both be `APPROVE 0/0/0`. Fix findings and repeat both roles before C1A closure.

### Task 3: C1A Branch Gate And Handoff

**Files:**
- Verify only; no product edit unless a gate exposes a defect.
- Evidence root: set `GATE_TIMESTAMP=$(date +%Y%m%d-%H%M%S)` and `C1A_SHA=$(git rev-parse HEAD)`, then use `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/branch-gates/${GATE_TIMESTAMP}-${C1A_SHA}/`. Every command gets a `.log` file containing stdout/stderr and an `.exit` file containing its numeric status.

- [ ] **Step 1: Freeze exact clean trees**

```bash
C1A_SHA=$(git rev-parse HEAD)
test -z "$(git status --short)"
git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/OpenTake-wave1a-review merge --ff-only "$C1A_SHA"
test -z "$(git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/OpenTake-wave1a-review status --short)"
```

Create the evidence directory, record both exact SHAs and both porcelain
statuses, and do not reuse an earlier directory:

```bash
GATE_TIMESTAMP=$(date +%Y%m%d-%H%M%S)
EVIDENCE_DIR="/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/branch-gates/${GATE_TIMESTAMP}-${C1A_SHA}"
mkdir -p "$EVIDENCE_DIR/final-audit"
git rev-parse HEAD >"$EVIDENCE_DIR/integration-head.txt"
git status --porcelain=v1 >"$EVIDENCE_DIR/integration-status.txt"
git -C "/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review" rev-parse HEAD >"$EVIDENCE_DIR/review-head.txt"
git -C "/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review" status --porcelain=v1 >"$EVIDENCE_DIR/review-status.txt"
```

- [ ] **Step 2: Run complete current branch gates**

Use this exact zsh helper in the integration tree. It deliberately records all
statuses instead of aborting after the first failure:

```zsh
record_gate() {
  local name="$1"
  shift
  "$@" >"$EVIDENCE_DIR/${name}.log" 2>&1
  local status=$?
  print -r -- "$status" >"$EVIDENCE_DIR/${name}.exit"
}

record_gate cargo-fmt cargo fmt --all --check
record_gate cargo-clippy-workspace cargo clippy --workspace --all-targets -- -D warnings
record_gate cargo-clippy-tauri-nodefault cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings
record_gate cargo-test-workspace cargo test --workspace --all-targets -- --test-threads=1
record_gate web-test pnpm -C web test
record_gate web-build pnpm -C web build
record_gate web-audit pnpm -C web audit --audit-level high
record_gate git-diff-check git diff --check "31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace..${C1A_SHA}"
```

Expected: all exit 0. If an external advisory fetch fails, record the network error and retry once; never report an unrun audit as passed.

- [ ] **Step 3: Prove the production surface is fail closed**

Use these exact zsh helpers so an absent search is distinguished from an `rg`
execution error while every raw status remains durable:

```zsh
record_absent() {
  local name="$1"
  local pattern="$2"
  local file="$3"
  rg -n -- "$pattern" "$file" >"$EVIDENCE_DIR/${name}.log" 2>&1
  local raw_status=$?
  print -r -- "$raw_status" >"$EVIDENCE_DIR/${name}.rg-exit"
  if [[ $raw_status -eq 1 ]]; then
    print -r -- 0 >"$EVIDENCE_DIR/${name}.exit"
  else
    print -r -- 1 >"$EVIDENCE_DIR/${name}.exit"
  fi
}

record_present() {
  local name="$1"
  local pattern="$2"
  local file="$3"
  rg -n -- "$pattern" "$file" >"$EVIDENCE_DIR/${name}.log" 2>&1
  local raw_status=$?
  print -r -- "$raw_status" >"$EVIDENCE_DIR/${name}.rg-exit"
  print -r -- "$raw_status" >"$EVIDENCE_DIR/${name}.exit"
}

record_absent no-bundle-handler 'export::export_bundle,' src-tauri/src/lib.rs
record_absent no-bundle-command '\bfn[[:space:]]+export_bundle\b' src-tauri/src/export.rs
record_absent no-bundle-mode '\{ id: "bundle" as const' web/src/components/shell/ExportDialog.tsx
record_present rust-test-seam 'pub fn run_bundle_export' src-tauri/src/export.rs
record_gate tauri-surface-test cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
record_gate archive-security-test cargo test -p opentake-project --test archive_security -- --test-threads=1

gate_failed=0
for status_file in "$EVIDENCE_DIR"/*.exit; do
  if [[ "$(<"$status_file")" != "0" ]]; then
    print -r -- "FAILED: $status_file"
    gate_failed=1
  fi
done
(( gate_failed == 0 ))
```

Expected: the first three single-line source invariants find nothing; the final
search confirms only the non-command Rust seam remains for later secure
integration. Capture each search's stdout/stderr and its expected status under
`$EVIDENCE_DIR`. A negative search passes only with raw `rg` status 1; status 2
is an execution error. The final aggregate command must exit 0 before audit.

- [ ] **Step 4: Dispatch final fresh C1A whole-slice auditors**

Dispatch one spec/security auditor and one quality/integration auditor. Both
verify exact SHA/clean state from the recorded files, inspect all `.exit` and
`.log` evidence, rerun the surface and archive-security tests, and check video
export remains reachable. The spec report must be written to
`$EVIDENCE_DIR/final-audit/spec-security-review.md`; the quality report must be
written to `$EVIDENCE_DIR/final-audit/quality-integration-review.md`. Each report
records reviewed SHA, commands rerun, verdict, and Critical/Important/Minor
counts, and must state `APPROVE` with `0/0/0`. After both reports land, record
their paths and verdicts in `$EVIDENCE_DIR/results.md` with `apply_patch`. Any
finding creates a new fix commit, a new evidence directory, and restarts Task 3
with both fresh roles.

- [ ] **Step 5: Prepare the next executable plan before more product code**

Create and independently review `docs/superpowers/plans/2026-07-12-opentake-wave-1b-c1b-safe-filesystem.md`. Its scope is only capability interfaces plus complete Unix/macOS/Windows implementations and private unit fixtures; it must not re-enable the product bundle entry. C1A is complete after this handoff, while C1 and Wave 1B-C remain incomplete.

## Execution Mode

Use `superpowers:subagent-driven-development`: one fresh implementation agent per task, controller integration/verification, then the exact-commit independent review gates above. The user delegated execution choice and explicitly requires the review agents.
