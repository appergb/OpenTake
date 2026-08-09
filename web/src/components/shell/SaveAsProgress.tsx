import { cancelSaveAsMedia } from "../../store/editActions";
import { useEditorUiStore, type SaveAsProgressState } from "../../store/uiStore";
import { useT } from "../../i18n";

export function SaveAsProgress() {
  const progress = useEditorUiStore((state) => state.saveAsProgress);
  if (!progress) return null;
  return <SaveAsProgressView progress={progress} />;
}

export function SaveAsProgressView({ progress }: { progress: SaveAsProgressState }) {
  const t = useT();
  const fraction = Math.max(0, Math.min(1, progress.done / Math.max(1, progress.total)));

  return (
    <div
      role="status"
      aria-label={progress.label}
      className="app-toast"
      style={{
        position: "fixed",
        right: 24,
        bottom: 24,
        zIndex: 10000,
        width: 300,
        padding: 12,
        display: "flex",
        flexDirection: "column",
        gap: 8,
        background: "var(--bg-elevated)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: "var(--radius-md)",
        boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span style={{ flex: 1, fontSize: "var(--fs-sm)", color: "var(--text-primary)" }}>
          {progress.label}
        </span>
        <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {Math.round(fraction * 100)}%
        </span>
      </div>
      <progress value={progress.done} max={Math.max(1, progress.total)} style={{ width: "100%" }} />
      <button
        type="button"
        disabled={!progress.cancellable || progress.cancelling}
        onClick={() => void cancelSaveAsMedia()}
        style={{
          alignSelf: "flex-end",
          minWidth: 72,
          height: 26,
          padding: "0 10px",
          borderRadius: "var(--radius-sm)",
          background: "var(--bg-raised)",
          color: "var(--text-primary)",
          opacity: !progress.cancellable || progress.cancelling ? 0.55 : 1,
        }}
      >
        {progress.cancelling
          ? t("saveAs.cancelling")
          : progress.cancellable
            ? t("saveAs.cancel")
            : t("saveAs.preparing")}
      </button>
    </div>
  );
}
