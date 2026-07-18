//! Chat session + message model (`agent-SPEC.md` §5). The in-app chat client
//! shares the same [`crate::mcp::dispatch::Dispatcher`] as the external MCP
//! server, so tool calls land on the SAME single-edit-entry `EditCommand`
//! pipeline. Sessions are per-conversation; history is retained in Rust so the
//! front end holds only a read-only mirror (mirrors the timeline discipline).

use serde::{Deserialize, Serialize};

use crate::tools::result::Block;

/// Conversation role. Wire values match OpenAI/Anthropic chat conventions so
/// the LLM client can serialize a session directly into a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// One tool invocation captured mid-turn. `result` is filled after the dispatch
/// shell runs; until then the message carries the request only (so the UI can
/// render an in-flight card).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// True when the dispatch returned an error (`result` carries the message).
    /// Kept so the UI can style failed calls distinctly without re-parsing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolCall {
    pub fn request(
        id: impl Into<String>,
        name: impl Into<String>,
        args: serde_json::Value,
    ) -> Self {
        ToolCall {
            id: id.into(),
            name: name.into(),
            args,
            result: None,
            is_error: None,
        }
    }
}

/// Structured local chat content. The tagged representation mirrors the
/// upstream Agent model while the legacy flat `content` / `toolCalls` fields on
/// [`ChatMessage`] remain available during the desktop UI migration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(rename = "isError")]
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    ToolResult {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        content: Vec<Block>,
        #[serde(rename = "isError")]
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// One chat message. `tool_calls` is non-empty only for assistant turns that
/// requested tools; `content` may be empty on a pure tool-calling turn.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub role: Role,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<AgentContentBlock>,
    pub created_at: i64,
    /// When role == Tool: the `tool_call_id` this result answers. OpenAI's
    /// tool-result messages require it; absent for other roles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// When role == Tool: true when the provider must treat this result as an
    /// error. Anthropic carries this as `tool_result.is_error`; OpenAI has no
    /// equivalent field and receives the structured error content instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_is_error: Option<bool>,
}

impl ChatMessage {
    pub fn user(text: impl Into<String>) -> Self {
        let mut message = ChatMessage {
            id: next_id(),
            role: Role::User,
            content: text.into(),
            tool_calls: Vec::new(),
            blocks: Vec::new(),
            created_at: now_millis(),
            tool_call_id: None,
            tool_is_error: None,
        };
        message.refresh_blocks();
        message
    }

    pub fn assistant(text: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        let mut message = ChatMessage {
            id: next_id(),
            role: Role::Assistant,
            content: text.into(),
            tool_calls,
            blocks: Vec::new(),
            created_at: now_millis(),
            tool_call_id: None,
            tool_is_error: None,
        };
        message.refresh_blocks();
        message
    }

    pub fn system(text: impl Into<String>) -> Self {
        let mut message = ChatMessage {
            id: next_id(),
            role: Role::System,
            content: text.into(),
            tool_calls: Vec::new(),
            blocks: Vec::new(),
            created_at: now_millis(),
            tool_call_id: None,
            tool_is_error: None,
        };
        message.refresh_blocks();
        message
    }

    /// A tool-result message: the JSON returned by the dispatch shell, answering
    /// a specific `tool_call_id`.
    pub fn tool_result(tool_call_id: impl Into<String>, result: serde_json::Value) -> Self {
        Self::tool_result_blocks(
            tool_call_id,
            vec![Block::text(result.to_string())],
            result,
            false,
        )
    }

    /// A tool-result message retaining the original ordered Text/Image blocks.
    /// `legacy_result` is the fail-closed provider/UI summary kept during the
    /// flat-message compatibility window; native blocks remain authoritative
    /// for persistence and multimodal Anthropic requests.
    pub fn tool_result_blocks(
        tool_call_id: impl Into<String>,
        content: Vec<Block>,
        legacy_result: serde_json::Value,
        is_error: bool,
    ) -> Self {
        let tool_call_id = tool_call_id.into();
        ChatMessage {
            id: next_id(),
            role: Role::Tool,
            content: legacy_result.to_string(),
            tool_calls: Vec::new(),
            blocks: vec![AgentContentBlock::ToolResult {
                tool_use_id: tool_call_id.clone(),
                content,
                is_error: is_error.then_some(true),
            }],
            created_at: now_millis(),
            tool_call_id: Some(tool_call_id),
            tool_is_error: is_error.then_some(true),
        }
    }

