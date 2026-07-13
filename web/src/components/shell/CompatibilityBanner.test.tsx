import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

const bannerState = vi.hoisted(() => ({
  locale: "zh-CN" as "zh-CN" | "en",
  readOnly: false,
  blockers: [] as string[],
}));

vi.mock("../../store/projectStore", () => ({
  useProjectStore: (selector: (state: object) => unknown) =>
    selector({
      compatibilityReadOnly: bannerState.readOnly,
      compatibilityBlockers: bannerState.blockers,
    }),
}));

vi.mock("../../i18n", async () => {
  const { DICTS } = await import("../../i18n/dict");
  return {
    useT: () => (key: string, vars?: Record<string, string | number>) => {
      const template = DICTS[bannerState.locale][key] ?? key;
      return template.replace(/\{(\w+)\}/g, (_match, name: string) =>
        vars && name in vars ? String(vars[name]) : `{${name}}`,
      );
    },
  };
});

import { CompatibilityBanner } from "./CompatibilityBanner";

describe("CompatibilityBanner", () => {
  beforeEach(() => {
    bannerState.locale = "zh-CN";
    bannerState.readOnly = false;
    bannerState.blockers = [];
  });

  it("explains the inspectable read-only mode in Chinese without exposing blocker paths", () => {
    bannerState.readOnly = true;
    bannerState.blockers = [
      "timeline.tracks[0].futureField",
      "/Volumes/Secret/project.opentake/manifest.json",
    ];

    const html = renderToStaticMarkup(<CompatibilityBanner />);

    expect(html).toContain("兼容性只读模式");
    expect(html).toContain("仍可查看项目内容");
    expect(html).toContain("2 个兼容性问题");
    expect(html).not.toContain("futureField");
    expect(html).not.toContain("/Volumes/Secret");
  });

  it("explains the inspectable read-only mode in English", () => {
    bannerState.locale = "en";
    bannerState.readOnly = true;
    bannerState.blockers = ["manifest.futureField"];

    const html = renderToStaticMarkup(<CompatibilityBanner />);

    expect(html).toContain("Compatibility read-only mode");
    expect(html).toContain("You can still inspect the project");
    expect(html).toContain("1 compatibility issue");
  });

  it("does not render for a writable project", () => {
    expect(renderToStaticMarkup(<CompatibilityBanner />)).toBe("");
  });
});
