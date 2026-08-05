import { describe, expect, it } from "vitest";
import { formatBytes } from "./storageFormat";

describe("formatBytes", () => {
  it("renders zero and invalid inputs as 0 B", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-5)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
    expect(formatBytes(Number.POSITIVE_INFINITY)).toBe("0 B");
  });

  it("renders whole bytes without a unit scale", () => {
    expect(formatBytes(1)).toBe("1 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("scales to KB with one decimal below 10", () => {
    expect(formatBytes(1024)).toBe("1 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(10240)).toBe("10 KB");
  });

  it("scales to MB and GB", () => {
    expect(formatBytes(1048576)).toBe("1 MB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5 MB");
    expect(formatBytes(5.5 * 1024 * 1024 * 1024)).toBe("5.5 GB");
  });

  it("rounds to the nearest tenth like the file-style formatter", () => {
    expect(formatBytes(1500)).toBe("1.5 KB");
    expect(formatBytes(1540)).toBe("1.5 KB");
    expect(formatBytes(1590)).toBe("1.6 KB");
    expect(formatBytes(1600)).toBe("1.6 KB");
  });
});
