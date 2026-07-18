import { useEffect, useRef, useState } from "react";
import type { CompositeFrame } from "../../lib/api";
import type { PlaybackFrameEvent, PlaybackIdentity } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";
import { nativePlaybackController, samePlaybackIdentity } from "./nativePlaybackSession";
import {
  createRustFrameBufferState,
  failRustFrame,
  loadRustFrame,
  releaseRustFrameAfterComposite,
  requestRustFrame,
  syncRustFrameBufferIdentity,
  type RustFrameBufferEffect,
  type RustFrameBufferState,
} from "./rustFrameBuffer";

export interface RustFrameBufferProps {
  event: PlaybackFrameEvent | null;
  endpoint: string | null;
  projectEpoch: number;
  timelineVersion: number;
  engineDriving: boolean;
  /** Frame to composite while native streaming is idle (paused/scrubbing). */
  stillFrame?: number | null;
  requestCompositeStill: (frame: number) => Promise<CompositeFrame | null>;
  onTerminalFailure: () => void;
}

export function afterPaint(callback: () => void): void {
  if (typeof requestAnimationFrame === "function") {
    requestAnimationFrame(() => requestAnimationFrame(() => callback()));
  } else {
    queueMicrotask(callback);
  }
}

export function rustFrameEventSource(target: Pick<HTMLImageElement, "currentSrc">): string {
  return target.currentSrc;
}

function paintLiveFrame(
  canvas: HTMLCanvasElement,
  image: HTMLImageElement,
  preserveCurrentFrame: boolean,
): boolean {
  if (image.naturalWidth <= 0 || image.naturalHeight <= 0) return false;
  try {
    const context = canvas.getContext("2d", { alpha: false });
    if (!context) return false;
    const sizeChanged =
      canvas.width !== image.naturalWidth || canvas.height !== image.naturalHeight;
    if (sizeChanged && preserveCurrentFrame) {
      const staging = document.createElement("canvas");
      staging.width = image.naturalWidth;
      staging.height = image.naturalHeight;
      const stagingContext = staging.getContext("2d", { alpha: false });
      if (!stagingContext) return false;
      stagingContext.drawImage(image, 0, 0);
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      context.drawImage(staging, 0, 0);
      return true;
    }
    if (sizeChanged) {
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
    }
    context.drawImage(image, 0, 0);
    return true;
  } catch {
    return false;
  }
}

function identityFor(frame: PlaybackFrameEvent): PlaybackIdentity {
  return {
    projectEpoch: frame.projectEpoch,
    timelineVersion: frame.timelineVersion,
    sessionId: frame.sessionId,
  };
}

export interface RustFrameBufferEffectDependencies {
  afterPaint: (callback: () => void) => void;
  isCurrentIdentity: (identity: PlaybackIdentity) => boolean;
  setPlaying: (playing: boolean) => void;
  stop: (identity: PlaybackIdentity) => Promise<void>;
  onTerminalFailure: () => void;
}

export function applyRustFrameBufferEffect(
  effect: RustFrameBufferEffect,
  frame: PlaybackFrameEvent,
  dependencies: RustFrameBufferEffectDependencies,
): void {
  const identity = identityFor(frame);
  if (effect === "terminal-exhausted") {
    if (!dependencies.isCurrentIdentity(identity)) return;
    dependencies.onTerminalFailure();
    dependencies.setPlaying(false);
    void dependencies.stop(identity).catch(() => undefined);
    return;
  }
  if (effect === "terminal-promoted") {
    dependencies.afterPaint(() => {
      if (!dependencies.isCurrentIdentity(identity)) return;
      const stopped = dependencies.stop(identity);
      dependencies.setPlaying(false);
      void stopped.catch(() => undefined);
    });
  }
}

