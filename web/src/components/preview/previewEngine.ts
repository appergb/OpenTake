/**
 * Timeline playback engine (issue #142). The SINGLE clock + element registry for
 * timeline preview, mirroring upstream's app-level VideoEngine (the engine owns
 * playback; the view only renders — VideoEngine.swift / PreviewView.swift).
 *
 * One requestAnimationFrame loop is the only authority over the playhead. It
 * advances the playhead while PLAYING (audio element as master clock, dt
 * fallback through gaps) and live-seeks the source elements while SCRUBBING; the
 * settled (paused, not scrubbing) frame is drawn by the Rust GPU composite. The
 * old dual-clock arbitration (playbackClock refcount + usePlaybackTicker) is
 * gone — there is exactly one loop here.
 *
 * Surface model = the browser equivalent of upstream's exact / interactiveScrub
 * seek modes: PLAY and SCRUB use the cheap real-time <video>/<audio> stack;
 * SETTLED uses the high-fidelity GPU composite. Full transform/crop/text
 * compositing DURING playback is the larger streaming engine (#53), tracked
 * separately; this engine is the faithful single-clock structure around what the
 * WebView can do today.
 */

import { useEffect } from "react";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { totalFrames } from "../../lib/geometry";
import {
  activeAudioClips,
  activeVisualClip,
  advancePlayhead,
  clipVolume,
  frameForSourceTime,
  sourceTimeSec,
  visualAudioIsDuplicated,
  type ActiveMedia,
} from "./timelinePlayback";
import type { Timeline } from "../../lib/types";

// --- Shared element registry ---------------------------------------------
// clipId -> media element, written by <TimelinePlayback> ref callbacks and read
// by this engine loop. A DOM media element REMOVED from the tree keeps playing
// (the browser does not auto-pause it), so the renderer pauses on detach via
// `remove` before dropping the entry.
const elements = new Map<string, HTMLMediaElement>();

export const previewElements = {
  set(id: string, el: HTMLMediaElement): void {
    elements.set(id, el);
  },
  remove(id: string): void {
    elements.get(id)?.pause();
    elements.delete(id);
  },
  get(id: string): HTMLMediaElement | null {
    return elements.get(id) ?? null;
  },
};

// --- Tuning (ported 1:1 from the previous in-component clock) -------------
/** Re-seek a follower only once its drift exceeds this (seconds) — small drifts
 *  are inaudible/invisible and self-correct at the next clip boundary. */
const DRIFT_SEC = 0.35;
/** A store `activeFrame` jump beyond this (frames) means an external seek while
 *  playing, so push the new position to the elements instead of reading them. */
const SEEK_EPSILON_FRAMES = 2;

/** Active clips at `frame`: the top visual clip first (if any), then every audio
 *  clip — the elements the engine drives. */
function activeAt(tl: Timeline, frame: number): ActiveMedia[] {
  const r = Math.round(frame);
  const v = activeVisualClip(tl, r);
  const list = activeAudioClips(tl, r);
  return v ? [v, ...list] : list;
}

/** Live scrub: pause every active element and seek it to its source frame so the
 *  preview tracks the drag (the cheap path the single-media preview already
 *  uses). Audio is silenced while scrubbing. */
function scrubTo(tl: Timeline, frame: number, fps: number): void {
  for (const m of activeAt(tl, frame)) {
    const el = previewElements.get(m.clip.id);
    if (!el) continue; // images carry no media element
    el.muted = true;
    if (!el.paused) el.pause();
    const desired = sourceTimeSec(m.clip, frame, fps);
    if (Math.abs(el.currentTime - desired) > 0.01) el.currentTime = desired;
  }
}

/**
 * The single timeline playback clock. Mount once (App). Runs only while playing
 * or scrubbing; otherwise every registered element is paused and the playhead is
 * owned by the settled GPU composite.
 */
