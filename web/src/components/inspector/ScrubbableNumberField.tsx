/**
 * ScrubbableNumberField (SPEC §6.6). Warm-colored, right-aligned, tabular value.
 * Horizontal drag changes the value (Shift x10, Cmd x0.1); a 3px threshold
 * distinguishes drag from click; click switches to a text input (Enter/blur
 * commit, Esc cancel). `mixed` shows an em dash.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { LAYOUT } from "../../lib/theme";

interface Props {
  ariaLabel?: string;
  disabled?: boolean;
  value: number;
  mixed?: boolean;
  min: number;
  max: number;
  /** Display units changed per pixel of horizontal drag. */
  sensitivity: number;
  /** Format the numeric value into display text (without suffix handled here). */
  format: (v: number) => string;
  suffix?: string;
  width?: number;
  onChange?: (v: number) => void; // during drag (optional live)
  onCommit: (v: number) => void;
  /** Override the rendered text (e.g. "-∞ dB" for the volume floor). */
  displayTextOverride?: (v: number) => string | null;
}

export function ScrubbableNumberField(p: Props) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const dragRef = useRef<{
    startX: number;
    startValue: number;
    moved: boolean;
    pointerId: number;
    captureTarget: HTMLElement;
  } | null>(null);
  const provisionalRef = useRef<number | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const displayRef = useRef<HTMLSpanElement>(null);
  const restoreDisplayFocusRef = useRef(false);

  useEffect(() => {
    if (editing) {
      inputRef.current?.focus();
      inputRef.current?.select();
      return;
    }
    if (restoreDisplayFocusRef.current) {
      restoreDisplayFocusRef.current = false;
      displayRef.current?.focus();
    }
  }, [editing]);

  const clamp = (v: number) => Math.max(p.min, Math.min(p.max, v));

  const text = (() => {
    if (p.mixed) return "—";
    const override = p.displayTextOverride?.(p.value);
    if (override) return override;
    return p.format(p.value) + (p.suffix ?? "");
  })();

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      if (p.disabled) return;
      e.preventDefault();
      const captureTarget = e.currentTarget as HTMLElement;
      captureTarget.focus();
      dragRef.current = {
        startX: e.clientX,
        startValue: p.value,
        moved: false,
        pointerId: e.pointerId,
        captureTarget,
      };
      provisionalRef.current = null;
      captureTarget.setPointerCapture(e.pointerId);
    },
    [p.disabled, p.value],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      const d = dragRef.current;
      if (!d) return;
      const dx = e.clientX - d.startX;
      if (!d.moved && Math.abs(dx) < LAYOUT.dragThreshold) return;
      d.moved = true;
      let mult = p.sensitivity;
      if (e.shiftKey) mult *= 10;
      if (e.metaKey) mult *= 0.1;
      const next = clamp(d.startValue + dx * mult);
      provisionalRef.current = next;
      p.onChange?.(next);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [p.sensitivity, p.min, p.max],
  );

  const onPointerUp = useCallback(
    (_e: React.PointerEvent) => {
      const d = dragRef.current;
      dragRef.current = null;
      if (!d) return;
      try {
        d.captureTarget.releasePointerCapture(d.pointerId);
      } catch {
        // Capture may already be gone when the browser ends the gesture.
      }
      if (p.disabled) {
        provisionalRef.current = null;
        return;
      }
      if (d.moved && provisionalRef.current !== null) {
        p.onCommit(provisionalRef.current);
        provisionalRef.current = null;
      } else {
        setDraft(p.format(p.value));
        setEditing(true);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [p],
  );

  const cancelPointer = useCallback(() => {
    const d = dragRef.current;
    dragRef.current = null;
    provisionalRef.current = null;
    if (!d) return;
    try {
      d.captureTarget.releasePointerCapture(d.pointerId);
    } catch {
      // `lostpointercapture` means there is no capture left to release.
    }
    d.captureTarget.focus();
  }, []);

  useEffect(() => {
    if (!p.disabled) return;
    const d = dragRef.current;
    dragRef.current = null;
    provisionalRef.current = null;
    restoreDisplayFocusRef.current = false;
    if (d) {
      try {
        d.captureTarget.releasePointerCapture(d.pointerId);
      } catch {
        // The browser may already have released capture while disabling.
      }
    }
    setEditing(false);
  }, [p.disabled]);

  const finishEditing = useCallback((restoreFocus: boolean) => {
    restoreDisplayFocusRef.current = restoreFocus;
    setEditing(false);
  }, []);

  const commitEdit = useCallback((restoreFocus: boolean) => {
    const cleaned = draft.replace(p.suffix ?? "", "").replace(",", ".").trim();
    const parsed = Number(cleaned);
    if (!p.disabled && Number.isFinite(parsed)) p.onCommit(clamp(parsed));
    finishEditing(restoreFocus);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft, p, finishEditing]);

  const beginEditing = useCallback(() => {
    if (p.disabled) return;
    setDraft(p.format(p.value));
    setEditing(true);
  }, [p]);

  if (editing) {
    return (
      <input
        ref={inputRef}
        aria-label={p.ariaLabel ?? "Value"}
        disabled={p.disabled}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => commitEdit(false)}
        onKeyDown={(e) => {
          if (e.key === "Enter") commitEdit(true);
          else if (e.key === "Escape") finishEditing(true);
        }}
        className="tabular"
        style={{
          width: p.width ?? 56,
          textAlign: "right",
          background: "var(--bg-raised)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: "var(--radius-xs)",
          color: "var(--accent-primary)",
          fontSize: "var(--fs-sm)",
          padding: "1px 4px",
        }}
      />
    );
  }

  return (
    <span
      ref={displayRef}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={cancelPointer}
      onLostPointerCapture={cancelPointer}
      role="spinbutton"
      aria-label={p.ariaLabel ?? "Value"}
      aria-valuemin={p.min}
      aria-valuemax={p.max}
      aria-valuenow={p.mixed ? undefined : p.value}
      aria-valuetext={text}
      aria-disabled={p.disabled || undefined}
      tabIndex={p.disabled ? -1 : 0}
      data-interaction-state={p.disabled ? "disabled" : "enabled"}
      onKeyDown={(e) => {
        if (p.disabled) return;
        if (e.key === "Escape" && dragRef.current) {
          e.preventDefault();
          cancelPointer();
          return;
        }
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          beginEditing();
          return;
        }
        if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
        e.preventDefault();
        const modifier = (e.shiftKey ? 10 : 1) * (e.metaKey || e.ctrlKey ? 0.1 : 1);
        p.onCommit(clamp(p.value + (e.key === "ArrowUp" ? 1 : -1) * p.sensitivity * modifier));
      }}
      className="tabular"
      style={{
        width: p.width ?? 56,
        display: "inline-block",
        textAlign: "right",
        color: p.mixed ? "var(--text-tertiary)" : "var(--accent-primary)",
        fontSize: "var(--fs-sm)",
        cursor: p.disabled ? "not-allowed" : "ew-resize",
        userSelect: "none",
      }}
    >
      {text}
    </span>
  );
}
