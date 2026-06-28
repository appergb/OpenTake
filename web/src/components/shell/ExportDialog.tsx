/**
 * ExportDialog (SPEC §2.4 / #112). Modal shown from the title bar to render the
 * whole timeline to a real video file via the `export_video` backend command
 * (per-frame GPU composite → ffmpeg H.264 / .mp4 + AAC mux).
 *
 * Scope mirrors the backend's first cut:
 *  - Format: H.264 / .mp4 is the only wired path; H.265 / ProRes are shown
 *    disabled with a "not wired yet" note (the backend `resolve_preset` rejects
 *    them, so we never let the user pick one and hit a server error).
 *  - Resolution: 720p / 1080p / 4K short-edge presets. The default pre-selects
 *    the preset matching the timeline's own shorter edge so a standard project
 *    round-trips its native size; it falls back to 1080p (the backend default).
 *
 * The backend runs to completion with no progress callback, so this is a
 * deliberate "status + toast" surface — never a faked progress bar. While the
 * export runs the controls are disabled and the button reads "Exporting…";
 * success / failure both `pushToast` and close on success.
 */

import { useEffect, useMemo, useState } from "react";
import { X } from "lucide-react";
import { Icon } from "../ui/Icon";
import { Dropdown } from "../ui/Dropdown";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { useT } from "../../i18n";
import * as api from "../../lib/api";
import type { ExportCodec, ExportQuality } from "../../lib/api";
import { saveDialog } from "../../lib/dialog";

const MP4_EXT = "mp4";

/** Ensure a chosen path carries the `.mp4` extension (the H.264 container). */
export function withMp4Ext(path: string): string {
  return path.toLowerCase().endsWith(`.${MP4_EXT}`) ? path : `${path}.${MP4_EXT}`;
}

/**
 * Default export filename: the open project's base name with `.mp4`, falling
 * back to "Timeline.mp4" for an unsaved project. The bundle path ends in
 * `…/Name.opentake`, so strip the directory and the `.opentake` suffix.
 */
export function defaultMp4Name(projectPath: string | null): string {
  if (!projectPath) return `Timeline.${MP4_EXT}`;
  const base = projectPath.split(/[\\/]/).pop() ?? projectPath;
  const stem = base.replace(/\.opentake$/i, "");
  return `${stem || "Timeline"}.${MP4_EXT}`;
}

/** Pick the preset whose short edge best matches the timeline's shorter side. */
export function defaultQuality(width: number, height: number): ExportQuality {
  const shortEdge = Math.min(width, height);
  if (shortEdge >= 1620) return "4k"; // ≥ 1620 rounds to the 2160 bucket
  if (shortEdge <= 840) return "720p"; // ≤ 840 rounds to the 720 bucket
  return "1080p";
}

