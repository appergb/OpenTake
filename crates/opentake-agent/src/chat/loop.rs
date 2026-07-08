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

use opentake_gen::KeyStore;

use crate::chat::llm::{
    no_key_guide, provider_from_choice, stream_chat, ChatRequest, LlmError, StreamEvent, ToolSchema,
};
use crate::chat::session::{ChatMessage, ChatSession, ToolCall};
use crate::mcp::dispatch::Dispatcher;
use crate::plugin::registry::PluginRegistry;
use crate::prompt::assemble::assemble_system_prompt;
use crate::signal::engine::build_signal;
use crate::tools::descriptions::{description, input_schema};
use crate::tools::names::ToolName;

/// Events the loop emits during a turn. The Tauri shell forwards each to a
/// matching front-end event.
#[derive(Clone, Debug)]
pub enum LoopEvent {
    /// A text chunk from the assistant (concatenate in order).
    Delta { session_id: String, delta: String },
    /// A tool call. Emitted twice per call: once when the model requests it
    /// (result=None), once after dispatch fills the result.
    ToolCall {
        session_id: String,
        tool_call: ToolCall,
    },
    /// The assistant turn is complete (final text + all tool calls resolved).
    Done {
        session_id: String,
        message: ChatMessage,
    },
}

/// Errors a turn can hit. The shell maps these to a final `chat_done` with an
/// error-styled assistant message so the user sees what went wrong.
#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),
    #[error("cancelled")]
    Cancelled,
}

/// The bound a host provides so the loop can emit events back to the UI. The
/// Tauri shell implements this with `AppHandle::emit`; tests implement it with
/// an in-memory channel.
pub trait EmitLoop: Send + Sync {
    fn emit(&self, event: LoopEvent);
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
    /// turn (cheap; ~44 tools) so the model always sees the current schema.
    ///
    /// When the dispatcher lacks a media bridge, hide the bridge-dependent
    /// tools instead of advertising tools that would only fail at runtime.
    fn tool_catalog(&self) -> Vec<ToolSchema> {
        ToolName::ALL
            .iter()
            .copied()
            .filter(|tool| {
                self.dispatcher.has_media_bridge()
                    || !matches!(tool, ToolName::InspectTimeline | ToolName::ImportMedia)
            })
            .map(|tool| ToolSchema {
                name: tool.as_str().to_string(),
                description: description(tool).to_string(),
                parameters: input_schema(tool),
            })
            .collect()
    }

    /// Assemble the system prompt: base + active plugin + a live Context Signal
    /// block describing the current timeline.
    fn system_prompt(&self) -> String {
        let timeline = self.dispatcher.timeline();
        let registry = self.registry.read().unwrap_or_else(|e| e.into_inner());
        let plugin = registry.active();
        let mut s = assemble_system_prompt(&registry, "default");
        let signal = build_signal(&timeline, plugin, None);
        if let Ok(json) = serde_json::to_value(&signal) {
            s.push_str("\n\n# Current timeline context signal\n");
            s.push_str(&serde_json::to_string_pretty(&json).unwrap_or_default());
            s.push_str("\n\nUse this signal to pick the right tool without re-reading the timeline first. For example, if the user asks to tighten silences on a talking-head timeline, call `tighten_silences` then `ripple_delete_ranges` with the returned ranges.");
        }
        s
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
    ) -> Result<(), LoopError> {
        let provider = provider_from_choice(&provider_choice)?;
        session.provider = Some(provider_choice);
        session.model = Some(provider.default_model().to_string());
        session.messages.push(ChatMessage::user(user_text));

        if self
            .store
            .load(provider.key().account())
            .map_err(|e| LlmError::Network(format!("keychain: {e}")))?
            .is_none()
        {
            let guide = no_key_guide(provider);
            let msg = ChatMessage::assistant(guide.clone(), Vec::new());
            emitter.emit(LoopEvent::Delta {
                session_id: session.id.clone(),
                delta: guide,
            });
            emitter.emit(LoopEvent::Done {
                session_id: session.id.clone(),
                message: msg.clone(),
            });
            session.messages.push(msg);
            return Ok(());
        }

        let tools = self.tool_catalog();
        let sid = session.id.clone();

        const MAX_ROUNDS: usize = 8;
        for _ in 0..MAX_ROUNDS {
            if cancel.load(Ordering::Relaxed) {
                return Err(LoopError::Cancelled);
            }

            let system = self.system_prompt();
            let mut messages = Vec::with_capacity(session.messages.len() + 1);
            messages.push(ChatMessage::system(system));
            messages.extend(session.messages.iter().cloned());

            let req = ChatRequest {
                messages: &messages,
                tools: &tools,
                model: session.model.as_deref(),
            };

            let sid_clone = sid.clone();
            let turn = stream_chat(provider, self.store.as_ref(), req, |ev| match ev {
                StreamEvent::Delta(delta) => emitter.emit(LoopEvent::Delta {
                    session_id: sid_clone.clone(),
                    delta,
                }),
                StreamEvent::ToolCall(tool_call) => emitter.emit(LoopEvent::ToolCall {
                    session_id: sid_clone.clone(),
                    tool_call,
                }),
            })
            .await?;

            let mut resolved = Vec::with_capacity(turn.tool_calls.len());
            for mut tc in turn.tool_calls {
                if cancel.load(Ordering::Relaxed) {
                    return Err(LoopError::Cancelled);
                }
                let dispatcher = self.dispatcher.clone();
                let name = tc.name.clone();
                let args = tc.args.clone();
                let result = tokio::task::spawn_blocking(move || dispatcher.dispatch(&name, args))
                    .await
                    .map_err(|e| LlmError::Network(format!("dispatch task: {e}")))?;
                let result_json = serde_json::json!({
                    "summary": result.text_joined(),
                    "isError": result.is_error,
                });
                let tc_id = tc.id.clone();
                tc.result = Some(result_json.clone());
                tc.is_error = Some(result.is_error);
                emitter.emit(LoopEvent::ToolCall {
                    session_id: sid.clone(),
                    tool_call: tc.clone(),
                });
                session
                    .messages
                    .push(ChatMessage::tool_result(tc_id, result_json));
                resolved.push(tc);
            }

            let assistant = ChatMessage::assistant(turn.content, resolved);
            let no_tool_calls = assistant.tool_calls.is_empty();
            session.messages.push(assistant.clone());

            if no_tool_calls {
                emitter.emit(LoopEvent::Done {
                    session_id: sid.clone(),
                    message: assistant,
                });
                return Ok(());
            }
        }

        let last = session.messages.last().cloned().unwrap_or_else(|| {
            ChatMessage::assistant("Reached the tool-call round limit; stopping.", Vec::new())
        });
        emitter.emit(LoopEvent::Done {
            session_id: sid,
            message: last,
        });
        Ok(())
    }
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
                LoopEvent::Delta { delta, .. } => format!("delta:{delta}"),
                LoopEvent::ToolCall { tool_call, .. } => {
                    format!(
                        "tool:{}:{}",
                        tool_call.name,
                        tool_call.is_error.unwrap_or(false)
                    )
                }
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
