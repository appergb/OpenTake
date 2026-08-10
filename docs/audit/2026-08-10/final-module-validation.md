# OpenTake 1.0.0-beta.4 最终模块验收

日期：2026-08-10（Asia/Shanghai）

对应发布范围见 [Beta 4 发布说明](../../releases/1.0.0-beta.4.md)。

## 范围与隔离

- 验收工作树：`OpenTake-generation`；当时的功能冻结快照后续由
  `release/v1.0.0-beta.4` 承载发布元数据。
- GUI 仅使用隔离包 `OpenTake QA.app`（bundle id `com.opentake.desktop.qa`）和 `/tmp` 工程副本；没有替换、退出或修改 `/Applications/OpenTake.app`。
- QA 工程：`/tmp/opentake-gui-qa.wTAGrR/final-interaction.opentake`、`playback-av.opentake`、`export-qa.opentake`。
- 未执行付费 AI 请求、登录/登出或 Keychain 写入；AI/Account 的真实凭据页面不做 Computer Use，避免访问生产用户共享的 Keychain service。
- GUI 验收阶段未 commit、push、创建/移动 tag 或发布 Release；发布准备阶段仅按
  workflow 约定配置了两个 updater 签名 secret 名称，未记录或输出私钥与口令。

## 自动化门禁

冻结快照的实际结果：

- Web：142 个文件、1210 个测试全部通过；`pnpm build` 通过。
- Rust workspace：`cargo test --workspace --no-fail-fast --locked` exit 0；Tauri lib 607/607、播放 integration 7/7、transport integration 6/6 均通过。
- `cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 全部通过。
- Windows locked dependency graph 的最高 `rust-version` 为 `kstring 2.0.4` 声明的 1.96；公开
  README 与 CONTRIBUTING 的构建前置因此统一为 Rust 1.96，未继续声称 1.82/1.88 兼容。
- Updater：35/35；安装 admission、RID 恢复、慢请求 deadline、并发 dismiss/check、系统代理、REST rate-limit Atom 回退、签名/attestation/下载上限均有回归。
- Release 合同：release 57、attestation 5、manifest 12、Windows validator 85 全部通过；C1B、strict YAML、actionlint 与 exact 17-asset/6-signature 合同通过。
- 9 个 ignored probe 均是显式真机/外部素材门：2 个媒体、3 个导出、4 个播放 probe；没有把 ignored 当作通过。

## Computer Use 真机结果

### 播放与画面

- 在 splitter、Timeline、Agent 面板等非文本区域按 Space 均能播放/暂停；搜索输入框中的 Space 只输入空格，播放头保持不变。
- 冷启动播放 1.1 秒推进至 33 帧；暂停落在 36 帧，450 ms 后仍为 36。
- 同一原生会话快速播放/暂停三轮分别停在 41、47、52 帧；每轮 400 ms 后完全不动，没有延迟暂停或回抽。
- 最终重建包的额外快检从 0 帧启动后推进到 16 帧；第二次 Space 后立即停在 16，500 ms 后仍为 16。
- 跳到结尾 1040 帧后按 Space，从 0 重新播放并停在第 9 帧；400 ms 后仍为 9。
- 预览画面实际可见、内容正确，未观察到突然黑屏、绿色底边或解码异常。

### 转场与持久化

- 真实 AX 点击第二片段 → 转场页 → 交叉溶解时，cut pair 保持，卡片变为 `Value: on`，不再因 PanelShell focus 清空选择。
- 从第 90 帧播放跨过第 100 帧切点，在 115 帧暂停，500 ms 后仍为 115；画面正确。
- 保存、完全退出 QA、重启并重开后，UI 仍显示交叉溶解和移除按钮；`project.json` 的 `transitionOut.kind` 为 `crossDissolve`。

### Home、素材库、设置、布局与 Agent

- Home recent 能保存并重开 4 个隔离工程。
- 素材库的全部/视频等分类、搜索、最近收藏/最早收藏/按类型排序均可操作并能恢复初值。
- 设置的通用、外观、导入、MCP 说明、快捷键、存储、关于逐页打开；快捷键页显示“播放/暂停 — Space”与 `GPL-3.0`。
  该隔离 QA 包在 Beta 4 元数据迁移前构建，关于页当时显示 `1.0.0-beta.3`；Beta 4
  的 `1.0.0-beta.4` 身份由 Cargo/Tauri/Web/WiX 合同与 exact-tag 发布构建验证，不借用旧包证明。
- 默认/竖屏布局可切换，Media/Preview/Inspector/Timeline 均保留；最终恢复默认布局。
- Agent 面板可开关，对话/动效入口存在，空输入时发送按钮禁用；Agent 面板背景聚焦下 Space 播放并立即暂停。

### 导出

- H.264 导出类型显示 `视频 (.mp4)`；切换 ProRes 后显示 `视频 (.mov)`。
- 空时间线导出命令边界、字幕/交换格式和真实编码路径由 Web/Rust integration 与既有导出 probe 覆盖；本轮没有重复生成大文件。

### 自动更新

- App 启动后的后台检查保持静默；原生 App 菜单“检查更新…”和设置 → 关于“检查更新”均启用。
- 手动检查的错误态可恢复，固定“GitHub Releases 手动下载”按钮实际通过系统浏览器打开 `https://github.com/appergb/OpenTake/releases`；没有任意 URL 参数。
- 当前共享出口的匿名 GitHub REST API 已触发 rate limit，macOS 同时通过 `127.0.0.1:1082` 系统代理联网。最终候选在这两个真实条件下已由打包 App 返回“OpenTake 已是最新版本”；没有把网络错误当作成功。
- 验收时 QA 包位于 `target/aarch64-apple-darwin/release/bundle/macos/OpenTake QA.app`，实际复验的
  可执行文件 SHA-256 为 `34c1de53e29e04d8756ed1564d334c694fcc8adc54065b4341562e589d40b759`；
  后续为释放构建空间已清理该可重建的 `target/` 产物。
- 仓库已配置两个必需的签名 secret，但仍没有较新的、由同一 OpenTake updater key 签名的
  N+1 Release，因此本轮不能执行真实 N→N+1 安装；不据此宣称安装 E2E 已通过。

## 已知外部边界

- macOS QA 包为 ad-hoc 签名，`codesign --verify --deep --strict` 通过，但没有 Developer ID 公证。
- Windows WebView2、MSI/NSIS 原地升级和原生音频设备需由 exact-SHA Windows CI/实机最终验证；macOS 结果不能替代。
- Windows `ort-tract` 链已从 `tract-* 0.21.10` 升到 `0.22.3`，`cargo audit --no-fetch --stale --target-os windows --target-arch x86_64` exit 0，RUSTSEC-2026-0217 已消失。macOS 主机上的 Windows 交叉编译仍被宿主缺少 MSVC C/C++ 头文件与 `lib.exe` 挡在第三方 native build，需 exact-SHA Windows CI 完成最终平台编译门禁。
