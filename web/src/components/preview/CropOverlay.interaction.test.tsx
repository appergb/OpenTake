// @vitest-environment happy-dom

import { act, StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Clip } from "../../lib/types";

const uiStore = vi.hoisted(() => ({
  activeFrame: 60,
  cropAspectLock: "free" as const,
  pushToast: vi.fn(),
}));
const editContext = vi.hoisted(() => ({
  expected: { projectEpoch: 7, projectPath: "/project.opentake", timelineVersion: 11 },
  sequenceId: "sequence-a",
}));
const editSpies = vi.hoisted(() => ({
  captureProjectEditContext: vi.fn(() => editContext),
  upsertKeyframe: vi.fn(),
  setClipProperties: vi.fn(),
}));

vi.mock("../../store/uiStore", () => ({
  useEditorUiStore: Object.assign(
    (selector: (state: typeof uiStore) => unknown) => selector(uiStore),
    { getState: () => uiStore },
  ),
}));
vi.mock("../../store/editActions", () => editSpies);

import { CropOverlay } from "./CropOverlay";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

function animatedClip(overrides: Partial<Clip> = {}): Clip {
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
      width: 0.8,
      height: 0.8,
      rotation: 0,
      flipHorizontal: false,
      flipVertical: false,
    },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    cropTrack: {
      keyframes: [{
        frame: 10,
        value: { left: 0.1, top: 0.1, right: 0.1, bottom: 0.1 },
        interpolationOut: "hold",
      }],
    },
    ...overrides,
  };
}

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.clearAllMocks();
  editSpies.upsertKeyframe.mockResolvedValue(undefined);
  editSpies.setClipProperties.mockResolvedValue(undefined);
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

async function renderOverlay(overrides: Partial<Clip> = {}): Promise<HTMLElement> {
  await act(async () => {
    root.render(
      <StrictMode>
        <CropOverlay
          clip={animatedClip(overrides)}
          canvasPx={{ width: 1000, height: 1000 }}
          sourcePixelAspect={null}
        />
      </StrictMode>,
    );
  });
  return container.querySelector<HTMLElement>("[data-crop-pan-surface]")!;
}

it("samples and commits one animated crop keyframe at the floored playback frame", async () => {
  uiStore.activeFrame = 60.6;
  const surface = await renderOverlay({
    cropTrack: {
      keyframes: [
        {
          frame: 10,
          value: { left: 0.1, top: 0.1, right: 0.1, bottom: 0.1 },
          interpolationOut: "linear",
        },
        {
          frame: 11,
          value: { left: 0.2, top: 0.2, right: 0.2, bottom: 0.2 },
          interpolationOut: "hold",
        },
      ],
    },
  });
  await act(async () => {
    surface.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      cancelable: true,
      clientX: 100,
      clientY: 100,
    }));
    window.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      clientX: 140,
      clientY: 120,
    }));
    window.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
  });

  expect(editSpies.upsertKeyframe).toHaveBeenCalledTimes(1);
  expect(editSpies.upsertKeyframe).toHaveBeenCalledWith(
    "clip",
    "crop",
    60,
    expect.objectContaining({ kind: "crop" }),
    editContext,
  );
  const sampledValue = editSpies.upsertKeyframe.mock.calls[0][3].value;
  expect(sampledValue.left).toBeCloseTo(0.15);
  expect(sampledValue.top).toBeCloseTo(0.125);
  expect(sampledValue.right).toBeCloseTo(0.05);
  expect(sampledValue.bottom).toBeCloseTo(0.075);
  expect(editSpies.captureProjectEditContext).toHaveBeenCalledTimes(1);
  expect(editSpies.setClipProperties).not.toHaveBeenCalled();
});

it("blocks animated crop writes while the playhead is outside the clip", async () => {
  uiStore.activeFrame = 150;
  const surface = await renderOverlay();
  expect(surface.getAttribute("aria-disabled")).toBe("true");
  expect(surface.style.pointerEvents).toBe("none");

  await act(async () => {
    surface.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      cancelable: true,
      clientX: 100,
      clientY: 100,
    }));
    window.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      clientX: 140,
      clientY: 120,
    }));
    window.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
  });

  expect(editSpies.upsertKeyframe).not.toHaveBeenCalled();
  expect(editSpies.setClipProperties).not.toHaveBeenCalled();
});

