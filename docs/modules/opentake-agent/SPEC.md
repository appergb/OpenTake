# opentake-agent 实现就绪规格（Issue #9）

> 范围：`crates/opentake-agent` —— rmcp MCP server（`127.0.0.1:19789`）+ 31 个工具 + 短 ID 系统 + 统一执行壳 + 面向 LLM 的精确路径错误 + 应用内 chat（reqwest→Anthropic，BYOK + prompt cache）+ **Agent Context Signal 注入** + **Workflow Plugin 系统**。
>
> 设计来源（已逐行核读）：上游 `palmier-pro-upstream/Sources/PalmierPro/Agent/`（29 文件），以及 OpenTake `docs/AGENT-CONTEXT-SIGNAL.md`、`docs/WORKFLOW-PLUGIN-SYSTEM.md`、`docs/ARCHITECTURE.md §7/§9`、`docs/MODULE-PORT-MAP.md`「Agent」、`docs/_analysis/04-MCP与Agent工具.md`、`docs/ROADMAP.md` Phase 7/S/W。
>
> 核心架构原则（上游验证，OpenTake 照搬）：**编辑能力只有一处真实定义**（`opentake-core` 的 `EditCommand` 路由 → `opentake-ops`），**MCP server 与应用内 chat 是它的两个对等前端**，不写两套。Agent 层「非常薄」——31 个工具是 `opentake-core` 命令的薄包装；真正的编辑算法在 `opentake-ops`/`opentake-domain`（不在本 crate）。
>
> 约束注记：本规格只描述 `opentake-agent`。它**不实现**编辑算法（`opentake-ops`）、领域模型（`opentake-domain`）、媒体引擎（`opentake-media`）、渲染（`opentake-render`）、生成后端（`opentake-gen`）——这些由各自 crate 提供，本 crate 通过 `opentake-core` 调用（见 §8）。

---

## 0. 上游证据索引（所有结论的代码出处，绝对路径）

| 主题 | 上游文件 | 关键行 |
|---|---|---|
| MCP HTTP server（TCP 外壳 + 路由 + loopback 绑定） | `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/MCP/MCPHTTPServer.swift` | 绑定 `:23-27`；连接级校验管线 `:45-55`；路由 `:66-91` |
| MCP server 装配（端口、Server 注册、工具/资源映射、参数桥接） | `…/Agent/MCP/MCPService.swift` | `port=19789 :9`；`enabledKey :11`；`start() :35-61`；`registerTools :72-94`；`registerResources :96-119` |
| 31 工具名 + schema + 描述 | `…/Agent/Tools/ToolDefinitions.swift` | `ToolName :4-36`（30 case + undo）；`all :45-588`；`objectSchema :590` |
| 短 ID 系统（出站缩短/入站展开/歧义报错） | `…/Agent/Tools/ToolExecutor+ShortId.swift` | `idPrefixFloor=8 :8`；`scalarIdKeys :10`；`arrayIdKeys :16`；`uuidRegex :22`；`currentIdUniverse :26`；`shorteningIds :43`；`shortIdMap :54`；`expandingIdPrefixes :68`；`expandOne :91` |
| 统一执行壳 + undo 守卫 + 错误格式化 + 参数校验 | `…/Agent/Tools/ToolExecutor.swift` | `execute :22-70`；`run dispatch :72-106`；`undo :109-123`；`validateUnknownKeys :166`；`decodeToolArgs :177`；`firstNonFiniteNumberPath :194`；`formatDecodingError :210` |
| 中立结果类型 + MCP 转换 | `…/Agent/Tools/ToolResult.swift` | `Block :5`；`ok/error :13-19`；`toMCPResult :23` |
| 系统提示词（唯一一处） | `…/Agent/Tools/AgentInstructions.swift` | `serverInstructions :4-143` |
| 应用内 chat（agentic loop、上下文、tool loop、孤儿修复） | `…/Agent/AgentService.swift` | `selectClient :52`；`send :297`；`runLoop :341`；`runPendingToolUses :422`；`resolveOrphanToolUses :458`；`apiMessages :514`；`inlineImageBlocks :529` |
| Anthropic 直连客户端（BYOK） | `…/Agent/Clients/AnthropicClient.swift` | `endpoint :37`；`run header/body :57-86`；keychain + DEBUG env `:7-30` |
| 共享 SSE 解析 + 请求体构造 + prompt cache 边界 | `…/Agent/Clients/AgentClientTypes.swift` | `AnthropicModel :5`（含 `claude-opus-4-8`）；`AgentClient :63`；`AnthropicSSE.parse :88`；`AnthropicRequestBody.build :156`（cache_control `:168/:179/:188`） |
| 后端代理客户端（计费通道，OpenTake 替换） | `…/Agent/Clients/PalmierClient.swift` | `endpoint v1/agent/stream :35`；错误信封 `:80-101` |
| get_timeline 编码（轨道/clip 结构 + 压缩规则，Context Signal 检测依据） | `…/Agent/Tools/ToolExecutor+Timeline.swift` | `getTimeline :17`；`trackDefaults :60`（`muted/hidden/syncLocked`）；`clipDefaults :62`；`compactTrack :73`；`compactClip :112` |
| add_clips 行为（覆写、自动建轨、linked audio、全有或全无） | `…/Agent/Tools/ToolExecutor+Clips.swift` | `AddClipsInput :5`；`addClips :13`；`Mixed trackIndex :171-174`；`insertTrack :194/:199`；`clearRegion+placeClip :225-226` |
| OpenTake 目标 crate 边界 + §7 MCP 设计 | `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/docs/ARCHITECTURE.md` | crate 布局 `:64-87`；`EditCommand/EditResult :105-116`；§7 `:148-154`；§9 目录 `:165-177` |
| Context Signal 全设计 | `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/docs/AGENT-CONTEXT-SIGNAL.md` | 发射时机表 `:37-47`；数据结构 `:50-83`；插件叠加 `:88-98`；类型检测 `:104-140`；轨道角色 `:148-173`；规则 `:177-203` |
| Workflow Plugin 全设计 | `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/docs/WORKFLOW-PLUGIN-SYSTEM.md` | 目录 `:18-24`；plugin.json schema `:28-96`；激活 `:100-104`；影响 Agent `:108-118`；与 Core 关系 `:136-141` |
| Phase 7 / S / W 验证标准 | `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/docs/ROADMAP.md` | Phase 7 `:52-59`；Phase S `:99-110`；Phase W `:113-119` |

---

## 1. rmcp MCP server：`127.0.0.1:19789` + loopback/Origin 校验（tower layer）

### 1.1 上游真相

上游不是用 SDK 内置 listener，而是「**自定义 TCP 外壳（Apple `Network.framework` `NWListener`）+ SDK 无状态 transport 内核（`StatelessHTTPServerTransport`）**」。关键三点：

1. **只绑 IPv4 回环**：`params.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: 19789)`，注释「never reachable from the LAN」（`MCPHTTPServer.swift:23-27`）。
2. **每条 TCP 连接 = 一对全新 `Server` + `Transport`**（stateless，无跨连接会话）（`MCPService.swift:36-49`）。
3. **连接级校验管线**（`MCPHTTPServer.swift:46-50`）：
   - `OriginValidator.localhost(port: 19789)` —— DNS-rebinding 防护（校验 `Origin`/`Host` ∈ {localhost, 127.0.0.1, [::1]} + 端口）。
   - `ContentTypeValidator()` —— 必须 `application/json`。
   - `ProtocolVersionValidator()` —— 校验 `MCP-Protocol-Version`。
   - （注：上游**省略**了 SDK 默认的 `AcceptHeaderValidator(.jsonOnly)`，放宽 Accept 头。OpenTake 也省略，避免 Claude Desktop shim 的 Accept 头不匹配。）

路由（`MCPHTTPServer.swift:66-91`）：

