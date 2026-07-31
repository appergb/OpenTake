# Data safety shared core command real-device evidence — 2026-07-31

## Scope

- Plan: `data-safety-implementation.md`, Task 7 `DS-shared-core-command-complete`.
- Requirements: the 10 records mapped to the shared `EditorState` / `EditCommand` / `AppCore` / `EventBus` command path.
- Boundary: versioned command application, unchanged-command handling, undo/redo, and convergence between the packaged desktop client and live MCP client.

## Exact code evidence

- `commit_undo_redo_cycle_restores_and_versions`: PASS, 1/1.
- `apply_bumps_version_and_emits_once`: PASS, 1/1.
- `unchanged_command_does_not_emit_or_bump`: PASS, 1/1.
- `undo_redo_through_core_bumps_version_and_emits`: PASS, 1/1.

These owning tests prove that committed edits bump the shared revision and emit once, unchanged commands neither bump nor emit, and undo/redo restore state while producing versioned results. They were already green at the audited baseline; no artificial RED failure was introduced.

## Packaged desktop command result

The debug packaged application opened `/private/tmp/opentake-ds-generation-seed-real-device.opentake`, initially containing the persisted text clip `Generation seed verified`. Using the production desktop controls:

1. **Add Text** created a second clip with ID `29f96e16-4133-440c-acf5-2733b28f23a8`.
2. **Undo** removed that clip and enabled Redo.
3. **Redo** restored the same clip ID and disabled Redo.

The resulting timeline visibly contained exactly the two expected text clips.

## Live MCP command result

Against the same running packaged process and project, the production MCP transport returned:

- initial `get_timeline`: 90 total frames and two clips (`29f96e16`, `fd55ad76`);
- `add_texts`: added `Agent shared path` as clip `99b61c61` at frame 90 for 30 frames;
- next `get_timeline`: 120 total frames and three clips;
- `undo`: returned `Undid last edit`;
- final `get_timeline`: restored 90 total frames and exactly the original two clip IDs.

The already-open desktop UI updated live after the MCP undo: it showed the same two clips and enabled Redo. This demonstrates that desktop and Agent/MCP edits converge through the shared production command/history path rather than maintaining isolated client state.

## Regression gate

- `cargo fmt --all -- --check`: PASS.
- all four focused owning tests: PASS.
- `cargo test --workspace --no-fail-fast --quiet`: PASS; all executed workspace tests passed, with only the repository's explicitly ignored tests skipped.
- `git diff --check`: PASS.

## Outcome

Task 7 is complete. The exact Rust version/event contracts and two real packaged clients agree on edit, undo, redo, identity restoration, and visible state. This closes the 10 mapped records only; it does not reclassify later data-safety tasks or authorize Beta publication.
