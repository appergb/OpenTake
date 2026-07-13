# OpenTake Wave 1B-C1A Fail-Closed Bundle Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every intermediate branch state safe by removing the renderer-callable bundle exporter immediately and refusing any direct archive attempt that targets an existing path.

**Architecture:** First withdraw the Tauri command registration/wrapper and remove the bundle choice from the only UI mode selector in one coherent, compiling commit while retaining Rust/Web compatibility seams that can no longer reach a registered backend command. Then change the project crate from destructive replacement to typed new-destination-only refusal. The secure C1B–C1E implementation plans re-enable bundle export only after capability, disclosure, staging, CAS, and Rust-owned dialogs are complete in one integration commit.

**Tech Stack:** Rust 2021, Tauri 2, React 18, TypeScript 5.6, Vitest 4, Cargo workspace tests.

## Global Constraints

- The approved design is `docs/superpowers/specs/2026-07-12-opentake-wave-1b-c-filesystem-security-design.md` at exact commit `31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace`.
- C1A is fail-closed removal only. It does not complete C1 or Wave 1B-C and must not re-enable bundle export.
- No C1A command accepts a renderer-provided destination path or renderer approval token. The retained Web compatibility function must terminate at Tauri's unknown-command refusal.
- Bundle export remains disabled until Rust-owned native disclosure, Rust-owned save selection, source/destination capabilities, receipt-backed staging, no-replace publication, and revision CAS are integrated together under later approved plans.
- Every product and plan edit uses `apply_patch`; formatting tools may perform only mechanical rewrites after the semantic patch.
- Each task commit and the whole C1A slice require two fresh exact-commit reviewers: spec/security and quality/implementation. Both must report `APPROVE` with Critical/Important/Minor `0/0/0`; every finding is fixed and both roles repeat.

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

fn identifiers(source: &str) -> impl Iterator<Item = &str> {
    source
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| !token.is_empty())
}

