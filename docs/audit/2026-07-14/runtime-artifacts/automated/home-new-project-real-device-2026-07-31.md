# Home new-project acceptance — 2026-07-31

Scope: Home Shell Task 9 (`implementation-slice-60d775675af9091c`) on the exact arm64 debug bundle at `target/debug/bundle/macos/OpenTake.app`.

## Code verification

- RED: the three exact candidate tests failed because activating New Project exposed no shared `home.creating` state and did not disable the other Home lifecycle controls while creation was pending.
- The previous native flow called `project_new` before the initial `project_save(path)`. A failed first save could therefore discard the live project before reporting the failure.
- GREEN: all sidebar, empty-launcher, and populated-launcher New Project controls now share the Home lifecycle single-flight state with Open Project and recent cards. During creation, both New controls announce `正在创建…`; every New/Open/recent entry is natively disabled with `aria-busy`; settlement restores all controls.
- `project_new(path)` now creates and saves a separate fresh `AppCore` on a blocking worker, reopens the resulting bundle, and commits the prepared project only after the entire operation succeeds. A 15-second lifecycle timeout and the shared coordinator prevent main-thread stalls and overlapping New/Open publication. The no-path browser fallback remains supported.
- Failure tests prove that an invalid destination leaves the live Rust core, front-end project snapshot, media state, and Home view unchanged while exposing the localized structured error toast. Successful creation returns the committed bundle path directly and no longer performs a second `project_save` call.
- Focused results: 31/31 Home/project-action tests passed and 5/5 Rust async lifecycle tests passed. Each of the three plan-prescribed candidate commands executed successfully.
- Regression results: 72 Web files / 742 tests passed; production Web build passed; default and `--no-default-features` Tauri checks passed; both Clippy gates passed with `-D warnings`; `cargo fmt --all -- --check && cargo test --workspace --no-fail-fast` passed. Remaining output is limited to the repository's pre-existing `block v0.1.6` future-incompatibility warning, the known Vite chunk/dynamic-import warnings, and explicitly ignored hardware probe tests.

## Real-device verification

1. Started from populated Home and activated the sidebar `新建项目`. The native macOS save sheet opened. Cancelling it returned to Home with all controls enabled and no project/recent mutation.
2. Repeated the sidebar action, chose `/private/tmp/Task9Sidebar-20260731.opentake`, and saved. The app entered a fresh editor with an empty media library and `00:00:00` timeline. The bundle contains valid `project.json` and `media.json`; the timeline is 1920×1080 at 30 fps with zero tracks.
3. Returned to populated Home and activated the hero `新建项目`, saving `/private/tmp/Task9Populated-20260731.opentake`. The same fresh-editor and on-disk bundle checks passed.
4. Removed recent entries only (the bundles remained untouched) to expose the empty launcher, then activated its `新建项目` and saved `/private/tmp/Task9Empty-20260731.opentake`. The app entered the fresh editor and produced the same valid bundle structure.
5. Returned Home and double-clicked the `Task9Empty-20260731` recent card. The just-created bundle reopened successfully in the same packaged process, proving the first save produced a reusable project rather than a UI-only session.
6. Selected `/private/tmp` itself through the native Open Project picker as an invalid bundle. Home exposed `打开失败：missing required project.json in bundle at /private/tmp`, remained responsive, and recovered all lifecycle controls. This confirms the shared failure/retry path after the Task 9 state refactor.
7. Terminated duplicate stale debug/installed processes that shared the bundle identifier, relaunched exactly one current debug bundle, and repeated the final checks to exclude automation routing ambiguity.
8. Reopened the preserved `/private/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake`; the editor showed `talking-head-30s` and duration `00:29:20`. Returning Home restored `TalkingHeadQA` alongside `Task9Empty-20260731` in recents. No pre-existing project bundle was modified or deleted.

Result: PASS. All three planned New Project controls have native cancel/retry, success, persisted-bundle, reopen, busy exclusion, and explicit failure/recovery evidence, while initial-save failure can no longer replace the active project.