| 路径 / 方法 | 行为 |
|---|---|
| `POST /mcp` 或 `POST /` | 核心：`transport.handleRequest` → 写回 JSON-RPC 响应 |
| `GET /mcp` / `GET /` | 返回 `text/event-stream` 的 `: connected\n\n`（SSE 占位，无实际推流） |
| `GET /.well-known/oauth-protected-resource` | `{"resource":"http://127.0.0.1:19789"}` |
| 其它 | 404 |

对外发布形态：`Resources/MCPB/palmier-pro.mcpb` 是一个 Node.js stdio→HTTP shim（`mcp-remote → http://127.0.0.1:19789/mcp`），给 Claude Desktop 用。安全边界**仅靠「绑 loopback + Origin 校验」**。

### 1.2 OpenTake Rust 设计（rmcp + axum + tower）

Rust 侧比 Swift 更省事：rmcp 的 `streamable-http-server` feature 自带基于 axum/hyper 的 Streamable HTTP transport，**不必手写 TCP 外壳**。只需把 axum listener 绑死 loopback，并用 `tower::Layer` 补三个校验（rmcp 不内置 Origin 校验）。

```toml
# crates/opentake-agent/Cargo.toml（依赖清单，非完整）
rmcp            = { version = "*", features = ["server", "transport-streamable-http-server"] }
axum            = "*"
tower           = "*"
tower-http      = "*"
reqwest         = { version = "*", features = ["json", "stream"] }
eventsource-stream = "*"   # Anthropic SSE
serde           = { version = "*", features = ["derive"] }
serde_json      = "*"
serde_path_to_error = "*"  # 面向 LLM 的精确路径错误（§4）
schemars        = "*"       # 工具入参 JSON Schema 自动派生（可选，见 §2.3）
tokio           = { version = "*", features = ["full"] }
keyring         = "*"       # OS keychain 存 Anthropic key（§5）
regex           = "*"       # 短 ID UUID 扫描（§3）
```

#### 1.2.1 绑定 + 幂等开关（照搬上游语义）

- 端口常量：`pub const MCP_PORT: u16 = 19789;`（沿用，见 ARCHITECTURE §7 与 ROADMAP）。
- 偏好键：`io.opentake.mcp.enabled`，**缺省 true**（对应上游 `io.palmier.pro.mcp.enabled`，`MCPService.swift:11-22`）。落到 Tauri Store / `settings.json`。
- 启动幂等：「已运行则幂等、偏好关闭不启动」（ARCHITECTURE `:1060`）。
- 绑定地址**必须** `SocketAddr` = `127.0.0.1:19789`（`Ipv4Addr::LOCALHOST`）。**禁止** `0.0.0.0`。

```rust
// 伪代码：server 装配
pub struct McpServer { core: Arc<CoreHandle>, signal: Arc<ContextSignalEngine>, plugins: Arc<PluginRegistry> }

pub async fn start(core: Arc<CoreHandle>, /* signal, plugins */) -> anyhow::Result<JoinHandle<()>> {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), MCP_PORT); // 锁回环
    let listener = tokio::net::TcpListener::bind(addr).await?;            // 绑死 loopback
    let mcp_service = StreamableHttpService::new(/* stateless 配置 */ move || ToolServer::new(core.clone()));
    let app = axum::Router::new()
        .nest_service("/mcp", mcp_service)
        .route("/.well-known/oauth-protected-resource", get(well_known)) // {"resource":"http://127.0.0.1:19789"}
        .layer(tower::ServiceBuilder::new()
            .layer(OriginGuardLayer::localhost(MCP_PORT))   // ① DNS-rebinding 防护
            .layer(ContentTypeGuardLayer::json())           // ② application/json
            .layer(ProtocolVersionGuardLayer::new()));      // ③ MCP-Protocol-Version
    Ok(tokio::spawn(async move { axum::serve(listener, app).await.ok(); }))
}
```

#### 1.2.2 三个 tower layer（**必须保留，DNS-rebinding 防护**）

`OriginGuardLayer`（对应 `OriginValidator.localhost`）：
- 若请求带 `Origin` 头：解析其 host:port，**必须** host ∈ {`localhost`,`127.0.0.1`,`[::1]`,`::1`} 且 port == 19789（或无端口时按 80/443 拒绝——本地服务只接受显式 19789 或同源无 Origin 的 stdio shim）。不匹配 → `403 Forbidden`。
- 同时校验 `Host` 头同上集合（防 rebinding）。
- 无 `Origin`（典型：stdio→HTTP shim、`claude mcp add` 的本地连接）→ 放行（上游 `OriginValidator.localhost` 同样对无 Origin 宽容；本地回环已是第一道边界）。

`ContentTypeGuardLayer`：仅对 `POST /mcp`，`Content-Type` 必须以 `application/json` 起始，否则 `415`。

`ProtocolVersionGuardLayer`：若带 `MCP-Protocol-Version` 头则校验为受支持版本（rmcp 协商版本集合），不匹配 `400`；缺失则按 rmcp 默认协商。

> 验证（ROADMAP Phase 7 `:59`）：`claude mcp add --transport http http://127.0.0.1:19789/mcp` 能连；从 `192.168.x.x` 或带伪造 `Origin: http://evil.com` 的请求被 403/拒绝。

#### 1.2.3 stdio→HTTP shim（Claude Desktop 接入，对外发布形态）

照搬上游 `.mcpb` 思路，打包一个等价 shim（`mcp-remote → http://127.0.0.1:19789/mcp`），由 OpenTake 打包流程产出，或在 Help/Settings 面板给出纯 JSON 手动配置指引（ARCHITECTURE `:992`、`:948`：MCP 接入向导为 4 种客户端拼接 `http://127.0.0.1:19789/mcp` 派生配置）。本 crate 只需保证 HTTP server 行为正确；shim 打包属 packaging 任务。

### 1.3 server 元数据 + 能力（照搬 `MCPService.start()`）

```
name:    "opentake"
version: "1.0.0"
instructions: <见 §6.4 组装后的系统提示词（base + 激活插件的 instructions.md）>
capabilities: { resources: { subscribe:false, listChanged:false },
                tools:     { listChanged:false } }
```

注册：`ListTools` / `CallTool` / `ListResources` / `ReadResource`（`MCPService.swift:77-119`）。

**Resources（2 个，只读，非工具）**（`MCPService.swift:96-133`）：
- `opentake://models/video` → 视频模型目录 JSON（来自 `opentake-gen` 的 `ModelCatalog`）。
- `opentake://models/image` → 图片模型目录 JSON。
（上游 URI 前缀 `palmier://` 改 `opentake://`。）

---

## 2. 31 个工具完整提取（工具名 + 关键参数 + 行为描述）

> **来源**：`ToolDefinitions.swift:45-588`（描述字段 `description:`）。**描述字符串原样保留**（ARCHITECTURE §7 `:151`「工具描述字符串承载行为契约，原样照搬」）。下表「关键参数」列出 schema 字段；`allowedKeys`（用于严格未知字段校验，§4）来自 `ToolExecutor+Clips.swift` 等各 `DecodableToolArgs`。
>
> **实现指令**：每个工具的 `description` 直接复制本节附带的 JSON（§2.2）中的字符串到 Rust 工具注册，**一字不改**（含 `\n`、`•`、emoji 等）。Rust 侧分发到 `opentake-core` 的对应 `EditCommand`（§8）。

### 2.1 工具清单（31 个，分域 + 关键参数 + 背后命令）

枚举顺序严格按 `ToolName`（`ToolDefinitions.swift:4-36`）：`get_timeline, get_media, add_clips, insert_clips, remove_clips, remove_tracks, move_clips, set_clip_properties, set_keyframes, split_clip, ripple_delete_ranges, undo, add_texts, add_captions, generate_video, generate_image, generate_audio, upscale_media, import_media, list_models, inspect_media, get_transcript, inspect_timeline, search_media, list_folders, create_folder, move_to_folder, rename_media, rename_folder, delete_media, delete_folder`。

#### A. 读 / 内省（只读，7 个）

