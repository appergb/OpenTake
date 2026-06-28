/**
 * Timeline playback engine (issue #142). The SINGLE clock + element registry for
 * timeline preview, mirroring upstream's app-level VideoEngine (the engine owns
 * playback; the view only renders — VideoEngine.swift / PreviewView.swift).
 *
 * One requestAnimationFrame loop is the only authority over the playhead. It
 * advances the playhead while PLAYING (audio element as master clock, dt
 * fallback through gaps) and live-seeks the source elements while SCRUBBING.
 * When paused, those same elements stay mounted and frozen on the current frame. The
 * old dual-clock arbitration (playbackClock refcount + usePlaybackTicker) is
 * gone — there is exactly one loop here.
 *
 * Surface model = the browser equivalent of upstream's exact / interactiveScrub
 * seek modes: PLAY, SCRUB, and PAUSE all use the same <video>/<audio> stack, so
 * pausing cannot change color management or sizing by swapping renderers.
 */

import { useEffect, useRef } from "react";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { totalFrames } from "../../lib/geometry";
import {
  activeAudioClips,
  activeVisualClips,
  advancePlayhead,
  clipVolume,
  frameForSourceTime,
  sourceTimeSec,
  type ActiveMedia,
} from "./timelinePlayback";
import {
  cancelInteractiveSeek,
  createInteractiveSeekQueue,
  enqueueInteractiveSeek,
  flushPendingInteractiveSeek,
  interactiveToleranceSec,
} from "./interactiveSeek";
import type { Timeline } from "../../lib/types";

// --- Shared element registry ---------------------------------------------
// playback key -> media element, written by <TimelinePlayback> ref callbacks and
// read by this engine loop. A DOM media element REMOVED from the tree keeps playing
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
const interactiveSeekQueue = createInteractiveSeekQueue();
let interactiveSeekTimer: ReturnType<typeof setTimeout> | null = null;

/** Active clips at `frame`: every visual layer, then every audio clip — the
 *  elements the engine drives. */
function activeAt(tl: Timeline, frame: number): ActiveMedia[] {
  const r = Math.max(0, Math.floor(frame));
  return [...activeVisualClips(tl, r), ...activeAudioClips(tl, r)];
}

export function previewElementKey(media: ActiveMedia): string {
  return `${media.trackIndex}:${media.track.id}:${media.clip.mediaRef}:${media.clip.mediaType}`;
}

export function activeVideoForPausedSnap(tl: Timeline, frame: number): ActiveMedia | null {
  const visuals = activeVisualClips(tl, frame);
  for (let i = visuals.length - 1; i >= 0; i--) {
    if (visuals[i].clip.mediaType === "video") return visuals[i];
  }
  return null;
}

export function shouldSyncPausedMediaToFrame(args: {
  isPlaying: boolean;
  isScrubbing: boolean;
  wasPlaying: boolean;
  wasScrubbing: boolean;
}): boolean {
  return (
    !args.isPlaying &&
    !args.isScrubbing &&
    !args.wasPlaying &&
    !args.wasScrubbing
  );
}

export function pausedSeekToleranceSec(fps: number, speed = 1): number {
  const safeFps = fps > 0 ? fps : 30;
  const safeSpeed = speed > 0 ? speed : 1;
  return (0.5 * safeSpeed) / safeFps + 0.002;
}

export function pausedPlayheadFrameFromFrozenVideo(
  media: ActiveMedia | null,
  currentTimeSec: number,
  fps: number,
): number | null {
  if (!media || media.clip.mediaType !== "video") return null;
  const frame = frameForSourceTime(media.clip, currentTimeSec, fps);
  return Number.isFinite(frame) ? Math.max(0, Math.floor(frame)) : null;
}

export function shouldSeekPlayingFollower(args: {
  previousClipId: string | null;
  currentClipId: string;
  currentTimeSec: number;
  desiredTimeSec: number;
  driftSec?: number;
}): boolean {
  if (args.previousClipId !== null && args.previousClipId !== args.currentClipId) return true;
  return Math.abs(args.currentTimeSec - args.desiredTimeSec) > (args.driftSec ?? DRIFT_SEC);
}

