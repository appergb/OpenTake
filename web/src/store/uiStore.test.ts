// @vitest-environment happy-dom

import { beforeEach, describe, expect, it } from "vitest";
import type { Timeline } from "../lib/types";
import { useProjectStore } from "./projectStore";
import { createEditorUiStore, useEditorUiStore } from "./uiStore";

const timeline: Timeline = {
  fps: 30,
  width: 1920,
  height: 1080,
  settingsConfigured: true,
  tracks: [
    {
      id: "v1",
      type: "video",
      muted: false,
      hidden: false,
      syncLocked: true,
      clips: [
        {
          id: "c1",
          mediaRef: "m1",
          mediaType: "video",
          sourceClipType: "video",
          startFrame: 0,
          durationFrames: 300,
          trimStartFrame: 0,
          trimEndFrame: 300,
          speed: 1,
          volume: 1,
          fadeInFrames: 0,
          fadeOutFrames: 0,
          fadeInInterpolation: "smooth",
          fadeOutInterpolation: "smooth",
          opacity: 1,
          transform: {
            centerX: 0.5,
            centerY: 0.5,
            width: 1,
            height: 1,
            rotation: 0,
            flipHorizontal: false,
            flipVertical: false,
          },
          crop: { left: 0, top: 0, right: 0, bottom: 0 },
        },
      ],
    },
  ],
};

