//! OpenTake desktop shell (Tauri 2).
//!
//! Owns the single authoritative [`AppCore`] as Tauri managed state, registers
//! the `#[tauri::command]` surface ([`commands`]), and bridges the core's
//! [`CoreEvent`] bus to the WebView: every core event is re-emitted as a Tauri
//! event so the front-end read-only mirror can re-sync (`docs/ARCHITECTURE.md`
//! §2 — "真相源在 Rust，前端持镜像").

mod commands;
mod media;
mod secret;

use opentake_core::{AppCore, CoreEvent};
use opentake_media::MediaEngine;
use tauri::{Emitter, Manager};

use crate::media::MediaState;

/// Build and run the Tauri application. The `main.rs` binary calls this.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
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

            app.manage(core);
            app.manage(MediaState::new(engine));
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
            media::import_folder,
            media::import_media,
            media::get_media,
            secret::secret_save,
            secret::secret_load,
            secret::secret_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
