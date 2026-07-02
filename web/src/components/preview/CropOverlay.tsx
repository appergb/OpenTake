/**
 * CropOverlay (T3-11). On-canvas Crop manipulation for the single selected
 * visual clip: a crop rectangle with the outside region dimmed, rule-of-thirds
 * guide lines, 4 corner resize handles, and drag-inside-to-pan — 1:1 port of
 * upstream `CropOverlayView.swift`'s ACTUAL behavior. Mounted by `Preview.tsx`
 * in place of `TransformOverlay` while `cropEditingActive` is true
 * (`PreviewContainerView.swift:37-41`'s `if cropEditingActive { CropOverlayView() }
 * else { TransformOverlayView() }` — the two overlays are mutually exclusive,
 * never both on screen).
 *
 * Same architecture as the sibling `TransformOverlay.tsx`: a `pointerEvents:none`
 * container with interactive children only, window pointermove/up listeners +
 * a cleanup ref + an unmount-safety effect, local-optimistic drag preview, and
 * ONE commit on release (`applyCrop`/`commitCrop`'s upstream split —
 * CropOverlayView.swift:76-87,113-125 — collapses here to local state + one
 * commit, matching how `TransformOverlay` already collapses upstream's
 * apply/commit split for Transform).
 *
 * Pure geometry (pan/resize/aspect-lock math) lives in `../../lib/cropOverlay.ts`
 * so it's independently unit-tested; this file only wires pointer events, pixel
 * conversion, and the render tree.
 */

import { useEffect, useRef, useState } from "react";
import { useEditorUiStore } from "../../store/uiStore";
import * as edit from "../../store/editActions";
import { cropAt, rotateDeltaIntoLocalFrame, sampledTransform } from "../../lib/clip";
import {
  cropAspectLockPixelAspect,
  lockedAspectNormalized,
  pannedCrop,
  resizedCrop,
  type CropResizeCorner,
} from "../../lib/cropOverlay";
import { ACCENT, SPACE } from "../../lib/theme";
import type { Clip, Crop } from "../../lib/types";

/** AppTheme.Spacing.smMd (CropOverlayView.swift:6). */
const HANDLE_SIZE = SPACE.smMd;
/** AppTheme.Accent.timecodeColor (CropOverlayView.swift:7,9). */
const BORDER_COLOR = ACCENT.timecode;
/** black @ AppTheme.Opacity.strong (CropOverlayView.swift:8). */
const DIM_COLOR = "rgba(0,0,0,0.55)";
/** AppTheme.Accent.timecodeColor @ AppTheme.Opacity.medium (CropOverlayView.swift:9). */
const GUIDE_COLOR = "rgba(242,153,51,0.35)";
/** AppTheme.BorderWidth.thin (CropOverlayView.swift:37, thirds guides). */
const GUIDE_WIDTH = 1;
/** AppTheme.BorderWidth.medium (CropOverlayView.swift:38, crop rect border). */
const BORDER_WIDTH = 2;

const CORNERS: CropResizeCorner[] = ["topLeft", "topRight", "bottomLeft", "bottomRight"];

/** Corner handle position as a fraction (0 or 1) of the CROP rect's own
 *  width/height — resolved to px against `rect` (not the outer clip box) at
 *  render time, since the crop rect is a sub-region of the clip box
 *  (CropOverlayView.swift:247-254's `cornerPosition(_:in:)`, `in: cropRect`). */
const CORNER_FRACTION: Record<CropResizeCorner, { x: number; y: number }> = {
  topLeft: { x: 0, y: 0 },
  topRight: { x: 1, y: 0 },
  bottomLeft: { x: 0, y: 1 },
  bottomRight: { x: 1, y: 1 },
};

const CORNER_CURSOR: Record<CropResizeCorner, string> = {
  topLeft: "nwse-resize",
  bottomRight: "nwse-resize",
  topRight: "nesw-resize",
  bottomLeft: "nesw-resize",
};

function cropRectPx(crop: Crop, clipRectPx: { width: number; height: number }) {
  const visW = Math.max(0, 1 - crop.left - crop.right);
  const visH = Math.max(0, 1 - crop.top - crop.bottom);
  return {
    left: crop.left * clipRectPx.width,
    top: crop.top * clipRectPx.height,
    width: visW * clipRectPx.width,
    height: visH * clipRectPx.height,
  };
}

