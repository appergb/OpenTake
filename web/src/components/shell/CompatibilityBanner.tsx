import { useT } from "../../i18n";
import { useProjectStore } from "../../store/projectStore";

export function CompatibilityBanner() {
  const readOnly = useProjectStore((state) => state.compatibilityReadOnly);
  const blockerCount = useProjectStore((state) => state.compatibilityBlockers.length);
  const t = useT();

  if (!readOnly) return null;

  return (
    <div
      role="status"
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        padding: "var(--space-sm) var(--space-md)",
        background: "var(--bg-raised)",
        borderBottom: "var(--bw-thin) solid var(--status-warning)",
        color: "var(--text-primary)",
        fontSize: "var(--fs-sm)",
      }}
    >
      <strong>{t("compatibility.title")}</strong>
      <span>{t("compatibility.description")}</span>
      {blockerCount > 0 && (
        <span style={{ color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
          {t(
            blockerCount === 1
              ? "compatibility.blockerCount.one"
              : "compatibility.blockerCount.many",
            { count: blockerCount },
          )}
        </span>
      )}
    </div>
  );
}
