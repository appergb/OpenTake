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
let setPointerCapture: ReturnType<typeof vi.spyOn>;
let releasePointerCapture: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  vi.clearAllMocks();
  setPointerCapture = vi.spyOn(HTMLElement.prototype, "setPointerCapture")
    .mockImplementation(() => {});
  releasePointerCapture = vi.spyOn(HTMLElement.prototype, "releasePointerCapture")
    .mockImplementation(() => {});
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
  vi.restoreAllMocks();
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

it("control-9f9173ff2ee37464 resize track display height", async () => {
  const grip = container.querySelector<HTMLElement>("[data-track-resize='audio-1']")!;
  expect(grip.getAttribute("role")).toBe("separator");
  expect(grip.getAttribute("aria-orientation")).toBe("horizontal");
  expect(grip.getAttribute("aria-valuemin")).toBe("32");
  expect(grip.getAttribute("aria-valuemax")).toBe("200");
  expect(grip.getAttribute("aria-valuenow")).toBe("50");
  expect(grip.getAttribute("aria-label")).toContain("A1");
  expect(grip.tabIndex).toBe(0);

  await act(async () => {
    grip.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      pointerId: 9,
      clientY: 100,
    }));
    grip.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      pointerId: 9,
      clientY: 400,
    }));
  });
  expect(setPointerCapture).toHaveBeenCalledWith(9);
  expect(grip.dataset.interactionState).toBe("dragging");
  expect(useEditorUiStore.getState().trackDisplayHeights["audio-1"]).toBe(200);
  expect(grip.getAttribute("aria-valuenow")).toBe("200");

  await act(async () => {
    grip.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerId: 9 }));
  });
  expect(releasePointerCapture).toHaveBeenCalledWith(9);
  expect(grip.dataset.interactionState).toBe("enabled");

  await act(async () => {
    grip.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      pointerId: 10,
      clientY: 200,
    }));
    grip.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      pointerId: 10,
      clientY: -200,
    }));
    grip.dispatchEvent(new PointerEvent("pointercancel", { bubbles: true, pointerId: 10 }));
  });
  expect(useEditorUiStore.getState().trackDisplayHeights["audio-1"]).toBe(32);
  expect(releasePointerCapture).toHaveBeenCalledWith(10);
  expect(grip.dataset.interactionState).toBe("enabled");

  grip.focus();
  await act(async () => {
    grip.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));
  });
  expect(useEditorUiStore.getState().trackDisplayHeights["audio-1"]).toBe(200);
  await act(async () => {
    grip.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));
  });
  await act(async () => {
    grip.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
  });
  expect(useEditorUiStore.getState().trackDisplayHeights["audio-1"]).toBe(42);
  expect(document.activeElement).toBe(grip);
  expect(editSpies.setTrackProps).not.toHaveBeenCalled();
  expect(editSpies.swapTracks).not.toHaveBeenCalled();
});

it("control-db7fbb7edbcca44d dismiss track reorder menu", async () => {
  const row = container.querySelector<HTMLElement>("[data-track-row='audio-1']")!;
  row.focus();
  await act(async () => {
    row.dispatchEvent(new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: 80,
      clientY: 90,
    }));
  });
  expect(document.body.querySelector("[role='menu']")).not.toBeNull();
  expect(document.activeElement?.getAttribute("role")).toBe("menu");

  const backdrop = document.body.querySelector<HTMLElement>("[data-track-menu-backdrop]")!;
  await act(async () => {
    backdrop.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
  });
  expect(document.body.querySelector("[role='menu']")).toBeNull();
  expect(document.activeElement).toBe(row);

  await act(async () => {
    row.dispatchEvent(new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: 80,
      clientY: 90,
    }));
  });
  const contextBackdrop = document.body.querySelector<HTMLElement>("[data-track-menu-backdrop]")!;
  const contextEvent = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
  await act(async () => contextBackdrop.dispatchEvent(contextEvent));
  expect(contextEvent.defaultPrevented).toBe(true);
  expect(document.body.querySelector("[role='menu']")).toBeNull();
  expect(document.activeElement).toBe(row);

  await act(async () => {
    row.dispatchEvent(new KeyboardEvent("keydown", {
      key: "F10",
      shiftKey: true,
      bubbles: true,
    }));
  });
  const keyboardMenu = document.body.querySelector<HTMLElement>("[role='menu']")!;
  expect(keyboardMenu).not.toBeNull();
  await act(async () => {
    keyboardMenu.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  });
  expect(document.body.querySelector("[role='menu']")).toBeNull();
  expect(document.activeElement).toBe(row);
  expect(editSpies.setTrackProps).not.toHaveBeenCalled();
  expect(editSpies.swapTracks).not.toHaveBeenCalled();
});
