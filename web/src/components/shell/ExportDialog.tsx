/**
 * ExportDialog (SPEC §2.4 / #112). Modal shown from the title bar to render the
 * whole timeline to a real video file via the `export_video` backend command
 * (per-frame GPU composite → ffmpeg + AAC/LPCM mux).
 *
 * Scope mirrors the backend:
 *  - Format: H.264 / H.265 (`.mp4`) and ProRes 422 (`.mov`). The output path's
 *    extension tracks the selected codec so it always matches what the
 *    backend's `resolve_preset` requires (see `extForCodec`/`withExt` below).
 *  - Resolution: 720p / 1080p / 4K short-edge presets. The default pre-selects
 *    the preset matching the timeline's own shorter edge so a standard project
 *    round-trips its native size; it falls back to 1080p (the backend default).
 *
 * Progress + cancel (mirrors upstream's 200ms `AVAssetExportSession.progress`
 * poll + cooperative cancel): while busy, a determinate bar tracks the
 * `"export://progress"` event (`done`/`total` frames) and the footer's Cancel
 * button is enabled, calling `api.cancelExport()`. A cancelled result closes
 * the dialog with a neutral toast, distinct from the failure toast. Success /
 * failure both still `pushToast` and close on success.
 */

import { useEffect, useMemo, useRef, useState } from "react";
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
const MOV_EXT = "mov";

/** The container extension the backend's `resolve_preset` requires for a codec. */
export function extForCodec(codec: ExportCodec): typeof MP4_EXT | typeof MOV_EXT {
  return codec === "prores" ? MOV_EXT : MP4_EXT;
}

/** Ensure a chosen path carries the given extension (does not strip a wrong one). */
export function withExt(path: string, ext: string): string {
  return path.toLowerCase().endsWith(`.${ext}`) ? path : `${path}.${ext}`;
}

/** Ensure a chosen path carries the `.mp4` extension (the H.264 container). */
export function withMp4Ext(path: string): string {
  return withExt(path, MP4_EXT);
}

/**
 * Default export filename: the open project's base name with the codec's
 * container extension, falling back to "Timeline.<ext>" for an unsaved
 * project. The bundle path ends in `…/Name.opentake`, so strip the directory
 * and the `.opentake` suffix.
 */
export function defaultExportName(projectPath: string | null, ext: string): string {
  if (!projectPath) return `Timeline.${ext}`;
  const base = projectPath.split(/[\\/]/).pop() ?? projectPath;
  const stem = base.replace(/\.opentake$/i, "");
  return `${stem || "Timeline"}.${ext}`;
}

/** Default export filename for the `.mp4` container (H.264 / H.265). */
export function defaultMp4Name(projectPath: string | null): string {
  return defaultExportName(projectPath, MP4_EXT);
}

/** Pick the preset whose short edge best matches the timeline's shorter side. */
export function defaultQuality(width: number, height: number): ExportQuality {
  const shortEdge = Math.min(width, height);
  if (shortEdge >= 1620) return "4k"; // ≥ 1620 rounds to the 2160 bucket
  if (shortEdge <= 840) return "720p"; // ≤ 840 rounds to the 720 bucket
  return "1080p";
}

/** Format an `{done, total}` progress event as a clamped whole-number percent
 *  (0-100). `total <= 0` (not yet known, or a zero-frame timeline) reports 0
 *  rather than dividing by zero. */
export function progressPercent(done: number, total: number): number {
  if (total <= 0) return 0;
  const pct = Math.round((done / total) * 100);
  return Math.min(100, Math.max(0, pct));
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
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  // Guards against unsubscribing a listener from a stale/overlapping export run
  // (belt-and-suspenders; only one export runs at a time in practice).
  const progressUnlisten = useRef<(() => void) | null>(null);

  // Re-seed the resolution default from the timeline each time the dialog opens.
  useEffect(() => {
    if (open) {
      setQuality(defaultQuality(timeline.width, timeline.height));
      setError(null);
      setProgress(null);
    }
  }, [open, timeline.width, timeline.height]);

  // Safety net: unsubscribe on unmount even if a run is somehow still in
  // flight (the normal path unsubscribes in `onExport`'s `finally`).
  useEffect(() => {
    return () => {
      progressUnlisten.current?.();
      progressUnlisten.current = null;
    };
  }, []);

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
      { id: "h265" as const, label: t("export.codec.h265") },
      { id: "prores" as const, label: t("export.codec.prores") },
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
    const ext = extForCodec(codec);
    const projectPath = useProjectStore.getState().projectPath;
    const dir = projectPath
      ? projectPath.replace(/[\\/][^\\/]*$/, "")
      : await api.getDefaultProjectDir().catch(() => "");
    const sep = dir && !dir.endsWith("/") ? "/" : "";
    const defaultPath = dir
      ? `${dir}${sep}${defaultExportName(projectPath, ext)}`
      : undefined;

    const chosen = await save({
      title: t("export.saveDialog"),
      defaultPath,
      filters: [
        {
          name: t(ext === MOV_EXT ? "export.saveFilterMov" : "export.saveFilter"),
          extensions: [ext],
        },
      ],
    });
    if (typeof chosen !== "string") return; // cancelled

    setBusy(true);
    setProgress(null);
    progressUnlisten.current = await api.onExportProgress(({ done, total }) => {
      setProgress({ done, total });
    });
    try {
      const summary = await api.exportVideo({
        outPath: withExt(chosen, ext),
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
      if (message === api.EXPORT_CANCELLED_SENTINEL) {
        // User-initiated cancel: neutral toast, not the failure path.
        pushToast(t("export.cancelled"));
        setOpen(false);
      } else {
        setError(message);
        pushToast(t("export.failed"));
      }
    } finally {
      progressUnlisten.current?.();
      progressUnlisten.current = null;
      setProgress(null);
      setBusy(false);
    }
  }

  async function onCancel(): Promise<void> {
    if (!busy) {
      setOpen(false);
      return;
    }
    await api.cancelExport();
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

          {busy && (
            <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
              <div
                role="progressbar"
                aria-label={t("export.title")}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={progressPercent(progress?.done ?? 0, progress?.total ?? 0)}
                style={{
                  height: 6,
                  borderRadius: "var(--radius-xs-sm)",
                  background: "var(--bg-base)",
                  overflow: "hidden",
                }}
              >
                <div
                  style={{
                    height: "100%",
                    width: `${progressPercent(progress?.done ?? 0, progress?.total ?? 0)}%`,
                    background: "var(--accent-primary)",
                    transition: "width 150ms var(--ease-out-expo, ease-out)",
                  }}
                />
              </div>
              <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-secondary)" }}>
                {t("export.progress", {
                  percent: progressPercent(progress?.done ?? 0, progress?.total ?? 0),
                })}
              </span>
            </div>
          )}

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
            onClick={onCancel}
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
              cursor: "pointer",
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
