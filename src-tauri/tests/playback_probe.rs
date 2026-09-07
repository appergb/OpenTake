//! P0 real-device acceptance probe for the playback engine (the engine half of
//! the `PLAYBACK-ENGINE.md` acceptance checklist).
//!
//! Unlike the unit tests, this drives `PlaybackEngine` end-to-end on real GPU +
//! real media + a real cpal audio device on this machine. Verifies the #192 fix
//! (`run_render_thread` sleeping only the remainder of the frame budget instead
//! of unconditionally sleeping a full `frame_dur` on top of render time, which
//! capped playback at ~21-23fps regardless of target fps).
//!
//! Default `#[ignore]`; run manually on a real machine with a GPU + audio
//! device:
//! ```sh
//! OPENTAKE_PROBE_VIDEO=/path/to/real_video_with_audio.mp4 \
//!   cargo test -p opentake-tauri --test playback_probe -- --ignored --nocapture
//! ```
//! `probe_realtime_playback_with_audio_or_safe_fallback` needs
//! `OPENTAKE_PROBE_VIDEO` (a
//! real asset with an audio track). A live cpal callback drives playback; when
//! the host accepts the stream but never invokes its callback, the probe also
//! verifies the production wall-clock fallback rather than allowing a frozen
//! audio clock. Set `OPENTAKE_REQUIRE_AUDIO_CALLBACK=1` on an audio-qualified
//! host to make callback fallback a hard failure. It soft-skips (prints and
//! returns) when the media env var is unset.
//! The other two probes generate their own fixtures at runtime via ffmpeg into
//! the OS temp dir and soft-skip when ffmpeg is unavailable.
#![cfg(feature = "playback-engine")]

use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use opentake_domain::{
    Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, Timeline, Track,
};
use opentake_render::{DecodedFrame, RenderSize};
use opentake_tauri_lib::playback::project::source_preview_timeline;
use opentake_tauri_lib::playback::{
    audio::build_clock_paused, project_media, FrameSink, MediaInfo, PlaybackEngine, PlayheadEmitter,
};

#[derive(Clone, Debug)]
struct FrameQuality {
    min_nonblack_ratio: f64,
    max_neon_green_ratio: f64,
    first_green_frame: Option<i32>,
}

impl Default for FrameQuality {
    fn default() -> Self {
        Self {
            min_nonblack_ratio: 1.0,
            max_neon_green_ratio: 0.0,
            first_green_frame: None,
        }
    }
}

/// Collects frames and quality facts across the complete playback run.
struct ProbeSink {
    frames: AtomicI32,
    last: Mutex<Option<DecodedFrame>>,
    quality: Mutex<FrameQuality>,
}

impl FrameSink for ProbeSink {
    fn push_frame(&self, frame: &DecodedFrame) {
        let frame_index = self.frames.fetch_add(1, Ordering::SeqCst);
        let nonblack = nonblack_ratio(frame);
        let neon_green = neon_green_ratio(frame);
        let mut quality = self.quality.lock().unwrap();
        quality.min_nonblack_ratio = quality.min_nonblack_ratio.min(nonblack);
        quality.max_neon_green_ratio = quality.max_neon_green_ratio.max(neon_green);
        if neon_green >= 0.01 && quality.first_green_frame.is_none() {
            quality.first_green_frame = Some(frame_index);
        }
        *self.last.lock().unwrap() = Some(frame.clone());
    }
}

struct ProbeEmitter {
    last_frame: AtomicI32,
}

impl PlayheadEmitter for ProbeEmitter {
    fn emit(&self, frame: i32) {
        self.last_frame.store(frame, Ordering::SeqCst);
    }
}

fn video_clip(id: &str, media: &str, start: i32, dur: i32) -> Clip {
    let mut c = Clip::new(id, media, start, dur);
    c.media_type = ClipType::Video;
    c.source_clip_type = ClipType::Video;
    c
}

/// Run a timeline for `secs` seconds, returning frames received, last playhead,
/// last frame, whether a live audio clock was installed, and whole-run quality.
fn run_engine(
    timeline: Timeline,
    media: HashMap<String, MediaInfo>,
    sizes: HashMap<String, (u32, u32)>,
    secs: f64,
) -> (i32, i32, Option<DecodedFrame>, bool, FrameQuality) {
    run_engine_from(timeline, media, sizes, 0, secs)
}

