# rmcp MCP server：`127.0.0.1:19789` + loopback/Origin 校验（tower layer）

## 1.1 上游真相

上游不是用 SDK 内置 listener，而是「**自定义 TCP 外壳（Apple `Network.framework` `NWListener`）+ SDK 无状态 transport 内核（`StatelessHTTPServerTransport`）**」。关键三点：

1. **只绑 IPv4 回环**：`params.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: 19789)`，注释「never reachable from the LAN」（`MCPHTTPServer.swift:23-27`）。
2. **每条 TCP 连接 = 一对全新 `Server` + `Transport`**（stateless，无跨连接会话）（`MCPService.swift:36-49`）。
3. **连接级校验管线**（`MCPHTTPServer.swift:46-50`）：
   - `OriginValidator.localhost(port: 19789)` —— DNS-rebinding 防护（校验 `Origin`/`Host` ∈ {localhost, 127.0.0.1, [::1]} + 端口）。
   - `ContentTypeValidator()` —— 必须 `application/json`。
   - `ProtocolVersionValidator()` —— 校验 `MCP-Protocol-Version`。
   - （注：上游**省略**了 SDK 默认的 `AcceptHeaderValidator(.jsonOnly)`，放宽 Accept 头。OpenTake 也省略，避免 Claude Desktop shim 的 Accept 头不匹配。）

路由（`MCPHTTPServer.swift:66-91`）：

| 路径 / 方法 | 行为 |
|---|---|
| `POST /mcp` 或 `POST /` | 核心：`transport.handleRequest` → 写回 JSON-RPC 响应 |
| `GET /mcp` / `GET /` | 返回 `text/event-stream` 的 `: connected\n\n`（SSE 占位，无实际推流） |
| `GET /.well-known/oauth-protected-resource` | `{"resource":"http://127.0.0.1:19789"}` |
| 其它 | 404 |

对外发布形态：`Resources/MCPB/palmier-pro.mcpb` 是一个 Node.js stdio→HTTP shim（`mcp-remote → http://127.0.0.1:19789/mcp`），给 Claude Desktop 用。安全边界**仅靠「绑 loopback + Origin 校验」**。

## 1.2 OpenTake Rust 设计（rmcp + axum + tower）

Rust 侧比 Swift 更省事：rmcp 的 `streamable-http-server` feature 自带基于 axum/hyper 的 Streamable HTTP transport，**不必手写 TCP 外壳**。只需把 axum listener 绑死 loopback，并用 `tower::Layer` 补三个校验（rmcp 不内置 Origin 校验）。

```toml
# crates/opentake-agent/Cargo.toml（依赖清单，非完整）
rmcp            = { version = "*", features = ["server", "transport-streamable-http-server"] }
axum            = "*"
tower           = "*"
tower-http      = "*"
reqwest         = { version = "*", features = ["json", "stream"] }
eventsource-stream = "*"   # Anthropic SSE
serde           = { version = "*", features = ["derive"] }
serde_json      = "*"
serde_path_to_error = "*"  # 面向 LLM 的精确路径错误（§4）
schemars        = "*"       # 工具入参 JSON Schema 自动派生（可选，见 §2.3）
tokio           = { version = "*", features = ["full"] }
keyring         = "*"       # OS keychain 存 Anthropic key（§5）
regex           = "*"       # 短 ID UUID 扫描（§3）
```

### 1.2.1 绑定 + 幂等开关（照搬上游语义）

- 端口常量：`pub const MCP_PORT: u16 = 19789;`（沿用，见 ARCHITECTURE §7 与 ROADMAP）。
- 偏好键：`io.opentake.mcp.enabled`，**缺省 true**（对应上游 `io.palmier.pro.mcp.enabled`，`MCPService.swift:11-22`）。落到 Tauri Store / `settings.json`。
- 启动幂等：「已运行则幂等、偏好关闭不启动」（ARCHITECTURE `:1060`）。
- 绑定地址**必须** `SocketAddr` = `127.0.0.1:19789`（`Ipv4Addr::LOCALHOST`）。**禁止** `0.0.0.0`。

