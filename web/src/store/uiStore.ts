/**
 * UI-only editor state (SPEC §10.2): selection, zoom, playhead, panels, etc.
 * The front end owns and freely mutates this; it is never sourced from Rust.
 * Persisted keys (layout/panel visibility) are mirrored to localStorage.
 */

import { create } from "zustand";
import { ZOOM } from "../lib/theme";
import { useProjectStore } from "./projectStore";
import { totalFrames } from "../lib/geometry";
import type { CropAspectLock } from "../lib/cropOverlay";
import { withRangeStart, withRangeEnd, type TimelineRange } from "../lib/timelineRange";
import type { GapSelection } from "../lib/timelineGap";

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
/** 素材/音频下的二级标签：导入（全部素材）/ 我的（星标收藏）。音频 tab 额外有
 *  提取（从视频提取音频）/ 音效（全局音效库，#91/#115）。 */
export type MediaSubTabId = "import" | "mine" | "extract" | "sound";
export type InspectorTabId = "text" | "video" | "audio" | "aiEdit";

export interface SaveAsProgressState {
  operationId: string;
  label: string;
  done: number;
  total: number;
  cancellable: boolean;
  cancelling: boolean;
}

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

function settledFrame(frame: number): number {
  return Math.max(0, Math.round(frame));
}

interface UiState {
  // Top-level navigation
  view: AppView;
  setView: (view: AppView) => void;
  settingsOpen: boolean;
  setSettingsOpen: (open: boolean) => void;
  /** Whether the video-export dialog (§2.4 / #112) is shown. */
  exportDialogOpen: boolean;
  setExportDialogOpen: (open: boolean) => void;
  /** Visible progress owner for clip/range save-as operations. */
  saveAsProgress: SaveAsProgressState | null;
  setSaveAsProgress: (progress: SaveAsProgressState | null) => void;

  // Playback / playhead
  currentFrame: number;
  activeFrame: number;
  isPlaying: boolean;
  isScrubbing: boolean;
  /** Runtime escape hatch for the Rust playback engine: set true when a play
   *  attempt can't bring the engine up (spawn rejects, or no frame by the startup
   *  deadline), so the current session falls back to the legacy <video> stack and
   *  the MJPEG overlay unmounts. Reset to false at the start of every play. */
  rustEngineFailed: boolean;

  // Selection
  selectedClipIds: Set<string>;
  selectedMediaAssetIds: Set<string>;
  selectedFolderIds: Set<string>;
  isMarqueeSelecting: boolean;
  /** Marked in/out timeline range (I/O keys, upstream `selectedTimelineRange`).
   *  Raw endpoints — `startFrame` may exceed `endFrame` mid-mark; consumers gate
   *  through `validRange`. `null` when no range is marked. */
  selectedTimelineRange: TimelineRange | null;
  /** Selected empty gap between clips on one track (upstream `selectedGap`).
   *  Mutually exclusive with clip selection. `null` when no gap is selected. */
  selectedGap: GapSelection | null;

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
  /** Preview render quality (short-edge px) fed to the composite/capture/stream
   *  paths that accept a `max_size` cap. `null` = backend default. Set by the
   *  Preview Quality badge menu (upstream QualityPreset drives resolution, but
   *  OpenTake keeps the timeline dims authoritative and treats this purely as a
   *  preview-render cap; see `previewQualityMaxSize`). Does NOT change the
   *  timeline W/H. */
  previewQualityShortEdge: number | null;
  setPreviewQualityShortEdge: (shortEdge: number | null) => void;
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

  /** On-canvas Crop editing (T3-11). Mirrors upstream `EditorViewModel.cropEditingActive`
   *  (EditorViewModel.swift:91): while `true`, the Preview canvas swaps its
   *  TransformOverlay for a CropOverlay (PreviewContainerView.swift:37-41) and
   *  the Inspector's crop toggle shows active. Reset to `false` on clip-selection
   *  change and on leaving the Video inspector tab (InspectorView.swift:67,90) —
   *  see `selectClips`/`setInspectorTab` below. */
  cropEditingActive: boolean;
  setCropEditingActive: (active: boolean) => void;
  toggleCropEditingActive: () => void;
  /** The crop overlay's aspect-lock preset (T3-11). Mirrors upstream
   *  `EditorViewModel.cropAspectLock` (EditorViewModel.swift:91), default `"free"`. */
  cropAspectLock: CropAspectLock;
  setCropAspectLock: (preset: CropAspectLock) => void;

  // Media panel navigation
  mediaPanelCurrentFolderId: string | null;
  setMediaPanelCurrentFolderId: (id: string | null) => void;

