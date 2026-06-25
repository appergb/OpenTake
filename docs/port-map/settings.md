# Settings 移植规格

**职责**:
- 渲染设置窗口的整体布局:侧边栏(IdentityStrip + 分页按钮)+ 详情区(标题 + ScrollView),用 AppKit NSWindowController 托管一个独立暗色磨砂窗口(SettingsWindowController.shared)。
- 分页可见性逻辑:当账户后端未配置(isMisconfigured)时隐藏 Account 分页;若当前选中分页不可见则回退到第一个可见分页或 General。
- Account 分页:根据 isLoading / isSignedIn / isPaid 状态机切换 UI;展示订阅计划卡片(价格/折扣价/月度积分额度)、剩余积分进度条、Top-off 充值输入,触发登录/登出/订阅/管理订阅/购买积分等账户动作(全部转发给 AccountService)。
- General 分页:通知开关(写 AppNotifications.isEnabled 并按需 requestAuthorization)与隐私遥测开关(写 Telemetry.isEnabled,提示需重启);二者都是 UserDefaults 布尔偏好。
- Models 分页:从 ModelCatalog(image/video/audio 三类)拉取模型列表,提供搜索过滤与每个模型的启用/禁用开关(写 ModelPreferences,本质是一个 disabledModelIds 集合)。
- Agent 分页:管理用户自带的 Anthropic API Key(SecureField + 掩码显示 + 存/删 macOS Keychain),以及本地 MCP HTTP 服务器(127.0.0.1:19789)的运行状态指示与开关。
- Storage 分页:显示并清理磁盘缓存(预览/波形/缩略图)、显示并清理设备端媒体搜索索引(embeddings)、显示并移除已下载的本地 SigLIP 模型;字节大小用 ByteCountFormatter 展示,清理操作在后台 Task 中跑。
- 提供可复用子组件 SettingsToggleRow(标题+副标题+开关)供各分页统一样式。

**核心类型**:
- `SettingsTab` (enum) — 设置分页枚举(account/general/models/agent/storage),提供 label 与 SF Symbol 名;CaseIterable 驱动侧边栏。Rust/前端可直接复刻为字符串枚举。
- `SettingsView` (struct) — 设置窗口根视图:HStack(侧边栏 220pt + 详情区);含分页可见性过滤逻辑与初始分页注入。
- `SettingsWindowController` (class) — AppKit 单例窗口控制器(@MainActor NSWindowController.shared),创建并托管设置窗口(暗色、磨砂、无标题栏、可拖拽、frameAutosaveName=PalmierProSettings-v2),show(tab:) 可定位到指定分页并强制刷新(.id(UUID()))。纯 macOS 窗口管理,Tauri 下用独立 WebviewWindow 替代。
- `SettingsToggleRow` (struct) — 通用开关行(标题+副标题+右侧 switch),被通知/隐私分页复用。纯展示组件。
- `AccountPane` (struct) — 账户分页视图;消费 AccountService.shared,渲染订阅/积分/计划卡片,触发计费动作。本地状态仅 topOffDollars(默认 20)。
- `AgentPane` (struct) — Agent 分页;管理 Anthropic Key(经 AnthropicKeychain 存取 Keychain,掩码=36 个圆点+末 4 位)与 MCP 服务器开关(经 AppState.setMCPEnabled)。
- `ModelsPane` (struct) — 模型分页;从 ModelCatalog 取 image/video/audio 列表,按 displayName 做不区分大小写的子串搜索,每行开关读写 ModelPreferences。
- `StoragePane` (struct) — 存储分页;聚合三处磁盘占用(预览缓存、搜索索引、本地模型),提供清理/移除按钮,后台计算字节数并刷新。
- `NotificationsPane / PrivacyPane` (struct) — General 分页的两块:系统通知开关、匿名崩溃遥测开关(改后提示重启)。仅读写 UserDefaults 并联动 AppNotifications/Telemetry。

