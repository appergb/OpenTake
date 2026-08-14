import { describe, expect, it } from "vitest";
import { motionKeyframeNames } from "./MotionTimeline";

describe("Motion timeline CSS keyframe discovery", () => {
  it("uses parsed CSS, ignores comments and strings, and bounds unique results", () => {
    const declarations = Array.from(
      { length: 40 },
      (_, index) => `@keyframes 动画${index} { from { opacity: 0; } }`,
    ).join("\n");
    const css = `
      /* @keyframes commented { from { opacity: 0; } } */
      .label { content: "@keyframes quoted { from {} }"; }
      @keyframes 动画0 { to { opacity: 1; } }
      ${declarations}
    `;

    const names = motionKeyframeNames(css);

    expect(names).toHaveLength(24);
    expect(names[0]).toBe("动画0");
    expect(new Set(names).size).toBe(24);
    expect(names).not.toContain("commented");
    expect(names).not.toContain("quoted");
  });
});
