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
  requestCompositeStill,
  onTerminalFailure,
}: RustFrameBufferProps) {
  const [state, setState] = useState<RustFrameBufferState>(createRustFrameBufferState);
  const stateRef = useRef(state);
  const [composite, setComposite] = useState<{
    image: CompositeFrame;
    frame: PlaybackFrameEvent;
    loaded: boolean;
  } | null>(null);
  const failedTerminalKeys = useRef(new Set<string>());

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

  const onLoad = (slot: 0 | 1, src: string) => {
    const pendingFrame = stateRef.current.slots[slot].frame;
    const result = loadRustFrame(stateRef.current, slot, src);
    if (result.state !== stateRef.current) commit(result.state);
    if (pendingFrame?.terminal && result.effect === "terminal-promoted") {
      finishTransport(pendingFrame, result.effect);
    }
  };

  const onError = (slot: 0 | 1, src: string) => {
    const pendingFrame = stateRef.current.slots[slot].frame;
    const result = failRustFrame(stateRef.current, slot, src);
    if (result.state !== stateRef.current) commit(result.state);
    if (pendingFrame?.terminal && result.effect === "terminal-exhausted") {
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
      {state.slots.map((slot, index) => (
        <img
          key={index}
          data-rust-frame-slot={index}
          src={slot.src ?? undefined}
          alt=""
          draggable={false}
          onLoad={(event) =>
            onLoad(index as 0 | 1, rustFrameEventSource(event.currentTarget))
          }
          onError={(event) =>
            onError(index as 0 | 1, rustFrameEventSource(event.currentTarget))
          }
          style={{
            position: "absolute",
            inset: 0,
            width: "100%",
            height: "100%",
            objectFit: "contain",
            visibility: slot.visible && displayedIdentityIsCurrent ? "visible" : "hidden",
          }}
        />
      ))}
    </div>
  );
}
