//! MJPEG loopback frame transport for streaming playback (#64).
//!
//! The render thread ([`super::engine`]) composites frames and hands each to a
//! [`super::engine::FrameSink`]; [`MjpegSink`] JPEG-encodes it and pushes it into
//! a `broadcast` channel. A loopback axum server relays those JPEGs as a
//! `multipart/x-mixed-replace` stream, which the WebView consumes with a single
//! `<img>` — the browser decodes JPEG on its own threads and paces the display.
//!
//! This is the transport half of #53. Unlike the abandoned PR #153 (which fed the
//! stream from the slow per-frame `composite_frame`), the producer here is the
//! continuous render thread, so the stream is real-time. The sink is a trait so
//! the transport can be swapped (WS binary / custom scheme) if `multipart` proves
//! unreliable on a given WebView, without touching the engine.
//!
//! Security: the server binds `127.0.0.1:<random port>` (not externally
//! reachable) and the `/stream` route additionally rejects any request carrying a
//! non-loopback `Origin` (defence-in-depth, mirroring the MCP server's guard).

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast;

use opentake_render::DecodedFrame;

use super::engine::{FrameSink, PlayheadEmitter};

/// Broadcast channel depth. 2 keeps latency low: a slow `<img>` consumer drops
/// stale frames (the receiver sees `Lagged`) rather than back-pressuring the
/// render thread.
const FRAME_CHANNEL_DEPTH: usize = 2;

/// JPEG quality for preview frames (0–100). 75 is visually clean for a preview
/// while keeping each frame small enough for a 30–60 fps loopback stream.
const JPEG_QUALITY: u8 = 75;

/// The multipart boundary marker for the MJPEG stream.
const BOUNDARY: &str = "opentake_mjpeg_boundary";

/// The loopback MJPEG preview server: a bound port + the frame broadcast sender.
/// The axum task is spawned on the Tauri async runtime and shuts down when the
/// process exits. Managed as Tauri state so `get_preview_endpoint` and the sink
/// can reach it.
pub struct PreviewServer {
    port: u16,
    tx: broadcast::Sender<Bytes>,
}

impl PreviewServer {
    /// Start the MJPEG server on a random loopback port. Must run inside the
    /// Tauri async runtime (call via `tauri::async_runtime::block_on` in setup).
    pub async fn start() -> Result<Arc<Self>, String> {
        let (tx, _rx) = broadcast::channel::<Bytes>(FRAME_CHANNEL_DEPTH);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("MJPEG bind: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("MJPEG local_addr: {e}"))?
            .port();

        let tx_clone = tx.clone();
        tauri::async_runtime::spawn(async move {
            let app = axum::Router::new()
                .route("/stream", axum::routing::get(stream_handler))
                .with_state(tx_clone);
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("[mjpeg] server error: {e}");
            }
        });

        Ok(Arc::new(Self { port, tx }))
    }

    /// The `<img>`-pointable MJPEG stream URL.
    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/stream", self.port)
    }

    /// A frame sink that JPEG-encodes composited frames into this server's stream.
    pub fn sink(&self) -> MjpegSink {
        MjpegSink {
            tx: self.tx.clone(),
        }
    }
}

/// `Origin` defence-in-depth: allow requests with no `Origin` (a plain `<img>`
/// load omits it) or a loopback / Tauri-webview origin; reject anything else.
fn origin_is_allowed(headers: &HeaderMap) -> bool {
    match headers.get(axum::http::header::ORIGIN) {
        None => true,
        Some(value) => match value.to_str() {
            Ok(origin) => {
                origin.starts_with("http://127.0.0.1")
                    || origin.starts_with("http://localhost")
                    || origin.starts_with("https://localhost")
                    || origin.starts_with("tauri://")
                    || origin.starts_with("http://tauri.localhost")
            }
            Err(_) => false,
        },
    }
}

