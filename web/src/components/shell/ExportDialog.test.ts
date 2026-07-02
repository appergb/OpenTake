import { describe, expect, it } from "vitest";
import {
  defaultMp4Name,
  defaultQuality,
  progressPercent,
  withMp4Ext,
} from "./ExportDialog";

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
