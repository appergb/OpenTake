// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useI18nStore } from "../../i18n";
import type { Clip, RemoveObjectResult } from "../../lib/types";
import {
  ObjectRemovalSection,
  type ObjectRemovalDependencies,
} from "./ObjectRemovalSection";

function clip(overrides: Partial<Clip> = {}): Clip {
  return {
    id: "clip-1",
    mediaRef: "media-1",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 10,
    durationFrames: 20,
    trimStartFrame: 0,
    trimEndFrame: 20,
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
    masks: [
      {
        shape: {
          kind: "circle",
          center: { x: 0.5, y: 0.5 },
          radius: { x: 0.2, y: 0.2 },
        },
        feather: 0.01,
        invert: false,
      },
    ],
    ...overrides,
  };
}

function result(applied: boolean): RemoveObjectResult {
  return {
    result: {
      clipId: "clip-1",
      sourceMediaRef: "media-1",
      assetId: applied ? "removed-1" : null,
      applied,
      cacheKey: "cache",
      previewPath: "/tmp/cache/removed.mov",
      frameCount: 20,
      width: 64,
      height: 48,
      fps: 10,
      provider: "opentake-local",
      model: "opentake-boundary-fill-v1",
      sourceSha256: "source",
      maskIndex: 0,
      startFrame: 10,
      endFrame: 30,
    },
    actionName: applied ? "Remove Masked Object" : null,
  };
}

describe("ObjectRemovalSection", () => {
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

  it("retries_then_applies_and_undoes_the_reviewed_derivative", async () => {
    const generate = vi
      .fn()
      .mockRejectedValueOnce(new Error("fixture failure"))
      .mockResolvedValueOnce(result(false))
      .mockResolvedValueOnce(result(true));
    const undo = vi.fn().mockResolvedValue(undefined);
    const dependencies: ObjectRemovalDependencies = {
      generate,
      cancel: vi.fn(),
      undo,
    };
    await act(async () =>
      root.render(<ObjectRemovalSection clip={clip()} dependencies={dependencies} />),
    );
    const button = (text: string) =>
      Array.from(container.querySelectorAll("button")).find((candidate) =>
        candidate.textContent?.includes(text),
      )!;

    await act(async () => button("Generate removal preview").click());
    expect(container.textContent).toContain("fixture failure");
    await act(async () => button("Retry removal preview").click());
    expect(generate).toHaveBeenNthCalledWith(2, "clip-1", false, {
      startFrame: 10,
      endFrame: 30,
    });
    expect(button("Apply to clip").disabled).toBe(false);

    await act(async () => button("Apply to clip").click());
    expect(generate).toHaveBeenNthCalledWith(3, "clip-1", true, {
      startFrame: 10,
      endFrame: 30,
    });
    expect(container.textContent).toContain("undo restores");
    await act(async () => button("Undo object removal").click());
    expect(undo).toHaveBeenCalledOnce();
    expect(container.textContent).toContain("Generate removal preview");
  });

  it("exposes_cancellation_without_accepting_a_stale_completion", async () => {
    let resolve: ((value: RemoveObjectResult) => void) | undefined;
    const generate = vi.fn(
      () => new Promise<RemoveObjectResult>((done) => { resolve = done; }),
    );
    const cancel = vi.fn().mockResolvedValue(true);
    const dependencies: ObjectRemovalDependencies = {
      generate,
      cancel,
      undo: vi.fn(),
    };
    await act(async () =>
      root.render(<ObjectRemovalSection clip={clip()} dependencies={dependencies} />),
    );
    const button = (text: string) =>
      Array.from(container.querySelectorAll("button")).find((candidate) =>
        candidate.textContent?.includes(text),
      )!;
    await act(async () => button("Generate removal preview").click());
    await act(async () => button("Cancel").click());
    expect(cancel).toHaveBeenCalledOnce();
    await act(async () => resolve?.(result(false)));
    expect(container.querySelector("video")).toBeNull();
  });

  it("keeps_the_undo_surface_when_apply_clears_the_baked_mask_before_returning", async () => {
    let resolveApply: ((value: RemoveObjectResult) => void) | undefined;
    const generate = vi
      .fn()
      .mockResolvedValueOnce(result(false))
      .mockImplementationOnce(
        () => new Promise<RemoveObjectResult>((done) => { resolveApply = done; }),
      );
    const dependencies: ObjectRemovalDependencies = {
      generate,
      cancel: vi.fn(),
      undo: vi.fn(),
    };
    await act(async () =>
      root.render(<ObjectRemovalSection clip={clip()} dependencies={dependencies} />),
    );
    const button = (text: string) =>
      Array.from(container.querySelectorAll("button")).find((candidate) =>
        candidate.textContent?.includes(text),
      )!;
    await act(async () => button("Generate removal preview").click());
    await act(async () => button("Apply to clip").click());
    await act(async () =>
      root.render(
        <ObjectRemovalSection
          clip={clip({ mediaRef: "removed-1", masks: [] })}
          dependencies={dependencies}
        />,
      ),
    );
    await act(async () => resolveApply?.(result(true)));
    expect(button("Undo object removal")).toBeDefined();
  });

  it("requires_an_editable_mask_and_refuses_incompatible_clips", async () => {
    const generate = vi.fn();
    const dependencies: ObjectRemovalDependencies = {
      generate,
      cancel: vi.fn(),
      undo: vi.fn(),
    };
    await act(async () =>
      root.render(
        <ObjectRemovalSection clip={clip({ masks: [] })} dependencies={dependencies} />,
      ),
    );
    expect(container.textContent).toContain("Enable and adjust a mask");
    expect(container.textContent).not.toContain("Generate removal preview");

    await act(async () =>
      root.render(
        <ObjectRemovalSection clip={clip({ reversed: true })} dependencies={dependencies} />,
      ),
    );
    expect(container.textContent).toContain("ordinary forward 1x video clips");
    expect(generate).not.toHaveBeenCalled();
  });
});
