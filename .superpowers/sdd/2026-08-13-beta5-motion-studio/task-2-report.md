# Motion Studio Task 2 report

Status: COMPLETE — independently reviewed and approved before commit.

## Scope

- Added a project-confined Motion Studio document store for the only editable
  sources, `index.html` and `styles.css`, plus a bounded typed manifest and
  revision catalog.
- Added a visible bilingual starter template, LF normalization, exact SHA-256
  revision hashes, bounded non-overlapping byte-offset edits, stale-baseline
  rejection, and expected-result verification.
- Published immutable revision directories through one synced atomic catalog
  replacement. Failed publication removes the unpublished revision and leaves
  the prior catalog/revision readable after restart.
- A post-replacement directory-sync failure is returned to the caller as a
  durability error while retaining the now-published revision; immediate and
  restarted reads converge on that catalog instead of deleting its target.
- Opened the current project and every nested directory/file through retained
  no-follow `cap-std` authorities. IDs, titles, directory names, sources,
  parameters, manifests, catalogs, and patch counts all have explicit bounds.
- Added Windows by-handle replacement with `ReplaceIfExists`, avoiding the
  non-overwriting behavior of `std::fs::rename` on Windows.
- Registered four asynchronous Tauri commands. Blocking capability I/O and
  fsync work runs through `spawn_blocking`. Each command captures the exact
  retained project authority before it can queue and revalidates that authority
  after obtaining the store/publication/identity gates.
- Serialized Motion component commits against generation's complete-bundle
  replacement so neither workflow can publish from a stale source tree. The
  publication gate is released before synchronous core events, with a
  subscriber re-entry regression to prevent deadlock.
- Opens Unix inputs with `O_NONBLOCK`, then rejects non-regular entries, so a
  corrupt project FIFO cannot hang the blocking pool or store mutex.
- Manifest pretty-encoding is bounded before any revision directory is made;
  patch offsets are explicitly UTF-8 byte offsets and must be codepoint
  boundaries (the web adapter will convert CodeMirror UTF-16 positions).
- Extended complete Save As, same-target publication, generated-media
  publication, and archive collection so project-local Motion documents remain
  inside the `.opentake` bundle.

## TDD evidence

Initial RED:

```text
cargo test -p opentake-tauri motion_documents::tests --lib
compile failed: MotionDocumentStore, DTOs, commands, and hash helpers absent
```

Save As RED:

```text
cargo test -p opentake-project complete_publish_carries_motion_documents_across_save_as
failed: destination motion-documents/catalog.json did not exist
```

Line-ending RED:

```text
cargo test -p opentake-tauri motion_documents::tests::normalizes_crlf_and_lone_cr_before_hashing_and_persistence --lib
failed: expected normalized result hash did not match
```

Final fresh GREEN:

```text
cargo test -p opentake-tauri motion_documents:: --lib -- --nocapture
15 passed

cargo test -p opentake-core
72 library + 9 integration tests passed; 0 failed

cargo test -p opentake-project
167 library + 41 integration tests passed; 0 failed

cargo test -p opentake-tauri --lib
690 passed; 0 failed

cargo clippy -p opentake-core -p opentake-project -p opentake-tauri --all-targets -- -D warnings
passed

cargo fmt --all -- --check
git diff --check
passed
```

Windows cross-check was attempted with:

```text
cargo check -p opentake-tauri --lib --no-default-features --target x86_64-pc-windows-msvc
```

It stopped in third-party `ring 0.17.14` before project code because the local
macOS cross toolchain has no MSVC `assert.h`. The checked-in Windows branch uses
the same retained-root/by-handle rename contract already exercised by the
project transaction layer; Windows CI remains the executable platform gate.

## Review

Independent review found and the implementation fixed: queued IPC requests
crossing project replacement, generated complete-bundle publication losing a
Motion commit, blocking FIFO opens, swallowed post-rename directory-sync
errors, oversized pretty manifests, and ambiguous UTF-8 edit offsets. A later
review found the first publication gate was held during synchronous core event
emission; it is now dropped before broadcasting and covered by a re-entrant
subscriber test.

Final re-review verdict: **Spec PASS / Quality PASS / APPROVE**, with zero
critical, high, medium, or low findings. The reviewer independently re-ran the
publication, Motion store, project carry, strict Clippy, formatting, and diff
gates. Windows runtime execution remains CI-owned because the local macOS
cross-toolchain stops in third-party `ring` before project code.

## Commit

Pending: `feat(motion): persist confined HTML and CSS documents`.
