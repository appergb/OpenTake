# OpenTake Beta 功能验证记录（2026-08-02）

状态：进行中。只有在代码门禁、原生界面逐项验证、布局/动效复测及候选包重装全部完成后，才会改为发布通过。

## 2026-08-03 进度快照

- **Web 门禁全绿**：129 个测试文件 / 1072 项测试全部通过（Vitest），`pnpm build` 通过（仅既有 large chunk / dynamic import 警告），`git diff --check` 通过。
- **Rust 门禁验证（08-03 已修复 3 项，workspace 全量确认全绿）**：
  - 前置：`cargo test --workspace` 需先 `scripts/provision_ffmpeg_sidecars.py`
    provision FFmpeg sidecar（`src-tauri/binaries/` 被 gitignore），否则 Tauri build 脚本报
    `binaries/ffmpeg-* doesn't exist`。
  - 修复 1：`project_root::first_save_refuses_a_target_that_appears_after_staging` ——
    publish 对 staging 后出现的目标 fail-closed 已生效，但错误消息未区分"存在性变化"；现
    目标存在性变化报 `existence changed`（`crates/opentake-project/src/project_root.rs`）。
  - 修复 2：`project_root::publish_never_deletes_a_foreign_bundle_rebound_at_backup_name`
    —— **安全缺陷**：publish 清理 backup 前未复验其 identity，外来对象反弹到 backup 名会被
    误删。现 backup 清理前复验 identity == 原 target，不匹配则 fail-closed 保留外来对象。
  - 修复 3：`mcp::inspect_project_media_reads_the_retained_bundle_after_path_rebind` ——
    `inspect_source_media` 图片分支原按路径读取，bundle 路径反弹会读到反弹 bundle（新增
    测试断言溢出暴露）。现新增 `AppCore::open_project_asset`（retained no-follow 打开）+
    `image_thumbnail_reader`，Project 源图片经 retained 权威读取，反弹路径不再重定向。
  - **最终结论**：`cargo test --workspace` 全量运行（含 ffmpeg/GPU/chromium 集成）全部通过，
    无 FAILED、无编译错误；opentake-project 234、opentake-media 419、opentake-tauri 492、
    opentake-ops 208 等目标全绿。此前观察的 2 次偶发失败
    （`recovery_refuses_an_unmarked_target...`、opentake-media 某用例）在独立/串行/全量复跑
    中均稳定通过，判定为 workspace 并发偶发 flaky，收尾时持续关注。
- **代码核查（08-03）确认以下曾列 finding 的实现已落地**，待独立 review 与候选包重装后关闭：
  - MED 删除锁：`web/src/store/mediaDeleteActions.ts`（模块级删除锁 + 工程身份键，防跨工程污染）。
  - SET-04 原子身份租约：`crates/opentake-core/src/core.rs` 的 `OwnedUndoResult` /
    `ProjectAssetAuthority` / `PreparedProjectOpen::is_current_namespace`，`opentake-agent`
    `chat::ChatTurnGate` + mcp server 逐轮工程绑定会话。
  - MCP 返回脱敏：`crates/opentake-agent/tests/get_media_redaction.rs`（白名单 DTO，路径/签名
    URL/request id/prompt/hash 不外泄）。
  - 生命周期/交互补测：`web/src/App.lifecycle.test.tsx`、`CropOverlay.interaction` /
    `TransformOverlay.interaction` / `SettingsView.interaction`、`api.pathExists` /
    `api.searchIndex` / `asset` / `editActions.analysisIdentity` 等新增测试。
  - 安全加固：`src-tauri/src/safe_asset_protocol.rs`、`fs_availability.rs`、
    `crates/opentake-project/src/path_policy.rs`、`crates/opentake-media/src/process_tree.rs`。
- **尚未完成（收尾清单）**：① Rust 全量测试终审结论；② 各未决 finding 的独立 review 清零；
  ③ 候选包重装 + 真机 GUI 逐项验收（HOME/MED/TL/PV/IN/CAP/SMART/AG/MOT/EXP/SET 各批次）；
  ④ Windows exact-SHA CI 与实机验证；⑤ 独立 prerelease 标签发布与资产上传（需仓库所有者授权）。

## 验证对象

- 源码分支：`agent/advanced-ai-workflows`
- 验证起点：`c73c192f6fdc01e35a1711089b6e59edc3312309`
- 本机应用：`/Applications/OpenTake.app`
- 版本：`1.0.0-beta.1`（arm64）
- 应用主二进制 SHA-256：`03fdaaf023f9d9913a86fc43df7875e10b3a60c80a71fed8d00842135568e0da`
- 发布 DMG SHA-256：`32d97d35db7872253866b2427bd7da9816f365ce7bf9cb490d2b11a31c46cdfd`

