//! OpenTake desktop shell (Tauri 2).
//!
//! Owns the single authoritative [`AppCore`] as Tauri managed state, registers
//! the `#[tauri::command]` surface ([`commands`]), and bridges the core's
//! [`CoreEvent`] bus to the WebView: every core event is re-emitted as a Tauri
//! event so the front-end read-only mirror can re-sync (`docs/architecture/ARCHITECTURE.md`
//! §2 — "真相源在 Rust，前端持镜像").

mod account;
mod advanced;
mod captions;
mod chat;
mod commands;
// `pub` so the ffmpeg-gated integration test (`tests/export_integration.rs`) can
// drive the export orchestrator (`export::run_export`) against the library
// target. The Tauri command itself is registered below like the other modules.
pub mod export;
pub mod feedback;
mod generation;
mod haptic;
mod home;
mod library;
mod lut;
mod mcp;
mod media;
pub mod motion;
// Public for the same reason as `export`: integration acceptance drives the
// standalone compositing path against a generated project snapshot.
pub mod render;
mod samples;
mod search;
mod secret;
pub mod telemetry;
mod transcribe;

// Streaming playback engine (#53). Feature-gated (`playback-engine`, now a DEFAULT
// feature) and `pub` so the gated GPU+ffmpeg integration test can drive the render
// loop directly. The `playback_*` commands are registered below; a minimal build
// (`--no-default-features`) drops this module and those commands.
#[cfg(feature = "playback-engine")]
pub mod playback;

use std::sync::Arc;

use opentake_core::{AppCore, CoreEvent, IdGen};
use opentake_media::library::LibraryStore;
use opentake_media::MediaEngine;
use tauri::{Emitter, Manager, WindowEvent};
// `RunEvent::Reopen` (Dock click) is a macOS-only variant.
#[cfg(target_os = "macos")]
use tauri::RunEvent;

use crate::media::prewarm::PrewarmScheduler;
use crate::media::MediaState;

/// Production entity IDs must remain unique across save/reopen boundaries.
/// The core's sequential default is intentionally deterministic for tests, but
/// a new desktop process would restart it at `id-1` and collide with IDs loaded
/// from an existing project. UUIDs match the upstream persistence contract and
/// need no mutable state to survive an application restart.
#[derive(Debug, Default)]
struct UuidIdGen;

impl IdGen for UuidIdGen {
    fn next_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

/// Build and run the Tauri application. The `main.rs` binary calls this.
///
/// Lifecycle mirrors upstream's "the app stays resident; closing the window
/// returns to Home" (AppDelegate). Tauri's default — quit when the last window
/// closes — is overridden: [`WindowEvent::CloseRequested`] is intercepted to
/// **hide** the window and tell the front end to return Home, so the process
/// keeps running in the background. Dock-reopen ([`RunEvent::Reopen`]) shows it
/// again. `Cmd+Q` still exits (it raises `ExitRequested`, not prevented here).
pub fn run() {
    // Telemetry is a strict opt-in at the configuration boundary: without an
    // explicit packaged/environment DSN this creates no SDK client or network.
    let _telemetry = telemetry::init_telemetry();

    // Pin ffmpeg/ffprobe before anything decodes (see `resolve_media_tools`).
    resolve_media_tools();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Native dialog selections expand Tauri's runtime scopes. Persist the
        // resulting file + asset grants for Recents, while keeping the static
        // asset scope limited to application-owned cache/data/resources.
        // Tauri requires fs to be initialized before persisted-scope.
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_persisted_scope::init())
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
            let mut core = AppCore::new();
            core.set_id_gen(Arc::new(UuidIdGen));
            let initial_project_epoch = core.project_revision().project_epoch;

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
            let engine = MediaEngine::new(cache_root.clone(), models_dir.clone());

