/**
 * KeyframesLaneRow (SPEC §6.4). One animatable property row inside the
 * KeyframesPanel. Renders a label + stamp/clear buttons, and a track strip
 * with draggable diamond markers (one per keyframe).
 *
 * Interaction model:
 *  - Click empty track area → seek the playhead to that frame.
 *  - Click a diamond → starts a drag (window mousemove/mouseup); the diamond
 *    follows the cursor in real time, snaps to the nearest of {playhead, clip
 *    start/end, every OTHER property's keyframes} within ±5 frames (1:1 port
 *    of upstream `KeyframesLaneRow.applySnap` / `snapTargets`,
 *    Inspector/Keyframes/KeyframesLane.swift:177-216), and commits via
 *    `edit.moveKeyframe` on mouseup.
 *  - Right-click a diamond → context menu (delete / set interpolation).
 *  - Stamp button → `edit.stampKeyframe` at the current playhead.
 *  - Clear button → `edit.setKeyframes` with an empty keyframe array.
 *
 * Diamond rendering: HTML divs positioned with `left: %` and rotated 45°, NOT
 * SVG polygons. SVG `polygon points` cannot take percentage strings, and the
 * `viewBox + preserveAspectRatio="none"` alternative distorts the diamonds
 * (non-uniform x/y scaling). HTML divs with `transform: rotate(45deg)` produce
 * correctly-proportioned diamonds at any track width and scale naturally.
 */

import { useState, useRef, useCallback, useEffect } from "react";
import { useEditorUiStore } from "../../store/uiStore";
import * as edit from "../../store/editActions";
import { snapFrame } from "../../lib/keyframeSnap";
import type {
  AnimPair,
  Clip,
  Crop,
  Interpolation,
  KeyframeProperty,
  KeyframeTrack,
} from "../../lib/types";
import type { TFunction } from "../../i18n";

const DIAMOND_SIZE = 8;
const SNAP_FRAMES = 5;
const LANE_HEIGHT = 24;

/** Union of all concrete keyframe-track value types (mirror of Clip's *Track
 *  fields). Used so `getTrack` can return a single typed union. */
type AnyKeyframeTrack =
  | KeyframeTrack<number>
  | KeyframeTrack<AnimPair>
  | KeyframeTrack<Crop>;

/** All animatable properties, used to collect cross-property snap targets
 *  regardless of which subset (video vs audio) is currently rendered as rows. */
const ALL_PROPERTIES: KeyframeProperty[] = [
  "opacity",
  "volume",
  "rotation",
  "position",
  "scale",
  "crop",
];

/** Absolute-frame snap targets from every OTHER property's keyframes on this
 *  clip, plus the clip's own start/end. Excludes `property` (the row being
 *  dragged) so a keyframe never snaps to a sibling on its own track — matches
 *  upstream's `for p in AnimatableProperty.allCases where p != property`
 *  (KeyframesLane.swift:210-214). Playhead is added separately by the caller
 *  since it isn't a track-derived target. */
function crossPropertyAndBoundTargets(clip: Clip, property: KeyframeProperty): number[] {
  const targets: number[] = [clip.startFrame, clip.startFrame + clip.durationFrames];
  for (const p of ALL_PROPERTIES) {
    if (p === property) continue;
    const otherTrack = getTrack(clip, p);
    if (!otherTrack) continue;
    for (const kf of otherTrack.keyframes) {
      targets.push(kf.frame + clip.startFrame);
    }
  }
  return targets;
}

