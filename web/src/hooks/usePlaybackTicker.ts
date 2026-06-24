/**
 * Local playback ticker. While `isPlaying`, advances the playhead at the
 * timeline fps via requestAnimationFrame, stopping at the end. Real playback
 * (audio sync + composite frames) is a Rust concern (SPEC §11); this provides a
 * usable transport in v1.
 */

import { useEffect, useRef } from "react";
import { useEditorUiStore } from "../store/uiStore";
import { useProjectStore } from "../store/projectStore";
import { mediaClock } from "../components/preview/playbackClock";

export function usePlaybackTicker() {
  const isPlaying = useEditorUiStore((s) => s.isPlaying);
  const lastTsRef = useRef<number | null>(null);

  useEffect(() => {
    if (!isPlaying) {
      lastTsRef.current = null;
      return;
    }
    let raf = 0;
    const tick = (ts: number) => {
      // Yield while `<TimelinePlayback>` drives the playhead from real media
      // elements (audio/video) — this fallback only advances through gaps or
      // when the preview is unmounted. Keep looping so it resumes if released.
      if (mediaClock.active) {
        lastTsRef.current = null;
        raf = requestAnimationFrame(tick);
        return;
      }
      const ui = useEditorUiStore.getState();
      const tl = useProjectStore.getState().timeline;
      const fps = tl.fps > 0 ? tl.fps : 30;
      let total = 0;
      for (const t of tl.tracks)
        for (const c of t.clips) total = Math.max(total, c.startFrame + c.durationFrames);

      if (lastTsRef.current !== null) {
        const dtSec = (ts - lastTsRef.current) / 1000;
        const next = ui.activeFrame + dtSec * fps;
        // Clips are half-open [start, end): the last DRAWABLE frame is total-1.
        // Stopping at total over-shoots one frame past all content and the
        // composite goes black, so clamp the end to total-1.
        const last = Math.max(0, total - 1);
        if (next >= last) {
          ui.setCurrentFrame(last);
          ui.setPlaying(false);
          lastTsRef.current = null;
          return;
        }
        ui.setActiveFrame(next);
      }
      lastTsRef.current = ts;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [isPlaying]);
}
