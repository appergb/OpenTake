# 应用内 chat（reqwest→Anthropic SSE / BYOK + prompt cache）

> **来源**：`AgentService.swift`（loop）、`AnthropicClient.swift`（直连）、`PalmierClient.swift`（代理）、`AgentClientTypes.swift`（SSE + 请求体 + cache）。**与 MCP 共享同一套工具 + 同一系统提示词**（§2、§6）。

## 5.1 模型与双通道（`selectClient:52-59`）

```rust
pub enum AnthropicModel { Sonnet46, Opus48, Haiku45 }
impl AnthropicModel { fn id(&self) -> &str { match self {
    Sonnet46 => "claude-sonnet-4-6",
    Opus48   => "claude-opus-4-8",
    Haiku45  => "claude-haiku-4-5-20251001", } } }
```
（与上游 `AgentClientTypes.swift:5-17` 完全一致。）

通道选择：
- **BYOK**（OS keychain `anthropic-api-key`，DEBUG 下可用 `ANTHROPIC_API_KEY` 环境变量，`AnthropicClient.swift:16-23`）→ `AnthropicClient` 直连 `https://api.anthropic.com/v1/messages`，三个模型可用。
- **托管**（OpenTake 自有鉴权/计费，**替换**上游 Clerk+Convex）→ `OpenTakeProxyClient` 打到 `opentake-gen-proxy` 的 `/v1/agent/stream`，带 OpenTake token。付费 Sonnet、免费 Haiku（策略可配）。

> ARCHITECTURE §7 `:153` / `:161`：**BYOK 直连可保留；上游 Convex/Clerk 付费代理属闭源云，换成 OpenTake 自有鉴权或去掉只留 BYO-key**。MVP 可只实现 BYOK，托管通道留接口（`trait AgentClient`）。

两 client 共用 `trait AgentClient`：
```rust
trait AgentClient: Send + Sync {
    fn stream(&self, system: &str, tools: &[ToolSchema], messages: &[AnthropicMessage])
        -> impl Stream<Item = Result<StreamEvent, ClientError>>;
}
enum StreamEvent { TextDelta(String), ToolUseComplete{ id:String, name:String, input_json:String }, MessageStop(StopReason) }
```
（对应 `AgentClientTypes.swift:41-69`。）

## 5.2 HTTP + SSE（`AnthropicClient.run:57-86` + `AnthropicSSE.parse:88-152`）

请求头（BYOK）：
```
x-api-key: {key}
anthropic-version: 2023-06-01
content-type: application/json
accept: text/event-stream
```
body：`AnthropicRequestBody.build`（§5.4），`stream: true`。

SSE 解析（用 `eventsource-stream` 或手解 `data:` 行；逐字照搬上游事件机）：
```
逐行：行以 "data:" 开头 → JSON 解 → switch event["type"]:
  message_start          → 记 usage（cache 命中率，§5.5）
  content_block_start    → 若 content_block.type=="tool_use" → pending_tools[index]=(id,name,"")
  content_block_delta    → text_delta:  yield TextDelta(text)
                            input_json_delta: pending_tools[index].json += partial_json
  content_block_stop     → 若 index 有 pending → yield ToolUseComplete(id,name, json.is_empty?"{}":json)
  message_delta          → 若 delta.stop_reason → yield MessageStop(StopReason::from(raw))
  error                  → finish(throwing streamError(msg))
```
HTTP ≥400：读完 body → `httpError(status, body)`（直连）；代理侧解 `{error:{code,message}}` 信封（`PalmierClient.from:80-101`，OpenTake 复用同款信封）。

## 5.3 Agentic loop（`runLoop:341-396` + `runPendingToolUses:422-447`）

