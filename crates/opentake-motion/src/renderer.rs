//! The rendering contract and its implementations.
//!
//! [`MotionRenderer`] is the single trait the rest of the system depends on:
//! given a validated [`MotionRenderRequest`], produce a [`RenderedClip`] (a
//! sequence of on-disk RGBA frames). Two implementations live here:
//!
//! - [`StubRenderer`] — a deterministic, dependency-free renderer that paints
//!   each frame a solid color derived from `(frame, content-hash)`. It exists so
//!   the whole pipeline (validation → cache → frame files → compositor ingest)
//!   is unit-testable offline with **no browser**.
//! - [`HeadlessChromiumRenderer`] — the live CDP backend (virtual time +
//!   per-frame screenshot with alpha), gated behind the `chromium` cargo
//!   feature. Without that feature it returns a clear
//!   [`MotionError::RendererUnavailable`].
//!
//! Both share [`deterministic_clock_script`] — the injected JS that freezes the
//! page clock and exposes `OpenTake.seek(seconds)`, the render contract authors
//! animate against.

#[cfg(feature = "chromium")]
use std::path::Path;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::cache::{content_hash, MotionCache};
use crate::error::{MotionError, MotionResult};
use crate::sandbox::SandboxPolicy;
use crate::source::{MotionRenderRequest, MotionSource, RenderedClip};

/// The render contract. Implementors turn a request into on-disk frames.
///
/// Implementations MUST be deterministic: the same request must yield
/// byte-identical frames every time (this is what makes preview == export and
/// what the content-hash cache relies on).
pub trait MotionRenderer {
    /// Render `req` to frames on disk, returning the clip handle. The request is
    /// assumed already validated by the caller (see
    /// [`MotionRenderRequest::validate`]); implementations re-apply the sandbox
    /// document-size / network checks they are responsible for.
    fn render(&self, req: &MotionRenderRequest) -> MotionResult<RenderedClip>;
}

/// Cooperative cancellation shared between the caller and a live browser
/// render. Cancelling is idempotent and may happen from any thread.
#[derive(Clone, Debug, Default)]
pub struct MotionCancellationToken(Arc<AtomicBool>);

impl MotionCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// The deterministic clock contract injected into every native fallback
/// rendered document. It:
/// 1. Pauses CSS/Web animations by pinning `document.timeline.currentTime`.
/// 2. Exposes `window.OpenTake.seek(seconds)` so the host advances time per
///    frame deterministically instead of relying on the wall clock.
///
/// Returned as a string so the CDP backend can `Page.addScriptToEvaluateOnNewDocument`
/// it before any author script runs. Pure + testable.
pub fn deterministic_clock_script() -> &'static str {
    // Kept intentionally small and dependency-free. Real backends evaluate this
    // as an "on new document" script so it wins the race against author code.
    r#"(function () {
  if (window.OpenTake && window.OpenTake.__installed) return;
  var current = 0;
  var listeners = [];
  var randomState = 0x6d2b79f5;
  try { Date.now = function () { return Math.round(current * 1000); }; } catch (e) {}
  try {
    Object.defineProperty(performance, 'now', {
      configurable: true,
      value: function () { return current * 1000; }
    });
  } catch (e) {}
  try {
    Math.random = function () {
      randomState = (randomState + 0x6d2b79f5) | 0;
      var n = Math.imul(randomState ^ (randomState >>> 15), 1 | randomState);
      n = (n + Math.imul(n ^ (n >>> 7), 61 | n)) ^ n;
      return ((n ^ (n >>> 14)) >>> 0) / 4294967296;
    };
  } catch (e) {}
  window.OpenTake = {
    __installed: true,
    // Current virtual time in seconds.
    currentTime: function () { return current; },
    // Host calls this once per frame with t = frameIndex / fps.
    seek: async function (seconds) {
      current = seconds;
      randomState = (0x6d2b79f5 ^ Math.round(seconds * 1000000)) | 0;
      try {
        if (document.timeline) {
          // Freeze the document timeline to the virtual clock (ms).
          Object.defineProperty(document.timeline, 'currentTime', {
            configurable: true,
            get: function () { return seconds * 1000; }
          });
        }
      } catch (e) { /* timeline may be read-only; listeners still fire */ }
      var pending = [];
      for (var i = 0; i < listeners.length; i++) {
        try { pending.push(Promise.resolve(listeners[i](seconds))); } catch (e) {}
      }
      await Promise.all(pending);
    },
    // Authors register frame callbacks: OpenTake.onSeek(t => { ... }).
    onSeek: function (fn) { if (typeof fn === 'function') listeners.push(fn); }
  };
})();"#
}

/// A deterministic, browser-free renderer for tests and offline pipelines.
///
/// Each frame is a solid RGBA fill whose color is a pure function of the frame
/// index and the request's content hash, so output is reproducible and distinct
/// per request. When the request is transparent, alpha ramps across the clip so
/// tests can assert the alpha channel survived.
#[derive(Clone, Debug)]
pub struct StubRenderer {
    cache: MotionCache,
}

impl StubRenderer {
    /// Build a stub renderer writing frames under `cache`.
    pub fn new(cache: MotionCache) -> Self {
        StubRenderer { cache }
    }

    /// The deterministic RGBA for a given frame of a given hash.
    fn frame_color(hash: &str, frame: u32, total: u32, transparent: bool) -> [u8; 4] {
        // Derive RGB from the first hash bytes + the frame index so consecutive
        // frames differ and different requests differ.
        let h = hash.as_bytes();
        let b = |i: usize| h.get(i).copied().unwrap_or(0);
        let r = b(0) ^ (frame as u8);
        let g = b(1).wrapping_add(frame as u8);
        let bl = b(2);
        let a = if transparent {
            // Linear ramp 0..=255 across the clip; single-frame clips are opaque.
            if total <= 1 {
                255
            } else {
                ((frame * 255) / (total - 1)) as u8
            }
        } else {
            255
        };
        [r, g, bl, a]
    }
}

impl MotionRenderer for StubRenderer {
    fn render(&self, req: &MotionRenderRequest) -> MotionResult<RenderedClip> {
        req.validate()?;
        // Even the stub honors the sandbox document-size ceiling so the security
        // contract is exercised by tests.
        let policy = SandboxPolicy::default();
        if let MotionSource::Code { html_css_js } = &req.source {
            policy.check_document_size(html_css_js)?;
        }

        let hash = content_hash(req);
        if self.cache.is_cached(req) {
            let dir = self.cache.dir_for(req);
            return Ok(RenderedClip {
                content_hash: hash,
                frames: (0..req.duration_frames as usize)
                    .map(|index| MotionCache::frame_file(&dir, index))
                    .collect(),
                fps: req.fps,
                width: req.width,
                height: req.height,
                transparent: req.transparent,
            });
        }
        let dir = self.cache.begin_render(req)?;

        let mut frames: Vec<PathBuf> = Vec::with_capacity(req.duration_frames as usize);
        for frame in 0..req.duration_frames {
            let path = MotionCache::frame_file(&dir, frame as usize);
            let color = Self::frame_color(&hash, frame, req.duration_frames, req.transparent);
            write_solid_rgba_png(&path, req.width, req.height, color)?;
            frames.push(path);
        }
        MotionCache::mark_complete(&dir)?;

        Ok(RenderedClip {
            content_hash: hash,
            frames,
            fps: req.fps,
            width: req.width,
            height: req.height,
            transparent: req.transparent,
        })
    }
}

/// Write a solid-color RGBA PNG. Minimal hand-rolled encoder is avoided in favor
/// of the `image` dev-dep only in tests; here in lib code we keep a tiny
/// dependency-free encoder so the stub is usable outside tests too.
fn write_solid_rgba_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgba: [u8; 4],
) -> MotionResult<()> {
    let buf = encode_solid_rgba_png(width, height, rgba);
    std::fs::write(path, buf)?;
    Ok(())
}

/// Encode a solid-color image as a (valid, if uncompressed-deflate) RGBA PNG.
/// Dependency-free: builds the PNG container with a single stored-block zlib
/// stream so it round-trips through any standard PNG decoder. Pure → testable.
pub(crate) fn encode_solid_rgba_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    // Raw image data: each row is a filter byte (0 = none) followed by RGBA
    // pixels. Build one canonical row, then repeat it for every scanline.
    let mut row = Vec::with_capacity(1 + (width as usize) * 4);
    row.push(0u8); // filter: None
    for _ in 0..width {
        row.extend_from_slice(&rgba);
    }
    let mut raw = Vec::with_capacity(row.len() * height as usize);
    for _ in 0..height {
        raw.extend_from_slice(&row);
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]); // signature

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_chunk(&mut png, b"IHDR", &ihdr);

    // IDAT: zlib stream wrapping stored (uncompressed) deflate blocks.
    let idat = zlib_store(&raw);
    write_chunk(&mut png, b"IDAT", &idat);

    // IEND
    write_chunk(&mut png, b"IEND", &[]);
    png
}

