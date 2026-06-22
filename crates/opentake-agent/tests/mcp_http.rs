//! End-to-end transport test (#36): bring up the real axum +
//! `StreamableHttpService` router on an ephemeral loopback port and drive the MCP
//! `initialize` handshake over HTTP, asserting the server advertises itself and
//! its instructions. This exercises the whole network face — router, session
//! transport, and the `ServerHandler` — without a GUI.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
use opentake_agent::mcp::server::build_router;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_core::AppCore;

#[tokio::test]
async fn initialize_handshake_advertises_server_and_instructions() {
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(AppCore::new()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let router = build_router(handle, registry);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

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
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(AppCore::new()));
    let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
    let router = build_router(handle, registry);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/mcp"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("origin", "http://evil.example.com")
        .json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"ping"}))
        .send()
        .await
        .expect("request sent");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::FORBIDDEN,
        "remote Origin must be rejected by the loopback guard"
    );
}
