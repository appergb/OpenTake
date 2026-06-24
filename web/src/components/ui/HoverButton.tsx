/**
 * Icon/glyph button with the upstream HoverHighlight behavior (SPEC §4.2, §9.7):
 * a square 24x24 hit frame, faint hover background, stronger when active.
 */

import type { ReactNode, CSSProperties, MouseEvent } from "react";

interface HoverButtonProps {
  children: ReactNode;
  title?: string;
  active?: boolean;
  disabled?: boolean;
  onClick?: (e: MouseEvent) => void;
  size?: number; // hit-frame edge, default 24
  style?: CSSProperties;
  className?: string;
}

export function HoverButton({
  children,
  title,
  active = false,
  disabled = false,
  onClick,
  size = 24,
  style,
  className,
}: HoverButtonProps) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      disabled={disabled}
      onClick={onClick}
      // Don't grab keyboard focus on mouse click: a focused transport button
      // would re-activate on Space/Arrow (WebKit fires that on keyup, which the
      // window keydown handler's preventDefault doesn't cancel) and double-fire
      // the global shortcut — e.g. Space toggling play twice so the button never
      // flips state. onClick still fires; Tab focus (accessibility) is unaffected.
      onMouseDown={(e) => e.preventDefault()}
      className={`hover-area${active ? " is-active" : ""}${className ? " " + className : ""}`}
      style={{
        width: size,
        height: size,
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        flex: "0 0 auto",
        color: active ? "var(--text-primary)" : "var(--text-secondary)",
        opacity: disabled ? 0.35 : 1,
        cursor: disabled ? "default" : "pointer",
        ...style,
      }}
    >
      {children}
    </button>
  );
}
