# src-tauri — Tauri 2 desktop shell (Phase 0 skeleton)

This directory holds the Tauri 2 shell skeleton. It is **intentionally excluded
from the root Cargo workspace** during Phase 0 (`exclude = ["src-tauri"]` in
`../Cargo.toml`) so that `cargo build --workspace` and CI stay fast and do not
pull the full Tauri native dependency tree.

## Why excluded for now

- The Tauri 2 dependency tree is large and platform-toolchain heavy (system
  webview, bundler, icon pipeline). Phase 0's goal is to prove the **8 core
  crates** and the **web** build green in CI — not to ship a runnable desktop app.
- Keeping it out of the workspace means `cargo build --workspace` does not
  require a populated `icons/` set or Tauri system deps to succeed.

## TODO to activate (later phase)

1. Add real bundle icons under `src-tauri/icons/` (use `cargo tauri icon`),
   or adjust `tauri.conf.json > bundle.icon`.
2. Re-include in the workspace: remove the `exclude` entry and add
   `"src-tauri"` to `members` in `../Cargo.toml`, switching this manifest to
   `version.workspace = true` etc.
3. Add `tauri-plugin-*` as needed and depend on `opentake-core`.
4. Wire `#[tauri::command]` thin wrappers per `docs/ARCHITECTURE.md` §2
   (edit_apply / project_open|save / undo|redo / get_timeline / seek /
   import_media / export_start) and the event channel
   (timeline_changed / preview_frame / progress).
5. Run `cargo tauri dev`.

## Current files

- `Cargo.toml` — skeleton manifest (not built by the workspace today).
- `build.rs` — `tauri_build::build()`.
- `src/main.rs` — minimal `tauri::Builder` entrypoint.
- `tauri.conf.json` — points `frontendDist` at `../web/dist`, dev server at `:1420`.
