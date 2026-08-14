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
  function pinAnimations(seconds) {
    if (!document.getAnimations) return;
    var animations = document.getAnimations();
    for (var i = 0; i < animations.length; i++) {
      try {
        animations[i].pause();
        animations[i].currentTime = seconds * 1000;
      } catch (e) { /* a detached animation may disappear while seeking */ }
    }
  }
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
      pinAnimations(seconds);
      var pending = [];
      for (var i = 0; i < listeners.length; i++) {
        try { pending.push(Promise.resolve(listeners[i](seconds))); } catch (e) {}
      }
      await Promise.all(pending);
      // A seek listener may create a CSS/Web Animation. Freeze those at the
      // same exact playhead before the compositor is allowed to paint.
      pinAnimations(seconds);
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
/// 2. Create an engine-owned `(width + 1)`×`(height + 1)` host document. Author
///    markup runs in an exact-size `<iframe sandbox="allow-scripts">`; the
///    external row/column is a compositor-generation guard.
/// 3. Auto-attach the sandboxed OOPIF paused, install
///    [`deterministic_clock_script`] plus Fetch/Log enforcement in its own CDP
///    session, then resume it. This guarantees the clock and network policy are
///    active before author code runs.
/// 4. `Emulation.setVirtualTimePolicy { policy: "pause" }` in the author session
///    to stop real time.
/// 5. For each frame `i`, call `OpenTake.seek(i / fps)` in the author context.
///    Every black/white readback uses three fresh paused main-target
///    PageHandlers. An engine-owned marker in the author OOPIF moves across
///    three pixels; each root readback must contain both that author generation
///    and a unique host guard. Every output pixel is reconstructed from at
///    least two unmarked, byte-identical samples.
/// 6. Return the [`RenderedClip`].
///
/// Compatibility boundary: author code is intentionally not top-level
/// (`window.top != window`), has an opaque `null` origin, and cannot read or
/// navigate the host DOM. Motion code that requires parent/same-origin access is
/// unsupported; declared external resources continue to use the exact
/// [`SandboxPolicy`] allowlist.
///
/// The CDP wiring is gated behind the `chromium` cargo feature so the default
/// build does not require a browser or websocket dependency. The live path
/// locates Chrome/Chromium/Edge and reuses one renderer-owned browser profile
/// and process across successful renders. Each render gets a fresh root CDP
/// connection, disposable browser context, and target. The backend injects a
/// strict CSP, intercepts every request with `Fetch`, and kills the browser on
/// cancellation, timeout, or protocol failure. Without the feature, [`render`]
/// returns [`MotionError::RendererUnavailable`].
#[derive(Clone, Debug)]
pub struct HeadlessChromiumRenderer {
    cache: MotionCache,
    policy: SandboxPolicy,
    browser_path: Option<PathBuf>,
    cancellation: MotionCancellationToken,
    #[cfg(feature = "chromium")]
    browser_pool: Arc<chromium_backend::BrowserPool>,
}

impl HeadlessChromiumRenderer {
    /// Build the renderer with a cache and sandbox policy.
    pub fn new(cache: MotionCache, policy: SandboxPolicy) -> Self {
        HeadlessChromiumRenderer {
            cache,
            policy,
            browser_path: None,
            cancellation: MotionCancellationToken::new(),
            #[cfg(feature = "chromium")]
            browser_pool: Arc::new(chromium_backend::BrowserPool::new()),
        }
    }

    /// Override browser discovery. Useful for portable app bundles and for
    /// deterministic crash-path tests.
    pub fn with_browser_path(mut self, path: impl Into<PathBuf>) -> Self {
        #[cfg(feature = "chromium")]
        {
            self.browser_pool = Arc::new(chromium_backend::BrowserPool::new());
        }
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

    /// The plan of per-frame virtual-time stamps the backend will seek through,
    /// beginning at `start_frame / fps`. Pure helper that documents and tests
    /// the time grid without launching anything.
    pub fn frame_time_grid(req: &MotionRenderRequest) -> Vec<f64> {
        (0..req.duration_frames)
            .map(|i| (req.start_frame + i) as f64 / req.fps as f64)
            .collect()
    }

    /// Render with a token scoped to this call while retaining the renderer's
    /// reusable Chromium process for later successful calls.
    pub fn render_with_cancellation(
        &self,
        req: &MotionRenderRequest,
        cancellation: &MotionCancellationToken,
    ) -> MotionResult<RenderedClip> {
        let validated = (|| {
            req.validate()?;
            if let MotionSource::Code { html_css_js } = &req.source {
                self.policy.check_document_size(html_css_js)?;
            }
            Ok(())
        })();
        if let Err(error) = validated {
            #[cfg(feature = "chromium")]
            self.browser_pool.invalidate_idle();
            return Err(error);
        }

        #[cfg(feature = "chromium")]
        {
            chromium_backend::render(self, req, cancellation)
        }
        #[cfg(not(feature = "chromium"))]
        {
            let _ = (&self.cache, cancellation);
            Err(MotionError::renderer_unavailable(
                "headless-Chromium backend is not compiled in; build with the \
                 `chromium` feature, or use StubRenderer for offline/deterministic rendering",
            ))
        }
    }
}

impl MotionRenderer for HeadlessChromiumRenderer {
    fn render(&self, req: &MotionRenderRequest) -> MotionResult<RenderedClip> {
        self.render_with_cancellation(req, &self.cancellation)
    }
}

#[cfg(feature = "chromium")]
mod chromium_backend {
    use std::collections::hash_map::RandomState;
    use std::hash::BuildHasher;
    use std::io::{BufRead, BufReader, Cursor};
    use std::net::TcpStream;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{mpsc, Mutex, MutexGuard, TryLockError};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use base64::Engine as _;
    use opentake_process_tree::{configure_command, ProcessTree};
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::{Message, WebSocket};

    use super::*;

    static PROFILE_COUNTER: AtomicU64 = AtomicU64::new(0);
    static AUTHOR_FENCE_COUNTER: AtomicU64 = AtomicU64::new(0);
    const GPU_TRACE_FIELD_LIMIT: usize = 96;
    const GPU_TRACE_STATUS_LIMIT: usize = 48;

    fn trace_enabled() -> bool {
        std::env::var_os("OPENTAKE_MOTION_TRACE").is_some()
    }

    fn trace(message: impl AsRef<str>) {
        if trace_enabled() {
            eprintln!("[opentake-motion] {}", message.as_ref());
        }
    }

    fn browser_launch_args() -> &'static [&'static str] {
        &[
            "--headless=new",
            "--remote-debugging-port=0",
            "--remote-debugging-address=127.0.0.1",
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
        ]
    }

    fn drain_browser_stderr<R: BufRead>(reader: R, sender: mpsc::Sender<String>) -> usize {
        let mut drained = 0usize;
        for line in reader.lines() {
            let Ok(line) = line else {
                break;
            };
            drained += 1;
            // Endpoint discovery drops its receiver as soon as launch returns.
            // Keep consuming the inherited Chrome/GPU/Viz pipe after that:
            // stopping here can fill the Windows pipe and block compositor
            // threads while unrelated CDP commands continue to respond.
            let _ = sender.send(line);
        }
        drained
    }

    pub(super) struct BrowserPool {
        slot: Mutex<Option<LiveBrowser>>,
        invalidation_pending: AtomicBool,
    }

    impl BrowserPool {
        pub(super) fn new() -> Self {
            Self {
                slot: Mutex::new(None),
                invalidation_pending: AtomicBool::new(false),
            }
        }

        fn acquire<'a>(
            &'a self,
            executable: &Path,
            deadline: Instant,
            timeout: Duration,
            cancellation: &MotionCancellationToken,
        ) -> MotionResult<BrowserLease<'a>> {
            let mut slot = loop {
                match self.slot.try_lock() {
                    Ok(slot) => break slot,
                    Err(TryLockError::WouldBlock) => {
                        check_abort_state(cancellation, deadline, timeout)?;
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(TryLockError::Poisoned(error)) => {
                        drop(error.into_inner().take());
                        self.slot.clear_poison();
                        return Err(MotionError::render_failed(
                            "Chromium browser pool lock was poisoned",
                        ));
                    }
                }
            };

            let observed_invalidation = self.invalidation_pending.swap(false, Ordering::AcqRel);
            let invalidated_browser = if observed_invalidation {
                slot.take()
            } else {
                None
            };
            drop(invalidated_browser);

            if slot
                .as_ref()
                .is_some_and(|browser| browser.executable != executable)
            {
                drop(slot.take());
            }
            if let Some(browser) = slot.as_mut() {
                if let Some(status) = browser.process.try_wait()? {
                    drop(slot.take());
                    return Err(MotionError::render_failed(format!(
                        "reusable Chromium exited before the next render: {status}"
                    )));
                }
                trace("reusing live Chromium process");
            } else {
                let (process, websocket_url) =
                    BrowserProcess::launch(executable, deadline, timeout, cancellation)?;
                *slot = Some(LiveBrowser {
                    process,
                    websocket_url,
                    executable: executable.to_path_buf(),
                });
                trace("browser launched and CDP endpoint is ready");
            }

            Ok(BrowserLease {
                pool: self,
                slot: Some(slot),
                reusable: false,
                observed_invalidation,
            })
        }

        pub(super) fn invalidate_idle(&self) {
            self.invalidation_pending.store(true, Ordering::Release);
            self.drain_pending_invalidation();
        }

