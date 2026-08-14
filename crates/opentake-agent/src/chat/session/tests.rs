use super::*;

#[test]
fn roles_serialize_lowercase() {
    assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
    assert_eq!(
        serde_json::to_string(&Role::Assistant).unwrap(),
        "\"assistant\""
    );
    assert_eq!(serde_json::to_string(&Role::Tool).unwrap(), "\"tool\"");
}

#[test]
fn message_camelcase_round_trip() {
    let m = ChatMessage::assistant("hi", vec![]);
    let v = serde_json::to_value(&m).unwrap();
    assert_eq!(v["role"], "assistant");
    assert_eq!(v["content"], "hi");
    assert_eq!(
        v["createdAt"],
        serde_json::Value::Number(m.created_at.into())
    );
    assert!(v["toolCalls"].is_array());
    assert!(v.get("toolCallId").is_none());
}

#[test]
fn tool_call_carries_result_only_when_present() {
    let mut tc = ToolCall::request("call-1", "get_timeline", serde_json::json!({}));
    let v = serde_json::to_value(&tc).unwrap();
    assert!(v.get("result").is_none());
    assert!(v.get("isError").is_none());
    tc.result = Some(serde_json::json!({"ok": true}));
    tc.is_error = Some(false);
    let v = serde_json::to_value(&tc).unwrap();
    assert_eq!(v["result"]["ok"], true);
    assert_eq!(v["isError"], false);
}

#[test]
fn tool_result_message_has_tool_call_id() {
    let m = ChatMessage::tool_result("call-1", serde_json::json!({"summary": "ok"}));
    let v = serde_json::to_value(&m).unwrap();
    assert_eq!(v["role"], "tool");
    assert_eq!(v["toolCallId"], "call-1");
    assert!(v.get("toolIsError").is_none());
    assert!(v["content"].as_str().unwrap().contains("summary"));
}

#[test]
fn tool_error_result_round_trips_an_explicit_error_marker() {
    let m = ChatMessage::tool_error_result("call-1", serde_json::json!({"error": "Cancelled"}));
    let v = serde_json::to_value(&m).unwrap();
    assert_eq!(v["toolIsError"], true);
    let back: ChatMessage = serde_json::from_value(v).unwrap();
    assert_eq!(back.tool_is_error, Some(true));
}

#[test]
fn ids_are_unique_under_rapid_minting() {
    let mut ids = std::collections::HashSet::new();
    for _ in 0..1000 {
        ids.insert(next_message_id());
    }
    assert_eq!(ids.len(), 1000);
}

#[test]
fn session_round_trip() {
    let mut s = ChatSession::new("sess-1");
    s.provider = Some("openai".into());
    s.is_open = false;
    s.messages.push(ChatMessage::user("hello"));
    let json = serde_json::to_string(&s).unwrap();
    let back: ChatSession = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, "sess-1");
    assert_eq!(back.provider.as_deref(), Some("openai"));
    assert!(!back.is_open);
    assert_eq!(back.messages.len(), 1);
    assert_eq!(back.messages[0].role, Role::User);
}

#[test]
fn content_blocks_use_the_tagged_camel_case_wire_contract() {
    let message = ChatMessage::assistant(
        "working",
        vec![ToolCall::request(
            "call-1",
            "split_clip",
            serde_json::json!({"clipId": "c1"}),
        )],
    );

    let value = serde_json::to_value(&message).unwrap();

    assert_eq!(
        value["blocks"][0],
        serde_json::json!({
            "type": "text",
            "text": "working"
        })
    );
    assert_eq!(
        value["blocks"][1],
        serde_json::json!({
            "type": "toolUse",
            "id": "call-1",
            "name": "split_clip",
            "input": {"clipId": "c1"}
        })
    );
}

