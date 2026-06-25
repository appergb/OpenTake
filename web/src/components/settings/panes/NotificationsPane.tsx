/**
 * Notifications pane. Generation-complete notification toggle.
 *
 * Extracted from SettingsView.tsx (Issue #40 review). Fix:
 * - Toggle now persists to settingsStore (localStorage) instead of
 *   local useState (review #7 — "开关未持久化").
 */

import { useT } from "../../../i18n";
import { useSettingsStore } from "../../../store/settingsStore";
import { Section, Field } from "../shared";

export function NotificationsPane() {
  const t = useT();
  const enabled = useSettingsStore((s) => s.notificationsGeneration);
  const setEnabled = useSettingsStore((s) => s.setNotificationsGeneration);

  return (
    <Section title={t("settings.section.notifications")}>
      <Field
        label={t("notifications.generation")}
        description={t("notifications.generationDesc")}
        control={
          <button
            type="button"
            role="switch"
            aria-checked={enabled}
            onClick={() => setEnabled(!enabled)}
            style={{
              width: 36,
              height: 20,
              borderRadius: 10,
              background: enabled ? "var(--accent-primary)" : "var(--bg-base)",
              border: "var(--bw-thin) solid var(--border-primary)",
              position: "relative",
              transition: "background var(--anim-transition) var(--ease-out)",
            }}
          >
            <span
              style={{
                position: "absolute",
                top: 1,
                left: enabled ? 17 : 1,
                width: 16,
                height: 16,
                borderRadius: "50%",
                background: "var(--text-primary)",
                transition: "left var(--anim-transition) var(--ease-out)",
              }}
            />
          </button>
        }
      />
      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-muted)" }}>
        {t("notifications.restartHint")}
      </div>
    </Section>
  );
}