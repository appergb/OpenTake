//! LLM chat client (`agent-SPEC.md` §5.2). BYOK over the existing key-store
//! boundary — the same OS keychain the Tauri `secret_*` commands write to. Two
//! providers are supported in this phase:
//!
//! - **OpenAI**: `POST /v1/chat/completions` with `stream:true`, standard SSE
//!   `data:` chunks, `choices[0].delta.content` for text and
//!   `choices[0].delta.tool_calls` for tool requests.
//! - **Anthropic**: `POST /v1/messages` with `stream:true`,
//!   `content_block_delta` / `content_block_start` events.
//!
//! Provider choice is explicit: the desktop settings already persist a chosen
//! provider, so chat must either use that provider or fail clearly. No
//! auto-fallback to another provider when the selected one lacks a key.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use opentake_gen::{KeyStore, ProviderKey};

use crate::chat::session::{AgentContentBlock, ChatMessage, Role, ToolCall};
use crate::tools::result::Block;

/// Which BYOK provider a chat session talks to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LlmProvider {
    OpenAi,
    Anthropic,
}

impl LlmProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            LlmProvider::OpenAi => "openai",
            LlmProvider::Anthropic => "anthropic",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            LlmProvider::OpenAi => "OpenAI",
            LlmProvider::Anthropic => "Anthropic",
        }
    }

    pub(crate) fn key(self) -> ProviderKey {
        match self {
            LlmProvider::OpenAi => ProviderKey::OpenAI,
            LlmProvider::Anthropic => ProviderKey::Anthropic,
        }
    }

    /// Default model id per provider (P1; a model picker is a follow-up).
    pub fn default_model(self) -> &'static str {
        match self {
            LlmProvider::OpenAi => "gpt-4o-mini",
            LlmProvider::Anthropic => "claude-3-5-haiku-latest",
        }
    }
}

/// Resolve the explicit provider string chosen in Settings.
pub fn provider_from_choice(choice: &str) -> Result<LlmProvider, LlmError> {
    match choice {
        "openai" => Ok(LlmProvider::OpenAi),
        "anthropic" => Ok(LlmProvider::Anthropic),
        "google" => Err(LlmError::UnsupportedProvider(choice.to_string())),
        other => Err(LlmError::UnknownProvider(other.to_string())),
    }
}

/// The guided message shown when the selected provider has no BYOK key yet.
pub fn no_key_guide(provider: LlmProvider) -> String {
    format!(
        "I can't connect to {} yet — no API key is configured. Open Settings → AI and add a {} key, then send your message again.",
        provider.display_name(),
        provider.display_name()
    )
}

/// A tool schema in the OpenAI function-calling shape (`name` + `description` +
/// `parameters` JSON Schema). Built from [`crate::tools::names::ToolName`] by
/// the chat loop; the wire form here is exactly what OpenAI/Anthropic expect.
#[derive(Clone, Debug, Serialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Events the streaming client emits mid-turn. The chat loop forwards each to a
/// matching Tauri event so the UI renders incrementally.
#[derive(Clone, Debug)]
pub enum StreamEvent {
    /// A text chunk addressed to the provider's content-block index.
    BlockDelta { block_index: usize, delta: String },
    /// Insert or replace one provider-addressed content block.
    BlockUpsert {
        block_index: usize,
        block: AgentContentBlock,
    },
}

/// The final assistant turn after the stream closes: full text + any tool calls
/// the model requested this turn.
#[derive(Clone, Debug)]
pub struct TurnResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub blocks: Vec<AgentContentBlock>,
}

/// Everything a chat turn needs: the full history (system + user + assistant +
/// tool results) and the tool catalog. Built by the chat loop.
pub struct ChatRequest<'a> {
    pub messages: &'a [ChatMessage],
    pub tools: &'a [ToolSchema],
    pub model: Option<&'a str>,
}

/// Lightweight error: the loop turns any failure into a single error string the
/// UI can surface (Tauri commands map `Err(String)`).
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error(
        "chat does not support provider `{0}` yet; choose OpenAI or Anthropic in Settings → AI"
    )]
    UnsupportedProvider(String),
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("no API key configured for {0}; open Settings → AI to add one")]
    NoKey(&'static str),
    #[error("network error: {0}")]
    Network(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("bad stream: {0}")]
    Stream(String),
    #[error("cancelled")]
    Cancelled,
}

impl From<reqwest::Error> for LlmError {
    fn from(e: reqwest::Error) -> Self {
        LlmError::Network(e.to_string())
    }
}