#[test]
fn blocks_preserve_interleaved_assistant_order_and_round_trip() {
    let blocks = vec![
        AgentContentBlock::Text { text: "A".into() },
        AgentContentBlock::ToolUse {
            id: "call-1".into(),
            name: "split_clip".into(),
            input: serde_json::json!({"clipId": "c1"}),
            result: None,
            is_error: None,
        },
        AgentContentBlock::Text { text: "B".into() },
        AgentContentBlock::ToolUse {
            id: "call-2".into(),
            name: "delete_clip".into(),
            input: serde_json::json!({"clipId": "c2"}),
            result: Some(serde_json::json!({"ok": true})),
            is_error: Some(false),
        },
    ];
    let message = ChatMessage::assistant_blocks_with_id("assistant-ordered", blocks.clone());

    assert_eq!(message.blocks, blocks);
    assert_eq!(message.content, "AB");
    assert_eq!(
        message
            .tool_calls
            .iter()
            .map(|call| call.id.as_str())
            .collect::<Vec<_>>(),
        vec!["call-1", "call-2"]
    );

    let wire = serde_json::to_value(&message).unwrap();
    assert_eq!(wire["id"], "assistant-ordered");
    assert_eq!(wire["blocks"], serde_json::to_value(&blocks).unwrap());
    let restored: ChatMessage = serde_json::from_value(wire).unwrap();
    assert_eq!(restored.blocks, blocks);
}

#[test]
fn blocks_append_text_deltas_only_consolidates_adjacent_text() {
    let mut message = ChatMessage::assistant_blocks_with_id("assistant-stream", Vec::new());

    assert_eq!(message.append_text_delta("A"), 0);
    assert_eq!(message.append_text_delta("1"), 0);
    assert_eq!(
        message.upsert_tool_use(ToolCall::request(
            "call-1",
            "split_clip",
            serde_json::json!({"clipId": "c1"}),
        )),
        1
    );
    assert_eq!(message.append_text_delta("B"), 2);
    assert_eq!(message.append_text_delta("2"), 2);

    assert_eq!(
        message.blocks,
        vec![
            AgentContentBlock::Text { text: "A1".into() },
            AgentContentBlock::ToolUse {
                id: "call-1".into(),
                name: "split_clip".into(),
                input: serde_json::json!({"clipId": "c1"}),
                result: None,
                is_error: None,
            },
            AgentContentBlock::Text { text: "B2".into() },
        ]
    );
    assert_eq!(message.content, "A1B2");
}

#[test]
fn blocks_tool_result_wire_preserves_text_and_image_order() {
    let block = AgentContentBlock::ToolResult {
        tool_use_id: "call-image".into(),
        content: vec![
            Block::text("before"),
            Block::image("aW1hZ2U=", "image/png"),
            Block::text("after"),
        ],
        is_error: Some(true),
    };

    let wire = serde_json::to_value(&block).unwrap();
    assert_eq!(
        wire,
        serde_json::json!({
            "type": "toolResult",
            "toolUseId": "call-image",
            "content": [
                {"kind": "text", "text": "before"},
                {"kind": "image", "base64": "aW1hZ2U=", "mediaType": "image/png"},
                {"kind": "text", "text": "after"}
            ],
            "isError": true
        })
    );
    let restored: AgentContentBlock = serde_json::from_value(wire).unwrap();
    assert_eq!(restored, block);
}

#[test]
fn blocks_refresh_legacy_fields_is_one_way_and_keeps_block_order() {
    let original_blocks = vec![
        AgentContentBlock::Text { text: "A".into() },
        AgentContentBlock::ToolUse {
            id: "call-1".into(),
            name: "split_clip".into(),
            input: serde_json::json!({"clipId": "c1"}),
            result: None,
            is_error: None,
        },
        AgentContentBlock::Text { text: "B".into() },
    ];
    let mut message =
        ChatMessage::assistant_blocks_with_id("assistant-authoritative", original_blocks.clone());
    message.content = "stale legacy text".into();
    message.tool_calls.clear();

    message.refresh_legacy_fields();

    assert_eq!(message.blocks, original_blocks);
    assert_eq!(message.content, "AB");
    assert_eq!(message.tool_calls.len(), 1);
    assert_eq!(message.tool_calls[0].id, "call-1");
}