| # | 工具 | 关键参数（schema 字段；`*`=required） | required 字段 | 背后命令（opentake-core） | Context Signal 附加（§6） |
|---|---|---|---|---|---|
| 1 | `get_timeline` | `startFrame?:int`, `endFrame?:int`（窗口分页） | — | `core.timeline_snapshot(window)` → 压缩编码 | `video_classification`+`track_roles`+`editing_stage`+`stage_guidance` |
| 2 | `get_media` | （无） | — | `core.media_manifest()` | — |
| 3 | `inspect_media` | `mediaRef*:string`, `clipId?:string`, `maxFrames?:int(≤12)`, `startSeconds?:number`, `endSeconds?:number`, `wordTimestamps?:bool`, `overview?:bool` | `mediaRef` | `opentake-media`：FFmpeg 抽帧 + whisper 转写 | `clip_analysis_hint`（镜头类型/景别） |
| 4 | `get_transcript` | `startFrame?:int`, `endFrame?:int`, `clipId?:string` | — | `opentake-media`：遍历时间线音/视轨，映射 trim/speed/position | `break_analysis`（气口/句界/重复/啰嗦） |
| 5 | `inspect_timeline` | `startFrame?:int`, `endFrame?:int`, `maxFrames?:int(≤12)` | — | `opentake-render`：合成帧（transform/opacity/crop/keyframe + 文字烧入） | — |
| 6 | `search_media` | `query*:string`, `scope?:enum{visual,spoken,both}`, `mediaRef?:string`, `limit?:int(≤50)` | `query` | `opentake-media`：CLIP 视觉 + 转写口语检索 | `material_match_hint`（B-roll 匹配优先级） |
| 7 | `list_models` | `type?:enum{video,image,audio,upscale}` | — | `opentake-gen`：`ModelCatalog` | — |

#### B. 时间线编辑（写，11 个）——核心剪辑能力

| # | 工具 | 关键参数 | required | `allowedKeys`（顶层 / Entry） | 背后命令 | Context Signal 校验 |
|---|---|---|---|---|---|---|
| 8 | `add_clips` | `entries[]`{`mediaRef*`,`trackIndex?`,`startFrame*`,`durationFrames*`,`trimStartFrame?`,`trimEndFrame?`} | `entries`；entry: `mediaRef,startFrame,durationFrames` | `{entries}` / `{mediaRef,trackIndex,startFrame,durationFrames,trimStartFrame,trimEndFrame}` | `EditCommand::AddClips`（覆写：clearRegion+placeClip；全省略 trackIndex 自动建共享轨；视频带音频自动建 linked audio；trackIndex 全有或全无） | `placement_validation`（轨道类型匹配 / A/V 拆分） |
| 9 | `insert_clips` | `trackIndex*`,`atFrame*`,`entries[]`{`mediaRef*`,`durationFrames?`,`trimStartFrame?`,`trimEndFrame?`} | `trackIndex,atFrame,entries`；entry:`mediaRef` | `{trackIndex,atFrame,entries}` / `{mediaRef,durationFrames,trimStartFrame,trimEndFrame}` | `EditCommand::InsertClips`（ripple：atFrame 及之后右移，sync-locked 轨 + linked audio 同步） | `placement_validation` |
| 10 | `remove_clips` | `clipIds*[]:string` | `clipIds` | `{clipIds}` | `EditCommand::RemoveClips`（link group 连带删；空轨 prune 并提示索引变） | 口播精剪规则（删主干 warning） |
| 11 | `remove_tracks` | `trackIndexes*[]:int` | `trackIndexes` | `{trackIndexes}` | `EditCommand::RemoveTracks`（余轨索引下移；其它轨 linked partner 不删） | — |
| 12 | `move_clips` | `moves[]`{`clipId*`,`toTrack?`,`toFrame?`}（每条至少一个 to*） | `moves`；move:`clipId` | `{moves}` / `{clipId,toTrack,toFrame}` | `EditCommand::MoveClips`（目标重叠覆写；linked partner 跟帧 delta，轨不传播） | 节奏/结构规则 |
| 13 | `set_clip_properties` | `clipIds*[]` + 任意组合 `durationFrames?`,`trimStartFrame?`,`trimEndFrame?`,`speed?`,`volume?`,`opacity?`,`transform?{centerX,centerY,width,height,flipHorizontal,flipVertical}`, 文字专用 `content?`,`fontName?`,`fontSize?`,`color?`,`alignment?{left,center,right}` | `clipIds` | `{clipIds,durationFrames,trimStartFrame,trimEndFrame,speed,volume,opacity,transform,content,fontName,fontSize,color,alignment}` | `EditCommand::SetClipProperties`（同值套全批；set volume/opacity 清该属性 keyframe；timing 传播 linked partner，文字伙伴跳过 trim/speed） | — |
| 14 | `set_keyframes` | `clipId*`,`property*:enum{volume,opacity,rotation,position,scale,crop}`,`keyframes*[][]`（`[frame,...values,interp?]`，interp∈{linear,hold,smooth}默认 smooth） | `clipId,property,keyframes` | `{clipId,property,keyframes}` | `EditCommand::SetKeyframes`（替换式，空数组清空；frame=clip 相对） | — |
| 15 | `split_clip` | `clipId*`,`atFrame*`（严格在 clip 内） | `clipId,atFrame` | （无专用结构，直接取参） | `EditCommand::SplitClip` | 口播精剪：不在词中间切 warning |
| 16 | `ripple_delete_ranges` | 二选一 `trackIndex?`/`clipId?`；`ranges*[][start,end]`；`units?:enum{seconds,frames}`默认 frames | `ranges` | `{clipId,trackIndex,ranges,units}` | `EditCommand::RippleDeleteRanges`（重叠合并；linked 同区间删；sync-locked 同步左移，放不下整体拒绝；返回 anchor 轨剪后布局） | `break_analysis` 一致性 |
| 17 | `undo` | （无） | — | — | `EditCommand::Undo` + agentUndoStack 守卫（§4.3） | — |
| 18 | `add_texts` | `entries[]`{`startFrame*`,`durationFrames*`,`content*`,`transform?{centerX,centerY,width,height}`,`fontName?`,`fontSize?`,`color?`,`alignment?`} | `entries`；entry:`startFrame,durationFrames,content` | `{entries}` / `{trackIndex,startFrame,durationFrames,content,transform,fontName,fontSize,color,alignment}` | `EditCommand::AddTexts`（overlay；同轨重叠覆写；同时显示需放不同轨；全省略 trackIndex 顶部新建一轨） | `text_placement_hint`（层级 / 安全区） |
| 19 | `add_captions` | `clipIds?[]`,`language?`,`fontName?`,`fontSize?`,`color?`,`centerX?`,`centerY?`,`textCase?:enum{auto,upper,lower}`,`censorProfanity?` | — | `{clipIds,language,fontName,fontSize,color,centerX,centerY,textCase,censorProfanity}` | `EditCommand::AddCaptions`（端侧转写 + 样式化 caption clip 到新轨；省略 clipIds 自动挑语音最多的轨） | `caption_style_hint` |

#### C. 媒体生成 / 导入（写，5 个）——媒体管线接入点（依赖 `opentake-gen`）

