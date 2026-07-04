/**
 * Privacy pane — telemetry toggles (Issue #40 review #2).
 * Two switches: anonymous usage telemetry + crash reports.
 * Both default off; persisted to settingsStore (localStorage).
 */

import { useT } from "../../../i18n";
import { useSettingsStore } from "../../../store/settingsStore";
import { Section, Field } from "../shared";

export function PrivacyPane() {
  const t = useT();
  const usage = useSettingsStore((s) => s.telemetryUsage);
  const crash = useSettingsStore((s) => s.telemetryCrash);
  const setUsage = useSettingsStore((s) => s.setTelemetryUsage);
  const setCrash = useSettingsStore((s) => s.setTelemetryCrash);

  const toggleStyle = (enabled: boolean) => ({
    width: 36,
    height: 20,
    borderRadius: 10,
    background: enabled ? "var(--accent-primary)" : "var(--bg-base)",
    border: "var(--bw-thin) solid var(--border-primary)",
    position: "relative" as const,
    transition: "background var(--anim-transition) var(--ease-out)",
  });

  const knobStyle = (enabled: boolean) => ({
    position: "absolute" as const,
    top: 1,
    left: enabled ? 17 : 1,
    width: 16,
    height: 16,
    borderRadius: "50%",
    background: "var(--text-primary)",
    transition: "left var(--anim-transition) var(--ease-out)",
  });

  return (
    <Section title={t("settings.section.privacy")}>
      <Field
        label={t("settings.privacyTelemetry")}
        description={t("settings.privacyTelemetryDesc")}
        control={
          <button
            type="button"
            role="switch"
            aria-checked={usage}
            onClick={() => setUsage(!usage)}
            style={toggleStyle(usage)}
          >
            <span style={knobStyle(usage)} />
          </button>
        }
      />
      <Field
        label={t("settings.privacyCrash")}
        description={t("settings.privacyCrashDesc")}
        control={
          <button
            type="button"
            role="switch"
            aria-checked={crash}
            onClick={() => setCrash(!crash)}
            style={toggleStyle(crash)}
          >
            <span style={knobStyle(crash)} />
          </button>
        }
      />
    </Section>
  );
}