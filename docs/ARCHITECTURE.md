# OpenTake 架构设计

> 基于对 palmier-pro-upstream(Swift macOS 视频编辑器)的逐模块拆解综合得出。
> 目标:忠实复刻其编辑逻辑,做成跨平台(macOS/Windows/Linux)、开源(GPL-3.0)、内置更强提示词与能力的版本。
> 详见 `docs/MODULE-PORT-MAP.md`(20 模块逐项规格)与 `docs/_analysis/`(4 份横切报告)。

## 0. 一句话洞察

上游本质是 **「单一可观测状态容器(EditorViewModel)+ 帧本位不可变值类型模型(Timeline)+ AVFoundation 投影层」**。
- **领域模型 + 编辑算法几乎是纯函数式值语义** → 可近乎机械地 1:1 移植到 Rust。
- **唯一的"脏"耦合是 `CompositionBuilder → AVFoundation`** → 换成 FFmpeg(编解码)+ wgpu(合成)。
- **UI 是纯消费者**(读 store、发命令,不持有领域逻辑) → React 重建。
- **编辑能力只有一处真实定义**(ToolExecutor → EditorViewModel),UI / 应用内 Agent / 外部 MCP 是它的三个对等客户端 → OpenTake 照搬「单一能力层、多前端」。

## 1. ⚠️ 最关键的工程现实:媒体引擎 = FFmpeg + wgpu(不是纯 FFmpeg)

可移植性审计的核心结论:**上游所有像素级合成都委托给 AVFoundation 的声明式合成器(`AVVideoComposition` + layer instructions + ramps),没有任何手动渲染、没有 Metal、没有 CoreImage。**

因此:
- **FFmpeg 负责**:解码、编码/导出、缩略图抽帧、(配合 Symphonia)波形——这些都有成熟 Rust 绑定,机械移植。
- **FFmpeg 单独做不到**:带任意关键帧曲线(smooth/hold/linear)的多轨叠加 + transform/crop/opacity ramp 合成。`filter_complex` 只能近似。
- **必须自建 wgpu 帧合成器**:每帧「解码源帧 → 采样关键帧 → 仿射变换/裁剪/混合 → 多轨合成」。这是**全项目的命门**,也是唯一无现成替代、必须从零的部分。

三大 blocker(按难度):
1. 🔴 **wgpu 帧合成器**(决定成败)
2. 🔴 **播放/预览引擎**(ffmpeg 解码 + wgpu 上屏 + cpal 音频,自写 A/V 同步、精确 seek、scrub 节流)
3. 🟠 **文字/字幕渲染**(cosmic-text 排版 + tiny-skia/Vello 光栅 → 纹理 → 合成器)

> 落地策略:**第一步就做合成器 PoC**(单轨视频 + 一个 transform 关键帧 + 一条字幕,导出帧与上游做像素 diff),用它验证整条 Rust core 路线,再铺开其余。

> 💡 **风险即机会**:这个被迫自建的 wgpu 合成器,同时是 OpenTake 反超上游的最大窗口。上游被 AVFoundation 声明式合成锁死,**做不了**特效/转场/调色/绿幕/蒙版;而这些本质都是「合成片元着色器里的像素数学」,wgpu 合成器天生适合承载。对标剪映的进阶能力深化见 [ADVANCED-FEATURES.md](ADVANCED-FEATURES.md)。

## 2. 分层架构

```
┌──────────────────────────────────────────────────────────────┐
│ React + TypeScript 前端 (渲染 + 交互,不持有领域逻辑)            │
│  • TimelineView(轨道/clip/playhead/拖拽,像素↔帧换算)           │
│  • Preview(<canvas>/WebGL 显示 Rust 合成帧 + DOM 叠字)          │
│  • Inspector / MediaPanel / AgentPanel                         │
│  • 状态(Zustand):Timeline 只读镜像 + UI-only 态(selection/zoom)│
└───────────────┬──────────────────────────────────────────────┘
                │ Tauri invoke(命令) + event(状态推送)
┌───────────────▼──────────────────────────────────────────────┐
│ Tauri command 边界 (薄胶水:序列化 + 路由)                       │
│  invoke: edit_apply / project_open|save / undo|redo /          │
│          get_timeline / seek / import_media / export_start     │
│  event:  timeline_changed{version} / preview_frame / progress  │
└───────────────┬──────────────────────────────────────────────┘
┌───────────────▼──────────────────────────────────────────────┐
│ Rust core (真相源 + 全部领域逻辑)                               │
│  domain(值类型) · ops(编辑命令+UndoStack) · project(serde) ·    │
│  command(唯一编辑入口=上游 ToolExecutor) · render(RenderPlan)    │
│         ▲                          │                            │
│         │ MCP server(rmcp,内嵌)    │ 调用                        │
│  in-app agent(reqwest→Anthropic)   ▼                            │
│                          media: FFmpeg(编解码) + wgpu(合成) +    │
│                                 cpal(音频) + whisper-rs(转写)    │
└───────────────────────────────────────────────────────────────┘
```

