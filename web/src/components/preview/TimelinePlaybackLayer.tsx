/**
 * Real-time timeline playback (#53). While the timeline is PLAYING this mounts
 * the actual `<video>`/`<audio>` elements for the clips under the playhead and
 * plays them — smooth motion WITH sound — instead of the throttled GPU
 * composite-frame sequence used before (which played at ~11 fps and silent).
 *
 * This mirrors upstream's model (VideoEngine.swift): a single clock plays the
 * composition and a periodic observer drives the playhead. We can't GPU-
 * composite live in the WebView, so we play the source media directly and let
 * the GPU composite render the accurate frame (text / effects) when PAUSED.
 *
 * Clock: the active AUDIO element is the master (audio glitches are the most
 * audible, so we never re-seek it); the playhead is read from its `currentTime`.
 * With no audio the visual video is the master; with no media at all a dt-based
 * fallback advances the playhead through gaps. Followers are nudged back only
 * past a generous drift threshold so normal playback stays smooth.
 */

import { useEffect, useRef } from "react";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import { assetUrl } from "../../lib/asset";
import { totalFrames } from "../../lib/geometry";
import { mediaClock } from "./playbackClock";
import {
  activeAudioClips,
  activeVisualClip,
  clipOpacity,
  clipVolume,
  frameForSourceTime,
  sourceTimeSec,
  visualAudioIsDuplicated,
  type ActiveMedia,
} from "./timelinePlayback";
import type { Clip, Timeline } from "../../lib/types";

/** Re-seek a follower only once its drift exceeds this (seconds) — small drifts
 *  are inaudible/invisible and self-correct at the next clip boundary. */
const DRIFT_SEC = 0.35;
/** A store `activeFrame` jump beyond this (frames) means the user scrubbed while
 *  playing, so push the new position to the elements instead of reading them. */
const SEEK_EPSILON_FRAMES = 2;
/** If the master element's clock is this far from the playhead it isn't aligned
 *  yet (just mounted/seeked); advance by dt and nudge it rather than snapping. */
const MASTER_ALIGN_FRAMES = 15;

