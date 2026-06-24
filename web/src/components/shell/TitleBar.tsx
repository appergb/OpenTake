/**
 * Title bar (SPEC §2.8). Leading: Home (return to launcher). Trailing:
 * Settings + Export. (UpdateBadge/Avatar belong to a separate issue.)
 *
 * The Agent panel is toggled from the §2.9 View menu (ViewMenu) and the
 * keyboard shortcut — the dedicated title-bar toggle button was removed by
 * request. Layout presets and panel-visibility toggles also live in the View
 * menu, the in-app menu entry point for an environment without a native menu bar.
 */

import { Upload, Home, Settings as SettingsIcon, Library } from "lucide-react";
import { Icon } from "../ui/Icon";
import { ViewMenu } from "./ViewMenu";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { useT } from "../../i18n";
import * as api from "../../lib/api";
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

export function TitleBar() {
  const setView = useEditorUiStore((s) => s.setView);
  const projectPath = useProjectStore((s) => s.projectPath);
  const t = useT();

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

      <div style={{ flex: 1 }} />

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
        onClick={() => setView("settings")}
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
