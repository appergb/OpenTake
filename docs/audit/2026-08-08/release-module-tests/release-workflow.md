# OpenTake 1.0.0-beta.2 GitHub 发布流水线合约验证

- 日期：2026-08-08（Asia/Shanghai）
- 范围：tag 触发的 GitHub Release workflow、静态 fail-closed 合约、普通 CI 接线和发布/回滚说明
- 仓库 / 分支：`OpenTake-generation` / `audit/remediation-2026-08-05`
- 审计起始 HEAD：`f9398a9302e57690caa3ea21ff995fdd0ce97706`
- 本地合约裁决：**PASS**
- 原生发布裁决：**NOT RUN**（本任务明确不创建 tag、不发布 Release）

## 交付内容

- `.github/workflows/release.yml`：新增 `Release` workflow。仅 `push.tags: ['v*']`
  和已存在 tag 的失败重跑 dispatch 可进入；并发组按 tag 隔离且不取消正在运行的同组发布。
- `scripts/check_release_workflow.py`：基于 Ruby Psych AST + YAML 1.2 标量规则的
  unique-key 结构验证器、GraphQL draft 状态解析器和 annotated/lightweight
  远端 tag 解析器，同时检查当前 Cargo/Tauri/Web Beta 2 版本和 release notes。
- `scripts/test_check_release_workflow.py`：41 个合约、状态机和破坏测试。
- `.github/workflows/ci.yml`：仅在 Ubuntu `rust` job 增加发布验证器和其单测的调用。
- `docs/releases/1.0.0-beta.2.md`：补充发布顺序、资产、失败重跑、签名边界和不移动 tag 的回滚规则。

## TDD RED

先只写仓库发布 workflow 存在性合约，在旧状态执行：

```text
python3 -B -m unittest scripts/test_check_release_workflow.py -v
```

结果：exit 1，1 test / 1 failure；失败原因是 `.github/workflows/release.yml`
不存在，断言为 `release.yml must exist`。这是预期 RED，不是测试语法或导入故障。

完整合约落盘后，在实现 workflow 前再执行一次：13 个用例中 12 个因
canonical workflow/变异 fixture 缺失而失败，仅仓库 Beta 2 版本元数据检查通过。

## 审查整改 RED

针对 release review 的 3 个 HIGH 和 1 个 MEDIUM 补充测试后，首先确认了这些
旧行为会被拒绝或缺少保护：REST tag endpoint 无法可靠定位 draft、远端 tag
未在 mutation 前二次绑定、非 publish job 可提升写权限、额外 trigger 和
`if: false` 可绕过必需保护。新增 annotated/lightweight tag 解析测试时，两项
用例在解析器实现前也先 RED。整改过程中复测依次收敛到 4 项、1 项失败，最终
全部 GREEN。

第二轮 Python review 先增加 command-decoy、YAML 重复 key、checkout 顺序/数量和
metadata 异常测试。旧 validator 对行尾注释、echo/字符串诱饵、五个 job 的第二次
checkout 或 assertion 前置 checkout 均未拒绝，非对象 JSON 和 notes I/O 故障还会
抛出 traceback。该轮 RED 为 5 failures + 2 errors；补齐 checkout 子测试后，五个
job 的 duplicate checkout 也分别确认 RED。

第三轮 Python 复审针对剩余两个 HIGH 增加 4 个 mutation：将 quality 或 publish
命令包在 `if false`、在 SHA assertion 后执行 `git checkout main`、以及追加只
`echo clean` 的伪 reassert step。旧 validator 对四项均返回空错误列表，4/4
确认 RED；加入完整 run/job 模板和 source reassert 后全部转 GREEN。