fn run_engine_from(
    timeline: Timeline,
    media: HashMap<String, MediaInfo>,
    sizes: HashMap<String, (u32, u32)>,
    start_frame: i32,
    secs: f64,
) -> (i32, i32, Option<DecodedFrame>, bool, FrameQuality) {
    let fps = timeline.fps;
    let (clock, audio) = build_clock_paused(&timeline, &media, fps, start_frame)
        .expect("probe prepared audio clock");
    let audio_active = audio.is_some();
    let sink = Arc::new(ProbeSink {
        frames: AtomicI32::new(0),
        last: Mutex::new(None),
        quality: Mutex::new(FrameQuality::default()),
    });
    let emitter = Arc::new(ProbeEmitter {
        last_frame: AtomicI32::new(-1),
    });
    let engine = PlaybackEngine::spawn_ready(
        timeline,
        media,
        HashMap::new(),
        sizes,
        RenderSize::new(640, 360),
        clock,
        sink.clone(),
        emitter.clone(),
        start_frame,
    )
    .expect("engine ready (GPU acquire + first frame)");
    if let Some(audio) = audio.as_ref() {
        audio
            .prepare_resume()
            .expect("prepared audio callback resumes muted");
    }
    engine.resume(start_frame).expect("prepared engine resumes");
    if let Some(audio) = audio.as_ref() {
        audio.commit_resume();
    }
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    engine.stop();
    let n = sink.frames.load(Ordering::SeqCst);
    let head = emitter.last_frame.load(Ordering::SeqCst);
    let last = sink.last.lock().unwrap().clone();
    let quality = sink.quality.lock().unwrap().clone();
    (n, head, last, audio_active, quality)
}

fn nonblack_ratio(f: &DecodedFrame) -> f64 {
    let total = (f.width * f.height) as f64;
    let mut nonblack = 0u64;
    for px in f.rgba.as_chunks::<4>().0.iter() {
        if px[0] > 16 || px[1] > 16 || px[2] > 16 {
            nonblack += 1;
        }
    }
    nonblack as f64 / total
}

fn neon_green_ratio(f: &DecodedFrame) -> f64 {
    let total = (f.width * f.height) as f64;
    let neon_green = f
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] < 32 && pixel[1] > 224 && pixel[2] < 32)
        .count();
    neon_green as f64 / total
}

fn ffmpeg_ready() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Acceptance 1+4: real media (with an audio track, cpal master clock) plays
/// continuously; frame rate meets target and the playhead advances.
#[test]
#[ignore = "real-device probe: needs GPU + audio device + a real video asset"]
fn probe_realtime_playback_with_audio_or_safe_fallback() {
    let Ok(src) = std::env::var("OPENTAKE_PROBE_VIDEO") else {
        eprintln!(
            "skip: set OPENTAKE_PROBE_VIDEO to a real video file (with audio) to run this probe"
        );
        return;
    };
    if !std::path::Path::new(&src).exists() {
        eprintln!("skip: OPENTAKE_PROBE_VIDEO does not exist: {src}");
        return;
    }
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 1920;
    tl.height = 1080;
    let mut track = Track::new("t-v1", ClipType::Video);
    track.clips.push(video_clip("c-1", "m-1", 0, 300));
    tl.tracks.push(track);

    let mut media = HashMap::new();
    media.insert(
        "m-1".to_string(),
        MediaInfo {
            path: src.into(),
            straight_alpha: false,
        },
    );
    let mut sizes = HashMap::new();
    sizes.insert("m-1".to_string(), (1584u32, 1080u32));

    let (frames, playhead, last, audio_active, _quality) = run_engine(tl, media, sizes, 3.0);
    let clock_mode = if audio_active {
        "audio"
    } else {
        "wall-fallback"
    };
    eprintln!("[probe] frames={frames} playhead={playhead} clock={clock_mode}");
    if std::env::var("OPENTAKE_REQUIRE_AUDIO_CALLBACK").as_deref() == Ok("1") {
        assert!(
            audio_active,
            "strict audio qualification requires a live device callback"
        );
    }
    let last = last.expect("no frame captured");
    let ratio = nonblack_ratio(&last);
    let green_ratio = neon_green_ratio(&last);
    eprintln!("[probe] nonblack_ratio={ratio:.3} neon_green_ratio={green_ratio:.3}");
    assert!(ratio > 0.01, "frames are black (ratio {ratio:.4})");
    assert!(
        green_ratio < 0.01,
        "frames contain a neon-green corruption region (ratio {green_ratio:.4})"
    );
    // 3s @30fps targets 90 frames. Pre-#192-fix, the render thread stacked a
    // full frame period on top of render time and measured ~67 frames/3s
    // (~22fps) on this asset+machine; post-fix it consistently measures
    // ~75-77 frames/3s (~25.5fps), bounded by a separate, pre-existing GPU
    // readback cost in the compositor (a synchronous `device.poll(Wait)`)
    // rather than by sleep pacing. >=73 asserts the fix (not just "some
    // number passes") while leaving margin below the observed floor.
    assert!(frames >= 73, "frame rate too low: {frames} frames in 3s");
    assert!(playhead >= 73, "playhead did not advance: {playhead}");
}

