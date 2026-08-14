# Motion Studio Task 3 report

Status: COMPLETE — independently reviewed and approved before commit.

## Scope

- Added a typed `MotionDocumentSource` compiler for the project's real
  `index.html` and `styles.css`. It emits one self-contained document with the
  OpenTake seek contract and a fail-closed CSP, while rejecting author scripts,
  event handlers, navigation/resource attributes, embedded documents,
  filesystem URLs, CSS imports, and URL-bearing or executable CSS.
- Added structured, one-based Unicode line/column source diagnostics without
  returning project paths or renderer internals.
- Extended `MotionRenderRequest` with a backwards-compatible absolute
  `startFrame`; request validation and the versioned cache key include it, and
  Chromium renders the exact integer frame at `(startFrame + i) / fps`.
- Hardened the deterministic browser clock so every CSS/Web Animation is
  paused and pinned to the exact seek time, including animations created by a
  seek listener. This fixed a live Chromium readback race found by the new
  pixel-exact test.
- Added a bounded single-PNG reader: exactly one regular PNG frame, at most
  8 MiB before base64 expansion, with streaming growth detection and PNG magic
  validation.
- Added the typed `motion_preview` Tauri command. It captures project authority
  at IPC admission, reads the exact requested document revision, validates
  canvas/fps/duration/frame bounds, uses the shared production Chromium pool,
  checks project identity after capture, and returns only a bounded data URL,
  exact revision/frame, and sanitized diagnostics.
- Preview generations are explicit. A newer preview cancels older work without
  dropping its updater lease early; final publishing cannot overlap any active
  preview, and project/app lifecycle cancellation reaches every generation.
  Cancellation is rechecked after Chromium, around bounded PNG handling and
  after base64 encoding, so a superseded request cannot publish a late success.

## TDD evidence

Initial RED:

```text
cargo test -p opentake-motion preview_ --features chromium -- --nocapture
compile failed: MotionDocumentSource and MotionRenderRequest::with_start_frame were absent
```

The first production-clock attempt then exposed a real live-browser RED:

```text
preview_frame_is_deterministic_and_visibly_advances
failed: Chromium opaque-white-background author-fenced pair diverged
```

Root cause: the virtual-time policy froze timers but did not pin compositor-run
CSS animations between fenced readbacks. `OpenTake.seek` now pauses and sets
every Web Animation's `currentTime` before and after seek callbacks.

Final fresh GREEN:

```text
cargo test -p opentake-motion preview_ --features chromium -- --nocapture
5 unit tests + 1 live Chromium test passed

cargo test -p opentake-tauri motion::tests --lib -- --nocapture
6 passed

cargo test -p opentake-motion -- --nocapture
62 library + 1 integration + 3 pipeline tests passed

cargo test -p opentake-tauri motion_documents:: --lib -- --nocapture
15 passed

cargo clippy -p opentake-motion -p opentake-tauri --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
passed
```

The live Chromium test removes the completed cache between two captures of the
same frame and compares decoded pixels exactly. It separately captures another
integer frame and requires a meaningful visible pixel difference. Chromium was
available locally; this test did not take the no-browser skip path.

## Review

The first independent review found two release-blocking issues. A superseded
preview could return a late success after Chromium finished, and Chromium
accepted a no-whitespace quoted-attribute payload such as
`<svg id="x"onload="...">`; the generated CSP then allowed its event handler.
The implementation now checks cancellation through response finalization,
recognizes quoted attribute adjacency, removes the redundant inline bridge, and
uses `script-src 'none'`. The deterministic runtime remains the pre-document
CDP injection, and its real Chromium pixel test remains green with scripts
disabled in the author document.

Final independent re-review verdict: **Spec PASS / Quality APPROVE**, with zero
critical, high, medium, or low findings. The reviewer reran the exact
quote-adjacent active-content regression, cancelled-response regression, live
Chromium deterministic preview, formatting, and scoped diff checks.

## Commit

Pending: `feat(motion): preview real HTML and CSS deterministically`.
