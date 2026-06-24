/**
 * KeyframesPanel (SPEC §6.4). Inspector sub-panel that renders one row per
 * animatable property for the selected clip, each row a `KeyframesLaneRow` with
 * draggable diamond markers. A panel-wide red playhead line overlays every row
 * so the user can see the current frame against every track at once.
 *
 * Layout: the outer div carries the top border + padding (visual spacing from
 * the Inspector edges). An inner `position: relative` wrapper (no padding)
 * holds the ruler, the rows, and the playhead overlay. Both the playhead
 * (`left: X%`) and the keyframe diamonds inside each row (`left: X%`) resolve
 * against the same width — the inner wrapper's content box — so they align
 * exactly at every frame. (If the overlay were positioned against the padded
 * outer box, the playhead would drift from the diamonds by up to the padding
 * width at the clip's start/end.)
 */

import { useEditorUiStore } from "../../store/uiStore";
import type { TFunction } from "../../i18n";
import type { Clip, KeyframeProperty } from "../../lib/types";
import { KeyframesLaneRow } from "./KeyframesLaneRow";

const VIDEO_PROPERTIES: KeyframeProperty[] = ["position", "scale", "rotation", "opacity", "crop"];
const AUDIO_PROPERTIES: KeyframeProperty[] = ["volume"];

export function KeyframesPanel({ clip, t }: { clip: Clip; t: TFunction }) {
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const properties = clip.mediaType === "audio" ? AUDIO_PROPERTIES : VIDEO_PROPERTIES;
  const startFrame = clip.startFrame;
  const endFrame = clip.startFrame + clip.durationFrames;
  const duration = Math.max(1, endFrame - startFrame);

  // Playhead position within the clip (0..1), clamped to the clip's span.
  const playheadRatio = Math.max(0, Math.min(1, (activeFrame - startFrame) / duration));

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
        {/* Ruler bar — a thin tinted strip showing the clip's span. */}
        <div
          style={{
            height: 4,
            background: "var(--bg-raised)",
            borderRadius: 2,
            position: "relative",
            marginBottom: "var(--space-xs)",
          }}
        >
          <div
            style={{
              position: "absolute",
              inset: 0,
              background: "var(--accent-primary)",
              opacity: 0.3,
              borderRadius: 2,
            }}
          />
        </div>

        {/* Property rows. */}
        {properties.map((prop) => (
          <KeyframesLaneRow key={prop} clip={clip} property={prop} t={t} />
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
      </div>
    </div>
  );
}
