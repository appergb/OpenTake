/**
 * Pure crop-inset geometry for the on-canvas Crop overlay (T3-11). 1:1 port of
 * upstream `CropOverlayView.swift`'s drag math — the corner/pan handlers stay
 * pure functions here (unlike upstream's `@State dragStart` + view methods)
 * so they're independently testable, mirroring how `clip.ts`'s
 * `resizeTransformFromCorner`/`moveTransformByDelta` already extracted
 * `TransformOverlayView.swift`'s math into pure functions.
 *
 * Coordinate space: a `Crop` is 4 normalized (0–1) edge insets of the SOURCE
 * rect (`left`/`top`/`right`/`bottom`), exactly as stored on `Clip.crop` —
 * see `types.ts`'s `Crop` interface and `Models/Timeline.swift:501-510`. All
 * functions here take/return that same `Crop` shape; screen-pixel deltas are
 * converted to normalized fractions via the caller-supplied `clipRect`
 * (the clip's on-canvas box in pixels, i.e. `TransformOverlay`'s
 * `sampledTransform(clip, frame) * canvasPx`), matching upstream's
 * `clipRect.width`/`clipRect.height` divisors (CropOverlayView.swift:103-104,
 * 135-136, 152-153).
 */

import type { Crop } from "./types";

/** `CropOverlayView.Corner` (CropOverlayView.swift:271-273). */
export type CropResizeCorner = "topLeft" | "topRight" | "bottomLeft" | "bottomRight";

/** Minimum visible-fraction floor for unlocked resize (CropOverlayView.swift:130,
 *  `let minVis = 0.05`). */
const MIN_VISIBLE = 0.05;

function visibleWidthFraction(c: Crop): number {
  return Math.max(0, 1 - c.left - c.right);
}
function visibleHeightFraction(c: Crop): number {
  return Math.max(0, 1 - c.top - c.bottom);
}

/**
 * Pan: shift the crop window by a clip-local pixel delta, preserving its size,
 * clamped so it never leaves the source rect. 1:1 port of `pannedCrop`
 * (CropOverlayView.swift:101-110).
 */
export function pannedCrop(
  start: Crop,
  translationPx: { width: number; height: number },
  clipRect: { width: number; height: number },
): Crop {
  if (clipRect.width <= 0 || clipRect.height <= 0) return start;
  const dx = translationPx.width / clipRect.width;
  const dy = translationPx.height / clipRect.height;
  const visW = 1 - start.left - start.right;
  const visH = 1 - start.top - start.bottom;
  const left = Math.max(0, Math.min(start.left + dx, 1 - visW));
  const top = Math.max(0, Math.min(start.top + dy, 1 - visH));
  return { left, top, right: 1 - visW - left, bottom: 1 - visH - top };
}

/**
 * Free (unlocked) corner resize: moves the dragged corner's two edges, clamped
 * to a `MIN_VISIBLE` floor. 1:1 port of the non-aspect-locked branch of
 * `resizedCrop` (CropOverlayView.swift:135-148).
 */
function resizedCropFree(
  start: Crop,
  corner: CropResizeCorner,
  translationPx: { width: number; height: number },
  clipRect: { width: number; height: number },
): Crop {
  const dx = translationPx.width / clipRect.width;
  const dy = translationPx.height / clipRect.height;
  let { left, top, right, bottom } = start;
  switch (corner) {
    case "topLeft":
      left += dx;
      top += dy;
      break;
    case "topRight":
      right -= dx;
      top += dy;
      break;
    case "bottomLeft":
      left += dx;
      bottom -= dy;
      break;
    case "bottomRight":
      right -= dx;
      bottom -= dy;
      break;
  }
  left = Math.max(0, Math.min(left, 1 - MIN_VISIBLE - right));
  right = Math.max(0, Math.min(right, 1 - MIN_VISIBLE - left));
  top = Math.max(0, Math.min(top, 1 - MIN_VISIBLE - bottom));
  bottom = Math.max(0, Math.min(bottom, 1 - MIN_VISIBLE - top));
  return { left, top, right, bottom };
}