```
loop while !cancelled:
  resolve_orphan_tool_uses()                 // §5.6 健壮性
  api_msgs = api_messages()                    // §5.7 上下文构建
  push 一个空 assistant 消息（占位，流式填充）
  stream = client.stream(system, tools, api_msgs)   // system = 组装后的提示词（§6.4）
  for event in stream:
     TextDelta(c)            → 追加到 assistant 最后一个 text block（或新建）
     ToolUseComplete(id,n,j) → assistant.blocks.push(ToolUse{id,n,j})
     MessageStop(reason)     → stop_reason = reason
  if stop_reason == ToolUse:
     run_pending_tool_uses()  // 对每个未解析 tool_use 调【同一个 ToolExecutor.execute】，结果作为 user 角色 tool_result 追加
     continue loop            // 再请求，直到 end_turn
  break loop
```
工具执行：`run_pending_tool_uses` 调**与 MCP 完全相同的 `ToolExecutor::execute`**（`AgentService.swift:441` 调 `executor.execute`）——这是「单一能力层、双前端」的落点。取消时给未跑的 tool_use 补 `tool_result{is_error:true, "Cancelled"}`。

`tools` 列表：`tool_definitions().map(|d| ToolSchema{ name, description, input_schema })`（`AgentService.swift:346`，与 MCP 用同一份 `ToolDefinitions`）。

## 5.4 请求体 + prompt cache 边界（`AnthropicRequestBody.build:156-194`）——**逐字复刻**

```
body = {
  model: model.id,
  max_tokens: 8192,                  // 上游 maxTokens=8192（AnthropicClient.swift:35）
  stream: true,
  system: [{ type:"text", text: system, cache_control:{type:"ephemeral"} }],   // ① system 打 cache
  messages: message_blocks,
}
tools = tools.map{ name, description, input_schema }
if !tools.empty: tools.last.cache_control = {type:"ephemeral"}   // ② 最后一个 tool 打 cache（覆盖 system+tools 边界）
body.tools = tools

// ③ 会话前缀缓存：最后一条消息的最后一个 content block 打 cache
if let last_msg = message_blocks.last, last_block = last_msg.content.last:
    last_block.cache_control = {type:"ephemeral"}
```
即：**system + tools 一个缓存边界 + 会话前缀一个缓存边界**。JSON 序列化用 sorted keys（上游 `options:[.sortedKeys]`，`AnthropicClient.swift:75`）以稳定缓存键。

> ARCHITECTURE §7 `:153`「复刻 prompt caching（system+tools+会话前缀打 ephemeral）」。这是直接的成本收益，照搬即可。

## 5.5 用量日志（`AgentUsageLog.record:73-84`）

DEBUG 下打印缓存命中率：`billed = input + cache_creation + cache_read`；`read% = cache_read/billed`。Rust 用 `tracing::debug!`。

## 5.6 孤儿 tool_use 修复（`resolveOrphanToolUses:458-495`）

发往 API 的消息序列必须合法（每个 `tool_use` 后必须紧跟配对 `tool_result`）。取消/出错会留下未配对的 `tool_use`。修复：扫描每个 assistant 消息的 tool_use id 集合，与下一条 user 消息的 tool_result id 集合比对，给孤儿补合成 `tool_result{is_error:true, content:[Text("Cancelled")]}`（插到下一条 user 消息开头，或新建一条 user 消息）。**必须复刻**，否则取消后再发会被 API 拒。

## 5.7 上下文构建 + @提及（`apiMessages:514-527` + `inlineImageBlocks:529-554`）

把本地 `[AgentMessage]`（块模型：`Text`/`ToolUse`/`ToolResult`）转 Anthropic `content` 数组。**@提及上下文**（`AgentMentionContext`）：
- 用户可 @ 媒体资源 / 时间线 clip / 选中区间。
- 发送时把被引用提及拼成一段 JSON `hint`（`Referenced assets and timeline context...`），**插到该用户消息最前面**。
- 图像类提及直接 base64 **内联成 image block**，并标 `inlined:true` 告诉模型别再 `inspect_media`。
- clip 提及附 `clipSummary`（clipId/mediaRef/帧位/trim/speed）。

> @提及是 chat UI 能力，**MVP 可后置**（MCP 通道无此概念）。但 `hint` 注入机制与 §6 的 Context Signal 注入是两套独立通道（一个进对话消息、一个进工具结果），不要混淆。

## 5.8 会话持久化（`ChatSessionStore`）

存到工程目录 `chat-sessions/`（每会话一个 JSON，ARCHITECTURE §9 `:176`）。多 tab（`ChatSession.isOpen`）。块模型 `AgentContentBlock`（`AgentService.swift:609-657`）用 serde 复刻（tagged enum：`Text`/`ToolUse`/`ToolResult`）。
