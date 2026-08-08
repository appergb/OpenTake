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
///    `i / fps` and call `OpenTake.seek(i / fps)`. Each black/white sample uses
///    a fresh paused PageHandler with a bounded compositor screencast; isolated
///    stable samples recover transparency before writing
///    `cache_dir/frame_iiiii.png`.
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

            trace(format!("frame {index}: compositor capture start"));
            let png = cdp.capture_frame_png(
                &target_id,
                &session,
                req.transparent,
                req.width,
                req.height,
                index,
            )?;
            check_abort(renderer, deadline)?;
            cdp.ensure_no_blocked_url()?;
            trace(format!("frame {index}: compositor captured"));
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
                device_metrics_params(width, height),
                Some(session),
            )?;
            Ok(())
        }

        fn start_screencast(&mut self, session: &str, width: u32, height: u32) -> MotionResult<()> {
            self.command(
                "Page.startScreencast",
                json!({
                    "format": "png",
                    "maxWidth": width,
                    "maxHeight": height,
                    "everyNthFrame": 1
                }),
                Some(session),
            )?;
            self.ensure_no_blocked_url()
        }

        fn stop_screencast(&mut self, session: &str) -> MotionResult<()> {
            let stopped = self.command("Page.stopScreencast", json!({}), Some(session));
            let drained = if stopped.is_ok() {
                self.ack_pending_screencast_frames(session)
            } else {
                Ok(())
            };

            stopped?;
            drained?;
            self.ensure_no_blocked_url()
        }

        fn capture_isolated_viewport(
            &mut self,
            target_id: &str,
            rgb: [u8; 3],
            width: u32,
            height: u32,
            frame_index: usize,
            background: &str,
        ) -> MotionResult<image::RgbaImage> {
            let attached = self.command(
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
                None,
            )?;
            let capture_session = required_string(&attached, "sessionId")?;
            let mut started = false;
            let captured = (|| -> MotionResult<image::RgbaImage> {
                self.command("Page.enable", json!({}), Some(&capture_session))?;
                self.command("Page.bringToFront", json!({}), Some(&capture_session))?;
                self.command(
                    "Emulation.setVirtualTimePolicy",
                    json!({"policy": "pause"}),
                    Some(&capture_session),
                )?;
                self.command(
                    "Emulation.setDefaultBackgroundColorOverride",
                    json!({
                        "color": {
                            "r": rgb[0],
                            "g": rgb[1],
                            "b": rgb[2],
                            "a": 1.0
                        }
                    }),
                    Some(&capture_session),
                )?;
                self.start_screencast(&capture_session, width, height)?;
                started = true;
                let png = self.receive_and_ack_screencast_png(&capture_session, frame_index)?;
                let image = decode_viewport_png(&png, background, frame_index)?;
                if image.dimensions() != (width, height) {
                    return Err(MotionError::render_failed(format!(
                        "Chromium isolated {background} screencast has the wrong size for frame {frame_index}: actual={:?}, expected=({width}, {height})",
                        image.dimensions()
                    )));
                }
                Ok(image)
            })();

            let stopped = if started {
                self.stop_screencast(&capture_session)
            } else {
                Ok(())
            };
            let detached = self
                .command(
                    "Target.detachFromTarget",
                    json!({"sessionId": capture_session}),
                    None,
                )
                .and_then(|_| self.ensure_no_blocked_url());
            self.pending_events.retain(|event| {
                event.get("method").and_then(Value::as_str) != Some("Page.screencastFrame")
            });

            match captured {
                Err(primary) => Err(primary),
                Ok(image) => {
                    stopped?;
                    detached?;
                    self.ensure_no_blocked_url()?;
                    Ok(image)
                }
            }
        }

        fn receive_and_ack_screencast_png(
            &mut self,
            session: &str,
            frame_index: usize,
        ) -> MotionResult<Vec<u8>> {
            let event = self.next_screencast_event(session)?;
            let params = event.get("params").ok_or_else(|| {
                MotionError::render_failed("Chromium screencast frame has no params")
            })?;
            let screencast_session_id = params
                .get("sessionId")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    MotionError::render_failed(format!(
                        "Chromium screencast frame has no integer sessionId: {event}"
                    ))
                })?;
            self.command(
                "Page.screencastFrameAck",
                json!({"sessionId": screencast_session_id}),
                Some(session),
            )?;
            self.ensure_no_blocked_url()?;
            let encoded = required_string(params, "data")?;
            let png = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|error| {
                    MotionError::render_failed(format!(
                        "Chromium returned malformed screencast data for frame {frame_index}: {error}"
                    ))
                })?;
            if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Err(MotionError::render_failed(format!(
                    "Chromium returned a non-PNG screencast frame for frame {frame_index}"
                )));
            }
            Ok(png)
        }

        fn next_screencast_event(&mut self, session: &str) -> MotionResult<Value> {
            if let Some(index) = self
                .pending_events
                .iter()
                .position(|event| is_screencast_event_for_session(event, session))
            {
                return Ok(self.pending_events.remove(index));
            }
            loop {
                let value = self.read()?;
                if is_screencast_event_for_session(&value, session) {
                    return Ok(value);
                }
                self.handle_event_or_queue(value)?;
            }
        }

        fn ack_pending_screencast_frames(&mut self, session: &str) -> MotionResult<()> {
            while let Some(index) = self
                .pending_events
                .iter()
                .position(|event| is_screencast_event_for_session(event, session))
            {
                let event = self.pending_events.remove(index);
                let screencast_session_id = event
                    .get("params")
                    .and_then(|params| params.get("sessionId"))
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        MotionError::render_failed(format!(
                            "Chromium screencast frame has no integer sessionId: {event}"
                        ))
                    })?;
                self.command(
                    "Page.screencastFrameAck",
                    json!({"sessionId": screencast_session_id}),
                    Some(session),
                )?;
            }
            Ok(())
        }

        fn capture_frame_png(
            &mut self,
            target_id: &str,
            session: &str,
            transparent: bool,
            width: u32,
            height: u32,
            frame_index: usize,
        ) -> MotionResult<Vec<u8>> {
            self.check_abort()?;
            if !transparent {
                self.set_capture_background(session, [0, 0, 0])?;
                let white = self.capture_stable_background(
                    target_id,
                    session,
                    [255, 255, 255],
                    "opaque-white",
                    (width, height),
                    frame_index,
                )?;
                self.check_abort()?;
                let normalized = encode_viewport_png(white, frame_index)?;
                self.check_abort()?;
                return Ok(normalized);
            }

            // Each background uses one prime handler plus two independent
            // PageHandlers whose exact images must agree. This is six compositor
            // encodes for transparency and bounds decoded working memory to the
            // two stable samples plus the recovered output.
            let black = self.capture_stable_background(
                target_id,
                session,
                [0, 0, 0],
                "black",
                (width, height),
                frame_index,
            )?;
            let white = self.capture_stable_background(
                target_id,
                session,
                [255, 255, 255],
                "white",
                (width, height),
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

        fn set_capture_background(&mut self, session: &str, rgb: [u8; 3]) -> MotionResult<()> {
            self.command(
                "Emulation.setDefaultBackgroundColorOverride",
                json!({
                    "color": {
                        "r": rgb[0],
                        "g": rgb[1],
                        "b": rgb[2],
                        "a": 1.0
                    }
                }),
                Some(session),
            )?;
            self.ensure_no_blocked_url()
        }

        fn capture_stable_background(
            &mut self,
            target_id: &str,
            session: &str,
            rgb: [u8; 3],
            background: &str,
            size: (u32, u32),
            frame_index: usize,
        ) -> MotionResult<image::RgbaImage> {
            let (width, height) = size;
            self.set_capture_background(session, rgb)?;
            trace(format!("frame {frame_index}: {background} prime start"));
            let prime = self.capture_isolated_viewport(
                target_id,
                rgb,
                width,
                height,
                frame_index,
                background,
            )?;
            drop(prime);
            let first = self.capture_isolated_viewport(
                target_id,
                rgb,
                width,
                height,
                frame_index,
                background,
            )?;
            let second = self.capture_isolated_viewport(
                target_id,
                rgb,
                width,
                height,
                frame_index,
                background,
            )?;
            if let Err(error) =
                ensure_stable_viewport_images(&first, &second, background, frame_index)
            {
                return Err(MotionError::render_failed(format!(
                    "{error}; sandbox_blocked_url_seen={}",
                    self.blocked_url.is_some()
                )));
            }
            drop(first);
            self.check_abort()?;
            Ok(second)
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

    fn is_screencast_event_for_session(event: &Value, session: &str) -> bool {
        event.get("method").and_then(Value::as_str) == Some("Page.screencastFrame")
            && event.get("sessionId").and_then(Value::as_str) == Some(session)
    }

    fn device_metrics_params(width: u32, height: u32) -> Value {
        json!({
            "width": width,
            "height": height,
            "deviceScaleFactor": 1,
            "mobile": false,
            "screenWidth": width,
            "screenHeight": height
        })
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
            let (width, height) = fence.dimensions();
            let mut differing_channels = 0usize;
            let mut sample_coordinates = Vec::with_capacity(8);
            for ((x, y, left), right) in fence.enumerate_pixels().zip(captured.pixels()) {
                let changed_channels = left
                    .0
                    .iter()
                    .zip(right.0.iter())
                    .filter(|(left, right)| left != right)
                    .count();
                differing_channels += changed_channels;
                if changed_channels > 0 && sample_coordinates.len() < 8 {
                    sample_coordinates.push((x, y));
                }
            }
            return Err(MotionError::render_failed(format!(
                "Chromium {background}-background did not reach a stable readback for frame {frame_index}: dimensions=({width}, {height}), differing_channels={differing_channels}, sample_coordinates={sample_coordinates:?}"
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
        fn device_metrics_keep_layout_and_capture_viewport_exact() {
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
                            "screenHeight": 32
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
        fn viewport_readback_uses_a_bounded_screencast_session() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            let mut server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            let png = base64::engine::general_purpose::STANDARD.encode(
                encode_viewport_png(
                    image::RgbaImage::from_pixel(48, 32, image::Rgba([1, 2, 3, 255])),
                    0,
                )
                .unwrap(),
            );
            let server = thread::spawn(move || {
                let start = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected text CDP request, got {other:?}"),
                };
                assert_eq!(
                    start,
                    json!({
                        "id": 1,
                        "method": "Page.startScreencast",
                        "params": {
                            "format": "png",
                            "maxWidth": 48,
                            "maxHeight": 32,
                            "everyNthFrame": 1
                        },
                        "sessionId": "render-session"
                    })
                );
                server_socket
                    .send(Message::text(json!({"id": 1, "result": {}}).to_string()))
                    .unwrap();
                server_socket
                    .send(Message::text(
                        json!({
                            "method": "Page.screencastFrame",
                            "params": {
                                "data": png,
                                "metadata": {},
                                "sessionId": 7
                            },
                            "sessionId": "render-session"
                        })
                        .to_string(),
                    ))
                    .unwrap();

                let ack = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected screencast ack, got {other:?}"),
                };
                assert_eq!(
                    ack,
                    json!({
                        "id": 2,
                        "method": "Page.screencastFrameAck",
                        "params": {"sessionId": 7},
                        "sessionId": "render-session"
                    })
                );
                server_socket
                    .send(Message::text(json!({"id": 2, "result": {}}).to_string()))
                    .unwrap();

                let stop = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected screencast stop, got {other:?}"),
                };
                assert_eq!(
                    stop,
                    json!({
                        "id": 3,
                        "method": "Page.stopScreencast",
                        "params": {},
                        "sessionId": "render-session"
                    })
                );
                server_socket
                    .send(Message::text(
                        json!({
                            "method": "Page.screencastFrame",
                            "params": {
                                "data": "late-frame",
                                "metadata": {},
                                "sessionId": 7
                            },
                            "sessionId": "render-session"
                        })
                        .to_string(),
                    ))
                    .unwrap();
                server_socket
                    .send(Message::text(json!({"id": 3, "result": {}}).to_string()))
                    .unwrap();

                let late_ack = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected late-frame ack, got {other:?}"),
                };
                assert_eq!(
                    late_ack,
                    json!({
                        "id": 4,
                        "method": "Page.screencastFrameAck",
                        "params": {"sessionId": 7},
                        "sessionId": "render-session"
                    })
                );
                server_socket
                    .send(Message::text(json!({"id": 4, "result": {}}).to_string()))
                    .unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            cdp.start_screencast("render-session", 48, 32).unwrap();
            let captured = cdp
                .receive_and_ack_screencast_png("render-session", 0)
                .unwrap();
            assert!(captured.starts_with(b"\x89PNG\r\n\x1a\n"));
            cdp.stop_screencast("render-session").unwrap();
            server.join().unwrap();
        }

        #[test]
        fn screencast_readback_acks_and_stops_when_the_frame_is_invalid() {
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
                let start = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected screencast start, got {other:?}"),
                };
                assert_eq!(start["method"], "Page.startScreencast");
                server_socket
                    .send(Message::text(json!({"id": 1, "result": {}}).to_string()))
                    .unwrap();
                server_socket
                    .send(Message::text(
                        json!({
                            "method": "Page.screencastFrame",
                            "params": {"metadata": {}, "sessionId": 7},
                            "sessionId": "render-session"
                        })
                        .to_string(),
                    ))
                    .unwrap();

                let ack = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected invalid-frame ack, got {other:?}"),
                };
                assert_eq!(ack["method"], "Page.screencastFrameAck");
                assert_eq!(ack["params"]["sessionId"], 7);
                server_socket
                    .send(Message::text(json!({"id": 2, "result": {}}).to_string()))
                    .unwrap();

                let stop = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected cleanup stop, got {other:?}"),
                };
                assert_eq!(stop["method"], "Page.stopScreencast");
                server_socket
                    .send(Message::text(json!({"id": 3, "result": {}}).to_string()))
                    .unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            cdp.start_screencast("render-session", 48, 32).unwrap();
            let captured = cdp.receive_and_ack_screencast_png("render-session", 0);
            let stopped = cdp.stop_screencast("render-session");
            assert!(matches!(
                captured,
                Err(MotionError::RenderFailed(message))
                    if message.contains("missing string field \"data\"")
            ));
            stopped.unwrap();
            server.join().unwrap();
        }

        #[test]
        fn screencast_ack_fails_closed_on_a_late_blocked_request() {
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
                let start = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected screencast start, got {other:?}"),
                };
                assert_eq!(start["method"], "Page.startScreencast");
                server_socket
                    .send(Message::text(json!({"id": 1, "result": {}}).to_string()))
                    .unwrap();
                server_socket
                    .send(Message::text(
                        json!({
                            "method": "Page.screencastFrame",
                            "params": {"data": "ignored", "metadata": {}, "sessionId": 7},
                            "sessionId": "render-session"
                        })
                        .to_string(),
                    ))
                    .unwrap();

                let ack = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected screencast ack, got {other:?}"),
                };
                assert_eq!(ack["method"], "Page.screencastFrameAck");
                server_socket
                    .send(Message::text(
                        json!({
                            "method": "Fetch.requestPaused",
                            "params": {
                                "requestId": "late-request",
                                "request": {"url": "https://example.com/late-screencast"}
                            },
                            "sessionId": "render-session"
                        })
                        .to_string(),
                    ))
                    .unwrap();
                server_socket
                    .send(Message::text(json!({"id": 2, "result": {}}).to_string()))
                    .unwrap();

                let failed = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected late request rejection, got {other:?}"),
                };
                assert_eq!(failed["id"], 3);
                assert_eq!(failed["method"], "Fetch.failRequest");
                server_socket
                    .send(Message::text(json!({"id": 3, "result": {}}).to_string()))
                    .unwrap();

                let stop = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected screencast stop, got {other:?}"),
                };
                assert_eq!(stop["id"], 4);
                assert_eq!(stop["method"], "Page.stopScreencast");
                server_socket
                    .send(Message::text(json!({"id": 4, "result": {}}).to_string()))
                    .unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            cdp.start_screencast("render-session", 48, 32).unwrap();
            let captured = cdp.receive_and_ack_screencast_png("render-session", 0);
            let stopped = cdp.stop_screencast("render-session");
            assert!(matches!(captured, Err(MotionError::Sandbox(_))));
            stopped.unwrap();
            server.join().unwrap();
        }

        #[test]
        fn screencast_readback_filters_the_outer_target_session() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            let mut server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            let current_png = base64::engine::general_purpose::STANDARD.encode(
                encode_viewport_png(
                    image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255])),
                    0,
                )
                .unwrap(),
            );
            let server = thread::spawn(move || {
                let start = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected screencast start, got {other:?}"),
                };
                assert_eq!(start["method"], "Page.startScreencast");
                server_socket
                    .send(Message::text(json!({"id": 1, "result": {}}).to_string()))
                    .unwrap();
                for (outer_session, inner_session, data) in [
                    ("other-target", 99, "wrong-target".to_owned()),
                    ("render-session", 7, current_png),
                ] {
                    server_socket
                        .send(Message::text(
                            json!({
                                "method": "Page.screencastFrame",
                                "params": {
                                    "data": data,
                                    "metadata": {},
                                    "sessionId": inner_session
                                },
                                "sessionId": outer_session
                            })
                            .to_string(),
                        ))
                        .unwrap();
                }

                let current_ack = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected current-frame ack, got {other:?}"),
                };
                assert_eq!(current_ack["method"], "Page.screencastFrameAck");
                assert_eq!(current_ack["params"]["sessionId"], 7);
                server_socket
                    .send(Message::text(json!({"id": 2, "result": {}}).to_string()))
                    .unwrap();
                let stop = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected screencast stop, got {other:?}"),
                };
                assert_eq!(stop["method"], "Page.stopScreencast");
                server_socket
                    .send(Message::text(json!({"id": 3, "result": {}}).to_string()))
                    .unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            cdp.start_screencast("render-session", 48, 32).unwrap();
            let captured = cdp
                .receive_and_ack_screencast_png("render-session", 0)
                .unwrap();
            assert!(captured.starts_with(b"\x89PNG\r\n\x1a\n"));
            cdp.stop_screencast("render-session").unwrap();
            assert_eq!(cdp.pending_events.len(), 1);
            assert_eq!(cdp.pending_events[0]["sessionId"], "other-target");
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
        fn unstable_high_entropy_viewport_reports_bounded_coordinates_without_pixel_values() {
            let mut first = image::RgbaImage::new(64, 64);
            let mut second = image::RgbaImage::new(64, 64);
            for y in 0..64 {
                for x in 0..64 {
                    first.put_pixel(x, y, image::Rgba([x as u8, y as u8, (x + y) as u8, 255]));
                    second.put_pixel(
                        x,
                        y,
                        image::Rgba([(x + 101) as u8, (y + 109) as u8, (x + y + 127) as u8, 254]),
                    );
                }
            }

            let error = ensure_stable_viewport_images(&first, &second, "black", 17)
                .expect_err("different high-entropy images must fail closed");
            let MotionError::RenderFailed(message) = error else {
                panic!("expected render failure, got {error:?}");
            };
            assert!(message.len() <= 512, "diagnostic must remain bounded");
            assert!(message.contains("dimensions=(64, 64)"));
            assert!(message.contains("differing_channels=16384"));
            assert!(message.contains(
                "sample_coordinates=[(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0)]"
            ));
            assert!(!message.contains("(8, 0)"));
            assert!(!message.contains("[0, 0, 0, 255]"));
            assert!(!message.contains("[101, 109, 127, 254]"));
            assert!(!message.contains("unique"));
            assert!(!message.contains("corner"));
        }

        #[test]
        fn transparent_capture_uses_six_isolated_page_handlers() {
            fn read_json(socket: &mut WebSocket<TcpStream>) -> Value {
                match socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str(text.as_ref()).unwrap(),
                    other => panic!("expected CDP command, got {other:?}"),
                }
            }

            fn send_json(socket: &mut WebSocket<TcpStream>, value: Value) {
                socket.send(Message::text(value.to_string())).unwrap();
            }

            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            let mut server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            let encoded = |pixel: [u8; 4]| {
                base64::engine::general_purpose::STANDARD.encode(
                    encode_viewport_png(image::RgbaImage::from_pixel(1, 1, image::Rgba(pixel)), 0)
                        .unwrap(),
                )
            };
            let black = encoded([128, 0, 0, 255]);
            let white = encoded([255, 127, 127, 255]);
            let server = thread::spawn(move || {
                let mut next_id = 1u64;
                let mut capture_index = 0usize;
                for (rgb, current, stale_prime) in [
                    ([0, 0, 0], black.clone(), white.clone()),
                    ([255, 255, 255], white, black),
                ] {
                    let background = read_json(&mut server_socket);
                    assert_eq!(
                        background,
                        json!({
                            "id": next_id,
                            "method": "Emulation.setDefaultBackgroundColorOverride",
                            "params": {
                                "color": {"r": rgb[0], "g": rgb[1], "b": rgb[2], "a": 1.0}
                            },
                            "sessionId": "main-session"
                        })
                    );
                    send_json(&mut server_socket, json!({"id": next_id, "result": {}}));
                    next_id += 1;

                    let mut previous_capture_session = None::<String>;
                    for sample_index in 0..3 {
                        let capture_session = format!("capture-{capture_index}");
                        capture_index += 1;
                        let attach = read_json(&mut server_socket);
                        assert_eq!(
                            attach,
                            json!({
                                "id": next_id,
                                "method": "Target.attachToTarget",
                                "params": {"targetId": "target-id", "flatten": true}
                            })
                        );
                        send_json(
                            &mut server_socket,
                            json!({"id": next_id, "result": {"sessionId": capture_session}}),
                        );
                        next_id += 1;

                        for (method, params) in [
                            ("Page.enable", json!({})),
                            ("Page.bringToFront", json!({})),
                            ("Emulation.setVirtualTimePolicy", json!({"policy": "pause"})),
                            (
                                "Emulation.setDefaultBackgroundColorOverride",
                                json!({
                                    "color": {
                                        "r": rgb[0],
                                        "g": rgb[1],
                                        "b": rgb[2],
                                        "a": 1.0
                                    }
                                }),
                            ),
                            (
                                "Page.startScreencast",
                                json!({
                                    "format": "png",
                                    "maxWidth": 1,
                                    "maxHeight": 1,
                                    "everyNthFrame": 1
                                }),
                            ),
                        ] {
                            let command = read_json(&mut server_socket);
                            assert_eq!(
                                command,
                                json!({
                                    "id": next_id,
                                    "method": method,
                                    "params": params,
                                    "sessionId": capture_session
                                })
                            );
                            send_json(&mut server_socket, json!({"id": next_id, "result": {}}));
                            next_id += 1;
                        }

                        if sample_index == 2 {
                            let prior = previous_capture_session
                                .as_deref()
                                .expect("the stable sample has a prior PageHandler");
                            send_json(
                                &mut server_socket,
                                json!({
                                    "method": "Page.screencastFrame",
                                    "params": {"data": current, "metadata": {}, "sessionId": 7},
                                    "sessionId": prior
                                }),
                            );
                        }
                        let data = if sample_index == 0 {
                            stale_prime.clone()
                        } else {
                            current.clone()
                        };
                        send_json(
                            &mut server_socket,
                            json!({
                                "method": "Page.screencastFrame",
                                "params": {"data": data, "metadata": {}, "sessionId": 7},
                                "sessionId": capture_session
                            }),
                        );

                        let ack = read_json(&mut server_socket);
                        assert_eq!(
                            ack,
                            json!({
                                "id": next_id,
                                "method": "Page.screencastFrameAck",
                                "params": {"sessionId": 7},
                                "sessionId": capture_session
                            })
                        );
                        send_json(&mut server_socket, json!({"id": next_id, "result": {}}));
                        next_id += 1;

                        let stop = read_json(&mut server_socket);
                        assert_eq!(
                            stop,
                            json!({
                                "id": next_id,
                                "method": "Page.stopScreencast",
                                "params": {},
                                "sessionId": capture_session
                            })
                        );
                        send_json(&mut server_socket, json!({"id": next_id, "result": {}}));
                        next_id += 1;

                        let detach = read_json(&mut server_socket);
                        assert_eq!(
                            detach,
                            json!({
                                "id": next_id,
                                "method": "Target.detachFromTarget",
                                "params": {"sessionId": capture_session}
                            })
                        );
                        send_json(&mut server_socket, json!({"id": next_id, "result": {}}));
                        next_id += 1;
                        previous_capture_session = Some(capture_session);
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
                .capture_frame_png("target-id", "main-session", true, 1, 1, 0)
                .unwrap();
            assert_eq!(
                image::load_from_memory(&png)
                    .unwrap()
                    .to_rgba8()
                    .get_pixel(0, 0)
                    .0,
                [255, 0, 0, 128]
            );
            assert!(
                cdp.pending_events
                    .iter()
                    .all(|event| event["method"] != "Page.screencastFrame"),
                "late frames owned by a detached PageHandler must not leak into another capture"
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
