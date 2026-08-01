// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useI18nStore } from "../../i18n";
import type { Clip, MotionTrackingResult } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";
import {
  MotionTrackingSection,
  type MotionTrackingDependencies,
} from "./MotionTrackingSection";

function clip(overrides: Partial<Clip> = {}): Clip {
  return {
    id: "clip-1",
    mediaRef: "video-1",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 10,
    durationFrames: 30,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform: { centerX: 0.5, centerY: 0.5, width: 1, height: 1, rotation: 0, flipHorizontal: false, flipVertical: false },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    ...overrides,
  };
}

function result(applied: boolean): MotionTrackingResult {
  return {
    result: {
      clipId: "clip-1",
      applied,
      algorithm: "opentake.region-block-match",
      algorithmVersion: 1,
      minimumConfidence: 0.91,
      region: { x: 0.25, y: 0.25, width: 0.5, height: 0.5 },
      keyframes: [
        { frame: 0, position: { x: 0.5, y: 0.5 }, interpolation: "linear" },
        { frame: 29, position: { x: 0.6, y: 0.55 }, interpolation: "linear" },
      ],
    },
    actionName: applied ? "Set Keyframes" : null,
  };
}

describe("MotionTrackingSection", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useI18nStore.setState({ locale: "en" });
    useEditorUiStore.setState({
      motionTrackingSelection: null,
      selectedClipIds: new Set(["clip-1"]),
    });
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it("selects_a_preview_region_then_previews_applies_and_undoes", async () => {
    const generate = vi.fn().mockResolvedValueOnce(result(false)).mockResolvedValueOnce(result(true));
    const undo = vi.fn().mockResolvedValue(undefined);
    const dependencies: MotionTrackingDependencies = { generate, cancel: vi.fn(), undo };
    await act(async () => root.render(<MotionTrackingSection clip={clip()} dependencies={dependencies} />));
    const button = (text: string) => Array.from(container.querySelectorAll("button")).find((candidate) => candidate.textContent?.includes(text))!;

    await act(async () => button("Select region in preview").click());
    expect(useEditorUiStore.getState().motionTrackingSelection).toEqual({
      clipId: "clip-1",
      region: { x: 0.25, y: 0.25, width: 0.5, height: 0.5 },
    });
    await act(async () => button("Analyze motion").click());
    expect(generate).toHaveBeenNthCalledWith(
      1,
      "clip-1",
      { x: 0.25, y: 0.25, width: 0.5, height: 0.5 },
      { startFrame: 10, endFrame: 40 },
      false,
    );
    expect(container.textContent).toContain("91%");
    expect(container.textContent).toContain("2 keyframes");
    await act(async () => button("Apply tracking").click());
    expect(generate).toHaveBeenNthCalledWith(2, "clip-1", expect.any(Object), expect.any(Object), true);
    await act(async () => button("Undo tracking").click());
    expect(undo).toHaveBeenCalledOnce();
  });

  it("cancels_pending_tracking_and_rejects_incompatible_clips", async () => {
    let resolve: ((value: MotionTrackingResult) => void) | undefined;
    const generate = vi.fn(() => new Promise<MotionTrackingResult>((done) => { resolve = done; }));
    const cancel = vi.fn().mockResolvedValue(true);
    const dependencies: MotionTrackingDependencies = { generate, cancel, undo: vi.fn() };
    await act(async () => root.render(<MotionTrackingSection clip={clip()} dependencies={dependencies} />));
    const button = (text: string) => Array.from(container.querySelectorAll("button")).find((candidate) => candidate.textContent?.includes(text))!;
    await act(async () => button("Analyze motion").click());
    await act(async () => button("Cancel").click());
    expect(cancel).toHaveBeenCalledOnce();
    await act(async () => resolve?.(result(false)));
    expect(container.textContent).not.toContain("91%");

    await act(async () => root.render(<MotionTrackingSection clip={clip({ reversed: true })} dependencies={dependencies} />));
    expect(container.textContent).toContain("ordinary forward 1x video clips");
  });
});
