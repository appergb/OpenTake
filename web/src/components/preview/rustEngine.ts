/**
 * Runtime gate for the Rust streaming playback engine (#53).
 *
 * Existing production callers remain default-ON until the capability route is
 * wired in Task 6.2. PLAY then goes through the Rust path (continuous decode →
 * wgpu composite → exact `/frame` JPEG transport + cpal master clock).
 * Because compositing happens in wgpu, the preview shows the FULL GPU result —
 * color grade, chroma key, masks, shader effects — exactly like the export. The
 * legacy single-rAF `<video>` stack (DOM/CSS composite, no GPU effects) is the
 * ESCAPE HATCH: it takes over on explicit opt-out or when the engine can't start
 * (runtime watchdog in previewEngine.ts).
 *
 * The macOS one-frame bug was the old multipart `<img>` transport. The active
 * path now reloads a session-scoped, cache-busted exact-frame endpoint.
 *
 * Opt OUT / force ON from the devtools console (no rebuild), to A/B the paths:
 *
 *   localStorage.setItem('opentake.rustEngine', '0')  // force legacy <video>
 *   localStorage.setItem('opentake.rustEngine', '1')  // force Rust engine
 *   localStorage.removeItem('opentake.rustEngine')     // back to default (ON)
 *
 * Exact "0" opts out and exact "1" opts in. Missing/stray values use the
 * caller's automatic default; existing no-argument callers remain default-ON
 * until the pure playback-route contract is wired in Task 6.2.
 */
const FLAG_KEY = "opentake.rustEngine";

export function rustEngineEnabled(autoDefault = true): boolean {
  try {
    if (typeof localStorage === "undefined") return autoDefault;
    const value = localStorage.getItem(FLAG_KEY);
    if (value === "0") return false;
    if (value === "1") return true;
    return autoDefault;
  } catch {
    return autoDefault;
  }
}
