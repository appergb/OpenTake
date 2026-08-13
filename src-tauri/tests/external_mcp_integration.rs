use std::{
    fs,
    net::{Ipv4Addr, TcpListener},
    path::{Path, PathBuf},
    time::Duration,
};

use opentake_gen::{KeyStore, KeyringStore};
use opentake_tauri_lib::external_mcp::{
    ExternalMcpIntegrationHarness, ExternalMcpIntegrationReceipt, ExternalMcpListenerState,
};
use rmcp::{
    model::CallToolRequestParams,
    service::RunningService,
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
    },
    RoleClient, ServiceExt,
};

const RUN_REAL_KEYCHAIN_ENV: &str = "OPENTAKE_RUN_REAL_KEYCHAIN_MCP";
const PROCESS_EXIT_CHILD_ENV: &str = "OPENTAKE_MCP_PROCESS_EXIT_CHILD";
const PROCESS_EXIT_ROOT_ENV: &str = "OPENTAKE_MCP_PROCESS_EXIT_ROOT";
const PROCESS_EXIT_SERVICE_ENV: &str = "OPENTAKE_MCP_PROCESS_EXIT_SERVICE";
const PROCESS_EXIT_PROJECT_ENV: &str = "OPENTAKE_MCP_PROCESS_EXIT_PROJECT";
const ENDPOINT: &str = "http://127.0.0.1:19789/mcp";
const PORT: u16 = 19_789;

type RmcpClient = RunningService<RoleClient, ()>;

struct ExactKeychainCleanup {
    service: String,
    accounts: Vec<String>,
}

impl ExactKeychainCleanup {
    fn new(service: String) -> Self {
        Self {
            service,
            accounts: Vec::new(),
        }
    }

    fn track(&mut self, receipt: &ExternalMcpIntegrationReceipt) {
        self.accounts
            .push(format!("external-mcp:{}", receipt.client_id));
    }

    fn cleanup_now(&mut self) {
        let store = KeyringStore::with_service(self.service.clone());
        for account in self.accounts.drain(..) {
            require(
                store.delete(&account),
                "delete exact integration credential",
            );
            assert!(
                require(
                    store.load(&account),
                    "verify integration credential cleanup"
                )
                .is_none(),
                "exact integration credential remains in the keychain"
            );
        }
    }
}

impl Drop for ExactKeychainCleanup {
    fn drop(&mut self) {
        let store = KeyringStore::with_service(self.service.clone());
        for account in &self.accounts {
            let _ = store.delete(account);
        }
    }
}

#[derive(Clone, Default)]
struct CapturingSubscriber {
    events: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    next_span: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

struct StringVisitor<'a>(&'a mut String);

impl tracing::field::Visit for StringVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write as _;

        let _ = write!(self.0, " {}={value:?}", field.name());
    }
}

impl tracing::Subscriber for CapturingSubscriber {
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        let mut recorded = format!("span {}", span.metadata().name());
        span.record(&mut StringVisitor(&mut recorded));
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(recorded);
        let id = self
            .next_span
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        tracing::span::Id::from_u64(id)
    }

    fn record(&self, _span: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        let mut recorded = "span record".to_string();
        values.record(&mut StringVisitor(&mut recorded));
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(recorded);
    }

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut recorded = event.metadata().name().to_owned();
        event.record(&mut StringVisitor(&mut recorded));
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

fn require<T, E>(result: Result<T, E>, message: &'static str) -> T {
    result.unwrap_or_else(|_| panic!("{message}"))
}

async fn connect_rmcp(token: &str) -> Result<RmcpClient, ()> {
    let config = StreamableHttpClientTransportConfig::with_uri(ENDPOINT).auth_header(token);
    let transport = StreamableHttpClientTransport::from_config(config);
    tokio::time::timeout(Duration::from_secs(5), ().serve(transport))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
}

