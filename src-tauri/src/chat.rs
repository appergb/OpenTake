//! In-app chat commands (HANDOFF §3.3, P1). Thin Tauri surface over the
//! agent's [`ChatLoop`]: `chat_send` spawns a turn, streaming `chat_delta` /
//! `chat_tool_call` / `chat_done` events as the loop runs; `chat_history`
//! returns the current message log; `chat_cancel` stops a running turn.
//!
//! The chat loop shares the SAME dispatcher shape the MCP server uses: live
//! [`AppCore`] handle, the same workflow registry scan, the same media bridge,
//! and the same BYOK key-store boundary. That keeps tool availability and tool
//! behavior consistent between the panel and the external MCP surface.

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use opentake_agent::chat::{
    ChatLoop, ChatMessage, ChatSession, ChatSessionStore, ChatTurnGate, EmitLoop, LoopError,
    LoopEvent,
};
use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::tools::result::ToolResult;
use opentake_gen::{KeyStore, KeyringStore};

use opentake_core::AppCore;

/// Managed state: one [`ChatLoop`] over the shared core + plugin registry, a
/// map of live sessions, and a map of cancel flags for in-flight turns.
#[derive(Clone)]
pub struct ChatState {
    core: AppCore,
    loop_: ChatLoop,
    sessions: Arc<Mutex<HashMap<SessionKey, ChatSession>>>,
    cancels: Arc<Mutex<HashMap<SessionKey, Arc<AtomicBool>>>>,
    persistence: Arc<Mutex<()>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SessionKey {
    project_epoch: u64,
    project_dir: PathBuf,
    session_id: String,
}

#[derive(Clone)]
struct ChatProjectContext {
    project_epoch: u64,
    project_dir: PathBuf,
    store: Arc<ChatSessionStore>,
}

impl ChatProjectContext {
    fn key(&self, session_id: &str) -> SessionKey {
        SessionKey {
            project_epoch: self.project_epoch,
            project_dir: self.project_dir.clone(),
            session_id: session_id.to_string(),
        }
    }
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
        let bridge = crate::mcp::build_media_bridge(core.clone(), cache_root, models_dir);
        let dispatcher = Arc::new(Dispatcher::with_bridge(
            handle,
            registry.clone(),
            Some(bridge),
        ));
        let store: Arc<dyn KeyStore> = Arc::new(KeyringStore::new());
        let sessions = Arc::new(Mutex::new(HashMap::new()));
        let cancels = Arc::new(Mutex::new(HashMap::new()));
        let state = ChatState {
            core,
            loop_: ChatLoop::new(dispatcher, registry, store),
            sessions: sessions.clone(),
            cancels: cancels.clone(),
            persistence: Arc::new(Mutex::new(())),
        };
        state.core.subscribe(move |event| {
            let (project_epoch, project_dir) = match event {
                opentake_core::CoreEvent::ProjectOpened {
                    path,
                    project_epoch,
                    ..
                }
                | opentake_core::CoreEvent::ProjectSaved {
                    path,
                    project_epoch,
                } => (
                    *project_epoch,
                    (!path.is_empty()).then(|| PathBuf::from(path)),
                ),
                _ => return,
            };
            if let Ok(mut running) = cancels.lock() {
                running.retain(|key, cancel| {
                    let current = key.project_epoch == project_epoch
                        && project_dir.as_ref() == Some(&key.project_dir);
                    if !current {
                        cancel.store(true, Ordering::Relaxed);
                    }
                    current
                });
            }
            if let Ok(mut cached) = sessions.lock() {
                cached.retain(|key, _| {
                    key.project_epoch == project_epoch
                        && project_dir.as_ref() == Some(&key.project_dir)
                });
            }
        });
        state
    }

    fn project_context(&self) -> Result<ChatProjectContext, String> {
        let _identity = self.core.lock_project_identity_workflow();
        let snapshot = self.core.runtime_snapshot();
        let project_dir = snapshot
            .project_dir
            .ok_or_else(|| "save the project before starting an Agent chat".to_string())?;
        let store = Arc::new(ChatSessionStore::open(&project_dir).map_err(|e| e.to_string())?);
        self.core
            .ensure_project_root_identity_for_project(
                snapshot.project_epoch,
                &project_dir,
                store.root().identity(),
            )
            .map_err(|e| e.to_string())?;
        Ok(ChatProjectContext {
            project_epoch: snapshot.project_epoch,
            project_dir,
            store,
        })
    }

