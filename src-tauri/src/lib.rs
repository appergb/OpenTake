//! OpenTake desktop shell (Tauri 2).
//!
//! Owns the single authoritative [`AppCore`] as Tauri managed state, registers
//! the `#[tauri::command]` surface ([`commands`]), and bridges the core's
//! [`CoreEvent`] bus to the WebView: every core event is re-emitted as a Tauri
//! event so the front-end read-only mirror can re-sync (`docs/ARCHITECTURE.md`
//! §2 — "真相源在 Rust，前端持镜像").

mod commands;
// `pub` so the ffmpeg-gated integration test (`tests/export_integration.rs`) can
// drive the export orchestrator (`export::run_export`) against the library
// target. The Tauri command itself is registered below like the other modules.
pub mod export;
mod library;
mod mcp;
mod media;
mod render;
mod secret;

use opentake_core::{AppCore, CoreEvent};
use opentake_media::library::LibraryStore;
use opentake_media::MediaEngine;
use tauri::{Emitter, Manager, WindowEvent};
// `RunEvent::Reopen` (Dock click) is a macOS-only variant.
#[cfg(target_os = "macos")]
use tauri::RunEvent;

use crate::media::MediaState;

/// Build and run the Tauri application. The `main.rs` binary calls this.
///
/// Lifecycle mirrors upstream's "the app stays resident; closing the window
/// returns to Home" (AppDelegate). Tauri's default — quit when the last window
/// closes — is overridden: [`WindowEvent::CloseRequested`] is intercepted to
/// **hide** the window and tell the front end to return Home, so the process
/// keeps running in the background. Dock-reopen ([`RunEvent::Reopen`]) shows it
/// again. `Cmd+Q` still exits (it raises `ExitRequested`, not prevented here).
pub fn run() {
    // Pin ffmpeg/ffprobe before anything decodes (see `resolve_media_tools`).
    resolve_media_tools();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Background-run: don't quit, hide and return to Home.
                api.prevent_close();
                // Flush the open project before hiding so background-run never
                // loses edits (autosave is debounced; this is the final write).
                // No-op when no project is open (save_project returns an error we
                // intentionally ignore).
                if let Some(core) = window.app_handle().try_state::<AppCore>() {
                    let _ = core.save_project(None);
                }
                let _ = window.hide();
                let _ = window.app_handle().emit("go_home", ());
            }
        })
        .setup(|app| {
            // Keep a Dock icon + normal app behavior while the window is hidden,
            // so the user can reopen from the Dock (upstream: NSApp .regular).
            #[cfg(target_os = "macos")]
            let _ = app
                .handle()
                .set_activation_policy(tauri::ActivationPolicy::Regular);

            // The one authoritative editing session, shared with every command.
            let core = AppCore::new();

            // Forward core events to the WebView. The closure runs on whatever
            // thread emitted the event (after the core released its lock), so
            // calling back into Tauri here is safe.
            let handle = app.handle().clone();
            core.subscribe(move |event: &CoreEvent| {
                forward_event(&handle, event);
            });

            // The media engine: cache root = app cache dir, models dir = app
            // data dir (SPEC §8.4). Fall back to the OS temp dir if either
            // platform path is unavailable, so importing still works.
            let cache_root = app
                .path()
                .app_cache_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("media-cache");
            let models_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("models");
            let engine = MediaEngine::new(cache_root, models_dir);

            // Bring up the loopback MCP server (#36) over a session-sharing clone
            // of the core, before the core is moved into managed state. Bundled +
            // user workflow plugins live under <app_data_dir>/workflows.
            let workflows_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("workflows");
            mcp::spawn(core.clone(), workflows_dir);

            // Global asset library (#37/#54): a cross-project copy-on-favorite
            // store under <app_data_dir>/OpenTake/Library, falling back to the OS
            // temp dir if the platform data path is unavailable so favoriting
            // still works. Lazily created on first write by the store itself.
            let library_root = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("OpenTake")
                .join("Library");
            let library_store = LibraryStore::new(library_root);

            app.manage(core);
            app.manage(MediaState::new(engine));
            app.manage(crate::library::LibraryState::new(library_store));
            // Lazily-acquired GPU context for timeline composite previews (#47).
            app.manage(render::RenderState::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_timeline,
            commands::edit_apply,
            commands::undo,
            commands::redo,
            commands::can_undo,
            commands::can_redo,
            commands::project_new,
            commands::project_open,
            commands::project_save,
            commands::get_default_project_dir,
            commands::export_fcpxml,
            media::import_folder,
            media::import_media,
            media::relink_media,
            media::get_media,
            media::get_waveform,
            render::composite_frame,
            export::export_video,
            secret::secret_save,
            secret::secret_load,
            secret::secret_delete,
            library::library_list,
            library::library_favorite,
            library::library_unfavorite,
            library::library_categorize,
            library::library_rename,
            library::library_delete,
            library::library_import_to_project,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, _event| {
            // Dock-reopen with no visible window (we hide on close) shows it again.
            // `RunEvent::Reopen` only exists on macOS; other platforms rely on the
            // tray / OS to re-surface the window (a cross-platform follow-up).
            #[cfg(target_os = "macos")]
            if let RunEvent::Reopen {
                has_visible_windows,
                ..
            } = _event
            {
                if !has_visible_windows {
                    if let Some(win) = _app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
            }
        });
}

/// Locate `ffmpeg` / `ffprobe` and export `OPENTAKE_FFMPEG` / `OPENTAKE_FFPROBE`
/// (the override `opentake-media`'s `ff` module reads) so decoding works in a
/// packaged app.
///
/// A macOS `.app` launched from Finder/Dock inherits the minimal **launchd**
/// `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin`), which omits Homebrew
/// (`/opt/homebrew/bin`) and `/usr/local/bin`. A PATH-only `ffmpeg` lookup then
/// fails and every frame decode returns nothing — the timeline preview stays
/// black even though the code is correct. Pin an absolute path from the common
/// install locations instead. (Bundling the binaries via Tauri `externalBin` is
/// the cross-machine follow-up; this unblocks any host that has ffmpeg on disk.)
fn resolve_media_tools() {
    for (key, bin) in [
        ("OPENTAKE_FFMPEG", "ffmpeg"),
        ("OPENTAKE_FFPROBE", "ffprobe"),
    ] {
        if std::env::var_os(key).is_some() {
            continue; // an explicit override always wins
        }
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        if let Some(path) = std::env::var_os("PATH") {
            dirs.extend(std::env::split_paths(&path));
        }
        for p in [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/opt/local/bin",
            "/usr/bin",
        ] {
            dirs.push(std::path::PathBuf::from(p));
        }
        if let Some(found) = dirs.into_iter().map(|d| d.join(bin)).find(|c| c.is_file()) {
            std::env::set_var(key, found);
        }
    }
}

/// Map a [`CoreEvent`] onto a front-end Tauri event. The event name matches the
/// `kind` tag the front end listens for; the payload is the event itself
/// (serialized with its `kind`-tagged shape).
fn forward_event(app: &tauri::AppHandle, event: &CoreEvent) {
    let name = match event {
        CoreEvent::TimelineChanged { .. } => "timeline_changed",
        CoreEvent::ProjectOpened { .. } => "project_opened",
        CoreEvent::ProjectSaved { .. } => "project_saved",
        CoreEvent::MediaChanged { .. } => "media_changed",
    };
    // Best-effort: a missing WebView (e.g. during teardown) must not panic the
    // emitting thread.
    let _ = app.emit(name, event);
}
