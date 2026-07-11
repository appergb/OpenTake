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
  clipVolumeAt,
  frameForSourceTime,
  isExternalSeekWhilePlaying,
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
import { isTauri, onPlaybackFrame } from "../../lib/api";
import {
  clearNativePlaybackPublication,
  getNativePlaybackPublication,
  nativePlaybackController,
  subscribeNativePlaybackPublication,
} from "./nativePlaybackSession";
import { rustEngineEnabled } from "./rustEngine";
import { resolveTimelinePlaybackRoute } from "./playbackRoute";

interface NativeFrameListenerSlot {
  registration: NativeFrameListenerRegistration | null;
  registrationGeneration: number;
  references: number;
  teardownGeneration: number;
}

interface NativeFrameListenerRegistration {
  generation: number;
  ready: Promise<() => void>;
}

interface NativeFrameListenerLease {
  ensureReady(): Promise<() => void>;
  release(): void;
}

let nativeFrameListener: NativeFrameListenerSlot | null = null;

// StrictMode performs setup → cleanup → setup in one turn. A deferred teardown
// lets the second setup reclaim the still-pending listen Promise; the generation
// checks also prevent a resolved old cleanup from unlistening a newer lease.
function ensureNativeFrameListenerRegistration(
  slot: NativeFrameListenerSlot,
): Promise<() => void> {
  if (slot.registration) return slot.registration.ready;

  const registration: NativeFrameListenerRegistration = {
    generation: ++slot.registrationGeneration,
    ready: onPlaybackFrame((event) => {
      nativePlaybackController.acceptFrame(event);
    }),
  };
  slot.registration = registration;
  void registration.ready.catch(() => {
    if (
      nativeFrameListener === slot &&
      slot.registration?.generation === registration.generation
    ) {
      slot.registration = null;
    }
  });
  return registration.ready;
}

function acquireNativeFrameListener(): NativeFrameListenerLease {
  if (!nativeFrameListener) {
    nativeFrameListener = {
      registration: null,
      registrationGeneration: 0,
      references: 0,
      teardownGeneration: 0,
    };
  }
  const slot = nativeFrameListener;
  slot.references += 1;
  slot.teardownGeneration += 1;
  ensureNativeFrameListenerRegistration(slot);
  let released = false;
  return {
    ensureReady: () => ensureNativeFrameListenerRegistration(slot),
    release() {
      if (released) return;
      released = true;
      slot.references = Math.max(0, slot.references - 1);
      const teardownGeneration = ++slot.teardownGeneration;
      queueMicrotask(() => {
        if (
          nativeFrameListener !== slot ||
          slot.references !== 0 ||
          slot.teardownGeneration !== teardownGeneration
        ) {
          return;
        }
        const registration = slot.registration;
        if (!registration) {
          nativeFrameListener = null;
          return;
        }
        void registration.ready.then(
          (unlisten) => {
            if (
              nativeFrameListener !== slot ||
              slot.references !== 0 ||
              slot.teardownGeneration !== teardownGeneration ||
              slot.registration?.generation !== registration.generation
            ) {
              return;
            }
            nativeFrameListener = null;
            slot.registration = null;
            unlisten();
          },
          () => {
            if (
              nativeFrameListener === slot &&
              slot.references === 0 &&
              slot.teardownGeneration === teardownGeneration
            ) {
              nativeFrameListener = null;
            }
          },
        );
      });
    },
  };
}

export async function startNativePlaybackAfterListener<T>(
  listenerReady: Promise<unknown>,
  start: () => Promise<T>,
): Promise<T> {
  await listenerReady;
  return start();
}

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

function setWebKitMediaPlayback(
  element: Pick<HTMLMediaElement, "currentTime" | "pause" | "paused" | "play">,
  playing: boolean,
  desiredTimeSec?: number,
): void {
  if (!playing) {
    element.pause();
    return;
  }
  if (!element.paused) return;
  if (
    desiredTimeSec !== undefined &&
    Math.abs(element.currentTime - desiredTimeSec) > 0.05
  ) {
    element.currentTime = desiredTimeSec;
  }
  void element.play().catch(() => {});
}