    /// A failed tool result. The explicit marker is required by Anthropic's
    /// wire protocol so cancellation repair is not mistaken for success.
    pub fn tool_error_result(tool_call_id: impl Into<String>, result: serde_json::Value) -> Self {
        Self::tool_result_blocks(
            tool_call_id,
            vec![Block::text(result.to_string())],
            result,
            true,
        )
    }

    /// Rebuild the structured representation after an in-place update to the
    /// temporary legacy view fields.
    pub(crate) fn refresh_blocks(&mut self) {
        let mut blocks = Vec::new();
        if !self.content.is_empty() && self.role != Role::Tool {
            blocks.push(AgentContentBlock::Text {
                text: self.content.clone(),
            });
        }
        if self.role == Role::Assistant {
            blocks.extend(
                self.tool_calls
                    .iter()
                    .map(|tool_call| AgentContentBlock::ToolUse {
                        id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        input: tool_call.args.clone(),
                        result: tool_call.result.clone(),
                        is_error: tool_call.is_error,
                    }),
            );
        }
        if self.role == Role::Tool {
            blocks.push(AgentContentBlock::ToolResult {
                tool_use_id: self.tool_call_id.clone().unwrap_or_default(),
                content: vec![Block::text(self.content.clone())],
                is_error: self.tool_is_error,
            });
        }
        self.blocks = blocks;
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessageWire {
    id: String,
    role: Role,
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
    #[serde(default)]
    blocks: Vec<AgentContentBlock>,
    created_at: i64,
    #[serde(default)]
    tool_call_id: Option<String>,
    #[serde(default)]
    tool_is_error: Option<bool>,
}

impl<'de> Deserialize<'de> for ChatMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ChatMessageWire::deserialize(deserializer)?;
        let mut message = ChatMessage {
            id: wire.id,
            role: wire.role,
            content: wire.content,
            tool_calls: wire.tool_calls,
            blocks: wire.blocks,
            created_at: wire.created_at,
            tool_call_id: wire.tool_call_id,
            tool_is_error: wire.tool_is_error,
        };
        if message.blocks.is_empty() {
            message.refresh_blocks();
        } else {
            message.apply_blocks_to_legacy_view();
        }
        Ok(message)
    }
}

impl ChatMessage {
    fn apply_blocks_to_legacy_view(&mut self) {
        if self.role != Role::Tool {
            self.content = self
                .blocks
                .iter()
                .filter_map(|block| match block {
                    AgentContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>();
        }
        if self.role == Role::Assistant {
            self.tool_calls = self
                .blocks
                .iter()
                .filter_map(|block| match block {
                    AgentContentBlock::ToolUse {
                        id,
                        name,
                        input,
                        result,
                        is_error,
                    } => Some(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        args: input.clone(),
                        result: result.clone(),
                        is_error: *is_error,
                    }),
                    _ => None,
                })
                .collect();
        }
        if self.role == Role::Tool {
            if let Some(AgentContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            }) = self
                .blocks
                .iter()
                .find(|block| matches!(block, AgentContentBlock::ToolResult { .. }))
            {
                if self.content.is_empty() {
                    self.content = match content.as_slice() {
                        [Block::Text { text }] => text.clone(),
                        _ => serde_json::to_string(content).unwrap_or_default(),
                    };
                }
                self.tool_call_id = Some(tool_use_id.clone());
                self.tool_is_error = *is_error;
            }
        }
    }
}

/// One conversation: an ordered message log. The chat loop appends user turns,
/// assistant turns (with any tool calls), and tool-result turns; the front end
/// reads the whole list back via `chat_history`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    pub id: String,
    pub messages: Vec<ChatMessage>,
    pub created_at: i64,
    #[serde(default = "default_true")]
    pub is_open: bool,
    /// The provider the last turn used (`openai` / `anthropic` / `google`...).
    /// The session mirrors the user's explicit current choice; unsupported
    /// providers are rejected before any turn runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// The model id last used (display only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl ChatSession {
    pub fn new(id: impl Into<String>) -> Self {
        ChatSession {
            id: id.into(),
            messages: Vec::new(),
            created_at: now_millis(),
            is_open: true,
            provider: None,
            model: None,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Monotonic counter + millis timestamp → a stable, sort-friendly id without
/// pulling a uuid dep into the leaf chat module. Two ids minted in the same
/// millisecond are disambiguated by the counter.
static ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn next_id() -> String {
    let n = ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("m{}-{n}", now_millis())
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
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
            ids.insert(next_id());
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
}
