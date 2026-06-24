/**
 * UI-only editor state (SPEC §10.2): selection, zoom, playhead, panels, etc.
 * The front end owns and freely mutates this; it is never sourced from Rust.
 * Persisted keys (layout/panel visibility) are mirrored to localStorage.
 */

import { create } from "zustand";
import { ZOOM } from "../lib/theme";
import { useProjectStore } from "./projectStore";
import { totalFrames } from "../lib/geometry";

export type Panel = "agent" | "media" | "preview" | "inspector" | "timeline";
/** Top-level app view (SPEC: 启动先进主页). The editor is one of three views;
 *  switching is in-app (no router) so editor state survives navigation. */
export type AppView = "home" | "editor" | "settings" | "library";
export type ToolMode = "pointer" | "razor";
export type LayoutPreset = "default" | "media" | "vertical";
/** 剪映式顶部素材面板主标签（英文标识符，中文文案在 dict）。
 *  目前仅 material/audio 可用，其余为置灰占位（功能未做）。 */
export type MediaTabId =
  | "material"
  | "audio"
  | "text"
  | "sticker"
  | "effect"
  | "transition"
  | "subtitle"
  | "smartPack";
/** 素材/音频下的二级标签：导入（全部素材）/ 我的（星标收藏）。 */
export type MediaSubTabId = "import" | "mine";
export type InspectorTabId = "text" | "video" | "audio" | "aiEdit";

const LS = {
  layoutPreset: "layoutPreset",
  agentPanelVisible: "agentPanelVisible",
  mediaPanelVisible: "mediaPanelVisible",
  inspectorPanelVisible: "inspectorPanelVisible",
  keyframesPanelVisible: "keyframesPanelVisible",
} as const;

function loadBool(key: string, fallback: boolean): boolean {
  if (typeof localStorage === "undefined") return fallback;
  const v = localStorage.getItem(key);
  return v === null ? fallback : v === "true";
}
function loadPreset(): LayoutPreset {
  if (typeof localStorage === "undefined") return "default";
  const v = localStorage.getItem(LS.layoutPreset);
  return v === "media" || v === "vertical" ? v : "default";
}
function persist(key: string, value: string) {
  if (typeof localStorage !== "undefined") localStorage.setItem(key, value);
}

interface UiState {
  // Top-level navigation
  view: AppView;
  setView: (view: AppView) => void;

  // Playback / playhead
  currentFrame: number;
  activeFrame: number;
  isPlaying: boolean;
  isScrubbing: boolean;

  // Selection
  selectedClipIds: Set<string>;
  selectedMediaAssetIds: Set<string>;
  selectedFolderIds: Set<string>;
  isMarqueeSelecting: boolean;

  // Timeline view
  zoomScale: number;
  minZoomScale: number;
  scrollLeft: number;
  scrollTop: number;
  timelineVisibleWidth: number;
  toolMode: ToolMode;
  trackDisplayHeights: Record<string, number>;

  // Preview canvas
  canvasZoom: number;
  canvasOffset: { width: number; height: number };
  /** Media asset previewed in the canvas (clicked in the media panel). `null`
   *  shows the timeline composite. Mirrors upstream `openPreviewTab(mediaAsset)`. */
  previewMediaId: string | null;
  setPreviewMedia: (id: string | null) => void;

  // Panels
  focusedPanel: Panel | null;
  maximizedPanel: Panel | null;
  layoutPreset: LayoutPreset;
  agentPanelVisible: boolean;
  mediaPanelVisible: boolean;
  inspectorPanelVisible: boolean;
  keyframesPanelVisible: boolean;

  // Sub-tabs
  mediaTab: MediaTabId;
  mediaSubTab: MediaSubTabId;
  inspectorTab: InspectorTabId;
  previewActiveTabId: string;

  // Media panel navigation
  mediaPanelCurrentFolderId: string | null;

  // Actions
  setActiveFrame: (frame: number) => void;
  setCurrentFrame: (frame: number) => void;
  setPlaying: (playing: boolean) => void;
  /** Toggle play/pause. When STARTING from the parked end-of-timeline frame,
   *  rewinds to 0 first (both tickers stop at the last drawable frame, so without
   *  this the stop check fires immediately and play does nothing). Mirrors
   *  upstream VideoEngine.playbackStartFrame(). */
  togglePlay: () => void;
  setScrubbing: (scrubbing: boolean) => void;

  selectClips: (ids: Set<string>) => void;
  clearSelection: () => void;
  selectMediaAssets: (ids: Set<string>) => void;
  clearMediaSelection: () => void;

  setZoomScale: (zoom: number) => void;
  setMinZoomScale: (zoom: number) => void;
  setScroll: (left: number, top: number) => void;
  setVisibleWidth: (w: number) => void;
  setToolMode: (mode: ToolMode) => void;
  setTrackHeight: (trackId: string, height: number) => void;

  setCanvasZoom: (zoom: number) => void;
  setCanvasOffset: (offset: { width: number; height: number }) => void;

  focusPanel: (panel: Panel) => void;
  setMaximizedPanel: (panel: Panel | null) => void;
  setLayoutPreset: (preset: LayoutPreset) => void;
  toggleAgentPanel: () => void;
  toggleMediaPanel: () => void;
  toggleInspectorPanel: () => void;
  toggleKeyframesPanel: () => void;

  setMediaTab: (tab: MediaTabId) => void;
  setMediaSubTab: (tab: MediaSubTabId) => void;
  setInspectorTab: (tab: InspectorTabId) => void;
}

