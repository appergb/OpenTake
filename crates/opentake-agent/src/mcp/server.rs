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
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, RwLock};

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Implementation, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Map, Value};

use crate::mcp::advanced::AdvancedWorkflowBridge;
use crate::mcp::convert::to_call_tool_result;
use crate::mcp::core_handle::CoreHandle;
use crate::mcp::dispatch::Dispatcher;
use crate::mcp::generation::GenerationBridge;
use crate::mcp::media_bridge::{MediaBridge, MCP_REQUEST_BODY_MAX};
use crate::mcp::motion::MotionBridge;
use crate::plugin::registry::PluginRegistry;
use crate::prompt::assemble::assemble_system_prompt;
use crate::tools::descriptions::{description, input_schema};
use crate::tools::errors::first_non_finite_json_number_path;
#[cfg(test)]
use crate::tools::names::ToolName;
use crate::tools::panic_boundary::with_redacted_dispatch_panic;

/// Default loopback bind address for the MCP server (`agent-SPEC.md` §8.4).
pub const DEFAULT_ADDR: &str = "127.0.0.1:19789";
pub const MCP_PORT: u16 = 19789;

fn map_dispatch_join_error(error: tokio::task::JoinError) -> McpError {
    tracing::error!(
        target: "opentake::mcp::private",
        task_cancelled = error.is_cancelled(),
        task_panic = error.is_panic(),
        "MCP tool dispatch task failed"
    );
    McpError::internal_error("tool dispatch task failed", None)
}

/// One MCP session: owns a [`Dispatcher`] (its own agent-undo stack) and the
/// system-prompt instructions snapshotted at construction.
pub struct McpServer {
    dispatcher: Arc<Dispatcher>,
    instructions: String,
}

impl McpServer {
    /// Build a session server over the shared document handle + plugin registry,
    /// with no media bridge (render/import tools then report "not available").
    pub fn new(handle: Arc<dyn CoreHandle>, registry: Arc<RwLock<PluginRegistry>>) -> Self {
        Self::with_bridge(handle, registry, None)
    }

    /// Build a session server with an optional [`MediaBridge`] injected, so
    /// `inspect_timeline` / `import_media` reach the real GPU + import paths.
    pub fn with_bridge(
        handle: Arc<dyn CoreHandle>,
        registry: Arc<RwLock<PluginRegistry>>,
        bridge: Option<Arc<dyn MediaBridge>>,
    ) -> Self {
        Self::with_bridges(handle, registry, bridge, None)
    }

    pub fn with_bridges(
        handle: Arc<dyn CoreHandle>,
        registry: Arc<RwLock<PluginRegistry>>,
        bridge: Option<Arc<dyn MediaBridge>>,
        generation_bridge: Option<Arc<dyn GenerationBridge>>,
    ) -> Self {
        Self::with_capability_bridges(handle, registry, bridge, generation_bridge, None)
    }

    pub fn with_capability_bridges(
        handle: Arc<dyn CoreHandle>,
        registry: Arc<RwLock<PluginRegistry>>,
        bridge: Option<Arc<dyn MediaBridge>>,
        generation_bridge: Option<Arc<dyn GenerationBridge>>,
        motion_bridge: Option<Arc<dyn MotionBridge>>,
    ) -> Self {
        Self::with_all_capability_bridges(
            handle,
            registry,
            bridge,
            generation_bridge,
            motion_bridge,
            None,
        )
    }

    pub fn with_all_capability_bridges(
        handle: Arc<dyn CoreHandle>,
        registry: Arc<RwLock<PluginRegistry>>,
        bridge: Option<Arc<dyn MediaBridge>>,
        generation_bridge: Option<Arc<dyn GenerationBridge>>,
        motion_bridge: Option<Arc<dyn MotionBridge>>,
        advanced_bridge: Option<Arc<dyn AdvancedWorkflowBridge>>,
    ) -> Self {
        let instructions = registry
            .read()
            .map(|r| assemble_system_prompt(&r, "default"))
            .unwrap_or_default();
        McpServer {
            dispatcher: Arc::new(Dispatcher::with_all_capability_bridges(
                handle,
                registry,
                bridge,
                generation_bridge,
                motion_bridge,
                advanced_bridge,
            )),
            instructions,
        }
    }

