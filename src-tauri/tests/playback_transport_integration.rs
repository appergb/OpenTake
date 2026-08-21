//! Fail-closed live HTTP transport integration for native playback.
#![cfg(feature = "playback-engine")]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use opentake_domain::{
    Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, Timeline, Track,
};
use opentake_media::{decode_frame_at, FrameRequest};
use opentake_render::DecodedFrame;
use opentake_render::RenderSize;
use opentake_tauri_lib::playback::session::PlaybackIdentity;
use opentake_tauri_lib::playback::transport::PublicationGate;
use opentake_tauri_lib::playback::{
    project_media, project_text, FrameSink, PreviewServer, RenderLoop,
};

const BOUNDARY: &[u8] = b"\r\n--opentake_mjpeg_boundary\r\n";
const WARMUP_TRIES: usize = 200;
const WARMUP_SLEEP: Duration = Duration::from_millis(10);

struct HttpHead {
    status: u16,
    headers: BTreeMap<String, String>,
}

fn port_of(endpoint: &str) -> u16 {
    endpoint
        .rsplit(':')
        .next()
        .and_then(|tail| tail.split('/').next())
        .and_then(|port| port.parse().ok())
        .expect("endpoint carries a port")
}

fn start_server() -> Arc<PreviewServer> {
    tauri::async_runtime::block_on(PreviewServer::start())
        .expect("preview server must bind its loopback port")
}

fn ffmpeg_ready() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn make_distinct_cfr_video(path: &Path, w: u32, h: u32, fps: u32, frames: u32) {
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc2=size={w}x{h}:rate={fps}"),
            "-frames:v",
            &frames.to_string(),
            "-c:v",
            "libx264",
            "-g",
            &frames.to_string(),
            "-keyint_min",
            &frames.to_string(),
            "-sc_threshold",
            "0",
            "-pix_fmt",
            "yuv420p",
            "-fps_mode",
            "cfr",
            "-y",
        ])
        .arg(path)
        .output()
        .expect("required ffmpeg must start for transport parity fixture");
    assert!(
        output.status.success(),
        "generate transport parity fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn external_entry(id: &str, path: &Path, w: i32, h: i32, fps: f64) -> MediaManifestEntry {
    MediaManifestEntry {
        id: id.into(),
        name: format!("{id}.mp4"),
        kind: ClipType::Video,
        source: MediaSource::External {
            absolute_path: path.to_string_lossy().into_owned(),
        },
        duration: 1.0,
        generation_input: None,
        source_width: Some(w),
        source_height: Some(h),
        source_fps: Some(fps),
        has_audio: Some(false),
        color: None,
        proxy: None,
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    }
}

fn try_render_loop(
    timeline: Timeline,
    manifest: &MediaManifest,
    render_size: RenderSize,
) -> Option<RenderLoop> {
    let (sizes, media) = project_media(manifest, &None);
    let text = project_text(&timeline);
    match RenderLoop::new(timeline, media, text, sizes, render_size) {
        Ok(render_loop) => Some(render_loop),
        Err(error) if error.contains("no GPU device") => {
            eprintln!("skip: no GPU adapter available ({error})");
            None
        }
        Err(error) => panic!("render loop init failed: {error}"),
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn open_request(port: u16, path: &str, extra_headers: &str) -> (TcpStream, HttpHead, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect loopback");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("set read timeout");
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n{extra_headers}Connection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).expect("write request");

    let mut received = Vec::new();
    let header_end = loop {
        if let Some(index) = find_bytes(&received, b"\r\n\r\n") {
            break index + 4;
        }
        let mut chunk = [0u8; 1024];
        let count = stream.read(&mut chunk).expect("read response head");
        assert!(count > 0, "connection closed before response head");
        received.extend_from_slice(&chunk[..count]);
        assert!(received.len() <= 64 * 1024, "response head is unbounded");
    };
    let head_text = std::str::from_utf8(&received[..header_end]).expect("ASCII response head");
    let mut lines = head_text.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse().ok())
        .expect("numeric HTTP status");
    let mut headers = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').expect("well-formed response header");
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }
    let remainder = received.split_off(header_end);
    (stream, HttpHead { status, headers }, remainder)
}

