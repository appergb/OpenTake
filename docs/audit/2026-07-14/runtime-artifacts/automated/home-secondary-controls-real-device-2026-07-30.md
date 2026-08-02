# Home secondary controls acceptance — 2026-07-30

Scope: Home Shell Task 11 (`implementation-slice-94554777e3aef548`) on the single current arm64 debug bundle at `target/debug/bundle/macos/OpenTake.app`.

## Code verification

- Baseline: the Home interaction runner existed from Task 12, but the four exact Task 11 names were absent; the focused filter executed zero candidates and skipped the file.
- Added exact tests for Home → Library, Home → Settings, background selection clearing, and recent-entry removal without project opening.
- Focused result: 5/5 Home interaction tests passed, including the four Task 11 candidates.
- Regression result: 72 Web files / 731 tests passed; production Web build passed with only the pre-existing dynamic-import and chunk-size warnings.

## Real-device verification

1. From Home, activated `素材库`; the app showed the global Library with `返回主页`, category controls, search, and empty-library state.
2. Returned Home and activated `设置`; the modal exposed `完成`, `通用`, `外观`, `导入`, `AI`, `MCP 说明`, `账户`, and `关于`, then closed back to Home.
3. Single-clicked `TalkingHeadQA`; accessibility changed it to `Value: on`. Clicked the empty Home workspace; it returned to `Value: off` without navigation.
4. Hovered `TalkingHeadQA` and activated `从最近中移除`. The Home count changed from 2 to 1, `TalkingHeadQA` disappeared, no editor opened, and `/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake` still existed on disk.
5. Used Home → Open Project, navigated the native directory picker to the exact preserved bundle, and opened it. The editor showed `talking-head-30s` and `00:29:20`; returning Home showed 2 recent entries again.

Result: PASS. Navigation, selection clearing, and recent-only removal behave correctly, and the recoverable recent entry was restored after validation.
