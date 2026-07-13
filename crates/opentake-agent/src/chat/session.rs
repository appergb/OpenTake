//! Chat session + message model (`agent-SPEC.md` §5). The in-app chat client
//! shares the same [`crate::mcp::dispatch::Dispatcher`] as the external MCP
//! server, so tool calls land on the SAME single-edit-entry `EditCommand`
//! pipeline. Sessions are per-conversation; history is retained in Rust so the
//! front end holds only a read-only mirror (mirrors the timeline discipline).

use serde::{Deserialize, Serialize};

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

/// One chat message. `tool_calls` is non-empty only for assistant turns that
/// requested tools; `content` may be empty on a pure tool-calling turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub role: Role,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    pub created_at: i64,
    /// When role == Tool: the `tool_call_id` this result answers. OpenAI's
    /// tool-result messages require it; absent for other roles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn user(text: impl Into<String>) -> Self {
        ChatMessage {
            id: next_id(),
            role: Role::User,
            content: text.into(),
            tool_calls: Vec::new(),
            created_at: now_millis(),
            tool_call_id: None,
        }
    }

    pub fn assistant(text: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        ChatMessage {
            id: next_id(),
            role: Role::Assistant,
            content: text.into(),
            tool_calls,
            created_at: now_millis(),
            tool_call_id: None,
        }
    }

    pub fn system(text: impl Into<String>) -> Self {
        ChatMessage {
            id: next_id(),
            role: Role::System,
            content: text.into(),
            tool_calls: Vec::new(),
            created_at: now_millis(),
            tool_call_id: None,
        }
    }

    /// A tool-result message: the JSON returned by the dispatch shell, answering
    /// a specific `tool_call_id`.
    pub fn tool_result(tool_call_id: impl Into<String>, result: serde_json::Value) -> Self {
        ChatMessage {
            id: next_id(),
            role: Role::Tool,
            content: result.to_string(),
            tool_calls: Vec::new(),
            created_at: now_millis(),
            tool_call_id: Some(tool_call_id.into()),
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
            provider: None,
            model: None,
        }
    }
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
        assert!(v["content"].as_str().unwrap().contains("summary"));
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
        s.messages.push(ChatMessage::user("hello"));
        let json = serde_json::to_string(&s).unwrap();
        let back: ChatSession = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "sess-1");
        assert_eq!(back.provider.as_deref(), Some("openai"));
        assert_eq!(back.messages.len(), 1);
        assert_eq!(back.messages[0].role, Role::User);
    }
}
