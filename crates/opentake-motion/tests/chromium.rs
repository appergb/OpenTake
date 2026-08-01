#[cfg(feature = "chromium")]
mod live {
    use std::collections::BTreeSet;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::thread;
    use std::time::Duration;

    use opentake_motion::{
        HeadlessChromiumRenderer, MotionCache, MotionCancellationToken, MotionClipSource,
        MotionError, MotionRenderRequest, MotionRenderer, MotionSource, SandboxPolicy,
    };
    use opentake_render::{DecodedFrame, FrameProvider};

    fn browser() -> PathBuf {
        HeadlessChromiumRenderer::find_browser()
            .expect("the live chromium test requires Chrome, Chromium, or Edge")
    }

    fn request(document: &str) -> MotionRenderRequest {
        MotionRenderRequest::new(MotionSource::code(document), 10, 3, 48, 32)
    }

    fn renderer(root: &std::path::Path) -> HeadlessChromiumRenderer {
        HeadlessChromiumRenderer::new(
            MotionCache::new(root),
            SandboxPolicy::offline_with_timeout(Duration::from_secs(20)),
        )
        .with_browser_path(browser())
    }

    fn live_profiles() -> BTreeSet<PathBuf> {
        let prefix = format!("opentake-chromium-{}-", std::process::id());
        fs::read_dir(std::env::temp_dir())
            .unwrap()
            .flatten()
            .filter_map(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&prefix)
                    .then(|| entry.path())
            })
            .collect()
    }

    fn decoded(path: &std::path::Path) -> Option<DecodedFrame> {
        let rgba = image::open(path).ok()?.to_rgba8();
        Some(DecodedFrame::new(
            rgba.width(),
            rgba.height(),
            rgba.into_raw(),
            false,
        ))
    }

    pub(super) fn run() {
        let profiles_before = live_profiles();
        let animation = r#"<!doctype html><html><body style="margin:0;background:transparent">
          <div id="box" style="width:48px;height:32px"></div>
          <script>
            OpenTake.onSeek((t) => {
              const value = Math.round(t * 1000);
              box.style.background = `rgb(${value}, 20, 30)`;
              box.dataset.clock = `${Date.now()}:${performance.now()}`;
            });
          </script>
        </body></html>"#;

        let first_root = tempfile::tempdir().unwrap();
        let second_root = tempfile::tempdir().unwrap();
        let first = renderer(first_root.path())
            .render(&request(animation))
            .unwrap();
        let second = renderer(second_root.path())
            .render(&request(animation))
            .unwrap();

        assert_eq!(first.frame_count(), 3);
        assert_eq!(second.frame_count(), 3);
        for (a, b) in first.frames.iter().zip(&second.frames) {
            assert_eq!(fs::read(a).unwrap(), fs::read(b).unwrap());
        }
        assert_ne!(
            fs::read(&first.frames[0]).unwrap(),
            fs::read(&first.frames[2]).unwrap(),
            "virtual time must advance the visible animation"
        );
        let source = MotionClipSource::new(first.clone(), decoded);
        let composited = source
            .decoded_frame("motion", 2)
            .expect("Chromium PNG enters MotionClipSource");
        assert_eq!((composited.width, composited.height), (48, 32));
        assert_eq!(composited.rgba.len(), 48 * 32 * 4);

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let served = Arc::new(AtomicBool::new(false));
        let server_observed = Arc::clone(&served);
        let server = thread::spawn(move || {
            for _ in 0..250 {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut request = [0u8; 2048];
                        let _ = stream.read(&mut request);
                        let svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='2' height='2'><rect width='2' height='2' fill='red'/></svg>";
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            svg.len()
                        );
                        // Chromium may close the socket as soon as the image is
                        // decoded and the frame is captured. A late BrokenPipe
                        // therefore confirms neither a server nor render
                        // failure; accepting the request is the network-policy
                        // boundary this fixture needs to prove.
                        server_observed.store(true, Ordering::Release);
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.write_all(svg);
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(error) => panic!("loopback server failed: {error}"),
                }
            }
        });
        let allowed_root = tempfile::tempdir().unwrap();
        let allowed = HeadlessChromiumRenderer::new(
            MotionCache::new(allowed_root.path()),
            SandboxPolicy::offline_with_timeout(Duration::from_secs(20)).allow_origin(&origin),
        )
        .with_browser_path(browser())
        .render(&request(&format!("<img src=\"{origin}/pixel.svg\">")))
        .unwrap();
        assert_eq!(allowed.frame_count(), 3);
        server.join().unwrap();
        assert!(served.load(Ordering::Acquire));

        let blocked_root = tempfile::tempdir().unwrap();
        let blocked = renderer(blocked_root.path())
            .render(&request(
                r#"<img src="https://example.com/disallowed.png">"#,
            ))
            .unwrap_err();
        assert!(matches!(blocked, MotionError::Sandbox(_)), "{blocked:?}");
        assert!(
            fs::read_dir(blocked_root.path())
                .unwrap()
                .all(|entry| fs::read_dir(entry.unwrap().path())
                    .unwrap()
                    .next()
                    .is_none()),
            "a rejected render must not leave partial frames"
        );

        let filesystem_root = tempfile::tempdir().unwrap();
        let filesystem = renderer(filesystem_root.path())
            .render(&request(r#"<img src="file:///etc/passwd">"#))
            .unwrap_err();
        assert!(
            matches!(filesystem, MotionError::Sandbox(_)),
            "{filesystem:?}"
        );

        let timeout_root = tempfile::tempdir().unwrap();
        let timeout_renderer = HeadlessChromiumRenderer::new(
            MotionCache::new(timeout_root.path()),
            SandboxPolicy::offline_with_timeout(Duration::from_millis(500)),
        )
        .with_browser_path(browser());
        assert!(matches!(
            timeout_renderer.render(&request("<script>while(true){}</script>")),
            Err(MotionError::Timeout(_))
        ));

        let crash_root = tempfile::tempdir().unwrap();
        let crash_renderer = HeadlessChromiumRenderer::new(
            MotionCache::new(crash_root.path()),
            SandboxPolicy::default(),
        )
        .with_browser_path(if cfg!(windows) {
            PathBuf::from(r"C:\Windows\System32\where.exe")
        } else {
            PathBuf::from("/usr/bin/false")
        });
        let crashed = crash_renderer
            .render(&request("<div>crash</div>"))
            .unwrap_err();
        assert!(
            matches!(crashed, MotionError::RenderFailed(_)),
            "{crashed:?}"
        );

        let malformed_root = tempfile::tempdir().unwrap();
        assert!(matches!(
            renderer(malformed_root.path()).render(&request("   ")),
            Err(MotionError::InvalidSource(_))
        ));

        let cancellation = MotionCancellationToken::new();
        let cancelled_root = tempfile::tempdir().unwrap();
        let cancellation_for_render = cancellation.clone();
        let cancelled_cache = cancelled_root.path().to_path_buf();
        let cancelled_browser = browser();
        let render_thread = thread::spawn(move || {
            HeadlessChromiumRenderer::new(
                MotionCache::new(cancelled_cache),
                SandboxPolicy::offline_with_timeout(Duration::from_secs(20)),
            )
            .with_browser_path(cancelled_browser)
            .with_cancellation_token(cancellation_for_render)
            .render(&request("<script>while(true){}</script>"))
        });
        thread::sleep(Duration::from_millis(200));
        cancellation.cancel();
        assert!(matches!(
            render_thread.join().unwrap(),
            Err(MotionError::Cancelled)
        ));

        assert_eq!(
            live_profiles(),
            profiles_before,
            "success, policy failure, timeout, crash, and cancellation must clean browser profiles"
        );
    }
}

#[cfg(feature = "chromium")]
#[test]
fn virtual_time_network_csp_timeout_cleanup_and_frame_identity() {
    live::run();
}

#[cfg(not(feature = "chromium"))]
#[test]
fn virtual_time_network_csp_timeout_cleanup_and_frame_identity() {
    use opentake_motion::{
        HeadlessChromiumRenderer, MotionCache, MotionError, MotionRenderRequest, MotionRenderer,
        MotionSource, SandboxPolicy,
    };

    let root = tempfile::tempdir().unwrap();
    let renderer =
        HeadlessChromiumRenderer::new(MotionCache::new(root.path()), SandboxPolicy::default());
    let request = MotionRenderRequest::new(MotionSource::code("<div/>"), 30, 1, 16, 16);
    assert!(matches!(
        renderer.render(&request),
        Err(MotionError::RendererUnavailable(_))
    ));
}
