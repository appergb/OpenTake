我已经完成对整个 Agent 目录的深度分析,所有结论都基于真实代码并已交叉验证。下面输出完整报告。

---

# PalmierPro Agent 子系统深度分析报告

> 范围:`palmier-pro-upstream/Sources/PalmierPro/Agent/`(29 个 Swift 文件,约 7344 行),交叉核对 `Package.swift`、swift-sdk 0.12.0 源码、`Resources/MCPB/palmier-pro.mcpb`、`App/AppState.swift`。
> 结论先行:**这一层非常薄**。真正的"编辑引擎"在 `EditorViewModel`(领域模型 + 编辑算法,不在 Agent 目录内)。Agent 目录干三件事:(1) 把 `EditorViewModel` 的能力包装成 31 个工具;(2) 用官方 swift-sdk 把这些工具起成一个 localhost HTTP MCP server;(3) 一个应用内 chat,直连 Anthropic / Palmier 后端,**复用完全相同的工具与系统提示词**。

---

## 1. MCP server 如何起在 127.0.0.1:19789

### 1.1 关键事实

- **端口硬编码**:`MCPService.port: UInt16 = 19789`(`MCP/MCPService.swift:9`)。
- **SDK**:官方 `modelcontextprotocol/swift-sdk`,`Package.swift` 声明 `from: "0.11.0"`,`Package.resolved` 实际锁定 **0.12.0**(`import MCP`)。
- **启动入口**:`AppDelegate.applicationDidFinishLaunching` → `AppState.startMCPService()`(`App/AppState.swift:17`)。可由设置开关 `io.palmier.pro.mcp.enabled`(默认开)控制;`MCPService` 是 `@MainActor @Observable`,`editorProvider` 闭包惰性取当前工程的 `EditorViewModel`——**没有打开工程时工具会返回 "Editor not available"**。

### 1.2 关键设计:这是一个"自定义 TCP 外壳 + SDK 无状态 transport 内核"的组合

swift-sdk 0.12.0 自带的 HTTP transport(`StatelessHTTPServerTransport`)**只负责 JSON-RPC 层**,不自带 TCP listener。PalmierPro 用 Apple `Network.framework`(`NWListener`)自己写了 TCP/HTTP 外壳,把解析出的请求喂给 SDK 的 transport。文件:`MCP/MCPHTTPServer.swift`(146 行,`actor`)。

启动流程(`MCPService.start()` → `MCPHTTPServer.start()`):

1. **`NWListener` 仅绑定 IPv4 loopback**:`params.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: 19789)` + `allowLocalEndpointReuse = true`。注释明说"never reachable from the LAN"。`MCPHTTPServer.swift:23-27`。
2. **每条 TCP 连接 = 一对全新的 `Server` + `Transport`**(`makeServer` 闭包,见 `MCPService.start()`)。每连接独立 server 实例,无跨连接会话状态——契合 stateless 语义。
3. 每连接构造一个 **`StatelessHTTPServerTransport(validationPipeline:)`**,显式传入 `StandardValidationPipeline`,含三个官方校验器:
   - `OriginValidator.localhost(port: 19789)` —— DNS-rebinding 防护(校验 `Origin`/`Host` 必须是 localhost/127.0.0.1/[::1] + 端口)。
   - `ContentTypeValidator()` —— 必须 `application/json`。
   - `ProtocolVersionValidator()` —— 校验 `MCP-Protocol-Version`。
   - (注:SDK 默认 pipeline 还含 `AcceptHeaderValidator(.jsonOnly)`,PalmierPro 这里省略了它,放宽了 Accept 头。)
4. `try await server.start(transport: transport)`,然后 `connection.receive(...)` 异步读字节(单次最大 1 MB)。

### 1.3 HTTP 路由(全部手写在 `handle(data:connection:transport:)`)

| 路径 / 方法 | 行为 | 代码位置 |
|---|---|---|
| `POST /mcp` 或 `POST /` | 核心:`await transport.handleRequest(request)` → 写回 JSON 响应 | `:89` |
| `GET /mcp` / `GET /` | 返回 `text/event-stream` 的 `: connected\n\n`(SSE 占位,无实际推流) | `:84-86` |
| `GET /.well-known/oauth-protected-resource` | 返回 `{"resource":"http://127.0.0.1:19789"}` | `:72-77` |
| 其它路径 | 404 | `:79-82` |

