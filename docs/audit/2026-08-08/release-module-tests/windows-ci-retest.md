# Windows 代码逻辑与 CI 最终独立复测

- 日期：2026-08-08（Asia/Shanghai）
- 仓库 / 分支：`OpenTake-generation` / `audit/remediation-2026-08-05`
- 复测时 HEAD：`f9398a9302e57690caa3ea21ff995fdd0ce97706`
- 范围：Windows 代码逻辑、CI/release 静态合约与本机可执行测试；不包含 Windows 实机 GUI 验收

## 裁决

- **本地逻辑与静态 CI 合约：PASS。** Windows contract 35/35、release contract 22/22、C1B CI/evidence Ruby validators、YAML 严格解析、本机 Rust 安全/库逻辑和 Windows project cross-target check 均通过。
- **GitHub Windows exact-SHA 原生门禁：WAITING / NOT RUN。** 当前是含未提交修改的共享工作树，不能用现有 HEAD 代表本次内容；也没有本次最终候选 SHA 的 `windows-2022` / `windows-latest` receipts。
- **发布裁决：HOLD，等待最终提交及原生 CI。** 这不是本地逻辑失败；在最终 40-hex 候选 SHA 上跑绿 Windows product/security/library/safe-fs 与 tag release Windows job 后，才可提升为原生 Windows PASS。

本报告没有执行 Windows GUI、WebView2 交互或人工安装验收，也不作相关声明。release job 中的 5 秒进程存活检查只属于 launch smoke。

## 工作树与 exact-SHA 边界

复测时分支相对远端为 ahead 2，且 `.github/workflows/ci.yml`、Windows 可达 Rust/Web 代码和 validators 均有未提交修改；`.github/workflows/release.yml`、release validator/tests 仍是 untracked。`v1.0.0-beta.2` tag 本地不存在。因此：

1. `f9398a9302e57690caa3ea21ff995fdd0ce97706` 不是当前工作树内容的不可变标识；
2. 本报告的本地结果绑定的是复测当时的共享工作树快照，不是可由 GitHub checkout 的候选提交；
3. 任何旧 Windows run/receipt 都不能替代最终提交的 exact-SHA run。

## `.github/workflows/ci.yml` 复核

独立结构脚本解析 YAML job/step，而不是只相信仓库 validator。当前三个 Windows job 的 checkout 数都为 1。

| Job | 40-hex 与唯一 checkout | HEAD / tree / clean | JSON receipt / aggregate | 当前门禁 |
|---|---|---|---|---|
| `windows-product` | dispatch=`inputs.commit_sha`、PR=head SHA、push=`github.sha`；先拒绝非 40-hex；唯一 checkout 使用 `ref: env.TARGET_SHA`、depth 0、无持久凭据 | 比较 HEAD=TARGET_SHA、验证 commit object、要求含 untracked 的 clean tree；未单列 `HEAD^{tree}` 比较 | JSON 记录 source SHA、MSI/NSIS path/bytes/SHA-256；artifact 名绑定 bind SHA。无 outcome aggregate，原因是所有必需步骤无条件且未使用 `continue-on-error`，任一步失败即不会生成成功产物 | workspace fmt/clippy/tests、Web tests/build、固定 sidecar、Windows 定向回归、MSI+NSIS、静默 NSIS 安装、安装目录 sidecar、5 秒 launch smoke |
| `windows-security` | 同一 TARGET_SHA 选择式和 40-hex 检查；唯一 exact checkout | 显式比较 HEAD、`HEAD^{tree}`、目标 tree，并要求 clean | `always()` receipt 记录 requested/checked SHA、runner、4 项 outcome 与 `aggregate_exit`；artifact 绑定 checked SHA；最终 `always()` 同时检查 SHA、aggregate 和每项 outcome | 固定 FFmpeg cancellation 2 项、race-free process tree、Tauri imports/manifest/native exports、junction/retained-output/reparse 4 项 |
| `windows-library-security` | 同一 TARGET_SHA 选择式和 40-hex 检查；唯一 exact checkout | 显式比较 HEAD/tree 并要求 clean | `always()` receipt、checked-SHA artifact、4 项 outcome aggregate 和最终 fail-closed enforce | media library、完整 project、Tauri library、media all-target clippy |

