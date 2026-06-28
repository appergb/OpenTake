# dispatch-tools — 统一派发壳 + 工具层

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：[`../../../crates/opentake-agent/src/mcp/dispatch.rs`](../../../crates/opentake-agent/src/mcp/dispatch.rs)、[`../../../crates/opentake-agent/src/tools/`](../../../crates/opentake-agent/src/tools/)

---

## 职责

一条**单一能力管线**包裹**每一个**工具（= 上游 `ToolExecutor.execute`），把所有前端（MCP / 应用内 Agent）的工具调用归一处理；编辑类工具最终归一到 `opentake-ops::EditCommand`。工具层（`tools/`）则是这些工具的"单一事实源"：名称、描述、Schema、类型化参数、错误措辞、短 id、紧凑编码。

## 派发壳：Dispatcher（`mcp/dispatch.rs`，~2650 行）

`Dispatcher` 持有三件东西：`Arc<dyn CoreHandle>`（文档边界，见 [core-handle-convert.md](core-handle-convert.md)）、`Arc<RwLock<PluginRegistry>>`（取激活插件，见 [plugin-system.md](plugin-system.md)）、`Mutex<Vec<String>>` 的**会话级 agent-undo 栈**。

### dispatch() 八步管线

`dispatch(name, args) -> ToolResult`：

