//! Chat turn loop (`agent-SPEC.md` §5.3). Wires the LLM client to the SAME
//! [`crate::mcp::dispatch::Dispatcher`] the MCP server uses, so every tool call
//! lands on the single `EditCommand` entry — the in-app chat is a peer client
//! of the external MCP server, not a second editing path.
//!
//! One user message drives a turn that may take multiple LLM round-trips:
//! model → tool_call → dispatch → tool-result → model → … → final text. Each
//! intermediate event (text delta, tool call) is surfaced via [`LoopEvent`] so
//! the Tauri shell can emit `chat_delta` / `chat_tool_call` / `chat_done`.
//!
//! The system prompt is assembled once per round from the base prompt + active
//! plugin (via [`crate::prompt::assemble`]) plus a live Context Signal block so
//! the model knows what the timeline looks like right now without a
//! `get_timeline` round-trip.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use opentake_domain::Timeline;
use opentake_gen::KeyStore;

use crate::chat::llm::{
    no_key_guide, provider_from_choice, stream_chat, ChatRequest, LlmError, StreamEvent, ToolSchema,
};
use crate::chat::session::{
    next_message_id, AgentContentBlock, ChatMessage, ChatSession, ToolCall,
};
use crate::mcp::convert::safe_tool_result_for_llm;
use crate::mcp::dispatch::Dispatcher;
use crate::plugin::registry::PluginRegistry;
use crate::prompt::assemble::assemble_system_prompt;
use crate::signal::engine::build_signal;
use crate::tools::descriptions::{description, input_schema};
use crate::tools::panic_boundary::with_redacted_dispatch_panic;
use crate::tools::result::ToolResult;

fn tool_result_for_model(result: &ToolResult) -> serde_json::Value {
    safe_tool_result_for_llm(result)
}

fn tool_result_message(
    tool_call_id: impl Into<String>,
    result: &ToolResult,
    safe_result: serde_json::Value,
) -> ChatMessage {
    let blocks = if result.is_error {
        vec![crate::tools::result::Block::text(safe_result.to_string())]
    } else {
        result.content.clone()
    };
    ChatMessage::tool_result_blocks(tool_call_id, blocks, safe_result, result.is_error)
}

fn map_dispatch_join_error(error: tokio::task::JoinError) -> LlmError {
    tracing::error!(
        target: "opentake::chat::private",
        task_cancelled = error.is_cancelled(),
        task_panic = error.is_panic(),
        "chat tool dispatch task failed"
    );
    LlmError::Network("tool dispatch task failed".to_string())
}

fn has_trailing_user_message(session: &ChatSession, text: &str) -> bool {
    matches!(
        session.messages.last(),
        Some(ChatMessage {
            role: crate::chat::session::Role::User,
            content,
            ..
        }) if content == text
    )
}

fn persist_assistant_tool_round(session: &mut ChatSession, message: ChatMessage) -> usize {
    session.messages.push(message);
    session.messages.len() - 1
}

fn update_assistant_tool_call(
    session: &mut ChatSession,
    assistant_index: usize,
    resolved_tool_call: &ToolCall,
) -> Option<(usize, AgentContentBlock)> {
    if let Some(message) = session.messages.get_mut(assistant_index) {
        let block_index = message.upsert_tool_use(resolved_tool_call.clone());
        return message
            .blocks
            .get(block_index)
            .cloned()
            .map(|block| (block_index, block));
    }
    None
}