HTTP 请求由手写的 `parseHTTPRequest`(`:107`)按 `\r\n\r\n` 切分头/体,组装成 SDK 的 `HTTPRequest(method:headers:body:path:)`。响应用 SDK 的 `HTTPResponse`(`statusCode` / `headers` / `bodyData`)拼回原始 HTTP 报文。`HTTPRequest`/`HTTPResponse` 都是 SDK 公共类型(`Sources/MCP/Base/Transports/HTTPServer/HTTPServerTypes.swift`)。

`Server` 实例上注册的能力(`MCPService.start()`):
```
Server(name: "palmier-pro", version: "1.0.0",
       instructions: AgentInstructions.serverInstructions,
       capabilities: .init(resources: .init(subscribe: false, listChanged: false),
                           tools: .init(listChanged: false)))
```
然后 `withMethodHandler(ListTools.self)` / `withMethodHandler(CallTool.self)` / `ListResources` / `ReadResource`。

### 1.4 对外发布形态:这台 HTTP server 不是给外部直接连的

`Resources/MCPB/palmier-pro.mcpb` 实际是个 zip,解出来是一个 **Node.js stdio→HTTP shim**(给 Claude Desktop 用):
```js
// server/index.js
process.argv = [process.argv[0], 'mcp-remote',
  'http://127.0.0.1:19789/mcp', '--allow-http', '--transport', 'http-only'];
require('mcp-remote/dist/proxy.js');
```
依赖 `mcp-remote@0.1.38`。即:Claude Desktop 用 stdio 启动这个 shim,shim 把 stdio 桥接到本机 `127.0.0.1:19789/mcp` 的 HTTP server。

> **对 OpenTake 的直接启示**:对外暴露形态 = "localhost HTTP MCP server + 一个 stdio→HTTP MCPB shim";真正的安全边界靠"只绑 loopback + Origin 校验"。Rust 侧可一比一复刻。

---

## 2. 对外暴露的 MCP 工具全集(31 个)——OpenTake 要复刻的后端能力 API

工具名与 schema 在 `Tools/ToolDefinitions.swift`(`ToolName` 枚举 30 个 + `undo` 共 31 个),分发在 `Tools/ToolExecutor.swift` 的 `run(_:_:_:)`,实现散在 `ToolExecutor+*.swift`。**每个工具几乎都是 `EditorViewModel` 上一个方法的薄包装**——这点对移植极重要:复刻工具 ≠ 复刻能力,真正要复刻的是 `EditorViewModel` 的编辑算法。

下面按域分组。"如何操作时间线/媒体"一列指向背后调用的 `EditorViewModel` 方法。

### A. 读 / 内省(只读,7 个)

