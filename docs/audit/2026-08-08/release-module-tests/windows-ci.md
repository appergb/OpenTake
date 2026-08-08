# Windows 代码逻辑与 CI 发布前独立测试

- 日期：2026-08-08（Asia/Shanghai）
- 范围：只审查 Windows 代码逻辑、交叉编译可达性与 CI 合约；**未做 Windows 实机 GUI 测试，也不作此类声明**。
- 仓库 / 分支：`OpenTake-generation` / `audit/remediation-2026-08-05`
- 审计起始 HEAD：`f9398a9302e57690caa3ea21ff995fdd0ce97706`
- 最终裁决：**FAIL**

## 裁决摘要

现有 CI 已有很强的 Windows 原生覆盖：exact-SHA `windows-product` 会在 `windows-2022` 上执行完整 Rust/Web 逻辑、构建 MSI+NSIS、安装 NSIS、验证安装后的固定 FFmpeg sidecar、启动已安装程序，并生成含 SHA-256 的安装包 receipt；`safe-filesystem` 也有 exact-SHA Windows matrix receipt。Windows 取消、Job Object、reparse、library capability 和 safe-fs 代码/测试均真实存在，不是占位命令。

但当前工作树还不能被裁为“提交后由 CI 验证的完整 Windows 发布门禁”，原因如下：

1. `windows-security` 和 `windows-library-security` 在 `workflow_dispatch(commit_sha=...)` 下没有使用 `commit_sha`、没有 exact-SHA checkout/assertion，也没有绑定候选 SHA 的逻辑 receipt；同一运行可能让 `windows-product`/`safe-filesystem` 测候选 SHA，而两个专项 job 测 dispatcher/default ref。
2. 仓库只有 `.github/workflows/ci.yml` 一个 workflow，tag trigger 为 0，`v1.0.0-beta.2` tag 也不存在；尚无 tag→exact commit→测试→发布资产的 release workflow。
3. safe-fs 的两个仓库自带合约/证据自测当前都失败：`validate-c1b-ci.rb` 的允许 job 集未包含已存在的 `motion-canvas`，因而把规范 `ci.yml` 判为 `workflow contains unknown or missing jobs`。该验证器也没有接入当前 CI。
4. 审计 HEAD 未被任何 remote branch/tag 包含，且 Windows 相关的 `proxy.rs`、`media.rs`、`safe_asset_protocol*` 仍有未提交改动；不存在当前工作树的 exact-SHA Windows 运行 receipt。

因此，此结论不是“Windows 逻辑明显坏掉”，而是“发布候选的 exact-SHA 门禁链和可证明性尚未闭环”。

## 工作区与提交覆盖

`git merge-base --is-ancestor <sha> HEAD` 对下列关键提交均 exit 0：

| 门禁/实现 | 引入或关键提交 | HEAD 包含 |
|---|---:|---:|
| exact-SHA Windows full product | `1d46d253` | 是 |
| 固定 FFmpeg Windows cancellation | `c73c192` | 是 |
| Windows library capability job | `99dc3074` | 是 |
| exact-SHA safe-fs receipts | `f579b999` | 是 |
| 安装包固定 sidecar | `21f7e9eb` | 是 |
| 跨平台发布门禁加固 | `83016dea` | 是 |
| Job Object / Windows cfg 后续修复 | `815db33` | 是 |

审计时 `git diff HEAD --` 显示下列 Windows 可达代码仍未提交；门禁/workflow/lock/config 本身没有未提交差异：

- `crates/opentake-media/src/proxy.rs`：`+671/-41`
- `src-tauri/src/media.rs`：`+326/-43`
- `src-tauri/src/safe_asset_protocol.rs`：`+205/-24`
- `src-tauri/src/safe_asset_protocol/helper.rs`：`+59/-21`
- `src-tauri/src/safe_asset_protocol/tests.rs`：`+354/-0`

`git branch -r --contains HEAD` 与 `git tag --contains HEAD` 均无输出，故本地 HEAD 本身不能已有远端 Windows CI receipt。

## Windows 门禁盘点

### 1. Full product / bundle

`.github/workflows/ci.yml:149-331` 的 `windows-product`：

