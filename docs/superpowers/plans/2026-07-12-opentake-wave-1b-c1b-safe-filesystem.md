# OpenTake Wave 1B-C1B Safe Filesystem Capability Implementation Plan — Attempt 6

> **Execution rule:** use `superpowers:subagent-driven-development` for every task. Every task is RED → GREEN → focused commit → two fresh exact-SHA reviews. No implementation starts until this plan and both appendices are independently approved 0/0/0.

**Goal:** deliver a crate-private, recursive, byte-capable Linux/macOS/Windows filesystem authority substrate for C1C/C1D without re-enabling bundle export or adding ambient-path fallbacks.

**Bindings:** approved design `docs/superpowers/specs/2026-07-12-opentake-wave-1b-c-filesystem-security-design.md` at `31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace`; C1B baseline `e67917260ace36e4db1ede4e36eecbc401825bb1`.

**Normative appendices (part of this plan):**

- `docs/superpowers/plans/c1b/2026-07-12-c1b-common-unix-normative.md`: complete common enums/types/facade, recursive `DirectoryAuthority`, retained `FileCapability` byte I/O, Unix adapter, error/absence table, deterministic seams, quarantine/fail-leak protocol, tests, and Task 2A/2B/4/5 execution blocks.
- `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md`: complete Windows NT contracts, raw-status mapping, parsers, DACL, HANDLE-bound mutation, Task 6A/6B/7A/7B/7C patches, exact-SHA workflow/receipt YAML, repository validators, evidence schema, and remote BLOCKED rule.

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
- Unix post-create failure: after successful `mkdirat` or `openat(CREATE|EXCL)`, every ordinary metadata/filesystem/case/parent-duplication failure consumes the retained fd through same-parent random quarantine, identity verification, and removal, so the created name is absent. If identity cannot be established or either name is rebound, return typed `StageIdentityLost(RollbackCreatedEntry, Created*)`, preserve all unproven objects, and never delete by an ambient or unverified name.
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

Fresh reviewers write `spec-security-review.md` and `implementation-review.md`. Each records the exact evidence task ID (`3`, `4`, `5`, `6a`, `6b`, `7a`, `7b`, `7c`, or `8`, lowercase where alphanumeric), binds the full SHA, and states `Verdict: APPROVE`, `Critical: 0`, `Important: 0`, `Minor: 0`. Any finding produces a new correction commit using that task's exact GREEN commit subject, a new attempt directory, and both roles repeat; the validator accepts one or more consecutive same-subject GREEN corrections after the single task RED chain, but no unrelated or merge commit.

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

## Task 2A — Compile RED, then fail-closed substrate scaffold

Execute Common/Unix appendix section 7, Task 2A exactly. First make a test-only commit that adds only private `mod safe_fs;` to `lib.rs`; the exact `cargo check` must then fail with `E0583` because the module files are absent. GREEN is a separate commit that creates the private common modules plus selected adapters, all refusing authority acquisition. `component.rs` also refuses every component. No filesystem authority or product surface is enabled.

Commit only the appendix add set as `feat(project): add fail-closed C1B filesystem skeleton`; run the three target checks and both exact-SHA reviews before Task 2B.

## Task 2B — Common types, component validation, and unsupported adapter

Execute Common/Unix appendix section 7, Task 2B exactly. First make a test-only commit containing the single named component contract. Its evidence must show `running 1 test`, exactly one failure, and nonzero exit; `0 tests`, compile failure, or a missing module invalidates the RED. GREEN replaces only the temporary component refusal with the complete common validator. All acquisition adapters remain fail closed.

Commit `feat(project): validate C1B filesystem components`; run common tests, three-target compile/clippy, workspace check, and two fresh reviews. This task does not claim open/create/I/O/mutation behavior.

## Task 3 — Immutable-SHA native workflow

Execute Windows/CI appendix sections 18.7–18.8 exactly; its section 13–15 workflow, receipt, and validator contracts remain normative. The RED commit adds both validator tests plus two fail-closed validator scaffolds. The CI test must fail semantically on the missing immutable-SHA workflow binding, while the evidence test builds complete synthetic Task 3→Task 4→Task 5 predecessor/gate chains and fails only on the evidence-validator refusal. GREEN simultaneously installs the exact workflow, repository-versioned Windows expected-RED harness, and both complete validators; both test suites must pass. `actionlint` is blocking when available; otherwise the committed Ruby validators remain blocking. Use the appendix's exact Task 3 RED, review, receipt, and reviewed-stage manifest directory; do not substitute a Task 7A/7B/7C/8 path.

