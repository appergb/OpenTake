// @vitest-environment happy-dom

import { act, StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Clip } from "../../lib/types";

const uiStore = vi.hoisted(() => ({ activeFrame: 60, pushToast: vi.fn() }));
const editContext = vi.hoisted(() => ({
  expected: { projectEpoch: 7, projectPath: "/project.opentake", timelineVersion: 11 },
  sequenceId: "sequence-a",
}));
const editSpies = vi.hoisted(() => ({
  captureProjectEditContext: vi.fn(() => editContext),
  setClipProperties: vi.fn(),
  setTransformAtFrame: vi.fn(),
}));

vi.mock("../../store/uiStore", () => ({
  useEditorUiStore: Object.assign(
    (selector: (state: typeof uiStore) => unknown) => selector(uiStore),
    { getState: () => uiStore },
  ),
}));

vi.mock("../../store/editActions", () => editSpies);

import { TransformOverlay } from "./TransformOverlay";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

function clip(over: Partial<Clip> = {}): Clip {
  return {
    id: "clip",
    mediaRef: "asset",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 50,
    durationFrames: 100,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "smooth",
    fadeOutInterpolation: "smooth",
    opacity: 1,
    transform: {
      centerX: 0.5,
      centerY: 0.5,
      width: 0.4,
      height: 0.3,
      rotation: 0,
      flipHorizontal: false,
      flipVertical: false,
    },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    positionTrack: {
      keyframes: [{ frame: 10, value: { a: 0.3, b: 0.35 }, interpolationOut: "hold" }],
    },
    ...over,
  };
}

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.clearAllMocks();
  editSpies.setTransformAtFrame.mockResolvedValue(undefined);
  uiStore.activeFrame = 60;
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.useRealTimers();
});

it("commits an animated canvas drag through one atomic transform-at-frame command", async () => {
  await act(async () => {
    root.render(
      <StrictMode>
        <TransformOverlay
          clip={clip()}
          canvasPx={{ width: 1000, height: 1000 }}
          mediaAspect={null}
        />
      </StrictMode>,
    );
  });

  const surface = container.querySelector<HTMLElement>("[data-transform-move-surface]")!;
  await act(async () => {
    surface.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      cancelable: true,
      clientX: 100,
      clientY: 100,
    }));
    window.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      clientX: 150,
      clientY: 120,
    }));
    window.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
  });

  expect(editSpies.setTransformAtFrame).toHaveBeenCalledTimes(1);
  expect(editSpies.setTransformAtFrame).toHaveBeenCalledWith(
    "clip",
    60,
    expect.objectContaining({ centerX: 0.55, centerY: 0.52 }),
    editContext,
  );
  expect(editSpies.captureProjectEditContext).toHaveBeenCalledTimes(1);
  expect(editSpies.setClipProperties).not.toHaveBeenCalled();
});

it("does not start an animated canvas edit outside the clip range", async () => {
  uiStore.activeFrame = 150;
  await act(async () => {
    root.render(
      <StrictMode>
        <TransformOverlay
          clip={clip()}
          canvasPx={{ width: 1000, height: 1000 }}
          mediaAspect={null}
        />
      </StrictMode>,
    );
  });

  const surface = container.querySelector<HTMLElement>("[data-transform-move-surface]")!;
  expect(surface.getAttribute("aria-disabled")).toBe("true");
  await act(async () => {
    surface.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      cancelable: true,
      clientX: 100,
      clientY: 100,
    }));
    window.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      clientX: 150,
      clientY: 120,
    }));
    window.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
  });

  expect(editSpies.setTransformAtFrame).not.toHaveBeenCalled();
  expect(editSpies.setClipProperties).not.toHaveBeenCalled();
});

it("exposes 24px keyboard-operable move and resize controls", async () => {
  await act(async () => {
    root.render(
      <StrictMode>
        <TransformOverlay
          clip={clip()}
          canvasPx={{ width: 1000, height: 1000 }}
          mediaAspect={null}
        />
      </StrictMode>,
    );
  });

  const controls = [...container.querySelectorAll<HTMLButtonElement>(
    'button[aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight"]',
  )];
  expect(controls).toHaveLength(5);
  expect(controls.every((control) => control.getAttribute("aria-label"))).toBe(true);
  expect(controls.slice(1).every((control) => control.style.width === "24px")).toBe(true);
  expect(controls.slice(1).every((control) => control.style.height === "24px")).toBe(true);

  await act(async () => {
    controls[0].dispatchEvent(new KeyboardEvent("keydown", {
      key: "ArrowRight",
      shiftKey: true,
      bubbles: true,
    }));
    controls[0].dispatchEvent(new KeyboardEvent("keyup", {
      key: "ArrowRight",
      shiftKey: true,
      bubbles: true,
    }));
  });
  expect(editSpies.setTransformAtFrame).toHaveBeenCalledOnce();
  expect(editSpies.setTransformAtFrame).toHaveBeenCalledWith(
    "clip",
    60,
    expect.objectContaining({ centerX: 0.51 }),
    editContext,
  );
});

