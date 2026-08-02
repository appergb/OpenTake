# Data Safety Task 12 — native Windows safe-fs receipt (2026-07-31)

Status: **native implementation and focused Windows acceptance complete; whole-workflow and independent-review closure pending.**

## Bound contract

- Plan: `data-safety-implementation.md`, Task 12
  (`implementation-slice-c7f1cd8463f97ad5`).
- Exact tests:
  `crates/opentake-project/src/safe_fs/tests.rs#windows_contract` and
  `crates/opentake-project/src/safe_fs/tests.rs#synchronous_nt_pending_is_invariant_error`.
- Exact source SHA: `a1e9fd0ba30ac3471dd106dbe5307f18c49cf5ee`.

## Native Windows receipt

- Workflow: [run 30617000877](https://github.com/appergb/OpenTake/actions/runs/30617000877).
- Job: [Safe filesystem (windows-x86_64)](https://github.com/appergb/OpenTake/actions/runs/30617000877/job/91112395644), success.
- Artifact: `c1b-native-windows-x86_64-a1e9fd0ba30ac3471dd106dbe5307f18c49cf5ee`, ID `8787791443`, 3,197 bytes, server digest `sha256:ed079ecf16dacc28a3f1f6775d295ddf5a5ed699869994591f90d5387991ed60`.
- Receipt binds both `requested_sha` and `checked_out_sha` to the exact source
  SHA, run attempt 1, Windows Server 2022 x64, and aggregate exit 0.

All retained command exits were zero:

```text
cargo fmt --all --check                                           0
cargo clippy -p opentake-project --lib --tests -- -D warnings     0
cargo test -p opentake-project --lib safe_fs -- --test-threads=1  0
cargo test -p opentake-project --test archive_security -- --test-threads=1  0
```

The native safe-fs runner executed 26 tests with 26 passed, 0 failed. This
includes both exact Task 12 tests plus retained-handle I/O, owner-only DACL
validation and malformed descriptor rejection, same-handle rollback at every
post-create validation point, quarantine/publish without self-conflict,
no-replace rename against every target kind, recursive reparse-point cleanup,
access non-escalation, and retained deletion without following a rebound source
name.

## Pre-GREEN failure and correction

The prior exact run
[30616574158](https://github.com/appergb/OpenTake/actions/runs/30616574158)
retained all four command exits and failed the aggregate because one newly
added test required a leaf rename even when the filesystem rejected the
same-handle simulation with `STATUS_SHARING_VIOLATION`. A later parallel suite
also proved that Windows can allow that same simulation after the retained open.
The corrected test accepts both native outcomes while asserting the security
invariant in each: if rebinding is blocked, the original name disappears; if it
succeeds, consuming deletion removes the retained original and preserves the
new replacement at that name. Production share or deletion behavior was not
weakened to make the test pass.

## Remaining closure

The focused native contract is GREEN. Task 12 remains open until the remaining
Windows jobs and the whole exact-SHA workflow finish successfully and the
plan's independent-review criterion has a valid receipt. This file therefore
does not authorize Beta publication by itself.
