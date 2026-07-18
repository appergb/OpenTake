// @vitest-environment happy-dom

import React from "react";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Clip, Timeline } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";

vi.mock("../../lib/asset", () => ({
  assetUrl: (path: string | null | undefined) => (path ? `asset://${path}` : null),
}));

import { TimelinePlayback } from "./TimelinePlaybackLayer";

afterEach(() => {
  document.body.replaceChildren();
  useMediaStore.setState({ items: [], folders: [], importing: false, error: null });
});

describe("WebKit video failure handoff", () => {
  it("reports a decode error so the shared playback route can switch to native", async () => {
    const clip = {
      id: "main10-clip",
      mediaRef: "main10",
      mediaType: "video",
      sourceClipType: "video",
      startFrame: 204,
      durationFrames: 6_321,
      trimStartFrame: 0,
      trimEndFrame: 0,
      speed: 1,
      volume: 1,
      fadeInFrames: 0,
      fadeOutFrames: 0,
      fadeInInterpolation: "linear",
      fadeOutInterpolation: "linear",
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
    } as Clip;
    const timeline: Timeline = {
      fps: 30,
      width: 1_920,
      height: 1_080,
      settingsConfigured: true,
      tracks: [
        {
          id: "v1",
          type: "video",
          muted: false,
          hidden: false,
          syncLocked: true,
          clips: [clip],
        },
      ],
    };
    useEditorUiStore.setState({ activeFrame: 480 });
    useMediaStore.setState({
      items: [
        {
          id: "main10",
          name: "A-roll",
          type: "video",
          duration: 210,
          hasAudio: true,
          path: "/main10.mov",
          favorite: false,
        },
      ],
    });
    const onPlaybackFailure = vi.fn();
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);

    await act(async () =>
      root.render(
        React.createElement(TimelinePlayback, {
          timeline,
          fps: 30,
          onPlaybackFailure,
        } as never),
      ),
    );
    const video = container.querySelector("video");
    expect(video).not.toBeNull();
    await act(async () => video!.dispatchEvent(new Event("error")));

    expect(onPlaybackFailure).toHaveBeenCalledWith("main10-clip");
    await act(async () => root.unmount());
  });
});
