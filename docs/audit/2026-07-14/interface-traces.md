# OpenTake interface control trace

Audit date: 2026-07-14

This trace is the human-readable index for the machine-verifiable control review in
`controls.json`. It covers every tracked `web/src/**/*.tsx` candidate frozen in
`control-candidates.json`; it is not a claim that every candidate is a distinct
product action or that a passing generic smoke test proves an individual control.

## Fail-closed result

| Scope | Candidates | Complete | Incomplete | Obsolete non-actions | Duplicate wiring | Contradicted | Unverified |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| All tracked TSX controls | 259 | 0 | 212 | 34 | 13 | 0 | 0 |

No actionable control is marked complete. A `complete` disposition requires
candidate-specific direct evidence for the declared handler, state transition,
outcome paths, accessibility contract, and return path. Static source tracing,
generic suite success, or application launch evidence is supporting evidence only.

## Trace contract

Each `controls.json` record binds the stable candidate ID to:

1. the exact source path, line, column, element, label, handler, disabled expression,
   and ARIA role from the frozen candidate ledger;
2. visibility and enabled conditions, user input, handler and state transition;
3. the store/API/Tauri/Rust boundary trace, including explicit `N/A` where there is
   no backend boundary;
4. success, pending, empty, disabled, cancel, retry, and failure outcomes;
5. focus, accessible name, shortcut, and return-path obligations;
6. exact automated-test or typed runtime-receipt references;
7. one legal final disposition and, for incomplete actions, executable acceptance
   criteria plus one legal gap group.

`tools/completion-audit.mjs verify --scope controls` re-extracts controls from the
tracked current worktree source and rejects source drift, missing/extra/reordered IDs,
invalid stable IDs, illegal dispositions, unsupported complete claims, duplicate
chains, count drift, gap-count drift, or untyped runtime evidence. A deterministic
test can support `complete` only when its tracked exact name binds the candidate ID
and its test body contains a real assertion. A runtime receipt must use the exact
typed envelope, strict timezone timestamps, known candidate IDs, hashed tracked or
Git-ignored artifacts, a consistent summary, and verified browser/native cleanup.

## Surface matrix

| Surface | Candidates | Complete | Incomplete | Obsolete | Duplicate | Open gap groups |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Agent | 7 | 0 | 6 | 0 | 1 | agent-settings-generation |
| Home | 15 | 0 | 11 | 3 | 1 | home-shell |
| Inspector | 77 | 0 | 66 | 10 | 1 | accessibility-polish; inspector-text-keyframes |
| Media | 60 | 0 | 45 | 8 | 7 | agent-settings-generation; media-library |
| Preview | 15 | 0 | 14 | 0 | 1 | accessibility-polish; preview-timeline |
| Settings | 21 | 0 | 19 | 1 | 1 | agent-settings-generation |
| Shell | 25 | 0 | 23 | 2 | 0 | accessibility-polish; home-shell; media-render-playback-export |
| Timeline | 24 | 0 | 17 | 6 | 1 | accessibility-polish; preview-timeline |
| Toolbar | 10 | 0 | 9 | 1 | 0 | command-contracts |
| UI primitives and test fixture | 5 | 0 | 2 | 3 | 0 | accessibility-polish |

## Component index

The record-level source-to-backend and outcome traces remain in `controls.json`;
this table makes coverage and dispositions inspectable without collapsing helper
definitions into product actions.

