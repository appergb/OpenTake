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
 * commits via ONE `setClipProperties` call on release. `setClipProperties`
 * round-trips through Tauri IPC and clones the whole Timeline for the undo
 * stack (see project CLAUDE.md), so calling it per pointermove would spam the
 * undo stack and add IPC latency to every frame of the drag — the same
 * reasoning KeyframesLaneRow already applies to `moveKeyframe`.
 */

import { useEffect, useRef, useState } from "react";
import { useEditorUiStore } from "../../store/uiStore";
import * as edit from "../../store/editActions";
import {
  moveTransformByDeltaWithSnap,
  resizeTransformFromCorner,
  rotateDeltaIntoLocalFrame,
  sampledTransform,
  type CenterSnap,
  type TransformResizeCorner,
} from "../../lib/clip";
import { SNAP, SPACE } from "../../lib/theme";
import { CENTER_GUIDE_COLOR } from "./previewLayerStyles";
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

export function TransformOverlay({
  clip,
  canvasPx,
  mediaAspect,
}: {
  clip: Clip;
  canvasPx: { width: number; height: number };
  mediaAspect: number | null;
}) {
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  // Live-sampled rest position (matches upstream `clip.transformAt(frame:)`) —
  // follows keyframed position/scale/rotation tracks the same way the actual
  // composited frame does, so the box always aligns with the rendered clip.
  const restTransform = sampledTransform(clip, activeFrame);
  const [dragTransform, setDragTransform] = useState<Transform | null>(null);
  // Per-axis canvas-center snap flags for the current move-drag (Item 3). Only a
  // move sets these; a resize/idle leaves them false, so the guides never show
  // outside a move that lands on center. Drives the pink guide lines below.
  const [dragSnap, setDragSnap] = useState<CenterSnap>({ x: false, y: false });
  const dragCleanupRef = useRef<(() => void) | null>(null);

  // Selection moved to a different clip mid-drag (e.g. clicked elsewhere) —
  // don't let a stale local preview from the PREVIOUS clip leak onto this one.
  useEffect(() => {
    setDragTransform(null);
    setDragSnap({ x: false, y: false });
  }, [clip.id]);

  // Unmount safety: remove any active drag's window listeners.
  useEffect(() => {
    return () => {
      dragCleanupRef.current?.();
      dragCleanupRef.current = null;
    };
  }, []);

  const display = dragTransform ?? restTransform;

  // Shared drag scaffolding: registers window pointermove/up, feeds each move's
  // pixel delta through `computeNext` for live local preview, and commits once
  // via setClipProperties on release. `computeNext` carries the move-vs-resize
  // math difference; the listener lifecycle is identical for both.
  const beginDrag = (
    e: React.PointerEvent,
    // Returns the next transform plus optional per-axis center-snap flags (only
    // the move drag reports snap; a resize returns undefined and the guides stay
    // hidden). Computed once per move so both states share one calculation.
    computeNext: (dxPx: number, dyPx: number) => { transform: Transform; snap?: CenterSnap },
  ) => {
    e.stopPropagation();
    e.preventDefault();
    const startClientX = e.clientX;
    const startClientY = e.clientY;
    const onMove = (ev: PointerEvent) => {
      const { transform, snap } = computeNext(ev.clientX - startClientX, ev.clientY - startClientY);
      setDragTransform(transform);
      if (snap) setDragSnap(snap);
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      dragCleanupRef.current = null;
      setDragSnap({ x: false, y: false });
      setDragTransform((cur) => {
        if (cur) void edit.setClipProperties([clip.id], { transform: cur });
        return null;
      });
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    dragCleanupRef.current = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  };

  const handleMoveDown = (e: React.PointerEvent) => {
    const start = restTransform;
    const rotated = start.rotation !== 0;
    // `moveTransformByDeltaWithSnap` returns the landed transform + the per-axis
    // center-snap flags (upstream `movedTransform`'s `(x, y)` return) — the box
    // preview uses the transform, the guides use the flags.
    beginDrag(e, (dx, dy) =>
      moveTransformByDeltaWithSnap(start, { width: dx, height: dy }, canvasPx, rotated, SNAP.thresholdPixels),
    );
  };

  const handleResizeDown = (e: React.PointerEvent, corner: TransformResizeCorner) => {
    const start = restTransform;
    const rotated = start.rotation !== 0;
    beginDrag(e, (dx, dy) => {
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
          rotated,
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
        <div
          onPointerDown={handleMoveDown}
          style={{
            position: "absolute",
            inset: 0,
            border: `${BORDER_WIDTH}px solid ${BORDER_COLOR}`,
            cursor: dragTransform ? "grabbing" : "move",
            pointerEvents: "auto",
          }}
        />
        {CORNERS.map((corner) => (
          <div
            key={corner}
            onPointerDown={(e) => handleResizeDown(e, corner)}
            style={{
              position: "absolute",
              left: CORNER_POSITION[corner].left,
              top: CORNER_POSITION[corner].top,
              width: HANDLE_SIZE,
              height: HANDLE_SIZE,
              marginLeft: -HANDLE_SIZE / 2,
              marginTop: -HANDLE_SIZE / 2,
              background: BORDER_COLOR,
              cursor: CORNER_CURSOR[corner],
              pointerEvents: "auto",
            }}
          />
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
