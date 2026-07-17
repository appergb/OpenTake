# Task 11 report — DS-unix-consuming-tests

Base revision: `32f90c89555b4515fdf904bebef22b2088af70c4`

## Result

- Replaced the Linux/macOS `include!("unsupported.rs")` adapter with the
  normative capability-relative retained-fd implementation.
- Added no-follow acquisition/query/open, local-filesystem and case proof,
  complete namespace revalidation, regular-file I/O, kernel-random same-parent
  post-create rollback, no-replace quarantine/publish, identity-recording
  cleanup capabilities, and recursive consuming cleanup.
- Restored the complete Unix test group in the sole `safe_fs/tests.rs` runner:
  seven authority/I/O/probe names, ten post-create rollback names, and six
  public consuming names.
- Added a test-only Unix contract mutex. Every platform-gated Unix test holds it
  for its complete body, preserving all one-shot seam assertions while making
  ordinary parallel project/workspace gates deterministic.

## Files changed

- `Cargo.lock` — authorized mechanical `opentake-project` dependency edges for
  `libc` and `rustix`; no package/version drift.
- `crates/opentake-project/Cargo.toml` — target-gated exact pins
  `rustix = 1.1.4` (`fs`) and `libc = 0.2.186`.
- `crates/opentake-project/src/safe_fs/unix.rs`
- `crates/opentake-project/src/safe_fs/test_seam.rs`
- `crates/opentake-project/src/safe_fs/tests.rs`
- `docs/audit/2026-07-14/implementation-plans/data-safety-implementation.md`
- `docs/superpowers/plans/c1b/2026-07-12-c1b-common-unix-normative.md`
- `.superpowers/sdd/task-11-report.md`

The common capability/ops facade and the Windows adapter were not modified.

## RED evidence

Before adding tests, `cargo test -p opentake-project --lib safe_fs::tests --
--list` collected only the existing component test.

After adding tests but before changing production code:

- `cargo test -p opentake-project --lib safe_fs::tests::unix_contract -- --list`
  collected 21 native macOS tests. The two additional Linux-only probe tests
  were present in source and correctly not collected on macOS.
- Exact `macos_local_and_case_probe_matrix_is_enforced` ran one test and exited
  101 at the accepted-probe assertion with `UnsupportedTarget`.
- Exact `post_create_metadata_failure_removes_new_file` ran one test and exited
  101 at capture with `UnsupportedTarget`.
- Exact
  `nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories`
  ran one test and exited 101 at capture with `UnsupportedTarget`.

Each RED showed `running 1 test`, `0 passed; 1 failed`; no zero-test success was
used as evidence.

## GREEN verification

- Six public consuming tests, each exact and serialized: 6/6 passed.
- Ten post-create rollback tests, each exact and serialized: 10/10 passed.
- Five current-host authority/I/O/probe tests, each exact and serialized: 5/5
  passed.
- `cargo test -p opentake-project --lib safe_fs::tests::unix_contract --
  --test-threads=1`: 21/21 passed.
- `cargo test -p opentake-project -- --test-threads=1`: 141 unit tests plus all
  project integration suites passed.
- Default parallel `cargo test -p opentake-project --quiet`: passed twice
  consecutively after the test-only serialization fix (141 unit tests and all
  integration suites on each run).
- `cargo check -p opentake-project --lib --tests --target
  x86_64-unknown-linux-gnu`: passed.
- `cargo check -p opentake-project --lib --tests --target
  x86_64-pc-windows-msvc`: passed.
- `cargo clippy -p opentake-project --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo test --workspace --no-fail-fast`: passed. Cargo emitted only the
  pre-existing future-incompatibility notice for `block 0.1.6`.

One initial default-parallel project run reproduced a real test-harness race:
multiple Unix one-shot seam users raced, asserted, and poisoned a seam mutex.
The fix did not relax one-shot checks or production behavior; it serialized the
23 platform-gated Unix test bodies internally. The default project gate then
passed twice and the default workspace gate passed.

## Native platform limits

- Native behavior was executed on macOS only: 21 collected Unix tests.
- Linux source and its two Linux-only probe tests were cross-compiled, not run
  natively. A native Linux receipt remains required and is not claimed here.
- Windows was cross-compiled only to demonstrate that target gating and the
  untouched Windows adapter still compile.

## Self-review

- Rustfmt-normalized `unix.rs`, `test_seam.rs`, and `tests.rs` match their three
  normative code fences exactly.
- No production Unix operation joins or re-resolves an ambient cleanup path.
  Acquisition, query, open, rename, and unlink are descriptor-relative; query
  and identity reads use `SYMLINK_NOFOLLOW`.
- FIFO/special-node query and cleanup never open the entry for byte I/O.
- Post-create rollback names use `libc::getentropy`, are same-parent, and use
  no-replace rename; inability to prove retained/name identity returns typed
  `StageIdentityLost` and deliberately preserves the unproven object.
- Public quarantine restores or fail-leaks on verification ambiguity. Cleanup
  records identity in a move-only capability and performs a final no-follow
  identity read before consuming deletion.
- The final Unix read-to-name-syscall window remains explicit and is covered by
  the same-account-boundary regression; no stronger handle-bound claim is made.
- `Cargo.lock` contains only the two expected dependency-list additions.
- `git diff --check` passed and all changed paths are within the authorized Task
  11 ownership set (including the controller-authorized lockfile exception).