const OPENAI_URL: &str = "https://api.openai.com/v1/chat/completions";
const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(100);

fn http_client() -> Result<reqwest::Client, LlmError> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| LlmError::Network(e.to_string()))
}

fn take_sse_frame(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let end = buffer.windows(2).position(|window| window == b"\n\n")?;
    let frame = buffer.drain(..end).collect::<Vec<u8>>();
    buffer.drain(..2);
    Some(frame)
}

fn drain_sse_frames(buffer: &mut Vec<u8>) -> Result<Vec<String>, LlmError> {
    let mut frames = Vec::new();
    while let Some(frame) = take_sse_frame(buffer) {
        frames.push(String::from_utf8(frame).map_err(|e| LlmError::Stream(e.to_string()))?);
    }
    Ok(frames)
}

async fn next_chunk_or_cancel<S, B, E>(
    stream: &mut S,
    cancel: &AtomicBool,
) -> Result<Option<Result<B, E>>, LlmError>
where
    S: Stream<Item = Result<B, E>> + Unpin,
{
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(LlmError::Cancelled);
        }
        tokio::select! {
            chunk = stream.next() => return Ok(chunk),
            _ = tokio::time::sleep(CANCEL_POLL_INTERVAL) => {}
        }
    }
}

/// Stream one chat turn. Text deltas and tool-call requests are delivered via
/// `on_event`; the final aggregated turn is returned.
pub async fn stream_chat<F>(
    provider: LlmProvider,
    store: &dyn KeyStore,
    req: ChatRequest<'_>,
    cancel: &AtomicBool,
    mut on_event: F,
) -> Result<TurnResult, LlmError>
where
    F: FnMut(StreamEvent) + Send,
{
    let key = store
        .load(provider.key().account())
        .map_err(|e| LlmError::Network(format!("keychain: {e}")))?
        .ok_or(LlmError::NoKey(provider.as_str()))?;

    let model = req.model.unwrap_or_else(|| provider.default_model());
    match provider {
        LlmProvider::OpenAi => stream_openai(&key, model, req, cancel, &mut on_event).await,
        LlmProvider::Anthropic => stream_anthropic(&key, model, req, cancel, &mut on_event).await,
    }
}

// MARK: - OpenAI streaming

/// Build the OpenAI request body from the session messages + tool schemas.
fn openai_body(model: &str, messages: &[ChatMessage], tools: &[ToolSchema]) -> serde_json::Value {
    let msgs: Vec<serde_json::Value> = messages.iter().map(openai_message).collect();
    let mut body = serde_json::json!({
        "model": model,
        "messages": msgs,
        "stream": true,
    });
    if !tools.is_empty() {
        body["tools"] = serde_json::Value::Array(
            tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect(),
        );
    }
    body
}