        fn drain_pending_invalidation(&self) {
            if !self.invalidation_pending.load(Ordering::Acquire) {
                return;
            }

            let mut slot = match self.slot.try_lock() {
                Ok(slot) => slot,
                Err(TryLockError::WouldBlock) => return,
                Err(TryLockError::Poisoned(error)) => {
                    let slot = error.into_inner();
                    self.slot.clear_poison();
                    slot
                }
            };
            let browser = if self.invalidation_pending.swap(false, Ordering::AcqRel) {
                slot.take()
            } else {
                None
            };
            drop(slot);
            drop(browser);
        }
    }

    impl std::fmt::Debug for BrowserPool {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("BrowserPool")
                .finish_non_exhaustive()
        }
    }

    struct LiveBrowser {
        process: BrowserProcess,
        websocket_url: String,
        executable: PathBuf,
    }

    struct BrowserLease<'a> {
        pool: &'a BrowserPool,
        slot: Option<MutexGuard<'a, Option<LiveBrowser>>>,
        reusable: bool,
        observed_invalidation: bool,
    }

    impl BrowserLease<'_> {
        fn websocket_url(&self) -> MotionResult<&str> {
            self.slot
                .as_ref()
                .and_then(|slot| slot.as_ref())
                .map(|browser| browser.websocket_url.as_str())
                .ok_or_else(|| MotionError::render_failed("Chromium browser lease was empty"))
        }

        fn commit_reuse(&mut self) {
            if self.observed_invalidation
                || self.pool.invalidation_pending.swap(false, Ordering::AcqRel)
            {
                let browser = self.slot.as_mut().and_then(|slot| slot.take());
                drop(browser);
                return;
            }
            self.reusable = true;
        }
    }

    impl Drop for BrowserLease<'_> {
        fn drop(&mut self) {
            let pending = self.pool.invalidation_pending.swap(false, Ordering::AcqRel);
            let mut slot = self
                .slot
                .take()
                .expect("Chromium browser lease guard is present until drop");
            let browser = (!self.reusable || pending).then(|| slot.take()).flatten();
            drop(slot);
            drop(browser);

            // Covers invalidation set after the final in-lock check but before
            // the guard was released. A later acquirer also checks the pending
            // bit before it can reuse the retained browser.
            self.pool.drain_pending_invalidation();
        }
    }

    pub(super) fn render(
        renderer: &HeadlessChromiumRenderer,
        req: &MotionRenderRequest,
        cancellation: &MotionCancellationToken,
    ) -> MotionResult<RenderedClip> {
        let result = render_inner(renderer, req, cancellation);
        if result.is_err() {
            renderer.browser_pool.invalidate_idle();
        }
        result
    }

    fn render_inner(
        renderer: &HeadlessChromiumRenderer,
        req: &MotionRenderRequest,
        cancellation: &MotionCancellationToken,
    ) -> MotionResult<RenderedClip> {
        if cancellation.is_cancelled() {
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

        let author_document = match &req.source {
            MotionSource::Code { html_css_js } => sandboxed_document(html_css_js, &renderer.policy),
            MotionSource::Template { id, .. } => {
                return Err(MotionError::unknown_template(format!(
                    "{id} (HeadlessChromiumRenderer requires the caller to resolve templates to inline Code)"
                )));
            }
        };
        let guarded_width = req
            .width
            .checked_add(1)
            .ok_or_else(|| MotionError::render_failed("Chromium host guard width overflowed"))?;
        let guarded_height = req
            .height
            .checked_add(1)
            .ok_or_else(|| MotionError::render_failed("Chromium host guard height overflowed"))?;
        let document =
            host_wrapper_document(&author_document, req.width, req.height, &renderer.policy);

        let hash = content_hash(req);
        if renderer.cache.is_cached(req) {
            return Ok(clip_from_cache(req, hash, renderer.cache.dir_for(req)));
        }

        let deadline = Instant::now()
            .checked_add(renderer.policy.timeout)
            .unwrap_or_else(Instant::now);
        check_abort(cancellation, deadline, renderer.policy.timeout)?;
        let mut browser = renderer.browser_pool.acquire(
            &browser_path,
            deadline,
            renderer.policy.timeout,
            cancellation,
        )?;
        check_abort(cancellation, deadline, renderer.policy.timeout)?;
        if renderer.cache.is_cached(req) {
            browser.commit_reuse();
            return Ok(clip_from_cache(req, hash, renderer.cache.dir_for(req)));
        }

        let dir = renderer.cache.begin_render(req)?;
        remove_partial_frames(&dir)?;
        let mut partial = PartialFrames::new(dir.clone());
        let websocket_url = browser.websocket_url()?.to_owned();
        let (socket, _) = tungstenite::connect(websocket_url.as_str()).map_err(|error| {
            MotionError::render_failed(format!("failed to connect to Chromium CDP: {error}"))
        })?;
        trace("connected to browser CDP");
        set_socket_poll_timeout(&socket)?;
        let mut cdp = Cdp::new(
            socket,
            renderer.policy.clone(),
            cancellation.clone(),
            deadline,
        );
        trace_gpu_backend_if_enabled(&mut cdp, trace_enabled())?;

        let browser_context_id = cdp.create_browser_context()?;
        let target = cdp.command(
            "Target.createTarget",
            json!({
                "url": "about:blank",
                "background": false,
                "browserContextId": browser_context_id
            }),
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
        cdp.set_device_metrics(&session, guarded_width, guarded_height)?;
        let alpha = if req.transparent { 0.0 } else { 1.0 };
        cdp.command(
            "Emulation.setDefaultBackgroundColorOverride",
            json!({"color": {"r": 255, "g": 255, "b": 255, "a": alpha}}),
            Some(&session),
        )?;
        let installed = install_host_document(&mut cdp, &session, &document)?;
        trace("inline motion document loaded");
        cdp.ensure_no_blocked_url()?;
        let InstalledHost {
            author_session_id: author_session,
            author_context_id,
            author_fence,
        } = installed;
        let contract = cdp.command(
            "Runtime.evaluate",
            json!({
                "expression": r#"(() => {
                  let parentAccessError = '';
                  try { void window.parent.document.documentElement; }
                  catch (error) { parentAccessError = error && error.name || ''; }
                  return {
                    clockInstalled: !!(window.OpenTake && window.OpenTake.__installed),
                    isChild: window.top !== window,
                    parentAccessError,
                    origin: location.origin,
                    innerWidth: window.innerWidth,
                    innerHeight: window.innerHeight,
                    visualWidth: window.visualViewport && window.visualViewport.width,
                    visualHeight: window.visualViewport && window.visualViewport.height
                  };
                })()"#,
                "contextId": author_context_id,
                "returnByValue": true
            }),
            Some(&author_session),
        )?;
        let contract_value = contract
            .get("result")
            .and_then(|result| result.get("value"))
            .ok_or_else(|| {
                MotionError::render_failed(
                    "Chromium author frame did not return its sandbox contract",
                )
            })?;
        let exact_author_context = contract_value
            .get("clockInstalled")
            .and_then(Value::as_bool)
            == Some(true)
            && contract_value.get("isChild").and_then(Value::as_bool) == Some(true)
            && contract_value
                .get("parentAccessError")
                .and_then(Value::as_str)
                == Some("SecurityError")
            && contract_value.get("origin").and_then(Value::as_str) == Some("null")
            && contract_value.get("innerWidth").and_then(Value::as_u64)
                == Some(u64::from(req.width))
            && contract_value.get("innerHeight").and_then(Value::as_u64)
                == Some(u64::from(req.height))
            && contract_value.get("visualWidth").and_then(Value::as_f64)
                == Some(f64::from(req.width))
            && contract_value.get("visualHeight").and_then(Value::as_f64)
                == Some(f64::from(req.height));
        if !exact_author_context {
            return Err(MotionError::render_failed(
                "Chromium author frame violated the isolated viewport contract",
            ));
        }
        // Pausing before installing the host document also pauses its load
        // lifecycle in recent Chromium. The deterministic clock is installed
        // before the author child is created; freeze the browser's own timeline
        // immediately after that child has loaded and before any frame capture.
        cdp.command(
            "Emulation.setVirtualTimePolicy",
            json!({"policy": "pause"}),
            Some(&author_session),
        )?;
        cdp.ensure_no_blocked_url()?;

        let mut frames = Vec::with_capacity(req.duration_frames as usize);
        for (index, seconds) in HeadlessChromiumRenderer::frame_time_grid(req)
            .into_iter()
            .enumerate()
        {
            check_abort(cancellation, deadline, renderer.policy.timeout)?;
            trace(format!("frame {index}: seek start at {seconds:.17}s"));
            let expression = format!(
                "(async () => {{ if (!window.OpenTake) throw new Error('OpenTake clock missing'); await window.OpenTake.seek({seconds:.17}); return window.OpenTake.currentTime(); }})()"
            );
            let evaluated = cdp.command(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                    "contextId": author_context_id
                }),
                Some(&author_session),
            )?;
            trace(format!("frame {index}: seek complete"));
            if evaluated
                .get("exceptionDetails")
                .and_then(Value::as_object)
                .is_some()
            {
                return Err(MotionError::render_failed(format!(
                    "author document failed while seeking frame {index}"
                )));
            }
            cdp.ensure_no_blocked_url()?;

            trace(format!("frame {index}: compositor settle start"));
            cdp.settle_compositor(&author_session)?;
            trace(format!("frame {index}: compositor settle complete"));
            cdp.ensure_no_blocked_url()?;

            trace(format!("frame {index}: compositor capture start"));
            let png = cdp.capture_frame_png(
                &target_id,
                &session,
                &author_fence,
                req.transparent,
                CaptureFrame {
                    width: req.width,
                    height: req.height,
                    index,
                },
            )?;
            check_abort(cancellation, deadline, renderer.policy.timeout)?;
            cdp.ensure_no_blocked_url()?;
            trace(format!("frame {index}: compositor captured"));
            let path = MotionCache::frame_file(&dir, index);
            std::fs::write(&path, png)?;
            frames.push(path);
        }

        cdp.close_target(&target_id)?;
        cdp.dispose_browser_context(&browser_context_id)?;
        check_abort(cancellation, deadline, renderer.policy.timeout)?;
        drop(cdp);
        publish_completed_render(
            cancellation,
            renderer.policy.timeout,
            deadline,
            &dir,
            &mut partial,
        )?;
        browser.commit_reuse();

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
        let csp = sandbox_csp(policy, "'none'");
        format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\"><style>html,body{{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}}</style>{document}"
        )
    }

    fn sandbox_csp(policy: &SandboxPolicy, frame_src: &str) -> String {
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
        format!(
            "default-src 'none'; script-src 'unsafe-inline' data: {sources}; style-src 'unsafe-inline' data: {sources}; img-src data: {sources}; media-src data: {sources}; font-src data: {sources}; connect-src {sources}; object-src 'none'; base-uri 'none'; form-action 'none'; frame-src {frame_src}; worker-src 'none'"
        )
    }

    fn host_wrapper_document(
        author_document: &str,
        width: u32,
        height: u32,
        policy: &SandboxPolicy,
    ) -> String {
        let author_url = format!(
            "data:text/html;charset=utf-8,{}",
            percent_encode_html(author_document)
        );
        let csp = sandbox_csp(policy, "data:");
        format!(
            "<!doctype html><html><head><meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\"><style>html,body{{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}}#opentake-host-background{{position:fixed;inset:0;z-index:0;background:transparent}}iframe{{position:absolute;left:0;top:0;z-index:1;width:{width}px;height:{height}px;border:0;display:block;background:transparent}}</style></head><body><div id=\"opentake-host-background\" aria-hidden=\"true\"></div><iframe sandbox=\"allow-scripts\" referrerpolicy=\"no-referrer\" src=\"{author_url}\"></iframe></body></html>"
        )
    }

    struct InstalledHost {
        author_session_id: String,
        author_context_id: u64,
        author_fence: AuthorPaintFence,
    }

    #[derive(Clone, Debug)]
    struct AuthorPaintFence {
        session_id: String,
        context_id: u64,
        nonce: String,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct AuthorMarker {
        x: u32,
        y: u32,
        rgb: [u8; 3],
    }

    #[derive(Clone, Copy, Debug)]
    struct CaptureFrame {
        width: u32,
        height: u32,
        index: usize,
    }

    #[derive(Clone, Copy, Debug)]
    struct CapturePass<'a> {
        frame: CaptureFrame,
        background: &'a str,
    }

    fn install_host_document(
        cdp: &mut Cdp,
        session: &str,
        document: &str,
    ) -> MotionResult<InstalledHost> {
        cdp.command(
            "Target.setAutoAttach",
            json!({
                "autoAttach": true,
                "waitForDebuggerOnStart": true,
                "flatten": true,
                "filter": [
                    {"type": "iframe", "exclude": false},
                    {"exclude": true}
                ]
            }),
            Some(session),
        )?;
        let initial_tree = cdp.command("Page.getFrameTree", json!({}), Some(session))?;
        let main_frame_id = initial_tree
            .get("frameTree")
            .and_then(|tree| tree.get("frame"))
            .and_then(|frame| frame.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                MotionError::render_failed("Chromium about:blank frame tree has no root frame id")
            })?;
        cdp.command(
            "Page.setDocumentContent",
            json!({"frameId": main_frame_id, "html": document}),
            Some(session),
        )?;

        let attached =
            cdp.wait_for_event_matching("Target.attachedToTarget", Some(session), |event| {
                let params = event.get("params");
                params
                    .and_then(|params| params.get("waitingForDebugger"))
                    .and_then(Value::as_bool)
                    == Some(true)
                    && params
                        .and_then(|params| params.get("targetInfo"))
                        .and_then(|target| target.get("type"))
                        .and_then(Value::as_str)
                        == Some("iframe")
            })?;
        let author_session_id = attached
            .get("params")
            .and_then(|params| params.get("sessionId"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                MotionError::render_failed("Chromium author iframe attach has no child session")
            })?;
        let author_frame_id = attached
            .get("params")
            .and_then(|params| params.get("targetInfo"))
            .and_then(|target| target.get("targetId"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                MotionError::render_failed("Chromium author iframe attach has no target id")
            })?;
        for (method, params) in [
            ("Runtime.enable", json!({})),
            ("Page.enable", json!({})),
            ("Log.enable", json!({})),
            (
                "Fetch.enable",
                json!({"patterns": [{"urlPattern": "*", "requestStage": "Request"}]}),
            ),
            (
                "Page.addScriptToEvaluateOnNewDocument",
                json!({"source": deterministic_clock_script()}),
            ),
        ] {
            cdp.command(method, params, Some(&author_session_id))?;
        }
        cdp.command(
            "Runtime.runIfWaitingForDebugger",
            json!({}),
            Some(&author_session_id),
        )?;

        let navigated = cdp.wait_for_author_event(
            "Page.frameNavigated",
            &author_session_id,
            &author_frame_id,
            |event| {
                event
                    .get("params")
                    .and_then(|params| params.get("frame"))
                    .and_then(|frame| frame.get("id"))
                    .and_then(Value::as_str)
                    == Some(author_frame_id.as_str())
            },
        )?;
        validate_author_navigation(&navigated, &author_frame_id, &main_frame_id)?;
        let context = cdp.wait_for_author_event(
            "Runtime.executionContextCreated",
            &author_session_id,
            &author_frame_id,
            |event| {
                let auxiliary = event
                    .get("params")
                    .and_then(|params| params.get("context"))
                    .and_then(|context| context.get("auxData"));
                auxiliary
                    .and_then(|auxiliary| auxiliary.get("frameId"))
                    .and_then(Value::as_str)
                    == Some(author_frame_id.as_str())
                    && auxiliary
                        .and_then(|auxiliary| auxiliary.get("isDefault"))
                        .and_then(Value::as_bool)
                        == Some(true)
            },
        )?;
        let author_context_id = context
            .get("params")
            .and_then(|params| params.get("context"))
            .and_then(|context| context.get("id"))
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                MotionError::render_failed(
                    "Chromium author frame has no default JavaScript execution context",
                )
            })?;
        cdp.wait_for_author_event(
            "Page.frameStoppedLoading",
            &author_session_id,
            &author_frame_id,
            |event| {
                event
                    .get("params")
                    .and_then(|params| params.get("frameId"))
                    .and_then(Value::as_str)
                    == Some(author_frame_id.as_str())
            },
        )?;
        cdp.wait_for_event("Page.loadEventFired", Some(&author_session_id))?;
        cdp.ensure_no_blocked_url()?;

        let fence_nonce = author_fence_nonce();
        let isolated = cdp.command(
            "Page.createIsolatedWorld",
            json!({
                "frameId": author_frame_id,
                "worldName": format!("opentake-paint-fence-{fence_nonce}"),
                "grantUniveralAccess": false
            }),
            Some(&author_session_id),
        )?;
        let fence_context_id = isolated
            .get("executionContextId")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                MotionError::render_failed(
                    "Chromium author paint-fence world has no execution context",
                )
            })?;
        let installed = cdp.command(
            "Runtime.evaluate",
            json!({
                "expression": author_fence_install_expression(&fence_nonce),
                "contextId": fence_context_id,
                "returnByValue": true
            }),
            Some(&author_session_id),
        )?;
        if !runtime_boolean(&installed) {
            return Err(MotionError::render_failed(
                "Chromium author paint-fence controller installation failed",
            ));
        }

        Ok(InstalledHost {
            author_fence: AuthorPaintFence {
                session_id: author_session_id.clone(),
                context_id: fence_context_id,
                nonce: fence_nonce,
            },
            author_session_id,
            author_context_id,
        })
    }

    fn validate_author_navigation(
        event: &Value,
        author_frame_id: &str,
        main_frame_id: &str,
    ) -> MotionResult<()> {
        let frame = event
            .get("params")
            .and_then(|params| params.get("frame"))
            .ok_or_else(|| MotionError::render_failed("Chromium author navigation has no frame"))?;
        if frame.get("id").and_then(Value::as_str) != Some(author_frame_id) {
            return Err(MotionError::render_failed(
                "Chromium author navigation does not match its iframe target",
            ));
        }
        if frame.get("parentId").and_then(Value::as_str) != Some(main_frame_id) {
            return Err(MotionError::render_failed(
                "Chromium author iframe is not a child of the host frame",
            ));
        }
        if !frame
            .get("url")
            .and_then(Value::as_str)
            .is_some_and(|url| url.starts_with("data:"))
        {
            return Err(MotionError::render_failed(
                "Chromium author iframe did not commit a data document",
            ));
        }
        Ok(())
    }

    fn check_abort(
        cancellation: &MotionCancellationToken,
        deadline: Instant,
        timeout: Duration,
    ) -> MotionResult<()> {
        check_abort_state(cancellation, deadline, timeout)
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
        cancellation: &MotionCancellationToken,
        timeout: Duration,
        deadline: Instant,
        dir: &Path,
        partial: &mut PartialFrames,
    ) -> MotionResult<()> {
        check_abort(cancellation, deadline, timeout)?;
        MotionCache::mark_complete(dir)?;
        if let Err(error) = check_abort(cancellation, deadline, timeout) {
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

    fn runtime_boolean(evaluated: &Value) -> bool {
        evaluated
            .get("result")
            .and_then(|result| result.get("value"))
            .and_then(Value::as_bool)
            == Some(true)
            && evaluated.get("exceptionDetails").is_none()
    }

    fn author_fence_nonce() -> String {
        let counter = AUTHOR_FENCE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let process = std::process::id();
        let first = RandomState::new().hash_one((process, nanos, counter, 0_u8));
        let second = RandomState::new().hash_one((process, nanos, counter, 1_u8));
        format!("{first:016x}{second:016x}")
    }

    fn author_fence_key(nonce: &str) -> String {
        format!("__opentakeAuthorPaintFence_{nonce}")
    }

    fn author_fence_install_expression(nonce: &str) -> String {
        const TEMPLATE: &str = r#"(() => {
  const key = __KEY__;
  const nonce = __NONCE__;
  if (Object.prototype.hasOwnProperty.call(globalThis, key)) return false;
  let host = null;
  let canvas = null;
  const retiredHosts = [];
  const setImportant = (name, value) => host.style.setProperty(name, value, 'important');
  const controller = (request) => {
    if (!request || request.nonce !== nonce) return false;
    if (retiredHosts.some((retired) => retired.isConnected)) return false;
    if (request.action === 'clear') {
      if (!host || !host.isConnected || !canvas || !canvas.isConnected) return false;
      canvas.width = 0;
      canvas.height = 0;
      canvas.remove();
      host.remove();
      if (host.isConnected || canvas.isConnected) return false;
      retiredHosts.push(host);
      host = null;
      canvas = null;
      return true;
    }
    if (request.action !== 'paint'
        || !Number.isInteger(request.x) || !Number.isInteger(request.y)
        || request.x < 0 || request.y < 0
        || !Array.isArray(request.rgb) || request.rgb.length !== 3
        || request.rgb.some((channel) => !Number.isInteger(channel) || channel < 0 || channel > 255)) {
      return false;
    }
    if (host) {
      if (!host.isConnected || !canvas || !canvas.isConnected) return false;
      const current = getComputedStyle(host);
      if (current.position !== 'fixed' || current.width !== '1px'
          || current.height !== '1px' || current.opacity !== '1'
          || current.display === 'none' || current.visibility !== 'visible') return false;
    } else {
      host = document.createElement('opentake-paint-fence');
      const shadow = host.attachShadow({mode: 'closed'});
      canvas = document.createElement('canvas');
      canvas.width = 1;
      canvas.height = 1;
      canvas.style.cssText = 'all:initial!important;display:block!important;width:1px!important;height:1px!important';
      shadow.appendChild(canvas);
      setImportant('all', 'initial');
      setImportant('position', 'fixed');
      setImportant('display', 'block');
      setImportant('visibility', 'visible');
      setImportant('width', '1px');
      setImportant('height', '1px');
      setImportant('margin', '0');
      setImportant('padding', '0');
      setImportant('border', '0');
      setImportant('opacity', '1');
      setImportant('filter', 'none');
      setImportant('transform', 'none');
      setImportant('mix-blend-mode', 'normal');
      setImportant('pointer-events', 'none');
      setImportant('overflow', 'hidden');
      setImportant('z-index', '2147483647');
      document.documentElement.appendChild(host);
    }
    setImportant('left', `${request.x}px`);
    setImportant('top', `${request.y}px`);
    const context = canvas.getContext('2d', {alpha: false});
    if (!context) return false;
    context.globalCompositeOperation = 'copy';
    context.fillStyle = `rgb(${request.rgb[0]} ${request.rgb[1]} ${request.rgb[2]})`;
    context.fillRect(0, 0, 1, 1);
    const pixel = context.getImageData(0, 0, 1, 1).data;
    const rect = host.getBoundingClientRect();
    return host.isConnected && canvas.isConnected
      && rect.x === request.x && rect.y === request.y
      && rect.width === 1 && rect.height === 1
      && pixel[0] === request.rgb[0] && pixel[1] === request.rgb[1]
      && pixel[2] === request.rgb[2] && pixel[3] === 255;
  };
  Object.defineProperty(globalThis, key, {
    configurable: false,
    enumerable: false,
    writable: false,
    value: controller
  });
  return true;
})()"#;
        TEMPLATE
            .replace(
                "__KEY__",
                &serde_json::to_string(&author_fence_key(nonce))
                    .expect("author fence key is serializable"),
            )
            .replace(
                "__NONCE__",
                &serde_json::to_string(nonce).expect("author fence nonce is serializable"),
            )
    }

    fn author_marker_expression(fence: &AuthorPaintFence, marker: AuthorMarker) -> String {
        let key = serde_json::to_string(&author_fence_key(&fence.nonce))
            .expect("author fence key is serializable");
        let nonce =
            serde_json::to_string(&fence.nonce).expect("author fence nonce is serializable");
        format!(
            "(() => {{ const controller = globalThis[{key}]; return typeof controller === 'function' && controller({{nonce:{nonce},action:'paint',x:{},y:{},rgb:[{},{},{}]}}); }})()",
            marker.x, marker.y, marker.rgb[0], marker.rgb[1], marker.rgb[2]
        )
    }

    fn clear_author_marker_expression(fence: &AuthorPaintFence) -> String {
        let key = serde_json::to_string(&author_fence_key(&fence.nonce))
            .expect("author fence key is serializable");
        let nonce =
            serde_json::to_string(&fence.nonce).expect("author fence nonce is serializable");
        format!(
            "(() => {{ const controller = globalThis[{key}]; return typeof controller === 'function' && controller({{nonce:{nonce},action:'clear'}}); }})()"
        )
    }

    fn author_marker_plan(nonce: &str, generation: u64) -> [AuthorMarker; 3] {
        let positions = [(0, 0), (1, 0), (0, 1)];
        let mut colors = [[0_u8; 3]; 3];
        for index in 0..colors.len() {
            let mut hasher = Sha256::new();
            hasher.update(nonce.as_bytes());
            hasher.update(generation.to_le_bytes());
            hasher.update([index as u8]);
            let digest = hasher.finalize();
            let mut color = [digest[0], digest[1], digest[2]];
            while colors[..index].contains(&color) {
                color[2] = color[2].wrapping_add(1);
            }
            colors[index] = color;
        }
        std::array::from_fn(|index| AuthorMarker {
            x: positions[index].0,
            y: positions[index].1,
            rgb: colors[index],
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
                .args(browser_launch_args())
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
                drain_browser_stderr(BufReader::new(stderr), sender);
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

        fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
            self.child
                .as_mut()
                .ok_or_else(|| std::io::Error::other("Chromium child handle is missing"))?
                .try_wait()
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
        next_capture_generation: u32,
        next_author_marker_generation: u64,
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
                next_capture_generation: 0,
                next_author_marker_generation: 0,
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

        fn gpu_backend_trace(&mut self) -> MotionResult<String> {
            let id = self.next_id;
            self.next_id += 1;
            self.send(json!({
                "id": id,
                "method": "SystemInfo.getInfo",
                "params": {}
            }))?;

            loop {
                let value = self.read()?;
                if value.get("id").and_then(Value::as_u64) == Some(id) {
                    if value.get("error").is_some() {
                        return Ok("gpu backend unavailable reason=command-rejected".to_owned());
                    }
                    return Ok(value
                        .get("result")
                        .and_then(gpu_backend_summary)
                        .unwrap_or_else(|| {
                            "gpu backend unavailable reason=incomplete-result".to_owned()
                        }));
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

        fn wait_for_event_matching(
            &mut self,
            method: &str,
            session: Option<&str>,
            mut predicate: impl FnMut(&Value) -> bool,
        ) -> MotionResult<Value> {
            if let Some(index) = self.pending_events.iter().position(|event| {
                event.get("method").and_then(Value::as_str) == Some(method)
                    && session.is_none_or(|expected| {
                        event.get("sessionId").and_then(Value::as_str) == Some(expected)
                    })
                    && predicate(event)
            }) {
                return Ok(self.pending_events.remove(index));
            }
            loop {
                let value = self.read()?;
                if value.get("method").and_then(Value::as_str) == Some(method)
                    && session.is_none_or(|expected| {
                        value.get("sessionId").and_then(Value::as_str) == Some(expected)
                    })
                    && predicate(&value)
                {
                    return Ok(value);
                }
                self.handle_event_or_queue(value)?;
            }
        }

        fn wait_for_author_event(
            &mut self,
            method: &str,
            session: &str,
            author_frame_id: &str,
            mut predicate: impl FnMut(&Value) -> bool,
        ) -> MotionResult<Value> {
            loop {
                if let Some(index) = self
                    .pending_events
                    .iter()
                    .position(|event| is_frame_detached_event(event, session, author_frame_id))
                {
                    self.pending_events.remove(index);
                    return Err(MotionError::render_failed(
                        "Chromium author frame detached before loading completed",
                    ));
                }
                if let Some(index) = self.pending_events.iter().position(|event| {
                    event.get("method").and_then(Value::as_str) == Some(method)
                        && event.get("sessionId").and_then(Value::as_str) == Some(session)
                        && predicate(event)
                }) {
                    return Ok(self.pending_events.remove(index));
                }

                let value = self.read()?;
                if is_frame_detached_event(&value, session, author_frame_id) {
                    return Err(MotionError::render_failed(
                        "Chromium author frame detached before loading completed",
                    ));
                }
                if value.get("method").and_then(Value::as_str) == Some(method)
                    && value.get("sessionId").and_then(Value::as_str) == Some(session)
                    && predicate(&value)
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
            // Screencast sizing is based on the compositor surface, so resize
            // the browser contents before overriding the logical viewport.
            let window = self.command("Browser.getWindowForTarget", json!({}), Some(session))?;
            let window_id = window
                .get("windowId")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    MotionError::render_failed(format!(
                        "Chromium CDP response is missing integer field \"windowId\": {window}"
                    ))
                })?;
            self.command(
                "Browser.setContentsSize",
                json!({
                    "windowId": window_id,
                    "width": width,
                    "height": height
                }),
                None,
            )?;
            self.command(
                "Emulation.setDeviceMetricsOverride",
                device_metrics_params(width, height),
                Some(session),
            )?;
            Ok(())
        }

        #[cfg(test)]
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
            pass: CapturePass<'_>,
            author_marker: Option<(&AuthorPaintFence, AuthorMarker)>,
        ) -> MotionResult<image::RgbaImage> {
            let CapturePass { frame, background } = pass;
            let CaptureFrame {
                width,
                height,
                index: frame_index,
            } = frame;
            let generation = self.next_capture_generation;
            self.next_capture_generation =
                self.next_capture_generation.checked_add(1).ok_or_else(|| {
                    MotionError::render_failed("Chromium capture generation overflowed")
                })?;
            let (seed, transition) = capture_generation_colors(generation)?;
            let guarded_width = width
                .checked_add(1)
                .ok_or_else(|| MotionError::render_failed("Chromium guard width overflowed"))?;
            let guarded_height = height
                .checked_add(1)
                .ok_or_else(|| MotionError::render_failed("Chromium guard height overflowed"))?;
            let attached = self.command(
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
                None,
            )?;
            let capture_session = required_string(&attached, "sessionId")?;
            let mut started = false;
            let captured = (|| -> MotionResult<image::RgbaImage> {
                self.command("Page.enable", json!({}), Some(&capture_session))?;
                self.command(
                    "Emulation.setVirtualTimePolicy",
                    json!({"policy": "pause"}),
                    Some(&capture_session),
                )?;
                self.set_host_background(&capture_session, seed)?;
                self.command(
                    "Page.startScreencast",
                    json!({
                        "format": "png",
                        "maxWidth": guarded_width,
                        "maxHeight": guarded_height,
                        "everyNthFrame": 1
                    }),
                    Some(&capture_session),
                )?;
                started = true;
                self.ensure_no_blocked_url()?;
                if let Some((fence, marker)) = author_marker {
                    self.set_author_marker(fence, marker)?;
                }
                trace(format!(
                    "frame {frame_index}: {background} transition guard start"
                ));
                self.set_host_background(&capture_session, transition)?;
                // The author runs in a separately paused OOPIF target. Advancing
                // only the script-free host target gives Windows Chromium a
                // bounded lifecycle/compositor turn without advancing author
                // timers between the black and white alpha samples.
                self.settle_compositor(&capture_session)?;
                let transition_image =
                    self.receive_guarded_generation(&capture_session, transition, pass, None)?;
                drop(transition_image);
                trace(format!(
                    "frame {frame_index}: {background} transition guard complete; desired guard start"
                ));
                self.set_host_background(&capture_session, rgb)?;
                self.settle_compositor(&capture_session)?;
                let desired_image = self.receive_guarded_generation(
                    &capture_session,
                    rgb,
                    pass,
                    author_marker.map(|(_, marker)| marker),
                )?;
                trace(format!(
                    "frame {frame_index}: {background} desired guard complete"
                ));
                crop_guarded_image(desired_image, width, height, frame_index)
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

        fn receive_guarded_generation(
            &mut self,
            session: &str,
            expected_guard: [u8; 3],
            pass: CapturePass<'_>,
            author_marker: Option<AuthorMarker>,
        ) -> MotionResult<image::RgbaImage> {
            let CapturePass { frame, background } = pass;
            let CaptureFrame {
                width,
                height,
                index: frame_index,
            } = frame;
            loop {
                self.check_abort()?;
                let png = self.receive_and_ack_screencast_png(session, frame_index)?;
                let image = decode_viewport_png(&png, background, frame_index)?;
                let expected_dimensions = (
                    width.checked_add(1).ok_or_else(|| {
                        MotionError::render_failed("Chromium guard width overflowed")
                    })?,
                    height.checked_add(1).ok_or_else(|| {
                        MotionError::render_failed("Chromium guard height overflowed")
                    })?,
                );
                if image.dimensions() != expected_dimensions {
                    return Err(MotionError::render_failed(format!(
                        "Chromium guarded {background} screencast has the wrong size for frame {frame_index}: actual={:?}, expected={expected_dimensions:?}",
                        image.dimensions()
                    )));
                }
                if external_guard_matches(&image, width, height, expected_guard)
                    && author_marker.is_none_or(|marker| author_marker_matches(&image, marker))
                {
                    return Ok(image);
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
            author_fence: &AuthorPaintFence,
            transparent: bool,
            frame: CaptureFrame,
        ) -> MotionResult<Vec<u8>> {
            self.check_abort()?;
            if !transparent {
                let white = self.capture_stable_background(
                    target_id,
                    session,
                    author_fence,
                    [255, 255, 255],
                    CapturePass {
                        frame,
                        background: "opaque-white",
                    },
                )?;
                self.check_abort()?;
                let normalized = encode_viewport_png(white, frame.index)?;
                self.check_abort()?;
                self.clear_author_marker(author_fence)?;
                return Ok(normalized);
            }

            // Three independently guard-committed author marker generations
            // provide two exact, unobscured samples for every output pixel.
            // Transparency therefore uses six root compositor readbacks;
            // opaque output uses three.
            let black = self.capture_stable_background(
                target_id,
                session,
                author_fence,
                [0, 0, 0],
                CapturePass {
                    frame,
                    background: "black",
                },
            )?;
            let white = self.capture_stable_background(
                target_id,
                session,
                author_fence,
                [255, 255, 255],
                CapturePass {
                    frame,
                    background: "white",
                },
            )?;
            self.check_abort()?;
            let recovered = recover_transparent_images(black, white, frame.index)?;
            self.check_abort()?;
            self.clear_author_marker(author_fence)?;
            Ok(recovered)
        }

        fn check_abort(&self) -> MotionResult<()> {
            check_abort_state(&self.cancellation, self.deadline, self.policy.timeout)
        }

        fn close_target(&mut self, target_id: &str) -> MotionResult<()> {
            let closed =
                self.command("Target.closeTarget", json!({"targetId": target_id}), None)?;
            if closed.get("success").and_then(Value::as_bool) != Some(true) {
                return Err(MotionError::render_failed(
                    "Chromium did not close the render target",
                ));
            }
            self.ensure_no_blocked_url()
        }

        fn create_browser_context(&mut self) -> MotionResult<String> {
            let created = self.command(
                "Target.createBrowserContext",
                json!({"disposeOnDetach": true}),
                None,
            )?;
            required_string(&created, "browserContextId")
        }

        fn dispose_browser_context(&mut self, browser_context_id: &str) -> MotionResult<()> {
            self.command(
                "Target.disposeBrowserContext",
                json!({"browserContextId": browser_context_id}),
                None,
            )?;
            self.ensure_no_blocked_url()
        }

        fn set_host_background(&mut self, session: &str, rgb: [u8; 3]) -> MotionResult<()> {
            let evaluated = self.command(
                "Runtime.evaluate",
                json!({
                    "expression": host_background_expression(rgb),
                    "returnByValue": true
                }),
                Some(session),
            )?;
            if !runtime_boolean(&evaluated) {
                return Err(MotionError::render_failed(
                    "Chromium host background layer update failed",
                ));
            }
            self.ensure_no_blocked_url()
        }

        fn set_author_marker(
            &mut self,
            fence: &AuthorPaintFence,
            marker: AuthorMarker,
        ) -> MotionResult<()> {
            let evaluated = self.command(
                "Runtime.evaluate",
                json!({
                    "expression": author_marker_expression(fence, marker),
                    "contextId": fence.context_id,
                    "returnByValue": true
                }),
                Some(&fence.session_id),
            )?;
            if !runtime_boolean(&evaluated) {
                return Err(MotionError::render_failed(
                    "Chromium author paint-fence marker update failed",
                ));
            }
            self.ensure_no_blocked_url()
        }

        fn clear_author_marker(&mut self, fence: &AuthorPaintFence) -> MotionResult<()> {
            let evaluated = self.command(
                "Runtime.evaluate",
                json!({
                    "expression": clear_author_marker_expression(fence),
                    "contextId": fence.context_id,
                    "returnByValue": true
                }),
                Some(&fence.session_id),
            )?;
            if !runtime_boolean(&evaluated) {
                return Err(MotionError::render_failed(
                    "Chromium author paint-fence marker cleanup failed",
                ));
            }
            self.ensure_no_blocked_url()
        }

        fn next_author_marker_plan(
            &mut self,
            fence: &AuthorPaintFence,
        ) -> MotionResult<[AuthorMarker; 3]> {
            let generation = self.next_author_marker_generation;
            self.next_author_marker_generation = self
                .next_author_marker_generation
                .checked_add(1)
                .ok_or_else(|| {
                    MotionError::render_failed("Chromium author marker generation overflowed")
                })?;
            Ok(author_marker_plan(&fence.nonce, generation))
        }

        fn capture_stable_background(
            &mut self,
            target_id: &str,
            session: &str,
            author_fence: &AuthorPaintFence,
            rgb: [u8; 3],
            pass: CapturePass<'_>,
        ) -> MotionResult<image::RgbaImage> {
            let CapturePass { frame, background } = pass;
            self.set_host_background(session, rgb)?;
            let markers = self.next_author_marker_plan(author_fence)?;
            let first = self.capture_isolated_viewport(
                target_id,
                rgb,
                pass,
                Some((author_fence, markers[0])),
            )?;
            let second = self.capture_isolated_viewport(
                target_id,
                rgb,
                pass,
                Some((author_fence, markers[1])),
            )?;
            let second_p0 =
                validate_author_marker_pair(&first, &second, markers, background, frame.index);
            if let Err(error) = second_p0.as_ref() {
                return Err(MotionError::render_failed(format!(
                    "{error}; sandbox_blocked_url_seen={}",
                    self.blocked_url.is_some()
                )));
            }
            let second_p0 = second_p0?;
            // I0/I1 have already established every pixel except p0/p1.
            // Release I1 before capturing I2 so transparent 4K recovery keeps
            // at most two readbacks for the active background in memory.
            drop(second);
            let third = self.capture_isolated_viewport(
                target_id,
                rgb,
                pass,
                Some((author_fence, markers[2])),
            )?;
            let stable = reconcile_author_marker_third(
                first,
                third,
                markers,
                second_p0,
                background,
                frame.index,
            );
            if let Err(error) = stable.as_ref() {
                return Err(MotionError::render_failed(format!(
                    "{error}; sandbox_blocked_url_seen={}",
                    self.blocked_url.is_some()
                )));
            }
            self.check_abort()?;
            stable
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

    fn trace_gpu_backend_if_enabled(cdp: &mut Cdp, enabled: bool) -> MotionResult<()> {
        if enabled {
            trace(cdp.gpu_backend_trace()?);
        }
        Ok(())
    }

    fn is_frame_detached_event(event: &Value, session: &str, frame_id: &str) -> bool {
        event.get("method").and_then(Value::as_str) == Some("Page.frameDetached")
            && event.get("sessionId").and_then(Value::as_str) == Some(session)
            && event
                .get("params")
                .and_then(|params| params.get("frameId"))
                .and_then(Value::as_str)
                == Some(frame_id)
    }

    fn is_screencast_event_for_session(event: &Value, session: &str) -> bool {
        event.get("method").and_then(Value::as_str) == Some("Page.screencastFrame")
            && event.get("sessionId").and_then(Value::as_str) == Some(session)
    }

    fn gpu_backend_summary(result: &Value) -> Option<String> {
        let gpu = result.get("gpu")?;
        let device = gpu.get("devices")?.as_array()?.first()?;
        let vendor =
            bounded_gpu_trace_field(device.get("vendorString")?.as_str()?, GPU_TRACE_FIELD_LIMIT);
        let device =
            bounded_gpu_trace_field(device.get("deviceString")?.as_str()?, GPU_TRACE_FIELD_LIMIT);
        let gpu_compositing = bounded_gpu_trace_field(
            gpu.get("featureStatus")?.get("gpu_compositing")?.as_str()?,
            GPU_TRACE_STATUS_LIMIT,
        );
        let backend = format!("{vendor} {device}").to_ascii_lowercase();
        let class = if backend.contains("swiftshader") {
            "swiftshader"
        } else if vendor.eq_ignore_ascii_case("disabled") || device.eq_ignore_ascii_case("disabled")
        {
            "disabled"
        } else {
            "driver"
        };
        Some(format!(
            "gpu backend class={class} vendor=\"{vendor}\" device=\"{device}\" gpu_compositing=\"{gpu_compositing}\""
        ))
    }

    fn bounded_gpu_trace_field(value: &str, limit: usize) -> String {
        let mut chars = value.chars();
        let mut bounded = String::with_capacity(limit + 1);
        for character in chars.by_ref().take(limit) {
            let safe = match character {
                ' '..='!' | '#'..='[' | ']'..='~' => character,
                _ => '?',
            };
            bounded.push(safe);
        }
        if chars.next().is_some() {
            bounded.push('…');
        }
        bounded
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

    #[cfg(test)]
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

    fn validate_author_marker_pair(
        first: &image::RgbaImage,
        second: &image::RgbaImage,
        markers: [AuthorMarker; 3],
        background: &str,
        frame_index: usize,
    ) -> MotionResult<[u8; 4]> {
        let dimensions = first.dimensions();
        if second.dimensions() != dimensions {
            return Err(MotionError::render_failed(format!(
                "Chromium {background}-background author-fenced pair returned inconsistent sizes for frame {frame_index}: first={dimensions:?}, second={:?}",
                second.dimensions()
            )));
        }
        if dimensions.0 < 2 || dimensions.1 < 2 {
            return Err(MotionError::render_failed(format!(
                "Chromium author paint fence requires a viewport of at least 2x2 for frame {frame_index}"
            )));
        }
        for (image, marker) in [(first, markers[0]), (second, markers[1])] {
            if !author_marker_matches(image, marker) {
                return Err(MotionError::render_failed(format!(
                    "Chromium {background}-background author marker was missing from frame {frame_index} at ({}, {})",
                    marker.x, marker.y
                )));
            }
        }

        let mut differing_pixels = 0_usize;
        let mut sample_coordinates = Vec::with_capacity(8);
        for y in 0..dimensions.1 {
            for x in 0..dimensions.0 {
                let coordinate = (x, y);
                if coordinate == (markers[0].x, markers[0].y)
                    || coordinate == (markers[1].x, markers[1].y)
                {
                    continue;
                }
                if first.get_pixel(x, y) != second.get_pixel(x, y) {
                    differing_pixels += 1;
                    if sample_coordinates.len() < 8 {
                        sample_coordinates.push(coordinate);
                    }
                }
            }
        }
        if differing_pixels != 0 {
            return Err(MotionError::render_failed(format!(
                "Chromium {background}-background author-fenced pair diverged for frame {frame_index}: dimensions={dimensions:?}, differing_pixels={differing_pixels}, sample_coordinates={sample_coordinates:?}"
            )));
        }
        Ok(second.get_pixel(markers[0].x, markers[0].y).0)
    }

    fn reconcile_author_marker_third(
        mut first: image::RgbaImage,
        third: image::RgbaImage,
        markers: [AuthorMarker; 3],
        second_p0: [u8; 4],
        background: &str,
        frame_index: usize,
    ) -> MotionResult<image::RgbaImage> {
        let dimensions = first.dimensions();
        if third.dimensions() != dimensions {
            return Err(MotionError::render_failed(format!(
                "Chromium {background}-background author-fenced third readback returned an inconsistent size for frame {frame_index}: first={dimensions:?}, third={:?}",
                third.dimensions()
            )));
        }
        if !author_marker_matches(&third, markers[2]) {
            return Err(MotionError::render_failed(format!(
                "Chromium {background}-background author marker was missing from frame {frame_index} at ({}, {})",
                markers[2].x, markers[2].y
            )));
        }

        let p0 = (markers[0].x, markers[0].y);
        let p2 = (markers[2].x, markers[2].y);
        let mut differing_pixels = 0_usize;
        let mut sample_coordinates = Vec::with_capacity(8);
        for y in 0..dimensions.1 {
            for x in 0..dimensions.0 {
                let coordinate = (x, y);
                if coordinate == p2 {
                    // I0 and I1 already provided the two unmarked exact
                    // samples for p2 before I1 was released.
                    continue;
                }
                let first_pixel = if coordinate == p0 {
                    second_p0
                } else {
                    first.get_pixel(x, y).0
                };
                if first_pixel != third.get_pixel(x, y).0 {
                    differing_pixels += 1;
                    if sample_coordinates.len() < 8 {
                        sample_coordinates.push(coordinate);
                    }
                }
            }
        }
        if differing_pixels != 0 {
            return Err(MotionError::render_failed(format!(
                "Chromium {background}-background author-fenced third readback diverged for frame {frame_index}: dimensions={dimensions:?}, differing_pixels={differing_pixels}, sample_coordinates={sample_coordinates:?}"
            )));
        }
        first.put_pixel(markers[0].x, markers[0].y, image::Rgba(second_p0));
        Ok(first)
    }

    #[cfg(test)]
    fn reconcile_author_marker_images(
        images: [image::RgbaImage; 3],
        markers: [AuthorMarker; 3],
        background: &str,
        frame_index: usize,
    ) -> MotionResult<image::RgbaImage> {
        let [first, second, third] = images;
        let second_p0 =
            validate_author_marker_pair(&first, &second, markers, background, frame_index)?;
        drop(second);
        reconcile_author_marker_third(first, third, markers, second_p0, background, frame_index)
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

    fn capture_generation_colors(generation: u32) -> MotionResult<([u8; 3], [u8; 3])> {
        let generation = u16::try_from(generation).map_err(|_| {
            MotionError::render_failed("Chromium capture generation exceeded its unique range")
        })?;
        let [low, high] = generation.to_le_bytes();
        Ok(([low, high, 90], [low, high, 165]))
    }

    fn host_background_expression(rgb: [u8; 3]) -> String {
        format!(
            "(() => {{ const layer = document.getElementById('opentake-host-background'); if (!layer) return false; layer.style.backgroundColor = 'rgb({} {} {})'; return true; }})()",
            rgb[0], rgb[1], rgb[2]
        )
    }

    fn external_guard_matches(
        image: &image::RgbaImage,
        width: u32,
        height: u32,
        expected: [u8; 3],
    ) -> bool {
        let Some(guarded_width) = width.checked_add(1) else {
            return false;
        };
        let Some(guarded_height) = height.checked_add(1) else {
            return false;
        };
        if image.dimensions() != (guarded_width, guarded_height) {
            return false;
        }
        let expected = [expected[0], expected[1], expected[2], 255];
        (0..guarded_height).all(|y| image.get_pixel(width, y).0 == expected)
            && (0..guarded_width).all(|x| image.get_pixel(x, height).0 == expected)
    }

    fn author_marker_matches(image: &image::RgbaImage, marker: AuthorMarker) -> bool {
        image
            .get_pixel_checked(marker.x, marker.y)
            .is_some_and(|pixel| pixel.0 == [marker.rgb[0], marker.rgb[1], marker.rgb[2], 255])
    }

    fn crop_guarded_image(
        image: image::RgbaImage,
        width: u32,
        height: u32,
        frame_index: usize,
    ) -> MotionResult<image::RgbaImage> {
        let guarded_width = width
            .checked_add(1)
            .ok_or_else(|| MotionError::render_failed("Chromium host guard width overflowed"))?;
        let guarded_height = height
            .checked_add(1)
            .ok_or_else(|| MotionError::render_failed("Chromium host guard height overflowed"))?;
        if image.dimensions() != (guarded_width, guarded_height) {
            return Err(MotionError::render_failed(format!(
                "Chromium guarded frame {frame_index} has the wrong size: actual={:?}, expected=({guarded_width}, {guarded_height})",
                image.dimensions()
            )));
        }

        let guarded_stride = guarded_width as usize * 4;
        let content_stride = width as usize * 4;
        let mut raw = image.into_raw();
        for row in 1..height as usize {
            let source = row * guarded_stride;
            let destination = row * content_stride;
            raw.copy_within(source..source + content_stride, destination);
        }
        raw.truncate(content_stride * height as usize);
        image::RgbaImage::from_raw(width, height, raw).ok_or_else(|| {
            MotionError::render_failed(format!(
                "failed to crop Chromium host guard for frame {frame_index}"
            ))
        })
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

        fn fake_cdp_pair() -> (Cdp, WebSocket<TcpStream>) {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            let server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            (
                Cdp::new(
                    client_socket,
                    SandboxPolicy::default(),
                    MotionCancellationToken::new(),
                    Instant::now() + Duration::from_secs(1),
                ),
                server_socket,
            )
        }

        fn test_author_fence() -> AuthorPaintFence {
            AuthorPaintFence {
                session_id: "author-session".to_owned(),
                context_id: 77,
                nonce: "0123456789abcdef0123456789abcdef".to_owned(),
            }
        }

        fn validate_browser_launch_args_contract(args: &[&str]) -> Result<(), String> {
            const EXPECTED: &[&str] = &[
                "--headless=new",
                "--remote-debugging-port=0",
                "--remote-debugging-address=127.0.0.1",
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
            ];
            let mut keys = std::collections::BTreeSet::new();
            for argument in args.iter().filter(|argument| argument.starts_with("--")) {
                let key = argument.split_once('=').map_or(*argument, |(key, _)| key);
                if key == "--disable-gpu" {
                    return Err(format!("forbidden Chromium switch key: {key}"));
                }
                if !keys.insert(key) {
                    return Err(format!("duplicate Chromium switch key: {key}"));
                }
            }
            if args != EXPECTED {
                return Err("Chromium launch arguments differ from the expected contract".into());
            }
            Ok(())
        }

        #[test]
        fn browser_stderr_is_drained_after_the_endpoint_receiver_disconnects() {
            let mut stderr =
                b"DevTools listening on ws://127.0.0.1/devtools/browser/test\n".to_vec();
            for index in 0..512 {
                stderr.extend_from_slice(format!("gpu-viz-diagnostic-{index:04}\n").as_bytes());
            }
            let (sender, receiver) = mpsc::channel();
            drop(receiver);

            assert_eq!(
                drain_browser_stderr(Cursor::new(stderr), sender),
                513,
                "Chrome stderr must be consumed through EOF even after endpoint discovery"
            );
        }

        #[test]
        fn pending_browser_invalidation_covers_idle_busy_and_handoff_interleavings() {
            let pool = BrowserPool::new();

            pool.invalidate_idle();
            assert!(!pool.invalidation_pending.load(Ordering::Acquire));

            let active_lease_seam = pool.slot.lock().unwrap();
            pool.invalidate_idle();
            assert!(pool.invalidation_pending.load(Ordering::Acquire));
            drop(active_lease_seam);

            pool.drain_pending_invalidation();
            assert!(!pool.invalidation_pending.load(Ordering::Acquire));

            pool.invalidate_idle();
            assert!(!pool.invalidation_pending.load(Ordering::Acquire));
        }

        #[test]
        fn acquire_observed_invalidation_taints_the_new_browser_lease() {
            let pool = BrowserPool::new();
            let slot = pool.slot.lock().unwrap();
            pool.invalidation_pending.store(true, Ordering::Release);
            let observed_invalidation = pool.invalidation_pending.swap(false, Ordering::AcqRel);
            assert!(observed_invalidation);

            let mut lease = BrowserLease {
                pool: &pool,
                slot: Some(slot),
                reusable: false,
                observed_invalidation,
            };
            lease.commit_reuse();
            assert!(
                !lease.reusable,
                "an acquire overlapping invalidation must not retain a subsequently launched browser"
            );
        }

        #[test]
        fn browser_launch_args_bind_remote_debugging_to_loopback() {
            validate_browser_launch_args_contract(browser_launch_args()).unwrap();
        }

        #[test]
        fn browser_launch_args_contract_rejects_disable_gpu() {
            let mut disable_gpu = browser_launch_args().to_vec();
            disable_gpu.push("--disable-gpu=true");
            assert_eq!(
                validate_browser_launch_args_contract(&disable_gpu).unwrap_err(),
                "forbidden Chromium switch key: --disable-gpu"
            );
        }

        #[test]
        fn host_wrapper_isolates_author_in_an_exact_child_viewport() {
            let wrapper = host_wrapper_document(
                r#"<main data-value="a&b"><script>window.test = "<ok>"</script></main>"#,
                48,
                32,
                &SandboxPolicy::default(),
            );
            assert!(wrapper.contains("sandbox=\"allow-scripts\""));
            assert!(!wrapper.contains("allow-same-origin"));
            assert!(!wrapper.contains("allow-top-navigation"));
            assert!(wrapper.contains("frame-src data:"));
            assert!(wrapper.contains("width:48px;height:32px"));
            assert!(wrapper.contains("overflow:hidden;background:transparent"));
            assert!(wrapper.contains("id=\"opentake-host-background\""));
            assert!(wrapper.contains(
                "#opentake-host-background{position:fixed;inset:0;z-index:0;background:transparent}"
            ));
            assert!(wrapper.contains("iframe{position:absolute;left:0;top:0;z-index:1"));
            assert!(wrapper.contains("src=\"data:text/html;charset=utf-8,"));
            assert!(!wrapper.contains("srcdoc="));
            assert!(!wrapper.contains("<main"));
            assert!(!wrapper.contains("data-value=\"a&b\""));
        }

        #[test]
        fn host_and_author_csp_share_the_same_offline_resource_policy() {
            let policy = SandboxPolicy::default();
            assert_eq!(
                sandbox_csp(&policy, "data:"),
                "default-src 'none'; script-src 'unsafe-inline' data: 'none'; style-src 'unsafe-inline' data: 'none'; img-src data: 'none'; media-src data: 'none'; font-src data: 'none'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-src data:; worker-src 'none'"
            );
            assert_eq!(
                sandbox_csp(&policy, "'none'"),
                "default-src 'none'; script-src 'unsafe-inline' data: 'none'; style-src 'unsafe-inline' data: 'none'; img-src data: 'none'; media-src data: 'none'; font-src data: 'none'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-src 'none'; worker-src 'none'"
            );
        }

        #[test]
        fn host_and_author_csp_mirror_an_allowed_loopback_origin() {
            let policy = SandboxPolicy::default().allow_origin("http://127.0.0.1:51203");
            let author = sandbox_csp(&policy, "'none'");
            let host = sandbox_csp(&policy, "data:");
            assert_eq!(
                host,
                author.replacen("frame-src 'none'", "frame-src data:", 1)
            );
            for directive in [
                "script-src 'unsafe-inline' data: http://127.0.0.1:51203",
                "style-src 'unsafe-inline' data: http://127.0.0.1:51203",
                "img-src data: http://127.0.0.1:51203",
                "media-src data: http://127.0.0.1:51203",
                "font-src data: http://127.0.0.1:51203",
                "connect-src http://127.0.0.1:51203",
            ] {
                assert!(host.contains(directive));
                assert!(author.contains(directive));
            }
        }

        #[test]
        fn host_document_is_set_into_the_existing_about_blank_frame() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            set_socket_poll_timeout(&client_socket).unwrap();
            let mut server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            let wrapper =
                host_wrapper_document("<main>author</main>", 48, 32, &SandboxPolicy::default());
            let expected_wrapper = wrapper.clone();
            let server = thread::spawn(move || {
                let auto_attach = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected auto-attach request, got {other:?}"),
                };
                assert_eq!(
                    auto_attach,
                    json!({
                        "id": 1,
                        "method": "Target.setAutoAttach",
                        "params": {
                            "autoAttach": true,
                            "waitForDebuggerOnStart": true,
                            "flatten": true,
                            "filter": [
                                {"type": "iframe", "exclude": false},
                                {"exclude": true}
                            ]
                        },
                        "sessionId": "render-session"
                    })
                );
                server_socket
                    .send(Message::text(json!({"id": 1, "result": {}}).to_string()))
                    .unwrap();

                let get_tree = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected frame-tree request, got {other:?}"),
                };
                assert_eq!(
                    get_tree,
                    json!({
                        "id": 2,
                        "method": "Page.getFrameTree",
                        "params": {},
                        "sessionId": "render-session"
                    })
                );
                server_socket
                    .send(Message::text(
                        json!({
                            "id": 2,
                            "result": {"frameTree": {"frame": {"id": "main-frame"}}}
                        })
                        .to_string(),
                    ))
                    .unwrap();

                let set_content = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected document-content request, got {other:?}"),
                };
                assert_eq!(
                    set_content,
                    json!({
                        "id": 3,
                        "method": "Page.setDocumentContent",
                        "params": {"frameId": "main-frame", "html": expected_wrapper},
                        "sessionId": "render-session"
                    })
                );
                for event in [
                    json!({
                        "method": "Page.frameAttached",
                        "params": {"frameId": "provisional-frame", "parentFrameId": "main-frame"},
                        "sessionId": "render-session"
                    }),
                    json!({
                        "method": "Page.frameDetached",
                        "params": {"frameId": "provisional-frame", "reason": "swap"},
                        "sessionId": "render-session"
                    }),
                    json!({
                        "method": "Target.attachedToTarget",
                        "params": {
                            "sessionId": "author-session",
                            "targetInfo": {
                                "targetId": "author-frame",
                                "type": "iframe",
                                "url": "",
                                "attached": true
                            },
                            "waitingForDebugger": true
                        },
                        "sessionId": "render-session"
                    }),
                ] {
                    server_socket
                        .send(Message::text(event.to_string()))
                        .unwrap();
                }
                server_socket
                    .send(Message::text(json!({"id": 3, "result": {}}).to_string()))
                    .unwrap();

                for (id, method, params) in [
                    (4, "Runtime.enable", json!({})),
                    (5, "Page.enable", json!({})),
                    (6, "Log.enable", json!({})),
                    (
                        7,
                        "Fetch.enable",
                        json!({"patterns": [{"urlPattern": "*", "requestStage": "Request"}]}),
                    ),
                    (
                        8,
                        "Page.addScriptToEvaluateOnNewDocument",
                        json!({"source": deterministic_clock_script()}),
                    ),
                    (9, "Runtime.runIfWaitingForDebugger", json!({})),
                ] {
                    let command = match server_socket.read().unwrap() {
                        Message::Text(text) => {
                            serde_json::from_str::<Value>(text.as_ref()).unwrap()
                        }
                        other => panic!("expected child setup request, got {other:?}"),
                    };
                    assert_eq!(
                        command,
                        json!({
                            "id": id,
                            "method": method,
                            "params": params,
                            "sessionId": "author-session"
                        })
                    );
                    if method == "Runtime.runIfWaitingForDebugger" {
                        server_socket
                            .send(Message::text(
                                json!({
                                    "method": "Fetch.requestPaused",
                                    "params": {
                                        "requestId": "author-data-request",
                                        "request": {"url": "data:text/html,author"}
                                    },
                                    "sessionId": "author-session"
                                })
                                .to_string(),
                            ))
                            .unwrap();
                    }
                    server_socket
                        .send(Message::text(json!({"id": id, "result": {}}).to_string()))
                        .unwrap();
                }

                let continued = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected child data request continuation, got {other:?}"),
                };
                assert_eq!(
                    continued,
                    json!({
                        "id": 10,
                        "method": "Fetch.continueRequest",
                        "params": {"requestId": "author-data-request"},
                        "sessionId": "author-session"
                    })
                );
                server_socket
                    .send(Message::text(json!({"id": 10, "result": {}}).to_string()))
                    .unwrap();

                for event in [
                    json!({
                        "method": "Page.frameNavigated",
                        "params": {"frame": {
                            "id": "author-frame",
                            "parentId": "main-frame",
                            "url": "data:text/html"
                        }},
                        "sessionId": "author-session"
                    }),
                    json!({
                        "method": "Runtime.executionContextCreated",
                        "params": {"context": {
                            "id": 17,
                            "auxData": {"frameId": "author-frame", "isDefault": true}
                        }},
                        "sessionId": "author-session"
                    }),
                    json!({
                        "method": "Page.frameStoppedLoading",
                        "params": {"frameId": "author-frame"},
                        "sessionId": "author-session"
                    }),
                    json!({
                        "method": "Page.loadEventFired",
                        "params": {"timestamp": 2.0},
                        "sessionId": "author-session"
                    }),
                ] {
                    server_socket
                        .send(Message::text(event.to_string()))
                        .unwrap();
                }

                let isolated_world = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected isolated-world request, got {other:?}"),
                };
                assert_eq!(isolated_world["id"], 11);
                assert_eq!(isolated_world["method"], "Page.createIsolatedWorld");
                assert_eq!(isolated_world["params"]["frameId"], "author-frame");
                assert_eq!(isolated_world["params"]["grantUniveralAccess"], false);
                assert_eq!(isolated_world["sessionId"], "author-session");
                let nonce = isolated_world["params"]["worldName"]
                    .as_str()
                    .and_then(|name| name.strip_prefix("opentake-paint-fence-"))
                    .expect("paint-fence world carries its nonce")
                    .to_owned();
                assert_eq!(nonce.len(), 32);
                assert!(nonce.bytes().all(|byte| byte.is_ascii_hexdigit()));
                server_socket
                    .send(Message::text(
                        json!({"id": 11, "result": {"executionContextId": 23}}).to_string(),
                    ))
                    .unwrap();

                let install_fence = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected paint-fence install request, got {other:?}"),
                };
                assert_eq!(
                    install_fence,
                    json!({
                        "id": 12,
                        "method": "Runtime.evaluate",
                        "params": {
                            "expression": author_fence_install_expression(&nonce),
                            "contextId": 23,
                            "returnByValue": true
                        },
                        "sessionId": "author-session"
                    })
                );
                server_socket
                    .send(Message::text(
                        json!({
                            "id": 12,
                            "result": {"result": {"type": "boolean", "value": true}}
                        })
                        .to_string(),
                    ))
                    .unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            let installed = install_host_document(&mut cdp, "render-session", &wrapper).unwrap();
            assert_eq!(installed.author_session_id, "author-session");
            assert_eq!(installed.author_context_id, 17);
            assert_eq!(installed.author_fence.session_id, "author-session");
            assert_eq!(installed.author_fence.context_id, 23);
            assert_eq!(installed.author_fence.nonce.len(), 32);
            server.join().unwrap();
        }

        #[test]
        fn author_navigation_must_be_a_data_child_of_the_host_frame() {
            let navigation = |url: &str, parent: &str| {
                json!({
                    "method": "Page.frameNavigated",
                    "params": {"frame": {
                        "id": "author-frame",
                        "parentId": parent,
                        "url": url
                    }},
                    "sessionId": "author-session"
                })
            };
            validate_author_navigation(
                &navigation("data:text/html,author", "main-frame"),
                "author-frame",
                "main-frame",
            )
            .unwrap();
            for rejected in [
                navigation("about:blank", "main-frame"),
                navigation("http://127.0.0.1/author", "main-frame"),
                navigation("data:text/html,author", "other-main-frame"),
            ] {
                assert!(
                    validate_author_navigation(&rejected, "author-frame", "main-frame").is_err()
                );
            }
        }

        #[test]
        fn oopif_fetch_routes_loopback_policy_on_the_child_session() {
            fn assert_route(policy: SandboxPolicy, expected_method: &str, should_block: bool) {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
                let (server_stream, _) = listener.accept().unwrap();
                let client_socket = WebSocket::from_raw_socket(
                    MaybeTlsStream::Plain(client_stream),
                    Role::Client,
                    None,
                );
                let mut server_socket =
                    WebSocket::from_raw_socket(server_stream, Role::Server, None);
                let expected_method = expected_method.to_owned();
                let server = thread::spawn(move || {
                    let command = match server_socket.read().unwrap() {
                        Message::Text(text) => {
                            serde_json::from_str::<Value>(text.as_ref()).unwrap()
                        }
                        other => panic!("expected child Fetch decision, got {other:?}"),
                    };
                    assert_eq!(command["method"], expected_method);
                    assert_eq!(command["params"]["requestId"], "oopif-loopback");
                    assert_eq!(command["sessionId"], "author-session");
                });
                let mut cdp = Cdp::new(
                    client_socket,
                    policy,
                    MotionCancellationToken::new(),
                    Instant::now() + Duration::from_secs(1),
                );
                cdp.handle_event_or_queue(json!({
                    "method": "Fetch.requestPaused",
                    "params": {
                        "requestId": "oopif-loopback",
                        "request": {"url": "http://127.0.0.1:51203/pixel.svg"}
                    },
                    "sessionId": "author-session"
                }))
                .unwrap();
                assert_eq!(cdp.ensure_no_blocked_url().is_err(), should_block);
                server.join().unwrap();
            }

            assert_route(SandboxPolicy::default(), "Fetch.failRequest", true);
            assert_route(
                SandboxPolicy::default().allow_origin("http://127.0.0.1:51203"),
                "Fetch.continueRequest",
                false,
            );
        }

        #[test]
        fn external_guard_validation_and_crop_preserve_the_authors_legal_edge() {
            let guard = [90, 91, 92];
            let mut guarded = image::RgbaImage::from_pixel(3, 3, image::Rgba([90, 91, 92, 255]));
            for (x, y, pixel) in [
                (0, 0, [1, 2, 3, 255]),
                (1, 0, [4, 5, 6, 255]),
                (0, 1, [7, 8, 9, 255]),
                (1, 1, [10, 11, 12, 255]),
            ] {
                guarded.put_pixel(x, y, image::Rgba(pixel));
            }
            assert!(external_guard_matches(&guarded, 2, 2, guard));
            guarded.put_pixel(1, 1, image::Rgba([200, 201, 202, 255]));
            assert!(
                external_guard_matches(&guarded, 2, 2, guard),
                "the author's legal bottom-right pixel is content, not guard"
            );
            let mut wrong_guard = guarded.clone();
            wrong_guard.put_pixel(2, 1, image::Rgba([90, 91, 93, 255]));
            assert!(!external_guard_matches(&wrong_guard, 2, 2, guard));

            let cropped = crop_guarded_image(guarded, 2, 2, 0).unwrap();
            assert_eq!(cropped.dimensions(), (2, 2));
            assert_eq!(cropped.get_pixel(0, 0).0, [1, 2, 3, 255]);
            assert_eq!(cropped.get_pixel(1, 0).0, [4, 5, 6, 255]);
            assert_eq!(cropped.get_pixel(0, 1).0, [7, 8, 9, 255]);
            assert_eq!(cropped.get_pixel(1, 1).0, [200, 201, 202, 255]);
        }

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
        fn browser_contents_are_resized_before_device_metrics() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client_stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server_stream, _) = listener.accept().unwrap();
            let client_socket = WebSocket::from_raw_socket(
                MaybeTlsStream::Plain(client_stream),
                Role::Client,
                None,
            );
            let mut server_socket = WebSocket::from_raw_socket(server_stream, Role::Server, None);
            let (observed_sender, observed_receiver) = mpsc::channel();
            let server = thread::spawn(move || {
                let mut observed = Vec::new();
                loop {
                    let request = match server_socket.read().unwrap() {
                        Message::Text(text) => {
                            serde_json::from_str::<Value>(text.as_ref()).unwrap()
                        }
                        other => panic!("expected viewport-sizing request, got {other:?}"),
                    };
                    let id = request["id"].as_u64().unwrap();
                    let method = request["method"].as_str().unwrap().to_owned();
                    let result = if method == "Browser.getWindowForTarget" {
                        json!({"windowId": 42, "bounds": {}})
                    } else {
                        json!({})
                    };
                    server_socket
                        .send(Message::text(
                            json!({"id": id, "result": result}).to_string(),
                        ))
                        .unwrap();
                    observed.push(request);
                    if method == "Emulation.setDeviceMetricsOverride" {
                        break;
                    }
                }
                observed_sender.send(observed).unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            cdp.set_device_metrics("render-session", 48, 32).unwrap();
            server.join().unwrap();
            assert_eq!(
                observed_receiver.recv().unwrap(),
                vec![
                    json!({
                        "id": 1,
                        "method": "Browser.getWindowForTarget",
                        "params": {},
                        "sessionId": "render-session"
                    }),
                    json!({
                        "id": 2,
                        "method": "Browser.setContentsSize",
                        "params": {
                            "windowId": 42,
                            "width": 48,
                            "height": 32
                        }
                    }),
                    json!({
                        "id": 3,
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
                ]
            );
        }

        #[test]
        fn guarded_candidate_requires_transition_then_desired_generation() {
            fn guarded_png(author: [u8; 4], guard: [u8; 3]) -> String {
                let mut image = image::RgbaImage::from_pixel(
                    2,
                    2,
                    image::Rgba([guard[0], guard[1], guard[2], 255]),
                );
                image.put_pixel(0, 0, image::Rgba(author));
                base64::engine::general_purpose::STANDARD
                    .encode(encode_viewport_png(image, 0).unwrap())
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
            let seed = [0, 0, 90];
            let transition = [0, 0, 165];
            let desired = [0, 0, 0];
            let wrong = guarded_png([1, 2, 3, 255], [17, 18, 19]);
            let transition_frame = guarded_png([4, 5, 6, 255], transition);
            let desired_frame = guarded_png([128, 0, 0, 255], desired);
            let host_background = |rgb: [u8; 3]| {
                json!({
                    "expression": format!(
                        "(() => {{ const layer = document.getElementById('opentake-host-background'); if (!layer) return false; layer.style.backgroundColor = 'rgb({} {} {})'; return true; }})()",
                        rgb[0], rgb[1], rgb[2]
                    ),
                    "returnByValue": true
                })
            };
            let server = thread::spawn(move || {
                let read_json = |socket: &mut WebSocket<TcpStream>| match socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected CDP command, got {other:?}"),
                };
                let send_json = |socket: &mut WebSocket<TcpStream>, value: Value| {
                    socket.send(Message::text(value.to_string())).unwrap();
                };

                let attach = read_json(&mut server_socket);
                assert_eq!(attach["method"], "Target.attachToTarget");
                send_json(
                    &mut server_socket,
                    json!({"id": 1, "result": {"sessionId": "capture-session"}}),
                );
                for (id, method, params) in [
                    (2, "Page.enable", json!({})),
                    (
                        3,
                        "Emulation.setVirtualTimePolicy",
                        json!({"policy": "pause"}),
                    ),
                    (4, "Runtime.evaluate", host_background(seed)),
                    (
                        5,
                        "Page.startScreencast",
                        json!({
                            "format": "png",
                            "maxWidth": 2,
                            "maxHeight": 2,
                            "everyNthFrame": 1
                        }),
                    ),
                    (6, "Runtime.evaluate", host_background(transition)),
                ] {
                    let command = read_json(&mut server_socket);
                    assert_eq!(
                        command,
                        json!({
                            "id": id,
                            "method": method,
                            "params": params,
                            "sessionId": "capture-session"
                        })
                    );
                    let result = if method == "Runtime.evaluate" {
                        json!({"result": {"type": "boolean", "value": true}})
                    } else {
                        json!({})
                    };
                    send_json(&mut server_socket, json!({"id": id, "result": result}));
                }

                let transition_fence = read_json(&mut server_socket);
                assert_eq!(
                    transition_fence,
                    json!({
                        "id": 7,
                        "method": "Emulation.setVirtualTimePolicy",
                        "params": {
                            "policy": "advance",
                            "budget": 1,
                            "maxVirtualTimeTaskStarvationCount": 10_000
                        },
                        "sessionId": "capture-session"
                    })
                );
                send_json(&mut server_socket, json!({"id": 7, "result": {}}));
                send_json(
                    &mut server_socket,
                    json!({
                        "method": "Emulation.virtualTimeBudgetExpired",
                        "params": {},
                        "sessionId": "capture-session"
                    }),
                );

                for (id, data) in [(8, wrong), (9, transition_frame.clone())] {
                    send_json(
                        &mut server_socket,
                        json!({
                            "method": "Page.screencastFrame",
                            "params": {"data": data, "metadata": {}, "sessionId": 70 + id},
                            "sessionId": "capture-session"
                        }),
                    );
                    let ack = read_json(&mut server_socket);
                    assert_eq!(ack["id"], id);
                    assert_eq!(ack["method"], "Page.screencastFrameAck");
                    send_json(&mut server_socket, json!({"id": id, "result": {}}));
                }

                let desired_command = read_json(&mut server_socket);
                assert_eq!(
                    desired_command,
                    json!({
                        "id": 10,
                        "method": "Runtime.evaluate",
                        "params": host_background(desired),
                        "sessionId": "capture-session"
                    })
                );
                send_json(
                    &mut server_socket,
                    json!({"id": 10, "result": {"result": {"type": "boolean", "value": true}}}),
                );

                let desired_fence = read_json(&mut server_socket);
                assert_eq!(
                    desired_fence,
                    json!({
                        "id": 11,
                        "method": "Emulation.setVirtualTimePolicy",
                        "params": {
                            "policy": "advance",
                            "budget": 1,
                            "maxVirtualTimeTaskStarvationCount": 10_000
                        },
                        "sessionId": "capture-session"
                    })
                );
                send_json(&mut server_socket, json!({"id": 11, "result": {}}));
                send_json(
                    &mut server_socket,
                    json!({
                        "method": "Emulation.virtualTimeBudgetExpired",
                        "params": {},
                        "sessionId": "capture-session"
                    }),
                );

                for (id, data) in [(12, transition_frame), (13, desired_frame)] {
                    send_json(
                        &mut server_socket,
                        json!({
                            "method": "Page.screencastFrame",
                            "params": {"data": data, "metadata": {}, "sessionId": 70 + id},
                            "sessionId": "capture-session"
                        }),
                    );
                    let ack = read_json(&mut server_socket);
                    assert_eq!(ack["id"], id);
                    assert_eq!(ack["method"], "Page.screencastFrameAck");
                    send_json(&mut server_socket, json!({"id": id, "result": {}}));
                }

                for (id, method, params, session) in [
                    (
                        14,
                        "Page.stopScreencast",
                        json!({}),
                        Some("capture-session"),
                    ),
                    (
                        15,
                        "Target.detachFromTarget",
                        json!({"sessionId": "capture-session"}),
                        None,
                    ),
                ] {
                    let command = read_json(&mut server_socket);
                    assert_eq!(command["id"], id);
                    assert_eq!(command["method"], method);
                    assert_eq!(command["params"], params);
                    assert_eq!(command.get("sessionId").and_then(Value::as_str), session);
                    send_json(&mut server_socket, json!({"id": id, "result": {}}));
                }
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            let image = cdp
                .capture_isolated_viewport(
                    "target-id",
                    desired,
                    CapturePass {
                        frame: CaptureFrame {
                            width: 1,
                            height: 1,
                            index: 0,
                        },
                        background: "black",
                    },
                    None,
                )
                .unwrap();
            assert_eq!(image.dimensions(), (1, 1));
            assert_eq!(image.get_pixel(0, 0).0, [128, 0, 0, 255]);
            server.join().unwrap();
        }

        #[test]
        fn guarded_candidate_detaches_and_preserves_a_pre_start_error() {
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
                let read_json = |socket: &mut WebSocket<TcpStream>| match socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected CDP command, got {other:?}"),
                };
                let attach = read_json(&mut server_socket);
                assert_eq!(attach["method"], "Target.attachToTarget");
                server_socket
                    .send(Message::text(
                        json!({"id": 1, "result": {"sessionId": "capture-session"}}).to_string(),
                    ))
                    .unwrap();
                let enable = read_json(&mut server_socket);
                assert_eq!(enable["method"], "Page.enable");
                server_socket
                    .send(Message::text(
                        json!({"id": 2, "error": {"code": -1, "message": "setup-primary"}})
                            .to_string(),
                    ))
                    .unwrap();
                let detach = read_json(&mut server_socket);
                assert_eq!(detach["method"], "Target.detachFromTarget");
                assert_eq!(detach["params"]["sessionId"], "capture-session");
                server_socket
                    .send(Message::text(
                        json!({"id": 3, "error": {"code": -2, "message": "cleanup-secondary"}})
                            .to_string(),
                    ))
                    .unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            let error = cdp
                .capture_isolated_viewport(
                    "target-id",
                    [0, 0, 0],
                    CapturePass {
                        frame: CaptureFrame {
                            width: 1,
                            height: 1,
                            index: 0,
                        },
                        background: "black",
                    },
                    None,
                )
                .unwrap_err();
            assert!(error.to_string().contains("setup-primary"));
            assert!(!error.to_string().contains("cleanup-secondary"));
            server.join().unwrap();
        }

        #[test]
        fn guarded_candidate_stops_detaches_and_preserves_a_post_start_error() {
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
                let read_json = |socket: &mut WebSocket<TcpStream>| match socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected CDP command, got {other:?}"),
                };
                let attach = read_json(&mut server_socket);
                assert_eq!(attach["method"], "Target.attachToTarget");
                server_socket
                    .send(Message::text(
                        json!({"id": 1, "result": {"sessionId": "capture-session"}}).to_string(),
                    ))
                    .unwrap();
                for id in 2..=6 {
                    let command = read_json(&mut server_socket);
                    if id == 5 {
                        assert_eq!(command["method"], "Page.startScreencast");
                    }
                    let result = if command["method"] == "Runtime.evaluate" {
                        json!({"result": {"type": "boolean", "value": true}})
                    } else {
                        json!({})
                    };
                    server_socket
                        .send(Message::text(
                            json!({"id": id, "result": result}).to_string(),
                        ))
                        .unwrap();
                }
                let transition_fence = read_json(&mut server_socket);
                assert_eq!(transition_fence["method"], "Emulation.setVirtualTimePolicy");
                assert_eq!(transition_fence["params"]["policy"], "advance");
                server_socket
                    .send(Message::text(json!({"id": 7, "result": {}}).to_string()))
                    .unwrap();
                server_socket
                    .send(Message::text(
                        json!({
                            "method": "Emulation.virtualTimeBudgetExpired",
                            "params": {},
                            "sessionId": "capture-session"
                        })
                        .to_string(),
                    ))
                    .unwrap();
                server_socket
                    .send(Message::text(
                        json!({
                            "method": "Page.screencastFrame",
                            "params": {"data": "not-base64", "metadata": {}, "sessionId": 7},
                            "sessionId": "capture-session"
                        })
                        .to_string(),
                    ))
                    .unwrap();
                let ack = read_json(&mut server_socket);
                assert_eq!(ack["method"], "Page.screencastFrameAck");
                server_socket
                    .send(Message::text(json!({"id": 8, "result": {}}).to_string()))
                    .unwrap();
                let stop = read_json(&mut server_socket);
                assert_eq!(stop["method"], "Page.stopScreencast");
                server_socket
                    .send(Message::text(
                        json!({"id": 9, "error": {"code": -1, "message": "stop-secondary"}})
                            .to_string(),
                    ))
                    .unwrap();
                let detach = read_json(&mut server_socket);
                assert_eq!(detach["method"], "Target.detachFromTarget");
                server_socket
                    .send(Message::text(
                        json!({"id": 10, "error": {"code": -2, "message": "detach-secondary"}})
                            .to_string(),
                    ))
                    .unwrap();
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            let error = cdp
                .capture_isolated_viewport(
                    "target-id",
                    [0, 0, 0],
                    CapturePass {
                        frame: CaptureFrame {
                            width: 1,
                            height: 1,
                            index: 0,
                        },
                        background: "black",
                    },
                    None,
                )
                .unwrap_err();
            assert!(error.to_string().contains("malformed screencast data"));
            assert!(!error.to_string().contains("stop-secondary"));
            assert!(!error.to_string().contains("detach-secondary"));
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
        fn author_fenced_reconstruction_rejects_a_stale_oopif_surface() {
            let markers = author_marker_plan("0123456789abcdef0123456789abcdef", 9);
            let marked = |pixel: [u8; 4], marker: AuthorMarker| {
                let mut image = image::RgbaImage::from_pixel(2, 2, image::Rgba(pixel));
                image.put_pixel(
                    marker.x,
                    marker.y,
                    image::Rgba([marker.rgb[0], marker.rgb[1], marker.rgb[2], 255]),
                );
                image
            };
            let stale = marked([9, 8, 7, 255], markers[0]);
            let current_a = marked([1, 2, 3, 255], markers[1]);
            let current_b = marked([1, 2, 3, 255], markers[2]);

            let error = reconcile_author_marker_images(
                [stale, current_a, current_b],
                markers,
                "opaque-white",
                0,
            )
            .expect_err("one stale OOPIF generation must fail closed");
            let MotionError::RenderFailed(message) = error else {
                panic!("expected render failure, got {error:?}");
            };
            assert!(message.contains("author-fenced pair diverged"));
            assert!(message.contains("differing_pixels=2"));
            assert!(!message.contains("0123456789abcdef"));
            assert!(!message.contains("[9, 8, 7"));
        }

        #[test]
        fn transparent_capture_uses_three_author_fenced_generations_per_background() {
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
            let encoded = |pixel: [u8; 4], guard: [u8; 3], marker: AuthorMarker| {
                let mut image = image::RgbaImage::from_pixel(
                    3,
                    3,
                    image::Rgba([guard[0], guard[1], guard[2], 255]),
                );
                for y in 0..2 {
                    for x in 0..2 {
                        image.put_pixel(x, y, image::Rgba(pixel));
                    }
                }
                image.put_pixel(
                    marker.x,
                    marker.y,
                    image::Rgba([marker.rgb[0], marker.rgb[1], marker.rgb[2], 255]),
                );
                base64::engine::general_purpose::STANDARD
                    .encode(encode_viewport_png(image, 0).unwrap())
            };
            let author_fence = test_author_fence();
            let server_fence = author_fence.clone();
            let server = thread::spawn(move || {
                let mut next_id = 1u64;
                let mut capture_index = 0usize;
                for (marker_generation, rgb, current) in [
                    (0_u64, [0, 0, 0], [128, 0, 0, 255]),
                    (1_u64, [255, 255, 255], [255, 127, 127, 255]),
                ] {
                    let markers = author_marker_plan(&server_fence.nonce, marker_generation);
                    let background = read_json(&mut server_socket);
                    assert_eq!(
                        background,
                        json!({
                            "id": next_id,
                            "method": "Runtime.evaluate",
                            "params": {
                                "expression": host_background_expression(rgb),
                                "returnByValue": true
                            },
                            "sessionId": "main-session"
                        })
                    );
                    send_json(
                        &mut server_socket,
                        json!({"id": next_id, "result": {"result": {"type": "boolean", "value": true}}}),
                    );
                    next_id += 1;

                    let mut previous_capture_session = None::<String>;
                    for marker in markers {
                        let capture_session = format!("capture-{capture_index}");
                        let seed = [capture_index as u8, 0, 90];
                        let transition = [capture_index as u8, 0, 165];
                        let transition_frame = encoded(current, transition, marker);
                        let wrong_guard_frame = transition_frame.clone();
                        let wrong_marker_frame = encoded(
                            current,
                            rgb,
                            AuthorMarker {
                                rgb: [marker.rgb[0] ^ 0xff, marker.rgb[1], marker.rgb[2]],
                                ..marker
                            },
                        );
                        let desired_frame = encoded(current, rgb, marker);
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
                            ("Emulation.setVirtualTimePolicy", json!({"policy": "pause"})),
                            (
                                "Runtime.evaluate",
                                json!({
                                    "expression": host_background_expression(seed),
                                    "returnByValue": true
                                }),
                            ),
                            (
                                "Page.startScreencast",
                                json!({
                                    "format": "png",
                                    "maxWidth": 3,
                                    "maxHeight": 3,
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
                            let result = if method == "Runtime.evaluate" {
                                json!({"result": {"type": "boolean", "value": true}})
                            } else {
                                json!({})
                            };
                            send_json(&mut server_socket, json!({"id": next_id, "result": result}));
                            next_id += 1;
                        }

                        let author_generation = read_json(&mut server_socket);
                        assert_eq!(
                            author_generation,
                            json!({
                                "id": next_id,
                                "method": "Runtime.evaluate",
                                "params": {
                                    "expression": author_marker_expression(&server_fence, marker),
                                    "contextId": server_fence.context_id,
                                    "returnByValue": true
                                },
                                "sessionId": server_fence.session_id
                            })
                        );
                        send_json(
                            &mut server_socket,
                            json!({"id": next_id, "result": {"result": {"type": "boolean", "value": true}}}),
                        );
                        next_id += 1;

                        let transition_command = read_json(&mut server_socket);
                        assert_eq!(
                            transition_command,
                            json!({
                                "id": next_id,
                                "method": "Runtime.evaluate",
                                "params": {
                                    "expression": host_background_expression(transition),
                                    "returnByValue": true
                                },
                                "sessionId": capture_session
                            })
                        );
                        send_json(
                            &mut server_socket,
                            json!({"id": next_id, "result": {"result": {"type": "boolean", "value": true}}}),
                        );
                        next_id += 1;

                        let transition_fence = read_json(&mut server_socket);
                        assert_eq!(
                            transition_fence,
                            json!({
                                "id": next_id,
                                "method": "Emulation.setVirtualTimePolicy",
                                "params": {
                                    "policy": "advance",
                                    "budget": 1,
                                    "maxVirtualTimeTaskStarvationCount": 10_000
                                },
                                "sessionId": capture_session
                            })
                        );
                        send_json(&mut server_socket, json!({"id": next_id, "result": {}}));
                        next_id += 1;
                        send_json(
                            &mut server_socket,
                            json!({
                                "method": "Emulation.virtualTimeBudgetExpired",
                                "params": {},
                                "sessionId": capture_session
                            }),
                        );

                        if let Some(prior) = previous_capture_session.as_deref() {
                            send_json(
                                &mut server_socket,
                                json!({
                                    "method": "Page.screencastFrame",
                                    "params": {"data": desired_frame.clone(), "metadata": {}, "sessionId": 6},
                                    "sessionId": prior
                                }),
                            );
                        }
                        send_json(
                            &mut server_socket,
                            json!({
                                "method": "Page.screencastFrame",
                                "params": {"data": transition_frame, "metadata": {}, "sessionId": 7},
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

                        let desired = read_json(&mut server_socket);
                        assert_eq!(
                            desired,
                            json!({
                                "id": next_id,
                                "method": "Runtime.evaluate",
                                "params": {
                                    "expression": host_background_expression(rgb),
                                    "returnByValue": true
                                },
                                "sessionId": capture_session
                            })
                        );
                        send_json(
                            &mut server_socket,
                            json!({"id": next_id, "result": {"result": {"type": "boolean", "value": true}}}),
                        );
                        next_id += 1;

                        let desired_fence = read_json(&mut server_socket);
                        assert_eq!(
                            desired_fence,
                            json!({
                                "id": next_id,
                                "method": "Emulation.setVirtualTimePolicy",
                                "params": {
                                    "policy": "advance",
                                    "budget": 1,
                                    "maxVirtualTimeTaskStarvationCount": 10_000
                                },
                                "sessionId": capture_session
                            })
                        );
                        send_json(&mut server_socket, json!({"id": next_id, "result": {}}));
                        next_id += 1;
                        send_json(
                            &mut server_socket,
                            json!({
                                "method": "Emulation.virtualTimeBudgetExpired",
                                "params": {},
                                "sessionId": capture_session
                            }),
                        );

                        send_json(
                            &mut server_socket,
                            json!({
                                "method": "Page.screencastFrame",
                                "params": {
                                    "data": wrong_guard_frame,
                                    "metadata": {},
                                    "sessionId": 8
                                },
                                "sessionId": capture_session
                            }),
                        );
                        let wrong_guard_ack = read_json(&mut server_socket);
                        assert_eq!(
                            wrong_guard_ack,
                            json!({
                                "id": next_id,
                                "method": "Page.screencastFrameAck",
                                "params": {"sessionId": 8},
                                "sessionId": capture_session
                            })
                        );
                        send_json(&mut server_socket, json!({"id": next_id, "result": {}}));
                        next_id += 1;

                        send_json(
                            &mut server_socket,
                            json!({
                                "method": "Page.screencastFrame",
                                "params": {
                                    "data": wrong_marker_frame,
                                    "metadata": {},
                                    "sessionId": 9
                                },
                                "sessionId": capture_session
                            }),
                        );
                        let wrong_marker_ack = read_json(&mut server_socket);
                        assert_eq!(
                            wrong_marker_ack,
                            json!({
                                "id": next_id,
                                "method": "Page.screencastFrameAck",
                                "params": {"sessionId": 9},
                                "sessionId": capture_session
                            })
                        );
                        send_json(&mut server_socket, json!({"id": next_id, "result": {}}));
                        next_id += 1;

                        send_json(
                            &mut server_socket,
                            json!({
                                "method": "Page.screencastFrame",
                                "params": {
                                    "data": desired_frame,
                                    "metadata": {},
                                    "sessionId": 10
                                },
                                "sessionId": capture_session
                            }),
                        );
                        let desired_ack = read_json(&mut server_socket);
                        assert_eq!(
                            desired_ack,
                            json!({
                                "id": next_id,
                                "method": "Page.screencastFrameAck",
                                "params": {"sessionId": 10},
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
                let clear = read_json(&mut server_socket);
                assert_eq!(
                    clear,
                    json!({
                        "id": next_id,
                        "method": "Runtime.evaluate",
                        "params": {
                            "expression": clear_author_marker_expression(&server_fence),
                            "contextId": server_fence.context_id,
                            "returnByValue": true
                        },
                        "sessionId": server_fence.session_id
                    })
                );
                send_json(
                    &mut server_socket,
                    json!({"id": next_id, "result": {"result": {"type": "boolean", "value": true}}}),
                );
                assert_eq!(capture_index, 6);
            });

            let mut cdp = Cdp::new(
                client_socket,
                SandboxPolicy::default(),
                MotionCancellationToken::new(),
                Instant::now() + Duration::from_secs(1),
            );
            let png = cdp
                .capture_frame_png(
                    "target-id",
                    "main-session",
                    &author_fence,
                    true,
                    CaptureFrame {
                        width: 2,
                        height: 2,
                        index: 0,
                    },
                )
                .unwrap();
            let recovered = image::load_from_memory(&png).unwrap().to_rgba8();
            assert_eq!(recovered.dimensions(), (2, 2));
            assert!(
                recovered.pixels().all(|pixel| pixel.0 == [255, 0, 0, 128]),
                "all marker positions must be reconstructed from two exact unobscured samples"
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
            let cancellation = MotionCancellationToken::new();
            assert!(matches!(
                publish_completed_render(
                    &cancellation,
                    Duration::from_secs(1),
                    Instant::now(),
                    &dir,
                    &mut partial
                ),
                Err(MotionError::Timeout(_))
            ));
            drop(partial);
            assert!(!cache.is_cached(&request));

            let cancelled_request =
                MotionRenderRequest::new(MotionSource::code("<cancelled/>"), 30, 2, 8, 8);
            let cancelled_dir = cache.begin_render(&cancelled_request).unwrap();
            complete_frames(&cancelled_dir);
            let mut cancelled_partial = PartialFrames::new(cancelled_dir.clone());
            let cancelled = MotionCancellationToken::new();
            cancelled.cancel();
            assert!(matches!(
                publish_completed_render(
                    &cancelled,
                    Duration::from_secs(1),
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
        fn target_close_rejects_a_false_success_result() {
            let (mut cdp, mut server_socket) = fake_cdp_pair();
            let server = thread::spawn(move || {
                let request = server_socket.read().unwrap();
                assert!(matches!(request, Message::Text(_)));
                server_socket
                    .send(Message::text(
                        json!({"id": 1, "result": {"success": false}}).to_string(),
                    ))
                    .unwrap();
            });

            assert!(matches!(
                cdp.close_target("render-target"),
                Err(MotionError::RenderFailed(message))
                    if message.contains("did not close the render target")
            ));
            server.join().unwrap();
        }

        #[test]
        fn render_browser_context_is_disposable_and_root_scoped() {
            let (mut cdp, mut server_socket) = fake_cdp_pair();
            let server = thread::spawn(move || {
                let create = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected createBrowserContext request, got {other:?}"),
                };
                assert_eq!(
                    create,
                    json!({
                        "id": 1,
                        "method": "Target.createBrowserContext",
                        "params": {"disposeOnDetach": true}
                    })
                );
                server_socket
                    .send(Message::text(
                        json!({"id": 1, "result": {"browserContextId": "context-1"}}).to_string(),
                    ))
                    .unwrap();

                let dispose = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected disposeBrowserContext request, got {other:?}"),
                };
                assert_eq!(
                    dispose,
                    json!({
                        "id": 2,
                        "method": "Target.disposeBrowserContext",
                        "params": {"browserContextId": "context-1"}
                    })
                );
                server_socket
                    .send(Message::text(json!({"id": 2, "result": {}}).to_string()))
                    .unwrap();
            });

            let context_id = cdp.create_browser_context().unwrap();
            assert_eq!(context_id, "context-1");
            cdp.dispose_browser_context(&context_id).unwrap();
            server.join().unwrap();
        }

        #[test]
        fn gpu_backend_trace_uses_root_session_and_bounds_safe_fields() {
            let (mut cdp, mut server_socket) = fake_cdp_pair();
            let server = thread::spawn(move || {
                let request = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected SystemInfo.getInfo request, got {other:?}"),
                };
                assert_eq!(
                    request,
                    json!({
                        "id": 1,
                        "method": "SystemInfo.getInfo",
                        "params": {}
                    }),
                    "GPU diagnostics must run on the root CDP session"
                );
                server_socket
                    .send(Message::text(
                        json!({
                            "id": 1,
                            "result": {
                                "gpu": {
                                    "devices": [{
                                        "vendorString": "Google Inc. (Google)",
                                        "deviceString": format!(
                                            "ANGLE SwiftShader\nforged-log-line {}",
                                            "x".repeat(160)
                                        )
                                    }],
                                    "auxAttributes": {
                                        "commandLine": "--api-key=must-not-appear"
                                    },
                                    "featureStatus": {
                                        "gpu_compositing": "disabled_software"
                                    }
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap();
            });

            let summary = cdp.gpu_backend_trace().unwrap();
            assert!(summary.starts_with("gpu backend class=swiftshader "));
            assert!(summary.contains("vendor=\"Google Inc. (Google)\""));
            assert!(summary.contains("gpu_compositing=\"disabled_software\""));
            assert!(!summary.contains("must-not-appear"));
            assert!(summary.contains("device=\"ANGLE SwiftShader?forged-log-line "));
            assert!(!summary.contains('\n'));
            assert!(summary.contains('…'));
            assert!(
                summary.chars().count() <= 320,
                "diagnostic summary must remain bounded: {summary}"
            );
            server.join().unwrap();
        }

        #[test]
        fn disabled_gpu_backend_trace_leaves_the_root_socket_untouched() {
            let (mut cdp, mut server_socket) = fake_cdp_pair();
            let server = thread::spawn(move || {
                let request = match server_socket.read().unwrap() {
                    Message::Text(text) => serde_json::from_str::<Value>(text.as_ref()).unwrap(),
                    other => panic!("expected Target.getTargets request, got {other:?}"),
                };
                assert_eq!(
                    request,
                    json!({
                        "id": 1,
                        "method": "Target.getTargets",
                        "params": {}
                    })
                );
                server_socket
                    .send(Message::text(json!({"id": 1, "result": {}}).to_string()))
                    .unwrap();
            });

            trace_gpu_backend_if_enabled(&mut cdp, false).unwrap();
            cdp.command("Target.getTargets", json!({}), None).unwrap();
            server.join().unwrap();
        }

        #[test]
        fn gpu_backend_trace_treats_only_the_diagnostic_rejection_as_unavailable() {
            let (mut cdp, mut server_socket) = fake_cdp_pair();
            let server = thread::spawn(move || {
                let request = server_socket.read().unwrap();
                assert!(matches!(request, Message::Text(_)));
                server_socket
                    .send(Message::text(
                        json!({
                            "id": 1,
                            "error": {
                                "code": -32601,
                                "message": "SystemInfo.getInfo was not found --secret"
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap();
            });

            assert_eq!(
                cdp.gpu_backend_trace().unwrap(),
                "gpu backend unavailable reason=command-rejected"
            );
            server.join().unwrap();

            let (mut cdp, mut server_socket) = fake_cdp_pair();
            let server = thread::spawn(move || {
                let request = server_socket.read().unwrap();
                assert!(matches!(request, Message::Text(_)));
                server_socket
                    .send(Message::text(
                        json!({
                            "method": "Inspector.targetCrashed",
                            "params": {"status": "crashed"}
                        })
                        .to_string(),
                    ))
                    .unwrap();
            });

            assert!(matches!(
                cdp.gpu_backend_trace(),
                Err(MotionError::RenderFailed(message))
                    if message == "Chromium render target crashed"
            ));
            server.join().unwrap();

            let (mut cdp, server_socket) = fake_cdp_pair();
            drop(server_socket);
            assert!(matches!(
                cdp.gpu_backend_trace(),
                Err(MotionError::RenderFailed(_))
            ));

            let (mut cdp, mut server_socket) = fake_cdp_pair();
            cdp.cancellation.cancel();
            let server = thread::spawn(move || {
                let request = server_socket.read().unwrap();
                assert!(matches!(request, Message::Text(_)));
            });
            assert!(matches!(
                cdp.gpu_backend_trace(),
                Err(MotionError::Cancelled)
            ));
            server.join().unwrap();
        }

        #[test]
        fn gpu_backend_trace_reports_incomplete_results_without_exposing_payloads() {
            let (mut cdp, mut server_socket) = fake_cdp_pair();
            let server = thread::spawn(move || {
                let request = server_socket.read().unwrap();
                assert!(matches!(request, Message::Text(_)));
                server_socket
                    .send(Message::text(
                        json!({
                            "id": 1,
                            "result": {
                                "gpu": {
                                    "auxAttributes": {
                                        "commandLine": "--password=must-not-appear"
                                    }
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap();
            });

            assert_eq!(
                cdp.gpu_backend_trace().unwrap(),
                "gpu backend unavailable reason=incomplete-result"
            );
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
        assert!(s.contains("document.getAnimations"));
        assert!(s.contains("animations[i].pause()"));
        assert!(s.contains("animations[i].currentTime = seconds * 1000"));
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

    #[cfg(feature = "chromium")]
    #[test]
    fn changing_browser_path_detaches_the_reusable_pool() {
        let tmp = tempfile::tempdir().unwrap();
        let renderer =
            HeadlessChromiumRenderer::new(MotionCache::new(tmp.path()), SandboxPolicy::default());
        let shared = renderer.clone();
        assert!(Arc::ptr_eq(&renderer.browser_pool, &shared.browser_pool));

        let changed = shared.with_browser_path("different-browser");
        assert!(!Arc::ptr_eq(&renderer.browser_pool, &changed.browser_pool));
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