**核心算法/逻辑(供 Rust 复刻)**:
- 【订阅计划与积分的纯数值规则,务必一比一复刻】预算积分 budgetCredits = (plan.monthlyBudgetCredits ?? 0) + (user.purchasedCredits ?? 0);已花费 spentCredits = user.spentCreditsThisPeriod ?? 0;剩余 remainingCredits = max(0, budgetCredits - spentCredits);hasCredits = remaining > 0。进度比例 remaining = budget>0 ? min(1.0, left/budget) : 0,其中 left = max(0, budget - spent)。
- 【积分进度条配色阈值】按剩余比例 r:r < 0.05 → 红色;0.05 ≤ r < 0.25 → 橙色;否则 → 主题强调色。这是 CreditSummaryView.barColor 的精确分段。
- 【计划卡片价格显示】有效月价 effectiveMonthlyPriceUsd = hasDiscount ? discountedMonthlyPriceUsd! : monthlyPriceUsd;hasDiscount 当且仅当 discountedMonthlyPriceUsd 存在且 < monthlyPriceUsd(此时原价加删除线展示)。所有价格是整数美元。
- 【Top-off 充值换算与校验(关键)】积分 = max(0, dollars) * 100(即 1 美元=100 积分);合法区间 isValid = dollars ∈ [TopOffLimits.minDollars, TopOffLimits.maxDollars]。注意源码中两处常量不一致:TopOffLimits 定义为 min=5/max=1000,但 AccountPane 文案硬编码显示 "$\(min)–$\(max)"。校验在 UI(isValid)与 AccountService.buyCredits 双重进行;buyCredits 还有重入保护(isBuyingCredits 为真时直接返回)。复刻时以常量 5..1000 为准。
- 【账户状态机(AccountService)】isSignedIn = (!isMisconfigured && authState==.authenticated);tier = account.user.tier ?? .none;isPaid = tier != .none。UI 分三态:isLoading→"Loading…";isSignedIn 且 isPaid→订阅区+积分区;isSignedIn 未付费→订阅引导(有计划则显示 Pro/Max 卡片,否则显示两个升级按钮);未登录→"Sign in with Google"。lastError 非空时底部红字展示。
- 【订阅周期日期格式化】currentPeriodEnd 是以毫秒计的 Unix 时间戳(Double);转 Date = Date(timeIntervalSince1970: endMs/1000),再以 abbreviated 日期、省略时间格式化。cancelAtPeriodEnd==true 时额外橙字提示 "Cancels <date>";积分卡显示 "Resets <date>"。
- 【账户/计费云调用映射(需在 Rust 后端重建为对自有服务的调用)】登录=Clerk OAuth(provider .google,redirect palmier://callback);provision=convex.mutation "users:upsertFromAuth"(失败重试 3 次,间隔 500ms);账户数据=convex.subscribe "account:get";计划列表=convex.subscribe "billing:listPlans";订阅结账=convex.action "billing:createCheckoutSession"{tier};充值结账=convex.action "billing:createTopOffCheckoutSession"{dollars: Double};管理订阅=convex.action "billing:createPortalSession";反馈=convex.action "feedback:send"。
- 【打开计费 URL 的安全白名单(务必复刻)】openInBrowser 仅允许 https 且 host ∈ {checkout.stripe.com, billing.stripe.com},否则置 lastError="Refused to open untrusted URL." 不打开。这是防 open-redirect 的硬校验。
- 【Anthropic API Key 的存取与掩码】保存:trim 后非空才存,存入 Keychain(service=bundleId,account="anthropic-api-key",可访问性 kSecAttrAccessibleAfterFirstUnlock,先 SecItemUpdate,errSecItemNotFound 时 SecItemAdd);读取:DEBUG 下优先读环境变量 ANTHROPIC_API_KEY(trim 非空),否则读 Keychain(读出后 trim,空则视为无)。掩码 mask(key):若 key.count>4 → 36 个 U+2022 圆点 + 末尾 4 位明文;否则 32 个圆点。存/删后都发 NotificationCenter 通知 .anthropicAPIKeyChanged 让 AgentService 重建客户端。UI 中只要草稿 trim 非空就显示 Save 按钮,否则若已有 key 显示删除(垃圾桶)按钮。
- 【模型启用偏好(ModelPreferences)】数据结构=一个 disabledModelIds: Set<String>,持久化到 UserDefaults 键 "disabledModelIds"(存为字符串数组)。isEnabled(id)= !contains(id);setEnabled(id,true)=remove,(id,false)=insert,每次改动立即 persist。即默认全部启用,只记录被关掉的。
- 【模型搜索过滤】query trim+lowercase 后,对每类(image/video/audio)的 displayName.lowercased().contains(q) 过滤;q 为空则不过滤;过滤后空的 section 整段隐藏。模型目录来自 convex.subscribe "models:list"(ModelCatalog),未加载时显示 "Loading models…"。
- 【通知开关(AppNotifications)】偏好键 "io.palmier.pro.notifications.enabled",缺省视为 true(object==nil→true)。开启时调用 configure():仅当运行于 .app 包内(bundleURL 后缀==app 且 bundleId 含".")才 requestAuthorization([.alert,.sound]) 并设代理。生成完成通知文案:count>1→"<count> <type>s are ready in Palmier Pro.";否则有名字→"<name> is ready." 无名字→"Your <type> is ready."。点击通知回调会 reveal 对应资产。
- 【隐私遥测开关(Telemetry)】偏好键 "io.palmier.pro.telemetry.enabled",缺省 true。enabledForCurrentLaunch 在启动时快照一次;PrivacyPane.didChange = (当前开关值 != enabledForCurrentLaunch),为真时显示"需重启"提示(因为 Sentry 只在启动时按快照初始化一次)。Telemetry.start() 仅当本次启动启用且 DSN 非空才 SentrySDK.start(sendDefaultPii=false, tracesSampleRate=0.1, appHangTimeout=8s 等)。
- 【MCP 服务器开关(MCPService/AppState)】偏好键 "io.palmier.pro.mcp.enabled",缺省 true。固定端口 19789,绑 127.0.0.1。setMCPEnabled(true)→若未运行则 startMCPService(创建 MCPHTTPServer,注册 ToolDefinitions.all 工具与两个资源 palmier://models/{video,image});false→stop。UI 圆点:运行=绿,停=灰;运行时显示 "Running on 127.0.0.1:19789"。
- 【存储:磁盘缓存清理】缓存目录根=~/Library/Caches/PalmierPro;StoragePane 聚合两个 DiskCache 实例 [ImageVideoGenerator.cache("ImageVideos"), MediaVisualCache.diskCache("MediaVisualCache")]。size()=递归累加目录下所有文件 .fileSize;clear()=删除目录内所有顶层条目(保留目录本身)。显示路径把 $HOME 替换为 "~"。清理在 Task.detached 中执行,期间显示 "Clearing…",完成后刷新。Clear 按钮在清理中或字节为 0 时禁用。
- 【存储:搜索索引(EmbeddingStore)二进制格式 — 需精确复刻】文件后缀 .embed,路径 ~/Library/Caches/<subsystem>/Embeddings/<key>.embed。布局:magic(8 字节 ASCII "PALMEMB1")+ UInt32 小端 headerLen + header(JSON: model, modelVersion, samplerVersion, dim, count)+ count 行,每行 = 3 个 Float64(time, shotStart, shotEnd,共 24 字节)紧跟 dim 个 Float16 向量值(每行字节数 rowBytes = 3*8 + dim*2)。读出时把 Float16 转 Float32 存入扁平 count×dim 数组。文件总长必须恰好 = 8+4+headerLen+count*rowBytes,否则视为 corrupt。整数全部小端、无对齐(loadUnaligned)。写入用 .atomic。
- 【存储:缓存键(EmbeddingStore.key)】身份串 = "<文件路径>|<修改时间 timeIntervalSince1970(Double)>|<文件字节数>",对其 UTF8 取 SHA256,十六进制小写,取前 32 字符作为 key。索引时效性 isCurrent = header 的 model+modelVersion+samplerVersion 三者全部匹配当前值。
- 【存储:清理索引/移除模型的级联】Clear index = SearchIndexCoordinator.clearIndexGlobally():先 resetAll(取消所有项目正在进行的索引、清空内存中 loadedIndexes/failedIds),再 EmbeddingStore.clearAll()(删整个 Embeddings 目录),再 sweepAll()(重新入队)。Remove model = VisualModelLoader.remove():resetAll + 删除 ~/Application Support/PalmierPro/Models 整个目录 + 状态置 notInstalled。媒体搜索开关 setEnabled 写 SearchIndexConfig.enabled(键 "searchIndexEnabled",缺省 true),开→prepare()+sweepAll,关→cancelAll+释放 embedder。
- 【存储:本地模型下载与校验(ModelDownloader)】安装目录 ~/Application Support/PalmierPro/Models/<model>-v<version>/{ImageEncoder.mlmodelc, TextEncoder.mlmodelc, tokenizer/, spec.json}。下载 3 个 zip(image/text 编码器 + tokenizer),逐个用流式 SHA256(1MiB 分块)对比 manifest 中的期望 sha256,不符抛 checksumMismatch;用 /usr/bin/ditto -x -k 解压,每个 zip 必须恰好一个顶层条目;.mlpackage 用 CoreML 编译成 .mlmodelc,tokenizer zip 直接搬。进度=按各文件字节数加权的 0…1。manifest 固定:siglip2-base-patch16-256,version 1,embeddingDim 768,imageSize 256,contextLength 64;托管地址 huggingface.co/palmier-io/siglip2-base-coreml。视觉匹配余弦下限 visualMatchCosineFloor=0.05。

**苹果框架使用**:
- SwiftUI [high] — 全部 5 个设置分页与子组件的声明式 UI、@Observable 状态绑定、Toggle/SecureField/TextField/ProgressView 等控件、ScrollView 与磨砂材质 .ultraThinMaterial。
- AppKit [medium] — SettingsWindowController 用 NSWindow + NSHostingController 托管一个独立暗色无标题栏可拖拽窗口(frameAutosaveName、darkAqua、fullSizeContentView);NSWorkspace.shared.open 打开外部 URL(Anthropic 控制台、Stripe 结账)。
- Security (Keychain Services) [medium] — KeychainStore 用 kSecClassGenericPassword + kSecAttrAccessibleAfterFirstUnlock 存取用户自带的 Anthropic API Key。
- UserNotifications [low] — AppNotifications 申请通知权限并在生成完成时投递本地系统通知,点击回调跳转到对应资产。
- Foundation (UserDefaults/ByteCountFormatter/FileManager) [none] — 各类布尔偏好持久化(通知/遥测/MCP/搜索索引启用、禁用模型 ID 列表)、缓存目录字节统计与清理、字节大小本地化展示。
- CryptoKit (SHA256) [none] — 经 EmbeddingStore(缓存键)与 ModelDownloader(下载校验)间接关联,Storage 分页通过它们工作。
- CoreML [high] — 经 ModelDownloader.MLModel.compileModel 与 VisualEmbedder 间接关联(本地 SigLIP 推理),Storage 分页只负责展示大小与删除文件。

**闭源云**:是,且程度较深(但集中在 Account/Models 两块)。Account 分页经 AccountService 通过 ClerkKit(身份认证,Google OAuth)+ ConvexMobile(实时后端)访问闭源云:provision(users:upsertFromAuth)、account:get、billing:listPlans、billing:createCheckoutSession/createTopOffCheckoutSession/createPortalSession、feedback:send,结账最终跳转 Stripe(checkout/billing.stripe.com,有 host 白名单)。Models 分页的 ModelCatalog 也经 Convex 订阅 models:list 拉取可用生成模型清单。Privacy 分页的遥测经 Sentry(第三方崩溃云)。Agent 分页保存的是用户自带的 Anthropic API Key,Key 本身只存本地 Keychain,但其用途是直连 api.anthropic.com 做生成式 AI 聊天(由 AgentService/AnthropicClient 使用,不在本模块发起网络请求)。Storage 分页本身不触云,但其管理的 SigLIP 模型下载自 huggingface.co(公开权重,非生成式云)。General 的通知、Models 的开关、Storage 的清理逻辑均为纯本地。

**移植策略**:整个 Settings 模块的 UI 必须在 React/TypeScript 中重建(SwiftUI/AppKit 无法移植);把它作为 Tauri 的一个独立设置窗口(WebviewWindow)。其中可直接移到 Rust core 的是少量纯逻辑与持久化:(1) 偏好读写——把 UserDefaults 的各布尔键(notifications/telemetry/mcp/searchIndex enabled,缺省全为 true)与 disabledModelIds 集合,统一落到一个 Rust 端 settings.json 或 Tauri Store;模型启用规则保持'默认启用、只记录禁用集'语义。(2) 积分/计费纯数值规则(budget=plan额度+已购、remaining=max(0,budget-spent)、进度比 min(1,left/budget)、配色阈值 0.05/0.25、Top-off 美元×100=积分、合法区间 5..1000、有效价=折扣价优先)——这些是可单测的纯函数,直接在 Rust 复刻并暴露给前端。(3) 字节统计/缓存清理——用 std::fs 递归累加文件大小、删除目录内顶层条目即可一比一复刻 DiskCache;路径根改为跨平台缓存目录(Tauri app_cache_dir)。(4) EmbeddingStore 二进制格式(magic PALMEMB1 + u32-LE headerLen + JSON header + 行:3×f64 + dim×f16,小端无对齐,总长校验,Float16 转 f32)与缓存键(path|mtime|size 的 SHA256 取前 32 hex)——可在 Rust 用 byteorder + half + sha2 精确重写,Storage 分页只读其大小、整体删除目录。(5) ModelDownloader 的下载-SHA256校验-解压-安装流程可在 Rust 重写(reqwest + sha2 流式 + zip 解压),但 .mlpackage→.mlmodelc 的 CoreML 编译与设备端 SigLIP 推理是 Apple 专有,跨平台需改用 ONNX Runtime/candle 加载等价 SigLIP 权重,属另一模块的工作。需要 cloud-rebuild 的部分:账户/登录/订阅/积分购买/反馈——Clerk+Convex+Stripe 的闭源栈要替换为 OpenTake 自有的后端(自建 auth + 计费 + 模型目录接口),前端只改调用端点,UI 状态机(三态:加载/已登录付费/已登录未付费/未登录,以及 lastError 红字)与 Stripe host 白名单等行为照搬。Anthropic Key 的本地存储用 keyring crate(跨平台 Keychain/Credential Manager/libsecret)替代 Security.framework,掩码规则(>4 位时 36 点+末4位,否则 32 点)与变更后发事件通知 Agent 重连的行为照搬。MCP 服务器开关在 Tauri 下指向 Rust 实现的 MCP server(端口/绑 127.0.0.1 行为照搬,端口 19789 可沿用)。通知改用 tauri-plugin-notification;遥测可选 Sentry Rust SDK 或置空,'改后需重启'的提示因为是启动时快照初始化,可照搬该 UX。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/SettingsView.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/AccountPane.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/AgentPane.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/StoragePane.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/ModelsPane.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/PrivacyPane.swift

