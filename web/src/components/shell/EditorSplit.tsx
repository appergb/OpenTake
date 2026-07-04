/**
 * Editor split (SPEC §2.2-2.4). Outermost is always [agent column | preset
 * subtree]; the preset subtree is one of three layouts with the documented
 * initial proportions. Panel visibility (media/inspector) and maximize collapse
 * the corresponding regions.
 */

import { useEffect, useRef, useState } from "react";
import { SplitPane } from "./SplitPane";
import { PanelShell } from "../ui/PanelShell";
import { MediaPanel } from "../media/MediaPanel";
import { Preview } from "../preview/Preview";
import { Inspector } from "../inspector/Inspector";
import { AgentPanel } from "../agent/AgentPanel";
import { TimelineRegion } from "../timeline/TimelineRegion";
import { useEditorUiStore } from "../../store/uiStore";

// Upstream defaults (Constants.swift): mediaPanelDefault=500, inspectorDefault=260.
const MEDIA_DEFAULT = 500;
const INSPECTOR_DEFAULT = 260;
const AGENT_DEFAULT = 320;

function useContainerSize() {
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () => setSize({ w: el.clientWidth, h: el.clientHeight });
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return { ref, size };
}

const Media = () => (
  <PanelShell panel="media">
    <MediaPanel />
  </PanelShell>
);
const PreviewPanel = () => (
  <PanelShell panel="preview">
    <Preview />
  </PanelShell>
);
const InspectorPanel = () => (
  <PanelShell panel="inspector">
    <Inspector />
  </PanelShell>
);

export function EditorSplit() {
  const agentVisible = useEditorUiStore((s) => s.agentPanelVisible);
  const maximized = useEditorUiStore((s) => s.maximizedPanel);

  // Maximized panel takes the whole area.
  if (maximized) {
    return (
      <div style={{ width: "100%", height: "100%" }}>
        {maximized === "media" && <Media />}
        {maximized === "preview" && <PreviewPanel />}
        {maximized === "inspector" && <InspectorPanel />}
        {maximized === "timeline" && <TimelineRegion />}
        {maximized === "agent" && (
          <PanelShell panel="agent">
            <AgentPanel />
          </PanelShell>
        )}
      </div>
    );
  }

  const presetSubtree = <PresetSubtree />;

  if (!agentVisible) {
    return <div style={{ width: "100%", height: "100%" }}>{presetSubtree}</div>;
  }

  return (
    <SplitPane
      mode="horizontal"
      initial={AGENT_DEFAULT}
      min={240}
      secondMin={400}
      first={
        <PanelShell panel="agent">
          <AgentPanel />
        </PanelShell>
      }
      second={presetSubtree}
    />
  );
}

function PresetSubtree() {
  const layoutPreset = useEditorUiStore((s) => s.layoutPreset);
  if (layoutPreset === "media") return <MediaLayout />;
  if (layoutPreset === "vertical") return <VerticalLayout />;
  return <DefaultLayout />;
}

/** Default (SPEC §2.2): top [Media|Preview|Inspector] (70% h) over [Timeline]. */
function DefaultLayout() {
  const { ref, size } = useContainerSize();
  const mediaVisible = useEditorUiStore((s) => s.mediaPanelVisible);
  const inspectorVisible = useEditorUiStore((s) => s.inspectorPanelVisible);

  const topHeight = Math.round(size.h * 0.7) || 1;

  const topRow = (
    <ThreeColumn
      left={mediaVisible ? <Media /> : null}
      leftWidth={MEDIA_DEFAULT}
      right={inspectorVisible ? <InspectorPanel /> : null}
      rightWidth={INSPECTOR_DEFAULT}
      center={<PreviewPanel />}
    />
  );

  return (
    <div ref={ref} style={{ width: "100%", height: "100%" }}>
      {size.h > 0 && (
        <SplitPane
          mode="vertical"
          initial={topHeight}
          min={200}
          secondMin={120}
          first={topRow}
          second={<TimelineRegion />}
        />
      )}
    </div>
  );
}

