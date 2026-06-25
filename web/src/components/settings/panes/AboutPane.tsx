/**
 * About settings pane — version + license info.
 * Extracted from SettingsView.tsx (Issue #40 review).
 */

import { useT } from "../../../i18n";
import { Section, Field, Value } from "../shared";

export function AboutPane() {
  const t = useT();
  return (
    <Section title={t("settings.section.about")}>
      <Field label={t("settings.aboutVersion")} control={<Value>{__APP_VERSION__}</Value>} />
      <Field label={t("settings.aboutLicense")} control={<Value>GPL-3.0</Value>} />
      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
        {t("settings.aboutDesc")}
      </div>
    </Section>
  );
}