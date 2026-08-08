/**
 * Icon/glyph button with the upstream HoverHighlight behavior (SPEC §4.2, §9.7):
 * a square 24x24 hit frame, faint hover background, stronger when active.
 */

import type {
  AriaAttributes,
  CSSProperties,
  KeyboardEventHandler,
  MouseEvent,
  ReactNode,
  Ref,
} from "react";

interface HoverButtonProps {
  children: ReactNode;
  title?: string;
  active?: boolean;
  disabled?: boolean;
  onClick?: (e: MouseEvent) => void;
  size?: number; // hit-frame edge, default 24
  style?: CSSProperties;
  className?: string;
  buttonRef?: Ref<HTMLButtonElement>;
  ariaHasPopup?: AriaAttributes["aria-haspopup"];
  ariaExpanded?: boolean;
  ariaControls?: string;
  onKeyDown?: KeyboardEventHandler<HTMLButtonElement>;
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
  buttonRef,
  ariaHasPopup,
  ariaExpanded,
  ariaControls,
  onKeyDown,
}: HoverButtonProps) {
  return (
    <button
      ref={buttonRef}
      type="button"
      title={title}
      aria-label={title}
      aria-haspopup={ariaHasPopup}
      aria-expanded={ariaExpanded}
      aria-controls={ariaControls}
      disabled={disabled}
      data-interaction-state={disabled ? "disabled" : active ? "active" : "enabled"}
      onClick={onClick}
      onKeyDown={onKeyDown}
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
        cursor: disabled ? "not-allowed" : "pointer",
        ...style,
      }}
    >
      {children}
    </button>
  );
}
