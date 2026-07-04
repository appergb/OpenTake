# Context Signal 注入（每个工具返回如何附 `context_signal`）

> **来源**：`docs/AGENT-CONTEXT-SIGNAL.md`（全文）。知识源：ClipSkills（`appergb/ClipSkills`，MIT，12 册软件无关剪辑知识内核）。**核心原则：不让 Agent 自己读技能文件；软件在 Agent 操作时主动推送结构化 `context_signal`**。
>
> 落点：§4.1 执行壳第 10 步——`run` 之后、`shorten_ids` 之前，由 `ContextSignalEngine::attach(tool, result, core, plugins)` 给结果追加一个 `context_signal` 块。

## 6.1 注入时机（哪些工具附什么信号，`AGENT-CONTEXT-SIGNAL.md:37-47`）

| 工具 | 附加信号 | 内容 |
|---|---|---|
| `get_timeline` | `video_classification` + `track_roles` + `editing_stage` + `stage_guidance` | 视频类型判定、轨道用途映射、当前剪辑阶段 + 下一步建议 |
| `inspect_media` | `clip_analysis_hint` | 该片段镜头类型（广角/中景/特写）、景别建议 |
| `add_clips` / `insert_clips` | `placement_validation` | 轨道类型是否匹配片源、是否应拆 A/V + §6.6 规则校验 |
| `get_transcript` | `break_analysis` | 识别出的气口/句界/重复/啰嗦列表 |
| `search_media` | `material_match_hint` | B-roll 匹配优先级建议 |
| `add_texts` | `text_placement_hint` | 文字层级建议、安全区提醒 |
| `add_captions` | `caption_style_hint` | 当前视频类型的字幕风格建议 |
| 写工具（remove/move/split/ripple/set_*） | 规则校验 warning（§6.6） | 仅当操作触发某条规则才附 |

注入形态：在 `ToolResult.content` 末尾追加一个 `Block::Text`，内容是 `{"context_signal": {...}}` 的 JSON（与工具主结果分开，便于 LLM 区分「这是软件给的指引」）。**仅在该工具有对应信号时附加**；纯 CRUD（folders 组）不附。

## 6.2 数据结构（Rust，定义在 `opentake-domain`，本 crate 消费 + 序列化；`AGENT-CONTEXT-SIGNAL.md:50-83`）

```rust
// crates/opentake-domain/src/context_signal.rs（domain 定义）；opentake-agent 序列化进工具结果
#[derive(Serialize, Clone)]
pub struct ContextSignal {
    pub video_type: VideoType,
    pub confidence: f32,                         // 0.0..1.0
    pub track_roles: Vec<TrackRoleEntry>,        // {track_index, role}
    pub editing_stage: EditingStage,
    pub stage_guidance: StageGuidance,           // {description, next_actions[], warnings[]}
    pub editing_skeleton: EditingSkeleton,       // {approach, flow[], rules[]}
    pub track_hints: Vec<TrackHint>,             // {track_index, role, advice}
}

#[derive(Serialize, Clone)]
pub enum VideoType { TalkingHead, Vlog, Montage, Interview, ShortForm, LongForm }

#[derive(Serialize, Clone)]
pub enum TrackRole { MainCamera, BRollOverlay, TextOverlay, VoiceOver, Bgm, Sfx, GenericVideo, GenericAudio }

#[derive(Serialize, Clone)]
pub enum EditingStage { Importing, Classifying, RoughCut, BRollOverlay, AudioPolish, ColorGrade, ExportReady }

#[derive(Serialize, Clone)]
pub struct StageGuidance { pub description: String, pub next_actions: Vec<String>, pub warnings: Vec<String> }

#[derive(Serialize, Clone)]
pub struct EditingSkeleton { pub approach: String, pub flow: Vec<String>, pub rules: Vec<String> }

#[derive(Serialize, Clone)]
pub struct TrackHint { pub track_index: usize, pub role: TrackRole, pub advice: String }
```

