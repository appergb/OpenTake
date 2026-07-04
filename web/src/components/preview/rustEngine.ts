/**
 * Runtime gate for the Rust streaming playback engine (#53).
 *
 * DEFAULT-OFF: the shipped default is the legacy single-rAF `<video>` stack —
 * it composites the timeline in the DOM/CSS (transform, crop, fades, z-order),
 * is cross-platform (works in every WebView), and is the proven path that worked
 * before the streaming engine landed. The Rust path (continuous decode → wgpu
 * composite → WebSocket transport + cpal master clock) is now OPT-IN: its macOS
 * transport showed only one frame in WKWebView (the MJPEG `<img>` bug; now a WS
 * canvas, but not yet real-machine verified), so it stays behind an explicit flag
 * until it's confirmed on a real build.
 *
 * Force ON / off from the devtools console (no rebuild), to A/B the paths:
 *
 *   localStorage.setItem('opentake.rustEngine', '1')  // opt IN to Rust engine
 *   localStorage.setItem('opentake.rustEngine', '0')  // force legacy <video>
 *   localStorage.removeItem('opentake.rustEngine')     // back to default (legacy)
 *
 * Only the exact string "1" opts in; anything else (a missing key, an unreadable
 * localStorage, or a stray value) resolves to the default legacy path. Whether the
 * engine is actually USED still additionally requires a Tauri context (see
 * `shouldUseRustEngine`), so this stays inert in a plain browser shell.
 */
const FLAG_KEY = "opentake.rustEngine";

export function rustEngineEnabled(): boolean {
  try {
    if (typeof localStorage === "undefined") return false;
    // Opt-in only: the exact string "1" enables the Rust engine; a missing key
    // (null) or any other value keeps the proven legacy <video> path.
    return localStorage.getItem(FLAG_KEY) === "1";
  } catch {
    // localStorage can throw in locked-down/private contexts. Inability to read
    // the flag must not enable an unverified path → treat as the legacy default.
    return false;
  }
}
