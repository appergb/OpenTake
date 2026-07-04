import { describe, expect, it, vi } from "vitest";
import type { Clip, ClipPropertiesReq } from "../../lib/types";
import { clipContextMenuItems, fadeInterpolationMenuItems } from "./ClipContextMenu";

const labels = {
  copy: "Copy",
  paste: "Paste",
  split: "Split",
  delete: "Delete",
  link: "Link",
  unlink: "Unlink",
  swapMedia: "Swap Media",
};

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

describe("clipContextMenuItems", () => {
  function actions(selectedClipIds: string[] = ["c1", "c2"]) {
    return {
      ensureSelected: vi.fn(),
      selectedClipIds: vi.fn(() => selectedClipIds),
      onCopy: vi.fn(),
      onPaste: vi.fn(),
      onSplit: vi.fn(),
      onDelete: vi.fn(),
      onLink: vi.fn(),
      onUnlink: vi.fn(),
      onSwapMedia: vi.fn(),
    };
  }

  it("shows paste only when the clipboard has content", () => {
    const withoutClipboard = clipContextMenuItems({
      clip: clip(),
      hasClipboardContent: false,
      labels,
      ...actions(),
    });
    const withClipboard = clipContextMenuItems({
      clip: clip(),
      hasClipboardContent: true,
      labels,
      ...actions(),
    });

    expect(withoutClipboard.map((item) => item.label)).not.toContain("Paste");
    expect(withClipboard.map((item) => item.label)).toContain("Paste");
  });

  it("wires copy, paste, link, and swap actions", () => {
    const a = actions(["c1", "c2"]);
    const items = clipContextMenuItems({
      clip: clip(),
      hasClipboardContent: true,
      labels,
      ...a,
    });

    items.find((item) => item.label === "Copy")?.action();
    items.find((item) => item.label === "Paste")?.action();
    items.find((item) => item.label === "Link")?.action();
    items.find((item) => item.label === "Swap Media")?.action();

    expect(a.ensureSelected).toHaveBeenCalledTimes(3);
    expect(a.onCopy).toHaveBeenCalledTimes(1);
    expect(a.onPaste).toHaveBeenCalledTimes(1);
    expect(a.onLink).toHaveBeenCalledWith(["c1", "c2"]);
    expect(a.onSwapMedia).toHaveBeenCalledTimes(1);
  });

  it("shows swap media only for video and image clips", () => {
    const videoItems = clipContextMenuItems({
      clip: clip({ mediaType: "video" }),
      hasClipboardContent: false,
      labels,
      ...actions(),
    });
    const imageItems = clipContextMenuItems({
      clip: clip({ mediaType: "image", sourceClipType: "image" }),
      hasClipboardContent: false,
      labels,
      ...actions(),
    });
    const audioItems = clipContextMenuItems({
      clip: clip({ mediaType: "audio", sourceClipType: "audio" }),
      hasClipboardContent: false,
      labels,
      ...actions(),
    });

    expect(videoItems.map((item) => item.label)).toContain("Swap Media");
    expect(imageItems.map((item) => item.label)).toContain("Swap Media");
    expect(audioItems.map((item) => item.label)).not.toContain("Swap Media");
  });
});