| Component | Candidates | Incomplete | Obsolete | Duplicate |
| --- | ---: | ---: | ---: | ---: |
| `web/src/components/agent/AgentPanel.tsx` | 7 | 6 | 0 | 1 |
| `web/src/components/home/HomeView.tsx` | 15 | 11 | 3 | 1 |
| `web/src/components/inspector/Inspector.tsx` | 49 | 45 | 3 | 1 |
| `web/src/components/inspector/KeyframesLaneRow.tsx` | 11 | 9 | 2 | 0 |
| `web/src/components/inspector/KeyframesPanel.tsx` | 1 | 0 | 1 | 0 |
| `web/src/components/inspector/ScrubbableNumberField.tsx` | 2 | 2 | 0 | 0 |
| `web/src/components/inspector/SwapMediaSection.tsx` | 2 | 2 | 0 | 0 |
| `web/src/components/inspector/TextTab.tsx` | 12 | 8 | 4 | 0 |
| `web/src/components/media/CaptionsTab.tsx` | 14 | 12 | 2 | 0 |
| `web/src/components/media/LibraryView.tsx` | 13 | 9 | 4 | 0 |
| `web/src/components/media/MediaPanel.tsx` | 23 | 15 | 2 | 6 |
| `web/src/components/media/MediaSearch.tsx` | 6 | 6 | 0 | 0 |
| `web/src/components/media/MediaTabBar.tsx` | 2 | 2 | 0 | 0 |
| `web/src/components/media/SoundLibraryTab.tsx` | 2 | 1 | 0 | 1 |
| `web/src/components/preview/CropOverlay.tsx` | 2 | 2 | 0 | 0 |
| `web/src/components/preview/Preview.tsx` | 11 | 10 | 0 | 1 |
| `web/src/components/preview/TransformOverlay.tsx` | 2 | 2 | 0 | 0 |
| `web/src/components/settings/AccountPane.tsx` | 6 | 6 | 0 | 0 |
| `web/src/components/settings/SettingsView.tsx` | 15 | 13 | 1 | 1 |
| `web/src/components/shell/ExportDialog.tsx` | 8 | 7 | 1 | 0 |
| `web/src/components/shell/SaveAsProgress.tsx` | 1 | 1 | 0 | 0 |
| `web/src/components/shell/SplitPane.tsx` | 1 | 1 | 0 | 0 |
| `web/src/components/shell/TitleBar.tsx` | 9 | 9 | 0 | 0 |
| `web/src/components/shell/ViewMenu.tsx` | 6 | 5 | 1 | 0 |
| `web/src/components/timeline/ClipContextMenu.tsx` | 1 | 1 | 0 | 0 |
| `web/src/components/timeline/SwapMediaPicker.tsx` | 4 | 3 | 1 | 0 |
| `web/src/components/timeline/TimelineContainer.tsx` | 7 | 4 | 3 | 0 |
| `web/src/components/timeline/TimelineRangeContextMenu.tsx` | 1 | 1 | 0 | 0 |
| `web/src/components/timeline/TimelineRegion.tsx` | 1 | 1 | 0 | 0 |
| `web/src/components/timeline/TrackHeaderColumn.tsx` | 10 | 7 | 2 | 1 |
| `web/src/components/toolbar/Toolbar.tsx` | 10 | 9 | 1 | 0 |
| `web/src/components/ui/Dropdown.tsx` | 2 | 1 | 1 | 0 |
| `web/src/components/ui/HoverButton.test.tsx` | 1 | 0 | 1 | 0 |
| `web/src/components/ui/HoverButton.tsx` | 1 | 0 | 1 | 0 |
| `web/src/components/ui/PanelShell.tsx` | 1 | 1 | 0 | 0 |

## Open gap inventory

Only incomplete controls contribute to these totals.

| Gap group | Controls | Required closure evidence |
| --- | ---: | --- |
| accessibility-polish | 18 | Direct keyboard/focus/name/state checks for the candidate's exact interaction and return path |
| agent-settings-generation | 37 | Exact agent/settings/generation handler, outcome, disabled/failure, and persistence evidence |
| command-contracts | 9 | Exact command payload/count/error contract, including authoritative edit behavior |
| home-shell | 25 | Direct launcher/menu/dialog/window-flow evidence including cancel and return focus |
| inspector-text-keyframes | 57 | Exact field/keyframe/style mutation plus rejection/recovery and keyboard evidence |
| media-library | 33 | Exact import/search/library/card action and empty/failure/retry behavior |
| media-render-playback-export | 8 | Exact render/playback/export lifecycle and failure/cancel/retry evidence |
| preview-timeline | 25 | Exact gesture/seek/timeline mutation plus focus and authoritative-command evidence |

## Material traced risks

- Crop and transform source/tests do not prove that pointer release emits exactly
  one authoritative edit command.
- Dropdown, View/TitleBar menus, and Export/Settings dialogs do not yet have complete
  keyboard focus movement and return-focus evidence.
- Keyframe lanes, diamonds, and menu surfaces do not yet have complete keyboard
  navigation and exact-command interaction evidence.
- MediaPanel AI generation remains a disabled visible placeholder; its generation
  boundary is not a completed product path.
- MediaPanel view-mode, sort, and filter HoverButtons have no candidate-owned
  `onClick` handler.
- Inspector and timeline fire-and-forget branches generally do not expose backend
  rejection and recovery to the user.
- Preview scrubbing, SplitPane, ScrubbableNumberField display mode, and several
  media/recent/search cards retain keyboard reachability or operability gaps.
- Semantic-search index/query failures and track mute/hide/sync-lock keyboard/state
  semantics remain incomplete.

## Runtime evidence interpretation

`runtime-evidence.json` is the typed receipt ledger. A receipt records the command
or interaction, timestamps, exit status, candidate assertions, artifacts,
limitations, and exact named tests. Browser/native launch or generic suite receipts
are supporting evidence. A direct automated receipt needs per-candidate assertions
plus a tracked exact named test; a direct browser/native receipt needs per-candidate
assertions plus a hashed tracked artifact. Supporting evidence can confirm that a
surface is reachable without promoting any individual control to `complete`.

The authoritative machine result is generated in `control-verification.json`.
