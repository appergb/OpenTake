# Telemetry 移植规格

**职责**:
- 读取 Info.plist 中的 SentryDSN 与版本号,在 App 启动时初始化 Sentry SDK(start())
- 管理遥测启用开关:存储在 UserDefaults(key=io.palmier.pro.telemetry.enabled),默认开启;并缓存一份'本次启动时的值'供 UI 检测是否需要重启
- 添加面包屑日志(breadcrumb):带 message/category/level/data 字典
- 捕获消息(captureMessage)与捕获错误对象(captureError)上报到 Sentry
- 分级日志辅助:logWarning(走面包屑)/logError(走 capture message, level=error)/logFault(走 capture message, level=fatal),给每条挂 log_category tag 与 log extra
- 设置 scope 的 extra 自定义上下文键值(setExtra),用于附加当前工程快照等信息
- 包裹同步/异步闭包为 Sentry 性能事务(trace),成功 finish、抛错则以 .internalError 状态 finish 并重抛
- 提供 shortId 工具:取字符串前 8 字符,用于在遥测里脱敏/缩短 UUID 类 id

**核心类型**:
- `Telemetry` (enum) — 无 case 的命名空间式 enum,全部为 static 成员。承载 DSN 读取、isEnabled 开关、start/breadcrumb/captureMessage/captureError/setExtra/logWarning/logError/logFault/trace/shortId 等所有遥测 API。是本模块唯一对外类型。
- `Telemetry.Payload` (other) — typealias = [String: Any],遥测附加数据字典的别名。被 Log.swift 等调用方广泛复用作为参数类型。

**核心算法/逻辑(供 Rust 复刻)**:
- 【启用开关三态默认逻辑】isEnabled.get:读 UserDefaults.standard;若 object(forKey:enabledKey) == nil(即用户从未设置过)返回 true(默认开启遥测);否则返回 bool(forKey:)。set:写入 UserDefaults。Rust 复刻:用持久化偏好(如 JSON/SQLite 设置表)存布尔,键名固定为 'io.palmier.pro.telemetry.enabled';'缺省即 true' 这一三态语义必须保留(键不存在=开启,而非关闭)。
- 【启动门控】enabledForCurrentLaunch = isEnabled 在类型加载时求值一次并缓存为 static let,代表'本次进程启动时刻'的开关值。UI(PrivacyPane)用它与当前 isEnabled 比较来判断是否需要提示重启。didStart 为 nonisolated(unsafe) static var 布尔标志,初值 false。
- 【start() 初始化序列与门控】先 guard enabledForCurrentLaunch(关则直接 return,本次进程不上报);再 guard !dsn.isEmpty(无 DSN 则 return)。然后 SentrySDK.start 配置:sendDefaultPii=false(不发个人信息);environment 按编译条件 DEBUG→'development' 否则 'production';tracesSampleRate=0.1(10% 性能采样);appHangTimeoutInterval=8.0 秒(应用卡顿判定阈值);attachStacktrace=true;enableCaptureFailedRequests=false(不自动上报失败网络请求);enableUncaughtNSExceptionReporting=true;releaseName 由 Info.plist 的 CFBundleShortVersionString 与 CFBundleVersion 拼成 'palmier-pro@<version>+<build>'(两者都存在时才设置)。成功后置 didStart=true。所有其它 API 都以 'guard didStart else return'(trace 例外,见下)为前置,确保未初始化时静默 no-op。
- 【DSN 来源】dsn = Bundle.main Info.plist 的 'SentryDSN' 字符串,缺失则空串。Rust 复刻:从打包配置/环境变量读取 DSN,空则不初始化。
- 【breadcrumb】didStart 才执行;构造 Breadcrumb(level, category),设置 message 与 data,调用 SentrySDK.addBreadcrumb。category 默认 'app',level 默认 .info。
- 【分级日志映射规则(重要)】logWarning → breadcrumb(level=.warning)(注意:warning 只进面包屑,不单独上报为事件);logError → captureLogMessage(level=.error);logFault → captureLogMessage(level=.fatal)。captureLogMessage 内部:guard didStart;SentrySDK.capture(message:){ scope.setLevel(level); scope.setTag(value:category, key:'log_category'); 若 data 非空则 scope.setExtra(value:data, key:'log') }。即:错误/致命会产生独立 Sentry 事件并打 log_category 标签与 log 附加数据,而警告仅作为后续事件的上下文面包屑。
- 【captureMessage / captureError】captureMessage(message, level 默认 .warning):didStart 才执行,SentrySDK.capture(message:){ scope.setLevel(level) }。captureError(error):didStart 才执行,SentrySDK.capture(error:)。
- 【setExtra】didStart 才执行;SentrySDK.configureScope{ scope.setExtra(value:, key:) }。用于把当前工程快照等挂到全局 scope(调用方 EditorViewModel 用 key='project')。
- 【trace 性能事务(同步与异步两个重载)】若 !didStart 则直接执行 work() 并返回(不创建事务,保证零开销直通);否则 txn = SentrySDK.startTransaction(name, operation);do{ result=work(); txn.finish(); return result } catch { txn.finish(status:.internalError); throw }。operation 默认 'task'。同步版用 rethrows + 闭包,异步版用 async rethrows + async 闭包,语义一致。
- 【shortId 脱敏工具】String(id.prefix(8)),取前 8 个字符。调用方对 assetId/资源 id 用它做缩短与轻度脱敏后再放入遥测 payload。Rust 复刻:取字符串前 8 个 Unicode scalar/char(需注意是字符前缀而非字节,避免截断多字节字符)。
- 【调用方耦合点(便于复刻接线)】(1) App/main.swift:Log.bootstrap() 后立即 Telemetry.start()。(2) Utilities/Log.swift 的 CategoryLog 是主要门面:notice(telemetry:) 在提供 telemetry 文案时调 breadcrumb;warning/error/fault 分别调 logWarning/logError/logFault,并把 telemetry 文案兜底为普通消息 m。(3) Settings/PrivacyPane.swift:开关 UI 写 Telemetry.isEnabled,并用 didChange=telemetryEnabled != enabledForCurrentLaunch 提示用户重启生效。(4) Agent/ToolExecutor 与 EditorViewModel 通过 Log 门面写带 Payload 的遥测(工具耗时、timelineChanged 等)。

