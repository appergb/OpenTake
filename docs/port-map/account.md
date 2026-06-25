# Account 移植规格

**职责**:
- 鉴权:通过 Clerk 发起 Google OAuth 登录/登出,并通过 ClerkConvexAuthProvider 把 Clerk 会话桥接给 Convex,监听 AuthState(loading/authenticated/unauthenticated)
- 账户配置 provision:登录后调用 Convex mutation users:upsertFromAuth,把 Clerk 用户的 email/name/image 写入后端(带 3 次重试)
- 账户数据实时同步:订阅 Convex 的 account:get(账户+套餐)与 billing:listPlans(可购套餐),实时推送到 @Observable 状态
- 积分账本计算:budget = 套餐月度积分 + 已购积分;remaining = max(0, budget - 本期已花);并据此派生 hasCredits/remainingCredits
- 计费动作:发起订阅结账(billing:createCheckoutSession)、积分充值(billing:createTopOffCheckoutSession)、管理订阅门户(billing:createPortalSession),并用 NSWorkspace 打开 Stripe URL
- URL 安全校验:只允许打开 https 且 host 属于 checkout.stripe.com / billing.stripe.com 的链接,否则拒绝
- 对外开关:isSignedIn / aiAllowed / isPaid / hasCredits,被全 App 用来 gate AI 生成与 Agent 聊天
- 反馈上报:feedback:send action,携带消息/邮箱/可否联系/截图 base64/App 版本/OS 版本
- UI 呈现:头像(UserAvatar)、身份条(IdentityStrip)、账户气泡卡(AccountPopoverCard)、积分摘要(CreditSummaryView)、充值输入(TopOffField)、设置页账户面板(AccountPane)
- 错误与日志:统一 lastError 字符串供 UI 展示,关键事件经 Log/Telemetry 上报 Sentry

**核心类型**:
- `AccountService` (class) — 核心单例(@Observable @MainActor,static shared)。持有 Convex 客户端、所有订阅与鉴权任务,集中管理登录态/账户数据/积分/计费动作,是整个模块对外的唯一状态与行为入口。
- `AccountTier` (enum) — 套餐档位 none/pro/max(String 可解码)。提供 isPaid、planLabel('Free'/'Pro plan'/'Max plan')、upgradeLabel('' /'Pro'/'Max')。
- `AccountUser` (struct) — 后端返回的用户主体:email/name/image/tier/currentPeriodEnd(毫秒时间戳)/cancelAtPeriodEnd/spentCreditsThisPeriod/purchasedCredits。派生 displayName(trim 后非空)、firstName(按空格取首段)。
- `AccountPlan` (struct) — 当前账户所处套餐:tier/monthlyPriceUsd/monthlyBudgetCredits(可空)。用于积分预算计算。
- `AvailablePlan` (struct) — 可购买套餐(Identifiable,id=tier.rawValue):tier/monthlyPriceUsd/discountedMonthlyPriceUsd/monthlyBudgetCredits。派生 hasDiscount(折扣价 < 原价)与 effectiveMonthlyPriceUsd(有折扣取折扣价)。
- `AccountResponse` (struct) — account:get 订阅的载荷:{user: AccountUser, plan: AccountPlan?}。
- `TopOffLimits` (enum) — 充值金额边界常量:minDollars=5,maxDollars=1000(纯命名空间常量)。
- `AuthState<String>` (enum) — 来自 ConvexMobile 的泛型鉴权状态枚举,三态 .loading/.authenticated/.unauthenticated;泛型参数(此处 String)为身份/令牌类型。AccountService 据此驱动整个登录流程。
- `AccountPopoverCard` (struct) — SwiftUI 视图。点头像弹出的紧凑账户卡:身份块+套餐块(积分进度条/升级按钮)+底部(设置/反馈/登录登出)。
- `CreditSummaryView` (struct) — SwiftUI 视图,两种样式:.full(设置页大进度条)与 .compact(生成面板胶囊小芯片,点开 CreditActionsPopover 充值/升级)。
- `TopOffField` (struct) — SwiftUI 泛型视图(带 Trailing slot)。美元输入框,实时换算 credits=美元*100,做 5–1000 校验,触发 buyCredits。
- `UserAvatar / IdentityStrip / UserAvatarButton` (struct) — SwiftUI 头像与身份条组件:登录显示首字母圆/远程头像,未登录显示占位符号;Strip 显示主/次文本(名/邮箱)。
- `BackendConfig` (enum) — 从 Info.plist 读取后端配置:PalmierClerkPublishableKey / PalmierConvexDeploymentURL / PalmierConvexHttpURL;isConfigured 判断是否齐全。

