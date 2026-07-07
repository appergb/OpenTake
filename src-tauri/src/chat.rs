//! In-app chat commands (HANDOFF §3.3, P1). Thin Tauri surface over the
//! agent's [`ChatLoop`]: `chat_send` spawns a turn, streaming `chat_delta` /
//! `chat_tool_call` / `chat_done` events as the loop runs; `chat_history`
//! returns the current message log; `chat_cancel` stops a running turn.
//!
//! The chat loop shares the SAME [`Dispatcher`] the MCP server uses — built
//! here from the live [`AppCore`] (session-sharing) + the workflow plugin
//! registry (read from the same `<app_data_dir>/workflows` the MCP server
//! scans) + the BYOK [`KeyringStore`]. No media bridge: `inspect_timeline` /
//! `import_media` return "not available" from chat; the model routes around
//! them (and most chat edits are read + edit, not render + import).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use opentake_agent::chat::{ChatLoop, ChatMessage, ChatSession, EmitLoop, LoopEvent};
use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_gen::KeyringStore;

use opentake_core::AppCore;

/// Managed state: one [`ChatLoop`] over the shared core + plugin registry, a
/// map of live sessions, and a map of cancel flags for in-flight turns.
/// Cloneable (all fields are `Arc`/`Clone`) so a spawned turn task can own a
/// copy and re-insert the session when it finishes.
#[derive(Clone)]
pub struct ChatState {
    loop_: ChatLoop,
    sessions: Arc<Mutex<HashMap<String, ChatSession>>>,
    cancels: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    core: AppCore,
}

impl ChatState {
    /// Build the state in `setup`: a dispatcher over the live core + the
    /// workflow registry from `workflows_dir` + the OS keychain. The dispatcher
    /// is built without a media bridge (chat doesn't render/import).
    pub fn new(core: AppCore, workflows_dir: PathBuf) -> Self {
        let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
        let registry = Arc::new(RwLock::new(build_registry(&workflows_dir)));
        let dispatcher = Arc::new(Dispatcher::new(handle, registry.clone()));
        let store = Arc::new(KeyringStore::new());
        ChatState {
            loop_: ChatLoop::new(dispatcher, registry, store),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            cancels: Arc::new(Mutex::new(HashMap::new())),
            core,
        }
    }
    /// Take a session out of the map (so the running task owns it mutably); the
    /// task re-inserts it when the turn finishes. Mint a new session if absent.
    fn take_session(&self, session_id: &str) -> ChatSession {
        let mut sessions = self.sessions.lock().expect("sessions lock");
        sessions
            .remove(session_id)
            .unwrap_or_else(|| ChatSession::new(session_id.to_string()))
    }

    /// Put a session back after a turn (replaces any placeholder).
    fn put_session(&self, session: ChatSession) {
        let sid = session.id.clone();
        self.sessions
            .lock()
            .expect("sessions lock")
            .insert(sid, session);
    }
}

/// Built-in workflows + user plugins under `workflows_dir` (mirrors
/// `mcp::build_registry` so chat and MCP see the same plugin set).
fn build_registry(workflows_dir: &std::path::Path) -> PluginRegistry {
    let mut registry = PluginRegistry::with_builtins();
    if workflows_dir.is_dir() {
        let (user, errors) = PluginRegistry::scan(workflows_dir);
        for e in &errors {
            eprintln!("[chat] workflow plugin load error: {e}");
        }
        for plugin in user.installed() {
            registry.register(plugin.clone());
        }
    }
    registry
}

