# OpenTake 1.0.0-beta.5 本地候选审计

日期：2026-08-15（Asia/Shanghai）

候选分支：`release/v1.0.0-beta.5`

候选源码：`a6e6ebf1de7a228cc61a630eb23144e5ed834245`

环境：macOS 26.6.1 (25G76) arm64；Rust/Cargo 1.96.0；Node 22.17.1；pnpm 10.30.1；npm 10.9.2。

## 当前结论

Beta 5 的源码级、真实 Keychain/MCP、真实 Chromium/FFmpeg、Web、许可证、发布工作流、macOS ARM64 本地打包及最终 `.app` GUI 点击/截图门禁均已完成。ad-hoc `.app` 与 DMG 已生成并完成签名结构、磁盘映像、sidecar、版本/架构及真实启动检查；同一最终包完成 Home、素材库、设置、Motion Studio、真实 Codex 清空时间线 PNG 与标题栏像素验收。GitHub Actions 的两个 updater signing secret 名称已做只读存在性预检。当前候选仍不是可公开发布状态：尚未经过远端 PR/main CI、不可变 tag、真实签名产物与十七项资产验证。

## 版本与发布身份

- commit `35de353`：`chore(release): prepare v1.0.0-beta.5 metadata`
- commit `3226456`：`fix(release): enforce CodeMirror license inventory`
- commit `152c24c`：`fix(settings): allow packaged window resizing`
- commit `b79d81d`：`test(shell): measure packaged titlebar alignment`
- commit `d9a12ce`：`fix(agent): preserve Codex MCP image results`
- commit `a6e6ebf`：`fix(shell): calibrate packaged traffic lights`
- 11 个 `opentake-*` workspace package、Tauri 与 Web：`1.0.0-beta.5`
- Windows WiX：`1.0.0.5`
- 正常 tag push 只绑定 Beta 5；唯一获批 `workflow_dispatch` 恢复链只绑定 Beta 4 / WiX `1.0.0.4`。Beta 4 与 Beta 5 交叉身份均 fail closed。
- 发布身份与许可修复的独立复核最终结论均为 Spec PASS / Quality APPROVE；五个 job digest 与受影响的 validate/quality run digest 均独立重算一致。CodeMirror fail-closed checker/test 已进入 CI 与 release workflow。两个 signing secret 名称已存在；完整发布只因远端 PR/main CI、真实签名产物与 tag 后资产验证尚未完成而保持 BLOCK。

## 聚焦产品门禁

| 范围 | 命令摘要 | 结果 |
|---|---|---|
| 外部 MCP lifecycle/catalog | `cargo test -p opentake-tauri external_mcp::tests --lib -- --test-threads=1` | 40/40 通过 |
| 真实 Keychain 与 loopback transport | `OPENTAKE_RUN_REAL_KEYCHAIN_MCP=1 cargo test -p opentake-tauri --features external-mcp-integration --test external_mcp_integration -- --nocapture --test-threads=1` | 2/2 通过；重启认证、Host/Origin、撤销、端口冲突、disable/exit socket、跨 session undo、项目切换取消与 token 扫描均通过 |
| Agent MCP / clear-timeline PNG | `cargo test -p opentake-agent mcp:: -- --test-threads=1` | 191/191 通过 |
| Agent ordered protocol | Agent `chat::` + Tauri `chat::tests` | 61/61 + 24/24 通过 |
| Motion document/publish/render bridge | Tauri `motion_documents::tests`、`motion::tests`、`render::tests` | 17/17 + 12/12 + 17/17 通过 |
| 设置、Storage、素材库、Home、标题栏、Agent、Motion UI | Beta 5 UI 聚焦矩阵；最终 Codex/标题栏增量矩阵 | 299/299 + 43/43 通过 |
| 真实 Chromium | `cargo test -p opentake-motion --all-features --test chromium -- --nocapture --test-threads=1` | 最终 7/7 通过；4K opaque 5.949 s、transparent 9.572 s，整组 40.38 s |
| 真实 Motion FFmpeg | `OPENTAKE_RUN_FFMPEG_TESTS=1 cargo test -p opentake-tauri --test motion_integration -- --nocapture --test-threads=1` | 1/1 通过 |
| Motion command | `cargo test -p opentake-tauri --test motion_command -- --nocapture --test-threads=1` | 1/1 通过 |