export function KeyframesLaneRow({
  clip,
  property,
  t,
  onSnapChange,
}: {
  clip: Clip;
  property: KeyframeProperty;
  t: TFunction;
  /** Reports the absolute frame currently snapped to during a drag (or null
   *  when not snapped / not dragging) so KeyframesPanel can render a
   *  panel-wide yellow snap-guide line. */
  onSnapChange?: (absFrame: number | null) => void;
}) {
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const setActiveFrame = useEditorUiStore((s) => s.setActiveFrame);
  const track = getTrack(clip, property);
  const trackRef = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState<{ fromFrame: number; currentFrame: number } | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; frame: number } | null>(null);
  /** Holds the cleanup function for the active drag's window listeners.
   *  Cleared on unmount via the useEffect below to prevent leaks. */
  const dragCleanupRef = useRef<(() => void) | null>(null);

  // Unmount safety: remove any active drag listeners and clear any snap line
  // the panel may be showing on our behalf (otherwise a mid-drag unmount —
  // e.g. deselecting the clip — would leave a stale yellow line onscreen).
  useEffect(() => {
    return () => {
      dragCleanupRef.current?.();
      dragCleanupRef.current = null;
      onSnapChange?.(null);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const startFrame = clip.startFrame;
  const duration = Math.max(1, clip.durationFrames);

  // Frame → ratio (0..1) for diamond positioning.
  const frameToRatio = useCallback((frame: number) => frame / duration, [duration]);
  // Client X → clip-relative frame (rounded), clamped to [0, duration].
  const xToFrame = useCallback(
    (clientX: number) => {
      const el = trackRef.current;
      if (!el) return 0;
      const rect = el.getBoundingClientRect();
      const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
      return Math.round(ratio * duration);
    },
    [duration],
  );

  const handleStamp = () => {
    void edit.stampKeyframe(clip.id, property, activeFrame);
  };

  // Clear the whole track — kind depends on the property's value type.
  const handleClear = () => {
    if (property === "position" || property === "scale") {
      void edit.setKeyframes(clip.id, property, { kind: "pair", keyframes: [] });
    } else if (property === "crop") {
      void edit.setKeyframes(clip.id, property, { kind: "crop", keyframes: [] });
    } else {
      void edit.setKeyframes(clip.id, property, { kind: "scalar", keyframes: [] });
    }
  };

  // Click empty track → seek playhead. Skipped when clicking a diamond (child).
  const handleTrackClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget) return;
    const rel = xToFrame(e.clientX);
    setActiveFrame(startFrame + rel);
  };

  // Start a keyframe drag. Uses window listeners so the drag continues even
  // when the cursor leaves the track (matches the app's existing drag pattern).
  // The cleanup ref ensures listeners are removed if the component unmounts
  // mid-drag (e.g. user deselects the clip).
  const handleDiamondMouseDown = (e: React.MouseEvent<HTMLDivElement>, absFrame: number) => {
    e.stopPropagation();
    e.preventDefault();
    setDragging({ fromFrame: absFrame, currentFrame: absFrame });
    // Clamp to [startFrame, startFrame + duration - 1] (half-open clip range).
    const lastFrame = startFrame + duration - 1;
    // Cross-property + clip-bound targets are stable for the whole drag (they
    // don't depend on the cursor). Playhead is read fresh per-move below since
    // it's the one target that could (in theory) change during a drag.
    const boundTargets = crossPropertyAndBoundTargets(clip, property);
    const onMove = (ev: globalThis.MouseEvent) => {
      const rel = xToFrame(ev.clientX);
      let newFrame = startFrame + rel;
      // Clamp to valid clip range.
      newFrame = Math.max(startFrame, Math.min(lastFrame, newFrame));
      // Snap to the nearest of {playhead, clip start/end, other properties'
      // keyframes} within SNAP_FRAMES (upstream KeyframesLane.swift:177-216).
      const { frame: snapped, snappedTo } = snapFrame(
        newFrame,
        [...boundTargets, activeFrame],
        SNAP_FRAMES,
      );
      newFrame = snapped;
      onSnapChange?.(snappedTo);
      setDragging((d) => (d ? { ...d, currentFrame: newFrame } : d));
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      dragCleanupRef.current = null;
      onSnapChange?.(null);
      setDragging((d) => {
        if (d && d.fromFrame !== d.currentFrame) {
          void edit.moveKeyframe(clip.id, property, d.fromFrame, d.currentFrame);
        }
        return null;
      });
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    dragCleanupRef.current = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  };

  const handleDiamondContextMenu = (e: React.MouseEvent<HTMLDivElement>, absFrame: number) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, frame: absFrame });
  };

  const closeMenu = () => setMenu(null);

  const handleDelete = (frame: number) => {
    void edit.removeKeyframe(clip.id, property, frame);
    closeMenu();
  };

  const handleSetInterpolation = (frame: number, interp: Interpolation) => {
    void edit.setKeyframeInterpolation(clip.id, property, frame, interp);
    closeMenu();
  };

  const keyframes = track?.keyframes ?? [];
  const propertyLabel = t(`inspector.keyframes.property.${property}`);

  // Build display list: apply the live drag offset to the dragged keyframe so
  // it follows the cursor before the commit fires.
  const displayKeyframes = keyframes.map((kf) => {
    const absFrame = kf.frame + startFrame;
    if (dragging && absFrame === dragging.fromFrame) {
      return { key: absFrame, frame: dragging.currentFrame - startFrame, isDragging: true };
    }
    return { key: absFrame, frame: kf.frame, isDragging: false };
  });

  return (
    <div style={{ position: "relative" }}>
      {/* Label + action buttons */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          height: 20,
          marginBottom: 2,
        }}
      >
        <span
          style={{
            fontSize: "var(--fs-xxs)",
            color: "var(--text-muted)",
            textTransform: "uppercase",
            letterSpacing: "var(--tracking-wide)",
          }}
        >
          {propertyLabel}
        </span>
        <div style={{ display: "flex", gap: "var(--space-sm)" }}>
          <button
            onClick={handleStamp}
            style={{ fontSize: "var(--fs-xxs)", color: "var(--text-tertiary)", padding: "0 4px" }}
            title={t("inspector.keyframes.stamp")}
          >
            +
          </button>
          {keyframes.length > 0 && (
            <button
              onClick={handleClear}
              style={{ fontSize: "var(--fs-xxs)", color: "var(--text-tertiary)", padding: "0 4px" }}
              title={t("inspector.keyframes.clear")}
            >
              ×
            </button>
          )}
        </div>
      </div>

      {/* Keyframe track strip */}
      <div
        ref={trackRef}
        onClick={handleTrackClick}
        onContextMenu={(e) => e.preventDefault()}
        style={{
          position: "relative",
          height: LANE_HEIGHT,
          background: "var(--bg-raised)",
          borderRadius: 3,
          cursor: "pointer",
          overflow: "visible",
        }}
      >
        {displayKeyframes.map((kf) => (
          <div
            key={kf.key}
            onMouseDown={(e) => handleDiamondMouseDown(e, kf.key)}
            onContextMenu={(e) => handleDiamondContextMenu(e, kf.key)}
            style={{
              position: "absolute",
              left: `${frameToRatio(kf.frame) * 100}%`,
              top: "50%",
              width: DIAMOND_SIZE,
              height: DIAMOND_SIZE,
              background: kf.isDragging ? "var(--accent-primary)" : "var(--track-lottie)",
              border: "0.5px solid rgba(0,0,0,0.4)",
              transform: "translate(-50%, -50%) rotate(45deg)",
              cursor: "grab",
              pointerEvents: "auto",
            }}
          />
        ))}

        {keyframes.length === 0 && !dragging && (
          <div
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              pointerEvents: "none",
            }}
          >
            <span style={{ fontSize: "var(--fs-xxs)", color: "var(--text-muted)" }}>
              {t("inspector.keyframes.empty")}
            </span>
          </div>
        )}
      </div>

      {/* Right-click context menu (fixed overlay + menu, like ClipContextMenu) */}
      {menu && (
        <KeyframeContextMenu
          x={menu.x}
          y={menu.y}
          t={t}
          onDelete={() => handleDelete(menu.frame)}
          onSetInterpolation={(interp) => handleSetInterpolation(menu.frame, interp)}
          onClose={closeMenu}
        />
      )}
    </div>
  );
}

