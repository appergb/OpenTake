# OpenTake Wave 1B-C1B Safe Filesystem Capability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` task by task. Each task follows RED → GREEN → focused commit → exact-commit double review. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a private, complete Linux/macOS/Windows capability-relative filesystem substrate that later C1C/C1D code can use without inventing path-based fallbacks, while keeping bundle export fail closed and unavailable in the product.

**Architecture:** `opentake-project::safe_fs` owns raw OS-component validation, retained directory/file authority, stable identities, namespace snapshots, nofollow/reparse inspection, relative open/create/enumerate/unlink primitives, and same-parent atomic no-replace rename. Linux/macOS use `rustix` from global `/`; Windows uses retained HANDLEs and `NtCreateFile`/`NtOpenFile` with `OBJECT_ATTRIBUTES.RootDirectory`. C1B does not construct archive source plans, stages, receipts, dialogs, CAS state, Tauri commands, or Web UI.

**Tech Stack:** Rust 2021, rustix 1.1.4, windows-sys 0.61.2, native GitHub Actions on Ubuntu/macOS/Windows, Cargo cross-target checks.

## Global Constraints

- Approved design: `docs/superpowers/specs/2026-07-12-opentake-wave-1b-c-filesystem-security-design.md` at `31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace`.
- Exact C1B baseline: `e67917260ace36e4db1ede4e36eecbc401825bb1`; C1A evidence is `OpenTake-safety/20260712-wave1bc-filesystem/branch-gates/20260712-203854-e67917260ace36e4db1ede4e36eecbc401825bb1-WjVSRm/results.md` with final aggregate 0.
- `safe_fs` remains crate-private, opaque, non-Clone for authority objects, and unused by `archive()`, `bundle.rs`, Tauri, Core, MCP, Agent, and Web in C1B.
- The registered `export_bundle` handler and Web bundle mode remain absent at every intermediate commit. C1B does not claim C1 or Wave 1B-C completion.
- No authority decision uses `canonicalize()`, `to_string_lossy()`, ambient joined-path `std::fs` operations, `CreateFileW(parent.join(child))`, ordinary rename, or check-then-rename fallback.
- Raw handles never leave `safe_fs`; later callers receive only typed capability methods accepting `ComponentName` or `RelativeComponents`.
- C1B reports metadata/link counts and raw link/reparse data; C1C owns archive role policy and disclosure. C1D owns `StageGuard`, receipts, recursive cleanup orchestration, and publish semantics. C1E owns revision CAS, native dialogs, Tauri/Web integration, and the single coherent product re-enable commit.
- Linux, macOS, and Windows native tests are blocking evidence. Cross-compilation is an additional compile gate and cannot be reported as native behavioral success.
- Product/plan edits use `apply_patch`; formatters may make only mechanical rewrites. Every task commit receives two fresh exact-SHA reviews (filesystem/spec and implementation/quality), both `APPROVE 0/0/0`; every finding is fixed and both roles repeat.

---

## Frozen Decisions

### Capability and error ownership

`SafeFsError` is private and role-neutral:

```rust
pub(crate) enum SafeFsError {
    InvalidComponent(ComponentViolation),
    NotFound,
    AlreadyExists,
    SymlinkOrReparsePoint,
    UnsupportedEntryType(EntryKind),
    IdentityChanged { expected: StableIdentity, actual: StableIdentity },
    NamespaceChanged,
    UnsupportedSecureFilesystem(SecureFilesystemReason),
    UnsupportedAtomicPublish(AtomicPublishReason),
    Io { operation: SafeFsOperation, source: std::io::Error },
}
```

C1B does not add role-specific `ProjectError` variants. C1C maps source context; C1D maps destination/stage context. `NotFound` is never silently treated as missing inside C1B.

### Complete private operation surface

`ops.rs` exposes exactly these crate-private operations and no arbitrary `&Path` child operation:

