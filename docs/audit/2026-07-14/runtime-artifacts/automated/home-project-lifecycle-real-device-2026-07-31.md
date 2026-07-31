# Home project lifecycle real-device verification (2026-07-31)

## Scope

- Contract: `HS-project-card-lifecycle`
- Owning requirement: `requirement-c4af18634c223a8d`
- Exact packaged artifact: `target/debug/bundle/macos/OpenTake.app`
- DMG artifact: `target/debug/bundle/dmg/OpenTake_1.0.0_aarch64.dmg`

## RED evidence

- `cargo test -p opentake-tauri missing_entry_survives_registry_load_and_safe_trash_removes_only_after_success` failed to compile because `ProjectRegistry` did not exist.
- `pnpm -C web exec vitest run src/components/home/HomeView.test.tsx -t 'missing_card_reveal_remove_and_trash_states'` failed because Home rendered only the stale path and did not render `home.fileMissing` or any safe file actions.

## GREEN evidence

- Exact Rust owning test: PASS (`1 passed`).
- Exact Home owning test: PASS (`1 passed`).
- Home component regression pair: PASS (`2` files, `16` tests).
- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --offline -p opentake-tauri --all-targets -- -D warnings`: PASS.
- `cargo test --offline --workspace --no-fail-fast`: PASS; hardware-only probes remain explicitly ignored by their owning runners.
- `pnpm -C web test -- --run`: PASS (`76` files, `753` tests).
- `pnpm -C web build`: PASS with the existing Vite mixed-import and bundle-size advisories.
- `git diff --check`: PASS.

## Final artifact identity

- App executable SHA-256: `5e345b5a143fee720eca5049bf673c09a57ab08428666f1da0b660f3a1b6f5d1`.
- DMG SHA-256: `a775931f5f622ddbb3507b31ca7d000c2ad373b7af21bf7766e007bae3f92b71`.
- Both bundles were produced successfully after the final native trash implementation.

## Packaged macOS walkthrough

All destructive checks used temporary copies of the built-in tutorial. The three pre-existing user recent projects were never targeted and remained present at the end.

1. Opened `/tmp/OpenTake-HomeLifecycle-QA-20260731-1157.opentake` through the native Open panel. Home changed from `3 recent` to `4 recent`, proving native registration and legacy/local mirror synchronization.
2. Expanded `项目操作`; the accessible dialog exposed `在 Finder 中显示`, `从最近项目中移除`, and `移到废纸篓`.
3. `在 Finder 中显示` opened Finder at `/private/tmp` with the exact QA bundle selected.
4. The first packaged trash attempt exposed a real platform failure: Finder Apple Events waited indefinitely without Automation authorization. The UI stayed in pending state; after terminating only the stuck QA helper process, it returned an explicit retryable error. The bundle remained at its source path and the Home record remained at `4 recent`.
5. The implementation was changed to Foundation's native `NSFileManager.trashItem`, rebuilt, and relaunched from the exact `.app` path.
6. Activating `移到废纸篓` showed a named confirmation dialog. Confirming immediately returned Home to `3 recent`; the source path was absent and the complete bundle existed at `~/.Trash/OpenTake-HomeLifecycle-QA-20260731-1157.opentake`.
7. Opened a second QA bundle, relocated it to a recovery path while the editor was active, then returned Home. The registry preserved the card and exposed the accessible label `OpenTake-Missing-QA-20260731-1201 · 文件缺失`.
8. Double activation of the missing card stayed on Home. Reveal opened its existing `/private/tmp` parent rather than raising a silent error.
9. `从最近项目中移除` changed `4 recent` back to `3 recent` without touching the recovery bundle.
10. After making ordinary recent removal native-first, rebuilt both bundles and launched the exact final `.app` as the only running OpenTake instance. Opened `/tmp/OpenTake-AtomicRemove-QA-20260731-1215.opentake`, observed `3 recent` → `4 recent`, then used `从最近项目中移除` and observed `4 recent` → `3 recent`. The native registry no longer contained that path while the complete bundle remained at its source, proving removal committed before the UI mirror changed and did not delete the project. The QA copy was then moved to `~/.Trash/OpenTake-AtomicRemove-QA-20260731-1215.opentake` for recoverable cleanup.

## Safety and recovery assertions

- The native trash command rejects relative paths, non-`.opentake` paths, and paths not present in the native registry before invoking any operating-system capability.
- The injected permission-denied Rust path leaves both the project directory and registry entry unchanged.
- A missing registered bundle is treated as a removable stale record; no filesystem delete is attempted.
- Registry snapshots are staged, synced, and atomically renamed; mutation becomes visible in memory only after persistence succeeds.
- On desktop, ordinary recent removal awaits the native atomic registry commit before updating the local Home mirror; a persistence failure remains visible and retryable instead of hiding the card.
- All three temporary QA bundles were placed in the user's system Trash after verification and remain recoverable. Existing user project data was not modified.
