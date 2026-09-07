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
import { isTauri } from "../lib/api";
import { t } from "../i18n";
import type { ProjectSettingsTarget } from "../lib/projectSettings";
import type { MotionTrackingRegion } from "../lib/types";

export type Panel = "agent" | "media" | "preview" | "inspector" | "timeline";
/** Top-level app view (SPEC: 启动先进主页). The editor is one of four primary views;
 *  switching is in-app (no router) so editor state survives navigation. */
export type AppView = "home" | "editor" | "motion" | "settings" | "library";
export type ToolMode = "pointer" | "razor";
export type LayoutPreset = "default" | "media" | "vertical";
export type SettingsPaneId =
  | "general"
  | "appearance"
  | "import"
  | "ai"
  | "mcp"
  | "shortcuts"
  | "account"
  | "storage"
  | "about";
/** 剪映式顶部素材面板主标签（英文标识符，中文文案在 dict）。 */
export type MediaTabId =
  | "material"
  | "audio"
  | "music"
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

export interface ProjectSettingsPromptState {
  current: ProjectSettingsTarget;
  suggested: ProjectSettingsTarget;
}

export interface MotionTrackingSelection {
  clipId: string;
  region: MotionTrackingRegion;
}

const STORAGE_PREFIX = "opentake.ui.v1.";
const LS = {
  view: `${STORAGE_PREFIX}view`,
  layoutPreset: `${STORAGE_PREFIX}layoutPreset`,
  agentPanelVisible: `${STORAGE_PREFIX}agentPanelVisible`,
  mediaPanelVisible: `${STORAGE_PREFIX}mediaPanelVisible`,
  inspectorPanelVisible: `${STORAGE_PREFIX}inspectorPanelVisible`,
  keyframesPanelVisible: `${STORAGE_PREFIX}keyframesPanelVisible`,
  zoomScale: `${STORAGE_PREFIX}zoomScale`,
} as const;

type UiStorageKey = (typeof LS)[keyof typeof LS];

const LEGACY_LS: Record<UiStorageKey, string> = {
  [LS.view]: "view",
  [LS.layoutPreset]: "layoutPreset",
  [LS.agentPanelVisible]: "agentPanelVisible",
  [LS.mediaPanelVisible]: "mediaPanelVisible",
  [LS.inspectorPanelVisible]: "inspectorPanelVisible",
  [LS.keyframesPanelVisible]: "keyframesPanelVisible",
  [LS.zoomScale]: "zoomScale",
};

function loadAppView(): AppView {
  const stored = readPersisted(LS.view);
  if (
    stored.value !== "home" &&
    stored.value !== "editor" &&
    stored.value !== "motion" &&
    stored.value !== "library"
  ) {
    if (stored.value !== null) discardPersisted(LS.view);
    return "home";
  }
  if (stored.legacy) persist(LS.view, stored.value);
  return stored.value;
}

function readPersisted(key: UiStorageKey): { value: string | null; legacy: boolean } {
  if (typeof localStorage === "undefined") return { value: null, legacy: false };
  try {
    const value = localStorage.getItem(key);
    if (value !== null) return { value, legacy: false };
    return { value: localStorage.getItem(LEGACY_LS[key]), legacy: true };
  } catch {
    return { value: null, legacy: false };
  }
}

function discardPersisted(key: UiStorageKey): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.removeItem(key);
    localStorage.removeItem(LEGACY_LS[key]);
  } catch {
    // Storage may be unavailable; defaults still fail closed for this session.
  }
}

function loadBool(key: UiStorageKey, fallback: boolean): boolean {
  const stored = readPersisted(key);
  if (stored.value !== "true" && stored.value !== "false") {
    if (stored.value !== null) discardPersisted(key);
    return fallback;
  }
  if (stored.legacy) persist(key, stored.value);
  return stored.value === "true";
}
function loadPreset(): LayoutPreset {
  const stored = readPersisted(LS.layoutPreset);
  if (stored.value !== "default" && stored.value !== "media" && stored.value !== "vertical") {
    if (stored.value !== null) discardPersisted(LS.layoutPreset);
    return "default";
  }
  if (stored.legacy) persist(LS.layoutPreset, stored.value);
  return stored.value;
}
function loadZoomScale(): number {
  const stored = readPersisted(LS.zoomScale);
  if (stored.value === null) return ZOOM.default;
  const value = Number(stored.value);
  if (!Number.isFinite(value) || value < 0.05 || value > ZOOM.max) {
    discardPersisted(LS.zoomScale);
    return ZOOM.default;
  }
  if (stored.legacy) persist(LS.zoomScale, String(value));
  return value;
}
function persist(key: UiStorageKey, value: string): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(key, value);
  } catch {
    // Sandboxed/private browsing can deny storage. UI state must still update
    // for the current session and simply fall back to defaults next launch.
  }
}

