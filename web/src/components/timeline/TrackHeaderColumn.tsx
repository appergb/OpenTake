/**
 * Track header column (SPEC §5.5). Fixed 100px-wide left column: per-track color
 * strip, V1/A1 label, and the right-side toggles (sync-lock; mute for audio /
 * hide for visual). Follows vertical scroll only. Track-height drag adjusts the
 * UI-only displayHeight (not persisted).
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Eye, EyeOff, Volume2, VolumeX, Link, Unlink } from "lucide-react";
import { Icon } from "../ui/Icon";
import { useT } from "../../i18n";
import { LAYOUT, TRACK_SIZE } from "../../lib/theme";
import { trackColor } from "../../lib/clip";
import { trackDisplayLabel, firstAudioIndex } from "../../lib/zones";
import { trackDisplayHeight } from "../../lib/geometry";
import { useEditorUiStore } from "../../store/uiStore";
import { setTrackProps, swapTracks } from "../../store/editActions";
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
              trackId={track.id}
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
              canSwapUp={i > 0 && timeline.tracks[i - 1].type === track.type}
              canSwapDown={i + 1 < timeline.tracks.length && timeline.tracks[i + 1].type === track.type}
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
  trackId: string;
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
  canSwapUp: boolean;
  canSwapDown: boolean;
  onResize: (delta: number) => void;
}

const TRACK_RESIZE_HIT_TARGET = 24;

function TrackHeaderRow(p: RowProps) {
  const t = useT();
  const pushToast = useEditorUiStore((s) => s.pushToast);
  const rowRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{
    startY: number;
    pointerId: number;
    target: HTMLElement;
  } | null>(null);
  const [isResizing, setIsResizing] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  const closeMenu = useCallback(() => {
    setMenu(null);
    rowRef.current?.focus();
  }, []);

  const finishResize = useCallback((releaseCapture: boolean) => {
    const drag = dragRef.current;
    if (!drag) return;
    dragRef.current = null;
    setIsResizing(false);
    if (releaseCapture) {
      try {
        drag.target.releasePointerCapture(drag.pointerId);
      } catch {
        // Capture can already be gone when the browser ends the gesture.
      }
    }
  }, []);

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const target = e.currentTarget as HTMLElement;
      dragRef.current = { startY: e.clientY, pointerId: e.pointerId, target };
      setIsResizing(true);
      target.focus();
      target.setPointerCapture(e.pointerId);
    },
    [],
  );
  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== e.pointerId) return;
      const delta = e.clientY - drag.startY;
      drag.startY = e.clientY;
      p.onResize(delta);
    },
    [p],
  );
  const onPointerUp = useCallback(
    (e: React.PointerEvent) => {
      if (dragRef.current?.pointerId !== e.pointerId) return;
      finishResize(true);
    },
    [finishResize],
  );
  const onKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      let delta: number | null = null;
      if (e.key === "ArrowUp") delta = -10;
      else if (e.key === "ArrowDown") delta = 10;
      else if (e.key === "Home") delta = TRACK_SIZE.minHeight - p.height;
      else if (e.key === "End") delta = TRACK_SIZE.maxHeight - p.height;
      else if (e.key === "Escape" && dragRef.current) {
        e.preventDefault();
        finishResize(true);
        return;
      }
      if (delta === null) return;
      e.preventDefault();
      p.onResize(delta);
    },
    [finishResize, p],
  );

  const iconColor = (active: boolean) =>
    active ? "var(--text-secondary)" : "rgba(255,255,255,0.186)"; // 0.62*0.3

  const updateTrack = (properties: { muted?: boolean; hidden?: boolean; syncLocked?: boolean }) => {
    void setTrackProps(p.index, properties).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      pushToast(t("timeline.trackUpdateFailed", { error: message }));
    });
  };

  return (
    <div
      ref={rowRef}
      data-track-row={p.trackId}
      tabIndex={0}
      aria-label={p.label}
      onContextMenu={(e) => {
        e.preventDefault();
        setMenu({ x: e.clientX, y: e.clientY });
      }}
      onKeyDown={(e) => {
        if (e.currentTarget !== e.target) return;
        if (e.key !== "ContextMenu" && !(e.shiftKey && e.key === "F10")) return;
        e.preventDefault();
        const rect = e.currentTarget.getBoundingClientRect();
        setMenu({ x: rect.left + 8, y: rect.top + 8 });
      }}
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
          <button
            type="button"
            data-track-action="mute"
            data-track-index={p.index}
            aria-label={t("timeline.mute")}
            aria-pressed={p.muted}
            title={t("timeline.mute")}
            onPointerDown={(e) => e.stopPropagation()}
            onClick={() => updateTrack({ muted: !p.muted })}
            className="hover-area"
            style={{
              width: 24,
              height: 24,
              color: iconColor(!p.muted),
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              cursor: "pointer",
            }}
          >
            <Icon icon={p.muted ? VolumeX : Volume2} size={11} />
          </button>
        ) : (
          <button
            type="button"
            data-track-action="hide"
            data-track-index={p.index}
            aria-label={t("timeline.hide")}
            aria-pressed={p.hidden}
            title={t("timeline.hide")}
            onPointerDown={(e) => e.stopPropagation()}
            onClick={() => updateTrack({ hidden: !p.hidden })}
            className="hover-area"
            style={{
              width: 24,
              height: 24,
              color: iconColor(!p.hidden),
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              cursor: "pointer",
            }}
          >
            <Icon icon={p.hidden ? EyeOff : Eye} size={11} />
          </button>
        )}
        <button
          type="button"
          data-track-action="sync-lock"
          data-track-index={p.index}
          aria-label={t("timeline.syncLock")}
          aria-pressed={p.syncLocked}
          title={t("timeline.syncLock")}
          onPointerDown={(e) => e.stopPropagation()}
          onClick={() => updateTrack({ syncLocked: !p.syncLocked })}
          className="hover-area"
          style={{
            width: 24,
            height: 24,
            color: iconColor(p.syncLocked),
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            cursor: "pointer",
          }}
        >
          <Icon icon={p.syncLocked ? Link : Unlink} size={11} />
        </button>
      </div>
      {menu &&
        createPortal(
          <TrackHeaderContextMenu
            x={menu.x}
            y={menu.y}
            canSwapUp={p.canSwapUp}
            canSwapDown={p.canSwapDown}
            onSwapUp={() => void swapTracks(p.index, p.index - 1)}
            onSwapDown={() => void swapTracks(p.index, p.index + 1)}
            onClose={closeMenu}
            menuLabel={p.label}
            labels={{
              moveUp: t("timeline.moveTrackUp"),
              moveDown: t("timeline.moveTrackDown"),
            }}
          />,
          document.body,
        )}
      {/* Bottom resize grip. */}
      <div
        data-track-resize={p.trackId}
        data-interaction-state={isResizing ? "dragging" : "enabled"}
        role="separator"
        aria-label={p.label}
        aria-orientation="horizontal"
        aria-valuemin={TRACK_SIZE.minHeight}
        aria-valuemax={TRACK_SIZE.maxHeight}
        aria-valuenow={p.height}
        tabIndex={0}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => finishResize(true)}
        onLostPointerCapture={() => finishResize(false)}
        onKeyDown={onKeyDown}
        style={{
          position: "absolute",
          left: 0,
          // Keep the 24px drag target out from under the two 24px track
          // buttons; the label-side segment remains a large resize affordance.
          right: 56,
          bottom: -TRACK_RESIZE_HIT_TARGET / 2,
          height: TRACK_RESIZE_HIT_TARGET,
          // Later track rows are rendered after this row and otherwise win the
          // real browser hit test over the lower half of this 24px target.
          zIndex: 3,
          cursor: "ns-resize",
        }}
      />
    </div>
  );
}

