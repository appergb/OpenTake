/**
 * General settings pane — language selection.
 * Extracted from SettingsView.tsx (Issue #40 review).
 */

import { useT, useI18nStore, LOCALES } from "../../../i18n";
import { Dropdown } from "../../ui/Dropdown";
import { Section, Field } from "../shared";

export function GeneralPane() {
  const t = useT();
  const locale = useI18nStore((s) => s.locale);
  const setLocale = useI18nStore((s) => s.setLocale);
  return (
    <Section title={t("settings.section.general")}>
      <Field
        label={t("settings.language")}
        description={t("settings.languageDesc")}
        control={
          <Dropdown
            value={locale}
            options={LOCALES}
            onChange={setLocale}
            ariaLabel={t("settings.language")}
          />
        }
      />
    </Section>
  );
}