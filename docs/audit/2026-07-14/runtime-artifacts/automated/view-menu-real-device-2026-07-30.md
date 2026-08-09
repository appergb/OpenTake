# View menu control acceptance — 2026-07-30

Scope: Home Shell Task 16 (`implementation-slice-4a4cd45e11211989`) on the rebuilt arm64 macOS application at `target/debug/bundle/macos/OpenTake.app`, with `/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake` open.

## Code verification

- The specified owning runner did not exist. `pnpm -C web exec vitest run src/components/shell/ViewMenu.interaction.test.tsx` therefore produced the expected RED result: `No test files found, exiting with code 1`.
- The plan's generated `pnpm -C web test -- --run ...` spelling was not used as focused evidence because Vitest treated the extra `--` as an argument boundary and ran the full suite. The direct Vitest spelling above is the verified focused command.
- Added exactly five candidate-named interaction tests covering menu toggle/Escape/outside dismissal, preset selection and close, and Agent/Media/Inspector panel toggles with persisted state and checked-state assertions.
- Focused result: 1 file, 5 tests passed.
- Regression result: 71 files, 726 tests passed.
- `pnpm -C web build` passed. Vite emitted only the existing dynamic-import and chunk-size warnings.

## Real-device verification

Computer Use exercised the packaged Tauri application through its accessibility surface:

1. Opened `视图`; accessibility exposed a menu with `默认布局`, `媒体布局`, `竖屏布局`, `Agent 面板`, `媒体面板`, and `检查器`.
2. Selected `媒体布局`; the menu closed, the layout changed, and reopening reported `媒体布局 Value: 1` with the other presets at `0`.
3. Toggled Agent, Media, and Inspector while the menu stayed open. Each accessibility value changed from `1` to `0`, and the corresponding panel disappeared from the editor.
4. Pressed Escape; the menu closed and the editor accessibility tree returned.
5. Reopened the menu and clicked the preview outside it; the menu closed.
6. Restored `默认布局` and all three panels to `Value: 1`, then dismissed the menu. The final packaged-app screenshot and accessibility tree showed the original Agent + Media + Preview + Inspector arrangement.

Result: PASS. All five planned controls behaved correctly in component tests and in the packaged macOS app.
