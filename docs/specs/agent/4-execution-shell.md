# 统一执行壳 + 面向 LLM 的精确路径错误

> **来源**：`ToolExecutor.execute:22-70`（执行壳）、`validateUnknownKeys/decodeToolArgs/firstNonFiniteNumberPath/formatDecodingError:166-229`（校验 + 错误格式化）。

## 4.1 执行壳（`execute`，逐步照搬 + Rust 化）

```
execute(name, args) -> ToolResult:
  1. tool = ToolName::from_str(name)?            // 未知工具 → error "Unknown tool: {name}"
  2. core 可用？否 → error "Editor not available"
  3. before = core.timeline_snapshot()           // 快照（用于检测是否真变）
  4. t0 = Instant::now()
  5. log: "tool start name={tool}" + telemetry
  6. resolved = expand_id_prefixes(args)?         // §3.4 入站展开（歧义在此报错）
  7. result = run(tool, resolved).await           // 分发到 opentake-core 命令（§8）
  8. // undo 记账：非 undo、非 error、且 timeline 真变了 → 压 agentUndoStack
     if tool != Undo && !result.is_error && core.timeline_changed_since(before):
         agent_undo_stack.push(core.last_action_name())
  9. elapsed = t0.elapsed(); log ok/failed + telemetry{tool,durationSeconds,timelineChanged}
 10. // §6：注入 context_signal（OpenTake 新增，上游无）
     result = context_signal_engine.attach(tool, result, core, plugins)
 11. // §3.3：出站缩短（在 run 后的状态上做，新建 ID 也缩短）
     result = shorten_ids(result, core)
 12. return result
```

错误捕获：上游 `catch ToolError → .error(msg)`；`catch _ → .error(localizedDescription)`。Rust 用 `Result<ToolResult, ToolError>`，在 `execute` 末端把 `Err(ToolError)` 转成 `ToolResult::error(msg)`（永远返回 `ToolResult`，绝不向 MCP 抛 panic）。

**并发模型**：上游靠 `@MainActor` 串行化整条执行。Rust 用 `opentake-core` 的 `EditorState` actor（单线程 task + mpsc 命令队列）或 `tokio::sync::Mutex<EditorState>` 串行化（ARCHITECTURE 验证：`command` 是唯一编辑入口）。`agent_undo_stack` 是 chat/MCP **各自一份**会话状态（上游 `ToolExecutor` 实例持有；MCP 每连接一个 server 实例，chat 一个实例）。

## 4.2 严格输入校验（三层，**面向 LLM 的错误工程**）

### 4.2.1 未知字段拒绝（`validateUnknownKeys:166-171`）

```
unknown = keys(entry) - allowed
if !unknown.empty:
    error "{path}: unknown field(s) '{a}', '{b}'. Allowed: {sorted allowed joined ', '}."
```
**嵌套也查**：上游对 `entries[]` 逐条调 `validateUnknownKeys(d, allowed: Entry.allowedKeys, path: "entries[3]")`（`+Clips.swift:136`）。因为 serde/Decodable 默认不拒嵌套未知键。Rust：先 `serde_json::Value` 层手动比对 `allowedKeys`（每个工具一个 `&[&str]` 常量），再反序列化到强类型。

### 4.2.2 非有限数拒绝（`firstNonFiniteNumberPath:194-208`）

递归找第一个 `NaN`/`Inf` 的路径：

```
firstNonFiniteNumberPath(value, path):
  if value is f64 && !finite: return path
  if array: for (i, v): recurse "{path}[{i}]"
  if object: for (k, v): recurse "{path}.{k}"
  return None
// 命中 → error "{badPath}: value must be finite"
```

### 4.2.3 路径化解码错误（`formatDecodingError:210-229` + `decodeToolArgs:177`）

上游把 `DecodingError` 翻成精确路径：
- `keyNotFound` → `"{path}{trail}: missing required field '{key}'"`
- `typeMismatch` → `"{path}{trail}: expected {type}, got something else"`
- `valueNotFound` → `"{path}{trail}: missing required {type} value"`
- `dataCorrupted` → `"{path}{trail}: {debugDescription}"`
其中 `trail` 是 codingPath 拼成的 `.field[idx]`（例：`entries[3].startFrame`）。

**Rust 复刻**：用 `serde_path_to_error::deserialize`：