第四轮 exact-SHA 复审为八个 source reassert 分别注入
`?? untracked-release-source.rs`，并把 publish 输出根目录改回工作树。旧合约对
八个 untracked mutation 和一个 publish-root mutation 均未给出对应语义错误，
9/9 子场景确认 RED。整改后所有 reassert 对
`git status --porcelain=v1 --untracked-files=all` 要求严格空输出，publish 的下载、
staging、notes、API 状态和最终复验文件全部位于按 run/attempt 隔离的
`$RUNNER_TEMP` 目录；不再使用 untracked allowlist。

## TDD GREEN 与破坏测试

```text
python3 -B scripts/check_release_workflow.py
python3 -B -m unittest discover -s scripts -p 'test_check_release_workflow.py' -v
```

结果：exit 0；验证器输出 `Release workflow contract is complete`，41/41 测试通过。
每个 mutation 都会被验证器拒绝，覆盖：

1. tag-only trigger 被替换为 branch trigger，或增加 `pull_request`/branches 等额外入口。
2. 移除初始远端 `main` HEAD 绑定，或移除 draft/public 前任一次远端 tag 重绑定。
3. lightweight/annotated tag 的 direct/peeled ref 解析错误或不匹配 source SHA。
4. 绕过 Cargo/Tauri/Web 公开版本比对。
5. publish 移除任一必需 job dependency。
6. DMG/MSI/NSIS/receipt 严格数量被放宽。
7. draft 先于 upload/publication 的约束被取消。
8. draft 使用 REST tag endpoint 查找，或 published 分支可在 asset DELETE 后才失败。
9. `SHA256SUMS` 生成/全量复验改为非 SHA-256。
10. 必需步骤加入 `continue-on-error: true` 或 `if: false`。
11. build job 不再 checkout validate 输出的 exact SHA。
12. 顶层不是 `contents: read`、publish 不是 `contents: write`，或其他 job 提升写权限。
13. 外部 action 未固定到允许列表中的 40-hex commit SHA。
14. 最终 GitHub API 校验不再绑定 target source SHA。
15. canonical workflow 和仓库 Beta 2 版本/release-notes 完整性。
16. YAML 任意层重复 key、alias/tag 或非结构化 job/step。
17. cargo quality、macOS/Windows 打包和 publish 命令被行尾注释、echo、普通字符串
    或 heredoc 字符串伪造。
18. validate/quality/macOS/Windows/publish 任一 job 出现第二次 checkout，或 checkout
    未严格位于 exact-SHA assertion 之前。
19. 非对象 Tauri/Web JSON、非法 metadata 和 release notes I/O 故障产生稳定合约错误。
20. quality 未在 Python validator 前使用完整 commit SHA 固定的 Ruby 3.3。
21. 必需单行 gate 被 shell `if false` 等控制流包裹而实际不执行。
22. SHA assertion 后执行 `checkout/switch/reset/restore/clean/read-tree` 等 Git
    HEAD 或工作树 mutation。
23. reassert step 被删除、移动、增加伪步骤，或任一受审复杂 run/job 内容改变。
24. 任一 reassert 放行源码树 untracked 文件，或 publish 输出根目录回落到工作树。

GraphQL 状态解析器由纯函数驱动并用 missing/draft/published 三种 mock payload
覆盖；published mock 在返回任何可删除 asset 计划前即抛错。另以只读
`gh api graphql` 对公开的 `v1.0.0-beta.1` 验证 GitHub schema：release target
使用 `tagCommit.oid`，draft mutation 使用 numeric release database ID。

workflow 结构解析不依赖 PyYAML。Python 以固定 argv 启动 Ruby Psych，workflow
内容只经 stdin 输入；Psych AST 遍历逐层拒绝重复 key、alias、anchor 和 tag，普通
标量再按 YAML 1.2 的 boolean/null/integer 规则归一化。quality job 在 validator
之前用 40-hex SHA 固定 `ruby/setup-ruby` 3.3；顺序由 mutation test 保护。
所有简单 run 必须与审核模板整个字符串完全一致；复杂 run 使用移除行尾空白并
统一换行后的 SHA-256，五个 job 还分别绑定精确 step identity/order 和完整结构
digest。它们不是从待验证 workflow 动态生成，因此注释、字符串或控制流诱饵都会
改变受审值并 fail closed。