```rust
pub(crate) fn capture_absolute_directory(path: &Path) -> Result<AnchoredDirectory>;
pub(crate) fn revalidate_namespace(directory: &AnchoredDirectory) -> Result<()>;
pub(crate) fn query_child_nofollow(parent: &AnchoredDirectory, name: &ComponentName) -> Result<ChildState>;
pub(crate) fn open_dir_nofollow(parent: &AnchoredDirectory, name: &ComponentName) -> Result<DirectoryCapability>;
pub(crate) fn open_file_nofollow(parent: &AnchoredDirectory, name: &ComponentName, access: OpenAccess) -> Result<FileCapability>;
pub(crate) fn metadata_from_dir(directory: &DirectoryCapability) -> Result<EntryMetadata>;
pub(crate) fn metadata_from_file(file: &FileCapability) -> Result<EntryMetadata>;
pub(crate) fn enumerate(directory: &DirectoryCapability) -> Result<Vec<ComponentName>>;
pub(crate) fn read_link_component(parent: &AnchoredDirectory, name: &ComponentName) -> Result<RawLinkTarget>;
pub(crate) fn create_dir_new(parent: &AnchoredDirectory, name: &ComponentName, permissions: CreatePermissions) -> Result<DirectoryCapability>;
pub(crate) fn create_file_new(parent: &AnchoredDirectory, name: &ComponentName, permissions: CreatePermissions) -> Result<FileCapability>;
pub(crate) fn unlink_file_if_identity(parent: &AnchoredDirectory, name: &ComponentName, expected: &StableIdentity) -> Result<()>;
pub(crate) fn remove_dir_if_identity(parent: &AnchoredDirectory, name: &ComponentName, expected: &StableIdentity) -> Result<()>;
pub(crate) fn rename_noreplace_same_parent(parent: &AnchoredDirectory, from: &ComponentName, expected_from: &StableIdentity, to: &ComponentName) -> Result<()>;
```

All capability structs are move-only and `Send`, use redacted `Debug`, and expose no `AsRawFd`/`AsRawHandle` method outside platform modules.

### Platform decisions

- Unix support is exactly `target_os = "linux"` or `target_os = "macos"`, not generic `cfg(unix)`. A namespace snapshot stores global-root identity plus every normal component and `(st_dev, st_ino)`; revalidation freshly rewalks from `/`.
- macOS accepts only filesystems whose `statfs` reports `MNT_LOCAL`. Linux accepts only an explicit local stable-identity allowlist (ext2/3/4 magic, XFS, Btrfs, tmpfs, and ZFS); network, FUSE, proc/sys/device pseudo-filesystems, and unknown types return `UnsupportedSecureFilesystem`.
- Windows initial absolute capture records the current volume-mount mapping, canonical volume GUID, volume serial, and every ancestor `FILE_ID_128`; UNC/remote/unknown mapping fails closed.
- Every Windows child operation uses `NtCreateFile`/`NtOpenFile` with `OBJECT_ATTRIBUTES.RootDirectory = retained_parent`. `CreateFileW` is allowed only for the initial absolute volume/mount anchor.
- Windows queries `FileCaseSensitiveInfo` on the retained parent. It omits `OBJ_CASE_INSENSITIVE` for a case-sensitive directory, uses it for a proven insensitive directory, and fails closed if the query is unavailable.
- `ComponentName` on Windows rejects NUL, both separators, colon/ADS, trailing dot/space, `.`/`..`, and case-insensitive DOS device stems (`CON`, `PRN`, `AUX`, `NUL`, `COM1`…`COM9`, `LPT1`…`LPT9`). It retains the original UTF-16 units, including unpaired surrogates.
- Unix owner-only create uses 0700 directories / 0600 files. Windows owner-only create passes an explicit protected DACL for the current token owner in the create call; inheriting the parent ACL is not equivalent.
- Unix atomic rename uses `rustix::fs::renameat_with(..., RenameFlags::NOREPLACE)`. Windows renames the retained source handle with `FILE_RENAME_INFO`, `ReplaceIfExists = FALSE`, and retained parent `RootDirectory`. No fallback exists.