  /** Pending Swap Media flow (SPEC §5.10). When set, a media-picker modal is
   *  shown for the clip with this id; the picker pre-filters candidates by
   *  `item.type === clip.mediaType` (strict, mirroring backend
   *  `isAssetCompatibleWithPendingSwap`). `null` = no swap in flight. */
  pendingSwapClipId: string | null;
  setPendingSwapClipId: (id: string | null) => void;

  // Actions
  setActiveFrame: (frame: number) => void;
  setCurrentFrame: (frame: number) => void;
  setPlaying: (playing: boolean) => void;
  /** Trip the Rust-engine runtime fallback for the current play session. */
  setRustEngineFailed: (failed: boolean) => void;
  /** Toggle play/pause. When STARTING from the parked end-of-timeline frame,
   *  rewinds to 0 first (both tickers stop at the last drawable frame, so without
   *  this the stop check fires immediately and play does nothing). Mirrors
   *  upstream VideoEngine.playbackStartFrame(). */
  togglePlay: () => void;
  mediaPreviewToggleRequest: number;
  requestMediaPreviewToggle: () => void;
  setScrubbing: (scrubbing: boolean) => void;

  selectClips: (ids: Set<string>) => void;
  clearSelection: () => void;
  selectMediaAssets: (ids: Set<string>) => void;
  clearMediaSelection: () => void;

  /** Mark the range START at `frame` (upstream `markTimelineRangeStart`). Also
   *  clears any clip / gap selection (the range is its own selection mode). */
  markRangeStart: (frame: number) => void;
  /** Mark the range END at `frame` (upstream `markTimelineRangeEnd`). */
  markRangeEnd: (frame: number) => void;
  /** Clear the marked range (upstream `clearTimelineRange`, e.g. on Escape). */
  clearTimelineRange: () => void;
  /** Select an empty gap (upstream sets `selectedGap`). Clears clip selection —
   *  gap and clip selection are mutually exclusive. `null` deselects the gap. */
  selectGap: (gap: GapSelection | null) => void;

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
  /** Clear project-scoped runtime state when a different project/session starts.
   *  Preserve user layout and panel visibility preferences. */
  resetProjectRuntimeState: () => void;

  // Toast (transient message)
  toast: { message: string; id: number } | null;
  pushToast: (message: string) => void;
  clearToast: () => void;
}

