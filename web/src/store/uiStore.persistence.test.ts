// @vitest-environment happy-dom

import { beforeEach, describe, expect, it, vi } from "vitest";
import { createEditorUiStore } from "./uiStore";

const V1 = {
  layoutPreset: "opentake.ui.v1.layoutPreset",
  agentPanelVisible: "opentake.ui.v1.agentPanelVisible",
  mediaPanelVisible: "opentake.ui.v1.mediaPanelVisible",
  inspectorPanelVisible: "opentake.ui.v1.inspectorPanelVisible",
  keyframesPanelVisible: "opentake.ui.v1.keyframesPanelVisible",
  zoomScale: "opentake.ui.v1.zoomScale",
} as const;

const LEGACY = {
  layoutPreset: "layoutPreset",
  agentPanelVisible: "agentPanelVisible",
  mediaPanelVisible: "mediaPanelVisible",
  inspectorPanelVisible: "inspectorPanelVisible",
  keyframesPanelVisible: "keyframesPanelVisible",
  zoomScale: "zoomScale",
} as const;

function expectDefaults() {
  expect(createEditorUiStore().getState()).toMatchObject({
    view: "home",
    currentFrame: 0,
    layoutPreset: "default",
    agentPanelVisible: false,
    mediaPanelVisible: true,
    inspectorPanelVisible: true,
    keyframesPanelVisible: false,
  });
}

beforeEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});

describe("uiStore schema-safe persistence", () => {
  it("schema_safe_layout_panel_and_keyframe_state_survive_restart", () => {
    expectDefaults();

    localStorage.setItem(V1.layoutPreset, "vertical");
    localStorage.setItem(V1.agentPanelVisible, "true");
    localStorage.setItem(V1.mediaPanelVisible, "false");
    localStorage.setItem(V1.inspectorPanelVisible, "false");
    localStorage.setItem(V1.keyframesPanelVisible, "true");
    localStorage.setItem(V1.zoomScale, "8");
    expect(createEditorUiStore().getState()).toMatchObject({
      layoutPreset: "vertical",
      agentPanelVisible: true,
      mediaPanelVisible: false,
      inspectorPanelVisible: false,
      keyframesPanelVisible: true,
      zoomScale: 8,
    });

    localStorage.clear();
    localStorage.setItem(LEGACY.layoutPreset, "media");
    localStorage.setItem(LEGACY.agentPanelVisible, "true");
    localStorage.setItem(LEGACY.mediaPanelVisible, "false");
    localStorage.setItem(LEGACY.inspectorPanelVisible, "false");
    localStorage.setItem(LEGACY.keyframesPanelVisible, "true");
    localStorage.setItem(LEGACY.zoomScale, "6");
    expect(createEditorUiStore().getState()).toMatchObject({
      layoutPreset: "media",
      agentPanelVisible: true,
      mediaPanelVisible: false,
      inspectorPanelVisible: false,
      keyframesPanelVisible: true,
      zoomScale: 6,
    });
    for (const key of Object.keys(V1) as Array<keyof typeof V1>) {
      expect(localStorage.getItem(V1[key])).toBe(localStorage.getItem(LEGACY[key]));
    }

    localStorage.clear();
    localStorage.setItem(V1.layoutPreset, "wide");
    localStorage.setItem(V1.agentPanelVisible, "1");
    localStorage.setItem(V1.mediaPanelVisible, "yes");
    localStorage.setItem(V1.inspectorPanelVisible, "FALSE");
    localStorage.setItem(V1.keyframesPanelVisible, "null");
    localStorage.setItem(V1.zoomScale, "41");
    expectDefaults();
    for (const key of Object.values(V1)) expect(localStorage.getItem(key)).toBeNull();

    localStorage.clear();
    for (const key of Object.values(LEGACY)) localStorage.setItem(key, "corrupt");
    expectDefaults();
    for (const key of Object.values(V1)) expect(localStorage.getItem(key)).toBeNull();
    for (const key of Object.values(LEGACY)) expect(localStorage.getItem(key)).toBeNull();

    localStorage.clear();
    const first = createEditorUiStore();
    const setItem = vi.spyOn(localStorage, "setItem");
    const writesOnly = (key: string, action: () => void, value: string) => {
      setItem.mockClear();
      action();
      expect(setItem.mock.calls).toEqual([[key, value]]);
    };
    writesOnly(V1.layoutPreset, () => first.getState().setLayoutPreset("vertical"), "vertical");
    writesOnly(V1.agentPanelVisible, () => first.getState().toggleAgentPanel(), "true");
    writesOnly(V1.mediaPanelVisible, () => first.getState().toggleMediaPanel(), "false");
    writesOnly(V1.inspectorPanelVisible, () => first.getState().toggleInspectorPanel(), "false");
    writesOnly(V1.keyframesPanelVisible, () => first.getState().toggleKeyframesPanel(), "true");
    writesOnly(V1.zoomScale, () => first.getState().setZoomScale(8), "8");

    first.getState().setView("editor");
    first.getState().setCurrentFrame(120);
    first.getState().selectClips(new Set(["project-clip"]));
    const restarted = createEditorUiStore().getState();
    expect(restarted).toMatchObject({
      view: "editor",
      currentFrame: 0,
      activeFrame: 0,
      maximizedPanel: null,
      layoutPreset: "vertical",
      agentPanelVisible: true,
      mediaPanelVisible: false,
      inspectorPanelVisible: false,
      keyframesPanelVisible: true,
      zoomScale: 8,
    });
    expect(restarted.selectedClipIds).toEqual(new Set());

    vi.spyOn(localStorage, "getItem").mockImplementation(() => {
      throw new DOMException("storage unavailable", "SecurityError");
    });
    expectDefaults();
    vi.restoreAllMocks();

    vi.stubGlobal("localStorage", undefined);
    expectDefaults();
    vi.unstubAllGlobals();

    localStorage.clear();
    const unavailable = createEditorUiStore();
    vi.spyOn(localStorage, "setItem").mockImplementation(() => {
      throw new DOMException("quota exceeded", "QuotaExceededError");
    });
    expect(() => unavailable.getState().setLayoutPreset("media")).not.toThrow();
    expect(() => unavailable.getState().toggleAgentPanel()).not.toThrow();
    expect(unavailable.getState()).toMatchObject({
      layoutPreset: "media",
      agentPanelVisible: true,
    });
  });
});
