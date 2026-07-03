/**
 * Preview badge preset tables (Item 1 + 2), ported 1:1 from upstream's private
 * enums so the values are unit-tested and shared between the menus and their
 * active-state checks. UI-free on purpose.
 *
 * Upstream refs (palmier-pro, PreviewContainerView.swift):
 *  - ZoomPreset   :847-873
 *  - AspectPreset :775-810
 *  - QualityPreset:812-845
 */

import { CANVAS_ZOOM_FIT } from "./previewZoom";

// MARK: - Zoom presets (PreviewContainerView.swift:847-873)

export interface ZoomPreset {
  label: string;
  value: number;
}

/** 25 / 50 / 75 / Fit(1.0) / 125 / 150 / 200 % (PreviewContainerView.swift:850-872). */
export const ZOOM_PRESETS: readonly ZoomPreset[] = [
  { label: "25%", value: 0.25 },
  { label: "50%", value: 0.5 },
  { label: "75%", value: 0.75 },
  { label: "Fit", value: CANVAS_ZOOM_FIT },
  { label: "125%", value: 1.25 },
  { label: "150%", value: 1.5 },
  { label: "200%", value: 2.0 },
] as const;

/** True when `zoom` is (within 0.01) the preset's value — upstream
 *  `isZoomPresetActive` (PreviewContainerView.swift:234-236). */
export function isZoomPresetActive(preset: ZoomPreset, zoom: number): boolean {
  return Math.abs(zoom - preset.value) < 0.01;
}

/** Badge text: "Fit" when at the fit preset, else "NN%" (PreviewContainerView.swift:226-232). */
export function zoomBadgeLabel(zoom: number): string {
  const fit = ZOOM_PRESETS.find((p) => p.value === CANVAS_ZOOM_FIT);
  if (fit && isZoomPresetActive(fit, zoom)) return "Fit";
  return `${Math.round(zoom * 100)}%`;
}

// MARK: - Aspect presets (PreviewContainerView.swift:775-810)

export interface AspectPreset {
  label: string;
  width: number;
  height: number;
}

/** Exact upstream dims (PreviewContainerView.swift:789-809). */
export const ASPECT_PRESETS: readonly AspectPreset[] = [
  { label: "16:9", width: 1920, height: 1080 },
  { label: "9:14", width: 1080, height: 1680 },
  { label: "9:16", width: 1080, height: 1920 },
  { label: "1:1", width: 1080, height: 1080 },
  { label: "4:3", width: 1440, height: 1080 },
  { label: "2.4:1", width: 2560, height: 1080 },
] as const;

/** True when the timeline currently matches this aspect preset's exact dims
 *  (upstream check in `aspectMenuItems`, PreviewContainerView.swift:165). */
export function isAspectPresetActive(preset: AspectPreset, width: number, height: number): boolean {
  return width === preset.width && height === preset.height;
}

// MARK: - Quality presets (PreviewContainerView.swift:812-845)

export interface QualityPreset {
  label: string;
  /** Short-edge target px (PreviewContainerView.swift:837-844). */
  shortEdge: number;
}

/** 720p / 1080p / 2K / 4K by short edge (PreviewContainerView.swift:815-844). */
export const QUALITY_PRESETS: readonly QualityPreset[] = [
  { label: "720p", shortEdge: 720 },
  { label: "1080p", shortEdge: 1080 },
  { label: "2K", shortEdge: 1440 },
  { label: "4K", shortEdge: 2160 },
] as const;

/**
 * Scale the current dims to this quality's short edge, preserving aspect — 1:1
 * with upstream `QualityPreset.resolution(currentWidth:currentHeight:)`
 * (PreviewContainerView.swift:825-831). The short edge becomes `shortEdge`; the
 * long edge scales proportionally (integer-truncated, matching Swift `Int(...)`).
 */
export function qualityResolution(
  preset: QualityPreset,
  currentWidth: number,
  currentHeight: number,
): { width: number; height: number } {
  const target = preset.shortEdge;
  if (currentWidth <= currentHeight) {
    return { width: target, height: Math.trunc((target * currentHeight) / currentWidth) };
  }
  return { width: Math.trunc((target * currentWidth) / currentHeight), height: target };
}

/** True when the timeline's short edge equals this preset's — upstream
 *  `QualityPreset.matches(width:height:)` (PreviewContainerView.swift:833-835). */
export function isQualityPresetActive(preset: QualityPreset, width: number, height: number): boolean {
  return Math.min(width, height) === preset.shortEdge;
}

/** Short-name badge label from the timeline's short edge — upstream
 *  `qualityBadgeLabel` (PreviewContainerView.swift:245-251). */
export function qualityBadgeLabel(width: number, height: number): string {
  const h = Math.min(width, height);
  if (h <= 720) return "HD";
  if (h <= 1080) return "FHD";
  if (h <= 1440) return "2K";
  return "4K";
}

/**
 * Convert a preview-quality SHORT-edge target to the LONGEST-side cap that the
 * Rust composite path takes (`composite_frame`'s `max_size`,
 * `preview_render_size` caps the longest side). Preserves the current aspect:
 * `cap = round(shortEdge * max(w,h)/min(w,h))`. Returns `undefined` for a
 * `null`/invalid short edge (→ backend default cap).
 *
 * This is OpenTake's honest interpretation of a "preview quality" selector: it
 * only bounds the render resolution of the paths that accept a cap; it does not
 * touch the timeline W/H (that is the Aspect badge's job via SetTimelineSettings).
 */
export function previewQualityMaxSize(
  shortEdge: number | null,
  width: number,
  height: number,
): number | undefined {
  if (shortEdge == null || shortEdge <= 0) return undefined;
  const short = Math.min(width, height);
  const long = Math.max(width, height);
  if (short <= 0) return undefined;
  return Math.round((shortEdge * long) / short);
}