| # | 工具 | 关键参数 | required | 背后命令 |
|---|---|---|---|---|
| 20 | `generate_video` | `prompt*`,`name?`,`model?`,`duration?`,`aspectRatio?`,`resolution?`,`startFrameMediaRef?`,`endFrameMediaRef?`,`sourceVideoMediaRef?`,`sourceClipId?`,`referenceImageMediaRefs?[]`,`referenceVideoMediaRefs?[]`,`referenceAudioMediaRefs?[]`,`folderId?` | `prompt` | `opentake-gen`：异步提交，立即返回 placeholder asset ID；**花钱、不可撤销** |
| 21 | `generate_image` | `prompt*`,`name?`,`model?`,`aspectRatio?`,`resolution?`,`quality?`,`referenceMediaRefs?[]`,`folderId?` | `prompt` | `opentake-gen`：异步提交 placeholder |
| 22 | `generate_audio` | `prompt?`,`name?`,`model?`,`voice?`,`lyrics?`,`styleInstructions?`,`instrumental?`,`duration?`,`videoSourceStartFrame?`,`videoSourceEndFrame?`,`videoSourceMediaRef?`,`folderId?` | （无） | `opentake-gen`：TTS / 文生乐 / 视频配乐；时间线区间结果自动落轨 |
| 23 | `upscale_media` | `mediaRef*`,`model?`,`sourceClipId?` | `mediaRef` | `opentake-gen`：升分辨率 placeholder |
| 24 | `import_media` | `source*`{三选一 `url?`(HTTPS≤1GB)/`path?`(本地，可目录递归)/`bytes?`(base64≤~15MB)，`mimeType?`},`name?`,`folderId?` | `source` | `opentake-core`/`opentake-project`：url 后台下载、path/bytes 同步；扩展名白名单 |

#### D. 媒体库组织（写，7 个）——均可撤销，均支持「单条参数 或 `entries[]` 批量」二选一

| # | 工具 | 关键参数 | required | 背后命令 |
|---|---|---|---|---|
| 25 | `list_folders` | （无） | — | `core.folders()` |
| 26 | `create_folder` | `name?`+`parentFolderId?` **或** `entries[]`{`name*`,`parentFolderId?`} | （二选一） | `EditCommand::CreateFolder` |
| 27 | `move_to_folder` | `assetIds?[]`+`folderId?` **或** `entries[]`{`assetIds*`,`folderId?`} | （二选一） | `EditCommand::MoveToFolder` |
| 28 | `rename_media` | `mediaRef?`+`name?` **或** `entries[]`{`mediaRef*`,`name*`} | （二选一） | `core.rename_media` |
| 29 | `rename_folder` | `folderId?`+`name?` **或** `entries[]`{`folderId*`,`name*`} | （二选一） | `core.rename_folder` |
| 30 | `delete_media` | `assetIds*[]` | `assetIds` | `core.delete_media`（连带删引用 clip，同撤销步） |
| 31 | `delete_folder` | `folderIds*[]` | `folderIds` | `core.delete_folder`（连带删子文件夹/资源/clip） |

### 2.2 工具描述完整 JSON（供实现直接复制 `description` 字符串）

> 下方是 31 个工具的 `name` + `description`（**原样**，含换行/项目符号/emoji）+ `inputSchema`（已从 Swift 字典转 JSON）。实现时把每条 `description` 一字不差地放进 rmcp 工具注册的描述位；`inputSchema` 可直接作为 JSON Schema，或用 `schemars` 从对应 Rust 入参结构体派生后**人工核对字段名/描述一致**。
>
> 由于篇幅，完整 31 条以「工具名 → 上游行号」精确锚定（每条描述均在该范围内逐字可取）：

| 工具 | 描述行范围（`ToolDefinitions.swift`） | inputSchema 行范围 |
|---|---|---|
| `get_timeline` | `:48`（单条长字符串） | `:49-54` |
| `get_media` | `:58` | `:59` |
| `inspect_media` | `:63` | `:64-75` |
| `get_transcript` | `:79` | `:80-87` |
| `inspect_timeline` | `:90` | `:91-98` |
| `search_media` | `:101` | `:102-111` |
| `add_clips` | `:114` | `:115-136` |
| `insert_clips` | `:139` | `:140-161` |
| `remove_clips` | `:164` | `:165-175` |
| `remove_tracks` | `:178` | `:179-189` |
| `move_clips` | `:192` | `:193-211` |
| `set_clip_properties` | `:214` | `:215-248` |
| `set_keyframes` | `:251` | `:252-268` |
| `split_clip` | `:271` | `:272-279` |
| `ripple_delete_ranges` | `:282` | `:283-296` |
| `undo` | `:299` | `:300` |
| `add_texts` | `:304` | `:305-338` |
| `add_captions` | `:341` | `:342-355` |
| `generate_video` | `:358` | `:359-378` |
| `generate_image` | `:381` | `:382-395` |
| `generate_audio` | `:398` | `:399-416` |
| `upscale_media` | `:419` | `:420-428` |
| `import_media` | `:431` | `:432-449` |
| `list_models` | `:581` | `:582-587` |
| `list_folders` | `:452` | `:453` |
| `create_folder` | `:457` | `:458-476` |
| `move_to_folder` | `:479` | `:480-506` |
| `rename_media` | `:509` | `:510-528` |
| `rename_folder` | `:531` | `:532-550` |
| `delete_media` | `:553` | `:554-564` |
| `delete_folder` | `:567` | `:568-578` |

> **实现要点**：把这 31 条描述抽到 `crates/opentake-agent/src/tools/descriptions.rs`（`const`/`include_str!`），与 schema 一一对应。改写时**仅**把 `palmier`/`Palmier`/`palmier-pro` 字样替换为 `opentake`/`OpenTake`（例如 `get_timeline` 的「tell the user to sign in to Palmier and subscribe」、`canGenerate`）；产品名以外的行为契约文本**一字不改**。`palmier://models/*` 资源 URI → `opentake://models/*`。

### 2.3 工具入参 Schema 策略（Rust）

两条可选路线，**推荐前者**：

1. **手抄 JSON Schema（与上游 1:1）**：直接用 §2.2 的 `inputSchema` JSON 作为静态 schema。优点：与上游行为契约完全对齐，描述字段（schema 内嵌的 per-field `description`）也照搬，对 LLM 自纠正最有利。
2. **`schemars` 派生 + 人工核对**：为每个工具定义 `#[derive(Deserialize, JsonSchema)]` 入参结构体（`Option<T>` 表 optional，`Vec` 表 array），用 `#[schemars(description="...")]` 填字段描述。优点：与 `serde_path_to_error`（§4）天然配合。**风险**：派生出的 schema 字段顺序/描述需逐条核对与上游一致，否则削弱描述的契约性。

无论哪条，顶层 `additionalProperties` 行为由 §4 的「严格未知字段校验」补足（上游 `DecodableToolArgs.allowedKeys` 连嵌套 entry 也查，JSON Schema 的 `additionalProperties:false` 不足以覆盖 entry 级，必须用 §4 的运行时校验）。

---

## 3. 短 ID 系统（出站缩短 / 入站展开）算法

> **来源**：`ToolExecutor+ShortId.swift`（逐行可译）。ARCHITECTURE §7 `:152`「省 token 的关键设计，必须复刻」。

### 3.1 为什么

实体 ID 是完整 UUID（36 字符），verbatim 发送会撑爆 `get_timeline`/`get_transcript`。方案：**出站把每个已知 UUID 替成「在全工程唯一的最短前缀（≥8 字符）」；入站把任意前缀展开回完整 UUID（歧义则报错）**。系统提示词专门叮嘱「原样传回前缀，别补全」（`AgentInstructions.swift:19-20`）。

### 3.2 ID 宇宙（`currentIdUniverse`，`+ShortId.swift:26-39`）

每次工具执行时从当前 `Timeline` + 媒体库 + 文件夹收集**所有 Agent 可见/可命名的 ID**：

```
for track in timeline.tracks:
    ids += track.id
    for clip in track.clips:
        ids += clip.id
        ids += clip.caption_group_id (if Some)
        ids += clip.link_group_id    (if Some)
for asset in media_assets: ids += asset.id
for folder in folders:     ids += folder.id
```

返回 `HashSet<String>`。

### 3.3 出站缩短（`shorteningIds` + `shortIdMap`，`:43-64`）

