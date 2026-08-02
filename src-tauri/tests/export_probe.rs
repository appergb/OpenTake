//! T7 真机导出验收探针:H.264 / H.265 / ProRes 三编码 + 文本 overlay。
//! `cargo test -p opentake-tauri --test export_probe -- --ignored --nocapture`
#![cfg(not(target_os = "windows"))]

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use opentake_domain::{
    Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, TextStyle, Timeline, Track,
};
use opentake_tauri_lib::export::{run_export, ExportCodec, ExportQuality, ExportRequest};

/// Generate the probe source once per run: 2s of ffmpeg's `testsrc2` pattern at
/// the probe timeline's size/fps. Self-contained so the probe runs on any
/// machine with ffmpeg — no checked-in or session-scoped fixture.
fn testbar_path() -> PathBuf {
    static GEN: OnceLock<PathBuf> = OnceLock::new();
    GEN.get_or_init(|| {
        let out = std::env::temp_dir().join("opentake-export-probe-src.mp4");
        let stale = std::fs::metadata(&out)
            .map(|m| m.len() == 0)
            .unwrap_or(true);
        if stale {
            let status = Command::new("ffmpeg")
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=640x360:rate=30:duration=2",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:v",
                    "libx264",
                ])
                .arg(&out)
                .status()
                .expect("ffmpeg run (probe fixture)");
            assert!(status.success(), "probe fixture generation failed");
        }
        out
    })
    .clone()
}

fn probe_timeline() -> (Timeline, MediaManifest) {
    let mut tl = Timeline::new();
    tl.fps = 30;
    tl.width = 640;
    tl.height = 360;
    let mut v1 = Track::new("t-v1", ClipType::Video);
    let mut c = Clip::new("c-1", "m-1", 0, 60);
    c.media_type = ClipType::Video;
    c.source_clip_type = ClipType::Video;
    v1.clips.push(c);
    tl.tracks.push(v1);

    // 顶层文本轨:验证导出路径的文字光栅化。
    let mut v2 = Track::new("t-v2", ClipType::Video);
    let mut t = Clip::new("c-t", "", 0, 60);
    t.media_type = ClipType::Text;
    t.source_clip_type = ClipType::Text;
    t.text_content = Some("EXPORT PROBE 导出验收".to_string());
    t.text_style = Some(TextStyle::default());
    v2.clips.push(t);
    tl.tracks.push(v2);

    let mut manifest = MediaManifest::default();
    manifest.entries.push(MediaManifestEntry {
        id: "m-1".to_string(),
        name: "testbar.mp4".to_string(),
        kind: ClipType::Video,
        source: MediaSource::External {
            absolute_path: testbar_path().to_string_lossy().to_string(),
        },
        duration: 2.0,
        generation_input: None,
        source_width: Some(640),
        source_height: Some(360),
        source_fps: Some(30.0),
        has_audio: Some(false),
        color: None,
        proxy: None,
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    });
    (tl, manifest)
}

fn ffprobe_codec(path: &PathBuf) -> String {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,nb_frames",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .expect("ffprobe run");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn run_one(codec: ExportCodec, ext: &str, expect_codec: &str) {
    assert!(testbar_path().exists(), "probe asset missing");
    let (tl, manifest) = probe_timeline();
    let out = std::env::temp_dir().join(format!("opentake-export-probe.{ext}"));
    let _ = std::fs::remove_file(&out);
    let req = ExportRequest {
        out_path: out.to_string_lossy().to_string(),
        codec,
        quality: ExportQuality::P720,
    };
    let summary = run_export(&tl, &manifest, &None, &req).expect("export failed");
    eprintln!("[probe] {expect_codec} summary: {summary:?}");
    assert!(out.exists(), "output missing: {out:?}");
    let meta = ffprobe_codec(&out);
    eprintln!("[probe] {expect_codec} ffprobe: {meta}");
    assert!(
        meta.starts_with(expect_codec),
        "wrong codec: got '{meta}', want '{expect_codec}'"
    );
    let size = std::fs::metadata(&out).unwrap().len();
    assert!(size > 10_000, "suspiciously small file: {size} bytes");
}

#[test]
#[ignore = "real-device probe: GPU + ffmpeg"]
fn probe_export_h264() {
    run_one(ExportCodec::H264, "mp4", "h264");
}

#[test]
#[ignore = "real-device probe: GPU + ffmpeg"]
fn probe_export_h265() {
    run_one(ExportCodec::H265, "hevc.mp4", "hevc");
}

#[test]
#[ignore = "real-device probe: GPU + ffmpeg"]
fn probe_export_prores() {
    run_one(ExportCodec::Prores, "mov", "prores");
}
