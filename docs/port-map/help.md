# Help 移植规格

**职责**:
- 渲染快捷键速查表:把硬编码的 7 组(Playback/Tools/Editing/Timeline/File/Edit/View)快捷键按两列布局展示,左列前 4 组、右列其余 3 组
- 渲染 MCP 接入向导:动态拼接服务器地址 http://127.0.0.1:19789/mcp,为 4 种 MCP 客户端生成对应的 CLI 命令 / JSON 配置 / 深链接,并提供复制按钮与可折叠的手动配置说明
- Cursor 一键安装:把 {type:http,url} 配置做 JSON 序列化→Base64→URL 百分号编码,拼成 cursor:// 深链接并用 NSWorkspace 打开
- Claude Desktop 一键安装:打开 App bundle 内置的 palmier-pro.mcpb 文件
- 渲染反馈表单:多行描述(上限 10000 字符)、未登录时的可选邮箱、是否允许回访的勾选、截图缩略图预览与开关、环境信息提示;提交成功后切换为致谢视图
- 捕获主窗口截图作为反馈附件:用 AppKit 把当前主窗口 contentView 离屏渲染为 PNG,超过 1920px 时按比例缩小
- 通过 NSWindowController 单例管理 Help 窗口与 Feedback 窗口的创建、定位、深色外观与显示
- 把剪贴板复制(NSPasteboard)、提交反馈(经 AccountService→Convex 云)等副作用封装在小型按钮/表单组件里

**核心类型**:
- `HelpTab` (enum) — Help 窗口的 Tab 枚举,仅两个 case:shortcuts(图标 keyboard)与 mcp(图标 network)。CaseIterable+Identifiable,驱动侧边栏列表
- `HelpView` (struct) — Help 主界面 SwiftUI View:左侧 220pt 固定宽侧边栏 + 右侧 detail 区,detail 根据 selectedTab 切换 ShortcutsPane 或 MCPInstructionsPane。最小尺寸 820x520
- `HelpWindowController` (class) — @MainActor NSWindowController 单例(shared),用 NSHostingController 承载 HelpView,管理无边框深色玻璃窗口;show(tab:) 用 .id(UUID()) 强制重建视图以切到指定 Tab
- `ShortcutsPane` (struct) — 快捷键速查表 View,核心是 static let allShortcuts 这份硬编码数据(7 组,每组若干 (按键, 描述) 元组),以及把它切成左右两列的 leftColumn/rightColumn
- `ShortcutGroup` (struct) — 快捷键分组数据模型:title:String + shortcuts:[(String,String)]
- `MCPInstructionsPane` (struct) — MCP 接入向导 View:所有连接字符串/JSON/深链接都是基于 MCPService.port 计算的 computed property;含 Overview/Server URL/Cursor/Claude Desktop/Claude Code/Codex 六个 section
- `FeedbackView` (struct) — 反馈表单 View:管理 message/email/includeScreenshot/mayContact/isSending/errorText/didSend 等本地 @State;canSubmit 校验(非空且≤10000 字符);submit() 异步调用 AccountService.sendFeedback
- `FeedbackWindowController` (class) — @MainActor NSWindowController 单例,show(prefill:) 时先于窗口成为 key 之前捕获主窗口截图(避免把反馈窗自身拍进去),再构建 FeedbackView
- `FeedbackScreenshot` (enum) — @MainActor 无实例的工具命名空间,captureMainWindow() 把主窗口离屏渲染为 PNG 并按需缩小;依赖 AppKit 的 cacheDisplay/NSBitmapImageRep/CGContext

