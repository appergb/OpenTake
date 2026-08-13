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
    ChatLoop, ChatMessage, ChatSession, ChatSessionStore, ChatTurnGate, EmitLoop, LlmError,
    LoopError, LoopEvent, Role,
};
use opentake_agent::mcp::advanced::AdvancedWorkflowBridge;
use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::mcp::generation::GenerationBridge;
use opentake_agent::mcp::motion::MotionBridge;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_agent::tools::result::ToolResult;
use opentake_gen::{KeyStore, KeyringStore};

use opentake_core::AppCore;

/// Managed state: one [`ChatLoop`] over the shared core + plugin registry, a
/// map of live sessions, and a map of cancel flags for in-flight turns.
#[derive(Clone)]
pub struct ChatState {
    core: AppCore,
    dispatcher: Arc<Dispatcher>,
    registry: Arc<RwLock<PluginRegistry>>,
    loop_: ChatLoop,
    sessions: Arc<Mutex<HashMap<SessionKey, ChatSession>>>,
    turns: Arc<Mutex<TurnRegistry>>,
    persistence: Arc<Mutex<()>>,
    admission: crate::updater::InstallAdmissionGate,
}

/// Immutable handles for long-lived MCP sessions to enter the exact same
/// dispatcher and workflow registry used by in-app Agent chat.
#[derive(Clone)]
#[allow(dead_code)] // Task 4's listener consumes both handles.
pub(crate) struct ExternalMcpComponents {
    pub(crate) dispatcher: Arc<Dispatcher>,
    pub(crate) registry: Arc<RwLock<PluginRegistry>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SessionKey {
    project_epoch: u64,
    project_dir: PathBuf,
    session_id: String,
}

fn agent_undo_scope(key: &SessionKey) -> String {
    format!(
        "opentake:chat:{}:{}:{}",
        key.project_epoch,
        key.project_dir.to_string_lossy(),
        key.session_id
    )
}

struct TurnCancel {
    requested: Arc<AtomicBool>,
    media: opentake_media::MediaCancelToken,
    phase: Mutex<TurnPhase>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TurnPhase {
    Running,
    Cancelled,
    Finalizing,
}

impl TurnCancel {
    fn new() -> Self {
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            media: opentake_media::MediaCancelToken::new(),
            phase: Mutex::new(TurnPhase::Running),
        }
    }

    fn request(&self) {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *phase != TurnPhase::Running {
            return;
        }
        *phase = TurnPhase::Cancelled;
        self.requested.store(true, Ordering::Release);
        self.media.cancel();
    }