/**
 * Aspect-locked corner resize: drives a single visible-width variable `s` from
 * whichever axis the user dragged further (in width-equivalent units), clamps
 * `s` so both the horizontal and vertical bounds are respected, then derives
 * the opposite-edge insets to hold the target pixel aspect. 1:1 port of
 * `resizedCropLocked` (CropOverlayView.swift:151-199).
 *
 * `aspectN` is the crop-target aspect NORMALIZED against the source's own
 * pixel aspect (target / sourcePixelAspect — see `lockedAspectFromPixelAspect`
 * below), matching upstream's `aspectN` parameter naming exactly.
 */
function resizedCropLocked(
  start: Crop,
  corner: CropResizeCorner,
  translationPx: { width: number; height: number },
  clipRect: { width: number; height: number },
  aspectN: number,
): Crop {
  const dx = translationPx.width / clipRect.width;
  const dy = translationPx.height / clipRect.height;
  const { left: L, top: T, right: R, bottom: B } = start;
  const startVisW = 1 - L - R;
  const startVisH = 1 - T - B;

  let widthDelta: number;
  let heightDelta: number;
  switch (corner) {
    case "topLeft":
      widthDelta = -dx;
      heightDelta = -dy;
      break;
    case "topRight":
      widthDelta = dx;
      heightDelta = -dy;
      break;
    case "bottomLeft":
      widthDelta = -dx;
      heightDelta = dy;
      break;
    case "bottomRight":
      widthDelta = dx;
      heightDelta = dy;
      break;
  }

  const sFromW = startVisW + widthDelta;
  const sFromH = aspectN * (startVisH + heightDelta);
  let s = Math.abs(widthDelta) > Math.abs(heightDelta * aspectN) ? sFromW : sFromH;

  const sMaxFromX = corner === "topRight" || corner === "bottomRight" ? 1 - L : 1 - R;
  const sMaxFromY =
    corner === "bottomLeft" || corner === "bottomRight" ? aspectN * (1 - T) : aspectN * (1 - B);
  const sMax = Math.min(sMaxFromX, sMaxFromY);
  const sMin = Math.max(MIN_VISIBLE, MIN_VISIBLE * aspectN);
  if (sMax < sMin) return start;
  s = Math.min(Math.max(s, sMin), sMax);

  const newVisW = s;
  const newVisH = s / aspectN;
  let newL = L;
  let newT = T;
  let newR = R;
  let newB = B;
  switch (corner) {
    case "topLeft":
      newL = 1 - R - newVisW;
      newT = 1 - B - newVisH;
      break;
    case "topRight":
      newR = 1 - L - newVisW;
      newT = 1 - B - newVisH;
      break;
    case "bottomLeft":
      newL = 1 - R - newVisW;
      newB = 1 - T - newVisH;
      break;
    case "bottomRight":
      newR = 1 - L - newVisW;
      newB = 1 - T - newVisH;
      break;
  }
  return { left: newL, top: newT, right: newR, bottom: newB };
}

/**
 * Corner resize dispatcher: free when `aspectN` is `null`, aspect-locked
 * otherwise. 1:1 port of `resizedCrop`'s branch (CropOverlayView.swift:128-133).
 */
export function resizedCrop(
  start: Crop,
  corner: CropResizeCorner,
  translationPx: { width: number; height: number },
  clipRect: { width: number; height: number },
  aspectN: number | null,
): Crop {
  if (clipRect.width <= 0 || clipRect.height <= 0) return start;
  if (aspectN !== null) {
    return resizedCropLocked(start, corner, translationPx, clipRect, aspectN);
  }
  return resizedCropFree(start, corner, translationPx, clipRect);
}

/**
 * The crop-target aspect normalized against the source's own pixel aspect —
 * 1:1 port of `lockedAspectNormalized(for:)` (CropOverlayView.swift:201-205):
 * `target / srcAspect`, or `null` when either is unavailable (free/original
 * presets, whose `CropAspectLock.pixelAspect` is `nil` — see `CropAspectLock`
 * in `Models/Timeline.swift:513-540` — or when source pixel dimensions are
 * unknown). Returns `null` (not clamped) so callers fall back to the free
 * resize branch, exactly like upstream's `guard let` early-return.
 */
