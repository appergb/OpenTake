# OpenTake Wave 1B-C1 Secure Bundle Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the destructive renderer-authorized bundle exporter with a Rust-authorized, capability-relative, staged, validated, atomic no-replace export that preserves source data and never publishes stale or undisclosed content.

**Architecture:** Land an emergency no-delete guard first, then introduce a whole-bundle revision in `AppCore`, platform `safe_fs` capabilities, immutable source/disclosure plans, a receipt-backed stage, and one lock-scoped final CAS/publish operation. Tauri owns the external-source disclosure and save panel; Web only requests export and renders typed results. C2 Save-As reuses these primitives later but is not implemented by this plan.

**Tech Stack:** Rust 2021, `rustix 1.1`, `windows-sys 0.61`, `sha2 0.10`, Tauri 2 / `tauri-plugin-dialog 2.7`, AppKit / Win32 common controls / GTK 3 native adapters, React 18, TypeScript 5.6, Vitest 4, GitHub Actions.

---

## Approved Inputs And Non-Negotiable Gates

- Approved design: `docs/superpowers/specs/2026-07-12-opentake-wave-1b-c-filesystem-security-design.md` at `31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace`.
- C1 never overwrites an existing destination. `DestinationExists` is a refusal, not a prompt to replace.
- The renderer never supplies the bundle destination or external-source approval.
- The existing lexical `standardize()` behavior remains the dedup key; it is never used as a security decision.
- Every task ends in one focused commit. Before the next task starts, fast-forward the clean detached review tree to that exact commit and dispatch two fresh agents: one spec/security reviewer and one quality/implementation reviewer. Each report must state exact commit, `APPROVE`, and Critical/Important/Minor `0/0/0`. Fix every finding in a new commit and repeat both reviews.
- Review reports use `TASK_NAME`, `SLICE_SHA`, and `ATTEMPT` shell variables to create `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/logs/c1-${TASK_NAME}-${SLICE_SHA}-attempt-${ATTEMPT}/` and are created with `apply_patch`.
- Product code and documentation edits use `apply_patch`. Formatting tools may rewrite mechanically after the semantic patch.

## File Responsibility Map

**Create:**

- `crates/opentake-project/src/safe_fs/mod.rs` — opaque capability and stable-identity interfaces, platform dispatch, error-normalization contract.
- `crates/opentake-project/src/safe_fs/unix.rs` — global-root/source/destination anchors, nofollow traversal, receipt IO, Linux/macOS no-replace rename.
- `crates/opentake-project/src/safe_fs/windows.rs` — volume mapping, reparse rejection, handle identity, Win32 no-replace rename.
- `crates/opentake-project/src/safe_fs/unsupported.rs` — fail-closed adapter for unsupported targets.
- `crates/opentake-project/src/archive/source_plan.rs` — project/external/aux source validation, bounded disclosure and copy plans.
- `crates/opentake-project/src/archive/path_encoding.rs` — raw Unix-byte / Windows UTF-16-unit reversible ASCII disclosure encoding and safe missing placeholder names.
- `crates/opentake-project/src/archive/staging.rs` — `StageGuard`, `BuildReceipt`, stage build, final validation, cleanup, publication.
- `crates/opentake-project/tests/archive_security.rs` — traversal, symlink, hard-link, special-file, disclosure, source-namespace regression tests.
- `crates/opentake-project/tests/archive_publication.rs` — destination, stage, receipt, concurrent-create, no-replace tests.
- `crates/opentake-project/tests/archive_races.rs` — deterministic check/open, namespace, source and stage seam tests.
- `crates/opentake-core/src/core.rs` test module — export-input revision/CAS regression tests using its existing private session fixtures.
- `src-tauri/src/export/bundle.rs` — Rust-owned C1 orchestration, destination defaults, typed IPC DTO mapping.
- `src-tauri/src/export/native_disclosure/mod.rs` — Rust-owned disclosure model adapter and test seam.
- `src-tauri/src/export/native_disclosure/macos.rs` — AppKit table implementation.
- `src-tauri/src/export/native_disclosure/windows.rs` — Win32 list-view implementation.
- `src-tauri/src/export/native_disclosure/linux.rs` — GTK scrolled tree implementation.
- `src-tauri/tests/bundle_export_security.rs` — dialog/CAS/cancel/error workflow integration tests.

**Modify:**

- `Cargo.toml`, `Cargo.lock`, `crates/opentake-project/Cargo.toml` — secure filesystem and digest dependencies.
- `crates/opentake-project/src/archive.rs` — public preparation/build/report orchestration and retained upstream naming/dedup helpers.
- `crates/opentake-project/src/error.rs`, `src/lib.rs` — typed errors and opaque public C1 surface.
- `crates/opentake-project/tests/archive.rs` — update missing-path privacy and new-destination semantics while retaining upstream positive parity.
- `crates/opentake-core/src/core.rs`, `src/lib.rs` — `BundleIdentity`, monotonic bundle revision, pre-write comparison and final CAS/publish.
- `crates/opentake-core/src/session.rs` — report real manifest/log/path mutations needed to advance bundle revision.
- `src-tauri/Cargo.toml`, `src-tauri/src/export.rs`, `src-tauri/src/lib.rs` — bundle submodule, native dependencies and pathless async command.
- `src-tauri/tests/bundle_export_integration.rs`, `src-tauri/tests/schema_compat_integration.rs` — fake-dialog workflow and typed refusal coverage.
- `web/src/lib/api.ts`, `web/src/lib/api.test.ts` — pathless nullable API and strict typed error decoder.
- `web/src/store/uiStore.ts`, `web/src/store/uiStore.test.ts` — explicit export dialog mode.
- `web/src/components/shell/TitleBar.tsx`, `TitleBar.visual.test.ts` — independent empty-timeline bundle entry.
- `web/src/components/shell/ExportDialog.tsx`, `ExportDialog.test.ts` — remove bundle save panel/path construction; render cancel/results/typed errors.
- `web/src/i18n/dict.ts` — Chinese/English stable error-code messages.
- `.github/workflows/ci.yml` — Linux/macOS/Windows native C1 compile and race gates.

### Task 1: Emergency No-Delete Guard And Safe Default

**Files:**
- Create: `crates/opentake-project/tests/archive_security.rs`
- Modify: `crates/opentake-project/src/error.rs`
- Modify: `crates/opentake-project/src/archive.rs:68-115`
- Modify: `web/src/components/shell/ExportDialog.tsx:77-87`
- Test: `crates/opentake-project/tests/archive_security.rs`
- Test: `web/src/components/shell/ExportDialog.test.ts`

- [ ] **Step 1: Write the source-equality and existing-destination RED tests**

```rust
#[test]
fn archive_rejects_source_destination_without_mutation() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("Source.opentake");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("sentinel"), b"internal").unwrap();

    let error = archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        Some(&source),
        &source,
    )
    .unwrap_err();

    assert!(matches!(error, ProjectError::DestinationExists { .. }));
    assert_eq!(std::fs::read(source.join("sentinel")).unwrap(), b"internal");
}

#[test]
fn archive_rejects_existing_destination_without_mutation() {
    let tmp = tempfile::tempdir().unwrap();
    let destination = tmp.path().join("Existing.opentake");
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(destination.join("sentinel"), b"keep").unwrap();

    let error = archive(
        &Timeline::new(),
        &MediaManifest::new(),
        &GenerationLog::new(),
        None,
        &destination,
    )
    .unwrap_err();

    assert!(matches!(error, ProjectError::DestinationExists { .. }));
    assert_eq!(std::fs::read(destination.join("sentinel")).unwrap(), b"keep");
}
```

- [ ] **Step 2: Run the focused Rust tests and record RED evidence**

Run:

```bash
cargo test -p opentake-project --test archive_security -- --test-threads=1
```

Expected: FAIL because `ProjectError::DestinationExists` does not exist and current `archive()` deletes the destination.

