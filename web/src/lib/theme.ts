/**
 * Numeric design constants for canvas drawing (canvas2d cannot read CSS
 * variables). 1:1 with `styles/tokens.css` and upstream `AppTheme.swift` /
 * `Constants.swift`. See docs/specs/frontend-UI-1to1-SPEC.md §1, §5.
 */

/** §1.1 Background colors (rgb strings for canvas fillStyle). */
export const BG = {
  base: "rgb(10,10,10)",
  surface: "rgb(22,22,22)",
  raised: "rgb(30,30,30)",
  prominent: "rgb(44,44,44)",
  previewCanvas: "#000",
} as const;

/** §1.2 Borders. */
export const BORDER = {
  primary: "rgba(255,255,255,0.16)",
  subtle: "rgba(255,255,255,0.12)",
  divider: "rgba(255,255,255,0.44)",
} as const;

/** §1.3 Text colors. */
export const TEXT = {
  primary: "rgba(255,255,255,1)",
  secondary: "rgba(255,255,255,0.8)",
  tertiary: "rgba(255,255,255,0.62)",
  muted: "rgba(255,255,255,0.34)",
} as const;

/** §1.4 Accents / status. */
export const ACCENT = {
  timecode: "rgb(242,153,51)",
  primary: "rgb(245,239,228)",
  error: "rgb(229,79,79)",
  // System colors upstream uses directly in the timeline.
  systemRed: "rgb(255,59,48)",
  systemYellow: "rgb(255,204,0)",
  systemOrange: "rgb(255,149,0)",
  offsetBadge: "rgb(255,71,71)",
} as const;

/** §1.5 Track colors keyed by ClipType. */
export const TRACK_COLOR: Record<string, string> = {
  video: "rgb(0,145,194)",
  audio: "rgb(88,168,34)",
  image: "rgb(183,45,210)",
  text: "rgb(183,45,210)",
  lottie: "rgb(224,168,0)",
};

/** §1.7 Spacing (px). */
export const SPACE = {
  xxs: 2,
  xs: 4,
  sm: 6,
  smMd: 8,
  md: 10,
  mdLg: 12,
  lg: 14,
  lgXl: 16,
  xl: 20,
  xlXxl: 24,
  xxl: 28,
} as const;

/** §1.6 Radius (px). */
export const RADIUS = {
  xs: 3,
  xsSm: 4,
  sm: 6,
  md: 10,
  mdLg: 12,
  lg: 14,
  xl: 20,
} as const;

/** §1.8 Font sizes (px). */
export const FS = {
  micro: 8,
  xxs: 9,
  xs: 10,
  sm: 11,
  smMd: 12,
  md: 13,
  mdLg: 14,
  lg: 15,
  xl: 18,
} as const;

export const FONT_UI =
  '-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", system-ui, sans-serif';
export const FONT_MONO = 'ui-monospace, "SF Mono", Menlo, Consolas, monospace';

/**
 * §5.2 Timeline layout constants (Constants.swift). Each value is the exact
 * upstream literal — do not approximate.
 */
export const LAYOUT = {
  rulerHeight: 24, // Constants.swift:46
  dropZoneHeight: 60, // :47
  trackHeaderWidth: 100, // :48
  insertThreshold: 10, // :52
  dragThreshold: 3, // :53
  previewMinHeight: 320, // :57
} as const;

/** §5.2 Trim constants (Constants.swift:99-102). */
export const TRIM = {
  handleWidth: 4,
  clipCornerRadius: 3,
} as const;

/** §5.5 Track size bounds (Constants.swift:76-78; Timeline.swift:33-34). */
export const TRACK_SIZE = {
  defaultHeight: 50,
  minHeight: 32,
  maxHeight: 200,
  resizeHandleZone: 6,
} as const;

/** §5.7 Snap constants (Constants.swift:70-72). */
export const SNAP = {
  thresholdPixels: 8,
  stickyMultiplier: 1.5,
  playheadMultiplier: 1.5,
} as const;

/** §5.8 Zoom constants (Constants.swift:61, 84-87). */
export const ZOOM = {
  default: 4.0, // Defaults.pixelsPerFrame
  max: 40,
  scrollSensitivity: 0.04,
  magnifySensitivity: 1.5,
  panSpeed: 5,
} as const;

/** §5.6 Playhead triangle size (PlayheadOverlay.swift). */
export const PLAYHEAD_TRIANGLE = 8;

/** §5.4 Clip rendering insets (ClipRenderer.swift). */
export const CLIP = {
  stripWidth: 3, // left color strip
  labelBarHeight: 16,
  keyframeDiamondRadius: 3,
  minWidthForLabel: 20,
} as const;