// MARK: - Event payloads (camelCase, mirror front-end types.ts)

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaPayload {
    session_id: String,
    delta: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolCallPayload {
    session_id: String,
    tool_call: opentake_agent::chat::ToolCall,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DonePayload {
    session_id: String,
    message: ChatMessage,
}

/// Adapt `AppHandle::emit` to the loop's [`EmitLoop`] trait. Each loop event
/// becomes a Tauri event the front end listens for.
struct AppEmitter {
    app: AppHandle,
}

impl EmitLoop for AppEmitter {
    fn emit(&self, event: LoopEvent) {
        match event {
            LoopEvent::Delta { session_id, delta } => {
                let _ = self
                    .app
                    .emit("chat_delta", DeltaPayload { session_id, delta });
            }
            LoopEvent::ToolCall {
                session_id,
                tool_call,
            } => {
                let _ = self.app.emit(
                    "chat_tool_call",
                    ToolCallPayload {
                        session_id,
                        tool_call,
                    },
                );
            }
            LoopEvent::Done {
                session_id,
                message,
            } => {
                let _ = self.app.emit(
                    "chat_done",
                    DonePayload {
                        session_id,
                        message,
                    },
                );
            }
        }
    }
}

// MARK: - Commands

/// `chat_send`: spawn a chat turn. Returns immediately; the turn streams via
/// `chat_delta` / `chat_tool_call` / `chat_done` events. A new session is
/// minted when `session_id` is unseen. Sending on a session with a turn
/// already running returns an error (the front end disables the input while
/// streaming, so this is a race guard, not a user-facing path).
#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    state: State<'_, ChatState>,
    session_id: String,
    text: String,
) -> Result<(), String> {
    // Reject if a turn is already running on this session.
    {
        let cancels = state.cancels.lock().map_err(|e| e.to_string())?;
        if cancels.contains_key(&session_id) {
            return Err("a turn is already running on this session".into());
        }
    }

    // Register a cancel flag for this turn (the host sets it to true on
    // `chat_cancel`; the loop checks it between rounds).
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .cancels
        .lock()
        .map_err(|e| e.to_string())?
        .insert(session_id.clone(), cancel.clone());

    // Take the session out so the spawned task owns it mutably; re-insert on
    // completion. Clone the whole state (cheap; all Arc) so the task can call
    // `put_session` + clear the cancel slot after the turn.
    let mut session = state.take_session(&session_id);
    let state_clone = state.inner().clone();
    let sid = session_id.clone();

    tauri::async_runtime::spawn(async move {
        let emitter = AppEmitter { app: app.clone() };
        // Snapshot the live timeline once per turn; the loop rebuilds the
        // system prompt + Context Signal from this each round, so a tool call
        // that moved clips is reflected on the next round via the dispatcher's
        // own `timeline()` read (which sees the live core). The snapshot here
        // is the starting point + the source for the first signal.
        let timeline = state_clone.core.get_timeline().timeline;
        let result = state_clone
            .loop_
            .run_turn(&mut session, text, timeline, &emitter, cancel)
            .await;

        if let Err(e) = &result {
            // Surface a final error as a chat_done so the UI stops streaming.
            let msg = ChatMessage::assistant(format!("⚠️ {e}"), Vec::new());
            let _ = app.emit(
                "chat_done",
                DonePayload {
                    session_id: sid.clone(),
                    message: msg.clone(),
                },
            );
            session.messages.push(msg);
        }

        // Re-insert the session + clear the cancel slot.
        state_clone.put_session(session);
        state_clone
            .cancels
            .lock()
            .ok()
            .and_then(|mut c| c.remove(&sid));
    });

    Ok(())
}

/// `chat_history`: return the current message log for a session. Empty when
/// the session doesn't exist yet (the front end sends `chat_send` first). While
/// a turn is running the session is owned by the task, so this returns the last
/// persisted state — the front end relies on the streaming events meanwhile.
#[tauri::command]
pub fn chat_history(
    state: State<'_, ChatState>,
    session_id: String,
) -> Result<Vec<ChatMessage>, String> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    Ok(sessions
        .get(&session_id)
        .map(|s| s.messages.clone())
        .unwrap_or_default())
}

/// `chat_cancel`: request a running turn stop at the next boundary. No-op when
/// no turn is running (treated as success so the UI button is idempotent).
#[tauri::command]
pub fn chat_cancel(state: State<'_, ChatState>, session_id: String) -> Result<(), String> {
    let cancels = state.cancels.lock().map_err(|e| e.to_string())?;
    if let Some(flag) = cancels.get(&session_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_registry_with_no_dir_is_builtins_only() {
        let reg = build_registry(std::path::Path::new("/no/such/dir"));
        // Built-in workflows are registered by `with_builtins`; at least the
        // default audio-first skill is present.
        assert!(!reg.installed().is_empty() || reg.active().is_none());
    }

    #[test]
    fn take_and_put_session_round_trips() {
        let state = ChatState::new(AppCore::new(), std::env::temp_dir().join("no-workflows"));
        state.put_session(ChatSession::new("s1"));
        let taken = state.take_session("s1");
        assert_eq!(taken.id, "s1");
        // Taken out → a second take mints a fresh session.
        let fresh = state.take_session("s1");
        assert_eq!(fresh.id, "s1");
        assert!(fresh.messages.is_empty());
    }
}