- `windows-2022`，120 分钟硬超时。
- `TARGET_SHA` 分别绑定 dispatch input、PR head SHA、push SHA；checkout 使用该 SHA、`fetch-depth: 0`、`persist-credentials: false`，随后比较实际/期望 SHA 并要求 clean tree。
- 固定安装 Node 22、pnpm 10、Ruby 3.3；Cargo cache 明确不缓存 `target`。
- 执行 workspace fmt/clippy/test、minimal-feature clippy、Web test/build，以及 3 个 Windows Chromium/motion 定向回归。
- 用 `scripts/provision_ffmpeg_sidecars.py --target x86_64-pc-windows-msvc` 物化 lock 中固定 SHA 的 FFmpeg/ffprobe，并在 PATH 为空的边界验证其 probe/decode/encode 能力。
- 清空旧 bundle 后执行 `tauri build --ci --bundles msi,nsis`。
- 静默安装唯一 NSIS，定位安装后的 `opentake.exe`，对安装目录再次执行 sidecar 验证，并做 5 秒 launch smoke；这不是 GUI 功能验收。
- receipt 至少要求一个 MSI 和一个 NSIS，记录 source SHA、runner、path、bytes、SHA-256；artifact 名称绑定 checkout SHA，`if-no-files-found: error`。

结论：**该 job 的 exact-SHA full-product 合约成立，contract validator 和 19 个 mutation tests 均通过。**

### 2. Cancel / process tree / reparse

`.github/workflows/ci.yml:333-478` 的 `windows-security` 在原生 Windows runner 上执行：

- 固定 sidecar 可运行性 + 2 个 FFmpeg cancellation 生命周期测试：
  - `windows_cancelling_running_pcm_child_reaps_both_pipe_readers`
  - `windows_cancelling_mux_wait_reaps_child`
- 1 个 race-free Job Object 测试：
  - `process_tree::tests::windows_suspended_job_contains_fast_exit_descendant`
- 4 个 retained output / junction / reparse 测试：
  - `windows_project_media_junction_is_rejected_without_writing_target`
  - `windows_directory_handoff_blocks_junction_replacement_before_child_create`
  - `windows_retained_output_handle_blocks_final_name_replacement`
  - `retained_external_source_rejects_windows_reparse_contract`
- 另外检查 Tauri test image 的 native imports、manifest 与导出符号，并上传诊断 artifact。

