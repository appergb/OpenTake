import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const titleBarSource = readFileSync(new URL("./TitleBar.tsx", import.meta.url), "utf8");
const viewMenuSource = readFileSync(new URL("./ViewMenu.tsx", import.meta.url), "utf8");

describe("TitleBar alignment", () => {
  it("does not manually offset buttons with top: -2 (lets flex alignItems center do the work)", () => {
    // 历史遗留：4 个按钮都被 `position: relative; top: -2` 强制定位
    // 导致与 macOS 交通灯（OS 控制）和 ViewMenu（无偏移）不在同一水平线
    expect(titleBarSource).not.toContain("top: -2");
    expect(titleBarSource).not.toContain("top:-2");
    expect(viewMenuSource).not.toContain("top: -2");
    expect(viewMenuSource).not.toContain("top:-2");
  });

  it("keeps the view menu trigger on the same 26px button plane as Home", () => {
    expect(viewMenuSource).toContain("width: 26");
    expect(viewMenuSource).toContain("height: 26");
  });

  it("keeps the titlebar-safe-left padding reserved for macOS traffic lights", () => {
    expect(titleBarSource).toContain("var(--titlebar-safe-left)");
  });
});
