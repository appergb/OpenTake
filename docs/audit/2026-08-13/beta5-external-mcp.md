# Beta 5 External MCP 持久连接边界审计

日期：2026-08-14

基线：`0506e9c198d8bc193848ad2456559961c57370c2`

环境：macOS 26.6.1（25G76）、Rust/Cargo 1.96.0

## 结论

External MCP 的真实 loopback Streamable HTTP 边界通过了跨进程状态重建与安全矩阵。测试使用正式 `rmcp` 客户端、临时 catalog、每次运行唯一的 macOS Keychain service，以及仅由本次 pairing receipt 推导出的精确 account 清单。测试不会输出 bearer；结束和 panic 清理都只删除这些精确 accounts，正常结束还会逐项回读并确认不存在。

测试目标只在显式启用 `external-mcp-integration` feature 时编译；普通、`--no-default-features` 和发布构建中 `external_mcp` 模块保持私有，且不包含 public harness。进入该测试后仍必须显式设置 `OPENTAKE_RUN_REAL_KEYCHAIN_MCP=1` 才会访问真实 Keychain；未设置时安全跳过真实矩阵。本机已显式 opt in 并实际执行通过。

## 真实矩阵

| 边界 | 实际路径 | 结果 |
|---|---|---|
| 跨重启认证 | 第一个 lifecycle 写入临时 catalog/Keychain 后关闭；新建 lifecycle 使用同一目录和 service；正式 `rmcp` 客户端以原 bearer 重新初始化并列出工具 | 通过 |
| 撤销 | 保留第二个有效客户端和正在监听的 endpoint；撤销第一个客户端；旧 bearer 无法重新建立 `rmcp` 会话，第二个 bearer 仍可列出工具 | 通过 |
| Host / Origin | 对真实 endpoint 发送携带有效 bearer 的边界探针；非 loopback `Host` 与远端 `Origin` 分别返回 HTTP 403 | 通过 |
| 项目切换取消 | 正式 `rmcp` 会话发起阻塞的 `import_media`；并发打开另一个已保存项目；请求被取消且新项目 media 保持为空 | 通过 |
| undo 会话隔离 | 会话 A 创建 folder 后直接确认 folder 存在；会话 B 的 `undo` 被拒绝后 folder 仍存在；会话 A `undo` 后直接确认 folder 消失 | 通过 |
| 固定端口冲突 | 先占用 `127.0.0.1:19789` 再启用 endpoint；状态为 `portConflict`，未选择替代端口；释放后恢复监听 | 通过 |
| disable 关闭 socket | disable 返回后立即在 `127.0.0.1:19789` 重绑 | 通过 |
| 进程退出关闭 socket | 子测试进程在 listener 存活时直接 `process::exit(0)`；父进程等待退出后立即重绑固定端口 | 通过 |

Host/Origin 是刻意使用 `reqwest` 的补充 HTTP 边界探针；所有正常会话、工具发现和工具调用均使用 `rmcp 2.2.0` 的正式 Streamable HTTP client，没有用 raw HTTP 代替协议客户端。

## 凭据泄漏与清理检查

测试在内存中保留每个生成的完整 bearer，仅用于认证和最终比较，不包含在断言消息、测试摘要或审计文档中。完成矩阵后，对以下原始字节逐个扫描每个完整 bearer，要求零匹配：

- 测试矩阵日志；
- 全局 `tracing` subscriber 捕获到的事件及字段；
- 进程退出子测试的 stdout/stderr；
- 临时 `external-mcp/` catalog 目录内全部普通文件。

结果为零匹配。随后只删除 `external-mcp:<client-id>` 形式且由本轮 receipt 记录的 accounts，并通过同一唯一 Keychain service 逐项回读确认 `None`。未使用 service 级或通配符清理。

## TDD 与依赖记录

