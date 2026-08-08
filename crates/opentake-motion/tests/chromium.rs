#[cfg(feature = "chromium")]
static LIVE_CHROMIUM_TEST_GATE: std::sync::OnceLock<std::sync::Mutex<()>> =
    std::sync::OnceLock::new();

#[cfg(feature = "chromium")]
fn test_gate_guard(
    gate: &'static std::sync::OnceLock<std::sync::Mutex<()>>,
) -> std::sync::MutexGuard<'static, ()> {
    gate.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("live Chromium test gate was poisoned by an earlier test failure")
}

#[cfg(feature = "chromium")]
fn live_test_guard() -> std::sync::MutexGuard<'static, ()> {
    test_gate_guard(&LIVE_CHROMIUM_TEST_GATE)
}

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
    use std::time::{Duration, Instant};

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
            // Generous bound: Chrome boot + virtual-time seeks + capture must
            // finish within it even on a loaded CI runner. The timeout
            // semantics themselves are asserted by the 500ms test below.
            SandboxPolicy::offline_with_timeout(Duration::from_secs(60)),
        )
        .with_browser_path(browser())
    }

    fn four_k_renderer(root: &std::path::Path) -> HeadlessChromiumRenderer {
        HeadlessChromiumRenderer::new(
            MotionCache::new(root),
            SandboxPolicy::offline_with_timeout(Duration::from_secs(180)),
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

    pub(super) fn assert_gate_serializes_concurrent_callers() {
        static PROBE_GATE: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        let first = super::test_gate_guard(&PROBE_GATE);
        let (attempted_tx, attempted_rx) = std::sync::mpsc::channel();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let second = thread::spawn(move || {
            attempted_tx.send(()).unwrap();
            let _second = super::test_gate_guard(&PROBE_GATE);
            entered_tx.send(()).unwrap();
        });

        attempted_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second gate caller did not start");
        assert!(
            matches!(
                entered_rx.recv_timeout(Duration::from_millis(50)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ),
            "a second live Chromium test must not enter while the first guard is held"
        );
        drop(first);
        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second gate caller did not enter after the first guard was released");
        second.join().unwrap();
    }

    pub(super) fn wrapper_probe() {
        let root = tempfile::tempdir().unwrap();
        let document = r#"<!doctype html>
          <html style="margin:0;width:100%;height:100%;background:rgb(12,34,56)">
          <body style="margin:0;width:100%;height:100%;background:rgb(12,34,56)">
            <div id="edge" style="position:fixed;right:0;bottom:0;width:1px;height:1px;background:rgb(1,2,3)"></div>
            <script>
              if (!window.OpenTake) throw new Error('child OpenTake clock missing');
              OpenTake.onSeek(() => {
                let parentDomAccessible = true;
                try { void window.parent.document.documentElement; }
                catch (error) { parentDomAccessible = !(error instanceof DOMException); }
                const isolated = window.top !== window
                  && !parentDomAccessible
                  && location.origin === 'null';
                const exactViewport = window.innerWidth === 48
                  && window.innerHeight === 32
                  && visualViewport.width === 48
                  && visualViewport.height === 32;
                if (!isolated || !exactViewport) {
                  fetch('https://example.com/wrapper-contract-violation');
                  return;
                }
                edge.style.background = 'rgb(7,8,9)';
              });
            </script>
          </body></html>"#;
        let rendered = renderer(root.path())
            .render(&MotionRenderRequest::new(
                MotionSource::code(document),
                10,
                1,
                48,
                32,
            ))
            .unwrap();
        let pixels = image::open(&rendered.frames[0]).unwrap().to_rgba8();
        assert_eq!(pixels.dimensions(), (48, 32));
        assert_eq!(pixels.get_pixel(0, 0).0, [12, 34, 56, 255]);
        assert_eq!(
            pixels.get_pixel(47, 31).0,
            [7, 8, 9, 255],
            "child seek must run in the unique default child context and preserve legal right/bottom-edge content"
        );
    }

    pub(super) fn four_k_budget_smoke() {
        const WIDTH: u32 = 3840;
        const HEIGHT: u32 = 2160;

        let opaque_root = tempfile::tempdir().unwrap();
        let opaque_started = Instant::now();
        let opaque = four_k_renderer(opaque_root.path())
            .render(
                &MotionRenderRequest::new(
                    MotionSource::code(
                        r#"<!doctype html><style>html,body{margin:0;width:100%;height:100%;background:rgb(12,34,56)}</style>"#,
                    ),
                    30,
                    1,
                    WIDTH,
                    HEIGHT,
                )
                .with_transparent(false),
            )
            .unwrap();
        let opaque_elapsed = opaque_started.elapsed();
        let opaque_pixels = image::open(&opaque.frames[0]).unwrap().to_rgba8();
        assert_eq!(opaque_pixels.dimensions(), (WIDTH, HEIGHT));
        assert!(
            opaque_pixels.pixels().all(|pixel| pixel.0[3] == 255),
            "the 4K opaque smoke frame must remain fully opaque"
        );
        eprintln!(
            "opentake-motion 4K opaque single-frame elapsed_ms={}",
            opaque_elapsed.as_millis()
        );

        let transparent_root = tempfile::tempdir().unwrap();
        let transparent_started = Instant::now();
        let transparent = four_k_renderer(transparent_root.path())
            .render(&MotionRenderRequest::new(
                MotionSource::code(
                    r#"<!doctype html><style>html,body{margin:0;width:100%;height:100%;background:transparent}#fill{position:fixed;inset:-16px;background:rgba(10,20,30,.5)}</style><div id="fill"></div>"#,
                ),
                30,
                1,
                WIDTH,
                HEIGHT,
            ))
            .unwrap();
        let transparent_elapsed = transparent_started.elapsed();
        let transparent_pixels = image::open(&transparent.frames[0]).unwrap().to_rgba8();
        assert_eq!(transparent_pixels.dimensions(), (WIDTH, HEIGHT));
        assert!(
            transparent_pixels
                .pixels()
                .all(|pixel| pixel.0[3] > 0 && pixel.0[3] < 255),
            "the 4K transparent smoke frame must retain non-trivial alpha"
        );
        eprintln!(
            "opentake-motion 4K transparent single-frame elapsed_ms={}",
            transparent_elapsed.as_millis()
        );
    }

    pub(super) fn run() {
        let profiles_before = live_profiles();
        let page_background_root = tempfile::tempdir().unwrap();
        let page_background_document = r#"<!doctype html>
          <html style="margin:0;width:100%;height:100%;background:rgb(12,34,56)">
          <body style="margin:0;width:100%;height:100%;background:rgb(12,34,56)">
          </body></html>"#;
        let page_background = renderer(page_background_root.path())
            .render(&MotionRenderRequest::new(
                MotionSource::code(page_background_document),
                10,
                1,
                48,
                32,
            ))
            .unwrap();
        let page_background_pixels = image::open(&page_background.frames[0]).unwrap().to_rgba8();
        assert!(
            page_background_pixels
                .pixels()
                .all(|pixel| pixel.0 == [12, 34, 56, 255]),
            "opaque author html/body backgrounds must render exactly without interfering with capture-session isolation"
        );

        let animation = r#"<!doctype html><html><body style="margin:0;background:transparent">
          <div id="box" style="width:24px;height:16px"></div>
          <script>
            OpenTake.onSeek((t) => {
              const value = Math.round(t * 1000);
              box.style.background = `rgb(${value}, 20, 30)`;
              box.dataset.clock = `${Date.now()}:${performance.now()}`;
            });
          </script>
        </body></html>"#;

        // The normal post-seek fence advances author time once. Background
        // readback for alpha recovery must not advance it again between the
        // black and white samples, or these primary colors become inconsistent.
        let timer_root = tempfile::tempdir().unwrap();
        let timer_document = r#"<!doctype html><html><body style="margin:0;background:transparent">
          <div id="box" style="position:fixed;inset:-16px;background:rgba(0,0,255,.5)"></div>
          <script>
            let timer;
            OpenTake.onSeek(() => {
              if (window.innerWidth !== 48 || window.innerHeight !== 32) {
                fetch('https://example.com/capture-session-changed-layout');
              }
              clearInterval(timer);
              let ticks = 0;
              timer = setInterval(() => {
                ticks += 1;
                box.style.background = ticks === 1
                  ? 'rgba(255,0,0,.5)'
                  : 'rgba(0,255,0,.5)';
                if (ticks === 2) {
                  fetch('https://example.com/timer-between-backgrounds');
                }
              }, 1);
            });
          </script>
        </body></html>"#;
        let timer_request =
            MotionRenderRequest::new(MotionSource::code(timer_document), 10, 2, 48, 32);
        let timer_frame = renderer(timer_root.path()).render(&timer_request).unwrap();
        let timer_pixels = image::open(&timer_frame.frames[0]).unwrap().to_rgba8();
        let unique_timer_pixels = timer_pixels
            .pixels()
            .map(|pixel| pixel.0)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            unique_timer_pixels.len(),
            1,
            "the full-canvas timer fixture must remain spatially uniform: {unique_timer_pixels:?}"
        );
        let timer_pixel = timer_pixels.get_pixel(0, 0).0;
        assert_eq!(
            timer_pixels.get_pixel(47, 31).0,
            timer_pixel,
            "an exact-size capture must preserve valid content touching the right/bottom edge"
        );
        assert!(
            timer_pixel[0] > timer_pixel[1]
                && timer_pixel[0] > timer_pixel[2]
                && timer_pixel[3] > 0
                && timer_pixel[3] < 255,
            "tick 1 must produce one uniform, red-dominant translucent state; actual={timer_pixel:?}"
        );

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
            let a_bytes = fs::read(a).unwrap();
            let b_bytes = fs::read(b).unwrap();
            if a_bytes != b_bytes {
                let a_pixels = image::load_from_memory(&a_bytes).unwrap().to_rgba8();
                let b_pixels = image::load_from_memory(&b_bytes).unwrap().to_rgba8();
                assert_eq!(a_pixels.dimensions(), b_pixels.dimensions());
                let differences = a_pixels
                    .as_raw()
                    .iter()
                    .zip(b_pixels.as_raw())
                    .filter(|(left, right)| left != right)
                    .count();
                let max_delta = a_pixels
                    .as_raw()
                    .iter()
                    .zip(b_pixels.as_raw())
                    .map(|(left, right)| left.abs_diff(*right))
                    .max()
                    .unwrap_or(0);
                let (width, height) = a_pixels.dimensions();
                let mut differing_pixels = 0usize;
                let mut canvas_edge_pixels = 0usize;
                let mut bbox = None::<(u32, u32, u32, u32)>;
                let mut samples = Vec::new();
                for (x, y, left) in a_pixels.enumerate_pixels() {
                    let right = b_pixels.get_pixel(x, y);
                    if left != right {
                        differing_pixels += 1;
                        if x == 0 || y == 0 || x + 1 == width || y + 1 == height {
                            canvas_edge_pixels += 1;
                        }
                        bbox = Some(match bbox {
                            Some((min_x, min_y, max_x, max_y)) => {
                                (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                            }
                            None => (x, y, x, y),
                        });
                        if samples.len() < 20 {
                            samples.push((x, y, left.0, right.0));
                        }
                    }
                }
                let unique_a = a_pixels
                    .pixels()
                    .map(|pixel| pixel.0)
                    .collect::<BTreeSet<_>>();
                let unique_b = b_pixels
                    .pixels()
                    .map(|pixel| pixel.0)
                    .collect::<BTreeSet<_>>();
                panic!(
                    "deterministic captures differ: {differences} channels, max delta {max_delta}, differing_pixels={differing_pixels}, bbox={bbox:?}, canvas_edge_pixels={canvas_edge_pixels}, interior_pixels={}, samples={samples:?}, png_bytes=({}, {}), unique_a={unique_a:?}, unique_b={unique_b:?}",
                    differing_pixels - canvas_edge_pixels,
                    a_bytes.len(),
                    b_bytes.len()
                );
            }
        }
        assert_ne!(
            fs::read(&first.frames[0]).unwrap(),
            fs::read(&first.frames[2]).unwrap(),
            "virtual time must advance the visible animation"
        );
        let first_png = image::open(&first.frames[0]).unwrap().to_rgba8();
        assert_eq!(first_png.get_pixel(0, 0)[3], 255);
        assert_eq!(
            first_png.get_pixel(47, 31)[3],
            0,
            "surface capture must preserve the transparent canvas outside content"
        );
        let source = MotionClipSource::new(first.clone(), decoded);
        let composited = source
            .decoded_frame("motion", 2)
            .expect("Chromium PNG enters MotionClipSource");
        assert_eq!((composited.width, composited.height), (48, 32));
        assert_eq!(composited.rgba.len(), 48 * 32 * 4);

        // Opaque clips use isolated stable PageHandlers without alpha recovery.
        // Render twice to prove determinism across independent browser processes.
        let opaque_root = tempfile::tempdir().unwrap();
        let opaque = renderer(opaque_root.path())
            .render(&request(animation).with_transparent(false))
            .unwrap();
        let opaque_again_root = tempfile::tempdir().unwrap();
        let opaque_again = renderer(opaque_again_root.path())
            .render(&request(animation).with_transparent(false))
            .unwrap();
        assert_eq!(opaque.frame_count(), 3);
        assert_eq!(opaque_again.frame_count(), 3);
        for (first, second) in opaque.frames.iter().zip(&opaque_again.frames) {
            assert_eq!(
                fs::read(first).unwrap(),
                fs::read(second).unwrap(),
                "opaque compositor captures must be byte-identical across browsers"
            );
        }
        let opaque_first = image::open(&opaque.frames[0]).unwrap().to_rgba8();
        assert_eq!(opaque_first.dimensions(), (48, 32));
        assert!(opaque_first.pixels().all(|pixel| pixel[3] == 255));
        assert_ne!(
            fs::read(&opaque.frames[0]).unwrap(),
            fs::read(&opaque.frames[2]).unwrap(),
            "opaque view capture must retain deterministic frame animation"
        );

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let served = Arc::new(AtomicBool::new(false));
        let server_observed = Arc::clone(&served);
        let stop_server = Arc::new(AtomicBool::new(false));
        let server_stopped = Arc::clone(&stop_server);
        let server = thread::spawn(move || {
            while !server_stopped.load(Ordering::Acquire) {
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
            SandboxPolicy::offline_with_timeout(Duration::from_secs(60)).allow_origin(&origin),
        )
        .with_browser_path(browser())
        .render(&request(&format!("<img src=\"{origin}/pixel.svg\">")));
        stop_server.store(true, Ordering::Release);
        server.join().unwrap();
        assert_eq!(allowed.unwrap().frame_count(), 3);
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

        let late_blocked_root = tempfile::tempdir().unwrap();
        let late_blocked = renderer(late_blocked_root.path())
            .render(&request(
                r#"<script>
                  OpenTake.onSeek((t) => {
                    if (t >= 0.2 && !window.lateFetchScheduled) {
                      window.lateFetchScheduled = true;
                      setTimeout(() => fetch('https://example.com/late-forbidden'), 0);
                    }
                  });
                </script>"#,
            ))
            .unwrap_err();
        assert!(
            matches!(late_blocked, MotionError::Sandbox(_)),
            "{late_blocked:?}"
        );
        assert!(
            fs::read_dir(late_blocked_root.path())
                .unwrap()
                .all(|entry| fs::read_dir(entry.unwrap().path())
                    .unwrap()
                    .next()
                    .is_none()),
            "a timer-triggered policy failure must not leave partial frames"
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
                SandboxPolicy::offline_with_timeout(Duration::from_secs(60)),
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
fn host_wrapper_context_csp_and_guard_probe() {
    live::assert_gate_serializes_concurrent_callers();
    let _live_test_guard = live_test_guard();
    live::wrapper_probe();
}

#[cfg(feature = "chromium")]
#[test]
fn four_k_single_frame_opaque_and_transparent_budget_smoke() {
    let _live_test_guard = live_test_guard();
    live::four_k_budget_smoke();
}

#[cfg(feature = "chromium")]
#[test]
fn virtual_time_network_csp_timeout_cleanup_and_frame_identity() {
    let _live_test_guard = live_test_guard();
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