/// Real 4K HEVC Main10 playback probe. This validates every published frame's
/// pixels and transport progress without reusing the calibrated throughput
/// threshold from the smaller acceptance asset above.
#[test]
#[ignore = "real-device probe: needs GPU + a real HEVC Main10 video asset"]
fn probe_main10_playback_has_no_black_or_green_frames() {
    let Some(src) = std::env::var_os("OPENTAKE_MAIN10_FIXTURE") else {
        eprintln!("skip: set OPENTAKE_MAIN10_FIXTURE to a real HEVC Main10 video file");
        return;
    };
    let src = std::path::PathBuf::from(src);
    if !src.exists() {
        eprintln!(
            "skip: OPENTAKE_MAIN10_FIXTURE does not exist: {}",
            src.display()
        );
        return;
    }
    let probe = opentake_media::probe(&src).expect("probe Main10 fixture");
    let source_size = (
        probe.width.expect("Main10 fixture width"),
        probe.height.expect("Main10 fixture height"),
    );
    let source_fps = probe.fps.expect("Main10 fixture fps");
    let trim_start_frame = std::env::var("OPENTAKE_PROBE_START_FRAME")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_572);
    assert!(
        trim_start_frame + 300 < (probe.duration_secs * source_fps) as i32,
        "Main10 probe window exceeds source duration"
    );

    let mut manifest = MediaManifest::new();
    manifest.entries.push(MediaManifestEntry {
        id: "m-1".into(),
        name: "Main10.mov".into(),
        kind: ClipType::Video,
        source: MediaSource::External {
            absolute_path: src.to_string_lossy().into_owned(),
        },
        duration: probe.duration_secs,
        generation_input: None,
        source_width: Some(source_size.0 as i32),
        source_height: Some(source_size.1 as i32),
        source_fps: Some(source_fps),
        has_audio: Some(true),
        color: probe.color,
        proxy: None,
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    });
    let timeline = source_preview_timeline(&manifest, "m-1", 30)
        .expect("Main10 source projects through the production preview seam");
    let (sizes, media) = project_media(&manifest, &None);

    let (frames, playhead, _last, _audio_active, quality) =
        run_engine_from(timeline, media, sizes, trim_start_frame, 3.0);
    eprintln!(
        "[probe] Main10 frames={frames} playhead={playhead} min_nonblack={:.3} max_neon_green={:.3}",
        quality.min_nonblack_ratio, quality.max_neon_green_ratio
    );
    assert!(
        frames > 1,
        "Main10 playback did not publish consecutive frames"
    );
    assert!(
        playhead > trim_start_frame,
        "Main10 source-preview seek did not advance: {playhead}"
    );
    assert!(
        quality.min_nonblack_ratio > 0.01,
        "Main10 playback published a black frame (minimum nonblack ratio {:.4})",
        quality.min_nonblack_ratio
    );
    assert!(
        quality.max_neon_green_ratio < 0.01,
        "Main10 playback published a green-corrupted frame at {:?} (maximum ratio {:.4})",
        quality.first_green_frame,
        quality.max_neon_green_ratio
    );
}

