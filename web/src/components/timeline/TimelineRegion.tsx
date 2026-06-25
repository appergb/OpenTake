/**
 * Timeline region (SPEC §3.1): the toolbar (height 38) stacked above the
 * timeline container, all inside one timeline PanelShell.
 *
 * Also the drop target for media dragged from the MediaPanel: a media item
 * carries its asset id on a private dataTransfer type; dropping it adds the clip
 * to a compatible track via `addMediaToTimeline`. A dashed ring + hint overlay
 * shows while a valid drag is over the region.
 */

import { useState } from "react";
import { PanelShell } from "../ui/PanelShell";
import { Toolbar } from "../toolbar/Toolbar";
import { TimelineContainer } from "./TimelineContainer";
import { MEDIA_DND_TYPE } from "../media/MediaPanel";
import { useMediaStore } from "../../store/mediaStore";
import { useEditorUiStore } from "../../store/uiStore";
import { addMediaToTimeline } from "../../store/editActions";
import { useT } from "../../i18n";

export function TimelineRegion() {
  const t = useT();
  const [dragOver, setDragOver] = useState(false);
  const setPreviewMedia = useEditorUiStore((s) => s.setPreviewMedia);

  const hasMediaPayload = (e: React.DragEvent) =>
    e.dataTransfer.types.includes(MEDIA_DND_TYPE);

  const onDragOver = (e: React.DragEvent) => {
    if (!hasMediaPayload(e)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    if (!dragOver) setDragOver(true);
  };

  const onDragLeave = (e: React.DragEvent) => {
    // Ignore leaves into child elements: only clear when leaving the region.
    if (e.currentTarget.contains(e.relatedTarget as Node)) return;
    setDragOver(false);
  };

  const onDrop = (e: React.DragEvent) => {
    if (!hasMediaPayload(e)) return;
    e.preventDefault();
    setDragOver(false);
    const id = e.dataTransfer.getData(MEDIA_DND_TYPE);
    const item = useMediaStore.getState().items.find((m) => m.id === id);
    if (item) void addMediaToTimeline(item);
  };

  return (
    <PanelShell panel="timeline">
      <Toolbar />
      <div
        style={{ position: "relative", flex: 1, minHeight: 0 }}
        // Focusing the editing area returns the preview to the Timeline tab
        // (so the composite — not the last-clicked media — is shown).
        onPointerDownCapture={() => setPreviewMedia(null)}
        onDragOver={onDragOver}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
      >
        <TimelineContainer />
        {dragOver && (
          <div
            style={{
              position: "absolute",
              inset: 0,
              pointerEvents: "none",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              border: "var(--bw-medium) dashed var(--accent-primary)",
              background: "rgba(245,239,228,0.06)",
              color: "var(--text-secondary)",
              fontSize: "var(--fs-sm-md)",
              fontWeight: "var(--fw-medium)",
              zIndex: 30,
            }}
          >
            {t("media.dropToAdd")}
          </div>
        )}
      </div>
    </PanelShell>
  );
}
