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
        sequence: u64,
        block_index: usize,
        delta: String,
    },
    /// Insert or replace one authoritative content block.
    BlockUpsert {
        session_id: String,
        message_id: String,
        sequence: u64,
        block_index: usize,
        block: AgentContentBlock,
    },
    /// The assistant turn is complete (final text + all tool calls resolved).
    Done {
        session_id: String,
        message_id: String,
        sequence: u64,
        message: ChatMessage,
    },
}

#[derive(Default)]
struct EventSequence(u64);

impl EventSequence {
    fn take(&mut self) -> u64 {
        let sequence = self.0;
        self.0 = self.0.saturating_add(1);
        sequence
    }

    fn next(&self) -> u64 {
        self.0
    }
}

fn apply_stream_event(
    assistant: &mut ChatMessage,
    session_id: &str,
    emitter: &dyn EmitLoop,
    sequence: &mut EventSequence,
    event: StreamEvent,
) {
    match event {
        StreamEvent::BlockDelta { block_index, delta } => {
            let applied = assistant.append_text_delta_at(block_index, &delta);
            debug_assert!(applied, "provider emitted an invalid text block index");
            if applied {
                emitter.emit(LoopEvent::BlockDelta {
                    session_id: session_id.to_string(),
                    message_id: assistant.id.clone(),
                    sequence: sequence.take(),
                    block_index,
                    delta,
                });
            }
        }
        StreamEvent::BlockUpsert { block_index, block } => {
            let applied = assistant.upsert_block_at(block_index, block.clone());
            debug_assert!(applied, "provider emitted a non-contiguous block index");
            if applied {
                emitter.emit(LoopEvent::BlockUpsert {
                    session_id: session_id.to_string(),
                    message_id: assistant.id.clone(),
                    sequence: sequence.take(),
                    block_index,
                    block,
                });
            }
        }
    }
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
        sequence: u64,
    },
    #[error("cancelled")]
    Cancelled { message_id: String, sequence: u64 },
}

impl LoopError {
    pub fn llm(source: LlmError, message_id: &str, sequence: u64) -> Self {
        Self::Llm {
            source,
            message_id: message_id.to_string(),
            sequence,
        }
    }

    pub fn cancelled(message_id: &str, sequence: u64) -> Self {
        Self::Cancelled {
            message_id: message_id.to_string(),
            sequence,
        }
    }

    pub fn message_id(&self) -> &str {
        match self {
            Self::Llm { message_id, .. } | Self::Cancelled { message_id, .. } => message_id,
        }
    }

    pub fn sequence(&self) -> u64 {
        match self {
            Self::Llm { sequence, .. } | Self::Cancelled { sequence, .. } => *sequence,
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
            .map_err(|error| LoopError::llm(error, &first_message_id, 0))?;
        session.provider = Some(provider_choice);
        session.model = Some(provider.default_model().to_string());
        if !has_trailing_user_message(session, &user_text) {
            session.messages.push(ChatMessage::user(user_text));
        }

        if self
            .store
            .load(provider.key().account())
            .map_err(|e| LlmError::Network(format!("keychain: {e}")))
            .map_err(|error| LoopError::llm(error, &first_message_id, 0))?
            .is_none()
        {
            let mut sequence = EventSequence::default();
            let guide = no_key_guide(provider);
            let msg = ChatMessage::assistant_with_id(&first_message_id, &guide, Vec::new());
            emitter.emit(LoopEvent::BlockDelta {
                session_id: session.id.clone(),
                message_id: first_message_id.clone(),
                sequence: sequence.take(),
                block_index: 0,
                delta: guide,
            });
            emitter.emit(LoopEvent::Done {
                session_id: session.id.clone(),
                message_id: first_message_id.clone(),
                sequence: sequence.take(),
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
            let mut sequence = EventSequence::default();
            if cancel.load(Ordering::Relaxed) {
                return Err(LoopError::cancelled(&message_id, sequence.next()));
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
                return Err(LoopError::cancelled(&message_id, sequence.next()));
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

            let mut assistant = ChatMessage::assistant_blocks_with_id(&message_id, Vec::new());
            let turn = match stream_chat(provider, self.store.as_ref(), req, &cancel, |event| {
                apply_stream_event(&mut assistant, &sid, emitter, &mut sequence, event)
            })
            .await
            {
                Ok(turn) => turn,
                Err(LlmError::Cancelled) => {
                    return Err(LoopError::cancelled(&message_id, sequence.next()))
                }
                Err(error) => return Err(LoopError::llm(error, &message_id, sequence.next())),
            };

            if cancel.load(Ordering::Relaxed) {
                return Err(LoopError::cancelled(&message_id, sequence.next()));
            }

            debug_assert_eq!(assistant.content, turn.content);
            debug_assert_eq!(assistant.tool_calls.len(), turn.tool_calls.len());
            debug_assert_eq!(assistant.blocks, turn.blocks);
            let assistant_index = persist_assistant_tool_round(session, assistant);

            let mut resolved = Vec::with_capacity(turn.tool_calls.len());
            for mut tc in turn.tool_calls {
                if cancel.load(Ordering::Relaxed) {
                    return Err(LoopError::cancelled(&message_id, sequence.next()));
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
                .map_err(|error| LoopError::llm(error, &message_id, sequence.next()))?;
                let Some(result) = result else {
                    cancel.store(true, Ordering::Relaxed);
                    return Err(LoopError::cancelled(&message_id, sequence.next()));
                };
                if cancel.load(Ordering::Relaxed) {
                    return Err(LoopError::cancelled(&message_id, sequence.next()));
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
                        sequence: sequence.take(),
                        block_index,
                        block,
                    });
                }
                let tool_result = tool_result_message(tc_id, &result, result_json);
                if let Some(block) = tool_result.blocks.first().cloned() {
                    emitter.emit(LoopEvent::BlockUpsert {
                        session_id: sid.clone(),
                        message_id: tool_result.id.clone(),
                        sequence: 0,
                        block_index: 0,
                        block,
                    });
                }
                session.messages.push(tool_result);
                resolved.push(tc);
            }

            if resolved.is_empty() {
                if cancel.load(Ordering::Relaxed) {
                    return Err(LoopError::cancelled(&message_id, sequence.next()));
                }
                let assistant = session.messages[assistant_index].clone();
                emitter.emit(LoopEvent::Done {
                    session_id: sid.clone(),
                    message_id: message_id.clone(),
                    sequence: sequence.take(),
                    message: assistant,
                });
                return Ok(message_id);
            }
            message_id = next_message_id();
        }

        if cancel.load(Ordering::Relaxed) {
            return Err(LoopError::cancelled(&message_id, 0));
        }
        let mut sequence = EventSequence::default();
        let text = "Reached the tool-call round limit; stopping.";
        let last = ChatMessage::assistant_with_id(&message_id, text, Vec::new());
        emitter.emit(LoopEvent::BlockDelta {
            session_id: sid.clone(),
            message_id: message_id.clone(),
            sequence: sequence.take(),
            block_index: 0,
            delta: text.to_string(),
        });
        emitter.emit(LoopEvent::Done {
            session_id: sid,
            message_id: message_id.clone(),
            sequence: sequence.take(),
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
mod tests;