RED 阶段先创建 integration test；`cargo test -p opentake-tauri --test external_mcp_integration --no-run` 因 `external_mcp` 模块私有、integration harness 缺失且 `rmcp` 未声明而失败。补齐最小 seam 后，`rmcp 2.2.0` 的 reqwest transport 暴露了锁定版 `sse-stream 0.2.3` 的兼容性问题：上游调用 `from_bytes_stream`，而 0.2.3 只有早期拼写 `from_byte_stream`。最终精确锁定 `sse-stream = 0.2.4`；该版本提供兼容 alias，测试恢复编译。

第一次真实运行还暴露了 reqwest 0.13/rustls 未安装进程级 crypto provider 的 panic。integration test 显式安装 `ring` provider 后转绿；没有修改产品 TLS 或 listener 行为。

## API 暴露审查

为了让 crate 外 integration test 组装真实生产 lifecycle，新增了专用 `external-mcp-integration` feature 和带 `required-features` 的测试目标。只有显式启用该 feature 时，`external_mcp` 模块、`ExternalMcpListenerState`、`ExternalMcpIntegrationHarness` 与 receipt 才对 crate 外可见；普通产品构建不编译 harness 或取消 probe。harness 不是 Tauri command，不绕过 bearer、Host/Origin 或 live-project gate，也不直接暴露 catalog/store。

取消 probe 的事件边沿使用 `Notify::notify_one()` 保留 permit，避免 waiter 在读取 atomic flag 与注册 `notified()` 之间丢失通知。一个 2,048 轮并发 signal/wait 回归专门锁定该边界。

## 验证记录

```text
OPENTAKE_RUN_REAL_KEYCHAIN_MCP=1 cargo test -p opentake-tauri --features external-mcp-integration --test external_mcp_integration -- --nocapture
PASS — 2 passed；真实矩阵全部通过；完整 bearer 扫描零匹配

cargo test -p opentake-tauri --features external-mcp-integration --test external_mcp_integration -- --nocapture
PASS — 2 passed；未 opt in 时真实 Keychain 矩阵安全跳过

cargo test -p opentake-tauri --features external-mcp-integration external_mcp::tests::integration_cancel_probe_never_loses_a_concurrent_entry_signal --lib
PASS — 1 passed；2,048 轮并发 signal/wait

cargo test -p opentake-tauri --test external_mcp_integration --no-run
EXPECTED REFUSAL — 目标要求 `external-mcp-integration`

cargo test -p opentake-tauri --no-default-features --test external_mcp_integration --no-run
EXPECTED REFUSAL — 目标要求 `external-mcp-integration`

cargo test -p opentake-tauri external_mcp::tests --lib
PASS — 40 passed

cargo test -p opentake-agent mcp::server::tests -- --nocapture
PASS — 30 passed
```

此外，已分别构建默认与 `--no-default-features` library，并用 `nm`
检查产物；两者均不含 `ExternalMcpIntegrationHarness` 或
`IntegrationCancelProbe` 符号。

本轮 review fix 对 3 个 Task 6 所有的 Rust 文件执行了定向
`rustfmt --check`，结果通过。严格定向 Clippy 仅被同时编辑中的
`src-tauri/src/render.rs` 阻断：默认构建为 `dead_code`，no-default
构建另有 `too_many_arguments`。仅放行这两个与 Task 6 无关的 lint 后，
两个 Task 6 Clippy 目标均通过；父任务会在 composite 收敛后重跑无放行的统一门禁。

严格 Clippy、fmt 与 diff 检查结果见同任务报告。

## 限制

- 测试独占固定端口 `19789`，不能与运行中的 OpenTake External MCP listener 或同一测试的并行实例同时执行。
- 项目切换取消使用 doc-hidden harness 注入的阻塞 media bridge，以确定性观察 request-local cancellation；传输、dispatcher、live-project gate 和取消路径均为真实实现，但不会启动真实解码器或媒体网络读取。
- 进程退出场景证明 listener 存活时 OS 进程终止会释放 socket；它不启动完整 Tauri GUI event loop。`RunEvent::Exit` 主动 shutdown 路径由 lifecycle 单元测试覆盖。
- 独立 security review 已完成；其发现的 public harness 面、取消通知丢失窗口和 live undo 状态证据缺口均已在本轮修复。
