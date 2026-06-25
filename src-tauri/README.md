# src-tauri — Tauri 2 desktop shell

The Tauri 2 desktop shell for OpenTake. It is a workspace member
(`members = [..., "src-tauri"]` in `../Cargo.toml`) and holds the authoritative
[`opentake_core::AppCore`] as managed state, exposing a thin `#[tauri::command]`
surface over the core's DTO handlers plus a `CoreEvent` → Tauri-event bridge
(`docs/ARCHITECTURE.md` §2 — "真相源在 Rust，前端持镜像").

## Commands (`src/commands.rs`)

| Command | Returns | Maps to |
|---|---|---|
| `get_timeline` | `{ timeline, version }` | `AppCore::get_timeline` |
| `edit_apply { command }` | `EditResult` | `EditCommand` (via `EditRequest`) |
| `undo` / `redo` | `EditResult` | `AppCore::undo/redo` |
| `can_undo` / `can_redo` | `bool` | history affordances |
| `project_new` | — | fresh session |
| `project_open { path }` | `{ timeline, version }` | open `.opentake` |
| `project_save { path? }` | written path | save / save-as |

`EditCommand` is not `Deserialize` (it carries engine value types), so editing
goes through a serde-friendly `EditRequest` (tagged `{ "type": "addClips", … }`)
that maps 1:1 onto the variants the front end issues.

## Events (`src/lib.rs`)

`AppCore`'s `EventBus` is subscribed in `setup` and forwarded to the WebView:

- `timeline_changed { version }` — re-sync the read-only mirror
- `project_opened { path, version }`
- `project_saved { path }`

## Running

```bash
# build only (no Tauri CLI needed)
cargo build -p opentake-tauri

# dev (front end + shell); needs the Tauri CLI
pnpm -C web exec tauri dev     # or: cargo tauri dev  (if cargo-tauri installed)
```

`tauri.conf.json` points `frontendDist` at `../web/dist`, the dev server at
`:1420`, and window size to 1600×1000 / min 960×600 (SPEC §2.8). Icons live in
`icons/`; `capabilities/default.json` grants the `dialog` plugin + event perms.

## Files

- `Cargo.toml` — manifest (depends on `opentake-core`, `opentake-ops`, `opentake-domain`, `tauri-plugin-dialog`).
- `build.rs` — `tauri_build::build()`.
- `src/main.rs` — calls `opentake_tauri_lib::run()`.
- `src/lib.rs` — builder + state + event bridge.
- `src/commands.rs` — `#[tauri::command]` shims + `EditRequest`.
- `tauri.conf.json`, `capabilities/default.json`, `icons/`.
