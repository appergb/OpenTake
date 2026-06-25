//! Spawns the loopback MCP server (#36) on the Tauri async runtime.
//!
//! The server exposes the in-process tool dispatcher over Streamable-HTTP at
//! `http://127.0.0.1:19789/mcp` so external agents (`claude mcp add --transport
//! http opentake http://127.0.0.1:19789/mcp`, Cursor, Codex, …) can drive the
//! same [`AppCore`] the UI edits. The plugin registry seeds the bundled
//! workflows (e.g. the default audio-first Skill) plus any user-authored plugins
//! under `<app_data_dir>/workflows`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
use opentake_agent::mcp::server;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_core::AppCore;

/// Built-in workflows + any user-authored plugins under `workflows_dir`
/// (user plugins override a built-in with the same id, since `register` replaces
/// by id and runs after the built-ins).
fn build_registry(workflows_dir: &Path) -> PluginRegistry {
    let mut registry = PluginRegistry::with_builtins();
    if workflows_dir.is_dir() {
        let (user, errors) = PluginRegistry::scan(workflows_dir);
        for e in &errors {
            eprintln!("[mcp] workflow plugin load error: {e}");
        }
        for plugin in user.installed() {
            registry.register(plugin.clone());
        }
    }
    registry
}

/// Spawn the MCP server. `core` is a clone that shares the live session;
/// `workflows_dir` is `<app_data_dir>/workflows`. A bind failure (port in use) is
/// logged, not fatal — the app keeps running without the agent network face.
pub fn spawn(core: AppCore, workflows_dir: PathBuf) {
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core));
    let registry = Arc::new(RwLock::new(build_registry(&workflows_dir)));
    tauri::async_runtime::spawn(async move {
        let addr = match server::DEFAULT_ADDR.parse() {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("[mcp] invalid bind address {}: {e}", server::DEFAULT_ADDR);
                return;
            }
        };
        if let Err(e) = server::serve(addr, handle, registry).await {
            eprintln!("[mcp] server stopped: {e}");
        }
    });
}
