import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { DICTS } from "../../i18n/dict";

const titleBarSource = readFileSync(new URL("./TitleBar.tsx", import.meta.url), "utf8");
const viewMenuSource = readFileSync(new URL("./ViewMenu.tsx", import.meta.url), "utf8");
const tauriConfig = JSON.parse(
  readFileSync(new URL("../../../../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
) as {
  app: { windows: Array<{ trafficLightPosition?: { x: number; y: number } }> };
};

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

  it("exposes the Agent panel from the View menu for mouse and accessibility users", () => {
    expect(viewMenuSource).toContain("view.agentPanel");
    expect(viewMenuSource).toContain("toggleAgentPanel");
    expect(viewMenuSource).toContain("⌘⌥A");
  });

  it("localizes the Agent panel menu name instead of exposing its translation key", () => {
    expect(DICTS["zh-CN"]["view.agentPanel"]).toBe("Agent 面板");
    expect(DICTS.en["view.agentPanel"]).toBe("Agent Panel");
  });

  it("keeps the titlebar-safe-left padding reserved for macOS traffic lights", () => {
    expect(titleBarSource).toContain("var(--titlebar-safe-left)");
  });

  it("vertically centers the native macOS traffic lights on the 38px icon plane", () => {
    const trafficLights = tauriConfig.app.windows[0]?.trafficLightPosition;
    const titleBarGeometry = titleBarSource.match(
      /data-tauri-drag-region\s+style=\{\{\s+height:\s*(\d+),([\s\S]*?)padding:/,
    );

    expect(titleBarGeometry).not.toBeNull();
    const titleBarHeight = Number(titleBarGeometry![1]);
    expect(titleBarGeometry![2]).toContain('alignItems: "center"');
    expect(titleBarSource.match(/width:\s*26,\s*height:\s*26,/g)).toHaveLength(5);
    expect(trafficLights).toEqual({ x: 18, y: 12 });
    expect(trafficLights!.y + 14 / 2).toBe(titleBarHeight / 2);
  });
});

describe("TitleBar interchange export menu", () => {
  it("offers all four interchange formats with their extensions and commands", () => {
    // Each format must map to the right extension + backend command.
    for (const [ext, run] of [
      ["xml", "exportXmeml"],
      ["fcpxml", "exportFcpxmlModern"],
      ["otio", "exportOtio"],
      ["edl", "exportEdl"],
    ] as const) {
      expect(titleBarSource).toContain(`ext: "${ext}"`);
      expect(titleBarSource).toContain(`api.${run}`);
    }
  });

  it("renders the export trigger as a popup menu (not a single-format button)", () => {
    expect(titleBarSource).toContain('aria-haspopup="menu"');
    expect(titleBarSource).toContain("INTERCHANGE_FORMATS.map");
  });
});

describe("TitleBar subtitle export menu", () => {
  it("subtitle export menu routes srt and vtt", () => {
    expect(titleBarSource).toContain('(["srt", "vtt"] as const).map');
    expect(titleBarSource).toContain("onExportSubtitles(fmt)");
    expect(titleBarSource).toContain("api.exportSubtitles(withExt(chosen, format), format)");
    expect(titleBarSource).toContain('extensions: [format]');
  });
});