安装校验已完成：DMG、GitHub digest 与公开 `SHA256SUMS` 一致；`hdiutil verify` 通过；应用 `codesign --verify --deep --strict` 通过；FFmpeg/ffprobe sidecar 实际完成 1 秒 H.264 生成与探测。当前包为 ad-hoc hardened runtime，不是 Developer ID 签名/公证包，因此不能把本机 `spctl` 结果当作公证证明。

## 代码基线

初始候选的代码门禁：

- Web：119 个测试文件、882 项测试通过。
- Web 生产构建：通过；保留既有 large chunk / dynamic import 警告。
- Rust：`cargo test --workspace` 通过；硬件 GPU/音频探针为测试声明中的显式 ignore。

生命周期第一轮修复后：

- 聚焦：5 个测试文件、61 项测试通过。
- Web：120 个测试文件、893 项测试通过。
- `tsc -b && vite build`：通过。
- 独立 review 尚确认 Save As 路径采用、启动重试和事件窗口仍有待修复项，因此 893/893 不是最终发布结论。

HOME / Save As 最终修复后：

- Web：121 个测试文件、903 项测试通过。
- `pnpm build`：通过；仅保留既有 large chunk / dynamic import 警告。
- `git diff --check`：通过。
- 独立 code reviewer：APPROVE，CRITICAL/HIGH/MEDIUM/LOW 均为 0。

布局第一轮及媒体/生命周期第二轮合并点（尚非发布冻结点）：

- Web：123 个测试文件、941 项测试通过。
- `pnpm build`：通过；仅保留既有 dynamic import / large chunk 警告。
- `git diff --check`：通过。
- 后续独立终审又发现媒体跨工程删除、搜索事件身份及 timeline refresh owner
  协调缺口；这些 finding 正在补红测和整改。因此 941/941 只能证明该合并点，不能作为
  当前工作树或候选包的最终通过证据。

## 功能矩阵

当前生产入口已拆成约 100 个原子功能，按以下批次执行：

1. `PRJ` / `SHELL`：工程、Home、窗口和全局快捷键。
2. `MED`：导入、文件夹、预览、搜索、收藏、全局素材库、代理与生成占位。
3. `TL` / `PV`：时间线手势、预览、画布与原子撤销。
4. `IN`：Inspector、关键帧、调色、Mask、AI 本地工作流和音频。
5. `CAP` / `SMART`：字幕、翻译与 SmartPack。
6. `AG` / `MOT`：Agent、MCP 工具与 Motion Canvas。
7. `EXP`：视频、字幕与交换格式导出。
8. `SET`：语言、外观、导入目录、Codex/ChatGPT 官方登录、BYOK、MCP、Account 与 About。

每个 mutation 用例附加四项 oracle：工程 epoch/path 防迟到写；一次用户提交对应一次 Rust `EditCommand`；取消/重试 identity 防旧结果提交；原生错误可见且状态不变。

## Agent 执行记录

### HOME-01 — Home 与工程生命周期

Agent：`/root/func_home_lifecycle`。

已通过：安装/版本、Home 主入口、三个样例、新建并立即落盘、自动保存与 `⌘S`、最近工程、Enter/双击、原生打开选择器、完全退出/重启恢复、关窗驻留/Dock 恢复、缺失路径、Finder/移除最近/废纸篓取消、版本更新弹窗、键盘 Tab 顺序和 `⌘N`/`⌘O`。

原始 QA 标记：`HOME01-SAVE-MARKER`；工程 `project.json` SHA-256 为 `f3530509abb81bec0fc273dae63c271c124b46f63d69fd8828148d5def1d49be`。

发现：

- `HOME-BLOCK-01`（发布阻断）：默认 `未命名.opentake` 已存在时，新建面板进入旧工程包内部，可能生成嵌套工程。
- `H-17`（P3）：中文首页项目数硬编码为 `{count} recent`。

修复与复验：默认工程名现在按 `未命名.opentake`、`未命名 2.opentake` 递增避让，
兼容 Windows/Unix 分隔符并在探测失败时安全回退父目录；最近项目数量已完整中英文国际化。
Save As 会合并重复手势，同工程并发编辑后采用原生已提交路径但保留脏状态，跨工程迟到结果
不会接管当前工程。上述实现已通过 903/903 全量测试和独立零 finding 审查，关闭
`HOME-BLOCK-01` 与 `H-17`。

