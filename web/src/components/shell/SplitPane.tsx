/**
 * Two-pane split with a draggable divider. The divider hit-area is widened by
 * panelGap/2 each side (SPEC §2.4 `effectiveRect`). `initial` is the first
 * pane's size in px; `mode` is the split axis. Sizes are clamped to [min, max]
 * for the first pane and a minimum for the second.
 */

import { useCallback, useRef, useState, type ReactNode } from "react";

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
  const isH = mode === "horizontal";
  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState(initial);
  const dragging = useRef(false);

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      dragging.current = true;
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    },
    [],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!dragging.current || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const total = isH ? rect.width : rect.height;
      const pos = isH ? e.clientX - rect.left : e.clientY - rect.top;
      const clamped = Math.max(min, Math.min(total - secondMin, pos));
      setSize(clamped);
    },
    [isH, min, secondMin],
  );

  const onPointerUp = useCallback((e: React.PointerEvent) => {
    dragging.current = false;
    (e.target as HTMLElement).releasePointerCapture(e.pointerId);
  }, []);

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
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        style={{
          position: "relative",
          flex: "0 0 0px",
          cursor: isH ? "col-resize" : "row-resize",
          zIndex: 50,
        }}
      >
        {/* widened hit-area centered on the seam */}
        <div
          style={{
            position: "absolute",
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
