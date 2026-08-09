import { readFileSync } from "node:fs";
import { relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

interface ExactOwner {
  capability: string;
  owner: string;
  source: URL;
  liveBoundary: string;
  visibleEvidence: URL;
}

const exactOwners: ExactOwner[] = [
  {
    capability: "AI Edit 建议与审阅",
    owner: "AiEditTab",
    source: new URL("../inspector/AiEditTab.tsx", import.meta.url),
    liveBoundary: "edit.setClipProperties",
    visibleEvidence: new URL("../inspector/AiEditTab.test.tsx", import.meta.url),
  },
  {
    capability: "音乐浏览、导入与放置",
    owner: "MusicTab",
    source: new URL("../media/MusicTab.tsx", import.meta.url),
    liveBoundary: "addMediaToTimeline",
    visibleEvidence: new URL("../media/MusicTab.test.tsx", import.meta.url),
  },
  {
    capability: "转场选择与应用",
    owner: "TransitionTab",
    source: new URL("../media/TransitionTab.tsx", import.meta.url),
    liveBoundary: "setTransition",
    visibleEvidence: new URL("../media/TransitionTab.test.tsx", import.meta.url),
  },
  {
    capability: "画布变换覆盖层",
    owner: "TransformOverlay",
    source: new URL("../preview/TransformOverlay.tsx", import.meta.url),
    liveBoundary: "edit.setTransformAtFrame",
    visibleEvidence: new URL("../preview/TransformOverlay.interaction.test.tsx", import.meta.url),
  },
  {
    capability: "画布裁剪覆盖层",
    owner: "CropOverlay",
    source: new URL("../preview/CropOverlay.tsx", import.meta.url),
    liveBoundary: "edit.setClipProperties",
    visibleEvidence: new URL("../../lib/cropOverlay.test.ts", import.meta.url),
  },
];

describe("ShellComponentMapping", () => {
  it("every_documented_shell_component_has_exact_owner", () => {
    const repositoryRoot = fileURLToPath(new URL("../../../../", import.meta.url));
    const componentMap = readFileSync(
      new URL("../../../../docs/specs/frontend/3-components.md", import.meta.url),
      "utf8",
    );

    for (const entry of exactOwners) {
      const source = readFileSync(entry.source, "utf8");
      const evidence = readFileSync(entry.visibleEvidence, "utf8");
      const relativeSource = relative(repositoryRoot, fileURLToPath(entry.source))
        .split(sep)
        .join("/");

      expect(componentMap, `${entry.capability} is missing from the component map`).toContain(
        `| ${entry.capability} |`,
      );
      expect(componentMap, `${entry.owner} does not have one exact documented source`).toContain(
        `\`${entry.owner}\` → \`${relativeSource}\``,
      );
      expect(source).toContain(`export function ${entry.owner}`);
      expect(source).toContain(entry.liveBoundary);
      expect(evidence).toMatch(/describe\(|it\(/);
    }

    const transitionCanvasEvidence = readFileSync(
      new URL("../timeline/timelineOverlays.test.ts", import.meta.url),
      "utf8",
    );
    expect(transitionCanvasEvidence).toContain(
      "paints a cut marker for a valid adjacent cross dissolve",
    );
  });
});
