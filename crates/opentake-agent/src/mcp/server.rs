//! The rmcp Streamable-HTTP MCP server (`agent-SPEC.md` §8; port of upstream
//! `MCPService` / `MCPHTTPServer`). A thin shim over the transport-free
//! [`Dispatcher`]:
//!
//! - [`McpServer`] implements rmcp's [`ServerHandler`]: `get_info` advertises the
//!   assembled system prompt (base + active workflow plugin) and the tools
//!   capability; `list_tools` returns all [`ToolName`] schemas; `call_tool`
//!   dispatches through the in-process pipeline and converts the result.
//! - [`build_router`] mounts the `StreamableHttpService` at `/mcp` behind a
//!   loopback-only Origin/Host guard (DNS-rebinding defense), plus a minimal
//!   `/.well-known/oauth-protected-resource` so probing clients get a definitive
//!   "no auth" answer.
//! - [`serve`] binds the loopback listener and runs the server.

use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use rmcp::model::{
    CallToolRequestParam, CallToolResult, Implementation, ListToolsResult, PaginatedRequestParam,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Map, Value};

use crate::mcp::convert::to_call_tool_result;
use crate::mcp::core_handle::CoreHandle;
use crate::mcp::dispatch::Dispatcher;
use crate::plugin::registry::PluginRegistry;
use crate::prompt::assemble::assemble_system_prompt;
use crate::tools::descriptions::{description, input_schema};
use crate::tools::names::ToolName;

/// Default loopback bind address for the MCP server (`agent-SPEC.md` §8.4).
pub const DEFAULT_ADDR: &str = "127.0.0.1:19789";

/// One MCP session: owns a [`Dispatcher`] (its own agent-undo stack) and the
/// system-prompt instructions snapshotted at construction.
pub struct McpServer {
    dispatcher: Dispatcher,
    instructions: String,
}

impl McpServer {
    /// Build a session server over the shared document handle + plugin registry.
    pub fn new(handle: Arc<dyn CoreHandle>, registry: Arc<RwLock<PluginRegistry>>) -> Self {
        let instructions = registry
            .read()
            .map(|r| assemble_system_prompt(&r, "default"))
            .unwrap_or_default();
        McpServer {
            dispatcher: Dispatcher::new(handle, registry),
            instructions,
        }
    }

    /// All tool schemas (1:1 with [`ToolName::ALL`]).
    fn tools() -> Vec<Tool> {
        ToolName::ALL
            .iter()
            .map(|&t| {
                let obj = input_schema(t)
                    .as_object()
                    .cloned()
                    .unwrap_or_else(Map::new);
                Tool::new(
                    Cow::Borrowed(t.as_str()),
                    Cow::Borrowed(description(t)),
                    Arc::new(obj),
                )
            })
            .collect()
    }

    /// Dispatch one tool call and convert it to the rmcp result. Split out so it
    /// is unit-testable without constructing a transport `RequestContext`.
    pub fn call(&self, name: &str, arguments: Option<Map<String, Value>>) -> CallToolResult {
        let args = arguments
            .map(Value::Object)
            .unwrap_or(Value::Object(Map::new()));
        to_call_tool_result(self.dispatcher.dispatch(name, args))
    }
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "opentake".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Implementation::default()
            },
            instructions: Some(self.instructions.clone()),
            ..ServerInfo::default()
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: Self::tools(),
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.call(&request.name, request.arguments))
    }
}

// MARK: - Transport (axum + StreamableHttpService)