The three expanded jobs must checkout and assert the PR head SHA, immutable dispatch SHA, or main push SHA as applicable, emit the specified receipt schema, and never use a synthetic merge SHA for native evidence. Receipt IDs bind one-to-one to runner OS/label/architecture and to repository `appergb/OpenTake`, workflow `CI`, file `.github/workflows/ci.yml`, job `safe-filesystem`, and an allowed trigger event; synthetic mutation fixtures reject relabeling or foreign provenance. Commit `ci: verify C1B receipts and evidence on exact SHAs`, double review, and do not push or dispatch.

## Task 4 — Unix recursive namespace and platform-dispatched file I/O

From Task 4 onward, every blocking native gate uses one executable intake shape rather than per-platform ad hoc `results.json` files. After both GREEN-SHA reviews approve, create `$SAFETY_ROOT/branch-gates/c1b-task-<TASK>-<GREEN_SHA>-<16-lower-hex-NONCE>/`, run the ten local ledger commands, copy the two reviews under `reviews/`, and use Windows/CI appendix section 18.8's authenticated REST archive protocol to retain all three matrix artifacts (`linux-x86_64`, `macos-native`, `windows-x86_64`) from one run/attempt. Both reviews and `results.md` record the exact task; `results.md` also records frozen baseline `e67917260ace36e4db1ede4e36eecbc401825bb1` and the approved predecessor SHA. Invoke the already committed evidence validator with `TASK GREEN_SHA PREDECESSOR_SHA PREDECESSOR_PROOF`; its zero exit and gate `results.md` are the only native-intake approval. A task may emphasize its changed native OS in review, but no task invents a different receipt path or skips the other matrix receipts. If the exact SHA cannot be published/dispatched under existing authority, record `BLOCKED` and do not advance.

The predecessor binding is exact: Task 4 uses Task 3 GREEN plus its review-manifest directory; Task 5 uses the validated Task 4 branch gate; Task 6A uses the validated Task 5 branch gate; Task 6B uses the validated Task 6A branch gate; Tasks 7A/7B/7C use the validated Task 6B/7A/7B branch gate respectively; Task 8 uses the validated Task 7C branch gate. Every branch-gate proof has its three authenticated native receipts revalidated live, so a raw-exit/results/review text copy is insufficient. No older ancestor, free-form path, or relabeled same-SHA gate is accepted as `PREDECESSOR_PROOF`.

Execute Common/Unix appendix section 7, Task 4 exactly. The test-only commit adds exactly seventeen named platform-gated tests: seven authority/I/O tests (three common, two Linux-only, and two macOS-only) plus ten post-create rollback regressions. The current-host probe-matrix test and one ordinary post-create rollback test are both exact behavioral REDs and must each run once and fail against the approved Unix refusal adapter before implementation. GREEN implements full anchor-plus-child-scope rewalk, fail-closed local-filesystem/case proof, nonblocking nofollow query, recursive directory authority, create/open/enumerate, and platform-dispatched file read/write/seek/flush/sync/copy. It also installs create's internal all-or-capability rollback: after native creation, ordinary metadata/filesystem/case/parent-duplication failures use the retained fd, cross-platform `getentropy` kernel randomness, same-parent no-replace quarantine, repeated identity verification, and removal; deterministic retained-identity/name/quarantine/delete failures return the exact typed fail-leak reason without deleting an unproven name. All ten rollback tests must pass at this Task 4 GREEN SHA, so implementation is never reviewed without its regressions. Enumeration returns every validated component name, including symlink/reparse and special-entry names, but does not follow them or grant authority; validation callers query/reject metadata and cleanup callers obtain a separate consuming capability. Public mutation operations remain explicit typed refusal stubs.

Commit `feat(project): add Unix recursive filesystem authorities`; require same-SHA Linux and macOS native receipts plus both exact-SHA reviews. At the receipt gate, run the read-only authority probe from Windows/CI appendix section 14. Without explicit remote publication authority, write its exact BLOCKED record and stop without push/PR/dispatch or Task 5.

## Task 5 — Unix consuming quarantine, recursive cleanup, and publish

Execute Common/Unix appendix section 7, Task 5 exactly. The test-only commit adds exactly six public mutation/cleanup tests covering restore, occupied-name fail-leak, the explicit final name window, cleanup-capability identity recording, nested postorder cleanup of files/symlinks/FIFOs/directories, and destination collision preservation. The ten post-create rollback regressions remain owned by Task 4 and are not re-added here. RED runs only the named recursive-cleanup test and fails because the approved public mutation stubs refuse.

GREEN installs the move-only `StageCapability → QuarantinedCapability → CleanupCapability` state machine and bounded race seam. Unix quarantine is name-linearized, re-opened and identity-verified; mismatch restores once or fail-leaks. Final Unix name mutation remains inside the documented same-account boundary. Commit `feat(project): add Unix consuming quarantine cleanup`; require same-SHA Linux/macOS receipts and double review.

## Task 6A — Compile-complete fail-closed Windows platform scaffold