- [ ] **Step 3: Add the typed refusal and remove destructive replacement**

```rust
#[error("bundle export destination already exists: {path}")]
DestinationExists { path: PathBuf },
```

At the top of `archive()` replace the current `remove_dir_all` block with:

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

- [ ] **Step 4: Change the temporary renderer default away from the source**

```ts
export function defaultBundleName(projectPath: string | null): string {
  if (!projectPath) return `Untitled-export.${BUNDLE_EXT}`;
  const base = projectPath.split(/[\\/]/).pop() ?? projectPath;
  const stem = base.replace(/\.opentake$/i, "") || "Untitled";
  return `${stem}-export.${BUNDLE_EXT}`;
}
```

Update the existing Vitest expectation so saved `My Film.opentake` yields `My Film-export.opentake` and unsaved yields `Untitled-export.opentake`.

- [ ] **Step 5: Run focused GREEN checks**

```bash
cargo test -p opentake-project --test archive_security -- --test-threads=1
cargo test -p opentake-project --test archive -- --test-threads=1
pnpm -C web test -- src/components/shell/ExportDialog.test.ts
git diff --check
```

Expected: all pass; existing destination and source manifests remain byte-identical.

- [ ] **Step 6: Commit the emergency guard**

```bash
git add crates/opentake-project/src/error.rs crates/opentake-project/src/archive.rs crates/opentake-project/tests/archive_security.rs web/src/components/shell/ExportDialog.tsx web/src/components/shell/ExportDialog.test.ts
git commit -m "fix: refuse destructive bundle export destinations"
```

- [ ] **Step 7: Run the mandatory exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, fast-forward the clean detached review tree to `$SLICE_SHA`, and dispatch fresh spec/security and quality agents. Both must review Task 1 against `31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace`, write reports under `c1-task-1-${SLICE_SHA}-attempt-1`, and return `APPROVE 0/0/0`. Any finding is fixed in a new commit and both roles repeat.

### Task 2: Whole-Bundle Revision And Identity CAS

**Files:**
- Modify: `crates/opentake-core/src/core.rs:98-136,227-264,294-397,423-496`
- Modify: `crates/opentake-core/src/error.rs`
- Modify: `crates/opentake-core/src/session.rs:245-419`
- Modify: `crates/opentake-core/src/lib.rs`
- Test: `crates/opentake-core/src/core.rs`

- [ ] **Step 1: Write RED tests for timeline, media, log and path identity changes**

```rust
use opentake_project::GenerationLogEntry;

#[test]
fn every_bundle_input_change_advances_bundle_revision() {
    let core = core_with_track();
    let start = core.bundle_identity();

    core.apply(add_one_clip()).unwrap();
    let timeline = core.bundle_identity();
    assert!(timeline.bundle_revision > start.bundle_revision);

    core.import_media_file("/tmp/a.mp4", "a", &ProbedMedia::default()).unwrap();
    let media = core.bundle_identity();
    assert!(media.bundle_revision > timeline.bundle_revision);

    core.append_generation_log_entry(GenerationLogEntry::new(
        "row-1", "test-model", None, None,
    )).unwrap();
    let log = core.bundle_identity();
    assert!(log.bundle_revision > media.bundle_revision);
}

#[test]
fn compare_bundle_identity_checks_epoch_revision_and_path_together() {
    let core = AppCore::new();
    let expected = core.bundle_identity();
    core.import_media_file("/tmp/a.mp4", "a", &ProbedMedia::default()).unwrap();
    assert_eq!(core.compare_bundle_identity(&expected), Err(CoreError::ProjectChanged));
}
```

- [ ] **Step 2: Run the new test and capture the missing-symbol failures**

```bash
cargo test -p opentake-core core::tests::every_bundle_input_change_advances_bundle_revision -- --exact
cargo test -p opentake-core core::tests::compare_bundle_identity_checks_epoch_revision_and_path_together -- --exact
```

Expected: FAIL because `BundleIdentity`, `bundle_revision`, the comparison API and log mutator are not defined.

- [ ] **Step 3: Add the identity types and one-lock snapshot**

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleIdentity {
    pub project_epoch: u64,
    pub bundle_revision: u64,
    pub project_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct BundleExportSnapshot {
    pub timeline: Timeline,
    pub manifest: MediaManifest,
    pub generation_log: GenerationLog,
    pub compatibility: ProjectCompatibility,
    pub identity: BundleIdentity,
}

struct CoreSessionSlot {
    project_epoch: u64,
    bundle_revision: u64,
    editor: EditorSession,
}
```

Add `CoreError::ProjectChanged` with the stable message `project changed during bundle authorization`.

Implement `CoreSessionSlot::bundle_identity()` and make `bundle_export_snapshot()` copy the identity under the same mutex guard as every export input.

- [ ] **Step 4: Advance only on real successful export-input mutations**

Use the result's changed/version signal for timeline commands; compare manifest state around import/relink where the current API does not expose a changed bit; use `changed > 0` for favorites; increment on project-path change; increment after a generation-log append.

```rust
fn advance_bundle_revision(&mut self) {
    self.bundle_revision = self.bundle_revision.wrapping_add(1);
}

pub fn compare_bundle_identity(&self, expected: &BundleIdentity) -> Result<()> {
    let session = self.lock();
    if session.bundle_identity() == *expected {
        Ok(())
    } else {
        Err(CoreError::ProjectChanged)
    }
}
```

- [ ] **Step 5: Add the generation-log mutation wrapper**

```rust
pub fn append_generation_log_entry(&self, entry: GenerationLogEntry) -> Result<()> {
    let mut session = self.lock();
    session.editor.append_generation_log_entry(entry);
    session.advance_bundle_revision();
    Ok(())
}
```

The `EditorSession` method pushes exactly one entry and has no filesystem side effect.

- [ ] **Step 6: Run focused and regression tests**

```bash
cargo test -p opentake-core core::tests::every_bundle_input_change_advances_bundle_revision -- --exact
cargo test -p opentake-core core::tests::compare_bundle_identity_checks_epoch_revision_and_path_together -- --exact
cargo test -p opentake-core --test schema_compat -- --test-threads=1
cargo test -p opentake-core core::tests -- --test-threads=1
cargo clippy -p opentake-core --all-targets -- -D warnings
```

Expected: all pass; unchanged/no-op operations do not advance the revision, while every exported-input change does.

- [ ] **Step 7: Commit the bundle identity slice**

```bash
git add crates/opentake-core/src/core.rs crates/opentake-core/src/error.rs crates/opentake-core/src/session.rs crates/opentake-core/src/lib.rs
git commit -m "feat: track whole-bundle export revisions"
```

- [ ] **Step 8: Run the mandatory exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, fast-forward the clean detached review tree, and dispatch fresh spec/concurrency and quality agents. Require exact Task 2 reports at `c1-task-2-${SLICE_SHA}-attempt-1` with `APPROVE 0/0/0`; fix and re-run both roles before Task 3.

### Task 3: Capability And Stable-Identity Foundation

**Files:**
- Create: `crates/opentake-project/src/safe_fs/mod.rs`
- Create: `crates/opentake-project/src/safe_fs/unix.rs`
- Create: `crates/opentake-project/src/safe_fs/windows.rs`
- Create: `crates/opentake-project/src/safe_fs/unsupported.rs`
- Modify: `crates/opentake-project/Cargo.toml`
- Modify: `crates/opentake-project/src/lib.rs`
- Test: `crates/opentake-project/tests/archive_races.rs`

- [ ] **Step 1: Add target dependencies**

```toml
[dependencies]
sha2 = "0.10"
hex = "0.4"

[target.'cfg(unix)'.dependencies]
rustix = { version = "1", features = ["fs"] }

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61", features = [
  "Win32_Foundation",
  "Win32_Security",
  "Win32_Storage_FileSystem",
  "Win32_System_IO",
] }
```

Run `cargo check -p opentake-project` to update `Cargo.lock` with the already resolved versions.

- [ ] **Step 2: Write RED unit tests for path components, namespace mapping and nofollow identity**

```rust
#[test]
fn normal_relative_path_rejects_parent_root_and_prefix() {
    assert!(NormalRelativePath::new(Path::new("media/a.mov")).is_ok());
    assert!(NormalRelativePath::new(Path::new("../secret")).is_err());
    assert!(NormalRelativePath::new(Path::new("/tmp/secret")).is_err());
}