    /// Tool schemas for capabilities live in this exact host session.
    fn tools(&self) -> Vec<Tool> {
        self.dispatcher
            .advertised_tools()
            .into_iter()
            .map(|t| {
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
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("opentake", env!("CARGO_PKG_VERSION")))
            .with_instructions(self.instructions.clone())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.tools(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let dispatcher = self.dispatcher.clone();
        let name = request.name.to_string();
        let args = request
            .arguments
            .map(Value::Object)
            .unwrap_or(Value::Object(Map::new()));
        let cancel = opentake_media::MediaCancelToken::new();
        let worker_cancel = cancel.clone();
        // rmcp cancels `context.ct` for the protocol's explicit
        // `notifications/cancelled`. This does not claim raw TCP disconnect
        // detection; it is the MCP cancellation semantic exposed by rmcp.
        let mut worker = tokio::task::spawn_blocking(move || {
            with_redacted_dispatch_panic(|| {
                to_call_tool_result(dispatcher.dispatch_cancellable(&name, args, &worker_cancel))
            })
        });
        let result = tokio::select! {
            result = &mut worker => result,
            () = context.ct.cancelled() => {
                cancel.cancel();
                worker.await
            }
        }
        .map_err(map_dispatch_join_error);
        result
    }
}

// MARK: - Transport (axum + StreamableHttpService)

/// Whether a `Host` header authority points at a loopback interface.
/// The expected listener port is required; malformed ports, userinfo, paths,
/// and suffixes after bracketed IPv6 literals are rejected rather than parsed.
fn host_is_local(value: &str, expected_port: u16) -> bool {
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_ascii_whitespace())
        || value.contains(['/', '?', '#', '@'])
    {
        return false;
    }

    let (host, port) = if let Some(bracketed) = value.strip_prefix('[') {
        let Some(closing) = bracketed.find(']') else {
            return false;
        };
        let suffix = &bracketed[closing + 1..];
        let Some(port) = parse_required_port(suffix) else {
            return false;
        };
        (&bracketed[..closing], port)
    } else {
        match value.matches(':').count() {
            1 => {
                let (host, port) = value.rsplit_once(':').expect("one colon");
                let Some(port) = parse_port(port) else {
                    return false;
                };
                if host.is_empty() {
                    return false;
                }
                (host, port)
            }
            _ => return false,
        }
    };

    port == expected_port
        && (host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback()))
}

fn parse_required_port(suffix: &str) -> Option<u16> {
    suffix.strip_prefix(':').and_then(parse_port)
}

fn parse_port(port: &str) -> Option<u16> {
    (!port.is_empty())
        .then(|| port.parse::<u16>().ok())
        .flatten()
}

/// Whether an `Origin` is an HTTP(S) origin with a loopback authority.
fn origin_is_local(value: &str, expected_port: u16) -> bool {
    let Some((scheme, authority)) = value.split_once("://") else {
        return false;
    };
    matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
        && host_is_local(authority, expected_port)
}

fn local_header_values<F>(
    headers: &axum::http::HeaderMap,
    name: axum::http::header::HeaderName,
    allow_absent: bool,
    validator: F,
) -> bool
where
    F: Fn(&str) -> bool,
{
    let mut values = headers.get_all(name).iter();
    let Some(first) = values.next() else {
        return allow_absent;
    };
    std::iter::once(first)
        .chain(values)
        .all(|value| value.to_str().is_ok_and(&validator))
}

/// Reject requests whose `Host` or `Origin` is not loopback (DNS-rebinding
/// defense for the locally-bound server).
async fn localhost_guard(
    axum::extract::State(expected_port): axum::extract::State<u16>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let headers = request.headers();
    let host_ok = local_header_values(headers, axum::http::header::HOST, false, |value| {
        host_is_local(value, expected_port)
    });
    let origin_ok = local_header_values(headers, axum::http::header::ORIGIN, true, |value| {
        origin_is_local(value, expected_port)
    });
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

/// Reject explicit protocol versions that the linked rmcp SDK cannot serve.
/// Missing versions retain rmcp's backwards-compatible negotiation behavior.
async fn protocol_version_guard(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use rmcp::model::ProtocolVersion;

    if request.uri().path() == "/mcp" {
        let mut versions = request.headers().get_all("mcp-protocol-version").iter();
        if let Some(first) = versions.next() {
            let supported = std::iter::once(first).chain(versions).all(|value| {
                value.to_str().is_ok_and(|version| {
                    ProtocolVersion::KNOWN_VERSIONS
                        .iter()
                        .any(|known| known.as_str() == version)
                })
            });
            if !supported {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    "unsupported MCP-Protocol-Version",
                )
                    .into_response();
            }
        }
    }
    next.run(request).await
}