function pauseAll(): void {
  for (const el of elements.values()) setWebKitMediaPlayback(el, false);
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
  // Re-run the paused sync when the timeline itself changes (a clip added /
  // removed / swapped while paused). The pause-sync effect's other deps don't
  // change on an edit, so without this a just-dropped clip would hold its source
  // frame 0 instead of the playhead frame.
  const timelineVersion = useProjectStore((s) => s.timelineVersion);
  const projectEpoch = useProjectStore((s) => s.projectEpoch);
  const previousTransportState = useRef({ isPlaying: false, isScrubbing: false });
  const lastEngineFrameRef = useRef<number | null>(null);
  const activeNativeIdentityRef = useRef<ReturnType<
    typeof nativePlaybackController.currentIdentity
  >>(null);
  const nativeFrameListenerLeaseRef = useRef<NativeFrameListenerLease | null>(null);
  const engineFailed = useEditorUiStore((s) => s.rustEngineFailed);
  const setEngineFailed = useEditorUiStore((s) => s.setRustEngineFailed);

  useEffect(() => {
    const lease = acquireNativeFrameListener();
    nativeFrameListenerLeaseRef.current = lease;
    return () => {
      if (nativeFrameListenerLeaseRef.current === lease) {
        nativeFrameListenerLeaseRef.current = null;
      }
      lease.release();
    };
  }, []);

  useEffect(() => {
    const prev = previousTransportState.current;
    if (!isPlaying && !isScrubbing) {
      cancelPendingInteractiveSeek();
      pauseAll();
      const tl = useProjectStore.getState().timeline;
      const fps = tl.fps > 0 ? tl.fps : 30;
      const project = useProjectStore.getState();
      const nativeIdentity = activeNativeIdentityRef.current;
      const nativeDrovePreviousPlay =
        prev.isPlaying &&
        !engineFailed &&
        nativeIdentity !== null &&
        nativeIdentity.projectEpoch === project.projectEpoch &&
        nativeIdentity.timelineVersion === project.timelineVersion;
      if (prev.isPlaying && !nativeDrovePreviousPlay) {
        const visual = activeVideoForPausedSnap(tl, Math.max(0, Math.floor(activeFrame)));
        const el = visual ? previewElements.get(previewElementKey(visual)) : null;
        const pausedFrame = pausedPlayheadFrameFromFrozenVideo(visual, el?.currentTime ?? NaN, fps);
        if (pausedFrame !== null) useEditorUiStore.getState().setActiveFrame(pausedFrame);
      } else if (
        // A scrub just ended: settle every active element on the final frame.
        // Without this, a clip the scrub entered near the end can be left on its
        // source frame 0 ("track head") because its <video> mounted mid-scrub
        // and the throttled scrub seek never reached it.
        prev.isScrubbing ||
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
  }, [activeFrame, engineFailed, isPlaying, isScrubbing, projectEpoch, timelineVersion]);

  useEffect(() => {
    const timeline = useProjectStore.getState().timeline;
    const route = resolveTimelinePlaybackRoute(timeline, {
      rustAvailable: isTauri,
      rustEnabled: rustEngineEnabled() && !engineFailed,
    });
    if (route.kind === "unsupported") {
      cancelPendingInteractiveSeek();
      pauseAll();
      if (isPlaying) useEditorUiStore.getState().setPlaying(false);
      return;
    }
    if (route.kind === "rust" && isPlaying && !isScrubbing) {
      pauseAll();
      let disposed = false;
      let identity = activeNativeIdentityRef.current;
      const unsubscribePublication = subscribeNativePlaybackPublication(() => {
        const current = getNativePlaybackPublication();
        if (!current || disposed || !activeNativeIdentityRef.current) return;
        lastEngineFrameRef.current = current.frame;
        const ui = useEditorUiStore.getState();
        ui.setActiveFrame(current.frame);
      });

      const startFrame = Math.max(0, Math.floor(useEditorUiStore.getState().activeFrame));
      const listenerReady = nativeFrameListenerLeaseRef.current?.ensureReady();
      if (!listenerReady) {
        unsubscribePublication();
        setEngineFailed(true);
        return;
      }
      const start = startNativePlaybackAfterListener(listenerReady, () =>
        nativePlaybackController.start({ projectEpoch, timelineVersion }, startFrame, {
          onIdentity: (started) => {
            identity = started;
            activeNativeIdentityRef.current = started;
            lastEngineFrameRef.current = null;
          },
        }),
      );
      void start
        .then((started) => {
          if (disposed) {
            void nativePlaybackController.cleanup(started, "stop", startFrame);
            return;
          }
          identity = started;
          activeNativeIdentityRef.current = started;
        })
        .catch((error) => {
          if (disposed) return;
          if (nativePlaybackController.shouldFallback(error)) {
            clearNativePlaybackPublication();
            setEngineFailed(true);
          } else {
            useEditorUiStore.getState().setPlaying(false);
          }
        });

      return () => {
        disposed = true;
        unsubscribePublication();
        const current = identity ?? activeNativeIdentityRef.current;
        if (!current) return;
        const project = useProjectStore.getState();
        const ui = useEditorUiStore.getState();
        const sameRevision =
          current.projectEpoch === project.projectEpoch &&
          current.timelineVersion === project.timelineVersion;
        const action = sameRevision && !ui.rustEngineFailed ? "pause" : "stop";
        if (ui.isScrubbing) clearNativePlaybackPublication();
        void nativePlaybackController.cleanup(
          current,
          action,
          Math.max(0, Math.floor(ui.activeFrame)),
        );
      };
    }

    if (route.kind === "rust") {
      cancelPendingInteractiveSeek();
      pauseAll();
      return;
    }

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
        // Frame-aware gain: static volume x dB keyframe automation x fade ramp
        // (Clip::volume_at). The true gain can exceed 1 (boosted keyframes / a
        // >1 static volume) but HTMLMediaElement.volume is capped to [0,1] and
        // throws a RangeError above 1, so the clamp lives here at the
        // assignment, not inside the pure helper.
        // TODO(>0dB): route through a Web Audio GainNode to make >0 dB boosts audible.
        const gain = clipVolumeAt(m.track, m.clip, r);
        const isVisualVideo = visuals.some((visual) => visual.clip.id === m.clip.id);
        el.muted = gain <= 0 || (isVisualVideo && duplicatedVisualAudioRefs.has(m.clip.mediaRef));
        el.volume = Math.min(1, gain);
        const desired = sourceTimeSec(m.clip, f, fps);
        const previousClipId = lastClipByKey.get(key) ?? null;
        lastClipByKey.set(key, m.clip.id);
        if (el.paused) {
          setWebKitMediaPlayback(el, true, desired);
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
  }, [isPlaying, isScrubbing, engineFailed, projectEpoch, timelineVersion, setEngineFailed]);

  useEffect(() => {
    const route = resolveTimelinePlaybackRoute(useProjectStore.getState().timeline, {
      rustAvailable: isTauri,
      rustEnabled: rustEngineEnabled() && !engineFailed,
    });
    if (route.kind !== "rust" || !isPlaying || isScrubbing) {
      return;
    }
    const identity = activeNativeIdentityRef.current;
    if (
      identity &&
      isExternalSeekWhilePlaying({
        activeFrame,
        lastEngineFrame: lastEngineFrameRef.current,
      })
    ) {
      lastEngineFrameRef.current = Math.max(0, Math.floor(activeFrame));
      void nativePlaybackController.seek(identity, activeFrame);
    }
  }, [activeFrame, engineFailed, isPlaying, isScrubbing]);
}
