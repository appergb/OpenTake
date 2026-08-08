import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { textContrastRatio } from "../../../test/contrast";
import {
  defaultBundleName,
  defaultMp4Name,
  defaultQuality,
  formatBytes,
  progressPercent,
  withMp4Ext,
} from "./ExportDialog";

type Rgb = readonly [number, number, number];

function parseHexColor(value: string): Rgb {
  const normalized = value.trim().replace(/^#/, "");
  const hex = normalized.length === 3
    ? normalized.split("").map((digit) => digit.repeat(2)).join("")
    : normalized;
  if (!/^[0-9a-f]{6}$/i.test(hex)) throw new Error(`Unsupported color: ${value}`);
  return [
    Number.parseInt(hex.slice(0, 2), 16),
    Number.parseInt(hex.slice(2, 4), 16),
    Number.parseInt(hex.slice(4, 6), 16),
  ];
}

function relativeLuminance([red, green, blue]: Rgb): number {
  const linear = [red, green, blue].map((channel) => {
    const srgb = channel / 255;
    return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!;
}

function contrastRatio(first: Rgb, second: Rgb): number {
  const lighter = Math.max(relativeLuminance(first), relativeLuminance(second));
  const darker = Math.min(relativeLuminance(first), relativeLuminance(second));
  return (lighter + 0.05) / (darker + 0.05);
}

describe("withMp4Ext", () => {
  it("appends .mp4 when missing", () => {
    expect(withMp4Ext("/out/clip")).toBe("/out/clip.mp4");
  });

  it("keeps an existing .mp4 extension (case-insensitive)", () => {
    expect(withMp4Ext("/out/clip.mp4")).toBe("/out/clip.mp4");
    expect(withMp4Ext("/out/clip.MP4")).toBe("/out/clip.MP4");
  });

  it("appends .mp4 to a path with a different extension (does not strip it)", () => {
    // The save dialog filters to .mp4, but guard the H.264 container regardless.
    expect(withMp4Ext("/out/clip.mov")).toBe("/out/clip.mov.mp4");
  });
});

describe("defaultMp4Name", () => {
  it("falls back to Timeline.mp4 for an unsaved project", () => {
    expect(defaultMp4Name(null)).toBe("Timeline.mp4");
  });

  it("derives the name from the project bundle, stripping dir + .opentake", () => {
    expect(defaultMp4Name("/Users/me/Documents/OpenTake/My Film.opentake")).toBe(
      "My Film.mp4",
    );
  });

  it("handles a bare bundle name with no directory", () => {
    expect(defaultMp4Name("Demo.opentake")).toBe("Demo.mp4");
  });
});

describe("defaultBundleName", () => {
  it("falls back to Untitled.opentake for an unsaved project", () => {
    expect(defaultBundleName(null)).toBe("Untitled.opentake");
  });

  it("round-trips a saved project bundle name (dir stripped, extension kept)", () => {
    expect(
      defaultBundleName("/Users/me/Documents/OpenTake/My Film.opentake"),
    ).toBe("My Film.opentake");
  });

  it("handles a bare bundle name with no directory", () => {
    expect(defaultBundleName("Demo.opentake")).toBe("Demo.opentake");
  });
});

describe("formatBytes", () => {
  it("reports 0 B for zero, negative, or non-finite sizes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-10)).toBe("0 B");
    expect(formatBytes(NaN)).toBe("0 B");
  });

  it("keeps raw bytes below 1 KB with no decimal", () => {
    expect(formatBytes(512)).toBe("512 B");
  });

  it("scales into KB / MB / GB with one decimal", () => {
    expect(formatBytes(1024)).toBe("1 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5 MB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1 GB");
  });
});

describe("defaultQuality", () => {
  it("maps standard 1080p timelines to the 1080p bucket", () => {
    expect(defaultQuality(1920, 1080)).toBe("1080p");
  });

  it("maps a vertical 1080-wide timeline to 1080p (short edge drives it)", () => {
    expect(defaultQuality(1080, 1920)).toBe("1080p");
  });

  it("maps small (≤840 short edge) timelines to 720p", () => {
    expect(defaultQuality(1280, 720)).toBe("720p");
  });

  it("maps large (≥1620 short edge) timelines to 4k", () => {
    expect(defaultQuality(3840, 2160)).toBe("4k");
  });
});

describe("progressPercent", () => {
  it("reports 0 before any frames are done", () => {
    expect(progressPercent(0, 300)).toBe(0);
  });

  it("computes a whole-number percent", () => {
    expect(progressPercent(150, 300)).toBe(50);
  });

  it("reports 100 when done reaches total", () => {
    expect(progressPercent(300, 300)).toBe(100);
  });

  it("rounds to the nearest whole percent", () => {
    expect(progressPercent(1, 3)).toBe(33);
    expect(progressPercent(2, 3)).toBe(67);
  });

  it("returns 0 for a zero (or negative) total instead of dividing by zero", () => {
    expect(progressPercent(0, 0)).toBe(0);
    expect(progressPercent(5, 0)).toBe(0);
    expect(progressPercent(0, -1)).toBe(0);
  });

  it("clamps done beyond total to 100", () => {
    expect(progressPercent(400, 300)).toBe(100);
  });
});

it("offers no bundle mode while the secure native workflow is under construction", () => {
  const source = readFileSync(new URL("./ExportDialog.tsx", import.meta.url), "utf8");
  expect(source).not.toMatch(/\bid\s*:\s*["']bundle["']/);
});

it("keeps normal text on the primary export action at WCAG AA contrast", () => {
  const source = readFileSync(new URL("./ExportDialog.tsx", import.meta.url), "utf8");
  const tokens = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");
  const primaryButton = source.match(
    /<button\s+type="button"\s+disabled=\{busy\}\s+onClick=\{mode === "bundle"[\s\S]*?<\/button>/,
  )?.[0];
  const foreground = primaryButton?.match(/\bcolor:\s*"(#[0-9a-f]+)"/i)?.[1];
  const accent = tokens.match(
    /--accent-primary:\s*rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)/,
  );

  expect(primaryButton).toBeDefined();
  expect(foreground).toBeDefined();
  expect(accent).not.toBeNull();
  const background: Rgb = [Number(accent![1]), Number(accent![2]), Number(accent![3])];
  expect(contrastRatio(parseHexColor(foreground!), background)).toBeGreaterThanOrEqual(4.5);
});

it("keeps the small export error alert at WCAG AA normal-text contrast", () => {
  const source = readFileSync(new URL("./ExportDialog.tsx", import.meta.url), "utf8");
  const alert = source.match(/\{error && \([\s\S]*?role="alert"[\s\S]*?\{error\}[\s\S]*?<\/div>/)?.[0];
  const foreground = alert?.match(/\bcolor:\s*"([^"]+)"/)?.[1];
  const background = alert?.match(/\bbackground:\s*"([^"]+)"/)?.[1];

  expect(alert).toBeDefined();
  expect(foreground).toBeDefined();
  expect(background).toBeDefined();
  expect(
    textContrastRatio(foreground!, background!, "var(--bg-elevated)"),
  ).toBeGreaterThanOrEqual(4.5);
});
