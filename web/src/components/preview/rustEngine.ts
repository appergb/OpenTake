/**
 * Runtime gate for the Rust streaming playback engine (#53).
 *
 * DEFAULT-ON: PLAY (and its paused/scrub frames) go through the Rust path
 * (continuous decode → wgpu composite → WebSocket transport + cpal master clock).
 * Because compositing happens in wgpu, the preview shows the FULL GPU result —
 * color grade, chroma key, masks, shader effects — exactly like the export. The
 * legacy single-rAF `<video>` stack (DOM/CSS composite, no GPU effects) is the
 * ESCAPE HATCH: it takes over on explicit opt-out or when the engine can't start
 * (runtime watchdog in previewEngine.ts).
 *
 * (The macOS one-frame bug was the transport, not the engine: an `<img>` on a
 * multipart MJPEG stream, which WebKit only paints once. The transport is now a
 * WebSocket → canvas, which WebKit streams fine.)
 *
 * Opt OUT / force ON from the devtools console (no rebuild), to A/B the paths:
 *
 *   localStorage.setItem('opentake.rustEngine', '0')  // force legacy <video>
 *   localStorage.setItem('opentake.rustEngine', '1')  // force Rust engine
 *   localStorage.removeItem('opentake.rustEngine')     // back to default (ON)
 *
 * Only the exact string "0" opts out; anything else (a missing key, an unreadable
 * localStorage, or a stray value) resolves to the default-ON engine. Whether the
 * engine is actually USED still additionally requires a Tauri context (see
 * `shouldUseRustEngine`), so this stays inert in a plain browser shell.
 */
const FLAG_KEY = "opentake.rustEngine";

export function rustEngineEnabled(): boolean {
  try {
    if (typeof localStorage === "undefined") return true;
    // Explicit opt-out is the only value that disables the default path; a
    // missing key (null) or any other value keeps the engine on.
    return localStorage.getItem(FLAG_KEY) !== "0";
  } catch {
    // localStorage can throw in locked-down/private contexts. Inability to read
    // the opt-out flag must not silently disable the shipped default → treat as on.
    return true;
  }
}