/// Map one [`ChatMessage`] to the OpenAI wire shape. Tool-result messages carry
/// `tool_call_id`; assistant turns with tool calls carry `tool_calls`.
fn openai_message(m: &ChatMessage) -> serde_json::Value {
    let text = m
        .blocks
        .iter()
        .filter_map(|block| match block {
            AgentContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>();
    match m.role {
        Role::System => serde_json::json!({"role": "system", "content": text}),
        Role::User => serde_json::json!({"role": "user", "content": text}),
        Role::Assistant => {
            let tool_calls = m
                .blocks
                .iter()
                .filter_map(|block| match block {
                    AgentContentBlock::ToolUse {
                        id, name, input, ..
                    } => Some(serde_json::json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": input.to_string(),
                        }
                    })),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut v = serde_json::json!({"role": "assistant", "content": text});
            if !tool_calls.is_empty() {
                v["tool_calls"] = serde_json::Value::Array(tool_calls);
            }
            v
        }
        Role::Tool => {
            let (tool_call_id, content) = m
                .blocks
                .iter()
                .find_map(|block| match block {
                    AgentContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => Some((
                        tool_use_id.clone(),
                        legacy_tool_result_content(content, is_error.unwrap_or(false)),
                    )),
                    _ => None,
                })
                .unwrap_or_default();
            serde_json::json!({
                "role": "tool",
                "content": content,
                "tool_call_id": tool_call_id,
            })
        }
    }
}

async fn stream_openai<F>(
    key: &str,
    model: &str,
    req: ChatRequest<'_>,
    cancel: &AtomicBool,
    on_event: &mut F,
) -> Result<TurnResult, LlmError>
where
    F: FnMut(StreamEvent) + Send,
{
    let body = openai_body(model, req.messages, req.tools);
    let resp = http_client()?
        .post(OPENAI_URL)
        .bearer_auth(key)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(LlmError::Provider(format!("HTTP {status}: {text}")));
    }

    let mut full = String::new();
    let mut tool_buf: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk) = next_chunk_or_cancel(&mut stream, cancel).await? {
        let bytes = chunk.map_err(|e| LlmError::Stream(e.to_string()))?;
        buf.extend_from_slice(bytes.as_ref());
        for event in drain_sse_frames(&mut buf)? {
            if let Some(rest) = event.strip_prefix("data: ") {
                let rest = rest.trim();
                if rest == "[DONE]" {
                    continue;
                }
                let v: serde_json::Value = match serde_json::from_str(rest) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let Some(delta) = v
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("delta"))
                else {
                    continue;
                };
                if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
                    if !text.is_empty() {
                        full.push_str(text);
                        on_event(StreamEvent::BlockDelta {
                            block_index: 0,
                            delta: text.to_string(),
                        });
                    }
                }
                if let Some(arr) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                    for tc in arr {
                        let idx = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        let entry = tool_buf
                            .entry(idx)
                            .or_insert_with(|| (String::new(), String::new(), String::new()));
                        if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                            entry.0 = id.to_string();
                        }
                        if let Some(fn_obj) = tc.get("function") {
                            if let Some(name) = fn_obj.get("name").and_then(|n| n.as_str()) {
                                entry.1 = name.to_string();
                            }
                            if let Some(args) = fn_obj.get("arguments").and_then(|a| a.as_str()) {
                                entry.2.push_str(args);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut blocks = (!full.is_empty())
        .then(|| AgentContentBlock::Text { text: full.clone() })
        .into_iter()
        .collect::<Vec<_>>();
    let mut tool_calls = Vec::new();
    if cancel.load(Ordering::Relaxed) {
        return Err(LlmError::Cancelled);
    }
    for (index, (id, name, args_str)) in tool_buf {
        let args = if args_str.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&args_str).unwrap_or(serde_json::json!({"_raw": args_str}))
        };
        let tc = ToolCall::request(id, name, args);
        let block_index = blocks.len();
        debug_assert_eq!(block_index, index + usize::from(!full.is_empty()));
        let block = AgentContentBlock::from(tc.clone());
        on_event(StreamEvent::BlockUpsert {
            block_index,
            block: block.clone(),
        });
        blocks.push(block);
        tool_calls.push(tc);
    }

    Ok(TurnResult {
        content: full,
        tool_calls,
        blocks,
    })
}

// MARK: - Anthropic streaming

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

fn anthropic_tool_result_content(content: &[Block]) -> serde_json::Value {
    serde_json::Value::Array(
        content
            .iter()
            .map(|block| match block {
                Block::Text { text } => serde_json::json!({
                    "type": "text",
                    "text": text,
                }),
                Block::Image { base64, media_type } => serde_json::json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": media_type,
                        "data": base64,
                    },
                }),
            })
            .collect(),
    )
}

