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

use futures::StreamExt;
use serde::{Deserialize, Serialize};

use opentake_gen::{KeyStore, ProviderKey};

use crate::chat::session::{ChatMessage, Role, ToolCall};

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
    /// A text chunk from the assistant. Concatenated in order = full turn text.
    Delta(String),
    /// The assistant requested a tool call. `ToolCall.result` is `None` here;
    /// the loop dispatches, fills the result, and re-feeds the LLM.
    ToolCall(ToolCall),
}

/// The final assistant turn after the stream closes: full text + any tool calls
/// the model requested this turn.
#[derive(Clone, Debug)]
pub struct TurnResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
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
}

impl From<reqwest::Error> for LlmError {
    fn from(e: reqwest::Error) -> Self {
        LlmError::Network(e.to_string())
    }
}

const OPENAI_URL: &str = "https://api.openai.com/v1/chat/completions";
const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Stream one chat turn. Text deltas and tool-call requests are delivered via
/// `on_event`; the final aggregated turn is returned.
pub async fn stream_chat<F>(
    provider: LlmProvider,
    store: &dyn KeyStore,
    req: ChatRequest<'_>,
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
        LlmProvider::OpenAi => stream_openai(&key, model, req, &mut on_event).await,
        LlmProvider::Anthropic => stream_anthropic(&key, model, req, &mut on_event).await,
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
    match m.role {
        Role::System => serde_json::json!({"role": "system", "content": m.content}),
        Role::User => serde_json::json!({"role": "user", "content": m.content}),
        Role::Assistant => {
            let mut v = serde_json::json!({"role": "assistant", "content": m.content});
            if !m.tool_calls.is_empty() {
                v["tool_calls"] = serde_json::Value::Array(
                    m.tool_calls
                        .iter()
                        .map(|tc| {
                            serde_json::json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.args.to_string(),
                                }
                            })
                        })
                        .collect(),
                );
            }
            v
        }
        Role::Tool => serde_json::json!({
            "role": "tool",
            "content": m.content,
            "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
        }),
    }
}

async fn stream_openai<F>(
    key: &str,
    model: &str,
    req: ChatRequest<'_>,
    on_event: &mut F,
) -> Result<TurnResult, LlmError>
where
    F: FnMut(StreamEvent) + Send,
{
    let body = openai_body(model, req.messages, req.tools);
    let resp = reqwest::Client::new()
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
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| LlmError::Stream(e.to_string()))?;
        buf.push_str(std::str::from_utf8(&bytes).map_err(|e| LlmError::Stream(e.to_string()))?);
        while let Some(idx) = buf.find("\n\n") {
            let event = buf[..idx].to_string();
            buf.drain(..idx + 2);
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
                        on_event(StreamEvent::Delta(text.to_string()));
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

    let mut tool_calls = Vec::new();
    for (_, (id, name, args_str)) in tool_buf {
        let args = if args_str.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&args_str).unwrap_or(serde_json::json!({"_raw": args_str}))
        };
        let tc = ToolCall::request(id, name, args);
        on_event(StreamEvent::ToolCall(tc.clone()));
        tool_calls.push(tc);
    }

    Ok(TurnResult {
        content: full,
        tool_calls,
    })
}

// MARK: - Anthropic streaming

/// Build the Anthropic request body. System prompt is a top-level field (not a
/// message); tool results are `role:user` `tool_result` content blocks.
fn anthropic_body(
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
                system.push_str(&m.content);
            }
            Role::User => turns.push(serde_json::json!({"role": "user", "content": m.content})),
            Role::Assistant => {
                let mut blocks = Vec::new();
                if !m.content.is_empty() {
                    blocks.push(serde_json::json!({"type": "text", "text": m.content}));
                }
                for tc in &m.tool_calls {
                    blocks.push(serde_json::json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": tc.args,
                    }));
                }
                turns.push(serde_json::json!({"role": "assistant", "content": blocks}));
            }
            Role::Tool => {
                let block = serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                    "content": m.content,
                });
                if let Some(last) = turns.last_mut() {
                    if last.get("role").and_then(|r| r.as_str()) == Some("user") {
                        if let Some(arr) = last.get_mut("content").and_then(|c| c.as_array_mut()) {
                            arr.push(block);
                            continue;
                        }
                    }
                }
                turns.push(serde_json::json!({"role": "user", "content": vec![block]}));
            }
        }
    }

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "stream": true,
        "messages": turns,
    });
    if !system.is_empty() {
        body["system"] = serde_json::Value::String(system);
    }
    if !tools.is_empty() {
        body["tools"] = serde_json::Value::Array(
            tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect(),
        );
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

async fn stream_anthropic<F>(
    key: &str,
    model: &str,
    req: ChatRequest<'_>,
    on_event: &mut F,
) -> Result<TurnResult, LlmError>
where
    F: FnMut(StreamEvent) + Send,
{
    let body = anthropic_body(model, req.messages, req.tools);
    let resp = reqwest::Client::new()
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

    let mut full = String::new();
    let mut tool_buf: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| LlmError::Stream(e.to_string()))?;
        buf.push_str(std::str::from_utf8(&bytes).map_err(|e| LlmError::Stream(e.to_string()))?);
        while let Some(idx) = buf.find("\n\n") {
            let event = buf[..idx].to_string();
            buf.drain(..idx + 2);
            let data_line = event
                .lines()
                .find_map(|l| l.strip_prefix("data: "))
                .unwrap_or("");
            if data_line.is_empty() {
                continue;
            }
            let ev: AnthEvent = match serde_json::from_str(data_line) {
                Ok(e) => e,
                Err(_) => continue,
            };
            match ev.typ.as_str() {
                "content_block_start" => {
                    if let Some(cb) = ev.content_block {
                        if cb.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            let i = ev.index.unwrap_or(0) as usize;
                            let id = cb
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let name = cb
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            tool_buf.insert(i, (id, name, String::new()));
                        }
                    }
                }
                "content_block_delta" => {
                    let Some(delta) = ev.delta else { continue };
                    let i = ev.index.unwrap_or(0) as usize;
                    match delta.get("type").and_then(|t| t.as_str()) {
                        Some("text_delta") => {
                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                if !text.is_empty() {
                                    full.push_str(text);
                                    on_event(StreamEvent::Delta(text.to_string()));
                                }
                            }
                        }
                        Some("input_json_delta") => {
                            if let Some(partial) = delta.get("partial").and_then(|p| p.as_str()) {
                                if let Some(entry) = tool_buf.get_mut(&i) {
                                    entry.2.push_str(partial);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                "message_stop" => break,
                _ => {}
            }
        }
    }

    let mut tool_calls = Vec::new();
    for (_, (id, name, args_str)) in tool_buf {
        let args = if args_str.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&args_str).unwrap_or(serde_json::json!({"_raw": args_str}))
        };
        let tc = ToolCall::request(id, name, args);
        on_event(StreamEvent::ToolCall(tc.clone()));
        tool_calls.push(tc);
    }

    Ok(TurnResult {
        content: full,
        tool_calls,
    })
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
        assert_eq!(body["system"], "you are an editor");
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
        assert_eq!(body["messages"][0]["role"], "user");
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
            ChatMessage::tool_result("c1", serde_json::json!({"fps": 30})),
        ];
        let body = anthropic_body("claude", &msgs, &[]);
        let turns = body["messages"].as_array().unwrap();
        assert_eq!(turns.len(), 3);
        let last = turns.last().unwrap();
        assert_eq!(last["role"], "user");
        assert_eq!(last["content"][0]["type"], "tool_result");
        assert_eq!(last["content"][0]["tool_use_id"], "c1");
    }
}
