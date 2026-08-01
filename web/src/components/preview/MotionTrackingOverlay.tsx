import { useEffect, useRef, useState } from "react";
import { useT } from "../../i18n";
import type { MotionTrackingRegion } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";

const MIN_SIZE = 0.02;

function clamp(value: number): number {
  return Math.max(0, Math.min(1, value));
}

export function normalizedMotionRegion(
  start: { x: number; y: number },
  end: { x: number; y: number },
): MotionTrackingRegion {
  const left = clamp(Math.min(start.x, end.x));
  const top = clamp(Math.min(start.y, end.y));
  const right = clamp(Math.max(start.x, end.x));
  const bottom = clamp(Math.max(start.y, end.y));
  return {
    x: Math.min(left, 1 - MIN_SIZE),
    y: Math.min(top, 1 - MIN_SIZE),
    width: Math.max(MIN_SIZE, right - left),
    height: Math.max(MIN_SIZE, bottom - top),
  };
}

export function MotionTrackingOverlay({ canvasPx }: {
  canvasPx: { width: number; height: number };
}) {
  const t = useT();
  const selection = useEditorUiStore((state) => state.motionTrackingSelection);
  const setRegion = useEditorUiStore((state) => state.setMotionTrackingRegion);
  const [draft, setDraft] = useState<MotionTrackingRegion | null>(null);
  const cleanupRef = useRef<(() => void) | null>(null);

  useEffect(() => () => cleanupRef.current?.(), []);
  if (!selection || canvasPx.width <= 0 || canvasPx.height <= 0) return null;
  const region = draft ?? selection.region;

  function begin(event: React.PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    event.stopPropagation();
    const bounds = event.currentTarget.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) return;
    const start = {
      x: clamp((event.clientX - bounds.left) / bounds.width),
      y: clamp((event.clientY - bounds.top) / bounds.height),
    };
    let latest = normalizedMotionRegion(start, start);
    setDraft(latest);
    const move = (next: PointerEvent) => {
      latest = normalizedMotionRegion(start, {
        x: (next.clientX - bounds.left) / bounds.width,
        y: (next.clientY - bounds.top) / bounds.height,
      });
      setDraft(latest);
    };
    const finish = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      cleanupRef.current = null;
      setDraft(null);
      setRegion(latest);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
    cleanupRef.current = finish;
  }

  return (
    <div
      data-testid="motion-tracking-overlay"
      aria-label={t("inspector.motionTracking.previewRegionLabel")}
      onPointerDown={begin}
      style={{ position: "absolute", inset: 0, zIndex: 8, cursor: "crosshair", touchAction: "none" }}
    >
      <div
        style={{
          position: "absolute",
          left: `${region.x * 100}%`,
          top: `${region.y * 100}%`,
          width: `${region.width * 100}%`,
          height: `${region.height * 100}%`,
          border: "2px solid var(--accent-primary)",
          background: "color-mix(in srgb, var(--accent-primary) 12%, transparent)",
          boxShadow: "0 0 0 1px rgba(0,0,0,0.65)",
          pointerEvents: "none",
        }}
      >
        <span style={{ position: "absolute", left: 4, top: 3, color: "white", fontSize: "var(--fs-xxs)", textShadow: "0 1px 2px black" }}>
          {t("inspector.motionTracking.subject")}
        </span>
      </div>
    </div>
  );
}