it("accumulates repeated unsnapped arrow nudges and commits one gesture", async () => {
  await act(async () => {
    root.render(
      <StrictMode>
        <TransformOverlay
          clip={clip()}
          canvasPx={{ width: 1000, height: 1000 }}
          mediaAspect={null}
        />
      </StrictMode>,
    );
  });

  const surface = container.querySelector<HTMLButtonElement>("[data-transform-move-surface]")!;
  await act(async () => {
    for (let index = 0; index < 3; index += 1) {
      surface.dispatchEvent(new KeyboardEvent("keydown", {
        key: "ArrowRight",
        bubbles: true,
      }));
    }
  });
  expect(editSpies.setTransformAtFrame).not.toHaveBeenCalled();

  await act(async () => {
    surface.dispatchEvent(new KeyboardEvent("keyup", {
      key: "ArrowRight",
      bubbles: true,
    }));
  });
  expect(editSpies.setTransformAtFrame).toHaveBeenCalledOnce();
  expect(editSpies.setTransformAtFrame).toHaveBeenCalledWith(
    "clip",
    60,
    expect.objectContaining({ centerX: 0.503 }),
    editContext,
  );
  expect(editSpies.captureProjectEditContext).toHaveBeenCalledOnce();
});

it("samples, validates, and writes the same floored playback frame", async () => {
  uiStore.activeFrame = 60.6;
  await act(async () => {
    root.render(
      <StrictMode>
        <TransformOverlay
          clip={clip({
            positionTrack: {
              keyframes: [
                { frame: 10, value: { a: 0.3, b: 0.35 }, interpolationOut: "linear" },
                { frame: 11, value: { a: 0.7, b: 0.75 }, interpolationOut: "hold" },
              ],
            },
          })}
          canvasPx={{ width: 1000, height: 1000 }}
          mediaAspect={null}
        />
      </StrictMode>,
    );
  });

  const overlay = container.querySelector<HTMLElement>("[data-testid=transform-overlay]")!;
  expect(overlay.style.left).toBe("500px");
  const surface = container.querySelector<HTMLButtonElement>("[data-transform-move-surface]")!;
  await act(async () => {
    surface.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    surface.dispatchEvent(new KeyboardEvent("keyup", { key: "ArrowRight", bubbles: true }));
  });
  expect(editSpies.setTransformAtFrame).toHaveBeenCalledWith(
    "clip",
    60,
    expect.objectContaining({ centerX: 0.501 }),
    editContext,
  );
});

it("keeps the visible last fractional frame editable", async () => {
  uiStore.activeFrame = 149.6;
  await act(async () => {
    root.render(
      <StrictMode>
        <TransformOverlay
          clip={clip()}
          canvasPx={{ width: 1000, height: 1000 }}
          mediaAspect={null}
        />
      </StrictMode>,
    );
  });

  const surface = container.querySelector<HTMLButtonElement>("[data-transform-move-surface]")!;
  expect(surface.disabled).toBe(false);
  await act(async () => {
    surface.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    surface.dispatchEvent(new KeyboardEvent("keyup", { key: "ArrowRight", bubbles: true }));
  });
  expect(editSpies.setTransformAtFrame).toHaveBeenCalledWith(
    "clip",
    149,
    expect.any(Object),
    editContext,
  );
});

it("does not split a held arrow gesture before OS key repeat starts", async () => {
  vi.useFakeTimers();
  await act(async () => {
    root.render(
      <StrictMode>
        <TransformOverlay
          clip={clip()}
          canvasPx={{ width: 1000, height: 1000 }}
          mediaAspect={null}
        />
      </StrictMode>,
    );
  });
  const surface = container.querySelector<HTMLButtonElement>("[data-transform-move-surface]")!;
  await act(async () => {
    surface.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    vi.advanceTimersByTime(300);
  });
  expect(editSpies.setTransformAtFrame).not.toHaveBeenCalled();
  await act(async () => {
    surface.dispatchEvent(new KeyboardEvent("keyup", { key: "ArrowRight", bubbles: true }));
  });
  expect(editSpies.setTransformAtFrame).toHaveBeenCalledOnce();
  vi.useRealTimers();
});

it("coalesces a pending keyboard nudge into one following pointer drag", async () => {
  vi.useFakeTimers();
  await act(async () => {
    root.render(
      <StrictMode>
        <TransformOverlay
          clip={clip()}
          canvasPx={{ width: 1000, height: 1000 }}
          mediaAspect={null}
        />
      </StrictMode>,
    );
  });
  const surface = container.querySelector<HTMLButtonElement>("[data-transform-move-surface]")!;
  await act(async () => {
    surface.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    surface.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      cancelable: true,
      clientX: 100,
      clientY: 100,
    }));
    window.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      clientX: 110,
      clientY: 100,
    }));
    window.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
    vi.advanceTimersByTime(300);
  });
  expect(editSpies.setTransformAtFrame).toHaveBeenCalledOnce();
  expect(editSpies.setTransformAtFrame).toHaveBeenCalledWith(
    "clip",
    60,
    expect.objectContaining({ centerX: 0.511 }),
    editContext,
  );
  expect(editSpies.captureProjectEditContext).toHaveBeenCalledOnce();
  vi.useRealTimers();
});
