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
fn anthropic_interleaved_sse_preserves_loop_event_and_next_round_body_order() {
    struct EventCollector {
        events: Arc<Mutex<Vec<LoopEvent>>>,
    }
    impl EmitLoop for EventCollector {
        fn emit(&self, event: LoopEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    let sse = concat!(
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"A\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call-1\",\"name\":\"split_clip\",\"input\":{}}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"clipId\\\":\\\"c1\\\"}\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"text_delta\",\"text\":\"B\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":2}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    );
    let events = Arc::new(Mutex::new(Vec::new()));
    let emitter = EventCollector {
        events: events.clone(),
    };
    let mut assistant = ChatMessage::assistant_blocks_with_id("assistant-sse", Vec::new());
    let mut decoder = crate::chat::llm::AnthropicStreamDecoder::default();

    for chunk in sse.as_bytes().chunks(37) {
        decoder
            .push_chunk(chunk, &mut |event| {
                apply_stream_event(&mut assistant, "session-sse", &emitter, event)
            })
            .unwrap();
    }
    let turn = decoder.finish().unwrap();

    assert_eq!(assistant.blocks, turn.blocks);
    assert!(matches!(
        &assistant.blocks[..],
        [
            AgentContentBlock::Text { text: first },
            AgentContentBlock::ToolUse { id, input, .. },
            AgentContentBlock::Text { text: second }
        ] if first == "A"
            && id == "call-1"
            && input == &serde_json::json!({"clipId": "c1"})
            && second == "B"
    ));
    let addressed = events
        .lock()
        .unwrap()
        .iter()
        .map(|event| match event {
            LoopEvent::BlockDelta { block_index, .. }
            | LoopEvent::BlockUpsert { block_index, .. } => *block_index,
            LoopEvent::Done { .. } => usize::MAX,
        })
        .collect::<Vec<_>>();
    assert_eq!(addressed, vec![0, 0, 0, 1, 1, 2, 2, 2]);

    let body = crate::chat::llm::anthropic_body("claude", &[assistant], &[]);
    assert_eq!(
        body["messages"][0]["content"],
        serde_json::json!([
            {"type": "text", "text": "A"},
            {
                "type": "tool_use",
                "id": "call-1",
                "name": "split_clip",
                "input": {"clipId": "c1"}
            },
            {"type": "text", "text": "B", "cache_control": {"type": "ephemeral"}}
        ])
    );
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
        with_redacted_dispatch_panic(|| panic!("provider panic carried oauth-super-secret-token"))
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