#[test]
fn blocks_legacy_flat_messages_migrate_to_stable_text_then_tool_order() {
    let legacy = serde_json::json!({
        "id": "legacy-ordered",
        "role": "assistant",
        "content": "working",
        "toolCalls": [
            {"id": "call-1", "name": "split_clip", "args": {"clipId": "c1"}},
            {"id": "call-2", "name": "delete_clip", "args": {"clipId": "c2"}}
        ],
        "createdAt": 1
    });

    let message: ChatMessage = serde_json::from_value(legacy).unwrap();

    assert!(matches!(
        &message.blocks[..],
        [
            AgentContentBlock::Text { text },
            AgentContentBlock::ToolUse { id: first, .. },
            AgentContentBlock::ToolUse { id: second, .. }
        ] if text == "working" && first == "call-1" && second == "call-2"
    ));
}

#[test]
fn blocks_explicit_empty_array_is_serialized_for_beta5_messages() {
    let message = ChatMessage::assistant_blocks_with_id("assistant-empty", Vec::new());

    let wire = serde_json::to_value(message).unwrap();

    assert_eq!(wire["blocks"], serde_json::json!([]));
}

#[test]
fn blocks_explicit_empty_array_wins_over_stale_legacy_fields() {
    let wire = serde_json::json!({
        "id": "assistant-empty",
        "role": "assistant",
        "content": "stale text",
        "toolCalls": [{
            "id": "stale-call",
            "name": "split_clip",
            "args": {"clipId": "stale"}
        }],
        "blocks": [],
        "createdAt": 1
    });

    let message: ChatMessage = serde_json::from_value(wire).unwrap();

    assert!(message.blocks.is_empty());
    assert!(message.content.is_empty());
    assert!(message.tool_calls.is_empty());
}

#[test]
fn blocks_explicit_empty_tool_message_clears_stale_tool_metadata() {
    let wire = serde_json::json!({
        "id": "tool-empty",
        "role": "tool",
        "content": "stale result",
        "toolCalls": [],
        "blocks": [],
        "createdAt": 1,
        "toolCallId": "stale-call",
        "toolIsError": true
    });

    let message: ChatMessage = serde_json::from_value(wire).unwrap();

    assert!(message.blocks.is_empty());
    assert!(message.content.is_empty());
    assert_eq!(message.tool_call_id, None);
    assert_eq!(message.tool_is_error, None);
}

#[test]
fn legacy_flat_messages_migrate_to_content_blocks() {
    let legacy = serde_json::json!({
        "id": "legacy-1",
        "role": "assistant",
        "content": "working",
        "toolCalls": [{
            "id": "call-1",
            "name": "split_clip",
            "args": {"clipId": "c1"},
            "result": {"ok": true},
            "isError": false
        }],
        "createdAt": 1
    });

    let message: ChatMessage = serde_json::from_value(legacy).unwrap();

    assert_eq!(message.blocks.len(), 2);
    assert!(matches!(
        &message.blocks[0],
        AgentContentBlock::Text { text } if text == "working"
    ));
    assert!(matches!(
        &message.blocks[1],
        AgentContentBlock::ToolUse { id, is_error, .. }
            if id == "call-1" && *is_error == Some(false)
    ));
}

#[test]
fn legacy_sessions_without_is_open_default_to_open() {
    let legacy = serde_json::json!({
        "id": "legacy-session",
        "messages": [],
        "createdAt": 1
    });

    let session: ChatSession = serde_json::from_value(legacy).unwrap();

    assert!(session.is_open);
}

#[test]
fn native_tool_result_blocks_round_trip_images_in_order() {
    use crate::tools::result::Block;

    let message = ChatMessage::tool_result_blocks(
        "call-image",
        vec![
            Block::text("before"),
            Block::image("aW1hZ2U=", "image/png"),
            Block::text("after"),
        ],
        serde_json::json!({"summary": "beforeafter", "isError": false}),
        false,
    );

    let json = serde_json::to_string(&message).unwrap();
    let restored: ChatMessage = serde_json::from_str(&json).unwrap();

    let AgentContentBlock::ToolResult { content, .. } = &restored.blocks[0] else {
        panic!("expected a native tool result block");
    };
    assert_eq!(
        content,
        &vec![
            Block::text("before"),
            Block::image("aW1hZ2U=", "image/png"),
            Block::text("after"),
        ]
    );
    assert_eq!(
        restored.content,
        serde_json::json!({"summary": "beforeafter", "isError": false}).to_string()
    );
}
