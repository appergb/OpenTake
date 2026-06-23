/**
 * ClipContextMenu (SPEC §5.8). Right-click menu for timeline clips. MVP items:
 * Split at Playhead / Delete / Link or Unlink. Copy/Cut/Paste will be added
 * once the clipboard PR (#94) lands. Closes on outside click or item action.
 */

import { useEffect, useRef } from "react";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import * as edit from "../../store/editActions";
import { useT } from "../../i18n";

export function ClipContextMenu({
  clipId,
  onClose,
}: {
  clipId: string;
  onClose: () => void;
}) {
  const t = useT();
  const timeline = useProjectStore((s) => s.timeline);
  const selectedClipIds = useEditorUiStore((s) => s.selectedClipIds);
  const selectClips = useEditorUiStore((s) => s.selectClips);
  const ref = useRef<HTMLDivElement>(null);

  // Close on outside click or Escape.
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  // Locate the clip to read linkGroupId.
  let clip: { linkGroupId?: string } | null = null;
  for (const track of timeline.tracks) {
    const found = track.clips.find((c) => c.id === clipId);
    if (found) {
      clip = found;
      break;
    }
  }
  if (!clip) {
    onClose();
    return null;
  }

  // The menu acts on the current selection; if the right-clicked clip isn't
  // selected, select just it (mirrors typical NLE behavior).
  const isSelected = selectedClipIds.has(clipId);
  const ensureSelected = () => {
    if (!isSelected) selectClips(new Set([clipId]));
  };

  const items: Array<{ label: string; action: () => void; danger?: boolean }> = [
    {
      label: t("contextMenu.split"),
      action: () => {
        ensureSelected();
        void edit.splitAtPlayhead();
      },
    },
    {
      label: t("contextMenu.delete"),
      action: () => {
        ensureSelected();
        void edit.deleteSelectedClips();
      },
      danger: true,
    },
  ];

  // Link/Unlink: operate on the full selection (>= 2 clips to link).
  if (clip.linkGroupId) {
    items.push({
      label: t("contextMenu.unlink"),
      action: () => {
        ensureSelected();
        const ids = [...useEditorUiStore.getState().selectedClipIds];
        if (ids.length > 0) void edit.unlinkClips(ids);
      },
    });
  } else {
    items.push({
      label: t("contextMenu.link"),
      action: () => {
        ensureSelected();
        const ids = [...useEditorUiStore.getState().selectedClipIds];
        if (ids.length >= 2) void edit.linkClips(ids);
      },
    });
  }

  return (
    <div
      ref={ref}
      style={{
        position: "fixed",
        zIndex: 1000,
        minWidth: 160,
        padding: "4px 0",
        background: "var(--bg-elevated)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: 6,
        boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
        fontSize: "var(--fs-sm)",
      }}
    >
      {items.map((item, i) => (
        <button
          key={i}
          onClick={() => {
            item.action();
            onClose();
          }}
          style={{
            display: "block",
            width: "100%",
            padding: "6px 12px",
            textAlign: "left",
            color: item.danger ? "var(--accent-danger, #ff6b6b)" : "var(--text-primary)",
            background: "transparent",
            border: "none",
            cursor: "pointer",
            fontFamily: "var(--font-sans)",
            fontSize: "var(--fs-sm)",
          }}
          onMouseEnter={(e) => {
            (e.currentTarget as HTMLElement).style.background = "var(--bg-hover, rgba(255,255,255,0.08))";
          }}
          onMouseLeave={(e) => {
            (e.currentTarget as HTMLElement).style.background = "transparent";
          }}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
