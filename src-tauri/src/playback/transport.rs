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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Query;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast;

use opentake_render::DecodedFrame;

use super::engine::{FrameSink, PlayheadEmitter};
use super::session::PlaybackIdentity;

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
    latest: LatestFrameStore,
}

/// Shared axum state: the live broadcast sender plus the latest encoded frame
/// (for the polling `/frame` route).
#[derive(Clone)]
struct ServerState {
    tx: broadcast::Sender<Bytes>,
    latest: LatestFrameStore,
}

#[derive(Clone, Debug)]
struct LatestFrame {
    identity: PlaybackIdentity,
    frame: i32,
    sequence: u64,
    terminal: bool,
    jpeg: Bytes,
}

#[derive(Clone, Default)]
struct LatestFrameStore(Arc<RwLock<Option<LatestFrame>>>);

impl LatestFrameStore {
    fn publish(
        &self,
        identity: PlaybackIdentity,
        frame: i32,
        sequence: u64,
        terminal: bool,
        jpeg: Bytes,
    ) {
        *self
            .0
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(LatestFrame {
            identity,
            frame,
            sequence,
            terminal,
            jpeg,
        });
    }

    fn lookup(&self, query: &FrameQuery) -> Option<Bytes> {
        let latest = self
            .0
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        latest.as_ref().and_then(|latest| {
            (latest.identity.project_epoch == query.project_epoch
                && latest.identity.timeline_version == query.timeline_version
                && latest.identity.session_id == query.session_id
                && latest.frame == query.frame
                && latest.sequence == query.sequence)
                .then(|| latest.jpeg.clone())
        })
    }

