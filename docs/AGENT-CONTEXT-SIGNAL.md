# Agent Context Signal — 软件主动向 Agent 发送剪辑指引

> 核心原则：**不让 Agent 自己去读技能文件。软件在 Agent 操作轨道/时间线时，主动推送结构化的上下文信号。**
> 来源：ClipSkills 技能套件（`appergb/ClipSkills`）——12 册软件无关的剪辑知识内核，MIT 许可。此处将其内化为软件的"信号发射器"。

## 0. 设计理念

传统方式：Agent 自行加载技能文件 → 理解 → 做出剪辑决策。问题在于 (a) 上下文窗口被技能文件占据 (b) Agent 需要在"理解技能"和"理解当前工程"之间切换注意力。

OpenTake 的方式：软件的 **MCP server 在每次 Agent 调用工具时，在返回结果中附带一段 `context_signal`**，直接告知 Agent 当前操作的语义背景和剪辑指引。Agent 不需要知道这些指引来自哪里——它只需要按指引行动。

```
Agent 调用 get_timeline
  → MCP server 返回 timeline JSON + context_signal:
    {
      "video_type": "talking_head",
      "track_roles": {"V1":"main_camera","V2":"b_roll_overlay","A1":"voice","A2":"bgm"},
      "editing_skeleton": {
        "approach": "audio_driven",
        "flow": [
          "提取主音轨 → 转写为字幕 → 识别气口/断点",
          "精剪 A-roll → 语义匹配 B-roll → 贴画面上层"
        ]
      },
      "editing_stage": "RoughCut",
      "stage_guidance": {
        "next_actions": ["识别并标记所有气口和句界断点"],
        "warnings": ["不要在词中间切分"]
      }
    }
```

## 1. 信号体系设计

### 1.1 信号发射时机

| 工具调用 | 发射的信号 | 内容 |
|---|---|---|
| `get_timeline` | `video_classification` + `track_roles` | 视频类型判定、轨道用途映射、当前剪辑阶段 |
| `inspect_media` | `clip_analysis_hint` | 该片段的镜头类型（广角/中景/特写）、景别建议 |
| `add_clips` / `insert_clips` | `placement_validation` | 轨道类型是否匹配片源类型、是否应拆分 A/V |
| `get_transcript` | `break_analysis` | 识别出的气口/句界/重复/啰嗦列表 |
| `search_media` | `material_match_hint` | B-roll 匹配优先级建议 |
| `add_texts` | `text_placement_hint` | 文字层级建议、安全区提醒 |
| `add_captions` | `caption_style_hint` | 当前视频类型的字幕风格建议 |
| `export_start` | `export_profile` | 按平台推荐的导出参数 |

### 1.2 信号数据结构

```
ContextSignal {
    // 视频类型识别（来自 ClipSkills §01）
    video_type: TalkingHead | Vlog | Montage | Interview | ShortForm | LongForm,
    confidence: 0.0..1.0,

    // 轨道角色映射
    track_roles: [{track_index, role: MainCamera|B_Roll|Voice|BGM|SFX|Text|Caption}],

    // 当前剪辑阶段
    editing_stage: Importing | Classifying | RoughCut | BRollOverlay | AudioPolish | ColorGrade | ExportReady,

    // 当阶段建议（指引 Agent 下一步）
    stage_guidance: {
        description: "当前阶段说明",
        next_actions: ["下一步操作1", "下一步操作2"],
        warnings: ["注意事项"],
    },

    // 按视频类型的剪辑骨架（来自 ClipSkills §01-§05）
    editing_skeleton: {
        approach: "audio_driven" | "montage_beat" | "vlog_segment" | "interview_multicam",
        flow: ["Step1", "Step2", ...],
        rules: ["规则1", "规则2", ...],
    },

    // 当前轨道的操作提示
    track_hints: [{
        track_index: 0,
        role: "MainCamera",
        advice: "主轨 A-roll，承载口播主线。避免在此轨道上做大幅缩放。硬切处用放大+位移遮蔽或贴 B-roll。",
    }],
}
```


## 1.4 工作流插件对信号的增强

当用户激活一个工作流插件（见 [WORKFLOW-PLUGIN-SYSTEM.md](WORKFLOW-PLUGIN-SYSTEM.md)）时，插件的以下内容**叠加**到 ContextSignal 中：

| 插件字段 | 注入位置 | 规则 |
|---|---|---|
| `plugin.json → workflow.stages` | `ContextSignal.stage_guidance` | 插件的阶段列表追加到内置阶段之后，标注来源为 `plugin:{plugin_id}` |
| `plugin.json → workflow.rules` | `ContextSignal.track_hints[].advice` | 插件的 do/dont 规则在每次工具调用返回时校验，违规产生 warning |
| `plugin.json → track_roles` | `ContextSignal.track_roles` | 插件定义的轨道角色**覆盖**自动检测的角色（手动指定优先） |
| `plugin.json → video_type` | `ContextSignal.video_type` | 插件声明的视频类型**覆盖**自动检测的类型 |
| `instructions.md` | **系统提示词**（不在 context_signal 中） | 注入到 MCP server 的 serverInstructions |

