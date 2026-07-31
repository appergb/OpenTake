// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../media/MediaPanel", () => ({
  MediaPanel: () => <div data-panel-content="media" />,
}));
vi.mock("../preview/Preview", () => ({
  Preview: () => <div data-panel-content="preview" />,
}));
vi.mock("../inspector/Inspector", () => ({
  Inspector: () => <div data-panel-content="inspector" />,
}));
vi.mock("../agent/AgentPanel", () => ({
  AgentPanel: () => <div data-panel-content="agent" />,
}));
vi.mock("../timeline/TimelineRegion", async () => {
  const React = await import("react");
  const { PanelShell } = await import("../ui/PanelShell");
  return {
    TimelineRegion: () =>
      React.createElement(
        PanelShell,
        { panel: "timeline" },
        React.createElement("div", { "data-panel-content": "timeline" }),
      ),
  };
});

import { useEditorUiStore, type LayoutPreset, type Panel } from "../../store/uiStore";
import { useI18nStore } from "../../i18n";
import { EditorSplit } from "./EditorSplit";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let viewport = { width: 1600, height: 1000 };

class ImmediateResizeObserver implements ResizeObserver {
  constructor(private readonly callback: ResizeObserverCallback) {}

  observe(target: Element): void {
    this.callback(
      [
        {
          target,
          contentRect: {
            width: viewport.width,
            height: viewport.height,
          },
        } as ResizeObserverEntry,
      ],
      this,
    );
  }

  disconnect(): void {}
  unobserve(): void {}
}

let root: Root | null = null;
let container: HTMLDivElement | null = null;

function panels(): Panel[] {
  return [...(container?.querySelectorAll<HTMLElement>("[data-editor-panel]") ?? [])].map(
    (panel) => panel.dataset.editorPanel as Panel,
  );
}

async function renderPreset(layoutPreset: LayoutPreset): Promise<void> {
  await act(async () => {
    useEditorUiStore.setState({ layoutPreset, maximizedPanel: null });
    root?.render(<EditorSplit />);
  });
}

beforeEach(() => {
  viewport = { width: 1600, height: 1000 };
  vi.stubGlobal("ResizeObserver", ImmediateResizeObserver);
  Object.defineProperties(HTMLElement.prototype, {
    clientWidth: { configurable: true, get: () => viewport.width },
    clientHeight: { configurable: true, get: () => viewport.height },
  });
  useEditorUiStore.setState({
    layoutPreset: "default",
    agentPanelVisible: false,
    mediaPanelVisible: true,
    inspectorPanelVisible: true,
    focusedPanel: "timeline",
    maximizedPanel: null,
    selectedClipIds: new Set(["clip-1"]),
    selectedMediaAssetIds: new Set(["media-1"]),
  });
  useI18nStore.setState({ locale: "en" });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  if (root) await act(async () => root?.unmount());
  container?.remove();
  root = null;
  container = null;
  vi.unstubAllGlobals();
});