## File Responsibilities

- `crates/opentake-project/src/archive.rs` — Task 1 cfg-only fix for Unix test helpers; no archive behavior change.
- `crates/opentake-project/Cargo.toml` / `Cargo.lock` — exact platform dependencies and features.
- `crates/opentake-project/src/lib.rs` — declare private `mod safe_fs`; never re-export it.
- `crates/opentake-project/src/safe_fs/component.rs` — OS-unit-preserving component/relative value types.
- `crates/opentake-project/src/safe_fs/capability.rs` — identities, snapshots, metadata, opaque capability wrappers.
- `crates/opentake-project/src/safe_fs/error.rs` — private typed errors and deterministic normalization.
- `crates/opentake-project/src/safe_fs/ops.rs` — complete facade listed above.
- `crates/opentake-project/src/safe_fs/unix.rs` — global-root descriptor backend.
- `crates/opentake-project/src/safe_fs/windows.rs` — NT/HANDLE-relative backend and all unsafe FFI.
- `crates/opentake-project/src/safe_fs/unsupported.rs` — deterministic fail-closed adapter.
- `crates/opentake-project/src/safe_fs/tests.rs` — private platform-neutral seams; native tests remain adjacent to platform modules.
- `.github/workflows/ci.yml` — lightweight three-platform native `safe_fs` matrix without weakening existing Ubuntu workspace jobs.

### Task 1: Make Project Fixtures Three-Target Lint Clean

**Files:** Modify `crates/opentake-project/src/archive.rs` only.

- [ ] **Step 1: Record the existing Windows RED**

```bash
CARGO_TARGET_DIR=/tmp/opentake-c1b-task1-windows cargo clippy -p opentake-project --lib --tests --target x86_64-pc-windows-msvc -- -D warnings
```

Expected: FAIL only for dead `external_entry`, `TestDir`, and its `new`/`path` methods because their consumers are Unix-only.

- [ ] **Step 2: Apply exact cfg ownership**

Split the import exactly as follows:

```rust
use opentake_domain::ClipType;
#[cfg(unix)]
use opentake_domain::MediaManifestEntry;
```

Then insert a standalone `#[cfg(unix)]` attribute immediately above each of the
four existing items named `external_entry`, `TestDir`, `impl TestDir`, and
`impl Drop for TestDir`, without changing any body. Do not add
`allow(dead_code)` and do not modify archive behavior.

- [ ] **Step 3: Run three-target GREEN and focused regression**

```bash
cargo fmt --all --check
cargo clippy -p opentake-project --lib --tests --target aarch64-apple-darwin -- -D warnings
cargo clippy -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu -- -D warnings
cargo clippy -p opentake-project --lib --tests --target x86_64-pc-windows-msvc -- -D warnings
cargo test -p opentake-project --lib archive::tests -- --test-threads=1
git diff --check
```

- [ ] **Step 4: Commit and exact double review**

```bash
git add crates/opentake-project/src/archive.rs
git commit -m "test: make project fixtures target-clean"
```

Fresh reviewers: platform/spec and Rust quality. Reports under `OpenTake-safety/20260712-wave1bc-filesystem/logs/c1b-task-1-${SHA}-attempt-1/`; both 0/0/0 before Task 2.

### Task 2: Freeze Common Types, Full Facade, and Unsupported Backend

**Files:** Create `safe_fs/{mod,component,capability,error,ops,unsupported,tests}.rs`; modify crate `src/lib.rs`.

- [ ] **Step 1: Add private RED tests first**

Tests in `safe_fs/tests.rs` must cover these exact cases:

```rust
#[test] fn component_rejects_empty_dot_parent_separator_and_nul();
#[test] fn relative_components_reject_absolute_prefix_and_parent();
#[cfg(unix)] #[test] fn component_preserves_non_utf8_bytes();
#[cfg(windows)] #[test] fn component_preserves_unpaired_utf16_and_rejects_ads_dos_and_trailing_ambiguity();
#[test] fn snapshot_comparison_rejects_mapping_depth_name_or_identity_change();
#[test] fn authority_types_are_send_and_have_redacted_debug();
#[test] fn unsupported_backend_read_create_and_rename_all_fail_closed();
```