```rust
// 伪代码：server 装配
pub struct McpServer { core: Arc<CoreHandle>, signal: Arc<ContextSignalEngine>, plugins: Arc<PluginRegistry> }

pub async fn start(core: Arc<CoreHandle>, /* signal, plugins */) -> anyhow::Result<JoinHandle<()>> {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), MCP_PORT); // 锁回环
    let listener = tokio::net::TcpListener::bind(addr).await?;            // 绑死 loopback
    let mcp_service = StreamableHttpService::new(/* stateless 配置 */ move || ToolServer::new(core.clone()));
    let app = axum::Router::new()
        .nest_service("/mcp", mcp_service)
        .route("/.well-known/oauth-protected-resource", get(well_known)) // {"resource":"http://127.0.0.1:19789"}
        .layer(tower::ServiceBuilder::new()
            .layer(OriginGuardLayer::localhost(MCP_PORT))   // ① DNS-rebinding 防护
            .layer(ContentTypeGuardLayer::json())           // ② application/json
            .layer(ProtocolVersionGuardLayer::new()));      // ③ MCP-Protocol-Version
    Ok(tokio::spawn(async move { axum::serve(listener, app).await.ok(); }))
}
```

### 1.2.2 三个 tower layer（**必须保留，DNS-rebinding 防护**）

`OriginGuardLayer`（对应 `OriginValidator.localhost`）：
- 若请求带 `Origin` 头：解析其 host:port，**必须** host ∈ {`localhost`,`127.0.0.1`,`[::1]`,`::1`} 且 port == 19789（或无端口时按 80/443 拒绝——本地服务只接受显式 19789 或同源无 Origin 的 stdio shim）。不匹配 → `403 Forbidden`。
- 同时校验 `Host` 头同上集合（防 rebinding）。
- 无 `Origin`（典型：stdio→HTTP shim、`claude mcp add` 的本地连接）→ 放行（上游 `OriginValidator.localhost` 同样对无 Origin 宽容；本地回环已是第一道边界）。

`ContentTypeGuardLayer`：仅对 `POST /mcp`，`Content-Type` 必须以 `application/json` 起始，否则 `415`。

`ProtocolVersionGuardLayer`：若带 `MCP-Protocol-Version` 头则校验为受支持版本（rmcp 协商版本集合），不匹配 `400`；缺失则按 rmcp 默认协商。

> 验证（ROADMAP Phase 7 `:59`）：`claude mcp add --transport http http://127.0.0.1:19789/mcp` 能连；从 `192.168.x.x` 或带伪造 `Origin: http://evil.com` 的请求被 403/拒绝。

### 1.2.3 stdio→HTTP shim（Claude Desktop 接入，对外发布形态）

照搬上游 `.mcpb` 思路，打包一个等价 shim（`mcp-remote → http://127.0.0.1:19789/mcp`），由 OpenTake 打包流程产出，或在 Help/Settings 面板给出纯 JSON 手动配置指引（ARCHITECTURE `:992`、`:948`：MCP 接入向导为 4 种客户端拼接 `http://127.0.0.1:19789/mcp` 派生配置）。本 crate 只需保证 HTTP server 行为正确；shim 打包属 packaging 任务。

## 1.3 server 元数据 + 能力（照搬 `MCPService.start()`）

```
name:    "opentake"
version: "1.0.0"
instructions: <见 §6.4 组装后的系统提示词（base + 激活插件的 instructions.md）>
capabilities: { resources: { subscribe:false, listChanged:false },
                tools:     { listChanged:false } }
```

注册：`ListTools` / `CallTool` / `ListResources` / `ReadResource`（`MCPService.swift:77-119`）。

**Resources（2 个，只读，非工具）**（`MCPService.swift:96-133`）：
- `opentake://models/video` → 视频模型目录 JSON（来自 `opentake-gen` 的 `ModelCatalog`）。
- `opentake://models/image` → 图片模型目录 JSON。
（上游 URI 前缀 `palmier://` 改 `opentake://`。）
