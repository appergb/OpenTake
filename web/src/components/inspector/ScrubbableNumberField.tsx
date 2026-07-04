/**
 * ScrubbableNumberField (SPEC §6.6). Warm-colored, right-aligned, tabular value.
 * Horizontal drag changes the value (Shift x10, Cmd x0.1); a 3px threshold
 * distinguishes drag from click; click switches to a text input (Enter/blur
 * commit, Esc cancel). `mixed` shows an em dash.
 */

import { useCallback, useRef, useState } from "react";
import { LAYOUT } from "../../lib/theme";

interface Props {
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
  const dragRef = useRef<{ startX: number; startValue: number; moved: boolean } | null>(null);
  const provisionalRef = useRef<number | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const clamp = (v: number) => Math.max(p.min, Math.min(p.max, v));

  const text = (() => {
    if (p.mixed) return "—";
    const override = p.displayTextOverride?.(p.value);
    if (override) return override;
    return p.format(p.value) + (p.suffix ?? "");
  })();

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      dragRef.current = { startX: e.clientX, startValue: p.value, moved: false };
      provisionalRef.current = null;
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    },
    [p.value],
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
    (e: React.PointerEvent) => {
      const d = dragRef.current;
      dragRef.current = null;
      (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      if (!d) return;
      if (d.moved && provisionalRef.current !== null) {
        p.onCommit(provisionalRef.current);
        provisionalRef.current = null;
      } else {
        setDraft(p.format(p.value));
        setEditing(true);
        requestAnimationFrame(() => inputRef.current?.select());
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [p],
  );

  const commitEdit = useCallback(() => {
    const cleaned = draft.replace(p.suffix ?? "", "").replace(",", ".").trim();
    const parsed = Number(cleaned);
    if (Number.isFinite(parsed)) p.onCommit(clamp(parsed));
    setEditing(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft, p]);

  if (editing) {
    return (
      <input
        ref={inputRef}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commitEdit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commitEdit();
          else if (e.key === "Escape") setEditing(false);
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
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      className="tabular"
      style={{
        width: p.width ?? 56,
        display: "inline-block",
        textAlign: "right",
        color: p.mixed ? "var(--text-tertiary)" : "var(--accent-primary)",
        fontSize: "var(--fs-sm)",
        cursor: "ew-resize",
        userSelect: "none",
      }}
    >
      {text}
    </span>
  );
}
