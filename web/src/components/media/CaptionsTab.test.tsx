// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CaptionTranslationResult, Clip } from "../../lib/types";
import { useI18nStore } from "../../i18n";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";

const mocks = vi.hoisted(() => ({
  translate: vi.fn(),
  apply: vi.fn(),
  cancel: vi.fn(),
  undo: vi.fn(),
}));

vi.mock("../../lib/api", () => ({
  isTauri: true,
  applyCaptionTranslationReview: mocks.apply,
  cancelAdvancedWorkflow: mocks.cancel,
  downloadTranscribeModel: vi.fn(),
  onTranscribeProgress: vi.fn(),
  translateCaptions: mocks.translate,
  transcribeModelStatus: vi.fn(),
}));

vi.mock("../../store/editActions", () => ({
  generateCaptions: vi.fn(),
  undo: mocks.undo,
}));

import { CaptionsTab } from "./CaptionsTab";

function caption(id: string, text: string, startFrame: number): Clip {
  return {
    id,
    mediaRef: "",
    mediaType: "text",
    sourceClipType: "text",
    startFrame,
    durationFrames: 12,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform: { centerX: 0.5, centerY: 0.85, width: 1, height: 0.1, rotation: 0, flipHorizontal: false, flipVertical: false },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    captionGroupId: "group-1",
    textContent: text,
  };
}

function translationResult(): CaptionTranslationResult {
  return {
    result: {
      projectEpoch: 7,
      version: 9,
      sourceLocale: "en-US",
      targetLocale: "zh-CN",
      provider: "openai",
      model: "gpt-4o-mini",
      review: [
        { id: "cap-1", sourceText: "Hello", translatedText: "你好" },
        { id: "cap-2", sourceText: "World", translatedText: "世界" },
      ],
      errors: [],
      captionCount: 2,
      translatedCount: 2,
      applied: false,
    },
    actionName: null,
  };
}

describe("CaptionsTab translation review", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useI18nStore.setState({ locale: "en" });
    useProjectStore.getState().replaceProjectSnapshot({
      projectEpoch: 7,
      projectPath: "/tmp/Captions.opentake",
      compatibilityReadOnly: false,
      compatibilityBlockers: [],
      version: 9,
      timeline: {
        fps: 30,
        width: 1920,
        height: 1080,
        settingsConfigured: true,
        tracks: [{ id: "captions", type: "video", muted: false, hidden: false, syncLocked: true, clips: [caption("cap-1", "Hello", 4), caption("cap-2", "World", 21)] }],
      },
    });
    useEditorUiStore.setState({ selectedClipIds: new Set(["cap-1", "cap-2"]) });
    mocks.translate.mockReset().mockResolvedValue(translationResult());
    mocks.apply.mockReset().mockResolvedValue({ result: { applied: true }, actionName: "Translate Captions" });
    mocks.cancel.mockReset().mockResolvedValue(true);
    mocks.undo.mockReset().mockResolvedValue(undefined);
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it("reviews individual changes, rejects one, applies the accepted item, and undoes", async () => {
    await act(async () => root.render(<CaptionsTab />));
    const consent = Array.from(container.querySelectorAll("input[type=checkbox]")).find((input) => input.parentElement?.textContent?.includes("possible API charges")) as HTMLInputElement;
    await act(async () => consent.click());
    const button = (label: string) => Array.from(container.querySelectorAll("button")).find((candidate) => candidate.textContent?.includes(label))!;
    await act(async () => button("Translate & Review").click());
    expect(mocks.translate).toHaveBeenCalledWith(["cap-1", "cap-2"], "auto", "zh-CN", "openai", true);
    expect(container.textContent).toContain("你好");
    expect(container.textContent).toContain("世界");

    const reviewChecks = Array.from(container.querySelectorAll('input[aria-label^="Accept translation"]')) as HTMLInputElement[];
    await act(async () => reviewChecks[1].click());
    await act(async () => button("Apply 1").click());
    expect(mocks.apply).toHaveBeenCalledWith(
      translationResult().result,
      [{ id: "cap-1", sourceText: "Hello", translatedText: "你好" }],
    );
    expect(container.textContent).toContain("caption IDs and frame ranges are unchanged");
    await act(async () => button("Undo Translation").click());
    expect(mocks.undo).toHaveBeenCalledOnce();
  });
});