#[test]
fn namespace_anchor_rejects_injected_mapping_change() {
    let fs = FakeSecureFs::with_mapping_change_after_capture();
    let anchor = fs.capture_namespace("/Volumes/work/out").unwrap();
    assert_eq!(fs.revalidate_namespace(&anchor), Err(ProjectError::DestinationNamespaceChanged));
}

#[test]
fn destination_policy_rejects_relative_suffix_existing_and_overlap() {
    let source = fixture_source_bundle();
    for candidate in [
        PathBuf::from("relative.opentake"),
        source.parent().unwrap().join("wrong.OPENTAKE"),
        source.clone(),
        source.join("nested.opentake"),
        source.parent().unwrap().to_path_buf(),
    ] {
        assert!(capture_destination_for_source(&candidate, Some(&source)).is_err());
    }
}
```

- [ ] **Step 3: Define opaque capability types and the test seam**

```rust
pub(crate) struct SourceCapability { inner: platform::SourceCapability }
pub(crate) struct DestinationCapability { inner: platform::DestinationCapability }
pub(crate) struct StageCapability { inner: platform::StageCapability }

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StableIdentity {
    pub volume: Vec<u8>,
    pub file: Vec<u8>,
}

pub(crate) trait SecureFs: Send + Sync {
    fn capture_source(&self, path: &Path) -> Result<SourceCapability>;
    fn capture_destination(&self, path: &Path) -> Result<DestinationCapability>;
    fn revalidate_source(&self, source: &SourceCapability) -> Result<()>;
    fn revalidate_destination(&self, destination: &DestinationCapability) -> Result<()>;
    fn create_stage(&self, destination: &DestinationCapability) -> Result<StageCapability>;
    fn publish_new(&self, stage: &mut StageCapability) -> Result<PathBuf>;
}
```

Keep constructors and raw handles crate-private. Export only higher-level opaque archive types from `lib.rs`.

`capture_destination` requires an absolute path, literal lowercase `.opentake`, an existing canonicalizable parent, a nonexistent final entry, and no lexical/physical equality or ancestor/descendant overlap with source, source roots, approved media or auxiliary trees. Every component after the nearest existing ancestor must be a validated normal component.

- [ ] **Step 4: Implement Unix global-root traversal and no-replace dispatch**

Use `rustix::fs::openat` with `OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC`, record the `/`-relative component identity chain, and rewalk it with `statat(..., AtFlags::SYMLINK_NOFOLLOW)`. Linux calls `renameat_with(..., RenameFlags::NOREPLACE)`; macOS uses the same Rustix API mapping to `RENAME_EXCL`. Map unsupported `EINVAL`/`ENOTSUP` to `UnsupportedAtomicPublish` and never call ordinary `rename`.

```rust
pub(crate) fn publish_new(stage: &mut StageCapability) -> Result<PathBuf> {
    revalidate_namespace(&stage.destination.anchor)?;
    verify_stage_root(stage)?;
    rustix::fs::renameat_with(
        &stage.destination.parent,
        &stage.stage_name,
        &stage.destination.parent,
        &stage.destination.final_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(map_no_replace_error)?;
    Ok(stage.destination.display_path.clone())
}
```

- [ ] **Step 5: Implement the Windows identity/mapping contract and unsupported target**

Open every ancestor with `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT` and without `FILE_SHARE_DELETE`; reject reparse points, record volume GUID/serial/file ID, and reopen the current drive/mount mapping at validation. `SetFileInformationByHandle(FileRenameInfo)` uses `ReplaceIfExists = FALSE`. UNC/remote or unavailable identity returns `UnsupportedSecureFilesystem`. Non-Unix/non-Windows builds compile `unsupported.rs` and fail closed.

- [ ] **Step 6: Run platform-neutral and local-native tests**

```bash
cargo fmt --all --check
cargo test -p opentake-project --test archive_races -- --test-threads=1
cargo test -p opentake-project --lib safe_fs -- --test-threads=1
cargo clippy -p opentake-project --all-targets -- -D warnings
```

Expected: fake mapping/symlink/reparse seams fail closed; the host-native adapter compiles and passes identity/no-replace tests.

- [ ] **Step 7: Commit the capability foundation**

```bash
git add Cargo.lock crates/opentake-project/Cargo.toml crates/opentake-project/src/lib.rs crates/opentake-project/src/safe_fs crates/opentake-project/tests/archive_races.rs
git commit -m "feat: add secure bundle filesystem capabilities"
```

- [ ] **Step 8: Run the mandatory exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, sync the detached review tree, and dispatch fresh filesystem-security and cross-platform quality agents. Require `APPROVE 0/0/0` reports under `c1-task-3-${SLICE_SHA}-attempt-1`; do not begin source planning until both pass.

### Task 4: Immutable Source Plan, Disclosure Encoding, And Missing Privacy

**Files:**
- Create: `crates/opentake-project/src/archive/source_plan.rs`
- Create: `crates/opentake-project/src/archive/path_encoding.rs`
- Modify: `crates/opentake-project/src/archive.rs`
- Modify: `crates/opentake-project/src/error.rs`
- Modify: `crates/opentake-project/src/lib.rs`
- Test: `crates/opentake-project/tests/archive_security.rs`
- Test: `crates/opentake-project/tests/archive_races.rs`

- [ ] **Step 1: Write RED tests for internal traversal, hard links, special files and source rebind**

```rust
#[test]
fn project_relative_source_must_be_normal_media_path() {
    let fixture = BundleFixture::minimal();
    for relative_path in ["../secret", "/tmp/secret", "chat-sessions/a", "media/../secret"] {
        let error = prepare_archive_sources(
            snapshot_with_manifest(manifest_with_project_source(relative_path)),
            Some(fixture.source_path()),
        )
        .unwrap_err();
        assert!(matches!(error, ProjectError::UnsafeSource { .. }));
    }
}

#[cfg(unix)]
#[test]
fn internal_and_aux_hard_links_are_rejected() {
    let fixture = BundleFixture::with_media_thumbnail_and_chat_hard_links();
    let error = prepare_archive_sources(
        snapshot_with_manifest(fixture.manifest.clone()),
        Some(fixture.source_path()),
    )
    .unwrap_err();
    assert!(matches!(error, ProjectError::UnsupportedSourceType { .. }));
}

#[test]
fn source_namespace_change_after_dialog_is_rejected() {
    let fs = FakeSecureFs::source_mapping_changes_after_capture();
    let plan = prepare_archive_sources_with_fs(
        &fs,
        snapshot_with_manifest(manifest()),
        Some(source_path()),
    )
    .unwrap();
    assert_eq!(plan.revalidate_source(), Err(ProjectError::SourceNamespaceChanged));
}
```

- [ ] **Step 2: Write RED tests for raw path-unit encoding and privacy placeholders**

```rust
#[test]
fn unix_path_bytes_encode_to_reversible_ascii() {
    let encoded = encode_unix_path_units(b"a\n\xff\\b");
    assert_eq!(encoded, r"a\x0A\xFF\x5Cb");
    assert_eq!(decode_unix_path_units(&encoded).unwrap(), b"a\n\xff\\b");
}

#[test]
fn missing_placeholder_never_contains_raw_id_or_absolute_path() {
    let relative = missing_placeholder(7, "../../Users/alice/key", "/Users/alice/.ssh/id_rsa").unwrap();
    assert!(relative.starts_with("media/missing/"));
    assert!(!relative.contains("alice"));
    assert!(!relative.contains(".."));
    assert!(NormalRelativePath::new(Path::new(&relative)).is_ok());
}
```

- [ ] **Step 3: Run the focused tests and preserve RED output**

```bash
cargo test -p opentake-project --test archive_security -- --test-threads=1
cargo test -p opentake-project --test archive_races -- --test-threads=1
```

Expected: FAIL because the immutable plan, path-unit encoder, hard-link policy and typed source errors do not exist.

- [ ] **Step 4: Define the source/disclosure plan types**

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalDisclosureRow {
    pub ordinal: usize,
    pub original: String,
    pub resolved: Option<String>,
    pub status: ExternalSourceStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalSourceStatus { Regular, Missing }

pub struct ArchiveSourcePreparation {
    source: Option<SourceCapability>,
    internal: Vec<PlannedInternalSource>,
    external: Vec<PlannedExternalSource>,
    extras: PlannedExtras,
    disclosure: ExternalDisclosureModel,
}

pub struct ArchiveSnapshot {
    pub timeline: Timeline,
    pub manifest: MediaManifest,
    pub generation_log: GenerationLog,
    pub compatibility: ProjectCompatibility,
}

pub fn prepare_archive_sources(
    snapshot: ArchiveSnapshot,
    source_bundle: Option<&Path>,
) -> Result<ArchiveSourcePreparation>;
```

Cap the model at 4,096 entries / 4 MiB encoded content and each field at 4,096 ASCII columns. Only `NotFound` becomes `Missing`; permission, metadata, type, loop, reparse and link-count errors are typed failures.

- [ ] **Step 5: Implement raw OS-unit encoding and domain-separated missing names**

```rust
fn missing_placeholder(ordinal: usize, id: &str, lexical: &OsStr) -> Result<String> {
    let mut digest = Sha256::new();
    digest.update(b"OpenTake missing external v1\0");
    digest.update((ordinal as u64).to_be_bytes());
    update_length_prefixed(&mut digest, id.as_bytes());
    update_length_prefixed(&mut digest, os_str_bytes(lexical));
    let hex = hex::encode(digest.finalize());
    let basename = sanitize_basename(lexical).unwrap_or_else(|| "missing".into());
    let relative = format!("media/missing/{hex}-{basename}");
    NormalRelativePath::new(Path::new(&relative))?;
    Ok(relative)
}
```

Unix encodes every non-printable-ASCII raw byte and backslash as `\xNN`; Windows encodes every non-printable-ASCII UTF-16 unit and backslash as `\uNNNN`.

- [ ] **Step 6: Implement bounded streaming identity plans**

Record canonical target plus stable identity, size, file type and link count; do not retain one handle per media entry. Reopen one source leaf at a time relative to the source/root capability, recheck identity/type/link count before and after copy, and close it before the next item. Preserve lexical dedup keys so two manifest symlink paths to one external target remain two collected entries.

- [ ] **Step 7: Run focused source-policy tests**

```bash
cargo test -p opentake-project --test archive_security -- --test-threads=1
cargo test -p opentake-project --test archive_races -- --test-threads=1
cargo test -p opentake-project --lib archive::tests::two_symlinks_to_one_file_are_not_deduped -- --exact
cargo clippy -p opentake-project --all-targets -- -D warnings
```

Expected: all pass, including low-FD, relative-external, FIFO/socket/device, permission, symlink-loop, hard-link and source-mapping seams.

- [ ] **Step 8: Commit the immutable source-plan slice**

```bash
git add crates/opentake-project/src/archive.rs crates/opentake-project/src/archive/source_plan.rs crates/opentake-project/src/archive/path_encoding.rs crates/opentake-project/src/error.rs crates/opentake-project/src/lib.rs crates/opentake-project/tests/archive_security.rs crates/opentake-project/tests/archive_races.rs
git commit -m "feat: validate bundle export source plans"
```

- [ ] **Step 9: Run the mandatory exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, sync the clean review tree, and dispatch fresh source-security and compatibility/quality reviewers. Require both reports under `c1-task-4-${SLICE_SHA}-attempt-1` to be `APPROVE 0/0/0`; repair and re-review both roles before native UI work.

### Task 5: Rust-Owned Native External Disclosure

**Files:**
- Create: `src-tauri/src/export/native_disclosure/mod.rs`
- Create: `src-tauri/src/export/native_disclosure/macos.rs`
- Create: `src-tauri/src/export/native_disclosure/windows.rs`
- Create: `src-tauri/src/export/native_disclosure/linux.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/export.rs`
- Test: `src-tauri/tests/bundle_export_security.rs`

- [ ] **Step 1: Write a renderer-isolation RED test around the adapter contract**

```rust
#[test]
fn disclosure_approval_is_native_and_model_complete() {
    let model = disclosure_model_with_rows(32);
    let adapter = RecordingDisclosure::approve();
    assert_eq!(adapter.confirm(&model).unwrap(), DisclosureDecision::Approve);
    assert_eq!(adapter.seen_rows(), model.rows());
    assert!(!adapter.used_renderer_ipc());
}

#[test]
fn cancel_stops_before_destination_dialog() {
    let dialogs = FakeDialogs::cancel_disclosure();
    let result = run_bundle_export_workflow(&core(), &dialogs).unwrap();
    assert_eq!(result, None);
    assert_eq!(dialogs.destination_calls(), 0);
}
```

- [ ] **Step 2: Define the platform-neutral native contract**

```rust
pub enum DisclosureDecision { Approve, Cancel }

pub trait NativeDisclosure: Send + Sync {
    fn confirm(
        &self,
        parent: RawWindowHandle,
        model: &ExternalDisclosureModel,
    ) -> Result<DisclosureDecision, NativeDisclosureError>;
}
```

The production adapter owns the model until a native decision returns. No row, approval token or destination path is serialized to the main WebView.

- [ ] **Step 3: Add direct target dependencies**

Enable AppKit features `NSPanel`, `NSScrollView`, `NSSearchField`, `NSTableColumn`, `NSTableView`, `NSTextField`, `NSButton`, `NSView`, `NSWindow`. Add Linux `gtk = "0.18"` / `glib = "0.18"`, and Windows `windows-sys 0.61` UI/common-control features under target-specific dependency tables.

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2-app-kit = { version = "0.3", default-features = false, features = [
  "NSHapticFeedback", "NSPanel", "NSScrollView", "NSSearchField",
  "NSTableColumn", "NSTableView", "NSTextField", "NSButton", "NSView", "NSWindow"
] }

[target.'cfg(target_os = "linux")'.dependencies]
gtk = "0.18"
glib = "0.18"

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61", features = [
  "Win32_Foundation", "Win32_Graphics_Gdi", "Win32_System_LibraryLoader",
  "Win32_UI_Controls", "Win32_UI_WindowsAndMessaging"
] }
```

- [ ] **Step 4: Implement the macOS table**

Construct an application-modal `NSPanel` containing search, `NSTableView` in `NSScrollView`, Cancel and Approve All. The data source reads immutable Rust-owned rows; approval is disabled until the data source count equals the model count. Return only `Approve` or `Cancel` through the Rust call stack.

```rust
fn row_value(model: &ExternalDisclosureModel, row: NSInteger, column: Column) -> Retained<NSString> {
    let item = &model.rows()[row as usize];
    NSString::from_str(match column {
        Column::Original => &item.original,
        Column::Resolved => item.resolved.as_deref().unwrap_or("missing"),
        Column::Status => item.status.as_str(),
    })
}
```

- [ ] **Step 5: Implement Win32 and GTK tables with the same immutable columns**

Win32 uses a modal owner-bound window and `SysListView32`; GTK uses `gtk::Dialog`, `gtk::SearchEntry`, `gtk::TreeView` and `gtk::ScrolledWindow`. Both populate all rows before enabling Approve and return a native decision directly. Unsupported display initialization returns `NativeDisclosureUnavailable`, never implicit approval.

- [ ] **Step 6: Add native-model/accessibility seam tests**

Test that Unix non-UTF8 bytes, Windows unpaired surrogates, long fields, missing status and final rows are exposed byte-for-byte. Platform UI tests query the native accessibility/tree model; headless unit tests use the recording adapter and exact model equality.

- [ ] **Step 7: Run focused compile/tests**

```bash
cargo test -p opentake-tauri --test bundle_export_security disclosure_ -- --test-threads=1
cargo test -p opentake-tauri --lib native_disclosure -- --test-threads=1
cargo check -p opentake-tauri --no-default-features --all-targets
cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings
```

Expected: native host adapter compiles; fake approval/cancel/model tests pass; no renderer invoke surface exists.

- [ ] **Step 8: Commit native disclosure**

```bash
git add Cargo.lock src-tauri/Cargo.toml src-tauri/src/export.rs src-tauri/src/export/native_disclosure src-tauri/tests/bundle_export_security.rs
git commit -m "feat: add native bundle source disclosure"
```

- [ ] **Step 9: Run the mandatory exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, sync the review tree, and dispatch fresh renderer-boundary/security and platform-quality agents. Require `APPROVE 0/0/0` under `c1-task-5-${SLICE_SHA}-attempt-1`; rerun both after any fix.

### Task 6: Receipt-Backed Stage And Atomic No-Replace Publication

**Files:**
- Create: `crates/opentake-project/src/archive/staging.rs`
- Create: `crates/opentake-project/src/archive/receipt.rs`
- Modify: `crates/opentake-project/src/archive.rs`
- Modify: `crates/opentake-project/src/error.rs`
- Modify: `crates/opentake-project/src/lib.rs`
- Test: `crates/opentake-project/tests/archive_publication.rs`
- Test: `crates/opentake-project/tests/archive_races.rs`

- [ ] **Step 1: Write publication RED tests**

```rust
#[test]
fn concurrent_destination_creation_is_never_replaced() {
    for kind in DestinationKind::ALL {
        let fixture = PublicationFixture::new();
        let mut stage = fixture.build_valid_stage();
        fixture.create_destination_at_publish_seam(kind);
        let before = fixture.destination_digest();
        let error = stage.publish_new().unwrap_err();
        assert!(matches!(error, ProjectError::DestinationExists { .. }));
        assert_eq!(fixture.destination_digest(), before);
    }
}

