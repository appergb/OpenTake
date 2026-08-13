use super::openai::{body as openai_body, message as openai_message};
use super::*;
use crate::tools::result::Block;
use opentake_gen::MemoryKeyStore;

fn store_with_key(provider: LlmProvider, value: &str) -> MemoryKeyStore {
    MemoryKeyStore::new().with_key(provider.key(), value)
}

#[test]
fn provider_choice_is_explicit() {
    assert_eq!(provider_from_choice("openai").unwrap(), LlmProvider::OpenAi);
    assert_eq!(
        provider_from_choice("anthropic").unwrap(),
        LlmProvider::Anthropic
    );
    let err = provider_from_choice("google").unwrap_err().to_string();
    assert!(err.contains("does not support provider"));
    let err = provider_from_choice("mystery").unwrap_err().to_string();
    assert!(err.contains("unknown provider"));
}

#[test]
fn stream_chat_requires_a_key_for_the_selected_provider() {
    let store = MemoryKeyStore::new();
    let err = futures::executor::block_on(stream_chat(
        LlmProvider::OpenAi,
        &store,
        ChatRequest {
            messages: &[ChatMessage::user("hi")],
            tools: &[],
            model: None,
        },
        &AtomicBool::new(false),
        |_| {},
    ))
    .unwrap_err()
    .to_string();
    assert!(err.contains("no API key configured for openai"));
}

#[test]
fn no_key_guide_mentions_settings_and_provider() {
    let msg = no_key_guide(LlmProvider::Anthropic);
    assert!(msg.contains("Settings"));
    assert!(msg.contains("Anthropic"));
}

#[test]
fn memory_store_round_trips_selected_provider_key() {
    let store = store_with_key(LlmProvider::OpenAi, "sk-test");
    let dyn_store: &dyn KeyStore = &store;
    assert_eq!(
        dyn_store
            .load(ProviderKey::OpenAI.account())
            .unwrap()
            .as_deref(),
        Some("sk-test")
    );
    assert_eq!(
        dyn_store.load(ProviderKey::Anthropic.account()).unwrap(),
        None
    );
}

#[test]
fn openai_body_shape_minimum() {
    let msgs = vec![ChatMessage::user("hi")];
    let body = openai_body("gpt-4o-mini", &msgs, &[]);
    assert_eq!(body["model"], "gpt-4o-mini");
    assert_eq!(body["stream"], true);
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "hi");
    assert!(body.get("tools").is_none());
}

#[test]
fn openai_body_with_tools() {
    let tools = vec![ToolSchema {
        name: "get_timeline".into(),
        description: "read".into(),
        parameters: serde_json::json!({"type": "object"}),
    }];
    let body = openai_body("m", &[ChatMessage::user("x")], &tools);
    let t = &body["tools"][0];
    assert_eq!(t["type"], "function");
    assert_eq!(t["function"]["name"], "get_timeline");
    assert_eq!(t["function"]["parameters"]["type"], "object");
}

#[test]
fn openai_assistant_with_tool_calls_round_trips() {
    let tc = ToolCall::request("call-1", "split_clip", serde_json::json!({"atFrame": 10}));
    let m = ChatMessage::assistant("splitting", vec![tc]);
    let v = openai_message(&m);
    assert_eq!(v["role"], "assistant");
    assert_eq!(v["tool_calls"][0]["id"], "call-1");
    assert_eq!(v["tool_calls"][0]["function"]["name"], "split_clip");
    assert_eq!(
        v["tool_calls"][0]["function"]["arguments"],
        "{\"atFrame\":10}"
    );
}

#[test]
fn openai_message_derives_assistant_fields_from_authoritative_blocks() {
    let mut message = ChatMessage::assistant_blocks_with_id(
        "assistant-blocks",
        vec![
            AgentContentBlock::Text { text: "A".into() },
            AgentContentBlock::ToolUse {
                id: "call-block".into(),
                name: "split_clip".into(),
                input: serde_json::json!({"atFrame": 10}),
                result: None,
                is_error: None,
            },
            AgentContentBlock::Text { text: "B".into() },
        ],
    );
    message.content = "stale".into();
    message.tool_calls = vec![ToolCall::request(
        "call-stale",
        "delete_clip",
        serde_json::json!({}),
    )];

    let wire = openai_message(&message);

    assert_eq!(wire["content"], "AB");
    assert_eq!(wire["tool_calls"].as_array().unwrap().len(), 1);
    assert_eq!(wire["tool_calls"][0]["id"], "call-block");
    assert_eq!(wire["tool_calls"][0]["function"]["name"], "split_clip");
}