`windows-product` 的 exact HEAD 相等在 Git 对象模型上已经绑定该 commit 的 tree；但 specialist 两项提供了更直观的显式 tree 证据。若发布证据规范要求每个 Windows job 都必须出现字面 `HEAD^{tree}` assertion，product 仍应补齐该行及 mutation test。

`safe-filesystem` 仍包含 `windows-2022 / Windows / X64` matrix：40-hex exact checkout、HEAD/tree/clean、PowerShell AST parse、fmt/clippy/safe_fs/archive gates、逐命令 raw exit、`always()` JSON receipt、checked-SHA artifact 和最终 aggregate enforce 均保留。

## C1B allowlist 与 validator

`C1bCiValidator::NORMAL_JOBS` 的最终集合为：

```text
motion-canvas,rust,safe-filesystem,web,windows-library-security,windows-product,windows-security
```

`motion-canvas` 已明确进入 allowlist。规范 workflow 的直接 validator、CI mutation tests 与 evidence mutation tests 均通过；此前 `unknown or missing jobs` 漂移已消失。

## `.github/workflows/release.yml` Windows 路径复核

最终 release contract 已扩展为 22 项 mutation tests，并全部通过。与 Windows 发布直接相关的静态性质如下：

- 只接受 `v*` tag push 或显式已有 tag 的 dispatch；validate 把 tag commit、checkout HEAD、远端 main、Cargo workspace/Tauri/Web 版本和 release notes 绑定。
- workflow 中 13 个外部 action 引用全部固定为 40-hex action commit；顶层 `contents: read`，只有 publish job 提升到 `contents: write`。
- `windows_x64` 使用 `windows-2022`，恰好一次 checkout，ref 为 validate 输出的 `source_sha`，随后比较 HEAD=TARGET_SHA、验证 commit object并要求 clean。它没有额外的字面 `HEAD^{tree}` 比较；与 product 相同，exact commit 等价绑定 tree，但显式证据可以进一步加固。
- 先按 lock/checksum provision Windows FFmpeg sidecars，并在构建前执行 sidecar 供应测试；随后执行 workspace clippy/tests、Web tests/build 和 minimal-feature clippy。
- 清理旧目录后构建 `msi,nsis`，强制恰好一个 MSI 和一个 NSIS；静默安装 NSIS，定位安装后的 `opentake.exe`，在安装目录运行固定 sidecar 测试，并做 5 秒 launch smoke。
- Windows receipt 强制恰好一个 MSI 和一个 NSIS，记录 source SHA、runner、文件名、bytes、SHA-256，并以 source SHA 命名 artifact。
- publish 必须依赖 validate/quality/macOS/Windows；严格核对安装包和 receipt 数量、receipt source SHA/bytes/hash，生成并校验 `SHA256SUMS`。发布草稿前和公开前均重新读取远端 tag；同 tag 已公开 release 不可变，流程保持 fail closed。

Windows release receipt 没有 specialist 风格的逐逻辑门禁 outcome aggregate，因为该 job 的测试、构建、安装、sidecar 与 receipt 步骤是普通无条件步骤，任一非零都会阻止 upload/publish。若未来允许 `always()` 收集失败诊断，则应同时增加 outcome aggregate，避免 receipt 被误读为成功证明。

## 执行记录

