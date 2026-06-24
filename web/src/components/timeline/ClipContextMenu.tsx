/**
 * ClipContextMenu (SPEC §5.8). Right-click menu for timeline clips. MVP items:
 * Split at Playhead / Delete / Link or Unlink. Copy/Cut/Paste will be added
 * once the clipboard PR (#94) lands. Closes on outside click or item action.
 *
 * Positioning (#93 review #108): the root element is position:fixed and placed
 * at the viewport coords (left/top) passed in from TimelineContainer, which
 * captured them from the contextmenu event's clientX/clientY. If the menu would
 * overflow the viewport edge it flips to the opposite side of the click point.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import * as edit from "../../store/editActions";
import { useT } from "../../i18n";
import { isSingleLinkGroup } from "../../lib/clip";
import type { Clip } from "../../lib/types";

// Fixed size estimate for viewport-boundary flipping before the menu is
// measured. Close to the rendered size so the flip decision is correct on the
// first paint; the actual size is re-measured in a layout effect below.
const MENU_ESTIMATE = { width: 180, height: 240 };

export function ClipContextMenu({
  clipId,
  left,
  top,
  onClose,
}: {
  clipId: string;
  left: number;
  top: number;
  onClose: () => void;
}) {
  const t = useT();
  const timeline = useProjectStore((s) => s.timeline);
  const selectedClipIds = useEditorUiStore((s) => s.selectedClipIds);
  const selectClips = useEditorUiStore((s) => s.selectClips);
  const setPendingSwapClipId = useEditorUiStore((s) => s.setPendingSwapClipId);
  const ref = useRef<HTMLDivElement>(null);

  // Compute the final position with viewport-boundary flipping. Start from the
  // estimate so the first paint is already correct; re-measure after mount.
  const [pos, setPos] = useState(() => ({
    left: flipLeft(left, MENU_ESTIMATE.width),
    top: flipTop(top, MENU_ESTIMATE.height),
  }));

  // Re-measure with the real DOM size once mounted (before paint, no flicker).
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    setPos({ left: flipLeft(left, w), top: flipTop(top, h) });
  }, [left, top]);

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

  // Locate the clip to read linkGroupId + mediaType for menu gating. The parent
  // (TimelineContainer) already validates the clip exists in onContextMenu before
  // opening the menu, so a missing clip here is a stale-state edge case — just
  // render nothing. Do NOT call onClose() during render (React render purity,
  // review #108 item 2).
  let clip: Clip | null = null;
  for (const track of timeline.tracks) {
    const found = track.clips.find((c) => c.id === clipId);
    if (found) {
      clip = found;
      break;
    }
  }
  if (!clip) return null;

  // The menu acts on the current selection; if the right-clicked clip isn't
  // selected, select just it (mirrors typical NLE behavior).
  const isSelected = selectedClipIds.has(clipId);
  const ensureSelected = () => {
    if (!isSelected) selectClips(new Set([clipId]));
  };

  const items: Array<{ id: string; label: string; action: () => void; danger?: boolean; disabled?: boolean }> = [
    {
      id: "split",
      label: t("contextMenu.split"),
      action: () => {
        ensureSelected();
        void edit.splitAtPlayhead();
      },
    },
    {
      id: "delete",
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
      id: "unlink",
      label: t("contextMenu.unlink"),
      action: () => {
        ensureSelected();
        const ids = [...useEditorUiStore.getState().selectedClipIds];
        if (ids.length > 0) void edit.unlinkClips(ids);
      },
    });
  } else {
    items.push({
      id: "link",
      label: t("contextMenu.link"),
      action: () => {
        ensureSelected();
        const ids = [...useEditorUiStore.getState().selectedClipIds];
        if (ids.length >= 2) void edit.linkClips(ids);
      },
    });
  }

  // Swap Media: enabled only for non-text clips that are alone in their link
  // group (SPEC §5.10 "非 text 且单链组"). A multi-clip link group (e.g. linked
  // A/V pair) is disabled to avoid desyncing partners. On click, opens the
  // SwapMediaPicker modal (pre-filters candidates by strict type equality).
  const swapDisabled = clip.mediaType === "text" || !isSingleLinkGroup(clip, timeline);

  // Disabled placeholders for upcoming features (no action on click).
  items.push(
    {
      id: "swapMedia",
      label: t("contextMenu.swapMedia"),
      action: () => {
        ensureSelected();
        setPendingSwapClipId(clipId);
      },
      disabled: swapDisabled,
    },
    { id: "saveAsMedia", label: t("contextMenu.saveAsMedia"), action: () => {}, disabled: true },
    { id: "extractAudio", label: t("contextMenu.extractAudio"), action: () => {}, disabled: true },
  );

  return (
    <div
      ref={ref}
      style={{
        position: "fixed",
        left: pos.left,
        top: pos.top,
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
      {items.map((item) => (
        <button
          key={item.id}
          disabled={item.disabled}
          onClick={() => {
            item.action();
            onClose();
          }}
          style={{
            display: "block",
            width: "100%",
            padding: "6px 12px",
            textAlign: "left",
            color: item.danger
              ? "var(--accent-danger, #ff6b6b)"
              : item.disabled
                ? "var(--text-disabled, rgba(255,255,255,0.35))"
                : "var(--text-primary)",
            background: "transparent",
            border: "none",
            cursor: item.disabled ? "default" : "pointer",
            fontFamily: "var(--font-sans)",
            fontSize: "var(--fs-sm)",
            opacity: item.disabled ? 0.5 : 1,
          }}
          onMouseEnter={(e) => {
            if (!item.disabled) {
              (e.currentTarget as HTMLElement).style.background = "var(--bg-hover, rgba(255,255,255,0.08))";
            }
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

// Flip the menu to stay within the viewport horizontally. If the menu would
// overflow the right edge, render it to the left of the click point instead.
function flipLeft(left: number, menuWidth: number): number {
  if (left + menuWidth > window.innerWidth) {
    return Math.max(0, left - menuWidth);
  }
  return left;
}

// Flip the menu to stay within the viewport vertically. If the menu would
// overflow the bottom edge, render it above the click point instead.
function flipTop(top: number, menuHeight: number): number {
  if (top + menuHeight > window.innerHeight) {
    return Math.max(0, top - menuHeight);
  }
  return top;
}
