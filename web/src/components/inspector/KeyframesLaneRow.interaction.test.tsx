// @vitest-environment happy-dom

import { act, StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Clip } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";

const editContext = vi.hoisted(() => ({
  expected: { projectEpoch: 7, projectPath: "/project.opentake", timelineVersion: 11 },
  sequenceId: "sequence-a",
}));
const editSpies = vi.hoisted(() => ({
  captureProjectEditContext: vi.fn(() => editContext),
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
  editSpies.captureProjectEditContext.mockReturnValue(editContext);
  useEditorUiStore.setState({ activeFrame: 100, toast: null });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  await act(async () => {
    root.render(
      <StrictMode>
        <KeyframesLaneRow
          clip={clip}
          property="opacity"
          t={(key) => key}
        />
      </StrictMode>,
    );
  });
});

async function renderOpacityKeyframe(): Promise<void> {
  await act(async () => {
    root.render(
      <StrictMode>
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
        />
      </StrictMode>,
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
  expect(diamond?.style.width).toBe("24px");
  expect(diamond?.style.height).toBe("24px");
  const diamondGlyph = diamond?.querySelector<HTMLElement>("[data-keyframe-diamond-glyph]");
  expect(diamondGlyph?.style.width).toBe("8px");
  expect(diamondGlyph?.style.height).toBe("8px");

  await act(async () => {
    diamond?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, clientX: 150 }));
    window.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, clientX: 150 }));
  });
  expect(editSpies.moveKeyframe).not.toHaveBeenCalled();

  await act(async () => {
    diamond?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, clientX: 150 }));
    window.dispatchEvent(new MouseEvent("mousemove", { bubbles: true, clientX: 230 }));
    window.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, clientX: 230 }));
  });
  expect(editSpies.moveKeyframe).toHaveBeenCalledTimes(1);
  expect(editSpies.moveKeyframe).toHaveBeenLastCalledWith(
    "clip-1",
    "opacity",
    105,
    113,
    editContext,
  );

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

it("keeps dragged keyframes inside the backend half-open clip range", async () => {
  await renderOpacityKeyframe();
  const diamond = container.querySelector<HTMLElement>("[data-keyframe-diamond='105']")!;

  await act(async () => {
    diamond.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, clientX: 150 }));
    window.dispatchEvent(new MouseEvent("mousemove", { bubbles: true, clientX: 300 }));
    window.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, clientX: 300 }));
  });

  expect(editSpies.moveKeyframe).toHaveBeenCalledWith(
    "clip-1",
    "opacity",
    105,
    119,
    editContext,
  );
});

it("disables lane stamping while the playhead is outside the clip", async () => {
  await act(async () => {
    useEditorUiStore.setState({ activeFrame: 99 });
    root.render(
      <StrictMode>
        <KeyframesLaneRow clip={clip} property="opacity" t={(key) => key} />
      </StrictMode>,
    );
  });

  const stamp = container.querySelector<HTMLButtonElement>(
    "[data-keyframe-stamp]",
  );
  expect(stamp?.disabled).toBe(true);
  await act(async () => stamp?.click());
  expect(editSpies.stampKeyframe).not.toHaveBeenCalled();
});

it("normalizes a fractional playback frame before stamping", async () => {
  await act(async () => useEditorUiStore.setState({ activeFrame: 100.4 }));
  const stamp = container.querySelector<HTMLButtonElement>("[data-keyframe-stamp]")!;

  await act(async () => stamp.click());

  expect(editSpies.stampKeyframe).toHaveBeenCalledOnce();
  expect(editSpies.stampKeyframe).toHaveBeenCalledWith("clip-1", "opacity", 100);
});

it("control-6e36c47f93f0d4fb dismiss keyframe context menu", async () => {
  await renderOpacityKeyframe();
  const diamond = container.querySelector<HTMLElement>("[data-keyframe-diamond='105']")!;

  diamond.focus();
  await act(async () => {
    diamond.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  });
  expect(container.querySelector("[role='menu']")).not.toBeNull();
  expect(document.activeElement?.getAttribute("role")).toBe("menu");

  const backdrop = container.querySelector<HTMLElement>("[data-keyframe-menu-backdrop]")!;
  await act(async () => backdrop.dispatchEvent(new MouseEvent("click", { bubbles: true })));
  expect(container.querySelector("[role='menu']")).toBeNull();
  expect(document.activeElement).toBe(diamond);
  expect(Object.values(editSpies).every((spy) => spy.mock.calls.length === 0)).toBe(true);

  await act(async () => {
    diamond.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  });
  const secondBackdrop = container.querySelector<HTMLElement>("[data-keyframe-menu-backdrop]")!;
  const dismissContextMenu = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
  await act(async () => {
    expect(secondBackdrop.dispatchEvent(dismissContextMenu)).toBe(false);
  });
  expect(container.querySelector("[role='menu']")).toBeNull();
  expect(document.activeElement).toBe(diamond);

  await act(async () => {
    diamond.dispatchEvent(new KeyboardEvent("keydown", { key: "ContextMenu", bubbles: true }));
  });
  const menu = container.querySelector<HTMLElement>("[role='menu']")!;
  await act(async () => {
    menu.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  });
  expect(container.querySelector("[role='menu']")).toBeNull();
  expect(document.activeElement).toBe(diamond);
  expect(Object.values(editSpies).every((spy) => spy.mock.calls.length === 0)).toBe(true);
});

