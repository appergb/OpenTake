/**
 * Byte-size formatting for the Settings Storage pane. Semantic port of
 * upstream's `ByteCountFormatter` (`.file` style): binary units (1 KB = 1024 B),
 * one decimal below 10, integers above, "0 B" for zero/negative/non-finite.
 * Pure and dependency-free so it is unit-testable in isolation.
 */

const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** Format `bytes` as a human file size, e.g. `1536` -> `"1.5 KB"`. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0) return `${Math.round(value)} ${UNITS[unit]}`;
  const rounded = Math.round(value * 10) / 10;
  const text = rounded >= 10 ? String(Math.round(rounded)) : String(rounded);
  return `${text} ${UNITS[unit]}`;
}
