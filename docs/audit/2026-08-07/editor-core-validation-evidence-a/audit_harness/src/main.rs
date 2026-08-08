use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use opentake_domain::{Clip, ClipType, Timeline, Track};
use opentake_render::RenderSize;
use opentake_tauri_lib::playback::session::PlaybackIdentity;
use opentake_tauri_lib::playback::transport::{EncodedFramePublication, PublicationGate};
use opentake_tauri_lib::playback::{
    FrameSink, InstantClock, MediaInfo, PlaybackClock, PlaybackEngine, PlayheadEmitter,
    PreviewServer,
};

struct CommitEmitter {
    publication: EncodedFramePublication,
    last_frame: i32,
    published: AtomicI32,
    newest_frame: AtomicI32,
}

impl PlayheadEmitter for CommitEmitter {
    fn emit(&self, frame: i32) {
        if self.publication.commit(frame, self.last_frame).is_some() {
            self.published.fetch_add(1, Ordering::SeqCst);
            self.newest_frame.store(frame, Ordering::SeqCst);
        }
    }
}

fn video_clip(id: &str, media_ref: &str, duration: i32) -> Clip {
    let mut clip = Clip::new(id, media_ref, 0, duration);
    clip.media_type = ClipType::Video;
    clip.source_clip_type = ClipType::Video;
    clip
}

fn http_frame(endpoint: &str, identity: &PlaybackIdentity, frame: i32) -> (u16, Vec<u8>) {
    let address = endpoint
        .strip_prefix("http://")
        .and_then(|rest| rest.strip_suffix("/frame"))
        .expect("loopback endpoint");
    let mut stream = TcpStream::connect(address).expect("connect loopback frame server");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set read timeout");
    write!(
        stream,
        "GET /frame?projectEpoch={}&timelineVersion={}&sessionId={}&frame={frame}&sequence=1 HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n",
        identity.project_epoch, identity.timeline_version, identity.session_id,
    )
    .expect("write HTTP request");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).expect("read HTTP response");
    let header_end = response
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .map(|index| index + 4)
        .expect("HTTP response header");
    let status = String::from_utf8_lossy(&response[..header_end])
        .split_whitespace()
        .nth(1)
        .expect("HTTP status")
        .parse()
        .expect("numeric HTTP status");
    (status, response[header_end..].to_vec())
}

fn main() {
    let source = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: audit-harness <4k60-video>");
    assert!(source.is_file(), "fixture is not a regular file: {}", source.display());

    let frames = 240;
    let mut timeline = Timeline::new();
    timeline.width = 3840;
    timeline.height = 2160;
    timeline.fps = 60;
    let mut lower = Track::new("track-lower", ClipType::Video);
    lower.clips.push(video_clip("clip-lower", "media-lower", frames));
    let mut upper = Track::new("track-upper", ClipType::Video);
    upper.clips.push(video_clip("clip-upper", "media-upper", frames));
    timeline.tracks.extend([lower, upper]);

    let mut media = HashMap::new();
    for media_ref in ["media-lower", "media-upper"] {
        media.insert(
            media_ref.to_owned(),
            MediaInfo {
                path: source.clone(),
                straight_alpha: false,
            },
        );
    }
    let sizes = HashMap::from([
        ("media-lower".to_owned(), (3840, 2160)),
        ("media-upper".to_owned(), (3840, 2160)),
    ]);

    let identity = PlaybackIdentity::new(77, 9, "audit-4k60-dual-track").unwrap();
    let server = tauri::async_runtime::block_on(PreviewServer::start())
        .expect("start loopback preview server");
    let sink = server.sink(identity.clone(), PublicationGate::open());
    let emitter = Arc::new(CommitEmitter {
        publication: sink.publication(),
        last_frame: frames - 1,
        published: AtomicI32::new(0),
        newest_frame: AtomicI32::new(-1),
    });
    let clock = Arc::new(InstantClock::new(0)) as Arc<dyn PlaybackClock>;
    let engine = PlaybackEngine::spawn_ready(
        timeline,
        media,
        HashMap::new(),
        sizes,
        RenderSize::new(1280, 720),
        clock,
        Arc::new(sink) as Arc<dyn FrameSink>,
        Arc::clone(&emitter) as Arc<dyn PlayheadEmitter>,
        0,
    )
    .expect("real GPU engine ready");
    engine.resume(0).expect("resume real engine");

    let started = Instant::now();
    let deadline = started + Duration::from_secs(3);
    let mut delayed_frames = VecDeque::new();
    let mut http_200 = 0;
    let mut http_204 = 0;
    let mut invalid_jpeg = 0;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(120));
        let newest = emitter.newest_frame.load(Ordering::SeqCst);
        if newest < 0 {
            continue;
        }
        delayed_frames.push_back(newest);
        let requested = if delayed_frames.len() >= 3 {
            delayed_frames.pop_front().unwrap()
        } else {
            0
        };
        let (status, body) = http_frame(&server.endpoint_frame(), &identity, requested);
        match status {
            200 => {
                http_200 += 1;
                if !body.windows(2).any(|bytes| bytes == [0xff, 0xd8]) {
                    invalid_jpeg += 1;
                }
            }
            204 => http_204 += 1,
            other => panic!("unexpected frame HTTP status {other}"),
        }
    }
    engine.stop();

    let published = emitter.published.load(Ordering::SeqCst);
    let newest = emitter.newest_frame.load(Ordering::SeqCst);
    let (future_status, _) = http_frame(&server.endpoint_frame(), &identity, newest + 1);
    let foreign = PlaybackIdentity::new(77, 9, "audit-foreign-session").unwrap();
    let (foreign_status, _) = http_frame(&server.endpoint_frame(), &foreign, newest.max(0));
    println!(
        "audit_result published={published} newest_frame={newest} http_200={http_200} http_204={http_204} invalid_jpeg={invalid_jpeg} future_status={future_status} foreign_status={foreign_status} elapsed_ms={}",
        started.elapsed().as_millis()
    );

    assert!(published >= 3, "heavy engine failed to publish consecutive frames");
    assert!(newest >= 2, "heavy engine playhead did not advance");
    assert!(http_200 >= 2, "slow HTTP consumer received no usable frames");
    assert_eq!(http_204, 0, "slow same-session consumer regressed to 204 freeze");
    assert_eq!(invalid_jpeg, 0, "frame endpoint returned invalid JPEG data");
    assert_eq!(future_status, 204, "future frame must remain rejected");
    assert_eq!(foreign_status, 204, "cross-session frame must remain rejected");
}