function settledFrame(frame: number): number {
  return Math.max(0, Math.round(frame));
}

const PREVIEW_TIMELINE_TAB_ID = "timeline";

export function previewMediaTabId(mediaId: string): string {
  return `media_${mediaId}`;
}

export function previewTabMediaId(tabId: string): string | null {
  return tabId.startsWith("media_") && tabId.length > "media_".length
    ? tabId.slice("media_".length)
    : null;
}

function dedupePreviewTabIds(previewTabIds: string[]): string[] {
  return [...new Set(previewTabIds.filter((id) => id !== ""))];
}

function previewHistoryWithActivation(previewTabHistory: string[], mediaId: string): string[] {
  return [...previewTabHistory.filter((id) => id !== mediaId), mediaId];
}

interface EffectivePreviewState {
  previewTabIds: string[];
  previewTabHistory: string[];
  previewActiveTabId: string;
}

export function resolveEffectivePreviewState(
  state: Pick<
    UiState,
    | "previewTabHistory"
    | "previewTabIds"
    | "previewActiveTabId"
    | "previewMediaId"
  >,
): EffectivePreviewState {
  const previewTabIds = dedupePreviewTabIds(state.previewTabIds);
  const previewMediaId = state.previewMediaId;
  const activeMediaId = previewTabMediaId(state.previewActiveTabId);
  const activeTabValid =
    state.previewActiveTabId === PREVIEW_TIMELINE_TAB_ID
      ? previewMediaId === null
      : activeMediaId !== null && previewTabIds.includes(activeMediaId);
  const effectivePreviewTabIds =
    previewMediaId &&
    !previewTabIds.includes(previewMediaId) &&
    (previewTabIds.length === 0 || !activeTabValid)
      ? [...previewTabIds, previewMediaId]
      : previewTabIds;
  const previewTabHistory = state.previewTabHistory.filter((id) =>
    effectivePreviewTabIds.includes(id),
  );
  const effectivePreviewTabHistory =
    previewMediaId &&
    effectivePreviewTabIds.includes(previewMediaId) &&
    !previewTabHistory.includes(previewMediaId)
      ? [...previewTabHistory, previewMediaId]
      : previewTabHistory;
  const effectivePreviewActiveTabId =
    state.previewActiveTabId !== PREVIEW_TIMELINE_TAB_ID &&
    activeMediaId !== null &&
    effectivePreviewTabIds.includes(activeMediaId)
      ? state.previewActiveTabId
      : previewMediaId && effectivePreviewTabIds.includes(previewMediaId)
        ? previewMediaTabId(previewMediaId)
        : PREVIEW_TIMELINE_TAB_ID;

  return {
    previewTabIds: effectivePreviewTabIds,
    previewTabHistory: effectivePreviewTabHistory,
    previewActiveTabId: effectivePreviewActiveTabId,
  };
}