**真相源在 Rust,前端只持镜像。** 上游是单进程单实例所以 UI 与 MCP 天然一致;OpenTake 跨进程,必须由 Rust 持有权威 `Timeline`,前端拿快照 + 单调递增版本号(对应上游 `timelineRenderRevision`),每次 `edit_apply` 广播 `timeline_changed{version}`,前端据此重取。

## 3. Cargo workspace 布局

```
OpenTake/
├── crates/
│   ├── opentake-domain/      # 值类型模型:Timeline/Track/Clip/Keyframe/Transform/Crop/TextStyle/MediaAsset
│   │                         #   + 派生函数(end_frame/source_frames_consumed/*_at 采样/fade)。零 IO,纯逻辑,可全单测
│   ├── opentake-ops/         # 编辑算法:OverwriteEngine/RippleEngine/SnapEngine(纯函数)+ EditCommand::apply + UndoStack
│   ├── opentake-project/     # .opentake 目录包读写(serde,#[serde(default)] 容错)+ 媒体清单/归档
│   ├── opentake-render/      # RenderPlan(Timeline→每帧合成指令,纯函数)+ wgpu 合成器 + ffmpeg 编解码后端
│   ├── opentake-media/       # 解码/缩略图/波形/转写/语义搜索(ffmpeg-next/symphonia/whisper-rs/ort)
│   ├── opentake-agent/       # 工具层(=ToolExecutor)+ MCP server(rmcp)+ 应用内 chat 客户端 + 短ID + 系统提示词
│   ├── opentake-gen/         # 生成后端客户端(BYOK 直连 / 自建代理),复刻 GenerationParams 联合类型
│   └── opentake-core/        # 组装:EditorState(持有 timeline+manifest)、command 路由、事件总线
├── src-tauri/                # Tauri 2 app:#[tauri::command] 薄封装 + 窗口/菜单/生命周期
├── web/                      # React + TS 前端(Vite)
├── services/
│   └── opentake-gen-proxy/   # 可选:开源可自托管生成代理(axum),对接 fal.ai/Replicate/...
└── docs/
```

依赖法则(经上游验证):`domain` 零依赖叶子;`ops` 只依赖 `domain`;`command` 是唯一编辑入口;UI/Agent/MCP 是命令层三个对等客户端。

> 后期新增 `crates/opentake-motion/`(Web 动效模块 / 插件宿主,Agent 用 HTML/CSS/JS 写动画,见 [MOTION-GRAPHICS-PLUGIN.md](MOTION-GRAPHICS-PLUGIN.md))。它只是「又一个片段源」接入 `opentake-render`,不改动上述 8 个核心 crate 的边界。

## 4. 领域模型(可直接复刻,见 MODULE-PORT-MAP.md「Models」)

要点(务必保持语义一致以便与上游工程对拍):
- **一切以整数帧为单位**;fps 为工程级常量;秒只在 import/源媒体交互时换算。`seconds→frame` 用 **截断**(`Int(s*fps)`)而非四舍五入。
- `Timeline{fps,width,height,tracks}` → `Track{type,muted,hidden,sync_locked,clips}` → `Clip{...}` 全是 `Clone+Serialize+PartialEq` 的值类型。
- Clip 派生逻辑(纯函数,必须原样移植):`end_frame`、`source_frames_consumed=round(duration*speed)`、`opacity_at/volume_at/transform_at/crop_at`(先采样关键帧→叠加 fade→乘静态值)、`fade_multiplier`(in/out 取 min,linear/smoothstep)。
- 关键帧:**存储用 clip 相对帧偏移,公开 API 用绝对时间线帧**(`to_offset`/`to_abs`);采样端点钳制 + 按左端点 `interpolation_out` 决定 hold/linear/smooth;`smoothstep(t)=t*t*(3-2t)`。
- 媒体:**运行时富对象 `MediaAsset` 与磁盘 `MediaManifestEntry` 分离**;clip 永不存路径,只存 `media_ref`(=asset id);`MediaSource::Project(rel)|External(abs)`。