| 命令 | Exit | 计数 / 结果 |
|---|---:|---|
| `python3 -B scripts/check_windows_product_ci.py` | 0 | `Windows product CI contract is complete` |
| `python3 -B -m unittest discover -s scripts -p 'test_check_windows_product_ci.py'` | 0 | 35 passed，0 failed |
| `python3 -B scripts/check_release_workflow.py` | 0 | `Release workflow contract is complete` |
| `python3 -B -m unittest discover -s scripts -p 'test_check_release_workflow.py'` | 0 | 22 passed，0 failed |
| `ruby scripts/validate-c1b-ci.rb .github/workflows/ci.yml scripts/run-c1b-windows-red.ps1` | 0 | `c1b-ci-validation=ok` |
| `ruby scripts/tests/validate-c1b-ci-test.rb` | 0 | `c1b-ci-validator-tests=ok` |
| `ruby scripts/tests/validate-c1b-evidence-test.rb` | 0 | `c1b-evidence-validator-tests=ok` |
| `python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v` | 0 | sidecar provisioner 4 passed，0 failed |
| PyYAML duplicate-key rejecting parse：`ci.yml` + `release.yml` | 0 | 2/2 parsed，无重复 key |
| Ruby Psych parse：`ci.yml` + `release.yml` | 0 | 2/2 parsed |
| `actionlint` | SKIP | 本机未安装 |
| PowerShell static parse | SKIP | 本机无 `pwsh` / `powershell`；Windows safe-fs job 自带 AST parse gate |
| `cargo check -p opentake-project --lib --target x86_64-pc-windows-msvc` | 0 | 已安装 Windows MSVC Rust target；project/safe-fs Windows cfg 类型检查通过 |
| `cargo test -p opentake-project --lib safe_fs -- --test-threads=1` | 0 | 26 passed，0 failed，139 filtered |
| `cargo test -p opentake-project --test archive_security -- --test-threads=1` | 0 | 4 passed，0 failed |
| `cargo test -p opentake-media library::tests -- --test-threads=1` | 0 | 48 passed，0 failed |
| `cargo test -p opentake-tauri library::tests -- --test-threads=1` | 0 | 22 passed，0 failed；另有 future-incompat dependency warning，不影响 exit |

本机 Rust 逻辑共 100 passed、0 failed。它们运行于 `aarch64-apple-darwin`；Windows target 的 `cargo check` 只证明可编译面，不证明 Windows test image 可链接/运行，也不证明 Job Object、reparse、MSI/NSIS 或 WebView2 行为。

复测过程中共享树的 release 实现仍在并发加固，曾观察到 22 项 suite 从 6 failures 降到 4、1，最终为 22/22。上表只使用实现 Agent 完成后的最终磁盘状态和新鲜复跑结果。

## 可执行缺口清单

### P0 — 提升为原生 Windows PASS 前必须完成

1. 将当前完整工作树形成一个最终 40-hex 提交并推送；不得使用旧 HEAD receipt 代表未提交内容。
2. 对该 SHA 运行 `windows-product`、`windows-security`、`windows-library-security` 和 safe-fs 的 Windows matrix；保存 source/checked SHA 完全一致、aggregate=0 的 specialist/safe-fs receipts，以及 product MSI+NSIS receipt。
3. 在最终 tag 指向同一远端 main SHA 后运行 release workflow；保存 `windows_x64` 安装包 receipt、MSI、NSIS、`SHA256SUMS` 和 release run URL。所有 Windows 测试、构建、静默安装、安装目录 sidecar 和 launch smoke 必须成功。

### P1 — 静态证据加固

1. 给 `windows-product` 与 release `windows_x64` 的 checkout assertion 增加显式 `HEAD^{tree}` 比较。
2. 扩展 contracts，使 product/release Windows job 都拒绝第二个 checkout；当前磁盘实测均只有一个，但 product/release helper 的 mutation coverage 不如两个 specialist job 严格。
3. 若 product/release receipt 将来改成失败后也生成，加入逐 gate outcome 和 aggregate enforcement；当前无条件、无 `continue-on-error` 的流程已能 fail closed。

## Blocker

唯一发布阻断不是本地逻辑红灯，而是**尚无最终候选提交及其 GitHub-hosted Windows exact-SHA 原生证据**。在该证据形成前，结论保持：本地逻辑/静态 CI **PASS**，Windows 原生与发布 **WAITING/HOLD**。