证据目录：`/private/tmp/opentake-home01-qa-20260802.8ADdfg/evidence`。

### 生命周期并发与监听

Agent：`/root/feature_matrix`；独立 code review 同 Agent 的后续只读轮次。

首轮修复：跨工程 Save As 迟到前端写入、timeline/media sync 首次失败后不可重试、迟到 listener 泄漏、Library 首次失败后不可重试。

Review 仍确认：

- P1：同工程编辑期间 Save As 原生成功后，exact snapshot guard 会跳过 committed path，造成 native/store 路径分裂。
- P1：App 启动注册失败被静默吞掉，当前挂载期间不重试。
- P2：refresh 后 subscribe 存在丢事件窗口。
- P2：async event handler rejection 可能未处理且 mirror 停留旧状态。
- P3：旧 `go_home` callback 未检查 disposed；Library 测试泄漏 module-level `started`。

Save As 的 P1 项已在 HOME-01 最终实现中关闭。LIFE-02 已补 App 注册有界重试、
refresh-register-refresh、handler rejection、迟到 listener 清理、离开 Editor 时同步停止
DOM/transport/native playback，以及 Library stop/re-enter 的 request/lifecycle token。

第一次终审继续复现了 startup 无-floor refresh 取消已观察事件 floor、Library 旧请求迟到覆盖；
第二次终审继续复现了正常 `forceRefresh` 接替事件 refresh 后的虚假失败，以及 Media 同步恢复后
旧错误不清。对应实现 Agent 已用 deferred 交错先得到 3 项失败，再实现共享 mirror refresh owner、
最高 floor 重检/追赶及 media sync error owner channel；关联 5 个文件、94 项测试和生产构建通过。
当前等待稳定工作树上的最终 code/TypeScript reviewer，未清零前仍不关闭 PRJ-07/08、SHELL-01。

### Media 交互

Agent：`/root/fix_media_interactions`。

范围：鼠标/键盘素材选择统一、Delete/menu 对可见选中生效、文件夹选择/Enter 可达、搜索清空与失败的 stale response 防护、异步 listener disposed 清理。

代码验证：红测先得到 2 个文件中 6 fail / 3 pass；第一轮修复后聚焦 2 个文件、13 项全部通过，
全量 Web 122 个文件、909 项全部通过，生产构建与 `git diff --check` 通过。误在 sibling
`OpenTake` 工作树产生的本任务 hunks 已用 `apply_patch` 精确撤销，并证明只保留该工作树原有
preload 改动。

第一轮独立 review 提出 roving focus/Delete 错目标、Space 被 transport capture 抢占、Delete repeat、
旧 index status rejection、ARIA/menu 焦点以及搜索右键共 6 项；整改后聚焦 3 个文件、33 项、
共享全量 941 项及构建通过。最终 review 没有据此放行，又复现：Tab/直接 focus 卡片仍可删除隐藏
selection、模块级删除锁跨工程污染、应用菜单 Delete 绕过统一事务、搜索 index progress 的跨工程
事件、pointer 菜单 Escape 焦点，以及原 Space 集成测试停在 Home 导致捕获层未运行的假阳性。
这些问题正在以 A deferred -> 切换 B、hidden A -> focus B -> Delete、Editor 普通 Space 对照组等
行为测试返工；finding 清零前不关闭 MED 代码或原生验收。

### SET-04 — 官方 Codex CLI / ChatGPT

2026-08-02 复核时本机已是 `codex-cli 0.146.0`，`codex login status` 实际返回
`Logged in using ChatGPT`。
使用应用代码同款 `codex exec --json --ephemeral --ignore-user-config --ignore-rules
--sandbox read-only` 与唯一 `opentake` MCP 配置完成一次真实只读调用；`get_timeline`
返回当前工程 `1280x720`、`30 fps`、2 条轨道，进程退出码 0，工程未变更。

首次调用仍注入用户技能/Agent 描述，消耗约 100,009 input tokens。增加
`skills.include_instructions=false` 及关闭 multi-agent/plugins/shell/apps/browser/computer-use
等无关能力后，同一调用降至 36,310 input tokens，且仍只使用 `opentake.get_timeline` 成功。
用户 Agent role TOML 与本地 model cache 的诊断仍会由 CLI 输出；正式集成需要在候选包中固定
最小能力参数并再次完成一项可撤销的真实 MCP 编辑，不能把当前只读成功当作 SET-04 最终通过。