```rust
const ID_PREFIX_FLOOR: usize = 8;

// 每个 id → 不与任何其它 id 共享的最短前缀（≥8）
fn short_id_map(ids: &HashSet<String>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for id in ids {
        let mut len = ID_PREFIX_FLOOR;
        while len < id.len()
            && ids.iter().any(|other| other != id && other.starts_with(&id[..len])) {
            len += 1;
        }
        out.insert(id.clone(), id[..len].to_string());
    }
    out
}
```
注意：按 **char**（UTF-8 字节）切片需小心，UUID 全 ASCII 故 `[..len]` 安全；若 ID 含非 ASCII（不应发生），改用 `chars().take(len)`。

应用：用正则扫描结果文本里的**完整 UUID**，逐个查 map 替换（不在 map 的 UUID——如嵌在文件名里的——原样透传）。

```
uuid_regex = r"[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}"
for block in result.content where block is Text:
    block.text = uuid_regex.replace_all(text, |m| map.get(m).cloned().unwrap_or(m.to_string()))
```
**关键时序**（`ToolExecutor.execute:69`）：缩短在**工具运行后**的 timeline 状态上做，这样新建实体的 ID（出现在 summary 里）也能被缩短。

### 3.4 入站展开（`expandingIdPrefixes` + `expandOne`，`:68-99`）

工具执行前，把入参里**指定键**的前缀展开为完整 ID：

- **标量键**（`scalarIdKeys`，`:10-15`）：`clipId, sourceClipId, mediaRef, startFrameMediaRef, endFrameMediaRef, sourceVideoMediaRef, videoSourceMediaRef, folderId, parentFolderId`。
- **数组键**（`arrayIdKeys`，`:16-20`）：`clipIds, assetIds, folderIds, referenceMediaRefs, referenceImageMediaRefs, referenceVideoMediaRefs, referenceAudioMediaRefs`。

递归遍历入参 JSON：遇到 scalar 键的字符串值 → `expand_one`；遇到 array 键的字符串数组 → 逐元素 `expand_one`；其它对象/数组递归下探（覆盖 `entries[].mediaRef`、`moves[].clipId` 等嵌套）。

```rust
fn expand_one(reference: &str, universe: &HashSet<String>) -> Result<String, ToolError> {
    if universe.contains(reference) { return Ok(reference.to_string()); }      // 已是完整 ID
    let matches: Vec<&String> = universe.iter().filter(|id| id.starts_with(reference)).collect();
    match matches.len() {
        1 => Ok(matches[0].clone()),
        0 => Ok(reference.to_string()),  // 未知 → 原样传，让工具自己报 not-found
        _ => Err(ToolError::new(format!(
            "Ambiguous id '{reference}' matches {} items; re-read with get_timeline or get_media for current ids.",
            matches.len()))),
    }
}
```

> **测试对拍**（与 Swift 必须一致）：① 两个 UUID 共享前 8 字符 → 各自缩短到第 9 字符；② 传一个唯一前缀 → 展开为完整 ID；③ 传一个被两个 ID 共享的前缀 → 报「Ambiguous」；④ 出站文本里嵌在文件名中的 UUID（不在宇宙）→ 不被替换。

---

## 4. 统一执行壳 + 面向 LLM 的精确路径错误

> **来源**：`ToolExecutor.execute:22-70`（执行壳）、`validateUnknownKeys/decodeToolArgs/firstNonFiniteNumberPath/formatDecodingError:166-229`（校验 + 错误格式化）。

### 4.1 执行壳（`execute`，逐步照搬 + Rust 化）

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

### 4.2 严格输入校验（三层，**面向 LLM 的错误工程**）

#### 4.2.1 未知字段拒绝（`validateUnknownKeys:166-171`）

```
unknown = keys(entry) - allowed
if !unknown.empty:
    error "{path}: unknown field(s) '{a}', '{b}'. Allowed: {sorted allowed joined ', '}."
```
**嵌套也查**：上游对 `entries[]` 逐条调 `validateUnknownKeys(d, allowed: Entry.allowedKeys, path: "entries[3]")`（`+Clips.swift:136`）。因为 serde/Decodable 默认不拒嵌套未知键。Rust：先 `serde_json::Value` 层手动比对 `allowedKeys`（每个工具一个 `&[&str]` 常量），再反序列化到强类型。

#### 4.2.2 非有限数拒绝（`firstNonFiniteNumberPath:194-208`）

递归找第一个 `NaN`/`Inf` 的路径：

```
firstNonFiniteNumberPath(value, path):
  if value is f64 && !finite: return path
  if array: for (i, v): recurse "{path}[{i}]"
  if object: for (k, v): recurse "{path}.{k}"
  return None
// 命中 → error "{badPath}: value must be finite"
```

#### 4.2.3 路径化解码错误（`formatDecodingError:210-229` + `decodeToolArgs:177`）

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

#### 4.2.4 业务级守卫（照搬上游逐工具检查，举证）

以 `add_clips`（`+Clips.swift:13-174`）为例，错误措辞**原样**：
- `"Missing or empty 'entries' array"`
- `"entries[{idx}]: track index {ti} out of range (0..{max})"`
- `"entries[{idx}]: asset type {a} is not compatible with {b} track at index {ti}"`
- `"entries[{idx}]: durationFrames must be >= 1 (got {n})"`
- `"entries[{idx}]: startFrame must be >= 0 (got {n})"`
- `"entries[{idx}]: trimStartFrame must be >= 0 (got {t})"` / `trimEndFrame` 同
- `"Mixed trackIndex: {k} of {n} entries omitted trackIndex. Either set it on every entry or omit it on every entry (to auto-create shared tracks)."`

这些守卫属 `opentake-core`/`opentake-ops` 的命令校验层；本 crate 透传其错误文本（措辞与上游对齐，便于 LLM 自纠）。

### 4.3 助手专属 undo（`undo:109-123`）

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

### 4.4 中立结果类型（`ToolResult.swift`）

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

---

## 5. 应用内 chat（reqwest→Anthropic SSE / BYOK + prompt cache）

> **来源**：`AgentService.swift`（loop）、`AnthropicClient.swift`（直连）、`PalmierClient.swift`（代理）、`AgentClientTypes.swift`（SSE + 请求体 + cache）。**与 MCP 共享同一套工具 + 同一系统提示词**（§2、§6）。

### 5.1 模型与双通道（`selectClient:52-59`）

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

### 5.2 HTTP + SSE（`AnthropicClient.run:57-86` + `AnthropicSSE.parse:88-152`）

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

### 5.3 Agentic loop（`runLoop:341-396` + `runPendingToolUses:422-447`）

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

### 5.4 请求体 + prompt cache 边界（`AnthropicRequestBody.build:156-194`）——**逐字复刻**

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

### 5.5 用量日志（`AgentUsageLog.record:73-84`）

DEBUG 下打印缓存命中率：`billed = input + cache_creation + cache_read`；`read% = cache_read/billed`。Rust 用 `tracing::debug!`。

### 5.6 孤儿 tool_use 修复（`resolveOrphanToolUses:458-495`）

发往 API 的消息序列必须合法（每个 `tool_use` 后必须紧跟配对 `tool_result`）。取消/出错会留下未配对的 `tool_use`。修复：扫描每个 assistant 消息的 tool_use id 集合，与下一条 user 消息的 tool_result id 集合比对，给孤儿补合成 `tool_result{is_error:true, content:[Text("Cancelled")]}`（插到下一条 user 消息开头，或新建一条 user 消息）。**必须复刻**，否则取消后再发会被 API 拒。

### 5.7 上下文构建 + @提及（`apiMessages:514-527` + `inlineImageBlocks:529-554`）

把本地 `[AgentMessage]`（块模型：`Text`/`ToolUse`/`ToolResult`）转 Anthropic `content` 数组。**@提及上下文**（`AgentMentionContext`）：
- 用户可 @ 媒体资源 / 时间线 clip / 选中区间。
- 发送时把被引用提及拼成一段 JSON `hint`（`Referenced assets and timeline context...`），**插到该用户消息最前面**。
- 图像类提及直接 base64 **内联成 image block**，并标 `inlined:true` 告诉模型别再 `inspect_media`。
- clip 提及附 `clipSummary`（clipId/mediaRef/帧位/trim/speed）。

