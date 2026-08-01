// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useI18nStore } from "../../i18n";
import type { Clip, GenerateMatteResult, MattingModelStatus } from "../../lib/types";
import { MattingSection, type MattingDependencies } from "./MattingSection";

function clip(overrides: Partial<Clip> = {}): Clip {
  return {
    id: "clip-1",
    mediaRef: "media-1",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 0,
    durationFrames: 2,
    trimStartFrame: 0,
    trimEndFrame: 2,
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

const installed: MattingModelStatus = {
  installed: true,
  model: "rvm-mobilenetv3-fp32-v1.0.0",
  bytes: 14_975_696,
  sha256: "88d4",
};

function result(applied: boolean): GenerateMatteResult {
  return {
    result: {
      clipId: "clip-1",
      sourceMediaRef: "media-1",
      assetId: applied ? "matte-1" : null,
      applied,
      cacheKey: "cache",
      previewPath: "/tmp/cache/matte.mov",
      frameCount: 2,
      width: 64,
      height: 64,
      fps: 5,
      model: installed.model,
      modelSha256: installed.sha256,
      sourceSha256: "source",
      startFrame: 0,
      endFrame: 2,
    },
    actionName: applied ? "Edit Motion Graphic" : null,
  };
}

describe("MattingSection", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useI18nStore.setState({ locale: "en" });
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it("previews_then_applies_and_undoes_the_same_cached_matte", async () => {
    const generate = vi
      .fn()
      .mockResolvedValueOnce(result(false))
      .mockResolvedValueOnce(result(true));
    const undo = vi.fn().mockResolvedValue(undefined);
    const dependencies: MattingDependencies = {
      status: vi.fn().mockResolvedValue(installed),
      install: vi.fn(),
      cancelInstall: vi.fn(),
      onProgress: vi.fn().mockResolvedValue(() => undefined),
      generate,
      cancelWorkflow: vi.fn(),
      undo,
    };
    await act(async () => root.render(<MattingSection clip={clip()} dependencies={dependencies} />));

    const button = (text: string) =>
      Array.from(container.querySelectorAll("button")).find((candidate) =>
        candidate.textContent?.includes(text),
      )!;
    await act(async () => button("Generate matte preview").click());
    expect(generate).toHaveBeenNthCalledWith(1, "clip-1", false);
    expect(button("Apply to clip").disabled).toBe(false);

    await act(async () => button("Apply to clip").click());
    expect(generate).toHaveBeenNthCalledWith(2, "clip-1", true);
    expect(container.textContent).toContain("original media remains");

    await act(async () => button("Undo matte").click());
    expect(undo).toHaveBeenCalledOnce();
    expect(container.textContent).toContain("Generate matte preview");
  });

  it("installs_with_progress_and_exposes_cancellation", async () => {
    let progress: ((value: { fraction: number; downloadedBytes: number; totalBytes: number }) => void) | undefined;
    let resolveInstall: ((status: MattingModelStatus) => void) | undefined;
    const install = vi.fn(
      () =>
        new Promise<MattingModelStatus>((resolve) => {
          resolveInstall = resolve;
        }),
    );
    const cancelInstall = vi.fn().mockResolvedValue(true);
    const dependencies: MattingDependencies = {
      status: vi.fn().mockResolvedValue({ ...installed, installed: false }),
      install,
      cancelInstall,
      onProgress: vi.fn(async (handler) => {
        progress = handler;
        return () => undefined;
      }),
      generate: vi.fn(),
      cancelWorkflow: vi.fn(),
      undo: vi.fn(),
    };
    await act(async () => root.render(<MattingSection clip={clip()} dependencies={dependencies} />));
    const button = (text: string) =>
      Array.from(container.querySelectorAll("button")).find((candidate) =>
        candidate.textContent?.includes(text),
      )!;
    await act(async () => button("Install local model").click());
    await act(async () => progress?.({ fraction: 0.5, downloadedBytes: 7, totalBytes: 14 }));
    expect(container.textContent).toContain("50%");
    await act(async () => button("Cancel").click());
    expect(cancelInstall).toHaveBeenCalledOnce();
    await act(async () => resolveInstall?.(installed));
  });

  it("refuses_reversed_or_retimed_clips_before_invoking_the_backend", async () => {
    const generate = vi.fn();
    const dependencies: MattingDependencies = {
      status: vi.fn().mockResolvedValue(installed),
      install: vi.fn(),
      cancelInstall: vi.fn(),
      onProgress: vi.fn().mockResolvedValue(() => undefined),
      generate,
      cancelWorkflow: vi.fn(),
      undo: vi.fn(),
    };
    await act(async () =>
      root.render(<MattingSection clip={clip({ reversed: true })} dependencies={dependencies} />),
    );
    expect(container.textContent).toContain("ordinary forward 1x video clips");
    expect(container.textContent).not.toContain("Generate matte preview");
    expect(generate).not.toHaveBeenCalled();
  });
});
