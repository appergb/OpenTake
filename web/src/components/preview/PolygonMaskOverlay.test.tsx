// @vitest-environment happy-dom

import React from "react";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Clip } from "../../lib/types";

const setMasks = vi.hoisted(() => vi.fn());

vi.mock("../../store/editActions", () => ({ setMasks }));

import {
  inverseMaskDelta,
  PolygonMaskOverlay,
  transformMaskPoint,
} from "./PolygonMaskOverlay";

function clip(): Clip {
  return {
    id: "clip",
    mediaRef: "asset",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 0,
    durationFrames: 100,
    trimStartFrame: 0,
    trimEndFrame: 0,
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
    masks: [
      {
        shape: {
          kind: "poly",
          points: [
            { x: 0.2, y: 0.2 },
            { x: 0.8, y: 0.2 },
            { x: 0.5, y: 0.8 },
          ],
        },
        feather: 0.1,
        invert: false,
        transform: {
          offset: { x: 0.1, y: -0.05 },
          scale: { x: 2, y: 0.5 },
          rotationDegrees: 90,
        },
      },
    ],
  };
}

afterEach(() => {
  document.body.replaceChildren();
  setMasks.mockReset();
});

describe("PolygonMaskOverlay", () => {
  it("uses the renderer's center-relative mask transform", () => {
    const transform = clip().masks?.[0]?.transform;
    expect(transformMaskPoint({ x: 0.75, y: 0.5 }, transform)).toEqual({
      x: 0.6,
      y: 0.95,
    });
    const delta = inverseMaskDelta({ x: 0, y: 0.2 }, transform);
    expect(delta.x).toBeCloseTo(0.1);
    expect(delta.y).toBeCloseTo(0);
  });

  it("renders every polygon point and commits one undoable mask edit on release", async () => {
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(<PolygonMaskOverlay clip={clip()} canvasPx={{ width: 1000, height: 500 }} />),
    );

    expect(container.querySelector('[data-testid="polygon-mask-overlay"]')).not.toBeNull();
    expect(container.querySelectorAll("circle")).toHaveLength(3);

    const point = container.querySelector('[data-testid="polygon-mask-point-0"]');
    await act(async () => {
      point?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, clientX: 100, clientY: 100 }));
      window.dispatchEvent(new PointerEvent("pointermove", { clientX: 100, clientY: 200 }));
      window.dispatchEvent(new PointerEvent("pointerup"));
    });

    expect(setMasks).toHaveBeenCalledTimes(1);
    const committed = setMasks.mock.calls[0]?.[1]?.[0];
    expect(committed.shape.points[0].x).toBeCloseTo(0.3);
    expect(committed.shape.points[0].y).toBeCloseTo(0.2);
    expect(committed.feather).toBe(0.1);
    await act(async () => root.unmount());
  });
});