#[test]
fn final_validation_rejects_valid_json_replacement_and_extra_entry() {
    let fixture = PublicationFixture::new();
    let mut stage = fixture.build_valid_stage();
    fixture.replace_timeline_with_other_valid_json(&stage);
    fixture.add_extra_entry(&stage);
    assert!(matches!(stage.validate_final(), Err(ProjectError::StageValidation { .. })));
}
```

- [ ] **Step 2: Define receipt and guard types**

```rust
pub struct BuildReceipt {
    directories: BTreeMap<NormalRelativePath, StableIdentity>,
    leaves: BTreeMap<NormalRelativePath, LeafReceipt>,
}

pub struct LeafReceipt {
    identity: StableIdentity,
    kind: LeafKind,
    len: u64,
    sha256: [u8; 32],
}

pub struct StageGuard {
    fs: Arc<dyn SecureFs>,
    destination: DestinationCapability,
    stage: StageCapability,
    receipt: BuildReceipt,
    published: bool,
}
```

- [ ] **Step 3: Build only through create-new handles and collect receipts**

Write JSON and copied files through exclusive destination handles. Hash bytes while writing/copying and record directory/leaf identities. Missing external media gets a receipt only for its rewritten JSON entry, not an absent file.

```rust
fn write_leaf(&mut self, path: &NormalRelativePath, bytes: &[u8]) -> Result<()> {
    let mut file = self.stage.create_new(path)?;
    file.write_all(bytes).map_err(ProjectError::from_io)?;
    file.flush().map_err(ProjectError::from_io)?;
    let identity = file.stable_identity()?;
    self.receipt.insert_leaf(path.clone(), identity, bytes.len() as u64, sha256(bytes))
}
```

- [ ] **Step 4: Implement the final complete validation**

Enumerate the stage capability, require the exact receipt path set, reopen every leaf nofollow, compare identity/type/length/full SHA-256, strictly decode `project.json`, `media.json`, `generation-log.json`, and resolve every rewritten media reference inside the stage. Revalidate source/destination namespace immediately before returning success.

- [ ] **Step 5: Implement RAII cleanup and atomic publication**

`Drop`/explicit cleanup removes only entries reachable through the retained stage handle. Remove the stage name only if its identity still matches; otherwise return/log `StageIdentityLost` without deleting the replacement. Publication calls only the platform no-replace adapter. After `validate_final()` no dialog, copy or hashing is allowed before the core finalizer.

- [ ] **Step 6: Run focused publication/race tests**

```bash
cargo test -p opentake-project --test archive_publication -- --test-threads=1
cargo test -p opentake-project --test archive_races -- --test-threads=1
cargo test -p opentake-project --test archive_security -- --test-threads=1
cargo clippy -p opentake-project --all-targets -- -D warnings
```

Expected: file/empty-dir/non-empty-dir/symlink destination collisions preserve exact bytes; receipt, namespace, cleanup and unsupported-filesystem seams fail closed.

- [ ] **Step 7: Commit the staged publisher**

```bash
git add crates/opentake-project/src/archive.rs crates/opentake-project/src/archive/staging.rs crates/opentake-project/src/archive/receipt.rs crates/opentake-project/src/error.rs crates/opentake-project/src/lib.rs crates/opentake-project/tests/archive_publication.rs crates/opentake-project/tests/archive_races.rs
git commit -m "feat: publish validated bundle stages atomically"
```

- [ ] **Step 8: Run the mandatory exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, sync the clean review tree, and dispatch fresh filesystem-security and race/quality reviewers. Require both reports under `c1-task-6-${SLICE_SHA}-attempt-1` to be `APPROVE 0/0/0` before wiring the archive.

### Task 7: Integrate The Secure Archive Pipeline

**Files:**
- Modify: `crates/opentake-project/src/archive.rs`
- Modify: `crates/opentake-project/src/lib.rs`
- Modify: `crates/opentake-project/tests/archive.rs`
- Modify: `crates/opentake-project/tests/archive_security.rs`
- Modify: `crates/opentake-project/tests/archive_publication.rs`

- [ ] **Step 1: Write an end-to-end RED test for a self-contained fresh export**

```rust
#[test]
fn secure_archive_reopens_and_contains_no_external_absolute_source() {
    let fixture = ArchiveFixture::present_and_missing_external();
    let prepared = prepare_archive_sources(fixture.snapshot, fixture.source_path()).unwrap();
    let approved = prepared.approve_for_test();
    let destination = fixture.fresh_destination("Portable.opentake");
    let mut stage = complete_archive_preflight(approved, &destination).unwrap();
    let report = stage.build_and_validate().unwrap();
    stage.publish_new().unwrap();

    let reopened = Project::open(&destination).unwrap();
    assert_eq!(report.missing.len(), 1);
    assert!(reopened.manifest.entries.iter().all(|entry| {
        !matches!(&entry.source, MediaSource::External { .. })
    }));
}
```

- [ ] **Step 2: Replace the old destructive `archive()` body with explicit phases**

Expose these opaque, typed operations:

```rust
pub fn prepare_archive_sources(
    snapshot: ArchiveSnapshot,
    source_bundle: Option<&Path>,
) -> Result<ArchiveSourcePreparation>;
pub fn complete_archive_preflight(
    preparation: ArchiveSourcePreparation,
    approval: ExternalApproval,
    destination: PathBuf,
) -> Result<PreparedArchive>;
impl PreparedArchive {
    pub fn build_and_validate(self) -> Result<(StageGuard, ArchiveReport)>;
}
```

Keep `standardize`, `filename_for`, Swift-equivalent extension parsing and collision behavior. Delete path-based `fs::copy`, `is_file`, `copy_dir_recursive`, destination deletion and direct JSON writes from the callable archive path.

- [ ] **Step 3: Update positive parity and missing privacy expectations**

Existing present external/internal, lexical dedup, thumbnail/chat and naming-collision tests must still pass. Update missing external expectations so the output source is `MediaSource::Project { relative_path: "media/missing/{64-hex-sha256}-{sanitized-basename}".into() }` and contains no original absolute source field.

- [ ] **Step 4: Prove the compatibility guard runs before all filesystem mutation**

Add a test using unknown-schema compatibility plus a fresh destination and race hook; expect `CompatibilityReadOnly`, zero destination/stage entries, and byte-identical source.

- [ ] **Step 5: Run the complete project-crate gate**

```bash
cargo test -p opentake-project -- --test-threads=1
cargo clippy -p opentake-project --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

