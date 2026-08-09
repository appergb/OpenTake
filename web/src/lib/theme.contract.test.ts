import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  ACCENT,
  ANIM,
  BG,
  BORDER,
  BORDER_WIDTH,
  FS,
  FONT_WEIGHT,
  ICON_SIZE,
  LAYOUT,
  OPACITY,
  RADIUS,
  SHADOW,
  SPACE,
  TEXT,
  TRACK_COLOR,
  TRACKING,
} from "./theme";

const cssSource = readFileSync(new URL("../styles/tokens.css", import.meta.url), "utf8");

function cssToken(name: string): string | undefined {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return cssSource.match(new RegExp(`--${escaped}\\s*:\\s*([^;]+);`))?.[1]
    ?.replace(/\s+/g, " ")
    .replace(/\s*,\s*/g, ",")
    .trim();
}

describe("design-token contract", () => {
  it("complete_upstream_token_table_and_css_projection", () => {
    expect(BG).toEqual({
      base: "rgb(10,10,10)", surface: "rgb(22,22,22)", raised: "rgb(30,30,30)",
      prominent: "rgb(44,44,44)", placeholder: "rgb(30,30,30)", previewCanvas: "#000",
    });
    expect(BORDER).toEqual({
      primary: "rgba(255,255,255,0.16)", subtle: "rgba(255,255,255,0.12)",
      divider: "rgba(255,255,255,0.44)",
    });
    expect(BORDER_WIDTH).toEqual({ hairline: 0.5, thin: 1, medium: 1.5, thick: 2 });
    expect(TEXT).toEqual({
      primary: "rgba(255,255,255,1)", secondary: "rgba(255,255,255,0.8)",
      tertiary: "rgba(255,255,255,0.62)", muted: "rgba(255,255,255,0.34)",
    });
    expect(ACCENT).toMatchObject({
      timecode: "rgb(242,153,51)", primary: "rgb(245,239,228)",
      spotlight: "rgb(255,69,69)", error: "rgb(229,79,79)",
      glassTint: "rgba(245,239,228,0.05)",
    });
    expect(TRACK_COLOR).toEqual({
      video: "rgb(0,145,194)", audio: "rgb(88,168,34)",
      image: "rgb(183,45,210)", text: "rgb(183,45,210)", lottie: "rgb(224,168,0)",
    });
    expect(RADIUS).toEqual({ xs: 3, xsSm: 4, sm: 6, md: 10, mdLg: 12, lg: 14, xl: 20 });
    expect(SPACE).toEqual({ xxs: 2, xs: 4, sm: 6, smMd: 8, md: 10, mdLg: 12, lg: 14, lgXl: 16, xl: 20, xlXxl: 24, xxl: 28 });
    expect(FS).toEqual({ micro: 8, xxs: 9, xs: 10, sm: 11, smMd: 12, md: 13, mdLg: 14, lg: 15, xl: 18, title1: 22, title2: 28, display: 36 });
    expect(FONT_WEIGHT).toEqual({ light: 300, regular: 400, medium: 500, semibold: 600, bold: 700 });
    expect(TRACKING).toEqual({ tight: -0.5, normal: 0, wide: 1.5 });
    expect(ICON_SIZE).toEqual({ xxs: 12, xs: 14, sm: 18, smMd: 20, md: 22, mdLg: 24, lg: 26, lgXl: 28, xl: 30 });
    expect(OPACITY).toEqual({ opaque: 1, subtle: 0.04, hint: 0.06, faint: 0.08, soft: 0.1, muted: 0.15, moderate: 0.25, medium: 0.35, strong: 0.55, prominent: 0.8 });
    expect(SHADOW).toEqual({ sm: "0 0.5px 1px rgba(0,0,0,0.3)", md: "0 2px 4px rgba(0,0,0,0.3)", lg: "0 8px 24px rgba(0,0,0,0.25)" });
    expect(ANIM).toEqual({ hoverMs: 150, transitionMs: 200 });
    expect(LAYOUT).toMatchObject({
      panelHeaderHeight: 28, toolbarHeight: 38, panelGap: 5, tabRailWidth: 38,
      contextRowHeight: 22, projectCardWidth: 150, projectCardHeight: 120,
      captionPreviewMaxHeight: 150, toolImagePreviewMaxHeight: 50,
      captionDefaultFontSize: 48, captionMinFontSize: 12, captionMaxFontSize: 300,
      captionCenterSnapThreshold: 0.02, captionDefaultCenterY: 0.9,
      generationReferenceTileWidth: 80, generationReferenceTileHeight: 56,
      keyframeRowHeight: 22, keyframeRulerHeight: 18, keyframeStripHeight: 14,
      keyframeHeaderHeight: 32, keyframeDiamondSize: 8,
    });

    const cssProjection: Record<string, string> = {
      "bg-base": "rgb(10,10,10)", "bg-surface": "rgb(22,22,22)",
      "bg-raised": "rgb(30,30,30)", "bg-prominent": "rgb(44,44,44)",
      "bg-placeholder": "rgb(30,30,30)", "bg-preview-canvas": "#000",
      "border-primary": "rgba(255,255,255,0.16)", "border-subtle": "rgba(255,255,255,0.12)",
      "border-divider": "rgba(255,255,255,0.44)", "bw-hairline": "0.5px",
      "bw-thin": "1px", "bw-medium": "1.5px", "bw-thick": "2px",
      "text-primary": "rgba(255,255,255,1)", "text-secondary": "rgba(255,255,255,0.8)",
      "text-tertiary": "rgba(255,255,255,0.62)", "text-muted": "rgba(255,255,255,0.34)",
      "accent-timecode": "rgb(242,153,51)", "accent-primary": "rgb(245,239,228)",
      "accent-spotlight": "rgb(255,69,69)", "status-error": "rgb(229,79,79)",
      "glass-tint": "rgba(245,239,228,0.05)",
      "track-video": "rgb(0,145,194)", "track-audio": "rgb(88,168,34)",
      "track-image": "rgb(183,45,210)", "track-text": "rgb(183,45,210)", "track-lottie": "rgb(224,168,0)",
      "radius-xs": "3px", "radius-xs-sm": "4px", "radius-sm": "6px", "radius-md": "10px",
      "radius-md-lg": "12px", "radius-lg": "14px", "radius-xl": "20px",
      "space-xxs": "2px", "space-xs": "4px", "space-sm": "6px", "space-sm-md": "8px",
      "space-md": "10px", "space-md-lg": "12px", "space-lg": "14px", "space-lg-xl": "16px",
      "space-xl": "20px", "space-xl-xxl": "24px", "space-xxl": "28px",
      "fs-micro": "8px", "fs-xxs": "9px", "fs-xs": "10px", "fs-sm": "11px",
      "fs-sm-md": "12px", "fs-md": "13px", "fs-md-lg": "14px", "fs-lg": "15px",
      "fs-xl": "18px", "fs-title1": "22px", "fs-title2": "28px", "fs-display": "36px",
      "fw-light": "300", "fw-regular": "400", "fw-medium": "500", "fw-semibold": "600", "fw-bold": "700",
      "tracking-tight": "-0.5px", "tracking-normal": "0", "tracking-wide": "1.5px",
      "icon-xxs": "12px", "icon-xs": "14px", "icon-sm": "18px", "icon-sm-md": "20px",
      "icon-md": "22px", "icon-md-lg": "24px", "icon-lg": "26px", "icon-lg-xl": "28px", "icon-xl": "30px",
      "op-opaque": "1", "op-subtle": "0.04", "op-hint": "0.06", "op-faint": "0.08", "op-soft": "0.1",
      "op-muted": "0.15", "op-moderate": "0.25", "op-medium": "0.35", "op-strong": "0.55", "op-prominent": "0.8",
      "shadow-sm": "0 0.5px 1px rgba(0,0,0,0.3)", "shadow-md": "0 2px 4px rgba(0,0,0,0.3)",
      "shadow-lg": "0 8px 24px rgba(0,0,0,0.25)", "anim-hover": "150ms", "anim-transition": "200ms",
      "panel-header-height": "28px", "toolbar-height": "38px", "panel-gap": "5px", "tab-rail-width": "38px",
      "context-row-height": "22px", "project-card-width": "150px", "project-card-height": "120px",
      "caption-preview-max-height": "150px", "tool-image-preview-max-height": "50px",
      "caption-default-font-size": "48px", "caption-min-font-size": "12px", "caption-max-font-size": "300px",
      "caption-center-snap-threshold": "0.02", "caption-default-center-y": "0.9",
      "generation-reference-tile-width": "80px", "generation-reference-tile-height": "56px",
      "keyframe-row-height": "22px", "keyframe-ruler-height": "18px", "keyframe-strip-height": "14px",
      "keyframe-header-height": "32px", "keyframe-diamond-size": "8px",
    };
    for (const [name, expected] of Object.entries(cssProjection)) {
      expect(cssToken(name), `--${name}`).toBe(expected);
    }
  });
});
