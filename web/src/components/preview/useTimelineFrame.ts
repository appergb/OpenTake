/**
 * Drives the timeline composite preview (#47). Fetches the GPU-composited frame
 * for `frame` from Rust (`composite_frame`) and returns its PNG data URL for the
 * Preview to paint.
 *
 * Requests self-coalesce (one in flight; the latest requested frame runs next),
 * and during playback a `minIntervalMs` gate caps the request RATE. Without the
 * cap, the rAF playhead fires ~30–60 composites/sec — each a fresh ffmpeg decode
 * + full PNG/base64 — which spikes memory and stutters far below realtime. The
 * cap (≈10 fps) bounds the churn until the streaming playback engine (#53) lands;
 * paused/scrub passes `minIntervalMs=0` for immediate response.
 *
 * `enabled` gates fetching (Timeline tab active, not single-media preview).
 * `refreshKey` forces a refetch when the document changes (pass the timeline
 * snapshot). Returns null outside Tauri and before the first frame resolves.
 */

import { useEffect, useRef, useState } from "react";
import { compositeFrame, isTauri } from "../../lib/api";

export function useTimelineFrame(
  frame: number,
  enabled: boolean,
  refreshKey: unknown,
  minIntervalMs = 0,
): string | null {
  const [dataUrl, setDataUrl] = useState<string | null>(null);
  const inFlight = useRef(false);
  const pending = useRef<number | null>(null);
  const lastStart = useRef(0);
  const timer = useRef<number | null>(null);
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;
  const intervalRef = useRef(minIntervalMs);
  intervalRef.current = minIntervalMs;

  // Stable runner/scheduler held in refs so the coalescing + rate-gate survive
  // re-renders (each render refreshes captured `enabled`/`interval` via refs).
  const run = useRef<(f: number) => void>(() => {});
  const schedule = useRef<(f: number) => void>(() => {});

  run.current = (f: number) => {
    inFlight.current = true;
    lastStart.current = performance.now();
    void compositeFrame(f)
      .then((res) => {
        if (res && enabledRef.current) setDataUrl(res.dataUrl);
      })
      .catch(() => {
        // A failed composite leaves the last good frame.
      })
      .finally(() => {
        inFlight.current = false;
        if (pending.current !== null) {
          const next = pending.current;
          pending.current = null;
          schedule.current(next);
        }
      });
  };

  schedule.current = (f: number) => {
    if (inFlight.current) {
      pending.current = f; // coalesce to the latest; run when the current resolves
      return;
    }
    const wait = Math.max(0, intervalRef.current - (performance.now() - lastStart.current));
    if (wait <= 0) {
      run.current(f);
      return;
    }
    pending.current = f;
    if (timer.current === null) {
      timer.current = window.setTimeout(() => {
        timer.current = null;
        const p = pending.current;
        pending.current = null;
        if (p !== null && enabledRef.current) run.current(p);
      }, wait);
    }
  };

  useEffect(() => {
    if (!enabled || !isTauri) {
      // Keep the last composited frame rather than clearing to null. Clearing
      // made the preview flash to a black placeholder on every play→pause switch
      // (enabled goes false during playback) until the next composite resolved.
      // The frame is refreshed when re-enabled (pause) / the timeline changes.
      return;
    }
    schedule.current(frame);
  }, [frame, enabled, refreshKey]);

  // Clear any pending timer on unmount.
  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
    },
    [],
  );

  return dataUrl;
}