| 工具 | 作用 | 关键参数 | 如何操作时间线/媒体 |
|---|---|---|---|
| **get_timeline** | 会话开头必调。返回 fps/分辨率/totalFrames、tracks(类型+顺序)、所有 clip(帧位+属性)、`canGenerate`、`currentFrame` | `startFrame?`,`endFrame?`(窗口分页) | 把 `editor.timeline` 编码为 JSON,**做大量压缩**:剥离等于默认值的字段;caption clip 折叠成 `captionGroups`(共享样式 hoist + `[clipId,startFrame,durationFrames,text]` 行,上限 200 行);keyframe 压成紧凑数组;浮点保留 3 位。`ToolExecutor+Timeline.swift:17` |
| **get_media** | 引用任何资源前必调。返回媒体库清单 + `generationStatus`(generating/downloading/rendering/failed/none) | — | 编码 `editor.mediaManifest`。`+Timeline.swift:246` |
| **inspect_media** | "看"一个素材:图像(图+EXIF)、视频(抽帧+转写)、音频(转写)、Lottie(抽帧)。**核心多模态能力** | `mediaRef`,`clipId?`(转写时间映射到工程帧),`maxFrames?`(≤12),`startSeconds/endSeconds?`,`wordTimestamps?`(词级,≤10000),`overview?`(storyboard 网格) | 抽帧用 `AVAssetImageGenerator`;`overview` 用 `OverviewRenderer`(keyframe-snapped 密集采样 + 去重 + 烧时间戳,6 列≤36 格);转写走 `TranscriptCache.shared.transcript`(**端侧** `Transcription`)。视觉+转写**并发**跑。`+Timeline.swift:258` |
| **get_transcript** | 当前**时间线**的语音转写(后剪辑结果),按时间线顺序拼接,每词映射过 trim/speed/position | `startFrame/endFrame?`,`clipId?` | 遍历 `editor.captionTargets`,对每个唯一源转写一次(缓存),用 `spanFrames` 把"源秒"映射成"工程帧",词按 midpoint 归属唯一 clip,输出 `[text,startFrame,endFrame]`。`+Timeline.swift:548` |
| **inspect_timeline** | 渲染**合成后**画面(用户在预览看到的):所有视频轨叠加 + transform/opacity/crop/keyframe + 文字/字幕烧入 | `startFrame?`,`endFrame?`,`maxFrames?`(≤12) | `CompositionBuilder.build` 建 AVComposition + `AVAssetImageGenerator`(tolerance=0 精确帧)+ `TextLayerController.buildSnapshot` 烧字 + `compositeCapture`。`+InspectTimeline.swift:12` |
| **search_media** | 按内容搜库:视觉(端侧语义)+ 口语(关键词叠端侧语义转写)。两组独立排名 | `query`,`scope`(visual/spoken/both),`mediaRef?`,`limit?`(≤50) | 视觉走 `editor.searchIndex`(`VisualModelLoader`/CLIP 类嵌入);口语走 `TranscriptSearch.search`。命中是源秒区间(图像无区间)。`+Search.swift:6` |
| **list_models** | 列 AI 模型能力(时长/画幅/分辨率/首尾帧/引用/voice/upscaler 速度) | `type?`(video/image/audio/upscale) | 读 `VideoModelConfig`/`ImageModelConfig`/`AudioModelConfig`/`UpscaleModelConfig` + `ModelCatalog.shared.isLoaded`。`+Generate.swift:373` |

### B. 时间线编辑(写,11 个)——核心剪辑算法

