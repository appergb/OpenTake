/**
 * Title bar (SPEC §2.8). Leading: Home (return to launcher). Trailing:
 * Settings + Export. (UpdateBadge/Avatar belong to a separate issue.)
 *
 * The Agent panel is toggled from the §2.9 View menu (ViewMenu) and the
 * keyboard shortcut — the dedicated title-bar toggle button was removed by
 * request. Layout presets and panel-visibility toggles also live in the View
 * menu, the in-app menu entry point for an environment without a native menu bar.
 */

import { useEffect, useRef, useState } from "react";
import { Upload, Home, Settings as SettingsIcon, Library, Film, Captions } from "lucide-react";
import { Icon } from "../ui/Icon";
import { ViewMenu } from "./ViewMenu";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { useT } from "../../i18n";
import * as api from "../../lib/api";
import type { SubtitleFormat } from "../../lib/api";
import { saveDialog } from "../../lib/dialog";

const XML_EXT = "xml";

/** Ensure a chosen path carries the `.xml` extension. */
function withXmlExt(path: string): string {
  return path.endsWith(`.${XML_EXT}`) ? path : `${path}.${XML_EXT}`;
}

/**
 * Default export filename: the open project's base name with `.xml`, falling
 * back to "Timeline.xml" for an unsaved project. The bundle path ends in
 * `…/Name.opentake`, so strip the directory and the `.opentake` suffix.
 */
function defaultXmlName(projectPath: string | null): string {
  if (!projectPath) return `Timeline.${XML_EXT}`;
  const base = projectPath.split(/[\\/]/).pop() ?? projectPath;
  const stem = base.replace(/\.opentake$/i, "");
  return `${stem || "Timeline"}.${XML_EXT}`;
}

/** The open project's base name (without the `.opentake` suffix), or "Timeline". */
function projectStem(projectPath: string | null): string {
  if (!projectPath) return "Timeline";
  const base = projectPath.split(/[\\/]/).pop() ?? projectPath;
  return base.replace(/\.opentake$/i, "") || "Timeline";
}

/** Ensure a chosen path carries the given subtitle extension (`srt` / `vtt`). */
function withSubtitleExt(path: string, ext: string): string {
  return path.toLowerCase().endsWith(`.${ext}`) ? path : `${path}.${ext}`;
}

