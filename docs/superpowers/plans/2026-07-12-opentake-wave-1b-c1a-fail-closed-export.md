# OpenTake Wave 1B-C1A Fail-Closed Bundle Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every intermediate branch state safe by removing the renderer-callable bundle exporter immediately and refusing any direct archive attempt that targets an existing path.

**Architecture:** First withdraw the Tauri command registration, Web API, and bundle-mode UI in one coherent, compiling commit while retaining the pure Rust exporter only for tests and later replacement. Then change the project crate from destructive replacement to typed new-destination-only refusal. The secure C1B–C1E implementation plans re-enable bundle export only after capability, disclosure, staging, CAS, and Rust-owned dialogs are complete in one integration commit.

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
- `web/src/lib/api.ts` / `api.test.ts` — remove bundle IPC surface and lock absence with a source-contract test.
- `web/src/components/shell/ExportDialog.tsx` / `.test.ts` — return to video-only export and remove all bundle destination/mode logic.
- `crates/opentake-project/src/error.rs` — typed `DestinationExists` refusal.
- `crates/opentake-project/src/archive.rs` — fail before mutation when any destination entry exists; never delete it.
- `crates/opentake-project/tests/archive_security.rs` — black-box byte-preservation regressions using the repository's dependency-free `TempDir`.

### Task 1: Withdraw The Unsafe Production Surface

**Files:**
- Create: `src-tauri/tests/bundle_export_surface.rs`
- Modify: `src-tauri/src/lib.rs:167-236`
- Modify: `src-tauri/src/export.rs:964-1001`
- Modify: `web/src/lib/api.ts:320-357`
- Modify: `web/src/lib/api.test.ts`
- Modify: `web/src/components/shell/ExportDialog.tsx:35-599`
- Modify: `web/src/components/shell/ExportDialog.test.ts:1-57`

- [ ] **Step 1: Add a RED Rust source-contract test**

Create `src-tauri/tests/bundle_export_surface.rs` exactly as follows:

```rust
const LIB_RS: &str = include_str!("../src/lib.rs");
const EXPORT_RS: &str = include_str!("../src/export.rs");

#[test]
fn bundle_export_is_not_registered_or_exposed_as_a_tauri_command() {
    assert!(!LIB_RS.contains("export::export_bundle,"));
    assert!(!EXPORT_RS.contains("#[tauri::command]\npub fn export_bundle"));
    assert!(!EXPORT_RS.contains("#[tauri::command]\npub async fn export_bundle"));
}
```

- [ ] **Step 2: Run the Rust test and record the intended RED result**

```bash
cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
```

Expected: FAIL because `export::export_bundle` is registered and the wrapper is a Tauri command.

- [ ] **Step 3: Add Web source-contract RED tests**

Add the `node:fs` import with the other imports at the top of `web/src/lib/api.test.ts`, then append the test:

```ts
import { readFileSync } from "node:fs";

it("does not expose renderer bundle export IPC during C1A", () => {
  const source = readFileSync(new URL("./api.ts", import.meta.url), "utf8");
  expect(source).not.toContain("export async function exportBundle");
  expect(source).not.toContain('invokeImpl<BundleReport>("export_bundle"');
});
```

In `ExportDialog.test.ts`, add the `node:fs` import at the top, remove `defaultBundleName` and `formatBytes` from the component import, delete their test blocks, then add:

```ts
import { readFileSync } from "node:fs";

it("contains no bundle mode or renderer bundle destination flow during C1A", () => {
  const source = readFileSync(new URL("./ExportDialog.tsx", import.meta.url), "utf8");
  expect(source).not.toContain('"bundle"');
  expect(source).not.toContain("onExportBundle");
  expect(source).not.toContain("defaultBundleName");
  expect(source).not.toContain("api.exportBundle");
});
```

- [ ] **Step 4: Run the Web tests and record RED**

```bash
pnpm -C web test -- src/lib/api.test.ts src/components/shell/ExportDialog.test.ts
```

Expected: FAIL because the API and component still contain the bundle surface.

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

Keep `BundleReportDto`, `MissingMediaDto`, and `run_bundle_export` unchanged so current Rust positive-path integration tests continue compiling while the safe replacement is built. Update `run_bundle_export`'s doc comment to say it is a non-command compatibility/test seam intentionally unreachable from the registered Tauri/Web surface during C1A.

- [ ] **Step 6: Remove the Web IPC surface**

Delete `MissingMedia`, `BundleReport`, and `exportBundle(outPath)` from `api.ts`. They have no remaining production consumer after the component change. Do not touch video, XML, EDL, OTIO, FCPXML, project-save, or media APIs.

- [ ] **Step 7: Reduce `ExportDialog` to video-only behavior**

Make these exact structural changes:

