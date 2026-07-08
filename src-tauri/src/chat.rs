//! In-app chat commands (HANDOFF §3.3, P1). Thin Tauri surface over the
//! agent's [`ChatLoop`]: `chat_send` spawns a turn, streaming `chat_delta` /
//! `chat_tool_call` / `chat_done` events as the loop runs; `chat_history`
//! returns the current message log; `chat_cancel` stops a running turn.
//!
//! The chat loop shares the SAME dispatcher shape the MCP server uses: live
//! [`AppCore`] handle, the same workflow registry scan, the same media bridge,
//! and the same BYOK key-store boundary. That keeps tool availability and tool
//! behavior consistent between the panel and the external MCP surface.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use opentake_agent::chat::{ChatLoop, ChatMessage, ChatSession, EmitLoop, LoopError, LoopEvent};
use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_gen::{KeyStore, KeyringStore};

use opentake_core::AppCore;

/// Managed state: one [`ChatLoop`] over the shared core + plugin registry, a
/// map of live sessions, and a map of cancel flags for in-flight turns.
#[derive(Clone)]
pub struct ChatState {
    loop_: ChatLoop,
    sessions: Arc<Mutex<HashMap<String, ChatSession>>>,
    cancels: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl ChatState {
    /// Build the state in `setup`: a dispatcher over the live core + workflow
    /// registry + the same media bridge the desktop MCP server uses.
    pub fn new(
        core: AppCore,
        workflows_dir: PathBuf,
        cache_root: PathBuf,
        models_dir: PathBuf,
    ) -> Self {
        let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
        let registry = Arc::new(RwLock::new(crate::mcp::build_registry(&workflows_dir)));
        let bridge = crate::mcp::build_media_bridge(core, cache_root, models_dir);
        let dispatcher = Arc::new(Dispatcher::with_bridge(
            handle,
            registry.clone(),
            Some(bridge),
        ));
        let store: Arc<dyn KeyStore> = Arc::new(KeyringStore::new());
        ChatState {
            loop_: ChatLoop::new(dispatcher, registry, store),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            cancels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Snapshot a session for a running turn. The map entry stays in place so
    /// `chat_history` can still return the last persisted state while the turn
    /// is in flight.
    fn take_session(&self, session_id: &str) -> ChatSession {
        let sessions = self.sessions.lock().expect("sessions lock");
        sessions
            .get(session_id)
            .cloned()
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
/// `chat_delta` / `chat_tool_call` / `chat_done` events.
#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    state: State<'_, ChatState>,
    session_id: String,
    text: String,
    chat_provider: String,
) -> Result<(), String> {
    {
        let cancels = state.cancels.lock().map_err(|e| e.to_string())?;
        if cancels.contains_key(&session_id) {
            return Err("a turn is already running on this session".into());
        }
    }

    let cancel = Arc::new(AtomicBool::new(false));
    state
        .cancels
        .lock()
        .map_err(|e| e.to_string())?
        .insert(session_id.clone(), cancel.clone());

    let mut session = state.take_session(&session_id);
    session.provider = Some(chat_provider.clone());
    session.messages.push(ChatMessage::user(text.clone()));
    state.put_session(session.clone());
    let state_clone = state.inner().clone();
    let sid = session_id.clone();

    tauri::async_runtime::spawn(async move {
        let emitter = AppEmitter { app: app.clone() };
        let result = state_clone
            .loop_
            .run_turn(&mut session, chat_provider, text, &emitter, cancel)
            .await;

        match &result {
            Err(LoopError::Cancelled) => {
                let _ = app.emit(
                    "chat_done",
                    DonePayload {
                        session_id: sid.clone(),
                        message: ChatMessage::assistant(String::new(), Vec::new()),
                    },
                );
            }
            Err(e) => {
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
            Ok(()) => {}
        }

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
/// the session doesn't exist yet.
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
/// no turn is running.
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
    fn take_and_put_session_round_trips() {
        let state = ChatState::new(
            AppCore::new(),
            std::env::temp_dir().join("no-workflows"),
            std::env::temp_dir().join("chat-cache"),
            std::env::temp_dir().join("chat-models"),
        );
        state.put_session(ChatSession::new("s1"));
        let taken = state.take_session("s1");
        assert_eq!(taken.id, "s1");
        let second = state.take_session("s1");
        assert_eq!(second.id, "s1");
        assert!(second.messages.is_empty());
    }

    #[test]
    fn history_snapshot_remains_visible_while_turn_owns_a_clone() {
        let state = ChatState::new(
            AppCore::new(),
            std::env::temp_dir().join("no-workflows"),
            std::env::temp_dir().join("chat-cache"),
            std::env::temp_dir().join("chat-models"),
        );
        let mut session = ChatSession::new("s1");
        session.messages.push(ChatMessage::user("hello"));
        state.put_session(session.clone());

        let running_copy = state.take_session("s1");
        assert_eq!(running_copy.messages.len(), 1);

        let history = state
            .sessions
            .lock()
            .unwrap()
            .get("s1")
            .cloned()
            .unwrap()
            .messages;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].content, "hello");
    }

    #[test]
    fn event_payloads_serialize_in_camel_case() {
        let payload = DeltaPayload {
            session_id: "sess-1".into(),
            delta: "hi".into(),
        };
        let json = serde_json::to_value(payload).unwrap();
        assert_eq!(json["sessionId"], "sess-1");
        assert_eq!(json["delta"], "hi");

        let done = DonePayload {
            session_id: "sess-1".into(),
            message: ChatMessage::assistant("done", Vec::new()),
        };
        let json = serde_json::to_value(done).unwrap();
        assert_eq!(json["sessionId"], "sess-1");
        assert_eq!(json["message"]["role"], "assistant");
    }
}