**苹果框架使用**:
- Foundation [none] — UserDefaults.standard 读写遥测开关;Bundle.main.object(forInfoDictionaryKey:) 读取 SentryDSN、CFBundleShortVersionString、CFBundleVersion;String.prefix 做 shortId。均为标准跨平台基础设施,无音视频或图形相关用途。

**闭源云**:不触达 Convex/Clerk/任何闭源生成式 AI 云。唯一的网络出口是第三方崩溃监控服务 Sentry(自托管或 sentry.io 均可,DSN 从 Info.plist 注入)。上报内容刻意脱敏:sendDefaultPii=false、enableCaptureFailedRequests=false,且产品文案明确声明"媒体与工程内容绝不收集",仅发送崩溃/错误/面包屑与脱敏后的 id(shortId 取前 8 位)。此为可选诊断遥测,与生成式 AI 业务云无关,且受用户开关控制(默认开启,可在隐私设置关闭后重启生效)。

**移植策略**:逻辑本身简单可直接移植,但 Sentry Cocoa SDK 必须替换为 Rust 生态对应物。方案:在 Rust core 用官方 `sentry` crate(sentry-rust,支持 native 崩溃/panic 捕获、breadcrumb、scope、performance transaction,概念一一对应)。映射:SentrySDK.start→sentry::init(ClientOptions{ dsn, environment, traces_sample_rate:0.1, release:Some('palmier-pro@<ver>+<build>'.into()), send_default_pii:false, .. });breadcrumb→sentry::add_breadcrumb(Breadcrumb{ message, category, level, data });captureMessage/Error→sentry::capture_message / capture_error(或 anyhow/Error 经 sentry-anyhow);setExtra→sentry::configure_scope(|s| s.set_extra(...));logWarning/Error/Fault 的'warning 只进面包屑、error/fatal 才成事件并打 log_category tag' 这套分级语义需在 Rust 侧用同样的分支显式复刻;trace→sentry 的 start_transaction + finish(失败时 set status=internal_error)。isEnabled 三态默认(键缺省=开启)用 Tauri 的设置存储或自管 JSON/SQLite 复刻,键名沿用 'io.palmier.pro.telemetry.enabled' 或改 OpenTake 命名空间。appHangTimeoutInterval(8s 卡顿)、enableUncaughtNSExceptionReporting 这类 macOS 特有项无对应,可丢弃或用 sentry panic handler 近似。前端(React)若也要采集可用 @sentry/browser/@sentry/tauri,但核心崩溃上报建议放 Rust 端。隐私/合规:保留默认脱敏(不发 PII、不收媒体/工程内容)、保留用户可关开关、上报前对 id 做 shortId(前 8 char)截断。整体属诊断基础设施,非编辑算法,移植优先级低,可后置。

**关键文件**:Sources/PalmierPro/Telemetry/Telemetry.swift、Sources/PalmierPro/Utilities/Log.swift、Sources/PalmierPro/Settings/PrivacyPane.swift、Sources/PalmierPro/App/main.swift

