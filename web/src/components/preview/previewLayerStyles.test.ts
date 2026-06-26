import { describe, expect, it } from "vitest";
import {
  aspectFitBox,
  timelinePreviewClipStyle,
  timelinePreviewCropMaskStyle,
  timelinePreviewCroppedMediaStyle,
  timelinePreviewCanvasStyle,
  timelinePreviewLayerStyle,
  timelinePreviewMediaStyle,
} from "./previewLayerStyles";
import type { Clip } from "../../lib/types";

function clip(over: Partial<Clip> = {}): Clip {
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
    fadeInInterpolation: "smooth",
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
    ...over,
  };
}

describe("timeline preview layer styles", () => {
  it("creates one aspect-fit timeline canvas for live and settled frames", () => {
    expect(timelinePreviewCanvasStyle(1920, 1080)).toMatchObject({
      position: "relative",
      aspectRatio: "1920 / 1080",
      width: "100%",
      maxWidth: "100%",
      maxHeight: "100%",
      overflow: "hidden",
    });
  });

  it("uses one absolute layer box for live playback and settled composite frames", () => {
    expect(timelinePreviewLayerStyle).toMatchObject({
      position: "absolute",
      inset: 0,
      width: "100%",
      height: "100%",
      pointerEvents: "none",
    });
  });

  it("fills the upstream transform box without a second object-fit pass", () => {
    expect(timelinePreviewMediaStyle).toMatchObject({
      position: "absolute",
      top: 0,
      left: 0,
      width: "100%",
      height: "100%",
      objectFit: "fill",
      maxWidth: "none",
      maxHeight: "none",
    });
  });

  it("fits the canvas by width when the preview stage is narrow", () => {
    expect(aspectFitBox(600, 500, 1920, 1080)).toEqual({
      width: 600,
      height: 337.5,
    });
  });

  it("fits the canvas by height when the preview stage is short", () => {
    expect(aspectFitBox(1000, 300, 1920, 1080)).toEqual({
      width: 533.3333333333333,
      height: 300,
    });
  });

  it("places and crops a clip inside the shared timeline canvas", () => {
    const c = clip({
      transform: {
        centerX: 0.5,
        centerY: 0.5,
        width: 0.5,
        height: 0.5,
        rotation: 15,
        flipHorizontal: false,
        flipVertical: false,
      },
      crop: { left: 0.25, top: 0, right: 0.25, bottom: 0 },
    });

    expect(timelinePreviewClipStyle(c, 0)).toMatchObject({
      left: "25%",
      top: "25%",
      width: "50%",
      height: "50%",
      transform: "rotate(15deg)",
    });
    expect(timelinePreviewCropMaskStyle(c, 0)).toMatchObject({
      left: "25%",
      top: "0%",
      width: "50%",
      height: "100%",
      overflow: "hidden",
    });
    expect(timelinePreviewCroppedMediaStyle(c, 0)).toMatchObject({
      objectFit: "fill",
      width: "200%",
      left: "-50%",
      top: "0%",
    });
  });

  it("maps a fitted vertical clip into its upstream transform box without re-containing it", () => {
    const c = clip({
      transform: {
        centerX: 0.5,
        centerY: 0.5,
        width: 0.31640625,
        height: 1,
        rotation: 0,
        flipHorizontal: false,
        flipVertical: false,
      },
    });

    expect(timelinePreviewClipStyle(c, 0)).toMatchObject({
      left: "34.1796875%",
      top: "0%",
      width: "31.640625%",
      height: "100%",
    });
    expect(timelinePreviewCropMaskStyle(c, 0)).toMatchObject({
      left: "0%",
      top: "0%",
      width: "100%",
      height: "100%",
    });
    expect(timelinePreviewCroppedMediaStyle(c, 0)).toMatchObject({
      width: "100%",
      height: "100%",
      objectFit: "fill",
    });
  });
});
