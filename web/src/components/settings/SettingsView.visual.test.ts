// @vitest-environment happy-dom

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { useI18nStore } from "../../i18n";
import { useSettingsStore } from "../../store/settingsStore";
import { useEditorUiStore } from "../../store/uiStore";
import { SettingsView } from "./SettingsView";

const settingsSource = readFileSync(
  resolve(process.cwd(), "src/components/settings/SettingsView.tsx"),
  "utf8",
);

let container: HTMLDivElement | null = null;
let root: Root | null = null;

function expectZeroLength(value: string) {
  expect(["0", "0px"]).toContain(value);
}

function appearanceChoices(windowSize: "standard" | "compact"): Array<{
  text: string;
  flex: string;
  labelStyle: string;
  svgCount: number;
}> {
  useI18nStore.setState({ locale: "zh-CN" });
  useEditorUiStore.setState({ settingsPane: "appearance" });
  useSettingsStore.setState({ windowSize });
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(createElement(SettingsView)));
  const choices = [...container.querySelectorAll<HTMLElement>('[role="radio"]')].map((choice) => ({
    text: choice.textContent?.trim() ?? "",
    flex: choice.style.flex,
    labelStyle: choice.querySelector<HTMLElement>("span")?.getAttribute("style") ?? "",
    svgCount: choice.querySelectorAll("svg").length,
  }));
  act(() => root.unmount());
  container.remove();
  return choices;
}

function renderSettings(pane: "general" | "appearance" = "general") {
  useI18nStore.setState({ locale: "zh-CN" });
  useEditorUiStore.setState({ settingsPane: pane });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  act(() => root?.render(createElement(SettingsView)));
  return container.querySelector<HTMLElement>('[role="dialog"]')!;
}

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

afterEach(() => {
  act(() => root?.unmount());
  container?.remove();
  root = null;
  container = null;
});

describe("SettingsView minimal embedded visual direction", () => {
  it("uses one unified settings surface without header divider", () => {
    expect(settingsSource).toContain("settingsPanelStyle");
    expect(settingsSource).not.toContain("borderBottom");
  });

  it("does not render sections as bordered raised cards", () => {
    expect(settingsSource).toContain("settingsSectionStyle");
    expect(settingsSource).not.toContain("background: \"var(--bg-raised)\",");
    expect(settingsSource).not.toContain("border: \"var(--bw-thin) solid var(--border-primary)\",");
  });

  it("keeps dark layout choices equal-sized with stable label geometry", () => {
    const standard = appearanceChoices("standard");
    const compact = appearanceChoices("compact");

    expect(standard).toHaveLength(2);
    expect(standard.map((choice) => choice.text)).toEqual([
      "深色 · 标准",
      "深色 · 紧凑",
    ]);
    expect(standard.map((choice) => choice.flex)).toEqual(["1 1 0px", "1 1 0px"]);
    expect(standard.map((choice) => choice.svgCount)).toEqual([0, 0]);
    expect(standard.map((choice) => choice.labelStyle)).toEqual(
      compact.map((choice) => choice.labelStyle),
    );
  });

  it("uses a wide settings window with a left sidebar", () => {
    const dialog = renderSettings();
    const surface = dialog as HTMLElement;
    const bodyRow = surface.querySelector("header + div") as HTMLElement;
    const sidebar = surface.querySelector("nav") as HTMLElement;
    const content = sidebar.nextElementSibling as HTMLElement;
    const surfaceStyle = surface.getAttribute("style") ?? "";

    expect(settingsSource).toContain('width: "min(960px, 100%)"');
    expect(surface.style.height).toBe("100%");
    expect(surface.style.maxWidth).toBe("960px");
    expect(surfaceStyle).toMatch(/max-height:\s*min\([^,]+,\s*100%\)/);
    expect(surface.style.maxHeight).toContain("min(");
    expect(surface.style.maxHeight).toContain("100%");
    expectZeroLength(surface.style.minWidth);
    expectZeroLength(surface.style.minHeight);
    expect(bodyRow.style.display).toBe("flex");
    expectZeroLength(bodyRow.style.minHeight);
    expect(settingsSource).toContain('width: "clamp(138px, 16vw, 150px)"');
    expect(sidebar.style.overflowY).toBe("auto");
    expect(content.style.flex).toBe("1 1 0%");
    expectZeroLength(content.style.minWidth);
    expect(content.style.overflowY).toBe("auto");
  });

  it("renders one active settings pane instead of stacking every section", () => {
    expect(settingsSource).toContain("SETTINGS_PANES");
    expect(settingsSource).toContain("activePane");
    expect(settingsSource).toContain("renderActivePane");
    expect(settingsSource).not.toContain("<GeneralPane />\n            <AppearancePane />\n            <ImportPane />\n            <AiPane />\n            <AboutPane />");
  });

  it("exposes the optional account scaffold as a separate settings pane", () => {
    expect(settingsSource).toContain('id: "account"');
    expect(settingsSource).toContain('labelKey: "settings.section.account"');
    expect(settingsSource).toContain("return <AccountPane />");
  });

  it("supports official Codex ChatGPT login without rendering an API-key field", () => {
    expect(settingsSource).toContain('{ id: "codex", label: "Codex / ChatGPT" }');
    expect(settingsSource).toContain("codexAuthStatus");
    expect(settingsSource).toContain("codexLoginStart");
    expect(settingsSource).toContain("codexLogout");
    expect(settingsSource).toContain("isCodex ? (");
  });

  it("registers the storage pane with its own sidebar entry", () => {
    expect(settingsSource).toContain('id: "storage"');
    expect(settingsSource).toContain('labelKey: "settings.section.storage"');
    expect(settingsSource).toContain("return <StoragePane />");
  });
});