fn finite_get(port: u16, path: &str, extra_headers: &str) -> (HttpHead, Vec<u8>) {
    let (mut stream, head, mut body) = open_request(port, path, extra_headers);
    let content_length = head
        .headers
        .get("content-length")
        .map(|value| value.parse::<usize>().expect("numeric Content-Length"))
        .unwrap_or_else(|| {
            assert_eq!(head.status, 204, "finite response needs Content-Length");
            0
        });
    assert!(
        body.len() <= content_length,
        "received more than declared Content-Length"
    );
    let already_read = body.len();
    body.resize(content_length, 0);
    stream
        .read_exact(&mut body[already_read..])
        .expect("read complete Content-Length body");
    (head, body)
}

fn frame_path(identity: &PlaybackIdentity, frame: i32, sequence: u64) -> String {
    format!(
        "/frame?projectEpoch={}&timelineVersion={}&sessionId={}&frame={frame}&sequence={sequence}",
        identity.project_epoch, identity.timeline_version, identity.session_id
    )
}

fn solid_frame(width: u32, height: u32, rgb: [u8; 3]) -> DecodedFrame {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..width * height {
        rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
    }
    DecodedFrame::new(width, height, rgba, false)
}

fn render_until_matches(
    render_loop: &mut RenderLoop,
    target: i32,
    expected_rgba: &[u8],
    width: u32,
    height: u32,
) -> Option<DecodedFrame> {
    for _ in 0..WARMUP_TRIES {
        let frame = render_loop
            .render_frame(target)
            .expect("render target frame");
        assert_eq!((frame.width, frame.height), (width, height));
        if frame.rgba == expected_rgba {
            return Some(frame);
        }
        sleep(WARMUP_SLEEP);
    }
    None
}

struct ChunkedReader {
    stream: TcpStream,
    raw: Vec<u8>,
}

impl ChunkedReader {
    fn next(&mut self) -> Vec<u8> {
        loop {
            if let Some(line_end) = find_bytes(&self.raw, b"\r\n") {
                let size_text = std::str::from_utf8(&self.raw[..line_end])
                    .expect("ASCII chunk size")
                    .split(';')
                    .next()
                    .expect("chunk size field");
                let size = usize::from_str_radix(size_text.trim(), 16).expect("hex chunk size");
                let payload_start = line_end + 2;
                let payload_end = payload_start + size;
                if self.raw.len() >= payload_end + 2 {
                    assert_ne!(size, 0, "stream closed before two JPEG parts");
                    assert_eq!(&self.raw[payload_end..payload_end + 2], b"\r\n");
                    let payload = self.raw[payload_start..payload_end].to_vec();
                    self.raw.drain(..payload_end + 2);
                    return payload;
                }
            }
            let mut chunk = [0u8; 4096];
            let count = self.stream.read(&mut chunk).expect("read stream chunk");
            assert!(count > 0, "stream closed before complete chunk");
            self.raw.extend_from_slice(&chunk[..count]);
        }
    }
}

fn complete_multipart_parts(reader: &mut ChunkedReader, count: usize) -> Vec<Vec<u8>> {
    let mut decoded = Vec::new();
    loop {
        decoded.extend_from_slice(&reader.next());
        let mut parts = Vec::new();
        let mut cursor = 0;
        while let Some(relative) = find_bytes(&decoded[cursor..], BOUNDARY) {
            let header_start = cursor + relative + BOUNDARY.len();
            let Some(relative_end) = find_bytes(&decoded[header_start..], b"\r\n\r\n") else {
                break;
            };
            let header_end = header_start + relative_end;
            let header_text = std::str::from_utf8(&decoded[header_start..header_end])
                .expect("ASCII multipart headers");
            let content_length = header_text
                .split("\r\n")
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().expect("multipart length"))
                })
                .expect("multipart Content-Length");
            let body_start = header_end + 4;
            let body_end = body_start + content_length;
            if decoded.len() < body_end {
                break;
            }
            parts.push(decoded[body_start..body_end].to_vec());
            cursor = body_end;
        }
        if parts.len() >= count {
            return parts;
        }
    }
}