export function CropOverlay({
  clip,
  canvasPx,
  sourcePixelAspect,
}: {
  clip: Clip;
  canvasPx: { width: number; height: number };
  /** Raw source pixel aspect (sourceWidth / sourceHeight), distinct from the
   *  timeline-canvas-normalized `mediaCanvasAspect` — 1:1 with upstream
   *  `sourcePixelAspect(for:)` (CropOverlayView.swift:207-212). */
  sourcePixelAspect: number | null;
}) {
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const cropAspectLock = useEditorUiStore((s) => s.cropAspectLock);

  // Live-sampled rest transform/crop (matches upstream `clip.transformAt(frame:)`
  // / `clip.cropAt(frame:)`) — follows keyframed tracks so the overlay always
  // aligns with the rendered frame (CropOverlayView.swift:16-19).
  const restTransform = sampledTransform(clip, activeFrame);
  const restCrop = cropAt(clip, activeFrame);
  const [dragCrop, setDragCrop] = useState<Crop | null>(null);
  const dragCleanupRef = useRef<(() => void) | null>(null);

  const cropAnimated = !!clip.cropTrack && clip.cropTrack.keyframes.length > 0;

  useEffect(() => {
    setDragCrop(null);
  }, [clip.id]);

  useEffect(() => {
    return () => {
      dragCleanupRef.current?.();
      dragCleanupRef.current = null;
    };
  }, []);

  const display = dragCrop ?? restCrop;

  const commitCrop = (next: Crop) => {
    if (cropAnimated) {
      void edit.upsertKeyframe(clip.id, "crop", activeFrame, { kind: "crop", value: next });
    } else {
      void edit.setClipProperties([clip.id], { crop: next });
    }
  };

  // Shared drag scaffolding, mirroring `TransformOverlay`'s `beginDrag`:
  // registers window pointermove/up, feeds each move's pixel delta through
  // `computeNext` for live local preview, and commits once on release.
  const beginDrag = (e: React.PointerEvent, computeNext: (dxPx: number, dyPx: number) => Crop) => {
    e.stopPropagation();
    e.preventDefault();
    const startClientX = e.clientX;
    const startClientY = e.clientY;
    const onMove = (ev: PointerEvent) => {
      setDragCrop(computeNext(ev.clientX - startClientX, ev.clientY - startClientY));
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      dragCleanupRef.current = null;
      setDragCrop((cur) => {
        if (cur) commitCrop(cur);
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

  const clipRectPx = { width: restTransform.width * canvasPx.width, height: restTransform.height * canvasPx.height };

  const handlePanDown = (e: React.PointerEvent) => {
    const start = restCrop;
    beginDrag(e, (dx, dy) => {
      const local = rotateDeltaIntoLocalFrame({ width: dx, height: dy }, restTransform.rotation);
      return pannedCrop(start, local, clipRectPx);
    });
  };

  const handleResizeDown = (e: React.PointerEvent, corner: CropResizeCorner) => {
    const start = restCrop;
    const targetPixelAspect = cropAspectLockPixelAspect(cropAspectLock);
    const aspectN = lockedAspectNormalized(targetPixelAspect, sourcePixelAspect);
    beginDrag(e, (dx, dy) => {
      const local = rotateDeltaIntoLocalFrame({ width: dx, height: dy }, restTransform.rotation);
      return resizedCrop(start, corner, local, clipRectPx, aspectN);
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

  const rect = cropRectPx(display, clipRectPx);

  return (
    <div
      data-testid="crop-overlay"
      style={{
        position: "absolute",
        left: restTransform.centerX * canvasPx.width,
        top: restTransform.centerY * canvasPx.height,
        width: clipRectPx.width,
        height: clipRectPx.height,
        // Same translate-then-rotate idiom as TransformOverlay: center the
        // (still-unrotated) clip box on its point, then rotate about its own
        // center (CropOverlayView.swift:62-64's `rotationEffect(anchor:
        // clipRect.mid)`).
        transform: `translate(-50%, -50%) rotate(${restTransform.rotation}deg)`,
        pointerEvents: "none",
        zIndex: 3,
      }}
    >
      {/* Dim mask: 4 rects covering the clip box outside the crop rect
          (CropOverlayView.swift:22-27). */}
      <div
        style={{
          position: "absolute",
          left: 0,
          top: 0,
          width: "100%",
          height: rect.top,
          background: DIM_COLOR,
        }}
      />
      <div
        style={{
          position: "absolute",
          left: 0,
          top: rect.top + rect.height,
          width: "100%",
          height: Math.max(0, clipRectPx.height - rect.top - rect.height),
          background: DIM_COLOR,
        }}
      />
      <div
        style={{
          position: "absolute",
          left: 0,
          top: rect.top,
          width: rect.left,
          height: rect.height,
          background: DIM_COLOR,
        }}
      />
      <div
        style={{
          position: "absolute",
          left: rect.left + rect.width,
          top: rect.top,
          width: Math.max(0, clipRectPx.width - rect.left - rect.width),
          height: rect.height,
          background: DIM_COLOR,
        }}
      />

      {/* Rule-of-thirds guide lines (CropOverlayView.swift:29-37): 2 vertical
          + 2 horizontal, at 1/3 and 2/3 of the crop rect. */}
      {[1, 2].map((i) => {
        const f = i / 3;
        return (
          <div key={`v${i}`}>
            <div
              style={{
                position: "absolute",
                left: rect.left + rect.width * f,
                top: rect.top,
                width: GUIDE_WIDTH,
                height: rect.height,
                background: GUIDE_COLOR,
              }}
            />
            <div
              style={{
                position: "absolute",
                left: rect.left,
                top: rect.top + rect.height * f,
                width: rect.width,
                height: GUIDE_WIDTH,
                background: GUIDE_COLOR,
              }}
            />
          </div>
        );
      })}

      {/* Crop rect border (CropOverlayView.swift:38). */}
      <div
        style={{
          position: "absolute",
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
          border: `${BORDER_WIDTH}px solid ${BORDER_COLOR}`,
          boxSizing: "border-box",
        }}
      />

      {/* Drag-inside-to-pan surface (CropOverlayView.swift:42-50). */}
      <div
        onPointerDown={handlePanDown}
        style={{
          position: "absolute",
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
          cursor: "grab",
          pointerEvents: "auto",
        }}
      />

      {CORNERS.map((corner) => {
        const frac = CORNER_FRACTION[corner];
        return (
          <div
            key={corner}
            onPointerDown={(e) => handleResizeDown(e, corner)}
            style={{
              position: "absolute",
              left: rect.left + rect.width * frac.x,
              top: rect.top + rect.height * frac.y,
              width: HANDLE_SIZE,
              height: HANDLE_SIZE,
              marginLeft: -HANDLE_SIZE / 2,
              marginTop: -HANDLE_SIZE / 2,
              background: BORDER_COLOR,
              cursor: CORNER_CURSOR[corner],
              pointerEvents: "auto",
            }}
          />
        );
      })}
    </div>
  );
}