- delete `BUNDLE_EXT`, `ExportMode`, `defaultBundleName`, `formatBytes`, `mode`, `bundleMissing`, `modeOptions`, `onModeChange`, and `onExportBundle`;
- remove the mode dropdown row and bundle/missing conditional JSX;
- render codec/quality rows unconditionally;
- make Cancel always call `api.cancelExport()` only when video export is busy;
- make the action button always call `onExport`, use the existing video disabled rule, and use the existing video label/progress label.

The resulting relevant state and actions must be:

```ts
const [codec, setCodec] = useState<ExportCodec>("h264");
const [quality, setQuality] = useState<ExportQuality>("1080p");
const [busy, setBusy] = useState(false);
const [error, setError] = useState<string | null>(null);
const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);

async function onCancel(): Promise<void> {
  if (!busy) {
    setOpen(false);
    return;
  }
  await api.cancelExport();
}
```

Keep `formatBytes` deleted from tests/imports. Keep all video helper tests and video behavior unchanged.

- [ ] **Step 8: Run focused GREEN checks**

```bash
cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
cargo test -p opentake-tauri --test bundle_export_integration -- --test-threads=1
pnpm -C web test -- src/lib/api.test.ts src/components/shell/ExportDialog.test.ts src/components/shell/TitleBar.visual.test.ts
pnpm -C web exec tsc -b --pretty false
pnpm -C web build
cargo check --workspace --all-targets
cargo check -p opentake-tauri --no-default-features --all-targets
git diff --check
```

Expected: all pass. `run_bundle_export` remains testable from Rust, but the generated Tauri handler and Web code expose no bundle export route.

- [ ] **Step 9: Commit the fail-closed surface**

```bash
git add src-tauri/src/lib.rs src-tauri/src/export.rs src-tauri/tests/bundle_export_surface.rs web/src/lib/api.ts web/src/lib/api.test.ts web/src/components/shell/ExportDialog.tsx web/src/components/shell/ExportDialog.test.ts
git commit -m "fix: withdraw unsafe renderer bundle export"
```

- [ ] **Step 10: Run the exact-commit double review gate**

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

Expected: both tests compile but fail at `unwrap_err()` because current `archive()` removes and recreates the destination. Preserve this output in the task log.

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

Expected: all pass. Source/destination sentinels remain byte-identical, and the pure Rust positive export still succeeds only to a fresh path.

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
- Evidence root: set `GATE_TIMESTAMP=$(date +%Y%m%d-%H%M%S)` and `C1A_SHA=$(git rev-parse HEAD)`, then use `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/branch-gates/${GATE_TIMESTAMP}-${C1A_SHA}/`.

- [ ] **Step 1: Freeze exact clean trees**

```bash
C1A_SHA=$(git rev-parse HEAD)
test -z "$(git status --short)"
git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/OpenTake-wave1a-review merge --ff-only "$C1A_SHA"
test -z "$(git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/OpenTake-wave1a-review status --short)"
```

- [ ] **Step 2: Run complete current branch gates**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings
cargo test --workspace --all-targets -- --test-threads=1
pnpm -C web test
pnpm -C web build
pnpm -C web audit --audit-level high
git diff --check 31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace.."$C1A_SHA"
```

Expected: all exit 0. If an external advisory fetch fails, record the network error and retry once; never report an unrun audit as passed.

- [ ] **Step 3: Prove the production surface is fail closed**

```bash
! rg -n 'export::export_bundle,' src-tauri/src/lib.rs
! rg -n '#\[tauri::command\][[:space:]]*pub (async )?fn export_bundle' src-tauri/src/export.rs
! rg -n 'exportBundle|onExportBundle|defaultBundleName|"bundle"' web/src/lib/api.ts web/src/components/shell/ExportDialog.tsx
rg -n 'pub fn run_bundle_export' src-tauri/src/export.rs
```

Expected: the first three searches find nothing; the final search confirms only the non-command Rust seam remains for later secure integration.

- [ ] **Step 4: Dispatch final fresh C1A whole-slice auditors**

Dispatch one spec/security auditor and one quality/integration auditor. Both verify exact SHA/clean state, rerun the surface and archive-security tests, check video export remains reachable, and write `APPROVE 0/0/0`. Any finding creates a new fix commit and restarts Task 3 with both roles.

- [ ] **Step 5: Prepare the next executable plan before more product code**

Create and independently review `docs/superpowers/plans/2026-07-12-opentake-wave-1b-c1b-safe-filesystem.md`. Its scope is only capability interfaces plus complete Unix/macOS/Windows implementations and private unit fixtures; it must not re-enable the product bundle entry. C1A is complete after this handoff, while C1 and Wave 1B-C remain incomplete.

## Execution Mode

Use `superpowers:subagent-driven-development`: one fresh implementation agent per task, controller integration/verification, then the exact-commit independent review gates above. The user delegated execution choice and explicitly requires the review agents.
