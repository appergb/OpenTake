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
  for (const spy of Object.values(editSpies)) spy.mockResolvedValue(undefined);
  useEditorUiStore.setState({ activeFrame: 100, toast: null });
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

async function renderOpacityKeyframe(): Promise<void> {
  await act(async () => {
    root.render(
      <KeyframesLaneRow
        clip={{
          ...clip,
          opacityTrack: {
            kind: "scalar",
            keyframes: [{ frame: 5, value: 0.5, interpolation: "linear" }],
          },
        }}
        property="opacity"
        t={(key, vars) => {
          if (key === "inspector.keyframes.diamondLabel") {
            return `${vars?.property} keyframe at frame ${vars?.frame}`;
          }
          return vars?.error ? `${key}:${vars.error}` : key;
        }}
      />,
    );
  });
  const lane = container.querySelector<HTMLElement>("[data-keyframe-lane]");
  lane!.getBoundingClientRect = () =>
    ({ left: 100, width: 200, right: 300, top: 0, bottom: 22, height: 22 }) as DOMRect;
}

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

it("control-4e0a20c7d0e54f3e keyframe diamond drag/context menu", async () => {
  await renderOpacityKeyframe();
  const diamond = container.querySelector<HTMLElement>("[data-keyframe-diamond='105']");
  expect(diamond).not.toBeNull();
  expect(diamond?.getAttribute("role")).toBe("button");
  expect(diamond?.tabIndex).toBe(0);
  expect(diamond?.getAttribute("aria-label")).toContain("105");

  await act(async () => {
    diamond?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, clientX: 150 }));
    window.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, clientX: 150 }));
  });
  expect(editSpies.moveKeyframe).not.toHaveBeenCalled();

  await act(async () => {
    diamond?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, clientX: 150 }));
    window.dispatchEvent(new MouseEvent("mousemove", { bubbles: true, clientX: 240 }));
    window.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, clientX: 240 }));
  });
  expect(editSpies.moveKeyframe).toHaveBeenCalledTimes(1);
  expect(editSpies.moveKeyframe).toHaveBeenLastCalledWith("clip-1", "opacity", 105, 114);

  vi.clearAllMocks();
  await act(async () => {
    diamond?.dispatchEvent(new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: 44,
      clientY: 55,
    }));
  });
  const menu = container.querySelector<HTMLElement>("[data-keyframe-context-menu]");
  expect(menu?.style.left).toBe("44px");
  expect(menu?.style.top).toBe("55px");
  expect(Object.values(editSpies).every((spy) => spy.mock.calls.length === 0)).toBe(true);

  await act(async () => {
    diamond?.focus();
    diamond?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
  });
  expect(document.activeElement).toBe(diamond);
  expect(editSpies.moveKeyframe).toHaveBeenCalledTimes(1);
  expect(editSpies.moveKeyframe).toHaveBeenLastCalledWith("clip-1", "opacity", 105, 106);

  editSpies.moveKeyframe.mockRejectedValueOnce(new Error("occupied"));
  await act(async () => {
    diamond?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true }));
    await Promise.resolve();
  });
  expect(useEditorUiStore.getState().toast?.message).toBe(
    "inspector.keyframes.moveFailed:occupied",
  );
});
