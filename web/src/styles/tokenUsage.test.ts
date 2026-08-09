import { readFileSync, readdirSync } from "node:fs";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const srcRoot = new URL("..", import.meta.url);
const componentsRoot = new URL("../components", import.meta.url);
const componentsPath = fileURLToPath(componentsRoot);
const tokens = readFileSync(new URL("./tokens.css", import.meta.url), "utf8");

function source(path: string): string {
  return readFileSync(new URL(path, srcRoot), "utf8");
}

function productionFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return productionFiles(path);
    if (![".ts", ".tsx", ".css"].includes(extname(path)) || /\.(test|spec)\./.test(path)) return [];
    return [path];
  });
}

describe("design-token usage", () => {
  it("representative_panel_spacing_type_radius_and_color_matrix", () => {
    const matrix: Array<[string, string, string[]]> = [
      ["shell", source("components/ui/PanelShell.tsx") + source("styles/global.css"), ["--bg-base", "--bg-surface", "--panel-gap", "--radius-sm", "--accent-primary"]],
      ["toolbar", source("components/toolbar/Toolbar.tsx"), ["--toolbar-height", "--bg-surface", "--space-md", "--border-primary", "--fw-semibold"]],
      ["media", source("components/media/MediaPanel.tsx"), ["--bg-placeholder", "--space-lg", "--fs-sm", "--radius-md", "--text-secondary"]],
      ["inspector", source("components/inspector/Inspector.tsx"), ["--bg-raised", "--space-xl", "--fs-xxs", "--radius-sm", "--text-tertiary"]],
      ["preview", source("components/preview/Preview.tsx"), ["--bg-prominent", "--space-md", "--fs-sm-md", "--radius-md", "--accent-timecode"]],
      ["timeline", source("components/timeline/TimelineContainer.tsx") + source("components/timeline/clipRenderer.ts"), ["LAYOUT", "ACCENT", "TEXT", "BORDER", "CLIP"]],
    ];
    for (const [panel, panelSource, expected] of matrix) {
      for (const token of expected) expect(panelSource, `${panel}: ${token}`).toContain(token);
    }

    const captions = source("components/media/CaptionsTab.tsx");
    for (const token of [
      "LAYOUT.captionDefaultFontSize", "LAYOUT.captionMinFontSize",
      "LAYOUT.captionMaxFontSize", "LAYOUT.captionDefaultCenterY",
      "LAYOUT.captionCenterSnapThreshold",
    ]) expect(captions, `captions: ${token}`).toContain(token);
    const keyframeUi = source("components/inspector/KeyframesLaneRow.tsx") + source("components/inspector/KeyframesRuler.tsx");
    for (const token of ["LAYOUT.keyframeRowHeight", "LAYOUT.keyframeRulerHeight", "LAYOUT.keyframeDiamondSize"]) {
      expect(keyframeUi, `keyframes: ${token}`).toContain(token);
    }

    const defined = new Set([...tokens.matchAll(/--([a-z0-9-]+)\s*:/g)].map((match) => match[1]));
    const undefinedReferences = productionFiles(componentsPath).flatMap((path) => {
      const contents = readFileSync(path, "utf8");
      return [...contents.matchAll(/var\(--([a-z0-9-]+)/g)]
        .map((match) => match[1])
        .filter((name) => !defined.has(name))
        .map((name) => `${path.replace(componentsPath, "components/")}: --${name}`);
    });
    expect(undefinedReferences).toEqual([]);
  });
});