> @提及是 chat UI 能力，**MVP 可后置**（MCP 通道无此概念）。但 `hint` 注入机制与 §6 的 Context Signal 注入是两套独立通道（一个进对话消息、一个进工具结果），不要混淆。

### 5.8 会话持久化（`ChatSessionStore`）

存到工程目录 `chat-sessions/`（每会话一个 JSON，ARCHITECTURE §9 `:176`）。多 tab（`ChatSession.isOpen`）。块模型 `AgentContentBlock`（`AgentService.swift:609-657`）用 serde 复刻（tagged enum：`Text`/`ToolUse`/`ToolResult`）。

---

## 6. Context Signal 注入（每个工具返回如何附 `context_signal`）

> **来源**：`docs/AGENT-CONTEXT-SIGNAL.md`（全文）。知识源：ClipSkills（`appergb/ClipSkills`，MIT，12 册软件无关剪辑知识内核）。**核心原则：不让 Agent 自己读技能文件；软件在 Agent 操作时主动推送结构化 `context_signal`**。
>
> 落点：§4.1 执行壳第 10 步——`run` 之后、`shorten_ids` 之前，由 `ContextSignalEngine::attach(tool, result, core, plugins)` 给结果追加一个 `context_signal` 块。

### 6.1 注入时机（哪些工具附什么信号，`AGENT-CONTEXT-SIGNAL.md:37-47`）

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

### 6.2 数据结构（Rust，定义在 `opentake-domain`，本 crate 消费 + 序列化；`AGENT-CONTEXT-SIGNAL.md:50-83`）

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

### 6.3 视频类型自动检测（`AGENT-CONTEXT-SIGNAL.md:104-140`）

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

### 6.4 轨道角色自动识别（`AGENT-CONTEXT-SIGNAL.md:148-173`）

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

### 6.5 系统提示词组装（base + 插件 instructions.md，`AGENT-CONTEXT-SIGNAL.md:96` + `WORKFLOW-PLUGIN-SYSTEM.md:108-110`）

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

#### 6.5.1 基础系统提示词（OpenTake 版）

来源：上游 `AgentInstructions.serverInstructions`（`AgentInstructions.swift:4-143`，分节 Core model / Always do / Editing / Generation / Audio generation / Prompt craft / Communication）。

**ARCHITECTURE §7 `:154` 的 OpenTake 增强**：从「单块字符串」升级为**分层可组合**，且**模型策略从配置注入**（上游把 Seedance/Nano Banana/Veo 等具体模型写死在提示词里，`AgentInstructions.swift:79-89`）。OpenTake：
- 拆成 `core_model` / `always_do` / `editing` / `generation`（模型策略占位，从 `opentake-gen` 的可用模型动态填）/ `communication` 多段常量，运行时拼装。
- `core_model` 段保留上游关键约束（**逐字保留契约性强的句子**）：
  - 「All timing is in FRAMES, not seconds: frame = seconds × fps.」
  - 「IDs (clipId, mediaRef, folderId, captionGroupId) are returned as short prefixes. Pass them back exactly as given — never pad, complete, or guess a longer form.」（短 ID 契约，**必须保留**，否则 §3 失效）
- `editing` 段保留 transcript-driven 删除的警告（`AgentInstructions.swift:65-69`「read the WORD-level get_transcript end-to-end as prose at least once before deduping」）。
- `communication` 段保留「默认一两句、报结果不报过程、别旁白 'let me…'、匹配冷静克制 HIG 风格」（`:133-142`）。
- 产品名 Palmier → OpenTake。

### 6.6 规则校验（内置规则 + 插件规则，`AGENT-CONTEXT-SIGNAL.md:177-212`）

写工具执行后，软件检查操作是否与剪辑规则一致，不匹配则在 `context_signal` 里附 warning。**内置规则（ClipSkills 通用）+ 插件规则同时生效，不互斥**；校验顺序：内置规则 → 插件规则 → 组合 warning 列表返回（`AGENT-CONTEXT-SIGNAL.md:206-212`）。

```rust
fn validate_operation(tool: ToolName, op: &OpContext, signal: &ContextSignal, plugins: &PluginRegistry)
    -> Vec<String> {
    let mut warnings = builtin_rules(tool, op, signal);       // §6.6.1
    warnings.extend(plugin_rules(op, plugins));               // §6.6.2
    warnings
}
```

#### 6.6.1 内置规则（`AGENT-CONTEXT-SIGNAL.md:177-203`，warning 文本原样）

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

#### 6.6.2 插件规则（`WORKFLOW-PLUGIN-SYSTEM.md:116-118`）

`plugin.json → workflow.rules.do/dont` 作为额外规则层。违反 `dont` 返回 warning。`do` 用作 `stage_guidance` 提示。匹配方式：MVP 用关键词/结构启发式（如「不要连续 3 段以上无 B-roll 覆盖」→ 检测 MainCamera 轨连续 N 段无上层 BRollOverlay 覆盖）。无法机器判定的规则降级为「软提醒」（在 stage_guidance.warnings 里原样列出供 LLM 自检）。

---

## 7. Workflow Plugin（plugin.json 加载、activate_workflow、instructions.md 注入、rules 校验）

> **来源**：`docs/WORKFLOW-PLUGIN-SYSTEM.md`（全文）。**纯 JSON + Markdown，不修改 Rust core 编辑逻辑，完全在 Agent 层运作**（`:136-141`）。不需 Rust 编译、不需 WASM 运行时（`:141`）。

### 7.1 插件格式（`WORKFLOW-PLUGIN-SYSTEM.md:18-96`）

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

### 7.2 插件注册表 + 加载（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`、`:136-141`）

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

### 7.3 激活方式（`WORKFLOW-PLUGIN-SYSTEM.md:100-104`）

| 方式 | 触发 | 实现 |
|---|---|---|
| 自动匹配 | 工程特征匹配 `video_type.detection_hints` | `get_timeline` 检测时比对，命中则**推荐**（不强制激活；提示用户/Agent） |
| 手动选择 | 用户在工程设置选 | 前端调 Tauri 命令 → `registry.activate(id)` |
| Agent 指定 | Agent 调 MCP 工具 `activate_workflow` | §7.4 |

### 7.4 `activate_workflow` MCP 工具（**OpenTake 新增的第 32 个工具**，`WORKFLOW-PLUGIN-SYSTEM.md:104`、ROADMAP Phase W `:119`）

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

### 7.5 插件对 Agent 的三处影响（`WORKFLOW-PLUGIN-SYSTEM.md:108-118`，与 §6.5/§6.6 衔接）

1. **系统提示词注入**：`instructions.md` → 注入 system（§6.5），附当前轨道角色映射 + workflow rules。
2. **工具返回增强**：每次工具调用返回时附该阶段操作提示（`stage_guidance` 来自 `workflow.stages`，标来源 `plugin:{id}`，§6.1 表与 `AGENT-CONTEXT-SIGNAL.md:92`）。
3. **规则校验**：`workflow.rules` 在编辑操作时校验，违 `dont` 返 warning（§6.6.2）。

### 7.6 插件字段 → ContextSignal 叠加（`AGENT-CONTEXT-SIGNAL.md:88-98`，**叠加优先级与覆盖语义**）

| 插件字段 | 注入位置 | 规则 |
|---|---|---|
| `workflow.stages` | `ContextSignal.stage_guidance` | 插件阶段列表**追加**到内置阶段之后，标来源 `plugin:{id}` |
| `workflow.rules` | `ContextSignal.track_hints[].advice` + warning | 每次工具调用校验，违规产 warning |
| `track_roles` | `ContextSignal.track_roles` | 插件定义的角色**覆盖**自动检测（手动指定优先） |
| `video_type` | `ContextSignal.video_type` | 插件声明类型**覆盖**自动检测 |

叠加优先级：**插件声明 > 用户手动设置 > 软件自动检测 > 默认值**。

---

