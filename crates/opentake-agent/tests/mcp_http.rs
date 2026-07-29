//! End-to-end transport test (#36): bring up the real axum +
//! `StreamableHttpService` router on an ephemeral loopback port and drive the MCP
//! `initialize` handshake over HTTP, asserting the server advertises itself and
//! its instructions. This exercises the whole network face — router, session
//! transport, and the `ServerHandler` — without a GUI.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::body::Body;
use axum::http::{header::HOST, HeaderValue, Request};
use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
use opentake_agent::mcp::media_bridge::{
    BridgeError, ImportOutcome, ImportSource, MediaBridge, MCP_REQUEST_BODY_MAX,
};
use opentake_agent::mcp::server::{
    build_router, build_router_for_port, build_router_with_bridge_for_port, serve,
};
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_core::AppCore;
use opentake_domain::{ClipType, MediaManifest, Timeline};
use opentake_ops::command::{EditCommand, EditResult};
use tower::ServiceExt;

fn insert_video_track(core: &AppCore) {
    core.apply(EditCommand::InsertTrack {
        kind: ClipType::Video,
        at: None,
    })
    .unwrap();
}

async fn start_router(
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = build_router_for_port(handle, registry, addr.port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    addr
}

async fn start_router_with_bridge(
    handle: Arc<dyn CoreHandle>,
    registry: Arc<RwLock<PluginRegistry>>,
    bridge: Arc<dyn MediaBridge>,
) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = build_router_with_bridge_for_port(handle, registry, Some(bridge), addr.port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    addr
}

fn valid_host(addr: std::net::SocketAddr) -> String {
    format!("127.0.0.1:{}", addr.port())
}

async fn initialize_session(
    client: &reqwest::Client,
    addr: std::net::SocketAddr,
) -> reqwest::header::HeaderValue {
    let response = client
        .post(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "opentake-test", "version": "0" }
            }
        }))
        .send()
        .await
        .expect("initialize sent");
    assert!(response.status().is_success(), "initialize failed");
    response
        .headers()
        .get("mcp-session-id")
        .expect("stateful server returns a session id")
        .clone()
}

fn remove_track_call() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "remove_tracks",
            "arguments": { "trackIndexes": [0] }
        }
    })
}

#[tokio::test]
async fn initialize_handshake_advertises_server_and_instructions() {
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(AppCore::new()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let addr = start_router(handle, registry).await;

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "opentake-test", "version": "0" }
        }
    });
    let resp = client
        .post(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .json(&body)
        .send()
        .await
        .expect("request sent");

    assert!(
        resp.status().is_success(),
        "initialize HTTP status: {}",
        resp.status()
    );
    let text = resp.text().await.expect("body");
    // The initialize result carries our serverInfo (name "opentake") and the
    // assembled instructions (which mention the bundled audio-first Skill).
    assert!(
        text.contains("opentake"),
        "response should carry serverInfo: {text}"
    );
}

#[tokio::test]
async fn non_local_origin_is_rejected() {
    let core = AppCore::new();
    insert_video_track(&core);
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let addr = start_router(handle, registry).await;
    let client = reqwest::Client::new();
    let session_id = initialize_session(&client, addr).await;
    let resp = client
        .post(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id.clone())
        .header("mcp-protocol-version", "2025-06-18")
        .header("origin", format!("http://evil.example.com:{}", addr.port()))
        .json(&remove_track_call())
        .send()
        .await
        .expect("request sent");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::FORBIDDEN,
        "remote Origin must be rejected by the loopback guard"
    );
    assert_eq!(
        core.get_timeline().timeline.tracks.len(),
        1,
        "a rejected Origin must never invoke the tool"
    );

    let resp = client
        .post(format!("http://{addr}/mcp"))
        .header("host", format!("[::1].evil:{}", addr.port()))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id.clone())
        .header("mcp-protocol-version", "2025-06-18")
        .json(&remove_track_call())
        .send()
        .await
        .expect("request sent");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::FORBIDDEN,
        "a loopback-looking Host suffix attack must be rejected"
    );
    assert_eq!(
        core.get_timeline().timeline.tracks.len(),
        1,
        "a rejected Host must never invoke the tool"
    );

    let resp = client
        .post(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id)
        .header("mcp-protocol-version", "2025-06-18")
        .header("origin", format!("http://127.0.0.1:{}", addr.port()))
        .json(&remove_track_call())
        .send()
        .await
        .expect("local tool request sent");
    assert!(resp.status().is_success(), "local tool request failed");
    let _ = resp.text().await.expect("local tool response body");
    assert_eq!(
        core.get_timeline().timeline.tracks.len(),
        0,
        "the same tool payload executes after valid transport headers"
    );

    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(AppCore::new()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let router = build_router(handle, registry);
    let mut request = Request::post("/mcp")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(Body::from(
            serde_json::json!({"jsonrpc":"2.0","id":3,"method":"ping"}).to_string(),
        ))
        .unwrap();
    request
        .headers_mut()
        .insert(HOST, HeaderValue::from_bytes(&[0xff]).unwrap());
    let resp = router.clone().oneshot(request).await.unwrap();
    assert_eq!(
        resp.status(),
        axum::http::StatusCode::FORBIDDEN,
        "a present but non-UTF-8 Host must be rejected"
    );

    let request = Request::post("/mcp")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(Body::from(
            serde_json::json!({"jsonrpc":"2.0","id":4,"method":"ping"}).to_string(),
        ))
        .unwrap();
    let resp = router.oneshot(request).await.unwrap();
    assert_eq!(
        resp.status(),
        axum::http::StatusCode::FORBIDDEN,
        "a missing Host must be rejected"
    );
}

