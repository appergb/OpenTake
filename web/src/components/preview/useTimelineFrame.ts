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
 * snapshot). Returns `{ url, frame }` where `frame` is the frame the current
 * `url` was rendered for — callers display the composite only once `frame`
 * matches the requested frame, so a stale composite never flashes during the
 * decode of a new one (issue #142). `url` is null outside Tauri / before the
 * first frame resolves.
 */

import { useEffect, useRef, useState } from "react";
import { compositeFrame, isTauri } from "../../lib/api";

export interface TimelineFrame {
  url: string | null;
  frame: number | null;
}

interface CommitTimelineFrameResultArgs {
  enabled: boolean;
  resultFrame: number;
  requestId: number;
  latestRequestId: number | null;
  latestRequestedFrame: number | null;
}

export function shouldCommitTimelineFrameResult({
  enabled,
  resultFrame,
  requestId,
  latestRequestId,
  latestRequestedFrame,
}: CommitTimelineFrameResultArgs): boolean {
  return enabled && requestId === latestRequestId && resultFrame === latestRequestedFrame;
}

export function useTimelineFrame(
  frame: number,
  enabled: boolean,
  refreshKey: unknown,
  minIntervalMs = 0,
): TimelineFrame {
  const [dataUrl, setDataUrl] = useState<string | null>(null);
  const [resolvedFrame, setResolvedFrame] = useState<number | null>(null);
  const inFlight = useRef(false);
  const pending = useRef<{ frame: number; requestId: number } | null>(null);
  const lastStart = useRef(0);
  const timer = useRef<number | null>(null);
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;
  const intervalRef = useRef(minIntervalMs);
  intervalRef.current = minIntervalMs;
  const requestSeq = useRef(0);
  const latestRequest = useRef<{ frame: number; requestId: number } | null>(null);

  // Stable runner/scheduler held in refs so the coalescing + rate-gate survive
  // re-renders (each render refreshes captured `enabled`/`interval` via refs).
  const run = useRef<(f: number, requestId: number) => void>(() => {});
  const schedule = useRef<(f: number) => void>(() => {});
  const scheduleRequest = useRef<(request: { frame: number; requestId: number }) => void>(() => {});

  run.current = (f: number, requestId: number) => {
    inFlight.current = true;
    lastStart.current = performance.now();
    void compositeFrame(f)
      .then((res) => {
        if (
          res &&
          shouldCommitTimelineFrameResult({
            enabled: enabledRef.current,
            resultFrame: f,
            requestId,
            latestRequestId: latestRequest.current?.requestId ?? null,
            latestRequestedFrame: latestRequest.current?.frame ?? null,
          })
        ) {
          setDataUrl(res.dataUrl);
          setResolvedFrame(f);
        }
      })
      .catch(() => {
        // A failed composite leaves the last good frame.
      })
      .finally(() => {
        inFlight.current = false;
        if (pending.current !== null) {
          const next = pending.current;
          pending.current = null;
          scheduleRequest.current(next);
        }
      });
  };

  schedule.current = (f: number) => {
    const request = { frame: f, requestId: ++requestSeq.current };
    latestRequest.current = request;
    scheduleRequest.current(request);
  };

  scheduleRequest.current = (request: { frame: number; requestId: number }) => {
    if (inFlight.current) {
      pending.current = request; // coalesce to the latest; run when the current resolves
      return;
    }
    const wait = Math.max(0, intervalRef.current - (performance.now() - lastStart.current));
    if (wait <= 0) {
      run.current(request.frame, request.requestId);
      return;
    }
    pending.current = request;
    if (timer.current === null) {
      timer.current = window.setTimeout(() => {
        timer.current = null;
        const p = pending.current;
        pending.current = null;
        if (p !== null && enabledRef.current) run.current(p.frame, p.requestId);
      }, wait);
    }
  };

  useEffect(() => {
    if (!enabled || !isTauri) {
      // Keep the last composited frame rather than clearing to null. Clearing
      // made the preview flash to a black placeholder on every play→pause switch
      // (enabled goes false during playback) until the next composite resolved.
      // The frame is refreshed when re-enabled (pause) / the timeline changes.
      latestRequest.current = null;
      requestSeq.current += 1;
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

  return { url: dataUrl, frame: resolvedFrame };
}
