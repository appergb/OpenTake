# Windows 专项 CI exact-SHA、receipt 与 action pin 加固修复

- 日期：2026-08-08（Asia/Shanghai）
- 仓库 / 分支：`OpenTake-generation` / `audit/remediation-2026-08-05`
- 实施基线 HEAD：`f9398a9302e57690caa3ea21ff995fdd0ce97706`
- 修复来源：`release-module-tests/windows-ci.md` P0 blockers + Python reviewer follow-up
- 本机裁决：**CI 合约、mutation tests 与静态门禁 PASS；Windows 原生执行待最终提交的 exact-SHA CI**

## 修复结果

### 1. 两个 Windows 专项 job 绑定不可变 SHA

`windows-security` 与 `windows-library-security` 现在和 `windows-product` 使用同一候选 SHA 选择表达式：dispatch 使用必填 `commit_sha`，PR 使用 head SHA，push 使用 `github.sha`。两个 job 均：

1. 拒绝非 40-hex `TARGET_SHA`；
2. 以 `ref: ${{ env.TARGET_SHA }}`、`fetch-depth: 0`、`persist-credentials: false` checkout；
3. 只允许一个 checkout，拒绝 exact checkout 后再插入默认 checkout；
4. 比较 `HEAD`、commit object 与 tree，并要求包含 untracked files 的 clean worktree；
5. 由 checkout assertion 输出规范化 checked SHA；
6. 在 `always()` JSON receipt 中记录 schema、repository、workflow、run ID、attempt、job、event、requested/checked-out SHA、runner OS/arch、关键命令、gate outcome 和 aggregate exit；
7. artifact 名绑定 checked SHA，receipt 缺失或 aggregate/status 非全 success 时 fail closed；
8. 未使用 `continue-on-error`。

原有 Windows 原生覆盖保持：

- `windows-security`：固定 sidecar / FFmpeg cancellation、race-free Job Object containment、Tauri native imports、manifest、native exports、retained-output / reparse tests。
- `windows-library-security`：media library、完整 project、Tauri library tests 与 media clippy。

### 2. 不可分割的 gate / receipt 合约

`scripts/check_windows_product_ci.py` 引入 `@dataclass(frozen=True)` 的 `GateContract` 与 `JobContract`，取消旧 helper 的 14 个参数和 `gate_steps` / `outcome_ids` 平行 tuple。

- 每个 `GateContract` 同时绑定 step name、step id、shell、完整 active step SHA-256、receipt id 和 receipt command。
- 每个 `JobContract` 显式要求恰好 4 个 gate，并拒绝重复 step name/id/receipt id。
- validator 要求 input → unique checkout → bind → 4 个 gates → receipt → upload → enforce 保持顺序。
- receipt 的 `(receipt_id, command, steps.<real-step-id>.outcome)` 必须与 gates 一一、同序、同长绑定。
- 两个专项 job 的 receipt build 与 final enforce 步骤还分别绑定完整步骤的规范化 SHA-256；规范化只统一换行与行尾空白，不删除注释，因此不能用行尾注释伪造 `requested_sha` / `checked_out_sha` 或 aggregate enforcement。
- `Write-Host` / `echo`、注释、字符串诱饵、no-op、step id 替换、outcome 互换、command 漂移和 gate 乱序均被 mutation tests 拒绝。

### 3. CI 全量外部 action 供应链 pin

`.github/workflows/ci.yml` 的 25 处外部 `uses:` 已全部由浮动 major tag 改为经核验的 40-hex commit；Python validator 以单一 `ACTION_PINS` 不可变映射要求全 workflow 精确命中：

| Action | REST-verified commit |
|---|---|
| `actions/checkout` | `11d5960a326750d5838078e36cf38b85af677262` |
| `actions/setup-node` | `49933ea5288caeca8642d1e84afbd3f7d6820020` |
| `actions/cache` | `0057852bfaa89a56745cba8c7296529d2fc39830` |
| `actions/upload-artifact` | `ea165f8d65b6e75b540449e92b4886f43607fa02` |
| `pnpm/action-setup` | `b906affcce14559ad1aafd4ab0e942779e9f58b1` |
| `ruby/setup-ruby` | `95ef2b042f9d7a56d8268cba8559e2842e2ad01b` |

首次只看 `git ls-remote` raw tag 输出时，pnpm annotated tag object 与 peeled commit、以及随后 ruby 输出发生了归属错位。该结果在完成前已停用和纠正；最终 6 个 SHA 均通过各自官方仓库的 `GET /repos/{repo}/commits/{sha}` REST endpoint 返回同 SHA。

