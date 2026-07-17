# Task 11 isolated RED replay evidence

Date: 2026-07-17 (Asia/Shanghai)

Host/toolchain: `Darwin 25.5.0 arm64`; `rustc 1.96.0
(ac68faa20 2026-05-25)`; `cargo 1.96.0 (30a34c682 2026-05-25)`.

## Evidence status and limitation

This is a post-hoc isolated replay. The original Task 11 execution did not
create the required separate test-only commit or persistent RED receipt. This
replay deliberately does not rewrite that history and does not satisfy the
normative separate-test-commit protocol. It only demonstrates, on hash-bound
detached trees, that the named tests fail at the intended pre-implementation
boundaries. Task 11 remains `REJECTED/BLOCKED`.

Both temporary worktrees were detached, contained only the test-layer changes
listed below, and were removed after the replay. No implementation patch was
applied in either replay.

## Replay A — original Unix tests against the unsupported adapter

- Base/HEAD: `32f90c89555b4515fdf904bebef22b2088af70c4`
- Test-layer source: selected paths from
  `e8d6b1640c58ed915c4f5013b065d22c2ff07980`
- Patch SHA-256:
  `4f636fc7542089184dec13f78bab8469b4be24244a49c3840574d7e44984314a`
- Staged replay tree: `b61214e9fad667b10b7cb557461390c30c2f168e`
- Unchanged `safe_fs/unix.rs` blob:
  `866b5b4fa5f5d8a723c055e2d4fcd3b4e2ecb5bc` (equal to the base tree)

The patch bytes were produced and applied with:

```bash
git worktree add --detach /tmp/opentake-task11-red-replay-a-20260717 32f90c89555b4515fdf904bebef22b2088af70c4
git diff --binary 32f90c89555b4515fdf904bebef22b2088af70c4 e8d6b1640c58ed915c4f5013b065d22c2ff07980 -- Cargo.lock crates/opentake-project/Cargo.toml crates/opentake-project/src/safe_fs/test_seam.rs crates/opentake-project/src/safe_fs/tests.rs | tee /tmp/opentake-task11-red-replay-a.patch | shasum -a 256
git -C /tmp/opentake-task11-red-replay-a-20260717 apply --index /tmp/opentake-task11-red-replay-a.patch
git -C /tmp/opentake-task11-red-replay-a-20260717 write-tree
```

The staged path/blob inventory was:

| Path | Git blob |
|---|---|
| `Cargo.lock` | `76834601723941fdaba82f78d4a92cc3329c4786` |
| `crates/opentake-project/Cargo.toml` | `a96e4d2c582549d549c0bc3d1e26f8648bd0c2b2` |
| `crates/opentake-project/src/safe_fs/test_seam.rs` | `83b31292d5a54bfde4a9ba0891b751664f589e96` |
| `crates/opentake-project/src/safe_fs/tests.rs` | `e46d7b19d50ccd93ed168d628ff195371c9d0b23` |

Each command below exited `101` and printed `running 1 test` plus `0 passed;
1 failed`:

| Exact command | Actual failure boundary |
|---|---|
| `cargo test -p opentake-project --lib safe_fs::tests::unix_contract::macos_local_and_case_probe_matrix_is_enforced -- --exact --test-threads=1` | `tests.rs:313`: expected the injected `MNT_LOCAL`/case sample to be accepted; capture returned `UnsupportedSecureFilesystem { operation: CaptureNamespaceRoot, reason: UnsupportedTarget }`. |
| `cargo test -p opentake-project --lib safe_fs::tests::unix_contract::post_create_metadata_failure_removes_new_file -- --exact --test-threads=1` | `tests.rs:458`: fixture capture failed with the same typed `UnsupportedTarget` before file creation. |
| `cargo test -p opentake-project --lib safe_fs::tests::unix_contract::nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories -- --exact --test-threads=1` | `tests.rs:763`: fixture capture failed with the same typed `UnsupportedTarget` before stage creation/cleanup. |

## Replay B — independent-review regressions against `e8d6b16`

- Base/HEAD: `e8d6b1640c58ed915c4f5013b065d22c2ff07980`
- Test-layer source: only the four-test hunk from
  `304acf1f12f259e27b4cb5d16b471d0c21c018a0`; no `unix.rs` hunk
- Patch SHA-256:
  `343908f4174ce2c74cba5d7cd6e57ab939ab055e0a9e55464d6e5c54b5d53981`
- Staged replay tree: `62fbd808d85f45344f06bf7a1ab01d2f437b241b`
- Patched `tests.rs` blob:
  `e0949ce54904a7cf2543aa771243bf2d95e1e6d6`
- Unchanged `safe_fs/unix.rs` blob:
  `0eea2def4d6b9bc3567c9cdc90bb887f432b11d2` (equal to the base tree)

The patch bytes were produced by retaining only the second tests-file hunk and
then applied:

```bash
git worktree add --detach /tmp/opentake-task11-red-replay-b-20260717 e8d6b1640c58ed915c4f5013b065d22c2ff07980
git diff --unified=3 e8d6b1640c58ed915c4f5013b065d22c2ff07980 304acf1f12f259e27b4cb5d16b471d0c21c018a0 -- crates/opentake-project/src/safe_fs/tests.rs | awk 'NR <= 4 { print; next } /^@@ -396,6 \+399,79 / { keep=1 } keep && /^@@ / && $0 !~ /^@@ -396,6 \+399,79 / { exit } keep { print }' > /tmp/opentake-task11-red-replay-b.patch
shasum -a 256 /tmp/opentake-task11-red-replay-b.patch
git -C /tmp/opentake-task11-red-replay-b-20260717 apply --index /tmp/opentake-task11-red-replay-b.patch
git -C /tmp/opentake-task11-red-replay-b-20260717 write-tree
```

Each command below exited `101` and printed `running 1 test` plus `0 passed;
1 failed`:

| Exact command | Actual failure boundary |
|---|---|
| `cargo test -p opentake-project --lib safe_fs::tests::unix_contract::read_parent_cannot_escalate_child_directory_access -- --exact --test-threads=1` | `tests.rs:408`: `Read` parent successfully derived a `MutateChildren` child instead of exact `AccessMismatch(OpenDirectory)`. |
| `cargo test -p opentake-project --lib safe_fs::tests::unix_contract::read_parent_cannot_escalate_file_access -- --exact --test-threads=1` | `tests.rs:425`: `Read` parent successfully derived a `ReadWrite` file instead of exact `AccessMismatch(OpenFile)`. |
| `cargo test -p opentake-project --lib safe_fs::tests::unix_contract::create_directory_typed_refuses_before_namespace_mutation -- --exact --test-threads=1` | `tests.rs:439`: the old directory-create path completed instead of returning exact `UnsupportedAtomicPublish { operation: CreateDirectory, reason: PrimitiveUnavailable }`. |
| `cargo test -p opentake-project --lib safe_fs::tests::unix_contract::create_stage_directory_typed_refuses_before_namespace_mutation -- --exact --test-threads=1` | `tests.rs:461`: the old stage-create path completed instead of returning exact `UnsupportedAtomicPublish { operation: CreateStageDirectory, reason: PrimitiveUnavailable }`. |

## Cleanup

After recording the hashes and results, the detached worktrees were removed
with `git worktree remove --force` and pruned. The two untracked patch files in
`/tmp` were deleted. The primary repository was not modified by replay commands.