export function lockedAspectNormalized(
  targetPixelAspect: number | null,
  sourcePixelAspect: number | null,
): number | null {
  if (targetPixelAspect === null || sourcePixelAspect === null || sourcePixelAspect <= 0) {
    return null;
  }
  return targetPixelAspect / sourcePixelAspect;
}

/** `CropAspectLock` (Models/Timeline.swift:513-540): the preset menu's exact
 *  case set (order matches upstream's `CaseIterable` synthesis) with each
 *  preset's target PIXEL aspect (width/height), or `null` for free/original
 *  (both apply no aspect constraint going forward). */
export type CropAspectLock =
  | "free"
  | "original"
  | "r16x9"
  | "r9x16"
  | "r1x1"
  | "r4x3"
  | "r3x4"
  | "r21x9";

export const CROP_ASPECT_LOCKS: CropAspectLock[] = [
  "free",
  "original",
  "r16x9",
  "r9x16",
  "r1x1",
  "r4x3",
  "r3x4",
  "r21x9",
];

/** `CropAspectLock.pixelAspect` (Models/Timeline.swift:529-539). */
export function cropAspectLockPixelAspect(preset: CropAspectLock): number | null {
  switch (preset) {
    case "free":
    case "original":
      return null;
    case "r16x9":
      return 16 / 9;
    case "r9x16":
      return 9 / 16;
    case "r1x1":
      return 1;
    case "r4x3":
      return 4 / 3;
    case "r3x4":
      return 3 / 4;
    case "r21x9":
      return 21 / 9;
  }
}

const IDENTITY_CROP: Crop = { left: 0, top: 0, right: 0, bottom: 0 };

/**
 * Largest centered crop of `targetPixelAspect` inside a `sourcePixelAspect`
 * source. 1:1 port of `EditorViewModel.cropFittingAspect(for:targetPixelAspect:)`
 * (EditorViewModel.swift:459-474). Returns the identity crop when the source
 * is unavailable, non-positive, or already (near-)matches the target — the
 * `abs(sourceAspect - target) < 0.0001` upstream tolerance is preserved as-is.
 */
export function cropFittingAspect(
  sourcePixelAspect: number | null,
  targetPixelAspect: number,
): Crop {
  if (sourcePixelAspect === null || sourcePixelAspect <= 0 || targetPixelAspect <= 0) {
    return IDENTITY_CROP;
  }
  if (Math.abs(sourcePixelAspect - targetPixelAspect) < 0.0001) return IDENTITY_CROP;
  if (sourcePixelAspect > targetPixelAspect) {
    const visibleWidthFrac = targetPixelAspect / sourcePixelAspect;
    const inset = (1 - visibleWidthFrac) / 2;
    return { left: inset, top: 0, right: inset, bottom: 0 };
  }
  const visibleHeightFrac = sourcePixelAspect / targetPixelAspect;
  const inset = (1 - visibleHeightFrac) / 2;
  return { left: 0, top: inset, right: 0, bottom: inset };
}

/**
 * Applying a preset from the crop aspect menu — 1:1 port of
 * `applyCropPreset(_:on:)`'s crop-mutation branches (InspectorView.swift:
 * 851-863; the `editor.cropAspectLock = preset` state write is the caller's
 * responsibility, mirrored by `uiStore.setCropAspectLock`). `free` intentionally
 * returns `null` (no crop mutation — the user keeps the current shape and drags
 * freely, InspectorView.swift:854-856); callers must skip the commit in that case.
 */
export function cropForPreset(preset: CropAspectLock, sourcePixelAspect: number | null): Crop | null {
  if (preset === "free") return null;
  if (preset === "original") return IDENTITY_CROP;
  const target = cropAspectLockPixelAspect(preset);
  if (target === null) return null;
  return cropFittingAspect(sourcePixelAspect, target);
}

export { visibleWidthFraction, visibleHeightFraction };
