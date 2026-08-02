// @vitest-environment happy-dom

import React from "react";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Clip } from "../../lib/types";
import { AiEditTab, type AiEditProposal } from "./AiEditTab";

function clip(): Clip {
  return {
    id: "clip-1",
    mediaRef: "media-1",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 0,
    durationFrames: 120,
    trimStartFrame: 0,
    trimEndFrame: 120,
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
  };
}

afterEach(() => document.body.replaceChildren());

describe("AiEditTab", () => {
  it("generates_reviews_applies_rejects_and_undoes_one_command", async () => {
    const proposal: AiEditProposal = {
      id: "gentle-fade",
      title: "柔和淡入淡出",
      explanation: "在片段两端添加 8 帧淡化。",
      properties: { fadeInFrames: 8, fadeOutFrames: 8 },
    };
    const suggest = vi.fn().mockResolvedValue([proposal]);
    const apply = vi.fn().mockResolvedValue(undefined);
    const undo = vi.fn().mockResolvedValue(undefined);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        <AiEditTab clip={clip()} fps={30} suggest={suggest} onApply={apply} onUndo={undo} />,
      ),
    );

    const input = container.querySelector<HTMLTextAreaElement>("textarea")!;
    await act(async () => {
      input.value = "让开头和结尾更柔和";
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="generate"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(suggest).toHaveBeenCalledOnce();
    expect(container.textContent).toContain("柔和淡入淡出");

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="reject"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(apply).not.toHaveBeenCalled();

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="generate"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="apply"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(apply).toHaveBeenCalledOnce();
    expect(apply).toHaveBeenCalledWith("clip-1", proposal.properties);

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="undo"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(undo).toHaveBeenCalledOnce();
    await act(async () => root.unmount());
  });

  it("surfaces_generation_failure_without_mutating_the_clip", async () => {
    const suggest = vi.fn().mockRejectedValue(new Error("suggestion engine unavailable"));
    const apply = vi.fn();
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(<AiEditTab clip={clip()} fps={30} suggest={suggest} onApply={apply} />),
    );

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="generate"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(container.querySelector('[role="alert"]')?.textContent).toContain(
      "suggestion engine unavailable",
    );
    expect(apply).not.toHaveBeenCalled();
    await act(async () => root.unmount());
  });

  it("cancels_an_inflight_suggestion_without_mutating_the_clip", async () => {
    let requestSignal: AbortSignal | null = null;
    const suggest = vi.fn(
      (_clip: Clip, _intent: string, _fps: number, signal: AbortSignal) => {
        requestSignal = signal;
        return new Promise<AiEditProposal[]>(() => undefined);
      },
    );
    const apply = vi.fn();
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(<AiEditTab clip={clip()} fps={30} suggest={suggest} onApply={apply} />),
    );

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="generate"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(container.querySelector('[data-action="cancel"]')).not.toBeNull();

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="cancel"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(requestSignal?.aborted).toBe(true);
    expect(container.querySelector('[data-action="cancel"]')).toBeNull();
    expect(apply).not.toHaveBeenCalled();
    await act(async () => root.unmount());
  });
});