function pauseAll(): void {
  for (const el of elements.values()) el.pause();
}

function nowMs(): number {
  return typeof performance !== "undefined" ? performance.now() : Date.now();
}

function clearInteractiveSeekTimer(): void {
  if (interactiveSeekTimer === null) return;
  clearTimeout(interactiveSeekTimer);
  interactiveSeekTimer = null;
}

function cancelPendingInteractiveSeek(): void {
  clearInteractiveSeekTimer();
  cancelInteractiveSeek(interactiveSeekQueue);
}

function syncPausedTo(tl: Timeline, frame: number, fps: number): void {
  for (const m of activeAt(tl, frame)) {
    const el = previewElements.get(previewElementKey(m));
    if (!el) continue;
    if (!el.paused) el.pause();
    const desired = sourceTimeSec(m.clip, frame, fps);
    const tolerance = pausedSeekToleranceSec(fps, m.clip.speed);
    if (Math.abs(el.currentTime - desired) > tolerance) el.currentTime = desired;
  }
}

function performInteractiveSeek(tl: Timeline, frame: number, fps: number): void {
  for (const m of activeAt(tl, frame)) {
    const el = previewElements.get(previewElementKey(m));
    if (!el) continue; // images carry no media element
    el.muted = true;
    if (!el.paused) el.pause();
    const desired = sourceTimeSec(m.clip, frame, fps);
    if (Math.abs(el.currentTime - desired) > 0.01) el.currentTime = desired;
  }
}

function scheduleInteractiveSeekFlush(delayMs: number): void {
  if (interactiveSeekTimer !== null) return;
  interactiveSeekTimer = setTimeout(() => {
    interactiveSeekTimer = null;
    const ui = useEditorUiStore.getState();
    if (!ui.isScrubbing) {
      cancelInteractiveSeek(interactiveSeekQueue);
      return;
    }
    const tl = useProjectStore.getState().timeline;
    const fps = tl.fps > 0 ? tl.fps : 30;
    const pending = flushPendingInteractiveSeek(interactiveSeekQueue, nowMs());
    if (pending) performInteractiveSeek(tl, pending.frame, fps);
  }, delayMs);
}

/** Live scrub: pause every active element and seek it to its source frame so the
 *  preview tracks the drag (the cheap path the single-media preview already
 *  uses). Audio is silenced while scrubbing. */
function scrubTo(tl: Timeline, frame: number, fps: number): void {
  const scrubFrame = Math.max(0, Math.floor(frame));
  const request = {
    frame: scrubFrame,
    toleranceSec: interactiveToleranceSec(activeVisualClips(tl, scrubFrame).length),
  };
  const result = enqueueInteractiveSeek(interactiveSeekQueue, request, nowMs());
  if (result.kind === "flush") {
    performInteractiveSeek(tl, result.request.frame, fps);
  } else {
    scheduleInteractiveSeekFlush(result.delayMs);
  }
}

/**
 * The single timeline playback clock. Mount once (App). Runs only while playing
 * or scrubbing; otherwise every registered element is paused on its current
 * decoded frame.
 */
