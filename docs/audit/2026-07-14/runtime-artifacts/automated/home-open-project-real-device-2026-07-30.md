# Home open-project acceptance — 2026-07-30

Scope: Home Shell Task 10 (`implementation-slice-526c91ba76edbda6`) on the exact arm64 debug bundle at `target/debug/bundle/macos/OpenTake.app`.

## Code verification

- RED: the three exact candidate tests reached the `home.opening` state but failed because their controls remained enabled while `openProjectViaDialog` was pending.
- GREEN: the sidebar, empty-launcher, and populated-launcher controls now share one Home-level pending state and expose native `disabled`/`aria-busy` state until the open attempt settles. All Open, New, and recent-project entries are disabled together, a second activation cannot start another dialog, and a rejection test proves recovery to the enabled label.
- Project-open failures preserve the current front-end state, surface the structured Tauri error message through the localized Home toast, and remain rejected to non-UI callers.
- `AppCore::prepare_project_open` runs in `spawn_blocking` behind a 15-second timeout; the prepared value is committed only after successful completion. A late timed-out worker has no reference to the live core and cannot replace the current project. A managed single-flight lifecycle coordinator also rejects overlapping Open/New transitions before either can supersede the other.
- Focused results: 26/26 Home/project-action tests passed; 6/6 Rust project-open/lifecycle tests passed, including single-flight exclusion, off-thread execution, timeout/no-late-commit, failure preservation, successful activation, and mapped-media acceptance.
- Regression results: 72 Web files / 737 tests passed; production Web build passed; default and `--no-default-features` Tauri checks passed; both CI-equivalent Clippy gates passed with `-D warnings`; `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast` passed, with only the repository's pre-existing future-incompatibility warning for `block v0.1.6` and the explicitly ignored real-device probe tests.

## Real-device verification

1. Activated the Home sidebar Open Project control with a native double-click. Exactly one macOS directory sheet appeared.
2. In populated Home, activated the hero Open Project control with a native double-click. Exactly one directory sheet appeared.
3. Removed the recent entry only, entered the empty-launcher state, and activated its Open Project control with a native double-click. Exactly one directory sheet appeared.
4. Selected `/Users/lvbaiqing/Documents/OpenTake/未命名.opentake/Untitled.opentake`, whose visible components carry iCloud `Not downloaded` state and whose root `open(2)` previously froze the application. The sidebar control became disabled and announced `正在打开…`; the exact debug process remained responsive while the filesystem call stayed isolated off the main thread.
5. At the 15-second boundary the control returned to `打开项目` and the UI exposed `打开失败：project open timed out after 15s`. No editor transition or recent entry was published for the failed bundle.
6. In the same process, opened the preserved local bundle `/private/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake`. The editor showed media `talking-head-30s` and duration `00:29:20`, proving the timeout did not corrupt or supersede the valid session.
7. Restored `TalkingHeadQA` to Home recents after the empty-state check. The FileProvider-backed `Untitled.opentake` bundle was not modified or deleted.

Result: PASS. All three planned controls have pending, cancel/retry, success, and explicit failure/recovery evidence; a non-responsive cloud-backed directory no longer freezes the application or mutates the active project.
