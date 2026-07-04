/**
 * SwapMediaSection — Inspector's "替换媒体" picker (SPEC §5.10, §6).
 *
 * Isolated into its own file so Inspector.tsx stays free of mediaStore
 * coupling (review #121 point 4: avoid touching the media area from the
 * Inspector). When #91 rewrites the media system, this is the single
 * touchpoint to update.
 *
 * Opens an inline media picker that lists every library asset of the SAME
 * type as the clip (strict type match, no isVisual leniency). Selecting one
 * fires `edit.swapMedia`, which preserves all editing attributes
 * (resetTrim=false: trim / speed / start / duration are untouched). Text
 * clips don't render this section (they have no source media to swap), and
 * the backend refuses type mismatches.
 *
 * Gate (SPEC §5.10, frontend-UI-1to1-SPEC.md:665): "非 text 且单链组" — the
 * entry is hidden when the clip is part of a multi-clip link group, because
 * swapping would cascade to every partner sharing the old mediaRef. Linked
 * clips should be swapped via the timeline right-click menu where the group
 * selection is explicit.
 */

import { useState } from "react";
import { RefreshCw } from "lucide-react";
import { Icon } from "../ui/Icon";
import { useProjectStore } from "../../store/projectStore";
import { useMediaStore } from "../../store/mediaStore";
import * as edit from "../../store/editActions";
import { type TFunction } from "../../i18n";
import type { Clip, MediaItem } from "../../lib/types";

/** A compact media-type badge label. */
function mediaTypeLabel(type: MediaItem["type"]): string {
  switch (type) {
    case "video":
      return "Video";
    case "audio":
      return "Audio";
    case "image":
      return "Image";
    case "text":
      return "Text";
    case "lottie":
      return "Lottie";
  }
}

export function SwapMediaSection({ clip, t }: { clip: Clip; t: TFunction }) {
  const [open, setOpen] = useState(false);
  const items = useMediaStore((s) => s.items);
  const timeline = useProjectStore((s) => s.timeline);

  // "单链组" gate: hide when the clip belongs to a link group with > 1 member.
  if (clip.linkGroupId) {
    const groupSize = timeline.tracks.reduce(
      (n, tr) => n + tr.clips.filter((c) => c.linkGroupId === clip.linkGroupId).length,
      0,
    );
    if (groupSize > 1) return null;
  }

  // Exclude the current media source; only assets of the SAME type are
  // candidates (the backend will refuse any other kind anyway).
  const candidates = items.filter(
    (m) => m.id !== clip.mediaRef && m.type === clip.mediaType,
  );

  const handlePick = (item: MediaItem) => {
    void edit.swapMedia(clip.id, item.id);
    setOpen(false);
  };

  return (
    <section>
      <button
        onClick={() => setOpen((v) => !v)}
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: "var(--space-xs)",
          fontSize: "var(--fs-sm)",
          color: open ? "var(--text-primary)" : "var(--text-tertiary)",
          background: "none",
          border: "none",
          cursor: "pointer",
          padding: 0,
        }}
      >
        <Icon icon={RefreshCw} size={12} />
        {t("inspector.swapMedia")}
      </button>

      {open && (
        <div
          style={{
            marginTop: "var(--space-sm)",
            maxHeight: 200,
            overflowY: "auto",
            borderRadius: "var(--radius-sm)",
            border: "var(--bw-thin) solid var(--border-primary)",
            background: "var(--bg-secondary)",
          }}
        >
          <div
            style={{
              padding: "var(--space-xs) var(--space-sm)",
              fontSize: "var(--fs-xxs)",
              fontWeight: "var(--fw-semibold)",
              letterSpacing: "var(--tracking-wide)",
              color: "var(--text-muted)",
              textTransform: "uppercase",
              borderBottom: "var(--bw-thin) solid var(--border-primary)",
            }}
          >
            {t("inspector.swapMediaTitle")}
          </div>
          {candidates.length === 0 ? (
            <div
              style={{
                padding: "var(--space-sm)",
                fontSize: "var(--fs-xs)",
                color: "var(--text-tertiary)",
              }}
            >
              {t("inspector.swapMediaEmpty")}
            </div>
          ) : (
            candidates.map((item) => (
              <button
                key={item.id}
                onClick={() => handlePick(item)}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  width: "100%",
                  padding: "var(--space-xs) var(--space-sm)",
                  fontSize: "var(--fs-xs)",
                  color: "var(--text-secondary)",
                  background: "none",
                  border: "none",
                  borderBottom: "var(--bw-thin) solid var(--border-primary)",
                  cursor: "pointer",
                  textAlign: "left",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = "var(--bg-hover)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = "none";
                }}
              >
                <span
                  style={{
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                    flex: 1,
                  }}
                >
                  {item.name || item.id}
                </span>
                <span style={{ color: "var(--text-muted)", marginLeft: "var(--space-sm)" }}>
                  {mediaTypeLabel(item.type)}
                </span>
              </button>
            ))
          )}
        </div>
      )}
    </section>
  );
}
