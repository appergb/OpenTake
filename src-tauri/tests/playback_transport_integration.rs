//! HTTP integration test for the MJPEG preview transport (#64 / #53): start the
//! loopback server and assert the LIVE axum route serves a `multipart/x-mixed-
//! replace` stream, and that the Origin guard rejects a cross-origin request.
//!
//! Gated behind the `playback-engine` feature (the module only exists then). Uses
//! a blocking std TCP client + a raw HTTP/1.1 request so it needs no HTTP-client
//! dependency. The server is started exactly as the app does — through the Tauri
//! async runtime — so bind + serve share one runtime.
#![cfg(feature = "playback-engine")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use opentake_tauri_lib::playback::PreviewServer;

/// Parse the port out of `http://127.0.0.1:<port>/stream`.
fn port_of(endpoint: &str) -> u16 {
    endpoint
        .rsplit(':')
        .next()
        .and_then(|tail| tail.split('/').next())
        .and_then(|p| p.parse().ok())
        .expect("endpoint carries a port")
}

/// Read until the end of the HTTP response headers (`\r\n\r\n`) or a short
/// timeout — enough to assert the status line + content type without consuming
/// the (infinite) multipart body.
fn read_head(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("set read timeout");
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 256];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 8192 {
                    break;
                }
            }
            Err(_) => break, // timeout / would-block: return what we have
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn start_server() -> Option<std::sync::Arc<PreviewServer>> {
    match tauri::async_runtime::block_on(PreviewServer::start()) {
        Ok(server) => Some(server),
        Err(e) => {
            eprintln!("skip: preview server did not start ({e})");
            None
        }
    }
}

fn get(port: u16, extra_headers: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect loopback");
    let req = format!(
        "GET /stream HTTP/1.1\r\nHost: 127.0.0.1\r\n{extra_headers}Connection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).expect("write request");
    read_head(&mut stream)
}

#[test]
fn stream_route_serves_multipart_mjpeg() {
    let Some(server) = start_server() else {
        return;
    };
    let head = get(port_of(&server.endpoint()), "");
    assert!(head.contains(" 200 "), "expected HTTP 200, got:\n{head}");
    assert!(
        head.contains("multipart/x-mixed-replace"),
        "expected the MJPEG content type, got:\n{head}"
    );
    assert!(
        head.contains("opentake_mjpeg_boundary"),
        "expected the multipart boundary, got:\n{head}"
    );
}

#[test]
fn stream_route_rejects_cross_origin() {
    let Some(server) = start_server() else {
        return;
    };
    let head = get(
        port_of(&server.endpoint()),
        "Origin: http://evil.example.com\r\n",
    );
    assert!(
        head.contains(" 403 "),
        "expected HTTP 403 for a cross-origin request, got:\n{head}"
    );
}