1. **解析名** → `ToolName`（未知 → `ToolResult::error`）。
2. **快照** `before = handle.timeline()` + `manifest = handle.media()`。
3. **展开入站短 id 前缀**：用 `before`+`manifest` 构造 id 全集，[`short_id::expand_id_prefixes`](#short_idrs短-id-系统) 把参数里的短前缀还原成完整 id。
4. + 5. **解码类型化参数 + 跑 body**：`run_body` 用 `decode_tool_args` 解码（精确路径错误 → error），编辑工具构造 `EditCommand` 并 apply，读工具序列化状态；`OpContext op` 顺便收集"这次操作做了什么"供规则层用。
6. **附 Context Signal**：取 `after = handle.timeline()` + 激活插件，调 [`signal::engine::attach`](context-signal.md)（短 id 缩短前）。
7. **缩短出站 id**：用 `after` 的 id 全集 `short_id::shorten_ids`（新生成的 id 也缩短）。
8. 返回 `ToolResult`。

> 注：当前 `manual_video_type` 恒为 `None`（项目级手动视频类型设置尚未接线）。

### apply / agent-undo 治理

- `apply(cmd)` → `apply_raw` 调 `handle.apply`；若 `res.changed`，把 `action_name` 压入 agent-undo 栈。
- `undo()` 只在栈非空（即本会话 Agent 真改过）时弹栈并 `apply_raw(EditCommand::Undo)`；否则 `"undo: no agent edits to revert"`。这保护用户的手动编辑不被 Agent 撤销（照搬上游 `agentUndoStack` 守卫）。

### body 分类（44 工具）

- **读类（序列化状态）**：`get_timeline`（[紧凑编码](#encode_timelinersget_timeline-紧凑编码)）、`get_media`、`list_folders`、`list_models`（[投影 gen 目录](core-handle-convert.md)）。
- **编辑类（→ EditCommand → CoreHandle::apply）**：`add_clips`（含 `AddClipsAutoTrack` 自动建轨）、`insert_clips`、`move_clips`、`remove_clips`、`remove_tracks`、`split_clip`、`set_keyframes`、`ripple_delete_ranges`、`add_texts`、`set_clip_properties`、`create_folder`、`move_to_folder`、`rename_media`、`rename_folder`、`delete_media`、`delete_folder`、`set_color_grade`、`chroma_key`、`set_mask`、`apply_effect`、`undo`。
- **工作流插件类**：`list_workflows` / `activate_workflow` / `deactivate_workflow`（操作 `PluginRegistry`，激活时回发插件 `instructions.md`）。
- **分析驱动类（预览/建议，`applied:false`）**：`detect_beats` / `auto_cut_to_beats` / `tighten_silences`。经 `CoreHandle::extract_analysis_pcm` 抽 16k 单声道 PCM，调 `opentake-media` 的 `detect_beats` / `detect_silences`，把结果折算回项目帧后**返回建议**（节拍帧 / 切点 / 待执行的 `ripple_delete_ranges` 命令列表），由模型再调编辑工具落地——不直接改时间线。
- **honest stub（解码后返回 "not yet implemented"）**：`inspect_media`、`get_transcript`、`inspect_timeline`、`search_media`、`generate_video`、`generate_image`、`generate_audio`、`upscale_media`、`import_media`、`add_captions`、`add_motion_graphic`、`edit_motion_graphic`。
- **返回错误**：`smart_reframe`（需视觉/显著性后端，`CoreHandle` 未暴露采样帧）。

帧/秒折算、`speed` 归一、`source↔timeline` 帧映射等纯数学集中在本文件的自由函数（如 `source_seconds_to_timeline_frame_clamped`），遵循移植铁律的取整方向。

## 工具层文件（`tools/`）

### names.rs：44 工具枚举

`ToolName` 枚举 + `as_str()`（线名，与上游/规格逐字一致）+ `FromStr`。两个常量：

- **`ALL: [ToolName; 44]`** — 全部工具，注册顺序。
- **`UPSTREAM: [ToolName; 31]`** — 上游对齐子集（Issue #9 的"31 工具"）。

13 个 OpenTake 扩展 = `ALL` − `UPSTREAM`：分析驱动 4 + 工作流 3 + A-tier 效果 4 + Motion Canvas 2（详见 [总览](OVERVIEW.md)）。

### args.rs：类型化参数

每个工具一个 `#[serde(rename_all = "camelCase")]` 结构（`Option<T>` = 可选，`Vec<T>` = 数组），并实现 `ToolArgs`（带 `ALLOWED_KEYS`）。**线上多词字段必须 camelCase**（如 `atFrame` / `trackIndex` / `clipIds`）——与 IPC 层 `EditRequest` 同源的 camelCase 约定。

### descriptions.rs：描述 + Schema（~1040 行）

`description(tool)` 与 `input_schema(tool)`。描述字符串**逐字**移植自上游 `ToolDefinitions.swift`，**唯一改动**是产品名 `Palmier`→`OpenTake`、资源 URI `palmier://`→`opentake://`。原因：描述是驱动 LLM 行为的契约，不是装饰（ARCHITECTURE §7）。Schema 即上游 `inputSchema` JSON，直接用作 MCP 工具 schema。

### errors.rs：精确路径错误

`ToolError`（LLM 面消息，永不跨 MCP 边界 panic）。`ToolArgs::ALLOWED_KEYS` + `validate_unknown_keys` 拒绝未知字段（含 JSON Schema `additionalProperties:false` 够不着的**嵌套 `entries[]` 键**）；`first_non_finite_number_path` 查 `NaN`/`Inf`；`decode_tool_args` 用 `serde_path_to_error` 给出"`entries[3].startFrame: missing required field`"这类精确路径。**这些措辞是行为契约**——精确路径直接驱动模型的自我纠错率（对应历史上 IPC camelCase 不对齐导致"删除/分割/Inspector 全静默失效"的教训）。

### result.rs：中立结果类型

`Block`（`Text` / `Image`）+ `ToolResult { content, is_error }`，1:1 移植上游 `ToolResult.swift`。传输无关，故 MCP server 与（未来的）聊天循环共用；`push` 供 Context Signal 在主结果后追加信号块。

### short_id.rs：短 id 系统

实体 id 是完整 UUID（~36 字符），主导大 `get_timeline`/`get_transcript` 负载。出站缩成项目唯一最短前缀（≥ `ID_PREFIX_FLOOR = 8`），入站接受任意前缀还原成完整 id（工具始终在完整 id 上跑）。`current_id_universe` 从时间线 + 媒体清单收集所有 id；`SCALAR_ID_KEYS` / `ARRAY_ID_KEYS` 指明哪些参数键含 id 前缀。系统提示里有"前缀原样传回"的契约句（见 [prompt.md](prompt.md)）。1:1 移植上游 `ToolExecutor+ShortId.swift`。

### encode_timeline.rs：get_timeline 紧凑编码

token 友好的时间线表示（1:1 上游 `ToolExecutor+Timeline.swift`）：剥默认值字段（`speed:1`/`volume:1`/`opacity:1`/恒等 transform 等）、字幕 clip 折叠成 `captionGroups`（共享样式上提 + 每行 `[clipId, startFrame, durationFrames, text]`，每组上限 200 行）、浮点保留 3 位、`startFrame`/`endFrame` 窗口分页、轨道显示标签（V1/A1/…）而非存储 id。这是 Agent 层的"表示"职责，不是 `opentake-core` 的。

## 上游对照

| 上游 | 本子系统 |
|---|---|
| `ToolExecutor.execute`（ID 展开 → 分发 → diff timeline 决定入栈 → 缩短 ID） | `Dispatcher::dispatch` 八步管线 |
| `ToolDefinitions / AgentTool / ToolName` | `tools::names` / `descriptions` / `args` |
| `validateUnknownKeys` / `firstNonFiniteNumberPath` / `formatDecodingError` | `tools::errors` |
| `ToolExecutor+ShortId` | `tools::short_id` |
| `ToolExecutor+Timeline` | `tools::encode_timeline` |
| `ToolResult` | `tools::result` |
| `agentUndoStack` 守卫 | `Dispatcher` 的 `agent_undo` + `undo()` |

## 完成状态

- 已实现：完整八步管线、21 类编辑/读工具接线、3 分析工具（预览式）、3 工作流工具、agent-undo 栈、工具层全文件。
- 计划中：12 个 honest stub（媒体读 / 生成 / 导入 / 字幕 / Motion Canvas）+ `smart_reframe` 报错 + `create_folder`/`move_to_folder` 批量形式 + `get_timeline` 的 `canGenerate` 恒 `false`。详见 [总览](OVERVIEW.md) 完成状态。

---

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
