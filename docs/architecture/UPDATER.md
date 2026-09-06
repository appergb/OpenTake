# OpenTake 自动更新架构

> 状态：draft · 阶段：partial-implementation · 路由同步：2026-09-06。本文包含设计目标与已有实现；须按源码和日期化证据逐项判断。
> 当前执行与验收：[公开 Beta 计划](../plans/active/2026-09-06-public-beta.md) · [当日验证](../audit/2026-09-06/public-beta-validation.md)。


OpenTake 使用 Tauri v2 官方 updater 完成「检查 → 下载 → 签名验证 → 安装 → 安全重启」。更新源只允许独立仓库 `appergb/OpenTake`，不复用 OpenLess 的 URL 或签名密钥。

## 发布与发现契约

- Rust 首先请求 `https://api.github.com/repos/appergb/OpenTake/releases?per_page=30`，不使用 `releases/latest`，因此 GitHub prerelease Beta 也能被发现。匿名 REST 因共享出口额度返回 403/429 时，仅回退到 GitHub 官方固定 feed `https://github.com/appergb/OpenTake/releases.atom`；不抓取 release HTML。
- Atom 响应与 REST 一样限制为 1 MiB，最多解析 100 个 entry；`feed` 必须是文档唯一根节点，根外 entry/结构一律拒绝。Feed id/self link 必须精确对应 `appergb/OpenTake`，entry 只接受无凭据、无端口/查询/片段的 `https://github.com/appergb/OpenTake/releases/tag/v<SemVer>`；Atom HTML notes 不进入 UI。
- release tag 必须是规范的 `v<SemVer>`。草稿、GitHub prerelease 标记与 SemVer 不一致的 release、同版本、降级和 stable → prerelease 均被忽略。
- 每个 tag 必须包含专属清单 `updater-${tag}.json`，例如 `updater-v1.0.0-beta.4.json`。
- 清单和安装包首跳 URL 固定在 `https://github.com/appergb/OpenTake/releases/download/${tag}/...`。重定向仅允许 HTTPS 的 GitHub release CDN 主机。
- 平台键由发布流程生成：`darwin-aarch64`、`windows-x86_64-msi`、`windows-x86_64-nsis`；不提供会跨安装器回退的通用 Windows 键。每个平台/安装器都发布独立的包签名与签名 attestation。

普通开发/QA 构建的 `bundle.createUpdaterArtifacts` 固定为 `false`，不需要签名私钥。发布 workflow 仅在已注入 `TAURI_SIGNING_PRIVATE_KEY` 与非空 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 时，通过 Tauri CLI `--config` overlay 临时开启 updater artifact 生成。

## 运行时与资源生命周期

`src-tauri/src/updater.rs` 为唯一网络与安装边界：

1. 以 15 秒超时、禁止重定向、1 MiB 响应上限读取固定 GitHub API；仅在 REST 403/429 时读取上述固定 Atom feed。
2. 在本地完成 SemVer/asset allowlist 选择；没有更高版本时不构造、更不会请求 manifest。
3. 对选定 tag 创建单 endpoint updater，以 20 秒超时检查签名清单。
4. 将 Tauri `Update` 保存为 Webview Resource/RID；协调器只允许一个 check/pending/install。
5. 用户取消时关闭准确 RID；安装开始后取走 RID，任何失败都不能复用旧资源。
6. Rust 以有界流下载包，严格比对已签名 attestation 中的大小与 SHA-256，并用嵌入公钥复验包的 Minisign；验证后只把已验证字节交给插件安装。前端只接收进度事件，不接触安装包路径。

Debug Rust 构建直接返回“无更新”，不联网。前端只在 Tauri 环境启动调度：启动后延迟 4 秒静默检查，之后每 60 分钟检查；卸载时释放 timeout/interval。后台无更新或网络错误不弹窗，手动检查会显示明确结果。

## 代理语义

