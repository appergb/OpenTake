import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useT } from "../../i18n";
import type { TimelineRange } from "../../lib/timelineRange";
import * as edit from "../../store/editActions";
import { useEditorUiStore } from "../../store/uiStore";

type RangeMenuItem = {
  label: string;
  action: () => void;
};

export function rangeContextMenuItems({
  range,
  labels,
  onSave,
  onClear,
}: {
  range: TimelineRange;
  labels: { save: string; clear: string };
  onSave: (range: TimelineRange) => void | Promise<void>;
  onClear: () => void;
}): RangeMenuItem[] {
  return [
    { label: labels.save, action: () => void onSave(range) },
    { label: labels.clear, action: onClear },
  ];
}

export function TimelineRangeContextMenu({
  range,
  x,
  y,
  onClose,
}: {
  range: TimelineRange;
  x: number;
  y: number;
  onClose: () => void;
}) {
  const t = useT();
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });

  useEffect(() => {
    const onDown = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) onClose();
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const { width, height } = element.getBoundingClientRect();
    const margin = 8;
    setPos({
      left: x + width + margin > window.innerWidth ? Math.max(margin, x - width) : x,
      top: y + height + margin > window.innerHeight ? Math.max(margin, y - height) : y,
    });
  }, [x, y]);

  const items = rangeContextMenuItems({
    range,
    labels: {
      save: t("contextMenu.saveRangeAsMedia"),
      clear: t("contextMenu.clearRange"),
    },
    onSave: edit.saveMarkedRangeAsMedia,
    onClear: useEditorUiStore.getState().clearTimelineRange,
  });

  return (
    <div
      ref={ref}
      role="menu"
      style={{
        position: "fixed",
        left: pos.left,
        top: pos.top,
        zIndex: 1000,
        minWidth: 180,
        padding: "4px 0",
        background: "var(--bg-elevated)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: 6,
        boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
        fontSize: "var(--fs-sm)",
      }}
    >
      {items.map((item) => (
        <button
          key={item.label}
          type="button"
          role="menuitem"
          onClick={() => {
            item.action();
            onClose();
          }}
          style={{
            display: "block",
            width: "100%",
            padding: "6px 12px",
            textAlign: "left",
            color: "var(--text-primary)",
            background: "transparent",
            border: "none",
            cursor: "pointer",
            fontFamily: "var(--font-sans)",
            fontSize: "var(--fs-sm)",
          }}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