/// Build the Anthropic request body. System prompt is a top-level field (not a
/// message); tool results are `role:user` `tool_result` content blocks.
pub(crate) fn anthropic_body(
    model: &str,
    messages: &[ChatMessage],
    tools: &[ToolSchema],
) -> serde_json::Value {
    let mut system = String::new();
    let mut turns: Vec<serde_json::Value> = Vec::new();
    for m in messages {
        match m.role {
            Role::System => {
                if !system.is_empty() {
                    system.push_str("\n\n");
                }
                for block in &m.blocks {
                    if let AgentContentBlock::Text { text } = block {
                        system.push_str(text);
                    }
                }
            }
            Role::User => {
                let blocks = m
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        AgentContentBlock::Text { text } => {
                            Some(serde_json::json!({"type": "text", "text": text}))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                turns.push(serde_json::json!({"role": "user", "content": blocks}));
            }
            Role::Assistant => {
                let blocks = m
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        AgentContentBlock::Text { text } => {
                            Some(serde_json::json!({"type": "text", "text": text}))
                        }
                        AgentContentBlock::ToolUse {
                            id, name, input, ..
                        } => Some(serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input,
                        })),
                        AgentContentBlock::ToolResult { .. } => None,
                    })
                    .collect::<Vec<_>>();
                turns.push(serde_json::json!({"role": "assistant", "content": blocks}));
            }
            Role::Tool => {
                for result in &m.blocks {
                    let AgentContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } = result
                    else {
                        continue;
                    };
                    let mut block = serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": anthropic_tool_result_content(content),
                    });
                    if let Some(is_error) = is_error {
                        block["is_error"] = serde_json::Value::Bool(*is_error);
                    }
                    if let Some(last) = turns.last_mut() {
                        if last.get("role").and_then(|r| r.as_str()) == Some("user") {
                            if let Some(arr) =
                                last.get_mut("content").and_then(|c| c.as_array_mut())
                            {
                                arr.push(block);
                                continue;
                            }
                        }
                    }
                    turns.push(serde_json::json!({"role": "user", "content": vec![block]}));
                }
            }
        }
    }

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": 8192,
        "stream": true,
        "messages": turns,
    });
    if !system.is_empty() {
        body["system"] = serde_json::json!([{
            "type": "text",
            "text": system,
            "cache_control": {"type": "ephemeral"},
        }]);
    }
    if !tools.is_empty() {
        let mut wire_tools: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect();
        if let Some(last) = wire_tools.last_mut() {
            last["cache_control"] = serde_json::json!({"type": "ephemeral"});
        }
        body["tools"] = serde_json::Value::Array(wire_tools);
    }
    if let Some(last_block) = body["messages"]
        .as_array_mut()
        .and_then(|messages| messages.last_mut())
        .and_then(|message| message.get_mut("content"))
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|content| content.last_mut())
    {
        last_block["cache_control"] = serde_json::json!({"type": "ephemeral"});
    }
    body
}

#[derive(Deserialize)]
struct AnthEvent {
    #[serde(rename = "type")]
    typ: String,
    #[serde(default)]
    delta: Option<serde_json::Value>,
    #[serde(default)]
    content_block: Option<serde_json::Value>,
    #[serde(default)]
    index: Option<u64>,
}

#[derive(Clone, Debug)]
enum AnthropicBlockState {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        initial_input: serde_json::Value,
        partial_json: String,
    },
}

impl AnthropicBlockState {
    fn content_block(&self) -> AgentContentBlock {
        match self {
            Self::Text(text) => AgentContentBlock::Text { text: text.clone() },
            Self::ToolUse {
                id,
                name,
                initial_input,
                partial_json,
            } => {
                let input = if partial_json.is_empty() {
                    initial_input.clone()
                } else {
                    serde_json::from_str(partial_json)
                        .unwrap_or_else(|_| serde_json::json!({"_raw": partial_json}))
                };
                AgentContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input,
                    result: None,
                    is_error: None,
                }
            }
        }
    }
}

/// Stateful Anthropic SSE decoder shared by the live HTTP path and protocol
/// tests. Content-block indices remain authoritative from start through stop.
#[derive(Default)]
pub(crate) struct AnthropicStreamDecoder {
    buffer: Vec<u8>,
    blocks: BTreeMap<usize, AnthropicBlockState>,
    message_stopped: bool,
}

impl AnthropicStreamDecoder {
    pub(crate) fn push_chunk<F>(&mut self, bytes: &[u8], on_event: &mut F) -> Result<bool, LlmError>
    where
        F: FnMut(StreamEvent),
    {
        if self.message_stopped {
            return Ok(true);
        }
        self.buffer.extend_from_slice(bytes);
        for frame in drain_sse_frames(&mut self.buffer)? {
            if self.handle_frame(&frame, on_event)? {
                self.message_stopped = true;
                break;
            }
        }
        Ok(self.message_stopped)
    }

