/**
 * KeyframesPanel (SPEC §6.4). Inspector sub-panel that renders one row per
 * animatable property for the selected clip, each row a `KeyframesLaneRow` with
 * draggable diamond markers. A panel-wide red playhead line overlays every row
 * so the user can see the current frame against every track at once. A second,
 * yellow, panel-wide line appears only while a keyframe drag is actively
 * snapped (cross-property / playhead / clip-bound target) — 1:1 port of
 * upstream's `KeyframesPanel.snapOverlay` (Inspector/Keyframes/KeyframesLane.swift:301-313).
 *
 * Layout: the outer div carries the top border + padding (visual spacing from
 * the Inspector edges). An inner `position: relative` wrapper (no padding)
 * holds the ruler, the rows, and the overlays. The playhead/snap overlays
 * (`left: X%`) and the keyframe diamonds inside each row (`left: X%`) resolve
 * against the same width — the inner wrapper's content box — so they align
 * exactly at every frame. (If the overlay were positioned against the padded
 * outer box, the playhead would drift from the diamonds by up to the padding
 * width at the clip's start/end.)
 *
 * Snap-line state lives here (lifted from the individual rows) because the
 * line must span every row, not just the one being dragged — each
 * `KeyframesLaneRow` reports its live snap frame via `onSnapChange`, which
 * only ever writes to this state while that row owns the active drag.
 */

import { useState } from "react";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import type { TFunction } from "../../i18n";
import type { Clip, KeyframeProperty } from "../../lib/types";
import { KeyframesLaneRow } from "./KeyframesLaneRow";
import { KeyframesRuler } from "./KeyframesRuler";

const VIDEO_PROPERTIES: KeyframeProperty[] = ["position", "scale", "rotation", "opacity", "crop"];
const AUDIO_PROPERTIES: KeyframeProperty[] = ["volume"];

export function KeyframesPanel({ clip, t }: { clip: Clip; t: TFunction }) {
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const fps = useProjectStore((s) => s.timeline.fps);
  const [snappedFrame, setSnappedFrame] = useState<number | null>(null);
  const properties = clip.mediaType === "audio" ? AUDIO_PROPERTIES : VIDEO_PROPERTIES;
  const startFrame = clip.startFrame;
  const endFrame = clip.startFrame + clip.durationFrames;
  const duration = Math.max(1, endFrame - startFrame);

  // Playhead position within the clip (0..1), clamped to the clip's span.
  const playheadRatio = Math.max(0, Math.min(1, (activeFrame - startFrame) / duration));
  const snapRatio =
    snappedFrame === null
      ? null
      : Math.max(0, Math.min(1, (snappedFrame - startFrame) / duration));

  return (
    <div
      style={{
        borderTop: "var(--bw-thin) solid var(--border-primary)",
        padding: "var(--space-md) var(--space-lg)",
      }}
    >
      <div
        style={{
          position: "relative",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-sm)",
        }}
      >
        {/* Real tick ruler spanning the clip's frame range. */}
        <KeyframesRuler duration={duration} fps={fps} />

        {/* Property rows. */}
        {properties.map((prop) => (
          <KeyframesLaneRow key={prop} clip={clip} property={prop} t={t} onSnapChange={setSnappedFrame} />
        ))}

        {/* Panel-wide playhead overlay — spans the full height of the inner
            wrapper (ruler + every row) so the user sees the playhead cross all
            tracks. pointerEvents:none so it never blocks row clicks/drags.
            zIndex above the rows but below the context menu (z-index 1000+). */}
        <div
          style={{
            position: "absolute",
            left: `${playheadRatio * 100}%`,
            top: 0,
            bottom: 0,
            width: 1,
            background: "var(--accent-spotlight)",
            pointerEvents: "none",
            zIndex: 5,
          }}
        />

        {/* Panel-wide yellow snap-guide overlay — only visible while a
            keyframe drag is actively snapped to a target. Dashed, matching
            both upstream's `StrokeStyle(dash: [4, 4])` and the main
            timeline's own SnapIndicator dashed line. */}
        {snapRatio !== null && (
          <div
            style={{
              position: "absolute",
              left: `${snapRatio * 100}%`,
              top: 0,
              bottom: 0,
              width: 1,
              backgroundImage:
                "repeating-linear-gradient(to bottom, var(--status-warning) 0 4px, transparent 4px 8px)",
              pointerEvents: "none",
              zIndex: 6,
            }}
          />
        )}
      </div>
    </div>
  );
}