| 工具 | 作用 | 关键参数 | 如何操作时间线 |
|---|---|---|---|
| **add_clips** | 放一个或多个素材到时间线,单步可撤销。同轨重叠则**覆写**(裁/切/删既有 clip,镜像 UI 拖拽);视频带音频自动建 linked audio clip;`trackIndex` 全省略则自动建轨 | `entries[]`{`mediaRef`,`trackIndex?`,`startFrame`,`durationFrames`,`trimStartFrame?`,`trimEndFrame?`} | `editor.clearRegion(prune:false)` + `editor.placeClip(...)`;自动建轨 `editor.insertTrack`;trackIndex 要么全给要么全不给(否则拒绝)。`+Clips.swift:129` |
| **insert_clips** | 在一点插入并**ripple**:atFrame 及之后全体右移,不覆写。sync-locked 轨与 linked audio 同步右移 | `trackIndex`(必填),`atFrame`,`entries[]`{`mediaRef`,`durationFrames?`,`trim*?`} | `editor.rippleInsertClips(specs:trackIndex:atFrame:)`。`+Clips.swift:264` |
| **remove_clips** | 按 ID 删 clip,单步撤销。属于 link group 的连带删除 | `clipIds[]` | `editor.expandToLinkGroup` + `editor.removeClips`;空轨自动 prune(并提示索引已变)。`+Clips.swift:313` |
| **remove_tracks** | 删整轨及其全部 clip。其它轨上的 linked partner 不删;余轨索引下移 | `trackIndexes[]` | `editor.removeTracks(ids:)`。`+Clips.swift:803` |
| **move_clips** | 移 clip 到新轨/新帧。目标重叠按 add_clips 覆写;linked partner 跟随帧 delta(保 L-cut/J-cut),轨不传播 | `moves[]`{`clipId`,`toTrack?`,`toFrame?`} | `editor.partnerMoves` 展开伙伴 + `editor.moveClips([(clipId,toTrack,toFrame)])`。`+Clips.swift:335` |
| **set_clip_properties** | 给一批 clip 套同一组属性值,单步撤销 | `clipIds[]` + 任意组合:`durationFrames`/`trimStartFrame`/`trimEndFrame`/`speed`/`volume`/`opacity`/`transform`(归一化 0–1,部分合并,可 flip)/ 文字专用 `content`/`fontName`/`fontSize`/`color`/`alignment` | `editor.commitClipProperty`;speed 改会按 `sourceConsumed/v` 重算时长;set volume/opacity 会**清除**该属性 keyframe 轨;timing 字段传播给 linked partner(文字伙伴跳过 trim/speed)。`+Clips.swift:419` |
| **set_keyframes** | 给单 clip 单属性设 keyframe 轨(替换式;空数组清空)。帧是 clip-relative | `clipId`,`property`(volume/opacity/rotation/position/scale/crop),`keyframes[]`(`[frame, ...values, interp?]`,interp∈linear/hold/smooth) | 按 property 解析 scalar/pair/crop 行 → `editor.commitClipProperty` 写对应 `*Track`。`+Clips.swift:576` |
| **split_clip** | 在 atFrame 切成两段(必须严格在 clip 内) | `clipId`,`atFrame` | `editor.splitClip(clipId:atFrame:)`。`+Clips.swift:619` |
| **ripple_delete_ranges** | 删多段并闭合空隙,单步撤销。**filler/dead-air 删除的快路径** | 二选一:`trackIndex`(工程帧,跨多 clip)或 `clipId`(单 clip 内,允许 `units:"seconds"`);`ranges[][start,end]`;`units`(frames/seconds) | `editor.rippleDeleteRangesOnTrack(trackIndex:ranges:)`;重叠区间合并,linked 伙伴同区间删,sync-locked 轨同步左移(放不下则**整体拒绝不改动**),返回 anchor 轨剪后布局。`+Clips.swift:639` |
| **add_texts** | 加文字 clip(标题/字幕/下三分之一),overlay。同轨重叠覆写;同时显示需放不同轨;`trackIndex` 全省略则在顶部新建一轨 | `entries[]`{`startFrame`,`durationFrames`,`content`(支持 `\n`),`transform?`,`fontName?`,`fontSize?`,`color?`,`alignment?`} | 解析 transform(`{centerX,centerY}` 自动 fit 或全四字段),`TextLayout.naturalSize` 算自然尺寸 → 写 text clip。`+Texts.swift:42` |
| **add_captions** | 自动字幕:端侧转写 + 放样式化 caption clip 到新轨(同编辑器 Captions 标签管线) | `clipIds?`(省略=自动挑语音最多的轨),`language?`(BCP-47),`fontName/fontSize/color/centerX/centerY?`,`textCase`(auto/upper/lower),`censorProfanity?` | `editor.generateCaptions(for: CaptionRequest)`。`+Captions.swift:9` |
| **undo** | 撤销助手本会话最近一次编辑(只撤助手的,拒绝撤用户的) | — | 维护 `agentUndoStack`,核对 `editor.undoManager.undoActionName` 必须等于期望值才 `undo()`。`ToolExecutor.swift:109` |

### C. 媒体生成 / 导入(写,5 个)——媒体管线接入点

| 工具 | 作用 | 关键参数 | 如何操作媒体 |
|---|---|---|---|
| **generate_video** | 异步 AI 生视频。立即返回 placeholder asset ID,后台跑;**花钱、不可撤销** | `prompt`(必填),`model?`,`duration?`,`aspectRatio?`,`resolution?`,`startFrameMediaRef?`/`endFrameMediaRef?`,`sourceVideoMediaRef?`+`sourceClipId?`(只送 trim 段),`referenceImage/Video/AudioMediaRefs?`,`folderId?` | 走 `VideoGenerationSubmission.make(...).submit(service: editor.generationService, ...)`;文本→视频 vs 视频→视频(edit 模型)分流;模型能力用 `VideoModelConfig.validate`。`+Generate.swift:4` |
| **generate_image** | 异步 AI 生图。立即返回 placeholder | `prompt`(必填),`model?`,`aspectRatio?`,`resolution?`,`quality?`,`referenceMediaRefs?`,`folderId?` | `ImageGenerationSubmission.make(...).submit(...)`。`+Generate.swift:150` |
| **generate_audio** | 异步 TTS / 文生乐 / 视频配乐(video-to-music/sfx) | `prompt?`,`model?`,`voice?`,`lyrics?`,`styleInstructions?`,`instrumental?`,`duration?`,`videoSourceStartFrame/EndFrame?`(时间线区间,**结果自动放回该区间**),`videoSourceMediaRef?`,`folderId?` | video-to-audio 会先 `TimelineRenderer.render` 出 mp4 → `GenerationBackend.uploadReference` → 提交;给了时间线区间则 `editor.placeGeneratingAudioClip` 自动落轨。`+Generate.swift:197` |
| **upscale_media** | 升分辨率(视频/图) | `mediaRef`,`model?`,`sourceClipId?`(只 upscale trim 段) | `EditSubmitter.submitUpscale(...)`。`+Generate.swift:313` |
| **import_media** | 导入外部媒体(其它 MCP / 本地文件)。**异构来源的桥** | `source`{三选一:`url`(HTTPS,≤1GB,后台下载)/`path`(本地,可目录递归)/`bytes`(base64,≤~15MB),`mimeType?`},`name?`,`folderId?` | url 走带 `ImportDownloadDelegate`(超限取消)的后台下载→`editor.importMediaAsset`;path/bytes 同步;目录走 `editor.importFinderItems`。校验 https/无凭据/有 host/扩展名白名单。`+Import.swift:11` |

