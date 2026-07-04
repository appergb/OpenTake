# context-signal — Context Signal（软件主动发信号）

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：[`../../../crates/opentake-agent/src/signal/`](../../../crates/opentake-agent/src/signal/) · 设计：[AGENT-CONTEXT-SIGNAL.md](AGENT-CONTEXT-SIGNAL.md)

---

## 职责

这是 OpenTake **新增**的能力（上游无对应）：软件**主动**把"这条片子是什么类型、每条轨道是什么角色、剪到哪一步、刚才那一刀有没有问题"作为一个 `context_signal` JSON 块**回发给模型**，让 Agent 不靠猜就知道该怎么剪。信号类型定义在 `opentake-domain`；本层只**生成 + 附挂**。完整设计见 [AGENT-CONTEXT-SIGNAL.md](AGENT-CONTEXT-SIGNAL.md)。

> MVP 说明：结构化可判定的特征已实现；语义重的特征（连续人声、第一人称元数据、章节标记、气口词级时间戳）需 `opentake-media`，当前用**结构化近似**或降级为软提醒。

## 子文件

### classify.rs：视频类型自动判定

`classify(timeline) -> (VideoType, confidence)`，纯结构推断。规则按"最具体优先"：多机位（≥2 视频轨且有同起始帧）→ `Interview(0.9)`；1~2 视频轨 + 有长音频（≥10s 连续音）→ `TalkingHead(0.9)`；竖屏 + ≥3 文字 clip → `ShortForm(0.85)`；多视频轨 + 全短 clip + 有音频 → `Montage(0.85)`；≥8 短 clip → `Vlog(0.8)`；总时长 >600s → `LongForm(0.8)`；兜底 `TalkingHead(0.5)`。

### track_roles.rs：轨道角色检测 + 逐角色建议

`detect_track_roles(timeline)` 给每条轨道判一个 `TrackRole`（`MainCamera` / `BRoll` / `Voice` / `Bgm` / `Sfx` / `Text` / `Caption`）：视频轨长连续 clip → `MainCamera`，主画面之上的短 clip → `BRoll`，全文字 → `Text`，全字幕 → `Caption`；音频轨长连续 → `Voice`，极短多 clip → `Sfx`，中长 → `Bgm`（频谱判定留给媒体层）。`role_advice(role)` 给逐角色建议文本（**逐字**取自 [AGENT-CONTEXT-SIGNAL.md](AGENT-CONTEXT-SIGNAL.md) §3.2，如 B-roll 五注意、气口三规则）；`track_hints` 打包成 `[track_index, role, advice]`。

### stages.rs：剪辑阶段 + 阶段指引 + 剪辑骨架

- `infer_stage(timeline)` —— 粗启发推断 `EditingStage`：无轨道 `Importing` → 有轨无 clip `Classifying` → 有 clip 无字幕 `RoughCut` → 有字幕无 B-roll 轨 `AudioPolish` → 有 B-roll 叠层 `BRollOverlay`。
- `stage_guidance(stage)` —— 每阶段的 `description` + `next_actions` + `warnings`（插件阶段在引擎层追加其上）。
- `editing_skeleton(video_type)` —— 视频类型 → `approach` + `flow`（flow 文本**逐字**，如口播 `audio_driven`：提取主音轨 → 转写字幕 → 识别气口 → 精剪 A-roll → 语义匹配 B-roll → …）。

### rules.rs：内置规则告警 + OpContext

`OpContext` 是派发层从"已解码参数 + 前后时间线"蒸馏出的"这次写操作做了什么"（主轨索引 / 删改的 clip ids / 新增 mediaRefs / 目标轨是否静音 / 切点是否在词中间）。`builtin_rules(tool, op, roles, timeline)` 返回告警（文本**逐字**取自 `agent-SPEC.md` §6.6.1）：

- `split_clip` 词中切 → "切点位于词中间，会导致漏字…"；主声音轨且未知 → 气口三规则软提醒。
- `remove_clips` 在主声音/主画面轨 → "该 clip 为主干内容，删除会破坏叙事…"。
- `add_clips` 重复素材 → "该素材已于 frame N 处使用…"；B-roll 轨未静音 → "B-roll 通常无声，已自动静音…"。

结构化可判定的规则真判定，语义重的降级为软提醒（待 `opentake-media` 落地）。

### engine.rs：构建 + 附挂 + 插件覆盖

- `build_signal(timeline, plugin, manual_video_type)` —— 组装完整 `ContextSignal`，并应用**插件覆盖**：视频类型优先级 **插件 > 手动 > 自动**；插件 `track_roles`（按 V1/A1 标签匹配）覆盖检测；插件 `workflow.stages` 的动作 tip 追加到 `next_actions`（带 `[plugin:{id}]` 标签）。
- `tool_emits_signal(tool)` —— 哪些工具带信号（`get_timeline` / 读媒体 / 加 clip/文字 / 写工具）；纯 CRUD（文件夹组）不带。
- `attach(tool, result, timeline, plugin, manual_video_type, op)` —— 在派发管线第 6 步调用（短 id 缩短前）。错误结果或无信号工具直接返回。`get_timeline` 附**完整**信号（视频类型 + 轨道角色 + 阶段指引 + 剪辑骨架 + 逐轨建议）；加 clip/文字/读媒体类附**轻量**信号（轨道角色 + 阶段指引 + 告警）；纯写工具仅在有告警时附告警。告警 = 内置规则 + 插件规则（见 [plugin-system.md](plugin-system.md)）。
- `extract_signal(result)` —— 从结果末块提取 `context_signal`（测试与聊天层用）。

## 数据流（在派发管线内）

```
工具 body 跑完 → after = handle.timeline() + 取激活插件
  → engine::attach(tool, result, after, plugin, None, op)
      build_signal（classify + track_roles + stages，应用插件覆盖）
      按工具类别选 完整 / 轻量 / 仅告警
      告警 = builtin_rules + plugin_rules
      result.push(Block::text({"context_signal": …}))
  → 短 id 缩短 → 返回
```

## 上游对照

无直接上游对应——Context Signal 是 OpenTake 把 ClipSkills 剪辑知识内化进 Agent 反馈环的新增设计（见 [AGENT-CONTEXT-SIGNAL.md](AGENT-CONTEXT-SIGNAL.md) 与 `agent-SPEC.md` §6）。

## 完成状态

- 已实现：视频类型/轨道角色/阶段三套结构化判定、内置规则、插件覆盖与追加、附挂引擎。逐角色建议与骨架/告警文本逐字落地，测试覆盖。
- 计划中（结构化近似，待 `opentake-media`）：连续人声/频谱判定、词级气口（`OpContext.mid_word` 多数情况为 `None`）、信息密度/时钟理论等语义规则；`manual_video_type` 项目级设置尚未接线（恒 `None`）。

---

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
