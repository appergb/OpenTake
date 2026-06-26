import type { CSSProperties } from "react";
import { cropAt, opacityAt, rotationAt, sizeAt, topLeftAt } from "../../lib/clip";
import type { Clip } from "../../lib/types";

export function timelinePreviewCanvasStyle(width: number, height: number): CSSProperties {
  return {
    position: "relative",
    aspectRatio: `${width} / ${height}`,
    width: "100%",
    maxWidth: "100%",
    maxHeight: "100%",
    background: "var(--bg-preview-canvas)",
    overflow: "hidden",
  };
}

export function aspectFitBox(
  containerWidth: number,
  containerHeight: number,
  contentWidth: number,
  contentHeight: number,
): { width: number; height: number } | null {
  if (containerWidth <= 0 || containerHeight <= 0 || contentWidth <= 0 || contentHeight <= 0) {
    return null;
  }
  const contentAspect = contentWidth / contentHeight;
  const containerAspect = containerWidth / containerHeight;
  if (containerAspect > contentAspect) {
    const height = containerHeight;
    return { width: height * contentAspect, height };
  }
  const width = containerWidth;
  return { width, height: width / contentAspect };
}

export const timelinePreviewLayerStyle: CSSProperties = {
  position: "absolute",
  inset: 0,
  pointerEvents: "none",
  width: "100%",
  height: "100%",
  overflow: "hidden",
};

export const timelinePreviewMediaStyle: CSSProperties = {
  position: "absolute",
  top: 0,
  left: 0,
  width: "100%",
  height: "100%",
  objectFit: "fill",
  objectPosition: "center center",
  display: "block",
  maxWidth: "none",
  maxHeight: "none",
};

export function timelinePreviewClipStyle(clip: Clip, frame: number): CSSProperties {
  const topLeft = topLeftAt(clip, frame);
  const [width, height] = sizeAt(clip, frame);
  const transforms = [`rotate(${rotationAt(clip, frame)}deg)`];
  if (clip.transform.flipHorizontal) transforms.push("scaleX(-1)");
  if (clip.transform.flipVertical) transforms.push("scaleY(-1)");
  return {
    position: "absolute",
    left: `${topLeft.x * 100}%`,
    top: `${topLeft.y * 100}%`,
    width: `${width * 100}%`,
    height: `${height * 100}%`,
    overflow: "hidden",
    opacity: opacityAt(clip, frame),
    transform: transforms.join(" "),
    transformOrigin: "center center",
  };
}

function cropFractions(clip: Clip, frame: number) {
  const crop = cropAt(clip, frame);
  const visibleWidth = Math.max(0.0001, 1 - crop.left - crop.right);
  const visibleHeight = Math.max(0.0001, 1 - crop.top - crop.bottom);
  return { crop, visibleWidth, visibleHeight };
}

export function timelinePreviewCropMaskStyle(clip: Clip, frame: number): CSSProperties {
  const { crop, visibleWidth, visibleHeight } = cropFractions(clip, frame);
  return {
    position: "absolute",
    left: `${crop.left * 100}%`,
    top: `${crop.top * 100}%`,
    width: `${visibleWidth * 100}%`,
    height: `${visibleHeight * 100}%`,
    overflow: "hidden",
  };
}

export function timelinePreviewCroppedMediaStyle(clip: Clip, frame: number): CSSProperties {
  const { crop, visibleWidth, visibleHeight } = cropFractions(clip, frame);
  return {
    ...timelinePreviewMediaStyle,
    width: `${100 / visibleWidth}%`,
    height: `${100 / visibleHeight}%`,
    left: `${(-crop.left / visibleWidth) * 100}%`,
    top: `${(-crop.top / visibleHeight) * 100}%`,
  };
}
