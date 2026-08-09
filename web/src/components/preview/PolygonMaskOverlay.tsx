/** On-canvas point editing for the first polygon mask on the selected clip. */

import { useEffect, useRef, useState } from "react";
import * as edit from "../../store/editActions";
import type { Clip, Mask, MaskTransform, Point2 } from "../../lib/types";

const HANDLE_RADIUS = 5;
const OUTLINE = "rgba(255,255,255,0.85)";
const HANDLE_FILL = "var(--accent-primary)";

function completeTransform(transform: Mask["transform"]): MaskTransform {
  return {
    offset: transform?.offset ?? { x: 0, y: 0 },
    scale: transform?.scale ?? { x: 1, y: 1 },
    rotationDegrees: transform?.rotationDegrees ?? 0,
  };
}

/** Apply the same center-relative transform used by the Rust CPU/GPU mask path. */
export function transformMaskPoint(point: Point2, transform: Mask["transform"]): Point2 {
  const complete = completeTransform(transform);
  const radians = (complete.rotationDegrees * Math.PI) / 180;
  const x = (point.x - 0.5) * complete.scale.x;
  const y = (point.y - 0.5) * complete.scale.y;
  return {
    x: x * Math.cos(radians) - y * Math.sin(radians) + 0.5 + complete.offset.x,
    y: x * Math.sin(radians) + y * Math.cos(radians) + 0.5 + complete.offset.y,
  };
}

/** Convert a display-space drag delta back into the polygon's local coordinates. */
export function inverseMaskDelta(delta: Point2, transform: Mask["transform"]): Point2 {
  const complete = completeTransform(transform);
  const radians = (-complete.rotationDegrees * Math.PI) / 180;
  const x = delta.x * Math.cos(radians) - delta.y * Math.sin(radians);
  const y = delta.x * Math.sin(radians) + delta.y * Math.cos(radians);
  return {
    x: x / Math.max(Math.abs(complete.scale.x), 0.0001),
    y: y / Math.max(Math.abs(complete.scale.y), 0.0001),
  };
}

export function PolygonMaskOverlay({
  clip,
  canvasPx,
}: {
  clip: Clip;
  canvasPx: { width: number; height: number };
}) {
  const mask = clip.masks?.[0];
  const polygon = mask?.shape.kind === "poly" ? mask.shape : null;
  const [dragPoints, setDragPoints] = useState<Point2[] | null>(null);
  const dragPointsRef = useRef<Point2[] | null>(null);
  const cleanupRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    setDragPoints(null);
    dragPointsRef.current = null;
  }, [clip.id, polygon]);

  useEffect(
    () => () => {
      cleanupRef.current?.();
      cleanupRef.current = null;
    },
    [],
  );

  if (
    !mask ||
    !polygon ||
    polygon.points.length < 3 ||
    !Number.isFinite(canvasPx.width) ||
    !Number.isFinite(canvasPx.height) ||
    canvasPx.width <= 0 ||
    canvasPx.height <= 0
  ) {
    return null;
  }

  const points = dragPoints ?? polygon.points;
  const displayPoints = points.map((point) => transformMaskPoint(point, mask.transform));

  const beginPointDrag = (event: React.PointerEvent, index: number) => {
    event.preventDefault();
    event.stopPropagation();
    const startClient = { x: event.clientX, y: event.clientY };
    const startPoints = polygon.points.map((point) => ({ ...point }));
    const onMove = (nextEvent: PointerEvent) => {
      const localDelta = inverseMaskDelta(
        {
          x: (nextEvent.clientX - startClient.x) / canvasPx.width,
          y: (nextEvent.clientY - startClient.y) / canvasPx.height,
        },
        mask.transform,
      );
      const nextPoints = startPoints.map((point, pointIndex) =>
        pointIndex === index
          ? {
              x: Math.max(0, Math.min(1, point.x + localDelta.x)),
              y: Math.max(0, Math.min(1, point.y + localDelta.y)),
            }
          : point,
      );
      dragPointsRef.current = nextPoints;
      setDragPoints(nextPoints);
    };
    const finish = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", finish);
      cleanupRef.current = null;
      const committed = dragPointsRef.current;
      dragPointsRef.current = null;
      setDragPoints(null);
      if (committed) {
        void edit.setMasks(
          [clip.id],
          [{ ...mask, shape: { kind: "poly", points: committed } }, ...(clip.masks?.slice(1) ?? [])],
        );
      }
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", finish);
    cleanupRef.current = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", finish);
    };
  };

  return (
    <svg
      data-testid="polygon-mask-overlay"
      width={canvasPx.width}
      height={canvasPx.height}
      viewBox={`0 0 ${canvasPx.width} ${canvasPx.height}`}
      style={{ position: "absolute", inset: 0, zIndex: 5, overflow: "visible", pointerEvents: "none" }}
    >
      <polygon
        points={displayPoints.map((point) => `${point.x * canvasPx.width},${point.y * canvasPx.height}`).join(" ")}
        fill="rgba(255,255,255,0.08)"
        stroke={OUTLINE}
        strokeWidth={1}
        vectorEffect="non-scaling-stroke"
      />
      {displayPoints.map((point, index) => (
        <circle
          key={index}
          data-testid={`polygon-mask-point-${index}`}
          cx={point.x * canvasPx.width}
          cy={point.y * canvasPx.height}
          r={HANDLE_RADIUS}
          fill={HANDLE_FILL}
          stroke={OUTLINE}
          strokeWidth={1}
          style={{ cursor: "move", pointerEvents: "auto" }}
          onPointerDown={(event) => beginPointDrag(event, index)}
        />
      ))}
    </svg>
  );
}