/// Restore the provider protocol after cancellation or a failed dispatch left
/// an assistant `tool_use` without its immediately-following `tool_result`.
/// Repairs are inserted before the next conversational turn and mirrored onto
/// the UI-facing [`ToolCall`] so a resumed session is both wire-valid and
/// visibly marked as cancelled.
fn resolve_orphan_tool_uses(messages: &mut Vec<ChatMessage>) -> usize {
    let mut repaired = 0;
    let mut index = 0;

    while index < messages.len() {
        if messages[index].role != crate::chat::session::Role::Assistant
            || messages[index].tool_calls.is_empty()
        {
            index += 1;
            continue;
        }

        let mut insert_at = index + 1;
        let mut answered = std::collections::HashSet::new();
        while let Some(message) = messages.get(insert_at) {
            if message.role != crate::chat::session::Role::Tool {
                break;
            }
            if let Some(tool_call_id) = &message.tool_call_id {
                answered.insert(tool_call_id.clone());
            }
            insert_at += 1;
        }

        let missing: Vec<String> = messages[index]
            .tool_calls
            .iter()
            .filter(|tool_call| !answered.contains(&tool_call.id))
            .map(|tool_call| tool_call.id.clone())
            .collect();
        for tool_call_id in missing {
            let cancelled = serde_json::json!({"error": "Cancelled"});
            let mut tool_call = messages[index]
                .tool_calls
                .iter()
                .find(|tool_call| tool_call.id == tool_call_id)
                .cloned()
                .unwrap_or_else(|| ToolCall::request(&tool_call_id, "", serde_json::json!({})));
            tool_call.result = Some(cancelled.clone());
            tool_call.is_error = Some(true);
            messages[index].upsert_tool_use(tool_call);
            messages.insert(
                insert_at,
                ChatMessage::tool_error_result(tool_call_id, cancelled),
            );
            insert_at += 1;
            repaired += 1;
        }
        index = insert_at;
    }

    repaired
}

/// Events the loop emits during a turn. The Tauri shell forwards each to a
/// matching front-end event.
#[derive(Clone, Debug)]
pub enum LoopEvent {
    /// A text chunk addressed to one authoritative message block.
    BlockDelta {
        session_id: String,
        message_id: String,
        block_index: usize,
        delta: String,
    },
    /// Insert or replace one authoritative content block.
    BlockUpsert {
        session_id: String,
        message_id: String,
        block_index: usize,
        block: AgentContentBlock,
    },
    /// The assistant turn is complete (final text + all tool calls resolved).
    Done {
        session_id: String,
        message_id: String,
        message: ChatMessage,
    },
}

/// Errors a turn can hit. The shell maps these to a final `chat_done` with an
/// error-styled assistant message so the user sees what went wrong.
#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("LLM error: {source}")]
    Llm {
        #[source]
        source: LlmError,
        message_id: String,
    },
    #[error("cancelled")]
    Cancelled { message_id: String },
}

impl LoopError {
    pub fn llm(source: LlmError, message_id: &str) -> Self {
        Self::Llm {
            source,
            message_id: message_id.to_string(),
        }
    }

    pub fn cancelled(message_id: &str) -> Self {
        Self::Cancelled {
            message_id: message_id.to_string(),
        }
    }

    pub fn message_id(&self) -> &str {
        match self {
            Self::Llm { message_id, .. } | Self::Cancelled { message_id } => message_id,
        }
    }
}

/// The bound a host provides so the loop can emit events back to the UI. The
/// Tauri shell implements this with `AppHandle::emit`; tests implement it with
/// an in-memory channel.
pub trait EmitLoop: Send + Sync {
    fn emit(&self, event: LoopEvent);
}

/// Per-turn project authority around every live timeline snapshot and tool
/// dispatch. Desktop chat supplies an epoch/path/root gate; standalone callers
/// use the direct gate below. `None` means the turn's authority is stale and
/// the loop must stop without publishing that result.
pub trait ChatTurnGate: Send + Sync {
    fn timeline(&self, dispatcher: &Dispatcher) -> Option<Timeline>;
    fn dispatch(
        &self,
        dispatcher: &Dispatcher,
        name: &str,
        args: serde_json::Value,
    ) -> Option<ToolResult>;

    /// Dispatch with the transport request's cancellation token. Turn-bound
    /// hosts normally cancel their own token from [`Self::request_cancel`];
    /// long-lived MCP authorities can instead forward this request-local token
    /// without coupling unrelated sessions.
    fn dispatch_cancellable(
        &self,
        dispatcher: &Dispatcher,
        name: &str,
        args: serde_json::Value,
        _request_cancel: &opentake_media::MediaCancelToken,
    ) -> Option<ToolResult> {
        self.dispatch(dispatcher, name, args)
    }