validator 提供可重放的在线归属检查：

```text
python3 -B scripts/check_windows_product_ci.py --verify-action-pins-online
External action pin ownership is REST-verified
```

该模式通过已认证 `gh api` 调用 GitHub REST，避免未认证共享 IP rate limit。普通离线 validator 不依赖网络。

离线 action 合约不再用逐行 regex 找 `uses:`。它用 Ruby Psych 从 stdin 解析 workflow 结构，统一处理单/双引号标量，并遍历所有 job-level 和 step-level `uses`：

- 任意未在 `ACTION_PINS` 的 action、非 40-hex 或不匹配所属 action 的 revision 都 fail closed；
- checkout 以 `actions/checkout` repository 身份计数，不依赖行的引号形式；
- 八个 CI jobs 分别绑定精确 checkout 数量和 ref 序列，包括 Windows RED 的 dispatcher/target 两次 checkout；
- 额外的带引号 checkout 或 `ref: main` 均会被 mutation tests 拒绝。

### 4. 单一严格 workflow AST 与全 Windows job 完整性

`scripts/workflow_yaml.py` 现在是 release/Windows 两个 Python validators 共享的唯一 workflow 解析实现。它用 `Psych.parse_stream` 转换单一 YAML 1.2-style AST，显式拒绝：

- 重复 mapping key（包括重复 `windows-security` job）；
- anchor 和 alias；
- 任意 tagged scalar/mapping/sequence；
- 多 document、空 document 或不支持的 YAML node。

Windows validator 仅解析一次，action、checkout、job、step、gate、receipt 和顺序校验全部消费该同一 AST；旧的 regex job slicing / 第二次 Psych 解析路径已删除。C1B validator 在自身语义校验前调用 Windows `--workflow-only` 模式，因此共用同一 duplicate/anchor/alias/tag 裁决，没有第三份 Psych 规则。

5 个 SHA-bound Windows jobs（product、security、library security、safe-filesystem、Windows RED）均由 frozen `ShaBoundJobContract` 绑定完整 step identities/order 和 canonical AST job SHA-256。每个 job 在首个受审 gate 前、所有 gates 后重新校验 checked HEAD、commit/tree、index，并以 `git status --porcelain=v1 --untracked-files=all` 要求完整 worktree 为空；Windows RED 的 pre/post reassert 同时独立检查 dispatcher/target 两个 checkout。bind 后任意 `git checkout/switch/reset/restore/clean/read-tree` 均 fail closed。

为避免合法 gate 输出污染源码完整性判定，Windows security 的 imports、manifest 与 native export probe 全部写入 `$RUNNER_TEMP/windows-security-evidence`，safe-filesystem 的 raw exits、logs 与 JSON receipt 全部写入 `$RUNNER_TEMP/c1b-native-receipt`。product/library receipts 本就在最终 source reassert 之后生成，Windows RED evidence 原本已在 `$RUNNER_TEMP/c1b-red`；没有增加任何路径 allowlist。

### 5. C1B 兼容与普通 CI 自测

- 普通 Linux `rust` job 保留 release workflow contract step，并接入 C1B CI/evidence Ruby 自测。
- `validate-c1b-ci.rb` 的 normal job allowlist 包含现有 `motion-canvas`；checkout/cache/upload 也使用集中 exact-SHA 常量。
- Ruby mutation tests 会拒绝将 C1B safe-filesystem action 降级回 `@v4`。
- evidence synthetic repository 现在同时复制并提交当前 workflow 与 validators，避免“新 validator + 旧 committed workflow”的假失败。

## TDD 记录

### 首轮 RED → GREEN

- 首轮 33 tests 中 14 个 RED：两个专项 job 缺 TARGET_SHA、exact checkout/tree/clean assertion、SHA receipt 和 final aggregate。
- 追加“exact checkout 后插入第二 checkout”的两个 tests 均 RED。
- 两个 Ruby suites 均因 `motion-canvas` allowlist 漂移而 RED。
- 首轮实现后 35/35 Python tests 与两个 Ruby suites GREEN。

### 首次 Reviewer RED → GREEN

在旧 35 项基础上先加 tests：

```text
Ran 46 tests
FAILED (failures=7, errors=4)
```

RED 精确证明旧 validator 可接受命令字符串诱饵、gate id 漂移、receipt command/outcome 错绑、gate 乱序，且尚无 frozen contracts / 全局 action pin 合约。后续 action REST ownership API 的两个 tests 也先因函数不存在而 RED。

