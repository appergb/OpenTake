# HS-menu-contract packaged macOS evidence — 2026-07-31

## Scope

- Plan: `home-shell-implementation.md`, Task 7 `HS-menu-contract`
- App under test: `target/debug/bundle/macos/OpenTake.app`
- Original project: `/private/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake`
- Save-As fixture: `/private/tmp/Task7NativeSaveAs-20260731.opentake`
- Host: macOS desktop package, exercised through the native accessibility tree

This task covers both menu surfaces required by the source specification: the
complete native application menu (App/File/Edit/View/Help) and the cross-platform
in-app View menu. The two surfaces dispatch the same project, edit, media, and UI
actions; native enabled/checked/text state is synchronized from the owning stores.

## Code and package gates

- True focused RED before implementation:
  `pnpm -C web test --run src/components/shell/ViewMenu.test.tsx -t "commands_shortcuts_checked_state_and_disabled_rules"`
  failed because the planned owning test file did not exist.
- Focused GREEN after implementation: 1/1.
- Save-As action regression: 20/20, including atomic success, failed-publication
  rollback, and compatibility-read-only rejection.
- Web regression: 73 files / 746 tests passed.
- Web production build passed.
- `cargo check -p opentake-tauri` passed.
- `./web/node_modules/.bin/tauri build --debug` passed and produced the app and
  debug DMG under `target/debug/bundle/`.
- `cargo fmt --all -- --check` passed.
- `cargo test --workspace` passed on the final run. The preceding run had one
  three-second readiness timeout in the mocked generation-provider smoke test;
  that exact test passed in isolation and the immediate full rerun also passed,
  identifying a timing flake rather than a persistent product failure.
- `git diff --check` passed.
- Tauri capability validation includes only the additional
  `core:window:allow-set-fullscreen` permission required by the command.

The plan-prescribed focused command contains an extra `--`; Vitest interprets
the trailing arguments as passthrough and runs the whole suite. It is retained
for plan parity, while the true focused form above proves that the named test
itself executed.

## Native application menu matrix

The packaged app exposed exactly five top-level groups and 32 specified entries:
App 4, File 6, Edit 10, View 8, Help 4. Update, Tutorial, and Feedback are visible,
explicitly labelled unavailable in Beta, and disabled rather than silently wired
to nonexistent behavior.

| Context / group | Packaged result |
| --- | --- |
| Home / File | New and Open enabled; Save, Save As, Import Media, and Export disabled. `⌘N` opened the native Save dialog and `⌘O` opened the native Open dialog; both were cancelled without side effects. |
| Editor / File | All six entries enabled. Save As published the independent fixture atomically; it contains `project.json`, `media.json`, `thumbnail.jpg`, and the chat session. Its external media reference remained valid. Save then rewrote that fixture successfully. Import Media opened the native media picker and cancel restored the editor. Export opened the application export panel and cancel restored the editor. |
| Home / Edit | All ten entries disabled. |
| Editor / Edit, no selection | Undo/Redo, Cut/Copy/Paste, trims, and Delete disabled; Select All and Split enabled because the project contains clips. |
| Editor / Edit, selection | Select All selected all ten timeline items; Cut, Copy, both trims, and Delete became enabled. Copy was non-mutating and enabled Paste. Escape cleared the selection and returned those commands to the disabled state. |
| App | About, Settings, and Quit present; Check for Updates visible and disabled for Beta. Settings opened the General pane. Quit was not activated during verification. |
| Help | Keyboard Shortcuts and MCP Instructions enabled; Tutorial and Send Feedback visible and disabled for Beta. Both enabled commands opened their exact Settings panes, including an accessible shortcut table. |
| Runtime language | Switching Settings from Chinese to English immediately changed native group and item labels (`文件/编辑/视图/帮助` → `File/Edit/View/Help`) without restart; switching back restored Chinese. |

The original `TalkingHeadQA.opentake` was never written by Save As. The app was
restarted after fixture verification and the Home recent list showed both the
original project and the new fixture, proving the canonical path changed only
after successful publication.

## View command matrix

The in-app menu opened with keyboard focus on Media Panel. Every item exposed
the specified accelerator and accessibility checked state. The same View entries
were present and enabled in the native View menu, including the three-item Layout
submenu.

| Command | Pointer/native result | Shortcut result | Checked/result evidence |
| --- | --- | --- | --- |
| Media Panel | panel removed/restored through both in-app and native menus | `⌘0` removed it | state changed on → off → on |
| Inspector | panel removed/restored | `⌘⌥0` removed it | state changed on → off |
| Agent Panel | panel removed/restored | `⌘⌥A` removed it | title toggle and menu changed on → off |
| Maximize Focused Panel | timeline expanded to the editor body | backquote expanded/restored it | state changed off → on |
| Default Layout | default geometry restored | `⌘1` restored it | sole selected layout item |
| Media Layout | media geometry applied | `⌘2` applied it | selected state moved to Media |
| Vertical Layout | vertical geometry applied | `⌘3` applied it | selected state moved to Vertical |
| Enter Full Screen | macOS window entered/exited fullscreen | `⌘F` entered/exited it | traffic-light controls disappeared/reappeared |

Down moved focus from the first enabled command to Inspector; Home/End and arrow
traversal are covered by the focused contract. Escape closed the menu, returned
focus to its trigger, and did not leak to the editor-wide maximize handler.

The focused contract also proves the defensive rule that Maximize Focused Panel
is disabled when `focusedPanel === null`. Hiding a currently focused/maximized
collapsible panel moves focus to timeline and clears maximize.

## Defects found and corrected during packaged validation

1. The first implementation covered only the in-app View popover, while the
   requirement literally called for the complete application/main menu. The
   native 32-entry menu, shared command router, store synchronization, Settings
   deep links, and Save-As action were added before the task was classified.
2. Escape originally reached both the menu dismissal handler and the global
   editor handler, cancelling maximize. Capture-phase consumption now makes one
   Escape perform one action.
3. Fullscreen initially failed because Tauri's window capability allowed reads
   but not writes. The exact set permission was added and failure now produces a
   localized recovery toast.
4. Native labels initially reflected only startup locale. Menu/submenu handles
   now subscribe to the i18n store and update text immediately; packaged Chinese
   → English → Chinese verification passed.
5. Self-review found that the first synchronization pass would resend every
   native label on unrelated high-frequency UI changes such as playhead motion.
   State and locale synchronization are now separate and snapshot-deduplicated;
   the rebuilt package retained immediate language switching.

## Final state

- Application returned to Home with Chinese restored.
- Original and Save-As fixture both remained available in Recents.
- No destructive edit command was executed against the original project.
- Default layout, visible panels, non-maximized state, and non-fullscreen state
  were restored before the final restart.