> 注：`AGENT-CONTEXT-SIGNAL.md §6 实现路线`明确 `ContextSignal`/`TrackRole`/`VideoType` 类型定义在 **Phase A（随 Phase 0-1）于 `opentake-domain`**；MCP 工具附加信号在 **Phase B/C（随 Phase 7）于 `opentake-agent`**。本 crate **只负责生成 + 附加**，类型定义引用 `opentake-domain`。

## 6.3 视频类型自动检测（`AGENT-CONTEXT-SIGNAL.md:104-140`）

检测输入是 `get_timeline` 拿到的 `Timeline`（轨道类型/数量、clip 时长分布、是否有连续人声等）。检测在 `get_timeline` 调用时跑（`AGENT-CONTEXT-SIGNAL.md:105`）。

**检测规则表**（直接编码为 `fn classify(timeline, media, transcripts) -> (VideoType, confidence)`）：

| 特征 | 推断类型 | 置信度 |
|---|---|---|
| 1-2 条视频轨 + 音频轨有长段连续人声 | `TalkingHead` | 0.9 |
| 多视频轨 + 每条 clip 很短(<3s) + 有音乐轨 | `Montage` | 0.85 |
| 大量短 clip + 第一人称元数据 + 无固定人声 | `Vlog` | 0.8 |
| 多轨 + 同时间戳多机位 clip | `Interview` | 0.9 |
| 竖屏项目(width<height) + 大量文字 clip | `ShortForm` | 0.85 |
| 总时长 > 10min + 有章节标记 | `LongForm` | 0.8 |

**数据依据**（`get_timeline` 编码的真实结构，`ToolExecutor+Timeline.swift:60-112`）：Track 有 `type`(video/audio)、`muted`、`hidden`、`syncLocked`、`clips`；Clip 有 `mediaType`、`startFrame`、`durationFrames`、`sourceClipType`、`textStyle`。「连续人声」需 `opentake-media` 转写信号（轨上有长段语音 segment）；「竖屏」从 `timeline.width < timeline.height`；「文字 clip」从 `clip.mediaType == "text"`；「短 clip」从 `durationFrames / fps`。

**优先级**（`AGENT-CONTEXT-SIGNAL.md:98`）：**插件声明 > 用户手动设置（工程设置） > 软件自动检测 > 默认值**。即 `final_type = plugin.video_type ?? project.manual_video_type ?? auto_classify() ?? default`。

**类型 → 剪辑骨架映射**（`editing_skeleton`，`AGENT-CONTEXT-SIGNAL.md:122-140`，flow 文本原样）：
- **TalkingHead** → `audio_driven`：`提取主音轨 → 转写为字幕 → 识别气口/断点 → 精剪 A-roll → 语义匹配 B-roll → 贴画面上层 → BGM 卡点 → 调色导出`
- **Montage** → `montage_beat`：`铺主音乐 → 检测节拍/重音 → 素材按景别分类(远/中/特) → 景别递进匹配镜头 → 在节拍点切镜 → 调色导出`
- **Vlog** → `vlog_segment`：`乱序思维导图 → 提炼主线 → 分段式独立剪辑 → 旁白/节奏点串联 → 时钟理论布置爆点 → 调色导出`
- **Interview** → `interview_multicam`：`按音频波形对齐合板 → 导播式粗剪(谁说切谁) → 加人名条 → 提取金句 → BGM 铺底 → 导出`

## 6.4 轨道角色自动识别（`AGENT-CONTEXT-SIGNAL.md:148-173`）

`fn detect_track_roles(timeline, media) -> Vec<TrackRoleEntry>`，规则：

```
视频轨(type=video)：
  clip 长度 > 10s 且连续         → MainCamera (A-roll)
  clip 长度 < 5s 且在 MainCamera 上方(更高 track index) → BRollOverlay
  clip 类型全为 text             → TextOverlay
  否则                          → GenericVideo
音频轨(type=audio)：
  有长段人声(语音检测)           → VoiceOver
  连续音乐(频谱丰富)             → Bgm
  短促(<2s)且非语音              → Sfx
  否则                          → GenericAudio
```
「长段人声」「频谱丰富」需 `opentake-media` 信号（转写覆盖率 / 频谱分析）；纯结构特征（clip 时长、轨道相对位置、是否全 text）从 `Timeline` 直接算。

