import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

const cssSource = readFileSync(new URL("./tokens.css", import.meta.url), "utf8");

function cssColorToken(name: string): string | undefined {
  const value = cssSource.match(new RegExp(`--${name}\\s*:\\s*([^;]+);`))?.[1];
  return value?.replace(/\s+/g, "").trim();
}

it("bg_placeholder_equals_raised_rgb_30", () => {
  const raised = cssColorToken("bg-raised");
  const placeholder = cssColorToken("bg-placeholder");

  expect(raised).toBe("rgb(30,30,30)");
  expect(placeholder).toBe("rgb(30,30,30)");
  expect(placeholder).toBe(raised);
});
