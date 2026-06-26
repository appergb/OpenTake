import { beforeEach, describe, expect, it } from "vitest";
import type { Timeline } from "../lib/types";
import { useProjectStore } from "./projectStore";
import { useEditorUiStore } from "./uiStore";

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
    useProjectStore.setState({ timeline, timelineVersion: 1 });
    useEditorUiStore.setState({
      currentFrame: 0,
      activeFrame: 0,
      isPlaying: false,
      isScrubbing: false,
      previewMediaId: null,
    });
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
});