it("exposes 24px keyboard-operable pan and resize controls", async () => {
  const surface = await renderOverlay();
  const controls = [...container.querySelectorAll<HTMLButtonElement>(
    'button[aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight"]',
  )];
  expect(surface).toBe(controls[0]);
  expect(controls).toHaveLength(5);
  expect(controls.every((control) => control.getAttribute("aria-label"))).toBe(true);
  expect(controls.slice(1).every((control) => control.style.width === "24px")).toBe(true);
  expect(controls.slice(1).every((control) => control.style.height === "24px")).toBe(true);

  await act(async () => {
    surface.dispatchEvent(new KeyboardEvent("keydown", {
      key: "ArrowRight",
      shiftKey: true,
      bubbles: true,
    }));
    surface.dispatchEvent(new KeyboardEvent("keyup", {
      key: "ArrowRight",
      shiftKey: true,
      bubbles: true,
    }));
  });
  expect(editSpies.upsertKeyframe).toHaveBeenCalledOnce();
  expect(editSpies.upsertKeyframe).toHaveBeenCalledWith(
    "clip",
    "crop",
    60,
    expect.objectContaining({ kind: "crop" }),
    editContext,
  );
});

it("accumulates repeated crop arrow nudges and commits once on keyup", async () => {
  const surface = await renderOverlay();
  await act(async () => {
    for (let index = 0; index < 3; index += 1) {
      surface.dispatchEvent(new KeyboardEvent("keydown", {
        key: "ArrowRight",
        bubbles: true,
      }));
    }
  });
  expect(editSpies.upsertKeyframe).not.toHaveBeenCalled();

  await act(async () => {
    surface.dispatchEvent(new KeyboardEvent("keyup", {
      key: "ArrowRight",
      bubbles: true,
    }));
  });
  expect(editSpies.upsertKeyframe).toHaveBeenCalledOnce();
  expect(editSpies.upsertKeyframe).toHaveBeenCalledWith(
    "clip",
    "crop",
    60,
    expect.objectContaining({ kind: "crop" }),
    editContext,
  );
  const accumulatedValue = editSpies.upsertKeyframe.mock.calls[0][3].value;
  expect(accumulatedValue.left).toBeCloseTo(0.10375);
  expect(accumulatedValue.top).toBeCloseTo(0.1);
  expect(accumulatedValue.right).toBeCloseTo(0.09625);
  expect(accumulatedValue.bottom).toBeCloseTo(0.1);
  expect(editSpies.captureProjectEditContext).toHaveBeenCalledOnce();
});

it("keeps the visible last fractional crop frame editable", async () => {
  uiStore.activeFrame = 149.6;
  const surface = await renderOverlay();
  expect((surface as HTMLButtonElement).disabled).toBe(false);
  await act(async () => {
    surface.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    surface.dispatchEvent(new KeyboardEvent("keyup", { key: "ArrowRight", bubbles: true }));
  });
  expect(editSpies.upsertKeyframe).toHaveBeenCalledWith(
    "clip",
    "crop",
    149,
    expect.objectContaining({ kind: "crop" }),
    editContext,
  );
});

it("does not start a second keyboard crop transaction during a pointer drag", async () => {
  const surface = await renderOverlay();
  await act(async () => {
    surface.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      cancelable: true,
      clientX: 100,
      clientY: 100,
    }));
    window.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      clientX: 140,
      clientY: 100,
    }));
    surface.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    surface.dispatchEvent(new KeyboardEvent("keyup", { key: "ArrowRight", bubbles: true }));
    window.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
  });
  expect(editSpies.upsertKeyframe).toHaveBeenCalledOnce();
  const value = editSpies.upsertKeyframe.mock.calls[0][3].value;
  expect(value.left).toBeCloseTo(0.15);
  expect(value.right).toBeCloseTo(0.05);
  expect(editSpies.captureProjectEditContext).toHaveBeenCalledOnce();
});