export function ExportDialog() {
  const t = useT();
  const open = useEditorUiStore((s) => s.exportDialogOpen);
  const setOpen = useEditorUiStore((s) => s.setExportDialogOpen);
  const pushToast = useEditorUiStore((s) => s.pushToast);
  const timeline = useProjectStore((s) => s.timeline);

  const [codec, setCodec] = useState<ExportCodec>("h264");
  const [quality, setQuality] = useState<ExportQuality>("1080p");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Re-seed the resolution default from the timeline each time the dialog opens.
  useEffect(() => {
    if (open) {
      setQuality(defaultQuality(timeline.width, timeline.height));
      setError(null);
    }
  }, [open, timeline.width, timeline.height]);

  // Close on Escape (ignored while an export is in flight so the run isn't
  // abandoned mid-encode from the user's point of view).
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) setOpen(false);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, busy, setOpen]);

  const codecOptions = useMemo(
    () => [
      { id: "h264" as const, label: t("export.codec.h264") },
      { id: "h265" as const, label: t("export.codec.h265"), disabled: true },
      { id: "prores" as const, label: t("export.codec.prores"), disabled: true },
    ],
    [t],
  );

  const qualityOptions = useMemo(
    () => [
      { id: "720p" as const, label: t("export.quality.720p") },
      { id: "1080p" as const, label: t("export.quality.1080p") },
      { id: "4k" as const, label: t("export.quality.4k") },
    ],
    [t],
  );

  if (!open) return null;

  async function onExport(): Promise<void> {
    if (busy) return;
    setError(null);

    const save = await saveDialog();
    if (!save) {
      // No native save panel (outside Tauri) — the export can't run here.
      pushToast(t("export.unavailable"));
      return;
    }
    const projectPath = useProjectStore.getState().projectPath;
    const dir = projectPath
      ? projectPath.replace(/[\\/][^\\/]*$/, "")
      : await api.getDefaultProjectDir().catch(() => "");
    const sep = dir && !dir.endsWith("/") ? "/" : "";
    const defaultPath = dir
      ? `${dir}${sep}${defaultMp4Name(projectPath)}`
      : undefined;

    const chosen = await save({
      title: t("export.saveDialog"),
      defaultPath,
      filters: [{ name: t("export.saveFilter"), extensions: [MP4_EXT] }],
    });
    if (typeof chosen !== "string") return; // cancelled

    setBusy(true);
    try {
      const summary = await api.exportVideo({
        outPath: withMp4Ext(chosen),
        codec,
        quality,
      });
      pushToast(
        t("export.done", {
          width: summary.width,
          height: summary.height,
          frames: summary.frameCount,
        }),
      );
      setOpen(false);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setError(message);
      pushToast(t("export.failed"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1100,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "rgba(0,0,0,0.5)",
      }}
      onClick={() => {
        if (!busy) setOpen(false);
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("export.title")}
        style={{
          width: 360,
          display: "flex",
          flexDirection: "column",
          background: "var(--bg-elevated)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: 8,
          boxShadow: "0 12px 32px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div
          style={{
            padding: "10px 14px",
            borderBottom: "var(--bw-thin) solid var(--border-primary)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
          }}
        >
          <span style={{ fontSize: "var(--fs-sm)", fontWeight: 600 }}>
            {t("export.title")}
          </span>
          <button
            type="button"
            disabled={busy}
            onClick={() => setOpen(false)}
            className="hover-area"
            aria-label={t("export.close")}
            style={{
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              width: 24,
              height: 24,
              background: "transparent",
              border: "none",
              color: "var(--text-secondary)",
              cursor: busy ? "default" : "pointer",
              opacity: busy ? 0.4 : 1,
            }}
          >
            <Icon icon={X} size={14} />
          </button>
        </div>

        {/* Body: format + resolution rows. */}
        <div
          style={{
            padding: "14px",
            display: "flex",
            flexDirection: "column",
            gap: "var(--space-md)",
          }}
        >
          <Row label={t("export.format")}>
            <Dropdown
              value={codec}
              options={codecOptions}
              onChange={(id) => setCodec(id)}
              ariaLabel={t("export.format")}
              minWidth={160}
            />
          </Row>

          <Row
            label={t("export.resolution")}
            hint={t("export.timelineSize", {
              width: timeline.width,
              height: timeline.height,
            })}
          >
            <Dropdown
              value={quality}
              options={qualityOptions}
              onChange={(id) => setQuality(id)}
              ariaLabel={t("export.resolution")}
              minWidth={160}
            />
          </Row>

          {error && (
            <div
              style={{
                fontSize: "var(--fs-xs)",
                color: "var(--accent-danger, #ff6b6b)",
                background: "rgba(255,107,107,0.08)",
                borderRadius: "var(--radius-xs-sm)",
                padding: "6px 8px",
                wordBreak: "break-word",
              }}
            >
              {error}
            </div>
          )}
        </div>

        {/* Footer: cancel + export. */}
        <div
          style={{
            padding: "10px 14px",
            borderTop: "var(--bw-thin) solid var(--border-primary)",
            display: "flex",
            alignItems: "center",
            justifyContent: "flex-end",
            gap: "var(--space-sm)",
          }}
        >
          <button
            type="button"
            disabled={busy}
            onClick={() => setOpen(false)}
            className="hover-area"
            style={{
              height: 28,
              padding: "0 var(--space-md)",
              background: "var(--bg-base)",
              border: "var(--bw-thin) solid var(--border-primary)",
              borderRadius: "var(--radius-sm)",
              color: "var(--text-secondary)",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
              cursor: busy ? "default" : "pointer",
              opacity: busy ? 0.4 : 1,
            }}
          >
            {t("export.cancel")}
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={onExport}
            style={{
              height: 28,
              padding: "0 var(--space-lg)",
              background: "var(--accent-primary)",
              border: "var(--bw-thin) solid var(--accent-primary)",
              borderRadius: "var(--radius-sm)",
              color: "#fff",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
              cursor: busy ? "wait" : "pointer",
              opacity: busy ? 0.7 : 1,
            }}
          >
            {busy ? t("export.exporting") : t("export.run")}
          </button>
        </div>
      </div>
    </div>
  );
}

/** One labelled control row (label left, control right, optional hint below). */
function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "var(--space-md)",
        }}
      >
        <span style={{ fontSize: "var(--fs-sm)", color: "var(--text-secondary)" }}>
          {label}
        </span>
        {children}
      </div>
      {hint && (
        <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary, var(--text-secondary))" }}>
          {hint}
        </span>
      )}
    </div>
  );
}
