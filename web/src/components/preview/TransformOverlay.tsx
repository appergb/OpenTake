/**
 * TransformOverlay (T3-10). On-canvas Transform manipulation for the single
 * selected visual clip: a bounding box with 4 corner resize handles and a
 * move-drag surface, mounted as an add-on layer over the composited preview
 * canvas. 1:1 port of upstream TransformOverlayView.swift's ACTUAL behavior —
 * see clip.ts's moveTransformByDelta/rotateDeltaIntoLocalFrame/sampledTransform
 * doc comments for exact upstream line references.
 *
 * Two things upstream's file does NOT have, despite being easy to assume from
 * a casual feature description: edge (midpoint) handles and a rotation handle.
 * It only has `ForEach(Corner.allCases)` — 4 corners, nothing else — and there
 * is zero rotation-gesture code (`atan2`, a rotation handle view, etc.) anywhere
 * in the upstream repo. Rotation is Inspector-only (see Inspector.tsx's
 * rotation ScrubbableNumberField, already existing, unrelated to this file).
 * Neither is invented here.
 *
 * Pointer drags use window pointermove/up listeners + a cleanup ref + an
 * unmount-safety effect (KeyframesLaneRow.tsx's drag pattern). Every move
 * updates only local state so the box tracks the cursor live; the actual clip
 * commits via ONE `setTransformAtFrame` call on release. `setTransformAtFrame`
 * round-trips through Tauri IPC and clones the whole Timeline for the undo
 * stack (see project CLAUDE.md), so calling it per pointermove would spam the
 * undo stack and add IPC latency to every frame of the drag — the same
 * reasoning KeyframesLaneRow already applies to `moveKeyframe`.
 */

import { useEffect, useRef, useState } from "react";
import { useEditorUiStore } from "../../store/uiStore";
import * as edit from "../../store/editActions";
import { useT } from "../../i18n";
import {
  moveTransformByDeltaWithSnap,
  moveTransformByDelta,
  resizeTransformFromCorner,
  rotateDeltaIntoLocalFrame,
  sampledTransform,
  type CenterSnap,
  type TransformResizeCorner,
} from "../../lib/clip";
import { SNAP, SPACE } from "../../lib/theme";
import { CENTER_GUIDE_COLOR } from "./previewLayerStyles";
import { playbackFrameFromActiveFrame } from "./timelinePlayback";
import type { Clip, Transform } from "../../lib/types";

/** AppTheme.Spacing.smMd (TransformOverlayView.swift:6). */
const HANDLE_SIZE = SPACE.smMd;
/** white @ AppTheme.Opacity.strong (TransformOverlayView.swift:7). */
const BORDER_COLOR = "rgba(255,255,255,0.55)";
/** AppTheme.BorderWidth.thin (TransformOverlayView.swift:31). */
const BORDER_WIDTH = 1;

const CORNERS: TransformResizeCorner[] = ["topLeft", "topRight", "bottomLeft", "bottomRight"];

const CORNER_POSITION: Record<TransformResizeCorner, { left: string; top: string }> = {
  topLeft: { left: "0%", top: "0%" },
  topRight: { left: "100%", top: "0%" },
  bottomLeft: { left: "0%", top: "100%" },
  bottomRight: { left: "100%", top: "100%" },
};

export const CORNER_CURSOR: Record<TransformResizeCorner, string> = {
  topLeft: "nwse-resize",
  bottomRight: "nwse-resize",
  topRight: "nesw-resize",
  bottomLeft: "nesw-resize",
};

const CORNER_LABEL_KEY: Record<TransformResizeCorner, string> = {
  topLeft: "preview.transform.resizeTopLeft",
  topRight: "preview.transform.resizeTopRight",
  bottomLeft: "preview.transform.resizeBottomLeft",
  bottomRight: "preview.transform.resizeBottomRight",
};

type TransformKeyboardTarget = "move" | TransformResizeCorner;

interface TransformKeyboardGesture {
  target: TransformKeyboardTarget;
  start: Transform;
  delta: { width: number; height: number };
  next: Transform;
  frame: number;
  context: edit.ProjectEditContext;
}