## 兼容回归和格式检查

```text
python3 -B scripts/check_windows_product_ci.py
python3 -B -m unittest discover -s scripts -p 'test_check_windows_product_ci.py' -v
ruby scripts/tests/validate-c1b-ci-test.rb
ruby scripts/tests/validate-c1b-evidence-test.rb
python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v
python3 -m py_compile scripts/check_release_workflow.py scripts/test_check_release_workflow.py
go run github.com/rhysd/actionlint/cmd/actionlint@v1.7.7 \
  .github/workflows/release.yml .github/workflows/ci.yml
git diff --check -- .github/workflows/release.yml .github/workflows/ci.yml \
  scripts/check_release_workflow.py scripts/test_check_release_workflow.py \
  docs/releases/1.0.0-beta.2.md \
  docs/audit/2026-08-08/release-module-tests/release-workflow.md
```

结果：

- Windows product contract：58/58 passed。
- C1B CI/evidence validator：两组测试均输出 `ok`。
- FFmpeg sidecar provisioner 单测：4/4 passed。
- Python compile：exit 0。
- actionlint v1.7.7（通过 `go run` 执行）：`release.yml`、`ci.yml` 均 exit 0。
- `git diff --check`：exit 0。
- 使用 PyYAML 自定义 unique-key loader 解析 `release.yml` 和 `ci.yml`：
  两者均解析成功，无重复 key。

## 工作流安全性质

- validate 把已存在的 SemVer tag、tag commit、checkout HEAD 和远端 `main`
  当前 HEAD 绑定为同一 40-hex SHA，后续每个 job 都按该 SHA exact checkout。
- Ubuntu quality、`macos-14` ARM64 和 `windows-2022` x64 三个 job 并行；
  publish 必须 `needs` 全部成功。
- macOS receipt 记录 source SHA、DMG SHA-256/bytes 和 `ad-hoc` 签名模式；
  Windows receipt 记录同一 source SHA 与 MSI/NSIS 双摘要。
- publish 对三个包+两个 receipt 生成 `SHA256SUMS`，在 draft 上传后先查
  API，公开后再查 API 并下载全部资产复验 checksum。
- publish 在创建/刷新 draft 前和公开前分别重新解析 origin 上的 direct/peeled
  tag ref 并绑定 source SHA；draft 通过 GraphQL 查找，published 状态在任何
  asset DELETE 前 fail closed。
- workflow 顶层只有 `contents: read`，仅 publish job 有 `contents: write`；所有
  外部 action 固定到合约允许的完整 commit SHA。
- assertion 后禁止 Git HEAD/工作树 mutation。validate/quality 在 gate 末，macOS
  和 Windows 在构建前及 packaging smoke 后，publish 在 draft mutation 与公开前，
  都重新核对 HEAD、commit tree、tracked diff/index，并要求包含所有 untracked 文件
  的工作树状态严格为空。publish 的六资产 staging、GraphQL/REST 状态、release notes
  和最终下载复验均写入 `$RUNNER_TEMP`，因此无需工作树 allowlist。
- 不引用生产 secret；不创建/移动 tag；不将 ad-hoc macOS 产物描述为
  Developer ID/已公证，不将 Windows launch smoke 描述为人工 GUI 验收。

## 限制

- 本机没有触发 GitHub-hosted `macos-14` / `windows-2022` job，因此未声称
  DMG/MSI/NSIS 已在本次产出、安装或启动。
- 没有创建/移动 `v1.0.0-beta.2` tag，没有调用 `gh release create/edit/upload`，
  没有向 GitHub 写入任何 Release 或资产。
- 只有对最终位于远端 `main` HEAD 的完整候选 SHA 执行一次真实 tag
  run，才能将“本地合约 PASS”提升为“原生发布 PASS”。
