// @vitest-environment happy-dom

import { beforeEach, expect, it, vi } from "vitest";

async function freshRuntime(persisted?: string) {
  vi.resetModules();
  localStorage.clear();
  if (persisted !== undefined) localStorage.setItem("locale", persisted);
  document.documentElement.lang = "";
  return import("./index");
}

beforeEach(() => {
  localStorage.clear();
  document.documentElement.lang = "";
});

it("defaults_zh_cn_supports_en_and_preserves_unknown_named_placeholders", async () => {
  const defaults = await freshRuntime();
  expect(defaults.useI18nStore.getState().locale).toBe("zh-CN");
  defaults.initI18n();
  expect(document.documentElement.lang).toBe("zh-CN");
  expect(defaults.t("home.samples")).toBe("示例项目");
  expect(defaults.t("missing.translation.key")).toBe("missing.translation.key");
  expect(defaults.t("export.done", { width: 1920, height: "1080" })).toContain("1920×1080");
  expect(defaults.t("export.done", { width: 1920 })).toContain("{height}");
  expect(defaults.t("export.done", { width: 1920 })).toContain("{frames}");

  defaults.useI18nStore.getState().setLocale("en");
  expect(localStorage.getItem("locale")).toBe("en");
  expect(document.documentElement.lang).toBe("en");
  expect(defaults.t("home.samples")).toBe("Sample projects");

  const restored = await freshRuntime("en");
  expect(restored.useI18nStore.getState().locale).toBe("en");
  restored.initI18n();
  expect(document.documentElement.lang).toBe("en");

  const invalid = await freshRuntime("fr-CA");
  expect(invalid.useI18nStore.getState().locale).toBe("zh-CN");
  expect(localStorage.getItem("locale")).toBeNull();
});