- 发现用的 `reqwest 0.12` 与 manifest/attestation/package 用的 `reqwest 0.13` 均显式开启 `system-proxy`，不依赖跨主版本 feature 偶然合并。
- HTTPS 的优先级为 `HTTPS_PROXY` 大写 → 小写 → macOS/Windows 系统 HTTPS 设置 → `ALL_PROXY` 大写 → 小写。`NO_PROXY` 大写优先小写，保留 reqwest 的域名、IP/CIDR 与 `*` 语义。
- 环境或 updater 显式代理 URL 只允许 `http://` / `https://` 且必须有 host；拒绝 userinfo 凭据、路径、query 与 fragment。校验失败只返回固定错误，不回显原始 URL 或凭据。
- 所有远程更新 client 使用明确标识 `OpenTake-Updater` 的固定 User-Agent；该 header 同时出现在 HTTPS CONNECT 握手中，不携带账号、token 或设备信息。发现请求只对连接层错误按短间隔重试，并受 20 秒总时限约束；HTTP 状态、响应类型与内容校验失败不重试。
- macOS 通过 SystemConfiguration/SCDynamicStore 读取手动 HTTP/HTTPS 代理，不启动 `scutil` 子进程；Windows 读取系统 Internet Settings；Linux 使用上述环境变量。PAC、SOCKS、macOS ExceptionsList 与系统代理凭据不会被自动导入。
- 本地 tokenized manifest verifier 始终显式 `.no_proxy()`，所以 `127.0.0.1` 校验不会被环境或系统代理拦截。所有远程 URL allowlist、HTTPS 和 redirect 上限保持不变。

## 安装前 fail-closed gate

安装前必须原子获得全局 `InstallAdmissionGate` 的 install lease，并取得 export 独占 lease；两者一直持有到安装及返回平台的后置保存结束，以避免 TOCTOU。所有会修改项目、manifest、全局持久化状态或在异步完成后提交结果的命令，都必须在真实工作生命周期内持有同一 gate 的 activity lease；这包括生成、动效、Agent turn、字幕/媒体分析、模型与样例物化、项目/媒体/资源库/账号/密钥写入及其后台 worker。任一 activity 或 export 尚未结束时，本次安装 fail-closed，要求任务停止后重试；安装已取得 lease 后，新写入和新长任务会在 Rust 命令边界被拒绝。播放会在项目切换边界停止。随后按最新 current project epoch/path 保存工程；保存失败不调用安装器。

下载、安装、重启状态下，前端 capture keydown 会阻断编辑器全局快捷键；原生菜单同时禁用自定义 action/check 并在命令边界再次拒绝 stale accelerator。Tauri 的 predefined Quit 没有 `setEnabled` API，因此 Rust 在 updater coordinator 的 Installing 生命周期内拒绝用户 `ExitRequested`，但放行 updater 自己的 programmatic restart。

Windows 官方 updater 在 `Update::install` 内启动安装器后直接退出进程，因此跨平台必需的保存屏障紧邻 install 之前。macOS/Linux 的 install 返回后会再读取一次 fresh epoch/path 并保存，后置保存失败则明确返回错误、不发送 restarting 事件且不重启。这里不假设 Windows 能执行安装后的代码。

## UI 入口

- App 启动后台检查（无更新/错误均静默）。
- 原生 App 菜单“检查更新…”执行手动检查。
- 设置 → 关于提供手动检查按钮。
- 可用、下载、安装、重启和错误状态集中在 `updateStore`；发现更新后必须由用户点击“安装并重启”。
- 错误页固定提供 [GitHub Releases](https://github.com/appergb/OpenTake/releases) 手动下载兜底。

## 签名材料

- 嵌入的 OpenTake 专属公钥：`dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEU5QjFEQjYyQjk1OUVCClJXVHJXYmxpMjdIcEFLNml0MEU3cUJ2WXNjKzdHZ1luSFVyczZPSVArWGtIZUFjbzVYMzYrNUh1Cg==`
- 本机公钥：`~/.tauri/opentake-updater.key.pub`（0600）
- 本机加密私钥：`~/.tauri/opentake-updater.key`（0600，绝不提交）
- 私钥口令仅保存在 macOS Keychain service `OpenTake Updater Signing Key`；文档、代码、日志均不保存或输出口令。

更换密钥必须同时更新本机/CI 签名材料与 `src-tauri/tauri.conf.json` 公钥；不得以 placeholder、空公钥或关闭签名验证过渡。