describe("timeline playback state", () => {
  beforeEach(() => {
    localStorage.clear();
    useProjectStore.setState({ timeline, timelineVersion: 1 });
    useEditorUiStore.setState({
      currentFrame: 0,
      activeFrame: 0,
      isPlaying: false,
      isScrubbing: false,
      previewTabIds: [],
      previewTabHistory: [],
      previewActiveTabId: "timeline",
      previewMediaId: null,
      selectedClipIds: new Set(),
      selectedMediaAssetIds: new Set(),
      selectedFolderIds: new Set(),
    });
  });

  it("persists Motion Studio as a primary view and rejects an invalid persisted view", () => {
    const first = createEditorUiStore();
    first.getState().setView("motion");
    expect(localStorage.getItem("opentake.ui.v1.view")).toBe("motion");
    expect(createEditorUiStore().getState().view).toBe("motion");

    localStorage.setItem("opentake.ui.v1.view", "not-a-real-view");
    expect(createEditorUiStore().getState().view).toBe("home");
    expect(localStorage.getItem("opentake.ui.v1.view")).toBeNull();
  });

  it("commits the active playhead frame immediately when pausing", () => {
    useEditorUiStore.setState({ currentFrame: 0, activeFrame: 42, isPlaying: true });

    useEditorUiStore.getState().togglePlay();

    const state = useEditorUiStore.getState();
    expect(state.isPlaying).toBe(false);
    expect(state.activeFrame).toBe(42);
    expect(state.currentFrame).toBe(42);
  });

  it("clears a stale scrub gesture when starting playback", () => {
    useEditorUiStore.setState({ activeFrame: 42, isScrubbing: true });

    useEditorUiStore.getState().togglePlay();

    const state = useEditorUiStore.getState();
    expect(state.isPlaying).toBe(true);
    expect(state.isScrubbing).toBe(false);
  });

  it("clears a stale scrub gesture when pausing playback", () => {
    useEditorUiStore.setState({ activeFrame: 42, isPlaying: true, isScrubbing: true });

    useEditorUiStore.getState().togglePlay();

    const state = useEditorUiStore.getState();
    expect(state.isPlaying).toBe(false);
    expect(state.isScrubbing).toBe(false);
    expect(state.activeFrame).toBe(42);
    expect(state.currentFrame).toBe(42);
  });

  it("resets the Rust-engine fallback flag when starting playback", () => {
    // A previous session tripped the runtime fallback; the next play must retry
    // the engine, not stay pinned to the legacy stack.
    useEditorUiStore.setState({ activeFrame: 42, isPlaying: false, rustEngineFailed: true });

    useEditorUiStore.getState().togglePlay();

    const state = useEditorUiStore.getState();
    expect(state.isPlaying).toBe(true);
    expect(state.rustEngineFailed).toBe(false);
  });

  it("keeps the fallback flag set through a pause (only a new play resets it)", () => {
    useEditorUiStore.setState({ activeFrame: 42, isPlaying: true, rustEngineFailed: true });

    useEditorUiStore.getState().togglePlay(); // pause

    expect(useEditorUiStore.getState().isPlaying).toBe(false);
    // Pausing must not clear it — the paused frame came from the legacy <video>.
    expect(useEditorUiStore.getState().rustEngineFailed).toBe(true);
  });

  it("opens preview tabs idempotently and keeps the newest one active", () => {
    useEditorUiStore.setState({
      previewTabIds: [],
      previewActiveTabId: "timeline",
      previewMediaId: null,
      selectedClipIds: new Set(["clip-1"]),
      selectedFolderIds: new Set(["folder-1"]),
    });

    useEditorUiStore.getState().openPreviewTab("a");
    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: ["a"],
      previewActiveTabId: "media_a",
      previewMediaId: "a",
    });
    expect([...useEditorUiStore.getState().selectedClipIds]).toEqual([]);
    expect([...useEditorUiStore.getState().selectedFolderIds]).toEqual([]);
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["a"]);

    useEditorUiStore.getState().openPreviewTab("a");
    useEditorUiStore.getState().openPreviewTab("b");
    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: ["a", "b"],
      previewActiveTabId: "media_b",
      previewMediaId: "b",
    });
  });

  it("selects timeline without changing preview tab order", () => {
    useEditorUiStore.setState({
      previewTabIds: ["a", "b"],
      previewActiveTabId: "media_b",
      previewMediaId: "b",
      selectedMediaAssetIds: new Set(["b"]),
    });

    useEditorUiStore.getState().selectPreviewTab("timeline");

    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: ["a", "b"],
      previewActiveTabId: "timeline",
      previewMediaId: null,
    });
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual([]);
  });

  it("falls back to the previous valid tab when closing the active source", () => {
    useEditorUiStore.setState({
      previewTabIds: ["a", "b"],
      previewActiveTabId: "media_b",
      previewMediaId: "b",
    });

    useEditorUiStore.getState().closePreviewTab("media_b");
    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: ["a"],
      previewActiveTabId: "media_a",
      previewMediaId: "a",
    });

    useEditorUiStore.getState().closePreviewTab("media_a");
    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: [],
      previewActiveTabId: "timeline",
      previewMediaId: null,
    });

    useEditorUiStore.getState().closePreviewTab("timeline");
    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: [],
      previewActiveTabId: "timeline",
      previewMediaId: null,
    });
  });

  it("returns to the most recent valid preview tab from history when closing the active tab", () => {
    useEditorUiStore.getState().openPreviewTab("a");
    useEditorUiStore.getState().openPreviewTab("b");
    useEditorUiStore.getState().openPreviewTab("c");
    useEditorUiStore.getState().selectPreviewTab("media_a");

    useEditorUiStore.getState().closePreviewTab("media_a");

    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: ["b", "c"],
      previewActiveTabId: "media_c",
      previewMediaId: "c",
    });
  });

  it("does not change the active tab when closing a non-active media tab", () => {
    useEditorUiStore.getState().openPreviewTab("a");
    useEditorUiStore.getState().openPreviewTab("b");
    useEditorUiStore.getState().openPreviewTab("c");

    useEditorUiStore.getState().closePreviewTab("media_b");

    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: ["a", "c"],
      previewActiveTabId: "media_c",
      previewMediaId: "c",
    });
  });

  it("restores the exact timeline-only state when all preview tabs close", () => {
    useEditorUiStore.setState({
      previewTabIds: ["a", "b"],
      previewActiveTabId: "media_b",
      previewMediaId: "b",
    });

    useEditorUiStore.getState().closeAllPreviewTabs();

    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: [],
      previewActiveTabId: "timeline",
      previewMediaId: null,
    });
  });

  it("normalizes legacy previewMediaId-only state when selecting and closing a synthetic media tab", () => {
    useEditorUiStore.setState({
      previewTabIds: [],
      previewTabHistory: [],
      previewActiveTabId: "timeline",
      previewMediaId: "legacy",
      selectedClipIds: new Set(["clip-1"]),
      selectedFolderIds: new Set(["folder-1"]),
      selectedMediaAssetIds: new Set(),
    });

    useEditorUiStore.getState().selectPreviewTab("media_legacy");
    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: ["legacy"],
      previewActiveTabId: "media_legacy",
      previewMediaId: "legacy",
    });
    expect([...useEditorUiStore.getState().selectedClipIds]).toEqual([]);
    expect([...useEditorUiStore.getState().selectedFolderIds]).toEqual([]);
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["legacy"]);

    useEditorUiStore.getState().closePreviewTab("media_legacy");
    expect(useEditorUiStore.getState()).toMatchObject({
      previewTabIds: [],
      previewActiveTabId: "timeline",
      previewMediaId: null,
    });
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual([]);
  });
});
