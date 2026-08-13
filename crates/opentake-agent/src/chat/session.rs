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
    #[serde(default)]
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
        let text = text.into();
        let mut message = ChatMessage {
            id: next_message_id(),
            role: Role::User,
            content: String::new(),
            tool_calls: Vec::new(),
            blocks: (!text.is_empty())
                .then_some(AgentContentBlock::Text { text })
                .into_iter()
                .collect(),
            created_at: now_millis(),
            tool_call_id: None,
            tool_is_error: None,
        };
        message.refresh_legacy_fields();
        message
    }

    pub fn assistant(text: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self::assistant_with_id(next_message_id(), text, tool_calls)
    }

    pub fn assistant_with_id(
        id: impl Into<String>,
        text: impl Into<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        let text = text.into();
        let mut blocks = Vec::with_capacity(usize::from(!text.is_empty()) + tool_calls.len());
        if !text.is_empty() {
            blocks.push(AgentContentBlock::Text { text });
        }
        blocks.extend(tool_calls.into_iter().map(AgentContentBlock::from));
        Self::assistant_blocks_with_id(id, blocks)
    }

    pub fn assistant_blocks(blocks: Vec<AgentContentBlock>) -> Self {
        Self::assistant_blocks_with_id(next_message_id(), blocks)
    }

    pub fn assistant_blocks_with_id(id: impl Into<String>, blocks: Vec<AgentContentBlock>) -> Self {
        let mut message = ChatMessage {
            id: id.into(),
            role: Role::Assistant,
            content: String::new(),
            tool_calls: Vec::new(),
            blocks,
            created_at: now_millis(),
            tool_call_id: None,
            tool_is_error: None,
        };
        message.refresh_legacy_fields();
        message
    }

    pub fn system(text: impl Into<String>) -> Self {
        let text = text.into();
        let mut message = ChatMessage {
            id: next_message_id(),
            role: Role::System,
            content: String::new(),
            tool_calls: Vec::new(),
            blocks: (!text.is_empty())
                .then_some(AgentContentBlock::Text { text })
                .into_iter()
                .collect(),
            created_at: now_millis(),
            tool_call_id: None,
            tool_is_error: None,
        };
        message.refresh_legacy_fields();
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
            id: next_message_id(),
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

    /// Append a streamed text chunk and return the authoritative block index.
    /// Only an adjacent text block is consolidated; text separated by a tool
    /// event remains a distinct block.
    pub fn append_text_delta(&mut self, delta: impl AsRef<str>) -> usize {
        let delta = delta.as_ref();
        let block_index = match self.blocks.last_mut() {
            Some(AgentContentBlock::Text { text }) => {
                text.push_str(delta);
                self.blocks.len() - 1
            }
            _ => {
                self.blocks.push(AgentContentBlock::Text {
                    text: delta.to_string(),
                });
                self.blocks.len() - 1
            }
        };
        self.refresh_legacy_fields();
        block_index
    }

    /// Append a text delta to one provider-addressed block. A new block may be
    /// created only at the current tail; gaps or type mismatches are rejected.
    pub fn append_text_delta_at(&mut self, block_index: usize, delta: impl AsRef<str>) -> bool {
        let delta = delta.as_ref();
        let applied = if block_index == self.blocks.len() {
            self.blocks.push(AgentContentBlock::Text {
                text: delta.to_string(),
            });
            true
        } else if let Some(AgentContentBlock::Text { text }) = self.blocks.get_mut(block_index) {
            text.push_str(delta);
            true
        } else {
            false
        };
        if applied {
            self.refresh_legacy_fields();
        }
        applied
    }

    /// Insert or replace one provider-addressed block without changing its
    /// position. Provider indices must be contiguous.
    pub fn upsert_block_at(&mut self, block_index: usize, block: AgentContentBlock) -> bool {
        let applied = if block_index == self.blocks.len() {
            self.blocks.push(block);
            true
        } else if let Some(existing) = self.blocks.get_mut(block_index) {
            *existing = block;
            true
        } else {
            false
        };
        if applied {
            self.refresh_legacy_fields();
        }
        applied
    }

    /// Insert a tool request in event order, or update the already-addressed
    /// block when dispatch later fills its result.
    pub fn upsert_tool_use(&mut self, tool_call: ToolCall) -> usize {
        if let Some((index, block)) = self.blocks.iter_mut().enumerate().find(|(_, block)| {
            matches!(block, AgentContentBlock::ToolUse { id, .. } if id == &tool_call.id)
        }) {
            *block = AgentContentBlock::from(tool_call);
            self.refresh_legacy_fields();
            return index;
        }
        self.blocks.push(AgentContentBlock::from(tool_call));
        let index = self.blocks.len() - 1;
        self.refresh_legacy_fields();
        index
    }

    /// Derive temporary flat compatibility fields from authoritative blocks.
    /// This never mutates or reorders `blocks`.
    pub fn refresh_legacy_fields(&mut self) {
        self.content = if self.role == Role::Tool {
            self.blocks
                .iter()
                .find_map(|block| match block {
                    AgentContentBlock::ToolResult {
                        content, is_error, ..
                    } => Some(legacy_tool_result_content(
                        content,
                        is_error.unwrap_or(false),
                    )),
                    _ => None,
                })
                .unwrap_or_default()
        } else {
            self.blocks
                .iter()
                .filter_map(|block| match block {
                    AgentContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>()
        };
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
        self.tool_call_id = None;
        self.tool_is_error = None;
        if self.role == Role::Tool {
            if let Some(AgentContentBlock::ToolResult {
                tool_use_id,
                is_error,
                ..
            }) = self
                .blocks
                .iter()
                .find(|block| matches!(block, AgentContentBlock::ToolResult { .. }))
            {
                self.tool_call_id = Some(tool_use_id.clone());
                self.tool_is_error = *is_error;
            }
        }
    }

    /// Migrate the temporary Beta 4 flat fields when no authoritative blocks
    /// were persisted yet.
    fn migrate_legacy_fields_to_blocks(&mut self) {
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

fn legacy_tool_result_content(content: &[Block], is_error: bool) -> String {
    if let [Block::Text { text }] = content {
        if serde_json::from_str::<serde_json::Value>(text).is_ok() {
            return text.clone();
        }
    }
    let summary = content
        .iter()
        .filter_map(|block| match block {
            Block::Text { text } => Some(text.as_str()),
            Block::Image { .. } => None,
        })
        .collect::<String>();
    serde_json::json!({"summary": summary, "isError": is_error}).to_string()
}

impl From<ToolCall> for AgentContentBlock {
    fn from(tool_call: ToolCall) -> Self {
        AgentContentBlock::ToolUse {
            id: tool_call.id,
            name: tool_call.name,
            input: tool_call.args,
            result: tool_call.result,
            is_error: tool_call.is_error,
        }
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
    blocks: Option<Vec<AgentContentBlock>>,
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
        let has_authoritative_blocks = wire.blocks.is_some();
        let mut message = ChatMessage {
            id: wire.id,
            role: wire.role,
            content: wire.content,
            tool_calls: wire.tool_calls,
            blocks: wire.blocks.unwrap_or_default(),
            created_at: wire.created_at,
            tool_call_id: wire.tool_call_id,
            tool_is_error: wire.tool_is_error,
        };
        if has_authoritative_blocks {
            message.refresh_legacy_fields();
        } else {
            message.migrate_legacy_fields_to_blocks();
        }
        Ok(message)
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

pub fn next_message_id() -> String {
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
mod tests;