Expected: full crate passes; no callable path deletes an existing destination or follows project/aux symlinks/hard links.

- [ ] **Step 6: Commit secure archive integration**

```bash
git add crates/opentake-project/src/archive.rs crates/opentake-project/src/lib.rs crates/opentake-project/tests/archive.rs crates/opentake-project/tests/archive_security.rs crates/opentake-project/tests/archive_publication.rs
git commit -m "feat: integrate secure self-contained archives"
```

- [ ] **Step 7: Run the mandatory exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, sync the review tree, and dispatch fresh archive-spec/security and compatibility-quality agents. Require `APPROVE 0/0/0` under `c1-task-7-${SLICE_SHA}-attempt-1`; any fix triggers both roles again.

### Task 8: Lock-Scoped Final CAS And Rust-Owned Tauri Workflow

**Files:**
- Create: `src-tauri/src/export/bundle.rs`
- Modify: `crates/opentake-core/src/core.rs`
- Modify: `crates/opentake-core/src/lib.rs`
- Modify: `src-tauri/src/export.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tests/bundle_export_integration.rs`
- Modify: `src-tauri/tests/schema_compat_integration.rs`
- Test: `src-tauri/tests/bundle_export_security.rs`

- [ ] **Step 1: Write RED tests for cancel, pathless authority and stale finalization**