    fn handle_frame<F>(&mut self, frame: &str, on_event: &mut F) -> Result<bool, LlmError>
    where
        F: FnMut(StreamEvent),
    {
        let data_line = frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap_or("");
        if data_line.is_empty() {
            return Ok(false);
        }
        let ev: AnthEvent = match serde_json::from_str(data_line) {
            Ok(event) => event,
            Err(_) => return Ok(false),
        };
        match ev.typ.as_str() {
            "content_block_start" => {
                let index = ev.index.unwrap_or(0) as usize;
                if index != self.blocks.len() {
                    return Err(LlmError::Stream(format!(
                        "non-contiguous Anthropic content block index {index}"
                    )));
                }
                let Some(content_block) = ev.content_block else {
                    return Err(LlmError::Stream(
                        "Anthropic content block start omitted content_block".into(),
                    ));
                };
                let state = match content_block.get("type").and_then(|value| value.as_str()) {
                    Some("text") => AnthropicBlockState::Text(
                        content_block
                            .get("text")
                            .and_then(|value| value.as_str())
                            .unwrap_or("")
                            .to_string(),
                    ),
                    Some("tool_use") => AnthropicBlockState::ToolUse {
                        id: content_block
                            .get("id")
                            .and_then(|value| value.as_str())
                            .unwrap_or("")
                            .to_string(),
                        name: content_block
                            .get("name")
                            .and_then(|value| value.as_str())
                            .unwrap_or("")
                            .to_string(),
                        initial_input: content_block
                            .get("input")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!({})),
                        partial_json: String::new(),
                    },
                    other => {
                        return Err(LlmError::Stream(format!(
                            "unsupported Anthropic content block type {other:?}"
                        )))
                    }
                };
                let block = state.content_block();
                self.blocks.insert(index, state);
                on_event(StreamEvent::BlockUpsert {
                    block_index: index,
                    block,
                });
            }
            "content_block_delta" => {
                let index = ev.index.unwrap_or(0) as usize;
                let Some(delta) = ev.delta else {
                    return Ok(false);
                };
                match delta.get("type").and_then(|value| value.as_str()) {
                    Some("text_delta") => {
                        let text = delta
                            .get("text")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        let Some(AnthropicBlockState::Text(full)) = self.blocks.get_mut(&index)
                        else {
                            return Err(LlmError::Stream(format!(
                                "text delta addressed non-text block {index}"
                            )));
                        };
                        full.push_str(text);
                        if !text.is_empty() {
                            on_event(StreamEvent::BlockDelta {
                                block_index: index,
                                delta: text.to_string(),
                            });
                        }
                    }
                    Some("input_json_delta") => {
                        let partial = delta
                            .get("partial_json")
                            .or_else(|| delta.get("partial"))
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        let Some(AnthropicBlockState::ToolUse { partial_json, .. }) =
                            self.blocks.get_mut(&index)
                        else {
                            return Err(LlmError::Stream(format!(
                                "input delta addressed non-tool block {index}"
                            )));
                        };
                        partial_json.push_str(partial);
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                let index = ev.index.unwrap_or(0) as usize;
                let Some(state) = self.blocks.get(&index) else {
                    return Err(LlmError::Stream(format!(
                        "content block stop addressed missing block {index}"
                    )));
                };
                on_event(StreamEvent::BlockUpsert {
                    block_index: index,
                    block: state.content_block(),
                });
            }
            "message_stop" => return Ok(true),
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn finish(self) -> Result<TurnResult, LlmError> {
        if !self.buffer.iter().all(u8::is_ascii_whitespace) {
            return Err(LlmError::Stream(
                "Anthropic stream ended with an incomplete SSE frame".into(),
            ));
        }
        let blocks = self
            .blocks
            .into_values()
            .map(|state| state.content_block())
            .collect::<Vec<_>>();
        let content = blocks
            .iter()
            .filter_map(|block| match block {
                AgentContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        let tool_calls = blocks
            .iter()
            .filter_map(|block| match block {
                AgentContentBlock::ToolUse {
                    id, name, input, ..
                } => Some(ToolCall::request(id, name, input.clone())),
                _ => None,
            })
            .collect();
        Ok(TurnResult {
            content,
            tool_calls,
            blocks,
        })
    }
}

async fn stream_anthropic<F>(
    key: &str,
    model: &str,
    req: ChatRequest<'_>,
    cancel: &AtomicBool,
    on_event: &mut F,
) -> Result<TurnResult, LlmError>
where
    F: FnMut(StreamEvent) + Send,
{
    let body = anthropic_body(model, req.messages, req.tools);
    let resp = http_client()?
        .post(ANTHROPIC_URL)
        .header("x-api-key", key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(LlmError::Provider(format!("HTTP {status}: {text}")));
    }

    let mut stream = resp.bytes_stream();
    let mut decoder = AnthropicStreamDecoder::default();
    while let Some(chunk) = next_chunk_or_cancel(&mut stream, cancel).await? {
        let bytes = chunk.map_err(|e| LlmError::Stream(e.to_string()))?;
        if decoder.push_chunk(bytes.as_ref(), on_event)? {
            break;
        }
    }

    if cancel.load(Ordering::Relaxed) {
        return Err(LlmError::Cancelled);
    }
    decoder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
