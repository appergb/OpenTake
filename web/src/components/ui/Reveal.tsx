import {
  type AriaRole,
  type CSSProperties,
  type ReactNode,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import "../../styles/components.css";

const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";
const FALLBACK_DISCLOSURE_DURATION_MS = 180;

type RevealStyle = CSSProperties & {
  "--reveal-block-size": string;
};

export interface RevealProps {
  open: boolean;
  children: ReactNode;
  id?: string;
  role?: AriaRole;
  onExited?: () => void;
}

function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia(REDUCED_MOTION_QUERY).matches;
}

function disclosureDurationMs(): number {
  if (typeof window === "undefined") return FALLBACK_DISCLOSURE_DURATION_MS;
  const value = window
    .getComputedStyle(document.documentElement)
    .getPropertyValue("--motion-disclosure-duration")
    .trim();
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed)) return FALLBACK_DISCLOSURE_DURATION_MS;
  return value.endsWith("ms") ? parsed : value.endsWith("s") ? parsed * 1_000 : parsed;
}

export function Reveal({ open, children, id, role, onExited }: RevealProps) {
  const [present, setPresent] = useState(open);
  const [expanded, setExpanded] = useState(open);
  const [blockSize, setBlockSize] = useState(0);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const exitTimerRef = useRef<number | null>(null);
  const frameRef = useRef<number | null>(null);
  const onExitedRef = useRef(onExited);
  onExitedRef.current = onExited;

  useLayoutEffect(() => {
    const wrapper = wrapperRef.current;
    if (!wrapper) return;
    if (open) wrapper.removeAttribute("inert");
    else wrapper.setAttribute("inert", "");
  }, [open, present]);

  useLayoutEffect(() => {
    if (!present) return;
    const content = contentRef.current;
    if (!content) return;

    const measure = (height?: number) => {
      const measured = Math.max(height ?? 0, content.scrollHeight);
      setBlockSize((current) => (current === measured ? current : measured));
    };

    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => measure(entry?.contentRect.height));
    observer.observe(content);
    return () => observer.disconnect();
  }, [present]);

  useLayoutEffect(() => {
    if (exitTimerRef.current !== null) {
      window.clearTimeout(exitTimerRef.current);
      exitTimerRef.current = null;
    }
    if (frameRef.current !== null) {
      window.cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    }

    if (open) {
      if (!present) {
        setExpanded(false);
        setPresent(true);
        return;
      }
      if (!expanded) {
        if (prefersReducedMotion()) {
          setExpanded(true);
        } else {
          frameRef.current = window.requestAnimationFrame(() => {
            frameRef.current = null;
            setExpanded(true);
          });
        }
      }
      return;
    }

    if (!present) return;
    const activeElement = document.activeElement;
    if (activeElement instanceof HTMLElement && contentRef.current?.contains(activeElement)) {
      activeElement.blur();
    }
    setExpanded(false);

    const duration = prefersReducedMotion() ? 0 : disclosureDurationMs();
    if (duration <= 0) {
      setPresent(false);
      onExitedRef.current?.();
      return;
    }

    exitTimerRef.current = window.setTimeout(() => {
      exitTimerRef.current = null;
      setPresent(false);
      onExitedRef.current?.();
    }, duration);

    return () => {
      if (exitTimerRef.current !== null) {
        window.clearTimeout(exitTimerRef.current);
        exitTimerRef.current = null;
      }
    };
  }, [expanded, open, present]);

  useLayoutEffect(
    () => () => {
      if (exitTimerRef.current !== null) window.clearTimeout(exitTimerRef.current);
      if (frameRef.current !== null) window.cancelAnimationFrame(frameRef.current);
    },
    [],
  );

  if (!present) return null;

  const style: RevealStyle = {
    "--reveal-block-size": `${blockSize}px`,
  };

  return (
    <div
      ref={wrapperRef}
      className="reveal"
      data-state={expanded ? "open" : "closed"}
      id={id}
      role={role}
      aria-hidden={open ? undefined : true}
      style={style}
    >
      <div className="reveal__content" ref={contentRef}>
        {children}
      </div>
    </div>
  );
}
