/**
 * ClipContextMenu (SPEC §5.8). Right-click menu for timeline clips. Appears AT
 * the cursor (`x`/`y`, flipped to stay inside the viewport) and closes on
 * outside click, Escape, or item action.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useClipboardStore } from "../../store/clipboardStore";
import * as edit from "../../store/editActions";
import { useT } from "../../i18n";
import type { Clip, ClipPropertiesReq, Interpolation } from "../../lib/types";
import type { TimelineRange } from "../../lib/timelineRange";
import type { FadeEdge } from "./hitTest";
import { rangeContextMenuItems } from "./TimelineRangeContextMenu";

type MenuItem = {
  label: string;
  action: () => void;
  danger?: boolean;
  checked?: boolean;
};

type FadeInterpolation = Extract<Interpolation, "linear" | "smooth">;

const FADE_INTERPOLATIONS: Array<{ label: string; value: FadeInterpolation }> = [
  { label: "Linear", value: "linear" },
  { label: "Smooth", value: "smooth" },
];
const CHECKMARK = "\u2713";

type ClipMenuLabels = {
  copy: string;
  paste: string;
  split: string;
  delete: string;
  link: string;
  unlink: string;
  swapMedia: string;
  saveAsMedia: string;
  freezeFrame: string;
  reverse: string;
  reverseOn: string;
  reverseTooLong: string;
};

export function clipContextMenuItems({
  clip,
  hasClipboardContent,
  labels,
  ensureSelected,
  selectedClipIds,
  onCopy,
  onPaste,
  onSplit,
  onDelete,
  onLink,
  onUnlink,
  onSwapMedia,
  onSaveAsMedia,
  onFreezeFrame,
  onReverse,
  reverseInfo,
}: {
  clip: Clip;
  hasClipboardContent: boolean;
  labels: ClipMenuLabels;
  ensureSelected: () => void;
  selectedClipIds: () => string[];
  onCopy: () => void;
  onPaste: () => void | Promise<void>;
  onSplit: () => void | Promise<void>;
  onDelete: () => void | Promise<void>;
  onLink: (ids: string[]) => void | Promise<void>;
  onUnlink: (ids: string[]) => void | Promise<void>;
  onSwapMedia: () => void;
  onSaveAsMedia: () => void;
  onFreezeFrame: () => void | Promise<void>;
  onReverse: () => void;
  reverseInfo: { isReversed: boolean; tooLong: boolean };
}): MenuItem[] {
  const items: MenuItem[] = [
    {
      label: labels.copy,
      action: () => {
        ensureSelected();
        onCopy();
      },
    },
  ];

  if (hasClipboardContent) {
    items.push({
      label: labels.paste,
      action: () => {
        void onPaste();
      },
    });
  }

  items.push(
    {
      label: labels.split,
      action: () => {
        ensureSelected();
        void onSplit();
      },
    },
    {
      label: labels.delete,
      action: () => {
        ensureSelected();
        void onDelete();
      },
      danger: true,
    },
  );

  // Link/Unlink: operate on the full selection (>= 2 clips to link).
  if (clip.linkGroupId) {
    items.push({
      label: labels.unlink,
      action: () => {
        ensureSelected();
        const ids = selectedClipIds();
        if (ids.length > 0) void onUnlink(ids);
      },
    });
  } else {
    items.push({
      label: labels.link,
      action: () => {
        ensureSelected();
        const ids = selectedClipIds();
        if (ids.length >= 2) void onLink(ids);
      },
    });
  }

  if (clip.mediaType === "video" || clip.mediaType === "image") {
    items.push({
      label: labels.swapMedia,
      action: () => {
        ensureSelected();
        onSwapMedia();
      },
    });

    items.push({
      label: labels.freezeFrame,
      action: () => {
        ensureSelected();
        void onFreezeFrame();
      },
    });
  }

  // Reverse stays video-only.
  if (clip.mediaType === "video") {
    const tooLongAndNotReversed = reverseInfo.tooLong && !reverseInfo.isReversed;
    items.push({
      label: tooLongAndNotReversed
        ? labels.reverseTooLong
        : reverseInfo.isReversed
          ? labels.reverseOn
          : labels.reverse,
      checked: reverseInfo.isReversed,
      action: () => {
        if (tooLongAndNotReversed) return;
        ensureSelected();
        onReverse();
      },
    });
  }

  if (clip.mediaType === "video" || clip.mediaType === "audio") {
    items.push({
      label: labels.saveAsMedia,
      action: () => {
        ensureSelected();
        onSaveAsMedia();
      },
    });
  }

  return items;
}

export function fadeInterpolationMenuItems(
  clip: Clip,
  edge: FadeEdge,
  apply: (properties: ClipPropertiesReq) => void,
  labels: Partial<Record<FadeInterpolation, string>> = {},
): MenuItem[] {
  const current = edge === "left" ? clip.fadeInInterpolation : clip.fadeOutInterpolation;
  return FADE_INTERPOLATIONS.map(({ label, value }) => ({
    label: labels[value] ?? label,
    checked: current === value,
    action: () => {
      apply(edge === "left" ? { fadeInInterpolation: value } : { fadeOutInterpolation: value });
    },
  }));
}

export function ClipContextMenu({
  clipId,
  fadeEdge,
  x,
  y,
  range,
  onClose,
}: {
  clipId: string;
  fadeEdge?: FadeEdge;
  x: number;
  y: number;
  range?: TimelineRange;
  onClose: () => void;
}) {
  const t = useT();
  const rootTimeline = useProjectStore((s) => s.timeline);
  const activeNestedSequenceId = useEditorUiStore((s) => s.activeNestedSequenceId);
  const enterNestedSequence = useEditorUiStore((s) => s.enterNestedSequence);
  const timeline =
    rootTimeline.nestedSequences?.find(
      (sequence) => sequence.id === activeNestedSequenceId,
    )?.timeline ?? rootTimeline;
  const selectedClipIds = useEditorUiStore((s) => s.selectedClipIds);
  const selectClips = useEditorUiStore((s) => s.selectClips);
  const pushToast = useEditorUiStore((s) => s.pushToast);
  const setPendingSwapClipId = useEditorUiStore((s) => s.setPendingSwapClipId);
  const hasClipboardContent = useClipboardStore((s) => s.hasContent);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });

  // Locate the full clip for link state and fade interpolation state.
  let clip: Clip | null = null;
  for (const track of timeline.tracks) {
    const found = track.clips.find((c) => c.id === clipId);
    if (found) {
      clip = found;
      break;
    }
  }
  const clipMissing = !clip;

  // Close on outside click / Escape, and close (via effect, NOT during render) if
  // the target clip was removed out from under the menu.
  useEffect(() => {
    if (clipMissing) {
      onClose();
      return;
    }
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
  }, [onClose, clipMissing]);

  // Place the menu at the cursor, flipping left/up when it would overflow the
  // viewport so it's never clipped off-screen.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const margin = 8;
    let left = x;
    let top = y;
    if (left + width + margin > window.innerWidth) left = Math.max(margin, x - width);
    if (top + height + margin > window.innerHeight) top = Math.max(margin, y - height);
    setPos({ left, top });
  }, [x, y, clipMissing]);

  if (!clip) return null;
  const currentClip: Clip = clip;

  // The menu acts on the current selection; if the right-clicked clip isn't
  // selected, select just it (mirrors typical NLE behavior).
  const isSelected = selectedClipIds.has(clipId);
  const ensureSelected = () => {
    if (!isSelected) selectClips(new Set([clipId]));
  };

  let items: MenuItem[];
  if (fadeEdge) {
    items = fadeInterpolationMenuItems(
      currentClip,
      fadeEdge,
      (properties) => {
        void edit.setClipProperties([clipId], properties);
      },
      {
        linear: t("inspector.interpolation.linear"),
        smooth: t("inspector.interpolation.smooth"),
      },
    );
  } else {
    items = clipContextMenuItems({
      clip: clip!,
      hasClipboardContent,
      labels: {
        copy: t("contextMenu.copy"),
        paste: t("contextMenu.paste"),
        split: t("contextMenu.split"),
        delete: t("contextMenu.delete"),
        link: t("contextMenu.link"),
        unlink: t("contextMenu.unlink"),
        swapMedia: t("contextMenu.swapMedia"),
        saveAsMedia: t("contextMenu.saveAsMedia"),
        freezeFrame: t("contextMenu.freezeFrame"),
        reverse: t("contextMenu.reverse"),
        reverseOn: t("contextMenu.reverseOn"),
        reverseTooLong: t("contextMenu.reverseTooLong"),
      },
      ensureSelected,
      selectedClipIds: () => [...useEditorUiStore.getState().selectedClipIds],
      onCopy: edit.copyClips,
      onPaste: edit.pasteClipsAtPlayhead,
      onSplit: async () => {
        try {
          await edit.splitAtPlayhead();
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          pushToast(t("timeline.splitFailed", { error: message }));
        }
      },
      onDelete: edit.deleteSelectedClips,
      onLink: edit.linkClips,
      onUnlink: edit.unlinkClips,
      onSwapMedia: () => setPendingSwapClipId(clipId),
      onSaveAsMedia: () => void edit.saveClipAsMedia(clipId),
      onFreezeFrame: async () => {
        const input = window.prompt(
          t("contextMenu.freezeFramePrompt"),
          String(edit.DEFAULT_FREEZE_FRAMES),
        );
        if (input == null) return;
        const frames = parseInt(input, 10);
        if (!Number.isFinite(frames) || frames < 1) return;
        await edit.freezeClipAtPlayhead(currentClip, frames);
      },
      onReverse: () => {
        void edit.setClipProperties([clipId], { reversed: !currentClip.reversed });
      },
      reverseInfo: {
        isReversed: Boolean(currentClip.reversed),
        tooLong: currentClip.durationFrames > Math.round(timeline.fps * 60),
      },
    });
    if (activeNestedSequenceId) {
      const rootOnlyLabels = new Set([
        t("contextMenu.freezeFrame"),
        t("contextMenu.saveAsMedia"),
      ]);
      items = items.filter((item) => !rootOnlyLabels.has(item.label));
    }
    if (currentClip.nestedSequenceId) {
      const mediaOnlyLabels = new Set([
        t("contextMenu.swapMedia"),
        t("contextMenu.freezeFrame"),
        t("contextMenu.reverse"),
        t("contextMenu.reverseOn"),
        t("contextMenu.reverseTooLong"),
        t("contextMenu.saveAsMedia"),
      ]);
      items = items.filter((item) => !mediaOnlyLabels.has(item.label));
      items.unshift({
        label: t("contextMenu.openCompound"),
        action: () => enterNestedSequence(currentClip.nestedSequenceId!),
      });
      if (!activeNestedSequenceId) {
        items.push({
          label: t("contextMenu.dissolveCompound"),
          action: () => void edit.dissolveNestedSequence(clipId),
        });
      }
    } else if (!activeNestedSequenceId) {
      items.push({
        label: t("contextMenu.createCompound"),
        action: () => {
          ensureSelected();
          void edit.createNestedSequence(t("compound.defaultName"));
        },
      });
    }
  }
  if (range) {
    items.push(
      ...rangeContextMenuItems({
        range,
        labels: {
          save: t("contextMenu.saveRangeAsMedia"),
          clear: t("contextMenu.clearRange"),
        },
        onSave: edit.saveMarkedRangeAsMedia,
        onClear: useEditorUiStore.getState().clearTimelineRange,
      }),
    );
  }

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
      role="menu"
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
          role={item.checked === undefined ? "menuitem" : "menuitemradio"}
          aria-checked={item.checked ?? undefined}
          onMouseEnter={(e) => {
            (e.currentTarget as HTMLElement).style.background = "var(--bg-hover, rgba(255,255,255,0.08))";
          }}
          onMouseLeave={(e) => {
            (e.currentTarget as HTMLElement).style.background = "transparent";
          }}
        >
          {item.checked === undefined ? item.label : `${item.checked ? `${CHECKMARK} ` : "  "}${item.label}`}
        </button>
      ))}
    </div>
  );
}