            // Bring up the loopback MCP server (#36) over a session-sharing clone
            // of the core, before the core is moved into managed state. Bundled +
            // user workflow plugins live under <app_data_dir>/workflows. The
            // media bridge (agent inspect_timeline / import_media) is built from
            // the SAME cache/models dirs as the UI's engine, so imports share the
            // same poster/manifest caches.
            let workflows_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("workflows");
            let generation_bridge =
                generation::build_bridge(core.clone(), cache_root.clone(), models_dir.clone());
            let motion_bridge = Arc::new(motion::TauriMotionBridge::new(
                core.clone(),
                cache_root.clone(),
            ));
            let advanced_bridge = Arc::new(advanced::TauriAdvancedWorkflowBridge::new(
                core.clone(),
                cache_root.clone(),
                models_dir.clone(),
            ));
            mcp::spawn(
                core.clone(),
                workflows_dir.clone(),
                cache_root.clone(),
                models_dir.clone(),
                generation_bridge.clone(),
                motion_bridge.clone(),
                advanced_bridge.clone(),
            );
            let chat_state = chat::ChatState::new_with_capabilities(
                core.clone(),
                workflows_dir,
                cache_root.clone(),
                models_dir.clone(),
                generation_bridge.clone(),
                motion_bridge.clone(),
                advanced_bridge.clone(),
            );

            // A global favorite must never silently become a temporary file.
            // Keep the editor usable if app-data resolution fails, but make all
            // library commands return the same explicit initialization error.
            let library_state = match app.path().app_data_dir() {
                Ok(data_dir) => crate::library::LibraryState::new(LibraryStore::new(
                    data_dir.join("OpenTake").join("Library"),
                )),
                Err(error) => crate::library::LibraryState::unavailable(format!(
                    "global library unavailable: could not resolve app data directory: {error}"
                )),
            };

            app.manage(core);
            app.manage(commands::ProjectLifecycleCoordinator::default());
            app.manage(generation_bridge);
            let motion_state = motion::MotionCommandState::new(motion_bridge);
            let motion_transition_state = motion_state.clone();
            app.state::<AppCore>()
                .subscribe_project_identity_transition(move |pending| {
                    if pending {
                        motion_transition_state.cancel_active();
                    }
                });
            app.manage(motion_state);
            app.manage(advanced::AdvancedWorkflowCommandState::new(advanced_bridge));
            app.manage(advanced::MattingModelInstallState::default());
            let advanced_transition_handle = app.handle().clone();
            app.state::<AppCore>()
                .subscribe_project_identity_transition(move |pending| {
                    if pending {
                        advanced_transition_handle
                            .state::<advanced::AdvancedWorkflowCommandState>()
                            .cancel_active();
                    }
                });
            app.manage(chat_state);
            app.manage(MediaState::new(engine));
            app.manage(media::StabilizationAnalysisState::default());
            app.manage(media::LoudnessAnalysisState::default());
            app.manage(media::DenoiseAnalysisState::default());
            app.manage(media::StemSeparationState::default());
            app.manage(media::MediaProxyState::default());
            let proxy_transition_handle = app.handle().clone();
            app.state::<AppCore>()
                .subscribe_project_identity_transition(move |pending| {
                    if pending {
                        proxy_transition_handle
                            .state::<media::MediaProxyState>()
                            .cancel();
                    }
                });
            app.manage(PrewarmScheduler::new(initial_project_epoch));
            app.manage(library_state);
            // Lazily-acquired GPU context for timeline composite previews (#47).
            app.manage(render::RenderState::new());
            // Shared cancel flag for the in-flight `export_video` (#112 progress
            // + cancel). One export runs at a time, so a single flag suffices.
            app.manage(export::ExportControl::default());
            app.manage(feedback::FeedbackState::default());
            // Optional account scaffold. It starts offline and never performs
            // network I/O until the user configures a backend and logs in.
            app.manage(account::AccountState::default());