export const useEditorUiStore = create<UiState>((set, get) => ({
  view: "home",
  setView: (view) => set({ view }),

  currentFrame: 0,
  activeFrame: 0,
  isPlaying: false,
  isScrubbing: false,

  selectedClipIds: new Set(),
  selectedMediaAssetIds: new Set(),
  selectedFolderIds: new Set(),
  isMarqueeSelecting: false,

  zoomScale: ZOOM.default,
  minZoomScale: 0.05,
  scrollLeft: 0,
  scrollTop: 0,
  timelineVisibleWidth: 0,
  toolMode: "pointer",
  trackDisplayHeights: {},

  canvasZoom: 1,
  canvasOffset: { width: 0, height: 0 },
  previewMediaId: null,

  focusedPanel: "timeline",
  maximizedPanel: null,
  layoutPreset: loadPreset(),
  agentPanelVisible: loadBool(LS.agentPanelVisible, false),
  mediaPanelVisible: loadBool(LS.mediaPanelVisible, true),
  inspectorPanelVisible: loadBool(LS.inspectorPanelVisible, true),
  keyframesPanelVisible: loadBool(LS.keyframesPanelVisible, false),

  mediaTab: "material",
  mediaSubTab: "import",
  inspectorTab: "video",
  previewActiveTabId: "timeline",

  mediaPanelCurrentFolderId: null,

  setActiveFrame: (activeFrame) => set({ activeFrame }),
  setCurrentFrame: (currentFrame) => set({ currentFrame, activeFrame: currentFrame }),
  setPlaying: (isPlaying) => set({ isPlaying }),
  togglePlay: () => {
    const { isPlaying, activeFrame } = get();
    if (isPlaying) {
      set({ isPlaying: false });
      return;
    }
    // Starting playback: if parked at/after the last drawable frame (where the
    // ticker stopped), rewind to the start so `next >= last` doesn't fire on the
    // very first tick and stall play. Without media there's nothing to rewind.
    const last = Math.max(0, totalFrames(useProjectStore.getState().timeline) - 1);
    if (activeFrame >= last) {
      set({ currentFrame: 0, activeFrame: 0, isPlaying: true });
    } else {
      set({ isPlaying: true });
    }
  },
  setScrubbing: (isScrubbing) => set({ isScrubbing }),

  selectClips: (selectedClipIds) => set({ selectedClipIds }),
  clearSelection: () =>
    set({ selectedClipIds: new Set(), isMarqueeSelecting: false }),
  selectMediaAssets: (selectedMediaAssetIds) => set({ selectedMediaAssetIds }),
  clearMediaSelection: () => set({ selectedMediaAssetIds: new Set() }),
  setPreviewMedia: (previewMediaId) => set({ previewMediaId }),

  setZoomScale: (zoomScale) =>
    set({ zoomScale: Math.max(get().minZoomScale, Math.min(ZOOM.max, zoomScale)) }),
  setMinZoomScale: (minZoomScale) => set({ minZoomScale }),
  setScroll: (scrollLeft, scrollTop) => set({ scrollLeft, scrollTop }),
  setVisibleWidth: (timelineVisibleWidth) => set({ timelineVisibleWidth }),
  setToolMode: (toolMode) => set({ toolMode }),
  setTrackHeight: (trackId, height) =>
    set((s) => ({
      trackDisplayHeights: { ...s.trackDisplayHeights, [trackId]: height },
    })),

  setCanvasZoom: (canvasZoom) =>
    set({ canvasZoom, canvasOffset: canvasZoom <= 1 ? { width: 0, height: 0 } : get().canvasOffset }),
  setCanvasOffset: (canvasOffset) => set({ canvasOffset }),

  focusPanel: (panel) => {
    // Panel-click side effects (EditorWindowController.swift:188-189):
    // entering media clears clip selection; entering timeline clears asset sel.
    if (panel === "media") set({ focusedPanel: panel, selectedClipIds: new Set() });
    else if (panel === "timeline")
      set({ focusedPanel: panel, selectedMediaAssetIds: new Set() });
    else set({ focusedPanel: panel });
  },
  setMaximizedPanel: (maximizedPanel) => set({ maximizedPanel }),
  setLayoutPreset: (layoutPreset) => {
    persist(LS.layoutPreset, layoutPreset);
    set({ layoutPreset });
  },
  toggleAgentPanel: () =>
    set((s) => {
      const agentPanelVisible = !s.agentPanelVisible;
      persist(LS.agentPanelVisible, String(agentPanelVisible));
      return { agentPanelVisible };
    }),
  toggleMediaPanel: () =>
    set((s) => {
      const mediaPanelVisible = !s.mediaPanelVisible;
      persist(LS.mediaPanelVisible, String(mediaPanelVisible));
      return { mediaPanelVisible };
    }),
  toggleInspectorPanel: () =>
    set((s) => {
      const inspectorPanelVisible = !s.inspectorPanelVisible;
      persist(LS.inspectorPanelVisible, String(inspectorPanelVisible));
      return { inspectorPanelVisible };
    }),
  toggleKeyframesPanel: () =>
    set((s) => {
      const keyframesPanelVisible = !s.keyframesPanelVisible;
      persist(LS.keyframesPanelVisible, String(keyframesPanelVisible));
      return { keyframesPanelVisible };
    }),

  setMediaTab: (mediaTab) => set({ mediaTab }),
  setMediaSubTab: (mediaSubTab) => set({ mediaSubTab }),
  setInspectorTab: (inspectorTab) => set({ inspectorTab }),
}));