export function TimelinePlayback({ timeline, fps, playing }: { timeline: Timeline; fps: number; playing: boolean }) {
  // Subscribe to activeFrame so the right clips stay mounted as the playhead moves.
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const items = useMediaStore((s) => s.items);
  const frame = Math.round(activeFrame);

  const visual = activeVisualClip(timeline, frame);
  const audios = activeAudioClips(timeline, frame);

  const urlFor = (mediaRef: string): string | null =>
    assetUrl(items.find((m) => m.id === mediaRef)?.path);

  // clipId -> media element registry, read by the clock loop.
  const els = useRef<Map<string, HTMLMediaElement>>(new Map());
  // Stable ref callback per clip id (cached) so a clip's element isn't
  // detached/re-attached every rAF re-render — only the function identity
  // changing would do that, so we keep one callback per id.
  const cbCache = useRef<Map<string, (el: HTMLMediaElement | null) => void>>(new Map());
  const register = (id: string) => {
    let cb = cbCache.current.get(id);
    if (!cb) {
      cb = (el: HTMLMediaElement | null) => {
        if (el) {
          els.current.set(id, el);
        } else {
          // Detaching (clip left the active window, or the layer is
          // unmounting on pause). A media element REMOVED from the DOM keeps
          // playing — the browser does not auto-pause it — so we must pause it
          // here, while we still hold the reference. React detaches refs
          // (commit phase, synchronous) BEFORE the effect cleanup (passive,
          // async) runs, so by the time the cleanup loop runs `els` is already
          // empty; pausing on detach is what actually stops playback. Without
          // this, hitting Pause leaves the audio/video playing on.
          els.current.get(id)?.pause();
          els.current.delete(id);
        }
      };
      cbCache.current.set(id, cb);
    }
    return cb;
  };

  // Latest model in refs so the once-mounted clock reads current values.
  const tlRef = useRef(timeline);
  tlRef.current = timeline;
  const fpsRef = useRef(fps);
  fpsRef.current = fps;

  useEffect(() => {
    if (!playing) {
      // Paused: keep elements in DOM but silence the clock and media.
      mediaClock.release();

      for (const el of els.current.values()) el.pause();
      return;
    }
    mediaClock.claim();
    let raf = 0;
    let lastTs: number | null = null;
    let lastSet: number | null = null;

    const elFor = (id: string) => els.current.get(id) ?? null;

    const activeAt = (tl: Timeline, f: number): ActiveMedia[] => {
      const r = Math.round(f);
      const v = activeVisualClip(tl, r);
      const list = activeAudioClips(tl, r);
      return v ? [v, ...list] : list;
    };

    const pickMaster = (tl: Timeline, f: number): ActiveMedia | null => {
      const r = Math.round(f);
      for (const a of activeAudioClips(tl, r)) {
        const el = elFor(a.clip.id);
        if (el && el.readyState >= 2 && !el.paused) return a;
      }
      const v = activeVisualClip(tl, r);
      if (v && v.clip.mediaType === "video") {
        const el = elFor(v.clip.id);
        if (el && el.readyState >= 2 && !el.paused) return v;
      }
      return null;
    };

    const syncFollowers = (tl: Timeline, f: number, masterId: string | null) => {
      const safeFps = fpsRef.current > 0 ? fpsRef.current : 30;
      const r = Math.round(f);
      const v = activeVisualClip(tl, r);
      const auds = activeAudioClips(tl, r);
      const dup = visualAudioIsDuplicated(v, auds);
      for (const m of activeAt(tl, f)) {
        const el = elFor(m.clip.id);
        if (!el) continue; // images carry no media element
        const vol = clipVolume(m.track, m.clip);
        const isVisualVideo = v !== null && m.clip.id === v.clip.id;
        el.muted = vol <= 0 || (isVisualVideo && dup);
        el.volume = vol;
        const desired = sourceTimeSec(m.clip, f, safeFps);
        if (el.paused) {
          if (Math.abs(el.currentTime - desired) > 0.05) el.currentTime = desired;
          el.play().catch(() => {});
        } else if (m.clip.id !== masterId && Math.abs(el.currentTime - desired) > DRIFT_SEC) {
          el.currentTime = desired;
        }
      }
    };

    const seekAll = (tl: Timeline, f: number) => {
      const safeFps = fpsRef.current > 0 ? fpsRef.current : 30;
      for (const m of activeAt(tl, f)) {
        const el = elFor(m.clip.id);
        if (el) el.currentTime = sourceTimeSec(m.clip, f, safeFps);
      }
    };

    const tick = (ts: number) => {
      const ui = useEditorUiStore.getState();
      const tl = tlRef.current;
      const safeFps = fpsRef.current > 0 ? fpsRef.current : 30;
      const last = Math.max(0, totalFrames(tl) - 1);
      const f = ui.activeFrame;

      // External seek while playing: adopt it and reposition the elements.
      if (lastSet !== null && Math.abs(f - lastSet) > SEEK_EPSILON_FRAMES) {
        seekAll(tl, f);
        syncFollowers(tl, f, null);
        lastSet = f;
        lastTs = ts;
        raf = requestAnimationFrame(tick);
        return;
      }

      const master = pickMaster(tl, f);
      const dt = lastTs !== null ? (ts - lastTs) / 1000 : 0;
      let next: number;
      if (master) {
        const el = elFor(master.clip.id);
        const cand = el ? frameForSourceTime(master.clip, el.currentTime, safeFps) : f;
        // Guard against a just-mounted / just-seeked master whose currentTime
        // hasn't aligned yet (e.g. starting playback mid-timeline): don't snap
        // the playhead to it — advance by dt and nudge the element into place.
        if (Math.abs(cand - f) > MASTER_ALIGN_FRAMES) {
          next = f + dt * safeFps;
          if (el) el.currentTime = sourceTimeSec(master.clip, next, safeFps);
        } else {
          next = cand;
        }
      } else {
        next = f + dt * safeFps;
      }

      if (next >= last) {
        ui.setCurrentFrame(last);
        ui.setPlaying(false);
        return; // stop: cleanup pauses the elements
      }
      if (next < 0) next = 0;
      ui.setActiveFrame(next);
      lastSet = next;
      lastTs = ts;
      syncFollowers(tl, next, master?.clip.id ?? null);
      raf = requestAnimationFrame(tick);
    };

    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      mediaClock.release();
      for (const el of els.current.values()) el.pause();
    };
  }, [playing]);

  // Aspect-fit via intrinsic media size + max-width/height; the parent stage
  // flex-centers us. No absolute positioning (which would escape an unpositioned
  // ancestor and mis-place the frame).
  const fill: React.CSSProperties = {
    maxWidth: "100%",
    maxHeight: "100%",
    objectFit: "contain",
    display: "block",
  };

  const visualUrl = visual ? urlFor(visual.clip.mediaRef) : null;

  // Seek a freshly-mounted element to the right source position immediately, so
  // entering a clip (or starting playback mid-timeline) shows the correct frame
  // instead of the source's frame 0.
  const seekOnLoad = (clip: Clip) => (e: React.SyntheticEvent<HTMLMediaElement>) => {
    const f = Math.round(useEditorUiStore.getState().activeFrame);
    e.currentTarget.currentTime = sourceTimeSec(clip, f, fpsRef.current > 0 ? fpsRef.current : 30);
  };

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        overflow: "hidden",
      }}
    >
      {visual && visualUrl && visual.clip.mediaType === "video" && (
        <video
          key={visual.clip.id}
          ref={register(visual.clip.id)}
          src={visualUrl}
          playsInline
          preload="auto"
          onLoadedData={seekOnLoad(visual.clip)}
          style={{ ...fill, opacity: playing ? clipOpacity(visual.clip) : 0 }}
        />
      )}
      {visual && visualUrl && visual.clip.mediaType === "image" && (
        <img
          key={visual.clip.id}
          src={visualUrl}
          alt=""
          draggable={false}
          style={{ ...fill, opacity: playing ? clipOpacity(visual.clip) : 0 }}
        />
      )}
      {audios.map((a) => {
        const url = urlFor(a.clip.mediaRef);
        return url ? (
          <audio
            key={a.clip.id}
            ref={register(a.clip.id)}
            src={url}
            preload="auto"
            onLoadedData={seekOnLoad(a.clip)}
            style={{ display: "none" }}
          />
        ) : null;
      })}
    </div>
  );
}