**进阶能力的 domain 扩展**(对标剪映,详见 [ADVANCED-FEATURES.md](ADVANCED-FEATURES.md);全部 `#[serde(default)] + Option`,不破坏旧工程):
- `Clip` 新增:`effects: Vec<Effect>`(着色器特效链)、`masks: Vec<Mask>`(线性/圆形/钢笔)、`chroma_key: Option<ChromaKey>`(绿幕)、`color_grade: Option<ColorGrade>`(浮点调色链)、`speed_curve: Option<KeyframeTrack<f64>>`(曲线变速)。
- `Interpolation` 扩展:在 linear/hold/smooth 之外加 `Bezier(c1,c2)` / `Spring(..)`(物理级缓动)。
- 新增 `Transition{kind,duration,params}`(作用于相邻 clip 重叠区);`MediaSource::Nested(child_timeline_id)`(复合片段嵌套)。

## 5. 编辑命令层(唯一入口 = 上游 ToolExecutor)

UI 手势、应用内 Agent、外部 MCP **全部归一到一个 `EditCommand` 枚举**,撤销/校验/遥测只写一遍:

```rust
enum EditCommand {
    AddClips{..}, InsertClips{..}/*ripple*/, MoveClips{..}, RemoveClips{..},
    SplitClip{clip_id, at_frame}, TrimClips{..}, SetClipProperties{..},
    SetKeyframes{clip_id, property, keyframes}, RippleDeleteRanges{..},
    AddTexts{..}, AddCaptions{..}, Link{..}, Unlink{..},
    RemoveTracks{..}, CreateFolder{..}, MoveToFolder{..}, Undo, Redo,
}
struct EditResult { changed: bool, action_name: String, affected_clip_ids: Vec<String>, timeline_version: u64, summary: String }
```

`command::apply` 实现 = 上游 `withTimelineSwap` 事务模式:
`快照 timeline → 改 → 若 before != after(PartialEq 短路)→ 压 UndoStack(整树快照)→ version+1 → 广播 timeline_changed`。
**撤销栈在 Rust,整树快照(`Timeline` derive `Clone`)**,前端不做撤销。

编辑算法纯函数(直译 + 单测对拍):`OverwriteEngine::compute_overwrite`(覆写重叠 clip 的 remove/trim/split)、`RippleEngine`(收缩/推开/按 range 收缩 + 合并)、ripple 的**拒绝语义**(sync-locked 跟随轨收缩会碰撞/越 0 则整体拒绝而非破坏对齐)、split(`left_source=round(offset*speed)`,6 条关键帧轨在切点各插边界关键帧)、A/V 链接(同 `link_group_id` 作为一个单位 move/trim/split/delete)。

## 6. 渲染管线:RenderPlan + 双 FFmpeg 后端 + wgpu

把上游 `CompositionBuilder.buildVisuals`(关键帧→分段线性 ramp,smooth 段按 `smoothSegments=8` 细分)移植成 **Rust 纯函数 `Timeline → RenderPlan`**(每 clip 每帧的属性值),再交执行层:
- **预览后端**:低延迟(ffmpeg 解码到最近关键帧+丢帧到目标 → wgpu 合成单帧 → 上屏)。
- **导出后端**:全质量(wgpu 逐帧合成 → ffmpeg 编码;预设 H.264/H.265/ProRes × 720p/1080p/4K)。
- 两者**共享同一个 RenderPlan**,保证预览与导出像素一致。
- **媒体物化策略照搬**:图片/文字/Lottie 在合成前物化为纹理(content-hash 缓存);自建合成器后,上游「图片烧成静止视频」「Lottie 烧 ProRes」这类 hack **整类消失**。

媒体能力 → OpenTake 栈映射:

| 能力 | OpenTake | 风险 |
|---|---|---|
| 合成器(关键帧 ramp/transform/crop) | **wgpu 自写**(语义照搬上游公式) | 🔴 blocker |
| 解码/读帧 | ffmpeg-next | 中(成熟) |
| 编码/导出 | ffmpeg-next(x264/x265/ProRes) | 中(预设需对齐) |
| 播放 | 自建(ffmpeg+wgpu+cpal,自写同步/seek) | 🔴 blocker |
| 字幕/文字 | cosmic-text + tiny-skia/Vello → 纹理 | 🟠 |
| 缩略图 | ffmpeg seek + image crate(sprite 网格缓存照搬) | 低 |
| 波形 | Symphonia 解 PCM + RMS 降采样 | 低 |
| 转写 | whisper-rs(word/segment 时间戳)| 中 |
| 语义搜索(CLIP) | candle/ort 跑 SigLIP2 + tokenizers | 中 |
| Lottie | rlottie FFI / velato → 纹理 | 中 |
| 字体 | fontdb + cosmic-text | 低 |

