/**
 * Storage pane. Surfaces cache location and a clear-cache action.
 *
 * Extracted from SettingsView.tsx (Issue #40 review). Fix:
 * - Clear cache button now shows "unavailable" feedback instead of
 *   fake success (review #6 — "清缓存是 no-op 却显示成功").
 */

import { useState } from "react";
import { useT } from "../../../i18n";
import { Section, Field } from "../shared";

export function StoragePane() {
  const t = useT();
  const [showUnavailable, setShowUnavailable] = useState(false);

  const clear = () => {
    // Fix #6: Instead of fake success, surface that the feature is not yet
    // available (the cache-clear Tauri command hasn't been wired).
    setShowUnavailable(true);
    setTimeout(() => setShowUnavailable(false), 3000);
  };

  return (
    <Section title={t("settings.section.storage")}>
      <Field
        label={t("storage.cache")}
        description={t("storage.cacheDesc")}
        control={
          <button
            type="button"
            onClick={clear}
            className="hover-area"
            style={{
              height: 26,
              padding: "0 var(--space-md)",
              borderRadius: "var(--radius-sm)",
              border: "var(--bw-thin) solid var(--border-primary)",
              color: "var(--text-secondary)",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
            }}
          >
            {t("storage.clearCache")}
          </button>
        }
      />
      {showUnavailable && (
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("storage.clearCacheUnavailable")}
        </div>
      )}
      <Field
        label={t("storage.searchIndex")}
        description={t("storage.searchIndexDesc")}
        control={<span style={{ fontSize: "var(--fs-sm)", color: "var(--text-muted)" }}>—</span>}
      />
      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-muted)" }}>
        {t("storage.placeholder")}
      </div>
    </Section>
  );
}