生产实现并非伪门禁：`process_tree.rs` 在 Windows 先以 `CREATE_SUSPENDED` 启动，设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`，把进程分配到 Job Object 后才恢复主线程；attach 失败时子进程仍未执行用户代码，drop/terminate 路径 fail closed。取消测试会等待 FFmpeg child 被 kill+wait，并验证两个 pipe reader 都退出。

缺口：该专项 job 只用默认 `actions/checkout@v4`，没有 `TARGET_SHA`、`ref`、SHA assertion 或逻辑 receipt。PR/push 通常会拿到事件 SHA，但 release 所需的 `workflow_dispatch(commit_sha=...)` 并没有同一候选绑定。虽然 exact-SHA `windows-product` 的 workspace test 会再次覆盖这些测试，专项安全证据仍不是候选 SHA 独占 receipt。

### 3. Library capability security

`.github/workflows/ci.yml:506-538` 的 `windows-library-security` 执行：

- `cargo test -p opentake-media library::tests -- --test-threads=1`
- `cargo test -p opentake-project -- --test-threads=1`
- `cargo test -p opentake-tauri library::tests -- --test-threads=1`
- `cargo clippy -p opentake-media --all-targets -- -D warnings`

Windows 实现包含 no-follow 打开、retained file identity、DELETE access/share-mode、由 retained handle 做 rename/delete、junction 拒绝等真实分支。macOS 本机只能验证共享/Unix 可达行为：media library 48/48、Tauri library 22/22 通过；Windows 专属的 delete-sharing、handle rename 和 junction 测试只会在 Windows runner 执行。

缺口与 `windows-security` 相同：job 没有 dispatch candidate 的 exact-SHA checkout/assertion/receipt。

### 4. Safe filesystem exact-SHA matrix

`.github/workflows/ci.yml:540-698` 的 `safe-filesystem`：

- matrix 明确包含 `windows-x86_64 / windows-2022 / Windows / X64`。
- dispatch/PR/push 都生成 40-hex `TARGET_SHA`，exact checkout 后再次比较 commit/tree 并要求 clean tree。
- Windows runner 会用 PowerShell AST 解析 expected-RED harness。
- 每条 native gate 单独保留命令 log 与 raw exit：fmt、project clippy、`safe_fs` 单测、`archive_security`。
- `always()` 生成含 requested/checked-out/dispatcher SHA、runner provenance、每条 command exit、aggregate exit 的 JSON receipt；artifact 名称绑定 checked-out SHA；最后强制 aggregate 必须为 0。

`safe_fs/windows.rs` 是约 3,000 行的真实 NT/Win32 retained-capability 实现，覆盖 no-follow query/open、capability revalidation、owner-only create/rollback、retained no-replace rename、quarantine/delete 等；Windows 编译面包含 23 个内部测试，外层另有 common component、`windows_contract` 和 synchronous-pending contract，CI 的 `safe_fs` 过滤器会覆盖这些 Windows-only 测试。

缺口：`scripts/tests/validate-c1b-ci-test.rb` 和 `validate-c1b-evidence-test.rb` 当前都 exit 1。根因是 `scripts/validate-c1b-ci.rb:21-29` 的 `NORMAL_JOBS`/`ALL_JOBS` 未包含 `.github/workflows/ci.yml:39` 的 `motion-canvas`，并在第 147 行要求 job 集精确相等。这是 validator 漂移，不是 safe-fs Rust test failure，但会阻断可独立验证的 receipt 证明链。

## 实际执行记录

| 命令 | Exit | 计数 / 结果 |
|---|---:|---|
| `python3 -B scripts/check_windows_product_ci.py` | 0 | `Windows product CI contract is complete` |
| `python3 -B -m unittest discover -s scripts -p 'test_check_windows_product_ci.py' -v` | 0 | 19 passed，0 failed |
| `python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v` | 0 | 4 passed，0 failed |
| Ruby Psych 解析 `ci.yml` | 0 | YAML syntax OK |
| PyYAML unique-key loader 解析 `ci.yml` | 0 | 无重复 key |
| PowerShell AST 解析 `run-c1b-windows-red.ps1` | SKIP | 本机无 `pwsh`/`powershell`；Windows CI 有同等 parse step |
| `actionlint .github/workflows/ci.yml` | SKIP | 本机未安装 `actionlint` |
| `cargo check -p opentake-project --lib --target x86_64-pc-windows-msvc` | 0 | Windows safe-fs/project 分支类型检查通过 |
| `cargo check -p opentake-media --lib --target x86_64-pc-windows-msvc` | 101 | `onig_sys` C build 找不到 `stdlib.h`；本机无 Windows C/MSVC SDK headers |
| `cargo test -p opentake-project --lib safe_fs --no-run --target x86_64-pc-windows-msvc` | 101 | Rust target 已安装，但链接阶段找不到 `link.exe` |
| `cargo test -p opentake-project --lib safe_fs -- --test-threads=1` | 0 | macOS common/Unix 26 passed，0 failed，139 filtered |
| `cargo test -p opentake-project --test archive_security -- --test-threads=1` | 0 | 4 passed，0 failed |
| `cargo test -p opentake-media library::tests -- --test-threads=1` | 0 | macOS 48 passed，0 failed；其他 test binary 均被过滤 |
| `cargo test -p opentake-tauri library::tests -- --test-threads=1` | 0 | macOS 22 passed，0 failed |
| `ruby scripts/tests/validate-c1b-ci-test.rb` | 1 | canonical workflow 被判 `unknown or missing jobs` |
| `ruby scripts/tests/validate-c1b-evidence-test.rb` | 1 | 同一 validator 漂移导致 synthetic evidence 被拒 |
| `git rev-parse -q --verify 'refs/tags/v1.0.0-beta.2^{commit}'` | 1 | Beta 2 tag 不存在 |
| 版本快照断言 | 0 | workspace/Tauri/Web=`1.0.0-beta.2`，WiX=`1.0.0.2` |
| workflow/tag trigger 计数 | 0 | workflow=1，含 `tags:` 的 workflow=0 |

交叉编译边界必须严格解释：本机 host 是 `aarch64-apple-darwin`，已安装 Rust `x86_64-pc-windows-msvc` 标准库，因此 `cargo check` 成功可以证明 Rust Windows 分支可编译；缺 `link.exe`/MSVC SDK/Windows C headers 使 test image 无法生成和运行。这些结果不能替代 Windows native CI，更不能替代 GUI 实机测试。

## Tag release workflow 要求审查

| 要求 | 当前状态 | 证据 / 缺口 |
|---|---|---|
| tag 精确 SHA | **FAIL** | 无 tag workflow、无 Beta 2 tag；只有 `ci.yml` 的 product/safe-fs 两个 job 能绑定 dispatch SHA |
| 版本一致 | **PARTIAL** | 四个当前值相互匹配；validator 只检查 Tauri public version 与 WiX core，不检查 tag、workspace、Web |
| Windows MSI+NSIS | **PASS（CI job 能力）** | product job 清理旧输出、构建两类包并 fail if missing；尚未发布到 tag release |
| 固定 sidecar | **PASS（CI job 能力）** | Windows FFmpeg/ffprobe URL、version、下载 SHA 固定，安装后在无 PATH 边界复验 |
| Windows 逻辑测试 | **PARTIAL** | full product exact-SHA workspace tests + 多个专项 native job；security/library 专项 job 不绑定 dispatch candidate |
| 安装包 receipt | **PARTIAL** | product receipt 有 source SHA、bytes、SHA-256；没有 tag/version/逻辑 job receipt 汇总，也没有 release 发布链 |
| SHA-256 | **PARTIAL** | MSI/NSIS hash 在 JSON receipt；没有 release workflow 上传资产/独立 checksum 清单并回读验证 |
| fail closed | **PARTIAL** | product/safe-fs job 内部无忽略失败且缺资产即失败；release job 不存在，C1B receipt validator 当前自身失败且未接 CI |

## 可执行缺口清单

### P0 — 发布前阻断

1. 新增 tag release workflow，限定 `v1.0.0-beta.2`（或受控 prerelease tag pattern），解析 tag 指向的 commit，checkout 后比较 `HEAD^{commit}` 与 tag commit，要求 clean tree；后续测试、构建、receipt、发布全部只消费该不可变 SHA。
2. 让 `windows-security` 与 `windows-library-security` 使用和 product/safe-fs 相同的 `TARGET_SHA` 选择式、`checkout ref`、commit/tree/clean assertion，并生成候选 SHA 绑定的专项 receipt；或者把这些专项命令收敛进一个 exact-SHA reusable Windows gate，并让 release job `needs` 它。
3. release 发布资产步骤必须 `needs` 所有 Windows product/security/library/safe-fs gate，且只在每个 result 为 success 时运行；任何测试、receipt、安装包或 checksum 缺失都必须终止发布。
4. 修复 `validate-c1b-ci.rb` 的 job allowlist（至少纳入 `motion-canvas`），恢复两个 Ruby 自测为绿，并把这些 validator 自测接入普通 CI，防止工作流再次漂移。
5. 将当前 Windows 可达未提交改动提交成唯一候选 SHA 后再跑 native gates；不得把 `f9398a9` 的旧 HEAD 结果当作当前工作树证据。

### P1 — receipt 与版本闭环

1. 在 release gate 中比较：去掉 `v` 后的 tag、workspace version、Tauri version、Web package version；同时验证 prerelease→WiX numeric mapping（本版应为 `1.0.0-beta.2` → `1.0.0.2`）。
2. 生成 release-level receipt，至少包含 tag、tag commit、checked-out SHA、workflow SHA/ref、Windows runner、所有门禁 run/job/exit、MSI/NSIS path/bytes/SHA-256、sidecar lock digest。
3. 同时上传 JSON receipt 与人可读 `SHA256SUMS`，发布后下载回读，逐项确认 release asset hash 与 receipt 相同；任何额外/缺失安装包都失败。
4. 在 Windows runner 上保留 PowerShell AST、MSI+NSIS 安装/启动、无 PATH sidecar、Job Object、reparse、library、safe-fs 的原生证据；macOS cross-check 不得替代。

## Blocker 与最终边界

- 本机不是 Windows，缺 `link.exe`、Windows C/MSVC SDK headers、PowerShell；无法运行 Windows-only test image、MSI/NSIS 安装或 WebView GUI。
- 当前候选 SHA 尚未产生，HEAD 也未被 remote ref/tag 包含，无法给出 exact-SHA Windows Actions run/artifact ID。
- 只有上述 P0 闭环并对最终候选 SHA 跑绿后，才能把本报告的 **FAIL** 提升为 Windows 代码逻辑/CI **PASS**；Windows GUI/安装交互的实机验收仍须单独声明和取证。