    /// Dispatch under a transport-supplied undo owner. Long-lived MCP gates use
    /// this to isolate rmcp sessions; project-turn gates may ignore it and retain
    /// their stable OpenTake ChatSession owner.
    fn dispatch_cancellable_scoped(
        &self,
        dispatcher: &Dispatcher,
        name: &str,
        args: serde_json::Value,
        _undo_scope: &str,
        request_cancel: &opentake_media::MediaCancelToken,
    ) -> Option<ToolResult> {
        self.dispatch_cancellable(dispatcher, name, args, request_cancel)
    }

    /// Request cancellation of the whole turn. Standalone callers have no
    /// project-bound cancellation state, so their default remains a no-op.
    fn request_cancel(&self) {}

    /// Stop in-flight dispatcher work while cleaning up an internally failed
    /// provider turn. Project hosts may keep this distinct from a user-requested
    /// whole-turn cancellation so the terminal failure can still be persisted.
    fn request_dispatch_cancel(&self) {
        self.request_cancel();
    }
}

struct DirectChatTurnGate;

impl ChatTurnGate for DirectChatTurnGate {
    fn timeline(&self, dispatcher: &Dispatcher) -> Option<Timeline> {
        Some(dispatcher.timeline())
    }

    fn dispatch(
        &self,
        dispatcher: &Dispatcher,
        name: &str,
        args: serde_json::Value,
    ) -> Option<ToolResult> {
        Some(dispatcher.dispatch(name, args))
    }
}

/// The chat loop. Clonable so the Tauri shell can hold one per session cheaply.
/// Holds a shared [`Dispatcher`] (same pipeline as MCP), a plugin registry
/// (read-locked for the active workflow), and the BYOK key store.
#[derive(Clone)]
pub struct ChatLoop {
    dispatcher: Arc<Dispatcher>,
    registry: Arc<RwLock<PluginRegistry>>,
    store: Arc<dyn KeyStore>,
}

impl ChatLoop {
    /// New loop over the same dispatcher + registry the MCP server uses.
    pub fn new(
        dispatcher: Arc<Dispatcher>,
        registry: Arc<RwLock<PluginRegistry>>,
        store: Arc<dyn KeyStore>,
    ) -> Self {
        ChatLoop {
            dispatcher,
            registry,
            store,
        }
    }

    /// The tool catalog in the OpenAI function-calling shape. Built fresh per
    /// turn (cheap; currently 38 base live tools) so the model always sees the
    /// current fail-closed catalog.
    ///
    /// When the dispatcher lacks a media bridge, hide the bridge-dependent
    /// tools instead of advertising tools that would only fail at runtime.
    fn tool_catalog(&self) -> Vec<ToolSchema> {
        self.dispatcher
            .advertised_tools()
            .into_iter()
            .map(|tool| ToolSchema {
                name: tool.as_str().to_string(),
                description: description(tool).to_string(),
                parameters: input_schema(tool),
            })
            .collect()
    }

    /// Assemble the system prompt: base + active plugin + a live Context Signal
    /// block describing the current timeline.
    fn system_prompt_for_timeline(&self, timeline: Timeline) -> String {
        let registry = self.registry.read().unwrap_or_else(|e| e.into_inner());
        let plugin = registry.active();
        let mut s = assemble_system_prompt(&registry, "default");
        let signal = build_signal(&timeline, plugin, None);
        if let Ok(json) = serde_json::to_value(&signal) {
            s.push_str("\n\n# Current timeline context signal\n");
            s.push_str(&serde_json::to_string_pretty(&json).unwrap_or_default());
            s.push_str("\n\nUse this signal to pick the right tool without re-reading the timeline first. For example, if the user asks to tighten silences on a talking-head timeline, call `tighten_silences` then `ripple_delete_ranges` with the accepted returned ranges.");
            if self.dispatcher.has_media_bridge() {
                s.push_str(" If the user asks to remove filler words, call `remove_filler_words`, let them review the word-aligned cuts, then apply only the accepted ranges with `ripple_delete_ranges`.");
            }
        }
        s
    }

