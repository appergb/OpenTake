/**
 * Import settings pane — default import folder.
 * Extracted from SettingsView.tsx (Issue #40 review).
 */

import { FolderOpen } from "lucide-react";
import { Icon } from "../../ui/Icon";
import { useT } from "../../../i18n";
import { useSettingsStore } from "../../../store/settingsStore";
import { openDialog } from "../../../lib/dialog";
import { Section, Field } from "../shared";

export function ImportPane() {
  const t = useT();
  const folder = useSettingsStore((s) => s.defaultImportFolder);
  const setFolder = useSettingsStore((s) => s.setDefaultImportFolder);

  const choose = async () => {
    const open = await openDialog();
    if (!open) return;
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") setFolder(selected);
  };

  return (
    <Section title={t("settings.section.import")}>
      <Field
        label={t("settings.defaultImportFolder")}
        description={folder ?? t("settings.notSet")}
        control={
          <div style={{ display: "inline-flex", gap: "var(--space-xs)" }}>
            <button
              type="button"
              onClick={() => void choose()}
              className="hover-area"
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 4,
                height: 26,
                padding: "0 var(--space-md)",
                borderRadius: "var(--radius-sm)",
                border: "var(--bw-thin) solid var(--border-primary)",
                color: "var(--text-secondary)",
                fontSize: "var(--fs-sm)",
                fontWeight: "var(--fw-medium)",
              }}
            >
              <Icon icon={FolderOpen} size={13} />
              {t("settings.chooseFolder")}
            </button>
            {folder && (
              <button
                type="button"
                onClick={() => setFolder(null)}
                className="hover-area"
                style={{
                  height: 26,
                  padding: "0 var(--space-md)",
                  borderRadius: "var(--radius-sm)",
                  color: "var(--text-tertiary)",
                  fontSize: "var(--fs-sm)",
                }}
              >
                {t("settings.clear")}
              </button>
            )}
          </div>
        }
      />
    </Section>
  );
}