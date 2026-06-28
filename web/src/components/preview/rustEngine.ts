/**
 * Runtime gate for the Rust streaming playback engine (#53).
 *
 * Off by default: PLAY keeps using the legacy single-rAF `<video>` path until the
 * Rust path (continuous decode → wgpu composite → MJPEG transport + cpal master
 * clock) is verified on a real machine. Flip it from the devtools console to A/B
 * the two paths without a rebuild:
 *
 *   localStorage.setItem('opentake.rustEngine', '1')  // enable
 *   localStorage.removeItem('opentake.rustEngine')    // back to <video>
 *
 * A follow-up makes it default-on (and adds a Settings toggle) once real-machine
 * playback is confirmed. Always false outside a browser/Tauri context.
 */
const FLAG_KEY = "opentake.rustEngine";

export function rustEngineEnabled(): boolean {
  try {
    return typeof localStorage !== "undefined" && localStorage.getItem(FLAG_KEY) === "1";
  } catch {
    // localStorage can throw in locked-down/private contexts — treat as off.
    return false;
  }
}
