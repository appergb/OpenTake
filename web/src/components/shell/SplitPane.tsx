/**
 * Two-pane split with a draggable divider. The semantic separator has a 24px
 * effective target centered on the visual seam without consuming layout. The
 * container starts a resize only when that band is hit over non-interactive
 * content, so controls that touch a seam remain clickable. `initial` is the
 * first pane's size in px; `mode` is the split axis.
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

const SEPARATOR_HIT_TARGET = 24;
const INTERACTIVE_SELECTOR = [
  "button",
  "a[href]",
  "input",
  "select",
  "textarea",
  "summary",
  "canvas",
  "video",
  "[contenteditable='true']",
  "[role='button']",
  "[role='checkbox']",
  "[role='combobox']",
  "[role='gridcell']",
  "[role='link']",
  "[role='menuitem']",
  "[role='option']",
  "[role='radio']",
  "[role='slider']",
  "[role='spinbutton']",
  "[role='switch']",
  "[role='tab']",
  "[role='textbox']",
  "[draggable='true']",
  "[data-split-pane-interactive]",
].join(",");

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
  const separatorRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState(initial);
  const [totalSize, setTotalSize] = useState(initial + secondMin);
  const dragging = useRef(false);
  const captureRef = useRef<{ element: HTMLElement; pointerId: number } | null>(null);
  const [dragActive, setDragActive] = useState(false);
  const [pointerNear, setPointerNear] = useState(false);

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

  const isInEffectiveTarget = useCallback(
    (clientX: number, clientY: number) => {
      const container = containerRef.current;
      if (!container) return false;
      const rect = container.getBoundingClientRect();
      const position = isH ? clientX - rect.left : clientY - rect.top;
      return Math.abs(position - size) <= SEPARATOR_HIT_TARGET / 2;
    },
    [isH, size],
  );

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      if (!isInEffectiveTarget(e.clientX, e.clientY)) return;
      const target = e.target instanceof Element ? e.target : null;
      if (target?.closest(INTERACTIVE_SELECTOR)) return;
      e.preventDefault();
      e.stopPropagation();
      dragging.current = true;
      setDragActive(true);
      setPointerNear(true);
      const element = e.currentTarget as HTMLElement;
      separatorRef.current?.focus();
      captureRef.current = { element, pointerId: e.pointerId };
      element.setPointerCapture(e.pointerId);
    },
    [isInEffectiveTarget],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!dragging.current) {
        const target = e.target instanceof Element ? e.target : null;
        setPointerNear(
          !target?.closest(INTERACTIVE_SELECTOR) &&
            isInEffectiveTarget(e.clientX, e.clientY),
        );
        return;
      }
      if (!containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const total = isH ? rect.width : rect.height;
      const pos = isH ? e.clientX - rect.left : e.clientY - rect.top;
      setTotalSize(total);
      setClampedSize(pos, total);
    },
    [isH, isInEffectiveTarget, setClampedSize],
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
        overflow: "hidden",
        cursor: pointerNear ? (isH ? "col-resize" : "row-resize") : undefined,
      }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={() => endPointerDrag(true)}
      onLostPointerCapture={() => endPointerDrag(false)}
      onPointerLeave={() => {
        if (!dragging.current) setPointerNear(false);
      }}
    >
      <div
        style={{
          flex: `0 0 ${size}px`,
          minWidth: 0,
          minHeight: 0,
          overflow: "hidden",
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
        {/* Pointer events stay on the container so seam-adjacent controls win
            hit testing; the 24px semantic rect remains focusable by keyboard. */}
        <div
          ref={separatorRef}
          className="split-pane-separator"
          role="separator"
          aria-label={t("layout.resizePanels")}
          aria-orientation={isH ? "vertical" : "horizontal"}
          aria-valuemin={min}
          aria-valuemax={Math.max(min, totalSize - secondMin)}
          aria-valuenow={Math.round(size)}
          aria-valuetext={t("layout.positionPixels", { value: Math.round(size) })}
          data-interaction-state={dragActive ? "dragging" : "enabled"}
          data-pointer-near={pointerNear ? "true" : undefined}
          tabIndex={0}
          onKeyDown={onKeyDown}
          style={{
            position: "absolute",
            pointerEvents: "none",
            cursor: isH ? "col-resize" : "row-resize",
            ...(isH
              ? {
                  top: 0,
                  bottom: 0,
                  left: -SEPARATOR_HIT_TARGET / 2,
                  width: SEPARATOR_HIT_TARGET,
                }
              : {
                  left: 0,
                  right: 0,
                  top: -SEPARATOR_HIT_TARGET / 2,
                  height: SEPARATOR_HIT_TARGET,
                }),
          }}
        />
      </div>
      <div
        style={{
          flex: "1 1 0",
          minWidth: 0,
          minHeight: 0,
          overflow: "hidden",
          position: "relative",
        }}
      >
        {second}
      </div>
    </div>
  );
}