/** Media (SPEC §2.3): [Media(30%) | (top [Preview|Inspector] 55% / Timeline)]. */
function MediaLayout() {
  const { ref, size } = useContainerSize();
  const mediaVisible = useEditorUiStore((s) => s.mediaPanelVisible);
  const inspectorVisible = useEditorUiStore((s) => s.inspectorPanelVisible);

  const mediaWidth = Math.round(size.w * 0.3) || 1;
  const right = (
    <RightVerticalSplit
      topRatio={0.55}
      top={
        <ThreeColumn
          left={null}
          leftWidth={0}
          center={<PreviewPanel />}
          right={inspectorVisible ? <InspectorPanel /> : null}
          rightWidth={INSPECTOR_DEFAULT}
        />
      }
      bottom={<TimelineRegion />}
    />
  );

  return (
    <div ref={ref} style={{ width: "100%", height: "100%" }}>
      {size.w > 0 &&
        (mediaVisible ? (
          <SplitPane
            mode="horizontal"
            initial={mediaWidth}
            min={200}
            secondMin={400}
            first={<Media />}
            second={right}
          />
        ) : (
          right
        ))}
    </div>
  );
}

/** Vertical (SPEC §2.4): [left subtree(50%) | Preview]. */
function VerticalLayout() {
  const { ref, size } = useContainerSize();
  const mediaVisible = useEditorUiStore((s) => s.mediaPanelVisible);
  const inspectorVisible = useEditorUiStore((s) => s.inspectorPanelVisible);

  const leftWidth = Math.round(size.w * 0.5) || 1;
  const left = (
    <RightVerticalSplit
      topRatio={0.55}
      top={
        <ThreeColumn
          left={mediaVisible ? <Media /> : null}
          leftWidth={MEDIA_DEFAULT}
          center={<InspectorPanel />}
          right={null}
          rightWidth={0}
          centerIsInspector={inspectorVisible}
        />
      }
      bottom={<TimelineRegion />}
    />
  );

  return (
    <div ref={ref} style={{ width: "100%", height: "100%" }}>
      {size.w > 0 && (
        <SplitPane
          mode="horizontal"
          initial={leftWidth}
          min={300}
          secondMin={300}
          first={left}
          second={<PreviewPanel />}
        />
      )}
    </div>
  );
}

/** A vertical split whose top height is a ratio of the container. */
function RightVerticalSplit({
  topRatio,
  top,
  bottom,
}: {
  topRatio: number;
  top: React.ReactNode;
  bottom: React.ReactNode;
}) {
  const { ref, size } = useContainerSize();
  const topH = Math.round(size.h * topRatio) || 1;
  return (
    <div ref={ref} style={{ width: "100%", height: "100%" }}>
      {size.h > 0 && (
        <SplitPane mode="vertical" initial={topH} min={160} secondMin={120} first={top} second={bottom} />
      )}
    </div>
  );
}

/** Horizontal three-column row with optional left/right panels and a flexible
 *  center. Hidden side panels collapse to give the center their space. */
function ThreeColumn({
  left,
  leftWidth,
  center,
  right,
  rightWidth,
  centerIsInspector,
}: {
  left: React.ReactNode | null;
  leftWidth: number;
  center: React.ReactNode;
  right: React.ReactNode | null;
  rightWidth: number;
  centerIsInspector?: boolean;
}) {
  // center may itself be the inspector (vertical layout) — collapse when hidden.
  const renderedCenter = centerIsInspector === false ? null : center;

  if (left && right) {
    return (
      <SplitPane
        mode="horizontal"
        initial={leftWidth}
        min={160}
        secondMin={200}
        first={left}
        second={
          <SplitPaneRightAnchored rightWidth={rightWidth} center={renderedCenter} right={right} />
        }
      />
    );
  }
  if (left && !right) {
    return (
      <SplitPane
        mode="horizontal"
        initial={leftWidth}
        min={160}
        secondMin={200}
        first={left}
        second={renderedCenter ?? <div style={{ width: "100%", height: "100%", background: "var(--bg-base)" }} />}
      />
    );
  }
  if (!left && right) {
    return (
      <SplitPaneRightAnchored rightWidth={rightWidth} center={renderedCenter} right={right} />
    );
  }
  return <div style={{ width: "100%", height: "100%" }}>{renderedCenter}</div>;
}

/** center (flex) + right panel of a fixed initial width. */
function SplitPaneRightAnchored({
  rightWidth,
  center,
  right,
}: {
  rightWidth: number;
  center: React.ReactNode;
  right: React.ReactNode;
}) {
  const { ref, size } = useContainerSize();
  const firstWidth = Math.max(200, size.w - rightWidth) || 1;
  return (
    <div ref={ref} style={{ width: "100%", height: "100%" }}>
      {size.w > 0 && (
        <SplitPane
          mode="horizontal"
          initial={firstWidth}
          min={200}
          secondMin={160}
          first={center ?? <div style={{ width: "100%", height: "100%", background: "var(--bg-base)" }} />}
          second={right}
        />
      )}
    </div>
  );
}
