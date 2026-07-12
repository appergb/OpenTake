# OpenTake Wave 1B-C1B Safe Filesystem Capability Implementation Plan — Attempt 3

> **Execution rule:** use `superpowers:subagent-driven-development` for every task. Every task is RED → GREEN → focused commit → two fresh exact-SHA reviews. No implementation starts until this plan and both appendices are independently approved 0/0/0.

**Goal:** deliver a crate-private, recursive, byte-capable Linux/macOS/Windows filesystem authority substrate for C1C/C1D without re-enabling bundle export or adding ambient-path fallbacks.

**Bindings:** approved design `docs/superpowers/specs/2026-07-12-opentake-wave-1b-c-filesystem-security-design.md` at `31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace`; C1B baseline `e67917260ace36e4db1ede4e36eecbc401825bb1`.

**Normative appendices (part of this plan):**

- `docs/superpowers/plans/c1b/2026-07-12-c1b-common-unix-normative.md`: complete common enums/types/facade, recursive `DirectoryAuthority`, retained `FileCapability` byte I/O, Unix adapter, error/absence table, deterministic seams, quarantine/fail-leak protocol, tests, and Task 2A/2B/4/5 execution blocks.
- `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md`: complete Windows NT contracts, raw-status mapping, parsers, DACL, HANDLE-bound mutation, exact-SHA workflow/receipt YAML, evidence schema, and remote BLOCKED rule.

A task brief cites this entry plus exact appendix sections. No unversioned source is normative.

## Frozen boundaries

- Integration worktree: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence`.
- Preserved dirty checkout: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake`; read-only.
- Review worktree: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review`.
- Safety root: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem`.
- One move-only recursive directory authority is returned by absolute capture, child open, and child create. Every returned directory can be a parent.
- `FileCapability` supports controlled read/write/seek/flush/sync and bounded retained-handle copy. Raw fd/HANDLE never leaves `safe_fs`.
- Only `query_child_nofollow` maps absence to `Ok(Absent)`; all other mappings follow the common appendix table and preserve operation/raw status.
- Unix cleanup: same-parent no-replace quarantine → nofollow reopen → identity verify. Mismatch gets one no-replace restore if the original name is free; collision/ambiguity fail-leaks quarantine and returns `StageIdentityLost`. Final identity-read→name-syscall hook is mandatory. No Unix API/test claims handle-identity-bound unlink/source rename.
- Windows delete/rename consume retained HANDLEs. Rename uses only `NtSetInformationFile(FileRenameInformation)` with `FILE_RENAME_INFORMATION.RootDirectory`.
- `export_bundle` Tauri registration and Web bundle mode remain absent at every commit.
- Forbidden: authority via `canonicalize`/`to_string_lossy`, joined-path child mutation, ordinary rename fallback, `DeleteFileW`, `RemoveDirectoryW`, `MoveFileExW`, and `SetFileInformationByHandle(FILE_RENAME_INFO)`.
- Native Linux/macOS/Windows receipts bind the exact reviewed SHA. Cross-target checks are compile evidence only.
- Local commits are authorized. Push, PR, dispatch, and remote mutation are not. At the first native gate, absent explicit remote authority, write the appendix BLOCKED record and stop.

## Linearization contract

- Unix quarantine: successful same-parent no-replace rename; source is name-linearized, then reopened/verified.
- Unix recovery: one quarantine→original no-replace restore; collision/ambiguity preserves all reachable names.
- Unix cleanup: final `unlinkat` after fresh identity read; same-account mutation in that deterministic hook window is outside the approved boundary.
- Unix publish: destination no-replace after C1D validation; source remains name-linearized.
- Windows delete: successful `NtSetInformationFile(FileDispositionInformation)` on retained object HANDLE.
- Windows rename: successful `NtSetInformationFile(FileRenameInformation)` on retained stage HANDLE and parent HANDLE.

## Universal task protocol

Run from the integration worktree. Require a clean start and follow each task's exact order. Task 2A records compile RED before its scaffold commit. Every later behavioral slice commits only the named failing tests, proves `running 1 test` plus nonzero RED, then makes a separate GREEN implementation commit. Capture every SHA, enforce the exact add set, then run `git diff --check "$BASE_SHA..$SHA"` and `git status --short`.