it("control-c191a17716450b1a delete keyframe", async () => {
  await renderOpacityKeyframe();
  const diamond = container.querySelector<HTMLElement>("[data-keyframe-diamond='105']")!;
  diamond.focus();
  await act(async () => {
    diamond.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  });

  const deleteItem = container.querySelector<HTMLButtonElement>(
    "[data-keyframe-menu-action='delete']",
  );
  expect(deleteItem?.getAttribute("role")).toBe("menuitem");
  expect(deleteItem?.type).toBe("button");
  expect(deleteItem?.style.minHeight).toBe("24px");
  await act(async () => deleteItem?.click());
  expect(editSpies.removeKeyframe).toHaveBeenCalledTimes(1);
  expect(editSpies.removeKeyframe).toHaveBeenCalledWith("clip-1", "opacity", 105);
  expect(editSpies.moveKeyframe).not.toHaveBeenCalled();
  expect(editSpies.setKeyframeInterpolation).not.toHaveBeenCalled();
  expect(container.querySelector("[role='menu']")).toBeNull();
  expect(document.activeElement).toBe(diamond);

  vi.clearAllMocks();
  editSpies.removeKeyframe.mockRejectedValueOnce(new Error("locked"));
  await act(async () => {
    diamond.dispatchEvent(new KeyboardEvent("keydown", { key: "ContextMenu", bubbles: true }));
  });
  const retryDelete = container.querySelector<HTMLButtonElement>(
    "[data-keyframe-menu-action='delete']",
  )!;
  await act(async () => {
    retryDelete.click();
    await Promise.resolve();
  });
  expect(editSpies.removeKeyframe).toHaveBeenCalledTimes(1);
  expect(container.querySelector("[role='menu']")).toBeNull();
  expect(document.activeElement).toBe(diamond);
  expect(useEditorUiStore.getState().toast?.message).toBe(
    "inspector.keyframes.deleteFailed:locked",
  );
});

async function exerciseInterpolation(
  interpolation: "linear" | "hold" | "smooth",
): Promise<void> {
  await renderOpacityKeyframe();
  const diamond = container.querySelector<HTMLElement>("[data-keyframe-diamond='105']")!;
  diamond.focus();
  await act(async () => {
    diamond.dispatchEvent(new KeyboardEvent("keydown", { key: "ContextMenu", bubbles: true }));
  });
  const item = container.querySelector<HTMLButtonElement>(
    `[data-keyframe-menu-action='${interpolation}']`,
  )!;
  expect(item.getAttribute("role")).toBe("menuitem");
  await act(async () => item.click());
  expect(editSpies.setKeyframeInterpolation).toHaveBeenCalledTimes(1);
  expect(editSpies.setKeyframeInterpolation).toHaveBeenCalledWith(
    "clip-1",
    "opacity",
    105,
    interpolation,
  );
  expect(editSpies.moveKeyframe).not.toHaveBeenCalled();
  expect(editSpies.removeKeyframe).not.toHaveBeenCalled();
  expect(container.querySelector("[role='menu']")).toBeNull();
  expect(document.activeElement).toBe(diamond);

  vi.clearAllMocks();
  editSpies.setKeyframeInterpolation.mockRejectedValueOnce(new Error("locked"));
  await act(async () => {
    diamond.dispatchEvent(new KeyboardEvent("keydown", { key: "ContextMenu", bubbles: true }));
  });
  const rejectedItem = container.querySelector<HTMLButtonElement>(
    `[data-keyframe-menu-action='${interpolation}']`,
  )!;
  await act(async () => {
    rejectedItem.click();
    await Promise.resolve();
  });
  expect(editSpies.setKeyframeInterpolation).toHaveBeenCalledTimes(1);
  expect(container.querySelector("[role='menu']")).toBeNull();
  expect(document.activeElement).toBe(diamond);
  expect(useEditorUiStore.getState().toast?.message).toBe(
    "inspector.keyframes.interpolationFailed:locked",
  );
}

it("control-3b4230aba22c9422 linear keyframe interpolation", async () => {
  await exerciseInterpolation("linear");
});

it("control-16737eebbe9cb784 hold keyframe interpolation", async () => {
  await exerciseInterpolation("hold");
});

it("control-ab879256f29c4e0a smooth keyframe interpolation", async () => {
  await exerciseInterpolation("smooth");
});