/// Parse a Content-Type value whose media type must be `application/json`.
fn content_type_is_json(value: &str) -> bool {
    let trimmed = value.trim_end();
    if trimmed.ends_with(';') || has_unquoted_empty_parameter(trimmed) {
        return false;
    }
    value.parse::<mime::Mime>().is_ok_and(|media_type| {
        media_type.type_() == mime::APPLICATION && media_type.subtype() == mime::JSON
    })
}

fn has_unquoted_empty_parameter(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut cursor = 0;
    let mut quoted = false;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' if quoted => cursor += 2,
            b'"' => {
                quoted = !quoted;
                cursor += 1;
            }
            b'=' if !quoted => {
                cursor += 1;
                while bytes
                    .get(cursor)
                    .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
                {
                    cursor += 1;
                }
                if bytes.get(cursor).is_none_or(|byte| *byte == b';') {
                    return true;
                }
            }
            _ => cursor += 1,
        }
    }
    false
}

fn request_content_type_is_json(headers: &axum::http::HeaderMap) -> bool {
    let mut values = headers.get_all(axum::http::header::CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return false;
    };
    values.next().is_none() && value.to_str().is_ok_and(content_type_is_json)
}

/// OpenTake's explicit Content-Type boundary, before body limits and rmcp.
async fn content_type_guard(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    if request.method() == axum::http::Method::POST
        && request.uri().path() == "/mcp"
        && !request_content_type_is_json(request.headers())
    {
        return (
            axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "OpenTake MCP requires a single Content-Type: application/json",
        )
            .into_response();
    }
    next.run(request).await
}

/// Buffer the already bounded MCP request once so non-standard JSON numeric
/// tokens and exponent overflow can be rejected with the tool-relative path
/// before rmcp's JSON decoder loses that context.
async fn finite_number_guard(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    if request.method() != axum::http::Method::POST || request.uri().path() != "/mcp" {
        return next.run(request).await;
    }
    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, MCP_REQUEST_BODY_MAX).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "OpenTake MCP request body is too large",
            )
                .into_response();
        }
    };
    if let Some(path) = first_non_finite_json_number_path(&bytes) {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            format!("{path}: value must be finite"),
        )
            .into_response();
    }
    next.run(axum::http::Request::from_parts(
        parts,
        axum::body::Body::from(bytes),
    ))
    .await
}

/// Minimal OAuth protected-resource metadata: the server requires no auth (it is
/// loopback-only), so it advertises no authorization servers.
async fn oauth_protected_resource() -> axum::Json<Value> {
    axum::Json(serde_json::json!({
        "resource": "opentake",
        "authorization_servers": [],
    }))
}

/// Build the axum router with no media bridge (render/import tools report "not
/// available"). See [`build_router_with_bridge`].
pub fn build_router(
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
) -> axum::Router {
    build_router_with_bridge_for_port(handle, registry, None, MCP_PORT)
}

/// Build a no-bridge router for an explicitly selected loopback listener port.
/// Integration tests and embedders that bind port `0` use the actual bound port.
pub fn build_router_for_port(
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    expected_port: u16,
) -> axum::Router {
    build_router_with_bridge_for_port(handle, registry, None, expected_port)
}

/// Build the axum router: `StreamableHttpService` at `/mcp`, the OAuth
/// well-known endpoint, and the loopback guard layered over everything. The
/// optional [`MediaBridge`] is cloned into each per-session [`McpServer`].
pub fn build_router_with_bridge(
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    bridge: Option<Arc<dyn MediaBridge>>,
) -> axum::Router {
    build_router_with_bridge_for_port(handle, registry, bridge, MCP_PORT)
}

/// Build a bridge-enabled router whose Host/Origin guards expect the supplied
/// listener port. This is the dynamic-port counterpart of
/// [`build_router_with_bridge`].
pub fn build_router_with_bridge_for_port(
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    bridge: Option<Arc<dyn MediaBridge>>,
    expected_port: u16,
) -> axum::Router {
    build_router_with_bridges_for_port(handle, registry, bridge, None, expected_port)
}

pub fn build_router_with_bridges_for_port(
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    bridge: Option<Arc<dyn MediaBridge>>,
    generation_bridge: Option<Arc<dyn GenerationBridge>>,
    expected_port: u16,
) -> axum::Router {
    build_router_with_capability_bridges_for_port(
        handle,
        registry,
        bridge,
        generation_bridge,
        None,
        expected_port,
    )
}

pub fn build_router_with_capability_bridges_for_port(
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    bridge: Option<Arc<dyn MediaBridge>>,
    generation_bridge: Option<Arc<dyn GenerationBridge>>,
    motion_bridge: Option<Arc<dyn MotionBridge>>,
    expected_port: u16,
) -> axum::Router {
    build_router_with_all_capability_bridges_for_port(
        handle,
        registry,
        bridge,
        generation_bridge,
        motion_bridge,
        None,
        expected_port,
    )
}

