# Workflow Plugin（plugin.json 加载、activate_workflow、instructions.md 注入、rules 校验）

> **来源**：`docs/WORKFLOW-PLUGIN-SYSTEM.md`（全文）。**纯 JSON + Markdown，不修改 Rust core 编辑逻辑，完全在 Agent 层运作**（`:136-141`）。不需 Rust 编译、不需 WASM 运行时（`:141`）。

## 7.1 插件格式（`WORKFLOW-PLUGIN-SYSTEM.md:18-96`）

目录结构：
```
opentake-workflow-{id}/
├── plugin.json          # 元数据 + 工作流定义
├── instructions.md      # 给 Agent 的剪辑指引（Markdown）→ 注入系统提示词
├── assets/              # 可选动效模板
└── examples/            # 可选示例工程
```

`plugin.json` 的 Rust 模型（serde，对照 `:28-96` schema）：

```rust
// crates/opentake-agent/src/plugin/model.rs
#[derive(Deserialize, Clone)]
pub struct PluginManifest {
    pub schema_version: String,                 // "1.0"
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: PluginAuthor,                    // {name, url?}
    pub license: String,
    #[serde(default)] pub tags: Vec<String>,
    pub video_type: PluginVideoType,             // {primary, subtypes[], detection_hints{...}}
    pub workflow: PluginWorkflow,                // {approach, stages[], rules{do[],dont[]}}
    #[serde(default)] pub track_roles: HashMap<String, PluginTrackRole>, // {"V1":{role,label,locked?},...}
}
#[derive(Deserialize, Clone)]
pub struct PluginWorkflow {
    pub approach: String,                         // "audio_driven" 等
    pub stages: Vec<PluginStage>,                 // {id,name,order,actions[{tool,tip}]}
    pub rules: PluginRules,                        // {do:Vec<String>, dont:Vec<String>}
}
#[derive(Deserialize, Clone)]
pub struct PluginStage { pub id: String, pub name: String, pub order: u32, #[serde(default)] pub actions: Vec<PluginAction> }
#[derive(Deserialize, Clone)]
pub struct PluginAction { pub tool: String, pub tip: String }
#[derive(Deserialize, Clone)]
pub struct PluginRules { #[serde(default, rename="do")] pub do_: Vec<String>, #[serde(default)] pub dont: Vec<String> }
```
所有字段 `#[serde(default)]` 容错（缺字段不崩）。`rules.do` 用 `rename="do"`（`do` 是 Rust 关键字）。

加载后是运行时对象（含已读入的 `instructions_md: String`）：
```rust
pub struct LoadedPlugin { pub manifest: PluginManifest, pub instructions_md: String, pub dir: PathBuf }
```

## 7.2 插件注册表 + 加载（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`、`:136-141`）

```rust
pub struct PluginRegistry {
    installed: Vec<LoadedPlugin>,        // 从插件目录扫描加载的全部
    active: Option<String>,              // 当前激活的 plugin id（单激活；可扩展为多激活）
}
impl PluginRegistry {
    // 启动时 / activate 时从磁盘加载：读 plugin.json + instructions.md
    pub fn load_dir(dir: &Path) -> Result<LoadedPlugin, PluginError> {
        let manifest: PluginManifest = serde_json::from_str(&fs::read_to_string(dir.join("plugin.json"))?)?;
        let instructions_md = fs::read_to_string(dir.join("instructions.md")).unwrap_or_default();
        validate_manifest(&manifest)?;        // schema_version 支持、id 非空、stages.order 唯一、actions.tool ∈ 31 工具名
        Ok(LoadedPlugin { manifest, instructions_md, dir: dir.to_path_buf() })
    }
    pub fn active(&self) -> Option<&LoadedPlugin> { ... }
}
```
插件目录：工程级 `{project}/plugins/` 或用户级 `{config}/opentake/plugins/`（MVP 取一处即可）。

**校验**（对应 `WORKFLOW-PLUGIN-SYSTEM.md:131` `opentake plugin validate`）：`schema_version` 在支持集合；`id`/`name` 非空；`workflow.stages[].order` 唯一；`workflow.stages[].actions[].tool` 必须是 31 个合法工具名之一（否则 warning）；`track_roles` 的 role 字符串可解析为 `TrackRole`。