Run and retain the compile RED:

```bash
cargo test -p opentake-project --lib safe_fs::tests -- --test-threads=1
```

- [ ] **Step 2: Implement component value types without lossy conversion**

`ComponentName` stores `OsString`; `RelativeComponents` stores a non-empty `Vec<ComponentName>`. Constructors iterate `Path::components()` and accept only `Component::Normal`. Unix rejects embedded NUL with `OsStrExt::as_bytes`. Windows validates `encode_wide()` against the frozen rules above and never reconstructs through Unicode scalar strings. Provide only `as_os_str()` and iteration; no unchecked constructor outside platform modules.

- [ ] **Step 3: Implement the complete common contract**

Use these exact common types:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StableIdentity {
    Unix { device: u64, inode: u64 },
    Windows { volume_serial: u64, file_id: [u8; 16] },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryKind { RegularFile, Directory, SymlinkOrReparse, Other }

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EntryMetadata {
    pub(crate) identity: StableIdentity,
    pub(crate) kind: EntryKind,
    pub(crate) len: u64,
    pub(crate) link_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NamespaceMapping {
    UnixRoot { identity: StableIdentity },
    WindowsVolume { mapping: Vec<u16>, volume_guid: Vec<u16>, serial: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NamespaceComponent {
    pub(crate) name: ComponentName,
    pub(crate) identity: StableIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NamespaceSnapshot {
    pub(crate) mapping: NamespaceMapping,
    pub(crate) components: Vec<NamespaceComponent>,
}

pub(crate) enum ChildState { Absent, Present(EntryMetadata) }
pub(crate) enum CreatePermissions { Inherit, OwnerOnly }
pub(crate) enum OpenAccess { Read, ReadWrite }
pub(crate) enum RawLinkTarget { Unix(Vec<u8>), Windows { tag: u32, bytes: Vec<u8> } }
```

`compare_namespace(expected, actual)` compares mapping and every ordered component and returns only `SafeFsError::NamespaceChanged` on any difference. Opaque capability wrappers own platform handles, implement custom redacted `Debug`, do not implement `Clone`, and have compile assertions for `Send`.

- [ ] **Step 4: Implement the exact facade and fail-closed unsupported target**

`ops.rs` implements every frozen signature by dispatching to the selected platform module. During Task 2 every target dispatches to `unsupported`, whose acquisition returns `UnsupportedSecureFilesystem(UnsupportedTarget)` and whose mutation returns either the same or `UnsupportedAtomicPublish(PrimitiveUnavailable)`. `mod.rs` contains a narrow module-level dead-code allowance with this exact removal note:

```rust
#![allow(dead_code)] // C1B private foundation; remove when C1C/C1D consumes the facade.
```

`lib.rs` adds only `mod safe_fs;`. It does not re-export types.

- [ ] **Step 5: GREEN gates and focused commit**

```bash
cargo fmt --all --check
cargo test -p opentake-project --lib safe_fs -- --test-threads=1
cargo clippy -p opentake-project --lib --tests -- -D warnings
cargo check -p opentake-project --lib --tests --target aarch64-apple-darwin
cargo check -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu
cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc
cargo check --workspace --all-targets
cargo check -p opentake-tauri --no-default-features --all-targets
cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
git diff --check
```

```bash
git add crates/opentake-project/src/lib.rs crates/opentake-project/src/safe_fs
git commit -m "feat(project): define safe filesystem capabilities"
```

Double review: interface/spec completeness and visibility/type quality; both 0/0/0.

### Task 3: Implement Linux/macOS Read-Only Capability Operations

**Files:** Modify `crates/opentake-project/Cargo.toml`, `Cargo.lock`, `safe_fs/{mod,ops}.rs`; create `safe_fs/unix.rs`; modify `.github/workflows/ci.yml` to add the native matrix now.

- [ ] **Step 1: Add the direct dependency and native matrix**

```toml
[target.'cfg(any(target_os = "linux", target_os = "macos"))'.dependencies]
rustix = { version = "=1.1.4", features = ["fs"] }
```

Add a `safe-filesystem` matrix for `ubuntu-latest`, `macos-14`, and `windows-latest`. It runs `cargo fmt --all --check`, `cargo clippy -p opentake-project --lib --tests -- -D warnings`, `cargo test -p opentake-project --lib safe_fs -- --test-threads=1`, and `cargo test -p opentake-project --test archive_security -- --test-threads=1`; retain existing jobs and include `Cargo.lock` in the new cache key.

- [ ] **Step 2: Add native RED fixtures**

Adjacent `unix.rs` tests cover: global-root component chain; directory/file/dangling symlink rejection; hard-link count 2; FIFO/socket classified Other; ancestor rename/replacement causes `NamespaceChanged`; retained-parent operation still hits the retained fd after path replacement; enumeration revalidates names; non-UTF8 link bytes preserved; deterministic local-filesystem/mapping seam rejection.

- [ ] **Step 3: Implement capture, revalidation, query/open, metadata, enumerate, and readlink**

Use only these syscall shapes:

```rust
const DIR_FLAGS: OFlags = OFlags::RDONLY.union(OFlags::DIRECTORY).union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC);
const FILE_READ_FLAGS: OFlags = OFlags::RDONLY.union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC);

let root = rustix::fs::open("/", DIR_FLAGS, Mode::empty())?;
let child = rustix::fs::openat(&parent, name.as_os_str(), DIR_FLAGS, Mode::empty())?;
let metadata = rustix::fs::fstat(&child)?;
let link = rustix::fs::readlinkat(&parent, name.as_os_str(), Vec::new())?;
```

Capture walks from `/`, retains each directory fd plus name/identity, and records every mount-point component through the ordered chain. Revalidation opens a fresh `/` and repeats the walk before `compare_namespace`. `query_child_nofollow` uses `statat(..., AtFlags::SYMLINK_NOFOLLOW)` only for discovery; opened-handle `fstat` is authoritative. `Dir::read_from` supplies names, but every name is passed through `ComponentName` and re-queried nofollow. Only `Errno::NOENT` becomes `NotFound`; `LOOP` becomes `SymlinkOrReparsePoint`; all other errno values remain typed IO/unsupported.

- [ ] **Step 4: Host and native Linux/macOS GREEN**

```bash
cargo test -p opentake-project --lib safe_fs::unix::tests::read_ -- --test-threads=1
cargo test -p opentake-project --lib safe_fs::unix::tests::namespace_ -- --test-threads=1
cargo clippy -p opentake-project --lib --tests -- -D warnings
cargo check -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu
```

Commit `feat(project): capture Unix filesystem capabilities`. Exact review requires macOS native receipt and Linux native matrix receipt for the same SHA; cross-check alone is insufficient.

### Task 4: Implement Linux/macOS Create, Verified Removal, and No-Replace

**Files:** Modify `safe_fs/unix.rs` and private tests only.

- [ ] **Step 1: RED tests**

Tests cover 0700 directory/0600 file exclusive create; existing file/dir/symlink never overwritten; identity swap immediately before unlink/remove refuses and preserves replacement; same-parent no-replace succeeds once; collisions with file/empty dir/non-empty dir/symlink remain byte/link-identical; injected `NOSYS`/`NOTSUP`/`OPNOTSUPP`/validated-argument `INVAL` returns `UnsupportedAtomicPublish`; no ordinary rename counter is ever incremented.

- [ ] **Step 2: Implement exact primitive behavior**

Use `mkdirat(parent, name, Mode::RUSR | Mode::WUSR | Mode::XUSR)` then reopen nofollow and verify directory identity/mode. Files use `WRONLY | CREATE | EXCL | NOFOLLOW | CLOEXEC` and `Mode::RUSR | Mode::WUSR`. Before `unlinkat`, reopen/query the child nofollow and compare `StableIdentity`; directory removal uses `AtFlags::REMOVEDIR`. Rename first reopens `from` nofollow and compares identity, then calls exactly:

```rust
rustix::fs::renameat_with(
    parent_fd,
    from.as_os_str(),
    parent_fd,
    to.as_os_str(),
    RenameFlags::NOREPLACE,
)
```

Map `EXIST`/`NOTEMPTY` to `AlreadyExists`, unsupported primitive errno to `UnsupportedAtomicPublish`, `XDEV` to unsupported/invariant failure, and never call `renameat`/`std::fs::rename`.

- [ ] **Step 3: GREEN, commit, and double review**

Run all Unix `safe_fs` tests plus three-target checks, workspace/no-default checks, C1A surface test, and `git diff --check`. Commit `feat(project): add Unix capability-relative mutations`. Exact review requires macOS + Linux native receipts at the same SHA.

### Task 5: Implement Windows Read-Only Capability Operations

**Files:** Add target dependency/features; create `safe_fs/windows.rs`; modify dispatch and private tests.

- [ ] **Step 1: Add exact Windows dependency**

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "=0.61.2", features = [
  "Wdk_Foundation", "Wdk_Storage_FileSystem", "Win32_Foundation",
  "Win32_Security", "Win32_Storage_FileSystem", "Win32_System_IO",
  "Win32_System_Ioctl", "Win32_System_SystemServices",
] }
```

- [ ] **Step 2: RED native Windows fixtures**

Tests cover file/directory symlink, dangling link, junction/mount-point reparse, hard-link count, directory/other kind, volume mapping/ancestor identity change, share-delete block-or-detect, case-sensitive lookup, remote/UNC refusal, unpaired UTF-16 component round-trip, and exact RAII handle baseline after repeated open/drop. Test seam uses barriers/condition variables, never sleep.

- [ ] **Step 3: Implement FFI helpers and read-only operations**

`windows.rs` begins with `#![deny(unsafe_op_in_unsafe_fn)]`. Each unsafe block documents pointer lifetime, byte length, structure layout, and ownership. Initial absolute capture may use `CreateFileW` with `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT`, `FILE_SHARE_READ | FILE_SHARE_WRITE` (no DELETE), then validates `FileAttributeTagInfo`, `FileIdInfo`, `FileStandardInfo`, volume GUID/serial, and case-sensitive info.

Every child open builds a single-component `UNICODE_STRING` and `OBJECT_ATTRIBUTES { RootDirectory: parent_handle, Attributes: case_flag, ... }`, then calls `NtCreateFile` with `FILE_OPEN`, `FILE_OPEN_REPARSE_POINT`, `FILE_SYNCHRONOUS_IO_NONALERT`, plus `FILE_DIRECTORY_FILE` or `FILE_NON_DIRECTORY_FILE`. `RtlNtStatusToDosError` preserves error classes. `NtQueryDirectoryFile` enumerates relative entries. `FSCTL_GET_REPARSE_POINT` returns raw tag/data only; C1C owns allowlisting/resolution.

- [ ] **Step 4: Windows native and cross-target GREEN**

```powershell
cargo test -p opentake-project --lib safe_fs::windows::tests::read_ -- --test-threads=1
cargo test -p opentake-project --lib safe_fs::windows::tests::namespace_ -- --test-threads=1
cargo clippy -p opentake-project --lib --tests -- -D warnings
```

On macOS also run `cargo check ... --target x86_64-pc-windows-msvc`. Commit `feat(project): capture Windows filesystem capabilities`. Exact review is blocked without the Windows native matrix receipt at this SHA.

### Task 6: Implement Windows Create, Verified Removal, and No-Replace

**Files:** Modify `safe_fs/windows.rs`, target feature list if the exact imports require it, and private tests.

- [ ] **Step 1: RED native fixtures**

Tests cover explicit protected owner-only DACL; exclusive directory/file create; existing file/dir/reparse preservation; RootDirectory-relative enumeration/create/open/delete; identity swap before delete; stage-name rebound preservation; no-replace collision with file/empty dir/non-empty dir/reparse; `FILE_RENAME_INFO` byte layout; ACL/access failure not misreported as collision; remote volume unsupported.

- [ ] **Step 2: Implement exact mutation primitives**

Build current-owner SID and protected DACL before create and pass it through the create security descriptor. Relative `NtCreateFile(FILE_CREATE)` receives parent `RootDirectory`; created handle metadata/mode/DACL is verified before return. Identity-bound delete opens the child relative to parent with DELETE access and uses handle disposition information only after identity comparison.

No-replace constructs one variable-length `FILE_RENAME_INFO` buffer with `ReplaceIfExists = FALSE`, `RootDirectory = retained_parent`, UTF-16 byte length (not unit count), and the validated target component; call `SetFileInformationByHandle(FileRenameInfo)` on the retained source handle. On failure, re-query target relative to the retained parent: proven present maps to `AlreadyExists`; otherwise preserve ACL/sharing/IO or `UnsupportedAtomicPublish`. Never call `MoveFileExW` or a joined-path API.

- [ ] **Step 3: Native GREEN, commit, and double review**

Run all Windows safe-fs native tests, cross checks for all three targets, macOS/Linux existing native tests, workspace/no-default checks, surface test, and diff check. Commit `feat(project): add Windows capability-relative mutations`. Review roles are Windows filesystem security and FFI/layout/handle quality; both require the native Windows receipt and 0/0/0.

### Task 7: Three-Platform Convergence Gate and C1B Handoff

**Files:** Verify only; modify `.github/workflows/ci.yml` or product code only if a gate exposes a defect.

- [ ] **Step 1: Freeze exact SHA and native receipts**

Record integration/review HEAD and clean status. Require the `safe-filesystem` Ubuntu/macOS/Windows jobs to name the same exact SHA and exit 0; a cross-check receipt cannot substitute for a missing native job.

- [ ] **Step 2: Run local complete gates**

```bash
cargo fmt --all --check
cargo clippy -p opentake-project --lib --tests -- -D warnings
cargo test -p opentake-project --lib safe_fs -- --test-threads=1
cargo test -p opentake-project --all-targets -- --test-threads=1
cargo check -p opentake-project --lib --tests --target aarch64-apple-darwin
cargo check -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu
cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc
cargo check --workspace --all-targets
cargo check -p opentake-tauri --no-default-features --all-targets
cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
pnpm -C web exec vitest run src/components/shell/ExportDialog.test.ts
git diff --check e67917260ace36e4db1ede4e36eecbc401825bb1..HEAD
```

- [ ] **Step 3: Prove C1B stayed private and product stayed closed**

```bash
! rg -n '\bexport_bundle\b' src-tauri/src/lib.rs
! rg -n '\bid\s*:\s*["'"']bundle["'"']' web/src/components/shell/ExportDialog.tsx
! rg -n 'safe_fs' crates/opentake-project/src/archive.rs crates/opentake-project/src/bundle.rs src-tauri web
rg -n '^mod safe_fs;$' crates/opentake-project/src/lib.rs
! rg -n '^pub (mod|use).*safe_fs' crates/opentake-project/src/lib.rs
```

- [ ] **Step 4: Final exact-SHA double audit and receipt**

Create an exclusive evidence directory with every command/log/exit, three native job receipts, dependency diff, and final before/after clean SHA. Dispatch fresh design-boundary/security and three-platform quality auditors. Both reports must bind Role/Commit and state `APPROVE 0/0/0`. Any finding or nonzero job creates a new fix commit and reruns all three native jobs and both auditors.

## Completion Statement

C1B completes only the private safe-filesystem substrate. It does not complete C1 or Wave 1B-C and does not make bundle export available. After C1B 0/0/0 closure, the controller creates and independently reviews a separate C1C source-policy/disclosure plan before any further product code.
