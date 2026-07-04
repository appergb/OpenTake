# mcp-server — rmcp MCP server 网络面

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
- **`list_tools`** — 返回全部 44 个工具 schema（`ToolName::ALL`，描述/Schema 来自 [`tools::descriptions`](dispatch-tools.md)）。
- **`call_tool`** — 把工具调用交给 `Dispatcher::dispatch`。因为所有已接线工具是同步的，用 `tokio::task::spawn_blocking` 在阻塞线程池跑，避免堵住 async 运行时；结果经 [`convert::to_call_tool_result`](core-handle-convert.md) 转成 rmcp `CallToolResult`。
- **`call`** — 与 `call_tool` 等价的同步入口，单独拆出以便**不构造传输 `RequestContext`** 就能单测一次工具派发。

### 传输：axum + StreamableHttpService

`build_router` 组装 axum 路由：

- `nest_service("/mcp", StreamableHttpService::new(...))` —— 每次会话用 `McpServer::new(handle, registry)` 新建（`LocalSessionManager` 管理会话）。
- `GET /.well-known/oauth-protected-resource` —— 返回 `{ resource: "opentake", authorization_servers: [] }`，让探测客户端得到明确的"无需鉴权"回答（服务仅回环，故不挂任何授权服务器）。
- 整条路由外层 `from_fn(localhost_guard)`。

`serve(addr, handle, registry)` 绑定回环 `TcpListener` 并 `axum::serve` 到进程退出。`DEFAULT_ADDR = "127.0.0.1:19789"`（端口沿用上游）。

### 回环 Origin/Host 守卫（DNS-rebinding 防御）

`localhost_guard` 中间件检查请求头：

- `Host` 与 `Origin` **若存在**必须指向回环；**缺省即放行**（原生 MCP 客户端常不带 `Origin`）。
- 存在但非回环 → `403 "non-local Origin/Host rejected"`。

`host_is_local` 解析规则：剥协议（`http://host:port` 形式）→ 剥路径/查询 → 剥端口（IPv6 括号形式 `[::1]:port` 单独处理）→ 匹配 `localhost` / `127.0.0.1` / `::1`。这是防 DNS-rebinding 把本地回环服务暴露给 LAN/网页的关键（对应上游 `NWParameters.requiredLocalEndpoint` 锁回环）。

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
- 测试覆盖：列 44 工具、`get_info` 带提示与能力、`get_timeline` 成功、未知工具报错、回环守卫接受本地/拒绝远端。
- 计划中：无独立缺口（依赖的工具 stub 见 [dispatch-tools.md](dispatch-tools.md)）。

---

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
