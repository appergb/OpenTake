use std::path::PathBuf;

use opentake_domain::{
    Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, TextStyle, Timeline, Track,
};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let source = args
        .next()
        .map(PathBuf::from)
        .expect("usage: interchange-harness <source-video> <output-dir>");
    let out_dir = args.next().map(PathBuf::from).expect("missing output directory");
    assert!(source.is_file(), "source fixture missing: {}", source.display());
    std::fs::create_dir_all(&out_dir).expect("create output directory");

    let mut timeline = Timeline::new();
    timeline.width = 1280;
    timeline.height = 720;
    timeline.fps = 30;

    let mut base_track = Track::new("video-primary", ClipType::Video);
    let mut base = Clip::new("clip-primary", "fixture-video", 0, 60);
    base.media_type = ClipType::Video;
    base.source_clip_type = ClipType::Video;
    base.trim_start_frame = 10;
    base_track.clips.push(base);

    let mut overlay_track = Track::new("video-overlay", ClipType::Video);
    let mut overlay = Clip::new("clip-overlay", "fixture-video", 15, 30);
    overlay.media_type = ClipType::Video;
    overlay.source_clip_type = ClipType::Video;
    overlay.opacity = 0.75;
    overlay.transform.width = 0.5;
    overlay.transform.height = 0.5;
    overlay_track.clips.push(overlay);

    let mut caption_track = Track::new("captions", ClipType::Text);
    let mut caption = Clip::new("caption-1", "", 15, 30);
    caption.media_type = ClipType::Text;
    caption.source_clip_type = ClipType::Text;
    caption.caption_group_id = Some("caption-group-1".to_string());
    caption.text_content = Some("你好，OpenTake\nSecond line".to_string());
    caption.text_style = Some(TextStyle::default());
    caption_track.clips.push(caption);

    timeline.tracks.extend([base_track, overlay_track, caption_track]);

    let mut manifest = MediaManifest::new();
    manifest.entries.push(MediaManifestEntry {
        id: "fixture-video".to_string(),
        name: "audit-4k60-h264.mp4".to_string(),
        kind: ClipType::Video,
        source: MediaSource::External {
            absolute_path: source.to_string_lossy().into_owned(),
        },
        duration: 4.0,
        generation_input: None,
        source_width: Some(3840),
        source_height: Some(2160),
        source_fps: Some(60.0),
        has_audio: Some(false),
        color: None,
        proxy: None,
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    });

    let outputs = [
        (
            "timeline-xmeml.xml",
            opentake_project::export_xmeml(&timeline, &manifest, None),
        ),
        (
            "timeline-modern.fcpxml",
            opentake_project::export_fcpxml(&timeline, &manifest, None),
        ),
        (
            "timeline.edl",
            opentake_project::export_edl(&timeline, &manifest),
        ),
        (
            "timeline.otio",
            opentake_project::export_otio(&timeline, &manifest, None),
        ),
        ("captions.srt", opentake_domain::export_srt(&timeline)),
        ("captions.vtt", opentake_domain::export_vtt(&timeline)),
    ];
    for (name, body) in outputs {
        let path = out_dir.join(name);
        std::fs::write(&path, body.as_bytes()).expect("write interchange artifact");
        println!("artifact={} bytes={}", path.display(), body.len());
    }
    println!(
        "cue_count={}",
        opentake_domain::collect_caption_cues(&timeline).len()
    );
}