**核心算法/逻辑(供 Rust 复刻)**:
- [积分预算核心算法] budgetCredits:若无 user 返回 nil;否则 tierBudget = account.plan.monthlyBudgetCredits ?? 0,再加 user.purchasedCredits ?? 0(套餐月额度 + 已购充值额度)。spentCredits = user.spentCreditsThisPeriod ?? 0。remainingCredits = max(0, (budgetCredits ?? 0) - spentCredits)。hasCredits = remainingCredits > 0。注意:只有当 budgetCredits 非 nil(即已有 user)时,UI 才显示积分块;Rust 复刻需保留‘nil 表示未知/不展示’与‘0 表示已耗尽’的区分。
- [美元→积分换算] credits = max(0, dollars) * 100(每 1 美元 = 100 积分,整数运算)。充值合法区间 [5,1000] 美元(闭区间,含端点)。TopOffField 中 isValid = (5...1000).contains(dollars);按钮文案合法时为 'Buy $<n>' 否则 'Buy';换算文案 1 时显示 '= 1 credit' 否则 '= <n> credits'。
- [充值动作 buyCredits] 入参 dollars:Int。先校验 (5...1000).contains(dollars),越界则设 lastError='Amount must be $5–$1000.' 并直接返回。若已有 isBuyingCredits 在跑则忽略(去重/防重复点击)。置 isBuyingCredits=true,起一个 @MainActor Task,defer 里复位 isBuyingCredits=false 并清空 buyCreditsTask。调用 Convex action 'billing:createTopOffCheckoutSession',参数 {dollars: Double(dollars)}(注意后端要 Double),拿到 {url} 后走 openInBrowser。异常写 lastError。
- [订阅动作 subscribe] 入参 tier。先清 lastError;若 tier 非付费(none)或无 convex 直接返回。调用 action 'billing:createCheckoutSession' 参数 {tier: tier.rawValue},得 {url} 后 openInBrowser。异常写 lastError。
- [管理订阅 manageSubscription] 调用 action 'billing:createPortalSession'(无参),得 {url} 后 openInBrowser。
- [URL 安全闸门 openInBrowser] 解析 URL,必须满足:scheme=='https' 且 host 非空 且 host ∈ {checkout.stripe.com, billing.stripe.com}(白名单 Set)。任一不满足则 lastError='Refused to open untrusted URL.' 且不打开。通过则 NSWorkspace.shared.open。Rust 端复刻需保留同样的协议+host 白名单校验后再交给系统浏览器打开。
- [鉴权观察主循环 startAuthObservation] 先自旋等待 Clerk 恢复缓存会话:最多 50 次、每次 sleep 100ms(即最长 5 秒),条件 while !Clerk.shared.isLoaded。随后 for await 监听 convex.authState.values:loading→isLoading=true;authenticated→先 await provisionAndSubscribe() 再 isLoading=false;unauthenticated→clearAccount() 且 isLoading = (Clerk.shared.session != nil)(即仍有本地会话时保持 loading,等待 Convex token 就绪,避免登录瞬间闪现登出 UI)。这是一个易错的边界条件,需精确复刻。
- [provision 重试策略 provisionAndSubscribe] 组装 name = [firstName,lastName] 去 nil 后空格拼接,空串则传 nil;args = {email, name, image}。循环 attempt 0..<3 调用 mutation 'users:upsertFromAuth':成功 break;失败写 lastError,若 attempt==2(最后一次)记录失败并 return(不再订阅);否则 sleep 500ms 重试。成功后调用 startAccountSubscription()。即:最多 3 次、每次失败间隔 0.5 秒。
- [实时订阅模型] 用 Combine。startPlansSubscription:订阅 'billing:listPlans' yielding [AvailablePlan],receive(on: main),sink 把值写 availablePlans,失败写 lastError。startAccountSubscription:订阅 'account:get' yielding AccountResponse,收到值写 account 并清空 lastError,失败写 lastError。两条订阅互相独立;plans 订阅在 configure 阶段(尚未登录)就启动,account 订阅在 provision 成功后才启动。
- [登录态派生] isSignedIn = (!isMisconfigured && authState==.authenticated)。aiAllowed = isSignedIn && !isMisconfigured。tier = account?.user.tier ?? .none。isPaid = tier.isPaid。这些是被全 App gate AI 功能的真值来源(Agent.canStream 还要求 hasCredits;ToolExecutor 生成前 guard isSignedIn 再 guard hasCredits)。
- [clearAccount 清理] 取消并置空 accountSubscription、取消并置空 buyCreditsTask,account=nil,isBuyingCredits=false(注意不动 plansSubscription,登出后仍可看套餐)。
- [configure 幂等] 用 didConfigure 布尔保证只配置一次。若 BackendConfig 缺 clerkPublishableKey 或 convexDeploymentURL,则 isMisconfigured=true、isLoading=false 并告警返回(此后全 App 把 AI 功能直接当作不可用而非‘需登录’)。配置成功则 Clerk.configure(redirectUrl='palmier://callback', scheme='palmier'),创建 ConvexClientWithAuth(deploymentUrl, ClerkConvexAuthProvider()),启动 plans 订阅与鉴权观察。
- [周期结束时间换算] currentPeriodEnd 是毫秒级 Unix 时间戳(Double)。展示用 Date(timeIntervalSince1970: endMs/1000) 再 abbreviated 日期。cancelAtPeriodEnd==true 时显示 'Cancels <date>' 否则积分块显示 'Resets <date>'。
- [积分进度条颜色阈值] remaining = budget>0 ? min(1.0, left/budget) : 0(left=max(0,budget-spent))。颜色:<0.05 红;<0.25 橙;否则品牌主色。CreditSummaryView 与 AccountPopoverCard 用同一套阈值。
- [套餐积分简写] creditsShortLabel:若 credits>=1000 且能被 1000 整除,显示 '<n/1000>k credits';否则 '<千分位格式> credits'。
- [反馈上报 sendFeedback] 无 convex 抛 NSError(domain 'Palmier.Feedback', code -1)。args 必含 message/mayContact/appVersion/osVersion;email 与 screenshotPngBase64 可选(非 nil 才加入)。调用 action 'feedback:send' 期望 {ok}。截图以 PNG 的 base64 字符串上传。

