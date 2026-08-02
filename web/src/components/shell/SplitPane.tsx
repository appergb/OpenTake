/**
 * Two-pane split with a draggable divider. The divider hit-area is widened by
 * panelGap/2 each side (SPEC §2.4 `effectiveRect`). `initial` is the first
 * pane's size in px; `mode` is the split axis. Sizes are clamped to [min, max]
 * for the first pane and a minimum for the second.
 */

import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useT } from "../../i18n";

interface SplitPaneProps {
  mode: "horizontal" | "vertical"; // horizontal = side-by-side; vertical = stacked
  initial: number;
  min?: number;
  secondMin?: number;
  first: ReactNode;
  second: ReactNode;
}

const GAP = 5; // --panel-gap

export function SplitPane({
  mode,
  initial,
  min = 120,
  secondMin = 120,
  first,
  second,
}: SplitPaneProps) {
  const t = useT();
  const isH = mode === "horizontal";
  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState(initial);
  const [totalSize, setTotalSize] = useState(initial + secondMin);
  const dragging = useRef(false);
  const captureRef = useRef<{ element: HTMLElement; pointerId: number } | null>(null);
  const [dragActive, setDragActive] = useState(false);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const measure = () => {
      const nextTotal = isH ? container.clientWidth : container.clientHeight;
      if (nextTotal <= 0) return;
      setTotalSize(nextTotal);
      setSize((current) => Math.max(min, Math.min(nextTotal - secondMin, current)));
    };
    measure();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", measure);
      return () => window.removeEventListener("resize", measure);
    }
    const observer = new ResizeObserver(measure);
    observer.observe(container);
    return () => observer.disconnect();
  }, [isH, min, secondMin]);

  const setClampedSize = useCallback(
    (next: number, total = totalSize) => {
      setSize(Math.max(min, Math.min(Math.max(min, total - secondMin), next)));
    },
    [min, secondMin, totalSize],
  );

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      dragging.current = true;
      setDragActive(true);
      const element = e.currentTarget as HTMLElement;
      element.focus();
      captureRef.current = { element, pointerId: e.pointerId };
      element.setPointerCapture(e.pointerId);
    },
    [],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!dragging.current || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const total = isH ? rect.width : rect.height;
      const pos = isH ? e.clientX - rect.left : e.clientY - rect.top;
      setTotalSize(total);
      setClampedSize(pos, total);
    },
    [isH, setClampedSize],
  );

  const endPointerDrag = useCallback((releaseCapture: boolean) => {
    dragging.current = false;
    setDragActive(false);
    const capture = captureRef.current;
    captureRef.current = null;
    if (!releaseCapture || !capture) return;
    try {
      capture.element.releasePointerCapture(capture.pointerId);
    } catch {
      // Pointer capture may already have been released by the browser.
    }
  }, []);

  const onPointerUp = useCallback(() => endPointerDrag(true), [endPointerDrag]);

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === "Escape" && dragging.current) {
        event.preventDefault();
        endPointerDrag(true);
        return;
      }
      const decrease = isH ? event.key === "ArrowLeft" : event.key === "ArrowUp";
      const increase = isH ? event.key === "ArrowRight" : event.key === "ArrowDown";
      if (!decrease && !increase && event.key !== "Home" && event.key !== "End") return;
      event.preventDefault();
      if (event.key === "Home") setClampedSize(min);
      else if (event.key === "End") setClampedSize(totalSize - secondMin);
      else setClampedSize(size + (increase ? 10 : -10));
    },
    [endPointerDrag, isH, min, secondMin, setClampedSize, size, totalSize],
  );

  return (
    <div
      ref={containerRef}
      style={{
        display: "flex",
        flexDirection: isH ? "row" : "column",
        width: "100%",
        height: "100%",
        minWidth: 0,
        minHeight: 0,
      }}
    >
      <div
        style={{
          flex: `0 0 ${size}px`,
          minWidth: 0,
          minHeight: 0,
          position: "relative",
        }}
      >
        {first}
      </div>
      <div
        style={{
          position: "relative",
          flex: "0 0 0px",
          zIndex: 50,
        }}
      >
        {/* widened hit-area centered on the seam */}
        <div
          className="split-pane-separator"
          role="separator"
          aria-label={t("layout.resizePanels")}
          aria-orientation={isH ? "vertical" : "horizontal"}
          aria-valuemin={min}
          aria-valuemax={Math.max(min, totalSize - secondMin)}
          aria-valuenow={Math.round(size)}
          aria-valuetext={t("layout.positionPixels", { value: Math.round(size) })}
          data-interaction-state={dragActive ? "dragging" : "enabled"}
          tabIndex={0}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={() => endPointerDrag(true)}
          onLostPointerCapture={() => endPointerDrag(false)}
          onKeyDown={onKeyDown}
          style={{
            position: "absolute",
            cursor: isH ? "col-resize" : "row-resize",
            ...(isH
              ? { top: 0, bottom: 0, left: -(GAP / 2), width: GAP }
              : { left: 0, right: 0, top: -(GAP / 2), height: GAP }),
          }}
        />
      </div>
      <div style={{ flex: "1 1 0", minWidth: 0, minHeight: 0, position: "relative" }}>
        {second}
      </div>
    </div>
  );
}
