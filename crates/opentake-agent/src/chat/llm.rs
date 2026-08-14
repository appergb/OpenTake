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

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::{Stream, StreamExt};
use serde::Serialize;

use opentake_gen::{KeyStore, ProviderKey};

use crate::chat::session::{AgentContentBlock, ChatMessage, ToolCall};

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
        LlmProvider::OpenAi => openai::stream(&key, model, req, cancel, &mut on_event).await,
        LlmProvider::Anthropic => anthropic::stream(&key, model, req, cancel, &mut on_event).await,
    }
}

#[cfg(test)]
pub(crate) use anthropic::{body as anthropic_body, StreamDecoder as AnthropicStreamDecoder};

mod anthropic;
mod openai;

#[cfg(test)]
mod tests;
