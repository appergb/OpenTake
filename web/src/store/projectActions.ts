/**
 * Project lifecycle gestures driven from the Home view. "New" starts a fresh
 * session and enters the editor; "Open" picks an `.opentake` bundle (a directory
 * on disk) via the native dialog, opens it in the core, records it in recents,
 * and enters the editor. All paths degrade gracefully outside Tauri so the
 * browser shell can still navigate into the editor.
 */

import * as api from "../lib/api";
import { forceRefresh } from "./sync";
import { useEditorUiStore } from "./uiStore";
import { useProjectStore } from "./projectStore";
import { useRecentStore } from "./recentStore";
import { openDialog, saveDialog } from "../lib/dialog";
import { t } from "../i18n";

const PROJECT_EXT = "opentake";

/** Ensure a chosen path carries the `.opentake` bundle extension. */
function withExt(path: string): string {
  return path.endsWith(`.${PROJECT_EXT}`) ? path : `${path}.${PROJECT_EXT}`;
}

/**
 * New project. Mirrors upstream `AppState.createNewProject` (`NSSavePanel`):
 * prompt for a save location + name (default `~/Documents/OpenTake`), then
 * create the session and **immediately write the `.opentake` bundle to disk** so
 * the project has a real location (the user's complaint was "new project can't
 * choose where it saves"). Records it in recents and enters the editor.
 *
 * Outside Tauri (browser shell) there is no save panel — fall back to a fresh
 * in-memory session so the UI is still explorable.
 */
export async function newProjectAndEnter(): Promise<void> {
  const save = await saveDialog();
  if (!save) {
    await api.projectNew();
    await forceRefresh();
    useEditorUiStore.getState().setView("editor");
    return;
  }

  const defaultDir = await api.getDefaultProjectDir().catch(() => "");
  const sep = defaultDir && !defaultDir.endsWith("/") ? "/" : "";
  const defaultPath = defaultDir
    ? `${defaultDir}${sep}${t("home.untitled")}.${PROJECT_EXT}`
    : undefined;

  const chosen = await save({
    title: t("home.newProject"),
    defaultPath,
    filters: [{ name: "OpenTake", extensions: [PROJECT_EXT] }],
  });
  if (typeof chosen !== "string") return; // cancelled

  const path = withExt(chosen);
  await api.projectNew();
  await api.projectSave(path);
  useProjectStore.getState().setProjectPath(path);
  useProjectStore.getState().markSaved();
  useRecentStore.getState().add(path);
  await forceRefresh();
  useEditorUiStore.getState().setView("editor");
}

/**
 * Save the open project back to its bundle (`project_save(None)`). Used by the
 * Cmd/Ctrl+S shortcut and the debounced autosave. No-op when no project is open
 * (Home view) or outside Tauri. The backend already knows the bundle path from
 * the initial save, so no path is passed. Best-effort: a failure leaves the
 * dirty state so the next autosave/Cmd+S retries.
 */
export async function saveCurrentProject(): Promise<void> {
  const { projectPath } = useProjectStore.getState();
  if (!projectPath) return;
  try {
    await api.projectSave(null);
    useProjectStore.getState().markSaved();
  } catch {
    // Keep the document dirty so a later save retries; surfaced via UI later.
  }
}

/** Open `path` (a `.opentake` bundle), refresh the mirror, record it, and enter
 *  the editor. Used by both the dialog flow and the recents list. */
export async function openProjectPath(path: string): Promise<void> {
  const snap = await api.projectOpen(path);
  useProjectStore.getState().setMirror(snap.timeline, snap.version);
  useProjectStore.getState().setProjectPath(path);
  useProjectStore.getState().markSaved();
  useRecentStore.getState().add(path);
  useEditorUiStore.getState().setView("editor");
}

/** Pick a project bundle with the native dialog, then open it. `.opentake`
 *  bundles are directories, so the picker is a directory chooser (mirrors
 *  upstream's package-as-folder open panel). */
export async function openProjectViaDialog(): Promise<void> {
  const open = await openDialog();
  if (!open) {
    // Browser shell: no file system. Just enter the editor on the demo mirror.
    useEditorUiStore.getState().setView("editor");
    return;
  }
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected !== "string") return; // cancelled
  await openProjectPath(selected);
}
