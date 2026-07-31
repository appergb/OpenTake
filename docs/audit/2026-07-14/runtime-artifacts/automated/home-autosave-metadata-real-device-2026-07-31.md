# Home autosave and metadata real-device verification (2026-07-31)

## Scope

- Contract: `HS-autosave-metadata-mixed`
- Owning requirement: `requirement-034f4a07aa8a3c47`
- Exact final application: `target/debug/bundle/macos/OpenTake.app`
- Exact final DMG: `target/debug/bundle/dmg/OpenTake_1.0.0_aarch64.dmg`

## RED and GREEN evidence

- The planned test `autosave_and_home_metadata_have_separate_owners` first failed because a successful save left `modifiedAt` at `750`, left `thumbnailPath` null, and therefore exposed no persisted Home metadata update.
- The native metadata test first failed to compile because `HomeProjectEntry` had no `modified_at` or `thumbnail_path` fields.
- The Home interaction test first failed because the project card rendered no image and no relative-time label.
- Planned exact test: PASS (`1 passed`).
- Native metadata exact test: PASS (`1 passed`).
- Focused project/recent/Home regression set: PASS (`4` files, `27` tests).
- Web full suite: PASS (`77` files, `755` tests).
- Web production build: PASS with only the existing Vite mixed-import and bundle-size advisories.
- Rust Clippy: PASS with `-D warnings`.
- `cargo fmt --all -- --check && cargo test --offline --workspace --no-fail-fast`: PASS; the Tauri library ran `376` unit tests and hardware-only probes retained their explicit owning-runner ignores.
- `git diff --check`: PASS.

## Artifact identity

- Final app executable SHA-256: `adf7b04897ef0704b9cc43c4ef565b5ccdc06ce31cfafb67528c97374eecb219`.
- Final DMG SHA-256: `f2098fd08970230569f085302f862e51f5642156ce01fd7b1aa2d38562af4420`.
- Both bundles were rebuilt after the final visual contract changed from path-plus-time to relative-time-only metadata.

## Packaged edit → save → close → reopen walkthrough

All mutations used `/tmp/OpenTake-Autosave-QA-20260731-1231.opentake`, a temporary copy of the built-in tutorial. A valid 320×180 JPEG cover was copied into that QA bundle. No pre-existing user project was modified.

1. Opened the QA bundle through the native directory picker. Its baseline `project.json` SHA-256 was `e4352964a8bcd331f5f25db19a26c63121890d92f68d8a2cee512f7ad677e421` and the timeline contained three clips.
2. Activated `添加文本`. After the 1.5-second debounce, the persisted project hash changed to `8ddaaecd6d2ffc3f117db9e3d3b67492bdd25ce69065285da5c5cdf4a847ca01`, proving silent autosave completed.
3. Activated `添加文本` again and immediately requested window close, before the debounce could fire. The native CloseRequested flush changed the persisted hash to `5571da3a4878af4d19a6ab37fc3947cdb9d40ea9026bc5a2dbef068c21d0a2d7`; the saved project contained three tracks and five total clips.
4. After the close-to-Home event settled, reopened the resident app. Home contained four recent projects and the QA card showed its 16:9 cover plus `今天`; the absolute path was no longer used as the visible subtitle.
5. Double-opened the QA card. The editor exposed both newly persisted clip IDs plus the original three tutorial clips, proving the autosave and close-flush writes both survived reopen.
6. Returned Home, removed only the QA recent entry (`4 recent` → `3 recent`), then moved the temporary QA bundle to `~/.Trash/OpenTake-Autosave-QA-20260731-1231.opentake` for recoverable cleanup.
7. Rebuilt the final app and DMG after the relative-time-only visual correction. Launched the exact final `.app` as the sole OpenTake instance and confirmed all three retained user cards display covers/placeholders and localized relative time without visible filesystem paths.

## Ownership and recovery assertions

- `openedAt` owns recent ordering and changes only when a project opens; a successful save updates `modifiedAt` and `thumbnailPath` without moving the card.
- The native registry refreshes metadata from the persisted `project.json` mtime and optional bundle-root `thumbnail.jpg`; missing bundles retain their last metadata and do not attempt cover loading.
- Save failure does not update Home metadata, while the existing save coordinator keeps the project dirty and exposes its error toast.
- The temporary QA bundle remains recoverable in system Trash. The three original recent projects remained registered at the end.
