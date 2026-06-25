/**
 * Shortcuts pane — read-only keyboard shortcut reference (Issue #40 review #2).
 * Displays common shortcuts using Field components with <kbd> tags.
 */

import { useT } from "../../../i18n";
import { Section, Field } from "../shared";

const KBD: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  height: 22,
  minWidth: 22,
  padding: "0 6px",
  background: "var(--bg-base)",
  border: "var(--bw-thin) solid var(--border-primary)",
  borderRadius: "var(--radius-xs)",
  fontSize: "var(--fs-xs)",
  fontFamily: "var(--font-mono, ui-monospace, monospace)",
  color: "var(--text-secondary)",
  whiteSpace: "nowrap",
};

function Kbd({ children }: { children: string }) {
  return <span style={KBD}>{children}</span>;
}

function KbdGroup({ keys }: { keys: string[] }) {
  return (
    <span style={{ display: "inline-flex", gap: 4, alignItems: "center" }}>
      {keys.map((k, i) => (
        <span key={i} style={{ display: "inline-flex", gap: 4, alignItems: "center" }}>
          {i > 0 && <span style={{ color: "var(--text-muted)", fontSize: "var(--fs-xs)" }}>+</span>}
          <Kbd>{k}</Kbd>
        </span>
      ))}
    </span>
  );
}

export function ShortcutsPane() {
  const t = useT();

  const shortcuts = [
    { label: t("settings.shortcutsPlay"), keys: ["Space"] },
    { label: t("settings.shortcutsUndo"), keys: ["Ctrl", "Z"] },
    { label: t("settings.shortcutsRedo"), keys: ["Ctrl", "Shift", "Z"] },
    { label: t("settings.shortcutsDelete"), keys: ["Delete"] },
    { label: t("settings.shortcutsSave"), keys: ["Ctrl", "S"] },
    { label: t("settings.shortcutsNew"), keys: ["Ctrl", "N"] },
  ];

  return (
    <Section title={t("settings.section.shortcuts")}>
      {shortcuts.map((s) => (
        <Field
          key={s.label}
          label={s.label}
          control={<KbdGroup keys={s.keys} />}
        />
      ))}
    </Section>
  );
}