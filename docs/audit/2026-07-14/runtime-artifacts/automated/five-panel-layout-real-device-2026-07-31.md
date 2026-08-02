# Five-panel layout packaged macOS evidence — 2026-07-31

## Scope

- Plans: `home-shell-implementation.md` Task 5 `HS-layout-geometry + CC-layout-misgrouped`; `accessibility-polish-implementation.md` Task 9 `five-panel-layout-focus`
- App: `target/debug/bundle/macos/OpenTake.app`
- Project: `/private/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake`
- Host: packaged macOS app, exercised through the native accessibility tree and captured screenshots
- Locale/theme/fixture: Chinese, dark theme, the same TalkingHeadQA project throughout
- Executable SHA-256: `3c7b66247716f17ba38ca4ae1d46320c6f3211273112c3be22e21bdcd267862f`
- Debug DMG SHA-256: `d15d7dc7f77f16fac3b27da37abd8276c07c73813287cf7a1977aded30a4d435`

## Deterministic code contract

The two exact owning names execute the same full matrix so the Home Shell and
Accessibility ledgers cannot drift:

- `all_presets_match_geometry_visibility_maximize_and_focus_shell`
- `all_presets_ratios_gutters_surfaces_focus`

At 1600×1000 the matrix proves Default 70%, Media 30%/55%, Vertical 50%/55%,
500px Media, 260px Inspector, 320px Agent, leaf order, visibility, focus-ring
transition, click side effects, maximize, and separator keyboard resizing. At
960×600 it proves the compact boundary keeps Preview and Timeline available
with both collapsible side panels hidden. PanelShell uses the shared 5px gap,
6px radius, surface/base tokens, and 1.5px accent focus ring at opacity 0.6.

## Gates

- True focused RED: the Home Shell owning name failed while all prior 747 Web
  tests passed because the layout had no declared preset boundary, semantic
  panel/splitter contract, or complete collapse behavior.
- Both exact focused GREEN names passed independently (1 executed / 1 skipped
  in the shared two-name matrix).
- Full Web regression: 75 files / 749 tests passed.
- TypeScript/Vite production build passed. Its existing bundle-size and
  ineffective-dynamic-import warnings were unchanged and non-blocking.
- `./web/node_modules/.bin/tauri build --debug` passed and produced the exact app
  and DMG identified above.
- `git diff --check` passed.

## Packaged sequence

1. Quit the prior process and launched the exact rebuilt app bundle, then opened
   TalkingHeadQA from Home.
2. Default exposed Media / Preview / Inspector above Timeline. The accessibility
   splitters reported 500px Media, 260px right anchoring through the measured
   center split, and a 70% top split; the screenshot showed uninterrupted 5px
   base grooves and rounded surface cards.
3. Clicking Media moved the 0.6-opacity accent ring from Timeline to Media.
4. `⌘2` applied Media layout: Media occupied 30% at left; Preview / Inspector
   occupied the 55% upper-right region; Timeline filled the lower-right region.
5. `⌘3` applied Vertical layout: the left subtree occupied 50%, its upper
   Media / Inspector region occupied 55%, Timeline remained below, and Preview
   filled the complete right side.
6. `⌘⌥0` collapsed Inspector. Media expanded across the complete upper-left
   region with no empty sibling split. `⌘0` then collapsed Media; only the
   non-collapsible Timeline and Preview regions remained.
7. Restored Media and Inspector and enabled Agent with `⌘⌥A`. The accessibility
   tree exposed all five localized regions plus four localized splitters,
   including the 320px Agent column.
8. Clicked Preview and pressed backquote. Only Preview remained and filled the
   editor body. Escape restored all five panels.
9. Restored Default layout, Agent hidden, Media and Inspector visible, and
   non-maximized state for the final packaged app state.

The host display constrains the native window below an unscaled 1600×1000
capture, so exact 1600×1000 and 960×600 geometry is owned by the deterministic
component fixtures; the packaged screenshots cover the same three presets and
state transitions at the largest host-available window. No project, timeline,
media, or external file was mutated by this verification.
