# Agent 移植规格

**职责**:
- 对话编排：维护多会话(ChatSession)、消息历史、流式 SSE 解析、工具调用循环(runLoop)、孤儿 tool_use 修复、提示缓存边界设置
- LLM 客户端抽象(AgentClient)：AnthropicClient 直连 api.anthropic.com；PalmierClient 经 Clerk JWT 鉴权访问自有 Convex 后端 /v1/agent/stream(计费/积分路径)
- 工具定义(ToolDefinitions)：31 个工具的名称、自然语言描述、JSON Schema 输入约束(同时供 Anthropic tools 与 MCP)
- 工具执行(ToolExecutor + 13 个扩展文件)：参数解码/未知字段拒绝/有限性校验、ID 前缀展开、调用 EditorViewModel 完成编辑、结果 ID 缩短
- 帧/秒/源-时间线坐标换算：trim/speed/startFrame 之间的双向映射(剪辑、波纹删除、转写词映射、上采样裁剪)
- 时间线只读视图压缩：get_timeline/get_transcript/inspect_* 的默认值剥离、字幕组折叠、关键帧行化、窗口分页、UUID→最短唯一前缀压缩
- 媒体读取与理解：AVFoundation 抽帧、OverviewRenderer 故事板雪碧图、on-device 转写(Speech)、语义检索(search_media)
- 生成/上采样/导入：把 generate_* / upscale / import_media 转成后台异步提交，返回占位 assetId
- 本地 MCP HTTP server(MCPHTTPServer/MCPService)：仅绑 127.0.0.1:19789，Origin/ContentType/协议版本校验，暴露同一套 ToolExecutor
- SwiftUI 聊天面板 UI(Panel/*)：输入框、@提及弹窗、消息渲染、Markdown、思考动画
- 撤销治理：agentUndoStack 仅允许撤销助手自己本会话产生的编辑，拒绝撤销用户手动编辑
- API Key 安全存取：AnthropicKeychain 经 Keychain 读写，DEBUG 下可读环境变量

**核心类型**:
- `AgentService` (class) — @MainActor @Observable 对话主控。持有 sessions/messages/draft/mentions，选择后端(selectClient)，跑工具循环(runLoop)，做 SSE→消息块累积、孤儿 tool_use 合成、提示缓存、会话持久化、@提及上下文注入。
- `ToolExecutor` (class) — @MainActor 工具执行核心。execute() 统一做：ID前缀展开→分发到具体工具→对比 timeline 变化决定是否压入 agentUndoStack→缩短结果中的ID→遥测。被应用内 agent 与 MCP server 共用。
- `AgentClient/AnthropicClient/PalmierClient` (protocol) — 流式后端抽象。两实现都构造 Anthropic Messages API body(含 ephemeral 提示缓存)，用 URLSession.bytes 拿 SSE，交给 AnthropicSSE.parse。PalmierClient 额外走 Clerk 鉴权 + Convex 后端。
- `ToolDefinitions/AgentTool/ToolName` (enum) — 31 个工具的单一事实源：枚举名→rawValue 工具名、长描述、JSON Schema。MCP 与 Anthropic 共用；ToolArgsBridge 在 MCP Value 与 [String:Any] 间互转。
- `AnthropicSSE/AnthropicRequestBody` (enum) — 共享 SSE 解析器(解析 text_delta/input_json_delta/content_block_stop/message_delta/error，按 index 累积 tool_use 的分片 JSON)与请求体构造器(在 system+tools 末尾与会话最后一块打 cache_control ephemeral)。
- `ToolResult` (struct) — 工具返回值：[Block] 内容(text 或 base64 图像) + isError。可转成 MCP CallTool.Result，也可 Codable 持久化进消息历史。
- `Clip/Timeline/Track` (struct) — (引用自 Models/Timeline.swift)被工具读写的核心领域模型。Clip 含 startFrame/durationFrames/trimStart/trimEnd/speed/volume/opacity/transform/crop/linkGroupId/captionGroupId/textContent/textStyle 及 6 条关键帧轨。totalFrames=各轨 max(clip.endFrame)。
- `OverviewRenderer` (enum) — 把一段视频抽成单张故事板雪碧图：密集抽候选帧→用 LumaGrid 亮度网格丢弃近重复帧(meanDiff>12 才保留)→6 列网格、最多 36 块、CoreText 烧入时间码。
- `AgentMentionContext/AgentMention/AgentTimelineRangeMention` (struct) — @提及上下文：把被引用的媒体资产/时间线 clip/选中时间范围序列化成 JSON hint 注入用户消息；图像提及内联为 base64 image block。范围为半开区间(start 含 end 不含)。
- `MCPHTTPServer/MCPService` (actor) — 本地 MCP HTTP 服务：NWListener 仅绑 127.0.0.1:19789，每 TCP 连接一对 Server+StatelessHTTPServerTransport；手写 HTTP 解析，处理 /mcp、/.well-known/oauth-protected-resource、SSE GET。
- `RippleEngine/OverwriteEngine` (enum) — (引用自 Editor/)纯函数引擎。RippleEngine 算波纹位移(合并范围、按 end<=clip.start 累加左移量、push 右移)；OverwriteEngine 算覆盖落点的 remove/trimEnd/trimStart/split 动作。

**核心算法/逻辑(供 Rust 复刻)**:
- 【帧/秒基本换算】timeline 有固定 fps；frame = round(seconds × fps)。所有工具时间单位默认是项目帧(timeline fps)，不是源媒体 fps。clip.endFrame = startFrame + durationFrames(半开区间 [start,end))。timeline.totalFrames = 各 track 的 max(clip.endFrame)，空则 0。Swift Int(Double.rounded()) 默认 round-half-away-from-zero，Rust 复刻需用 (x).round()。
- 【trim/speed 与源-时间线映射(最关键)】clip.sourceFramesConsumed = round(durationFrames × speed)。源帧→时间线帧：timelineFrame = round(startFrame + (sourceFrame − trimStartFrame)/max(speed,0.0001))；要求 sourceFrame≥trimStartFrame 且结果落在 [startFrame,endFrame) 否则视为不可见。get_transcript 的项目帧 P → 该 clip 源 trim 偏移：trimStartFrame + (P − startFrame)×speed。
- 【spanFrames(转写词→项目帧)】先把源秒区间钳到 clip 可见窗口：visStart=trimStartFrame(源帧)，visEnd=visStart + durationFrames×max(speed,0.0001)；s=max(start×fps, visStart)，e=min(end×fps, visEnd)，若 e<=s 丢弃；再 toTimeline(x)=round(startFrame + (x−visStart)/max(speed,0.0001))，返回 (a, max(a,toTimeline(e)))，保证 end>=start。边界跨越的词只产出真实碎片，零长词(start==end 四舍五入成 0 帧)会被丢弃——系统提示明确要求按词级而非段级去重。
- 【get_transcript 词归属】对每个音/视频 clip 取其源转写词，按词中点判定归属：midFrame=(s+e)/2×fps，要求 visStart<=midFrame<visEnd，故跨 clip 缝的词只发射一次；再经 spanFrames 映射；窗口过滤 f.end<=startFrame 或 f.start>=endFrame；按 (start,end) 排序；全局 10000 词上限，超出给 nextStartFrame 分页。每源仅转写一次(缓存)，单资产失败跳过不致命。
- 【add_clips 覆盖式落点】对每个 entry：校验 trackIndex 在范围、资产类型与轨道类型兼容(video/image 可互换；audio 需 audio 轨)、durationFrames>=1、startFrame>=0、trim>=0。trackIndex 全省略=自动建轨(视觉类共享一条 video 轨、音频类共享一条 audio 轨，都插在 index 0)；混用(部分给部分省略)直接拒绝。放置前对落点区间 clearRegion(覆盖式裁/分/删已有 clip)，再 placeClip。批内按(音频优先, trackId, startFrame)排序放置。带音轨的视频放到 video 轨会自动在 audio 轨创建 linkGroupId 关联的镜像音频 clip。整批一个 undo。
- 【clearRegion/OverwriteEngine 覆盖决策】对落点区间 [regionStart,regionEnd) 内每个相交 clip：完全被包含→remove；clip 跨整个区间(cs<start 且 ce>end)→split(左半 duration=regionStart−cs；右半 startFrame=regionEnd, rightTrimStart=trimStart+round((regionEnd−cs)×speed), rightDuration=ce−regionEnd)；仅左缘相交(cs<start)→trimEnd(newDuration=regionStart−cs)；仅右缘相交→trimStart(newStartFrame=regionEnd, newTrimStart=trimStart+round((regionEnd−cs)×speed), newDuration=ce−regionEnd)。
- 【insert_clips 波纹插入】trackIndex 必填。entries 从 atFrame 起首尾相接铺放；总推移量=各 entry duration 之和。推移施加到目标轨 + 所有 syncLocked 轨 + 自动镜像音频落点轨。插入前对每条被推轨上跨 atFrame 的 clip 做 split(使其右半随波纹走而非被覆盖)；splitClip 也会切并重组其链接伙伴。RippleEngine.computeRipplePush：startFrame>=insertFrame 的 clip 一律 +pushAmount。
- 【ripple_delete_ranges(波纹删除，两模式)】恰好二选一传 clipId 或 trackIndex。clipId 模式：ranges 钳到该 clip 可见区间，units 可为 seconds(源秒，经 toFrame=startFrame+(v×fps−trimStart)/max(speed,0.0001) 映射)或 frames(项目帧)。trackIndex 模式：ranges 必须是 frames(项目帧)，可跨该轨任意多 clip。每个 range 要求 end>start，二元组。合并重叠范围(mergeRanges：排序后 range.start<=last.end 即并)。被触及 clip 的链接 A/V 伙伴在同范围被切以保持同步。删除后剩余 clip 左移闭合空隙；syncLocked 轨随之左移以保对齐(其内容不被切)。若某 syncLocked 轨吸收位移会越过帧 0 或产生碰撞→整体 refused 不改任何东西。返回 anchor 轨删后布局(clip ids/frames)免重读。
- 【RippleEngine.computeRippleShiftsForRanges 左移量】对每个剩余 clip：左移量 = 所有满足 range.end<=clip.startFrame 的已删范围长度之和；>0 才产生 ClipShift(newStartFrame=startFrame−shift)。validateShifts 干跑：任一 clip 移后 start<0 报“移过时间线起点”；排序后相邻区间 start<前一 end 报“无空间波纹”。
- 【split_clip 与关键帧切分】atFrame 必须严格在 (startFrame,endFrame) 内。splitOffset=atFrame−startFrame；leftSource=round(splitOffset×speed)，rightSource=round((duration−splitOffset)×speed)。左 clip：duration=splitOffset, trimEnd+=rightSource, fadeOut=0；右 clip：新id, startFrame=atFrame, duration=duration−splitOffset, trimStart+=leftSource, fadeIn=0。每条关键帧轨在切点采样出边界关键帧并各自重基(左保留<=splitOffset 并补边界点；右过滤>=splitOffset 后整体减 splitOffset 并补 0 帧边界点)以保曲线连续。有链接组则同时切所有伙伴并把右半重组为新链接组。
- 【move_clips】每个 move 至少给 toTrack 或 toFrame 之一；toTrack 必须与 clip 媒体类型兼容。链接伙伴跟随：startFrame 以增量传播(toFrame−当前 startFrame，伙伴新帧=max(0, 伙伴 start+delta))以保 l-cut/j-cut 偏移；轨道变化不传播。实现先把被移 clip 从源轨摘除→对各目标区间 clearRegion(覆盖式)→按精确目标帧落下→各轨排序→剪空轨。
- 【set_clip_properties】对 clipIds 施加同一组值(duration/trim/speed/volume/opacity/transform/文本字段)。文本专用字段(content/fontName/fontSize/color/alignment)若 clipIds 含非文本 clip→拒绝。speed 改动：若未同时给 durationFrames 且 speed>0，按 sourceConsumed=duration×旧speed 重算 duration=max(1, round(sourceConsumed/新speed))。设 volume/opacity 标量会清空该属性已有关键帧轨。改 duration/speed 都会 clampKeyframesToDuration + clampFadesToDuration。文本改 content/font 且未显式给 transform 时自动 refit 包围盒(fitTextClipToContent)。timing 类(duration/trim/speed)传播到链接伙伴(伙伴若是文本则跳过 trim/speed)。
- 【set_keyframes 行解析与插值】property∈{volume,opacity,rotation,position,scale,crop}。行格式 [frame, ...values, interp?]，interp∈{linear,hold,smooth}(默认 smooth)。value 数量按属性：scalar=1(volume/opacity/rotation)、pair=2(position 是左上角 x,y 归一化；scale 是归一化宽高，非缩放因子)、crop=4(top,right,bottom,left 归一化边距)。帧是 clip 相对(0=clip 首帧)。内部按 frame 排序并去重(同帧后者覆盖)。空数组清空轨。采样算法 KeyframeTrack.sample：空→fallback；单点→该值；frame<=首/>=尾→边界值；否则取首个 frame>查询的 b，a=b前一，raw=(frame−a.frame)/(b.frame−a.frame)，按 a.interpolationOut：hold→a.value；linear→lerp(a,b,raw)；smooth→lerp(a,b,smoothstep(raw))，smoothstep(t)=t²(3−2t)。运动关键帧(position/scale/rotation)激活时覆盖静态 transform。
- 【fade 包络(与关键帧叠加)】fadeMultiplier(rel)：rel=frame−startFrame，越界(rel<0 或 rel>duration)返回 0；inMul=fadeIn>0 时 t=min(1,rel/fadeIn)，smooth 则 smoothstep(t)；outMul=fadeOut>0 时 t=min(1,(duration−rel)/fadeOut)，取 min(inMul,outMul)。clampFadesToDuration：fadeIn=clamp(0..duration)，fadeOut=clamp(0..duration−fadeIn)。有效音量=volume × dB转线性(关键帧采样,VolumeScale.linearFromDb) × fadeMultiplier；音频 clip 不应用 fade 到 opacity。
- 【ID 前缀压缩(双向)】实体 UUID 在输出文本里替换为最短(>=8 字符)且全集内唯一的前缀(shortIdMap：从 8 起逐字符加长直到无他者共享该前缀)。输入侧 expandingIdPrefixes：扫描已知 scalar/array ID 键，前缀唯一则展开成全 UUID，多义则抛 Ambiguous 错误，未知则原样透传让具体工具报 not-found。ID 全集 = 所有 track.id/clip.id/captionGroupId/linkGroupId/asset.id/folder.id。
- 【get_timeline 负载压缩】默认值剥离：mediaType=video、sourceClipType=mediaType、speed=1、volume/opacity=1、trim/fade=0、单位 transform/crop、默认 textStyle、track muted/hidden=false、syncLocked=true。文本 clip 不报 trim。关键帧轨折叠成 keyframes 行(frame,值...,非 smooth 才附 interp)。同 captionGroupId 的 clip 折叠成 captionGroups：众数残差属性提到 shared，每行 [clipId,startFrame,durationFrames,text]，字幕框宽高(自动 fit)剔除，每组上限 200 行超出分页，偏离众数的字幕 clip 单独列出。窗口 [startFrame,endFrame) 只返回相交 clip，浮点数四舍五入到 3 位。
- 【undo 治理】每次工具执行后若 timeline 真的变了且非 undo 工具且无错，记录 undoManager.undoActionName 入 agentUndoStack。undo 工具只在栈顶动作名等于当前 undoManager.undoActionName 时执行 undoManager.undo() 并弹栈；否则拒绝(“最近改动不是助手做的”)，保护用户手动编辑。底层撤销用 withTimelineSwap：禁登记跑改动→对比 before/after timeline→登记一个双向 timeline 整体快照 swap(registerTimelineSwap 自递归注册逆向)。
- 【add_captions 流程】on-device 转写(Speech)候选 clip→若 autoDetect 选说话词数最多的轨(dominantSpeechTrack)→CaptionBuilder.phrases 按行宽是否 fit(captionLineFits：自然尺寸宽<=画布宽×比率)与最小时长断句→短语按与 clip 可见区间重叠最大且重叠>=短语一半归属(bestClip)→应用大小写(auto/upper/lower)→CaptionBuilder.specs 生成共享 captionGroupId 的文本 clip→在 index 0 插新 video 轨并 placeTextClips，整体一个 undo。语言经 Transcription.matchLocale 校验支持。
- 【import_media 安全约束】source 恰好二选一 url/path/bytes。url 必须 https、无内嵌凭据、有 host；类型由扩展名推断或 mimeType 覆盖；后台下载(超 1GB 取消)。bytes 需 mimeType，base64 上限约 15MB，写入项目 media 目录。path 可为目录(递归镜像子文件夹为媒体文件夹)。支持类型：video(mov/mp4/m4v)、audio(mp3/wav/aac/m4a)、image(png/jpg/jpeg/tiff/heic)、json(Lottie)，其余拒绝。
- 【generate_*/upscale 异步提交】先检查 AccountService.isSignedIn && hasCredits，否则报错引导登录/充值。经 list_models 校验 model 支持的 duration/aspectRatio/resolution/references/voice/类型；构造 GenerationInput→对应 *GenerationSubmission.make().submit() 到 editor.generationService(走 Convex 后端)，立即返回占位 assetId，后台完成后在 get_media 可见。video-to-audio 给时间线 span 时会自动把结果放到该 span。

**苹果框架使用**:
- AVFoundation [high] — AVURLAsset/AVAssetImageGenerator 在 inspect_media 抽视频帧、OverviewRenderer 抽候选帧；CompositionBuilder 产出 AVComposition+AVVideoComposition 供 inspect_timeline 渲染合成帧；读 video track、loadTracks、appliesPreferredTrackTransform、时间容差控制
- CoreMedia [low] — CMTime(seconds:preferredTimescale:600)/CMTimeScale(fps) 做抽帧时间点与时基换算，requestedTimeTolerance 控制抽帧精度
- CoreGraphics [medium] — CGContext 合成 Lottie 透明帧到灰底、OverviewRenderer 拼网格雪碧图、inspect_timeline 合成视频+文本层；CGImage 像素操作与 sRGB 色彩空间
- ImageIO [low] — CGImageSourceCreateWithURL + CopyPropertiesAtIndex 读图片像素尺寸/EXIF/方向/色彩模型(inspect_media 图像分支)
- CoreText [low] — OverviewRenderer 用 CTFontCreateWithName/CTLineCreateWithAttributedString/CTLineDraw 把时间码烧入雪碧图块
- Speech [high] — on-device 语音转写(经 Transcription/TranscriptCache)，支撑 inspect_media/get_transcript/add_captions/search_media spoken；matchLocale/supportedLocales 校验语言
- Network [low] — MCPHTTPServer 用 NWListener 仅绑 127.0.0.1:19789、NWConnection 收发；NWParameters.requiredLocalEndpoint 锁回环防 LAN 访问
- AppKit [medium] — 波纹拒绝时 NSSound.beep()；UndoManager 全套(beginUndoGrouping/registerUndo/undoActionName/disableUndoRegistration) 是撤销治理的核心；NSAttributedString
- SwiftUI [high] — 整个 Panel/* 聊天 UI(AgentPanelView/AgentInputBox/AgentMessageView/ChatHistoryList/MarkdownText/MentionPopover/ThinkingDots)，需 React/TS 重建
- Observation [low] — @Observable 让 AgentService/MCPService 状态驱动 SwiftUI 刷新
- Foundation [low] — URLSession.bytes 拿 SSE 流、JSONSerialization/JSONEncoder 编解码、URLSession.download 导入下载、Regex(/UUID/)、FileManager、UserDefaults、NotificationCenter

**闭源云**:是。三处触达闭源云：(1) AnthropicClient 直连 https://api.anthropic.com/v1/messages(用户自带 key 时，流式生成式 AI)；(2) PalmierClient 经 ClerkKit 取 JWT 后请求自有 Convex 后端 BackendConfig.convexHttpURL + /v1/agent/stream(无 key 的付费/积分代理路径，本质仍是 Anthropic 模型)；(3) 所有 generate_video/image/audio 与 upscale_media 经 editor.generationService/GenerationBackend 提交到 Convex 后端调度第三方生成式模型(Seedance/Kling/Veo/Nano Banana/ElevenLabs/Lyria 等)，AccountService(Clerk 登录+积分)门控。import_media 的 url 模式还会发起任意 https 下载。on-device 转写(Speech)与语义检索不触云。

**移植策略**:分层处理：①工具执行/校验/帧秒换算/ID 压缩/负载压缩(ToolExecutor 全家 + AgentMentionContext + ShortId + Timeline/Keyframe/RippleEngine/OverwriteEngine 算法)——可 direct-port 到 Rust core，是 OpenTake 领域模型与 MCP server 的核心，coreLogic 中的公式按整数帧 + round(half-away-from-zero) 一比一复刻即可。②LLM 客户端(AgentClient/SSE/RequestBody)——Rust 用 reqwest + eventsource 流重写，提示缓存 cache_control 边界逻辑照搬；自带 Anthropic key 直连可保留，Convex/Clerk 付费代理属 closed-cloud，需换成 OpenTake 自己的鉴权/计费或去掉只留 BYO-key。③MCP server(MCPHTTPServer/MCPService/ToolResult/ToolDefinitions)——改用 Rust MCP SDK(rmcp) + 仅绑 127.0.0.1，工具 schema(JSON)可几乎照抄，这是 OpenTake 暴露给外部 agent 的关键能力。④媒体读取(inspect_media 抽帧/overview/inspect_timeline 合成、OverviewRenderer 亮度去重 LumaGrid/雪碧图、Lottie)——AVFoundation→FFmpeg(ffmpeg -ss 抽帧、scale、合成滤镜) + image crate 拼图；CoreText 烧字→ab_glyph/cosmic-text。⑤Speech 转写与语义/视觉检索——Apple Speech 无 Rust 等价，需换 whisper.cpp(转写) + CLIP/embedding(视觉)+ 文本检索；这是工作量大头。⑥UndoManager 撤销治理——Rust 自建命令栈/timeline 快照 swap(本模块已是整 timeline diff swap 模式，易移植)，agentUndoStack 守卫逻辑照搬。⑦SwiftUI Panel/*——React/TS 全量重建(ui-rebuild)。⑧生成/上采样/导入——cloud-rebuild：重接 OpenTake 自己的生成后端或第三方 API。⑨AppKit NSSound.beep 等纯反馈可丢弃或换前端提示。

**关键文件**:Agent/AgentService.swift、Agent/Tools/ToolExecutor.swift、Agent/Tools/ToolExecutor+Clips.swift、Agent/Tools/ToolExecutor+Timeline.swift、Agent/Tools/ToolDefinitions.swift、Agent/Tools/AgentInstructions.swift

