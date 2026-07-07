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
//! The system prompt is assembled once per turn from the base prompt + active
//! plugin (via [`crate::prompt::assemble`]) plus a live Context Signal block
//! (video type / track roles / stage guidance) so the model knows what the
//! timeline looks like right now without a `get_timeline` round-trip.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use opentake_domain::Timeline;
use opentake_gen::KeyringStore;

use crate::chat::llm::{
    pick_provider, stream_chat, ChatRequest, LlmError, StreamEvent, ToolSchema, NO_KEY_GUIDE,
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
    registry: Arc<std::sync::RwLock<PluginRegistry>>,
    store: Arc<KeyringStore>,
}

impl ChatLoop {
    /// New loop over the same dispatcher + registry the MCP server uses. The
    /// store is the production `KeyringStore` (OS keychain).
    pub fn new(
        dispatcher: Arc<Dispatcher>,
        registry: Arc<std::sync::RwLock<PluginRegistry>>,
        store: Arc<KeyringStore>,
    ) -> Self {
        ChatLoop {
            dispatcher,
            registry,
            store,
        }
    }

    /// The tool catalog in the OpenAI function-calling shape. Built fresh per
    /// turn (cheap; 44 tools) so the model always sees the current schema.
    fn tool_catalog() -> Vec<ToolSchema> {
        ToolName::ALL
            .iter()
            .map(|&t| ToolSchema {
                name: t.as_str().to_string(),
                description: description(t).to_string(),
                parameters: input_schema(t),
            })
            .collect()
    }

    /// Assemble the system prompt: base + active plugin + a live Context Signal
    /// block describing the current timeline. The signal block means the model
    /// can act on "tighten the silences" without first calling `get_timeline`.
    fn system_prompt(&self, timeline: &Timeline) -> String {
        let registry = self.registry.read().unwrap_or_else(|e| e.into_inner());
        let plugin = registry.active();
        let mut s = assemble_system_prompt(&registry, "default");
        let signal = build_signal(timeline, plugin, None);
        if let Ok(json) = serde_json::to_value(&signal) {
            s.push_str("\n\n# Current timeline context signal\n");
            s.push_str(&serde_json::to_string_pretty(&json).unwrap_or_default());
            s.push_str("\n\nUse this signal to pick the right tool without re-reading the timeline first. For example, if the user asks to tighten silences on a talking-head timeline, call `tighten_silences` then `ripple_delete_ranges` with the returned ranges.");
        }
        s
    }