Create review directories exclusively (no `mkdir -p`):

```bash
SAFETY_ROOT='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem'
REVIEW_DIR="$SAFETY_ROOT/logs/c1b-task-$TASK-$SHA-attempt-$ATTEMPT"
mkdir "$REVIEW_DIR"
```

Fresh reviewers write `spec-security-review.md` and `implementation-review.md`. Each binds the full SHA and states `Verdict: APPROVE`, `Critical: 0`, `Important: 0`, `Minor: 0`. Any finding produces a new commit/attempt and both roles repeat.

## Task 1 — Three-target fixture lint cleanup

Modify only `crates/opentake-project/src/archive.rs`.

RED:

```bash
CARGO_TARGET_DIR=/tmp/opentake-c1b-task1-windows cargo clippy -p opentake-project --lib --tests --target x86_64-pc-windows-msvc -- -D warnings
```

Expected only dead Unix-only `external_entry`, `TestDir`, and `new`/`path` diagnostics. Gate `MediaManifestEntry`, `external_entry`, `TestDir`, its impl and Drop impl with `#[cfg(unix)]`; no body/behavior change and no allow.

GREEN:

```bash
cargo fmt --all --check
cargo clippy -p opentake-project --lib --tests --target aarch64-apple-darwin -- -D warnings
cargo clippy -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu -- -D warnings
cargo clippy -p opentake-project --lib --tests --target x86_64-pc-windows-msvc -- -D warnings
cargo test -p opentake-project --lib archive::tests -- --test-threads=1
git diff --check
git add crates/opentake-project/src/archive.rs
git commit -m "test: make project fixtures target-clean"
```

Double review before Task 2A.

## Task 2A — Compile-complete fail-closed substrate scaffold

Execute Common/Unix appendix section 7, Task 2A exactly. This is the only permitted compile-scaffold RED: the pre-change command must fail because `safe_fs` is absent, then GREEN creates the private common modules plus selected adapters that all refuse authority acquisition. `component.rs` also refuses every component. No filesystem authority or product surface is enabled.

Commit only the appendix add set as `feat(project): add fail-closed C1B filesystem skeleton`; run the three target checks and both exact-SHA reviews before Task 2B.

## Task 2B — Common types, component validation, and unsupported adapter

Execute Common/Unix appendix section 7, Task 2 exactly. First make a test-only commit containing the single named component contract. Its evidence must show `running 1 test`, exactly one failure, and nonzero exit; `0 tests`, compile failure, or a missing module invalidates the RED. GREEN replaces only the temporary component refusal with the complete common validator. All acquisition adapters remain fail closed.

Commit `feat(project): validate C1B filesystem components`; run common tests, three-target compile/clippy, workspace check, and two fresh reviews. This task does not claim open/create/I/O/mutation behavior.

## Task 3 — Immutable-SHA native workflow

Execute Windows/CI appendix section 16 item 1 and sections 13–15 exactly. The test-only commit adds only `scripts/validate-c1b-ci.rb` and its test; RED must be a semantic missing-SHA-binding assertion, not YAML syntax failure. GREEN changes only `.github/workflows/ci.yml` and must pass `actionlint`; if `actionlint` is unavailable, the appendix's fixed Ruby schema validator plus the committed test script are blocking.

The three expanded jobs must checkout and assert the PR head SHA, immutable dispatch SHA, or main push SHA as applicable, emit the specified receipt schema, and never use a synthetic merge SHA for native evidence. Commit `ci: bind safe filesystem receipts to immutable sha`, double review, and do not push or dispatch.

## Task 4 — Unix recursive namespace and platform-dispatched file I/O

Execute Common/Unix appendix section 7, Task 3 exactly. The test-only commit adds the three named blocking tests; the focused RED must actually run and fail one test. GREEN implements full anchor-plus-child-scope rewalk, local-filesystem/case proof, nonblocking nofollow query, recursive directory authority, create/open/enumerate, and platform-dispatched file read/write/seek/flush/sync/copy. Mutation operations remain explicit typed refusal stubs.

