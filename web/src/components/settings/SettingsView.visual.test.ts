import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const settingsSource = readFileSync(new URL("./SettingsView.tsx", import.meta.url), "utf8");

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

  it("keeps settings controls restrained instead of heavy bordered blocks", () => {
    expect(settingsSource).toContain("settingsControlStyle");
    expect(settingsSource).toContain("background: \"var(--home-hover)\"");
  });

  it("uses a wide settings window with a left sidebar", () => {
    expect(settingsSource).toContain("width: 960");
    expect(settingsSource).toContain("height: 620");
    expect(settingsSource).toContain("SettingsSidebar");
    expect(settingsSource).toContain("settingsSidebarStyle");
  });

  it("renders one active settings pane instead of stacking every section", () => {
    expect(settingsSource).toContain("SETTINGS_PANES");
    expect(settingsSource).toContain("activePane");
    expect(settingsSource).toContain("renderActivePane");
    expect(settingsSource).not.toContain("<GeneralPane />\n            <AppearancePane />\n            <ImportPane />\n            <AiPane />\n            <AboutPane />");
  });
});