叠加优先级：插件声明 > 用户手动设置 > 软件自动检测 > 默认值。


## 2. 视频类型检测机制

### 2.1 自动检测（从工程特征推断）

软件在 Agent 调用 `get_timeline` 时，自动分析工程特征并推断视频类型：

| 特征 | 推断类型 | 置信度 |
|---|---|---|
| 1-2 条视频轨 + 音频轨有长段连续人声 | `talking_head` | 0.9 |
| 多视频轨 + 每条 clip 很短(<3s) + 有音乐轨 | `montage` | 0.85 |
| 大量短 clip + 第一人称元数据 + 无固定人声 | `vlog` | 0.8 |
| 多轨 + 同时间戳多机位 clip | `interview` | 0.9 |
| 竖屏项目 + 大量文字 clip | `short_form` | 0.85 |
| 总时长 > 10min + 有章节标记 | `long_form` | 0.8 |

### 2.2 手动声明（工程设置）

用户可在工程设置中显式声明视频类型，优先级高于自动检测。

### 2.3 类型 → 剪辑骨架映射

**口播教程（Talking Head）** — audio_driven 流水线：
```
提取主音轨 → 转写为字幕 → 识别气口/断点 → 精剪 A-roll → 语义匹配 B-roll → 贴画面上层 → BGM 卡点 → 调色导出
```

**混剪卡点（Montage）** — montage_beat 流水线：
```
铺主音乐 → 检测节拍/重音 → 素材按景别分类(远/中/特) → 景别递进匹配镜头 → 在节拍点切镜 → 调色导出
```

**Vlog（Vlog）** — vlog_segment 流水线：
```
乱序思维导图 → 提炼主线 → 分段式独立剪辑 → 旁白/节奏点串联 → 时钟理论布置爆点 → 调色导出
```

**采访多机位（Interview）** — interview_multicam 流水线：
```
按音频波形对齐合板 → 导播式粗剪(谁说切谁) → 加人名条 → 提取金句 → BGM 铺底 → 导出
```

## 3. 轨道类型感知系统

### 3.1 轨道角色自动识别

软件在 Agent 读写轨道时，自动分析并标注每个轨道的角色：

```
Track Role 检测规则：
├── 视频轨(V1,V2,...)
│   ├── clip 长度 > 10s 且连续 → MainCamera (A-roll)
│   ├── clip 长度 < 5s 且在 MainCamera 上方 → B_RollOverlay
│   ├── clip 类型全为 text → TextOverlay
│   └── 否则 → GenericVideo
├── 音频轨(A1,A2,...)
│   ├── 有长段人声(语音检测) → VoiceOver
│   ├── 连续音乐(频谱丰富) → BGM
│   ├── 短促(<2s)且非语音 → SFX
│   └── 否则 → GenericAudio
```

### 3.2 轨道 → Agent 指引映射

每个轨道的 role 标注后，在 `get_timeline` 返回结果中附带操作指引：

| 轨道角色 | 给 Agent 的指引 |
|---|---|
| `MainCamera` | 这是口播/讲解的主画面(A-roll)。不要在这条轨上做大幅缩放；硬切处用放大+位移遮蔽或贴 B-roll。主干时间轴，删 clip 会影响整体结构。 |
| `B_RollOverlay` | 补充画面层。B-roll 遵循五注意：对齐口播时长 / 成组添加 / 遮蔽硬切 / 不重复 / 整轨静音。不够长就换素材，不要漏字。 |
| `TextOverlay` | 文字层。文字安全区在画布中央 80%。避免压在人物脸上。竖屏项目注意上下留白。 |
| `VoiceOver` | 主声音轨。气口按三规则处理（保留/扩充/叠化）；切点选在句界或重音；有 BGM 时做侧链让位。不可整轨静音。 |
| `BGM` | 背景音乐。检测节拍作为镜头切换参考点；口播段压低让位人声(侧链/手动)；段落间做 J/L-cut 过渡。 |
| `SFX` | 音效轨。上升音效(Rise)用于段落过渡前；低频轰鸣(Sub Boom)用于重点落点；环境音提前画面 2-3 秒渐入。 |

## 4. 剪辑手法内化（ClipSkills 核心原则 → 软件提示）

### 4.1 口播精剪规则（Agent 操作 remove_clips / split_clip 时触发）

当 Agent 在 `VoiceOver` 轨道上操作时，软件检查操作是否与以下规则一致：

