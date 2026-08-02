# Recent project card acceptance — 2026-07-30

Scope: Home Shell Task 12 (`implementation-slice-8e481b772d7d2357`) on the current arm64 debug bundle at `target/debug/bundle/macos/OpenTake.app`.

## Code verification

- Baseline RED: `pnpm -C web exec vitest run src/components/home/HomeView.interaction.test.tsx -t 'control-9697b53d4d2cf1ca select or open a recent project card'` exited 1 because the planned owning runner did not exist.
- Added the exact candidate-named interaction test for single-click selection, focus selection, Enter opening, and double-click opening through `openProjectPath(entry.path)`.
- Focused result: 1 file / 1 test passed. Nearby Home visual result: 11/11 passed.
- Regression result: 72 Web files / 727 tests passed; the production Web build and complete Rust workspace passed.

## Product correction

The original recent-card root was a pointer-only `div`. It was replaced with a real named button carrying `aria-pressed`, and its remove control remains a separate sibling button. Focus and single click select the path; the existing launcher Return handler and double-click both call the same project-open boundary. Return events from other focused interactive controls are ignored, preventing a stale card selection from opening alongside another Home action.

## Real-device verification

1. The initial pointer-only build exposed `TalkingHeadQA` and `Untitled` only as text in the macOS accessibility tree, confirming the product-level RED.
2. Rebuilt the exact app and terminated two pre-existing debug instances before launching a single current bundle, preventing stale-process evidence from being mixed with the candidate.
3. The fresh accessibility tree exposed `toggle button TalkingHeadQA, Value: off` and `toggle button Untitled, Value: off`.
4. A single click selected `TalkingHeadQA` (`Value: on`) while the app remained on Home.
5. Tab focus reached `TalkingHeadQA`; pressing Return opened the editor with the expected `talking-head-30s` media and 29:20 timeline.
6. Returned Home and invoked a native two-click activation (`click_count: 2`) on `TalkingHeadQA`; it opened the same editor and project content.

Result: PASS. The planned pointer, keyboard, accessibility, and project-open paths agree in the packaged application.