最终 GREEN：

```text
python3 -B scripts/check_windows_product_ci.py
# Windows product CI contract is complete

python3 -B -m unittest discover -s scripts -p 'test_check_windows_product_ci.py'
# Ran 48 tests — OK

python3 -B scripts/check_windows_product_ci.py --verify-action-pins-online
# External action pin ownership is REST-verified
```

### 结构化 uses / receipt digest Reviewer RED → GREEN

先扩展到 54 项 tests，未改 validator 时的结果为：

```text
Ran 54 tests
FAILED (failures=6, errors=1)
```

6 个 failures 证明旧 regex/fragment validator 会接受带引号的未批准 action、第二 checkout / `ref: main`，以及 `requested_sha = ('0' * 40) # ...`、checked SHA 和 enforce 注释诱饵；1 个 error 证明 `JobContract` 尚未绑定 receipt/enforce 完整步骤 digest。实现 Psych AST uses 遍历、checkout repository 计数/ref 合约和两类完整步骤 digest 后，54/54 GREEN。

### 单 AST / source mutation Reviewer RED → GREEN

在 54 项基线上先新增两项 mutation：

```text
test_duplicate_windows_security_job_key_is_rejected ... FAIL
test_git_checkout_main_after_security_bind_is_rejected ... FAIL
```

旧 validator 对两个变异均返回空 errors。迁移为共享单 AST、5 个 full-job 合约、pre/post source reassert 和禁止 git source mutation 后，原 54 项保持 GREEN；连同 duplicate、checkout-main、anchor/alias/tag 与 frozen job-contract 断言，最终 58/58 GREEN。

### Untracked source Reviewer RED → GREEN

在 58 项基线上新增两项 TDD mutation。第一项遍历 5 个 SHA-bound jobs 的 pre/post reassert，初始产生 10 个 subtest failures：普通 Windows jobs 缺严格 untracked guard，RED 的 dispatcher/target 也未检查 untracked files。第二项用

```text
printf '%s\n' '?? untracked-security-source.rs' # test -z "$(git status --porcelain=v1 --untracked-files=all)"
```

替换 security bind 的真实 guard，旧 validator 没有返回专门的 untracked-source 错误。实现完整 worktree 空检查、精确 active-code-line 校验、临时证据目录并刷新 full-job/gate/C1B digests 后，Windows Python suites 扩展为 60/60 GREEN；该字符串/注释诱饵被 `complete SHA-bound Windows untracked source guards` 拒绝。

## 本机验证

| 检查 | 结果 |
|---|---|
| Windows product validator | PASS |
| Windows product Python mutation tests | 60/60 PASS |
| Action pin GitHub REST ownership | 6/6 PASS |
| Release workflow validator | PASS |
| Release workflow Python tests | 41/41 PASS |
| `scripts/tests` Python tests | 4/4 PASS |
| C1B CI Ruby mutation tests | PASS |
| C1B evidence Ruby mutation tests | PASS |
| PyYAML duplicate-key rejecting parse | PASS |
| `actionlint v1.7.7` | PASS |
| Python `py_compile` | PASS |
| Ruby syntax | PASS |
| `git diff --check` | PASS |

## 修改文件与边界

- `.github/workflows/ci.yml`
- `scripts/check_windows_product_ci.py`
- `scripts/test_check_windows_product_ci.py`
- `scripts/workflow_yaml.py`
- `scripts/check_release_workflow.py`（仅将既有 Psych parser 提取为共享模块）
- `scripts/validate-c1b-ci.rb`
- `scripts/tests/validate-c1b-ci-test.rb`
- `scripts/tests/validate-c1b-evidence-test.rb`
- 本报告

未修改 `.github/workflows/release.yml`、release 合约规则 / digest、Rust/Web 业务代码或 C1B evidence validator。没有 commit 或 push。

## 仍需最终 exact-SHA Windows CI

本机为 macOS，未执行 Windows-only test image、Job Object、reparse、native import/manifest/export、MSI/NSIS 或 GUI 验收，因此本报告不声明 Windows native PASS。当前工作树也尚不是可 dispatch 的不可变候选 SHA。发布裁决必须在本次修改与相关产品改动形成最终提交后，以该 40-hex SHA 运行 GitHub Actions，并保存 `windows-security-<checked_sha>` 与 `windows-library-security-<checked_sha>` receipts；两个 job 与所有 release gates 成功后，才能将 Windows 原生门禁裁为 PASS。