## 8. 与 opentake-core 的接口

> `opentake-agent` 不实现编辑算法；它把工具调用翻译成 `opentake-core` 的命令并消费结果。`opentake-core` 是「唯一编辑入口」，持有权威 `Timeline`（ARCHITECTURE §2 `:62`、§5 `:103-122`）。

### 8.1 命令枚举与结果（ARCHITECTURE `:105-116`，已定义在 `opentake-core`/`opentake-ops`）

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
`command::apply` = 上游 `withTimelineSwap` 事务（`快照 → 改 → before!=after 才压 UndoStack 整树快照 → version+1 → 广播 timeline_changed`，ARCHITECTURE `:118-120`）。

### 8.2 `opentake-agent` 需要 `opentake-core` 暴露的接口（`CoreHandle`）

本 crate 通过一个 `CoreHandle`（`Arc<Mutex<EditorState>>` 或 actor mpsc）调用：

```rust
pub trait CoreHandle: Send + Sync {
    // 读
    fn timeline(&self) -> Timeline;                                   // 当前权威 Timeline（短 ID 宇宙、Context Signal 检测、execute 快照都用它）
    fn timeline_version(&self) -> u64;
    fn media_manifest(&self) -> MediaManifest;
    fn folders(&self) -> Vec<Folder>;
    fn media_assets(&self) -> Vec<MediaAsset>;
    fn current_frame(&self) -> u64;
    fn can_generate(&self) -> bool;                                   // get_timeline 的 canGenerate
    // 写（唯一入口）
    fn apply(&self, cmd: EditCommand) -> Result<EditResult, CoreError>;
    // undo 治理（§4.3）
    fn can_undo(&self) -> bool;
    fn undo_action_name(&self) -> Option<String>;
    // 媒体/转写/渲染/搜索/生成 —— 转发到对应 crate（opentake-media / opentake-render / opentake-gen）
    async fn inspect_media(&self, args: InspectMediaArgs) -> Result<ToolResult, CoreError>;
    async fn get_transcript(&self, args: GetTranscriptArgs) -> Result<ToolResult, CoreError>;
    async fn inspect_timeline(&self, args: InspectTimelineArgs) -> Result<ToolResult, CoreError>;
    async fn search_media(&self, args: SearchMediaArgs) -> Result<ToolResult, CoreError>;
    fn list_models(&self, kind: Option<ModelKind>) -> ToolResult;
    async fn submit_generation(&self, req: GenerationRequest) -> Result<ToolResult, CoreError>;
    async fn import_media(&self, src: ImportSource, name: Option<String>, folder: Option<String>) -> Result<ToolResult, CoreError>;
}
```

映射表（工具 → CoreHandle 调用）：

| 工具 | CoreHandle 调用 |
|---|---|
| `get_timeline` | `timeline()` + 压缩编码（§8.3） + `total_frames`/`current_frame`/`can_generate` |
| `get_media` | `media_manifest()` |
| `inspect_media` / `get_transcript` / `inspect_timeline` / `search_media` | 对应 async 转发（→ `opentake-media`/`opentake-render`） |
| `list_models` | `list_models(kind)`（→ `opentake-gen`） |
| add/insert/remove/move/split/trim/set_*/ripple/add_texts/add_captions/remove_tracks/create_folder/move_to_folder | `apply(EditCommand::...)` |
| `undo` | §4.3（`can_undo`/`undo_action_name`/`apply(Undo)`） |
| `generate_*`/`upscale_media` | `submit_generation(...)` |
| `import_media` | `import_media(...)` |
| `rename_media`/`rename_folder`/`delete_media`/`delete_folder` | `apply(...)` 或专用方法 |

### 8.3 get_timeline 编码（压缩规则，照搬 `ToolExecutor+Timeline.swift:17-112`）

`opentake-agent` 负责把 `Timeline` 编码成 LLM 友好 JSON（**省 token**）：
- 剥离等于默认值的字段：track 默认 `{muted:false, hidden:false, syncLocked:true}`（`:60`）；clip 默认 `{mediaType:"video", speed:1, volume:1, opacity:1, trims/fades:0, identity transform/crop, default textStyle}`；`sourceClipType` 等于 `mediaType` 时剥离（`:113-115`）；text clip 不报 trim（`:117-120`）。
- caption clip（共享 `captionGroupId`）折叠成 `captionGroups`：共享样式 hoist + 每 clip `[clipId, startFrame, durationFrames, text]` 行，**上限 200 行**（`:7-8` captionRowLimit/captionRowFormat、`compactTrack` 分组逻辑）。偏离组的 caption clip 单独列入 `clips`。
- keyframe 压成紧凑数组；浮点保留 **3 位**（`roundJSONFloatingPointNumbers(..., toPlaces: 3)`，`:46`）。
- 窗口分页：`startFrame`/`endFrame` → 只返回相交 clip；被窗口隐藏时报 `totalClips`/`totalFrames`（`:32-44`）。
- track 报**显示标签**（镜像视频编号），非存储 seed（`:38`）。

> 这层是 `opentake-agent` 的职责（不是 `opentake-core`），因为它是「面向 LLM 的表示」，与短 ID 缩短（§3.3）同属出站表示层。

### 8.4 跨进程一致性（ARCHITECTURE §2 `:62`）

OpenTake 跨进程：Rust 持有权威 `Timeline`，前端拿快照 + 单调递增 `timeline_version`；每次 `apply` 广播 `timeline_changed{version}`。**MCP/chat 工具结果里的 ID/帧位在下一次编辑前可能因前端或另一前端的编辑而失效**——上游单进程无此问题。处理：工具结果照常返回当前状态；系统提示词保留「Re-read with get_timeline after a failure that suggests your model is stale」（上游已有，`AgentInstructions.swift:25-28`）。

---

## 9. 实施清单

> 阶段对齐 ROADMAP：Phase 7（MCP+chat+工具）/ Phase S（Context Signal）/ Phase W（Workflow Plugin），三者同步交付。依赖 `opentake-domain`/`opentake-ops`/`opentake-core` 先行（命令层 + 领域模型 + 编辑算法）。

### 9.1 crate 骨架（`crates/opentake-agent/`）

```
src/
├── lib.rs                  # 导出 start_mcp_server / AgentService / ToolExecutor
├── mcp/
│   ├── server.rs           # rmcp StreamableHttpService 装配 + axum + 绑 127.0.0.1:19789（§1.2）
│   ├── guards.rs           # OriginGuardLayer / ContentTypeGuardLayer / ProtocolVersionGuardLayer（§1.2.2）
│   └── resources.rs        # opentake://models/{video,image}（§1.3）
├── tools/
│   ├── names.rs            # ToolName 枚举（31 + activate_workflow 等）
│   ├── descriptions.rs     # 31 条描述（§2.2，原样 const / include_str!）
│   ├── schemas.rs          # 入参 JSON Schema（§2.3）
│   ├── args.rs             # 各工具 DecodableToolArgs（allowedKeys + serde 结构体，§4）
│   ├── executor.rs         # ToolExecutor::execute 统一壳（§4.1）
│   ├── short_id.rs         # 短 ID 出站/入站（§3）
│   ├── errors.rs           # ToolError + serde_path_to_error 路径化（§4.2.3）
│   └── encode_timeline.rs  # get_timeline 压缩编码（§8.3）
├── chat/
│   ├── service.rs          # AgentService agentic loop（§5.3/5.6）
│   ├── client.rs           # trait AgentClient + AnthropicClient（BYOK）+ OpenTakeProxyClient（§5.1）
│   ├── sse.rs              # AnthropicSSE 解析（§5.2）
│   ├── request.rs          # AnthropicRequestBody + prompt cache（§5.4）
│   ├── session.rs          # ChatSession 持久化（§5.8）
│   └── keychain.rs         # keyring 存取 anthropic-api-key（§5.1）
├── signal/
│   ├── engine.rs           # ContextSignalEngine::attach（§6.1）
│   ├── classify.rs         # 视频类型检测（§6.3）
│   ├── track_roles.rs      # 轨道角色检测 + advice 常量（§6.4）
│   ├── stages.rs           # editing_stage + stage_guidance + editing_skeleton（§6.3/§6.5）
│   └── rules.rs            # 内置规则校验 + warning 常量（§6.6.1）
├── plugin/
│   ├── model.rs            # PluginManifest serde（§7.1）
│   ├── registry.rs         # PluginRegistry 加载/激活/校验（§7.2/§7.3）
│   ├── inject.rs           # instructions.md → system prompt 组装（§6.5）
│   └── rules.rs            # 插件 rules 校验（§6.6.2）
└── prompt/
    ├── base.rs             # 分层基础系统提示词（§6.5.1）
    └── assemble.rs         # base + 激活插件组装（§6.5）
```