```rust
#[test]
fn renderer_workflow_has_no_destination_argument_and_cancel_writes_nothing() {
    let core = core_with_project();
    let dialogs = FakeDialogs::cancel_destination();
    let result = run_bundle_export_workflow(&core, &dialogs).unwrap();
    assert_eq!(result, None);
    assert!(dialogs.all_candidate_destinations_absent());
}

#[test]
fn finalizer_blocks_mutation_between_compare_and_rename() {
    let core = Arc::new(core_with_project());
    let expected = core.bundle_identity();
    let mut stage = valid_stage_with_blocking_publish_hook();
    let worker = spawn_publish(core.clone(), expected, &mut stage);
    stage.wait_until_finalizer_holds_core_lock();
    let edit = spawn_edit(core.clone());
    stage.release_publish();
    worker.join().unwrap().unwrap();
    edit.join().unwrap().unwrap();
    assert!(stage.destination_exists());
}

#[test]
fn rust_default_never_aliases_source_and_skips_collisions() {
    let parent = tempfile::tempdir().unwrap();
    let source = parent.path().join("My Film.opentake");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(parent.path().join("My Film-export.opentake")).unwrap();
    let default = bundle_destination_default(Some(&source), "Untitled").unwrap();
    assert_eq!(default.file_name, "My Film-export-2.opentake");
    assert_ne!(default.full_path(), source);
}
```

- [ ] **Step 2: Add the lock-scoped core finalizer**

```rust
pub fn publish_bundle_if_identity(
    &self,
    expected: &BundleIdentity,
    stage: &mut StageGuard,
) -> Result<PathBuf> {
    let session = self.lock();
    if session.bundle_identity() != *expected {
        return Err(CoreError::ProjectChanged);
    }
    stage.revalidate_final_roots()?;
    stage.publish_new().map_err(CoreError::from)
}
```

The complete receipt/hash validation has already succeeded immediately before this call. Keep the core guard alive across only the bounded namespace/stage-root checks and one no-replace rename; do not perform dialogs, copies or hashes under it.

- [ ] **Step 3: Define stable serializable error DTOs**

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundleExportErrorDto {
    pub code: BundleExportErrorCode,
    pub message: String,
    pub display_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BundleExportErrorCode {
    DestinationExists,
    UnsafeSource,
    UnsafeDestination,
    UnsupportedSourceType,
    ProjectChanged,
    StageFailure,
    StageIdentityLost,
    DestinationNamespaceChanged,
    SourceNamespaceChanged,
    UnsupportedFilesystem,
    TooManyExternalSources,
    ExternalDisclosureTooLong,
    Io,
}
```

Map `ProjectError` once with exhaustive `match`; never classify a string. `Ok(None)` is reserved for user cancel.

- [ ] **Step 4: Define injectable Rust-owned dialogs**

```rust
pub trait BundleExportDialogs: Send + Sync {
    fn confirm_external(&self, model: &ExternalDisclosureModel) -> Result<bool, BundleExportErrorDto>;
    fn choose_destination(&self, default: &BundleDestinationDefault) -> Result<Option<PathBuf>, BundleExportErrorDto>;
}

pub async fn run_bundle_export_workflow(
    core: &AppCore,
    dialogs: &dyn BundleExportDialogs,
) -> Result<Option<BundleReportDto>, BundleExportErrorDto>;
```

The production chooser uses `app.dialog().file().set_directory(...).set_file_name(...).add_filter("OpenTake", &["opentake"]).blocking_save_file()` off the main thread, converts `FilePath` to `PathBuf`, and refuses non-local virtual paths.

`bundle_destination_default` appends `-export.opentake` to the project stem, or uses localized `Untitled-export.opentake`, then increments `-2`, `-3`, and so on while any file/directory/symlink exists. The backend still rejects collisions and invalid suffixes after the panel returns.

- [ ] **Step 5: Implement the ordered workflow**

The workflow performs exactly: one-lock snapshot → `SourceCapability` → immediate identity/source recheck → native disclosure → source recheck → Rust save panel → identity/source recheck → `DestinationCapability`/leaf plan → blocking stage build/final validation → `publish_bundle_if_identity` → DTO. Any mismatch cleans the stage through `StageGuard` and returns a typed error.

- [ ] **Step 6: Replace the Tauri command signature**

```rust
#[tauri::command]
pub async fn export_bundle(
    app: tauri::AppHandle,
    core: tauri::State<'_, AppCore>,
) -> Result<Option<BundleReportDto>, BundleExportErrorDto> {
    run_bundle_export_workflow(&core, &ProductionBundleDialogs::new(app)).await
}
```

Remove `out_path` from `export_bundle` and `run_bundle_export`; retain the handler name in `generate_handler!`.

- [ ] **Step 7: Run focused Core/Tauri tests**

```bash
cargo test -p opentake-core core::tests -- --test-threads=1
cargo test -p opentake-tauri --test bundle_export_integration -- --test-threads=1
cargo test -p opentake-tauri --test bundle_export_security -- --test-threads=1
cargo test -p opentake-tauri --test schema_compat_integration -- --test-threads=1
cargo test -p opentake-tauri --no-default-features --test bundle_export_security -- --test-threads=1
cargo clippy -p opentake-tauri --all-targets -- -D warnings
cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings
```

Expected: cancellation writes nothing; source/media/log/path changes at dialog/build seams refuse; queued mutation cannot land between final compare and rename; DTO JSON is stable camelCase/snake_case.

- [ ] **Step 8: Commit the native workflow**

```bash
git add crates/opentake-core/src/core.rs crates/opentake-core/src/lib.rs src-tauri/src/export.rs src-tauri/src/export/bundle.rs src-tauri/src/lib.rs src-tauri/tests/bundle_export_integration.rs src-tauri/tests/bundle_export_security.rs src-tauri/tests/schema_compat_integration.rs
git commit -m "feat: authorize bundle export in Rust"
```

- [ ] **Step 9: Run the mandatory exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, sync the detached review tree, and dispatch fresh authorization/concurrency and Tauri-quality agents. Both reports under `c1-task-8-${SLICE_SHA}-attempt-1` must be `APPROVE 0/0/0`; fix and repeat both before touching Web.

### Task 9: Pathless Web API, Typed Errors, And Empty-Timeline Bundle Entry

**Files:**
- Modify: `web/src/lib/api.ts`
- Modify: `web/src/lib/api.test.ts`
- Modify: `web/src/store/uiStore.ts`
- Modify: `web/src/store/uiStore.test.ts`
- Modify: `web/src/components/shell/TitleBar.tsx`
- Modify: `web/src/components/shell/TitleBar.visual.test.ts`
- Modify: `web/src/components/shell/ExportDialog.tsx`
- Modify: `web/src/components/shell/ExportDialog.test.ts`
- Modify: `web/src/i18n/dict.ts`

- [ ] **Step 1: Write RED API tests proving the invoke has no path**

```ts
it("invokes export_bundle without renderer destination data", async () => {
  const calls: Array<[string, unknown]> = [];
  setInvokeForTest(async (command, args) => {
    calls.push([command, args]);
    return null;
  });
  await exportBundle();
  expect(calls).toEqual([["export_bundle", undefined]]);
});

it("strictly decodes bundle export errors", () => {
  expect(decodeBundleExportError({ code: "project_changed", message: "changed", displayPath: null }))
    .toEqual({ code: "project_changed", message: "changed", displayPath: null });
  expect(() => decodeBundleExportError({ code: "unknown", message: "x" })).toThrow();
});
```

- [ ] **Step 2: Implement the nullable API and typed decoder**

```ts
export type BundleExportErrorCode =
  | "destination_exists" | "unsafe_source" | "unsafe_destination"
  | "unsupported_source_type" | "project_changed" | "stage_failure"
  | "stage_identity_lost" | "destination_namespace_changed"
  | "source_namespace_changed" | "unsupported_filesystem"
  | "too_many_external_sources" | "external_disclosure_too_long" | "io";

export interface BundleExportError {
  code: BundleExportErrorCode;
  message: string;
  displayPath: string | null;
}

export async function exportBundle(): Promise<BundleReport | null> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<BundleReport | null>("export_bundle");
  throw new Error("bundle export requires the desktop app");
}
```

`decodeBundleExportError` checks object shape, known code, string message, and nullable string `displayPath`; it never parses `message`.

- [ ] **Step 3: Add explicit export dialog mode to the UI store**

```ts
export type ExportDialogMode = "video" | "bundle";

