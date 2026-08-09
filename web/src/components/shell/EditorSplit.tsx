/**
 * Editor split (SPEC §2.2-2.4). Outermost is always [agent column | preset
 * subtree]; the preset subtree is one of three layouts with the documented
 * initial proportions. Panel visibility (media/inspector) and maximize collapse
 * the corresponding regions.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { SplitPane } from "./SplitPane";
import { PanelShell } from "../ui/PanelShell";
import { MediaPanel } from "../media/MediaPanel";
import { Preview } from "../preview/Preview";
import { Inspector } from "../inspector/Inspector";
import { AgentPanel } from "../agent/AgentPanel";
import { TimelineRegion } from "../timeline/TimelineRegion";
import { useEditorUiStore, type LayoutPreset } from "../../store/uiStore";

// Upstream defaults (Constants.swift): mediaPanelDefault=500, inspectorDefault=260.
const MEDIA_DEFAULT = 500;
const INSPECTOR_DEFAULT = 260;
const AGENT_DEFAULT = 320;
const AGENT_MIN = 240;
const MEDIA_MIN = 160;
const MEDIA_LAYOUT_MEDIA_MIN = 200;
const PREVIEW_MIN = 200;
const INSPECTOR_MIN = 160;
const VERTICAL_LEFT_BASE_MIN = 300;
const VERTICAL_PREVIEW_MIN = 300;

function verticalLeftMinimumWidth(mediaVisible: boolean, inspectorVisible: boolean): number {
  const nestedMinimum =
    (mediaVisible ? MEDIA_MIN : 0) + (inspectorVisible ? INSPECTOR_MIN : 0);
  return Math.max(VERTICAL_LEFT_BASE_MIN, nestedMinimum);
}

function presetMinimumWidth(
  preset: LayoutPreset,
  mediaVisible: boolean,
  inspectorVisible: boolean,
): number {
  if (preset === "media") {
    return (
      (mediaVisible ? MEDIA_LAYOUT_MEDIA_MIN : 0) +
      PREVIEW_MIN +
      (inspectorVisible ? INSPECTOR_MIN : 0)
    );
  }
  if (preset === "vertical") {
    return verticalLeftMinimumWidth(mediaVisible, inspectorVisible) + VERTICAL_PREVIEW_MIN;
  }
  return (
    PREVIEW_MIN +
    (mediaVisible ? MEDIA_MIN : 0) +
    (inspectorVisible ? INSPECTOR_MIN : 0)
  );
}

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
  const { ref, size } = useContainerSize();
  const agentSplitRef = useRef<HTMLDivElement>(null);
  const agentContentRef = useRef<HTMLDivElement>(null);
  const agentVisible = useEditorUiStore((s) => s.agentPanelVisible);
  const maximized = useEditorUiStore((s) => s.maximizedPanel);
  const layoutPreset = useEditorUiStore((s) => s.layoutPreset);
  const mediaVisible = useEditorUiStore((s) => s.mediaPanelVisible);
  const inspectorVisible = useEditorUiStore((s) => s.inspectorPanelVisible);
  const workspaceMinimum = presetMinimumWidth(layoutPreset, mediaVisible, inspectorVisible);
  // Preserve every editing pane before the optional Agent column. The default
  // preset fits exactly at 760px; wider-minimum presets temporarily fold Agent.
  const responsiveAgentCollapsed =
    maximized === null &&
    agentVisible &&
    size.w > 0 &&
    size.w < AGENT_MIN + workspaceMinimum;

  useLayoutEffect(() => {
    if (!responsiveAgentCollapsed) return;
    const active = document.activeElement;
    if (!(active instanceof HTMLElement)) return;
    const outerSplit = agentSplitRef.current?.firstElementChild;
    const outerDivider = outerSplit?.children.item(1);
    const focusWillBeHidden = agentContentRef.current?.contains(active) ||
      outerDivider?.contains(active);
    if (!focusWillBeHidden) return;
    ref.current
      ?.querySelector<HTMLElement>("[data-editor-panel]:not([data-editor-panel='agent'])")
      ?.focus({ preventScroll: true });
  }, [ref, responsiveAgentCollapsed]);

  // Maximized panel takes the whole area.
  if (maximized) {
    return (
      <div ref={ref} style={{ width: "100%", height: "100%" }}>
        <div data-maximized-panel={maximized} style={{ width: "100%", height: "100%" }}>
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
      </div>
    );
  }

  const presetSubtree = <PresetSubtree />;

  return (
    <div
      ref={ref}
      data-editor-split-root
      data-responsive-collapsed-agent={responsiveAgentCollapsed ? "true" : undefined}
      style={{ width: "100%", height: "100%" }}
    >
      {!agentVisible ? (
        presetSubtree
      ) : (
        <>
          <style>{`
            .editor-agent-split[data-responsive-collapsed="true"] > :first-child > :first-child {
              flex-basis: 0 !important;
              overflow: hidden;
              visibility: hidden;
            }
            .editor-agent-split[data-responsive-collapsed="true"] > :first-child > :nth-child(2) {
              display: none;
            }
          `}</style>
          <div
            ref={agentSplitRef}
            className="editor-agent-split"
            data-responsive-collapsed={responsiveAgentCollapsed ? "true" : undefined}
            style={{ width: "100%", height: "100%" }}
          >
            <SplitPane
              mode="horizontal"
              initial={AGENT_DEFAULT}
              min={AGENT_MIN}
              secondMin={workspaceMinimum}
              first={
                <div
                  ref={agentContentRef}
                  data-responsive-agent-content
                  aria-hidden={responsiveAgentCollapsed ? "true" : undefined}
                  style={{ width: "100%", height: "100%" }}
                >
                  <PanelShell panel="agent">
                    <AgentPanel />
                  </PanelShell>
                </div>
              }
              second={presetSubtree}
            />
          </div>
        </>
      )}
    </div>
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
    <div
      ref={ref}
      data-layout-preset="default"
      style={{ width: "100%", height: "100%" }}
    >
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
  const rightMinimum = PREVIEW_MIN + (inspectorVisible ? INSPECTOR_MIN : 0);
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
    <div ref={ref} data-layout-preset="media" style={{ width: "100%", height: "100%" }}>
      {size.w > 0 &&
        (mediaVisible ? (
          <SplitPane
            mode="horizontal"
            initial={mediaWidth}
            min={MEDIA_LAYOUT_MEDIA_MIN}
            secondMin={rightMinimum}
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
  const leftMinimum = verticalLeftMinimumWidth(mediaVisible, inspectorVisible);
  const left = (
    <RightVerticalSplit
      topRatio={0.55}
      top={
        <div data-layout-slot="vertical-top" style={{ width: "100%", height: "100%" }}>
          <ThreeColumn
            left={mediaVisible ? <Media /> : null}
            leftWidth={MEDIA_DEFAULT}
            center={<InspectorPanel />}
            right={null}
            rightWidth={0}
            centerIsInspector={inspectorVisible}
            centerMin={INSPECTOR_MIN}
          />
        </div>
      }
      bottom={<TimelineRegion />}
    />
  );

  return (
    <div
      ref={ref}
      data-layout-preset="vertical"
      style={{ width: "100%", height: "100%" }}
    >
      {size.w > 0 && (
        <SplitPane
          mode="horizontal"
          initial={leftWidth}
          min={leftMinimum}
          secondMin={VERTICAL_PREVIEW_MIN}
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
  centerMin = PREVIEW_MIN,
}: {
  left: React.ReactNode | null;
  leftWidth: number;
  center: React.ReactNode;
  right: React.ReactNode | null;
  rightWidth: number;
  centerIsInspector?: boolean;
  centerMin?: number;
}) {
  // center may itself be the inspector (vertical layout) — collapse when hidden.
  const renderedCenter = centerIsInspector === false ? null : center;

  // In Vertical layout the center is the Inspector. When it is collapsed, do
  // not retain a split whose second pane is only an empty base-colored slot;
  // the remaining Media panel must consume the full top-left region just as a
  // collapsed NSSplitView item does upstream.
  if (!renderedCenter) {
    if (left) return <div style={{ width: "100%", height: "100%" }}>{left}</div>;
    if (right) return <div style={{ width: "100%", height: "100%" }}>{right}</div>;
    return <div style={{ width: "100%", height: "100%", background: "var(--bg-base)" }} />;
  }

  if (left && right) {
    return (
      <SplitPane
        mode="horizontal"
        initial={leftWidth}
        min={MEDIA_MIN}
        secondMin={centerMin + INSPECTOR_MIN}
        first={left}
        second={
          <SplitPaneRightAnchored
            rightWidth={rightWidth}
            center={renderedCenter}
            right={right}
            centerMin={centerMin}
          />
        }
      />
    );
  }
  if (left && !right) {
    return (
      <SplitPane
        mode="horizontal"
        initial={leftWidth}
        min={MEDIA_MIN}
        secondMin={centerMin}
        first={left}
        second={renderedCenter ?? <div style={{ width: "100%", height: "100%", background: "var(--bg-base)" }} />}
      />
    );
  }
  if (!left && right) {
    return (
      <SplitPaneRightAnchored
        rightWidth={rightWidth}
        center={renderedCenter}
        right={right}
        centerMin={centerMin}
      />
    );
  }
  return <div style={{ width: "100%", height: "100%" }}>{renderedCenter}</div>;
}

/** center (flex) + right panel of a fixed initial width. */
function SplitPaneRightAnchored({
  rightWidth,
  center,
  right,
  centerMin,
}: {
  rightWidth: number;
  center: React.ReactNode;
  right: React.ReactNode;
  centerMin: number;
}) {
  const { ref, size } = useContainerSize();
  const firstWidth = Math.max(centerMin, size.w - rightWidth) || 1;
  return (
    <div
      ref={ref}
      data-layout-split="preview-inspector"
      style={{ width: "100%", height: "100%" }}
    >
      {size.w > 0 && (
        <SplitPane
          mode="horizontal"
          initial={firstWidth}
          min={centerMin}
          secondMin={INSPECTOR_MIN}
          first={center ?? <div style={{ width: "100%", height: "100%", background: "var(--bg-base)" }} />}
          second={right}
        />
      )}
    </div>
  );
}