export const useEditorUiStore = create<UiState>((set, get) => ({
  view: "home",
  setView: (view) => set({ view }),
  settingsOpen: false,
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  exportDialogOpen: false,
  setExportDialogOpen: (exportDialogOpen) => set({ exportDialogOpen }),
  saveAsProgress: null,
  setSaveAsProgress: (saveAsProgress) => set({ saveAsProgress }),

  currentFrame: 0,
  activeFrame: 0,
  isPlaying: false,
  isScrubbing: false,
  rustEngineFailed: false,

  selectedClipIds: new Set(),
  selectedMediaAssetIds: new Set(),
  selectedFolderIds: new Set(),
  isMarqueeSelecting: false,
  selectedTimelineRange: null,
  selectedGap: null,

  zoomScale: ZOOM.default,
  minZoomScale: 0.05,
  scrollLeft: 0,
  scrollTop: 0,
  timelineVisibleWidth: 0,
  toolMode: "pointer",
  trackDisplayHeights: {},

  canvasZoom: 1,
  canvasOffset: { width: 0, height: 0 },
  previewQualityShortEdge: null,
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

  cropEditingActive: false,
  setCropEditingActive: (cropEditingActive) => set({ cropEditingActive }),
  toggleCropEditingActive: () => set((s) => ({ cropEditingActive: !s.cropEditingActive })),
  cropAspectLock: "free",
  setCropAspectLock: (cropAspectLock) => set({ cropAspectLock }),

  mediaPanelCurrentFolderId: null,
  setMediaPanelCurrentFolderId: (mediaPanelCurrentFolderId) => set({ mediaPanelCurrentFolderId }),

  pendingSwapClipId: null,
  setPendingSwapClipId: (pendingSwapClipId) => set({ pendingSwapClipId }),

  setActiveFrame: (activeFrame) => set({ activeFrame }),
  setCurrentFrame: (currentFrame) => set({ currentFrame, activeFrame: currentFrame }),
  setPlaying: (isPlaying) => {
    if (isPlaying) {
      // Reset the Rust-engine fallback on every fresh play so a one-off failure
      // last session doesn't pin this one to legacy.
      set({ isPlaying: true, isScrubbing: false, rustEngineFailed: false });
      return;
    }
    const frame = settledFrame(get().activeFrame);
    set({ currentFrame: frame, activeFrame: frame, isPlaying: false, isScrubbing: false });
  },
  setRustEngineFailed: (rustEngineFailed) => set({ rustEngineFailed }),
  togglePlay: () => {
    const { isPlaying, activeFrame } = get();
    if (isPlaying) {
      const frame = settledFrame(activeFrame);
      set({ currentFrame: frame, activeFrame: frame, isPlaying: false, isScrubbing: false });
      return;
    }
    // Starting playback: if parked at/after the last drawable frame (where the
    // ticker stopped), rewind to the start so `next >= last` doesn't fire on the
    // very first tick and stall play. Without media there's nothing to rewind.
    // Reset the Rust-engine fallback here too (both start-play paths clear it).
    const last = Math.max(0, totalFrames(useProjectStore.getState().timeline) - 1);
    if (activeFrame >= last) {
      set({ currentFrame: 0, activeFrame: 0, isPlaying: true, isScrubbing: false, rustEngineFailed: false });
    } else {
      set({ isPlaying: true, isScrubbing: false, rustEngineFailed: false });
    }
  },
  setScrubbing: (isScrubbing) => set({ isScrubbing }),
  mediaPreviewToggleRequest: 0,
  requestMediaPreviewToggle: () => set((s) => ({ mediaPreviewToggleRequest: s.mediaPreviewToggleRequest + 1 })),

  // Selection change ends crop editing (InspectorView.swift:60-61,90:
  // `resolvePreferredTab()`, called on every `selectedClipIds` change,
  // unconditionally clears `cropEditingActive`).
  // Selecting clips clears any gap selection (upstream: a clip mousedown sets
  // `selectedGap = nil` — the two are mutually exclusive).
  selectClips: (selectedClipIds) =>
    set({ selectedClipIds, selectedGap: null, cropEditingActive: false }),
  clearSelection: () =>
    set({
      selectedClipIds: new Set(),
      selectedGap: null,
      isMarqueeSelecting: false,
      cropEditingActive: false,
    }),
  selectMediaAssets: (selectedMediaAssetIds) => set({ selectedMediaAssetIds }),
  clearMediaSelection: () => set({ selectedMediaAssetIds: new Set() }),
  setPreviewMedia: (previewMediaId) => set({ previewMediaId }),

  // Marking a range is its own selection mode: upstream's ruler range gesture
  // (`beginTimelineRangeSelection`) clears clip + gap selection when it starts.
  markRangeStart: (frame) =>
    set((s) => ({
      selectedTimelineRange: withRangeStart(s.selectedTimelineRange, frame),
      selectedClipIds: new Set(),
      selectedGap: null,
    })),
  markRangeEnd: (frame) =>
    set((s) => ({
      selectedTimelineRange: withRangeEnd(s.selectedTimelineRange, frame),
      selectedClipIds: new Set(),
      selectedGap: null,
    })),
  clearTimelineRange: () => set({ selectedTimelineRange: null }),
  // Selecting a gap clears clip selection (mutual exclusivity, upstream behavior).
  selectGap: (selectedGap) =>
    set(selectedGap ? { selectedGap, selectedClipIds: new Set() } : { selectedGap: null }),

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
  setPreviewQualityShortEdge: (previewQualityShortEdge) => set({ previewQualityShortEdge }),

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
  // Leaving the Video tab ends crop editing (InspectorView.swift:66-68:
  // `if newTab != .video { editor.cropEditingActive = false }`).
  setInspectorTab: (inspectorTab) =>
    set({ inspectorTab, cropEditingActive: inspectorTab === "video" ? get().cropEditingActive : false }),
  resetProjectRuntimeState: () =>
    set({
      currentFrame: 0,
      activeFrame: 0,
      isPlaying: false,
      isScrubbing: false,
      rustEngineFailed: false,
      selectedClipIds: new Set(),
      selectedMediaAssetIds: new Set(),
      selectedFolderIds: new Set(),
      isMarqueeSelecting: false,
      selectedTimelineRange: null,
      selectedGap: null,
      scrollLeft: 0,
      scrollTop: 0,
      toolMode: "pointer",
      trackDisplayHeights: {},
      canvasZoom: 1,
      canvasOffset: { width: 0, height: 0 },
      previewMediaId: null,
      focusedPanel: "timeline",
      maximizedPanel: null,
      cropEditingActive: false,
      cropAspectLock: "free",
      mediaPanelCurrentFolderId: null,
      pendingSwapClipId: null,
    }),

  toast: null,
  pushToast: (message) => set({ toast: { message, id: Date.now() } }),
  clearToast: () => set({ toast: null }),
}));