openExportDialog(mode: ExportDialogMode) {
  set({ exportDialogOpen: true, exportDialogMode: mode });
}
```

Closing resets transient dialog state on next component open; the selected entry mode does not leak from a prior opening.

- [ ] **Step 4: Add independent TitleBar entries**

The direct video button and video menu item call `openExportDialog("video")` and remain disabled when `!hasClips`. Add an always-enabled `export.bundle.menu` item calling `openExportDialog("bundle")`. Interchange exports remain unchanged.

- [ ] **Step 5: Remove renderer bundle path construction**

Keep `saveDialog` only in the video branch. Delete `BUNDLE_EXT`, `defaultBundleName`, bundle directory/default-path/filter logic and `api.exportBundle(path)`. The bundle handler becomes:

```ts
async function onExportBundle(): Promise<void> {
  if (busy) return;
  setBusy(true);
  setError(null);
  setBundleMissing(null);
  try {
    const report = await api.exportBundle();
    if (report === null) return;
    if (report.missing.length === 0) {
      pushToast(report.collected.length
        ? t("export.bundle.done", { collected: report.collected.length, size: formatBytes(report.totalBytes) })
        : t("export.bundle.doneNoMedia"));
      setOpen(false);
    } else {
      setBundleMissing(report.missing);
      pushToast(t("export.bundle.missing", { count: report.missing.length }));
    }
  } catch (unknownError) {
    const typed = api.decodeBundleExportError(unknownError);
    setError(t(`export.bundle.error.${typed.code}`, { path: typed.displayPath ?? "" }));
  } finally {
    setBusy(false);
  }
}
```

- [ ] **Step 6: Make the empty-timeline rule mode-specific**

```ts
const actionDisabled = busy || (mode === "video" && !hasClips);
```

Opening from the bundle menu remains possible on empty/unsaved projects. Switching to video inside the dialog disables the action.

- [ ] **Step 7: Add Chinese/English stable error messages**

Add one key for every `BundleExportErrorCode`, plus distinct mode titles/menu labels. `destination_exists` instructs choosing a fresh name; namespace/source/project changes instruct retry; unsupported filesystem explains fail-closed behavior. Do not expose raw native error text as the classification.

- [ ] **Step 8: Run focused and full Web gates**

```bash
pnpm -C web test -- src/lib/api.test.ts src/store/uiStore.test.ts src/components/shell/ExportDialog.test.ts src/components/shell/TitleBar.visual.test.ts
pnpm -C web exec tsc -b --pretty false
pnpm -C web test
pnpm -C web build
```

Expected: all pass; source contract shows no bundle `saveDialog`/destination construction, while video still uses the renderer save panel.

- [ ] **Step 9: Commit the Web boundary**

```bash
git add web/src/lib/api.ts web/src/lib/api.test.ts web/src/store/uiStore.ts web/src/store/uiStore.test.ts web/src/components/shell/TitleBar.tsx web/src/components/shell/TitleBar.visual.test.ts web/src/components/shell/ExportDialog.tsx web/src/components/shell/ExportDialog.test.ts web/src/i18n/dict.ts
git commit -m "feat: expose safe bundle export workflow"
```

- [ ] **Step 10: Run the mandatory exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, sync the review tree, and dispatch fresh UI-spec/security and Web-quality agents. Require `APPROVE 0/0/0` reports under `c1-task-9-${SLICE_SHA}-attempt-1`; re-run both after fixes.

### Task 10: Three-Platform Native CI Gate

**Files:**
- Modify: `.github/workflows/ci.yml`
- Test: `.github/workflows/ci.yml`

- [ ] **Step 1: Add a fail-fast-disabled native security matrix**

```yaml
  bundle-security-native:
    name: Bundle security (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-14, windows-latest]
    runs-on: ${{ matrix.os }}
```

- [ ] **Step 2: Add platform-specific setup without weakening existing Ubuntu gates**

Ubuntu installs existing GTK/Tauri packages. macOS uses the system AppKit SDK. Windows uses the MSVC toolchain and the test binary initializes common controls. Cache keys include `${{ runner.os }}`.

```yaml
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - name: Install Linux native dependencies
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libglib2.0-dev libsoup-3.0-dev libasound2-dev pkg-config
      - uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-bundle-security-${{ hashFiles('**/Cargo.toml') }}
```

- [ ] **Step 3: Add exact native commands**

```yaml
      - name: Secure archive native tests
        shell: bash
        run: |
          cargo test -p opentake-project --test archive_security -- --test-threads=1
          cargo test -p opentake-project --test archive_publication -- --test-threads=1
          cargo test -p opentake-project --test archive_races -- --test-threads=1
      - name: Native disclosure and workflow compile/tests
        run: cargo test -p opentake-tauri --no-default-features --test bundle_export_security -- --test-threads=1
      - name: Native adapters clippy
        run: cargo clippy -p opentake-project -p opentake-tauri --no-default-features --all-targets -- -D warnings