pub fn build_router_with_all_capability_bridges_for_port(
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    bridge: Option<Arc<dyn MediaBridge>>,
    generation_bridge: Option<Arc<dyn GenerationBridge>>,
    motion_bridge: Option<Arc<dyn MotionBridge>>,
    advanced_bridge: Option<Arc<dyn AdvancedWorkflowBridge>>,
    expected_port: u16,
) -> axum::Router {
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService,
    };
    use tower::ServiceBuilder;
    use tower_http::limit::RequestBodyLimitLayer;

    let service = StreamableHttpService::new(
        move || {
            Ok(McpServer::with_all_capability_bridges(
                handle.clone(),
                registry.clone(),
                bridge.clone(),
                generation_bridge.clone(),
                motion_bridge.clone(),
                advanced_bridge.clone(),
            ))
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let service = ServiceBuilder::new()
        .layer(RequestBodyLimitLayer::new(MCP_REQUEST_BODY_MAX))
        .service(service);

    axum::Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            axum::routing::get(oauth_protected_resource),
        )
        .route_service("/mcp", service)
        .layer(axum::middleware::from_fn(finite_number_guard))
        .layer(axum::middleware::from_fn(content_type_guard))
        .layer(axum::middleware::from_fn(protocol_version_guard))
        .layer(axum::middleware::from_fn_with_state(
            expected_port,
            localhost_guard,
        ))
}

/// Bind `addr` (loopback) and serve the MCP router with no media bridge. See
/// [`serve_with_bridge`].
pub async fn serve(
    addr: SocketAddr,
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
) -> std::io::Result<()> {
    serve_with_bridge(addr, handle, registry, None).await
}

/// Bind `addr` (loopback) and serve the MCP router until the process exits, with
/// an optional [`MediaBridge`] injected (the Tauri shell passes `Some`).
pub async fn serve_with_bridge(
    addr: SocketAddr,
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    bridge: Option<Arc<dyn MediaBridge>>,
) -> std::io::Result<()> {
    serve_with_bridges(addr, handle, registry, bridge, None).await
}

pub async fn serve_with_bridges(
    addr: SocketAddr,
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    bridge: Option<Arc<dyn MediaBridge>>,
    generation_bridge: Option<Arc<dyn GenerationBridge>>,
) -> std::io::Result<()> {
    serve_with_capability_bridges(addr, handle, registry, bridge, generation_bridge, None).await
}

pub async fn serve_with_capability_bridges(
    addr: SocketAddr,
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    bridge: Option<Arc<dyn MediaBridge>>,
    generation_bridge: Option<Arc<dyn GenerationBridge>>,
    motion_bridge: Option<Arc<dyn MotionBridge>>,
) -> std::io::Result<()> {
    serve_with_all_capability_bridges(
        addr,
        handle,
        registry,
        bridge,
        generation_bridge,
        motion_bridge,
        None,
    )
    .await
}