#[test]
fn openai_tool_result_carries_tool_call_id() {
    let m = ChatMessage::tool_result("call-1", serde_json::json!({"summary": "ok"}));
    let v = openai_message(&m);
    assert_eq!(v["role"], "tool");
    assert_eq!(v["tool_call_id"], "call-1");
}

#[test]
fn anthropic_system_prompt_hoisted_top_level() {
    let msgs = vec![
        ChatMessage::system("you are an editor"),
        ChatMessage::user("hi"),
    ];
    let body = anthropic_body("claude", &msgs, &[]);
    assert_eq!(body["system"][0]["type"], "text");
    assert_eq!(body["system"][0]["text"], "you are an editor");
    assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    assert_eq!(body["messages"][0]["role"], "user");
}

#[test]
fn anthropic_body_preserves_authoritative_interleaved_assistant_blocks() {
    let message = ChatMessage::assistant_blocks_with_id(
        "assistant-interleaved",
        vec![
            AgentContentBlock::Text { text: "A".into() },
            AgentContentBlock::ToolUse {
                id: "call-1".into(),
                name: "split_clip".into(),
                input: serde_json::json!({"clipId": "c1"}),
                result: None,
                is_error: None,
            },
            AgentContentBlock::Text { text: "B".into() },
        ],
    );

    let body = anthropic_body("claude", &[message], &[]);

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
fn anthropic_request_sets_all_prompt_cache_boundaries_and_upstream_token_limit() {
    let tools = vec![
        ToolSchema {
            name: "get_timeline".into(),
            description: "read".into(),
            parameters: serde_json::json!({"type": "object"}),
        },
        ToolSchema {
            name: "split_clip".into(),
            description: "edit".into(),
            parameters: serde_json::json!({"type": "object"}),
        },
    ];
    let messages = vec![
        ChatMessage::system("system"),
        ChatMessage::user("first"),
        ChatMessage::assistant("ack", vec![]),
        ChatMessage::user("latest"),
    ];

    let body = anthropic_body("claude", &messages, &tools);

    assert_eq!(body["max_tokens"], 8192);
    assert!(body["tools"][0].get("cache_control").is_none());
    assert_eq!(body["tools"][1]["cache_control"]["type"], "ephemeral");
    assert_eq!(
        body["messages"][2]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert!(body["messages"][0]["content"][0]
        .get("cache_control")
        .is_none());
}

#[test]
fn anthropic_tool_results_nest_under_user_turns() {
    let msgs = vec![
        ChatMessage::user("please"),
        ChatMessage::assistant(
            "",
            vec![ToolCall::request(
                "c1",
                "get_timeline",
                serde_json::json!({}),
            )],
        ),
        ChatMessage::tool_error_result("c1", serde_json::json!({"error": "Cancelled"})),
    ];
    let body = anthropic_body("claude", &msgs, &[]);
    let turns = body["messages"].as_array().unwrap();
    assert_eq!(turns.len(), 3);
    let last = turns.last().unwrap();
    assert_eq!(last["role"], "user");
    assert_eq!(last["content"][0]["type"], "tool_result");
    assert_eq!(last["content"][0]["tool_use_id"], "c1");
    assert_eq!(last["content"][0]["is_error"], true);
}

#[test]
fn anthropic_tool_results_preserve_native_image_blocks() {
    let message = ChatMessage::tool_result_blocks(
        "c-image",
        vec![
            Block::text("before"),
            Block::image("aW1hZ2U=", "image/png"),
            Block::text("after"),
        ],
        serde_json::json!({"summary": "beforeafter", "isError": false}),
        false,
    );

    let body = anthropic_body("claude", &[message], &[]);
    let content = &body["messages"][0]["content"][0]["content"];

    assert_eq!(
        content[0],
        serde_json::json!({"type": "text", "text": "before"})
    );
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["type"], "base64");
    assert_eq!(content[1]["source"]["media_type"], "image/png");
    assert_eq!(content[1]["source"]["data"], "aW1hZ2U=");
    assert_eq!(
        content[2],
        serde_json::json!({"type": "text", "text": "after"})
    );
}

fn decode_anthropic_sse(sse: &str) -> Result<TurnResult, LlmError> {
    let mut decoder = AnthropicStreamDecoder::default();
    decoder.push_chunk(sse.as_bytes(), &mut |_| {})?;
    decoder.finish()
}

#[test]
fn anthropic_stream_rejects_eof_before_text_block_and_message_stop() {
    let sse = concat!(
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"partial\"}}\n\n",
    );

    let error = decode_anthropic_sse(sse).unwrap_err().to_string();

    assert!(error.contains("before content block 0 stopped"));
}

#[test]
fn anthropic_stream_rejects_eof_before_tool_block_and_message_stop() {
    let sse = concat!(
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call-1\",\"name\":\"split_clip\",\"input\":{}}}\n\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"clipId\\\":\"}}\n\n",
    );

    let error = decode_anthropic_sse(sse).unwrap_err().to_string();

    assert!(error.contains("before content block 0 stopped"));
}

#[test]
fn anthropic_stream_rejects_eof_without_message_stop() {
    let sse = concat!(
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    );

    let error = decode_anthropic_sse(sse).unwrap_err().to_string();

    assert!(error.contains("before message_stop"));
}

#[test]
fn anthropic_stream_rejects_message_stop_while_a_block_is_open() {
    let sse = concat!(
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );

    let error = decode_anthropic_sse(sse).unwrap_err().to_string();

    assert!(error.contains("message_stop before content block 0 stopped"));
}

#[test]
fn anthropic_stream_rejects_repeated_content_block_stop() {
    let sse = concat!(
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );

    let error = decode_anthropic_sse(sse).unwrap_err().to_string();

    assert!(error.contains("content block 0 stopped more than once"));
}

#[test]
fn anthropic_stream_rejects_delta_after_content_block_stop() {
    let sse = concat!(
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"late\"}}\n\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );

    let error = decode_anthropic_sse(sse).unwrap_err().to_string();

    assert!(error.contains("delta after content block 0 stopped"));
}

#[test]
fn anthropic_stream_rejects_repeated_message_stop() {
    let sse = concat!(
        "data: {\"type\":\"message_stop\"}\n\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );

    let error = decode_anthropic_sse(sse).unwrap_err().to_string();

    assert!(error.contains("message_stop more than once"));
}

#[test]
fn drain_sse_frames_handles_split_utf8_chunks() {
    let mut buffer = Vec::new();
    let bytes = "data: {\"text\":\"你\"}\n\n".as_bytes();
    buffer.extend_from_slice(&bytes[..15]);
    assert!(drain_sse_frames(&mut buffer).unwrap().is_empty());
    buffer.extend_from_slice(&bytes[15..]);
    let frames = drain_sse_frames(&mut buffer).unwrap();
    assert_eq!(frames, vec!["data: {\"text\":\"你\"}"]);
    assert!(buffer.is_empty());
}

#[tokio::test(start_paused = true)]
async fn next_chunk_or_cancel_interrupts_pending_stream() {
    let cancel = AtomicBool::new(false);
    let mut stream = futures::stream::pending::<Result<Vec<u8>, std::io::Error>>();
    let wait = next_chunk_or_cancel(&mut stream, &cancel);
    tokio::pin!(wait);

    tokio::task::yield_now().await;
    tokio::time::advance(CANCEL_POLL_INTERVAL).await;
    cancel.store(true, Ordering::Relaxed);
    tokio::time::advance(CANCEL_POLL_INTERVAL).await;

    let err = wait.await.unwrap_err().to_string();
    assert!(err.contains("cancelled"));
}
