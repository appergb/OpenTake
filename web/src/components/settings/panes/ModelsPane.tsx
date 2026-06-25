/**
 * Models pane — AI model selection (Issue #40 review #2).
 * Hardcoded model list per provider; persisted to settingsStore.
 */

import { useT } from "../../../i18n";
import { useSettingsStore } from "../../../store/settingsStore";
import { Section, Field, Segmented } from "../shared";

const MODELS: Record<string, Array<{ id: string; label: string }>> = {
  anthropic: [
    { id: "claude-sonnet-4-20250514", label: "Claude Sonnet 4" },
    { id: "claude-opus-4-20250514", label: "Claude Opus 4" },
  ],
  openai: [
    { id: "gpt-4o", label: "GPT-4o" },
    { id: "gpt-4o-mini", label: "GPT-4o mini" },
  ],
  google: [
    { id: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
    { id: "gemini-2.0-flash", label: "Gemini 2.0 Flash" },
  ],
};

export function ModelsPane() {
  const t = useT();
  const provider = useSettingsStore((s) => s.byokProvider);
  const model = useSettingsStore((s) => s.byokModel);
  const setModel = useSettingsStore((s) => s.setByokModel);

  const options = MODELS[provider] ?? MODELS.anthropic;
  const current = model || (options[0]?.id ?? "");

  return (
    <Section title={t("settings.section.models")}>
      <div style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-tertiary)" }}>
        {t("settings.modelsDesc")}
      </div>
      <Field
        label={t("settings.modelsProviderModel")}
        description={t("settings.modelsModelDesc")}
        control={
          <Segmented<string>
            value={current}
            options={options}
            onChange={setModel}
          />
        }
      />
    </Section>
  );
}