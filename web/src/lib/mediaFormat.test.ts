import { describe, expect, it } from "vitest";
import { formatFileSize, formatMediaDuration } from "./mediaFormat";

describe("formatMediaDuration", () => {
  it("formats sub-hour durations as M:SS", () => {
    expect(formatMediaDuration(0)).toBe("0:00");
    expect(formatMediaDuration(5)).toBe("0:05");
    expect(formatMediaDuration(65)).toBe("1:05");
    expect(formatMediaDuration(600)).toBe("10:00");
  });

  it("formats hour-plus durations as H:MM:SS", () => {
    expect(formatMediaDuration(3600)).toBe("1:00:00");
    expect(formatMediaDuration(3661)).toBe("1:01:01");
    expect(formatMediaDuration(7325)).toBe("2:02:05");
  });

  it("rounds fractional seconds to the nearest whole second (upstream)", () => {
    expect(formatMediaDuration(4.6)).toBe("0:05");
    expect(formatMediaDuration(59.6)).toBe("1:00"); // rolls the minute over
  });

  it("floors negatives to zero", () => {
    expect(formatMediaDuration(-3)).toBe("0:00");
  });
});

describe("formatFileSize", () => {
  it("renders bytes as a plain integer", () => {
    expect(formatFileSize(0)).toBe("0 bytes");
    expect(formatFileSize(512)).toBe("512 bytes");
    expect(formatFileSize(999)).toBe("999 bytes");
  });

  it("uses DECIMAL (1000-based) units like macOS .file style", () => {
    expect(formatFileSize(1000)).toBe("1 KB");
    expect(formatFileSize(1500)).toBe("1.5 KB");
    expect(formatFileSize(1_000_000)).toBe("1 MB");
    expect(formatFileSize(1_500_000)).toBe("1.5 MB");
    expect(formatFileSize(2_000_000_000)).toBe("2 GB");
  });

  it("trims a trailing .0 for whole values", () => {
    expect(formatFileSize(3000)).toBe("3 KB");
    expect(formatFileSize(3000)).not.toContain(".0");
  });

  it("returns empty string for invalid input", () => {
    expect(formatFileSize(-1)).toBe("");
    expect(formatFileSize(Number.NaN)).toBe("");
  });
});