async fn call_tool(
    client: &RmcpClient,
    name: &'static str,
    arguments: serde_json::Map<String, serde_json::Value>,
) -> rmcp::model::CallToolResult {
    require(
        tokio::time::timeout(
            Duration::from_secs(5),
            client.call_tool(CallToolRequestParams::new(name).with_arguments(arguments)),
        )
        .await
        .map_err(|_| ())
        .and_then(|result| result.map_err(|_| ())),
        "rmcp tool call did not complete",
    )
}

async fn close_rmcp(client: &mut RmcpClient) {
    let _ = tokio::time::timeout(Duration::from_secs(3), client.close()).await;
}

fn arguments(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    value.as_object().cloned().expect("literal object")
}

fn create_project(path: &Path) {
    let core = opentake_core::AppCore::new();
    require(
        core.save_project(Some(path.to_path_buf())),
        "create integration project",
    );
}

fn assert_port_closed() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, PORT))
        .unwrap_or_else(|_| panic!("external MCP socket is still open"));
    drop(listener);
}

fn catalog_bytes(root: &Path) -> Vec<u8> {
    let directory = root.join("external-mcp");
    let mut bytes = Vec::new();
    let Ok(entries) = fs::read_dir(directory) else {
        return bytes;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_file()) {
            bytes.extend(require(
                fs::read(entry.path()),
                "read integration catalog file",
            ));
        }
    }
    bytes
}

