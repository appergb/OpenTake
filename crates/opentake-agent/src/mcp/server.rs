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
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ErrorCode, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Map, Value};
use subtle::ConstantTimeEq;
use tokio_util::sync::CancellationToken;

use crate::chat::ChatTurnGate;
use crate::mcp::advanced::AdvancedWorkflowBridge;
use crate::mcp::convert::to_call_tool_result;
use crate::mcp::core_handle::CoreHandle;
use crate::mcp::dispatch::{dispatch_admission_class, DispatchAdmissionClass, Dispatcher};
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

const MCP_MAX_CONCURRENT_DISPATCHES: usize = 8;

/// Default loopback bind address for the MCP server (`agent-SPEC.md` §8.4).
pub const DEFAULT_ADDR: &str = "127.0.0.1:19789";
pub const MCP_PORT: u16 = 19789;
static NEXT_MCP_UNDO_SCOPE: AtomicU64 = AtomicU64::new(1);

fn next_mcp_undo_scope() -> Arc<str> {
    format!(
        "opentake:mcp:{}:{}",
        std::process::id(),
        NEXT_MCP_UNDO_SCOPE.fetch_add(1, Ordering::Relaxed)
    )
    .into()
}

fn turn_inactive_error() -> McpError {
    McpError::new(
        ErrorCode(-32000),
        "OpenTake turn is no longer active",
        Some(serde_json::json!({
            "code": "OPENTAKE_TURN_CANCELLED"
        })),
    )
}

#[derive(Clone)]
enum DispatchAuthority {
    Direct,
    Gated {
        gate: Arc<dyn ChatTurnGate>,
        activity: Arc<DispatchActivity>,
        undo_scope: Arc<str>,
    },
}

impl DispatchAuthority {
    fn try_enter(
        &self,
        request_cancel: opentake_media::MediaCancelToken,
        client: Option<AuthenticatedMcpClient>,
    ) -> Result<Option<DispatchPermit>, McpError> {
        match self {
            Self::Direct => Ok(None),
            Self::Gated { activity, .. } => activity
                .try_enter(request_cancel, client)
                .map(Some)
                .ok_or_else(turn_inactive_error),
        }
    }

    fn dispatch(
        &self,
        dispatcher: &Dispatcher,
        name: &str,
        args: Value,
        request_cancel: &opentake_media::MediaCancelToken,
    ) -> Option<crate::tools::result::ToolResult> {
        match self {
            Self::Direct => Some(dispatcher.dispatch_cancellable(name, args, request_cancel)),
            Self::Gated {
                gate, undo_scope, ..
            } => {
                gate.dispatch_cancellable_scoped(dispatcher, name, args, undo_scope, request_cancel)
            }
        }
    }

    fn request_cancel(&self) {
        if let Self::Gated { gate, .. } = self {
            gate.request_cancel();
        }
    }
}

struct DispatchActivity {
    state: Mutex<DispatchActivityState>,
    changed: tokio::sync::Notify,
}

struct DispatchActivityState {
    accepting: bool,
    active: usize,
    invalidated: HashSet<AuthenticatedMcpClient>,
    requests: Vec<ActiveDispatch>,
}

struct ActiveDispatch {
    client: Option<AuthenticatedMcpClient>,
    cancel: opentake_media::MediaCancelToken,
}

impl DispatchActivity {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(DispatchActivityState {
                accepting: true,
                active: 0,
                invalidated: HashSet::new(),
                requests: Vec::new(),
            }),
            changed: tokio::sync::Notify::new(),
        })
    }

    fn try_enter(
        self: &Arc<Self>,
        request_cancel: opentake_media::MediaCancelToken,
        client: Option<AuthenticatedMcpClient>,
    ) -> Option<DispatchPermit> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.accepting
            || client
                .as_ref()
                .is_some_and(|client| state.invalidated.contains(client))
        {
            return None;
        }
        state.active = state.active.saturating_add(1);
        state.requests.push(ActiveDispatch {
            client,
            cancel: request_cancel.clone(),
        });
        Some(DispatchPermit {
            activity: self.clone(),
            request_cancel,
        })
    }

    fn stop_accepting(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.accepting = false;
        if state.active == 0 {
            self.changed.notify_one();
        }
    }

    /// Stop admission and synchronously cancel every admitted request-local
    /// worker. This is independent of the host's optional whole-turn
    /// cancellation hook, so managed endpoints drain even for gates that leave
    /// that hook at its no-op default.
    fn stop_and_cancel(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.accepting = false;
        for request in &state.requests {
            request.cancel.cancel();
        }
        if state.active == 0 {
            self.changed.notify_one();
        }
    }

    async fn wait_zero(&self) {
        loop {
            let changed = self.changed.notified();
            let active = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .active;
            if active == 0 {
                return;
            }
            changed.await;
        }
    }

    fn invalidate_client(&self, client: &AuthenticatedMcpClient) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.invalidated.insert(client.clone());
        for request in &state.requests {
            if request.client.as_ref() == Some(client) {
                request.cancel.cancel();
            }
        }
        if !state
            .requests
            .iter()
            .any(|request| request.client.as_ref() == Some(client))
        {
            self.changed.notify_waiters();
        }
    }

    fn restore_client(&self, client: &AuthenticatedMcpClient) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invalidated
            .remove(client);
    }

    async fn wait_client_zero(&self, client: &AuthenticatedMcpClient) {
        loop {
            let changed = self.changed.notified();
            let active = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .requests
                .iter()
                .any(|request| request.client.as_ref() == Some(client));
            if !active {
                return;
            }
            changed.await;
        }
    }

    #[cfg(test)]
    fn active(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
    }
}

struct DispatchPermit {
    activity: Arc<DispatchActivity>,
    request_cancel: opentake_media::MediaCancelToken,
}

impl Drop for DispatchPermit {
    fn drop(&mut self) {
        let mut state = self
            .activity
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.active = state.active.saturating_sub(1);
        if let Some(index) = state
            .requests
            .iter()
            .position(|tracked| tracked.cancel.same_instance(&self.request_cancel))
        {
            state.requests.swap_remove(index);
        }
        self.activity.changed.notify_waiters();
    }
}

#[derive(Clone)]
struct DispatchAdmission {
    total: Arc<tokio::sync::Semaphore>,
    mutation: Arc<tokio::sync::Semaphore>,
}

struct DispatchAdmissionPermit {
    _total: tokio::sync::OwnedSemaphorePermit,
    _mutation: Option<tokio::sync::OwnedSemaphorePermit>,
}

impl DispatchAdmission {
    fn new() -> Self {
        Self::with_total_limit(MCP_MAX_CONCURRENT_DISPATCHES)
    }

    fn with_total_limit(total_limit: usize) -> Self {
        Self {
            total: Arc::new(tokio::sync::Semaphore::new(total_limit.max(1))),
            mutation: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }

    fn try_enter(
        &self,
        class: DispatchAdmissionClass,
    ) -> Result<DispatchAdmissionPermit, McpError> {
        let total = self
            .total
            .clone()
            .try_acquire_owned()
            .map_err(|_| dispatch_busy_error())?;
        let mutation = match class {
            DispatchAdmissionClass::ReadOnly => None,
            DispatchAdmissionClass::Mutation => Some(
                self.mutation
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| dispatch_busy_error())?,
            ),
        };
        Ok(DispatchAdmissionPermit {
            _total: total,
            _mutation: mutation,
        })
    }
}