    #[cfg(test)]
    fn system_prompt(&self) -> String {
        self.system_prompt_for_timeline(self.dispatcher.timeline())
    }

    /// Run one user turn. Appends the user message, then drives LLM round-trips
    /// until the model produces a final text turn with no further tool calls.
    pub async fn run_turn(
        &self,
        session: &mut ChatSession,
        provider_choice: String,
        user_text: String,
        emitter: &dyn EmitLoop,
        cancel: Arc<AtomicBool>,
    ) -> Result<String, LoopError> {
        let message_id = next_message_id();
        self.run_turn_gated(
            session,
            provider_choice,
            user_text,
            ChatTurn {
                first_message_id: message_id,
                cancel,
                gate: Arc::new(DirectChatTurnGate),
            },
            emitter,
        )
        .await
    }

    /// Run one user turn with an authority gate around every dispatcher read
    /// and side effect. The desktop uses this to bind the whole turn to the
    /// project identity captured when `chat_send` was accepted.
    pub async fn run_turn_gated(
        &self,
        session: &mut ChatSession,
        provider_choice: String,
        user_text: String,
        turn: ChatTurn,
        emitter: &dyn EmitLoop,
    ) -> Result<String, LoopError> {
        let ChatTurn {
            first_message_id,
            cancel,
            gate,
        } = turn;
        let provider = provider_from_choice(&provider_choice)
            .map_err(|error| LoopError::llm(error, &first_message_id))?;
        session.provider = Some(provider_choice);
        session.model = Some(provider.default_model().to_string());
        if !has_trailing_user_message(session, &user_text) {
            session.messages.push(ChatMessage::user(user_text));
        }

        if self
            .store
            .load(provider.key().account())
            .map_err(|e| LlmError::Network(format!("keychain: {e}")))
            .map_err(|error| LoopError::llm(error, &first_message_id))?
            .is_none()
        {
            let guide = no_key_guide(provider);
            let msg = ChatMessage::assistant_with_id(&first_message_id, &guide, Vec::new());
            emitter.emit(LoopEvent::BlockDelta {
                session_id: session.id.clone(),
                message_id: first_message_id.clone(),
                block_index: 0,
                delta: guide,
            });
            emitter.emit(LoopEvent::Done {
                session_id: session.id.clone(),
                message_id: first_message_id.clone(),
                message: msg.clone(),
            });
            session.messages.push(msg);
            return Ok(first_message_id);
        }

        let tools = self.tool_catalog();
        let sid = session.id.clone();
        let mut message_id = first_message_id;

        const MAX_ROUNDS: usize = 8;
        for _ in 0..MAX_ROUNDS {
            if cancel.load(Ordering::Relaxed) {
                return Err(LoopError::cancelled(&message_id));
            }

            let repaired = resolve_orphan_tool_uses(&mut session.messages);
            if repaired > 0 {
                tracing::debug!(
                    session_id = %session.id,
                    repaired,
                    "repaired orphaned chat tool uses before provider request"
                );
            }

            let Some(timeline) = gate.timeline(&self.dispatcher) else {
                cancel.store(true, Ordering::Relaxed);
                return Err(LoopError::cancelled(&message_id));
            };
            let system = self.system_prompt_for_timeline(timeline);
            let mut messages = Vec::with_capacity(session.messages.len() + 1);
            messages.push(ChatMessage::system(system));
            messages.extend(session.messages.iter().cloned());

            let req = ChatRequest {
                messages: &messages,
                tools: &tools,
                model: session.model.as_deref(),
            };

            let sid_clone = sid.clone();
            let mut assistant = ChatMessage::assistant_blocks_with_id(&message_id, Vec::new());
            let turn = match stream_chat(provider, self.store.as_ref(), req, &cancel, |ev| match ev
            {
                StreamEvent::Delta(delta) => {
                    let block_index = assistant.append_text_delta(&delta);
                    emitter.emit(LoopEvent::BlockDelta {
                        session_id: sid_clone.clone(),
                        message_id: assistant.id.clone(),
                        block_index,
                        delta,
                    });
                }
                StreamEvent::ToolCall(tool_call) => {
                    let block_index = assistant.upsert_tool_use(tool_call);
                    if let Some(block) = assistant.blocks.get(block_index).cloned() {
                        emitter.emit(LoopEvent::BlockUpsert {
                            session_id: sid_clone.clone(),
                            message_id: assistant.id.clone(),
                            block_index,
                            block,
                        });
                    }
                }
            })
            .await
            {
                Ok(turn) => turn,
                Err(LlmError::Cancelled) => return Err(LoopError::cancelled(&message_id)),
                Err(error) => return Err(LoopError::llm(error, &message_id)),
            };

            if cancel.load(Ordering::Relaxed) {
                return Err(LoopError::cancelled(&message_id));
            }

            debug_assert_eq!(assistant.content, turn.content);
            debug_assert_eq!(assistant.tool_calls.len(), turn.tool_calls.len());
            let assistant_index = persist_assistant_tool_round(session, assistant);

            let mut resolved = Vec::with_capacity(turn.tool_calls.len());
            for mut tc in turn.tool_calls {
                if cancel.load(Ordering::Relaxed) {
                    return Err(LoopError::cancelled(&message_id));
                }
                let dispatcher = self.dispatcher.clone();
                let gate = gate.clone();
                let name = tc.name.clone();
                let args = tc.args.clone();
                let result = tokio::task::spawn_blocking(move || {
                    with_redacted_dispatch_panic(|| gate.dispatch(&dispatcher, &name, args))
                })
                .await
                .map_err(map_dispatch_join_error)
                .map_err(|error| LoopError::llm(error, &message_id))?;
                let Some(result) = result else {
                    cancel.store(true, Ordering::Relaxed);
                    return Err(LoopError::cancelled(&message_id));
                };
                if cancel.load(Ordering::Relaxed) {
                    return Err(LoopError::cancelled(&message_id));
                }
                let result_json = tool_result_for_model(&result);
                let tc_id = tc.id.clone();
                tc.result = Some(result_json.clone());
                tc.is_error = Some(result.is_error);
                if let Some((block_index, block)) =
                    update_assistant_tool_call(session, assistant_index, &tc)
                {
                    emitter.emit(LoopEvent::BlockUpsert {
                        session_id: sid.clone(),
                        message_id: message_id.clone(),
                        block_index,
                        block,
                    });
                }
                let tool_result = tool_result_message(tc_id, &result, result_json);
                if let Some(block) = tool_result.blocks.first().cloned() {
                    emitter.emit(LoopEvent::BlockUpsert {
                        session_id: sid.clone(),
                        message_id: tool_result.id.clone(),
                        block_index: 0,
                        block,
                    });
                }
                session.messages.push(tool_result);
                resolved.push(tc);
            }

            if resolved.is_empty() {
                if cancel.load(Ordering::Relaxed) {
                    return Err(LoopError::cancelled(&message_id));
                }
                let assistant = session.messages[assistant_index].clone();
                emitter.emit(LoopEvent::Done {
                    session_id: sid.clone(),
                    message_id: message_id.clone(),
                    message: assistant,
                });
                return Ok(message_id);
            }
            message_id = next_message_id();
        }

        if cancel.load(Ordering::Relaxed) {
            return Err(LoopError::cancelled(&message_id));
        }
        let text = "Reached the tool-call round limit; stopping.";
        let last = ChatMessage::assistant_with_id(&message_id, text, Vec::new());
        emitter.emit(LoopEvent::BlockDelta {
            session_id: sid.clone(),
            message_id: message_id.clone(),
            block_index: 0,
            delta: text.to_string(),
        });
        emitter.emit(LoopEvent::Done {
            session_id: sid,
            message_id: message_id.clone(),
            message: last,
        });
        Ok(message_id)
    }
}