/** Floating context menu for a single keyframe. A full-screen invisible
 *  overlay captures outside clicks/right-clicks to close. */
function KeyframeContextMenu({
  x,
  y,
  t,
  onDelete,
  onSetInterpolation,
  onClose,
}: {
  x: number;
  y: number;
  t: TFunction;
  onDelete: () => void;
  onSetInterpolation: (interp: Interpolation) => void;
  onClose: () => void;
}) {
  return (
    <>
      <div
        style={{ position: "fixed", inset: 0, zIndex: 1000 }}
        onClick={onClose}
        onContextMenu={(e) => {
          e.preventDefault();
          onClose();
        }}
      />
      <div
        style={{
          position: "fixed",
          left: x,
          top: y,
          zIndex: 1001,
          background: "var(--bg-raised)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: 6,
          padding: "var(--space-xs) 0",
          minWidth: 140,
          boxShadow: "0 4px 12px rgba(0,0,0,0.3)",
        }}
      >
        <MenuItem onClick={onDelete} label={t("inspector.keyframes.delete")} />
        <div style={{ height: 1, background: "var(--border-primary)", margin: "2px 0" }} />
        <div
          style={{
            padding: "2px 12px",
            fontSize: "var(--fs-xxs)",
            color: "var(--text-muted)",
            textTransform: "uppercase",
          }}
        >
          {t("inspector.keyframes.interpolation")}
        </div>
        <MenuItem
          onClick={() => onSetInterpolation("linear")}
          label={t("inspector.keyframes.interpolation.linear")}
        />
        <MenuItem
          onClick={() => onSetInterpolation("hold")}
          label={t("inspector.keyframes.interpolation.hold")}
        />
        <MenuItem
          onClick={() => onSetInterpolation("smooth")}
          label={t("inspector.keyframes.interpolation.smooth")}
        />
      </div>
    </>
  );
}

function MenuItem({ onClick, label }: { onClick: () => void; label: string }) {
  return (
    <div
      onClick={onClick}
      style={{
        padding: "4px 12px",
        fontSize: "var(--fs-xs)",
        color: "var(--text-primary)",
        cursor: "pointer",
      }}
      onMouseEnter={(e) => (e.currentTarget.style.background = "var(--bg-prominent)")}
      onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
    >
      {label}
    </div>
  );
}

/** Resolve the clip's keyframe track for a given property. Returns undefined
 *  when the clip has no keyframes on that property. */
function getTrack(
  clip: Clip,
  property: KeyframeProperty,
): AnyKeyframeTrack | undefined {
  switch (property) {
    case "opacity":
      return clip.opacityTrack;
    case "volume":
      return clip.volumeTrack;
    case "rotation":
      return clip.rotationTrack;
    case "position":
      return clip.positionTrack;
    case "scale":
      return clip.scaleTrack;
    case "crop":
      return clip.cropTrack;
  }
}