**插件覆盖**（`AGENT-CONTEXT-SIGNAL.md:94`）：插件 `track_roles`（如 `{"V1":{"role":"MainCamera"},...}`）**覆盖**自动检测（手动指定优先）。

**轨道 → Agent 指引**（`track_hints[].advice`，`AGENT-CONTEXT-SIGNAL.md:166-173`，advice 文本原样作为常量）：

| role | advice（原样） |
|---|---|
| `MainCamera` | 这是口播/讲解的主画面(A-roll)。不要在这条轨上做大幅缩放；硬切处用放大+位移遮蔽或贴 B-roll。主干时间轴，删 clip 会影响整体结构。 |
| `BRollOverlay` | 补充画面层。B-roll 遵循五注意：对齐口播时长 / 成组添加 / 遮蔽硬切 / 不重复 / 整轨静音。不够长就换素材，不要漏字。 |
| `TextOverlay` | 文字层。文字安全区在画布中央 80%。避免压在人物脸上。竖屏项目注意上下留白。 |
| `VoiceOver` | 主声音轨。气口按三规则处理（保留/扩充/叠化）；切点选在句界或重音；有 BGM 时做侧链让位。不可整轨静音。 |
| `Bgm` | 背景音乐。检测节拍作为镜头切换参考点；口播段压低让位人声(侧链/手动)；段落间做 J/L-cut 过渡。 |
| `Sfx` | 音效轨。上升音效(Rise)用于段落过渡前；低频轰鸣(Sub Boom)用于重点落点；环境音提前画面 2-3 秒渐入。 |

## 6.5 系统提示词组装（base + 插件 instructions.md，`AGENT-CONTEXT-SIGNAL.md:96` + `WORKFLOW-PLUGIN-SYSTEM.md:108-110`）

**注意：`instructions.md` 注入系统提示词，不进 `context_signal`**（`AGENT-CONTEXT-SIGNAL.md:96`「注入到 MCP server 的 serverInstructions」）。组装：

```
fn assemble_system_prompt(plugins: &PluginRegistry) -> String {
    let mut s = BASE_INSTRUCTIONS.to_string();   // §6.5.1 OpenTake 版基础提示词
    for plugin in plugins.active() {
        s.push_str("\n\n# Workflow Plugin: ");
        s.push_str(&plugin.name);
        s.push_str("\n");
        s.push_str(&plugin.instructions_md);     // 注入 instructions.md
        s.push_str("\n");
        s.push_str(&render_track_roles(&plugin.track_roles));  // 附当前轨道角色映射
        s.push_str(&render_workflow_rules(&plugin.workflow.rules)); // 附 do/dont
    }
    s
}
```
MCP server 启动时（`MCPService.start` 对应位置）用组装后的字符串作为 `instructions`；chat 用同一字符串作为 `system`（§5.3）。**插件激活后需重建 server 的 instructions / chat 下次请求用新 system**。

### 6.5.1 基础系统提示词（OpenTake 版）

来源：上游 `AgentInstructions.serverInstructions`（`AgentInstructions.swift:4-143`，分节 Core model / Always do / Editing / Generation / Audio generation / Prompt craft / Communication）。

**ARCHITECTURE §7 `:154` 的 OpenTake 增强**：从「单块字符串」升级为**分层可组合**，且**模型策略从配置注入**（上游把 Seedance/Nano Banana/Veo 等具体模型写死在提示词里，`AgentInstructions.swift:79-89`）。OpenTake：
- 拆成 `core_model` / `always_do` / `editing` / `generation`（模型策略占位，从 `opentake-gen` 的可用模型动态填）/ `communication` 多段常量，运行时拼装。
- `core_model` 段保留上游关键约束（**逐字保留契约性强的句子**）：
  - 「All timing is in FRAMES, not seconds: frame = seconds × fps.」
  - 「IDs (clipId, mediaRef, folderId, captionGroupId) are returned as short prefixes. Pass them back exactly as given — never pad, complete, or guess a longer form.」（短 ID 契约，**必须保留**，否则 §3 失效）
