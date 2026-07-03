/**
 * Pure formatting helpers for the Inspector's Source (media-asset) state —
 * File section values. 1:1 with upstream `InspectorView.swift`:
 *   - `formatDuration(_:)` (:1049-1058): integer seconds → `h:mm:ss` (with an
 *     hour) or `m:ss`.
 *   - `fileSize(for:)` (:1041-1047): `ByteCountFormatter` with `.file` count
 *     style — macOS `.file` is DECIMAL (1000-based) KB/MB/GB, so we mirror that
 *     base and unit set rather than binary (1024) KiB.
 */

/** Integer-second duration → `H:MM:SS` when an hour is present, else `M:SS`.
 *  Mirrors upstream `formatDuration` (rounds to whole seconds first). */
export function formatMediaDuration(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  const hours = Math.floor(total / 3600);
  const mins = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return hours > 0 ? `${hours}:${pad(mins)}:${pad(secs)}` : `${mins}:${pad(secs)}`;
}

const SIZE_UNITS = ["bytes", "KB", "MB", "GB", "TB", "PB"] as const;

/** Byte count → human string using DECIMAL units (matching macOS
 *  `ByteCountFormatter.countStyle = .file`, which is 1000-based). Bytes render
 *  as a plain integer ("512 bytes"); larger units keep one decimal ("1.5 MB").
 *  Trailing ".0" is trimmed for whole values ("2 KB", not "2.0 KB"). */
export function formatFileSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "";
  if (bytes < 1000) return `${Math.round(bytes)} ${SIZE_UNITS[0]}`;
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < SIZE_UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const rounded = Math.round(value * 10) / 10;
  const text = Number.isInteger(rounded) ? rounded.toString() : rounded.toFixed(1);
  return `${text} ${SIZE_UNITS[unit]}`;
}