export function useTimelinePlaybackEngine(): void {
  const isPlaying = useEditorUiStore((s) => s.isPlaying);
  const isScrubbing = useEditorUiStore((s) => s.isScrubbing);

  useEffect(() => {
    if (!isPlaying && !isScrubbing) {
      for (const el of elements.values()) el.pause();
      return;
    }

    let raf = 0;
    let lastTs: number | null = null;
    let lastSet: number | null = null;

    const pickMaster = (tl: Timeline, f: number): ActiveMedia | null => {
      const r = Math.round(f);
      for (const a of activeAudioClips(tl, r)) {
        const el = previewElements.get(a.clip.id);
        if (el && el.readyState >= 2 && !el.paused) return a;
      }
      const v = activeVisualClip(tl, r);
      if (v && v.clip.mediaType === "video") {
        const el = previewElements.get(v.clip.id);
        if (el && el.readyState >= 2 && !el.paused) return v;
      }
      return null;
    };

    const syncFollowers = (tl: Timeline, f: number, masterId: string | null, fps: number) => {
      const r = Math.round(f);
      const v = activeVisualClip(tl, r);
      const auds = activeAudioClips(tl, r);
      const dup = visualAudioIsDuplicated(v, auds);
      for (const m of activeAt(tl, f)) {
        const el = previewElements.get(m.clip.id);
        if (!el) continue; // images carry no media element
        const vol = clipVolume(m.track, m.clip);
        const isVisualVideo = v !== null && m.clip.id === v.clip.id;
        el.muted = vol <= 0 || (isVisualVideo && dup);
        el.volume = vol;
        const desired = sourceTimeSec(m.clip, f, fps);
        if (el.paused) {
          if (Math.abs(el.currentTime - desired) > 0.05) el.currentTime = desired;
          el.play().catch(() => {});
        } else if (m.clip.id !== masterId && Math.abs(el.currentTime - desired) > DRIFT_SEC) {
          el.currentTime = desired;
        }
      }
    };

    const seekAll = (tl: Timeline, f: number, fps: number) => {
      for (const m of activeAt(tl, f)) {
        const el = previewElements.get(m.clip.id);
        if (el) el.currentTime = sourceTimeSec(m.clip, f, fps);
      }
    };

    const tick = (ts: number) => {
      const ui = useEditorUiStore.getState();
      const tl = useProjectStore.getState().timeline;
      const fps = tl.fps > 0 ? tl.fps : 30;

      // SCRUB takes priority over play: live-seek to the scrub frame and never
      // advance the playhead (the user owns it during a drag).
      if (ui.isScrubbing) {
        scrubTo(tl, Math.round(ui.activeFrame), fps);
        lastTs = null;
        lastSet = null;
        raf = requestAnimationFrame(tick);
        return;
      }

      // A straggler tick can run after Pause flipped isPlaying=false (queued
      // before the effect cleanup cancelled it). Bail before writing the
      // playhead so it stays frozen at the pause frame — this is the fix for the
      // "pause jumps to a random frame / twitches" bug.
      if (!ui.isPlaying) return;

      const last = Math.max(0, totalFrames(tl) - 1);
      const f = ui.activeFrame;

      // External seek while playing (scrub-to-here, keyboard step): adopt it and
      // reposition the elements rather than reading the now-stale master.
      if (lastSet !== null && Math.abs(f - lastSet) > SEEK_EPSILON_FRAMES) {
        seekAll(tl, f, fps);
        syncFollowers(tl, f, null, fps);
        lastSet = f;
        lastTs = ts;
        raf = requestAnimationFrame(tick);
        return;
      }

      const master = pickMaster(tl, f);
      const dt = lastTs !== null ? (ts - lastTs) / 1000 : 0;
      const masterEl = master ? previewElements.get(master.clip.id) : null;
      const masterFrame =
        master && masterEl ? frameForSourceTime(master.clip, masterEl.currentTime, fps) : null;
      let next = advancePlayhead({ currentFrame: f, masterFrame, dtSec: dt, fps });
      // Advanced by dt despite having a master (it isn't aligned yet) → nudge the
      // element toward the playhead so it converges instead of fighting it.
      if (master && masterEl && masterFrame !== null && next !== masterFrame) {
        masterEl.currentTime = sourceTimeSec(master.clip, next, fps);
      }

      if (next >= last) {
        ui.setCurrentFrame(last);
        ui.setPlaying(false);
        return; // stop: effect cleanup pauses the elements
      }
      if (next < 0) next = 0;
      ui.setActiveFrame(next);
      lastSet = next;
      lastTs = ts;
      syncFollowers(tl, next, master?.clip.id ?? null, fps);
      raf = requestAnimationFrame(tick);
    };

    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      for (const el of elements.values()) el.pause();
    };
  }, [isPlaying, isScrubbing]);
}