/// `/stream`: relay each broadcast JPEG as a `multipart/x-mixed-replace` part.
async fn stream_handler(
    State(tx): State<broadcast::Sender<Bytes>>,
    headers: HeaderMap,
) -> Response {
    if !origin_is_allowed(&headers) {
        return (StatusCode::FORBIDDEN, "cross-origin preview stream denied").into_response();
    }

    let mut rx = tx.subscribe();
    // Bridge the broadcast receiver to an axum body stream via an unbounded mpsc.
    let (body_tx, body_rx) = tokio::sync::mpsc::unbounded_channel::<Result<Bytes, Infallible>>();

    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(jpeg) => {
                    let header = format!(
                        "\r\n--{BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        jpeg.len()
                    );
                    if body_tx.send(Ok(Bytes::from(header))).is_err() {
                        break; // client disconnected
                    }
                    if body_tx.send(Ok(jpeg)).is_err() {
                        break;
                    }
                }
                // Slow consumer: skip the dropped frames and keep going (live preview).
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let stream = futures::stream::unfold(body_rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    let body = axum::body::Body::from_stream(stream);
    (
        [
            (
                axum::http::header::CONTENT_TYPE,
                format!("multipart/x-mixed-replace; boundary={BOUNDARY}"),
            ),
            (axum::http::header::CACHE_CONTROL, "no-cache".to_string()),
        ],
        body,
    )
        .into_response()
}

/// A [`FrameSink`] that JPEG-encodes each composited frame and broadcasts it to
/// the MJPEG stream. Dropping frames when no `<img>` is connected (or the channel
/// is full) is intentional — playback never blocks on the transport.
#[derive(Clone)]
pub struct MjpegSink {
    tx: broadcast::Sender<Bytes>,
}

impl FrameSink for MjpegSink {
    fn push_frame(&self, frame: &DecodedFrame) {
        // No subscribers → nothing to encode (saves CPU when the preview `<img>`
        // is not mounted).
        if self.tx.receiver_count() == 0 {
            return;
        }
        if let Some(jpeg) = encode_jpeg(frame) {
            let _ = self.tx.send(Bytes::from(jpeg));
        }
    }
}

/// Encode an RGBA composite to JPEG (alpha dropped — the preview canvas is
/// opaque). Returns `None` on an encode error (logged, frame skipped).
fn encode_jpeg(frame: &DecodedFrame) -> Option<Vec<u8>> {
    // JPEG has no alpha: pack RGBA → RGB.
    let mut rgb = Vec::with_capacity((frame.width * frame.height * 3) as usize);
    for px in frame.rgba.chunks_exact(4) {
        rgb.extend_from_slice(&px[..3]);
    }
    let mut out = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    match enc.encode(
        &rgb,
        frame.width,
        frame.height,
        image::ExtendedColorType::Rgb8,
    ) {
        Ok(()) => Some(out),
        Err(e) => {
            eprintln!("[mjpeg] jpeg encode failed: {e}");
            None
        }
    }
}

/// Playhead frame number broadcast to the front end, so it can move the
/// playhead / timecode while the pixels arrive over the MJPEG stream.
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayheadDto {
    frame: i32,
}

/// A [`PlayheadEmitter`] that emits the current frame as a Tauri `playback_frame`
/// event. Throttling is unnecessary: one small event per rendered frame.
pub struct TauriPlayheadEmitter {
    app: AppHandle,
}

impl TauriPlayheadEmitter {
    pub fn new(app: AppHandle) -> Self {
        TauriPlayheadEmitter { app }
    }
}

impl PlayheadEmitter for TauriPlayheadEmitter {
    fn emit(&self, frame: i32) {
        let _ = self.app.emit("playback_frame", PlayheadDto { frame });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn origin_guard_allows_missing_and_loopback_origins() {
        let empty = HeaderMap::new();
        assert!(origin_is_allowed(&empty), "no Origin (plain <img>) allowed");

        for ok in [
            "http://127.0.0.1:1420",
            "http://localhost:1420",
            "tauri://localhost",
            "http://tauri.localhost",
        ] {
            let mut h = HeaderMap::new();
            h.insert(
                axum::http::header::ORIGIN,
                HeaderValue::from_str(ok).unwrap(),
            );
            assert!(origin_is_allowed(&h), "{ok} should be allowed");
        }
    }

    #[test]
    fn origin_guard_rejects_remote_origin() {
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::ORIGIN,
            HeaderValue::from_static("http://evil.example.com"),
        );
        assert!(!origin_is_allowed(&h));
    }

    #[test]
    fn jpeg_encode_produces_jpeg_magic() {
        // 2x2 opaque RGBA → a valid JPEG starting with the SOI marker 0xFFD8.
        let frame = DecodedFrame::new(2, 2, vec![255; 2 * 2 * 4], false);
        let jpeg = encode_jpeg(&frame).expect("encode");
        assert!(jpeg.len() > 2);
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "JPEG SOI marker");
    }

    #[test]
    fn multipart_part_header_is_well_formed() {
        // Sanity-check the boundary framing the handler emits.
        let len = 1234;
        let header = format!(
            "\r\n--{BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {len}\r\n\r\n"
        );
        assert!(header.starts_with("\r\n--opentake_mjpeg_boundary\r\n"));
        assert!(header.contains("Content-Type: image/jpeg"));
        assert!(header.ends_with("\r\n\r\n"));
    }
}