pub struct ChatTurn {
    pub first_message_id: String,
    pub cancel: Arc<AtomicBool>,
    pub gate: Arc<dyn ChatTurnGate>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::core_handle::CoreHandle;
    use opentake_domain::{Clip, ClipType, MediaManifest, Timeline, Track};
    use opentake_gen::MemoryKeyStore;
    use opentake_ops::{EditCommand, EditResult};
    use std::path::PathBuf;
    use std::sync::Mutex;

    #[test]
    fn events_are_addressed_by_session_message_and_block() {
        let block = crate::chat::AgentContentBlock::Text { text: "A".into() };
        let message = ChatMessage::assistant_blocks_with_id("assistant-1", vec![block.clone()]);
        let events = [
            LoopEvent::BlockDelta {
                session_id: "session-1".into(),
                message_id: "assistant-1".into(),
                block_index: 0,
                delta: "A".into(),
            },
            LoopEvent::BlockUpsert {
                session_id: "session-1".into(),
                message_id: "assistant-1".into(),
                block_index: 0,
                block,
            },
            LoopEvent::Done {
                session_id: "session-1".into(),
                message_id: "assistant-1".into(),
                message,
            },
        ];

        assert!(matches!(
            &events[0],
            LoopEvent::BlockDelta {
                session_id,
                message_id,
                block_index: 0,
                delta,
            } if session_id == "session-1" && message_id == "assistant-1" && delta == "A"
        ));
        assert!(matches!(
            &events[1],
            LoopEvent::BlockUpsert {
                message_id,
                block_index: 0,
                ..
            } if message_id == "assistant-1"
        ));
        assert!(matches!(
            &events[2],
            LoopEvent::Done {
                message_id,
                message,
                ..
            } if message_id == &message.id
        ));
    }

