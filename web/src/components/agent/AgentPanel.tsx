/**
 * Agent panel (SPEC §2.1 — belongs to a separate Issue; here only its layout
 * column is reserved). Collapsed by default. Scaffolded.
 */

import { useT } from "../../i18n";

export function AgentPanel() {
  const t = useT();
  return (
    <div
      style={{
        height: "100%",
        width: "100%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: "var(--text-muted)",
        fontSize: "var(--fs-sm)",
        textAlign: "center",
        padding: "var(--space-lg)",
      }}
    >
      {t("agent.placeholder")}
    </div>
  );
}
