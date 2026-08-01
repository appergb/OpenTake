// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Clip } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";

const editSpies = vi.hoisted(() => ({
  moveKeyframe: vi.fn(),
  removeKeyframe: vi.fn(),
  setKeyframeInterpolation: vi.fn(),
  setKeyframes: vi.fn(),
  stampKeyframe: vi.fn(),
}));

vi.mock("../../store/editActions", () => editSpies);

import { KeyframesLaneRow } from "./KeyframesLaneRow";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const clip: Clip = {
  id: "clip-1",
  mediaRef: "media-1",
  mediaType: "video",
  sourceClipType: "video",
  startFrame: 100,
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
  transform: { position: { x: 0, y: 0 }, scale: { x: 1, y: 1 }, rotationDegrees: 0 },
  crop: { top: 0, right: 0, bottom: 0, left: 0 },
  opacityTrack: { kind: "scalar", keyframes: [] },
};

let container: HTMLDivElement;
let root: Root;

beforeEach(async () => {
  vi.clearAllMocks();
  useEditorUiStore.setState({ activeFrame: 100 });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  await act(async () => {
    root.render(
      <KeyframesLaneRow
        clip={clip}
        property="opacity"
        t={(key) => key}
      />,
    );
  });
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
});

it("control-75a9964d0b81961a keyframe lane seek", async () => {
  const lane = container.querySelector<HTMLElement>("[data-keyframe-lane]");
  expect(lane).not.toBeNull();
  expect(lane?.getAttribute("role")).toBe("slider");
  expect(lane?.tabIndex).toBe(0);
  expect(lane?.getAttribute("aria-label")).toBe("inspector.keyframes.property.opacity");

  lane!.getBoundingClientRect = () =>
    ({ left: 100, width: 200, right: 300, top: 0, bottom: 22, height: 22 }) as DOMRect;

  await act(async () => {
    lane?.dispatchEvent(new MouseEvent("click", { bubbles: true, clientX: 200 }));
  });
  expect(useEditorUiStore.getState().activeFrame).toBe(110);

  const child = lane?.firstElementChild;
  await act(async () => {
    child?.dispatchEvent(new MouseEvent("click", { bubbles: true, clientX: 260 }));
  });
  expect(useEditorUiStore.getState().activeFrame).toBe(110);

  const contextMenu = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
  expect(lane?.dispatchEvent(contextMenu)).toBe(false);
  expect(useEditorUiStore.getState().activeFrame).toBe(110);
  expect(Object.values(editSpies).every((spy) => spy.mock.calls.length === 0)).toBe(true);

  lane?.focus();
  await act(async () => {
    lane?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
  });
  expect(document.activeElement).toBe(lane);
  expect(useEditorUiStore.getState().activeFrame).toBe(111);
  expect(lane?.getAttribute("aria-valuenow")).toBe("111");
});