/// Whether a `Host` / `Origin` header value points at the loopback interface.
/// Accepts `localhost`, `127.0.0.1`, `::1` (optionally bracketed / with a port /
/// with a scheme). An absent header is allowed (native MCP clients often omit
/// `Origin`); a present-but-non-local value is rejected.
fn host_is_local(value: &str) -> bool {
    // Strip an optional scheme (origin form `http://host:port`).
    let after_scheme = value.split("://").last().unwrap_or(value);
    // Strip path/query if any.
    let authority = after_scheme.split('/').next().unwrap_or(after_scheme);
    // Strip port: IPv6 is bracketed `[::1]:port`; IPv4/host is `host:port`.
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest)
    } else {
        authority.split(':').next().unwrap_or(authority)
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

/// Reject requests whose `Host` or `Origin` is not loopback (DNS-rebinding
/// defense for the locally-bound server).
async fn localhost_guard(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let headers = request.headers();
    let host_ok = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(host_is_local)
        .unwrap_or(true);
    let origin_ok = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(host_is_local)
        .unwrap_or(true);
    if host_ok && origin_ok {
        next.run(request).await
    } else {
        (
            axum::http::StatusCode::FORBIDDEN,
            "non-local Origin/Host rejected",
        )
            .into_response()
    }
}

/// Minimal OAuth protected-resource metadata: the server requires no auth (it is
/// loopback-only), so it advertises no authorization servers.
async fn oauth_protected_resource() -> axum::Json<Value> {
    axum::Json(serde_json::json!({
        "resource": "opentake",
        "authorization_servers": [],
    }))
}

/// Build the axum router: `StreamableHttpService` at `/mcp`, the OAuth
/// well-known endpoint, and the loopback guard layered over everything.
pub fn build_router(
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
) -> axum::Router {
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService,
    };

    let service = StreamableHttpService::new(
        move || Ok(McpServer::new(handle.clone(), registry.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    axum::Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            axum::routing::get(oauth_protected_resource),
        )
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn(localhost_guard))
}

/// Bind `addr` (loopback) and serve the MCP router until the process exits.
pub async fn serve(
    addr: SocketAddr,
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
) -> std::io::Result<()> {
    let router = build_router(handle, registry);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("MCP server listening on http://{addr}/mcp");
    axum::serve(listener, router).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::core_handle::CoreHandle;
    use opentake_core::AppCore;
    use opentake_domain::{ClipType, MediaManifest, Timeline};
    use opentake_ops::command::{EditCommand, EditResult};
    use std::path::PathBuf;

    struct TestHandle {
        core: AppCore,
    }
    impl TestHandle {
        fn new() -> Self {
            let core = AppCore::new();
            core.apply(EditCommand::InsertTrack {
                kind: ClipType::Video,
            })
            .unwrap();
            TestHandle { core }
        }
    }
    impl CoreHandle for TestHandle {
        fn timeline(&self) -> Timeline {
            self.core.get_timeline().timeline
        }
        fn media(&self) -> MediaManifest {
            self.core.media()
        }
        fn apply(&self, cmd: EditCommand) -> anyhow::Result<EditResult> {
            self.core.apply(cmd).map_err(|e| anyhow::anyhow!("{e}"))
        }
        fn project_dir(&self) -> Option<PathBuf> {
            self.core.project_dir()
        }
    }

    fn server() -> McpServer {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        McpServer::new(Arc::new(TestHandle::new()), registry)
    }

    #[test]
    fn lists_all_40_tools() {
        assert_eq!(McpServer::tools().len(), ToolName::ALL.len());
        // Names round-trip to the wire names.
        let names: Vec<String> = McpServer::tools()
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        assert!(names.contains(&"add_clips".to_string()));
        assert!(names.contains(&"activate_workflow".to_string()));
    }

    #[test]
    fn get_info_advertises_instructions_and_tools() {
        let s = server();
        let info = s.get_info();
        assert!(info.capabilities.tools.is_some(), "tools capability");
        let instr = info.instructions.unwrap_or_default();
        assert!(!instr.is_empty(), "system prompt instructions present");
    }

    #[test]
    fn call_get_timeline_succeeds() {
        let s = server();
        let res = s.call("get_timeline", None);
        assert_ne!(res.is_error, Some(true), "{res:?}");
        assert!(!res.content.is_empty());
    }

    #[test]
    fn call_unknown_tool_is_error() {
        let s = server();
        let res = s.call("not_a_tool", None);
        assert_eq!(res.is_error, Some(true));
    }

    #[test]
    fn host_guard_accepts_local_rejects_remote() {
        assert!(host_is_local("127.0.0.1:19789"));
        assert!(host_is_local("localhost"));
        assert!(host_is_local("http://127.0.0.1:19789"));
        assert!(host_is_local("[::1]:19789"));
        assert!(!host_is_local("evil.example.com"));
        assert!(!host_is_local("http://attacker.test/mcp"));
    }
}
