/**
 * Custom-drawn dropdown (no native <select>) for enum settings that read better
 * as a menu than a segmented control — e.g. the language picker, which grows as
 * locales are added. Styling mirrors the rest of the settings surface (base
 * field + raised popup + check on the active row); the option labels come from
 * the caller (i18n language pack), so adding a locale needs no change here.
 *
 * Closes on outside click or Escape. Generic over the option id type so callers
 * keep their narrow union (`Locale`, `Theme`, …).
 */

import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown } from "lucide-react";
import { Icon } from "./Icon";

interface DropdownOption<T extends string> {
  id: T;
  label: string;
}

interface DropdownProps<T extends string> {
  value: T;
  options: ReadonlyArray<DropdownOption<T>>;
  onChange: (id: T) => void;
  /** Accessible name for the trigger (already-translated string). */
  ariaLabel?: string;
  /** Minimum width of trigger and popup, in px. */
  minWidth?: number;
}

export function Dropdown<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  minWidth = 132,
}: DropdownProps<T>) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const selected = options.find((o) => o.id === value);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} style={{ position: "relative", display: "inline-block" }}>
      <button
        type="button"
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="hover-area"
        style={{
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "var(--space-sm)",
          minWidth,
          height: 28,
          padding: "0 var(--space-sm) 0 var(--space-md)",
          background: "var(--bg-base)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: "var(--radius-sm)",
          color: "var(--text-primary)",
          fontSize: "var(--fs-sm)",
          fontWeight: "var(--fw-medium)",
        }}
      >
        <span>{selected?.label ?? value}</span>
        <span
          style={{
            display: "inline-flex",
            opacity: 0.6,
            transform: open ? "rotate(180deg)" : "none",
            transition: "transform var(--duration-fast, 120ms) ease",
          }}
        >
          <Icon icon={ChevronDown} size={13} />
        </span>
      </button>

      {open && (
        <div
          role="listbox"
          style={{
            position: "absolute",
            top: "calc(100% + var(--space-xs))",
            right: 0,
            minWidth,
            padding: "var(--space-xxs)",
            background: "var(--bg-raised)",
            border: "var(--bw-thin) solid var(--border-primary)",
            borderRadius: "var(--radius-md)",
            boxShadow: "var(--shadow-lg)",
            zIndex: 50,
            display: "flex",
            flexDirection: "column",
            gap: 1,
          }}
        >
          {options.map((opt) => {
            const active = opt.id === value;
            return (
              <button
                key={opt.id}
                type="button"
                role="option"
                aria-selected={active}
                onClick={() => {
                  onChange(opt.id);
                  setOpen(false);
                }}
                className="hover-area"
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "var(--space-sm)",
                  height: 28,
                  padding: "0 var(--space-sm)",
                  borderRadius: "var(--radius-xs-sm)",
                  background: active ? "var(--bg-prominent)" : "transparent",
                  color: active ? "var(--text-primary)" : "var(--text-secondary)",
                  fontSize: "var(--fs-sm)",
                  fontWeight: "var(--fw-medium)",
                  textAlign: "left",
                }}
              >
                <span
                  style={{ width: 12, display: "inline-flex", justifyContent: "center", flex: "0 0 auto" }}
                >
                  {active && <Icon icon={Check} size={11} />}
                </span>
                <span style={{ flex: 1 }}>{opt.label}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
