import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./AccountPane.tsx", import.meta.url), "utf8");

describe("AccountPane credential boundaries", () => {
  it("keeps login optional and uses only the dedicated account API", () => {
    expect(source).toContain('t("account.disclaimer")');
    expect(source).toContain("accountSetBackendUrl");
    expect(source).toContain("accountLogin");
    expect(source).toContain("accountLogout");
    expect(source).not.toContain("localStorage");
  });

  it("treats the token as a password and never asks the backend for it", () => {
    expect(source).toContain('type="password"');
    expect(source).toContain('autoComplete="off"');
    expect(source).not.toContain("accountGetToken");
  });
});