export function TitleBar() {
  const setView = useEditorUiStore((s) => s.setView);
  const setSettingsOpen = useEditorUiStore((s) => s.setSettingsOpen);
  const setExportDialogOpen = useEditorUiStore((s) => s.setExportDialogOpen);
  const pushToast = useEditorUiStore((s) => s.pushToast);
  const projectPath = useProjectStore((s) => s.projectPath);
  const tracks = useProjectStore((s) => s.timeline.tracks);
  const t = useT();

  // Subtitle-format popover (SRT / VTT). Dismiss on outside click / Escape.
  const [subMenuOpen, setSubMenuOpen] = useState(false);
  const subMenuRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!subMenuOpen) return;
    const onDown = (e: MouseEvent) => {
      if (subMenuRef.current && !subMenuRef.current.contains(e.target as Node)) {
        setSubMenuOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setSubMenuOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [subMenuOpen]);

  // Video export needs something to render: disable the entry when no track
  // holds a clip (an empty timeline would only encode black frames).
  const hasClips = tracks.some((track) => track.clips.length > 0);

  /**
   * Export the timeline as Final Cut Pro 7 XML (`.xml`). Mirrors the new-project
   * save flow (`projectActions.newProjectAndEnter`): open the native save panel,
   * default the name to the project, then write via `export_fcpxml`. No-op
   * outside Tauri (no save panel / file system).
   */
  async function onExport(): Promise<void> {
    const save = await saveDialog();
    if (!save) return;
    const dir = projectPath
      ? projectPath.replace(/[\\/][^\\/]*$/, "")
      : await api.getDefaultProjectDir().catch(() => "");
    const sep = dir && !dir.endsWith("/") ? "/" : "";
    const defaultPath = dir
      ? `${dir}${sep}${defaultXmlName(projectPath)}`
      : undefined;

    const chosen = await save({
      title: t("title.exportXmlDialog"),
      defaultPath,
      filters: [{ name: t("title.exportXmlFilter"), extensions: [XML_EXT] }],
    });
    if (typeof chosen !== "string") return; // cancelled
    await api.exportFcpxml(withXmlExt(chosen));
  }

  /**
   * Export the timeline's captions as SubRip (`.srt`) or WebVTT (`.vtt`). Same
   * save-panel flow as the XML export, then `export_subtitles`. The backend
   * returns the cue count: zero means the timeline carries no caption clips, so
   * we surface a friendly "no subtitles" toast instead of a silent empty file.
   */
  async function onExportSubtitles(format: SubtitleFormat): Promise<void> {
    setSubMenuOpen(false);
    const save = await saveDialog();
    if (!save) return; // outside Tauri — no save panel / file system
    const dir = projectPath
      ? projectPath.replace(/[\\/][^\\/]*$/, "")
      : await api.getDefaultProjectDir().catch(() => "");
    const sep = dir && !dir.endsWith("/") ? "/" : "";
    const defaultPath = dir
      ? `${dir}${sep}${projectStem(projectPath)}.${format}`
      : undefined;

    const chosen = await save({
      title: t(format === "srt" ? "title.exportSrtDialog" : "title.exportVttDialog"),
      defaultPath,
      filters: [
        {
          name: t(format === "srt" ? "title.exportSrtFilter" : "title.exportVttFilter"),
          extensions: [format],
        },
      ],
    });
    if (typeof chosen !== "string") return; // cancelled

    try {
      const summary = await api.exportSubtitles(withSubtitleExt(chosen, format), format);
      pushToast(
        summary.cueCount > 0
          ? t("title.exportSubtitlesDone", { count: summary.cueCount })
          : t("title.exportSubtitlesEmpty"),
      );
    } catch {
      pushToast(t("title.exportSubtitlesFailed"));
    }
  }

  return (
    <div
      data-tauri-drag-region
      style={{
        height: 38,
        flex: "0 0 auto",
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        padding: "0 var(--space-md) 0 var(--titlebar-safe-left)",
        background: "var(--bg-base)",
        borderBottom: "var(--bw-thin) solid var(--border-primary)",
      }}
    >
      {/* Leading: Home (return to launcher). */}
      <button
        title={t("title.backHome")}
        aria-label={t("title.backHome")}
        onClick={() => setView("home")}
        className="hover-area"
        style={{
          width: 26,
          height: 26,
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-secondary)",
        }}
      >
        <Icon icon={Home} size={13} />
      </button>

      {/* §2.9 menu entry point (hosts Layout presets + Agent panel + visibility). */}
      <ViewMenu />

      <div data-tauri-drag-region style={{ flex: 1 }} />

      {/* Trailing: Library + Settings + Export. */}
      <button
        title={t("library.entry")}
        aria-label={t("library.entry")}
        onClick={() => setView("library")}
        className="hover-area"
        style={{
          width: 26,
          height: 26,
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-secondary)",
        }}
      >
        <Icon icon={Library} size={13} />
      </button>
      <button
        title={t("title.settings")}
        aria-label={t("title.settings")}
        onClick={() => setSettingsOpen(true)}
        className="hover-area"
        style={{
          width: 26,
          height: 26,
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-secondary)",
        }}
      >
        <Icon icon={SettingsIcon} size={13} />
      </button>
      <button
        title={hasClips ? t("title.exportVideoHint") : t("title.exportVideoEmpty")}
        aria-label={t("title.exportVideo")}
        disabled={!hasClips}
        onClick={() => setExportDialogOpen(true)}
        className={hasClips ? "hover-area" : undefined}
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 4,
          height: 26,
          padding: "0 var(--space-sm)",
          color: "var(--text-secondary)",
          fontSize: "var(--fs-sm)",
          fontWeight: "var(--fw-medium)",
          cursor: hasClips ? "pointer" : "default",
          opacity: hasClips ? 1 : 0.4,
        }}
      >
        <Icon icon={Film} size={13} />
        {t("title.exportVideo")}
      </button>

      {/* Subtitle export (.srt / .vtt) with a small format popover. */}
      <div ref={subMenuRef} style={{ position: "relative", display: "inline-flex" }}>
        <button
          title={t("title.exportSubtitlesHint")}
          aria-label={t("title.exportSubtitles")}
          aria-haspopup="menu"
          aria-expanded={subMenuOpen}
          onClick={() => setSubMenuOpen((v) => !v)}
          className="hover-area"
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            height: 26,
            padding: "0 var(--space-sm)",
            color: "var(--text-secondary)",
            fontSize: "var(--fs-sm)",
            fontWeight: "var(--fw-medium)",
          }}
        >
          <Icon icon={Captions} size={13} />
          {t("title.exportSubtitles")}
        </button>
        {subMenuOpen && (
          <div
            role="menu"
            aria-label={t("title.exportSubtitles")}
            style={{
              position: "absolute",
              top: "calc(100% + 4px)",
              right: 0,
              zIndex: 1200,
              minWidth: 200,
              display: "flex",
              flexDirection: "column",
              padding: "var(--space-xs)",
              background: "var(--bg-elevated)",
              border: "var(--bw-thin) solid var(--border-primary)",
              borderRadius: "var(--radius-sm)",
              boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
            }}
          >
            {(["srt", "vtt"] as const).map((fmt) => (
              <button
                key={fmt}
                role="menuitem"
                onClick={() => void onExportSubtitles(fmt)}
                className="hover-area"
                style={{
                  display: "flex",
                  alignItems: "center",
                  height: 28,
                  padding: "0 var(--space-sm)",
                  background: "transparent",
                  border: "none",
                  borderRadius: "var(--radius-xs-sm)",
                  color: "var(--text-primary)",
                  fontSize: "var(--fs-sm)",
                  textAlign: "left",
                  cursor: "pointer",
                }}
              >
                {t(fmt === "srt" ? "title.exportSrt" : "title.exportVtt")}
              </button>
            ))}
          </div>
        )}
      </div>

      <button
        title={t("title.exportHint")}
        aria-label={t("title.export")}
        onClick={onExport}
        className="hover-area"
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 4,
          height: 26,
          padding: "0 var(--space-sm)",
          color: "var(--text-secondary)",
          fontSize: "var(--fs-sm)",
          fontWeight: "var(--fw-medium)",
        }}
      >
        <Icon icon={Upload} size={13} />
        {t("title.export")}
      </button>
    </div>
  );
}