/// Append a PNG chunk (length, type, data, CRC32).
fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = Crc32::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(&crc.finalize().to_be_bytes());
}

/// Wrap `data` in a zlib stream using stored (type 0) deflate blocks + Adler-32.
fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x78); // CMF: deflate, 32K window
    out.push(0x01); // FLG: no dict, fastest (FCHECK makes 0x7801 % 31 == 0)
                    // Stored blocks, max 65535 bytes each.
    let mut i = 0;
    while i < data.len() {
        let chunk = &data[i..(i + 65535).min(data.len())];
        let is_last = i + chunk.len() >= data.len();
        out.push(if is_last { 1 } else { 0 }); // BFINAL + BTYPE=00
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
        i += chunk.len();
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// Adler-32 checksum (zlib trailer).
fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

/// Minimal CRC-32 (PNG/zlib polynomial), table-free for zero static state.
struct Crc32 {
    value: u32,
}

impl Crc32 {
    fn new() -> Self {
        Crc32 { value: 0xFFFF_FFFF }
    }
    fn update(&mut self, data: &[u8]) {
        for &byte in data {
            self.value ^= byte as u32;
            for _ in 0..8 {
                let mask = (self.value & 1).wrapping_neg();
                self.value = (self.value >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
    }
    fn finalize(self) -> u32 {
        self.value ^ 0xFFFF_FFFF
    }
}

/// The real headless-Chromium backend.
///
/// Its deterministic fallback flow is:
/// 1. Launch an offscreen Chromium with no network, an empty profile, and no
///    filesystem access beyond the served document — applying [`SandboxPolicy`].
/// 2. `Emulation.setDeviceMetricsOverride` to the requested `width`×`height`.
/// 3. `Page.addScriptToEvaluateOnNewDocument` with
///    [`deterministic_clock_script`] so the page clock is frozen before author
///    code runs.
/// 4. `Emulation.setVirtualTimePolicy { policy: "pause" }` to stop real time.
/// 5. Navigate to the document (inline `data:` URL for `Code`, or the template's
///    served `entry`).
/// 6. For each frame `i` in `0..duration_frames`: advance virtual time to
///    `i / fps` and call `OpenTake.seek(i / fps)`, then
///    `Page.captureScreenshot { format: "png", ... }` (transparent background
///    when `transparent`), writing the PNG to `cache_dir/frame_iiiii.png`.
/// 7. Return the [`RenderedClip`].
///
/// The CDP wiring is gated behind the `chromium` cargo feature so the default
/// build does not require a browser or websocket dependency. The live path
/// locates Chrome/Chromium/Edge, uses a fresh disposable profile, injects a
/// strict CSP, intercepts every request with `Fetch`, and kills the browser on
/// cancellation, timeout, or protocol failure. Without the feature, [`render`]
/// returns [`MotionError::RendererUnavailable`].
#[derive(Clone, Debug)]
pub struct HeadlessChromiumRenderer {
    cache: MotionCache,
    policy: SandboxPolicy,
    browser_path: Option<PathBuf>,
    cancellation: MotionCancellationToken,
}

impl HeadlessChromiumRenderer {
    /// Build the renderer with a cache and sandbox policy.
    pub fn new(cache: MotionCache, policy: SandboxPolicy) -> Self {
        HeadlessChromiumRenderer {
            cache,
            policy,
            browser_path: None,
            cancellation: MotionCancellationToken::new(),
        }
    }

    /// Override browser discovery. Useful for portable app bundles and for
    /// deterministic crash-path tests.
    pub fn with_browser_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.browser_path = Some(path.into());
        self
    }

    /// Attach a cooperative cancellation token.
    pub fn with_cancellation_token(mut self, token: MotionCancellationToken) -> Self {
        self.cancellation = token;
        self
    }

    /// Locate Chrome, Chromium, or Edge without launching it. An explicit
    /// `OPENTAKE_CHROMIUM_PATH` wins, followed by platform install locations and
    /// finally PATH.
    pub fn find_browser() -> Option<PathBuf> {
        if let Some(path) = std::env::var_os("OPENTAKE_CHROMIUM_PATH").map(PathBuf::from) {
            if path.is_file() {
                return Some(path);
            }
        }

        const COMMON: &[&str] = &[
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/usr/bin/microsoft-edge",
        ];
        if let Some(path) = COMMON.iter().map(PathBuf::from).find(|path| path.is_file()) {
            return Some(path);
        }

        for base in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
            let Some(base) = std::env::var_os(base) else {
                continue;
            };
            for relative in [
                "Google/Chrome/Application/chrome.exe",
                "Microsoft/Edge/Application/msedge.exe",
                "Chromium/Application/chrome.exe",
            ] {
                let path = PathBuf::from(&base).join(relative);
                if path.is_file() {
                    return Some(path);
                }
            }
        }

        let path = std::env::var_os("PATH")?;
        for dir in std::env::split_paths(&path) {
            for name in [
                "google-chrome-stable",
                "google-chrome",
                "chromium",
                "chromium-browser",
                "microsoft-edge",
                "chrome.exe",
                "msedge.exe",
            ] {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    /// The sandbox policy in effect.
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    /// The cache root used for rendered frames.
    pub fn cache(&self) -> &MotionCache {
        &self.cache
    }

    /// Build the inline `data:` document URL for a `Code` source. The
    /// deterministic clock script is injected by the engine via
    /// `addScriptToEvaluateOnNewDocument`, not inlined here, so author code can't
    /// observe or strip it from the document text. Pure → testable.
    pub fn data_url_for_code(html_css_js: &str) -> String {
        // Percent-encode the markup for a text/html data: URL. Engines accept
        // unescaped data: HTML, but encoding keeps it well-formed across CDP.
        let encoded = percent_encode_html(html_css_js);
        format!("data:text/html;charset=utf-8,{encoded}")
    }

    /// The plan of per-frame virtual-time stamps the backend will seek through:
    /// `[0/fps, 1/fps, ..., (n-1)/fps]`. Pure helper that documents and tests the
    /// time grid without launching anything.
    pub fn frame_time_grid(req: &MotionRenderRequest) -> Vec<f64> {
        (0..req.duration_frames)
            .map(|i| i as f64 / req.fps as f64)
            .collect()
    }
}

impl MotionRenderer for HeadlessChromiumRenderer {
    fn render(&self, req: &MotionRenderRequest) -> MotionResult<RenderedClip> {
        // Always validate + apply the sandbox checks we own, even on the path
        // that ends in "unavailable", so a caller wiring this up sees policy
        // failures regardless of whether a browser is present.
        req.validate()?;
        if let MotionSource::Code { html_css_js } = &req.source {
            self.policy.check_document_size(html_css_js)?;
        }

        #[cfg(feature = "chromium")]
        {
            chromium_backend::render(self, req)
        }
        #[cfg(not(feature = "chromium"))]
        {
            let _ = &self.cache;
            Err(MotionError::renderer_unavailable(
                "headless-Chromium backend is not compiled in; build with the \
                 `chromium` feature, or use StubRenderer for offline/deterministic rendering",
            ))
        }
    }
}

#[cfg(feature = "chromium")]
mod chromium_backend {
    use std::io::{BufRead, BufReader, Cursor};
    use std::net::TcpStream;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use base64::Engine as _;
    use opentake_process_tree::{configure_command, ProcessTree};
    use serde_json::{json, Value};
    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::{Message, WebSocket};

    use super::*;

    static PROFILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn trace(message: impl AsRef<str>) {
        if std::env::var_os("OPENTAKE_MOTION_TRACE").is_some() {
            eprintln!("[opentake-motion] {}", message.as_ref());
        }
    }

    pub(super) fn render(
        renderer: &HeadlessChromiumRenderer,
        req: &MotionRenderRequest,
    ) -> MotionResult<RenderedClip> {
        if renderer.cancellation.is_cancelled() {
            return Err(MotionError::Cancelled);
        }

        let browser_path = renderer
            .browser_path
            .clone()
            .or_else(HeadlessChromiumRenderer::find_browser)
            .ok_or_else(|| {
                MotionError::renderer_unavailable(
                    "no supported Chrome, Chromium, or Edge executable was found; set OPENTAKE_CHROMIUM_PATH",
                )
            })?;
        trace(format!("browser={}", browser_path.display()));

        let document = match &req.source {
            MotionSource::Code { html_css_js } => sandboxed_document(html_css_js, &renderer.policy),
            MotionSource::Template { id, .. } => {
                return Err(MotionError::unknown_template(format!(
                    "{id} (HeadlessChromiumRenderer requires the caller to resolve templates to inline Code)"
                )));
            }
        };

        let hash = content_hash(req);
        if renderer.cache.is_cached(req) {
            return Ok(clip_from_cache(req, hash, renderer.cache.dir_for(req)));
        }

        let dir = renderer.cache.begin_render(req)?;
        remove_partial_frames(&dir)?;
        let mut partial = PartialFrames::new(dir.clone());
        let deadline = Instant::now()
            .checked_add(renderer.policy.timeout)
            .unwrap_or_else(Instant::now);
        check_abort(renderer, deadline)?;

        let (mut browser, websocket_url) = BrowserProcess::launch(
            &browser_path,
            deadline,
            renderer.policy.timeout,
            &renderer.cancellation,
        )?;
        trace("browser launched and CDP endpoint is ready");
        let (socket, _) = tungstenite::connect(websocket_url.as_str()).map_err(|error| {
            MotionError::render_failed(format!("failed to connect to Chromium CDP: {error}"))
        })?;
        trace("connected to browser CDP");
        set_socket_poll_timeout(&socket)?;
        let mut cdp = Cdp::new(
            socket,
            renderer.policy.clone(),
            renderer.cancellation.clone(),
            deadline,
        );

        let target = cdp.command(
            "Target.createTarget",
            json!({"url": "about:blank", "background": false}),
            None,
        )?;
        let target_id = required_string(&target, "targetId")?;
        let attached = cdp.command(
            "Target.attachToTarget",
            json!({"targetId": target_id, "flatten": true}),
            None,
        )?;
        let session = required_string(&attached, "sessionId")?;
        trace("created and attached render target");

        cdp.command("Page.enable", json!({}), Some(&session))?;
        cdp.command("Runtime.enable", json!({}), Some(&session))?;
        cdp.command("Log.enable", json!({}), Some(&session))?;
        // View-based screenshots require an active target, and Windows can
        // throttle a background target's compositor indefinitely.
        cdp.command("Page.bringToFront", json!({}), Some(&session))?;
        cdp.command(
            "Fetch.enable",
            json!({"patterns": [{"urlPattern": "*", "requestStage": "Request"}]}),
            Some(&session),
        )?;
        cdp.set_device_metrics(&session, req.width, req.height)?;
        let alpha = if req.transparent { 0.0 } else { 1.0 };
        cdp.command(
            "Emulation.setDefaultBackgroundColorOverride",
            json!({"color": {"r": 255, "g": 255, "b": 255, "a": alpha}}),
            Some(&session),
        )?;
        cdp.command(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source": deterministic_clock_script()}),
            Some(&session),
        )?;

        let url = HeadlessChromiumRenderer::data_url_for_code(&document);
        cdp.command("Page.navigate", json!({"url": url}), Some(&session))?;
        cdp.wait_for_event("Page.loadEventFired", Some(&session))?;
        trace("inline motion document loaded");
        // Pausing before navigation also pauses the load lifecycle itself in
        // recent Chromium. The deterministic clock is already installed before
        // author code; freeze the browser's own timeline immediately after the
        // synchronous inline document has loaded and before any frame capture.
        cdp.command(
            "Emulation.setVirtualTimePolicy",
            json!({"policy": "pause"}),
            Some(&session),
        )?;
        cdp.ensure_no_blocked_url()?;

        let mut frames = Vec::with_capacity(req.duration_frames as usize);
        for (index, seconds) in HeadlessChromiumRenderer::frame_time_grid(req)
            .into_iter()
            .enumerate()
        {
            check_abort(renderer, deadline)?;
            trace(format!("frame {index}: seek start at {seconds:.17}s"));
            let expression = format!(
                "(async () => {{ if (!window.OpenTake) throw new Error('OpenTake clock missing'); await window.OpenTake.seek({seconds:.17}); return window.OpenTake.currentTime(); }})()"
            );
            let evaluated = cdp.command(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true
                }),
                Some(&session),
            )?;
            trace(format!("frame {index}: seek complete"));
            if evaluated
                .get("exceptionDetails")
                .and_then(Value::as_object)
                .is_some()
            {
                return Err(MotionError::render_failed(format!(
                    "author document failed while seeking frame {index}: {evaluated}"
                )));
            }
            cdp.ensure_no_blocked_url()?;

            trace(format!("frame {index}: compositor settle start"));
            cdp.settle_compositor(&session)?;
            trace(format!("frame {index}: compositor settle complete"));
            cdp.ensure_no_blocked_url()?;

            trace(format!("frame {index}: screenshot capture start"));
            let png =
                cdp.capture_frame_png(&session, req.transparent, req.width, req.height, index)?;
            check_abort(renderer, deadline)?;
            cdp.ensure_no_blocked_url()?;
            trace(format!("frame {index}: screenshot captured"));
            let path = MotionCache::frame_file(&dir, index);
            std::fs::write(&path, png)?;
            frames.push(path);
        }

        cdp.close_target(&target_id)?;
        check_abort(renderer, deadline)?;
        browser.shutdown()?;
        publish_completed_render(renderer, deadline, &dir, &mut partial)?;

        Ok(RenderedClip {
            content_hash: hash,
            frames,
            fps: req.fps,
            width: req.width,
            height: req.height,
            transparent: req.transparent,
        })
    }

    fn clip_from_cache(
        req: &MotionRenderRequest,
        content_hash: String,
        dir: PathBuf,
    ) -> RenderedClip {
        RenderedClip {
            content_hash,
            frames: (0..req.duration_frames as usize)
                .map(|index| MotionCache::frame_file(&dir, index))
                .collect(),
            fps: req.fps,
            width: req.width,
            height: req.height,
            transparent: req.transparent,
        }
    }

    fn sandboxed_document(document: &str, policy: &SandboxPolicy) -> String {
        let origins = policy
            .allowed_origins
            .iter()
            .map(|origin| origin.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let sources = if origins.is_empty() {
            "'none'".to_owned()
        } else {
            origins
        };
        let csp = format!(
            "default-src 'none'; script-src 'unsafe-inline' data: {sources}; style-src 'unsafe-inline' data: {sources}; img-src data: {sources}; media-src data: {sources}; font-src data: {sources}; connect-src {sources}; object-src 'none'; base-uri 'none'; form-action 'none'; frame-src 'none'; worker-src 'none'"
        );
        format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\"><style>html,body{{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}}</style>{document}"
        )
    }

    fn check_abort(renderer: &HeadlessChromiumRenderer, deadline: Instant) -> MotionResult<()> {
        check_abort_state(&renderer.cancellation, deadline, renderer.policy.timeout)
    }

    fn check_abort_state(
        cancellation: &MotionCancellationToken,
        deadline: Instant,
        timeout: Duration,
    ) -> MotionResult<()> {
        if cancellation.is_cancelled() {
            return Err(MotionError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(MotionError::Timeout(timeout));
        }
        Ok(())
    }

    fn publish_completed_render(
        renderer: &HeadlessChromiumRenderer,
        deadline: Instant,
        dir: &Path,
        partial: &mut PartialFrames,
    ) -> MotionResult<()> {
        check_abort(renderer, deadline)?;
        MotionCache::mark_complete(dir)?;
        if let Err(error) = check_abort(renderer, deadline) {
            MotionCache::remove_completion_marker(dir)?;
            return Err(error);
        }
        partial.commit();
        Ok(())
    }

    fn required_string(value: &Value, key: &str) -> MotionResult<String> {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                MotionError::render_failed(format!(
                    "Chromium CDP response is missing string field {key:?}: {value}"
                ))
            })
    }

    fn remove_partial_frames(dir: &Path) -> MotionResult<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("frame_") && name.ends_with(".png") {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    struct PartialFrames {
        dir: PathBuf,
        committed: bool,
    }

    impl PartialFrames {
        fn new(dir: PathBuf) -> Self {
            Self {
                dir,
                committed: false,
            }
        }

        fn commit(&mut self) {
            self.committed = true;
        }
    }

    impl Drop for PartialFrames {
        fn drop(&mut self) {
            if !self.committed {
                let _ = MotionCache::remove_completion_marker(&self.dir);
                let _ = remove_partial_frames(&self.dir);
            }
        }
    }

    struct BrowserProcess {
        child: Option<Child>,
        profile: PathBuf,
        tree: ProcessTree,
        shutdown_complete: bool,
    }

    impl BrowserProcess {
        fn launch(
            executable: &Path,
            deadline: Instant,
            timeout: Duration,
            cancellation: &MotionCancellationToken,
        ) -> MotionResult<(Self, String)> {
            let profile = unique_profile_dir();
            std::fs::create_dir_all(&profile)?;
            let mut command = Command::new(executable);
            command
                .args([
                    "--headless=new",
                    "--remote-debugging-port=0",
                    "--no-first-run",
                    "--no-default-browser-check",
                    "--disable-background-networking",
                    "--disable-component-update",
                    "--disable-client-side-phishing-detection",
                    "--disable-domain-reliability",
                    "--disable-sync",
                    "--disable-background-timer-throttling",
                    "--disable-backgrounding-occluded-windows",
                    "--disable-renderer-backgrounding",
                    "--run-all-compositor-stages-before-draw",
                    "--metrics-recording-only",
                    "--disable-breakpad",
                    "--disable-extensions",
                    "--disable-dev-shm-usage",
                    "--disable-features=FileSystemAccessAPI,InterestFeedContentSuggestions,OptimizationHints,MediaRouter",
                    "--password-store=basic",
                    "--use-mock-keychain",
                    "about:blank",
                ])
                .arg(format!("--user-data-dir={}", profile.display()))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            configure_command(&mut command);
            let mut child = command.spawn().map_err(|error| {
                let _ = std::fs::remove_dir_all(&profile);
                if error.kind() == std::io::ErrorKind::NotFound {
                    MotionError::renderer_unavailable(format!(
                        "Chromium executable does not exist at {}",
                        executable.display()
                    ))
                } else {
                    MotionError::render_failed(format!(
                        "failed to launch Chromium at {}: {error}",
                        executable.display()
                    ))
                }
            })?;
            let tree = match ProcessTree::attach(child.id()) {
                Ok(tree) => tree,
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_dir_all(&profile);
                    return Err(MotionError::render_failed(format!(
                        "failed to contain Chromium process tree: {error}"
                    )));
                }
            };
            let mut process = BrowserProcess {
                child: Some(child),
                profile,
                tree,
                shutdown_complete: false,
            };
            let Some(stderr) = process.child.as_mut().and_then(|child| child.stderr.take()) else {
                let _ = process.shutdown();
                return Err(MotionError::render_failed(
                    "Chromium stderr was not captured",
                ));
            };
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                for line in BufReader::new(stderr).lines() {
                    match line {
                        Ok(line) => {
                            if sender.send(line).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            loop {
                if cancellation.is_cancelled() {
                    return Err(MotionError::Cancelled);
                }
                if Instant::now() >= deadline {
                    return Err(MotionError::Timeout(timeout));
                }
                if let Some(status) = process
                    .child
                    .as_mut()
                    .expect("launched Chromium child is present")
                    .try_wait()?
                {
                    return Err(MotionError::render_failed(format!(
                        "Chromium exited before CDP was ready: {status}"
                    )));
                }
                match receiver.recv_timeout(Duration::from_millis(20)) {
                    Ok(line) => {
                        if let Some((_, url)) = line.split_once("DevTools listening on ") {
                            return Ok((process, url.trim().to_owned()));
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(MotionError::render_failed(
                            "Chromium closed stderr before publishing its CDP endpoint",
                        ));
                    }
                }
            }
        }

        fn shutdown(&mut self) -> std::io::Result<()> {
            const PROCESS_TREE_EXIT_TIMEOUT: Duration = Duration::from_secs(5);

            if self.shutdown_complete {
                return Ok(());
            }
            trace("browser process-tree shutdown start");
            let termination = self.tree.terminate();
            let child_wait = if let Some(mut child) = self.child.take() {
                // TerminateJobObject is authoritative on Windows; `kill` is a
                // fallback for an attach/termination failure and is harmless
                // when the root has already exited.
                let _ = child.kill();
                let result = child.wait().map(|_| ());
                // ActiveProcesses stays nonzero while an external process
                // handle remains open, so release Child before the Job query.
                drop(child);
                result
            } else {
                Ok(())
            };

            termination?;
            self.tree.wait_for_exit(PROCESS_TREE_EXIT_TIMEOUT)?;
            self.tree.disarm();
            self.shutdown_complete = true;
            child_wait?;
            trace("browser process-tree shutdown complete");
            Ok(())
        }

        fn remove_profile(&self) {
            // Chromium can keep helper processes alive for a few milliseconds
            // after its root process exits. Those helpers may race a one-shot
            // remove_dir_all by creating a final state file, leaving profiles
            // behind on Linux timeout and cancellation paths.
            const CLEANUP_ATTEMPTS: usize = 100;
            for attempt in 0..CLEANUP_ATTEMPTS {
                match std::fs::remove_dir_all(&self.profile) {
                    Ok(()) => return,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                    Err(_) if attempt + 1 < CLEANUP_ATTEMPTS => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => return,
                }
            }
        }
    }

    impl Drop for BrowserProcess {
        fn drop(&mut self) {
            let _ = self.shutdown();
            self.remove_profile();
        }
    }

    fn unique_profile_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let counter = PROFILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "opentake-chromium-{}-{nanos}-{counter}",
            std::process::id()
        ))
    }

    type CdpSocket = WebSocket<MaybeTlsStream<TcpStream>>;

    fn set_socket_poll_timeout(socket: &CdpSocket) -> MotionResult<()> {
        match socket.get_ref() {
            MaybeTlsStream::Plain(stream) => stream
                .set_read_timeout(Some(Duration::from_millis(50)))
                .map_err(MotionError::Io),
            _ => Err(MotionError::render_failed(
                "the local Chromium CDP endpoint unexpectedly used TLS",
            )),
        }
    }

    struct Cdp {
        socket: CdpSocket,
        next_id: u64,
        policy: SandboxPolicy,
        cancellation: MotionCancellationToken,
        deadline: Instant,
        blocked_url: Option<String>,
        pending_events: Vec<Value>,
    }

    impl Cdp {
        fn new(
            socket: CdpSocket,
            policy: SandboxPolicy,
            cancellation: MotionCancellationToken,
            deadline: Instant,
        ) -> Self {
            Self {
                socket,
                next_id: 1,
                policy,
                cancellation,
                deadline,
                blocked_url: None,
                pending_events: Vec::new(),
            }
        }

        fn command(
            &mut self,
            method: &str,
            params: Value,
            session: Option<&str>,
        ) -> MotionResult<Value> {
            let id = self.next_id;
            self.next_id += 1;
            let mut message = json!({"id": id, "method": method, "params": params});
            if let Some(session) = session {
                message["sessionId"] = Value::String(session.to_owned());
            }
            self.send(message)?;

            loop {
                let value = self.read()?;
                if value.get("id").and_then(Value::as_u64) == Some(id) {
                    if let Some(error) = value.get("error") {
                        return Err(MotionError::render_failed(format!(
                            "Chromium CDP {method} failed: {error}"
                        )));
                    }
                    return Ok(value.get("result").cloned().unwrap_or_else(|| json!({})));
                }
                self.handle_event_or_queue(value)?;
            }
        }

        fn wait_for_event(&mut self, method: &str, session: Option<&str>) -> MotionResult<Value> {
            if let Some(index) = self.pending_events.iter().position(|event| {
                event.get("method").and_then(Value::as_str) == Some(method)
                    && session.is_none_or(|expected| {
                        event.get("sessionId").and_then(Value::as_str) == Some(expected)
                    })
            }) {
                return Ok(self.pending_events.remove(index));
            }
            loop {
                let value = self.read()?;
                if value.get("method").and_then(Value::as_str) == Some(method)
                    && session.is_none_or(|expected| {
                        value.get("sessionId").and_then(Value::as_str) == Some(expected)
                    })
                {
                    return Ok(value);
                }
                self.handle_event_or_queue(value)?;
            }
        }

        fn ensure_no_blocked_url(&mut self) -> MotionResult<()> {
            if let Some(blocked) = self.blocked_url.take() {
                Err(MotionError::sandbox(format!(
                    "network access to {blocked:?} is not in the allowlist"
                )))
            } else {
                Ok(())
            }
        }

        fn settle_compositor(&mut self, session: &str) -> MotionResult<()> {
            trace("virtual-time advance command start");
            self.command(
                "Emulation.setVirtualTimePolicy",
                json!({
                    "policy": "advance",
                    "budget": 1,
                    "maxVirtualTimeTaskStarvationCount": 10_000
                }),
                Some(session),
            )?;
            trace("virtual-time advance command complete; budget expiry wait start");
            self.wait_for_event("Emulation.virtualTimeBudgetExpired", Some(session))?;
            trace("virtual-time budget expired");
            Ok(())
        }

        fn set_device_metrics(
            &mut self,
            session: &str,
            width: u32,
            height: u32,
        ) -> MotionResult<()> {
            self.command(
                "Emulation.setDeviceMetricsOverride",
                device_metrics_params(width, height)?,
                Some(session),
            )?;
            Ok(())
        }

        fn capture_viewport(
            &mut self,
            session: &str,
            width: u32,
            height: u32,
        ) -> MotionResult<Value> {
            let guard_width = guarded_dimension(width)?;
            let guard_height = guarded_dimension(height)?;
            // Capture the page plus one CSS pixel of page-external guard on the
            // right and bottom. `fromSurface=false` can expose an unstable OS
            // backing-view boundary there; the guard is cropped before resize.
            self.command(
                "Page.captureScreenshot",
                json!({
                    "format": "png",
                    "fromSurface": false,
                    "captureBeyondViewport": false,
                    "optimizeForSpeed": false,
                    "clip": {
                        "x": 0,
                        "y": 0,
                        "width": guard_width,
                        "height": guard_height,
                        "scale": 1
                    }
                }),
                Some(session),
            )
        }

        fn capture_frame_png(
            &mut self,
            session: &str,
            transparent: bool,
            width: u32,
            height: u32,
            frame_index: usize,
        ) -> MotionResult<Vec<u8>> {
            self.check_abort()?;
            if !transparent {
                let image = self.capture_normalized_viewport_image(
                    session,
                    "opaque",
                    width,
                    height,
                    frame_index,
                )?;
                self.check_abort()?;
                let normalized = encode_viewport_png(image, frame_index)?;
                self.check_abort()?;
                return Ok(normalized);
            }

            let black = self.capture_stable_background_image(
                session,
                [0, 0, 0, 255],
                "black",
                width,
                height,
                frame_index,
            )?;
            let white = self.capture_stable_background_image(
                session,
                [255, 255, 255, 255],
                "white",
                width,
                height,
                frame_index,
            )?;
            self.check_abort()?;
            let recovered = recover_transparent_images(black, white, frame_index)?;
            self.check_abort()?;
            Ok(recovered)
        }

        fn check_abort(&self) -> MotionResult<()> {
            check_abort_state(&self.cancellation, self.deadline, self.policy.timeout)
        }

        fn close_target(&mut self, target_id: &str) -> MotionResult<()> {
            self.command("Target.closeTarget", json!({"targetId": target_id}), None)?;
            self.ensure_no_blocked_url()
        }

        fn set_capture_background(&mut self, session: &str, rgba: [u8; 4]) -> MotionResult<()> {
            self.command(
                "Emulation.setDefaultBackgroundColorOverride",
                json!({
                    "color": {
                        "r": rgba[0],
                        "g": rgba[1],
                        "b": rgba[2],
                        "a": f64::from(rgba[3]) / 255.0
                    }
                }),
                Some(session),
            )?;
            self.ensure_no_blocked_url()
        }

        fn capture_stable_background_image(
            &mut self,
            session: &str,
            rgba: [u8; 4],
            background: &str,
            width: u32,
            height: u32,
            frame_index: usize,
        ) -> MotionResult<image::RgbaImage> {
            self.set_capture_background(session, rgba)?;
            trace(format!(
                "frame {frame_index}: {background}-background readback fence start"
            ));
            let fence = self.capture_normalized_viewport_image(
                session,
                background,
                width,
                height,
                frame_index,
            )?;
            self.check_abort()?;
            trace(format!(
                "frame {frame_index}: {background}-background used readback start"
            ));
            let captured = self.capture_normalized_viewport_image(
                session,
                background,
                width,
                height,
                frame_index,
            )?;
            self.check_abort()?;
            ensure_stable_viewport_images(&fence, &captured, background, frame_index)?;
            self.check_abort()?;
            Ok(captured)
        }

        fn capture_normalized_viewport_image(
            &mut self,
            session: &str,
            background: &str,
            width: u32,
            height: u32,
            frame_index: usize,
        ) -> MotionResult<image::RgbaImage> {
            let png = self.capture_viewport_png(session, width, height, frame_index)?;
            self.check_abort()?;
            let image = decode_viewport_png(&png, background, frame_index)?;
            normalize_guarded_viewport_image(image, width, height, background, frame_index)
        }

        fn capture_viewport_png(
            &mut self,
            session: &str,
            width: u32,
            height: u32,
            frame_index: usize,
        ) -> MotionResult<Vec<u8>> {
            let captured = self.capture_viewport(session, width, height)?;
            self.ensure_no_blocked_url()?;
            let encoded = required_string(&captured, "data")?;
            let png = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|error| {
                    MotionError::render_failed(format!(
                        "Chromium returned malformed screenshot data for frame {frame_index}: {error}"
                    ))
                })?;
            if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Err(MotionError::render_failed(format!(
                    "Chromium returned a non-PNG screenshot for frame {frame_index}"
                )));
            }
            Ok(png)
        }

        fn send(&mut self, value: Value) -> MotionResult<()> {
            self.socket
                .send(Message::text(value.to_string()))
                .map_err(|error| {
                    MotionError::render_failed(format!(
                        "failed to send Chromium CDP command: {error}"
                    ))
                })
        }

        fn read(&mut self) -> MotionResult<Value> {
            loop {
                self.check_abort()?;
                match self.socket.read() {
                    Ok(Message::Text(text)) => {
                        return serde_json::from_str(text.as_ref()).map_err(|error| {
                            MotionError::render_failed(format!(
                                "Chromium sent malformed CDP JSON: {error}"
                            ))
                        });
                    }
                    Ok(Message::Ping(payload)) => {
                        self.socket.send(Message::Pong(payload)).map_err(|error| {
                            MotionError::render_failed(format!(
                                "failed to answer Chromium CDP ping: {error}"
                            ))
                        })?;
                    }
                    Ok(Message::Close(reason)) => {
                        return Err(MotionError::render_failed(format!(
                            "Chromium CDP connection closed unexpectedly: {reason:?}"
                        )));
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(error))
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) => {}
                    Err(error) => {
                        return Err(MotionError::render_failed(format!(
                            "failed to read Chromium CDP response: {error}"
                        )));
                    }
                }
            }
        }

        fn handle_event_or_queue(&mut self, value: Value) -> MotionResult<()> {
            match value.get("method").and_then(Value::as_str) {
                Some("Fetch.requestPaused") => self.handle_request(&value),
                Some("Log.entryAdded") => {
                    let text = value
                        .get("params")
                        .and_then(|params| params.get("entry"))
                        .and_then(|entry| entry.get("text"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if (text.contains("Content Security Policy")
                        || text.contains("Refused to load")
                        || text.contains("Not allowed to load local resource")
                        || text.contains("violates the following"))
                        && self.blocked_url.is_none()
                    {
                        self.blocked_url = Some(text.to_owned());
                    }
                    Ok(())
                }
                Some("Inspector.targetCrashed" | "Target.targetCrashed") => {
                    Err(MotionError::render_failed("Chromium render target crashed"))
                }
                Some(_) => {
                    if self.pending_events.len() >= 256 {
                        self.pending_events.remove(0);
                    }
                    self.pending_events.push(value);
                    Ok(())
                }
                None => Ok(()),
            }
        }

        fn handle_request(&mut self, event: &Value) -> MotionResult<()> {
            let params = event.get("params").ok_or_else(|| {
                MotionError::render_failed("Fetch.requestPaused event has no params")
            })?;
            let request_id = required_string(params, "requestId")?;
            let url = params
                .get("request")
                .and_then(|request| request.get("url"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    MotionError::render_failed("Fetch.requestPaused event has no request URL")
                })?;
            let session = event.get("sessionId").and_then(Value::as_str);
            let allowed = url == "about:blank" || self.policy.check_url(url).is_ok();
            let id = self.next_id;
            self.next_id += 1;
            let (method, params) = if allowed {
                ("Fetch.continueRequest", json!({"requestId": request_id}))
            } else {
                if self.blocked_url.is_none() {
                    self.blocked_url = Some(url.to_owned());
                }
                (
                    "Fetch.failRequest",
                    json!({"requestId": request_id, "errorReason": "BlockedByClient"}),
                )
            };
            let mut message = json!({"id": id, "method": method, "params": params});
            if let Some(session) = session {
                message["sessionId"] = Value::String(session.to_owned());
            }
            trace(format!("{method}: request observed"));
            self.send(message)
        }
    }

    fn guarded_dimension(value: u32) -> MotionResult<u32> {
        value.checked_add(1).ok_or_else(|| {
            MotionError::render_failed("viewport dimension overflowed while adding capture guard")
        })
    }

    fn device_metrics_params(width: u32, height: u32) -> MotionResult<Value> {
        let guard_width = guarded_dimension(width)?;
        let guard_height = guarded_dimension(height)?;
        Ok(json!({
            "width": width,
            "height": height,
            "deviceScaleFactor": 1,
            "mobile": false,
            "screenWidth": width,
            "screenHeight": height,
            "viewport": {
                "x": 0,
                "y": 0,
                "width": guard_width,
                "height": guard_height,
                "scale": 1
            }
        }))
    }

    fn recover_transparent_images(
        black: image::RgbaImage,
        white: image::RgbaImage,
        frame_index: usize,
    ) -> MotionResult<Vec<u8>> {
        if black.dimensions() != white.dimensions() {
            return Err(MotionError::render_failed(format!(
                "Chromium returned inconsistent viewport sizes for frame {frame_index}: black={:?}, white={:?}",
                black.dimensions(),
                white.dimensions()
            )));
        }

        let rgba = recover_straight_alpha(black.as_raw(), white.as_raw())?;
        let (width, height) = black.dimensions();
        let image = image::RgbaImage::from_raw(width, height, rgba).ok_or_else(|| {
            MotionError::render_failed(format!(
                "recovered RGBA buffer has the wrong size for frame {frame_index}"
            ))
        })?;
        encode_viewport_png(image, frame_index)
    }

    fn ensure_stable_viewport_images(
        fence: &image::RgbaImage,
        captured: &image::RgbaImage,
        background: &str,
        frame_index: usize,
    ) -> MotionResult<()> {
        if fence.dimensions() != captured.dimensions() {
            return Err(MotionError::render_failed(format!(
                "Chromium {background}-background returned inconsistent stable-readback sizes for frame {frame_index}: fence={:?}, captured={:?}",
                fence.dimensions(),
                captured.dimensions()
            )));
        }
        if fence.as_raw() != captured.as_raw() {
            let differing_channels = fence
                .as_raw()
                .iter()
                .zip(captured.as_raw())
                .filter(|(left, right)| left != right)
                .count();
            return Err(MotionError::render_failed(format!(
                "Chromium {background}-background did not reach a stable readback for frame {frame_index}: {differing_channels} RGBA channels changed"
            )));
        }
        Ok(())
    }

    fn decode_viewport_png(
        png: &[u8],
        background: &str,
        frame_index: usize,
    ) -> MotionResult<image::RgbaImage> {
        let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .map(image::DynamicImage::into_rgba8)
            .map_err(|error| {
                MotionError::render_failed(format!(
                    "Chromium returned an invalid {background}-background PNG for frame {frame_index}: {error}"
                ))
            })?;
        let (actual_width, actual_height) = image.dimensions();
        if actual_width == 0 || actual_height == 0 {
            return Err(MotionError::render_failed(format!(
                "Chromium returned an empty {background}-background viewport for frame {frame_index}"
            )));
        }
        Ok(image)
    }

    fn normalize_guarded_viewport_image(
        image: image::RgbaImage,
        width: u32,
        height: u32,
        background: &str,
        frame_index: usize,
    ) -> MotionResult<image::RgbaImage> {
        let (actual_width, actual_height) = image.dimensions();
        let guard_width = guarded_dimension(width)?;
        let guard_height = guarded_dimension(height)?;
        if u64::from(actual_width) * u64::from(guard_height)
            != u64::from(actual_height) * u64::from(guard_width)
        {
            return Err(MotionError::render_failed(format!(
                "Chromium returned an unexpected guarded {background}-background viewport size for frame {frame_index}: actual=({actual_width}, {actual_height}), guard=({guard_width}, {guard_height})"
            )));
        }

        let content_width_numerator = u64::from(actual_width) * u64::from(width);
        let content_height_numerator = u64::from(actual_height) * u64::from(height);
        if content_width_numerator % u64::from(guard_width) != 0
            || content_height_numerator % u64::from(guard_height) != 0
        {
            return Err(MotionError::render_failed(format!(
                "Chromium guarded viewport does not map to integer page pixels for frame {frame_index}: actual=({actual_width}, {actual_height}), guard=({guard_width}, {guard_height})"
            )));
        }
        let content_width = u32::try_from(content_width_numerator / u64::from(guard_width))
            .map_err(|_| MotionError::render_failed("guarded viewport width overflowed"))?;
        let content_height = u32::try_from(content_height_numerator / u64::from(guard_height))
            .map_err(|_| MotionError::render_failed("guarded viewport height overflowed"))?;
        if content_width == 0 || content_height == 0 {
            return Err(MotionError::render_failed(format!(
                "Chromium guarded viewport mapped to an empty page for frame {frame_index}"
            )));
        }
        Ok(resize_guarded_subimage(
            &image,
            content_width,
            content_height,
            width,
            height,
        ))
    }

    fn resize_guarded_subimage(
        image: &image::RgbaImage,
        content_width: u32,
        content_height: u32,
        width: u32,
        height: u32,
    ) -> image::RgbaImage {
        let content = image::imageops::crop_imm(image, 0, 0, content_width, content_height);
        if (content_width, content_height) == (width, height) {
            content.to_image()
        } else {
            image::imageops::resize(
                &*content,
                width,
                height,
                image::imageops::FilterType::Triangle,
            )
        }
    }

    fn encode_viewport_png(image: image::RgbaImage, frame_index: usize) -> MotionResult<Vec<u8>> {
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .map_err(|error| {
                MotionError::render_failed(format!(
                    "failed to encode recovered transparent frame {frame_index}: {error}"
                ))
            })?;
        Ok(encoded.into_inner())
    }

    fn recover_straight_alpha(black: &[u8], white: &[u8]) -> MotionResult<Vec<u8>> {
        if black.len() != white.len() || !black.len().is_multiple_of(4) {
            return Err(MotionError::render_failed(
                "black/white viewport captures have incompatible RGBA buffers",
            ));
        }

        let mut recovered = Vec::with_capacity(black.len());
        for (black, white) in black.chunks_exact(4).zip(white.chunks_exact(4)) {
            let mut deltas = [
                white[0].saturating_sub(black[0]),
                white[1].saturating_sub(black[1]),
                white[2].saturating_sub(black[2]),
            ];
            deltas.sort_unstable();
            let alpha = 255_u8.saturating_sub(deltas[1]);
            for channel in &black[..3] {
                let straight = if alpha == 0 {
                    0
                } else {
                    ((u32::from(*channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255)
                        as u8
                };
                recovered.push(straight);
            }
            recovered.push(alpha);
        }
        Ok(recovered)
    }

    #[cfg(test)]
    mod tests {
        use std::net::{TcpListener, TcpStream};

        use tungstenite::protocol::Role;

        use super::*;

        #[test]
        fn compositor_fence_advances_a_finite_budget_and_waits_for_expiry() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            let mut server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            let server = thread::spawn(move || {
                let request = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected text CDP request, got {other:?}"),
                };
                assert_eq!(
                    request,
                    json!({
                        "id": 1,
                        "method": "Emulation.setVirtualTimePolicy",
                        "params": {
                            "policy": "advance",
                            "budget": 1,
                            "maxVirtualTimeTaskStarvationCount": 10_000
                        },
                        "sessionId": "render-session"
                    })
                );
                server_socket
                    .send(Message::text(
                        json!({
                            "method": "Emulation.virtualTimeBudgetExpired",
                            "params": {},
                            "sessionId": "render-session"
                        })
                        .to_string(),
                    ))
                    .unwrap();
                server_socket
                    .send(Message::text(json!({"id": 1, "result": {}}).to_string()))
                    .unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            cdp.settle_compositor("render-session").unwrap();
            assert!(cdp.pending_events.is_empty());
            server.join().unwrap();
        }

        #[test]
        fn device_metrics_add_a_page_external_guard_without_changing_layout_size() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            let mut server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            let server = thread::spawn(move || {
                let request = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected device-metrics request, got {other:?}"),
                };
                assert_eq!(
                    request,
                    json!({
                        "id": 1,
                        "method": "Emulation.setDeviceMetricsOverride",
                        "params": {
                            "width": 48,
                            "height": 32,
                            "deviceScaleFactor": 1,
                            "mobile": false,
                            "screenWidth": 48,
                            "screenHeight": 32,
                            "viewport": {
                                "x": 0,
                                "y": 0,
                                "width": 49,
                                "height": 33,
                                "scale": 1
                            }
                        },
                        "sessionId": "render-session"
                    })
                );
                server_socket
                    .send(Message::text(json!({"id": 1, "result": {}}).to_string()))
                    .unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            cdp.set_device_metrics("render-session", 48, 32).unwrap();
            server.join().unwrap();
        }

        #[test]
        fn viewport_capture_avoids_surface_and_includes_the_guard() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            let mut server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            let server = thread::spawn(move || {
                let request = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected text CDP request, got {other:?}"),
                };
                assert_eq!(
                    request,
                    json!({
                        "id": 1,
                        "method": "Page.captureScreenshot",
                        "params": {
                            "format": "png",
                            "fromSurface": false,
                            "captureBeyondViewport": false,
                            "optimizeForSpeed": false,
                            "clip": {
                                "x": 0,
                                "y": 0,
                                "width": 49,
                                "height": 33,
                                "scale": 1
                            }
                        },
                        "sessionId": "render-session"
                    })
                );
                server_socket
                    .send(Message::text(
                        json!({"id": 1, "result": {"data": "png"}}).to_string(),
                    ))
                    .unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            assert_eq!(
                cdp.capture_viewport("render-session", 48, 32).unwrap(),
                json!({"data": "png"})
            );
            server.join().unwrap();
        }

        #[test]
        fn dual_background_view_capture_recovers_straight_alpha() {
            let black = [
                0, 0, 0, 255, // fully transparent
                128, 0, 0, 255, // 50% red over black
                10, 20, 30, 255, // opaque color
            ];
            let white = [
                255, 255, 255, 255, // fully transparent
                255, 127, 127, 255, // 50% red over white
                10, 20, 30, 255, // opaque color
            ];
            assert_eq!(
                recover_straight_alpha(&black, &white).unwrap(),
                vec![
                    0, 0, 0, 0, // transparent RGB canonicalizes to zero
                    255, 0, 0, 128, // straight, not premultiplied, red
                    10, 20, 30, 255,
                ]
            );

            assert!(matches!(
                recover_straight_alpha(&black[..4], &white),
                Err(MotionError::RenderFailed(_))
            ));

            let first = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
            let changed = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
            assert!(matches!(
                ensure_stable_viewport_images(&first, &changed, "black", 0),
                Err(MotionError::RenderFailed(_))
            ));
        }

        #[test]
        fn guarded_viewport_discards_only_page_external_pixels() {
            // requested=1x1, guard=2x2 CSS px, backing scale=2 => raw=4x4.
            // The requested page occupies the upper-left 2x2 backing pixels.
            let mut first = image::RgbaImage::from_pixel(4, 4, image::Rgba([10, 20, 30, 40]));
            let mut second = first.clone();
            for coordinate in 0..4 {
                first.put_pixel(3, coordinate, image::Rgba([200, 0, 0, 255]));
                first.put_pixel(coordinate, 3, image::Rgba([0, 200, 0, 255]));
                second.put_pixel(2, coordinate, image::Rgba([0, 0, 200, 255]));
                second.put_pixel(coordinate, 2, image::Rgba([200, 200, 0, 255]));
            }
            assert_eq!(
                normalize_guarded_viewport_image(first.clone(), 1, 1, "black", 0).unwrap(),
                normalize_guarded_viewport_image(second, 1, 1, "black", 0).unwrap(),
                "the full page-external guard must not enter the requested canvas"
            );

            let mut content_changed = first.clone();
            content_changed.put_pixel(1, 1, image::Rgba([0, 0, 200, 255]));
            assert_ne!(
                normalize_guarded_viewport_image(first, 1, 1, "black", 0).unwrap(),
                normalize_guarded_viewport_image(content_changed, 1, 1, "black", 0).unwrap(),
                "content inside the requested page must remain observable"
            );
        }

        #[test]
        fn guarded_subimage_resizes_without_losing_valid_right_bottom_content() {
            let mut raw = image::RgbaImage::from_pixel(4, 4, image::Rgba([240, 0, 240, 255]));
            raw.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
            raw.put_pixel(1, 0, image::Rgba([0, 255, 0, 255]));
            raw.put_pixel(0, 1, image::Rgba([0, 0, 255, 255]));
            raw.put_pixel(1, 1, image::Rgba([255, 255, 0, 255]));

            let expected_content = image::imageops::crop_imm(&raw, 0, 0, 2, 2).to_image();
            let expected = image::imageops::resize(
                &expected_content,
                1,
                1,
                image::imageops::FilterType::Triangle,
            );
            assert_eq!(resize_guarded_subimage(&raw, 2, 2, 1, 1), expected);

            let exact = resize_guarded_subimage(&raw, 2, 2, 2, 2);
            assert_eq!(exact, expected_content);
            assert_eq!(
                exact.get_pixel(1, 1).0,
                [255, 255, 0, 255],
                "valid content touching the requested right/bottom edge must be preserved"
            );
        }

        #[test]
        fn transparent_capture_uses_stable_readbacks_without_advancing_author_time() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            let mut server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            let black_png = base64::engine::general_purpose::STANDARD.encode(
                encode_viewport_png(
                    image::RgbaImage::from_pixel(2, 2, image::Rgba([128, 0, 0, 255])),
                    0,
                )
                .unwrap(),
            );
            let white_png = base64::engine::general_purpose::STANDARD.encode(
                encode_viewport_png(
                    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 127, 127, 255])),
                    0,
                )
                .unwrap(),
            );
            let server = thread::spawn(move || {
                let backgrounds = [([0, 0, 0], black_png), ([255, 255, 255], white_png)];
                let mut next_id = 1;
                for (rgb, png) in backgrounds {
                    let background = match server_socket.read().unwrap() {
                        Message::Text(text) => {
                            serde_json::from_str::<Value>(text.as_ref()).unwrap()
                        }
                        other => panic!("expected background override, got {other:?}"),
                    };
                    assert_eq!(
                        background,
                        json!({
                            "id": next_id,
                            "method": "Emulation.setDefaultBackgroundColorOverride",
                            "params": {"color": {"r": rgb[0], "g": rgb[1], "b": rgb[2], "a": 1.0}},
                            "sessionId": "render-session"
                        })
                    );
                    server_socket
                        .send(Message::text(
                            json!({"id": next_id, "result": {}}).to_string(),
                        ))
                        .unwrap();
                    next_id += 1;

                    for _ in 0..2 {
                        let capture = match server_socket.read().unwrap() {
                            Message::Text(text) => {
                                serde_json::from_str::<Value>(text.as_ref()).unwrap()
                            }
                            other => panic!("expected view readback fence, got {other:?}"),
                        };
                        assert_eq!(capture["id"], next_id);
                        assert_eq!(capture["method"], "Page.captureScreenshot");
                        assert_eq!(capture["params"]["fromSurface"], false);
                        assert_eq!(capture["params"]["clip"]["width"], 2);
                        assert_eq!(capture["params"]["clip"]["height"], 2);
                        assert_eq!(capture["sessionId"], "render-session");
                        server_socket
                            .send(Message::text(
                                json!({"id": next_id, "result": {"data": png}}).to_string(),
                            ))
                            .unwrap();
                        next_id += 1;
                    }
                }
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            let png = cdp
                .capture_frame_png("render-session", true, 1, 1, 0)
                .unwrap();
            assert_eq!(
                image::load_from_memory(&png)
                    .unwrap()
                    .to_rgba8()
                    .get_pixel(0, 0)
                    .0,
                [255, 0, 0, 128]
            );
            server.join().unwrap();
        }

        #[test]
        fn completion_checkpoint_rejects_timeout_and_cancellation_without_cache_commit() {
            fn complete_frames(dir: &Path) {
                for index in 0..2 {
                    std::fs::write(MotionCache::frame_file(dir, index), b"png").unwrap();
                }
            }

            let root = tempfile::tempdir().unwrap();
            let cache = MotionCache::new(root.path());
            let request = MotionRenderRequest::new(MotionSource::code("<timeout/>"), 30, 2, 8, 8);
            let dir = cache.begin_render(&request).unwrap();
            complete_frames(&dir);
            let mut partial = PartialFrames::new(dir.clone());
            let renderer = HeadlessChromiumRenderer::new(
                cache.clone(),
                SandboxPolicy::offline_with_timeout(Duration::from_secs(1)),
            );
            assert!(matches!(
                publish_completed_render(&renderer, Instant::now(), &dir, &mut partial),
                Err(MotionError::Timeout(_))
            ));
            drop(partial);
            assert!(!cache.is_cached(&request));

            let cancelled_request =
                MotionRenderRequest::new(MotionSource::code("<cancelled/>"), 30, 2, 8, 8);
            let cancelled_dir = cache.begin_render(&cancelled_request).unwrap();
            complete_frames(&cancelled_dir);
            let mut cancelled_partial = PartialFrames::new(cancelled_dir.clone());
            let cancellation = MotionCancellationToken::new();
            cancellation.cancel();
            let cancelled_renderer = HeadlessChromiumRenderer::new(
                cache.clone(),
                SandboxPolicy::offline_with_timeout(Duration::from_secs(1)),
            )
            .with_cancellation_token(cancellation);
            assert!(matches!(
                publish_completed_render(
                    &cancelled_renderer,
                    Instant::now() + Duration::from_secs(1),
                    &cancelled_dir,
                    &mut cancelled_partial
                ),
                Err(MotionError::Cancelled)
            ));
            drop(cancelled_partial);
            assert!(!cache.is_cached(&cancelled_request));
        }

        #[test]
        fn target_close_fails_closed_on_a_late_blocked_request() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            let mut server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            let server = thread::spawn(move || {
                let close = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected closeTarget request, got {other:?}"),
                };
                assert_eq!(
                    close,
                    json!({
                        "id": 1,
                        "method": "Target.closeTarget",
                        "params": {"targetId": "render-target"}
                    })
                );
                server_socket
                    .send(Message::text(
                        json!({
                            "method": "Fetch.requestPaused",
                            "params": {
                                "requestId": "late-request",
                                "request": {"url": "https://example.com/late"}
                            },
                            "sessionId": "render-session"
                        })
                        .to_string(),
                    ))
                    .unwrap();
                server_socket
                    .send(Message::text(
                        json!({"id": 1, "result": {"success": true}}).to_string(),
                    ))
                    .unwrap();
                let failed = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected Fetch.failRequest, got {other:?}"),
                };
                assert_eq!(failed["method"], "Fetch.failRequest");
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            assert!(matches!(
                cdp.close_target("render-target"),
                Err(MotionError::Sandbox(_))
            ));
            server.join().unwrap();
        }

        #[test]
        fn partial_frame_guard_only_preserves_a_published_marker_after_commit() {
            let root = tempfile::tempdir().unwrap();
            let cache = MotionCache::new(root.path());
            let request = MotionRenderRequest::new(MotionSource::code("<x/>"), 30, 2, 8, 8);
            let dir = cache.begin_render(&request).unwrap();
            for index in 0..2 {
                std::fs::write(MotionCache::frame_file(&dir, index), b"png").unwrap();
            }
            MotionCache::mark_complete(&dir).unwrap();
            assert!(cache.is_cached(&request));

            drop(PartialFrames::new(dir.clone()));
            assert!(
                !cache.is_cached(&request),
                "an uncommitted guard must invalidate a published marker"
            );

            let dir = cache.begin_render(&request).unwrap();
            for index in 0..2 {
                std::fs::write(MotionCache::frame_file(&dir, index), b"png").unwrap();
            }
            MotionCache::mark_complete(&dir).unwrap();
            let mut completed = PartialFrames::new(dir);
            completed.commit();
            drop(completed);
            assert!(cache.is_cached(&request));
        }
    }
}

/// Percent-encode HTML for a `data:` URL: keep unreserved chars, encode the rest.
fn percent_encode_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &byte in s.as_bytes() {
        let keep = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if keep {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(hex_upper(byte >> 4));
            out.push(hex_upper(byte & 0x0f));
        }
    }
    out
}

fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + (nibble - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::MotionSource;

    fn stub_with_tmp() -> (StubRenderer, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let cache = MotionCache::new(tmp.path());
        (StubRenderer::new(cache), tmp)
    }

    #[test]
    fn clock_script_exposes_seek_contract() {
        let s = deterministic_clock_script();
        assert!(s.contains("OpenTake"));
        assert!(s.contains("seek"));
        assert!(s.contains("currentTime"));
        assert!(s.contains("onSeek"));
    }

    #[test]
    fn stub_renders_expected_number_of_frames() {
        let (renderer, _tmp) = stub_with_tmp();
        let req = MotionRenderRequest::new(MotionSource::code("<div>hi</div>"), 30, 5, 16, 8);
        let clip = renderer.render(&req).unwrap();
        assert_eq!(clip.frame_count(), 5);
        assert_eq!(clip.width, 16);
        assert_eq!(clip.height, 8);
        assert_eq!(clip.content_hash, content_hash(&req));
        for p in &clip.frames {
            assert!(p.exists(), "frame file should exist: {p:?}");
        }
    }

    #[test]
    fn stub_output_is_deterministic() {
        // Two separate caches, same request -> identical frame bytes.
        let tmp_a = tempfile::tempdir().unwrap();
        let tmp_b = tempfile::tempdir().unwrap();
        let ra = StubRenderer::new(MotionCache::new(tmp_a.path()));
        let rb = StubRenderer::new(MotionCache::new(tmp_b.path()));
        let req = MotionRenderRequest::new(MotionSource::code("<x/>"), 24, 3, 8, 8);
        let ca = ra.render(&req).unwrap();
        let cb = rb.render(&req).unwrap();
        for (fa, fb) in ca.frames.iter().zip(cb.frames.iter()) {
            let ba = std::fs::read(fa).unwrap();
            let bb = std::fs::read(fb).unwrap();
            assert!(ba.starts_with(b"\x89PNG\r\n\x1a\n"));
            assert_eq!(ba, bb, "same request must produce identical bytes");
        }
    }

    #[test]
    fn stub_png_decodes_with_correct_dimensions_and_alpha() {
        // Validates the hand-rolled PNG encoder against a real decoder, and that
        // the transparent flag actually varies alpha across frames.
        let (renderer, _tmp) = stub_with_tmp();
        let req = MotionRenderRequest::new(MotionSource::code("<x/>"), 30, 3, 4, 2)
            .with_transparent(true);
        let clip = renderer.render(&req).unwrap();

        let first = image::open(&clip.frames[0]).unwrap().to_rgba8();
        assert_eq!(first.dimensions(), (4, 2));
        // frame 0 alpha == 0 (ramp start), last frame alpha == 255.
        assert_eq!(first.get_pixel(0, 0)[3], 0);
        let last = image::open(clip.frames.last().unwrap()).unwrap().to_rgba8();
        assert_eq!(last.get_pixel(0, 0)[3], 255);

        // 200x100 RGBA scanlines exceed one 65,535-byte stored-deflate block.
        // Decode a direct encoder result to prove multi-block zlib framing and
        // exact RGBA values, not just the tiny single-block fixture above.
        let rgba = [17, 34, 51, 68];
        let big_a = encode_solid_rgba_png(200, 100, rgba);
        let big_b = encode_solid_rgba_png(200, 100, rgba);
        assert_eq!(big_a, big_b);
        let big = image::load_from_memory_with_format(&big_a, image::ImageFormat::Png)
            .unwrap()
            .to_rgba8();
        assert_eq!(big.dimensions(), (200, 100));
        assert_eq!(big.get_pixel(0, 0).0, rgba);
        assert_eq!(big.get_pixel(199, 99).0, rgba);
    }

    #[test]
    fn stub_opaque_frames_are_fully_opaque() {
        let (renderer, _tmp) = stub_with_tmp();
        let req = MotionRenderRequest::new(MotionSource::code("<x/>"), 30, 2, 3, 3)
            .with_transparent(false);
        let clip = renderer.render(&req).unwrap();
        let img = image::open(&clip.frames[0]).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0)[3], 255);
    }

    #[test]
    fn chromium_skeleton_reports_unavailable_not_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let r =
            HeadlessChromiumRenderer::new(MotionCache::new(tmp.path()), SandboxPolicy::default());
        let req = MotionRenderRequest::new(MotionSource::code("<x/>"), 30, 2, 10, 10);
        #[cfg(not(feature = "chromium"))]
        {
            let err = r.render(&req).unwrap_err();
            assert!(
                matches!(err, MotionError::RendererUnavailable(_)),
                "expected RendererUnavailable, got {err:?}"
            );
        }
        #[cfg(feature = "chromium")]
        {
            if HeadlessChromiumRenderer::find_browser().is_some() {
                let clip = r.render(&req).expect("feature-enabled browser render");
                assert_eq!(clip.frame_count(), 2);
            } else {
                assert!(matches!(
                    r.render(&req),
                    Err(MotionError::RendererUnavailable(_))
                ));
            }
        }
    }

    #[test]
    fn chromium_applies_sandbox_size_before_unavailable() {
        // Stub checks the default ceiling before creating its content-hash dir.
        let stub_tmp = tempfile::tempdir().unwrap();
        let stub = StubRenderer::new(MotionCache::new(stub_tmp.path()));
        let oversized = "x".repeat(crate::sandbox::DEFAULT_MAX_DOCUMENT_BYTES + 1);
        let stub_req = MotionRenderRequest::new(MotionSource::code(oversized), 30, 1, 10, 10);
        assert!(matches!(
            stub.render(&stub_req),
            Err(MotionError::Sandbox(_))
        ));
        assert_eq!(std::fs::read_dir(stub_tmp.path()).unwrap().count(), 0);

        // Chromium checks its policy before browser discovery/launch and before
        // creating a content-hash dir, in both feature configurations.
        let tmp = tempfile::tempdir().unwrap();
        let policy = SandboxPolicy {
            max_document_bytes: 4,
            ..Default::default()
        };
        let r = HeadlessChromiumRenderer::new(MotionCache::new(tmp.path()), policy);
        let req =
            MotionRenderRequest::new(MotionSource::code("<this-is-too-long/>"), 30, 1, 10, 10);
        let err = r.render(&req).unwrap_err();
        assert!(matches!(err, MotionError::Sandbox(_)), "got {err:?}");
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn data_url_encodes_html() {
        let url = HeadlessChromiumRenderer::data_url_for_code("<b>a b</b>");
        assert!(url.starts_with("data:text/html;charset=utf-8,"));
        // space encoded, angle brackets encoded, alnum kept
        assert!(url.contains("%3Cb%3E")); // <b>
        assert!(url.contains("a%20b"));
    }

    #[test]
    fn frame_time_grid_is_correct() {
        let req = MotionRenderRequest::new(MotionSource::code("<x/>"), 10, 5, 8, 8);
        let grid = HeadlessChromiumRenderer::frame_time_grid(&req);
        assert_eq!(grid, vec![0.0, 0.1, 0.2, 0.3, 0.4]);
    }
}
