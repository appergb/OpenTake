// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LAYOUT } from "../../lib/theme";
import type { Timeline } from "../../lib/types";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { TimelineContainer } from "./TimelineContainer";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const emptyTimeline: Timeline = {
  fps: 30,
  width: 1920,
  height: 1080,
  settingsConfigured: true,
  tracks: [],
};

let container: HTMLDivElement;
let root: Root;

function rulerX(frame: number): number {
  return LAYOUT.trackHeaderWidth + frame;
}

function pointerEvent(
  type: string,
  frame: number,
  options: { shiftKey?: boolean; pointerId?: number } = {},
): PointerEvent {
  return new PointerEvent(type, {
    bubbles: true,
    button: 0,
    buttons: type === "pointerup" || type === "pointercancel" ? 0 : 1,
    clientX: rulerX(frame),
    clientY: 10,
    pointerId: options.pointerId ?? 1,
    shiftKey: options.shiftKey ?? false,
  });
}

function dispatchCanvas(type: string, frame: number, options?: { shiftKey?: boolean }) {
  const canvas = container.querySelectorAll("canvas")[0];
  expect(canvas).toBeTruthy();
  act(() => {
    canvas.dispatchEvent(pointerEvent(type, frame, options));
  });
}

function setRange(startFrame: number, endFrame: number) {
  act(() => {
    useEditorUiStore.setState({ selectedTimelineRange: { startFrame, endFrame } });
  });
}

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", class {
    observe() {}
    disconnect() {}
  });
  vi.spyOn(HTMLElement.prototype, "setPointerCapture").mockImplementation(() => {});
  vi.spyOn(HTMLElement.prototype, "releasePointerCapture").mockImplementation(() => {});
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(() =>
    ({ left: 0, top: 0, width: 1000, height: 500, right: 1000, bottom: 500 }) as DOMRect,
  );
  useProjectStore.setState({
    timeline: emptyTimeline,
    projectEpoch: 1,
    compatibilityReadOnly: false,
  });
  useEditorUiStore.setState({
    zoomScale: 1,
    minZoomScale: 0.01,
    activeFrame: 100,
    isPlaying: false,
    isScrubbing: false,
    selectedClipIds: new Set(),
    selectedGap: null,
    selectedTimelineRange: null,
    trackDisplayHeights: {},
  });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  act(() => root.render(<TimelineContainer />));
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("TimelineContainer ruler range gestures", () => {
  it("keeps ordinary ruler scrub separate from range selection", () => {
    setRange(10, 30);
    dispatchCanvas("pointerdown", 50);
    dispatchCanvas("pointermove", 55);
    dispatchCanvas("pointerup", 55);

    expect(useEditorUiStore.getState().activeFrame).toBe(55);
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual({
      startFrame: 10,
      endFrame: 30,
    });
  });

  it("marks a range with Shift-drag and preserves an existing edge priority", () => {
    dispatchCanvas("pointerdown", 10, { shiftKey: true });
    dispatchCanvas("pointermove", 30, { shiftKey: true });
    dispatchCanvas("pointerup", 30, { shiftKey: true });
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual({
      startFrame: 10,
      endFrame: 30,
    });

    setRange(10, 30);
    dispatchCanvas("pointerdown", 10, { shiftKey: true });
    dispatchCanvas("pointermove", 5, { shiftKey: true });
    dispatchCanvas("pointerup", 5, { shiftKey: true });
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual({
      startFrame: 5,
      endFrame: 30,
    });

    dispatchCanvas("pointerdown", 40, { shiftKey: true });
    dispatchCanvas("pointermove", 10, { shiftKey: true });
    dispatchCanvas("pointerup", 10, { shiftKey: true });
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual({
      startFrame: 40,
      endFrame: 10,
    });

    dispatchCanvas("pointerdown", 20, { shiftKey: true });
    dispatchCanvas("pointermove", 20, { shiftKey: true });
    dispatchCanvas("pointerup", 20, { shiftKey: true });
    expect(useEditorUiStore.getState().selectedTimelineRange).toBeNull();

    dispatchCanvas("pointerdown", 92, { shiftKey: true });
    dispatchCanvas("pointermove", 120, { shiftKey: true });
    dispatchCanvas("pointerup", 120, { shiftKey: true });
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual({
      startFrame: 92,
      endFrame: 120,
    });
  });

  it("moves range edges, clears a collapsed range, and restores on cancel", () => {
    setRange(10, 30);
    dispatchCanvas("pointerdown", 10);
    dispatchCanvas("pointermove", 15);
    dispatchCanvas("pointerup", 15);
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual({
      startFrame: 15,
      endFrame: 30,
    });

    setRange(10, 30);
    dispatchCanvas("pointerdown", 10);
    dispatchCanvas("pointermove", 20);
    dispatchCanvas("pointercancel", 20);
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual({
      startFrame: 10,
      endFrame: 30,
    });

    dispatchCanvas("pointerdown", 10);
    dispatchCanvas("pointermove", 20);
    dispatchCanvas("lostpointercapture", 20);
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual({
      startFrame: 10,
      endFrame: 30,
    });

    setRange(10, 30);
    dispatchCanvas("pointerdown", 10);
    dispatchCanvas("pointermove", 30);
    dispatchCanvas("pointerup", 30);
    expect(useEditorUiStore.getState().selectedTimelineRange).toBeNull();

    setRange(10, 30);
    dispatchCanvas("pointerdown", 50, { shiftKey: true });
    dispatchCanvas("pointermove", 60, { shiftKey: true });
    dispatchCanvas("pointercancel", 60, { shiftKey: true });
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual({
      startFrame: 10,
      endFrame: 30,
    });

    dispatchCanvas("pointerdown", 50, { shiftKey: true });
    dispatchCanvas("pointermove", 60, { shiftKey: true });
    dispatchCanvas("lostpointercapture", 60, { shiftKey: true });
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual({
      startFrame: 10,
      endFrame: 30,
    });
  });
});