#[test]
fn frame_route_transitions_from_204_to_valid_200_jpeg() {
    let server = start_server();
    let port = port_of(&server.endpoint());
    let identity = PlaybackIdentity::new(7, 11, "session-frame-transition").unwrap();
    let path = frame_path(&identity, 4, 1);
    let (empty, body) = finite_get(port, &path, "");
    assert_eq!(empty.status, 204);
    assert!(body.is_empty());

    let sink = server.sink(identity.clone(), PublicationGate::open());
    sink.push_frame(&solid_frame(3, 2, [220, 20, 20]));
    sink.publication().commit(4, 20).expect("commit frame");

    let (ready, jpeg) = finite_get(port, &path, "");
    assert_eq!(ready.status, 200);
    assert_eq!(
        ready.headers.get("content-type").map(String::as_str),
        Some("image/jpeg")
    );
    let image = image::load_from_memory(&jpeg).expect("decode complete JPEG body");
    assert_eq!((image.width(), image.height()), (3, 2));
}

#[test]
fn frame_route_returns_complete_decodable_jpeg_body() {
    let server = start_server();
    let port = port_of(&server.endpoint());
    let identity = PlaybackIdentity::new(3, 5, "session-complete-body").unwrap();
    let sink = server.sink(identity.clone(), PublicationGate::open());
    sink.push_frame(&solid_frame(5, 4, [10, 200, 40]));
    sink.publication().commit(9, 30).expect("commit frame");

    let (head, jpeg) = finite_get(port, &frame_path(&identity, 9, 1), "");
    assert_eq!(head.status, 200);
    assert_eq!(
        head.headers["content-length"].parse::<usize>().unwrap(),
        jpeg.len()
    );
    let image = image::load_from_memory(&jpeg).expect("decode complete JPEG body");
    assert_eq!((image.width(), image.height()), (5, 4));
}

#[test]
fn frame_route_rejects_cross_origin() {
    let server = start_server();
    let identity = PlaybackIdentity::new(1, 0, "session-origin").unwrap();
    let (head, _) = finite_get(
        port_of(&server.endpoint()),
        &frame_path(&identity, 0, 1),
        "Origin: http://127.0.0.1.evil.example\r\n",
    );
    assert_eq!(head.status, 403);
}

#[test]
fn frame_route_returns_204_for_wrong_session_identity() {
    let server = start_server();
    let port = port_of(&server.endpoint());
    let identity = PlaybackIdentity::new(2, 8, "session-current").unwrap();
    let sink = server.sink(identity.clone(), PublicationGate::open());
    sink.push_frame(&solid_frame(2, 2, [80, 90, 100]));
    sink.publication().commit(6, 10).expect("commit frame");

    let wrong = PlaybackIdentity::new(2, 8, "session-replaced").unwrap();
    let (head, body) = finite_get(port, &frame_path(&wrong, 6, 1), "");
    assert_eq!(head.status, 204);
    assert!(body.is_empty());
}

#[test]
fn stream_route_delivers_two_distinct_complete_jpeg_parts() {
    let server = start_server();
    let port = port_of(&server.endpoint());
    let (stream, head, remainder) = open_request(port, "/stream", "");
    assert_eq!(head.status, 200);
    assert!(head.headers["content-type"].contains("multipart/x-mixed-replace"));
    assert_eq!(
        head.headers.get("transfer-encoding").map(String::as_str),
        Some("chunked")
    );

    let identity = PlaybackIdentity::new(9, 2, "session-stream").unwrap();
    let sink = server.sink(identity, PublicationGate::open());
    sink.push_frame(&solid_frame(2, 2, [255, 0, 0]));
    sink.push_frame(&solid_frame(4, 3, [0, 0, 255]));

    let mut reader = ChunkedReader {
        stream,
        raw: remainder,
    };
    let parts = complete_multipart_parts(&mut reader, 2);
    assert_ne!(parts[0], parts[1]);
    let first = image::load_from_memory(&parts[0]).expect("decode first complete JPEG");
    let second = image::load_from_memory(&parts[1]).expect("decode second complete JPEG");
    assert_eq!((first.width(), first.height()), (2, 2));
    assert_eq!((second.width(), second.height()), (4, 3));
}

