import { describe, expect, it, vi } from "vitest";
import type { Clip, ClipPropertiesReq } from "../../lib/types";
import { fadeInterpolationMenuItems } from "./ClipContextMenu";

function clip(overrides: Partial<Clip> = {}): Clip {
  return {
    id: "c1",
    mediaRef: "m1",
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
    fadeOutInterpolation: "smooth",
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
    ...overrides,
  };
}

describe("fadeInterpolationMenuItems", () => {
  it("marks the current left fade interpolation and writes fadeInInterpolation", () => {
    const apply = vi.fn<(props: ClipPropertiesReq) => void>();
    const items = fadeInterpolationMenuItems(clip({ fadeInInterpolation: "linear" }), "left", apply);

    expect(items.map((item) => ({ label: item.label, checked: item.checked }))).toEqual([
      { label: "Linear", checked: true },
      { label: "Smooth", checked: false },
    ]);

    items[1].action();

    expect(apply).toHaveBeenCalledWith({ fadeInInterpolation: "smooth" });
  });

  it("marks the current right fade interpolation and writes fadeOutInterpolation", () => {
    const apply = vi.fn<(props: ClipPropertiesReq) => void>();
    const items = fadeInterpolationMenuItems(clip({ fadeOutInterpolation: "smooth" }), "right", apply);

    expect(items.map((item) => ({ label: item.label, checked: item.checked }))).toEqual([
      { label: "Linear", checked: false },
      { label: "Smooth", checked: true },
    ]);

    items[0].action();

    expect(apply).toHaveBeenCalledWith({ fadeOutInterpolation: "linear" });
  });
});