function previewSelectionState(
  state: Pick<
    UiState,
    | "previewTabHistory"
    | "previewTabIds"
    | "previewActiveTabId"
    | "previewMediaId"
  >,
  previewTabIds: string[],
  tabId: string,
  options?: { updateHistory?: boolean },
) {
  const normalizedPreviewTabIds = dedupePreviewTabIds(previewTabIds);
  const normalizedPreviewTabHistory = state.previewTabHistory.filter((id) =>
    normalizedPreviewTabIds.includes(id),
  );
  if (tabId === PREVIEW_TIMELINE_TAB_ID) {
    return {
      previewTabIds: normalizedPreviewTabIds,
      previewActiveTabId: PREVIEW_TIMELINE_TAB_ID,
      previewMediaId: null,
      previewTabHistory: normalizedPreviewTabHistory,
      selectedMediaAssetIds: new Set<string>(),
    };
  }
  const mediaId = previewTabMediaId(tabId);
  if (!mediaId || !normalizedPreviewTabIds.includes(mediaId)) return null;
  return {
    previewTabIds: normalizedPreviewTabIds,
    previewActiveTabId: tabId,
    previewMediaId: mediaId,
    previewTabHistory:
      options?.updateHistory === false
        ? normalizedPreviewTabHistory
        : previewHistoryWithActivation(normalizedPreviewTabHistory, mediaId),
    selectedClipIds: new Set<string>(),
    selectedFolderIds: new Set<string>(),
    selectedMediaAssetIds: new Set([mediaId]),
  };
}

interface UiState {
  // Top-level navigation
  view: AppView;
  setView: (view: AppView) => void;
  settingsOpen: boolean;
  setSettingsOpen: (open: boolean) => void;
  settingsPane: SettingsPaneId;
  openSettingsPane: (pane: SettingsPaneId) => void;
  setSettingsPane: (pane: SettingsPaneId) => void;
  /** Whether the video-export dialog (§2.4 / #112) is shown. */
  exportDialogOpen: boolean;
  setExportDialogOpen: (open: boolean) => void;
  /** Visible progress owner for clip/range save-as operations. */
  saveAsProgress: SaveAsProgressState | null;
  setSaveAsProgress: (progress: SaveAsProgressState | null) => void;
  projectSettingsPrompt: ProjectSettingsPromptState | null;
  projectSettingsPromptResolver: ((applySuggested: boolean) => void) | null;
  requestProjectSettingsPrompt: (prompt: ProjectSettingsPromptState) => Promise<boolean>;
  resolveProjectSettingsPrompt: (applySuggested: boolean) => void;

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
  /** Project/timeline revision whose WebKit video decoder failed. That exact
   *  revision is retried through the native decoder; other revisions ignore it. */
  webkitPlaybackFailedRevision: string | null;

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
  /** Root-registry sequence currently opened for direct timeline editing. */
  activeNestedSequenceId: string | null;
  enterNestedSequence: (sequenceId: string) => void;
  exitNestedSequence: () => void;
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
  /** Open media preview tabs in insertion order; the timeline tab is implicit. */
  previewTabIds: string[];
  /** Most-recently activated media preview tabs; timeline is implicit. */
  previewTabHistory: string[];
  /** Media asset previewed in the canvas (clicked in the media panel). `null`
   *  shows the timeline composite. Mirrors upstream `openPreviewTab(mediaAsset)`. */
  previewMediaId: string | null;
  openPreviewTab: (mediaId: string) => void;
  selectPreviewTab: (tabId: string) => void;
  closePreviewTab: (tabId: string) => void;
  closeAllPreviewTabs: () => void;
  setPreviewMedia: (id: string | null) => void;
  /** Active on-canvas subject rectangle for the Inspector motion tracker. */
  motionTrackingSelection: MotionTrackingSelection | null;
  setMotionTrackingSelection: (selection: MotionTrackingSelection | null) => void;
  setMotionTrackingRegion: (region: MotionTrackingRegion) => void;

  // Panels
  focusedPanel: Panel | null;
  maximizedPanel: Panel | null;
  /** Mirrors the desktop/browser window's fullscreen state for the checked View
   *  menu item. The action always queries the real window before toggling. */
  fullscreen: boolean;
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
  setWebkitPlaybackFailedRevision: (revision: string | null) => void;
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