## 7.3 激活方式（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`）

| 方式 | 触发 | 实现 |
|---|---|---|
| 自动匹配 | 工程特征匹配 `video_type.detection_hints` | `get_timeline` 检测时比对，命中则**推荐**（不强制激活；提示用户/Agent） |
| 手动选择 | 用户在工程设置选 | 前端调 Tauri 命令 → `registry.activate(id)` |
| Agent 指定 | Agent 调 MCP 工具 `activate_workflow` | §7.4 |

## 7.4 `activate_workflow` MCP 工具（**OpenTake 新增的第 32 个工具**，`WORKFLOW-PLUGIN-SYSTEM.md:104`、ROADMAP Phase W `:119`）

> 上游无此工具——这是 OpenTake 的工作流插件能力。它**改变 server 状态**（激活的插件 → 影响后续 system prompt + context_signal），需在工具列表中注册。

```
name: "activate_workflow"
description（建议，OpenTake 自拟，风格对齐上游工具描述）:
  "Activates a workflow plugin for the current project. A workflow plugin packages
   editing conventions for one video type (talking-head, vlog, montage, interview,
   review, wedding, ...): it injects type-specific guidance into your instructions and
   adds rule checks to your edits. Call list_workflows first to see installed plugins
   and their ids. Activating replaces any previously active workflow. The plugin's
   track-role mapping and declared video_type override auto-detection."
inputSchema: { type:"object",
  properties: { workflowId: {type:"string", description:"Plugin id from list_workflows (e.g. 'opentake-workflow-popular-science')."} },
  required: ["workflowId"] }
```
执行：`registry.activate(workflow_id)?` → 触发系统提示词重组装（§6.5）→ 返回 `ok("Activated workflow: {name}. Re-read get_timeline for updated track roles and stage guidance.")`。

配套（建议同时加）：`list_workflows`（列已安装插件 `{id,name,description,video_type.primary,active}`）、`deactivate_workflow`。这三个属 Agent 层状态工具，不进 `EditCommand`（不改 timeline）。

> 工具计数：上游 31 + OpenTake 新增 `activate_workflow`(+可选 `list_workflows`/`deactivate_workflow`) + ARCHITECTURE §7 `:154` 建议的 `remove_filler_words`/`tighten_silences`/`get_capabilities`。**Issue #9 的「31 工具」指上游对等集（§2）；workflow/增强工具是 OpenTake 叠加。**

## 7.5 插件对 Agent 的三处影响（`WORKFLOW-PLUGIN-SYSTEM.md:108-118`，与 §6.5/§6.6 衔接）

1. **系统提示词注入**：`instructions.md` → 注入 system（§6.5），附当前轨道角色映射 + workflow rules。
2. **工具返回增强**：每次工具调用返回时附该阶段操作提示（`stage_guidance` 来自 `workflow.stages`，标来源 `plugin:{id}`，§6.1 表与 `AGENT-CONTEXT-SIGNAL.md:92`）。
3. **规则校验**：`workflow.rules` 在编辑操作时校验，违 `dont` 返 warning（§6.6.2）。

## 7.6 插件字段 → ContextSignal 叠加（`AGENT-CONTEXT-SIGNAL.md:88-98`，**叠加优先级与覆盖语义**）

| 插件字段 | 注入位置 | 规则 |
|---|---|---|
| `workflow.stages` | `ContextSignal.stage_guidance` | 插件阶段列表**追加**到内置阶段之后，标来源 `plugin:{id}` |
| `workflow.rules` | `ContextSignal.track_hints[].advice` + warning | 每次工具调用校验，违规产 warning |
| `track_roles` | `ContextSignal.track_roles` | 插件定义的角色**覆盖**自动检测（手动指定优先） |
| `video_type` | `ContextSignal.video_type` | 插件声明类型**覆盖**自动检测 |

叠加优先级：**插件声明 > 用户手动设置 > 软件自动检测 > 默认值**。