describe("EditorSplit", () => {
  it.each([
    "all_presets_match_geometry_visibility_maximize_and_focus_shell",
    "all_presets_ratios_gutters_surfaces_focus",
  ])("%s", async () => {
    await renderPreset("default");
    expect(container?.querySelector("[data-layout-preset='default']")).not.toBeNull();
    expect(panels()).toEqual(["media", "preview", "inspector", "timeline"]);

    const defaultSeparators = [
      ...(container?.querySelectorAll<HTMLElement>("[role='separator']") ?? []),
    ];
    expect(defaultSeparators.some((separator) =>
      separator.getAttribute("aria-orientation") === "horizontal" &&
      separator.getAttribute("aria-valuenow") === "700"
    )).toBe(true);
    expect(defaultSeparators.some((separator) =>
      separator.getAttribute("aria-orientation") === "vertical" &&
      separator.getAttribute("aria-valuenow") === "500"
    )).toBe(true);

    const timeline = container?.querySelector<HTMLElement>("[data-editor-panel='timeline']");
    const media = container?.querySelector<HTMLElement>("[data-editor-panel='media']");
    expect(timeline?.getAttribute("role")).toBe("region");
    expect(timeline?.getAttribute("aria-label")).toBe("Timeline panel");
    expect(timeline?.classList.contains("editor-panel-shell")).toBe(true);
    expect(timeline?.querySelector(".editor-panel-card")).not.toBeNull();
    expect(
      timeline?.querySelector<HTMLElement>("[data-panel-focus-ring]")?.classList
        .contains("editor-panel-focus-ring"),
    ).toBe(true);
    expect(
      timeline?.querySelector<HTMLElement>("[data-panel-focus-ring]")?.style.opacity,
    ).toBe("0.6");

    await act(async () => media?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true })));
    expect(useEditorUiStore.getState().focusedPanel).toBe("media");
    expect(useEditorUiStore.getState().selectedClipIds.size).toBe(0);
    expect(
      media?.querySelector<HTMLElement>("[data-panel-focus-ring]")?.style.opacity,
    ).toBe("0.6");
    expect(
      timeline?.querySelector<HTMLElement>("[data-panel-focus-ring]")?.style.opacity,
    ).toBe("0");

    await renderPreset("media");
    expect(container?.querySelector("[data-layout-preset='media']")).not.toBeNull();
    expect(panels()).toEqual(["media", "preview", "inspector", "timeline"]);
    const mediaSeparators = [
      ...(container?.querySelectorAll<HTMLElement>("[role='separator']") ?? []),
    ];
    expect(mediaSeparators.some((separator) => separator.getAttribute("aria-valuenow") === "480"))
      .toBe(true);
    expect(mediaSeparators.some((separator) => separator.getAttribute("aria-valuenow") === "550"))
      .toBe(true);

    await renderPreset("vertical");
    expect(container?.querySelector("[data-layout-preset='vertical']")).not.toBeNull();
    expect(panels()).toEqual(["media", "inspector", "timeline", "preview"]);
    const verticalSeparators = [
      ...(container?.querySelectorAll<HTMLElement>("[role='separator']") ?? []),
    ];
    expect(verticalSeparators.some((separator) => separator.getAttribute("aria-valuenow") === "800"))
      .toBe(true);
    expect(verticalSeparators.some((separator) => separator.getAttribute("aria-valuenow") === "550"))
      .toBe(true);

    await act(async () => useEditorUiStore.setState({ inspectorPanelVisible: false }));
    expect(panels()).toEqual(["media", "timeline", "preview"]);
    expect(
      container?.querySelector("[data-layout-slot='vertical-top'] [role='separator']"),
    ).toBeNull();

    await act(async () => useEditorUiStore.setState({ mediaPanelVisible: false }));
    expect(panels()).toEqual(["timeline", "preview"]);

    await act(async () => useEditorUiStore.setState({
      mediaPanelVisible: true,
      inspectorPanelVisible: true,
      agentPanelVisible: true,
    }));
    expect(panels()).toEqual(["agent", "media", "inspector", "timeline", "preview"]);
    expect(
      [...(container?.querySelectorAll<HTMLElement>("[role='separator']") ?? [])]
        .some((separator) => separator.getAttribute("aria-valuenow") === "320"),
    ).toBe(true);

    await act(async () => {
      useEditorUiStore.getState().focusPanel("preview");
      useEditorUiStore.getState().toggleMaximizedFocusedPanel();
    });
    expect(panels()).toEqual(["preview"]);
    expect(container?.querySelector("[role='separator']")).toBeNull();

    await act(async () => useEditorUiStore.getState().setMaximizedPanel(null));
    const firstSeparator = container?.querySelector<HTMLElement>("[role='separator']");
    expect(firstSeparator?.tabIndex).toBe(0);
    const before = Number(firstSeparator?.getAttribute("aria-valuenow"));
    await act(async () =>
      firstSeparator?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true })),
    );
    expect(Number(firstSeparator?.getAttribute("aria-valuenow"))).toBeGreaterThan(before);

    viewport = { width: 960, height: 600 };
    await act(async () => {
      root?.unmount();
      root = createRoot(container!);
      useEditorUiStore.setState({
        layoutPreset: "default",
        agentPanelVisible: false,
        mediaPanelVisible: false,
        inspectorPanelVisible: false,
      });
      root.render(<EditorSplit />);
    });
    expect(panels()).toEqual(["preview", "timeline"]);
    expect(container?.querySelector("[data-layout-preset='default']")).not.toBeNull();
  });
});
