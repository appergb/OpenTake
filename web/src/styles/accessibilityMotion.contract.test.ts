import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./global.css", import.meta.url), "utf8");

describe("desktop accessibility and motion contract", () => {
  it("uses restrained entry/dialog/toast motion with a reduced-motion escape hatch", () => {
    expect(css).toContain("@keyframes app-view-enter");
    expect(css).toContain("@keyframes dialog-surface-enter");
    expect(css).toContain("@keyframes toast-enter");
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
    expect(css).toMatch(/\.app-dialog-backdrop\s*\{/);
    expect(css).toMatch(/\.app-dialog-surface\s*\{/);
  });

  it("keeps range inputs operable without suppressing their visible focus ring", () => {
    expect(css).toMatch(/input\[type="range"\][\s\S]*?min-height:\s*24px/);
    expect(css).not.toMatch(/\.zoom-slider\s*\{[^}]*outline:\s*none/);
  });
});