function TrackHeaderContextMenu({
  x,
  y,
  canSwapUp,
  canSwapDown,
  onSwapUp,
  onSwapDown,
  onClose,
  menuLabel,
  labels,
}: {
  x: number;
  y: number;
  canSwapUp: boolean;
  canSwapDown: boolean;
  onSwapUp: () => void;
  onSwapDown: () => void;
  onClose: () => void;
  menuLabel: string;
  labels: { moveUp: string; moveDown: string };
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  const items = [
    { label: labels.moveUp, enabled: canSwapUp, action: onSwapUp },
    { label: labels.moveDown, enabled: canSwapDown, action: onSwapDown },
  ];

  useEffect(() => {
    const firstEnabled = menuRef.current?.querySelector<HTMLButtonElement>(
      "button:not(:disabled)",
    );
    (firstEnabled ?? menuRef.current)?.focus();
  }, []);

  const moveFocus = (direction: 1 | -1) => {
    const enabled = Array.from(
      menuRef.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [],
    );
    if (enabled.length === 0) return;
    const current = enabled.indexOf(document.activeElement as HTMLButtonElement);
    const next = current < 0
      ? direction === 1 ? 0 : enabled.length - 1
      : (current + direction + enabled.length) % enabled.length;
    enabled[next].focus();
  };

  return (
    <div
      data-track-menu-backdrop
      onMouseDown={onClose}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }}
      style={{ position: "fixed", inset: 0, zIndex: 1000 }}
    >
      <div
        ref={menuRef}
        aria-label={menuLabel}
        tabIndex={-1}
        style={{
          position: "fixed",
          left: x,
          top: y,
          minWidth: 136,
          padding: 4,
          background: "var(--bg-elevated)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: 6,
          boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
          fontSize: "var(--fs-sm)",
        }}
        role="menu"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          } else if (e.key === "ArrowDown") {
            e.preventDefault();
            moveFocus(1);
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            moveFocus(-1);
          } else if (e.key === "Home") {
            e.preventDefault();
            menuRef.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
          } else if (e.key === "End") {
            e.preventDefault();
            const enabled = menuRef.current?.querySelectorAll<HTMLButtonElement>(
              "button:not(:disabled)",
            );
            enabled?.[enabled.length - 1]?.focus();
          }
        }}
      >
        {items.map((item) => (
          <button
            key={item.label}
            disabled={!item.enabled}
            onClick={() => {
              item.action();
              onClose();
            }}
            style={{
              display: "block",
              width: "100%",
              padding: "6px 12px",
              textAlign: "left",
              color: item.enabled ? "var(--text-primary)" : "var(--text-muted)",
              background: "transparent",
              border: "none",
              cursor: item.enabled ? "pointer" : "default",
              fontFamily: "var(--font-sans)",
              fontSize: "var(--fs-sm)",
            }}
            role="menuitem"
            onMouseEnter={(e) => {
              if (item.enabled) e.currentTarget.style.background = "var(--bg-hover, rgba(255,255,255,0.08))";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = "transparent";
            }}
          >
            {item.label}
          </button>
        ))}
      </div>
    </div>
  );
}