Commit `feat(project): add Unix recursive filesystem authorities`; require same-SHA Linux and macOS native receipts plus both exact-SHA reviews. At the receipt gate, run the read-only authority probe from Windows/CI appendix section 14. Without explicit remote publication authority, write its exact BLOCKED record and stop without push/PR/dispatch or Task 5.

## Task 5 — Unix consuming quarantine, recursive cleanup, and publish

Execute Common/Unix appendix section 7, Task 4 exactly. The test-only commit covers restore, occupied-name fail-leak, the explicit final name window, nested postorder cleanup of files/symlinks/FIFOs/directories, and destination collision preservation. RED must run one named test and fail because the approved mutation stubs refuse.

GREEN installs the move-only `StageCapability → QuarantinedCapability → CleanupCapability` state machine and bounded race seam. Unix quarantine is name-linearized, re-opened and identity-verified; mismatch restores once or fail-leaks. Final Unix name mutation remains inside the documented same-account boundary. Commit `feat(project): add Unix consuming quarantine cleanup`; require same-SHA Linux/macOS receipts and double review.

## Task 6 — Windows recursive read/I/O substrate

Execute Windows/CI appendix section 16 item 2 using the unified common facade. The test-only commit adds the complete Windows fixture/spy/race helpers and read/I/O tests without production NT bodies. The appendix `expect_red` function must prove each focused test ran exactly once and failed behaviorally.

GREEN adds Windows appendix sections 2–8 and 11 production bodies: exact operation contracts, synchronous IOSB, case/volume/remote proof, bounds-checked directory/reparse parsers, NTSTATUS-first mapping, recursive authority, and platform-dispatched I/O. Query reports reparse metadata as present; open rejects it. Commit `feat(project): capture Windows filesystem capabilities`; require the full native Windows test group, exact-SHA Windows receipt, three-target checks, product-closed checks, and two reviews.

## Task 7 — Windows DACL and retained-HANDLE mutation

Execute Windows/CI appendix section 16 item 3. The test-only commit adds the complete DACL/delete/rename tests and must produce three behavioral RED receipts through `expect_red`. GREEN adds appendix sections 8–10 mutation bodies. Protected owner DACL storage outlives `NtCreateFile`; cleanup and publication consume the original DELETE-capable HANDLE; rename uses only `NtSetInformationFile(FileRenameInformation)` relative to the retained parent.

Commit `feat(project): add Windows capability-relative mutations`; require full Windows native tests, same-SHA receipt, all common/product-closed gates, and two reviews.

## Task 8 — Exact-SHA convergence

Create exclusive branch gate `$SAFETY_ROOT/branch-gates/c1b-<UTCSTAMP>-<SHA>-<NONCE>/` and use Windows appendix section 15 command-ledger/results schema.

Run with per-command log/raw-exit:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check -p opentake-tauri --no-default-features --all-targets
cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
cargo test -p opentake-project --test archive_security -- --test-threads=1
cargo check -p opentake-project --lib --tests --target aarch64-apple-darwin
cargo check -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu
cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc
git diff --check
```

Validate three receipt schemas, unique IDs, requested=checked-out=final SHA, all command/aggregate exits 0. Spawn fresh filesystem/spec/security and implementation/quality/integration auditors; both bind final SHA and approve 0/0/0. `results.md` records baseline/final SHA, pre/post clean status, all local gates, run IDs/attempts/receipt SHAs, audits, aggregate. C1B completes only when results validation exits 0; C1/C1C–C1E/Wave 1B-C remain incomplete.

## Residual risks and stop rules

- Windows behavior is unverified until native receipt exists; cross-check cannot replace it.
- The workflow cannot run for a local-only object. Without explicit authorized PR/push or existing dispatchable remote SHA, native gate is BLOCKED.
- Unix has no portable fd-bound unlink/source rename. Quarantine/fail-leak narrows but does not erase the approved same-account final window.
- Filesystem allowlists intentionally reject unknown families; adding one needs separate design/native evidence.
- API mismatch, unavailable primitive, ambiguous mutation, malformed buffer, remote/unknown FS, unavailable case proof, or receipt mismatch fails closed and never permits a pathname fallback.
