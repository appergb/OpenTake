# plugin-system — 工作流插件

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：[`../../../crates/opentake-agent/src/plugin/`](../../../crates/opentake-agent/src/plugin/) · 设计：[WORKFLOW-PLUGIN-SYSTEM.md](WORKFLOW-PLUGIN-SYSTEM.md)

---

## 职责

把"针对某题材该怎么剪"的领域知识做成**可安装的工作流插件**：纯 JSON（`plugin.json`，**无 Rust 编译、无 WASM**）声明视频类型覆盖、轨道角色、分阶段动作提示、do/dont 规则。激活后它三路影响 Agent——进系统提示（[prompt.md](prompt.md)）、进 Context Signal 覆盖与告警（[context-signal.md](context-signal.md)）、由 `list_workflows`/`activate_workflow`/`deactivate_workflow` 三个工具操作（[dispatch-tools.md](dispatch-tools.md)）。纯 Agent 层状态，**不碰任何 `opentake-core` 编辑逻辑**。完整设计见 [WORKFLOW-PLUGIN-SYSTEM.md](WORKFLOW-PLUGIN-SYSTEM.md)。

## 子文件

### model.rs：plugin.json 模型

`PluginManifest` 及其子结构（`PluginWorkflow` / `PluginStage` / `PluginAction` / `PluginRules` / `PluginTrackRole` / `PluginVideoType` …）。所有字段 `#[serde(default)]` 容错——**部分/老版 manifest 永不解码失败**（校验是另一道宽松流程）。要点：`do` 是 Rust 关键字，故 `PluginRules` 里 `rename = "do"` → `do_`；`track_roles` 用 `BTreeMap` 保证稳定顺序（键如 `"V1"` / `"A1"`）。这呼应移植铁律"所有 serde 模型加 `#[serde(default)]` + `Option<T>`"。

### registry.rs：注册表（扫描 / 校验 / 激活）

`LoadedPlugin` = `manifest` + 读入的 `instructions_md` + 目录 + 非致命 `warnings`。`PluginRegistry` 持 `installed` 列表 + `active` id（单激活，可扩展多激活）。

- **加载**：`load_dir`（读 `plugin.json` + `instructions.md`）、`load_from_strings`（内存，测试/无盘激活用）、`scan(root)`（扫子目录，失败的跳过并收集错误）。
- **校验** `validate_manifest`：致命错误（不支持的 `schema_version`，仅 `"1.0"`；空 id/name；阶段 `order` 重复）→ `Err`；非致命（未知工具名、无法解析的角色、未识别的 video_type）→ 警告列表。镜像 `opentake plugin validate`。
- **角色/类型映射**：`parse_track_role`（容多种拼写，如 `VoiceOver`→`Voice`、`BRollOverlay`→`BRoll`）、`parse_video_type`（snake_case→`VideoType`）；未识别返回 `None`（覆盖落空则回退自动检测）。
- **激活**：`register`（同 id 替换）、`activate(id)`（替换前一个）、`deactivate`、`active()`、`installed()`。

### rules.rs：插件 dont 规则层

`plugin_rules(plugin, roles, timeline)` 在内置规则之上加一层（顺序：内置 → 插件）。对机器可判定的措辞做结构匹配——`parse_consecutive_no_broll` 识别"不要连续 N 段以上无 B-roll 覆盖"，`max_uncovered_main_camera_run` 用重叠测试算主画面无 B-roll 覆盖的最长连续段，达阈值才告警（带 `[plugin:{id}]` 标签 + 实测段数）。其余无法机判的 `dont` 原样输出为软提醒，交模型自查。

### builtin/audio-first/：内置默认工作流

`audio-first`（音频先入，id `opentake-workflow-audio-first`）通过 `include_str!` 把 `plugin.json` + `instructions.md` **编译进二进制**——默认剪辑 Skill 永远可用，无需文件系统播种。`builtin_plugins()` 返回它们；`PluginRegistry::with_builtins()` 预装这些内置插件，生产从这里起步再叠加用户插件。其 `approach = audio_driven`，分阶段提示（先铺音频 → 精剪口播 → 铺画面 → …），覆盖口播/Vlog/混剪/综艺等多数题材。

## 优先级与三路影响

```
激活插件 plugin
 ├─ 系统提示（prompt::assemble）：instructions.md（不可信围栏）+ 轨道角色 + do/dont
 ├─ Context Signal（signal::engine::build_signal）：
 │     video_type 覆盖（插件 > 手动 > 自动）
 │     track_roles 覆盖（按 V1/A1 标签）
 │     workflow.stages 动作 tip 追加 next_actions（[plugin:{id}]）
 │     workflow.rules.dont → plugin_rules 告警
 └─ 工具：list_workflows / activate_workflow（回发 instructions.md）/ deactivate_workflow
```

## src-tauri 集成

`src-tauri/src/mcp.rs` 的 `build_registry(workflows_dir)`：`PluginRegistry::with_builtins()` 起步，再 `scan(workflows_dir)` 叠加用户自写插件，包进 `Arc<RwLock<…>>` 喂给 `server::serve`。

## 上游对照

无直接上游对应——工作流插件是 OpenTake 新增（见 [WORKFLOW-PLUGIN-SYSTEM.md](WORKFLOW-PLUGIN-SYSTEM.md)、`agent-SPEC.md` §7）。

## 完成状态

- 已实现：JSON 模型（全容错）、注册表（扫描/校验/激活）、插件规则层、内置 `audio-first`、三个工作流工具、与提示及信号的接线、`src-tauri` 集成。测试覆盖。
- 计划中：多激活（当前单激活）；`create_folder`/`move_to_folder` 批量形式属派发层（见 [dispatch-tools.md](dispatch-tools.md)），与本子系统无关。

---

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
