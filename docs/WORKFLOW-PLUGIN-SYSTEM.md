# OpenTake 工作流插件系统

> 目标：让外部贡献者可以为特定视频类型（口播、Vlog、混剪、采访等）编写可复用的"剪辑工作流插件"，Agent 加载插件后获得该类型的完整剪辑决策链。
> 灵感来源：剪映的"一键成片"模板系统、DaVinci Resolve 的 Workflow Integrations、以及 ClipSkills 的分类型剪辑骨架。
> 插件不替代 Agent——插件提供**结构化指引**，Agent 仍是执行者。

## 0. 为什么是插件

- **可扩展**：每种视频类型（口播教程、产品评测、游戏实况、婚礼剪辑……）有独特的剪辑惯例，无法在核心软件中穷举。
- **社区驱动**：插件可由剪辑师/UP 主编写，封装其个人剪辑方法论和审美偏好。
- **Agent 友好**：Agent 加载插件后获得完整的"该类型怎么剪"指引，无需用户每次重复说明。
- **轻量**：插件本质是 JSON + Markdown，无需编译、无需 Node/Python 运行时。

## 1. 插件格式

### 1.1 目录结构

```
opentake-workflow-popular-science/
├── plugin.json          # 元数据 + 工作流定义
├── instructions.md      # 给 Agent 的剪辑指引（Markdown）
├── assets/              # 可选的动效模板
└── examples/            # 可选：示例工程
```

### 1.2 plugin.json schema

```json
{
  "schema_version": "1.0",
  "id": "opentake-workflow-popular-science",
  "name": "科普视频工作流",
  "description": "适用于科普类视频的剪辑工作流：口播精剪 + 示意图 B-roll + 数据可视化",
  "author": { "name": "作者名", "url": "https://..." },
  "license": "MIT",
  "tags": ["科普", "教育", "口播"],
  "video_type": {
    "primary": "talking_head",
    "subtypes": ["educational", "science"],
    "detection_hints": {
      "track_patterns": ["1 video track + 1 audio track", "text-heavy B-roll"],
      "broll_ratio": 0.3
    }
  },
  "workflow": {
    "approach": "audio_driven",
    "stages": [
      {
        "id": "import", "name": "导入素材", "order": 0,
        "actions": [
          {"tool": "import_media", "tip": "将口播、示意图分别导入"},
          {"tool": "add_captions", "tip": "先做口播转写，获得时间戳字幕"}
        ]
      },
      {
        "id": "rough_cut", "name": "精剪口播", "order": 1,
        "actions": [
          {"tool": "split_clip", "tip": "在气口/句界处分割；删除啰嗦重复段"},
          {"tool": "ripple_delete_ranges", "tip": "删除废段后闭合空隙"}
        ]
      },
      {
        "id": "broll", "name": "贴示意图", "order": 2,
        "actions": [
          {"tool": "search_media", "tip": "按口播语义匹配示意图"},
          {"tool": "add_clips", "tip": "B-roll 放 V2 轨道，对齐口播时长"}
        ]
      },
      {
        "id": "audio", "name": "音频精修", "order": 3,
        "actions": [
          {"tool": "set_clip_properties", "tip": "BGM 音量 20%，口播段压低到 10%"}
        ]
      }
    ],
    "rules": {
      "do": [
        "每段科普内容应配至少一张示意图",
        "关键术语出现时应在画面上叠文字标注",
        "段落过渡用 J-cut：声音先入，画面后切"
      ],
      "dont": [
        "不要连续 3 段以上无 B-roll 覆盖",
        "不要在示意图上叠加过多文字",
        "不要使用过于花哨的转场"
      ]
    }
  },
  "track_roles": {
    "V1": {"role": "MainCamera", "label": "口播主画面"},
    "V2": {"role": "B_RollOverlay", "label": "示意图"},
    "A1": {"role": "VoiceOver", "label": "口播音轨", "locked": true},
    "A2": {"role": "BGM", "label": "背景音乐"}
  }
}
```

## 2. 插件加载与激活

| 激活方式 | 触发条件 |
|---|---|
| **自动匹配** | 软件检测到工程特征匹配 `video_type.detection_hints`，自动推荐激活 |
| **手动选择** | 用户在工程设置中显式选择工作流 |
| **Agent 指定** | Agent 调用 MCP 工具 `activate_workflow` 指定 |

## 3. 插件如何影响 Agent

### 3.1 系统提示词注入

插件激活后，`instructions.md` 的内容被注入 Agent 的系统提示词，附带当前轨道角色映射和工作流规则。

### 3.2 工具返回增强

每次工具调用返回时，附加工件流上下文中该阶段的操作提示。见 [AGENT-CONTEXT-SIGNAL.md](AGENT-CONTEXT-SIGNAL.md)。

### 3.3 规则校验

`plugin.json` 中的 `workflow.rules` 在 Agent 编辑操作时被软件用作校验规则。违反 `dont` 规则时返回 warning 信号。

## 4. 插件分发

### 4.1 官方仓库

在线仓库 `plugins.opentake.io` 提供插件索引与下载，按视频类型标签分类。

### 4.2 本地开发

```
opentake plugin create my-workflow    # 创建新插件
opentake plugin validate my-workflow/ # 验证格式
opentake plugin package my-workflow/  # 打包分发
```

## 5. 与 Core 的关系

插件系统不修改 Rust core 的编辑逻辑。它完全是 **Agent 层的能力**：
- `plugin.json` → 在 MCP server 启动时或 `activate_workflow` 被调用时加载
- `instructions.md` → 注入系统提示词
- `rules` → MCP server 在每次工具调用时校验

不需要 Rust 代码编译，不需要 WASM 运行时。纯 JSON + Markdown，完全在 Agent 层运作。

## 6. 实现路线

| 阶段 | 内容 |
|---|---|
| Phase W1（随 Phase 7） | 插件格式定义 + `activate_workflow` MCP 工具 + `instructions.md` 注入 |
| Phase W2（随 Phase 7） | `plugin.json` 中 `workflow.rules` 的校验引擎 |
| Phase W3（后续） | 在线插件仓库 + `opentake plugin` CLI |
| Phase W4（后续） | 插件市场前端（可选） |

## 7. 相关文档

- [AGENT-CONTEXT-SIGNAL.md](AGENT-CONTEXT-SIGNAL.md) — Agent 上下文信号系统
- [ARCHITECTURE.md](ARCHITECTURE.md) — §7 MCP + Agent 层
- [MOTION-GRAPHICS-PLUGIN.md](MOTION-GRAPHICS-PLUGIN.md) — Web 动效插件系统（执行层插件）
