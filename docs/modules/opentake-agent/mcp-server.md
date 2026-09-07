# mcp-server — rmcp MCP server 网络面

> 状态：draft · 阶段：implementation-backed · 源码同步：2026-09-06。
> 实际桌面认证与候选验证见[当日审计](../../audit/2026-09-06/public-beta-validation.md)。

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：[`../../../crates/opentake-agent/src/mcp/server.rs`](../../../crates/opentake-agent/src/mcp/server.rs)

---

## 职责

把 MCP 协议的网络传输面挂到统一派发壳 [`Dispatcher`](dispatch-tools.md) 之上。它是上游 `MCPService` / `MCPHTTPServer`（NWListener 手写 HTTP）的移植，但传输层换成 **rmcp + axum**——`mixed → needs-replacement`（见 [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md) Agent 段）。它只做"传输 + 安全壳"，不含任何工具逻辑。

## 关键概念

### McpServer：一次 MCP 会话

`McpServer` 实现 rmcp 的 `ServerHandler`，持有一个 `Arc<Dispatcher>`（自带会话级 agent-undo 栈）+ 构造时快照的系统提示 `instructions`。

- **`get_info`** — 广告 `instructions`（base 提示 + 激活插件，构造时由 [`assemble_system_prompt`](prompt.md) 生成）与 tools 能力；`server_info.name = "opentake"`，版本取 `CARGO_PKG_VERSION`。
- **`list_tools`** — 从基础工具集合（`ToolName::ALL`）按当前主机的媒体桥能力过滤，在授权可用时追加 4 个生成工具，并在桌面 Motion 渲染桥可用时追加 add/edit 两工具；描述/Schema 来自 [`tools::descriptions`](dispatch-tools.md)。
- **`call_tool`** — 把工具调用交给 `Dispatcher::dispatch`。因为所有已接线工具是同步的，用 `tokio::task::spawn_blocking` 在阻塞线程池跑，避免堵住 async 运行时；结果经 [`convert::to_call_tool_result`](core-handle-convert.md) 转成 rmcp `CallToolResult`。
- **`call`** — 与 `call_tool` 等价的同步入口，单独拆出以便**不构造传输 `RequestContext`** 就能单测一次工具派发。

### 传输：axum + StreamableHttpService

`build_router` 组装 axum 路由：

- `nest_service("/mcp", StreamableHttpService::new(...))` —— 每次会话用 `McpServer::new(handle, registry)` 新建（`LocalSessionManager` 管理会话）。
- `GET /.well-known/oauth-protected-resource` —— 返回 `{ resource: "opentake", authorization_servers: [] }`，该发现信息不列出 OAuth authorization server；不能由空数组推导为桌面端无需 Bearer 认证。
- 整条路由外层 `from_fn(localhost_guard)`。

`build_router*` 是库层装配面；桌面生产路径使用 gated/authorized transport。官方 Codex 逐轮临时启动认证 listener，Beta 5 外部 MCP 使用显式配对和可撤销凭据。`DEFAULT_ADDR = "127.0.0.1:19789"` 是默认常量，不能等同于默认启动的未认证服务。生命周期见 `src-tauri/src/codex.rs` 与 `src-tauri/src/external_mcp.rs`。

### 回环 Origin/Host 守卫（DNS-rebinding 防御）

`localhost_guard` 中间件检查请求头：

- Host/Origin 的缺省、重复值、authority 与端口按 `localhost_guard`、`host_is_local` 和 `origin_is_local` 的实际规则校验；认证由外层 Bearer authorizer 与工程/请求 gate 完成。
- 存在但非回环 → `403 "non-local Origin/Host rejected"`。

`host_is_local` 解析规则：剥协议（`http://host:port` 形式）→ 剥路径/查询 → 剥端口（IPv6 括号形式 `[::1]:port` 单独处理）→ 匹配 `localhost` / `127.0.0.1` / `::1`。这是回环请求校验的一部分，不能替代 Bearer、工程身份和请求边界检查（对应上游 `NWParameters.requiredLocalEndpoint` 锁回环）。

## 数据流

```
MCP 客户端 → http://127.0.0.1:19789/mcp
  → localhost_guard（Host/Origin 回环校验，否则 403）
  → StreamableHttpService → McpServer::call_tool
  → spawn_blocking(Dispatcher::dispatch)   // 见 dispatch-tools.md
  → convert::to_call_tool_result → CallToolResult → 客户端
```

## 上游对照

| 上游 | 本文件 |
|---|---|
| `MCPHTTPServer / MCPService`（NWListener，仅绑 `127.0.0.1:19789`，手写 HTTP 解析 `/mcp` 与 well-known，SSE GET） | `server.rs`（rmcp `StreamableHttpService` + axum） |
| `NWParameters.requiredLocalEndpoint`（锁回环防 LAN） | `localhost_guard` + `host_is_local` |
| 注册 `ToolDefinitions.all` 工具 + 资源 `palmier://models/*` | `list_tools` 返回 `ToolName::ALL`；模型目录改由 `list_models` 工具暴露（见 [core-handle-convert.md](core-handle-convert.md)） |

端口/绑定/well-known 的行为照搬上游；传输实现换 rmcp，鉴权改为"回环即信任"（上游的 Convex/Clerk 付费代理属闭源云，见 [`../../upstream-analysis/03-闭源云边界.md`](../../upstream-analysis/03-闭源云边界.md)）。

## 完成状态

- 已实现：`McpServer`（`get_info`/`list_tools`/`call_tool`/`call`）、`build_router`、`serve`、回环守卫、OAuth well-known。已被 `src-tauri/src/mcp.rs` 集成（`build_registry` + `server::serve`）。
- 测试覆盖：`list_tools` 列全部可发布工具（默认无媒体桥主机 = `ToolName::ALL` 38 − 7 媒体桥门控 = 31，能力门控后按能力发布）、`get_info` 带提示与能力、`get_timeline` 成功、未知工具报错、回环守卫接受本地/拒绝远端。
- 计划中：无独立缺口（依赖的工具 stub 见 [dispatch-tools.md](dispatch-tools.md)）。

---

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