fn dispatch_busy_error() -> McpError {
    McpError::new(
        ErrorCode(-32001),
        "OpenTake MCP endpoint is busy",
        Some(serde_json::json!({
            "code": "OPENTAKE_MCP_BUSY",
            "retryable": true
        })),
    )
}

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
    authority: DispatchAuthority,
    admission: DispatchAdmission,
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
        Self::with_all_capability_bridges_and_admission(
            handle,
            registry,
            bridge,
            generation_bridge,
            motion_bridge,
            advanced_bridge,
            DispatchAdmission::new(),
        )
    }

    fn with_all_capability_bridges_and_admission(
        handle: Arc<dyn CoreHandle>,
        registry: Arc<RwLock<PluginRegistry>>,
        bridge: Option<Arc<dyn MediaBridge>>,
        generation_bridge: Option<Arc<dyn GenerationBridge>>,
        motion_bridge: Option<Arc<dyn MotionBridge>>,
        advanced_bridge: Option<Arc<dyn AdvancedWorkflowBridge>>,
        admission: DispatchAdmission,
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
            authority: DispatchAuthority::Direct,
            admission,
        }
    }

    fn from_gated_dispatcher(
        dispatcher: Arc<Dispatcher>,
        instructions: String,
        gate: Arc<dyn ChatTurnGate>,
        activity: Arc<DispatchActivity>,
        admission: DispatchAdmission,
    ) -> Self {
        Self {
            dispatcher,
            instructions,
            authority: DispatchAuthority::Gated {
                gate,
                activity,
                undo_scope: next_mcp_undo_scope(),
            },
            admission,
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

    async fn dispatch_tool(
        &self,
        name: String,
        args: Value,
        request_cancelled: CancellationToken,
        client: Option<AuthenticatedMcpClient>,
    ) -> Result<CallToolResult, McpError> {
        if request_cancelled.is_cancelled()
            && matches!(&self.authority, DispatchAuthority::Gated { .. })
        {
            self.authority.request_cancel();
            return Err(turn_inactive_error());
        }
        let admission_permit = self
            .admission
            .try_enter(dispatch_admission_class(&name, &args))?;
        let cancel = opentake_media::MediaCancelToken::new();
        let permit = self.authority.try_enter(cancel.clone(), client)?;
        let dispatcher = self.dispatcher.clone();
        let authority = self.authority.clone();
        let worker_cancel = cancel.clone();
        let worker_authority = authority.clone();
        let mut worker = tokio::task::spawn_blocking(move || {
            let _admission_permit = admission_permit;
            let _permit = permit;
            with_redacted_dispatch_panic(|| {
                worker_authority
                    .dispatch(&dispatcher, &name, args, &worker_cancel)
                    .map(to_call_tool_result)
                    .ok_or_else(turn_inactive_error)
            })
        });
        let joined = tokio::select! {
            result = &mut worker => result,
            () = request_cancelled.cancelled() => {
                cancel.cancel();
                authority.request_cancel();
                worker.await
            }
        }
        .map_err(map_dispatch_join_error)?;
        if request_cancelled.is_cancelled() {
            cancel.cancel();
            authority.request_cancel();
        }
        joined
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
        let name = request.name.to_string();
        let args = request
            .arguments
            .map(Value::Object)
            .unwrap_or(Value::Object(Map::new()));
        // rmcp cancels `context.ct` for the protocol's explicit
        // `notifications/cancelled`. This does not claim raw TCP disconnect
        // detection; it is the MCP cancellation semantic exposed by rmcp.
        let client = context
            .extensions
            .get::<http::request::Parts>()
            .and_then(|parts| parts.extensions.get::<AuthenticatedMcpClient>())
            .cloned();
        self.dispatch_tool(name, args, context.ct, client).await
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

/// The external client authenticated for an MCP HTTP request. Credential
/// generations distinguish a freshly regenerated long-lived credential from a
/// prior credential for the same client identity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AuthenticatedMcpClient {
    pub client_id: Arc<str>,
    pub credential_generation: u64,
}

/// Resolves a syntactically valid bearer candidate without retaining or
/// reporting its secret value. Implementations are responsible for comparing
/// their active credentials in constant time and returning the matching client.
pub trait BearerAuthorizer: Send + Sync {
    fn authorize(&self, token: &str) -> Option<AuthenticatedMcpClient>;
}

struct SingleBearerAuthorizer {
    token: Arc<str>,
    client: AuthenticatedMcpClient,
}

impl SingleBearerAuthorizer {
    fn new(token: Arc<str>) -> Self {
        Self {
            token,
            client: AuthenticatedMcpClient {
                client_id: Arc::from("ephemeral"),
                credential_generation: 0,
            },
        }
    }
}

impl BearerAuthorizer for SingleBearerAuthorizer {
    fn authorize(&self, candidate: &str) -> Option<AuthenticatedMcpClient> {
        (candidate.len() == self.token.len()
            && bool::from(candidate.as_bytes().ct_eq(self.token.as_bytes())))
        .then(|| self.client.clone())
    }
}

fn bearer_candidate(headers: &axum::http::HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(axum::http::header::AUTHORIZATION).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    let Ok(value) = value.to_str() else {
        return None;
    };
    let (scheme, supplied) = value.split_once(' ')?;
    (scheme.eq_ignore_ascii_case("bearer")
        && !supplied.is_empty()
        && !supplied.chars().any(char::is_whitespace))
    .then_some(supplied)
}

fn authentication_required() -> axum::response::Response {
    use axum::response::IntoResponse;

    (
        axum::http::StatusCode::UNAUTHORIZED,
        [(axum::http::header::WWW_AUTHENTICATE, "Bearer")],
        "OpenTake MCP authentication required",
    )
        .into_response()
}

/// Authenticate every route before any MCP session is created. This boundary
/// parses the bearer syntax once, delegates credential matching, and adds only
/// the authenticated public identity to the request extensions.
#[derive(Clone)]
struct ManagedAuthorizationState {
    authorizer: Arc<dyn BearerAuthorizer>,
    sessions: Option<Arc<ManagedClientSessions>>,
}

struct ManagedClientSessions {
    state: Mutex<ManagedClientSessionsState>,
    manager: Arc<rmcp::transport::streamable_http_server::session::local::LocalSessionManager>,
}

#[derive(Default)]
struct ManagedClientSessionsState {
    owners: HashMap<Arc<str>, AuthenticatedMcpClient>,
    invalidated: HashSet<AuthenticatedMcpClient>,
}

impl ManagedClientSessions {
    fn new(
        manager: Arc<rmcp::transport::streamable_http_server::session::local::LocalSessionManager>,
    ) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(ManagedClientSessionsState::default()),
            manager,
        })
    }

    fn permits(&self, session_id: &str, client: &AuthenticatedMcpClient) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        !state.invalidated.contains(client)
            && state
                .owners
                .get(session_id)
                .is_some_and(|owner| owner == client)
    }

    fn bind(&self, session_id: Arc<str>, client: AuthenticatedMcpClient) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.invalidated.contains(&client) {
            return false;
        }
        state.owners.insert(session_id, client);
        true
    }

    fn remove(&self, session_id: &str) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .owners
            .remove(session_id);
    }

    fn invalidate(&self, client: &AuthenticatedMcpClient) -> Vec<Arc<str>> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.invalidated.insert(client.clone());
        let sessions = state
            .owners
            .iter()
            .filter(|(_, owner)| *owner == client)
            .map(|(session, _)| session.clone())
            .collect::<Vec<_>>();
        for session in &sessions {
            state.owners.remove(session);
        }
        sessions
    }

    fn restore(&self, client: &AuthenticatedMcpClient) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invalidated
            .remove(client);
    }

    async fn close(&self, session_id: &Arc<str>) -> Result<(), String> {
        use rmcp::transport::streamable_http_server::session::SessionManager as _;
        self.manager
            .close_session(session_id)
            .await
            .map_err(|error| error.to_string())
    }
}

fn unknown_managed_session() -> axum::response::Response {
    use axum::response::IntoResponse;
    (
        axum::http::StatusCode::NOT_FOUND,
        "Not Found: Session not found",
    )
        .into_response()
}