按当前官方 Codex 手册重新核对后，又执行了一次 `--strict-config` 最小能力实测。第一轮严格校验
以退出码 1 拒绝了当前 CLI 尚不识别的 `tools.view_image`（虽然当前在线配置参考已列出该字段），
证明参数不能只按文档猜测；移除该字段后的相同调用退出码 0，只出现
`opentake.get_timeline` 一次 started/completed，返回 `1920x1080`、`30/1 fps`、0 轨道，usage 为
38,623 input / 24,064 cached input / 210 output / 59 reasoning tokens。JSONL 还含 6 条用户 agent role
格式诊断，stderr 含 4 条本地 model manager timeout，但均未成为 turn failure，应用现有 parser 也不会
把它们显示成工具失败。

代码审计同时确认现有应用内 Codex 仍连接无工程身份的全局 MCP URL。对话启动虽校验 epoch/path 并
在切工程时请求取消外部进程，但迟到或在途 HTTP tool call 还缺少与 `ProjectTurnGate` 等价的原子身份
租约。正式方案将为每个应用内 Codex turn 创建临时、工程绑定的 loopback MCP 会话，并在切换工程时
拒绝/取消迟到写；该竞态测试、最小能力参数及一项可撤销真实编辑全部通过前，SET-04 保持未关闭。

## 布局与可用性实测

浏览器仅用于可重复的 DOM/尺寸诊断；不能替代 Tauri 桌面成功证据。

已测视口：1200×800、1024×700、800×600，以及应用声明的最小窗口 760×480。

通过：Home 与 Library 在所有上述尺寸无 body overflow；800×600 Settings 的主要内容可操作；Codex/ChatGPT provider 入口、缺 CLI 状态和 disabled 登录按钮语义清晰。

LAYOUT-01 已修复并经三轮独立 code review 清零：Settings 在 760×480 为 720×440，内容与侧栏
独立滚动，具备 dialog 语义、初始 Shift+Tab/Tab focus trap、嵌套 Dropdown 第一次 Escape 仅关闭
listbox 并回焦 trigger、第二次 Escape 关闭 Settings 并恢复入口焦点；三栏组合最小宽度逐层传播，
Default + Agent 在 760px 的实测宽度为 Agent 240 / Media 160 / Preview 200 / Inspector 160。
不足以容纳的 Media preset 会保留 Agent mounted subtree/draft 后自适应折叠，并安全移交焦点；800px
恢复时同一节点和状态仍在。Library 搜索输入为真实 26px target 并具 search 语义。

验证：聚焦 5 个文件、20 项；共享全量 123/941；生产构建和 `git diff --check` 均通过。Chromium
实测上述键盘流、live resize、节点身份与 exact widths；应用 console 0 error（仅 favicon 404）。
截图：`output/playwright/layout-01/reviewer-fixes/editor-agent-760x480-final.png`、
`editor-media-agent-folded-760x480.png`。

Accessibility / motion 独立验收已关闭后续 4 个 HIGH：Editor 紧凑控件与轨道高度拖拽均具备
至少 24px 命中区；SplitPane 采用不占布局、且不遮挡缝边按钮的 24px effective band；Export 与
Project Settings 具备初始焦点、双向 focus trap、Escape 与入口焦点恢复；Home / Library /
Editor 保活节点、local state 与滚动位置，隐藏 Home 不再处理 Enter/Escape。真实 Chromium 在
760×480 证明无全局 overflow、无 24px 黑色分隔带，空白分隔缝可鼠标拖拽且三处分隔栏均可由
Tab 和方向键操作；focused 13 个文件、94 项测试通过。该范围独立 reviewer 结论为 APPROVE。

当前共享树的 Web 全量与 `tsc -b` 仍因时间线原子手势重构的落盘中间态失败；上述范围通过不
替代最终稳定树上的 full / typecheck / build，也不会据此提前放行候选包。

截图位于 `output/playwright/`；最终候选包将重新生成对应 before/after 证据。

## 外部条件

以下不允许伪造成功：

- 真实付费生成/翻译/Avatar/Voice Clone 需要 provider 凭据与明确费用授权；无凭据时只验证 fail-closed。
- Windows 原生 UI 必须在真实 Windows 交互环境验证；本机 macOS 结果不能替代。
- macOS Developer ID 与 notarization 需要发布凭据；当前 ad-hoc 包只可作为本机 Beta 使用包。
- GitHub 上 `v1.0.0-beta.1` 已发布为 prerelease，目标提交为起点 `c73c192...`；当前本机
  `gh` keyring token 已失效。修复后候选仍可本机构建、重装和验收，但上传新资产或发布后继
  prerelease 前必须由仓库所有者重新授权，不能伪造上传成功。