Execute Windows/CI appendix sections 16 and 18.1 exactly. This focused scaffold commit replaces Task 2A's Windows `include!("unsupported.rs")` with the complete Windows platform surface and target dependency. Every acquisition, I/O, DACL, quarantine, publish, and cleanup operation returns a structured fail-closed error; no authority can be acquired and no filesystem mutation occurs. After both exact-SHA reviews, create the normal `c1b-task-6a-<GREEN_SHA>-<NONCE>` branch gate with `PREDECESSOR_SHA=Task 5 GREEN` and `PREDECESSOR_PROOF=Task 5 gate`; all three authenticated native receipts and the same evidence validator are blocking before Task 6B.

## Task 6B — Windows recursive read/I/O substrate

Execute Windows/CI appendix sections 12, 16, 18.2, and the read/I/O portions of sections 2–7/9/11. The test-only commit adds exactly seven Task 6B-row bodies, `TestDir`/`name`/`root`, and the compile-only create-failure seam. `nested_retained_io_roundtrip` and `windows_post_create_metadata_failure_rolls_back_same_handle` are separate exact RED commands; each must compile, run once, and fail behaviorally at the Task 6A refusal. A compile failure or `0 tests` invalidates RED. GREEN replaces the compile-only seam with the final rollback implementation and all seven tests must pass before review.

GREEN adds only Windows acquisition/I/O production bodies: exact operation contracts, synchronous IOSB, case/volume/remote proof, bounds-checked directory/reparse parsers, NTSTATUS-first mapping, recursive authority, `CreatePermissions::Inherit`, and platform-dispatched I/O. It also owns the complete production fresh-mapping revalidation body and its Task 7B parent seam, plus `mark_delete_handle`, the all-or-capability create rollback helpers, and the pure `RenameInformationBuffer`/`map_rename_failure` helpers required for Task 7B's test-only parent to compile. Every successful `NtCreateFile(FILE_CREATE)` already holds `DELETE`; metadata/filesystem/type/case/snapshot/parent-duplicate failure uses that same HANDLE for disposition before returning. The six injected regressions prove ordinary failures leave the name absent, while the metadata test additionally injects disposition failure and requires typed `CreatedRollbackDeleteFailed` with the created entry deliberately retained. Query reports reparse metadata as present; enumeration returns every validated child name, including reparse names, without granting authority, while open rejects reparse traversal. OwnerOnly/DACL and every public mutation entry remain explicit compiling refusal bodies for Tasks 7A–7C; DACL and retained-delete future seams are added only with their Task 7A/7C test-only bodies to avoid dead code, and Task 6B's pure rename helpers do not perform namespace mutation. Later tasks add only their exact section-12 test rows and wire their owned public behavior without replacing Task 6B production helpers. Commit `feat(project): capture Windows filesystem capabilities`; expected-RED evidence uses the repository harness and REST intake at `$SAFETY_ROOT/red/c1b-task-6b-$TEST_SHA-$NONCE/`. Require the Task 6B native group, exact-SHA Windows receipt, three-target checks, product-closed checks, and two reviews.

## Task 7A — Windows OwnerOnly creation and DACL verification

Execute Windows/CI appendix Task 7A block exactly. Its test-only commit adds exactly the 22 Task 7A-row names in the one-owner matrix: 15 Task 6B pure bodies whose parent symbols now exist, plus seven OwnerOnly/DACL/security bodies and their test-only fixture seam. The first 15 run as exact PASS commands with individual logs at the test-only SHA; only the named OwnerOnly test is the behavioral RED against Task 6B's compiling permission refusal. GREEN adds only owner SID/ACL/security-descriptor construction, exact `BOOL`/pointer lifetimes, `READ_CONTROL` create access, bounds-first ACE/ACL/SID verification, and security rollback through Task 6B's same-DELETE-HANDLE path. OwnerOnly file, directory, and stage creation must pass natively; malformed wrong-type/undersized/oversized/invalid/null-owner plus out-of-range owner/DACL/ACL-size/ACE fixtures must fail before unsafe typed dereference and leave the created name absent; rename and delete remain typed refusals.

Commit `feat(project): enforce Windows owner-only creation`. Reviews use `$SAFETY_ROOT/logs/c1b-task-7a-$GREEN_SHA-attempt-$REVIEW_ATTEMPT/`, RED uses `$SAFETY_ROOT/red/c1b-task-7a-$TEST_SHA-$NONCE/`, and native intake uses the common per-task branch-gate protocol above; require its same-SHA three-receipt validator success before Task 7B.

## Task 7B — Windows quarantine and publish rename