Chromium 首次运行在 opaque 5.311 s 后，transparent 触发测试内 180 s timeout，随后三项因共享 gate poison 继发失败。进程结束后没有残留 Chrome；独立 4K 复跑以 6.336 s / 9.752 s 通过，随后同一完整单线程命令 7/7 通过。该首次失败保留为环境/共享浏览器资源波动证据，不被覆盖成一次全绿历史。

## 完整 Rust 与 Web 门禁

| 命令 | 结果 |
|---|---|
| `cargo test --workspace --no-fail-fast -- --test-threads=1` | exit 0；`--list` 发现 2,844 项；所有运行项通过。显式 ignored：Main10 真实素材 1、media Main10 fixture 1、真实设备 export probe 3、真实设备 playback probe 4 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 通过；仅 Cargo 既有 `block 0.1.6` future-incompatibility 提示 |
| `cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| playback-engine transport integration | 6/6 通过 |
| `pnpm -C web test` | 149 files / 1,371 tests 通过 |
| `pnpm -C web build` | 通过；仅既有 ineffective dynamic import 与大 chunk 警告 |
| `pnpm -C web install --frozen-lockfile --offline` | 通过，lockfile 无变化 |

## 许可证、依赖与供应链

- `scripts/test_check_license_inventory.py`：10/10；`scripts/check_license_inventory.py`：通过。合同精确覆盖全部五个 CodeMirror 直接生产依赖，并拒绝任何未登记的第六个 `codemirror` / `@codemirror/*` 依赖。
- Web production license inventory：CodeMirror/Lezer 与记录版本均为 MIT；Tauri/React 等许可证清单生成成功。
- Motion Canvas `npm ci --ignore-scripts`：第一次下载收到 `ECONNRESET`；验证 1,639 个 npm cache object 后原命令重试成功。
- Motion Canvas `npm audit --audit-level=moderate`：0 vulnerabilities；license lock：123 records；job tests：2/2；runner build 与已提交 `bundle/runner.html` byte-exact，无 diff。
- `cargo audit` 在线更新 RustSec 时因 GitHub I/O 失败未完成 fetch。使用 2026-08-12 的本机 advisory-db（commit `69f93e1d…`）执行 `cargo audit --no-fetch --stale`：0 vulnerability，22 条允许的 informational warning；其中包含 Linux GTK3/glib 0.18 的 unmaintained/unsound 通告以及既有传递依赖通告。不能把该缓存结果表述为 2026-08-14 在线最新审计。

## 发布与完整性门禁

- Windows product contract：88/88。
- updater attestation：5/5；updater manifest：12/12。
- release workflow：默认 Beta 5 环境 83/83；Beta 4 recovery 环境完整 83/83；checker 通过。CI 与 Beta 5 release quality 均在 Web 依赖安装后运行许可 checker/test；Beta 4 recovery 对 CodeMirror 直接依赖执行通用空集合断言。
- provisioner tests：7/7；Windows sidecar provision fixture 验证通过。
- `actionlint`、严格 YAML parse、`git diff --check`：通过。
- Gitleaks 8.30.1 以 `gitleaks git . --log-opts="50aebdfb2e0dad4cdeacfdcde1180444cfa96589..a6e6ebf1de7a228cc61a630eb23144e5ed834245" --no-banner --no-color --redact --report-format json --report-path -` 重扫 61 commits / 1,654,531 bytes：3 个命中仍全部来自 `ExternalMcpPane.test.tsx` / `SettingsView.interaction.test.tsx` 中固定假 `tokenDigest`（`abc123def456` / `def456abc123`）及历史版本；没有真实 token/private key。真实 Keychain MCP 矩阵对 captured log/catalog 的完整 token 扫描同样为 0 matches。

## macOS ARM64 本地打包证据

使用已校验的 `aarch64-apple-darwin` FFmpeg/FFprobe sidecar 执行：

```text
./web/node_modules/.bin/tauri build --ci --target aarch64-apple-darwin \
  --bundles app,dmg \
  --config '{"bundle":{"createUpdaterArtifacts":false,"macOS":{"signingIdentity":"-"}}}'
```

- `.app`：`target/aarch64-apple-darwin/release/bundle/macos/OpenTake.app`，`du -sk` 为 170,040 KiB。
- DMG：`target/aarch64-apple-darwin/release/bundle/dmg/OpenTake_1.0.0-beta.5_aarch64.dmg`，70,337,681 bytes，SHA-256 `be1b5334f0272a6378085ad913eff318ce85b2730be77c8a01febb4e148804b3`。
- 主程序：75,284,176 bytes，SHA-256 `20babb3ad5f42313f29fda73cce903fb99032eda8b04dc2ac43ea51a1a3599d8`。
- 包内 `Contents/MacOS/ffmpeg`：48,789,568 bytes，SHA-256 `a83e9395c338b9e759cfba5b797e4206e60d46572c05b7467d2a33e30f53fcc3`。
- 包内 `Contents/MacOS/ffprobe`：48,731,824 bytes，SHA-256 `8a5ff4c6b60ce86cc6e4c7b0bd026aa7a6e91ebfb7748dbbcae68b6ef1d04739`。
- `.app`、主程序及两个 sidecar 的 `codesign --verify --deep --strict` 均通过，签名类型均为 ad-hoc。
- `hdiutil verify` 通过；挂载后的 `.app` 再次通过 `codesign`，挂载包内两个 sidecar 的执行检查通过。
- `Info.plist` 为 `CFBundleShortVersionString=1.0.0-beta.5`、`CFBundleIdentifier=com.opentake.desktop`；主程序与 sidecar 均为 Mach-O arm64。
- 使用最终 `.app/Contents/MacOS/opentake` 绝对路径启动，并为应用进程提供正式 `codex-cli 0.146.0` 所在 PATH；GUI 验收期间确认运行的 executable 精确来自该最终 bundle。第一次把相对路径传给 `open -na` 的失败属于命令用法错误，不计为产品启动失败。

本地构建有意关闭 updater artifact，并没有使用或伪造发布签名 secret；因此未生成 `.app.tar.gz.sig`，也不能替代远端签名/发布工作流。

## 可视证据

- `screenshots/motion-studio-editor.png`：1600×1000，110,680 bytes，SHA-256 `c808bd9b290df3a49c0b021827ccbffe3121efbd05bd7d88b42c044cefb90501`。
- `screenshots/motion-studio-preview.png`：生产 `HeadlessChromiumRenderer` 输出 1280×720，639,940 bytes，SHA-256 `06ba67b1f537d80112d5e0edd88f412b84e5c34c9b7455157cad87953d31e60b`。
- `screenshots/beta5-packaged-home.png`：最终 `.app` Home 与 16:9 项目卡，2132×1332，337,532 bytes，SHA-256 `7bc7f19ae79e919b459b9e6949842d07dfb78b4b2472958b53ef0f7b933c0f3b`。
- `screenshots/beta5-packaged-library.png`：最终 `.app` 素材库，Home 为左栏首控件且页面头不重复，2132×1332，171,370 bytes，SHA-256 `a2a2ffe570f2d1a2ccddf6cdc915dceabe88458b31819bfc8d5f59c816189e83`。
- `screenshots/beta5-packaged-agent.png`：正式 Codex 删除真实 Motion clip 后，`remove_clips` 披露内联显示生产 PNG，2132×1332，352,175 bytes，SHA-256 `8d479a608abb9763e1f15a3f59edb95941f32ae290cb9cfcbfb1cd003f4ef527`。
- `screenshots/beta5-packaged-motion.png`：工程重开后独立 Motion Studio 的真实 HTML、CSS 动画预览、90 帧时间线与发布入口，2132×1332，277,986 bytes，SHA-256 `6cfadca6e44175ac3defbb29831dd361c94dfe6672bb5e4cac9c895df0194f51`。
- `screenshots/beta5-packaged-settings.png`：最终 `.app` 外观设置只显示“深色 · 标准 / 深色 · 紧凑”，无浅色开关和勾号，2132×1332，156,492 bytes，SHA-256 `8a36c679fc90a22a8ddfd5fb6fb909bde2ca861c60268c53ed3be978eb839927`。
- 最终 `.app` 的 Storage 页还实际打开并取消了 141 MB 模型的内联移除确认：说明文字在同一固定行下方呈现，没有跳到独立浮层或删除模型；正常/`prefers-reduced-motion` 进入退出、迟到异步和焦点恢复由 Storage/Reveal 自动化回归覆盖。
- 最终 `.app` 的 External MCP 页实际执行启用、创建 `Beta5 packaged GUI` 配对、清除一次性回执、撤销与 Keychain readback。启用后 `127.0.0.1:19789` 由最终 bundle 进程监听；撤销后 catalog 记录 `revokedAt`，`security find-generic-password -s io.opentake.app -a external-mcp:<client-id>` 返回 44 且端口无 listener。`enabled=true` 按设计持久化，但没有活跃客户端时 lifecycle 为 paused，未留下可连接端点。
- 最终 `.app` 中已实际执行：重开有持久 Motion 文档的工程、发布 90 帧/3 秒 Motion Graphic、创建新 Agent 会话、输入 `Remove all clips from the timeline and report the result.`、等待正式 Codex 调用 `get_timeline` / `remove_clips` 并展开工具结果。时间线由 1 clip 变为空；新 session JSON 中 `remove_clips.result.content` 为 `text + image`，PNG base64 长度 10,268，且不存在 `structuredContent`。
- 标题栏以最终 Agent PNG 和 macOS AX 几何共同测量。原始 AX receipt：window `position=202,162 size=1066,666`；close `219,173 16×16`、minimize `242,173 16×16`、fullscreen `265,173 16×16`；Home `284,167 26×27`、Chat `316,167 26×27`、Motion `348,167 26×27`。将屏幕坐标减窗口原点再乘 Retina scale 2，得到 PNG traffic group `34,22,124,32`，Home/Chat/Motion 分别为 `164,10,52,54` / `228,10,52,54` / `292,10,52,54`。完整重放命令与输出：

```text
python3 -B scripts/measure_titlebar_alignment.py \
  docs/audit/2026-08-13/screenshots/beta5-packaged-agent.png \
  --scale 2 \
  --traffic-rect 'traffic:34,22,124,32' \
  --icon-rect 'home:164,10,52,54' \
  --icon-rect 'chat:228,10,52,54' \
  --icon-rect 'motion:292,10,52,54'

traffic: center 19.000 CSS px
home: center 18.500 CSS px; deviation 0.500 CSS px
chat: center 18.500 CSS px; deviation 0.500 CSS px
motion: center 18.500 CSS px; deviation 0.500 CSS px
maximum deviation: 0.500 CSS px
PASS
```
- Web/Tauri 几何合同继续覆盖 16:9 preview、38px titlebar、26×26 控件；macOS 原生 traffic light 配置使用经最终包实测校准的 `y=21`。

## 未完成 / 发布阻断

1. 本地 `.app`/DMG 已生成并完成 GUI/像素/内容验收，两个 updater signing secret 名称已在 Actions 中存在；但真实 `.app.tar.gz.sig` 只能由 tag workflow 生成，不能在本机伪造，十七项远端资产/attestation 验证尚未运行。
2. 尚未 push release branch、创建/合并 PR、等待远端 main CI、创建 `v1.0.0-beta.5` annotated tag 或公开 GitHub prerelease。
3. 本地 macOS 包仍为 ad-hoc 且未 notarize；Windows 仍无 Authenticode。必须继续在 release note 与最终 receipt 中明确。

在上述阻断关闭前，不创建、不移动也不发布 Beta 5 tag；Beta 4 tag 与资产保持不可变回滚候选。