pub async fn serve_with_all_capability_bridges(
    addr: SocketAddr,
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    bridge: Option<Arc<dyn MediaBridge>>,
    generation_bridge: Option<Arc<dyn GenerationBridge>>,
    motion_bridge: Option<Arc<dyn MotionBridge>>,
    advanced_bridge: Option<Arc<dyn AdvancedWorkflowBridge>>,
) -> std::io::Result<()> {
    if !addr.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("MCP server requires a loopback bind address, got {addr}"),
        ));
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound_addr = listener.local_addr()?;
    let router = build_router_with_all_capability_bridges_for_port(
        handle,
        registry,
        bridge,
        generation_bridge,
        motion_bridge,
        advanced_bridge,
        bound_addr.port(),
    );
    tracing::info!("MCP server listening on http://{bound_addr}/mcp");
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
                at: None,
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
    fn lists_every_advertised_tool() {
        let server = server();
        let expected = ToolName::ALL
            .iter()
            .filter(|tool| !tool.requires_media_bridge())
            .count();
        assert_eq!(server.tools().len(), expected);
        // Names round-trip to the wire names.
        let names: Vec<String> = server.tools().iter().map(|t| t.name.to_string()).collect();
        assert!(names.contains(&"add_clips".to_string()));
        assert!(names.contains(&"detect_beats".to_string()));
        assert!(names.contains(&"activate_workflow".to_string()));
        assert!(!names.contains(&"remove_filler_words".to_string()));
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
        let wire = serde_json::to_string(&res).unwrap();
        assert!(wire.contains("MCP_UNKNOWN_TOOL"), "{wire}");
        assert!(wire.contains("Unknown tool name."), "{wire}");
        assert!(!wire.contains("not_a_tool"), "{wire}");
    }

    #[test]
    fn call_invalid_arguments_exposes_typed_safe_preflight_detail() {
        let s = server();
        let args = serde_json::json!({
            "entries": [{
                "mediaRef": "asset",
                "startFrame": "wrong",
                "durationFrames": 30
            }]
        })
        .as_object()
        .cloned();
        let res = s.call("add_clips", args);
        assert_eq!(res.is_error, Some(true));
        let wire = serde_json::to_string(&res).unwrap();
        assert!(wire.contains("MCP_INVALID_ARGUMENTS"), "{wire}");
        assert!(wire.contains("entries[0].startFrame"), "{wire}");
    }

    #[test]
    fn call_dynamic_param_error_never_echoes_caller_owned_key() {
        let s = server();
        let secret = "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890";
        let args = serde_json::json!({
            "clipId": "clip",
            "params": {(secret): []}
        })
        .as_object()
        .cloned();
        let res = s.call("edit_motion_graphic", args);
        let wire = serde_json::to_string(&res).unwrap();
        assert!(wire.contains("MCP_INVALID_ARGUMENTS"), "{wire}");
        assert!(wire.contains("params"), "{wire}");
        assert!(!wire.contains(secret), "{wire}");
    }

    #[tokio::test]
    async fn join_error_does_not_expose_panic_payload() {
        let join = tokio::task::spawn_blocking(|| {
            with_redacted_dispatch_panic(|| {
                panic!("provider panic carried oauth-super-secret-token")
            })
        })
        .await
        .expect_err("worker must panic");
        let error = map_dispatch_join_error(join);
        let wire = serde_json::to_string(&error).unwrap();
        assert!(wire.contains("tool dispatch task failed"));
        assert!(!wire.contains("oauth-super-secret-token"));
    }

    #[test]
    fn host_guard_accepts_local_rejects_remote() {
        assert!(host_is_local("127.0.0.1:19789", MCP_PORT));
        assert!(host_is_local("LOCALHOST:19789", MCP_PORT));
        assert!(host_is_local("127.0.0.2:19789", MCP_PORT));
        assert!(host_is_local("[::1]:19789", MCP_PORT));
        assert!(origin_is_local("http://127.0.0.1:19789", MCP_PORT));
        assert!(origin_is_local("https://LOCALHOST:19789", MCP_PORT));
        assert!(!host_is_local("localhost", MCP_PORT));
        assert!(!host_is_local("127.0.0.1:19790", MCP_PORT));
        assert!(!host_is_local("evil.example.com:19789", MCP_PORT));
        assert!(!host_is_local("[::1].evil:19789", MCP_PORT));
        assert!(!host_is_local("localhost:not-a-port", MCP_PORT));
        assert!(!origin_is_local("http://attacker.test:19789", MCP_PORT));
        assert!(!origin_is_local("http://localhost:19789/mcp", MCP_PORT));
    }

    #[test]
    fn host_is_required_while_origin_may_be_absent() {
        let headers = axum::http::HeaderMap::new();
        assert!(!local_header_values(
            &headers,
            axum::http::header::HOST,
            false,
            |value| host_is_local(value, MCP_PORT)
        ));
        assert!(local_header_values(
            &headers,
            axum::http::header::ORIGIN,
            true,
            |value| origin_is_local(value, MCP_PORT)
        ));
    }

    #[test]
    fn content_type_guard_accepts_one_parseable_json_media_type() {
        let mut headers = axum::http::HeaderMap::new();
        assert!(!request_content_type_is_json(&headers));

        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("APPLICATION/JSON; Charset=utf-8"),
        );
        assert!(request_content_type_is_json(&headers));

        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json; profile=\"a,b\""),
        );
        assert!(request_content_type_is_json(&headers));

        headers.append(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        assert!(!request_content_type_is_json(&headers));

        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_bytes(&[0xff]).unwrap(),
        );
        assert!(!request_content_type_is_json(&headers));

        for invalid in [
            "text/plain",
            "application/json, text/plain",
            "application/json;",
            "application/json; charset",
            "application/json; charset=",
            "application/json; note=\"\"",
            "application/json; =utf-8",
            "application/json; charset=\"unterminated",
        ] {
            headers.insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_str(invalid).unwrap(),
            );
            assert!(!request_content_type_is_json(&headers), "{invalid}");
        }
    }
}