**核心算法/逻辑(供 Rust 复刻)**:
- 【快捷键数据是静态硬编码,不是从配置/实时绑定读取的】ShortcutsPane.allShortcuts 共 7 组,顺序固定:1) Playback: Space=Play/Pause, ←=Step Backward, →=Step Forward, Shift+←=Skip Backward, Shift+→=Skip Forward;2) Tools: V=Selection Tool, C=Razor Tool;3) Editing: Cmd+K=Split at Playhead, [或Q=Trim Start to Playhead, ]或W=Trim End to Playhead, Backspace=Delete, Shift+Backspace=Ripple Delete, Opt+Drag=Duplicate Clip;4) Timeline: Shift+Drag Ruler=Select Range, Drag Range Edge=Adjust Range, I=Mark Range Start, O=Mark Range End, Opt+Scroll=Zoom to Cursor, Pinch=Zoom to Cursor, Cmd+Scroll=Scroll Horizontally;5) File: Cmd+N=New, Cmd+O=Open, Cmd+S=Save, Cmd+Shift+S=Save As, Cmd+I=Import Media, Cmd+E=Export;6) Edit: Cmd+Z=Undo, Cmd+Shift+Z=Redo, Cmd+X=Cut, Cmd+C=Copy, Cmd+V=Paste, Cmd+A=Select All;7) View: Cmd+F=Full Screen, `=Maximize Focused Panel, Cmd+Scroll=Zoom Preview to Cursor, Esc=Deselect & Reset Tool。注意:这只是一份给人看的速查表,真正的快捷键行为实现不在本模块——它是上游编辑器键位逻辑的权威清单,复刻时应据此核对 Rust/前端实际键绑定。
- 【快捷键两列分配规则】leftColumn = allShortcuts.prefix(4)(即前 4 组 Playback/Tools/Editing/Timeline),rightColumn = allShortcuts.dropFirst(4)(即后 3 组 File/Edit/View)。两列各自 VStack,组间距 20,组内行距 6,按键列固定宽 118pt 左对齐等宽字体,描述列 fixedSize 不换行。
- 【MCP 端点拼接规则,单位/常量精确】serverURL = "http://127.0.0.1:\(MCPService.port)",其中 MCPService.port = 19789(UInt16 常量);mcpEndpoint = serverURL + "/mcp"。即固定本地回环地址 http://127.0.0.1:19789/mcp。所有客户端配置都从这一个端点派生。
- 【Claude Code 命令】claude mcp add --transport http palmier-pro {mcpEndpoint}。【Codex 命令】codex mcp add palmier-pro --url {mcpEndpoint}。【Cursor JSON】mcpServers.palmier-pro = {type:http, url:mcpEndpoint},说明放进 ~/.cursor/mcp.json。【Claude Desktop JSON】用 npx -y mcp-remote {mcpEndpoint} --allow-http --transport http-only 作为 command/args(因 Claude Desktop 不支持直连 http transport,需 mcp-remote 桥接)。
- 【Cursor 深链接生成算法,需一比一复刻】config = ["type":"http", "url": mcpEndpoint];步骤:(1) JSONSerialization.data(config, options:[.sortedKeys]) 得到键名按字典序排序的 JSON 字节;(2) data.base64EncodedString();(3) 对 base64 字符串再做 addingPercentEncoding(withAllowedCharacters:.urlQueryAllowed) 百分号编码;(4) 拼成 URL: cursor://anysphere.cursor-deeplink/mcp/install?name=palmier-pro&config={encoded}。任一步失败返回 nil(按钮点击则什么都不做)。复刻要点:必须 sortedKeys 保证确定性,且 base64 之后还要再 URL-encode 一层。
- 【Claude Desktop 一键安装】openClaudeDesktopBundle():取 Bundle.main.resourceURL,拼接子路径 "palmier-pro.mcpb";仅当 FileManager 确认该文件存在时,用 NSWorkspace.shared.open 打开(交给系统注册的 .mcpb 处理器/Claude Desktop)。文件不存在则静默不动作。
- 【反馈表单提交校验规则】maxMessageLen = 10000;trimmedMessage = message 去首尾空白与换行;canSubmit = (!isSending) && (!trimmedMessage.isEmpty) && (message.count <= 10000)。注意非空判断用 trim 后的文本,但长度上限判断用未 trim 的原始 message.count(边界细节:纯空白也算长度)。
- 【是否有回信邮箱 hasReplyEmail】若已登录(account.isSignedIn)则取决于 account.account?.user.email 是否非 nil;若未登录则取决于 trimmedEmail 非空。该值决定『允许回访』勾选框是否可用(disabled 当无邮箱),且无邮箱时即使勾选,提交时 mayContact 也被强制改为 false(submit 里 mayContact: hasReplyEmail ? mayContact : false)。
- 【提交流程 submit()】guard canSubmit;清空 errorText;isSending=true;启动 @MainActor Task,defer 里复位 isSending;计算 attachedScreenshot = (includeScreenshot ? screenshot : nil)?.base64EncodedString()(即用户关掉开关就不带截图);await AccountService.shared.sendFeedback(message: trimmedMessage, email: trimmedEmail 为空则 nil, mayContact: 上述强制规则, screenshotPngBase64: attachedScreenshot, appVersion, osVersion);成功 didSend=true(切到致谢视图),失败把 error.localizedDescription 写入 errorText 显示为红字。
- 【环境信息采集】appVersion = "{CFBundleShortVersionString} ({CFBundleVersion})"(任一缺失用 "?");osVersion = ProcessInfo.processInfo.operatingSystemVersion 拼成 "major.minor.patch"。这两个值随每次反馈一并上报。
- 【致谢文案分支 successDetailText】replyAddr = 登录邮箱 ?? (trimmedEmail 为空则 nil 否则 trimmedEmail);若 replyAddr 存在且 mayContact:『...may reach out at {replyAddr}』;若 replyAddr 存在但不允许:『...won't email you, as requested』;若无邮箱:『...Add an email next time...』。
- 【主窗口截图算法 captureMainWindow(),需用 FFmpeg 体系外的方案复刻】候选窗口选择顺序:NSApp.keyWindow ?? NSApp.mainWindow ?? 第一个 (isVisible 且 title 不以 "Send feedback" 开头) 的窗口;取其 contentView;用 view.bitmapImageRepForCachingDisplay(in: bounds) + view.cacheDisplay(in:to:) 做离屏位图缓存;representation(using:.png) 得 PNG。关键时序:FeedbackWindowController.show() 必须在反馈窗口成为 key 之前先截图,否则会把反馈窗自身拍进去。
- 【截图缩放规则 downscaledIfNeeded】maxDimension = 1920;若宽和高都 ≤1920 直接返回原 PNG;否则 scale = min(1920/width, 1920/height),newW=Int(width*scale)、newH=Int(height*scale);用 CGContext(8 位/分量, premultipliedLast, sRGB 或源色彩空间, interpolationQuality=.high)把 cgImage 重绘到新尺寸,再转回 PNG。任一步失败均回退返回原始 PNG(降级而非报错)。
- 【窗口外观】Help 与 Feedback 窗口都是:深色 darkAqua 外观、半透明(backgroundColor = base.withAlphaComponent(0.4)、isOpaque=false)、标题隐藏、titlebar 透明、可拖背景移动、fullSizeContentView。Feedback 额外 isReleasedWhenClosed=false。show 时 .id(UUID()) 强制重建保证状态重置。

**苹果框架使用**:
- SwiftUI [high] — 全部三个面板与反馈表单的视图层(View/ScrollView/VStack/HStack/Toggle/TextEditor/TextField/ProgressView、@State/@Bindable、withAnimation 等)
- AppKit [high] — NSWindowController/NSWindow/NSHostingController 管理无边框深色窗口;NSWorkspace.shared.open 打开 cursor:// 深链接与 .mcpb 文件;NSPasteboard 复制文本;NSImage 展示截图缩略图;NSAppearance(darkAqua);NSApp.activate/keyWindow/mainWindow
- AppKit(离屏渲染子集) [medium] — FeedbackScreenshot 截图:NSView.bitmapImageRepForCachingDisplay + cacheDisplay、NSBitmapImageRep、CGContext/CGImage/CGColorSpace 做 PNG 编码与缩放
- Foundation [none] — JSONSerialization 生成 Cursor 深链接配置;Base64/百分号编码;Bundle.main info plist 读取版本号与资源路径;ProcessInfo 取 OS 版本;FileManager 检查 .mcpb;Task.sleep 控制复制按钮提示
- Combine [low] — 经 AccountService.shared 的 @Observable/@Bindable 间接订阅登录状态(isSignedIn、邮箱)

**闭源云**:是。反馈提交是闭源云触点:FeedbackView.submit() → AccountService.sendFeedback(...) → convex.action(\"feedback:send\", with: args)(args 含 message/mayContact/appVersion/osVersion 以及可选 email、screenshotPngBase64)。底层走 ConvexMobile 客户端连接 Convex 部署(BackendConfig.convexDeploymentURL),身份用 ClerkKit/ClerkConvex。若 convex 未配置则抛错『Backend not configured.』。注意:这里的 Convex 仅作反馈收集后端,本调用本身不触达生成式 AI 云(gener AI 模型调用在 Generation/* 模块)。另一处网络相关是 MCP server,但那是本机 127.0.0.1:19789 的本地 HTTP server,不是闭源云。截图/快捷键/MCP 向导本身都无网络请求。

**移植策略**:整体在 OpenTake 用 React/TypeScript 重建为帮助面板(可做成应用内 Modal/独立窗口,Tauri 多窗口或单窗口路由皆可),Rust 侧基本不需要承载逻辑。分块方案:(1) 快捷键面板——把这份硬编码清单直接搬成 TS 常量(建议做成 i18n 资源),按相同两列分配(前4组/后3组)渲染;务必让它与前端真实键位绑定保持一致,它是上游键位的权威来源。(2) MCP 向导——端口常量改由 Rust core 暴露(本地 MCP/HTTP server 端口,可仍用 19789 或改为可配置),前端用同样算法拼 4 种客户端配置;Cursor 深链接生成需一比一复刻『JSON(sortedKeys)→base64→urlQueryAllowed 百分号编码→cursor://anysphere.cursor-deeplink/mcp/install?name=palmier-pro&config=』,可在 TS 用 JSON.stringify(按 key 排序)+btoa+encodeURIComponent 实现;打开深链接/.mcpb 用 Tauri 的 shell/opener 插件替代 NSWorkspace;Claude Desktop 的 .mcpb 包需在新打包流程里产出或改为纯 JSON 手动配置指引。(3) 复制按钮用 navigator.clipboard 或 Tauri clipboard 插件,1.4s 后复位提示的行为照搬。(4) 反馈表单——UI 用 React 重写,校验规则(≤10000 字符、trim 非空、无邮箱强制 mayContact=false、关开关不带截图)逐条复刻;提交后端要在 OpenTake 自建(自有 HTTP 端点/邮件/工单),不要复用 Convex『feedback:send』这个闭源云 action——属 cloud-rebuild 的子项。(5) 截图附件——用 Tauri 的窗口/屏幕截图能力或前端 canvas 抓取替代 AppKit cacheDisplay;保留『提交前先截图、排除反馈窗自身、>1920px 等比缩小、失败降级返回原图』的规则。窗口外观(深色半透明无边框)用前端样式 + Tauri 窗口装饰配置近似还原。

**关键文件**:Sources/PalmierPro/Help/HelpView.swift、Sources/PalmierPro/Help/ShortcutsPane.swift、Sources/PalmierPro/Help/MCPInstructionsPane.swift、Sources/PalmierPro/Help/FeedbackView.swift、Sources/PalmierPro/Help/FeedbackScreenshot.swift