### D. 媒体库组织(写,7 个)

全部在 `+Folders.swift`,均可撤销,均支持"单条参数 或 `entries[]` 批量"二选一:
- **list_folders** → 读 `editor.folders`(`{id,name,parentFolderId}`)
- **create_folder** → `editor.createFolder(name:in:)`
- **move_to_folder** → `editor.moveAssetsToFolder(assetIds:folderId:)`(省略 folderId=移到根)
- **rename_media** → `editor.renameMediaAsset(id:name:)`
- **rename_folder** → `editor.renameFolder(id:name:)`
- **delete_media** → `editor.deleteMediaAssets(ids:)`(连带删引用它的 clip,同一撤销步)
- **delete_folder** → `editor.deleteFolders(ids:)`(连带删子文件夹/资源/clip)

### E. MCP Resources(2 个,只读,非工具)

`palmier://models/video`、`palmier://models/image`(JSON 模型目录)。`MCPService.registerResources`。

### F. 三个贯穿所有工具的横切机制(OpenTake 必须一并复刻)

1. **短 ID 系统**(`+ShortId.swift`):实体 ID 是完整 UUID(36 字符,会撑爆 get_timeline)。出站时把每个已知 UUID 替成"在全工程唯一的最短前缀(≥8 字符)";入站时把任意前缀展开回完整 ID(歧义则报错)。这是**省 token 的关键设计**。系统提示词专门叮嘱"原样传回前缀,别补全"。
2. **统一执行壳**(`ToolExecutor.execute`):快照 `editor.timeline` → 展开 ID 前缀 → 跑工具 → 若时间线真的变了就把 `undoActionName` 压入 `agentUndoStack`(支撑助手专属 undo)→ telemetry 计时 → 出站缩短 ID。
3. **严格输入校验**:`DecodableToolArgs.allowedKeys` 拒绝未知字段(连嵌套 entry 也查);`firstNonFiniteNumberPath` 拒 NaN/Inf;`formatDecodingError` 给出 `entries[3].startFrame: missing required field` 这种精确路径错误。**面向 LLM 的错误信息工程**。

---

## 3. 应用内 chat 架构

### 3.1 模型与后端(双通道)

`AgentClientTypes.swift` 定义 `AnthropicModel`:`claude-sonnet-4-6` / `claude-opus-4-8` / `claude-haiku-4-5-20251001`。

`AgentService.selectClient()`(`AgentService.swift:52`)双通道:
- **自带 API key**(Keychain `anthropic-api-key`,DEBUG 下可用 `ANTHROPIC_API_KEY` 环境变量)→ `AnthropicClient` **直连** `https://api.anthropic.com/v1/messages`,可用全部三个模型。
- **无 key 但登录了 Palmier**(Clerk)→ `PalmierClient`,带 Clerk JWT 打到 `{convexHttpURL}/v1/agent/stream`(后端代理 + 计费)。付费用户给 Sonnet 4.6,免费给 Haiku 4.5。

两个 client 都实现 `protocol AgentClient { func stream(system:tools:messages:) -> AsyncThrowingStream<AnthropicStreamEvent,Error> }`,**共用同一套** `AnthropicRequestBody.build`(请求体构造)和 `AnthropicSSE.parse`(SSE 解析)。即:应用内 chat 与远端代理走的是**字节级相同的 Anthropic Messages API 协议**,只是 endpoint/鉴权不同。

