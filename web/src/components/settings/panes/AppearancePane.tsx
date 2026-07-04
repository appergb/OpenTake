/**
 * Appearance settings pane — theme toggle.
 * Extracted from SettingsView.tsx (Issue #40 review).
 */

import { useT } from "../../../i18n";
import { useSettingsStore, type Theme } from "../../../store/settingsStore";
import { Section, Field, Segmented } from "../shared";

export function AppearancePane() {
  const t = useT();
  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);
  return (
    <Section title={t("settings.section.appearance")}>
      <Field
        label={t("settings.theme")}
        description={t("settings.themeDesc")}
        control={
          <Segmented<Theme>
            value={theme}
            options={[
              { id: "dark", label: t("settings.theme.dark") },
              { id: "light", label: t("settings.theme.light") },
            ]}
            onChange={setTheme}
          />
        }
      />
    </Section>
  );
}