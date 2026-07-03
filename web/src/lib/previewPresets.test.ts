import { describe, expect, it } from "vitest";
import {
  ASPECT_PRESETS,
  QUALITY_PRESETS,
  ZOOM_PRESETS,
  isAspectPresetActive,
  isQualityPresetActive,
  isZoomPresetActive,
  previewQualityMaxSize,
  qualityBadgeLabel,
  qualityResolution,
  zoomBadgeLabel,
} from "./previewPresets";

describe("ZOOM_PRESETS", () => {
  it("matches upstream ZoomPreset list + values exactly", () => {
    expect(ZOOM_PRESETS.map((p) => p.label)).toEqual([
      "25%",
      "50%",
      "75%",
      "Fit",
      "125%",
      "150%",
      "200%",
    ]);
    expect(ZOOM_PRESETS.map((p) => p.value)).toEqual([0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0]);
  });

  it("isZoomPresetActive uses a 0.01 epsilon", () => {
    const fit = ZOOM_PRESETS.find((p) => p.label === "Fit")!;
    expect(isZoomPresetActive(fit, 1.005)).toBe(true);
    expect(isZoomPresetActive(fit, 1.02)).toBe(false);
  });

  it("zoomBadgeLabel shows Fit at 1.0 and a percentage otherwise", () => {
    expect(zoomBadgeLabel(1.0)).toBe("Fit");
    expect(zoomBadgeLabel(1.5)).toBe("150%");
    expect(zoomBadgeLabel(0.25)).toBe("25%");
  });
});

describe("ASPECT_PRESETS", () => {
  it("matches upstream AspectPreset dims exactly", () => {
    expect(ASPECT_PRESETS).toEqual([
      { label: "16:9", width: 1920, height: 1080 },
      { label: "9:14", width: 1080, height: 1680 },
      { label: "9:16", width: 1080, height: 1920 },
      { label: "1:1", width: 1080, height: 1080 },
      { label: "4:3", width: 1440, height: 1080 },
      { label: "2.4:1", width: 2560, height: 1080 },
    ]);
  });

  it("isAspectPresetActive matches exact dims", () => {
    expect(isAspectPresetActive(ASPECT_PRESETS[0], 1920, 1080)).toBe(true);
    expect(isAspectPresetActive(ASPECT_PRESETS[0], 1280, 720)).toBe(false);
  });
});

describe("QUALITY_PRESETS", () => {
  it("matches upstream short-edge targets", () => {
    expect(QUALITY_PRESETS.map((p) => [p.label, p.shortEdge])).toEqual([
      ["720p", 720],
      ["1080p", 1080],
      ["2K", 1440],
      ["4K", 2160],
    ]);
  });

  it("qualityResolution scales a landscape canvas by its short edge", () => {
    // 1920x1080 (landscape) → 1080p short edge → unchanged.
    expect(qualityResolution(QUALITY_PRESETS[1], 1920, 1080)).toEqual({ width: 1920, height: 1080 });
    // 1920x1080 → 4K short edge 2160 → 3840x2160.
    expect(qualityResolution(QUALITY_PRESETS[3], 1920, 1080)).toEqual({ width: 3840, height: 2160 });
  });

  it("qualityResolution scales a portrait canvas by its short edge", () => {
    // 1080x1920 (portrait) → 720p short edge → 720x1280.
    expect(qualityResolution(QUALITY_PRESETS[0], 1080, 1920)).toEqual({ width: 720, height: 1280 });
  });

  it("qualityResolution truncates like Swift Int(...)", () => {
    // 1000x1080 landscape (short edge = width 1000) → target 720 →
    // width 720, height trunc(720*1080/1000)=trunc(777.6)=777.
    expect(qualityResolution(QUALITY_PRESETS[0], 1080, 1000)).toEqual({ width: 777, height: 720 });
  });

  it("isQualityPresetActive matches on the short edge", () => {
    expect(isQualityPresetActive(QUALITY_PRESETS[1], 1920, 1080)).toBe(true);
    expect(isQualityPresetActive(QUALITY_PRESETS[1], 1280, 720)).toBe(false);
  });

  it("qualityBadgeLabel buckets by short edge", () => {
    expect(qualityBadgeLabel(1280, 720)).toBe("HD");
    expect(qualityBadgeLabel(1920, 1080)).toBe("FHD");
    expect(qualityBadgeLabel(2560, 1440)).toBe("2K");
    expect(qualityBadgeLabel(3840, 2160)).toBe("4K");
  });
});

describe("previewQualityMaxSize", () => {
  it("converts a short-edge target to a longest-side cap preserving aspect", () => {
    // 16:9, short edge 1080 → long cap = 1080 * 1920/1080 = 1920.
    expect(previewQualityMaxSize(1080, 1920, 1080)).toBe(1920);
    // 16:9, short edge 720 → long cap = 720 * 16/9 = 1280.
    expect(previewQualityMaxSize(720, 1920, 1080)).toBe(1280);
    // Portrait 9:16, short edge 1080 → long cap = 1080 * 1920/1080 = 1920.
    expect(previewQualityMaxSize(1080, 1080, 1920)).toBe(1920);
  });

  it("returns undefined for null or invalid input (backend default cap)", () => {
    expect(previewQualityMaxSize(null, 1920, 1080)).toBeUndefined();
    expect(previewQualityMaxSize(0, 1920, 1080)).toBeUndefined();
    expect(previewQualityMaxSize(1080, 0, 0)).toBeUndefined();
  });
});