#[tokio::test]
async fn oversized_request_body_is_rejected() {
    let core = AppCore::new();
    insert_video_track(&core);
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = build_router_for_port(handle, registry, addr.port());
    let serving_router = router.clone();
    tokio::spawn(async move {
        let _ = axum::serve(listener, serving_router).await;
    });

    let client = reqwest::Client::new();
    let session_id = initialize_session(&client, addr).await;
    let mut body = remove_track_call().to_string();
    body.extend(std::iter::repeat_n(
        ' ',
        MCP_REQUEST_BODY_MAX + 1 - body.len(),
    ));
    let request = Request::post("/mcp")
        .header("host", valid_host(addr))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id)
        .header("mcp-protocol-version", "2025-06-18")
        .header("content-length", body.len())
        .body(Body::from(body))
        .unwrap();
    let resp = router.oneshot(request).await.unwrap();
    assert_eq!(
        resp.status(),
        axum::http::StatusCode::PAYLOAD_TOO_LARGE,
        "oversized MCP body must be rejected before dispatch"
    );
    assert_eq!(
        core.get_timeline().timeline.tracks.len(),
        1,
        "an oversized otherwise-valid tool call must never be dispatched"
    );
}

#[tokio::test]
async fn serve_rejects_non_loopback_bind() {
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(AppCore::new()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let addr = "0.0.0.0:0".parse().unwrap();

    let result = tokio::time::timeout(Duration::from_millis(250), serve(addr, handle, registry))
        .await
        .expect("non-loopback startup must reject instead of serving");
    let error = result.expect_err("non-loopback bind must fail");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("loopback"), "{error}");
}

#[tokio::test]
async fn unsupported_protocol_version_is_400() {
    let core = AppCore::new();
    insert_video_track(&core);
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let addr = start_router(handle, registry).await;
    let client = reqwest::Client::new();
    let session_id = initialize_session(&client, addr).await;

    let response = client
        .post(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id)
        .header("mcp-protocol-version", "1900-01-01")
        .json(&remove_track_call())
        .send()
        .await
        .expect("tool request sent");

    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(
        core.get_timeline().timeline.tracks.len(),
        1,
        "unsupported protocol versions must never invoke the tool"
    );
}