- `editing` 段保留 transcript-driven 删除的警告（`AgentInstructions.swift:65-69`「read the WORD-level get_transcript end-to-end as prose at least once before deduping」）。
- `communication` 段保留「默认一两句、报结果不报过程、别旁白 'let me…'、匹配冷静克制 HIG 风格」（`:133-142`）。
- 产品名 Palmier → OpenTake。

## 6.6 规则校验（内置规则 + 插件规则，`AGENT-CONTEXT-SIGNAL.md:177-212`）

写工具执行后，软件检查操作是否与剪辑规则一致，不匹配则在 `context_signal` 里附 warning。**内置规则（ClipSkills 通用）+ 插件规则同时生效，不互斥**；校验顺序：内置规则 → 插件规则 → 组合 warning 列表返回（`AGENT-CONTEXT-SIGNAL.md:206-212`）。

```rust
fn validate_operation(tool: ToolName, op: &OpContext, signal: &ContextSignal, plugins: &PluginRegistry)
    -> Vec<String> {
    let mut warnings = builtin_rules(tool, op, signal);       // §6.6.1
    warnings.extend(plugin_rules(op, plugins));               // §6.6.2
    warnings
}
```

### 6.6.1 内置规则（`AGENT-CONTEXT-SIGNAL.md:177-203`，warning 文本原样）

**口播精剪**（Agent 在 `VoiceOver` 轨 `remove_clips`/`split_clip`/trim 时）：
| 规则 | 检查 | warning（原样） |
|---|---|---|
| 气口三规则 | split/trim 在气口处 | "该处为气口，请判断：保留(衔接不自然)/扩充(太急促)/叠化(去不掉时)" |
| 不在词中间切 | split 在字幕词中 | "切点位于词中间，会导致漏字。请移到句界（语义完整处）。" |
| 删啰嗦不删主干 | remove clip 为主时间线 | "该 clip 为主干内容，删除会破坏叙事。确认这是啰嗦/卡顿？" |

**B-roll 匹配**（Agent `search_media`/`add_clips` 时）：
| 规则 | 检查 | warning（原样） |
|---|---|---|
| 时长对齐 | B-roll duration < 对端口播时长 | "B-roll 太短，话没说完画面就切了。换更长素材或让它盖到句尾。" |
| 不重复 | 选用 B-roll 与已有 clip 相同 | "该素材已于 frame X 处使用。避免同一素材重复出现。" |
| 成组添加 | 只选了一个短镜头 | "建议成组添加 2-3 个不同景别的镜头作为镜头组。" |
| 静音 | B-roll 音频未静音 | "B-roll 通常无声，已自动静音该轨。" |

**节奏与结构**（Agent `move_clips`/ripple 时）：
| 规则 | 检查 | warning（原样） |
|---|---|---|
| 信息密度 | clip 时长过长/过短 | "该 clip 信息量 [评估]，建议时长为 X-Y 秒" |
| 时钟理论 | 爆点位置缺少高能内容 | "当前 3 点位置（约 X 分钟处）暂无爆点，建议在此安排高能片段" |
| 波峰制 | 连续长段无起伏 | "已连续 Y 段平淡内容，建议在位置 Z 插入高潮" |

> 检查所需的语义信号（是否气口、是否词中、是否主干、是否高能）来自 `opentake-media`（转写/节拍/能量）。**MVP 可先实现纯结构可判定的规则**（不在词中间切=需词级时间戳；时长对齐=纯帧算；不重复=mediaRef 比对；静音=轨 muted 状态），语义重的（气口判断、信息密度、时钟理论）随 `opentake-media` 能力到位再开。

### 6.6.2 插件规则（`WORKFLOW-PLUGIN-SYSTEM.md:116-118`）

`plugin.json → workflow.rules.do/dont` 作为额外规则层。违反 `dont` 返回 warning。`do` 用作 `stage_guidance` 提示。匹配方式：MVP 用关键词/结构启发式（如「不要连续 3 段以上无 B-roll 覆盖」→ 检测 MainCamera 轨连续 N 段无上层 BRollOverlay 覆盖）。无法机器判定的规则降级为「软提醒」（在 stage_guidance.warnings 里原样列出供 LLM 自检）。