### 9.2 任务清单（按依赖序）

**Phase 7 — MCP + chat + 工具（核心）**
1. [ ] `tools/names.rs` + `tools/descriptions.rs`：31 工具名 + 描述原样落地（产品名替换为 OpenTake，URI `opentake://`）。— 验证：描述与 §2.2 行号逐条对拍。
2. [ ] `tools/short_id.rs`：§3 出站缩短 + 入站展开 + 歧义报错。— 验证：§3.4 四个对拍用例（与 Swift 一致）。
3. [ ] `tools/errors.rs` + `tools/args.rs`：`serde_path_to_error` 路径化 + `allowedKeys` 未知字段拒绝 + 非有限数拒绝。— 验证：构造 `entries[3].startFrame` 缺失/类型错/未知字段/NaN，输出措辞与 §4.2 一致。
4. [ ] `tools/executor.rs`：§4.1 统一壳（快照→展开→run→undo 记账→signal→缩短）。
5. [ ] `tools/encode_timeline.rs`：§8.3 压缩编码（默认值剥离、captionGroups 折叠 200 行、浮点 3 位、窗口分页）。
6. [ ] `mcp/server.rs` + `mcp/guards.rs` + `mcp/resources.rs`：rmcp + axum 绑 `127.0.0.1:19789` + 三个 tower layer + 2 resources + 偏好开关幂等。— 验证：`claude mcp add` 连通；每工具走通；伪造 Origin/外网 IP 被拒。
7. [ ] `chat/`：`trait AgentClient` + `AnthropicClient`（BYOK，keyring）+ SSE 解析 + 请求体 cache_control + agentic loop + 孤儿修复 + 会话持久化。— 验证：应用内 chat 多步链式编辑；助手专属 undo 正确；DEBUG 缓存命中率日志非零。
8. [ ] `opentake-core` 对接：实现/对接 `CoreHandle`（§8.2），31 工具映射到 `EditCommand`/转发。

**Phase S — Context Signal（随 Phase 7）**
9. [ ] `opentake-domain` 增 `ContextSignal`/`TrackRole`/`VideoType`/`EditingStage` 等类型（§6.2，Phase A 已规划）。
10. [ ] `signal/classify.rs` + `signal/track_roles.rs`：§6.3/§6.4 检测规则 + advice/skeleton 常量（文本原样）。
11. [ ] `signal/engine.rs` + `signal/rules.rs`：§6.1 按工具附信号 + §6.6.1 内置规则校验。— 验证（ROADMAP Phase S `:110`）：`get_timeline` 结果含 `context_signal`；轨道角色标注正确；规则不匹配产 warning。

**Phase W — Workflow Plugin（随 Phase 7）**
12. [ ] `plugin/model.rs` + `plugin/registry.rs`：§7.1/§7.2 plugin.json 加载 + 校验 + instructions.md 读入。
13. [ ] `tools/`：`activate_workflow`（+ `list_workflows`/`deactivate_workflow`）工具（§7.4）。
14. [ ] `prompt/assemble.rs` + `plugin/inject.rs`：§6.5 base + 插件 instructions.md 组装；激活后重建 system。
15. [ ] `plugin/rules.rs`：§6.6.2 插件 do/dont 校验，与内置规则组合（顺序：内置→插件）。— 验证（ROADMAP Phase W `:119`）：`activate_workflow` 激活后 system 含 instructions.md；后续工具结果含插件阶段提示与规则 warning。

**OpenTake 增强（ARCHITECTURE §7 `:154`，可后置）**
16. [ ] 系统提示词分层化 + 模型策略从 `opentake-gen` 配置注入（§6.5.1）。
17. [ ] 高阶工具 `remove_filler_words` / `tighten_silences`（把易错帧算术在 Rust 内一次完成）。
18. [ ] 写工具统一返回结构化 JSON（§4.4 增强）。
19. [ ] `get_capabilities`（一次性返回 ASR/视觉索引/生成/编解码就绪状态）。

### 9.3 测试要求（对应 testing 规则 80% 覆盖 + 与上游对拍）

- **单元（纯逻辑，可全覆盖）**：短 ID 缩短/展开（§3）、错误路径格式化（§4.2）、未知字段/非有限数校验、get_timeline 压缩编码（默认值剥离/captionGroups 折叠/浮点 3 位）、视频类型检测规则、轨道角色检测规则、plugin.json 解析容错、提示词组装、cache_control 边界（请求体 JSON 结构）。
- **集成**：MCP `initialize`/`tools/list`/`tools/call` 端到端（rmcp 客户端打本地 server）；Origin/ContentType/版本 guard 的拒绝路径；chat agentic loop（mock `AgentClient` 吐 SSE → 工具执行 → 再请求 → end_turn）；孤儿修复后消息序列合法性。
- **安全**：外网 IP / 伪造 Origin 必须被拒（DNS-rebinding）；keychain 不落工程文件；BYOK key 不进日志。
- **对拍**：短 ID 算法、错误措辞、压缩编码字段集与上游 Swift 输出逐条比对（关键的 LLM 行为相关项）。

### 9.4 安全检查清单（提交前）

- [ ] MCP server **仅绑 `127.0.0.1`**，禁 `0.0.0.0`；三个 tower guard 生效。
- [ ] Anthropic key 存 OS keychain（`keyring`），绝不入 `project.json`/日志/遥测。
- [ ] 工具入参全部经 §4 校验（未知字段/非有限数/类型）；`import_media` 的 url 必须 HTTPS + 扩展名白名单 + ≤1GB（上游 `+Import.swift` 语义）。
- [ ] 错误信息不泄露内部路径/密钥（错误措辞对 LLM 友好但不含敏感数据）。
- [ ] 插件 `instructions.md` 注入系统提示词前不可信内容隔离（标注来源 `plugin:{id}`，避免提示注入冒充系统指令）。

---

## 附：与上游的关键差异（OpenTake 决策）

| 项 | 上游 PalmierPro | OpenTake |
|---|---|---|
| MCP 传输 | NWListener 手写 TCP + SDK StatelessHTTPServerTransport | rmcp `streamable-http-server`（axum）自带，省手写外壳 |
| Origin 校验 | SDK `OriginValidator.localhost` | tower layer 手写等价（rmcp 不内置） |
| 计费通道 | Clerk JWT → Convex `/v1/agent/stream`（闭源云） | 去掉或换 OpenTake 自建 `opentake-gen-proxy`；MVP 只留 BYOK |
| 系统提示词 | 单块字符串，模型策略写死 | 分层可组合 + 模型策略从配置注入 + 插件 instructions.md 叠加 |
| Context Signal | 无 | **新增**：每工具返回附 `context_signal`（§6） |
| Workflow Plugin | 无 | **新增**：plugin.json + `activate_workflow`（§7） |
| 写工具结果 | 多为人话字符串 | 统一结构化 JSON（§4.4 增强） |
| 进程模型 | 单进程，UI/MCP 天然一致 | 跨进程，Rust 持权威 Timeline + version 广播（§8.4） |
| 工具计数 | 31（30 case + undo） | 31 对等 + `activate_workflow` 等 Agent 层工具 + 增强工具 |