```rust
fn decode_tool_args<T: DeserializeOwned>(dict: &Value, path: &str) -> Result<T, ToolError> {
    validate_unknown_keys(dict, T::ALLOWED_KEYS, path)?;          // §4.2.1
    if let Some(bad) = first_non_finite_number_path(dict, path) {  // §4.2.2
        return Err(ToolError::new(format!("{bad}: value must be finite")));
    }
    let de = &mut serde_json::Deserializer::from_str(&dict.to_string());
    serde_path_to_error::deserialize(de).map_err(|e| {
        let p = e.path().to_string();                  // 例 "entries.3.startFrame"
        let p = normalize_path(path, &p);              // → "entries[3].startFrame"（数组下标加方括号）
        ToolError::new(map_serde_err(&p, e.inner()))   // missing field / invalid type → 上游同款措辞
    })
}
```
`normalize_path` 把 serde_path_to_error 的 `.` 分隔数字段转成 `[n]`，对齐上游 `entries[3].startFrame` 风格。`map_serde_err` 把 `serde_json::error::Category`（Data/Syntax）+ classify 成上游四类措辞。

> **为什么重要**：ARCHITECTURE §7 `:152` 与分析 04 `:209` 明确「这种精确路径错误直接决定 agent 自我纠正率」。这是必须复刻的、对 LLM 行为强相关的设计。

### 4.2.4 业务级守卫（照搬上游逐工具检查，举证）

以 `add_clips`（`+Clips.swift:13-174`）为例，错误措辞**原样**：
- `"Missing or empty 'entries' array"`
- `"entries[{idx}]: track index {ti} out of range (0..{max})"`
- `"entries[{idx}]: asset type {a} is not compatible with {b} track at index {ti}"`
- `"entries[{idx}]: durationFrames must be >= 1 (got {n})"`
- `"entries[{idx}]: startFrame must be >= 0 (got {n})"`
- `"entries[{idx}]: trimStartFrame must be >= 0 (got {t})"` / `trimEndFrame` 同
- `"Mixed trackIndex: {k} of {n} entries omitted trackIndex. Either set it on every entry or omit it on every entry (to auto-create shared tracks)."`

这些守卫属 `opentake-core`/`opentake-ops` 的命令校验层；本 crate 透传其错误文本（措辞与上游对齐，便于 LLM 自纠）。

## 4.3 助手专属 undo（`undo:109-123`）

```
undo(core) -> ToolResult:
  expected = agent_undo_stack.last() else error
      "No assistant edit to undo this session. The user's own edits are theirs to undo."
  if !core.can_undo():
      agent_undo_stack.clear(); error "Nothing to undo."
  if core.undo_action_name() != expected:
      error "The most recent change ('{actual}') wasn't made by the assistant — not undoing it."
  core.undo()
  agent_undo_stack.pop()
  ok "Undid: {expected}. The timeline is restored to its state before that edit; re-read with get_timeline or get_transcript before editing again."
```
依赖 `opentake-core` 暴露 `can_undo()` / `undo_action_name()` / `undo()`（对应上游 `editor.undoManager`）。

## 4.4 中立结果类型（`ToolResult.swift`）

```rust
pub enum Block { Text(String), Image { base64: String, media_type: String } }
pub struct ToolResult { pub content: Vec<Block>, pub is_error: bool }
impl ToolResult {
    pub fn ok(s: impl Into<String>) -> Self { Self { content: vec![Block::Text(s.into())], is_error: false } }
    pub fn error(s: impl Into<String>) -> Self { Self { content: vec![Block::Text(s.into())], is_error: true } }
}
```
转 rmcp 的 `CallToolResult`：`Text → Content::text`、`Image → Content::image{data,mime_type}`，`is_error: Some(true) | None`（上游 `is_error ? true : nil`，`ToolResult.swift:32`）。chat 侧直接用 `content`/`is_error`（§5）。

> **OpenTake 增强（ARCHITECTURE §7 `:154`）**：写工具统一返回**结构化 JSON**（变更的 clipId/帧位/新建轨），而非上游多数工具的人话字符串。可在 `Block::Text` 里放 JSON（与上游 `ripple_delete_ranges`/`remove_tracks` 已返回 JSON 的风格统一），便于多步链式编辑。**保留**上游已返回 JSON 的工具的字段形态。
