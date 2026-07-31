use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use opentake_media::{
    create_proxy, probe, MediaCancelToken, MediaError, ProxyProgressCallback, ProxyRequest,
};

fn make_video(path: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000",
            "-t",
            "1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(path)
        .status()
        .expect("spawn ffmpeg fixture");
    assert!(status.success());
}

#[test]
fn proxy_creation_is_cancellable_atomic_persistent_and_source_preserving() {
    if !opentake_media::ffmpeg_status::ffmpeg_available()
        || !opentake_media::ffmpeg_status::ffprobe_available()
    {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.mp4");
    let output = temp.path().join("media/proxies/source-proxy.mp4");
    make_video(&source);
    let source_before = fs::read(&source).unwrap();
    let progress_values = Arc::new(Mutex::new(Vec::new()));
    let capture = progress_values.clone();
    let progress: ProxyProgressCallback = Arc::new(move |done, total| {
        capture.lock().unwrap().push((done, total));
    });

    let result = create_proxy(
        ProxyRequest {
            source: &source,
            output: &output,
            max_size: (320, 180),
        },
        &MediaCancelToken::new(),
        Some(progress),
    )
    .expect("create bounded proxy");

    assert_eq!(result.path, output);
    assert_eq!(result.width, 320);
    assert_eq!(result.height, 180);
    assert_eq!(result.source_sha256.len(), 64);
    assert_eq!(fs::read(&source).unwrap(), source_before);
    let proxy_probe = probe(&output).unwrap();
    assert_eq!(
        (proxy_probe.width, proxy_probe.height),
        (Some(320), Some(180))
    );
    assert!(proxy_probe.has_video && proxy_probe.has_audio);
    let values = progress_values.lock().unwrap();
    assert_eq!(values.first().copied(), Some((0, 1000)));
    assert_eq!(values.last().copied(), Some((1000, 1000)));
    assert!(!temp
        .path()
        .join("media/proxies/source-proxy.mp4.partial")
        .exists());

    let cancelled = temp.path().join("media/proxies/cancelled.mp4");
    let cancel = MediaCancelToken::new();
    cancel.cancel();
    assert!(matches!(
        create_proxy(
            ProxyRequest {
                source: &source,
                output: &cancelled,
                max_size: (320, 180),
            },
            &cancel,
            None,
        ),
        Err(MediaError::Cancelled)
    ));
    assert!(!cancelled.exists());
    assert!(!cancelled.with_extension("mp4.partial").exists());
}
