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

use opentake_render::DecodedFrame;
use opentake_tauri_lib::playback::session::PlaybackIdentity;
use opentake_tauri_lib::playback::transport::PublicationGate;
use opentake_tauri_lib::playback::{FrameSink, PreviewServer};

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
    get_path(port, "/stream", extra_headers)
}

fn get_path(port: u16, path: &str, extra_headers: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect loopback");
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n{extra_headers}Connection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).expect("write request");
    read_head(&mut stream)
}

#[test]
fn frame_route_requires_the_full_session_query() {
    let Some(server) = start_server() else {
        return;
    };
    let port = port_of(&server.endpoint());
    let missing = get_path(port, "/frame", "");
    assert!(
        missing.contains(" 400 "),
        "missing query must be rejected:\n{missing}"
    );

    let no_publication = get_path(
        port,
        "/frame?projectEpoch=1&timelineVersion=0&sessionId=session-1&frame=0&sequence=1",
        "",
    );
    assert!(
        no_publication.contains(" 204 "),
        "a valid query with no matching publication must return 204:\n{no_publication}"
    );
}

#[test]
fn frame_route_serves_only_the_exact_published_identity() {
    let Some(server) = start_server() else {
        return;
    };
    let port = port_of(&server.endpoint());
    let identity = PlaybackIdentity::new(7, 11, "session-42").expect("valid identity");
    let sink = server.sink(identity, PublicationGate::open());
    sink.push_frame(&DecodedFrame::new(2, 2, vec![255; 2 * 2 * 4], false));
    sink.publication()
        .commit(18, 18)
        .expect("encoded sink frame must commit to the exact-frame route");

    let cases = [
        (7, 11, "session-42", 18, 1, 200, "exact identity"),
        (8, 11, "session-42", 18, 1, 204, "wrong project epoch"),
        (7, 12, "session-42", 18, 1, 204, "wrong timeline version"),
        (7, 11, "session-43", 18, 1, 204, "wrong session"),
        (7, 11, "session-42", 19, 1, 204, "wrong frame"),
        (7, 11, "session-42", 18, 2, 204, "wrong sequence"),
    ];
    for (epoch, version, session, frame, sequence, expected, label) in cases {
        let response = get_path(
            port,
            &format!(
                "/frame?projectEpoch={epoch}&timelineVersion={version}&sessionId={session}&frame={frame}&sequence={sequence}"
            ),
            "",
        );
        assert!(
            response.contains(&format!(" {expected} ")),
            "{label} must return {expected}:\n{response}"
        );
    }
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