    #[test]
    fn orphan_tool_uses_are_repaired_before_the_next_user_turn() {
        let mut orphan =
            ToolCall::request("missing", "split_clip", serde_json::json!({"clipId": "c1"}));
        let resolved = ToolCall::request("resolved", "get_timeline", serde_json::json!({}));
        let mut messages = vec![
            ChatMessage::assistant("working", vec![resolved, orphan.clone()]),
            ChatMessage::tool_result("resolved", serde_json::json!({"ok": true})),
            ChatMessage::user("continue"),
        ];

        assert_eq!(resolve_orphan_tool_uses(&mut messages), 1);
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[2].role, crate::chat::Role::Tool);
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("missing"));
        assert_eq!(messages[2].tool_is_error, Some(true));
        assert!(messages[2].content.contains("Cancelled"));
        assert_eq!(messages[3].role, crate::chat::Role::User);
        orphan.result = Some(serde_json::json!({"error": "Cancelled"}));
        orphan.is_error = Some(true);
        assert_eq!(messages[0].tool_calls[1].result, orphan.result);
        assert_eq!(messages[0].tool_calls[1].is_error, Some(true));

        assert_eq!(resolve_orphan_tool_uses(&mut messages), 0);
        assert_eq!(messages.len(), 4);
    }

    #[test]
    fn dispatcher_failures_become_provider_error_results() {
        let failed_result = ToolResult::error("invalid clip");
        let failed_safe = tool_result_for_model(&failed_result);
        let failed = tool_result_message("call-failed", &failed_result, failed_safe.clone());
        assert_eq!(failed.tool_call_id.as_deref(), Some("call-failed"));
        assert_eq!(failed.tool_is_error, Some(true));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&failed.content).unwrap(),
            failed_safe
        );

        let succeeded_result = ToolResult::ok("done");
        let succeeded = tool_result_message(
            "call-ok",
            &succeeded_result,
            tool_result_for_model(&succeeded_result),
        );
        assert_eq!(succeeded.tool_is_error, None);
    }

    /// A minimal CoreHandle over an in-memory timeline + manifest, so the loop
    /// can dispatch read tools without a full AppCore.
    struct FakeHandle {
        timeline: Timeline,
    }
    impl CoreHandle for FakeHandle {
        fn timeline(&self) -> Timeline {
            self.timeline.clone()
        }
        fn media(&self) -> MediaManifest {
            MediaManifest::new()
        }
        fn apply(&self, _cmd: EditCommand) -> anyhow::Result<EditResult> {
            Ok(EditResult {
                changed: false,
                timeline_changed: false,
                manifest_changed: false,
                action_name: "noop".into(),
                affected_clip_ids: Vec::new(),
                timeline_version: 0,
                summary: "noop".into(),
            })
        }
        fn project_dir(&self) -> Option<PathBuf> {
            None
        }
    }

    /// An emitter that just collects events for assertions.
    struct CollectEmitter {
        events: Arc<Mutex<Vec<String>>>,
    }
    impl EmitLoop for CollectEmitter {
        fn emit(&self, event: LoopEvent) {
            let s = match event {
                LoopEvent::BlockDelta { delta, .. } => format!("delta:{delta}"),
                LoopEvent::BlockUpsert { block, .. } => format!("block:{block:?}"),
                LoopEvent::Done { message, .. } => format!("done:{}", message.content),
            };
            self.events.lock().unwrap().push(s);
        }
    }

    fn talking_head_timeline() -> Timeline {
        let mut tl = Timeline::new();
        let mut v = Track::new("v1", ClipType::Video);
        v.clips.push(Clip::new("c1", "asset", 0, 30 * 20));
        tl.tracks.push(v);
        tl
    }

    fn build_loop(timeline: Timeline, store: Arc<dyn KeyStore>) -> ChatLoop {
        let handle: Arc<dyn CoreHandle> = Arc::new(FakeHandle { timeline });
        let registry = Arc::new(RwLock::new(PluginRegistry::new()));
        let dispatcher = Arc::new(Dispatcher::new(handle, registry.clone()));
        ChatLoop::new(dispatcher, registry, store)
    }

    #[test]
    fn tool_catalog_hides_bridge_tools_when_bridge_is_missing() {
        let loop_ = build_loop(talking_head_timeline(), Arc::new(MemoryKeyStore::new()));
        let tools = loop_.tool_catalog();
        assert!(tools.iter().any(|t| t.name == "tighten_silences"));
        assert!(!tools.iter().any(|t| t.name == "remove_filler_words"));
        assert!(!tools.iter().any(|t| t.name == "get_transcript"));
        assert!(!tools.iter().any(|t| t.name == "search_media"));
        assert!(!tools.iter().any(|t| t.name == "inspect_media"));
        assert!(!tools.iter().any(|t| t.name == "inspect_timeline"));
        assert!(!tools.iter().any(|t| t.name == "import_media"));
    }

    #[test]
    fn system_prompt_includes_context_signal() {
        let loop_ = build_loop(talking_head_timeline(), Arc::new(MemoryKeyStore::new()));
        let prompt = loop_.system_prompt();
        assert!(prompt.contains("context signal"));
        assert!(prompt.contains("talking_head") || prompt.contains("video_type"));
    }

    #[test]
    fn tool_round_persists_assistant_before_tool_results() {
        let mut session = ChatSession::new("s1");
        session.messages.push(ChatMessage::user("trim this"));

        let requested = vec![ToolCall::request(
            "call-1",
            "get_timeline",
            serde_json::json!({}),
        )];
        let assistant_index = persist_assistant_tool_round(
            &mut session,
            ChatMessage::assistant("working", requested.clone()),
        );

        let mut resolved = requested[0].clone();
        resolved.result = Some(serde_json::json!({"summary": "ok"}));
        resolved.is_error = Some(false);
        update_assistant_tool_call(&mut session, assistant_index, &resolved);
        session.messages.push(ChatMessage::tool_result(
            resolved.id.clone(),
            resolved.result.clone().unwrap(),
        ));

        assert_eq!(
            session.messages[1].role,
            crate::chat::session::Role::Assistant
        );
        assert_eq!(session.messages[1].tool_calls.len(), 1);
        assert_eq!(
            session.messages[1].tool_calls[0].result,
            Some(serde_json::json!({"summary": "ok"}))
        );
        assert_eq!(session.messages[2].role, crate::chat::session::Role::Tool);
        assert_eq!(session.messages[2].tool_call_id.as_deref(), Some("call-1"));
    }

    #[test]
    fn chat_tool_result_uses_shared_fail_closed_error_boundary() {
        let private = "quota exhausted for customer alice plan enterprise";
        let value = tool_result_for_model(&ToolResult::error(private));
        let wire = value.to_string();
        assert!(wire.contains("MCP_TOOL_ERROR_REDACTED"));
        assert!(!wire.contains(private));
        assert_eq!(value["isError"], true);
    }

    #[tokio::test]
    async fn chat_join_error_does_not_expose_panic_payload() {
        let join = tokio::task::spawn_blocking(|| {
            with_redacted_dispatch_panic(|| {
                panic!("provider panic carried oauth-super-secret-token")
            })
        })
        .await
        .expect_err("worker must panic");
        let error = map_dispatch_join_error(join).to_string();
        assert!(error.contains("tool dispatch task failed"));
        assert!(!error.contains("oauth-super-secret-token"));
    }

    #[tokio::test]
    async fn no_key_path_emits_guide_and_done() {
        let loop_ = build_loop(talking_head_timeline(), Arc::new(MemoryKeyStore::new()));
        let mut session = ChatSession::new("s1");
        let events = Arc::new(Mutex::new(Vec::new()));
        let emitter = CollectEmitter {
            events: events.clone(),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        loop_
            .run_turn(
                &mut session,
                "openai".into(),
                "tighten silences".into(),
                &emitter,
                cancel,
            )
            .await
            .unwrap();
        let evs = events.lock().unwrap().clone();
        assert!(evs.iter().any(|e| e.contains("Settings")));
        assert!(evs.iter().any(|e| e.starts_with("done:")));
        assert_eq!(session.messages.len(), 2);
    }

    #[tokio::test]
    async fn events_no_key_path_reuses_message_id_from_first_delta_through_done() {
        struct IdentityEmitter {
            events: Arc<Mutex<Vec<LoopEvent>>>,
        }
        impl EmitLoop for IdentityEmitter {
            fn emit(&self, event: LoopEvent) {
                self.events.lock().unwrap().push(event);
            }
        }

        let loop_ = build_loop(talking_head_timeline(), Arc::new(MemoryKeyStore::new()));
        let mut session = ChatSession::new("session-identity");
        let events = Arc::new(Mutex::new(Vec::new()));
        let emitter = IdentityEmitter {
            events: events.clone(),
        };
        let message_id = loop_
            .run_turn(
                &mut session,
                "openai".into(),
                "hello".into(),
                &emitter,
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .unwrap();

        let events = events.lock().unwrap();
        assert!(matches!(
            &events[0],
            LoopEvent::BlockDelta {
                session_id,
                message_id: delta_message_id,
                block_index: 0,
                ..
            } if session_id == "session-identity" && delta_message_id == &message_id
        ));
        assert!(matches!(
            &events[1],
            LoopEvent::Done {
                message_id: done_message_id,
                message,
                ..
            } if done_message_id == &message_id && message.id == message_id
        ));
        assert_eq!(session.messages.last().unwrap().id, message_id);
    }

    #[test]
    fn events_errors_and_cancellation_retain_the_active_message_id() {
        let cancelled = LoopError::cancelled("assistant-active");
        let failed = LoopError::llm(
            LlmError::Provider("provider failed".into()),
            "assistant-active",
        );

        assert_eq!(cancelled.message_id(), "assistant-active");
        assert_eq!(failed.message_id(), "assistant-active");
    }

    #[tokio::test]
    async fn unsupported_provider_fails_before_streaming() {
        let loop_ = build_loop(talking_head_timeline(), Arc::new(MemoryKeyStore::new()));
        let mut session = ChatSession::new("s1");
        let cancel = Arc::new(AtomicBool::new(false));
        let emitter = CollectEmitter {
            events: Arc::new(Mutex::new(Vec::new())),
        };
        let err = loop_
            .run_turn(
                &mut session,
                "google".into(),
                "hello".into(),
                &emitter,
                cancel,
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("does not support provider"));
        assert!(session.messages.is_empty());
    }
}
