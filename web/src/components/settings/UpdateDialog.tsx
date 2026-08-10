import { useEffect, useRef, useState, type CSSProperties } from "react";
import { ExternalLink } from "lucide-react";
import { useT } from "../../i18n";
import { openUpdateReleases } from "../../lib/api";
import {
  isUpdateInstallationBlocking,
  useUpdateStore,
  type UpdatePhase,
} from "../../store/updateStore";
import { Icon } from "../ui/Icon";

interface UpdateDialogProps {
  phase: UpdatePhase;
  version: string;
  notes: string | null;
  progress: number | null;
  error: string | null;
  onInstall: () => void;
  onClose: () => void;
}

const actionStyle: CSSProperties = {
  minWidth: 82,
  height: 30,
  padding: "0 var(--space-md)",
  borderRadius: "var(--radius-sm)",
  fontSize: "var(--fs-sm)",
  fontWeight: "var(--fw-semibold)",
};

function phaseLabel(phase: UpdatePhase, t: ReturnType<typeof useT>): string {
  if (phase === "checking") return t("update.checking");
  if (phase === "downloading") return t("update.downloading");
  if (phase === "installing") return t("update.installing");
  if (phase === "restarting") return t("update.restarting");
  if (phase === "upToDate") return t("update.upToDate");
  if (phase === "error") return t("update.error");
  return t("update.available");
}

export function UpdateDialog({
  phase,
  version,
  notes,
  progress,
  error,
  onInstall,
  onClose,
}: UpdateDialogProps) {
  const t = useT();
  const dialogRef = useRef<HTMLDivElement>(null);
  const [releaseOpenError, setReleaseOpenError] = useState(false);
  const busy = phase === "checking" || phase === "closing" || phase === "downloading" || phase === "installing" || phase === "restarting";

  useEffect(() => {
    dialogRef.current?.focus({ preventScroll: true });
    const onKeyDown = (event: KeyboardEvent) => {
      if (isUpdateInstallationBlocking(phase)) {
        event.preventDefault();
        event.stopImmediatePropagation();
        return;
      }
      if (event.key === "Escape" && !busy) {
        event.preventDefault();
        event.stopImmediatePropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [busy, onClose, phase]);

  return (
    <div
      role="presentation"
      className="app-dialog-backdrop"
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1200,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: "var(--space-xl)",
        background: "rgba(0, 0, 0, 0.65)",
        backdropFilter: "blur(12px)",
      }}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="update-dialog-title"
        tabIndex={-1}
        className="app-dialog-surface"
        style={{
          width: "min(440px, 100%)",
          padding: "var(--space-xl)",
          borderRadius: "var(--radius-lg)",
          background: "var(--bg-elevated)",
          boxShadow: "var(--shadow-lg)",
          color: "var(--text-primary)",
        }}
      >
        <h2 id="update-dialog-title" style={{ margin: 0, fontSize: "var(--fs-lg)" }}>
          {phaseLabel(phase, t)}
        </h2>

        {version && (
          <p className="tabular" style={{ margin: "var(--space-sm) 0 0", color: "var(--text-secondary)" }}>
            {t("update.version")} {version}
          </p>
        )}
        {phase === "available" && (
          <p style={{ color: "var(--text-secondary)" }}>{t("update.availableBody")}</p>
        )}
        {notes && (
          <div style={{ whiteSpace: "pre-wrap", maxHeight: 160, overflowY: "auto", fontSize: "var(--fs-sm)", color: "var(--text-secondary)" }}>
            {notes}
          </div>
        )}
        {phase === "error" && error && (
          <p role="alert" style={{ color: "var(--status-error)", overflowWrap: "anywhere" }}>
            {error}
          </p>
        )}
        {phase === "error" && (
          <button
            type="button"
            onClick={async () => {
              setReleaseOpenError(false);
              try {
                await openUpdateReleases();
              } catch {
                setReleaseOpenError(true);
              }
            }}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "var(--space-xs)",
              padding: 0,
              border: 0,
              background: "transparent",
              color: "var(--accent-primary)",
              fontSize: "var(--fs-sm)",
              cursor: "pointer",
            }}
          >
            {t("update.releases")}
            <Icon icon={ExternalLink} size={13} />
          </button>
        )}
        {phase === "error" && releaseOpenError && (
          <p role="alert" style={{ color: "var(--status-error)", fontSize: "var(--fs-sm)" }}>
            {t("update.releasesOpenFailed")}
          </p>
        )}
        {(phase === "downloading" || phase === "installing") && (
          <div style={{ marginTop: "var(--space-lg)" }}>
            <progress
              role="progressbar"
              max={100}
              value={progress ?? undefined}
              aria-valuenow={progress ?? undefined}
              aria-label={t("update.progress")}
              style={{ width: "100%" }}
            />
            {progress !== null && (
              <div className="tabular" style={{ marginTop: "var(--space-xs)", textAlign: "right", fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
                {progress}%
              </div>
            )}
          </div>
        )}

        <div style={{ display: "flex", justifyContent: "flex-end", gap: "var(--space-sm)", marginTop: "var(--space-xl)" }}>
          <button
            type="button"
            disabled={busy}
            onClick={onClose}
            className="hover-area"
            style={{ ...actionStyle, background: "var(--home-hover)", color: "var(--text-primary)", opacity: busy ? 0.45 : 1 }}
          >
            {t("update.close")}
          </button>
          {phase === "available" && (
            <button
              type="button"
              onClick={onInstall}
              className="hover-area"
              style={{ ...actionStyle, background: "var(--accent-primary)", color: "white" }}
            >
              {t("update.install")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export function UpdateCenter() {
  const dialogOpen = useUpdateStore((state) => state.dialogOpen);
  const phase = useUpdateStore((state) => state.phase);
  const update = useUpdateStore((state) => state.update);
  const progress = useUpdateStore((state) => state.progress);
  const error = useUpdateStore((state) => state.error);
  const install = useUpdateStore((state) => state.install);
  const dismiss = useUpdateStore((state) => state.dismiss);

  if (!dialogOpen) return null;
  return (
    <UpdateDialog
      phase={phase}
      version={update?.version ?? ""}
      notes={update?.notes ?? null}
      progress={progress}
      error={error}
      onInstall={() => void install()}
      onClose={() => void dismiss()}
    />
  );
}

export function UpdateSettingsControl() {
  const t = useT();
  const phase = useUpdateStore((state) => state.phase);
  const check = useUpdateStore((state) => state.check);
  const busy = phase === "checking" || phase === "closing" || phase === "downloading" || phase === "installing" || phase === "restarting";
  const status = phase === "idle" ? t("update.aboutStatus") : phaseLabel(phase, t);

  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--space-md)" }}>
      <span style={{ color: "var(--text-secondary)", fontSize: "var(--fs-sm)" }}>{status}</span>
      <button
        type="button"
        disabled={busy}
        onClick={() => void check("manual")}
        className="hover-area"
        style={{ ...actionStyle, background: "var(--home-hover)", color: "var(--text-primary)", opacity: busy ? 0.45 : 1 }}
      >
        {phase === "checking" ? t("update.checking") : t("update.check")}
      </button>
    </div>
  );
}
