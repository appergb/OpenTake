# Task 11 report — DS-unix-consuming-tests

Base revision: `32f90c89555b4515fdf904bebef22b2088af70c4`

## Result — safe partial implementation; Task 11 `REJECTED/BLOCKED`

- Replaced the Linux/macOS `include!("unsupported.rs")` adapter with a
  capability-relative retained-fd implementation for the portable subset.
- Added no-follow acquisition/query/open, local-filesystem and case proof,
  complete namespace revalidation, regular-file I/O, kernel-random same-parent
  post-create rollback, no-replace quarantine/publish, identity-recording
  cleanup capabilities, and recursive consuming cleanup.
- Enforced authority monotonicity: a `Read` directory may derive only a `Read`
  child directory or file; exact escalation attempts return `AccessMismatch`.
- Production Unix directory and stage creation now returns exact
  `UnsupportedAtomicPublish(PrimitiveUnavailable)` before namespace mutation.
  The former `mkdirat` then `openat` bodies are `#[cfg(test)]` and explicitly
  named `*_trusted_fixture`; they seed downstream algorithm tests only.
- Restored and reconciled the Unix test group in the sole `safe_fs/tests.rs`
  runner: eleven authority/access/I/O/probe/refusal names, ten post-create
  rollback names, and six consuming names. macOS collects 25; two additional
  Linux-only probes are present in source.
- Strengthened recursive cleanup with an out-of-quarantine symlink canary, and
  destination-collision checks with byte/type/emptiness/symlink-target
  preservation assertions for every destination kind.
- Added precise `// SAFETY:` justifications to both native unsafe calls and
  replaced owned-fd raw conversions with safe `File::from(OwnedFd)`.
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

## RED evidence — post-hoc isolated replay only

The original Task 11 turn did not create the normative separate test-only commit
or durable receipt. Its interactive RED observations therefore do **not** satisfy
the section-7 test-only-commit protocol. The later isolated, hash-bound replay is
recorded in [`task-11-red-replay.md`](task-11-red-replay.md). It makes the failure
boundaries reproducible without rewriting history, but does not cure the missing
original protocol evidence; Task 11 remains `REJECTED/BLOCKED`.

The following are the non-durable original interactive observations, retained
only as context and not treated as protocol evidence.

Before adding tests, `cargo test -p opentake-project --lib safe_fs::tests --
--list` collected only the existing component test.

After adding the original tests but before changing production code:

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

Each observation showed `running 1 test`, `0 passed; 1 failed`; the hash-bound
post-hoc replay independently reproduced those markers, but no original receipt
or test-only commit exists.

The replay also covers the independent-review RED/GREEN boundary:

- Exact `read_parent_cannot_escalate_child_directory_access` and
  `read_parent_cannot_escalate_file_access` each ran once and exited 101 against
  the original implementation, which incorrectly granted the stronger child
  capability. Both pass after the access-lattice fix.
- Exact `create_directory_typed_refuses_before_namespace_mutation` and
  `create_stage_directory_typed_refuses_before_namespace_mutation` each ran once
  and exited 101 against the pre-reconciliation implementation, which created
  the requested name. Both now pass and additionally prove the name is absent.

## GREEN verification

- Six consuming tests, each exact and serialized through trusted fixture
  creation: 6/6 passed. This is downstream algorithm evidence, not production
  directory-create evidence.
- Ten post-create rollback tests, each exact and serialized: 10/10 passed.
- Nine current-host authority/access/I/O/probe/refusal tests, each exact and
  serialized: 9/9 passed.
- `cargo test -p opentake-project --lib safe_fs::tests::unix_contract --
  --test-threads=1`: 25/25 passed.
- Default-parallel `cargo test -p opentake-project --quiet`: 145 unit tests and
  every project integration suite passed.
- `cargo check -p opentake-project --lib --tests --target
  x86_64-unknown-linux-gnu`: passed.
- `cargo check -p opentake-project --lib --tests --target
  x86_64-pc-windows-msvc`: passed.
- `cargo clippy -p opentake-project --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo test --workspace --no-fail-fast`: passed, including the workspace
  integration and doc-test gates; only explicitly hardware-dependent probes
  remained ignored by their existing declarations.

One initial default-parallel project run reproduced a real test-harness race:
multiple Unix one-shot seam users raced, asserted, and poisoned a seam mutex.
The fix did not relax one-shot checks or production behavior; it serialized the
27 source platform-gated Unix test bodies internally. The default project gate then
passed twice and the default workspace gate passed.

## Architecture ruling and blocked contract

Linux and macOS do not expose a portable operation that both creates a directory
and returns an identity-bearing descriptor for that exact creation. The previous
sequence performed `mkdirat(parent, name)` and then `openat(parent, name)`. A
same-account namespace actor can rename the created directory and bind a
replacement between those calls. `mkdirat` returns no identity that can be
compared with the later fd, so the implementation cannot prove that the opened
object is the one it created. A randomized temporary name reduces guessability
but merely moves the race and does not supply identity proof.

The architecture ruling therefore selected strict fail-closed reconciliation:

- production directory/stage create typed-refuses before any namespace mutation;
- regular-file `openat(CREATE|EXCL)` creation remains, because that syscall
  returns the fd for the object it atomically created;
- old directory-create code is compiled only for tests and explicitly labeled
  trusted fixture code;
- rollback/quarantine/publish/cleanup tests prove those downstream algorithms
  only and do not satisfy the production directory-create contract.

Task 11 is consequently `REJECTED/BLOCKED`, even when all validation gates pass.
Future architectural options are a single-file fd-backed container or a
privileged/private namespace broker.

## Native platform limits

- Native behavior was executed on macOS only: 25 collected Unix tests.
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
- Production regular-file post-create rollback names use `libc::getentropy`, are
  same-parent, and use no-replace rename; inability to prove retained/name
  identity returns typed `StageIdentityLost` and deliberately preserves the
  unproven object. Directory rollback is test-fixture-only.
- Public quarantine restores or fail-leaks on verification ambiguity. Cleanup
  records identity in a move-only capability and performs a final no-follow
  identity read before consuming deletion.
- The final Unix read-to-name-syscall window remains explicit and is covered by
  the same-account-boundary regression. It is name-linearized, outside the
  approved threat boundary, and provides no no-data-loss guarantee against a
  same-account namespace actor; no stronger handle-bound claim is made.
- `Cargo.lock` contains only the two expected dependency-list additions.
- `git diff --check` passed and all follow-up paths remain within the authorized
  Task 11 implementation, test, normative, ledger, and report scope.