## 7. MCP + Agent(单一能力层、双前端)

- **MCP server**:用官方 **`rmcp`**(streamable-http-server feature,基于 axum)起在 `127.0.0.1:19789`。**只绑 loopback + Origin 校验**(DNS-rebinding 防护,用 tower layer 复刻),Claude Desktop 经 stdio→HTTP shim 接入。
- **31 个工具**(读 7 / 时间线编辑 11 / 生成导入 5 / 库组织 7 / Resources 2)全是 `EditorCore` 方法的薄包装——见 MODULE-PORT-MAP.md「Agent」与横切报告 04。工具描述字符串(承载行为契约)**原样照搬**。
- **必须复刻的三个横切机制**:① 短 ID 系统(出站缩短 UUID 到唯一最短前缀≥8 字符、入站展开,省 token)② 统一执行壳(快照→展开ID→执行→变更则压 agentUndoStack→遥测→缩短ID)③ 面向 LLM 的精确路径错误(`entries[3].startFrame: missing required field`,用 `serde_path_to_error`)。
- **应用内 chat 与 MCP 共享同一套工具 + 同一系统提示词**;chat 走 reqwest→Anthropic(BYOK)或自建代理;复刻 prompt caching(system+tools+会话前缀打 ephemeral)。
- **OpenTake 增强点**(超越上游):系统提示词从单块字符串升级为**分层可组合 + 模型策略从配置注入**;新增高阶工具 `remove_filler_words`/`tighten_silences`(把易错的帧算术在 Rust 内一次完成);写工具统一返回**结构化 JSON**;新增 `get_capabilities`(一次性返回 ASR/索引/生成/编解码就绪状态)。

## 8. 生成式 AI 后端(自建,因上游闭源)

上游是瘦客户端,生成全走闭源 Convex+Clerk+Stripe,客户端从不直连 fal/Replicate。但**客户端↔后端契约完整暴露**(params 联合类型、job 状态机、模型目录 schema),是现成蓝图。

OpenTake 设计:
- **双模运行**:① **BYOK 模式**(Rust core 用用户自带 fal/Replicate/OpenAI key,**完全本地直连厂商,零后端、零运营成本**,自托管核心卖点)② **托管模式**(自建 `opentake-gen-proxy`,axum 单二进制,持厂商 key + 积分计费)。
- **沿用上游干净抽象**:`model id` 不透明字符串;`/v1/models` 运行时下发能力矩阵 + 定价,客户端数据驱动动态适配;后端只吃 URL(素材先预签名上传);统一 job 抽象 `{queued→running→succeeded/failed}+resultUrls`;每厂商一个 adapter。
- 密钥存 OS keychain(`keyring` crate),绝不入工程文件。

## 9. 持久化:`.opentake` 目录包

沿用上游目录包格式(Tauri 下就是普通目录,跨平台无 file-package 概念反而更简单),字段与上游 `project.json` 保持一致以便迁移与对拍:

```
MyProject.opentake/
├── project.json         # Timeline
├── media.json           # MediaManifest(entries + folders)
├── generation-log.json  # AI 生成审计(append-only)
├── thumbnail.jpg
├── media/               # 内部媒体(.project 相对路径指向这里)
└── chat-sessions/       # Agent 对话历史(每会话一个 json)
```

所有 serde 模型用 `#[serde(default)]` + `Option<T>` 实现向后兼容容错解码(对应上游手写容错 decode)。

## 10. 技术选型清单

| 关注点 | 选型 |
|---|---|
| 桌面外壳 | Tauri 2 |
| 前端 | React + TypeScript + Vite;状态 Zustand |
| 核心语言 | Rust(workspace,多 crate) |
| 编解码 | ffmpeg-next(libav*) |
| 合成 | wgpu(自写帧合成器) |
| 音频播放 | cpal |
| 2D 光栅/文字 | cosmic-text + tiny-skia 或 Vello;字体 fontdb |
| 波形 | symphonia |
| 转写 | whisper-rs(whisper.cpp) |
| 语义搜索 | candle 或 ort(onnxruntime)+ tokenizers,模型 SigLIP2 |
| MCP | rmcp(streamable-http-server) |
| LLM 客户端 | reqwest + eventsource-stream(SSE) |
| 生成代理(可选) | axum 单二进制 |
| 密钥存储 | keyring |
| 序列化 | serde / serde_json |
```
