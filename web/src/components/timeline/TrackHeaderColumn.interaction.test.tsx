// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Timeline } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";

const editSpies = vi.hoisted(() => ({
  setTrackProps: vi.fn(),
  swapTracks: vi.fn(),
}));

vi.mock("../../store/editActions", () => editSpies);

import { TrackHeaderColumn } from "./TrackHeaderColumn";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const timeline: Timeline = {
  fps: 30,
  width: 1920,
  height: 1080,
  settingsConfigured: true,
  tracks: [
    { id: "audio-1", type: "audio", muted: false, hidden: false, syncLocked: true, clips: [] },
    { id: "video-1", type: "video", muted: false, hidden: false, syncLocked: false, clips: [] },
  ],
};

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.clearAllMocks();
  editSpies.setTrackProps.mockResolvedValue(undefined);
  editSpies.swapTracks.mockResolvedValue(undefined);
  useEditorUiStore.setState({ trackDisplayHeights: {}, toast: null });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  act(() => {
    root.render(<TrackHeaderColumn timeline={timeline} scrollTop={0} totalHeight={300} />);
  });
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
});

async function exerciseTrackAction(
  action: "mute" | "hide" | "sync-lock",
  trackIndex: number,
  expected: Record<string, boolean>,
): Promise<void> {
  const button = container.querySelector<HTMLButtonElement>(
    `[data-track-action='${action}'][data-track-index='${trackIndex}']`,
  );
  expect(button?.type).toBe("button");
  expect(button?.getAttribute("aria-pressed")).not.toBeNull();
  button?.focus();
  await act(async () => button?.click());
  expect(document.activeElement).toBe(button);
  expect(editSpies.setTrackProps).toHaveBeenCalledTimes(1);
  expect(editSpies.setTrackProps).toHaveBeenCalledWith(trackIndex, expected);
  expect(editSpies.swapTracks).not.toHaveBeenCalled();

  vi.clearAllMocks();
  editSpies.setTrackProps.mockRejectedValueOnce(new Error("locked"));
  await act(async () => {
    button?.click();
    await Promise.resolve();
  });
  expect(editSpies.setTrackProps).toHaveBeenCalledTimes(1);
  expect(useEditorUiStore.getState().toast?.message).toContain("locked");
}

it("control-4c72d4f81e47c57d mute audio track", async () => {
  await exerciseTrackAction("mute", 0, { muted: true });
});

it("control-74289f5806f8162a hide visual track", async () => {
  await exerciseTrackAction("hide", 1, { hidden: true });
});

it("control-71e7fa7fcd6aa730 toggle track sync lock", async () => {
  await exerciseTrackAction("sync-lock", 1, { syncLocked: true });
});