#[test]
fn bundle_export_is_not_registered_or_exposed_as_a_tauri_command() {
    assert!(!identifiers(LIB_RS).any(|token| token == "export_bundle"));
    assert!(!identifiers(EXPORT_RS).any(|token| token == "export_bundle"));
}
```

- [ ] **Step 2: Run the Rust test and record the intended RED result**

```bash
cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
```

Expected: FAIL because both current files contain the standalone identifier
`export_bundle`. Splitting on every non-identifier character makes the
regression independent of whitespace, comments, `::` spacing, and line layout;
it does not confuse the retained identifier `run_bundle_export` with the
forbidden standalone identifier.

- [ ] **Step 3: Add the UI-entry RED test**

Insert this import before the existing Vitest import:

```ts
import { readFileSync } from "node:fs";
```

Then append this test inside the existing file without adding a second import:

```ts
it("offers no bundle mode while the secure native workflow is under construction", () => {
  const source = readFileSync(new URL("./ExportDialog.tsx", import.meta.url), "utf8");
  expect(source).not.toMatch(/\bid\s*:\s*["']bundle["']/);
});
```

- [ ] **Step 4: Run the Web tests and record RED**

```bash
pnpm -C web exec vitest run src/components/shell/ExportDialog.test.ts
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
pnpm -C web exec vitest run src/components/shell/ExportDialog.test.ts src/components/shell/TitleBar.visual.test.ts
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
- Evidence root: an exclusive `mktemp -d` child matching `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/branch-gates/${GATE_TIMESTAMP}-${C1A_SHA}-XXXXXX/`.
- Every ordinary gate gets a same-name `.log` containing stdout/stderr and a normalized `.exit`; raw statuses that need interpretation use `.raw-exit` as well. The dependency audit deliberately uses immutable `web-audit-attempt-1.log` / optional `web-audit-retry.log` plus normalized `web-audit.exit`, and `web-audit-disposition.txt` identifies which log authorized it.

- [ ] **Step 1: Create a fresh receipt root and freeze exact clean trees**

Run from the integration tree in zsh. The initial variable capture chooses the
directory; the recorded `integration-pre-head` gate independently proves its
value before any test or review begins.

```zsh
C1A_SHA=$(git rev-parse HEAD)
GATE_TIMESTAMP=$(date +%Y%m%d-%H%M%S)
EVIDENCE_PARENT="/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/branch-gates"
mkdir -p -- "$EVIDENCE_PARENT" || exit 1
EVIDENCE_DIR=$(mktemp -d "$EVIDENCE_PARENT/${GATE_TIMESTAMP}-${C1A_SHA}-XXXXXX") || exit 1
mkdir "$EVIDENCE_DIR/final-audit" || exit 1
print -r -- "created $EVIDENCE_DIR/final-audit" >"$EVIDENCE_DIR/evidence-dir-create.log"
print -r -- 0 >"$EVIDENCE_DIR/evidence-dir-create.exit"

record_gate() {
  local name="$1"
  shift
  "$@" >"$EVIDENCE_DIR/${name}.log" 2>&1
  local gate_status=$?
  print -r -- "$gate_status" >"$EVIDENCE_DIR/${name}.exit"
}

record_clean() {
  local name="$1"
  shift
  "$@" >"$EVIDENCE_DIR/${name}.log" 2>&1
  local raw_status=$?
  print -r -- "$raw_status" >"$EVIDENCE_DIR/${name}.raw-exit"
  if [[ $raw_status -eq 0 && ! -s "$EVIDENCE_DIR/${name}.log" ]]; then
    print -r -- 0 >"$EVIDENCE_DIR/${name}.exit"
  else
    print -r -- 1 >"$EVIDENCE_DIR/${name}.exit"
  fi
}

record_head() {
  local name="$1"
  shift
  "$@" >"$EVIDENCE_DIR/${name}.log" 2>&1
  local raw_status=$?
  print -r -- "$raw_status" >"$EVIDENCE_DIR/${name}.raw-exit"
  if [[ $raw_status -eq 0 && "$(<"$EVIDENCE_DIR/${name}.log")" == "$C1A_SHA" ]]; then
    print -r -- 0 >"$EVIDENCE_DIR/${name}.exit"
  else
    print -r -- 1 >"$EVIDENCE_DIR/${name}.exit"
  fi
}

record_head integration-pre-head git rev-parse HEAD
record_clean integration-pre-status git status --porcelain=v1
record_gate review-fast-forward git -C "/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review" merge --ff-only "$C1A_SHA"
record_head review-pre-head git -C "/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review" rev-parse HEAD
record_clean review-pre-status git -C "/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review" status --porcelain=v1
```

All five normalized `.exit` receipts above must be 0. `record_head` proves each
tree equals `C1A_SHA`; `record_clean` proves `git status` both executed
successfully and emitted no stdout/stderr. `mktemp -d` creates the evidence root
exclusively; a restart always receives a new random suffix and can never reuse
or overwrite an earlier attempt directory.

- [ ] **Step 2: Run complete current branch gates with an append-only audit retry**

Continue in the same zsh shell so `C1A_SHA`, `EVIDENCE_DIR`, and the helpers
remain defined:

```zsh
record_gate cargo-fmt cargo fmt --all --check
record_gate cargo-clippy-workspace cargo clippy --workspace --all-targets -- -D warnings
record_gate cargo-clippy-tauri-nodefault cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings
record_gate cargo-test-workspace cargo test --workspace --all-targets -- --test-threads=1
record_gate web-test pnpm -C web test
record_gate web-build pnpm -C web build
record_gate git-diff-check git diff --check "31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace..${C1A_SHA}"

pnpm -C web audit --audit-level high >"$EVIDENCE_DIR/web-audit-attempt-1.log" 2>&1
audit_status=$?
print -r -- "$audit_status" >"$EVIDENCE_DIR/web-audit-attempt-1.raw-exit"
if [[ $audit_status -eq 0 ]]; then
  print -r -- "attempt-1-pass" >"$EVIDENCE_DIR/web-audit-disposition.txt"
  print -r -- 0 >"$EVIDENCE_DIR/web-audit.exit"
elif rg -q 'ERR_PNPM_META_FETCH_FAIL|EAI_AGAIN|ENETUNREACH|ECONNRESET|ECONNREFUSED|ETIMEDOUT' "$EVIDENCE_DIR/web-audit-attempt-1.log"; then
  pnpm -C web audit --audit-level high >"$EVIDENCE_DIR/web-audit-retry.log" 2>&1
  retry_status=$?
  print -r -- "$retry_status" >"$EVIDENCE_DIR/web-audit-retry.raw-exit"
  print -r -- "network-failure-then-one-retry" >"$EVIDENCE_DIR/web-audit-disposition.txt"
  print -r -- "$retry_status" >"$EVIDENCE_DIR/web-audit.exit"
else
  print -r -- "non-network-failure-no-retry" >"$EVIDENCE_DIR/web-audit-disposition.txt"
  print -r -- "$audit_status" >"$EVIDENCE_DIR/web-audit.exit"
fi
```

Expected: every normalized `.exit` is 0. The first audit log is never
overwritten. Only the enumerated transport failures permit one distinctly
named retry; vulnerability/policy failures are not retried or normalized away.

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
  print -r -- "$raw_status" >"$EVIDENCE_DIR/${name}.raw-exit"
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
  print -r -- "$raw_status" >"$EVIDENCE_DIR/${name}.raw-exit"
  print -r -- "$raw_status" >"$EVIDENCE_DIR/${name}.exit"
}

record_absent no-bundle-handler '\bexport_bundle\b' src-tauri/src/lib.rs
record_absent no-bundle-command '\bfn[[:space:]]+export_bundle\b' src-tauri/src/export.rs
record_absent no-bundle-mode "\\bid[[:space:]]*:[[:space:]]*['\\\"]bundle['\\\"]" web/src/components/shell/ExportDialog.tsx
record_present rust-test-seam 'pub fn run_bundle_export' src-tauri/src/export.rs
record_gate tauri-surface-test cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
record_gate archive-security-test cargo test -p opentake-project --test archive_security -- --test-threads=1

record_head integration-post-head git rev-parse HEAD
record_clean integration-post-status git status --porcelain=v1
record_head review-post-head git -C "/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review" rev-parse HEAD
record_clean review-post-status git -C "/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review" status --porcelain=v1

gate_failed=0
{
  for status_file in "$EVIDENCE_DIR"/*.exit; do
    if [[ "$(<"$status_file")" != "0" ]]; then
      print -r -- "FAILED: $status_file"
      gate_failed=1
    fi
  done
  if [[ $gate_failed -eq 0 ]]; then
    print -r -- "PASS: all pre-audit branch gates"
  fi
} >"$EVIDENCE_DIR/pre-audit-aggregate.log" 2>&1
print -r -- "$gate_failed" >"$EVIDENCE_DIR/pre-audit-aggregate.exit"
[[ $gate_failed -eq 0 ]]
```

Expected: the first three single-line source invariants find nothing; the final
search confirms only the non-command Rust seam remains for later secure
integration. Capture each search's stdout/stderr and its expected status under
`$EVIDENCE_DIR`. A negative search passes only with raw `rg` status 1; status 2
is an execution error. Both post-gate trees must still equal `C1A_SHA` and be
clean. `pre-audit-aggregate.exit` must be 0 before auditors are dispatched.

- [ ] **Step 4: Dispatch final fresh C1A whole-slice auditors**

Dispatch one spec/security auditor and one quality/integration auditor. Both
verify exact SHA/clean state from the pre/post recorded files, inspect all `.exit` and
`.log` evidence, rerun the surface and archive-security tests, and check video
export remains reachable. The spec report must be written to
`$EVIDENCE_DIR/final-audit/spec-security-review.md`; the quality report must be
written to `$EVIDENCE_DIR/final-audit/quality-integration-review.md`. Each report
records reviewed SHA, commands rerun, verdict, and Critical/Important/Minor
counts using these exact header lines:

```text
Role: spec-security
Commit: <the literal 40-character C1A_SHA>
Verdict: APPROVE
Critical: 0
Important: 0
Minor: 0
```

The quality report uses `Role: quality-integration` and the same literal
40-character commit. `<the literal 40-character C1A_SHA>` is an instruction to
insert the value from the already-recorded and validated
`integration-post-head.log`; that angle-bracket text must not appear in either
report. The later `integration-final-head` and `review-final-head` gates
independently re-prove the same SHA after both reports land.

After both reports land, continue in the same zsh shell and validate the report
shape plus the final exact/clean trees:

```zsh
record_approval_report() {
  local name="$1"
  local report="$2"
  local expected_role="$3"
  : >"$EVIDENCE_DIR/${name}.log"
  local report_status=0
  local expected
  local count
  for expected in \
    "Role: ${expected_role}" \
    "Commit: ${C1A_SHA}" \
    'Verdict: APPROVE' \
    'Critical: 0' \
    'Important: 0' \
    'Minor: 0'; do
    count=$(rg -c -F -x -- "$expected" "$report" 2>>"$EVIDENCE_DIR/${name}.log")
    print -r -- "$expected => ${count:-0}" >>"$EVIDENCE_DIR/${name}.log"
    if [[ "$count" != "1" ]]; then
      report_status=1
    fi
  done
  local prefix_count
  for prefix in Role Commit Verdict Critical Important Minor; do
    prefix_count=$(rg -c -- "^${prefix}:" "$report" 2>>"$EVIDENCE_DIR/${name}.log")
    print -r -- "${prefix} header count => ${prefix_count:-0}" >>"$EVIDENCE_DIR/${name}.log"
    if [[ "$prefix_count" != "1" ]]; then
      report_status=1
    fi
  done
  print -r -- "$report_status" >"$EVIDENCE_DIR/${name}.exit"
}

record_approval_report spec-security-audit "$EVIDENCE_DIR/final-audit/spec-security-review.md" spec-security
record_approval_report quality-integration-audit "$EVIDENCE_DIR/final-audit/quality-integration-review.md" quality-integration
record_head integration-final-head git rev-parse HEAD
record_clean integration-final-status git status --porcelain=v1
record_head review-final-head git -C "/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review" rev-parse HEAD
record_clean review-final-status git -C "/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review" status --porcelain=v1

pre_results_failed=0
{
  for status_file in "$EVIDENCE_DIR"/*.exit; do
    if [[ "$(<"$status_file")" != "0" ]]; then
      print -r -- "FAILED: $status_file"
      pre_results_failed=1
    fi
  done
  if [[ $pre_results_failed -eq 0 ]]; then
    print -r -- "PASS: exact-commit gates and both audits are ready for results"
  fi
} >"$EVIDENCE_DIR/pre-results-aggregate.log" 2>&1
print -r -- "$pre_results_failed" >"$EVIDENCE_DIR/pre-results-aggregate.exit"
[[ $pre_results_failed -eq 0 ]]
```

Only after `pre-results-aggregate.exit` is 0, create `results.md` with
`apply_patch`. In the body below, replace every `RESOLVED_C1A_SHA` with the
literal 40-character contents of `integration-final-head.log`; the marker must
not remain in the file. If `web-audit-disposition.txt` is `attempt-1-pass`, use
the first dependency-audit bullet. If it is
`network-failure-then-one-retry`, use the second. No other body variation is
permitted.

```markdown
# C1A Branch Gate Result

- Overall: APPROVE
- Exact commit: RESOLVED_C1A_SHA
- Integration final SHA: RESOLVED_C1A_SHA
- Review final SHA: RESOLVED_C1A_SHA
- Integration clean status: 0
- Review clean status: 0
- Dependency audit: attempt 1 passed; see web-audit-attempt-1.log and web-audit-attempt-1.raw-exit.
- Dependency audit: attempt 1 had an enumerated transport failure and the single retry passed; see web-audit-attempt-1.log, web-audit-attempt-1.raw-exit, web-audit-retry.log, and web-audit-retry.raw-exit.
- Spec/security audit: final-audit/spec-security-review.md; Role spec-security; Commit RESOLVED_C1A_SHA; APPROVE 0/0/0.
- Quality/integration audit: final-audit/quality-integration-review.md; Role quality-integration; Commit RESOLVED_C1A_SHA; APPROVE 0/0/0.
- Scope: C1A fail-closed removal complete; C1 and Wave 1B-C remain incomplete.

| Normalized gate | Status |
| --- | ---: |
| evidence-dir-create | 0 |
| integration-pre-head | 0 |
| integration-pre-status | 0 |
| review-fast-forward | 0 |
| review-pre-head | 0 |
| review-pre-status | 0 |
| cargo-fmt | 0 |
| cargo-clippy-workspace | 0 |
| cargo-clippy-tauri-nodefault | 0 |
| cargo-test-workspace | 0 |
| web-test | 0 |
| web-build | 0 |
| git-diff-check | 0 |
| web-audit | 0 |
| no-bundle-handler | 0 |
| no-bundle-command | 0 |
| no-bundle-mode | 0 |
| rust-test-seam | 0 |
| tauri-surface-test | 0 |
| archive-security-test | 0 |
| integration-post-head | 0 |
| integration-post-status | 0 |
| review-post-head | 0 |
| review-post-status | 0 |
| pre-audit-aggregate | 0 |
| spec-security-audit | 0 |
| quality-integration-audit | 0 |
| integration-final-head | 0 |
| integration-final-status | 0 |
| review-final-head | 0 |
| review-final-status | 0 |
| pre-results-aggregate | 0 |
| results-validation | 0 |
| final-aggregate | 0 |
```

The two dependency-audit bullets above are mutually exclusive: the
`results.md` file contains exactly one of them. Validate the concrete receipt,
then compute the final aggregate:

```zsh
record_results() {
  local report="$EVIDENCE_DIR/results.md"
  local result_status=0
  : >"$EVIDENCE_DIR/results-validation.log"

  if rg -n -F -- 'RESOLVED_C1A_SHA' "$report" >>"$EVIDENCE_DIR/results-validation.log" 2>&1; then
    result_status=1
  fi

  local expected
  local count
  for expected in \
    '# C1A Branch Gate Result' \
    '- Overall: APPROVE' \
    "- Exact commit: ${C1A_SHA}" \
    "- Integration final SHA: ${C1A_SHA}" \
    "- Review final SHA: ${C1A_SHA}" \
    '- Integration clean status: 0' \
    '- Review clean status: 0' \
    "- Spec/security audit: final-audit/spec-security-review.md; Role spec-security; Commit ${C1A_SHA}; APPROVE 0/0/0." \
    "- Quality/integration audit: final-audit/quality-integration-review.md; Role quality-integration; Commit ${C1A_SHA}; APPROVE 0/0/0." \
    '- Scope: C1A fail-closed removal complete; C1 and Wave 1B-C remain incomplete.'; do
    count=$(rg -c -F -x -- "$expected" "$report" 2>>"$EVIDENCE_DIR/results-validation.log")
    print -r -- "$expected => ${count:-0}" >>"$EVIDENCE_DIR/results-validation.log"
    if [[ "$count" != "1" ]]; then
      result_status=1
    fi
  done

  local gate
  for gate in \
    evidence-dir-create integration-pre-head integration-pre-status \
    review-fast-forward review-pre-head review-pre-status cargo-fmt \
    cargo-clippy-workspace cargo-clippy-tauri-nodefault cargo-test-workspace \
    web-test web-build git-diff-check web-audit no-bundle-handler \
    no-bundle-command no-bundle-mode rust-test-seam tauri-surface-test \
    archive-security-test integration-post-head integration-post-status \
    review-post-head review-post-status pre-audit-aggregate \
    spec-security-audit quality-integration-audit integration-final-head \
    integration-final-status review-final-head review-final-status \
    pre-results-aggregate results-validation final-aggregate; do
    expected="| ${gate} | 0 |"
    count=$(rg -c -F -x -- "$expected" "$report" 2>>"$EVIDENCE_DIR/results-validation.log")
    print -r -- "$expected => ${count:-0}" >>"$EVIDENCE_DIR/results-validation.log"
    if [[ "$count" != "1" ]]; then
      result_status=1
    fi
  done
  local gate_row_count
  gate_row_count=$(rg -c -- '^\| [a-z][^|]* \| 0 \|$' "$report" 2>>"$EVIDENCE_DIR/results-validation.log")
  print -r -- "normalized zero gate rows => ${gate_row_count:-0}" >>"$EVIDENCE_DIR/results-validation.log"
  if [[ "$gate_row_count" != "34" ]]; then
    result_status=1
  fi
  if rg -n -- '^\| [a-z][^|]* \| [^|]*[1-9][^|]* \|$' "$report" >>"$EVIDENCE_DIR/results-validation.log" 2>&1; then
    result_status=1
  fi

  local disposition
  disposition="$(<"$EVIDENCE_DIR/web-audit-disposition.txt")"
  if [[ "$disposition" == "attempt-1-pass" ]]; then
    expected='- Dependency audit: attempt 1 passed; see web-audit-attempt-1.log and web-audit-attempt-1.raw-exit.'
  elif [[ "$disposition" == "network-failure-then-one-retry" ]]; then
    expected='- Dependency audit: attempt 1 had an enumerated transport failure and the single retry passed; see web-audit-attempt-1.log, web-audit-attempt-1.raw-exit, web-audit-retry.log, and web-audit-retry.raw-exit.'
  else
    expected='INVALID-AUDIT-DISPOSITION'
    result_status=1
  fi
  count=$(rg -c -F -x -- "$expected" "$report" 2>>"$EVIDENCE_DIR/results-validation.log")
  print -r -- "$expected => ${count:-0}" >>"$EVIDENCE_DIR/results-validation.log"
  if [[ "$count" != "1" ]]; then
    result_status=1
  fi

  local dependency_lines
  dependency_lines=$(rg -c -- '^- Dependency audit:' "$report" 2>>"$EVIDENCE_DIR/results-validation.log")
  if [[ "$dependency_lines" != "1" ]]; then
    result_status=1
  fi
  print -r -- "$result_status" >"$EVIDENCE_DIR/results-validation.exit"
}

record_results

final_failed=0
{
  for status_file in "$EVIDENCE_DIR"/*.exit; do
    if [[ "$(<"$status_file")" != "0" ]]; then
      print -r -- "FAILED: $status_file"
      final_failed=1
    fi
  done
  if [[ $final_failed -eq 0 ]]; then
    print -r -- "PASS: C1A exact-commit gate, results receipt, and both audits"
  fi
} >"$EVIDENCE_DIR/final-aggregate.log" 2>&1
print -r -- "$final_failed" >"$EVIDENCE_DIR/final-aggregate.exit"
[[ $final_failed -eq 0 ]]
```

The literal `final-aggregate | 0` row becomes true only when the last command
returns 0. A nonzero result prevents approval and preserves the failed receipt.

Any finding or nonzero normalized receipt creates a fix commit, a new evidence
directory, and a complete repeat of Task 3 with both fresh roles.

## Controller Handoff After C1A

After this plan is complete, the controller separately creates and independently
reviews `docs/superpowers/plans/2026-07-12-opentake-wave-1b-c1b-safe-filesystem.md`
under the `writing-plans` workflow. That separate plan owns capability
interfaces, complete Unix/macOS/Windows implementations, private unit fixtures,
its own commit/SHA/report paths/re-review loop, and must not re-enable the
product bundle entry. It is not an executable C1A task or a condition hidden
inside C1A's final receipt.

## Execution Mode

Use `superpowers:subagent-driven-development`: one fresh implementation agent per task, controller integration/verification, then the exact-commit independent review gates above. The user delegated execution choice and explicitly requires the review agents.
