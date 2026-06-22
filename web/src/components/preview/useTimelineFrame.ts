/**
 * Drives the timeline composite preview (#47). Fetches the GPU-composited frame
 * for `frame` from Rust (`composite_frame`) and returns its PNG data URL for the
 * Preview to paint.
 *
 * Requests self-coalesce: only one is in flight at a time, and the latest
 * requested frame is fetched as soon as the previous resolves. This paces the
 * backend to its actual decode/composite throughput, so fast scrubbing never
 * floods it (the SPEC §8 `interactiveSeekInterval` intent, achieved by
 * back-pressure instead of a fixed timer).
 *
 * `enabled` gates fetching (Timeline tab active, not single-media preview).
 * `refreshKey` forces a refetch when the document changes (pass the timeline
 * snapshot — its identity changes on every `timeline_changed`). Returns null
 * outside Tauri and before the first frame resolves.
 */

import { useEffect, useRef, useState } from "react";
import { compositeFrame, isTauri } from "../../lib/api";

export function useTimelineFrame(
  frame: number,
  enabled: boolean,
  refreshKey: unknown,
): string | null {
  const [dataUrl, setDataUrl] = useState<string | null>(null);
  const inFlight = useRef(false);
  const pending = useRef<number | null>(null);
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;

  // A stable runner held in a ref so the coalescing logic survives re-renders
  // (each render refreshes the closure's captured `enabled` via `enabledRef`).
  const run = useRef<(f: number) => void>(() => {});
  run.current = (f: number) => {
    inFlight.current = true;
    void compositeFrame(f)
      .then((res) => {
        if (res && enabledRef.current) setDataUrl(res.dataUrl);
      })
      .catch(() => {
        // A failed composite (e.g. no GPU) just leaves the last good frame.
      })
      .finally(() => {
        inFlight.current = false;
        if (pending.current !== null) {
          const next = pending.current;
          pending.current = null;
          run.current(next);
        }
      });
  };

  useEffect(() => {
    if (!enabled || !isTauri) {
      setDataUrl(null);
      return;
    }
    if (inFlight.current) {
      pending.current = frame;
    } else {
      run.current(frame);
    }
  }, [frame, enabled, refreshKey]);

  return dataUrl;
}