**苹果框架使用**:
- AppKit [low] — NSWorkspace.shared.open 打开 Stripe 结账/门户链接;构造 NSError 作为反馈失败错误
- SwiftUI [medium] — 全部账户相关界面(头像、身份条、账户气泡卡、积分摘要、充值输入、设置账户面板),使用 @Observable/@Bindable 双向绑定、AsyncImage 加载远程头像、ProgressView 画积分条、popover
- Observation [low] — @Observable + @ObservationIgnored 标注 AccountService,驱动 UI 自动刷新
- Combine [low] — 对 Convex 的 account:get / billing:listPlans 订阅用 AnyCancellable.sink 接收并切回主线程
- Foundation [none] — URL/URLSession(参考文件上传见 GenerationBackend)、Bundle 读 Info.plist 配置、Date 周期时间换算、JSONDecoder 解码、Task/async 并发
- os.Logger [none] — 经 Log/CategoryLog('account') 输出分类日志并镜像到 stderr

**闭源云**:是,且本模块的存在本质即为访问闭源云:1) Clerk(闭源鉴权 SaaS)做 Google OAuth 登录与会话;2) Convex(闭源 BaaS,convex-swift + ConvexClientWithAuth)做实时数据库订阅(account:get、billing:listPlans、generations:*)与 serverless mutation/action(users:upsertFromAuth、billing:createCheckoutSession/createTopOffCheckoutSession/createPortalSession、feedback:send、uploads:*);3) Stripe(经 Convex 返回 checkout.stripe.com / billing.stripe.com 链接)做支付;4) Sentry 做遥测上报。注意:本模块本身不直接调用生成式 AI,但它是 AI 云能力的‘鉴权+计费闸门’——下游 GenerationBackend/PalmierClient 通过 AccountService.convex 与积分开关访问后端的图像/视频/音频/Claude 聊天生成。closedCloudTouch = 全链路闭源云依赖。

**移植策略**:这是闭源云客户端层,不能直接 port,需在 OpenTake 里整体重建为自有后端方案。具体替换:1) 鉴权:用开源方案替代 Clerk——Tauri 端用系统浏览器走 OAuth2 PKCE / 设备码,Rust core 用 oauth2 crate + 自定义 deep-link(opentake://callback,对应原 palmier://callback)接回 token,令牌存 OS keychain(keyring crate);或自托管 authentik/Keycloak/Supabase Auth。2) 后端实时数据与函数:用 Convex 的开源自托管版,或换 Supabase(Postgres + Realtime + Edge Functions)/ 自建 axum 服务 + WebSocket/SSE 订阅,复刻 account:get、billing:listPlans 的‘服务端推送即时刷新’语义(前端用 TanStack Query + WS 失效)。3) 计费:Stripe 仍可用——Rust 后端用 stripe-rust 创建 checkout/portal session,Tauri 用 tauri-plugin-shell/opener 打开返回 URL;务必复刻 openInBrowser 的‘https + host 白名单(checkout/billing.stripe.com)’安全闸门(在 Rust 侧用 url crate 校验 scheme/host 后再 open)。4) 积分账本算法是纯整数运算,可在 Rust 端 1:1 直译(budget = 套餐额度 + 已购;remaining = max(0, budget-spent);dollars*100=credits;5..=1000 充值区间;阈值 0.05/0.25 上色)——这部分属 direct-port。5) 状态机(loading/authenticated/unauthenticated、登出时 isLoading 取决于是否仍有本地会话、provision 3 次×0.5s 重试、Clerk 恢复会话最多 5s 自旋)建议在 Rust 用 async + tokio 复刻,前端只读状态。6) UI(头像/身份条/积分条/充值框/设置账户面板)全部 ui-rebuild 为 React/TS 组件,沿用 AppTheme 令牌映射成 CSS 变量;AsyncImage→<img> 懒加载,ProgressView→进度条组件。7) 遥测:Sentry 有官方 Rust/JS SDK,可平移。整体工作量集中在‘自有后端 + 鉴权 + 计费’的重建,而非编辑逻辑。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/AccountService.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/BackendConfig.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/AccountPopoverCard.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/CreditSummaryView.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/TopOffField.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/IdentityViews.swift