```

- [ ] **Step 4: Keep privileged mapping tests optional and deterministic seams blocking**

Ordinary jobs run injected mount/volume-mapping changes. Optional privileged real-remount jobs may run when runner capabilities allow, but their absence never skips the deterministic seam suite.

- [ ] **Step 5: Validate YAML and run local host equivalents**

```bash
git diff --check
cargo test -p opentake-project --test archive_security -- --test-threads=1
cargo test -p opentake-project --test archive_publication -- --test-threads=1
cargo test -p opentake-project --test archive_races -- --test-threads=1
cargo test -p opentake-tauri --no-default-features --test bundle_export_security -- --test-threads=1
```

Expected: local host passes; workflow contains native jobs for all three adapters.

- [ ] **Step 6: Commit the CI matrix**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: gate bundle security on three platforms"
```

- [ ] **Step 7: Run the mandatory exact-commit double review gate**

Set `SLICE_SHA=$(git rev-parse HEAD)`, sync the review tree, and dispatch fresh CI-spec/security and CI-quality agents. Require `APPROVE 0/0/0` under `c1-task-10-${SLICE_SHA}-attempt-1`; fix and repeat both roles.

### Task 11: Full Branch Gate And Independent C1 Audit

**Files:**
- Verify only; do not modify product files unless a failing gate exposes a defect.
- Evidence: set `GATE_TIMESTAMP=$(date +%Y%m%d-%H%M%S)` and `C1_SHA=$(git rev-parse HEAD)`, then write under `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/branch-gates/${GATE_TIMESTAMP}-${C1_SHA}/`.

- [ ] **Step 1: Freeze the candidate and verify both trees**

```bash
C1_SHA=$(git rev-parse HEAD)
test -z "$(git status --short)"
git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/OpenTake-wave1a-review merge --ff-only "$C1_SHA"
test -z "$(git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/OpenTake-wave1a-review status --short)"
```

- [ ] **Step 2: Run the complete Rust/Web branch gate and capture every log**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings
cargo test --workspace --all-targets -- --test-threads=1
cargo audit
pnpm -C web test
pnpm -C web build
pnpm -C web audit --audit-level high
```

Expected: every command exits 0. `cargo audit` also runs; if advisory index fetch fails, record the network error and retry once online without hiding allowed-warning output.

- [ ] **Step 3: Verify the exact diff and absence of destructive APIs**

```bash
git diff --check 31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace.."$C1_SHA"
rg -n 'remove_dir_all\(dest|exportBundle\([^)]|export_bundle\([^)]*out_path|fs::copy\(' crates/opentake-project/src src-tauri/src web/src
```

Expected: no destination deletion, raw bundle path IPC, or callable path-based source copy remains. Any intentional unrelated `fs::copy` match is documented with file/line and excluded only after review.

- [ ] **Step 4: Dispatch two fresh whole-C1 auditors**

One auditor reviews spec/security across `31bfd57..$C1_SHA`; the other reviews quality/concurrency/platform behavior. Each verifies the detached tree exact SHA and clean status, runs focused tests, and writes a report with `APPROVE 0/0/0`. Any finding produces a new fix commit, invalidates this gate directory, and restarts Task 11 from Step 1.

### Task 12: Exact App Bundle And Real UI QA

**Files:**
- Verify exact candidate only.
- Evidence: set `QA_TIMESTAMP=$(date +%Y%m%d-%H%M%S)` and `C1_SHA=$(git rev-parse HEAD)`, then write under `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/qa/${QA_TIMESTAMP}-${C1_SHA}/`.

- [ ] **Step 1: Build the exact candidate bundle**

```bash
pnpm -C web build
(cd src-tauri && ../web/node_modules/.bin/tauri build)
shasum -a 256 target/release/bundle/macos/OpenTake.app/Contents/MacOS/opentake
shasum -a 256 target/release/bundle/dmg/OpenTake_1.0.0_aarch64.dmg
find target/release/bundle/macos/OpenTake.app -type f -print0 | sort -z | xargs -0 shasum -a 256 | shasum -a 256
codesign -dv --verbose=4 target/release/bundle/macos/OpenTake.app
codesign --verify --deep --strict --verbose=2 target/release/bundle/macos/OpenTake.app
spctl --assess --type execute --verbose=4 target/release/bundle/macos/OpenTake.app
otool -L target/release/bundle/macos/OpenTake.app/Contents/MacOS/opentake
```

Record exact Git SHA, executable/app-tree/DMG hashes, codesign identity, strict verification, `spctl`, and `otool -L`. Signing/notarization failures remain explicit Wave 1B-D blockers and are not relabeled as C1 success.

- [ ] **Step 2: Create recursive before-manifests for every source and pre-existing destination**

Use `find -P ... -print0 | sort -z | xargs -0 shasum -a 256` plus type/link metadata. Fixtures cover saved internal media/thumbnail/chat, empty unsaved project, readable and missing external paths, symlink/hard-link/special-file refusals, and an existing destination sentinel.

- [ ] **Step 3: Use the computer-use skill to run real native UI scenarios**

Launch only the exact built `.app`. Verify:

- saved and empty/unsaved project bundle entries are available while empty video export is disabled;
- saved `My Film.opentake` defaults to `My Film-export.opentake`; unsaved defaults to localized `Untitled-export.opentake`; neither equals the source;
- disclosure Cancel and save-panel Cancel create nothing;
- native disclosure shows every external row and approval produces a fresh self-contained bundle;
- existing destination, source alias, traversal, symlink, hard-link and typed namespace/source errors refuse without mutation;
- missing external media is reported and rewritten to an absent project-relative digest path with no external absolute source field;
- successful output reopens in the exact app with internal media, thumbnail and chat intact;
- app exit leaves no OpenTake, ffmpeg, playback or helper process.

- [ ] **Step 4: Compare recursive after-manifests and published output**

All refused/cancelled source and destination manifests must be byte-identical. A successful destination must contain no stage names, extra files, stale media or external absolute source fields, and must reopen through `Project::open` and the UI.

- [ ] **Step 5: Write the QA receipt and final exact-SHA audit**

The receipt lists commands, exits, screenshots, fixture hashes, app hashes, typed outcomes, signing limitations and process cleanup. Dispatch a final fresh auditor over code + branch gate + exact bundle + UI evidence; require `APPROVE 0/0/0`. A finding produces a new commit and restarts Tasks 11–12.

## C1 Completion Condition

C1 is complete only when Tasks 1–10 each passed their own exact-commit double review, Task 11 passed full gates and whole-slice double audit, and Task 12 passed exact-bundle real UI QA plus final independent audit. At that point update the full-convergence tracker: C1 is complete, C2 Save-As is next, and Wave 1B-C as a whole remains incomplete until C2 passes its own plan, implementation, exact-bundle QA and reviews.

## Design Coverage Check

- Emergency source/self-overwrite and existing-destination preservation: Task 1.
- Whole-export identity, media/log/path mutation coverage and stale CAS: Tasks 2 and 8.
- Absolute/lowercase/new destination, overlap, namespace mapping, nofollow/reparse and atomic no-replace: Tasks 3 and 6.
- Source capability, traversal, hard links, external type/NotFound-only, bounded handles and removable-volume rebind: Task 4.
- Complete renderer-independent external disclosure and raw OS path-unit encoding: Task 5.
- Receipt provenance, strict stage validation, cleanup, no extra/stale entries and final publish: Tasks 6 and 7.
- Rust-owned dialog, safe defaults, typed error DTO and cancellation: Task 8.
- Pathless Web API, empty-timeline bundle access, video disable rule and localized typed results: Task 9.
- Linux/macOS/Windows native verification: Task 10.
- Full regression, security audit, exact app artifact, signing disclosure and real UI proof: Tasks 11 and 12.

## Execution Mode

Use `superpowers:subagent-driven-development`. The user already delegated execution choice and requires independent agent review before progression, so the controller dispatches a fresh implementation agent for each task, integrates and verifies the commit, then dispatches the two fresh review roles defined above. Inline execution is not selected for C1.