    fn ensure_project_context(&self, project: &ChatProjectContext) -> Result<(), String> {
        self.core
            .ensure_project_root_identity_for_project(
                project.project_epoch,
                &project.project_dir,
                project.store.root().identity(),
            )
            .map_err(|e| e.to_string())
    }

    fn project_context_for(
        &self,
        expected_project_epoch: u64,
        expected_project_path: &str,
    ) -> Result<ChatProjectContext, String> {
        let project = self.project_context()?;
        if project.project_epoch != expected_project_epoch
            || project.project_dir.as_path() != std::path::Path::new(expected_project_path)
        {
            return Err("stale Agent chat project identity".to_string());
        }
        Ok(project)
    }

    /// Snapshot a session for a running turn. The map entry stays in place so
    /// `chat_history` can still return the last persisted state while the turn
    /// is in flight.
    fn take_project_session(
        &self,
        project: &ChatProjectContext,
        session_id: &str,
    ) -> Result<ChatSession, String> {
        let _identity = self.core.lock_project_identity_workflow();
        self.ensure_project_context(project)?;
        let key = project.key(session_id);
        if let Some(session) = self
            .sessions
            .lock()
            .map_err(|e| e.to_string())?
            .get(&key)
            .cloned()
        {
            return Ok(session);
        }
        Ok(project
            .store
            .load(session_id)
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| ChatSession::new(session_id.to_string())))
    }

    /// Atomically persist a session only while its original project identity
    /// still owns the core. Stale turn completion cannot publish into A or B.
    fn put_project_session(
        &self,
        project: &ChatProjectContext,
        session: ChatSession,
    ) -> Result<(), String> {
        let _identity = self.core.lock_project_identity_workflow();
        self.ensure_project_context(project)?;
        let _persistence = self.persistence.lock().map_err(|e| e.to_string())?;
        project.store.save(&session).map_err(|e| e.to_string())?;
        let key = project.key(&session.id);
        self.sessions
            .lock()
            .map_err(|e| e.to_string())?
            .insert(key, session);
        Ok(())
    }

    fn list_project_sessions(
        &self,
        project: &ChatProjectContext,
    ) -> Result<Vec<ChatSession>, String> {
        let _identity = self.core.lock_project_identity_workflow();
        self.ensure_project_context(project)?;
        let _persistence = self.persistence.lock().map_err(|e| e.to_string())?;
        project.store.list().map_err(|e| e.to_string())
    }

    fn reserve_turn(&self, key: SessionKey, cancel: Arc<AtomicBool>) -> Result<(), String> {
        let mut cancels = self.cancels.lock().map_err(|e| e.to_string())?;
        match cancels.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(cancel);
                Ok(())
            }
            Entry::Occupied(_) => Err("a turn is already running on this session".into()),
        }
    }
}

