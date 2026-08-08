import { readFileSync } from "node:fs";
import { resolve } from "node:path";

type Rgb = readonly [number, number, number];
type Rgba = readonly [number, number, number, number];

const tokens = readFileSync(resolve(process.cwd(), "src/styles/tokens.css"), "utf8");

function tokenValue(name: string): string {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const value = tokens.match(new RegExp(`${escaped}:\\s*([^;]+);`))?.[1]?.trim();
  if (!value) throw new Error(`Unknown CSS token: ${name}`);
  return value;
}

function resolveColor(value: string): string {
  let resolved = value.trim();
  const visited = new Set<string>();
  while (resolved.startsWith("var(")) {
    const name = resolved.match(/^var\(\s*(--[\w-]+)/)?.[1];
    if (!name || visited.has(name)) throw new Error(`Unsupported CSS color: ${value}`);
    visited.add(name);
    resolved = tokenValue(name);
  }
  return resolved;
}

function parseColor(value: string): Rgba {
  const resolved = resolveColor(value);
  const match = resolved.match(
    /^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)(?:\s*,\s*([\d.]+))?\s*\)$/,
  );
  if (!match) throw new Error(`Unsupported CSS color: ${value}`);
  return [
    Number(match[1]),
    Number(match[2]),
    Number(match[3]),
    match[4] === undefined ? 1 : Number(match[4]),
  ];
}

function composite([red, green, blue, alpha]: Rgba, backdrop: Rgb): Rgb {
  return [
    red * alpha + backdrop[0] * (1 - alpha),
    green * alpha + backdrop[1] * (1 - alpha),
    blue * alpha + backdrop[2] * (1 - alpha),
  ];
}

function relativeLuminance([red, green, blue]: Rgb): number {
  const linear = [red, green, blue].map((channel) => {
    const srgb = channel / 255;
    return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!;
}

/** Calculates normal-text contrast after alpha compositing both layers. */
export function textContrastRatio(
  foreground: string,
  background: string,
  backdrop = background,
): number {
  const backdropColor = parseColor(backdrop);
  if (backdropColor[3] !== 1) throw new Error("Backdrop must be opaque");
  const backdropRgb: Rgb = [backdropColor[0], backdropColor[1], backdropColor[2]];
  const backgroundRgb = composite(parseColor(background), backdropRgb);
  const foregroundRgb = composite(parseColor(foreground), backgroundRgb);
  const lighter = Math.max(relativeLuminance(foregroundRgb), relativeLuminance(backgroundRgb));
  const darker = Math.min(relativeLuminance(foregroundRgb), relativeLuminance(backgroundRgb));
  return (lighter + 0.05) / (darker + 0.05);
}
