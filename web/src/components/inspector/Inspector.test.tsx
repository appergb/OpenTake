// @vitest-environment happy-dom

import React from "react";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Clip, Timeline } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import { useProjectStore } from "../../store/projectStore";
import * as edit from "../../store/editActions";
import * as api from "../../lib/api";
import { Inspector } from "./Inspector";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function visualClip(overrides: Partial<Clip> = {}): Clip {
  return {
    id: "clip-1",
    mediaRef: "media-1",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 0,
    durationFrames: 90,
    trimStartFrame: 0,
    trimEndFrame: 90,
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
    ...overrides,
  };
}

function timelineWith(clip: Clip): Timeline {
  return {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks: [
      {
        id: "video-1",
        name: "Video 1",
        type: "video",
        muted: false,
        hidden: false,
        syncLocked: false,
        clips: [clip],
      },
    ],
  };
}

afterEach(() => {
  document.body.replaceChildren();
  useEditorUiStore.setState({
    selectedClipIds: new Set(),
    inspectorTab: "video",
    keyframesPanelVisible: false,
  });
  useMediaStore.setState({ items: [], folders: [], importing: false, error: null });
  useProjectStore.getState().clearProjectSnapshot();
});

describe("Inspector completion surface", () => {
  it("four_states_tabs_fields_and_lanes", async () => {
    const clip = visualClip();
    useProjectStore.setState({ timeline: timelineWith(clip), projectPath: "/tmp/demo.opentake" });
    useEditorUiStore.setState({ selectedClipIds: new Set([clip.id]), inspectorTab: "video" });

    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<Inspector />));

    const tabs = [...container.querySelectorAll<HTMLElement>('[role="tab"]')];
    expect(tabs.map((tab) => tab.textContent)).toEqual(["视频", "AI 编辑"]);
    expect(tabs[0]?.getAttribute("aria-selected")).toBe("true");

    await act(async () => tabs[1]?.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    expect(container.querySelector('[data-testid="ai-edit-tab"]')).not.toBeNull();

    await act(async () => root.unmount());
  });

  it("opens_video_and_ai_tabs_for_one_linked_av_selection", async () => {
    const video = visualClip({ id: "video", linkGroupId: "linked" });
    const audio = visualClip({
      id: "audio",
      mediaRef: "media-1",
      mediaType: "audio",
      linkGroupId: "linked",
    });
    const linkedTimeline = timelineWith(video);
    linkedTimeline.tracks.push({
      id: "audio-1",
      type: "audio",
      muted: false,
      hidden: false,
      syncLocked: true,
      clips: [audio],
    });
    useProjectStore.setState({ timeline: linkedTimeline });
    useEditorUiStore.setState({
      selectedClipIds: new Set([video.id, audio.id]),
      inspectorTab: "video",
    });

    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<Inspector />));

    expect(container.textContent).not.toContain("已选择 2 项");
    expect(
      [...container.querySelectorAll<HTMLElement>('[role="tab"]')].map((tab) => tab.textContent),
    ).toEqual(["视频", "AI 编辑"]);
    await act(async () => root.unmount());
  });

  it("creates edits and deletes a polygon mask through the undoable command route", async () => {
    const clip = visualClip();
    useProjectStore.setState({ timeline: timelineWith(clip), projectPath: "/tmp/demo.opentake" });
    useEditorUiStore.setState({ selectedClipIds: new Set([clip.id]), inspectorTab: "video" });
    const setMasks = vi.spyOn(edit, "setMasks").mockResolvedValue();
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<Inspector />));

    const maskSection = [...container.querySelectorAll("section")].find((section) =>
      section.textContent?.includes("蒙版"),
    );
    expect(maskSection).not.toBeUndefined();
    const enabled = maskSection?.querySelector<HTMLInputElement>('input[type="checkbox"]');
    await act(async () => enabled?.click());
    expect(setMasks).toHaveBeenLastCalledWith(
      [clip.id],
      [expect.objectContaining({ shape: expect.objectContaining({ kind: "circle" }) })],
    );

    const select = maskSection?.querySelector<HTMLSelectElement>("select");
    if (select) select.value = "poly";
    await act(async () => select?.dispatchEvent(new Event("change", { bubbles: true })));
    expect(setMasks).toHaveBeenLastCalledWith(
      [clip.id],
      [expect.objectContaining({ shape: expect.objectContaining({ kind: "poly" }) })],
    );
    expect(maskSection?.textContent).toContain("添加点");

    const deleteButton = [...(maskSection?.querySelectorAll("button") ?? [])].find(
      (button) => button.textContent === "删除蒙版",
    );
    await act(async () => deleteButton?.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    expect(setMasks).toHaveBeenLastCalledWith([clip.id], []);

    setMasks.mockRestore();
    await act(async () => root.unmount());
  });

  it("analyzes, displays, and resets stabilization through production actions", async () => {
    const clip = visualClip();
    useProjectStore.setState({ timeline: timelineWith(clip), projectPath: "/tmp/demo.opentake" });
    useEditorUiStore.setState({ selectedClipIds: new Set([clip.id]), inspectorTab: "video" });
    const solution = {
      model: "opentake.motion-smoothing",
      modelVersion: 1,
      sourceIdentity: clip.mediaRef,
      strength: 1,
      cropMargin: 0,
      keyframes: [
        { frame: 0, translationX: 0, translationY: 0, rotationDegrees: 0 },
        { frame: 89, translationX: 0.02, translationY: -0.01, rotationDegrees: 0 },
      ],
    };
    const analyze = vi
      .spyOn(edit, "analyzeAndApplyStabilization")
      .mockResolvedValue(solution);
    const cancel = vi.spyOn(edit, "cancelStabilizationAnalysis").mockResolvedValue(true);
    const reset = vi.spyOn(edit, "resetStabilization").mockResolvedValue();
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<Inspector />));

    const analyzeButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="stabilization-section"] button',
    );
    expect(analyzeButton).not.toBeNull();
    await act(async () => {
      analyzeButton?.click();
      await Promise.resolve();
    });
    expect(analyze).toHaveBeenCalledWith(clip.id);

    await act(async () => {
      useProjectStore.setState({
        timeline: timelineWith(visualClip({ stabilization: solution })),
      });
    });
    const stabilization = container.querySelector('[data-testid="stabilization-section"]');
    expect(stabilization?.textContent).toContain("opentake.motion-smoothing v1");
    expect(stabilization?.textContent).toContain("100%");
    let failReanalysis!: (reason: Error) => void;
    analyze.mockImplementationOnce(
      () =>
        new Promise((_resolve, reject) => {
          failReanalysis = reject;
        }),
    );
    const reanalyzeButton = [...(stabilization?.querySelectorAll("button") ?? [])].find(
      (button) => button.textContent === "重新分析",
    );
    await act(async () => {
      reanalyzeButton?.click();
      await Promise.resolve();
    });
    expect(reanalyzeButton?.disabled).toBe(true);
    const cancelButton = [...(stabilization?.querySelectorAll("button") ?? [])].find(
      (button) => button.textContent === "取消分析",
    );
    expect(cancelButton).not.toBeNull();
    await act(async () => cancelButton?.click());
    expect(cancel).toHaveBeenCalledOnce();
    await act(async () => failReanalysis(new Error("cancelled")));
    expect(stabilization?.querySelector('[role="alert"]')).toBeNull();
    const resetButton = [...(stabilization?.querySelectorAll("button") ?? [])].find(
      (button) => button.textContent === "重置防抖",
    );
    await act(async () => resetButton?.click());
    expect(reset).toHaveBeenCalledWith(clip.id);

    analyze.mockRestore();
    cancel.mockRestore();
    reset.mockRestore();
    await act(async () => root.unmount());
  });

  it("analyzes, reports progress, displays, and resets loudness normalization", async () => {
    const clip = visualClip();
    useProjectStore.setState({ timeline: timelineWith(clip), projectPath: "/tmp/demo.opentake" });
    useMediaStore.setState({
      items: [{ id: clip.mediaRef, name: "speech.wav", type: "video", duration: 3, hasAudio: true }],
      folders: [],
      importing: false,
      error: null,
    });
    useEditorUiStore.setState({ selectedClipIds: new Set([clip.id]), inspectorTab: "audio" });
    const normalization = {
      targetLufs: -16,
      truePeakCeilingDbtp: -1,
      inputIntegratedLufs: -23,
      inputTruePeakDbtp: -8,
      gainDb: 7,
      outputIntegratedLufs: -16,
      outputTruePeakDbtp: -1,
    };
    const listen = vi.spyOn(api, "onLoudnessProgress").mockImplementation(async (_id, handler) => {
      handler({ clipId: clip.id, done: 40, total: 100 });
      return () => {};
    });
    const analyze = vi.spyOn(edit, "analyzeAndApplyLoudness").mockResolvedValue(normalization);
    const reset = vi.spyOn(edit, "setLoudnessNormalization").mockResolvedValue();
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<Inspector />));

    const section = container.querySelector('[data-testid="loudness-section"]');
    const analyzeButton = [...(section?.querySelectorAll("button") ?? [])].find(
      (button) => button.textContent === "分析并应用",
    );
    await act(async () => {
      analyzeButton?.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(listen).toHaveBeenCalledWith(clip.id, expect.any(Function));
    expect(analyze).toHaveBeenCalledWith(clip.id, -16, -1);

    await act(async () => {
      useProjectStore.setState({
        timeline: timelineWith(visualClip({ loudnessNormalization: normalization })),
      });
    });
    const result = container.querySelector('[data-testid="loudness-result"]');
    expect(result?.textContent).toContain("-23.0 → -16.0 LUFS");
    expect(result?.textContent).toContain("+7.0 dB");
    const resetButton = [...(section?.querySelectorAll("button") ?? [])].find(
      (button) => button.textContent === "重置响度",
    );
    await act(async () => resetButton?.click());
    expect(reset).toHaveBeenCalledWith(clip.id, null);

    listen.mockRestore();
    analyze.mockRestore();
    reset.mockRestore();
    await act(async () => root.unmount());
  });

  it("adds reorders adjusts toggles and removes generic effects through undoable commands", async () => {
    const clip = visualClip({
      effects: [
        { name: "grayscale", params: {}, enabled: true },
        { name: "invert", params: { amount: 0.5 }, enabled: true },
      ],
    });
    useProjectStore.setState({ timeline: timelineWith(clip), projectPath: "/tmp/demo.opentake" });
    useEditorUiStore.setState({ selectedClipIds: new Set([clip.id]), inspectorTab: "video" });
    const setEffects = vi.spyOn(edit, "setEffects").mockResolvedValue();
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<Inspector />));

    const section = container.querySelector('[data-testid="generic-effects-section"]');
    expect(section).not.toBeNull();
    const add = [...(section?.querySelectorAll("button") ?? [])].find(
      (button) => button.textContent === "添加效果",
    );
    await act(async () => add?.click());
    expect(setEffects).toHaveBeenLastCalledWith(
      [clip.id],
      expect.arrayContaining([expect.objectContaining({ name: "grayscale" })]),
    );

    let items = [...(section?.querySelectorAll<HTMLElement>('[data-testid="generic-effect-item"]') ?? [])];
    const moveUp = items[2]?.querySelector<HTMLButtonElement>('button[aria-label="上移效果"]');
    await act(async () => moveUp?.click());
    expect(setEffects.mock.calls.at(-1)?.[1].map((effect) => effect.name)).toEqual([
      "grayscale",
      "grayscale",
      "invert",
    ]);

    items = [...(section?.querySelectorAll<HTMLElement>('[data-testid="generic-effect-item"]') ?? [])];
    const amount = items[0]?.querySelector<HTMLInputElement>('input[type="range"]');
    await act(async () => {
      if (amount) {
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(
          amount,
          "0.35",
        );
      }
      amount?.dispatchEvent(new InputEvent("input", { bubbles: true }));
    });
    expect(setEffects.mock.calls.at(-1)?.[1][0]?.params.amount).toBe(0.35);

    const enabled = items[0]?.querySelector<HTMLInputElement>('input[type="checkbox"]');
    await act(async () => enabled?.click());
    expect(setEffects.mock.calls.at(-1)?.[1][0]?.enabled).toBe(false);

    const remove = items[0]?.querySelector<HTMLButtonElement>('button[aria-label="删除效果"]');
    await act(async () => remove?.click());
    expect(setEffects.mock.calls.at(-1)?.[1]).toHaveLength(2);

    setEffects.mockRestore();
    await act(async () => root.unmount());
  });
});