### 3.2 系统提示词的位置

**`Tools/AgentInstructions.swift` 的 `serverInstructions`(144 行,单个 Swift 字符串字面量)**。这是全系统**唯一**的系统提示词,分节:Core model / Always do / Editing / Generation / Audio generation / Prompt craft / Communication。内含大量领域规则,例如:
- "一切按 **帧** 不按秒,frame = seconds × fps"
- 生成"先图后视频"默认流;**具体模型选型策略**(图默认 Nano Banana Pro + GPT Image,视频默认 Seedance 2.0 Fast@720p,Seedance 报错退 Kling v3,Veo 仅按需)
- transcript-driven 删除前"至少把词级 transcript 当散文读一遍"
- Communication 段:"默认一两句、报结果不报过程、别旁白 'let me…'、匹配 App 冷静克制的 HIG 风格"

### 3.3 上下文如何构建

会话循环在 `AgentService.runLoop()`(`:341`),典型 agentic loop:

1. `apiMessages()`(`:514`)把 `[AgentMessage]`(本地块模型:`.text`/`.toolUse`/`.toolResult`)转成 Anthropic `content` 数组。
2. **@提及上下文**(`AgentMentionContext.swift` + `AgentService` 提及逻辑):用户可 @ 媒体资源 / 时间线 clip / 选中的时间线区间。发送时把被引用的提及拼成一段 JSON `hint`(`Referenced assets and timeline context...`),**插到该用户消息最前面**;图像类提及直接 base64 **内联成 image block**(并标 `inlined:true` 告诉模型别再 `inspect_media`)。clip 提及会附 `clipSummary`(clipId/mediaRef/帧位/trim/speed 等)。
3. `client.stream(system: AgentInstructions.serverInstructions, tools: ..., messages: ...)`。
4. SSE 流出 `textDelta` / `toolUseComplete` / `messageStop(stopReason)`。
5. `stopReason == .toolUse` → `runPendingToolUses`(`:422`)调**同一个 `ToolExecutor`**执行,结果作为 `user` 角色的 `tool_result` 追加,`continue loop` 再请求——直到 `end_turn`。
6. 健壮性:`resolveOrphanToolUses` 给未配对的 tool_use 补合成 tool_result(取消/出错场景),保证发往 API 的消息序列合法。

**Prompt caching**(`AnthropicRequestBody.build`,`AgentClientTypes.swift:156`):在 system、最后一个 tool、最后一条消息的最后一个 block 上打 `cache_control: ephemeral`——把 system+tools 和会话前缀都纳入缓存边界(DEBUG 下 `AgentUsageLog` 打印命中率)。

会话持久化:`ChatSessionStore` 存到工程目录,多 tab(`ChatSession.isOpen`)。

### 3.4 是否与 MCP 工具共享同一套工具/prompt —— **是,完全共享**

这是整个架构最干净的一点,三处共享:

| 共享物 | MCP server | 应用内 chat |
|---|---|---|
| **工具定义** | `ToolDefinitions.all` → `Tool(...)` | `ToolDefinitions.all` → `AnthropicToolSchema(...)`(`AgentService.swift:346`) |
| **工具执行** | `MCPService.dispatchCall` → `ToolExecutor.execute` | `runPendingToolUses` → **同类** `ToolExecutor.execute`(`:441`) |
| **系统提示词** | `Server(instructions: AgentInstructions.serverInstructions)` | `client.stream(system: AgentInstructions.serverInstructions)`(`:359`) |

差异仅在传输:MCP 走 `MCP.Tool` + `CallTool.Result`;chat 走裸 Anthropic JSON。`ToolResult`(`ToolResult.swift`)是中立结果类型,`toMCPResult()` 转 MCP,chat 直接用其 `content`/`isError`。`ToolExecutor` 双初始化器(`init(editor:)` 给 chat、`init(editorProvider:)` 给 MCP)就是为了双场景复用。

> 含义:**"后端能力"只有一处真实定义**(`ToolDefinitions` + `ToolExecutor` + `EditorViewModel`),MCP 和 chat 都是它的前端。OpenTake 照搬这个"单一能力层、双前端"结构即可。

---

## 4. 移植到 Rust(rmcp)的可行性与接口建议

### 4.1 可行性:高。逐项映射