/// Generate an N-second ProRes (422, profile 2) + PCM audio fixture at `path`
/// via ffmpeg. Returns false (→ skip) on failure.
fn make_prores_fixture(path: &std::path::Path, secs: u32) -> bool {
    Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc2=size=1280x720:rate=30:duration={secs}"),
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency=440:duration={secs}"),
            "-c:v",
            "prores_ks",
            "-profile:v",
            "2",
            "-c:a",
            "pcm_s16le",
        ])
        .arg(path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Acceptance 2: ProRes (a format the WebView `<video>` element can't play)
/// decodes and plays.
#[test]
#[ignore = "real-device probe: needs GPU + audio device"]
fn probe_prores_playback() {
    if !ffmpeg_ready() {
        eprintln!("skip: ffmpeg not available");
        return;
    }
    let src = std::env::temp_dir().join("opentake-probe-192-prores.mov");
    if !make_prores_fixture(&src, 3) {
        eprintln!("skip: could not generate ProRes fixture at {src:?}");
        return;
    }

    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 1280;
    tl.height = 720;
    let mut track = Track::new("t-v1", ClipType::Video);
    track.clips.push(video_clip("c-1", "m-1", 0, 90));
    tl.tracks.push(track);

    let mut media = HashMap::new();
    media.insert(
        "m-1".to_string(),
        MediaInfo {
            path: src,
            straight_alpha: false,
        },
    );
    let mut sizes = HashMap::new();
    sizes.insert("m-1".to_string(), (1280u32, 720u32));

    let (frames, _playhead, last, _audio_active, _quality) = run_engine(tl, media, sizes, 2.0);
    eprintln!("[probe] prores frames={frames}");
    // 2s @30fps targets 60 frames. Post-#192-fix this consistently measures
    // ~50-51 frames/2s on this machine (bounded by the same compositor GPU
    // readback floor as the audio-clock probe above, not by sleep pacing);
    // pre-fix baseline was >=40. >=48 asserts the fix while leaving margin
    // below the observed floor.
    assert!(frames >= 48, "prores playback too slow: {frames} in 2s");
    let last = last.expect("no prores frame captured");
    let ratio = nonblack_ratio(&last);
    eprintln!("[probe] prores nonblack_ratio={ratio:.3}");
    // testsrc2 is a color-bar pattern, so the non-black ratio should be high.
    assert!(ratio > 0.5, "prores frames black (ratio {ratio:.4})");
}

/// Generate a short H.264 color-bar fixture at `path` via ffmpeg. Returns
/// false (→ skip) on failure.
fn make_colorbar_fixture(path: &std::path::Path, secs: u32) -> bool {
    Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc2=size=640x360:rate=30:duration={secs}"),
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency=440:duration={secs}"),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Acceptance 3: GPU-only color grading is visible in playback frames
/// (saturation=0 → grayscale output).
#[test]
#[ignore = "real-device probe: needs GPU + audio device"]
fn probe_color_grade_visible_in_playback() {
    if !ffmpeg_ready() {
        eprintln!("skip: ffmpeg not available");
        return;
    }
    let src = std::env::temp_dir().join("opentake-probe-192-colorbar.mp4");
    if !make_colorbar_fixture(&src, 2) {
        eprintln!("skip: could not generate color-bar fixture at {src:?}");
        return;
    }

    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 640;
    tl.height = 360;
    let mut track = Track::new("t-v1", ClipType::Video);
    let mut clip = video_clip("c-1", "m-1", 0, 60);
    let grade = opentake_domain::ColorGrade {
        saturation: 0.0, // grayscale
        ..Default::default()
    };
    clip.color_grade = Some(grade);
    track.clips.push(clip);
    tl.tracks.push(track);

    let mut media = HashMap::new();
    media.insert(
        "m-1".to_string(),
        MediaInfo {
            path: src,
            straight_alpha: false,
        },
    );
    let mut sizes = HashMap::new();
    sizes.insert("m-1".to_string(), (640u32, 360u32));

    let (frames, _playhead, last, _audio_active, _quality) = run_engine(tl, media, sizes, 1.5);
    // 1.5s @30fps targets 45 frames. Post-#192-fix this measures ~32-39
    // frames/1.5s on this machine across repeated runs (short 1.5s window,
    // so warm-up cost is a larger share and variance is higher than the
    // longer probes above; same GPU readback floor, not sleep pacing);
    // pre-fix baseline was >=20. >=30 asserts the fix while leaving margin
    // below the observed floor.
    assert!(frames >= 30, "grade playback too slow: {frames}");
    let last = last.expect("no graded frame captured");
    // The color-bar source is highly saturated; grade saturation=0 should
    // leave every pixel with R≈G≈B.
    let mut max_dev = 0i32;
    for px in last.rgba.as_chunks::<4>().0.iter() {
        let (r, g, b) = (px[0] as i32, px[1] as i32, px[2] as i32);
        let dev = (r - g).abs().max((g - b).abs()).max((r - b).abs());
        max_dev = max_dev.max(dev);
    }
    eprintln!("[probe] grade frames={frames} max_channel_dev={max_dev}");
    assert!(
        max_dev <= 8,
        "saturation=0 grade not applied in playback (max channel dev {max_dev})"
    );
}