    fn clear_session(&self, identity: &PlaybackIdentity) {
        let mut latest = self
            .0
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if latest
            .as_ref()
            .is_some_and(|latest| &latest.identity == identity)
        {
            *latest = None;
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrameQuery {
    project_epoch: u64,
    timeline_version: u64,
    session_id: String,
    frame: i32,
    sequence: u64,
}

impl FrameQuery {
    fn new(
        project_epoch: u64,
        timeline_version: u64,
        session_id: impl Into<String>,
        frame: i32,
        sequence: u64,
    ) -> Self {
        Self {
            project_epoch,
            timeline_version,
            session_id: session_id.into(),
            frame,
            sequence,
        }
    }

    fn valid(&self) -> bool {
        self.frame >= 0
            && PlaybackIdentity::new(
                self.project_epoch,
                self.timeline_version,
                self.session_id.clone(),
            )
            .is_ok()
    }
}

#[derive(Clone, Default)]
pub struct PublicationGate(Arc<Mutex<bool>>);

impl PublicationGate {
    pub fn open() -> Self {
        let gate = Self::default();
        *gate
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        gate
    }

    pub fn close(&self) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = false;
    }

    pub fn reopen(&self) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    }

    pub(crate) fn is_open(&self) -> bool {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn with_open<T>(&self, publish: impl FnOnce() -> T) -> Option<T> {
        let open = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !*open {
            return None;
        }
        Some(publish())
    }
}

#[derive(Clone)]
struct PendingFrame {
    identity: PlaybackIdentity,
    sequence: u64,
    jpeg: Bytes,
}

#[derive(Clone, Default)]
struct PendingFrameStore(Arc<Mutex<Option<PendingFrame>>>);

impl PreviewServer {
    /// Start the MJPEG server on a random loopback port. Must run inside the
    /// Tauri async runtime (call via `tauri::async_runtime::block_on` in setup).
    pub async fn start() -> Result<Arc<Self>, String> {
        let (tx, _rx) = broadcast::channel::<Bytes>(FRAME_CHANNEL_DEPTH);
        let latest = LatestFrameStore::default();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("MJPEG bind: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("MJPEG local_addr: {e}"))?
            .port();

        let state = ServerState {
            tx: tx.clone(),
            latest: latest.clone(),
        };
        tauri::async_runtime::spawn(async move {
            let app = axum::Router::new()
                .route("/stream", axum::routing::get(stream_handler))
                .route("/ws", axum::routing::get(ws_handler))
                .route("/frame", axum::routing::get(frame_handler))
                .with_state(state);
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("[mjpeg] server error: {e}");
            }
        });

        Ok(Arc::new(Self { port, tx, latest }))
    }

    /// The `<img>`-pointable MJPEG stream URL. Kept for debugging; the preview
    /// canvas uses [`Self::endpoint_ws`] instead (WebKit only paints the first
    /// part of a `multipart/x-mixed-replace` `<img>` — see the module note).
    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/stream", self.port)
    }

    /// The WebSocket URL the preview canvas connects to for binary JPEG frames.
    /// WebKit/WKWebView renders these reliably (WebSocket + `createImageBitmap` +
    /// canvas), which the MJPEG `<img>` path does not.
    pub fn endpoint_ws(&self) -> String {
        format!("ws://127.0.0.1:{}/ws", self.port)
    }

    /// The single-frame poll URL (`GET /frame` -> latest JPEG). This is what the
    /// preview `<img>` actually uses: WKWebView's secure `tauri://` context
    /// blocks plain-`ws://` WebSockets to loopback as mixed content (silently —
    /// no TCP connect ever happens), while a passive `<img>` load over loopback
    /// http is allowed. The playhead's `playback_frame` event drives one `<img>`
    /// reload per rendered frame.
    pub fn endpoint_frame(&self) -> String {
        format!("http://127.0.0.1:{}/frame", self.port)
    }

    /// A frame sink that JPEG-encodes composited frames into this server's stream.
    pub fn sink(&self, identity: PlaybackIdentity, gate: PublicationGate) -> MjpegSink {
        MjpegSink {
            tx: self.tx.clone(),
            pending: PendingFrameStore::default(),
            sequence: Arc::new(AtomicU64::new(0)),
            identity,
            gate,
        }
    }

    pub fn clear_session(&self, identity: &PlaybackIdentity) {
        self.latest.clear_session(identity);
    }

    /// Publish one already-encoded session frame into the exact `/frame` lookup
    /// store. The render emitter uses the same store after sink encoding; this
    /// boundary is also useful to validate the live HTTP route independently of
    /// Tauri event delivery.
    pub fn publish_encoded_frame(
        &self,
        identity: PlaybackIdentity,
        frame: i32,
        sequence: u64,
        terminal: bool,
        jpeg: Bytes,
    ) {
        self.latest
            .publish(identity, frame, sequence, terminal, jpeg);
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
async fn stream_handler(State(state): State<ServerState>, headers: HeaderMap) -> Response {
    if !origin_is_allowed(&headers) {
        return (StatusCode::FORBIDDEN, "cross-origin preview stream denied").into_response();
    }

    let mut rx = state.tx.subscribe();
    // Bridge the broadcast receiver to an axum body stream via a BOUNDED mpsc: a
    // slow client drops frames (live preview) instead of growing memory without
    // limit.
    let (body_tx, body_rx) = tokio::sync::mpsc::channel::<Result<Bytes, Infallible>>(4);

    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(jpeg) => {
                    // Pack header + body into ONE multipart part: a part must never
                    // be split across sends, or a dropped half corrupts the stream.
                    let header = format!(
                        "\r\n--{BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        jpeg.len()
                    );
                    let mut part = Vec::with_capacity(header.len() + jpeg.len());
                    part.extend_from_slice(header.as_bytes());
                    part.extend_from_slice(&jpeg);
                    match body_tx.try_send(Ok(Bytes::from(part))) {
                        Ok(()) => {}
                        // Client can't keep up: drop this frame, keep streaming.
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => continue,
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break,
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

/// `/ws`: push each broadcast JPEG to the preview canvas as a binary WebSocket
/// message. WebKit/WKWebView consumes these reliably; the `/stream` MJPEG `<img>`
/// path only ever paints the first frame there (see the module note).
async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<ServerState>,
) -> Response {
    if !origin_is_allowed(&headers) {
        return (StatusCode::FORBIDDEN, "cross-origin preview stream denied").into_response();
    }
    ws.on_upgrade(move |socket| ws_stream(socket, state.tx.subscribe()))
}

/// `/frame`: one session-scoped composited JPEG. The preview requests the exact
/// project epoch, timeline version, session id, frame, and publication sequence;
/// a mismatch returns 204. WKWebView permits this passive loopback image request
/// while blocking plain `ws://` in the secure `tauri://` context.
async fn frame_handler(
    State(state): State<ServerState>,
    Query(query): Query<FrameQuery>,
    headers: HeaderMap,
) -> Response {
    if !origin_is_allowed(&headers) {
        return (StatusCode::FORBIDDEN, "cross-origin preview stream denied").into_response();
    }
    if !query.valid() {
        return (StatusCode::NO_CONTENT, "").into_response();
    }
    match state.latest.lookup(&query) {
        Some(jpeg) => (
            [
                (axum::http::header::CONTENT_TYPE, "image/jpeg".to_string()),
                (
                    axum::http::header::CACHE_CONTROL,
                    "no-store, no-cache, must-revalidate".to_string(),
                ),
            ],
            jpeg,
        )
            .into_response(),
        None => (StatusCode::NO_CONTENT, "").into_response(),
    }
}

/// Forward broadcast JPEG frames to one connected preview socket until it closes.
/// A slow/dead socket drops frames (live preview never back-pressures the render
/// thread) or ends the loop; encoding stops once the last subscriber is gone.
async fn ws_stream(mut socket: WebSocket, mut rx: broadcast::Receiver<Bytes>) {
    loop {
        match rx.recv().await {
            Ok(jpeg) => {
                if socket.send(Message::Binary(jpeg)).await.is_err() {
                    break;
                }
            }
            // Slow consumer: skip the dropped frames and keep going (live preview).
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// A [`FrameSink`] that JPEG-encodes each composited frame and broadcasts it to
/// the MJPEG stream. Dropping frames when no `<img>` is connected (or the channel
/// is full) is intentional — playback never blocks on the transport.
#[derive(Clone)]
pub struct MjpegSink {
    tx: broadcast::Sender<Bytes>,
    pending: PendingFrameStore,
    sequence: Arc<AtomicU64>,
    identity: PlaybackIdentity,
    gate: PublicationGate,
}

impl FrameSink for MjpegSink {
    fn push_frame(&self, frame: &DecodedFrame) {
        if !self.gate.is_open() {
            return;
        }
        // Always encode: the polling `/frame` route reads `latest` without ever
        // subscribing to the broadcast channel, so receiver_count()==0 no longer
        // means "nobody is watching". Playback always has exactly one consumer
        // (the preview `<img>`), so the old idle-skip saved nothing real.
        let Some(jpeg) = encode_jpeg(frame) else {
            return;
        };
        let jpeg = Bytes::from(jpeg);
        let _ = self.gate.with_open(|| {
            let sequence = self.sequence.fetch_add(1, Ordering::AcqRel) + 1;
            *self
                .pending
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(PendingFrame {
                identity: self.identity.clone(),
                sequence,
                jpeg: jpeg.clone(),
            });
            if self.tx.receiver_count() > 0 {
                let _ = self.tx.send(jpeg);
            }
        });
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
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayheadDto {
    project_epoch: u64,
    timeline_version: u64,
    session_id: String,
    frame: i32,
    sequence: u64,
    terminal: bool,
}

impl PlayheadDto {
    fn new(identity: PlaybackIdentity, frame: i32, sequence: u64, terminal: bool) -> Self {
        Self {
            project_epoch: identity.project_epoch,
            timeline_version: identity.timeline_version,
            session_id: identity.session_id,
            frame,
            sequence,
            terminal,
        }
    }
}

/// A [`PlayheadEmitter`] that emits the current frame as a Tauri `playback_frame`
/// event. Throttling is unnecessary: one small event per rendered frame.
pub struct TauriPlayheadEmitter {
    app: AppHandle,
    identity: PlaybackIdentity,
    gate: PublicationGate,
    pending: PendingFrameStore,
    latest: LatestFrameStore,
    last_frame: i32,
}

impl TauriPlayheadEmitter {
    pub fn new(
        app: AppHandle,
        server: &PreviewServer,
        sink: &MjpegSink,
        identity: PlaybackIdentity,
        gate: PublicationGate,
        last_frame: i32,
    ) -> Self {
        TauriPlayheadEmitter {
            app,
            identity,
            gate,
            pending: sink.pending.clone(),
            latest: server.latest.clone(),
            last_frame,
        }
    }
}

impl PlayheadEmitter for TauriPlayheadEmitter {
    fn emit(&self, frame: i32) {
        let _ = self.gate.with_open(|| {
            let pending = self
                .pending
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            let Some(pending) = pending else {
                return;
            };
            if pending.identity != self.identity {
                return;
            }
            let terminal = frame >= self.last_frame;
            self.latest.publish(
                self.identity.clone(),
                frame,
                pending.sequence,
                terminal,
                pending.jpeg,
            );
            let _ = self.app.emit(
                "playback_frame",
                PlayheadDto::new(self.identity.clone(), frame, pending.sequence, terminal),
            );
        });
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

    #[test]
    fn playhead_event_carries_session_revision_sequence_and_terminal() {
        let identity = super::super::session::PlaybackIdentity::new(7, 11, "session-42")
            .expect("valid identity");
        let dto = PlayheadDto::new(identity, 123, 9, true);

        assert_eq!(
            serde_json::to_value(dto).expect("serialize"),
            serde_json::json!({
                "projectEpoch": 7,
                "timelineVersion": 11,
                "sessionId": "session-42",
                "frame": 123,
                "sequence": 9,
                "terminal": true,
            })
        );
    }

    #[test]
    fn frame_route_never_serves_another_session_latest() {
        let latest = LatestFrameStore::default();
        let identity =
            super::super::session::PlaybackIdentity::new(3, 5, "current").expect("valid identity");
        latest.publish(identity.clone(), 18, 4, false, Bytes::from_static(b"jpeg"));

        assert!(latest
            .lookup(&FrameQuery::new(3, 5, "stale", 18, 4))
            .is_none());
        assert!(latest
            .lookup(&FrameQuery::new(2, 5, "current", 18, 4))
            .is_none());
        assert!(latest
            .lookup(&FrameQuery::new(3, 4, "current", 18, 4))
            .is_none());
        assert!(latest
            .lookup(&FrameQuery::new(3, 5, "current", 18, 3))
            .is_none());
        assert_eq!(
            latest.lookup(&FrameQuery::new(3, 5, "current", 18, 4)),
            Some(Bytes::from_static(b"jpeg"))
        );
    }
}