            // Streaming playback (#53): start the loopback MJPEG transport on the
            // Tauri async runtime (mirrors the MCP server spawn) and register the
            // playback session state. Behind the (now default) `playback-engine`
            // feature; absent in a `--no-default-features` minimal build.
            #[cfg(feature = "playback-engine")]
            {
                let preview_server =
                    tauri::async_runtime::block_on(playback::transport::PreviewServer::start())?;
                app.manage(preview_server);
                app.manage(playback::commands::PlaybackState::new());
            }
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
            commands::export_xmeml,
            commands::export_fcpxml,
            commands::export_fcpxml_modern,
            commands::export_edl,
            commands::export_otio,
            commands::export_subtitles,
            feedback::submit_feedback,
            commands::check_path_exists,
            home::home_projects_sync,
            home::home_project_register,
            home::home_project_remove,
            home::home_project_trash,
            home::home_project_reveal,
            samples::sample_project_materialize,
            media::import_folder,
            media::import_media,
            lut::import_lut,
            media::relink_media,
            media::get_media,
            media::toggle_favorite,
            media::sync_project_favorites,
            media::save_clip_as_media,
            media::extract_audio,
            media::get_waveform,
            media::analyze_stabilization,
            media::cancel_stabilization_analysis,
            media::analyze_loudness,
            media::cancel_loudness_analysis,
            media::prepare_denoise,
            media::cancel_denoise_analysis,
            media::separate_audio_stems,
            media::cancel_stem_separation,
            media::import_stems_to_tracks,
            media::create_media_proxy,
            media::cancel_media_proxy,
            media::remove_media_proxy,
            media::set_proxy_playback_enabled,
            media::get_proxy_playback_enabled,
            media::generate_thumbnail,
            media::request_timeline_sprite,
            media::set_timeline_sprite_interactive,
            media::preview_poster,
            media::preload_media,
            haptic::snap_haptic,
            render::composite_frame,
            render::cancel_composite_frame,
            render::capture_frame_to_media,
            export::export_video,
            export::save_range_as_media,
            export::cancel_export,
            generation::generation_cancel,
            generation::generation_retry,
            motion::motion_capability,
            motion::motion_add,
            motion::motion_edit,
            motion::motion_cancel,
            advanced::matting_model_status,
            advanced::download_matting_model,
            advanced::cancel_matting_model_download,
            advanced::advanced_generate_matte,
            advanced::advanced_remove_object,
            advanced::advanced_match_color,
            advanced::advanced_translate_captions,
            advanced::advanced_apply_caption_translation_review,
            advanced::advanced_script_to_video,
            advanced::cancel_advanced_workflow,
            secret::secret_save,
            secret::secret_load,
            secret::secret_delete,
            account::account_set_backend_url,
            account::account_get_backend_url,
            account::account_login,
            account::account_logout,
            account::account_get_status,
            chat::chat_send,
            chat::chat_history,
            chat::chat_sessions,
            chat::chat_session_set_open,
            chat::chat_cancel,
            transcribe::transcribe_model_status,
            transcribe::download_transcribe_model,
            transcribe::transcribe_media,
            transcribe::transcript_get,
            captions::generate_captions,
            search::search_model_status,
            search::download_search_model,
            search::search_index_status,
            search::search_index_start,
            search::search_query,
            library::library_list,
            library::library_favorite,
            library::library_unfavorite,
            library::library_categorize,
            library::library_rename,
            library::library_delete,
            library::library_import_to_project,
            #[cfg(feature = "playback-engine")]
            playback::commands::playback_start,
            #[cfg(feature = "playback-engine")]
            playback::commands::playback_pause,
            #[cfg(feature = "playback-engine")]
            playback::commands::playback_stop,
            #[cfg(feature = "playback-engine")]
            playback::commands::playback_seek,
            #[cfg(feature = "playback-engine")]
            playback::commands::get_preview_endpoint,
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

/// Pin `OPENTAKE_FFMPEG` / `OPENTAKE_FFPROBE` before any media initialization.
///
/// A packaged application must use the two regular sidecars beside its own
/// executable. Release builds deliberately pin the expected sibling paths even
/// when a file is missing, so a corrupt package fails closed instead of silently
/// invoking an attacker-controlled or developer-installed binary from PATH.
/// Debug builds retain explicit overrides and host discovery for development.
fn resolve_media_tools() {
    let packaged = ["ffmpeg", "ffprobe"].map(opentake_media::ffmpeg_status::packaged_sidecar_path);
    if let [Some(ffmpeg), Some(ffprobe)] = packaged {
        std::env::set_var("OPENTAKE_FFMPEG", ffmpeg);
        std::env::set_var("OPENTAKE_FFPROBE", ffprobe);
        return;
    }

    if !cfg!(debug_assertions) {
        if let Ok(executable) = std::env::current_exe() {
            if let Some(parent) = executable.parent() {
                let extension = if cfg!(windows) { ".exe" } else { "" };
                std::env::set_var("OPENTAKE_FFMPEG", parent.join(format!("ffmpeg{extension}")));
                std::env::set_var(
                    "OPENTAKE_FFPROBE",
                    parent.join(format!("ffprobe{extension}")),
                );
            }
        }
        return;
    }

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
    if let CoreEvent::ProjectOpened { project_epoch, .. } = event {
        if let Some(prewarm) = app.try_state::<PrewarmScheduler>() {
            prewarm.activate_project(*project_epoch);
        }
        if let Some(generation) =
            app.try_state::<std::sync::Arc<generation::TauriGenerationBridge>>()
        {
            generation.recover_current_project();
        }
    }
    #[cfg(feature = "playback-engine")]
    {
        if let Some(playback) = app.try_state::<playback::PlaybackState>() {
            let invalidated = match event {
                CoreEvent::TimelineChanged {
                    project_epoch,
                    version,
                } => playback.invalidate_timeline(*project_epoch, *version),
                CoreEvent::ProjectOpened { project_epoch, .. } => {
                    playback.activate_project_event(*project_epoch)
                }
                CoreEvent::ProjectSaved { .. } | CoreEvent::MediaChanged { .. } => None,
            };
            if let (Some(identity), Some(server)) = (
                invalidated,
                app.try_state::<std::sync::Arc<playback::PreviewServer>>(),
            ) {
                server.clear_session(&identity);
            }
        }
    }
    forward_core_event(event, |name, payload| app.emit(name, payload));
}

/// Emit the stable front-end name and the original tagged payload while making
/// teardown failures explicitly non-fatal. Keeping this boundary independent
/// of `AppHandle` lets the full mapping and failure policy run in unit tests.
fn forward_core_event<E>(
    event: &CoreEvent,
    emit: impl FnOnce(&'static str, &CoreEvent) -> Result<(), E>,
) {
    let name = match event {
        CoreEvent::TimelineChanged { .. } => "timeline_changed",
        CoreEvent::ProjectOpened { .. } => "project_opened",
        CoreEvent::ProjectSaved { .. } => "project_saved",
        CoreEvent::MediaChanged { .. } => "media_changed",
    };
    let _ = emit(name, event);
}

#[cfg(test)]
mod id_tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn production_ids_are_unique_and_uuid_shaped() {
        let generator = UuidIdGen;
        let ids = (0..256)
            .map(|_| generator.next_id())
            .collect::<HashSet<_>>();

        assert_eq!(ids.len(), 256);
        assert!(ids.iter().all(|id| uuid::Uuid::parse_str(id).is_ok()));
    }

    #[test]
    fn core_event_forwarding_maps_every_name_and_tagged_payload() {
        let cases = [
            (
                CoreEvent::TimelineChanged {
                    project_epoch: 1,
                    version: 2,
                },
                "timeline_changed",
                serde_json::json!({
                    "kind": "timeline_changed",
                    "projectEpoch": 1,
                    "version": 2,
                }),
            ),
            (
                CoreEvent::ProjectOpened {
                    path: "/project.otk".into(),
                    project_epoch: 3,
                    version: 0,
                },
                "project_opened",
                serde_json::json!({
                    "kind": "project_opened",
                    "path": "/project.otk",
                    "projectEpoch": 3,
                    "version": 0,
                }),
            ),
            (
                CoreEvent::ProjectSaved {
                    path: "/project.otk".into(),
                    project_epoch: 3,
                },
                "project_saved",
                serde_json::json!({
                    "kind": "project_saved",
                    "path": "/project.otk",
                    "projectEpoch": 3,
                }),
            ),
            (
                CoreEvent::MediaChanged {
                    project_epoch: 3,
                    count: 4,
                },
                "media_changed",
                serde_json::json!({
                    "kind": "media_changed",
                    "projectEpoch": 3,
                    "count": 4,
                }),
            ),
        ];

        for (event, expected_name, expected_payload) in cases {
            forward_core_event(&event, |name, payload| {
                assert_eq!(name, expected_name);
                assert_eq!(serde_json::to_value(payload).unwrap(), expected_payload);
                Ok::<(), ()>(())
            });
        }
    }

    #[test]
    fn core_event_forwarding_swallows_emit_failure_and_delivery_continues() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let bus = opentake_core::EventBus::new();
        let delivered = Arc::new(AtomicUsize::new(0));
        bus.subscribe(|event| {
            forward_core_event(event, |_name, _payload| Err::<(), _>("WebView unavailable"));
        });
        let delivered_sink = Arc::clone(&delivered);
        bus.subscribe(move |_| {
            delivered_sink.fetch_add(1, Ordering::SeqCst);
        });

        bus.emit(&CoreEvent::TimelineChanged {
            project_epoch: 5,
            version: 8,
        });
        assert_eq!(delivered.load(Ordering::SeqCst), 1);
    }
}