#[tokio::test]
async fn non_json_content_type_is_415() {
    let core = AppCore::new();
    insert_video_track(&core);
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let addr = start_router(handle, registry).await;

    let client = reqwest::Client::new();
    let session_id = initialize_session(&client, addr).await;
    let response = client
        .post(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("content-type", "text/plain")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id)
        .header("mcp-protocol-version", "2025-06-18")
        .body(remove_track_call().to_string())
        .send()
        .await
        .expect("tool request sent");

    assert_eq!(
        response.status(),
        reqwest::StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    assert_eq!(
        response.text().await.expect("guard response body"),
        "OpenTake MCP requires a single Content-Type: application/json"
    );
    assert_eq!(
        core.get_timeline().timeline.tracks.len(),
        1,
        "invalid content type must be rejected before tool dispatch"
    );
}

#[tokio::test]
async fn only_exact_mcp_route_is_served() {
    let core = AppCore::new();
    insert_video_track(&core);
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let addr = start_router(handle, registry).await;

    let client = reqwest::Client::new();
    let session_id = initialize_session(&client, addr).await;

    let trailing_slash = client
        .post(format!("http://{addr}/mcp/"))
        .header("host", valid_host(addr))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id.clone())
        .header("mcp-protocol-version", "1900-01-01")
        .json(&remove_track_call())
        .send()
        .await
        .expect("trailing-slash request sent");
    assert_eq!(trailing_slash.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(
        core.get_timeline().timeline.tracks.len(),
        1,
        "unsupported protocol must not reach rmcp through /mcp/"
    );

    let nested_path = client
        .post(format!("http://{addr}/mcp/x"))
        .header("host", valid_host(addr))
        .header("content-type", "text/plain")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id.clone())
        .header("mcp-protocol-version", "2025-06-18")
        .body(remove_track_call().to_string())
        .send()
        .await
        .expect("nested-path request sent");
    assert_eq!(nested_path.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(
        core.get_timeline().timeline.tracks.len(),
        1,
        "invalid content type must not reach rmcp through /mcp/x"
    );

    let get = client
        .get(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("accept", "text/event-stream")
        .header("mcp-session-id", session_id.clone())
        .header("mcp-protocol-version", "2025-06-18")
        .send()
        .await
        .expect("GET /mcp sent");
    assert!(get.status().is_success(), "GET /mcp must remain served");
    drop(get);

    let delete = client
        .delete(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("mcp-session-id", session_id)
        .header("mcp-protocol-version", "2025-06-18")
        .send()
        .await
        .expect("DELETE /mcp sent");
    assert_eq!(delete.status(), reqwest::StatusCode::ACCEPTED);
}

#[tokio::test]
async fn transport_rejects_nonfinite_numbers_before_dispatch() {
    struct DispatchCountingHandle {
        calls: Arc<AtomicUsize>,
    }
    impl CoreHandle for DispatchCountingHandle {
        fn timeline(&self) -> Timeline {
            self.calls.fetch_add(1, Ordering::AcqRel);
            Timeline::new()
        }

        fn media(&self) -> MediaManifest {
            self.calls.fetch_add(1, Ordering::AcqRel);
            MediaManifest::new()
        }

        fn apply(&self, _cmd: EditCommand) -> anyhow::Result<EditResult> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            anyhow::bail!("non-finite parser proof must not dispatch")
        }

        fn project_dir(&self) -> Option<PathBuf> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            None
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let handle: Arc<dyn CoreHandle> = Arc::new(DispatchCountingHandle {
        calls: calls.clone(),
    });
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let addr = start_router(handle, registry).await;
    let client = reqwest::Client::new();
    let session_id = initialize_session(&client, addr).await;
    calls.store(0, Ordering::Release);

    for (id, number) in ["NaN", "Infinity", "-Infinity", "1e400"]
        .into_iter()
        .enumerate()
    {
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":{},"method":"tools/call","params":{{"name":"add_clips","arguments":{{"entries":[{{"mediaRef":"asset","startFrame":0,"durationFrames":1}},{{"mediaRef":"asset","startFrame":0,"durationFrames":1}},{{"mediaRef":"asset","startFrame":0,"durationFrames":1}},{{"mediaRef":"asset","startFrame":{number},"durationFrames":1}}]}}}}}}"#,
            id + 100
        );
        let response = client
            .post(format!("http://{addr}/mcp"))
            .header("host", valid_host(addr))
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", session_id.clone())
            .header("mcp-protocol-version", "2025-06-18")
            .body(body)
            .send()
            .await
            .expect("raw non-finite request sent");
        let status = response.status();
        let text = response.text().await.expect("parser response body");
        assert_eq!(status, reqwest::StatusCode::BAD_REQUEST, "{number}: {text}");
        assert_eq!(
            text, "entries[3].startFrame: value must be finite",
            "{number} path/message drifted"
        );
        assert_eq!(
            calls.load(Ordering::Acquire),
            0,
            "{number} reached dispatch"
        );
    }
}

#[derive(Default)]
struct CancellationBridge {
    started: AtomicBool,
    observed: AtomicBool,
}

impl MediaBridge for CancellationBridge {
    fn import_media_cancellable(
        &self,
        _source: ImportSource,
        _name: Option<String>,
        _folder_id: Option<String>,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<ImportOutcome, BridgeError> {
        self.started.store(true, Ordering::Release);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if cancel.checkpoint() {
                self.observed.store(true, Ordering::Release);
                return Err(BridgeError::new("import cancelled by MCP notification"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(BridgeError::new("timed out waiting for MCP cancellation"))
    }
}

struct PrivateFailureBridge;

impl MediaBridge for PrivateFailureBridge {
    fn import_media(
        &self,
        _source: ImportSource,
        _name: Option<String>,
        _folder_id: Option<String>,
    ) -> Result<ImportOutcome, BridgeError> {
        Err(BridgeError::new(
            "quota exhausted for customer alice plan enterprise",
        ))
    }
}

#[tokio::test]
async fn real_mcp_transport_redacts_private_bridge_error() {
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(AppCore::new()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let addr = start_router_with_bridge(handle, registry, Arc::new(PrivateFailureBridge)).await;
    let client = reqwest::Client::new();
    let session_id = initialize_session(&client, addr).await;

    let response = client
        .post(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id)
        .header("mcp-protocol-version", "2025-06-18")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 76,
            "method": "tools/call",
            "params": {
                "name": "import_media",
                "arguments": {
                    "source": {"url": "https://example.com/clip.mp4"}
                }
            }
        }))
        .send()
        .await
        .expect("tool request sent");
    assert!(response.status().is_success(), "MCP request failed");
    let wire = response.text().await.expect("MCP response body");
    assert!(wire.contains("MCP_TOOL_ERROR_REDACTED"), "{wire}");
    assert!(
        !wire.contains("quota exhausted for customer alice plan enterprise"),
        "real MCP response leaked bridge error: {wire}"
    );
}

#[tokio::test]
async fn real_mcp_transport_never_echoes_dynamic_param_key() {
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(AppCore::new()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let addr = start_router(handle, registry).await;
    let client = reqwest::Client::new();
    let session_id = initialize_session(&client, addr).await;
    let secret = "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890";

    let response = client
        .post(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id)
        .header("mcp-protocol-version", "2025-06-18")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 75,
            "method": "tools/call",
            "params": {
                "name": "edit_motion_graphic",
                "arguments": {
                    "clipId": "clip",
                    "params": {(secret): []}
                }
            }
        }))
        .send()
        .await
        .expect("tool request sent");
    assert!(response.status().is_success(), "MCP request failed");
    let wire = response.text().await.expect("MCP response body");
    assert!(wire.contains("MCP_INVALID_ARGUMENTS"), "{wire}");
    assert!(!wire.contains(secret), "dynamic key leaked: {wire}");
}

#[tokio::test]
async fn cancelled_notification_reaches_media_cancel_token() {
    let core = AppCore::new();
    let before = core.media();
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core.clone()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let bridge = Arc::new(CancellationBridge::default());
    let addr = start_router_with_bridge(handle, registry, bridge.clone()).await;
    let client = reqwest::Client::new();
    let session_id = initialize_session(&client, addr).await;

    let tool_client = client.clone();
    let tool_session = session_id.clone();
    let tool_call = tokio::spawn(async move {
        tool_client
            .post(format!("http://{addr}/mcp"))
            .header("host", valid_host(addr))
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", tool_session)
            .header("mcp-protocol-version", "2025-06-18")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 77,
                "method": "tools/call",
                "params": {
                    "name": "import_media",
                    "arguments": {
                        "source": {"url": "https://example.com/clip.mp4"}
                    }
                }
            }))
            .send()
            .await
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        while !bridge.started.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("import bridge started");

    let notification = client
        .post(format!("http://{addr}/mcp"))
        .header("host", valid_host(addr))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-session-id", session_id)
        .header("mcp-protocol-version", "2025-06-18")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": {"requestId": 77, "reason": "test cancellation"}
        }))
        .send()
        .await
        .expect("cancellation notification sent");
    assert!(
        notification.status().is_success(),
        "cancellation notification failed: {}",
        notification.status()
    );

    tokio::time::timeout(Duration::from_secs(2), async {
        while !bridge.observed.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("bridge observed cancellation");
    let _ = tokio::time::timeout(Duration::from_secs(2), tool_call)
        .await
        .expect("cancelled tool request completed");
    assert_eq!(core.media(), before);
}