| PalmierPro(Swift) | OpenTake(Rust)对应 | 说明 |
|---|---|---|
| `modelcontextprotocol/swift-sdk` 0.12 | **`rmcp`**(官方 Rust SDK,crates.io `rmcp`) | rmcp 提供 `#[tool]`/`#[tool_router]` 宏、`ServerHandler`、`streamable-http-server` feature(基于 axum/hyper 的 Streamable HTTP transport) |
| `NWListener` 绑 127.0.0.1 + 手写 HTTP | **axum/hyper `TcpListener::bind("127.0.0.1:19789")`** | Rust 侧反而更省事:rmcp 的 streamable-http transport 自带,不必像 Swift 那样手写 TCP 外壳。只需把 axum 的 listener 绑死 loopback |
| `StatelessHTTPServerTransport` | rmcp `StreamableHttpService`(可配 stateless/无 session) | 语义对齐:单 JSON 响应、无 SSE 推流 |
| `OriginValidator.localhost` + `ContentTypeValidator` + `ProtocolVersionValidator` | **axum middleware / `tower::Layer`** 手写三个等价校验 | rmcp 不一定内置 Origin 校验,用 tower layer 补;这是 DNS-rebinding 防护,**必须保留** |
| `ToolDefinitions`(JSON schema 手写) | `#[tool]` 宏 + `schemars::JsonSchema` 派生 | Rust 用 `schemars` 从参数结构体自动派生 input schema,比 Swift 手写字典更安全。但 PalmierPro 的工具描述极长且承载行为契约——**描述字符串要原样照搬** |
| `ToolExecutor.execute`(MainActor) | `EditorCore` 上的 async 方法 + `tokio::sync::Mutex`/actor | Swift 靠 `@MainActor` 串行化;Rust 用单线程 actor 或 `Mutex<EditorCore>` |
| `EditorViewModel`(真正的编辑引擎) | **OpenTake 的 Rust 领域模型/编辑操作模块** | **这才是工作量大头**,见下 |
| `AVFoundation`(抽帧/合成/转写) | **FFmpeg**(抽帧/合成)+ 端侧 ASR(如 whisper.cpp / `whisper-rs`) | `inspect_media`/`inspect_timeline`/`get_transcript`/`add_captions`/`search_media` 全依赖它 |
| `AnthropicClient`(直连)/ `PalmierClient`(代理) | `reqwest` + SSE(`eventsource-stream`)| 协议一致,直接复刻 `AnthropicRequestBody.build` 的 cache_control 策略 |

### 4.2 强烈建议保留的 3 个设计

1. **单一能力层、双前端**:Rust 里把工具实现写成 `EditorCore` 的方法,MCP server(rmcp)与应用内 chat(reqwest→Anthropic)都调它。不要写两套。
2. **短 ID 系统**(`+ShortId.swift` 的算法)几乎可逐行翻译成 Rust——这是 token 成本的关键,且与 LLM 行为强相关,务必复刻"出站缩短/入站展开+歧义报错"。
3. **面向 LLM 的错误工程**:`entries[3].startFrame: missing required field` 这种精确路径错误,直接决定 agent 自我纠错率。Rust 里用 `serde_path_to_error` + 自定义校验复刻。

### 4.3 给 OpenTake 的能力分级建议(对照真实实现)

- **第一梯队(纯领域逻辑,无外部依赖,优先)**:`get_timeline`/`get_media`、A→B 组的所有编辑工具(add/insert/remove/move/split/set_clip_properties/set_keyframes/ripple_delete_ranges/add_texts/undo)。这些只碰 `EditorCore` 内存模型,可纯 Rust 实现并直接单测(Swift 侧编辑算法不在 Agent 目录,需另行从 `EditorViewModel` 复刻)。
- **第二梯队(媒体引擎,依赖 FFmpeg/ASR)**:`inspect_media`/`inspect_timeline`/`get_transcript`/`add_captions`/`search_media`。这是 AVFoundation→FFmpeg 替换的主战场。
- **第三梯队(需后端/计费)**:`generate_*`/`upscale_media`/`import_media(url)`。依赖远端生成服务,OpenTake 可定义同形接口、接入自己的供应商。

---

## 5. OpenTake 可内置"更优美、更强大"的提示词与能力的具体着力点

基于真实代码里能看到的取舍,以下是可超越上游处:

