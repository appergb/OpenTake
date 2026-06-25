# 上游证据索引（所有结论的代码出处，绝对路径）

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
