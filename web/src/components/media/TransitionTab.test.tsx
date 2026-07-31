// @vitest-environment happy-dom

import React from "react";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Clip, Timeline } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { PanelShell } from "../ui/PanelShell";
import { resolveTransitionPair, TransitionTab } from "./TransitionTab";

function clip(id: string, startFrame: number, durationFrames: number): Clip {
  return {
    id,
    mediaRef: id,
    mediaType: "video",
    sourceClipType: "video",
    startFrame,
    durationFrames,
    trimStartFrame: 0,
    trimEndFrame: durationFrames,
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

function timeline(clips: Clip[]): Timeline {
  return {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks: [
      {
        id: "v1",
        name: "Video 1",
        type: "video",
        muted: false,
        hidden: false,
        syncLocked: true,
        clips,
      },
    ],
  };
}

afterEach(() => {
  document.body.replaceChildren();
  useEditorUiStore.setState({ selectedClipIds: new Set() });
  useProjectStore.getState().clearProjectSnapshot();
});

describe("TransitionTab", () => {
  it("resolves_a_cut_from_one_linked_video_and_audio_selection", () => {
    const a = clip("a", 0, 60);
    a.linkGroupId = "linked";
    const b = clip("b", 60, 60);
    const linkedAudio = clip("a-audio", 0, 60);
    linkedAudio.mediaType = "audio";
    linkedAudio.linkGroupId = "linked";
    const linkedTimeline = timeline([a, b]);
    linkedTimeline.tracks.push({
      id: "audio-1",
      type: "audio",
      muted: false,
      hidden: false,
      syncLocked: true,
      clips: [linkedAudio],
    });

    expect(
      resolveTransitionPair(linkedTimeline, new Set([a.id, linkedAudio.id]))?.from.id,
    ).toBe("a");
  });

  it("applies_and_removes_cross_dissolve_at_the_selected_cut", async () => {
    const a = clip("a", 0, 60);
    const b = clip("b", 60, 60);
    useProjectStore.setState({ timeline: timeline([a, b]) });
    useEditorUiStore.setState({ selectedClipIds: new Set([a.id]) });
    const apply = vi.fn().mockResolvedValue(undefined);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<TransitionTab onApply={apply} />));

    expect(container.textContent).toContain("a → b");
    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="apply-transition"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(apply).toHaveBeenCalledWith("a", "b", "crossDissolve", 15);

    useProjectStore.setState({
      timeline: timeline([
        { ...a, transitionOut: { toClipId: "b", kind: "crossDissolve", durationFrames: 15 } },
        b,
      ]),
    });
    await act(async () => undefined);
    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="remove-transition"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(apply).toHaveBeenLastCalledWith("a", "b", null, 15);
    await act(async () => root.unmount());
  });

  it("preserves_a_visible_failure_for_retry", async () => {
    const a = clip("a", 0, 60);
    const b = clip("b", 60, 60);
    useProjectStore.setState({ timeline: timeline([a, b]) });
    useEditorUiStore.setState({ selectedClipIds: new Set([a.id]) });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        <TransitionTab onApply={vi.fn().mockRejectedValue(new Error("cut changed"))} />,
      ),
    );

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="apply-transition"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("cut changed");
    expect(container.querySelector<HTMLButtonElement>('[data-action="apply-transition"]')?.disabled).toBe(
      false,
    );
    await act(async () => root.unmount());
  });

  it("keeps_the_timeline_cut_selected_while_editing_inside_the_media_panel", async () => {
    const a = clip("a", 0, 60);
    const b = clip("b", 60, 60);
    useProjectStore.setState({ timeline: timeline([a, b]) });
    useEditorUiStore.setState({ selectedClipIds: new Set([a.id]) });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        <PanelShell panel="media">
          <TransitionTab onApply={vi.fn().mockResolvedValue(undefined)} />
        </PanelShell>,
      ),
    );

    const apply = container.querySelector<HTMLButtonElement>('[data-action="apply-transition"]')!;
    await act(async () => {
      apply.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
      apply.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(useEditorUiStore.getState().selectedClipIds).toEqual(new Set([a.id]));
    await act(async () => root.unmount());
  });

  it("clears_stale_removal_feedback_when_global_undo_restores_the_transition", async () => {
    const a = clip("a", 0, 60);
    const b = clip("b", 60, 60);
    const withTransition = timeline([
      { ...a, transitionOut: { toClipId: "b", kind: "crossDissolve", durationFrames: 15 } },
      b,
    ]);
    useProjectStore.setState({ timeline: withTransition });
    useEditorUiStore.setState({ selectedClipIds: new Set([a.id]) });
    const apply = vi.fn(async () => {
      useProjectStore.setState({ timeline: timeline([a, b]) });
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<TransitionTab onApply={apply} />));

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="remove-transition"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(container.querySelector('[role="status"]')?.textContent).toContain("转场已移除");

    await act(async () => useProjectStore.setState({ timeline: withTransition }));
    expect(container.querySelector('[role="status"]')).toBeNull();
    expect(
      container.querySelector<HTMLButtonElement>('button[aria-pressed="true"]'),
    ).not.toBeNull();
    await act(async () => root.unmount());
  });
});
