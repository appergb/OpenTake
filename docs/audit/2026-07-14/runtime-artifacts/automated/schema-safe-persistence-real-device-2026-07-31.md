# HS-schema-safe-persistence packaged macOS evidence — 2026-07-31

## Scope

- Plan: `home-shell-implementation.md`, Task 6 `HS-schema-safe-persistence`
- App: `target/debug/bundle/macos/OpenTake.app`
- Project: `/private/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake`
- Host: packaged macOS app, exercised through the native accessibility tree

## Code contract

The store persists only five global UI preferences, each in its own versioned
key:

- `opentake.ui.v1.layoutPreset`
- `opentake.ui.v1.agentPanelVisible`
- `opentake.ui.v1.mediaPanelVisible`
- `opentake.ui.v1.inspectorPanelVisible`
- `opentake.ui.v1.keyframesPanelVisible`

The exact owning test covers defaults; every valid and invalid current value;
valid and invalid legacy unprefixed values; one-time valid migration; isolated
per-action writes; fresh-store rehydration; unavailable/throwing reads; rejected
writes; and exclusion of view, playhead, selection, and maximize session state.

## Gates

- True focused RED: the planned test failed because `createEditorUiStore` did
  not exist and the old singleton could not prove a fresh-session boundary.
- True focused GREEN: 1/1.
- Plan-form focused command executed after implementation.
- Web regression: 74 files / 747 tests passed.
- Web production build passed.
- `./web/node_modules/.bin/tauri build --debug` passed and produced both the app
  and debug DMG.
- The preceding Task 7 full Rust workspace and formatting gates passed; Task 6
  changes only TypeScript store persistence, its tests, and documentation.

## Packaged restart sequence

1. Opened `TalkingHeadQA` from Home. The existing unprefixed preferences were
   accepted and migrated: Agent reopened visible while Media and Inspector used
   their stored/default visibility.
2. Selected a text clip and opened the Keyframes panel.
3. Applied Vertical layout, kept Agent visible, and hid Media and Inspector.
   The packaged screen showed the expected distinct vertical geometry.
4. Terminated that exact debug app process and relaunched the same bundle.
5. The app correctly started at Home, proving `view=editor` was not persisted.
6. Reopened `TalkingHeadQA`: Vertical layout and Agent-visible/Media-hidden/
   Inspector-hidden preferences were restored. Playhead was `00:00:00` and every
   timeline clip was unselected, proving project/session state did not leak.
7. Re-enabled Inspector and selected the text clip. The Keyframes lanes appeared
   immediately without pressing the Keyframes button, proving that preference
   also survived the process restart.
8. Restored Default layout, Agent off, Media and Inspector visible, Keyframes
   closed, and cleared selection for the final app state.

No project file, timeline command, media entry, or external output was written
by this verification.
