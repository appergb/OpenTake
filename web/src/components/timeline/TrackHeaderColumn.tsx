/**
 * Track header column (SPEC §5.5). Fixed 100px-wide left column: per-track color
 * strip, V1/A1 label, and the right-side toggles (sync-lock; mute for audio /
 * hide for visual). Follows vertical scroll only. Track-height drag adjusts the
 * UI-only displayHeight (not persisted).
 */

import { useCallback, useRef } from "react";
import { Eye, EyeOff, Volume2, VolumeX, Link, Unlink } from "lucide-react";
import { Icon } from "../ui/Icon";
import { useT } from "../../i18n";
import { LAYOUT, TRACK_SIZE } from "../../lib/theme";
import { trackColor } from "../../lib/clip";
import { trackDisplayLabel, firstAudioIndex } from "../../lib/zones";
import { trackDisplayHeight } from "../../lib/geometry";
import { useEditorUiStore } from "../../store/uiStore";
import { setTrackProps } from "../../store/editActions";
import type { Timeline } from "../../lib/types";

interface Props {
  timeline: Timeline;
  scrollTop: number;
  totalHeight: number;
}

export function TrackHeaderColumn({ timeline, scrollTop, totalHeight }: Props) {
  const trackHeights = useEditorUiStore((s) => s.trackDisplayHeights);
  const setTrackHeight = useEditorUiStore((s) => s.setTrackHeight);
  const firstAudio = firstAudioIndex(timeline);

  return (
    <div
      style={{
        position: "absolute",
        top: 0,
        left: 0,
        width: LAYOUT.trackHeaderWidth,
        height: "100%",
        background: "var(--bg-surface)",
        borderRight: "var(--bw-thin) solid var(--border-primary)",
        overflow: "hidden",
        zIndex: 20,
      }}
    >
      {/* Top spacer aligned with the ruler. */}
      <div
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          right: 0,
          height: LAYOUT.rulerHeight,
          background: "var(--bg-surface)",
          borderBottom: "var(--bw-thin) solid var(--border-primary)",
          zIndex: 2,
        }}
      />
      {/* Scrolled content. */}
      <div
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          right: 0,
          height: totalHeight,
          transform: `translateY(${-scrollTop}px)`,
        }}
      >
        {timeline.tracks.map((track, i) => {
          const top = trackTop(timeline, i, trackHeights);
          const h = trackDisplayHeight(track, trackHeights);
          return (
            <TrackHeaderRow
              key={track.id || i}
              index={i}
              label={trackDisplayLabel(timeline, i)}
              color={trackColor(track.type)}
              top={top}
              height={h}
              isAudio={track.type === "audio"}
              muted={track.muted}
              hidden={track.hidden}
              syncLocked={track.syncLocked}
              regionDivider={firstAudio > 0 && i === firstAudio}
              onResize={(delta) => {
                const next = Math.max(
                  TRACK_SIZE.minHeight,
                  Math.min(TRACK_SIZE.maxHeight, h + delta),
                );
                setTrackHeight(track.id, next);
              }}
            />
          );
        })}
      </div>
    </div>
  );
}

function trackTop(
  timeline: Timeline,
  i: number,
  heights: Record<string, number>,
): number {
  let y = LAYOUT.rulerHeight + LAYOUT.dropZoneHeight;
  for (let k = 0; k < i; k++) y += trackDisplayHeight(timeline.tracks[k], heights);
  return y;
}

interface RowProps {
  index: number;
  label: string;
  color: string;
  top: number;
  height: number;
  isAudio: boolean;
  muted: boolean;
  hidden: boolean;
  syncLocked: boolean;
  regionDivider: boolean;
  onResize: (delta: number) => void;
}

function TrackHeaderRow(p: RowProps) {
  const t = useT();
  const dragRef = useRef<{ startY: number } | null>(null);

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      e.stopPropagation();
      dragRef.current = { startY: e.clientY };
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    },
    [],
  );
  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!dragRef.current) return;
      const delta = e.clientY - dragRef.current.startY;
      dragRef.current.startY = e.clientY;
      p.onResize(delta);
    },
    [p],
  );
  const onPointerUp = useCallback((e: React.PointerEvent) => {
    dragRef.current = null;
    (e.target as HTMLElement).releasePointerCapture(e.pointerId);
  }, []);

  const iconColor = (active: boolean) =>
    active ? "var(--text-secondary)" : "rgba(255,255,255,0.186)"; // 0.62*0.3

  return (
    <div
      style={{
        position: "absolute",
        top: p.top,
        left: 0,
        right: 0,
        height: p.height,
        borderTop: "var(--bw-thin) solid var(--border-primary)",
        ...(p.regionDivider
          ? { borderTop: "var(--bw-thick) solid var(--border-divider)" }
          : {}),
        display: "flex",
        alignItems: "center",
      }}
    >
      {/* Left color strip. */}
      <div style={{ width: 3, height: "100%", background: p.color, flex: "0 0 auto" }} />
      {/* Label. */}
      <span
        style={{
          marginLeft: 6,
          fontSize: "var(--fs-sm)",
          fontWeight: "var(--fw-medium)",
          color: "var(--text-secondary)",
          flex: 1,
        }}
      >
        {p.label}
      </span>
      {/* Toggles. Clicking dispatches SetTrackProps (toggles the field). */}
      <div style={{ display: "flex", alignItems: "center", gap: 2, paddingRight: 4 }}>
        {p.isAudio ? (
          <span
            title={t("timeline.mute")}
            role="button"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={() => void setTrackProps(p.index, { muted: !p.muted })}
            style={{ color: iconColor(!p.muted), display: "inline-flex", cursor: "pointer" }}
          >
            <Icon icon={p.muted ? VolumeX : Volume2} size={11} />
          </span>
        ) : (
          <span
            title={t("timeline.hide")}
            role="button"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={() => void setTrackProps(p.index, { hidden: !p.hidden })}
            style={{ color: iconColor(!p.hidden), display: "inline-flex", cursor: "pointer" }}
          >
            <Icon icon={p.hidden ? EyeOff : Eye} size={11} />
          </span>
        )}
        <span
          title={t("timeline.syncLock")}
          role="button"
          onPointerDown={(e) => e.stopPropagation()}
          onClick={() => void setTrackProps(p.index, { syncLocked: !p.syncLocked })}
          style={{ color: iconColor(p.syncLocked), display: "inline-flex", cursor: "pointer" }}
        >
          <Icon icon={p.syncLocked ? Link : Unlink} size={11} />
        </span>
      </div>
      {/* Bottom resize grip. */}
      <div
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        style={{
          position: "absolute",
          left: 0,
          right: 0,
          bottom: -TRACK_SIZE.resizeHandleZone / 2,
          height: TRACK_SIZE.resizeHandleZone,
          cursor: "ns-resize",
        }}
      />
    </div>
  );
}