    /// Run one user turn. Appends the user message, then drives LLM round-trips
    /// until the model produces a final text turn with no further tool calls.
    /// Each delta / tool call / final message is surfaced via `emitter`.
    ///
    /// `cancel` is a shared atomic flag the host sets to request a stop; the
    /// loop checks it between rounds (no tokio dependency so the host can be a
    /// `--no-default-features` Tauri build without the optional tokio crate).
    pub async fn run_turn(
        &self,
        session: &mut ChatSession,
        user_text: String,
        timeline: Timeline,
        emitter: &dyn EmitLoop,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), LoopError> {
        // 1. Pick provider; short-circuit to the guided message if no key.
        let provider = match pick_provider(&self.store) {
            Some(p) => p,
            None => {
                let msg = ChatMessage::assistant(NO_KEY_GUIDE.to_string(), Vec::new());
                emitter.emit(LoopEvent::Delta {
                    session_id: session.id.clone(),
                    delta: NO_KEY_GUIDE.to_string(),
                });
                emitter.emit(LoopEvent::Done {
                    session_id: session.id.clone(),
                    message: msg.clone(),
                });
                session.messages.push(ChatMessage::user(user_text));
                session.messages.push(msg);
                return Ok(());
            }
        };
        session.provider = Some(provider.as_str().to_string());
        session.model = Some(
            session
                .model
                .clone()
                .unwrap_or_else(|| provider.default_model().to_string()),
        );

        // 2. Append the user message.
        session.messages.push(ChatMessage::user(user_text));

        let tools = Self::tool_catalog();
        let sid = session.id.clone();

        // 3. Round-trips: model → tool_call → dispatch → tool-result → model.
        // Bound the loop so a model stuck calling tools forever can't hang the
        // session (P1 hard cap; a real budget lives in a follow-up).
        const MAX_ROUNDS: usize = 8;
        for _ in 0..MAX_ROUNDS {
            if cancel.load(Ordering::Relaxed) {
                return Err(LoopError::Cancelled);
            }

            // Rebuild the system prompt each round so the Context Signal
            // reflects the latest timeline (a prior tool call may have moved
            // clips). Cheap: the signal is a pure derivation.
            let system = self.system_prompt(&timeline);
            let mut messages = Vec::with_capacity(session.messages.len() + 1);
            messages.push(ChatMessage::system(system));
            messages.extend(session.messages.iter().cloned());

            let req = ChatRequest {
                messages: &messages,
                tools: &tools,
                model: session.model.as_deref(),
            };

            let sid_clone = sid.clone();
            let turn = stream_chat(provider, &self.store, req, |ev| match ev {
                StreamEvent::Delta(d) => emitter.emit(LoopEvent::Delta {
                    session_id: sid_clone.clone(),
                    delta: d,
                }),
                StreamEvent::ToolCall(tc) => emitter.emit(LoopEvent::ToolCall {
                    session_id: sid_clone.clone(),
                    tool_call: tc,
                }),
            })
            .await?;

            // 4. Dispatch each tool call through the shared pipeline. Each
            //    dispatch is sync; run on spawn_blocking so the async runtime
            //    isn't held.
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
                // ToolResult → a compact JSON value for the front-end card +
                // the next LLM round. text_joined concatenates text blocks
                // (image blocks, rare in chat, are dropped for token budget).
                let result_json: serde_json::Value = serde_json::json!({
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
                // Append a tool-result message so the next LLM round sees it.
                session
                    .messages
                    .push(ChatMessage::tool_result(tc_id, result_json));
                resolved.push(tc);
            }

            // 5. Record the assistant turn (with any tool calls) and decide
            //    whether to continue.
            let assistant = ChatMessage::assistant(turn.content.clone(), resolved);
            let no_tool_calls = assistant.tool_calls.is_empty();
            session.messages.push(assistant.clone());

            if no_tool_calls {
                // No tool calls this round → final turn.
                emitter.emit(LoopEvent::Done {
                    session_id: sid.clone(),
                    message: assistant,
                });
                return Ok(());
            }
            // Otherwise loop: the tool results are now in history, the model
            // gets to respond to them next round.
        }

        // Hit the round cap: emit what we have with a truncation note.
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
    use opentake_domain::{Clip, ClipType, Timeline, Track};
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
        fn media(&self) -> opentake_domain::MediaManifest {
            opentake_domain::MediaManifest::new()
        }
        fn apply(
            &self,
            _cmd: opentake_ops::EditCommand,
        ) -> anyhow::Result<opentake_ops::EditResult> {
            Ok(opentake_ops::EditResult {
                changed: false,
                action_name: "noop".into(),
                affected_clip_ids: Vec::new(),
                timeline_version: 0,
                summary: "noop".into(),
            })
        }
        fn project_dir(&self) -> Option<std::path::PathBuf> {
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

    fn build_loop(timeline: Timeline) -> ChatLoop {
        let handle: Arc<dyn CoreHandle> = Arc::new(FakeHandle { timeline });
        let registry = Arc::new(std::sync::RwLock::new(PluginRegistry::new()));
        let dispatcher = Arc::new(Dispatcher::new(handle, registry.clone()));
        let store = Arc::new(KeyringStore::new());
        ChatLoop::new(dispatcher, registry, store)
    }

    #[test]
    fn tool_catalog_covers_all_44_tools() {
        let tools = ChatLoop::tool_catalog();
        assert_eq!(tools.len(), ToolName::ALL.len());
        assert!(tools.iter().any(|t| t.name == "tighten_silences"));
        assert!(tools.iter().any(|t| t.name == "get_timeline"));
    }

    #[test]
    fn system_prompt_includes_context_signal() {
        let loop_ = build_loop(talking_head_timeline());
        let s = loop_.system_prompt(&talking_head_timeline());
        assert!(s.contains("context signal"));
        assert!(s.contains("talking_head") || s.contains("video_type"));
    }

    #[tokio::test]
    async fn no_key_path_emits_guide_and_done() {
        let loop_ = build_loop(talking_head_timeline());
        if pick_provider(&loop_.store).is_some() {
            eprintln!("skip: a provider key is present on this machine");
            return;
        }
        let mut session = ChatSession::new("s1");
        let events = Arc::new(Mutex::new(Vec::new()));
        let emitter = CollectEmitter {
            events: events.clone(),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let tl = talking_head_timeline();
        loop_
            .run_turn(
                &mut session,
                "tighten silences".into(),
                tl,
                &emitter,
                cancel,
            )
            .await
            .unwrap();
        let evs = events.lock().unwrap().clone();
        assert!(evs.iter().any(|e| e.contains("Settings")));
        assert!(evs.iter().any(|e| e.starts_with("done:")));
        assert_eq!(session.messages.len(), 2); // user + assistant(guide)
    }
}