// MARK: - Event payloads (camelCase, mirror front-end types.ts)

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaPayload {
    project_epoch: u64,
    project_path: String,
    session_id: String,
    delta: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolCallPayload {
    project_epoch: u64,
    project_path: String,
    session_id: String,
    tool_call: opentake_agent::chat::ToolCall,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DonePayload {
    project_epoch: u64,
    project_path: String,
    session_id: String,
    message: ChatMessage,
}

/// Adapt `AppHandle::emit` to the loop's [`EmitLoop`] trait. Each loop event
/// becomes a Tauri event the front end listens for.
struct AppEmitter {
    app: AppHandle,
    state: ChatState,
    project: ChatProjectContext,
}

impl EmitLoop for AppEmitter {
    fn emit(&self, event: LoopEvent) {
        let _identity = self.state.core.lock_project_identity_workflow();
        if self.state.ensure_project_context(&self.project).is_err() {
            return;
        }
        match event {
            LoopEvent::Delta { session_id, delta } => {
                let _ = self.app.emit(
                    "chat_delta",
                    DeltaPayload {
                        project_epoch: self.project.project_epoch,
                        project_path: self.project.project_dir.to_string_lossy().into_owned(),
                        session_id,
                        delta,
                    },
                );
            }
            LoopEvent::ToolCall {
                session_id,
                tool_call,
            } => {
                let _ = self.app.emit(
                    "chat_tool_call",
                    ToolCallPayload {
                        project_epoch: self.project.project_epoch,
                        project_path: self.project.project_dir.to_string_lossy().into_owned(),
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
                        project_epoch: self.project.project_epoch,
                        project_path: self.project.project_dir.to_string_lossy().into_owned(),
                        session_id,
                        message,
                    },
                );
            }
        }
    }
}

/// Binds every Context Signal snapshot and complete tool dispatch to the
/// project accepted by `chat_send`. The read lease makes identity check + tool
/// side effect one atomic project-lifecycle boundary, including MediaBridge
/// calls that bypass `CoreHandle`.
struct ProjectTurnGate {
    state: ChatState,
    project: ChatProjectContext,
    cancel: Arc<AtomicBool>,
}

impl ProjectTurnGate {
    fn with_current_project<T>(&self, operation: impl FnOnce() -> T) -> Option<T> {
        let _identity = self.state.core.lock_project_identity_workflow();
        if self.state.ensure_project_context(&self.project).is_err() {
            self.cancel.store(true, Ordering::Relaxed);
            return None;
        }
        Some(operation())
    }
}

impl ChatTurnGate for ProjectTurnGate {
    fn timeline(&self, dispatcher: &Dispatcher) -> Option<opentake_domain::Timeline> {
        self.with_current_project(|| dispatcher.timeline())
    }

    fn dispatch(
        &self,
        dispatcher: &Dispatcher,
        name: &str,
        args: serde_json::Value,
    ) -> Option<ToolResult> {
        self.with_current_project(|| dispatcher.dispatch(name, args))
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
    expected_project_epoch: u64,
    expected_project_path: String,
) -> Result<(), String> {
    let project = state.project_context_for(expected_project_epoch, &expected_project_path)?;
    let session_key = project.key(&session_id);
    let cancel = Arc::new(AtomicBool::new(false));
    state.reserve_turn(session_key.clone(), cancel.clone())?;

    let mut session = match state.take_project_session(&project, &session_id) {
        Ok(session) => session,
        Err(error) => {
            state
                .cancels
                .lock()
                .ok()
                .and_then(|mut cancels| cancels.remove(&session_key));
            return Err(error);
        }
    };
    session.provider = Some(chat_provider.clone());
    session.messages.push(ChatMessage::user(text.clone()));
    if let Err(error) = state.put_project_session(&project, session.clone()) {
        state
            .cancels
            .lock()
            .ok()
            .and_then(|mut cancels| cancels.remove(&session_key));
        return Err(error);
    }
    let state_clone = state.inner().clone();
    let sid = session_id.clone();

    tauri::async_runtime::spawn(async move {
        let emitter = AppEmitter {
            app: app.clone(),
            state: state_clone.clone(),
            project: project.clone(),
        };
        let gate: Arc<dyn ChatTurnGate> = Arc::new(ProjectTurnGate {
            state: state_clone.clone(),
            project: project.clone(),
            cancel: cancel.clone(),
        });
        let result = state_clone
            .loop_
            .run_turn_gated(&mut session, chat_provider, text, &emitter, cancel, gate)
            .await;

        match &result {
            Err(LoopError::Cancelled) => {
                emitter.emit(LoopEvent::Done {
                    session_id: sid.clone(),
                    message: ChatMessage::assistant(String::new(), Vec::new()),
                });
            }
            Err(e) => {
                let msg = ChatMessage::assistant(format!("⚠️ {e}"), Vec::new());
                emitter.emit(LoopEvent::Done {
                    session_id: sid.clone(),
                    message: msg.clone(),
                });
                session.messages.push(msg);
            }
            Ok(()) => {}
        }

        if let Err(error) = state_clone.put_project_session(&project, session) {
            emitter.emit(LoopEvent::Done {
                session_id: sid.clone(),
                message: ChatMessage::assistant(
                    format!("⚠️ Chat history could not be saved: {error}"),
                    Vec::new(),
                ),
            });
        }
        state_clone
            .cancels
            .lock()
            .ok()
            .and_then(|mut c| c.remove(&session_key));
    });

    Ok(())
}

/// `chat_history`: return the current message log for a session. Empty when
/// the session doesn't exist yet.
#[tauri::command]
pub fn chat_history(
    state: State<'_, ChatState>,
    session_id: String,
    expected_project_epoch: u64,
    expected_project_path: String,
) -> Result<Vec<ChatMessage>, String> {
    let project = state.project_context_for(expected_project_epoch, &expected_project_path)?;
    Ok(state.take_project_session(&project, &session_id)?.messages)
}

/// `chat_sessions`: newest-first persistent conversations for the project.
#[tauri::command]
pub fn chat_sessions(
    state: State<'_, ChatState>,
    expected_project_epoch: u64,
    expected_project_path: String,
) -> Result<Vec<ChatSession>, String> {
    let project = state.project_context_for(expected_project_epoch, &expected_project_path)?;
    state.list_project_sessions(&project)
}

/// `chat_cancel`: request a running turn stop at the next boundary. No-op when
/// no turn is running.
#[tauri::command]
pub fn chat_cancel(
    state: State<'_, ChatState>,
    session_id: String,
    expected_project_epoch: u64,
    expected_project_path: String,
) -> Result<(), String> {
    let project = state.project_context_for(expected_project_epoch, &expected_project_path)?;
    let key = project.key(&session_id);
    let cancels = state.cancels.lock().map_err(|e| e.to_string())?;
    if let Some(flag) = cancels.get(&key) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_and_put_session_round_trips() {
        let temp = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        core.save_project(Some(temp.path().join("RoundTrip.opentake")))
            .unwrap();
        let state = ChatState::new(
            core,
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project = state.project_context().unwrap();
        state
            .put_project_session(&project, ChatSession::new("s1"))
            .unwrap();
        let taken = state.take_project_session(&project, "s1").unwrap();
        assert_eq!(taken.id, "s1");
        let second = state.take_project_session(&project, "s1").unwrap();
        assert_eq!(second.id, "s1");
        assert!(second.messages.is_empty());
    }

    #[test]
    fn history_snapshot_remains_visible_while_turn_owns_a_clone() {
        let temp = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        core.save_project(Some(temp.path().join("Snapshot.opentake")))
            .unwrap();
        let state = ChatState::new(
            core,
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project = state.project_context().unwrap();
        let mut session = ChatSession::new("s1");
        session.messages.push(ChatMessage::user("hello"));
        state
            .put_project_session(&project, session.clone())
            .unwrap();

        let running_copy = state.take_project_session(&project, "s1").unwrap();
        assert_eq!(running_copy.messages.len(), 1);

        let key = project.key("s1");
        let history = state
            .sessions
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .unwrap()
            .messages;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].content, "hello");
    }

    #[test]
    fn event_payloads_serialize_in_camel_case() {
        let payload = DeltaPayload {
            project_epoch: 7,
            project_path: "/tmp/A.opentake".into(),
            session_id: "sess-1".into(),
            delta: "hi".into(),
        };
        let json = serde_json::to_value(payload).unwrap();
        assert_eq!(json["projectEpoch"], 7);
        assert_eq!(json["projectPath"], "/tmp/A.opentake");
        assert_eq!(json["sessionId"], "sess-1");
        assert_eq!(json["delta"], "hi");

        let done = DonePayload {
            project_epoch: 7,
            project_path: "/tmp/A.opentake".into(),
            session_id: "sess-1".into(),
            message: ChatMessage::assistant("done", Vec::new()),
        };
        let json = serde_json::to_value(done).unwrap();
        assert_eq!(json["sessionId"], "sess-1");
        assert_eq!(json["message"]["role"], "assistant");
    }

    #[test]
    fn project_chat_session_reloads_from_the_bundle_after_state_recreation() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Persist.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let state = ChatState::new(
            core.clone(),
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project = state.project_context().unwrap();
        let mut session = ChatSession::new("chat-persisted");
        session.messages.push(ChatMessage::user("remember me"));
        state.put_project_session(&project, session).unwrap();
        drop(state);

        let reopened = ChatState::new(
            core,
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project = reopened.project_context().unwrap();
        let loaded = reopened
            .take_project_session(&project, "chat-persisted")
            .unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].content, "remember me");
        assert_eq!(reopened.list_project_sessions(&project).unwrap().len(), 1);
    }

    #[test]
    fn stale_project_turn_cannot_overwrite_the_previous_bundle() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("A.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle.clone())).unwrap();
        let state = ChatState::new(
            core.clone(),
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project_a = state.project_context().unwrap();
        let mut baseline = ChatSession::new("chat-stale");
        baseline.messages.push(ChatMessage::user("A baseline"));
        state
            .put_project_session(&project_a, baseline.clone())
            .unwrap();

        core.new_project();
        let bundle_b = temp.path().join("B.opentake");
        core.save_project(Some(bundle_b)).unwrap();
        baseline
            .messages
            .push(ChatMessage::assistant("stale result", vec![]));
        assert!(state.put_project_session(&project_a, baseline).is_err());

        let disk = opentake_agent::chat::ChatSessionStore::open(bundle)
            .unwrap()
            .load("chat-stale")
            .unwrap()
            .unwrap();
        assert_eq!(disk.messages.len(), 1);
        assert_eq!(disk.messages[0].content, "A baseline");
    }

    #[test]
    fn stale_project_turn_gate_cannot_dispatch_into_the_replacement_project() {
        let temp = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        core.save_project(Some(temp.path().join("A.opentake")))
            .unwrap();
        let state = ChatState::new(
            core.clone(),
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project_a = state.project_context().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let gate = ProjectTurnGate {
            state,
            project: project_a,
            cancel: cancel.clone(),
        };
        let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
        let registry = Arc::new(RwLock::new(crate::mcp::build_registry(
            &temp.path().join("no-workflows"),
        )));
        let dispatcher = Dispatcher::new(handle, registry);

        let current = gate
            .dispatch(
                &dispatcher,
                "create_folder",
                serde_json::json!({"name": "A folder"}),
            )
            .expect("the captured project is initially current");
        assert!(!current.is_error, "{}", current.text_joined());

        core.new_project();
        core.save_project(Some(temp.path().join("B.opentake")))
            .unwrap();
        assert!(gate
            .dispatch(
                &dispatcher,
                "create_folder",
                serde_json::json!({"name": "stale folder"}),
            )
            .is_none());
        assert!(cancel.load(Ordering::Relaxed));
        assert!(core.media().folders.is_empty());
    }

    #[test]
    fn reserving_a_turn_is_atomic_per_project_session() {
        let temp = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        let state = ChatState::new(
            core,
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let key = SessionKey {
            project_epoch: 7,
            project_dir: temp.path().join("A.opentake"),
            session_id: "chat-one".into(),
        };

        state
            .reserve_turn(key.clone(), Arc::new(AtomicBool::new(false)))
            .unwrap();
        let error = state
            .reserve_turn(key, Arc::new(AtomicBool::new(false)))
            .expect_err("a second turn must not replace the first cancel token");

        assert!(error.contains("already running"));
        assert_eq!(state.cancels.lock().unwrap().len(), 1);
    }

    #[test]
    fn save_as_cancels_and_purges_the_previous_project_turn() {
        let temp = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        let source = temp.path().join("A.opentake");
        let target = temp.path().join("B.opentake");
        core.save_project(Some(source)).unwrap();
        let state = ChatState::new(
            core.clone(),
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project_a = state.project_context().unwrap();
        state
            .put_project_session(&project_a, ChatSession::new("chat-same"))
            .unwrap();
        let old_cancel = Arc::new(AtomicBool::new(false));
        state
            .reserve_turn(project_a.key("chat-same"), old_cancel.clone())
            .unwrap();

        core.save_project(Some(target)).unwrap();

        assert!(old_cancel.load(Ordering::Relaxed));
        assert!(state.cancels.lock().unwrap().is_empty());
        assert!(state.sessions.lock().unwrap().is_empty());
        let project_b = state.project_context().unwrap();
        state
            .reserve_turn(project_b.key("chat-same"), Arc::new(AtomicBool::new(false)))
            .expect("same session id in the Save As target is independent");
    }
}