async fn bearer_authorization_guard(
    axum::extract::State(state): axum::extract::State<ManagedAuthorizationState>,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let Some(client) =
        bearer_candidate(request.headers()).and_then(|token| state.authorizer.authorize(token))
    else {
        return authentication_required();
    };
    let session_id = request
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(Arc::<str>::from);
    if let (Some(sessions), Some(session_id)) = (&state.sessions, &session_id) {
        if !sessions.permits(session_id, &client) {
            return unknown_managed_session();
        }
    }
    let deleting = request.method() == axum::http::Method::DELETE;
    request.extensions_mut().insert(client.clone());
    let response = next.run(request).await;
    let Some(sessions) = state.sessions else {
        return response;
    };
    if let Some(session_id) = session_id {
        if (deleting && response.status().is_success())
            || response.status() == axum::http::StatusCode::NOT_FOUND
        {
            sessions.remove(&session_id);
        }
        return response;
    }
    let Some(session_id) = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(Arc::<str>::from)
    else {
        return response;
    };
    if sessions.bind(session_id.clone(), client) {
        response
    } else {
        let _ = sessions.close(&session_id).await;
        authentication_required()
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

    let admission = DispatchAdmission::new();
    let service = StreamableHttpService::new(
        move || {
            Ok(McpServer::with_all_capability_bridges_and_admission(
                handle.clone(),
                registry.clone(),
                bridge.clone(),
                generation_bridge.clone(),
                motion_bridge.clone(),
                advanced_bridge.clone(),
                admission.clone(),
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

struct GatedRouterTransport {
    shutdown: CancellationToken,
    expected_port: u16,
    authorization: Option<ManagedAuthorizationState>,
}

fn build_gated_router_for_port(
    dispatcher: Arc<Dispatcher>,
    instructions: String,
    gate: Arc<dyn ChatTurnGate>,
    activity: Arc<DispatchActivity>,
    transport: GatedRouterTransport,
) -> axum::Router {
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService,
    };
    use tower::ServiceBuilder;
    use tower_http::limit::RequestBodyLimitLayer;

    let mut config = StreamableHttpServerConfig::default();
    config.cancellation_token = transport.shutdown;
    let admission = DispatchAdmission::new();
    let session_manager = transport.authorization.as_ref().map_or_else(
        || Arc::new(LocalSessionManager::default()),
        |authorization| {
            authorization.sessions.as_ref().map_or_else(
                || Arc::new(LocalSessionManager::default()),
                |sessions| sessions.manager.clone(),
            )
        },
    );
    let service = StreamableHttpService::new(
        move || {
            Ok(McpServer::from_gated_dispatcher(
                dispatcher.clone(),
                instructions.clone(),
                gate.clone(),
                activity.clone(),
                admission.clone(),
            ))
        },
        session_manager,
        config,
    );
    let service = ServiceBuilder::new()
        .layer(RequestBodyLimitLayer::new(MCP_REQUEST_BODY_MAX))
        .service(service);

    let router = axum::Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            axum::routing::get(oauth_protected_resource),
        )
        .route_service("/mcp", service)
        .layer(axum::middleware::from_fn(finite_number_guard))
        .layer(axum::middleware::from_fn(content_type_guard))
        .layer(axum::middleware::from_fn(protocol_version_guard))
        .layer(axum::middleware::from_fn_with_state(
            transport.expected_port,
            localhost_guard,
        ));
    match transport.authorization {
        Some(authorization) => router.layer(axum::middleware::from_fn_with_state(
            authorization,
            bearer_authorization_guard,
        )),
        None => router,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EphemeralMcpError {
    #[error("could not bind the private OpenTake MCP endpoint")]
    Bind(#[source] std::io::Error),
    #[error("the private OpenTake MCP endpoint failed")]
    Serve(#[source] std::io::Error),
    #[error("the private OpenTake MCP endpoint task failed")]
    Join,
    #[error("could not create private OpenTake MCP credentials")]
    Entropy(#[source] getrandom::Error),
}

struct CancelTokenOnDrop(CancellationToken);

impl Drop for CancelTokenOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// A project-authorized MCP endpoint owned by exactly one in-app Agent turn.
/// Call [`Self::close`] before releasing the turn so blocking tool work cannot
/// outlive its project identity.
#[must_use = "the endpoint must be closed before its Agent turn is released"]
pub struct EphemeralMcpEndpoint {
    addr: SocketAddr,
    url: String,
    bearer_token: Arc<str>,
    shutdown: CancellationToken,
    activity: Arc<DispatchActivity>,
    cancel_gate: Arc<dyn ChatTurnGate>,
    stopped: CancellationToken,
    join: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
    closed: bool,
}

impl EphemeralMcpEndpoint {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Per-turn bearer credential. It is intentionally absent from the URL and
    /// has no `Debug` representation; callers should place it only in a child
    /// process environment variable.
    pub fn bearer_token(&self) -> &str {
        &self.bearer_token
    }

    /// Completes if the listener exits before the owner begins normal cleanup.
    pub async fn stopped(&self) {
        self.stopped.cancelled().await;
    }

    /// Stop admission first, terminate transport sessions, then wait for every
    /// blocking dispatcher call before joining the listener task.
    pub async fn close(mut self) -> Result<(), EphemeralMcpError> {
        self.activity.stop_accepting();
        self.shutdown.cancel();
        self.activity.wait_zero().await;
        let result = match self.join.as_mut() {
            Some(join) => match join.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(EphemeralMcpError::Serve(error)),
                Err(error) => {
                    tracing::error!(
                        target: "opentake::mcp::private",
                        task_cancelled = error.is_cancelled(),
                        task_panic = error.is_panic(),
                        "private MCP listener task failed"
                    );
                    Err(EphemeralMcpError::Join)
                }
            },
            None => Err(EphemeralMcpError::Join),
        };
        self.join.take();
        self.closed = true;
        result
    }
}

impl Drop for EphemeralMcpEndpoint {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        self.activity.stop_accepting();
        self.cancel_gate.request_cancel();
        self.shutdown.cancel();
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedMcpError {
    #[error("could not use the managed OpenTake MCP listener")]
    Bind(#[source] std::io::Error),
    #[error("the managed OpenTake MCP endpoint failed")]
    Serve(#[source] std::io::Error),
    #[error("the managed OpenTake MCP endpoint task failed")]
    Join,
}

/// A long-lived, externally authorized MCP endpoint. The authorizer is queried
/// for every request, so credential revocation and regeneration take effect
/// without restarting the listener.
#[must_use = "the endpoint must be shut down and awaited before release"]
pub struct ManagedMcpEndpoint {
    addr: SocketAddr,
    shutdown: CancellationToken,
    activity: Arc<DispatchActivity>,
    cancel_gate: Arc<dyn ChatTurnGate>,
    client_sessions: Arc<ManagedClientSessions>,
    join: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
    closed: bool,
}

impl ManagedMcpEndpoint {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stop admission and transport sessions. Call [`Self::wait`] afterwards to
    /// wait for admitted blocking work and the listener task to finish.
    pub fn shutdown(&self) {
        self.activity.stop_and_cancel();
        self.cancel_gate.request_cancel();
        self.shutdown.cancel();
    }

    /// Invalidate one exact credential generation, terminate only its rmcp
    /// sessions, cancel only its admitted dispatch workers, and await their
    /// drain without interrupting unrelated clients or listener admission.
    pub async fn cancel_client(
        &self,
        client: &AuthenticatedMcpClient,
    ) -> Result<(), ManagedMcpError> {
        self.activity.invalidate_client(client);
        let sessions = self.client_sessions.invalidate(client);
        self.activity.wait_client_zero(client).await;
        for session in sessions {
            self.client_sessions
                .close(&session)
                .await
                .map_err(|error| ManagedMcpError::Serve(std::io::Error::other(error)))?;
        }
        Ok(())
    }

    /// Re-admit a generation when durable credential mutation failed before
    /// publication. Its prior sessions remain terminated; fresh ones may start.
    pub fn restore_client(&self, client: &AuthenticatedMcpClient) {
        self.client_sessions.restore(client);
        self.activity.restore_client(client);
    }

    /// Complete shutdown safely after [`Self::shutdown`] has stopped admission.
    pub async fn wait(mut self) -> Result<(), ManagedMcpError> {
        self.shutdown();
        self.activity.wait_zero().await;
        let result = match self.join.as_mut() {
            Some(join) => match join.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(ManagedMcpError::Serve(error)),
                Err(error) => {
                    tracing::error!(
                        target: "opentake::mcp::private",
                        task_cancelled = error.is_cancelled(),
                        task_panic = error.is_panic(),
                        "managed MCP listener task failed"
                    );
                    Err(ManagedMcpError::Join)
                }
            },
            None => Err(ManagedMcpError::Join),
        };
        self.join.take();
        self.closed = true;
        result
    }
}

impl Drop for ManagedMcpEndpoint {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        self.shutdown();
        if let Some(join) = self.join.take() {
            reap_managed_listener(self.activity.clone(), join);
        }
    }
}

async fn drain_managed_listener(
    activity: Arc<DispatchActivity>,
    join: tokio::task::JoinHandle<std::io::Result<()>>,
) {
    activity.wait_zero().await;
    if let Err(error) = join.await {
        tracing::error!(
            target: "opentake::mcp::private",
            task_cancelled = error.is_cancelled(),
            task_panic = error.is_panic(),
            "managed MCP listener reaper task failed"
        );
    }
}

/// Preserve the managed drain invariant even when its owner is dropped from a
/// synchronous context after the originating Tokio runtime has ended.
fn reap_managed_listener(
    activity: Arc<DispatchActivity>,
    join: tokio::task::JoinHandle<std::io::Result<()>>,
) {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(drain_managed_listener(activity, join));
        }
        Err(_) => {
            let spawn = std::thread::Builder::new()
                .name("opentake-mcp-reaper".to_owned())
                .spawn(move || {
                    match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => runtime.block_on(drain_managed_listener(activity, join)),
                        Err(error) => tracing::error!(
                            target: "opentake::mcp::private",
                            %error,
                            "could not start managed MCP listener reaper runtime"
                        ),
                    }
                });
            if let Err(error) = spawn {
                tracing::error!(
                    target: "opentake::mcp::private",
                    %error,
                    "could not start managed MCP listener reaper thread"
                );
            }
        }
    }
}

/// Serve a long-lived externally authorized MCP endpoint on a caller-bound
/// loopback listener. Passing the listener directly makes bind behavior
/// deterministic for integration tests and lets the Tauri shell own port
/// selection without duplicating transport setup.
pub async fn bind_managed_gated_on(
    listener: tokio::net::TcpListener,
    dispatcher: Arc<Dispatcher>,
    registry: Arc<RwLock<PluginRegistry>>,
    gate: Arc<dyn ChatTurnGate>,
    authorizer: Arc<dyn BearerAuthorizer>,
) -> Result<ManagedMcpEndpoint, ManagedMcpError> {
    let bound_addr = listener.local_addr().map_err(ManagedMcpError::Bind)?;
    if !bound_addr.ip().is_loopback() {
        return Err(ManagedMcpError::Bind(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "managed MCP endpoint requires a loopback address",
        )));
    }
    let instructions = registry
        .read()
        .map(|registry| assemble_system_prompt(&registry, "default"))
        .unwrap_or_default();
    let activity = DispatchActivity::new();
    let shutdown = CancellationToken::new();
    let session_manager = Arc::new(
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager::default(),
    );
    let client_sessions = ManagedClientSessions::new(session_manager);
    let cancel_gate = gate.clone();
    let router = build_gated_router_for_port(
        dispatcher,
        instructions,
        gate,
        activity.clone(),
        GatedRouterTransport {
            shutdown: shutdown.clone(),
            expected_port: bound_addr.port(),
            authorization: Some(ManagedAuthorizationState {
                authorizer,
                sessions: Some(client_sessions.clone()),
            }),
        },
    );
    let listener_shutdown = shutdown.clone();
    let join = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(listener_shutdown.cancelled_owned())
            .await
    });
    Ok(ManagedMcpEndpoint {
        addr: bound_addr,
        shutdown,
        activity,
        cancel_gate,
        client_sessions,
        join: Some(join),
        closed: false,
    })
}

/// Bind a per-turn project-authorized MCP server on a fresh IPv4 loopback port.
pub async fn bind_ephemeral_gated(
    dispatcher: Arc<Dispatcher>,
    registry: Arc<RwLock<PluginRegistry>>,
    gate: Arc<dyn ChatTurnGate>,
) -> Result<EphemeralMcpEndpoint, EphemeralMcpError> {
    bind_ephemeral_gated_on(
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        dispatcher,
        registry,
        gate,
    )
    .await
}

async fn bind_ephemeral_gated_on(
    addr: SocketAddr,
    dispatcher: Arc<Dispatcher>,
    registry: Arc<RwLock<PluginRegistry>>,
    gate: Arc<dyn ChatTurnGate>,
) -> Result<EphemeralMcpEndpoint, EphemeralMcpError> {
    if !addr.ip().is_loopback() {
        return Err(EphemeralMcpError::Bind(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "private MCP endpoint requires a loopback address",
        )));
    }
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(EphemeralMcpError::Bind)?;
    let bound_addr = listener.local_addr().map_err(EphemeralMcpError::Bind)?;
    let mut secret = [0_u8; 32];
    getrandom::fill(&mut secret).map_err(EphemeralMcpError::Entropy)?;
    let mut encoded_secret = String::with_capacity(secret.len() * 2);
    for byte in secret {
        write!(&mut encoded_secret, "{byte:02x}").expect("writing to a String cannot fail");
    }
    let bearer_token: Arc<str> = encoded_secret.into();
    let instructions = registry
        .read()
        .map(|registry| assemble_system_prompt(&registry, "default"))
        .unwrap_or_default();
    let activity = DispatchActivity::new();
    let shutdown = CancellationToken::new();
    let stopped = CancellationToken::new();
    let cancel_gate = gate.clone();
    let router = build_gated_router_for_port(
        dispatcher,
        instructions,
        gate,
        activity.clone(),
        GatedRouterTransport {
            shutdown: shutdown.clone(),
            expected_port: bound_addr.port(),
            authorization: Some(ManagedAuthorizationState {
                authorizer: Arc::new(SingleBearerAuthorizer::new(bearer_token.clone())),
                sessions: None,
            }),
        },
    );
    let listener_shutdown = shutdown.clone();
    let listener_stopped = stopped.clone();
    let join = tokio::spawn(async move {
        let _stopped = CancelTokenOnDrop(listener_stopped);
        axum::serve(listener, router)
            .with_graceful_shutdown(listener_shutdown.cancelled_owned())
            .await
    });
    Ok(EphemeralMcpEndpoint {
        addr: bound_addr,
        url: format!("http://{bound_addr}/mcp"),
        bearer_token,
        shutdown,
        activity,
        cancel_gate,
        stopped,
        join: Some(join),
        closed: false,
    })
}

/// Serve a long-lived loopback MCP endpoint over an already-constructed shared
/// dispatcher. Every call still passes through `gate`; unlike the direct legacy
/// constructors this cannot silently create a second undo/plugin/capability
/// universe beside the in-app Agent.
pub async fn serve_gated_dispatcher(
    addr: SocketAddr,
    dispatcher: Arc<Dispatcher>,
    registry: Arc<RwLock<PluginRegistry>>,
    gate: Arc<dyn ChatTurnGate>,
) -> std::io::Result<()> {
    if !addr.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("MCP server requires a loopback bind address, got {addr}"),
        ));
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound_addr = listener.local_addr()?;
    let instructions = registry
        .read()
        .map(|registry| assemble_system_prompt(&registry, "default"))
        .unwrap_or_default();
    let router = build_gated_router_for_port(
        dispatcher,
        instructions,
        gate,
        DispatchActivity::new(),
        GatedRouterTransport {
            shutdown: CancellationToken::new(),
            expected_port: bound_addr.port(),
            authorization: None,
        },
    );
    tracing::info!("MCP server listening on http://{bound_addr}/mcp");
    axum::serve(listener, router).await
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
    use crate::tools::result::ToolResult;
    use opentake_core::AppCore;
    use opentake_domain::{ClipType, MediaManifest, Timeline};
    use opentake_ops::command::{EditCommand, EditResult};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Condvar;

    struct CatalogMotionDocumentBridge;

    impl crate::mcp::motion_documents::MotionDocumentBridge for CatalogMotionDocumentBridge {
        fn can_edit_motion_documents(&self) -> bool {
            true
        }

        fn admit(
            &self,
            _request: crate::mcp::motion_documents::MotionDocumentRequest,
        ) -> Result<
            Box<dyn crate::mcp::motion_documents::AdmittedMotionDocumentOperation>,
            crate::mcp::motion_documents::MotionDocumentBridgeError,
        > {
            unreachable!("catalog inspection never admits an operation")
        }
    }

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
    fn motion_document_bridge_registers_exact_server_schemas() {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(
            Dispatcher::new(Arc::new(TestHandle::new()), registry)
                .with_motion_document_bridge(Some(Arc::new(CatalogMotionDocumentBridge))),
        );
        let server = McpServer::from_gated_dispatcher(
            dispatcher,
            String::new(),
            Arc::new(CountingGate::new(true)),
            DispatchActivity::new(),
            DispatchAdmission::new(),
        );
        let tools = server.tools();
        for expected in ToolName::MOTION_DOCUMENTS {
            assert!(
                tools
                    .iter()
                    .any(|tool| tool.name.as_ref() == expected.as_str()),
                "missing {}",
                expected.as_str()
            );
        }
        let patch = tools
            .iter()
            .find(|tool| tool.name.as_ref() == "patch_motion_document")
            .expect("patch schema");
        assert_eq!(
            patch.input_schema.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
        assert_eq!(
            patch.input_schema.get("required"),
            Some(&serde_json::json!([
                "documentId",
                "file",
                "baselineHash",
                "edits"
            ]))
        );
    }

    struct CountingGate {
        dispatches: AtomicUsize,
        cancellations: AtomicUsize,
        allow: AtomicBool,
    }

    impl CountingGate {
        fn new(allow: bool) -> Self {
            Self {
                dispatches: AtomicUsize::new(0),
                cancellations: AtomicUsize::new(0),
                allow: AtomicBool::new(allow),
            }
        }
    }

    impl ChatTurnGate for CountingGate {
        fn timeline(&self, dispatcher: &Dispatcher) -> Option<Timeline> {
            Some(dispatcher.timeline())
        }

        fn dispatch(
            &self,
            _dispatcher: &Dispatcher,
            _name: &str,
            _args: Value,
        ) -> Option<ToolResult> {
            self.dispatches.fetch_add(1, Ordering::SeqCst);
            self.allow
                .load(Ordering::SeqCst)
                .then(|| ToolResult::ok("gated"))
        }

        fn request_cancel(&self) {
            self.cancellations.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct BlockingGate {
        entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
        cancellation_seen: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
        released: Mutex<bool>,
        release_changed: Condvar,
    }

    impl BlockingGate {
        fn new(
            entered: tokio::sync::oneshot::Sender<()>,
            cancellation_seen: tokio::sync::oneshot::Sender<()>,
        ) -> Self {
            Self {
                entered: Mutex::new(Some(entered)),
                cancellation_seen: Mutex::new(Some(cancellation_seen)),
                released: Mutex::new(false),
                release_changed: Condvar::new(),
            }
        }

        fn release(&self) {
            *self
                .released
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            self.release_changed.notify_all();
        }
    }

    impl ChatTurnGate for BlockingGate {
        fn timeline(&self, dispatcher: &Dispatcher) -> Option<Timeline> {
            Some(dispatcher.timeline())
        }

        fn dispatch(
            &self,
            _dispatcher: &Dispatcher,
            _name: &str,
            _args: Value,
        ) -> Option<ToolResult> {
            if let Some(entered) = self
                .entered
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                let _ = entered.send(());
            }
            let mut released = self
                .released
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while !*released {
                released = self
                    .release_changed
                    .wait(released)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            Some(ToolResult::ok("released"))
        }

        fn request_cancel(&self) {
            if let Some(seen) = self
                .cancellation_seen
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                let _ = seen.send(());
            }
        }
    }

    struct RequestTokenOnlyGate {
        entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    }

    impl RequestTokenOnlyGate {
        fn new(entered: tokio::sync::oneshot::Sender<()>) -> Self {
            Self {
                entered: Mutex::new(Some(entered)),
            }
        }
    }

    impl ChatTurnGate for RequestTokenOnlyGate {
        fn timeline(&self, dispatcher: &Dispatcher) -> Option<Timeline> {
            Some(dispatcher.timeline())
        }

        fn dispatch(
            &self,
            _dispatcher: &Dispatcher,
            _name: &str,
            _args: Value,
        ) -> Option<ToolResult> {
            panic!("managed requests must use the request-local cancellation path")
        }

        fn dispatch_cancellable(
            &self,
            _dispatcher: &Dispatcher,
            _name: &str,
            _args: Value,
            request_cancel: &opentake_media::MediaCancelToken,
        ) -> Option<ToolResult> {
            if let Some(entered) = self
                .entered
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                let _ = entered.send(());
            }
            while !request_cancel.is_cancelled() {
                std::thread::yield_now();
            }
            Some(ToolResult::ok("request cancelled"))
        }
    }

    struct RecordingBlockingGate {
        blocked_name: Option<&'static str>,
        entered: tokio::sync::mpsc::UnboundedSender<String>,
        released: Mutex<bool>,
        release_changed: Condvar,
    }

    impl RecordingBlockingGate {
        fn new(
            blocked_name: Option<&'static str>,
            entered: tokio::sync::mpsc::UnboundedSender<String>,
        ) -> Self {
            Self {
                blocked_name,
                entered,
                released: Mutex::new(false),
                release_changed: Condvar::new(),
            }
        }

        fn release(&self) {
            *self
                .released
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            self.release_changed.notify_all();
        }
    }

    impl ChatTurnGate for RecordingBlockingGate {
        fn timeline(&self, dispatcher: &Dispatcher) -> Option<Timeline> {
            Some(dispatcher.timeline())
        }

        fn dispatch(
            &self,
            _dispatcher: &Dispatcher,
            name: &str,
            _args: Value,
        ) -> Option<ToolResult> {
            let _ = self.entered.send(name.to_string());
            if self.blocked_name.is_none() || self.blocked_name == Some(name) {
                let mut released = self
                    .released
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                while !*released {
                    released = self
                        .release_changed
                        .wait(released)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
            }
            Some(ToolResult::ok("admitted"))
        }
    }

    fn gated_server(gate: Arc<dyn ChatTurnGate>, activity: Arc<DispatchActivity>) -> McpServer {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(Dispatcher::new(Arc::new(TestHandle::new()), registry));
        McpServer::from_gated_dispatcher(
            dispatcher,
            String::new(),
            gate,
            activity,
            DispatchAdmission::new(),
        )
    }

    fn gated_server_with_shared_admission(
        dispatcher: Arc<Dispatcher>,
        gate: Arc<dyn ChatTurnGate>,
        activity: Arc<DispatchActivity>,
        admission: DispatchAdmission,
    ) -> McpServer {
        McpServer::from_gated_dispatcher(dispatcher, String::new(), gate, activity, admission)
    }

    struct TestBearerAuthorizer {
        credentials: RwLock<Vec<(String, AuthenticatedMcpClient)>>,
    }

    impl TestBearerAuthorizer {
        fn with_credential(token: &str, client_id: &str, credential_generation: u64) -> Self {
            Self {
                credentials: RwLock::new(vec![(
                    (token).to_owned(),
                    AuthenticatedMcpClient {
                        client_id: Arc::from(client_id),
                        credential_generation,
                    },
                )]),
            }
        }

        fn replace_credential(&self, token: &str, client_id: &str, credential_generation: u64) {
            *self
                .credentials
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = vec![(
                token.to_owned(),
                AuthenticatedMcpClient {
                    client_id: Arc::from(client_id),
                    credential_generation,
                },
            )];
        }

        fn revoke_all(&self) {
            self.credentials
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
        }
    }

    impl BearerAuthorizer for TestBearerAuthorizer {
        fn authorize(&self, candidate: &str) -> Option<AuthenticatedMcpClient> {
            self.credentials
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .find_map(|(token, client)| {
                    (token.len() == candidate.len()
                        && bool::from(token.as_bytes().ct_eq(candidate.as_bytes())))
                    .then(|| client.clone())
                })
        }
    }

    #[derive(Clone, Default)]
    struct CapturingSubscriber {
        events: Arc<Mutex<Vec<String>>>,
    }

    impl tracing::Subscriber for CapturingSubscriber {
        fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
            true
        }

        fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }

        fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

        fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

        fn event(&self, event: &tracing::Event<'_>) {
            struct Visitor<'a>(&'a mut String);

            impl tracing::field::Visit for Visitor<'_> {
                fn record_debug(
                    &mut self,
                    field: &tracing::field::Field,
                    value: &dyn std::fmt::Debug,
                ) {
                    let _ = write!(self.0, " {}={value:?}", field.name());
                }
            }

            let mut recorded = event.metadata().name().to_owned();
            event.record(&mut Visitor(&mut recorded));
            self.events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(recorded);
        }

        fn enter(&self, _span: &tracing::span::Id) {}

        fn exit(&self, _span: &tracing::span::Id) {}

        fn register_callsite(
            &self,
            _metadata: &'static tracing::Metadata<'static>,
        ) -> tracing::subscriber::Interest {
            tracing::subscriber::Interest::always()
        }

        fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
            Some(tracing::level_filters::LevelFilter::TRACE)
        }
    }

    fn managed_fixture() -> (
        Arc<Dispatcher>,
        Arc<RwLock<PluginRegistry>>,
        Arc<CountingGate>,
    ) {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(TestHandle::new()),
            registry.clone(),
        ));
        (dispatcher, registry, Arc::new(CountingGate::new(true)))
    }

    fn initialize_body() -> Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "managed-test", "version": "0" }
            }
        })
    }

    async fn bind_managed_test_endpoint(
        authorizer: Arc<dyn BearerAuthorizer>,
    ) -> ManagedMcpEndpoint {
        let (dispatcher, registry, gate) = managed_fixture();
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind managed test listener");
        bind_managed_gated_on(listener, dispatcher, registry, gate, authorizer)
            .await
            .expect("bind managed endpoint")
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

    #[tokio::test]
    async fn gated_dispatch_never_bypasses_gate_and_fails_closed() {
        let gate = Arc::new(CountingGate::new(true));
        let server = gated_server(gate.clone(), DispatchActivity::new());
        let result = server
            .dispatch_tool(
                "get_timeline".into(),
                serde_json::json!({}),
                CancellationToken::new(),
                None,
            )
            .await
            .expect("authorized gate result");
        assert_ne!(result.is_error, Some(true));
        assert_eq!(gate.dispatches.load(Ordering::SeqCst), 1);

        gate.allow.store(false, Ordering::SeqCst);
        let error = server
            .dispatch_tool(
                "get_timeline".into(),
                serde_json::json!({}),
                CancellationToken::new(),
                None,
            )
            .await
            .expect_err("stale gate must fail closed");
        let wire = serde_json::to_string(&error).unwrap();
        assert!(wire.contains("OPENTAKE_TURN_CANCELLED"), "{wire}");
        assert_eq!(gate.dispatches.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn protocol_cancel_requests_whole_turn_and_awaits_worker() {
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
        let gate = Arc::new(BlockingGate::new(entered_tx, cancel_tx));
        let activity = DispatchActivity::new();
        let server = Arc::new(gated_server(gate.clone(), activity.clone()));
        let request_cancel = CancellationToken::new();
        let worker_server = server.clone();
        let worker_cancel = request_cancel.clone();
        let task = tokio::spawn(async move {
            worker_server
                .dispatch_tool(
                    "get_timeline".into(),
                    serde_json::json!({}),
                    worker_cancel,
                    None,
                )
                .await
        });

        entered_rx.await.expect("worker entered the gate");
        assert_eq!(activity.active(), 1);
        request_cancel.cancel();
        cancel_rx.await.expect("whole-turn cancellation requested");
        assert!(!task.is_finished(), "blocking worker must still be awaited");
        assert_eq!(activity.active(), 1);

        gate.release();
        task.await
            .expect("dispatch task joined")
            .expect("tool result");
        assert_eq!(activity.active(), 0);
    }

    #[tokio::test]
    async fn stopping_admission_rejects_new_calls_and_waits_for_active_permit() {
        let activity = DispatchActivity::new();
        let permit = activity
            .try_enter(opentake_media::MediaCancelToken::new(), None)
            .expect("first dispatch admitted");
        activity.stop_accepting();
        assert!(
            activity
                .try_enter(opentake_media::MediaCancelToken::new(), None)
                .is_none(),
            "new dispatch must be rejected"
        );

        let waiter_activity = activity.clone();
        let (drained_tx, mut drained_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            waiter_activity.wait_zero().await;
            let _ = drained_tx.send(());
        });
        assert!(
            matches!(
                drained_rx.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Empty)
            ),
            "drain must wait for the active permit"
        );
        drop(permit);
        drained_rx.await.expect("drain completed after permit drop");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn endpoint_total_admission_caps_concurrent_read_workers() {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(Dispatcher::new(Arc::new(TestHandle::new()), registry));
        let activity = DispatchActivity::new();
        let admission = DispatchAdmission::with_total_limit(2);
        let (entered_tx, mut entered_rx) = tokio::sync::mpsc::unbounded_channel();
        let gate = Arc::new(RecordingBlockingGate::new(None, entered_tx));

        let first = Arc::new(gated_server_with_shared_admission(
            dispatcher.clone(),
            gate.clone(),
            activity.clone(),
            admission.clone(),
        ));
        let second = Arc::new(gated_server_with_shared_admission(
            dispatcher.clone(),
            gate.clone(),
            activity.clone(),
            admission.clone(),
        ));
        let third = gated_server_with_shared_admission(
            dispatcher,
            gate.clone(),
            activity.clone(),
            admission,
        );
        let first_task = tokio::spawn(async move {
            first
                .dispatch_tool(
                    "get_timeline".into(),
                    serde_json::json!({}),
                    CancellationToken::new(),
                    None,
                )
                .await
        });
        let second_task = tokio::spawn(async move {
            second
                .dispatch_tool(
                    "get_media".into(),
                    serde_json::json!({}),
                    CancellationToken::new(),
                    None,
                )
                .await
        });
        let mut entered = vec![
            entered_rx.recv().await.expect("first read entered"),
            entered_rx.recv().await.expect("second read entered"),
        ];
        entered.sort();
        assert_eq!(entered, ["get_media", "get_timeline"]);

        let error = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            third.dispatch_tool(
                "list_folders".into(),
                serde_json::json!({}),
                CancellationToken::new(),
                None,
            ),
        )
        .await
        .expect("over-cap call must fail immediately")
        .expect_err("third read must be rejected while capacity is full");
        let wire = serde_json::to_string(&error).expect("encode busy error");
        assert!(wire.contains("OPENTAKE_MCP_BUSY"), "{wire}");
        assert!(entered_rx.try_recv().is_err(), "busy call reached the gate");
        assert_eq!(activity.active(), 2);

        gate.release();
        first_task
            .await
            .expect("first task joined")
            .expect("first read completed");
        second_task
            .await
            .expect("second task joined")
            .expect("second read completed");
        assert_eq!(activity.active(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn endpoint_serializes_mutations_without_blocking_admitted_reads() {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(Dispatcher::new(Arc::new(TestHandle::new()), registry));
        let activity = DispatchActivity::new();
        let admission = DispatchAdmission::with_total_limit(2);
        let (entered_tx, mut entered_rx) = tokio::sync::mpsc::unbounded_channel();
        let gate = Arc::new(RecordingBlockingGate::new(Some("add_clips"), entered_tx));
        let mutation_server = Arc::new(gated_server_with_shared_admission(
            dispatcher.clone(),
            gate.clone(),
            activity.clone(),
            admission.clone(),
        ));
        let competing_mutation = gated_server_with_shared_admission(
            dispatcher.clone(),
            gate.clone(),
            activity.clone(),
            admission.clone(),
        );
        let read_server = gated_server_with_shared_admission(
            dispatcher,
            gate.clone(),
            activity.clone(),
            admission,
        );

        let mutation_task = tokio::spawn(async move {
            mutation_server
                .dispatch_tool(
                    "add_clips".into(),
                    serde_json::json!({}),
                    CancellationToken::new(),
                    None,
                )
                .await
        });
        assert_eq!(
            entered_rx.recv().await.as_deref(),
            Some("add_clips"),
            "first mutation entered"
        );

        let error = competing_mutation
            .dispatch_tool(
                "remove_clips".into(),
                serde_json::json!({}),
                CancellationToken::new(),
                None,
            )
            .await
            .expect_err("a second mutation must fail busy");
        let wire = serde_json::to_string(&error).expect("encode busy error");
        assert!(wire.contains("OPENTAKE_MCP_BUSY"), "{wire}");
        assert!(entered_rx.try_recv().is_err(), "busy mutation reached gate");

        let read = read_server
            .dispatch_tool(
                "get_timeline".into(),
                serde_json::json!({}),
                CancellationToken::new(),
                None,
            )
            .await
            .expect("read admitted beside mutation");
        assert_ne!(read.is_error, Some(true));
        assert_eq!(entered_rx.recv().await.as_deref(), Some("get_timeline"));

        gate.release();
        mutation_task
            .await
            .expect("mutation task joined")
            .expect("first mutation completed");
        assert_eq!(activity.active(), 0);
    }

    #[tokio::test]
    async fn ephemeral_endpoint_uses_dynamic_port_and_closes_listener() {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(TestHandle::new()),
            registry.clone(),
        ));
        let gate = Arc::new(CountingGate::new(true));
        let endpoint = bind_ephemeral_gated(dispatcher, registry, gate.clone())
            .await
            .expect("bind private endpoint");
        let addr = endpoint.addr();
        assert_ne!(addr.port(), 0);
        assert_eq!(endpoint.url(), format!("http://{addr}/mcp"));
        assert_eq!(endpoint.bearer_token().len(), 64);
        let stream = tokio::net::TcpStream::connect(addr)
            .await
            .expect("listener accepts while active");
        drop(stream);
        endpoint.close().await.expect("close private endpoint");
        assert_eq!(gate.cancellations.load(Ordering::SeqCst), 0);
        assert!(tokio::net::TcpStream::connect(addr).await.is_err());
    }

    #[tokio::test]
    async fn dropping_ephemeral_endpoint_cancels_gate_and_aborts_listener() {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(TestHandle::new()),
            registry.clone(),
        ));
        let gate = Arc::new(CountingGate::new(true));
        let endpoint = bind_ephemeral_gated(dispatcher, registry, gate.clone())
            .await
            .expect("bind private endpoint");
        let addr = endpoint.addr();
        drop(endpoint);

        assert_eq!(gate.cancellations.load(Ordering::SeqCst), 1);
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if tokio::net::TcpStream::connect(addr).await.is_err() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("dropped endpoint listener must terminate");
    }

    #[tokio::test]
    async fn stopped_guard_fires_when_its_listener_task_panics() {
        let stopped = CancellationToken::new();
        let task_token = stopped.clone();
        let task = tokio::spawn(async move {
            let _stopped = CancelTokenOnDrop(task_token);
            panic!("simulated private listener panic");
        });
        assert!(task.await.expect_err("task must panic").is_panic());
        assert!(stopped.is_cancelled());
    }

    #[tokio::test]
    async fn ephemeral_http_route_dispatches_only_through_the_gate() {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(TestHandle::new()),
            registry.clone(),
        ));
        let gate = Arc::new(CountingGate::new(true));
        let endpoint = bind_ephemeral_gated(dispatcher, registry, gate.clone())
            .await
            .expect("bind gated endpoint");
        let client = reqwest::Client::new();
        let initialize_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "gated-test", "version": "0" }
            }
        });
        let missing = client
            .post(endpoint.url())
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body)
            .send()
            .await
            .expect("unauthenticated initialize request");
        assert_eq!(missing.status(), reqwest::StatusCode::UNAUTHORIZED);
        let wrong = client
            .post(endpoint.url())
            .bearer_auth("0".repeat(64))
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body)
            .send()
            .await
            .expect("wrong-token initialize request");
        assert_eq!(wrong.status(), reqwest::StatusCode::UNAUTHORIZED);
        let initialize = client
            .post(endpoint.url())
            .bearer_auth(endpoint.bearer_token())
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body)
            .send()
            .await
            .expect("initialize request");
        assert!(initialize.status().is_success());
        let session = initialize
            .headers()
            .get("mcp-session-id")
            .expect("stateful session")
            .clone();

        let call = |id| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": { "name": "get_timeline", "arguments": {} }
            })
        };
        let allowed = client
            .post(endpoint.url())
            .bearer_auth(endpoint.bearer_token())
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", session.clone())
            .header("mcp-protocol-version", "2025-06-18")
            .json(&call(2))
            .send()
            .await
            .expect("allowed tool call");
        assert!(allowed.status().is_success());
        assert!(allowed.text().await.unwrap().contains("gated"));
        assert_eq!(gate.dispatches.load(Ordering::SeqCst), 1);

        gate.allow.store(false, Ordering::SeqCst);
        let denied = client
            .post(endpoint.url())
            .bearer_auth(endpoint.bearer_token())
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", session)
            .header("mcp-protocol-version", "2025-06-18")
            .json(&call(3))
            .send()
            .await
            .expect("denied tool call");
        assert!(denied.status().is_success());
        assert!(denied
            .text()
            .await
            .unwrap()
            .contains("OPENTAKE_TURN_CANCELLED"));
        assert_eq!(gate.dispatches.load(Ordering::SeqCst), 2);
        endpoint.close().await.expect("close gated endpoint");
    }

    #[tokio::test]
    async fn ephemeral_bearer_token_is_unique_and_expires_with_its_turn() {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(TestHandle::new()),
            registry.clone(),
        ));
        let first = bind_ephemeral_gated(
            dispatcher.clone(),
            registry.clone(),
            Arc::new(CountingGate::new(true)),
        )
        .await
        .expect("bind first endpoint");
        let expired = first.bearer_token().to_owned();
        first.close().await.expect("close first endpoint");

        let second = bind_ephemeral_gated(dispatcher, registry, Arc::new(CountingGate::new(true)))
            .await
            .expect("bind second endpoint");
        assert_ne!(expired, second.bearer_token());
        let response = reqwest::Client::new()
            .post(second.url())
            .bearer_auth(expired)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": "expired-test", "version": "0" }
                }
            }))
            .send()
            .await
            .expect("send expired credential");
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
        second.close().await.expect("close second endpoint");
    }

    #[tokio::test]
    async fn managed_authentication_failures_have_one_redacted_public_shape() {
        let valid = "managed-valid-credential";
        let wrong = "managed-wrong-credential";
        let authorizer = Arc::new(TestBearerAuthorizer::with_credential(valid, "external", 7));
        let endpoint = bind_managed_test_endpoint(authorizer.clone()).await;
        let client = reqwest::Client::new();

        let missing = client
            .post(format!("http://{}/mcp", endpoint.addr()))
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("send missing credential");
        let wrong = client
            .post(format!("http://{}/mcp", endpoint.addr()))
            .bearer_auth(wrong)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("send wrong credential");
        let malformed = client
            .post(format!("http://{}/mcp", endpoint.addr()))
            .header("authorization", format!("Bearer {valid} extra"))
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("send malformed credential");
        authorizer.revoke_all();
        let revoked = client
            .post(format!("http://{}/mcp", endpoint.addr()))
            .bearer_auth(valid)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("send revoked credential");

        let mut shapes = Vec::new();
        for response in [missing, wrong, malformed, revoked] {
            let status = response.status();
            let www_authenticate = response.headers().get("www-authenticate").cloned();
            let body = response.text().await.expect("read authentication response");
            assert!(
                !body.contains(valid),
                "authentication response leaked valid credential"
            );
            assert!(
                !body.contains("managed-wrong-credential"),
                "authentication response leaked candidate credential"
            );
            shapes.push((status, www_authenticate, body));
        }
        assert!(shapes.iter().all(|shape| shape == &shapes[0]));
        assert_eq!(shapes[0].0, reqwest::StatusCode::UNAUTHORIZED);

        endpoint.shutdown();
        endpoint.wait().await.expect("stop managed endpoint");
    }

    #[tokio::test]
    async fn managed_authorizer_observes_regeneration_before_new_initialize() {
        let old_token = "managed-generation-one";
        let new_token = "managed-generation-two";
        let authorizer = Arc::new(TestBearerAuthorizer::with_credential(
            old_token, "external", 1,
        ));
        let endpoint = bind_managed_test_endpoint(authorizer.clone()).await;
        let client = reqwest::Client::new();
        authorizer.replace_credential(new_token, "external", 2);

        let old = client
            .post(format!("http://{}/mcp", endpoint.addr()))
            .bearer_auth(old_token)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("send old credential");
        assert_eq!(old.status(), reqwest::StatusCode::UNAUTHORIZED);

        let regenerated = client
            .post(format!("http://{}/mcp", endpoint.addr()))
            .bearer_auth(new_token)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("send regenerated credential");
        assert!(regenerated.status().is_success());

        endpoint.shutdown();
        endpoint.wait().await.expect("stop managed endpoint");
    }

    #[tokio::test]
    async fn managed_session_is_owned_by_the_authenticated_client_generation() {
        let authorizer = Arc::new(TestBearerAuthorizer {
            credentials: RwLock::new(vec![
                (
                    "client-a-token".to_owned(),
                    AuthenticatedMcpClient {
                        client_id: Arc::from("client-a"),
                        credential_generation: 1,
                    },
                ),
                (
                    "client-b-token".to_owned(),
                    AuthenticatedMcpClient {
                        client_id: Arc::from("client-b"),
                        credential_generation: 1,
                    },
                ),
            ]),
        });
        let endpoint = bind_managed_test_endpoint(authorizer).await;
        let client = reqwest::Client::new();
        let url = format!("http://{}/mcp", endpoint.addr());
        let initialized = client
            .post(&url)
            .bearer_auth("client-a-token")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("initialize client A session");
        let session = initialized
            .headers()
            .get("mcp-session-id")
            .expect("client A session id")
            .clone();

        let foreign = client
            .post(&url)
            .bearer_auth("client-b-token")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", session.clone())
            .header("mcp-protocol-version", "2025-06-18")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }))
            .send()
            .await
            .expect("attempt foreign session reuse");
        assert_eq!(foreign.status(), reqwest::StatusCode::NOT_FOUND);

        let owner = client
            .post(&url)
            .bearer_auth("client-a-token")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", session)
            .header("mcp-protocol-version", "2025-06-18")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/list",
                "params": {}
            }))
            .send()
            .await
            .expect("use owned session");
        assert!(owner.status().is_success());

        endpoint.shutdown();
        endpoint.wait().await.expect("stop managed endpoint");
    }

    #[tokio::test]
    async fn managed_cancel_client_terminates_only_target_sessions() {
        let authorizer = Arc::new(TestBearerAuthorizer {
            credentials: RwLock::new(vec![
                (
                    "target-token".to_owned(),
                    AuthenticatedMcpClient {
                        client_id: Arc::from("target"),
                        credential_generation: 1,
                    },
                ),
                (
                    "survivor-token".to_owned(),
                    AuthenticatedMcpClient {
                        client_id: Arc::from("survivor"),
                        credential_generation: 1,
                    },
                ),
            ]),
        });
        let endpoint = bind_managed_test_endpoint(authorizer).await;
        let client = reqwest::Client::new();
        let url = format!("http://{}/mcp", endpoint.addr());
        let mut sessions = Vec::new();
        for token in ["target-token", "survivor-token"] {
            let response = client
                .post(&url)
                .bearer_auth(token)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .json(&initialize_body())
                .send()
                .await
                .expect("initialize managed session");
            sessions.push(
                response
                    .headers()
                    .get("mcp-session-id")
                    .expect("managed session id")
                    .clone(),
            );
        }

        endpoint
            .cancel_client(&AuthenticatedMcpClient {
                client_id: Arc::from("target"),
                credential_generation: 1,
            })
            .await
            .expect("cancel target client");

        for (token, session, expected) in [
            ("target-token", &sessions[0], reqwest::StatusCode::NOT_FOUND),
            ("survivor-token", &sessions[1], reqwest::StatusCode::OK),
        ] {
            let response = client
                .post(&url)
                .bearer_auth(token)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-session-id", session.clone())
                .header("mcp-protocol-version", "2025-06-18")
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/list",
                    "params": {}
                }))
                .send()
                .await
                .expect("use managed session after selective cancellation");
            assert_eq!(response.status(), expected);
        }

        endpoint.shutdown();
        endpoint.wait().await.expect("stop managed endpoint");
    }

    #[tokio::test]
    async fn managed_endpoint_keeps_loopback_origin_and_host_guards() {
        let token = "managed-loopback-credential";
        let endpoint = bind_managed_test_endpoint(Arc::new(TestBearerAuthorizer::with_credential(
            token, "external", 1,
        )))
        .await;
        let client = reqwest::Client::new();
        let url = format!("http://{}/mcp", endpoint.addr());

        let remote_host = client
            .post(&url)
            .bearer_auth(token)
            .header(
                "host",
                format!("attacker.example:{}", endpoint.addr().port()),
            )
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("send remote host");
        assert_eq!(remote_host.status(), reqwest::StatusCode::FORBIDDEN);

        let remote_origin = client
            .post(&url)
            .bearer_auth(token)
            .header(
                "origin",
                format!("http://attacker.example:{}", endpoint.addr().port()),
            )
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("send remote origin");
        assert_eq!(remote_origin.status(), reqwest::StatusCode::FORBIDDEN);

        let loopback_origin = client
            .post(&url)
            .bearer_auth(token)
            .header(
                "origin",
                format!("http://127.0.0.1:{}", endpoint.addr().port()),
            )
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("send loopback origin");
        assert!(loopback_origin.status().is_success());

        endpoint.shutdown();
        endpoint.wait().await.expect("stop managed endpoint");
    }

    #[tokio::test]
    async fn managed_shutdown_stops_listener_admission_and_workers() {
        let token = "managed-shutdown-credential";
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(TestHandle::new()),
            registry.clone(),
        ));
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
        let gate = Arc::new(BlockingGate::new(entered_tx, cancel_tx));
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind managed shutdown listener");
        let endpoint = bind_managed_gated_on(
            listener,
            dispatcher,
            registry,
            gate.clone(),
            Arc::new(TestBearerAuthorizer::with_credential(token, "external", 1)),
        )
        .await
        .expect("bind managed endpoint");
        let addr = endpoint.addr();
        let client = reqwest::Client::new();
        let url = format!("http://{addr}/mcp");
        let initialized = client
            .post(&url)
            .bearer_auth(token)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("initialize managed session");
        let session = initialized
            .headers()
            .get("mcp-session-id")
            .expect("stateful managed session")
            .clone();
        let call_client = client.clone();
        let call_url = url.clone();
        let call = tokio::spawn(async move {
            call_client
                .post(call_url)
                .bearer_auth(token)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-session-id", session)
                .header("mcp-protocol-version", "2025-06-18")
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": { "name": "get_timeline", "arguments": {} }
                }))
                .send()
                .await
        });
        entered_rx
            .await
            .expect("blocking managed worker entered gate");
        endpoint.shutdown();
        cancel_rx
            .await
            .expect("managed shutdown requested gate cancellation");
        let mut stopped = tokio::spawn(async move { endpoint.wait().await });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut stopped)
                .await
                .is_err(),
            "managed shutdown returned before its admitted worker finished"
        );
        gate.release();
        let _ = call.await.expect("managed call joined");
        stopped
            .await
            .expect("managed endpoint task joined")
            .expect("stop managed endpoint");
        assert!(tokio::net::TcpStream::connect(addr).await.is_err());
    }

    #[tokio::test]
    async fn managed_shutdown_cancels_request_local_workers_when_gate_cancel_is_noop() {
        let token = "managed-request-token-credential";
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(TestHandle::new()),
            registry.clone(),
        ));
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let gate = Arc::new(RequestTokenOnlyGate::new(entered_tx));
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind request-token managed listener");
        let endpoint = bind_managed_gated_on(
            listener,
            dispatcher,
            registry,
            gate,
            Arc::new(TestBearerAuthorizer::with_credential(token, "external", 1)),
        )
        .await
        .expect("bind managed endpoint");
        let client = reqwest::Client::new();
        let url = format!("http://{}/mcp", endpoint.addr());
        let initialized = client
            .post(&url)
            .bearer_auth(token)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .send()
            .await
            .expect("initialize request-token session");
        let session = initialized
            .headers()
            .get("mcp-session-id")
            .expect("stateful request-token session")
            .clone();
        let call_client = client.clone();
        let call_url = url.clone();
        let call = tokio::spawn(async move {
            call_client
                .post(call_url)
                .bearer_auth(token)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-session-id", session)
                .header("mcp-protocol-version", "2025-06-18")
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": { "name": "get_timeline", "arguments": {} }
                }))
                .send()
                .await
        });
        entered_rx
            .await
            .expect("request-local worker entered no-op gate");

        endpoint.shutdown();
        tokio::time::timeout(std::time::Duration::from_secs(5), endpoint.wait())
            .await
            .expect("managed shutdown must not wait for a no-op gate")
            .expect("managed endpoint stopped");
        let response = call
            .await
            .expect("request-local call joined")
            .expect("request-local call completed");
        assert!(response.status().is_success());
    }

    #[test]
    fn dropping_managed_endpoint_after_its_runtime_stops_does_not_panic() {
        let endpoint = {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build endpoint runtime");
            runtime.block_on(async {
                bind_managed_test_endpoint(Arc::new(TestBearerAuthorizer::with_credential(
                    "managed-drop-credential",
                    "external",
                    1,
                )))
                .await
            })
        };

        drop(endpoint);
    }

    #[tokio::test]
    async fn managed_authorization_attaches_client_identity_to_request_extensions() {
        use axum::extract::Extension;
        use tower::ServiceExt as _;

        async fn identity(Extension(client): Extension<AuthenticatedMcpClient>) -> String {
            format!("{}:{}", client.client_id, client.credential_generation)
        }

        let router = axum::Router::new()
            .route("/identity", axum::routing::get(identity))
            .layer(axum::middleware::from_fn_with_state(
                ManagedAuthorizationState {
                    authorizer: Arc::new(TestBearerAuthorizer::with_credential(
                        "extension-token",
                        "external",
                        9,
                    )),
                    sessions: None,
                },
                bearer_authorization_guard,
            ));
        let response = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/identity")
                    .header("authorization", "Bearer extension-token")
                    .body(axum::body::Body::empty())
                    .expect("identity request"),
            )
            .await
            .expect("identity response");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .expect("read identity response");
        assert_eq!(&body[..], b"external:9");
    }

    #[test]
    fn bearer_authorization_never_records_candidate_tokens() {
        use tower::ServiceExt as _;

        let router = axum::Router::new()
            .route("/", axum::routing::get(|| async { "authorized" }))
            .layer(axum::middleware::from_fn_with_state(
                ManagedAuthorizationState {
                    authorizer: Arc::new(TestBearerAuthorizer::with_credential(
                        "active-token",
                        "external",
                        1,
                    )),
                    sessions: None,
                },
                bearer_authorization_guard,
            ));
        let subscriber = CapturingSubscriber::default();
        let events = subscriber.events.clone();
        let dispatch = tracing::Dispatch::new(subscriber);
        tracing::dispatcher::with_default(&dispatch, || {
            for candidate in ["wrong-candidate-a", "wrong-candidate-b"] {
                let response = futures::executor::block_on(
                    router.clone().oneshot(
                        axum::http::Request::builder()
                            .uri("/")
                            .header("authorization", format!("Bearer {candidate}"))
                            .body(axum::body::Body::empty())
                            .expect("authorization request"),
                    ),
                )
                .expect("authorization response");
                assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
            }
        });
        let captured = events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .join("\n");
        assert!(!captured.contains("wrong-candidate-a"));
        assert!(!captured.contains("wrong-candidate-b"));
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