Execute Windows/CI appendix Task 7B block exactly. Its test-only commit adds exactly five names: `every_revalidation_field_is_bound_before_mutation`, `quarantine_and_publish_refuse_changed_probe_without_mutation`, `quarantine_and_publish_success_do_not_self_conflict`, `rename_never_replaces_any_target_kind`, and `create_stage_collision_is_typed_and_preserves_original`. Mapping tests exercise the production body already committed in Task 6B; rename-buffer `used`/alignment/RootDirectory assertions are embedded in the success body, so there is no unowned sixth test. GREEN adds only consuming quarantine/publish rename using `NtSetInformationFile(FileRenameInformation)` relative to the retained parent, with no conflicting post-success reopen. Fresh full-chain mapping revalidation precedes quarantine and publish; collisions preserve every source/destination kind.

GREEN reuses Task 6B's already compiled pure buffer/mapping helpers and adds only retained-HANDLE `NtSetInformationFile(FileRenameInformation)` execution plus `quarantine_stage`/`publish_stage_noreplace`. Commit `feat(project): add Windows capability-relative rename`. Reviews use `$SAFETY_ROOT/logs/c1b-task-7b-$GREEN_SHA-attempt-$REVIEW_ATTEMPT/`, RED uses `$SAFETY_ROOT/red/c1b-task-7b-$TEST_SHA-$NONCE/`, and native intake uses the common per-task branch-gate protocol above; require its same-SHA three-receipt validator success before Task 7C.

## Task 7C — Windows retained-HANDLE cleanup and delete

Execute Windows/CI appendix Task 7C block exactly. Its test-only commit adds exactly `cleanup_quarantined_tree_deletes_nested_reparse_without_traversal` and `retained_delete_survives_real_name_rebound` against the Task 6B typed cleanup refusal. The latter uses the Task 7B parent symbol `RenameInformationBuffer.used`; its exact test-only SHA must compile, run one focused test, and fail only at `OpenCleanupEntry/UnsupportedTarget`. GREEN adds only cleanup capability acquisition and `NtSetInformationFile(FileDispositionInformation)` on the original DELETE-capable HANDLE. Leaf and final directory delete both pass all five `dispose_retained` arguments and use the same retained-delete hook policy. Recursive enumeration must hand every validated name, including reparse entries, to nofollow cleanup acquisition; deleting a retained reparse link must leave its target byte-identical.

Commit `feat(project): add Windows retained-handle cleanup`. Reviews use `$SAFETY_ROOT/logs/c1b-task-7c-$GREEN_SHA-attempt-$REVIEW_ATTEMPT/`, RED uses `$SAFETY_ROOT/red/c1b-task-7c-$TEST_SHA-$NONCE/`, and native intake uses the common per-task branch-gate protocol above; require its final same-SHA three-receipt validator success before Task 8.

## Task 8 — Exact-SHA convergence

Task 8 must not add, modify, or commit validator/test code. Create the final-SHA exclusive branch gate `$SAFETY_ROOT/branch-gates/c1b-<YYYYMMDDTHHMMSSZ>-<SHA>-<16-lower-hex-NONCE>/` and use Windows appendix section 15 command-ledger/results schema, then invoke the validators already committed and reviewed in Task 3 with `TASK=8` and `PREDECESSOR_SHA=FINAL_SHA=Task 7C GREEN SHA`.

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

Use authenticated `gh api --hostname github.com` REST calls to bind each receipt to `appergb/OpenTake` run/job/artifact metadata, retain each REST-downloaded `artifact.zip`, require its SHA-256 to equal the REST artifact digest, copy both reviews inside the gate, and then validate every gate-relative confined path plus task/baseline/predecessor/requested=checked-out=run=job=artifact-workflow-run=final SHA using `scripts/validate-c1b-evidence.rb` and Windows/CI appendix section 18.8. Spawn fresh filesystem/spec/security and implementation/quality/integration auditors; both bind Task 8 and final SHA and approve 0/0/0. `results.md` records task/baseline/predecessor/final SHA, pre/post clean status, all local gates, run/job/artifact IDs, attempts, digests, receipt SHAs, audits, aggregate. C1B completes only when results validation exits 0; C1/C1C–C1E/Wave 1B-C remain incomplete.

## Residual risks and stop rules

- Windows behavior is unverified until native receipt exists; cross-check cannot replace it.
- The workflow cannot run for a local-only object. Without explicit authorized PR/push or existing dispatchable remote SHA, native gate is BLOCKED.
- Unix has no portable fd-bound unlink/source rename. Quarantine/fail-leak narrows but does not erase the approved same-account final window.
- Linux allows only Ext/XFS/Btrfs; tmpfs and every unknown family fail closed as `UnknownFilesystem` until separate design review and native case-semantics evidence approve an addition.
- API mismatch, unavailable primitive, ambiguous mutation, malformed buffer, remote/unknown FS, unavailable case proof, or receipt mismatch fails closed and never permits a pathname fallback.