export function TransformOverlay({
  clip,
  canvasPx,
  mediaAspect,
}: {
  clip: Clip;
  canvasPx: { width: number; height: number };
  mediaAspect: number | null;
}) {
  const t = useT();
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const editFrame = playbackFrameFromActiveFrame(activeFrame);
  const pushToast = useEditorUiStore((s) => s.pushToast);
  // Live-sampled rest position (matches upstream `clip.transformAt(frame:)`) —
  // follows keyframed position/scale/rotation tracks the same way the actual
  // composited frame does, so the box always aligns with the rendered clip.
  const restTransform = sampledTransform(clip, editFrame);
  const [dragTransform, setDragTransform] = useState<Transform | null>(null);
  const [keyboardTransform, setKeyboardTransform] = useState<Transform | null>(null);
  // Per-axis canvas-center snap flags for the current move-drag (Item 3). Only a
  // move sets these; a resize/idle leaves them false, so the guides never show
  // outside a move that lands on center. Drives the pink guide lines below.
  const [dragSnap, setDragSnap] = useState<CenterSnap>({ x: false, y: false });
  const dragCleanupRef = useRef<(() => void) | null>(null);
  const dragTransformRef = useRef<Transform | null>(null);
  const keyboardGestureRef = useRef<TransformKeyboardGesture | null>(null);

  // Selection moved to a different clip mid-drag (e.g. clicked elsewhere) —
  // don't let a stale local preview from the PREVIOUS clip leak onto this one.
  useEffect(() => {
    dragCleanupRef.current?.();
    dragCleanupRef.current = null;
    dragTransformRef.current = null;
    keyboardGestureRef.current = null;
    setDragTransform(null);
    setKeyboardTransform(null);
    setDragSnap({ x: false, y: false });
  }, [clip.id, editFrame]);

  // Unmount safety: remove any active drag's window listeners.
  useEffect(() => {
    return () => {
      dragCleanupRef.current?.();
      dragCleanupRef.current = null;
      dragTransformRef.current = null;
      keyboardGestureRef.current = null;
    };
  }, []);

  const display = dragTransform ?? keyboardTransform ?? restTransform;
  const transformAnimated = [clip.positionTrack, clip.scaleTrack, clip.rotationTrack]
    .some((track) => !!track && track.keyframes.length > 0);
  const editable =
    !transformAnimated ||
    (editFrame >= clip.startFrame && editFrame < clip.startFrame + clip.durationFrames);

  const commitTransform = (
    next: Transform,
    frame: number,
    context: edit.ProjectEditContext,
  ) => {
    void edit.setTransformAtFrame(clip.id, frame, next, context).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      pushToast(t("preview.transformEditFailed", { error: message }));
    });
  };

  const finishKeyboardGesture = () => {
    const gesture = keyboardGestureRef.current;
    keyboardGestureRef.current = null;
    setKeyboardTransform(null);
    if (gesture) commitTransform(gesture.next, gesture.frame, gesture.context);
  };

  const updateKeyboardGesture = (
    target: TransformKeyboardTarget,
    delta: { width: number; height: number },
    compute: (start: Transform, total: { width: number; height: number }) => Transform,
  ) => {
    let gesture = keyboardGestureRef.current;
    if (!gesture) {
      gesture = {
        target,
        start: restTransform,
        delta: { width: 0, height: 0 },
        next: restTransform,
        frame: editFrame,
        context: edit.captureProjectEditContext(),
      };
    } else if (gesture.target !== target) {
      gesture = {
        ...gesture,
        target,
        start: gesture.next,
        delta: { width: 0, height: 0 },
      };
    }
    gesture.delta = {
      width: gesture.delta.width + delta.width,
      height: gesture.delta.height + delta.height,
    };
    gesture.next = compute(gesture.start, gesture.delta);
    keyboardGestureRef.current = gesture;
    setKeyboardTransform(gesture.next);
  };

  const handleKeyboardKeyUp = (e: React.KeyboardEvent) => {
    if (!keyboardDelta(e)) return;
    e.preventDefault();
    e.stopPropagation();
    finishKeyboardGesture();
  };

  const keyboardDelta = (e: React.KeyboardEvent): { width: number; height: number } | null => {
    const step = e.shiftKey ? 10 : 1;
    if (e.key === "ArrowLeft") return { width: -step, height: 0 };
    if (e.key === "ArrowRight") return { width: step, height: 0 };
    if (e.key === "ArrowUp") return { width: 0, height: -step };
    if (e.key === "ArrowDown") return { width: 0, height: step };
    return null;
  };

  const handleMoveKeyDown = (e: React.KeyboardEvent) => {
    if (!editable) return;
    const delta = keyboardDelta(e);
    if (!delta) return;
    e.preventDefault();
    e.stopPropagation();
    if (dragCleanupRef.current) return;
    updateKeyboardGesture("move", delta, (start, total) =>
      moveTransformByDelta(start, total, canvasPx, start.rotation !== 0, 0),
    );
  };

  const handleResizeKeyDown = (e: React.KeyboardEvent, corner: TransformResizeCorner) => {
    if (!editable) return;
    const delta = keyboardDelta(e);
    if (!delta) return;
    e.preventDefault();
    e.stopPropagation();
    if (dragCleanupRef.current) return;
    updateKeyboardGesture(corner, delta, (start, total) =>
      resizeTransformFromCorner(
        start,
        corner,
        rotateDeltaIntoLocalFrame(total, start.rotation),
        canvasPx,
        mediaAspect,
        start.rotation !== 0,
        0,
      ),
    );
  };

  // Shared drag scaffolding: registers window pointermove/up, feeds each move's
  // pixel delta through `computeNext` for live local preview, and commits once
  // via setTransformAtFrame on release. `computeNext` carries the move-vs-resize
  // math difference; the listener lifecycle is identical for both.
  const beginDrag = (
    e: React.PointerEvent,
    // Returns the next transform plus optional per-axis center-snap flags (only
    // the move drag reports snap; a resize returns undefined and the guides stay
    // hidden). Computed once per move so both states share one calculation.
    computeNext: (
      start: Transform,
      dxPx: number,
      dyPx: number,
    ) => { transform: Transform; snap?: CenterSnap },
  ) => {
    if (!editable) return;
    e.stopPropagation();
    e.preventDefault();
    dragCleanupRef.current?.();
    dragCleanupRef.current = null;
    const pendingKeyboard = keyboardGestureRef.current;
    keyboardGestureRef.current = null;
    setKeyboardTransform(null);
    const start = pendingKeyboard?.next ?? restTransform;
    const editContext = pendingKeyboard?.context ?? edit.captureProjectEditContext();
    const gestureFrame = pendingKeyboard?.frame ?? editFrame;
    dragTransformRef.current = pendingKeyboard ? start : null;
    setDragTransform(pendingKeyboard ? start : null);
    // Clear/coalesce the keyboard draft before focus can blur another handle;
    // that blur then observes no independent keyboard transaction to commit.
    (e.currentTarget as HTMLElement).focus();
    const startClientX = e.clientX;
    const startClientY = e.clientY;
    const onMove = (ev: PointerEvent) => {
      const { transform, snap } = computeNext(
        start,
        ev.clientX - startClientX,
        ev.clientY - startClientY,
      );
      dragTransformRef.current = transform;
      setDragTransform(transform);
      if (snap) setDragSnap(snap);
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      dragCleanupRef.current = null;
      setDragSnap({ x: false, y: false });
      const committed = dragTransformRef.current;
      dragTransformRef.current = null;
      setDragTransform(null);
      if (committed) {
        commitTransform(committed, gestureFrame, editContext);
      }
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    dragCleanupRef.current = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  };

  const handleMoveDown = (e: React.PointerEvent) => {
    // `moveTransformByDeltaWithSnap` returns the landed transform + the per-axis
    // center-snap flags (upstream `movedTransform`'s `(x, y)` return) — the box
    // preview uses the transform, the guides use the flags.
    beginDrag(e, (start, dx, dy) =>
      moveTransformByDeltaWithSnap(
        start,
        { width: dx, height: dy },
        canvasPx,
        start.rotation !== 0,
        SNAP.thresholdPixels,
      ),
    );
  };

  const handleResizeDown = (e: React.PointerEvent, corner: TransformResizeCorner) => {
    beginDrag(e, (start, dx, dy) => {
      // Corner handles rotate with the box on screen, so a raw screen-space
      // delta must be rotated into the box's own local frame first — see
      // rotateDeltaIntoLocalFrame's doc comment for why move doesn't need this.
      const local = rotateDeltaIntoLocalFrame({ width: dx, height: dy }, start.rotation);
      // A resize reports no center-snap (upstream draws the center guides only
      // for the move gesture), so `snap` is omitted.
      return {
        transform: resizeTransformFromCorner(
          start,
          corner,
          local,
          canvasPx,
          mediaAspect,
          start.rotation !== 0,
          SNAP.thresholdPixels,
        ),
      };
    });
  };

  if (
    !Number.isFinite(canvasPx.width) ||
    !Number.isFinite(canvasPx.height) ||
    canvasPx.width <= 0 ||
    canvasPx.height <= 0
  ) {
    return null;
  }

  // Guides are drawn (only while a move-drag snaps the clip center to the canvas
  // center) as SIBLINGS of the box, positioned over the whole canvas — they
  // belong to the canvas, not the rotated/translated clip box, so they can't be
  // the box's children (TransformOverlayView.swift:46-59). Shown only during a
  // drag: gate on `dragTransform` so an idle/resting selection never shows them.
  const dragging = dragTransform !== null;
  return (
    <>
      <div
        data-testid="transform-overlay"
        style={{
          position: "absolute",
          left: display.centerX * canvasPx.width,
          top: display.centerY * canvasPx.height,
          width: display.width * canvasPx.width,
          height: display.height * canvasPx.height,
          // translate first centers the (still-unrotated) box on the point,
          // then rotate turns it around its own center — same idiom already
          // used for keyframe diamonds (KeyframesLaneRow.tsx).
          transform: `translate(-50%, -50%) rotate(${display.rotation}deg)`,
          pointerEvents: "none",
          zIndex: 3,
        }}
      >
        {/* Move-drag surface + visual outline in one element (upstream's box
            border, TransformOverlayView.swift:30-31). */}
        <button
          type="button"
          data-transform-move-surface
          aria-label={t("preview.transform.move")}
          aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight"
          aria-disabled={!editable}
          disabled={!editable}
          onPointerDown={handleMoveDown}
          onKeyDown={handleMoveKeyDown}
          onKeyUp={handleKeyboardKeyUp}
          onBlur={finishKeyboardGesture}
          style={{
            position: "absolute",
            inset: 0,
            minWidth: 24,
            minHeight: 24,
            padding: 0,
            background: "transparent",
            border: `${BORDER_WIDTH}px solid ${BORDER_COLOR}`,
            cursor: !editable ? "not-allowed" : dragTransform ? "grabbing" : "move",
            pointerEvents: editable ? "auto" : "none",
          }}
          />
        {CORNERS.map((corner) => (
          <button
            type="button"
            key={corner}
            aria-label={t(CORNER_LABEL_KEY[corner])}
            aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight"
            aria-disabled={!editable}
            disabled={!editable}
            onPointerDown={(e) => handleResizeDown(e, corner)}
            onKeyDown={(e) => handleResizeKeyDown(e, corner)}
            onKeyUp={handleKeyboardKeyUp}
            onBlur={finishKeyboardGesture}
            style={{
              position: "absolute",
              left: CORNER_POSITION[corner].left,
              top: CORNER_POSITION[corner].top,
              width: 24,
              height: 24,
              marginLeft: -12,
              marginTop: -12,
              padding: 0,
              border: 0,
              background: "transparent",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              cursor: editable ? CORNER_CURSOR[corner] : "not-allowed",
              pointerEvents: editable ? "auto" : "none",
            }}
          >
            <span
              aria-hidden
              style={{
                width: HANDLE_SIZE,
                height: HANDLE_SIZE,
                background: BORDER_COLOR,
                pointerEvents: "none",
              }}
            />
          </button>
        ))}
      </div>

      {/* Pink center guide lines over the canvas center (TransformOverlayView
          .swift:46-59). Vertical line when the X center snaps, horizontal when
          the Y center snaps; each spans the full canvas. pointer-events:none. */}
      {dragging && dragSnap.x && (
        <div
          data-testid="transform-guide-x"
          style={{
            position: "absolute",
            left: canvasPx.width / 2,
            top: 0,
            width: 1,
            height: canvasPx.height,
            marginLeft: -0.5,
            background: CENTER_GUIDE_COLOR,
            pointerEvents: "none",
            zIndex: 4,
          }}
        />
      )}
      {dragging && dragSnap.y && (
        <div
          data-testid="transform-guide-y"
          style={{
            position: "absolute",
            left: 0,
            top: canvasPx.height / 2,
            width: canvasPx.width,
            height: 1,
            marginTop: -0.5,
            background: CENTER_GUIDE_COLOR,
            pointerEvents: "none",
            zIndex: 4,
          }}
        />
      )}
    </>
  );
}