1. **系统提示词从"单块字符串"升级为分层、可组合**:上游 144 行硬编码在 `AgentInstructions.swift` 一个字面量里,且与具体模型(Seedance/Nano Banana 等)强耦合。OpenTake 可拆成 `core_model` / `editing` / `generation`(模型策略从配置注入而非写死)/ `communication` 多段,按工程状态/可用模型动态拼装,并支持用户级 override。

2. **把"批量优先"做成一等公民并在 prompt 里强约束**:上游已有 `add_clips`/`set_clip_properties` 批量,但 `set_clip_properties` 对"每 clip 不同值"要求拆多次调用(`+Clips.swift:419` 注释)。OpenTake 可设计 `set_clip_properties` 支持 per-clip override 数组,减少 round-trip。

3. **更强的 transcript 编辑原语**:上游靠 `get_transcript` + `ripple_delete_ranges` 组合做 filler 删除,且系统提示词反复警告"段视图有损、要读词级散文"(说明这是踩过坑的痛点)。OpenTake 可内置一个 `remove_filler_words` / `tighten_silences` 高阶工具(参数化阈值),把"读词→定位→ripple"在 Rust 内一次完成,避免把易错的帧算术外包给 LLM。

4. **结构化结果而非纯文本**:多数写工具返回人话字符串(如 `"Added 2 clips: ..."`),只有 `ripple_delete_ranges`/`remove_tracks` 返回 JSON。OpenTake 可统一返回结构化 JSON(变更的 clipId/帧位/新建轨),让 agent 无需解析自然语言即可继续——更利于多步链式编辑。

5. **能力探活与降级语义前置**:上游把 `canGenerate`、`search_media.status`(indexing/modelNotInstalled…)、`list_models.loaded` 散落各处。OpenTake 可提供一个 `get_capabilities` 工具,一次性返回"端侧 ASR 是否就绪 / 视觉索引进度 / 生成是否可用 / FFmpeg 编解码支持",让 agent 在动作前一次性规划,减少试错。

6. **prompt cache 边界可再优化**:上游已正确地把 system+tools+会话前缀纳入 ephemeral cache(`AnthropicRequestBody.build`)。OpenTake 直接复刻这套即可拿到同等收益;若自建代理,还可做跨会话的 tools/system 持久缓存。

---

## 关键文件清单(均为绝对路径)

- MCP server / HTTP:
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/MCP/MCPHTTPServer.swift`(TCP 外壳 + 路由)
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/MCP/MCPService.swift`(端口 19789、Server 注册、工具/资源映射)
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/App/AppState.swift`(启动/停止开关)
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Resources/MCPB/palmier-pro.mcpb`(Claude Desktop 用的 stdio→HTTP shim,内含 `mcp-remote → http://127.0.0.1:19789/mcp`)
- 工具定义与执行:
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/Tools/ToolDefinitions.swift`(31 工具 + JSON schema)
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/Tools/ToolExecutor.swift`(分发 + 执行壳 + undo)
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/Tools/ToolExecutor+Clips.swift`(11 个时间线编辑工具,核心算法入口)
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/Tools/ToolExecutor+Timeline.swift`(get_timeline/get_media/inspect_media/get_transcript 压缩与帧映射)
  - `…/ToolExecutor+InspectTimeline.swift`、`+Search.swift`、`+Captions.swift`、`+Texts.swift`、`+Generate.swift`、`+Import.swift`、`+Folders.swift`、`+ShortId.swift`(短 ID 系统)、`ToolResult.swift`
- 应用内 chat:
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/AgentService.swift`(agentic loop、上下文构建、tool loop)
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/Tools/AgentInstructions.swift`(**唯一系统提示词**)
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/Clients/AnthropicClient.swift`、`PalmierClient.swift`、`AgentClientTypes.swift`(模型、SSE、请求体 + prompt cache)
  - `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/AgentMentionContext.swift`(@提及上下文)
- 依赖佐证:`Package.swift`(swift-sdk from 0.11.0)、`Package.resolved`(锁定 swift-sdk 0.12.0)。swift-sdk 的 `StatelessHTTPServerTransport`/`StandardValidationPipeline`/`OriginValidator.localhost(port:)`/`ContentTypeValidator`/`ProtocolVersionValidator`/`HTTPRequest`/`HTTPResponse` 均来自 `Sources/MCP/Base/Transports/HTTPServer/`(官方 SDK,非项目自定义)。