export function useTimelinePlaybackEngine(): void {
  const isPlaying = useEditorUiStore((s) => s.isPlaying);
  const isScrubbing = useEditorUiStore((s) => s.isScrubbing);
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const previousTransportState = useRef({ isPlaying: false, isScrubbing: false });

  useEffect(() => {
    const prev = previousTransportState.current;
    if (!isPlaying && !isScrubbing) {
      cancelPendingInteractiveSeek();
      pauseAll();
      const tl = useProjectStore.getState().timeline;
      const fps = tl.fps > 0 ? tl.fps : 30;
      if (prev.isPlaying) {
        const visual = activeVideoForPausedSnap(tl, Math.max(0, Math.floor(activeFrame)));
        const el = visual ? previewElements.get(previewElementKey(visual)) : null;
        const pausedFrame = pausedPlayheadFrameFromFrozenVideo(visual, el?.currentTime ?? NaN, fps);
        if (pausedFrame !== null) useEditorUiStore.getState().setActiveFrame(pausedFrame);
      } else if (
        shouldSyncPausedMediaToFrame({
          isPlaying,
          isScrubbing,
          wasPlaying: prev.isPlaying,
          wasScrubbing: prev.isScrubbing,
        })
      ) {
        syncPausedTo(tl, Math.max(0, Math.floor(activeFrame)), fps);
      }
    }
    previousTransportState.current = { isPlaying, isScrubbing };
  }, [activeFrame, isPlaying, isScrubbing]);

  useEffect(() => {
    if (!isPlaying && !isScrubbing) {
      cancelPendingInteractiveSeek();
      pauseAll();
      return;
    }

    let raf = 0;
    let lastTs: number | null = null;
    let lastSet: number | null = null;
    const lastClipByKey = new Map<string, string>();

    const syncFollowers = (tl: Timeline, f: number, fps: number) => {
      const r = Math.max(0, Math.floor(f));
      const visuals = activeVisualClips(tl, r);
      const auds = activeAudioClips(tl, r);
      const duplicatedVisualAudioRefs = new Set(auds.map((a) => a.clip.mediaRef));
      for (const m of activeAt(tl, f)) {
        const key = previewElementKey(m);
        const el = previewElements.get(key);
        if (!el) continue; // images carry no media element
        const vol = clipVolume(m.track, m.clip);
        const isVisualVideo = visuals.some((visual) => visual.clip.id === m.clip.id);
        el.muted = vol <= 0 || (isVisualVideo && duplicatedVisualAudioRefs.has(m.clip.mediaRef));
        el.volume = vol;
        const desired = sourceTimeSec(m.clip, f, fps);
        const previousClipId = lastClipByKey.get(key) ?? null;
        lastClipByKey.set(key, m.clip.id);
        if (el.paused) {
          if (Math.abs(el.currentTime - desired) > 0.05) el.currentTime = desired;
          el.play().catch(() => {});
        } else if (
          shouldSeekPlayingFollower({
            previousClipId,
            currentClipId: m.clip.id,
            currentTimeSec: el.currentTime,
            desiredTimeSec: desired,
          })
        ) {
          el.currentTime = desired;
        }
      }
    };

    const seekAll = (tl: Timeline, f: number, fps: number) => {
      for (const m of activeAt(tl, f)) {
        const el = previewElements.get(previewElementKey(m));
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
        scrubTo(tl, Math.max(0, Math.floor(ui.activeFrame)), fps);
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
        syncFollowers(tl, f, fps);
        lastSet = f;
        lastTs = ts;
        raf = requestAnimationFrame(tick);
        return;
      }

      const dt = lastTs !== null ? (ts - lastTs) / 1000 : 0;
      let next = advancePlayhead({ currentFrame: f, dtSec: dt, fps });

      if (next >= last) {
        ui.setCurrentFrame(last);
        ui.setPlaying(false);
        return; // stop: effect cleanup pauses the elements
      }
      if (next < 0) next = 0;
      ui.setActiveFrame(next);
      lastSet = next;
      lastTs = ts;
      syncFollowers(tl, next, fps);
      raf = requestAnimationFrame(tick);
    };

    if (isPlaying && !isScrubbing) {
      const tl = useProjectStore.getState().timeline;
      const fps = tl.fps > 0 ? tl.fps : 30;
      const f = useEditorUiStore.getState().activeFrame;
      // Resume from pause: do NOT force-seek every element. They are already
      // frozen on the resume frame; re-seeking flushes each <video>'s decode
      // buffer and causes sustained stutter after resume (timeline-only, with
      // many elements). syncFollowers re-seeks only on >0.05s drift, then plays —
      // so an already-correct element just resumes without a buffer flush.
      syncFollowers(tl, f, fps);
      lastSet = f;
    }

    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      cancelPendingInteractiveSeek();
      pauseAll();
    };
  }, [isPlaying, isScrubbing]);
}
