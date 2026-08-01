// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useI18nStore } from "../../i18n";
import type { Clip, MatchColorResult, MediaItem } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import { ColorMatchSection, type ColorMatchDependencies } from "./ColorMatchSection";

function clip(overrides: Partial<Clip> = {}): Clip {
  return {
    id: "clip-1",
    mediaRef: "target-1",
    mediaType: "image",
    sourceClipType: "image",
    startFrame: 10,
    durationFrames: 20,
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

function media(id: string, name: string, type: "image" | "video" = "image"): MediaItem {
  return { id, name, type, duration: 1, hasAudio: false, favorite: false };
}

function result(applied: boolean): MatchColorResult {
  return {
    result: {
      clipId: "clip-1",
      referenceMediaRef: "reference-1",
      referenceFrame: 0,
      targetFrame: 12,
      algorithm: "opentake-luma-preserving-mean-match",
      algorithmVersion: 1,
      grade: {
        exposure: 0,
        temperature: 0,
        tint: 0,
        liftGammaGain: {
          lift: { r: 0, g: 0, b: 0 },
          gamma: { r: 1, g: 1, b: 1 },
          gain: { r: 0.8, g: 1.1, b: 1.4 },
        },
        contrast: 0,
        saturation: 1,
      },
      targetMeanLinear: { r: 0.3, g: 0.2, b: 0.1 },
      referenceMeanLinear: { r: 0.15, g: 0.25, b: 0.35 },
      matchedMeanLinear: { r: 0.2, g: 0.22, b: 0.2 },
      deltaEBefore: 22.5,
      deltaEAfter: 0.2,
      targetLumaBefore: 0.21,
      targetLumaAfter: 0.211,
      applied,
    },
    actionName: applied ? "Match Color" : null,
  };
}

describe("ColorMatchSection", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useI18nStore.setState({ locale: "en" });
    useEditorUiStore.setState({ activeFrame: 12 });
    useMediaStore.setState({ items: [media("target-1", "Target"), media("reference-1", "Reference")] });
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it("previews_measured_improvement_then_applies_and_undoes", async () => {
    const generate = vi.fn().mockResolvedValueOnce(result(false)).mockResolvedValueOnce(result(true));
    const undo = vi.fn().mockResolvedValue(undefined);
    const dependencies: ColorMatchDependencies = { generate, cancel: vi.fn(), undo };
    await act(async () => root.render(<ColorMatchSection clip={clip()} dependencies={dependencies} />));
    const button = (text: string) => Array.from(container.querySelectorAll("button")).find((candidate) => candidate.textContent?.includes(text))!;

    await act(async () => button("Analyze match").click());
    expect(generate).toHaveBeenNthCalledWith(1, "clip-1", "reference-1", 0, 12, false);
    expect(container.textContent).toContain("22.50 → 0.20");
    expect(button("Apply editable grade").disabled).toBe(false);

    await act(async () => button("Apply editable grade").click());
    expect(generate).toHaveBeenNthCalledWith(2, "clip-1", "reference-1", 0, 12, true);
    expect(container.textContent).toContain("saved in the project");
    await act(async () => button("Undo color match").click());
    expect(undo).toHaveBeenCalledOnce();
  });

  it("cancels_a_pending_analysis_and_ignores_its_stale_result", async () => {
    let resolve: ((value: MatchColorResult) => void) | undefined;
    const generate = vi.fn(() => new Promise<MatchColorResult>((done) => { resolve = done; }));
    const cancel = vi.fn().mockResolvedValue(true);
    const dependencies: ColorMatchDependencies = { generate, cancel, undo: vi.fn() };
    await act(async () => root.render(<ColorMatchSection clip={clip()} dependencies={dependencies} />));
    const button = (text: string) => Array.from(container.querySelectorAll("button")).find((candidate) => candidate.textContent?.includes(text))!;
    await act(async () => button("Analyze match").click());
    await act(async () => button("Cancel").click());
    expect(cancel).toHaveBeenCalledOnce();
    await act(async () => resolve?.(result(false)));
    expect(container.textContent).not.toContain("22.50 → 0.20");
  });

  it("requires_a_reference_and_refuses_reversed_clips", async () => {
    const dependencies: ColorMatchDependencies = { generate: vi.fn(), cancel: vi.fn(), undo: vi.fn() };
    useMediaStore.setState({ items: [media("target-1", "Target")] });
    await act(async () => root.render(<ColorMatchSection clip={clip()} dependencies={dependencies} />));
    expect(container.textContent).toContain("No other image or video");
    await act(async () => root.render(<ColorMatchSection clip={clip({ reversed: true })} dependencies={dependencies} />));
    expect(container.textContent).toContain("ordinary forward 1x image or video clips");
  });
});