| 规则 | Agent 操作检查 | 不匹配时的信号 |
|---|---|---|
| 气口三规则 | split/trim 在气口处 | "该处为气口，请判断：保留(衔接不自然)/扩充(太急促)/叠化(去不掉时)" |
| 不在词中间切 | split 在字幕词中 | "切点位于词中间，会导致漏字。请移到句界（语义完整处）。" |
| 删啰嗦不删主干 | remove clip 为主时间线 | "该 clip 为主干内容，删除会破坏叙事。确认这是啰嗦/卡顿？" |

### 4.2 B-roll 匹配规则（Agent 调用 search_media / add_clips 时触发）

| 规则 | Agent 操作检查 | 不匹配时的信号 |
|---|---|---|
| 时长对齐 | B-roll duration < 对端口播时长 | "B-roll 太短，话没说完画面就切了。换更长素材或让它盖到句尾。" |
| 不重复 | 选用的 B-roll 与已有 clip 相同 | "该素材已于 frame X 处使用。避免同一素材重复出现。" |
| 成组添加 | 只选了一个短镜头 | "建议成组添加 2-3 个不同景别的镜头作为镜头组。" |
| 静音 | B-roll 音频未静音 | "B-roll 通常无声，已自动静音该轨。" |

### 4.3 节奏与结构规则（Agent 操作 move_clips / ripple 时触发）

| 规则 | Agent 操作检查 | 不匹配时的信号 |
|---|---|---|
| 信息密度 | clip 时长过长/过短 | "该 clip 信息量 [评估]，建议时长为 X-Y 秒" |
| 时钟理论 | 爆点位置缺少高能内容 | "当前 3 点位置（约 X 分钟处）暂无爆点，建议在此安排高能片段" |
| 波峰制 | 连续长段无起伏 | "已连续 Y 段平淡内容，建议在位置 Z 插入高潮" |


## 4.4 规则集成：内置规则 + 插件规则

§4.1-§4.3 描述的是**内置剪辑规则**（来自 ClipSkills 通用知识）。当工作流插件激活时：

- 插件 `workflow.rules.do` 和 `workflow.rules.dont` 作为**额外规则层**参与校验
- 内置规则和插件规则**同时生效**，不互斥
- 若同一操作同时触发内置规则 warning 和插件规则 warning，两者都返回
- 规则校验顺序：内置规则 → 插件规则 → 组合 warning 列表返回

## 5. 外部工具能力内化

### 5.1 AI Cut / 剪映智能能力内化

将剪映的 AI 能力作为 OpenTake 的工具/信号：

| 剪映能力 | OpenTake 内化方式 | 实现 |
|---|---|---|
| 智能镜头分割 | `analyze_clips` 工具 | whisper-rs 转写 + ffmpeg 场景检测 |
| 自动卡点 | `detect_beats` 工具 | Symphonia PCM + 节拍检测 |
| AI 调色 | `color_match` 工具 | wgpu 着色器直方图匹配 |
| AI 抠像 | `ai_matte` 工具 | ort 跑 RVM/BiRefNet |
| 文本转语音 | `generate_voice` 工具 | BYOK ElevenLabs / 自建 TTS |
| 曲线变速 | `set_speed_curve` 工具 | domain 升级 speed → KeyframeTrack |
| 智能字幕 | `add_captions`(已有) | whisper-rs 转写 + 样式化 |

### 5.2 Pika / Runway 生成能力内化

| 能力 | OpenTake 内化方式 |
|---|---|
| 文生视频 | `generate_video` 工具 → BYOK fal.ai / Replicate |
| 图生视频 | `generate_video` + reference image |
| 视频扩展 | 生成后接 `insert_clips` 拼接到时间线 |
| 风格迁移 | `apply_effect` + wgpu 着色器或云端 API |

## 6. 实现路线

| 阶段 | 内容 | 依赖 |
|---|---|---|
| Phase A（随 Phase 0-1） | 在 `opentake-domain` 定义 `ContextSignal` / `TrackRole` / `VideoType` 类型 | Phase 1 domain 模型 |
| Phase B（随 Phase 7） | MCP 工具在返回结果中附加 `context_signal` | Phase 7 MCP server |
| Phase C（随 Phase 7） | 视频类型自动检测 + 轨道角色自动标注 | Phase 7 + Phase 2 转写 |
| Phase D（随 Phase 3.5+） | 剪辑规则校验 + 操作不匹配信号 | Phase 3.5 wgpu + Phase 7 MCP |
| Phase E（后续） | 外部工具能力 API 对接 | Phase 9 生成后端 |

## 7. 相关文档

- [ARCHITECTURE.md](ARCHITECTURE.md) — §7 MCP + Agent 层
- [ROADMAP.md](ROADMAP.md) — Phase 7 MCP server
- [WORKFLOW-PLUGIN-SYSTEM.md](WORKFLOW-PLUGIN-SYSTEM.md) — 工作流插件系统
- [MODULE-PORT-MAP.md](MODULE-PORT-MAP.md) — Agent 工具层移植
- `appergb/ClipSkills` — 技能来源，MIT 许可