#[test]
fn stream_route_rejects_cross_origin() {
    let server = start_server();
    let (head, _) = finite_get(
        port_of(&server.endpoint()),
        "/stream",
        "Origin: http://localhost.evil.example\r\n",
    );
    assert_eq!(head.status, 403);
}

#[test]
fn frame_route_preserves_fractional_speed_publication_order_and_decodability() {
    if !ffmpeg_ready() {
        eprintln!("skip: ffmpeg not available");
        return;
    }

    let dir = tempfile::tempdir().expect("fixture tempdir");
    let src = dir.path().join("speed-15-cfr.mp4");
    let (w, h, fps, frames) = (160u32, 90u32, 12u32, 12u32);
    make_distinct_cfr_video(&src, w, h, fps, frames);

    let mut timeline = Timeline::new();
    timeline.fps = fps as i32;
    let mut track = Track::new("t1", ClipType::Video);
    let mut clip = Clip::new("clip-1", "asset-1", 0, 6);
    clip.trim_start_frame = 2;
    clip.speed = 1.5;
    track.clips.push(clip);
    timeline.tracks.push(track);

    let mut manifest = MediaManifest::new();
    manifest.entries.push(external_entry(
        "asset-1", &src, w as i32, h as i32, fps as f64,
    ));

    let Some(mut render_loop) = try_render_loop(timeline, &manifest, RenderSize::new(w, h)) else {
        return;
    };

    let server = start_server();
    let port = port_of(&server.endpoint());
    let identity = PlaybackIdentity::new(19, 27, "session-speed-15").unwrap();
    let sink = server.sink(identity.clone(), PublicationGate::open());
    let publication = sink.publication();
    let targets = [(0, 2), (3, 7), (5, 10)];
    let last_frame = targets.last().expect("terminal target").0;
    let mut emitted = Vec::new();

    for (target, source_frame) in targets {
        let request = FrameRequest {
            time_secs: source_frame as f64 / fps as f64,
            max_size: (w, h),
            tolerance_secs: 0.0,
            apply_rotation: true,
        };
        let (_, expected) = decode_frame_at(&src, &request).expect("decode exact source frame");
        let frame = render_until_matches(&mut render_loop, target, &expected.rgba, w, h)
            .expect("render loop should converge to the planned source frame");
        sink.push_frame(&frame);
        let event = publication
            .commit(target, last_frame)
            .expect("commit staged playback frame");
        let payload = serde_json::to_value(&event).expect("serialize playback publication");
        let sequence = payload["sequence"]
            .as_u64()
            .expect("publication sequence is numeric");
        let (head, jpeg) = finite_get(port, &frame_path(&identity, target, sequence), "");
        assert_eq!(head.status, 200, "published frame {target} should resolve");
        let image = image::load_from_memory(&jpeg).expect("decode published playback JPEG");
        assert_eq!((image.width(), image.height()), (w, h));
        emitted.push(payload);
    }

    assert_eq!(
        emitted
            .iter()
            .map(|payload| payload["frame"].as_i64().expect("frame integer"))
            .collect::<Vec<_>>(),
        vec![0, 3, 5]
    );
    assert_eq!(
        emitted
            .iter()
            .map(|payload| payload["sequence"].as_u64().expect("sequence integer"))
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        emitted
            .iter()
            .map(|payload| payload["terminal"].as_bool().expect("terminal bool"))
            .collect::<Vec<_>>(),
        vec![false, false, true]
    );
}