fn assert_tokens_absent(tokens: &[String], bytes: &[u8]) {
    for token in tokens {
        assert!(
            !bytes
                .windows(token.len())
                .any(|window| window == token.as_bytes()),
            "generated credential leaked into captured bytes"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_mcp_process_exit_child() {
    if std::env::var_os(PROCESS_EXIT_CHILD_ENV).as_deref() != Some(std::ffi::OsStr::new("1")) {
        return;
    }
    let root = PathBuf::from(require(
        std::env::var(PROCESS_EXIT_ROOT_ENV),
        "read child catalog root",
    ));
    let service = require(
        std::env::var(PROCESS_EXIT_SERVICE_ENV),
        "read child keychain service",
    );
    let project = PathBuf::from(require(
        std::env::var(PROCESS_EXIT_PROJECT_ENV),
        "read child project path",
    ));
    let child = require(
        ExternalMcpIntegrationHarness::new(&root, &service),
        "construct process-exit child state",
    );
    require(child.core().open_project(project), "open child project");
    child.initialize().await;
    assert_eq!(
        child.listener_state().await,
        ExternalMcpListenerState::Listening
    );
    std::process::exit(0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_keychain_restart_and_security_matrix() {
    if std::env::var_os(RUN_REAL_KEYCHAIN_ENV).as_deref() != Some(std::ffi::OsStr::new("1")) {
        eprintln!("real keychain matrix skipped; set {RUN_REAL_KEYCHAIN_ENV}=1 to opt in");
        return;
    }
    let _ = rustls::crypto::ring::default_provider().install_default();
    let captured = CapturingSubscriber::default();
    let captured_events = captured.events.clone();
    require(
        tracing::subscriber::set_global_default(captured),
        "install integration tracing capture",
    );

    let root = require(tempfile::tempdir(), "create integration root");
    let namespace = uuid::Uuid::new_v4().simple().to_string();
    let service = format!("io.opentake.integration.external-mcp.{namespace}");
    let mut cleanup = ExactKeychainCleanup::new(service.clone());
    let project_a = root.path().join("A.opentake");
    let project_b = root.path().join("B.opentake");
    create_project(&project_a);
    create_project(&project_b);
    let mut tokens = Vec::new();
    let mut matrix_log = Vec::<String>::new();

    let first = require(
        ExternalMcpIntegrationHarness::new(root.path(), &service),
        "construct first external MCP state",
    );
    require(first.core().open_project(&project_a), "open project A");
    require(first.set_enabled(true).await, "enable first endpoint");
    let paired = require(
        first.pair("integration-primary").await,
        "pair primary client",
    );
    cleanup.track(&paired);
    tokens.push(paired.bearer_token.clone());
    assert_eq!(
        first.listener_state().await,
        ExternalMcpListenerState::Listening
    );

    let mut session_a = require(connect_rmcp(&paired.bearer_token).await, "connect rmcp A");
    let mut session_b = require(connect_rmcp(&paired.bearer_token).await, "connect rmcp B");
    let tools = require(session_a.list_all_tools().await, "list rmcp tools");
    assert!(tools.iter().any(|tool| tool.name == "create_folder"));

    let created = call_tool(
        &session_a,
        "create_folder",
        arguments(serde_json::json!({ "name": "owned-by-a" })),
    )
    .await;
    assert_ne!(created.is_error, Some(true));
    let foreign_undo = call_tool(&session_b, "undo", serde_json::Map::new()).await;
    assert_eq!(foreign_undo.is_error, Some(true));
    let owner_undo = call_tool(&session_a, "undo", serde_json::Map::new()).await;
    assert_ne!(owner_undo.is_error, Some(true));
    matrix_log.push("cross-session undo isolation: pass".to_string());
    close_rmcp(&mut session_a).await;
    close_rmcp(&mut session_b).await;
    require(first.shutdown().await, "stop first endpoint");
    assert_port_closed();

    let restarted = require(
        ExternalMcpIntegrationHarness::new(root.path(), &service),
        "construct restarted external MCP state",
    );
    require(
        restarted.core().open_project(&project_a),
        "reopen project A",
    );
    restarted.initialize().await;
    assert_eq!(
        restarted.listener_state().await,
        ExternalMcpListenerState::Listening
    );
    let mut restored = require(
        connect_rmcp(&paired.bearer_token).await,
        "authenticate after restart",
    );
    require(
        restored.list_all_tools().await,
        "use restarted rmcp session",
    );
    matrix_log.push("authenticated catalog/keychain restart: pass".to_string());

    let raw = reqwest::Client::new();
    let initialize = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "boundary-probe", "version": "0" }
        }
    });
    let remote_host = require(
        raw.post(ENDPOINT)
            .bearer_auth(&paired.bearer_token)
            .header("host", "attacker.example:19789")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize)
            .send()
            .await,
        "send remote Host probe",
    );
    assert_eq!(remote_host.status(), reqwest::StatusCode::FORBIDDEN);
    let remote_origin = require(
        raw.post(ENDPOINT)
            .bearer_auth(&paired.bearer_token)
            .header("origin", "https://attacker.example")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize)
            .send()
            .await,
        "send remote Origin probe",
    );
    assert_eq!(remote_origin.status(), reqwest::StatusCode::FORBIDDEN);
    matrix_log.push("Host/Origin rejection: pass".to_string());

    let peer = restored.peer().clone();
    let blocked = tokio::spawn(async move {
        peer.call_tool(
            CallToolRequestParams::new("import_media").with_arguments(arguments(
                serde_json::json!({
                    "source": { "bytes": "AA==", "mimeType": "image/png" },
                    "name": "must-not-commit"
                }),
            )),
        )
        .await
    });
    require(
        tokio::time::timeout(Duration::from_secs(3), restarted.wait_for_cancel_probe())
            .await
            .map_err(|_| ()),
        "blocking tool did not reach the project gate",
    );
    let switch_core = restarted.core();
    let switch_target = project_b.clone();
    let switched = tokio::task::spawn_blocking(move || switch_core.open_project(switch_target));
    let blocked_result = require(
        tokio::time::timeout(Duration::from_secs(5), blocked)
            .await
            .map_err(|_| ())
            .and_then(|result| result.map_err(|_| ())),
        "project switch did not cancel active rmcp work",
    );
    assert!(
        blocked_result.is_err() || blocked_result.is_ok_and(|result| result.is_error == Some(true))
    );
    require(
        require(switched.await, "join project switch"),
        "switch to project B",
    );
    assert!(restarted.cancel_probe_observed());
    assert!(restarted.core().media().entries.is_empty());
    matrix_log.push("project-switch cancellation: pass".to_string());
    close_rmcp(&mut restored).await;

    let survivor = require(
        restarted.pair("integration-survivor").await,
        "pair surviving client",
    );
    cleanup.track(&survivor);
    tokens.push(survivor.bearer_token.clone());
    let mut survivor_session = require(
        connect_rmcp(&survivor.bearer_token).await,
        "connect surviving client",
    );
    require(
        restarted.revoke(&paired.client_id).await,
        "revoke primary client",
    );
    assert_eq!(
        restarted.listener_state().await,
        ExternalMcpListenerState::Listening
    );
    let revoked = connect_rmcp(&paired.bearer_token).await;
    assert!(revoked.is_err(), "revoked credential authenticated");
    require(
        survivor_session.list_all_tools().await,
        "surviving credential stopped working after targeted revoke",
    );
    close_rmcp(&mut survivor_session).await;
    matrix_log.push("revoked credential rejection: pass".to_string());

    require(
        restarted.set_enabled(false).await,
        "stop endpoint before fixed-port probe",
    );
    let occupied = require(
        tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, PORT)).await,
        "occupy fixed port",
    );
    require(
        restarted.set_enabled(true).await,
        "enable endpoint during port conflict",
    );
    assert_eq!(
        restarted.listener_state().await,
        ExternalMcpListenerState::PortConflict
    );
    matrix_log.push("fixed-port conflict: pass".to_string());
    drop(occupied);

    restarted.initialize().await;
    assert_eq!(
        restarted.listener_state().await,
        ExternalMcpListenerState::Listening
    );
    let mut final_client = require(
        connect_rmcp(&survivor.bearer_token).await,
        "connect after port conflict clears",
    );
    require(
        final_client.list_all_tools().await,
        "use final rmcp session",
    );
    close_rmcp(&mut final_client).await;

    require(
        restarted.set_enabled(false).await,
        "disable external endpoint",
    );
    assert_port_closed();
    matrix_log.push("disable socket closure: pass".to_string());
    require(
        restarted.set_enabled(true).await,
        "re-enable external endpoint",
    );
    require(
        restarted.shutdown().await,
        "release parent endpoint before process-exit child",
    );
    assert_port_closed();
    let child_output = require(
        std::process::Command::new(require(
            std::env::current_exe(),
            "resolve integration test executable",
        ))
        .arg("--exact")
        .arg("external_mcp_process_exit_child")
        .arg("--nocapture")
        .env(PROCESS_EXIT_CHILD_ENV, "1")
        .env(PROCESS_EXIT_ROOT_ENV, root.path())
        .env(PROCESS_EXIT_SERVICE_ENV, &service)
        .env(PROCESS_EXIT_PROJECT_ENV, &project_b)
        .output(),
        "run process-exit child",
    );
    assert!(child_output.status.success(), "process-exit child failed");
    assert_port_closed();
    matrix_log.push("application-exit socket closure: pass".to_string());

    let mut log_bytes = matrix_log.join("\n").into_bytes();
    log_bytes.extend_from_slice(&child_output.stdout);
    log_bytes.extend_from_slice(&child_output.stderr);
    for event in captured_events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
    {
        log_bytes.extend_from_slice(event.as_bytes());
        log_bytes.push(b'\n');
    }
    let persisted_bytes = catalog_bytes(root.path());
    assert_tokens_absent(&tokens, &log_bytes);
    assert_tokens_absent(&tokens, &persisted_bytes);
    cleanup.cleanup_now();
    eprintln!("{}", matrix_log.join("\n"));
    eprintln!("credential scan: zero full-token matches in captured log/catalog bytes");
}