export function RustFrameBuffer({
  event,
  endpoint,
  projectEpoch,
  timelineVersion,
  engineDriving,
  stillFrame = null,
  requestCompositeStill,
  onTerminalFailure,
}: RustFrameBufferProps) {
  const [state, setState] = useState<RustFrameBufferState>(createRustFrameBufferState);
  const stateRef = useRef(state);
  const liveCanvasRef = useRef<HTMLCanvasElement>(null);
  const [composite, setComposite] = useState<{
    image: CompositeFrame;
    frame: PlaybackFrameEvent;
    loaded: boolean;
  } | null>(null);
  const [idleComposite, setIdleComposite] = useState<CompositeFrame | null>(null);
  const idleQueueRef = useRef<{
    inFlight: boolean;
    pending: {
      frame: number;
      revision: string;
      generation: number;
      key: string;
      requester: RustFrameBufferProps["requestCompositeStill"];
    } | null;
    latestKey: string | null;
    revision: string;
    generation: number;
  }>({
    inFlight: false,
    pending: null,
    latestKey: null,
    revision: `${projectEpoch}:${timelineVersion}`,
    generation: 0,
  });
  const requestCompositeStillRef = useRef(requestCompositeStill);
  requestCompositeStillRef.current = requestCompositeStill;
  const idleRevisionRef = useRef(`${projectEpoch}:${timelineVersion}`);
  idleRevisionRef.current = `${projectEpoch}:${timelineVersion}`;
  const mountedRef = useRef(true);
  const pumpIdleCompositeRef = useRef<() => void>(() => undefined);
  const failedTerminalKeys = useRef(new Set<string>());

  pumpIdleCompositeRef.current = () => {
    const queue = idleQueueRef.current;
    if (queue.inFlight || !queue.pending) return;
    const job = queue.pending;
    queue.pending = null;
    queue.inFlight = true;
    void job.requester(job.frame)
      .then((image) => {
        const current = idleQueueRef.current;
        if (
          image &&
          mountedRef.current &&
          current.generation === job.generation &&
          current.latestKey === job.key &&
          idleRevisionRef.current === job.revision &&
          requestCompositeStillRef.current === job.requester
        ) {
          setIdleComposite(image);
        }
      })
      .catch(() => undefined)
      .finally(() => {
        const current = idleQueueRef.current;
        current.inFlight = false;
        pumpIdleCompositeRef.current();
      });
  };

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      const queue = idleQueueRef.current;
      queue.generation += 1;
      queue.pending = null;
      queue.latestKey = null;
    };
  }, []);

  const commit = (next: RustFrameBufferState) => {
    stateRef.current = next;
    setState(next);
  };

  const finishTransport = (frame: PlaybackFrameEvent, effect: RustFrameBufferEffect) => {
    const key = `${frame.projectEpoch}:${frame.timelineVersion}:${frame.sessionId}:${frame.sequence}`;
    if (effect === "terminal-exhausted") {
      if (failedTerminalKeys.current.has(key)) return;
      failedTerminalKeys.current.add(key);
    }
    applyRustFrameBufferEffect(effect, frame, {
      afterPaint,
      isCurrentIdentity: (identity) =>
        samePlaybackIdentity(nativePlaybackController.currentIdentity(), identity),
      setPlaying: (playing) => useEditorUiStore.getState().setPlaying(playing),
      stop: (identity) => nativePlaybackController.stop(identity),
      onTerminalFailure,
    });
  };

  useEffect(() => {
    const current = stateRef.current;
    if (
      current.identity &&
      (current.identity.projectEpoch !== projectEpoch ||
        current.identity.timelineVersion !== timelineVersion)
    ) {
      commit(
        syncRustFrameBufferIdentity(current, {
          projectEpoch,
          timelineVersion,
          sessionId: current.identity.sessionId,
        }),
      );
      setComposite(null);
    }
  }, [projectEpoch, timelineVersion]);

  useEffect(() => {
    const queue = idleQueueRef.current;
    const revision = `${projectEpoch}:${timelineVersion}`;
    if (queue.revision !== revision) {
      queue.revision = revision;
      queue.generation += 1;
      queue.pending = null;
      queue.latestKey = null;
      setIdleComposite(null);
    }
    if (engineDriving || stillFrame === null) {
      queue.generation += 1;
      queue.pending = null;
      queue.latestKey = null;
      // Keep the paused composite during native-engine startup. It is removed
      // only after the first live frame has actually loaded, preventing a
      // visible black flash at the route handoff.
      if (!engineDriving) setIdleComposite(null);
      return;
    }
    const key = `${revision}:${stillFrame}:${queue.generation}`;
    queue.latestKey = key;
    queue.pending = {
      frame: stillFrame,
      revision,
      generation: queue.generation,
      key,
      requester: requestCompositeStill,
    };
    pumpIdleCompositeRef.current();
  }, [engineDriving, projectEpoch, requestCompositeStill, stillFrame, timelineVersion]);

  useEffect(() => {
    if (!event) return;
    const previousIdentity = stateRef.current.identity;
    const result = requestRustFrame(stateRef.current, event, endpoint);
    if (
      previousIdentity &&
      (previousIdentity.projectEpoch !== event.projectEpoch ||
        previousIdentity.timelineVersion !== event.timelineVersion ||
        previousIdentity.sessionId !== event.sessionId)
    ) {
      setComposite(null);
    }
    if (result.state !== stateRef.current) commit(result.state);
  }, [endpoint, event]);

  const activeFrame = state.activeSlot === null ? null : state.slots[state.activeSlot].frame;
  useEffect(() => {
    if (engineDriving || !activeFrame?.terminal || composite?.frame.sequence === activeFrame.sequence) {
      return;
    }
    let disposed = false;
    void requestCompositeStill(activeFrame.frame)
      .then((image) => {
        if (!disposed && image) setComposite({ image, frame: activeFrame, loaded: false });
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
    };
  }, [activeFrame, composite?.frame.sequence, engineDriving, requestCompositeStill]);

  const rejectFrame = (slot: 0 | 1, src: string) => {
    const pendingFrame = stateRef.current.slots[slot].frame;
    const result = failRustFrame(stateRef.current, slot, src);
    if (result.state !== stateRef.current) commit(result.state);
    if (pendingFrame?.terminal && result.effect === "terminal-exhausted") {
      finishTransport(pendingFrame, result.effect);
    }
  };

  const onLoad = (slot: 0 | 1, target: HTMLImageElement) => {
    const src = rustFrameEventSource(target);
    const current = stateRef.current;
    if (current.pendingSlot !== slot || current.slots[slot].src !== src) return;
    const pendingFrame = current.slots[slot].frame;
    const canvas = liveCanvasRef.current;
    if (!canvas || !paintLiveFrame(canvas, target, current.activeSlot !== null)) {
      rejectFrame(slot, src);
      return;
    }
    const result = loadRustFrame(current, slot, src);
    if (result.state !== current) commit(result.state);
    setIdleComposite(null);
    if (pendingFrame?.terminal && result.effect === "terminal-promoted") {
      finishTransport(pendingFrame, result.effect);
    }
  };

  const displayedIdentityIsCurrent =
    state.identity === null ||
    (state.identity.projectEpoch === projectEpoch &&
      state.identity.timelineVersion === timelineVersion &&
      (!event ||
        (state.identity.projectEpoch === event.projectEpoch &&
          state.identity.timelineVersion === event.timelineVersion &&
          state.identity.sessionId === event.sessionId)));
  const compositeIsCurrent =
    composite !== null &&
    composite.frame.projectEpoch === projectEpoch &&
    composite.frame.timelineVersion === timelineVersion &&
    (!event || composite.frame.sessionId === event.sessionId);

  return (
    <div style={{ position: "absolute", inset: 0, pointerEvents: "none" }}>
      {composite && compositeIsCurrent && (
        <img
          data-testid="rust-composite-still"
          src={composite.image.dataUrl}
          alt=""
          onLoad={() => {
            const loaded = { ...composite, loaded: true };
            setComposite(loaded);
            commit(
              releaseRustFrameAfterComposite(stateRef.current, {
                projectEpoch: composite.frame.projectEpoch,
                timelineVersion: composite.frame.timelineVersion,
                sessionId: composite.frame.sessionId,
                frame: composite.frame.frame,
                engineDriving,
                compositeLoaded: true,
              }),
            );
          }}
          style={{ position: "absolute", inset: 0, width: "100%", height: "100%", objectFit: "contain" }}
        />
      )}
      <canvas
        ref={liveCanvasRef}
        data-testid="rust-live-canvas"
        aria-hidden="true"
        style={{
          position: "absolute",
          inset: 0,
          width: "100%",
          height: "100%",
          visibility:
            state.activeSlot !== null && displayedIdentityIsCurrent ? "visible" : "hidden",
        }}
      />
      {state.slots.map((slot, index) => (
        <img
          key={index}
          data-rust-frame-slot={index}
          src={slot.src ?? undefined}
          alt=""
          draggable={false}
          onLoad={(event) => onLoad(index as 0 | 1, event.currentTarget)}
          onError={(event) =>
            rejectFrame(index as 0 | 1, rustFrameEventSource(event.currentTarget))
          }
          style={{
            position: "absolute",
            width: 1,
            height: 1,
            visibility: "hidden",
          }}
        />
      ))}
      {idleComposite && (
        <img
          data-testid="rust-idle-composite-still"
          src={idleComposite.dataUrl}
          alt=""
          draggable={false}
          style={{
            position: "absolute",
            inset: 0,
            width: "100%",
            height: "100%",
            objectFit: "contain",
            zIndex: 2,
          }}
        />
      )}
    </div>
  );
}
