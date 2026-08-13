use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::chat::session::{AgentContentBlock, ChatMessage, Role, ToolCall};
use crate::tools::result::Block;

use super::{
    drain_sse_frames, http_client, next_chunk_or_cancel, ChatRequest, LlmError, StreamEvent,
    ToolSchema, TurnResult,
};

const URL: &str = "https://api.openai.com/v1/chat/completions";

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

/// Build the OpenAI request body from the session messages + tool schemas.
pub(super) fn body(
    model: &str,
    messages: &[ChatMessage],
    tools: &[ToolSchema],
) -> serde_json::Value {
    let msgs: Vec<serde_json::Value> = messages.iter().map(message).collect();
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
pub(super) fn message(m: &ChatMessage) -> serde_json::Value {
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