    fn begin_finalization(&self) -> bool {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *phase != TurnPhase::Running {
            return false;
        }
        *phase = TurnPhase::Finalizing;
        true
    }
}

#[derive(Default)]
struct TurnRegistry {
    running: HashMap<SessionKey, Arc<TurnCancel>>,
    transition_depth: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TurnFinalization {
    Committed,
    Cancelled,
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
    pub(crate) fn external_mcp_components(&self) -> ExternalMcpComponents {
        ExternalMcpComponents {
            dispatcher: self.dispatcher.clone(),
            registry: self.registry.clone(),
        }
    }

    fn project_turn_gate(
        &self,
        project: &ChatProjectContext,
        session_key: &SessionKey,
        cancel: Arc<TurnCancel>,
    ) -> Arc<dyn ChatTurnGate> {
        Arc::new(ProjectTurnGate {
            state: self.clone(),
            project: project.clone(),
            cancel,
            undo_scope: agent_undo_scope(session_key),
        })
    }

    #[cfg(test)]
    pub(crate) fn project_turn_gate_for_test(&self, session_id: &str) -> Arc<dyn ChatTurnGate> {
        let project = self.project_context().expect("saved test project");
        self.put_project_session(&project, ChatSession::new(session_id))
            .expect("persist test chat session");
        let session_key = project.key(session_id);
        self.project_turn_gate(&project, &session_key, Arc::new(TurnCancel::new()))
    }

    #[cfg(test)]
    pub fn new(
        core: AppCore,
        workflows_dir: PathBuf,
        cache_root: PathBuf,
        models_dir: PathBuf,
    ) -> Self {
        Self::new_inner(
            core,
            workflows_dir,
            cache_root,
            models_dir,
            None,
            None,
            None,
            crate::updater::InstallAdmissionGate::default(),
        )
    }

    #[cfg(test)]
    fn new_with_admission(
        core: AppCore,
        workflows_dir: PathBuf,
        cache_root: PathBuf,
        models_dir: PathBuf,
        admission: crate::updater::InstallAdmissionGate,
    ) -> Self {
        Self::new_inner(
            core,
            workflows_dir,
            cache_root,
            models_dir,
            None,
            None,
            None,
            admission,
        )
    }

    /// Build the state in `setup`: a dispatcher over the live core + workflow
    /// registry + the same media bridge the desktop MCP server uses.
    #[allow(clippy::too_many_arguments)] // explicit dependency-injection boundary
    pub fn new_with_capabilities(
        core: AppCore,
        workflows_dir: PathBuf,
        cache_root: PathBuf,
        models_dir: PathBuf,
        generation_bridge: Arc<dyn GenerationBridge>,
        motion_bridge: Arc<dyn MotionBridge>,
        advanced_bridge: Arc<dyn AdvancedWorkflowBridge>,
        admission: crate::updater::InstallAdmissionGate,
    ) -> Self {
        Self::new_inner(
            core,
            workflows_dir,
            cache_root,
            models_dir,
            Some(generation_bridge),
            Some(motion_bridge),
            Some(advanced_bridge),
            admission,
        )
    }

    #[allow(clippy::too_many_arguments)] // keeps optional production bridges explicit in tests
    fn new_inner(
        core: AppCore,
        workflows_dir: PathBuf,
        cache_root: PathBuf,
        models_dir: PathBuf,
        generation_bridge: Option<Arc<dyn GenerationBridge>>,
        motion_bridge: Option<Arc<dyn MotionBridge>>,
        advanced_bridge: Option<Arc<dyn AdvancedWorkflowBridge>>,
        admission: crate::updater::InstallAdmissionGate,
    ) -> Self {
        let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
        let registry = Arc::new(RwLock::new(crate::mcp::build_registry(&workflows_dir)));
        let bridge = crate::mcp::build_media_bridge(core.clone(), cache_root, models_dir);
        let dispatcher = Arc::new(Dispatcher::with_all_capability_bridges(
            handle,
            registry.clone(),
            Some(bridge),
            generation_bridge,
            motion_bridge,
            advanced_bridge,
        ));
        let store: Arc<dyn KeyStore> = Arc::new(KeyringStore::new());
        let sessions = Arc::new(Mutex::new(HashMap::new()));
        let turns = Arc::new(Mutex::new(TurnRegistry::default()));
        let state = ChatState {
            core,
            dispatcher: dispatcher.clone(),
            registry: registry.clone(),
            loop_: ChatLoop::new(dispatcher, registry, store),
            sessions: sessions.clone(),
            turns: turns.clone(),
            persistence: Arc::new(Mutex::new(())),
            admission,
        };
        let transition_turns = turns.clone();
        state
            .core
            .subscribe_project_identity_transition(move |pending| {
                if let Ok(mut turns) = transition_turns.lock() {
                    if pending {
                        turns.transition_depth = turns.transition_depth.saturating_add(1);
                        for cancel in turns.running.values() {
                            cancel.request();
                        }
                    } else {
                        turns.transition_depth = turns.transition_depth.saturating_sub(1);
                    }
                }
            });
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
            if let Ok(mut turns) = turns.lock() {
                turns.running.retain(|key, cancel| {
                    let current = key.project_epoch == project_epoch
                        && project_dir.as_ref() == Some(&key.project_dir);
                    if !current {
                        cancel.request();
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

    fn take_open_project_session_for_turn(
        &self,
        project: &ChatProjectContext,
        session_id: &str,
    ) -> Result<ChatSession, String> {
        let session = self.take_project_session(project, session_id)?;
        if session.is_open {
            Ok(session)
        } else {
            Err("the Agent chat tab is closed".into())
        }
    }

    /// Atomically persist a session only while its original project identity
    /// still owns the core. Stale turn completion cannot publish into A or B.
    fn put_project_session(
        &self,
        project: &ChatProjectContext,
        session: ChatSession,
    ) -> Result<(), String> {
        let _identity = self.core.lock_project_identity_workflow();
        self.put_project_session_with_identity_held(project, session)
    }

    fn put_project_session_with_identity_held(
        &self,
        project: &ChatProjectContext,
        session: ChatSession,
    ) -> Result<(), String> {
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

    /// Linearize whole-turn cancellation against the durable Codex terminal
    /// commit. Holding the turn registry excludes `chat_cancel` and project
    /// transition hooks; `TurnCancel::begin_finalization` covers cancellation
    /// sources that hold the token directly. The identity read lease then keeps
    /// the accepted project stable through the session write.
    fn finalize_project_turn(
        &self,
        project: &ChatProjectContext,
        key: &SessionKey,
        owner: &Arc<TurnCancel>,
        session: ChatSession,
    ) -> Result<TurnFinalization, String> {
        let mut turns = self.turns.lock().map_err(|e| e.to_string())?;
        let owns_turn = turns
            .running
            .get(key)
            .is_some_and(|registered| Arc::ptr_eq(registered, owner));
        if !owns_turn || turns.transition_depth > 0 || !owner.begin_finalization() {
            if owns_turn {
                turns.running.remove(key);
            }
            return Ok(TurnFinalization::Cancelled);
        }

        let _identity = self.core.lock_project_identity_workflow();
        let persisted = self.put_project_session_with_identity_held(project, session);
        turns.running.remove(key);
        persisted?;
        Ok(TurnFinalization::Committed)
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

    fn set_project_session_open(
        &self,
        project: &ChatProjectContext,
        session_id: &str,
        is_open: bool,
    ) -> Result<ChatSession, String> {
        let key = project.key(session_id);
        let turns = self.turns.lock().map_err(|e| e.to_string())?;
        if turns.running.contains_key(&key) {
            return Err("finish or cancel the running turn before changing its tab state".into());
        }
        let mut session = self.take_project_session(project, session_id)?;
        session.is_open = is_open;
        self.put_project_session(project, session.clone())?;
        drop(turns);
        Ok(session)
    }

    fn reserve_turn(
        &self,
        key: SessionKey,
        cancel: Arc<TurnCancel>,
    ) -> Result<crate::updater::ActivityLease, String> {
        let admission = self.admission.begin_activity()?;
        let mut turns = self.turns.lock().map_err(|e| e.to_string())?;
        if turns.transition_depth > 0 {
            cancel.request();
            return Err("project transition is in progress".into());
        }
        match turns.running.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(cancel);
                Ok(admission)
            }
            Entry::Occupied(_) => Err("a turn is already running on this session".into()),
        }
    }

    fn release_turn(&self, key: &SessionKey) {
        if let Ok(mut turns) = self.turns.lock() {
            turns.running.remove(key);
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
    cancel: Arc<TurnCancel>,
    undo_scope: String,
}

impl ProjectTurnGate {
    fn with_current_project<T>(&self, operation: impl FnOnce() -> T) -> Option<T> {
        let _identity = self.state.core.lock_project_identity_workflow();
        if self.cancel.requested.load(Ordering::Acquire)
            || self.state.ensure_project_context(&self.project).is_err()
        {
            self.cancel.request();
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
        self.with_current_project(|| {
            dispatcher.dispatch_cancellable_scoped(&self.undo_scope, name, args, &self.cancel.media)
        })
    }

    fn request_cancel(&self) {
        self.cancel.request();
    }

    fn request_dispatch_cancel(&self) {
        self.cancel.media.cancel();
    }
}

fn truncate_for_codex_prompt(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn codex_turn_prompt(session: &ChatSession, user_text: &str) -> String {
    let mut history = session
        .messages
        .iter()
        .rev()
        .skip(1)
        .filter_map(|message| match message.role {
            Role::User => Some(format!(
                "User: {}",
                truncate_for_codex_prompt(&message.content, 2_000)
            )),
            Role::Assistant if !message.content.trim().is_empty() => Some(format!(
                "Assistant: {}",
                truncate_for_codex_prompt(&message.content, 2_000)
            )),
            _ => None,
        })
        .take(6)
        .collect::<Vec<_>>();
    history.reverse();
    let history = history.join("\n");
    format!(
        "You are the official Codex Agent embedded in OpenTake, a desktop video editor. \
Use only the `opentake` MCP server for reading or changing the current project. \
Do not use shell commands, direct file edits, web search, plugins, or other MCP servers. \
Treat the current saved OpenTake project as the sole editing target. Explain the completed result concisely.\n\n\
Recent conversation:\n{history}\n\nCurrent user request:\n{}",
        truncate_for_codex_prompt(user_text, 20_000)
    )
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
    let turn_cancel = Arc::new(TurnCancel::new());
    let turn_admission = state.reserve_turn(session_key.clone(), turn_cancel.clone())?;
    let cancel = turn_cancel.requested.clone();

    let mut session = match state.take_open_project_session_for_turn(&project, &session_id) {
        Ok(session) => session,
        Err(error) => {
            state.release_turn(&session_key);
            return Err(error);
        }
    };
    session.provider = Some(chat_provider.clone());
    session.messages.push(ChatMessage::user(text.clone()));
    if let Err(error) = state.put_project_session(&project, session.clone()) {
        state.release_turn(&session_key);
        return Err(error);
    }
    let state_clone = state.inner().clone();
    let sid = session_id.clone();

    tauri::async_runtime::spawn(async move {
        let _turn_admission = turn_admission;
        let emitter = AppEmitter {
            app: app.clone(),
            state: state_clone.clone(),
            project: project.clone(),
        };
        let turn_owner = turn_cancel.clone();
        let gate = state_clone.project_turn_gate(&project, &session_key, turn_cancel);
        let is_codex = chat_provider == "codex";
        let mut codex_final: Option<(String, ChatMessage)> = None;
        let result = if is_codex {
            session.provider = Some("codex".into());
            session.model = Some("official-codex-default".into());
            let prompt = codex_turn_prompt(&session, &text);
            let context = crate::codex::CodexTurnContext {
                dispatcher: state_clone.dispatcher.clone(),
                registry: state_clone.registry.clone(),
                gate: gate.clone(),
                cancel: cancel.clone(),
            };
            match crate::codex::run_agent_turn(context, &prompt, |tool_call| {
                emitter.emit(LoopEvent::ToolCall {
                    session_id: sid.clone(),
                    tool_call,
                });
            })
            .await
            {
                Ok(output) => {
                    let message = ChatMessage::assistant(output.text.clone(), output.tool_calls);
                    session.messages.push(message.clone());
                    codex_final = Some((output.text, message));
                    Ok(())
                }
                Err(crate::codex::CodexTurnError::Cancelled) => Err(LoopError::Cancelled),
                Err(crate::codex::CodexTurnError::Unavailable) => {
                    let guide = "Official Codex CLI was not found. Install Codex, then return to Settings → AI and choose Official Codex / ChatGPT.".to_string();
                    let message = ChatMessage::assistant(guide.clone(), Vec::new());
                    session.messages.push(message.clone());
                    codex_final = Some((guide, message));
                    Ok(())
                }
                Err(crate::codex::CodexTurnError::NotAuthenticated) => {
                    let guide = "Codex is not signed in. Open Settings → AI, choose Official Codex / ChatGPT, and sign in with ChatGPT.".to_string();
                    let message = ChatMessage::assistant(guide.clone(), Vec::new());
                    session.messages.push(message.clone());
                    codex_final = Some((guide, message));
                    Ok(())
                }
                Err(crate::codex::CodexTurnError::IncompatibleCli)
                | Err(crate::codex::CodexTurnError::StrictConfigRejected) => {
                    let guide = "The installed official Codex CLI is not compatible with this OpenTake Beta. Update Codex CLI to version 0.146.0 or newer, then try again.".to_string();
                    let message = ChatMessage::assistant(guide.clone(), Vec::new());
                    session.messages.push(message.clone());
                    codex_final = Some((guide, message));
                    Ok(())
                }
                Err(crate::codex::CodexTurnError::McpStart)
                | Err(crate::codex::CodexTurnError::Timeout)
                | Err(crate::codex::CodexTurnError::Protocol)
                | Err(crate::codex::CodexTurnError::ProviderFailed) => {
                    Err(LoopError::Llm(LlmError::Provider(
                        "official Codex turn failed; check the Codex login status and try again"
                            .into(),
                    )))
                }
            }
        } else {
            state_clone
                .loop_
                .run_turn_gated(&mut session, chat_provider, text, &emitter, cancel, gate)
                .await
        };

        if is_codex {
            let terminal = match result {
                Ok(()) => codex_final,
                Err(LoopError::Cancelled) => Some((
                    String::new(),
                    ChatMessage::assistant(String::new(), Vec::new()),
                )),
                Err(error) => {
                    let message = ChatMessage::assistant(format!("⚠️ {error}"), Vec::new());
                    session.messages.push(message.clone());
                    Some((String::new(), message))
                }
            };
            match state_clone.finalize_project_turn(
                &project,
                &session_key,
                &turn_owner,
                session.clone(),
            ) {
                Ok(TurnFinalization::Committed) => {
                    if let Some((delta, message)) = terminal {
                        if !delta.is_empty() {
                            emitter.emit(LoopEvent::Delta {
                                session_id: sid.clone(),
                                delta,
                            });
                        }
                        emitter.emit(LoopEvent::Done {
                            session_id: sid.clone(),
                            message,
                        });
                    }
                }
                Ok(TurnFinalization::Cancelled) => {
                    emitter.emit(LoopEvent::Done {
                        session_id: sid.clone(),
                        message: ChatMessage::assistant(String::new(), Vec::new()),
                    });
                }
                Err(error) => {
                    emitter.emit(LoopEvent::Done {
                        session_id: sid.clone(),
                        message: ChatMessage::assistant(
                            format!("⚠️ Chat history could not be saved: {error}"),
                            Vec::new(),
                        ),
                    });
                }
            }
            return;
        }

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
        state_clone.release_turn(&session_key);
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

/// Persist whether a conversation is represented by an open Agent tab. Opening
/// a new id creates an empty project-local session immediately, so tabs survive
/// restart even before their first message.
#[tauri::command]
pub fn chat_session_set_open(
    state: State<'_, ChatState>,
    session_id: String,
    is_open: bool,
    expected_project_epoch: u64,
    expected_project_path: String,
) -> Result<ChatSession, String> {
    let _activity = crate::updater::begin_mutating_activity(&state.admission)?;
    let project = state.project_context_for(expected_project_epoch, &expected_project_path)?;
    state.set_project_session_open(&project, &session_id, is_open)
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
    let key = SessionKey {
        project_epoch: expected_project_epoch,
        project_dir: PathBuf::from(expected_project_path),
        session_id,
    };
    let turns = state.turns.lock().map_err(|e| e.to_string())?;
    if let Some(flag) = turns.running.get(&key) {
        flag.request();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RedactionMediaBridge;

    impl opentake_agent::mcp::media_bridge::MediaBridge for RedactionMediaBridge {
        fn inspect_media(
            &self,
            request: &opentake_agent::mcp::media_bridge::InspectMediaRequest,
        ) -> Result<
            opentake_agent::mcp::media_bridge::InspectMediaResult,
            opentake_agent::mcp::media_bridge::BridgeError,
        > {
            Ok(opentake_agent::mcp::media_bridge::InspectMediaResult {
                frames: vec![opentake_agent::mcp::media_bridge::InspectedMediaFrame {
                    timestamp_seconds: 0.0,
                    bytes: vec![0xff, 0xd8, 0xff, 0xe0],
                    media_type: "image/jpeg".into(),
                }],
                overview_timestamps: Vec::new(),
                duration_seconds: 3.0,
                width: Some(1920),
                height: Some(1080),
                fps: Some(30.0),
                has_audio: request.kind == opentake_domain::ClipType::Video,
                byte_size: 4,
                transcript: None,
                transcription_unavailable: false,
            })
        }
    }

    struct RedactionAdvancedBridge;

    const MOTION_PRIVATE_RENDERER: &str = "PRIVATE_CHAT_MOTION_RENDERER";
    const MOTION_PRIVATE_VERSION: &str = "PRIVATE_CHAT_MOTION_RENDERER_VERSION";
    const MOTION_PRIVATE_OUTPUT: &str = "/Users/private/chat-motion-output.mp4";
    const MOTION_PRIVATE_HASH: &str = "PRIVATE_CHAT_MOTION_CONTENT_HASH";
    const MOTION_PRIVATE_ACTION: &str = "PRIVATE_CHAT_MOTION_ACTION";

    impl opentake_agent::mcp::advanced::AdvancedWorkflowBridge for RedactionAdvancedBridge {
        fn supported_tools(&self) -> Vec<opentake_agent::tools::names::ToolName> {
            vec![opentake_agent::tools::names::ToolName::GenerateAvatar]
        }

        fn execute(
            &self,
            _request: opentake_agent::mcp::advanced::AdvancedWorkflowRequest,
            _cancel: &opentake_media::MediaCancelToken,
        ) -> Result<
            opentake_agent::mcp::advanced::AdvancedWorkflowCommit,
            opentake_agent::mcp::advanced::AdvancedWorkflowError,
        > {
            Ok(opentake_agent::mcp::advanced::AdvancedWorkflowCommit {
                result: serde_json::json!({
                    "status": "completed",
                    "assetId": "avatar-safe-asset",
                    "imported": true,
                    "previewPath": "/Users/private/avatar-preview.mov",
                    "signedUrl": "https://provider.invalid/avatar?token=SIGNED_AVATAR_SECRET",
                    "providerRequestId": "PRIVATE_PROVIDER_REQUEST_ID",
                    "prompt": "PRIVATE_ADVANCED_PROMPT",
                    "errors": [{"message": "provider failed with sk-private-avatar-key"}],
                }),
                action_name: None,
            })
        }
    }

    struct RedactionMotionBridge;

    fn redaction_motion_commit(
        clip_id: String,
        asset_id: &str,
    ) -> opentake_agent::mcp::motion::MotionCommit {
        opentake_agent::mcp::motion::MotionCommit {
            clip_id,
            asset_id: asset_id.into(),
            content_hash: MOTION_PRIVATE_HASH.into(),
            action_name: MOTION_PRIVATE_ACTION.into(),
            output: opentake_agent::mcp::motion::MotionOutputMetadata {
                renderer: MOTION_PRIVATE_RENDERER.into(),
                renderer_version: MOTION_PRIVATE_VERSION.into(),
                output_file: MOTION_PRIVATE_OUTPUT.into(),
                fps: 30.0,
                width: 1920,
                height: 1080,
                duration_frames: 90,
                duration_seconds: 3.0,
                content_hash: MOTION_PRIVATE_HASH.into(),
            },
        }
    }

    impl opentake_agent::mcp::motion::MotionBridge for RedactionMotionBridge {
        fn can_render_motion(&self) -> bool {
            true
        }

        fn add(
            &self,
            _request: opentake_agent::mcp::motion::AddMotionRequest,
            _cancel: &opentake_media::MediaCancelToken,
        ) -> Result<
            opentake_agent::mcp::motion::MotionCommit,
            opentake_agent::mcp::motion::MotionBridgeError,
        > {
            Ok(redaction_motion_commit(
                "motion-safe-add-clip".into(),
                "motion-safe-add-asset",
            ))
        }

        fn edit(
            &self,
            request: opentake_agent::mcp::motion::EditMotionRequest,
            _cancel: &opentake_media::MediaCancelToken,
        ) -> Result<
            opentake_agent::mcp::motion::MotionCommit,
            opentake_agent::mcp::motion::MotionBridgeError,
        > {
            Ok(redaction_motion_commit(
                request.clip_id,
                "motion-safe-edit-asset",
            ))
        }
    }

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
    fn codex_prompt_keeps_recent_context_and_limits_unbounded_history() {
        let mut session = ChatSession::new("codex-prompt");
        session.messages.push(ChatMessage::user("earlier request"));
        session
            .messages
            .push(ChatMessage::assistant("earlier result", Vec::new()));
        session.messages.push(ChatMessage::user("x".repeat(30_000)));

        let prompt = codex_turn_prompt(&session, &"当前请求".repeat(10_000));
        assert!(prompt.contains("User: earlier request"));
        assert!(prompt.contains("Assistant: earlier result"));
        assert!(prompt.contains("Current user request:"));
        assert!(prompt.chars().count() < 21_000);
        assert!(prompt.ends_with('…'));
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
    fn sensitive_media_and_advanced_results_never_reach_persisted_chat_history() {
        use opentake_agent::mcp::convert::safe_tool_result_for_llm;
        use opentake_domain::{
            ClipType, GenerationInput, GenerationJobStatus, MediaManifestEntry, MediaSource,
        };

        const ABSOLUTE_PATH: &str = "/Users/private/launch-cut.mov";
        const SIGNED_RESULT_URL: &str =
            "https://cdn.example.invalid/result.mov?token=SIGNED_RESULT_SECRET";
        const SIGNED_REFERENCE_URL: &str =
            "https://cdn.example.invalid/reference.png?token=SIGNED_REFERENCE_SECRET";
        const SIGNED_IMAGE_URL: &str =
            "https://cdn.example.invalid/input.png?token=SIGNED_IMAGE_SECRET";
        const PRIVATE_PROMPT: &str = "PRIVATE_PROVIDER_PROMPT_SECRET";
        const ADVANCED_PATH: &str = "/Users/private/avatar-preview.mov";
        const ADVANCED_URL_SECRET: &str = "SIGNED_AVATAR_SECRET";
        const ADVANCED_PROVIDER_ID: &str = "PRIVATE_PROVIDER_REQUEST_ID";
        const ADVANCED_PROMPT: &str = "PRIVATE_ADVANCED_PROMPT";
        const ADVANCED_KEY: &str = "sk-private-avatar-key";

        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Redaction.opentake");
        let mut project = opentake_project::Project::new(bundle.clone());
        project.manifest.entries.push(MediaManifestEntry {
            id: "generated-asset".into(),
            name: "Generated clip".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: ABSOLUTE_PATH.into(),
            },
            duration: 3.0,
            generation_input: Some(GenerationInput {
                prompt: PRIVATE_PROMPT.into(),
                model: "provider-model".into(),
                duration: 3,
                aspect_ratio: "16:9".into(),
                image_urls: Some(vec![SIGNED_IMAGE_URL.into()]),
                reference_image_urls: Some(vec![SIGNED_REFERENCE_URL.into()]),
                status: Some(GenerationJobStatus::Ready),
                ..GenerationInput::default()
            }),
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: Some(SIGNED_RESULT_URL.into()),
            cached_remote_url_expires_at: Some(123_456.0),
        });
        project.save().unwrap();

        let core = AppCore::new();
        core.open_project(bundle.clone()).unwrap();
        let state = ChatState::new(
            core,
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project = state.project_context().unwrap();
        let result = state
            .dispatcher
            .dispatch("get_media", serde_json::json!({}));
        assert!(!result.is_error, "{}", result.text_joined());

        let inspect_dispatcher = Dispatcher::with_bridge(
            Arc::new(AppCoreHandle::new(state.core.clone())),
            Arc::new(RwLock::new(PluginRegistry::new())),
            Some(Arc::new(RedactionMediaBridge)),
        );
        let inspect_result = inspect_dispatcher.dispatch(
            "inspect_media",
            serde_json::json!({"mediaRef": "generated-asset"}),
        );
        assert!(!inspect_result.is_error, "{}", inspect_result.text_joined());

        let advanced_dispatcher = Dispatcher::with_all_capability_bridges(
            Arc::new(AppCoreHandle::new(state.core.clone())),
            Arc::new(RwLock::new(PluginRegistry::new())),
            None,
            None,
            None,
            Some(Arc::new(RedactionAdvancedBridge)),
        );
        let advanced_result = advanced_dispatcher.dispatch(
            "generate_avatar",
            serde_json::json!({
                "portraitMediaRef": "generated-asset",
                "audioMediaRef": "generated-asset",
                "consentId": "consent-safe",
                "costAuthorized": true
            }),
        );
        assert!(
            !advanced_result.is_error,
            "{}",
            advanced_result.text_joined()
        );

        let motion_dispatcher = Dispatcher::with_capability_bridges(
            Arc::new(AppCoreHandle::new(state.core.clone())),
            Arc::new(RwLock::new(PluginRegistry::new())),
            None,
            None,
            Some(Arc::new(RedactionMotionBridge)),
        );
        let add_motion_result = motion_dispatcher.dispatch(
            "add_motion_graphic",
            serde_json::json!({
                "source": {"code": "export default {}"},
                "startFrame": 0,
                "durationFrames": 90
            }),
        );
        assert!(
            !add_motion_result.is_error,
            "{}",
            add_motion_result.text_joined()
        );
        let edit_motion_result = motion_dispatcher.dispatch(
            "edit_motion_graphic",
            serde_json::json!({
                "clipId": "motion-safe-edit-clip",
                "code": "export default {}"
            }),
        );
        assert!(
            !edit_motion_result.is_error,
            "{}",
            edit_motion_result.text_joined()
        );

        let mut session = ChatSession::new("get-media-redaction");
        session.messages.push(ChatMessage::tool_result_blocks(
            "call-get-media",
            result.content.clone(),
            safe_tool_result_for_llm(&result),
            false,
        ));
        session.messages.push(ChatMessage::tool_result_blocks(
            "call-inspect-media",
            inspect_result.content.clone(),
            safe_tool_result_for_llm(&inspect_result),
            false,
        ));
        session.messages.push(ChatMessage::tool_result_blocks(
            "call-generate-avatar",
            advanced_result.content.clone(),
            safe_tool_result_for_llm(&advanced_result),
            false,
        ));
        session.messages.push(ChatMessage::tool_result_blocks(
            "call-add-motion",
            add_motion_result.content.clone(),
            safe_tool_result_for_llm(&add_motion_result),
            false,
        ));
        session.messages.push(ChatMessage::tool_result_blocks(
            "call-edit-motion",
            edit_motion_result.content.clone(),
            safe_tool_result_for_llm(&edit_motion_result),
            false,
        ));
        state.put_project_session(&project, session).unwrap();

        let persisted =
            std::fs::read_to_string(bundle.join("chat-sessions/get-media-redaction.json")).unwrap();
        for secret in [
            ABSOLUTE_PATH,
            SIGNED_RESULT_URL,
            SIGNED_REFERENCE_URL,
            SIGNED_IMAGE_URL,
            PRIVATE_PROMPT,
            ADVANCED_PATH,
            ADVANCED_URL_SECRET,
            ADVANCED_PROVIDER_ID,
            ADVANCED_PROMPT,
            ADVANCED_KEY,
            MOTION_PRIVATE_RENDERER,
            MOTION_PRIVATE_VERSION,
            MOTION_PRIVATE_OUTPUT,
            MOTION_PRIVATE_HASH,
            MOTION_PRIVATE_ACTION,
        ] {
            assert!(
                !persisted.contains(secret),
                "persisted chat history leaked {secret}: {persisted}"
            );
        }
        assert!(persisted.contains("generated-asset"));
        assert!(persisted.contains("Generated clip"));
        assert!(persisted.contains("avatar-safe-asset"));
        assert!(persisted.contains("motion-safe-add-clip"));
        assert!(persisted.contains("motion-safe-add-asset"));
        assert!(persisted.contains("motion-safe-edit-clip"));
        assert!(persisted.contains("motion-safe-edit-asset"));
    }

    #[test]
    fn session_open_state_persists_across_state_recreation() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Tabs.opentake");
        let core = AppCore::new();
        core.save_project(Some(bundle)).unwrap();
        let state = ChatState::new(
            core.clone(),
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project = state.project_context().unwrap();

        let opened = state
            .set_project_session_open(&project, "chat-tab", true)
            .unwrap();
        assert!(opened.is_open);
        let closed = state
            .set_project_session_open(&project, "chat-tab", false)
            .unwrap();
        assert!(!closed.is_open);
        assert!(state
            .take_open_project_session_for_turn(&project, "chat-tab")
            .is_err());

        let recreated = ChatState::new(
            core,
            temp.path().join("no-workflows-2"),
            temp.path().join("chat-cache-2"),
            temp.path().join("chat-models-2"),
        );
        let recreated_project = recreated.project_context().unwrap();
        let sessions = recreated.list_project_sessions(&recreated_project).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "chat-tab");
        assert!(!sessions[0].is_open);
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
        let cancel = Arc::new(TurnCancel::new());
        let gate = ProjectTurnGate {
            state,
            project: project_a,
            cancel: cancel.clone(),
            undo_scope: "test:stale-project".into(),
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
        assert!(cancel.requested.load(Ordering::Relaxed));
        assert!(cancel.media.is_cancelled());
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
            .reserve_turn(key.clone(), Arc::new(TurnCancel::new()))
            .unwrap();
        let error = state
            .reserve_turn(key, Arc::new(TurnCancel::new()))
            .expect_err("a second turn must not replace the first cancel token");

        assert!(error.contains("already running"));
        assert_eq!(state.turns.lock().unwrap().running.len(), 1);
    }

    #[test]
    fn chat_turn_admission_spans_reservation_and_rejects_turns_during_install() {
        let temp = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        let admission = crate::updater::InstallAdmissionGate::default();
        let state = ChatState::new_with_admission(
            core,
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
            admission.clone(),
        );
        let key = SessionKey {
            project_epoch: 7,
            project_dir: temp.path().join("A.opentake"),
            session_id: "chat-admission".into(),
        };

        let turn_admission = state
            .reserve_turn(key.clone(), Arc::new(TurnCancel::new()))
            .unwrap();
        assert!(admission.begin_install().is_err());
        state.release_turn(&key);
        drop(turn_admission);

        let install = admission.begin_install().unwrap();
        assert!(state
            .reserve_turn(key.clone(), Arc::new(TurnCancel::new()))
            .is_err());
        drop(install);
        let resumed = state
            .reserve_turn(key.clone(), Arc::new(TurnCancel::new()))
            .unwrap();
        state.release_turn(&key);
        drop(resumed);
    }

    #[test]
    fn project_turn_gate_request_cancel_cancels_the_whole_turn() {
        let temp = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        core.save_project(Some(temp.path().join("Project.opentake")))
            .unwrap();
        let state = ChatState::new(
            core,
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let cancel = Arc::new(TurnCancel::new());
        let gate = ProjectTurnGate {
            project: state.project_context().unwrap(),
            state,
            cancel: cancel.clone(),
            undo_scope: "test:cancel".into(),
        };

        gate.request_dispatch_cancel();
        assert!(!cancel.requested.load(Ordering::Acquire));
        assert!(cancel.media.is_cancelled());

        gate.request_cancel();

        assert!(cancel.requested.load(Ordering::Acquire));
        assert!(cancel.media.is_cancelled());
    }

    #[test]
    fn codex_terminal_commit_linearizes_with_whole_turn_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let core = AppCore::new();
        core.save_project(Some(temp.path().join("Project.opentake")))
            .unwrap();
        let state = ChatState::new(
            core,
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project = state.project_context().unwrap();

        let mut cancelled_session = ChatSession::new("cancel-wins");
        cancelled_session
            .messages
            .push(ChatMessage::user("request"));
        state
            .put_project_session(&project, cancelled_session.clone())
            .unwrap();
        let cancelled_owner = Arc::new(TurnCancel::new());
        let cancelled_key = project.key(&cancelled_session.id);
        state
            .reserve_turn(cancelled_key.clone(), cancelled_owner.clone())
            .unwrap();
        cancelled_owner.request();
        cancelled_session
            .messages
            .push(ChatMessage::assistant("must not commit", Vec::new()));
        assert_eq!(
            state
                .finalize_project_turn(
                    &project,
                    &cancelled_key,
                    &cancelled_owner,
                    cancelled_session,
                )
                .unwrap(),
            TurnFinalization::Cancelled
        );
        let persisted = state.take_project_session(&project, "cancel-wins").unwrap();
        assert_eq!(persisted.messages.len(), 1);

        let mut committed_session = ChatSession::new("commit-wins");
        committed_session
            .messages
            .push(ChatMessage::user("request"));
        state
            .put_project_session(&project, committed_session.clone())
            .unwrap();
        let committed_owner = Arc::new(TurnCancel::new());
        let committed_key = project.key(&committed_session.id);
        state
            .reserve_turn(committed_key.clone(), committed_owner.clone())
            .unwrap();
        committed_session
            .messages
            .push(ChatMessage::assistant("committed", Vec::new()));
        assert_eq!(
            state
                .finalize_project_turn(
                    &project,
                    &committed_key,
                    &committed_owner,
                    committed_session,
                )
                .unwrap(),
            TurnFinalization::Committed
        );
        committed_owner.request();
        assert!(!committed_owner.requested.load(Ordering::Acquire));
        let persisted = state.take_project_session(&project, "commit-wins").unwrap();
        assert_eq!(persisted.messages.len(), 2);
        assert_eq!(persisted.messages[1].content, "committed");
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
        let old_cancel = Arc::new(TurnCancel::new());
        state
            .reserve_turn(project_a.key("chat-same"), old_cancel.clone())
            .unwrap();

        core.save_project(Some(target)).unwrap();

        assert!(old_cancel.requested.load(Ordering::Relaxed));
        assert!(old_cancel.media.is_cancelled());
        assert!(state.turns.lock().unwrap().running.is_empty());
        assert!(state.sessions.lock().unwrap().is_empty());
        let project_b = state.project_context().unwrap();
        state
            .reserve_turn(project_b.key("chat-same"), Arc::new(TurnCancel::new()))
            .expect("same session id in the Save As target is independent");
    }

    #[test]
    fn save_as_cancels_an_active_tool_before_waiting_for_its_identity_lease() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("Source.opentake");
        let target = temp.path().join("Target.opentake");
        let core = AppCore::new();
        core.save_project(Some(source)).unwrap();
        let state = ChatState::new(
            core.clone(),
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project = state.project_context().unwrap();
        let cancel = Arc::new(TurnCancel::new());
        state
            .reserve_turn(project.key("chat-active"), cancel.clone())
            .unwrap();
        let gate = ProjectTurnGate {
            state,
            project,
            cancel: cancel.clone(),
            undo_scope: "test:save-as".into(),
        };
        let (tool_started_tx, tool_started_rx) = std::sync::mpsc::channel();
        let (tool_finished_tx, tool_finished_rx) = std::sync::mpsc::channel();

        let tool = std::thread::spawn(move || {
            gate.with_current_project(|| {
                tool_started_tx.send(()).unwrap();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while !cancel.media.is_cancelled() && std::time::Instant::now() < deadline {
                    std::thread::yield_now();
                }
                if cancel.media.is_cancelled() {
                    tool_finished_tx.send(()).unwrap();
                }
            })
        });
        tool_started_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("the simulated media tool must hold the project identity lease");

        let saver = std::thread::spawn(move || core.save_project(Some(target)));

        tool_finished_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("Save As must cancel the active tool before waiting for the write lease");
        assert!(tool.join().unwrap().is_some());
        saver.join().unwrap().unwrap();
    }

    #[test]
    fn save_as_rejects_a_turn_registered_after_the_cancel_sweep() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("Source.opentake");
        let target = temp.path().join("Target.opentake");
        let core = AppCore::new();
        core.save_project(Some(source)).unwrap();
        let state = ChatState::new(
            core.clone(),
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project = state.project_context().unwrap();
        let (hook_entered_tx, hook_entered_rx) = std::sync::mpsc::channel();
        let hook_release = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let hook_release_for_listener = hook_release.clone();
        core.subscribe_project_identity_transition(move |pending| {
            if !pending {
                return;
            }
            hook_entered_tx.send(()).unwrap();
            let (released, wake) = &*hook_release_for_listener;
            let mut released = released.lock().unwrap();
            while !*released {
                released = wake.wait(released).unwrap();
            }
        });

        let saver = std::thread::spawn(move || core.save_project(Some(target)));
        hook_entered_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("Save As must reach the pre-transition hook");

        let late_cancel = Arc::new(TurnCancel::new());
        let reserve = state.reserve_turn(project.key("chat-too-late"), late_cancel.clone());
        let (released, wake) = &*hook_release;
        *released.lock().unwrap() = true;
        wake.notify_all();
        saver.join().unwrap().unwrap();

        assert!(
            reserve.is_err(),
            "a pending transition must reject new turns"
        );
        assert!(late_cancel.requested.load(Ordering::Relaxed));
        assert!(late_cancel.media.is_cancelled());
        let current = state.project_context().unwrap();
        state
            .reserve_turn(current.key("chat-after-save"), Arc::new(TurnCancel::new()))
            .expect("the transition flag must clear after Save As completes");
    }

    #[test]
    fn overlapping_save_as_transitions_keep_new_turns_blocked_until_both_finish() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("Source.opentake");
        let target_a = temp.path().join("Target-A.opentake");
        let target_b = temp.path().join("Target-B.opentake");
        let core = AppCore::new();
        core.save_project(Some(source)).unwrap();
        let state = ChatState::new(
            core.clone(),
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let true_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (first_entered_tx, first_entered_rx) = std::sync::mpsc::channel();
        let (second_entered_tx, second_entered_rx) = std::sync::mpsc::channel();
        let first_release = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let true_count_for_listener = true_count.clone();
        let first_release_for_listener = first_release.clone();
        core.subscribe_project_identity_transition(move |pending| {
            if !pending {
                return;
            }
            let index = true_count_for_listener.fetch_add(1, Ordering::Relaxed);
            if index == 0 {
                first_entered_tx.send(()).unwrap();
                let (released, wake) = &*first_release_for_listener;
                let mut released = released.lock().unwrap();
                while !*released {
                    released = wake.wait(released).unwrap();
                }
            } else {
                second_entered_tx.send(()).unwrap();
            }
        });

        let core_a = core.clone();
        let saver_a = std::thread::spawn(move || core_a.save_project(Some(target_a)));
        first_entered_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("the first transition must pause before its writer");
        let core_b = core.clone();
        let saver_b = std::thread::spawn(move || core_b.save_project(Some(target_b)));
        second_entered_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("the second transition must overlap the first");
        saver_b.join().unwrap().unwrap();

        let current = state.project_context().unwrap();
        let late_cancel = Arc::new(TurnCancel::new());
        let reserve = state.reserve_turn(current.key("chat-between-saves"), late_cancel.clone());
        let (released, wake) = &*first_release;
        *released.lock().unwrap() = true;
        wake.notify_all();
        saver_a.join().unwrap().unwrap();

        assert!(
            reserve.is_err(),
            "one completed transition must not clear another transition's pending state"
        );
        assert!(late_cancel.requested.load(Ordering::Relaxed));
        assert!(late_cancel.media.is_cancelled());
        let current = state.project_context().unwrap();
        state
            .reserve_turn(current.key("chat-after-both"), Arc::new(TurnCancel::new()))
            .expect("new turns may resume after both transitions finish");
    }

    #[test]
    fn failed_save_as_clears_the_transition_flag() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("Source.opentake");
        let invalid_target = temp.path().join("not-a-bundle");
        let core = AppCore::new();
        core.save_project(Some(source)).unwrap();
        let state = ChatState::new(
            core.clone(),
            temp.path().join("no-workflows"),
            temp.path().join("chat-cache"),
            temp.path().join("chat-models"),
        );
        let project = state.project_context().unwrap();
        std::fs::write(&invalid_target, b"regular file").unwrap();

        core.save_project(Some(invalid_target))
            .expect_err("Save As to a regular file must fail");

        assert_eq!(state.turns.lock().unwrap().transition_depth, 0);
        state
            .reserve_turn(
                project.key("chat-after-failure"),
                Arc::new(TurnCancel::new()),
            )
            .expect("a failed transition must not block later Agent turns");
    }
}
