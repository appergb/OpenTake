use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;

use crate::chat::session::{AgentContentBlock, ChatMessage, Role, ToolCall};
use crate::tools::result::Block;

use super::{
    drain_sse_frames, http_client, next_chunk_or_cancel, ChatRequest, LlmError, StreamEvent,
    ToolSchema, TurnResult,
};

const URL: &str = "https://api.anthropic.com/v1/messages";
const VERSION: &str = "2023-06-01";

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
pub(crate) fn body(
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

#[derive(Clone, Debug)]
struct AnthropicBlockLifecycle {
    state: AnthropicBlockState,
    stopped: bool,
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
pub(crate) struct StreamDecoder {
    buffer: Vec<u8>,
    blocks: BTreeMap<usize, AnthropicBlockLifecycle>,
    message_stopped: bool,
}

impl StreamDecoder {
    pub(crate) fn push_chunk<F>(&mut self, bytes: &[u8], on_event: &mut F) -> Result<bool, LlmError>
    where
        F: FnMut(StreamEvent),
    {
        self.buffer.extend_from_slice(bytes);
        for frame in drain_sse_frames(&mut self.buffer)? {
            self.handle_frame(&frame, on_event)?;
        }
        Ok(self.message_stopped)
    }

    fn handle_frame<F>(&mut self, frame: &str, on_event: &mut F) -> Result<(), LlmError>
    where
        F: FnMut(StreamEvent),
    {
        let data_line = frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap_or("");
        if data_line.is_empty() {
            return Ok(());
        }
        let ev: AnthEvent = match serde_json::from_str(data_line) {
            Ok(event) => event,
            Err(_) => return Ok(()),
        };
        if self.message_stopped {
            let detail = if ev.typ == "message_stop" {
                "message_stop more than once".to_string()
            } else {
                format!("{} after message_stop", ev.typ)
            };
            return Err(LlmError::Stream(detail));
        }
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
                self.blocks.insert(
                    index,
                    AnthropicBlockLifecycle {
                        state,
                        stopped: false,
                    },
                );
                on_event(StreamEvent::BlockUpsert {
                    block_index: index,
                    block,
                });
            }
            "content_block_delta" => {
                let index = ev.index.unwrap_or(0) as usize;
                let Some(delta) = ev.delta else {
                    return Ok(());
                };
                let Some(lifecycle) = self.blocks.get_mut(&index) else {
                    return Err(LlmError::Stream(format!(
                        "delta addressed missing content block {index}"
                    )));
                };
                if lifecycle.stopped {
                    return Err(LlmError::Stream(format!(
                        "delta after content block {index} stopped"
                    )));
                }
                match delta.get("type").and_then(|value| value.as_str()) {
                    Some("text_delta") => {
                        let text = delta
                            .get("text")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        let AnthropicBlockState::Text(full) = &mut lifecycle.state else {
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
                        let AnthropicBlockState::ToolUse { partial_json, .. } =
                            &mut lifecycle.state
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
                let Some(lifecycle) = self.blocks.get_mut(&index) else {
                    return Err(LlmError::Stream(format!(
                        "content block stop addressed missing block {index}"
                    )));
                };
                if lifecycle.stopped {
                    return Err(LlmError::Stream(format!(
                        "content block {index} stopped more than once"
                    )));
                }
                lifecycle.stopped = true;
                on_event(StreamEvent::BlockUpsert {
                    block_index: index,
                    block: lifecycle.state.content_block(),
                });
            }
            "message_stop" => {
                if let Some((index, _)) = self.blocks.iter().find(|(_, block)| !block.stopped) {
                    return Err(LlmError::Stream(format!(
                        "message_stop before content block {index} stopped"
                    )));
                }
                self.message_stopped = true;
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<TurnResult, LlmError> {
        if !self.buffer.iter().all(u8::is_ascii_whitespace) {
            return Err(LlmError::Stream(
                "Anthropic stream ended with an incomplete SSE frame".into(),
            ));
        }
        if let Some((index, _)) = self.blocks.iter().find(|(_, block)| !block.stopped) {
            return Err(LlmError::Stream(format!(
                "Anthropic stream ended before content block {index} stopped"
            )));
        }
        if !self.message_stopped {
            return Err(LlmError::Stream(
                "Anthropic stream ended before message_stop".into(),
            ));
        }
        let blocks = self
            .blocks
            .into_values()
            .map(|block| block.state.content_block())
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

pub(super) async fn stream<F>(
    key: &str,
    model: &str,
    req: ChatRequest<'_>,
    cancel: &AtomicBool,
    on_event: &mut F,
) -> Result<TurnResult, LlmError>
where
    F: FnMut(StreamEvent) + Send,
{
    let body = body(model, req.messages, req.tools);
    let resp = http_client()?
        .post(URL)
        .header("x-api-key", key)
        .header("anthropic-version", VERSION)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(LlmError::Provider(format!("HTTP {status}: {text}")));
    }

    let mut stream = resp.bytes_stream();
    let mut decoder = StreamDecoder::default();
    while let Some(chunk) = next_chunk_or_cancel(&mut stream, cancel).await? {
        let bytes = chunk.map_err(|e| LlmError::Stream(e.to_string()))?;
        decoder.push_chunk(bytes.as_ref(), on_event)?;
    }

    if cancel.load(Ordering::Relaxed) {
        return Err(LlmError::Cancelled);
    }
    decoder.finish()
}
