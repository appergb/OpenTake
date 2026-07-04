/**
 * Snap feedback (the "tick" when a clip edge / playhead snaps), 1:1 with
 * upstream's `NSHapticFeedbackManager.perform(.alignment)`. On macOS it fires a
 * real trackpad haptic via the `snap_haptic` Tauri command; other platforms
 * (no trackpad haptics) get a very short, quiet click sound instead. Deduped so
 * holding a snap doesn't buzz repeatedly — it fires once per fresh engagement.
 */

import { isTauri } from "./api";

const isMac =
  typeof navigator !== "undefined" &&
  /Mac/i.test(navigator.userAgent || (navigator as { platform?: string }).platform || "");

let lastSnapFrame: number | null = null;
let audioCtx: AudioContext | null = null;

async function performHaptic(): Promise<void> {
  if (!isTauri) return;
  try {
    const core = await import("@tauri-apps/api/core");
    await (core.invoke as <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>)(
      "snap_haptic",
    );
  } catch {
    // best-effort: a missing haptic must never disrupt the edit gesture
  }
}

function playTick(): void {
  try {
    const Ctx =
      window.AudioContext ||
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!Ctx) return;
    audioCtx ||= new Ctx();
    const ctx = audioCtx;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.frequency.value = 1800;
    gain.gain.setValueAtTime(0.0001, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.04, ctx.currentTime + 0.004);
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.03);
    osc.connect(gain).connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.035);
  } catch {
    // best-effort
  }
}

/** Fire snap feedback once when a NEW snap target engages. `snappedFrame` is the
 *  engaged target frame, or `null` when no snap is active (which re-arms the
 *  next engagement). macOS → trackpad haptic; other platforms → a short tick. */
export function maybeSnapFeedback(snappedFrame: number | null): void {
  if (snappedFrame === null) {
    lastSnapFrame = null;
    return;
  }
  if (snappedFrame === lastSnapFrame) return;
  lastSnapFrame = snappedFrame;
  if (isMac) void performHaptic();
  else playTick();
}