  focusPanel: (panel: Panel, preserveTimelineSelection?: boolean) => void;
  setMaximizedPanel: (panel: Panel | null) => void;
  toggleMaximizedFocusedPanel: () => void;
  syncFullscreen: () => Promise<void>;
  toggleFullscreen: () => Promise<void>;
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

export const createEditorUiStore = () => create<UiState>((set, get) => ({
  view: loadAppView(),
  setView: (view) => {
    if (view === "home" || view === "editor" || view === "motion" || view === "library") {
      persist(LS.view, view);
    } else {
      discardPersisted(LS.view);
    }
    set({ view });
  },
  settingsOpen: false,
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  settingsPane: "general",
  openSettingsPane: (settingsPane) => set({ settingsPane, settingsOpen: true }),
  setSettingsPane: (settingsPane) => set({ settingsPane }),
  exportDialogOpen: false,
  setExportDialogOpen: (exportDialogOpen) => set({ exportDialogOpen }),
  saveAsProgress: null,
  setSaveAsProgress: (saveAsProgress) => set({ saveAsProgress }),
  projectSettingsPrompt: null,
  projectSettingsPromptResolver: null,
  requestProjectSettingsPrompt: (projectSettingsPrompt) =>
    new Promise<boolean>((resolve) => {
      get().projectSettingsPromptResolver?.(false);
      set({ projectSettingsPrompt, projectSettingsPromptResolver: resolve });
    }),
  resolveProjectSettingsPrompt: (applySuggested) => {
    const resolve = get().projectSettingsPromptResolver;
    set({ projectSettingsPrompt: null, projectSettingsPromptResolver: null });
    resolve?.(applySuggested);
  },

  currentFrame: 0,
  activeFrame: 0,
  isPlaying: false,
  isScrubbing: false,
  rustEngineFailed: false,
  webkitPlaybackFailedRevision: null,

  selectedClipIds: new Set(),
  selectedMediaAssetIds: new Set(),
  selectedFolderIds: new Set(),
  isMarqueeSelecting: false,
  selectedTimelineRange: null,
  selectedGap: null,

  activeNestedSequenceId: null,
  enterNestedSequence: (activeNestedSequenceId) =>
    set({
      activeNestedSequenceId,
      selectedClipIds: new Set(),
      selectedGap: null,
      selectedTimelineRange: null,
      currentFrame: 0,
      activeFrame: 0,
      scrollLeft: 0,
      scrollTop: 0,
    }),
  exitNestedSequence: () =>
    set({
      activeNestedSequenceId: null,
      selectedClipIds: new Set(),
      selectedGap: null,
      selectedTimelineRange: null,
      currentFrame: 0,
      activeFrame: 0,
      scrollLeft: 0,
      scrollTop: 0,
    }),
  zoomScale: loadZoomScale(),
  minZoomScale: 0.05,
  scrollLeft: 0,
  scrollTop: 0,
  timelineVisibleWidth: 0,
  toolMode: "pointer",
  trackDisplayHeights: {},

  canvasZoom: 1,
  canvasOffset: { width: 0, height: 0 },
  previewQualityShortEdge: null,
  previewTabIds: [],
  previewTabHistory: [],
  previewMediaId: null,
  motionTrackingSelection: null,
  setMotionTrackingSelection: (motionTrackingSelection) => set({ motionTrackingSelection }),
  setMotionTrackingRegion: (region) =>
    set((state) => ({
      motionTrackingSelection: state.motionTrackingSelection
        ? { ...state.motionTrackingSelection, region }
        : null,
    })),

  focusedPanel: "timeline",
  maximizedPanel: null,
  fullscreen: false,
  layoutPreset: loadPreset(),
  agentPanelVisible: loadBool(LS.agentPanelVisible, false),
  mediaPanelVisible: loadBool(LS.mediaPanelVisible, true),
  inspectorPanelVisible: loadBool(LS.inspectorPanelVisible, true),
  keyframesPanelVisible: loadBool(LS.keyframesPanelVisible, false),

  mediaTab: "material",
  mediaSubTab: "import",
  inspectorTab: "video",
  previewActiveTabId: PREVIEW_TIMELINE_TAB_ID,

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
  setWebkitPlaybackFailedRevision: (webkitPlaybackFailedRevision) =>
    set({ webkitPlaybackFailedRevision }),
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
    set((state) => ({
      selectedClipIds,
      selectedGap: null,
      cropEditingActive: false,
      motionTrackingSelection:
        state.motionTrackingSelection && selectedClipIds.has(state.motionTrackingSelection.clipId)
          ? state.motionTrackingSelection
          : null,
    })),
  clearSelection: () =>
    set({
      selectedClipIds: new Set(),
      selectedGap: null,
      isMarqueeSelecting: false,
      cropEditingActive: false,
      motionTrackingSelection: null,
    }),
  selectMediaAssets: (selectedMediaAssetIds) => set({ selectedMediaAssetIds }),
  clearMediaSelection: () => set({ selectedMediaAssetIds: new Set() }),
  openPreviewTab: (mediaId) =>
    set((state) => {
      if (!mediaId) return {};
      const effectivePreviewState = resolveEffectivePreviewState(state);
      const previewTabIds = effectivePreviewState.previewTabIds.includes(mediaId)
        ? effectivePreviewState.previewTabIds
        : [...effectivePreviewState.previewTabIds, mediaId];
      return (
        previewSelectionState(state, previewTabIds, previewMediaTabId(mediaId)) ?? {}
      );
    }),
  selectPreviewTab: (tabId) =>
    set((state) => {
      const effectivePreviewState = resolveEffectivePreviewState(state);
      return (
        previewSelectionState(state, effectivePreviewState.previewTabIds, tabId) ?? {}
      );
    }),
  closePreviewTab: (tabId) =>
    set((state) => {
      if (tabId === PREVIEW_TIMELINE_TAB_ID) return {};
      const mediaId = previewTabMediaId(tabId);
      if (!mediaId) return {};
      const effectivePreviewState = resolveEffectivePreviewState(state);
      const closingIndex = effectivePreviewState.previewTabIds.indexOf(mediaId);
      if (closingIndex < 0) return {};
      const previewTabIds = effectivePreviewState.previewTabIds.filter((id) => id !== mediaId);
      if (effectivePreviewState.previewActiveTabId !== tabId) {
        return {
          ...(previewSelectionState(
            state,
            previewTabIds,
            effectivePreviewState.previewActiveTabId,
            { updateHistory: false },
          ) ?? {}),
          previewTabHistory: effectivePreviewState.previewTabHistory.filter((id) => id !== mediaId),
        };
      }
      const previewTabHistory = effectivePreviewState.previewTabHistory.filter((id) => id !== mediaId);
      const fallbackMediaId =
        previewTabHistory[previewTabHistory.length - 1] ??
        (closingIndex > 0 ? effectivePreviewState.previewTabIds[closingIndex - 1] ?? null : null);
      return (
        previewSelectionState(
          { ...state, previewTabHistory },
          previewTabIds,
          fallbackMediaId ? previewMediaTabId(fallbackMediaId) : PREVIEW_TIMELINE_TAB_ID,
          { updateHistory: false },
        ) ?? {}
      );
    }),
  closeAllPreviewTabs: () =>
    set((state) => previewSelectionState(state, [], PREVIEW_TIMELINE_TAB_ID) ?? {}),
  setPreviewMedia: (previewMediaId) => {
    if (previewMediaId === null) {
    set((state) =>
        previewSelectionState(
          state,
          resolveEffectivePreviewState(state).previewTabIds,
          PREVIEW_TIMELINE_TAB_ID,
        ) ?? {},
      );
      return;
    }
    set((state) => {
      const effectivePreviewState = resolveEffectivePreviewState(state);
      const previewTabIds = effectivePreviewState.previewTabIds.includes(previewMediaId)
        ? effectivePreviewState.previewTabIds
        : [...effectivePreviewState.previewTabIds, previewMediaId];
      return (
        previewSelectionState(state, previewTabIds, previewMediaTabId(previewMediaId)) ?? {}
      );
    });
  },

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

  setZoomScale: (zoomScale) => {
    const requested = Number.isFinite(zoomScale) ? zoomScale : ZOOM.default;
    const bounded = Math.max(get().minZoomScale, Math.min(ZOOM.max, requested));
    persist(LS.zoomScale, String(bounded));
    set({ zoomScale: bounded });
  },
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

  focusPanel: (panel, preserveTimelineSelection = false) => {
    // Panel-click side effects (EditorWindowController.swift:188-189):
    // entering media clears clip selection; entering timeline clears asset sel.
    // Transition authoring is the one media-panel flow that consumes the
    // timeline cut selection, so its marked controls focus without clearing it.
    if (panel === "media" && !preserveTimelineSelection)
      set({ focusedPanel: panel, selectedClipIds: new Set() });
    else if (panel === "media") set({ focusedPanel: panel });
    else if (panel === "timeline")
      set({ focusedPanel: panel, selectedMediaAssetIds: new Set() });
    else set({ focusedPanel: panel });
  },
  setMaximizedPanel: (maximizedPanel) => set({ maximizedPanel }),
  toggleMaximizedFocusedPanel: () =>
    set((state) => {
      if (!state.focusedPanel) return {};
      return {
        maximizedPanel: state.maximizedPanel ? null : state.focusedPanel,
      };
    }),
  syncFullscreen: async () => {
    if (isTauri) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      set({ fullscreen: await getCurrentWindow().isFullscreen() });
      return;
    }
    set({ fullscreen: Boolean(document.fullscreenElement) });
  },
  toggleFullscreen: async () => {
    try {
      if (isTauri) {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const window = getCurrentWindow();
        const fullscreen = !(await window.isFullscreen());
        await window.setFullscreen(fullscreen);
        set({ fullscreen });
        return;
      }
      if (document.fullscreenElement) {
        await document.exitFullscreen?.();
      } else {
        await document.documentElement.requestFullscreen?.();
      }
      set({ fullscreen: Boolean(document.fullscreenElement) });
    } catch {
      get().pushToast(t("view.fullscreenFailed"));
    }
  },
  setLayoutPreset: (layoutPreset) => {
    persist(LS.layoutPreset, layoutPreset);
    set({ layoutPreset });
  },
  toggleAgentPanel: () =>
    set((s) => {
      const agentPanelVisible = !s.agentPanelVisible;
      persist(LS.agentPanelVisible, String(agentPanelVisible));
      return {
        agentPanelVisible,
        ...(agentPanelVisible
          ? {}
          : {
              focusedPanel: s.focusedPanel === "agent" ? "timeline" : s.focusedPanel,
              maximizedPanel: s.maximizedPanel === "agent" ? null : s.maximizedPanel,
            }),
      };
    }),
  toggleMediaPanel: () =>
    set((s) => {
      const mediaPanelVisible = !s.mediaPanelVisible;
      persist(LS.mediaPanelVisible, String(mediaPanelVisible));
      return {
        mediaPanelVisible,
        ...(mediaPanelVisible
          ? {}
          : {
              focusedPanel: s.focusedPanel === "media" ? "timeline" : s.focusedPanel,
              maximizedPanel: s.maximizedPanel === "media" ? null : s.maximizedPanel,
            }),
      };
    }),
  toggleInspectorPanel: () =>
    set((s) => {
      const inspectorPanelVisible = !s.inspectorPanelVisible;
      persist(LS.inspectorPanelVisible, String(inspectorPanelVisible));
      return {
        inspectorPanelVisible,
        ...(inspectorPanelVisible
          ? {}
          : {
              focusedPanel: s.focusedPanel === "inspector" ? "timeline" : s.focusedPanel,
              maximizedPanel: s.maximizedPanel === "inspector" ? null : s.maximizedPanel,
            }),
      };
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
    set({
      inspectorTab,
      cropEditingActive: inspectorTab === "video" ? get().cropEditingActive : false,
      motionTrackingSelection:
        inspectorTab === "video" ? get().motionTrackingSelection : null,
    }),
  resetProjectRuntimeState: () => {
    get().projectSettingsPromptResolver?.(false);
    set({
      currentFrame: 0,
      activeFrame: 0,
      isPlaying: false,
      isScrubbing: false,
      rustEngineFailed: false,
      webkitPlaybackFailedRevision: null,
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
      previewTabIds: [],
      previewTabHistory: [],
      previewActiveTabId: PREVIEW_TIMELINE_TAB_ID,
      previewMediaId: null,
      motionTrackingSelection: null,
      focusedPanel: "timeline",
      maximizedPanel: null,
      cropEditingActive: false,
      cropAspectLock: "free",
      mediaPanelCurrentFolderId: null,
      pendingSwapClipId: null,
      projectSettingsPrompt: null,
      projectSettingsPromptResolver: null,
    });
  },

  toast: null,
  pushToast: (message) => set({ toast: { message, id: Date.now() } }),
  clearToast: () => set({ toast: null }),
}));

export const useEditorUiStore = createEditorUiStore();
